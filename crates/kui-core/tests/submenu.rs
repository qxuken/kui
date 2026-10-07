//! Submenus (backlog F128, from Noticon): a row that opens a menu of its
//! own beside it — "Move to ▸", "Sort by ▸" — in the drawn context menu
//! and the drawn menu bar, by the pointer and by the keyboard, and the
//! doors a host that shows menus itself reports a row inside one through.

use kui_core::testing::{click, edit_key, hover};
use kui_core::{
    BarMenu, Core, EditKey, Key, Menu, MenuBar, MenuItem, Mods, NodeSpec, Rect, Role, Size,
    TextStyle, Value, Vec2,
};

/// A card of text the menus are about, with whatever the core has open
/// drawn over it by `Ui::finish`; the bar when one is given.
fn frame_with(core: &mut Core, bar: Option<MenuBar>) -> Key {
    let mut ui = core.frame(Size::new(600.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    if let Some(bar) = bar {
        kui_core::widgets::menu_bar(&mut ui, bar);
    }
    let card = ui.with_keyed("card", NodeSpec::column().grow_width(), |ui| {
        ui.text("a note", TextStyle::new(14.0));
    });
    ui.finish();
    card
}

fn frame(core: &mut Core) -> Key {
    frame_with(core, None)
}

/// Noticon's note menu, cut down: a plain row, a submenu of folders with
/// one disabled, a submenu nested in a submenu, and a plain row after.
fn items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Open").id("open"),
        MenuItem::submenu(
            "Move to",
            vec![
                MenuItem::new("Inbox").id("move.inbox"),
                MenuItem::new("Archive").id("move.archive").enabled(false),
                MenuItem::submenu(
                    "Projects",
                    vec![MenuItem::new("kui").id("move.kui").accel("mod+shift+k")],
                ),
            ],
        ),
        MenuItem::new("Delete").id("delete"),
    ]
}

fn open(core: &mut Core) -> Key {
    let card = frame(core);
    core.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()));
    frame(core);
    card
}

/// The access node for the menu row `label`, if it is drawn.
fn row(core: &mut Core, label: &str) -> Option<kui_core::AccessNode> {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::MenuItem && n.name.as_deref() == Some(label))
        .cloned()
}

fn center(r: Rect) -> Vec2 {
    Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn row_center(core: &mut Core, label: &str) -> Vec2 {
    center(
        row(core, label)
            .unwrap_or_else(|| panic!("no menu row named {label:?}"))
            .rect,
    )
}

fn menus(core: &mut Core) -> Vec<String> {
    core.access_tree()
        .nodes
        .iter()
        .filter(|n| n.role == Role::Menu)
        .map(|n| n.name.clone().unwrap_or_default())
        .collect()
}

fn key(core: &mut Core, k: EditKey) -> Vec<kui_core::UiEvent> {
    edit_key(core, k, Mods::default())
}

fn focused_row(core: &mut Core) -> Option<String> {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.focused)
        .and_then(|n| n.name.clone())
}

#[test]
fn a_submenu_row_draws_closed_and_reads_as_collapsed() {
    let mut core = Core::new();
    open(&mut core);
    assert_eq!(menus(&mut core), ["Menu"], "only the menu itself is drawn");
    let move_to = row(&mut core, "Move to").expect("the row is drawn");
    assert_eq!(
        move_to.expanded,
        Some(false),
        "a reader hears it opens something"
    );
    assert!(
        row(&mut core, "Inbox").is_none(),
        "nothing inside is drawn yet"
    );
    assert_eq!(core.menu_submenus(), &[] as &[usize]);
}

/// Resting the pointer on the row opens its submenu beside it, level with
/// it; resting on another row of the same menu closes it again.
#[test]
fn the_pointer_opens_a_submenu_and_another_row_closes_it() {
    let mut core = Core::new();
    open(&mut core);
    let at = row_center(&mut core, "Move to");
    hover(&mut core, at);
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1]);
    assert_eq!(menus(&mut core), ["Menu", "Move to"]);
    let parent = row(&mut core, "Move to").unwrap();
    assert_eq!(parent.expanded, Some(true));
    let inbox = row(&mut core, "Inbox")
        .expect("the submenu's rows are drawn")
        .rect;
    assert!(
        inbox.x >= parent.rect.x + parent.rect.w,
        "beside its row: {inbox:?} against {:?}",
        parent.rect
    );
    assert!(
        (inbox.y - parent.rect.y).abs() < 1.0,
        "its first row level with it: {} against {}",
        inbox.y,
        parent.rect.y
    );

    // Into the submenu and down a level.
    let at = row_center(&mut core, "Projects");
    hover(&mut core, at);
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1, 2]);
    assert!(row(&mut core, "kui").is_some());

    // Back on a plain row of the first submenu: the nested one closes.
    let at = row_center(&mut core, "Inbox");
    hover(&mut core, at);
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1]);
    assert!(row(&mut core, "kui").is_none());

    // And on a plain row of the menu itself: everything closes.
    let at = row_center(&mut core, "Delete");
    hover(&mut core, at);
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[] as &[usize]);
    assert_eq!(menus(&mut core), ["Menu"]);
}

/// A row inside a submenu is chosen as any row is: its own `menu` event,
/// on the node the menu is about, and the whole menu closes. A click on a
/// row that opens a submenu chooses nothing and leaves the menu open.
#[test]
fn choosing_a_row_inside_posts_it_and_closes_every_level() {
    let mut core = Core::new();
    let card = open(&mut core);
    let at = row_center(&mut core, "Move to");
    let evs = click(&mut core, at);
    assert!(
        evs.is_empty(),
        "the submenu's row is nobody's click: {evs:?}"
    );
    assert!(core.menu().is_some(), "still open");
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1], "and the click opened it");

    let at = row_center(&mut core, "Projects");
    click(&mut core, at);
    frame(&mut core);
    let at = row_center(&mut core, "kui");
    let evs = click(&mut core, at);
    assert_eq!(evs.len(), 1, "{evs:?}");
    assert_eq!(evs[0].key, card, "on the node the menu is about");
    assert_eq!(evs[0].kind(), Some("menu"));
    assert_eq!(evs[0].payload.get_str("item"), Some("move.kui"));
    assert!(core.menu().is_none(), "every level closes");
    assert_eq!(core.menu_submenus(), &[] as &[usize]);
}

/// A disabled row inside a submenu is inert, as one outside is.
#[test]
fn a_disabled_row_inside_is_inert() {
    let mut core = Core::new();
    open(&mut core);
    let at = row_center(&mut core, "Move to");
    click(&mut core, at);
    frame(&mut core);
    let at = row_center(&mut core, "Archive");
    let evs = click(&mut core, at);
    assert!(evs.is_empty(), "{evs:?}");
    assert!(core.menu().is_some());
}

/// The keyboard: Right on a row with a submenu opens it with focus on its
/// first row, the arrows walk that submenu's rows, Left closes it with
/// focus back on its row, and Escape closes the innermost submenu before
/// it closes the menu.
#[test]
fn the_arrows_open_and_close_a_submenu_and_escape_closes_one_level() {
    let mut core = Core::new();
    open(&mut core);
    assert_eq!(
        focused_row(&mut core).as_deref(),
        Some("Open"),
        "the menu opens on its first row"
    );
    key(&mut core, EditKey::Down);
    frame(&mut core);
    assert_eq!(focused_row(&mut core).as_deref(), Some("Move to"));

    // Right opens it, and the first row inside has the keyboard.
    let evs = key(&mut core, EditKey::Right);
    assert!(evs.is_empty(), "{evs:?}");
    assert_eq!(core.menu_submenus(), &[1], "opened by the key");
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1]);
    assert_eq!(focused_row(&mut core).as_deref(), Some("Inbox"));

    // The arrows walk the submenu's rows, stepping over the dead one.
    key(&mut core, EditKey::Down);
    frame(&mut core);
    assert_eq!(focused_row(&mut core).as_deref(), Some("Projects"));
    key(&mut core, EditKey::Right);
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1, 2]);
    assert_eq!(focused_row(&mut core).as_deref(), Some("kui"));

    // Left: back out a level, on the row it hung from.
    key(&mut core, EditKey::Left);
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1]);
    assert_eq!(focused_row(&mut core).as_deref(), Some("Projects"));

    // Escape: the submenu, not the menu.
    let evs = key(&mut core, EditKey::Escape);
    assert!(evs.is_empty(), "{evs:?}");
    frame(&mut core);
    assert!(core.menu().is_some(), "the menu is still open");
    assert_eq!(core.menu_submenus(), &[] as &[usize]);
    assert_eq!(focused_row(&mut core).as_deref(), Some("Move to"));
    // And again: now the menu.
    key(&mut core, EditKey::Escape);
    assert!(core.menu().is_none());

    // Enter on the row opens it too, from the keyboard, focus inside.
    let mut core = Core::new();
    open(&mut core);
    key(&mut core, EditKey::Down);
    frame(&mut core);
    assert_eq!(focused_row(&mut core).as_deref(), Some("Move to"));
    key(&mut core, EditKey::Enter);
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1]);
    assert_eq!(focused_row(&mut core).as_deref(), Some("Inbox"));
    let evs = key(&mut core, EditKey::Enter);
    assert_eq!(evs.len(), 1, "{evs:?}");
    assert_eq!(evs[0].payload.get_str("item"), Some("move.inbox"));
}

/// A pointer resting on a row does not undo what the keyboard did: the
/// hover opens and closes on a change of row only.
#[test]
fn a_resting_pointer_does_not_undo_the_keyboard() {
    let mut core = Core::new();
    open(&mut core);
    let at = row_center(&mut core, "Open");
    hover(&mut core, at);
    frame(&mut core);
    key(&mut core, EditKey::Down);
    frame(&mut core);
    key(&mut core, EditKey::Right);
    frame(&mut core);
    frame(&mut core);
    assert_eq!(
        core.menu_submenus(),
        &[1],
        "still open under a pointer on Open"
    );
}

/// A submenu's accelerator is the platform's spelling too, and the row
/// with the submenu draws a chevron where an accelerator would be.
#[test]
fn a_row_inside_reads_its_accelerator_in_the_platforms_spelling() {
    let mut core = Core::new();
    open(&mut core);
    let kui = MenuItem::at_path(&core.menu().unwrap().items, &[1, 2, 0])
        .unwrap()
        .accel
        .clone();
    let want = if cfg!(target_os = "macos") {
        "\u{21e7}\u{2318}K"
    } else {
        "Ctrl+Shift+K"
    };
    assert_eq!(kui.as_deref(), Some(want));
}

/// A host that shows menus itself reports a row inside a submenu by its
/// path; a row that opens one is refused, as the platform never reports
/// it, and so is a dead one.
#[test]
fn a_host_reports_a_row_inside_by_its_path() {
    let mut core = Core::new();
    let card = open(&mut core);
    core.set_native_menus(true);
    assert_eq!(
        core.activate_menu_item(1),
        None,
        "the row that opens a submenu"
    );
    assert_eq!(core.activate_menu_path(&[1]), None);
    assert_eq!(core.activate_menu_path(&[1, 1]), None, "a dead row inside");
    assert!(core.menu().is_some());
    let evs = core.activate_menu_path(&[1, 2, 0]).expect("taken");
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, card);
    assert_eq!(evs[0].payload.get_str("item"), Some("move.kui"));
    assert!(core.menu().is_none());
}

/// The rows come from plain data the way every binding sends them: a
/// row's `items` are its submenu.
#[test]
fn a_rows_items_are_its_submenu() {
    let v = Value::List(vec![Value::map([
        ("label", Value::str("Sort by")),
        (
            "items",
            Value::List(vec![
                Value::map([
                    ("label", Value::str("Name")),
                    ("checked", Value::Bool(true)),
                ]),
                Value::map([("label", Value::str("Date"))]),
            ]),
        ),
    ])]);
    let rows = MenuItem::list_from_value(&v).unwrap();
    assert!(rows[0].has_submenu());
    assert_eq!(rows[0].submenu.len(), 2);
    assert!(rows[0].submenu[0].checked);
    assert!(MenuItem::KEYS.contains(&"items"));
    // A row in a submenu is read with the same rules as any row.
    let bad = Value::List(vec![Value::map([
        ("label", Value::str("Sort by")),
        (
            "items",
            Value::List(vec![Value::map([("role", Value::str("custom"))])]),
        ),
    ])]);
    assert!(MenuItem::list_from_value(&bad).is_err());
}

/// The drawn menu bar's menus nest the same way: a title opens its menu,
/// the pointer opens a submenu in it, and a row inside posts its item.
#[test]
fn the_drawn_bar_opens_submenus_too() {
    let bar = || {
        MenuBar::new(vec![BarMenu::new(
            "View",
            vec![
                MenuItem::new("Zoom in").id("zoom"),
                MenuItem::submenu(
                    "Sort by",
                    vec![
                        MenuItem::new("Name").id("sort.name").checked(true),
                        MenuItem::new("Date").id("sort.date"),
                    ],
                ),
            ],
        )])
    };
    let mut core = Core::new();
    frame_with(&mut core, Some(bar()));
    let at = row_center(&mut core, "View");
    click(&mut core, at);
    frame_with(&mut core, Some(bar()));
    assert_eq!(core.menu_bar_open(), Some(0));
    let at = row_center(&mut core, "Sort by");
    hover(&mut core, at);
    frame_with(&mut core, Some(bar()));
    assert_eq!(core.menu_bar_submenus(), &[1]);
    let at = row_center(&mut core, "Date");
    let evs = click(&mut core, at);
    assert_eq!(evs.len(), 1, "{evs:?}");
    assert_eq!(evs[0].payload.get_str("item"), Some("sort.date"));
    assert_eq!(core.menu_bar_open(), None, "the bar's menu closes");
    assert_eq!(core.menu_bar_submenus(), &[] as &[usize]);

    // And the platform's bar reports a row inside by its path.
    let evs = core.activate_menu_bar_path(0, &[1, 0]);
    assert_eq!(evs[0].payload.get_str("item"), Some("sort.name"));
    assert!(
        core.activate_menu_bar_path(0, &[1]).is_empty(),
        "the submenu's own row"
    );
}
