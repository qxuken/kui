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
    assert_eq!(
        evs[0].payload.get("kind").and_then(Value::as_str),
        Some("menu")
    );
    assert_eq!(
        evs[0].payload.get("item").and_then(Value::as_str),
        Some("light")
    );
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
