//! Keys a focused control does not claim bubble to the nearest enclosing
//! key sink (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`): an app
//! shell gets its shortcuts while the Tab ring keeps working underneath it,
//! and the controls in the ring keep the keys the core presses them with.

use kui_core::{
    Core, EditKey, EventKind, InputEvent, Key, KeyCode, KeyMods, KeyPress, Mods, NodeSpec, Role,
    Size, UiEvent, Value,
};

const H: f32 = 20.0;

/// The keys of the shell fixture, in tree order.
struct Shell {
    shell: Key,
    go: Key,
    vol: Key,
    inner: Key,
    deep: Key,
}

/// One `on_key` shell wrapping a ring: a button (something to activate), a
/// slider (arrows, and nothing to activate — its `on_change` is what moves
/// it), and a nested sink with a button of its own — the shape every app shell has, and the shape the
/// pomodoro in backlog F7 could not have.
fn shell(core: &mut Core, disabled_shell: bool) -> Shell {
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = || NodeSpec::row().size(100.0, H);
    let mut keys = (Key::ROOT, Key::ROOT, Key::ROOT, Key::ROOT);
    let shell = ui.with_keyed(
        "shell",
        NodeSpec::column()
            .fill()
            .on_key("shell")
            .key_up()
            .disabled(disabled_shell),
        |ui| {
            keys.0 = ui.leaf_keyed("go", row().on_click("go").label("Go"));
            keys.1 = ui.leaf_keyed(
                "vol",
                row()
                    .role(Role::Slider)
                    .on_drag("vol")
                    .on_change("vol")
                    .label("Volume")
                    .value_now(3.0)
                    .value_min(0.0)
                    .value_max(10.0),
            );
            keys.2 = ui.with_keyed("inner", row().on_key("inner"), |ui| {
                keys.3 = ui.leaf_keyed("deep", row().on_click("deep").label("Deep"));
            });
        },
    );
    ui.finish();
    Shell {
        shell,
        go: keys.0,
        vol: keys.1,
        inner: keys.2,
        deep: keys.3,
    }
}

/// The same ring with no sink over it: what every key below has to keep
/// doing where nothing is listening.
fn bare(core: &mut Core) -> (Key, Key) {
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = || NodeSpec::row().size(100.0, H);
    let go = ui.leaf_keyed("go", row().on_click("go").label("Go"));
    let vol = ui.leaf_keyed("vol", row().role(Role::Slider).on_drag("vol").label("V"));
    ui.finish();
    (go, vol)
}

/// Everything a real window sends for one press, in the order it sends it
/// (`crates/kui-native/src/lib.rs`): the raw press first, then the editing key it
/// maps to, then the text it would insert. The order is why a claim is
/// resolved from the node and the key rather than from what a handler did.
fn window_press(
    core: &mut Core,
    code: KeyCode,
    mods: KeyMods,
    ek: Option<EditKey>,
    text: Option<&str>,
) -> Vec<UiEvent> {
    let mut out = core.handle_input(InputEvent::KeyDown(KeyPress::new(code, mods)));
    if let Some(ek) = ek {
        // `doc` the way the runner derives it (`KeyMods::primary`): the
        // platform's primary modifier and not "either", which no driver
        // sends (AR10's F6 trap).
        let m = Mods {
            shift: mods.shift,
            word: mods.alt,
            doc: mods.primary(),
            ctrl: false,
        };
        out.extend(core.handle_input(InputEvent::Key(ek, m)));
    }
    if let Some(t) = text {
        out.extend(core.handle_input(InputEvent::Text(t.to_string())));
    }
    out
}

fn letter(core: &mut Core, c: char) -> Vec<UiEvent> {
    window_press(
        core,
        KeyCode::Char(c),
        KeyMods::default(),
        None,
        Some(&c.to_string()),
    )
}

fn space(core: &mut Core) -> Vec<UiEvent> {
    window_press(core, KeyCode::Space, KeyMods::default(), None, Some(" "))
}

fn named(core: &mut Core, code: KeyCode, ek: EditKey) -> Vec<UiEvent> {
    window_press(core, code, KeyMods::default(), Some(ek), None)
}

/// `(key, phase, code)` of every key event in a batch: which sink heard
/// it, which phase, which key — read from the `key` field of the sink's
/// message (`{tag: "shell", key: {phase, code, ...}}`).
fn sink_events(evs: &[UiEvent]) -> Vec<(Key, String, String)> {
    evs.iter()
        .filter(|e| e.is(EventKind::Key))
        .map(|e| {
            let at = |k: &str| {
                e.data()
                    .and_then(|d| d.get(k))
                    .and_then(Value::as_str)
                    .unwrap_or("")
                    .to_string()
            };
            (e.key, at("phase"), at("code"))
        })
        .collect()
}

#[test]
fn tab_walks_the_ring_inside_a_shell_sink() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = named(&mut core, KeyCode::Tab, EditKey::Tab);
    // Tab is the ring's wherever focus is: it moves, and the shell above
    // hears nothing at all — otherwise a shortcut layer would be a Tab
    // trap, which is the bug this ADR exists to end.
    assert_eq!(core.focus(), Some(k.vol));
    assert_eq!(sink_events(&evs), vec![]);
}

#[test]
fn a_key_the_control_does_not_claim_reaches_the_sink_above_it() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = letter(&mut core, 'm');
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "m".to_string())]
    );
    assert_eq!(core.focus(), Some(k.go), "a bubbled key moves no focus");
}

#[test]
fn space_presses_a_button_and_never_reaches_the_shell() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = space(&mut core);
    assert_eq!(sink_events(&evs), vec![]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, k.go);
    assert_eq!(evs[0].payload.get_str("tag"), Some("go"));
}

#[test]
fn enter_presses_a_button_and_never_reaches_the_shell() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = named(&mut core, KeyCode::Enter, EditKey::Enter);
    assert_eq!(sink_events(&evs), vec![]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.get_str("tag"), Some("go"));
}

#[test]
fn a_slider_keeps_its_arrows() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.vol));
    let evs = named(&mut core, KeyCode::Right, EditKey::Right);
    assert_eq!(sink_events(&evs), vec![]);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, k.vol);
    // One step up from 3, proposed through the slider's `on_change`.
    assert_eq!(evs[0].event, EventKind::Change);
    assert_eq!(evs[0].payload.get_str("tag"), Some("vol"));
    let value = evs[0].data().and_then(|d| d.get("value")?.as_float());
    assert!(value.is_some_and(|v| v > 3.0), "{value:?}");
}

#[test]
fn a_control_with_nothing_to_activate_lets_space_bubble() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.vol));
    // The slider has no click payload, so Space presses nothing: the core
    // claims only the keys it would itself act on, and the app's global
    // Space keeps working while a slider holds focus.
    let evs = space(&mut core);
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "space".to_string())]
    );
    assert!(evs.iter().all(|e| e.key == k.shell));
}

#[test]
fn a_chord_bubbles_even_when_the_bare_key_is_the_controls() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let cmd = KeyMods::NONE.with_super();
    let evs = window_press(&mut core, KeyCode::Enter, cmd, Some(EditKey::Enter), None);
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "enter".to_string())]
    );
    assert!(
        evs.iter().all(|e| e.key == k.shell),
        "the button must not also be pressed"
    );
}

/// The modifier `KeyMods::primary` ignores — Ctrl on macOS, Super
/// elsewhere. A chord on it is the one the second channel used to fold
/// away.
fn non_primary() -> KeyMods {
    if cfg!(target_os = "macos") {
        KeyMods::NONE.with_ctrl()
    } else {
        KeyMods::NONE.with_super()
    }
}

/// AR10: `edit_event` folds the modifiers to `word: alt, doc: primary()`,
/// so the non-primary of Ctrl/Super was gone by the time the `Key` arm
/// asked whether the press bubbled — the raw press had bubbled as a
/// chord, and then Enter pressed the button as well, the disagreement
/// ADR 0011 decision 3 forbids. The chord bit is the `KeyDown`'s now,
/// on both channels.
#[test]
fn a_chord_on_the_non_primary_modifier_does_not_also_press_the_control() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = window_press(
        &mut core,
        KeyCode::Enter,
        non_primary(),
        Some(EditKey::Enter),
        None,
    );
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "enter".to_string())]
    );
    assert!(
        evs.iter().all(|e| e.key == k.shell),
        "the button must not also be pressed: {evs:?}"
    );
}

/// Space under a modifier other than Shift is a chord like any other:
/// it reaches the sink as a press and inserts nothing, presses nothing.
/// Before, `edit_event` said " " whatever was held, so Ctrl+Space (an IME
/// toggle, an Emacs mark) typed a space and clicked a button both.
#[test]
fn space_under_a_modifier_is_a_chord_and_types_nothing() {
    let ctrl_space = KeyPress::new(KeyCode::Space, non_primary());
    assert_eq!(ctrl_space.edit_event(), None, "no text channel for a chord");
    let shift_space = KeyPress::new(KeyCode::Space, KeyMods::NONE.with_shift());
    assert_eq!(
        shift_space.edit_event(),
        Some(InputEvent::Text(" ".into())),
        "Shift+Space is still a space"
    );

    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = core.press(ctrl_space);
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "space".to_string())]
    );
    assert!(evs.iter().all(|e| e.key == k.shell), "{evs:?}");
}

/// The `Text` channel asks the press's chord bit too: a driver that still
/// sends a text for a chorded Space — as every driver did before AR10 —
/// has it bubble with the press rather than press the control.
#[test]
fn a_text_after_a_chorded_press_bubbles_with_it() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = window_press(&mut core, KeyCode::Space, non_primary(), None, Some(" "));
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "space".to_string())]
    );
    assert!(evs.iter().all(|e| e.key == k.shell), "{evs:?}");
    // And a bare `Text` with no press before it — a test driving one
    // channel — has no chord to agree with, and presses as it always did.
    let evs = core.handle_input(InputEvent::Text(" ".into()));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.get_str("tag"), Some("go"));
}

#[test]
fn the_nearest_sink_hears_it_and_the_ones_above_do_not() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.deep));
    let evs = letter(&mut core, 'm');
    assert_eq!(
        sink_events(&evs),
        vec![(k.inner, "down".to_string(), "m".to_string())]
    );
}

#[test]
fn a_bubbled_press_is_held_and_its_release_follows_it() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let mut evs = letter(&mut core, 'm');
    evs.extend(core.handle_input(InputEvent::KeyUp(KeyPress::new(
        KeyCode::Char('m'),
        KeyMods::default(),
    ))));
    assert_eq!(
        sink_events(&evs),
        vec![
            (k.shell, "down".to_string(), "m".to_string()),
            (k.shell, "up".to_string(), "m".to_string()),
        ]
    );
}

#[test]
fn a_claimed_key_is_never_held_so_it_has_no_release() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let mut evs = space(&mut core);
    evs.extend(core.handle_input(InputEvent::KeyUp(KeyPress::new(
        KeyCode::Space,
        KeyMods::default(),
    ))));
    assert_eq!(sink_events(&evs), vec![]);
}

#[test]
fn focus_leaving_mid_hold_releases_the_bubbled_key_first() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let mut evs = letter(&mut core, 'm');
    // Tab moves focus to the slider; the key still down was heard by the
    // shell, so its release is the shell's too.
    evs.extend(named(&mut core, KeyCode::Tab, EditKey::Tab));
    assert_eq!(core.focus(), Some(k.vol));
    assert_eq!(
        sink_events(&evs),
        vec![
            (k.shell, "down".to_string(), "m".to_string()),
            (k.shell, "up".to_string(), "m".to_string()),
        ]
    );
}

#[test]
fn escape_bubbles_where_a_sink_is_listening() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.go));
    let evs = named(&mut core, KeyCode::Escape, EditKey::Escape);
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "escape".to_string())]
    );
    assert_eq!(
        core.focus(),
        Some(k.go),
        "the shell owns Escape, so it does not also blur"
    );
}

#[test]
fn escape_still_lets_go_of_a_control_with_nothing_listening() {
    let mut core = Core::new();
    let (go, _) = bare(&mut core);
    core.set_focus(Some(go));
    let evs = named(&mut core, KeyCode::Escape, EditKey::Escape);
    assert_eq!(sink_events(&evs), vec![]);
    assert_eq!(core.focus(), None);
}

#[test]
fn a_key_nothing_claims_and_nothing_hears_does_what_it_always_did() {
    let mut core = Core::new();
    let (_, vol) = bare(&mut core);
    core.set_focus(Some(vol));
    // No sink anywhere: Space on a slider is still the nothing it was.
    let evs = space(&mut core);
    assert!(evs.is_empty());
    assert_eq!(core.focus(), Some(vol));
}

#[test]
fn a_sink_that_holds_focus_still_keeps_every_key() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    core.set_focus(Some(k.shell));
    let evs = named(&mut core, KeyCode::Tab, EditKey::Tab);
    assert_eq!(
        sink_events(&evs),
        vec![(k.shell, "down".to_string(), "tab".to_string())]
    );
    assert_eq!(
        core.focus(),
        Some(k.shell),
        "ADR 0002 decision 3: a sink owns its keyboard, Tab included"
    );
}

#[test]
fn a_disabled_sink_hears_nothing() {
    let mut core = Core::new();
    let k = shell(&mut core, true);
    core.set_focus(Some(k.go));
    let evs = letter(&mut core, 'm');
    assert_eq!(sink_events(&evs), vec![]);
}

/// A shell sink around an app, and a modal dialog over it with a button of
/// its own: the app behind is inert, and its shortcuts are part of what is
/// inert (`docs/adr/0003-modal-surfaces.md`).
fn modal_over_a_shell(core: &mut Core) -> (Key, Key) {
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut ok = Key::ROOT;
    let shell = ui.with_keyed("shell", NodeSpec::column().fill().on_key("shell"), |ui| {
        ui.leaf_keyed(
            "go",
            NodeSpec::row().size(100.0, H).on_click("go").label("Go"),
        );
        ui.with_keyed(
            "dialog",
            NodeSpec::column()
                .size(120.0, 80.0)
                .modal("dlg")
                .label("Settings"),
            |ui| {
                ok = ui.leaf_keyed(
                    "ok",
                    NodeSpec::row().size(80.0, H).on_click("ok").label("OK"),
                );
            },
        );
    });
    ui.finish();
    (shell, ok)
}

#[test]
fn bubbling_stops_at_the_modal_boundary() {
    let mut core = Core::new();
    let (_, ok) = modal_over_a_shell(&mut core);
    assert_eq!(core.focus(), Some(ok), "the modal takes focus into itself");
    let evs = letter(&mut core, 'm');
    assert_eq!(
        sink_events(&evs),
        vec![],
        "a shell outside the dialog is as inert as the app it wraps"
    );
}

/// A sink that is a stop only for being a sink, with stops inside it, is a
/// keymap over them and no Tab stop of its own (ADR 0011, amended
/// 2026-10-26): Tab from nothing lands on the shell's first control, and
/// the ring is the controls alone — the inner sink, around a button, is
/// one too. The Kit gallery's shell was the first stop, and once there it
/// kept every Tab: Tab moved nothing anywhere in the app.
#[test]
fn a_shell_sink_around_tab_stops_is_no_stop_of_its_own() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    let mut ring = Vec::new();
    for _ in 0..4 {
        core.focus_next(true);
        ring.push(core.focus().unwrap());
    }
    assert_eq!(ring, vec![k.go, k.vol, k.deep, k.go], "the controls, round");
    core.focus_next(false);
    assert_eq!(core.focus(), Some(k.deep), "Shift-Tab round the other way");
    assert!(!ring.contains(&k.shell) && !ring.contains(&k.inner));
}

/// A sink with no stop inside it — a terminal, a canvas — is a stop as it
/// always was.
#[test]
fn a_sink_with_nothing_focusable_inside_is_a_stop() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let go = ui.leaf_keyed(
        "go",
        NodeSpec::row().size(100.0, H).on_click("go").label("Go"),
    );
    let term = ui.with_keyed(
        "term",
        NodeSpec::row().size(100.0, H).on_key("term"),
        |ui| {
            ui.text("$ ", kui_core::TextStyle::new(12.0));
        },
    );
    ui.finish();
    core.focus_next(true);
    assert_eq!(core.focus(), Some(go));
    core.focus_next(true);
    assert_eq!(core.focus(), Some(term), "the sink is a stop");
}

/// A press on a control inside a shell focuses the control (ADR 0002,
/// decision 9 as amended by backlog C135), and Tab goes on from it — as a
/// browser's Tab goes on from a click — not from the top. Focus moved
/// anywhere else lets the press's place go.
#[test]
fn tab_handed_on_by_a_shell_a_press_focused_goes_on_from_the_press() {
    let mut core = Core::new();
    let k = shell(&mut core, false);
    kui_core::testing::click_at(&mut core, 50.0, H * 1.5);
    assert_eq!(
        core.focus(),
        Some(k.vol),
        "the press on the slider focuses the slider"
    );
    core.focus_next(true);
    assert_eq!(core.focus(), Some(k.deep), "on from the slider pressed");
    kui_core::testing::click_at(&mut core, 50.0, H * 0.5);
    assert_eq!(core.focus(), Some(k.go));
    core.focus_next(false);
    assert_eq!(
        core.focus(),
        Some(k.deep),
        "back from the button pressed, round the start"
    );
    // Focus put on the shell by the view starts from the shell itself.
    core.set_focus(Some(k.go));
    core.set_focus(Some(k.shell));
    core.focus_next(true);
    assert_eq!(
        core.focus(),
        Some(k.go),
        "a shell focused by the view goes on to its first stop"
    );
}
