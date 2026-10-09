//! The exit named at the removal (backlog F136): `Ui::exit_with` aims the
//! exit a node leaves by in the frame that drops it, over the one it
//! declared, so a card a button throws aside goes the way the button says
//! with no frame drawn first to set it. Named and not used, it is gone when
//! the frame finishes.

use kui_core::{Color, Core, Easing, Enter, Key, NodeSpec, Size, Transition};

const VIEW: Size = Size { w: 600.0, h: 200.0 };

/// Builds a frame at `now`: the card at (20, 20) when `show`, with the
/// exit `named` aimed at it first when given. Returns the drawn quads' x.
fn frame(core: &mut Core, now: f64, show: bool, named: Option<Enter>, declares: bool) -> Vec<f32> {
    core.set_time(now);
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    if let Some(e) = named {
        let key = ui.key_of("card").expect("the card was declared");
        ui.exit_with(key, e);
    }
    if show {
        let mut spec = NodeSpec::column()
            .size(40.0, 40.0)
            .bg(Color::WHITE)
            .transition_with(Transition::ms(1000.0).easing(Easing::Linear));
        if declares {
            spec = spec.exit(Enter::default().opacity(0.0));
        }
        ui.leaf_keyed("card", spec);
    }
    ui.finish();
    kui_core::testing::solids(core)
        .iter()
        .map(|q| q.rect.x)
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.6
}

#[test]
fn the_named_exit_wins_over_the_declared_one() {
    let mut core = Core::new();
    frame(&mut core, 0.0, true, None, true);
    let throw = Enter::from(400.0, 0.0).opacity(0.0);
    frame(&mut core, 0.0, false, Some(throw), true);
    let mid = frame(&mut core, 0.5, false, None, true);
    assert_eq!(mid.len(), 1, "the ghost is drawn: {mid:?}");
    assert!(near(mid[0], 220.0), "halfway along the throw: {mid:?}");
    assert!(
        frame(&mut core, 1.2, false, None, true).is_empty(),
        "and gone"
    );
}

#[test]
fn a_named_exit_the_frame_did_not_use_is_forgotten() {
    let mut core = Core::new();
    frame(&mut core, 0.0, true, None, true);
    // Named while the card stays: nothing leaves, and the name lapses.
    frame(&mut core, 0.0, true, Some(Enter::from(400.0, 0.0)), true);
    frame(&mut core, 0.0, false, None, true);
    let mid = frame(&mut core, 0.5, false, None, true);
    assert_eq!(mid.len(), 1, "{mid:?}");
    assert!(near(mid[0], 20.0), "it fades where it was: {mid:?}");
}

#[test]
fn a_node_without_a_declared_exit_is_not_aimed() {
    let mut core = Core::new();
    frame(&mut core, 0.0, true, None, false);
    frame(&mut core, 0.0, false, Some(Enter::from(400.0, 0.0)), false);
    assert!(
        frame(&mut core, 0.5, false, None, false).is_empty(),
        "it leaves at once, as it would have"
    );
}

#[test]
fn a_key_nothing_declares_is_harmless() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.exit_with(Key(42), Enter::from(1.0, 1.0));
    ui.finish();
    assert!(!core.animating());
}
