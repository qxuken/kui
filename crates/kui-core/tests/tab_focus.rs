//! Tab focus traversal: cycling between edit widgets in tree order,
//! multiline editors keeping Tab as indentation, sink precedence, and the
//! ring confined to a modal subtree
//! (`docs/adr/0003-modal-surfaces.md`).

use kui_core::{
    Align, Core, EditKey, EditOptions, FloatConfig, InputEvent, Key, Mods, NodeSpec, Size, Sizing,
    Value,
};

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
    core.set_focus(Some(keys[3]));
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
    core.set_focus(None);
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

// -- Modal scope (docs/adr/0003-modal-surfaces.md) --------------------------

/// Two background buttons and, when `dialog` says so, a floated modal
/// holding two more — plus a confirm nested inside it when `confirm` does.
/// Rows are 100x20: the background at the top left, the modal at the
/// bottom right of a 200x300 viewport.
fn modal_frame(core: &mut Core, dialog: bool, confirm: bool) -> Vec<Key> {
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = || {
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(20.0))
    };
    let mut keys = vec![
        ui.with_keyed("open", row().on_click(Value::str("open")), |_| {}),
        ui.with_keyed("other", row().on_click(Value::str("other")), |_| {}),
    ];
    if dialog {
        let dlg = ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(40.0))
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End),
                )
                .modal(Value::str("dlg"))
                .label("Settings"),
            |ui| {
                ui.with_keyed("ok", row().on_click(Value::str("ok")), |_| {});
                ui.with_keyed("cancel", row().on_click(Value::str("cancel")), |_| {});
                if confirm {
                    ui.with_keyed(
                        "confirm",
                        NodeSpec::column()
                            .width(Sizing::Fixed(100.0))
                            .height(Sizing::Fixed(20.0))
                            .float(FloatConfig::viewport())
                            .modal(Value::Null)
                            .label("Sure?"),
                        |ui| {
                            ui.with_keyed("yes", row().on_click(Value::str("yes")), |_| {});
                        },
                    );
                }
            },
        );
        keys.push(dlg);
        keys.push(Key::ROOT.str("dialog").str("ok"));
        keys.push(Key::ROOT.str("dialog").str("cancel"));
    }
    ui.finish();
    keys
}

#[test]
fn tab_stays_inside_the_modal() {
    let mut core = Core::new();
    let k = modal_frame(&mut core, false, false);
    core.set_focus(Some(k[0]));

    // The modal appears: focus enters it, without showing a ring — the app
    // opened the dialog, nobody pressed a key.
    let k = modal_frame(&mut core, true, false);
    let (open, ok, cancel) = (k[0], k[3], k[4]);
    assert_eq!(core.focus(), Some(ok), "focus enters the modal");
    assert!(!core.focus_visible());

    // The ring is the modal's subtree, and it wraps inside it.
    tab(&mut core, Mods::default());
    assert_eq!(core.focus(), Some(cancel));
    tab(&mut core, Mods::default());
    assert_eq!(core.focus(), Some(ok), "the ring wraps inside the modal");
    tab(&mut core, SHIFT);
    assert_eq!(core.focus(), Some(cancel), "backwards wraps too");
    assert!(core.focus_visible(), "a Tab press shows");

    // Nothing behind it is reachable, from either end or by hand: focus
    // put outside is pulled back in on the next frame.
    core.set_focus(Some(open));
    modal_frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(ok));
}

#[test]
fn the_innermost_modal_is_the_one_in_effect() {
    let mut core = Core::new();
    modal_frame(&mut core, true, false);
    let yes = Key::ROOT.str("dialog").str("confirm").str("yes");
    let k = modal_frame(&mut core, true, true);
    let (ok, cancel) = (k[3], k[4]);
    assert_eq!(core.modal(), Some(Key::ROOT.str("dialog").str("confirm")));
    assert_eq!(core.focus(), Some(yes), "the confirm takes focus");
    tab(&mut core, Mods::default());
    assert_eq!(
        core.focus(),
        Some(yes),
        "a one-stop ring: the dialog under the confirm is out of it"
    );

    // The confirm goes away: the dialog is the modal again, and focus is
    // back where the confirm found it.
    let k2 = modal_frame(&mut core, true, false);
    assert_eq!(core.modal(), Some(k2[2]));
    assert_eq!(core.focus(), Some(ok));
    tab(&mut core, Mods::default());
    assert_eq!(core.focus(), Some(cancel), "the dialog's ring is back");
}

#[test]
fn focus_returns_where_the_modal_found_it() {
    let mut core = Core::new();
    let k = modal_frame(&mut core, false, false);
    let (open, other) = (k[0], k[1]);
    core.set_focus(Some(open));

    let k = modal_frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k[3]));
    // Declared again: it is the same modal, so the saved focus stands.
    modal_frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k[3]));

    // Gone: focus goes back to the button that opened it, and the whole
    // ring is reachable again.
    modal_frame(&mut core, false, false);
    assert_eq!(core.focus(), Some(open));
    tab(&mut core, Mods::default());
    assert_eq!(core.focus(), Some(other));

    // A modal that displaced nothing gives nothing back.
    core.set_focus(None);
    modal_frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k[3]));
    modal_frame(&mut core, false, false);
    assert_eq!(core.focus(), None);
}

#[test]
fn a_modal_with_no_controls_holds_focus_by_holding_none() {
    let mut core = Core::new();
    let k = modal_frame(&mut core, false, false);
    core.set_focus(Some(k[0]));
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("open", NodeSpec::row().on_click(Value::str("open")), |_| {});
    ui.with_keyed(
        "note",
        NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(20.0))
            .float(FloatConfig::viewport())
            .modal(Value::Null)
            .label("Working"),
        |_| {},
    );
    ui.finish();
    assert_eq!(
        core.focus(),
        None,
        "nothing focusable inside: nothing holds"
    );
    tab(&mut core, Mods::default());
    assert_eq!(core.focus(), None, "and Tab cannot leave");
}

#[test]
fn a_dialog_opened_from_the_keyboard_shows_its_ring_at_once() {
    // Entering the modal keeps the visibility focus had: Tab-and-Enter
    // shows the ring on the first control, a click shows none until Tab.
    let mut core = Core::new();
    modal_frame(&mut core, false, false);
    tab(&mut core, Mods::default());
    assert!(core.focus_visible(), "Tab showed the ring on `open`");
    let k = modal_frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k[3]));
    assert!(core.focus_visible(), "and the ring came into the dialog");

    let mut core = Core::new();
    modal_frame(&mut core, false, false);
    core.handle_input(InputEvent::CursorMoved(kui_core::Vec2::new(50.0, 10.0)));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::mouse_up());
    assert!(!core.focus_visible(), "a click is pointer focus");
    let k = modal_frame(&mut core, true, false);
    assert_eq!(core.focus(), Some(k[3]));
    assert!(!core.focus_visible(), "no ring until the first Tab");
}
