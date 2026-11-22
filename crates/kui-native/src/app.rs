use crate::{Core, InputEvent, Ui, UiEvent, Waker};

/// What a kui application implements: a `view` that declares each frame
/// and an `on_event` that answers what the frame produced.
///
/// Only [`view`](Self::view) is required. Hand a value of the type to
/// `Launcher::run`, or to `testing::Drive` in a test.
pub trait App {
    /// Observes a translated input before routing it. Returning true consumes
    /// it. Presentation-only hosts use this to forward input to another core.
    fn on_input(&mut self, _input: &InputEvent) -> bool {
        false
    }

    /// Builds one window's frame. Called once per open window per frame;
    /// `ui.window_name()` says which (`"main"` for the launcher's).
    fn view(&mut self, ui: &mut Ui<'_>);
    /// Receives one event the frame produced: a click's message, a key
    /// press, a resize, a window event. Called after the frame, once per
    /// event; [`UiEvent::message`] reads a `#[derive(Message)]` back out.
    /// The default ignores every event.
    fn on_event(&mut self, _ev: UiEvent) {}
    /// [`Self::on_event`] with the [`Core`] of the window the event came
    /// from. Override it when answering an event means calling the core:
    /// writing the clipboard, asking for a paste, moving focus, revealing a
    /// row, asking for a frame. A clipboard write goes out with this turn's
    /// actions; a focus move or a reveal is seen by the next frame. The
    /// default calls `on_event`, so an app that overrides only that one is
    /// unchanged.
    fn on_event_with(&mut self, ev: UiEvent, _core: &mut Core) {
        self.on_event(ev)
    }
    /// Called after an input and its UI events, before applying the native
    /// cursor, even if the input produced no UI event. Also called after
    /// native window focus changes (which can release held modifiers).
    /// Update input-dependent output such as [`Core::set_cursor`] here,
    /// against the last finished layout, without waiting for a rendered
    /// frame. The default does nothing.
    fn after_input(&mut self, _core: &mut Core) {}
    /// Called once, before the window opens, with a [`Waker`] the app may
    /// clone into any thread. The loop parks between events, so a thread
    /// that changed what `view` will show (a PTY reader, a file watcher, a
    /// socket) calls [`Waker::wake`] to get a frame drawn. The default
    /// drops the waker.
    fn setup(&mut self, _waker: Waker) {}
    /// Called on every [`Waker::wake`] as the loop takes it, before the
    /// frame it asks for — which waits for the display's next refresh.
    /// For work a thread is waiting on that needs no `Ui`: answering a
    /// request a server thread queued, handing a reply to a task. The
    /// frame still follows. The default does nothing.
    fn on_wake(&mut self) {}
    /// Called immediately before a window's frame starts, with the previous
    /// frame's event tree still intact. A host can drain queued input through
    /// `Core::handle_input` here and deliver its events through `on_event_with`.
    /// Do not dispatch input from `view`: starting that frame resets the tree.
    /// Called by the native and headless runners, including while replaying a
    /// cached view. Input and other preparation here are timed as input work.
    fn before_frame(&mut self, _core: &mut Core) {}
    /// A revision of everything [`Self::view`] reads that the core does
    /// not see — the app's model, what its threads left it — moved
    /// whenever any of it may have (backlog C138). `Some` lets the runner
    /// build a frame only an animation of the core's own asks for (a
    /// keyframe cycle, a transition) from the last frame's tree without
    /// calling `view`, while the revision, every extension root's state
    /// (`Extension::root_state`) and every fact of the frame the build
    /// read stay put (`Core::reframe`). `None`, the default, calls `view`
    /// for every frame.
    fn view_state(&self) -> Option<u64> {
        None
    }
    /// Called once, when the main window is going for good (its close
    /// button, Quit, `WindowCommand::Close` on it, a pumped runner ending),
    /// before `run` returns. Save a session, a draft or a position here;
    /// the frame is over, there is no `Ui` and nothing draws. A crash under
    /// `run` does not reach it; a panic unwinding through a `PumpRunner`
    /// does, and a `teardown` that panics there aborts. The default does
    /// nothing.
    fn teardown(&mut self) {}
}
