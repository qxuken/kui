//! A menu taller than the window (backlog F210, from Carnet's page menu in
//! a short window): the core's context menu, its submenus and the drawn
//! bar's dropped menu are cut to the window less `MENU_EDGE` above and
//! below and scroll — by the wheel, and by the keyboard, whose row is kept
//! in view — while a menu that fits is placed exactly where it was.

use kui_core::testing::{edit_key, hover};
use kui_core::widgets::MENU_EDGE;
use kui_core::{
    BarMenu, Core, EditKey, InputEvent, Key, Menu, MenuBar, MenuItem, Mods, NodeSpec, Rect, Role,
    Size, TextStyle, Value, Vec2,
};

const W: f32 = 600.0;
const H: f32 = 300.0;

fn frame_with(core: &mut Core, bar: Option<MenuBar>) -> Key {
    let mut ui = core.frame(Size::new(W, H), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    if let Some(bar) = bar {
        kui_core::widgets::menu_bar(&mut ui, bar);
    }
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

fn frame(core: &mut Core) -> Key {
    frame_with(core, None)
}

/// `n` rows, "Row 0" to "Row n-1", each its own id.
fn rows(n: usize) -> Vec<MenuItem> {
    (0..n)
        .map(|i| MenuItem::new(format!("Row {i}")).id(format!("row.{i}")))
        .collect()
}

fn open(core: &mut Core, at: Vec2, items: Vec<MenuItem>) -> Key {
    let card = frame(core);
    core.open_menu(Menu::new(card, at, items));
    frame(core);
    card
}

/// The menu panel named `name` — the context menu is "Menu", a submenu
/// its row's label — with its key and drawn rect.
fn panel(core: &mut Core, name: &str) -> (Key, Rect) {
    let n = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::Menu && n.name.as_deref() == Some(name))
        .cloned()
        .unwrap_or_else(|| panic!("no menu named {name:?}"));
    (n.key, n.rect)
}

fn row_rect(core: &mut Core, label: &str) -> Rect {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::MenuItem && n.name.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no menu row named {label:?}"))
        .rect
}

fn focused(core: &mut Core) -> Option<String> {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.focused)
        .and_then(|n| n.name.clone())
}

/// Whether `r` lies wholly inside the panel's box, and the window's: a
/// row past the window's bottom is not one anybody can see.
fn inside(r: Rect, panel: Rect) -> bool {
    let (top, bottom) = (panel.y.max(0.0), (panel.y + panel.h).min(H));
    r.h > 0.0 && r.y >= top - 0.5 && r.y + r.h <= bottom + 0.5
}

fn key(core: &mut Core, k: EditKey) {
    edit_key(core, k, Mods::default());
    frame(core);
}

#[test]
fn a_menu_that_fits_is_placed_at_the_point_and_does_not_scroll() {
    let mut core = Core::new();
    open(&mut core, Vec2::new(40.0, 30.0), rows(3));
    let (k, r) = panel(&mut core, "Menu");
    assert_eq!((r.x, r.y), (40.0, 30.0), "top-left at the point, as before");
    let g = core.scroll_geometry(k).expect("the panel is a scroller");
    assert!(g.content.h <= g.rect.h + 0.5, "nothing to scroll: {g:?}");
}

/// Near the bottom a menu that fits still slides up to the window's edge,
/// as `fit` always did: the margin is for a menu that does not fit.
#[test]
fn a_menu_that_fits_near_the_bottom_is_clamped_as_before() {
    let mut core = Core::new();
    open(&mut core, Vec2::new(40.0, 280.0), rows(3));
    let (_, r) = panel(&mut core, "Menu");
    assert!(
        (r.y + r.h - H).abs() < 0.5,
        "its bottom on the window's: {r:?}"
    );
}

#[test]
fn a_menu_taller_than_the_window_fills_the_room_and_scrolls() {
    let mut core = Core::new();
    open(&mut core, Vec2::new(40.0, 150.0), rows(40));
    let (k, r) = panel(&mut core, "Menu");
    assert_eq!(r.y, MENU_EDGE, "as high as it can go: {r:?}");
    assert!(
        (r.h - (H - 2.0 * MENU_EDGE)).abs() < 0.5,
        "cut to the window less the margin twice: {r:?}"
    );
    let g = core.scroll_geometry(k).unwrap();
    assert!(g.content.h > g.rect.h, "it overflows, so it scrolls: {g:?}");
    assert_eq!(r.x, 40.0, "across, still at the point");
    assert!(
        inside(row_rect(&mut core, "Row 0"), r),
        "the first row shows"
    );
    assert!(
        !inside(row_rect(&mut core, "Row 39"), r),
        "the last one is past the cut until scrolled to"
    );
}

#[test]
fn the_wheel_scrolls_a_cut_menu() {
    let mut core = Core::new();
    open(&mut core, Vec2::new(40.0, 150.0), rows(40));
    let (k, r) = panel(&mut core, "Menu");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(r.x + 20.0, r.y + 40.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -10_000.0)));
    frame(&mut core);
    assert!(core.scroll_offset(k).y > 0.0, "moved");
    assert!(
        inside(row_rect(&mut core, "Row 39"), panel(&mut core, "Menu").1),
        "the last row reached"
    );
    assert!(core.menu().is_some(), "and the menu is still open");
}

/// End, Home and the arrows walk the rows the composite's way, and the row
/// they land on is scrolled into view, as Tab's would be.
#[test]
fn the_keyboard_keeps_its_row_in_view() {
    let mut core = Core::new();
    open(&mut core, Vec2::new(40.0, 150.0), rows(40));
    assert_eq!(focused(&mut core).as_deref(), Some("Row 0"));
    key(&mut core, EditKey::End);
    assert_eq!(focused(&mut core).as_deref(), Some("Row 39"));
    let (k, r) = panel(&mut core, "Menu");
    assert!(inside(row_rect(&mut core, "Row 39"), r), "End shows it");
    key(&mut core, EditKey::Home);
    assert_eq!(focused(&mut core).as_deref(), Some("Row 0"));
    assert_eq!(core.scroll_offset(k).y, 0.0, "Home back to the top");
    for _ in 0..25 {
        key(&mut core, EditKey::Down);
    }
    assert_eq!(focused(&mut core).as_deref(), Some("Row 25"));
    let r = panel(&mut core, "Menu").1;
    assert!(inside(row_rect(&mut core, "Row 25"), r), "Down follows it");
    key(&mut core, EditKey::Up);
    assert!(inside(row_rect(&mut core, "Row 24"), r));
}

/// Type-ahead lands on a row past the cut and scrolls to it.
#[test]
fn type_ahead_keeps_its_row_in_view() {
    let mut core = Core::new();
    let mut items = rows(30);
    items.push(MenuItem::new("Zoom").id("zoom"));
    open(&mut core, Vec2::new(40.0, 150.0), items);
    core.handle_input(InputEvent::Text("z".into()));
    frame(&mut core);
    assert_eq!(focused(&mut core).as_deref(), Some("Zoom"));
    let r = panel(&mut core, "Menu").1;
    assert!(inside(row_rect(&mut core, "Zoom"), r));
}

/// A submenu taller than the window opens beside its row, cut to the same
/// room and scrolling on its own: the wheel over it leaves the menu it
/// hangs off where it was.
#[test]
fn a_tall_submenu_is_cut_and_scrolls_on_its_own() {
    let mut core = Core::new();
    let items = vec![
        MenuItem::new("Open").id("open"),
        MenuItem::submenu("Move to", rows(40)),
    ];
    open(&mut core, Vec2::new(40.0, 60.0), items);
    let at = row_rect(&mut core, "Move to");
    hover(&mut core, Vec2::new(at.x + at.w / 2.0, at.y + at.h / 2.0));
    frame(&mut core);
    assert_eq!(core.menu_submenus(), &[1]);
    let (outer, menu) = panel(&mut core, "Menu");
    let (sub, r) = panel(&mut core, "Move to");
    assert!(r.x >= menu.x + menu.w - 1.0, "beside its row: {r:?}");
    assert_eq!(r.y, MENU_EDGE, "{r:?}");
    assert!((r.h - (H - 2.0 * MENU_EDGE)).abs() < 0.5, "{r:?}");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(r.x + 20.0, r.y + 60.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -10_000.0)));
    frame(&mut core);
    assert!(core.scroll_offset(sub).y > 0.0, "the submenu scrolled");
    assert_eq!(core.scroll_offset(outer).y, 0.0, "its menu did not");
    assert_eq!(core.menu_submenus(), &[1], "and it is still open");
}

/// The keyboard into a tall submenu and down it: End lands on its last
/// row, in view.
#[test]
fn the_keyboard_walks_a_tall_submenu_in_view() {
    let mut core = Core::new();
    let items = vec![MenuItem::submenu("Move to", rows(40))];
    open(&mut core, Vec2::new(40.0, 60.0), items);
    key(&mut core, EditKey::Right);
    assert_eq!(focused(&mut core).as_deref(), Some("Row 0"));
    key(&mut core, EditKey::End);
    assert_eq!(focused(&mut core).as_deref(), Some("Row 39"));
    let r = panel(&mut core, "Move to").1;
    assert!(inside(row_rect(&mut core, "Row 39"), r));
}

/// The bar's dropped menu is the same panel: one taller than the room
/// below the bar keeps its top under the title and is cut at the room's
/// bottom, rather than sliding up over the bar.
#[test]
fn a_tall_bar_menu_stays_under_its_title() {
    let bar = || MenuBar::new(vec![BarMenu::new("File", rows(40))]);
    let mut core = Core::new();
    frame_with(&mut core, Some(bar()));
    core.set_menu_bar_open(Some(0));
    frame_with(&mut core, Some(bar()));
    let title = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.role == Role::MenuItem && n.name.as_deref() == Some("File"))
        .unwrap()
        .rect;
    let (k, r) = panel(&mut core, "File");
    assert!(r.y >= title.y + title.h, "under the title: {r:?} {title:?}");
    assert!(
        (r.y + r.h - (H - MENU_EDGE)).abs() < 0.5,
        "cut at the room's bottom: {r:?}"
    );
    let g = core.scroll_geometry(k).unwrap();
    assert!(g.content.h > g.rect.h);
}

/// The app's own limit (`Menu::max_size`): a menu held to 120 px is that
/// tall at its point, scrolls, and keeps the keyboard's row in view.
#[test]
fn a_menu_held_to_a_height_is_that_tall_at_its_point_and_scrolls() {
    let mut core = Core::new();
    let card = frame(&mut core);
    core.open_menu(Menu::new(card, Vec2::new(40.0, 30.0), rows(40)).max_size(f32::INFINITY, 120.0));
    frame(&mut core);
    let (k, r) = panel(&mut core, "Menu");
    assert_eq!((r.x, r.y), (40.0, 30.0), "it fits, so it is at the point");
    assert!((r.h - 120.0).abs() < 0.5, "held to the app's height: {r:?}");
    let g = core.scroll_geometry(k).unwrap();
    assert!(g.content.h > g.rect.h, "and scrolls");
    key(&mut core, EditKey::End);
    let r = panel(&mut core, "Menu").1;
    assert!(
        inside(row_rect(&mut core, "Row 39"), r),
        "End shows its row"
    );
}

/// Held to a width, a long row is ellipsized inside it.
#[test]
fn a_menu_held_to_a_width_is_no_wider() {
    let mut core = Core::new();
    let card = frame(&mut core);
    let long = MenuItem::new("A recent file with a long path, under a folder under another");
    core.open_menu(
        Menu::new(card, Vec2::new(10.0, 10.0), vec![long]).max_size(140.0, f32::INFINITY),
    );
    frame(&mut core);
    let r = panel(&mut core, "Menu").1;
    assert!(r.w <= 140.5, "held to the app's width: {r:?}");
}

/// A submenu is held to what its menu is.
#[test]
fn a_submenu_is_held_to_its_menus_size() {
    let mut core = Core::new();
    let card = frame(&mut core);
    let items = vec![MenuItem::submenu("Move to", rows(40))];
    core.open_menu(Menu::new(card, Vec2::new(40.0, 30.0), items).max_size(f32::INFINITY, 150.0));
    frame(&mut core);
    let at = row_rect(&mut core, "Move to");
    hover(&mut core, Vec2::new(at.x + at.w / 2.0, at.y + at.h / 2.0));
    frame(&mut core);
    let r = panel(&mut core, "Move to").1;
    assert!((r.h - 150.0).abs() < 0.5, "{r:?}");
}
