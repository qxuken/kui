//! `TextWrap::BreakSpaces` (backlog F106): whitespace takes its room like
//! any glyph, so a space that does not fit starts the next row — none
//! hangs past the edge, none vanishes at the break, and every byte has a
//! caret place inside the box. kawoosh's wrapped markdown line put the
//! caret of a space typed at a full row's end past its pane, then back on
//! the dot, then on the next row, as cosmic-text's `Word` hung the first
//! space and dropped the next two.

use kui_core::{
    Align, Color, Core, Key, NodeSpec, QuadKind, Rect, Role, Size, Sizing, Span, TextHit,
    TextStyle, TextWrap, Ui, Vec2,
};

const LH: f32 = 20.0;
const CELLS: f32 = 12.0;

fn mono(wrap: TextWrap) -> TextStyle {
    TextStyle::new(14.0).mono().line_height(LH).wrap(wrap)
}

fn cell(core: &mut Core) -> f32 {
    core.measure_text("M", &mono(TextWrap::None), None).width
}

/// `text` in a node `CELLS` cells wide at the origin; the text's key.
fn frame(core: &mut Core, text: &str, wrap: TextWrap) -> Key {
    let w = cell(core);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.text_in_keyed(
        "line",
        NodeSpec::row().width(Sizing::Fixed(CELLS * w + 0.5)),
        text,
        mono(wrap),
    );
    ui.finish();
    key.index(0)
}

/// Every byte's caret, as (row, cells in).
fn carets(core: &mut Core, key: Key, len: usize) -> Vec<(usize, f32)> {
    let w = cell(core);
    (0..=len)
        .map(|b| {
            let r = core.caret_rect(key, b).expect("drawn");
            ((r.y / LH).round() as usize, (r.x / w * 10.0).round() / 10.0)
        })
        .collect()
}

#[test]
fn a_space_typed_at_a_full_rows_end_starts_the_next_row() {
    let mut core = Core::new();
    // "aaa bbb one." is the row, exactly; then one, two, three spaces.
    for n in 0..4 {
        let text = format!("aaa bbb one.{}", " ".repeat(n));
        let key = frame(&mut core, &text, TextWrap::BreakSpaces);
        let at = carets(&mut core, key, text.len());
        let mut expect: Vec<(usize, f32)> = (0..=12).map(|i| (0, i as f32)).collect();
        if n > 0 {
            // The first space opens the second row: the caret before it is
            // there, and each space after moves the caret one cell on.
            expect[12] = (1, 0.0);
            expect.extend((1..=n).map(|i| (1, i as f32)));
        }
        assert_eq!(at, expect, "{n} spaces");
        let w = cell(&mut core);
        let m = core.measure_text(&text, &mono(TextWrap::BreakSpaces), Some(CELLS * w + 0.5));
        assert_eq!(m.height, if n > 0 { 2.0 * LH } else { LH }, "{n} spaces");
    }
}

#[test]
fn a_space_that_fits_stays_on_its_row_and_a_run_that_does_not_carries_on() {
    let mut core = Core::new();
    // "aaa " fits the row and keeps its space; "bbb " the next.
    let text = "aaa bbb ccc";
    let key = frame(&mut core, text, TextWrap::BreakSpaces);
    let at = carets(&mut core, key, text.len());
    assert_eq!(at[3], (0, 3.0), "the space after aaa is on its row");
    assert_eq!(at[4], (0, 4.0));
    assert_eq!(at[8], (0, 8.0));
    // Spaces past the row's end are a row of their own before the word:
    // twelve letters, then three spaces, then a word.
    let text = "abcdefghijkl   xy";
    let key = frame(&mut core, text, TextWrap::BreakSpaces);
    let at = carets(&mut core, key, text.len());
    assert_eq!(
        &at[12..],
        &[(1, 0.0), (1, 1.0), (1, 2.0), (1, 3.0), (1, 4.0), (1, 5.0)]
    );
    // Every caret of every text is inside the box, in reading order.
    for text in [
        "aaa bbb ccc",
        "a  b  c  d  e  f  g  h",
        "abcdefghijkl   xy",
        "   leading",
    ] {
        let key = frame(&mut core, text, TextWrap::BreakSpaces);
        let at = carets(&mut core, key, text.len());
        for (b, p) in at.iter().enumerate() {
            assert!(p.1 <= CELLS, "{text:?} byte {b} at {p:?}");
        }
        for pair in at.windows(2) {
            assert!(pair[0] < pair[1], "{text:?}: {at:?}");
        }
    }
}

#[test]
fn a_block_caret_on_a_wrapped_space_draws_where_the_space_is() {
    // An editor draws its block caret as the background of the character
    // under it: on the space opening the second row, inside the box.
    let mut core = Core::new();
    let w = cell(&mut core);
    let caret = Color::rgb8(255, 200, 0);
    let text = "aaa bbb one. ";
    let spans = [Span::new(&text[..12]), Span::new(&text[12..]).bg(caret)];
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("row", NodeSpec::column().width(CELLS * w + 0.5), |ui| {
        ui.rich_text(&spans, mono(TextWrap::BreakSpaces))
    });
    ui.finish();
    let (dl, _) = core.output();
    let bg: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == caret)
        .map(|q| q.rect)
        .collect();
    assert_eq!(bg.len(), 1, "{bg:?}");
    assert!(bg[0].x.abs() < 1.0 && (bg[0].y - LH).abs() < 1.0, "{bg:?}");
    assert!((bg[0].w - w).abs() < 1.0, "{bg:?}");
}

#[test]
fn word_still_hangs_its_trailing_space() {
    // `Word` is unchanged: a UI label's trailing space hangs, as a
    // browser's does, and the row is the one row.
    let mut core = Core::new();
    let text = "aaa bbb one. ";
    let key = frame(&mut core, text, TextWrap::Word);
    let at = carets(&mut core, key, text.len());
    assert_eq!(at[12], (0, 12.0));
    let w = cell(&mut core);
    let m = core.measure_text(text, &mono(TextWrap::Word), Some(CELLS * w + 0.5));
    assert_eq!(m.height, LH);
}

// ---- the alpha.22 regression pass ---------------------------------------
//
// Every non-empty `break-spaces` text takes the long line's path (C19), so
// what that path allowed itself for texts past 4 KB reaches an editor's
// short lines: these pin a short `break-spaces` line doing what a `word`
// one does.

/// `text` in a node `cells` cells wide, keyed `label`; the text's key.
fn text_at(ui: &mut Ui<'_>, label: &str, text: &str, cells: f32, w: f32, wrap: TextWrap) -> Key {
    ui.text_in_keyed(
        label,
        NodeSpec::row().width(Sizing::Fixed(cells * w + 0.5)),
        text,
        mono(wrap),
    )
    .index(0)
}

/// The glyph quads drawn with their top inside `y0..y1`, in emission order.
fn glyphs(core: &mut Core, y0: f32, y1: f32) -> Vec<Rect> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::GlyphMask && q.rect.y >= y0 && q.rect.y < y1)
        .map(|q| q.rect)
        .collect()
}

/// The rows glyphs sit on, counted from `y0`.
fn glyph_rows(rects: &[Rect], y0: f32) -> Vec<u32> {
    let mut rows: Vec<u32> = rects
        .iter()
        .map(|r| ((r.y - y0) / LH).floor() as u32)
        .collect();
    rows.sort_unstable();
    rows.dedup();
    rows
}

#[test]
fn one_text_at_two_widths_draws_and_answers_at_each() {
    // The same file open in two panes of different widths: one cache
    // entry behind two nodes, each drawn and queried at its own width.
    let mut core = Core::new();
    let w = cell(&mut core);
    let text = "aaa bbb ccc";
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let narrow = text_at(&mut ui, "narrow", text, 6.0, w, TextWrap::BreakSpaces);
    ui.leaf(NodeSpec::column().height(100.0));
    let wide = text_at(&mut ui, "wide", text, 20.0, w, TextWrap::BreakSpaces);
    ui.finish();
    // "aaa ", "bbb ", "ccc" in six cells; one row in twenty.
    assert_eq!(glyph_rows(&glyphs(&mut core, 0.0, 100.0), 0.0), [0, 1, 2]);
    assert_eq!(glyph_rows(&glyphs(&mut core, 100.0, 300.0), 160.0), [0]);
    let row_x = |r: Rect, y0: f32| (((r.y - y0) / LH).round(), (r.x / w).round());
    assert_eq!(row_x(core.caret_rect(narrow, 8).unwrap(), 0.0), (2.0, 0.0));
    assert_eq!(row_x(core.caret_rect(wide, 8).unwrap(), 160.0), (0.0, 8.0));
    assert_eq!(
        core.text_hit(narrow, Vec2::new(1.2 * w, LH + 5.0)),
        Some(TextHit { byte: 5, line: 1 })
    );
    assert_eq!(
        core.text_hit(wide, Vec2::new(9.2 * w, 165.0)),
        Some(TextHit { byte: 9, line: 0 })
    );
    // Measuring the text at a third width leaves both as they were.
    let m = core.measure_text(text, &mono(TextWrap::BreakSpaces), Some(3.0 * w + 0.5));
    assert!(m.lines > 3, "{m:?}");
    assert_eq!(row_x(core.caret_rect(narrow, 8).unwrap(), 0.0), (2.0, 0.0));
    assert_eq!(row_x(core.caret_rect(wide, 8).unwrap(), 160.0), (0.0, 8.0));
}

#[test]
fn a_line_of_several_texts_answers_across_them() {
    // A `line` row of two texts answers `caret_rect` and `text_hit` by
    // the row's key across both, under `break-spaces` as under `word`.
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces] {
        let mut core = Core::new();
        let w = cell(&mut core);
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let line = ui.with_keyed("l", NodeSpec::row().role(Role::Line), |ui| {
            ui.text("foo ", mono(wrap));
            ui.text("bar", mono(wrap));
        });
        ui.finish();
        for b in 0..=7 {
            let r = core.caret_rect(line, b).expect("drawn");
            assert_eq!(
                ((r.x / w).round(), r.y),
                (b as f32, 0.0),
                "{wrap:?} byte {b}"
            );
        }
        for b in 0..7 {
            let hit = core.text_hit(line, Vec2::new((b as f32 + 0.2) * w, 5.0));
            assert_eq!(hit, Some(TextHit { byte: b, line: 0 }), "{wrap:?} byte {b}");
        }
    }
}

#[test]
fn a_word_at_a_chunk_edge_starts_its_row() {
    // Ten cells a row, and "aaaaaaaaa " fills one exactly: 1.5 KB is two
    // chunks, the second's first word starting after a row the first
    // chunk filled.
    let mut core = Core::new();
    let w = cell(&mut core);
    let text = "aaaaaaaaa ".repeat(150);
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(400.0, 3200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        text_at(&mut ui, "t", &text, 10.0, w, TextWrap::BreakSpaces);
        ui.finish();
    }
    let key = Key::ROOT.str("t").index(0);
    let g = glyphs(&mut core, -1.0, 3200.0);
    assert_eq!(g.len(), 1350);
    let outside: Vec<_> = g.iter().filter(|r| r.x + r.w > 10.0 * w + 1.0).collect();
    assert!(outside.is_empty(), "{outside:?}");
    for b in (0..=text.len()).step_by(7).chain(1015..1026) {
        let r = core.caret_rect(key, b).unwrap();
        let at = ((r.y / LH).round() as usize, (r.x / w).round() as usize);
        let expect = if b == text.len() {
            (149, 10)
        } else {
            (b / 10, b % 10)
        };
        assert_eq!(at, expect, "byte {b}");
    }
}

#[test]
fn a_right_to_left_paragraph_wraps_as_word_does() {
    let mut core = Core::new();
    let text = "שלום עולם ".repeat(4);
    let style = |wrap| TextStyle::new(14.0).line_height(LH).wrap(wrap);
    let bw = core.measure_text(&text, &style(TextWrap::Word), None).width / 3.0;
    let mut drawn = Vec::new();
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces] {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.text_in_keyed(
            "t",
            NodeSpec::row().width(Sizing::Fixed(bw)),
            &text,
            style(wrap),
        );
        ui.finish();
        let g = glyphs(&mut core, -1.0, 300.0);
        let inside = g.iter().all(|r| r.x >= -1.0 && r.x + r.w <= bw + 1.0);
        let lines = core.measure_text(&text, &style(wrap), Some(bw)).lines;
        drawn.push((g.len(), glyph_rows(&g, 0.0).len(), inside, lines));
    }
    assert!(drawn[0].3 > 1, "the paragraph wraps: {drawn:?}");
    assert_eq!(drawn[0], drawn[1], "word, then break-spaces");
}

#[test]
fn the_first_frame_of_a_paragraph_past_a_chunk_is_laid_out_exactly() {
    // A proportional paragraph of two chunks: the first frame's box is the
    // one the next frame has, and the one measurement gives.
    let mut core = Core::new();
    let text = "iiii WWWW mm ll ".repeat(100);
    let style = TextStyle::new(14.0)
        .line_height(LH)
        .wrap(TextWrap::BreakSpaces);
    let mut below = Vec::new();
    for _ in 0..2 {
        let mut ui = core.frame(Size::new(400.0, 3000.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.text_in_keyed(
            "t",
            NodeSpec::column().width(Sizing::Fixed(200.0)),
            &text,
            style,
        );
        ui.leaf_keyed("below", NodeSpec::column().height(10.0).on_layout("l"));
        ui.finish();
        below.push(core.layout_of(Key::ROOT.str("below")).unwrap().y);
    }
    let m = core.measure_text(&text, &style, Some(200.0));
    assert_eq!(below, [m.height, m.height]);
}

#[test]
fn a_tight_line_height_does_not_cut_the_glyphs() {
    // `word` draws ascenders and descenders that overhang a row shorter
    // than the font under its parent's clip, and so does `break-spaces`.
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces] {
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill().pad(20.0));
        ui.text(
            "Égypt jumps",
            TextStyle::new(24.0).line_height(14.0).wrap(wrap),
        );
        ui.finish();
        let (dl, _) = core.output();
        let cut: Vec<_> = dl
            .quads
            .iter()
            .filter(|q| q.kind == QuadKind::GlyphMask)
            .filter(|q| {
                let c = dl.clips[q.clip as usize].rect;
                q.rect.y < c.y || q.rect.y + q.rect.h > c.y + c.h
            })
            .map(|q| q.rect)
            .collect();
        assert!(cut.is_empty(), "{wrap:?}: {cut:?}");
    }
}

#[test]
fn a_wrapped_text_claims_its_box_and_not_its_line_in_a_selection() {
    // A drag starting on the text beside a wrapped one starts there: the
    // wrapped text's box is its width, not its unwrapped line's.
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces] {
        let mut core = Core::new();
        let w = cell(&mut core);
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let scope = ui.with_keyed("scope", NodeSpec::row().selectable(), |ui| {
            text_at(ui, "a", "aaa bbb ccc ddd", 6.0, w, wrap);
            text_at(ui, "b", "xyz", 10.0, w, TextWrap::Word);
        });
        ui.finish();
        let bx = 6.0 * w + 0.5;
        assert!(core.begin_selection(scope, Vec2::new(bx + 1.2 * w, 5.0)));
        core.extend_selection(Vec2::new(bx + 3.5 * w, 5.0));
        assert_eq!(core.selection_text().as_deref(), Some("yz"), "{wrap:?}");
    }
}

#[test]
fn a_break_spaces_text_sits_on_the_baseline_a_word_one_does() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(NodeSpec::row().cross_align(Align::Baseline), |ui| {
        ui.text("H", TextStyle::new(14.0).line_height(LH));
        let bs = TextStyle::new(14.0)
            .line_height(LH)
            .wrap(TextWrap::BreakSpaces);
        ui.text("H", bs);
        ui.text("H", TextStyle::new(30.0).line_height(40.0));
    });
    ui.finish();
    let g = glyphs(&mut core, -1.0, 300.0);
    assert_eq!(g.len(), 3);
    assert_eq!(g[0].y + g[0].h, g[1].y + g[1].h, "{g:?}");
}

#[test]
fn a_hit_past_a_rows_end_stays_on_that_row() {
    // "aaa bbb ccc " fills the twelve cells and "ddd" is the next row: a
    // press past a row's end is on that row, and so is the caret for the
    // byte it answers.
    for wrap in [TextWrap::Word, TextWrap::BreakSpaces] {
        let mut core = Core::new();
        let key = frame(&mut core, "aaa bbb ccc ddd", wrap);
        for (y, row) in [(5.0, 0), (LH + 5.0, 1)] {
            let hit = core.text_hit(key, Vec2::new(390.0, y)).unwrap();
            assert_eq!(hit.line, row, "{wrap:?}: {hit:?}");
            let caret = core.caret_rect(key, hit.byte).unwrap();
            assert_eq!((caret.y / LH).round() as u32, row, "{wrap:?}: {hit:?}");
        }
    }
    // A word broken by glyph: the press lands before the row's last one.
    let mut core = Core::new();
    let key = frame(&mut core, "abcdefghijklmnop", TextWrap::BreakSpaces);
    let hit = core.text_hit(key, Vec2::new(390.0, 5.0)).unwrap();
    assert_eq!(hit, TextHit { byte: 11, line: 0 });
}
