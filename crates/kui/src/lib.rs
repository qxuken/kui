//! Batteries-included runner: winit windows + wgpu renderers around one
//! `kui_core::Core` per window, driving the Elm-ish loop — input becomes
//! `UiEvent`s routed to `App::on_event` (host) or extensions by origin, then
//! `App::view` rebuilds each window's frame.
//!
//! One event loop, any number of windows (`docs/adr/0004-multi-window.md`).
//! The launcher opens the main window; a frame that declares another
//! (`Ui::window`) has the core queue a `WindowCommand::Open`, and the runner
//! opens it as a [`Pane`] — a window, its surface, its `Core` and the
//! per-window input state — on the same `Session` and the same GPU device.
//! `App::view` runs once per pane per frame, with `Ui::window_name` saying
//! which; events carry the pane's `WindowId`.

use std::sync::Arc;

pub use kui_core::widgets;
pub use kui_core::*;

mod access_bridge;
pub mod audio;
/// ADR 0009's arithmetic: where a pointer in one window is in another.
mod retarget;
#[cfg(target_os = "windows")]
mod windows_nc;

use winit::application::ApplicationHandler;
use winit::dpi::{LogicalPosition, LogicalSize};
use winit::event::{ElementState, Ime, MouseButton as WinitButton, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop, EventLoopProxy};
use winit::keyboard::{Key as WinitKey, ModifiersState, NamedKey};
// `WindowId` is `kui_core`'s here (re-exported above); winit's own is the
// OS handle the event loop routes by, and only this file names it.
use winit::window::{CursorIcon, ResizeDirection, Window, WindowId as WinitWindowId};

pub trait App {
    /// Builds one window's frame. Called once per open window per frame;
    /// `ui.window_name()` says which (`"main"` for the launcher's).
    fn view(&mut self, ui: &mut Ui<'_>);
    fn on_event(&mut self, _ev: UiEvent) {}
    /// Called once, before the window opens, with the one thing the loop
    /// hands out: a [`Waker`] the app can clone into any thread. A PTY
    /// reader, a file watcher, an LSP client or a socket calls
    /// [`Waker::wake`] when it has changed what `view` will show, and the
    /// loop draws; nothing else ever wakes it, since it parks between
    /// events (backlog C21). The default keeps it: an app with no other
    /// thread has no use for one.
    fn setup(&mut self, _waker: Waker) {}
}

/// A handle into the event loop that any thread may hold: [`wake`] asks
/// for a frame from wherever the app's data arrived. Cheap to clone, and
/// harmless after the loop has ended (a wake nobody hears is dropped).
///
/// [`wake`]: Waker::wake
#[derive(Clone)]
pub struct Waker(EventLoopProxy<access_bridge::UserEvent>);

impl Waker {
    /// Asks every window for a frame. The loop wakes, `view` runs, and
    /// the frame is drawn — the same path a key press takes, minus the
    /// event. Safe from any thread and at any rate: wakes coalesce into
    /// the loop's next turn rather than queueing frames.
    pub fn wake(&self) {
        let _ = self.0.send_event(access_bridge::UserEvent::Wake);
    }
}

impl std::fmt::Debug for Waker {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Waker")
    }
}

/// Who draws the window chrome.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Chrome {
    /// The OS titlebar and buttons (the default).
    #[default]
    Native,
    /// The app draws its own titlebar (`widgets::titlebar`). On macOS the
    /// native traffic lights stay, overlaid on the content (their rect is
    /// reported in `env.window.native_controls`); elsewhere the window is
    /// undecorated and the runner synthesizes edge resizing, double-click
    /// maximize, and applies the `WindowCommand`s chrome nodes produce.
    Custom,
    /// No decorations and no chrome expectations (splash screens, popups).
    Borderless,
}

/// How outline glyphs are antialiased.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAa {
    /// LCD subpixel coverage when the GPU can blend per channel, grayscale
    /// otherwise (the default). `KUI_TEXT_AA=gray|subpixel` overrides.
    #[default]
    Auto,
    Grayscale,
    Subpixel,
}

/// Entry point: `kui::app("title").custom_titlebar().run(my_app)`.
pub fn app(title: &str) -> Launcher {
    Launcher {
        title: title.to_string(),
        chrome: Chrome::Native,
        size: (960.0, 640.0),
        min_size: None,
        max_size: None,
        extensions: Extensions::new(),
        text_aa: TextAa::Auto,
        diagnostics: None,
    }
}

/// Builder for the windowed runner.
pub struct Launcher {
    title: String,
    chrome: Chrome,
    size: (f64, f64),
    /// Inner-size bounds (logical px) handed to the OS, which enforces them
    /// for user resizing; `None` leaves that side unbounded.
    min_size: Option<(f64, f64)>,
    max_size: Option<(f64, f64)>,
    extensions: Extensions,
    text_aa: TextAa,
    /// Whether the core's diagnostics run (see `kui_core::diag`); None =
    /// on in debug builds, off in release.
    diagnostics: Option<bool>,
}

impl Launcher {
    pub fn chrome(mut self, chrome: Chrome) -> Self {
        self.chrome = chrome;
        self
    }

    /// Glyph antialiasing; see [`TextAa`].
    pub fn text_aa(mut self, aa: TextAa) -> Self {
        self.text_aa = aa;
        self
    }

    /// Whether the core looks for silent misconfigurations and the runner
    /// prints them to stderr (see `kui_core::diag`). Default: on in debug
    /// builds, off in release — a shipped app stays quiet, a development
    /// build says why the grow weight did nothing.
    pub fn diagnostics(mut self, on: bool) -> Self {
        self.diagnostics = Some(on);
        self
    }

    /// Shorthand for `.chrome(Chrome::Custom)`.
    pub fn custom_titlebar(self) -> Self {
        self.chrome(Chrome::Custom)
    }

    /// Shorthand for `.chrome(Chrome::Borderless)`.
    pub fn borderless(self) -> Self {
        self.chrome(Chrome::Borderless)
    }

    /// Initial inner size, logical px (`KUI_WINDOW=WxH` still overrides).
    /// Clamped into the `min_size`/`max_size` bounds, as the OS would.
    pub fn size(mut self, w: f64, h: f64) -> Self {
        self.size = (w, h);
        self
    }

    /// Smallest inner size the user may resize the window to, logical px.
    /// The OS enforces it; the initial size is clamped up into it.
    pub fn min_size(mut self, w: f64, h: f64) -> Self {
        self.min_size = Some((w, h));
        self
    }

    /// Largest inner size the user may resize the window to, logical px.
    /// A bound below the matching `min_size` loses to it, as on the OS side.
    pub fn max_size(mut self, w: f64, h: f64) -> Self {
        self.max_size = Some((w, h));
        self
    }

    /// Loads `ext` under its own name as its namespace — `import fs` binds
    /// `fs`. The slots it fills are declared as `ui.slot("<name>/<slot>")`
    /// (ADR 0014). Panics when the name is already another extension's
    /// namespace: two of one name need `extension_as`.
    pub fn extension(self, ext: impl Extension + 'static) -> Self {
        let ns = ext.name().to_owned();
        self.extension_as(ns, ext)
    }

    /// Loads `ext` under `namespace` — `import fs as left`. The host
    /// decides the namespace, so the same plugin loaded twice is two
    /// namespaces, two sets of slots and two sets of params. Panics on a
    /// namespace already taken, an empty one, or an extension whose slot
    /// names contain `/`: all three are programming errors at startup.
    pub fn extension_as(
        mut self,
        namespace: impl Into<String>,
        ext: impl Extension + 'static,
    ) -> Self {
        if let Err(e) = self.extensions.push_as(namespace, Box::new(ext)) {
            panic!("kui: {e}");
        }
        self
    }

    /// `extension` for each, in order.
    pub fn extensions(mut self, exts: Vec<Box<dyn Extension>>) -> Self {
        for ext in exts {
            if let Err(e) = self.extensions.push(ext) {
                panic!("kui: {e}");
            }
        }
        self
    }

    fn shell<A: App>(self, app: A) -> Shell<A> {
        // Diagnostics are a development aid: on in debug builds unless the
        // launcher says otherwise, so a shipped app pays and prints nothing.
        let diagnostics = self.diagnostics.unwrap_or(cfg!(debug_assertions));
        let session = Session::new();
        let mut core = Core::new_in(&session);
        core.set_diagnostics(diagnostics);
        Shell {
            title: self.title,
            chrome: self.chrome,
            size: clamp_size(self.size, self.min_size, self.max_size),
            min_size: self.min_size,
            max_size: self.max_size,
            text_aa: self.text_aa,
            diagnostics,
            subpixel: false,
            app,
            extensions: self.extensions,
            session,
            main_core: Some(core),
            panes: Vec::new(),
            gpu: None,
            epoch: std::time::Instant::now(),
            clipboard: arboard::Clipboard::new().ok(),
            audio: audio::Audio::new(),
            exit_requested: false,
            primary_down: None,
            armed: Vec::new(),
            swallowed_press: None,
            proxy: None,
        }
    }

    pub fn run<A: App>(self, app: A) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::<access_bridge::UserEvent>::with_user_event().build()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut shell = self.shell(app);
        shell.proxy = Some(event_loop.create_proxy());
        shell.app.setup(Waker(event_loop.create_proxy()));
        event_loop.run_app(&mut shell)?;
        Ok(())
    }

    /// Opens the window but keeps the event loop in the caller's hands: the
    /// returned [`PumpRunner`] processes OS events only when [`PumpRunner::pump`]
    /// is called, so a foreign loop (Node/libuv, a game loop, a test harness)
    /// can interleave with winit on the main thread. One event loop per
    /// process — winit event loops are not recreatable on every platform —
    /// but any number of windows on it.
    pub fn open<A: App>(self, app: A) -> Result<PumpRunner<A>, Box<dyn std::error::Error>> {
        let mut event_loop = EventLoop::<access_bridge::UserEvent>::with_user_event().build()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut shell = self.shell(app);
        shell.proxy = Some(event_loop.create_proxy());
        shell.app.setup(Waker(event_loop.create_proxy()));
        // First pump delivers `resumed`, creating the window + renderer.
        let alive = pump_once(&mut event_loop, &mut shell);
        Ok(PumpRunner {
            event_loop,
            shell,
            alive,
        })
    }
}

fn pump_once<A: App>(
    event_loop: &mut EventLoop<access_bridge::UserEvent>,
    shell: &mut Shell<A>,
) -> bool {
    use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
    match event_loop.pump_app_events(Some(std::time::Duration::ZERO), shell) {
        PumpStatus::Continue => !shell.exit_requested,
        PumpStatus::Exit(_) => false,
    }
}

/// A windowed runner driven from outside: same [`Shell`] as [`Launcher::run`]
/// (input mapping, IME, clipboard, chrome, caret blink), but the host calls
/// [`pump`](Self::pump) on its own cadence instead of parking in `run_app`.
pub struct PumpRunner<A: App> {
    event_loop: EventLoop<access_bridge::UserEvent>,
    shell: Shell<A>,
    alive: bool,
}

impl<A: App> PumpRunner<A> {
    /// Processes all pending OS events without blocking. Returns false once
    /// the main window has closed (further pumps are no-ops).
    pub fn pump(&mut self) -> bool {
        if !self.alive {
            return false;
        }
        self.alive = pump_once(&mut self.event_loop, &mut self.shell);
        self.alive
    }

    /// `pump`, but parked until an OS event, a [`Waker::wake`] or
    /// `deadline` — whichever comes first — so a host that owns the loop
    /// blocks on all three instead of polling on a timer (backlog C21).
    /// Returns false once the main window has closed.
    pub fn pump_until(&mut self, deadline: std::time::Instant) -> bool {
        use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
        if !self.alive {
            return false;
        }
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        self.alive = match self
            .event_loop
            .pump_app_events(Some(timeout), &mut self.shell)
        {
            PumpStatus::Continue => !self.shell.exit_requested,
            PumpStatus::Exit(_) => false,
        };
        self.alive
    }

    /// A [`Waker`] for this loop, to clone into the threads the host's
    /// data arrives on.
    pub fn waker(&self) -> Waker {
        Waker(self.event_loop.create_proxy())
    }

    pub fn app_mut(&mut self) -> &mut A {
        &mut self.shell.app
    }

    /// The main window's core. Every window of the app shares its session,
    /// so resources registered through it draw in all of them, and
    /// `Core::windows` on it lists them.
    pub fn core_mut(&mut self) -> &mut Core {
        self.shell.core_mut()
    }

    /// The main window's inner size (logical px) and its scale factor —
    /// what the next frame lays out against. Unlike `core_mut().viewport()`
    /// this is known before the first frame, so a host can size its model
    /// at setup.
    pub fn window_size(&self) -> (Size, f32) {
        self.shell.window_size()
    }

    /// Schedules a redraw of every window (call after changing what `view`
    /// will produce).
    pub fn request_redraw(&self) {
        for p in &self.shell.panes {
            p.window.request_redraw();
        }
    }

    /// Asks the app to close; the next `pump` observes it and returns false.
    pub fn request_exit(&mut self) {
        self.shell.exit_requested = true;
    }

    /// Hands the core's queued audio commands to the device now, rather
    /// than at the next pump — for hosts that call `Core::play` between
    /// pumps and want the sound to start at once.
    pub fn flush_audio(&mut self) {
        self.shell.apply_audio();
    }
}

pub fn run<A: App>(
    title: &str,
    application: A,
    extensions: Vec<Box<dyn Extension>>,
) -> Result<(), Box<dyn std::error::Error>> {
    app(title).extensions(extensions).run(application)
}

/// Clamps a requested inner size into `min`/`max` (logical px), the way the
/// OS clamps a resize once the window exists — so a host reading
/// `PumpRunner::window_size` before the first frame sees the real size.
/// `min` wins where the two bounds cross, matching the platforms.
fn clamp_size(size: (f64, f64), min: Option<(f64, f64)>, max: Option<(f64, f64)>) -> (f64, f64) {
    let (mut w, mut h) = size;
    if let Some((mw, mh)) = max {
        w = w.min(mw);
        h = h.min(mh);
    }
    if let Some((mw, mh)) = min {
        w = w.max(mw);
        h = h.max(mh);
    }
    (w, h)
}

/// Traffic-light keep-out rect under macOS custom chrome (logical px,
/// window coords); content starts at its right edge. AppKit gives no stable
/// public metric, so this uses gpui's measured TRAFFIC_LIGHT_PADDING: 78
/// with the macOS 26 SDK (71 on older SDKs — the extra pixel on the left is
/// the window border). We assume a current SDK.
const MACOS_TRAFFIC_LIGHTS: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 78.0,
    h: 28.0,
};
/// Width of the invisible resize band synthesized on undecorated windows.
const RESIZE_BAND: f32 = 6.0;
/// A second titlebar press within this window toggles maximize.
const DOUBLE_CLICK_MS: u128 = 350;
/// Presses within this window (and `MULTI_CLICK_SLOP` px) count up the
/// multi-click sent with `InputEvent::MouseDown` (double = word select).
const MULTI_CLICK_MS: u128 = 400;
const MULTI_CLICK_SLOP: f32 = 4.0;
/// Caret blink half-period while an edit widget is focused.
const BLINK_INTERVAL: std::time::Duration = std::time::Duration::from_millis(500);
/// How often the loop wakes to notice a playing sound finishing.
const AUDIO_POLL: std::time::Duration = std::time::Duration::from_millis(50);
/// How long to wait between tries at a window's first frame, and how many
/// tries to make: about a second of a just-shown window insisting it is
/// occluded, after which it is taken at its word and the ordinary redraw
/// path (a resize, an expose, any input) is what wakes it.
const FIRST_FRAME_RETRY: std::time::Duration = std::time::Duration::from_millis(16);
const FIRST_FRAME_RETRIES: u32 = 60;

/// The core's derived pointer shape in winit's vocabulary. One-to-one:
/// `CursorShape` is spelled after the platform names on purpose.
/// Chromeless, in the spelling each platform actually honours.
///
/// Everywhere but macOS that is `with_decorations(false)`. On macOS it is
/// **not**: `with_decorations(false)` gives a window with the borderless
/// style mask, and AppKit never sends `mouseUp:` to one — every press in it
/// lands and never releases, so nothing in the window can be clicked, no
/// drag ever ends, and a pressed style never clears. A hidden titlebar over
/// a fullsize content view looks the same, is a real window, and is what
/// `Chrome::Custom` already asks for; chromeless is that plus the traffic
/// lights hidden. It keeps the rounded corners, the drop shadow and the
/// native edge-resizing that a borderless window has none of.
///
/// Found by pressing a menu item and watching nothing happen (backlog W1);
/// the popup surface and `Chrome::Borderless` share this because they were
/// separately wrong in the same way.
fn undecorated(attrs: winit::window::WindowAttributes) -> winit::window::WindowAttributes {
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::WindowAttributesExtMacOS;
        attrs
            .with_titlebar_transparent(true)
            .with_fullsize_content_view(true)
            .with_title_hidden(true)
            .with_titlebar_buttons_hidden(true)
    }
    #[cfg(not(target_os = "macos"))]
    {
        attrs.with_decorations(false)
    }
}

fn cursor_icon(shape: CursorShape) -> CursorIcon {
    match shape {
        CursorShape::Default => CursorIcon::Default,
        CursorShape::Text => CursorIcon::Text,
        CursorShape::Pointer => CursorIcon::Pointer,
        CursorShape::Grab => CursorIcon::Grab,
        CursorShape::Grabbing => CursorIcon::Grabbing,
        CursorShape::NotAllowed => CursorIcon::NotAllowed,
        CursorShape::EwResize => CursorIcon::EwResize,
        CursorShape::NsResize => CursorIcon::NsResize,
        CursorShape::NwseResize => CursorIcon::NwseResize,
        CursorShape::NeswResize => CursorIcon::NeswResize,
    }
}

/// One window: the OS surface, its renderer, its `Core`, and every piece
/// of input state that belongs to a window rather than to the app — the
/// cursor, the click counter, the caret blink, the resize band, the cursor
/// icon last set, the accessibility adapter. The main window is the pane
/// with `WindowId::MAIN`; the rest are what frames declared.
struct Pane {
    id: WindowId,
    /// What kind of surface this is, from the `Open` that created it. A
    /// [`WindowKind::Popup`] is the one the runner treats differently after
    /// it is open: it takes the owner's keys while it is up, and Escape or
    /// a press anywhere else asks it to go away.
    kind: WindowKind,
    /// The window whose frame declared this one — itself for the main
    /// window, and for a popup the surface its anchor was measured against
    /// and whose keyboard it borrows.
    owner: WindowId,
    /// Whether it took OS focus when it opened. False is the popup default
    /// and what makes the borrowing necessary.
    activates: bool,
    /// A popup's anchor as its `Open` carried it, in the **owner's** logical
    /// coordinates: the rect an `onLayout` node reported. `popup_position`
    /// turned it into a screen position once; ADR 0009 decision 4 needs it
    /// again, to tell a release on the field that opened the menu from a
    /// release on nothing. Zero-sized for a window that is not a popup, and
    /// so a rect nothing lands in.
    anchor: Rect,
    /// A non-activating popup only: whether the keyboard has already been
    /// asked back for since this window last took it. One request per
    /// acquisition — the ask is advisory, so repeating it every batch until
    /// the platform agrees would be a request storm.
    handed_back: bool,
    /// Whether the OS says *this* window holds the keyboard — what winit
    /// last reported, before the borrowing in [`Shell::settle_focus`] is
    /// applied. `core.env.focused`, which is what a view reads, is derived
    /// from every pane's copy of this and is not always the same answer.
    os_focused: bool,
    core: Core,
    window: Arc<Window>,
    renderer: kui_wgpu::Renderer,
    /// Last title actually set on the window; views declare per frame and
    /// we only touch the window on change.
    applied_title: String,
    modifiers: ModifiersState,
    /// Time of the last titlebar press, for double-click maximize.
    last_titlebar_press: Option<std::time::Instant>,
    /// Last cursor position (logical px), for multi-click distance checks.
    cursor: Vec2,
    /// Last primary press: time, position, and its click count.
    last_click: Option<(std::time::Instant, Vec2, u8)>,
    /// Caret blink phase mirror + next toggle time; the clock lives here,
    /// the core only stores the visible flag.
    blink_visible: bool,
    blink_deadline: Option<std::time::Instant>,
    caret_stamp_seen: u64,
    /// Resize edge currently under the cursor (undecorated windows only).
    resize_edge: Option<ResizeDirection>,
    /// Cursor icon last set on the window, so a shape that did not change
    /// costs nothing.
    cursor_icon: CursorIcon,
    /// Tries left at getting this window's *first* frame onto the screen,
    /// and when to make the next one — `None` once a frame has landed. See
    /// the `Skip` arm of [`Shell::redraw`].
    first_frame: Option<(u32, std::time::Instant)>,
    /// The platform accessibility bridge.
    access: Option<access_bridge::Bridge>,
    /// Windows: answers WM_NCHITTEST from the frame's chrome regions, which
    /// enables snap layouts + native caption behavior over drawn controls.
    #[cfg(target_os = "windows")]
    nc: Option<windows_nc::NcHitTest>,
}

/// The US-QWERTY key at a physical position, in kui's own vocabulary (see
/// [`kui_core::KeyPress::physical`]). Letters and digits are their US
/// characters, punctuation the character US-QWERTY prints there, and the
/// named keys their names — no layout is consulted, which is the point.
///
/// The numeric keypad reports the digit and operator it always bears; kui
/// has no separate numpad vocabulary, and `code` already conflates the two
/// (winit's logical key for `Numpad1` is `"1"`).
fn physical_code(key: winit::keyboard::PhysicalKey) -> KeyCode {
    use winit::keyboard::{KeyCode as Phys, PhysicalKey};
    let PhysicalKey::Code(c) = key else {
        return KeyCode::Unknown;
    };
    // Letters and digits, in winit's own declaration order.
    const LETTERS: [(Phys, char); 26] = [
        (Phys::KeyA, 'a'),
        (Phys::KeyB, 'b'),
        (Phys::KeyC, 'c'),
        (Phys::KeyD, 'd'),
        (Phys::KeyE, 'e'),
        (Phys::KeyF, 'f'),
        (Phys::KeyG, 'g'),
        (Phys::KeyH, 'h'),
        (Phys::KeyI, 'i'),
        (Phys::KeyJ, 'j'),
        (Phys::KeyK, 'k'),
        (Phys::KeyL, 'l'),
        (Phys::KeyM, 'm'),
        (Phys::KeyN, 'n'),
        (Phys::KeyO, 'o'),
        (Phys::KeyP, 'p'),
        (Phys::KeyQ, 'q'),
        (Phys::KeyR, 'r'),
        (Phys::KeyS, 's'),
        (Phys::KeyT, 't'),
        (Phys::KeyU, 'u'),
        (Phys::KeyV, 'v'),
        (Phys::KeyW, 'w'),
        (Phys::KeyX, 'x'),
        (Phys::KeyY, 'y'),
        (Phys::KeyZ, 'z'),
    ];
    const DIGITS: [(Phys, char); 20] = [
        (Phys::Digit0, '0'),
        (Phys::Digit1, '1'),
        (Phys::Digit2, '2'),
        (Phys::Digit3, '3'),
        (Phys::Digit4, '4'),
        (Phys::Digit5, '5'),
        (Phys::Digit6, '6'),
        (Phys::Digit7, '7'),
        (Phys::Digit8, '8'),
        (Phys::Digit9, '9'),
        (Phys::Numpad0, '0'),
        (Phys::Numpad1, '1'),
        (Phys::Numpad2, '2'),
        (Phys::Numpad3, '3'),
        (Phys::Numpad4, '4'),
        (Phys::Numpad5, '5'),
        (Phys::Numpad6, '6'),
        (Phys::Numpad7, '7'),
        (Phys::Numpad8, '8'),
        (Phys::Numpad9, '9'),
    ];
    const PUNCT: [(Phys, char); 17] = [
        (Phys::Backquote, '`'),
        (Phys::Minus, '-'),
        (Phys::Equal, '='),
        (Phys::BracketLeft, '['),
        (Phys::BracketRight, ']'),
        (Phys::Backslash, '\\'),
        (Phys::Semicolon, ';'),
        (Phys::Quote, '\''),
        (Phys::Comma, ','),
        (Phys::Period, '.'),
        (Phys::Slash, '/'),
        (Phys::NumpadDivide, '/'),
        (Phys::NumpadMultiply, '*'),
        (Phys::NumpadSubtract, '-'),
        (Phys::NumpadAdd, '+'),
        (Phys::NumpadDecimal, '.'),
        (Phys::NumpadEqual, '='),
    ];
    for (p, ch) in LETTERS.iter().chain(&DIGITS).chain(&PUNCT) {
        if *p == c {
            return KeyCode::Char(*ch);
        }
    }
    match c {
        Phys::Space => KeyCode::Space,
        Phys::Enter | Phys::NumpadEnter => KeyCode::Enter,
        Phys::Tab => KeyCode::Tab,
        Phys::Backspace | Phys::NumpadBackspace => KeyCode::Backspace,
        Phys::Delete => KeyCode::Delete,
        Phys::Escape => KeyCode::Escape,
        Phys::Insert => KeyCode::Insert,
        Phys::Home => KeyCode::Home,
        Phys::End => KeyCode::End,
        Phys::PageUp => KeyCode::PageUp,
        Phys::PageDown => KeyCode::PageDown,
        Phys::ArrowLeft => KeyCode::Left,
        Phys::ArrowRight => KeyCode::Right,
        Phys::ArrowUp => KeyCode::Up,
        Phys::ArrowDown => KeyCode::Down,
        Phys::F1 => KeyCode::F(1),
        Phys::F2 => KeyCode::F(2),
        Phys::F3 => KeyCode::F(3),
        Phys::F4 => KeyCode::F(4),
        Phys::F5 => KeyCode::F(5),
        Phys::F6 => KeyCode::F(6),
        Phys::F7 => KeyCode::F(7),
        Phys::F8 => KeyCode::F(8),
        Phys::F9 => KeyCode::F(9),
        Phys::F10 => KeyCode::F(10),
        Phys::F11 => KeyCode::F(11),
        Phys::F12 => KeyCode::F(12),
        _ => KeyCode::Unknown,
    }
}

impl Pane {
    /// Inner size in logical px plus the scale factor.
    fn size(&self) -> (Size, f32) {
        let scale = self.window.scale_factor() as f32;
        let size = self.window.inner_size();
        (
            Size::new(size.width as f32 / scale, size.height as f32 / scale),
            scale,
        )
    }

    /// This window as retargeting sees it (`retarget::Surface`): where its
    /// client area sits on the screen in **physical** pixels, its scale, and
    /// its logical size. `None` when the platform will not say where the
    /// window is — the same thing `popup_position` gives up on, and the same
    /// answer: leave the pointer where it was.
    fn surface(&self) -> Option<retarget::Surface> {
        let origin = self.window.inner_position().ok()?;
        let (size, scale) = self.size();
        Some(retarget::Surface {
            origin: (origin.x as f64, origin.y as f64),
            scale: scale as f64,
            size,
        })
    }

    /// Undecorated windows get no OS resize borders; the runner synthesizes
    /// them from a band inside the window edges (macOS custom chrome keeps
    /// native edge resizing, and on Windows the non-client subclass answers
    /// WM_NCHITTEST with real border codes instead, so neither synthesizes).
    fn synthesizes_resize(&self, chrome: Chrome) -> bool {
        #[cfg(target_os = "windows")]
        if self.nc.is_some() {
            return false;
        }
        chrome != Chrome::Native && !cfg!(target_os = "macos")
    }

    fn resize_edge_at(&self, p: Vec2) -> Option<ResizeDirection> {
        let w = &self.window;
        if w.is_maximized() || w.fullscreen().is_some() {
            return None;
        }
        let (size, _) = self.size();
        let (sw, sh) = (size.w, size.h);
        let (l, r) = (p.x < RESIZE_BAND, p.x > sw - RESIZE_BAND);
        let (t, b) = (p.y < RESIZE_BAND, p.y > sh - RESIZE_BAND);
        Some(match (l, r, t, b) {
            (true, _, true, _) => ResizeDirection::NorthWest,
            (_, true, true, _) => ResizeDirection::NorthEast,
            (true, _, _, true) => ResizeDirection::SouthWest,
            (_, true, _, true) => ResizeDirection::SouthEast,
            (true, ..) => ResizeDirection::West,
            (_, true, ..) => ResizeDirection::East,
            (_, _, true, _) => ResizeDirection::North,
            (_, _, _, true) => ResizeDirection::South,
            _ => return None,
        })
    }

    /// Applies the pointer shape the core derived for this frame, with the
    /// synthesized resize band on top: the band is the runner's own edge,
    /// invisible to the core, and a press there resizes the window rather
    /// than reaching the UI, so what it says wins. Only touches the window
    /// when the answer changes.
    fn apply_cursor(&mut self) {
        let icon = match self.resize_edge {
            Some(ResizeDirection::West | ResizeDirection::East) => CursorIcon::EwResize,
            Some(ResizeDirection::North | ResizeDirection::South) => CursorIcon::NsResize,
            Some(ResizeDirection::NorthWest | ResizeDirection::SouthEast) => CursorIcon::NwseResize,
            Some(ResizeDirection::NorthEast | ResizeDirection::SouthWest) => CursorIcon::NeswResize,
            None => cursor_icon(self.core.cursor_shape()),
        };
        if icon == self.cursor_icon {
            return;
        }
        self.cursor_icon = icon;
        self.window.set_cursor(icon);
    }

    /// Hands the frame's access tree to the platform when assistive
    /// technology is attached and the tree changed, and whatever the frame
    /// asked to say (`ui.announce`) with it.
    ///
    /// The queue is drained **every frame, attached or not**, and what a
    /// silent window cannot deliver is dropped here: an announcement kept
    /// is an announcement said minutes after the thing it describes
    /// (`docs/adr/0008-live-regions-and-announcements.md`, decision 6).
    fn publish_access(&mut self) {
        let said = self.core.take_announcements();
        let Some(bridge) = self.access.as_mut() else {
            return;
        };
        if !bridge.active() {
            return;
        }
        let scale = self.window.scale_factor() as f32;
        bridge.publish(self.core.access_tree(), scale, &said);
    }

    /// The platform primary shortcut modifier (Cmd on macOS, Ctrl elsewhere).
    fn primary(&self) -> bool {
        if cfg!(target_os = "macos") {
            self.modifiers.super_key()
        } else {
            self.modifiers.control_key()
        }
    }

    fn kmods(&self) -> KeyMods {
        KeyMods {
            shift: self.modifiers.shift_key(),
            ctrl: self.modifiers.control_key(),
            alt: self.modifiers.alt_key(),
            super_key: self.modifiers.super_key(),
        }
    }
}

/// A popup this press is about (`docs/adr/0009-press-drag-release-into-a-popup.md`,
/// decision 1): one that opened while the primary button was down, or the
/// one the button went down inside. For the rest of that press the owner's
/// moves are retargeted into it and its release is classified against it.
struct Armed {
    /// The popup. Its owner and its anchor are the pane's, so nothing here
    /// can go stale against the window it names.
    id: WindowId,
    /// Whether the last retargeted move landed inside this popup, so that
    /// dragging off the list costs one `CursorLeft` and staying off it
    /// costs nothing.
    inside: bool,
}

struct Shell<A: App> {
    title: String,
    chrome: Chrome,
    /// Initial inner size (logical px), already clamped into the bounds.
    size: (f64, f64),
    min_size: Option<(f64, f64)>,
    max_size: Option<(f64, f64)>,
    text_aa: TextAa,
    /// What every core is created with; see `Launcher::diagnostics`.
    diagnostics: bool,
    /// Whether the GPU blends per channel, decided by the first renderer
    /// and applied to every core after it.
    subpixel: bool,
    app: A,
    /// Each under the namespace the host gave it; their `Fill` is what fills
    /// the slots a view declares (ADR 0014).
    extensions: Extensions,
    /// What every window shares: fonts, images, sounds, the audio queue and
    /// the declared window set.
    session: Session,
    /// The main window's core until `resumed` moves it into its pane, so
    /// `PumpRunner::core_mut` has an answer before the first pump.
    main_core: Option<Core>,
    /// The open windows, main first.
    panes: Vec<Pane>,
    /// The device every window renders with, from the first renderer.
    gpu: Option<kui_wgpu::Gpu>,
    /// Origin of the frame clock handed to the cores for transitions.
    epoch: std::time::Instant,
    clipboard: Option<arboard::Clipboard>,
    /// The audio device the core's audio commands drive; see `audio`.
    audio: audio::Audio,
    /// Set by `WindowCommand::Close` on the main window; honored at the end
    /// of the event.
    exit_requested: bool,
    /// Which pane the primary button is down in, if any. ADR 0009 arms a
    /// popup against it: a non-activating popup that opens while this is
    /// set joins that press, which is the observable form of "the drag
    /// whose press opened the popup" and needs no geometry.
    primary_down: Option<WindowId>,
    /// The popups armed into the press `primary_down` names, in opening
    /// order — where two overlap the last is on top. Emptied by the release
    /// that classifies it, and by `close_pane` for a window that goes
    /// first.
    armed: Vec<Armed>,
    /// A primary press that dismissed a non-activating popup and was
    /// consumed rather than dispatched (ADR 0009 decision 5), by the pane
    /// it landed in. Its release is swallowed with it: the core never saw
    /// the `down`, so nothing should see the `up`.
    swallowed_press: Option<WindowId>,
    /// Hands AccessKit a way back into the loop; set before the window
    /// exists.
    proxy: Option<EventLoopProxy<access_bridge::UserEvent>>,
}

impl<A: App> Shell<A> {
    /// The main window's core: its pane's once it exists, the one the
    /// launcher built before that.
    fn core_mut(&mut self) -> &mut Core {
        match self.panes.first_mut() {
            Some(p) => &mut p.core,
            None => self
                .main_core
                .as_mut()
                .expect("the main core exists until its pane takes it"),
        }
    }

    /// Main inner size in logical px plus the scale factor; the launcher's
    /// requested size until the window exists.
    fn window_size(&self) -> (Size, f32) {
        match self.panes.first() {
            Some(p) => p.size(),
            None => (Size::new(self.size.0 as f32, self.size.1 as f32), 1.0),
        }
    }

    fn pane_index(&self, id: WinitWindowId) -> Option<usize> {
        self.panes.iter().position(|p| p.window.id() == id)
    }

    fn pane_of(&self, id: WindowId) -> Option<usize> {
        self.panes.iter().position(|p| p.id == id)
    }

    /// Which pane a key event that arrived at pane `i` is for.
    ///
    /// A non-activating popup never takes OS focus — that is the point of
    /// it, since a combobox that blurred the field it belongs to would be
    /// useless — so the keyboard stays with the owner and the runner hands
    /// it on (ADR 0004 decision 9). The owner's `env.focused` is left true
    /// meanwhile, so the field still draws focused while the arrow keys
    /// walk the list. A popup that *did* ask to activate holds its own
    /// keyboard and needs none of this.
    fn key_target(&self, i: usize) -> usize {
        let owner = self.panes[i].id;
        self.panes
            .iter()
            .position(|p| p.kind == WindowKind::Popup && p.owner == owner && !p.activates)
            .unwrap_or(i)
    }

    /// Every popup that a press on pane `i`, or pane `i` losing the
    /// keyboard, should ask to go away: all of them except one the press
    /// landed in. A press in the owner counts — the owner is "outside" the
    /// popup, which is the whole distinction a separate surface makes.
    ///
    /// Each is paired with whether it took OS focus when it opened, because
    /// ADR 0009 decision 5 treats the two kinds differently: the press that
    /// dismisses a **non-activating** popup is consumed, while an
    /// activating one (a tear-off panel) keeps the pass-through it has.
    fn popups_outside(&self, i: usize) -> Vec<(WindowId, bool)> {
        self.panes
            .iter()
            .enumerate()
            .filter(|(j, p)| *j != i && p.kind == WindowKind::Popup)
            .map(|(_, p)| (p.id, p.activates))
            .collect()
    }

    /// Works out what each window's *view* should believe about keyboard
    /// focus, from what the OS said about all of them.
    ///
    /// The two are not the same answer, and ADR 0004 decision 9 is why: a
    /// non-activating popup must not take the focus ring off the field
    /// that opened it, so **a window that owns one reads as focused while
    /// the popup holds the keyboard**. Without that the owner draws one
    /// unfocused frame every time a popup opens — the platform hands the
    /// key window over and back, and the frame in between is real. It is
    /// derived rather than patched at each event, and derived at the end of
    /// the batch rather than inside it, because focus *moving* is two
    /// events — three around a new window, since winit queues a
    /// `Focused(false)` for every one it creates — and in the middle of any
    /// ordering of them there is a moment when no window claims the
    /// keyboard. Reading that moment is the flicker.
    ///
    /// Only what changed is written, so a window whose answer did not move
    /// is not redrawn and does not let go of a held key.
    fn settle_focus(&mut self) {
        // A non-activating popup that has ended up with the keyboard gives
        // it straight back. It should never have taken it — that is what
        // `activates: false` asked for — but a platform can make a window
        // key for reasons of its own, and pressing one is the reason that
        // matters: AppKit makes the popup key on mouse-down, and the
        // *platform's* titlebar greys out under a window that is not key
        // however `env.focused` reads. Asking again here is the only thing
        // that shortens that to a frame, since nothing else notices.
        for i in 0..self.panes.len() {
            let p = &self.panes[i];
            if p.kind != WindowKind::Popup || p.activates {
                continue;
            }
            if !p.os_focused {
                self.panes[i].handed_back = false;
                continue;
            }
            if p.handed_back {
                continue;
            }
            let owner = p.owner;
            let Some(j) = self.pane_of(owner) else {
                continue;
            };
            self.panes[i].handed_back = true;
            self.panes[j].window.focus_window();
        }
        for i in 0..self.panes.len() {
            let (id, owner, lends) = {
                let p = &self.panes[i];
                (p.id, p.owner, p.kind == WindowKind::Popup && !p.activates)
            };
            // The pair reads as focused together, because between them the
            // keyboard is being routed rather than lost: an owner while its
            // popup holds it, and the popup while the owner does.
            let together = self.panes.iter().any(|p| {
                p.os_focused
                    && if lends {
                        p.id == owner
                    } else {
                        p.kind == WindowKind::Popup && !p.activates && p.owner == id
                    }
            });
            let focused = self.panes[i].os_focused || together;
            let pane = &mut self.panes[i];
            if pane.core.env.focused == focused {
                continue;
            }
            pane.core.env.focused = focused;
            if !focused {
                // The OS stops sending key events to a window that lost
                // the keyboard, so the release of anything held over a
                // Cmd-Tab would never arrive. Let go now; the synthetic
                // `up`s route out with the pending events.
                pane.core.release_held_keys();
            }
            pane.window.request_redraw();
        }
    }

    /// The app has no window with the keyboard any more, so every popup is
    /// asked to go away: a menu left standing over another application is
    /// the one thing every platform agrees is wrong.
    ///
    /// Decided here, at the end of a batch of events, and not in the
    /// `Focused` handler — because focus *moving* is two events and their
    /// order is the platform's business. A popup opening deactivates its
    /// owner on some window managers, and acting on that `Focused(false)`
    /// alone would dismiss the popup on the frame it appeared. By the time
    /// the loop is about to wait, both halves have landed and "no window of
    /// ours holds the keyboard" is a fact rather than a moment.
    fn dismiss_popups_if_deactivated(&mut self) {
        // `os_focused` and not `env.focused`: the whole point of the
        // latter is that an owner reads as focused while its popup holds
        // the keyboard, which would make this condition unreachable.
        if self.panes.iter().any(|p| p.os_focused) {
            return;
        }
        let popups: Vec<WindowId> = self
            .panes
            .iter()
            .filter(|p| p.kind == WindowKind::Popup)
            .map(|p| p.id)
            .collect();
        for id in popups {
            self.dismiss(id, DismissReason::Outside);
        }
    }

    /// Reports a dismissal to the popup's own core and routes the event.
    /// **Closes nothing**: the app stops declaring the window on the frame
    /// it decides to, exactly as it answers a `modal` node's dismissal
    /// (ADR 0003 decision 6, one level up). Raised on the popup's own core,
    /// so `UiEvent::window` is the window it is about — the `window` event
    /// cannot do that, because the window it names has just stopped or not
    /// yet started existing.
    fn dismiss(&mut self, id: WindowId, reason: DismissReason) {
        let Some(i) = self.pane_of(id) else { return };
        self.panes[i].core.dismiss_window(id, reason);
        let events = self.panes[i].core.take_pending_events();
        self.route_events(events);
    }

    /// Feeds a move the pressed pane received to every popup armed into
    /// that press, in that popup's own coordinates (ADR 0009 decision 2).
    ///
    /// The OS gives a captured drag to the window of the mouse-down, so a
    /// press on a combobox field and a drag over its menu arrive here, at
    /// the owner, and the menu is sent nothing at all. This is the driver
    /// doing what `NSMenu`'s tracking loop, Win32's menu message loop and a
    /// GTK pointer grab do — moving the events, since it cannot move the
    /// drag — and the popup's core is never told: hover, `hover_bg`,
    /// `onHover` and the popup's own drag state all follow from an ordinary
    /// `CursorMoved`. Dragging off the list costs one `CursorLeft` to the
    /// popup left behind, so a row stops highlighting the way a native
    /// menu's does, and staying off it costs nothing.
    ///
    /// The owner keeps every move it would have had (decision 3): nothing
    /// is withheld from it here and nothing is added to it.
    fn retarget_move(&mut self, event_loop: &ActiveEventLoop, from: WindowId, p: Vec2) {
        if self.primary_down != Some(from) || self.armed.is_empty() {
            return;
        }
        let Some(owner) = self.pane_of(from).and_then(|i| self.panes[i].surface()) else {
            return;
        };
        // By id rather than by index: a popup that answers one of these
        // moves by closing takes its own entry out of `armed` underneath us.
        let armed: Vec<WindowId> = self.armed.iter().map(|a| a.id).collect();
        for id in armed {
            // The pane the press is in is armed only so its own release is
            // classified (decision 4's last paragraph); it already has
            // these moves first-hand.
            if id == from {
                continue;
            }
            let Some(k) = self.armed.iter().position(|a| a.id == id) else {
                continue;
            };
            let Some(j) = self.pane_of(id) else { continue };
            let Some(popup) = self.panes[j].surface() else {
                continue;
            };
            let at = retarget::retarget(&owner, p, &popup);
            let was = std::mem::replace(&mut self.armed[k].inside, at.is_some());
            match at {
                Some(q) => {
                    self.panes[j].cursor = q;
                    self.dispatch(event_loop, j, InputEvent::CursorMoved(q));
                }
                None if was => self.dispatch(event_loop, j, InputEvent::CursorLeft),
                None => {}
            }
        }
    }

    /// Classifies the primary release that ends an armed press (ADR 0009
    /// decision 4), and disarms it whatever the answer.
    ///
    /// **Over an armed popup**, the driver synthesises the press-and-release
    /// that popup never saw, straight into its core rather than back through
    /// the `MouseInput` arm below — so the press-outside rule never sees it
    /// and cannot dismiss the window it is choosing from. The popup's core
    /// then does everything a real press-and-release there does: the focus
    /// move, the pressed styling, the click sound, and the
    /// `pressed == hovered` check that makes it a `click` at all. Neither
    /// input event carries a point, so the core presses wherever its cursor
    /// is; the retargeted moves have already put it there, and the
    /// `CursorMoved` below is for the release that arrives without one.
    ///
    /// **Inside the anchor**, nothing: the press opened the menu and the
    /// release on the field keeps it, which is how this gesture degrades
    /// into the two-click interaction that was here first.
    ///
    /// **Anywhere else**, every armed popup is asked to go away, because a
    /// native menu closes when the pointer is dragged off it and released.
    /// That is also the measured case in backlog W2 — pressed in the popup
    /// and dragged off its top edge — where the pane the press is in is the
    /// armed popup itself: a release over it is its own release and gets no
    /// synthetic pair, and a release over the field it hangs under is that
    /// popup's own anchor, mapped into the window the press is in, and
    /// keeps the menu.
    fn classify_release(&mut self, event_loop: &ActiveEventLoop, from: WindowId, p: Vec2) {
        let armed = std::mem::take(&mut self.armed);
        if armed.is_empty() {
            return;
        }
        let Some(pressed) = self.pane_of(from).and_then(|i| self.panes[i].surface()) else {
            return;
        };
        // A popup whose window will not say where it is cannot be landed
        // on; an empty surface contains nothing, which is that answer.
        let nowhere = retarget::Surface {
            origin: (0.0, 0.0),
            scale: 1.0,
            size: Size::ZERO,
        };
        let surfaces: Vec<retarget::Surface> = armed
            .iter()
            .map(|a| {
                self.pane_of(a.id)
                    .and_then(|j| self.panes[j].surface())
                    .unwrap_or(nowhere)
            })
            .collect();
        // The last armed popup's anchor, in the coordinates of the window
        // the press is in. Usually it is already in them — the popup was
        // opened against a field in that very window — but for a press that
        // began *inside* the popup the field is a rect of the window next
        // door, and it maps here the way a point does, through the screen.
        // That is the measured case in backlog W2: dragged off the popup's
        // top edge, the pointer is over the field, and releasing there
        // keeps the menu.
        let anchor = armed
            .iter()
            .rev()
            .find_map(|a| {
                let j = self.pane_of(a.id)?;
                let (rect, owner) = (self.panes[j].anchor, self.panes[j].owner);
                if owner == from {
                    return Some(rect);
                }
                let space = self.pane_of(owner).and_then(|k| self.panes[k].surface())?;
                let tl = pressed.to_local(space.to_screen(Vec2::new(rect.x, rect.y)));
                let br =
                    pressed.to_local(space.to_screen(Vec2::new(rect.x + rect.w, rect.y + rect.h)));
                Some(Rect::new(tl.x, tl.y, br.x - tl.x, br.y - tl.y))
            })
            .unwrap_or(Rect::new(0.0, 0.0, 0.0, 0.0));
        match retarget::landing(&pressed, p, anchor, &surfaces) {
            retarget::Landing::Popup { index, at } => {
                let id = armed[index].id;
                if id == from {
                    return;
                }
                if let Some(j) = self.pane_of(id)
                    && self.panes[j].cursor != at
                {
                    self.panes[j].cursor = at;
                    self.dispatch(event_loop, j, InputEvent::CursorMoved(at));
                }
                for ev in [
                    InputEvent::MouseDown {
                        button: MouseButton::Primary,
                        clicks: 1,
                    },
                    InputEvent::MouseUp {
                        button: MouseButton::Primary,
                    },
                ] {
                    // Re-found each time: the press can close the window the
                    // release is for, and then there is nothing to release.
                    let Some(j) = self.pane_of(id) else { return };
                    self.dispatch(event_loop, j, ev);
                }
            }
            retarget::Landing::Anchor => {}
            retarget::Landing::Outside => {
                for a in &armed {
                    self.dismiss(a.id, DismissReason::Outside);
                }
            }
        }
    }

    fn dispatch(&mut self, event_loop: &ActiveEventLoop, i: usize, ev: InputEvent) {
        let t0 = std::time::Instant::now();
        let events = self.panes[i].core.handle_input(ev);
        self.route_events(events);
        self.apply_window_commands(event_loop);
        self.apply_audio();
        if let Some(pane) = self.panes.get_mut(i) {
            pane.apply_cursor();
            // Hover styling depends on input too, so any input redraws. A
            // damage pass can tighten this later.
            pane.core.stats.pending_input_ms += t0.elapsed().as_secs_f32() * 1e3;
            pane.window.request_redraw();
        }
    }

    /// Applies every window command every core queued: chrome intents on
    /// the window they name, and the `Open` / `Close` the declared set's
    /// diff produced.
    fn apply_window_commands(&mut self, event_loop: &ActiveEventLoop) {
        let mut cmds = Vec::new();
        for p in &mut self.panes {
            cmds.append(&mut p.core.take_window_commands());
        }
        self.apply_commands(event_loop, cmds);
    }

    fn apply_commands(&mut self, event_loop: &ActiveEventLoop, cmds: Vec<WindowCommand>) {
        for cmd in cmds {
            match cmd {
                WindowCommand::StartDrag(id) => {
                    let Some(i) = self.pane_of(id) else { continue };
                    let pane = &mut self.panes[i];
                    let now = std::time::Instant::now();
                    let double = pane
                        .last_titlebar_press
                        .take()
                        .is_some_and(|t| now.duration_since(t).as_millis() < DOUBLE_CLICK_MS);
                    if double {
                        pane.window.set_maximized(!pane.window.is_maximized());
                    } else {
                        pane.last_titlebar_press = Some(now);
                        let _ = pane.window.drag_window();
                    }
                }
                WindowCommand::Close(id) if id == WindowId::MAIN => self.exit_requested = true,
                // The chrome close button: the user closed it, as far as
                // the declared set is concerned, so it stays closed while
                // still declared. A `Close` the diff produced has already
                // been taken out of the set, and reporting it again is a
                // no-op there.
                WindowCommand::Close(id) => self.close_pane(event_loop, id),
                WindowCommand::Minimize(id) => {
                    if let Some(i) = self.pane_of(id) {
                        self.panes[i].window.set_minimized(true);
                    }
                }
                WindowCommand::ToggleMaximize(id) => {
                    if let Some(i) = self.pane_of(id) {
                        let w = &self.panes[i].window;
                        w.set_maximized(!w.is_maximized());
                    }
                }
                // Every origin may open a window here; a host that wants
                // to refuse an extension's checks `origin` before this.
                WindowCommand::Open {
                    id, owner, config, ..
                } => {
                    if self.pane_of(id).is_none() {
                        self.open_pane(event_loop, id, owner, config);
                    }
                }
                // The app asking, rather than the declaration: a live
                // window's config is never re-read, so this is the only
                // way its size moves from inside the app (ADR 0004
                // decision 5). winit reports the size it actually applied
                // at once on some platforms and as a later `Resized` on
                // others; either way it reaches the app as the ordinary
                // `resize`, so nothing here writes the pane's viewport.
                WindowCommand::SetSize { window, size } => {
                    if let Some(i) = self.pane_of(window) {
                        let _ = self.panes[i]
                            .window
                            .request_inner_size(LogicalSize::new(size.w as f64, size.h as f64));
                    }
                }
                WindowCommand::Focus(id) => {
                    if let Some(i) = self.pane_of(id) {
                        self.panes[i].window.focus_window();
                    }
                }
            }
        }
    }

    /// Opens the window a frame declared, on the shared session and device.
    fn open_pane(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        owner: WindowId,
        config: WindowConfig,
    ) {
        let size = if config.size.w > 0.0 && config.size.h > 0.0 {
            config.size
        } else {
            WindowConfig::DEFAULT_SIZE
        };
        // Untitled until its first frame's `window_title` lands (ADR 0004
        // decision 5): the declaration carries no string.
        let mut attrs = self
            .window_attrs("", (size.w as f64, size.h as f64))
            .with_active(config.activates);
        if config.kind == WindowKind::Popup {
            // A menu surface, not a window with the app's chrome: no
            // decorations whatever the launcher asked for, above its owner,
            // and placed against the anchor rather than wherever the window
            // manager would have put a new window.
            attrs = undecorated(attrs).with_window_level(winit::window::WindowLevel::AlwaysOnTop);
            if let Some(pos) = self.popup_position(owner, config, size) {
                attrs = attrs.with_position(pos);
            }
            // "Absent from the taskbar" is a Windows and X11 fact; macOS
            // has no per-window taskbar entry to be absent from (the Dock
            // is per application), so there is nothing to ask for there.
            #[cfg(target_os = "windows")]
            {
                use winit::platform::windows::WindowAttributesExtWindows;
                attrs = attrs.with_skip_taskbar(true);
            }
            #[cfg(all(unix, not(target_os = "macos")))]
            {
                use winit::platform::x11::{WindowAttributesExtX11, WindowType};
                attrs = attrs.with_x11_window_type(vec![WindowType::PopupMenu]);
            }
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(err) => {
                eprintln!("kui: cannot open window {}: {err}", id.0);
                return;
            }
        };
        let Some(gpu) = self.gpu.clone() else {
            eprintln!("kui: cannot open window {}: no device yet", id.0);
            return;
        };
        let px = window.inner_size();
        let renderer = match kui_wgpu::Renderer::new_in(&gpu, window.clone(), px.width, px.height) {
            Ok(r) => r,
            Err(err) => {
                eprintln!("kui: cannot open window {}: {err}", id.0);
                return;
            }
        };
        let mut core = Core::new_in(&self.session);
        core.set_diagnostics(self.diagnostics);
        core.set_subpixel_text(self.subpixel);
        core.env.window.id = id;
        self.push_pane(event_loop, id, config, owner, core, window, renderer);
        // ADR 0009 decision 1: a non-activating popup that opens while the
        // primary button is down **joins that press**. Evaluated once, here,
        // with no geometry — and it is tight because of the press-outside
        // rule, which guarantees that a press in the owner while a popup is
        // up dismisses that popup: a popup opening under a held button was
        // opened by that button, unless the app declined a dismissal, in
        // which case it is still the popup the user is pressing towards. A
        // second popup opening during the same press (a submenu the app
        // opened from a retargeted hover) joins the same press, whoever owns
        // it, and is on top of the ones before it.
        if config.kind == WindowKind::Popup && !config.activates && self.primary_down.is_some() {
            self.armed.push(Armed { id, inside: false });
        }
        if !config.activates {
            // Hand the keyboard back before the platform has even said it
            // took it. Ordering a window front is enough to make it key on
            // some platforms whatever `with_active(false)` asked for, and
            // `settle_focus` will ask again when it sees that happen — but
            // that is a batch later, and a batch is long enough for the
            // window behind to be drawn without its key styling. Asking
            // eagerly here closes the gap at the one moment it is
            // predictable; `settle_focus` covers every other way a popup
            // can end up with the keyboard, a press on it above all.
            if let Some(j) = self.pane_of(owner) {
                self.panes[j].window.focus_window();
            }
        }
    }

    /// Where a popup's top-left goes, in screen coordinates: below the
    /// anchor its owner reported, flipped above it when the monitor's
    /// bottom edge is nearer than the popup is tall — the placement a menu
    /// wants, and the one `FloatConfig::fit` cannot make, since it clamps
    /// into the window instead of leaving it.
    ///
    /// The anchor arrives in the owner's own logical coordinates (it is a
    /// rect an `onLayout` node reported), so this is where it stops being a
    /// window fact and becomes a screen one. `None` when the owner is gone
    /// or the platform will not say where it is; the window manager then
    /// places the popup and the app is no worse off than a float.
    fn popup_position(
        &self,
        owner: WindowId,
        config: WindowConfig,
        size: Size,
    ) -> Option<LogicalPosition<f64>> {
        let pane = self.panes.get(self.pane_of(owner)?)?;
        let scale = pane.window.scale_factor();
        let origin = pane.window.inner_position().ok()?.to_logical::<f64>(scale);
        let a = config.anchor;
        let x = origin.x + a.x as f64;
        let below = origin.y + (a.y + a.h) as f64;
        // The monitor the owner is on, in its own logical coordinates. With
        // no monitor to ask, "below" is the answer and the WM may move it.
        let y = match pane.window.current_monitor() {
            Some(m) => {
                let top = m.position().to_logical::<f64>(scale).y;
                let height = m.size().to_logical::<f64>(scale).height;
                if below + size.h as f64 > top + height {
                    // Above the anchor instead, unless there is even less
                    // room up there — then stay below and let it clip.
                    let above = origin.y + a.y as f64 - size.h as f64;
                    if above >= top { above } else { below }
                } else {
                    below
                }
            }
            None => below,
        };
        Some(LogicalPosition::new(x, y))
    }

    /// Finishes a window whose surface and renderer exist: the platform
    /// hooks, the pane, and showing it.
    #[allow(
        clippy::too_many_arguments,
        reason = "one private call site; every argument is a distinct fact about the \
                  window being adopted, and a struct to carry them would be built and \
                  destructured in the same breath"
    )]
    fn push_pane(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
        config: WindowConfig,
        owner: WindowId,
        core: Core,
        window: Arc<Window>,
        renderer: kui_wgpu::Renderer,
    ) {
        let access = self
            .proxy
            .clone()
            .and_then(|proxy| access_bridge::Bridge::new(event_loop, &window, proxy));
        #[cfg(target_os = "windows")]
        let nc = (self.chrome != Chrome::Native)
            .then(|| windows_nc::NcHitTest::install(&window, true))
            .flatten();
        window.set_visible(true);
        window.set_ime_allowed(true);
        window.request_redraw();
        self.panes.push(Pane {
            id,
            kind: config.kind,
            owner,
            activates: config.activates,
            anchor: config.anchor,
            core,
            window,
            renderer,
            applied_title: String::new(),
            modifiers: ModifiersState::empty(),
            last_titlebar_press: None,
            cursor: Vec2::ZERO,
            last_click: None,
            blink_visible: true,
            blink_deadline: None,
            caret_stamp_seen: 0,
            resize_edge: None,
            cursor_icon: CursorIcon::Default,
            os_focused: false,
            handed_back: false,
            first_frame: Some((FIRST_FRAME_RETRIES, std::time::Instant::now())),
            access,
            #[cfg(target_os = "windows")]
            nc,
        });
    }

    /// The attributes every window of this app is created with: the
    /// launcher's chrome, hidden until the accessibility adapter has
    /// hooked it (the platform adapters must see the window before it is
    /// shown).
    fn window_attrs(&self, title: &str, (w, h): (f64, f64)) -> winit::window::WindowAttributes {
        #[allow(unused_mut)]
        let mut attrs = Window::default_attributes()
            .with_title(title)
            .with_inner_size(LogicalSize::new(w, h))
            .with_visible(false);
        match self.chrome {
            Chrome::Native => {}
            Chrome::Custom => {
                // macOS: keep the native traffic lights, drawn over our
                // content; everywhere else drop decorations entirely.
                #[cfg(target_os = "macos")]
                {
                    use winit::platform::macos::WindowAttributesExtMacOS;
                    attrs = attrs
                        .with_titlebar_transparent(true)
                        .with_fullsize_content_view(true)
                        .with_title_hidden(true);
                }
                #[cfg(not(target_os = "macos"))]
                {
                    attrs = attrs.with_decorations(false);
                }
            }
            Chrome::Borderless => attrs = undecorated(attrs),
        }
        attrs
    }

    /// Closes a window other than the main one: the user did (its chrome
    /// or OS close button), or the diff stopped declaring it. The core is
    /// told either way — a window the diff already closed is a no-op there
    /// — and whatever it queues in answer (the `closed` event, and the
    /// `Close` of anything only this window declared) is routed before the
    /// pane goes.
    fn close_pane(&mut self, event_loop: &ActiveEventLoop, id: WindowId) {
        let Some(i) = self.pane_of(id) else { return };
        // Whatever this press was about, it is not about this window any
        // more (ADR 0009): a popup chosen from closes here, and its release
        // must not be classified against a surface that has stopped
        // existing.
        self.armed.retain(|a| a.id != id);
        // A non-activating popup that holds the keyboard is about to stop
        // existing, so the keyboard is going *somewhere*: say where, rather
        // than letting the owner read as unfocused until the platform gets
        // round to confirming it. Without this, choosing an item from a
        // menu — the click that closes it — flashes the window behind it
        // unfocused for a frame or two.
        let hand_back = (self.panes[i].kind == WindowKind::Popup
            && !self.panes[i].activates
            && self.panes[i].os_focused)
            .then_some(self.panes[i].owner);
        self.panes[i].core.window_closed(id);
        let events = self.panes[i].core.take_pending_events();
        let cmds = self.panes[i].core.take_window_commands();
        self.panes.remove(i);
        if let Some(j) = hand_back.and_then(|o| self.pane_of(o)) {
            self.panes[j].os_focused = true;
            self.panes[j].window.focus_window();
        }
        self.settle_focus();
        self.route_events(events);
        self.apply_commands(event_loop, cmds);
        for p in &self.panes {
            p.window.request_redraw();
        }
    }

    /// Hands the session's queued audio commands to the device. A session
    /// that holds a sound is going to play one: the device starts opening
    /// here, on its own thread, so the first play finds it open instead of
    /// stalling the frame for the ~90 ms the open takes.
    fn apply_audio(&mut self) {
        let core = self.core_mut();
        let cmds = core.take_audio_commands();
        let resources = core.resources.clone();
        if resources.has_sounds() {
            self.audio.warm();
        }
        // A `Stop` the device found still playing is a one-shot cut off,
        // which only a device can tell; the core turns the ones it queued
        // for a departing `audio` node into `truncated-playback`.
        let truncated = self.audio.apply(cmds, &resources);
        if truncated.is_empty() {
            return;
        }
        let core = self.core_mut();
        for (playback, at) in truncated {
            core.audio_truncated(playback, at);
        }
    }

    /// Folds playbacks that finished on their own back into the core, and
    /// routes the `sound` events tagged ones become.
    fn poll_audio(&mut self) {
        let ended = self.audio.poll_ended();
        if ended.is_empty() {
            return;
        }
        let core = self.core_mut();
        for playback in ended {
            core.audio_ended(playback);
        }
        let pending = core.take_pending_events();
        if !pending.is_empty() {
            self.route_events(pending);
            for p in &self.panes {
                p.window.request_redraw();
            }
        }
    }

    fn route_events(&mut self, events: Vec<UiEvent>) {
        let mut reached_app = false;
        for ev in events {
            if ev.origin == OriginId::HOST {
                reached_app = true;
                self.app.on_event(ev);
            } else if let Some(ext) = self.extensions.by_origin(ev.origin) {
                // An extension's replies go to the host (ADR 0014 decision
                // 6): not routed by origin — a reply is addressed by being
                // one — and carrying the extension's origin, the window and
                // the key of the event it answered, so the host knows who
                // spoke and from where.
                for payload in ext.on_event(&ev) {
                    reached_app = true;
                    self.app.on_event(UiEvent {
                        origin: ev.origin,
                        window: ev.window,
                        key: ev.key,
                        payload,
                    });
                }
            }
        }
        // One app, one model, N windows: a handler that ran in answer to
        // input in *this* window can change what *another* window declares
        // — choosing an item in a popup is the app closing the popup, and
        // the declaration that closes it lives in the window that opened
        // it. So anything that reached the app redraws every window; the
        // caller has already redrawn the one the input landed in. Guarded
        // on there being more than one, so the single-window path — every
        // hover, every keystroke — is exactly what it was.
        if reached_app && self.panes.len() > 1 {
            for p in &self.panes {
                p.window.request_redraw();
            }
        }
    }

    /// Direct edits (cut) mutate the document outside handle_input, so they
    /// must notify + redraw explicitly.
    fn after_direct_edit(&mut self, i: usize) {
        let pane = &mut self.panes[i];
        if let Some(key) = pane.core.edit.focused() {
            let origin = pane.core.edit.origin_of(key).unwrap_or(OriginId::HOST);
            let ev = UiEvent {
                origin,
                // Built outside the core, so this driver stamps it itself.
                window: pane.core.env.window.id,
                key,
                payload: Value::map([("kind", "changed".into())]),
            };
            self.route_events(vec![ev]);
        }
        self.panes[i].window.request_redraw();
    }

    fn on_key(&mut self, event_loop: &ActiveEventLoop, i: usize, event: winit::event::KeyEvent) {
        let pressed = event.state == ElementState::Pressed;
        // Full-keyboard path: every press *and release* travels as data to
        // the key-focused sink (`NodeSpec::on_key`) — the core delivers the
        // release only to a sink that said `key_up`, and drops both when
        // an edit widget holds focus instead. Everything below this block
        // is the editor path, which is press-only.
        let kmods = self.panes[i].kmods();
        let plain = !kmods.ctrl && !kmods.alt && !kmods.super_key;
        // With Alt held the logical key is the composed character on some
        // layouts (macOS ⌥o → "ø"); chords want the layout key, so report
        // the modifier-stripped one instead.
        let logical = if kmods.alt {
            use winit::platform::modifier_supplement::KeyEventExtModifierSupplement;
            event.key_without_modifiers()
        } else {
            event.logical_key.clone()
        };
        // Where the key *is*, which no layout moves.
        let physical = physical_code(event.physical_key);
        let (logical_code, ktext) = match &logical {
            WinitKey::Character(s) => (
                KeyCode::Char(s.chars().next().unwrap_or('\u{fffd}')),
                plain.then(|| s.to_string()),
            ),
            WinitKey::Named(n) => (
                match n {
                    NamedKey::Space => KeyCode::Space,
                    NamedKey::ArrowLeft => KeyCode::Left,
                    NamedKey::ArrowRight => KeyCode::Right,
                    NamedKey::ArrowUp => KeyCode::Up,
                    NamedKey::ArrowDown => KeyCode::Down,
                    NamedKey::Home => KeyCode::Home,
                    NamedKey::End => KeyCode::End,
                    NamedKey::PageUp => KeyCode::PageUp,
                    NamedKey::PageDown => KeyCode::PageDown,
                    NamedKey::Backspace => KeyCode::Backspace,
                    NamedKey::Delete => KeyCode::Delete,
                    NamedKey::Enter => KeyCode::Enter,
                    NamedKey::Tab => KeyCode::Tab,
                    NamedKey::Escape => KeyCode::Escape,
                    NamedKey::Insert => KeyCode::Insert,
                    NamedKey::F1 => KeyCode::F(1),
                    NamedKey::F2 => KeyCode::F(2),
                    NamedKey::F3 => KeyCode::F(3),
                    NamedKey::F4 => KeyCode::F(4),
                    NamedKey::F5 => KeyCode::F(5),
                    NamedKey::F6 => KeyCode::F(6),
                    NamedKey::F7 => KeyCode::F(7),
                    NamedKey::F8 => KeyCode::F(8),
                    NamedKey::F9 => KeyCode::F(9),
                    NamedKey::F10 => KeyCode::F(10),
                    NamedKey::F11 => KeyCode::F(11),
                    NamedKey::F12 => KeyCode::F(12),
                    _ => KeyCode::Unknown,
                },
                (plain && *n == NamedKey::Space).then(|| " ".to_string()),
            ),
            _ => (KeyCode::Unknown, None),
        };
        // `KeyPress::from_layout` resolves the two into the code a keymap
        // binds against — the layout's key while it speaks ASCII, the
        // US-QWERTY letter at that position when it does not. Every driver
        // goes through it, so a C host with its own windowing gets the same
        // rule as this one.
        let kp = KeyPress::from_layout(logical_code, physical, kmods);
        let kp = KeyPress {
            text: ktext,
            repeat: event.repeat,
            ..kp
        };
        if kp.code != KeyCode::Unknown {
            self.dispatch(
                event_loop,
                i,
                if pressed {
                    InputEvent::KeyDown(kp.clone())
                } else {
                    InputEvent::KeyUp(kp.clone().released())
                },
            );
        }
        if !pressed {
            return;
        }

        // Clipboard + select-all shortcuts (edit widgets only — a key
        // sink gets the raw chord and brings its own bindings).
        let pane = &mut self.panes[i];
        if pane.core.edit.focused().is_some()
            && pane.primary()
            && let WinitKey::Character(c) = &event.logical_key
        {
            match c.to_lowercase().as_str() {
                "c" => {
                    if let (Some(text), Some(cb)) =
                        (pane.core.copy_selection(), self.clipboard.as_mut())
                    {
                        let _ = cb.set_text(text);
                    }
                    return;
                }
                "x" => {
                    if let Some(text) = pane.core.cut_selection() {
                        if let Some(cb) = self.clipboard.as_mut() {
                            let _ = cb.set_text(text);
                        }
                        self.after_direct_edit(i);
                    }
                    return;
                }
                "v" => {
                    if let Some(text) = self.clipboard.as_mut().and_then(|cb| cb.get_text().ok()) {
                        self.dispatch(event_loop, i, InputEvent::Text(text));
                    }
                    return;
                }
                "a" => {
                    self.dispatch(
                        event_loop,
                        i,
                        InputEvent::Key(EditKey::SelectAll, Mods::default()),
                    );
                    return;
                }
                "z" => {
                    let key = if pane.modifiers.shift_key() {
                        EditKey::Redo
                    } else {
                        EditKey::Undo
                    };
                    self.dispatch(event_loop, i, InputEvent::Key(key, Mods::default()));
                    return;
                }
                "y" => {
                    self.dispatch(
                        event_loop,
                        i,
                        InputEvent::Key(EditKey::Redo, Mods::default()),
                    );
                    return;
                }
                _ => {}
            }
        }

        // The editor channel: the same press again, as what the core is
        // asked to *do* with that key. The table lives in the core
        // (`KeyPress::edit_event`) rather than here, so this runner and
        // every headless injector send the same second event for the same
        // key — pressing Escape dismissed a modal in a window and did
        // nothing in a test for as long as there were two copies of it
        // (backlog F6).
        if let Some(ev) = kp.edit_event() {
            // A popup owns Escape the way a modal node does, and for the
            // same reason (ADR 0003, one level up): it asks to go away, and
            // nothing else happens. A modal *inside* the popup is asked
            // first, which is the core's own precedence read at this level
            // — the surface nearest the user answers.
            let pane = &self.panes[i];
            if matches!(ev, InputEvent::Key(EditKey::Escape, _))
                && pane.kind == WindowKind::Popup
                && pane.core.modal().is_none()
            {
                let id = pane.id;
                self.dismiss(id, DismissReason::Escape);
                return;
            }
            self.dispatch(event_loop, i, ev);
            return;
        }
        // Plain typed text (IME commits arrive via WindowEvent::Ime). Not
        // the core's table's business: this is the *composed* character
        // the platform produced, which a chord-view `KeyPress` does not
        // carry — macOS's ⌥o is "ø" here and no text at all there.
        let pane = &self.panes[i];
        if !pane.primary()
            && !pane.modifiers.control_key()
            && let Some(text) = &event.text
            && text.chars().any(|c| !c.is_control())
        {
            self.dispatch(event_loop, i, InputEvent::Text(text.to_string()));
        }
    }

    /// Builds and draws one window's frame.
    fn redraw(&mut self, i: usize) {
        let Shell {
            app,
            extensions,
            panes,
            chrome,
            epoch,
            ..
        } = self;
        let pane = &mut panes[i];
        let window = &pane.window;
        let (viewport, scale) = pane.size();
        let size = window.inner_size();
        // The core turns a changed viewport into a `resize` event, routed
        // with the rest of the pending events after this frame.
        // Per-frame so it self-corrects when the window moves to another
        // monitor.
        pane.core.env.refresh_hz = window
            .current_monitor()
            .and_then(|m| m.refresh_rate_millihertz())
            .map(|mhz| mhz as f32 / 1000.0);
        pane.core.env.window = WindowEnv {
            id: pane.id,
            custom_chrome: *chrome != Chrome::Native,
            maximized: window.is_maximized(),
            fullscreen: window.fullscreen().is_some(),
            native_controls: (cfg!(target_os = "macos") && *chrome == Chrome::Custom)
                .then_some(MACOS_TRAFFIC_LIGHTS),
        };

        let t_view = std::time::Instant::now();
        pane.core.set_time(epoch.elapsed().as_secs_f64());
        // The extensions fill the slots the host's view declares, in place
        // (`Ui::slot`), and `"root"` after it unless the host placed that
        // too — `finish` below does the latter and reports slots nobody
        // declared (ADR 0014). The core numbers their origins.
        let mut ui = pane.core.frame_with(viewport, scale, extensions);
        app.view(&mut ui);
        let view_ms = t_view.elapsed().as_secs_f32() * 1e3;

        let t_layout = std::time::Instant::now();
        ui.finish();
        let layout_ms = t_layout.elapsed().as_secs_f32() * 1e3;

        // Silent misconfigurations the core noticed (a grow weight with
        // nothing to split against, a transition on a positional key, two
        // nodes on one key): each once, to stderr, so they stop looking
        // like "the feature is broken".
        for w in pane.core.take_warnings() {
            eprintln!(
                "kui: warning [{}] node {:016x}: {}",
                w.code, w.key.0, w.message
            );
        }

        // Mirror this frame's hit regions into the WM_NCHITTEST answerer.
        #[cfg(target_os = "windows")]
        if let Some(nc) = &pane.nc {
            nc.update(
                scale,
                window.is_maximized(),
                pane.core
                    .interaction
                    .hits()
                    .iter()
                    .map(|h| (h.window, h.rect.intersect(&h.clip))),
            );
        }

        if let Some(t) = pane.core.window_title()
            && t != pane.applied_title
        {
            pane.applied_title = t.to_string();
            window.set_title(&pane.applied_title);
        }

        // Anchor the OS IME candidate window at the focused caret.
        if let Some(r) = pane.core.ime_rect() {
            window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(r.x, r.y),
                winit::dpi::LogicalSize::new(r.w.max(1.0), r.h),
            );
        }

        let t_render = std::time::Instant::now();
        let (dl, atlas) = pane.core.output();
        let mut wait_ms = 0.0;
        match pane.renderer.render(dl, atlas) {
            Ok(report) => {
                wait_ms = report.vsync_wait_ms;
                pane.first_frame = None;
            }
            Err(kui_wgpu::RenderError::Reconfigure) => {
                pane.renderer.resize(size.width, size.height);
                window.request_redraw();
            }
            // Occluded or timed out: nothing to present, try next frame.
            // Occluded or timed out: nothing to present, try next frame —
            // and, until a window has managed one, *schedule* that next
            // frame. A window ordered front reports itself occluded for a
            // beat or two before the platform catches up, and nothing else
            // was asking for a redraw, so its first frame never landed and
            // it sat blank until a stray mouse move woke it. Only until it
            // has presented once, and only for a bounded number of tries,
            // so a window that really is hidden does not spin.
            Err(kui_wgpu::RenderError::Skip) => {}
            Err(err) => eprintln!("kui: render error: {err}"),
        }
        let render_ms = (t_render.elapsed().as_secs_f32() * 1e3 - wait_ms).max(0.0);

        pane.core.stats.push(FrameSample {
            input_ms: 0.0,
            view_ms,
            layout_ms,
            render_ms,
            wait_ms,
        });
    }
}

impl<A: App> ApplicationHandler<access_bridge::UserEvent> for Shell<A> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.panes.is_empty() {
            return;
        }
        // KUI_WINDOW=WxH overrides the initial size (useful for testing).
        let size = std::env::var("KUI_WINDOW")
            .ok()
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .map(|s| clamp_size(s, self.min_size, self.max_size))
            .unwrap_or(self.size);
        let mut attrs = self.window_attrs(&self.title.clone(), size);
        if let Some((mw, mh)) = self.min_size {
            attrs = attrs.with_min_inner_size(LogicalSize::new(mw, mh));
        }
        if let Some((mw, mh)) = self.max_size {
            attrs = attrs.with_max_inner_size(LogicalSize::new(mw, mh));
        }
        let window = Arc::new(event_loop.create_window(attrs).expect("create window"));
        let px = window.inner_size();
        let renderer =
            pollster::block_on(kui_wgpu::Renderer::new(window.clone(), px.width, px.height))
                .expect("init renderer");
        // Subpixel text only where the renderer blends per channel; the
        // env var wins over the builder for quick A/B comparisons.
        let wanted = match std::env::var("KUI_TEXT_AA").ok().as_deref() {
            Some("gray") | Some("grayscale") => TextAa::Grayscale,
            Some("subpixel") | Some("lcd") => TextAa::Subpixel,
            _ => self.text_aa,
        };
        self.subpixel = match wanted {
            TextAa::Grayscale => false,
            TextAa::Subpixel | TextAa::Auto => renderer.subpixel_text(),
        };
        self.gpu = Some(renderer.gpu().clone());
        let mut core = self
            .main_core
            .take()
            .expect("the main core is built once, by the launcher");
        core.set_subpixel_text(self.subpixel);
        core.env.window.id = WindowId::MAIN;
        self.push_pane(
            event_loop,
            WindowId::MAIN,
            WindowConfig::default(),
            WindowId::MAIN,
            core,
            window,
            renderer,
        );
        self.panes[0].applied_title = self.title.clone();
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WinitWindowId,
        event: WindowEvent,
    ) {
        let Some(i) = self.pane_index(id) else { return };
        {
            let pane = &mut self.panes[i];
            if let Some(bridge) = &mut pane.access {
                bridge.process_event(&pane.window, &event);
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.panes[i].id == WindowId::MAIN {
                    event_loop.exit();
                } else {
                    let id = self.panes[i].id;
                    self.close_pane(event_loop, id);
                }
            }
            WindowEvent::Resized(size) => {
                let pane = &mut self.panes[i];
                pane.renderer.resize(size.width, size.height);
                pane.window.request_redraw();
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.panes[i].window.request_redraw();
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pane = &mut self.panes[i];
                let scale = pane.window.scale_factor() as f32;
                let p = Vec2::new(position.x as f32 / scale, position.y as f32 / scale);
                if pane.synthesizes_resize(self.chrome) {
                    pane.resize_edge = pane.resize_edge_at(p);
                }
                pane.cursor = p;
                let from = pane.id;
                self.dispatch(event_loop, i, InputEvent::CursorMoved(p));
                // And then, if this pane is holding a press that a popup
                // joined, the same move again in that popup's coordinates.
                self.retarget_move(event_loop, from, p);
            }
            WindowEvent::CursorLeft { .. } => self.dispatch(event_loop, i, InputEvent::CursorLeft),
            // Recorded, not acted on: what a view reads is derived from
            // every window's copy at the end of the batch (`settle_focus`),
            // because focus *moving* is two events and neither alone is the
            // answer.
            WindowEvent::Focused(focused) => self.panes[i].os_focused = focused,
            // The four keyboard events go to `key_target`, which is this
            // pane unless it is lending its keyboard to a non-activating
            // popup. The modifier mirror follows them, or the popup would
            // read a stale Shift.
            WindowEvent::ModifiersChanged(m) => {
                let t = self.key_target(i);
                self.panes[t].modifiers = m.state();
                let kmods = self.panes[t].kmods();
                self.dispatch(event_loop, t, InputEvent::Modifiers(kmods));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let t = self.key_target(i);
                self.on_key(event_loop, t, event)
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                // Its own channel, not `Text`: a sink hears a commit as a
                // `text` event and a keystroke as a `key` event, once each
                // (backlog C17); a stock editor takes both the same way.
                let t = self.key_target(i);
                self.dispatch(event_loop, t, InputEvent::Commit(text))
            }
            WindowEvent::Ime(Ime::Preedit(text, cursor)) => {
                let t = self.key_target(i);
                self.dispatch(event_loop, t, InputEvent::Preedit(text, cursor));
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let scale = self.panes[i].window.scale_factor() as f32;
                let d = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        Vec2::new(Core::lines_to_px(x), Core::lines_to_px(y))
                    }
                    winit::event::MouseScrollDelta::PixelDelta(p) => {
                        Vec2::new(p.x as f32 / scale, p.y as f32 / scale)
                    }
                };
                self.dispatch(event_loop, i, InputEvent::Scroll(d));
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    WinitButton::Left => MouseButton::Primary,
                    WinitButton::Right => MouseButton::Secondary,
                    WinitButton::Middle => MouseButton::Middle,
                    // Nothing routes these, so the numbering only has to be
                    // stable: back, forward, then whatever the platform
                    // reports beyond them.
                    WinitButton::Back => MouseButton::Other(0),
                    WinitButton::Forward => MouseButton::Other(1),
                    WinitButton::Other(n) => {
                        MouseButton::Other(n.saturating_add(2).min(u8::MAX as u16) as u8)
                    }
                };
                let primary = button == MouseButton::Primary;
                let here = self.panes[i].id;
                // Which pane holds a primary press, for ADR 0009's arming.
                // Set for a consumed press too: the button is down whatever
                // the core was told, and the release below clears it.
                if primary {
                    self.primary_down = (state == ElementState::Pressed).then_some(here);
                    if state == ElementState::Pressed {
                        // A consumed press whose release never arrived — the
                        // window lost the keyboard mid-gesture, say — must
                        // not swallow the next one instead.
                        self.swallowed_press = None;
                    }
                }
                // A press anywhere but inside a popup is "outside" it —
                // the owner's own window included, which is exactly the
                // line a separate surface draws. Reported before the press
                // is dispatched, so an app that stops declaring the popup
                // on the dismissal still sees the click it was dismissed
                // by, the way a modal's dismissal works (ADR 0003).
                //
                // And then a primary press that dismissed a
                // **non-activating** popup is **consumed** — not dispatched
                // to the window it landed in, and its release swallowed with
                // it (ADR 0009 decision 5). The pass-through this replaces
                // claimed to mirror ADR 0003 and had it backwards: under an
                // in-window modal everything outside emits no hit region, so
                // the outside press dismisses and lands on nothing, while a
                // popup's owner is live (ADR 0004 decision 10) and the press
                // dismisses *and* acts. Invisible with click-to-open;
                // decisive with open-on-press, where a press on the open
                // field would otherwise dismiss and reopen in one gesture,
                // and no ordering of the two events lets the app tell that
                // press from the first. An **activating** popup — a tear-off
                // panel — keeps the pass-through, since someone working in a
                // panel beside the app expects a click in the app to act.
                if state == ElementState::Pressed {
                    let mut consumed = false;
                    for (id, activates) in self.popups_outside(i) {
                        self.dismiss(id, DismissReason::Outside);
                        consumed |= primary && !activates;
                    }
                    if consumed {
                        self.swallowed_press = Some(here);
                        for p in &self.panes {
                            p.window.request_redraw();
                        }
                        return;
                    }
                }
                // A press inside a non-activating popup arms that popup for
                // its own release (ADR 0009 decision 4's last paragraph):
                // the OS keeps the drag here whatever it wanders over, so a
                // drag off the top edge and a release on the desktop has to
                // be classified rather than ignored — the measured case in
                // backlog W2. It gets no retargeted moves, having the real
                // ones already.
                if primary
                    && state == ElementState::Pressed
                    && self.panes[i].kind == WindowKind::Popup
                    && !self.panes[i].activates
                    && !self.armed.iter().any(|a| a.id == here)
                {
                    self.armed.push(Armed {
                        id: here,
                        inside: true,
                    });
                }
                if state == ElementState::Released && primary {
                    // The release of a press the driver ate goes with it:
                    // the core never saw the `down`, so nothing should see
                    // this `up`.
                    if self.swallowed_press == Some(here) {
                        self.swallowed_press = None;
                        self.primary_down = None;
                        return;
                    }
                    let at = self.panes[i].cursor;
                    self.classify_release(event_loop, here, at);
                }
                // The frame the classification ran may have closed a window
                // and moved this one down the list.
                let Some(i) = self.pane_of(here) else { return };
                let pane = &mut self.panes[i];
                // A press on the synthesized resize band starts an OS resize
                // instead of reaching the UI.
                if primary
                    && state == ElementState::Pressed
                    && let Some(dir) = pane.resize_edge
                {
                    let _ = pane.window.drag_resize_window(dir);
                    return;
                }
                let ev = match state {
                    ElementState::Pressed => {
                        // Multi-click is the primary button's: a right
                        // press between two left ones does not break the
                        // run, and never counts up one of its own.
                        let clicks = if primary {
                            let now = std::time::Instant::now();
                            let clicks = match pane.last_click {
                                Some((t, p, n))
                                    if now.duration_since(t).as_millis() < MULTI_CLICK_MS
                                        && (p.x - pane.cursor.x).abs() < MULTI_CLICK_SLOP
                                        && (p.y - pane.cursor.y).abs() < MULTI_CLICK_SLOP =>
                                {
                                    // Cycle 1 → 2 → 3 → 1 like most editors.
                                    n % 3 + 1
                                }
                                _ => 1,
                            };
                            pane.last_click = Some((now, pane.cursor, clicks));
                            clicks
                        } else {
                            1
                        };
                        InputEvent::MouseDown { button, clicks }
                    }
                    ElementState::Released => InputEvent::MouseUp { button },
                };
                self.dispatch(event_loop, i, ev);
            }
            WindowEvent::RedrawRequested => {
                self.redraw(i);
                self.panes[i].publish_access();
                // Views can declare windows and window commands too
                // (ui.window, ui.window_command); apply them the same frame
                // they were declared. Likewise the sounds a frame started
                // (audio nodes, ui.play).
                self.apply_window_commands(event_loop);
                self.apply_audio();
                // The frame may have closed this very pane.
                let Some(i) = self.pane_index(id) else { return };
                // A new frame can put something else under a still cursor.
                self.panes[i].apply_cursor();
                // A frame can resize the viewport, and can change what sits
                // under a still cursor; route the resulting resize / hover
                // events now rather than with the next input, and redraw for
                // what they change.
                let pending = self.panes[i].core.take_pending_events();
                if !pending.is_empty() {
                    self.route_events(pending);
                    if let Some(p) = self.panes.get(i) {
                        p.window.request_redraw();
                    }
                }
                // Windows moves and resizes a window inside its own modal
                // loop, where `about_to_wait` — the pacing that asks for
                // the next frame of an animation — does not run, so a
                // transition froze while the title bar was held (backlog
                // W3). The modal loop does dispatch WM_PAINT, so an
                // animating pane asks for its next frame from here as
                // well; vsync paces it as before. Unverified on the
                // platform: filed from a report, not a reproduction.
                #[cfg(target_os = "windows")]
                if let Some(p) = self.panes.get(i)
                    && p.core.animating()
                {
                    p.window.request_redraw();
                }
            }
            _ => {}
        }
        if self.exit_requested {
            event_loop.exit();
        }
    }

    /// AccessKit's side of the conversation: assistive technology attaching
    /// (send it the tree), detaching, or asking for an action (input).
    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: access_bridge::UserEvent) {
        // A wake is the app saying "what `view` shows has changed": every
        // window draws, as after any input. Coalesced by the platform's
        // queue, so a thread waking a thousand times a frame costs one.
        if matches!(event, access_bridge::UserEvent::Wake) {
            for pane in &self.panes {
                pane.window.request_redraw();
            }
            return;
        }
        let Some(i) = access_bridge::window_of(&event).and_then(|w| self.pane_index(w)) else {
            return;
        };
        let Some(bridge) = &mut self.panes[i].access else {
            return;
        };
        if let Some(req) = bridge.on_event(event) {
            self.dispatch(event_loop, i, InputEvent::Access(req));
        }
        if let Some(pane) = self.panes.get_mut(i) {
            pane.publish_access();
        }
    }

    /// Runs after every event batch (including timer wake-ups): the caret
    /// blink clock, the transition clock, and the audio poll. Any caret
    /// activity re-arms the blink timer with the caret solid; each expiry
    /// toggles the phase and schedules the next. A transition mid-flight
    /// asks for the next frame right away (vsync paces it). While a sound
    /// plays, the loop wakes every `AUDIO_POLL` to notice it finishing.
    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.settle_focus();
        self.dismiss_popups_if_deactivated();
        self.poll_audio();
        self.apply_audio();
        let now = std::time::Instant::now();
        let mut deadline: Option<std::time::Instant> = None;
        for pane in &mut self.panes {
            if pane.core.animating() {
                pane.window.request_redraw();
            }
            // A window still waiting for its first frame asks again — one
            // frame apart, and only so many times. Paced rather than spun:
            // asking again the instant a try fails would burn every try in
            // a millisecond, which is exactly how long the platform has
            // *not* had to stop calling a just-shown window occluded.
            if let Some((left, at)) = &mut pane.first_frame
                && *left > 0
            {
                if now >= *at {
                    *left -= 1;
                    *at = now + FIRST_FRAME_RETRY;
                    pane.window.request_redraw();
                }
                deadline = Some(deadline.map_or(*at, |d| d.min(*at)));
            }
            if pane.core.edit.focused().is_none() {
                if !pane.blink_visible {
                    pane.blink_visible = true;
                    pane.core.edit.set_blink_visible(true);
                }
                pane.blink_deadline = None;
                continue;
            }
            let stamp = pane.core.edit.caret_stamp();
            if stamp != pane.caret_stamp_seen || pane.blink_deadline.is_none() {
                pane.caret_stamp_seen = stamp;
                pane.blink_deadline = Some(now + BLINK_INTERVAL);
                if !pane.blink_visible {
                    pane.blink_visible = true;
                    pane.core.edit.set_blink_visible(true);
                    pane.window.request_redraw();
                }
            } else if now >= pane.blink_deadline.unwrap() {
                pane.blink_visible = !pane.blink_visible;
                pane.core.edit.set_blink_visible(pane.blink_visible);
                pane.blink_deadline = Some(now + BLINK_INTERVAL);
                pane.window.request_redraw();
            }
            if let Some(d) = pane.blink_deadline {
                deadline = Some(deadline.map_or(d, |e| e.min(d)));
            }
        }
        if self.audio.active() {
            let poll = now + AUDIO_POLL;
            deadline = Some(deadline.map_or(poll, |d| d.min(poll)));
        }
        event_loop.set_control_flow(match deadline {
            Some(d) => ControlFlow::WaitUntil(d),
            None => ControlFlow::Wait,
        });
    }
}

// Re-exported so apps can reach the renderer without depending on kui-wgpu.
pub use kui_wgpu::{RenderError, Renderer, wgpu};

#[cfg(test)]
mod tests {
    use super::*;

    struct Empty;
    impl App for Empty {
        fn view(&mut self, _ui: &mut Ui<'_>) {}
    }

    #[test]
    fn clamp_size_honors_each_bound() {
        let bounds = (Some((400.0, 300.0)), Some((1200.0, 900.0)));
        assert_eq!(
            clamp_size((800.0, 600.0), bounds.0, bounds.1),
            (800.0, 600.0)
        );
        assert_eq!(
            clamp_size((100.0, 100.0), bounds.0, bounds.1),
            (400.0, 300.0)
        );
        assert_eq!(
            clamp_size((4000.0, 4000.0), bounds.0, bounds.1),
            (1200.0, 900.0)
        );
        // Per-axis, and unbounded sides pass through untouched.
        assert_eq!(
            clamp_size((100.0, 4000.0), bounds.0, bounds.1),
            (400.0, 900.0)
        );
        assert_eq!(clamp_size((10.0, 10.0), None, None), (10.0, 10.0));
        // A max below the min loses to it, as the platforms resolve it.
        assert_eq!(
            clamp_size((800.0, 600.0), Some((500.0, 500.0)), Some((200.0, 200.0))),
            (500.0, 500.0)
        );
    }

    #[test]
    fn diagnostics_follow_the_build_unless_told_otherwise() {
        assert_eq!(
            app("t").shell(Empty).core_mut().diagnostics(),
            cfg!(debug_assertions)
        );
        assert!(
            app("t")
                .diagnostics(true)
                .shell(Empty)
                .core_mut()
                .diagnostics()
        );
        assert!(
            !app("t")
                .diagnostics(false)
                .shell(Empty)
                .core_mut()
                .diagnostics()
        );
    }

    #[test]
    fn launcher_clamps_the_initial_size_into_the_bounds() {
        let shell = app("t")
            .size(320.0, 240.0)
            .min_size(640.0, 480.0)
            .shell(Empty);
        assert_eq!(shell.size, (640.0, 480.0));
        assert_eq!(shell.min_size, Some((640.0, 480.0)));

        let shell = app("t")
            .size(1600.0, 1200.0)
            .max_size(800.0, 600.0)
            .shell(Empty);
        assert_eq!(shell.size, (800.0, 600.0));
        assert_eq!(shell.max_size, Some((800.0, 600.0)));

        // Builder order is irrelevant: the clamp happens once, at `shell`.
        let shell = app("t")
            .min_size(640.0, 480.0)
            .size(320.0, 240.0)
            .shell(Empty);
        assert_eq!(shell.size, (640.0, 480.0));
    }
}
