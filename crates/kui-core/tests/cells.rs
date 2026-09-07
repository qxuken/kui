//! The cell grid (backlog C20): one node, a table walk to draw.

use kui_core::cells::{Cell, CellGrid, flags};
use kui_core::{CellCursor, Color, Core, NodeSpec, QuadKind, Size, TextStyle};

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(20.0)
}

fn grid(
    core: &mut Core,
    rows: usize,
    cols: usize,
    cells: &[Cell],
    cursor: Option<(usize, usize, CellCursor, Color)>,
) -> Vec<(QuadKind, f32, f32, f32, f32)> {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.cells(&CellGrid {
        rows,
        cols,
        cells,
        style: mono(),
        cursor,
    });
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .map(|q| (q.kind, q.rect.x, q.rect.y, q.rect.w, q.rect.h))
        .collect()
}

#[test]
fn glyphs_sit_on_the_grid_and_backgrounds_coalesce() {
    let mut core = Core::new();
    // The grid snaps its cell width to whole pixels.
    let w = core.measure_text("M", &mono(), None).width.round();
    // Two rows: "ab c" over a blue run, "  x " plain.
    let mut cells = vec![Cell::default(); 8];
    cells[0] = Cell::new('a', 0xffffffff, 0x0000ffff);
    cells[1] = Cell::new('b', 0xffffffff, 0x0000ffff);
    cells[2] = Cell::new(' ', 0xffffffff, 0x0000ffff);
    cells[3] = Cell::new('c', 0xffffffff, 0);
    cells[6] = Cell::new('x', 0xff0000ff, 0);
    let quads = grid(&mut core, 2, 4, &cells, None);
    let solids: Vec<_> = quads.iter().filter(|q| q.0 == QuadKind::Solid).collect();
    let glyphs: Vec<_> = quads
        .iter()
        .filter(|q| q.0 == QuadKind::GlyphMask)
        .collect();
    assert_eq!(
        solids.len(),
        1,
        "three blue cells are one background: {solids:?}"
    );
    assert!((solids[0].3 - 3.0 * w).abs() < 1.0, "{}", solids[0].3);
    assert_eq!(solids[0].4, 20.0);
    assert_eq!(glyphs.len(), 4, "a, b, c, x: {glyphs:?}");
    // `c` sits in the fourth column, `x` in the third of the second row.
    let c = glyphs[2];
    assert!(c.1 >= 3.0 * w - 1.0 && c.1 < 4.0 * w, "{}", c.1);
    let x = glyphs[3];
    assert!(
        x.1 >= 2.0 * w - 1.0 && x.1 < 3.0 * w && x.2 >= 20.0,
        "{:?}",
        x
    );
    // The node is the grid's size.
    let (dl, _) = core.output();
    assert!(dl.quads.len() >= 5);
}

#[test]
fn the_cursor_and_the_lines_are_quads_too() {
    let mut core = Core::new();
    // The grid snaps its cell width to whole pixels.
    let w = core.measure_text("M", &mono(), None).width.round();
    let cells = vec![
        Cell::new('u', 0xffffffff, 0).with(flags::UNDERLINE),
        Cell::new('u', 0xffffffff, 0).with(flags::UNDERLINE),
        Cell::new('s', 0xffffffff, 0).with(flags::STRIKETHROUGH),
    ];
    let quads = grid(
        &mut core,
        1,
        3,
        &cells,
        Some((0, 1, CellCursor::Block, Color::rgb8(0, 255, 0))),
    );
    let solids: Vec<_> = quads.iter().filter(|q| q.0 == QuadKind::Solid).collect();
    // The cursor block, one underline over two cells, one strikethrough.
    assert_eq!(solids.len(), 3, "{solids:?}");
    let cursor = solids[0];
    assert!(
        (cursor.1 - w).abs() < 1.0 && cursor.3 == w.round() && cursor.4 == 20.0,
        "{cursor:?}"
    );
    let under = solids[1];
    assert!(
        (under.3 - 2.0 * w).abs() < 1.0 && under.4 == 1.0,
        "{under:?}"
    );
    // The cursor is painted before the glyph on it.
    let first_glyph = quads
        .iter()
        .position(|q| q.0 == QuadKind::GlyphMask)
        .unwrap();
    let cursor_at = quads.iter().position(|q| *q == *cursor).unwrap();
    assert!(cursor_at < first_glyph);
}

#[test]
fn a_new_screen_shapes_nothing_new() {
    let mut core = Core::new();
    let a: Vec<Cell> = "hello world"
        .chars()
        .map(|c| Cell::new(c, 0xffffffff, 0))
        .collect();
    let b: Vec<Cell> = "world hello"
        .chars()
        .map(|c| Cell::new(c, 0xffffffff, 0))
        .collect();
    grid(&mut core, 1, 11, &a, None);
    let entries = core.text_cache_len();
    grid(&mut core, 1, 11, &b, None);
    assert_eq!(
        core.text_cache_len(),
        entries,
        "the grid's glyphs live in their own table"
    );
    let quads = grid(&mut core, 1, 11, &b, None);
    assert_eq!(
        quads.iter().filter(|q| q.0 == QuadKind::GlyphMask).count(),
        10
    );
}
