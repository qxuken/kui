//! Size expressions (backlog F109): `min()`, `max()` and `clamp()` over
//! lengths and percentages, resolved by layout against the parent's
//! content box — as a width, as a clamp, on a float and in a table — and
//! the same expression spelled, built as data and built in Rust landing
//! on one entry.
//!
//! Driven through `layout::compute` as `table_layout.rs` is, so every
//! number is exact.

use kui_core::calc::{self, Expr};
use kui_core::key::Key;
use kui_core::layout::{TextMeasure, compute};
use kui_core::resources::ImageId;
use kui_core::schema::{max_str, min_str, sizing_str};
use kui_core::scroll::ScrollStore;
use kui_core::spec::{FloatAnchor, FloatConfig, NodeSpec, Sizing};
use kui_core::tree::{NIL, NodeContent, OriginId, TextId, Tree};
use kui_core::value::Value;
use kui_core::{Edges, Size};

struct Stub;

impl TextMeasure for Stub {
    fn intrinsic(&mut self, id: TextId) -> Size {
        Size::new(10.0 * id.0 as f32, 20.0)
    }
    fn wrapped(&mut self, id: TextId, _max_w: f32) -> Size {
        Size::new(10.0 * id.0 as f32, 20.0)
    }
    fn image_size(&mut self, _id: ImageId) -> Size {
        Size::new(0.0, 0.0)
    }
}

struct T {
    tree: Tree,
}

impl T {
    fn new(root: NodeSpec) -> Self {
        let mut tree = Tree::new();
        tree.push(NIL, Key::ROOT, OriginId::HOST, root, NodeContent::Container);
        T { tree }
    }

    fn node(&mut self, parent: u32, spec: NodeSpec) -> u32 {
        let key = Key::ROOT.index(self.tree.len() as u64);
        self.tree
            .push(parent, key, OriginId::HOST, spec, NodeContent::Container)
    }

    fn run(&mut self, w: f32) {
        let mut scroll = ScrollStore::default();
        compute(
            &mut self.tree,
            &mut Stub,
            &mut scroll,
            Size::new(w, 600.0),
            1.0,
        );
    }

    fn w(&self, i: u32) -> f32 {
        self.tree.size[i as usize].w
    }
}

fn s(spelling: &str) -> Sizing {
    sizing_str(spelling).unwrap()
}

/// `clamp(400px, 80%, 1000px)` at the root: its three regimes against
/// the window, the minimum winning even past the window, as CSS's does.
#[test]
fn a_clamp_at_the_root_takes_the_window() {
    for (window, want) in [(300.0, 400.0), (1000.0, 800.0), (2000.0, 1000.0)] {
        let mut t = T::new(NodeSpec::column().width(s("clamp(400px, 80%, 1000px)")));
        t.run(window);
        assert_eq!(t.w(0), want, "a {window} px window");
    }
}

/// A child's room is its parent's content box — padding off, as a
/// percentage's is — whatever the parent's own sizing, and a column
/// inside a row gets its row's box, not the window's.
#[test]
fn a_child_takes_the_parent_s_content_box() {
    let mut t = T::new(NodeSpec::column().width(Sizing::GROW));
    let row = t.node(
        0,
        NodeSpec::row()
            .width(Sizing::Fixed(520.0))
            .padding(Edges::all(10.0))
            .gap(0.0),
    );
    let half = t.node(row, NodeSpec::column().width(s("min(50%, 300px)")));
    let rest = t.node(row, NodeSpec::column().width(s("max(25%, 100)")));
    let inner = t.node(
        half,
        NodeSpec::column().width(s("clamp(10px, 50%, 1000px)")),
    );
    t.run(1000.0);
    assert_eq!(t.w(half), 250.0, "50% of 500, under 300");
    assert_eq!(t.w(rest), 125.0, "25% of 500, over 100");
    assert_eq!(t.w(inner), 125.0, "half of its parent's 250");
}

/// A clamp as an expression: a `maxWidth` of `50%` holds a fixed child
/// to half its parent, a `minWidth` of `min(40%, 300)` floors a fit one,
/// and a `max` is none until the parent is sized — a fit parent is not
/// held back by it.
#[test]
fn a_clamp_is_an_expression_too() {
    let mut t = T::new(NodeSpec::column().width(Sizing::Fixed(600.0)));
    let wide = t.node(
        0,
        NodeSpec::column()
            .width(Sizing::Fixed(900.0))
            .max_width(max_str("50%").unwrap()),
    );
    let floor = t.node(
        0,
        NodeSpec::column().min_width(min_str("min(40%, 300)").unwrap()),
    );
    let fit = t.node(0, NodeSpec::row());
    let held = t.node(
        fit,
        NodeSpec::column()
            .width(Sizing::Fixed(80.0))
            .max_width(max_str("50%").unwrap()),
    );
    t.run(1000.0);
    assert_eq!(t.w(wide), 300.0, "900 held to half of 600");
    assert_eq!(t.w(floor), 240.0, "a fit child floored at 40% of 600");
    assert_eq!(
        t.w(fit),
        80.0,
        "the fit row read its child before the clamp"
    );
    assert_eq!(t.w(held), 40.0, "then the clamp took half of the row's 80");
}

/// A float sized against what it hangs from, and a table cell against
/// its row.
#[test]
fn a_float_and_a_table_cell() {
    let mut t = T::new(NodeSpec::column().width(Sizing::Fixed(400.0)));
    let anchor = t.node(
        0,
        NodeSpec::column()
            .width(Sizing::Fixed(200.0))
            .height(Sizing::Fixed(10.0)),
    );
    let float = t.node(
        anchor,
        NodeSpec::column()
            .float(FloatConfig::parent())
            .width(s("clamp(50px, 50%, 80px)")),
    );
    let table = t.node(0, NodeSpec::table().width(Sizing::GROW));
    let row = t.node(table, NodeSpec::row().width(Sizing::GROW));
    let cell = t.node(row, NodeSpec::column().width(s("min(25%, 90px)")));
    t.run(1000.0);
    assert_eq!(t.w(float), 80.0, "50% of 200 is 100, held to 80");
    assert_eq!(t.w(cell), 90.0, "25% of the row's 400 is 100, under 90");
}

/// Spelled, as data, and built in Rust: one entry.
#[test]
fn one_expression_three_ways() {
    let spelled = s("clamp(400px, 80%, 1000px)");
    let map = |k: &str, v: Value| Value::Map(vec![(k.to_string(), v)]);
    let data = calc::sizing_value(&map(
        "clamp",
        Value::List(vec![
            Value::Int(400),
            map("pct", Value::Int(80)),
            Value::Int(1000),
        ]),
    ))
    .unwrap();
    let built = calc::sizing_of(Expr::Clamp(
        Box::new(Expr::Px(400.0)),
        Box::new(Expr::Pct(0.8)),
        Box::new(Expr::Px(1000.0)),
    ))
    .unwrap();
    assert!(matches!(spelled, Sizing::Calc(_)));
    assert_eq!(spelled, data);
    assert_eq!(spelled, built);
    assert_eq!(spelled.describe(), "clamp(400px, 80%, 1000px)");
}

/// A float anchored to a node by key is sized in the sixth pass, against
/// its anchor's final rect; pass 1 met it with a room of 0 and wrote its
/// size-expression clamps over as px of that, so the anchored pass found
/// no expression left and a `maxWidth "50%"` held the float to 0 — and a
/// `maxWidth "100%"` under it, resolved against the float's 0 in pass 1,
/// held its child to 0 as well (backlog RG77).
#[test]
fn a_node_anchored_float_s_clamps_take_the_anchor() {
    let mut t = T::new(NodeSpec::column().width(Sizing::Fixed(400.0)));
    let anchor = t.node(
        0,
        NodeSpec::column()
            .width(Sizing::Fixed(200.0))
            .height(Sizing::Fixed(10.0)),
    );
    let k = t.tree.keys[anchor as usize];
    let on_node = || FloatConfig {
        anchor: FloatAnchor::Node(k),
        ..FloatConfig::parent()
    };
    let held = t.node(
        0,
        NodeSpec::column()
            .float(on_node())
            .width(Sizing::Fixed(300.0))
            .max_width(max_str("50%").unwrap())
            .max_height(max_str("min(50%, 100px)").unwrap())
            .height(Sizing::Fixed(300.0)),
    );
    let grow = t.node(0, NodeSpec::column().float(on_node()).width(Sizing::GROW));
    let in_grow = t.node(
        grow,
        NodeSpec::column()
            .width(Sizing::Fixed(300.0))
            .max_width(max_str("100%").unwrap()),
    );
    let pct = t.node(0, NodeSpec::column().float(on_node()).width(s("100%")));
    let in_pct = t.node(
        pct,
        NodeSpec::column()
            .width(Sizing::Fixed(300.0))
            .max_width(max_str("100%").unwrap())
            .min_width(min_str("clamp(10px, 25%, 90px)").unwrap()),
    );
    t.run(1000.0);
    assert_eq!(t.w(held), 100.0, "300 held to half the anchor's 200");
    assert_eq!(t.tree.size[held as usize].h, 5.0, "half the anchor's 10");
    assert_eq!(t.w(grow), 200.0, "a grow float is its anchor's width");
    assert_eq!(
        t.w(in_grow),
        200.0,
        "held to the float's 200, not pass 1's 0"
    );
    assert_eq!(t.w(pct), 200.0);
    assert_eq!(t.w(in_pct), 200.0);
    assert_eq!(
        t.tree.specs[in_pct as usize].layout.min_w.resolved(),
        50.0,
        "the floor, 25% of 200, as the anchored pass wrote it"
    );
    // Laid out again, as the next frame's tree is: the same numbers.
    t.run(1000.0);
    assert_eq!(t.w(held), 100.0);
    assert_eq!(t.w(in_grow), 200.0);
}
