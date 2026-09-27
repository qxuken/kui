//! `keepFocus` (backlog DX10): a press that acts and leaves keyboard focus
//! with the sink that had it. kawoosh took focus back by hand after ten
//! kinds of click, because a press on any `onClick` node focuses it.

use kui_core::testing::{click, tab};
use kui_core::{Core, NodeSpec, Size, Vec2};

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
