//! ADR 0074, through a live `Core`: what a node and its subtree paint is
//! cut by the nearest box above it that clips or scrolls, whatever its
//! float, move, keyframes, turn or sibling order — unless it declares it
//! leaves (a viewport float, or `escape` on a float anchored to its
//! parent). One test per way a node's paint could leave, each in the same
//! window: a 40 px bar over a scroller whose content is scrolled 100 px
//! under the bar, the node under test at the scroller's top edge and
//! pushed 20 px up into the bar.
//!
//! Each test checks the draw list two ways: no quad, cut by the clip it
//! names, shows over the bar; and the paint audit (`Core::paint_audit`)
//! found nothing.

use kui_core::testing::tab;
use kui_core::{
    Color, Core, Enter, FloatConfig, Key, Keyframe, NodeSpec, QuadKind, Rect, Size, Ui, Vec2,
};

const BAR: u32 = 0x3a3f52ff;
const RED: u32 = 0xd9738cff;
const BAR_H: f32 = 40.0;

/// One frame at `now`: the bar, then the scroller holding a 100 px spacer,
/// `content`, and room to scroll. Returns the scroller's key.
fn frame(core: &mut Core, now: f64, content: &dyn Fn(&mut Ui<'_>)) -> Key {
    core.set_time(now);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf(
        NodeSpec::row()
            .grow_width()
            .height(BAR_H)
            .bg(Color::hex(BAR)),
    );
    let pane = ui.with_keyed(
        "pane",
        NodeSpec::column().grow_width().height(200.0).scroll_y(),
        |ui| {
            ui.leaf(NodeSpec::column().size(10.0, 100.0));
            content(ui);
            ui.leaf(NodeSpec::column().size(10.0, 600.0));
        },
    );
    ui.finish();
    pane
}

/// The window with `content` scrolled to the scroller's top edge, the
/// audit on, drawn at 0 and again at `then`.
fn scrolled(content: &dyn Fn(&mut Ui<'_>), then: f64) -> Core {
    let mut core = Core::new();
    core.set_paint_audit(true);
    let pane = frame(&mut core, 0.0, content);
    core.set_scroll(pane, Vec2::new(0.0, 100.0));
    frame(&mut core, 0.0, content);
    frame(&mut core, then, content);
    core
}

/// The quads that show over the bar, each cut by the clip it names: the
/// bar's own left out.
fn over_bar(core: &mut Core) -> Vec<(QuadKind, Rect)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.color != Color::hex(BAR))
        .filter(|q| !matches!(q.kind, QuadKind::GroupBegin | QuadKind::GroupEnd))
        .filter_map(|q| {
            let d = q.rect.intersect(&dl.clip_of(q).rect);
            (d.w > 0.0 && d.h > 0.0 && d.y < BAR_H - 0.5).then_some((q.kind, d))
        })
        .collect()
}

/// Whether the node under test drew at all, in the scroller below the bar.
fn drew_red(core: &mut Core) -> bool {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .any(|q| q.color == Color::hex(RED) && q.rect.intersect(&dl.clip_of(q).rect).h > 0.0)
}

/// Nothing over the bar, the node drawn below it, the audit clean.
fn held(mut core: Core) {
    assert!(drew_red(&mut core), "the node draws below the edge");
    let over = over_bar(&mut core);
    assert!(over.is_empty(), "drawn over the bar: {over:?}");
    let audit = core.paint_audit().expect("on");
    assert!(audit.quads > 0, "the audit looked: {audit:?}");
    assert!(audit.escapes.is_empty(), "{:?}", audit.escapes);
}

fn red(w: f32, h: f32) -> NodeSpec {
    NodeSpec::column().size(w, h).bg(Color::hex(RED))
}

#[test]
fn a_float_anchored_to_its_parent_is_held() {
    held(scrolled(
        &|ui| {
            ui.with(NodeSpec::row().size(100.0, 20.0), |ui| {
                ui.leaf(red(40.0, 40.0).float(FloatConfig::parent().offset(0.0, -20.0)));
            });
        },
        0.0,
    ));
}

/// The way out is declared: a float that says `escape` is drawn whole over
/// the bar, and the audit leaves it alone; so is a viewport float.
#[test]
fn a_float_that_says_escape_leaves_and_is_not_an_escape() {
    for float in [
        FloatConfig::parent().offset(0.0, -20.0).escaping(),
        FloatConfig::below().offset(0.0, -50.0).escaping(),
        FloatConfig::viewport().offset(10.0, 10.0),
    ] {
        let mut core = scrolled(
            &|ui| {
                ui.with(NodeSpec::row().size(100.0, 20.0), |ui| {
                    ui.leaf(red(40.0, 40.0).float(float));
                });
            },
            0.0,
        );
        assert!(
            !over_bar(&mut core).is_empty(),
            "{float:?} shows over the bar"
        );
        let audit = core.paint_audit().unwrap();
        assert!(audit.escapes.is_empty(), "{float:?}: {:?}", audit.escapes);
    }
}

#[test]
fn a_node_moved_by_dx_dy_is_held() {
    held(scrolled(
        &|ui| {
            ui.leaf(red(100.0, 40.0).dx(10.0).dy(-20.0));
        },
        0.0,
    ));
}

#[test]
fn a_node_moved_by_keyframes_is_held() {
    held(scrolled(
        &|ui| {
            ui.leaf_keyed(
                "bob",
                red(100.0, 40.0)
                    .keyframes(vec![
                        Keyframe::default().at(0.0).dy(-20.0),
                        Keyframe::default().at(1.0).dy(-20.0),
                    ])
                    .transition(1000.0),
            );
        },
        0.25,
    ));
}

/// A turn and a scale draw past the node's own box, through the clip its
/// scroller hands it: still cut at the scroller's edge. (The audit leaves
/// turned nodes to this check: their rects are in a space of their own.)
#[test]
fn a_turned_or_scaled_node_is_held() {
    held(scrolled(
        &|ui| {
            ui.with(NodeSpec::row().gap(40.0), |ui| {
                ui.leaf(red(60.0, 30.0).rotate(0.125));
                ui.leaf(red(30.0, 30.0).scale(3.0));
                // Upright, for the audit to look at something.
                ui.leaf(red(20.0, 20.0));
            });
        },
        0.0,
    ));
}

#[test]
fn an_over_siblings_node_is_held() {
    held(scrolled(
        &|ui| {
            ui.with(NodeSpec::row(), |ui| {
                ui.leaf(
                    NodeSpec::column()
                        .size(60.0, 40.0)
                        .bg(Color::hex(0x73d98cff)),
                );
                ui.leaf(red(60.0, 40.0).over_siblings().dx(-30.0).dy(-20.0));
            });
        },
        0.0,
    ));
}

/// Floats painted in tree order (ADR 0053) are reordered, not released.
#[test]
fn floats_painted_in_tree_order_are_held() {
    held(scrolled(
        &|ui| {
            ui.with(
                NodeSpec::column().size(200.0, 40.0).floats_in_order(),
                |ui| {
                    ui.leaf(red(40.0, 40.0).float(FloatConfig::parent().offset(0.0, -20.0)));
                    ui.with(
                        NodeSpec::column()
                            .size(40.0, 40.0)
                            .bg(Color::hex(0x73d98cff))
                            .float(FloatConfig::parent().offset(60.0, -20.0)),
                        |ui| {
                            ui.leaf(
                                red(20.0, 20.0).float(FloatConfig::parent().offset(0.0, -10.0)),
                            );
                        },
                    );
                },
            );
        },
        0.0,
    ));
}

/// A row leaving the scroller plays its exit inside it: the ghost keeps
/// the clip it was held by on the frame it left.
#[test]
fn a_departing_node_is_held_by_the_box_it_left() {
    let mut core = Core::new();
    core.set_paint_audit(true);
    let row = |show: bool| {
        move |ui: &mut Ui<'_>| {
            if show {
                ui.leaf_keyed(
                    "row",
                    red(200.0, 40.0)
                        .transition(200.0)
                        .exit(Enter::from(0.0, -40.0)),
                );
            } else {
                ui.leaf(NodeSpec::column().size(10.0, 40.0));
            }
        }
    };
    let pane = frame(&mut core, 0.0, &row(true));
    core.set_scroll(pane, Vec2::new(0.0, 100.0));
    frame(&mut core, 0.0, &row(true));
    frame(&mut core, 0.0, &row(false));
    frame(&mut core, 0.1, &row(false));
    assert!(core.animating(), "the exit plays");
    held(core);
}

/// The core's focus ring, 2 px outside a control flush against the
/// scroller's top edge, is cut there; so is a ring a view draws itself as
/// a float 4 px out from a mark (Kit's checkbox).
#[test]
fn a_focus_ring_at_the_edge_is_held() {
    let content = |ui: &mut Ui<'_>| {
        ui.with(NodeSpec::row().gap(8.0), |ui| {
            ui.leaf_keyed("go", red(80.0, 30.0).focusable().on_click("go"));
            ui.with(NodeSpec::row().size(14.0, 14.0), |ui| {
                ui.leaf(
                    NodeSpec::column()
                        .size(22.0, 22.0)
                        .border(2.0, Color::hex(RED))
                        .float(FloatConfig::parent().offset(-4.0, -4.0)),
                );
            });
        });
    };
    let mut core = Core::new();
    core.set_paint_audit(true);
    let pane = frame(&mut core, 0.0, &content);
    frame(&mut core, 0.0, &content);
    tab(&mut core, false);
    frame(&mut core, 0.0, &content);
    assert!(core.focus_visible(), "the ring shows");
    // Tab revealed the control; scrolled back so it stands 4 px under the
    // edge, the rings 2 and 4 px further out.
    core.set_scroll(pane, Vec2::new(0.0, 104.0));
    frame(&mut core, 0.0, &content);
    frame(&mut core, 0.0, &content);
    assert!(core.focus_visible(), "the ring still shows");
    held(core);
}
