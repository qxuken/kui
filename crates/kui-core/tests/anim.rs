//! Transitions through a live `Core`: a split whose ratio changes eases its
//! children's widths across frames when the driver supplies a clock, and
//! snaps without one.

use kui_core::{Core, Easing, NodeSpec, Size, Sizing, Transition};

fn left_width(core: &mut Core) -> f32 {
    let (dl, _) = core.output();
    dl.quads.first().map_or(0.0, |q| q.rect.w)
}

/// Two Grow children sharing 400px; returns the left child's laid-out width
/// (it carries a background, so it is the first quad).
fn frame_bg(core: &mut Core, ratio: f32, transition: Option<Transition>) -> f32 {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let mut left = NodeSpec::column()
        .width(Sizing::Grow(ratio))
        .height(Sizing::Grow(1.0))
        .bg(kui_core::Color::WHITE);
    let mut right = NodeSpec::column()
        .width(Sizing::Grow(1.0 - ratio))
        .height(Sizing::Grow(1.0));
    if let Some(t) = transition {
        left = left.transition_with(t);
        right = right.transition_with(t);
    }
    ui.with_keyed("left", left, |_| {});
    ui.with_keyed("right", right, |_| {});
    ui.finish();
    left_width(core)
}

#[test]
fn grow_factors_ease_between_frames() {
    let mut core = Core::new();
    let t = Some(Transition::ms(100.0).easing(Easing::Linear));
    core.set_time(0.0);
    assert_eq!(frame_bg(&mut core, 0.5, t), 200.0, "first sight snaps");
    assert!(!core.animating());

    core.set_time(0.0);
    assert_eq!(
        frame_bg(&mut core, 1.0, t),
        200.0,
        "retarget starts where it was"
    );
    assert!(core.animating(), "a frame is owed");

    core.set_time(0.05);
    let mid = frame_bg(&mut core, 1.0, t);
    assert!((mid - 300.0).abs() < 1.0, "halfway: {mid}");
    assert!(core.animating());

    core.set_time(0.2);
    assert_eq!(frame_bg(&mut core, 1.0, t), 400.0);
    assert!(!core.animating(), "settled: no more frames owed");
}

#[test]
fn no_clock_snaps() {
    let mut core = Core::new();
    let t = Some(Transition::ms(100.0));
    frame_bg(&mut core, 0.5, t);
    assert_eq!(frame_bg(&mut core, 1.0, t), 400.0);
    assert!(!core.animating());
}

#[test]
fn a_frame_without_the_transition_resets_it() {
    // A divider drag renders the split without its transition so the ratio
    // tracks the cursor; when the transition comes back it must not replay
    // the whole drag.
    let mut core = Core::new();
    let t = Some(Transition::ms(100.0));
    core.set_time(0.0);
    frame_bg(&mut core, 0.5, t);
    core.set_time(0.01);
    frame_bg(&mut core, 0.9, None);
    core.set_time(0.02);
    assert_eq!(
        frame_bg(&mut core, 0.9, t),
        360.0,
        "snaps to the dragged ratio"
    );
    assert!(!core.animating());
}

#[test]
fn colors_ease_too() {
    let mut core = Core::new();
    let t = Transition::ms(100.0).easing(Easing::Linear);
    let paint = |core: &mut Core, a: f32| {
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        ui.with_keyed(
            "chip",
            NodeSpec::column()
                .width(Sizing::Fixed(10.0))
                .height(Sizing::Fixed(10.0))
                .bg(kui_core::Color::rgba(1.0, 1.0, 1.0, a))
                .transition_with(t),
            |_| {},
        );
        ui.finish();
        let (dl, _) = core.output();
        dl.quads[0].color.a
    };
    core.set_time(0.0);
    assert_eq!(paint(&mut core, 0.2), 0.2);
    core.set_time(0.0);
    paint(&mut core, 1.0);
    core.set_time(0.05);
    assert!((paint(&mut core, 1.0) - 0.6).abs() < 1e-3);
}
