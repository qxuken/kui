//! Hover tracking without clickability (`NodeSpec::hoverable`) — through a
//! live `Core`, including the badge-inside-a-floating-HUD arrangement that
//! motivated the flag.

use kui_core::{Align, Core, FloatConfig, InputEvent, Key, NodeSpec, Size, Sizing, Value, Vec2};

fn frame(core: &mut Core, badge_spec: NodeSpec) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let mut badge = Key(0);
    ui.with(NodeSpec::column(), |ui| {
        badge = ui.child_key("badge");
        ui.with_keyed(
            "badge",
            badge_spec
                .width(Sizing::Fixed(20.0))
                .height(Sizing::Fixed(20.0)),
            |_| {},
        );
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

    core.handle_input(InputEvent::MouseDown(1));
    let evs = core.handle_input(InputEvent::MouseUp);
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
    let badge = frame(&mut core, NodeSpec::column().on_click(Value::str("hit")));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    assert!(core.is_hovered(badge));
    core.handle_input(InputEvent::MouseDown(1));
    let evs = core.handle_input(InputEvent::MouseUp);
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
            .float(
                FloatConfig::viewport()
                    .at(Align::End, Align::End)
                    .self_at(Align::End, Align::End),
            )
            .pad(10.0),
        |ui| {
            badge = ui.child_key("badge");
            ui.with_keyed(
                "badge",
                NodeSpec::column()
                    .width(Sizing::Fixed(20.0))
                    .height(Sizing::Fixed(20.0))
                    .hoverable(),
                |_| {},
            );
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
