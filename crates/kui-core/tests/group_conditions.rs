//! Conditions on a hover group's state (backlog C106): a node in a
//! `hoverGroup` reads `group_hovered`, `group_pressed`, `group_focused` and
//! `group_focus_visible` of the group — a checkbox's mark rings while its
//! row holds the keyboard's focus, with no view code reading the row.

use kui_core::{Color, Cond, Core, NodeSpec, Size, Value, Vec2, testing};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn ring() -> Color {
    Color::hex(0xff0000ff)
}
fn lit() -> Color {
    Color::hex(0x0000ffff)
}
fn pressed() -> Color {
    Color::hex(0x00ff00ff)
}

/// A focusable row in the group "row" holding a mark in the same group,
/// and a plain button after it; returns the mark's colour.
fn frame(core: &mut Core) -> Color {
    let mut ui = core.frame(VIEW, 1.0);
    ui.leaf_keyed(
        "before",
        NodeSpec::row()
            .size(40.0, 20.0)
            .on_click(Value::str("b"))
            .label("Before"),
    );
    ui.with_keyed(
        "row",
        NodeSpec::row()
            .size(200.0, 30.0)
            .hover_group("row")
            .on_click(Value::str("toggle"))
            .label("Agree"),
        |ui| {
            ui.leaf_keyed(
                "mark",
                NodeSpec::row()
                    .size(16.0, 16.0)
                    .bg(Color::WHITE)
                    .hover_group("row")
                    .when(Cond::GroupHovered, |p| p.bg(lit()))
                    .when(Cond::GroupPressed, |p| p.bg(pressed()))
                    .when(Cond::GroupFocusVisible, |p| p.bg(ring())),
            );
        },
    );
    ui.finish();
    let quads = testing::solids(core);
    quads
        .iter()
        .find(|q| q.rect.w == 16.0)
        .expect("the mark")
        .color
}

#[test]
fn a_mark_rings_while_its_group_holds_visible_focus() {
    let mut core = Core::new();
    assert_eq!(frame(&mut core), Color::WHITE);
    // Tab onto "before", then onto the row.
    testing::tab(&mut core, false);
    frame(&mut core);
    testing::tab(&mut core, false);
    assert_eq!(frame(&mut core), ring(), "the row has the keyboard");
    // Focus moved by a click does not show, and the ring goes.
    testing::click_at(&mut core, 280.0, 100.0);
    testing::click_at(&mut core, 150.0, 35.0);
    frame(&mut core);
    assert_ne!(frame(&mut core), ring(), "pointer focus does not ring");
}

#[test]
fn a_mark_lights_while_its_group_is_hovered_and_pressed() {
    let mut core = Core::new();
    frame(&mut core);
    // Over the row's bare background, away from the mark.
    testing::hover(&mut core, Vec2::new(150.0, 35.0));
    assert_eq!(frame(&mut core), lit(), "a member under the pointer");
    core.handle_input(kui_core::InputEvent::MouseDown {
        button: kui_core::MouseButton::Primary,
        clicks: 1,
    });
    assert_eq!(frame(&mut core), pressed(), "a press held on a member");
}

#[test]
fn a_node_in_no_group_reads_no_group_state() {
    let mut core = Core::new();
    let view = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.leaf_keyed(
            "lone",
            NodeSpec::row()
                .size(16.0, 16.0)
                .bg(Color::WHITE)
                .on_click(Value::str("x"))
                .label("Lone")
                .when(Cond::GroupHovered, |p| p.bg(lit())),
        );
        ui.finish();
        testing::solids(core)[0].color
    };
    view(&mut core);
    testing::hover(&mut core, Vec2::new(8.0, 8.0));
    assert_eq!(view(&mut core), Color::WHITE);
}

#[test]
fn the_conditions_parse_in_either_spelling() {
    for name in [
        "group_focus_visible",
        "groupFocusVisible",
        "group_hovered",
        "groupPressed",
    ] {
        let v = Value::map([(name, Value::Bool(true))]);
        let cond = kui_core::rules::parse_cond(&v, None);
        assert!(cond.is_ok_and(|c| c.reads_state()), "{name}");
    }
}
