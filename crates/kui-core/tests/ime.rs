//! IME for an editor the app owns (backlog C17). The stock editor took
//! every composition; a custom editor — an `onKey` sink drawing `line`
//! rows with `caret` — got nothing: no preedit, no commit (the platform
//! reports neither as a key press with `text`), and the OS candidate
//! window at the window's origin. Now both arrive on the sink as data and
//! the candidate window is anchored at the `line`'s caret.

use kui_core::testing::kinds;
use kui_core::{Color, Core, InputEvent, Key, NodeSpec, Role, Size, TextStyle, Value};

const LH: f32 = 20.0;

fn mono() -> TextStyle {
    TextStyle::new(14.0).mono().line_height(LH)
}

/// A custom editor: one sink with two lines, the caret on the second at
/// byte `caret`. Returns the sink's key and the second line's.
fn frame(core: &mut Core, caret: u32) -> (Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::column()
            .on_key("ed")
            .role(Role::MultilineTextInput)
            .label("Buffer"),
        |ui| {
            ui.text_in_keyed(
                "l0",
                NodeSpec::row().height(LH).role(Role::Line),
                "first line",
                mono(),
            );
            ui.with_keyed(
                "l1",
                NodeSpec::row().height(LH).role(Role::Line).caret(caret),
                |ui| {
                    ui.text("let ", mono().color(Color::rgb8(200, 100, 255)));
                    ui.text("value", mono());
                },
            );
        },
    );
    ui.finish();
    (sink, sink.str("l1"))
}

/// A field of the event's data: `{tag: "ed", preedit: {text, cursor}}`
/// reads `text` and `cursor` from the `preedit` (ADR 0047).
fn field<'a>(ev: &'a kui_core::UiEvent, name: &str) -> Option<&'a Value> {
    ev.data()?.get(name)
}

#[test]
fn a_composition_reaches_the_focused_sink_as_data() {
    let mut core = Core::new();
    let (sink, _) = frame(&mut core, 6);
    core.set_focus(Some(sink));
    frame(&mut core, 6);

    let pre = core.handle_input(InputEvent::Preedit("日本".into(), Some((0, 6))));
    assert_eq!(kinds(&pre), ["preedit"]);
    assert_eq!(pre[0].key, sink);
    assert_eq!(field(&pre[0], "text"), Some(&Value::str("日本")));
    assert_eq!(
        field(&pre[0], "cursor"),
        Some(&Value::List(vec![Value::Int(0), Value::Int(6)]))
    );
    assert_eq!(
        pre[0].payload.get("tag"),
        Some(&Value::str("ed")),
        "the sink's tag rides along"
    );

    let done = core.handle_input(InputEvent::Commit("日本語".into()));
    assert_eq!(kinds(&done), ["text"]);
    assert_eq!(field(&done[0], "text"), Some(&Value::str("日本語")));
    assert_eq!(done[0].payload.get("tag"), Some(&Value::str("ed")));

    // A composition that ends without a commit says so: empty text, no cursor.
    let gone = core.handle_input(InputEvent::Preedit(String::new(), None));
    assert_eq!(kinds(&gone), ["preedit"]);
    assert_eq!(field(&gone[0], "text"), Some(&Value::str("")));
    assert_eq!(field(&gone[0], "cursor"), Some(&Value::Null));
}

/// Plain typing is not a commit: the sink hears the key press with its
/// `text`, and the `Text` channel — which every driver sends beside the
/// press — stays away from it, so nothing is typed twice.
#[test]
fn typing_reaches_a_sink_once() {
    let mut core = Core::new();
    let (sink, _) = frame(&mut core, 0);
    core.set_focus(Some(sink));
    frame(&mut core, 0);
    let events = core.press(kui_core::KeyPress {
        text: Some("a".into()),
        ..kui_core::KeyPress::new(kui_core::KeyCode::Char('a'), kui_core::KeyMods::default())
    });
    assert_eq!(kinds(&events), ["key"], "one event for one keystroke");
    let events = core.handle_input(InputEvent::Text("a".into()));
    assert!(events.is_empty(), "the text channel alone reaches no sink");
}

/// The candidate window goes where the `line` carrying `caret` says.
#[test]
fn the_ime_rect_follows_the_custom_caret() {
    let mut core = Core::new();
    let (sink, line) = frame(&mut core, 6);
    core.set_focus(Some(sink));
    frame(&mut core, 6);
    let rect = core
        .ime_rect()
        .expect("a caret row anchors the candidate window");
    assert_eq!(Some(rect), core.caret_rect(line, 6));
    let w = core.measure_text("M", &mono(), None).width;
    assert!((rect.x - (10.0 + 6.0 * w)).abs() < 0.75, "{}", rect.x);
    assert_eq!(rect.y, 10.0 + LH, "the second line");
    assert_eq!(rect.h, LH);
    // The caret moves, the anchor moves with it.
    frame(&mut core, 2);
    let moved = core.ime_rect().unwrap();
    assert!(moved.x < rect.x);
    // Nothing focused: nothing to anchor.
    core.set_focus(None);
    frame(&mut core, 2);
    assert_eq!(core.ime_rect(), None);
}

/// A textless line still has a caret. Losing its anchor left the native
/// window at the previous nonempty line while the editor kept moving.
#[test]
fn the_ime_anchor_follows_moves_onto_blank_lines() {
    let mut core = Core::new();
    let draw = |core: &mut Core, current: usize| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill().pad(10.0));
        let sink = ui.with_keyed("editor", NodeSpec::column().on_key("ed"), |ui| {
            for (i, text) in ["before", "", "after"].iter().enumerate() {
                let mut row = NodeSpec::row().width(300.0).height(LH).role(Role::Line);
                if i == current {
                    row = row.caret(0).caret_solid();
                }
                ui.with_indexed(i as u64, row, |ui| {
                    if !text.is_empty() {
                        ui.text(text, mono());
                    }
                });
            }
        });
        ui.take_key_focus(sink);
        ui.finish();
    };
    for current in [0, 1, 2, 1, 0] {
        draw(&mut core, current);
        let anchor = core
            .ime_rect()
            .expect("every line, including blank, anchors the IME");
        assert_eq!(anchor.x, 10.0);
        assert_eq!(anchor.y, 10.0 + current as f32 * LH);
        assert_eq!(anchor.h, LH);
        assert!(!core.has_caret(), "a solid caret still belongs to the app");
    }
}

#[test]
fn an_empty_line_anchors_inside_its_padding_and_ignores_decoration() {
    for empty_text in [false, true] {
        for scale in [1.0, 1.5, 2.0] {
            let mut core = Core::new();
            let mut ui = core.frame(Size::new(400.0, 300.0), scale);
            ui.configure_root(NodeSpec::column().fill().pad(10.0));
            let sink = ui.with_keyed("editor", NodeSpec::column().on_key("ed"), |ui| {
                ui.with(
                    NodeSpec::row()
                        .width(300.0)
                        .height(26.0)
                        .pad_xy(8.0, 3.0)
                        .role(Role::Line)
                        .caret(0),
                    |ui| {
                        ui.text_in(
                            NodeSpec::row()
                                .role(Role::None)
                                .float(kui_core::FloatConfig::parent()),
                            "decoration",
                            mono(),
                        );
                        if empty_text {
                            ui.text("", mono());
                        }
                    },
                );
            });
            ui.take_key_focus(sink);
            ui.finish();
            assert_eq!(
                core.ime_rect(),
                Some(kui_core::Rect::new(18.0, 13.0, 0.0, 20.0))
            );
        }
    }
}

#[test]
fn an_empty_run_marks_the_caret_column_after_a_gutter() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let sink = ui.with_keyed("editor", NodeSpec::column().on_key("ed"), |ui| {
        ui.with(
            NodeSpec::row()
                .width(300.0)
                .height(LH)
                .role(Role::Line)
                .caret(0),
            |ui| {
                ui.text_in(
                    NodeSpec::row().width(56.0).height(LH).role(Role::None),
                    "73",
                    mono(),
                );
                ui.text("", mono());
            },
        );
    });
    ui.take_key_focus(sink);
    ui.finish();
    assert_eq!(
        core.ime_rect(),
        Some(kui_core::Rect::new(66.0, 10.0, 0.0, LH))
    );
}

#[test]
fn a_clipped_line_does_not_get_an_empty_line_anchor() {
    for text in ["", "outside"] {
        let mut core = Core::new();
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        let sink = ui.with_keyed(
            "editor",
            NodeSpec::column()
                .width(300.0)
                .height(20.0)
                .clip()
                .on_key("ed"),
            |ui| {
                ui.leaf(NodeSpec::column().height(40.0).min_height(40.0));
                ui.with(
                    NodeSpec::row()
                        .width(300.0)
                        .height(LH)
                        .min_height(LH)
                        .role(Role::Line)
                        .caret(0),
                    |ui| {
                        if !text.is_empty() {
                            ui.text(text, mono());
                        }
                    },
                );
            },
        );
        ui.take_key_focus(sink);
        ui.finish();
        assert_eq!(
            core.ime_rect(),
            None,
            "{text:?}: the line is outside its clip"
        );
    }
}

#[test]
fn an_empty_line_anchor_follows_its_turn() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.5);
    ui.configure_root(NodeSpec::column().fill().pad(20.0));
    let sink = ui.with_keyed(
        "editor",
        NodeSpec::column()
            .size(200.0, 40.0)
            .pad(8.0)
            .rotate(0.5)
            .on_key("ed"),
        |ui| {
            ui.leaf(NodeSpec::row().height(LH).role(Role::Line).caret(0));
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    let anchor = core.ime_rect().unwrap();
    // The upright caret at (28, 28) turns about the card's (120, 40).
    assert!((anchor.x - 212.0).abs() < 0.001);
    assert!((anchor.y - 32.0).abs() < 0.001);
    assert!((anchor.h - LH).abs() < 0.001);
}

/// The stock editor still takes a composition itself: a commit there is a
/// `change`, not a `text` event.
#[test]
fn a_stock_editor_takes_the_commit_itself() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.text_edit(
        "note",
        "",
        &kui_core::EditOptions {
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().grow_width().on_change(Value::Null),
    );
    ui.finish();
    let events = core.handle_input(InputEvent::Commit("日本語".into()));
    assert_eq!(kinds(&events), ["change"]);
    assert_eq!(core.edit_text(key).as_deref(), Some("日本語"));
}

/// A stock editor's composition is typed text in waiting (backlog FZ7,
/// from the first fuzz round): a `\r` in it went into the buffer as it
/// came, cosmic-text broke the line there, and the composition's end -
/// counted by `\n` alone - lay past the line, so the key that ended it
/// split the buffer off a character boundary, a panic in cosmic-text. What
/// typing drops is dropped from a composition, and the IME's offsets move
/// with what stays.
#[test]
fn a_composition_holds_what_typing_would() {
    use kui_core::{EditKey, EditOptions, Mods};
    let frame = |core: &mut Core, multiline: bool| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        let opts = EditOptions {
            style: mono(),
            multiline,
            autofocus: true,
            ..Default::default()
        };
        let k = ui.text_edit("e", "ab", &opts, NodeSpec::column().width(300.0));
        ui.finish();
        k
    };
    for multiline in [false, true] {
        // A core of its own: the Escape below lets go of the field.
        let mut core = Core::new();
        let k = frame(&mut core, multiline);
        // The caret after `x\r` reported at byte 2, which the `\r` going
        // moves to 1; the commit is typed text, without it.
        core.handle_input(InputEvent::Preedit("x\ry".into(), Some((2, 2))));
        frame(&mut core, multiline);
        assert_eq!(core.edit_text(k).as_deref(), Some("ab"), "{multiline}");
        core.handle_input(InputEvent::Commit("x\ry".into()));
        frame(&mut core, multiline);
        // A field opens with its caret after its text, a multiline editor
        // at its start.
        let want = if multiline { "xyab" } else { "abxy" };
        assert_eq!(core.edit_text(k).as_deref(), Some(want), "{multiline}");
        // Every key that ends a composition ends this one cleanly.
        for key in [EditKey::Escape, EditKey::Left, EditKey::Backspace] {
            core.handle_input(InputEvent::Preedit("\r\u{1e}z\r".into(), None));
            frame(&mut core, multiline);
            core.handle_input(InputEvent::Key(key, Mods::NONE));
            frame(&mut core, multiline);
        }
    }
}

/// A stock editor's caret is a place in its committed text while a
/// composition is under way, as `caret_and_selection` promises and the
/// access tree reads it: the composition sits in the buffer to be drawn,
/// and a caret inside it reads as where it starts (found by the `events`
/// fuzz target: a composition into an empty field put the caret at 1 of
/// "").
#[test]
fn a_composition_moves_no_caret_in_the_committed_text() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        let mut ui = core.frame(Size::new(300.0, 100.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let key = ui.text_edit(
            "note",
            "ab",
            &kui_core::EditOptions {
                autofocus: true,
                ..Default::default()
            },
            NodeSpec::column().grow_width(),
        );
        ui.finish();
        key
    };
    let key = build(&mut core);
    assert_eq!(core.edit.caret_and_selection(key), Some((2, None)));
    core.handle_input(InputEvent::Preedit("にほ".into(), None));
    build(&mut core);
    assert_eq!(core.edit_text(key).as_deref(), Some("ab"));
    assert_eq!(
        core.edit.caret_and_selection(key),
        Some((2, None)),
        "the caret where the composition starts, not inside it"
    );
    core.handle_input(InputEvent::Commit("日本".into()));
    build(&mut core);
    assert_eq!(core.edit_text(key).as_deref(), Some("ab日本"));
    assert_eq!(core.edit.caret_and_selection(key), Some((8, None)));
}
