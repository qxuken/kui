//! Full-keyboard routing: nodes that declare `on_key` become key sinks, and
//! the key-focused sink receives presses as data — the path an app that owns
//! its own text model (a modal editor, a terminal) binds against.

use kui_core::{
    Core, InputEvent, Key, KeyCode, KeyMods, KeyPress, NodeSpec, Size, Sizing, UiEvent, Value, Vec2,
};

/// Two side-by-side key sinks (think: two editor panes), left one focused.
fn frame(core: &mut Core, focus_left: bool) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let left = ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .on_key(Value::map([("pane", Value::Int(0))])),
        |_| {},
    );
    let right = ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .on_key(Value::map([("pane", Value::Int(1))])),
        |_| {},
    );
    ui.take_key_focus(if focus_left { left } else { right });
    ui.finish();
    (left, right)
}

fn press(code: KeyCode) -> InputEvent {
    InputEvent::KeyDown(KeyPress::new(code, KeyMods::default()))
}

fn drive(core: &mut Core, events: &[InputEvent]) -> Vec<UiEvent> {
    let mut out = Vec::new();
    for ev in events {
        out.extend(core.handle_input(ev.clone()));
    }
    out
}

#[test]
fn focused_sink_receives_keys_as_data() {
    let mut core = Core::new();
    let (left, _) = frame(&mut core, true);
    let evs = drive(&mut core, &[press(KeyCode::Char('i'))]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, left);
    let p = &evs[0].payload;
    assert_eq!(p.get("kind").and_then(Value::as_str), Some("key"));
    assert_eq!(p.get("code").and_then(Value::as_str), Some("i"));
    assert_eq!(p.get("ctrl").and_then(Value::as_bool), Some(false));
    // The sink's on_key payload rides along under "tag".
    assert_eq!(
        p.get("tag")
            .and_then(|t| t.get("pane"))
            .and_then(Value::as_int),
        Some(0)
    );
}

#[test]
fn modifiers_and_text_cross_as_data() {
    let mut core = Core::new();
    frame(&mut core, true);
    let kp = KeyPress::new(
        KeyCode::Char('w'),
        KeyMods {
            ctrl: true,
            ..Default::default()
        },
    );
    let evs = drive(&mut core, &[InputEvent::KeyDown(kp)]);
    let p = &evs[0].payload;
    assert_eq!(p.get("ctrl").and_then(Value::as_bool), Some(true));
    assert_eq!(p.get("text"), Some(&Value::Null));

    let typed = KeyPress::new(KeyCode::Char('w'), KeyMods::default()).with_text("w");
    let evs = drive(&mut core, &[InputEvent::KeyDown(typed)]);
    assert_eq!(
        evs[0].payload.get("text").and_then(Value::as_str),
        Some("w")
    );
}

#[test]
fn clicking_a_sink_moves_key_focus() {
    let mut core = Core::new();
    let (_, right) = frame(&mut core, true);
    // Click in the right half; keys now land on the right sink even though
    // the frame's declaration said left — until the next declaration.
    let evs = drive(
        &mut core,
        &[
            InputEvent::CursorMoved(Vec2::new(300.0, 150.0)),
            InputEvent::mouse_down(1),
            InputEvent::mouse_up(),
            press(KeyCode::Escape),
        ],
    );
    let key_evs: Vec<_> = evs
        .iter()
        .filter(|e| e.payload.get("kind").and_then(Value::as_str) == Some("key"))
        .collect();
    assert_eq!(key_evs.len(), 1);
    assert_eq!(key_evs[0].key, right);
    assert_eq!(
        key_evs[0]
            .payload
            .get("tag")
            .and_then(|t| t.get("pane"))
            .and_then(Value::as_int),
        Some(1)
    );
}

#[test]
fn no_focus_no_events() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with(NodeSpec::column().fill().on_key(Value::Null), |_| {});
    ui.finish(); // sink declared, but nothing took focus
    assert!(drive(&mut core, &[press(KeyCode::Char('x'))]).is_empty());
}

#[test]
fn null_tag_omitted_from_payload() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with(NodeSpec::column().fill().on_key(Value::Null), |_| {});
    ui.take_key_focus(sink);
    ui.finish();
    let evs = drive(&mut core, &[press(KeyCode::Enter)]);
    assert_eq!(evs.len(), 1);
    assert_eq!(
        evs[0].payload.get("code").and_then(Value::as_str),
        Some("enter")
    );
    assert_eq!(evs[0].payload.get("tag"), None);
}

#[test]
fn modifier_changes_reach_the_host_as_data_and_are_queryable() {
    let mut core = Core::new();
    frame(&mut core, true);
    let cmd = KeyMods {
        super_key: true,
        ..Default::default()
    };
    let evs = core.handle_input(InputEvent::Modifiers(cmd));
    assert_eq!(evs.len(), 1);
    let p = &evs[0].payload;
    assert_eq!(p.get("kind").and_then(Value::as_str), Some("modifiers"));
    assert_eq!(p.get("super").and_then(Value::as_bool), Some(true));
    assert_eq!(p.get("shift").and_then(Value::as_bool), Some(false));
    assert_eq!(evs[0].key, Key::ROOT);
    assert_eq!(core.modifiers(), cmd);
    // Unchanged state is not re-reported.
    assert!(core.handle_input(InputEvent::Modifiers(cmd)).is_empty());
    // Release reports again.
    let evs = core.handle_input(InputEvent::Modifiers(KeyMods::default()));
    assert_eq!(evs.len(), 1);
    assert_eq!(
        evs[0].payload.get("super").and_then(Value::as_bool),
        Some(false)
    );
}
