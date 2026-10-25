//! What a chart's `path` costs when its `d` changes (backlog C96): an
//! area under a series, rasterized fresh, as a live chart does every
//! frame its data moves. Two shapes from Kit's line chart: 300 points
//! across a 360 × 120 plot, and a 1,400-vertex zigzag over the same
//! width — two points a physical pixel at scale 2, every scanline
//! crossed by every edge, the case that took 1.7 s a frame.
//!
//! The `raster_*` rows call `path::rasterize` alone, the fill with its
//! half-pixel bleed; `bleed_*` the bleed's stroke on its own; `frame_*`
//! a whole frame of one path node whose `d` moves every frame, which
//! after two frames draws from a texture of its own (ADR 0040).
//!
//! Run: cargo bench -p kui-core --bench paths

use kui_core::path::{FillRule, MaskPaint, rasterize};
use kui_core::{Color, Core, NodeSpec, Path, PathOp, Size};

fn main() {
    divan::main();
}

const PLOT_W: f32 = 360.0;
const PLOT_H: f32 = 120.0;
const SCALE: f32 = 2.0;

/// The area under `n` points across the plot, the series shifted by
/// `phase`: a smooth wave with a little noise, as a metric reads.
fn area(n: usize, phase: f32) -> Path {
    let mut p = Path::new().move_to(1.0, PLOT_H + 1.0);
    let mut seed = 0x9e37_79b9u32 ^ phase.to_bits();
    for i in 0..n {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        let x = i as f32 / (n - 1) as f32 * PLOT_W;
        let wave = ((i as f32 * 0.07 + phase).sin() * 0.4 + 0.5) * PLOT_H;
        let noise = (seed % 1000) as f32 / 1000.0 * 8.0 - 4.0;
        p = p.line_to(x + 1.0, (wave + noise).clamp(0.0, PLOT_H) + 1.0);
    }
    p.line_to(PLOT_W + 1.0, PLOT_H + 1.0).close()
}

/// The area under `n` vertices that swing from near the top to near the
/// bottom and back at every step: every edge crosses most scanlines.
fn zigzag(n: usize, phase: f32) -> Path {
    let mut p = Path::new().move_to(1.0, PLOT_H + 1.0);
    for i in 0..n {
        let x = i as f32 / (n - 1) as f32 * PLOT_W;
        let y = if i % 2 == 0 {
            4.0 + phase
        } else {
            PLOT_H - 4.0 - phase
        };
        p = p.line_to(x + 1.0, y + 1.0);
    }
    p.line_to(PLOT_W + 1.0, PLOT_H + 1.0).close()
}

/// Eight frames of one shape, as a chart keeps its series: the geometry
/// is built ahead and the raster is what is measured.
fn frames(shape: fn(usize, f32) -> Path, n: usize) -> Vec<Vec<PathOp>> {
    (0..8)
        .map(|f| shape(n, f as f32 * 0.37).ops().to_vec())
        .collect()
}

/// The mask a path node at scale 2 asks for: the plot and two logical
/// px round it.
fn mask_size() -> (u32, u32) {
    (
        ((PLOT_W + 4.0) * SCALE).ceil() as u32,
        ((PLOT_H + 4.0) * SCALE).ceil() as u32,
    )
}

fn bench_raster(bencher: divan::Bencher, frames: &[Vec<PathOp>], paint: MaskPaint) {
    let (w, h) = mask_size();
    let mut f = 0usize;
    bencher.bench_local(|| {
        f += 1;
        rasterize(&frames[f % frames.len()], SCALE, (0, 0), w, h, paint)
    });
}

const FILL: MaskPaint = MaskPaint::Fill(FillRule::NonZero);

#[divan::bench]
fn raster_area_300_scale2(bencher: divan::Bencher) {
    bench_raster(bencher, &frames(area, 300), FILL);
}

#[divan::bench]
fn raster_area_1400_scale2(bencher: divan::Bencher) {
    bench_raster(bencher, &frames(area, 1400), FILL);
}

#[divan::bench(sample_count = 10)]
fn raster_zigzag_1400_scale2(bencher: divan::Bencher) {
    bench_raster(bencher, &frames(zigzag, 1400), FILL);
}

/// The 1 px outline the fill's bleed is drawn from, alone.
#[divan::bench(sample_count = 10)]
fn bleed_zigzag_1400_scale2(bencher: divan::Bencher) {
    bench_raster(bencher, &frames(zigzag, 1400), MaskPaint::Stroke(1.0));
}

#[divan::bench]
fn bleed_area_300_scale2(bencher: divan::Bencher) {
    bench_raster(bencher, &frames(area, 300), MaskPaint::Stroke(1.0));
}

/// A frame of one path node at scale 2 whose `d` is a different frame of
/// the series each time.
fn bench_frame(bencher: divan::Bencher, frames: Vec<Path>) {
    let mut core = Core::new();
    let mut f = 0usize;
    bencher.bench_local(|| {
        f += 1;
        let mut ui = core.frame(Size::new(400.0, 200.0), SCALE);
        ui.configure_root(NodeSpec::column().fill());
        ui.path_keyed(
            "series",
            &frames[f % frames.len()],
            NodeSpec::column().bg(Color::rgb8(60, 120, 200)),
        );
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.len()
    });
}

#[divan::bench]
fn frame_area_300_changing(bencher: divan::Bencher) {
    bench_frame(
        bencher,
        (0..8).map(|f| area(300, f as f32 * 0.37)).collect(),
    );
}

#[divan::bench(sample_count = 10)]
fn frame_zigzag_1400_changing(bencher: divan::Bencher) {
    bench_frame(
        bencher,
        (0..8).map(|f| zigzag(1400, f as f32 * 0.37)).collect(),
    );
}
