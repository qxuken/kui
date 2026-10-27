//! A context menu in a popup window of its own (backlog F210, ADR 0004
//! amended 2026-10-27): the two cores a host's popup takes. The window's
//! own keeps the menu and draws none of it; the popup's draws the rows,
//! as the in-window menu would, and says back what was chosen rather than
//! performing it — the host hands the row to the window's core.

use kui_core::testing::{click, edit_key, hover};
use kui_core::widgets::MENU_EDGE;
use kui_core::{
    Core, EditKey, Key, Menu, MenuItem, MenuPopupAnswer, Mods, NodeSpec, Rect, Role, Size,
    TextStyle, Value, Vec2,
};

/// The display's work area the popup's rows are held to.
const ROOM: Size = Size {
    w: 1200.0,
    h: 400.0,
};

fn window_frame(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let card = ui.with_keyed(
        "card",
        NodeSpec::column().grow_width().on_menu(Value::Null),
        |ui| {
            ui.text("a page", TextStyle::new(14.0));
        },
    );
    ui.finish();
    card
}

/// A popup's frame: no view, the menu and nothing else.
fn popup_frame(core: &mut Core, size: Size) {
    let ui = core.frame(size, 1.0);
    ui.finish();
}

fn items() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Open").id("open"),
        MenuItem::submenu(
            "Move to",
            vec![
                MenuItem::new("Inbox").id("move.inbox"),
                MenuItem::new("Archive").id("move.archive"),
            ],
        ),
        MenuItem::new("Delete").id("delete"),
    ]
}

fn rows(n: usize) -> Vec<MenuItem> {
    (0..n)
        .map(|i| MenuItem::new(format!("Row {i}")).id(format!("row.{i}")))
        .collect()
}

/// A popup core showing `items`, laid out once at the room's size and
/// then at the size its panel asked for — the host's measure, then its
/// window.
fn popup(items: Vec<MenuItem>, submenu: bool) -> (Core, Size) {
    let mut core = Core::new();
    core.open_menu_popup(items, None, ROOM, submenu);
    popup_frame(&mut core, Size::new(ROOM.w, ROOM.h - 2.0 * MENU_EDGE));
    let size = core.menu_popup_size().expect("the panel was laid out");
    popup_frame(&mut core, size);
    (core, size)
}

fn menus(core: &mut Core) -> Vec<String> {
    core.access_tree()
        .nodes
        .iter()
        .filter(|n| n.role == Role::Menu)
        .map(|n| n.name.clone().unwrap_or_default())
        .collect()
}

fn row_rect(core: &mut Core, label: &str) -> Rect {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::MenuItem && n.name.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no menu row named {label:?}"))
        .rect
}

fn center(r: Rect) -> Vec2 {
    Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn focused(core: &mut Core) -> Option<String> {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.focused)
        .and_then(|n| n.name.clone())
}

#[test]
fn the_window_keeps_a_menu_in_a_popup_and_draws_none_of_it() {
    let mut core = Core::new();
    let card = window_frame(&mut core);
    core.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()));
    core.set_menu_in_popup(true);
    window_frame(&mut core);
    assert!(core.menu().is_some(), "still open, as state");
    assert!(core.menu_in_popup());
    assert!(menus(&mut core).is_empty(), "and not drawn here");
    // The next menu is the window's to draw until the host says again.
    core.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()));
    assert!(!core.menu_in_popup());
    window_frame(&mut core);
    assert_eq!(menus(&mut core), ["Menu"]);
    core.close_menu();
    core.set_menu_in_popup(true);
    assert!(!core.menu_in_popup(), "no menu, nothing to show elsewhere");
}

/// Where the host places menus, a menu waits for it: the frame that
/// opened it — from a view, as most apps open one — draws nothing, so a
/// menu bound for a popup never shows in the window first.
#[test]
fn a_menu_waits_for_a_host_that_places_menus() {
    let mut core = Core::new();
    core.set_menu_popups(true);
    let card = window_frame(&mut core);
    core.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()));
    assert!(core.menu_awaits_host());
    window_frame(&mut core);
    assert!(menus(&mut core).is_empty(), "drawn nowhere yet");
    // It fits: the host says so, and the window draws it.
    core.set_menu_in_popup(false);
    assert!(!core.menu_awaits_host());
    window_frame(&mut core);
    assert_eq!(menus(&mut core), ["Menu"]);
    // The next menu waits again.
    core.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()));
    assert!(core.menu_awaits_host());
    // A host that stops placing menus draws the one that waited.
    core.set_menu_popups(false);
    assert!(!core.menu_awaits_host());
    window_frame(&mut core);
    assert_eq!(menus(&mut core), ["Menu"]);
}

/// A menu that asked to stay in the window (`Menu::in_window`) never
/// waits for the host: it is drawn the frame it opens, where the host
/// places menus too.
#[test]
fn a_menu_kept_in_the_window_does_not_wait_for_the_host() {
    let mut core = Core::new();
    core.set_menu_popups(true);
    let card = window_frame(&mut core);
    core.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()).in_window());
    assert!(!core.menu_awaits_host() && !core.menu_in_popup());
    window_frame(&mut core);
    assert_eq!(menus(&mut core), ["Menu"]);
}

/// The popup is the panel: its window is the size the panel asked for,
/// the panel sits at its top-left, and nothing else is drawn.
#[test]
fn a_popup_is_as_big_as_its_menu() {
    let (mut core, size) = popup(items(), false);
    let panel = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Menu)
        .unwrap()
        .rect;
    assert_eq!((panel.x, panel.y), (0.0, 0.0));
    assert_eq!((panel.w, panel.h), (size.w, size.h));
    assert!(size.h < 200.0, "three rows, not the room: {size:?}");
    assert_eq!(focused(&mut core).as_deref(), Some("Open"), "first row");
}

/// A row chosen in the popup is said back, not performed: the window's
/// core performs it when the host hands it on.
#[test]
fn a_row_chosen_in_a_popup_is_said_back_and_performed_by_the_window() {
    let (mut core, size) = popup(items(), false);
    let at = center(row_rect(&mut core, "Delete"));
    let evs = click(&mut core, at);
    assert!(evs.is_empty(), "the popup posts nothing itself: {evs:?}");
    assert_eq!(
        core.take_menu_popup_answer(),
        Some(MenuPopupAnswer::Chose(vec![2]))
    );
    assert_eq!(core.take_menu_popup_answer(), None, "taken once");
    assert!(core.menu().is_none());
    popup_frame(&mut core, size);

    let mut window = Core::new();
    let card = window_frame(&mut window);
    window.open_menu(Menu::new(card, Vec2::new(20.0, 20.0), items()));
    window.set_menu_in_popup(true);
    let evs = window.activate_menu_path(&[2]).unwrap();
    let item = evs[0].data().and_then(|d| d.get_str("item"));
    assert_eq!(item, Some("delete"), "on the node the menu was about");
    assert!(window.menu().is_none() && !window.menu_in_popup());
}

#[test]
fn escape_dismisses_the_menus_popup_and_goes_back_from_a_submenus() {
    let (mut core, _) = popup(items(), false);
    edit_key(&mut core, EditKey::Escape, Mods::default());
    assert_eq!(
        core.take_menu_popup_answer(),
        Some(MenuPopupAnswer::Dismissed)
    );

    let (mut sub, _) = popup(rows(3), true);
    edit_key(&mut sub, EditKey::Escape, Mods::default());
    assert_eq!(sub.take_menu_popup_answer(), Some(MenuPopupAnswer::Back));

    let (mut sub, _) = popup(rows(3), true);
    edit_key(&mut sub, EditKey::Left, Mods::default());
    assert_eq!(sub.take_menu_popup_answer(), Some(MenuPopupAnswer::Back));
    assert!(
        sub.menu().is_some(),
        "Left asks; the host closes the window"
    );
}

/// A submenu is a popup of its own: the popup marks its row open and says
/// where it is, and draws no panel beside it.
#[test]
fn a_popup_opens_no_submenu_of_its_own_and_says_where_its_row_is() {
    let (mut core, size) = popup(items(), false);
    let row = row_rect(&mut core, "Move to");
    hover(&mut core, center(row));
    popup_frame(&mut core, size);
    assert_eq!(core.menu_submenus(), &[1]);
    let sub = core.menu_popup_submenu().expect("open");
    assert_eq!(sub.row, 1);
    assert_eq!(sub.rect, row);
    assert!(!sub.keyboard, "the pointer opened it");
    assert_eq!(menus(&mut core), ["Menu"], "no panel beside it");

    // The keyboard: down to the row, Right opens it.
    let (mut core, size) = popup(items(), false);
    edit_key(&mut core, EditKey::Down, Mods::default());
    popup_frame(&mut core, size);
    edit_key(&mut core, EditKey::Right, Mods::default());
    popup_frame(&mut core, size);
    let sub = core.menu_popup_submenu().expect("open");
    assert!(sub.keyboard);
    // Its popup went back: closed, and the keyboard on its row.
    core.close_menu_popup_submenu();
    popup_frame(&mut core, size);
    assert_eq!(core.menu_popup_submenu(), None);
    assert_eq!(focused(&mut core).as_deref(), Some("Move to"));
}

/// Taller than the display's room, the popup is cut to the room less the
/// margin, as an in-window menu is to the window, and scrolls; its window
/// is that size, and the keyboard keeps its row in view.
#[test]
fn a_popup_taller_than_the_room_is_cut_to_it_and_scrolls() {
    let (mut core, size) = popup(rows(60), false);
    assert!(
        (size.h - (ROOM.h - 2.0 * MENU_EDGE)).abs() < 0.5,
        "the room less the margin: {size:?}"
    );
    let menu = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Menu)
        .cloned()
        .unwrap();
    assert_eq!(
        (menu.rect.y, menu.rect.h),
        (0.0, size.h),
        "fills its window"
    );
    let g = core.scroll_geometry(menu.key).unwrap();
    assert!(g.content.h > g.rect.h);
    edit_key(&mut core, EditKey::End, Mods::default());
    popup_frame(&mut core, size);
    assert_eq!(focused(&mut core).as_deref(), Some("Row 59"));
    let r = row_rect(&mut core, "Row 59");
    assert!(r.y >= 0.0 && r.y + r.h <= size.h + 0.5, "in view: {r:?}");
}

/// A row's width is held to the display's room, not to the window it is
/// drawn in: in a window exactly as wide as the panel, a long label keeps
/// the width it was measured at rather than being cut by the margin a
/// window's menu keeps from its edges — so the panel does not shrink away
/// from its window's right edge.
#[test]
fn a_popups_rows_are_as_wide_as_the_room_lets_them_be() {
    let long = "A recent file with a long path, under a folder under another folder";
    let (mut core, size) = popup(vec![MenuItem::new(long).accel("mod+o")], false);
    assert!(size.w > 400.0, "measured against the room: {size:?}");
    let panel = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Menu)
        .unwrap()
        .rect;
    assert_eq!(panel.w, size.w, "still the window's width");
}

/// A popup is held to the menu's own limit, as the window's menu is.
#[test]
fn a_popup_is_held_to_the_menus_max_size() {
    let mut core = Core::new();
    core.open_menu_popup(rows(60), Some(Size::new(f32::INFINITY, 200.0)), ROOM, false);
    popup_frame(&mut core, Size::new(ROOM.w, ROOM.h - 2.0 * MENU_EDGE));
    let size = core.menu_popup_size().unwrap();
    assert!((size.h - 200.0).abs() < 0.5, "{size:?}");
}
