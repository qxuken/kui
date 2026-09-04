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

fn release(code: KeyCode) -> InputEvent {
    InputEvent::KeyUp(KeyPress::new(code, KeyMods::default()))
}

/// The `(phase, code)` of every key event in a batch, in order.
fn keys(evs: &[UiEvent]) -> Vec<(String, String)> {
    evs.iter()
        .filter(|e| e.payload.get("kind").and_then(Value::as_str) == Some("key"))
        .map(|e| {
            let at = |k: &str| {
                e.payload
                    .get(k)
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            (at("phase"), at("code"))
        })
        .collect()
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

// -- Press and release ---------------------------------------------------

#[test]
fn a_press_and_its_release_are_one_kind_with_a_phase() {
    let mut core = Core::new();
    let (left, _) = frame(&mut core, true);
    let evs = drive(
        &mut core,
        &[press(KeyCode::Char('w')), release(KeyCode::Char('w'))],
    );
    assert_eq!(evs.len(), 2);
    assert!(evs.iter().all(|e| e.key == left));
    assert_eq!(
        keys(&evs),
        [("down".into(), "w".into()), ("up".into(), "w".into())]
    );
    // Both halves carry the sink's tag; a release inserts nothing.
    for e in &evs {
        assert_eq!(
            e.payload
                .get("tag")
                .and_then(|t| t.get("pane"))
                .and_then(Value::as_int),
            Some(0)
        );
    }
    assert_eq!(evs[1].payload.get("text"), Some(&Value::Null));
    assert_eq!(evs[1].payload.get("repeat"), Some(&Value::Bool(false)));
}

#[test]
fn a_release_never_carries_text_or_repeat() {
    let mut core = Core::new();
    frame(&mut core, true);
    let held = KeyPress::new(KeyCode::Char('w'), KeyMods::default()).with_text("w");
    let evs = drive(
        &mut core,
        &[
            InputEvent::KeyDown(KeyPress {
                repeat: true,
                ..held.clone()
            }),
            // Even a driver that reports one, on the way up.
            InputEvent::KeyUp(KeyPress {
                repeat: true,
                ..held
            }),
        ],
    );
    assert_eq!(
        evs[0].payload.get("text").and_then(Value::as_str),
        Some("w")
    );
    assert_eq!(evs[0].payload.get("repeat"), Some(&Value::Bool(true)));
    assert_eq!(evs[1].payload.get("text"), Some(&Value::Null));
    assert_eq!(evs[1].payload.get("repeat"), Some(&Value::Bool(false)));
}

#[test]
fn os_repeat_is_the_same_key_still_held() {
    let mut core = Core::new();
    frame(&mut core, true);
    let down = |repeat| {
        InputEvent::KeyDown(KeyPress {
            repeat,
            ..KeyPress::new(KeyCode::Char('w'), KeyMods::default())
        })
    };
    // Press, two auto-repeats, release: four events, one release.
    let evs = drive(
        &mut core,
        &[
            down(false),
            down(true),
            down(true),
            release(KeyCode::Char('w')),
        ],
    );
    assert_eq!(
        keys(&evs),
        [
            ("down".into(), "w".into()),
            ("down".into(), "w".into()),
            ("down".into(), "w".into()),
            ("up".into(), "w".into()),
        ]
    );
    // And the one release is the only one: the key is no longer held.
    assert!(drive(&mut core, &[release(KeyCode::Char('w'))]).is_empty());
}

#[test]
fn a_release_without_a_delivered_press_resolves_nothing() {
    let mut core = Core::new();
    frame(&mut core, true);
    // Nobody pressed it — a stray release (an app that started with the key
    // already down) is not a phantom event on the sink.
    assert!(drive(&mut core, &[release(KeyCode::Char('w'))]).is_empty());
}

#[test]
fn focus_moving_releases_what_the_old_sink_held() {
    let mut core = Core::new();
    let (left, right) = frame(&mut core, true);
    let evs = drive(
        &mut core,
        &[press(KeyCode::Char('w')), press(KeyCode::Char('a'))],
    );
    assert_eq!(keys(&evs).len(), 2);
    // A click in the right pane takes focus; both held keys come up on the
    // left sink first, so a WASD binding cannot be left walking forever.
    let evs = drive(
        &mut core,
        &[
            InputEvent::CursorMoved(Vec2::new(300.0, 150.0)),
            InputEvent::mouse_down(1),
            InputEvent::mouse_up(),
        ],
    );
    let ups: Vec<_> = evs
        .iter()
        .filter(|e| e.payload.get("kind").and_then(Value::as_str) == Some("key"))
        .collect();
    assert_eq!(ups.len(), 2);
    assert!(ups.iter().all(|e| e.key == left));
    assert_eq!(
        ups.iter()
            .map(|e| e.payload.get("code").and_then(Value::as_str).unwrap())
            .collect::<Vec<_>>(),
        ["w", "a"]
    );
    assert!(
        ups.iter()
            .all(|e| e.payload.get("phase").and_then(Value::as_str) == Some("up"))
    );
    // The physical release lands after the move: the new sink never saw the
    // press, so it hears nothing.
    let evs = drive(&mut core, &[release(KeyCode::Char('w'))]);
    assert!(keys(&evs).is_empty());
    // The new sink still takes its own presses.
    let evs = drive(&mut core, &[press(KeyCode::Char('j'))]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, right);
}

#[test]
fn losing_the_keyboard_releases_held_keys() {
    let mut core = Core::new();
    let (left, _) = frame(&mut core, true);
    drive(&mut core, &[press(KeyCode::Char('w'))]);
    // What a driver calls on window blur: the OS will send no release.
    core.release_held_keys();
    let evs = core.take_pending_events();
    assert_eq!(keys(&evs), [("up".into(), "w".into())]);
    assert_eq!(evs[0].key, left);
    // Idempotent: nothing is held any more.
    core.release_held_keys();
    assert!(core.take_pending_events().is_empty());
}

#[test]
fn a_key_held_over_an_editor_taking_focus_is_not_delivered_twice() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with(NodeSpec::column().fill().on_key(Value::Null), |_| {});
    ui.take_key_focus(sink);
    ui.finish();
    drive(&mut core, &[press(KeyCode::Char('w'))]);
    // Focus goes somewhere that is not a sink at all: the release still has
    // to reach the sink that took the press.
    core.set_focus(None);
    let evs = core.take_pending_events();
    assert_eq!(keys(&evs), [("up".into(), "w".into())]);
    assert!(drive(&mut core, &[release(KeyCode::Char('w'))]).is_empty());
}

#[test]
fn key_names_round_trip_through_from_name() {
    for code in [
        KeyCode::Char('w'),
        KeyCode::Char('$'),
        KeyCode::F(5),
        KeyCode::F(24),
        KeyCode::Left,
        KeyCode::PageDown,
        KeyCode::Backspace,
        KeyCode::Escape,
        KeyCode::Space,
        KeyCode::Insert,
        KeyCode::Unknown,
    ] {
        assert_eq!(KeyCode::from_name(&code.name()), Some(code));
    }
    assert_eq!(KeyCode::from_name("f0"), None);
    assert_eq!(KeyCode::from_name("f25"), None);
    assert_eq!(KeyCode::from_name("nonsense"), None);
}
