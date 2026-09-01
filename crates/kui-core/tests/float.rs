//! Floating (out-of-flow) nodes: anchor placement, flow exclusion, paint
//! order, clip escape, and topmost hit-testing — through a live `Core`.

use kui_core::{
    Align, Color, Core, FloatConfig, InputEvent, NodeSpec, QuadKind, Size, Sizing, Value, Vec2,
};

fn solid_quads(core: &mut Core) -> Vec<(f32, f32, f32, f32, Color)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .map(|q| (q.rect.x, q.rect.y, q.rect.w, q.rect.h, q.color))
        .collect()
}

const RED: Color = Color { r: 1.0, g: 0.0, b: 0.0, a: 1.0 };
const BLUE: Color = Color { r: 0.0, g: 0.0, b: 1.0, a: 1.0 };

#[test]
fn float_does_not_affect_parent_flow() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    // A fit row with one in-flow child and one huge float: fit ignores the float.
    ui.with(NodeSpec::row().gap(10.0).bg(BLUE), |ui| {
        ui.with(NodeSpec::column().width(Sizing::Fixed(50.0)).height(Sizing::Fixed(20.0)), |_| {});
        ui.with(
            NodeSpec::column()
                .width(Sizing::Fixed(500.0))
                .height(Sizing::Fixed(500.0))
                .float(FloatConfig::parent()),
            |_| {},
        );
    });
    ui.finish();
    let quads = solid_quads(&mut core);
    let row = quads.iter().find(|q| q.4 == BLUE).unwrap();
    assert_eq!((row.2, row.3), (50.0, 20.0), "row must size to the in-flow child only");
}

#[test]
fn viewport_anchored_bottom_right() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.with(
        NodeSpec::column()
            .width(Sizing::Fixed(40.0))
            .height(Sizing::Fixed(30.0))
            .bg(RED)
            .float(FloatConfig::viewport().at(Align::End, Align::End).self_at(Align::End, Align::End).offset(-8.0, -8.0)),
        |_| {},
    );
    ui.finish();
    let quads = solid_quads(&mut core);
    let f = quads.iter().find(|q| q.4 == RED).unwrap();
    assert_eq!((f.0, f.1), (400.0 - 40.0 - 8.0, 300.0 - 30.0 - 8.0));
}

#[test]
fn tooltip_below_parent_centered() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(100.0));
    ui.with(
        NodeSpec::column().width(Sizing::Fixed(100.0)).height(Sizing::Fixed(40.0)).bg(BLUE),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(60.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(RED)
                    .float(FloatConfig::below()),
                |_| {},
            );
        },
    );
    ui.finish();
    let quads = solid_quads(&mut core);
    let f = quads.iter().find(|q| q.4 == RED).unwrap();
    // Parent at (100,100) 100x40; tooltip centered: x = 100+50-30, y = 140+6.
    assert_eq!((f.0, f.1), (120.0, 146.0));
}

#[test]
fn floats_paint_after_in_flow_siblings() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.with(NodeSpec::column(), |ui| {
        // Float declared FIRST, in-flow sibling after: float must still be on top.
        ui.with(
            NodeSpec::column()
                .width(Sizing::Fixed(50.0))
                .height(Sizing::Fixed(50.0))
                .bg(RED)
                .float(FloatConfig::parent()),
            |_| {},
        );
        ui.with(NodeSpec::column().width(Sizing::Fixed(80.0)).height(Sizing::Fixed(80.0)).bg(BLUE), |_| {});
    });
    ui.finish();
    let quads = solid_quads(&mut core);
    let red_idx = quads.iter().position(|q| q.4 == RED).unwrap();
    let blue_idx = quads.iter().position(|q| q.4 == BLUE).unwrap();
    assert!(red_idx > blue_idx, "float painted after (on top of) in-flow content");
}

#[test]
fn float_escapes_ancestor_clip_and_hits_topmost() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        // Clipping container at top-left; float positioned outside its bounds.
        ui.with(
            NodeSpec::column().width(Sizing::Fixed(50.0)).height(Sizing::Fixed(50.0)).clip(),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Fixed(60.0))
                        .height(Sizing::Fixed(30.0))
                        .bg(RED)
                        .on_click(Value::str("float"))
                        .float(FloatConfig::parent().at(Align::Start, Align::End).offset(0.0, 50.0)),
                    |_| {},
                );
            },
        );
        // In-flow clickable covering the same area, declared later.
        ui.with(
            NodeSpec::column()
                .width(Sizing::Fixed(300.0))
                .height(Sizing::Fixed(200.0))
                .on_click(Value::str("underneath")),
            |_| {},
        );
        ui.finish();
    };
    build(&mut core);
    let quads = solid_quads(&mut core);
    // Visible despite sitting outside the clipping parent (y 100..130).
    let f = quads.iter().find(|q| q.4 == RED);
    assert!(f.is_some(), "float must escape the ancestor clip");
    assert_eq!(f.unwrap().1, 100.0);

    // Click inside the float area: the float wins over the in-flow region.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(30.0, 110.0)));
    core.handle_input(InputEvent::MouseDown(1));
    let evs = core.handle_input(InputEvent::MouseUp);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.as_str(), Some("float"));
}

#[test]
fn fit_flips_below_to_above_near_viewport_bottom() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    // Parent hugs the bottom: below-placement (y 286..306) would leave the
    // 300px viewport, the mirrored above-placement fits — so it flips.
    ui.configure_root(NodeSpec::column().fill().main_align(Align::End));
    ui.with(
        NodeSpec::column().width(Sizing::Fixed(100.0)).height(Sizing::Fixed(40.0)).bg(BLUE),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(60.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(RED)
                    .float(FloatConfig::below().fit()),
                |_| {},
            );
        },
    );
    ui.finish();
    let quads = solid_quads(&mut core);
    let f = quads.iter().find(|q| q.4 == RED).unwrap();
    // Parent at (0,260) 100x40; flipped above: y = 260 - 20 - 6, x centered.
    assert_eq!((f.0, f.1), (20.0, 234.0));
}

#[test]
fn fit_clamps_when_flipping_cannot_help() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    // Wide tooltip centered under a parent at the left edge: both the
    // declared and mirrored x-placements stick out equally (Center mirrors
    // to itself), so the declared side is kept and clamped to x = 0.
    ui.with(
        NodeSpec::column().width(Sizing::Fixed(100.0)).height(Sizing::Fixed(40.0)).bg(BLUE),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(200.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(RED)
                    .float(FloatConfig::below().fit()),
                |_| {},
            );
        },
    );
    ui.finish();
    let quads = solid_quads(&mut core);
    let f = quads.iter().find(|q| q.4 == RED).unwrap();
    assert_eq!((f.0, f.1), (0.0, 46.0));
}

#[test]
fn fit_keeps_declared_side_when_it_fits() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(100.0));
    ui.with(
        NodeSpec::column().width(Sizing::Fixed(100.0)).height(Sizing::Fixed(40.0)).bg(BLUE),
        |ui| {
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(60.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(RED)
                    .float(FloatConfig::below().fit()),
                |_| {},
            );
        },
    );
    ui.finish();
    let quads = solid_quads(&mut core);
    let f = quads.iter().find(|q| q.4 == RED).unwrap();
    // Same placement as the no-fit tooltip_below_parent_centered test.
    assert_eq!((f.0, f.1), (120.0, 146.0));
}

#[test]
fn float_grow_sizes_against_viewport() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.with(NodeSpec::column().width(Sizing::Fixed(10.0)).height(Sizing::Fixed(10.0)), |ui| {
        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Percent(0.5))
                .bg(RED)
                .float(FloatConfig::viewport()),
            |_| {},
        );
    });
    ui.finish();
    let quads = solid_quads(&mut core);
    let f = quads.iter().find(|q| q.4 == RED).unwrap();
    assert_eq!((f.2, f.3), (400.0, 150.0));
}
