//! Full-keyboard routing: nodes that declare `on_key` become key sinks, and
//! the key-focused sink receives presses as data — the path an app that owns
//! its own text model (a modal editor, a terminal) binds against.

use kui_core::testing::{click_at, drive, key_press as press, key_release as release};
use kui_core::{
    Align, Core, EditKey, EditOptions, FloatConfig, InputEvent, Key, KeyCode, KeyMods, KeyPress,
    Mods, NodeSpec, Size, UiEvent, Value, Vec2,
};

/// Two side-by-side key sinks (think: two editor panes), left one focused.
/// Both opt into releases: the tests below are about the two halves of a
/// key, and a sink hears the second half only by asking (`key_up`).
fn frame(core: &mut Core, focus_left: bool) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let left = ui.leaf(
        NodeSpec::column()
            .fill()
            .on_key(Value::map([("pane", Value::Int(0))]))
            .key_up(),
    );
    let right = ui.leaf(
        NodeSpec::column()
            .fill()
            .on_key(Value::map([("pane", Value::Int(1))]))
            .key_up(),
    );
    ui.take_key_focus(if focus_left { left } else { right });
    ui.finish();
    (left, right)
}

/// The `(phase, code)` of every key event in a batch, in order.
fn keys(evs: &[UiEvent]) -> Vec<(String, String)> {
    evs.iter()
        .filter(|e| e.kind() == Some("key"))
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

#[test]
fn focused_sink_receives_keys_as_data() {
    let mut core = Core::new();
    let (left, _) = frame(&mut core, true);
    let evs = drive(&mut core, &[press(KeyCode::Char('i'))]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, left);
    let p = &evs[0].payload;
    assert_eq!(p.get_str("kind"), Some("key"));
    assert_eq!(p.get_str("code"), Some("i"));
    assert_eq!(p.get_bool("ctrl"), Some(false));
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
    let kp = KeyPress::new(KeyCode::Char('w'), KeyMods::NONE.with_ctrl());
    let evs = drive(&mut core, &[InputEvent::KeyDown(kp)]);
    let p = &evs[0].payload;
    assert_eq!(p.get_bool("ctrl"), Some(true));
    assert_eq!(p.get("text"), Some(&Value::Null));

    let typed = KeyPress::new(KeyCode::Char('w'), KeyMods::default()).with_text("w");
    let evs = drive(&mut core, &[InputEvent::KeyDown(typed)]);
    assert_eq!(evs[0].payload.get_str("text"), Some("w"));
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
    let key_evs: Vec<_> = evs.iter().filter(|e| e.kind() == Some("key")).collect();
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
    ui.leaf(NodeSpec::column().fill().key_sink());
    ui.finish(); // sink declared, but nothing took focus
    assert!(drive(&mut core, &[press(KeyCode::Char('x'))]).is_empty());
}

#[test]
fn null_tag_omitted_from_payload() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(100.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.leaf(NodeSpec::column().fill().key_sink());
    ui.take_key_focus(sink);
    ui.finish();
    let evs = drive(&mut core, &[press(KeyCode::Enter)]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.get_str("code"), Some("enter"));
    assert_eq!(evs[0].payload.get("tag"), None);
}

#[test]
fn modifier_changes_reach_the_host_as_data_and_are_queryable() {
    let mut core = Core::new();
    frame(&mut core, true);
    let cmd = KeyMods::NONE.with_super();
    let evs = core.handle_input(InputEvent::Modifiers(cmd));
    assert_eq!(evs.len(), 1);
    let p = &evs[0].payload;
    assert_eq!(p.get_str("kind"), Some("modifiers"));
    assert_eq!(p.get_bool("super"), Some(true));
    assert_eq!(p.get_bool("shift"), Some(false));
    assert_eq!(evs[0].key, Key::ROOT);
    assert_eq!(core.modifiers(), cmd);
    // Unchanged state is not re-reported.
    assert!(core.handle_input(InputEvent::Modifiers(cmd)).is_empty());
    // Release reports again.
    let evs = core.handle_input(InputEvent::Modifiers(KeyMods::default()));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.get_bool("super"), Some(false));
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
    let sink = ui.leaf(NodeSpec::column().fill().key_sink());
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
    assert_eq!(evs[0].payload.get_str("text"), Some("w"));
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
    let ups: Vec<_> = evs.iter().filter(|e| e.kind() == Some("key")).collect();
    assert_eq!(ups.len(), 2);
    assert!(ups.iter().all(|e| e.key == left));
    assert_eq!(
        ups.iter()
            .map(|e| e.payload.get_str("code").unwrap())
            .collect::<Vec<_>>(),
        ["w", "a"]
    );
    assert!(ups.iter().all(|e| e.payload.get_str("phase") == Some("up")));
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
    let sink = ui.leaf(NodeSpec::column().fill().key_sink().key_up());
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

// -- Both channels of one press (backlog F6) --------------------------------

/// A press is two channels and a window drives both: the raw key to the
/// focused sink, and then what the *core* is asked to do with that key.
/// [`KeyPress::edit_event`] is the second one, and the one table every
/// driver and every headless injector reads — so this pins the table.
#[test]
fn the_second_channel_of_a_press_is_one_table() {
    let plain = |code| KeyPress::new(code, KeyMods::default());
    let named = |ek| Some(InputEvent::Key(ek, Mods::default()));
    // The keys the editing vocabulary names.
    assert_eq!(plain(KeyCode::Escape).edit_event(), named(EditKey::Escape));
    assert_eq!(plain(KeyCode::Tab).edit_event(), named(EditKey::Tab));
    assert_eq!(plain(KeyCode::Right).edit_event(), named(EditKey::Right));
    assert_eq!(
        plain(KeyCode::Backspace).edit_event(),
        named(EditKey::Backspace)
    );
    // Shift-Tab is the same key carrying the modifier the ring reads, and
    // Alt is `word`, the platform primary `doc` — the whole of what the
    // editing vocabulary normalizes.
    let shift = KeyMods::NONE.with_shift();
    assert_eq!(
        KeyPress::new(KeyCode::Tab, shift).edit_event(),
        Some(InputEvent::Key(EditKey::Tab, Mods::NONE.with_shift()))
    );
    let alt = KeyMods::NONE.with_alt();
    assert_eq!(
        KeyPress::new(KeyCode::Left, alt).edit_event(),
        Some(InputEvent::Key(EditKey::Left, Mods::NONE.with_word()))
    );
    // Space is the text channel, not an `EditKey`: it presses a focused
    // control and inserts into a focused editor.
    assert_eq!(
        plain(KeyCode::Space).edit_event(),
        Some(InputEvent::Text(" ".to_string()))
    );
    // A printable key inserts what the layout said it inserts.
    assert_eq!(
        plain(KeyCode::Char('w')).with_text("w").edit_event(),
        Some(InputEvent::Text("w".to_string()))
    );
    // A chord carries no text, so it stops at the sink channel — as does a
    // key the vocabulary does not name at all.
    assert_eq!(plain(KeyCode::Char('w')).edit_event(), None);
    assert_eq!(plain(KeyCode::F(5)).edit_event(), None);
    assert_eq!(plain(KeyCode::Insert).edit_event(), None);
}

/// A key sink inside a modal — the field report's editor — and the two
/// channels one Escape travels: the keymap hears it, *and* the modal asks
/// to go away. Driving only `KeyDown` is what left every one of those
/// editors open.
fn sink_in_a_modal(core: &mut Core) -> (Key, Key) {
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.leaf_keyed(
        "behind",
        NodeSpec::row()
            .size(60.0, 20.0)
            .on_click("behind")
            .label("Behind"),
    );
    let mut sink = Key::ROOT;
    let dialog = ui.with_keyed(
        "dialog",
        NodeSpec::column()
            .size(120.0, 60.0)
            .float(FloatConfig::viewport().inside(Align::End, Align::End))
            .modal("editor")
            .label("Editor"),
        |ui| {
            sink = ui.leaf_keyed(
                "notes",
                NodeSpec::column()
                    .size(100.0, 40.0)
                    .on_key("notes")
                    .label("Notes"),
            );
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    (sink, dialog)
}

/// The `kind` of every event in a batch, in order.
fn kinds(evs: &[UiEvent]) -> Vec<&str> {
    evs.iter().map(|e| e.kind().unwrap_or("")).collect()
}

#[test]
fn a_press_reaches_the_sink_and_the_core_both() {
    let mut core = Core::new();
    let (sink, dialog) = sink_in_a_modal(&mut core);
    let escape = || KeyPress::new(KeyCode::Escape, KeyMods::default());

    // The half that was never enough: the keymap hears it and the modal
    // stays open, because dismissal lives on the other channel.
    let evs = core.handle_input(InputEvent::KeyDown(escape()));
    assert_eq!(kinds(&evs), ["key"]);
    assert_eq!(evs[0].key, sink);

    // The whole press: the same event, then the dismissal, in the order a
    // window sends them.
    let evs = core.press(escape());
    assert_eq!(kinds(&evs), ["key", "dismiss"]);
    assert_eq!(evs[0].key, sink);
    assert_eq!(evs[1].key, dialog);
    assert_eq!(evs[1].payload.get_str("reason"), Some("escape"));

    // The release is one channel, because only one has a second half. This
    // sink never asked for `key_up`, so it hears nothing at all.
    assert_eq!(kinds(&core.release(escape())), [] as [&str; 0]);
}

/// The pomodoro report's side of the same split: Tab, Space and the arrows
/// are *all* on the channel `KeyDown` is not, so a control reached by
/// keyboard did nothing until the whole press arrived.
#[test]
fn a_press_walks_the_ring_presses_a_control_and_nudges_a_slider() {
    use kui_core::access::Role;
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let go = ui.leaf_keyed(
        "go",
        NodeSpec::row().size(60.0, 20.0).on_click("go").label("Go"),
    );
    let vol = ui.leaf_keyed(
        "vol",
        NodeSpec::row()
            .size(100.0, 10.0)
            .role(Role::Slider)
            .label("Volume")
            .value_now(3.0)
            .value_min(0.0)
            .value_max(10.0)
            .on_drag("vol"),
    );
    ui.finish();

    assert_eq!(core.focus(), None, "nothing focused at first");
    let press = |core: &mut Core, code| core.press(KeyPress::new(code, KeyMods::default()));

    assert_eq!(kinds(&press(&mut core, KeyCode::Tab)), [] as [&str; 0]);
    assert_eq!(core.focus(), Some(go), "tab moved focus onto the button");

    let evs = press(&mut core, KeyCode::Space);
    assert_eq!(evs.len(), 1, "space pressed the focused control");
    assert_eq!(evs[0].key, go);
    assert_eq!(evs[0].payload.as_str(), Some("go"));

    press(&mut core, KeyCode::Tab);
    assert_eq!(core.focus(), Some(vol), "tab moved on to the slider");

    let evs = press(&mut core, KeyCode::Right);
    assert_eq!(kinds(&evs), ["access"]);
    assert_eq!(evs[0].key, vol);
    assert_eq!(evs[0].payload.get_str("action"), Some("increment"));
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
    let sink = ui.with_keyed("main", NodeSpec::row().fill().key_sink(), |ui| {
        pane = Some(
            ui.leaf_keyed(
                "pane1",
                NodeSpec::column()
                    .fill()
                    .on_click(Value::map([("kind", Value::str("focus"))])),
            ),
        );
    });
    ui.take_key_focus(sink);
    ui.finish();
    (sink, pane.unwrap())
}

#[test]
fn a_click_inside_a_sink_leaves_the_keyboard_on_the_sink() {
    let mut core = Core::new();
    let (sink, pane) = multiplexer(&mut core);
    // The click still reaches the pane as a click...
    let evs = click_at(&mut core, 200.0, 150.0);
    assert!(
        evs.iter()
            .any(|e| e.key == pane && e.kind() == Some("focus")),
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
    let sink = ui.with_keyed("main", NodeSpec::row().fill().key_sink(), |ui| {
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
    click_at(&mut core, 200.0, 150.0);
    assert_eq!(core.key_focus(), Some(edit));
}

#[test]
fn pressing_window_chrome_leaves_the_keyboard_where_it_was() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    // A custom titlebar above the app's own key sink, as every custom-chrome
    // app draws it.
    ui.leaf_keyed(
        "bar",
        NodeSpec::row().grow_width().height(40.0).window_drag(),
    );
    let sink = ui.leaf_keyed("main", NodeSpec::row().fill().key_sink());
    ui.take_key_focus(sink);
    ui.finish();
    // Grabbing the window to move it is the platform's business; the app
    // does not lose its chords over it.
    click_at(&mut core, 200.0, 20.0);
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

/// Shift under the fallback: a window reports `physical` from a table that
/// never sees Shift, so the letter that stands in has to be shifted here,
/// or `J` on a Russian layout is `j` and the `:` a vim hand reaches for on
/// the `;` key is `;`. The code is what US-QWERTY would have produced for
/// the same press — the upper-case letter, the shifted symbol.
#[test]
fn shift_under_the_fallback_is_the_us_shifted_symbol() {
    let shift = KeyMods::NONE.with_shift();
    for (layout, physical, want) in [
        ('О', 'j', 'J'),
        ('Ж', ';', ':'),
        ('Ё', '`', '~'),
        ('Б', ',', '<'),
        ('Ю', '.', '>'),
        ('Х', '[', '{'),
        ('№', '3', '#'),
    ] {
        let kp = KeyPress::from_layout(KeyCode::Char(layout), KeyCode::Char(physical), shift);
        assert_eq!(
            kp.code,
            KeyCode::Char(want),
            "shift-{physical} on a Russian layout"
        );
        assert_eq!(
            kp.physical,
            KeyCode::Char(physical),
            "the position stays unshifted"
        );
    }
    // A dead key under Shift names its slot's shifted symbol.
    let kp = KeyPress::from_layout(KeyCode::Unknown, KeyCode::Char('/'), shift);
    assert_eq!(kp.code, KeyCode::Char('?'));
    // Without Shift the fallback is the plain key, as before.
    let kp = KeyPress::from_layout(KeyCode::Char('ж'), KeyCode::Char(';'), KeyMods::default());
    assert_eq!(kp.code, KeyCode::Char(';'));
    // A layout that speaks ASCII under Shift is not touched: Russian's own
    // `:` on Shift+6, and a Latin layout's upper-case letter.
    let kp = KeyPress::from_layout(KeyCode::Char(':'), KeyCode::Char('6'), shift);
    assert_eq!(kp.code, KeyCode::Char(':'));
    let kp = KeyPress::from_layout(KeyCode::Char('J'), KeyCode::Char('j'), shift);
    assert_eq!(kp.code, KeyCode::Char('J'));
}

/// Under Alt the stand-in is the unshifted position, whatever the layout
/// composed: macOS US ⌥⇧J is `Ô`, which is not ASCII, and a door passing
/// it got `J` where the winit runner — reading the key with every
/// modifier stripped — got `j` (RG27). The rule is here, so every door
/// agrees, and `mods` still says Shift was held.
#[test]
fn alt_keeps_the_stand_in_unshifted() {
    let alt_shift = KeyMods::NONE.with_alt().with_shift();
    for (layout, physical) in [
        (KeyCode::Char('Ô'), 'j'),
        (KeyCode::Char('О'), 'j'),
        (KeyCode::Char('Ж'), ';'),
        (KeyCode::Unknown, '/'),
    ] {
        let kp = KeyPress::from_layout(layout, KeyCode::Char(physical), alt_shift);
        assert_eq!(kp.code, KeyCode::Char(physical), "⌥⇧ over {layout:?}");
        assert_eq!(kp.mods, alt_shift, "the chord keeps its Shift");
    }
    // Ctrl and Super do not compose, so Shift still shifts under them.
    let cmd_shift = KeyMods::NONE.with_super().with_shift();
    let kp = KeyPress::from_layout(KeyCode::Char('О'), KeyCode::Char('j'), cmd_shift);
    assert_eq!(kp.code, KeyCode::Char('J'));
}

/// What a door types for a host that did not say is the layout's key,
/// never the stand-in: ⇧ on Russian's `Ж` key binds as `:` and types `Ж`
/// (RG28). Nothing under a chord, a space for Space, nothing for a named
/// key or one the layout could not name.
#[test]
fn a_key_types_what_the_layout_named() {
    let shift = KeyMods::NONE.with_shift();
    let kp = KeyPress::from_layout(KeyCode::Char('Ж'), KeyCode::Char(';'), shift);
    assert_eq!(kp.code, KeyCode::Char(':'));
    assert_eq!(KeyCode::Char('Ж').typed(shift).as_deref(), Some("Ж"));
    assert_eq!(KeyCode::Space.typed(shift).as_deref(), Some(" "));
    assert_eq!(KeyCode::Enter.typed(KeyMods::default()), None);
    assert_eq!(KeyCode::Unknown.typed(KeyMods::default()), None);
    for chord in [
        KeyMods {
            ctrl: true,
            ..shift
        },
        KeyMods { alt: true, ..shift },
        KeyMods {
            super_key: true,
            ..shift
        },
    ] {
        assert_eq!(KeyCode::Char('Ж').typed(chord), None, "{chord:?}");
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
    assert_eq!(p.get_str("code"), Some("v"));
    assert_eq!(p.get_str("physical"), Some("v"));
    // `text` is the typing view and stays the layout's own character.
    assert_eq!(p.get_str("text"), Some("м"));
    let evs = drive(&mut core, &[InputEvent::KeyUp(ru.released())]);
    let p = &evs[0].payload;
    assert_eq!(p.get_str("code"), Some("v"));
    assert_eq!(p.get_str("physical"), Some("v"));
}

/// AR9, the WASD case ADR 0002 sells `keyUp` for: hold `w`, press Shift
/// to run, and the OS repeat arrives as `W`. Matched on `code` that was
/// a second held key, and letting go of `W` left `w` held until focus
/// moved — a synthetic `up w` at the wrong time. The position is what
/// the finger never left, so the repeat is the same key and the release
/// lets go of it, whichever case the layout put on it.
#[test]
fn shift_moving_under_a_held_key_does_not_make_it_a_second_key() {
    let mut core = Core::new();
    frame(&mut core, true);
    let w = KeyPress::new(KeyCode::Char('w'), KeyMods::default());
    let shift = KeyMods::NONE.with_shift();
    let big_w = KeyPress {
        code: KeyCode::Char('W'),
        physical: KeyCode::Char('w'),
        mods: shift,
        text: None,
        repeat: true,
    };
    let evs = drive(&mut core, &[InputEvent::KeyDown(w)]);
    assert_eq!(keys(&evs), [("down".into(), "w".into())]);
    // The repeat under Shift reports as the layout says, and is not a
    // second key held.
    let evs = drive(&mut core, &[InputEvent::KeyDown(big_w.clone())]);
    assert_eq!(keys(&evs), [("down".into(), "W".into())]);
    // Letting go of `W` is letting go of the one key.
    let evs = drive(&mut core, &[InputEvent::KeyUp(big_w.released())]);
    assert_eq!(keys(&evs), [("up".into(), "W".into())]);
    // Nothing is still held: moving focus releases no `w`.
    core.set_focus(None);
    assert!(
        keys(&core.take_pending_events()).is_empty(),
        "no key was left held"
    );
}

/// An injected press whose position the vocabulary cannot name has only
/// its `code` to be matched on, and is.
#[test]
fn a_press_with_no_position_is_matched_on_its_code() {
    let mut core = Core::new();
    frame(&mut core, true);
    let f = KeyPress::new(KeyCode::Char('f'), KeyMods::default()).with_physical(KeyCode::Unknown);
    drive(&mut core, &[InputEvent::KeyDown(f.clone())]);
    let evs = drive(&mut core, &[InputEvent::KeyUp(f.released())]);
    assert_eq!(keys(&evs), [("up".into(), "f".into())]);
}

/// A Dvorak user holding a key and letting go resolves it: the release is
/// matched on the key's position, which is what stays put across a press.
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

/// A focused sink the frame draws outside its scroller's clip — a column
/// scrolled off a ribbon, a pane an `enter` or a `slide` has not finished
/// moving — still hears the keyboard: a key reaches a node by focus, not
/// by being under a point (F79). Before it, the delivery read the hit
/// list, which only a node some point could land on is in, so the keys
/// typed at such a pane fell on the floor.
#[test]
fn a_focused_sink_outside_the_clip_still_hears_the_keyboard() {
    let mut core = Core::new();
    // A row 400 wide that scrolls, holding two 400-wide sinks: the
    // second one begins wholly past the right edge.
    let build = |core: &mut Core, focus_second: bool| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::row().fill());
        let mut cols = Vec::new();
        ui.with_keyed("strip", NodeSpec::row().fill().scroll_x(), |ui| {
            for i in 0..2 {
                cols.push(
                    ui.leaf_keyed(
                        &format!("col{i}"),
                        NodeSpec::column()
                            .width(400.0)
                            .grow_height()
                            .on_key(Value::map([("pane", Value::Int(i))])),
                    ),
                );
            }
        });
        let (first, second) = (cols[0], cols[1]);
        ui.take_key_focus(if focus_second { second } else { first });
        ui.finish();
        (first, second)
    };
    let (first, second) = build(&mut core, false);
    let evs = drive(&mut core, &[press(KeyCode::Char('a'))]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, first, "the sink on screen hears its key");
    // The focus moves to the sink that is off the viewport, and nothing
    // scrolls: no reveal, no wheel — the worst case, where the node is
    // drawn nowhere a pointer could reach it.
    let (_, second_again) = build(&mut core, true);
    assert_eq!(second_again, second);
    let evs = drive(&mut core, &[press(KeyCode::Char('b'))]);
    assert_eq!(evs.len(), 1, "the key was delivered");
    assert_eq!(evs[0].key, second);
    assert_eq!(
        evs[0]
            .payload
            .get("tag")
            .and_then(|t| t.get("pane"))
            .and_then(Value::as_int),
        Some(1),
        "with the sink's own tag"
    );
    // A click out there still finds nothing: the pointer's rule is
    // unchanged, and only the keyboard's was wrong.
    let evs = click_at(&mut core, 600.0, 100.0);
    assert!(evs.is_empty(), "no pointer event past the clip: {evs:?}");
}

/// The same for a sink a modal shuts out, and a disabled one: neither is
/// the keyboard's, in the tree or in the hit list.
#[test]
fn a_sink_a_modal_shuts_out_hears_nothing_wherever_it_is_drawn() {
    let mut core = Core::new();
    let build = |core: &mut Core, modal: bool| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let sink = ui.leaf_keyed(
            "sink",
            NodeSpec::column()
                .fill()
                .on_key(Value::map([("pane", Value::Int(0))])),
        );
        if modal {
            ui.leaf_keyed(
                "dialog",
                NodeSpec::column()
                    .size(100.0, 50.0)
                    .float(FloatConfig::parent().at(Align::Center, Align::Center))
                    .modal("dlg"),
            );
        }
        ui.take_key_focus(sink);
        ui.finish();
        sink
    };
    let sink = build(&mut core, false);
    let evs = drive(&mut core, &[press(KeyCode::Char('a'))]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, sink);
    build(&mut core, true);
    let evs = drive(&mut core, &[press(KeyCode::Char('b'))]);
    assert!(keys(&evs).is_empty(), "the modal has the keyboard: {evs:?}");
}
