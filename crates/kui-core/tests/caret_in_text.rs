//! Where a byte lands in a text no node has drawn yet (backlog F171):
//! `caret_in_text` / `caret_in_rich_text` answer the rect `caret_rect`
//! gives once a text node with the same content and style is laid out at
//! the same width — before any frame, during the build of the frame that
//! draws it, wrapped or not, on the run path and the long-line path.

use kui_core::{Core, Key, NodeSpec, Rect, Size, Span, TextStyle, TextWrap};

const PAD: f32 = 10.0;

/// `caret_rect` for every byte of the text node a column `w` wide drew,
/// less where its first byte's line starts: the text's own origin.
fn drawn(core: &mut Core, spans: &[Span<'_>], style: TextStyle, w: f32, scale: f32) -> Vec<Rect> {
    let mut ui = core.frame(Size::new(800.0, 600.0), scale);
    let mut key = None;
    ui.with(NodeSpec::column().pad(PAD), |ui| {
        ui.with(NodeSpec::column().width(w), |ui| {
            key = Some(ui.child_key_indexed(0));
            ui.rich_text(spans, style);
        });
    });
    ui.finish();
    let key: Key = key.expect("the text's key");
    let len: usize = spans.iter().map(|s| s.text.len()).sum();
    (0..=len)
        .map(|b| {
            let r = core.caret_rect(key, b).expect("a caret");
            Rect::new(r.x - PAD, r.y - PAD, r.w, r.h)
        })
        .collect()
}

fn asked(core: &mut Core, spans: &[Span<'_>], style: &TextStyle, w: f32) -> Vec<Rect> {
    let len: usize = spans.iter().map(|s| s.text.len()).sum();
    (0..=len)
        .map(|b| {
            core.caret_in_rich_text(spans, style, Some(w), b)
                .expect("a caret")
        })
        .collect()
}

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(20.0)
}

fn rows(rects: &[Rect]) -> usize {
    let mut ys: Vec<i32> = rects.iter().map(|r| r.y.round() as i32).collect();
    ys.dedup();
    ys.len()
}

#[test]
fn a_byte_on_every_line_is_where_layout_puts_it() {
    let spans = [
        Span::new("the quick "),
        Span::new("brown").bold(),
        Span::new(" fox jumps over the "),
        Span::new("lazy").italic(),
        Span::new(" dog and keeps on running"),
    ];
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces, TextWrap::Glyph] {
        let style = mono().wrap(wrap);
        let mut core = Core::new();
        // Before any frame: nothing has drawn the text.
        let before = asked(&mut core, &spans, &style, 120.0);
        let laid = drawn(&mut core, &spans, style, 120.0, 1.0);
        assert!(rows(&laid) >= 3, "{wrap:?} wraps: {laid:?}");
        assert_eq!(before, laid, "{wrap:?}: asked before the frame");
        // And after it, while a node holds the text at that width.
        assert_eq!(
            asked(&mut core, &spans, &style, 120.0),
            laid,
            "{wrap:?}: after"
        );
    }
}

#[test]
fn a_plain_text_answers_as_a_text_node() {
    let text = "a plain line that wraps at the width it is given";
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces] {
        let style = mono().wrap(wrap);
        let mut core = Core::new();
        let before: Vec<Rect> = (0..=text.len())
            .map(|b| {
                core.caret_in_text(text, &style, Some(100.0), b)
                    .expect("a caret")
            })
            .collect();
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        let key = ui.text_in(NodeSpec::column().width(100.0), text, style);
        ui.finish();
        let laid: Vec<Rect> = (0..=text.len())
            .map(|b| core.caret_rect(key, b).expect("a caret"))
            .collect();
        assert!(rows(&laid) >= 3, "{wrap:?} wraps");
        assert_eq!(before, laid, "{wrap:?}");
    }
}

#[test]
fn without_a_width_the_text_is_one_line() {
    let mut core = Core::new();
    let style = mono();
    let spans = [Span::new("one line "), Span::new("however long").bold()];
    let end = core
        .caret_in_rich_text(&spans, &style, None, usize::MAX)
        .expect("a caret");
    let width = core.measure_rich_text(&spans, &style, None).width;
    assert_eq!((end.x, end.y, end.w, end.h), (width, 0.0, 0.0, 20.0));
}

/// The frame that types: the node drew another text last frame, so its
/// `caret_rect` answers for that one; the query answers for the new text
/// at the node's width, and the frame's layout agrees.
#[test]
fn the_frame_a_text_is_new_answers_for_the_new_text() {
    let style = mono().wrap(TextWrap::BreakSpaces);
    let w = 120.0;
    let old = "the quick brown fox jumps over the lazy";
    let new = "the quick brown fox jumps over the lazyy";
    let mut core = Core::new();
    let frame = |core: &mut Core, text: &str, byte: usize| {
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        let mut seen = (None, None, None);
        ui.with(NodeSpec::column().width(w), |ui| {
            let key = ui.child_key_indexed(0);
            seen.0 = Some(key);
            seen.1 = ui.caret_rect(key, byte);
            seen.2 = ui.caret_in_text(text, &style, Some(w), byte);
            ui.text(text, style);
        });
        ui.finish();
        seen
    };
    frame(&mut core, old, old.len());
    let (key, last, asked) = frame(&mut core, new, new.len());
    let key = key.expect("a key");
    let now = core.caret_rect(key, new.len()).expect("laid out");
    assert_eq!(asked, Some(now), "the query is this frame's layout");
    assert_ne!(last, Some(now), "the old layout is a letter behind");
    assert!(now.y > 0.0, "on a later visual line: {now:?}");
}

/// Asking at another width than a node drew the text at leaves the
/// node's rows alone, as a measure does (backlog RG76).
#[test]
fn asking_at_another_width_leaves_the_drawn_rows() {
    let spans = [Span::new("the quick brown fox "), Span::new("jumps").bold()];
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces] {
        let style = mono().wrap(wrap);
        let mut core = Core::new();
        let laid = drawn(&mut core, &spans, style, 120.0, 1.0);
        let narrow = asked(&mut core, &spans, &style, 60.0);
        assert!(rows(&narrow) > rows(&laid), "{wrap:?}");
        let mut key = None;
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        ui.with(NodeSpec::column().pad(PAD), |ui| {
            ui.with(NodeSpec::column().width(120.0), |ui| {
                key = Some(ui.child_key_indexed(0));
                // Between the build and layout as well.
                let _ = ui.caret_in_rich_text(&spans, &style, Some(60.0), 3);
                ui.rich_text(&spans, style);
            });
        });
        ui.finish();
        let key = key.expect("a key");
        let again: Vec<Rect> = (0..laid.len())
            .map(|b| {
                let r = core.caret_rect(key, b).expect("a caret");
                Rect::new(r.x - PAD, r.y - PAD, r.w, r.h)
            })
            .collect();
        assert_eq!(again, laid, "{wrap:?}");
    }
}

/// A line past `LONG_LINE_BYTES` is broken into rows at the width as its
/// node's is: exact in a chunk already shaped — the first is shaped when
/// the line is built — and by the line's mean advance in one that never
/// showed, as its layout is, until a frame has drawn it.
#[test]
fn a_long_line_is_broken_at_the_width() {
    let text = "word ".repeat(1200);
    let style = mono().wrap(TextWrap::Word);
    let spans = [Span::new(&text)];
    let mut core = Core::new();
    let probes = [0, 7, 1000, 4100, 5999, text.len()];
    let ask = |core: &mut Core| -> Vec<Rect> {
        probes
            .iter()
            .map(|&b| {
                core.caret_in_rich_text(&spans, &style, Some(300.0), b)
                    .expect("a caret")
            })
            .collect()
    };
    let before = ask(&mut core);
    let laid = drawn(&mut core, &spans, style, 300.0, 1.0);
    let laid: Vec<Rect> = probes.iter().map(|&b| laid[b]).collect();
    assert!(laid[5].y > 0.0, "wrapped: {laid:?}");
    assert_eq!(before[..3], laid[..3], "the first chunk's bytes");
    assert_eq!(ask(&mut core), laid, "every byte once drawn");
}

#[test]
fn the_rect_is_logical_px_at_the_frame_scale() {
    let spans = [
        Span::new("scaled text "),
        Span::new("wraps").bold(),
        Span::new(" too"),
    ];
    let style = mono().wrap(TextWrap::Word);
    let mut core = Core::new();
    let laid = drawn(&mut core, &spans, style, 90.0, 2.0);
    assert!(rows(&laid) >= 2);
    assert_eq!(asked(&mut core, &spans, &style, 90.0), laid);
}
