//! IME composition through a live `Core`: preedit overlay emission,
//! clearing on commit/blur, and the caret rect drivers anchor the IME to.

use kui_core::{Core, EditOptions, InputEvent, Key, NodeSpec, Size, Sizing};

fn frame(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.text_edit(
        "field",
        "hello ",
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

fn quad_count(core: &mut Core) -> usize {
    let (dl, _) = core.output();
    dl.quads.len()
}

#[test]
fn preedit_draws_an_overlay_and_commit_clears_it() {
    let mut core = Core::new();
    let key = frame(&mut core);
    frame(&mut core);
    let base = quad_count(&mut core);

    core.handle_input(InputEvent::Preedit("かな".into(), Some((0, 0))));
    assert_eq!(core.edit.preedit(key), Some("かな"));
    frame(&mut core);
    let with_preedit = quad_count(&mut core);
    // Backdrop + glyphs + underline + caret, minus the hidden normal caret.
    assert!(
        with_preedit > base + 2,
        "expected overlay quads: {base} -> {with_preedit}"
    );

    // Commit: preedit clears, text lands in the document at the caret.
    core.handle_input(InputEvent::Preedit(String::new(), None));
    core.handle_input(InputEvent::Text("かな".into()));
    assert_eq!(core.edit.preedit(key), None);
    assert!(core.edit_text(key).unwrap().contains("かな"));
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
        Some("hello "),
        "nothing committed"
    );
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

    core.handle_input(InputEvent::Key(
        kui_core::EditKey::End,
        kui_core::Mods::default(),
    ));
    frame(&mut core);
    let end = core.ime_rect().unwrap();
    assert!(
        end.x > start.x,
        "caret at line end sits further right ({} vs {})",
        end.x,
        start.x
    );
}
