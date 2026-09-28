//! `TextWrap::BreakSpaces` (backlog F106): whitespace takes its room like
//! any glyph, so a space that does not fit starts the next row — none
//! hangs past the edge, none vanishes at the break, and every byte has a
//! caret place inside the box. kawoosh's wrapped markdown line put the
//! caret of a space typed at a full row's end past its pane, then back on
//! the dot, then on the next row, as cosmic-text's `Word` hung the first
//! space and dropped the next two.

use kui_core::{Color, Core, Key, NodeSpec, QuadKind, Size, Sizing, Span, TextStyle, TextWrap};

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
