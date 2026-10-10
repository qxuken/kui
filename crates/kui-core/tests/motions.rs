//! The editing modifiers as each platform's fields read them: on a Mac ⌥
//! moves by words and ⌥↑ ⌥↓ by paragraphs, ⌘ to the line's ends and ⌘↑ ⌘↓
//! to the text's; elsewhere Ctrl moves by words and Ctrl+↑ Ctrl+↓ by
//! paragraphs, Ctrl+Home and Ctrl+End to the text's ends. Each case puts a
//! `|` where the caret went.

use kui_core::{Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, Size, UiEvent};

const MAC: bool = cfg!(target_os = "macos");

fn rig(initial: &str, multiline: bool) -> (Core, Key) {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.text_edit(
        "f",
        initial,
        &EditOptions {
            multiline,
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().grow_width(),
    );
    ui.finish();
    (core, key)
}

fn press(core: &mut Core, k: EditKey, m: Mods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Key(k, m))
}

fn caret(core: &mut Core, key: Key) -> String {
    core.handle_input(InputEvent::Text("|".into()));
    core.edit_text(key).unwrap().replace('\n', "/")
}

/// The modifier that moves by words, and the one that goes to a line's
/// ends (none outside a Mac: Home and End do).
fn word() -> Mods {
    if MAC {
        Mods::NONE.with_word()
    } else {
        Mods::NONE.with_doc()
    }
}

fn field(from_start: bool, k: EditKey, m: Mods) -> String {
    let (mut core, key) = rig("one two three", false);
    if from_start {
        press(&mut core, EditKey::Home, Mods::NONE);
    }
    press(&mut core, k, m);
    caret(&mut core, key)
}

#[test]
fn a_field_moves_by_words_and_to_its_ends() {
    assert_eq!(field(false, EditKey::Left, word()), "one two |three");
    assert_eq!(field(true, EditKey::Right, word()), "one| two three");
    assert_eq!(field(false, EditKey::Backspace, word()), "one two |");
    // ↑ and ↓ by paragraphs: a field is one, so its ends.
    assert_eq!(field(false, EditKey::Up, word()), "|one two three");
    assert_eq!(field(true, EditKey::Down, word()), "one two three|");
    if MAC {
        let cmd = Mods::NONE.with_doc();
        assert_eq!(field(false, EditKey::Left, cmd), "|one two three");
        assert_eq!(field(true, EditKey::Right, cmd), "one two three|");
        assert_eq!(field(false, EditKey::Up, cmd), "|one two three");
        assert_eq!(field(true, EditKey::Down, cmd), "one two three|");
    } else {
        // Alt is a word too, as it was before Ctrl was.
        assert_eq!(
            field(false, EditKey::Left, Mods::NONE.with_word()),
            "one two |three"
        );
        let ctrl = Mods::NONE.with_doc();
        assert_eq!(field(false, EditKey::Home, ctrl), "|one two three");
        assert_eq!(field(true, EditKey::End, ctrl), "one two three|");
    }
}

/// A document, the caret after "three" on its middle line.
fn doc() -> (Core, Key) {
    let (mut core, key) = rig("one two\nthree four\nfive six", true);
    press(&mut core, EditKey::Down, Mods::NONE);
    for _ in 0..5 {
        press(&mut core, EditKey::Right, Mods::NONE);
    }
    (core, key)
}

#[test]
fn a_document_moves_by_paragraphs_and_on_past_one_it_is_at_the_end_of() {
    let (mut core, key) = doc();
    press(&mut core, EditKey::Up, word());
    assert_eq!(caret(&mut core, key), "one two/|three four/five six");
    let (mut core, key) = doc();
    press(&mut core, EditKey::Down, word());
    assert_eq!(caret(&mut core, key), "one two/three four|/five six");
    // Again from a paragraph's start: the one before's.
    let (mut core, key) = doc();
    press(&mut core, EditKey::Up, word());
    press(&mut core, EditKey::Up, word());
    assert_eq!(caret(&mut core, key), "|one two/three four/five six");
    let (mut core, key) = doc();
    press(&mut core, EditKey::Down, word());
    press(&mut core, EditKey::Down, word());
    assert_eq!(caret(&mut core, key), "one two/three four/five six|");
    // With Shift, the paragraph is selected: typing replaces it.
    let (mut core, key) = doc();
    press(&mut core, EditKey::Up, word());
    press(&mut core, EditKey::Down, word().with_shift());
    assert_eq!(caret(&mut core, key), "one two/|/five six");
    if MAC {
        let (mut core, key) = doc();
        press(&mut core, EditKey::Left, Mods::NONE.with_doc());
        assert_eq!(caret(&mut core, key), "one two/|three four/five six");
        let (mut core, key) = doc();
        press(&mut core, EditKey::Up, Mods::NONE.with_doc());
        assert_eq!(caret(&mut core, key), "|one two/three four/five six");
    }
}

#[test]
fn by_paragraphs_the_edge_is_only_where_the_caret_could_not_move() {
    let kinds = |evs: Vec<UiEvent>| {
        evs.iter()
            .filter_map(|e| e.kind().map(str::to_string))
            .collect::<Vec<_>>()
    };
    let (mut core, _) = rig("one two", false);
    // From the end: to the start, no edge yet; again: the edge.
    assert!(kinds(press(&mut core, EditKey::Up, word())).is_empty());
    assert_eq!(kinds(press(&mut core, EditKey::Up, word())), ["boundary"]);
    // A plain ↑ in a field is the edge at once, as before (F154).
    press(&mut core, EditKey::End, Mods::NONE);
    assert_eq!(
        kinds(press(&mut core, EditKey::Up, Mods::NONE)),
        ["boundary"]
    );
}
