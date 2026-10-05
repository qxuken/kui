//! The half of the path contract shader validation cannot see: the core
//! rasterizes an outline into the atlas and emits a `GlyphMask` quad whose
//! `uv` names the slot, and the shader's `kind == 1u` branch draws atlas
//! alpha times the quad's colour (`src/shader.wgsl`). Get the mask's
//! placement wrong — a quad a pixel off its slot, a bin baked in twice, a
//! ramp cut by the slot's edge — and the shape still draws, in the wrong
//! place or with a hard edge, and no compile sees it.
//!
//! This mirrors that branch on the CPU over the atlas and the quads a
//! core actually emitted, and proves the one promise ADR 0040 makes about
//! two fills: that where they share an edge, the background never shows.

use kui_core::{Color, Core, NodeSpec, Path, Quad, QuadKind, Size, Vec2};

/// What the glyph-mask branch of `shade` draws at a framebuffer pixel:
/// the slot's texel under it, times the colour's alpha. `None` outside
/// the quad.
fn coverage(core: &Core, q: &Quad, x: f32, y: f32) -> Option<f32> {
    if x < q.rect.x || y < q.rect.y || x >= q.rect.x + q.rect.w || y >= q.rect.y + q.rect.h {
        return None;
    }
    let tx = q.uv[0] + (x - q.rect.x) as u32;
    let ty = q.uv[1] + (y - q.rect.y) as u32;
    let at = ((ty * core.atlas.size + tx) * 4 + 3) as usize;
    Some(f32::from(core.atlas.pixels[at]) / 255.0 * q.color.a)
}

/// Every mask quad of one frame of `build`, with the core that holds the
/// atlas they index.
fn masks(build: impl FnOnce(&mut kui_core::Ui<'_>), scale: f32) -> (Core, Vec<Quad>) {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 100.0), scale);
    build(&mut ui);
    ui.finish();
    let quads: Vec<Quad> = core
        .output()
        .0
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::GlyphMask)
        .copied()
        .collect();
    (core, quads)
}

#[test]
fn a_square_is_opaque_inside_half_on_its_edge_and_nothing_past_it() {
    let (core, q) = masks(
        |ui| {
            ui.path(
                &Path::parse("M20 20 H60 V60 H20 Z").unwrap(),
                NodeSpec::column().bg(Color::rgba8(0, 0, 0, 255)),
            );
        },
        1.0,
    );
    assert_eq!(q.len(), 1);
    let q = &q[0];
    let cov = |x: f32, y: f32| coverage(&core, q, x, y).unwrap_or(0.0);
    assert!(cov(40.5, 40.5) > 0.99, "opaque inside: {}", cov(40.5, 40.5));
    assert!(cov(20.5, 40.5) > 0.99, "solid on the first pixel in");
    // The fill bleeds half a pixel: the pixel just outside the edge is
    // the bleed's half, the one past that is empty.
    let bleed = cov(19.5, 40.5);
    assert!((0.3..0.7).contains(&bleed), "half on the bleed: {bleed}");
    assert!(cov(18.5, 40.5) < 0.01, "nothing two pixels out");
    // The quad is padded past the ramp: zero along every side.
    assert!(cov(q.rect.x + 0.5, 40.5) < 0.01);
    assert!(cov(q.rect.x + q.rect.w - 0.5, 40.5) < 0.01);
    assert!(cov(40.5, q.rect.y + 0.5) < 0.01);
    assert!(cov(40.5, q.rect.y + q.rect.h - 0.5) < 0.01);
}

/// ADR 0040, decision 5: two fills sharing an edge composite to full
/// coverage along it, so the background never shows — the seam the
/// polygon pie had.
#[test]
fn two_wedges_sharing_an_edge_leave_no_seam() {
    let (core, q) = masks(
        |ui| {
            ui.path(
                &Path::sector(100.0, 50.0, 40.0, 0.0, 0.0, 0.25),
                NodeSpec::column().bg(Color::rgba8(255, 0, 0, 255)),
            );
            ui.path(
                &Path::sector(100.0, 50.0, 40.0, 0.0, 0.25, 0.25),
                NodeSpec::column().bg(Color::rgba8(0, 0, 255, 255)),
            );
        },
        1.0,
    );
    assert_eq!(q.len(), 2);
    // Along the shared radial edge, straight down from the centre.
    for y in [60.5, 70.5, 80.5] {
        for x in [99.5, 100.5] {
            let a = coverage(&core, &q[0], x, y).unwrap_or(0.0);
            let b = coverage(&core, &q[1], x, y).unwrap_or(0.0);
            let over = a + b * (1.0 - a);
            assert!(over > 0.99, "at ({x}, {y}): {a} under {b} leaves {over}");
        }
    }
    // And the arc is an arc: full at the rim's inside, empty past it.
    let a = (0.125f32) * std::f32::consts::TAU;
    let inside = coverage(&core, &q[0], 100.0 + 37.0 * a.cos(), 50.0 + 37.0 * a.sin());
    let past = coverage(&core, &q[0], 100.0 + 43.0 * a.cos(), 50.0 + 43.0 * a.sin());
    assert!(inside.unwrap_or(0.0) > 0.95, "{inside:?}");
    assert!(past.unwrap_or(0.0) < 0.05, "{past:?}");
}

/// At scale 2 the mask has twice the texels and sits on the physical
/// grid: a logical edge at 20 is a physical edge at 40.
#[test]
fn the_mask_is_rasterized_at_the_physical_scale() {
    let (core, q) = masks(
        |ui| {
            ui.path(
                &Path::parse("M20 20 H60 V60 H20 Z").unwrap(),
                NodeSpec::column().bg(Color::rgba8(0, 0, 0, 255)),
            );
        },
        2.0,
    );
    let q = &q[0];
    let cov = |x: f32, y: f32| coverage(&core, q, x, y).unwrap_or(0.0);
    assert!(cov(80.5, 80.5) > 0.99);
    assert!(
        cov(40.5, 80.5) > 0.99,
        "solid on the first physical pixel in"
    );
    assert!(cov(38.5, 80.5) < 0.01, "nothing two physical pixels out");
    assert_eq!(q.rect.x.fract(), 0.0, "on whole pixels");
}

/// A quad's fractional position is baked into the mask at the nearest
/// quarter pixel, not resampled: a square at x = 20.5 has its edge's
/// half-covered column where the edge is, and the quad on a whole pixel.
#[test]
fn a_fractional_position_is_baked_into_the_mask() {
    let (core, q) = masks(
        |ui| {
            ui.path(
                &Path::parse("M20.5 20 H60.5 V60 H20.5 Z").unwrap(),
                NodeSpec::column().bg(Color::rgba8(0, 0, 0, 255)),
            );
        },
        1.0,
    );
    let q = &q[0];
    assert_eq!(q.rect.x.fract(), 0.0);
    let cov = |x: f32, y: f32| coverage(&core, q, x, y).unwrap_or(0.0);
    // The edge is at 20.5 with half a pixel of bleed to 20.0: pixel 20
    // is whole, pixel 19 is empty — where the integer edge of the first
    // test left pixel 19 half covered. The half pixel moved the edge.
    assert!(cov(21.5, 40.5) > 0.99, "{}", cov(21.5, 40.5));
    assert!(cov(20.5, 40.5) > 0.95, "{}", cov(20.5, 40.5));
    assert!(cov(19.5, 40.5) < 0.05, "{}", cov(19.5, 40.5));
    assert!(Vec2::new(q.rect.x, q.rect.y).x <= 19.0);
}

/// What the same branch draws for a quad that carries a turn (ADR 0041):
/// the vertex stage turns the quad's corners about its centre, so a
/// framebuffer pixel reads the mask where the turn, undone, puts it —
/// between texels, through the linear sampler.
fn turned_coverage(core: &Core, q: &Quad, x: f32, y: f32) -> f32 {
    let (cx, cy) = (q.rect.x + q.rect.w * 0.5, q.rect.y + q.rect.h * 0.5);
    let (sin, cos) = (-q.blur).sin_cos();
    let (dx, dy) = (x - cx, y - cy);
    // The quad's own space, in texels from its corner.
    let lx = dx * cos - dy * sin + q.rect.w * 0.5;
    let ly = dx * sin + dy * cos + q.rect.h * 0.5;
    if lx < 0.0 || ly < 0.0 || lx >= q.rect.w || ly >= q.rect.h {
        return 0.0;
    }
    let texel = |tx: i32, ty: i32| -> f32 {
        let tx = (q.uv[0] as i32 + tx.clamp(0, q.uv[2] as i32 - 1)) as u32;
        let ty = (q.uv[1] as i32 + ty.clamp(0, q.uv[3] as i32 - 1)) as u32;
        f32::from(core.atlas.pixels[((ty * core.atlas.size + tx) * 4 + 3) as usize]) / 255.0
    };
    // Bilinear, texel centres at the half.
    let (fx, fy) = (lx - 0.5, ly - 0.5);
    let (x0, y0) = (fx.floor(), fy.floor());
    let (ax, ay) = (fx - x0, fy - y0);
    let (x0, y0) = (x0 as i32, y0 as i32);
    let top = texel(x0, y0) * (1.0 - ax) + texel(x0 + 1, y0) * ax;
    let bottom = texel(x0, y0 + 1) * (1.0 - ax) + texel(x0 + 1, y0 + 1) * ax;
    (top * (1.0 - ay) + bottom * ay) * q.color.a
}

/// The largest difference, over the canvas's pixels, between a path
/// turned by its quad and the same path with the turn in its ops,
/// rasterized where it lands; and the largest among pixels the
/// reference covers wholly or not at all.
fn turn_error(
    turned: impl FnOnce(&mut kui_core::Ui<'_>),
    reference: impl FnOnce(&mut kui_core::Ui<'_>),
) -> (f32, f32) {
    let (tc, tq) = masks(turned, 1.0);
    let (rc, rq) = masks(reference, 1.0);
    assert_eq!((tq.len(), rq.len()), (1, 1));
    let (mut worst, mut worst_flat) = (0.0f32, 0.0f32);
    for y in 0..100 {
        for x in 0..200 {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let want = coverage(&rc, &rq[0], px, py).unwrap_or(0.0);
            let got = turned_coverage(&tc, &tq[0], px, py);
            let d = (want - got).abs();
            worst = worst.max(d);
            // Flat: this pixel and its neighbours agree in the reference.
            let flat = [(-1.0, 0.0), (1.0, 0.0), (0.0, -1.0), (0.0, 1.0)]
                .iter()
                .all(|(ox, oy)| {
                    (coverage(&rc, &rq[0], px + ox, py + oy).unwrap_or(0.0) - want).abs() < 0.01
                });
            if flat {
                worst_flat = worst_flat.max(d);
            }
        }
    }
    (worst, worst_flat)
}

/// ADR 0041's measurement: a wedge turned by its quad against the same
/// wedge rasterized turned. Inside and outside they agree; on the edge
/// the turned one is the upright ramp resampled, and differs by a
/// fraction of a pixel's coverage.
#[test]
fn a_wedge_turned_by_its_quad_is_the_wedge_rasterized_turned() {
    let black = || NodeSpec::column().bg(Color::rgba8(0, 0, 0, 255));
    for degrees in [7.0f32, 45.0, 90.0] {
        let turns = degrees / 360.0;
        let (worst, flat) = turn_error(
            |ui| {
                ui.path(
                    &Path::sector(100.0, 50.0, 40.0, 0.0, 0.0, 0.3)
                        .pivot(100.0, 50.0)
                        .rotated(turns),
                    black(),
                );
            },
            |ui| {
                ui.path(&Path::sector(100.0, 50.0, 40.0, 0.0, turns, 0.3), black());
            },
        );
        println!("wedge {degrees}°: edge {worst:.3}, flat {flat:.3}");
        assert!(
            flat < 0.02,
            "{degrees}°: {flat} off where the wedge is flat"
        );
        assert!(worst < 0.35, "{degrees}°: {worst} off on an edge");
    }
}

/// The same for a thin stroke, the case a resampled mask is worst at:
/// recorded, and held to half a pixel's coverage.
#[test]
fn a_thin_arc_turned_by_its_quad_keeps_its_line() {
    let arc = |from: f32| {
        let (a0, a1) = (
            from * std::f32::consts::TAU,
            (from + 0.3) * std::f32::consts::TAU,
        );
        Path::new()
            .move_to(100.0 + 30.0 * a0.cos(), 50.0 + 30.0 * a0.sin())
            .arc_to(
                30.0,
                30.0,
                0.0,
                false,
                true,
                100.0 + 30.0 * a1.cos(),
                50.0 + 30.0 * a1.sin(),
            )
            .stroked(kui_core::Stroke::new(1.5, Color::rgba8(0, 0, 0, 255)))
    };
    for degrees in [7.0f32, 45.0, 90.0] {
        let turns = degrees / 360.0;
        let (worst, _) = turn_error(
            |ui| {
                ui.path(
                    &arc(0.0).pivot(100.0, 50.0).rotated(turns),
                    NodeSpec::column(),
                );
            },
            |ui| ui.path(&arc(turns), NodeSpec::column()),
        );
        println!("1.5 px arc {degrees}°: {worst:.3}");
        assert!(worst < 0.5, "{degrees}°: {worst} off along the line");
    }
}

/// ADR 0041, decision 7: two wedges that share an edge and turn together
/// about one pivot still leave no seam.
#[test]
fn two_wedges_turned_together_leave_no_seam() {
    let turns = 0.07;
    let (core, q) = masks(
        |ui| {
            for (from, color) in [
                (0.0, Color::rgba8(255, 0, 0, 255)),
                (0.25, Color::rgba8(0, 0, 255, 255)),
            ] {
                ui.path(
                    &Path::sector(100.0, 50.0, 40.0, 0.0, from, 0.25)
                        .pivot(100.0, 50.0)
                        .rotated(turns),
                    NodeSpec::column().bg(color),
                );
            }
        },
        1.0,
    );
    assert_eq!(q.len(), 2);
    // Along the shared edge, a quarter turn plus the turn from east.
    let a = (0.25 + turns) * std::f32::consts::TAU;
    let (nx, ny) = (-a.sin(), a.cos());
    let mut least = 1.0f32;
    for r in [10.0f32, 20.0, 30.0] {
        for off in [-0.5f32, 0.0, 0.5] {
            let x = 100.0 + r * a.cos() + off * nx;
            let y = 50.0 + r * a.sin() + off * ny;
            let (x, y) = (x.floor() + 0.5, y.floor() + 0.5);
            let c0 = turned_coverage(&core, &q[0], x, y);
            let c1 = turned_coverage(&core, &q[1], x, y);
            least = least.min(c0 + c1 * (1.0 - c0));
        }
    }
    println!("turned seam: least {least:.3}");
    assert!(least > 0.9, "the seam shows: {least}");
}
