//! The application menu bar
//! (`docs/adr/0018-a-menu-bar-the-app-declares.md`): what a frame
//! declares, what the drawn bar does with it, and what an app hears from
//! either path.

use kui_core::testing::click;
use kui_core::{
    BarMenu, Core, EditOptions, InputEvent, Key, MenuBar, MenuItem, MenuRole, NodeSpec, Role, Size,
    TextStyle, Value, Vec2,
};

fn bar() -> MenuBar {
    MenuBar::new(vec![
        BarMenu::new(
            "File",
            vec![
                MenuItem::new("New")
                    .id(Value::str("file.new"))
                    .accel("mod+n"),
                MenuItem::separator(),
                MenuItem::new("Close").id(Value::str("file.close")),
            ],
        ),
        BarMenu::new(
            "Edit",
            vec![
                MenuItem::role(MenuRole::Copy),
                MenuItem::new("Find").id(Value::str("edit.find")),
            ],
        ),
        // A menu with nothing in it opens nothing, the way a disabled one
        // does — both are in here so the tests below can say so.
        BarMenu::new("Empty", Vec::new()),
    ])
}

/// A frame that declares the bar and draws it above a card of text.
fn frame(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    kui_core::widgets::menu_bar(&mut ui, bar());
    let card = ui.text_in_keyed(
        "card",
        NodeSpec::column().grow_width().selectable(),
        "one two",
        TextStyle::new(14.0),
    );
    ui.finish();
    card
}

/// The middle of the node the access tree reports under `name` with
/// `role` — the laid-out rect, read where one is readable without a query.
fn center(core: &mut Core, role: Role, name: &str) -> Vec2 {
    let tree = core.access_tree().clone();
    let node = tree
        .nodes
        .iter()
        .find(|n| n.role == role && n.name.as_deref() == Some(name))
        .unwrap_or_else(|| panic!("no {role:?} named {name:?}"));
    let r = node.rect;
    Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

fn named(core: &mut Core, role: Role, name: &str) -> bool {
    let tree = core.access_tree().clone();
    tree.nodes
        .iter()
        .any(|n| n.role == role && n.name.as_deref() == Some(name))
}

#[test]
fn a_declaration_is_sticky_and_diffed() {
    let mut core = Core::new();
    assert!(core.menu_bar().is_none(), "nobody has declared one");
    frame(&mut core);
    let rev = core.menu_bar_revision();
    assert!(rev > 0);
    assert_eq!(core.menu_bar().map(|b| b.menus.len()), Some(3));
    // The same declaration again is not a change: a driver that diffs the
    // revision touches the platform once, not once a frame.
    frame(&mut core);
    assert_eq!(core.menu_bar_revision(), rev);
    // A frame that never calls the widget declares nothing, and leaves the
    // last declaration standing.
    let ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.finish();
    assert_eq!(core.menu_bar().map(|b| b.menus.len()), Some(3));
    // An empty bar is how it is taken away, and that *is* a change.
    core.declare_menu_bar(MenuBar::default());
    assert!(core.menu_bar().expect("declared").is_empty());
    assert!(core.menu_bar_revision() > rev);
}

#[test]
fn an_accelerator_is_normalized_into_the_platforms_spelling() {
    let mut core = Core::new();
    frame(&mut core);
    let accel = core.menu_bar().unwrap().item(0, 0).unwrap().accel.clone();
    let expected = if cfg!(target_os = "macos") {
        "\u{2318}N"
    } else {
        "Ctrl+N"
    };
    assert_eq!(accel.as_deref(), Some(expected));
}

#[test]
fn the_titles_are_drawn_and_no_menu_is_open() {
    let mut core = Core::new();
    frame(&mut core);
    assert!(named(&mut core, Role::MenuItem, "File"));
    assert!(named(&mut core, Role::MenuItem, "Edit"));
    assert_eq!(core.menu_bar_open(), None);
    assert!(
        !named(&mut core, Role::MenuItem, "New"),
        "nothing has dropped down yet"
    );
}

#[test]
fn a_press_on_a_title_opens_its_menu_and_a_second_closes_it() {
    let mut core = Core::new();
    frame(&mut core);
    let file = center(&mut core, Role::MenuItem, "File");
    let events = click(&mut core, file);
    assert!(
        events.is_empty(),
        "the bar's own click never reaches the app: {events:?}"
    );
    assert_eq!(core.menu_bar_open(), Some(0));
    // State, not paint: the frame after it draws the rows.
    frame(&mut core);
    assert!(named(&mut core, Role::MenuItem, "New"));
    click(&mut core, file);
    assert_eq!(core.menu_bar_open(), None, "the title is a toggle");
}

#[test]
fn a_menu_with_nothing_in_it_opens_nothing() {
    let mut core = Core::new();
    frame(&mut core);
    let empty = center(&mut core, Role::MenuItem, "Empty");
    click(&mut core, empty);
    assert_eq!(core.menu_bar_open(), None);
}

#[test]
fn hovering_another_title_while_open_moves_the_open_menu() {
    let mut core = Core::new();
    frame(&mut core);
    let file = center(&mut core, Role::MenuItem, "File");
    click(&mut core, file);
    frame(&mut core);
    let edit = center(&mut core, Role::MenuItem, "Edit");
    core.handle_input(InputEvent::CursorMoved(edit));
    // The frame that notices the hover is the frame that draws the new
    // menu — the widget resolves it before it builds anything.
    frame(&mut core);
    assert_eq!(core.menu_bar_open(), Some(1));
    assert!(named(&mut core, Role::MenuItem, "Find"));
    assert!(!named(&mut core, Role::MenuItem, "New"));
}

#[test]
fn choosing_a_row_posts_the_items_payload_and_closes() {
    let mut core = Core::new();
    frame(&mut core);
    let file = center(&mut core, Role::MenuItem, "File");
    click(&mut core, file);
    frame(&mut core);
    let row = center(&mut core, Role::MenuItem, "Close");
    let events = click(&mut core, row);
    assert_eq!(
        events.len(),
        1,
        "one event, not a raw click too: {events:?}"
    );
    let ev = &events[0];
    assert_eq!(ev.kind(), Some("menu"));
    assert_eq!(ev.payload.get_str("item"), Some("file.close"));
    assert_eq!(core.menu_bar_open(), None, "choosing closes the menu");
}

#[test]
fn escape_closes_the_open_menu_and_the_app_hears_nothing() {
    let mut core = Core::new();
    frame(&mut core);
    let file = center(&mut core, Role::MenuItem, "File");
    click(&mut core, file);
    frame(&mut core);
    let events = core.press(kui_core::KeyPress::new(
        kui_core::KeyCode::Escape,
        kui_core::KeyMods::default(),
    ));
    assert_eq!(core.menu_bar_open(), None);
    assert!(
        events.iter().all(|e| e.kind() != Some("dismiss")),
        "the bar's own dismissal is the bar's: {events:?}"
    );
}

#[test]
fn a_press_in_the_app_below_closes_the_menu_and_is_swallowed() {
    let mut core = Core::new();
    frame(&mut core);
    let file = center(&mut core, Role::MenuItem, "File");
    click(&mut core, file);
    frame(&mut core);
    // Well below the bar: the app's own content, inert while a menu is up.
    let events = click(&mut core, Vec2::new(200.0, 250.0));
    assert_eq!(core.menu_bar_open(), None);
    assert!(events.is_empty(), "{events:?}");
}

#[test]
fn an_edit_menus_copy_acts_on_the_selection_the_press_did_not_clear() {
    let mut core = Core::new();
    let card = frame(&mut core);
    core.select_all_in(card);
    let edit = center(&mut core, Role::MenuItem, "Edit");
    click(&mut core, edit);
    frame(&mut core);
    let copy = center(&mut core, Role::MenuItem, "Copy");
    let events = click(&mut core, copy);
    assert_eq!(
        core.take_menu_actions(),
        vec![kui_core::MenuAction::SetClipboard {
            text: "one two".into(),
            html: Some("one two".into()),
        }],
        "the standard role is performed exactly as the context menu's is"
    );
    assert_eq!(events[0].payload.get_str("role"), Some("copy"));
}

#[test]
fn the_platforms_bar_reports_a_choice_through_one_call() {
    let mut core = Core::new();
    core.set_native_menu_bar(true);
    frame(&mut core);
    assert!(
        !named(&mut core, Role::MenuItem, "File"),
        "nothing is drawn where the platform owns the bar"
    );
    let events = core.activate_menu_bar_item(0, 2);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload.get_str("item"), Some("file.close"));
    assert_eq!(
        events[0].key,
        Key::ROOT,
        "no node drew it, so it is the root's"
    );
    assert!(
        core.activate_menu_bar_item(9, 9).is_empty(),
        "a row this build does not have does nothing"
    );
}

/// The same bar with a field under it, for the test about the editor a menu
/// is *about*: a menu bar's Copy has to reach the field the user was in,
/// and the press that opens the menu is what moves focus off it.
fn frame_with_field(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    kui_core::widgets::menu_bar(&mut ui, bar());
    ui.text_edit(
        "note",
        "hello",
        &EditOptions::default(),
        NodeSpec::column().width(200.0).label("Note"),
    );
    ui.finish();
    core.key_of("note").expect("the editor is keyed")
}

#[test]
fn the_bars_copy_acts_on_the_field_the_press_took_focus_from() {
    let mut core = Core::new();
    let key = frame_with_field(&mut core);
    core.set_focus(Some(key));
    core.handle_input(InputEvent::Key(
        kui_core::EditKey::SelectAll,
        kui_core::Mods::default(),
    ));
    frame_with_field(&mut core);
    assert_eq!(core.edit.copy_selection(key).as_deref(), Some("hello"));

    // The press moves focus onto the title, so the editor has to have been
    // remembered before it did.
    let edit = center(&mut core, Role::MenuItem, "Edit");
    click(&mut core, edit);
    frame_with_field(&mut core);
    let copy = center(&mut core, Role::MenuItem, "Copy");
    click(&mut core, copy);
    assert_eq!(
        core.take_menu_actions(),
        vec![kui_core::MenuAction::SetClipboard {
            text: "hello".into(),
            html: None,
        }],
        "Copy takes the field's text, not the window selection"
    );
}

#[test]
fn a_different_bar_of_the_same_length_closes_the_open_menu() {
    let mut core = Core::new();
    frame(&mut core);
    let file = center(&mut core, Role::MenuItem, "File");
    click(&mut core, file);
    assert_eq!(core.menu_bar_open(), Some(0));
    // Same count, different menus: the open index would otherwise name a
    // menu the user never opened, and the row keys are index-derived.
    core.declare_menu_bar(MenuBar::new(vec![
        BarMenu::new("Project", vec![MenuItem::new("Build").id(Value::str("b"))]),
        BarMenu::new("Tools", vec![MenuItem::new("Options").id(Value::str("o"))]),
        BarMenu::new("Empty", Vec::new()),
    ]));
    assert_eq!(core.menu_bar_open(), None);
    // A menu whose *items* change under it stays open, which is what a row
    // enabling or a setting checking does.
    click(&mut core, file);
    assert_eq!(core.menu_bar_open(), Some(0));
    core.declare_menu_bar(MenuBar::new(vec![
        BarMenu::new(
            "Project",
            vec![MenuItem::new("Build").id(Value::str("b")).enabled(false)],
        ),
        BarMenu::new("Tools", vec![MenuItem::new("Options").id(Value::str("o"))]),
        BarMenu::new("Empty", Vec::new()),
    ]));
    assert_eq!(core.menu_bar_open(), Some(0));
}
