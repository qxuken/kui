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
    /// A text's longest word: three tenths of its line.
    fn min_content(&mut self, id: TextId) -> f32 {
        3.0 * id.0 as f32
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

/// In proportion to its size, down to each floor, as CSS's flex items
/// give (backlog RG92): the 60% share pays three fifths of the 100, and a
/// floored one stops at its floor while the other pays the rest.
#[test]
fn a_share_pays_in_proportion_down_to_its_floor() {
    let mut t = T::row(500.0, 0.0);
    let big = t.child(NodeSpec::column().width(pct(60.0)));
    let small = t.child(NodeSpec::column().width(pct(40.0)));
    let extra = t.child(NodeSpec::column().width(Sizing::Fixed(100.0)));
    t.run();
    assert_eq!(t.w(extra), 100.0, "a fixed child keeps its size");
    assert_eq!(
        (t.w(big), t.w(small)),
        (240.0, 160.0),
        "each by its share of the two"
    );
    // The review's case: 30% and 70% of 600 beside a fixed 300 keep
    // their ratio, where largest-first made them 150 and 150.
    let mut t = T::row(600.0, 0.0);
    t.child(NodeSpec::column().width(Sizing::Fixed(300.0)));
    let a = t.child(NodeSpec::column().width(pct(30.0)));
    let b = t.child(NodeSpec::column().width(pct(70.0)));
    t.run();
    assert_eq!((t.w(a), t.w(b)), (90.0, 210.0));

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

impl T {
    /// A column `h` px tall with `gap` between its children.
    fn column(h: f32, gap: f32) -> Self {
        let mut tree = Tree::new();
        tree.push(
            NIL,
            Key::ROOT,
            OriginId::HOST,
            NodeSpec::column().width(100.0).height(h).gap(gap),
            NodeContent::Container,
        );
        T { tree }
    }

    fn under(&mut self, parent: u32, spec: NodeSpec) -> u32 {
        let key = Key::ROOT.index(self.tree.len() as u64);
        self.tree
            .push(parent, key, OriginId::HOST, spec, NodeContent::Container)
    }

    fn text(&mut self, parent: u32, id: u32) -> u32 {
        let key = Key::ROOT.index(self.tree.len() as u64);
        self.tree.push(
            parent,
            key,
            OriginId::HOST,
            NodeSpec::column(),
            NodeContent::Text(TextId(id)),
        )
    }

    fn h(&self, i: u32) -> f32 {
        self.tree.size[i as usize].h
    }
}

/// A share with nothing declared stops at its content, CSS's
/// `min-width: auto` (backlog RG92): a `50%` column holding a 120 px
/// button beside a fixed 250 in 300 gives only down to 120 and the row
/// overflows the rest, where it went to 50 with the button spilling out.
#[test]
fn a_share_stops_at_its_content() {
    let mut t = T::row(300.0, 0.0);
    t.child(NodeSpec::column().width(Sizing::Fixed(250.0)));
    let col = t.child(NodeSpec::column().width(pct(50.0)));
    t.under(
        col,
        NodeSpec::column().width(Sizing::Fixed(120.0)).height(10.0),
    );
    t.run();
    assert_eq!(t.w(col), 120.0);

    // Its content is the widest thing that cannot wrap: a text's longest
    // word (the stub's three tenths of its 200 px line), not its line.
    let mut t = T::row(300.0, 0.0);
    t.child(NodeSpec::column().width(Sizing::Fixed(250.0)));
    let col = t.child(NodeSpec::column().width(pct(50.0)));
    t.text(col, 20);
    t.run();
    assert_eq!(t.w(col), 60.0, "the longest word, 60");

    // A row's content adds up, with its gaps and padding.
    let mut t = T::row(300.0, 0.0);
    t.child(NodeSpec::column().width(Sizing::Fixed(250.0)));
    let col = t.child(NodeSpec::row().width(pct(50.0)).gap(4.0).pad(3.0));
    t.under(
        col,
        NodeSpec::column().width(Sizing::Fixed(40.0)).height(10.0),
    );
    t.under(
        col,
        NodeSpec::column().width(Sizing::Fixed(30.0)).height(10.0),
    );
    t.run();
    assert_eq!(t.w(col), 80.0, "40 + 4 + 30 and 3 either side");
}

/// A declared `minWidth: 0` lets a share go below its content, as CSS's
/// `min-width: 0` does, and so does scrolling its overflow; a declared
/// floor is the floor, content or not (backlog RG92).
#[test]
fn min_width_zero_and_a_scroller_let_a_share_go() {
    let build = |spec: NodeSpec| {
        let mut t = T::row(300.0, 0.0);
        t.child(NodeSpec::column().width(Sizing::Fixed(250.0)));
        let col = t.child(spec.width(pct(50.0)));
        t.under(
            col,
            NodeSpec::column().width(Sizing::Fixed(120.0)).height(10.0),
        );
        t.run();
        t.w(col)
    };
    assert_eq!(build(NodeSpec::column().min_width(Min::px(0.0))), 50.0);
    assert_eq!(build(NodeSpec::column().scroll_x()), 50.0);
    assert_eq!(build(NodeSpec::column().min_width(Min::px(90.0))), 90.0);
    // Undeclared survives the trip through `min_width`.
    assert_eq!(build(NodeSpec::column().min_width(Min::AUTO)), 120.0);
}

/// In a row holding a share, a fit child gives by the same rule, in
/// proportion and down to its content: a 200 px text beside a `50%` in
/// 300 pays 200/350 of the 50 (backlog RG92). A row of fit children
/// alone keeps the largest-first rule.
#[test]
fn a_fit_child_beside_a_share_gives_as_it_does() {
    // A fit box around each text, as a row holds a label: the stub's
    // `wrapped` ignores the width, so a bare text would read its line
    // back after the shrink.
    let mut t = T::row(300.0, 0.0);
    let label = t.child(NodeSpec::column());
    t.text(label, 20);
    let half = t.child(NodeSpec::column().width(pct(50.0)));
    t.run();
    let (lw, hw) = (t.w(label), t.w(half));
    assert!((lw - (200.0 - 50.0 * 200.0 / 350.0)).abs() < 0.01, "{lw}");
    assert!((hw - (150.0 - 50.0 * 150.0 / 350.0)).abs() < 0.01, "{hw}");

    let mut t = T::row(200.0, 0.0);
    let a = t.child(NodeSpec::column());
    t.text(a, 20);
    let b = t.child(NodeSpec::column());
    t.text(b, 10);
    t.run();
    assert_eq!((t.w(a), t.w(b)), (100.0, 100.0), "largest first, as before");
}

/// The height axis the same way: a `50%` box in a column 300 tall beside
/// a fixed 250 stops at the 120 its child needs.
#[test]
fn a_share_of_a_column_stops_at_its_content() {
    let mut t = T::column(300.0, 0.0);
    t.under(0, NodeSpec::column().height(Sizing::Fixed(250.0)));
    let half = t.under(0, NodeSpec::column().height(pct(50.0)));
    t.under(
        half,
        NodeSpec::column().width(10.0).height(Sizing::Fixed(120.0)),
    );
    t.run();
    assert_eq!(t.h(half), 120.0);
}

/// With real text: a `50%` column holding a sentence gives down to its
/// longest word, measured off the shaped glyphs, and not below it
/// (backlog RG92).
#[test]
fn a_share_of_real_text_stops_at_its_longest_word() {
    use kui_core::{Core, TextStyle, Value};
    let mut core = Core::new();
    let style = TextStyle::new(14.0);
    let word = core.measure_text("breakfast", &style, None).width;
    let line = core.measure_text("a breakfast is ok", &style, None).width;
    assert!(word < line * 0.8, "a word well short of the line");
    let mut col = Key::ROOT;
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.with(NodeSpec::row().width(300.0), |ui| {
            ui.leaf(NodeSpec::column().width(290.0).height(10.0));
            col = ui.with_keyed(
                "col",
                NodeSpec::column().width(pct(50.0)).on_layout(Value::Null),
                |ui| {
                    ui.text("a breakfast is ok", style);
                },
            );
        });
        ui.finish();
    }
    let w = core.layout_of(col).expect("laid out").w;
    assert!((w - word).abs() < 0.5, "the column {w}, the word {word}");
}
