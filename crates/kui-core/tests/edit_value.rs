//! An editor's declared value (`EditOptions::value`, ADR 0047 decision
//! 8): on every frame it differs from the editor's text, the editor is
//! brought to it as a splice, so a caret before the span stays, one after
//! it moves with the text and one inside it goes to the end of what came
//! in. A splice on the frame after the user's own edit joins that edit's
//! undo step; any other is a step of its own. While an input method
//! composes the value waits, and an equal value touches nothing.

use kui_core::{Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, Size};

fn opts() -> EditOptions {
    EditOptions {
        autofocus: true,
        ..Default::default()
    }
}

/// One frame declaring a focused field keyed "field" whose value is
/// `value`; its key.
fn frame(core: &mut Core, value: &str, opts: &EditOptions) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.text_edit_value("field", value, opts, NodeSpec::column().grow_width());
    ui.finish();
    key
}

fn text(core: &Core, key: Key) -> String {
    core.edit_text(key).unwrap()
}

/// The caret as a byte offset into the text, and the selection.
fn caret(core: &Core, key: Key) -> (usize, Option<(usize, usize)>) {
    core.edit.caret_and_selection(key).unwrap()
}

fn put_caret(core: &mut Core, key: Key, at: usize) {
    core.edit.set_selection(key, (0, at), (0, at));
}

fn type_str(core: &mut Core, s: &str) {
    for ch in s.chars() {
        core.handle_input(InputEvent::Text(ch.to_string()));
    }
}

fn press(core: &mut Core, k: EditKey) {
    core.handle_input(InputEvent::Key(k, Mods::NONE));
}

#[test]
fn an_equal_value_touches_nothing() {
    let mut core = Core::new();
    let o = opts();
    let key = frame(&mut core, "hello", &o);
    type_str(&mut core, " w");
    let version = core.edit.version(key);
    let before = caret(&core, key);
    frame(&mut core, "hello w", &o);
    assert_eq!(text(&core, key), "hello w");
    assert_eq!(core.edit.version(key), version, "no edit was made");
    assert_eq!(caret(&core, key), before);
    assert_eq!(core.edit.history(key), (true, false));
    // The burst is still one unit: the declaration broke no coalescing.
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "hello");
}

#[test]
fn a_caret_before_the_span_stays_and_one_after_it_moves() {
    let mut core = Core::new();
    let o = opts();
    let key = frame(&mut core, "hello world", &o);
    put_caret(&mut core, key, 2);
    frame(&mut core, "hello big world", &o);
    assert_eq!(text(&core, key), "hello big world");
    assert_eq!(caret(&core, key), (2, None), "before the span: stays");

    // "hello big wor|ld" to "hi big wor|ld": "ello" became "i", so the
    // caret after it moves back by three.
    put_caret(&mut core, key, 13);
    frame(&mut core, "hi big world", &o);
    assert_eq!(text(&core, key), "hi big world");
    assert_eq!(caret(&core, key), (10, None), "after the span: moved");
}

#[test]
fn a_caret_inside_the_span_goes_to_its_end() {
    let mut core = Core::new();
    let o = opts();
    let key = frame(&mut core, "abcdef", &o);
    put_caret(&mut core, key, 3);
    frame(&mut core, "abXYZef", &o);
    assert_eq!(text(&core, key), "abXYZef");
    assert_eq!(caret(&core, key), (5, None), "the end of what came in");
}

#[test]
fn a_selection_over_the_span_collapses_and_one_clear_of_it_moves() {
    let mut core = Core::new();
    let o = opts();
    let key = frame(&mut core, "one two three", &o);
    // "one [two] three": the span is "two", so the selection goes.
    core.edit.set_selection(key, (0, 4), (0, 7));
    frame(&mut core, "one 2 three", &o);
    assert_eq!(text(&core, key), "one 2 three");
    assert_eq!(caret(&core, key), (5, None));

    // "one 2 [three]": a span before it moves the selection, kept.
    core.edit.set_selection(key, (0, 6), (0, 11));
    frame(&mut core, "1 2 three", &o);
    assert_eq!(text(&core, key), "1 2 three");
    assert_eq!(caret(&core, key), (9, Some((4, 9))));
}

#[test]
fn the_span_ends_on_grapheme_boundaries() {
    let mut core = Core::new();
    let o = opts();
    let key = frame(&mut core, "cafe", &o);
    // An accent after the e: the byte prefix is "cafe", but the e is half
    // a grapheme of the new text, so the span is the e and the é.
    frame(&mut core, "cafe\u{301}", &o);
    assert_eq!(text(&core, key), "cafe\u{301}");
    assert_eq!(caret(&core, key), (6, None));
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "cafe");
}

/// The span is found a block of bytes at a time from each end, then byte
/// by byte: a change far from both ends, in a document of several lines,
/// lands where it is and leaves the caret on its line.
#[test]
fn a_splice_far_from_both_ends_lands_where_it_is() {
    let mut core = Core::new();
    let o = EditOptions {
        multiline: true,
        ..opts()
    };
    let line = "x".repeat(99);
    let old = format!("{line}\n{line}a{line}\n{line}");
    let key = frame(&mut core, &old, &o);
    // The caret on the last line, after the span.
    core.edit.set_selection(key, (2, 10), (2, 10));
    let new = old.replacen('a', "bc", 1);
    frame(&mut core, &new, &o);
    assert_eq!(text(&core, key), new);
    assert_eq!(core.edit.selection_cursors(key), Some(((2, 10), (2, 10))));
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), old);
}

/// The filter ADR 0047 names: the app uppercases what the user typed and
/// declares it. The correction joins the keystroke's undo step, so one
/// undo takes both back — not to the `a`, which the filter would make an
/// `A` again.
#[test]
fn a_filters_correction_undoes_with_the_keystroke() {
    let mut core = Core::new();
    let o = opts();
    let mut model = String::new();
    let key = frame(&mut core, &model, &o);
    type_str(&mut core, "a");
    model = text(&core, key).to_uppercase();
    frame(&mut core, &model, &o);
    assert_eq!(text(&core, key), "A");
    assert_eq!(caret(&core, key), (1, None));

    press(&mut core, EditKey::Undo);
    assert_eq!(
        text(&core, key),
        "",
        "one undo: the keystroke and the correction"
    );
    model = text(&core, key).to_uppercase();
    frame(&mut core, &model, &o);
    assert_eq!(text(&core, key), "");

    press(&mut core, EditKey::Redo);
    assert_eq!(text(&core, key), "A", "one redo brings both back");
    assert_eq!(core.edit.history(key), (true, false));

    // A keystroke after a correction is not typed into it: each corrected
    // keystroke is a step.
    type_str(&mut core, "b");
    model = text(&core, key).to_uppercase();
    frame(&mut core, &model, &o);
    assert_eq!(text(&core, key), "AB");
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "A");
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "");
}

#[test]
fn a_model_reset_is_its_own_undo_step() {
    let mut core = Core::new();
    let o = opts();
    let mut model = String::new();
    let key = frame(&mut core, &model, &o);
    for ch in ["a", "b", "c"] {
        type_str(&mut core, ch);
        model = text(&core, key);
        frame(&mut core, &model, &o);
    }
    // A frame with no edit before it: the model clears the field.
    frame(&mut core, "", &o);
    assert_eq!(text(&core, key), "");
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "abc", "the reset undoes alone");
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "");
}

#[test]
fn a_value_waits_for_the_composition_and_lands_at_its_commit() {
    let mut core = Core::new();
    let o = opts();
    let key = frame(&mut core, "", &o);
    core.handle_input(InputEvent::Preedit("か".into(), Some((3, 3))));
    // The model moves while the user composes: held, and the composition
    // is not cancelled by it.
    frame(&mut core, "X", &o);
    assert_eq!(text(&core, key), "");
    assert_eq!(core.edit.preedit(key), Some("か"));
    frame(&mut core, "X", &o);
    assert_eq!(core.edit.preedit(key), Some("か"));

    core.handle_input(InputEvent::Commit("か".into()));
    assert_eq!(text(&core, key), "か");
    frame(&mut core, "X", &o);
    assert_eq!(
        text(&core, key),
        "X",
        "applied on the frame after the commit"
    );
    assert_eq!(caret(&core, key), (1, None));
    // The value followed the user's commit, so it joined its step.
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "");
}

/// A commit, then a composition begun before the next frame — what
/// continuous typing through an input method is — then the composition
/// abandoned: the value the model declared for the commit lands on the
/// frame after, and is still the commit's correction, undone with it.
#[test]
fn a_value_held_past_a_second_composition_still_joins_the_commit() {
    let mut core = Core::new();
    let o = opts();
    let key = frame(&mut core, "", &o);
    core.handle_input(InputEvent::Preedit("か".into(), Some((3, 3))));
    core.handle_input(InputEvent::Commit("か".into()));
    core.handle_input(InputEvent::Preedit("き".into(), Some((3, 3))));
    frame(&mut core, "カ", &o);
    assert_eq!(text(&core, key), "か", "held for the composition");
    core.handle_input(InputEvent::Preedit(String::new(), None));
    frame(&mut core, "カ", &o);
    assert_eq!(text(&core, key), "カ");
    press(&mut core, EditKey::Undo);
    assert_eq!(
        text(&core, key),
        "",
        "the commit and its correction, one step"
    );
}

#[test]
fn a_secure_editors_value_is_its_secret() {
    let mut core = Core::new();
    let o = EditOptions {
        secure: true,
        ..opts()
    };
    let key = frame(&mut core, "abc", &o);
    assert_eq!(text(&core, key), "abc");
    let bullet = kui_core::SECURE_BULLET.len_utf8();
    assert_eq!(caret(&core, key), (3 * bullet, None));
    frame(&mut core, "abc", &o);
    assert_eq!(caret(&core, key), (3 * bullet, None));

    // "ab|c": a grapheme inserted before the caret moves it a bullet on.
    put_caret(&mut core, key, 2 * bullet);
    frame(&mut core, "aXbc", &o);
    assert_eq!(text(&core, key), "aXbc");
    assert_eq!(caret(&core, key), (3 * bullet, None));
    let shown = core.access_tree().get(key).unwrap().value.clone();
    assert_eq!(
        shown.as_deref(),
        Some("••••"),
        "the bullets are the new text's"
    );
    assert_eq!(
        core.edit.history(key),
        (false, false),
        "no history, as before"
    );
}

/// A multiline value whose line breaks are `\r\n` is the text the buffer
/// holds with `\n`: equal, so it is not spliced again on every frame.
#[test]
fn a_value_that_differs_only_in_its_line_breaks_is_equal() {
    let mut core = Core::new();
    let o = EditOptions {
        multiline: true,
        ..opts()
    };
    let key = frame(&mut core, "a\r\nb", &o);
    assert_eq!(text(&core, key), "a\nb");
    let version = core.edit.version(key);
    frame(&mut core, "a\r\nb", &o);
    assert_eq!(core.edit.version(key), version);
}

/// The sync ADR 0047 pins: an app that stores the editor's text after
/// every input and declares it back as the value. After every frame the
/// editor's text is the model, the caret is where typing left it, and the
/// history still reaches the first keystroke.
#[test]
fn an_app_that_declares_back_what_was_typed_never_fights_it() {
    let mut core = Core::new();
    let o = opts();
    let mut model = String::new();
    let key = frame(&mut core, &model, &o);
    for ch in "hello world".chars() {
        core.handle_input(InputEvent::Text(ch.to_string()));
        model = text(&core, key);
        let before = caret(&core, key);
        let version = core.edit.version(key);
        frame(&mut core, &model, &o);
        assert_eq!(text(&core, key), model);
        assert_eq!(caret(&core, key), before, "after {ch:?}");
        assert_eq!(core.edit.version(key), version);
    }
    press(&mut core, EditKey::Left);
    press(&mut core, EditKey::Backspace);
    model = text(&core, key);
    frame(&mut core, &model, &o);
    assert_eq!(model, "hello word");
    let mut undos = 0;
    while core.edit.history(key).0 {
        press(&mut core, EditKey::Undo);
        model = text(&core, key);
        frame(&mut core, &model, &o);
        undos += 1;
    }
    assert_eq!(text(&core, key), "", "back to before the first keystroke");
    assert_eq!(undos, 2, "the typing burst and the Backspace");
}

/// A value with no `onChange` is a field that reverts every keystroke, and
/// the core says so; one with the row is the loop working as meant.
#[test]
fn a_value_without_on_change_warns() {
    let mut core = Core::new();
    let o = opts();
    frame(&mut core, "", &o);
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(
        codes.contains(&kui_core::diag::EDIT_VALUE_WITHOUT_HANDLER),
        "{codes:?}"
    );
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text_edit_value(
        "field",
        "",
        &o,
        NodeSpec::column().on_change(kui_core::Value::Null),
    );
    ui.finish();
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(
        !codes.contains(&kui_core::diag::EDIT_VALUE_WITHOUT_HANDLER),
        "{codes:?}"
    );
}

/// The loop an app runs (ADR 0047): the editor's `change` carries its
/// text, the app stores it, and the next frame declares it back as the
/// value. Driven from the event's own text — not from `edit_text` — the
/// model and the editor agree after every frame, the caret stays where
/// the typing put it, and one undo per word reaches back to empty.
#[test]
fn the_change_event_s_text_declared_back_keeps_the_editor_in_sync() {
    let mut core = Core::new();
    let o = opts();
    let declare = |core: &mut Core, model: &str| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let spec = NodeSpec::column()
            .grow_width()
            .on_change(kui_core::Value::map([(
                "kind",
                kui_core::Value::str("name"),
            )]));
        let key = ui.text_edit_value("field", model, &o, spec);
        ui.finish();
        key
    };
    let mut model = String::new();
    let key = declare(&mut core, &model);
    for ch in "hi there".chars() {
        for ev in core.handle_input(InputEvent::Text(ch.to_string())) {
            assert_eq!(
                ev.payload.get_str("kind"),
                Some("name"),
                "the declared message"
            );
            let change = ev.read();
            let kui_core::Event::EditChange(c) = change else {
                panic!("an edit's change, got {change:?}");
            };
            model = c.text;
        }
        let caret_before = caret(&core, key);
        declare(&mut core, &model);
        assert_eq!(text(&core, key), model, "after {ch:?}");
        assert_eq!(
            caret(&core, key),
            caret_before,
            "the caret stays after {ch:?}"
        );
    }
    assert_eq!(model, "hi there");
    let mut undos = 0;
    while !text(&core, key).is_empty() && undos < 10 {
        for ev in core.handle_input(InputEvent::Key(EditKey::Undo, Mods::NONE)) {
            if let kui_core::Event::EditChange(c) = ev.read() {
                model = c.text;
            }
        }
        declare(&mut core, &model);
        undos += 1;
    }
    assert_eq!(text(&core, key), "");
    assert!(undos <= 2, "typing undoes by the word, took {undos}");
}

/// A reader's `setValue` is an edit of the user's, recorded as typing is:
/// the history typed before it survives, it undoes in one step, and the
/// app's correction in the `update` it reached joins that step — where it
/// went through `set_edit_text` and cleared the history, so a filter's
/// correction was a step of its own and the typing before was gone.
#[test]
fn a_readers_set_value_undoes_like_typing() {
    let mut core = Core::new();
    let o = opts();
    let declare = |core: &mut Core, model: &str| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let key = ui.text_edit_value(
            "field",
            model,
            &o,
            NodeSpec::column().on_change(kui_core::Value::Null),
        );
        ui.finish();
        key
    };
    let mut model = String::new();
    let key = declare(&mut core, &model);
    // Typed first, and stored as the app stores it.
    type_str(&mut core, "ab");
    model = text(&core, key);
    declare(&mut core, &model);
    // A reader sets the value; the app upper-cases what it hears.
    let mut req = kui_core::AccessRequest::new(key, kui_core::AccessAction::SetValue);
    req.value = Some("hello".into());
    let evs = core.handle_input(InputEvent::Access(req));
    let change = evs
        .iter()
        .find(|e| e.is(kui_core::EventKind::Change))
        .expect("the reader's edit is a change");
    let kui_core::Event::EditChange(c) = change.read() else {
        panic!("an editor's change");
    };
    assert_eq!(c.text, "hello");
    model = c.text.to_uppercase();
    declare(&mut core, &model);
    assert_eq!(text(&core, key), "HELLO", "the correction applied");
    // One undo takes back the reader's edit and the correction together,
    // and the typing before it is still there to undo.
    press(&mut core, EditKey::Undo);
    assert_eq!(
        text(&core, key),
        "ab",
        "the reader's edit undone in one step"
    );
    press(&mut core, EditKey::Undo);
    assert_eq!(text(&core, key), "", "the typing before it survived");
}
