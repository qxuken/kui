//! Exit transitions: a subtree the view stopped declaring is copied out of
//! the frame that still had it and replayed — frozen, in its place, inert —
//! until its transition ends. `docs/adr/0005-the-paint-vocabulary.md`.

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
                ui.leaf_keyed(
                    "panel",
                    spec.clone()
                        .width(Sizing::Fixed(80.0))
                        .height(Sizing::Fixed(40.0))
                        .bg(Color::WHITE),
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
            ui.leaf_keyed(
                "panel",
                NodeSpec::column()
                    .width(Sizing::Fixed(80.0))
                    .height(Sizing::Fixed(40.0))
                    .bg(Color::WHITE)
                    .transition(100.0)
                    .exit(Enter::from(40.0, 0.0)),
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
/// not exist any more — while keeping its place after the clipper, which
/// painted before it.
#[test]
fn a_ghost_escapes_its_ancestors_clip_and_keeps_its_place() {
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
                    ui.leaf_keyed(
                        "inner",
                        NodeSpec::column()
                            .width(Sizing::Fixed(40.0))
                            .height(Sizing::Fixed(10.0))
                            .bg(Color::WHITE)
                            .transition(100.0)
                            .easing(Easing::Linear)
                            .exit(Enter::from(200.0, 0.0)),
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
    let ghost_clip = dl.clip_of(ghost);
    assert!(
        (ghost.rect.x - 100.0).abs() < 1e-3,
        "halfway to +200: {:?}",
        ghost.rect
    );
    // 100..140 is entirely outside the clipper's 0..50 box: a live child
    // there would have been clipped away to nothing.
    assert!(
        ghost_clip.rect.w > 1e8,
        "unclipped, though its parent clips: {:?}",
        ghost_clip.rect
    );
}

/// The clips *inside* the picture stay: a departing scroll box still
/// bounds what it held. The list example's `uniform_list` builds two
/// rows of overscan past its edge, and they showed the frame the list
/// left.
#[test]
fn a_ghost_keeps_the_clips_its_own_subtree_established() {
    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, show: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        if show {
            ui.with_keyed(
                "box",
                NodeSpec::column()
                    .width(Sizing::Fixed(50.0))
                    .height(Sizing::Fixed(20.0))
                    .clip()
                    .bg(Color::rgb8(1, 2, 3))
                    .transition(100.0)
                    .easing(Easing::Linear)
                    .exit(Enter::from(200.0, 0.0)),
                |ui| {
                    // Taller than the box: its bottom third is clipped.
                    ui.leaf(
                        NodeSpec::column()
                            .width(Sizing::Fixed(40.0))
                            .height(Sizing::Fixed(30.0))
                            .bg(Color::WHITE),
                    );
                    // Entirely past the box's edge: never painted.
                    ui.leaf(
                        NodeSpec::column()
                            .width(Sizing::Fixed(40.0))
                            .height(Sizing::Fixed(10.0))
                            .bg(Color::WHITE),
                    );
                },
            );
        }
        ui.finish();
    };
    build(&mut core, 0.0, true);
    build(&mut core, 0.0, false);
    build(&mut core, 0.05, false);
    let (dl, _) = core.output();
    let quads: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid)
        .collect();
    assert_eq!(
        quads.len(),
        2,
        "the box and the child it shows; the one past its edge is culled: {:?}",
        quads.iter().map(|q| q.rect).collect::<Vec<_>>()
    );
    let (bx, child) = (quads[0], quads[1]);
    let (bx_clip, child_clip) = (dl.clip_of(bx), dl.clip_of(child));
    assert!(
        bx_clip.rect.w > 1e8,
        "the box itself is unclipped: {:?}",
        bx_clip.rect
    );
    assert!((bx.rect.x - 100.0).abs() < 1e-3, "halfway: {:?}", bx.rect);
    assert!(
        (child_clip.rect.x - 100.0).abs() < 1e-3
            && (child_clip.rect.w - 50.0).abs() < 1e-3
            && (child_clip.rect.h - 20.0).abs() < 1e-3,
        "the child is clipped to the box where it now is: {:?}",
        child_clip.rect
    );
}

/// `rows` one-node rows, keyed `ROOT[from + i]`, every one declaring an
/// `exit` — the list the budget is for.
fn list(core: &mut Core, now: f64, from: usize, rows: usize) {
    core.set_time(now);
    let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
    for i in from..from + rows {
        ui.leaf_indexed(
            i as u64,
            NodeSpec::column()
                .width(Sizing::Fixed(10.0))
                .height(Sizing::Fixed(1.0))
                .bg(Color::WHITE)
                .transition(100.0)
                .exit(Enter::default().opacity(0.0)),
        );
    }
    ui.finish();
}

/// The budget, ADR 0012 decision 2: a frame that removes more nodes than
/// the store holds gets *none* of them animated — every row vanishes at
/// once, as a node without an `exit` does — rather than the first 512
/// sliding out and the rest blinking. And the core says so, naming the
/// frame's count, rather than leaving it to look like a bug.
#[test]
fn a_removal_over_the_budget_animates_nothing_and_says_so() {
    let mut core = Core::new();
    let rows = kui_core::depart::MAX_NODES + 100;
    list(&mut core, 0.0, 0, rows);
    core.take_warnings();
    list(&mut core, 0.01, 0, 0);
    assert!(core.depart.is_empty(), "whole or not at all");
    assert_eq!(core.depart.node_count(), 0);
    let warnings = core.take_warnings();
    assert_eq!(
        warnings.len(),
        1,
        "one warning for the frame, not one per row"
    );
    assert_eq!(warnings[0].code, "exit-budget");
    assert!(
        warnings[0]
            .message
            .contains(&format!("removed {rows} nodes")),
        "the sentence names the frame's removal: {}",
        warnings[0].message
    );
    assert!(!core.animating(), "and nothing is in flight");
}

/// ADR 0012 decision 3: a removal that fits the budget but not the room
/// beside exits already in flight takes that room from the oldest ghosts,
/// and is not a warning. Before this, the store protected the oldest and
/// refused the newest, so an unrelated dismissal 100 ms behind a big one
/// got twelve rows of two hundred.
#[test]
fn a_new_removal_outranks_the_ghosts_already_in_flight() {
    let mut core = Core::new();
    // Two lists side by side: 300 rows keyed 0.., 300 keyed 1000...
    let both = |core: &mut Core, now: f64, first: usize, second: usize| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 400.0), 1.0);
        for (from, rows) in [(0, first), (1000, second)] {
            for i in from..from + rows {
                ui.leaf_indexed(
                    i as u64,
                    NodeSpec::column()
                        .width(Sizing::Fixed(10.0))
                        .height(Sizing::Fixed(1.0))
                        .bg(Color::WHITE)
                        .transition(100.0)
                        .exit(Enter::default().opacity(0.0)),
                );
            }
        }
        ui.finish();
    };
    both(&mut core, 0.0, 300, 300);
    core.take_warnings();
    // The first list goes: 300 ghosts, in flight.
    both(&mut core, 0.01, 0, 300);
    assert_eq!(core.depart.node_count(), 300);
    // 20 ms later the second goes too: 300 + 300 is over the budget, so
    // the oldest 88 of the first list's ghosts give way and every row of
    // the second animates.
    both(&mut core, 0.03, 0, 0);
    assert_eq!(
        core.depart.node_count(),
        kui_core::depart::MAX_NODES,
        "full, with the new removal whole"
    );
    let keys: Vec<Key> = core.depart.keys().collect();
    assert_eq!(keys.len(), kui_core::depart::MAX_NODES);
    assert_eq!(keys[0], Key::ROOT.index(88), "the oldest 88 went, in order");
    assert_eq!(
        keys[212],
        Key::ROOT.index(1000),
        "then the whole second list"
    );
    assert_eq!(keys[511], Key::ROOT.index(1299));
    assert!(
        core.take_warnings().is_empty(),
        "eviction is the policy, not a warning"
    );
    assert!(core.animating());
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

/// Solid quads of the last frame named by the colour each node was given,
/// in paint order — the order the layering claims are about.
fn painted(core: &mut Core, names: &[(Color, &'static str)]) -> Vec<&'static str> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid)
        .map(|q| {
            names
                .iter()
                .find(|(c, _)| {
                    (c.r - q.color.r).abs() < 1e-3
                        && (c.g - q.color.g).abs() < 1e-3
                        && (c.b - q.color.b).abs() < 1e-3
                })
                .map(|(_, n)| *n)
                .unwrap_or("?")
        })
        .collect()
}

const PANEL: Color = Color {
    r: 10.0 / 255.0,
    g: 0.0 / 255.0,
    b: 0.0 / 255.0,
    a: 1.0,
};
const HUD: Color = Color {
    r: 0.0 / 255.0,
    g: 10.0 / 255.0,
    b: 0.0 / 255.0,
    a: 1.0,
};
const A: Color = Color {
    r: 0.0 / 255.0,
    g: 0.0 / 255.0,
    b: 10.0 / 255.0,
    a: 1.0,
};
const B: Color = Color {
    r: 20.0 / 255.0,
    g: 0.0 / 255.0,
    b: 0.0 / 255.0,
    a: 1.0,
};
const C: Color = Color {
    r: 0.0 / 255.0,
    g: 20.0 / 255.0,
    b: 0.0 / 255.0,
    a: 1.0,
};
const NAMES: &[(Color, &str)] = &[(PANEL, "panel"), (HUD, "hud"), (A, "a"), (B, "b"), (C, "c")];

fn leaving(spec: NodeSpec) -> NodeSpec {
    spec.transition(100.0)
        .easing(Easing::Linear)
        .exit(Enter::from(40.0, 0.0))
}

/// The toasts example's panel: a side panel float and a HUD float opened
/// in the same frame, panel first in the tree, so the HUD is painted over
/// it (ADR 0023: same frame, tree order). Its ghost stays under the HUD
/// for the exit, rather than jumping to the top of the window for its
/// last few frames.
#[test]
fn a_float_leaves_under_the_floats_that_were_above_it() {
    use kui_core::{Align, FloatConfig};

    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, panel: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        if panel {
            ui.leaf_keyed(
                "panel",
                leaving(
                    NodeSpec::column()
                        .float(FloatConfig::viewport())
                        .width(Sizing::Fixed(60.0))
                        .height(Sizing::Fixed(120.0))
                        .bg(PANEL),
                ),
            );
        }
        // Unkeyed on purpose: a keyed sibling does not consume a sibling
        // index, so the HUD keeps its key with the panel gone.
        ui.leaf(
            NodeSpec::column()
                .float(FloatConfig::viewport().at(Align::Start, Align::End))
                .width(Sizing::Fixed(80.0))
                .height(Sizing::Fixed(20.0))
                .bg(HUD),
        );
        ui.finish();
    };
    build(&mut core, 0.0, true);
    assert_eq!(painted(&mut core, NAMES), ["panel", "hud"]);
    build(&mut core, 0.0, false);
    assert_eq!(
        painted(&mut core, NAMES),
        ["panel", "hud"],
        "the ghost keeps the panel's place under the HUD"
    );
    build(&mut core, 0.05, false);
    assert_eq!(
        painted(&mut core, NAMES),
        ["panel", "hud"],
        "and stays there"
    );
    let (dl, _) = core.output();
    let ghost = dl.quads.iter().find(|q| q.color == PANEL).unwrap();
    assert!(
        (ghost.rect.x - 20.0).abs() < 1e-3,
        "and is mid-exit: {:?}",
        ghost.rect
    );
}

/// An in-flow node's ghost stays between the siblings it had, and under
/// every float — the same pass it painted in, at the same place.
#[test]
fn an_in_flow_ghost_keeps_its_place_between_its_siblings() {
    use kui_core::FloatConfig;

    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, b: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        let cell = |bg| {
            NodeSpec::column()
                .width(Sizing::Fixed(30.0))
                .height(Sizing::Fixed(10.0))
                .bg(bg)
        };
        ui.leaf_keyed("a", cell(A));
        if b {
            ui.leaf_keyed("b", leaving(cell(B)));
        }
        ui.leaf_keyed("c", cell(C));
        ui.leaf_keyed("hud", cell(HUD).float(FloatConfig::viewport()));
        ui.finish();
    };
    build(&mut core, 0.0, true);
    assert_eq!(painted(&mut core, NAMES), ["a", "b", "c", "hud"]);
    build(&mut core, 0.05, false);
    assert_eq!(
        painted(&mut core, NAMES),
        ["a", "b", "c", "hud"],
        "b's ghost is painted under c, and under the float"
    );
}

/// The node a ghost was painted under can leave too. The ghost takes the
/// place that node's ghost takes, behind it, so two neighbours that go one
/// after the other keep the order they had.
#[test]
fn a_ghost_whose_neighbour_leaves_too_keeps_the_order_they_had() {
    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, a: bool, b: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        let cell = |bg| {
            leaving(
                NodeSpec::column()
                    .width(Sizing::Fixed(30.0))
                    .height(Sizing::Fixed(10.0))
                    .bg(bg),
            )
        };
        if a {
            ui.leaf_keyed("a", cell(A));
        }
        if b {
            ui.leaf_keyed("b", cell(B));
        }
        ui.leaf_keyed("c", cell(C));
        ui.finish();
    };
    build(&mut core, 0.0, true, true);
    build(&mut core, 0.02, false, true);
    assert_eq!(
        painted(&mut core, NAMES),
        ["a", "b", "c"],
        "a's ghost under b"
    );
    build(&mut core, 0.04, false, false);
    assert_eq!(
        painted(&mut core, NAMES),
        ["a", "b", "c"],
        "b left too: a's ghost is under b's, and both under c"
    );
    build(&mut core, 0.06, false, false);
    assert_eq!(painted(&mut core, NAMES), ["a", "b", "c"]);
}

/// A ghost whose place is gone — the node it was painted under is not
/// declared any more and has no ghost of its own — ends its pass: after
/// the in-flow content it was among, and still under the floats.
#[test]
fn a_ghost_that_lost_its_place_ends_its_pass_under_the_floats() {
    use kui_core::FloatConfig;

    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, phase: u32| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        let cell = |bg| {
            NodeSpec::column()
                .width(Sizing::Fixed(30.0))
                .height(Sizing::Fixed(10.0))
                .bg(bg)
        };
        ui.leaf_keyed("a", cell(A));
        if phase < 1 {
            ui.leaf_keyed("b", leaving(cell(B)));
        }
        // `c` has no exit: when it goes, it goes at once, and b's ghost
        // has nothing to be under.
        if phase < 2 {
            ui.leaf_keyed("c", cell(C));
        }
        ui.leaf_keyed("hud", cell(HUD).float(FloatConfig::viewport()));
        ui.finish();
    };
    build(&mut core, 0.0, 0);
    build(&mut core, 0.02, 1);
    assert_eq!(painted(&mut core, NAMES), ["a", "b", "c", "hud"]);
    build(&mut core, 0.04, 2);
    assert_eq!(
        painted(&mut core, NAMES),
        ["a", "b", "hud"],
        "after the in-flow content, under the float"
    );
}

/// The stack, not the tree, says what a departed float was under: a
/// panel opened *after* the HUD was over it, so its ghost leaves over it
/// too — even though the panel comes first in the tree.
#[test]
fn a_float_that_opened_over_another_leaves_over_it() {
    use kui_core::{Align, FloatConfig};

    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, panel: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        if panel {
            ui.leaf_keyed(
                "panel",
                leaving(
                    NodeSpec::column()
                        .float(FloatConfig::viewport())
                        .width(Sizing::Fixed(60.0))
                        .height(Sizing::Fixed(120.0))
                        .bg(PANEL),
                ),
            );
        }
        ui.leaf(
            NodeSpec::column()
                .float(FloatConfig::viewport().at(Align::Start, Align::End))
                .width(Sizing::Fixed(80.0))
                .height(Sizing::Fixed(20.0))
                .bg(HUD),
        );
        ui.finish();
    };
    build(&mut core, 0.0, false);
    build(&mut core, 0.0, true);
    assert_eq!(painted(&mut core, NAMES), ["hud", "panel"], "opened later");
    build(&mut core, 0.0, false);
    assert_eq!(
        painted(&mut core, NAMES),
        ["hud", "panel"],
        "the ghost keeps the panel's place over the HUD"
    );
    build(&mut core, 0.05, false);
    assert_eq!(painted(&mut core, NAMES), ["hud", "panel"]);
    build(&mut core, 0.1, false);
    assert_eq!(painted(&mut core, NAMES), ["hud"]);
}

/// A ghost inside a float keeps its place in that float's layer: between
/// the rows it had, under the float's own chrome, and under a float over
/// it — not at the end of the frame.
#[test]
fn a_ghost_inside_a_float_stays_in_that_layer() {
    use kui_core::FloatConfig;

    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, b: bool| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        let cell = |bg| {
            NodeSpec::column()
                .width(Sizing::Fixed(30.0))
                .height(Sizing::Fixed(10.0))
                .bg(bg)
        };
        ui.with_keyed(
            "panel",
            NodeSpec::column()
                .float(FloatConfig::viewport())
                .width(Sizing::Fixed(60.0))
                .height(Sizing::Fixed(60.0))
                .bg(PANEL),
            |ui| {
                ui.leaf_keyed("a", cell(A));
                if b {
                    ui.leaf_keyed("b", leaving(cell(B)));
                }
                ui.leaf_keyed("c", cell(C));
            },
        );
        ui.leaf_keyed("hud", cell(HUD).float(FloatConfig::viewport()));
        ui.finish();
    };
    build(&mut core, 0.0, true);
    assert_eq!(painted(&mut core, NAMES), ["panel", "a", "b", "c", "hud"]);
    build(&mut core, 0.02, false);
    assert_eq!(
        painted(&mut core, NAMES),
        ["panel", "a", "b", "c", "hud"],
        "between its siblings, inside the panel's layer"
    );
}
