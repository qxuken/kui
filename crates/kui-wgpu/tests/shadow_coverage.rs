//! The half of the shadow contract shader validation cannot see: the core
//! inflates the quad by `blur` and the shader insets by the same amount to
//! recover the shape. Get either sign wrong and the shadow still compiles,
//! still draws, and sits in the wrong place at the wrong size.
//!
//! This mirrors `shade`'s `kind == 5u` branch in `src/shader.wgsl` on the CPU
//! and evaluates it over a quad the core actually emitted, so the two halves
//! are checked against each other rather than each against itself.

use kui_core::{Color, Core, NodeSpec, Quad, QuadKind, Shadow, Size, Sizing};

/// `sd_rounded_box` from the shader; `p` is centred, y down.
fn sd_rounded_box(p: [f32; 2], half: [f32; 2], radii: [f32; 4]) -> f32 {
    let right = p[0] > 0.0;
    let bottom = p[1] > 0.0;
    let top_r = if right { radii[1] } else { radii[0] };
    let bottom_r = if right { radii[2] } else { radii[3] };
    let r = if bottom { bottom_r } else { top_r };
    let rr = r.min(half[0].min(half[1]));
    let q = [p[0].abs() - half[0] + rr, p[1].abs() - half[1] + rr];
    let outside = (q[0].max(0.0).powi(2) + q[1].max(0.0).powi(2)).sqrt();
    outside + q[0].max(q[1]).min(0.0) - rr
}

fn smoothstep(e0: f32, e1: f32, x: f32) -> f32 {
    let t = ((x - e0) / (e1 - e0)).clamp(0.0, 1.0);
    t * t * (3.0 - 2.0 * t)
}

/// The shadow branch of `shade`, at a point in viewport (physical px) space.
fn coverage(q: &Quad, x: f32, y: f32) -> f32 {
    let local = [x - q.rect.x, y - q.rect.y];
    let half = [q.rect.w * 0.5, q.rect.h * 0.5];
    let shape = [(half[0] - q.blur).max(0.0), (half[1] - q.blur).max(0.0)];
    let d = sd_rounded_box([local[0] - half[0], local[1] - half[1]], shape, q.radius);
    let ramp = q.blur.max(0.75);
    q.color.a * (1.0 - smoothstep(-ramp, ramp, d))
}

/// The first quad of a one-card frame: an 80x40 box at (20, 20).
fn card_shadow(spec: NodeSpec) -> Quad {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    ui.with_keyed(
        "card",
        spec.width(Sizing::Fixed(80.0)).height(Sizing::Fixed(40.0)),
        |_| {},
    );
    ui.finish();
    let q = core.output().0.quads[0];
    assert_eq!(q.kind, QuadKind::Shadow);
    q
}

/// 4px down, blurred over 8, spread 2, behind a 6px-rounded card.
fn blurred() -> Quad {
    card_shadow(
        NodeSpec::column()
            .bg(Color::WHITE)
            .radius(6.0)
            .shadow(Shadow {
                color: Color::rgba8(0, 0, 0, 255),
                dx: 0.0,
                dy: 4.0,
                blur: 8.0,
                spread: 2.0,
            }),
    )
}

#[test]
fn the_shadow_lands_on_the_shape_the_core_described() {
    let q = blurred();
    // The shape is the card (20,20 80x40) moved down 4 and grown by 2:
    // x 18..100, y 22..64. Solid well inside it.
    assert!(coverage(&q, 60.0, 42.0) > 0.99, "opaque in the middle");
    assert!(coverage(&q, 30.0, 30.0) > 0.99, "opaque inside a corner");
    // Half coverage on the shape's own edge, where the ramp is centred.
    let edge = coverage(&q, 60.0, 22.0);
    assert!(
        (edge - 0.5).abs() < 0.05,
        "half at the shape's edge: {edge}"
    );
    // Nothing at the quad's outer boundary: the inflation is exactly the
    // reach of the ramp, so the shadow is not clipped by its own quad.
    let out = coverage(&q, 60.0, q.rect.y + 0.5);
    assert!(out < 0.01, "faded out by the quad's edge: {out}");
    assert!(coverage(&q, q.rect.x + 0.5, 42.0) < 0.01);
}

/// The offset moves the shape, not the quad's centre: the band above the
/// card stays faint while the band below it is nearly solid.
#[test]
fn the_offset_casts_the_shadow_downward() {
    let q = blurred();
    let above = coverage(&q, 60.0, 17.0);
    let below = coverage(&q, 60.0, 63.0);
    assert!(above < 0.35, "little above the card: {above}");
    assert!(below > 0.65, "much below it: {below}");
}

/// A blur of zero is the shape itself — no inflation to inset back out of —
/// and the SDF still antialiases its edge.
#[test]
fn a_hard_shadow_is_the_shape_itself() {
    let q = card_shadow(NodeSpec::column().shadow_color(Color::rgba8(0, 0, 0, 255)));
    assert_eq!(
        (q.rect.x, q.rect.y, q.rect.w, q.rect.h),
        (20.0, 20.0, 80.0, 40.0)
    );
    assert!(coverage(&q, 60.0, 40.0) > 0.99);
    assert!(coverage(&q, 60.0, 21.0) > 0.7, "its top edge is covered");
    assert!(coverage(&q, 60.0, 19.0) == 0.0, "and nothing above it");
}
