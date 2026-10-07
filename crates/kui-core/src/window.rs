//! Windows as data: what a frame declares about the OS windows it wants,
//! and the commands a frame driver applies to the real ones.
//!
//! You meet this module in three places. [`WindowConfig`] is what
//! `Ui::window` / `Core::declare_window` take to say that a named window
//! exists this frame; `NodeSpec::window_drag` and `NodeSpec::window_button`
//! make a node part of the window chrome; and a driver drains the resulting
//! [`WindowCommand`]s with `Core::take_window_commands` and applies them.
//! The core never touches a window itself, so a headless driver simply
//! never drains. Facts about the host window flow back in through
//! [`WindowEnv`] on `Env`.
//!
//! ```rust
//! use kui_core::{Rect, WindowCommand, WindowConfig, WindowId, WindowKind};
//!
//! // A second window, opened on the first frame that declares it.
//! let palette = WindowConfig::sized(320.0, 480.0);
//! assert_eq!(palette.kind, WindowKind::Normal);
//!
//! // A dropdown surface anchored to a field's rect; it does not take focus.
//! let field = Rect::new(20.0, 40.0, 200.0, 24.0);
//! let menu = WindowConfig::popup(field, 200.0, 160.0);
//! assert_eq!(menu.kind, WindowKind::Popup);
//! assert!(!menu.activates);
//!
//! // What a driver does with the commands it drains.
//! fn apply(cmd: WindowCommand) {
//!     match cmd {
//!         WindowCommand::Open { id, config, .. } => println!("open {id:?} at {:?}", config.size),
//!         WindowCommand::Close(WindowId::MAIN) => println!("exit"),
//!         WindowCommand::Close(id) => println!("close {id:?}"),
//!         WindowCommand::SetSize { window, size } => println!("resize {window:?} to {size:?}"),
//!         other => println!("{}", other.kind_name()),
//!     }
//! }
//! apply(WindowCommand::Close(WindowId::MAIN));
//! ```

use crate::geom::{Rect, Size};
use crate::tree::OriginId;

/// Which OS window something belongs to: an opaque integer the core
/// assigns when it opens a window, not a handle an app builds.
/// [`WindowId::MAIN`] is 0, the window the launcher opens, which is always
/// live. Apps name windows with a stable string (`Core::declare_window`);
/// the id is how the driver and `UiEvent::window` refer to the surface
/// that string opened.
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
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum WindowRole {
    /// Mouse-down here asks the driver to start an OS window drag (drivers
    /// conventionally promote a quick second press to a maximize toggle).
    Drag,
    /// Clicking here emits the button's window command.
    Button(WindowButton),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
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

/// What kind of OS surface a declared window is.
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
    /// Reach for it only for the placements a float cannot make: a list
    /// taller than the window, a menu near an edge with nowhere in-window
    /// to go, a panel beside the app. Everything else is cheaper as a
    /// float ([`crate::spec::FloatConfig::fit`]): a float costs one tree
    /// and one draw call, a popup an OS surface, a swapchain and a `Core`.
    Popup,
}

impl WindowKind {
    /// Every kind, in wire order: the index a binding that spells kinds
    /// as numbers sends. Append-only.
    pub const ALL: [WindowKind; 2] = [WindowKind::Normal, WindowKind::Popup];

    /// The wire name, for the bindings and the report.
    pub fn name(self) -> &'static str {
        match self {
            WindowKind::Normal => "normal",
            WindowKind::Popup => "popup",
        }
    }

    /// The kind a wire name spells: the inverse of [`Self::name`].
    pub fn from_name(name: &str) -> Option<WindowKind> {
        Self::ALL.into_iter().find(|k| k.name() == name)
    }
}

/// What a frame says about a window it declares (`Ui::window`,
/// `Core::declare_window`). Plain data, no title and no callbacks, so a
/// [`WindowCommand`] stays `Copy` and two declarations of one name compare
/// by value.
///
/// **Read on the opening edge only.** The config that reaches
/// [`WindowCommand::Open`] is the one the declaration carried on the frame
/// it started; re-declaring `"palette"` at a new size does not resize it,
/// because the user owns a window's geometry once it exists. Resize with
/// `Core::set_window_size` instead.
///
/// ```rust
/// use kui_core::{Rect, WindowConfig, WindowKind};
///
/// let normal = WindowConfig::sized(640.0, 400.0);
/// let popup = WindowConfig::popup(Rect::new(0.0, 0.0, 120.0, 24.0), 120.0, 200.0);
/// assert!(normal.activates && !popup.activates);
/// assert_eq!(WindowConfig::default().size, WindowConfig::DEFAULT_SIZE);
/// assert_eq!(WindowKind::from_name("popup"), Some(WindowKind::Popup));
/// ```
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
    /// `{kind, width, height, activates, anchor: {x, y, w, h}}`.
    pub fn to_value(&self) -> crate::value::Value {
        use crate::value::Value;
        Value::map([
            ("kind", Value::str(self.kind.name())),
            ("width", Value::float(self.size.w)),
            ("height", Value::float(self.size.h)),
            ("activates", Value::Bool(self.activates)),
            ("anchor", self.anchor.to_value()),
        ])
    }

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
    /// Non-activating, because a popup that takes OS focus blurs whatever
    /// opened it. Set `activates` back to true afterwards for the rare
    /// surface that should take focus.
    pub fn popup(anchor: Rect, w: f32, h: f32) -> Self {
        Self {
            size: Size::new(w, h),
            anchor,
            ..Self::of_kind(WindowKind::Popup)
        }
    }

    /// The defaults for a window of `kind`: the one decision the kind
    /// makes on its own is whether opening it takes OS focus (a normal
    /// window does, a popup does not).
    pub fn of_kind(kind: WindowKind) -> Self {
        Self {
            kind,
            activates: kind == WindowKind::Normal,
            ..Self::default()
        }
    }

    /// A declaration from plain data: a bare name (the defaults), or a map
    /// with `name`, `kind` (`"normal"` | `"popup"`), `width` and `height`
    /// (both and positive, or the default size), `activates`, and the `anchor` rect
    /// (`{x, y, w, h}`) a popup is placed against. Returns the name with
    /// the config. An unknown `kind` is an error rather than a normal
    /// window, so a typo cannot read as a popup having worked.
    pub fn from_value(v: &crate::value::Value) -> Result<(String, Self), String> {
        use crate::value::Value;
        match v {
            Value::Str(name) => Ok((name.clone(), Self::default())),
            Value::Map(_) => {
                let name = v
                    .get_str("name")
                    .ok_or("a windows entry needs a name")?
                    .to_string();
                let kind = match v.get_str("kind") {
                    None => WindowKind::Normal,
                    Some(k) => WindowKind::from_name(k).ok_or_else(|| {
                        format!(
                            "windows entry `{name}` has kind {k:?}; the kinds are {}",
                            WindowKind::ALL
                                .iter()
                                .map(|k| format!("{:?}", k.name()))
                                .collect::<Vec<_>>()
                                .join(" and ")
                        )
                    })?,
                };
                let mut cfg = Self::of_kind(kind);
                let num = |key: &str| v.get(key).and_then(Value::as_float).map(|n| n as f32);
                // Both, and positive: a zero is the default size, as C's
                // `kui_window_declare` reads it and the runner opens it.
                if let (Some(w), Some(h)) = (num("width"), num("height"))
                    && w > 0.0
                    && h > 0.0
                {
                    cfg.size = Size::new(w, h);
                }
                if let Some(a) = v.get_bool("activates") {
                    cfg.activates = a;
                }
                if let Some(a) = v.get("anchor") {
                    let f = |key: &str| a.get(key).and_then(Value::as_float).unwrap_or(0.0) as f32;
                    cfg.anchor = Rect::new(f("x"), f("y"), f("w"), f("h"));
                }
                Ok((name, cfg))
            }
            other => Err(format!(
                "a windows entry is a name or a table, not {}",
                other.type_name()
            )),
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

/// A window-level intent for the frame driver, drained with
/// `Core::take_window_commands` after each input dispatch and each frame.
/// Three things produce one: input on a chrome node, the diff of the
/// declared window set, and an app asking directly (`Core::set_window_size`,
/// `Core::focus_window`, `Core::push_window_command`). A headless driver
/// never drains.
///
/// Every variant says which window it is about ([`WindowCommand::window`])
/// and is `Copy`: an `Open` carries no title (the new window's first frame
/// declares one through `window_title`).
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
    /// (`Core::set_window_size`). A command rather than part of the
    /// declaration, because the user owns a window's size once it exists.
    /// The OS may answer with a different size (a minimum, a tiling
    /// manager); the frame that follows posts a `resize` event with
    /// whatever it actually became.
    SetSize {
        window: WindowId,
        size: Size,
    },
    /// Give `window` keyboard focus (`Core::focus_window`). Advisory, like
    /// every focus request an app makes of a window manager.
    Focus(WindowId),
    /// Draw `window` again: something another window's frame or input
    /// changed is shown there (the devtools window's row hover outlines a
    /// node in the main window). A driver that redraws every window on
    /// every event may ignore it.
    Redraw(WindowId),
}

impl WindowCommand {
    /// The command's wire name: `startDrag`, `close`, `minimize`,
    /// `toggleMaximize`, `open`, `setSize`, `focus`, `redraw`.
    pub fn kind_name(&self) -> &'static str {
        match self {
            WindowCommand::StartDrag(_) => "startDrag",
            WindowCommand::Close(_) => "close",
            WindowCommand::Minimize(_) => "minimize",
            WindowCommand::ToggleMaximize(_) => "toggleMaximize",
            WindowCommand::Open { .. } => "open",
            WindowCommand::SetSize { .. } => "setSize",
            WindowCommand::Focus(_) => "focus",
            WindowCommand::Redraw(_) => "redraw",
        }
    }

    /// `{kind, window}` plus what the variant carries: `width`/`height`
    /// for `setSize`; `owner`, `origin` and `config` for `open`.
    pub fn to_value(&self) -> crate::value::Value {
        use crate::value::Value;
        let mut out = vec![
            ("kind".to_string(), Value::str(self.kind_name())),
            ("window".to_string(), Value::Int(self.window().0 as i64)),
        ];
        match self {
            WindowCommand::SetSize { size, .. } => {
                out.push(("width".into(), Value::float(size.w)));
                out.push(("height".into(), Value::float(size.h)));
            }
            WindowCommand::Open {
                owner,
                origin,
                config,
                ..
            } => {
                out.push(("owner".into(), Value::Int(owner.0 as i64)));
                out.push(("origin".into(), Value::Int(origin.0 as i64)));
                out.push(("config".into(), config.to_value()));
            }
            _ => {}
        }
        Value::Map(out)
    }

    /// The window the command is about.
    pub fn window(&self) -> WindowId {
        match *self {
            WindowCommand::StartDrag(w)
            | WindowCommand::Close(w)
            | WindowCommand::Minimize(w)
            | WindowCommand::ToggleMaximize(w)
            | WindowCommand::Focus(w)
            | WindowCommand::Redraw(w) => w,
            WindowCommand::Open { id, .. } => id,
            WindowCommand::SetSize { window, .. } => window,
        }
    }
}

/// Why a window was asked to go away (`Core::dismiss_window`): the same
/// two reasons a `modal` node has, one level up.
///
/// A [`WindowKind::Popup`] is dismissed by the driver, because both facts
/// belong to the OS: a press outside a window lands in another surface,
/// and Escape reaches a non-activating popup only through whichever window
/// has the keyboard. The core raises `{kind:"dismiss", reason}` and closes
/// nothing; the app stops declaring the window on the frame it decides to,
/// so a dropdown that moves from a `modal` float to a popup window changes
/// its declaration and not its handler.
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

/// What shows through a window where its frame paints nothing, or paints
/// with alpha: the window's own opaque ground, the desktop, or a material
/// the OS draws behind the window — the blurred desktop of a macOS sidebar
/// (`NSVisualEffectView`), Windows 11's Mica and Acrylic (backlog F126).
///
/// Asked for by the app (`kui_native::Launcher::backdrop`) and reported
/// back, as what the driver actually got, in [`WindowEnv::backdrop`] — a
/// platform without the material reads `Transparent` or `Opaque`, and a
/// view that draws translucent chrome draws it opaque there. Everything
/// but `Opaque` clears the frame to nothing rather than to the theme's
/// `bg`, so a node's `bg` is what sits over the backdrop, with its alpha.
///
/// ```rust
/// use kui_core::Backdrop;
///
/// assert_eq!(Backdrop::default(), Backdrop::Opaque);
/// assert_eq!(Backdrop::from_name("sidebar"), Some(Backdrop::Sidebar));
/// assert!(Backdrop::Sidebar.is_material() && !Backdrop::Transparent.is_material());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Backdrop {
    /// The window is opaque: what every window is unless an app asks.
    #[default]
    Opaque,
    /// See-through where the frame is, with no material behind it: the
    /// desktop and the windows under this one show as they are. What a
    /// material falls back to on a platform with a compositor but no
    /// material (Linux).
    Transparent,
    /// The material behind a window's content: macOS's under-window
    /// background, Windows 11's Mica.
    Window,
    /// The material of a sidebar or a source list: macOS's sidebar,
    /// Windows 11's Mica Alt (the tabbed-window backdrop), which is the
    /// more tinted of its two.
    Sidebar,
    /// The material of a menu or a popover, more see-through than the
    /// two above: macOS's popover material, Windows 11's Acrylic.
    Transient,
}

impl Backdrop {
    /// Every backdrop, in wire order: the index C's `KUI_BACKDROP_*`
    /// spells. Append-only.
    pub const ALL: [Backdrop; 5] = [
        Backdrop::Opaque,
        Backdrop::Transparent,
        Backdrop::Window,
        Backdrop::Sidebar,
        Backdrop::Transient,
    ];

    /// The wire name, for the bindings and the report.
    pub fn name(self) -> &'static str {
        match self {
            Backdrop::Opaque => "opaque",
            Backdrop::Transparent => "transparent",
            Backdrop::Window => "window",
            Backdrop::Sidebar => "sidebar",
            Backdrop::Transient => "transient",
        }
    }

    /// The backdrop a wire name spells: the inverse of [`Self::name`].
    pub fn from_name(name: &str) -> Option<Backdrop> {
        Self::ALL.into_iter().find(|b| b.name() == name)
    }

    /// The backdrop at C's index, `None` past the end.
    pub fn from_code(code: u32) -> Option<Backdrop> {
        Self::ALL.get(code as usize).copied()
    }

    /// Whether the OS draws a material behind the window, as against the
    /// window being opaque or merely see-through.
    pub fn is_material(self) -> bool {
        matches!(
            self,
            Backdrop::Window | Backdrop::Sidebar | Backdrop::Transient
        )
    }

    /// Whether the window lets anything through: everything but `Opaque`.
    pub fn is_translucent(self) -> bool {
        self != Backdrop::Opaque
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
    /// The window is above every other app's: the level the driver set
    /// after the frame asked (`Core::set_always_on_top`), on a platform
    /// that has one. On Wayland there is no such call, so a driver reports
    /// false however often the app asks, which is why a pin button should
    /// draw its state from this field. It is the driver's record of what
    /// it set, not a query: a level the OS dropped afterwards (a
    /// fullscreen space, a tiling manager) is not seen here.
    pub always_on_top: bool,
    /// Area (logical px, window coords) covered by controls the OS still
    /// draws over our content — macOS traffic lights under custom chrome.
    /// Views keep out of it; `None` means the OS draws nothing over us.
    pub native_controls: Option<Rect>,
    /// What is behind the window's transparent pixels, as the driver got
    /// it — not as the app asked: a `Sidebar` asked for on Linux reads
    /// `Transparent`, and on Windows 10, or under a renderer that cannot
    /// present with alpha, `Opaque`. A view that draws a translucent
    /// sidebar over the material draws it opaque when this is `Opaque`
    /// (backlog F126). `Opaque` by default and in a headless core.
    pub backdrop: Backdrop,
}
