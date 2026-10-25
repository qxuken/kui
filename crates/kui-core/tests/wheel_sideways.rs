//! A mouse wheel reaches what scrolls sideways (ADR 0050, amended
//! 2026-10-25, backlog F202): Shift turns a notch sideways, as a
//! browser's wheel does off macOS, and a plain notch over a scroller that
//! only goes sideways moves it while it has room, then hands on to the
//! page around it. A trackpad's pixels carry their own `x` and are left
//! as they come.

use kui_core::{Core, InputEvent, KeyMods, NodeSpec, Size, UiEvent, Value, Vec2};

const VIEW: Size = Size { w: 300.0, h: 200.0 };
/// The strip's travel: one notch (40 px) and a bit.
const STRIP_TRAVEL: f32 = 60.0;

/// A page that scrolls down, holding a strip that scrolls only sideways
/// (rows 0–80) and, with `both`, a box below it that scrolls both ways
/// (rows 80–180); `named`, a handler round the page that names Shift.
fn frame(core: &mut Core, named: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut outer = NodeSpec::column().fill();
    if named {
        outer = outer
            .on_scroll(Value::str("named"))
            .scroll_mods(KeyMods::NONE.with_shift());
    }
    ui.with_keyed("outer", outer, |ui| {
        ui.with_keyed(
            "page",
            NodeSpec::column().size(300.0, 200.0).scroll_y(),
            |ui| {
                ui.with_keyed(
                    "strip",
                    NodeSpec::row().size(300.0, 80.0).scroll_x(),
                    |ui| {
                        ui.leaf(NodeSpec::row().size(300.0 + STRIP_TRAVEL, 80.0));
                    },
                );
                ui.with_keyed(
                    "both",
                    NodeSpec::column().size(300.0, 100.0).scroll_x().scroll_y(),
                    |ui| {
                        ui.leaf(NodeSpec::column().size(900.0, 900.0));
                    },
                );
                ui.leaf(NodeSpec::column().size(300.0, 2000.0));
            },
        );
    });
    ui.finish();
}

fn setup(named: bool) -> Core {
    // `scroll_offset` reads a glide's target: where the notch put it.
    let mut core = Core::new();
    frame(&mut core, named);
    frame(&mut core, named);
    core
}

const STRIP: f32 = 40.0;
const BOTH: f32 = 130.0;

fn notch(core: &mut Core, y: f32, ly: f32, begins: bool) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, y)));
    core.handle_input(InputEvent::Wheel {
        lines: Vec2::new(0.0, ly),
        begins,
    })
}

fn at(core: &mut Core, name: &str) -> Vec2 {
    let key = core.key_of(name).unwrap();
    core.scroll_offset(key)
}

/// A plain notch down over the strip moves it right; the page stays.
#[test]
fn a_plain_notch_moves_a_sideways_only_scroller() {
    let mut core = setup(false);
    notch(&mut core, STRIP, -1.0, true);
    assert_eq!(at(&mut core, "strip"), Vec2::new(40.0, 0.0));
    assert_eq!(at(&mut core, "page"), Vec2::ZERO, "the page stays");
    // Up goes back toward the start.
    notch(&mut core, STRIP, 1.0, true);
    assert_eq!(at(&mut core, "strip"), Vec2::ZERO);
}

/// At its end the strip hands a new turn on to the page, as a list at
/// its end does; a turn already under way keeps its target.
#[test]
fn a_sideways_scroller_at_its_end_hands_on_to_the_page() {
    let mut core = setup(false);
    notch(&mut core, STRIP, -1.0, true);
    notch(&mut core, STRIP, -1.0, false);
    frame(&mut core, false);
    assert_eq!(at(&mut core, "strip").x, STRIP_TRAVEL, "to its end");
    notch(&mut core, STRIP, -1.0, false);
    assert_eq!(
        at(&mut core, "page"),
        Vec2::ZERO,
        "the turn keeps the strip"
    );
    frame(&mut core, false);
    notch(&mut core, STRIP, -1.0, true);
    assert_eq!(
        at(&mut core, "page"),
        Vec2::new(0.0, 40.0),
        "a new turn goes on"
    );
    assert_eq!(at(&mut core, "strip").x, STRIP_TRAVEL);
}

/// Over a box that scrolls both ways a plain notch goes down, as it
/// always did, and Shift turns it sideways.
#[test]
fn shift_turns_a_notch_sideways() {
    let mut core = setup(false);
    notch(&mut core, BOTH, -1.0, true);
    assert_eq!(at(&mut core, "both"), Vec2::new(0.0, 40.0));
    core.handle_input(InputEvent::Modifiers(KeyMods::NONE.with_shift()));
    notch(&mut core, BOTH, -1.0, true);
    assert_eq!(at(&mut core, "both"), Vec2::new(40.0, 40.0));
    assert_eq!(at(&mut core, "page"), Vec2::ZERO);
    // Over the strip too, and over nothing that goes sideways it moves
    // nothing: the page under it does not scroll down instead.
    notch(&mut core, STRIP, -1.0, true);
    assert_eq!(at(&mut core, "strip"), Vec2::new(40.0, 0.0));
    notch(&mut core, 190.0, -1.0, true);
    assert_eq!(at(&mut core, "page"), Vec2::ZERO);
}

/// A handler that names Shift hears the turn as it came, down.
#[test]
fn a_handler_naming_shift_hears_the_turn_unturned() {
    let mut core = setup(true);
    core.handle_input(InputEvent::Modifiers(KeyMods::NONE.with_shift()));
    let evs = notch(&mut core, BOTH, -1.0, true);
    let scroll = evs.iter().find_map(|e| e.scroll()).expect("heard");
    assert_eq!(scroll.delta.x, 0.0, "{scroll:?}");
    assert!(scroll.delta.y < 0.0, "{scroll:?}");
    assert_eq!(at(&mut core, "both"), Vec2::ZERO);
}

/// A trackpad's pixels down over the strip scroll the page: a swipe has
/// an `x` of its own for the strip.
#[test]
fn pixels_are_left_as_they_come() {
    let mut core = setup(false);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, STRIP)));
    core.handle_input(InputEvent::ScrollGesture {
        delta: Vec2::new(0.0, -30.0),
        begins: true,
    });
    assert_eq!(at(&mut core, "strip"), Vec2::ZERO);
    assert_eq!(at(&mut core, "page"), Vec2::new(0.0, 30.0));
}
