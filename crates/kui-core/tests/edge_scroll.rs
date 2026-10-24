//! The edge scroll's feel (ADR 0029, amended 2026-10-25): a held drag
//! in a scroller's edge zone scrolls at a speed from how deep it is, how
//! long it has been held there and a cap; a pointer pinned against the
//! window's edge is fully deep, which is what a full-screen window
//! needs; easing off restarts the ramp, leaving stops it, a press that
//! has not moved scrolls nothing, and the content's ends stop it. The
//! same feel drives an app's own drag through `Ui::edge_scroll`.

use kui_core::testing::{press, release};
use kui_core::{
    Core, EdgeScrollFeel, EditOptions, InputEvent, Key, NodeSpec, Size, TextStyle, Vec2,
};

const LINE_H: f32 = 20.0;
const WIN_W: f32 = 400.0;
const WIN_H: f32 = 600.0;
const FRAME: f64 = 1.0 / 60.0;
const S: EdgeScrollFeel = EdgeScrollFeel::STANDARD;

/// A window-filling scroller over a document 600 lines long — twelve
/// thousand px, more than the cap covers in the seconds a test holds.
fn document(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(WIN_W, WIN_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let text: String = (0..600).map(|i| format!("line {i}\n")).collect();
    let opts = EditOptions {
        multiline: true,
        style: TextStyle::new(14.0).line_height(LINE_H),
        ..EditOptions::default()
    };
    ui.with_keyed(
        "doc-scroll",
        NodeSpec::column().grow_width().height(WIN_H).scroll_y(),
        |ui| {
            ui.text_edit("doc", &text, &opts, NodeSpec::column().width(300.0));
        },
    );
    ui.finish();
    core.key_of("doc-scroll").unwrap()
}

/// A clocked core with the document laid out once, and its scroller.
fn setup() -> (Core, Key, f64) {
    let mut core = Core::new();
    core.set_time(1.0);
    let key = document(&mut core);
    (core, key, 1.0)
}

/// One frame a sixtieth later; the speed it scrolled at, px/s.
fn frame(core: &mut Core, key: Key, t: &mut f64) -> f32 {
    let before = core.scroll_offset(key).y;
    *t += FRAME;
    core.set_time(*t);
    document(core);
    (core.scroll_offset(key).y - before) / FRAME as f32
}

/// A caret drag from the middle of the window to `y`.
fn drag_to(core: &mut Core, y: f32) {
    press(core, Vec2::new(10.0, WIN_H / 2.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, y)));
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.5
}

#[test]
fn a_drag_held_at_the_bottom_edge_speeds_up_over_the_ramp_to_the_cap_and_no_further() {
    let (mut core, key, mut t) = setup();
    // Pinned against the window's bottom edge, which is the scroller's.
    drag_to(&mut core, WIN_H - 0.5);
    let mut speeds = Vec::new();
    for _ in 0..150 {
        speeds.push(frame(&mut core, key, &mut t));
        assert!(core.animating(), "held in the zone, it asks for frames");
    }
    assert!(near(speeds[0], S.speed_px_s), "at once: {}", speeds[0]);
    assert!(
        speeds.windows(2).all(|w| w[1] >= w[0] - 0.1),
        "it only speeds up: {speeds:?}"
    );
    // Half way up the ramp at 0.75 s: three and a half times.
    let half = speeds[45];
    assert!(near(half, S.speed_px_s * 3.5), "at 0.75 s: {half}");
    // At 1.5 s the ramp's top, which is the cap; held on, no faster.
    assert!(near(speeds[90], S.cap_px_s), "at 1.5 s: {}", speeds[90]);
    assert!(
        speeds.iter().all(|&v| v <= S.cap_px_s + 0.5),
        "never past the cap"
    );
    assert!(speeds[90..].iter().all(|&v| near(v, S.cap_px_s)));
    release(&mut core);
    frame(&mut core, key, &mut t);
    assert!(!core.animating());
}

#[test]
fn a_pointer_pinned_at_the_windows_edge_counts_as_full_depth() {
    // A few px short of the edge: the curve, well under full speed.
    let (mut core, key, mut t) = setup();
    drag_to(&mut core, WIN_H - 5.0);
    let short = frame(&mut core, key, &mut t);
    let want = S.speed_px_s * ((S.zone_px - 5.0) / S.zone_px).powi(2);
    assert!(near(short, want), "5 px short: {short}, want {want}");
    release(&mut core);
    // Against it — where a maximised window's pointer stops, a pixel
    // short of the container's last row — all of it.
    let (mut core, key, mut t) = setup();
    drag_to(&mut core, WIN_H - 1.0);
    let pinned = frame(&mut core, key, &mut t);
    assert!(near(pinned, S.speed_px_s), "pinned: {pinned}");
    release(&mut core);
}

#[test]
fn pulling_back_past_the_threshold_drops_the_speed_and_restarts_the_ramp() {
    let (mut core, key, mut t) = setup();
    drag_to(&mut core, WIN_H - 0.5);
    for _ in 0..60 {
        frame(&mut core, key, &mut t);
    }
    let held = frame(&mut core, key, &mut t);
    assert!(held > S.speed_px_s * 3.0, "a second in: {held}");
    // Back by less than the threshold: the curve slows it a little and
    // the ramp goes on.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, WIN_H - 6.0)));
    let eased = frame(&mut core, key, &mut t);
    assert!(eased > S.speed_px_s * 2.5, "within the threshold: {eased}");
    // Back by more: the curve's speed with no ramp, at once.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, WIN_H - 12.0)));
    let back = frame(&mut core, key, &mut t);
    let curve = S.speed_px_s * ((S.zone_px - 12.0) / S.zone_px).powi(2);
    assert!(near(back, curve), "pulled back: {back}, want {curve}");
    // And the ramp climbs again from there.
    let next = frame(&mut core, key, &mut t);
    assert!(next > back, "{next} after {back}");
    release(&mut core);
}

#[test]
fn leaving_the_zone_stops_scrolling() {
    let (mut core, key, mut t) = setup();
    drag_to(&mut core, WIN_H - 0.5);
    for _ in 0..30 {
        frame(&mut core, key, &mut t);
    }
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, WIN_H / 2.0)));
    assert_eq!(frame(&mut core, key, &mut t), 0.0);
    assert!(!core.animating(), "out of the zone nothing asks for frames");
    // Back in, the ramp starts from the bottom.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, WIN_H - 0.5)));
    assert!(near(frame(&mut core, key, &mut t), S.speed_px_s));
    release(&mut core);
}

#[test]
fn a_press_that_has_not_moved_never_scrolls() {
    let (mut core, key, mut t) = setup();
    // Pressed in the bottom zone, held still, then shaken inside the
    // click slop: a click, which scrolls nothing however long.
    press(&mut core, Vec2::new(10.0, WIN_H - 2.0));
    for _ in 0..10 {
        assert_eq!(frame(&mut core, key, &mut t), 0.0);
    }
    core.handle_input(InputEvent::CursorMoved(Vec2::new(11.0, WIN_H - 1.0)));
    for _ in 0..10 {
        assert_eq!(frame(&mut core, key, &mut t), 0.0);
    }
    assert!(!core.animating());
    // Out of the slop it is a drag, and scrolls.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(16.0, WIN_H - 1.0)));
    assert!(frame(&mut core, key, &mut t) > 0.0);
    release(&mut core);
}

#[test]
fn scrolling_stops_at_the_contents_ends() {
    let (mut core, key, mut t) = setup();
    // At the top already: a drag into the top zone moves nothing and
    // asks for nothing.
    drag_to(&mut core, 0.5);
    assert_eq!(frame(&mut core, key, &mut t), 0.0);
    assert!(!core.animating());
    // Down to the end at the cap, and stopped there.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, WIN_H - 0.5)));
    for _ in 0..600 {
        frame(&mut core, key, &mut t);
    }
    let end = core.scroll_geometry(key).unwrap().max_offset.y;
    assert_eq!(core.scroll_offset(key).y, end);
    assert_eq!(frame(&mut core, key, &mut t), 0.0);
    assert!(!core.animating(), "at the end, idle under the held pointer");
    release(&mut core);
}

/// A scroller the app drags in itself: `Ui::edge_scroll` in the view,
/// with the pointer and the area the app chose, returning what moved.
fn app_drag(core: &mut Core, pointer: Option<Vec2>) -> (Key, Option<Vec2>) {
    let mut ui = core.frame(Size::new(WIN_W, WIN_H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.with_keyed(
        "board",
        NodeSpec::column().grow_width().height(300.0).scroll_y(),
        |ui| {
            for i in 0..100 {
                ui.leaf_keyed(
                    &format!("card{i}"),
                    NodeSpec::row().grow_width().height(40.0),
                );
            }
        },
    );
    let moved = pointer.and_then(|p| {
        let area = ui.scroll_geometry(key)?.rect;
        ui.edge_scroll(key, p, area)
    });
    ui.finish();
    (key, moved)
}

#[test]
fn an_apps_own_drag_scrolls_by_the_same_feel() {
    let mut core = Core::new();
    let mut t = 1.0;
    core.set_time(t);
    let (key, _) = app_drag(&mut core, None);
    // 40 px past the scroller's bottom, in a window that goes on below
    // it: the tangent past the zone, then the ramp.
    let p = Vec2::new(10.0, 340.0);
    let mut steps = Vec::new();
    for _ in 0..30 {
        t += FRAME;
        core.set_time(t);
        let (_, moved) = app_drag(&mut core, Some(p));
        steps.push(moved.expect("in the zone it moves").y / FRAME as f32);
        assert!(core.animating(), "it asks for the next frame");
    }
    let past = S.speed_px_s * (1.0 + S.curve * 40.0 / S.zone_px);
    assert!(near(steps[0], past), "{} want {past}", steps[0]);
    assert!(steps[29] > steps[0] * 2.0, "the ramp: {}", steps[29]);
    let at = core.scroll_offset(key).y;
    // The middle: nothing, and no frame asked.
    t += FRAME;
    core.set_time(t);
    assert_eq!(app_drag(&mut core, Some(Vec2::new(10.0, 150.0))).1, None);
    assert_eq!(core.scroll_offset(key).y, at);
    // A frame with no call ends the drag's ramp: back at the edge it
    // starts from the bottom.
    t += FRAME;
    core.set_time(t);
    app_drag(&mut core, None);
    t += FRAME;
    core.set_time(t);
    let again = app_drag(&mut core, Some(p)).1.unwrap().y / FRAME as f32;
    assert!(near(again, past), "{again}");
    // The top end: nothing to move toward.
    let mut core = Core::new();
    app_drag(&mut core, None);
    assert_eq!(app_drag(&mut core, Some(Vec2::new(10.0, 1.0))).1, None);
}
