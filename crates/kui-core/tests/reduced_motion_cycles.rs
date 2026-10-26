//! Reduced motion over a keyframe cycle (backlog C107): a cycle that runs
//! for ever holds still, the node showing its own props, and a finite one
//! rests where it ends, neither owing a frame — unless the node says its
//! motion is essential (`essential_motion`), when it runs as ever. Its
//! position stops (backlog F132) and its turn (ADR 0043) hold the same way.

use kui_core::{Color, Core, Easing, Keyframe, MotionPref, NodeSpec, Size, Transition, testing};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

/// One box whose width cycles 100 → 200 over a linear second, dropping
/// 10 px as it goes, drawn at `now`; returns its quad's width and top.
fn frame(core: &mut Core, now: f64, iterations: Option<f32>, essential: bool) -> (f32, f32) {
    core.set_time(now);
    let mut ui = core.frame(VIEW, 1.0);
    let mut spec = NodeSpec::column()
        .size(100.0, 20.0)
        .bg(Color::WHITE)
        .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
        .keyframes(vec![
            Keyframe::default().width(100.0),
            Keyframe::default().width(200.0).dy(10.0),
        ]);
    if let Some(n) = iterations {
        spec = spec.iterations(n);
    }
    if essential {
        spec = spec.essential_motion();
    }
    ui.leaf_keyed("box", spec);
    ui.finish();
    let r = testing::solids(core)[0].rect;
    (r.w, r.y)
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.6
}

fn reduced() -> Core {
    let mut core = Core::new();
    core.env.system.motion = MotionPref::Reduced;
    core
}

#[test]
fn a_cycle_for_ever_holds_still_under_reduced_motion() {
    let mut core = reduced();
    for now in [10.25, 10.5, 11.75] {
        let (w, y) = frame(&mut core, now, None, false);
        assert!(near(w, 100.0), "its own width at {now}: {w}");
        assert!(near(y, 0.0), "in its own place at {now}: {y}");
    }
    assert!(!core.owed().cycle, "and owes no frame");
    assert!(!core.animating());
}

#[test]
fn a_finite_cycle_rests_where_it_ends_under_reduced_motion() {
    let mut core = reduced();
    let (w, y) = frame(&mut core, 10.0, Some(1.0), false);
    assert!(near(w, 200.0), "at its end from the first frame: {w}");
    assert!(near(y, 10.0), "its last stop's place: {y}");
    assert!(!core.owed().cycle);
}

#[test]
fn essential_motion_keeps_running_under_reduced_motion() {
    let mut core = reduced();
    let (w, _) = frame(&mut core, 10.25, None, true);
    assert!(near(w, 125.0), "{w}");
    let (w, _) = frame(&mut core, 10.5, None, true);
    assert!(near(w, 150.0), "{w}");
    assert!(core.owed().cycle, "for ever");
}

#[test]
fn a_cycle_runs_as_ever_without_reduced_motion() {
    let mut core = Core::new();
    let (w, _) = frame(&mut core, 10.25, None, false);
    assert!(near(w, 125.0), "{w}");
    assert!(core.owed().cycle);
}

#[test]
fn a_spin_holds_upright_under_reduced_motion() {
    let spin = |core: &mut Core, now: f64| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        ui.leaf_keyed(
            "spin",
            NodeSpec::column()
                .size(40.0, 40.0)
                .bg(Color::WHITE)
                .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
                .keyframes(vec![
                    Keyframe::default().rotate(0.0),
                    Keyframe::default().rotate(1.0),
                ]),
        );
        ui.finish();
    };
    let mut core = reduced();
    spin(&mut core, 10.25);
    assert!(!core.owed().cycle, "no turn runs");
    let mut core = Core::new();
    spin(&mut core, 10.25);
    assert!(core.owed().cycle, "it spins with full motion");
}
