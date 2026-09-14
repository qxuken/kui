//! Transitions through a live `Core`: a split whose ratio changes eases its
//! children's widths across frames when the driver supplies a clock, and
//! snaps without one.

use kui_core::{
    Color, Core, Easing, Enter, FloatConfig, Keyframe, NodeSpec, Repeat, Size, Sizing, Transition,
};

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

/// Two keyed tabs in a row; returns the x of each (they carry backgrounds,
/// so they are the first two quads in tree order).
fn tab_xs(core: &mut Core, order: [&str; 2], slide: bool) -> Vec<(String, f32)> {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill().gap(10.0));
    for label in order {
        let mut spec = NodeSpec::column()
            .width(Sizing::Fixed(50.0))
            .height(Sizing::Fixed(20.0))
            .bg(kui_core::Color::WHITE)
            .transition_with(Transition::ms(100.0).easing(Easing::Linear));
        if slide {
            spec = spec.slide();
        }
        ui.with_keyed(label, spec, |_| {});
    }
    ui.finish();
    let (dl, _) = core.output();
    order
        .iter()
        .zip(dl.quads.iter())
        .map(|(l, q)| (l.to_string(), q.rect.x))
        .collect()
}

#[test]
fn slide_eases_reordered_siblings_into_place() {
    let mut core = Core::new();
    core.set_time(0.0);
    let xs = tab_xs(&mut core, ["a", "b"], true);
    assert_eq!(xs[0], ("a".into(), 0.0));
    assert_eq!(xs[1], ("b".into(), 60.0));

    // Swap: this frame both still draw where they were.
    core.set_time(0.0);
    let xs = tab_xs(&mut core, ["b", "a"], true);
    assert_eq!(xs[0], ("b".into(), 60.0), "b starts from its old slot");
    assert_eq!(xs[1], ("a".into(), 0.0));
    assert!(core.animating());

    core.set_time(0.05);
    let xs = tab_xs(&mut core, ["b", "a"], true);
    assert!((xs[0].1 - 30.0).abs() < 1.0, "b halfway: {}", xs[0].1);
    assert!((xs[1].1 - 30.0).abs() < 1.0, "a halfway: {}", xs[1].1);

    core.set_time(0.2);
    let xs = tab_xs(&mut core, ["b", "a"], true);
    assert_eq!(xs[0].1, 0.0);
    assert_eq!(xs[1].1, 60.0);
    assert!(!core.animating());
}

#[test]
fn without_slide_a_transition_does_not_move_positions() {
    let mut core = Core::new();
    core.set_time(0.0);
    tab_xs(&mut core, ["a", "b"], false);
    core.set_time(0.0);
    let xs = tab_xs(&mut core, ["b", "a"], false);
    assert_eq!(xs[0].1, 0.0, "b snaps to its new slot");
    assert!(!core.animating());
}

#[test]
fn request_frame_owes_exactly_one_frame() {
    let mut core = Core::new();
    let frame = |core: &mut Core, ask: bool| {
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        if ask {
            ui.request_frame();
        }
        ui.finish();
    };
    frame(&mut core, false);
    assert!(!core.animating());
    frame(&mut core, true);
    assert!(core.animating(), "the view asked for another frame");
    frame(&mut core, false);
    assert!(!core.animating(), "and only one");
}

/// The `animate` row is `request_frame` as a declaration: the frame owes
/// another for as long as a declared node carries it, and stops owing the
/// frame after the last one that does — the class of regression C27
/// measured as idle CPU, which until backlog AR47 no test in any binding
/// pinned (the C struct round-trip alone touched the row).
#[test]
fn an_animate_node_owes_a_frame_while_it_is_declared() {
    let mut core = Core::new();
    let frame = |core: &mut Core, animate: bool| {
        let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
        let mut spec = NodeSpec::column().width(Sizing::Fixed(10.0));
        if animate {
            spec = spec.animate();
        }
        ui.with_keyed("node", spec, |_| {});
        ui.finish();
    };
    frame(&mut core, false);
    assert!(!core.animating());
    frame(&mut core, true);
    assert!(
        core.animating(),
        "a declared `animate` asks for the next frame"
    );
    frame(&mut core, true);
    assert!(core.animating(), "and keeps asking while it is declared");
    frame(&mut core, false);
    assert!(
        !core.animating(),
        "the frame that stops declaring it stops asking — nothing is retained"
    );
}

/// A row whose left half is keyframed between grow 0 and grow 1 against a
/// grow-1 sibling; returns its laid-out width.
fn frame_keyframed(core: &mut Core, now: Option<f64>, repeat: Repeat) -> f32 {
    if let Some(now) = now {
        core.set_time(now);
    }
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let left = NodeSpec::column()
        .width(Sizing::Grow(0.0))
        .height(Sizing::Grow(1.0))
        .bg(Color::WHITE)
        .transition_with(Transition::ms(1000.0).easing(Easing::Linear).repeat(repeat))
        .keyframes(vec![
            Keyframe::default().width(Sizing::Grow(0.0)),
            Keyframe::default().width(Sizing::Grow(1.0)),
        ]);
    let right = NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Grow(1.0));
    ui.with_keyed("left", left, |_| {});
    ui.with_keyed("right", right, |_| {});
    ui.finish();
    left_width(core)
}

#[test]
fn keyframes_cycle_from_the_first_frame_without_retargets() {
    // The view never changes: the same spec every frame, and the width
    // still moves — a keyframed slot reads the clock, not a retarget.
    let mut core = Core::new();
    let w = |core: &mut Core, now: f64| frame_keyframed(core, Some(now), Repeat::Alternate);
    assert_eq!(w(&mut core, 0.0), 0.0);
    assert!(core.animating(), "a keyframed node always owes a frame");
    let half = w(&mut core, 0.5);
    assert!((half - 400.0 / 3.0).abs() < 1.0, "grow 0.5 of 1.5: {half}");
    assert_eq!(w(&mut core, 1.0), 200.0);
    // Alternate: on the way back.
    let back = w(&mut core, 1.5);
    assert!((back - 400.0 / 3.0).abs() < 1.0, "{back}");
    assert!(core.animating());
    // Normal wraps instead.
    let wrapped = frame_keyframed(&mut core, Some(1.5), Repeat::Normal);
    assert!((wrapped - 400.0 / 3.0).abs() < 1.0, "{wrapped}");
    assert_eq!(frame_keyframed(&mut core, Some(2.0), Repeat::Normal), 0.0);
}

#[test]
fn keyframes_without_a_clock_hold_the_declared_value() {
    let mut core = Core::new();
    assert_eq!(frame_keyframed(&mut core, None, Repeat::Alternate), 0.0);
    assert_eq!(frame_keyframed(&mut core, None, Repeat::Alternate), 0.0);
    assert!(!core.animating());
}

#[test]
fn a_slot_the_keyframes_skip_still_tweens() {
    // Keyframes on the width only: a bg change on the same node eases the
    // usual way, from the value it had.
    let mut core = Core::new();
    let frame = |core: &mut Core, now: f64, bg: Color| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        let spec = NodeSpec::column()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Grow(1.0))
            .bg(bg)
            .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
            .keyframes(vec![Keyframe::default().width(Sizing::Fixed(200.0))]);
        ui.with_keyed("k", spec, |_| {});
        ui.finish();
        let (dl, _) = core.output();
        let q = dl.quads.first().expect("a quad");
        (q.rect.w, q.color.r)
    };
    let (w, r) = frame(&mut core, 0.0, Color::BLACK);
    assert_eq!((w, r), (100.0, 0.0), "at 0: base width, first sight of bg");
    let (w, r) = frame(&mut core, 0.5, Color::WHITE);
    assert!((w - 150.0).abs() < 1e-3, "keyframed: {w}");
    assert_eq!(r, 0.0, "bg retargets from where it was");
    let (_, r) = frame(&mut core, 1.0, Color::WHITE);
    assert!(
        (r - 0.5).abs() < 1e-3,
        "bg halfway through its own 1s leg: {r}"
    );
}

/// A viewport float at `dx`; returns its drawn x (its background is the
/// first quad).
fn float_x(core: &mut Core, dx: f32, spec: NodeSpec) -> f32 {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let spec = spec
        .width(Sizing::Fixed(50.0))
        .height(Sizing::Fixed(20.0))
        .bg(Color::WHITE)
        .float(FloatConfig::viewport().offset(dx, 0.0));
    ui.with_keyed("toast", spec, |_| {});
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.first().map_or(f32::NAN, |q| q.rect.x)
}

fn linear(ms: f32) -> Transition {
    Transition::ms(ms).easing(Easing::Linear)
}

#[test]
fn a_sliding_float_glides_to_its_new_offset() {
    // `float.dx` is a layout input like any other: with `slide` the node
    // eases from where it was drawn to where the new offset puts it.
    let spec = || NodeSpec::column().transition_with(linear(100.0)).slide();
    let mut core = Core::new();
    core.set_time(0.0);
    assert_eq!(
        float_x(&mut core, -50.0, spec()),
        -50.0,
        "first sight snaps"
    );
    core.set_time(0.0);
    assert_eq!(
        float_x(&mut core, 100.0, spec()),
        -50.0,
        "starts from the old offset"
    );
    assert!(core.animating());
    core.set_time(0.05);
    assert!((float_x(&mut core, 100.0, spec()) - 25.0).abs() < 1e-3);
    core.set_time(0.2);
    assert_eq!(float_x(&mut core, 100.0, spec()), 100.0);
    assert!(!core.animating());
}

#[test]
fn enter_slides_a_node_in_on_first_sight() {
    let spec = || {
        NodeSpec::column()
            .transition_with(linear(100.0))
            .enter(Enter::from(-100.0, 0.0))
    };
    let mut core = Core::new();
    core.set_time(0.0);
    assert_eq!(
        float_x(&mut core, 100.0, spec()),
        0.0,
        "starts dx away from its place"
    );
    assert!(core.animating(), "the entrance owes a frame");
    core.set_time(0.05);
    assert!((float_x(&mut core, 100.0, spec()) - 50.0).abs() < 1e-3);
    core.set_time(0.2);
    assert_eq!(float_x(&mut core, 100.0, spec()), 100.0);
    assert!(!core.animating());
    // Without `slide`, only the entrance moved: a later move snaps.
    core.set_time(0.2);
    assert_eq!(float_x(&mut core, 300.0, spec()), 300.0);
    assert!(!core.animating());
    // Gone for a frame and back: it enters again.
    core.set_time(0.3);
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    ui.finish();
    core.set_time(0.3);
    assert_eq!(float_x(&mut core, 300.0, spec()), 200.0);
    assert!(core.animating());
}

#[test]
fn enter_with_slide_keeps_following_layout() {
    let spec = || {
        NodeSpec::column()
            .transition_with(linear(100.0))
            .enter(Enter::from(-100.0, 0.0))
            .slide()
    };
    let mut core = Core::new();
    core.set_time(0.0);
    assert_eq!(float_x(&mut core, 100.0, spec()), 0.0);
    core.set_time(0.2);
    assert_eq!(float_x(&mut core, 100.0, spec()), 100.0);
    core.set_time(0.2);
    assert_eq!(
        float_x(&mut core, 300.0, spec()),
        100.0,
        "a move still eases"
    );
    core.set_time(0.25);
    assert!((float_x(&mut core, 300.0, spec()) - 200.0).abs() < 1e-3);
}

#[test]
fn enter_fades_a_background_in() {
    fn alpha(core: &mut Core, enter: Enter) -> f32 {
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        let spec = NodeSpec::column()
            .width(Sizing::Fixed(50.0))
            .height(Sizing::Fixed(20.0))
            .bg(Color::WHITE)
            .transition_with(linear(100.0))
            .enter(enter);
        ui.with_keyed("toast", spec, |_| {});
        ui.finish();
        let (dl, _) = core.output();
        dl.quads.first().map_or(f32::NAN, |q| q.color.a)
    }
    // A fully transparent box emits no quad, so enter from a faint one.
    let faint = Color {
        a: 0.2,
        ..Color::WHITE
    };
    let mut core = Core::new();
    core.set_time(0.0);
    assert!((alpha(&mut core, Enter::default().bg(faint)) - 0.2).abs() < 1e-6);
    assert!(core.animating());
    core.set_time(0.05);
    assert!((alpha(&mut core, Enter::default().bg(faint)) - 0.6).abs() < 1e-3);
    core.set_time(0.2);
    assert_eq!(alpha(&mut core, Enter::default().bg(faint)), 1.0);
    assert!(!core.animating());
}

#[test]
fn enter_without_a_clock_snaps_like_everything_else() {
    let spec = || {
        NodeSpec::column()
            .transition_with(linear(100.0))
            .enter(Enter::from(-100.0, 0.0))
    };
    let mut core = Core::new();
    assert_eq!(float_x(&mut core, 100.0, spec()), 100.0);
    assert!(!core.animating());
}
