//! End-to-end text editing through a live `Core`: focus, typing, motion,
//! selection, deletion, click-to-position, and event emission.

use kui_core::{Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, Size, Vec2};

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
        NodeSpec::column().grow_width().pad(5.0),
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

/// A frame declaring no editor at all — the root and nothing else — for
/// the turn before a rename opens one.
fn empty_frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    ui.finish();
}

/// The key `frame` declares its editor under. Keys are paths, so this one
/// is spellable before anything declares it — which is the whole point of
/// setting an editor's text from the `update` that opens it.
fn field_key() -> Key {
    Key::ROOT.str("field")
}

#[test]
fn set_text_before_the_declare_seeds_the_editor() {
    // The `update` that opens a rename field runs a frame ahead of the
    // view that declares it, so `set_edit_text` from it names a key with
    // no editor behind it yet. The text is held for the frame that
    // declares the key and seeds it there, over `initial` (backlog F25).
    let mut core = Core::new();
    empty_frame(&mut core);
    core.set_edit_text(field_key(), "seeded");
    let key = frame(&mut core, "initial", false);
    assert_eq!(key, field_key(), "the key is spellable before the declare");
    assert_eq!(core.edit_text(key).unwrap(), "seeded");
    let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(
        !codes.contains(&"edit-text-without-editor"),
        "a claimed seed is not a warning: {codes:?}"
    );
    // And the caret is where the call leaves it: at the end, so typing
    // extends the seeded name instead of prepending to it.
    core.handle_input(InputEvent::Text("!".into()));
    assert_eq!(core.edit_text(key).unwrap(), "seeded!");
}

#[test]
fn a_seeded_document_opens_at_its_end_not_its_top() {
    // `initial` on a multiline editor opens at the top, as a text view
    // does. A held `set_edit_text` is not `initial` — it is that call
    // arriving where it can land — so it leaves the caret where the call
    // does, at the end, document or not.
    let mut core = Core::new();
    empty_frame(&mut core);
    core.set_edit_text(field_key(), "first line");
    let key = frame(&mut core, "ignored", true);
    core.handle_input(InputEvent::Text("\nsecond line".into()));
    assert_eq!(core.edit_text(key).unwrap(), "first line\nsecond line");
}

#[test]
fn a_seed_nobody_declares_is_dropped_with_a_warning() {
    // Held for the next frame, not for ever: a key no view draws is the
    // call with its view half missing, and it says so rather than sitting
    // in the store waiting to surprise a later frame.
    let mut core = Core::new();
    core.set_edit_text(field_key(), "nowhere");
    empty_frame(&mut core);
    let warnings = core.take_warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].code, "edit-text-without-editor");
    assert_eq!(warnings[0].key, field_key());
    // Dropped, so the frame that does declare the key gets `initial`.
    let key = frame(&mut core, "initial", false);
    assert_eq!(core.edit_text(key).unwrap(), "initial");
}

#[test]
fn set_text_by_label_before_the_declare_seeds_the_editor() {
    // The key an `update` would need comes from an event the node fired,
    // and an editor being opened for the first time has fired none — so
    // the name the view itself declares is the only one the call can use
    // (backlog F32). Held for the frame that declares it, like the key.
    let mut core = Core::new();
    empty_frame(&mut core);
    core.set_edit_text_by_label("field", "seeded");
    let key = frame(&mut core, "initial", false);
    assert_eq!(core.edit_text(key).unwrap(), "seeded");
    let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(
        !codes.contains(&"edit-text-without-editor"),
        "a claimed seed is not a warning: {codes:?}"
    );
    // The caret is where the call leaves it, as it is for a seed by key.
    core.handle_input(InputEvent::Text("!".into()));
    assert_eq!(core.edit_text(key).unwrap(), "seeded!");
}

#[test]
fn set_text_by_label_resets_a_returning_editor() {
    // The case a seed by key cannot reach. A draft is abandoned, the view
    // stops declaring the editor, and its state is retained under a key
    // that is off screen (backlog F20, F26) — which is also why `key_of`
    // will not resolve the label: no recent frame declared it. The
    // `update` that reopens the field sets the model's text by label, and
    // the editor that comes back has to show that and not the draft, so
    // the claim reaches an existing state with `set_text` where a new one
    // takes a seed.
    let mut core = Core::new();
    let key = frame(&mut core, "name", false);
    core.handle_input(InputEvent::Text(" edited".into()));
    assert_eq!(core.edit_text(key).unwrap(), "name edited");
    empty_frame(&mut core);
    assert_eq!(
        core.key_of("field"),
        None,
        "an editor closed for a frame is in no frame `key_of` reads"
    );
    core.set_edit_text_by_label("field", "name");
    let key = frame(&mut core, "name", false);
    assert_eq!(
        core.edit_text(key).unwrap(),
        "name",
        "the reopened editor shows the model's text, not the abandoned draft"
    );
    let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(!codes.contains(&"edit-text-without-editor"), "{codes:?}");
}

#[test]
fn a_label_nobody_declares_is_dropped_with_a_warning() {
    // The key path's rule, in the other spelling: held for the next
    // frame, not for ever, and the line says which name went unclaimed so
    // its reader looks at the right half of the call.
    let mut core = Core::new();
    core.set_edit_text_by_label("filed", "nowhere");
    empty_frame(&mut core);
    let warnings = core.take_warnings();
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(warnings[0].code, "edit-text-without-editor");
    assert_eq!(warnings[0].key, Key::ROOT.str("filed"));
    assert!(
        warnings[0].message.contains("\"filed\""),
        "the label is in the message: {}",
        warnings[0].message
    );
    // Dropped, so the frame that declares the right label gets `initial`.
    let key = frame(&mut core, "initial", false);
    assert_eq!(core.edit_text(key).unwrap(), "initial");
}

/// Why the label warning is keyed `Key::ROOT.str(label)` and not one
/// constant: the dedup is once per (code, key), so a constant key would
/// report the first unclaimed name in a frame and swallow the rest — and
/// two names spelt wrong is exactly the frame where a reader needs both.
#[test]
fn two_unclaimed_labels_in_one_frame_are_two_warnings() {
    let mut core = Core::new();
    core.set_edit_text_by_label("filed", "one");
    core.set_edit_text_by_label("feild", "two");
    empty_frame(&mut core);
    let mut named: Vec<String> = core
        .take_warnings()
        .into_iter()
        .filter(|w| w.code == "edit-text-without-editor")
        .map(|w| w.message)
        .collect();
    named.sort();
    assert_eq!(named.len(), 2, "{named:?}");
    assert!(named[0].contains("\"feild\""), "{named:?}");
    assert!(named[1].contains("\"filed\""), "{named:?}");
}

#[test]
fn set_text_by_a_declared_label_lands_at_once() {
    // A label the last frame declared resolves now, so this is
    // `set_edit_text` on that key with nothing held and nothing deferred.
    let mut core = Core::new();
    let key = frame(&mut core, "name", false);
    core.set_edit_text_by_label("field", "renamed");
    assert_eq!(core.edit_text(key).unwrap(), "renamed");
    let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(
        !codes.contains(&"edit-text-without-editor"),
        "nothing was held: {codes:?}"
    );
}

/// The call says whether the text landed or was held, in both spellings.
/// A binding redraws on the first and not the second: a redraw on a held
/// seed re-lowers the retained tree — the one that declares no editor —
/// and that frame is where the hold expires, so a `dispatch` made outside
/// the loop lost its seed to the redraw it asked for (backlog F42).
#[test]
fn set_text_says_whether_it_landed_or_was_held() {
    let mut core = Core::new();
    empty_frame(&mut core);
    assert!(
        !core.set_edit_text(field_key(), "held"),
        "a key nothing declared is held, not applied"
    );
    assert!(
        !core.set_edit_text_by_label("field", "held"),
        "a label nothing declared is held, not applied"
    );
    let key = frame(&mut core, "initial", false);
    assert!(
        core.set_edit_text(key, "landed"),
        "a declared key takes it now"
    );
    assert!(
        core.set_edit_text_by_label("field", "landed"),
        "a declared label resolves and takes it now"
    );
    assert_eq!(core.edit_text(key).unwrap(), "landed");
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
    assert!(rig.core.take_pending_events().is_empty());
    assert_eq!(rig.core.cut_selection().as_deref(), Some("hello"));
    assert_eq!(rig.text(), " world");
    // AR15: the cut's `changed` is pending, for the driver to route the
    // way it routes a `resize` — it used to build one by hand, and only
    // for its own chord.
    let evs = rig.core.take_pending_events();
    assert_eq!(evs.len(), 1, "{evs:?}");
    assert_eq!(
        evs[0].payload.get("kind").and_then(kui_core::Value::as_str),
        Some("changed")
    );
    // Nothing selected: nothing cut, nothing posted.
    assert!(rig.core.cut_selection().is_none());
    assert!(rig.core.take_pending_events().is_empty());
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

/// An editor that lost the keyboard keeps its selection and stops drawing
/// it, and draws it again when focus comes back (backlog F147): a page of
/// fields shows one highlight, the one ⌘C would copy.
#[test]
fn an_unfocused_editor_keeps_its_selection_and_hides_it() {
    let highlighted = |rig: &mut Rig| {
        rig.frame();
        let (dl, _) = rig.core.output();
        // In the field's band at the top of the window, not a menu's row.
        dl.quads.iter().any(|q| {
            q.kind == kui_core::QuadKind::Solid
                && q.rect.w > 10.0
                && q.rect.y < 60.0
                && q.color.a < 0.9
                && q.color.a > 0.1
        })
    };
    let mut rig = Rig::new("select me", false);
    rig.press(EditKey::SelectAll, Mods::default());
    assert!(highlighted(&mut rig), "focused: the selection shows");
    rig.core.set_focus(None);
    assert!(!highlighted(&mut rig), "blurred: no highlight");
    assert!(rig.core.edit.has_selection(rig.key), "the range is kept");
    rig.core.set_focus(Some(rig.key));
    assert!(highlighted(&mut rig), "focus back: the selection with it");
    // A menu opened over the field takes focus for its rows, and the
    // selection it would copy stays in sight while it is up.
    rig.core.open_menu(kui_core::Menu::new(
        rig.key,
        Vec2::new(20.0, 150.0),
        vec![kui_core::MenuItem::role(kui_core::MenuRole::Copy)],
    ));
    rig.frame();
    rig.core
        .handle_input(InputEvent::Key(EditKey::Down, Mods::default()));
    assert_ne!(
        rig.core.edit.focused(),
        Some(rig.key),
        "the menu's row has focus"
    );
    assert!(highlighted(&mut rig), "the menu's selection stays drawn");
}
