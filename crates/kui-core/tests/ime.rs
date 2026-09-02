//! IME composition through a live `Core`: the preedit lives inline in the
//! buffer (following text shifts, the caret sits inside it), commits and
//! cancels leave the committed text and undo history exactly right, and
//! the caret rect drivers anchor the IME to tracks it.

use kui_core::{
    Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, QuadKind, Size, Sizing,
};

fn frame(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.text_edit(
        "field",
        "hello world",
        &EditOptions {
            multiline: false,
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().width(Sizing::Grow(1.0)).pad(5.0),
    );
    ui.finish();
    key
}

/// x of the right-most glyph quad: where the visible text ends.
fn text_right_edge(core: &mut Core) -> f32 {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphColor))
        .map(|q| q.rect.x + q.rect.w)
        .fold(0.0, f32::max)
}

fn solid_count(core: &mut Core) -> usize {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .count()
}

/// Caret after "hello " (the IME composes mid-sentence).
fn caret_after_hello(core: &mut Core) {
    core.handle_input(InputEvent::Key(EditKey::Home, Mods::default()));
    for _ in 0..6 {
        core.handle_input(InputEvent::Key(EditKey::Right, Mods::default()));
    }
}

#[test]
fn composition_reflows_the_text_and_is_marked() {
    let mut core = Core::new();
    let key = frame(&mut core);
    caret_after_hello(&mut core);
    frame(&mut core);
    let edge = text_right_edge(&mut core);
    let solids = solid_count(&mut core);
    let caret = core.ime_rect().unwrap();

    core.handle_input(InputEvent::Preedit("かな".into(), Some((6, 6))));
    assert_eq!(core.edit.preedit(key), Some("かな"));
    frame(&mut core);
    assert!(
        text_right_edge(&mut core) > edge + 10.0,
        "\"world\" shifted right to make room for the composition"
    );
    assert!(
        solid_count(&mut core) >= solids + 2,
        "composition backdrop + underline are drawn"
    );
    // The caret moved past the composition (IME offset = its end).
    let composing = core.ime_rect().unwrap();
    assert!(
        composing.x > caret.x + 10.0,
        "{} vs {}",
        composing.x,
        caret.x
    );
    // The committed text is untouched.
    assert_eq!(core.edit_text(key).as_deref(), Some("hello world"));

    // Updating the composition replaces it in place.
    core.handle_input(InputEvent::Preedit("か".into(), Some((3, 3))));
    frame(&mut core);
    let shorter = core.ime_rect().unwrap();
    assert!(shorter.x < composing.x && shorter.x > caret.x);
    assert_eq!(core.edit_text(key).as_deref(), Some("hello world"));

    // Commit: the composition leaves, the text lands where it began.
    core.handle_input(InputEvent::Preedit(String::new(), None));
    core.handle_input(InputEvent::Text("仮名".into()));
    assert_eq!(core.edit.preedit(key), None);
    assert_eq!(core.edit_text(key).as_deref(), Some("hello 仮名world"));
    frame(&mut core);
    assert_eq!(solid_count(&mut core), solids, "no marking left behind");
}

#[test]
fn cancel_restores_the_text_and_leaves_undo_alone() {
    let mut core = Core::new();
    let key = frame(&mut core);
    caret_after_hello(&mut core);
    core.handle_input(InputEvent::Text("X".into()));
    assert_eq!(core.edit_text(key).as_deref(), Some("hello Xworld"));

    core.handle_input(InputEvent::Preedit("あい".into(), None));
    core.handle_input(InputEvent::Preedit(String::new(), None));
    assert_eq!(core.edit.preedit(key), None);
    assert_eq!(core.edit_text(key).as_deref(), Some("hello Xworld"));
    // Undo skips the composition entirely and removes the typed X.
    core.handle_input(InputEvent::Key(EditKey::Undo, Mods::default()));
    assert_eq!(core.edit_text(key).as_deref(), Some("hello world"));
    core.handle_input(InputEvent::Key(EditKey::Undo, Mods::default()));
    assert_eq!(
        core.edit_text(key).as_deref(),
        Some("hello world"),
        "nothing else recorded"
    );
}

#[test]
fn composing_over_a_selection_replaces_it() {
    let mut core = Core::new();
    let key = frame(&mut core);
    core.handle_input(InputEvent::Key(EditKey::SelectAll, Mods::default()));
    core.handle_input(InputEvent::Preedit("あ".into(), None));
    assert_eq!(
        core.edit_text(key).as_deref(),
        Some(""),
        "selection replaced"
    );
    core.handle_input(InputEvent::Preedit(String::new(), None));
    core.handle_input(InputEvent::Text("亜".into()));
    assert_eq!(core.edit_text(key).as_deref(), Some("亜"));
    core.handle_input(InputEvent::Key(EditKey::Undo, Mods::default()));
    assert_eq!(core.edit_text(key).as_deref(), Some(""));
    core.handle_input(InputEvent::Key(EditKey::Undo, Mods::default()));
    assert_eq!(core.edit_text(key).as_deref(), Some("hello world"));
}

#[test]
fn blur_abandons_the_composition() {
    let mut core = Core::new();
    let key = frame(&mut core);
    core.handle_input(InputEvent::Preedit("あ".into(), None));
    assert!(core.edit.preedit(key).is_some());
    core.edit.set_focus(None);
    assert_eq!(core.edit.preedit(key), None);
    assert_eq!(
        core.edit_text(key).as_deref(),
        Some("hello world"),
        "nothing committed"
    );
    frame(&mut core);
    assert!(text_right_edge(&mut core) > 0.0);
}

#[test]
fn ime_rect_tracks_the_focused_caret() {
    let mut core = Core::new();
    frame(&mut core);
    assert!(
        core.ime_rect().is_some(),
        "focused edit exposes a caret rect"
    );
    let start = core.ime_rect().unwrap();

    core.handle_input(InputEvent::Key(EditKey::End, Mods::default()));
    frame(&mut core);
    let end = core.ime_rect().unwrap();
    assert!(
        end.x > start.x,
        "caret at line end sits further right ({} vs {})",
        end.x,
        start.x
    );
}
