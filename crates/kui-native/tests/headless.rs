//! The runner with no window (ADR 0057, backlog C58): an app on the real
//! clock, paced by its wakes, input sent in from any thread, and its frames
//! kept as files when it asks — with no winit and no display.

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use kui_native::widgets;
use kui_native::{
    App, CapturePhase, Core, Event, EventKind, InputEvent, Message, MouseButton, NodeSpec,
    TextStyle, Ui, UiEvent, Vec2, WindowCommand, WindowEvents, WindowId,
};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Add,
    Shot,
    Resized,
}

#[derive(Default)]
struct Counter {
    count: u32,
    /// What each frame's view saw, in order.
    seen: Vec<u32>,
    /// Set from a thread `setup` started.
    remote: Arc<AtomicU32>,
    wake_thread: bool,
    copy: Option<String>,
    close_at: Option<u32>,
    frame_at: Option<f64>,
    resized: u32,
    shots: Vec<(CapturePhase, String, Option<String>)>,
    torn_down: u32,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        self.seen.push(self.count);
        ui.window_events(WindowEvents {
            on_resize: Some(Msg::Resized.into()),
            ..Default::default()
        });
        ui.with(NodeSpec::column().fill(), |ui| {
            widgets::button(ui, "Add", Msg::Add);
            let remote = self.remote.load(Ordering::SeqCst);
            ui.text(
                &format!("{} added, {remote} remote", self.count),
                TextStyle::new(12.0),
            );
        });
        if let Some(text) = self.copy.take() {
            ui.set_clipboard(text, None);
        }
        if let Some(at) = self.frame_at.take() {
            ui.request_frame_at(at);
        }
        if self.close_at == Some(self.count) {
            ui.window_command(WindowCommand::Close(WindowId::MAIN));
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.event == EventKind::Capture {
            if let Event::Capture(c) = ev.read() {
                self.shots.push((c.phase, c.path, c.error));
            }
            return;
        }
        match ev.message::<Msg>() {
            Some(Msg::Add) => self.count += 1,
            Some(Msg::Resized) => self.resized += 1,
            _ => {}
        }
    }

    fn setup(&mut self, waker: kui_native::Waker) {
        if !self.wake_thread {
            return;
        }
        let remote = self.remote.clone();
        std::thread::spawn(move || {
            std::thread::sleep(Duration::from_millis(30));
            remote.store(7, Ordering::SeqCst);
            waker.wake();
        });
    }

    fn teardown(&mut self) {
        self.torn_down += 1;
    }
}

fn open(app: Counter) -> kui_native::HeadlessRunner<Counter> {
    let mut r = kui_native::app("headless").open_headless(app, 320.0, 200.0, 2.0);
    r.core_mut().set_inspect(true);
    r
}

/// The middle of the node keyed `label` in the last frame.
fn middle(core: &Core, label: &str) -> Vec2 {
    let n = core
        .nodes()
        .into_iter()
        .find(|n| n.label.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no node {label}"));
    Vec2::new(n.rect.x + n.rect.w / 2.0, n.rect.y + n.rect.h / 2.0)
}

fn click(input: &kui_native::HeadlessInput, at: Vec2) {
    input.send(InputEvent::CursorMoved(at));
    input.send(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    input.send(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
}

/// The first turn draws the first frame; a turn with nothing to do draws
/// nothing, and says it is idle.
#[test]
fn the_first_turn_draws_and_an_idle_one_does_not() {
    let mut r = open(Counter::default());
    assert!(r.turn());
    assert_eq!(r.frames(), 1);
    // What the first frame made (the first `resize`) is answered.
    assert!(r.turn());
    let drawn = r.frames();
    assert!(r.turn());
    assert_eq!(r.frames(), drawn, "nothing owed, nothing drawn");
    assert!(r.idle());
    assert_eq!(r.next_deadline(), None);
    assert_eq!(r.size().0.w, 320.0);
}

/// Every input sent before a turn is handled before that turn's frame:
/// three clicks, one frame, which sees all three.
#[test]
fn input_sent_before_a_turn_is_all_handled_before_its_frame() {
    let mut r = open(Counter::default());
    r.turn();
    r.turn();
    let input = r.input();
    let at = middle(r.core_mut(), "Add");
    // From another thread, as a socket's reader would.
    let i = input.clone();
    std::thread::spawn(move || {
        for _ in 0..3 {
            click(&i, at);
        }
    })
    .join()
    .unwrap();
    let before = r.frames();
    assert!(r.turn());
    assert_eq!(r.app_mut().count, 3);
    assert_eq!(r.frames(), before + 1, "one frame for the three");
    assert_eq!(r.app_mut().seen.last(), Some(&3));
}

/// The waker `setup` was given wakes this runner: a thread's change is in
/// the frame its wake brings, and `turn_until` parks until it comes.
#[test]
fn a_wake_from_a_thread_setup_started_brings_a_frame() {
    let mut r = open(Counter {
        wake_thread: true,
        ..Default::default()
    });
    r.turn();
    r.turn();
    let before = r.frames();
    let t0 = Instant::now();
    let deadline = t0 + Duration::from_secs(5);
    while r.app_mut().remote.load(Ordering::SeqCst) == 0 || r.frames() == before {
        assert!(Instant::now() < deadline, "no wake within 5 s");
        r.turn_until(deadline);
    }
    assert!(t0.elapsed() < Duration::from_secs(5));
    assert!(
        r.core_mut()
            .nodes()
            .iter()
            .any(|n| n.text.as_deref() == Some("0 added, 7 remote")),
        "the frame after the wake shows the thread's change"
    );
}

/// A time the view asks for is the runner's next deadline, and the frame
/// comes when it does, on the real clock.
#[test]
fn a_time_the_view_asks_for_is_a_frame_then() {
    let mut r = open(Counter::default());
    r.turn();
    r.turn();
    let now = r.core_mut().now();
    r.app_mut().frame_at = Some(now + 0.08);
    r.waker().wake();
    r.turn();
    let at = r.next_deadline().expect("the asked time is the deadline");
    let before = r.frames();
    let t0 = Instant::now();
    while r.frames() == before {
        r.turn_until(t0 + Duration::from_secs(5));
    }
    assert!(
        Instant::now() + Duration::from_millis(5) >= at,
        "not before it"
    );
    assert!(t0.elapsed() < Duration::from_secs(2));
}

/// The clipboard is the runner's own: what the app copies is read back
/// here, and the system's is never written.
#[test]
fn a_copy_goes_to_the_runner_s_own_clipboard() {
    let mut r = open(Counter::default());
    r.turn();
    r.app_mut().copy = Some("copied in a run".into());
    r.waker().wake();
    r.turn();
    assert_eq!(r.clipboard(), "copied in a run");
    r.input().set_clipboard("from elsewhere");
    r.turn();
    assert_eq!(r.clipboard(), "from elsewhere");
}

/// A resize is the next frame's size, and the app hears it.
#[test]
fn a_resize_lays_out_the_next_frame_and_is_heard() {
    let mut r = open(Counter::default());
    r.turn();
    r.turn();
    let heard = r.app_mut().resized;
    r.input().resize(500.0, 300.0, 1.0);
    r.turn();
    r.turn();
    assert_eq!(r.size().0.w, 500.0);
    assert_eq!(r.size().1, 1.0);
    assert!(r.app_mut().resized > heard);
}

/// The main window's close ends the run and tears the app down, once;
/// a turn after it does nothing.
#[test]
fn the_main_window_s_close_ends_the_run_once() {
    let mut r = open(Counter {
        close_at: Some(1),
        ..Default::default()
    });
    r.turn();
    r.turn();
    click(&r.input(), middle(r.core_mut(), "Add"));
    // The close is applied with the frame that declared it: that turn
    // ends the run.
    assert!(!r.turn());
    assert!(r.ended());
    assert!(!r.turn());
    assert_eq!(r.app_mut().torn_down, 1);
    drop(r);
}

/// `run` parks between turns and returns once `close` is sent, from
/// another thread, the app torn down.
#[test]
fn run_returns_on_close_from_another_thread() {
    let torn = Arc::new(AtomicU32::new(0));
    struct Watch(Arc<AtomicU32>);
    impl App for Watch {
        fn view(&mut self, ui: &mut Ui<'_>) {
            ui.text("watching", TextStyle::new(12.0));
        }
        fn teardown(&mut self) {
            self.0.fetch_add(1, Ordering::SeqCst);
        }
    }
    let r = kui_native::app("run").open_headless(Watch(torn.clone()), 100.0, 100.0, 1.0);
    let input = r.input();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        input.close();
    });
    let t0 = Instant::now();
    r.run();
    assert!(t0.elapsed() < Duration::from_secs(5));
    assert_eq!(torn.load(Ordering::SeqCst), 1);
}

/// A capture asked between turns is served after the next frame and
/// answered with its message, as a window's is — or fails saying "no GPU"
/// on a machine without one.
#[test]
fn a_capture_is_written_after_the_next_frame_and_answered() {
    let dir = std::env::temp_dir().join(format!("kui-headless-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("shot.png");
    let t0 = Instant::now();
    let mut r = open(Counter::default());
    r.turn();
    r.core_mut()
        .capture_to(path.to_str().unwrap(), Some(Msg::Shot.into()));
    // The ask is a frame owed, paced as an animation's next one is: the
    // host waits for it rather than spinning, and the wait knows of it.
    assert!(r.next_deadline().is_some(), "the ask is a deadline");
    while r.app_mut().shots.is_empty() && t0.elapsed() < Duration::from_secs(10) {
        r.turn_until(t0 + Duration::from_secs(10));
    }
    let shots = &r.app_mut().shots;
    assert_eq!(shots.len(), 1, "{shots:?}");
    let (phase, at, error) = &shots[0];
    if *phase == CapturePhase::Failed {
        let e = error.as_deref().unwrap_or("");
        assert!(e.starts_with("no GPU"), "{e}");
        assert!(
            std::env::var_os("KUI_REQUIRE_GPU").is_none_or(|v| v.is_empty()),
            "KUI_REQUIRE_GPU is set, but {e}"
        );
        eprintln!("skipped: {e}");
        return;
    }
    assert_eq!(at, path.to_str().unwrap());
    assert!(
        t0.elapsed() < Duration::from_secs(5),
        "answered at the next frame, not at the wait's end: {:?}",
        t0.elapsed()
    );
    let png = std::fs::read(&path).unwrap();
    let px = kui_native::decode_image(&png).unwrap();
    assert_eq!((px.width, px.height), (640, 400), "physical px at scale 2");
    let _ = std::fs::remove_dir_all(&dir);
}

/// A recording runs on its own thread, as a window's does; the thread's
/// wake is this runner's, so the answer comes on the turn after the file
/// is written, with no turn spun for it.
#[test]
fn a_recording_is_written_on_its_thread_and_answered_on_its_wake() {
    let dir = std::env::temp_dir().join(format!("kui-headless-rec-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("rec.apng");
    let mut r = open(Counter::default());
    r.turn();
    r.core_mut()
        .record_to(path.to_str().unwrap(), Some(Msg::Shot.into()));
    let t0 = Instant::now();
    let until = t0 + Duration::from_secs(10);
    // A few frames recorded: clicks, each a frame.
    let input = r.input();
    let soon = || Instant::now() + Duration::from_millis(40);
    for _ in 0..3 {
        r.turn_until(soon());
        click(&input, middle(r.core_mut(), "Add"));
        r.turn_until(soon());
    }
    r.core_mut().stop_recording();
    while r.app_mut().shots.is_empty() && Instant::now() < until {
        r.turn_until(until);
    }
    let shots = &r.app_mut().shots;
    assert_eq!(shots.len(), 1, "{shots:?}");
    let (phase, at, error) = &shots[0];
    if *phase == CapturePhase::Failed {
        let e = error.as_deref().unwrap_or("");
        assert!(e.starts_with("no GPU"), "{e}");
        eprintln!("skipped: {e}");
        return;
    }
    assert_eq!(at, path.to_str().unwrap());
    assert!(std::fs::metadata(&path).unwrap().len() > 0);
    assert!(t0.elapsed() < Duration::from_secs(5), "{:?}", t0.elapsed());
    let _ = std::fs::remove_dir_all(&dir);
}
