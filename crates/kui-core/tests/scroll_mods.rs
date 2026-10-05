//! An `on_scroll` node that names modifiers (`scroll_mods`, backlog F122)
//! hears a scroll gesture begun with one of them held, ahead of every
//! scroller and plain handler under the pointer, and no other wheel.

use kui_core::{Core, InputEvent, KeyMods, NodeSpec, Size, UiEvent, Value, Vec2};

const CTRL_OR_SUPER: KeyMods = KeyMods::NONE.with_ctrl().with_super();

/// A window-wide `zoom` handler for Ctrl or Super; in it a list that
/// scrolls, whose first row is a plain handler (`pane`) and whose second
/// (with `canvas`) names Ctrl itself.
fn frame(core: &mut Core, canvas: bool) {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let zoom = NodeSpec::column()
        .fill()
        .on_scroll(Value::str("zoom"))
        .scroll_mods(CTRL_OR_SUPER);
    ui.with_keyed("zoom", zoom, |ui| {
        ui.with_keyed("list", NodeSpec::column().fill().scroll_y(), |ui| {
            ui.leaf_keyed(
                "pane",
                NodeSpec::row()
                    .grow_width()
                    .height(50.0)
                    .on_scroll(Value::str("pane")),
            );
            let mut second = NodeSpec::row().grow_width().height(50.0);
            if canvas {
                second = second
                    .on_scroll(Value::str("canvas"))
                    .scroll_mods(KeyMods::NONE.with_ctrl());
            }
            ui.leaf_keyed("second", second);
            ui.leaf_keyed("rest", NodeSpec::row().grow_width().height(900.0));
        });
    });
    ui.finish();
}

fn hold(core: &mut Core, mods: KeyMods) {
    core.handle_input(InputEvent::Modifiers(mods));
}

fn wheel(core: &mut Core, y: f32, dy: f32, begins: bool) -> Vec<UiEvent> {
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, y)));
    core.handle_input(InputEvent::ScrollGesture {
        delta: Vec2::new(0.0, dy),
        begins,
    })
}

/// Who heard the wheel, by tag.
fn heard(evs: &[UiEvent]) -> Vec<&str> {
    evs.iter()
        .filter(|e| e.scroll().is_some())
        .filter_map(|e| e.tag().and_then(Value::as_str))
        .collect()
}

fn offset(core: &mut Core) -> f32 {
    frame(core, false);
    let list = core.key_of("list").expect("the list");
    core.scroll_offset(list).y
}

const PANE: f32 = 25.0;
const SECOND: f32 = 75.0;
const REST: f32 = 150.0;

#[test]
fn a_named_modifier_held_is_the_handlers_ahead_of_every_scroller() {
    let mut core = Core::new();
    frame(&mut core, false);
    for mods in [KeyMods::NONE.with_ctrl(), KeyMods::NONE.with_super()] {
        hold(&mut core, mods);
        // Over the list's rows, and over a plain handler inside it.
        for y in [REST, PANE] {
            let evs = wheel(&mut core, y, -40.0, true);
            assert_eq!(heard(&evs), ["zoom"], "at {y}: {evs:?}");
            let s = evs[0].scroll().expect("a scroll");
            assert_eq!(s.delta, Vec2::new(0.0, -40.0));
            assert_eq!(s.mods, Some(mods), "the modifiers the gesture began with");
        }
        assert_eq!(offset(&mut core), 0.0, "the list stood still");
    }
}

#[test]
fn a_wheel_with_none_of_them_held_passes_the_handler_by() {
    for mods in [KeyMods::NONE, KeyMods::NONE.with_shift().with_alt()] {
        let mut core = Core::new();
        frame(&mut core, false);
        hold(&mut core, mods);
        let evs = wheel(&mut core, PANE, -40.0, true);
        assert_eq!(heard(&evs), ["pane"], "{evs:?}");
        assert_eq!(evs[0].scroll().unwrap().mods, None, "a plain handler's");
        assert!(heard(&wheel(&mut core, REST, -40.0, true)).is_empty());
        assert_eq!(offset(&mut core), 40.0, "the list scrolled");
    }
    // At the list's start, rolling up: the list passes the gesture on,
    // and the handler around it is still not asked.
    let mut core = Core::new();
    frame(&mut core, false);
    assert!(heard(&wheel(&mut core, REST, 40.0, true)).is_empty());
}

#[test]
fn the_innermost_handler_that_names_the_modifier_wins() {
    let mut core = Core::new();
    frame(&mut core, true);
    hold(&mut core, KeyMods::NONE.with_ctrl());
    assert_eq!(heard(&wheel(&mut core, SECOND, -40.0, true)), ["canvas"]);
    assert_eq!(heard(&wheel(&mut core, REST, -40.0, true)), ["zoom"]);
    // Super is the outer one's alone.
    hold(&mut core, KeyMods::NONE.with_super());
    assert_eq!(heard(&wheel(&mut core, SECOND, -40.0, true)), ["zoom"]);
}

#[test]
fn a_gesture_stays_what_it_began_as() {
    let mut core = Core::new();
    frame(&mut core, false);
    // Begun with the key held: its glide is the handler's with the key up.
    hold(&mut core, KeyMods::NONE.with_super());
    assert_eq!(heard(&wheel(&mut core, REST, -10.0, true)), ["zoom"]);
    hold(&mut core, KeyMods::NONE);
    let evs = wheel(&mut core, REST, -10.0, false);
    assert_eq!(heard(&evs), ["zoom"]);
    assert_eq!(
        evs[0].scroll().unwrap().mods,
        Some(KeyMods::NONE.with_super()),
        "as it began"
    );
    assert_eq!(offset(&mut core), 0.0);
    // Begun without: the key coming down in its glide changes nothing.
    assert!(heard(&wheel(&mut core, REST, -10.0, true)).is_empty());
    hold(&mut core, KeyMods::NONE.with_super());
    assert!(heard(&wheel(&mut core, REST, -10.0, false)).is_empty());
    assert_eq!(offset(&mut core), 20.0);
}

#[test]
fn the_names_parse_as_the_schema_spells_them() {
    assert_eq!(KeyMods::parse("ctrl super"), CTRL_OR_SUPER);
    assert_eq!(
        KeyMods::parse("shift, alt"),
        KeyMods::NONE.with_shift().with_alt()
    );
    assert_eq!(KeyMods::parse("control cmd"), KeyMods::NONE);
}
