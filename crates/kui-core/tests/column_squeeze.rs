//! A column of fit children that overflows gives no child less height
//! than its content (backlog F114): CSS's `min-height: auto`. Squeezed
//! toward 0 as a row's labels are across, a row holding a text was
//! shorter than the text, which then painted over the rows under it — a
//! pane's header in a short window, in kawoosh. A child that scrolls or
//! clips down still gives to 0, so a list in a dialog still takes what
//! the header leaves it, and a declared `min_h` still wins.
//!
//! Driven through `layout::compute` as `relative_shrink.rs` is.

use kui_core::Size;
use kui_core::key::Key;
use kui_core::layout::{TextMeasure, compute};
use kui_core::resources::ImageId;
use kui_core::scroll::ScrollStore;
use kui_core::spec::{NodeSpec, Sizing};
use kui_core::tree::{NIL, NodeContent, OriginId, TextId, Tree};

/// A text is 10 px a character and one 20 px line.
struct Stub;

impl TextMeasure for Stub {
    fn intrinsic(&mut self, id: TextId) -> Size {
        Size::new(10.0 * id.0 as f32, 20.0)
    }
    fn wrapped(&mut self, id: TextId, _max_w: f32) -> Size {
        Size::new(10.0 * id.0 as f32, 20.0)
    }
    fn min_content(&mut self, id: TextId) -> f32 {
        10.0 * id.0 as f32
    }
    fn image_size(&mut self, _id: ImageId) -> Size {
        Size::new(0.0, 0.0)
    }
}

struct T {
    tree: Tree,
}

impl T {
    /// The root: `spec`, 300 px wide.
    fn new(spec: NodeSpec) -> Self {
        let mut tree = Tree::new();
        tree.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            spec.width(300.0),
            NodeContent::Container,
        );
        T { tree }
    }

    fn node(&mut self, parent: u32, spec: NodeSpec) -> u32 {
        let key = Key::ROOT.index(self.tree.len() as u64);
        self.tree
            .push(parent, key, OriginId::HOST, spec, NodeContent::Container)
    }

    fn text(&mut self, parent: u32, chars: u32) -> u32 {
        let key = Key::ROOT.index(self.tree.len() as u64);
        self.tree.push(
            parent,
            key,
            OriginId::HOST,
            NodeSpec::default(),
            NodeContent::Text(TextId(chars)),
        )
    }

    /// A fit row holding one text: 20 px of content.
    fn line(&mut self, parent: u32) -> u32 {
        let r = self.node(parent, NodeSpec::row());
        self.text(r, 5);
        r
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

    fn h(&self, i: u32) -> f32 {
        self.tree.size[i as usize].h
    }

    fn y(&self, i: u32) -> f32 {
        self.tree.pos[i as usize].y
    }
}

/// The case that filed it: rows of text in a column too short for them
/// keep their text's height and overflow the column, where they were
/// squeezed to a few px each and their texts overlapped.
#[test]
fn a_row_keeps_the_height_of_its_text() {
    let mut t = T::new(NodeSpec::column().height(50.0));
    let rows: Vec<u32> = (0..4).map(|_| t.line(0)).collect();
    t.run();
    for (k, r) in rows.iter().enumerate() {
        assert_eq!(t.h(*r), 20.0, "row {k}");
        assert_eq!(t.y(*r), 20.0 * k as f32, "row {k} under the one before");
    }
}

/// And so does a fit column of such rows, its floor theirs: the pane's
/// header over a list that grows, the list taking what is left — here
/// nothing — and the header whole.
#[test]
fn a_header_is_whole_over_a_list_that_grows() {
    let mut t = T::new(NodeSpec::column().height(50.0));
    let head = t.node(0, NodeSpec::column().gap(4.0));
    let lines: Vec<u32> = (0..3).map(|_| t.line(head)).collect();
    let list = t.node(0, NodeSpec::column().height(Sizing::Grow(1.0)));
    t.line(list);
    t.run();
    assert_eq!(t.h(head), 68.0, "three lines and two gaps");
    assert!(lines.iter().all(|l| t.h(*l) == 20.0));
    assert_eq!(t.h(list), 0.0, "the list takes what is left");
}

/// A child that scrolls down, or clips, gives to 0: a dialog's list
/// under its title takes what the title leaves, and a fit column holding
/// that list gives with it — its content's floor is the title alone.
#[test]
fn a_scroller_and_a_clip_still_give() {
    let mut t = T::new(NodeSpec::column().height(100.0));
    t.line(0);
    let list = t.node(0, NodeSpec::column().scroll_y());
    for _ in 0..10 {
        t.line(list);
    }
    t.run();
    assert_eq!(t.h(list), 80.0, "the scroller, under the 20 px title");

    let mut t = T::new(NodeSpec::column().height(100.0));
    t.line(0);
    let clip = t.node(0, NodeSpec::column().clip());
    for _ in 0..10 {
        t.line(clip);
    }
    t.run();
    assert_eq!(t.h(clip), 80.0, "the clip likewise");

    let mut t = T::new(NodeSpec::column().height(100.0));
    let body = t.node(0, NodeSpec::column());
    t.line(body);
    let list = t.node(body, NodeSpec::column().scroll_y());
    for _ in 0..10 {
        t.line(list);
    }
    t.run();
    assert_eq!(t.h(body), 100.0, "the column around them fits the root");
    assert_eq!(t.h(list), 80.0, "its scroller gives what it had to");
}

/// A declared floor wins, 0 included: `min_h(0.0)` asks for the squeeze
/// back, and a px floor under the content is where the child stops.
#[test]
fn a_declared_floor_wins() {
    let mut t = T::new(NodeSpec::column().height(30.0));
    let a = t.node(0, NodeSpec::column().min_height(0.0));
    t.line(a);
    t.line(a);
    let b = t.node(0, NodeSpec::column().min_height(10.0));
    t.line(b);
    t.line(b);
    t.run();
    assert_eq!(t.h(a) + t.h(b), 30.0, "{} + {}", t.h(a), t.h(b));
    assert!(t.h(b) >= 10.0, "b stops at its floor: {}", t.h(b));
}

/// Across nothing changes: a row of fit boxes holding labels squeezes
/// them largest first, below their text, into their ellipses.
#[test]
fn a_row_still_squeezes_its_labels() {
    let mut t = T::new(NodeSpec::row().height(20.0));
    let a = t.node(0, NodeSpec::row());
    t.text(a, 25);
    let b = t.node(0, NodeSpec::row());
    t.text(b, 10);
    t.run();
    let w = |i: u32| t.tree.size[i as usize].w;
    assert_eq!((w(a), w(b)), (200.0, 100.0), "the 250 px label pays the 50");
}

/// A row that wraps is as tall as its lines stacked, and gives no less:
/// four chips in two lines under a 5 px cross gap keep 45 px in a column
/// of 30, where one line's 20 was all its floor saw.
#[test]
fn a_wrapped_row_keeps_its_lines() {
    let mut t = T::new(NodeSpec::column().height(30.0));
    let chips = t.node(
        0,
        NodeSpec::row().grow_width().wrap().gap(10.0).cross_gap(5.0),
    );
    for _ in 0..4 {
        t.node(chips, NodeSpec::row().width(100.0).height(20.0));
    }
    let under = t.line(0);
    t.run();
    assert_eq!(t.h(chips), 45.0, "two lines and the gap between");
    assert_eq!(t.y(under), 45.0, "the row under it below it");
}
