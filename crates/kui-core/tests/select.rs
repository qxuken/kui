//! The stock select (`widgets::select`): a field that shows the choice in
//! force and, clicked, opens the core's own menu of the choices under it.
//! The app holds no open state and hears one event: the choice, on the
//! field's key.

use kui_core::testing::click;
use kui_core::{Core, InputEvent, Key, NodeSpec, Role, Size, Value, Vec2, widgets};

const OPTIONS: [&str; 3] = ["app", "light", "dark"];

/// The field under a header row, so its rect is not at the origin.
fn frame(core: &mut Core, current: Option<usize>) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(20.0));
    ui.text("theme", kui_core::TextStyle::new(14.0));
    let key = widgets::select(&mut ui, "base", &OPTIONS, current);
    ui.finish();
    key
}

fn rect_of(core: &mut Core, role: Role, name: &str) -> Option<kui_core::Rect> {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .map(|n| n.rect)
}

fn field_node(core: &mut Core) -> kui_core::AccessNode {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.name.as_deref() == Some("base"))
        .cloned()
        .unwrap()
}

fn center(r: kui_core::Rect) -> Vec2 {
    Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

#[test]
fn a_click_opens_the_menu_under_the_field_and_a_row_posts_the_choice_on_it() {
    let mut core = Core::new();
    let key = frame(&mut core, Some(0));
    let field = rect_of(&mut core, Role::Button, "base").expect("the field reads as a button");
    let node = field_node(&mut core);
    assert_eq!(
        node.description.as_deref(),
        Some("app"),
        "described by the choice in force"
    );
    assert_eq!(node.expanded, Some(false));

    // The click is nobody's: the app hears nothing, and the menu is open.
    let evs = click(&mut core, center(field));
    assert!(
        evs.is_empty(),
        "the field's click never leaves the core: {evs:?}"
    );
    let menu = core.menu().expect("the menu opened");
    assert_eq!(menu.target, key, "about the field");
    assert_eq!(menu.at, Vec2::new(field.x, field.y + field.h), "under it");
    assert_eq!(menu.items.len(), 3);
    assert!(
        menu.items[0].checked && !menu.items[1].checked,
        "the current one checked"
    );

    // The next frame draws it, and the field reads as expanded.
    frame(&mut core, Some(0));
    let node = field_node(&mut core);
    assert_eq!(node.expanded, Some(true));
    let row = rect_of(&mut core, Role::MenuItem, "light").expect("the rows are drawn");
    assert!(row.y >= field.y + field.h, "below the field");

    // A row chosen: one event, on the field, naming the option.
    let evs = click(&mut core, center(row));
    assert_eq!(evs.len(), 1, "{evs:?}");
    assert_eq!(evs[0].key, key);
    assert_eq!(evs[0].kind(), Some("menu"));
    assert_eq!(evs[0].payload.get_str("item"), Some("light"));
    assert!(core.menu().is_none(), "and the menu is closed");

    // The view draws the new choice; nothing else is retained.
    frame(&mut core, Some(1));
    let node = field_node(&mut core);
    assert_eq!(node.description.as_deref(), Some("light"));
}

#[test]
fn escape_closes_it_and_the_app_hears_nothing() {
    let mut core = Core::new();
    frame(&mut core, None);
    let field = rect_of(&mut core, Role::Button, "base").unwrap();
    click(&mut core, center(field));
    frame(&mut core, None);
    assert!(core.menu().is_some());
    let evs = core.handle_input(InputEvent::Key(
        kui_core::EditKey::Escape,
        kui_core::Mods::default(),
    ));
    assert!(evs.is_empty(), "{evs:?}");
    assert!(core.menu().is_none());
}

#[test]
fn a_field_the_view_stopped_drawing_opens_nothing() {
    let mut core = Core::new();
    frame(&mut core, Some(2));
    let field = rect_of(&mut core, Role::Button, "base").unwrap();
    // A frame without the field: the click lands on nothing.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.finish();
    let evs = click(&mut core, center(field));
    assert!(evs.is_empty());
    assert!(core.menu().is_none());
}

/// A `current` that names no option — past the end, or a separator — is
/// none, with one `select-current-ignored` warning on the field (backlog
/// RG10): described by nothing, no row checked, where the separator used
/// to carry the check and grow every row a gutter for a mark never drawn.
#[test]
fn a_current_that_names_no_option_is_none_and_warned_once() {
    use kui_core::{MenuItem, diag};
    let items = vec![
        MenuItem::new("app"),
        MenuItem::separator(),
        MenuItem::new("dark"),
    ];
    let mut core = Core::new();
    let frame = |core: &mut Core, current: Option<usize>| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill().pad(20.0));
        let key = widgets::select_items(&mut ui, "base", &items, current);
        ui.finish();
        key
    };
    // Past the end.
    let key = frame(&mut core, Some(9));
    assert_eq!(field_node(&mut core).description.as_deref(), Some(""));
    let warned = core.take_warnings();
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert_eq!(warned[0].code, diag::SELECT_CURRENT_IGNORED);
    assert_eq!(warned[0].key, key, "on the field");
    assert!(
        warned[0].message.contains("names option 9 counted from 0")
            && warned[0].message.contains("the field has 3 options"),
        "{}",
        warned[0].message
    );
    let field = rect_of(&mut core, Role::Button, "base").unwrap();
    click(&mut core, center(field));
    let checked: Vec<bool> = core
        .menu()
        .unwrap()
        .items
        .iter()
        .map(|i| i.checked)
        .collect();
    assert_eq!(checked, [false, false, false], "nothing in force");
    core.close_menu();
    // Every frame drawn that way costs the one line.
    frame(&mut core, Some(9));
    assert!(core.take_warnings().is_empty());
    // On a separator: the same, and the divider is not checked.
    let mut core = Core::new();
    frame(&mut core, Some(1));
    assert_eq!(field_node(&mut core).description.as_deref(), Some(""));
    let warned = core.take_warnings();
    assert_eq!(warned.len(), 1, "{warned:?}");
    assert_eq!(warned[0].code, diag::SELECT_CURRENT_IGNORED);
    assert!(
        warned[0]
            .message
            .contains("option 1 counted from 0, which is a separator"),
        "{}",
        warned[0].message
    );
    let field = rect_of(&mut core, Role::Button, "base").unwrap();
    click(&mut core, center(field));
    let checked: Vec<bool> = core
        .menu()
        .unwrap()
        .items
        .iter()
        .map(|i| i.checked)
        .collect();
    assert_eq!(checked, [false, false, false]);
    // An option in force warns nothing; a disabled one is still in force.
    let mut core = Core::new();
    frame(&mut core, Some(2));
    assert_eq!(field_node(&mut core).description.as_deref(), Some("dark"));
    assert!(core.take_warnings().is_empty());
}

/// `activate_menu_item` on a select's disabled option is refused: false
/// at every door, nothing posted, the menu still open — the pointer never
/// reaches the row, and neither should a host's report (backlog RG9).
#[test]
fn a_disabled_option_cannot_be_chosen_through_the_door() {
    use kui_core::MenuItem;
    let items = vec![
        MenuItem::new("English"),
        MenuItem::new("Latin").id("la").enabled(false),
    ];
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(20.0));
    let key = widgets::select_items(&mut ui, "base", &items, Some(0));
    ui.finish();
    let field = rect_of(&mut core, Role::Button, "base").unwrap();
    click(&mut core, center(field));
    assert!(core.menu().is_some());
    assert_eq!(core.activate_menu_item(1), None, "refused");
    assert!(core.menu().is_some(), "and the menu stays open");
    let evs = core
        .activate_menu_item(0)
        .expect("the enabled one is taken");
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, key);
    assert_eq!(evs[0].payload.get_str("item"), Some("English"));
    assert!(core.menu().is_none());
}

/// The one shared reader refuses an empty list (backlog RG10): a select
/// with nothing to choose from is a field opening a menu of no rows.
#[test]
fn the_options_reader_refuses_an_empty_list() {
    use kui_core::MenuItem;
    let err = MenuItem::options_from_value(&Value::List(Vec::new())).unwrap_err();
    assert!(err.contains("at least one option"), "{err}");
    let one = MenuItem::options_from_value(&Value::List(vec![Value::str("a")])).unwrap();
    assert_eq!(one.len(), 1);
    assert_eq!(
        MenuItem::KEYS.to_vec(),
        [
            "label", "role", "enabled", "checked", "id", "accel", "items", "replay"
        ],
        "the keys `from_value` reads"
    );
}

/// An option is chosen, never opened: one handed over with a submenu — a
/// C `KuiMenuItem`'s `submenu`, a data option's `items` — opens a menu of
/// plain rows, so `current` names a row of that one menu; and the keys
/// no row reads are found inside a submenu as well as outside it, with
/// an option's `items` among them (backlog RG150).
#[test]
fn an_option_opens_no_submenu_and_stray_keys_are_found_inside_one() {
    use kui_core::MenuItem;
    let items = vec![
        MenuItem::new("Name"),
        MenuItem::submenu("Date", vec![MenuItem::new("Newest")]),
    ];
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(20.0));
    widgets::select_items(&mut ui, "sort", &items, Some(1));
    ui.finish();
    let field = rect_of(&mut core, Role::Button, "sort").unwrap();
    click(&mut core, center(field));
    let menu = core.menu().expect("open");
    assert!(
        menu.items.iter().all(|i| !i.has_submenu()),
        "{:?}",
        menu.items
    );
    assert!(menu.items[1].checked);

    let nested = Value::List(vec![Value::map([
        ("label", Value::str("Sort by")),
        (
            "items",
            Value::List(vec![Value::map([
                ("label", Value::str("Name")),
                ("disabled", Value::Bool(true)),
            ])]),
        ),
    ])]);
    assert_eq!(MenuItem::stray_keys(&nested), ["disabled"]);
    assert_eq!(MenuItem::stray_option_keys(&nested), ["items"]);
    let options = MenuItem::options_from_value(&nested).unwrap();
    assert!(!options[0].has_submenu(), "the reader leaves them out too");
}
