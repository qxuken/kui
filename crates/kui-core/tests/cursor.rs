//! The pointer shape the core resolves per frame (`Core::cursor_shape`):
//! what the node under the pointer declared with `cursor`, the I-beam
//! over text, and the arrow otherwise — a clickable or draggable node
//! that declared nothing included. The stock button is the one node that
//! declares a hand for itself.

use kui_core::{
    Core, CursorShape, EditOptions, InputEvent, NodeSpec, Size, Sizing, Value, Vec2, widgets,
};

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
    // 1: a clickable box that says it is a hand.
    ui.with_keyed(
        "button",
        band(NodeSpec::column())
            .on_click("go")
            .cursor(CursorShape::Pointer),
        |_| {},
    );
    // 2: a plain box — no hit region at all.
    ui.with_keyed("plain", band(NodeSpec::column()), |_| {});
    // 3: a drag source that says it is a grab.
    ui.with_keyed(
        "handle",
        band(NodeSpec::column())
            .on_drag("h")
            .cursor(CursorShape::Grab),
        |_| {},
    );
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
    // 10: a clickable box that said nothing — the ordinary case.
    ui.with_keyed("silent", band(NodeSpec::column()).on_click("go"), |_| {});
    // 11: a drag source that said nothing.
    ui.with_keyed("mute", band(NodeSpec::column()).on_drag("m"), |_| {});
    ui.finish();
}

/// The shape after moving the pointer into band `i`.
fn shape_over(core: &mut Core, i: usize) -> CursorShape {
    core.handle_input(InputEvent::CursorMoved(in_band(i)));
    core.cursor_shape()
}

#[test]
fn editor_is_a_caret_a_declared_hand_a_hand_plain_box_an_arrow() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 0), CursorShape::Text, "an editor");
    assert_eq!(shape_over(&mut core, 1), CursorShape::Pointer, "declared");
    assert_eq!(
        shape_over(&mut core, 2),
        CursorShape::Default,
        "a plain box"
    );
}

/// Nothing is implied from what a node does: an `on_click`, a
/// `focusable` and an `on_drag` that declared no cursor are the arrow,
/// exactly as a hover-only node is.
#[test]
fn a_node_that_declared_nothing_is_the_arrow_whatever_it_does() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 10), CursorShape::Default, "on_click");
    assert_eq!(shape_over(&mut core, 8), CursorShape::Default, "focusable");
    assert_eq!(shape_over(&mut core, 11), CursorShape::Default, "on_drag");
    assert_eq!(shape_over(&mut core, 9), CursorShape::Default, "hoverable");
    // And a drag captured on it holds the arrow off the node too.
    shape_over(&mut core, 11);
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::CursorMoved(in_band(0)));
    assert_eq!(
        core.cursor_shape(),
        CursorShape::Default,
        "captured over the editor, and still not its I-beam"
    );
    core.handle_input(InputEvent::mouse_up());
}

/// The stock button is the one node that declares the hand for itself,
/// so `<button>` in every binding is a hand; a caller's own `cursor` on
/// it stands, and an inert one is the arrow.
#[test]
fn the_stock_button_declares_the_hand() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let (t, m) = (ui.theme(), ui.metrics());
    // Each centred in a band of its own, so `in_band` lands on it.
    let band = |ui: &mut kui_core::Ui<'_>, body: &dyn Fn(&mut kui_core::Ui<'_>)| {
        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(BAND))
                .center(),
            |ui| body(ui),
        );
    };
    // 0: `widgets::button`.
    band(&mut ui, &|ui| widgets::button(ui, "Go", "go"));
    // 1: `button_with` with the caller's own cursor.
    band(&mut ui, &|ui| {
        widgets::button_with(
            ui,
            "wait",
            "Wait",
            widgets::button_spec(&t, &m)
                .on_click("wait")
                .cursor(CursorShape::NotAllowed),
            None,
        )
    });
    // 2: a disabled one.
    band(&mut ui, &|ui| {
        widgets::button_with(
            ui,
            "off",
            "Off",
            widgets::button_spec(&t, &m).on_click("off").disabled(true),
            None,
        )
    });
    ui.finish();
    for (i, want) in [
        CursorShape::Pointer,
        CursorShape::NotAllowed,
        CursorShape::Default,
    ]
    .into_iter()
    .enumerate()
    {
        core.handle_input(InputEvent::CursorMoved(in_band(i)));
        assert_eq!(core.cursor_shape(), want, "band {i}");
    }
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

/// A captured drag holds the dragged node's declared shape however far
/// the pointer wanders; `grabbing` is not promoted from `grab` by the
/// core — the view declares it as its drag state changes, as the drag
/// example does.
#[test]
fn a_drag_source_holds_its_declared_shape_through_the_drag() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 3), CursorShape::Grab, "resting");

    core.handle_input(InputEvent::mouse_down(1));
    assert_eq!(
        core.cursor_shape(),
        CursorShape::Grab,
        "pressed: as declared"
    );
    // The pointer is captured: wandering onto the plain box below changes
    // nothing until the release.
    core.handle_input(InputEvent::CursorMoved(in_band(2)));
    assert_eq!(core.cursor_shape(), CursorShape::Grab, "off the node");

    core.handle_input(InputEvent::mouse_up());
    assert_eq!(
        core.cursor_shape(),
        CursorShape::Default,
        "over the box now"
    );
    assert_eq!(shape_over(&mut core, 3), CursorShape::Grab, "back to rest");
}

#[test]
fn a_splitter_declares_its_own_arrows_and_keeps_them_captured() {
    let mut core = Core::new();
    frame(&mut core);
    assert_eq!(shape_over(&mut core, 4), CursorShape::EwResize, "resting");
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::CursorMoved(in_band(2)));
    assert_eq!(
        core.cursor_shape(),
        CursorShape::EwResize,
        "the captured drag keeps the splitter's own shape"
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
    assert_eq!(shape_over(&mut core, 6), CursorShape::Default, "quiet");
}

#[test]
fn the_topmost_node_answers() {
    // A hand inside a card: the card is hover-tracked and the button is
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
                    .on_click(Value::str("go"))
                    .cursor(CursorShape::Pointer),
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
