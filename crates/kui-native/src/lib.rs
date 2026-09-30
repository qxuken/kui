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
mod axis_lock;
mod clipboard;
mod dialogs;
mod icon;
/// ADR 0009's arithmetic: where a pointer in one window is in another.
mod keys;
/// The traffic lights' keep-out and the OS titlebar's height, measured
/// (backlog W17).
#[cfg(target_os = "macos")]
mod macos_chrome;
/// Files dragged in from the Finder, with where they are (ADR 0031).
#[cfg(target_os = "macos")]
mod macos_drop;
#[cfg(target_os = "macos")]
mod macos_force;
/// A non-activating window that refuses to become key (the popup flick).
#[cfg(target_os = "macos")]
mod macos_key;
/// The platform's own context menu, where there is one (ADR 0017 step 3).
#[cfg(target_os = "macos")]
mod macos_menu;
/// The palette's and dictation's inserts, which winit's view drops (W15).
#[cfg(target_os = "macos")]
mod macos_text_input;
mod menus;
mod pacer;
mod pane;
mod popups;
mod retarget;
mod retry;
mod scroll_gesture;
mod secure_input;
/// A headless driver for an `App` (backlog DX11).
pub mod testing;
mod windows;

use pane::{
    Pane, appearance_of, level_change, level_supported, option_as_alt_change, sync_env,
    theme_appearance,
};
/// The OS settings winit has no call for, asked once and re-asked when the
/// user has evidently been in a settings app.
mod system_env;
#[cfg(target_os = "windows")]
mod windows_anim;
/// The terminal a `windows_subsystem = "windows"` app was launched from,
/// given back to it (`attach_parent`).
#[cfg(target_os = "windows")]
mod windows_console;
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
    /// [`Self::on_event`] with the core of the window the event came from
    /// (`docs/adr/0036-an-event-handler-gets-its-window.md`): what the app
    /// does about an event beyond its model — write the clipboard, ask for
    /// a paste, move focus, reveal or scroll to a row, ask for a frame —
    /// is a call on it here, as Node's `update` makes on its surface,
    /// rather than a field parked for the next `view`. The verbs land
    /// where they say: a clipboard write goes out with this turn's
    /// actions, a focus move or a reveal is seen by the next frame, which
    /// the verb asks for. Building a frame (`frame`) is the runner's, not
    /// the handler's. The default calls `on_event`, so an app that
    /// overrides only that one is unchanged.
    fn on_event_with(&mut self, ev: UiEvent, _core: &mut Core) {
        self.on_event(ev)
    }
    /// Called once, before the window opens, with the one thing the loop
    /// hands out: a [`Waker`] the app can clone into any thread. A PTY
    /// reader, a file watcher, an LSP client or a socket calls
    /// [`Waker::wake`] when it has changed what `view` will show, and the
    /// loop draws; nothing else ever wakes it, since it parks between
    /// events (backlog C21). The default keeps it: an app with no other
    /// thread has no use for one.
    fn setup(&mut self, _waker: Waker) {}
    /// Called once, when the main window is going for good — its close
    /// button, Quit from the menu or the dock, `WindowCommand::Close` on
    /// it, a pumped runner ended — before `run` returns or the process
    /// exits (backlog F74). The place to keep what the app would
    /// otherwise lose with the window: a session, a draft, a position.
    /// The frame is over by then: there is no `Ui` and nothing draws.
    /// A crash under `run` does not reach it; under a pumped runner a
    /// panic unwinding through the host drops the runner, and the drop
    /// retires it, so it does (backlog RG1) — and a `teardown` that panics
    /// there aborts. The default does nothing.
    fn teardown(&mut self) {}
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

/// Entry point: `kui_native::app("title").custom_titlebar().run(my_app)`.
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
        core: None,
        setup_core: Vec::new(),
        deferred_events: false,
        system: SystemEnv::default(),
        icon: None,
        icon_resource: None,
        frame_latency: kui_wgpu::DEFAULT_FRAME_LATENCY,
    }
}

/// Builder for the windowed runner.
/// One [`Launcher::setup_core`] step.
type CoreSetup = Box<dyn FnOnce(&mut Core)>;

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
    /// The main window's core, when the host made it ahead
    /// ([`Launcher::core`]); the launcher makes one otherwise.
    core: Option<Core>,
    /// What to do to the main core before its first frame
    /// ([`Launcher::setup_core`]).
    setup_core: Vec<CoreSetup>,
    /// Whether the core's diagnostics run (see `kui_core::diag`); None =
    /// on in debug builds, off in release.
    diagnostics: Option<bool>,
    /// Whether the host answers events after the loop has handed them over
    /// (see [`Launcher::deferred_events`]).
    deferred_events: bool,
    /// The OS settings the app pinned ([`Launcher::system`]); unknown is
    /// not pinned.
    system: SystemEnv,
    /// The windows' icon ([`Launcher::icon`]), checked when it was given.
    icon: Option<winit::window::Icon>,
    /// The executable's icon resource on Windows
    /// ([`Launcher::icon_resource`]).
    icon_resource: Option<u16>,
    /// Frames queued ahead of the one on screen
    /// ([`Launcher::frame_latency`]).
    frame_latency: u32,
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

    /// How many frames may be queued ahead of the one on screen, for every
    /// window (backlog C47). Two by default
    /// ([`kui_wgpu::DEFAULT_FRAME_LATENCY`]): every vsync gets a frame at
    /// light load, where one lost 1–6% of them on macOS. On macOS 14+ the
    /// runner starts frames that run back to back at the display's vsync
    /// (`mod pacer`), so the second queued frame is slack and costs no
    /// latency; where it cannot — Linux, a pumped runner — such a frame
    /// reaches the screen a vsync later than with one. One on Windows,
    /// where one already delivered every vsync (RG46). `KUI_FRAME_LATENCY`
    /// overrides it, and `KUI_FRAME_PACING=0` turns the pacing off, for
    /// comparing without a rebuild. Values below one are one.
    pub fn frame_latency(mut self, frames: u32) -> Self {
        self.frame_latency = frames.max(1);
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

    /// Opens the app inside the core's devtools panel
    /// (`docs/adr/0024-the-devtools-are-the-cores.md`): the event stream,
    /// the facts and the tree, docked beside the app's own tree.
    /// `KUI_DEVTOOLS=1` in the environment is the same ask for an app
    /// that never made it. Sugar for `setup_core(|c| c.set_devtools(on))`.
    pub fn devtools(self, on: bool) -> Self {
        self.setup_core(move |core| core.set_devtools(on))
    }

    /// Respells the chord that moves the keyboard into the devtools panel
    /// and back out — and brings a hidden panel back — from its default
    /// `Ctrl+Shift+I`: `Accel::parse("f12")`, `"mod+shift+d"`, any
    /// spelling a menu item takes. The panel's other chords stay
    /// `Ctrl+Shift+<letter>`; with another chord set, `Ctrl+Shift+I` is
    /// the app's again. Sugar for `setup_core(|c| c.set_devtools_key(key))`.
    pub fn devtools_key(self, key: Accel) -> Self {
        self.setup_core(move |core| core.set_devtools_key(key))
    }

    /// Runs `f` on the main window's core before its first frame — the
    /// place for what a core is *told* rather than declared: the devtools
    /// doors, a pinned theme, `set_native_menus`. Every call adds one;
    /// they run in order.
    pub fn setup_core(mut self, f: impl FnOnce(&mut Core) + 'static) -> Self {
        self.setup_core.push(Box::new(f));
        self
    }

    /// Opens the main window on `core` rather than on one the launcher
    /// makes: everything the host registered on it beforehand — fonts,
    /// images, sounds, tokens, a pinned theme, the devtools doors,
    /// `set_native_menus`, the text-cache budget — reaches the window,
    /// and the core's session is the app's, so a declared second window
    /// joins it and the handles a headless frame minted keep drawing. What
    /// the launcher is told still applies on top, in the order it always
    /// has: [`Launcher::diagnostics`] (or the build's default), then
    /// `KUI_DEVTOOLS`, then every [`Launcher::setup_core`]. A C host
    /// registers on a context and hands it to `kui_run_with`, which is
    /// this door (backlog AR27); a Rust host that built a core to draw
    /// headless first has it too.
    pub fn core(mut self, core: Core) -> Self {
        self.core = Some(core);
        self
    }

    /// Says that this app answers an event *after* `on_event` returns —
    /// which only a host driving the loop itself can do, since only it has
    /// a turn between pumps ([`Launcher::open`], [`PumpRunner`]). Node's
    /// `update` is the case: `on_event` keeps the event, the pump returns,
    /// and JS runs the handler and submits the next view.
    ///
    /// What it changes is one thing: an input whose events reached the app
    /// **does not ask for the frame itself**. Ordinarily it does, and for
    /// an app that answered inside `on_event` that frame is right — it
    /// shows the button let go *and* what letting go did. For one that has
    /// not answered yet the same frame shows the button let go and the
    /// count still at its old value, with the new one a pump later: a
    /// two-frame release, plain to see at an 8 ms pump. Declining to ask
    /// leaves the frame to the host, which asks
    /// ([`PumpRunner::request_redraw`], and every `setView` and drained
    /// `pollEvents` does) once its handler has run, so the release and its
    /// answer land in one frame.
    ///
    /// Nothing else is suppressed. A transition, a caret blink, a
    /// first-frame retry, a `Waker` wake or the OS's own repaint still
    /// paint whenever they ask, including during a platform's modal
    /// move-resize loop, and an input that reached nobody still asks for
    /// its own frame — so the worst this can cost is that a frame some
    /// *other* subsystem asked for in the same pump shows the input's
    /// answer one frame late.
    pub fn deferred_events(mut self) -> Self {
        self.deferred_events = true;
        self
    }

    /// Pins part of `env.system` for this app's windows: every field of
    /// `pinned` that is not "cannot tell" is what the views read, over
    /// whatever the OS says, for as long as the app runs; the fields left
    /// at their default keep following the OS, and a change to one of
    /// those still arrives as the `system` event, carrying the pin with it.
    ///
    /// ```no_run
    /// # use kui_native::{SystemEnv, MotionPref};
    /// kui_native::app("mine").system(SystemEnv { motion: MotionPref::Reduced, ..Default::default() });
    /// ```
    ///
    /// For looking at the window a user who asked for less motion, or a
    /// dark appearance, would get — on a machine whose owner asked for
    /// neither. The headless core takes the same reading through
    /// `core.env.system` and needs none of this; a window cannot, because
    /// its runner writes the real reading before every frame, which is
    /// why there is no `set_env` on one and this is on the launcher
    /// instead: the app asking in its own code, the same place
    /// `KUI_SMOKE_FRAMES` was kept out of a shipped build for — an app you
    /// ship should not change its motion because of a variable in the
    /// environment it was launched from (backlog F47).
    pub fn system(mut self, pinned: SystemEnv) -> Self {
        self.system = pinned;
        self
    }

    /// The icon every window of the app is created with: `rgba` is
    /// `width` × `height` pixels, four bytes each, row by row from the top
    /// left, alpha not premultiplied. Windows
    /// shows it in the title bar, Alt-Tab and the taskbar and X11 in the
    /// window manager's; macOS draws the bundle's `.icns` in the Dock and
    /// Wayland the `.desktop` file's icon, and neither has a window icon,
    /// so there it is nothing. Something a taskbar can shrink cleanly —
    /// 64 to 256 px. Panics when the pixels are not that size, a
    /// programming error at startup; [`Launcher::try_icon`] says why
    /// instead.
    ///
    /// ```no_run
    /// # let rgba = vec![0u8; 64 * 64 * 4];
    /// kui_native::app("mine").icon(rgba, 64, 64);
    /// ```
    ///
    /// A Windows program's own icon is a resource linked into its
    /// executable, where Explorer finds it — and winit does not give it to
    /// the windows; [`Launcher::icon_resource`] does, and wins over the
    /// pixels there.
    pub fn icon(self, rgba: Vec<u8>, width: u32, height: u32) -> Self {
        match self.try_icon(rgba, width, height) {
            Ok(this) => this,
            Err(e) => panic!("kui: {e}"),
        }
    }

    /// [`Launcher::icon`] for pixels that came from outside the program —
    /// Node's `icon` option, C's `kui_set_icon` — refused with the reason
    /// rather than a panic. The launcher is consumed either way, as
    /// [`Launcher::try_extension_as`]'s is.
    pub fn try_icon(mut self, rgba: Vec<u8>, width: u32, height: u32) -> Result<Self, String> {
        self.icon = Some(icon::from_rgba(rgba, width, height)?);
        Ok(self)
    }

    /// The executable's icon resource `id` as every window's icon, on
    /// Windows: the `.ico` a `1 ICON "app.ico"` line in the program's `.rc`
    /// links in, the one Explorer already draws for the file — each of the
    /// title bar and the taskbar loads the frame drawn for its own size.
    /// A resource the executable does not have is said once on stderr, and
    /// [`Launcher::icon`]'s pixels are used if there are any. Nothing on
    /// other platforms, so an app passes both and each OS takes its own.
    pub fn icon_resource(mut self, id: u16) -> Self {
        self.icon_resource = Some(id);
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
    pub fn extension_as(self, namespace: impl Into<String>, ext: impl Extension + 'static) -> Self {
        match self.try_extension_as(namespace, ext) {
            Ok(this) => this,
            Err(e) => panic!("kui: {e}"),
        }
    }

    /// `extension_as` for a caller that has to report the refusal rather
    /// than die of it — a plugin path that came from outside the program,
    /// which is Node's `extensions` option. The launcher is consumed
    /// either way: a host that cannot load the extension it was told to
    /// load has nothing useful left to run.
    pub fn try_extension_as(
        mut self,
        namespace: impl Into<String>,
        ext: impl Extension + 'static,
    ) -> Result<Self, String> {
        self.extensions.push_as(namespace, Box::new(ext))?;
        Ok(self)
    }

    /// A list already loaded, replacing any `extension` calls before it —
    /// what a C host built into a context with `kui_ctx_add_extension`
    /// and hands to `kui_run_with`, so that the one loader and its error
    /// channel serve the window too.
    pub fn with_extensions(mut self, extensions: Extensions) -> Self {
        self.extensions = extensions;
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

    /// The shell for `app`, boxed: the one generic step between an app
    /// and the runner, kept to moving fields (C49). Everything after it
    /// takes the box unsized, as a [`DynShell`].
    fn shell<A: App>(mut self, app: A) -> Box<Shell<A>> {
        let (diagnostics, session, core) = self.main_core();
        Box::new(Shell {
            title: self.title,
            icon: icon::AppIcon::new(self.icon, self.icon_resource),
            chrome: self.chrome,
            size: clamp_size(self.size, self.min_size, self.max_size),
            min_size: self.min_size,
            max_size: self.max_size,
            text_aa: self.text_aa,
            frame_latency: wanted_frame_latency(self.frame_latency),
            diagnostics,
            subpixel: false,
            extensions: self.extensions,
            session,
            main_core: Some(core),
            panes: Vec::new(),
            gpu: None,
            reopened: None,
            reopen_owed: false,
            pretended_loss: false,
            locks: kui_core::KeyLocks::default(),
            epoch: std::time::Instant::now(),
            system: system_env::query(),
            pinned_system: self.system,
            clipboard: arboard::Clipboard::new().ok(),
            #[cfg(target_os = "macos")]
            native_menu: macos_menu::MacMenu::new(),
            #[cfg(target_os = "macos")]
            menu_shown: false,
            #[cfg(target_os = "macos")]
            native_menu_bar: macos_menu::MacMenuBar::new(),
            #[cfg(target_os = "macos")]
            applied_menu_bar: None,
            audio: audio::Audio::new(),
            audio_touch: std::time::Instant::now(),
            smoke_frames: Self::smoke_frames(),
            frames_drawn: 0,
            exit_requested: false,
            startup_error: None,
            torn_down: false,
            secure_input: secure_input::SecureInput::default(),
            pumped: false,
            opened: false,
            primary_down: None,
            armed: Vec::new(),
            swallowed_press: None,
            proxy: None,
            next_deadline: None,
            saw_event: false,
            woke: false,
            deferred_events: self.deferred_events,
            owed: std::cell::Cell::new(false),
            app,
        })
    }

    /// A shell as the runner sees it, for the tests.
    #[cfg(test)]
    fn dyn_shell<A: App + 'static>(self, app: A) -> Box<DynShell<'static>> {
        self.shell(app)
    }

    /// Whether diagnostics are on, the session, and the main window's core
    /// with the launcher's setup applied — the part of `shell` that does
    /// not need the app's type.
    fn main_core(&mut self) -> (bool, Session, Core) {
        // Before anything prints: a windows-subsystem app started from a
        // shell has no stdout until it takes its parent's, and the
        // diagnostics, the panics and `report_faults` are all worth
        // reading there (`mod windows_console`).
        #[cfg(target_os = "windows")]
        windows_console::attach_parent();
        // Diagnostics are a development aid: on in debug builds unless the
        // launcher says otherwise, so a shipped app pays and prints nothing.
        let diagnostics = self.diagnostics.unwrap_or(cfg!(debug_assertions));
        // A handed core brings its session; a made one gets a fresh one.
        let (session, mut core) = match self.core.take() {
            Some(core) => (core.session().clone(), core),
            None => {
                let session = Session::new();
                let core = Core::new_in(&session);
                (session, core)
            }
        };
        core.set_diagnostics(diagnostics);
        // `KUI_DEVTOOLS=1` opens the panel for a program that never asked
        // (ADR 0024); read here, for a window, and never by a headless
        // core. What the launcher was told comes after, and wins.
        core.devtools_from_env();
        for f in std::mem::take(&mut self.setup_core) {
            f(&mut core);
        }
        (diagnostics, session, core)
    }

    /// `KUI_SMOKE_FRAMES=n`, in a build that honours it.
    ///
    /// A development aid, gated the way `shell` gates the diagnostics: an app you
    /// ship should not close its own window because something in the
    /// environment it was launched from happened to set a variable, and
    /// the app's author never asked for that behaviour. Live in a dev
    /// build, which is where it is used by hand (`KUI_SMOKE_FRAMES=120
    /// cargo run --example fragment`), and in a release build that asks
    /// for it with `--features smoke` — which is what
    /// the smoke round passes under `--release`
    /// (examples/devtools/src/bin/smoke.rs), and
    /// the whole reason this is a feature rather than `debug_assertions`
    /// alone: the round is worth running against what actually ships.
    fn smoke_frames() -> Option<u32> {
        if !(cfg!(debug_assertions) || cfg!(feature = "smoke")) {
            return None;
        }
        std::env::var("KUI_SMOKE_FRAMES")
            .ok()
            .and_then(|s| s.parse().ok())
    }

    pub fn run<A: App>(self, app: A) -> Result<(), Box<dyn std::error::Error>> {
        let event_loop = run_loop()?;
        run_shell(event_loop, self.shell(app))
    }

    /// Opens the window but keeps the event loop in the caller's hands: the
    /// returned [`PumpRunner`] processes OS events only when [`PumpRunner::pump`]
    /// is called, so a foreign loop (Node/libuv, a game loop, a test harness)
    /// can interleave with winit on the main thread. One event loop per
    /// process — winit event loops are not recreatable on any desktop
    /// platform — but any number of windows on it, and any number of
    /// runners *in turn*: a runner whose main window has closed parks the
    /// loop, and the next `open` on the thread takes it back (backlog
    /// F58), so a process can open a window, close it, and open another.
    pub fn open<A: App>(self, app: A) -> Result<PumpRunner<A>, Box<dyn std::error::Error>> {
        let event_loop = take_event_loop()?;
        let mut shell = self.shell(app);
        let state = PumpState::open(event_loop, &mut *shell);
        let mut runner = PumpRunner {
            state,
            shell: std::mem::ManuallyDrop::new(shell),
        };
        if !runner.state.alive {
            runner.retire();
        }
        // Retired above, so the loop is parked and the next `open` can
        // try again (backlog RG47).
        if let Some(why) = runner.shell.startup_error.take() {
            return Err(why.into());
        }
        Ok(runner)
    }
}

/// The event loop `run` owns. Not a parked loop: a pumped runner leaves
/// winit's loop *running* (it never exits it — see `PARKED_LOOP`), and
/// `run_app` on a running loop is a `debug_assert` in winit's macOS path.
/// `run` is the one-shot runner; a process that has pumped opens again.
fn run_loop() -> Result<EventLoop<access_bridge::UserEvent>, Box<dyn std::error::Error>> {
    if PARKED_LOOP.with(|p| p.borrow().is_some()) {
        return Err(
            "kui: `run` cannot follow a pumped runner in this process — a loop \
                    a `PumpRunner` parked is still running; open another `PumpRunner`"
                .into(),
        );
    }
    Ok(EventLoop::<access_bridge::UserEvent>::with_user_event().build()?)
}

/// `Launcher::run` past the one generic step: compiled here, once.
fn run_shell(
    event_loop: EventLoop<access_bridge::UserEvent>,
    mut shell: Box<DynShell<'_>>,
) -> Result<(), Box<dyn std::error::Error>> {
    event_loop.set_control_flow(ControlFlow::Wait);
    shell.attach(&event_loop);
    event_loop.run_app(&mut Handler(&mut shell))?;
    match shell.startup_error.take() {
        Some(why) => Err(why.into()),
        None => Ok(()),
    }
}

impl DynShell<'_> {
    /// The main window, or its renderer, could not be made: `why` becomes
    /// the error `run` or `open` returns, and the runner ends. `run`'s
    /// loop is asked to exit; a pumped one is not — it is parked for the
    /// next runner, as when a window closes (backlog RG47).
    fn fail_open(&mut self, event_loop: &ActiveEventLoop, why: String) {
        self.startup_error = Some(why);
        self.exit_requested = true;
        if !self.pumped {
            event_loop.exit();
        }
    }

    /// What a shell takes from the loop it runs on before the first
    /// event: the proxy, the app's waker, and the wakers of the platform
    /// paths no winit event carries.
    fn attach(&mut self, event_loop: &EventLoop<access_bridge::UserEvent>) {
        self.proxy = Some(event_loop.create_proxy());
        self.app.setup(Waker(event_loop.create_proxy()));
        // A menu-bar item chosen by its ⌘-shortcut is the whole of the
        // event as far as winit is concerned — AppKit consumed the key —
        // so the bar rings the loop itself.
        #[cfg(target_os = "macos")]
        if let Some(bar) = &self.native_menu_bar {
            bar.set_waker(Waker(event_loop.create_proxy()));
        }
        // And so does an insert the platform makes from the palette or
        // dictation: no winit event carries it.
        #[cfg(target_os = "macos")]
        macos_text_input::set_waker(Waker(event_loop.create_proxy()));
        // And a file drag's position (ADR 0031), which winit's do not.
        #[cfg(target_os = "macos")]
        macos_drop::set_waker(Waker(event_loop.create_proxy()));
    }
}

thread_local! {
    /// The process's one event loop, parked between runners. winit refuses
    /// to build a second (`EventLoopError::RecreationAttempt`, a static
    /// flag it never clears), so a host that opens a window, closes it
    /// and opens another — a smoke test running two configurations in
    /// sequence, an app whose second launch is in-process — needs the
    /// first runner to hand the loop back rather than drop it. The pump
    /// path never calls winit's `exit()` for the same reason: an exited
    /// loop answers every later pump with `Exit` and nothing public clears
    /// that; the runner ends itself on `exit_requested` instead, and the
    /// loop stays live for the next shell (backlog F58).
    static PARKED_LOOP: std::cell::RefCell<Option<EventLoop<access_bridge::UserEvent>>> =
        const { std::cell::RefCell::new(None) };
}

/// The parked loop if an earlier runner left one, else a new one — which
/// winit allows once per process. For `open` only: `run` builds its own
/// (above), since a parked loop is a running one.
fn take_event_loop() -> Result<EventLoop<access_bridge::UserEvent>, winit::error::EventLoopError> {
    if let Some(parked) = PARKED_LOOP.with(|p| p.borrow_mut().take()) {
        return Ok(parked);
    }
    EventLoop::<access_bridge::UserEvent>::with_user_event().build()
}

/// Whether this input is one whose own visible effect is finished, so the
/// app's answer to it belongs in the same frame.
///
/// A press or a release changes what the core itself paints — the button
/// goes down, the button comes up — and that change reads as the whole of
/// what happened, so a frame showing the button let go with the count
/// unchanged is a frame that lies. A keystroke, an IME commit and an
/// assistive-technology action are discrete the same way.
///
/// A pointer moving, a wheel turning and a preedit being revised are not:
/// they arrive as a stream, every frame during one is superseded by the
/// next, and an app's content trailing the pointer by a frame is what
/// every toolkit does. Waiting on those would halve the frame rate of a
/// drag for nothing — measured at 18 frames against 36 in a 578 ms drag —
/// so they never wait.
/// Puts a copy on the system clipboard, with the formatting beside the
/// words where there is any (ADR 0017, decision 7).
///
/// Both flavours or neither: `set_html` writes the HTML *and* the plain
/// text it is given as an alternative, so an app that understands one
/// takes it and everything else takes the words. A clipboard holding only
/// HTML pastes markup into every plain-text field on the machine, which is
/// the failure mode this shape exists to avoid.
fn set_clipboard(clipboard: Option<&mut arboard::Clipboard>, text: String, html: Option<String>) {
    let Some(cb) = clipboard else { return };
    match html {
        Some(html) => {
            let _ = cb.set_html(html, Some(text));
        }
        None => {
            let _ = cb.set_text(text);
        }
    }
}

fn input_completes(ev: &InputEvent) -> bool {
    match ev {
        InputEvent::MouseDown { .. }
        | InputEvent::MouseUp { .. }
        // A force click is one moment and one answer, like a press.
        | InputEvent::ForceClick(_)
        | InputEvent::Key(..)
        | InputEvent::KeyDown(_)
        | InputEvent::KeyUp(_)
        | InputEvent::Text(_)
        | InputEvent::Commit(_)
        | InputEvent::Paste { .. }
        | InputEvent::Access(_)
        // A drop is a release; a cancel ends the drag the same way.
        | InputEvent::DropFiles { .. }
        | InputEvent::DragCancel
        // A dialog's answer is one moment, as a paste is.
        | InputEvent::Files(_) => true,
        InputEvent::CursorMoved(_)
        | InputEvent::CursorLeft
        | InputEvent::Scroll(_)
        | InputEvent::ScrollGesture { .. }
        | InputEvent::Preedit(..)
        | InputEvent::Modifiers(_)
        // Files moving over the window is the pointer moving.
        | InputEvent::DragFiles { .. } => false,
    }
}

/// Whether a redraw waits for the host's answer instead of painting now.
///
/// Two conditions, and the second is the one that makes this safe. `owed`
/// says an input reached an app that answers later, so what would be
/// painted predates that input ([`Launcher::deferred_events`]).
/// `deferred_last` says this window's previous redraw already waited — and
/// a frame never waits twice running.
///
/// That bound is not a nicety. Waiting until the host says otherwise is
/// the obvious rule and it starves the window: winit hands a pump its
/// input before that pump's redraw, so under a stream of input that
/// reaches the app — a drag, an auto-repeating key — each pump re-arms the
/// wait before the frame the host just asked for is delivered, and the
/// window paints **nothing** until the stream ends (measured: one frame in
/// a 578 ms drag). The same unbounded rule freezes a window for a whole
/// title-bar drag, since a platform's modal move loop never returns to the
/// host that would end the wait. Never twice running costs at worst half
/// the frame rate under continuous input, and bounds every one of those to
/// a single frame.
fn frame_waits_for_host(owed: bool, deferred_last: bool) -> bool {
    owed && !deferred_last
}

fn pump_once(
    event_loop: &mut EventLoop<access_bridge::UserEvent>,
    shell: &mut DynShell<'_>,
) -> bool {
    use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
    match event_loop.pump_app_events(Some(std::time::Duration::ZERO), &mut Handler(shell)) {
        PumpStatus::Continue => !shell.exit_requested,
        PumpStatus::Exit(_) => false,
    }
}

/// A windowed runner driven from outside: same [`Shell`] as [`Launcher::run`]
/// (input mapping, IME, clipboard, chrome, caret blink), but the host calls
/// [`pump`](Self::pump) on its own cadence instead of parking in `run_app`.
///
/// Typed by its app for [`app_mut`](Self::app_mut) and
/// [`route_events`](Self::route_events) alone: every method hands the
/// shell on unsized, so the work is compiled once in kui rather than in
/// every crate that opens one (backlog C49).
pub struct PumpRunner<A: App> {
    state: PumpState,
    /// Dropped by hand, unsized (`drop_shell`): as a plain field its drop
    /// glue — every window, core and store the shell owns — was generated
    /// in the app's crate.
    shell: std::mem::ManuallyDrop<Box<Shell<A>>>,
}

/// What a [`PumpRunner`] keeps beside its shell, and does to it, without
/// its app's type.
struct PumpState {
    /// `None` once the runner has retired: the loop is parked for the next
    /// runner on this thread (see `PARKED_LOOP`).
    event_loop: Option<EventLoop<access_bridge::UserEvent>>,
    alive: bool,
    /// Every turn this runner has taken — [`pump`](PumpRunner::pump) and
    /// [`pump_until`](PumpRunner::pump_until) alike, the first one that
    /// opened the window included. What a driver's backoff is measured in
    /// (backlog F62): the runner knows how often it pumped where the app
    /// could only read a process monitor.
    pumps: u64,
    /// The turns among `pumps` whose batch carried an OS event or a wake
    /// (backlog F94): what `saw_event` marks, counted once per turn.
    woken_pumps: u64,
}

impl PumpState {
    fn open(
        mut event_loop: EventLoop<access_bridge::UserEvent>,
        shell: &mut DynShell<'_>,
    ) -> PumpState {
        event_loop.set_control_flow(ControlFlow::Wait);
        shell.pumped = true;
        shell.attach(&event_loop);
        // First pump delivers `resumed`, creating the window + renderer —
        // or, on a loop taken back from an earlier runner, `about_to_wait`
        // does, since winit's init events came and went with the first.
        let alive = pump_once(&mut event_loop, shell);
        let mut state = PumpState {
            event_loop: Some(event_loop),
            alive,
            pumps: 1,
            woken_pumps: 0,
        };
        state.tally(shell);
        state
    }

    /// Counts the turn that just ran as woken if its batch saw an event.
    /// The flag is taken, so the quiet turns after it are not counted too.
    fn tally(&mut self, shell: &mut DynShell<'_>) {
        if std::mem::take(&mut shell.woke) {
            self.woken_pumps += 1;
        }
    }

    fn pump(&mut self, shell: &mut DynShell<'_>) -> bool {
        let Some(event_loop) = &mut self.event_loop else {
            return false;
        };
        self.pumps += 1;
        self.alive = pump_once(event_loop, shell);
        self.tally(shell);
        if !self.alive {
            self.retire(shell);
        }
        self.alive
    }

    /// The end of this runner: every window closed (dropping the panes
    /// is what closes them — the pump path never asks winit to exit) and
    /// the loop handed back for the next `Launcher::open` on the thread.
    fn retire(&mut self, shell: &mut DynShell<'_>) {
        self.alive = false;
        shell.teardown_once();
        // The main core outlives its window, as it predated it: a host
        // still reads events, warnings and the tree off a runner that
        // has ended (`core_mut`), and the Node driver does so for the
        // pump that returned false.
        let mut panes = std::mem::take(&mut shell.panes);
        // The facts the platform's text input reads are keyed by the
        // view's address, which the next window's view may get.
        #[cfg(target_os = "macos")]
        for pane in &panes {
            macos_text_input::detach(&pane.window);
            macos_drop::detach(&pane.window);
        }
        if !panes.is_empty() {
            shell.main_core = Some(panes.remove(0).core);
        }
        if let Some(event_loop) = self.event_loop.take() {
            PARKED_LOOP.with(|p| *p.borrow_mut() = Some(event_loop));
        }
    }

    fn pump_until(&mut self, shell: &mut DynShell<'_>, deadline: std::time::Instant) -> bool {
        use winit::platform::pump_events::{EventLoopExtPumpEvents, PumpStatus};
        let Some(event_loop) = &mut self.event_loop else {
            return false;
        };
        let timeout = deadline.saturating_duration_since(std::time::Instant::now());
        self.pumps += 1;
        self.alive = match event_loop.pump_app_events(Some(timeout), &mut Handler(shell)) {
            PumpStatus::Continue => !shell.exit_requested,
            PumpStatus::Exit(_) => false,
        };
        self.tally(shell);
        if !self.alive {
            self.retire(shell);
        }
        self.alive
    }

    fn waker(&self, shell: &DynShell<'_>) -> Waker {
        match &self.event_loop {
            Some(event_loop) => Waker(event_loop.create_proxy()),
            // Retired: the proxy the shell kept still names the loop.
            None => Waker(shell.proxy.clone().expect("a pump runner keeps its proxy")),
        }
    }
}

impl DynShell<'_> {
    fn core_mut_of(&mut self, id: WindowId) -> Option<&mut Core> {
        if id == WindowId::MAIN {
            return Some(self.core_mut());
        }
        self.panes
            .iter_mut()
            .find(|p| p.id == id)
            .map(|p| &mut p.core)
    }

    fn window_id(&mut self, name: &str) -> Option<WindowId> {
        self.core_mut()
            .windows()
            .into_iter()
            .find(|(_, n)| &**n == name)
            .map(|(id, _)| id)
    }

    fn request_redraw(&self) {
        self.owed.set(false);
        for p in &self.panes {
            p.redraw_for(FrameCause::HOST);
        }
    }
}

impl<A: App> Drop for PumpRunner<A> {
    /// A runner dropped while alive — a host that let go of it without
    /// pumping to the end — parks the loop too, so the next `open` on the
    /// thread is not refused for its sake.
    fn drop(&mut self) {
        // `retire` reaches `App::teardown` too, so a runner a panic unwinds
        // through hears the window go (backlog RG1); a second panic out of
        // that `teardown` is an abort, as any panic in a drop is.
        self.retire();
        // SAFETY: taken once, here, and the field is not read again.
        let shell: Box<Shell<A>> = unsafe { std::mem::ManuallyDrop::take(&mut self.shell) };
        drop_shell(shell);
    }
}

/// Drops a shell unsized, so its drop glue is kui's (see `PumpRunner`).
fn drop_shell(shell: Box<DynShell<'_>>) {
    drop(shell);
}

impl<A: App> PumpRunner<A> {
    fn shell(&self) -> &DynShell<'_> {
        &**self.shell
    }

    fn shell_mut(&mut self) -> &mut DynShell<'_> {
        &mut **self.shell
    }

    fn retire(&mut self) {
        self.state.retire(&mut **self.shell);
    }

    /// Processes all pending OS events without blocking. Returns false once
    /// the main window has closed — and from that pump on the windows are
    /// gone and the loop is parked for the next runner; further pumps are
    /// no-ops.
    pub fn pump(&mut self) -> bool {
        self.state.pump(&mut **self.shell)
    }

    /// How many turns this runner has taken, the one that opened the
    /// window included — every `pump` and `pump_until` that ran, not the
    /// no-ops after it retired. Monotonic, so two readings a second apart
    /// are the pump rate over that second, which is what a driver's
    /// backoff promises and what `frame_stats` cannot say (backlog F62).
    /// Which of them something outside the app caused is
    /// [`woken_pumps`](Self::woken_pumps).
    pub fn pumps(&self) -> u64 {
        self.state.pumps
    }

    /// How many of those [`pumps`](Self::pumps) found an OS event or a
    /// wake in their batch: any window event but a redraw — a key, the
    /// pointer crossing the window, a focus change, a resize, the window
    /// moved or occluded — or anything that came through the loop's
    /// proxy: a [`Waker::wake`], assistive technology asking, a file
    /// dialog's answer. It is what makes
    /// [`next_deadline`](Self::next_deadline) answer "now" after a pump,
    /// counted (backlog F94). Monotonic, and never more than `pumps`.
    ///
    /// Two readings a second apart with this one unmoved are a second the
    /// desktop left the window alone: whatever it drew was the app's own
    /// doing — a tick, a caret blink, a transition, the audio poll. A
    /// moved one is the desktop or a person reaching in, which resets a
    /// driver's backoff exactly as a regression would, so an idle-window
    /// test that sees it move re-measures instead of failing.
    pub fn woken_pumps(&self) -> u64 {
        self.state.woken_pumps
    }

    /// `pump`, but parked until an OS event, a [`Waker::wake`] or
    /// `deadline` — whichever comes first — so a host that owns the loop
    /// blocks on all three instead of polling on a timer (backlog C21).
    /// Returns false once the main window has closed.
    pub fn pump_until(&mut self, deadline: std::time::Instant) -> bool {
        self.state.pump_until(&mut **self.shell, deadline)
    }

    /// When the shell next needs pumping, as the last [`pump`](Self::pump)
    /// left it — `None` when nothing it knows about is due, which is
    /// `ControlFlow::Wait` for a loop that owns itself.
    ///
    /// A host driving from a foreign loop has to guess how long to leave
    /// between pumps, and the guess is what pays: a caret blink, a tick, a
    /// transition's next frame, the audio poll and a window's first-frame
    /// retry are all deadlines the shell has already worked out, and a
    /// driver on a fixed interval hits them a whole interval late. Ask
    /// after each pump and sleep to the answer instead.
    ///
    /// What it does *not* say is whether an OS event is waiting — nothing
    /// short of pumping can — so a driver still needs a ceiling of its own.
    /// This only ever tells it to come back sooner.
    pub fn next_deadline(&self) -> Option<std::time::Instant> {
        self.shell.next_deadline
    }

    /// A [`Waker`] for this loop, to clone into the threads the host's
    /// data arrives on.
    pub fn waker(&self) -> Waker {
        self.state.waker(&**self.shell)
    }

    pub fn app_mut(&mut self) -> &mut A {
        &mut self.shell.app
    }

    /// Delivers `events` the way this runner's own loop does
    /// (`Shell::route_events`, ADR 0014 decision 6): an extension's event
    /// to the extension, and what it replies to `to_app` carrying its
    /// origin. A host that drives the core directly — `core_mut().press`,
    /// an access action, a drained `take_pending_events` — produces events
    /// the loop never saw, and pushing those at the app would hand it a
    /// plugin's clicks and leave the plugin deaf to them.
    ///
    /// `to_app` is lent the core of the window each event came from, as
    /// the runner's own loop lends it to `App::on_event_with` (ADR 0036).
    pub fn route_events(
        &mut self,
        events: impl IntoIterator<Item = UiEvent>,
        mut to_app: impl FnMut(&mut A, UiEvent, &mut Core),
    ) {
        let Shell {
            extensions,
            app,
            panes,
            main_core,
            ..
        } = &mut **self.shell;
        extensions.route(events, |ev| {
            if let Some(core) = window_core(panes, main_core, ev.window) {
                to_app(app, ev, core);
            }
        });
    }

    /// The main window's core. Every window of the app shares its session,
    /// so resources registered through it draw in all of them, and
    /// `Core::windows` on it lists them.
    pub fn core_mut(&mut self) -> &mut Core {
        self.shell_mut().core_mut()
    }

    /// The core of the window `id` names, if that window is open — the
    /// main window's for `WindowId::MAIN`. Everything that is one
    /// window's rather than the session's — its focus, its editors' text,
    /// its scroll offsets, its tokens — is answered by this core and no
    /// other (backlog AR12).
    pub fn core_mut_of(&mut self, id: WindowId) -> Option<&mut Core> {
        self.shell_mut().core_mut_of(id)
    }

    /// The id of the open window named `name` (`"main"` for the launcher's),
    /// or `None` while no window of that name is open — before its first
    /// frame's diff, or after the user closed it.
    pub fn window_id(&mut self, name: &str) -> Option<WindowId> {
        self.shell_mut().window_id(name)
    }

    /// The main window's inner size (logical px) and its scale factor.
    /// Unlike `core_mut().viewport()` this is known before the first frame,
    /// so a host can size its model at setup — through
    /// `core_mut().host_area(size)`, which is what the next frame lays out
    /// against: the window less the devtools' dock while the panel is
    /// docked (`docs/adr/0024`), and the window itself otherwise.
    pub fn window_size(&self) -> (Size, f32) {
        self.shell().window_size()
    }

    /// Schedules a redraw of every window (call after changing what `view`
    /// will produce). Under [`Launcher::deferred_events`] it is also what
    /// ends a frame's wait: the host calling this is the host saying its
    /// view is current, so the frame that was waiting can be painted now —
    /// with the answer in it.
    pub fn request_redraw(&self) {
        self.shell().request_redraw();
    }

    /// Asks the app to close; the next `pump` observes it and returns false.
    pub fn request_exit(&mut self) {
        self.shell.exit_requested = true;
    }

    /// Hands the core's queued audio commands to the device now, rather
    /// than at the next pump — for hosts that call `Core::play` between
    /// pumps and want the sound to start at once.
    pub fn flush_audio(&mut self) {
        self.shell_mut().apply_audio();
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

/// How long the output device is held open after the last input or sound.
/// An open device is a real-time thread the OS runs ~94 times a second
/// whether or not anything is playing, which is the entire idle CPU cost of
/// an app that owns a sound — the counter example sat at 0.3% doing nothing.
/// Held rather than closed at once because the point of opening early is
/// that a click finds it open (the ~90 ms open used to stall the counter's
/// first click), and any input at all re-warms it: the pointer moving
/// towards a button is minutes of warning before the button is pressed.
const AUDIO_IDLE_CLOSE: std::time::Duration = std::time::Duration::from_secs(5);
/// How long after one try at opening a new device (`Shell::reopen_device`)
/// the next may be made. A device that will not open — a driver still
/// being installed, an adapter gone — is tried once a second, and the
/// loop sleeps until then rather than asking for frames that could only
/// ask again.
const REOPEN_INTERVAL: std::time::Duration = std::time::Duration::from_secs(1);
/// Frames in a row a surface may refuse for being configured wrong, each
/// answered by configuring it again, before it is given up with its device.
const SURFACE_TRIES: u8 = 3;

/// When a new device asked for at `now` may be opened: at once if none
/// has been tried, else `REOPEN_INTERVAL` after the last try and never
/// sooner. A time not after `now` means now; a later one is the wake the
/// loop sleeps until (`about_to_wait`), since nothing else is bound to ask
/// — an idle app's windows draw nothing on their own, and an animating
/// one's frames, with no surface to present to, are not paced by vsync
/// and would spin the loop until the second was up.
fn reopen_due(last_try: Option<std::time::Instant>, now: std::time::Instant) -> std::time::Instant {
    last_try.map_or(now, |t| (t + REOPEN_INTERVAL).max(now))
}

/// What a frame does with a surface refused for being configured wrong.
#[derive(Debug, PartialEq, Eq)]
enum Refused {
    /// Configure it to the window's size and draw again at once.
    Retry,
    /// Give it up with its device (`Shell::reopen_device`); `say` on the
    /// first frame past the tries, not on each one refused after it while
    /// the reopen waits out its second.
    GiveUp { say: bool },
}

/// Counts one more refusal in `tries` — saturating, since while a reopen
/// is owed every frame input asks for is refused again and counted, and
/// a `u8` that wrapped would start the retries over (or, in a debug
/// build, panic) — and says what the frame does with it: the first
/// `SURFACE_TRIES` are retried, the rest give the surface up.
fn surface_refused(tries: &mut u8) -> Refused {
    *tries = tries.saturating_add(1);
    if *tries <= SURFACE_TRIES {
        Refused::Retry
    } else {
        Refused::GiveUp {
            say: *tries == SURFACE_TRIES + 1,
        }
    }
}

/// Whether text is drawn with LCD subpixel masks: what was asked for
/// (`KUI_TEXT_AA` over the launcher's `text_aa`, `wanted_text_aa`), and
/// for `Auto` or `Subpixel` only where the device blends per channel —
/// a mask the blend cannot split draws coloured fringes. Decided with each
/// device, the first and every one opened after a loss, since a reopen can
/// land on another adapter.
fn subpixel_on(wanted: TextAa, dual_source: bool) -> bool {
    match wanted {
        TextAa::Grayscale => false,
        TextAa::Subpixel | TextAa::Auto => dual_source,
    }
}

/// The frame latency asked for: `KUI_FRAME_LATENCY` if it is a positive
/// number, for comparing without a rebuild, else the launcher's.
fn wanted_frame_latency(launcher: u32) -> u32 {
    std::env::var("KUI_FRAME_LATENCY")
        .ok()
        .and_then(|v| v.trim().parse::<u32>().ok())
        .filter(|n| *n >= 1)
        .unwrap_or(launcher)
}

/// The text antialiasing asked for: `KUI_TEXT_AA` if set, for quick A/B
/// comparisons, else what the launcher was told.
fn wanted_text_aa(launcher: TextAa) -> TextAa {
    match std::env::var("KUI_TEXT_AA").ok().as_deref() {
        Some("gray") | Some("grayscale") => TextAa::Grayscale,
        Some("subpixel") | Some("lcd") => TextAa::Subpixel,
        _ => launcher,
    }
}

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

/// The runner's state for one app, over every window it opens.
///
/// Generic only in its last field: everything is written against
/// [`DynShell`] and compiled once, here; a `Shell<A>` is only built,
/// boxed, and handed over unsized. Written `impl<A: App> Shell<A>`,
/// the whole runner was instantiated and optimised again inside every app
/// crate, on every edit: the counter's release rebuild went from 1.20 s
/// at alpha.9 to 1.57 s at alpha.18 as the runner grew (backlog C49).
struct Shell<A: App + ?Sized> {
    title: String,
    /// What every window is created with (`Launcher::icon`).
    icon: icon::AppIcon,
    chrome: Chrome,
    /// Initial inner size (logical px), already clamped into the bounds.
    size: (f64, f64),
    min_size: Option<(f64, f64)>,
    max_size: Option<(f64, f64)>,
    text_aa: TextAa,
    /// What every renderer is configured with: `Launcher::frame_latency`
    /// under `KUI_FRAME_LATENCY`.
    frame_latency: u32,
    /// What every core is created with; see `Launcher::diagnostics`.
    diagnostics: bool,
    /// Whether the GPU blends per channel, decided by the first renderer,
    /// again by each device opened after a loss (`reopen_device`), and
    /// applied to every core.
    subpixel: bool,
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
    /// When the device was last opened again after being lost
    /// (`reopen_device`), so a device that will not open is tried once a
    /// second rather than once a frame.
    reopened: Option<std::time::Instant>,
    /// A new device was asked for inside the second after the last try,
    /// or the last try left a window without a renderer: `about_to_wait`
    /// opens it when `reopen_due` says, and sleeps until then — the ask
    /// is kept here rather than in a frame asked for again, which in an
    /// idle app nothing would ask for and in an animating one would be
    /// asked for every turn with no vsync to pace it.
    reopen_owed: bool,
    /// Whether `KUI_LOSE_DEVICE` has had its one loss.
    pretended_loss: bool,
    /// Caps Lock and Num Lock as the lock keys' presses have turned them,
    /// in any window — what a press reports where the OS is not asked
    /// (`keys::lock_state`, backlog F108). One keyboard, one record: it
    /// was each pane's, so a popup began at off and a toggle in one window
    /// never reached another (backlog RG96).
    locks: kui_core::KeyLocks,
    /// Origin of the frame clock handed to the cores for transitions.
    epoch: std::time::Instant,
    /// What the OS was asked for at startup — the accent colour, the
    /// reduce-motion setting and the language (`mod system_env`) — pushed
    /// into every pane's env each frame and re-asked when the user has
    /// evidently been somewhere else. The appearance is not here: it is
    /// per-window and comes off the window itself.
    system: system_env::Queried,
    /// What the app pinned over it (`Launcher::system`); merged in
    /// `sync_env`, so it is never lost to the per-frame write.
    pinned_system: SystemEnv,
    clipboard: Option<arboard::Clipboard>,
    /// The platform's context menu, where the platform has one (ADR 0017
    /// step 3). `None` on every other platform and on a macOS build that
    /// could not reach the main thread, and then the core draws its own.
    #[cfg(target_os = "macos")]
    native_menu: Option<macos_menu::MacMenu>,
    /// Whether a menu the core opened has been handed to the platform and
    /// is waiting for an answer. One per app: only one menu can be up.
    /// Beside `native_menu` because it means nothing without one.
    #[cfg(target_os = "macos")]
    menu_shown: bool,
    /// The platform's application menu bar, where the platform has one
    /// (`docs/adr/0018-a-menu-bar-the-app-declares.md`). One per process,
    /// because that is what macOS has: it carries the declaration of the
    /// window that has the keyboard, or of whichever window made one.
    #[cfg(target_os = "macos")]
    native_menu_bar: Option<macos_menu::MacMenuBar>,
    /// What it currently carries: a declaration — the window it came from
    /// and that core's `menu_bar_revision` — or the standard bar. The diff
    /// that keeps a re-declared bar from being rebuilt sixty times a second.
    #[cfg(target_os = "macos")]
    applied_menu_bar: Option<AppliedBar>,
    /// The audio device the core's audio commands drive; see `audio`.
    audio: audio::Audio,
    /// When the app was last doing something that could lead to a sound:
    /// any input, or any audio command. The device is warmed while this is
    /// recent and let go once it is not — see `AUDIO_IDLE_CLOSE`. Starts at
    /// launch, so a session holding sounds still opens the device before
    /// its first click the way it always did.
    audio_touch: std::time::Instant,
    /// `KUI_SMOKE_FRAMES=n`: quit after the main window has presented `n`
    /// frames, so an example is a self-terminating check — a real window
    /// on a real GPU, driven by the real loop, that exits 0 when it drew
    /// and non-zero when it did not. It is what the smoke round
    /// (examples/devtools/src/bin/smoke.rs) runs; the wgpu validation
    /// error that made `fragments` panic on
    /// first paint (an alignment the adapter and the device disagreed
    /// about) is exactly the class of bug no headless test can see.
    /// `None` — unset, or unparseable — is the ordinary endless run.
    smoke_frames: Option<u32>,
    /// Main-window frames presented so far, counted only while
    /// `smoke_frames` is set.
    frames_drawn: u32,
    /// Set by `WindowCommand::Close` on the main window; honored at the end
    /// of the event.
    exit_requested: bool,
    /// Why the main window or its renderer could not be made, when they
    /// could not: `run` and `open` return it as their error. It was an
    /// `expect`, and a panic there aborted a Node process outright — the
    /// unwind cannot cross the addon's boundary — when a compositor that
    /// had just gone away refused the window (backlog RG47).
    startup_error: Option<String>,
    /// Whether `App::teardown` has run: once, whichever of the loop's
    /// exit and the runner's retirement comes first.
    torn_down: bool,
    /// The one count of secure keyboard entry this runner may hold, moved
    /// at the end of every batch to whether a window whose frame asked
    /// (`Ui::secure_input`) has the keyboard, given back at teardown and
    /// on drop (backlog F85, `mod secure_input`).
    secure_input: secure_input::SecureInput,
    /// Driven by a `PumpRunner` rather than `run_app`: the main window's
    /// close ends the runner (`exit_requested`) instead of exiting winit's
    /// loop, which the next runner on this thread reuses (backlog F58).
    pumped: bool,
    /// Whether `resumed` has opened the main window — once per shell, so a
    /// reused loop that delivers no `resumed` opens it from `about_to_wait`
    /// and a main window the user closed is not reopened from there.
    opened: bool,
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
    /// What the last `about_to_wait` decided the control flow should be, kept
    /// so a host that owns the loop can read it (`PumpRunner::next_deadline`).
    /// `None` is `ControlFlow::Wait`: nothing the shell knows about is due.
    next_deadline: Option<std::time::Instant>,
    /// Whether this batch carried an OS event the shell acted on. A driver
    /// cannot see most of them — a pointer crossing a window that declares no
    /// hover produces no *app* event at all, and neither does a key nothing
    /// is listening for — so a driver pacing itself on what the app saw
    /// concludes that a window being moved across is idle. It is the reason
    /// `next_deadline` reports "now" after an event: the shell asked for a
    /// redraw and wants pumping to present it, and that is also the honest
    /// signal for "somebody is using this window".
    saw_event: bool,
    /// `saw_event` as `about_to_wait` took it, left for the pump runner to
    /// take in turn and count (`PumpRunner::woken_pumps`, backlog F94). A
    /// loop that owns itself never reads it.
    woke: bool,
    /// The host answers events after the loop hands them over
    /// (`Launcher::deferred_events`).
    deferred_events: bool,
    /// An input reached the app and the host has not answered yet, so the
    /// next frame would be painted from a view that predates that input.
    /// Set only under `deferred_events`; cleared when the host says its
    /// view is current ([`PumpRunner::request_redraw`], which every
    /// `setView` and every drained `pollEvents` reaches).
    ///
    /// A `Cell` because the clearing is the host's `&self` call, and the
    /// alternative — taking `&mut self` there — is a signature break for
    /// what is bookkeeping.
    owed: std::cell::Cell<bool>,
    /// Last, so a `Box<Shell<A>>` unsizes to a `Box<DynShell>`.
    app: A,
}

/// A shell over any app: what the runner is written against. Generic only
/// in a lifetime, which is erased, so its code is kui's and not the app
/// crate's (backlog C49), and an app that borrows is still an app.
type DynShell<'a> = Shell<dyn App + 'a>;

/// The core of window `id`, from a shell's two homes for one: the panes,
/// and the main window's core before its pane exists — the main window's
/// for a window that has closed since its event was made. Over the fields
/// rather than `&mut Shell`, so the app can be lent beside it.
fn window_core<'a>(
    panes: &'a mut [Pane],
    main_core: &'a mut Option<Core>,
    id: WindowId,
) -> Option<&'a mut Core> {
    let at = panes
        .iter()
        .position(|p| p.id == id)
        .or_else(|| panes.iter().position(|p| p.id == WindowId::MAIN));
    match at {
        Some(i) => Some(&mut panes[i].core),
        None => main_core.as_mut(),
    }
}

/// What winit's loop drives: it wants a sized handler, and a `DynShell`
/// is not one.
struct Handler<'s, 'a>(&'s mut DynShell<'a>);

impl ApplicationHandler<access_bridge::UserEvent> for Handler<'_, '_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        self.0.resumed(event_loop);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window: WinitWindowId,
        event: WindowEvent,
    ) {
        self.0.window_event(event_loop, window, event);
    }

    fn exiting(&mut self, event_loop: &ActiveEventLoop) {
        self.0.exiting(event_loop);
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: access_bridge::UserEvent) {
        self.0.user_event(event_loop, event);
    }

    fn about_to_wait(&mut self, event_loop: &ActiveEventLoop) {
        self.0.about_to_wait(event_loop);
    }
}

impl DynShell<'_> {
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

    /// `App::teardown`, once (backlog F74): from `exiting` — the loop's
    /// last word, which the OS's Quit reaches too, on macOS through
    /// `applicationWillTerminate` where the process ends without `run`
    /// ever returning — and from a pumped runner's retirement, whichever
    /// comes first.
    fn teardown_once(&mut self) {
        // The keyboard is given back before the app's teardown runs, and
        // on every path here: a process that ends from `exiting` never
        // drops the runner (backlog F85).
        self.secure_input.set(false);
        if !self.torn_down {
            self.torn_down = true;
            self.app.teardown();
        }
    }

    /// The main window is done: `run_app`'s loop exits; a pumped loop is
    /// left running for the next runner, and the runner ends itself on
    /// the flag (see `PARKED_LOOP`).
    fn exit_main(&mut self, event_loop: &ActiveEventLoop) {
        self.exit_requested = true;
        if !self.pumped {
            event_loop.exit();
        }
    }

    /// Whether the platform's own drag callbacks are answering for every
    /// window, so winit's positionless file events are the duplicates.
    fn file_drag_is_overridden(&self) -> bool {
        #[cfg(target_os = "macos")]
        {
            macos_drop::installed()
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

    /// The file drags the platform reported since the last turn (ADR
    /// 0031): on macOS the delegate override's messages, each with the
    /// position AppKit gave it, dispatched to the pane whose delegate
    /// spoke and followed by the stamp that delegate answers the OS from;
    /// elsewhere the batch winit's per-file events built, at the pane's
    /// last cursor.
    fn pump_file_drag(&mut self, event_loop: &ActiveEventLoop) {
        #[cfg(target_os = "macos")]
        for (delegate, msg) in macos_drop::take_messages() {
            let Some(i) = self
                .panes
                .iter()
                .position(|p| macos_drop::delegate_ptr(&p.window) == Some(delegate))
            else {
                continue;
            };
            let ev = match msg {
                macos_drop::DragMsg::Over(paths, at) => InputEvent::DragFiles { paths, at },
                macos_drop::DragMsg::Drop(paths, at) => InputEvent::DropFiles { paths, at },
                macos_drop::DragMsg::Cancel => InputEvent::DragCancel,
            };
            if let Some(i) = self.dispatch(event_loop, i, ev) {
                let pane = &self.panes[i];
                macos_drop::stamp(&pane.window, pane.core.drop_target().is_some());
            }
        }
        // Collected first, then dispatched by window id: a drop's handler
        // may close its window, and a pane's index is not its identity
        // (backlog AR39).
        let batches: Vec<(WindowId, InputEvent)> = self
            .panes
            .iter_mut()
            .filter_map(|pane| {
                let dropped = pane.file_drag_pending.take()?;
                let paths = std::mem::take(&mut pane.file_drag);
                let at = pane.cursor;
                let ev = if dropped {
                    InputEvent::DropFiles { paths, at }
                } else {
                    InputEvent::DragFiles { paths, at }
                };
                Some((pane.id, ev))
            })
            .collect();
        for (id, ev) in batches {
            if let Some(i) = self.pane_of(id) {
                self.dispatch(event_loop, i, ev);
            }
        }
    }

    /// Hands one input to pane `i`'s core and does what followed from it:
    /// the events routed, the menu actions, the window commands, the
    /// audio. Returns where that pane is afterwards — the commands may
    /// have closed a window, and a close in front of it moves it down,
    /// so its index is not its identity (backlog AR39); `None` when the
    /// input closed the pane itself. A caller that goes on addressing the
    /// pane goes on with the returned index.
    fn dispatch(
        &mut self,
        event_loop: &ActiveEventLoop,
        i: usize,
        ev: InputEvent,
    ) -> Option<usize> {
        let t0 = std::time::Instant::now();
        // Someone is using the app, so a sound may be moments away: keep
        // the device warm (`AUDIO_IDLE_CLOSE`).
        self.audio_touch = t0;
        let completes = input_completes(&ev);
        let here = self.panes[i].id;
        let events = self.panes[i].core.handle_input(ev);
        let reached_app = self.route_events(events);
        self.owe_for(reached_app && completes);
        self.apply_menu_actions(event_loop, i);
        self.apply_window_commands(event_loop);
        self.apply_audio();
        self.pump_native_menu(event_loop);
        let i = self.pane_of(here)?;
        let pane = &mut self.panes[i];
        pane.apply_cursor();
        // Hover styling depends on input too, so any input redraws. A
        // damage pass can tighten this later.
        pane.core.stats.pending_input_ms += t0.elapsed().as_secs_f32() * 1e3;
        pane.window.request_redraw();
        Some(i)
    }

    /// Hands the session's queued audio commands to the device. A session
    /// that holds a sound is going to play one: the device starts opening
    /// here, on its own thread, so the first play finds it open instead of
    /// stalling the frame for the ~90 ms the open takes.
    fn apply_audio(&mut self) {
        let core = self.core_mut();
        let cmds = core.take_audio_commands();
        let resources = core.resources.clone();
        if !cmds.is_empty() {
            self.audio_touch = std::time::Instant::now();
        }
        // Warmed while the app is being used and let go when it is not.
        // Not every frame: a frame is drawn for the caret, for a
        // transition, for a window moving over the top — none of which is
        // anybody about to play anything, and re-warming on one of those
        // would reopen the device the moment `about_to_wait` closed it.
        if resources.has_sounds() && self.audio_touch.elapsed() < AUDIO_IDLE_CLOSE {
            self.audio.warm();
        }
        // Two things only the device knows come back here. A `Stop` it found
        // still playing is a one-shot cut off, which the core turns into
        // `truncated-playback` on the node that went away; a refused play
        // never starts and so never ends, so the core turns that into a
        // `refused` event and a warning, or a tagged node waits on an
        // `ended` that cannot come.
        let answered = self.audio.apply(cmds, &resources);
        if answered.truncated.is_empty() && answered.refused.is_empty() {
            return;
        }
        let core = self.core_mut();
        for (playback, at) in answered.truncated {
            core.audio_truncated(playback, at);
        }
        for playback in answered.refused {
            core.audio_refused(playback);
        }
        self.route_playback_events();
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
        self.route_playback_events();
    }

    /// Routes whatever the audio fold-back queued (`ended`, `refused`) and
    /// redraws for what handling it changed.
    fn route_playback_events(&mut self) {
        let pending = self.core_mut().take_pending_events();
        if pending.is_empty() {
            return;
        }
        self.route_events(pending);
        for p in &self.panes {
            p.redraw_for(FrameCause::AUDIO);
        }
    }

    /// An input's events reached the app: under `deferred_events` the next
    /// frame owes the host's answer (see `frame_waits_for_host`).
    fn owe_for(&self, reached_app: bool) {
        if reached_app && self.deferred_events {
            self.owed.set(true);
        }
    }

    /// Returns whether anything reached the app (as against an extension
    /// answering for itself).
    fn route_events(&mut self, events: Vec<UiEvent>) -> bool {
        let mut reached_app = false;
        // An extension's replies go to whoever declared its slot (ADR 0014
        // decision 6): not routed by origin — a reply is addressed by being
        // one — and carrying the extension's origin, the window and the key
        // of the event it answered, so the receiver knows who spoke and
        // from where. For every extension this host placed itself, the
        // receiver is this host; for one a guest placed, it is the guest,
        // and `route` is the walk up.
        let Shell {
            extensions,
            app,
            panes,
            main_core,
            ..
        } = self;
        extensions.route(events, |ev| {
            reached_app = true;
            // Lent the core of the window the event came from (ADR 0036).
            match window_core(panes, main_core, ev.window) {
                Some(core) => app.on_event_with(ev, core),
                None => app.on_event(ev),
            }
        });
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
                p.redraw_for(FrameCause::ELSEWHERE);
            }
        }
        reached_app
    }

    /// Direct edits (cut) mutate the document outside `handle_input`, so
    /// the `changed` the core queued for it is routed here, and the window
    /// redrawn. The core posts the event (AR15: this used to build one by
    /// hand, and the menu's Cut posted none), so it is stamped and routed
    /// like the ones a keystroke makes.
    fn after_direct_edit(&mut self, i: usize) {
        let events = self.panes[i].core.take_pending_events();
        // A cut is input too: the frame that shows the text gone should
        // show what the app made of `changed`.
        let reached_app = self.route_events(events);
        self.owe_for(reached_app);
        // The one caller is the runner's own ⌘X, which the core never
        // hears as a key.
        self.panes[i].redraw_for(FrameCause::KEY);
    }

    /// Builds and draws one window's frame.
    fn redraw(&mut self, i: usize) {
        let Shell {
            app,
            extensions,
            panes,
            min_size,
            max_size,
            epoch,
            system,
            pinned_system,
            audio,
            pretended_loss,
            ..
        } = self;
        let pane = &mut panes[i];
        // What the driver knows and the view only reads, refreshed for
        // this frame (it was already filled in when the pane opened).
        sync_env(pane, system, *pinned_system, audio.env());
        let window = &pane.window;
        // The core turns a changed viewport into a `resize` event, routed
        // with the rest of the pending events after this frame.
        let (viewport, scale) = pane.size();
        let size = window.inner_size();
        let t_view = std::time::Instant::now();
        pane.core.set_time(epoch.elapsed().as_secs_f64());
        // Why the runner asked, beside the input the core recorded: the
        // view reads both as `frame_cause` (backlog F111).
        pane.core.note_frame_cause(pane.cause.take());
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

        // The level, the same way (backlog C30): a per-frame fact, applied
        // when it differs from what this pane has, and a popup's is never
        // touched. What the OS made of it is `env.window.always_on_top`
        // on the next `sync_env`, from the record `level_change` keeps.
        if let Some(level) = level_change(
            pane.kind,
            pane.core.always_on_top(),
            &mut pane.applied_on_top,
        ) {
            window.set_window_level(level);
        }

        // Which Option keys are Alt, the same way (backlog F113): applied
        // when it differs from what this window has, never per frame. A
        // popup's own ask is applied too and never read — it cannot
        // become key on macOS, so its keys come through its owner.
        if let Some(_option_as_alt) =
            option_as_alt_change(pane.core.option_as_alt(), &mut pane.applied_option_as_alt)
        {
            #[cfg(target_os = "macos")]
            {
                use winit::platform::macos::{OptionAsAlt as Winit, WindowExtMacOS};
                window.set_option_as_alt(match _option_as_alt {
                    kui_core::OptionAsAlt::None => Winit::None,
                    kui_core::OptionAsAlt::Left => Winit::OnlyLeft,
                    kui_core::OptionAsAlt::Right => Winit::OnlyRight,
                    kui_core::OptionAsAlt::Both => Winit::Both,
                });
            }
        }

        // The floor the app declared is a floor on the *app*: while the
        // devtools are docked in the main window, the pane's extent goes
        // on top of it, so the OS stops the window where the app is at
        // its minimum and not where the app less the dock is. After the
        // frame, since the handle's drag and the placement buttons land
        // in one; on change only, since it is a window-manager call.
        if pane.id == WindowId::MAIN
            && let Some((mw, mh)) = *min_size
        {
            let inset = pane.core.devtools_inset();
            let want = clamp_size((mw + inset.w as f64, mh + inset.h as f64), None, *max_size);
            if pane.applied_min != Some(want) {
                pane.applied_min = Some(want);
                window.set_min_inner_size(Some(LogicalSize::new(want.0, want.1)));
            }
        }

        // Anchor the OS IME candidate window at the focused caret.
        if let Some(r) = pane.core.ime_rect() {
            window.set_ime_cursor_area(
                winit::dpi::LogicalPosition::new(r.x, r.y),
                winit::dpi::LogicalSize::new(r.w.max(1.0), r.h),
            );
        }
        // And say what the platform's own text input may ask the view:
        // whether there is somewhere to type, and where the caret is.
        #[cfg(target_os = "macos")]
        macos_text_input::stamp(window, &pane.core);

        let t_render = std::time::Instant::now();
        // KUI_LOSE_DEVICE=SECS marks the device lost that long after
        // launch, once, to see the reopening below happen without a
        // driver update to cause it.
        if let Some(secs) = lose_device_at()
            && !*pretended_loss
            && epoch.elapsed().as_secs_f64() >= secs
            && let Some(r) = pane.renderer.as_ref()
        {
            *pretended_loss = true;
            r.gpu().mark_lost();
        }
        // The ground under the frame is a theme role like any other
        // (ADR 0019). Without this a view that paints no root background
        // — every example that lets the window show through — would show
        // the renderer's own near-black on a light desktop, which is the
        // one surface a `bg` prop cannot reach.
        // Written straight through: the renderer asks for a non-sRGB
        // surface, so a clear component is the byte it lands as, the same
        // way a quad's colour is.
        let ground = pane.core.theme().bg;
        let clear = kui_wgpu::wgpu::Color {
            r: ground.r as f64,
            g: ground.g as f64,
            b: ground.b as f64,
            a: ground.a as f64,
        };
        let (dl, atlas) = pane.core.output();
        let mut wait_ms = 0.0;
        let mut reopen = false;
        let drawn = pane.renderer.as_mut().map(|r| {
            r.clear_color = clear;
            r.render(dl, atlas)
        });
        match drawn {
            Some(Ok(report)) => {
                wait_ms = report.vsync_wait_ms;
                pane.pacer
                    .presented(std::time::Instant::now(), (size.width, size.height));
                pane.retry.presented();
                pane.surface_tries = 0;
                pane.awaits_device = false;
            }
            Some(Err(kui_wgpu::RenderError::Reconfigure)) => {
                if let Some(r) = pane.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
                pane.redraw_for(FrameCause::RETRY);
            }
            // The surface is configured wrong for the window — a size the
            // platform never told us, a swapchain a driver update left
            // behind: configure it to the window's size and draw again;
            // a surface that stays wrong is given up with its device.
            // Past the tries the frame asks for no other: the reopen is
            // what asks for the next one, and if it has to wait out its
            // second, a frame asked for now would only be refused again —
            // at once, with no vsync to pace it. Said once per surface
            // given up, not once per refused frame.
            Some(Err(kui_wgpu::RenderError::Validation)) => {
                match surface_refused(&mut pane.surface_tries) {
                    Refused::Retry => {
                        if let Some(r) = pane.renderer.as_mut() {
                            r.resize(size.width, size.height);
                        }
                        pane.redraw_for(FrameCause::RETRY);
                    }
                    Refused::GiveUp { say } => {
                        if say {
                            eprintln!("kui: the surface stays invalid; reopening the device");
                        }
                        reopen = true;
                    }
                }
            }
            // Occluded or timed out: nothing to present, and the frame is
            // owed — *scheduled*, since nothing else may ask for one. A
            // window ordered front, or brought back with ⌘-Tab, reports
            // itself occluded for a beat or two before the platform
            // catches up; its frame dropped, it showed the one from
            // before until a stray mouse move woke it (F102). Only for a
            // bounded number of tries, and not while the platform says
            // the window is covered, so a hidden window does not spin.
            Some(Err(kui_wgpu::RenderError::Skip)) => {
                let now = std::time::Instant::now();
                pane.retry.skipped(now);
                // A skipped frame paces the next as a presented one does:
                // what asks while the surface skips (a waker, the host) is
                // held for the display, or its fallback, instead of drawn
                // at once into another skip (backlog RG98).
                pane.pacer.presented(now, (size.width, size.height));
            }
            // The device is gone (a driver update, a GPU reset), or a
            // frame ago it was and no new one could be opened: open one.
            Some(Err(kui_wgpu::RenderError::DeviceLost)) | None => reopen = true,
        }
        pane.awaits_device |= reopen;
        let render_ms = (t_render.elapsed().as_secs_f32() * 1e3 - wait_ms).max(0.0);

        pane.core.stats.push(FrameSample {
            input_ms: 0.0,
            view_ms,
            layout_ms,
            render_ms,
            wait_ms,
        });
        if reopen {
            self.reopen_device();
        }
    }

    /// The device is gone — a driver update or a GPU reset took it, and
    /// every swapchain with it — so one new device is opened, and a
    /// renderer on it for every window, each window's old one dropped
    /// before its new surface is made (DXGI gives a window one flip-model
    /// swapchain). The windows keep their cores: the next frame draws
    /// what the last one would have. A window whose renderer cannot be
    /// made has none, and the device is tried again a second later; a
    /// device that cannot be opened is tried, and said, once a second, not
    /// once a frame. Asked for inside that second, the ask is owed
    /// (`reopen_owed`) and `about_to_wait` makes it when the second is up.
    fn reopen_device(&mut self) {
        let now = std::time::Instant::now();
        if reopen_due(self.reopened, now) > now {
            self.reopen_owed = true;
            return;
        }
        self.reopened = Some(now);
        // Dropped, not leaked: the old swapchain has to be released for
        // DXGI to allow the window a new one (leaked, the new surface's
        // configure fails with "invalid surface").
        for pane in &mut self.panes {
            pane.renderer = None;
            pane.surface_tries = 0;
        }
        self.gpu = None;
        let mut gpu: Option<kui_wgpu::Gpu> = None;
        for pane in &mut self.panes {
            let px = pane.window.inner_size();
            let made = match &gpu {
                None => pollster::block_on(kui_wgpu::Renderer::new(
                    pane.window.clone(),
                    px.width,
                    px.height,
                )),
                Some(gpu) => {
                    kui_wgpu::Renderer::new_in(gpu, pane.window.clone(), px.width, px.height)
                }
            };
            match made {
                Ok(mut r) => {
                    r.set_frame_latency(self.frame_latency);
                    let opened = gpu.is_none();
                    gpu.get_or_insert_with(|| r.gpu().clone());
                    if opened {
                        eprintln!("kui: device reopened");
                    }
                    pane.renderer = Some(r);
                    pane.redraw_for(FrameCause::DEVICE);
                }
                Err(err) => eprintln!("kui: cannot reopen window {}: {err}", pane.id.0),
            }
            pane.awaits_device = pane.renderer.is_none();
        }
        // A window left without a renderer — or every window, when no
        // device would open — is tried again a second from now, whether
        // or not anything asks it for a frame.
        self.reopen_owed = self.panes.iter().any(|p| p.awaits_device);
        // The new device may be another adapter's, with or without the
        // per-channel blend LCD masks need: decided again, for every core,
        // or a core keeps rasterising masks the new pipeline cannot split.
        // A changed decision empties each core's atlas (`set_subpixel_text`),
        // which the fresh renderer was going to upload whole anyway.
        if let Some(gpu) = &gpu {
            let on = subpixel_on(wanted_text_aa(self.text_aa), gpu.dual_source());
            if on != self.subpixel {
                self.subpixel = on;
                for pane in &mut self.panes {
                    pane.core.set_subpixel_text(on);
                }
            }
        }
        self.gpu = gpu;
    }
}

/// Which bar `pump_menu_bar` last handed the platform.
#[cfg(target_os = "macos")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AppliedBar {
    /// The standard bar (ADR 0030): no window declared one, or the front
    /// window draws its own declaration and the platform's is the standard.
    Standard,
    /// A window's declaration, at that revision.
    Declared(WindowId, u64),
}

impl ApplicationHandler<access_bridge::UserEvent> for DynShell<'_> {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if !self.panes.is_empty() || self.opened {
            return;
        }
        self.opened = true;
        // KUI_WINDOW=WxH overrides the initial size (useful for testing).
        let size = std::env::var("KUI_WINDOW")
            .ok()
            .and_then(|s| {
                let (w, h) = s.split_once('x')?;
                Some((w.parse().ok()?, h.parse().ok()?))
            })
            .map(|s| clamp_size(s, self.min_size, self.max_size))
            .unwrap_or(self.size);
        let mut attrs = self.window_attrs(&self.title.clone(), size, self.chrome);
        if let Some((mw, mh)) = self.min_size {
            attrs = attrs.with_min_inner_size(LogicalSize::new(mw, mh));
        }
        if let Some((mw, mh)) = self.max_size {
            attrs = attrs.with_max_inner_size(LogicalSize::new(mw, mh));
        }
        let window = match event_loop.create_window(attrs) {
            Ok(w) => Arc::new(w),
            Err(e) => return self.fail_open(event_loop, format!("cannot open the window: {e}")),
        };
        self.icon.set_on(&window);
        let px = window.inner_size();
        let renderer =
            pollster::block_on(kui_wgpu::Renderer::new(window.clone(), px.width, px.height));
        let mut renderer = match renderer {
            Ok(r) => r,
            Err(e) => return self.fail_open(event_loop, format!("cannot draw in the window: {e}")),
        };
        renderer.set_frame_latency(self.frame_latency);
        // Subpixel text only where the renderer blends per channel; the
        // env var wins over the builder for quick A/B comparisons.
        self.subpixel = subpixel_on(wanted_text_aa(self.text_aa), renderer.subpixel_text());
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
            self.chrome,
            core,
            window,
            renderer,
        );
        self.panes[0].applied_title = self.title.clone();
        self.panes[0].applied_min = self.min_size;
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        id: WinitWindowId,
        event: WindowEvent,
    ) {
        let Some(i) = self.pane_index(id) else { return };
        // Everything but the redraw itself: a redraw is the *answer* to an
        // event, so counting it would make a frame its own reason for the
        // next one and an animation would report activity for ever.
        if !matches!(event, WindowEvent::RedrawRequested) {
            self.saw_event = true;
        }
        {
            let pane = &mut self.panes[i];
            if let Some(bridge) = &mut pane.access {
                bridge.process_event(&pane.window, &event);
            }
        }
        match event {
            WindowEvent::CloseRequested => {
                if self.panes[i].id == WindowId::MAIN {
                    self.exit_main(event_loop);
                } else {
                    let id = self.panes[i].id;
                    self.close_pane(event_loop, id);
                }
            }
            // On Windows minimizing and restoring are each a `WM_SIZE`:
            // the first is where the window went dark, the second the
            // frame that brings it back (backlog RG97).
            WindowEvent::Resized(size) => {
                let pane = &mut self.panes[i];
                if let Some(r) = pane.renderer.as_mut() {
                    r.resize(size.width, size.height);
                }
                if pane.minimized() {
                    pane.cause.went_dark();
                    pane.redraw_for(FrameCause::RESIZE);
                } else if pane.cause.is_dark() && cfg!(target_os = "windows") {
                    pane.came_back(FrameCause::RESIZE);
                } else {
                    // Elsewhere a window resized while covered is still
                    // covered; `Occluded(false)` is its way back.
                    pane.redraw_for(FrameCause::RESIZE);
                }
            }
            WindowEvent::ScaleFactorChanged { .. } => {
                self.panes[i].redraw_for(FrameCause::SCALE);
            }
            WindowEvent::CursorMoved { position, .. } => {
                let pane = &mut self.panes[i];
                let scale = pane.window.scale_factor() as f32;
                let p = Vec2::new(position.x as f32 / scale, position.y as f32 / scale);
                if pane.synthesizes_resize() {
                    pane.resize_edge = pane.resize_edge_at(p);
                }
                if pane.cursor != p {
                    pane.scroll_gesture.pointer_moved();
                }
                pane.cursor = p;
                let from = pane.id;
                self.dispatch(event_loop, i, InputEvent::CursorMoved(p));
                // And then, if this pane is holding a press that a popup
                // joined, the same move again in that popup's coordinates.
                self.retarget_move(event_loop, from, p);
            }
            WindowEvent::CursorLeft { .. } => {
                self.dispatch(event_loop, i, InputEvent::CursorLeft);
            }
            // Files dragged in from the OS, as winit reports them: one
            // event per file and no position (ADR 0031, decision 5). On
            // macOS the delegate override supersedes all three with the
            // position AppKit has (`mod macos_drop`); elsewhere the files
            // are collected per batch and dispatched at its end, at the
            // pane's last reported cursor — the point at enter and at
            // release, since no platform here reports the drag moving.
            WindowEvent::HoveredFile(path) => {
                if !self.file_drag_is_overridden() {
                    let pane = &mut self.panes[i];
                    pane.file_drag.push(path.to_string_lossy().into_owned());
                    pane.file_drag_pending = Some(false);
                }
            }
            WindowEvent::DroppedFile(path) => {
                if !self.file_drag_is_overridden() {
                    let pane = &mut self.panes[i];
                    // A hover batch still waiting in the same turn is the
                    // same files: the drop's list starts over.
                    if pane.file_drag_pending != Some(true) {
                        pane.file_drag.clear();
                    }
                    pane.file_drag.push(path.to_string_lossy().into_owned());
                    pane.file_drag_pending = Some(true);
                }
            }
            WindowEvent::HoveredFileCancelled => {
                if !self.file_drag_is_overridden() {
                    let pane = &mut self.panes[i];
                    pane.file_drag.clear();
                    pane.file_drag_pending = None;
                    self.dispatch(event_loop, i, InputEvent::DragCancel);
                }
            }
            // Recorded, not acted on: what a view reads is derived from
            // every window's copy at the end of the batch (`settle_focus`),
            // because focus *moving* is two events and neither alone is the
            // answer.
            WindowEvent::Focused(focused) => {
                self.panes[i].os_focused = focused;
                // A modifier let go elsewhere never comes up here: what
                // was down is forgotten with the keyboard (backlog F108).
                if !focused {
                    self.panes[i].modifier_keys_down.clear();
                }
                // Coming back to the app is the cheap, reliable sign that
                // the user may have been in a settings app: the accent and
                // the reduce-motion setting have no event to subscribe to
                // here, and re-asking costs microseconds against something
                // that happens when a human switches windows.
                if focused {
                    let before = (self.system, self.panes[i].appearance);
                    self.system = system_env::query();
                    // And the window's own setting, in case the platform
                    // changed it without an event while we were away.
                    self.panes[i].appearance = appearance_of(&self.panes[i].window);
                    // A change found this way has no event of its own
                    // behind it, so ask for the frame that reports it:
                    // `begin_frame` turns the difference into a `system`
                    // event, and a host that only draws on input would
                    // otherwise not learn of it until it drew for
                    // something else (F40).
                    if before != (self.system, self.panes[i].appearance) {
                        for p in &self.panes {
                            p.redraw_for(FrameCause::APPEARANCE);
                        }
                    }
                }
            }
            // Covered or uncovered — where the platform says (macOS, X11):
            // uncovered, the window shows what it presented before it was
            // covered, so a frame is owed now (F102); covered, frames the
            // surface skips are not retried.
            // Covered is where a window goes dark, minimized on macOS
            // included, and uncovered where it comes back (RG97).
            WindowEvent::Occluded(covered) => {
                self.panes[i]
                    .retry
                    .occluded(covered, std::time::Instant::now());
                if covered {
                    self.panes[i].cause.went_dark();
                } else {
                    self.panes[i].came_back(FrameCause::OCCLUSION);
                }
            }
            // The theme changing is the other one, and the only one that
            // arrives while the app is in front. The next frame reads the
            // appearance off the window anyway; the redraw is what makes
            // there *be* a next frame in an app that only draws on input.
            WindowEvent::ThemeChanged(theme) => {
                self.panes[i].appearance = theme_appearance(Some(theme));
                self.system = system_env::query();
                self.panes[i].redraw_for(FrameCause::APPEARANCE);
            }
            // The four keyboard events go to `key_target`, which is this
            // pane unless it is lending its keyboard to a non-activating
            // popup. The modifier mirror follows them, or the popup would
            // read a stale Shift.
            WindowEvent::ModifiersChanged(m) => {
                // The keyboard's state is one fact for the pair: the OS
                // delivers the edge to the owner, the target reads it for
                // the keys it borrows, and the owner keeps reading it once
                // the popup is gone — written to the target alone, a Shift
                // released while a menu was up left the owner's next key a
                // Shift chord (AR23).
                let t = self.key_target(i);
                self.panes[i].modifiers = m.state();
                self.panes[t].modifiers = m.state();
                use winit::keyboard::ModifiersKeyState::Pressed;
                let alt = (m.lalt_state() == Pressed, m.ralt_state() == Pressed);
                self.panes[i].alt_held = alt;
                self.panes[t].alt_held = alt;
                let kmods = self.panes[t].kmods();
                self.dispatch(event_loop, t, InputEvent::Modifiers(kmods));
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let t = self.key_target(i);
                self.on_key(event_loop, i, t, event)
            }
            WindowEvent::Ime(Ime::Commit(text)) => {
                // Its own channel, not `Text`: a sink hears a commit as a
                // `text` event and a keystroke as a `key` event, once each
                // (backlog C17); a stock editor takes both the same way.
                let t = self.key_target(i);
                self.dispatch(event_loop, t, InputEvent::Commit(text));
            }
            WindowEvent::Ime(Ime::Preedit(text, cursor)) => {
                let t = self.key_target(i);
                self.dispatch(event_loop, t, InputEvent::Preedit(text, cursor));
            }
            // Force Touch: stage 2 is the deepened press macOS calls a
            // force click. winit reports the whole ramp, and only the
            // *edge* into stage 2 is the gesture — a stage that stays at 2
            // while the finger presses harder is the same click still
            // happening (ADR 0017, decision 6). macOS-only: no other
            // winit backend reports pressure at all.
            WindowEvent::TouchpadPressure { stage, .. } => {
                let pane = &mut self.panes[i];
                let was = std::mem::replace(&mut pane.pressure_stage, stage);
                if stage >= 2 && was < 2 {
                    let at = pane.cursor;
                    self.dispatch(event_loop, i, InputEvent::ForceClick(at));
                }
            }
            WindowEvent::MouseWheel { delta, phase, .. } => {
                let started = phase == winit::event::TouchPhase::Started;
                let pane = &mut self.panes[i];
                let scale = pane.window.scale_factor() as f32;
                let now = std::time::Instant::now();
                let (d, kind) = match delta {
                    winit::event::MouseScrollDelta::LineDelta(x, y) => {
                        pane.axis_lock.line();
                        (
                            Vec2::new(Core::lines_to_px(x), Core::lines_to_px(y)),
                            scroll_gesture::Kind::Line,
                        )
                    }
                    // A trackpad's swipe keeps to its axis (`mod axis_lock`).
                    winit::event::MouseScrollDelta::PixelDelta(p) => (
                        pane.axis_lock
                            .pixel(Vec2::new(p.x as f32 / scale, p.y as f32 / scale), now),
                        scroll_gesture::Kind::Pixel,
                    ),
                };
                // The gesture it is part of keeps the targets it began
                // with (`mod scroll_gesture`, backlog F107).
                if d != Vec2::ZERO {
                    let begins = pane.scroll_gesture.begins(kind, started, now);
                    self.dispatch(
                        event_loop,
                        i,
                        InputEvent::ScrollGesture { delta: d, begins },
                    );
                } else {
                    pane.scroll_gesture.note(kind, started, now);
                }
            }
            WindowEvent::MouseInput { state, button, .. } => {
                let button = match button {
                    WinitButton::Left => MouseButton::Primary,
                    WinitButton::Right => MouseButton::Secondary,
                    WinitButton::Middle => MouseButton::Middle,
                    // `onButton` hears these as `Other`'s code (backlog
                    // F105), so the numbering has to be stable: back,
                    // forward, then whatever the platform reports beyond
                    // them.
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
                    let mut reached_app = false;
                    for (id, activates) in self.popups_outside(i) {
                        reached_app |= self.dismiss(id, DismissReason::Outside);
                        consumed |= primary && !activates;
                    }
                    if consumed {
                        self.swallowed_press = Some(here);
                        // Choosing in a menu closes it *and* does what it
                        // says: one frame, so this one waits for the host.
                        self.owe_for(reached_app);
                        // The press the core never saw.
                        for p in &self.panes {
                            p.redraw_for(FrameCause::BUTTON);
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
            WindowEvent::RedrawRequested
                if frame_waits_for_host(self.owed.get(), self.panes[i].deferred_frame) =>
            {
                // What would be painted predates an input the app has been
                // told about and not yet answered — the release of a
                // button, with the count still at its old value. The host
                // asks again the moment its handler has run, and that
                // frame carries both. Never twice running, so nothing here
                // can stop a window painting.
                self.panes[i].deferred_frame = true;
            }
            // A frame asked for while frames run back to back waits for
            // the display, which asks again at the vsync (`mod pacer`).
            WindowEvent::RedrawRequested if !self.panes[i].admit_frame() => {}
            WindowEvent::RedrawRequested => {
                self.panes[i].deferred_frame = false;
                self.redraw(i);
                // KUI_SMOKE_FRAMES: count what the main window actually
                // landed and quit at the target. Counted only once a
                // present succeeded, so a window that never draws never
                // counts and the run fails on the caller's timeout rather
                // than passing quietly. Asking for the next
                // one keeps an app that paints only on input painting, so
                // every example reaches the count at the same speed.
                if let Some(n) = self.smoke_frames
                    && let Some(p) = self.panes.get(i)
                    && p.id == WindowId::MAIN
                    && p.retry.presented_once()
                {
                    self.frames_drawn += 1;
                    if self.frames_drawn >= n {
                        self.exit_requested = true;
                    } else {
                        self.panes[i].redraw_for(FrameCause::SMOKE);
                    }
                }
                self.panes[i].publish_access();
                // Views can declare windows and window commands too
                // (ui.window, ui.window_command); apply them the same frame
                // they were declared. Likewise the sounds a frame started
                // (audio nodes, ui.play), and the clipboard work a view
                // queued (ui.set_clipboard, ui.request_paste — backlog
                // C33), which until now only an input's drain reached.
                self.apply_menu_actions(event_loop, i);
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
                        p.redraw_for(FrameCause::AFTER_FRAME);
                    }
                }
                // Windows moves and resizes a window inside its own modal
                // loop, where `about_to_wait` — the pacing that asks for
                // the next frame of an animation — does not run, so a
                // transition froze for as long as the title bar was held
                // (backlog W3). The pane's own timer is what answers that,
                // and this is where it learns whether there is anything to
                // animate; `mod windows_anim` is why it is a timer and not
                // a redraw asked for from right here. Not while the window
                // waits for a device: `about_to_wait` asks it for no
                // animation frames then (RG29), and the timer asked for 64
                // a second, each a view built for no renderer (RG40). The
                // reopen's own `request_redraw` arms it again. Nor while
                // it is minimized (RG45); the restore's `Resized` does.
                #[cfg(target_os = "windows")]
                if let Some(p) = self.panes.get_mut(i) {
                    let animating = p.animates_now();
                    if let Some(t) = &mut p.anim_timer {
                        t.set(animating);
                    }
                }
            }
            _ => {}
        }
        if self.exit_requested {
            self.exit_main(event_loop);
        }
    }

    /// AccessKit's side of the conversation: assistive technology attaching
    /// (send it the tree, and let the view know), detaching, or asking for
    /// an action (input).
    /// The loop's last event: the app's `teardown`, before the process
    /// goes (a Quit from the OS) or `run` returns (the main window
    /// closed).
    fn exiting(&mut self, _event_loop: &ActiveEventLoop) {
        self.teardown_once();
    }

    fn user_event(&mut self, event_loop: &ActiveEventLoop, event: access_bridge::UserEvent) {
        // A wake is the app saying "what `view` shows has changed": every
        // window draws, as after any input. Coalesced by the platform's
        // queue, so a thread waking a thousand times a frame costs one.
        self.saw_event = true;
        if matches!(event, access_bridge::UserEvent::Wake) {
            for pane in &self.panes {
                pane.redraw_for(FrameCause::WAKE);
            }
            return;
        }
        let Some(i) = access_bridge::window_of(&event).and_then(|w| self.pane_index(w)) else {
            return;
        };
        // A file dialog's answer, from the thread that waited on it: the
        // window that asked hears it as input.
        let event = match event {
            access_bridge::UserEvent::Files { paths, .. } => {
                self.dispatch(event_loop, i, InputEvent::Files(paths));
                return;
            }
            other => other,
        };
        let Some(bridge) = &mut self.panes[i].access else {
            return;
        };
        let was_listening = bridge.active();
        let req = bridge.on_event(event);
        // A client attaching or leaving is a fact the view reads
        // (`env.system.assistive`, backlog F48): `sync_env` writes it on
        // the next frame and the core reports it as a `system` event, so
        // the frame has to happen — an app that only redraws on input
        // would otherwise hear it with the next click.
        if bridge.active() != was_listening {
            self.panes[i].redraw_for(FrameCause::APPEARANCE);
        }
        if let Some(req) = req {
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
        // A loop taken back from an earlier runner delivered its init
        // events — `resumed` among them — to that runner's shell; this one
        // opens its window from the first turn it gets instead.
        if self.pumped && !self.opened && !self.exit_requested {
            self.resumed(event_loop);
        }
        // The platform's menu comes and goes between turns of the loop, so
        // this is where its answer is collected. The menu bar is handed
        // over and read back from the same place, for the same reason.
        self.pump_native_menu(event_loop);
        self.pump_menu_bar(event_loop);
        self.pump_text_input(event_loop);
        self.pump_file_drag(event_loop);
        self.settle_focus();
        self.apply_secure_input();
        self.dismiss_popups_if_deactivated();
        self.poll_audio();
        self.apply_audio();
        let now = std::time::Instant::now();
        let mut deadline: Option<std::time::Instant> = None;
        // A new device owed (`reopen_owed`): made now if its second is
        // up, and otherwise — or when this try left a window without one —
        // woken for when it is. This deadline is the only thing that
        // brings the loop back to it: a window waiting for a device asks
        // for no frames of its own, below.
        if self.reopen_owed {
            if reopen_due(self.reopened, now) <= now {
                self.reopen_device();
            }
            if self.reopen_owed {
                let at = reopen_due(self.reopened, now);
                deadline = Some(deadline.map_or(at, |d| d.min(at)));
            }
        }
        for pane in &mut self.panes {
            // Not while the window waits for a device: with nothing to
            // present to there is no vsync to pace the frame, and each one
            // would only find the device still owed. Nor while it is
            // minimized or covered (`Pane::animates_now`, RG45, RG98).
            if pane.animates_now() {
                pane.redraw_for(FrameCause::OWED);
            }
            // A frame held for a display that stopped firing is drawn
            // anyway once it has waited too long (`mod pacer`).
            if let Some((at, due)) = pane.pacer.overdue(now) {
                if due {
                    pane.redraw_for(FrameCause::OVERDUE);
                } else {
                    deadline = Some(deadline.map_or(at, |d| d.min(at)));
                }
            }
            // A frame the surface skipped asks again — a retry apart, and
            // only so many times (`mod retry`).
            let (ask, wake) = pane.retry.poll(now);
            if ask {
                pane.redraw_for(FrameCause::RETRY);
            }
            if let Some(at) = wake {
                deadline = Some(deadline.map_or(at, |d| d.min(at)));
            }
            // A caret to blink: the stock editor's, or the `caret` a
            // custom editor declares on one of its lines (backlog C35).
            // None, or a solid one (`caret_solid`, F68): the phase is
            // parked on — focused window or not, since the clock owns
            // the phase only of a caret it blinks; what a solid caret
            // looks like without the keyboard (hollow, as a GUI editor's
            // block goes; dimmed; gone) is the view's own reading of
            // `env.focused`, which the hiding below is not a substitute
            // for (RG14 (j), in F68's entry). Caught mid-blink — Escape
            // from insert mode on the off phase — the frame that handled
            // the key read the phase off, so one more is asked for; once,
            // not a loop.
            if !pane.core.has_caret() {
                if !pane.blink_visible {
                    pane.blink_visible = true;
                    pane.core.set_caret_visible(true);
                    pane.redraw_for(FrameCause::CARET);
                }
                pane.blink_deadline = None;
                continue;
            }
            // A caret in a window that does not have the keyboard is not
            // blinking on any platform, and the blink is the one thing in
            // an idle app that asks for a frame twice a second forever: an
            // editor left in the background drew 120 frames a minute for a
            // caret nobody could type into. Hidden rather than parked
            // solid, because solid is what a *focused* field looks like.
            // The caret comes back with the keyboard: `blink_deadline` is
            // cleared here, and a cleared deadline is what the arm below
            // reads as "start blinking", so the first `about_to_wait`
            // after focus returns shows it and re-arms.
            if !pane.core.env.focused {
                if pane.blink_visible {
                    pane.blink_visible = false;
                    pane.core.set_caret_visible(false);
                    pane.redraw_for(FrameCause::CARET);
                }
                pane.blink_deadline = None;
                continue;
            }
            let stamp = pane.core.caret_stamp();
            if stamp != pane.caret_stamp_seen || pane.blink_deadline.is_none() {
                pane.caret_stamp_seen = stamp;
                pane.blink_deadline = Some(now + BLINK_INTERVAL);
                if !pane.blink_visible {
                    pane.blink_visible = true;
                    pane.core.set_caret_visible(true);
                    pane.redraw_for(FrameCause::CARET);
                }
            } else if now >= pane.blink_deadline.unwrap() {
                pane.blink_visible = !pane.blink_visible;
                pane.core.set_caret_visible(pane.blink_visible);
                pane.blink_deadline = Some(now + BLINK_INTERVAL);
                pane.redraw_for(FrameCause::CARET);
            }
            if let Some(d) = pane.blink_deadline {
                deadline = Some(deadline.map_or(d, |e| e.min(d)));
            }
        }
        if self.audio.active() {
            let poll = now + AUDIO_POLL;
            deadline = Some(deadline.map_or(poll, |d| d.min(poll)));
        }
        // An output device nothing has used for `AUDIO_IDLE_CLOSE` is let
        // go: it is a real-time thread the OS keeps calling, and it is what
        // an idle app that owns a sound spends its whole CPU on. The wake
        // this schedules is the point — under `ControlFlow::Wait` a truly
        // idle app is never called again, so without a deadline the close
        // would be scheduled and never run.
        if self.audio.holds_device() {
            if !self.audio.active() && self.audio_touch.elapsed() >= AUDIO_IDLE_CLOSE {
                self.audio.close();
            }
            if self.audio.holds_device() {
                // Either it is still in use (wake when the hold expires) or
                // the close found it mid-open (ask again shortly).
                let at = (self.audio_touch + AUDIO_IDLE_CLOSE).max(now + AUDIO_POLL);
                deadline = Some(deadline.map_or(at, |d| d.min(at)));
            }
        }
        // A batch that carried an event has left a redraw asked for, so the
        // shell wants pumping at once to present it — and a driver reading
        // this is also being told that the window is in use, which is the
        // one thing it cannot work out from the events *it* was handed.
        if std::mem::take(&mut self.saw_event) {
            deadline = Some(now);
            self.woke = true;
        }
        self.next_deadline = deadline;
        event_loop.set_control_flow(match deadline {
            Some(d) => ControlFlow::WaitUntil(d),
            None => ControlFlow::Wait,
        });
    }
}

// Re-exported so apps can reach the renderer without depending on kui-wgpu.
pub use kui_wgpu::{RenderError, Renderer, wgpu};

/// `KUI_LOSE_DEVICE=SECS`, read once: when after launch to pretend the
/// device was lost (`Shell::redraw`), or never.
fn lose_device_at() -> Option<f64> {
    static AT: std::sync::OnceLock<Option<f64>> = std::sync::OnceLock::new();
    *AT.get_or_init(|| std::env::var("KUI_LOSE_DEVICE").ok()?.parse().ok())
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Empty;
    impl App for Empty {
        fn view(&mut self, _ui: &mut Ui<'_>) {}
    }

    /// An app that borrows is still an app: the runner is compiled once
    /// against `Shell<dyn App + 'a>` (backlog C49), and neither `run` nor
    /// `open` asks for `'static`. Checked by the compiler alone: a loop
    /// cannot be built on a test's worker thread.
    #[test]
    fn an_app_that_borrows_still_runs() {
        struct Borrowing<'a>(&'a str);
        impl App for Borrowing<'_> {
            fn view(&mut self, ui: &mut Ui<'_>) {
                ui.text(self.0, TextStyle::new(12.0));
            }
        }
        let title = String::from("t");
        let _run = || app("t").run(Borrowing(&title));
        let _open = || {
            let mut runner = app("t").open(Borrowing(&title))?;
            let Borrowing(s) = runner.app_mut();
            assert_eq!(*s, "t");
            Ok::<_, Box<dyn std::error::Error>>(())
        };
    }

    /// `App::teardown` runs once, whichever of the runner's ends comes
    /// first and however many come after (backlog F74, tested under RG1):
    /// `retire` — what a pump returning false, `request_exit` and the
    /// runner's drop all reach — and `teardown_once` itself, which is what
    /// the loop's `exiting` calls. The runner is built without a loop
    /// here: winit builds its loop on the main thread only, and a test
    /// runs on a worker.
    #[test]
    fn teardown_runs_once_across_retire_and_drop() {
        use std::cell::Cell;
        use std::rc::Rc;
        struct Counting(Rc<Cell<u32>>);
        impl App for Counting {
            fn view(&mut self, _ui: &mut Ui<'_>) {}
            fn teardown(&mut self) {
                self.0.set(self.0.get() + 1);
            }
        }
        let count = Rc::new(Cell::new(0));
        let runner_of = |app: Counting| PumpRunner {
            state: PumpState {
                event_loop: None,
                alive: true,
                pumps: 0,
                woken_pumps: 0,
            },
            shell: std::mem::ManuallyDrop::new(super::app("t").diagnostics(false).shell(app)),
        };
        let mut runner = runner_of(Counting(count.clone()));
        assert_eq!(count.get(), 0, "nothing before the end");
        runner.request_exit();
        assert_eq!(count.get(), 0, "asking is not the end");
        runner.retire();
        assert_eq!(count.get(), 1, "retiring is");
        assert!(!runner.state.alive);
        runner.retire();
        runner.shell_mut().teardown_once();
        assert_eq!(count.get(), 1, "once, however many ends come after");
        drop(runner);
        assert_eq!(
            count.get(),
            1,
            "the drop retires again, and it is still once"
        );

        // The drop alone — a runner let go of while alive — is an end too.
        let count = Rc::new(Cell::new(0));
        drop(runner_of(Counting(count.clone())));
        assert_eq!(count.get(), 1);
    }

    /// A turn whose batch saw an event is counted once, and the flag is
    /// taken with it: left set, every quiet turn after the first event
    /// would count, and a test reading `woken_pumps` (backlog F94) would
    /// see the desktop in every idle second. Driven through `tally`, the
    /// step each pump ends with, since `about_to_wait` — which sets the
    /// flag — needs a live loop, and winit builds one on the main thread
    /// only.
    #[test]
    fn a_woken_turn_is_counted_once() {
        let mut runner = PumpRunner {
            state: PumpState {
                event_loop: None,
                alive: true,
                pumps: 0,
                woken_pumps: 0,
            },
            shell: std::mem::ManuallyDrop::new(super::app("t").diagnostics(false).shell(Empty)),
        };
        let tally = |r: &mut PumpRunner<Empty>| r.state.tally(&mut **r.shell);
        tally(&mut runner);
        assert_eq!(runner.woken_pumps(), 0, "a quiet turn is not woken");
        runner.shell_mut().woke = true;
        tally(&mut runner);
        assert_eq!(runner.woken_pumps(), 1);
        assert!(!runner.shell().woke, "taken by the turn that counted it");
        tally(&mut runner);
        tally(&mut runner);
        assert_eq!(
            runner.woken_pumps(),
            1,
            "and the quiet turns after it are quiet"
        );
    }

    /// A frame waits only for an answer that is actually owed, and never
    /// twice running — the bound that keeps a stream of input, or a
    /// platform modal loop the host cannot interrupt, from stopping the
    /// window altogether.
    /// A press and a release finish something the core drew; a pointer
    /// moving and a wheel turning do not, and a frame that waited on those
    /// would cost a drag half its frames.
    #[test]
    fn only_a_discrete_input_is_worth_waiting_for() {
        assert!(input_completes(&InputEvent::mouse_down(1)));
        assert!(input_completes(&InputEvent::mouse_up()));
        assert!(input_completes(&InputEvent::Text("x".into())));
        assert!(!input_completes(&InputEvent::CursorMoved(Vec2::ZERO)));
        assert!(!input_completes(&InputEvent::Scroll(Vec2::ZERO)));
        assert!(!input_completes(&InputEvent::CursorLeft));
    }

    #[test]
    fn a_frame_never_waits_twice_running() {
        assert!(frame_waits_for_host(true, false));
        assert!(!frame_waits_for_host(true, true));
        assert!(!frame_waits_for_host(false, false));
        assert!(!frame_waits_for_host(false, true));
    }

    /// The default is an app that answers inside `on_event`; only a host
    /// driving the loop itself opts out.
    #[test]
    fn deferred_events_is_opt_in() {
        assert!(!app("t").deferred_events);
        assert!(app("t").deferred_events().deferred_events);
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
            app("t").dyn_shell(Empty).core_mut().diagnostics(),
            cfg!(debug_assertions)
        );
        assert!(
            app("t")
                .diagnostics(true)
                .dyn_shell(Empty)
                .core_mut()
                .diagnostics()
        );
        assert!(
            !app("t")
                .diagnostics(false)
                .dyn_shell(Empty)
                .core_mut()
                .diagnostics()
        );
    }

    /// `Launcher::core` (backlog AR27): what the host registered on the
    /// core it hands over is the window's, its session is the app's, and
    /// what the launcher was told still lands on top, in order.
    #[test]
    fn a_handed_core_is_the_main_window_s_and_its_session_the_app_s() {
        let session = Session::new();
        let mut core = Core::new_in(&session);
        core.set_devtools(true);
        core.set_native_menus(false);
        let image = core.resources.add_image(1, 1, vec![0; 4]);
        let mut shell = app("t").diagnostics(false).core(core).dyn_shell(Empty);
        assert!(shell.session.is(&session), "the session came along");
        assert!(shell.core_mut().devtools(), "the devtools door held");
        assert!(!shell.core_mut().native_menus(), "so did the menus one");
        assert!(
            !shell.core_mut().diagnostics(),
            "the launcher's own setting lands after"
        );
        assert_eq!(
            shell.core_mut().resources.image_size(image),
            Some((1, 1)),
            "a resource the host registered draws in the window"
        );

        // `setup_core` runs on the handed core, last.
        let mut core = Core::new();
        core.set_devtools(true);
        let mut shell = app("t")
            .core(core)
            .setup_core(|c| c.set_devtools(false))
            .dyn_shell(Empty);
        assert!(!shell.core_mut().devtools());
    }

    /// `Launcher::devtools_key` is the `set_devtools_key` door in builder
    /// form: the chord lands on the main window's core before its first
    /// frame, and the default stands for an app that never asked.
    #[test]
    fn the_devtools_chord_is_the_launcher_s_to_respell() {
        let mut shell = app("t").diagnostics(false).dyn_shell(Empty);
        assert_eq!(shell.core_mut().devtools_key().spelling(), "ctrl+shift+i");
        let mut shell = app("t")
            .diagnostics(false)
            .devtools_key(Accel::parse("f12").unwrap())
            .dyn_shell(Empty);
        assert_eq!(shell.core_mut().devtools_key().spelling(), "f12");
    }

    #[test]
    fn launcher_clamps_the_initial_size_into_the_bounds() {
        let shell = app("t")
            .size(320.0, 240.0)
            .min_size(640.0, 480.0)
            .dyn_shell(Empty);
        assert_eq!(shell.size, (640.0, 480.0));
        assert_eq!(shell.min_size, Some((640.0, 480.0)));

        let shell = app("t")
            .size(1600.0, 1200.0)
            .max_size(800.0, 600.0)
            .dyn_shell(Empty);
        assert_eq!(shell.size, (800.0, 600.0));
        assert_eq!(shell.max_size, Some((800.0, 600.0)));

        // Builder order is irrelevant: the clamp happens once, at `shell`.
        let shell = app("t")
            .min_size(640.0, 480.0)
            .size(320.0, 240.0)
            .dyn_shell(Empty);
        assert_eq!(shell.size, (640.0, 480.0));
    }

    /// A device that will not open is tried once a second (RG29): the
    /// first ask runs at once, an ask inside the second after a try waits
    /// for the second to be up — the instant `about_to_wait` sleeps until,
    /// not a frame asked for every turn — and one after it runs at once.
    #[test]
    fn a_reopen_waits_out_the_second_after_the_last_try() {
        let now = std::time::Instant::now();
        assert_eq!(reopen_due(None, now), now, "the first try is at once");
        let recent = now - std::time::Duration::from_millis(300);
        assert_eq!(
            reopen_due(Some(recent), now),
            recent + REOPEN_INTERVAL,
            "inside the second: woken when it is up"
        );
        assert!(reopen_due(Some(recent), now) > now, "and not before");
        let old = now - std::time::Duration::from_secs(5);
        assert_eq!(reopen_due(Some(old), now), now, "past it: at once");
        let edge = now - REOPEN_INTERVAL;
        assert_eq!(reopen_due(Some(edge), now), now, "at it: at once");
    }

    /// A surface refused frame after frame while its reopen waits (RG30):
    /// retried `SURFACE_TRIES` times, then given up — said once — and the
    /// count saturates rather than wrapping back into retries or, in a
    /// debug build, panicking, however many frames input asks for.
    #[test]
    fn a_refused_surface_is_retried_then_given_up_once_and_the_count_saturates() {
        let mut tries = 0u8;
        let answers: Vec<Refused> = (0..1000).map(|_| surface_refused(&mut tries)).collect();
        let retries = SURFACE_TRIES as usize;
        assert!(answers[..retries].iter().all(|a| *a == Refused::Retry));
        assert_eq!(answers[retries], Refused::GiveUp { say: true });
        assert!(
            answers[retries + 1..]
                .iter()
                .all(|a| *a == Refused::GiveUp { say: false }),
            "given up, and said only the once"
        );
        assert_eq!(tries, u8::MAX);
    }

    /// Subpixel text follows the device it is drawn on (RG32): asked for
    /// or left to `Auto`, it is on only where the device blends per
    /// channel — so a reopen onto an adapter without dual-source blending
    /// turns it off — and grayscale asked for is grayscale everywhere.
    #[test]
    fn subpixel_text_is_decided_by_each_device() {
        assert!(subpixel_on(TextAa::Auto, true));
        assert!(!subpixel_on(TextAa::Auto, false));
        assert!(subpixel_on(TextAa::Subpixel, true));
        assert!(!subpixel_on(TextAa::Subpixel, false));
        assert!(!subpixel_on(TextAa::Grayscale, true));
        assert!(!subpixel_on(TextAa::Grayscale, false));
    }
}
