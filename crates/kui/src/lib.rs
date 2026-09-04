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
#[cfg(target_os = "windows")]
mod windows_nc;

use winit::application::ApplicationHandler;
use winit::dpi::LogicalSize;
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
        extensions: Vec::new(),
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
    extensions: Vec<Box<dyn Extension>>,
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

    pub fn extension(mut self, ext: impl Extension + 'static) -> Self {
        self.extensions.push(Box::new(ext));
        self
    }

    pub fn extensions(mut self, exts: Vec<Box<dyn Extension>>) -> Self {
        self.extensions.extend(exts);
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
            proxy: None,
        }
    }

    pub fn run<A: App>(self, app: A) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = EventLoop::<access_bridge::UserEvent>::with_user_event().build()?;
        event_loop.set_control_flow(ControlFlow::Wait);
        let mut shell = self.shell(app);
        shell.proxy = Some(event_loop.create_proxy());
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

/// The core's derived pointer shape in winit's vocabulary. One-to-one:
/// `CursorShape` is spelled after the platform names on purpose.
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
    /// The platform accessibility bridge.
    access: Option<access_bridge::Bridge>,
    /// Windows: answers WM_NCHITTEST from the frame's chrome regions, which
    /// enables snap layouts + native caption behavior over drawn controls.
    #[cfg(target_os = "windows")]
    nc: Option<windows_nc::NcHitTest>,
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
    /// technology is attached and the tree changed; nothing otherwise.
    fn publish_access(&mut self) {
        let Some(bridge) = self.access.as_mut() else {
            return;
        };
        if !bridge.active() {
            return;
        }
        let scale = self.window.scale_factor() as f32;
        bridge.publish(self.core.access_tree(), scale);
    }

    fn mods(&self) -> Mods {
        Mods {
            shift: self.modifiers.shift_key(),
            word: self.modifiers.alt_key(),
            doc: if cfg!(target_os = "macos") {
                self.modifiers.super_key()
            } else {
                self.modifiers.control_key()
            },
        }
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
    extensions: Vec<Box<dyn Extension>>,
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
                WindowCommand::Open { id, config, .. } => {
                    if self.pane_of(id).is_none() {
                        self.open_pane(event_loop, id, config);
                    }
                }
            }
        }
    }

    /// Opens the window a frame declared, on the shared session and device.
    fn open_pane(&mut self, event_loop: &ActiveEventLoop, id: WindowId, config: WindowConfig) {
        let size = if config.size.w > 0.0 && config.size.h > 0.0 {
            config.size
        } else {
            WindowConfig::DEFAULT_SIZE
        };
        // Untitled until its first frame's `window_title` lands (ADR 0004
        // decision 5): the declaration carries no string.
        let attrs = self
            .window_attrs("", (size.w as f64, size.h as f64))
            .with_active(config.activates);
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
        self.push_pane(event_loop, id, core, window, renderer);
    }

    /// Finishes a window whose surface and renderer exist: the platform
    /// hooks, the pane, and showing it.
    fn push_pane(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WindowId,
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
            Chrome::Borderless => attrs = attrs.with_decorations(false),
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
        self.panes[i].core.window_closed(id);
        let events = self.panes[i].core.take_pending_events();
        let cmds = self.panes[i].core.take_window_commands();
        self.panes.remove(i);
        self.route_events(events);
        self.apply_commands(event_loop, cmds);
        for p in &self.panes {
            p.window.request_redraw();
        }
    }

    /// Hands the session's queued audio commands to the device.
    fn apply_audio(&mut self) {
        let core = self.core_mut();
        let cmds = core.take_audio_commands();
        if !cmds.is_empty() {
            let resources = core.resources.clone();
            self.audio.apply(cmds, &resources);
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
        for ev in events {
            if ev.origin == OriginId::HOST {
                self.app.on_event(ev);
            } else if let Some(ext) = self.extensions.get_mut(ev.origin.0 as usize - 1) {
                ext.on_event(&ev);
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
        // the key-focused sink (`NodeSpec::on_key`); the core drops it when
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
        let (code, ktext) = match &logical {
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
        if code != KeyCode::Unknown {
            let kp = KeyPress {
                code,
                mods: kmods,
                text: ktext,
                repeat: event.repeat,
            };
            self.dispatch(
                event_loop,
                i,
                if pressed {
                    InputEvent::KeyDown(kp)
                } else {
                    InputEvent::KeyUp(kp.released())
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

        let named = match &event.logical_key {
            WinitKey::Named(n) => Some(match n {
                NamedKey::ArrowLeft => EditKey::Left,
                NamedKey::ArrowRight => EditKey::Right,
                NamedKey::ArrowUp => EditKey::Up,
                NamedKey::ArrowDown => EditKey::Down,
                NamedKey::Home => EditKey::Home,
                NamedKey::End => EditKey::End,
                NamedKey::PageUp => EditKey::PageUp,
                NamedKey::PageDown => EditKey::PageDown,
                NamedKey::Backspace => EditKey::Backspace,
                NamedKey::Delete => EditKey::Delete,
                NamedKey::Enter => EditKey::Enter,
                NamedKey::Tab => EditKey::Tab,
                NamedKey::Escape => EditKey::Escape,
                NamedKey::Space => {
                    self.dispatch(event_loop, i, InputEvent::Text(" ".to_string()));
                    return;
                }
                _ => return,
            }),
            _ => None,
        };
        if let Some(key) = named {
            let mods = self.panes[i].mods();
            self.dispatch(event_loop, i, InputEvent::Key(key, mods));
            return;
        }
        // Plain typed text (IME commits arrive via WindowEvent::Ime).
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
        let mut ui = pane.core.frame(viewport, scale);
        app.view(&mut ui);
        for (i, ext) in extensions.iter_mut().enumerate() {
            ui.set_origin(OriginId(i as u16 + 1));
            if let Err(err) = ext.view(&mut ui) {
                eprintln!("kui: extension '{}' view error: {err}", ext.name());
                ui.text(
                    &format!("[{}] {err}", ext.name()),
                    TextStyle::new(13.0).color(Color::rgb8(0xe8, 0x5d, 0x5d)),
                );
            }
        }
        ui.set_origin(OriginId::HOST);
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
            Ok(report) => wait_ms = report.vsync_wait_ms,
            Err(kui_wgpu::RenderError::Reconfigure) => {
                pane.renderer.resize(size.width, size.height);
                window.request_redraw();
            }
            // Occluded or timed out: nothing to present, try next frame.
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
        self.push_pane(event_loop, WindowId::MAIN, core, window, renderer);
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
                self.dispatch(event_loop, i, InputEvent::CursorMoved(p));
            }
            WindowEvent::CursorLeft { .. } => self.dispatch(event_loop, i, InputEvent::CursorLeft),
            WindowEvent::Focused(focused) => {
                let pane = &mut self.panes[i];
                pane.core.env.focused = focused;
                if !focused {
                    // The OS stops sending key events to a window that
                    // lost the keyboard, so the release of anything held
                    // over a Cmd-Tab would never arrive. Let go now; the
                    // synthetic `up`s route out with the pending events.
                    pane.core.release_held_keys();
                }
                pane.window.request_redraw();
            }
            WindowEvent::ModifiersChanged(m) => {
                self.panes[i].modifiers = m.state();
                let kmods = self.panes[i].kmods();
                self.dispatch(event_loop, i, InputEvent::Modifiers(kmods));
            }
            WindowEvent::KeyboardInput { event, .. } => self.on_key(event_loop, i, event),
            WindowEvent::Ime(Ime::Commit(text)) => {
                self.dispatch(event_loop, i, InputEvent::Text(text))
            }
            WindowEvent::Ime(Ime::Preedit(text, cursor)) => {
                self.dispatch(event_loop, i, InputEvent::Preedit(text, cursor));
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
        self.poll_audio();
        self.apply_audio();
        let now = std::time::Instant::now();
        let mut deadline: Option<std::time::Instant> = None;
        for pane in &mut self.panes {
            if pane.core.animating() {
                pane.window.request_redraw();
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
