//! Per-corner radii: `radius` rounds all four, the per-corner builders
//! override one, the quads carry all four (scaled), and a transition eases
//! each corner independently.

use kui_core::{Core, NodeSpec, Size, Sizing, corner};

fn quad_radii(core: &mut Core, spec: NodeSpec, scale: f32) -> [f32; 4] {
    let mut ui = core.frame(Size::new(200.0, 100.0), scale);
    ui.with_keyed(
        "box",
        spec.bg(kui_core::Color::WHITE)
            .width(Sizing::Fixed(80.0))
            .height(Sizing::Fixed(40.0)),
        |_| {},
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads[0].radius
}

#[test]
fn radius_rounds_all_corners_and_builders_override_one() {
    let mut core = Core::new();
    assert_eq!(
        quad_radii(&mut core, NodeSpec::column().radius(6.0), 1.0),
        [6.0; 4]
    );
    // Later builders win, like CSS shorthand then longhand.
    assert_eq!(
        quad_radii(
            &mut core,
            NodeSpec::column().radius(6.0).radius_tr(0.0),
            1.0
        ),
        [6.0, 0.0, 6.0, 6.0]
    );
    assert_eq!(
        quad_radii(&mut core, NodeSpec::column().radius_top(8.0), 1.0),
        [8.0, 8.0, 0.0, 0.0]
    );
    let r = quad_radii(&mut core, NodeSpec::column().radii(1.0, 2.0, 3.0, 4.0), 2.0);
    assert_eq!(r, [2.0, 4.0, 6.0, 8.0], "quad radii are physical px");
    assert_eq!(r[corner::BL], 8.0);
}

#[test]
fn transition_eases_each_corner() {
    let mut core = Core::new();
    let spec = |tl: f32| {
        NodeSpec::column()
            .radii(tl, 0.0, 0.0, 10.0)
            .transition(100.0)
    };
    core.set_time(0.0);
    assert_eq!(quad_radii(&mut core, spec(0.0), 1.0), [0.0, 0.0, 0.0, 10.0]);
    // The retarget frame starts where it was; the following frames ease.
    assert_eq!(
        quad_radii(&mut core, spec(20.0), 1.0),
        [0.0, 0.0, 0.0, 10.0]
    );
    core.set_time(0.05);
    let mid = quad_radii(&mut core, spec(20.0), 1.0);
    assert!(
        mid[corner::TL] > 0.0 && mid[corner::TL] < 20.0,
        "tl mid-flight: {mid:?}"
    );
    assert_eq!(mid[corner::BL], 10.0, "unchanged corners stay put");
    assert!(core.animating());
    core.set_time(1.0);
    assert_eq!(
        quad_radii(&mut core, spec(20.0), 1.0),
        [20.0, 0.0, 0.0, 10.0]
    );
}
