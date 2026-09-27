//! Focus as events (backlog DX18): `onFocus` hears the keyboard entering
//! and leaving a subtree, with what moved it, and the window says when it
//! gains and loses the keyboard. kawoosh diffed `key_focus` every frame to
//! learn that a press had moved it into a pane, and `env.focused` in three
//! places to learn the window had come back.

use kui_core::testing::{click, tab};
use kui_core::{Core, NodeSpec, Size, UiEvent, Vec2};

const VIEW: Size = Size { w: 300.0, h: 100.0 };

/// A pane (`onFocus`) holding a sink and a button (`onFocus` too), and
/// beside the pane a second sink outside it.
fn frame(core: &mut Core, program: bool) {
    let mut ui = core.frame(VIEW, 1.0);
    ui.with(NodeSpec::row(), |ui| {
        ui.with_keyed("pane", NodeSpec::row().on_focus("pane"), |ui| {
            ui.leaf_keyed("sink", NodeSpec::row().size(100.0, 100.0).on_key("keys"));
            ui.leaf_keyed(
                "button",
                NodeSpec::row()
                    .size(50.0, 100.0)
                    .on_click("go")
                    .on_focus("button"),
            );
        });
        let other = ui.open_keyed("other", NodeSpec::row().size(100.0, 100.0).on_key("other"));
        if program {
            ui.take_key_focus(other);
        }
        ui.close();
    });
    ui.finish();
}

/// `phase:tag:by` of every focus event, in order.
fn focus(evs: &[UiEvent]) -> Vec<String> {
    evs.iter()
        .filter(|e| e.kind() == Some("focus"))
        .map(|e| {
            let p = &e.payload;
            let tag = e.tag().and_then(|t| t.as_str()).unwrap_or("");
            format!(
                "{}:{tag}:{}",
                p.get_str("phase").unwrap(),
                p.get_str("by").unwrap()
            )
        })
        .collect()
}

#[test]
fn a_subtree_hears_the_keyboard_come_and_go_with_what_moved_it() {
    let mut core = Core::new();
    frame(&mut core, false);

    // A press in the sink: into the pane, by the pointer.
    let evs = click(&mut core, Vec2::new(50.0, 50.0));
    assert_eq!(focus(&evs), ["in:pane:pointer"]);

    // A press on the button: still inside the pane, so only the button
    // hears it.
    let evs = click(&mut core, Vec2::new(125.0, 50.0));
    assert_eq!(focus(&evs), ["in:button:pointer"]);

    // Tab from the button to the sink outside: innermost out first.
    let evs = tab(&mut core, false);
    assert_eq!(focus(&evs), ["out:button:keyboard", "out:pane:keyboard"]);

    // The app moves it back into the pane, and the view out again: each
    // reported at the end of the frame that settles it.
    let sink = core.key_of("sink");
    core.set_key_focus(sink);
    frame(&mut core, false);
    assert_eq!(focus(&core.take_pending_events()), ["in:pane:program"]);
    frame(&mut core, true);
    assert_eq!(focus(&core.take_pending_events()), ["out:pane:program"]);
}

#[test]
fn the_window_says_when_it_gains_and_loses_the_keyboard() {
    let mut core = Core::new();
    frame(&mut core, false);
    core.take_pending_events();
    core.set_focused(false);
    core.set_focused(false);
    core.set_focused(true);
    let phases: Vec<_> = core
        .take_pending_events()
        .iter()
        .filter(|e| e.kind() == Some("window"))
        .map(|e| e.payload.get_str("phase").unwrap().to_string())
        .collect();
    assert_eq!(phases, ["blurred", "focused"], "once per change");
}
