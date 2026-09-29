//! A share of the room gives when the room is spent (backlog F110): a
//! `Percent` or a size expression in a row that overflows — two `"50%"`
//! children and the gap between them — compresses toward its floor with
//! the `Fit` children, largest first, as a flex item shrinks in CSS. A
//! `Fixed` child keeps its size, and a scrolling row overflows on purpose.
//!
//! Driven through `layout::compute` as `table_layout.rs` is.

use kui_core::Size;
use kui_core::key::Key;
use kui_core::layout::{TextMeasure, compute};
use kui_core::resources::ImageId;
use kui_core::schema::sizing_str;
use kui_core::scroll::ScrollStore;
use kui_core::spec::{Min, NodeSpec, Sizing};
use kui_core::tree::{NIL, NodeContent, OriginId, TextId, Tree};

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
    /// A row `w` px wide with `gap` between its children.
    fn row(w: f32, gap: f32) -> Self {
        let mut tree = Tree::new();
        tree.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            NodeSpec::row().width(w).gap(gap),
            NodeContent::Container,
        );
        T { tree }
    }

    fn child(&mut self, spec: NodeSpec) -> u32 {
        let key = Key::ROOT.index(self.tree.len() as u64);
        self.tree.push(
            0,
            key,
            OriginId::HOST,
            spec.height(10.0),
            NodeContent::Container,
        )
    }

    fn run(&mut self) {
        let mut scroll = ScrollStore::default();
        compute(
            &mut self.tree,
            &mut Stub,
            &mut scroll,
            Size::new(1000.0, 600.0),
            1.0,
        );
    }

    fn w(&self, i: u32) -> f32 {
        self.tree.size[i as usize].w
    }
}

fn pct(p: f32) -> Sizing {
    Sizing::Percent(p / 100.0)
}

/// The case that filed it: two halves and a 16 px gap in 500 px.
#[test]
fn two_halves_and_a_gap_fit_their_row() {
    let mut t = T::row(500.0, 16.0);
    let a = t.child(NodeSpec::column().width(pct(50.0)));
    let b = t.child(NodeSpec::column().width(pct(50.0)));
    t.run();
    assert_eq!((t.w(a), t.w(b)), (242.0, 242.0));
}

/// Largest first, down to each floor: the 60% share pays until it meets
/// the 40% one, and a floored one stops at its floor.
#[test]
fn the_largest_share_pays_first_down_to_its_floor() {
    let mut t = T::row(500.0, 0.0);
    let big = t.child(NodeSpec::column().width(pct(60.0)));
    let small = t.child(NodeSpec::column().width(pct(40.0)));
    let extra = t.child(NodeSpec::column().width(Sizing::Fixed(100.0)));
    t.run();
    assert_eq!(t.w(extra), 100.0, "a fixed child keeps its size");
    assert_eq!(t.w(big) + t.w(small), 400.0, "the shares paid the 100");
    assert_eq!(
        (t.w(big), t.w(small)),
        (200.0, 200.0),
        "the larger first, to equal"
    );

    let mut t = T::row(500.0, 0.0);
    let floored = t.child(
        NodeSpec::column()
            .width(pct(60.0))
            .min_width(Min::px(280.0)),
    );
    let other = t.child(NodeSpec::column().width(pct(60.0)));
    t.run();
    assert_eq!((t.w(floored), t.w(other)), (280.0, 220.0));
}

/// A size expression gives as a percentage does.
#[test]
fn a_size_expression_gives_too() {
    let mut t = T::row(500.0, 20.0);
    let s = sizing_str("clamp(100px, 60%, 400px)").unwrap();
    let a = t.child(NodeSpec::column().width(s));
    let b = t.child(NodeSpec::column().width(s));
    t.run();
    assert_eq!((t.w(a), t.w(b)), (240.0, 240.0));
}

/// Overflow is a scrolling row's point: nothing gives there.
#[test]
fn a_scrolling_row_keeps_its_shares() {
    let mut tree = Tree::new();
    tree.push(
        NIL,
        Key::ROOT,
        OriginId::HOST,
        NodeSpec::row().width(500.0).gap(16.0).scroll_x(),
        NodeContent::Container,
    );
    let mut t = T { tree };
    let a = t.child(NodeSpec::column().width(pct(50.0)));
    let b = t.child(NodeSpec::column().width(pct(50.0)));
    t.run();
    assert_eq!((t.w(a), t.w(b)), (250.0, 250.0));
}
