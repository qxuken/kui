//! `TextStyle::initial_letter` (backlog F164, from Carnet's "First
//! letter" setting): the first letter as a drop cap, as tall as a few
//! lines, the lines beside it moved in past it and the ones below at full
//! width again — one text still, so a caret, a hit, a selection's rects
//! and the access tree read every byte where it is drawn. Carnet could
//! only make the letter large on its first line, which stood that line
//! apart from the next.

use kui_core::{
    Color, Core, FontFamily, Key, NodeSpec, QuadKind, Rect, Role, Size, Sizing, Span, TextHit,
    TextStyle, Vec2,
};

const LH: f32 = 20.0;
const WIDTH: f32 = 240.0;
const TEXT: &str = "Once upon a time there was a letter that dropped three lines \
                    down the page, and the words went round it and then on below \
                    it as if nothing had happened at all, line after line.";

fn style(lines: u32) -> TextStyle {
    TextStyle::new(14.0).line_height(LH).initial_letter(lines)
}

/// `text` in a node `WIDTH` wide at (10, 20); the text's key.
fn frame(core: &mut Core, text: &str, style: TextStyle, width: f32) -> Key {
    let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    ui.leaf(NodeSpec::column().height(10.0));
    let key = ui.text_in_keyed(
        "para",
        NodeSpec::row().width(Sizing::Fixed(width)),
        text,
        style,
    );
    ui.finish();
    key.index(0)
}

/// Each row's top and the x its first byte's caret is at, relative to
/// the text's origin, from every byte's caret: the letter's own caret
/// (byte 0) left out.
fn rows(core: &mut Core, key: Key, text: &str) -> Vec<(f32, f32)> {
    let o = core.caret_rect(key, 0).expect("drawn");
    let mut rows: Vec<(f32, f32)> = Vec::new();
    for b in 1..=text.len() {
        if !text.is_char_boundary(b) {
            continue;
        }
        let r = core.caret_rect(key, b).expect("drawn");
        let (y, x) = (r.y - o.y, r.x - o.x);
        match rows.last_mut() {
            Some(last) if (last.0 - y).abs() < 0.5 => last.1 = last.1.min(x),
            _ => rows.push((y, x)),
        }
    }
    rows
}

#[test]
fn the_lines_beside_the_letter_are_moved_in_and_the_rest_are_not() {
    let mut core = Core::new();
    let key = frame(&mut core, TEXT, style(3), WIDTH);
    let rows = rows(&mut core, key, TEXT);
    assert!(rows.len() >= 6, "the paragraph wraps: {rows:?}");
    let indent = rows[0].1;
    assert!(
        indent > 20.0,
        "the first row starts past the letter: {rows:?}"
    );
    for (i, &(top, x)) in rows.iter().enumerate() {
        assert_eq!(top, i as f32 * LH, "row {i} on the pitch: {rows:?}");
        let want = if i < 3 { indent } else { 0.0 };
        assert_eq!(x, want, "row {i}: {rows:?}");
    }
    // The text's height is its rows, the letter inside them.
    let m = core.measure_text(TEXT, &style(3), Some(WIDTH));
    assert_eq!(m.height, rows.len() as f32 * LH);
    // Without the initial the same text starts every row at 0, and
    // fills its first rows further.
    let key = frame(&mut core, TEXT, style(0), WIDTH);
    let plain = self::rows(&mut core, key, TEXT);
    assert!(plain[1..].iter().all(|r| r.1 == 0.0), "{plain:?}");
    assert!(plain.len() <= rows.len());
}

#[test]
fn the_letter_is_as_tall_as_the_lines_it_drops_through() {
    let mut core = Core::new();
    for lines in [2, 3, 4] {
        let key = frame(&mut core, TEXT, style(lines), WIDTH);
        let o = core.caret_rect(key, 0).expect("drawn");
        // Its caret, before it, reaches from the first row's top to the
        // bottom of the last one it drops through.
        assert_eq!((o.x, o.y), (10.0, 20.0), "{lines} lines");
        assert_eq!(o.h, lines as f32 * LH, "{lines} lines");
        // The row it ends on is the last one moved in.
        let rows = rows(&mut core, key, TEXT);
        assert!(rows[lines as usize - 1].1 > 0.0, "{lines} lines: {rows:?}");
        assert_eq!(rows[lines as usize].1, 0.0, "{lines} lines: {rows:?}");
    }
    // A short text is still that tall: the letter's box is.
    let m = core.measure_text("Oh", &style(3), Some(WIDTH));
    assert_eq!(m.height, 3.0 * LH);
    let m = core.measure_text("Oh", &style(3), None);
    assert_eq!(m.height, 3.0 * LH);
}

#[test]
fn the_letters_glyph_is_drawn_large_with_its_foot_on_the_last_lines_baseline() {
    let mut core = Core::new();
    frame(&mut core, TEXT, style(3), WIDTH);
    let (dl, _) = core.output();
    let glyphs: Vec<Rect> = dl
        .quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
        .map(|q| q.rect)
        .collect();
    let tallest = glyphs
        .iter()
        .copied()
        .max_by(|a, b| a.h.total_cmp(&b.h))
        .expect("glyphs");
    // An `O` two lines and a cap height tall, give or take its overshoot,
    // at the text's left.
    assert!(tallest.h > 2.0 * LH && tallest.h < 3.0 * LH, "{tallest:?}");
    assert!((tallest.x - 10.0).abs() < 4.0, "{tallest:?}");
    // Its foot is on the third row's baseline, where that row's letters
    // stand: the bottom of an `n` on it.
    let third = 20.0 + 2.0 * LH;
    let small_on_third: Vec<&Rect> = glyphs
        .iter()
        .filter(|g| g.y >= third && g.y + g.h <= third + LH && g.h < LH)
        .collect();
    let foot = small_on_third
        .iter()
        .map(|g| g.y + g.h)
        .fold(f32::MIN, f32::max);
    assert!(
        (tallest.y + tallest.h - foot).abs() <= 2.0,
        "the letter's foot {} and the third row's {foot}",
        tallest.y + tallest.h
    );
}

#[test]
fn a_point_on_the_letter_beside_it_and_below_it_hits_the_byte_drawn_there() {
    let mut core = Core::new();
    let key = frame(&mut core, TEXT, style(3), WIDTH);
    let o = core.caret_rect(key, 0).expect("drawn");
    let after = core.caret_rect(key, 1).expect("drawn");
    let indent = after.x - o.x;
    // The letter's left half is before it, its right half after it, on
    // any of the rows it drops through.
    for row in 0..3 {
        let y = o.y + row as f32 * LH + LH / 2.0;
        assert_eq!(
            core.text_hit(key, Vec2::new(o.x + 2.0, y)),
            Some(TextHit { byte: 0, line: 0 }),
            "row {row}"
        );
        let hit = core.text_hit(key, Vec2::new(o.x + indent * 0.6, y));
        assert_eq!(hit.map(|h| h.byte), Some(1), "row {row}");
    }
    // Beside it, the row there, where it is drawn: each byte's caret
    // hits its own byte back.
    for b in [1usize, 5, 20, 40, 70, 100, 140] {
        let r = core.caret_rect(key, b).expect("drawn");
        let hit = core
            .text_hit(key, Vec2::new(r.x + 0.5, r.y + LH / 2.0))
            .expect("a hit");
        assert_eq!(hit.byte, b, "caret of byte {b} at {r:?}");
        assert_eq!(hit.line, ((r.y - o.y) / LH).round() as u32, "byte {b}");
    }
}

#[test]
fn a_text_laid_out_again_at_another_width_and_back_is_where_it_was() {
    let mut core = Core::new();
    let key = frame(&mut core, TEXT, style(3), WIDTH);
    let first = rows(&mut core, key, TEXT);
    let key = frame(&mut core, TEXT, style(3), 160.0);
    let narrow = rows(&mut core, key, TEXT);
    assert!(narrow.len() > first.len());
    assert!(narrow[..3].iter().all(|r| r.1 == first[0].1));
    assert!(narrow[3..].iter().all(|r| r.1 == 0.0));
    let key = frame(&mut core, TEXT, style(3), WIDTH);
    assert_eq!(rows(&mut core, key, TEXT), first);
    // Wide enough for one row: the letter's box is the height.
    let key = frame(&mut core, "Once upon", style(3), WIDTH);
    let o = core.caret_rect(key, 0).expect("drawn");
    let end = core.caret_rect(key, 9).expect("drawn");
    assert_eq!(end.y, o.y, "one row beside the letter");
    assert_eq!(o.h, 3.0 * LH);
}

#[test]
fn rich_text_keeps_its_spans_and_the_letter_its_face_and_colour() {
    let red = Color::hex(0xc02020ff);
    let wash = Color::hex(0xffe08aff);
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let para = ui.with_keyed("para", NodeSpec::row().width(Sizing::Fixed(WIDTH)), |ui| {
        ui.rich_text(
            &[
                Span::new("“O")
                    .color(red)
                    .family(FontFamily::Serif)
                    .bg(wash),
                Span::new("nce upon a time there was a letter, ")
                    .bold()
                    .bg(wash),
                Span::new("and then a long tail of words to wrap a few rows down."),
            ],
            style(2),
        );
    });
    ui.finish();
    let key = para.index(0);
    // The quote goes with the letter: the caret after both is beside it.
    let o = core.caret_rect(key, 0).expect("drawn");
    let quote = core.caret_rect(key, 3).expect("drawn");
    let after = core.caret_rect(key, 4).expect("drawn");
    assert_eq!(
        quote.h,
        2.0 * LH,
        "inside the initial, its caret is its height"
    );
    assert!(after.x - o.x > 20.0 && after.h == LH);
    let (dl, _) = core.output();
    let reds: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.color == red && q.kind != QuadKind::Solid)
        .collect();
    assert_eq!(reds.len(), 2, "the quote and the letter, in their colour");
    assert!(reds[1].rect.h > LH, "the letter is large: {reds:?}");
    // Its background is around it, the next span's beside it on the
    // first row: each drawn where its glyphs are.
    let washes: Vec<Rect> = dl
        .quads
        .iter()
        .filter(|q| q.color == wash && q.kind == QuadKind::Solid)
        .map(|q| q.rect)
        .collect();
    let letter = reds[1].rect;
    let around = washes
        .iter()
        .find(|w| w.x <= letter.x && w.x + w.w >= letter.x + letter.w)
        .expect("the letter's background");
    assert!(around.y <= letter.y && around.y + around.h >= letter.y + letter.h);
    // As tall as the two rows it drops through, as its caret is.
    assert_eq!((around.y, around.h), (o.y, 2.0 * LH), "{around:?}");
    assert!(
        washes
            .iter()
            .any(|w| w.y == 0.0 && w.x >= after.x - 0.5 && w.h == LH),
        "the next span's, moved in: {washes:?}"
    );
}

#[test]
fn a_line_budget_a_long_line_and_a_one_line_drop_have_no_initial() {
    let mut core = Core::new();
    for s in [style(3).max_lines(10), style(3).ellipsis(), style(1)] {
        let key = frame(&mut core, TEXT, s, WIDTH);
        let rows = rows(&mut core, key, TEXT);
        assert!(rows[1..].iter().all(|r| r.1 == 0.0), "{s:?}: {rows:?}");
        assert_eq!(core.caret_rect(key, 0).expect("drawn").h, LH);
    }
    // Text that opens with no letter has none either.
    let text = format!(" {TEXT}");
    let key = frame(&mut core, &text, style(3), WIDTH);
    let rows = rows(&mut core, key, &text);
    assert!(rows[1..].iter().all(|r| r.1 == 0.0), "{rows:?}");
    assert_eq!(core.caret_rect(key, 0).expect("drawn").h, LH);
}

/// The access tree reads the paragraph as one line: its runs carry its
/// bytes in order with no newline at the cuts the initial makes — the
/// space a row's break swallows is in no run, with an initial or
/// without — and their rects are where the rows are drawn.
#[test]
fn the_access_tree_reads_one_line_where_it_is_drawn() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with_keyed(
        "sink",
        NodeSpec::column()
            .fill()
            .on_key("ed")
            .role(Role::MultilineTextInput)
            .label("Doc"),
        |ui| {
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Fixed(WIDTH))
                    .role(Role::Line)
                    .caret(0),
                |ui| {
                    ui.text(TEXT, style(3));
                },
            );
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    let node = core.access_tree().get(sink).expect("the editor").clone();
    assert_eq!(node.value.as_deref(), Some(TEXT));
    let words = |s: &str| s.split_whitespace().collect::<String>();
    let text: String = node.runs.iter().map(|r| r.text.as_str()).collect();
    assert!(!text.contains('\n'), "no newline at a cut: {text:?}");
    assert_eq!(words(&text), words(TEXT));
    assert!(node.runs.iter().all(|r| r.line == 0));
    let mut at = 0;
    for r in &node.runs {
        assert!(r.start >= at && r.end > r.start, "{r:?}");
        assert_eq!(&TEXT[r.start..r.end], r.text, "{r:?}");
        at = r.end;
    }
    assert_eq!(at, TEXT.len());
    // The letter's run first, at the left; the row beside it moved in.
    assert_eq!(node.runs[0].text, "O");
    assert_eq!(node.runs[0].rect.x, 0.0);
    let indent = node.runs[1].rect.x;
    assert!(indent > 20.0, "{:?}", node.runs[1]);
    let line = node.runs[1].rect;
    assert_eq!(line.y, 0.0, "beside the letter, on the first row");
}

/// A one-letter word as the initial — "A calm …", "I think …" — and the
/// space after it.
const WORD: &str = "A calm notes app that keeps what you write, syncs it to \
                    every device you own and reads it back to you as you \
                    wrote it, line after line after line.";

/// The space after an initial that is a word of its own is collapsed, as
/// CSS Inline Layout collapses the white space after a dropped initial
/// (§ 7.8.1): the first row beside the letter starts its word where the
/// rows below it start theirs, not a space further in. The space keeps
/// its byte — its caret is at the row's start, where the caret after it
/// is — and a copy still reads it (backlog F166, from Carnet's review).
#[test]
fn the_space_after_a_one_letter_initial_is_collapsed() {
    let mut core = Core::new();
    let key = frame(&mut core, WORD, style(3), WIDTH);
    let o = core.caret_rect(key, 0).expect("drawn");
    let before = core.caret_rect(key, 1).expect("drawn");
    let after = core.caret_rect(key, 2).expect("drawn");
    let rows = rows(&mut core, key, WORD);
    assert!(rows.len() >= 5, "{rows:?}");
    let indent = rows[1].1;
    assert!(indent > 10.0, "the second row is moved in: {rows:?}");
    // "calm" starts where "that" and the rest of the rows beside start.
    assert!(
        (after.x - o.x - indent).abs() < 0.5,
        "the first word at {} where the rows below start at {indent}: {rows:?}",
        after.x - o.x
    );
    assert!(
        (rows[2].1 - indent).abs() < 0.5,
        "every row beside at one x: {rows:?}"
    );
    // The space is in the buffer, at no width: its caret, on the first
    // row, is where the caret after it is.
    assert_eq!(before.y, after.y, "both on the first row");
    assert_eq!(before.h, LH, "beside the letter, not in it");
    assert!((after.x - before.x).abs() < 0.5, "{before:?} {after:?}");
    // Without the space the first row starts at the same x: the space
    // takes nothing.
    let glued = WORD.replacen(' ', "", 1);
    let key = frame(&mut core, &glued, style(3), WIDTH);
    let g = core.caret_rect(key, 1).expect("drawn");
    let go = core.caret_rect(key, 0).expect("drawn");
    assert!(
        (g.x - go.x - (after.x - o.x)).abs() < 0.5,
        "{g:?} {after:?}"
    );
    // A tab, or a run of spaces, collapses as one space does; a no-break
    // space is kept, as CSS keeps it.
    for (lead, collapses) in [("\t", true), ("   ", true), ("\u{a0}", false)] {
        let text = WORD.replacen(' ', lead, 1);
        let key = frame(&mut core, &text, style(3), WIDTH);
        let o = core.caret_rect(key, 0).expect("drawn");
        let word = core.caret_rect(key, 1 + lead.len()).expect("drawn");
        let at = word.x - o.x;
        assert_eq!(
            (at - indent).abs() < 0.5,
            collapses,
            "{lead:?}: the first word at {at}, the rows beside at {indent}"
        );
    }
}

/// Over the collapsed space a hit, a selection and a background read
/// its byte where it is drawn: at the start of the first row, with no
/// width. A point in the gap after the letter is the caret after the
/// letter; a point on the first word's left half is that word's start.
#[test]
fn a_hit_a_selection_and_a_background_over_the_collapsed_space() {
    let mut core = Core::new();
    let key = frame(&mut core, WORD, style(3), WIDTH);
    let o = core.caret_rect(key, 0).expect("drawn");
    let after = core.caret_rect(key, 2).expect("drawn");
    let y = o.y + LH / 2.0;
    // The gap's right half, before the first word: the letter's end.
    let hit = core.text_hit(key, Vec2::new(after.x - 1.0, y));
    assert_eq!(hit.map(|h| h.byte), Some(1), "{after:?}");
    // The first word's first letter, its left half: the word's start.
    let hit = core.text_hit(key, Vec2::new(after.x + 1.0, y));
    assert_eq!(hit.map(|h| h.byte), Some(2), "{after:?}");

    // A selection over "A c" copies the space, and paints nothing in the
    // gap between the letter and the word.
    let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    ui.leaf(NodeSpec::column().height(10.0));
    let mut para = None;
    let scope = ui.with_keyed("scope", NodeSpec::column().selectable(), |ui| {
        para = Some(ui.text_in_keyed(
            "para",
            NodeSpec::row().width(Sizing::Fixed(WIDTH)),
            WORD,
            style(3),
        ));
    });
    ui.finish();
    let para = para.expect("built").index(0);
    core.set_selection(kui_core::Selection::new(
        scope,
        kui_core::Endpoint::new(para, 0),
        kui_core::Endpoint::new(para, 3),
    ));
    assert_eq!(core.selection_text().as_deref(), Some("A c"));
    let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    ui.leaf(NodeSpec::column().height(10.0));
    ui.with_keyed("scope", NodeSpec::column().selectable(), |ui| {
        ui.text_in_keyed(
            "para",
            NodeSpec::row().width(Sizing::Fixed(WIDTH)),
            WORD,
            style(3),
        );
    });
    ui.finish();
    let o = core.caret_rect(para, 0).expect("drawn");
    let after = core.caret_rect(para, 2).expect("drawn");
    let tint = core.theme().selection;
    let (dl, _) = core.output();
    let picked: Vec<Rect> = dl
        .quads
        .iter()
        .filter(|q| q.color == tint && q.kind == QuadKind::Solid)
        .map(|q| q.rect)
        .collect();
    assert!(!picked.is_empty(), "the selection is painted");
    for r in &picked {
        let letter = r.x < o.x + 0.5 && r.h > LH;
        let beside = r.x >= after.x - 0.5 && r.y == o.y;
        assert!(
            letter || beside || r.w < 0.5,
            "a selection rect in the gap: {r:?}, the letter at {o:?}, the word at {after:?}: {picked:?}"
        );
    }

    // A background on the space alone is no wider than it is.
    let wash = Color::hex(0xffe08aff);
    let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("rich", NodeSpec::row().width(Sizing::Fixed(WIDTH)), |ui| {
        ui.rich_text(
            &[
                Span::new("A"),
                Span::new(" ").bg(wash),
                Span::new(&WORD[2..]),
            ],
            style(3),
        );
    });
    ui.finish();
    let (dl, _) = core.output();
    let washes: Vec<Rect> = dl
        .quads
        .iter()
        .filter(|q| q.color == wash && q.kind == QuadKind::Solid)
        .map(|q| q.rect)
        .collect();
    assert!(washes.iter().all(|w| w.w < 0.5), "{washes:?}");
}
