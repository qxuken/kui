//! Window chrome as data. A node can declare a chrome role (drag handle,
//! close/minimize/maximize button); interacting with it produces
//! [`WindowCommand`]s that the frame driver drains and applies to the real
//! window. Host window facts flow back in through [`WindowEnv`] on `Env`.
//! The core never touches a window — headless drivers just never drain.

use crate::geom::Rect;

/// Role a node plays in window chrome (set via `NodeSpec::window_drag` /
/// `NodeSpec::window_button`). Chrome nodes never emit `UiEvent`s — their
/// interactions become [`WindowCommand`]s for the driver instead.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowRole {
    /// Mouse-down here asks the driver to start an OS window drag (drivers
    /// conventionally promote a quick second press to a maximize toggle).
    Drag,
    /// Clicking here emits the button's window command.
    Button(WindowButton),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowButton {
    Close,
    Minimize,
    Maximize,
}

impl WindowButton {
    pub fn command(self) -> WindowCommand {
        match self {
            WindowButton::Close => WindowCommand::Close,
            WindowButton::Minimize => WindowCommand::Minimize,
            WindowButton::Maximize => WindowCommand::ToggleMaximize,
        }
    }
}

/// A window-level intent produced by input on chrome nodes. Drained by the
/// frame driver via `Core::take_window_commands` after each input dispatch.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WindowCommand {
    /// Begin an interactive OS move (the press landed on a `Drag` node).
    StartDrag,
    Close,
    Minimize,
    ToggleMaximize,
}

/// What the host knows about its window, pushed into `Env` by the frame
/// driver. Views (e.g. `widgets::titlebar`) read this to adapt: reserve
/// space for native controls, pick the maximize/restore glyph, or render
/// nothing at all under native decorations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowEnv {
    /// The host asked the app to draw its own chrome (no native titlebar).
    pub custom_chrome: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    /// Area (logical px, window coords) covered by controls the OS still
    /// draws over our content — macOS traffic lights under custom chrome.
    /// Views keep out of it; `None` means the OS draws nothing over us.
    pub native_controls: Option<Rect>,
}
