//! A core told it runs on another platform than the build's (backlog
//! F220, `Core::set_platform`): every choice it makes by platform follows,
//! so a picture of a Mac's window taken on Windows is a Mac's — the
//! shortcuts a menu writes, what the primary modifier means to an editor,
//! the titlebar's height.

use kui_core::{
    Core, EditOptions, InputEvent, Key, KeyCode, KeyMods, KeyPress, Menu, MenuItem, MenuRole,
    NodeSpec, Platform, Size, Vec2,
};

/// The menu a core told it runs on `platform` opens, its rows as a host
/// reads them back: an app's own `mod+alt+8` and a Copy row with none.
fn menu_on(platform: Platform) -> Vec<Option<String>> {
    let mut core = Core::new();
    core.set_platform(platform);
    let ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.finish();
    core.open_menu(Menu::new(
        Key::ROOT,
        Vec2::new(10.0, 10.0),
        vec![
            MenuItem::new("Heading").accel("mod+alt+8"),
            MenuItem::role(MenuRole::Copy),
        ],
    ));
    let menu = core.menu().expect("the menu is open");
    menu.items
        .iter()
        .map(|i| i.accel_label_on(core.platform()).map(|l| l.into_owned()))
        .collect()
}

#[test]
fn a_menu_writes_its_shortcuts_as_the_platform_does() {
    assert_eq!(
        menu_on(Platform::MacOs),
        [Some("\u{2325}\u{2318}8".into()), Some("\u{2318}C".into())]
    );
    for p in [Platform::Windows, Platform::Linux] {
        assert_eq!(
            menu_on(p),
            [Some("Ctrl+Alt+8".into()), Some("Ctrl+C".into())],
            "{p:?}"
        );
    }
}

/// One field, focused, holding `hello world` with the caret at its end.
fn field_on(platform: Platform) -> (Core, Key) {
    let mut core = Core::new();
    core.set_platform(platform);
    let mut key = Key::ROOT;
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        key = ui.text_edit(
            "field",
            "",
            &EditOptions {
                autofocus: true,
                ..Default::default()
            },
            NodeSpec::column().size(300.0, 40.0),
        );
        ui.finish();
    }
    assert_eq!(core.focus(), Some(key));
    core.handle_input(InputEvent::Text("hello world".into()));
    (core, key)
}

fn backspace(platform: Platform, mods: KeyMods) -> String {
    let (mut core, key) = field_on(platform);
    core.press(KeyPress::new(KeyCode::Backspace, mods));
    core.edit_text(key).unwrap()
}

/// The primary modifier is the core's platform's, not the build's: ⌘⌫
/// takes a Mac's line back, Ctrl+⌫ a Windows word, and Ctrl on a Mac is
/// not the primary at all.
#[test]
fn an_editing_key_means_what_the_platform_s_modifier_means() {
    let cmd = KeyMods::NONE.with_super();
    let ctrl = KeyMods::NONE.with_ctrl();
    assert_eq!(backspace(Platform::MacOs, cmd), "");
    assert_eq!(backspace(Platform::MacOs, ctrl), "hello worl");
    assert_eq!(backspace(Platform::Windows, ctrl), "hello ");
    assert_eq!(backspace(Platform::Linux, ctrl), "hello ");
}

/// The `titlebar_h` metric is the platform's caption while it is the
/// stock number, and an app's own number stands.
#[test]
fn the_titlebar_metric_is_the_platform_s_caption() {
    let mut core = Core::new();
    core.set_platform(Platform::Windows);
    assert_eq!(core.metrics().titlebar_h, 32.0);
    core.set_platform(Platform::MacOs);
    assert_eq!(core.metrics().titlebar_h, 34.0);
    // A stock set handed in reads the platform's, whichever build wrote it.
    core.set_platform(Platform::Windows);
    core.set_metrics(kui_core::Metrics::compact());
    assert_eq!(core.metrics().titlebar_h, 32.0);
    // A driver's strip wins over the caption, and survives the change.
    core.set_platform_titlebar_h(Some(52.0));
    core.set_platform(Platform::MacOs);
    assert_eq!(core.metrics().titlebar_h, 52.0);
    // And a number of the app's own is the app's.
    let mut core = Core::new();
    core.set_metrics(kui_core::Metrics {
        titlebar_h: 40.0,
        ..Default::default()
    });
    core.set_platform(Platform::Windows);
    assert_eq!(core.metrics().titlebar_h, 40.0);
}
