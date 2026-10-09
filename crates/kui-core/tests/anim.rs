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
        .grow_height()
        .bg(kui_core::Color::WHITE);
    let mut right = NodeSpec::column()
        .width(Sizing::Grow(1.0 - ratio))
        .grow_height();
    if let Some(t) = transition {
        left = left.transition_with(t);
        right = right.transition_with(t);
    }
    ui.leaf_keyed("left", left);
    ui.leaf_keyed("right", right);
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
        ui.leaf_keyed(
            "chip",
            NodeSpec::column()
                .size(10.0, 10.0)
                .bg(kui_core::Color::rgba(1.0, 1.0, 1.0, a))
                .transition_with(t),
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
            .size(50.0, 20.0)
            .bg(kui_core::Color::WHITE)
            .transition_with(Transition::ms(100.0).easing(Easing::Linear));
        if slide {
            spec = spec.slide();
        }
        ui.leaf_keyed(label, spec);
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

/// F111: `owed()` says a frame is owed; `owed_by()` says who holds it,
/// read from inside the view of the frame it caused. A transition names
/// its node by the labels from the root and the slots still moving; a
/// `request_frame` names the line that called it; an `animate` node names
/// itself; and the frame's cause carries `owed`. Off, the record is empty.
#[test]
fn owed_by_names_who_holds_the_frame() {
    use kui_core::{FrameCause, OwedBy};
    let mut core = Core::new();
    core.set_frame_trace(true);
    let t = Transition::ms(100.0).easing(Easing::Linear);
    // One frame: `width` for the bar, whether to ask for the next frame,
    // whether the pulse animates; returns what the view read.
    let frame = |core: &mut Core, now: f64, width: f32, ask: bool, pulse: bool| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        let seen = (ui.core().owed_by().clone(), ui.core().frame_cause());
        ui.with_keyed("panel", NodeSpec::column().fill(), |ui| {
            ui.leaf_keyed(
                "bar",
                NodeSpec::column()
                    .width(width)
                    .height(10.0)
                    .transition_with(t),
            );
            let mut p = NodeSpec::column().width(5.0);
            if pulse {
                p = p.animate();
            }
            ui.leaf(p);
        });
        let line = line!() + 2;
        if ask {
            ui.request_frame();
        }
        ui.finish();
        (seen, line)
    };
    frame(&mut core, 0.0, 10.0, false, false);
    frame(&mut core, 0.01, 100.0, true, true);
    assert!(core.owed().transition && core.owed().requested);
    let ((by, cause), line) = frame(&mut core, 0.05, 100.0, false, false);
    assert!(cause.contains(FrameCause::OWED), "{cause:?}");
    assert_eq!(by.transitions.len(), 1, "{by:?}");
    assert_eq!(by.transitions[0].name, "panel/bar");
    assert_eq!(by.transitions[0].slots, vec!["width"]);
    assert_eq!(by.requests.len(), 1);
    assert_eq!(by.requests[0].why, "request_frame");
    assert!(by.requests[0].at.file().ends_with("anim.rs"));
    assert_eq!(by.requests[0].at.line(), line);
    assert_eq!(by.animate.len(), 1);
    assert!(
        by.animate[0].name.starts_with("panel/#"),
        "an unlabelled node ends in its key: {}",
        by.animate[0].name
    );
    assert!(by.cycles.is_empty() && by.departures.is_empty() && by.scrolls.is_empty());
    // Still mid-leg at 0.05 (the leg began at 0.01), and nothing asked.
    let ((by, _), _) = frame(&mut core, 0.2, 100.0, false, false);
    assert_eq!(by.transitions.len(), 1);
    assert!(by.requests.is_empty() && by.animate.is_empty());
    // The leg ended at 0.11: the frame at 0.2 owed nothing.
    assert!(!core.owed().any());
    let ((by, cause), _) = frame(&mut core, 0.3, 100.0, false, false);
    assert!(by.is_empty(), "{by:?}");
    assert!(!cause.contains(FrameCause::OWED));

    // Off: the reasons are kept, the holders are not.
    core.set_frame_trace(false);
    frame(&mut core, 0.4, 10.0, true, false);
    let ((by, cause), _) = frame(&mut core, 0.45, 10.0, false, false);
    assert_eq!(by, OwedBy::default());
    assert!(cause.contains(FrameCause::OWED));
}

/// F111: the transition holders are read back off the tweens, not
/// recorded while they were driven — so they must agree with the flag
/// `drive` set, frame by frame, to the end of every leg: a linear leg, a
/// spring's settle, and a slide.
#[test]
fn owed_by_transitions_agree_with_owed_every_frame() {
    for easing in [Easing::Linear, Easing::Spring, Easing::Bouncy] {
        let mut core = Core::new();
        core.set_frame_trace(true);
        let t = Transition::ms(120.0).easing(easing);
        let mut now = 0.0;
        let mut owed_before = false;
        for i in 0..200 {
            core.set_time(now);
            let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
            let held = !ui.core().owed_by().transitions.is_empty();
            assert_eq!(held, owed_before, "{easing:?} frame {i}");
            let w = if i == 0 { 10.0 } else { 200.0 };
            ui.leaf_keyed(
                "k",
                NodeSpec::column()
                    .width(w)
                    .height(10.0)
                    .bg(Color::WHITE)
                    .transition_with(t),
            );
            ui.finish();
            owed_before = core.owed().transition;
            now += 1.0 / 60.0;
        }
        assert!(!owed_before, "{easing:?} settled");
    }
}

/// F111: a keyframe cycle and a departure are named as well.
#[test]
fn owed_by_names_cycles_and_departures() {
    let mut core = Core::new();
    core.set_frame_trace(true);
    let frame = |core: &mut Core, now: f64, show: bool| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        let by = ui.core().owed_by().clone();
        ui.leaf_keyed(
            "spinner",
            NodeSpec::column()
                .size(10.0, 10.0)
                .transition_with(Transition::ms(1000.0).repeat(Repeat::Normal))
                .keyframes(vec![Keyframe::default().width(20.0)]),
        );
        if show {
            ui.leaf_keyed(
                "toast",
                NodeSpec::column()
                    .size(80.0, 40.0)
                    .bg(Color::WHITE)
                    .transition(100.0)
                    .exit(Enter::from(40.0, 0.0).opacity(0.0)),
            );
        }
        ui.finish();
        by
    };
    frame(&mut core, 0.0, true);
    frame(&mut core, 0.01, false);
    assert!(core.owed().cycle && core.owed().depart);
    let by = frame(&mut core, 0.02, false);
    let names =
        |hs: &[kui_core::FrameHolder]| hs.iter().map(|h| h.name.clone()).collect::<Vec<_>>();
    assert_eq!(names(&by.cycles), vec!["spinner"]);
    assert_eq!(names(&by.departures), vec!["toast"]);
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
        let mut spec = NodeSpec::column().width(10.0);
        if animate {
            spec = spec.animate();
        }
        ui.leaf_keyed("node", spec);
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
        .grow_height()
        .bg(Color::WHITE)
        .transition_with(Transition::ms(1000.0).easing(Easing::Linear).repeat(repeat))
        .keyframes(vec![
            Keyframe::default().width(Sizing::Grow(0.0)),
            Keyframe::default().grow_width(),
        ]);
    let right = NodeSpec::column().fill();
    ui.leaf_keyed("left", left);
    ui.leaf_keyed("right", right);
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
            .width(100.0)
            .grow_height()
            .bg(bg)
            .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
            .keyframes(vec![Keyframe::default().width(200.0)]);
        ui.leaf_keyed("k", spec);
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

/// F64: `animating()` is one bool, and under a keyframe cycle it never
/// clears — right for the driver, useless for a test that wants to know
/// whether the *transitions* have run out. `owed()` is the same reading
/// by kind: the cycle on its own bit, a transition on its own, and
/// `beyond_cycles()` the wait's predicate.
#[test]
fn what_is_owed_is_readable_by_kind() {
    let mut core = Core::new();
    let frame = |core: &mut Core, now: f64, bg: Color| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        let spec = NodeSpec::column()
            .width(100.0)
            .grow_height()
            .bg(bg)
            .transition_with(
                Transition::ms(1000.0)
                    .easing(Easing::Linear)
                    .repeat(Repeat::Alternate),
            )
            .keyframes(vec![Keyframe::default().width(200.0)]);
        ui.leaf_keyed("k", spec);
        ui.finish();
        core.owed()
    };
    let o = frame(&mut core, 0.0, Color::BLACK);
    assert!(
        o.cycle && !o.transition,
        "{o:?}: the cycle, and nothing else"
    );
    assert!(core.animating() && !o.beyond_cycles());
    // A retarget under the cycle: a transition is owed beside it.
    let o = frame(&mut core, 0.5, Color::WHITE);
    assert!(o.cycle && o.transition, "{o:?}");
    assert!(o.beyond_cycles());
    // A second later the leg is done and only the cycle is left — what
    // `quiet()` resolves on where `settled()` never would.
    let o = frame(&mut core, 2.0, Color::WHITE);
    assert!(o.cycle && !o.transition, "{o:?}");
    assert!(!o.beyond_cycles() && core.animating());
    // The other bits: a requested frame.
    core.set_time(3.0);
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    ui.request_frame();
    ui.finish();
    let o = core.owed();
    assert_eq!(
        o,
        kui_core::Owed {
            requested: true,
            ..Default::default()
        }
    );
    assert!(o.beyond_cycles());
}

/// A viewport float at `dx`; returns its drawn x (its background is the
/// first quad).
fn float_x(core: &mut Core, dx: f32, spec: NodeSpec) -> f32 {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let spec = spec
        .size(50.0, 20.0)
        .bg(Color::WHITE)
        .float(FloatConfig::viewport().offset(dx, 0.0));
    ui.leaf_keyed("toast", spec);
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
            .size(50.0, 20.0)
            .bg(Color::WHITE)
            .transition_with(linear(100.0))
            .enter(enter);
        ui.leaf_keyed("toast", spec);
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

/// The row's width at `now` under a keyframed grow 0 → 1 over a second,
/// with `t` as the transition (its easing and bounce).
fn keyframed_width(t: Transition, now: f64) -> f32 {
    let mut core = Core::new();
    core.set_time(0.0);
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        ui.leaf_keyed(
            "left",
            NodeSpec::column()
                .width(Sizing::Grow(0.0))
                .grow_height()
                .transition_with(t)
                .keyframes(vec![
                    Keyframe::default().width(Sizing::Grow(0.0)),
                    Keyframe::default().grow_width(),
                ]),
        );
        ui.leaf_keyed("right", NodeSpec::column().fill());
        ui.finish();
    };
    frame(&mut core);
    core.set_time(now);
    frame(&mut core);
    left_width(&mut core)
}

/// A spring between keyframe stops is drawn as `easeOut` and its `bounce`
/// does nothing (backlog F141): a cycle is sampled off the clock, and a
/// spring is integrated, so there is no spring to sample. What the
/// `keyframes` and `easing` rows now say.
#[test]
fn a_spring_in_a_cycle_is_ease_out_and_bounce_does_nothing() {
    for now in [0.2, 0.5, 0.8] {
        let ease_out = keyframed_width(Transition::ms(1000.0).easing(Easing::EaseOut), now);
        for t in [
            Transition::ms(1000.0).easing(Easing::Spring),
            Transition::ms(1000.0).easing(Easing::Bouncy),
            Transition::ms(1000.0).easing(Easing::Spring).bounce(0.8),
            Transition::ms(1000.0).easing(Easing::EaseOut).bounce(0.5),
        ] {
            assert_eq!(keyframed_width(t, now), ease_out, "{t:?} at {now}");
        }
    }
}
