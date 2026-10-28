//! A list's type-ahead starts at a letter or a digit (backlog F259):
//! punctuation pressed on a focused row bubbles to a sink above, where an
//! app's `?` or `[` is a key of its own, and a search already under way
//! still takes it.

use kui_core::testing::{key_down, tab};
use kui_core::{Core, EventKind, InputEvent, KeyCode, KeyMods, NodeSpec, Role, Size};

fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().on_key("shell"));
    ui.with_keyed(
        "list",
        NodeSpec::column().role(Role::List).label("Rows"),
        |ui| {
            for (i, name) in ["Alpha", "Beta", "Gamma"].iter().enumerate() {
                ui.leaf_keyed(
                    &format!("r{i}"),
                    NodeSpec::row()
                        .size(200.0, 20.0)
                        .role(Role::ListItem)
                        .focusable()
                        .on_click(format!("pick{i}"))
                        .label(*name),
                );
            }
        },
    );
    ui.finish();
}

/// A character typed: its press, then its text; whether the shell heard it.
fn typed(core: &mut Core, c: char) -> bool {
    let mut out = key_down(core, KeyCode::Char(c), KeyMods::default());
    out.extend(core.handle_input(InputEvent::Text(c.to_string())));
    frame(core);
    out.iter().any(|e| e.event == EventKind::Key)
}

fn focused(core: &mut Core) -> Option<String> {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.focused)
        .and_then(|n| n.name.clone())
}

#[test]
fn punctuation_on_a_focused_row_bubbles_and_a_letter_searches() {
    let mut core = Core::new();
    core.set_time(1.0);
    frame(&mut core);
    tab(&mut core, false);
    frame(&mut core);
    assert_eq!(focused(&mut core).as_deref(), Some("Alpha"));
    assert!(typed(&mut core, '?'), "the shell hears `?`");
    assert_eq!(
        focused(&mut core).as_deref(),
        Some("Alpha"),
        "and the list did not move"
    );
    assert!(!typed(&mut core, 'g'), "a letter is the list's search");
    assert_eq!(focused(&mut core).as_deref(), Some("Gamma"));
}
