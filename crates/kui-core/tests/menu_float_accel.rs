//! An accelerator closes an app's open menu float (backlog C109): a modal
//! node whose role is `menu` — Kit's select, a popover of rows — is asked
//! to go (`dismiss: {reason: "accelerator"}`) and the press is the
//! window's, focus back where the float took it from, as a native menu lets
//! a key equivalent that is not its own through and as the core's own menus
//! do (C98). An editor inside the float keeps its chords, and a modal that
//! is not a menu stays.

use kui_core::{
    Core, EditOptions, EventKind, Key, KeyCode, KeyMods, NodeSpec, Role, Size, UiEvent, Value,
    testing,
};

const VIEW: Size = Size { w: 400.0, h: 300.0 };

/// A field, and — while `open` — a modal float of `role` holding two
/// menu items (and, with `filter`, an editor first).
fn frame(core: &mut Core, open: bool, role: Role, filter: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.text_edit(
        "field",
        "hello",
        &EditOptions::default(),
        NodeSpec::row().size(200.0, 24.0).label("Field"),
    );
    if open {
        ui.with_keyed(
            "menu",
            NodeSpec::column()
                .size(160.0, 80.0)
                .float(kui_core::FloatConfig::viewport().offset(10.0, 40.0))
                .modal(Value::str("menu.dismiss"))
                .role(role),
            |ui| {
                if filter {
                    ui.text_edit(
                        "filter",
                        "",
                        &EditOptions::default(),
                        NodeSpec::row().size(140.0, 20.0).label("Filter"),
                    );
                }
                for (i, name) in ["One", "Two"].into_iter().enumerate() {
                    ui.leaf_keyed(
                        &format!("o{i}"),
                        NodeSpec::row()
                            .size(140.0, 20.0)
                            .role(Role::MenuItem)
                            .label(name)
                            .initial_focus()
                            .on_click(Value::str(name)),
                    );
                }
            },
        );
    }
    ui.finish();
}

fn field() -> Key {
    Key::ROOT.str("field")
}

fn dismissed(out: &[UiEvent]) -> Option<String> {
    out.iter()
        .find(|e| e.event == EventKind::Dismiss)
        .and_then(|e| e.data()?.get_str("reason").map(str::to_string))
}

/// The field focused, then the float opened over it, focus inside.
fn opened(role: Role, filter: bool) -> Core {
    let mut core = Core::new();
    frame(&mut core, false, role, filter);
    core.set_focus(Some(field()));
    frame(&mut core, false, role, filter);
    frame(&mut core, true, role, filter);
    frame(&mut core, true, role, filter);
    core
}

#[test]
fn an_accelerator_closes_the_menu_and_goes_to_the_field() {
    let mut core = opened(Role::Menu, false);
    assert_ne!(core.focus(), Some(field()), "the float took focus");
    let out = testing::key_down(&mut core, KeyCode::Char('a'), KeyMods::NONE.with_primary());
    assert_eq!(dismissed(&out).as_deref(), Some("accelerator"), "{out:?}");
    assert_eq!(core.focus(), Some(field()), "focus back on the field");
    // What a runner does with ⌘A after the press (`Shell::edit_chord`)
    // lands in the field.
    core.handle_input(kui_core::InputEvent::Key(
        kui_core::EditKey::SelectAll,
        kui_core::Mods::default(),
    ));
    assert_eq!(core.edit.copy_selection(field()).as_deref(), Some("hello"));
    // The app stops drawing it, and the focus stays where it went.
    frame(&mut core, false, Role::Menu, false);
    assert_eq!(core.focus(), Some(field()));
}

#[test]
fn a_plain_key_does_not_close_the_menu() {
    let mut core = opened(Role::Menu, false);
    let out = testing::key_down(&mut core, KeyCode::Char('a'), KeyMods::NONE);
    assert_eq!(dismissed(&out), None);
    let out = testing::key_down(&mut core, KeyCode::Shift, KeyMods::NONE.with_primary());
    assert_eq!(
        dismissed(&out),
        None,
        "a modifier on its own is no accelerator"
    );
}

#[test]
fn an_editor_inside_the_menu_keeps_its_chords() {
    let mut core = opened(Role::Menu, true);
    core.set_focus(Some(Key::ROOT.str("menu").str("filter")));
    frame(&mut core, true, Role::Menu, true);
    let out = testing::key_down(&mut core, KeyCode::Char('a'), KeyMods::NONE.with_primary());
    assert_eq!(dismissed(&out), None);
}

#[test]
fn a_modal_that_is_no_menu_stays() {
    let mut core = opened(Role::Dialog, false);
    let out = testing::key_down(&mut core, KeyCode::Char('c'), KeyMods::NONE.with_primary());
    assert_eq!(dismissed(&out), None);
}

#[test]
fn a_platform_bar_row_closes_the_menu_first() {
    let mut core = opened(Role::Menu, false);
    core.declare_menu_bar(kui_core::MenuBar::new(vec![kui_core::BarMenu::new(
        "Edit",
        vec![kui_core::MenuItem::new("Do").id("do")],
    )]));
    let out = core.activate_menu_bar_item(0, 0);
    assert_eq!(dismissed(&out).as_deref(), Some("accelerator"), "{out:?}");
    assert_eq!(core.focus(), Some(field()));
}
