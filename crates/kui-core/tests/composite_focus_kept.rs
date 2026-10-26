//! Removing a composite's focused item keeps focus in the composite
//! (backlog C125): the frame without the item lands focus on the item now
//! at its place, else the one before it, else the first — in a list keyed
//! by id, in one keyed by index (a checklist), in a table and in a windowed
//! list — and `focus_item` names an item by its index.

use kui_core::testing::{click_at, key_down, key_up, tab};
use kui_core::{
    Core, EditKey, InputEvent, Key, KeyCode, KeyMods, Mods, NodeSpec, Role, Size, Vec2,
};

const ROW_H: f32 = 20.0;

fn before() -> Key {
    Key::ROOT.str("before")
}

fn list() -> Key {
    Key::ROOT.str("list")
}

/// A button, then a list of focusable rows named by `names`: keyed by the
/// name, or opened by their data index when `indexed` (a checklist's rows,
/// keyed by place). The row at `start` declares `initial_focus`, so Tab
/// enters there. Returns the rows' keys.
fn frame(core: &mut Core, names: &[&str], indexed: bool, start: Option<usize>) -> Vec<Key> {
    let mut rows = Vec::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "before",
        NodeSpec::row()
            .size(60.0, 20.0)
            .on_click("b")
            .label("Before"),
    );
    ui.with_keyed(
        "list",
        NodeSpec::column()
            .role(Role::List)
            .label("Checklist")
            .on_item_key("item-key"),
        |ui| {
            for (i, name) in names.iter().enumerate() {
                let mut row = NodeSpec::row()
                    .size(200.0, ROW_H)
                    .role(Role::ListItem)
                    .focusable()
                    .label(name.to_string());
                if start == Some(i) {
                    row = row.initial_focus();
                }
                rows.push(if indexed {
                    ui.leaf_indexed(i as u64, row)
                } else {
                    ui.leaf_keyed(name, row)
                });
            }
        },
    );
    ui.finish();
    rows
}

/// Delete as a keyboard sends it.
fn delete(core: &mut Core) {
    key_down(core, KeyCode::Delete, KeyMods::NONE);
    core.handle_input(InputEvent::Key(EditKey::Delete, Mods::NONE));
    key_up(core, KeyCode::Delete, KeyMods::NONE);
}

#[test]
fn removing_the_focused_middle_item_focuses_the_one_now_at_its_place() {
    let mut core = Core::new();
    let rows = frame(&mut core, &["a", "b", "c", "d"], false, None);
    core.set_focus(Some(rows[1]));
    core.handle_input(InputEvent::Key(EditKey::Down, Mods::NONE));
    assert_eq!(core.focus(), Some(rows[2]), "on c");
    delete(&mut core);
    let now = frame(&mut core, &["a", "b", "d"], false, None);
    assert_eq!(core.focus(), Some(now[2]), "d took c's place, and focus");
    assert!(
        core.focus_visible(),
        "the arrows showed it, and it still shows"
    );
}

#[test]
fn removing_the_focused_last_item_focuses_the_one_before() {
    let mut core = Core::new();
    let rows = frame(&mut core, &["a", "b", "c"], false, None);
    core.set_focus(Some(rows[2]));
    frame(&mut core, &["a", "b", "c"], false, None);
    let now = frame(&mut core, &["a", "b"], false, None);
    assert_eq!(core.focus(), Some(now[1]), "b, the one before");
    let now = frame(&mut core, &["a"], false, None);
    assert_eq!(core.focus(), Some(now[0]), "and again");
    frame(&mut core, &[], false, None);
    assert_eq!(
        core.focus(),
        Some(now[0]),
        "an empty composite leaves focus where it was, as before"
    );
}

#[test]
fn tab_straight_onto_the_last_row_then_delete_lands_on_the_row_before() {
    // A checklist's rows are keyed by place: the last row's key is the one
    // that goes. Tab lands on it between frames, and Delete follows before
    // any frame has seen the focus there.
    let mut core = Core::new();
    let names = ["a", "b", "c"];
    frame(&mut core, &names, true, Some(2));
    core.set_focus(Some(before()));
    frame(&mut core, &names, true, Some(2));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(list().index(2)), "Tab entered on c");
    delete(&mut core);
    frame(&mut core, &["a", "b"], true, None);
    assert_eq!(core.focus(), Some(list().index(1)), "on b");
    assert!(core.focus_visible());
    // And the middle of a checklist: its key stays with the row after.
    core.handle_input(InputEvent::Key(EditKey::Home, Mods::NONE));
    frame(&mut core, &["a", "b"], true, None);
    let now = frame(&mut core, &["b"], true, None);
    assert_eq!(core.focus(), Some(now[0]));
}

#[test]
fn a_click_focus_stays_unshown_when_its_item_goes() {
    let mut core = Core::new();
    let rows = frame(&mut core, &["a", "b", "c"], false, None);
    // The third row, 20 px under the button and two rows down.
    click_at(&mut core, 10.0, 20.0 + 2.5 * ROW_H);
    assert_eq!(core.focus(), Some(rows[2]), "the click focused c");
    assert!(!core.focus_visible());
    let now = frame(&mut core, &["a", "b"], false, None);
    assert_eq!(core.focus(), Some(now[1]));
    assert!(!core.focus_visible(), "a removal is no new input");
}

#[test]
fn a_composite_that_goes_with_the_item_leaves_focus_as_before() {
    let mut core = Core::new();
    let rows = frame(&mut core, &["a", "b"], false, None);
    core.set_focus(Some(rows[1]));
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed("before", NodeSpec::row().size(60.0, 20.0).on_click("b"));
    ui.finish();
    assert_eq!(core.focus(), Some(rows[1]), "today's fallback");
}

#[test]
fn focus_moved_elsewhere_is_not_pulled_back() {
    let mut core = Core::new();
    let rows = frame(&mut core, &["a", "b", "c"], false, None);
    core.set_focus(Some(rows[2]));
    frame(&mut core, &["a", "b", "c"], false, None);
    core.set_focus(Some(before()));
    frame(&mut core, &["a", "b"], false, None);
    assert_eq!(core.focus(), Some(before()));
}

fn cell(row: &str, c: usize) -> Key {
    Key::ROOT.str("grid").str(row).str(&format!("c{c}"))
}

/// A table of rows keyed by `rows`, three focusable cells each.
fn table_frame(core: &mut Core, rows: &[&str]) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "grid",
        NodeSpec::column().role(Role::Table).label("table"),
        |ui| {
            for r in rows {
                ui.with_keyed(r, NodeSpec::row().role(Role::Row), |ui| {
                    for c in 0..3 {
                        ui.leaf_keyed(
                            &format!("c{c}"),
                            NodeSpec::row()
                                .size(60.0, ROW_H)
                                .role(Role::Cell)
                                .focusable()
                                .label(format!("{r},{c}")),
                        );
                    }
                });
            }
        },
    );
    ui.finish();
}

#[test]
fn a_table_row_that_goes_hands_focus_to_the_row_at_its_place_in_the_column() {
    let mut core = Core::new();
    table_frame(&mut core, &["x", "y", "z"]);
    core.set_focus(Some(cell("y", 2)));
    table_frame(&mut core, &["x", "y", "z"]);
    table_frame(&mut core, &["x", "z"]);
    assert_eq!(core.focus(), Some(cell("z", 2)), "z's cell in y's column");
    table_frame(&mut core, &["x"]);
    assert_eq!(
        core.focus(),
        Some(cell("x", 2)),
        "the last row's: the row before"
    );
}

// -- A windowed list -------------------------------------------------------

const VIEW_ROWS: u64 = 10;

/// A virtual list of `n` rows in a 200 px scroller, the rows the scroll
/// offset shows built by their data index, spacers keeping the rest.
fn virtual_frame(core: &mut Core, n: u64) {
    let off = core.scroll_offset(list()).y.max(0.0);
    let first = ((off / ROW_H).floor() as u64).min(n);
    let last = (first + VIEW_ROWS + 1).min(n);
    let mut ui = core.frame(Size::new(300.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "before",
        NodeSpec::row()
            .size(60.0, 20.0)
            .on_click("b")
            .label("Before"),
    );
    ui.with_keyed(
        "list",
        NodeSpec::column()
            .size(200.0, VIEW_ROWS as f32 * ROW_H)
            .scroll_y()
            .role(Role::List)
            .label("rows"),
        |ui| {
            ui.row_count(n);
            if first > 0 {
                ui.leaf_keyed(
                    "lead",
                    NodeSpec::row().width(200.0).height(first as f32 * ROW_H),
                );
            }
            for i in first..last {
                ui.leaf_indexed(
                    i,
                    NodeSpec::row()
                        .size(200.0, ROW_H)
                        .role(Role::ListItem)
                        .focusable()
                        .label(format!("row {i}")),
                );
            }
            if last < n {
                ui.leaf_keyed(
                    "tail",
                    NodeSpec::row()
                        .width(200.0)
                        .height((n - last) as f32 * ROW_H),
                );
            }
        },
    );
    ui.finish();
}

fn settle(core: &mut Core, n: u64) {
    for _ in 0..5 {
        virtual_frame(core, n);
        if !core.animating() {
            break;
        }
    }
}

#[test]
fn a_windowed_lists_last_row_going_lands_on_the_new_last() {
    let mut core = Core::new();
    let n = 1000;
    virtual_frame(&mut core, n);
    core.set_focus(Some(list().index(0)));
    core.handle_input(InputEvent::Key(EditKey::End, Mods::NONE));
    settle(&mut core, n);
    assert_eq!(core.focus(), Some(list().index(n - 1)));
    settle(&mut core, n - 1);
    assert_eq!(core.focus(), Some(list().index(n - 2)), "the new last row");
    // A middle row's removal leaves its key to the row after: focus stays.
    core.handle_input(InputEvent::Key(EditKey::Up, Mods::NONE));
    settle(&mut core, n - 1);
    settle(&mut core, n - 2);
    assert_eq!(core.focus(), Some(list().index(n - 3)));
}

#[test]
fn a_windowed_row_scrolled_out_keeps_focus_and_the_scroll() {
    let mut core = Core::new();
    let n = 1000;
    virtual_frame(&mut core, n);
    core.set_focus(Some(list().index(0)));
    virtual_frame(&mut core, n);
    core.set_scroll(list(), Vec2::new(0.0, 400.0));
    settle(&mut core, n);
    assert_eq!(
        core.focus(),
        Some(list().index(0)),
        "still on row 0, unbuilt"
    );
    assert_eq!(
        core.scroll_offset(list()).y,
        400.0,
        "and the list was not pulled back to it"
    );
}

// -- The door --------------------------------------------------------------

#[test]
fn focus_item_lands_on_an_item_by_its_place_or_data_index() {
    let mut core = Core::new();
    let rows = frame(&mut core, &["a", "b", "c"], false, None);
    core.focus_item(list(), 2);
    assert!(core.animating(), "a frame is owed");
    frame(&mut core, &["a", "b", "c"], false, None);
    assert_eq!(
        core.focus(),
        Some(rows[2]),
        "by place, rows carrying no index"
    );
    // From a view, naming the list it is about to declare.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.focus_item(list(), 0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", NodeSpec::column().role(Role::List), |ui| {
        for n in ["a", "b", "c"] {
            ui.leaf_keyed(
                n,
                NodeSpec::row()
                    .size(200.0, ROW_H)
                    .role(Role::ListItem)
                    .focusable(),
            );
        }
    });
    ui.finish();
    assert_eq!(core.focus(), Some(rows[0]), "landed when the view finished");
    // Past the items is nothing.
    core.focus_item(list(), 3);
    frame(&mut core, &["a", "b", "c"], false, None);
    assert_eq!(core.focus(), Some(rows[0]));
    // Nothing for a key that names no composite.
    core.focus_item(Key::ROOT.str("nothing"), 0);
    frame(&mut core, &["a", "b", "c"], false, None);
    assert_eq!(core.focus(), Some(rows[0]));
    core.focus_item(before(), 0);
    frame(&mut core, &["a", "b", "c"], false, None);
    assert_eq!(core.focus(), Some(rows[0]), "a button is no composite");
}

#[test]
fn focus_item_names_a_checklists_row_by_its_index() {
    let mut core = Core::new();
    frame(&mut core, &["a", "b", "c"], true, None);
    core.focus_item(list(), 1);
    frame(&mut core, &["a", "b", "c"], true, None);
    assert_eq!(core.focus(), Some(list().index(1)));
    assert!(
        !core.focus_visible(),
        "programmatic focus keeps the last modality"
    );
}

#[test]
fn focus_item_reaches_an_unbuilt_row_of_a_windowed_list() {
    let mut core = Core::new();
    let n = 1000;
    virtual_frame(&mut core, n);
    core.focus_item(list(), 500);
    settle(&mut core, n);
    assert_eq!(core.focus(), Some(list().index(500)), "reached and landed");
    assert!(core.scroll_offset(list()).y > 0.0);
    core.focus_item(list(), 5000);
    settle(&mut core, n);
    assert_eq!(
        core.focus(),
        Some(list().index(n - 1)),
        "past the count is the last"
    );
}

#[test]
fn focus_item_stands_against_a_closing_modals_restore() {
    let mut core = Core::new();
    let names = ["a", "b", "c"];
    let rows = frame(&mut core, &names, false, None);
    core.set_focus(Some(before()));
    // A modal opens over the list, and closes with a focus_item.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed("before", NodeSpec::row().size(60.0, 20.0).on_click("b"));
    ui.with_keyed("list", NodeSpec::column().role(Role::List), |ui| {
        for n in names {
            ui.leaf_keyed(
                n,
                NodeSpec::row()
                    .size(200.0, ROW_H)
                    .role(Role::ListItem)
                    .focusable(),
            );
        }
    });
    ui.with_keyed("dialog", NodeSpec::column().modal("close"), |ui| {
        ui.leaf_keyed("ok", NodeSpec::row().size(60.0, 20.0).on_click("ok"));
    });
    ui.finish();
    assert_eq!(core.focus(), Some(Key::ROOT.str("dialog").str("ok")));
    core.focus_item(list(), 1);
    frame(&mut core, &names, false, None);
    assert_eq!(core.focus(), Some(rows[1]), "not back on Before");
}
