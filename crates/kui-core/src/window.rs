//! Windows as data. A node can declare a chrome role (drag handle,
//! close/minimize/maximize button); interacting with it produces
//! [`WindowCommand`]s that the frame driver drains and applies to the real
//! window. A frame can declare that a *window exists* (`Core::declare_window`,
//! `docs/adr/0004-multi-window.md`): the core diffs the declared set and the
//! same queue carries the [`WindowCommand::Open`] / [`WindowCommand::Close`]
//! the diff produces. A declared window is a [`WindowKind::Normal`] one or
//! a [`WindowKind::Popup`] — borderless, owned, anchored, non-activating —
//! and a driver reports a popup dismissed the way it reports one closed
//! ([`DismissReason`]). Host window facts flow back in through [`WindowEnv`]
//! on `Env`. The core never touches a window — headless drivers just never
//! drain.

use crate::geom::{Rect, Size};
use crate::tree::OriginId;

/// Which OS window something belongs to: an opaque integer the core's
/// declaration diff assigns when it opens a window, not a handle an app
/// builds. [`WindowId::MAIN`] is 0 — the window the launcher opens, which is
/// always live. Apps name windows with a stable string
/// (`Core::declare_window`); the id is how the driver and the events refer
/// to the surface that string opened.
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
    /// The command a click on this button issues, for the window the
    /// button was drawn in.
    pub fn command(self, window: WindowId) -> WindowCommand {
        match self {
            WindowButton::Close => WindowCommand::Close(window),
            WindowButton::Minimize => WindowCommand::Minimize(window),
            WindowButton::Maximize => WindowCommand::ToggleMaximize(window),
        }
    }
}

/// What kind of OS surface a declared window is
/// (`docs/adr/0004-multi-window.md`, decision 9).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum WindowKind {
    /// A regular top-level window with the launcher's chrome.
    #[default]
    Normal,
    /// A menu surface: borderless, absent from the taskbar, owned by the
    /// window that declared it and closed when that window closes, placed
    /// in screen coordinates against [`WindowConfig::anchor`] rather than
    /// clamped into a viewport, and **non-activating** by default — it
    /// must not take OS focus, or opening a combobox would blur the field
    /// that opened it. While a non-activating popup is up the driver
    /// routes its owner's keyboard input to it and the owner's
    /// `env.focused` stays true, so the field still draws focused while
    /// the arrow keys walk the list.
    ///
    /// Reach for it only for the three placements a float cannot make: a
    /// list taller than the window, a menu near an edge with nowhere
    /// in-window to go, and a panel the user wants beside the app.
    /// Everything else is cheaper as a float — see
    /// [`crate::spec::FloatConfig::fit`] — because a float costs one tree
    /// and one draw call where this costs an OS surface, a swapchain, a
    /// `Core` and an accessibility adapter.
    Popup,
}

/// What a frame says about a window it declares (`Core::declare_window`).
/// Plain data by ADR 0004 decision 5 — no title, no callbacks — so a
/// `WindowCommand` stays `Copy` and equality is derived, which is how the
/// diff tells two declarations of one name apart.
///
/// **Read on the opening edge only.** The config that reaches
/// [`WindowCommand::Open`] is the one the declaration carried on the frame
/// it started; a live window's config is never looked at again, so
/// re-declaring `"palette"` at a new size does not resize it. The user owns
/// a window's geometry once it exists.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WindowConfig {
    pub kind: WindowKind,
    /// Initial inner size, logical px.
    pub size: Size,
    /// Whether opening it takes OS focus. True for a normal window;
    /// [`WindowConfig::popup`] turns it off, so the field that opened a
    /// dropdown keeps the ring while the list is up.
    pub activates: bool,
    /// [`WindowKind::Popup`] only: what the popup is placed against, as a
    /// rect in the **declaring window's** logical viewport coordinates —
    /// which is exactly the rect an `onLayout` node reports, so an app
    /// needs no new geometry query to fill it. The driver resolves it to
    /// screen coordinates against the owner's own position and puts the
    /// popup below it, flipping above when the display's bottom edge is
    /// nearer than the popup is tall.
    ///
    /// Read on the opening edge with the rest of the config and never
    /// again: a popup that has to follow a moving anchor stops being
    /// declared and is declared again, which is what a dropdown does when
    /// its field scrolls away anyway. Ignored by a `Normal` window.
    pub anchor: Rect,
}

impl WindowConfig {
    /// The size a declaration that names none gets.
    pub const DEFAULT_SIZE: Size = Size { w: 640.0, h: 480.0 };

    /// A normal, activating window of `w` x `h` logical px.
    pub fn sized(w: f32, h: f32) -> Self {
        Self {
            size: Size::new(w, h),
            ..Self::default()
        }
    }

    /// A [`WindowKind::Popup`] of `w` x `h` logical px placed against
    /// `anchor` — the rect, in the declaring window's viewport
    /// coordinates, that an `onLayout` handler reported for the field or
    /// button the menu belongs to.
    ///
    /// Non-activating, which is the default a popup wants and the reason
    /// this is a constructor rather than a `kind` you set: a popup that
    /// takes OS focus blurs whatever opened it. Set `activates` back to
    /// true afterwards for the rare surface that should steal focus.
    pub fn popup(anchor: Rect, w: f32, h: f32) -> Self {
        Self {
            kind: WindowKind::Popup,
            size: Size::new(w, h),
            activates: false,
            anchor,
        }
    }
}

impl Default for WindowConfig {
    fn default() -> Self {
        Self {
            kind: WindowKind::Normal,
            size: Self::DEFAULT_SIZE,
            activates: true,
            anchor: Rect::new(0.0, 0.0, 0.0, 0.0),
        }
    }
}

/// A window-level intent for the frame driver, drained via
/// `Core::take_window_commands` after each input dispatch and each frame.
/// Three things produce one: input on a chrome node, the declared set's
/// diff, and an app asking directly (`Core::set_window_size`,
/// `Core::focus_window`, `Core::push_window_command`). A headless driver
/// never drains, which is the whole of "the core never touches a window".
///
/// Every variant says which window it is about, and every variant is
/// `Copy` and pointer-free — an `Open` carries no title (the new window's
/// own first frame declares one through `window_title`) and a `SetSize`
/// carries two floats — so nothing borrowed ever enters a driver's drain
/// loop.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum WindowCommand {
    /// Begin an interactive OS move (the press landed on a `Drag` node).
    StartDrag(WindowId),
    /// Close this window: from its chrome close button, or because no
    /// frame declares it any more. For the main window a driver exits.
    Close(WindowId),
    Minimize(WindowId),
    ToggleMaximize(WindowId),
    /// Open a window the declared set gained. `id` is the one the core
    /// assigned and will stamp on the window's events; `origin` is the
    /// frontend whose declaration won (a host may refuse an extension's);
    /// `owner` is the window whose frame that declaration came from — what
    /// a [`WindowKind::Popup`] is anchored against and owned by, and what
    /// closing takes the popup with it (the owner's declarations leave the
    /// declared set when the owner does, so the diff closes the child in
    /// the same pass).
    Open {
        id: WindowId,
        owner: WindowId,
        origin: OriginId,
        config: WindowConfig,
    },
    /// Resize `window` to `size` (logical px), asked for by the app
    /// (`Core::set_window_size`). A command and not part of a declaration,
    /// because the user owns a window's size once it exists — a declared
    /// size would fight every drag of the window's edge, which is why
    /// `WindowConfig::size` is read on the opening edge only. The OS may
    /// answer with a different size (a minimum, a tiling manager); the
    /// frame that follows posts a `resize` event with whatever it actually
    /// became, the way every resize already does.
    SetSize {
        window: WindowId,
        size: Size,
    },
    /// Give `window` keyboard focus (`Core::focus_window`). Advisory, like
    /// every focus request an app makes of a window manager.
    Focus(WindowId),
}

impl WindowCommand {
    /// The window the command is about.
    pub fn window(&self) -> WindowId {
        match *self {
            WindowCommand::StartDrag(w)
            | WindowCommand::Close(w)
            | WindowCommand::Minimize(w)
            | WindowCommand::ToggleMaximize(w)
            | WindowCommand::Focus(w) => w,
            WindowCommand::Open { id, .. } => id,
            WindowCommand::SetSize { window, .. } => window,
        }
    }
}

/// Why a window was asked to go away (`Core::dismiss_window`): the same
/// two reasons ADR 0003 gave a modal node, one level up.
///
/// A [`WindowKind::Popup`] is dismissed by the driver, because both facts
/// are the OS's and not the frame's: a press outside a window lands in
/// another surface, and Escape reaches a non-activating popup only through
/// whichever window the OS gave the keyboard to. What the core does with
/// either is what ADR 0003 decided — raise `{kind:"dismiss", reason}` and
/// close nothing. The app stops declaring the window on the frame it
/// decides to, so a dropdown that graduates from a `modal` float to a
/// popup window changes its declaration and not its handler.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DismissReason {
    /// A press landed outside the window.
    Outside,
    /// Escape, wherever the OS delivered it.
    Escape,
}

impl DismissReason {
    /// The string the `dismiss` payload carries, the same spelling a
    /// modal's `reason` uses.
    pub fn as_str(self) -> &'static str {
        match self {
            DismissReason::Outside => "outside",
            DismissReason::Escape => "escape",
        }
    }
}

/// What the host knows about its window, pushed into `Env` by the frame
/// driver. Views (e.g. `widgets::titlebar`) read this to adapt: reserve
/// space for native controls, pick the maximize/restore glyph, or render
/// nothing at all under native decorations.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct WindowEnv {
    /// Which window this core is drawing: the id the `Open` that created
    /// it carried, written here by the driver (`MAIN` for the launcher's).
    /// A view reads it here rather than through a query, and it is what the
    /// core stamps onto every `UiEvent` it hands out — so a driver that
    /// pushes the rest of these facts has already said where its events
    /// came from. The window's *name* is `Core::window_name`.
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
