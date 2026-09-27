//! Rounded span backgrounds joined into one shape (backlog F101): a
//! background with a radius is one piece of the `JOIN` fragment a line,
//! told the pieces that meet it above and below — in the same colour and
//! radius, edge to edge, overlapping it — whichever text drew them.

use kui_core::{Color, Core, NodeSpec, QuadKind, Size, Span, TextStyle, TextWrap};

const LH: f32 = 20.0;
const SEL: Color = Color {
    r: 0.2,
    g: 0.4,
    b: 0.9,
    a: 0.35,
};

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(LH)
}

/// One joined piece: its quad in physical px and its params, the extents
/// made absolute again — `(own, above, below)`, each `None` when absent.
#[derive(Debug)]
struct Piece {
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    own: (f32, f32),
    above: Option<(f32, f32)>,
    below: Option<(f32, f32)>,
    radius: f32,
    color: Color,
}

fn pieces(core: &mut Core) -> Vec<Piece> {
    let (dl, _) = core.output();
    let mut out: Vec<Piece> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Fragment)
        .map(|q| {
            let p = dl.fragments[q.uv[0] as usize].params;
            let x = q.rect.x;
            let flags = p[7] as u32;
            Piece {
                x,
                y: q.rect.y,
                w: q.rect.w,
                h: q.rect.h,
                own: (x + p[0], x + p[1]),
                above: (flags & 1 != 0).then_some((x + p[2], x + p[3])),
                below: (flags & 2 != 0).then_some((x + p[4], x + p[5])),
                radius: p[6],
                color: q.color,
            }
        })
        .collect();
    out.sort_by(|a, b| a.y.total_cmp(&b.y));
    out
}

fn row(ui: &mut kui_core::Ui<'_>, parts: &[(&str, Option<(Color, f32)>)]) {
    let spans: Vec<Span<'_>> = parts
        .iter()
        .map(|(t, bg)| match bg {
            Some((c, r)) => Span::new(t).bg(*c).bg_radius(*r),
            None => Span::new(t),
        })
        .collect();
    ui.rich_text(&spans, mono());
}

/// Three rows, three texts: each row's selection is a piece that knows
/// the rows around it, and the pieces meet on one pixel line.
#[test]
fn a_rounded_background_is_one_shape_across_texts() {
    for scale in [1.0f32, 1.5, 2.0, 2.175] {
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 200.0), scale);
        ui.configure_root(NodeSpec::column().fill());
        row(&mut ui, &[("fn a() {", Some((SEL, 4.0)))]);
        row(&mut ui, &[("    let x = 1;", Some((SEL, 4.0)))]);
        row(&mut ui, &[("y", Some((SEL, 4.0))), (" rest", None)]);
        ui.finish();
        let p = pieces(&mut core);
        assert_eq!(p.len(), 3, "a piece a row, {scale}×: {p:#?}");
        let (dl, _) = core.output();
        assert!(
            !dl.quads
                .iter()
                .any(|q| q.kind == QuadKind::Solid && q.color == SEL),
            "no square background left, {scale}×"
        );
        assert_eq!(p[0].above, None);
        assert_eq!(p[0].below, Some(p[1].own), "{scale}×");
        assert_eq!(p[1].above, Some(p[0].own));
        assert_eq!(p[1].below, Some(p[2].own));
        assert_eq!(p[2].above, Some(p[1].own));
        assert_eq!(p[2].below, None);
        for w in p.windows(2) {
            assert_eq!(w[0].y + w[0].h, w[1].y, "they meet, {scale}×");
            assert_eq!(w[1].y.fract(), 0.0, "on a pixel line, {scale}×");
        }
        for q in &p {
            assert_eq!(q.radius, 4.0 * scale, "physical px");
            assert_eq!(q.color, SEL);
            // The quad holds the piece, and reaches past it by the radius
            // at most, where a neighbour reaches further.
            assert!(q.x <= q.own.0 && q.x + q.w >= q.own.1, "{q:?}");
            assert!(q.x >= q.own.0 - q.radius - 1.0, "{q:?}");
            assert!(q.x + q.w <= q.own.1 + q.radius + 1.0, "{q:?}");
        }
        // `fn a() {` is shorter than the line below: its quad reaches past
        // its end for the fillet; the last row's `y` is shorter still.
        assert!(p[0].x + p[0].w > p[0].own.1, "a fillet's room");
    }
}

/// A radius of zero is the square background it always was; another
/// colour, another radius or a gap between is another shape.
#[test]
fn only_what_meets_in_one_colour_and_radius_joins() {
    let other = Color::rgba(0.9, 0.3, 0.2, 0.35);
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    row(&mut ui, &[("square", Some((SEL, 0.0)))]);
    row(&mut ui, &[("one", Some((SEL, 4.0)))]);
    row(&mut ui, &[("other colour", Some((other, 4.0)))]);
    row(&mut ui, &[("other radius", Some((SEL, 6.0)))]);
    row(&mut ui, &[("nothing", None)]);
    row(&mut ui, &[("after a gap", Some((SEL, 6.0)))]);
    ui.finish();
    let (dl, _) = core.output();
    assert_eq!(
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::Solid && q.color == SEL)
            .count(),
        1,
        "the square one"
    );
    let p = pieces(&mut core);
    assert_eq!(p.len(), 4);
    for q in &p {
        assert_eq!((q.above, q.below), (None, None), "alone: {q:?}");
    }
}

/// A wrapped text's background is a piece a line, joined line to line
/// inside the one text.
#[test]
fn a_wrapped_background_joins_its_own_lines() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(
        NodeSpec::column().width(kui_core::Sizing::Fixed(120.0)),
        |ui| {
            ui.rich_text(
                &[Span::new("one two three four five six seven eight nine")
                    .bg(SEL)
                    .bg_radius(3.0)],
                mono().wrap(TextWrap::Word),
            );
        },
    );
    ui.finish();
    let p = pieces(&mut core);
    assert!(p.len() >= 3, "a piece a line: {p:#?}");
    assert_eq!(p[0].above, None);
    assert_eq!(p[0].below, Some(p[1].own));
    assert_eq!(p.last().unwrap().below, None);
    for w in p.windows(2) {
        assert_eq!(w[0].y + w[0].h, w[1].y);
        assert_eq!(w[0].below, Some(w[1].own));
    }
}

/// Two texts meeting end to end on a line — a line's text and the cell an
/// editor draws for its newline — are one extent: each piece is told the
/// whole, so no corner is rounded where they meet, and the line below is
/// told the whole too.
#[test]
fn pieces_meeting_end_to_end_on_a_line_are_one_extent() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.5);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(NodeSpec::row(), |ui| {
        row(ui, &[("let x = 1;", Some((SEL, 4.0)))]);
        row(ui, &[(" ", Some((SEL, 4.0)))]);
    });
    row(&mut ui, &[("y", Some((SEL, 4.0)))]);
    ui.finish();
    let p = pieces(&mut core);
    assert_eq!(p.len(), 3, "{p:#?}");
    let (text, cell, below) = (&p[0], &p[1], &p[2]);
    assert_eq!(text.y, cell.y);
    assert_eq!(text.own, cell.own, "one extent");
    assert!(
        text.own.1 > text.x + text.w - 0.01,
        "the text's quad ends inside it"
    );
    assert_eq!(
        below.above,
        Some(text.own),
        "the line below meets the whole"
    );
}
