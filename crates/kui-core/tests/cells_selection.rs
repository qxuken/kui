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
    core.handle_input(InputEvent::CursorMoved(from));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
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
    let copy = menu
        .items
        .iter()
        .find(|i| i.role == kui_core::MenuRole::Copy)
        .expect("a Copy row");
    assert!(copy.enabled, "cells are selected, so Copy can act");

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
