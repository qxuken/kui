//! A composite hears the keys its items do not claim (backlog C120):
//! `onItemKey` on a `list`, a `table` or any other composite container
//! takes Delete, Backspace and the rest of the keys its focused item
//! leaves, with the item named — while the container stays no Tab stop
//! and the items keep their arrows, which a sink on either would take.

use kui_core::testing::{key_down, key_up, tab, tags};
use kui_core::{
    Core, EditKey, Event, EventKind, InputEvent, ItemKey, Key, KeyCode, KeyMods, Mods, NodeSpec,
    Role, Size, Ui, UiEvent,
};

/// A button, a checklist of three focusable rows (opened by data index
/// 10, 11, 12 when `indexed`) whose list declares `onItemKey`, and a
/// button after; the whole under a shell sink when `shell`. Row 1 holds a
/// focusable control when `control`. Returns the rows' keys.
fn frame(core: &mut Core, indexed: bool, shell: bool, control: bool) -> Vec<Key> {
    let mut rows = Vec::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let mut root = NodeSpec::column().fill();
    if shell {
        root = root.on_key("shell");
    }
    ui.configure_root(root);
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
            for r in 0..3 {
                let row = NodeSpec::row()
                    .size(200.0, 20.0)
                    .role(Role::ListItem)
                    .focusable()
                    .on_click(format!("tick{r}"))
                    .label(format!("Row {r}"));
                let inner = |ui: &mut Ui<'_>| {
                    if control && r == 1 {
                        ui.leaf_keyed(
                            "x",
                            NodeSpec::row()
                                .size(16.0, 16.0)
                                .on_click("x")
                                .label("Remove"),
                        );
                    }
                };
                rows.push(if indexed {
                    ui.with_indexed(10 + r as u64, row, inner)
                } else {
                    ui.with_keyed(&format!("i{r}"), row, inner)
                });
            }
        },
    );
    ui.leaf_keyed(
        "after",
        NodeSpec::row()
            .size(60.0, 20.0)
            .on_click("a")
            .label("After"),
    );
    ui.finish();
    rows
}

/// A key as a keyboard sends it: the raw press, then the editing key a
/// press of it also is, then its release.
fn press(core: &mut Core, code: KeyCode, mods: KeyMods) -> Vec<UiEvent> {
    let mut out = key_down(core, code, mods);
    let ek = match code {
        KeyCode::Delete => Some(EditKey::Delete),
        KeyCode::Backspace => Some(EditKey::Backspace),
        KeyCode::Enter => Some(EditKey::Enter),
        KeyCode::Escape => Some(EditKey::Escape),
        KeyCode::Down => Some(EditKey::Down),
        _ => None,
    };
    if let Some(ek) = ek {
        let m = Mods {
            shift: mods.shift,
            ..Mods::NONE
        };
        out.extend(core.handle_input(InputEvent::Key(ek, m)));
    }
    out.extend(key_up(core, code, mods));
    out
}

fn item_keys(evs: &[UiEvent]) -> Vec<ItemKey> {
    evs.iter()
        .filter(|e| e.event == EventKind::ItemKey)
        .filter_map(|e| match e.read() {
            Event::ItemKey(k) => Some(k),
            _ => None,
        })
        .collect()
}

#[test]
fn delete_on_a_row_reaches_the_list_with_the_row_named() {
    let mut core = Core::new();
    let rows = frame(&mut core, true, false, false);
    core.set_focus(Some(Key::ROOT.str("before")));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(rows[0]), "Tab enters on a row");
    core.handle_input(InputEvent::Key(EditKey::Down, Mods::NONE));
    assert_eq!(core.focus(), Some(rows[1]), "the rows keep their arrows");

    let evs = press(&mut core, KeyCode::Delete, KeyMods::NONE);
    let heard = item_keys(&evs);
    assert_eq!(heard.len(), 1, "one itemkey: {evs:?}");
    assert_eq!(heard[0].press.code, KeyCode::Delete);
    assert_eq!(heard[0].item, rows[1]);
    assert_eq!(heard[0].index, 11, "the row's data index");
    assert_eq!(heard[0].column, None, "a list has no columns");
    assert_eq!(evs[0].key, Key::ROOT.str("list"), "on the list");
    assert_eq!(tags(&evs), ["item-key"], "and nothing else: {evs:?}");
    assert_eq!(core.focus(), Some(rows[1]), "focus stays on the row");

    let evs = press(&mut core, KeyCode::Backspace, KeyMods::NONE);
    assert_eq!(item_keys(&evs)[0].press.code, KeyCode::Backspace);
    let evs = press(&mut core, KeyCode::Delete, KeyMods::NONE.with_shift());
    assert_eq!(item_keys(&evs).len(), 1, "Shift is no chord");

    tab(&mut core, false);
    assert_eq!(
        core.focus(),
        Some(Key::ROOT.str("after")),
        "the list is one Tab stop and no stop of its own"
    );
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(rows[1]), "back onto the row it left");
}

#[test]
fn the_keys_the_composite_moves_with_are_not_item_keys() {
    let mut core = Core::new();
    let rows = frame(&mut core, true, false, false);
    core.set_focus(Some(rows[0]));
    let evs = press(&mut core, KeyCode::Down, KeyMods::NONE);
    assert!(item_keys(&evs).is_empty(), "an arrow moves: {evs:?}");
    assert_eq!(core.focus(), Some(rows[1]));
    let evs = press(&mut core, KeyCode::Enter, KeyMods::NONE);
    assert!(
        item_keys(&evs).is_empty(),
        "Enter presses a row that clicks"
    );
    assert_eq!(tags(&evs), ["tick1"]);
    let mut evs = key_down(&mut core, KeyCode::Char('r'), KeyMods::NONE);
    evs.extend(core.handle_input(InputEvent::Text("r".into())));
    assert!(item_keys(&evs).is_empty(), "a letter searches: {evs:?}");
    assert_eq!(core.focus(), Some(rows[2]), "onto the next row it names");
    // A chord is a shortcut layer's: never the composite's.
    let evs = press(&mut core, KeyCode::Delete, KeyMods::NONE.with_super());
    assert!(item_keys(&evs).is_empty(), "⌘Delete is no item key");
    // Escape lets go, as it does everywhere.
    let evs = press(&mut core, KeyCode::Escape, KeyMods::NONE);
    assert!(item_keys(&evs).is_empty());
    assert_eq!(core.focus(), None, "Escape let go of the row");
}

#[test]
fn a_row_with_no_index_is_counted_by_its_place() {
    let mut core = Core::new();
    let rows = frame(&mut core, false, false, false);
    core.set_focus(Some(rows[2]));
    let heard = item_keys(&press(&mut core, KeyCode::Delete, KeyMods::NONE));
    assert_eq!(heard[0].index, 2);
}

#[test]
fn the_list_is_nearer_than_a_shell_sink_which_keeps_the_chords() {
    let mut core = Core::new();
    let rows = frame(&mut core, true, true, false);
    core.set_focus(Some(rows[0]));
    let evs = press(&mut core, KeyCode::Delete, KeyMods::NONE);
    assert_eq!(item_keys(&evs).len(), 1);
    assert!(
        !evs.iter().any(|e| e.event == EventKind::Key),
        "the shell does not hear it too: {evs:?}"
    );
    let evs = press(&mut core, KeyCode::Char('n'), KeyMods::NONE.with_super());
    assert!(item_keys(&evs).is_empty());
    assert!(
        evs.iter().any(|e| e.event == EventKind::Key),
        "⌘N still reaches the shell: {evs:?}"
    );
}

#[test]
fn a_control_inside_a_row_reports_the_row() {
    let mut core = Core::new();
    let rows = frame(&mut core, true, false, true);
    core.set_focus(Some(rows[1].str("x")));
    let heard = item_keys(&press(&mut core, KeyCode::Delete, KeyMods::NONE));
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].item, rows[1], "the row the control is in");
    assert_eq!(heard[0].index, 11);
}

#[test]
fn a_disabled_list_hears_nothing() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut row = Key::ROOT;
    ui.with_keyed(
        "list",
        NodeSpec::column()
            .role(Role::List)
            .label("L")
            .on_item_key("k")
            .disabled(true),
        |ui| {
            row = ui.leaf_keyed(
                "i0",
                NodeSpec::row()
                    .size(100.0, 20.0)
                    .role(Role::ListItem)
                    .focusable()
                    .label("a"),
            );
        },
    );
    ui.finish();
    core.set_focus(Some(row));
    let evs = press(&mut core, KeyCode::Delete, KeyMods::NONE);
    assert!(item_keys(&evs).is_empty(), "{evs:?}");
}

#[test]
fn a_table_names_the_cells_row_and_column() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "grid",
        NodeSpec::column()
            .role(Role::Table)
            .label("t")
            .on_item_key("cell-key"),
        |ui| {
            for r in 0..2 {
                ui.with_keyed(&format!("r{r}"), NodeSpec::row().role(Role::Row), |ui| {
                    for c in 0..3 {
                        ui.leaf_keyed(
                            &format!("c{c}"),
                            NodeSpec::row()
                                .size(40.0, 20.0)
                                .role(Role::Cell)
                                .focusable()
                                .label(format!("{r}{c}")),
                        );
                    }
                });
            }
        },
    );
    ui.finish();
    let cell = Key::ROOT.str("grid").str("r1").str("c2");
    core.set_focus(Some(cell));
    let heard = item_keys(&press(&mut core, KeyCode::Delete, KeyMods::NONE));
    assert_eq!(heard.len(), 1);
    assert_eq!(heard[0].item, cell);
    assert_eq!((heard[0].index, heard[0].column), (1, Some(2)));
}

/// The platform's primary modifier: ⌘ on a Mac, Ctrl elsewhere.
fn primary() -> KeyMods {
    if cfg!(target_os = "macos") {
        KeyMods::NONE.with_super()
    } else {
        KeyMods::NONE.with_ctrl()
    }
}

/// Select all — the primary modifier and A — is the one chord a
/// composite's `onItemKey` hears: in a list or a table select all is the
/// rows' (Kit's multiple selection). Any other chord still bubbles past.
#[test]
fn select_all_is_the_one_chord_an_item_key_hears() {
    let mut core = Core::new();
    let rows = frame(&mut core, true, true, false);
    core.set_focus(Some(rows[1]));
    let mut evs = key_down(&mut core, KeyCode::Char('a'), primary());
    evs.extend(key_up(&mut core, KeyCode::Char('a'), primary()));
    let heard = item_keys(&evs);
    assert_eq!(heard.len(), 1, "{evs:?}");
    assert_eq!(heard[0].press.code, KeyCode::Char('a'));
    assert_eq!(heard[0].index, 11);
    assert!(
        !evs.iter().any(|e| e.event == EventKind::Key),
        "the shell does not hear it too: {evs:?}"
    );
    assert_eq!(core.focus(), Some(rows[1]), "no type-ahead moved focus");
    let evs = press(&mut core, KeyCode::Char('n'), primary());
    assert!(item_keys(&evs).is_empty(), "⌘N is the shell's: {evs:?}");
}

/// A list of three rows whose messages are maps, each reporting its focus.
fn map_rows(core: &mut Core) -> Vec<Key> {
    let mut rows = Vec::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "list",
        NodeSpec::column().role(Role::List).label("Rows"),
        |ui| {
            for r in 0..3 {
                let msg = |kind: &str| {
                    kui_core::Value::map([
                        ("kind", kui_core::Value::str(kind)),
                        ("row", kui_core::Value::Int(r)),
                    ])
                };
                rows.push(
                    ui.leaf_keyed(
                        &format!("r{r}"),
                        NodeSpec::row()
                            .size(200.0, 20.0)
                            .role(Role::ListItem)
                            .focusable()
                            .on_click(msg("click"))
                            .on_focus(msg("focus"))
                            .label(format!("Row {r}")),
                    ),
                );
            }
        },
    );
    ui.leaf_keyed(
        "bare",
        NodeSpec::row()
            .size(60.0, 20.0)
            .on_click("bare")
            .label("Bare"),
    );
    ui.finish();
    rows
}

fn data_of(evs: &[UiEvent], kind: EventKind) -> Vec<kui_core::Value> {
    evs.iter()
        .filter(|e| e.event == kind)
        .map(|e| e.data().cloned().unwrap_or(kui_core::Value::Null))
        .collect()
}

/// A click says what it was made with (backlog C101's facts): Enter or
/// Space, a modifier held, the second click of a double click — while a
/// plain click and a bare message arrive as declared. A focus move the
/// keyboard made under Shift says so too: what a list extending a range
/// reads.
#[test]
fn a_click_and_a_focus_move_say_what_they_were_made_with() {
    use kui_core::{Value, Vec2};
    let mut core = Core::new();
    let rows = map_rows(&mut core);
    core.set_focus(Some(rows[0]));
    let evs = press(&mut core, KeyCode::Enter, KeyMods::NONE);
    assert_eq!(
        data_of(&evs, EventKind::Click)[0].get_str("key"),
        Some("enter")
    );
    let mut evs = key_down(&mut core, KeyCode::Space, KeyMods::NONE);
    evs.extend(core.handle_input(InputEvent::Text(" ".into())));
    assert_eq!(
        data_of(&evs, EventKind::Click)[0].get_str("key"),
        Some("space")
    );

    // The pointer: a plain click carries nothing; ⌘ held, its mods; the
    // second click of a double click, the count.
    let click = |core: &mut Core, y: f32, mods: KeyMods, clicks: u8| {
        core.handle_input(InputEvent::Modifiers(mods));
        core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, y)));
        let mut out = core.handle_input(InputEvent::mouse_down(clicks));
        out.extend(core.handle_input(InputEvent::mouse_up()));
        core.handle_input(InputEvent::Modifiers(KeyMods::NONE));
        out
    };
    let evs = click(&mut core, 30.0, KeyMods::NONE, 1);
    assert_eq!(
        data_of(&evs, EventKind::Click),
        [Value::Null],
        "a plain click as declared: {evs:?}"
    );
    let evs = click(&mut core, 30.0, primary(), 1);
    let mods = data_of(&evs, EventKind::Click)[0]
        .get("mods")
        .cloned()
        .expect("mods");
    assert_eq!(
        mods.get_bool(if cfg!(target_os = "macos") {
            "super"
        } else {
            "ctrl"
        }),
        Some(true)
    );
    let evs = click(&mut core, 30.0, KeyMods::NONE, 2);
    assert_eq!(
        data_of(&evs, EventKind::Click)[0].get_int("clicks"),
        Some(2)
    );
    let evs = click(&mut core, 70.0, primary(), 2);
    assert_eq!(
        kui_core::testing::tags(&evs),
        ["bare"],
        "a bare message stays bare: {evs:?}"
    );

    // Shift+Down: the row it lands on hears focus with the modifiers.
    core.set_focus(Some(rows[0]));
    let evs = press(&mut core, KeyCode::Down, KeyMods::NONE.with_shift());
    let focus = data_of(&evs, EventKind::Focus);
    let landed = focus
        .iter()
        .find(|d| d.get_str("phase") == Some("in") && d.get_str("by") == Some("keyboard"))
        .expect("a focus in by the keyboard");
    assert_eq!(
        landed.get("mods").and_then(|m| m.get_bool("shift")),
        Some(true),
        "{landed:?}"
    );
    let evs = press(&mut core, KeyCode::Down, KeyMods::NONE);
    let focus = data_of(&evs, EventKind::Focus);
    assert!(
        focus.iter().all(|d| d.get("mods").is_none()),
        "no modifier, no mods: {focus:?}"
    );
}
