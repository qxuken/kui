//! A non-wrapping line past `LONG_LINE_BYTES` is shaped in chunks on
//! demand (backlog C19): the first frame shapes the screenful, not the
//! line; scrolling shapes what scrolls in; a keystroke reshapes the chunk
//! it lands in; and the two text queries answer through the chunks.

use kui_core::{
    Color, Core, Key, LONG_LINE_BYTES, NodeSpec, QuadKind, Size, Sizing, Span, TextStyle, TextWrap,
    Vec2,
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
    // A line budget is a property of the whole: `max_lines` or `ellipsis`
    // keeps the whole path, however long the text.
    let mut ui = core.frame(Size::new(VIEW_W, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text(&line(20_000), TextStyle::new(13.0).mono().max_lines(3));
    ui.text(&line(20_000), TextStyle::new(13.0).mono().ellipsis());
    ui.finish();
    assert_eq!(core.long_lines(), 0);
}

// ---- wrapped (C19 step 5) --------------------------------------------

fn wrapped(wrap: TextWrap) -> TextStyle {
    TextStyle::new(13.0).mono().line_height(LH).wrap(wrap)
}

/// The line as a paragraph in a vertically scrolling view of `VIEW_W`;
/// returns the view's key, the text node's, and the glyph quads drawn.
fn wrapped_frame(
    core: &mut Core,
    text: &str,
    wrap: TextWrap,
    scroll_y: Option<f32>,
) -> (Key, Key, usize) {
    let view = Key::ROOT.str("view");
    if let Some(y) = scroll_y {
        core.set_scroll(view, Vec2::new(0.0, y));
    }
    let mut ui = core.frame(Size::new(VIEW_W, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    ui.with_keyed(
        "view",
        NodeSpec::column()
            .width(Sizing::Fixed(VIEW_W))
            .height(Sizing::Fixed(100.0))
            .scroll_y(),
        |ui| {
            ui.with_keyed(
                "row",
                NodeSpec::column().width(Sizing::Fixed(VIEW_W)),
                |ui| ui.text(text, wrapped(wrap)),
            );
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

#[test]
fn a_wrapped_long_line_breaks_into_rows_and_shapes_the_screenful() {
    let mut core = Core::new();
    let text = line(100_000);
    let w = cell(&mut core);
    let (view, _, glyphs) = wrapped_frame(&mut core, &text, TextWrap::Word, None);
    assert_eq!(core.long_lines(), 1);
    // A screenful of rows, from a couple of chunks.
    let per_row = (VIEW_W / w).floor() as usize;
    let rows_visible = (100.0 / LH).ceil() as usize;
    assert!(
        glyphs > (rows_visible - 1) * per_row * 8 / 11 && glyphs <= (rows_visible + 1) * per_row,
        "{glyphs} glyphs for {rows_visible} rows of {per_row}"
    );
    assert!(
        core.text_cache_len() <= 4,
        "{} entries shaped",
        core.text_cache_len()
    );
    // The scroll container sees the paragraph's height: eleven-cell words
    // pack seven to a row of `per_row` cells, the trailing space hanging.
    let words_per_row = (per_row + 1) / 11;
    let rows = text.len().div_ceil(words_per_row * 11);
    let g = core.scroll_geometry(view).unwrap();
    let expect = rows as f32 * LH;
    assert!(
        (g.content.h - expect).abs() < expect * 0.05,
        "content {} vs ~{} rows",
        g.content.h,
        rows
    );
    // Every glyph drawn sits inside the box, on a row (a bearing may
    // hang a pixel or two past the box, as a live row's does).
    let (dl, _) = core.output();
    let outside: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::GlyphMask)
        .filter(|q| !(q.rect.x >= -2.0 && q.rect.x + q.rect.w <= VIEW_W + 2.0 && q.rect.y < 100.0))
        .map(|q| q.rect)
        .collect();
    assert!(outside.is_empty(), "{outside:?}");
}

#[test]
fn a_glyph_wrapped_long_line_fills_every_row() {
    let mut core = Core::new();
    let text = line(100_000);
    let w = cell(&mut core);
    let (view, _, _) = wrapped_frame(&mut core, &text, TextWrap::Glyph, None);
    let per_row = (VIEW_W / w).floor() as usize;
    let rows = text.len().div_ceil(per_row);
    let g = core.scroll_geometry(view).unwrap();
    let expect = rows as f32 * LH;
    assert!(
        (g.content.h - expect).abs() < expect * 0.03,
        "content {} vs ~{} rows",
        g.content.h,
        rows
    );
}

#[test]
fn scrolling_down_shapes_what_scrolls_in_and_draws_it_in_the_view() {
    let mut core = Core::new();
    let text = line(100_000);
    wrapped_frame(&mut core, &text, TextWrap::Word, None);
    let before = core.text_cache_len();
    let (_, _, glyphs) = wrapped_frame(&mut core, &text, TextWrap::Word, Some(12_000.0));
    let (_, _, glyphs2) = wrapped_frame(&mut core, &text, TextWrap::Word, Some(12_000.0));
    assert!(glyphs > 0 && glyphs == glyphs2, "{glyphs} / {glyphs2}");
    assert!(core.text_cache_len() > before);
    assert!(
        core.text_cache_len() < before + 8,
        "{}",
        core.text_cache_len()
    );
    let (dl, _) = core.output();
    assert!(
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::GlyphMask)
            .all(|q| q.rect.y + q.rect.h > 0.0 && q.rect.y < 100.0),
        "every glyph drawn is inside the view"
    );
}

#[test]
fn the_queries_answer_on_rows() {
    let mut core = Core::new();
    let text = line(100_000);
    let (_, node, _) = wrapped_frame(&mut core, &text, TextWrap::Word, None);
    // Where byte 50_000 is, by the estimate: far down. Scroll it in.
    let guess = core.caret_rect(node, 50_000).unwrap();
    assert!(guess.y > 5_000.0, "{guess:?}");
    let scroll = guess.y - 20.0;
    wrapped_frame(&mut core, &text, TextWrap::Word, Some(scroll));
    wrapped_frame(&mut core, &text, TextWrap::Word, Some(scroll));
    // Now exact: the caret is on a row inside the view, inside the box,
    // and the hit test lands back on the byte, naming the row.
    let r = core.caret_rect(node, 50_000).unwrap();
    assert!(r.y > -LH && r.y < 100.0, "{r:?}");
    assert!(r.x >= 0.0 && r.x < VIEW_W, "{r:?}");
    assert_eq!(r.h, LH);
    let hit = core
        .text_hit(node, Vec2::new(r.x + 1.0, r.y + LH / 2.0))
        .unwrap();
    assert_eq!(hit.byte, 50_000);
    assert!(
        (hit.line as f32 * LH - scroll - r.y).abs() < 0.5,
        "row {} vs y {}",
        hit.line,
        r.y
    );
    // The next byte's caret is one cell on, or at the start of the next
    // row; the end is on the last row.
    let next = core.caret_rect(node, 50_001).unwrap();
    assert!(
        next.y == r.y && next.x > r.x || next.y == r.y + LH && next.x == 0.0,
        "{next:?}"
    );
    let end = core.caret_rect(node, text.len()).unwrap();
    assert!(end.y > r.y, "{end:?}");
}

#[test]
fn measurement_matches_layout_for_a_wrapped_long_line() {
    let mut core = Core::new();
    let text = line(100_000);
    // Two frames: the first frame's emission shapes the overscan chunk
    // after layout measured, which moves the estimate a little (the
    // documented tolerance); the second frame's layout has caught up.
    wrapped_frame(&mut core, &text, TextWrap::Word, None);
    let (view, _, _) = wrapped_frame(&mut core, &text, TextWrap::Word, None);
    let g = core.scroll_geometry(view).unwrap();
    let m = core.measure_text(&text, &wrapped(TextWrap::Word), Some(VIEW_W));
    assert_eq!(m.width, VIEW_W);
    assert_eq!(m.height, g.content.h);
    assert_eq!(m.lines as f32 * LH, m.height);
    // Without a width it is one row, and a box it fits in keeps it one.
    let one = core.measure_text(&text, &wrapped(TextWrap::Word), None);
    assert_eq!(one.lines, 1);
    assert_eq!(one.height, LH);
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

// ---- rich (C42) ------------------------------------------------------

fn red() -> Color {
    Color::rgb8(200, 40, 40)
}
fn blue() -> Color {
    Color::rgb8(0, 0, 80)
}

/// `text` as three spans: a red run at `at..at + 20`, a blue background
/// under `at + 20..at + 40`, plain either side.
fn spans(text: &str, at: usize) -> Vec<Span<'_>> {
    vec![
        Span::new(&text[..at]),
        Span::new(&text[at..at + 20]).color(red()),
        Span::new(&text[at + 20..at + 40]).bg(blue()),
        Span::new(&text[at + 40..]),
    ]
}

/// The spans in the horizontally scrolling view of [`frame`]; returns the
/// view's key, the node's, the glyph quads drawn, and the quads of the
/// two marks: red glyphs and blue solids.
fn rich_frame(
    core: &mut Core,
    spans: &[Span<'_>],
    scroll_x: Option<f32>,
) -> (Key, Key, usize, usize, Vec<kui_core::Rect>) {
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
                ui.rich_text(spans, mono())
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
    let red = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::GlyphMask && q.color == red())
        .count();
    let blue = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == blue())
        .map(|q| q.rect)
        .collect();
    (view, view.str("row").index(0), glyphs, red, blue)
}

#[test]
fn a_rich_line_past_the_threshold_is_chunked_too() {
    let mut core = Core::new();
    let text = line(50_000);
    let w = cell(&mut core);
    let marked = spans(&text, 50);
    let (view, _, glyphs, red, blue) = rich_frame(&mut core, &marked, None);
    // One long line, a few chunks, the screenful of glyphs — and the
    // marks where the spans put them: twenty red glyphs at cell 50 (two
    // are spaces), one blue rect twenty cells wide after them.
    assert_eq!(core.long_lines(), 1);
    assert!(
        core.text_cache_len() <= 6,
        "{} entries shaped",
        core.text_cache_len()
    );
    let visible = (VIEW_W / w).ceil() as usize;
    assert!(glyphs <= visible + 4, "{glyphs} glyphs for {visible} cells");
    assert!((17..=20).contains(&red), "{red} red glyphs");
    assert_eq!(blue.len(), 1, "{blue:?}");
    assert!((blue[0].x - 70.0 * w).abs() < 1.0, "{} vs {}", blue[0].x, 70.0 * w);
    assert!((blue[0].w - 20.0 * w).abs() < 1.0, "{} vs {}", blue[0].w, 20.0 * w);
    assert!(
        core.text_cache_bytes() < 2 << 20,
        "{} bytes",
        core.text_cache_bytes()
    );
    // The line's width is the content's, as a plain long line's is.
    let g = core.scroll_geometry(view).unwrap();
    assert!((g.content.w - text.len() as f32 * w).abs() < 2.0, "{}", g.content.w);

    // A span in a chunk that never showed draws when it scrolls in, at
    // its place, and the cache grew by the chunks that came in.
    let before = core.text_cache_len();
    let (_, _, _, _, blue) = rich_frame(&mut core, &spans(&text, 30_000), Some(30_000.0 * w));
    assert_eq!(blue.len(), 1, "{blue:?}");
    assert!((blue[0].x - 20.0 * w).abs() < 2.0, "{}", blue[0].x);
    assert_eq!(core.long_lines(), 2);
    assert!(core.text_cache_len() <= before + 6, "{}", core.text_cache_len());
}

#[test]
fn a_caret_span_moving_along_a_rich_line_reshapes_one_chunk() {
    let mut core = Core::new();
    let text = line(50_000);
    let w = cell(&mut core);
    rich_frame(&mut core, &spans(&text, 30_000), Some(30_000.0 * w));
    rich_frame(&mut core, &spans(&text, 30_000), Some(30_000.0 * w));
    let before = core.text_cache_len();
    rich_frame(&mut core, &spans(&text, 30_001), Some(30_000.0 * w));
    // A new long line (its spans are its key), whose chunks all hit but
    // the one the span moved in — and the first, which every line shapes
    // for its metrics.
    assert_eq!(core.long_lines(), 2);
    assert!(
        core.text_cache_len() <= before + 3,
        "{before} -> {}: more than the moved chunk reshaped",
        core.text_cache_len()
    );
}

#[test]
fn a_short_rich_text_takes_the_path_it_always_took() {
    let mut core = Core::new();
    let text = line(LONG_LINE_BYTES - 1);
    rich_frame(&mut core, &spans(&text, 50), None);
    assert_eq!(core.long_lines(), 0);
    assert_eq!(core.text_cache_len(), 1);
    // A span with a line break of its own keeps the whole path too.
    let text = format!("{}\n{}", line(3000), line(3000));
    rich_frame(&mut core, &[Span::new(&text)], None);
    assert_eq!(core.long_lines(), 0);
}

#[test]
fn a_wrapped_rich_line_draws_a_background_on_every_row_it_covers() {
    let mut core = Core::new();
    let text = line(20_000);
    let w = cell(&mut core);
    let per_row = (VIEW_W / w).floor() as usize;
    // A background over a little more than two rows' worth of text,
    // starting on the first row: three rects, one a row, each on its
    // own row, together as wide as the span.
    let end = per_row * 2 + per_row / 2;
    let spans = [
        Span::new(&text[..10]),
        Span::new(&text[10..end]).bg(blue()),
        Span::new(&text[end..]),
    ];
    let view = Key::ROOT.str("view");
    let mut ui = core.frame(Size::new(VIEW_W, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::rgb8(0, 0, 0)));
    ui.with_keyed(
        "view",
        NodeSpec::column()
            .width(Sizing::Fixed(VIEW_W))
            .height(Sizing::Fixed(100.0))
            .scroll_y(),
        |ui| {
            ui.with_keyed(
                "row",
                NodeSpec::column().width(Sizing::Fixed(VIEW_W)),
                |ui| ui.rich_text(&spans, wrapped(TextWrap::Word)),
            );
        },
    );
    ui.finish();
    assert_eq!(core.long_lines(), 1);
    let (dl, _) = core.output();
    let mut blue: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == blue())
        .map(|q| q.rect)
        .collect();
    blue.sort_by(|a, b| a.y.total_cmp(&b.y));
    assert_eq!(blue.len(), 3, "{blue:?}");
    for (i, r) in blue.iter().enumerate() {
        assert!((r.y - i as f32 * LH).abs() < 1.0, "{blue:?}");
        // A row's trailing space hangs past the edge, and the background
        // follows it there, as it does on the whole path.
        assert!(r.x >= -1.0 && r.x + r.w <= VIEW_W + w + 1.0, "{blue:?}");
    }
    assert!((blue[0].x - 10.0 * w).abs() < 1.0, "{blue:?}");
    assert!(blue[1].x < 1.0, "{blue:?}");
    let total: f32 = blue.iter().map(|r| r.w).sum();
    let expect = (end - 10) as f32 * w;
    assert!(
        (total - expect).abs() < expect * 0.05,
        "{total} vs {expect} for {blue:?}"
    );
    let g = core.scroll_geometry(view).unwrap();
    assert!(g.content.h > 10.0 * LH, "{}", g.content.h);
}
