//! Tab focus traversal: cycling between edit widgets in tree order,
//! multiline editors keeping Tab as indentation, and sink precedence.

use kui_core::{Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, Size, Sizing, Value};

const SHIFT: Mods = Mods {
    shift: true,
    word: false,
    doc: false,
};

/// Three single-line fields and one multiline, in order.
fn frame(core: &mut Core, autofocus_first: bool) -> Vec<Key> {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0).gap(8.0));
    let mut keys = Vec::new();
    for (label, multiline) in [("a", false), ("b", false), ("c", false), ("notes", true)] {
        keys.push(ui.text_edit(
            label,
            label,
            &EditOptions {
                multiline,
                autofocus: autofocus_first && label == "a",
                ..Default::default()
            },
            NodeSpec::column().width(Sizing::Grow(1.0)),
        ));
    }
    ui.finish();
    keys
}

fn tab(core: &mut Core, mods: Mods) {
    core.handle_input(InputEvent::Key(EditKey::Tab, mods));
}

#[test]
fn tab_cycles_and_shift_tab_reverses() {
    let mut core = Core::new();
    let keys = frame(&mut core, true);
    assert_eq!(core.edit.focused(), Some(keys[0]));

    tab(&mut core, Mods::default());
    assert_eq!(core.edit.focused(), Some(keys[1]));
    tab(&mut core, Mods::default());
    assert_eq!(core.edit.focused(), Some(keys[2]));
    tab(&mut core, SHIFT);
    assert_eq!(core.edit.focused(), Some(keys[1]));
    // Backwards past the first wraps to the last (the multiline).
    tab(&mut core, SHIFT);
    tab(&mut core, SHIFT);
    assert_eq!(core.edit.focused(), Some(keys[3]));
}

#[test]
fn multiline_keeps_tab_as_indentation() {
    let mut core = Core::new();
    let keys = frame(&mut core, false);
    core.edit.set_focus(Some(keys[3]));
    tab(&mut core, Mods::default());
    assert_eq!(core.edit.focused(), Some(keys[3]), "focus stays");
    assert!(
        core.edit_text(keys[3]).unwrap().contains("    "),
        "tab indented"
    );
}

#[test]
fn tab_with_no_focus_enters_the_ring() {
    let mut core = Core::new();
    let keys = frame(&mut core, false);
    assert_eq!(core.edit.focused(), None);
    tab(&mut core, Mods::default());
    assert_eq!(core.edit.focused(), Some(keys[0]));
    core.edit.set_focus(None);
    tab(&mut core, SHIFT);
    assert_eq!(
        core.edit.focused(),
        Some(keys[3]),
        "shift-tab enters from the end"
    );
}

#[test]
fn key_sink_owns_tab_when_no_edit_is_focused() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with_keyed("app", NodeSpec::column().fill().on_key(Value::Null), |ui| {
        ui.text_edit("field", "x", &EditOptions::default(), NodeSpec::column());
    });
    ui.take_key_focus(sink);
    ui.finish();

    tab(&mut core, Mods::default());
    assert_eq!(core.edit.focused(), None, "the sink keeps the keyboard");
}
