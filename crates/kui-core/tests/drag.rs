//! Dragging through a live `Core`: on_drag nodes (pointer capture, click
//! suppression, payload shape) and scrollbar thumb/track interaction.

use kui_core::{Core, InputEvent, NodeSpec, Size, Sizing, UiEvent, Value, Vec2};

fn drag_frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    // A 100px left panel, a draggable 10px divider, the rest.
    ui.with(
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Grow(1.0)),
        |_| {},
    );
    ui.with_keyed(
        "divider",
        NodeSpec::column()
            .width(Sizing::Fixed(10.0))
            .height(Sizing::Grow(1.0))
            .on_click(Value::str("clicked"))
            .on_drag(Value::str("split")),
        |_| {},
    );
    ui.finish();
}

fn phases(evs: &[UiEvent]) -> Vec<String> {
    evs.iter()
        .filter(|e| e.payload.get("kind").and_then(Value::as_str) == Some("drag"))
        .map(|e| {
            e.payload
                .get("phase")
                .and_then(Value::as_str)
                .unwrap_or("?")
                .to_string()
        })
        .collect()
}

#[test]
fn drag_emits_start_move_end_with_geometry() {
    let mut core = Core::new();
    drag_frame(&mut core);

    let mut all = Vec::new();
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0))));
    all.extend(core.handle_input(InputEvent::mouse_down(1)));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 55.0))));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(160.0, 55.0))));
    all.extend(core.handle_input(InputEvent::mouse_up()));

    assert_eq!(phases(&all), ["start", "move", "move", "end"]);
    let mv = &all[1].payload;
    assert_eq!(mv.get("x").and_then(Value::as_float), Some(150.0));
    assert_eq!(mv.get("dx").and_then(Value::as_float), Some(45.0));
    assert_eq!(mv.get("tag").and_then(Value::as_str), Some("split"));
    // Parent rect: the root, i.e. the whole viewport.
    let parent = mv.get("parent").unwrap();
    assert_eq!(parent.get("w").and_then(Value::as_float), Some(400.0));
    assert_eq!(parent.get("h").and_then(Value::as_float), Some(300.0));
    // The real drag suppressed the click.
    assert!(!all.iter().any(|e| e.payload.as_str() == Some("clicked")));
}

#[test]
fn undragged_press_still_clicks() {
    let mut core = Core::new();
    drag_frame(&mut core);

    let mut all = Vec::new();
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0))));
    all.extend(core.handle_input(InputEvent::mouse_down(1)));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(106.0, 50.0)))); // sub-slop
    all.extend(core.handle_input(InputEvent::mouse_up()));

    assert_eq!(
        phases(&all),
        ["start", "end"],
        "sub-slop motion emits no move"
    );
    assert!(
        all.iter().any(|e| e.payload.as_str() == Some("clicked")),
        "a press that never left the slop is still a click"
    );
}

/// A scroll container whose content overflows by a lot.
fn scroll_frame(core: &mut Core) -> kui_core::Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.with_keyed(
        "list",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(100.0))
            .scroll_y(),
        |ui| {
            for _ in 0..20 {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Fixed(30.0)),
                    |_| {},
                );
            }
        },
    );
    ui.finish();
    key
}

#[test]
fn hover_keeps_tracking_other_nodes_during_a_drag() {
    // Reorderable tabs lean on this: while one node drags, hover still
    // follows the cursor onto its siblings.
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        for label in ["a", "b"] {
            ui.with_keyed(
                label,
                NodeSpec::column()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(30.0))
                    .on_drag(Value::str(label)),
                |_| {},
            );
        }
        ui.finish();
    };
    frame(&mut core);
    let (a, b) = {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        let keys = (ui.child_key("a"), ui.child_key("b"));
        ui.finish();
        keys
    };
    frame(&mut core);

    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 15.0)));
    core.handle_input(InputEvent::mouse_down(1));
    assert!(core.interaction.is_hovered(a));
    // Drag over the sibling: the drag stays captured on `a`, hover moves.
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 15.0)));
    assert!(
        core.interaction.is_hovered(b),
        "hover follows the cursor mid-drag"
    );
    assert!(
        evs.iter()
            .any(|e| e.key == a && e.payload.get("phase").and_then(Value::as_str) == Some("move")),
        "drag events keep landing on the pressed node"
    );
    core.handle_input(InputEvent::mouse_up());
}

#[test]
fn scrollbar_thumb_drags_the_offset() {
    let mut core = Core::new();
    let key = scroll_frame(&mut core);
    assert_eq!(core.scroll.offset(key), Vec2::ZERO);

    // Grab the thumb (top of the right gutter) and pull it down.
    let evs: Vec<UiEvent> = [
        InputEvent::CursorMoved(Vec2::new(395.0, 10.0)),
        InputEvent::mouse_down(1),
        InputEvent::CursorMoved(Vec2::new(395.0, 60.0)),
    ]
    .into_iter()
    .flat_map(|ev| core.handle_input(ev))
    .collect();
    assert!(
        evs.is_empty(),
        "scrollbar interaction emits no UiEvents, got {evs:?}"
    );
    let dragged = core.scroll.offset(key).y;
    assert!(
        dragged > 100.0,
        "thumb drag should scroll a large fraction, got {dragged}"
    );

    // Release ends the drag: further motion does nothing.
    core.handle_input(InputEvent::mouse_up());
    core.handle_input(InputEvent::CursorMoved(Vec2::new(395.0, 90.0)));
    assert_eq!(core.scroll.offset(key).y, dragged);
}

#[test]
fn scrollbar_track_press_jumps() {
    let mut core = Core::new();
    let key = scroll_frame(&mut core);

    // Press the bottom of the track, far from the thumb.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(395.0, 95.0)));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::mouse_up());
    let max = 20.0 * 30.0 - 100.0;
    let y = core.scroll.offset(key).y;
    assert!(
        y > max * 0.8,
        "track press near the end should jump most of the way (max {max}), got {y}"
    );
}

/// A press on an `on_drag` node emits a drag start, which hosts often
/// answer by adding hoverable nodes; those must not sit over the pressed
/// node or the release stops being a click (the pressed and hovered keys
/// differ). Pins the tab-bar pattern: a hover column hanging *below* it.
#[test]
fn hoverable_added_below_a_pressed_node_keeps_the_click() {
    let mut core = Core::new();
    let frame = |core: &mut Core, with_column: bool| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "tab",
            NodeSpec::row()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(30.0))
                .on_click(Value::str("clicked"))
                .on_drag(Value::str("lift")),
            |ui| {
                if with_column {
                    ui.with_keyed(
                        "col",
                        NodeSpec::column()
                            .float(
                                kui_core::FloatConfig::parent()
                                    .at(kui_core::Align::Start, kui_core::Align::End),
                            )
                            .width(Sizing::Percent(1.0))
                            .height(Sizing::Fixed(300.0))
                            .hoverable(),
                        |_| {},
                    );
                }
            },
        );
        ui.finish();
    };
    frame(&mut core, false);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 15.0)));
    let start = core.handle_input(InputEvent::mouse_down(1));
    assert_eq!(phases(&start), ["start"]);
    // The host reacts to the drag start by growing a column under the tab.
    frame(&mut core, true);
    let up = core.handle_input(InputEvent::mouse_up());
    assert!(
        up.iter().any(|e| e.payload.as_str() == Some("clicked")),
        "release on the same tab is still a click: {up:?}"
    );
    // And the column below it is what the cursor finds once it leaves.
    frame(&mut core, true);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 200.0)));
    let col = {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let k = ui.child_key("tab").str("col");
        ui.finish();
        k
    };
    frame(&mut core, true);
    assert!(core.interaction.is_hovered(col));
}

#[test]
fn a_captured_drag_stays_pressed_off_the_node() {
    // A divider dragged past its own rect (or out of the window) keeps its
    // pressed / hover styling: the pointer is captured until release.
    let mut core = Core::new();
    drag_frame(&mut core);
    let divider = {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        let key = ui.child_key("divider");
        ui.finish();
        key
    };
    drag_frame(&mut core);

    core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    assert!(core.interaction.is_pressed(divider));

    // Off the divider entirely: hover follows the cursor, the press doesn't.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(300.0, 50.0)));
    assert!(!core.interaction.is_hovered(divider));
    assert!(
        core.interaction.is_pressed(divider),
        "the captured drag holds the press"
    );

    // Same once the cursor leaves the window.
    core.handle_input(InputEvent::CursorLeft);
    assert!(core.interaction.is_pressed(divider));

    core.handle_input(InputEvent::mouse_up());
    assert!(!core.interaction.is_pressed(divider));
}

#[test]
fn a_captured_drag_keeps_its_hover_group_pressed() {
    let group = NodeSpec::hover_group_id("split");
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        ui.with_keyed(
            "handle",
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(30.0))
                .hover_group("split")
                .on_drag(Value::str("split")),
            |_| {},
        );
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);

    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 15.0)));
    core.handle_input(InputEvent::mouse_down(1));
    assert!(core.interaction.is_group_pressed(group));

    core.handle_input(InputEvent::CursorMoved(Vec2::new(300.0, 200.0)));
    assert!(!core.interaction.is_group_hovered(group));
    assert!(
        core.interaction.is_group_pressed(group),
        "the group stays pressed for the whole captured drag"
    );

    core.handle_input(InputEvent::mouse_up());
    assert!(!core.interaction.is_group_pressed(group));
}
