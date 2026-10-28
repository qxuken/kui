//! A trackpad's pinch (backlog F256) reaches the innermost `on_scroll`
//! node around the pointer when it begins, as that node's `scroll` with
//! `zoom` in it, and stays that node's to the gesture's end. No scroll
//! container moves for it, a handler that names `scroll_mods` is passed
//! by, and one begun over no handler is nobody's.

use kui_core::{Core, InputEvent, KeyMods, NodeSpec, ScrollAxes, Size, UiEvent, Value, Vec2};

/// A window-wide handler for Ctrl (`named`) around a plain handler,
/// `canvas` (rows 0–100), which narrows its axes to `y` and holds a list
/// that scrolls (rows 50–100); below the canvas a list of its own (rows
/// 100–200) that no handler is around but the named one.
fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let named = NodeSpec::column()
        .fill()
        .on_scroll(Value::str("named"))
        .scroll_mods(KeyMods::NONE.with_ctrl());
    ui.with_keyed("named", named, |ui| {
        let canvas = NodeSpec::column()
            .grow_width()
            .height(100.0)
            .on_scroll(Value::str("canvas"))
            .scroll_axes(ScrollAxes::Y);
        ui.with_keyed("canvas", canvas, |ui| {
            ui.leaf(NodeSpec::row().grow_width().height(50.0));
            ui.with_keyed(
                "inner",
                NodeSpec::column().grow_width().height(50.0).scroll_y(),
                |ui| {
                    ui.leaf(NodeSpec::row().grow_width().height(500.0));
                },
            );
        });
        ui.with_keyed(
            "list",
            NodeSpec::column().grow_width().height(100.0).scroll_y(),
            |ui| {
                ui.leaf(NodeSpec::row().grow_width().height(900.0));
            },
        );
    });
    ui.finish();
}

fn setup() -> Core {
    let mut core = Core::new();
    frame(&mut core);
    frame(&mut core);
    core
}

const CANVAS: f32 = 25.0;
const INNER: f32 = 75.0;
const LIST: f32 = 150.0;

fn pinch(core: &mut Core, y: f32, delta: f32, begins: bool) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, y)));
    core.handle_input(InputEvent::Pinch { delta, begins })
}

/// Who heard the pinch, by tag, with the zoom each heard.
fn heard(evs: &[UiEvent]) -> Vec<(&str, f32)> {
    evs.iter()
        .filter_map(|e| Some((e.payload.get("tag")?.as_str()?, e.scroll()?.zoom?)))
        .collect()
}

fn offset(core: &mut Core, name: &str) -> f32 {
    frame(core);
    let key = core.key_of(name).expect("the list");
    core.scroll_offset(key).y
}

#[test]
fn a_pinch_is_the_innermost_handlers_scroll_with_its_zoom() {
    let mut core = setup();
    // macOS's first event is a 0: it picks, and nobody hears it.
    assert!(pinch(&mut core, CANVAS, 0.0, true).is_empty());
    let evs = pinch(&mut core, CANVAS, 0.125, false);
    assert_eq!(heard(&evs), [("canvas", 0.125)], "{evs:?}");
    let s = evs[0].scroll().unwrap();
    assert_eq!(s.pos, Vec2::new(200.0, CANVAS), "the pointer");
    assert_eq!(s.delta, Vec2::ZERO);
    assert_eq!(s.lines, None);
    assert_eq!(s.mods, None);
    assert_eq!(
        evs[0].payload.get("scroll").and_then(|d| d.get("lines")),
        Some(&Value::Null),
        "lines is there, and null"
    );
}

#[test]
fn the_handler_hears_it_whatever_its_axes_and_over_a_scroller_inside_it() {
    let mut core = setup();
    // The canvas takes only `y`, and the list inside it scrolls `y`: a
    // pinch is neither's axis, and the list is no handler.
    let evs = pinch(&mut core, INNER, -0.5, true);
    assert_eq!(heard(&evs), [("canvas", -0.5)], "{evs:?}");
    assert_eq!(offset(&mut core, "inner"), 0.0, "the list stood still");
}

#[test]
fn a_pinch_stays_the_node_it_began_on() {
    let mut core = setup();
    pinch(&mut core, CANVAS, 0.0, true);
    // Off the canvas, over the list below it: still the canvas's.
    let evs = pinch(&mut core, LIST, 0.25, false);
    assert_eq!(heard(&evs), [("canvas", 0.25)], "{evs:?}");
    // A new pinch over the list is nobody's, and stays so when it
    // comes back over the canvas.
    assert!(pinch(&mut core, LIST, 0.25, true).is_empty());
    assert!(pinch(&mut core, CANVAS, 0.25, false).is_empty());
    assert_eq!(
        offset(&mut core, "list"),
        0.0,
        "no scroller moves for a pinch"
    );
    // The next pinch over the canvas is the canvas's again.
    let evs = pinch(&mut core, CANVAS, 0.25, true);
    assert_eq!(heard(&evs), [("canvas", 0.25)]);
}

#[test]
fn a_handler_that_names_modifiers_is_passed_by_held_or_not() {
    let mut core = setup();
    // Over the list, only the Ctrl handler is around the pointer.
    assert!(pinch(&mut core, LIST, 0.25, true).is_empty());
    core.handle_input(InputEvent::Modifiers(KeyMods::NONE.with_ctrl()));
    assert!(pinch(&mut core, LIST, 0.25, true).is_empty());
    // Over the canvas the plain handler is the pinch's, Ctrl held or not.
    let evs = pinch(&mut core, CANVAS, 0.25, true);
    assert_eq!(heard(&evs), [("canvas", 0.25)]);
}

#[test]
fn a_pinch_with_nothing_under_the_pointer_is_dropped() {
    let mut core = Core::new();
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
            ui.leaf(NodeSpec::row().grow_width().height(900.0));
        });
        ui.finish();
    }
    assert!(pinch(&mut core, 100.0, 0.5, true).is_empty());
    assert!(pinch(&mut core, 100.0, f32::NAN, false).is_empty());
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.finish();
    // No frame at all under it, and no pointer: nobody's either.
    core.handle_input(InputEvent::CursorLeft);
    assert!(
        core.handle_input(InputEvent::Pinch {
            delta: 0.5,
            begins: true
        })
        .is_empty()
    );
}

#[test]
fn a_latched_node_that_is_gone_is_picked_again() {
    let mut core = setup();
    pinch(&mut core, CANVAS, 0.0, true);
    // A frame without the canvas: the pinch goes on to what is under the
    // pointer now.
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "other",
        NodeSpec::column().fill().on_scroll(Value::str("other")),
    );
    ui.finish();
    let evs = pinch(&mut core, CANVAS, 0.25, false);
    assert_eq!(heard(&evs), [("other", 0.25)], "{evs:?}");
}
