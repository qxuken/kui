//! Caret behavior through a live `Core`: scroll-caret-into-view for edits
//! inside scroll containers, multi-click word/line selection, and the blink
//! gate on caret emission.

use kui_core::{
    Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, QuadKind, Size, Sizing, Vec2,
};

const VIEW_H: f32 = 100.0;

/// A multiline edit inside a fixed-height scroll container.
fn frame(core: &mut Core, initial: &str) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut edit_key = Key::ROOT;
    let scroll_key = ui.with_keyed(
        "scroll",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(VIEW_H))
            .scroll_y(),
        |ui| {
            edit_key = ui.text_edit(
                "doc",
                initial,
                &EditOptions {
                    multiline: true,
                    autofocus: true,
                    ..Default::default()
                },
                NodeSpec::column().width(Sizing::Grow(1.0)),
            );
        },
    );
    ui.finish();
    (scroll_key, edit_key)
}

fn many_lines(n: usize) -> String {
    (0..n)
        .map(|i| format!("line {i}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn caret_motion_scrolls_container() {
    let mut core = Core::new();
    let (scroll_key, _) = frame(&mut core, &many_lines(40));
    assert_eq!(core.scroll.offset(scroll_key), Vec2::ZERO);

    // Jump to the end of the document: far below the 100px viewport.
    core.handle_input(InputEvent::Key(
        EditKey::End,
        Mods {
            doc: true,
            ..Default::default()
        },
    ));
    frame(&mut core, "");
    let bottom = core.scroll.offset(scroll_key).y;
    assert!(
        bottom > VIEW_H,
        "caret at doc end should scroll far down, got {bottom}"
    );

    // And back to the start scrolls home again.
    core.handle_input(InputEvent::Key(
        EditKey::Home,
        Mods {
            doc: true,
            ..Default::default()
        },
    ));
    frame(&mut core, "");
    assert_eq!(core.scroll.offset(scroll_key).y, 0.0);
}

#[test]
fn typing_at_bottom_keeps_caret_visible() {
    let mut core = Core::new();
    let (scroll_key, _) = frame(&mut core, &many_lines(6));
    core.handle_input(InputEvent::Key(
        EditKey::End,
        Mods {
            doc: true,
            ..Default::default()
        },
    ));
    frame(&mut core, "");
    let mut last = core.scroll.offset(scroll_key).y;
    // Each new line pushes the caret below the view; the frame must follow.
    for i in 0..10 {
        core.handle_input(InputEvent::Text("\nmore".into()));
        frame(&mut core, "");
        let now = core.scroll.offset(scroll_key).y;
        assert!(now > last, "line {i}: offset should grow ({last} -> {now})");
        last = now;
    }
}

#[test]
fn unfocused_edits_do_not_scroll() {
    let mut core = Core::new();
    let (scroll_key, _) = frame(&mut core, &many_lines(40));
    core.edit.set_focus(None);
    core.handle_input(InputEvent::Key(
        EditKey::End,
        Mods {
            doc: true,
            ..Default::default()
        },
    ));
    frame(&mut core, "");
    assert_eq!(core.scroll.offset(scroll_key), Vec2::ZERO);
}

#[test]
fn double_click_selects_word_triple_selects_line() {
    let mut core = Core::new();
    let (_, edit_key) = frame(&mut core, "alpha beta gamma\nsecond line");

    // Double-click on the first word.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(12.0, 8.0)));
    core.handle_input(InputEvent::mouse_down(2));
    core.handle_input(InputEvent::mouse_up());
    let word = core.copy_selection().unwrap_or_default();
    assert_eq!(word, "alpha", "double click should select the word");

    // Triple-click selects the whole line.
    core.handle_input(InputEvent::mouse_down(3));
    core.handle_input(InputEvent::mouse_up());
    let line = core.copy_selection().unwrap_or_default();
    assert_eq!(
        line, "alpha beta gamma",
        "triple click should select the line"
    );
    assert!(core.is_focused(edit_key));
}

/// The other half of the double-click gesture: hold the second press and
/// drag, and an editor selects word by word rather than character by
/// character. cosmic-text's `Selection::Word` does the expanding — this
/// pins that our press and drag actually reach it, since a `selectable`
/// scope had to be taught the same trick by hand (ADR 0017).
#[test]
fn a_held_double_press_drags_an_editor_by_words() {
    let mut core = Core::new();
    frame(&mut core, "alpha beta gamma\nsecond line");
    core.handle_input(InputEvent::CursorMoved(Vec2::new(12.0, 8.0)));
    core.handle_input(InputEvent::mouse_down(2));
    assert_eq!(core.copy_selection().as_deref(), Some("alpha"));
    // Held, and dragged into the third word: whole words.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(96.0, 8.0)));
    let text = core.copy_selection().unwrap_or_default();
    assert!(
        text.starts_with("alpha") && text.ends_with("gamma"),
        "words, not characters: {text:?}"
    );
    core.handle_input(InputEvent::mouse_up());
}

#[test]
fn blink_gate_hides_caret_quads() {
    let caret_quads = |core: &mut Core| {
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::Solid && q.rect.w == 2.0)
            .count()
    };

    let mut core = Core::new();
    frame(&mut core, "hello");
    assert_eq!(
        caret_quads(&mut core),
        1,
        "focused edit draws its caret by default"
    );

    core.edit.set_blink_visible(false);
    frame(&mut core, "");
    assert_eq!(caret_quads(&mut core), 0, "blink-off frame draws no caret");

    core.edit.set_blink_visible(true);
    frame(&mut core, "");
    assert_eq!(caret_quads(&mut core), 1);
}
