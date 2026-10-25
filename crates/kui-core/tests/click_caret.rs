//! A click in an editor leaves a caret, not an empty selection (backlog
//! F170, from Carnet's title field). The release, and every frame and
//! every move of the pointer between the press and it, placed the caret
//! drag's live end again where the pointer was, and cosmic-text's drag
//! seeds a selection at the caret when there is none: a click left the
//! field holding a selection from the caret to itself. A field with a
//! selection reports no edge and has Backspace take the selection, so
//! the first Backspace, ←, → or ↓ after a click did nothing.
//!
//! Now a caret drag starts once the pointer leaves the click slop, as an
//! `on_drag` node's does, and a drag back to its anchor holds no
//! selection. A drag still selects, a second click the word, a third the
//! line, and a Shift-click extends from the caret.

use kui_core::{
    Core, EditKey, EditOptions, EventKind, InputEvent, Key, Mods, NodeSpec, Size, UiEvent, Value,
    Vec2,
};

const NONE: Mods = Mods {
    shift: false,
    word: false,
    doc: false,
    ctrl: false,
};

/// One field, "hello world", 300 by 40 at (10, 10), or a document of
/// two lines when `multiline`. It declares its change and boundary rows:
/// an editor with no row sends nothing (ADR 0047).
fn frame(core: &mut Core, multiline: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.text_edit(
        "field",
        if multiline {
            "hello world\nsecond line"
        } else {
            "hello world"
        },
        &EditOptions {
            multiline,
            ..Default::default()
        },
        NodeSpec::column()
            .size(300.0, if multiline { 80.0 } else { 40.0 })
            .on_change(Value::Null)
            .on_boundary(Value::Null),
    );
    ui.finish();
    key
}

fn rig(multiline: bool) -> (Core, Key) {
    let mut core = Core::new();
    let key = frame(&mut core, multiline);
    frame(&mut core, multiline);
    (core, key)
}

/// The middle of the field's first line, `x` in from its left edge.
fn at(x: f32) -> Vec2 {
    Vec2::new(10.0 + x, 22.0)
}

/// A press and its release as a window sends them: the pointer there, the
/// button down, a frame or two drawn while it is held — the press shows —
/// the pointer shaking by `shake` px on the way, and the button up where
/// the shake left it.
fn click(core: &mut Core, p: Vec2, clicks: u8, shake: f32, multiline: bool) {
    core.handle_input(InputEvent::CursorMoved(p));
    core.handle_input(InputEvent::mouse_down(clicks));
    frame(core, multiline);
    if shake > 0.0 {
        core.handle_input(InputEvent::CursorMoved(Vec2::new(p.x + shake, p.y)));
        frame(core, multiline);
        core.handle_input(InputEvent::CursorMoved(Vec2::new(p.x - shake / 2.0, p.y)));
    }
    frame(core, multiline);
    core.handle_input(InputEvent::mouse_up());
    frame(core, multiline);
}

fn edit(core: &mut Core, ek: EditKey, mods: Mods) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Key(ek, mods))
}

/// The edge of the boundary event, from its `boundary: {key, edge, ...}`.
fn boundary(evs: &[UiEvent]) -> Option<String> {
    let e = evs.iter().find(|e| e.is(EventKind::Boundary))?;
    e.data()?.get_str("edge").map(str::to_string)
}

/// The anchor and the caret, as byte offsets into the first line.
fn ends(core: &Core, key: Key) -> (usize, usize) {
    let (a, c) = core.edit.selection_cursors(key).unwrap();
    (a.1, c.1)
}

#[test]
fn the_first_backspace_after_a_click_deletes_one_character() {
    for shake in [0.0, 1.0, 2.5] {
        let (mut core, key) = rig(false);
        // Past the end of "hello world": the caret after the d.
        click(&mut core, at(290.0), 1, shake, false);
        assert_eq!(core.focus(), Some(key), "shake {shake}");
        assert_eq!(ends(&core, key), (11, 11), "shake {shake}: a caret");
        assert!(!core.edit.has_selection(key));
        let evs = edit(&mut core, EditKey::Backspace, NONE);
        assert!(evs.iter().any(|e| e.is(EventKind::Change)));
        assert_eq!(
            core.edit_text(key).as_deref(),
            Some("hello worl"),
            "shake {shake}: one character, at once"
        );
    }
}

#[test]
fn the_first_arrow_after_a_click_moves() {
    let (mut core, key) = rig(false);
    click(&mut core, at(290.0), 1, 1.0, false);
    edit(&mut core, EditKey::Left, NONE);
    assert_eq!(ends(&core, key), (10, 10), "← moved one character");
    edit(&mut core, EditKey::Right, NONE);
    assert_eq!(ends(&core, key), (11, 11), "→ moved back");

    // At the end, the first → and ↓ meet the edge and say so.
    let (mut core, _) = rig(false);
    click(&mut core, at(290.0), 1, 0.0, false);
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Right, NONE)).as_deref(),
        Some("end")
    );
    let (mut core, _) = rig(false);
    click(&mut core, at(290.0), 1, 0.0, false);
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Down, NONE)).as_deref(),
        Some("end"),
        "a field has no line below"
    );
    // At the start, the first ← and Backspace.
    let (mut core, key) = rig(false);
    click(&mut core, at(0.0), 1, 0.0, false);
    assert_eq!(ends(&core, key), (0, 0));
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Left, NONE)).as_deref(),
        Some("start")
    );
    assert_eq!(
        boundary(&edit(&mut core, EditKey::Backspace, NONE)).as_deref(),
        Some("start")
    );
    assert_eq!(core.edit_text(key).as_deref(), Some("hello world"));
}

#[test]
fn a_drag_still_selects() {
    let (mut core, key) = rig(false);
    core.handle_input(InputEvent::CursorMoved(at(0.0)));
    core.handle_input(InputEvent::mouse_down(1));
    frame(&mut core, false);
    core.handle_input(InputEvent::CursorMoved(at(290.0)));
    frame(&mut core, false);
    core.handle_input(InputEvent::mouse_up());
    frame(&mut core, false);
    assert_eq!(ends(&core, key), (0, 11));
    assert!(core.edit.has_selection(key));
    assert_eq!(core.copy_selection().as_deref(), Some("hello world"));
    edit(&mut core, EditKey::Backspace, NONE);
    assert_eq!(
        core.edit_text(key).as_deref(),
        Some(""),
        "Backspace takes it"
    );

    // A drag that comes back to where it began holds nothing: the next
    // key acts on the caret.
    let (mut core, key) = rig(false);
    core.handle_input(InputEvent::CursorMoved(at(290.0)));
    core.handle_input(InputEvent::mouse_down(1));
    core.handle_input(InputEvent::CursorMoved(at(0.0)));
    assert!(core.edit.has_selection(key), "selecting while held");
    core.handle_input(InputEvent::CursorMoved(at(290.0)));
    core.handle_input(InputEvent::mouse_up());
    frame(&mut core, false);
    assert_eq!(ends(&core, key), (11, 11));
    edit(&mut core, EditKey::Backspace, NONE);
    assert_eq!(core.edit_text(key).as_deref(), Some("hello worl"));
}

#[test]
fn a_second_click_takes_the_word_and_a_third_the_line() {
    let (mut core, key) = rig(true);
    // In "world".
    click(&mut core, at(1.0), 1, 0.0, true);
    click(&mut core, at(60.0), 1, 0.0, true);
    click(&mut core, at(60.0), 2, 1.0, true);
    assert_eq!(core.copy_selection().as_deref(), Some("world"));
    click(&mut core, at(60.0), 3, 1.0, true);
    let line = core.copy_selection().unwrap_or_default();
    assert!(line.starts_with("hello world"), "{line:?}");
    assert!(!line.contains("second"), "the line, not the text: {line:?}");
    let _ = key;
}

#[test]
fn a_shift_click_extends_from_the_caret() {
    let (mut core, key) = rig(false);
    click(&mut core, at(0.0), 1, 0.0, false);
    assert_eq!(ends(&core, key), (0, 0));
    core.handle_input(InputEvent::Modifiers(kui_core::KeyMods {
        shift: true,
        ..Default::default()
    }));
    click(&mut core, at(290.0), 1, 1.0, false);
    assert_eq!(
        ends(&core, key),
        (0, 11),
        "the anchor kept, the caret moved"
    );
    assert_eq!(core.copy_selection().as_deref(), Some("hello world"));
    // A Shift-click on the caret itself selects nothing.
    let (mut core, key) = rig(false);
    click(&mut core, at(290.0), 1, 0.0, false);
    core.handle_input(InputEvent::Modifiers(kui_core::KeyMods {
        shift: true,
        ..Default::default()
    }));
    click(&mut core, at(290.0), 1, 0.0, false);
    assert_eq!(ends(&core, key), (11, 11));
    core.handle_input(InputEvent::Modifiers(kui_core::KeyMods::default()));
    edit(&mut core, EditKey::Backspace, NONE);
    assert_eq!(core.edit_text(key).as_deref(), Some("hello worl"));
}

/// A pointer that shakes under a click — a trackpad's tap, a hand on a
/// mouse — moves nothing while it stays inside the click slop, wherever
/// the press lands: across the middle of a letter it would have selected
/// that letter, and put the caret on its other side.
#[test]
fn a_shake_inside_the_slop_is_still_a_click() {
    for x in 0..120 {
        let x = x as f32;
        let (mut still, key) = rig(false);
        click(&mut still, at(x), 1, 0.0, false);
        let (mut shaken, _) = rig(false);
        click(&mut shaken, at(x), 1, 2.5, false);
        assert!(!shaken.edit.has_selection(key), "at {x}");
        assert_eq!(ends(&shaken, key), ends(&still, key), "at {x}");
    }
}
