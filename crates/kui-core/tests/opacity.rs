//! Group opacity: a node's `opacity` multiplies into the alpha of every
//! quad it and its subtree emit, it compounds down the tree, and it leaves
//! layout, hit-testing and the access tree alone.

use kui_core::{Color, Core, InputEvent, NodeSpec, Size, Sizing, TextStyle, Value, Vec2};

const VIEW: Size = Size { w: 200.0, h: 100.0 };

/// A panel with a text child, at `outer` opacity with the panel's own box
/// at `inner`. Returns every quad's alpha in emission order.
fn alphas(core: &mut Core, outer: f32, inner: f32) -> Vec<f32> {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed(
        "panel",
        NodeSpec::column()
            .opacity(outer)
            .bg(Color::WHITE)
            .border(2.0, Color::WHITE)
            .width(Sizing::Fixed(80.0))
            .height(Sizing::Fixed(40.0)),
        |ui| {
            ui.with_keyed(
                "inner",
                NodeSpec::column()
                    .opacity(inner)
                    .bg(Color::WHITE)
                    .width(Sizing::Fixed(20.0))
                    .height(Sizing::Fixed(10.0)),
                |ui| ui.text("hi", TextStyle::new(12.0).color(Color::WHITE)),
            );
        },
    );
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.iter().map(|q| q.color.a).collect()
}

#[test]
fn opacity_fades_the_whole_subtree_and_compounds() {
    let mut core = Core::new();
    // The panel's box, the inner box, then one glyph per letter of "hi".
    assert_eq!(alphas(&mut core, 1.0, 1.0), vec![1.0; 4]);

    let half = alphas(&mut core, 0.5, 1.0);
    assert_eq!(half, vec![0.5; 4], "the boxes and the glyphs alike");

    // A nested fade multiplies with its ancestors, it does not replace them.
    let nested = alphas(&mut core, 0.5, 0.5);
    assert_eq!(nested, vec![0.5, 0.25, 0.25, 0.25]);
}

#[test]
fn opacity_fades_the_border_too() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.leaf_keyed(
        "panel",
        NodeSpec::column()
            .opacity(0.25)
            .border(2.0, Color::WHITE)
            .width(Sizing::Fixed(80.0))
            .height(Sizing::Fixed(40.0)),
    );
    ui.finish();
    let (dl, _) = core.output();
    assert_eq!(dl.quads[0].border_color.a, 0.25);
}

/// CSS's rule, and the one that makes a fade-out usable: an invisible
/// subtree still occupies its box, still answers the pointer, and is still
/// read out.
#[test]
fn a_faded_subtree_still_lays_out_and_still_takes_clicks() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed("panel", NodeSpec::column().opacity(0.0).pad(4.0), |ui| {
            ui.leaf_keyed(
                "btn",
                NodeSpec::column()
                    .on_click(Value::str("hit"))
                    .label("Save")
                    .width(Sizing::Fixed(40.0))
                    .height(Sizing::Fixed(20.0)),
            );
        });
        ui.finish();
    };
    build(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 12.0)));
    build(&mut core);
    core.handle_input(InputEvent::mouse_down(1));
    let events = core.handle_input(InputEvent::mouse_up());
    assert_eq!(
        events.iter().filter_map(|e| e.payload.as_str()).count(),
        1,
        "an invisible button still clicks"
    );
    build(&mut core);
    let names: Vec<_> = core
        .access_tree()
        .nodes
        .iter()
        .filter_map(|n| n.name.clone())
        .collect();
    assert!(names.iter().any(|n| n == "Save"), "{names:?}");
}

#[test]
fn a_transition_eases_opacity_and_enter_fades_a_panel_in() {
    let mut core = Core::new();
    let frame = |core: &mut Core, t: f64, o: f32| {
        core.set_time(t);
        let mut ui = core.frame(VIEW, 1.0);
        ui.leaf_keyed(
            "panel",
            NodeSpec::column()
                .transition(100.0)
                .easing(kui_core::Easing::Linear)
                .enter(kui_core::Enter::default().opacity(0.0))
                .opacity(o)
                .bg(Color::WHITE)
                .width(Sizing::Fixed(40.0))
                .height(Sizing::Fixed(20.0)),
        );
        ui.finish();
        let (dl, _) = core.output();
        dl.quads[0].color.a
    };
    // First sight starts at the entrance, not at the declared value.
    assert_eq!(frame(&mut core, 0.0, 1.0), 0.0);
    let mid = frame(&mut core, 0.05, 1.0);
    assert!(mid > 0.3 && mid < 0.7, "half way in: {mid}");
    assert_eq!(frame(&mut core, 0.2, 1.0), 1.0);
    // And it eases back out when the view lowers it: the frame that
    // retargets still sits at the old value, the next one is on its way.
    assert_eq!(frame(&mut core, 0.25, 0.0), 1.0);
    let out = frame(&mut core, 0.3, 0.0);
    assert!(out > 0.3 && out < 0.7, "easing out: {out}");
}
