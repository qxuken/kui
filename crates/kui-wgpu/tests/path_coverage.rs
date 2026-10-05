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
    assert!(cov(40.5, 80.5) > 0.99, "solid on the first physical pixel in");
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
    // The edge is at 20.5 with half a pixel of bleed to 20: pixel 20 is
    // fully the bleed's, pixel 19 is the bleed's outer half, 18 is empty.
    assert!(cov(21.5, 40.5) > 0.99, "{}", cov(21.5, 40.5));
    assert!(cov(20.5, 40.5) > 0.9, "{}", cov(20.5, 40.5));
    let outer = cov(19.5, 40.5);
    assert!(outer > 0.2 && outer < 0.8, "{outer}");
    assert!(cov(18.5, 40.5) < 0.01);
    assert!(Vec2::new(q.rect.x, q.rect.y).x <= 19.0);
}
