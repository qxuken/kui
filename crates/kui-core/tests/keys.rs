//! Full-keyboard routing: nodes that declare `on_key` become key sinks, and
//! the key-focused sink receives presses as data — the path an app that owns
//! its own text model (a modal editor, a terminal) binds against.

use kui_core::{
    Core, EditOptions, InputEvent, Key, KeyCode, KeyMods, KeyPress, NodeSpec, Size, Sizing,
    UiEvent, Value, Vec2,
};

/// Two side-by-side key sinks (think: two editor panes), left one focused.
/// Both opt into releases: the tests below are about the two halves of a
/// key, and a sink hears the second half only by asking (`key_up`).
fn frame(core: &mut Core, focus_left: bool) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let left = ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .on_key(Value::map([("pane", Value::Int(0))]))
            .key_up(),
        |_| {},
    );
    let right = ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .on_key(Value::map([("pane", Value::Int(1))]))
            .key_up(),
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

/// The keymap case, which is the default: a sink that says only `on_key`
/// hears the press and not the release, so Space toggles once. Nothing
/// else changes — a repeat is still a press, and the key is still tracked
/// as held, so the stray release resolves to nothing rather than to a
/// second event.
#[test]
fn a_sink_without_key_up_hears_presses_only() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with(NodeSpec::column().fill().on_key(Value::Null), |_| {});
    ui.take_key_focus(sink);
    ui.finish();
    let down = |repeat| {
        InputEvent::KeyDown(KeyPress {
            repeat,
            ..KeyPress::new(KeyCode::Char(' '), KeyMods::default())
        })
    };
    let evs = drive(
        &mut core,
        &[down(false), down(true), release(KeyCode::Char(' '))],
    );
    assert_eq!(
        keys(&evs),
        [("down".into(), " ".into()), ("down".into(), " ".into())],
        "a press and its repeat, and no release"
    );
    assert!(evs.iter().all(|e| e.key == sink));
    // Nor a synthetic one: focus leaving with a key held lets go of it
    // silently, since the sink never asked to hear the way up.
    drive(&mut core, &[press(KeyCode::Char('w'))]);
    core.set_focus(None);
    assert!(core.take_pending_events().is_empty());
    assert!(drive(&mut core, &[release(KeyCode::Char('w'))]).is_empty());
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
    let sink = ui.with(
        NodeSpec::column().fill().on_key(Value::Null).key_up(),
        |_| {},
    );
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

/// splitmux's shape: one sink wrapping the panes, each pane clickable so a
/// click can focus it. A pane is a derived button and so a Tab stop, but it
/// is drawn *by* the app that owns the keyboard — clicking it must not take
/// the keyboard away, or every Alt chord dies on the first click and never
/// comes back (`take_key_focus` is edge-triggered).
fn multiplexer(core: &mut Core) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut pane = None;
    let sink = ui.with_keyed("main", NodeSpec::row().fill().on_key(Value::Null), |ui| {
        pane = Some(
            ui.with_keyed(
                "pane1",
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .on_click(Value::map([("kind", Value::str("focus"))])),
                |_| {},
            ),
        );
    });
    ui.take_key_focus(sink);
    ui.finish();
    (sink, pane.unwrap())
}

fn click_at(x: f32, y: f32) -> [InputEvent; 3] {
    [
        InputEvent::CursorMoved(Vec2::new(x, y)),
        InputEvent::mouse_down(1),
        InputEvent::mouse_up(),
    ]
}

#[test]
fn a_click_inside_a_sink_leaves_the_keyboard_on_the_sink() {
    let mut core = Core::new();
    let (sink, pane) = multiplexer(&mut core);
    // The click still reaches the pane as a click...
    let evs = drive(&mut core, &click_at(200.0, 150.0));
    assert!(
        evs.iter().any(
            |e| e.key == pane && e.payload.get("kind").and_then(Value::as_str) == Some("focus")
        ),
        "the pane still hears its own click"
    );
    // ...but the keyboard stayed put, without the view asking again.
    assert_eq!(core.key_focus(), Some(sink));
    multiplexer(&mut core);
    let evs = drive(&mut core, &[press(KeyCode::Char('v'))]);
    assert_eq!(keys(&evs), [("down".into(), "v".into())]);
    assert_eq!(evs[0].key, sink);
}

#[test]
fn an_editor_inside_a_sink_still_takes_the_keyboard() {
    let mut core = Core::new();
    let mut edit = None;
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with_keyed("main", NodeSpec::row().fill().on_key(Value::Null), |ui| {
        edit = Some(ui.text_edit(
            "field",
            "hi",
            &EditOptions::default(),
            NodeSpec::row().fill(),
        ));
    });
    ui.take_key_focus(sink);
    ui.finish();
    let edit = edit.unwrap();
    // A sink owns its keyboard, but not against a real editor it drew: the
    // caret has to land where the user clicked.
    drive(&mut core, &click_at(200.0, 150.0));
    assert_eq!(core.key_focus(), Some(edit));
}

#[test]
fn pressing_window_chrome_leaves_the_keyboard_where_it_was() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    // A custom titlebar above the app's own key sink, as every custom-chrome
    // app draws it.
    ui.with_keyed(
        "bar",
        NodeSpec::row()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(40.0))
            .window_drag(),
        |_| {},
    );
    let sink = ui.with_keyed("main", NodeSpec::row().fill().on_key(Value::Null), |_| {});
    ui.take_key_focus(sink);
    ui.finish();
    // Grabbing the window to move it is the platform's business; the app
    // does not lose its chords over it.
    drive(&mut core, &click_at(200.0, 20.0));
    assert_eq!(core.key_focus(), Some(sink));
}

// -- Layout portability ----------------------------------------------------
// `code` is what a keymap binds; `physical` is where the key is. See ADR
// 0002 decision 11 and `KeyPress::from_layout`.

/// A press as a driver builds it: what the layout put on the key, and which
/// key it was.
fn layout_press(layout: KeyCode, physical: KeyCode) -> KeyPress {
    KeyPress::from_layout(layout, physical, KeyMods::default())
}

/// The whole point: a keymap written `match code { "v" => split }` has to
/// keep working when the layout does not speak Latin at all.
#[test]
fn a_non_latin_layout_reports_the_position_as_code() {
    // Russian ЙЦУКЕН: the key US-QWERTY prints V on produces "м".
    let kp = layout_press(KeyCode::Char('м'), KeyCode::Char('v'));
    assert_eq!(kp.code, KeyCode::Char('v'));
    assert_eq!(kp.physical, KeyCode::Char('v'));
    // Greek, Hebrew and Arabic fold the same way.
    for (layout, physical) in [('ς', 'w'), ('ט', 'y'), ('ب', 'b')] {
        let kp = layout_press(KeyCode::Char(layout), KeyCode::Char(physical));
        assert_eq!(kp.code, KeyCode::Char(physical));
    }
}

/// The other half: a Latin layout keeps its own key, so a chord lands on the
/// key the user can *see* rather than wherever QWERTY would have put it.
#[test]
fn a_latin_layout_keeps_the_key_it_prints() {
    // Dvorak: the key printed V sits where QWERTY prints ".".
    let kp = layout_press(KeyCode::Char('v'), KeyCode::Char('.'));
    assert_eq!(kp.code, KeyCode::Char('v'), "⌥v is on the key printed V");
    assert_eq!(kp.physical, KeyCode::Char('.'), "and reports where that is");
    // AZERTY's A (QWERTY Q), QWERTZ's Z (QWERTY Y).
    for (layout, physical) in [('a', 'q'), ('z', 'y'), ('q', 'a')] {
        let kp = layout_press(KeyCode::Char(layout), KeyCode::Char(physical));
        assert_eq!(kp.code, KeyCode::Char(layout));
        assert_eq!(kp.physical, KeyCode::Char(physical));
    }
}

/// Named keys are layout-independent already, and a layout key this
/// vocabulary cannot name falls back to the position like a non-Latin one.
#[test]
fn named_keys_pass_through_and_unnameable_ones_fall_back() {
    let kp = layout_press(KeyCode::Enter, KeyCode::Enter);
    assert_eq!(kp.code, KeyCode::Enter);
    let kp = layout_press(KeyCode::Unknown, KeyCode::Char('/'));
    assert_eq!(
        kp.code,
        KeyCode::Char('/'),
        "a dead key still names its slot"
    );
    // Nothing to fall back to: the press stays unknown rather than inventing.
    let kp = layout_press(KeyCode::Unknown, KeyCode::Unknown);
    assert_eq!(kp.code, KeyCode::Unknown);
}

/// A press names one key unless it is told otherwise, so every injected
/// press and every test above this line keeps meaning what it said.
#[test]
fn an_injected_press_is_its_own_position() {
    let kp = KeyPress::new(KeyCode::Char('w'), KeyMods::default());
    assert_eq!(kp.physical, KeyCode::Char('w'));
    assert_eq!(
        kp.with_physical(KeyCode::Char(',')).physical,
        KeyCode::Char(',')
    );
}

/// Both reach the app on the same payload, and a release carries them too.
#[test]
fn both_codes_cross_as_data() {
    let mut core = Core::new();
    let (left, _) = frame(&mut core, true);
    let ru = layout_press(KeyCode::Char('м'), KeyCode::Char('v')).with_text("м");
    let evs = drive(&mut core, &[InputEvent::KeyDown(ru.clone())]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, left);
    let p = &evs[0].payload;
    assert_eq!(p.get("code").and_then(Value::as_str), Some("v"));
    assert_eq!(p.get("physical").and_then(Value::as_str), Some("v"));
    // `text` is the typing view and stays the layout's own character.
    assert_eq!(p.get("text").and_then(Value::as_str), Some("м"));
    let evs = drive(&mut core, &[InputEvent::KeyUp(ru.released())]);
    let p = &evs[0].payload;
    assert_eq!(p.get("code").and_then(Value::as_str), Some("v"));
    assert_eq!(p.get("physical").and_then(Value::as_str), Some("v"));
}

/// A Dvorak user holding a key and letting go resolves it: the release is
/// matched on `code`, which is stable across the press for a given key.
#[test]
fn a_held_key_on_a_remapped_layout_resolves_its_release() {
    let mut core = Core::new();
    frame(&mut core, true);
    let v = layout_press(KeyCode::Char('v'), KeyCode::Char('.'));
    let evs = drive(&mut core, &[InputEvent::KeyDown(v.clone())]);
    assert_eq!(keys(&evs), [("down".into(), "v".into())]);
    let evs = drive(&mut core, &[InputEvent::KeyUp(v.released())]);
    assert_eq!(keys(&evs), [("up".into(), "v".into())]);
}
