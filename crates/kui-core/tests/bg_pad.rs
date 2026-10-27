//! A span background's padding (backlog F226, from Carnet's selection):
//! `Span::bg_pad` widens the two ends of the rect a background is drawn
//! in, and nothing else — not the glyphs, not the layout, not the rect's
//! middle where spans of one colour merge — and stops where it meets
//! another background rather than painting over it.

use kui_core::{Color, Core, NodeSpec, QuadKind, Rect, Size, Span, TextStyle};

const A: Color = Color {
    r: 0.2,
    g: 0.4,
    b: 0.9,
    a: 1.0,
};
const B: Color = Color {
    r: 0.9,
    g: 0.7,
    b: 0.1,
    a: 1.0,
};

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(20.0)
}

/// Draws `spans` in a `w`-wide column at scale 1 and answers the solid
/// rects of colour `c`, left to right, top to bottom.
fn frame(core: &mut Core, w: f32, spans: &[Span<'_>]) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(NodeSpec::column().width(w).pad_xy(20.0, 10.0), |ui| {
        ui.rich_text(spans, mono());
    });
    ui.finish();
}

fn rects(core: &mut Core, c: Color) -> Vec<Rect> {
    let (dl, _) = core.output();
    let mut out: Vec<Rect> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == c)
        .map(|q| q.rect)
        .collect();
    out.sort_by(|a, b| a.y.total_cmp(&b.y).then(a.x.total_cmp(&b.x)));
    out
}

/// Every glyph quad's place: what padding must leave alone.
fn glyphs(core: &mut Core) -> Vec<(f32, f32)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| {
            matches!(
                q.kind,
                QuadKind::GlyphMask | QuadKind::GlyphColor | QuadKind::GlyphSubpixel
            )
        })
        .map(|q| (q.rect.x, q.rect.y))
        .collect()
}

#[test]
fn the_padding_widens_both_ends_and_moves_no_glyph() {
    let mut plain = Core::new();
    frame(
        &mut plain,
        300.0,
        &[Span::new("a "), Span::new("word").bg(A), Span::new(" z")],
    );
    let before = rects(&mut plain, A);
    let glyphs_before = glyphs(&mut plain);
    let mut padded = Core::new();
    frame(
        &mut padded,
        300.0,
        &[
            Span::new("a "),
            Span::new("word").bg(A).bg_pad(4.0),
            Span::new(" z"),
        ],
    );
    let after = rects(&mut padded, A);
    assert_eq!((before.len(), after.len()), (1, 1));
    let (b, a) = (before[0], after[0]);
    assert!((a.x - (b.x - 4.0)).abs() < 0.01, "{b:?} -> {a:?}");
    assert!((a.w - (b.w + 8.0)).abs() < 0.01, "{b:?} -> {a:?}");
    assert_eq!((a.y, a.h), (b.y, b.h), "only across");
    assert_eq!(
        glyphs(&mut padded),
        glyphs_before,
        "the glyphs did not move"
    );
}

/// Spans of one background merged into one rect are padded at the
/// rect's two ends alone: the join between them stays seamless.
#[test]
fn merged_spans_are_padded_at_the_outer_ends_alone() {
    let mut plain = Core::new();
    frame(
        &mut plain,
        300.0,
        &[Span::new("ab").bg(A), Span::new("cd").bg(A)],
    );
    let b = rects(&mut plain, A);
    let mut padded = Core::new();
    frame(
        &mut padded,
        300.0,
        &[
            Span::new("ab").bg(A).bg_pad(3.0),
            Span::new("cd").bg(A).bg_pad(3.0),
        ],
    );
    let a = rects(&mut padded, A);
    assert_eq!((b.len(), a.len()), (1, 1), "one rect either way");
    assert!((a[0].w - (b[0].w + 6.0)).abs() < 0.01, "{b:?} -> {a:?}");
}

/// A selection beside a highlight of another colour: the edge they share
/// stays where it is — neither paints over the other — and the selection
/// is padded at its other end.
#[test]
fn the_padding_stops_where_another_background_begins() {
    let mut plain = Core::new();
    frame(
        &mut plain,
        300.0,
        &[Span::new("sel").bg(A), Span::new("hl").bg(B)],
    );
    let (a0, b0) = (rects(&mut plain, A)[0], rects(&mut plain, B)[0]);
    let mut padded = Core::new();
    frame(
        &mut padded,
        300.0,
        &[
            Span::new("sel").bg(A).bg_pad(4.0),
            Span::new("hl").bg(B).bg_pad(4.0),
        ],
    );
    let (a, b) = (rects(&mut padded, A)[0], rects(&mut padded, B)[0]);
    assert!((a.x - (a0.x - 4.0)).abs() < 0.01, "outer end padded: {a:?}");
    assert!(
        (a.x + a.w - (a0.x + a0.w)).abs() < 0.01,
        "shared edge kept: {a:?}"
    );
    assert!((b.x - b0.x).abs() < 0.01, "shared edge kept: {b:?}");
    assert!((b.x + b.w - (b0.x + b0.w + 4.0)).abs() < 0.01, "{b:?}");
}

/// Two backgrounds a space apart, each padded by more than half of it:
/// each takes its share of the gap, and they do not overlap.
#[test]
fn two_backgrounds_closer_than_their_padding_share_the_gap() {
    let spans = |pad: f32| {
        [
            Span::new("ab").bg(A).bg_pad(pad),
            Span::new(" "),
            Span::new("cd").bg(B).bg_pad(pad),
        ]
    };
    let mut plain = Core::new();
    frame(&mut plain, 300.0, &spans(0.0));
    let (a0, b0) = (rects(&mut plain, A)[0], rects(&mut plain, B)[0]);
    let gap = b0.x - (a0.x + a0.w);
    assert!(gap > 1.0, "a space between them: {gap}");
    let mut padded = Core::new();
    frame(&mut padded, 300.0, &spans(gap));
    let (a, b) = (rects(&mut padded, A)[0], rects(&mut padded, B)[0]);
    // Halfway, to the pixel a rect is snapped to, and never over the other.
    let mid = a0.x + a0.w + gap / 2.0;
    assert!(a.x + a.w <= b.x + 0.01, "{a:?} overlaps {b:?}");
    assert!((a.x + a.w - mid).abs() <= 0.5, "{a:?} meets {b:?} at {mid}");
    assert!((b.x - mid).abs() <= 0.5, "{a:?} meets {b:?} at {mid}");
}

/// A span that wraps is a rect a line, and each is padded at its ends.
#[test]
fn a_wrapped_background_is_padded_on_every_line() {
    let long = "a selection that runs on past the end of one line";
    let mut plain = Core::new();
    frame(&mut plain, 160.0, &[Span::new(long).bg(A)]);
    let b = rects(&mut plain, A);
    let mut padded = Core::new();
    frame(&mut padded, 160.0, &[Span::new(long).bg(A).bg_pad(3.0)]);
    let a = rects(&mut padded, A);
    assert!(b.len() >= 2, "it wraps: {b:?}");
    assert_eq!(a.len(), b.len());
    for (b, a) in b.iter().zip(&a) {
        assert!((a.x - (b.x - 3.0)).abs() < 0.01, "{b:?} -> {a:?}");
        assert!((a.w - (b.w + 6.0)).abs() < 0.01, "{b:?} -> {a:?}");
    }
}

/// A rounded background is padded the same: its joined piece's own
/// extent is wider by the padding at each end.
#[test]
fn a_rounded_background_is_padded_too() {
    let own = |core: &mut Core| {
        let (dl, _) = core.output();
        let q = dl
            .quads
            .iter()
            .find(|q| q.kind == QuadKind::Fragment)
            .expect("a rounded background");
        let p = dl.fragments[q.uv[0] as usize].params;
        (q.rect.x + p[0], q.rect.x + p[1])
    };
    let mut plain = Core::new();
    frame(&mut plain, 300.0, &[Span::new("word").bg(A).bg_radius(4.0)]);
    let (b0, b1) = own(&mut plain);
    let mut padded = Core::new();
    frame(
        &mut padded,
        300.0,
        &[Span::new("word").bg(A).bg_radius(4.0).bg_pad(4.0)],
    );
    let (a0, a1) = own(&mut padded);
    assert!(
        (a0 - (b0 - 4.0)).abs() < 0.01 && (a1 - (b1 + 4.0)).abs() < 0.01,
        "{b0},{b1} -> {a0},{a1}"
    );
}
