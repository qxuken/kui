//! Captures and recordings through a core (ADR 0055): what the app asks
//! a driver to keep of this window, queued as data, and the `capture`
//! event the driver's answer becomes. Nothing here touches a GPU or a
//! file; `kui_native::Captures` is what serves the queue.

use super::*;
use crate::captures::CaptureCommand;

impl Core {
    /// Asks for the next frame this window draws, written to `path` as one
    /// picture: `.png`, `.jpg` / `.jpeg` (quality 90, alpha dropped),
    /// `.webp` (lossless) or `.gif`. The driver answers with a `capture`
    /// event in `message` — `capture: {phase: "saved", path, width,
    /// height, frames: 1}`, or `phase: "failed"` with an `error` — and
    /// with no message writes the file and says nothing. Called from a
    /// view, the frame is the one that view builds; between frames, it
    /// asks for one. Queued until a driver serves it
    /// ([`Self::take_capture_commands`] drains the queue).
    #[track_caller]
    pub fn capture_to(&mut self, path: impl Into<String>, message: Option<Value>) {
        self.ask_capture(CaptureCommand::Still {
            path: path.into(),
            message,
        });
    }

    /// Asks for every frame this window draws from the next one on,
    /// written to `path` as an animation when [`Self::stop_recording`]
    /// ends it: `.png` / `.apng` (an animated PNG) or `.gif`, playing for
    /// ever. A frame shows from the frame-clock time it was drawn until
    /// the next frame drawn, the last until the stop. The answer is a
    /// `capture` event in `message`, as [`Self::capture_to`]'s is, once the
    /// file is written, with the `frames` it holds and its `seconds`. A
    /// recording asked for while one runs fails, naming that one. A window
    /// that closes while recording has its file written and is not heard.
    #[track_caller]
    pub fn record_to(&mut self, path: impl Into<String>, message: Option<Value>) {
        self.ask_capture(CaptureCommand::Record {
            path: path.into(),
            message,
        });
    }

    /// Asks for the next frame this window draws, put on the system
    /// clipboard as a picture — "Copy screenshot". The driver captures it
    /// as [`Self::capture_to`] does and queues it as a
    /// [`MenuAction::SetClipboardImage`](crate::menu::MenuAction::SetClipboardImage)
    /// (which the runner applies, and a host draining
    /// [`Self::take_menu_actions`] applies itself), then answers in
    /// `message` with `capture: {phase: "saved", path: "", width, height,
    /// frames: 1}`, or `"failed"` with an `error`.
    #[track_caller]
    pub fn capture_to_clipboard(&mut self, message: Option<Value>) {
        self.ask_capture(CaptureCommand::Clipboard { message });
    }

    /// Ends this window's recording with the next frame drawn, which is
    /// its last, and has its file written. Nothing, with none running.
    #[track_caller]
    pub fn stop_recording(&mut self) {
        self.ask_capture(CaptureCommand::Stop);
    }

    /// Drains the captures asked for since the last drain, in order. A
    /// driver serves them after a drawn frame; a headless test asserts
    /// that a button asked, with no GPU and no file.
    pub fn take_capture_commands(&mut self) -> Vec<CaptureCommand> {
        std::mem::take(&mut self.capture_commands)
    }

    /// The driver reports what it wrote for an ask that named `message`:
    /// a `capture` event, pending like a `resize` (see
    /// `take_pending_events`). An ask with no message reports nothing.
    pub fn captured(&mut self, message: Option<Value>, result: crate::event::Captured) {
        let Some(message) = message else { return };
        use crate::message::MessageField;
        self.pending.push(UiEvent::declared(
            crate::input::EventKind::Capture,
            OriginId::HOST,
            Key::ROOT,
            Some(&message),
            result.to_value(),
        ));
    }

    #[track_caller]
    fn ask_capture(&mut self, cmd: CaptureCommand) {
        self.taint_kept("it asked for a capture");
        self.capture_commands.push(cmd);
        // A frame being built is the one served; between frames, one is
        // drawn for the ask rather than waiting for the next input.
        if !self.building {
            self.request_frame();
        }
    }
}
