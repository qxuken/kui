//! A single-line `<edit>` is a field, not a short document: it measures
//! what its text wants, lays out on one line whatever it is given, and
//! scrolls that line under the caret — unless it declared `wrap`, when it
//! folds to its width like a document and keeps a field's keyboard.
//! Backlog F38 (the fit width that fed back on itself), F41 (the field
//! that wrapped) and F44 (the field that could not).

use kui_core::{
    Core, EditKey, EditOptions, InputEvent, Key, Mods, NodeSpec, QuadKind, Size, Sizing, TextStyle,
    TextWrap, Vec2,
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
    let spec = NodeSpec::column().pad(PAD).width(match width {
        Some(w) => Sizing::Fixed(w),
        None => Sizing::Fit,
    });
    let opts = EditOptions {
        multiline,
        autofocus: true,
        style: TextStyle::new(size),
        ..Default::default()
    };
    frame_with(core, initial, &opts, spec)
}

/// [`frame`] with the editor's options and box spelled out.
fn frame_with(
    core: &mut Core,
    initial: &str,
    opts: &EditOptions,
    spec: NodeSpec,
) -> (Key, kui_core::Rect) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.text_edit("field", initial, opts, spec);
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
/// The glyph quads, each with the clip it names resolved out of
/// `DisplayList::clips` — a quad carries an index, not the clip.
fn glyphs(core: &mut Core) -> Vec<(kui_core::Quad, kui_core::Clip)> {
    let (dl, _) = core.output();
    let mut out: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| !matches!(q.kind, QuadKind::Solid))
        .map(|q| (*q, dl.clip_of(q)))
        .collect();
    out.sort_by(|a, b| a.0.rect.x.total_cmp(&b.0.rect.x));
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
    const TEXT: &str = "Throwaway name that is long";
    let mut core = Core::new();
    let (_key, rect) = frame(&mut core, "", false, Some(100.0), 16.0);
    core.handle_input(InputEvent::Text(TEXT.into()));
    frame(&mut core, "", false, Some(100.0), 16.0);

    let content_l = rect.x + PAD;
    let content_r = rect.x + rect.w - PAD;
    let drawn = glyphs(&mut core);
    assert!(!drawn.is_empty(), "no glyphs emitted");
    assert!(
        drawn[0].0.rect.x < content_l,
        "the head of the text has scrolled out to the left: {} vs {content_l}",
        drawn[0].0.rect.x
    );
    // Every glyph is clipped to the field's content box, so the tail does
    // not run out over whatever sits beside it.
    for (_, clip) in &drawn {
        assert!(
            clip.rect.x >= content_l - 0.5 && clip.rect.x + clip.rect.w <= content_r + 0.5,
            "a glyph escaped the field: clip {:?} vs {content_l}..{content_r}",
            clip.rect
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

    // Home brings the head back. Where the head lands is not `content_l`
    // itself: a quad is a glyph's ink, and how far that ink sits from the
    // pen is the font's business — the first glyph of this text starts on
    // the pen in Helvetica Neue and a pixel left of it in DejaVu Sans, so
    // a tolerance around `content_l` only ever holds on one CI image. The
    // reference is the same text in a box wide enough never to scroll:
    // that field's head is where an unscrolled head belongs, whatever the
    // font drew.
    let mut unscrolled = Core::new();
    frame(&mut unscrolled, TEXT, false, Some(360.0), 16.0);
    let start = glyphs(&mut unscrolled)[0].0.rect.x;

    core.handle_input(InputEvent::Key(EditKey::Home, Mods::default()));
    frame(&mut core, "", false, Some(100.0), 16.0);
    let head = glyphs(&mut core)[0].0.rect.x;
    assert!(
        (head - start).abs() < 0.5,
        "Home should scroll the field back to the start: {head} vs {start}"
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

/// A field that declared `wrap`: a field's keyboard on a document's layout
/// (F44). `size` is the font's; `wrap` the mode the row picked.
fn folding(size: f32, wrap: TextWrap) -> EditOptions {
    EditOptions {
        multiline: false,
        autofocus: true,
        wrap: true,
        style: TextStyle::new(size).wrap(wrap),
        ..Default::default()
    }
}

/// The mind map's rename field: the node's label, in the node's width.
const LABEL: &str = "a considerably longer label than the box is wide";
const LABEL_W: f32 = 209.0;

#[test]
fn a_field_with_wrap_folds_to_its_width_and_still_submits() {
    // F44: F41 made every single-line editor take one line, and the rename
    // field that had declared `wrap="word"` to break where its label
    // breaks went from two lines to one, scrolled. With `wrap` honoured
    // the field folds like a document — and is still a field: Enter
    // submits, and the draft gains no newline.
    let mut core = Core::new();
    let one_line = frame(&mut core, "x", false, Some(LABEL_W), 16.0).1.h;

    let mut core = Core::new();
    let opts = folding(16.0, TextWrap::Word);
    let spec = NodeSpec::column().pad(PAD).width(LABEL_W);
    let (key, rect) = frame_with(&mut core, LABEL, &opts, spec.clone());
    // How many lines the label folds onto is the sans-serif face's to
    // say (two on Windows, three on a fontconfig CI); folded is what the
    // test checks.
    let folded = lines(&mut core, key);
    assert!(
        folded >= 2,
        "the draft folds onto more than one line: {folded}"
    );
    assert!(
        rect.h > one_line + 10.0,
        "and the box is more than one line tall: {} vs one line's {one_line}",
        rect.h
    );
    // Nothing scrolled: the head of the text is where it starts, and the
    // caret opened at the end of the draft, on the second line.
    let head = glyphs(&mut core)[0].0.rect.x;
    assert!(
        head >= rect.x + PAD - 1.0,
        "a folded field does not scroll under the caret: head at {head}, box at {}",
        rect.x
    );
    let caret = core.ime_rect().expect("the focused caret has a rect");
    assert!(
        caret.y > rect.y + PAD + one_line / 2.0,
        "the caret opens at the end, on the second line: {:?} in {:?}",
        caret,
        rect
    );
    // And it is not clipped to itself the way a scrolling field is (F41):
    // its lines never leave the box, so it keeps its ancestors' clip.
    for (_, clip) in glyphs(&mut core) {
        assert!(
            clip.rect.w > rect.w + 1.0,
            "a folded field keeps the node's clip: {:?} vs box {:?}",
            clip.rect,
            rect
        );
    }

    let evs = core.handle_input(InputEvent::Key(EditKey::Enter, Mods::default()));
    assert_eq!(evs.len(), 1, "Enter on a folded field is one event");
    assert_eq!(
        evs[0].payload.get("kind").and_then(kui_core::Value::as_str),
        Some("submit")
    );
    assert_eq!(
        core.edit_text(key).as_deref(),
        Some(LABEL),
        "no newline went in"
    );
    frame_with(&mut core, "", &opts, spec);
    assert_eq!(lines(&mut core, key), folded);
}

#[test]
fn a_folded_field_admits_no_newline_by_any_door() {
    // The same rule a plain field has (F41), kept: a seed, a `set_text`
    // and a paste all drop the newline, whatever the box's height.
    let mut core = Core::new();
    let opts = folding(16.0, TextWrap::Word);
    let spec = NodeSpec::column().pad(PAD).width(LABEL_W);
    let (key, _) = frame_with(&mut core, "one\ntwo", &opts, spec.clone());
    assert_eq!(core.edit_text(key).as_deref(), Some("onetwo"));
    core.set_edit_text(key, "three\nfour");
    frame_with(&mut core, "", &opts, spec.clone());
    assert_eq!(core.edit_text(key).as_deref(), Some("threefour"));
    core.handle_input(InputEvent::Text("\nfive".into()));
    frame_with(&mut core, "", &opts, spec);
    assert_eq!(core.edit_text(key).as_deref(), Some("threefourfive"));
}

#[test]
fn wrap_none_on_a_field_is_the_field() {
    // `wrap="none"` declared is a mode, not a request to fold: the field
    // takes one line and scrolls it, exactly as one that never said.
    let mut core = Core::new();
    let opts = folding(16.0, TextWrap::None);
    let spec = NodeSpec::column().pad(PAD).width(LABEL_W);
    let (key, rect) = frame_with(&mut core, LABEL, &opts, spec);
    assert_eq!(lines(&mut core, key), 1);
    let head = glyphs(&mut core)[0].0.rect.x;
    assert!(
        head < rect.x + PAD,
        "an unfolded field scrolls its head out to the left: {head} vs {}",
        rect.x + PAD
    );
}

#[test]
fn a_folded_field_breaks_by_glyph_when_asked() {
    // The row's mode reaches the buffer: `wrap="glyph"` breaks inside a
    // word that `wrap="word"` carries whole onto the next line, so the
    // glyph break fills the first line further.
    let text = "abcdefghijklmnop abcdefghijklmnop";
    let spec = || NodeSpec::column().pad(PAD).width(LABEL_W);
    let first_line_w = |core: &mut Core, key: Key| {
        core.access_tree()
            .nodes
            .iter()
            .find(|n| n.key == key)
            .map(|n| n.runs[0].rect.w)
            .expect("the editor is in the access tree")
    };
    let mut by_word = Core::new();
    let (w, _) = frame_with(&mut by_word, text, &folding(16.0, TextWrap::Word), spec());
    let mut by_glyph = Core::new();
    let (g, _) = frame_with(&mut by_glyph, text, &folding(16.0, TextWrap::Glyph), spec());
    assert_eq!(lines(&mut by_word, w), 2, "two words, one per line");
    assert_eq!(lines(&mut by_glyph, g), 2, "the same text broken anywhere");
    let (word_w, glyph_w) = (
        first_line_w(&mut by_word, w),
        first_line_w(&mut by_glyph, g),
    );
    assert!(
        glyph_w > word_w + 5.0,
        "a glyph break fills the first line further than a word break: {glyph_w} vs {word_w}"
    );

    // A mode that changes between declarations re-measures the same text:
    // two long words break by glyph either way, but the word break starts
    // the second on a line of its own, and the box is a line taller for it.
    let two = "abcdefghijklmnopqrstuvwxyz abcdefghijklmnopqrstuvwxyz";
    let mut core = Core::new();
    let (key, tall) = frame_with(&mut core, two, &folding(16.0, TextWrap::Word), spec());
    let by_word = lines(&mut core, key);
    let (_, short) = frame_with(&mut core, "", &folding(16.0, TextWrap::Glyph), spec());
    let by_glyph = lines(&mut core, key);
    assert!(by_glyph < by_word, "{by_glyph} vs {by_word} lines");
    assert!(
        short.h < tall.h - 10.0,
        "the box follows the mode on the frame it changes: {} -> {}",
        tall.h,
        short.h
    );
    // The same when the style did not move and only the kind did: a
    // document breaks between words whatever its style says, so one
    // styled `glyph` that becomes a folding field re-breaks by glyph, and
    // the measurement cache cannot answer from the document's layout.
    let mut core = Core::new();
    let doc = EditOptions {
        multiline: true,
        ..folding(16.0, TextWrap::Glyph)
    };
    let (key, as_doc) = frame_with(&mut core, two, &doc, spec());
    let (_, as_field) = frame_with(&mut core, "", &folding(16.0, TextWrap::Glyph), spec());
    assert_eq!(lines(&mut core, key), by_glyph);
    assert!(
        as_field.h < as_doc.h - 10.0,
        "a document turned folding field re-measures: {} -> {}",
        as_doc.h,
        as_field.h
    );
}

#[test]
fn a_folded_fit_field_grows_on_the_keystroke_frame() {
    // The mind map's spelling once F44 lands: `width="fit" maxWidth={…}
    // wrap="word"`. The box hugs a short draft on one line, stops growing
    // sideways at `maxWidth`, and from there grows *down* — on the frame
    // that lays out the keystroke, since the core measures the draft and
    // not the width the app declared last frame. No headroom to declare.
    const MAX_W: f32 = 120.0;
    let mut core = Core::new();
    let opts = folding(16.0, TextWrap::Word);
    let spec = NodeSpec::column()
        .pad(PAD)
        .width(Sizing::Fit)
        .max_width(MAX_W);
    let (key, empty) = frame_with(&mut core, "", &opts, spec.clone());
    assert_eq!(lines(&mut core, key), 1);
    let one_line = empty.h;

    let mut last_w = empty.w;
    let mut folded_at = None;
    for (i, word) in ["some ", "words ", "that ", "run ", "past ", "the ", "edge"]
        .iter()
        .enumerate()
    {
        core.handle_input(InputEvent::Text(word.to_string()));
        let (_, rect) = frame_with(&mut core, "", &opts, spec.clone());
        assert!(
            rect.w <= MAX_W + 0.5,
            "the box never passes maxWidth: {} after word {i}",
            rect.w
        );
        assert!(
            rect.w >= last_w - 0.5,
            "and never narrows as the draft grows: {last_w} -> {}",
            rect.w
        );
        // Every glyph inside the box on the very frame it was typed: the
        // bug the app measured was the last word past the field's edge.
        for (q, _) in glyphs(&mut core) {
            assert!(
                q.rect.x + q.rect.w <= rect.x + rect.w + 0.5,
                "a glyph past the box on the keystroke frame: {:?} vs {:?}",
                q.rect,
                rect
            );
        }
        let n = lines(&mut core, key);
        if n > 1 && folded_at.is_none() {
            folded_at = Some(i);
            assert!(
                rect.h > one_line + 10.0,
                "the box is taller the frame the draft folds: {} vs {one_line}",
                rect.h
            );
            assert!(
                (rect.w - MAX_W).abs() < 0.5,
                "and folds only once it is as wide as it may be: {}",
                rect.w
            );
        }
        last_w = rect.w;
    }
    assert!(
        folded_at.is_some(),
        "seven words should have folded at 120 px"
    );
    assert!(lines(&mut core, key) >= 2);
}
