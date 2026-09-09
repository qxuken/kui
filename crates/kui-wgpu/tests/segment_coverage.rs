//! The half of the segment contract shader validation cannot see: the core
//! writes the endpoints into `uv` as bits and pads the quad past the ramp,
//! the shader reads them back and evaluates a capsule in framebuffer space.
//! Get either side wrong — bits read as texels, a pad short of the ramp, a
//! width where a radius was meant — and the stroke still compiles, still
//! draws, and is the wrong shape in the wrong place.
//!
//! This mirrors `shade`'s `kind == 6u` branch in `src/shader.wgsl` on the
//! CPU (over the shared transcription in `tests/wgsl`) and evaluates it over
//! quads the core actually emitted.

use kui_core::{Clip, Color, Core, NodeSpec, Quad, QuadKind, Size, Stroke, Vec2};

mod wgsl;
use wgsl::{AA, inside, sd_segment, smoothstep};

/// The segment branch of `shade`, at a point in viewport (physical px)
/// space — what `instance_of` hands the shader, decoded the way it does.
fn coverage(q: &Quad, clip: &Clip, x: f32, y: f32) -> f32 {
    let e = q.segment_ends();
    let d = sd_segment([x, y], [e[0], e[1]], [e[2], e[3]], q.border_w * 0.5);
    q.color.a * (1.0 - smoothstep(-AA, AA, d)) * inside(clip, x, y)
}

/// One stroke in a 200×100 frame at the given scale, with the clip its
/// quads name — they all name the same one, since nothing here clips.
fn stroke(points: &[Vec2], stroke: Stroke, scale: f32) -> (Vec<Quad>, Clip) {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 100.0), scale);
    ui.polyline(points, stroke, NodeSpec::column());
    ui.finish();
    let dl = core.output().0;
    let quads = dl.quads.clone();
    assert!(quads.iter().all(|q| q.kind == QuadKind::Segment));
    let clip = quads.first().map_or(Clip::NONE, |q| dl.clip_of(q));
    (quads, clip)
}

/// [`stroke`] for the strokes that are one segment.
fn stroke_one(points: &[Vec2], s: Stroke, scale: f32) -> (Quad, Clip) {
    let (quads, clip) = stroke(points, s, scale);
    (quads[0], clip)
}

#[test]
fn the_stroke_lands_on_the_segment_the_core_described() {
    // A horizontal stroke 6 wide from (20, 50) to (120, 50).
    let (q, clip) = stroke_one(
        &[Vec2::new(20.0, 50.0), Vec2::new(120.0, 50.0)],
        Stroke::new(6.0, Color::rgba8(0, 0, 0, 255)),
        1.0,
    );
    assert!(
        coverage(&q, &clip, 70.0, 50.0) > 0.99,
        "opaque on the centre line"
    );
    assert!(
        coverage(&q, &clip, 70.0, 52.0) > 0.99,
        "opaque 2px off it, inside a 3px radius"
    );
    let edge = coverage(&q, &clip, 70.0, 53.0);
    assert!(
        (edge - 0.5).abs() < 0.05,
        "half on the stroke's edge: {edge}"
    );
    assert!(
        coverage(&q, &clip, 70.0, 56.0) < 0.01,
        "nothing 3px past it"
    );
    // The caps are round: the corner of the bounding box is empty, and the
    // point straight past the end at the radius is the cap's edge.
    assert!(coverage(&q, &clip, q.rect.x + 0.5, q.rect.y + 0.5) < 0.01);
    let cap = coverage(&q, &clip, 123.0, 50.0);
    assert!(
        (cap - 0.5).abs() < 0.05,
        "half at the end cap's edge: {cap}"
    );
    assert!(coverage(&q, &clip, 121.0, 50.0) > 0.9, "inside the cap");
    // The quad is padded past the ramp, so nothing is cut by its edge:
    // zero at every side of the box.
    assert!(coverage(&q, &clip, 70.0, q.rect.y + 0.5) < 0.01);
    assert!(coverage(&q, &clip, 70.0, q.rect.y + q.rect.h - 0.5) < 0.01);
    assert!(coverage(&q, &clip, q.rect.x + 0.5, 50.0) < 0.01);
    assert!(coverage(&q, &clip, q.rect.x + q.rect.w - 0.5, 50.0) < 0.01);
}

#[test]
fn a_diagonal_is_covered_along_its_length_and_not_in_its_box_corners() {
    let (q, clip) = stroke_one(
        &[Vec2::new(10.0, 10.0), Vec2::new(90.0, 70.0)],
        Stroke::new(4.0, Color::rgba8(0, 0, 0, 255)),
        1.0,
    );
    for t in [0.1f32, 0.5, 0.9] {
        let (x, y) = (10.0 + 80.0 * t, 10.0 + 60.0 * t);
        assert!(coverage(&q, &clip, x, y) > 0.99, "on the line at {t}");
    }
    // The bounding box's other two corners are far from the line.
    assert!(coverage(&q, &clip, 88.0, 12.0) < 0.01);
    assert!(coverage(&q, &clip, 12.0, 68.0) < 0.01);
}

#[test]
fn scale_is_applied_once_on_the_core_side() {
    // At scale 2 the same stroke is twice as wide in physical px, and the
    // endpoints are where the core put them, not doubled again.
    let (q, clip) = stroke_one(
        &[Vec2::new(20.0, 50.0), Vec2::new(120.0, 50.0)],
        Stroke::new(6.0, Color::rgba8(0, 0, 0, 255)),
        2.0,
    );
    assert_eq!(q.segment_ends(), [40.0, 100.0, 240.0, 100.0]);
    assert!(
        coverage(&q, &clip, 140.0, 105.0) > 0.99,
        "5 physical px off centre is inside a 6px radius"
    );
    let edge = coverage(&q, &clip, 140.0, 106.0);
    assert!((edge - 0.5).abs() < 0.05, "half at 6px: {edge}");
}

#[test]
fn a_polyline_joins_at_its_knot_without_a_gap() {
    // An elbow: the join is covered by both pieces' round caps, so a point
    // on the outer corner of the turn, within the radius, is still opaque.
    let (quads, clip) = stroke(
        &[
            Vec2::new(20.0, 20.0),
            Vec2::new(60.0, 20.0),
            Vec2::new(60.0, 60.0),
        ],
        Stroke::new(8.0, Color::rgba8(0, 0, 0, 255)),
        1.0,
    );
    assert_eq!(quads.len(), 2);
    let at = |x, y| {
        quads
            .iter()
            .map(|q| coverage(q, &clip, x, y))
            .fold(0.0f32, f32::max)
    };
    assert!(at(62.0, 18.0) > 0.99, "the outer corner is inside a cap");
    assert!(at(65.0, 15.0) < 0.5, "and the box corner past it is not");
}
