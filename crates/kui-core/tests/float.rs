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

const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

#[test]
fn float_does_not_affect_parent_flow() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    // A fit row with one in-flow child and one huge float: fit ignores the float.
    ui.with(NodeSpec::row().gap(10.0).bg(BLUE), |ui| {
        ui.leaf(NodeSpec::column().size(50.0, 20.0));
        ui.leaf(
            NodeSpec::column()
                .size(500.0, 500.0)
                .float(FloatConfig::parent()),
        );
    });
    ui.finish();
    let quads = solid_quads(&mut core);
    let row = quads.iter().find(|q| q.4 == BLUE).unwrap();
    assert_eq!(
        (row.2, row.3),
        (50.0, 20.0),
        "row must size to the in-flow child only"
    );
}

#[test]
fn viewport_anchored_bottom_right() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.leaf(
        NodeSpec::column().size(40.0, 30.0).bg(RED).float(
            FloatConfig::viewport()
                .inside(Align::End, Align::End)
                .offset(-8.0, -8.0),
        ),
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
    ui.with(NodeSpec::column().size(100.0, 40.0).bg(BLUE), |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(60.0, 20.0)
                .bg(RED)
                .float(FloatConfig::below()),
        );
    });
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
        ui.leaf(
            NodeSpec::column()
                .size(50.0, 50.0)
                .bg(RED)
                .float(FloatConfig::parent()),
        );
        ui.leaf(NodeSpec::column().size(80.0, 80.0).bg(BLUE));
    });
    ui.finish();
    let quads = solid_quads(&mut core);
    let red_idx = quads.iter().position(|q| q.4 == RED).unwrap();
    let blue_idx = quads.iter().position(|q| q.4 == BLUE).unwrap();
    assert!(
        red_idx > blue_idx,
        "float painted after (on top of) in-flow content"
    );
}

#[test]
fn float_escapes_ancestor_clip_and_hits_topmost() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        // Clipping container at top-left; float positioned outside its bounds.
        ui.with(NodeSpec::column().size(50.0, 50.0).clip(), |ui| {
            ui.leaf(
                NodeSpec::column()
                    .size(60.0, 30.0)
                    .bg(RED)
                    .on_click("float")
                    .float(
                        FloatConfig::parent()
                            .at(Align::Start, Align::End)
                            .offset(0.0, 50.0),
                    ),
            );
        });
        // In-flow clickable covering the same area, declared later.
        ui.leaf(NodeSpec::column().size(300.0, 200.0).on_click("underneath"));
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
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
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
    ui.with(NodeSpec::column().size(100.0, 40.0).bg(BLUE), |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(60.0, 20.0)
                .bg(RED)
                .float(FloatConfig::below().fit()),
        );
    });
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
    ui.with(NodeSpec::column().size(100.0, 40.0).bg(BLUE), |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(200.0, 20.0)
                .bg(RED)
                .float(FloatConfig::below().fit()),
        );
    });
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
    ui.with(NodeSpec::column().size(100.0, 40.0).bg(BLUE), |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(60.0, 20.0)
                .bg(RED)
                .float(FloatConfig::below().fit()),
        );
    });
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
    ui.with(NodeSpec::column().size(10.0, 10.0), |ui| {
        ui.leaf(
            NodeSpec::column()
                .grow_width()
                .height(Sizing::Percent(0.5))
                .bg(RED)
                .float(FloatConfig::viewport()),
        );
    });
    ui.finish();
    let quads = solid_quads(&mut core);
    let f = quads.iter().find(|q| q.4 == RED).unwrap();
    assert_eq!((f.2, f.3), (400.0, 150.0));
}

/// Drop-zone pattern: percent-sized floats anchored to a parent's edges and
/// center tile it into hit regions, later floats winning where they overlap.
#[test]
fn percent_floats_tile_their_parent_into_zones() {
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let zone = |ax: Align, ay: Align, w: f32, h: f32, tag: &str| {
            NodeSpec::column()
                .float(FloatConfig::parent().inside(ax, ay))
                .width(Sizing::Percent(w))
                .height(Sizing::Percent(h))
                .on_drag(Value::str(tag))
        };
        ui.with_keyed("pane", NodeSpec::column().fill().on_click("pane"), |ui| {
            ui.leaf_keyed("l", zone(Align::Start, Align::Start, 0.25, 1.0, "l"));
            ui.leaf_keyed("r", zone(Align::End, Align::Start, 0.25, 1.0, "r"));
            ui.leaf_keyed("t", zone(Align::Start, Align::Start, 1.0, 0.25, "t"));
            ui.leaf_keyed("b", zone(Align::Start, Align::End, 1.0, 0.25, "b"));
            ui.leaf_keyed("c", zone(Align::Center, Align::Center, 0.5, 0.5, "c"));
        });
        ui.finish();
    };
    frame(&mut core);
    let probe = |core: &mut Core, x: f32, y: f32| -> String {
        core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
        let mut evs = core.handle_input(InputEvent::mouse_down(1));
        evs.extend(core.handle_input(InputEvent::mouse_up()));
        evs.iter()
            .find_map(|e| e.payload.get_str("tag").map(str::to_string))
            .or_else(|| {
                evs.iter()
                    .find_map(|e| e.payload.as_str().map(str::to_string))
            })
            .unwrap_or_default()
    };
    assert_eq!(probe(&mut core, 20.0, 150.0), "l", "left quarter");
    assert_eq!(probe(&mut core, 380.0, 150.0), "r", "right quarter");
    assert_eq!(probe(&mut core, 200.0, 20.0), "t", "top quarter");
    assert_eq!(probe(&mut core, 200.0, 280.0), "b", "bottom quarter");
    assert_eq!(probe(&mut core, 200.0, 150.0), "c", "center tile wins");
    assert_eq!(
        probe(&mut core, 20.0, 20.0),
        "t",
        "corner: later float on top"
    );
}

/// A cursor-anchored float (viewport anchor + offset) near the right edge
/// must stay by the cursor, clamped — not mirror to the far side of the
/// window the way a parent-anchored tooltip flips above/below.
#[test]
fn viewport_fit_clamps_instead_of_mirroring() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf(
        NodeSpec::column()
            .float(FloatConfig::viewport().offset(380.0, 100.0).fit())
            .size(100.0, 20.0)
            .bg(RED),
    );
    ui.finish();
    let quads = solid_quads(&mut core);
    let (x, y, ..) = quads[0];
    assert_eq!(
        (x, y),
        (300.0, 100.0),
        "clamped to the right edge, same row"
    );
}

/// The mind map's canvas (backlog F90): a toolbar over a `clip` canvas,
/// and a node on the canvas, a parent-anchored float, panned 20 px above
/// the canvas's top edge. `node` is the node's float, so a test can
/// declare it clipped or not.
fn canvas_with_a_node(core: &mut Core, node: FloatConfig) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf(
        NodeSpec::row()
            .grow_width()
            .height(40.0)
            .bg(BLUE)
            .on_click("toolbar"),
    );
    ui.with(NodeSpec::column().fill().clip(), |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(80.0, 40.0)
                .bg(RED)
                .on_click("node")
                .float(node.offset(40.0, -20.0)),
        );
        // In flow, declared after the node and under it: the node is
        // still a layer over it, clipped or not.
        ui.leaf(NodeSpec::column().size(200.0, 200.0).on_click("under"));
    });
    ui.finish();
}

fn click_at(core: &mut Core, x: f32, y: f32) -> Option<String> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    evs.first()
        .and_then(|e| e.payload.as_str())
        .map(str::to_string)
}

/// The node's quad and the rect of the clip it names.
fn node_quad(core: &mut Core) -> (kui_core::Rect, kui_core::Rect, usize) {
    let (dl, _) = core.output();
    let i = dl
        .quads
        .iter()
        .position(|q| q.kind == QuadKind::Solid && q.color == RED)
        .expect("the node draws");
    let q = &dl.quads[i];
    (q.rect, dl.clips[q.clip as usize].rect, i)
}

/// A float that declares `clip` is cut by its parent's clip, as a child
/// is, and its hit region with it: the half panned past the canvas's top
/// edge neither draws over the toolbar nor takes the press there. The
/// half inside is still a layer over the in-flow content it sits on.
#[test]
fn a_clipped_float_is_cut_by_its_parent_and_not_hit_past_it() {
    let mut core = Core::new();
    canvas_with_a_node(&mut core, FloatConfig::parent().clipped());
    let (rect, clip, i) = node_quad(&mut core);
    assert_eq!((rect.y, rect.h), (20.0, 40.0), "placed half past the edge");
    assert_eq!(
        (clip.y, clip.h),
        (40.0, 260.0),
        "clipped to the canvas, not the window"
    );
    // Paint order is the float's: after the in-flow sibling declared
    // after it (the only other clickable box there, and transparent, so
    // the toolbar is the one other solid quad).
    let (dl, _) = core.output();
    let toolbar = dl
        .quads
        .iter()
        .position(|q| q.kind == QuadKind::Solid && q.color == BLUE)
        .unwrap();
    assert!(i > toolbar);

    // Over the toolbar, where the cut half would be: the toolbar.
    assert_eq!(click_at(&mut core, 60.0, 30.0).as_deref(), Some("toolbar"));
    // Inside the canvas: the node, over the in-flow box under it.
    assert_eq!(click_at(&mut core, 60.0, 50.0).as_deref(), Some("node"));
    // Beside it: the in-flow box.
    assert_eq!(click_at(&mut core, 150.0, 50.0).as_deref(), Some("under"));
}

/// Without `clip` the same node escapes as every float did (F90's
/// report): it draws over the toolbar and takes the press there.
#[test]
fn an_unclipped_float_still_escapes_its_parents_clip() {
    let mut core = Core::new();
    canvas_with_a_node(&mut core, FloatConfig::parent());
    let (_, clip, _) = node_quad(&mut core);
    assert!(clip.y <= 20.0, "escaped: {clip:?}");
    assert_eq!(click_at(&mut core, 60.0, 30.0).as_deref(), Some("node"));
}

/// `clip` is read with the parent anchor only: a viewport float is placed
/// against the window, not the parent, and escapes with the bit set.
#[test]
fn clip_is_read_with_the_parent_anchor_only() {
    let mut core = Core::new();
    canvas_with_a_node(
        &mut core,
        FloatConfig::viewport()
            .at(Align::Start, Align::Start)
            .clipped(),
    );
    let (rect, clip, _) = node_quad(&mut core);
    // At (40, -20) in the window: half off its top, and not cut at 40.
    assert_eq!(rect.y, -20.0);
    assert!(clip.y <= -20.0, "escaped: {clip:?}");
    assert_eq!(click_at(&mut core, 60.0, 10.0).as_deref(), Some("node"));
}

/// The exit ghost reads the same bit (RG26's rule for strokes, now any
/// clipped float's): a panel playing its exit keeps its node cut at the
/// canvas's edge rather than drawing it over the toolbar.
#[test]
fn a_departing_clipped_float_is_cut_by_its_parent() {
    let mut core = Core::new();
    let build = |core: &mut Core, now: f64, show: bool| {
        core.set_time(now);
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        if show {
            let panel = NodeSpec::column()
                .fill()
                .transition(100.0)
                .exit(kui_core::Enter::default().opacity(0.0));
            ui.with_keyed("panel", panel, |ui| {
                ui.leaf(NodeSpec::row().grow_width().height(40.0).bg(BLUE));
                ui.with(NodeSpec::column().fill().clip(), |ui| {
                    ui.leaf(
                        NodeSpec::column()
                            .size(80.0, 40.0)
                            .bg(RED)
                            .float(FloatConfig::parent().clipped().offset(40.0, -20.0)),
                    );
                });
            });
        }
        ui.finish();
    };
    build(&mut core, 0.0, true);
    build(&mut core, 0.0, false);
    build(&mut core, 0.01, false);
    assert!(core.animating(), "the exit is playing");
    let (dl, _) = core.output();
    let node: Vec<_> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.rect.w == 80.0 && q.rect.h == 40.0)
        .map(|q| (q.rect, dl.clips[q.clip as usize].rect))
        .collect();
    assert_eq!(node.len(), 1, "the ghost draws the node: {node:?}");
    let (rect, clip) = node[0];
    assert_eq!(rect.y, 20.0);
    assert_eq!(clip.y, 40.0, "the ghost's node over the toolbar: {clip:?}");
}
