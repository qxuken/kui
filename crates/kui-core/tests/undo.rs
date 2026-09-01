//! Undo/redo of the edit widget's own history: coalesced typing bursts,
//! delete runs, selection replaces, caret restore, and history limits.

use kui_core::{Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, Size, Sizing};

const SHIFT: Mods = Mods { shift: true, word: false, doc: false };

struct Rig {
    core: Core,
    key: Key,
}

impl Rig {
    fn new(initial: &str, multiline: bool) -> Rig {
        let mut core = Core::new();
        let key = frame(&mut core, initial, multiline);
        Rig { core, key }
    }

    fn text(&self) -> String {
        self.core.edit_text(self.key).unwrap()
    }

    fn type_str(&mut self, s: &str) {
        for ch in s.chars() {
            self.core.handle_input(InputEvent::Text(ch.to_string()));
        }
    }

    fn press(&mut self, k: EditKey, mods: Mods) -> usize {
        self.core.handle_input(InputEvent::Key(k, mods)).len()
    }

    fn undo(&mut self) -> usize {
        self.press(EditKey::Undo, Mods::default())
    }

    fn redo(&mut self) -> usize {
        self.press(EditKey::Redo, Mods::default())
    }
}

fn frame(core: &mut Core, initial: &str, multiline: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.text_edit(
        "field",
        initial,
        &EditOptions { multiline, autofocus: true, ..Default::default() },
        NodeSpec::column().width(Sizing::Grow(1.0)),
    );
    ui.finish();
    key
}

#[test]
fn typing_burst_undoes_as_one_unit() {
    let mut rig = Rig::new("", false);
    rig.type_str("hello");
    assert_eq!(rig.text(), "hello");
    let events = rig.undo();
    assert_eq!(rig.text(), "", "one undo removes the whole burst");
    assert_eq!(events, 1, "undo emits one changed event");
    rig.redo();
    assert_eq!(rig.text(), "hello");
}

#[test]
fn caret_motion_starts_a_new_unit() {
    let mut rig = Rig::new("", false);
    rig.type_str("ab");
    rig.press(EditKey::Left, Mods::default());
    rig.press(EditKey::Right, Mods::default());
    rig.type_str("cd");
    assert_eq!(rig.text(), "abcd");
    rig.undo();
    assert_eq!(rig.text(), "ab", "motion split the burst");
    rig.undo();
    assert_eq!(rig.text(), "");
}

#[test]
fn backspace_run_coalesces_and_restores() {
    let mut rig = Rig::new("hello", false);
    rig.press(EditKey::End, Mods::default());
    for _ in 0..3 {
        rig.press(EditKey::Backspace, Mods::default());
    }
    assert_eq!(rig.text(), "he");
    rig.undo();
    assert_eq!(rig.text(), "hello", "one undo restores the run");
    // Caret is back at the end: typing lands there.
    rig.type_str("!");
    assert_eq!(rig.text(), "hello!");
}

#[test]
fn forward_delete_coalesces() {
    let mut rig = Rig::new("hello", false);
    // Cursor starts at buffer start.
    for _ in 0..3 {
        rig.press(EditKey::Delete, Mods::default());
    }
    assert_eq!(rig.text(), "lo");
    rig.undo();
    assert_eq!(rig.text(), "hello");
}

#[test]
fn selection_replace_undoes_to_selection_text() {
    let mut rig = Rig::new("abcdef", false);
    for _ in 0..3 {
        rig.press(EditKey::Right, SHIFT);
    }
    rig.type_str("X");
    assert_eq!(rig.text(), "Xdef");
    rig.undo();
    assert_eq!(rig.text(), "abcdef");
    rig.redo();
    assert_eq!(rig.text(), "Xdef");
}

#[test]
fn newline_breaks_typing_coalescing() {
    let mut rig = Rig::new("", true);
    rig.type_str("ab");
    rig.press(EditKey::Enter, Mods::default());
    rig.type_str("cd");
    assert_eq!(rig.text(), "ab\ncd");
    rig.undo();
    assert_eq!(rig.text(), "ab\n");
    rig.undo();
    assert_eq!(rig.text(), "ab");
    rig.undo();
    assert_eq!(rig.text(), "");
}

#[test]
fn new_edit_clears_redo() {
    let mut rig = Rig::new("", false);
    rig.type_str("one");
    rig.undo();
    rig.type_str("two");
    let events = rig.redo();
    assert_eq!(rig.text(), "two", "redo after a fresh edit is a no-op");
    assert_eq!(events, 0, "a no-op redo emits nothing");
}

#[test]
fn cut_is_undoable() {
    let mut rig = Rig::new("hello world", false);
    rig.press(EditKey::Home, Mods::default());
    for _ in 0..5 {
        rig.press(EditKey::Right, SHIFT);
    }
    assert_eq!(rig.core.cut_selection().as_deref(), Some("hello"));
    assert_eq!(rig.text(), " world");
    rig.undo();
    assert_eq!(rig.text(), "hello world");
}

#[test]
fn set_text_clears_history() {
    let mut rig = Rig::new("", false);
    rig.type_str("typed");
    rig.core.set_edit_text(rig.key, "replaced");
    let events = rig.undo();
    assert_eq!(rig.text(), "replaced", "no history across a wholesale replace");
    assert_eq!(events, 0);
}

#[test]
fn undo_at_empty_history_emits_nothing() {
    let mut rig = Rig::new("abc", false);
    assert_eq!(rig.undo(), 0);
    assert_eq!(rig.redo(), 0);
    assert_eq!(rig.text(), "abc");
}

#[test]
fn backspace_at_buffer_start_emits_nothing() {
    let mut rig = Rig::new("x", false);
    // Cursor starts at buffer start: nothing to delete leftwards.
    let events = rig.press(EditKey::Backspace, Mods::default());
    assert_eq!(events, 0, "boundary backspace is not a change");
    assert_eq!(rig.text(), "x");
}

#[test]
fn multiline_edits_round_trip() {
    let mut rig = Rig::new("alpha\nbeta\ngamma", true);
    // Jump to doc end, remove "gamma" word-wise, then undo everything.
    rig.press(EditKey::End, Mods { doc: true, ..Mods::default() });
    rig.press(EditKey::Backspace, Mods { word: true, ..Mods::default() });
    assert_eq!(rig.text(), "alpha\nbeta\n");
    rig.press(EditKey::Backspace, Mods::default());
    assert_eq!(rig.text(), "alpha\nbeta");
    rig.undo();
    rig.undo();
    assert_eq!(rig.text(), "alpha\nbeta\ngamma");
}
