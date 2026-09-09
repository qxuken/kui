//! A single-line `<edit>` is a field, not a short document: it measures
//! what its text wants, lays out on one line whatever it is given, and
//! scrolls that line under the caret. Backlog F38 (the fit width that fed
//! back on itself) and F41 (the field that wrapped).

use kui_core::{
    Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, QuadKind, Size, Sizing, TextStyle,
    Vec2,
};

const PAD: f32 = 5.0;

/// One editor in a roomy root. `width` is the box's, `None` = fit its text.
fn frame(
    core: &mut Core,
    initial: &str,
    multiline: bool,
    width: Option<f32>,
    size: f32,
) -> (Key, kui_core::Rect) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let spec = NodeSpec::column().pad(PAD).width(match width {
        Some(w) => Sizing::Fixed(w),
        None => Sizing::Fit,
    });
    let key = ui.text_edit(
        "field",
        initial,
        &EditOptions {
            multiline,
            autofocus: true,
            style: TextStyle::new(size),
            ..Default::default()
        },
        spec,
    );
    ui.finish();
    let rect = core
        .access_tree()
        .nodes
        .iter()
        .find(|n| n.key == key)
        .map(|n| n.rect)
        .expect("the editor is in the access tree");
    (key, rect)
}

/// The glyph quads of the last frame, left to right.
fn glyphs(core: &mut Core) -> Vec<kui_core::Quad> {
    let (dl, _) = core.output();
    let mut out: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| !matches!(q.kind, QuadKind::Solid))
        .copied()
        .collect();
    out.sort_by(|a, b| a.rect.x.total_cmp(&b.rect.x));
    out
}

/// How many visual lines the editor's text is on, read off the access
/// tree's runs (one run per visual line).
fn lines(core: &mut Core, key: Key) -> usize {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.key == key)
        .map_or(0, |n| n.runs.len())
}

#[test]
fn a_field_that_hugs_its_text_grows_with_it() {
    // F38: the fit width used to be measured off a buffer still carrying
    // the last frame's wrap, so the box took the widest wrapped line, the
    // next frame wrapped to that, and a field seeded empty ratcheted down
    // to one character with every keystroke on its own line.
    let mut core = Core::new();
    let (key, empty) = frame(&mut core, "", false, None, 16.0);
    let mut last = empty.w;
    for ch in ["h", "e", "l", "l", "o"] {
        core.handle_input(InputEvent::Text(ch.to_string()));
        let (_, rect) = frame(&mut core, "", false, None, 16.0);
        assert_eq!(lines(&mut core, key), 1, "a field stays on one line");
        assert!(
            rect.w >= last,
            "the box must not narrow as the text grows: {last} -> {}",
            rect.w
        );
        last = rect.w;
    }
    assert_eq!(core.edit_text(key).as_deref(), Some("hello"));
    assert!(
        last > empty.w + 20.0,
        "five characters should have widened the box: {} -> {last}",
        empty.w
    );
}

#[test]
fn a_style_change_remeasures_the_same_text() {
    // The measurement caches are keyed on the metrics as well as the text:
    // the same string at 32px is not the size it was at 16.
    let mut core = Core::new();
    let (_, small) = frame(&mut core, "Throwaway", false, None, 16.0);
    let (_, big) = frame(&mut core, "", false, None, 32.0);
    assert!(
        big.w > small.w * 1.5,
        "a doubled font should about double the box: {} -> {}",
        small.w,
        big.w
    );
}

#[test]
fn a_field_does_not_wrap_and_a_document_does() {
    // F41: `multiline` decides it, and it used to decide nothing about
    // layout — both wrapped, so a name that outgrew its box was drawn two
    // lines tall inside a box measured for one.
    let long = "Throwaway name that is long";
    let mut core = Core::new();
    let (field, field_rect) = frame(&mut core, long, false, Some(80.0), 16.0);
    assert_eq!(lines(&mut core, field), 1, "a field is one line");

    let mut core = Core::new();
    let (doc, doc_rect) = frame(&mut core, long, true, Some(80.0), 16.0);
    assert!(
        lines(&mut core, doc) > 1,
        "a multiline editor still wraps to its box"
    );
    assert!(
        doc_rect.h > field_rect.h,
        "and is taller for it: {} vs {}",
        doc_rect.h,
        field_rect.h
    );
}

#[test]
fn a_field_scrolls_its_text_under_the_caret() {
    let mut core = Core::new();
    let (_key, rect) = frame(&mut core, "", false, Some(100.0), 16.0);
    core.handle_input(InputEvent::Text("Throwaway name that is long".into()));
    frame(&mut core, "", false, Some(100.0), 16.0);

    let content_l = rect.x + PAD;
    let content_r = rect.x + rect.w - PAD;
    let drawn = glyphs(&mut core);
    assert!(!drawn.is_empty(), "no glyphs emitted");
    assert!(
        drawn[0].rect.x < content_l,
        "the head of the text has scrolled out to the left: {} vs {content_l}",
        drawn[0].rect.x
    );
    // Every glyph is clipped to the field's content box, so the tail does
    // not run out over whatever sits beside it.
    for q in &drawn {
        assert!(
            q.clip.x >= content_l - 0.5 && q.clip.x + q.clip.w <= content_r + 0.5,
            "a glyph escaped the field: clip {:?} vs {content_l}..{content_r}",
            q.clip
        );
    }
    // The caret is at the end and inside the box, which is the whole point.
    // `ime_rect` is where the focused caret was drawn, in viewport px.
    let caret = core.ime_rect().expect("the focused caret has a rect");
    assert!(
        caret.x >= rect.x && caret.x <= content_r,
        "caret at {} is outside {}..{content_r}",
        caret.x,
        rect.x
    );

    // Home brings the head back.
    core.handle_input(InputEvent::Key(EditKey::Home, Mods::default()));
    frame(&mut core, "", false, Some(100.0), 16.0);
    let head = glyphs(&mut core)[0].rect.x;
    assert!(
        (head - content_l).abs() < 1.0,
        "Home should scroll the field back to the start: {head} vs {content_l}"
    );
}

#[test]
fn a_click_lands_on_the_character_under_it_while_scrolled() {
    let mut core = Core::new();
    let (key, rect) = frame(&mut core, "", false, Some(100.0), 16.0);
    core.handle_input(InputEvent::Text("aaaaaaaaaaaaaaaaaaaaaaaaaaaa".into()));
    frame(&mut core, "", false, Some(100.0), 16.0);
    let end = core.edit_text(key).unwrap().len();

    // A click at the left edge of the content box picks the first
    // character *shown*, which is deep into the text — not the first
    // character of the value.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(
        rect.x + PAD + 1.0,
        rect.y + rect.h / 2.0,
    )));
    core.handle_input(InputEvent::MouseDown {
        button: kui_core::MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::MouseUp {
        button: kui_core::MouseButton::Primary,
    });
    let (caret, _) = core
        .edit
        .caret_and_selection(key)
        .expect("a caret after the click");
    assert!(
        caret > 0 && caret < end,
        "the click should land inside the shown window of the text, not at {caret} of {end}"
    );
}

#[test]
fn a_field_keeps_its_value_to_one_line() {
    // Typing already dropped a newline into a field; a seed and a
    // `set_text` do too, so the buffer cannot hold a line the box was
    // never measured for.
    let mut core = Core::new();
    let (key, _) = frame(&mut core, "one\ntwo", false, None, 16.0);
    assert_eq!(core.edit_text(key).as_deref(), Some("onetwo"));
    assert_eq!(lines(&mut core, key), 1);

    core.set_edit_text(key, "three\nfour");
    frame(&mut core, "", false, None, 16.0);
    assert_eq!(core.edit_text(key).as_deref(), Some("threefour"));
    assert_eq!(lines(&mut core, key), 1);

    // A document keeps both.
    let mut core = Core::new();
    let (doc, _) = frame(&mut core, "one\ntwo", true, None, 16.0);
    assert_eq!(core.edit_text(doc).as_deref(), Some("one\ntwo"));
    assert_eq!(lines(&mut core, doc), 2);
}
