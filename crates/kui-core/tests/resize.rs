//! A changed viewport is data: `begin_frame` turns a new size or DPI into a
//! `resize` event on the root, drained with the frame's other pending events.

use kui_core::{Core, InputEvent, Key, NodeSpec, Size, UiEvent, Value};

fn frame(core: &mut Core, w: f32, h: f32, scale: f32) {
    let mut ui = core.frame(Size::new(w, h), scale);
    ui.with(NodeSpec::column(), |_| {});
    ui.finish();
}

/// `(width, height, scale)` of every resize event in `evs`.
fn resizes(evs: &[UiEvent]) -> Vec<(f64, f64, f64)> {
    evs.iter()
        .filter(|ev| ev.payload.get("kind").and_then(Value::as_str) == Some("resize"))
        .map(|ev| {
            let f = |k| ev.payload.get(k).and_then(Value::as_float).unwrap();
            (f("width"), f("height"), f("scale"))
        })
        .collect()
}

#[test]
fn the_first_frame_establishes_the_viewport_instead_of_resizing_it() {
    let mut core = Core::new();
    frame(&mut core, 400.0, 300.0, 1.0);
    assert!(resizes(&core.take_pending_events()).is_empty());
}

#[test]
fn a_changed_viewport_emits_one_resize_on_the_root() {
    let mut core = Core::new();
    frame(&mut core, 400.0, 300.0, 1.0);
    core.take_pending_events();

    frame(&mut core, 800.0, 600.0, 1.0);
    let pending = core.take_pending_events();
    assert_eq!(resizes(&pending), [(800.0, 600.0, 1.0)]);
    let ev = &pending[0];
    assert_eq!(ev.key, Key::ROOT);
    assert_eq!(ev.origin, kui_core::OriginId::HOST);

    // Same size again: nothing.
    frame(&mut core, 800.0, 600.0, 1.0);
    assert!(resizes(&core.take_pending_events()).is_empty());
}

#[test]
fn a_changed_scale_factor_resizes_too() {
    let mut core = Core::new();
    frame(&mut core, 400.0, 300.0, 1.0);
    core.take_pending_events();

    // Same logical size on a 2x display.
    frame(&mut core, 400.0, 300.0, 2.0);
    assert_eq!(resizes(&core.take_pending_events()), [(400.0, 300.0, 2.0)]);
}

#[test]
fn undrained_resizes_ride_along_with_the_next_input() {
    let mut core = Core::new();
    frame(&mut core, 400.0, 300.0, 1.0);
    core.take_pending_events();

    frame(&mut core, 500.0, 300.0, 1.0);
    let evs = core.handle_input(InputEvent::MouseDown(1));
    assert_eq!(resizes(&evs), [(500.0, 300.0, 1.0)]);
    // Drained by the input, not left for the next drain.
    assert!(resizes(&core.take_pending_events()).is_empty());
}

#[test]
fn a_zero_viewport_is_a_resize_like_any_other() {
    let mut core = Core::new();
    frame(&mut core, 400.0, 300.0, 1.0);
    core.take_pending_events();

    // A minimized window reports 0x0, and restoring reports the size again.
    frame(&mut core, 0.0, 0.0, 1.0);
    assert_eq!(resizes(&core.take_pending_events()), [(0.0, 0.0, 1.0)]);
    frame(&mut core, 400.0, 300.0, 1.0);
    assert_eq!(resizes(&core.take_pending_events()), [(400.0, 300.0, 1.0)]);
}

#[test]
fn the_viewport_and_scale_are_queryable() {
    let mut core = Core::new();
    assert_eq!(core.viewport(), Size::ZERO);
    frame(&mut core, 640.0, 480.0, 2.0);
    assert_eq!(core.viewport(), Size::new(640.0, 480.0));
    assert_eq!(core.scale(), 2.0);
}
