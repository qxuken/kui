//! Selecting inside a `cells` grid (ADR 0017, decision 4): a terminal's
//! screen selects in cells rather than in bytes, its ends are absolute
//! lines so a scroll does not move them, and a copy trims the trailing
//! blanks that make a screenful paste like a rectangle instead of text.

use kui_core::{
    Cell, CellGrid, Color, Core, InputEvent, Key, MouseButton, NodeSpec, Size, Sizing, TextStyle,
    Vec2,
};

const COLS: usize = 10;
const ROWS: usize = 3;

/// Three rows of text, padded with blanks the way a terminal pads a
/// screen: "hello", "brave world" (clipped), "bye".
fn screen() -> Vec<Cell> {
    let lines = ["hello", "brave  ", "bye"];
    let mut cells = vec![Cell::new(' ', 0xffffffff, 0); ROWS * COLS];
    for (r, line) in lines.iter().enumerate() {
        for (c, ch) in line.chars().take(COLS).enumerate() {
            cells[r * COLS + c] = Cell::new(ch, 0xffffffff, 0);
        }
    }
    cells
}

/// One grid, `selectable`, whose row 0 is absolute line `origin`.
fn frame(core: &mut Core, origin: u64) -> Key {
    let cells = screen();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let grid = CellGrid {
        rows: ROWS,
        cols: COLS,
        cells: &cells,
        style: TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        cursor: None,
        origin_line: origin,
    };
    let key = ui.child_key("term");
    ui.cells_keyed(
        "term",
        &grid,
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .selectable(),
    );
    ui.finish();
    key
}

/// The middle of cell (row, col), from the grid's own metrics.
fn cell_at(core: &mut Core, key: Key, row: usize, col: usize) -> Vec2 {
    let m = core.measure_text(
        "M",
        &TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        None,
    );
    let _ = key;
    Vec2::new(m.width * (col as f32 + 0.5), m.height * (row as f32 + 0.5))
}

fn drag(core: &mut Core, from: Vec2, to: Vec2) {
    drag_clicks(core, from, to, 1);
}

/// The same, with the click count the driver counted: 2 is a double click
/// held and dragged, 3 a triple.
fn drag_clicks(core: &mut Core, from: Vec2, to: Vec2, clicks: u8) {
    core.handle_input(InputEvent::CursorMoved(from));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks,
    });
    core.handle_input(InputEvent::CursorMoved(to));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
}

#[test]
fn a_drag_across_a_grid_selects_lines_and_copies_them_trimmed() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let from = cell_at(&mut core, key, 0, 0);
    let to = cell_at(&mut core, key, 2, 3);
    drag(&mut core, from, to);
    let sel = core.cell_selection().expect("a cell selection");
    assert_eq!(sel.node, key);
    assert_eq!(sel.anchor.line, 0);
    assert_eq!(
        core.copy_selection().as_deref(),
        Some("hello\nbrave\nbye"),
        "trailing blanks are trimmed per line, and `brave  ` loses its two"
    );
}

#[test]
fn the_ends_are_absolute_lines_so_a_scroll_does_not_move_them() {
    let mut core = Core::new();
    let key = frame(&mut core, 500);
    let from = cell_at(&mut core, key, 1, 0);
    let to = cell_at(&mut core, key, 1, 5);
    drag(&mut core, from, to);
    let sel = core.cell_selection().expect("a selection");
    assert_eq!(
        (sel.anchor.line, sel.focus.line),
        (501, 501),
        "row 1 of a screen whose row 0 is line 500"
    );
    assert_eq!(core.copy_selection().as_deref(), Some("brave"));
}

#[test]
fn a_selection_paints_over_the_cells_it_covers() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let from = cell_at(&mut core, key, 0, 0);
    let to = cell_at(&mut core, key, 0, 5);
    drag(&mut core, from, to);
    frame(&mut core, 0);
    let tint = kui_core::select::TINT;
    let tints = core
        .output()
        .0
        .quads
        .iter()
        .filter(|q| q.color == tint)
        .count();
    assert_eq!(tints, 1, "one run of selected columns on one row");
}

#[test]
fn a_grid_never_joins_the_text_scope_around_it() {
    let mut core = Core::new();
    let cells = screen();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let outer = ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| {
            ui.text("a label", TextStyle::new(14.0));
            let grid = CellGrid {
                rows: ROWS,
                cols: COLS,
                cells: &cells,
                style: TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
                cursor: None,
                origin_line: 0,
            };
            ui.cells_keyed("term", &grid, NodeSpec::column().width(Sizing::Grow(1.0)));
        },
    );
    ui.finish();
    // Selecting the card takes its text and none of the screen: the grid
    // is not part of the paragraph around it.
    assert!(core.select_all_in(outer));
    assert_eq!(core.selection_text().as_deref(), Some("a label"));
}

#[test]
fn one_selection_per_window_holds_across_the_two_kinds() {
    let mut core = Core::new();
    let cells = screen();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let card = ui.with_keyed(
        "card",
        NodeSpec::column().width(Sizing::Grow(1.0)).selectable(),
        |ui| ui.text("a label", TextStyle::new(14.0)),
    );
    let grid = CellGrid {
        rows: ROWS,
        cols: COLS,
        cells: &cells,
        style: TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        cursor: None,
        origin_line: 0,
    };
    let term = ui.child_key("term");
    ui.cells_keyed(
        "term",
        &grid,
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(60.0))
            .selectable(),
    );
    ui.finish();
    core.select_all_in(card);
    assert!(core.selection().is_some());
    core.set_cell_selection(kui_core::CellSelection::new(
        term,
        kui_core::CellEnd::new(0, 0),
        kui_core::CellEnd::new(0, 5),
    ));
    assert!(core.selection().is_none(), "the text selection went");
    assert_eq!(core.copy_selection().as_deref(), Some("hello"));
    let _ = Color::BLACK;
}

/// A right-click on a grid gets a menu that works on *cells*. Reading the
/// text selection for it left Copy dimmed over a terminal with half its
/// screen selected, and Select All did nothing at all.
#[test]
fn the_stock_menu_over_a_grid_copies_cells() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let from = cell_at(&mut core, key, 0, 0);
    let to = cell_at(&mut core, key, 0, 5);
    drag(&mut core, from, to);
    core.handle_input(InputEvent::CursorMoved(from));
    core.handle_input(InputEvent::MouseDown {
        button: kui_core::MouseButton::Secondary,
        clicks: 1,
    });
    let menu = core.menu().expect("a menu over the grid").clone();
    let at = menu
        .items
        .iter()
        .position(|i| i.role == kui_core::MenuRole::Copy)
        .expect("a Copy row");
    assert!(
        menu.items[at].enabled,
        "cells are selected, so Copy can act"
    );

    // And choosing it puts the cells on the clipboard — the row was lit
    // from the cell selection, so acting on it has to read the same one.
    core.activate_menu_item(at);
    let acts = core.take_menu_actions();
    assert!(
        matches!(
            acts.first(),
            Some(kui_core::MenuAction::SetClipboard { text, .. }) if text == "hello"
        ),
        "Copy over a grid copies the cells, not the (absent) text selection: {acts:?}"
    );

    // And Select All takes the whole screen it was given.
    core.close_menu();
    assert!(core.select_all_in(key));
    assert_eq!(core.copy_selection().as_deref(), Some("hello\nbrave\nbye"));
}

/// A selection scrolled entirely off the grid copies nothing — rather than
/// an empty string, which would let Cmd-C wipe the clipboard.
#[test]
fn a_grid_selection_scrolled_away_copies_nothing() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let from = cell_at(&mut core, key, 0, 0);
    let to = cell_at(&mut core, key, 0, 5);
    drag(&mut core, from, to);
    assert_eq!(core.copy_selection().as_deref(), Some("hello"));
    // The terminal scrolls: row 0 is now absolute line 900.
    frame(&mut core, 900);
    assert_eq!(
        core.copy_selection(),
        None,
        "nothing of the selection is on screen, so there is nothing to copy"
    );
}

/// A grid's padding moves the cells, so a hit test has to move with them:
/// layout reserves the padding and the glyphs start inside it, and reading
/// row 0 off the node's own corner picked the cell above and left of the
/// one under the pointer.
#[test]
fn a_padded_grid_counts_its_rows_from_inside_the_padding() {
    const PAD: f32 = 12.0;
    let mut core = Core::new();
    let cells = screen();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let grid = CellGrid {
        rows: ROWS,
        cols: COLS,
        cells: &cells,
        style: TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        cursor: None,
        origin_line: 0,
    };
    ui.cells_keyed(
        "term",
        &grid,
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .pad(PAD)
            .selectable(),
    );
    ui.finish();
    // The middle of row 1, column 1 — measured from the padded corner.
    let m = core.measure_text(
        "M",
        &TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        None,
    );
    let at = Vec2::new(PAD + m.width * 1.5, PAD + m.height * 1.5);
    drag(&mut core, at, at);
    let sel = core.cell_selection().expect("a selection");
    assert_eq!(
        (sel.anchor.line, sel.anchor.col),
        (1, 1),
        "the cell under the pointer, not the one the padding hides"
    );
}

/// A double click on a terminal takes the word under it, not the one cell
/// under it — the gesture every other selectable thing in the library has.
#[test]
fn a_double_click_takes_the_word_under_the_cell() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    // Row 1 is "brave  ": the click lands mid-word.
    let at = cell_at(&mut core, key, 1, 3);
    drag_clicks(&mut core, at, at, 2);
    let sel = core.cell_selection().expect("a selection");
    assert_eq!(
        (sel.anchor.col, sel.focus.col),
        (0, 5),
        "the whole word, not the cell"
    );
    assert_eq!(core.copy_selection().as_deref(), Some("brave"));
}

/// A double click on the blanks a terminal pads its lines with takes the
/// blanks — one class of cell at a time, the way a double click in text
/// takes the run of spaces it lands in.
#[test]
fn a_double_click_on_the_padding_takes_the_padding() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let at = cell_at(&mut core, key, 1, 7);
    drag_clicks(&mut core, at, at, 2);
    let sel = core.cell_selection().expect("a selection");
    assert_eq!((sel.anchor.col, sel.focus.col), (5, COLS));
}

/// A triple click takes the whole row, edge to edge; the copy is what
/// trims the blanks off it.
#[test]
fn a_triple_click_takes_the_whole_row() {
    let mut core = Core::new();
    let key = frame(&mut core, 500);
    let at = cell_at(&mut core, key, 1, 3);
    drag_clicks(&mut core, at, at, 3);
    let sel = core.cell_selection().expect("a selection");
    assert_eq!((sel.anchor.line, sel.anchor.col), (501, 0));
    assert_eq!((sel.focus.line, sel.focus.col), (501, COLS));
    assert_eq!(core.copy_selection().as_deref(), Some("brave"));
}

/// Holding the second click and dragging selects by words: both ends
/// round outwards, so the word the drag started in stays whole even when
/// the pointer turns back past it.
#[test]
fn a_held_double_click_drags_by_words() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    // Start in "brave" on row 1, drag up into "hello" on row 0.
    let from = cell_at(&mut core, key, 1, 3);
    let back = cell_at(&mut core, key, 0, 2);
    drag_clicks(&mut core, from, back, 2);
    let sel = core.cell_selection().expect("a selection");
    let (a, b) = sel.ordered();
    assert_eq!(
        ((a.line, a.col), (b.line, b.col)),
        ((0, 0), (1, 5)),
        "from the start of `hello` to the far edge of `brave`"
    );
    assert_eq!(core.copy_selection().as_deref(), Some("hello\nbrave"));
}

/// And forwards, where the anchor keeps its *near* edge and the live end
/// takes the far one.
#[test]
fn a_held_double_click_dragged_forwards_keeps_the_first_word_whole() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let from = cell_at(&mut core, key, 0, 3);
    let to = cell_at(&mut core, key, 1, 1);
    drag_clicks(&mut core, from, to, 2);
    let sel = core.cell_selection().expect("a selection");
    let (a, b) = sel.ordered();
    assert_eq!(((a.line, a.col), (b.line, b.col)), ((0, 0), (1, 5)));
}

/// A held triple click drags by whole rows.
#[test]
fn a_held_triple_click_drags_by_rows() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let from = cell_at(&mut core, key, 0, 3);
    let to = cell_at(&mut core, key, 2, 1);
    drag_clicks(&mut core, from, to, 3);
    let sel = core.cell_selection().expect("a selection");
    let (a, b) = sel.ordered();
    assert_eq!(((a.line, a.col), (b.line, b.col)), ((0, 0), (2, COLS)));
    assert_eq!(core.copy_selection().as_deref(), Some("hello\nbrave\nbye"));
}

/// A wide glyph and the blank the app leaves after it are one character,
/// not a character and a space: the `WIDE` flag is on the glyph, so the
/// spacer is the cell *after* a flagged one.
#[test]
fn a_wide_glyph_copies_as_itself_and_selects_with_its_spacer() {
    use kui_core::cells::flags;
    let mut core = Core::new();
    let mut cells = vec![Cell::new(' ', 0xffffffff, 0); ROWS * COLS];
    // "漢字x" — each wide glyph followed by the app's blank.
    let wide = |ch: char| Cell {
        ch,
        fg: 0xffffffff,
        bg: 0,
        flags: flags::WIDE,
    };
    cells[0] = wide('漢');
    cells[2] = wide('字');
    cells[4] = Cell::new('x', 0xffffffff, 0);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.child_key("term");
    ui.cells_keyed(
        "term",
        &CellGrid {
            rows: ROWS,
            cols: COLS,
            cells: &cells,
            style: TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
            cursor: None,
            origin_line: 0,
        },
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .selectable(),
    );
    ui.finish();
    assert!(core.select_all_in(key));
    assert_eq!(
        core.copy_selection()
            .as_deref()
            .map(|t| t.lines().next().unwrap().to_string()),
        Some("漢字x".to_string()),
        "the glyphs themselves, and no blank between them"
    );
    // And a double click on the second glyph takes it with its spacer.
    let at = cell_at(&mut core, key, 0, 2);
    drag_clicks(&mut core, at, at, 2);
    let sel = core.cell_selection().expect("a selection");
    assert_eq!(
        (sel.anchor.col, sel.focus.col),
        (0, 5),
        "`漢字x` is one word"
    );
}

/// A host reading the selection from inside its own view is asking while
/// the next frame's tree is half-built — the grid it means is last
/// frame's. Text has always answered there (the places swap); a grid was
/// looked up in the tree and so answered nothing, which is what a Lua or
/// Node view calling `selection_text()` saw.
#[test]
fn a_view_can_read_the_grid_selection_while_it_builds_the_next_frame() {
    let mut core = Core::new();
    let key = frame(&mut core, 0);
    let at = cell_at(&mut core, key, 1, 3);
    drag_clicks(&mut core, at, at, 2);
    assert_eq!(core.copy_selection().as_deref(), Some("brave"));

    // Mid-frame, before the grid has been declared again.
    let cells = screen();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    assert_eq!(
        ui.selection_text().as_deref(),
        Some("brave"),
        "last frame's grid answers for it"
    );
    let grid = CellGrid {
        rows: ROWS,
        cols: COLS,
        cells: &cells,
        style: TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        cursor: None,
        origin_line: 0,
    };
    ui.cells_keyed(
        "term",
        &grid,
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .selectable(),
    );
    ui.finish();
    assert_eq!(core.copy_selection().as_deref(), Some("brave"));
}
