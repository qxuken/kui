//! A secure editor, a password field (`EditOptions::secure`, backlog
//! F167, from Carnet's review): the text drawn as a bullet per grapheme,
//! never copied or cut, never composed into, kept out of the undo
//! history, read by the access tree as a password field whose value is the
//! bullets, and asking for secure keyboard entry and the input method off
//! while it has the keyboard. The app reads the text as it reads any
//! editor's, and a "show password" toggle is the same editor declared
//! without `secure`.

use kui_core::{
    ClipboardMarks, Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, QuadKind, Role,
    SECURE_BULLET, Size,
};

const SHIFT: Mods = Mods {
    shift: true,
    word: false,
    doc: false,
};
const WORD: Mods = Mods {
    shift: false,
    word: true,
    doc: false,
};

/// A field keyed "pw", focused, `secure` as asked; its key.
fn frame(core: &mut Core, initial: &str, secure: bool) -> Key {
    frame_with(core, initial, secure, false)
}

fn frame_with(core: &mut Core, initial: &str, secure: bool, multiline: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.text_edit(
        "pw",
        initial,
        &EditOptions {
            autofocus: true,
            secure,
            multiline,
            ..Default::default()
        },
        NodeSpec::column().width(300.0).pad(5.0).label("Password"),
    );
    ui.finish();
    key
}

fn bullets(n: usize) -> String {
    std::iter::repeat_n(SECURE_BULLET, n).collect()
}

fn glyphs(core: &mut Core) -> usize {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
        .count()
}

fn press(core: &mut Core, k: EditKey, mods: Mods) {
    core.handle_input(InputEvent::Key(k, mods));
}

/// "é" as `e` and a combining acute: one grapheme, one bullet.
const SECRET: &str = "pa\u{301}ss w0rd";

#[test]
fn the_text_is_drawn_as_a_bullet_per_grapheme_and_read_back_whole() {
    let mut core = Core::new();
    let key = frame(&mut core, SECRET, true);
    frame(&mut core, SECRET, true);
    assert_eq!(core.edit_text(key).as_deref(), Some(SECRET));
    // Nine graphemes, nine bullets: one glyph each, and a run of them
    // is all the access tree has.
    assert_eq!(glyphs(&mut core), 9);
    let node = core.access_tree().get(key).expect("the field").clone();
    assert_eq!(node.role, Role::PasswordInput);
    assert_eq!(node.name.as_deref(), Some("Password"));
    assert_eq!(node.value.as_deref(), Some(bullets(9).as_str()));
    let runs: String = node.runs.iter().map(|r| r.text.as_str()).collect();
    assert_eq!(runs, bullets(9));
    assert!(!format!("{node:?}").contains("w0rd"), "{node:?}");
    // Measured as the bullets: as wide as a plain field holding them.
    let mut plain = Core::new();
    let other = frame(&mut plain, &bullets(9), false);
    frame(&mut plain, &bullets(9), false);
    assert_eq!(
        core.edit_caret_rect(key).map(|r| r.x),
        plain.edit_caret_rect(other).map(|r| r.x),
        "the caret after the ninth bullet"
    );
}

#[test]
fn typing_deleting_and_selecting_edit_the_text_under_the_bullets() {
    let mut core = Core::new();
    let key = frame(&mut core, "", true);
    core.handle_input(InputEvent::Text("abc".into()));
    assert_eq!(core.edit_text(key).as_deref(), Some("abc"));
    press(&mut core, EditKey::Left, Mods::default());
    press(&mut core, EditKey::Backspace, Mods::default());
    assert_eq!(core.edit_text(key).as_deref(), Some("ac"));
    core.handle_input(InputEvent::Text("e\u{301}".into()));
    assert_eq!(core.edit_text(key).as_deref(), Some("ae\u{301}c"));
    // One Backspace takes the grapheme its bullet stands for.
    press(&mut core, EditKey::Backspace, Mods::default());
    assert_eq!(core.edit_text(key).as_deref(), Some("ac"));
    // A selection by Shift: typing replaces it.
    press(&mut core, EditKey::End, Mods::default());
    press(&mut core, EditKey::Left, SHIFT);
    core.handle_input(InputEvent::Text("Z!".into()));
    assert_eq!(core.edit_text(key).as_deref(), Some("aZ!"));
    // A paste is taken, as typing is.
    core.handle_input(InputEvent::Paste {
        text: "pasted".into(),
        marks: ClipboardMarks {
            concealed: true,
            transient: true,
        },
    });
    assert_eq!(core.edit_text(key).as_deref(), Some("aZ!pasted"));
    // A word's key takes the line: the bullets show no words.
    press(&mut core, EditKey::Left, Mods::default());
    press(&mut core, EditKey::Backspace, WORD);
    assert_eq!(core.edit_text(key).as_deref(), Some("d"));
    // Select all and type over it; `set_edit_text` replaces it.
    press(&mut core, EditKey::SelectAll, Mods::default());
    core.handle_input(InputEvent::Text("new".into()));
    assert_eq!(core.edit_text(key).as_deref(), Some("new"));
    assert!(core.set_edit_text(key, "set by the app"));
    assert_eq!(core.edit_text(key).as_deref(), Some("set by the app"));
    frame(&mut core, "", true);
    assert_eq!(
        core.access_tree().get(key).and_then(|n| n.value.clone()),
        Some(bullets(14))
    );
}

#[test]
fn a_copy_and_a_cut_are_refused_and_the_text_stays() {
    let mut core = Core::new();
    let key = frame(&mut core, "hunter2", true);
    press(&mut core, EditKey::SelectAll, Mods::default());
    assert!(core.edit.has_selection(key), "it can be selected");
    assert!(!core.edit.can_copy(key), "but not copied");
    assert_eq!(core.copy_selection(), None);
    assert_eq!(core.cut_selection(), None);
    assert_eq!(core.edit_text(key).as_deref(), Some("hunter2"));
    // Shown, the same selection copies.
    let key = frame(&mut core, "", false);
    press(&mut core, EditKey::SelectAll, Mods::default());
    assert!(core.edit.can_copy(key));
    assert_eq!(core.copy_selection().as_deref(), Some("hunter2"));
}

#[test]
fn no_composition_and_no_history() {
    let mut core = Core::new();
    let key = frame(&mut core, "", true);
    core.handle_input(InputEvent::Preedit("にほ".into(), Some((6, 6))));
    assert_eq!(core.edit.preedit(key), None, "nothing composes");
    assert_eq!(core.edit_text(key).as_deref(), Some(""));
    core.handle_input(InputEvent::Text("abc".into()));
    press(&mut core, EditKey::Backspace, Mods::default());
    assert_eq!(core.edit.history(key), (false, false));
    press(&mut core, EditKey::Undo, Mods::default());
    assert_eq!(
        core.edit_text(key).as_deref(),
        Some("ab"),
        "nothing to undo"
    );
}

#[test]
fn it_asks_for_secure_input_and_the_input_method_off_while_it_has_focus() {
    let mut core = Core::new();
    let key = frame(&mut core, "", true);
    assert!(core.is_focused(key));
    assert!(core.secure_input() && core.ime_off());
    core.set_focus(None);
    assert!(!core.secure_input() && !core.ime_off(), "not without focus");
    core.set_focus(Some(key));
    assert!(core.secure_input() && core.ime_off());
    // A field that is not secure asks for neither.
    let key = frame(&mut core, "", false);
    assert!(core.is_focused(key));
    assert!(!core.secure_input() && !core.ime_off());
}

#[test]
fn a_show_password_toggle_shows_the_text_and_masks_it_again() {
    let mut core = Core::new();
    let key = frame(&mut core, SECRET, true);
    // The caret before "w0rd": five graphemes in.
    press(&mut core, EditKey::Home, Mods::default());
    for _ in 0..5 {
        press(&mut core, EditKey::Right, Mods::default());
    }
    let masked = core.edit.caret_and_selection(key).map(|c| c.0);
    assert_eq!(masked, Some(5 * SECURE_BULLET.len_utf8()));
    // Shown: the text, the caret before the same grapheme.
    frame(&mut core, SECRET, false);
    frame(&mut core, SECRET, false);
    let node = core.access_tree().get(key).expect("the field").clone();
    assert_eq!(node.role, Role::TextInput);
    assert_eq!(node.value.as_deref(), Some(SECRET));
    assert_eq!(node.caret, Some(SECRET.find("w0rd").unwrap()));
    core.handle_input(InputEvent::Text("X".into()));
    assert_eq!(core.edit_text(key).as_deref(), Some("pa\u{301}ss Xw0rd"));
    assert_eq!(core.edit.history(key), (true, false), "shown, it has one");
    // Masked again: the bullets, the caret after the X, no history.
    frame(&mut core, SECRET, true);
    frame(&mut core, SECRET, true);
    let node = core.access_tree().get(key).expect("the field").clone();
    assert_eq!(node.role, Role::PasswordInput);
    assert_eq!(node.value.as_deref(), Some(bullets(10).as_str()));
    assert_eq!(node.caret, Some(6 * SECURE_BULLET.len_utf8()));
    assert_eq!(core.edit.history(key), (false, false));
    assert_eq!(core.edit_text(key).as_deref(), Some("pa\u{301}ss Xw0rd"));
}

#[test]
fn a_secure_editor_is_a_field() {
    let mut core = Core::new();
    let key = frame_with(&mut core, "a\nb", true, true);
    assert_eq!(core.edit_text(key).as_deref(), Some("ab"), "one line");
    let events = core.handle_input(InputEvent::Key(EditKey::Enter, Mods::default()));
    assert_eq!(core.edit_text(key).as_deref(), Some("ab"));
    assert!(
        events
            .iter()
            .any(|e| e.payload.get_str("kind") == Some("submit")),
        "Enter submits: {events:?}"
    );
    frame_with(&mut core, "", true, true);
    assert_eq!(
        core.access_tree().get(key).map(|n| n.role),
        Some(Role::PasswordInput)
    );
}
