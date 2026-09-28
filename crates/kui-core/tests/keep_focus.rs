//! `keepFocus` (backlog DX10): a press that acts and leaves keyboard focus
//! with the sink that had it. kawoosh took focus back by hand after ten
//! kinds of click, because a press on any `onClick` node focuses it.

use kui_core::testing::{click, hover, press, release, tab};
use kui_core::{Core, NodeSpec, Size, TextStyle, Vec2};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

/// A key sink and, beside it, a button — `keep_focus` on the button, on
/// the row around it, or on nothing.
fn frame(core: &mut Core, on_button: bool, on_row: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(NodeSpec::row(), |ui| {
        ui.leaf_keyed("sink", NodeSpec::row().size(100.0, 50.0).on_key("keys"));
        let row = NodeSpec::row();
        ui.with_keyed("bar", if on_row { row.keep_focus() } else { row }, |ui| {
            let b = NodeSpec::row().size(100.0, 50.0).on_click("go");
            ui.leaf_keyed("button", if on_button { b.keep_focus() } else { b });
        });
    });
    ui.finish();
}

fn pressed(on_button: bool, on_row: bool) -> (Core, Vec<kui_core::UiEvent>) {
    let mut core = Core::new();
    frame(&mut core, on_button, on_row);
    let sink = core.key_of("sink").unwrap();
    core.set_key_focus(Some(sink));
    frame(&mut core, on_button, on_row);
    let evs = click(&mut core, Vec2::new(150.0, 25.0));
    (core, evs)
}

#[test]
fn a_press_on_a_keep_focus_node_acts_and_leaves_the_sink_focused() {
    for (on_button, on_row) in [(true, false), (false, true)] {
        let (mut core, evs) = pressed(on_button, on_row);
        let sink = core.key_of("sink");
        assert!(
            evs.iter().any(|e| e.payload.as_str() == Some("go")),
            "the click still lands"
        );
        assert_eq!(core.key_focus(), sink, "button {on_button}, row {on_row}");
    }
}

#[test]
fn without_it_a_press_focuses_the_button_and_tab_still_reaches_it() {
    let (mut core, _) = pressed(false, false);
    assert_eq!(core.key_focus(), core.key_of("button"), "as before");

    // With it, the button is still a Tab stop — the ring's last, which
    // Shift-Tab from nothing lands on (the sink before it keeps a Tab
    // pressed in it, as a key sink does).
    let mut core = Core::new();
    frame(&mut core, true, false);
    tab(&mut core, true);
    assert_eq!(core.key_focus(), core.key_of("button"));
}

/// A selectable card of text and, below it, a Copy button that keeps
/// focus — or does not.
fn card(core: &mut Core, keep: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(NodeSpec::column(), |ui| {
        ui.text_in_keyed(
            "card",
            NodeSpec::row().size(300.0, 40.0).selectable(),
            "hello brave world",
            TextStyle::new(14.0),
        );
        let copy = NodeSpec::row().size(100.0, 40.0).on_click("copy");
        ui.leaf_keyed("copy", if keep { copy.keep_focus() } else { copy });
    });
    ui.finish();
}

#[test]
fn a_press_on_a_keep_focus_node_leaves_the_selection_it_acts_on() {
    for keep in [true, false] {
        let mut core = Core::new();
        card(&mut core, keep);
        press(&mut core, Vec2::new(2.0, 8.0));
        hover(&mut core, Vec2::new(200.0, 8.0));
        release(&mut core);
        card(&mut core, keep);
        let selected = core.selection();
        assert!(selected.is_some(), "the drag selected");
        let evs = click(&mut core, Vec2::new(50.0, 60.0));
        assert!(
            evs.iter().any(|e| e.payload.as_str() == Some("copy")),
            "the click lands"
        );
        if keep {
            // The toolbar button is for acting on the selection: a press
            // that cleared it would be a Copy that never copies — the
            // core's own menu and menu bar are spared for the same reason
            // (ADR 0017).
            assert_eq!(core.selection(), selected);
        } else {
            assert_eq!(core.selection(), None, "any other press ends it");
        }
    }
}

/// A button in the main ring, a sink after it, and a toolbar region holding
/// a keep-focus button and a plain one.
fn toolbar(core: &mut Core) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(NodeSpec::column(), |ui| {
        ui.leaf_keyed("a", NodeSpec::row().size(100.0, 30.0).on_click("a"));
        ui.leaf_keyed("b", NodeSpec::row().size(100.0, 30.0).on_click("b"));
        ui.with_keyed(
            "bar",
            NodeSpec::row().size(300.0, 30.0).focus_region(),
            |ui| {
                ui.leaf_keyed(
                    "bold",
                    NodeSpec::row()
                        .size(100.0, 30.0)
                        .on_click("bold")
                        .keep_focus(),
                );
                ui.leaf_keyed("plain", NodeSpec::row().size(100.0, 30.0).on_click("plain"));
            },
        );
    });
    ui.finish();
}

#[test]
fn a_press_on_a_keep_focus_node_leaves_the_ring_where_the_keyboard_is() {
    let mut core = Core::new();
    toolbar(&mut core);
    tab(&mut core, false);
    assert_eq!(core.key_focus(), core.key_of("a"));
    let region = core.region();
    // A press on the toolbar's keep-focus button acts and leaves the
    // keyboard on "a" — and Tab goes on from there, not into the toolbar's
    // ring under the pointer.
    let evs = click(&mut core, Vec2::new(50.0, 75.0));
    assert!(evs.iter().any(|e| e.payload.as_str() == Some("bold")));
    assert_eq!(core.key_focus(), core.key_of("a"));
    assert_eq!(core.region(), region, "the ring is the keyboard's");
    tab(&mut core, false);
    assert_eq!(core.key_focus(), core.key_of("b"));
}
