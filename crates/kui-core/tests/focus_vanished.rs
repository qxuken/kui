//! A focus left on a node the frame no longer has (backlog C136): when no
//! composite recovers it, it moves to the nearest key sink it had around
//! it, and is dropped where it had none, so a `keyFocus` shell takes it.
//! Before, every key went nowhere until a click.

use kui_core::testing::{click_at, drive, key_press};
use kui_core::{Core, Key, KeyCode, NodeSpec, Role, Size};

/// An app shell (a key sink) around a page: page `a` has a button, page
/// `b` a different one. With `sink` false there is no shell; with
/// `take` the root declares `keyFocus` while nothing is focused.
fn frame(core: &mut Core, page: &str, sink: bool, take: bool) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut button = Key::ROOT;
    let mut shell = NodeSpec::column().fill();
    if sink {
        shell = shell.on_key("keys");
    }
    let shell = ui.with_keyed("shell", shell, |ui| {
        ui.with_keyed(&format!("page.{page}"), NodeSpec::column().fill(), |ui| {
            button = ui.leaf_keyed(
                page,
                NodeSpec::row()
                    .size(100.0, 30.0)
                    .on_click(page)
                    .label(page.to_string()),
            );
        });
    });
    if take && ui.core().focus().is_none() {
        ui.take_key_focus(shell);
    }
    ui.finish();
    (shell, button)
}

#[test]
fn a_page_changed_under_the_focused_control_gives_the_keyboard_to_the_shell() {
    let mut core = Core::new();
    let (shell, a) = frame(&mut core, "a", true, false);
    click_at(&mut core, 50.0, 15.0);
    assert_eq!(core.focus(), Some(a), "the press focused the button");
    // The page changes: the button is gone.
    frame(&mut core, "b", true, false);
    assert_eq!(
        core.focus(),
        Some(shell),
        "the shell around it has the keyboard"
    );
    // And the shell hears the keys the page no longer could.
    let evs = drive(&mut core, &[key_press(KeyCode::Char('j'))]);
    assert!(evs.iter().any(|e| e.key == shell), "{evs:?}");
}

#[test]
fn with_no_sink_around_it_focus_is_dropped_and_a_key_focus_shell_takes_it() {
    let mut core = Core::new();
    let (_, a) = frame(&mut core, "a", false, false);
    click_at(&mut core, 50.0, 15.0);
    assert_eq!(core.focus(), Some(a));
    frame(&mut core, "b", false, false);
    assert_eq!(core.focus(), None, "nothing to fall back on: dropped");
    // A shell that takes the keyboard while nothing has it gets it on the
    // next frame.
    let (shell, _) = frame(&mut core, "b", true, true);
    assert_eq!(core.focus(), Some(shell));
}

#[test]
fn a_focus_on_a_node_never_built_is_left_alone() {
    let mut core = Core::new();
    frame(&mut core, "a", true, false);
    let later = Key::ROOT.str("shell").str("later");
    core.set_focus(Some(later));
    frame(&mut core, "a", true, false);
    assert_eq!(
        core.focus(),
        Some(later),
        "a view may focus what it builds next"
    );
}

#[test]
fn a_composite_still_recovers_its_own_item() {
    let mut core = Core::new();
    let rows = |core: &mut Core, names: &[&str]| {
        let mut keys = Vec::new();
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("shell", NodeSpec::column().fill().on_key("keys"), |ui| {
            ui.with_keyed(
                "list",
                NodeSpec::column().role(Role::List).label("L"),
                |ui| {
                    for n in names {
                        keys.push(
                            ui.leaf_keyed(
                                n,
                                NodeSpec::row()
                                    .size(100.0, 20.0)
                                    .role(Role::ListItem)
                                    .focusable()
                                    .label(n.to_string()),
                            ),
                        );
                    }
                },
            );
        });
        ui.finish();
        keys
    };
    let keys = rows(&mut core, &["x", "y", "z"]);
    core.set_focus(Some(keys[1]));
    rows(&mut core, &["x", "y", "z"]);
    let keys = rows(&mut core, &["x", "z"]);
    assert_eq!(
        core.focus(),
        Some(keys[1]),
        "the item now at its place, not the shell"
    );
}
