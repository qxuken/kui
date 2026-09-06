//! End-to-end text editing through a live `Core`: focus, typing, motion,
//! selection, deletion, click-to-position, and event emission.

use kui_core::{Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, Size, Sizing, Vec2};

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

    fn frame(&mut self) {
        frame(&mut self.core, "", false); // initial/multiline only matter on create
    }

    fn text(&self) -> String {
        self.core.edit_text(self.key).unwrap()
    }

    fn type_str(&mut self, s: &str) -> usize {
        let evs = self.core.handle_input(InputEvent::Text(s.to_string()));
        evs.len()
    }

    fn press(&mut self, k: EditKey, mods: Mods) {
        self.core.handle_input(InputEvent::Key(k, mods));
    }
}

fn frame(core: &mut Core, initial: &str, multiline: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.text_edit(
        "field",
        initial,
        &EditOptions {
            multiline,
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().width(Sizing::Grow(1.0)).pad(5.0),
    );
    ui.finish();
    key
}

#[test]
fn typing_inserts_at_cursor_end() {
    let mut rig = Rig::new("hello", false);
    assert!(
        rig.core.is_focused(rig.key),
        "autofocus should focus the field"
    );
    // A fresh single-line field opens with the caret after its seeded text
    // (backlog F20): no `End` press first.
    let events = rig.type_str(" world");
    assert_eq!(rig.text(), "hello world");
    assert_eq!(events, 1, "expected one changed event");
}

#[test]
fn a_seeded_document_opens_at_its_top() {
    // A multiline editor is a document: the caret opens at (0, 0), as a
    // native text view's does, so typing lands before the seed.
    let mut rig = Rig::new("second line", true);
    rig.type_str("first line\n");
    assert_eq!(rig.text(), "first line\nsecond line");
}

#[test]
fn a_returning_editor_keeps_its_draft() {
    // `initial` seeds a new editor only; the same key declared again with
    // another seed keeps what the user typed (the mind map's "abandoned
    // draft comes back" note). `set_edit_text` is what resets one.
    let mut rig = Rig::new("draft", false);
    rig.type_str("!");
    assert_eq!(rig.text(), "draft!");
    frame(&mut rig.core, "something else", false);
    assert_eq!(rig.text(), "draft!", "a redeclaration is not a reseed");
    rig.core.set_edit_text(rig.key, "reset");
    assert_eq!(rig.text(), "reset");
    rig.type_str("?");
    assert_eq!(
        rig.text(),
        "reset?",
        "set_edit_text leaves the caret at the end"
    );
}

#[test]
fn backspace_and_delete() {
    let mut rig = Rig::new("abc", false);
    rig.press(EditKey::End, Mods::default());
    rig.press(EditKey::Backspace, Mods::default());
    assert_eq!(rig.text(), "ab");
    rig.press(EditKey::Home, Mods::default());
    rig.press(EditKey::Delete, Mods::default());
    assert_eq!(rig.text(), "b");
}

#[test]
fn word_motion_and_word_backspace() {
    let mut rig = Rig::new("alpha beta gamma", false);
    rig.press(EditKey::End, Mods::default());
    rig.press(EditKey::Backspace, WORD);
    assert_eq!(rig.text(), "alpha beta ");
    rig.press(EditKey::Left, WORD);
    rig.press(EditKey::Backspace, Mods::default());
    assert_eq!(rig.text(), "alphabeta ");
}

#[test]
fn shift_selection_then_type_replaces() {
    let mut rig = Rig::new("abcdef", false);
    // Select "abc" from the start, then replace it. The field opens with
    // the caret after its seed (F20), so go to the start first.
    rig.press(EditKey::Home, Mods::default());
    for _ in 0..3 {
        rig.press(EditKey::Right, SHIFT);
    }
    rig.type_str("X");
    assert_eq!(rig.text(), "Xdef");
}

#[test]
fn select_all_then_delete_clears() {
    let mut rig = Rig::new("some longer content", false);
    rig.press(EditKey::SelectAll, Mods::default());
    rig.press(EditKey::Backspace, Mods::default());
    assert_eq!(rig.text(), "");
}

#[test]
fn copy_and_cut_selection() {
    let mut rig = Rig::new("hello world", false);
    rig.press(EditKey::Home, Mods::default());
    for _ in 0..5 {
        rig.press(EditKey::Right, SHIFT);
    }
    assert_eq!(rig.core.copy_selection().as_deref(), Some("hello"));
    assert_eq!(rig.core.cut_selection().as_deref(), Some("hello"));
    assert_eq!(rig.text(), " world");
}

#[test]
fn multiline_enter_splits_singleline_submits() {
    let mut multi = Rig::new("ab", true);
    multi.press(EditKey::End, Mods::default());
    multi.press(EditKey::Enter, Mods::default());
    multi.type_str("cd");
    assert_eq!(multi.text(), "ab\ncd");

    let mut single = Rig::new("ab", false);
    single.press(EditKey::End, Mods::default());
    let evs = single
        .core
        .handle_input(InputEvent::Key(EditKey::Enter, Mods::default()));
    assert_eq!(single.text(), "ab");
    assert_eq!(evs.len(), 1);
    assert_eq!(
        evs[0].payload.get("kind").and_then(kui_core::Value::as_str),
        Some("submit")
    );
}

#[test]
fn click_focuses_and_places_caret() {
    let mut rig = Rig::new("mmmm mmmm", false);
    rig.core.edit.set_focus(None);
    rig.frame(); // lay out so hit regions exist

    // Click near the left edge of the text content.
    rig.core
        .handle_input(InputEvent::CursorMoved(Vec2::new(17.0, 25.0)));
    rig.core.handle_input(InputEvent::mouse_down(1));
    rig.core.handle_input(InputEvent::mouse_up());
    assert!(rig.core.is_focused(rig.key), "click should focus");

    // Caret near the start: typing lands before most of the text.
    rig.type_str("X");
    let t = rig.text();
    assert!(
        t.starts_with('X') || t.starts_with("mX"),
        "caret should be near start, got {t}"
    );

    // Clicking outside any edit blurs.
    rig.frame();
    rig.core
        .handle_input(InputEvent::CursorMoved(Vec2::new(395.0, 295.0)));
    rig.core.handle_input(InputEvent::mouse_down(1));
    assert!(!rig.core.is_focused(rig.key), "click outside should blur");
}

#[test]
fn drag_selects_text() {
    let mut rig = Rig::new("hello world", false);
    rig.frame();
    // Press near start, drag to the right, release.
    rig.core
        .handle_input(InputEvent::CursorMoved(Vec2::new(16.0, 25.0)));
    rig.core.handle_input(InputEvent::mouse_down(1));
    rig.core
        .handle_input(InputEvent::CursorMoved(Vec2::new(120.0, 25.0)));
    rig.core.handle_input(InputEvent::mouse_up());
    let sel = rig.core.copy_selection();
    assert!(
        sel.as_deref().is_some_and(|s| s.len() > 2),
        "drag should select a few characters, got {sel:?}"
    );
}

#[test]
fn escape_blurs_and_set_text_replaces() {
    let mut rig = Rig::new("abc", false);
    rig.press(EditKey::Escape, Mods::default());
    assert!(!rig.core.is_focused(rig.key));
    rig.core.set_edit_text(rig.key, "replaced");
    assert_eq!(rig.text(), "replaced");
}

#[test]
fn selection_renders_highlight_and_caret() {
    let mut rig = Rig::new("select me", false);
    rig.press(EditKey::Home, Mods::default());
    for _ in 0..6 {
        rig.press(EditKey::Right, SHIFT);
    }
    rig.frame();
    let (dl, _) = rig.core.output();
    // Selection highlight: a solid quad wider than the 2px caret, translucent.
    let highlight = dl.quads.iter().any(|q| {
        q.kind == kui_core::QuadKind::Solid && q.rect.w > 10.0 && q.color.a < 0.9 && q.color.a > 0.1
    });
    assert!(highlight, "expected a selection highlight quad");
    // Caret: a 2px solid quad.
    let caret = dl
        .quads
        .iter()
        .any(|q| q.kind == kui_core::QuadKind::Solid && q.rect.w == 2.0);
    assert!(caret, "expected a caret quad");
}

#[test]
fn selection_highlight_stays_on_its_lines() {
    let mut rig = Rig::new("line one\nline two\nline three\nline four", true);
    // Cursor starts at buffer start; select a few chars on line two only.
    rig.press(EditKey::Down, Mods::default());
    for _ in 0..4 {
        rig.press(EditKey::Right, SHIFT);
    }
    rig.frame();
    let (dl, _) = rig.core.output();
    let ys: Vec<f32> = dl
        .quads
        .iter()
        .filter(|q| {
            q.kind == kui_core::QuadKind::Solid
                && q.rect.w > 10.0
                && q.color.a < 0.9
                && q.color.a > 0.1
        })
        .map(|q| q.rect.y)
        .collect();
    assert!(!ys.is_empty(), "expected a selection highlight");
    let (min_y, max_y) = ys
        .iter()
        .fold((f32::MAX, f32::MIN), |(lo, hi), y| (lo.min(*y), hi.max(*y)));
    // A one-line selection must highlight one line, not the rest of the doc.
    assert!(
        max_y - min_y < 1.0,
        "highlight leaked to other lines (quad tops span {min_y}..{max_y})"
    );
}
