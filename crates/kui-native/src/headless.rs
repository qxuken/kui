//! The runner with no window (ADR 0057): an [`App`] run on the real clock
//! and paced by its wakes, as a window runs it, drawing frames nobody sees
//! and keeping them as files when it asks ([`Core::capture_to`]).
//!
//! [`Launcher::open_headless`] builds one from the same launcher a window
//! is opened from — its core, its setup, its extensions, its pinned
//! [`SystemEnv`] — and hands back a [`HeadlessRunner`]. The app's
//! [`App::setup`] gets a [`Waker`] as a window's does, so the threads that
//! wake it wake this. Nothing of winit runs, so a machine with no display
//! (a box over ssh, a container) runs it; only a capture asks for a GPU.
//!
//! What a window gets from the OS, a headless runner is told:
//!
//! - **Input** through a [`HeadlessInput`], from any thread. Every input
//!   sent before a turn begins is handled before that turn's frame, so a
//!   host that sends a run of keys and then asks the app something reads
//!   the frame after all of them.
//! - **Its size and scale** at open, and through [`HeadlessInput::resize`].
//! - **Focus**: the window has the keyboard until
//!   [`HeadlessInput::focus`] says otherwise.
//! - **`env.system`**: only what the launcher pinned
//!   ([`Launcher::system`]). The OS is not asked, so a picture taken on one
//!   machine is the picture another takes.
//!
//! And what a window does to the OS, a headless runner keeps to itself:
//!
//! - **The clipboard** is the runner's own text ([`HeadlessRunner::clipboard`],
//!   [`HeadlessInput::set_clipboard`]): a run never writes the user's, and
//!   a paste reads what the run copied. A picture for the clipboard is
//!   dropped.
//! - **Sounds** are dropped, as with the `audio` feature off.
//! - **A file dialog** is answered at once with no files, as a cancel.
//! - **Other windows** a view declares are not opened; the main window's
//!   close ends the run.
//!
//! The caret does not blink: a frame is drawn for something the app did,
//! and two pictures of one state are the same picture.

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use crate::{
    App, Captures, Core, Extensions, FrameCause, FrameSample, InputEvent, Launcher, MenuAction,
    Session, Size, SystemEnv, UiEvent, WindowCommand, WindowEnv, WindowId,
};

/// How often a frame is drawn while something animates: there is no
/// display to pace it, so the rate one would.
const ANIMATION_HZ: f32 = 60.0;

/// What the other threads leave for the runner's next turn.
#[derive(Default)]
struct Inbox {
    woken: bool,
    wake_at: Option<Instant>,
    asks: Vec<Ask>,
}

/// One thing a [`HeadlessInput`] was asked, in the order asked.
enum Ask {
    Input(InputEvent),
    Resize(Size, f32),
    Focus(bool),
    Clipboard(String),
    Close,
}

/// The runner's inbox and the bell a thread rings when it leaves
/// something: what a headless [`Waker`] and a [`HeadlessInput`] hold.
#[derive(Default)]
pub(crate) struct Signal {
    inbox: Mutex<Inbox>,
    bell: Condvar,
}

impl Signal {
    fn with(&self, f: impl FnOnce(&mut Inbox)) {
        let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        f(&mut inbox);
        self.bell.notify_one();
    }

    pub(crate) fn wake(&self) {
        self.with(|i| i.woken = true);
    }

    pub(crate) fn wake_at(&self, at: Instant) {
        self.with(|i| i.wake_at = Some(i.wake_at.map_or(at, |w| w.min(at))));
    }

    fn ask(&self, ask: Ask) {
        self.with(|i| i.asks.push(ask));
    }

    /// The inbox, emptied, once it holds something or `until` has come
    /// (`None`: no limit). A wake for later is left in it until it is due.
    fn wait(&self, until: Option<Instant>) -> Inbox {
        let mut inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        loop {
            let now = Instant::now();
            let due = inbox.wake_at.is_some_and(|at| at <= now);
            if inbox.woken || due || !inbox.asks.is_empty() {
                break;
            }
            let until = match (until, inbox.wake_at) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (a, b) => a.or(b),
            };
            match until {
                Some(at) if at <= now => break,
                Some(at) => {
                    inbox = self
                        .bell
                        .wait_timeout(inbox, at - now)
                        .unwrap_or_else(|e| e.into_inner())
                        .0;
                }
                None => inbox = self.bell.wait(inbox).unwrap_or_else(|e| e.into_inner()),
            }
        }
        let now = Instant::now();
        let wake_at = inbox.wake_at.filter(|at| *at > now);
        let mut taken = std::mem::take(&mut *inbox);
        // A time asked for that has come is a wake; one still ahead stays.
        taken.woken |= taken.wake_at.is_some() && wake_at.is_none();
        inbox.wake_at = wake_at;
        taken
    }

    /// Whether something waits for the next turn.
    fn pending(&self) -> bool {
        let inbox = self.inbox.lock().unwrap_or_else(|e| e.into_inner());
        inbox.woken
            || !inbox.asks.is_empty()
            || inbox.wake_at.is_some_and(|at| at <= Instant::now())
    }

    /// [`Self::wait`] that does not wait.
    fn take(&self) -> Inbox {
        self.wait(Some(Instant::now()))
    }
}

/// A handle into a [`HeadlessRunner`] that any thread may hold: the input
/// a window would get from the OS, sent in. Cheap to clone; harmless once
/// the runner has ended (what nobody turns for is dropped with it).
///
/// Each call rings the runner, and its next turn handles everything sent
/// before it began, in the order sent, before it draws.
#[derive(Clone)]
pub struct HeadlessInput(Arc<Signal>);

impl HeadlessInput {
    /// One input, as the OS would have delivered it to the window: a key,
    /// a click, the pointer moving, text typed, the wheel.
    pub fn send(&self, ev: InputEvent) {
        self.0.ask(Ask::Input(ev));
    }

    /// The window's new size (logical px) and scale: the next frame lays
    /// out at it, and the app hears a `resize`.
    pub fn resize(&self, w: f32, h: f32, scale: f32) {
        self.0.ask(Ask::Resize(Size::new(w, h), scale));
    }

    /// Whether the window has the keyboard (`env.focused`). It has from the
    /// start.
    pub fn focus(&self, focused: bool) {
        self.0.ask(Ask::Focus(focused));
    }

    /// The runner's clipboard set to `text`, as if another app had copied
    /// it: what the app's next paste reads.
    pub fn set_clipboard(&self, text: impl Into<String>) {
        self.0.ask(Ask::Clipboard(text.into()));
    }

    /// Ends the run, as the main window's close button would: the app's
    /// [`App::teardown`] runs, and [`HeadlessRunner::run`] returns.
    pub fn close(&self) {
        self.0.ask(Ask::Close);
    }
}

/// The runner's state over its app, generic only in its last field, so the
/// runner is compiled once here and not again in every app crate (as
/// `Shell` is).
struct Headless<A: App + ?Sized> {
    signal: Arc<Signal>,
    extensions: Extensions,
    /// Kept for the run: the fonts, images and sounds the core draws from.
    _session: Session,
    core: Core,
    captures: Captures,
    viewport: Size,
    scale: f32,
    focused: bool,
    custom_chrome: bool,
    pinned_system: SystemEnv,
    clipboard: String,
    /// Origin of the frame clock.
    epoch: Instant,
    /// Why the next frame is owed, `NONE` when none is.
    owed: FrameCause,
    /// When the last frame was drawn, for an animation's pace.
    drawn_at: Option<Instant>,
    frames: u64,
    ended: bool,
    torn_down: bool,
    /// Last, so a `Box<Headless<A>>` unsizes to a `Box<DynHeadless>`.
    app: A,
}

type DynHeadless<'a> = Headless<dyn App + 'a>;

/// An app running with no window; see the [module docs](self). Made by
/// [`Launcher::open_headless`].
///
/// A host either hands it the thread ([`run`](Self::run), until the main
/// window's close or [`HeadlessInput::close`]) or turns it itself
/// ([`turn`](Self::turn), [`turn_until`](Self::turn_until)) and reads the
/// app and its core between turns.
///
/// ```rust,no_run
/// use kui_native::{App, InputEvent, Ui};
///
/// struct Hello;
/// impl App for Hello {
///     fn view(&mut self, ui: &mut Ui<'_>) {
///         ui.text("Hello", kui_native::TextStyle::new(24.0));
///     }
/// }
///
/// let mut runner = kui_native::app("Hello").open_headless(Hello, 360.0, 200.0, 2.0);
/// runner.core_mut().capture_to("hello.png", None);
/// runner.turn(); // the first frame, drawn, and written to hello.png
/// ```
pub struct HeadlessRunner<A: App> {
    /// Dropped by hand, unsized, so its drop glue is kui's.
    inner: std::mem::ManuallyDrop<Box<Headless<A>>>,
}

impl Launcher {
    /// Opens the app with no window: `w` by `h` logical px at `scale`, on
    /// the real clock, its frames drawn for nobody and kept as files when
    /// it asks. The launcher's core, setup, diagnostics, extensions, chrome
    /// and pinned `env.system` apply as they would to a window; its title,
    /// icon, size bounds and backdrop have nothing to apply to, and
    /// `KUI_DEVTOOLS` does not open the panel. [`App::setup`] runs
    /// now, with a [`Waker`] for this runner.
    pub fn open_headless<A: App>(
        mut self,
        app: A,
        w: f32,
        h: f32,
        scale: f32,
    ) -> HeadlessRunner<A> {
        let (_diagnostics, session, core) = self.main_core_for(false);
        let signal = Arc::new(Signal::default());
        let mut inner = Box::new(Headless {
            signal: signal.clone(),
            extensions: std::mem::take(&mut self.extensions),
            _session: session,
            core,
            captures: Captures::new(),
            viewport: Size::new(w, h),
            scale,
            focused: true,
            custom_chrome: self.chrome != crate::Chrome::Native,
            pinned_system: self.system,
            clipboard: String::new(),
            epoch: Instant::now(),
            owed: FrameCause::FIRST,
            drawn_at: None,
            frames: 0,
            ended: false,
            torn_down: false,
            app,
        });
        // A recording is written on a thread of its own, as a window's is:
        // the frame pays a draw and a submit, and the thread rings when the
        // file is written, for the turn that answers.
        let bell = signal.clone();
        inner.captures.write_on_thread(move || bell.wake());
        inner.app.setup(crate::Waker::headless(signal));
        HeadlessRunner {
            inner: std::mem::ManuallyDrop::new(inner),
        }
    }
}

impl<A: App> HeadlessRunner<A> {
    fn dyn_mut(&mut self) -> &mut DynHeadless<'_> {
        &mut **self.inner
    }

    fn dyn_ref(&self) -> &DynHeadless<'_> {
        &**self.inner
    }

    /// One turn, not waiting: everything sent in since the last one
    /// handled, then the frame, if one is owed — by input, a wake, a time
    /// asked for, an animation, the events the last frame made — and what
    /// it asked for done. Returns false once the run has ended, and from
    /// then on does nothing.
    pub fn turn(&mut self) -> bool {
        let s = self.dyn_mut();
        let inbox = s.signal.take();
        s.turn(inbox)
    }

    /// [`turn`](Self::turn), once there is something to turn for — input,
    /// a wake, a time asked for, the runner's own next deadline — or
    /// `deadline` has come, whichever is first.
    pub fn turn_until(&mut self, deadline: Instant) -> bool {
        let s = self.dyn_mut();
        let until = s.deadline().map_or(deadline, |d| d.min(deadline));
        let inbox = s.signal.wait(Some(until));
        s.turn(inbox)
    }

    /// Turns until the run ends: the main window's close, a
    /// [`HeadlessInput::close`]. Parks between turns until there is
    /// something to turn for. The app's [`App::teardown`] has run when it
    /// returns.
    pub fn run(mut self) {
        let s = self.dyn_mut();
        loop {
            let inbox = s.signal.wait(s.deadline());
            if !s.turn(inbox) {
                break;
            }
        }
    }

    /// A [`HeadlessInput`] for this runner, to clone into the threads the
    /// input comes from.
    pub fn input(&self) -> HeadlessInput {
        HeadlessInput(self.inner.signal.clone())
    }

    /// A [`Waker`] for this runner, as [`App::setup`] was given.
    pub fn waker(&self) -> crate::Waker {
        crate::Waker::headless(self.inner.signal.clone())
    }

    /// When the runner next has something to do on its own: now, when a
    /// frame is owed; an animation's next frame, or a frame the core was
    /// asked for (a capture, `request_frame`), at a display's pace; a time
    /// the view asked for. Read from the core as it is now, so an ask made
    /// between turns through [`core_mut`](Self::core_mut) is in it. `None`
    /// when it waits for input or a wake — a
    /// [`Waker::wake_at`](crate::Waker::wake_at) among them.
    pub fn next_deadline(&self) -> Option<Instant> {
        self.dyn_ref().deadline()
    }

    /// Whether a turn now would do nothing: no frame owed now and nothing
    /// sent in waiting. Something asked for later still may be.
    pub fn idle(&self) -> bool {
        !self.inner.signal.pending() && self.dyn_ref().deadline().is_none_or(|d| d > Instant::now())
    }

    /// The frames drawn so far.
    pub fn frames(&self) -> u64 {
        self.inner.frames
    }

    /// Whether the run has ended.
    pub fn ended(&self) -> bool {
        self.inner.ended
    }

    /// The window's size (logical px) and scale.
    pub fn size(&self) -> (Size, f32) {
        (self.inner.viewport, self.inner.scale)
    }

    /// The runner's clipboard: what the app last copied, or what
    /// [`HeadlessInput::set_clipboard`] put there.
    pub fn clipboard(&self) -> &str {
        &self.inner.clipboard
    }

    /// The app, for a host that reads or changes its state between turns.
    /// A change that `view` should show wants a wake ([`Self::waker`]).
    pub fn app_mut(&mut self) -> &mut A {
        &mut self.inner.app
    }

    /// The main window's core, between turns: to read the last frame
    /// (`nodes`, [`capture`](crate::capture)), or to ask for a capture.
    pub fn core_mut(&mut self) -> &mut Core {
        &mut self.inner.core
    }
}

impl<A: App> Drop for HeadlessRunner<A> {
    /// A runner dropped before its run ended — a host that stopped turning
    /// it — still tears its app down, once.
    fn drop(&mut self) {
        self.dyn_mut().end();
        // SAFETY: taken once, here, and the field is not read again.
        let inner: Box<Headless<A>> = unsafe { std::mem::ManuallyDrop::take(&mut self.inner) };
        drop_unsized(inner);
    }
}

fn drop_unsized(inner: Box<DynHeadless<'_>>) {
    drop(inner);
}

impl DynHeadless<'_> {
    fn now(&self) -> f64 {
        self.epoch.elapsed().as_secs_f64()
    }

    fn owe(&mut self, why: FrameCause) {
        self.owed |= why;
    }

    fn turn(&mut self, inbox: Inbox) -> bool {
        if self.ended {
            return false;
        }
        // The runner woke, and why — in a recording (ADR 0058, decision 5).
        if self.core.perf_measuring() {
            let by = if inbox.woken {
                "wake"
            } else if !inbox.asks.is_empty() {
                "input"
            } else {
                "timer"
            };
            self.core.note_loop(kui_core::LoopNote::Woke(by));
        }
        let alive = self.turn_inner(inbox);
        if alive && self.core.perf_measuring() {
            let for_ms = self
                .deadline()
                .map(|d| d.saturating_duration_since(Instant::now()).as_secs_f64() * 1e3);
            self.core.note_loop(kui_core::LoopNote::Sleep { for_ms });
        }
        alive
    }

    fn turn_inner(&mut self, inbox: Inbox) -> bool {
        if inbox.woken {
            // A named waker's wake reads by its name (ADR 0058, decision 8).
            for name in crate::take_wake_names() {
                self.core.note_wake(&name);
            }
            self.app.on_wake();
            self.owe(FrameCause::WAKE);
        }
        for ask in inbox.asks {
            match ask {
                Ask::Input(ev) => self.dispatch(ev),
                Ask::Resize(viewport, scale) => {
                    let why = if scale != self.scale {
                        FrameCause::SCALE
                    } else {
                        FrameCause::RESIZE
                    };
                    (self.viewport, self.scale) = (viewport, scale);
                    self.owe(why);
                }
                Ask::Focus(focused) => {
                    self.focused = focused;
                    self.owe(FrameCause::FOCUS);
                }
                Ask::Clipboard(text) => self.clipboard = text,
                Ask::Close => {
                    self.end();
                    return false;
                }
            }
            if self.ended {
                return false;
            }
        }
        let now = Instant::now();
        // A time the view asked for, on the frame clock.
        if let Some(at) = self.view_wants_at()
            && at <= now
        {
            self.owe(FrameCause::WAKE);
        }
        // An animation, at a display's pace.
        let gap = Duration::from_secs_f32(1.0 / ANIMATION_HZ);
        if self.core.animating() && self.drawn_at.is_none_or(|t| now >= t + gap) {
            self.owe(FrameCause::OWED);
        }
        if self.owed != FrameCause::NONE {
            self.frame();
            self.after_frame();
        }
        !self.ended
    }

    /// The frame clock's `next_frame_at` as an instant.
    fn view_wants_at(&self) -> Option<Instant> {
        let secs = self.core.next_frame_at().filter(|s| s.is_finite())?;
        Some(self.epoch + Duration::from_secs_f64(secs.max(0.0)))
    }

    fn deadline(&self) -> Option<Instant> {
        if self.ended {
            return None;
        }
        if self.owed != FrameCause::NONE {
            return Some(Instant::now());
        }
        let animation = self.core.animating().then(|| {
            let gap = Duration::from_secs_f32(1.0 / ANIMATION_HZ);
            self.drawn_at.map_or_else(Instant::now, |t| t + gap)
        });
        match (animation, self.view_wants_at()) {
            (Some(a), Some(b)) => Some(a.min(b)),
            (a, b) => a.or(b),
        }
    }

    /// One input to the core, its events to the app, and what they asked
    /// for done.
    fn dispatch(&mut self, ev: InputEvent) {
        let why = cause_of(&ev);
        let t0 = Instant::now();
        let now = self.now();
        self.core.set_time(now);
        let events = self.core.handle_input(ev);
        self.route(events);
        self.apply_asks();
        self.core.stats.pending_input_ms += t0.elapsed().as_secs_f32() * 1e3;
        self.owe(why);
    }

    fn route(&mut self, events: Vec<UiEvent>) -> bool {
        let now = self.now();
        self.core.set_time(now);
        let mut reached = false;
        let Headless {
            extensions,
            app,
            core,
            ..
        } = self;
        extensions.route(events, |ev| {
            reached = true;
            // Timed as `handler` in a recording (ADR 0058, decision 5).
            let span = core.span_begin(kui_core::kui_trace::analysis::HANDLER);
            app.on_event_with(ev, core);
            core.span_end(span);
        });
        reached
    }

    /// Builds and draws one frame, and serves what the app asked to keep
    /// of it.
    fn frame(&mut self) {
        let why = std::mem::replace(&mut self.owed, FrameCause::NONE);
        let now = self.now();
        let core = &mut self.core;
        core.env.refresh_hz = Some(ANIMATION_HZ);
        core.env.focused = self.focused;
        core.env.system = self.pinned_system.over(SystemEnv::default());
        core.env.window = WindowEnv {
            id: WindowId::MAIN,
            custom_chrome: self.custom_chrome,
            ..WindowEnv::default()
        };
        core.set_time(now);
        core.note_frame_cause(why);
        let t_view = Instant::now();
        let mut ui = core.frame_with(self.viewport, self.scale, &mut self.extensions);
        self.app.view(&mut ui);
        let view_ms = t_view.elapsed().as_secs_f32() * 1e3;
        let t_layout = Instant::now();
        ui.finish();
        let layout_ms = t_layout.elapsed().as_secs_f32() * 1e3;
        for w in self.core.take_warnings() {
            eprintln!(
                "kui: warning [{}] node {:016x}: {}",
                w.code, w.key.0, w.message
            );
        }
        // Drawn: what a renderer would read is read, so the frame's atlas
        // changes are the last ones and a capture finds the frame finished.
        let _ = self.core.output();
        let t_render = Instant::now();
        self.captures.serve(&mut self.core);
        self.core.push_frame_sample(FrameSample {
            input_ms: 0.0,
            view_ms,
            layout_ms,
            render_ms: t_render.elapsed().as_secs_f32() * 1e3,
            wait_ms: 0.0,
        });
        self.frames += 1;
        self.drawn_at = Some(Instant::now());
    }

    /// What follows a frame: what it asked for done, and the events it made
    /// — a resize, a hover, a capture's answer — to the app, with the frame
    /// they change owed.
    fn after_frame(&mut self) {
        self.apply_asks();
        let pending = self.core.take_pending_events();
        if !pending.is_empty() && self.route(pending) {
            self.owe(FrameCause::AFTER_FRAME);
            self.apply_asks();
        }
    }

    /// The core's asks of its driver, answered as a window with no OS
    /// behind it answers them.
    fn apply_asks(&mut self) {
        for action in self.core.take_menu_actions() {
            match action {
                MenuAction::SetClipboard { text, .. } => self.clipboard = text,
                MenuAction::SetClipboardSecret { text } => self.clipboard = text,
                MenuAction::SetClipboardImage { .. }
                | MenuAction::LookUp { .. }
                | MenuAction::ShowInFolder { .. } => {}
                MenuAction::Paste => {
                    let text = self.clipboard.clone();
                    self.dispatch(InputEvent::Paste {
                        text,
                        marks: Default::default(),
                    });
                }
            }
        }
        for _ in self.core.take_file_requests() {
            self.dispatch(InputEvent::Files(Vec::new()));
        }
        let _ = self.core.take_audio_commands();
        for cmd in self.core.take_window_commands() {
            if matches!(cmd, WindowCommand::Close(id) if id == WindowId::MAIN) {
                self.end();
            }
        }
    }

    /// The run over: the app torn down, once.
    fn end(&mut self) {
        self.ended = true;
        if !self.torn_down {
            self.torn_down = true;
            self.app.teardown();
            let _ = self.core.perf_flush();
        }
    }
}

/// The reason an input gives its frame, as the window's would.
fn cause_of(ev: &InputEvent) -> FrameCause {
    match ev {
        InputEvent::CursorMoved(_) => FrameCause::POINTER_MOVE,
        InputEvent::CursorLeft => FrameCause::POINTER_LEAVE,
        InputEvent::MouseDown { .. } | InputEvent::MouseUp { .. } => FrameCause::BUTTON,
        InputEvent::Scroll(_) | InputEvent::ScrollGesture { .. } | InputEvent::Wheel { .. } => {
            FrameCause::WHEEL
        }
        InputEvent::Text(_) | InputEvent::Commit(_) | InputEvent::Paste { .. } => FrameCause::TEXT,
        InputEvent::Preedit(..) => FrameCause::PREEDIT,
        InputEvent::Modifiers(_) => FrameCause::MODIFIERS,
        InputEvent::DragFiles { .. } | InputEvent::DropFiles { .. } | InputEvent::DragCancel => {
            FrameCause::FILE_DRAG
        }
        InputEvent::Files(_) | InputEvent::Open(_) => FrameCause::FILES,
        InputEvent::Access(_) => FrameCause::ACCESS,
        _ => FrameCause::KEY,
    }
}
