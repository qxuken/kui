//! The table (ADR 0033): a column whose rows' children line up in
//! columns, each column as wide as its widest cell.
//!
//! Driven through `layout::compute` against a deterministic `TextMeasure`
//! stub, as `wrap_layout.rs` is, so every number is exact. The warning at
//! the bottom goes through a real `Core`, where diagnostics are drained.

use kui_core::diag::WRAP_IGNORED;
use kui_core::key::Key;
use kui_core::layout::{TextMeasure, compute};
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
    let b = t.node(r, NodeSpec::row().width(Sizing::Grow(1.0)).height(px(10.0)));
    let c = t.node(r, NodeSpec::row().width(Sizing::Grow(2.0)).height(px(10.0)));
    let r2 = t.row(0, row_spec());
    t.boxed(r2, 20.0, 10.0);
    t.node(
        r2,
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
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
        ui.with(NodeSpec::table().width(Sizing::Fixed(100.0)), |ui| {
            ui.with(NodeSpec::row().wrap().width(grow()), |ui| {
                for _ in 0..3 {
                    ui.with(
                        NodeSpec::column()
                            .width(Sizing::Fixed(60.0))
                            .height(Sizing::Fixed(20.0)),
                        |_| {},
                    );
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
        ui.with(NodeSpec::table().width(Sizing::Fixed(100.0)), |ui| {
            ui.with(
                NodeSpec::row()
                    .wrap()
                    .width(Sizing::Fixed(100.0))
                    .float(kui_core::spec::FloatConfig::default()),
                |ui| {
                    for _ in 0..3 {
                        ui.with(
                            NodeSpec::column()
                                .width(Sizing::Fixed(60.0))
                                .height(Sizing::Fixed(20.0)),
                            |_| {},
                        );
                    }
                },
            );
        });
        ui.finish();
    }
    assert!(core.take_warnings().is_empty());
}
