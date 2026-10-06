//! The cell grid (backlog C20): one node, a table walk to draw.

use kui_core::cells::{Cell, CellGrid, flags};
use kui_core::{CellCursor, Color, Core, NodeSpec, QuadKind, Size, TextStyle, Vec2};

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
    ui.cells(
        &CellGrid {
            rows,
            cols,
            cells,
            style: mono(),
            cursor,
            origin_line: 0,
        },
        NodeSpec::default(),
    );
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

/// The element half (backlog C20): a click or drag on the grid says which
/// cell, and the access tree reads the screen as a terminal.
#[test]
fn a_click_names_its_cell_and_the_screen_is_the_value() {
    use kui_core::{InputEvent, Key, MouseButton, Role, Value};
    let mut core = Core::new();
    let cells: Vec<Cell> = "hello world"
        .chars()
        .chain("  bye   ".chars())
        .map(|c| Cell::new(c, 0xffffffff, 0))
        .collect();
    let draw = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill().pad(10.0));
        ui.cells_keyed(
            "term",
            &CellGrid {
                rows: 2,
                cols: 11,
                cells: &cells,
                style: mono(),
                cursor: None,
                origin_line: 0,
            },
            NodeSpec::default()
                .on_click(Value::map([("kind", Value::str("hit"))]))
                .on_drag("sel"),
        );
        ui.finish();
    };
    draw(&mut core);
    let key = Key::ROOT.str("term");
    let w = core.measure_text("M", &mono(), None).width.round();
    // A click in the fourth cell of the second row.
    let events = {
        core.handle_input(InputEvent::CursorMoved(Vec2::new(
            10.0 + 3.5 * w,
            10.0 + 25.0,
        )));
        core.handle_input(InputEvent::MouseDown {
            button: MouseButton::Primary,
            clicks: 1,
        });
        core.handle_input(InputEvent::MouseUp {
            button: MouseButton::Primary,
        })
    };
    let click = events
        .iter()
        .find(|e| matches!(&e.payload, Value::Map(m) if m.iter().any(|(k, v)| k == "kind" && *v == Value::str("hit"))))
        .expect("the click arrived");
    let Value::Map(m) = &click.payload else {
        panic!()
    };
    let cell = m.iter().find(|(k, _)| k == "cell").map(|(_, v)| v.clone());
    assert_eq!(
        cell,
        Some(Value::map([("row", Value::Int(1)), ("col", Value::Int(3))])),
        "{:?}",
        click.payload
    );
    // The access tree: a terminal whose value is the screen, trailing
    // blanks trimmed, rows joined.
    let tree = core.access_tree();
    let node = tree.get(key).expect("in the tree");
    assert_eq!(node.role, Role::Terminal);
    assert_eq!(node.value.as_deref(), Some("hello world\n  bye"));
}

/// AR11: `attach_cells` had neither the pointer filter nor the no-press
/// guard its twin had, so a grid with `on_key` got `cell: {row, col}` on
/// every `key` event, and on an Enter-made click, from wherever the
/// cursor rested. Only what a press made carries a cell now.
#[test]
fn a_key_on_the_grid_and_a_click_without_a_press_name_no_cell() {
    use kui_core::{InputEvent, KeyCode, KeyMods, KeyPress, Value};
    let mut core = Core::new();
    let cells: Vec<Cell> = "ab".chars().map(|c| Cell::new(c, 0xffffffff, 0)).collect();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.child_key("term");
    ui.cells_keyed(
        "term",
        &CellGrid {
            rows: 1,
            cols: 2,
            cells: &cells,
            style: mono(),
            cursor: None,
            origin_line: 0,
        },
        NodeSpec::default()
            .key_sink()
            .on_click(Value::map([("kind", Value::str("hit"))])),
    );
    ui.take_key_focus(key);
    ui.finish();
    let has_cell = |e: &kui_core::UiEvent| e.payload.get("cell").is_some();
    // The mouse rests on the grid.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(15.0, 15.0)));
    let evs = core.handle_input(InputEvent::KeyDown(KeyPress::new(
        KeyCode::Char('j'),
        KeyMods::default(),
    )));
    let k = evs
        .iter()
        .find(|e| e.kind() == Some("key"))
        .expect("the key arrived");
    assert!(!has_cell(k), "{:?}", k.payload);
    // A click Enter made: the sink holds focus, so it is the keyboard's
    // and no click arrives; a screen reader's click does, without a cell.
    use kui_core::{AccessAction, AccessRequest};
    let evs = core.handle_input(InputEvent::Access(AccessRequest::new(
        key,
        AccessAction::Click,
    )));
    let click = evs
        .iter()
        .find(|e| e.kind() == Some("hit"))
        .expect("the click arrived");
    assert!(!has_cell(click), "{:?}", click.payload);
    // And a real press does name it.
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    let click = evs
        .iter()
        .find(|e| e.kind() == Some("hit"))
        .expect("the click arrived");
    assert!(has_cell(click), "{:?}", click.payload);
}

/// The grid's box is a box like any leaf's (AR5): its `hover_bg` lights
/// under the pointer and its `transition` tweens the bg. The cells inside
/// are the app's picture and are not what a tween reaches.
#[test]
fn the_grids_box_hovers_and_eases_like_any_leaf() {
    use kui_core::{Easing, InputEvent, Transition};
    const BASE: Color = Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };
    const HOVER: Color = Color {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };
    let mut core = Core::new();
    let cells = vec![Cell::new('a', 0xffffffff, 0); 4];
    let draw = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.cells_keyed(
            "term",
            &CellGrid {
                rows: 1,
                cols: 4,
                cells: &cells,
                style: mono(),
                cursor: None,
                origin_line: 0,
            },
            NodeSpec::default()
                .bg(BASE)
                .hover_bg(HOVER)
                .transition_with(Transition::ms(100.0).easing(Easing::Linear)),
        );
        ui.finish();
    };
    // The node's own bg is the first solid quad: it paints under the grid.
    let bg = |core: &mut Core| {
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .find(|q| q.kind == QuadKind::Solid)
            .map(|q| q.color)
            .expect("the grid's box has a bg")
    };
    core.set_time(0.0);
    draw(&mut core);
    assert_eq!(bg(&mut core), BASE);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(5.0, 5.0)));
    draw(&mut core);
    assert_eq!(bg(&mut core), BASE, "the retarget starts where it was");
    assert!(core.animating(), "a frame is owed");
    core.set_time(0.05);
    draw(&mut core);
    let mid = bg(&mut core);
    assert!(
        mid.r > 0.1 && mid.r < 0.9,
        "halfway through the tween the grid's bg is between the two: {mid:?}"
    );
    core.set_time(0.2);
    draw(&mut core);
    assert_eq!(bg(&mut core), HOVER, "a hovered grid takes its hover_bg");
}

/// F66: box drawing, block elements and the Powerline arrows are drawn
/// from the cell box, not the font — a font's `│` is its own line box
/// tall, and the cell is `line_height` tall, so through the font every
/// `│` was a dash with a gap under it. The tests read the atlas back:
/// what the quad samples is the cell's whole height.
mod boxdraw {
    use super::*;

    /// Draws `rows × cols` of `text` at `scale` and returns, per glyph
    /// quad, its rect, its slot, and the coverage down the slot's fullest
    /// column and along its fullest row.
    struct Drawn {
        quads: Vec<(f32, f32, f32, f32)>,
        /// Alpha down the slot's most-covered column, top to bottom.
        columns: Vec<Vec<u8>>,
        /// Alpha along the slot's most-covered row, left to right.
        rows: Vec<Vec<u8>>,
        uvs: Vec<[u32; 4]>,
    }

    fn draw(core: &mut Core, text: &str, rows: usize, cols: usize, scale: f32, lh: f32) -> Drawn {
        let cells: Vec<Cell> = text.chars().map(|c| Cell::new(c, 0xffffffff, 0)).collect();
        let mut ui = core.frame(Size::new(400.0, 200.0), scale);
        ui.configure_root(NodeSpec::column().fill());
        ui.cells(
            &CellGrid {
                rows,
                cols,
                cells: &cells,
                style: TextStyle::new(13.0).mono().line_height(lh),
                cursor: None,
                origin_line: 0,
            },
            NodeSpec::default(),
        );
        ui.finish();
        let (dl, atlas) = core.output();
        let glyphs: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| q.kind == QuadKind::GlyphMask)
            .collect();
        let at = |x: u32, y: u32| atlas.pixels[((y * atlas.size + x) * 4 + 3) as usize];
        // The stroke's own column and row: the ones with the most ink.
        let columns = glyphs
            .iter()
            .map(|q| {
                let [x, y, w, h] = q.uv;
                let col: Vec<Vec<u8>> = (0..w)
                    .map(|dx| (0..h).map(|dy| at(x + dx, y + dy)).collect())
                    .collect();
                col.into_iter()
                    .max_by_key(|c| c.iter().map(|&a| a as u32).sum::<u32>())
                    .unwrap()
            })
            .collect();
        let rows = glyphs
            .iter()
            .map(|q| {
                let [x, y, w, h] = q.uv;
                let row: Vec<Vec<u8>> = (0..h)
                    .map(|dy| (0..w).map(|dx| at(x + dx, y + dy)).collect())
                    .collect();
                row.into_iter()
                    .max_by_key(|r| r.iter().map(|&a| a as u32).sum::<u32>())
                    .unwrap()
            })
            .collect();
        Drawn {
            quads: glyphs
                .iter()
                .map(|q| (q.rect.x, q.rect.y, q.rect.w, q.rect.h))
                .collect(),
            columns,
            rows,
            uvs: glyphs.iter().map(|q| q.uv).collect(),
        }
    }

    #[test]
    fn a_vertical_line_covers_the_cell_top_to_bottom_across_rows() {
        let mut core = Core::new();
        for ch in ['│', '▐', '█'] {
            let d = draw(&mut core, &format!("{ch}{ch}"), 2, 1, 1.0, 20.0);
            assert_eq!(d.quads.len(), 2, "{ch}");
            // Each quad is its whole cell, and the second starts where the
            // first ends.
            let (a, b) = (d.quads[0], d.quads[1]);
            assert_eq!((a.1, a.3), (0.0, 20.0), "{ch}: {a:?}");
            assert_eq!((b.1, b.3), (20.0, 20.0), "{ch}: {b:?}");
            // And the slot's centre column is solid every scanline.
            assert_eq!(d.columns[0].len(), 20);
            assert!(
                d.columns[0].iter().all(|&a| a == 255),
                "{ch}: a zero-alpha scanline: {:?}",
                d.columns[0]
            );
            assert_eq!(d.uvs[0], d.uvs[1], "{ch}: both rows share the slot");
        }
        // The same across two columns for a horizontal line.
        let d = draw(&mut core, "──", 1, 2, 1.0, 20.0);
        let (a, b) = (d.quads[0], d.quads[1]);
        assert_eq!(a.0 + a.2, b.0, "abutting: {a:?} {b:?}");
        assert!(d.rows[0].iter().all(|&a| a == 255), "{:?}", d.rows[0]);
    }

    /// `┌─┐` / `│ │` / `└─┘` at 2×: the corner's strokes are on the same
    /// pixel columns as `│` and the same pixel rows as `─`.
    #[test]
    fn corners_meet_edges_on_the_same_pixels() {
        let mut core = Core::new();
        let d = draw(&mut core, "┌─┐│ │└─┘", 3, 3, 2.0, 20.0);
        // Eight glyphs: the space draws nothing.
        assert_eq!(d.quads.len(), 8);
        let (_, atlas) = core.output();
        let mask = |uv: [u32; 4]| -> Vec<Vec<u8>> {
            let [x, y, w, h] = uv;
            (0..h)
                .map(|dy| {
                    (0..w)
                        .map(|dx| atlas.pixels[(((y + dy) * atlas.size + x + dx) * 4 + 3) as usize])
                        .collect()
                })
                .collect()
        };
        let solid = |row: &[u8]| -> Vec<usize> {
            row.iter()
                .enumerate()
                .filter(|(_, a)| **a == 255)
                .map(|(i, _)| i)
                .collect()
        };
        let (tl, top, _tr, left, _right, bl, _bottom, _br) = (
            mask(d.uvs[0]),
            mask(d.uvs[1]),
            mask(d.uvs[2]),
            mask(d.uvs[3]),
            mask(d.uvs[4]),
            mask(d.uvs[5]),
            mask(d.uvs[6]),
            mask(d.uvs[7]),
        );
        let h = tl.len();
        // The stem of ┌ on its bottom row is the column of │ on its top row.
        assert_eq!(solid(&tl[h - 1]), solid(&left[0]), "┌ stem vs │");
        assert_eq!(solid(&bl[0]), solid(&left[h - 1]), "└ stem vs │");
        assert!(!solid(&left[0]).is_empty());
        // The arm of ┌ on its right edge is the rows ─ fills on its left.
        let col = |m: &[Vec<u8>], x: usize| -> Vec<usize> {
            m.iter()
                .enumerate()
                .filter(|(_, r)| r[x] == 255)
                .map(|(i, _)| i)
                .collect()
        };
        let w = tl[0].len();
        assert_eq!(col(&tl, w - 1), col(&top, 0), "┌ arm vs ─");
        assert!(!col(&top, 0).is_empty());
        // At 2× the light stroke is 2 px, not 1.
        assert!(solid(&left[0]).len() >= 2, "{:?}", solid(&left[0]));
    }

    /// F112: yazi rounds its hovered row with U+E0B6 before it and
    /// U+E0B4 after; both are the cell, as `│` is, not a fallback font's
    /// glyph at its own size — and so are the rest of the Powerline Extra
    /// row, the arcs and the wedges.
    #[test]
    fn rounded_caps_are_the_cell() {
        let mut core = Core::new();
        let caps: String = (0xE0B4..=0xE0BFu32)
            .map(|cp| char::from_u32(cp).unwrap())
            .collect();
        let d = draw(&mut core, &caps, 1, 12, 1.0, 20.0);
        assert_eq!(d.quads.len(), 12);
        for (i, q) in d.quads.iter().enumerate() {
            assert_eq!((q.1, q.3), (0.0, 20.0), "U+{:04X}: {q:?}", 0xE0B4 + i);
        }
        // The filled caps are solid down their flat side.
        assert!(
            d.columns[0][2..18].iter().all(|&a| a == 255),
            "E0B4's flat side: {:?}",
            d.columns[0]
        );
        assert!(
            d.columns[2][2..18].iter().all(|&a| a == 255),
            "E0B6's flat side: {:?}",
            d.columns[2]
        );
    }

    /// One cell size, one slot; another size, another.
    #[test]
    fn a_slot_per_cell_size() {
        let mut core = Core::new();
        let a = draw(&mut core, "│", 1, 1, 1.0, 16.0).uvs[0];
        let b = draw(&mut core, "│", 1, 1, 1.0, 20.0).uvs[0];
        let c = draw(&mut core, "│", 1, 1, 1.0, 20.0).uvs[0];
        assert_ne!(a, b, "16 and 20 px rows are different slots");
        assert_eq!(b, c, "the same size twice is one");
        assert_eq!(a[3], 16);
        assert_eq!(b[3], 20);
    }

    /// Characters outside the set still go through the font, so the fast
    /// path is a fast path and not a regression: their slots are not the
    /// cell's size, and a space draws nothing.
    #[test]
    fn the_rest_is_still_shaped() {
        let mut core = Core::new();
        let d = draw(&mut core, "aé│", 1, 3, 1.0, 20.0);
        assert_eq!(d.quads.len(), 3);
        assert_ne!(
            d.quads[0].3, 20.0,
            "a is a glyph, not a cell: {:?}",
            d.quads[0]
        );
        assert_ne!(d.quads[1].3, 20.0, "é likewise: {:?}", d.quads[1]);
        assert_eq!(d.quads[2].3, 20.0, "│ is the cell: {:?}", d.quads[2]);
        // The value a screen reader gets and the cell a click names are
        // unchanged: drawing only.
        let cells: Vec<Cell> = "┌─┐".chars().map(|c| Cell::new(c, 0xffffffff, 0)).collect();
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.cells_keyed(
            "t",
            &CellGrid {
                rows: 1,
                cols: 3,
                cells: &cells,
                style: mono(),
                cursor: None,
                origin_line: 0,
            },
            NodeSpec::default(),
        );
        ui.finish();
        let tree = core.access_tree();
        let node = tree.get(kui_core::Key::ROOT.str("t")).unwrap();
        assert_eq!(node.value.as_deref(), Some("┌─┐"));
    }
}

/// F120: a character the grid's family has no glyph for is a fallback
/// face's, and the platform's first is a proportional one on macOS — a
/// `Ю` half as wide again as the cell, drawn over the letter after it. A
/// cell is a cell: whatever face it comes from, the glyph is inside its
/// own, and a wide one inside its two.
#[test]
fn a_fallback_glyph_stays_in_its_cell() {
    let mut core = Core::new();
    // Fixed-pitch, printable ASCII only.
    let font = core
        .add_font_data(kui_core::testing::font_face("Kui F120", 400, false, true))
        .expect("the fixture registers");
    let style = TextStyle::new(14.0).font(font).line_height(20.0);
    let mut draw = |text: &str, wide: bool| -> Vec<(f32, f32)> {
        let cells: Vec<Cell> = text
            .chars()
            .map(|c| Cell::new(c, 0xffffffff, 0).with(if wide { flags::WIDE } else { 0 }))
            .collect();
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.cells(
            &CellGrid {
                rows: 1,
                cols: cells.len(),
                cells: &cells,
                style,
                cursor: None,
                origin_line: 0,
            },
            NodeSpec::default(),
        );
        ui.finish();
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| q.kind != QuadKind::Solid)
            .map(|q| (q.rect.x, q.rect.w))
            .collect()
    };
    let own = draw("aa", false);
    let cw = own[1].0 - own[0].0;
    for ch in ["Ю", "ж", "ш", "Щ", "Ω", "あ"] {
        for (wide, span) in [(false, cw), (true, 2.0 * cw)] {
            let g = draw(ch, wide);
            let Some(&(x, w)) = g.first() else {
                continue; // no face on this machine has it
            };
            assert!(
                x >= -1.0 && x + w <= span + 1.0,
                "{ch} (wide: {wide}) spans {x}..{} of a {span} px cell",
                x + w
            );
        }
    }
}

/// A family with no `M` draws its own glyphs as its own (backlog RG118).
/// The grid told a fallback's glyph by the face `M` shaped with, and a
/// CJK-only or symbols-only family's `M` is a fallback's: its own glyphs
/// then read as another face's, to be asked of a monospaced face first,
/// fitted and centred. Here the family's one character is 字, half an em
/// wide: drawn as its face puts it, at the cell's left edge, where it sat
/// 4 px right on a Mac, centred in a cell as wide as another face's `M`.
#[test]
fn a_family_without_an_m_draws_its_own_glyphs_as_its_own() {
    let mut core = Core::new();
    let font = core
        .add_font_data(kui_core::testing::han_only_face("Kui RG118 Han Only"))
        .expect("the fixture registers");
    let style = TextStyle::new(20.0).font(font).line_height(24.0);
    let cells = [Cell::new('字', 0xffffffff, 0)];
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.cells(
        &CellGrid {
            rows: 1,
            cols: 1,
            cells: &cells,
            style,
            cursor: None,
            origin_line: 0,
        },
        NodeSpec::default(),
    );
    ui.finish();
    let (dl, _) = core.output();
    let glyphs: Vec<(f32, f32)> = dl
        .quads
        .iter()
        .filter(|q| q.kind != QuadKind::Solid)
        .map(|q| (q.rect.x, q.rect.w))
        .collect();
    assert_eq!(glyphs, [(0.0, 8.0)], "the face's own 字, where it puts it");
}
