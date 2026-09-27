//! Focus regions (`docs/adr/0022-focus-regions.md`): a `focus_region`
//! subtree is a Tab ring of its own that the main ring never enters and
//! never leaves; it is entered by `focus_region`, by a press, or by an
//! explicit focus, and the region in effect follows focus. A modal wins,
//! keys bubble through the boundary, a region that goes away hands focus
//! back — and, from the same round, a root sink hears what nothing claims
//! when nothing is focused.

use kui_core::testing::{click_at, key_down, tab, tags};
use kui_core::{
    Core, EditKey, InputEvent, Key, KeyCode, KeyMods, KeyPress, Mods, NodeSpec, Size, Sizing, Value,
};

const H: f32 = 20.0;

/// The frame's keys: three app buttons, then a dock region holding two
/// buttons (and, when asked, a third that is the region's `initial_focus`).
struct Keys {
    a: Key,
    b: Key,
    c: Key,
    dock: Key,
    d1: Key,
    d2: Key,
}

/// What the frame draws besides the three app buttons.
#[derive(Clone, Copy, Default)]
struct Shape {
    /// Whether the dock is declared at all.
    dock: bool,
    /// `initial_focus` on the dock's second button.
    entry: bool,
    /// A key sink on the root.
    root_sink: bool,
    /// A modal dialog floated over everything, holding one button.
    modal: bool,
}

/// Rows of height `H` in a 200x300 column: the buttons `a`, `b`, `c` at
/// rows 0..3 and, below them, the dock's box holding `d1` and `d2` at rows
/// 3..5 with dead space under them (the box is 100px tall).
fn frame(core: &mut Core, shape: Shape) -> Keys {
    let mut ui = core.frame(Size::new(200.0, 300.0), 1.0);
    let mut root = NodeSpec::column().fill();
    if shape.root_sink {
        root = root.on_key(Value::str("root"));
    }
    ui.configure_root(root);
    let row = |label: &str| {
        NodeSpec::row()
            .width(Sizing::Fixed(100.0))
            .height(Sizing::Fixed(H))
            .on_click(Value::str(label))
            .label(label)
    };
    let a = ui.leaf_keyed("a", row("a"));
    let b = ui.leaf_keyed("b", row("b"));
    let c = ui.leaf_keyed("c", row("c"));
    let (mut dock, mut d1, mut d2) = (Key::ROOT, Key::ROOT, Key::ROOT);
    if shape.dock {
        dock = ui.with_keyed(
            "dock",
            NodeSpec::column()
                .width(Sizing::Fixed(200.0))
                .height(Sizing::Fixed(100.0))
                .focus_region(),
            |ui| {
                d1 = ui.leaf_keyed("d1", row("d1"));
                let mut second = row("d2");
                if shape.entry {
                    second = second.initial_focus();
                }
                d2 = ui.leaf_keyed("d2", second);
            },
        );
    }
    if shape.modal {
        ui.with_keyed(
            "dlg",
            NodeSpec::column()
                .float(kui_core::FloatConfig::default())
                .width(Sizing::Fixed(100.0))
                .height(Sizing::Fixed(H))
                .modal(Value::str("dlg")),
            |ui| {
                ui.leaf_keyed("ok", row("ok"));
            },
        );
    }
    ui.finish();
    Keys {
        a,
        b,
        c,
        dock,
        d1,
        d2,
    }
}

const WITH_DOCK: Shape = Shape {
    dock: true,
    entry: false,
    root_sink: false,
    modal: false,
};

/// A raw press, the channel a key sink hears.
#[test]
fn the_main_ring_skips_a_region_and_the_region_keeps_its_own() {
    let mut core = Core::new();
    let k = frame(&mut core, WITH_DOCK);
    assert_eq!(core.region(), None);
    let mut seen = Vec::new();
    for _ in 0..4 {
        tab(&mut core, false);
        seen.push(core.focus().unwrap());
    }
    assert_eq!(
        seen,
        [k.a, k.b, k.c, k.a],
        "Tab wraps over the app and never enters the dock"
    );
    tab(&mut core, true);
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(k.b), "Shift-Tab wraps the same way");

    // Entered by a click on one of its buttons: the region follows focus.
    click_at(&mut core, 50.0, 4.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.d2));
    assert_eq!(core.region(), Some(k.dock));
    let mut seen = Vec::new();
    for _ in 0..3 {
        tab(&mut core, false);
        seen.push(core.focus().unwrap());
    }
    assert_eq!(
        seen,
        [k.d1, k.d2, k.d1],
        "inside, Tab wraps over the dock's two stops"
    );
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(k.d2));

    // Back out by a click on an app button.
    click_at(&mut core, 50.0, 10.0);
    assert_eq!(core.focus(), Some(k.a));
    assert_eq!(core.region(), None);
}

#[test]
fn a_press_on_dead_space_settles_the_region_under_the_pointer() {
    let mut core = Core::new();
    let k = frame(&mut core, WITH_DOCK);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.a));
    // Dead space inside the dock: nothing focuses, but Tab is now the
    // dock's.
    click_at(&mut core, 150.0, 5.0 * H + 10.0);
    assert_eq!(core.focus(), None);
    assert_eq!(core.region(), Some(k.dock));
    tab(&mut core, false);
    assert_eq!(
        core.focus(),
        Some(k.d1),
        "Tab with nothing focused enters the region in effect"
    );
    // Escape lets go; the region stands, so Tab re-enters the dock.
    core.handle_input(InputEvent::Key(EditKey::Escape, Mods::default()));
    assert_eq!(core.focus(), None);
    tab(&mut core, true);
    assert_eq!(core.focus(), Some(k.d2));
    // Dead space outside every region is the main ring.
    click_at(&mut core, 150.0, 10.0);
    assert_eq!(core.region(), None);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.a));
}

#[test]
fn a_press_on_dead_space_under_a_root_sink_still_settles_the_region() {
    // ADR 0011 gives a press on dead space to the enclosing sink — here the
    // root, which is outside every region — so focus lands in main while
    // the pointer said "the dock". The settle must hold across the frames
    // the sink keeps focus for, not be undone by the region following a
    // focus that did not move (decision 3).
    let mut core = Core::new();
    let shape = Shape {
        root_sink: true,
        ..WITH_DOCK
    };
    let k = frame(&mut core, shape);
    click_at(&mut core, 150.0, 5.0 * H + 10.0);
    assert_eq!(
        core.focus(),
        Some(Key::ROOT),
        "the press focused the root sink"
    );
    assert_eq!(core.region(), Some(k.dock));
    frame(&mut core, shape);
    frame(&mut core, shape);
    assert_eq!(
        core.region(),
        Some(k.dock),
        "the settle stands while focus sits still"
    );
    // A sink holding focus hands Tab on itself (ADR 0002): the step it asks
    // for walks the ring under the pointer.
    core.focus_next(true);
    assert_eq!(core.focus(), Some(k.d1));
    // And a focus that does move takes the region with it again.
    core.set_focus(Some(k.a));
    frame(&mut core, shape);
    assert_eq!(core.region(), None);
    // The other way round: a press on the example's dead space from inside
    // the dock is main's, even though focus goes to the same root sink.
    core.set_focus(Some(k.d2));
    assert_eq!(core.region(), Some(k.dock));
    click_at(&mut core, 150.0, 10.0);
    assert_eq!(core.focus(), Some(Key::ROOT));
    frame(&mut core, shape);
    assert_eq!(core.region(), None);
    core.focus_next(true);
    assert_eq!(core.focus(), Some(k.a));
}

#[test]
fn focus_region_enters_on_purpose_and_lands_where_it_should() {
    let mut core = Core::new();
    let k = frame(&mut core, WITH_DOCK);
    tab(&mut core, false);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.b));
    assert!(core.focus_visible());

    // Nothing remembered, no initial_focus: the first stop. Deferred to
    // the frame's end, like a Tab step a view asks for.
    core.focus_region(Some(k.dock));
    assert_eq!(core.focus(), Some(k.b), "not yet: the frame resolves it");
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.d1));
    assert_eq!(core.region(), Some(k.dock));
    assert!(core.focus_visible(), "entering shows, like a Tab step");

    // Move inside, then back to main: main remembered `b`.
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.d2));
    core.focus_region(None);
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.b), "main lands on what it last held");
    assert_eq!(core.region(), None);

    // And the dock remembered `d2`.
    core.focus_region(Some(k.dock));
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.d2));

    // By label, from a view that has the name and not the key.
    core.focus_region(None);
    frame(&mut core, WITH_DOCK);
    core.focus_region_by_label("dock");
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.d2));
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_region_toggled_on_and_entered_in_one_frame_is_entered() {
    let mut core = Core::new();
    let k = frame(&mut core, Shape::default());
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.a));
    // The dock does not exist yet; the label is all the caller has.
    core.focus_region_by_label("dock");
    let k = frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.d1));
    assert_eq!(core.region(), Some(k.dock));
}

#[test]
fn initial_focus_names_the_entry_and_a_remembered_focus_beats_it() {
    let mut core = Core::new();
    let shape = Shape {
        entry: true,
        ..WITH_DOCK
    };
    let k = frame(&mut core, shape);
    core.focus_region(Some(k.dock));
    frame(&mut core, shape);
    assert_eq!(core.focus(), Some(k.d2), "initial_focus is the entry");
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.d1));
    core.focus_region(None);
    frame(&mut core, shape);
    core.focus_region(Some(k.dock));
    frame(&mut core, shape);
    assert_eq!(
        core.focus(),
        Some(k.d1),
        "what it last held wins over initial_focus"
    );
}

#[test]
fn a_region_that_goes_away_hands_focus_back() {
    let mut core = Core::new();
    let k = frame(&mut core, WITH_DOCK);
    click_at(&mut core, 50.0, 2.0 * H + 10.0);
    assert_eq!(core.focus(), Some(k.c));
    core.focus_region(Some(k.dock));
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.d1));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.d2));
    // The dock is toggled off with focus inside it.
    frame(&mut core, Shape::default());
    assert_eq!(core.region(), None);
    assert_eq!(core.focus(), Some(k.c), "main gets back what it held");
    // Toggled on again and entered: the dock remembers where the user was.
    core.focus_region(Some(k.dock));
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.d2));
    // With nothing to hand back, nothing is focused.
    let mut fresh = Core::new();
    let k2 = frame(&mut fresh, WITH_DOCK);
    fresh.focus_region(Some(k2.dock));
    frame(&mut fresh, WITH_DOCK);
    assert_eq!(fresh.focus(), Some(k2.d1));
    frame(&mut fresh, Shape::default());
    assert_eq!(fresh.focus(), None, "main never held anything");
    assert_eq!(fresh.region(), None);
}

#[test]
fn an_explicit_focus_into_a_region_moves_the_ring_there() {
    let mut core = Core::new();
    let k = frame(&mut core, WITH_DOCK);
    core.set_focus(Some(k.d2));
    assert_eq!(core.region(), Some(k.dock));
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.d1), "Tab walks the dock, not the app");
    core.set_focus(Some(k.a));
    assert_eq!(core.region(), None);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.b));
}

#[test]
fn a_modal_wins_over_the_region_and_hands_focus_back_into_it() {
    let mut core = Core::new();
    let k = frame(&mut core, WITH_DOCK);
    core.focus_region(Some(k.dock));
    frame(&mut core, WITH_DOCK);
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.d2));
    let with_modal = Shape {
        modal: true,
        ..WITH_DOCK
    };
    frame(&mut core, with_modal);
    let ok = core.key_of("ok").unwrap();
    assert_eq!(core.focus(), Some(ok), "the modal takes focus into itself");
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(ok), "and the ring is the modal's alone");
    frame(&mut core, WITH_DOCK);
    assert_eq!(
        core.focus(),
        Some(k.d2),
        "closed: the displaced focus comes back"
    );
    assert_eq!(core.region(), Some(k.dock), "and the region follows it");
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(k.d1));
}

#[test]
fn keys_bubble_through_the_region_boundary_to_the_root_sink() {
    let mut core = Core::new();
    let shape = Shape {
        root_sink: true,
        ..WITH_DOCK
    };
    let k = frame(&mut core, shape);
    core.focus_region(Some(k.dock));
    frame(&mut core, shape);
    assert_eq!(core.focus(), Some(k.d1));
    let chord = KeyMods {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    let evs = key_down(&mut core, KeyCode::Char('i'), chord);
    assert_eq!(
        tags(&evs),
        ["root"],
        "a chord from inside the dock reaches the app's root sink"
    );
    let evs = key_down(&mut core, KeyCode::Char('x'), KeyMods::default());
    assert_eq!(
        tags(&evs),
        ["root"],
        "so does a plain key the button does not claim"
    );
}

#[test]
fn a_root_sink_hears_what_nothing_claims_when_nothing_is_focused() {
    let mut core = Core::new();
    let shape = Shape {
        root_sink: true,
        ..Shape::default()
    };
    frame(&mut core, shape);
    assert_eq!(core.focus(), None);
    let chord = KeyMods {
        ctrl: true,
        shift: true,
        ..Default::default()
    };
    let evs = key_down(&mut core, KeyCode::Char('i'), chord);
    assert_eq!(
        tags(&evs),
        ["root"],
        "no focus: the root sink is the target"
    );
    // Tab is still the ring's: it enters, and the sink does not hear it
    // on either channel.
    let raw = core.handle_input(InputEvent::KeyDown(KeyPress::new(
        KeyCode::Tab,
        KeyMods::default(),
    )));
    assert!(raw.is_empty(), "a bare Tab with no focus is not the sink's");
    tab(&mut core, false);
    assert_eq!(core.focus(), Some(core.key_of("a").unwrap()));
    // Without a root sink the key goes nowhere, as before.
    let mut plain = Core::new();
    frame(&mut plain, Shape::default());
    assert!(key_down(&mut plain, KeyCode::Char('i'), chord).is_empty());
    // And not under a modal: the root is inert like the rest of the app.
    let mut under = Core::new();
    frame(
        &mut under,
        Shape {
            root_sink: true,
            modal: true,
            ..Shape::default()
        },
    );
    under.set_focus(None);
    assert!(key_down(&mut under, KeyCode::Char('i'), chord).is_empty());
}

#[test]
fn a_region_the_frame_does_not_declare_is_a_warning_and_moves_nothing() {
    let mut core = Core::new();
    let k = frame(&mut core, WITH_DOCK);
    core.set_focus(Some(k.b));
    core.focus_region_by_label("inspector");
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.b));
    assert_eq!(core.region(), None);
    let w = core.take_warnings();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].code, kui_core::diag::FOCUS_REGION_WITHOUT_NODE);
    assert!(w[0].message.contains("\"inspector\""));
    // A node that exists but is not a region is the same warning.
    core.focus_region(Some(k.a));
    frame(&mut core, WITH_DOCK);
    assert_eq!(core.focus(), Some(k.b));
    let w = core.take_warnings();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].key, k.a);
}
