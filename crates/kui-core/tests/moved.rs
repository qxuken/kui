//! A node moved off its laid place (backlog C105): `dx` / `dy` move it and
//! its subtree after layout — drawn, hit and read there, on whole pixels,
//! the room it takes its place's and `onLayout` reporting the place — eased
//! by `transition`, and set by a `when` entry, so a button's face lifts on
//! hover and sinks on press with no view code.

use kui_core::{Color, Cond, Core, Easing, NodeSpec, Size, Transition, Value, Vec2, testing};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

/// A column of a 100×20 `face` (with a child `dot`) above a 100×20 `below`;
/// returns the quads' tops and lefts by colour: face, dot, below.
fn frame(core: &mut Core, face: NodeSpec) -> [(f32, f32); 3] {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed("face", face.size(100.0, 20.0).bg(Color::WHITE), |ui| {
        ui.leaf_keyed(
            "dot",
            NodeSpec::row().size(4.0, 4.0).bg(Color::hex(0xff0000ff)),
        );
    });
    ui.leaf_keyed(
        "below",
        NodeSpec::row().size(100.0, 20.0).bg(Color::hex(0x00ff00ff)),
    );
    ui.finish();
    let quads = testing::solids(core);
    let at = |c: Color| {
        let q = quads.iter().find(|q| q.color == c).expect("drawn");
        (q.rect.x, q.rect.y)
    };
    [
        at(Color::WHITE),
        at(Color::hex(0xff0000ff)),
        at(Color::hex(0x00ff00ff)),
    ]
}

#[test]
fn a_move_carries_the_subtree_and_leaves_the_room() {
    let mut core = Core::new();
    let [face, dot, below] = frame(&mut core, NodeSpec::column().dx(5.0).dy(-3.0));
    assert_eq!(face, (5.0, -3.0), "moved");
    assert_eq!(dot, (5.0, -3.0), "its subtree with it");
    assert_eq!(below, (0.0, 20.0), "the room it takes is its place's");
}

#[test]
fn a_move_is_on_whole_pixels_and_hit_where_drawn() {
    let mut core = Core::new();
    let face = || {
        NodeSpec::column()
            .dx(0.4)
            .dy(30.0)
            .on_click(Value::str("face"))
    };
    let [f, ..] = frame(&mut core, face());
    assert_eq!(f, (0.0, 30.0), "0.4 px rounds to the pixel it is on");
    frame(&mut core, face());
    // Where it is drawn now (y 30–50) is the face; where it was laid
    // (y 0–20) is not.
    let hit = testing::click_at(&mut core, 50.0, 40.0);
    assert!(hit.iter().any(|e| e.event.name() == "click"), "{hit:?}");
    let miss = testing::click_at(&mut core, 50.0, 10.0);
    assert!(miss.iter().all(|e| e.event.name() != "click"), "{miss:?}");
}

#[test]
fn on_layout_reports_the_place() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.leaf_keyed(
        "face",
        NodeSpec::row()
            .size(100.0, 20.0)
            .dy(12.0)
            .on_layout(Value::str("lay")),
    );
    ui.finish();
    let lay = core
        .take_pending_events()
        .into_iter()
        .find(|e| e.event.name() == "layout")
        .expect("laid");
    assert_eq!(lay.data().and_then(|d| d.get_f32("y")), Some(0.0));
}

/// The Ledger primary button's face: up a pixel under the pointer, down a
/// pixel while pressed, eased over 100 ms.
fn ledger(core: &mut Core, now: f64) -> f32 {
    core.set_time(now);
    let face = NodeSpec::column()
        .on_click(Value::str("go"))
        .label("Go")
        .transition_with(Transition::ms(100.0).easing(Easing::Linear))
        .when(Cond::Hovered, |p| p.dy(-2.0))
        .when(Cond::Pressed, |p| p.dy(2.0));
    frame(core, face)[0].1
}

#[test]
fn a_state_entry_moves_the_face_and_its_transition_eases_it() {
    let mut core = Core::new();
    assert_eq!(ledger(&mut core, 10.0), 0.0);
    testing::hover(&mut core, Vec2::new(50.0, 10.0));
    assert_eq!(ledger(&mut core, 10.0), 0.0, "the leg starts where it was");
    assert_eq!(ledger(&mut core, 10.05), -1.0, "halfway up");
    assert_eq!(ledger(&mut core, 10.2), -2.0, "lifted under the pointer");
    core.handle_input(kui_core::InputEvent::MouseDown {
        button: kui_core::MouseButton::Primary,
        clicks: 1,
    });
    ledger(&mut core, 10.2);
    assert_eq!(ledger(&mut core, 10.4), 2.0, "sunk while pressed");
    let w = core.take_warnings();
    assert!(w.is_empty(), "no entry refused: {w:?}");
}

#[test]
fn an_entry_that_sets_one_lane_keeps_the_other() {
    let mut core = Core::new();
    testing::hover(&mut core, Vec2::new(50.0, 10.0));
    let face = || {
        NodeSpec::column()
            .dx(6.0)
            .on_click(Value::str("go"))
            .when(Cond::Hovered, |p| p.dy(3.0))
    };
    frame(&mut core, face());
    testing::hover(&mut core, Vec2::new(56.0, 10.0));
    let [f, ..] = frame(&mut core, face());
    assert_eq!(f, (6.0, 3.0), "dx kept, dy set");
}
