//! `over_siblings`: a node and its subtree paint after their siblings, over any
//! they overlap, and take the press there — the sibling order and nothing
//! else (`docs/adr/0046-a-node-over-its-siblings-paints-after-them.md`).
//! From kawoosh's tab strip, where a tab moved left glided under the
//! neighbour making room for it and one moved right over it.

use kui_core::testing::click;
use kui_core::{
    Align, Color, Core, Easing, FloatConfig, NodeSpec, QuadKind, Size, TextStyle, Transition, Vec2,
};

const VIEW: Size = Size { w: 400.0, h: 200.0 };

const fn rgb(r: f32, g: f32, b: f32) -> Color {
    Color { r, g, b, a: 1.0 }
}
const RED: Color = rgb(1.0, 0.0, 0.0);
const GREEN: Color = rgb(0.0, 1.0, 0.0);
const BLUE: Color = rgb(0.0, 0.0, 1.0);
const GREY: Color = rgb(0.2, 0.2, 0.2);
const WHITE: Color = rgb(1.0, 1.0, 1.0);

/// Every solid quad of the last frame, named by its colour.
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

fn tile(bg: Color) -> NodeSpec {
    NodeSpec::column().size(60.0, 40.0).bg(bg)
}

#[test]
fn a_node_over_its_siblings_paints_after_them_and_before_what_follows_its_parent() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("strip", NodeSpec::row().bg(GREY), |ui| {
        ui.leaf_keyed("a", tile(RED));
        ui.with_keyed("b", tile(GREEN).over_siblings(), |ui| {
            ui.leaf_keyed("b-dot", NodeSpec::column().size(4.0, 4.0).bg(WHITE));
        });
        ui.leaf_keyed("c", tile(BLUE));
    });
    ui.leaf_keyed("after", NodeSpec::column().size(10.0, 10.0).bg(GREY));
    ui.finish();
    assert_eq!(
        painted(&mut core),
        ["grey", "red", "blue", "green", "white", "grey"],
        "the strip, its other tiles, the one over its siblings with its subtree, then the strip's sibling"
    );
}

#[test]
fn two_over_their_siblings_keep_tree_order_and_one_inside_a_sibling_stays_under_one_beside_it() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("strip", NodeSpec::row(), |ui| {
        ui.leaf_keyed("a", tile(RED).over_siblings());
        // The last child, so its subtree ends where the strip's does: the
        // node over its siblings inside it and the strip's two are due
        // at one index, and the deeper one goes first.
        ui.leaf_keyed("b", tile(GREEN).over_siblings());
        ui.with_keyed("c", tile(GREY), |ui| {
            ui.leaf_keyed(
                "c1",
                NodeSpec::column().size(4.0, 4.0).bg(BLUE).over_siblings(),
            );
            ui.leaf_keyed("c2", NodeSpec::column().size(4.0, 4.0).bg(WHITE));
        });
    });
    ui.finish();
    assert_eq!(
        painted(&mut core),
        ["grey", "white", "blue", "red", "green"],
        "c1 after c's other child, and a and b after all of c"
    );
}

/// Two 100-wide tabs; the one keyed by `over` declares `over_siblings`.
fn tabs(core: &mut Core, order: [&str; 2], over: Option<&str>) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("strip", NodeSpec::row(), |ui| {
        for label in order {
            let bg = if label == "a" { RED } else { BLUE };
            let mut spec = NodeSpec::column()
                .size(100.0, 30.0)
                .bg(bg)
                .on_click(label)
                .transition_with(Transition::ms(100.0).easing(Easing::Linear))
                .slide();
            if over == Some(label) {
                spec = spec.over_siblings();
            }
            ui.leaf_keyed(label, spec);
        }
    });
    ui.finish();
}

/// The reported case: `b` moved left past `a`. Mid-glide they overlap,
/// and tree order alone would put `a` — now in the later place — on top.
#[test]
fn a_tab_moved_left_glides_over_its_neighbour_when_over_its_siblings() {
    for over in [None, Some("b")] {
        let mut core = Core::new();
        core.set_time(0.0);
        tabs(&mut core, ["a", "b"], over);
        core.set_time(0.0);
        tabs(&mut core, ["b", "a"], over);
        core.set_time(0.03);
        tabs(&mut core, ["b", "a"], over);
        // b at 70, a at 30: they share 70..130.
        let order = painted(&mut core);
        let top = if over.is_some() { "blue" } else { "red" };
        assert_eq!(order.last(), Some(&top), "over={over:?}: {order:?}");
        let evs = click(&mut core, Vec2::new(100.0, 15.0));
        let hit = if over.is_some() { "b" } else { "a" };
        assert_eq!(
            evs.first().and_then(|e| e.payload.as_str()),
            Some(hit),
            "over={over:?}: the press goes to what is on top"
        );
    }
}

#[test]
fn a_node_over_its_siblings_in_a_float_paints_last_in_that_layer() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed("page", NodeSpec::column().size(50.0, 50.0).bg(GREY));
    ui.with_keyed(
        "pop",
        NodeSpec::row().float(
            FloatConfig::viewport()
                .inside(Align::Start, Align::Start)
                .offset(100.0, 50.0),
        ),
        |ui| {
            ui.leaf_keyed("x", tile(GREEN).over_siblings());
            ui.leaf_keyed("y", tile(RED));
        },
    );
    ui.leaf_keyed("tail", NodeSpec::column().size(50.0, 50.0).bg(BLUE));
    ui.finish();
    assert_eq!(
        painted(&mut core),
        ["grey", "blue", "red", "green"],
        "the in-flow layer, then the float's with x last"
    );
}

/// A selection reads runs in tree order, and emission records them as it
/// paints: a run painted late, over its siblings, still reads in its place.
#[test]
fn a_run_over_its_siblings_keeps_its_place_in_a_selection() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let card = ui.with_keyed("card", NodeSpec::column().grow_width().selectable(), |ui| {
        let style = TextStyle::new(14.0);
        ui.text_in_keyed("one", NodeSpec::row(), "alpha", style);
        ui.text_in_keyed("two", NodeSpec::row().over_siblings(), "beta", style);
        ui.text_in_keyed("three", NodeSpec::row(), "gamma", style);
    });
    ui.finish();
    assert!(core.select_all_in(card));
    let text = core.selection_text().unwrap();
    let at = |w: &str| text.find(w).unwrap();
    assert!(
        at("alpha") < at("beta") && at("beta") < at("gamma"),
        "reading order is the tree's: {text:?}"
    );
}
