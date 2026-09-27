//! Dragging through a live `Core`: on_drag nodes (pointer capture, click
//! suppression, payload shape) and scrollbar thumb/track interaction.

use kui_core::testing::click_at;
use kui_core::{Core, InputEvent, NodeSpec, Size, Sizing, UiEvent, Value, Vec2};

fn drag_frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    // A 100px left panel, a draggable 10px divider, the rest.
    ui.leaf(NodeSpec::column().width(100.0).grow_height());
    ui.leaf_keyed(
        "divider",
        NodeSpec::column()
            .width(10.0)
            .grow_height()
            .on_click("clicked")
            .on_drag("split"),
    );
    ui.finish();
}

fn phases(evs: &[UiEvent]) -> Vec<String> {
    evs.iter()
        .filter(|e| e.kind() == Some("drag"))
        .map(|e| e.payload.get_str("phase").unwrap_or("?").to_string())
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
    assert_eq!(mv.get_float("x"), Some(150.0));
    assert_eq!(mv.get_float("dx"), Some(45.0));
    assert_eq!(mv.get_str("tag"), Some("split"));
    // Parent rect: the root, i.e. the whole viewport.
    let parent = mv.get("parent").unwrap();
    assert_eq!(parent.get_float("w"), Some(400.0));
    assert_eq!(parent.get_float("h"), Some(300.0));
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
        NodeSpec::column().grow_width().height(100.0).scroll_y(),
        |ui| {
            for _ in 0..20 {
                ui.leaf(NodeSpec::column().grow_width().height(30.0));
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
            ui.leaf_keyed(
                label,
                NodeSpec::column()
                    .size(100.0, 30.0)
                    .on_drag(Value::str(label)),
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
            .any(|e| e.key == a && e.payload.get_str("phase") == Some("move")),
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
    click_at(&mut core, 395.0, 95.0);
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
                .size(100.0, 30.0)
                .on_click("clicked")
                .on_drag("lift"),
            |ui| {
                if with_column {
                    ui.leaf_keyed(
                        "col",
                        NodeSpec::column()
                            .float(
                                kui_core::FloatConfig::parent()
                                    .at(kui_core::Align::Start, kui_core::Align::End),
                            )
                            .width(Sizing::Percent(1.0))
                            .height(300.0)
                            .hoverable(),
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
        ui.leaf_keyed(
            "handle",
            NodeSpec::column()
                .size(100.0, 30.0)
                .hover_group("split")
                .on_drag("split"),
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

/// `dx`/`dy` are the displacement from the press point in every phase, not
/// the step since the last event (backlog F2). Two 2 px moves sit inside the
/// slop, then a 4 px one leaves it: the `move` carries the whole 8, and so
/// does `end` — which used to report zero, so "accumulate on `move`, commit
/// on `end`" snapped the dragged thing back to where it started, and the
/// distance under the slop went missing from every sum.
#[test]
fn drag_deltas_are_measured_from_the_press_point() {
    let mut core = Core::new();
    drag_frame(&mut core);

    let mut all = Vec::new();
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0))));
    all.extend(core.handle_input(InputEvent::mouse_down(1)));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(107.0, 50.0))));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(109.0, 50.0))));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(113.0, 50.0))));
    all.extend(core.handle_input(InputEvent::mouse_up()));

    let dx = |e: &UiEvent| e.payload.get_float("dx").unwrap();
    let dy = |e: &UiEvent| e.payload.get_float("dy").unwrap();
    // The slop is measured from the press: 2 px is inside it, 4 px is not,
    // and the move that leaves it reports the whole distance so far.
    assert_eq!(phases(&all), ["start", "move", "move", "end"]);
    assert_eq!((dx(&all[0]), dy(&all[0])), (0.0, 0.0), "start is zero");
    assert_eq!(
        dx(&all[1]),
        4.0,
        "the first move carries the sub-slop distance too"
    );
    assert_eq!(
        dx(&all[2]),
        8.0,
        "a move is the displacement from the press, not a step"
    );
    assert_eq!(
        dx(&all[3]),
        8.0,
        "end is the total, so an app can commit from it"
    );
    assert_eq!(all[3].payload.get_float("x"), Some(113.0));
    // And a click it was not.
    assert!(!all.iter().any(|e| e.payload.as_str() == Some("clicked")));
}

/// A slow pointer never covers the slop between two events; measured from
/// the press it still gets there, so a careful 1 px-at-a-time drag is a drag.
#[test]
fn a_slow_drag_still_leaves_the_slop() {
    let mut core = Core::new();
    drag_frame(&mut core);

    let mut all = Vec::new();
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0))));
    all.extend(core.handle_input(InputEvent::mouse_down(1)));
    for x in 106..=110 {
        all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(x as f32, 50.0))));
    }
    all.extend(core.handle_input(InputEvent::mouse_up()));

    assert_eq!(phases(&all), ["start", "move", "move", "end"]);
    let dx: Vec<f64> = all
        .iter()
        .filter_map(|e| e.payload.get_float("dx"))
        .collect();
    assert_eq!(dx, [0.0, 4.0, 5.0, 5.0]);
    assert!(!all.iter().any(|e| e.payload.as_str() == Some("clicked")));
}

/// A drag released with the cursor outside the window ends where the
/// pointer was last seen, and the total is measured to there.
#[test]
fn a_drag_released_off_window_ends_at_the_last_seen_point() {
    let mut core = Core::new();
    drag_frame(&mut core);

    let mut all = Vec::new();
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0))));
    all.extend(core.handle_input(InputEvent::mouse_down(1)));
    all.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(125.0, 60.0))));
    all.extend(core.handle_input(InputEvent::CursorLeft));
    all.extend(core.handle_input(InputEvent::mouse_up()));

    assert_eq!(phases(&all), ["start", "move", "end"]);
    let end = &all[2].payload;
    assert_eq!(end.get_float("x"), Some(125.0));
    assert_eq!(end.get_float("dx"), Some(20.0));
    assert_eq!(end.get_float("dy"), Some(10.0));
}
