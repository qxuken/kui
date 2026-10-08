//! A keyframe cycle runs so many times (backlog F133): `iterations` is
//! CSS's `animation-iteration-count`. A finite cycle plays from the first
//! frame its node is declared with it, holds its first stop through its
//! `delay`, rests where its last iteration ended, and once it is over owes
//! no frame. An infinite one reads the clock as it always did.

use kui_core::{Color, Core, Easing, Keyframe, NodeSpec, Repeat, Size, Transition};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

/// One 100-px-wide box whose width cycles 100 → 200 over a linear second,
/// drawn at `now`; returns its quad's width, or None when it is not drawn.
fn width(core: &mut Core, now: f64, repeat: Repeat, iterations: Option<f32>, delay: f32) -> f32 {
    core.set_time(now);
    let mut ui = core.frame(VIEW, 1.0);
    let mut spec = NodeSpec::column()
        .size(100.0, 20.0)
        .bg(Color::WHITE)
        .transition_with(
            Transition::ms(1000.0)
                .easing(Easing::Linear)
                .repeat(repeat)
                .delay(delay),
        )
        .keyframes(vec![
            Keyframe::default().width(100.0),
            Keyframe::default().width(200.0),
        ]);
    if let Some(n) = iterations {
        spec = spec.iterations(n);
    }
    ui.leaf_keyed("box", spec);
    ui.finish();
    kui_core::testing::solids(core)[0].rect.w
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.6
}

#[test]
fn one_iteration_plays_once_and_rests_at_the_last_stop() {
    let mut core = Core::new();
    // The node first appears at t = 10 s: its one cycle starts there, not
    // at the clock's origin, which an infinite cycle reads.
    assert!(near(
        width(&mut core, 10.0, Repeat::Normal, Some(1.0), 0.0),
        100.0
    ));
    assert!(core.owed().cycle, "it owes frames while it plays");
    assert!(near(
        width(&mut core, 10.5, Repeat::Normal, Some(1.0), 0.0),
        150.0
    ));
    let end = width(&mut core, 11.2, Repeat::Normal, Some(1.0), 0.0);
    assert!(near(end, 200.0), "rests at the last stop: {end}");
    assert!(!core.owed().cycle, "and owes nothing once it is over");
    assert!(!core.animating());
    assert!(near(
        width(&mut core, 15.0, Repeat::Normal, Some(1.0), 0.0),
        200.0
    ));
}

#[test]
fn an_infinite_cycle_still_reads_the_clock() {
    let mut core = Core::new();
    assert!(near(
        width(&mut core, 10.25, Repeat::Normal, None, 0.0),
        125.0
    ));
    assert!(near(
        width(&mut core, 11.25, Repeat::Normal, None, 0.0),
        125.0
    ));
    assert!(core.owed().cycle, "for ever");
}

#[test]
fn two_alternating_iterations_end_where_they_began() {
    let mut core = Core::new();
    width(&mut core, 0.0, Repeat::Alternate, Some(2.0), 0.0);
    assert!(
        near(
            width(&mut core, 1.5, Repeat::Alternate, Some(2.0), 0.0),
            150.0
        ),
        "on the way back"
    );
    assert!(near(
        width(&mut core, 2.5, Repeat::Alternate, Some(2.0), 0.0),
        100.0
    ));
    assert!(!core.owed().cycle);
}

#[test]
fn a_fraction_stops_partway_and_reverse_ends_at_the_first_stop() {
    let mut core = Core::new();
    width(&mut core, 0.0, Repeat::Normal, Some(0.5), 0.0);
    assert!(
        near(width(&mut core, 3.0, Repeat::Normal, Some(0.5), 0.0), 150.0),
        "half a cycle"
    );
    let mut core = Core::new();
    width(&mut core, 0.0, Repeat::Reverse, Some(1.0), 0.0);
    assert!(
        near(
            width(&mut core, 3.0, Repeat::Reverse, Some(1.0), 0.0),
            100.0
        ),
        "reverse ends at the first stop"
    );
}

#[test]
fn a_delay_holds_the_first_stop_and_then_plays() {
    let mut core = Core::new();
    // `reverse` starts at the last stop, so holding the first stop of the
    // first iteration is 200, not the node's own 100.
    assert!(near(
        width(&mut core, 0.0, Repeat::Reverse, Some(1.0), 500.0),
        200.0
    ));
    assert!(
        near(
            width(&mut core, 0.4, Repeat::Reverse, Some(1.0), 500.0),
            200.0
        ),
        "still waiting"
    );
    assert!(core.owed().cycle, "a waiting cycle owes its frames");
    assert!(near(
        width(&mut core, 1.0, Repeat::Reverse, Some(1.0), 500.0),
        150.0
    ));
    assert!(near(
        width(&mut core, 1.6, Repeat::Reverse, Some(1.0), 500.0),
        100.0
    ));
    assert!(!core.owed().cycle);
}

#[test]
fn a_node_that_leaves_and_comes_back_plays_again() {
    let mut core = Core::new();
    width(&mut core, 0.0, Repeat::Normal, Some(1.0), 0.0);
    assert!(near(
        width(&mut core, 2.0, Repeat::Normal, Some(1.0), 0.0),
        200.0
    ));
    // A frame without it, then it is back: the cycle starts again.
    core.set_time(2.5);
    let ui = core.frame(VIEW, 1.0);
    ui.finish();
    assert!(
        near(width(&mut core, 3.0, Repeat::Normal, Some(1.0), 0.0), 100.0),
        "from the top"
    );
    assert!(near(
        width(&mut core, 3.5, Repeat::Normal, Some(1.0), 0.0),
        150.0
    ));
}

#[test]
fn dropping_the_stops_for_a_frame_replays_them() {
    let mut core = Core::new();
    width(&mut core, 0.0, Repeat::Normal, Some(1.0), 0.0);
    width(&mut core, 2.0, Repeat::Normal, Some(1.0), 0.0);
    // The node stays; its stops are left out for one frame.
    core.set_time(2.5);
    let mut ui = core.frame(VIEW, 1.0);
    ui.leaf_keyed(
        "box",
        NodeSpec::column()
            .size(100.0, 20.0)
            .bg(Color::WHITE)
            .transition(1000.0),
    );
    ui.finish();
    assert!(
        near(width(&mut core, 3.0, Repeat::Normal, Some(1.0), 0.0), 100.0),
        "replayed"
    );
}

#[test]
fn a_finished_cycle_is_not_named_among_the_frames_owed() {
    let mut core = Core::new();
    core.set_frame_trace(true);
    // Each call builds a frame at `now` and returns who the frame before
    // it left a frame owed to, as the frame being built finds them.
    let frame = |core: &mut Core, now: f64| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        let by: Vec<String> = ui
            .core()
            .owed_by()
            .cycles
            .iter()
            .map(|h| h.name.clone())
            .collect();
        for (key, n) in [("once", Some(1.0)), ("always", None)] {
            let mut spec = NodeSpec::column()
                .size(10.0, 10.0)
                .transition(1000.0)
                .keyframes(vec![
                    Keyframe::default().opacity(0.0),
                    Keyframe::default().opacity(1.0),
                ]);
            if let Some(n) = n {
                spec = spec.iterations(n);
            }
            ui.leaf_keyed(key, spec);
        }
        ui.finish();
        by
    };
    frame(&mut core, 0.0);
    assert_eq!(frame(&mut core, 0.1), vec!["once", "always"], "both play");
    frame(&mut core, 2.0);
    assert_eq!(
        frame(&mut core, 2.1),
        vec!["always"],
        "the one-shot is over"
    );
}

#[test]
fn a_one_shot_turn_and_a_one_shot_bob_rest_at_their_ends() {
    let mut core = Core::new();
    let frame = |core: &mut Core, now: f64| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed(
            "pop",
            NodeSpec::column()
                .size(40.0, 40.0)
                .bg(Color::WHITE)
                .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
                .keyframes(vec![
                    Keyframe::default().rotate(0.0).dy(0.0),
                    Keyframe::default().rotate(0.25).dy(-10.0),
                ])
                .iterations(1.0),
        );
        ui.finish();
        let (dl, _) = core.output();
        let q = dl.quads[0];
        (q.rect.y, dl.clip_of(&q).transform.angle)
    };
    frame(&mut core, 0.0);
    let (y, angle) = frame(&mut core, 5.0);
    assert!(near(y, 10.0), "risen 10 px and resting: {y}");
    assert!(
        (angle - std::f32::consts::TAU / 4.0).abs() < 1e-3,
        "turned a quarter and resting: {angle}"
    );
    assert!(!core.owed().cycle);
}
