//! A menu row that is its chord (backlog F151): chosen, it plays its
//! `accel` where the keyboard was before the menu took it — a field's
//! undo through the field, a shell's shortcut to the shell — and posts no
//! `menu` event.

use kui_core::testing::click;
use kui_core::{
    Core, EditOptions, InputEvent, Key, KeyCode, KeyMods, Menu, MenuItem, NodeSpec, Size, UiEvent,
    Value, Vec2,
};

/// A shell sink around a field, the field focused.
fn frame(core: &mut Core) -> (Key, Key) {
    let mut ui = core.frame(Size::new(600.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut field = Key::ROOT;
    let shell = ui.with_keyed("shell", NodeSpec::column().fill().on_key("shell"), |ui| {
        field = ui.text_edit(
            "field",
            "ab",
            &EditOptions {
                autofocus: true,
                ..Default::default()
            },
            NodeSpec::column().size(200.0, 30.0),
        );
    });
    ui.finish();
    (shell, field)
}

fn rows() -> Vec<MenuItem> {
    vec![
        MenuItem::new("Undo").accel("mod+z").replay(),
        MenuItem::new("New Note").accel("mod+n").replay(),
        MenuItem::new("Start of Line").accel("mod+left").replay(),
        MenuItem::new("Plain").id("plain").accel("mod+p"),
    ]
}

fn row_center(core: &mut Core, label: &str) -> Vec2 {
    let tree = core.access_tree().clone();
    let node = tree
        .nodes
        .iter()
        .find(|n| n.role == kui_core::Role::MenuItem && n.name.as_deref() == Some(label))
        .unwrap_or_else(|| panic!("no menu row named {label:?}"));
    let r = node.rect;
    Vec2::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// Opens the menu over the field and clicks the row reading `label`.
fn choose(core: &mut Core, label: &str) -> Vec<UiEvent> {
    let (_, field) = frame(core);
    core.open_menu(Menu::new(field, Vec2::new(20.0, 60.0), rows()));
    frame(core);
    let at = row_center(core, label);
    let out = click(core, at);
    frame(core);
    out
}

fn menu_events(evs: &[UiEvent]) -> Vec<&UiEvent> {
    evs.iter().filter(|e| e.kind() == Some("menu")).collect()
}

fn rig() -> (Core, Key, Key) {
    let mut core = Core::new();
    let (shell, field) = frame(&mut core);
    frame(&mut core);
    core.handle_input(InputEvent::Text("c".into()));
    assert_eq!(core.edit_text(field).as_deref(), Some("abc"));
    (core, shell, field)
}

#[test]
fn a_replayed_undo_undoes_the_field_the_menu_was_opened_from() {
    let (mut core, _, field) = rig();
    let evs = choose(&mut core, "Undo");
    assert_eq!(core.edit_text(field).as_deref(), Some("ab"));
    assert!(menu_events(&evs).is_empty(), "no menu event: {evs:?}");
    assert_eq!(
        core.focus(),
        Some(field),
        "the keyboard is back in the field"
    );
}

#[test]
fn a_replayed_chord_the_field_does_not_take_reaches_the_shell() {
    let (mut core, shell, field) = rig();
    let evs = choose(&mut core, "New Note");
    let keys: Vec<_> = evs
        .iter()
        .filter(|e| e.kind() == Some("key") && e.key == shell)
        .map(|e| (e.payload.get_str("phase"), e.payload.get_str("code")))
        .collect();
    assert_eq!(keys, [(Some("down"), Some("n"))]);
    assert!(menu_events(&evs).is_empty());
    assert_eq!(core.edit_text(field).as_deref(), Some("abc"));
}

#[test]
fn a_replayed_editing_key_moves_the_caret_as_the_key_does() {
    let (mut core, _, field) = rig();
    choose(&mut core, "Start of Line");
    core.handle_input(InputEvent::Text(">".into()));
    assert_eq!(core.edit_text(field).as_deref(), Some(">abc"));
}

#[test]
fn a_row_that_is_not_its_chord_still_posts() {
    let (mut core, _, field) = rig();
    let evs = choose(&mut core, "Plain");
    let menu = menu_events(&evs);
    assert_eq!(menu.len(), 1);
    assert_eq!(menu[0].payload.get("item"), Some(&Value::str("plain")));
    assert_eq!(core.edit_text(field).as_deref(), Some("abc"));
}

#[test]
fn a_row_reads_replay_from_plain_data_and_plays_the_keyboards_press() {
    let row = MenuItem::from_value(&Value::map([
        ("label", Value::str("Redo")),
        ("accel", Value::str("mod+shift+z")),
        ("replay", Value::Bool(true)),
    ]))
    .unwrap();
    let kp = row.replayed().expect("a chord");
    // As the keyboard sends it: the letter in Shift's case, the key
    // itself the letter.
    assert_eq!(kp.code, KeyCode::Char('Z'));
    assert_eq!(kp.physical, KeyCode::Char('z'));
    assert!(kp.mods.shift && kp.mods.primary());
    // Not `replay`, or an accel kui cannot read: an ordinary row.
    assert!(MenuItem::new("x").accel("mod+z").replayed().is_none());
    assert!(
        MenuItem::new("x")
            .accel("hyper+q")
            .replay()
            .replayed()
            .is_none()
    );
    let _ = KeyMods::default();
}
