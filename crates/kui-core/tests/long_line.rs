//! A non-wrapping line past `LONG_LINE_BYTES` is shaped in chunks on
//! demand (backlog C19): the first frame shapes the screenful, not the
//! line; scrolling shapes what scrolls in; a keystroke reshapes the chunk
//! it lands in; and the two text queries answer through the chunks.

use kui_core::{
    Color, Core, Key, LONG_LINE_BYTES, NodeSpec, QuadKind, Size, Sizing, TextStyle, TextWrap, Vec2,
};

const LH: f32 = 18.0;
const VIEW_W: f32 = 600.0;

fn mono() -> TextStyle {
    TextStyle::new(13.0)
        .mono()
        .line_height(LH)
        .wrap(TextWrap::None)
}

/// `n` printable ASCII characters with a space every eleventh, so chunks
/// have whitespace to cut at.
fn line(n: usize) -> String {
    let mut x = 0x9E37_79B9_7F4A_7C15u64;
    let mut s = String::with_capacity(n);
    while s.len() < n {
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        s.push(if s.len() % 11 == 10 {
            ' '
        } else {
            (b'!' + (x % 93) as u8) as char
        });
    }
    s
}

/// The line in a horizontally scrolling view; returns the view's key and
/// the text node's, plus the glyph quads drawn.
fn frame(core: &mut Core, text: &str, scroll_x: Option<f32>) -> (Key, Key, usize) {
    let view = Key::ROOT.str("view");
    if let Some(x) = scroll_x {
        core.set_scroll(view, Vec2::new(x, 0.0));
    }
    let mut ui = core.frame(Size::new(VIEW_W, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    ui.with_keyed(
        "view",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(100.0))
            .scroll_x(),
        |ui| {
            ui.with_keyed("row", NodeSpec::row().height(Sizing::Fixed(LH)), |ui| {
                ui.text(text, mono())
            });
        },
    );
    ui.finish();
    let (dl, _) = core.output();
    let glyphs = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::GlyphMask)
        .count();
    (view, view.str("row").index(0), glyphs)
}

fn cell(core: &mut Core) -> f32 {
    core.measure_text("M", &mono(), None).width
}

#[test]
fn the_first_frame_shapes_the_screenful_and_not_the_line() {
    let mut core = Core::new();
    let text = line(100_000);
    let (view, _, glyphs) = frame(&mut core, &text, None);
    // What shows is a screenful of glyphs, and what was shaped is a few
    // chunks, not a hundred.
    let w = cell(&mut core);
    let visible = (VIEW_W / w).ceil() as usize;
    // Every eleventh cell is a space, which draws no glyph.
    let spaces = visible / 11 + 1;
    assert!(
        glyphs <= visible + 4,
        "{glyphs} glyphs for a {visible}-cell view"
    );
    assert!(
        glyphs + spaces + 4 >= visible,
        "{glyphs} glyphs for a {visible}-cell view"
    );
    assert!(
        core.text_cache_len() <= 6,
        "{} entries shaped",
        core.text_cache_len()
    );
    assert_eq!(core.long_lines(), 1);
    assert!(
        core.text_cache_bytes() < 2 << 20,
        "{} bytes for a line that used to cost ~47 MB",
        core.text_cache_bytes()
    );
    // The scroll container sees the whole line's width: monospace, so
    // the estimate is the truth.
    let g = core.scroll_geometry(view).unwrap();
    assert!(
        (g.content.w - text.len() as f32 * w).abs() < 2.0,
        "content {} vs {} cells",
        g.content.w,
        text.len()
    );
}

#[test]
fn scrolling_shapes_what_scrolls_in_and_draws_it() {
    let mut core = Core::new();
    let text = line(100_000);
    let w = cell(&mut core);
    frame(&mut core, &text, None);
    let before = core.text_cache_len();
    // Jump to the middle: the chunks there shape, the glyphs there draw.
    let (_, _, glyphs) = frame(&mut core, &text, Some(50_000.0 * w));
    let (_, _, glyphs2) = frame(&mut core, &text, Some(50_000.0 * w));
    assert!(glyphs > 0 && glyphs == glyphs2, "{glyphs} / {glyphs2}");
    assert!(core.text_cache_len() > before);
    assert!(
        core.text_cache_len() < before + 8,
        "{}",
        core.text_cache_len()
    );
    // The drawn glyphs sit inside the view.
    let (dl, _) = core.output();
    assert!(
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::GlyphMask)
            .all(|q| q.rect.x + q.rect.w > 0.0 && q.rect.x < VIEW_W),
        "every glyph drawn is inside the view"
    );
}

#[test]
fn a_keystroke_into_the_middle_reshapes_one_chunk() {
    let mut core = Core::new();
    let text = line(100_000);
    let w = cell(&mut core);
    frame(&mut core, &text, Some(50_000.0 * w));
    frame(&mut core, &text, Some(50_000.0 * w));
    let before = core.text_cache_len();
    let mut edited = text.clone();
    edited.insert(50_000, 'X');
    frame(&mut core, &edited, Some(50_000.0 * w));
    // A new long line (its key is its content), whose chunks all hit the
    // cache except the one the edit landed in - and the first, which
    // every line shapes for its metrics.
    assert_eq!(core.long_lines(), 2);
    assert!(
        core.text_cache_len() <= before + 3,
        "{} -> {}: more than the edited chunk reshaped",
        before,
        core.text_cache_len()
    );
}

#[test]
fn the_text_queries_answer_through_the_chunks() {
    let mut core = Core::new();
    let text = line(100_000);
    let w = cell(&mut core);
    let (_, node, _) = frame(&mut core, &text, Some(50_000.0 * w));
    frame(&mut core, &text, Some(50_000.0 * w));
    // Byte 50_100 is on screen: its caret is where a monospace grid puts
    // it, minus the scroll.
    let r = core.caret_rect(node, 50_100).unwrap();
    assert!((r.x - 100.0 * w).abs() < 1.0, "{} vs {}", r.x, 100.0 * w);
    assert_eq!(r.h, LH);
    let hit = core.text_hit(node, Vec2::new(100.2 * w, 5.0)).unwrap();
    assert_eq!(hit.byte, 50_100);
    // A byte in a chunk that never showed answers by the mean advance,
    // which under monospace is the same number.
    let far = core.caret_rect(node, 90_000).unwrap();
    assert!((far.x - (90_000.0 - 50_000.0) * w).abs() < 2.0, "{}", far.x);
    // The end is the end.
    assert_eq!(
        core.caret_rect(node, 999_999).unwrap().x,
        core.caret_rect(node, text.len()).unwrap().x
    );
}

#[test]
fn a_short_line_takes_the_path_it_always_took() {
    let mut core = Core::new();
    let text = line(LONG_LINE_BYTES - 1);
    frame(&mut core, &text, None);
    assert_eq!(core.long_lines(), 0);
    assert_eq!(core.text_cache_len(), 1);
    // A wrapping style is never chunked, however long.
    let mut ui = core.frame(Size::new(VIEW_W, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text(&line(20_000), TextStyle::new(13.0).mono());
    ui.finish();
    assert_eq!(core.long_lines(), 0);
}

#[test]
fn measurement_matches_layout_for_a_long_line() {
    let mut core = Core::new();
    let text = line(20_000);
    let w = cell(&mut core);
    let m = core.measure_text(&text, &mono(), None);
    assert!((m.width - text.len() as f32 * w).abs() < 2.0, "{}", m.width);
    assert_eq!(m.height, LH);
    assert_eq!(m.lines, 1);
    let clamped = core.measure_text(&text, &mono(), Some(300.0));
    assert_eq!(clamped.width, 300.0);
}
