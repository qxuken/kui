//! Rounded span backgrounds joined into one shape (backlog F101), drawn:
//! an editor's selection over lines of different lengths, each line its
//! own no-wrap text, composited through the `JOIN` fragment (mirrored in
//! `tests/wgsl`) the way the blend does, pixel by pixel, at scales where a
//! line is not whole physical pixels.

use kui_core::{Clip, Color, Core, NodeSpec, Quad, QuadKind, Size, Sizing, Span, TextStyle};

mod wgsl;
use wgsl::{inside, join_alpha};

const R: f32 = 4.0;
const LINES: [&str; 5] = ["fn a() {", "    let x = 1;", " ", "    y", "}"];

fn sel() -> Color {
    Color::rgba8(80, 140, 220, 90)
}

/// The rows' pieces: each quad, its params and the clip it names.
fn selected(scale: f32) -> Vec<(Quad, [f32; 16], Clip)> {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), scale);
    ui.configure_root(NodeSpec::column().pad(10.3));
    let style = TextStyle::new(13.0).mono().nowrap().line_height(20.0);
    for line in LINES {
        ui.with(NodeSpec::row().height(Sizing::Fixed(20.0)), |ui| {
            ui.rich_text(
                &[Span::new(line)
                    .color(Color::rgb8(220, 220, 220))
                    .bg(sel())
                    .bg_radius(R)],
                style,
            )
        });
    }
    ui.finish();
    let dl = core.output().0;
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Fragment)
        .map(|q| (*q, dl.fragments[q.uv[0] as usize].params, dl.clip_of(q)))
        .collect()
}

/// The alpha the blend leaves at the pixel centred on (`x`, `y`).
fn alpha_at(pieces: &[(Quad, [f32; 16], Clip)], x: f32, y: f32) -> f32 {
    let clear = pieces.iter().fold(1.0, |left, (q, p, clip)| {
        let r = q.rect;
        if !(x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h) {
            return left;
        }
        let a = join_alpha((x - r.x, y - r.y), (r.w, r.h), p);
        left * (1.0 - q.color.a * a * inside(clip, x, y))
    });
    1.0 - clear
}

#[test]
fn a_rounded_selection_over_lines_is_one_shape() {
    for scale in [1.0f32, 1.25, 1.5, 1.75, 2.0, 2.175] {
        let pieces = selected(scale);
        assert_eq!(pieces.len(), LINES.len(), "a piece a line, {scale}×");
        let sa = sel().a;
        let own = |i: usize| {
            let (q, p, _) = &pieces[i];
            (q.rect.x + p[0], q.rect.x + p[1])
        };
        let top = pieces[0].0.rect.y;
        let bottom = pieces.last().map(|(q, _, _)| q.rect.y + q.rect.h).unwrap();
        let x0 = own(0).0;
        let right = (0..pieces.len()).map(|i| own(i).1).fold(0.0, f32::max);
        // Nothing drawn twice, anywhere — the joins and the fillets past a
        // short line's end included.
        for py in top as i32..bottom as i32 {
            for px in x0 as i32 - 2..right as i32 + (R * scale) as i32 + 2 {
                let a = alpha_at(&pieces, px as f32 + 0.5, py as f32 + 0.5);
                assert!(a < sa + 1e-4, "drawn twice at ({px}, {py}), {scale}×: {a}");
            }
        }
        // The column every line covers is one surface, top to bottom but
        // for the outer corners.
        let r = R * scale;
        let cx = x0 + r.ceil() + 1.5;
        for py in (top + r).ceil() as i32..(bottom - r).floor() as i32 {
            let a = alpha_at(&pieces, cx, py as f32 + 0.5);
            assert!(
                (a - sa).abs() < 1e-4,
                "one surface at ({cx}, {py}), {scale}×: {a}"
            );
        }
        // The top-left corner, with nothing above, is round.
        assert!(
            alpha_at(&pieces, x0 + 0.5, top + 0.5) < sa * 0.5,
            "{scale}×"
        );
        // `fn a() {` over the longer `    let x = 1;`: past its end, at the
        // join, a concave fillet fills the pixel beside it — outside the
        // short line's own box, which its text clips to.
        let join = pieces[0].0.rect.y + pieces[0].0.rect.h;
        let fillet = alpha_at(&pieces, own(0).1 + 0.5, join - 0.5);
        assert!(fillet > sa * 0.5, "the fillet, {scale}×: {fillet}");
        // The longer line's bottom-right, over the one-cell line, is
        // convex: its corner pixel cut.
        let join1 = pieces[1].0.rect.y + pieces[1].0.rect.h;
        let corner = alpha_at(&pieces, own(1).1 - 0.5, join1 - 0.5);
        assert!(corner < sa * 0.9, "the convex corner, {scale}×: {corner}");
    }
}
