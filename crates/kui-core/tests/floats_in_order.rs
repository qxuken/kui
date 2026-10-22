//! `floats_in_order`: the floats under a node paint in tree order, the
//! last declared on top, worked out again every frame — not in the order
//! they opened (`docs/adr/0053-floats-under-a-node-paint-in-tree-order.md`).
//! From kawoosh's diagram pane, where a `.kdraw` file's items are floats
//! on a canvas and the file's order is the drawing's: an item panned into
//! view went over the rest, and moving a line in the file moved nothing.

use kui_core::testing::{click, hover};
use kui_core::{Align, Color, Core, FloatConfig, NodeSpec, QuadKind, Size, Vec2};

const VIEW: Size = Size { w: 400.0, h: 300.0 };

const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}
const RED: Color = rgb(1.0, 0.0, 0.0);
const GREEN: Color = rgb(0.0, 1.0, 0.0);
const BLUE: Color = rgb(0.0, 0.0, 1.0);
const GREY: Color = rgb(0.2, 0.2, 0.2);
const WHITE: Color = rgb(1.0, 1.0, 1.0);

/// Every solid quad of the last frame in a test colour, named by it; the
/// theme's quads (a hint's box) are `?`.
fn painted(core: &mut Core) -> Vec<&'static str> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .map(|q| match q.color {
            c if c == RED => "red",
            c if c == GREEN => "green",
            c if c == BLUE => "blue",
            c if c == GREY => "grey",
            c if c == WHITE => "white",
            _ => "?",
        })
        .collect()
}

/// A 100×80 item at (`x`, 40) on the canvas, clicked as its colour.
fn item(x: f32, bg: Color, tag: &'static str) -> NodeSpec {
    NodeSpec::column()
        .size(100.0, 80.0)
        .bg(bg)
        .on_click(tag)
        .float(
            FloatConfig::parent()
                .inside(Align::Start, Align::Start)
                .offset(x, 40.0),
        )
}

fn canvas() -> NodeSpec {
    NodeSpec::column().fill().floats_in_order()
}

fn tag_of(core: &mut Core, x: f32, y: f32) -> Option<String> {
    click(core, Vec2::new(x, y))
        .pop()
        .and_then(|e| e.payload.get_str("tag").map(str::to_string))
}

/// Draw `items` (key, x, colour) on the canvas in the order given.
fn draw(core: &mut Core, ordered: bool, items: &[(&'static str, f32, Color)]) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let spec = if ordered {
        canvas()
    } else {
        NodeSpec::column().fill()
    };
    ui.with_keyed("canvas", spec, |ui| {
        for &(key, x, bg) in items {
            ui.leaf_keyed(key, item(x, bg, key));
        }
    });
    ui.finish();
}

#[test]
fn a_float_added_under_an_ordered_node_takes_its_place_in_tree_order() {
    let mut core = Core::new();
    draw(&mut core, true, &[("a", 40.0, RED), ("c", 120.0, BLUE)]);
    assert_eq!(painted(&mut core), ["red", "blue"]);
    // `b` comes into view between them in the file: under `c`, over `a`.
    draw(
        &mut core,
        true,
        &[("a", 40.0, RED), ("b", 80.0, GREEN), ("c", 120.0, BLUE)],
    );
    assert_eq!(painted(&mut core), ["red", "green", "blue"]);
    assert_eq!(tag_of(&mut core, 130.0, 60.0).as_deref(), Some("c"));
    assert_eq!(tag_of(&mut core, 100.0, 60.0).as_deref(), Some("b"));

    // Without the prop the stack puts the newcomer on top: the case the
    // prop is for.
    let mut core = Core::new();
    draw(&mut core, false, &[("a", 40.0, RED), ("c", 120.0, BLUE)]);
    draw(
        &mut core,
        false,
        &[("a", 40.0, RED), ("b", 80.0, GREEN), ("c", 120.0, BLUE)],
    );
    assert_eq!(painted(&mut core), ["red", "blue", "green"]);
}

#[test]
fn reordering_two_keyed_floats_swaps_their_paint_and_their_press_on_the_next_frame() {
    let mut core = Core::new();
    draw(&mut core, true, &[("a", 40.0, RED), ("b", 80.0, GREEN)]);
    assert_eq!(painted(&mut core), ["red", "green"]);
    assert_eq!(tag_of(&mut core, 100.0, 60.0).as_deref(), Some("b"));
    draw(&mut core, true, &[("b", 80.0, GREEN), ("a", 40.0, RED)]);
    assert_eq!(painted(&mut core), ["green", "red"]);
    assert_eq!(tag_of(&mut core, 100.0, 60.0).as_deref(), Some("a"));
    // And it holds while nothing changes.
    draw(&mut core, true, &[("b", 80.0, GREEN), ("a", 40.0, RED)]);
    assert_eq!(painted(&mut core), ["green", "red"]);
}

/// A menu that opened after the canvas's floats stays over all of them,
/// a newcomer to the canvas included: the group keeps its place in the
/// stack, and only its order inside changes.
#[test]
fn the_group_keeps_its_place_among_the_floats_outside_it() {
    let frame = |core: &mut Core, menu: bool, b: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("canvas", canvas(), |ui| {
            ui.leaf_keyed("a", item(40.0, RED, "a"));
            if b {
                ui.leaf_keyed("b", item(80.0, GREEN, "b"));
            }
        });
        if menu {
            ui.leaf_keyed(
                "menu",
                NodeSpec::column()
                    .size(60.0, 200.0)
                    .bg(GREY)
                    .on_click("menu")
                    .float(
                        FloatConfig::viewport()
                            .inside(Align::Start, Align::Start)
                            .offset(110.0, 0.0),
                    ),
            );
        }
        ui.finish();
    };
    let mut core = Core::new();
    frame(&mut core, true, false);
    assert_eq!(painted(&mut core), ["red", "grey"]);
    frame(&mut core, true, true);
    assert_eq!(
        painted(&mut core),
        ["red", "green", "grey"],
        "b under the menu"
    );
    assert_eq!(tag_of(&mut core, 150.0, 60.0).as_deref(), Some("menu"));

    // A menu declared before the canvas but opened after it is over it
    // too: the stack's order between the group and the rest.
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("canvas", canvas(), |ui| {
        ui.leaf_keyed("a", item(40.0, RED, "a"));
    });
    ui.finish();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "menu",
        NodeSpec::column()
            .size(60.0, 200.0)
            .bg(GREY)
            .float(FloatConfig::viewport()),
    );
    ui.with_keyed("canvas", canvas(), |ui| {
        ui.leaf_keyed("a", item(40.0, RED, "a"));
        ui.leaf_keyed("b", item(80.0, GREEN, "b"));
    });
    ui.finish();
    assert_eq!(painted(&mut core), ["red", "green", "grey"]);
}

/// A float nested in an item is over that item and, being later in the
/// tree than it and earlier than the next item, under the next one —
/// tree order at every depth.
#[test]
fn a_float_nested_in_an_item_is_over_it_and_under_the_items_after_it() {
    let frame = |core: &mut Core, flip: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("canvas", canvas(), |ui| {
            let a = |ui: &mut kui_core::Ui<'_>| {
                ui.with_keyed("a", item(40.0, RED, "a"), |ui| {
                    ui.leaf_keyed(
                        "label",
                        NodeSpec::column().size(20.0, 20.0).bg(WHITE).float(
                            FloatConfig::parent()
                                .inside(Align::End, Align::Start)
                                .offset(10.0, 0.0),
                        ),
                    );
                });
            };
            if flip {
                ui.leaf_keyed("b", item(80.0, GREEN, "b"));
                a(ui);
            } else {
                a(ui);
                ui.leaf_keyed("b", item(80.0, GREEN, "b"));
            }
        });
        ui.finish();
    };
    let mut core = Core::new();
    frame(&mut core, false);
    assert_eq!(painted(&mut core), ["red", "white", "green"]);
    frame(&mut core, true);
    assert_eq!(painted(&mut core), ["green", "red", "white"]);
    frame(&mut core, false);
    assert_eq!(painted(&mut core), ["red", "white", "green"]);
}

/// The prop declared on a canvas whose floats the stack already holds
/// reorders them on that frame, though no key moved.
#[test]
fn declaring_it_orders_floats_the_stack_already_holds() {
    let mut core = Core::new();
    let items = [("a", 40.0, RED), ("b", 80.0, GREEN)];
    draw(&mut core, false, &items[..1]);
    draw(&mut core, false, &[("b", 80.0, GREEN)]);
    draw(&mut core, false, &[("b", 80.0, GREEN), ("a", 40.0, RED)]);
    assert_eq!(painted(&mut core), ["green", "red"], "a opened last");
    draw(&mut core, false, &items);
    assert_eq!(
        painted(&mut core),
        ["green", "red"],
        "the stack, not the tree"
    );
    draw(&mut core, true, &items);
    assert_eq!(painted(&mut core), ["red", "green"], "the tree");
}

/// `over_siblings` inside an item still raises a child over its siblings
/// in that item's layer, and on an item it does not move the item among
/// the floats: a float is a layer, and their order is the tree's.
#[test]
fn over_siblings_composes_with_it() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("canvas", canvas(), |ui| {
        ui.with_keyed("a", item(40.0, RED, "a").over_siblings(), |ui| {
            ui.leaf_keyed(
                "front",
                NodeSpec::column()
                    .size(30.0, 30.0)
                    .bg(WHITE)
                    .over_siblings(),
            );
            ui.leaf_keyed("back", NodeSpec::column().size(30.0, 30.0).bg(GREY));
        });
        ui.leaf_keyed("b", item(80.0, GREEN, "b"));
    });
    ui.finish();
    assert_eq!(painted(&mut core), ["red", "grey", "white", "green"]);
}

/// The hover hint the core floats for an item's `tooltip` goes over every
/// item, the ones after the hovered item included.
#[test]
fn a_tooltip_hint_goes_over_the_items() {
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("canvas", canvas(), |ui| {
            ui.with_keyed("a", item(40.0, RED, "a").tooltip("first"), |_| {});
            ui.leaf_keyed("b", item(80.0, GREEN, "b"));
        });
        ui.finish();
    };
    let mut core = Core::new();
    frame(&mut core);
    hover(&mut core, Vec2::new(60.0, 60.0));
    frame(&mut core);
    let painted = painted(&mut core);
    assert_eq!(&painted[..2], ["red", "green"], "{painted:?}");
    assert!(
        painted.len() > 2 && painted[2..].iter().all(|&p| p == "?"),
        "the hint after both items: {painted:?}"
    );
}
