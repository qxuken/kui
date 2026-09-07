//! Underline, strikethrough and a background per span (backlog C22): paint
//! beside the glyphs, placed where the face puts its lines, one rect per
//! run of a span per line so a background follows the span across a wrap
//! the way a box around a run cannot.

use kui_core::{Color, Core, NodeSpec, QuadKind, Size, Sizing, Span, TextStyle};

const LH: f32 = 20.0;

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(LH)
}

/// The solid quads of a frame, in emission order, as (x, y, w, h, color).
fn solids(core: &mut Core) -> Vec<(f32, f32, f32, f32, Color)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .map(|q| (q.rect.x, q.rect.y, q.rect.w, q.rect.h, q.color))
        .collect()
}

fn glyph_span(core: &mut Core) -> (f32, f32) {
    let (dl, _) = core.output();
    let xs: Vec<(f32, f32)> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::GlyphMask)
        .map(|q| (q.rect.x, q.rect.x + q.rect.w))
        .collect();
    let lo = xs.iter().map(|x| x.0).fold(f32::INFINITY, f32::min);
    let hi = xs.iter().map(|x| x.1).fold(f32::NEG_INFINITY, f32::max);
    (lo, hi)
}

#[test]
fn an_underline_and_a_strikethrough_are_lines_where_the_face_puts_them() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text("hello", mono().underline().strikethrough());
    ui.finish();
    let lines = solids(&mut core);
    assert_eq!(
        lines.len(),
        2,
        "one underline, one strikethrough: {lines:?}"
    );
    let (lo, hi) = glyph_span(&mut core);
    let (under, strike) = (lines[0], lines[1]);
    // Both span the text and are at least a pixel thick, in the text colour.
    for l in [under, strike] {
        assert!(
            l.0 <= lo + 1.0 && l.0 + l.2 >= hi - 1.0,
            "spans the glyphs: {l:?} vs {lo}..{hi}"
        );
        assert!(l.3 >= 1.0);
        assert_eq!(l.4, mono().color);
    }
    // The underline is below the strikethrough, and both within the line.
    assert!(
        under.1 > strike.1,
        "underline {} below strikethrough {}",
        under.1,
        strike.1
    );
    assert!(under.1 + under.3 <= LH + 1.0 && strike.1 >= 0.0);
    // Lines paint after the glyphs.
    let (dl, _) = core.output();
    let first_solid = dl
        .quads
        .iter()
        .position(|q| q.kind == QuadKind::Solid)
        .unwrap();
    let last_glyph = dl
        .quads
        .iter()
        .rposition(|q| q.kind == QuadKind::GlyphMask)
        .unwrap();
    assert!(first_solid > last_glyph);
}

#[test]
fn a_span_background_covers_the_span_alone_and_paints_under_it() {
    let mut core = Core::new();
    let sel = Color::rgba8(0x3b, 0x5b, 0xd4, 0x55);
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.rich_text(
        &[
            Span::new("let "),
            Span::new("value").bg(sel),
            Span::new(" = 1;"),
        ],
        mono(),
    );
    ui.finish();
    let w = core.measure_text("M", &mono(), None).width;
    let bgs = solids(&mut core);
    assert_eq!(bgs.len(), 1, "{bgs:?}");
    let bg = bgs[0];
    assert_eq!(bg.4, sel);
    assert!((bg.0 - 4.0 * w).abs() < 1.0, "starts at the span: {}", bg.0);
    assert!(
        (bg.2 - 5.0 * w).abs() < 1.5,
        "as wide as the span: {}",
        bg.2
    );
    assert_eq!((bg.1, bg.3), (0.0, LH), "the whole line height");
    let (dl, _) = core.output();
    let solid_at = dl
        .quads
        .iter()
        .position(|q| q.kind == QuadKind::Solid)
        .unwrap();
    let first_glyph = dl
        .quads
        .iter()
        .position(|q| q.kind == QuadKind::GlyphMask)
        .unwrap();
    assert!(
        solid_at < first_glyph,
        "a background paints under the glyphs"
    );
}

/// A span that wraps carries its background onto the next line: two rects,
/// one per line, which is what a box around the run could never do.
#[test]
fn a_wrapped_span_background_follows_it_onto_the_next_line() {
    let mut core = Core::new();
    let w = core.measure_text("M", &mono(), None).width;
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(NodeSpec::column().width(Sizing::Fixed(8.5 * w)), |ui| {
        ui.rich_text(
            &[
                Span::new("ab "),
                Span::new("cd ef gh").bg(Color::rgb8(255, 0, 0)).underline(),
            ],
            mono(),
        )
    });
    ui.finish();
    let rects = solids(&mut core);
    let bgs: Vec<_> = rects.iter().filter(|r| r.3 == LH).collect();
    let lines: Vec<_> = rects.iter().filter(|r| r.3 < LH).collect();
    assert_eq!(bgs.len(), 2, "one background per line: {rects:?}");
    assert_eq!(lines.len(), 2, "one underline per line: {rects:?}");
    assert_eq!(bgs[0].1, 0.0);
    assert_eq!(bgs[1].1, LH);
    assert!(
        bgs[1].0 < bgs[0].0,
        "the second line starts at the left edge"
    );
}

/// Decorations are paint, not shape: the decorated text measures like the
/// plain one, and it is a second cache entry rather than a re-wrap.
#[test]
fn decorations_do_not_change_what_the_text_measures() {
    let mut core = Core::new();
    let plain = core.measure_text("hello world", &mono(), None);
    let deco = core.measure_text("hello world", &mono().underline(), None);
    assert_eq!(plain, deco);
    assert_eq!(core.text_cache_len(), 2);
}
