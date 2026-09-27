//! Drop shadows: one `QuadKind::Shadow` quad behind the node, carrying the
//! blurred shape's geometry — the node's rect moved by the offset, grown by
//! the spread, inflated by the blur the backend ramps over.

use kui_core::{Color, Core, NodeSpec, QuadKind, Shadow, Size};

const VIEW: Size = Size { w: 200.0, h: 100.0 };

fn quads(core: &mut Core, spec: NodeSpec, scale: f32) -> Vec<kui_core::Quad> {
    let mut ui = core.frame(VIEW, scale);
    ui.configure_root(NodeSpec::column().pad(20.0));
    ui.leaf_keyed("card", spec.bg(Color::WHITE).size(80.0, 40.0));
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.clone()
}

const SHADOW: Shadow = Shadow {
    color: Color {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.5,
    },
    dx: 0.0,
    dy: 4.0,
    blur: 8.0,
    spread: 2.0,
};

#[test]
fn a_shadow_draws_behind_the_node_offset_spread_and_inflated_by_its_blur() {
    let mut core = Core::new();
    let qs = quads(
        &mut core,
        NodeSpec::column().shadow(SHADOW).radius(6.0),
        1.0,
    );
    assert_eq!(qs.len(), 2);
    let (shadow, box_) = (&qs[0], &qs[1]);
    assert_eq!(shadow.kind, QuadKind::Shadow, "the shadow paints first");
    assert_eq!(box_.kind, QuadKind::Solid);
    assert_eq!(shadow.color.a, 0.5);
    assert_eq!(shadow.blur, 8.0);
    // The node sits at (20, 20), 80x40. The shape is that moved down 4,
    // grown by 2 on every side; the quad is the shape inflated by the blur.
    assert_eq!(shadow.rect.x, 20.0 - 2.0 - 8.0);
    assert_eq!(shadow.rect.y, 20.0 + 4.0 - 2.0 - 8.0);
    assert_eq!(shadow.rect.w, 80.0 + 4.0 + 16.0);
    assert_eq!(shadow.rect.h, 40.0 + 4.0 + 16.0);
    // Radii grow with the spread, so the silhouette keeps its shape.
    assert_eq!(shadow.radius, [8.0; 4]);
}

#[test]
fn shadow_geometry_is_physical_pixels() {
    let mut core = Core::new();
    let qs = quads(&mut core, NodeSpec::column().shadow(SHADOW), 2.0);
    assert_eq!(qs[0].blur, 16.0);
    assert_eq!(qs[0].rect.x, 2.0 * (20.0 - 10.0));
    assert_eq!(qs[0].rect.w, 2.0 * 100.0);
}

/// The color is the whole switch: geometry without one paints nothing, and
/// a color without geometry is a hard shadow exactly behind the node.
#[test]
fn only_the_color_decides_whether_a_shadow_draws() {
    let mut core = Core::new();
    let blur_only = quads(&mut core, NodeSpec::column().shadow_blur(10.0), 1.0);
    assert_eq!(blur_only.len(), 1, "no color, no shadow");

    let color_only = quads(
        &mut core,
        NodeSpec::column().shadow_color(Color::WHITE),
        1.0,
    );
    assert_eq!(color_only.len(), 2);
    assert_eq!(color_only[0].blur, 0.0);
    assert_eq!(color_only[0].rect.x, 20.0);
    assert_eq!(color_only[0].rect.w, 80.0);
}

#[test]
fn a_transition_eases_the_shadow_geometry_and_its_color() {
    let mut core = Core::new();
    let frame = |core: &mut Core, t: f64, sh: Shadow| {
        core.set_time(t);
        let qs = quads(
            core,
            NodeSpec::column()
                .transition(100.0)
                .easing(kui_core::Easing::Linear)
                .shadow(sh),
            1.0,
        );
        (qs[0].blur, qs[0].color.a)
    };
    let lifted = Shadow {
        blur: 16.0,
        ..SHADOW
    };
    assert_eq!(
        frame(&mut core, 0.0, SHADOW),
        (8.0, 0.5),
        "first sight snaps"
    );
    assert_eq!(frame(&mut core, 0.05, lifted), (8.0, 0.5));
    let (blur, alpha) = frame(&mut core, 0.1, lifted);
    assert!(blur > 8.0 && blur < 16.0, "blur easing up: {blur}");
    assert_eq!(alpha, 0.5, "an unchanged color does not move");
    let clear = Shadow {
        color: Color::TRANSPARENT,
        ..lifted
    };
    // The frame that retargets still sits at the old value; the next moves.
    assert_eq!(frame(&mut core, 0.15, clear).1, 0.5);
    let faded = frame(&mut core, 0.2, clear).1;
    assert!(faded > 0.0 && faded < 0.5, "color easing out: {faded}");
}

/// A shadow is one of the node's own quads, so a faded subtree fades it.
#[test]
fn group_opacity_fades_the_shadow_with_the_node() {
    let mut core = Core::new();
    let qs = quads(
        &mut core,
        NodeSpec::column().shadow(SHADOW).opacity(0.5),
        1.0,
    );
    assert_eq!(qs[0].color.a, 0.25);
}
