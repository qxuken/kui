//! A custom editor's caret blinks (backlog C35): the `caret` a `line`
//! under the focused sink declares is what the driver's blink clock is
//! armed on — `has_caret` says there is one, `caret_stamp` changes when
//! it moves so the clock re-arms solid, and the phase the driver sets is
//! what `caret_visible` reads back in the view.

use kui_core::{Core, EditOptions, Key, NodeSpec, Role, Size, TextStyle};

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
            .key_sink()
            .role(Role::MultilineTextInput)
            .label("buf"),
        |ui| {
            for (n, text) in ["first line", "second"].iter().enumerate() {
                let mut row = NodeSpec::row().height(20.0).role(Role::Line);
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
                        ui.leaf_keyed("caret", NodeSpec::column().size(2.0, 16.0));
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
        NodeSpec::column().grow_width().label("note"),
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

/// A solid caret (backlog F68): a `caret` row declaring `caret_solid` —
/// a modal editor's block caret in normal mode — still anchors the IME
/// and is still the access tree's caret, but is no caret to blink, so a
/// driver's clock is not armed on it and an idle app draws no frame for
/// it. The phase stays true, so the view draws the block every frame;
/// the bar of insert mode blinks the moment the line stops declaring it
/// solid.
#[test]
fn a_solid_caret_anchors_and_reads_but_does_not_blink() {
    let mut core = Core::new();
    let frame = |core: &mut Core, solid: bool| -> Key {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let sink = ui.with_keyed(
            "editor",
            NodeSpec::column()
                .fill()
                .key_sink()
                .role(Role::MultilineTextInput)
                .label("buf"),
            |ui| {
                let mut row = NodeSpec::row().height(20.0).role(Role::Line).caret(3);
                if solid {
                    row = row.caret_solid();
                }
                ui.text_in(row, "first line", mono());
            },
        );
        ui.take_key_focus(sink);
        ui.finish();
        sink
    };
    // Insert mode: a bar that blinks.
    let sink = frame(&mut core, false);
    assert!(core.has_caret());
    let anchor = core.ime_rect().expect("the caret line anchors the IME");
    let stamp = core.caret_stamp();
    // Escape to normal mode: the same offset, now a solid block. Nothing
    // to blink — and nothing moved, so the stamp holds too.
    frame(&mut core, true);
    assert!(!core.has_caret(), "a solid caret is not a caret to blink");
    assert_eq!(core.caret_stamp(), stamp, "bar to block is not a move");
    assert_eq!(core.ime_rect(), Some(anchor), "the anchor held");
    assert_eq!(
        core.access_tree().get(sink).unwrap().caret,
        Some(3),
        "assistive technology still hears where the caret is"
    );
    // The driver, seeing no caret to blink, leaves the phase solid: the
    // view draws its block every frame.
    core.set_caret_visible(true);
    assert!(core.caret_visible());
    // Back to insert mode: a caret to blink again.
    frame(&mut core, false);
    assert!(core.has_caret());
    assert_eq!(core.caret_stamp(), stamp);
}

/// The caret follows the keys (backlog AR29): focus on a control inside
/// the custom editor — a pane button, an AT `Focus`, `set_focus` — still
/// routes keys and commits to the enclosing sink, and the caret is still
/// that editor's, so the blink clock stays armed and the IME keeps its
/// anchor. The candidates are the editor's lines as the access tree reads
/// them: a `role="none"` gutter's line is not one, and where two lines
/// declare a caret the last does, as `custom_editor` reads it.
#[test]
fn the_caret_is_the_enclosing_sinks_wherever_focus_sits_inside_it() {
    let mut core = Core::new();
    let frame = |core: &mut Core, carets: &[Option<u32>]| -> (Key, Key) {
        let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let mut button = Key::ROOT;
        let sink = ui.with_keyed(
            "editor",
            NodeSpec::column()
                .fill()
                .key_sink()
                .role(Role::MultilineTextInput)
                .label("buf"),
            |ui| {
                // A gutter the access tree skips, with a line of its own
                // that must not count — and a button inside the editor.
                ui.with(NodeSpec::row().role(Role::None), |ui| {
                    ui.text_in(
                        NodeSpec::row().height(20.0).role(Role::Line).caret(9),
                        "gutter",
                        mono(),
                    );
                });
                button = ui.leaf_keyed(
                    "wrap",
                    NodeSpec::row()
                        .size(20.0, 20.0)
                        .on_click("wrap")
                        .label("Wrap"),
                );
                for (n, text) in ["first line", "second"].iter().enumerate() {
                    let mut row = NodeSpec::row().height(20.0).role(Role::Line);
                    if let Some(b) = carets[n] {
                        row = row.caret(b);
                    }
                    ui.text_in(row, text, mono());
                }
            },
        );
        ui.finish();
        (sink, button)
    };
    let (sink, button) = frame(&mut core, &[Some(3), None]);
    core.set_focus(Some(sink));
    frame(&mut core, &[Some(3), None]);
    assert!(core.has_caret());
    let anchor = core.ime_rect().expect("the caret line anchors the IME");
    let stamp = core.caret_stamp();

    // Focus moves to the button inside the editor: keys still reach the
    // sink, and so the caret is still the sink's.
    core.set_focus(Some(button));
    frame(&mut core, &[Some(3), None]);
    assert_eq!(core.focus(), Some(button));
    assert!(
        core.has_caret(),
        "the enclosing sink's caret, not the button's none"
    );
    assert_eq!(core.ime_rect(), Some(anchor), "the anchor held");
    assert_eq!(core.caret_stamp(), stamp, "the caret did not move");

    // Two lines declare one: the last is the caret, as the access tree
    // reads it, and the gutter's never was.
    frame(&mut core, &[Some(3), Some(2)]);
    let moved = core.ime_rect().expect("still a caret");
    assert!(moved.y > anchor.y, "the second line's, below the first's");
    assert_ne!(core.caret_stamp(), stamp);

    // Focus outside the editor: no caret.
    core.set_focus(None);
    frame(&mut core, &[Some(3), Some(2)]);
    assert!(!core.has_caret());
    assert_eq!(core.ime_rect(), None);
}
