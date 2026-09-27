//! The table (ADR 0033): a column whose rows' children line up in
//! columns, each column as wide as its widest cell.
//!
//! Driven through `layout::compute` against a deterministic `TextMeasure`
//! stub, as `wrap_layout.rs` is, so every number is exact. The warning at
//! the bottom goes through a real `Core`, where diagnostics are drained.

use kui_core::diag::WRAP_IGNORED;
use kui_core::key::Key;
use kui_core::layout::{TextMeasure, compute};
use kui_core::resources::ImageId;
use kui_core::scroll::ScrollStore;
use kui_core::spec::{Min, NodeSpec, Sizing};
use kui_core::testing::codes;
use kui_core::tree::{NIL, NodeContent, OriginId, TextId, Tree};
use kui_core::{Core, Size, Vec2};

/// A text is `10px * chars` wide and wraps into 20px lines, with the char
/// count encoded in the id. Given no width at all it folds to a char a
/// line, as the real measurer folds a paragraph into a zero-wide box.
struct Stub;

impl TextMeasure for Stub {
    fn intrinsic(&mut self, id: TextId) -> Size {
        Size::new(10.0 * id.0 as f32, 20.0)
    }
    fn wrapped(&mut self, id: TextId, max_w: f32) -> Size {
        let full = 10.0 * id.0 as f32;
        if max_w <= 0.0 {
            return Size::new(10.0, 20.0 * id.0 as f32);
        }
        if full <= max_w {
            return Size::new(full, 20.0);
        }
        Size::new(max_w, (full / max_w).ceil() * 20.0)
    }
    fn image_size(&mut self, id: ImageId) -> Size {
        let w = (id.to_ffi() & 0xffff_ffff) as f32;
        Size::new(w, w)
    }
}

/// An image handle whose low bits are its width: the stub answers every
/// image as a square of that size.
fn image_id(w: f32) -> ImageId {
    ImageId::from_ffi((1u64 << 32) | w as u64)
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

    /// A row of the table under `parent`, `grow` wide unless told otherwise.
    fn row(&mut self, parent: u32, spec: NodeSpec) -> u32 {
        self.node(parent, spec)
    }

    /// A fixed `w`x`h` cell.
    fn boxed(&mut self, parent: u32, w: f32, h: f32) -> u32 {
        self.node(parent, NodeSpec::column().width(px(w)).height(px(h)))
    }

    /// A fit cell around a fixed `w`x`h` box.
    fn fit(&mut self, parent: u32, w: f32, h: f32) -> u32 {
        let cell = self.node(parent, NodeSpec::row());
        self.boxed(cell, w, h);
        cell
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

    /// A `Fit` image `w` px square.
    fn image(&mut self, parent: u32, w: f32) -> u32 {
        let key = Key::ROOT.index(self.tree.len() as u64);
        self.tree.push(
            parent,
            key,
            OriginId::HOST,
            NodeSpec::default(),
            NodeContent::Image(image_id(w), Default::default()),
        )
    }

    fn run(&mut self) {
        let mut scroll = ScrollStore::default();
        compute(
            &mut self.tree,
            &mut Stub,
            &mut scroll,
            Size::new(1000.0, 1000.0),
            1.0,
        );
    }

    fn size(&self, i: u32) -> Size {
        self.tree.size[i as usize]
    }

    fn pos(&self, i: u32) -> Vec2 {
        self.tree.pos[i as usize]
    }
}

fn px(v: f32) -> Sizing {
    Sizing::Fixed(v)
}

fn grow() -> Sizing {
    Sizing::Grow(1.0)
}

fn row_spec() -> NodeSpec {
    NodeSpec::row().width(grow())
}

// ---------------------------------------------------------------- columns --

#[test]
fn a_column_is_as_wide_as_its_widest_cell() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let r1 = t.row(0, row_spec());
    let a1 = t.boxed(r1, 30.0, 10.0);
    let b1 = t.boxed(r1, 10.0, 10.0);
    let r2 = t.row(0, row_spec());
    let a2 = t.boxed(r2, 50.0, 12.0);
    let b2 = t.boxed(r2, 20.0, 12.0);
    let r3 = t.row(0, row_spec());
    let a3 = t.boxed(r3, 20.0, 8.0);
    let b3 = t.boxed(r3, 40.0, 8.0);
    t.run();
    // Column 0 is 50 (the widest of 30/50/20), column 1 is 40.
    for c in [a1, a2, a3] {
        assert_eq!(t.size(c).w, 50.0);
        assert_eq!(t.pos(c).x, 0.0);
    }
    for c in [b1, b2, b3] {
        assert_eq!(t.size(c).w, 40.0);
        assert_eq!(
            t.pos(c).x,
            50.0,
            "every second cell starts where column 1 does"
        );
    }
    // Heights are the rows' own.
    assert_eq!(t.size(a1).h, 10.0);
    assert_eq!(t.size(r2).h, 12.0);
    assert_eq!(t.size(0).h, 30.0, "10 + 12 + 8");
}

#[test]
fn fit_cells_align_too_and_a_fit_table_fits_the_aligned_rows() {
    let mut t = T::new(NodeSpec::table());
    let r1 = t.row(0, NodeSpec::row().gap(8.0));
    let a1 = t.fit(r1, 30.0, 10.0);
    let b1 = t.fit(r1, 10.0, 10.0);
    let r2 = t.row(0, NodeSpec::row().gap(8.0));
    let a2 = t.fit(r2, 50.0, 10.0);
    let b2 = t.fit(r2, 20.0, 10.0);
    t.run();
    assert_eq!(t.size(a1).w, 50.0);
    assert_eq!(t.size(a2).w, 50.0);
    assert_eq!(t.size(b1).w, 20.0);
    assert_eq!(t.size(b2).w, 20.0);
    assert_eq!(t.pos(b1).x, 58.0, "column 0, then the row's gap");
    assert_eq!(t.pos(b2).x, 58.0);
    // The fit rows and the fit table are the aligned sum: 50 + 8 + 20.
    assert_eq!(t.size(r1).w, 78.0);
    assert_eq!(t.size(r2).w, 78.0);
    assert_eq!(t.size(0).w, 78.0);
}

#[test]
fn a_grow_cell_makes_its_column_grow() {
    let mut t = T::new(NodeSpec::table().width(px(200.0)));
    let r1 = t.row(0, row_spec().gap(10.0));
    let a1 = t.boxed(r1, 30.0, 10.0);
    let b1 = t.node(r1, NodeSpec::row().width(grow()).height(px(10.0)));
    let r2 = t.row(0, row_spec().gap(10.0));
    let a2 = t.boxed(r2, 50.0, 10.0);
    let b2 = t.boxed(r2, 20.0, 10.0);
    t.run();
    // Column 0 fits at 50; column 1 grows into the rest: 200 - 50 - 10.
    assert_eq!(t.size(a1).w, 50.0);
    assert_eq!(t.size(a2).w, 50.0);
    assert_eq!(t.size(b1).w, 140.0);
    assert_eq!(
        t.size(b2).w,
        140.0,
        "a fixed cell in a growing column takes the column"
    );
    assert_eq!(t.pos(b1).x, 60.0);
    assert_eq!(t.pos(b2).x, 60.0);
}

#[test]
fn two_grow_columns_split_by_factor_and_a_clamped_one_is_frozen() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let r = t.row(0, row_spec());
    let a = t.boxed(r, 60.0, 10.0);
    let b = t.node(r, NodeSpec::row().grow_width().height(px(10.0)));
    let c = t.node(r, NodeSpec::row().width(Sizing::Grow(2.0)).height(px(10.0)));
    let r2 = t.row(0, row_spec());
    t.boxed(r2, 20.0, 10.0);
    t.node(
        r2,
        NodeSpec::row()
            .grow_width()
            .max_width(40.0)
            .height(px(10.0)),
    );
    t.run();
    assert_eq!(t.size(a).w, 60.0);
    // 240 left; column 1 would take 80 but its second row caps it at 40,
    // so it freezes there and column 2 takes the remaining 200.
    assert_eq!(t.size(b).w, 40.0);
    assert_eq!(t.size(c).w, 200.0);
}

#[test]
fn a_percent_column_takes_its_cut_of_the_row() {
    let mut t = T::new(NodeSpec::table().width(px(200.0)));
    let r = t.row(0, row_spec());
    let a = t.node(
        r,
        NodeSpec::row()
            .width(Sizing::Percent(0.25))
            .height(px(10.0)),
    );
    let b = t.node(r, NodeSpec::row().width(grow()).height(px(10.0)));
    let r2 = t.row(0, row_spec());
    let a2 = t.boxed(r2, 10.0, 10.0);
    t.run();
    assert_eq!(t.size(a).w, 50.0);
    assert_eq!(t.size(a2).w, 50.0);
    assert_eq!(t.size(b).w, 150.0);

    // The basis is the room the columns are laid across — the row's
    // content less its gaps — so two 50% columns with a gap fill the row
    // exactly; a plain row's percent child is its cut of the content
    // box with the gap on top, as CSS has it, and overflows (ADR 0033,
    // decision 10).
    let mut t = T::new(NodeSpec::table().width(px(200.0)));
    let r = t.row(0, row_spec().gap(20.0));
    let a = t.node(
        r,
        NodeSpec::row().width(Sizing::Percent(0.5)).height(px(10.0)),
    );
    let b = t.node(
        r,
        NodeSpec::row().width(Sizing::Percent(0.5)).height(px(10.0)),
    );
    t.run();
    assert_eq!(t.size(a).w, 90.0);
    assert_eq!(t.size(b).w, 90.0);
    assert_eq!(t.pos(b).x, 110.0, "the gap between, the row filled");
    let mut t = T::new(NodeSpec::column().width(px(200.0)));
    let r = t.node(0, row_spec().gap(20.0));
    let a = t.node(
        r,
        NodeSpec::row().width(Sizing::Percent(0.5)).height(px(10.0)),
    );
    let b = t.node(
        r,
        NodeSpec::row().width(Sizing::Percent(0.5)).height(px(10.0)),
    );
    t.run();
    assert_eq!(t.size(a).w, 100.0);
    assert_eq!(t.size(b).w, 100.0);
    assert_eq!(
        t.pos(b).x,
        120.0,
        "a plain row: the cut of the content box, the gap on top"
    );
}

#[test]
fn a_fixed_cell_sets_the_columns_floor() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let r1 = t.row(0, row_spec());
    let a1 = t.fit(r1, 30.0, 10.0);
    let r2 = t.row(0, row_spec());
    let a2 = t.boxed(r2, 80.0, 10.0);
    t.run();
    assert_eq!(t.size(a1).w, 80.0);
    assert_eq!(t.size(a2).w, 80.0);
}

#[test]
fn a_row_with_fewer_cells_fills_the_first_columns() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let r1 = t.row(0, row_spec());
    let a1 = t.boxed(r1, 30.0, 10.0);
    let b1 = t.boxed(r1, 30.0, 10.0);
    let c1 = t.boxed(r1, 30.0, 10.0);
    let r2 = t.row(0, row_spec());
    let a2 = t.boxed(r2, 60.0, 10.0);
    t.run();
    assert_eq!(t.size(a1).w, 60.0);
    assert_eq!(t.size(a2).w, 60.0);
    assert_eq!(t.pos(b1).x, 60.0);
    assert_eq!(t.pos(c1).x, 90.0);
}

// ------------------------------------------------------------------- text --

#[test]
fn a_bare_text_is_a_cell_and_keeps_its_columns_width() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let r1 = t.row(0, row_spec().gap(8.0));
    let k1 = t.text(r1, 3); // 30 wide
    let v1 = t.text(r1, 5);
    let r2 = t.row(0, row_spec().gap(8.0));
    let k2 = t.text(r2, 7); // 70 wide
    let v2 = t.text(r2, 2);
    t.run();
    // The label column is the longest label, and the shorter one is held
    // at that width so the value beside it starts where every value does.
    assert_eq!(t.size(k1).w, 70.0);
    assert_eq!(t.size(k2).w, 70.0);
    assert_eq!(t.pos(v1).x, 78.0);
    assert_eq!(t.pos(v2).x, 78.0);
    assert_eq!(t.size(k1).h, 20.0, "one line");
}

#[test]
fn a_text_wider_than_its_grown_column_wraps_inside_it() {
    let mut t = T::new(NodeSpec::table().width(px(100.0)));
    let r = t.row(0, row_spec());
    t.boxed(r, 40.0, 10.0);
    let v = t.node(r, NodeSpec::row().width(grow()));
    let text = t.text(v, 12); // 120 wide, in a 60 column
    t.run();
    assert_eq!(t.size(v).w, 60.0);
    assert_eq!(t.size(text), Size::new(60.0, 40.0), "two lines");
}

// ----------------------------------------------------------------- shrink --

#[test]
fn fit_columns_shrink_largest_first_when_the_row_is_too_narrow() {
    let mut t = T::new(NodeSpec::table().width(px(100.0)));
    let r = t.row(0, row_spec());
    let a = t.fit(r, 80.0, 10.0);
    let b = t.fit(r, 40.0, 10.0);
    let r2 = t.row(0, row_spec());
    let a2 = t.fit(r2, 20.0, 10.0);
    t.run();
    // 120 into 100: the 80 column pays the 20, and every row agrees.
    assert_eq!(t.size(a).w, 60.0);
    assert_eq!(t.size(a2).w, 60.0);
    assert_eq!(t.size(b).w, 40.0);
    assert_eq!(t.pos(b).x, 60.0);
}

#[test]
fn a_fixed_column_never_shrinks_and_a_floor_holds() {
    let mut t = T::new(NodeSpec::table().width(px(100.0)));
    let r = t.row(0, row_spec());
    let a = t.boxed(r, 80.0, 10.0);
    let b = t.node(r, NodeSpec::row().min_width(Min::px(30.0)).height(px(10.0)));
    t.boxed(b, 40.0, 10.0);
    t.run();
    assert_eq!(t.size(a).w, 80.0, "fixed");
    assert_eq!(t.size(b).w, 30.0, "down to its floor, not below");
}

#[test]
fn a_scrolling_table_overflows_instead() {
    let mut t = T::new(NodeSpec::table().width(px(100.0)).scroll_x());
    let r = t.row(0, row_spec());
    let a = t.fit(r, 80.0, 10.0);
    let b = t.fit(r, 40.0, 10.0);
    t.run();
    assert_eq!(t.size(a).w, 80.0);
    assert_eq!(t.size(b).w, 40.0);
}

// ------------------------------------------------------------------- rows --

#[test]
fn a_row_keeps_its_own_padding_gap_and_height() {
    let mut t = T::new(NodeSpec::table().width(px(200.0)).gap(4.0));
    let r1 = t.row(0, row_spec().pad_xy(8.0, 2.0).gap(6.0).height(px(24.0)));
    let a1 = t.boxed(r1, 30.0, 10.0);
    let b1 = t.boxed(r1, 10.0, 10.0);
    let r2 = t.row(0, row_spec().pad_xy(8.0, 2.0).gap(6.0));
    let a2 = t.boxed(r2, 50.0, 10.0);
    t.run();
    assert_eq!(t.pos(a1), Vec2::new(8.0, 2.0));
    assert_eq!(t.pos(b1).x, 8.0 + 50.0 + 6.0);
    assert_eq!(t.pos(a2), Vec2::new(8.0, 24.0 + 4.0 + 2.0));
    assert_eq!(t.size(r1).h, 24.0);
    assert_eq!(t.size(r2).h, 14.0);
}

#[test]
fn a_fit_row_beside_grow_rows_is_as_wide_as_the_columns() {
    let mut t = T::new(NodeSpec::table().width(px(200.0)));
    let r1 = t.row(0, row_spec());
    t.boxed(r1, 30.0, 10.0);
    t.node(r1, NodeSpec::row().width(grow()).height(px(10.0)));
    let r2 = t.row(0, NodeSpec::row());
    let a2 = t.boxed(r2, 30.0, 10.0);
    let b2 = t.boxed(r2, 10.0, 10.0);
    t.run();
    // Column 1 grew to 170 in the grow row; the fit row holds the same
    // columns, so it is 200 too, and its cells sit where the others do.
    assert_eq!(t.size(b2).w, 170.0);
    assert_eq!(t.size(r2).w, 200.0);
    assert_eq!(t.pos(a2).x, 0.0);
    assert_eq!(t.pos(b2).x, 30.0);
}

#[test]
fn a_float_in_a_row_is_not_a_cell() {
    let mut t = T::new(NodeSpec::table().width(px(200.0)));
    let r1 = t.row(0, row_spec());
    let a1 = t.boxed(r1, 30.0, 10.0);
    t.node(
        r1,
        NodeSpec::column()
            .width(px(90.0))
            .height(px(10.0))
            .float(kui_core::spec::FloatConfig::default()),
    );
    let b1 = t.boxed(r1, 10.0, 10.0);
    let r2 = t.row(0, row_spec());
    let a2 = t.boxed(r2, 30.0, 10.0);
    let b2 = t.boxed(r2, 20.0, 10.0);
    t.run();
    assert_eq!(
        t.size(b1).w,
        20.0,
        "column 1 is the two boxes, not the float"
    );
    assert_eq!(t.size(b2).w, 20.0);
    assert_eq!(t.pos(b1).x, 30.0);
    assert_eq!(t.pos(a2).x, 0.0);
    assert_eq!(t.size(a1).w, 30.0);
}

#[test]
fn a_table_in_a_cell_aligns_on_its_own() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let r = t.row(0, row_spec());
    t.boxed(r, 20.0, 10.0);
    let inner = t.node(r, NodeSpec::table());
    let ir1 = t.row(inner, NodeSpec::row());
    let ia1 = t.boxed(ir1, 10.0, 5.0);
    let ib1 = t.boxed(ir1, 10.0, 5.0);
    let ir2 = t.row(inner, NodeSpec::row());
    t.boxed(ir2, 40.0, 5.0);
    let r2 = t.row(0, row_spec());
    let a2 = t.boxed(r2, 60.0, 10.0);
    t.run();
    assert_eq!(t.size(ia1).w, 40.0, "the inner table's own column");
    assert_eq!(
        t.pos(ib1).x,
        60.0 + 40.0,
        "after the inner column 0, in the outer column 1"
    );
    assert_eq!(t.size(inner).w, 50.0, "fit: 40 + 10");
    assert_eq!(t.pos(inner).x, 60.0, "the outer column 0 is 60");
    assert_eq!(t.size(a2).w, 60.0);
}

// --------------------------------------------------------------- warnings --

#[test]
fn a_wrapping_table_row_warns_and_lays_out_as_a_row() {
    let mut core = Core::new();
    {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.with(NodeSpec::table().width(100.0), |ui| {
            ui.with(NodeSpec::row().wrap().width(grow()), |ui| {
                for _ in 0..3 {
                    ui.leaf(NodeSpec::column().size(60.0, 20.0));
                }
            });
        });
        ui.finish();
    }
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [WRAP_IGNORED]);
    assert!(
        ws[0].message.contains("a table's row cannot wrap"),
        "{}",
        ws[0].message
    );
}

#[test]
fn a_table_says_nothing() {
    let mut core = Core::new();
    {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.with(NodeSpec::table().width(grow()), |ui| {
            for (k, v) in [("a", "one"), ("abc", "three")] {
                ui.with(NodeSpec::row().width(grow()).gap(8.0), |ui| {
                    ui.text(k, kui_core::spec::TextStyle::new(12.0));
                    ui.text(v, kui_core::spec::TextStyle::new(12.0));
                });
            }
        });
        ui.finish();
    }
    assert!(core.take_warnings().is_empty());
}

// -------------------------------------------------------- anchored floats --

/// A `Min::FIT` floor inside a float anchored to a node by key (a
/// devtools tab's content) is measured twice: once in the five passes,
/// before the float has a width — a row of one long text folds into
/// many lines there — and again in the sixth, at the anchor's width.
/// The floor is written back as a number, so the second run has to be
/// handed the declaration again, or the first run's fold is a floor.
#[test]
fn a_fit_floor_in_a_node_anchored_float_is_measured_at_the_anchor() {
    let mut t = T::new(NodeSpec::column().width(px(400.0)).height(px(300.0)));
    // The anchor comes *after* the float in preorder: the five passes
    // cannot size the float against it.
    let float = t.node(
        0,
        NodeSpec::column()
            .width(grow())
            .height(grow())
            .float(kui_core::spec::FloatConfig {
                anchor: kui_core::spec::FloatAnchor::Node(Key::ROOT.str("body")),
                ..Default::default()
            }),
    );
    let bar = t.node(
        float,
        NodeSpec::row().width(grow()).min_height(Min::FIT).gap(6.0),
    );
    let text = t.text(bar, 20); // 200 wide, one 20px line at the anchor's width
    let _spacer = t.node(bar, NodeSpec::row().width(grow()));
    let body = t.tree.push(
        0,
        Key::ROOT.str("body"),
        OriginId::HOST,
        NodeSpec::column().width(px(300.0)).height(px(200.0)),
        NodeContent::Container,
    );
    t.run();
    assert_eq!(t.size(body).w, 300.0);
    assert_eq!(t.size(float).w, 300.0, "the float takes the anchor's width");
    assert_eq!(
        t.size(text),
        Size::new(200.0, 20.0),
        "one line at that width"
    );
    assert_eq!(
        t.size(bar).h,
        20.0,
        "the floor is that line, not the fold before it"
    );
}

#[test]
fn a_leaf_straight_under_a_table_is_not_a_row_to_size() {
    // A heading text as a child of the table, beside its rows: it has no
    // cells, and it keeps its own width.
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let heading = t.text(0, 8); // 80 wide
    let r = t.row(0, row_spec());
    t.boxed(r, 30.0, 10.0);
    t.boxed(r, 20.0, 10.0);
    t.run();
    assert_eq!(t.size(heading), Size::new(80.0, 20.0));
}

#[test]
fn a_floating_row_under_a_table_wraps_and_is_not_warned_about() {
    let mut core = Core::new();
    {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.with(NodeSpec::table().width(100.0), |ui| {
            ui.with(
                NodeSpec::row()
                    .wrap()
                    .width(100.0)
                    .float(kui_core::spec::FloatConfig::default()),
                |ui| {
                    for _ in 0..3 {
                        ui.leaf(NodeSpec::column().size(60.0, 20.0));
                    }
                },
            );
        });
        ui.finish();
    }
    assert!(core.take_warnings().is_empty());
}

// ---------------------------------------- the regression pass (RG3–RG11) --

/// RG3. A `scrollX` table's content is its columns, not its rows: a
/// `grow` row is exactly the table's width, and the overflow it keeps
/// past that must still be scrollable — so a row of a scrolling table
/// is at least as wide as its columns, and `scroll_max.x` sees them.
#[test]
fn a_scrolling_table_of_grow_rows_can_scroll_to_its_columns() {
    let mut t = T::new(NodeSpec::table().width(px(100.0)).scroll_x());
    let r = t.row(0, row_spec());
    let a = t.fit(r, 80.0, 10.0);
    let b = t.fit(r, 40.0, 10.0);
    t.run();
    assert_eq!(t.size(a).w, 80.0);
    assert_eq!(t.size(b).w, 40.0);
    assert_eq!(t.size(r).w, 120.0, "the row spans its cells");
    assert_eq!(
        t.tree.scroll_max[0].x, 20.0,
        "and the table can scroll to them"
    );
    // With the row's own chrome counted.
    let mut t = T::new(NodeSpec::table().width(px(100.0)).scroll_x());
    let r = t.row(0, row_spec().pad_xy(5.0, 0.0).gap(10.0));
    t.fit(r, 80.0, 10.0);
    t.fit(r, 40.0, 10.0);
    t.run();
    assert_eq!(t.size(r).w, 140.0, "80 + 10 + 40 + 2 * 5");
    assert_eq!(t.tree.scroll_max[0].x, 40.0);
}

#[test]
fn a_table_that_does_not_scroll_keeps_its_grow_rows_at_its_width() {
    // The fixed columns overflow the row as fixed children overflow any
    // row; the row's box is not widened for them.
    let mut t = T::new(NodeSpec::table().width(px(100.0)));
    let r = t.row(0, row_spec());
    t.boxed(r, 80.0, 10.0);
    t.boxed(r, 40.0, 10.0);
    t.run();
    assert_eq!(t.size(r).w, 100.0);
}

/// RG6. The floor fix (F75) handed the sixth pass the `Min::FIT`
/// declarations again, and then clamped the float's own size with a
/// spec copied *before* the re-run resolved them: a `fit` floor read
/// as 0, and the float lost its own floor. Its width comes from the
/// anchor, its floor from its content, and the floor wins.
#[test]
fn a_node_anchored_floats_own_fit_floor_holds_against_its_anchor() {
    let mut t = T::new(NodeSpec::column().width(px(400.0)).height(px(300.0)));
    let float = t.node(
        0,
        NodeSpec::column()
            .width(grow())
            .min_width(Min::FIT)
            .height(grow())
            .min_height(Min::FIT)
            .float(kui_core::spec::FloatConfig {
                anchor: kui_core::spec::FloatAnchor::Node(Key::ROOT.str("body")),
                ..Default::default()
            }),
    );
    t.boxed(float, 400.0, 250.0);
    let body = t.tree.push(
        0,
        Key::ROOT.str("body"),
        OriginId::HOST,
        NodeSpec::column().width(px(300.0)).height(px(200.0)),
        NodeContent::Container,
    );
    t.run();
    assert_eq!(t.size(body), Size::new(300.0, 200.0));
    assert_eq!(
        t.size(float),
        Size::new(400.0, 250.0),
        "the anchor is 300 x 200; the float's content floors it at 400 x 250"
    );
}

/// RG7. A row of a table is a *row*: a column straight under the table
/// is a child with its own width, as a text there is, and its stacked
/// children are not cells.
#[test]
fn a_column_under_a_table_is_not_a_row_and_its_children_are_not_cells() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let section = t.node(0, NodeSpec::column());
    let heading = t.text(section, 8); // 80 wide
    let inner = t.node(section, NodeSpec::row());
    let wide = t.boxed(inner, 100.0, 10.0);
    let r = t.row(0, row_spec());
    let a = t.boxed(r, 30.0, 10.0);
    let b = t.boxed(r, 20.0, 10.0);
    t.run();
    assert_eq!(t.size(a).w, 30.0, "column 0 is the one row's cell");
    assert_eq!(t.size(b).w, 20.0);
    assert_eq!(t.pos(b).x, 30.0);
    assert_eq!(t.size(heading).w, 80.0, "the section's own children");
    assert_eq!(t.size(wide).w, 100.0);
    assert_eq!(t.size(section).w, 100.0, "fit: the wider of its two");
}

#[test]
fn a_table_straight_under_a_table_keeps_its_own_rows() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let inner = t.node(0, NodeSpec::table());
    let ir = t.row(inner, NodeSpec::row());
    let ia = t.boxed(ir, 10.0, 5.0);
    let ib = t.boxed(ir, 10.0, 5.0);
    let ir2 = t.row(inner, NodeSpec::row());
    t.boxed(ir2, 40.0, 5.0);
    let r = t.row(0, row_spec());
    let a = t.boxed(r, 60.0, 10.0);
    t.run();
    assert_eq!(t.size(ia).w, 40.0, "the inner table's column 0");
    assert_eq!(t.pos(ib).x, 40.0);
    assert_eq!(t.size(inner).w, 50.0);
    assert_eq!(t.size(a).w, 60.0, "not widened by the inner rows");
}

#[test]
fn the_table_flag_on_a_row_is_a_row() {
    // Reachable from Rust's public fields only: the flag is read on a
    // column, and a row carrying it lays out as the row it is.
    let mut spec = NodeSpec::row().width(px(300.0));
    spec.layout.table = true;
    let mut t = T::new(spec);
    let a = t.node(0, NodeSpec::row().width(grow()).height(px(10.0)));
    let b = t.boxed(0, 100.0, 10.0);
    t.run();
    assert_eq!(t.size(a).w, 200.0, "grows as a row's child does");
    assert_eq!(t.pos(b).x, 200.0);
}

/// RG8. An image straight in a row is a cell held to its column, and
/// its height is its own aspect at its *own* width — a 16 px icon in a
/// 200 px column is a 200 x 16 box (the pixels meet it by the image's
/// `fit` row), not a 200 x 200 one.
#[test]
fn an_image_cell_keeps_its_own_height() {
    let mut t = T::new(NodeSpec::table().width(px(300.0)));
    let r1 = t.row(0, row_spec());
    let icon = t.image(r1, 16.0); // 16 x 16
    t.boxed(r1, 30.0, 10.0);
    let r2 = t.row(0, row_spec());
    t.boxed(r2, 200.0, 10.0);
    let r3 = t.row(0, row_spec());
    let fixed = t.tree.push(
        r3,
        Key::ROOT.str("fixed"),
        OriginId::HOST,
        NodeSpec::default().width(px(32.0)),
        NodeContent::Image(image_id(16.0), Default::default()),
    );
    t.run();
    assert_eq!(
        t.size(icon),
        Size::new(200.0, 16.0),
        "the column, its own height"
    );
    assert_eq!(t.size(r1).h, 16.0);
    assert_eq!(
        t.size(fixed),
        Size::new(200.0, 32.0),
        "a fixed-width image's height is its aspect at that width"
    );
}

/// RG11. A `Fit` table's width is its columns' — the rows are the
/// table's, and their `grow` says how the columns share the table, not
/// that the table is nothing. The howto's key/value snippet is exactly
/// this shape.
#[test]
fn a_fit_table_of_grow_rows_is_as_wide_as_its_columns() {
    let mut t = T::new(NodeSpec::table());
    let r1 = t.row(0, row_spec().gap(8.0));
    let k1 = t.text(r1, 5);
    let v1 = t.text(r1, 3);
    let r2 = t.row(0, row_spec().gap(8.0));
    let k2 = t.text(r2, 2);
    let v2 = t.text(r2, 7);
    t.run();
    assert_eq!(t.size(0).w, 50.0 + 8.0 + 70.0, "the aligned columns");
    assert_eq!(t.size(r1).w, 128.0);
    assert_eq!(t.size(r2).w, 128.0);
    assert_eq!(t.size(k1), Size::new(50.0, 20.0), "one line");
    assert_eq!(t.size(k2).w, 50.0);
    assert_eq!(t.pos(v1).x, 58.0);
    assert_eq!(t.pos(v2).x, 58.0);
    assert_eq!(t.size(v2), Size::new(70.0, 20.0));
    // A `min: fit` floor on the table reads the same width.
    let mut t = T::new(NodeSpec::table().width(grow()).min_width(Min::FIT));
    let r = t.row(0, row_spec());
    t.boxed(r, 40.0, 10.0);
    t.boxed(r, 60.0, 10.0);
    t.run();
    assert_eq!(t.size(0).w, 1000.0, "grows to the viewport");
}

#[test]
fn a_fit_table_in_a_narrow_parent_holds_its_columns_floor() {
    let mut t = T::new(NodeSpec::row().width(px(50.0)));
    let table = t.node(0, NodeSpec::table().width(grow()).min_width(Min::FIT));
    let r = t.row(table, row_spec());
    let a = t.boxed(r, 40.0, 10.0);
    let b = t.boxed(r, 60.0, 10.0);
    t.run();
    assert_eq!(t.size(table).w, 100.0, "the floor is the columns");
    assert_eq!(t.size(a).w, 40.0);
    assert_eq!(t.pos(b).x, 40.0);
}
