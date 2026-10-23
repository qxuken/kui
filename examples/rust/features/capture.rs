//! An app keeping its own window as a file (ADR 0055): a "Save a
//! screenshot" button, a "Copy screenshot" one and a "Record 5 seconds"
//! toggle beside a spinner. The click's handler asks the core —
//! `capture_to` for a PNG, `capture_to_clipboard` for the clipboard,
//! `record_to` for an animated PNG, `stop_recording` to end one early —
//! and the runner serves the ask after the next frame it draws, drawing
//! that frame again on a GPU device of its own. What comes back is a
//! `capture` event in the message the ask named, with the file's path,
//! size and frames in it, never the pixels. The recording stops itself:
//! the handler asks for a frame five seconds on, and the view that runs
//! then says stop. The files go to the system's temporary directory, and
//! the status line names them.
//!
//! Run: cargo run -p kui-native --example capture [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{
    App, CaptureCommand, CapturePhase, Core, Easing, Event, Keyframe, Message, NodeSpec, Repeat,
    TextStyle, Transition, Ui, UiEvent, widgets,
};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Shot,
    Copy,
    Record,
    /// What a still's `capture` event arrives as.
    Saved,
    /// And a recording's.
    Recorded,
}

struct Keeper {
    dir: std::path::PathBuf,
    shots: u32,
    /// When the running recording stops itself, on the frame clock.
    until: Option<f64>,
    status: String,
}

impl Keeper {
    fn path(&self, name: &str) -> String {
        self.dir.join(name).to_string_lossy().into_owned()
    }
}

impl App for Keeper {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // The frame asked for five seconds on: its view ends the
        // recording, and this frame is the one left out.
        if let Some(until) = self.until
            && ui.now() >= until
        {
            ui.stop_recording();
            self.until = None;
        }
        let t = ui.theme();
        ui.with(
            NodeSpec::column().fill().pad(24.0).gap(16.0).bg(t.bg),
            |ui| {
                ui.leaf_keyed(
                    "spinner",
                    NodeSpec::column()
                        .size(48.0, 48.0)
                        .radius(10.0)
                        .bg(t.accent)
                        .transition_with(
                            Transition::ms(1600.0)
                                .easing(Easing::Linear)
                                .repeat(Repeat::Normal),
                        )
                        .keyframes(vec![
                            Keyframe::default().rotate(0.0),
                            Keyframe::default().rotate(1.0),
                        ]),
                );
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    widgets::button(ui, "Save a screenshot", Msg::Shot);
                    widgets::button(ui, "Copy screenshot", Msg::Copy);
                    let record = if self.until.is_some() {
                        "Stop recording"
                    } else {
                        "Record 5 seconds"
                    };
                    widgets::button(ui, record, Msg::Record);
                });
                ui.text_in_keyed(
                    "status",
                    NodeSpec::row().grow_width(),
                    &self.status,
                    TextStyle::new(12.0).mono().color(t.muted),
                );
            },
        );
    }

    fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
        match ev.split::<Msg>() {
            (Some(Msg::Shot), _) => {
                self.shots += 1;
                let path = self.path(&format!("kui-capture-{}.png", self.shots));
                core.capture_to(path, Some(Msg::Saved.into()));
            }
            (Some(Msg::Copy), _) => core.capture_to_clipboard(Some(Msg::Saved.into())),
            (Some(Msg::Record), _) if self.until.is_some() => {
                core.stop_recording();
                self.until = None;
            }
            (Some(Msg::Record), _) => {
                core.record_to(self.path("kui-capture.apng"), Some(Msg::Recorded.into()));
                let until = core.now() + 5.0;
                core.request_frame_at(until);
                self.until = Some(until);
                self.status = "recording…".into();
            }
            (Some(Msg::Saved | Msg::Recorded), Event::Capture(c)) => {
                self.status = match c.phase {
                    CapturePhase::Saved if c.frames > 1 => format!(
                        "{} frames, {:.1} s, {}x{}: {}",
                        c.frames, c.seconds, c.width, c.height, c.path
                    ),
                    CapturePhase::Saved if c.path.is_empty() => {
                        format!("{}x{}: on the clipboard", c.width, c.height)
                    }
                    CapturePhase::Saved => format!("{}x{}: {}", c.width, c.height, c.path),
                    CapturePhase::Failed => {
                        format!("not written: {}", c.error.unwrap_or_default())
                    }
                };
            }
            _ => {}
        }
    }
}

impl Example for Keeper {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(520.0, 220.0)
    }

    /// The asks as data, with no GPU and no file: what each button queues
    /// and what the recording's own stop is.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 520.0, 220.0);
        d.frame(self);
        let warnings = d.warnings();
        d.check(warnings.is_empty(), "no warnings")?;
        let shot = d
            .key_named("Save a screenshot")
            .ok_or("no screenshot button")?;
        d.click_key(self, shot);
        let asked = d.core.take_capture_commands();
        d.check(
            matches!(&asked[..], [CaptureCommand::Still { path, message: Some(m) }]
                if path.ends_with("kui-capture-1.png") && *m == Msg::Saved.into()),
            "the button asks for a still, answered as Saved",
        )?;
        let owed = d.core.animating();
        d.check(owed, "and for the frame it is served after")?;
        let copy = d.key_named("Copy screenshot").ok_or("no copy button")?;
        d.click_key(self, copy);
        let asked = d.core.take_capture_commands();
        d.check(
            matches!(&asked[..], [CaptureCommand::Clipboard { message: Some(_) }]),
            "the copy button asks for the clipboard",
        )?;
        let record = d.key_named("Record 5 seconds").ok_or("no record button")?;
        d.click_key(self, record);
        let asked = d.core.take_capture_commands();
        d.check(
            matches!(&asked[..], [CaptureCommand::Record { path, .. }] if path.ends_with(".apng")),
            "the toggle asks for an animated PNG",
        )?;
        let due = d.core.next_frame_at().ok_or("no stop asked for")?;
        d.check((due - 5.0).abs() < 1e-9, "and for a frame five seconds on")?;
        d.frame(self);
        let stop = d.key_named("Stop recording").is_some();
        d.check(stop, "the toggle reads Stop")?;
        d.advance(5.0);
        d.frame(self);
        let asked = d.core.take_capture_commands();
        d.check(
            asked == [CaptureCommand::Stop],
            "the frame five seconds on stops the recording",
        )?;
        d.frame(self);
        let again = d.key_named("Record 5 seconds").is_some();
        d.check(again, "and the toggle reads Record again")
    }
}

kui_devtools::main!(Keeper {
    dir: std::env::temp_dir(),
    shots: 0,
    until: None,
    status: "nothing kept yet".into(),
});
