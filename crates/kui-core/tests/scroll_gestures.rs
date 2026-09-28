//! A scroll gesture picks its target when it starts and keeps it (backlog
//! F107, ADR 0038): latching — a swipe that moved the strip goes on
//! moving it when a terminal comes under the pointer — and chaining — a
//! gesture starting over a scroller already at its limit goes to the one
//! around it, unless that one says `overscroll: contain`. Plus the axes
//! an `on_scroll` node takes (`scroll_axes`), so a sideways swipe passes a
//! terminal that scrolls only its history.

use kui_core::{Core, InputEvent, Key, NodeSpec, Overscroll, ScrollAxes, Size, UiEvent, Vec2};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn gesture(core: &mut Core, dx: f32, dy: f32, begins: bool) -> Vec<UiEvent> {
    core.handle_input(InputEvent::ScrollGesture {
        delta: Vec2::new(dx, dy),
        begins,
    })
}

fn scrolls(out: &[UiEvent], key: Key) -> Vec<(f64, f64)> {
    out.iter()
        .filter(|e| e.key == key && e.kind() == Some("scroll"))
        .map(|e| {
            (
                e.payload.get_float("dx").unwrap(),
                e.payload.get_float("dy").unwrap(),
            )
        })
        .collect()
}

/// kawoosh's shape: a strip scrolling x, and in it a list scrolling y
/// (0..150) and a terminal taking the wheel itself (150..300), then more
/// strip beyond.
fn strip(core: &mut Core, axes: ScrollAxes) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed(
        "strip",
        NodeSpec::row().size(300.0, 200.0).scroll_x(),
        |ui| {
            ui.with_keyed(
                "list",
                NodeSpec::column().size(150.0, 200.0).scroll_y(),
                |ui| {
                    ui.leaf(NodeSpec::column().size(150.0, 800.0));
                },
            );
            ui.with_keyed(
                "term",
                NodeSpec::column()
                    .size(150.0, 200.0)
                    .on_scroll("term")
                    .scroll_axes(axes),
                |_| {},
            );
            ui.leaf(NodeSpec::column().size(600.0, 200.0));
        },
    );
    ui.finish();
}

fn keys(core: &mut Core) -> (Key, Key, Key) {
    (
        core.key_of("strip").unwrap(),
        core.key_of("list").unwrap(),
        core.key_of("term").unwrap(),
    )
}

/// Item 3 of the report: a swipe that started on the strip goes on
/// scrolling it when the terminal, carried along, comes under the
/// pointer — even a terminal that takes both axes.
#[test]
fn a_sideways_swipe_keeps_the_strip_when_a_terminal_comes_under_the_pointer() {
    let mut core = Core::new();
    strip(&mut core, ScrollAxes::Both);
    strip(&mut core, ScrollAxes::Both);
    let (strip_k, _, term) = keys(&mut core);
    // Over the list's right edge: the list scrolls y only, so x is the
    // strip's.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(130.0, 50.0)));
    gesture(&mut core, -30.0, 0.0, true);
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(30.0, 0.0));
    // The terminal is at 120..270 now, under the still pointer. The
    // swipe goes on moving the strip, and the terminal hears nothing.
    for _ in 0..3 {
        let out = gesture(&mut core, -30.0, 0.0, false);
        assert!(
            scrolls(&out, term).is_empty(),
            "latched: not the terminal's"
        );
        strip(&mut core, ScrollAxes::Both);
    }
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(120.0, 0.0));
    // A new gesture over the terminal is the terminal's.
    let out = gesture(&mut core, -30.0, 0.0, true);
    assert_eq!(scrolls(&out, term), vec![(-30.0, 0.0)]);
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(120.0, 0.0));
}

/// Item 3's second half: a terminal that takes only `y` lets a sideways
/// gesture that starts over it pass to the strip, and keeps a vertical
/// one.
#[test]
fn a_y_only_handler_passes_a_sideways_gesture_to_the_strip() {
    let mut core = Core::new();
    strip(&mut core, ScrollAxes::Y);
    strip(&mut core, ScrollAxes::Y);
    let (strip_k, _, term) = keys(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 50.0)));
    let out = gesture(&mut core, -25.0, 0.0, true);
    assert!(scrolls(&out, term).is_empty());
    strip(&mut core, ScrollAxes::Y);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(25.0, 0.0));
    let out = gesture(&mut core, 0.0, -12.0, true);
    assert_eq!(scrolls(&out, term), vec![(0.0, -12.0)]);
    strip(&mut core, ScrollAxes::Y);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(25.0, 0.0));
    // A diagonal notch splits: y to the terminal, x to the strip.
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(-5.0, -6.0)));
    assert_eq!(scrolls(&out, term), vec![(0.0, -6.0)]);
    strip(&mut core, ScrollAxes::Y);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(30.0, 0.0));
}

/// Item 1: a gesture over a one-axis scroller locked to its axis is that
/// scroller's; locked to the other axis it goes to the ancestor that
/// scrolls that way; and an axis the gesture turns to midway picks its
/// own target then, keeping the first.
#[test]
fn a_gesture_on_the_other_axis_goes_to_the_ancestor() {
    let mut core = Core::new();
    strip(&mut core, ScrollAxes::Both);
    strip(&mut core, ScrollAxes::Both);
    let (strip_k, list, _) = keys(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 50.0)));
    gesture(&mut core, 0.0, -40.0, true);
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(list), Vec2::new(0.0, 40.0));
    assert_eq!(core.scroll_offset(strip_k), Vec2::ZERO);
    // The hand turns sideways without lifting: x is new to the gesture,
    // and the strip is the one that scrolls x here.
    gesture(&mut core, -20.0, 0.0, false);
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(20.0, 0.0));
    // And back: y is still the list's.
    gesture(&mut core, 0.0, -10.0, false);
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(list), Vec2::new(0.0, 50.0));
    // A sideways gesture of its own over the list is the strip's too.
    gesture(&mut core, -10.0, 0.0, true);
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(30.0, 0.0));
    assert_eq!(core.scroll_offset(list), Vec2::new(0.0, 50.0));
}

/// A page scrolling y with a list inside it scrolling y (40..140 of the
/// page), which can say `overscroll: contain`.
fn nested(core: &mut Core, contain: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed(
        "page",
        NodeSpec::column().size(300.0, 200.0).scroll_y(),
        |ui| {
            ui.leaf(NodeSpec::column().size(300.0, 40.0));
            let mut inner = NodeSpec::column().size(300.0, 100.0).scroll_y();
            if contain {
                inner = inner.overscroll(Overscroll::Contain);
            }
            ui.with_keyed("inner", inner, |ui| {
                ui.leaf(NodeSpec::column().size(300.0, 300.0));
            });
            ui.leaf(NodeSpec::column().size(300.0, 600.0));
        },
    );
    ui.finish();
}

fn nested_keys(core: &mut Core) -> (Key, Key) {
    (core.key_of("page").unwrap(), core.key_of("inner").unwrap())
}

/// Item 2: a gesture that starts over a scroller already at its limit
/// the way it goes is the scroller around it's; the other way it is
/// still the inner one's.
#[test]
fn a_gesture_starting_at_a_limit_chains_to_the_scroller_around() {
    let mut core = Core::new();
    nested(&mut core, false);
    nested(&mut core, false);
    let (page, inner) = nested_keys(&mut core);
    core.set_scroll(inner, Vec2::new(0.0, 1e9));
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 200.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 80.0)));
    // Down, past the inner list's end: the page's.
    gesture(&mut core, 0.0, -30.0, true);
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 30.0));
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 200.0));
    // The inner list is at 10..110 now, still under the pointer. Up:
    // the inner list can move that way, so it is its.
    gesture(&mut core, 0.0, 50.0, true);
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 150.0));
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 30.0));
    // At the top, a gesture up goes by it too — and the page, with room
    // up, takes it.
    core.set_scroll(inner, Vec2::ZERO);
    nested(&mut core, false);
    gesture(&mut core, 0.0, 10.0, true);
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 20.0));
    // A wheel notch is a gesture of its own and chains the same way.
    core.set_scroll(inner, Vec2::new(0.0, 1e9));
    nested(&mut core, false);
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -5.0)));
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 25.0));
}

/// A gesture that reaches a limit midway stops there: it does not chain
/// to the scroller around, as a browser's does not.
#[test]
fn a_gesture_reaching_a_limit_midway_stops_there() {
    let mut core = Core::new();
    nested(&mut core, false);
    nested(&mut core, false);
    let (page, inner) = nested_keys(&mut core);
    core.set_scroll(inner, Vec2::new(0.0, 180.0));
    nested(&mut core, false);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 80.0)));
    gesture(&mut core, 0.0, -15.0, true);
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 195.0));
    for _ in 0..4 {
        gesture(&mut core, 0.0, -15.0, false);
        nested(&mut core, false);
    }
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 200.0));
    assert_eq!(core.scroll_offset(page), Vec2::ZERO, "no chaining midway");
    // Turning back within the gesture moves the same list.
    gesture(&mut core, 0.0, 20.0, false);
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 180.0));
    // A new gesture down from the limit starts on the page.
    core.set_scroll(inner, Vec2::new(0.0, 1e9));
    nested(&mut core, false);
    gesture(&mut core, 0.0, -10.0, true);
    nested(&mut core, false);
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 10.0));
}

/// `overscroll: contain`: a gesture that starts over the scroller at its
/// limit stays with it and moves nothing — and turning back moves it.
#[test]
fn contain_keeps_a_gesture_at_the_limit() {
    let mut core = Core::new();
    nested(&mut core, true);
    nested(&mut core, true);
    let (page, inner) = nested_keys(&mut core);
    core.set_scroll(inner, Vec2::new(0.0, 1e9));
    nested(&mut core, true);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 80.0)));
    gesture(&mut core, 0.0, -30.0, true);
    nested(&mut core, true);
    assert_eq!(core.scroll_offset(page), Vec2::ZERO, "contained");
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 200.0));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -30.0)));
    nested(&mut core, true);
    assert_eq!(core.scroll_offset(page), Vec2::ZERO, "a notch too");
    gesture(&mut core, 0.0, 30.0, false);
    nested(&mut core, true);
    assert_eq!(core.scroll_offset(inner), Vec2::new(0.0, 170.0));
    // Over the page outside the list, the page is the page's.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 180.0)));
    gesture(&mut core, 0.0, -30.0, true);
    nested(&mut core, true);
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 30.0));
}

/// A contained list inside the strip still hands a sideways swipe to the
/// strip: `contain` holds only on the axes a scroller scrolls.
#[test]
fn contain_holds_only_on_the_axes_a_scroller_scrolls() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            "strip",
            NodeSpec::row().size(300.0, 200.0).scroll_x(),
            |ui| {
                ui.with_keyed(
                    "list",
                    NodeSpec::column()
                        .size(150.0, 200.0)
                        .scroll_y()
                        .overscroll(Overscroll::Contain),
                    |ui| {
                        ui.leaf(NodeSpec::column().size(150.0, 800.0));
                    },
                );
                ui.leaf(NodeSpec::column().size(600.0, 200.0));
            },
        );
        ui.finish();
    };
    build(&mut core);
    build(&mut core);
    let strip_k = core.key_of("strip").unwrap();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 50.0)));
    gesture(&mut core, -30.0, 0.0, true);
    build(&mut core);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(30.0, 0.0));
}

/// A latched target whose node goes is dropped, and the gesture's next
/// event picks again under the pointer.
#[test]
fn a_latched_target_that_goes_is_picked_again() {
    let mut core = Core::new();
    let build = |core: &mut Core, name: &str| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            name,
            NodeSpec::column().size(300.0, 200.0).scroll_y(),
            |ui| {
                ui.leaf(NodeSpec::column().size(300.0, 800.0));
            },
        );
        ui.finish();
    };
    build(&mut core, "a");
    build(&mut core, "a");
    let a = core.key_of("a").unwrap();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 50.0)));
    gesture(&mut core, 0.0, -10.0, true);
    build(&mut core, "a");
    assert_eq!(core.scroll_offset(a), Vec2::new(0.0, 10.0));
    // `a` goes; `b` takes its place under the pointer.
    build(&mut core, "b");
    build(&mut core, "b");
    let b = core.key_of("b").unwrap();
    gesture(&mut core, 0.0, -10.0, false);
    build(&mut core, "b");
    assert_eq!(core.scroll_offset(b), Vec2::new(0.0, 10.0));
}

/// Bare `Scroll` events — a wheel's notches from a driver that says
/// nothing of gestures — are each a gesture of their own: a notch after
/// the strip carried the terminal under the pointer is the terminal's.
#[test]
fn a_bare_notch_is_a_gesture_of_its_own() {
    let mut core = Core::new();
    strip(&mut core, ScrollAxes::Both);
    strip(&mut core, ScrollAxes::Both);
    let (strip_k, _, term) = keys(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(130.0, 50.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(-30.0, 0.0)));
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(30.0, 0.0));
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(-30.0, 0.0)));
    assert_eq!(scrolls(&out, term), vec![(-30.0, 0.0)]);
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(strip_k), Vec2::new(30.0, 0.0));
}

/// A latched gesture keeps going while the pointer is elsewhere: the
/// target is found by key, not under the pointer.
#[test]
fn a_latched_gesture_follows_its_target_not_the_pointer() {
    let mut core = Core::new();
    strip(&mut core, ScrollAxes::Both);
    strip(&mut core, ScrollAxes::Both);
    let (_, list, term) = keys(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 50.0)));
    gesture(&mut core, 0.0, -10.0, true);
    // The momentum glides on while the pointer moves over the terminal.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 50.0)));
    let out = gesture(&mut core, 0.0, -10.0, false);
    assert!(scrolls(&out, term).is_empty());
    strip(&mut core, ScrollAxes::Both);
    assert_eq!(core.scroll_offset(list), Vec2::new(0.0, 20.0));
}

/// A page scrolling y, and floated over it — declared beside it, not in
/// it — a 100 px list scrolling y, modal or not.
fn page_and_popover(core: &mut Core, modal: bool) {
    use kui_core::{Align, FloatConfig};
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed(
        "page",
        NodeSpec::column().size(300.0, 200.0).scroll_y(),
        |ui| {
            ui.leaf(NodeSpec::column().size(300.0, 800.0));
        },
    );
    let pop = NodeSpec::column()
        .float(
            FloatConfig::viewport()
                .inside(Align::Start, Align::Start)
                .offset(50.0, 50.0),
        )
        .size(100.0, 100.0)
        .scroll_y();
    ui.with_keyed("pop", if modal { pop.modal("m") } else { pop }, |ui| {
        ui.leaf(NodeSpec::column().size(100.0, 400.0));
    });
    ui.finish();
}

/// Chaining goes to the scroller *around* the one at its limit, by the
/// tree: a list at its end in a popover floated over a page it was not
/// declared in leaves that page alone, where the walk by paint order
/// scrolled it (the alpha.22 regression pass).
#[test]
fn a_popover_list_at_its_end_does_not_chain_to_the_page_it_floats_over() {
    let mut core = Core::new();
    page_and_popover(&mut core, false);
    page_and_popover(&mut core, false);
    let page = core.key_of("page").unwrap();
    let pop = core.key_of("pop").unwrap();
    core.set_scroll(pop, Vec2::new(0.0, 1e9));
    page_and_popover(&mut core, false);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(100.0, 100.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -30.0)));
    page_and_popover(&mut core, false);
    assert_eq!(core.scroll_offset(page), Vec2::ZERO);
    // Over the page itself it is the page's.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(250.0, 180.0)));
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -30.0)));
    page_and_popover(&mut core, false);
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 30.0));
}

/// A wheel event between `begin_frame` and `finish` — a C host feeding
/// input mid-build — routes by what the last finished frame drew; it read
/// the half-built tree by index and panicked (the alpha.22 regression
/// pass).
#[test]
fn a_wheel_during_a_build_routes_by_the_last_frame() {
    let mut core = Core::new();
    page_and_popover(&mut core, false);
    page_and_popover(&mut core, false);
    let page = core.key_of("page").unwrap();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(250.0, 180.0)));
    core.begin_frame(VIEW, 1.0);
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -10.0)));
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 10.0));
}

/// A glide latched on the page goes to the sheet a modal opened under
/// the pointer mid-gesture: the latched target is outside the modal's
/// scope, so the gesture picks again (ADR 0038, decision 3).
#[test]
fn a_latched_target_behind_a_new_modal_is_picked_again() {
    use kui_core::{Align, FloatConfig};
    let frame = |core: &mut Core, modal: bool| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with_keyed(
            "page",
            NodeSpec::column().size(300.0, 200.0).scroll_y(),
            |ui| {
                ui.leaf(NodeSpec::column().size(300.0, 800.0));
            },
        );
        if modal {
            ui.with_keyed(
                "sheet",
                NodeSpec::column()
                    .float(FloatConfig::viewport().inside(Align::Start, Align::Start))
                    .size(300.0, 200.0)
                    .scroll_y()
                    .modal("m"),
                |ui| {
                    ui.leaf(NodeSpec::column().size(300.0, 800.0));
                },
            );
        }
        ui.finish();
    };
    let mut core = Core::new();
    frame(&mut core, false);
    frame(&mut core, false);
    let page = core.key_of("page").unwrap();
    core.handle_input(InputEvent::CursorMoved(Vec2::new(100.0, 100.0)));
    gesture(&mut core, 0.0, -10.0, true);
    frame(&mut core, true);
    frame(&mut core, true);
    let sheet = core.key_of("sheet").unwrap();
    gesture(&mut core, 0.0, -10.0, false);
    frame(&mut core, true);
    assert_eq!(core.scroll_offset(page), Vec2::new(0.0, 10.0));
    assert_eq!(core.scroll_offset(sheet), Vec2::new(0.0, 10.0));
}
