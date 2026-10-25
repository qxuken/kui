//! Tab focus traversal: cycling between edit widgets in tree order,
//! multiline editors that take Tab with three ways out (backlog C99), sink precedence, and the
//! ring confined to a modal subtree
//! (`docs/adr/0003-modal-surfaces.md`).

use kui_core::testing::tab;
use kui_core::{
    Align, Core, EditOptions, FloatConfig, InputEvent, Key, KeyCode, KeyMods, KeyPress, NodeSpec,
    Size, Value,
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
            NodeSpec::column().grow_width(),
        ));
    }
    ui.finish();
    keys
}

#[test]
fn tab_cycles_and_shift_tab_reverses() {
    let mut core = Core::new();
    let keys = frame(&mut core, true);
    assert_eq!(core.edit.focused(), Some(keys[0]));

    tab(&mut core, false);
    assert_eq!(core.edit.focused(), Some(keys[1]));
    tab(&mut core, false);
    assert_eq!(core.edit.focused(), Some(keys[2]));
    tab(&mut core, true);
    assert_eq!(core.edit.focused(), Some(keys[1]));
    // Backwards past the first wraps to the last (the multiline).
    tab(&mut core, true);
    tab(&mut core, true);
    assert_eq!(core.edit.focused(), Some(keys[3]));
}

/// A field, a multiline editor and a button after it: what the C99 tests
/// below Tab through. `moves` declares `tab_moves_focus` on the editor.
fn area_frame(core: &mut Core, moves: bool) -> (Key, Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0).gap(8.0));
    let field = ui.text_edit(
        "a",
        "a",
        &EditOptions::default(),
        NodeSpec::column().grow_width(),
    );
    let area = ui.text_edit(
        "notes",
        "",
        &EditOptions {
            multiline: true,
            tab_moves_focus: moves,
            ..Default::default()
        },
        NodeSpec::column().grow_width(),
    );
    let after = ui.leaf_keyed("after", NodeSpec::row().size(80.0, 20.0).on_click("after"));
    ui.finish();
    (field, area, after)
}

fn key(code: KeyCode, mods: KeyMods) -> KeyPress {
    KeyPress::new(code, mods)
}

/// A multiline editor takes Tab as indentation by default — the editors'
/// model, not the textarea's (backlog C99) — and Shift-Tab takes the
/// indent back from the start of the line.
#[test]
fn a_multiline_editor_takes_tab_and_shift_tab_takes_it_back() {
    let mut core = Core::new();
    let (_, area, _) = area_frame(&mut core, false);
    core.set_focus(Some(area));
    tab(&mut core, false);
    assert_eq!(core.edit.focused(), Some(area), "focus stays");
    assert_eq!(
        core.edit_text(area).as_deref(),
        Some("    "),
        "tab indented"
    );
    core.handle_input(InputEvent::Text("x".into()));
    tab(&mut core, true);
    assert_eq!(core.edit.focused(), Some(area), "shift-tab stays too");
    assert_eq!(
        core.edit_text(area).as_deref(),
        Some("x"),
        "shift-tab outdented"
    );
    tab(&mut core, true);
    assert_eq!(
        core.edit_text(area).as_deref(),
        Some("x"),
        "nothing left to take"
    );
}

/// Escape then Tab moves focus on from the editor (CodeMirror's way out),
/// and Escape then Shift-Tab back: the Escape keeps the keyboard where it
/// is and arms the next Tab only.
#[test]
fn escape_then_tab_leaves_a_multiline_editor() {
    let mut core = Core::new();
    let (field, area, after) = area_frame(&mut core, false);
    core.set_focus(Some(area));
    core.press(key(KeyCode::Escape, KeyMods::NONE));
    assert_eq!(
        core.focus(),
        Some(area),
        "the escape keeps the editor focused"
    );
    core.press(key(KeyCode::Tab, KeyMods::NONE));
    assert_eq!(core.focus(), Some(after), "the armed tab went on");
    assert_eq!(core.edit_text(area).as_deref(), Some(""), "nothing typed");
    core.set_focus(Some(area));
    core.press(key(KeyCode::Escape, KeyMods::NONE));
    core.handle_input(InputEvent::KeyDown(key(
        KeyCode::Shift,
        KeyMods::NONE.with_shift(),
    )));
    core.press(key(KeyCode::Tab, KeyMods::NONE.with_shift()));
    assert_eq!(core.focus(), Some(field), "shift held between them, back");
}

/// The Escape arms the next Tab only: any other key in between disarms
/// it, so the Tab types again; and a second Escape lets go, as Escape
/// does in a field.
#[test]
fn any_other_key_disarms_the_escape_and_a_second_escape_lets_go() {
    let mut core = Core::new();
    let (_, area, _) = area_frame(&mut core, false);
    core.set_focus(Some(area));
    core.press(key(KeyCode::Escape, KeyMods::NONE));
    core.press(key(KeyCode::Char('x'), KeyMods::NONE).with_text("x"));
    core.press(key(KeyCode::Tab, KeyMods::NONE));
    assert_eq!(core.focus(), Some(area));
    assert_eq!(
        core.edit_text(area).as_deref(),
        Some("x    "),
        "the tab typed"
    );
    core.press(key(KeyCode::Escape, KeyMods::NONE));
    core.press(key(KeyCode::Left, KeyMods::NONE));
    core.press(key(KeyCode::Tab, KeyMods::NONE));
    assert_eq!(core.focus(), Some(area), "an arrow disarms too");
    core.press(key(KeyCode::Escape, KeyMods::NONE));
    core.press(key(KeyCode::Escape, KeyMods::NONE));
    assert_eq!(core.focus(), None, "the second escape blurs");
}

/// Ctrl+M (Ctrl+Shift+M on macOS) in such an editor flips the window's
/// "Tab moves focus" toggle, which the app reads; while it is on, Tab
/// walks the ring from every multiline editor, and the chord reaches no
/// sink.
#[test]
fn the_toggle_makes_tab_move_focus_and_an_app_reads_it() {
    let mut core = Core::new();
    let (_, area, after) = area_frame(&mut core, false);
    let chord = if cfg!(target_os = "macos") {
        KeyMods::NONE.with_ctrl().with_shift()
    } else {
        KeyMods::NONE.with_ctrl()
    };
    let m = if cfg!(target_os = "macos") { 'M' } else { 'm' };
    assert!(!core.tab_moves_focus());
    core.set_focus(Some(area));
    let out = core.press(key(KeyCode::Char(m), chord).with_physical(KeyCode::Char('m')));
    assert!(out.is_empty(), "nobody hears the chord: {out:?}");
    assert!(core.tab_moves_focus(), "toggled on");
    assert!(core.animating(), "a status label is owed a frame");
    core.press(key(KeyCode::Tab, KeyMods::NONE));
    assert_eq!(core.focus(), Some(after), "tab moves focus while it is on");
    // Off a multiline editor the chord is whoever's it was: no toggle.
    core.press(key(KeyCode::Char(m), chord).with_physical(KeyCode::Char('m')));
    assert!(core.tab_moves_focus(), "not toggled from a button");
    core.set_tab_moves_focus(false);
    core.set_focus(Some(area));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(area), "off again, tab types");
}

/// `tab_moves_focus` gives the textarea's behaviour: Tab and Shift-Tab
/// leave the editor as they leave a field, and Escape lets go at once.
#[test]
fn tab_moves_focus_leaves_as_a_field_does() {
    let mut core = Core::new();
    let (field, area, after) = area_frame(&mut core, true);
    core.set_focus(Some(area));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(after));
    core.set_focus(Some(area));
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(field));
    assert_eq!(core.edit_text(area).as_deref(), Some(""), "nothing typed");
    core.set_focus(Some(area));
    core.press(key(KeyCode::Escape, KeyMods::NONE));
    assert_eq!(core.focus(), None, "escape blurs it at once");
}

/// Tab under a modifier walks the ring from any multiline editor: the
/// way out a code editor always keeps.
#[test]
fn ctrl_tab_still_leaves_a_multiline_editor() {
    let mut core = Core::new();
    let (_, area, after) = area_frame(&mut core, false);
    core.set_focus(Some(area));
    core.handle_input(InputEvent::Key(
        kui_core::EditKey::Tab,
        kui_core::Mods::NONE.with_doc(),
    ));
    assert_eq!(core.edit.focused(), None);
    assert_eq!(core.focus(), Some(after), "ctrl-tab walks the ring");
}

#[test]
fn tab_with_no_focus_enters_the_ring() {
    let mut core = Core::new();
    let keys = frame(&mut core, false);
    assert_eq!(core.edit.focused(), None);
    tab(&mut core, false);
    assert_eq!(core.edit.focused(), Some(keys[0]));
    core.set_focus(None);
    tab(&mut core, true);
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
    let sink = ui.with_keyed("app", NodeSpec::column().fill().key_sink(), |ui| {
        ui.text_edit("field", "x", &EditOptions::default(), NodeSpec::column());
    });
    ui.take_key_focus(sink);
    ui.finish();

    tab(&mut core, false);
    assert_eq!(core.edit.focused(), None, "the sink keeps the keyboard");
}

// -- Modal scope (docs/adr/0003-modal-surfaces.md) --------------------------

/// Two background buttons and, when `dialog` says so, a floated modal
/// holding two more — plus a confirm nested inside it when `confirm` does.
/// Rows are 100x20: the background at the top left, the modal at the
/// bottom right of a 200x300 viewport.
fn modal_frame(core: &mut Core, dialog: bool, confirm: bool) -> Vec<Key> {
    modal_frame_with(core, dialog, confirm, Entry::First)
}

/// What the dialog in [`modal_frame_with`] says about where focus starts:
/// nothing (ADR 0003's first-focusable), `initialFocus` on its second
/// control, or `initialFocus` on a control the Tab ring skips.
#[derive(Clone, Copy, PartialEq)]
enum Entry {
    First,
    Cancel,
    Disabled,
}

fn modal_frame_with(core: &mut Core, dialog: bool, confirm: bool, entry: Entry) -> Vec<Key> {
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = || NodeSpec::row().size(100.0, 20.0);
    let mut keys = vec![
        ui.leaf_keyed("open", row().on_click("open")),
        ui.leaf_keyed("other", row().on_click("other")),
    ];
    if dialog {
        let dlg = ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .size(100.0, 40.0)
                .float(FloatConfig::viewport().inside(Align::End, Align::End))
                .modal("dlg")
                .label("Settings"),
            |ui| {
                // The destructive one first, so declaration order alone
                // would open the dialog on it.
                ui.leaf_keyed("ok", row().on_click("ok"));
                let mut cancel = row().on_click("cancel");
                if entry == Entry::Cancel {
                    cancel = cancel.initial_focus();
                }
                ui.leaf_keyed("cancel", cancel);
                if entry == Entry::Disabled {
                    // Declared, and skipped by the ring anyway.
                    ui.leaf_keyed(
                        "gone",
                        row().on_click("gone").disabled(true).initial_focus(),
                    );
                }
                if confirm {
                    ui.with_keyed(
                        "confirm",
                        NodeSpec::column()
                            .size(100.0, 20.0)
                            .float(FloatConfig::viewport())
                            .modal(Value::Null)
                            .label("Sure?"),
                        |ui| {
                            ui.leaf_keyed("yes", row().on_click("yes"));
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
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(cancel));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(ok), "the ring wraps inside the modal");
    tab(&mut core, true);
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
    tab(&mut core, false);
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
    tab(&mut core, false);
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
    tab(&mut core, false);
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
    ui.leaf_keyed("open", NodeSpec::row().on_click("open"));
    ui.leaf_keyed(
        "note",
        NodeSpec::column()
            .size(100.0, 20.0)
            .float(FloatConfig::viewport())
            .modal(Value::Null)
            .label("Working"),
    );
    ui.finish();
    assert_eq!(
        core.focus(),
        None,
        "nothing focusable inside: nothing holds"
    );
    tab(&mut core, false);
    assert_eq!(core.focus(), None, "and Tab cannot leave");
}

#[test]
fn a_dialog_opened_from_the_keyboard_shows_its_ring_at_once() {
    // Entering the modal keeps the visibility focus had: Tab-and-Enter
    // shows the ring on the first control, a click shows none until Tab.
    let mut core = Core::new();
    modal_frame(&mut core, false, false);
    tab(&mut core, false);
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

// -- Initial focus inside a modal (`initialFocus`) --------------------------
// ADR 0003 decision 3 enters a modal at its *first* focusable node, which
// makes a destructive confirm open on Delete whenever Delete is declared
// first. `initialFocus` names the entry instead; nothing declaring it is
// still the first-focusable rule.

#[test]
fn a_dialog_opens_on_the_control_that_asks_for_it() {
    let mut core = Core::new();
    let k = modal_frame_with(&mut core, false, false, Entry::Cancel);
    core.set_focus(Some(k[0]));

    let k = modal_frame_with(&mut core, true, false, Entry::Cancel);
    let (ok, cancel) = (k[3], k[4]);
    assert_eq!(
        core.focus(),
        Some(cancel),
        "the declared entry, not the first control"
    );
    // Everything else about the scope is unchanged: the ring is still the
    // whole subtree, in tree order, wrapping.
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(ok));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(cancel));
}

#[test]
fn without_a_declaration_the_first_focusable_still_wins() {
    let mut core = Core::new();
    let k = modal_frame_with(&mut core, false, false, Entry::First);
    core.set_focus(Some(k[0]));
    let k = modal_frame_with(&mut core, true, false, Entry::First);
    assert_eq!(core.focus(), Some(k[3]), "ADR 0003's rule, untouched");

    // And a declaration the ring cannot honour is the same as none: a
    // disabled node is not a Tab stop, so it is not an entry either.
    let mut core = Core::new();
    let k = modal_frame_with(&mut core, false, false, Entry::Disabled);
    core.set_focus(Some(k[0]));
    let k = modal_frame_with(&mut core, true, false, Entry::Disabled);
    assert_eq!(core.focus(), Some(k[3]));
}

#[test]
fn the_entry_is_read_once_and_not_re_taken_while_the_dialog_stays_up() {
    let mut core = Core::new();
    modal_frame_with(&mut core, false, false, Entry::Cancel);
    let k = modal_frame_with(&mut core, true, false, Entry::Cancel);
    let (ok, cancel) = (k[3], k[4]);
    assert_eq!(core.focus(), Some(cancel));

    // The user moves off it. The dialog is declared again — every frame,
    // the same declaration — and focus stays where they left it.
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(ok));
    modal_frame_with(&mut core, true, false, Entry::Cancel);
    assert_eq!(core.focus(), Some(ok), "a redeclaration is not an entry");
    modal_frame_with(&mut core, true, false, Entry::Cancel);
    assert_eq!(core.focus(), Some(ok));

    // A click inside is not an entry either.
    core.set_focus(Some(ok));
    modal_frame_with(&mut core, true, false, Entry::Cancel);
    assert_eq!(core.focus(), Some(ok));
}

#[test]
fn the_entry_does_not_disturb_the_focus_the_modal_gives_back() {
    // Why this is a row and not `keyFocus`: `keyFocus` moves focus while
    // the frame is being built, so the modal remembers *it* as the focus
    // it displaced (decision 4) and the opener never gets it back. The
    // entry is resolved after the scope is known, so it does not.
    let mut core = Core::new();
    let k = modal_frame_with(&mut core, false, false, Entry::Cancel);
    let (open, other) = (k[0], k[1]);
    core.set_focus(Some(open));

    let k = modal_frame_with(&mut core, true, false, Entry::Cancel);
    assert_eq!(core.focus(), Some(k[4]));
    modal_frame_with(&mut core, false, false, Entry::Cancel);
    assert_eq!(
        core.focus(),
        Some(open),
        "back to the button that opened it"
    );
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(other));
}

#[test]
fn a_confirm_closing_does_not_re_enter_the_dialog() {
    // The scope is re-entered from the inside: the confirm hands back the
    // focus it displaced, which is already in the dialog, so the dialog's
    // entry is not read a second time.
    let mut core = Core::new();
    modal_frame_with(&mut core, true, false, Entry::Cancel);
    let (ok, cancel) = (
        Key::ROOT.str("dialog").str("ok"),
        Key::ROOT.str("dialog").str("cancel"),
    );
    assert_eq!(core.focus(), Some(cancel));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(ok));

    let yes = Key::ROOT.str("dialog").str("confirm").str("yes");
    modal_frame_with(&mut core, true, true, Entry::Cancel);
    assert_eq!(core.focus(), Some(yes), "the confirm's own entry");

    modal_frame_with(&mut core, true, false, Entry::Cancel);
    assert_eq!(
        core.focus(),
        Some(ok),
        "where the confirm found it, not the dialog's entry"
    );
}
