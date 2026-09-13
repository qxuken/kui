//! A custom editor's caret blinks (backlog C35): the `caret` a `line`
//! under the focused sink declares is what the driver's blink clock is
//! armed on — `has_caret` says there is one, `caret_stamp` changes when
//! it moves so the clock re-arms solid, and the phase the driver sets is
//! what `caret_visible` reads back in the view.

use kui_core::{Core, EditOptions, Key, NodeSpec, Role, Size, Sizing, TextStyle, Value};

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(20.0)
}

/// A sink drawing two lines, the caret at `caret` (line, byte) when given.
fn frame(core: &mut Core, caret: Option<(usize, u32)>, draw_caret: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let visible = ui.caret_visible();
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::column()
            .fill()
            .on_key(Value::Null)
            .role(Role::MultilineTextInput)
            .label("buf"),
        |ui| {
            for (n, text) in ["first line", "second"].iter().enumerate() {
                let mut row = NodeSpec::row().height(Sizing::Fixed(20.0)).role(Role::Line);
                if let Some((l, b)) = caret
                    && l == n
                {
                    row = row.caret(b);
                }
                ui.with(row, |ui| {
                    ui.text(text, mono());
                    // The drawn caret: on the phase, and only when the
                    // app draws one at all.
                    if draw_caret && visible && caret.is_some_and(|(l, _)| l == n) {
                        ui.with_keyed(
                            "caret",
                            NodeSpec::column()
                                .width(Sizing::Fixed(2.0))
                                .height(Sizing::Fixed(16.0)),
                            |_| {},
                        );
                    }
                });
            }
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    sink
}

#[test]
fn a_sinks_caret_line_is_a_caret_to_blink_and_moving_it_restamps() {
    let mut core = Core::new();
    // Nothing focused, nothing declared: no caret.
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.text("plain", mono());
    ui.finish();
    assert!(!core.has_caret());
    let stamp0 = core.caret_stamp();

    // A focused sink whose line declares `caret`: there is one.
    frame(&mut core, Some((0, 3)), true);
    assert!(core.has_caret(), "the caret row on a line under the sink");
    let stamp1 = core.caret_stamp();
    assert_ne!(stamp1, stamp0, "focus arriving on a caret restamps");
    // The same caret declared again: nothing moved, nothing restamped.
    frame(&mut core, Some((0, 3)), true);
    assert_eq!(core.caret_stamp(), stamp1);
    // The offset moves: restamped, so the driver re-arms solid.
    frame(&mut core, Some((0, 5)), true);
    let stamp2 = core.caret_stamp();
    assert_ne!(stamp2, stamp1);
    // To another line: restamped again.
    frame(&mut core, Some((1, 0)), true);
    assert_ne!(core.caret_stamp(), stamp2);
    // The line stops declaring one: nothing to blink.
    frame(&mut core, None, true);
    assert!(!core.has_caret());
}

#[test]
fn the_phase_the_driver_sets_is_what_the_view_reads() {
    let mut core = Core::new();
    assert!(core.caret_visible(), "solid until a driver says otherwise");
    frame(&mut core, Some((0, 3)), true);
    assert!(
        core.key_of("caret").is_some(),
        "the on phase draws the caret"
    );
    core.set_caret_visible(false);
    assert!(!core.caret_visible());
    frame(&mut core, Some((0, 3)), true);
    assert!(core.key_of("caret").is_none(), "the off phase draws none");
    // And the `caret` row stayed declared through the off phase: the
    // clock is still armed on it, so it does not flip back to solid.
    assert!(core.has_caret());
    core.set_caret_visible(true);
    frame(&mut core, Some((0, 3)), true);
    assert!(core.key_of("caret").is_some());
}

#[test]
fn a_stock_editor_and_a_sink_share_the_stamp_and_the_phase() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text_edit(
        "note",
        "hello",
        &EditOptions::default(),
        NodeSpec::column().width(Sizing::Grow(1.0)).label("note"),
    );
    ui.finish();
    let note = core.key_of("note").unwrap();
    assert!(
        !core.has_caret(),
        "an unfocused editor has no caret to blink"
    );
    core.set_focus(Some(note));
    assert!(core.has_caret());
    let stamp = core.caret_stamp();
    core.handle_input(kui_core::InputEvent::Text("!".into()));
    assert_ne!(core.caret_stamp(), stamp, "typing moves the stock caret");
    // The stock editor's blink gate is the same phase.
    core.set_caret_visible(false);
    assert!(!core.caret_visible());
    core.set_caret_visible(true);
    // Focus moving to a sink with a caret keeps `has_caret` true and
    // restamps.
    let stamp = core.caret_stamp();
    frame(&mut core, Some((0, 1)), false);
    assert!(core.has_caret());
    assert_ne!(core.caret_stamp(), stamp);
}
