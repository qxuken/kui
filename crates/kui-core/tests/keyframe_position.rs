//! A keyframe stop names a position (backlog F132): `dx` / `dy` are an
//! offset from where layout put the node, sampled off the cycle and added
//! where an eased position is, so the node and its subtree are drawn and
//! hit along the stops while the room it takes stays its own place's.

use kui_core::testing::{click_at, kinds, solids};
use kui_core::{
    Color, Core, Easing, Keyframe, NodeSpec, Repeat, Size, TextStyle, Transition, Value,
};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

/// A row of two 40-px boxes at (20, 20): the first bobs by its stops over
/// a linear one-second cycle, the second sits beside it. Returns the two
/// boxes' quad rects, first then second.
fn bob(core: &mut Core, now: f64, stops: Vec<Keyframe>, slide_to: Option<f32>) -> Vec<(f32, f32)> {
    core.set_time(now);
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    ui.with(NodeSpec::row(), |ui| {
        let mut spec = NodeSpec::column()
            .size(40.0, 40.0)
            .bg(Color::WHITE)
            .on_click("bob")
            .transition_with(
                Transition::ms(1000.0)
                    .easing(Easing::Linear)
                    .repeat(Repeat::Normal),
            )
            .keyframes(stops);
        if let Some(x) = slide_to {
            spec = spec
                .slide()
                .float(kui_core::FloatConfig::parent().offset(x, 0.0));
        }
        ui.with_keyed("bob", spec, |ui| {
            ui.leaf(NodeSpec::column().size(10.0, 10.0).bg(Color::rgb8(1, 2, 3)));
        });
        ui.leaf_keyed(
            "still",
            NodeSpec::column().size(40.0, 40.0).bg(Color::rgb8(9, 9, 9)),
        );
    });
    ui.finish();
    solids(core).iter().map(|q| (q.rect.x, q.rect.y)).collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 0.51
}

#[test]
fn a_stop_s_dy_moves_the_node_and_its_subtree_and_nothing_else() {
    let mut core = Core::new();
    let stops = || vec![Keyframe::default().dy(0.0), Keyframe::default().dy(-20.0)];
    let at0 = bob(&mut core, 0.0, stops(), None);
    assert_eq!(at0[0], (20.0, 20.0), "the cycle starts at the node's place");
    let half = bob(&mut core, 0.5, stops(), None);
    assert!(near(half[0].1, 10.0), "halfway up: {half:?}");
    assert_eq!(half[0].0, 20.0, "a stop naming only dy leaves dx at 0");
    assert!(near(half[1].1, 10.0), "the child rides along: {half:?}");
    assert_eq!(half[2], (60.0, 20.0), "the sibling keeps its place");
    assert!(core.animating(), "a cycle owes its frames");
    assert!(core.owed().cycle && !core.owed().transition);
}

#[test]
fn a_moved_node_is_hit_where_it_is_drawn() {
    let mut core = Core::new();
    let stops = || {
        vec![
            Keyframe::default().offset(0.0, 0.0),
            Keyframe::default().offset(0.0, -40.0),
        ]
    };
    bob(&mut core, 0.5, stops(), None);
    // Drawn 20 px up: y 0..40 now, so y 5 is on it and y 50 is not.
    assert_eq!(kinds(&click_at(&mut core, 30.0, 5.0)), vec!["bob"]);
    bob(&mut core, 0.5, stops(), None);
    assert!(click_at(&mut core, 30.0, 50.0).is_empty());
}

#[test]
fn a_cycle_adds_to_a_slide() {
    let mut core = Core::new();
    let stops = || vec![Keyframe::default().dx(0.0), Keyframe::default().dx(10.0)];
    // Settled at x = 0 first, with the cycle at its start.
    bob(&mut core, 0.0, stops(), Some(0.0));
    bob(&mut core, 0.0, stops(), Some(0.0));
    // Retargeted to x = 100 at t = 0; at t = 0.5 the slide is halfway and
    // the cycle is halfway, so the box is at 20 + 50 + 5.
    bob(&mut core, 0.0, stops(), Some(100.0));
    let mid = bob(&mut core, 0.5, stops(), Some(100.0));
    // A float paints after the in-flow sibling: the box is the second.
    assert_eq!(mid[0], (20.0, 20.0), "the sibling, in flow");
    assert!(near(mid[1].0, 75.0), "slide 50 plus cycle 5: {mid:?}");
}

#[test]
fn stops_from_plain_data_take_dx_and_dy_and_refuse_a_word() {
    let stop = |fields: Vec<(&'static str, Value)>| Value::List(vec![Value::map(fields)]);
    let parsed = kui_core::keyframes::parse(&stop(vec![
        ("dx", Value::float(3.0)),
        ("dy", Value::float(-2.0)),
    ]))
    .unwrap();
    assert_eq!((parsed[0].dx, parsed[0].dy), (Some(3.0), Some(-2.0)));
    let only_dy = kui_core::keyframes::parse(&stop(vec![("dy", Value::float(4.0))])).unwrap();
    assert_eq!((only_dy[0].dx, only_dy[0].dy), (None, Some(4.0)));
    let err = kui_core::keyframes::parse(&stop(vec![("dx", Value::str("left"))])).unwrap_err();
    assert!(err.contains("dx must be a number"), "{err}");
}

#[test]
fn a_text_node_bobs_with_its_box() {
    let mut core = Core::new();
    core.set_time(0.5);
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    ui.text_in_keyed(
        "label",
        NodeSpec::row()
            .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
            .keyframes(vec![
                Keyframe::default().dy(0.0),
                Keyframe::default().dy(-20.0),
            ]),
        "hi",
        TextStyle::new(12.0),
    );
    ui.finish();
    let (dl, _) = core.output();
    let glyph_y = dl
        .quads
        .iter()
        .find(|q| {
            q.kind == kui_core::QuadKind::GlyphMask || q.kind == kui_core::QuadKind::GlyphSubpixel
        })
        .map(|q| q.rect.y);
    assert!(
        glyph_y.is_some_and(|y| y < 20.0),
        "the text rose with its box: {glyph_y:?}"
    );
}

#[test]
fn on_layout_reports_the_layout_rect_and_not_the_cycle() {
    let mut core = Core::new();
    let frame = |core: &mut Core, now: f64| {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed(
            "bob",
            NodeSpec::column()
                .size(40.0, 40.0)
                .on_layout("lay")
                .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
                .keyframes(vec![
                    Keyframe::default().dy(0.0),
                    Keyframe::default().dy(-20.0),
                ]),
        );
        ui.finish();
        core.take_pending_events()
            .into_iter()
            .filter(|e| e.kind() == Some("layout"))
            .map(|e| {
                e.payload
                    .get("y")
                    .and_then(Value::as_float)
                    .unwrap_or(f64::NAN)
            })
            .collect::<Vec<_>>()
    };
    assert_eq!(
        frame(&mut core, 0.0),
        vec![20.0],
        "the first frame reports its place"
    );
    // Mid-cycle the box is drawn 10 px up, and the view hears nothing:
    // the cycle is not a layout change, and it would be one every frame.
    assert!(frame(&mut core, 0.5).is_empty());
    assert!(frame(&mut core, 0.75).is_empty());
}
