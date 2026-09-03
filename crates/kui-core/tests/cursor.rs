//! The pointer shape the core derives per frame (`Core::cursor_shape`):
//! nothing declares one for the ordinary cases, and the `cursor` prop
//! overrides where the derivation cannot know.

use kui_core::{Core, CursorShape, EditOptions, InputEvent, NodeSpec, Size, Sizing, Value, Vec2};

const W: f32 = 400.0;
const H: f32 = 400.0;
/// Each node is a full-width band this tall, so a y picks one.
const BAND: f32 = 40.0;

/// A y inside band `i` (see `frame`).
fn in_band(i: usize) -> Vec2 {
    Vec2::new(W / 2.0, i as f32 * BAND + BAND / 2.0)
}

fn band(spec: NodeSpec) -> NodeSpec {
    spec.width(Sizing::Grow(1.0)).height(Sizing::Fixed(BAND))
}

/// One band per thing a cursor can be over, stacked in a column.
fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    // 0: an editor.
    ui.text_edit(
        "doc",
        "hello",
        &EditOptions::default(),
        band(NodeSpec::column()),
    );
    // 1: a button.
    ui.with_keyed("button", band(NodeSpec::column()).on_click("go"), |_| {});
    // 2: a plain box — no hit region at all.
    ui.with_keyed("plain", band(NodeSpec::column()), |_| {});
    // 3: a drag source.
    ui.with_keyed("handle", band(NodeSpec::column()).on_drag("h"), |_| {});
    // 4: a splitter: draggable, but it resizes rather than moves.
    ui.with_keyed(
        "splitter",
        band(NodeSpec::column())
            .on_drag("s")
            .cursor(CursorShape::EwResize),
        |_| {},
    );
    // 5: a disabled control that says so.
    ui.with_keyed(
        "refused",
        band(NodeSpec::column())
            .on_click("nope")
            .disabled(true)
            .cursor(CursorShape::NotAllowed),
        |_| {},
    );
    // 6: a disabled control that stays quiet.
    ui.with_keyed(
        "quiet",
        band(NodeSpec::column()).on_click("nope").disabled(true),
        |_| {},
    );
    // 7: a window-drag strip.
    ui.with_keyed("titlebar", band(NodeSpec::column()).window_drag(), |_| {});
    // 8: focusable without a click payload (a list row that opens on Enter).
    ui.with_keyed("row", band(NodeSpec::column()).focusable(), |_| {});
    // 9: hover-only (a tooltip badge).
    ui.with_keyed("badge", band(NodeSpec::column()).hoverable(), |_| {});
    ui.finish();
}

/// The shape after moving the pointer into band `i`.
fn shape_over(core: &mut Core, i: usize) -> CursorShape {
    core.handle_input(InputEvent::CursorMoved(in_band(i)));
    core.cursor_shape()
}

#[test]
fn editor_is_a_caret_button_a_hand_plain_box_an_arrow() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 0), CursorShape::Text, "an editor");
    assert_eq!(shape_over(&mut core, 1), CursorShape::Pointer, "a button");
    assert_eq!(
        shape_over(&mut core, 2),
        CursorShape::Default,
        "a plain box"
    );
}

#[test]
fn a_focusable_node_is_a_hand_and_a_hover_only_one_is_not() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 8), CursorShape::Pointer, "focusable");
    assert_eq!(shape_over(&mut core, 9), CursorShape::Default, "hoverable");
}

#[test]
fn window_chrome_keeps_the_platform_arrow() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 7), CursorShape::Default);
}

#[test]
fn no_pointer_is_the_arrow() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(
        core.cursor_shape(),
        CursorShape::Default,
        "before any input"
    );
    shape_over(&mut core, 0);
    core.handle_input(InputEvent::CursorLeft);
    assert_eq!(core.cursor_shape(), CursorShape::Default, "pointer gone");
}

#[test]
fn a_drag_source_grabs_and_holds_the_shape_through_the_drag() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 3), CursorShape::Grab, "resting");

    core.handle_input(InputEvent::mouse_down(1));
    assert_eq!(core.cursor_shape(), CursorShape::Grabbing, "pressed");
    // The pointer is captured: wandering onto the plain box below changes
    // nothing until the release.
    core.handle_input(InputEvent::CursorMoved(in_band(2)));
    assert_eq!(core.cursor_shape(), CursorShape::Grabbing, "off the node");

    core.handle_input(InputEvent::mouse_up());
    assert_eq!(
        core.cursor_shape(),
        CursorShape::Default,
        "over the box now"
    );
    assert_eq!(shape_over(&mut core, 3), CursorShape::Grab, "back to rest");
}

#[test]
fn a_declared_cursor_overrides_the_derivation() {
    let mut core = Core::new();
    frame(&mut core);
    // The splitter is an `on_drag` node, so it would derive `grab`.
    assert_eq!(shape_over(&mut core, 4), CursorShape::EwResize, "resting");
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::CursorMoved(in_band(2)));
    assert_eq!(
        core.cursor_shape(),
        CursorShape::EwResize,
        "the captured drag keeps the splitter's own shape, not grabbing"
    );
    core.handle_input(InputEvent::mouse_up());
}

#[test]
fn a_disabled_node_says_nothing_unless_it_declares_a_cursor() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(
        shape_over(&mut core, 5),
        CursorShape::NotAllowed,
        "declared"
    );
    // Disabled strips the click payload, so there is nothing to derive a
    // hand from — a quiet disabled control is just an arrow.
    assert_eq!(shape_over(&mut core, 6), CursorShape::Default, "quiet");
}

#[test]
fn the_topmost_node_answers() {
    // A button inside a card: the card is hover-tracked and the button is
    // on top of it, exactly as a click would resolve.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "card",
        NodeSpec::column().fill().pad(20.0).hoverable(),
        |ui| {
            ui.with_keyed(
                "button",
                NodeSpec::column()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(30.0))
                    .on_click(Value::str("go")),
                |_| {},
            );
        },
    );
    ui.finish();

    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 30.0)));
    assert_eq!(core.cursor_shape(), CursorShape::Pointer, "over the button");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 200.0)));
    assert_eq!(core.cursor_shape(), CursorShape::Default, "over the card");
}
