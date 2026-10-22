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
        .filter(|e| e.event.name() == "drag")
        .map(|e| {
            e.data()
                .and_then(|d| d.get_str("phase"))
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
    let mv = all[1].data().unwrap();
    assert_eq!(mv.get_float("x"), Some(150.0));
    assert_eq!(mv.get_float("dx"), Some(45.0));
    assert_eq!(all[1].payload.get_str("tag"), Some("split"));
    // Parent rect: the root, i.e. the whole viewport.
    let parent = mv.get("parent").unwrap();
    assert_eq!(parent.get_float("w"), Some(400.0));
    assert_eq!(parent.get_float("h"), Some(300.0));
    // The real drag suppressed the click.
    assert!(
        !all.iter()
            .any(|e| e.payload.get_str("tag") == Some("clicked"))
    );
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
        all.iter()
            .any(|e| e.payload.get_str("tag") == Some("clicked")),
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
            .any(|e| e.key == a && e.data().and_then(|d| d.get_str("phase")) == Some("move")),
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
        up.iter()
            .any(|e| e.payload.get_str("tag") == Some("clicked")),
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

    let dx = |e: &UiEvent| e.data().unwrap().get_float("dx").unwrap();
    let dy = |e: &UiEvent| e.data().unwrap().get_float("dy").unwrap();
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
    assert_eq!(all[3].data().unwrap().get_float("x"), Some(113.0));
    // And a click it was not.
    assert!(
        !all.iter()
            .any(|e| e.payload.get_str("tag") == Some("clicked"))
    );
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
        .filter_map(|e| e.data()?.get_float("dx"))
        .collect();
    assert_eq!(dx, [0.0, 4.0, 5.0, 5.0]);
    assert!(
        !all.iter()
            .any(|e| e.payload.get_str("tag") == Some("clicked"))
    );
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
    let end = all[2].data().unwrap();
    assert_eq!(end.get_float("x"), Some(125.0));
    assert_eq!(end.get_float("dx"), Some(20.0));
    assert_eq!(end.get_float("dy"), Some(10.0));
}

/// RG75: the window losing the keyboard mid-drag ends the drag where the
/// pointer was, as it ends a held button's capture (RG65) — the release
/// is not coming here. Every move after the window came back went on
/// dragging with no button down. And a press that never dragged clicks
/// nothing on the blur, nor on a release that does come.
#[test]
fn losing_the_keyboard_mid_drag_ends_it_without_a_click() {
    let mut core = Core::new();
    drag_frame(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    let moved = core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 55.0)));
    assert_eq!(phases(&moved), ["move"]);
    core.set_focused(false);
    let blur = core.take_pending_events();
    assert_eq!(phases(&blur), ["end"]);
    assert_eq!(
        blur.iter()
            .find(|e| e.event.name() == "drag")
            .unwrap()
            .data()
            .unwrap()
            .get_float("x"),
        Some(150.0)
    );
    core.set_focused(true);
    let after = core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 60.0)));
    assert!(phases(&after).is_empty(), "no drag with no button down");

    // A press held on the divider, not dragged, then the blur.
    drag_frame(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(105.0, 50.0)));
    core.handle_input(InputEvent::mouse_down(1));
    core.set_focused(false);
    let mut evs = core.take_pending_events();
    core.set_focused(true);
    evs.extend(core.handle_input(InputEvent::mouse_up()));
    assert!(
        !evs.iter()
            .any(|e| e.payload.get_str("tag") == Some("clicked")),
        "{evs:?}"
    );
}

/// A scrollbar built out of boxes: an 8px `on_drag` track down the right
/// of a 200px-tall panel, a 40px spacer, and a thumb under it that lights
/// up on hover. `thumb` builds the thumb's spec, so a test can give it
/// a claim of its own. The track's parent is the panel (x 100, w 100).
fn bar_frame(core: &mut Core, thumb: impl Fn() -> NodeSpec) -> (kui_core::Key, kui_core::Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    ui.leaf(NodeSpec::column().width(100.0).grow_height());
    let mut keys = (kui_core::Key::ROOT, kui_core::Key::ROOT);
    ui.with(NodeSpec::row().width(100.0).height(200.0), |ui| {
        ui.leaf(NodeSpec::column().width(92.0).grow_height());
        keys.0 = ui.with_keyed(
            "track",
            NodeSpec::column().width(8.0).grow_height().on_drag("thumb"),
            |ui| {
                ui.leaf(NodeSpec::column().height(40.0));
                keys.1 = ui.leaf_keyed("knob", thumb().width(6.0).height(30.0));
            },
        );
    });
    ui.finish();
    keys
}

fn hover_thumb() -> NodeSpec {
    NodeSpec::column()
        .radius(3.0)
        .hover_bg(kui_core::Color::hex(0x888888))
}

/// Presses at (195, y), moves 50px down, lets go.
fn drag_on_bar(core: &mut Core, y: f32) -> Vec<UiEvent> {
    [
        InputEvent::CursorMoved(Vec2::new(195.0, y)),
        InputEvent::mouse_down(1),
        InputEvent::CursorMoved(Vec2::new(195.0, y + 50.0)),
        InputEvent::mouse_up(),
    ]
    .into_iter()
    .flat_map(|ev| core.handle_input(ev))
    .collect()
}

/// A press on a child that does nothing with a press of its own — the
/// thumb only lights up under the pointer — is the enclosing `on_drag`
/// node's, as an unclaimed secondary press is the enclosing menu's
/// (backlog F176). The child's hover region used to take the press and
/// drop it: the bar dragged from its empty track and never by its thumb.
#[test]
fn a_press_on_a_child_that_claims_nothing_drags_the_enclosing_node() {
    let mut core = Core::new();
    let (track, _) = bar_frame(&mut core, hover_thumb);
    bar_frame(&mut core, hover_thumb);

    let evs = drag_on_bar(&mut core, 55.0);
    assert_eq!(phases(&evs), ["start", "move", "end"]);
    let drags: Vec<&UiEvent> = evs.iter().filter(|e| e.event.name() == "drag").collect();
    assert!(
        drags.iter().all(|e| e.key == track),
        "on the track: {drags:?}"
    );
    assert_eq!(drags[0].payload.get_str("tag"), Some("thumb"));
    // The geometry is the owner's: its parent is the 100px panel.
    let mv = drags[1].data().unwrap();
    assert_eq!(mv.get_float("dy"), Some(50.0));
    let parent = mv.get("parent").unwrap();
    assert_eq!(parent.get_float("x"), Some(100.0));
    assert_eq!(parent.get_float("w"), Some(100.0));

    // The track's empty part drags as it always did.
    let evs = drag_on_bar(&mut core, 150.0);
    assert_eq!(phases(&evs), ["start", "move", "end"]);
}

/// A child that does something with a press keeps it: a clickable knob
/// inside the track clicks, and the track does not drag.
#[test]
fn a_child_that_claims_the_press_keeps_it() {
    let mut core = Core::new();
    let knob = || hover_thumb().on_click("knob");
    bar_frame(&mut core, knob);
    bar_frame(&mut core, knob);

    let evs = [
        InputEvent::CursorMoved(Vec2::new(195.0, 55.0)),
        InputEvent::mouse_down(1),
        InputEvent::mouse_up(),
    ]
    .into_iter()
    .flat_map(|ev| core.handle_input(ev))
    .collect::<Vec<_>>();
    assert!(phases(&evs).is_empty(), "no drag: {evs:?}");
    assert!(
        evs.iter().any(|e| e.payload.get_str("tag") == Some("knob")),
        "{evs:?}"
    );
}

/// The walk up stops at the first node that does something with a
/// press: a hover-lit icon inside a clickable chip inside a draggable
/// card is the chip's, not the card's — the nested declaration wins, as
/// it does for menus and buttons.
#[test]
fn the_walk_stops_at_a_nearer_control() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "card",
            NodeSpec::column().size(200.0, 100.0).on_drag("card"),
            |ui| {
                ui.with_keyed(
                    "chip",
                    NodeSpec::row().size(60.0, 30.0).on_click("chip"),
                    |ui| {
                        ui.leaf_keyed("icon", hover_thumb().size(20.0, 20.0));
                    },
                );
            },
        );
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);

    let evs: Vec<UiEvent> = [
        InputEvent::CursorMoved(Vec2::new(10.0, 10.0)),
        InputEvent::mouse_down(1),
        InputEvent::CursorMoved(Vec2::new(60.0, 60.0)),
        InputEvent::mouse_up(),
    ]
    .into_iter()
    .flat_map(|ev| core.handle_input(ev))
    .collect();
    assert!(phases(&evs).is_empty(), "the card stays put: {evs:?}");

    // Past the chip, on the card itself, it drags.
    let evs: Vec<UiEvent> = [
        InputEvent::CursorMoved(Vec2::new(150.0, 80.0)),
        InputEvent::mouse_down(1),
        InputEvent::CursorMoved(Vec2::new(160.0, 90.0)),
        InputEvent::mouse_up(),
    ]
    .into_iter()
    .flat_map(|ev| core.handle_input(ev))
    .collect();
    assert_eq!(phases(&evs), ["start", "move", "end"]);
}

/// A disabled `on_drag` node lends nothing, and the walk goes on past it
/// to the one above, as a disabled node's own menu is skipped.
#[test]
fn a_disabled_owner_is_walked_past() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "outer",
            NodeSpec::column().size(200.0, 100.0).on_drag("outer"),
            |ui| {
                ui.with_keyed(
                    "inner",
                    NodeSpec::column()
                        .size(100.0, 50.0)
                        .on_drag("inner")
                        .disabled(true),
                    |ui| {
                        ui.leaf_keyed("lit", hover_thumb().size(20.0, 20.0));
                    },
                );
            },
        );
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);

    let evs: Vec<UiEvent> = [
        InputEvent::CursorMoved(Vec2::new(10.0, 10.0)),
        InputEvent::mouse_down(1),
        InputEvent::CursorMoved(Vec2::new(60.0, 60.0)),
        InputEvent::mouse_up(),
    ]
    .into_iter()
    .flat_map(|ev| core.handle_input(ev))
    .collect();
    let drags: Vec<&str> = evs
        .iter()
        .filter(|e| e.event.name() == "drag")
        .filter_map(|e| e.payload.get_str("tag"))
        .collect();
    assert_eq!(drags, ["outer", "outer", "outer"]);
}

/// A press in a `selectable` scope inside a draggable card is the
/// scope's drag-select, not the card's drag: the walk up does not climb
/// out of the scope the press is in — on the scope's own region (its
/// text has none) as from a hover-lit child inside it.
#[test]
fn a_press_in_a_selectable_scope_selects_and_does_not_drag() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "card",
            NodeSpec::column().size(300.0, 200.0).on_drag("card"),
            |ui| {
                ui.with_keyed(
                    "prose",
                    NodeSpec::column().grow_width().selectable(),
                    |ui| {
                        ui.text("one two three", kui_core::TextStyle::new(16.0));
                        ui.leaf_keyed("lit", hover_thumb().size(20.0, 20.0));
                    },
                );
            },
        );
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);

    for (from, to) in [(2.0, 100.0), (30.0, 5.0)] {
        let evs: Vec<UiEvent> = [
            InputEvent::CursorMoved(Vec2::new(2.0, from)),
            InputEvent::mouse_down(1),
            InputEvent::CursorMoved(Vec2::new(to, 8.0)),
            InputEvent::mouse_up(),
        ]
        .into_iter()
        .flat_map(|ev| core.handle_input(ev))
        .collect();
        assert!(phases(&evs).is_empty(), "the card stays put: {evs:?}");
        assert!(
            core.selection_text().is_some_and(|t| !t.is_empty()),
            "pressed at y {from}: {:?}",
            core.selection_text()
        );
        core.clear_selection();
    }
}
