//! Layout as data: an `on_layout` node reports the rect layout gave it —
//! on its first frame and whenever it changes, never on a frame that left
//! it alone — through the same pending queue a `resize` uses.

use kui_core::{Core, Easing, InputEvent, Key, NodeSpec, Size, Sizing, Transition, UiEvent, Value};

/// A row root with an optional 50px spacer before a keyed `panel` of
/// `panel_w` px; returns the panel's key.
fn frame(core: &mut Core, panel_w: f32, spacer: bool, tag: Value, slide: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    if spacer {
        ui.with_keyed(
            "spacer",
            NodeSpec::column()
                .width(Sizing::Fixed(50.0))
                .height(Sizing::Grow(1.0)),
            |_| {},
        );
    }
    let mut spec = NodeSpec::column()
        .width(Sizing::Fixed(panel_w))
        .height(Sizing::Grow(1.0))
        .on_layout(tag);
    if slide {
        spec = spec
            .transition_with(Transition::ms(100.0).easing(Easing::Linear))
            .slide();
    }
    let key = ui.with_keyed("panel", spec, |_| {});
    ui.finish();
    key
}

/// `(x, y, w, h)` of every layout event in `evs`.
fn rects(evs: &[UiEvent]) -> Vec<(f64, f64, f64, f64)> {
    evs.iter()
        .filter(|ev| ev.payload.get("kind").and_then(Value::as_str) == Some("layout"))
        .map(|ev| {
            let f = |k| ev.payload.get(k).and_then(Value::as_float).unwrap();
            (f("x"), f("y"), f("w"), f("h"))
        })
        .collect()
}

#[test]
fn first_sight_reports_the_rect_then_silence() {
    let mut core = Core::new();
    let panel = frame(&mut core, 100.0, false, Value::str("panel"), false);
    let evs = core.take_pending_events();
    assert_eq!(rects(&evs), [(0.0, 0.0, 100.0, 300.0)]);
    let ev = &evs[0];
    assert_eq!(ev.key, panel);
    assert_eq!(ev.payload.get("tag").and_then(Value::as_str), Some("panel"));
    // The parent rect rides along, as it does on drag events.
    let parent = ev.payload.get("parent").expect("parent rect");
    assert_eq!(parent.get("w").and_then(Value::as_float), Some(400.0));
    assert_eq!(parent.get("h").and_then(Value::as_float), Some(300.0));

    // Same layout again: nothing.
    frame(&mut core, 100.0, false, Value::str("panel"), false);
    assert!(rects(&core.take_pending_events()).is_empty());
}

#[test]
fn a_changed_rect_reports_again() {
    let mut core = Core::new();
    frame(&mut core, 100.0, false, Value::Null, false);
    core.take_pending_events();

    frame(&mut core, 150.0, false, Value::Null, false);
    assert_eq!(
        rects(&core.take_pending_events()),
        [(0.0, 0.0, 150.0, 300.0)]
    );

    // A sibling appearing before it moves it.
    frame(&mut core, 150.0, true, Value::Null, false);
    assert_eq!(
        rects(&core.take_pending_events()),
        [(50.0, 0.0, 150.0, 300.0)]
    );
}

#[test]
fn a_null_tag_leaves_tag_off_the_payload() {
    let mut core = Core::new();
    frame(&mut core, 100.0, false, Value::Null, false);
    let evs = core.take_pending_events();
    assert_eq!(evs.len(), 1);
    assert!(evs[0].payload.get("tag").is_none());
}

#[test]
fn a_node_that_leaves_and_returns_reports_again() {
    let mut core = Core::new();
    frame(&mut core, 100.0, false, Value::Null, false);
    core.take_pending_events();

    // A frame without the panel at all.
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.with(NodeSpec::column(), |_| {});
    ui.finish();
    assert!(rects(&core.take_pending_events()).is_empty());

    frame(&mut core, 100.0, false, Value::Null, false);
    assert_eq!(
        rects(&core.take_pending_events()),
        [(0.0, 0.0, 100.0, 300.0)]
    );
}

#[test]
fn undrained_layout_events_ride_along_with_the_next_input() {
    let mut core = Core::new();
    frame(&mut core, 100.0, false, Value::Null, false);
    let evs = core.handle_input(InputEvent::mouse_down(1));
    assert_eq!(rects(&evs).len(), 1);
    assert!(rects(&core.take_pending_events()).is_empty());
}

#[test]
fn an_easing_position_reports_every_frame_it_moves() {
    let mut core = Core::new();
    core.set_time(0.0);
    frame(&mut core, 100.0, false, Value::Null, true);
    core.take_pending_events();

    // The spacer appears: the panel's target moves to x=50 and it slides.
    core.set_time(0.0);
    frame(&mut core, 100.0, true, Value::Null, true);
    assert!(
        rects(&core.take_pending_events()).is_empty(),
        "the retarget frame itself still draws at x=0"
    );
    core.set_time(0.05);
    frame(&mut core, 100.0, true, Value::Null, true);
    assert_eq!(
        rects(&core.take_pending_events()),
        [(25.0, 0.0, 100.0, 300.0)]
    );
    core.set_time(0.2);
    frame(&mut core, 100.0, true, Value::Null, true);
    assert_eq!(
        rects(&core.take_pending_events()),
        [(50.0, 0.0, 100.0, 300.0)]
    );
    core.set_time(0.3);
    frame(&mut core, 100.0, true, Value::Null, true);
    assert!(rects(&core.take_pending_events()).is_empty(), "settled");
}

/// The inspector's reading of a frame (`Core::nodes`): off until asked,
/// then every node with its kind, label, rect and flags, in tree order.
#[test]
fn the_inspector_reads_the_frame_back_when_asked() {
    use kui_core::{NodeKind, TextStyle};
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(200.0, 100.0), 1.0);
        ui.with_keyed(
            "card",
            NodeSpec::column().pad(10.0).on_click(Value::str("go")),
            |ui| {
                ui.text("hello world", TextStyle::new(12.0));
            },
        );
        ui.finish();
    };
    build(&mut core);
    assert!(
        core.nodes().is_empty(),
        "nothing is copied until a tool asks"
    );
    core.set_inspect(true);
    build(&mut core);
    let nodes = core.nodes();
    assert_eq!(nodes.len(), 3, "root, card, text");
    assert_eq!(nodes[0].depth, 0);
    assert_eq!(nodes[1].label.as_deref(), Some("card"));
    assert_eq!(nodes[1].depth, 1);
    assert!(nodes[1].flags.contains(&"click"));
    assert_eq!(
        nodes[1].role,
        Some(kui_core::Role::Button),
        "the role is the access tree's reading, derived from the click"
    );
    assert_eq!(nodes[0].role, Some(kui_core::Role::Window));
    assert_eq!(nodes[1].parent, Some(nodes[0].key));
    assert_eq!(nodes[2].kind, NodeKind::Text);
    assert_eq!(nodes[2].text.as_deref(), Some("hello world"));
    assert!(
        nodes[2].rect.x >= 10.0 && nodes[2].rect.w > 0.0,
        "laid out: {:?}",
        nodes[2].rect
    );
    core.set_inspect(false);
    assert!(core.nodes().is_empty());
}
