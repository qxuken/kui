//! Span backgrounds that adjoin draw as one surface. An editor's selection
//! is a translucent background on every span of its syntax runs, row after
//! row; drawn as one rect a span, each ramped at its edges, it showed a
//! 2 px seam of the text's background at every join — between two runs of
//! a line and between two lines — even where the join sat on a whole
//! pixel (kawoosh, 2026-09-26). Snapped from the text's rounded origin,
//! a row's rect then reached a pixel into the next row wherever the rows'
//! pitch was not whole pixels, and every other join was a bright line of
//! the selection drawn twice.
//!
//! The core's rects are evaluated through `shade`'s coverage (mirrored in
//! `tests/wgsl`) and composited the way the blend does, pixel by pixel.

use kui_core::{Align, Clip, Color, Core, NodeSpec, Quad, QuadKind, Size, Span, TextStyle};

mod wgsl;
use wgsl::{inside, quad_coverage};

/// Enough rows that a pitch of a fractional number of pixels puts some
/// joins inside a pixel.
const ROWS: usize = 5;

fn sel() -> Color {
    Color::rgba8(80, 140, 220, 90)
}

/// [`ROWS`] rows of three spans of different colours, all under `sel()`,
/// stacked with no gap from a fractional offset at `scale`: the background quads, each with the clip it
/// names — a no-wrap text clips to its own box.
fn selected_rows(scale: f32) -> Vec<(Quad, Clip)> {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 120.0), scale);
    ui.configure_root(NodeSpec::column().pad(10.3));
    let style = TextStyle::new(13.0).mono().nowrap().line_height(20.0);
    for _ in 0..ROWS {
        ui.with(NodeSpec::row().height(20.0), |ui| {
            ui.rich_text(
                &[
                    Span::new("let ").color(Color::rgb8(255, 160, 60)).bg(sel()),
                    Span::new("before").bg(sel()),
                    Span::new(" = r.clone();")
                        .color(Color::rgb8(90, 200, 250))
                        .bg(sel()),
                ],
                style,
            )
        });
    }
    ui.finish();
    let dl = core.output().0;
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == sel())
        .map(|q| (*q, dl.clip_of(q)))
        .collect()
}

/// The alpha the blend leaves at the pixel centred on (`x`, `y`) after
/// every quad is drawn over it in turn.
fn alpha_at(quads: &[(Quad, Clip)], x: f32, y: f32) -> f32 {
    let clear = quads.iter().fold(1.0, |left, (q, clip)| {
        left * (1.0 - q.color.a * drawn(q, x, y) * inside(clip, x, y))
    });
    1.0 - clear
}

/// `quad_coverage` where the rasterizer runs the fragment at all: a quad
/// draws a pixel only when the pixel's centre is inside it.
fn drawn(q: &Quad, x: f32, y: f32) -> f32 {
    let r = q.rect;
    let hit = x >= r.x && x < r.x + r.w && y >= r.y && y < r.y + r.h;
    if hit { quad_coverage(q, x, y) } else { 0.0 }
}

#[test]
fn a_selection_over_runs_and_rows_is_one_even_surface() {
    for scale in [1.0, 1.1, 1.25, 1.3, 1.5, 1.75, 2.0, 2.175] {
        let quads = selected_rows(scale);
        assert_eq!(
            quads.len(),
            ROWS,
            "one rect a row, the three spans joined, at {scale}×: {quads:?}"
        );
        // Every pixel of the rows' common extent, both rows' first and
        // last pixel lines and the join between them included.
        let x0 = quads.iter().map(|(q, _)| q.rect.x).fold(f32::MIN, f32::max);
        let x1 = quads
            .iter()
            .map(|(q, _)| q.rect.x + q.rect.w)
            .fold(f32::MAX, f32::min);
        let y0 = quads.iter().map(|(q, _)| q.rect.y).fold(f32::MAX, f32::min);
        let y1 = quads
            .iter()
            .map(|(q, _)| q.rect.y + q.rect.h)
            .fold(f32::MIN, f32::max);
        assert_eq!(x0.fract(), 0.0, "on a whole pixel at {scale}×: {x0}");
        let mut px = x0 as i32;
        while (px as f32) < x1 {
            let mut py = y0 as i32;
            while (py as f32) < y1 {
                let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
                let a = alpha_at(&quads, cx, cy);
                assert!(
                    (a - sel().a).abs() < 1e-4,
                    "the selection's own alpha {} at ({cx}, {cy}), {scale}×: {a}",
                    sel().a
                );
                py += 1;
            }
            px += 1;
        }
    }
}

/// An editor's rows as kawoosh draws them: a fixed-height row centring a
/// no-wrap text whose spans carry the selection, then the selected
/// newline as a `pixelSnap` box a cell wide, an empty line only that box.
/// `lh` and `top` put the rows' joins inside pixels.
fn editor_rows(lh: f32, top: f32, scale: f32) -> Vec<(Quad, Clip)> {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 30.0 * lh + 2.0 * top), scale);
    ui.configure_root(NodeSpec::column().pad_xy(10.3, top));
    let style = TextStyle::new(16.0).mono().nowrap().line_height(lh);
    for i in 0..30 {
        ui.with(
            NodeSpec::row().height(lh).cross_align(Align::Center),
            |ui| {
                let text = ["    }", "}", ""][i % 3];
                if !text.is_empty() {
                    ui.rich_text(&[Span::new(text).bg(sel())], style);
                }
                ui.leaf(NodeSpec::column().size(9.63, lh).bg(sel()).pixel_snap());
            },
        );
    }
    ui.finish();
    let dl = core.output().0;
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.color == sel())
        .map(|q| (*q, dl.clip_of(q)))
        .collect()
}

#[test]
fn a_selection_past_a_line_end_meets_the_lines_beside_it() {
    for lh in [20.0, 21.75, 18.2] {
        for top in [0.0, 3.3, 7.1] {
            for scale in [1.0, 1.25, 1.5, 1.75, 2.0, 2.175] {
                let quads = editor_rows(lh, top, scale);
                let y0 = quads.iter().map(|(q, _)| q.rect.y).fold(f32::MAX, f32::min);
                let y1 = quads
                    .iter()
                    .map(|(q, _)| q.rect.y + q.rect.h)
                    .fold(f32::MIN, f32::max);
                let x1 = quads
                    .iter()
                    .map(|(q, _)| q.rect.x + q.rect.w)
                    .fold(f32::MIN, f32::max);
                let at = format!("line height {lh}, top {top}, {scale}×");
                // Nowhere drawn twice.
                for px in 0..x1.ceil() as i32 {
                    for py in y0.floor() as i32..y1.ceil() as i32 {
                        let (cx, cy) = (px as f32 + 0.5, py as f32 + 0.5);
                        let a = alpha_at(&quads, cx, cy);
                        assert!(a < sel().a + 1e-4, "drawn twice at ({cx}, {cy}), {at}: {a}");
                    }
                }
                // And the first cell's column, which every row covers —
                // with its text or, on the empty line, its newline's box —
                // is one surface from the first row to the last.
                let cx = (10.3 * scale).ceil() + 2.5;
                for py in y0 as i32..y1 as i32 {
                    let cy = py as f32 + 0.5;
                    let a = alpha_at(&quads, cx, cy);
                    assert!(
                        (a - sel().a).abs() < 1e-4,
                        "the selection's own alpha at ({cx}, {cy}), {at}: {a}"
                    );
                }
            }
        }
    }
}

#[test]
fn a_square_rect_on_whole_pixels_is_solid_to_its_edge_and_a_rounded_one_still_ramps() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().pad(10.0).gap(10.0));
    let fill = |r: f32| {
        NodeSpec::column()
            .size(40.0, 40.0)
            .radius(r)
            .bg(Color::rgb8(20, 20, 30))
    };
    ui.leaf(fill(0.0));
    ui.leaf(fill(8.0));
    ui.finish();
    let dl = core.output().0;
    let (square, round) = (dl.quads[0], dl.quads[1]);
    // The outermost pixels, the corner too, are fully covered, and the
    // pixel past the edge not at all.
    for (x, y) in [(10.5, 30.0), (49.5, 30.0), (30.0, 10.5), (10.5, 10.5)] {
        assert_eq!(quad_coverage(&square, x, y), 1.0, "({x}, {y})");
    }
    assert_eq!(quad_coverage(&square, 50.5, 30.0), 0.0);
    // A rounded rect keeps its antialiased edge and corner.
    let edge = quad_coverage(&round, round.rect.x + 0.5, round.rect.y + 20.0);
    assert!(edge > 0.9 && edge < 1.0, "ramped: {edge}");
    assert!(quad_coverage(&round, round.rect.x + 0.5, round.rect.y + 0.5) < 0.01);
}

/// How much of the pixel centred on (`x`, `y`) the opaque quads cover
/// between them.
fn covered(quads: &[Quad], x: f32, y: f32) -> f32 {
    let clear = quads
        .iter()
        .fold(1.0, |left, q| left * (1.0 - drawn(q, x, y)));
    1.0 - clear
}

#[test]
fn a_one_pixel_gap_between_boxes_is_there_at_any_scale() {
    // Snapping each box's edges to whole pixels (tried 2026-09-27, for
    // the seams above) closed it below 1×: a 0.75 px gap between two
    // edges rounded to the same pixel. A box is drawn where layout put
    // it, so the gap is always part of a pixel at least.
    for scale in [
        0.5, 0.75, 0.9, 1.0, 1.1, 1.25, 1.33, 1.5, 1.75, 2.0, 2.175, 3.0,
    ] {
        for off in [0.0, 0.3, 0.5, 0.7, 10.3] {
            for w in [10.0, 10.4, 13.7] {
                let mut core = Core::new();
                let mut ui = core.frame(Size::new(600.0, 40.0), scale);
                ui.configure_root(NodeSpec::row().pad_xy(off, 5.0).gap(1.0));
                for _ in 0..20 {
                    ui.leaf(NodeSpec::column().size(w, 10.0).bg(Color::WHITE));
                }
                ui.finish();
                let dl = core.output().0;
                let quads: Vec<Quad> = dl
                    .quads
                    .iter()
                    .filter(|q| q.kind == QuadKind::Solid)
                    .copied()
                    .collect();
                assert_eq!(quads.len(), 20);
                let cy = quads[0].rect.y + 5.0 * scale;
                let at = format!("{scale}×, offset {off}, width {w}");
                for pair in quads.windows(2) {
                    let (a, b) = (pair[0].rect, pair[1].rect);
                    let gap = b.x - (a.x + a.w);
                    assert!(
                        (gap - scale).abs() < 1e-3,
                        "the gap is layout's, {at}: {gap}"
                    );
                    // Some pixel across the gap shows what is under it.
                    let least = ((a.x + a.w).floor() as i32..=b.x.floor() as i32)
                        .map(|px| covered(&quads, px as f32 + 0.5, cy))
                        .fold(f32::MAX, f32::min);
                    assert!(least < 0.99, "the gap is drawn, {at}: {least}");
                }
            }
        }
    }
}

/// A column of opaque boxes 21.75 tall from a fractional offset at
/// `scale`, each `pixelSnap` or not: their quads, top to bottom.
fn band_rows(scale: f32, snap: bool) -> Vec<Quad> {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(100.0, 400.0), scale);
    ui.configure_root(NodeSpec::column().pad_xy(3.3, 7.7));
    for i in 0..12 {
        let mut spec = NodeSpec::column().size(50.5, 21.75).bg(if i % 2 == 0 {
            Color::rgb8(40, 40, 60)
        } else {
            Color::rgb8(60, 40, 40)
        });
        if snap {
            spec = spec.pixel_snap();
        }
        ui.leaf(spec);
    }
    ui.finish();
    let dl = core.output().0;
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .copied()
        .collect()
}

#[test]
fn snapped_boxes_stacked_at_a_fractional_pitch_are_one_surface() {
    for scale in [0.5, 0.75, 1.0, 1.25, 1.5, 1.75, 2.0, 2.175, 3.0] {
        let quads = band_rows(scale, true);
        assert_eq!(quads.len(), 12);
        let first = quads[0].rect;
        let last = quads[11].rect;
        let cx = first.x + first.w * 0.5;
        // Every pixel from the first box's top to the last's bottom is
        // covered exactly once: no join drawn by halves, none twice.
        let mut py = first.y as i32;
        while (py as f32) < last.y + last.h {
            let cy = py as f32 + 0.5;
            let n = quads.iter().filter(|q| drawn(q, cx, cy) > 0.0).count();
            assert_eq!(n, 1, "one box draws ({cx}, {cy}), {scale}×");
            let c = covered(&quads, cx, cy);
            assert_eq!(c, 1.0, "wholly, at ({cx}, {cy}), {scale}×: {c}");
            py += 1;
        }
    }
}

#[test]
fn a_box_that_does_not_ask_is_drawn_where_layout_put_it() {
    for scale in [0.75, 1.0, 1.5, 2.175] {
        let quads = band_rows(scale, false);
        let top = 7.7 * scale;
        for (i, q) in quads.iter().enumerate() {
            let y = top + i as f32 * 21.75 * scale;
            assert!(
                (q.rect.x - 3.3 * scale).abs() < 1e-3,
                "{scale}×: {:?}",
                q.rect
            );
            assert!(
                (q.rect.y - y).abs() < 1e-3,
                "{scale}×, row {i}: {:?}",
                q.rect
            );
            assert!(
                (q.rect.w - 50.5 * scale).abs() < 1e-3,
                "{scale}×: {:?}",
                q.rect
            );
            assert!(
                (q.rect.h - 21.75 * scale).abs() < 1e-3,
                "{scale}×: {:?}",
                q.rect
            );
        }
    }
}

/// A column of `fragment` nodes 21.75 tall from a fractional offset at
/// `scale`, `pixelSnap` or not: their quads, top to bottom.
fn fragment_rows(scale: f32, snap: bool) -> Vec<Quad> {
    let mut core = Core::new();
    let id = core
        .add_fragment(
            "fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {\n    return vec4<f32>(1.0);\n}\n",
        )
        .expect("a trivial fragment compiles");
    let mut ui = core.frame(Size::new(100.0, 400.0), scale);
    ui.configure_root(NodeSpec::column().pad_xy(3.3, 7.7));
    for _ in 0..12 {
        let mut spec = NodeSpec::column().size(50.5, 21.75);
        if snap {
            spec = spec.pixel_snap();
        }
        ui.fragment(id, &[], spec);
    }
    ui.finish();
    let dl = core.output().0;
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Fragment)
        .copied()
        .collect()
}

#[test]
fn snapped_fragments_stack_on_whole_pixels_and_others_where_layout_put_them() {
    for scale in [0.75, 1.0, 1.25, 1.5, 2.0, 2.175] {
        let snapped = fragment_rows(scale, true);
        assert_eq!(snapped.len(), 12);
        for w in snapped.windows(2) {
            let (a, b) = (w[0].rect, w[1].rect);
            assert_eq!(a.y + a.h, b.y, "they meet, {scale}×");
            assert_eq!(b.y.fract(), 0.0, "on a pixel line, {scale}×");
        }
        let exact = fragment_rows(scale, false);
        for (i, q) in exact.iter().enumerate() {
            let y = 7.7 * scale + i as f32 * 21.75 * scale;
            assert!(
                (q.rect.y - y).abs() < 1e-3,
                "{scale}×, row {i}: {:?}",
                q.rect
            );
        }
    }
}
