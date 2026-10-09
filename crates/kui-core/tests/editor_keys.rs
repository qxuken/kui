//! What a focused editor keeps and what it lets go of
//! (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, amended for
//! backlog F144–F146): a chord the editor does not act on reaches the
//! sink above it, a key that meets the edge of the text says so, and a
//! field declared `keep_tab` hands its Tab to the app.

use kui_core::{
    Core, EditKey, EditOptions, InputEvent, Key, KeyCode, KeyMods, KeyPress, Mods, NodeSpec, Size,
    UiEvent, Value, Vec2,
};

/// A shell sink around one editor (and a button after it, for Tab to
/// reach), or the editor alone with `shell: false`.
fn frame(core: &mut Core, opts: &EditOptions, shell: bool) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut field = Key::ROOT;
    let body = |ui: &mut kui_core::Ui<'_>, field: &mut Key| {
        *field = ui.text_edit(
            "field",
            "ab",
            &EditOptions {
                autofocus: true,
                ..opts.clone()
            },
            NodeSpec::column().size(200.0, 40.0),
        );
        ui.leaf_keyed(
            "after",
            NodeSpec::row()
                .size(100.0, 20.0)
                .on_click("after")
                .label("After"),
        );
    };
    let shell_key = if shell {
        ui.with_keyed(
            "shell",
            NodeSpec::column().fill().on_key("shell").key_up(),
            |ui| body(ui, &mut field),
        )
    } else {
        body(&mut ui, &mut field);
        Key::ROOT
    };
    ui.finish();
    (shell_key, field)
}

fn rig(opts: EditOptions, shell: bool) -> (Core, Key, Key) {
    let mut core = Core::new();
    let (s, f) = frame(&mut core, &opts, shell);
    frame(&mut core, &opts, shell);
    assert_eq!(core.focus(), Some(f), "the field took focus");
    (core, s, f)
}

fn primary() -> KeyMods {
    if cfg!(target_os = "macos") {
        KeyMods {
            super_key: true,
            ..KeyMods::default()
        }
    } else {
        KeyMods {
            ctrl: true,
            ..KeyMods::default()
        }
    }
}

/// One press as a window sends it: the raw press, then the editing key it
/// maps to, then what it types — `KeyPress::edit_event`'s answer for the
/// second, as the runner's.
fn press(core: &mut Core, code: KeyCode, mods: KeyMods, text: Option<&str>) -> Vec<UiEvent> {
    let kp = KeyPress {
        text: text.map(str::to_string),
        ..KeyPress::new(code, mods)
    };
    let mut out = core.handle_input(InputEvent::KeyDown(kp.clone()));
    if let Some(ev) = kp.edit_event() {
        out.extend(core.handle_input(ev));
    }
    out.extend(core.handle_input(InputEvent::KeyUp(kp.released())));
    out
}

fn heard(evs: &[UiEvent], sink: Key) -> Vec<(String, String)> {
    evs.iter()
        .filter(|e| e.key == sink && e.kind() == Some("key"))
        .map(|e| {
            let at = |k: &str| e.payload.get_str(k).unwrap_or("").to_string();
            (at("phase"), at("code"))
        })
        .collect()
}

fn boundary(evs: &[UiEvent], field: Key) -> Option<(String, String)> {
    let e = evs
        .iter()
        .find(|e| e.key == field && e.kind() == Some("boundary"))?;
    let at = |k: &str| e.payload.get_str(k).unwrap_or("").to_string();
    Some((at("key"), at("edge")))
}

fn edit(core: &mut Core, ek: EditKey, mods: Mods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Key(ek, mods))
}

const NONE: Mods = Mods {
    shift: false,
    word: false,
    doc: false,
};

#[test]
fn a_chord_the_editor_does_not_act_on_reaches_the_sink_above_it() {
    let (mut core, shell, field) = rig(EditOptions::default(), true);
    let evs = press(&mut core, KeyCode::Char('n'), primary(), None);
    assert_eq!(
        heard(&evs, shell),
        [("down".into(), "n".into()), ("up".into(), "n".into())],
        "the press and its release, on the shell"
    );
    assert_eq!(core.edit_text(field).as_deref(), Some("ab"));
    assert_eq!(core.focus(), Some(field), "focus stays in the field");
    // The sink's tag rides on it, as on any key a sink hears.
    let down = evs.iter().find(|e| e.key == shell).unwrap();
    assert_eq!(down.payload.get("tag"), Some(&Value::str("shell")));
    // Control alone is a chord on every platform.
    let ctrl = KeyMods {
        ctrl: true,
        ..KeyMods::default()
    };
    let evs = press(&mut core, KeyCode::Char('k'), ctrl, None);
    assert_eq!(heard(&evs, shell)[0], ("down".into(), "k".into()));
}

#[test]
fn the_editor_keeps_what_it_acts_on() {
    let (mut core, shell, field) = rig(EditOptions::default(), true);
    // The clipboard and undo chords a driver performs for it.
    for c in ['c', 'x', 'v', 'a', 'z', 'y'] {
        assert!(heard(&press(&mut core, KeyCode::Char(c), primary(), None), shell).is_empty());
    }
    let shifted = KeyMods {
        shift: true,
        ..primary()
    };
    assert!(heard(&press(&mut core, KeyCode::Char('Z'), shifted, None), shell).is_empty());
    // Typing, under Shift and under Option (a Mac's ø), and the editing
    // keys, chorded or not.
    let alt = KeyMods {
        alt: true,
        ..KeyMods::default()
    };
    let evs = [
        press(&mut core, KeyCode::Char('q'), KeyMods::default(), Some("q")),
        press(&mut core, KeyCode::Char('o'), alt, Some("ø")),
        press(&mut core, KeyCode::Left, primary(), None),
        press(&mut core, KeyCode::Backspace, alt, None),
        press(&mut core, KeyCode::Enter, primary(), None),
    ];
    for e in &evs {
        assert!(heard(e, shell).is_empty(), "{e:?}");
    }
    assert_eq!(core.focus(), Some(field));
}

#[test]
fn with_no_sink_above_a_chord_goes_nowhere() {
    let (mut core, _, field) = rig(EditOptions::default(), false);
    let evs = press(&mut core, KeyCode::Char('n'), primary(), None);
    assert!(evs.iter().all(|e| e.kind() != Some("key")));
    assert_eq!(core.edit_text(field).as_deref(), Some("ab"));
}

#[test]
fn a_key_at_the_edge_of_the_text_says_which_edge() {
    let (mut core, _, field) = rig(EditOptions::default(), true);
    // A field opens with the caret at the end.
    let at = |evs: &[UiEvent]| boundary(evs, field);
    assert_eq!(
        at(&edit(&mut core, EditKey::Right, NONE)),
        Some(("right".into(), "end".into()))
    );
    assert_eq!(
        at(&edit(&mut core, EditKey::Down, NONE)),
        Some(("down".into(), "end".into())),
        "a field has no line below"
    );
    assert_eq!(
        at(&edit(&mut core, EditKey::Delete, NONE)),
        Some(("delete".into(), "end".into()))
    );
    // A key that moved the caret is not at an edge.
    assert_eq!(at(&edit(&mut core, EditKey::Left, NONE)), None);
    assert_eq!(at(&edit(&mut core, EditKey::Left, NONE)), None);
    assert_eq!(
        at(&edit(&mut core, EditKey::Left, NONE)),
        Some(("left".into(), "start".into()))
    );
    assert_eq!(
        at(&edit(&mut core, EditKey::Up, NONE)),
        Some(("up".into(), "start".into()))
    );
    assert_eq!(
        at(&edit(&mut core, EditKey::Backspace, NONE)),
        Some(("backspace".into(), "start".into()))
    );
    assert_eq!(core.edit_text(field).as_deref(), Some("ab"));
    // Shift extends a selection: no edge to report.
    let shift = Mods {
        shift: true,
        ..NONE
    };
    assert_eq!(at(&edit(&mut core, EditKey::Left, shift)), None);
}

#[test]
fn a_selection_is_not_an_edge() {
    let (mut core, _, field) = rig(EditOptions::default(), true);
    // Select All leaves the caret at the end: → there drops the
    // selection, which is something done, and only then meets the edge.
    edit(&mut core, EditKey::SelectAll, NONE);
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Right, NONE), field),
        None
    );
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Right, NONE), field),
        Some(("right".into(), "end".into()))
    );
}

#[test]
fn a_document_reports_its_first_and_last_lines() {
    let opts = EditOptions {
        multiline: true,
        ..EditOptions::default()
    };
    let mut core = Core::new();
    let f = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let k = ui.text_edit(
            "doc",
            "one\ntwo",
            &EditOptions {
                autofocus: true,
                ..opts.clone()
            },
            NodeSpec::column().size(300.0, 100.0),
        );
        ui.finish();
        k
    };
    let doc = f(&mut core);
    f(&mut core);
    // A document opens at its top.
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Up, NONE), doc),
        Some(("up".into(), "start".into()))
    );
    assert_eq!(boundary(&edit(&mut core, EditKey::Down, NONE), doc), None);
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Down, NONE), doc),
        Some(("down".into(), "end".into()))
    );
}

#[test]
fn tab_walks_the_ring_from_a_field_unless_it_keeps_tab() {
    let (mut core, shell, _) = rig(EditOptions::default(), true);
    let evs = press(&mut core, KeyCode::Tab, KeyMods::default(), None);
    assert!(heard(&evs, shell).is_empty());
    assert_ne!(core.focus().map(|k| k.0), None);
    let after = core.focus();

    let keep = EditOptions {
        keep_tab: true,
        ..EditOptions::default()
    };
    let (mut core, shell, field) = rig(keep, true);
    let evs = press(&mut core, KeyCode::Tab, KeyMods::default(), None);
    assert_eq!(
        heard(&evs, shell),
        [("down".into(), "tab".into()), ("up".into(), "tab".into())]
    );
    assert_eq!(core.focus(), Some(field), "focus stayed in the field");
    assert_ne!(after, Some(field));
    let shift = KeyMods {
        shift: true,
        ..KeyMods::default()
    };
    let evs = press(&mut core, KeyCode::Tab, shift, None);
    assert_eq!(heard(&evs, shell)[0], ("down".into(), "tab".into()));
    assert_eq!(core.focus(), Some(field));
    assert_eq!(core.edit_text(field).as_deref(), Some("ab"), "no tab typed");
}

/// ↓ from the middle of a field's one line is the field's end at once: the
/// key leaves the caret on its visual line, wherever along it, and that is
/// the edge (backlog F154). The same for ↑ on a document's first line.
#[test]
fn up_and_down_on_the_edge_line_report_wherever_the_caret_is() {
    let (mut core, _, field) = rig(EditOptions::default(), true);
    edit(&mut core, EditKey::Left, NONE);
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Down, NONE), field),
        Some(("down".into(), "end".into())),
        "the first ↓ from the middle"
    );
    edit(&mut core, EditKey::Home, NONE);
    edit(&mut core, EditKey::Right, NONE);
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Up, NONE), field),
        Some(("up".into(), "start".into()))
    );
}

/// An editor's caret in window coordinates, and a caret put back by a
/// point: what carries a column from a field to the app's own text and
/// back (backlog F154).
#[test]
fn an_editors_caret_is_readable_and_placeable_by_point() {
    let (mut core, _, field) = rig(EditOptions::default(), true);
    // "ab", the caret at the end.
    let end = core.edit_caret_rect(field).expect("a caret");
    edit(&mut core, EditKey::Home, NONE);
    let start = core.edit_caret_rect(field).expect("a caret");
    assert!(end.x > start.x && (end.y - start.y).abs() < 0.5);
    assert!(end.h > 0.0);
    // Back to the end by its point, then to the start by a point left of it.
    assert!(core.edit_caret_to(field, Vec2::new(end.x + 50.0, end.y + 2.0)));
    assert_eq!(core.edit.caret_and_selection(field), Some((2, None)));
    assert!(core.edit_caret_to(field, Vec2::new(start.x - 30.0, start.y + 2.0)));
    assert_eq!(core.edit.caret_and_selection(field), Some((0, None)));
    assert_eq!(core.focus(), Some(field));
    assert!(!core.edit_caret_to(Key::ROOT, Vec2::new(0.0, 0.0)));
}
