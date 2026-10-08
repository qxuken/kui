//! The paint order as a stack of layers, and the input that reads it
//! (`docs/adr/0023-layers-stack-in-the-order-they-open.md`): the in-flow
//! tree, then one layer per float in the order they opened, each with its
//! own chrome — scrollbars, the ring — at its end. Pinned by quad index,
//! since the corpus digest carries order but is not checked in.

use kui_core::{
    Align, Color, Core, CursorShape, EditKey, FloatConfig, InputEvent, Mods, MouseButton, NodeSpec,
    QuadKind, Size, Vec2,
};

const VIEW: Size = Size { w: 400.0, h: 200.0 };

/// Colours that name their quads: a component of exactly 1.0 apiece.
const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const GREEN: Color = Color {
    r: 0.0,
    g: 1.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};
const GREY: Color = Color {
    r: 0.2,
    g: 0.2,
    b: 0.2,
    a: 1.0,
};

/// Every solid quad of the last frame, named: a coloured box by its
/// colour, a scrollbar thumb by its width, the ring by being a border
/// with no fill, anything else `?`.
fn painted(core: &mut Core) -> Vec<&'static str> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Solid)
        .map(|q| {
            if q.color == RED {
                "red"
            } else if q.color == GREEN {
                "green"
            } else if q.color == BLUE {
                "blue"
            } else if q.color == GREY {
                "grey"
            } else if q.rect.w == 4.0 || q.rect.w == 6.0 {
                "bar"
            } else if q.color.a == 0.0 && q.border_w > 0.0 {
                "ring"
            } else {
                "?"
            }
        })
        .collect()
}

/// A 200×100 float at (`x`, 50) in the viewport.
fn float_at(x: f32, bg: Color) -> NodeSpec {
    NodeSpec::column().size(200.0, 100.0).bg(bg).float(
        FloatConfig::viewport()
            .inside(Align::Start, Align::Start)
            .offset(x, 50.0),
    )
}

/// A scroller filling the window with twenty grey rows (600 of content in
/// 200), so it has a bar at `x≈394`, and a float that covers the bar.
fn scroller_and_float(core: &mut Core, modal: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
        for i in 0..20 {
            ui.leaf_keyed(
                &format!("row{i}"),
                NodeSpec::row().grow_width().height(30.0).bg(GREY),
            );
        }
    });
    let mut spec = float_at(250.0, RED)
        .on_click("menu")
        .cursor(CursorShape::Pointer);
    if modal {
        spec = spec.modal("menu");
    }
    ui.leaf_keyed("menu", spec);
    ui.finish();
}

#[test]
fn a_scrollers_bar_paints_under_a_float_over_it() {
    for modal in [false, true] {
        let mut core = Core::new();
        scroller_and_float(&mut core, modal);
        let order = painted(&mut core);
        let bar = order.iter().position(|n| *n == "bar").unwrap();
        let menu = order.iter().position(|n| *n == "red").unwrap();
        assert!(
            bar < menu,
            "modal={modal}: the bar is the in-flow layer's chrome, under the float: {order:?}"
        );
    }
}

/// The input half of the same order: a press on a non-modal float over the
/// bar's track reaches the float, not the track — and the cursor is the
/// float's, not the bar's arrow. Under the old order the press jumped the
/// scroller and the float never heard it.
#[test]
fn a_press_on_a_float_over_the_bar_reaches_the_float() {
    let mut core = Core::new();
    scroller_and_float(&mut core, false);
    let list = core.key_of("list").unwrap();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(395.0, 100.0)));
    assert_eq!(core.cursor_shape(), CursorShape::Pointer, "the float's");
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    let out = core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    assert_eq!(out.len(), 1, "one click: {out:?}");
    assert_eq!(out[0].payload.as_str(), Some("menu"));
    scroller_and_float(&mut core, false);
    assert_eq!(
        core.scroll_offset(list).y,
        0.0,
        "the track was never pressed"
    );

    // Beside the float the bar is what is there, and it still wins its
    // own scroller's rows: a track press below the thumb pages down.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(395.0, 180.0)));
    assert_eq!(core.cursor_shape(), CursorShape::Default);
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    scroller_and_float(&mut core, false);
    assert!(core.scroll_offset(list).y > 0.0, "the track jumped");
}

/// A menu bar at the top of the tree with its dropdown, a body with a
/// tooltip, a focusable control: what stacks over what depends on when
/// each float opened, not on where it is in the tree.
fn bar_and_body(core: &mut Core, dropdown: bool, tooltip: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("bar", NodeSpec::row().height(24.0), |ui| {
        if dropdown {
            ui.leaf_keyed("dropdown", float_at(100.0, RED));
        }
    });
    ui.leaf_keyed(
        "btn",
        NodeSpec::row()
            .size(80.0, 30.0)
            .bg(GREEN)
            .on_click("btn")
            .focusable(),
    );
    if tooltip {
        ui.leaf_keyed("tip", float_at(150.0, BLUE));
    }
    ui.finish();
}

#[test]
fn floats_stack_in_the_order_they_opened() {
    let mut core = Core::new();
    // Both in one frame: tree order breaks the tie.
    bar_and_body(&mut core, true, true);
    assert_eq!(painted(&mut core), ["green", "red", "blue"]);

    // The tooltip goes, and comes back: it opened later, so it is on top —
    // which is the same picture, and the same stack.
    bar_and_body(&mut core, true, false);
    assert_eq!(painted(&mut core), ["green", "red"]);
    bar_and_body(&mut core, true, true);
    assert_eq!(painted(&mut core), ["green", "red", "blue"]);

    // The dropdown closes and reopens over the tooltip that stayed: the
    // later opening wins, whatever the tree says.
    bar_and_body(&mut core, false, true);
    assert_eq!(painted(&mut core), ["green", "blue"]);
    bar_and_body(&mut core, true, true);
    assert_eq!(
        painted(&mut core),
        ["green", "blue", "red"],
        "the dropdown reopened over the tooltip"
    );
    // And stays there while both are declared.
    bar_and_body(&mut core, true, true);
    assert_eq!(painted(&mut core), ["green", "blue", "red"]);
}

/// A float's hit regions land in the same order as its quads, so the
/// stack decides the press too: the reopened dropdown over the tooltip is
/// what a click where they overlap reaches.
#[test]
fn the_hit_list_is_the_stack() {
    let mut core = Core::new();
    let build = |core: &mut Core, dropdown: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        if dropdown {
            ui.leaf_keyed("dropdown", float_at(100.0, RED).on_click("dropdown"));
        }
        ui.leaf_keyed("tip", float_at(150.0, BLUE).on_click("tip"));
        ui.finish();
    };
    let click = |core: &mut Core| -> Option<String> {
        core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 100.0)));
        core.handle_input(InputEvent::MouseDown {
            button: MouseButton::Primary,
            clicks: 1,
        });
        core.handle_input(InputEvent::MouseUp {
            button: MouseButton::Primary,
        })
        .pop()
        .and_then(|e| e.payload.as_str().map(str::to_string))
    };
    build(&mut core, true);
    assert_eq!(click(&mut core).as_deref(), Some("tip"), "tree order");
    build(&mut core, false);
    build(&mut core, true);
    assert_eq!(
        click(&mut core).as_deref(),
        Some("dropdown"),
        "reopened over the tooltip, so it takes the press"
    );
}

#[test]
fn the_ring_paints_at_the_end_of_its_layer() {
    let mut core = Core::new();
    bar_and_body(&mut core, true, true);
    let btn = core.key_of("btn").unwrap();
    core.set_focus(Some(btn));
    // A Tab makes focus keyboard-visible; the ring shows on the next frame.
    core.handle_input(InputEvent::Key(EditKey::Tab, Mods::default()));
    core.set_focus(Some(btn));
    bar_and_body(&mut core, true, true);
    assert_eq!(
        painted(&mut core),
        ["green", "ring", "red", "blue"],
        "the in-flow layer's ring, under every float"
    );
}

/// A focused control inside a float: its ring ends that float's layer,
/// under the floats over it.
#[test]
fn a_ring_inside_a_float_ends_that_layer() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("popover", float_at(100.0, RED), |ui| {
            ui.leaf_keyed(
                "ok",
                NodeSpec::row()
                    .size(40.0, 20.0)
                    .bg(GREEN)
                    .on_click("ok")
                    .focusable(),
            );
        });
        ui.leaf_keyed("tip", float_at(150.0, BLUE));
        ui.finish();
    };
    build(&mut core);
    let ok = core.key_of("ok").unwrap();
    core.set_focus(Some(ok));
    core.handle_input(InputEvent::Key(EditKey::Tab, Mods::default()));
    core.set_focus(Some(ok));
    build(&mut core);
    assert_eq!(painted(&mut core), ["red", "green", "ring", "blue"]);
}

/// A scroller inside a float: its bar ends the float's layer, over the
/// float's rows and under the next float.
#[test]
fn a_scroller_inside_a_float_bars_at_that_layers_end() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("menu", float_at(100.0, RED).scroll_y(), |ui| {
        for i in 0..10 {
            ui.leaf_keyed(
                &format!("row{i}"),
                NodeSpec::row().grow_width().height(30.0).bg(GREY),
            );
        }
    });
    ui.leaf_keyed("tip", float_at(150.0, BLUE));
    ui.finish();
    let order = painted(&mut core);
    let bar = order.iter().position(|n| *n == "bar").unwrap();
    let last_row = order.iter().rposition(|n| *n == "grey").unwrap();
    let tip = order.iter().position(|n| *n == "blue").unwrap();
    assert!(last_row < bar && bar < tip, "{order:?}");
}

/// A float inside a float is a layer of its own, above the one it is in —
/// opened the same frame (tree order) or later (on top either way).
#[test]
fn a_nested_float_is_above_the_float_it_is_in() {
    let mut core = Core::new();
    let build = |core: &mut Core, sub: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("menu", float_at(100.0, RED), |ui| {
            if sub {
                ui.leaf_keyed("submenu", float_at(120.0, BLUE));
            }
            ui.leaf_keyed("item", NodeSpec::row().size(40.0, 20.0).bg(GREEN));
        });
        ui.finish();
    };
    build(&mut core, true);
    assert_eq!(
        painted(&mut core),
        ["red", "green", "blue"],
        "the submenu is not painted where it sits in the menu's subtree"
    );
    build(&mut core, false);
    build(&mut core, true);
    assert_eq!(painted(&mut core), ["red", "green", "blue"]);
}

/// A box that keeps its key while it becomes a float, around a float it
/// already held: the outer float is new to the stack and the inner one is
/// not, and the inner one used to stay below it — painted under its own
/// parent, and the nested-above-its-float assertion tripped in a debug
/// build. Seen in berainder, a card stacked behind the top one becoming
/// the top card, its name panel a float inside it (backlog RG151).
#[test]
fn a_box_that_becomes_a_float_stays_under_the_float_it_held() {
    let mut core = Core::new();
    let build = |core: &mut Core, floating: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let card = if floating {
            float_at(100.0, RED)
        } else {
            NodeSpec::column().size(200.0, 100.0).bg(RED)
        };
        ui.with_keyed("card", card, |ui| {
            ui.leaf_keyed("name", float_at(120.0, BLUE));
            ui.leaf_keyed("photo", NodeSpec::row().size(40.0, 20.0).bg(GREEN));
        });
        ui.finish();
    };
    build(&mut core, false);
    assert_eq!(painted(&mut core), ["red", "green", "blue"]);
    build(&mut core, true);
    assert_eq!(
        painted(&mut core),
        ["red", "green", "blue"],
        "the name panel is above the card that became a float"
    );
    build(&mut core, true);
    assert_eq!(
        painted(&mut core),
        ["red", "green", "blue"],
        "and stays there"
    );
}

/// Two floats held by the box that becomes a float keep their order over
/// it: the second, opened over the first, stays over it. Moving each up
/// to just above the box in turn put the first on top (backlog RG153,
/// from the alpha.44 pre-tag pass).
#[test]
fn two_floats_held_by_a_box_that_becomes_a_float_keep_their_order() {
    let mut core = Core::new();
    let build = |core: &mut Core, floating: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let card = if floating {
            float_at(100.0, RED)
        } else {
            NodeSpec::column().size(200.0, 100.0).bg(RED)
        };
        ui.with_keyed("card", card, |ui| {
            ui.leaf_keyed("name", float_at(120.0, BLUE));
            ui.leaf_keyed("tip", float_at(130.0, GREEN));
        });
        ui.finish();
    };
    build(&mut core, false);
    assert_eq!(painted(&mut core), ["red", "blue", "green"]);
    build(&mut core, true);
    assert_eq!(
        painted(&mut core),
        ["red", "blue", "green"],
        "the tip, opened over the name, stays over it"
    );
    build(&mut core, true);
    assert_eq!(painted(&mut core), ["red", "blue", "green"]);
}

/// A float that moves into another float under a key the app keeps
/// (`leaf_key`): nothing opened or closed and every rank is where it was,
/// so the steady order would have left it under the float it is now in —
/// the box-to-float case's assertion, by another road (backlog RG153,
/// from the alpha.44 pre-tag pass).
#[test]
fn a_float_moved_into_a_float_under_its_own_key_goes_above_it() {
    let mut core = Core::new();
    let tip = kui_core::Key::ROOT.str("tip");
    // The tip alone; then a panel opened before it in tree order, so the
    // tip (opened first) is under it; then the tip declared inside the
    // panel, at the same ranks.
    let build = |core: &mut Core, panel: bool, inside: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        if panel {
            ui.with_keyed("panel", float_at(100.0, RED), |ui| {
                if inside {
                    ui.leaf_key(tip, float_at(120.0, BLUE));
                }
            });
        }
        if !inside {
            ui.leaf_key(tip, float_at(120.0, BLUE));
        }
        ui.finish();
    };
    build(&mut core, false, false);
    assert_eq!(painted(&mut core), ["blue"]);
    build(&mut core, true, false);
    assert_eq!(
        painted(&mut core),
        ["blue", "red"],
        "the panel opened over the tip"
    );
    build(&mut core, true, true);
    assert_eq!(
        painted(&mut core),
        ["red", "blue"],
        "the tip is above the panel it moved into"
    );
    build(&mut core, true, true);
    assert_eq!(painted(&mut core), ["red", "blue"]);
}

/// A float from outside the modal's scope that opens over it, holding a
/// control, is the same inert-over-interactive surface
/// `modal-behind-content` names for an in-flow modal. A picture over it
/// — a HUD, an inspector's outline — is not: nothing there could have
/// been pressed, so nothing reads as broken.
#[test]
fn a_float_over_a_modal_from_outside_it_warns_when_it_holds_a_control() {
    let mut core = Core::new();
    let build = |core: &mut Core, hud: bool, button: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.leaf_keyed(
            "dialog",
            float_at(100.0, RED).modal("dialog").label("Dialog"),
        );
        if hud {
            ui.with_keyed("hud", float_at(150.0, BLUE), |ui| {
                if button {
                    ui.leaf_keyed(
                        "close",
                        NodeSpec::row()
                            .size(20.0, 20.0)
                            .on_click("close")
                            .label("Close"),
                    );
                }
            });
        }
        ui.finish();
    };
    build(&mut core, false, false);
    assert!(core.take_warnings().is_empty());
    build(&mut core, true, false);
    assert!(core.take_warnings().is_empty(), "a picture over the dialog");
    build(&mut core, true, true);
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(codes, ["modal-behind-content"], "a control over it");
}

/// Every quad of the last frame in paint order, the strokes named too:
/// `painted` with `"stroke"` for a segment, since a polyline is what a
/// glyph drawn into a title bar is made of.
fn painted_with_strokes(core: &mut Core) -> Vec<&'static str> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::Solid | QuadKind::Segment))
        .map(|q| {
            if q.kind == QuadKind::Segment {
                "stroke"
            } else if q.color == RED {
                "red"
            } else if q.color == BLUE {
                "blue"
            } else if q.color == GREY {
                "grey"
            } else {
                "?"
            }
        })
        .collect()
}

/// A toast at the viewport's top-right, open since the first frame, and
/// a bar under it that draws a glyph — a polyline — from the second frame
/// on, as a pane opened after the toast draws its title bar's key cap.
/// `viewport` anchors the glyph to the viewport instead.
fn toast_and_bar(core: &mut Core, glyph: bool, viewport: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "bar",
        NodeSpec::row().grow_width().height(24.0).bg(GREY),
        |ui| {
            if glyph {
                let mut spec = NodeSpec::row();
                if viewport {
                    spec = spec.float(FloatConfig::viewport());
                }
                ui.polyline_keyed(
                    "glyph",
                    &[Vec2::new(300.0, 4.0), Vec2::new(316.0, 20.0)],
                    kui_core::Stroke::new(2.0, GREEN),
                    spec,
                );
            }
        },
    );
    ui.leaf_keyed("toast", float_at(200.0, RED));
    ui.finish();
}

/// A stroke in its parent's box space is the parent's content, so it
/// paints in the parent's layer at its place in the tree — a glyph drawn
/// into a bar under a toast that opened a frame earlier goes under the
/// toast, as the bar does (backlog F123, from kawoosh: a title bar's key
/// cap drawn over the update toast). Only a stroke that escapes, anchored
/// to the viewport, opens a layer of its own.
#[test]
fn a_stroke_paints_in_its_parents_layer_not_in_one_of_its_own() {
    let mut core = Core::new();
    toast_and_bar(&mut core, false, false);
    assert_eq!(painted_with_strokes(&mut core), ["grey", "red"]);
    toast_and_bar(&mut core, true, false);
    assert_eq!(
        painted_with_strokes(&mut core),
        ["grey", "stroke", "red"],
        "the glyph came a frame after the toast and is still under it"
    );
    toast_and_bar(&mut core, true, false);
    assert_eq!(painted_with_strokes(&mut core), ["grey", "stroke", "red"]);

    let mut core = Core::new();
    toast_and_bar(&mut core, false, true);
    toast_and_bar(&mut core, true, true);
    assert_eq!(
        painted_with_strokes(&mut core),
        ["grey", "red", "stroke"],
        "a viewport-anchored stroke is a layer, opened after the toast's"
    );
}

/// Two floats open together; a frame later the first gains a stroke. The
/// stroke is in the first float's layer, under the second — not a third
/// layer on top of both.
#[test]
fn a_stroke_inside_a_float_paints_in_that_floats_layer() {
    fn frame(core: &mut Core, stroke: bool) {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed("first", float_at(100.0, RED), |ui| {
            if stroke {
                ui.line(
                    Vec2::new(10.0, 10.0),
                    Vec2::new(190.0, 90.0),
                    kui_core::Stroke::new(2.0, GREEN),
                    NodeSpec::row(),
                );
            }
        });
        ui.leaf_keyed("second", float_at(150.0, BLUE));
        ui.finish();
    }
    let mut core = Core::new();
    frame(&mut core, false);
    assert_eq!(painted_with_strokes(&mut core), ["red", "blue"]);
    frame(&mut core, true);
    assert_eq!(painted_with_strokes(&mut core), ["red", "stroke", "blue"]);
}

/// Every quad of the last frame in paint order, each kind named: a solid
/// by its colour, a segment `stroke`, a polygon's fragment quad `polygon`,
/// a path's mask quad `path`; the rest left out.
fn painted_shapes(core: &mut Core) -> Vec<&'static str> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter_map(|q| match q.kind {
            QuadKind::Solid => Some(if q.color == RED {
                "red"
            } else if q.color == BLUE {
                "blue"
            } else if q.color == GREY {
                "grey"
            } else {
                "?"
            }),
            QuadKind::Segment => Some("stroke"),
            QuadKind::Fragment => Some("polygon"),
            QuadKind::GlyphMask => Some("path"),
            _ => None,
        })
        .collect()
}

/// F123's rule is for the three kinds the core floats for the room alone:
/// a `polygon` and a `path` drawn into the bar a frame after the toast
/// opened are under it, as the `line` is.
#[test]
fn a_polygon_and_a_path_paint_in_their_parents_layer_as_a_line_does() {
    fn frame(core: &mut Core, shapes: bool) {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "bar",
            NodeSpec::row().grow_width().height(24.0).bg(GREY),
            |ui| {
                if shapes {
                    ui.polygon_keyed(
                        "wedge",
                        &[
                            Vec2::new(300.0, 4.0),
                            Vec2::new(316.0, 4.0),
                            Vec2::new(308.0, 20.0),
                        ],
                        NodeSpec::row().bg(GREEN),
                    );
                    ui.path_d_keyed(
                        "icon",
                        "M 330 4 L 346 4 L 338 20 Z",
                        kui_core::FillRule::NonZero,
                        None,
                        None,
                        NodeSpec::row().bg(GREEN),
                    );
                }
            },
        );
        ui.leaf_keyed("toast", float_at(200.0, RED));
        ui.finish();
    }
    let mut core = Core::new();
    frame(&mut core, false);
    assert_eq!(painted_shapes(&mut core), ["grey", "red"]);
    frame(&mut core, true);
    assert_eq!(
        painted_shapes(&mut core),
        ["grey", "polygon", "path", "red"],
        "both came a frame after the toast and are under it"
    );
}

/// A stroke that departs keeps the place it painted in: its ghost is in
/// the bar's layer under the toast, not in a layer of its own on top
/// (`PaintOrder::of` reads `Tree::opens_layer` as the live pass does).
#[test]
fn a_departing_stroke_is_replayed_in_its_parents_layer() {
    fn frame(core: &mut Core, now: f64, glyph: bool) {
        core.set_time(now);
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "bar",
            NodeSpec::row().grow_width().height(24.0).bg(GREY),
            |ui| {
                if glyph {
                    ui.line_keyed(
                        "glyph",
                        Vec2::new(300.0, 4.0),
                        Vec2::new(316.0, 20.0),
                        kui_core::Stroke::new(2.0, GREEN),
                        NodeSpec::row()
                            .transition(100.0)
                            .exit(kui_core::Enter::default().opacity(0.0)),
                    );
                }
            },
        );
        ui.leaf_keyed("toast", float_at(200.0, RED));
        ui.finish();
    }
    let mut core = Core::new();
    frame(&mut core, 0.0, false);
    frame(&mut core, 0.0, true);
    assert_eq!(painted_with_strokes(&mut core), ["grey", "stroke", "red"]);
    frame(&mut core, 0.01, false);
    assert!(core.animating(), "the exit is playing");
    assert_eq!(
        painted_with_strokes(&mut core),
        ["grey", "stroke", "red"],
        "the ghost keeps the stroke's place under the toast"
    );
    frame(&mut core, 0.2, false);
    assert_eq!(painted_with_strokes(&mut core), ["grey", "red"]);
}

/// Only the core's own floats lost their layer: a float a view declared
/// with `clip` and a parent anchor (F90) is cut by the parent and still a
/// layer of its own, over the in-flow sibling declared after it.
#[test]
fn a_declared_float_with_clip_is_still_a_layer_of_its_own() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed("canvas", NodeSpec::column().fill().clip().bg(GREY), |ui| {
        ui.leaf_keyed(
            "node",
            NodeSpec::column()
                .size(80.0, 40.0)
                .bg(RED)
                .float(FloatConfig::parent().clipped().offset(10.0, 10.0)),
        );
        ui.leaf_keyed("sheet", NodeSpec::column().size(200.0, 200.0).bg(BLUE));
    });
    ui.finish();
    assert_eq!(
        painted(&mut core),
        ["grey", "blue", "red"],
        "the declared float paints after the sibling declared after it"
    );
}

/// A stroke in its parent's box space is held by the parent's clip (F78);
/// one anchored to the viewport still escapes it, and that is the one
/// that is a layer.
#[test]
fn a_viewport_anchored_stroke_still_escapes_its_parents_clip() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(NodeSpec::column().size(50.0, 50.0).clip(), |ui| {
        ui.line(
            Vec2::new(0.0, 25.0),
            Vec2::new(300.0, 25.0),
            kui_core::Stroke::new(2.0, GREEN),
            NodeSpec::row(),
        );
        ui.line(
            Vec2::new(0.0, 150.0),
            Vec2::new(300.0, 150.0),
            kui_core::Stroke::new(2.0, GREEN),
            NodeSpec::row().float(FloatConfig::viewport()),
        );
    });
    ui.finish();
    let (dl, _) = core.output();
    let clips: Vec<kui_core::Rect> = dl
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::Segment)
        .map(|q| dl.clips[q.clip as usize].rect)
        .collect();
    assert_eq!(clips.len(), 2, "both strokes draw");
    assert_eq!(
        (clips[0].w, clips[0].h),
        (50.0, 50.0),
        "held by the box's clip: {:?}",
        clips[0]
    );
    assert!(
        clips[1].contains(Vec2::new(150.0, 150.0)),
        "escaped the box: {:?}",
        clips[1]
    );
}

/// Input reads the stack as paint does (ADR 0023): a clickable stroke is
/// hit at its place among its siblings — under a card declared after it,
/// over one declared before — where it was topmost over every in-flow
/// node as a layer of its own.
#[test]
fn a_stroke_is_hit_at_its_place_among_its_siblings() {
    fn frame(core: &mut Core, stroke_first: bool) {
        let mut ui = core.frame(VIEW, 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.with_keyed(
            "bar",
            NodeSpec::row().grow_width().height(24.0).bg(GREY),
            |ui| {
                let glyph = |ui: &mut kui_core::Ui<'_>| {
                    ui.line_keyed(
                        "glyph",
                        Vec2::new(0.0, 12.0),
                        Vec2::new(100.0, 12.0),
                        kui_core::Stroke::new(4.0, GREEN),
                        NodeSpec::row().on_click("glyph"),
                    );
                };
                let card = |ui: &mut kui_core::Ui<'_>| {
                    ui.leaf_keyed(
                        "card",
                        NodeSpec::column()
                            .size(60.0, 24.0)
                            .bg(BLUE)
                            .on_click("card"),
                    );
                };
                if stroke_first {
                    glyph(ui);
                    card(ui);
                } else {
                    card(ui);
                    glyph(ui);
                }
            },
        );
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
    let mut core = Core::new();
    frame(&mut core, true);
    assert_eq!(painted_with_strokes(&mut core), ["grey", "stroke", "blue"]);
    assert_eq!(
        click_at(&mut core, 30.0, 12.0).as_deref(),
        Some("card"),
        "the card declared after the stroke is over it"
    );
    assert_eq!(click_at(&mut core, 80.0, 12.0).as_deref(), Some("glyph"));

    let mut core = Core::new();
    frame(&mut core, false);
    assert_eq!(painted_with_strokes(&mut core), ["grey", "blue", "stroke"]);
    assert_eq!(
        click_at(&mut core, 30.0, 12.0).as_deref(),
        Some("glyph"),
        "declared after the card, the stroke is over it"
    );
}

/// An image fills its box, so a border painted with the box's background,
/// under the content, was covered by the picture: `border` on an image
/// drew nothing. The border is a ring over the image, as it is over a
/// gradient; the background stays under it (backlog RG152, from
/// berainder's match screen).
#[test]
fn an_images_border_is_drawn_over_the_picture() {
    let mut core = Core::new();
    let img = core.resources.add_image(2, 2, vec![0xff; 16]);
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.image(
        img,
        NodeSpec::column()
            .size(80.0, 80.0)
            .bg(GREY)
            .radius(40.0)
            .border(3.0, RED),
    );
    ui.finish();
    let (dl, _) = core.output();
    let kinds: Vec<(QuadKind, bool)> = dl
        .quads
        .iter()
        .map(|q| (q.kind, q.border_w > 0.0 && q.border_color == RED))
        .collect();
    let image = kinds.iter().position(|(k, _)| *k == QuadKind::Image);
    let ring = kinds.iter().position(|(_, ringed)| *ringed);
    assert!(
        matches!((image, ring), (Some(i), Some(r)) if r > i),
        "the border after the picture: {kinds:?}"
    );
    assert_eq!(
        kinds.iter().filter(|(_, ringed)| *ringed).count(),
        1,
        "drawn once: {kinds:?}"
    );
    let bg = dl.quads.iter().position(|q| q.color == GREY).unwrap();
    assert!(bg < image.unwrap(), "the background stays under it");
}
