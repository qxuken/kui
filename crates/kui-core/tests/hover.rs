//! Hover tracking without clickability (`NodeSpec::hoverable`) — through a
//! live `Core`, including the badge-inside-a-floating-HUD arrangement that
//! motivated the flag.

use kui_core::{Align, Core, FloatConfig, InputEvent, Key, NodeSpec, Size, Value, Vec2};

fn frame(core: &mut Core, badge_spec: NodeSpec) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let mut badge = Key(0);
    ui.with(NodeSpec::column(), |ui| {
        badge = ui.child_key("badge");
        ui.leaf_keyed("badge", badge_spec.size(20.0, 20.0));
    });
    ui.finish();
    badge
}

#[test]
fn hoverable_tracks_hover_but_click_emits_nothing() {
    let mut core = Core::new();
    let badge = frame(&mut core, NodeSpec::column().hoverable());

    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(
        core.is_hovered(badge),
        "hoverable node must register a hit region"
    );

    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    assert!(
        evs.is_empty(),
        "hover-only region must not emit click events"
    );

    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 200.0)));
    assert!(!core.is_hovered(badge));
}

#[test]
fn plain_node_is_not_hover_tracked() {
    let mut core = Core::new();
    let badge = frame(&mut core, NodeSpec::column());
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(!core.is_hovered(badge));
}

#[test]
fn on_click_still_emits_and_hover_tracks() {
    let mut core = Core::new();
    let badge = frame(&mut core, NodeSpec::column().on_click("hit"));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(core.is_hovered(badge));
    core.handle_input(InputEvent::mouse_down(1));
    let evs = core.handle_input(InputEvent::mouse_up());
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.as_str(), Some("hit"));
}

#[test]
fn hoverable_badge_inside_viewport_float_hovers() {
    // The latency-HUD arrangement: a hoverable badge nested in a
    // viewport-anchored float pinned to the bottom-right corner.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let mut badge = Key(0);
    ui.with(
        NodeSpec::column()
            .float(FloatConfig::viewport().inside(Align::End, Align::End))
            .pad(10.0),
        |ui| {
            badge = ui.child_key("badge");
            ui.leaf_keyed("badge", NodeSpec::column().size(20.0, 20.0).hoverable());
        },
    );
    ui.finish();
    // Float is 40x40 at (360,260); the badge sits inside its padding.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(380.0, 280.0)));
    assert!(
        core.is_hovered(badge),
        "hoverable inside a float subtree must hit-test"
    );
}

// ---------------------------------------------------------------------------
// Declarative hover styling (`hover_bg` / `pressed_bg` / `hover_group`) and
// `on_hover` events — the core resolves the state, views stay data-only.

use kui_core::Color;

const BASE: Color = Color {
    r: 0.1,
    g: 0.2,
    b: 0.3,
    a: 1.0,
};
const HOVER: Color = Color {
    r: 0.4,
    g: 0.5,
    b: 0.6,
    a: 1.0,
};
const PRESSED: Color = Color {
    r: 0.7,
    g: 0.8,
    b: 0.9,
    a: 1.0,
};

/// Two 50x50 blocks side by side; each gets `spec` (plus a base bg and
/// fixed size). Returns their keys.
fn pair_frame(core: &mut Core, spec: impl Fn(&str) -> NodeSpec) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let (mut a, mut b) = (Key(0), Key(0));
    for (label, out) in [("a", &mut a), ("b", &mut b)] {
        *out = ui.child_key(label);
        ui.leaf_keyed(label, spec(label).bg(BASE).size(50.0, 50.0));
    }
    ui.finish();
    (a, b)
}

/// The bg quad color of the block at `x` (its top-left corner).
fn bg_at(core: &mut Core, x: f32) -> Color {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| q.rect.x == x && q.rect.w == 50.0)
        .map(|q| q.color)
        .expect("block quad")
}

#[test]
fn hover_bg_and_pressed_bg_resolve_in_the_core() {
    let mut core = Core::new();
    let spec = |_: &str| NodeSpec::column().hover_bg(HOVER).pressed_bg(PRESSED);
    pair_frame(&mut core, spec);
    assert_eq!(bg_at(&mut core, 0.0), BASE);

    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    pair_frame(&mut core, spec);
    assert_eq!(bg_at(&mut core, 0.0), HOVER, "hovered block takes hover_bg");
    assert_eq!(bg_at(&mut core, 50.0), BASE, "its neighbour does not");

    core.handle_input(InputEvent::mouse_down(1));
    pair_frame(&mut core, spec);
    assert_eq!(
        bg_at(&mut core, 0.0),
        PRESSED,
        "pressed block takes pressed_bg"
    );

    core.handle_input(InputEvent::mouse_up());
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 200.0)));
    pair_frame(&mut core, spec);
    assert_eq!(bg_at(&mut core, 0.0), BASE, "back to the plain bg");
}

#[test]
fn hover_group_lights_every_member() {
    let mut core = Core::new();
    let spec = |_: &str| {
        NodeSpec::column()
            .hover_bg(HOVER)
            .pressed_bg(PRESSED)
            .hover_group("pair")
    };
    pair_frame(&mut core, spec);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    pair_frame(&mut core, spec);
    assert_eq!(bg_at(&mut core, 0.0), HOVER);
    assert_eq!(bg_at(&mut core, 50.0), HOVER, "the other member hovers too");

    core.handle_input(InputEvent::mouse_down(1));
    pair_frame(&mut core, spec);
    assert_eq!(
        bg_at(&mut core, 50.0),
        PRESSED,
        "the group presses together"
    );

    // Sliding from one member onto the other keeps the group pressed.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, 10.0)));
    pair_frame(&mut core, spec);
    assert_eq!(bg_at(&mut core, 0.0), PRESSED);
    assert_eq!(bg_at(&mut core, 50.0), PRESSED);

    // A member with no colors of its own still drives the group.
    let mixed = |label: &str| {
        let s = NodeSpec::column().hover_group("pair");
        if label == "a" { s.hover_bg(HOVER) } else { s }
    };
    core.handle_input(InputEvent::mouse_up());
    core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, 10.0)));
    pair_frame(&mut core, mixed);
    pair_frame(&mut core, mixed);
    assert_eq!(bg_at(&mut core, 0.0), HOVER, "hovering b lights a");
    assert_eq!(bg_at(&mut core, 50.0), BASE, "b declared no hover color");
}

#[test]
fn hover_group_id_is_stable_and_named() {
    assert_eq!(
        NodeSpec::hover_group_id("pair"),
        NodeSpec::column()
            .hover_group("pair")
            .interact()
            .hover_group
            .unwrap()
    );
    assert_ne!(NodeSpec::hover_group_id("a"), NodeSpec::hover_group_id("b"));
}

fn hover_phases(evs: &[kui_core::UiEvent]) -> Vec<(Key, String)> {
    evs.iter()
        .filter(|e| e.kind() == Some("hover"))
        .map(|e| (e.key, e.payload.get_str("phase").unwrap_or("?").to_string()))
        .collect()
}

#[test]
fn on_hover_emits_enter_and_leave_with_tag() {
    let mut core = Core::new();
    let spec = |label: &str| NodeSpec::column().on_hover(Value::str(label));
    let (a, b) = pair_frame(&mut core, spec);

    let enter = core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert_eq!(hover_phases(&enter), [(a, "enter".to_string())]);
    assert_eq!(enter[0].payload.get_str("tag"), Some("a"));

    // Moving within the node is silent; crossing to the neighbour leaves
    // then enters.
    assert!(
        core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 20.0)))
            .is_empty()
    );
    let cross = core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, 10.0)));
    assert_eq!(
        hover_phases(&cross),
        [(a, "leave".to_string()), (b, "enter".to_string())]
    );

    let leave = core.handle_input(InputEvent::CursorLeft);
    assert_eq!(hover_phases(&leave), [(b, "leave".to_string())]);
}

#[test]
fn on_hover_fires_when_a_frame_moves_a_node_under_a_still_cursor() {
    let mut core = Core::new();
    let (a, _) = pair_frame(&mut core, |l| NodeSpec::column().on_hover(Value::str(l)));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(core.take_pending_events().is_empty());

    // Next frame: nothing hover-tracked at the cursor.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.leaf(NodeSpec::column());
    ui.finish();
    let pending = core.take_pending_events();
    assert_eq!(hover_phases(&pending), [(a, "leave".to_string())]);

    // Left un-drained, they ride along with the next input instead.
    pair_frame(&mut core, |l| NodeSpec::column().on_hover(Value::str(l)));
    let next = core.handle_input(InputEvent::mouse_down(1));
    assert_eq!(hover_phases(&next), [(a, "enter".to_string())]);
}

#[test]
fn hover_props_alone_make_a_node_hover_tracked() {
    let mut core = Core::new();
    let badge = frame(&mut core, NodeSpec::column().hover_bg(HOVER));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(core.is_hovered(badge));
    let badge = frame(&mut core, NodeSpec::column().hover_group("g"));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(core.is_hovered(badge));
    assert!(core.is_group_hovered(NodeSpec::hover_group_id("g")));
}

/// A window-drag strip hears hover like any node (backlog W23): a chrome
/// node's press becomes a window command, its hover does not, so a
/// toolbar that comes back when the pointer reaches the titlebar can be
/// built on the strip itself.
#[test]
fn a_window_drag_strip_hears_hover() {
    let mut core = kui_core::Core::new();
    let frame = |core: &mut kui_core::Core| {
        let mut ui = core.frame(kui_core::Size::new(400.0, 300.0), 1.0);
        ui.configure_root(kui_core::NodeSpec::column().fill());
        ui.leaf_keyed(
            "strip",
            kui_core::NodeSpec::row()
                .grow_width()
                .height(52.0)
                .window_drag()
                .on_hover("strip"),
        );
        ui.finish();
    };
    frame(&mut core);
    frame(&mut core);
    let evs = core.handle_input(kui_core::InputEvent::CursorMoved(kui_core::Vec2::new(
        100.0, 20.0,
    )));
    let hover = evs
        .iter()
        .find(|e| e.kind() == Some("hover"))
        .expect("the strip heard the pointer arrive");
    assert_eq!(hover.payload.get_str("phase"), Some("enter"));
    assert_eq!(hover.payload.get_str("tag"), Some("strip"));
}
