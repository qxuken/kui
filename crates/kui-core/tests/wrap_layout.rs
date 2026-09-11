//! Flex wrapping: a row whose children don't fit the main axis breaks them
//! onto more lines instead of overflowing or shrinking.
//!
//! Driven through `layout::compute` against a deterministic `TextMeasure`
//! stub rather than through `Core`, so every number here is exact and none
//! of it depends on an installed font. The warnings at the bottom go
//! through a real `Core`, since that is where diagnostics are drained.

use kui_core::diag::WRAP_IGNORED;
use kui_core::key::Key;
use kui_core::layout::{TextMeasure, compute};
use kui_core::scroll::ScrollStore;
use kui_core::spec::{Align, NodeSpec, Sizing};
use kui_core::testing::codes;
use kui_core::tree::{NIL, NodeContent, OriginId, TextId, Tree};
use kui_core::{Core, Size, Vec2};

/// The same stub `layout.rs`'s own tests use: a text is `10px * chars`
/// wide and wraps into 20px lines, with the char count encoded in the id.
struct Stub;

impl TextMeasure for Stub {
    fn intrinsic(&mut self, id: TextId) -> Size {
        Size::new(10.0 * id.0 as f32, 20.0)
    }
    fn wrapped(&mut self, id: TextId, max_w: f32) -> Size {
        let full = 10.0 * id.0 as f32;
        if max_w <= 0.0 || full <= max_w {
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

    /// A fixed `w`x`h` child of `parent`.
    fn boxed(&mut self, parent: u32, w: f32, h: f32) -> u32 {
        self.node(parent, NodeSpec::column().width(px(w)).height(px(h)))
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

// --------------------------------------------------------------- breaking --

#[test]
fn a_child_that_no_longer_fits_starts_a_line() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)));
    let a = t.boxed(0, 40.0, 10.0);
    let b = t.boxed(0, 40.0, 10.0);
    let c = t.boxed(0, 40.0, 10.0);
    t.run();
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(40.0, 0.0));
    assert_eq!(t.pos(c), Vec2::new(0.0, 10.0), "c starts the second line");
    // A fit height is the lines stacked.
    assert_eq!(t.size(0), Size::new(100.0, 20.0));
}

#[test]
fn gap_runs_along_a_line_and_cross_gap_between_them() {
    let mut t = T::new(
        NodeSpec::row()
            .wrap()
            .width(px(100.0))
            .gap(10.0)
            .cross_gap(6.0),
    );
    let a = t.boxed(0, 40.0, 10.0);
    let b = t.boxed(0, 40.0, 10.0);
    let c = t.boxed(0, 40.0, 10.0);
    t.run();
    // 40 + 10 + 40 = 90 fits; a third would need 140.
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(50.0, 0.0));
    assert_eq!(t.pos(c), Vec2::new(0.0, 16.0));
    assert_eq!(t.size(0).h, 26.0, "10 + cross gap 6 + 10");
}

#[test]
fn a_fit_row_wraps_at_its_max_width() {
    // The responsive-width pattern: fit, but no wider than this, and the
    // overflow becomes lines rather than a squeeze.
    let mut t = T::new(NodeSpec::column());
    let row = t.node(0, NodeSpec::row().wrap().max_width(100.0));
    let a = t.boxed(row, 40.0, 10.0);
    let b = t.boxed(row, 40.0, 10.0);
    let c = t.boxed(row, 40.0, 10.0);
    t.run();
    assert_eq!(t.size(row), Size::new(100.0, 20.0));
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(40.0, 0.0));
    assert_eq!(t.pos(c), Vec2::new(0.0, 10.0));
    // Nothing was compressed to make it fit.
    for n in [a, b, c] {
        assert_eq!(t.size(n).w, 40.0);
    }
}

#[test]
fn percent_children_break_on_the_size_they_resolve_to() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(200.0)));
    let specs = |t: &mut T| {
        (0..3)
            .map(|_| {
                t.node(
                    0,
                    NodeSpec::column()
                        .width(Sizing::Percent(0.4))
                        .height(px(10.0)),
                )
            })
            .collect::<Vec<_>>()
    };
    let n = specs(&mut t);
    t.run();
    for c in &n {
        assert_eq!(t.size(*c).w, 80.0);
    }
    assert_eq!(t.pos(n[0]), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(n[1]), Vec2::new(80.0, 0.0));
    assert_eq!(t.pos(n[2]), Vec2::new(0.0, 10.0));
}

#[test]
fn floats_take_no_place_in_a_line() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)));
    let a = t.boxed(0, 60.0, 10.0);
    let f = t.node(
        0,
        NodeSpec::column()
            .width(px(200.0))
            .height(px(50.0))
            .float(kui_core::spec::FloatConfig::parent()),
    );
    let b = t.boxed(0, 60.0, 10.0);
    t.run();
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(0.0, 10.0), "the float didn't break it");
    assert_eq!(t.size(0).h, 20.0, "and doesn't grow the fit height");
    assert_eq!(t.pos(f), Vec2::new(0.0, 0.0));
}

// ------------------------------------------------------------------ grow --

#[test]
fn grow_fills_the_rest_of_its_own_line() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)));
    let a = t.boxed(0, 60.0, 10.0);
    let b = t.boxed(0, 60.0, 10.0);
    let c = t.node(
        0,
        NodeSpec::column().width(Sizing::Grow(1.0)).height(px(10.0)),
    );
    t.run();
    // a alone, then b and the grow child: the grow takes the 40 left of
    // *that* line, not of the container.
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(0.0, 10.0));
    assert_eq!(t.size(c).w, 40.0);
    assert_eq!(t.pos(c), Vec2::new(60.0, 10.0));
}

#[test]
fn cross_grow_fills_its_line_not_the_container() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)).height(px(200.0)));
    let a = t.boxed(0, 60.0, 20.0);
    let b = t.boxed(0, 60.0, 40.0);
    let c = t.node(
        0,
        NodeSpec::column().width(px(30.0)).height(Sizing::Grow(1.0)),
    );
    t.run();
    // Content extents 20 and 40; 140 left over, split evenly between the
    // two lines, so they are 90 and 110 tall.
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(0.0, 90.0));
    assert_eq!(t.pos(c), Vec2::new(60.0, 90.0));
    assert_eq!(t.size(c).h, 110.0);
}

/// CSS's `align-content: stretch`, and what makes the single-line identity
/// below hold for a container taller than its content.
#[test]
fn lines_share_the_containers_leftover_cross_space() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)).height(px(100.0)));
    let a = t.boxed(0, 60.0, 20.0);
    let b = t.boxed(0, 60.0, 40.0);
    t.run();
    // Extents 20 and 40 in a 100 box: 40 left over, 20 to each line, so
    // they start at 0 and 40.
    assert_eq!(t.pos(a).y, 0.0);
    assert_eq!(t.pos(b).y, 40.0);
}

#[test]
fn a_fit_height_leaves_the_lines_their_own_size() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)));
    let a = t.boxed(0, 60.0, 20.0);
    let b = t.boxed(0, 60.0, 40.0);
    let c = t.boxed(0, 30.0, 5.0);
    t.run();
    assert_eq!(t.size(0).h, 60.0, "20 + 40, nothing to share out");
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(0.0, 20.0));
    assert_eq!(t.pos(c), Vec2::new(60.0, 20.0));
}

// ------------------------------------------------------------- alignment --

/// The property that makes `wrapChildren` safe to leave on: with one line,
/// aligning the line inside the box and the child inside the line composes
/// back into exactly the unwrapped placement.
#[test]
fn a_row_that_fits_lays_out_exactly_like_an_unwrapped_one() {
    let build = |wrap: bool| {
        let mut root = NodeSpec::row()
            .width(px(300.0))
            .height(px(120.0))
            .pad(10.0)
            .gap(8.0)
            .main_align(Align::Center)
            .cross_align(Align::Center);
        if wrap {
            root = root.wrap().cross_gap(4.0);
        }
        let mut t = T::new(root);
        let a = t.boxed(0, 40.0, 10.0);
        let b = t.boxed(0, 40.0, 30.0);
        let c = t.node(
            0,
            NodeSpec::column().width(px(20.0)).height(Sizing::Grow(1.0)),
        );
        let d = t.node(
            0,
            NodeSpec::column().width(Sizing::Grow(1.0)).height(px(12.0)),
        );
        t.run();
        [a, b, c, d].map(|n| (t.size(n), t.pos(n)))
    };
    assert_eq!(build(false), build(true));
}

#[test]
fn main_align_places_every_line_on_its_own() {
    let mut t = T::new(
        NodeSpec::row()
            .wrap()
            .width(px(100.0))
            .height(px(50.0))
            .main_align(Align::End),
    );
    let a = t.boxed(0, 40.0, 10.0);
    let b = t.boxed(0, 40.0, 10.0);
    let c = t.boxed(0, 30.0, 10.0);
    t.run();
    // First line is 80 wide in a 100 box; the second is 30.
    assert_eq!(t.pos(a).x, 20.0);
    assert_eq!(t.pos(b).x, 60.0);
    assert_eq!(t.pos(c).x, 70.0);
}

#[test]
fn cross_align_places_a_child_inside_its_line() {
    let mut t = T::new(
        NodeSpec::row()
            .wrap()
            .width(px(100.0))
            .cross_align(Align::End),
    );
    let a = t.boxed(0, 60.0, 20.0);
    let b = t.boxed(0, 60.0, 40.0);
    let c = t.boxed(0, 30.0, 10.0);
    t.run();
    assert_eq!(t.pos(a).y, 0.0);
    assert_eq!(t.pos(b).y, 20.0);
    // Bottom of a 40-tall line that starts at 20.
    assert_eq!(t.pos(c).y, 50.0);
}

// ------------------------------------------------- wrapping versus shrink --

/// Wrapping and shrinking answer the same overflow, and wrapping answers
/// it first: a child that can move to the next line moves instead of being
/// compressed, and its neighbours keep their size.
#[test]
fn a_child_that_can_move_to_the_next_line_is_not_shrunk() {
    let build = |wrap: bool| {
        let mut root = NodeSpec::row().width(px(100.0));
        if wrap {
            root = root.wrap();
        }
        let mut t = T::new(root);
        // Fit-sized columns are what the shrink pass compresses.
        let a = t.node(0, NodeSpec::column());
        t.boxed(a, 80.0, 10.0);
        let b = t.node(0, NodeSpec::column());
        t.boxed(b, 140.0, 10.0);
        t.run();
        (t.size(a).w, t.size(b).w, t.pos(b))
    };
    // Unwrapped: 220 into 100, both squeezed to 50.
    assert_eq!(build(false), (50.0, 50.0, Vec2::new(50.0, 0.0)));
    // Wrapped: `a` keeps its size and `b` goes to a line of its own —
    // where it is still too wide, so *there* shrinking takes over.
    assert_eq!(build(true), (80.0, 100.0, Vec2::new(0.0, 10.0)));
}

#[test]
fn a_lone_child_shrinks_only_to_its_own_min() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)));
    let a = t.node(0, NodeSpec::column().min_width(120.0));
    t.boxed(a, 140.0, 10.0);
    let b = t.node(0, NodeSpec::column());
    t.boxed(b, 60.0, 10.0);
    t.run();
    assert_eq!(t.size(a).w, 120.0, "stops at its min and overflows");
    assert_eq!(t.size(b).w, 60.0, "and never pays for it");
    assert_eq!(t.pos(b), Vec2::new(0.0, 10.0));
}

// ------------------------------------------------------------------ text --

#[test]
fn text_children_break_between_lines() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)));
    let a = t.text(0, 5); // 50px
    let b = t.text(0, 6); // 60px
    let c = t.text(0, 3); // 30px
    t.run();
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(0.0, 20.0));
    assert_eq!(t.pos(c), Vec2::new(60.0, 20.0));
    assert_eq!(t.size(0).h, 40.0);
}

#[test]
fn text_too_long_for_the_box_gets_a_line_and_rewraps_in_it() {
    let mut t = T::new(NodeSpec::row().wrap().width(px(100.0)));
    let a = t.text(0, 4); // 40px
    let b = t.text(0, 20); // 200px: no break helps, so it shrinks and rewraps
    t.run();
    assert_eq!(t.size(a), Size::new(40.0, 20.0));
    assert_eq!(t.size(b), Size::new(100.0, 40.0), "two 20px lines");
    assert_eq!(t.pos(b), Vec2::new(0.0, 20.0));
    assert_eq!(t.size(0).h, 60.0, "the rewrap grows the row it is on");
}

// ---------------------------------------------------------------- scroll --

#[test]
fn wrapped_lines_scroll_across_the_main_axis() {
    let mut t = T::new(
        NodeSpec::row()
            .wrap()
            .scroll_y()
            .width(px(100.0))
            .height(px(15.0)),
    );
    t.boxed(0, 40.0, 10.0);
    t.boxed(0, 40.0, 10.0);
    t.boxed(0, 40.0, 10.0);
    t.run();
    // Two 10px lines in a 15px box.
    assert_eq!(t.tree.scroll_max[0], Vec2::new(0.0, 5.0));
}

// ------------------------------------------------- where it does not apply --

#[test]
fn a_column_lays_out_as_if_the_flag_were_absent() {
    let mut t = T::new(NodeSpec::column().wrap().width(px(50.0)).height(px(100.0)));
    let a = t.boxed(0, 50.0, 40.0);
    let b = t.boxed(0, 50.0, 40.0);
    let c = t.boxed(0, 50.0, 40.0);
    t.run();
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(0.0, 40.0));
    assert_eq!(t.pos(c), Vec2::new(0.0, 80.0), "stacked and overflowing");
}

#[test]
fn a_scrolling_main_axis_lays_out_as_if_the_flag_were_absent() {
    let mut t = T::new(
        NodeSpec::row()
            .wrap()
            .scroll_x()
            .width(px(100.0))
            .height(px(20.0)),
    );
    let a = t.boxed(0, 40.0, 10.0);
    let b = t.boxed(0, 40.0, 10.0);
    let c = t.boxed(0, 40.0, 10.0);
    t.run();
    assert_eq!(t.pos(a), Vec2::new(0.0, 0.0));
    assert_eq!(t.pos(b), Vec2::new(40.0, 0.0));
    assert_eq!(t.pos(c), Vec2::new(80.0, 0.0), "one row, scrolled");
    assert_eq!(t.tree.scroll_max[0], Vec2::new(20.0, 0.0));
}

// ------------------------------------------------------------- warnings --

fn frame(core: &mut Core, root: NodeSpec) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(root);
    for _ in 0..3 {
        ui.with(
            NodeSpec::column()
                .width(Sizing::Fixed(60.0))
                .height(Sizing::Fixed(20.0)),
            |_| {},
        );
    }
    ui.finish();
}

#[test]
fn wrapping_where_nothing_can_break_warns() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::column().wrap().fill());
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [WRAP_IGNORED]);
    assert!(
        ws[0].message.contains("only a row wraps"),
        "{}",
        ws[0].message
    );

    let mut core = Core::new();
    frame(&mut core, NodeSpec::row().wrap().scroll_x().fill());
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [WRAP_IGNORED]);
    assert!(ws[0].message.contains("scrollX"), "{}", ws[0].message);
}

#[test]
fn a_wrapping_row_says_nothing() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::row().wrap().fill());
    assert!(core.take_warnings().is_empty());
}
