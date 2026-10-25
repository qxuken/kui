//! A press on a node that does nothing with one goes to the nearest
//! ancestor that does (backlog C91): a child that declares only a hover,
//! a cursor or a hover group tracks the pointer, so it is the topmost
//! region under it, and it took the press its clickable parent never saw.
//! Hover composes as it always did; the press now composes the same way.

use kui_core::testing::{click_at, press, release};
use kui_core::{Color, Core, CursorShape, Key, NodeSpec, Role, Size, Value, Vec2};

fn lit() -> Color {
    Color::hex(0x3366ccff)
}

/// A 200x100 card that clicks, holding a 40x40 badge at its top left that
/// declares only a hover colour and a cursor. `card` is what the card
/// declares beside its size.
fn frame(core: &mut Core, card: impl FnOnce(NodeSpec) -> NodeSpec) -> (Key, Key) {
    let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut badge = Key::ROOT;
    let card = ui.with_keyed("card", card(NodeSpec::row().size(200.0, 100.0)), |ui| {
        badge = ui.leaf_keyed(
            "badge",
            NodeSpec::row()
                .size(40.0, 40.0)
                .hover_bg(lit())
                .cursor(CursorShape::Pointer),
        );
    });
    ui.finish();
    (card, badge)
}

fn clicks(events: &[kui_core::UiEvent]) -> Vec<&Value> {
    events
        .iter()
        .filter(|e| e.event.name() == "click")
        .map(|e| &e.payload)
        .collect()
}

#[test]
fn a_press_on_a_hover_only_child_clicks_its_parent() {
    let mut core = Core::new();
    let (card, badge) = frame(&mut core, |s| s.on_click("card"));
    let events = click_at(&mut core, 20.0, 20.0);
    let got = clicks(&events);
    assert_eq!(got.len(), 1, "the card heard it: {events:?}");
    assert_eq!(events[events.len() - 1].key, card);
    // Hover is the child's, as it was.
    assert!(core.interaction.is_hovered(badge));
}

#[test]
fn both_show_pressed_while_held_and_a_release_off_both_clicks_nothing() {
    let mut core = Core::new();
    let (card, badge) = frame(&mut core, |s| s.on_click("card"));
    press(&mut core, Vec2::new(20.0, 20.0));
    assert!(core.interaction.is_pressed(card), "the card is pressed");
    assert!(
        core.interaction.is_pressed(badge),
        "and so is the badge under the pointer"
    );
    // Onto the card's own room: still the card's press.
    core.handle_input(kui_core::InputEvent::CursorMoved(Vec2::new(150.0, 50.0)));
    assert!(core.interaction.is_pressed(card));
    assert!(!core.interaction.is_pressed(badge));
    // Off both: the release clicks nothing.
    core.handle_input(kui_core::InputEvent::CursorMoved(Vec2::new(250.0, 150.0)));
    let events = release(&mut core);
    assert!(clicks(&events).is_empty(), "{events:?}");
}

#[test]
fn a_slider_track_takes_the_press_its_handle_lands() {
    let mut core = Core::new();
    let (track, _) = frame(&mut core, |s| {
        s.role(Role::Slider)
            .label("volume")
            .value_now(0.0)
            .value_min(0.0)
            .value_max(1.0)
            .on_change("vol")
    });
    let events = press(&mut core, Vec2::new(20.0, 20.0));
    let change = events
        .iter()
        .find(|e| e.event.name() == "change")
        .unwrap_or_else(|| panic!("the track proposed a value: {events:?}"));
    assert_eq!(change.key, track);
    release(&mut core);
}

#[test]
fn a_hover_only_child_of_a_focusable_node_focuses_it() {
    let mut core = Core::new();
    let (card, _) = frame(&mut core, |s| s.focusable().label("card"));
    click_at(&mut core, 20.0, 20.0);
    assert_eq!(core.focus(), Some(card));
}

/// A disabled control keeps a press it does nothing with: the badge in a
/// disabled button inside a clickable panel clicks neither.
#[test]
fn a_disabled_ancestor_swallows_the_press() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "panel",
        NodeSpec::column().size(300.0, 200.0).on_click("panel"),
        |ui| {
            ui.with_keyed(
                "button",
                NodeSpec::row()
                    .size(200.0, 100.0)
                    .on_click("button")
                    .disabled(true),
                |ui| {
                    ui.leaf_keyed("badge", NodeSpec::row().size(40.0, 40.0).hover_bg(lit()));
                },
            );
        },
    );
    ui.finish();
    let events = click_at(&mut core, 20.0, 20.0);
    assert!(clicks(&events).is_empty(), "{events:?}");
}

/// With nothing above that takes a press, the child keeps it, and the
/// press does what it did: nothing, focus dropped.
#[test]
fn with_nothing_above_the_child_keeps_the_press() {
    let mut core = Core::new();
    let (card, _) = frame(&mut core, |s| s);
    let events = click_at(&mut core, 20.0, 20.0);
    assert!(clicks(&events).is_empty());
    assert_eq!(core.focus(), None);
    assert_ne!(core.focus(), Some(card));
}
