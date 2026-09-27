//! The paint order as a stack of layers, and the input that reads it
//! (`docs/adr/0023-layers-stack-in-the-order-they-open.md`): the in-flow
//! tree, then one layer per float in the order they opened, each with its
//! own chrome — scrollbars, the ring — at its end. Pinned by quad index,
//! since the corpus digest carries order but is not checked in.

use kui_core::{
    Align, Color, Core, CursorShape, EditKey, FloatConfig, InputEvent, Mods, MouseButton, NodeSpec,
    QuadKind, Size, Sizing, Value, Vec2,
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
    NodeSpec::column()
        .width(Sizing::Fixed(200.0))
        .height(Sizing::Fixed(100.0))
        .bg(bg)
        .float(
            FloatConfig::viewport()
                .at(Align::Start, Align::Start)
                .self_at(Align::Start, Align::Start)
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
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(30.0))
                    .bg(GREY),
            );
        }
    });
    let mut spec = float_at(250.0, RED)
        .on_click(Value::str("menu"))
        .cursor(CursorShape::Pointer);
    if modal {
        spec = spec.modal(Value::str("menu"));
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
    ui.with_keyed("bar", NodeSpec::row().height(Sizing::Fixed(24.0)), |ui| {
        if dropdown {
            ui.leaf_keyed("dropdown", float_at(100.0, RED));
        }
    });
    ui.leaf_keyed(
        "btn",
        NodeSpec::row()
            .width(Sizing::Fixed(80.0))
            .height(Sizing::Fixed(30.0))
            .bg(GREEN)
            .on_click(Value::str("btn"))
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
            ui.leaf_keyed(
                "dropdown",
                float_at(100.0, RED).on_click(Value::str("dropdown")),
            );
        }
        ui.leaf_keyed("tip", float_at(150.0, BLUE).on_click(Value::str("tip")));
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
                    .width(Sizing::Fixed(40.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(GREEN)
                    .on_click(Value::str("ok"))
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
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(30.0))
                    .bg(GREY),
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
            ui.leaf_keyed(
                "item",
                NodeSpec::row()
                    .width(Sizing::Fixed(40.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(GREEN),
            );
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
            float_at(100.0, RED)
                .modal(Value::str("dialog"))
                .label("Dialog"),
        );
        if hud {
            ui.with_keyed("hud", float_at(150.0, BLUE), |ui| {
                if button {
                    ui.leaf_keyed(
                        "close",
                        NodeSpec::row()
                            .width(Sizing::Fixed(20.0))
                            .height(Sizing::Fixed(20.0))
                            .on_click(Value::str("close"))
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
