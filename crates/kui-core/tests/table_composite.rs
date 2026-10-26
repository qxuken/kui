//! The table of cells and composites that reach past their window
//! (`docs/adr/0070-a-table-of-cells-and-composites-past-the-window.md`, backlog C89
//! and C90): a `table`'s `cell`s in `row`s are one Tab stop, the arrows
//! moving in two dimensions; and a `list` or `table` whose rows carry their
//! data `index` under a `rowCount` walks rows the frame has not built,
//! scrolling toward them and landing when the next frame builds them.

use kui_core::testing::tab;
use kui_core::{Core, EditKey, InputEvent, Key, Mods, NodeSpec, Role, Size};

const ROW_H: f32 = 20.0;

fn key(core: &mut Core, ek: EditKey, mods: Mods) {
    core.handle_input(InputEvent::Key(ek, mods));
}

fn cell_key(r: usize, c: usize) -> Key {
    Key::ROOT
        .str("grid")
        .str(&format!("r{r}"))
        .str(&format!("c{c}"))
}

/// A 3×3 table of focusable cells, 60×20 each, after a button; the cell at
/// (1, 1) disabled when `hole`.
fn cells_frame(core: &mut Core, hole: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let before = ui.leaf_keyed(
        "before",
        NodeSpec::row()
            .size(60.0, 20.0)
            .on_click("b")
            .label("Before"),
    );
    ui.with_keyed(
        "grid",
        NodeSpec::column().role(Role::Table).label("table"),
        |ui| {
            for r in 0..3 {
                ui.with_keyed(&format!("r{r}"), NodeSpec::row().role(Role::Row), |ui| {
                    for c in 0..3 {
                        let mut cell = NodeSpec::row()
                            .size(60.0, ROW_H)
                            .role(Role::Cell)
                            .focusable()
                            .label(format!("{r},{c}"));
                        if hole && (r, c) == (1, 1) {
                            cell = cell.disabled(true);
                        }
                        ui.leaf_keyed(&format!("c{c}"), cell);
                    }
                });
            }
        },
    );
    ui.finish();
    before
}

#[test]
fn a_table_is_one_tab_stop_and_its_arrows_move_in_two_dimensions() {
    let mut core = Core::new();
    let before = cells_frame(&mut core, false);
    core.set_focus(Some(before));
    tab(&mut core, false);
    assert_eq!(
        core.focus(),
        Some(cell_key(0, 0)),
        "entered on its first cell"
    );
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(before), "one stop: Tab leaves the table");
    core.set_focus(Some(cell_key(0, 0)));
    key(&mut core, EditKey::Right, Mods::NONE);
    assert_eq!(core.focus(), Some(cell_key(0, 1)));
    key(&mut core, EditKey::Down, Mods::NONE);
    assert_eq!(core.focus(), Some(cell_key(1, 1)), "down keeps the column");
    key(&mut core, EditKey::Down, Mods::NONE);
    key(&mut core, EditKey::Down, Mods::NONE);
    assert_eq!(
        core.focus(),
        Some(cell_key(2, 1)),
        "and clamps at the last row"
    );
    key(&mut core, EditKey::End, Mods::NONE);
    assert_eq!(core.focus(), Some(cell_key(2, 2)), "End: the row's last");
    key(&mut core, EditKey::Right, Mods::NONE);
    assert_eq!(
        core.focus(),
        Some(cell_key(2, 2)),
        "clamps at the row's end"
    );
    key(&mut core, EditKey::Home, Mods::NONE.with_ctrl());
    assert_eq!(
        core.focus(),
        Some(cell_key(0, 0)),
        "Ctrl+Home: the table's first"
    );
    key(&mut core, EditKey::End, Mods::NONE.with_doc());
    assert_eq!(
        core.focus(),
        Some(cell_key(2, 2)),
        "the primary does it too"
    );
    assert!(core.focus_visible());
}

#[test]
fn a_disabled_cell_is_stepped_past_and_keeps_its_column() {
    let mut core = Core::new();
    cells_frame(&mut core, true);
    core.set_focus(Some(cell_key(1, 0)));
    key(&mut core, EditKey::Right, Mods::NONE);
    assert_eq!(core.focus(), Some(cell_key(1, 2)), "past the disabled cell");
    core.set_focus(Some(cell_key(0, 1)));
    key(&mut core, EditKey::Down, Mods::NONE);
    assert_eq!(
        core.focus(),
        Some(cell_key(1, 0)),
        "onto the live cell nearest the column, the earlier on a tie"
    );
}

#[test]
fn a_table_and_its_rows_and_cells_reach_assistive_technology() {
    let mut core = Core::new();
    cells_frame(&mut core, false);
    let tree = core.access_tree().clone();
    let role = |k: Key| tree.nodes.iter().find(|n| n.key == k).map(|n| n.role);
    assert_eq!(role(Key::ROOT.str("grid")), Some(Role::Table));
    assert_eq!(role(Key::ROOT.str("grid").str("r0")), Some(Role::Row));
    assert_eq!(role(cell_key(0, 0)), Some(Role::Cell));
    let cell = tree.nodes.iter().find(|n| n.key == cell_key(1, 2)).unwrap();
    assert_eq!(
        cell.pos_in_set, None,
        "a cell is placed by row and column, not in one set"
    );
}

// -- Windowed (C89) ---------------------------------------------------------

const N: u64 = 1000;
const VIEW_ROWS: u64 = 10;

/// A virtual list of `N` rows, 20 px each, in a 200 px scroller: the rows
/// the scroll offset shows are built, opened by their data index, and the
/// spacers keep the height of the rest — the shape Kit's list has.
/// `table` makes it a table of rows of three cells instead.
fn virtual_frame(core: &mut Core, table: bool) {
    let list = Key::ROOT.str("list");
    let off = core.scroll_offset(list).y.max(0.0);
    let first = ((off / ROW_H).floor() as u64).min(N);
    let last = (first + VIEW_ROWS + 1).min(N);
    let mut ui = core.frame(Size::new(300.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "before",
        NodeSpec::row()
            .size(60.0, 20.0)
            .on_click("b")
            .label("Before"),
    );
    let role = if table { Role::Table } else { Role::List };
    ui.with_keyed(
        "list",
        NodeSpec::column()
            .size(200.0, VIEW_ROWS as f32 * ROW_H)
            .scroll_y()
            .role(role)
            .label("rows")
            .on_typeahead("ta"),
        |ui| {
            ui.row_count(N);
            if first > 0 {
                ui.leaf_keyed(
                    "lead",
                    NodeSpec::row().width(200.0).height(first as f32 * ROW_H),
                );
            }
            for i in first..last {
                if table {
                    ui.with_indexed(i, NodeSpec::row().height(ROW_H).role(Role::Row), |ui| {
                        for c in 0..3 {
                            ui.leaf_keyed(
                                &format!("c{c}"),
                                NodeSpec::row()
                                    .size(60.0, ROW_H)
                                    .role(Role::Cell)
                                    .focusable()
                                    .label(format!("{i},{c}")),
                            );
                        }
                    });
                } else {
                    ui.leaf_indexed(
                        i,
                        NodeSpec::row()
                            .size(200.0, ROW_H)
                            .role(Role::ListItem)
                            .focusable()
                            .label(format!("row {i}")),
                    );
                }
            }
            if last < N {
                ui.leaf_keyed(
                    "tail",
                    NodeSpec::row()
                        .width(200.0)
                        .height((N - last) as f32 * ROW_H),
                );
            }
        },
    );
    ui.finish();
}

fn row_key(i: u64) -> Key {
    Key::ROOT.str("list").index(i)
}

fn table_cell(i: u64, c: usize) -> Key {
    row_key(i).str(&format!("c{c}"))
}

/// Draws frames until nothing is owed (a few at most).
fn settle(core: &mut Core, table: bool) {
    for _ in 0..5 {
        virtual_frame(core, table);
        if !core.animating() {
            break;
        }
    }
}

#[test]
fn a_windowed_list_walks_past_its_window_and_lands_on_the_row_it_builds() {
    let mut core = Core::new();
    virtual_frame(&mut core, false);
    virtual_frame(&mut core, false);
    core.set_focus(Some(row_key(0)));
    // Down to the last row built, then once more: a row not built yet.
    for _ in 0..VIEW_ROWS {
        key(&mut core, EditKey::Down, Mods::NONE);
        settle(&mut core, false);
    }
    assert_eq!(
        core.focus(),
        Some(row_key(VIEW_ROWS)),
        "reached the row past the window"
    );
    assert!(
        core.scroll_offset(Key::ROOT.str("list")).y > 0.0,
        "the list scrolled to it"
    );
    // End: the last row of the data, a thousand rows down.
    key(&mut core, EditKey::End, Mods::NONE);
    settle(&mut core, false);
    assert_eq!(
        core.focus(),
        Some(row_key(N - 1)),
        "End is the data's last row"
    );
    assert!(core.focus_visible());
    // Home: back to the first.
    key(&mut core, EditKey::Home, Mods::NONE);
    settle(&mut core, false);
    assert_eq!(core.focus(), Some(row_key(0)));
    // Page Down: a page of the rows the scroller shows.
    key(&mut core, EditKey::PageDown, Mods::NONE);
    settle(&mut core, false);
    assert_eq!(core.focus(), Some(row_key(VIEW_ROWS)));
    // Down from the last row clamps.
    key(&mut core, EditKey::End, Mods::NONE);
    settle(&mut core, false);
    key(&mut core, EditKey::Down, Mods::NONE);
    settle(&mut core, false);
    assert_eq!(core.focus(), Some(row_key(N - 1)));
}

#[test]
fn keys_faster_than_frames_keep_going_from_the_row_reached_for() {
    let mut core = Core::new();
    virtual_frame(&mut core, false);
    core.set_focus(Some(row_key(VIEW_ROWS)));
    virtual_frame(&mut core, false);
    // Three Downs before any frame: the reach goes on from its own index.
    for _ in 0..3 {
        key(&mut core, EditKey::Down, Mods::NONE);
    }
    settle(&mut core, false);
    assert_eq!(core.focus(), Some(row_key(VIEW_ROWS + 3)));
}

#[test]
fn a_reach_is_let_go_when_focus_moves_another_way() {
    let mut core = Core::new();
    virtual_frame(&mut core, false);
    core.set_focus(Some(row_key(0)));
    key(&mut core, EditKey::End, Mods::NONE);
    let before = Key::ROOT.str("before");
    core.set_focus(Some(before));
    settle(&mut core, false);
    assert_eq!(
        core.focus(),
        Some(before),
        "the reach did not pull focus back"
    );
}

#[test]
fn a_windowed_table_reaches_an_unbuilt_row_and_keeps_the_column() {
    let mut core = Core::new();
    virtual_frame(&mut core, true);
    virtual_frame(&mut core, true);
    core.set_focus(Some(table_cell(0, 2)));
    key(&mut core, EditKey::End, Mods::NONE.with_ctrl());
    settle(&mut core, true);
    assert_eq!(
        core.focus(),
        Some(table_cell(N - 1, 2)),
        "the table's last cell"
    );
    key(&mut core, EditKey::Left, Mods::NONE);
    key(&mut core, EditKey::PageUp, Mods::NONE);
    settle(&mut core, true);
    assert_eq!(
        core.focus(),
        Some(table_cell(N - 1 - VIEW_ROWS, 1)),
        "a page up, same column"
    );
    key(&mut core, EditKey::Home, Mods::NONE.with_ctrl());
    settle(&mut core, true);
    assert_eq!(core.focus(), Some(table_cell(0, 0)));
}

/// A layout table that declares no role reads as a table (ADR 0070): the
/// `table()` is a `table`, its in-flow rows `row`s and their children
/// `cell`s — a text among them named by its text — and with focusable
/// cells it is the composite, nothing spelled.
#[test]
fn a_layout_table_implies_the_roles_and_its_focusable_cells_are_a_composite() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("t", NodeSpec::table(), |ui| {
        for r in 0..2 {
            ui.with_keyed(&format!("r{r}"), NodeSpec::row(), |ui| {
                ui.text(&format!("row {r}"), kui_core::TextStyle::new(12.0));
                ui.leaf_keyed(
                    "go",
                    NodeSpec::row().size(40.0, 20.0).focusable().label("go"),
                );
            });
        }
        // A clickable row is still a button, and holds no cells.
        ui.with_keyed("x", NodeSpec::row().on_click("x").label("extra"), |ui| {
            ui.leaf_keyed("in", NodeSpec::row().size(40.0, 20.0));
        });
    });
    ui.finish();
    let t = Key::ROOT.str("t");
    let tree = core.access_tree().clone();
    let role = |k: Key| tree.nodes.iter().find(|n| n.key == k).map(|n| n.role);
    assert_eq!(role(t), Some(Role::Table));
    assert_eq!(role(t.str("r0")), Some(Role::Row));
    assert_eq!(role(t.str("r0").str("go")), Some(Role::Cell));
    let text = tree
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("row 1"))
        .expect("the text cell");
    assert_eq!(text.role, Role::Cell);
    assert_eq!(role(t.str("x")), Some(Role::Button));
    assert_eq!(
        role(t.str("x").str("in")),
        None,
        "a button's box holds no cell"
    );
    core.set_focus(Some(t.str("r0").str("go")));
    key(&mut core, EditKey::Down, Mods::NONE);
    assert_eq!(
        core.focus(),
        Some(t.str("r1").str("go")),
        "the arrows walk it"
    );
}

/// Rows and columns reach a reader (ADR 0070): a cell's row and column, a
/// row's index, the table's counts; a header row of `columnHeader`s is
/// row 0 and the column index links a cell to its header; a table with no
/// focusable cell is a static table, one with them a grid.
#[test]
fn a_table_reports_its_rows_columns_and_headers() {
    let build = |core: &mut Core, focus: bool| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "t",
            NodeSpec::column().role(Role::Table).label("people"),
            |ui| {
                ui.with_keyed("head", NodeSpec::row().role(Role::Row), |ui| {
                    for c in 0..2 {
                        ui.leaf_keyed(
                            &format!("h{c}"),
                            NodeSpec::row()
                                .size(60.0, 20.0)
                                .role(Role::ColumnHeader)
                                .label(format!("H{c}")),
                        );
                    }
                });
                for r in 0..2 {
                    ui.with_keyed(&format!("r{r}"), NodeSpec::row().role(Role::Row), |ui| {
                        ui.leaf_keyed(
                            "name",
                            NodeSpec::row()
                                .size(60.0, 20.0)
                                .role(Role::RowHeader)
                                .label("n"),
                        );
                        let mut cell = NodeSpec::row().size(60.0, 20.0).role(Role::Cell).label("v");
                        if focus {
                            cell = cell.focusable();
                        }
                        ui.leaf_keyed("v", cell);
                    });
                }
            },
        );
        ui.finish();
    };
    let mut core = Core::new();
    build(&mut core, false);
    let t = Key::ROOT.str("t");
    let tree = core.access_tree().clone();
    let node = |k: Key| tree.nodes.iter().find(|n| n.key == k).cloned().unwrap();
    let table = node(t);
    assert_eq!((table.row_count, table.column_count), (Some(3), Some(2)));
    assert!(!table.cells_focus, "no focusable cell: a static table");
    let h1 = node(t.str("head").str("h1"));
    assert_eq!(
        (h1.role, h1.row_index, h1.column_index),
        (Role::ColumnHeader, Some(0), Some(1))
    );
    let v = node(t.str("r1").str("v"));
    assert_eq!(
        (v.row_index, v.column_index),
        (Some(2), Some(1)),
        "under H1 by its column"
    );
    assert_eq!(node(t.str("r1")).row_index, Some(2));
    assert_eq!(node(t.str("r1").str("name")).role, Role::RowHeader);
    build(&mut core, true);
    let tree = core.access_tree().clone();
    assert!(
        tree.nodes.iter().find(|n| n.key == t).unwrap().cells_focus,
        "a grid now"
    );
    // Navigation treats a header as the cell it is: Up from the first
    // body row's value lands on the column header above when it takes
    // focus — here it does not, so Up stays.
    core.set_focus(Some(t.str("r0").str("v")));
    key(&mut core, EditKey::Down, Mods::NONE);
    assert_eq!(core.focus(), Some(t.str("r1").str("v")));
}

/// A windowed list's rows are numbered by their data index and counted
/// by its `rowCount`: "row 120 of 4,000" with no `label`.
#[test]
fn a_windowed_list_numbers_its_rows_by_the_data() {
    let mut core = Core::new();
    let list = Key::ROOT.str("list");
    core.set_scroll(list, kui_core::Vec2::new(0.0, 120.0 * ROW_H));
    virtual_frame(&mut core, false);
    virtual_frame(&mut core, false);
    let tree = core.access_tree().clone();
    let row = tree
        .nodes
        .iter()
        .find(|n| n.key == row_key(120))
        .expect("row 120 built");
    assert_eq!((row.row_index, row.pos_in_set), (Some(120), Some(120)));
    let l = tree.nodes.iter().find(|n| n.key == list).unwrap();
    assert_eq!(
        (l.row_count, l.set_size),
        (Some(N as usize), Some(N as usize))
    );
}

/// Type-ahead in a windowed list (ADR 0070): the built rows are searched
/// first; a search none of them answers is asked of the app as the list's
/// `onTypeahead` message, and its answer — a row's data index — is
/// reached for as a step is. An answer is taken once, and not after focus
/// has left the list.
#[test]
fn an_unanswered_type_ahead_is_asked_and_the_answer_reached_for() {
    let mut core = Core::new();
    virtual_frame(&mut core, false);
    core.set_focus(Some(row_key(0)));
    virtual_frame(&mut core, false);
    let list = Key::ROOT.str("list");
    // "row 3" is built: found there, nothing asked.
    let out = core.handle_input(InputEvent::Text("row 3".into()));
    assert_eq!(core.focus(), Some(row_key(3)));
    assert!(out.iter().all(|e| e.event.name() != "typeahead"), "{out:?}");
    // Nothing built answers "z": the list is asked.
    let out = core.handle_input(InputEvent::Text("z".into()));
    let asked = out
        .iter()
        .find(|e| e.event.name() == "typeahead")
        .expect("asked");
    assert_eq!(asked.key, list);
    assert_eq!(asked.data().and_then(|d| d.get_str("text")), Some("z"));
    assert!(core.answer_typeahead(list, 500), "answered");
    assert!(!core.answer_typeahead(list, 600), "once");
    settle(&mut core, false);
    assert_eq!(core.focus(), Some(row_key(500)), "reached for and landed");
    // A stale answer: focus left the list before it came.
    core.handle_input(InputEvent::Text("q".into()));
    core.set_focus(Some(Key::ROOT.str("before")));
    assert!(!core.answer_typeahead(list, 7));
}
