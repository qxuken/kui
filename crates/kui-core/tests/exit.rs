//! Exit transitions: a subtree the view stopped declaring is copied out of
//! the frame that still had it and replayed — frozen, on top, inert — until
//! its transition ends. `docs/adr/0005-the-paint-vocabulary.md`.

use kui_core::{
    Color, Core, Easing, Enter, InputEvent, Key, NodeSpec, Size, Sizing, TextStyle, Value, Vec2,
};

const VIEW: Size = Size { w: 200.0, h: 120.0 };

/// The panel, drawn only while `show`, with an exit that slides it right
/// and fades it out over 100ms.
fn frame(core: &mut Core, now: f64, show: bool) {
    core.set_time(now);
    let mut ui = core.frame(VIEW, 1.0);
    if show {
        ui.with_keyed(
            "panel",
            NodeSpec::column()
                .width(Sizing::Fixed(80.0))
                .height(Sizing::Fixed(40.0))
                .bg(Color::WHITE)
                .on_click(Value::str("panel"))
                .focusable()
                .transition(100.0)
                .easing(Easing::Linear)
                .exit(Enter::from(40.0, 0.0).opacity(0.0)),
            |ui| ui.text("bye", TextStyle::new(12.0).color(Color::WHITE)),
        );
    }
    ui.finish();
}

/// Every solid quad of the last frame as (x, alpha).
fn solids(core: &mut Core) -> Vec<(f32, f32)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid)
        .map(|q| (q.rect.x, q.color.a))
        .collect()
}

#[test]
fn a_departing_subtree_is_replayed_and_then_dropped() {
    let mut core = Core::new();
    frame(&mut core, 0.0, true);
    let live = solids(&mut core);
    assert_eq!(live.len(), 1, "the panel");
    assert_eq!(live[0], (0.0, 1.0));
    assert!(
        !core.animating(),
        "nothing is in flight while it is declared"
    );

    // Gone from the view: the ghost draws where it was, on the very frame
    // it left, and owes the driver more frames.
    frame(&mut core, 0.0, false);
    assert_eq!(solids(&mut core), vec![(0.0, 1.0)], "frozen where it left");
    assert!(core.animating());
    assert_eq!(core.depart.node_count(), 2, "the box and its text");

    // Halfway: half the offset, half the fade.
    frame(&mut core, 0.05, false);
    let mid = solids(&mut core);
    assert_eq!(mid.len(), 1);
    assert!((mid[0].0 - 20.0).abs() < 1e-3, "halfway out: {:?}", mid[0]);
    assert!((mid[0].1 - 0.5).abs() < 1e-3, "halfway faded: {:?}", mid[0]);

    // Over: nothing left, and nothing owed.
    frame(&mut core, 0.1, false);
    assert!(solids(&mut core).is_empty());
    assert!(!core.animating());
    assert!(core.depart.is_empty());
    assert_eq!(core.depart.node_count(), 0);
}

/// A ghost's text is the text the node had. The subtree is copied out of
/// the previous frame, and a `TextId` indexes a list rebuilt every frame —
/// so resolving one against the *current* list draws whatever text happens
/// to sit at that index, or nothing at all.
#[test]
fn a_ghost_keeps_its_own_text() {
    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, show: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        // A live text node before the departing one, so the ids of the two
        // frames disagree: "live" is id 0 either way, and the panel's "bye"
        // is id 1 only while the panel is declared.
        ui.text("live", TextStyle::new(12.0).color(Color::WHITE));
        if show {
            ui.with_keyed(
                "panel",
                NodeSpec::column()
                    .width(Sizing::Fixed(80.0))
                    .height(Sizing::Fixed(40.0))
                    .transition(100.0)
                    .easing(Easing::Linear)
                    .exit(Enter::from(0.0, 40.0)),
                |ui| ui.text("bye", TextStyle::new(12.0).color(Color::WHITE)),
            );
        }
        ui.finish();
    };
    let glyphs = |core: &mut Core| {
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .filter(|q| q.kind != kui_core::QuadKind::Solid)
            .count()
    };
    build(&mut core, 0.0, true);
    let both = glyphs(&mut core);
    assert!(both > 4, "'live' and 'bye': {both}");

    build(&mut core, 0.0, false);
    assert_eq!(
        glyphs(&mut core),
        both,
        "the ghost still draws 'bye' beside the live 'live'"
    );
    // And it goes with the ghost.
    build(&mut core, 0.2, false);
    let alone = glyphs(&mut core);
    assert!(alone > 0 && alone < both, "just 'live' now: {alone}");
}

#[test]
fn a_ghost_is_a_picture_not_a_node() {
    let mut core = Core::new();
    frame(&mut core, 0.0, true);
    frame(&mut core, 0.02, false);
    assert!(!solids(&mut core).is_empty(), "it is on screen");

    // Nothing in the access tree but the window: a ghost is not read out.
    let roles: Vec<&str> = core
        .access_tree()
        .nodes
        .iter()
        .map(|n| n.role.name())
        .collect();
    assert_eq!(roles, vec!["window"], "no access row for a departing node");

    // No hit region: a click over it lands on nothing.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    let mut events = core.handle_input(InputEvent::mouse_down(1));
    events.extend(core.handle_input(InputEvent::mouse_up()));
    assert!(events.is_empty(), "a ghost takes no clicks: {events:?}");
    assert!(!core.is_hovered(Key::ROOT.str("panel")));

    // No place in the Tab ring.
    core.focus_next(true);
    assert_eq!(core.focus(), None, "a ghost is not a Tab stop");
}

#[test]
fn a_key_that_comes_back_wins_immediately() {
    let mut core = Core::new();
    frame(&mut core, 0.0, true);
    frame(&mut core, 0.02, false);
    assert_eq!(core.depart.node_count(), 2);
    // Re-declared mid-exit: one panel on screen, not a live one and a
    // ghost of it — the dismissed-and-re-shown toast.
    frame(&mut core, 0.04, true);
    assert!(core.depart.is_empty());
    assert_eq!(solids(&mut core), vec![(0.0, 1.0)]);
}

#[test]
fn without_both_halves_a_removed_node_still_vanishes_at_once() {
    for spec in [
        // No `exit`.
        NodeSpec::column().transition(100.0),
        // No `transition` with a duration to run over.
        NodeSpec::column()
            .exit(Enter::from(40.0, 0.0))
            .transition(0.0),
    ] {
        let mut core = Core::new();
        let build = |core: &mut Core, now: f64, show: bool| {
            core.set_time(now);
            let mut ui = core.frame(VIEW, 1.0);
            if show {
                ui.with_keyed(
                    "panel",
                    spec.clone()
                        .width(Sizing::Fixed(80.0))
                        .height(Sizing::Fixed(40.0))
                        .bg(Color::WHITE),
                    |_| {},
                );
            }
            ui.finish();
        };
        build(&mut core, 0.0, true);
        assert_eq!(solids(&mut core).len(), 1);
        build(&mut core, 0.01, false);
        assert!(solids(&mut core).is_empty(), "gone at once");
        assert!(core.depart.is_empty());
        assert!(!core.animating());
    }
}

#[test]
fn a_driver_without_a_clock_gets_the_disappearance_it_always_had() {
    let mut core = Core::new();
    // No `set_time` anywhere: every transition snaps, and so does this one.
    let build = |core: &mut Core, show: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        if show {
            ui.with_keyed(
                "panel",
                NodeSpec::column()
                    .width(Sizing::Fixed(80.0))
                    .height(Sizing::Fixed(40.0))
                    .bg(Color::WHITE)
                    .transition(100.0)
                    .exit(Enter::from(40.0, 0.0)),
                |_| {},
            );
        }
        ui.finish();
    };
    build(&mut core, true);
    build(&mut core, false);
    assert!(solids(&mut core).is_empty());
    assert!(core.depart.is_empty());
    assert!(!core.animating());
}

/// A departing subtree escapes the clip its ancestors imposed — they may
/// not exist any more — and draws after every live quad.
#[test]
fn a_ghost_escapes_its_ancestors_clip_and_draws_last() {
    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, show: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            "clipper",
            NodeSpec::column()
                .width(Sizing::Fixed(50.0))
                .height(Sizing::Fixed(20.0))
                .clip()
                .bg(Color::rgb8(1, 2, 3)),
            |ui| {
                if show {
                    ui.with_keyed(
                        "inner",
                        NodeSpec::column()
                            .width(Sizing::Fixed(40.0))
                            .height(Sizing::Fixed(10.0))
                            .bg(Color::WHITE)
                            .transition(100.0)
                            .easing(Easing::Linear)
                            .exit(Enter::from(200.0, 0.0)),
                        |_| {},
                    );
                }
            },
        );
        ui.finish();
    };
    build(&mut core, 0.0, true);
    // The frame it leaves freezes it where it was; the next one moves it.
    build(&mut core, 0.0, false);
    build(&mut core, 0.05, false);
    let (dl, _) = core.output();
    let quads: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid)
        .collect();
    assert_eq!(quads.len(), 2, "the clipper, and the ghost after it");
    let ghost = quads[1];
    assert!(
        (ghost.rect.x - 100.0).abs() < 1e-3,
        "halfway to +200: {:?}",
        ghost.rect
    );
    // 100..140 is entirely outside the clipper's 0..50 box: a live child
    // there would have been clipped away to nothing.
    assert!(
        ghost.clip.w > 1e8,
        "unclipped, though its parent clips: {:?}",
        ghost.clip
    );
}

/// The budget: more nodes departing at once than the store holds means the
/// ones past it vanish, as a node without an `exit` does — and the core
/// says so rather than leaving it to look like a bug.
#[test]
fn the_budget_bounds_the_store_and_raises_a_warning() {
    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, rows: usize| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
        for i in 0..rows {
            ui.with_indexed(
                i as u64,
                NodeSpec::column()
                    .width(Sizing::Fixed(10.0))
                    .height(Sizing::Fixed(1.0))
                    .bg(Color::WHITE)
                    .transition(100.0)
                    .exit(Enter::default().opacity(0.0)),
                |_| {},
            );
        }
        ui.finish();
    };
    let rows = kui_core::depart::MAX_NODES + 100;
    build(&mut core, 0.0, rows);
    core.take_warnings();
    build(&mut core, 0.01, 0);
    assert_eq!(
        core.depart.node_count(),
        kui_core::depart::MAX_NODES,
        "the store is bounded"
    );
    let codes: Vec<&str> = core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(codes, vec!["exit-budget"]);
}

/// The view the policy was decided against: a toast stack inside a
/// bottom-right float, every card with an entrance, an exit and a `slide`.
/// Dismissing the whole stack in one frame is the mass-removal case, and at
/// this size every card animates out while the driver keeps drawing.
#[test]
fn a_whole_toast_stack_departs_at_once_and_plays_out() {
    use kui_core::{Align, FloatConfig};

    const CARDS: usize = 6;
    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, count: usize| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.with(
            NodeSpec::column()
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End),
                )
                .gap(10.0),
            |ui| {
                for i in 0..count {
                    ui.with_keyed(
                        &format!("toast-{i}"),
                        NodeSpec::column()
                            .width(Sizing::Fixed(120.0))
                            .height(Sizing::Fixed(30.0))
                            .bg(Color::WHITE)
                            .transition(100.0)
                            .easing(Easing::Linear)
                            .enter(Enter::from(200.0, 0.0))
                            .exit(Enter::from(200.0, 0.0).opacity(0.0))
                            .slide(),
                        |ui| ui.text("saved", TextStyle::new(12.0).color(Color::WHITE)),
                    );
                }
            },
        );
        ui.finish();
    };

    // Settled with the whole stack up.
    build(&mut core, 0.0, CARDS);
    build(&mut core, 1.0, CARDS);
    assert_eq!(solids(&mut core).len(), CARDS);
    assert!(!core.animating());

    // "clear": every card leaves in one frame. Each is a card plus its
    // label, so the store holds two nodes per toast — nowhere near the
    // budget, and every one of them animates.
    build(&mut core, 1.0, 0);
    assert_eq!(core.depart.node_count(), CARDS * 2);
    assert_eq!(
        solids(&mut core).len(),
        CARDS,
        "all of them, frozen in place"
    );
    assert!(core.animating(), "and the driver owes frames for them");

    // Halfway out: each moved the same distance, each half faded, and each
    // still where its own row was.
    build(&mut core, 1.05, 0);
    let mid = solids(&mut core);
    assert_eq!(mid.len(), CARDS);
    for (x, a) in &mid {
        assert!((x - 380.0).abs() < 1e-3, "halfway to +200 from 280: {x}");
        assert!((a - 0.5).abs() < 1e-3, "halfway faded: {a}");
    }

    // Over: the frame is empty again and the window can idle.
    build(&mut core, 1.1, 0);
    assert!(solids(&mut core).is_empty());
    assert!(!core.animating());
    assert!(core.depart.is_empty());
}
