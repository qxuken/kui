//! A fit box across a column is its content's width but no wider than the
//! column's box (backlog F116): CSS's `fit-content`. It was its content's
//! width whatever room there was, so a text one wrapper deep in a card
//! with a `maxWidth` ran past the card on one line, where the same text
//! straight in the card wrapped. A fit child that fits keeps its own
//! width; a declared `minWidth` wins; a column that scrolls x, a fixed
//! child, a width a ratio derives, a terminal grid and an image keep
//! theirs; heights are not held.
//!
//! Driven through `layout::compute` as `column_squeeze.rs` is.

use kui_core::cells::CellsId;
use kui_core::key::Key;
use kui_core::layout::{TextMeasure, compute};
use kui_core::resources::{ImageId, ImageOpts};
use kui_core::scroll::ScrollStore;
use kui_core::spec::{Align, FloatConfig, Min, NodeSpec};
use kui_core::tree::{NIL, NodeContent, OriginId, TextId, Tree};
use kui_core::{Size, Vec2};

/// A text is 10 px a character on one 20 px line, and wraps into as many
/// 20 px lines as the width it is given takes.
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
    /// Every image 400 x 100.
    fn image_size(&mut self, _id: ImageId) -> Size {
        Size::new(400.0, 100.0)
    }
    /// Every grid 400 x 100: forty columns the app laid out, five rows.
    fn cells_size(&mut self, _id: CellsId) -> Size {
        Size::new(400.0, 100.0)
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

    /// A column capped at 200 under the root: the card.
    fn card(&mut self, spec: NodeSpec) -> u32 {
        self.node(0, spec.max_width(200.0))
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

    fn size(&self, i: u32) -> Size {
        self.tree.size[i as usize]
    }

    fn pos(&self, i: u32) -> Vec2 {
        self.tree.pos[i as usize]
    }
}

/// The case that filed it: a 400 px text in a fit column, and in a fit
/// row, inside a card capped at 200 wraps as it does straight in the card,
/// and the card is as tall as the wrapped lines.
#[test]
fn a_wrapper_in_a_capped_card_is_held_to_it_and_its_text_wraps() {
    for wrapper in [None, Some(NodeSpec::column()), Some(NodeSpec::row())] {
        let mut t = T::new(NodeSpec::column());
        let card = t.card(NodeSpec::column());
        let holder = match wrapper {
            Some(spec) => t.node(card, spec),
            None => card,
        };
        let text = t.text(holder, 40);
        t.run();
        assert_eq!(t.size(card), Size::new(200.0, 40.0));
        assert_eq!(t.size(holder), Size::new(200.0, 40.0));
        assert_eq!(t.size(text), Size::new(200.0, 40.0), "two lines");
    }
}

/// Held, the wrapper's own run gives along it: a message beside a 50 px
/// button in a fit row of the card takes the 150 left and wraps in it,
/// and the button keeps its width. Two wrappers deep is the same.
#[test]
fn the_run_in_a_held_wrapper_gives_along_it() {
    let mut t = T::new(NodeSpec::column());
    let card = t.card(NodeSpec::column());
    let outer = t.node(card, NodeSpec::column());
    let row = t.node(outer, NodeSpec::row());
    let text = t.text(row, 40);
    let button = t.node(row, NodeSpec::row().size(50.0, 20.0));
    t.run();
    assert_eq!(t.size(outer).w, 200.0);
    assert_eq!(t.size(row).w, 200.0);
    assert_eq!(t.size(button).w, 50.0);
    assert_eq!(
        t.size(text),
        Size::new(150.0, 60.0),
        "400 in 150: three lines"
    );
    assert_eq!(t.size(card).h, 60.0);
}

/// The window is the room too: a fit column under a 300 root, holding a
/// 500 px text, is 300 wide and the text wraps.
#[test]
fn the_root_is_a_room_like_any_other() {
    let mut t = T::new(NodeSpec::column());
    let col = t.node(0, NodeSpec::column());
    let text = t.text(col, 50);
    t.run();
    assert_eq!(t.size(col).w, 300.0);
    assert_eq!(t.size(text), Size::new(300.0, 40.0));
}

/// No stretch: a fit child that fits keeps its own width, and a centred
/// one is centred in the card.
#[test]
fn a_fit_child_that_fits_keeps_its_width() {
    let mut t = T::new(NodeSpec::column());
    let card = t.node(
        0,
        NodeSpec::column().width(200.0).cross_align(Align::Center),
    );
    let row = t.node(card, NodeSpec::row());
    t.text(row, 10);
    t.run();
    assert_eq!(t.size(row).w, 100.0);
    assert_eq!(t.pos(row).x, 50.0);
}

/// A declared `minWidth` wins — a number, and `fit`, the way to ask for
/// the overflow back.
#[test]
fn a_declared_floor_wins() {
    for (min, want) in [(Min::px(300.0), 300.0), (Min::FIT, 400.0)] {
        let mut t = T::new(NodeSpec::column());
        let card = t.card(NodeSpec::column());
        let col = t.node(card, NodeSpec::column().min_width(min));
        t.text(col, 40);
        t.run();
        assert_eq!(t.size(col).w, want);
    }
}

/// Kept as they were: a column that scrolls x, whose overflow is what it
/// scrolls; a fixed child; a width a ratio derives from a fixed height.
#[test]
fn a_scroller_a_fixed_child_and_a_ratio_keep_their_width() {
    let mut t = T::new(NodeSpec::column());
    let card = t.card(NodeSpec::column().scroll_x());
    let col = t.node(card, NodeSpec::column());
    let text = t.text(col, 40);
    t.run();
    assert_eq!(t.size(col).w, 400.0);
    assert_eq!(t.size(text), Size::new(400.0, 20.0));

    let mut t = T::new(NodeSpec::column());
    let card = t.card(NodeSpec::column());
    let fixed = t.node(card, NodeSpec::column().width(300.0));
    let ratio = t.node(card, NodeSpec::column().height(100.0).aspect_ratio(4.0));
    t.run();
    assert_eq!(t.size(fixed).w, 300.0);
    assert_eq!(t.size(ratio), Size::new(400.0, 100.0));
}

/// Heights are not held: across a row 20 px tall, a fit column of three
/// lines is still 60 tall.
#[test]
fn a_height_is_not_held() {
    let mut t = T::new(NodeSpec::column());
    let row = t.node(0, NodeSpec::row().height(20.0));
    let col = t.node(row, NodeSpec::column());
    for _ in 0..3 {
        t.text(col, 5);
    }
    t.run();
    assert_eq!(t.size(col).h, 60.0);
}

/// A float's box is the room of what is in it: a viewport float capped
/// at 200 holds its fit column, and the text wraps.
#[test]
fn a_capped_float_holds_its_children() {
    let mut t = T::new(NodeSpec::column());
    let float = t.node(
        0,
        NodeSpec::column()
            .max_width(200.0)
            .float(FloatConfig::viewport()),
    );
    let col = t.node(float, NodeSpec::column());
    let text = t.text(col, 40);
    t.run();
    assert_eq!(t.size(float).w, 200.0);
    assert_eq!(t.size(col).w, 200.0);
    assert_eq!(t.size(text), Size::new(200.0, 40.0));
}

/// A fit table in the card: its fit rows are held to the table's box and
/// its fit columns compress into it, largest first, as a table narrower
/// than its columns always did.
#[test]
fn a_table_in_a_capped_card_lays_its_columns_across_it() {
    let mut t = T::new(NodeSpec::column());
    let card = t.card(NodeSpec::column());
    let table = t.node(card, NodeSpec::table());
    let mut cells = Vec::new();
    for _ in 0..2 {
        let row = t.node(table, NodeSpec::row());
        cells.push((row, t.text(row, 15), t.text(row, 15)));
    }
    t.run();
    assert_eq!(t.size(table).w, 200.0);
    for (row, a, b) in cells {
        assert_eq!(t.size(row).w, 200.0);
        assert_eq!(t.size(a).w, 100.0);
        assert_eq!(t.size(b).w, 100.0);
        assert_eq!(t.size(a).h, 40.0, "150 in 100: two lines");
    }
}

/// A terminal grid and an image are their content's size, as CSS's
/// replaced elements are: the grid's columns are the app's, which a held
/// box would not reflow, and an image under a fixed height would stretch.
/// Both keep their width in the card, and the card's other child is held.
#[test]
fn a_grid_and_an_image_keep_their_width() {
    let mut t = T::new(NodeSpec::column());
    let card = t.card(NodeSpec::column());
    let key = Key::ROOT.index(t.tree.len() as u64);
    let grid = t.tree.push(
        card,
        key,
        OriginId::HOST,
        NodeSpec::default(),
        NodeContent::Cells(CellsId(1)),
    );
    let key = Key::ROOT.index(t.tree.len() as u64);
    let image = t.tree.push(
        card,
        key,
        OriginId::HOST,
        NodeSpec::default().height(100.0),
        NodeContent::Image(ImageId::default(), ImageOpts::default()),
    );
    let col = t.node(card, NodeSpec::column());
    t.text(col, 40);
    t.run();
    assert_eq!(t.size(grid).w, 400.0);
    assert_eq!(t.size(image), Size::new(400.0, 100.0));
    assert_eq!(t.size(col).w, 200.0);
}
