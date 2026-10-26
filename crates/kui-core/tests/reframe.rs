//! A frame only an animation of the core's asks for, built from the last
//! tree rather than from the view (backlog C138): `Core::reframe`.

use kui_core::{Color, Core, FrameCause, Keyframe, NodeSpec, Size, Transition};

fn view() -> Size {
    Size::new(200.0, 100.0)
}

/// A pulsing box and a plain one; `read_clock` has the view read the
/// frame clock, as a view animating by hand does.
fn build(core: &mut Core, now: f64, read_clock: bool) {
    core.set_time(now);
    let mut ui = core.frame(view(), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    if read_clock {
        let _ = ui.core().now();
    }
    ui.leaf_keyed(
        "pulse",
        NodeSpec::column()
            .size(40.0, 40.0)
            .bg(Color::hex(0x000000ff))
            .transition_with(Transition::ms(1000.0))
            .keyframes(vec![Keyframe::default().at(0.5).bg(Color::hex(0xffffffff))]),
    );
    ui.leaf_keyed(
        "still",
        NodeSpec::column()
            .size(40.0, 40.0)
            .bg(Color::hex(0x00ff00ff)),
    );
    ui.finish();
}

/// The pulse's colour as drawn: the first 40 px quad's red.
fn pulse_red(core: &mut Core) -> f32 {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| q.rect.w == 40.0 && q.rect.y == 0.0)
        .map_or(-1.0, |q| q.color.r)
}

#[test]
fn an_animation_frame_is_built_from_the_last_tree_with_the_cycle_sampled_again() {
    let mut core = Core::new();
    build(&mut core, 0.0, false);
    // Nothing is kept until a driver asks: the first ask is refused.
    assert!(!core.reframe(view(), 1.0));
    // A frame needs a frame owed by the animation before it.
    build(&mut core, 0.05, false);
    let before = pulse_red(&mut core);
    core.set_time(0.5);
    assert!(
        core.reframe(view(), 1.0),
        "only the cycle asks: built again"
    );
    let after = pulse_red(&mut core);
    assert!(
        after > before + 0.3,
        "the cycle sampled at the new clock: {before} → {after}"
    );
    assert!(core.owed().cycle, "and it still owes the next");
    // Again, and again: a frame built again may be built again.
    core.set_time(0.9);
    assert!(core.reframe(view(), 1.0));
    assert!(pulse_red(&mut core) < after, "on its way back");
    // The plain box is still there.
    let (dl, _) = core.output();
    assert!(
        dl.quads
            .iter()
            .any(|q| q.color.g > 0.99 && q.color.r == 0.0)
    );
}

#[test]
fn a_frame_something_else_asks_for_runs_the_view() {
    let mut core = Core::new();
    build(&mut core, 0.0, false);
    assert!(!core.reframe(view(), 1.0));
    build(&mut core, 0.05, false);
    core.set_time(0.1);
    core.note_frame_cause(FrameCause::WAKE);
    assert!(!core.reframe(view(), 1.0), "a wake is the app's");
    // Nor a resize.
    build(&mut core, 0.15, false);
    core.set_time(0.2);
    assert!(!core.reframe(Size::new(300.0, 100.0), 1.0));
}

#[test]
fn a_view_that_read_the_clock_runs_again() {
    let mut core = Core::new();
    build(&mut core, 0.0, true);
    assert!(!core.reframe(view(), 1.0));
    build(&mut core, 0.05, true);
    core.set_time(0.1);
    assert!(!core.reframe(view(), 1.0), "the clock never reads the same");
}

#[test]
fn nothing_animating_keeps_nothing() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(view(), 1.0);
        ui.leaf_keyed("still", NodeSpec::column().size(40.0, 40.0));
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);
    assert!(!core.reframe(view(), 1.0));
}
