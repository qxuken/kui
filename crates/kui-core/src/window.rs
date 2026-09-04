//! Window chrome as data. A node can declare a chrome role (drag handle,
//! close/minimize/maximize button); interacting with it produces
//! [`WindowCommand`]s that the frame driver drains and applies to the real
//! window. Host window facts flow back in through [`WindowEnv`] on `Env`.
//! The core never touches a window — headless drivers just never drain.

use crate::geom::Rect;

/// Which OS window something belongs to: an opaque integer the **driver**
/// assigns, not a handle an app builds. [`WindowId::MAIN`] is 0 — the window
/// the launcher opens, and the only one that exists until ADR 0004's step 3
/// lets a frame declare more.
///
/// It crosses every transport as a plain integer — `UiEvent::window` in
/// Rust, `window` on a JSX `UiEvent`, `KuiEvent.window` in C — so nothing
/// has to model window identity twice.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct WindowId(pub u32);

impl WindowId {
    /// The window the app starts in, and the answer everywhere until a
    /// driver opens a second one.
    pub const MAIN: WindowId = WindowId(0);
}

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
    /// Which window this core is drawing, assigned by the driver. A view
    /// reads it here rather than through a query, and it is what the core
    /// stamps onto every `UiEvent` it hands out — so a driver that pushes
    /// the rest of these facts has already said where its events came from.
    pub id: WindowId,
    /// The host asked the app to draw its own chrome (no native titlebar).
    pub custom_chrome: bool,
    pub maximized: bool,
    pub fullscreen: bool,
    /// Area (logical px, window coords) covered by controls the OS still
    /// draws over our content — macOS traffic lights under custom chrome.
    /// Views keep out of it; `None` means the OS draws nothing over us.
    pub native_controls: Option<Rect>,
}
