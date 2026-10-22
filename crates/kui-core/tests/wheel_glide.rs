//! A wheel's notch glides (ADR 0050, backlog F182): on a scroll container
//! a whole notch eases toward a target every further notch moves, the
//! drawn place chasing it with the speed it has, where pixels — a
//! trackpad's — and a fraction of a line are applied as they come.

use kui_core::{Core, InputEvent, Key, MotionPref, NodeSpec, Size, SystemEnv, UiEvent, Vec2};

const VIEW: Size = Size { w: 300.0, h: 200.0 };
const FRAME: f64 = 1.0 / 60.0;

fn wheel(core: &mut Core, lx: f32, ly: f32, begins: bool) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Wheel {
        lines: Vec2::new(lx, ly),
        begins,
    })
}

/// A 200 px tall list of 2000 px: 1800 px of travel.
fn list(core: &mut Core) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed(
        "list",
        NodeSpec::column().size(300.0, 200.0).scroll_y(),
        |ui| {
            ui.leaf(NodeSpec::column().size(300.0, 2000.0));
        },
    );
    ui.finish();
}

/// A core with the list laid out, the clock at 0 and the pointer over it.
fn setup() -> (Core, Key) {
    let mut core = Core::new();
    core.set_time(0.0);
    list(&mut core);
    list(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 100.0)));
    let key = core.key_of("list").unwrap();
    (core, key)
}

/// Whether a scroll offset is still easing — not the auto scrollbar's
/// fade, which a jump owes frames for too.
fn gliding(core: &Core) -> bool {
    core.owed().scroll
}

/// One frame `FRAME` after the last; the drawn offset it laid out.
fn tick(core: &mut Core, t: &mut f64, key: Key) -> f32 {
    *t += FRAME;
    core.set_time(*t);
    list(core);
    core.scroll_geometry(key).unwrap().offset.y
}

/// Frames until nothing is owed; the drawn offsets on the way.
fn settle(core: &mut Core, t: &mut f64, key: Key) -> Vec<f32> {
    let mut seen = Vec::new();
    for _ in 0..120 {
        seen.push(tick(core, t, key));
        if !gliding(core) {
            return seen;
        }
    }
    panic!("never settled: {seen:?}");
}

/// A notch down is part of the way after one frame, asks for the next,
/// and lands on its 40 px exactly.
#[test]
fn a_notch_glides_to_its_target() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    wheel(&mut core, 0.0, -1.0, true);
    assert_eq!(core.scroll_offset(key), Vec2::new(0.0, 40.0), "the target");
    let first = tick(&mut core, &mut t, key);
    assert!(
        first > 12.0 && first < 40.0,
        "part-way, fast first: {first}"
    );
    assert!(gliding(&core), "the glide owes the next frame");
    let seen = settle(&mut core, &mut t, key);
    assert_eq!(*seen.last().unwrap(), 40.0, "lands exactly: {seen:?}");
    assert!(
        seen.windows(2).all(|w| w[1] >= w[0]),
        "never back: {seen:?}"
    );
    let frames = seen.len() + 1;
    assert!(frames <= 15, "about 150 ms, not {frames} frames: {seen:?}");
}

/// Ten notches 100 ms apart: the drawn place never stalls between them
/// — a notch mid-glide adds to the target, not a restart from rest —
/// and ends at the ten notches.
#[test]
fn a_spun_wheel_keeps_moving() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    let mut seen = Vec::new();
    for i in 0..10 {
        wheel(&mut core, 0.0, -1.0, i == 0);
        for _ in 0..6 {
            seen.push(tick(&mut core, &mut t, key));
        }
    }
    assert!(
        seen.windows(2).all(|w| w[1] > w[0]),
        "always moving forward while notches come: {seen:?}"
    );
    seen.extend(settle(&mut core, &mut t, key));
    assert_eq!(*seen.last().unwrap(), 400.0);
}

/// Spun down, then up before landing: the glide turns and settles where
/// the notches add up to, never past the start.
#[test]
fn a_reversed_wheel_turns() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    wheel(&mut core, 0.0, -3.0, true);
    tick(&mut core, &mut t, key);
    tick(&mut core, &mut t, key);
    wheel(&mut core, 0.0, 2.0, false);
    let seen = settle(&mut core, &mut t, key);
    assert_eq!(*seen.last().unwrap(), 40.0, "{seen:?}");
    assert!(seen.iter().all(|&y| y >= 0.0), "{seen:?}");
}

/// A fraction of a line — a precision touchpad, a high-resolution wheel —
/// is applied on the next frame as pixels are; mid-glide it joins the
/// target instead of jumping the content back.
#[test]
fn a_fraction_of_a_line_is_not_eased() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    wheel(&mut core, 0.0, -0.25, true);
    assert_eq!(tick(&mut core, &mut t, key), 10.0);
    assert!(!gliding(&core));
    wheel(&mut core, 0.0, -1.0, false);
    let first = tick(&mut core, &mut t, key);
    assert!(first > 10.0 && first < 50.0, "{first}");
    wheel(&mut core, 0.0, -0.25, false);
    let next = tick(&mut core, &mut t, key);
    assert!(next > first && next < 60.0, "joined the glide: {next}");
    assert_eq!(*settle(&mut core, &mut t, key).last().unwrap(), 60.0);
}

/// The target stops at the end of the travel: spinning past it stores
/// nothing a reversed wheel would have to undo before the content moves.
#[test]
fn the_target_is_clamped_to_the_travel() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    wheel(&mut core, 0.0, -100.0, true);
    assert_eq!(core.scroll_offset(key).y, 1800.0);
    settle(&mut core, &mut t, key);
    wheel(&mut core, 0.0, 1.0, true);
    assert_eq!(core.scroll_offset(key).y, 1760.0, "one notch back moves");
}

/// A trackpad's pixels, a programmatic `set_scroll` and the scrollbar end
/// a glide from where the content is drawn.
#[test]
fn pixels_and_set_scroll_end_a_glide() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    wheel(&mut core, 0.0, -2.0, true);
    let at = tick(&mut core, &mut t, key);
    core.handle_input(InputEvent::ScrollGesture {
        delta: Vec2::new(0.0, -5.0),
        begins: true,
    });
    assert_eq!(core.scroll_offset(key).y, at + 5.0, "from the drawn place");
    assert_eq!(tick(&mut core, &mut t, key), at + 5.0);
    assert!(!gliding(&core), "the glide is over");

    wheel(&mut core, 0.0, -2.0, true);
    tick(&mut core, &mut t, key);
    core.set_scroll(key, Vec2::new(0.0, 500.0));
    assert_eq!(tick(&mut core, &mut t, key), 500.0, "a jump, not a glide");
    assert!(!gliding(&core));
}

/// Content that shrinks under a glide lands it at the new end.
#[test]
fn a_shrinking_list_clamps_the_glide() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    wheel(&mut core, 0.0, -10.0, true);
    tick(&mut core, &mut t, key);
    let short = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            "list",
            NodeSpec::column().size(300.0, 200.0).scroll_y(),
            |ui| {
                ui.leaf(NodeSpec::column().size(300.0, 300.0));
            },
        );
        ui.finish();
    };
    for _ in 0..60 {
        t += FRAME;
        core.set_time(t);
        short(&mut core);
        let y = core.scroll_geometry(key).unwrap().offset.y;
        assert!(y <= 100.0, "never past the new end: {y}");
        if !gliding(&core) {
            assert_eq!(y, 100.0);
            return;
        }
    }
    panic!("never settled");
}

/// Off, or under reduced motion, a notch jumps its 40 px on the next
/// frame as it always did.
#[test]
fn off_or_reduced_motion_jumps() {
    let (mut core, key) = setup();
    let mut t = 0.0;
    assert!(core.smooth_wheel(), "on by default");
    core.set_smooth_wheel(false);
    wheel(&mut core, 0.0, -1.0, true);
    assert_eq!(tick(&mut core, &mut t, key), 40.0);
    assert!(!gliding(&core));

    core.set_smooth_wheel(true);
    core.set_system(SystemEnv {
        motion: MotionPref::Reduced,
        ..Default::default()
    });
    wheel(&mut core, 0.0, -1.0, true);
    assert_eq!(tick(&mut core, &mut t, key), 80.0);
    assert!(!gliding(&core));
    assert!(core.smooth_wheel(), "the switch is unchanged");
}

/// A gesture is handed on by where the glide is going: a list whose
/// glide is headed for its end passes the next gesture to the scroller
/// around it at once.
#[test]
fn a_glide_headed_for_the_end_hands_the_next_gesture_on() {
    let mut core = Core::new();
    core.set_time(0.0);
    let page = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            "page",
            NodeSpec::column().size(300.0, 200.0).scroll_y(),
            |ui| {
                ui.with_keyed(
                    "list",
                    NodeSpec::column().size(300.0, 100.0).scroll_y(),
                    |ui| {
                        ui.leaf(NodeSpec::column().size(300.0, 140.0));
                    },
                );
                ui.leaf(NodeSpec::column().size(300.0, 600.0));
            },
        );
        ui.finish();
    };
    page(&mut core);
    page(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 50.0)));
    let list = core.key_of("list").unwrap();
    let page_k = core.key_of("page").unwrap();
    // One notch takes the list's whole 40 px of travel; its glide has
    // only begun.
    wheel(&mut core, 0.0, -1.0, true);
    core.set_time(FRAME);
    page(&mut core);
    let drawn = core.scroll_geometry(list).unwrap().offset.y;
    assert!(drawn < 40.0, "mid-glide: {drawn}");
    wheel(&mut core, 0.0, -1.0, true);
    assert_eq!(core.scroll_offset(page_k).y, 40.0, "the page's, at once");
    assert_eq!(core.scroll_offset(list).y, 40.0);
}

/// A handler hears a notch's pixels as an event, unsmoothed.
#[test]
fn a_handler_hears_the_pixels() {
    let mut core = Core::new();
    core.set_time(0.0);
    let term = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            "term",
            NodeSpec::column().size(300.0, 200.0).on_scroll("term"),
            |_| {},
        );
        ui.finish();
    };
    term(&mut core);
    term(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 100.0)));
    let out = wheel(&mut core, 0.0, -1.0, true);
    let dy = out
        .iter()
        .find(|e| e.event.name() == "scroll")
        .and_then(|e| e.data()?.get_float("dy"))
        .unwrap();
    assert_eq!(dy, -40.0);
}
