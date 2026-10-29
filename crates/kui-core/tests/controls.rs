//! The stock controls (`docs/adr/0034-stock-controls-over-the-roles.md`):
//! a toggle is drawn from the state the view declares and presses through
//! its `on_click`, and a slider that declared `on_change` has the core
//! turn the pointer and the keys into proposed values.

use kui_core::access::{AccessAction, AccessRequest, Role};
use kui_core::input::{InputEvent, KeyCode, KeyMods, KeyPress, UiEvent};
use kui_core::spec::NodeSpec;
use kui_core::testing::{kinds, press, release};
use kui_core::{Core, Key, Size, Value, Vec2, widgets};

/// The access node `key` names in the last frame's tree.
fn node(core: &mut Core, key: Key) -> kui_core::access::AccessNode {
    core.access_tree()
        .nodes
        .iter()
        .find(|n| n.key == key)
        .cloned()
        .expect("the node is in the access tree")
}

fn key(core: &mut Core, code: KeyCode) -> Vec<UiEvent> {
    core.press(KeyPress::new(code, KeyMods::default()))
}

/// A slider's proposal: the `change` field's value and phase.
fn value(ev: &UiEvent) -> (f64, &str) {
    let change = ev.data().expect("a change field");
    (
        change.get_float("value").unwrap(),
        change.get_str("phase").unwrap(),
    )
}

#[test]
fn a_checkbox_reads_its_state_and_presses_through_its_payload() {
    let mut core = Core::new();
    let build = |core: &mut Core, on: bool, mixed: bool| {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        let m = ui.metrics();
        let a = widgets::checkbox(&mut ui, "Mute", on, "mute");
        let b = widgets::toggle_with(
            &mut ui,
            widgets::Toggle::Checkbox,
            "all",
            "Select all",
            widgets::toggle_spec(&m).mixed(mixed).on_click("all"),
            None,
        );
        ui.finish();
        (a, b)
    };
    let keys = build(&mut core, false, true);
    let mute = node(&mut core, keys.0);
    assert_eq!(mute.role, Role::Checkbox);
    assert_eq!(mute.name.as_deref(), Some("Mute"));
    assert_eq!((mute.checked, mute.mixed), (Some(false), false));
    let all = node(&mut core, keys.1);
    assert!(all.mixed, "the select-all box reads as mixed");

    // Tab to it and press Space: the payload, once.
    key(&mut core, KeyCode::Tab);
    assert_eq!(core.focus(), Some(keys.0));
    let evs = key(&mut core, KeyCode::Space);
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.get_str("tag"), Some("mute"));

    build(&mut core, true, false);
    assert_eq!(node(&mut core, keys.0).checked, Some(true));
}

#[test]
fn a_radio_group_s_arrows_press_the_radio_they_land_on() {
    let mut core = Core::new();
    let build = |core: &mut Core, current: usize| {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        let k = widgets::radio_group(
            &mut ui,
            "Theme",
            &["Light", "Dark", "System"],
            Some(current),
            |i| Value::Int(i as i64),
        );
        ui.finish();
        k
    };
    let group = build(&mut core, 0);
    assert_eq!(node(&mut core, group).role, Role::RadioGroup);
    key(&mut core, KeyCode::Tab);
    let evs = key(&mut core, KeyCode::Down);
    assert_eq!(evs.len(), 1, "moving the choice pressed the next radio");
    assert_eq!(evs[0].payload.get_int("tag"), Some(1));
    build(&mut core, 1);
    let checked: Vec<Option<bool>> = core
        .access_tree()
        .nodes
        .iter()
        .filter(|n| n.role == Role::Radio)
        .map(|n| n.checked)
        .collect();
    assert_eq!(checked, [Some(false), Some(true), Some(false)]);
}

#[test]
fn a_switch_is_a_switch() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let k = widgets::switch(&mut ui, "Wi-Fi", true, "wifi");
    ui.finish();
    let n = node(&mut core, k);
    assert_eq!((n.role, n.checked), (Role::Switch, Some(true)));
}

/// A 216-wide slider over 0..100 by 10, laid out at the origin: its
/// content box — the track the pointer reads — runs from 8 to 208.
fn slider(core: &mut Core, now: f32, disabled: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let m = ui.metrics();
    let spec = widgets::slider_spec(&m)
        .width(kui_core::Sizing::Fixed(216.0))
        .value_now(now)
        .value_min(0.0)
        .value_max(100.0)
        .value_step(10.0)
        .on_change("vol")
        .disabled(disabled);
    let k = widgets::slider_with(&mut ui, "Volume", spec, None);
    ui.finish();
    k
}

#[test]
fn a_press_proposes_the_value_under_it_and_a_drag_each_new_step() {
    let mut core = Core::new();
    let k = slider(&mut core, 0.0, false);
    let n = node(&mut core, k);
    assert_eq!(n.role, Role::Slider);
    assert_eq!(n.name.as_deref(), Some("Volume"));
    assert_eq!(n.step, Some(10.0));

    // 8 + 0.31 * 200: 31, snapped to 30.
    let evs = press(&mut core, Vec2::new(70.0, 8.0));
    assert_eq!(kinds(&evs).last(), Some(&"change"));
    let change = evs
        .iter()
        .find(|e| kinds(std::slice::from_ref(e)) == ["change"])
        .unwrap();
    assert_eq!(value(change), (30.0, "move"));
    assert_eq!(change.key, k);
    assert_eq!(change.payload.get_str("tag"), Some("vol"));

    // Within the same step: nothing. Past it: the next one.
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(72.0, 8.0)));
    assert!(kinds(&evs).iter().all(|k| *k != "change"));
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 40.0)));
    let moved: Vec<(f64, &str)> = evs
        .iter()
        .filter(|e| e.event.name() == "change")
        .map(value)
        .collect();
    assert_eq!(moved, [(70.0, "move")]);
    // Off the end: clamped.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(900.0, 8.0)));
    let evs = release(&mut core);
    let ended: Vec<(f64, &str)> = evs
        .iter()
        .filter(|e| e.event.name() == "change")
        .map(value)
        .collect();
    assert_eq!(ended, [(100.0, "end")]);
    assert!(
        evs.iter().all(|e| !e.is(kui_core::EventKind::Click)),
        "a slide is not a click"
    );
}

#[test]
fn the_keys_move_it_by_its_step_and_to_its_ends() {
    let mut core = Core::new();
    let k = slider(&mut core, 40.0, false);
    key(&mut core, KeyCode::Tab);
    assert_eq!(core.focus(), Some(k));
    let step = |core: &mut Core, code| {
        let evs = key(core, code);
        evs.iter()
            .filter(|e| e.event.name() == "change")
            .map(|e| value(e).0)
            .collect::<Vec<_>>()
    };
    assert_eq!(step(&mut core, KeyCode::Right), [50.0]);
    assert_eq!(step(&mut core, KeyCode::Down), [30.0]);
    assert_eq!(step(&mut core, KeyCode::PageUp), [100.0]);
    assert_eq!(step(&mut core, KeyCode::Home), [0.0]);
    assert_eq!(step(&mut core, KeyCode::End), [100.0]);

    // At the top, Up proposes nothing.
    slider(&mut core, 100.0, false);
    assert_eq!(step(&mut core, KeyCode::Up), [] as [f64; 0]);

    // Assistive technology's increment is a step, as the arrow is.
    slider(&mut core, 40.0, false);
    let evs = core.handle_input(InputEvent::Access(AccessRequest::new(
        k,
        AccessAction::Increment,
    )));
    assert_eq!(value(&evs[0]), (50.0, "end"));
}

#[test]
fn a_disabled_slider_proposes_nothing() {
    let mut core = Core::new();
    let k = slider(&mut core, 40.0, true);
    let evs = press(&mut core, Vec2::new(70.0, 8.0));
    release(&mut core);
    assert!(kinds(&evs).iter().all(|k| *k != "change"));
    let evs = core.handle_input(InputEvent::Access(AccessRequest::new(
        k,
        AccessAction::Increment,
    )));
    assert!(evs.is_empty());
}

/// A slider without `on_change` has no message to propose a value in
/// (ADR 0047, decision 5): the arrows and a reader's requests move
/// nothing, and say so once as `slider-without-handler`; its Page, Home
/// and End keys are not its own.
#[test]
fn a_slider_without_on_change_does_not_move_and_warns() {
    use kui_core::diag::SLIDER_WITHOUT_HANDLER;
    use kui_core::testing::codes;

    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let k = ui.leaf_keyed(
        "vol",
        NodeSpec::row()
            .width(kui_core::Sizing::Fixed(100.0))
            .height(kui_core::Sizing::Fixed(10.0))
            .role(Role::Slider)
            .label("Volume")
            .value_now(3.0)
            .on_drag("vol"),
    );
    ui.finish();
    core.take_warnings();
    key(&mut core, KeyCode::Tab);
    assert_eq!(core.focus(), Some(k));
    assert!(key(&mut core, KeyCode::Right).is_empty(), "no access event");
    assert!(key(&mut core, KeyCode::End).is_empty());

    // A reader's set value is nothing too (RG42's request, refused).
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(k, AccessAction::SetValue).with_value("7.5"),
    ));
    assert!(evs.is_empty());
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [SLIDER_WITHOUT_HANDLER], "{ws:?}");
    assert_eq!(ws[0].key, k);
}

/// Windows' UI Automation moves a slider only by setting it — RangeValue
/// has no increment — and the request was dropped (backlog RG42): a set
/// value is a proposal, snapped and clamped as the pointer's is.
#[test]
fn a_reader_s_set_value_is_proposed_snapped_and_clamped() {
    let mut core = Core::new();
    let k = slider(&mut core, 40.0, false);
    assert!(node(&mut core, k).supports(AccessAction::SetValue));
    let set = |core: &mut Core, v: &str| {
        let evs = core.handle_input(InputEvent::Access(
            AccessRequest::new(k, AccessAction::SetValue).with_value(v),
        ));
        evs.iter()
            .map(value)
            .map(|(v, p)| (v, p.to_string()))
            .collect::<Vec<_>>()
    };
    assert_eq!(set(&mut core, "72"), [(70.0, "end".to_string())]);
    assert_eq!(set(&mut core, "250"), [(100.0, "end".to_string())]);
    assert_eq!(set(&mut core, "41"), [], "where it is: nothing");
    assert_eq!(set(&mut core, "25 minutes"), [], "not a number: nothing");

    let k = slider(&mut core, 40.0, true);
    assert!(!node(&mut core, k).supports(AccessAction::SetValue));
    assert_eq!(set(&mut core, "72"), []);
}

/// A step of a tenth lands on the decimal it names on the wire.
#[test]
fn a_fractional_step_proposes_the_decimal() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let k = widgets::slider(&mut ui, "Gain", 0.2, 0.0, 1.0, 0.1, "gain");
    ui.finish();
    key(&mut core, KeyCode::Tab);
    assert_eq!(core.focus(), Some(k));
    let evs = key(&mut core, KeyCode::Right);
    assert_eq!(value(&evs[0]).0, 0.3);
}

/// The track's fill and the track: the two solids a stock progress bar
/// draws, the fill first by width (it is the narrower or as wide).
fn bar_quads(core: &mut Core) -> (kui_core::Rect, kui_core::Rect) {
    let mut q: Vec<kui_core::Rect> = kui_core::testing::solids(core)
        .into_iter()
        .map(|q| q.rect)
        .filter(|r| (r.h - 4.0).abs() < 0.01)
        .collect();
    assert_eq!(q.len(), 2, "a track and a fill: {q:?}");
    q.sort_by(|a, b| a.w.total_cmp(&b.w));
    (q[0], q[1])
}

/// The stock progress bar (backlog F267): the progress role over the
/// value rows it was given, named by its label, a fill as far along the
/// track as the value is, and no focus, no press, no change — a key or a
/// click on it does nothing.
#[test]
fn a_progress_bar_fills_to_its_value_and_reads_it() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let k = widgets::progress(&mut ui, "Upload", Some(25.0), 0.0, 100.0);
    ui.finish();
    let n = node(&mut core, k);
    assert_eq!(n.role, Role::Progress);
    assert_eq!(n.name.as_deref(), Some("Upload"));
    assert_eq!(
        (n.number, n.min, n.max),
        (Some(25.0), Some(0.0), Some(100.0))
    );
    assert_eq!(
        n.actions,
        AccessAction::ScrollIntoView.bit(),
        "nothing to do to it"
    );
    let (fill, track) = bar_quads(&mut core);
    assert_eq!(track.x, fill.x, "the fill starts where the track does");
    assert!(
        (fill.w - track.w / 4.0).abs() < 0.5,
        "{fill:?} of {track:?}"
    );

    let evs = press(&mut core, Vec2::new(track.x + 5.0, track.y + 2.0));
    let evs = [evs, release(&mut core), key(&mut core, KeyCode::Tab)].concat();
    assert!(kinds(&evs).is_empty(), "{:?}", kinds(&evs));
    assert_eq!(core.focus(), None);
}

/// Busy: no value, so the role carries no number (a reader's
/// indeterminate) and the fill is a still third of the track, in its
/// middle — not at the start, where it would read as a third done.
#[test]
fn a_busy_progress_bar_has_no_number_and_fills_its_middle_third() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let k = widgets::progress(&mut ui, "Connecting", None, 0.0, 100.0);
    ui.finish();
    let n = node(&mut core, k);
    assert_eq!((n.role, n.number), (Role::Progress, None));
    let (fill, track) = bar_quads(&mut core);
    assert!(
        (fill.w - track.w / 3.0).abs() < 0.5,
        "{fill:?} of {track:?}"
    );
    let centre = |r: kui_core::Rect| r.x + r.w / 2.0;
    assert!(
        (centre(fill) - centre(track)).abs() < 0.5,
        "{fill:?} in {track:?}"
    );
}

/// `role(Meter)` on the spec makes the same drawing a meter; any other
/// role is the progress bar's, as the slider's role is its own. A
/// `value_text` is its reading, and a hint is its description.
#[test]
fn a_progress_spec_with_the_meter_role_is_a_meter() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    let m = ui.metrics();
    let meter = widgets::progress_with(
        &mut ui,
        "Battery",
        widgets::progress_spec(&m)
            .role(Role::Meter)
            .value_now(0.8)
            .value_max(1.0)
            .value_text("80 percent, charging"),
        Some("Charged by noon"),
    );
    let other = widgets::progress_with(
        &mut ui,
        "Sync",
        widgets::progress_spec(&m).role(Role::Button).value_now(1.0),
        None,
    );
    ui.finish();
    let n = node(&mut core, meter);
    assert_eq!(n.role, Role::Meter);
    assert_eq!(n.value.as_deref(), Some("80 percent, charging"));
    assert_eq!(n.description.as_deref(), Some("Charged by noon"));
    assert_eq!(node(&mut core, other).role, Role::Progress);
}
