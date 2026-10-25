//! Backlog C97: `slide` eases a node's place within its parent, so a move
//! its ancestors make — a window resize that moves a switch's track, the
//! parent's own slide, a card taken to another column — is followed at
//! once or carried on from where it was drawn, and only the node's own
//! move within the parent animates.

use kui_core::{Align, Color, Core, Easing, Edges, Key, NodeSpec, Size, Transition};

fn linear() -> Transition {
    Transition::ms(100.0).easing(Easing::Linear)
}

const TRACK: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};
const KNOB: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const CARD: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};

/// Where the quad painted in `c` is, or panics.
fn at(core: &mut Core, c: Color) -> (f32, f32) {
    let (dl, _) = core.output();
    let q = dl
        .quads
        .iter()
        .find(|q| q.color == c)
        .expect("a quad in that colour");
    (q.rect.x, q.rect.y)
}

/// A switch at the right-hand end of a window `w` wide: its track ends
/// the row, its knob at the track's start or end as `on` says. The track
/// has no `slide`; the knob has, as Kit's and the stock switch's do.
fn switch(core: &mut Core, w: f32, on: bool) -> ((f32, f32), (f32, f32)) {
    let mut ui = core.frame(Size::new(w, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill().main_align(Align::End));
    let track = NodeSpec::row()
        .size(40.0, 20.0)
        .pad(2.0)
        .bg(TRACK)
        .main_align(if on { Align::End } else { Align::Start });
    ui.with_keyed("track", track, |ui| {
        ui.leaf_keyed(
            "knob",
            NodeSpec::row()
                .size(16.0, 16.0)
                .bg(KNOB)
                .transition_with(linear())
                .slide(),
        );
    });
    ui.finish();
    (at(core, TRACK), at(core, KNOB))
}

#[test]
fn a_knob_keeps_to_its_track_while_the_window_resizes() {
    let mut core = Core::new();
    core.set_time(0.0);
    let (track, knob) = switch(&mut core, 400.0, false);
    assert_eq!(track, (360.0, 0.0));
    assert_eq!(knob, (362.0, 2.0));
    // The window narrows by 100 px: the track moves with layout, and the
    // knob, which did not move within it, moves with it on the same frame.
    for (t, w) in [(0.01, 300.0), (0.02, 250.0), (0.03, 420.0)] {
        core.set_time(t);
        let (track, knob) = switch(&mut core, w, false);
        assert_eq!(track.0, w - 40.0);
        assert_eq!(knob.0 - track.0, 2.0, "the knob keeps its place at {w}");
        assert!(!core.animating(), "and owes no frames for it");
    }
}

#[test]
fn a_knob_that_moves_within_its_track_still_eases_through_a_resize() {
    let mut core = Core::new();
    core.set_time(0.0);
    switch(&mut core, 400.0, false);
    // Turned on: the knob eases from the track's start to its end, 20 px.
    core.set_time(0.0);
    let (track, knob) = switch(&mut core, 400.0, true);
    assert_eq!(knob.0 - track.0, 2.0, "starts where it was");
    assert!(core.animating());
    // Halfway, while the window narrows: halfway along the track, wherever
    // the track is now.
    core.set_time(0.05);
    let (track, knob) = switch(&mut core, 300.0, true);
    assert_eq!(track.0, 260.0);
    assert_eq!(knob.0 - track.0, 12.0);
    core.set_time(0.2);
    let (track, knob) = switch(&mut core, 300.0, true);
    assert_eq!(knob.0 - track.0, 22.0);
    assert!(!core.animating());
}

/// Two sliding rows in a column, each holding a sliding knob at `inset`
/// from its start; returns the first-ordered row's and its knob's x and y.
/// `order` puts row "a" first or second.
fn rows(core: &mut Core, a_first: bool, inset: f32) -> ((f32, f32), (f32, f32)) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let labels = if a_first { ["a", "b"] } else { ["b", "a"] };
    for label in labels {
        let bg = if label == "a" { TRACK } else { Color::WHITE };
        let row = NodeSpec::row()
            .size(200.0, 40.0)
            .padding(Edges {
                l: inset,
                ..Edges::default()
            })
            .bg(bg)
            .transition_with(linear())
            .slide();
        ui.with_keyed(label, row, |ui| {
            if label == "a" {
                ui.leaf_keyed(
                    "knob",
                    NodeSpec::row()
                        .size(10.0, 10.0)
                        .bg(KNOB)
                        .transition_with(linear())
                        .slide(),
                );
            }
        });
    }
    ui.finish();
    (at(core, TRACK), at(core, KNOB))
}

#[test]
fn a_nested_slide_rides_its_sliding_parent_and_adds_only_its_own_move() {
    let mut core = Core::new();
    core.set_time(0.0);
    let (row, knob) = rows(&mut core, true, 0.0);
    assert_eq!((row, knob), ((0.0, 0.0), (0.0, 0.0)));
    // Row "a" moves down a place; its knob does not move within it, so it
    // rides the row's own ease exactly rather than trailing it.
    core.set_time(0.0);
    rows(&mut core, false, 0.0);
    for t in [0.025, 0.05, 0.075] {
        core.set_time(t);
        let (row, knob) = rows(&mut core, false, 0.0);
        assert!(
            row.1 > 0.0 && row.1 < 40.0,
            "the row is on its way: {row:?}"
        );
        assert_eq!(knob, row, "the knob is where the row is at {t}");
    }
    core.set_time(0.2);
    let (row, knob) = rows(&mut core, false, 0.0);
    assert_eq!((row, knob), ((0.0, 40.0), (0.0, 40.0)));
    assert!(!core.animating());

    // Both move: the row back up a place, the knob 40 px along it. Each
    // eases its own move, the knob's measured from where the row is drawn.
    core.set_time(1.0);
    rows(&mut core, true, 40.0);
    core.set_time(1.05);
    let (row, knob) = rows(&mut core, true, 40.0);
    assert_eq!(row.1, 20.0);
    assert_eq!(knob.1, row.1, "the knob keeps to the row's line");
    assert_eq!(knob.0 - row.0, 20.0, "and is halfway along it");
}

/// Two columns side by side; a card keyed the same wherever it is, in
/// column "left" or "right", below `above` px of other content.
fn columns(core: &mut Core, right: bool, above: f32) -> (f32, f32) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let card = Key::ROOT.str("card");
    for side in ["left", "right"] {
        ui.with_keyed(side, NodeSpec::column().size(200.0, 200.0), |ui| {
            if (side == "right") == right {
                ui.leaf(NodeSpec::column().size(10.0, above));
                ui.leaf_key(
                    card,
                    NodeSpec::row()
                        .size(50.0, 20.0)
                        .bg(CARD)
                        .transition_with(linear())
                        .slide(),
                );
            }
        });
    }
    ui.finish();
    at(core, CARD)
}

#[test]
fn a_card_taken_to_another_column_glides_from_where_it_was() {
    let mut core = Core::new();
    core.set_time(0.0);
    assert_eq!(columns(&mut core, false, 0.0), (0.0, 0.0));
    // To the other column and 40 px down: the frame it moves it is still
    // drawn where it was, and it glides across, not from the new column's
    // corner.
    core.set_time(0.0);
    assert_eq!(columns(&mut core, true, 40.0), (0.0, 0.0));
    assert!(core.animating());
    core.set_time(0.05);
    assert_eq!(columns(&mut core, true, 40.0), (100.0, 20.0));
    core.set_time(0.2);
    assert_eq!(columns(&mut core, true, 40.0), (200.0, 40.0));
    assert!(!core.animating());
}

/// Two sliding rows; row "a" holds a sliding viewport float at (300, 10).
/// Returns row "a"'s y and the float's.
fn rows_with_float(core: &mut Core, a_first: bool) -> (f32, f32) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let labels = if a_first { ["a", "b"] } else { ["b", "a"] };
    for label in labels {
        let bg = if label == "a" { TRACK } else { Color::WHITE };
        let row = NodeSpec::row()
            .size(200.0, 40.0)
            .bg(bg)
            .transition_with(linear())
            .slide();
        ui.with_keyed(label, row, |ui| {
            if label == "a" {
                ui.leaf_keyed(
                    "badge",
                    NodeSpec::row()
                        .size(10.0, 10.0)
                        .bg(KNOB)
                        .float(kui_core::FloatConfig::viewport().offset(300.0, 10.0))
                        .transition_with(linear())
                        .slide(),
                );
            }
        });
    }
    ui.finish();
    (at(core, TRACK).1, at(core, KNOB).1)
}

#[test]
fn a_sliding_viewport_float_is_carried_by_its_sliding_ancestor_without_a_lag_of_its_own() {
    let mut core = Core::new();
    core.set_time(0.0);
    assert_eq!(rows_with_float(&mut core, true), (0.0, 10.0));
    core.set_time(0.0);
    rows_with_float(&mut core, false);
    // Row "a" eases from 0 to 40; the float, which did not move in the
    // window, is carried as any node of the row is, by the row's lag alone.
    for t in [0.025, 0.05, 0.075] {
        core.set_time(t);
        let (row, badge) = rows_with_float(&mut core, false);
        assert_eq!(badge - 10.0, row - 40.0, "carried by the row at {t}");
    }
    core.set_time(0.2);
    assert_eq!(rows_with_float(&mut core, false), (40.0, 10.0));
    assert!(!core.animating());
}
