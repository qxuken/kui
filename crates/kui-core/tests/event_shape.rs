//! What an event arrives as (ADR 0047): the message its handler declared,
//! with the event's data in one field named for the event, and its kind on
//! the envelope. A map message keeps its own fields beside that one; a
//! `Null` message is the field alone; any other message is kept under
//! `tag`; a plain click arrives exactly as declared; a field the message
//! already had is the core's, and the core says so. A window's own events
//! arrive as the messages its root declared in `windowEvents` (ADR 0048),
//! and as nothing when it declared none.

use kui_core::diag::MESSAGE_FIELD_TAKEN;
use kui_core::{
    Content, Core, Event, EventKind, InputEvent, NodeSpec, Size, UiEvent, Value, Vec2, WindowEvents,
};

/// One frame of a 100 px square at the origin declaring `spec`.
fn frame(core: &mut Core, spec: NodeSpec) {
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed("pad", spec.width(100.0).height(100.0));
    ui.finish();
}

/// A press at (10, 10) dragged to (60, 10), and every event it made.
fn drag(core: &mut Core) -> Vec<UiEvent> {
    let mut out = Vec::new();
    out.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0))));
    out.extend(core.handle_input(InputEvent::mouse_down(1)));
    out.extend(core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, 10.0))));
    out.extend(core.handle_input(InputEvent::mouse_up()));
    out
}

fn pan() -> Value {
    Value::map([("kind", Value::str("pan")), ("id", Value::Int(7))])
}

#[test]
fn a_map_message_keeps_its_fields_beside_the_event_s() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::column().on_drag(pan()));
    let evs = drag(&mut core);
    let moved = evs.iter().find(|e| e.is(EventKind::Drag)).expect("a drag");
    assert_eq!(moved.payload.get_str("kind"), Some("pan"), "the app's kind");
    assert_eq!(moved.payload.get_int("id"), Some(7), "its own field kept");
    let data = moved.payload.get("drag").expect("the data under `drag`");
    assert_eq!(data.get_str("phase"), Some("start"));
    assert_eq!(moved.data(), Some(data));
    // The message decodes as the app declared it; the extra field is not
    // read by a reader that names none.
    let msg: Value = moved.message().expect("the message");
    assert_eq!(msg.get_str("kind"), Some("pan"));
}

#[test]
fn a_null_message_is_the_field_alone_and_a_string_one_is_kept_under_tag() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::column().on_drag(Value::Null));
    let evs = drag(&mut core);
    let Value::Map(entries) = &evs[0].payload else {
        panic!("a map: {:?}", evs[0].payload);
    };
    let keys: Vec<_> = entries.iter().map(|(k, _)| k.as_str()).collect();
    assert_eq!(keys, ["drag"]);

    let mut core = Core::new();
    frame(&mut core, NodeSpec::column().on_drag("split"));
    let evs = drag(&mut core);
    assert_eq!(evs[0].payload.get_str("tag"), Some("split"));
    assert!(evs[0].payload.get("drag").is_some());
}

#[test]
fn a_plain_click_arrives_exactly_as_declared() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::column().on_click(pan()));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    let mut evs = core.handle_input(InputEvent::mouse_down(1));
    evs.extend(core.handle_input(InputEvent::mouse_up()));
    let click = evs
        .iter()
        .find(|e| e.is(EventKind::Click))
        .expect("a click");
    assert_eq!(click.payload, pan());
    assert_eq!(click.data(), None, "a plain click carries no data");
    assert_eq!(click.read(), Event::Click(Default::default()));
}

/// An event with no data leaves the message as declared, so a field the
/// app named like the event is the app's: not the event's data, and not
/// taken (the alpha.60 pre-tag pass, backlog RG157).
#[test]
fn a_dataless_event_leaves_the_app_s_field_of_its_name_alone() {
    let mut core = Core::new();
    let own = Value::map([("kind", Value::str("k")), ("click", Value::Int(5))]);
    frame(&mut core, NodeSpec::column().on_click(own.clone()));
    core.take_warnings();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    let mut evs = core.handle_input(InputEvent::mouse_down(1));
    evs.extend(core.handle_input(InputEvent::mouse_up()));
    let click = evs
        .iter()
        .find(|e| e.is(EventKind::Click))
        .expect("a click");
    assert_eq!(click.payload, own, "the message as declared");
    assert_eq!(
        click.data(),
        None,
        "the app's `click` is not the event's data"
    );
    assert_eq!(click.read(), Event::Click(Default::default()));
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(!codes.contains(&MESSAGE_FIELD_TAKEN), "{codes:?}");
}

#[test]
fn a_field_the_message_already_has_is_the_core_s_and_it_says_so() {
    let mut core = Core::new();
    let clash = Value::map([("kind", Value::str("k")), ("drag", Value::str("mine"))]);
    frame(&mut core, NodeSpec::column().on_drag(clash));
    core.take_warnings();
    let evs = drag(&mut core);
    let data = evs[0].payload.get("drag").expect("the field");
    assert!(data.get_str("phase").is_some(), "the event's data won");
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(
        codes.iter().filter(|c| **c == MESSAGE_FIELD_TAKEN).count(),
        1,
        "once for the node, not once per event: {codes:?}"
    );
}

/// One frame of the same root declaring `events` as its `windowEvents`.
fn frame_with(core: &mut Core, events: WindowEvents) {
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.window_events(events);
    ui.finish();
}

fn shift(core: &mut Core, held: bool) -> Vec<UiEvent> {
    core.handle_input(InputEvent::Modifiers(kui_core::KeyMods {
        shift: held,
        ..Default::default()
    }))
}

#[test]
fn a_window_event_arrives_as_the_message_the_root_declared() {
    let mut core = Core::new();
    let mods = Value::map([("kind", Value::str("mods"))]);
    frame_with(&mut core, WindowEvents::default().on_modifiers(mods));
    let evs = shift(&mut core, true);
    let ev = evs
        .iter()
        .find(|e| e.is(EventKind::Modifiers))
        .expect("modifiers");
    assert_eq!(ev.payload.get_str("kind"), Some("mods"), "the app's kind");
    let (msg, event) = ev.split::<Value>();
    assert_eq!(msg.as_ref().and_then(|m| m.get_str("kind")), Some("mods"));
    let Event::Modifiers(m) = event else {
        panic!("modifiers, got {event:?}");
    };
    assert!(m.shift);
}

#[test]
fn a_window_event_nobody_declared_is_not_an_event() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::column());
    assert!(
        shift(&mut core, true).is_empty(),
        "no windowEvents, no event"
    );
    // A group that leaves the key out is the same.
    let mut core = Core::new();
    frame_with(&mut core, WindowEvents::default().on_resize("resized"));
    assert!(shift(&mut core, true).is_empty());
}

#[test]
fn off_the_root_the_group_is_ignored_and_said_so() {
    let mut core = Core::new();
    let mut props = kui_core::schema::PropsOut::new();
    props.window_events = Some(WindowEvents::default().on_modifiers("mods"));
    {
        let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        ui.core().open_from(props, Content::Box);
        ui.core().close();
        ui.finish();
    }
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert!(
        codes.contains(&kui_core::diag::WINDOW_EVENTS_OFF_ROOT),
        "{codes:?}"
    );
    assert!(shift(&mut core, true).is_empty(), "and nothing declared it");
}

#[test]
fn split_reads_the_message_and_the_event_for_one_match() {
    let mut core = Core::new();
    frame(&mut core, NodeSpec::column().on_drag(pan()));
    let evs = drag(&mut core);
    let mut seen = Vec::new();
    for ev in &evs {
        match ev.split::<Value>() {
            (Some(m), Event::Drag(d)) if m.get_str("kind") == Some("pan") => {
                seen.push(d.delta.x as i64);
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(seen, [0, 50, 50]);
}

/// A message spelled like its event — `Msg::Click` on a click, `Msg::Submit`
/// on a submit — is the app's: an event with no data arrives as the message
/// exactly.
#[test]
fn a_message_named_like_its_event_is_still_the_app_s() {
    let mut core = Core::new();
    let click = Value::map([("kind", Value::str("click"))]);
    frame(&mut core, NodeSpec::column().on_click(click.clone()));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    let mut evs = core.handle_input(InputEvent::mouse_down(1));
    evs.extend(core.handle_input(InputEvent::mouse_up()));
    let ev = evs
        .iter()
        .find(|e| e.is(EventKind::Click))
        .expect("a click");
    assert_eq!(ev.data(), None);
    assert_eq!(ev.message::<Value>(), Some(click));
}
