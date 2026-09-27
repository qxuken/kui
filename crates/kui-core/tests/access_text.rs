//! The access tree below the node: an editor's laid-out lines as runs
//! with every character placed, its caret and selection as positions in
//! them, selection requests resolving in the core — and an app that owns
//! its text getting the same tree from `role="line"` nodes, with the
//! requests it alone can honour coming back as events.

use kui_core::{
    AccessAction, AccessRequest, Core, EditOptions, InputEvent, Key, NodeSpec, Role, Size, TextPos,
    TextStyle, Value,
};

fn editor_frame(core: &mut Core, initial: &str) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(10.0));
    let key = ui.text_edit(
        "doc",
        initial,
        &EditOptions {
            multiline: true,
            autofocus: true,
            ..Default::default()
        },
        NodeSpec::column().width(300.0).pad(4.0).label("Doc"),
    );
    ui.finish();
    key
}

#[test]
fn a_built_in_editor_exposes_its_lines_as_runs() {
    let mut core = Core::new();
    let key = editor_frame(&mut core, "hello world\nsecond line");
    let tree = core.access_tree().clone();
    let node = tree.get(key).unwrap();
    assert_eq!(node.role, Role::MultilineTextInput);
    assert_eq!(node.value.as_deref(), Some("hello world\nsecond line"));
    assert!(node.focused);

    // Two lines, two runs; the first ends with its newline, a character
    // of its own with no width.
    assert_eq!(node.runs.len(), 2);
    let (a, b) = (&node.runs[0], &node.runs[1]);
    assert_eq!(a.text, "hello world\n");
    assert_eq!((a.line, a.start, a.end), (0, 0, 11));
    assert_eq!(a.char_lengths.len(), 12);
    assert!(a.char_lengths[..11].iter().all(|&l| l == 1));
    assert_eq!(a.word_starts, vec![0, 6]);
    assert_eq!(a.char_widths.len(), 12);
    assert!(a.char_widths[0] > 0.0);
    assert_eq!(a.char_widths[11], 0.0, "the newline has no width");
    assert!(
        a.char_positions.windows(2).all(|w| w[0] <= w[1]),
        "{:?}",
        a.char_positions
    );
    assert_eq!(
        a.char_positions[0], 0.0,
        "positions are relative to the run"
    );
    assert_eq!(b.text, "second line");
    assert_eq!((b.line, b.start, b.end), (1, 0, 11));
    assert_eq!(b.word_starts, vec![0, 7]);

    // Runs sit inside the node's content box, one under the other.
    assert!(a.rect.x >= node.rect.x + 4.0 - 0.01);
    assert!(a.rect.y >= node.rect.y + 4.0 - 0.01);
    assert!(b.rect.y > a.rect.y);
    assert!(a.rect.w > 0.0 && a.rect.h > 0.0);

    // A fresh caret: at the start, no selection.
    let start = TextPos {
        run: a.key,
        character: 0,
    };
    assert_eq!(node.focus, Some(start));
    assert_eq!(node.anchor, Some(start));
    assert_eq!(node.caret, Some(0));
    assert!(node.supports(AccessAction::SetTextSelection));
    assert!(node.supports(AccessAction::ReplaceSelectedText));

    // The same lines, the same runs: the hash is steady.
    let hash = tree.hash;
    editor_frame(&mut core, "ignored: the editor is retained");
    assert_eq!(core.access_tree().hash, hash);
}

#[test]
fn a_long_line_wraps_into_runs_and_splits_past_the_character_cap() {
    let mut core = Core::new();
    let long: String = (0..60).map(|i| format!("w{i} ")).collect();
    let key = editor_frame(&mut core, &long);
    let tree = core.access_tree().clone();
    let node = tree.get(key).unwrap();
    // Wrapped at 300px: several visual lines, all of buffer line 0, byte
    // ranges tiling the text in order without a newline anywhere.
    assert!(node.runs.len() > 1);
    let mut at = 0;
    for r in &node.runs {
        assert_eq!(r.line, 0);
        assert!(r.start >= at, "{} < {at}", r.start);
        assert!(r.char_lengths.len() <= kui_core::access::RUN_CHARS);
        assert!(!r.text.contains('\n'));
        assert_eq!(&long[r.start..r.end], r.text);
        at = r.end;
    }
    assert_eq!(at, long.trim_end().len().max(at.min(long.len())));
    // Later runs sit lower; positions inside each run start at zero.
    assert!(node.runs[1].rect.y > node.runs[0].rect.y);
    assert!(node.runs.iter().all(|r| r.char_positions[0] == 0.0));
}

#[test]
fn selection_requests_move_the_caret_and_type_over_it() {
    let mut core = Core::new();
    let key = editor_frame(&mut core, "hello world\nsecond line");
    let tree = core.access_tree().clone();
    let node = tree.get(key).unwrap();
    let (r0, r1) = (node.runs[0].key, node.runs[1].key);

    // "world\nsecond": from character 6 of line 0 to character 6 of line 1.
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(key, AccessAction::SetTextSelection).with_selection(
            TextPos {
                run: r0,
                character: 6,
            },
            TextPos {
                run: r1,
                character: 6,
            },
        ),
    ));
    assert!(evs.is_empty(), "a selection is not an edit");
    assert_eq!(core.copy_selection().as_deref(), Some("world\nsecond"));
    editor_frame(&mut core, "");
    let tree = core.access_tree().clone();
    let node = tree.get(key).unwrap();
    assert_eq!(
        node.anchor,
        Some(TextPos {
            run: node.runs[0].key,
            character: 6
        })
    );
    assert_eq!(
        node.focus,
        Some(TextPos {
            run: node.runs[1].key,
            character: 6
        })
    );
    assert_eq!(node.caret, Some(18));
    assert_eq!(node.selection, Some((6, 18)));

    // Typing over it: one changed event, the text follows, the caret
    // lands after what was typed.
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(key, AccessAction::ReplaceSelectedText).with_value("there"),
    ));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].kind(), Some("changed"));
    assert_eq!(core.edit_text(key).as_deref(), Some("hello there line"));
    editor_frame(&mut core, "");
    let node = core.access_tree().get(key).unwrap().clone();
    assert_eq!(node.caret, Some(11));
    assert_eq!(node.selection, None);
    assert_eq!(node.runs.len(), 1);

    // A collapsed request at a line's end is the caret before its newline.
    let key2 = {
        let mut core2 = Core::new();
        let k = editor_frame(&mut core2, "ab\ncd");
        let r0 = core2.access_tree().get(k).unwrap().runs[0].key;
        let at_end = TextPos {
            run: r0,
            character: 2,
        };
        core2.handle_input(InputEvent::Access(
            AccessRequest::new(k, AccessAction::SetTextSelection).with_selection(at_end, at_end),
        ));
        editor_frame(&mut core2, "");
        let node = core2.access_tree().get(k).unwrap().clone();
        assert_eq!(node.caret, Some(2));
        assert_eq!(node.selection, None);
        assert_eq!(node.focus, Some(at_end));
        // Past the newline clamps to the same place.
        let past = TextPos {
            run: r0,
            character: 3,
        };
        core2.handle_input(InputEvent::Access(
            AccessRequest::new(k, AccessAction::SetTextSelection).with_selection(past, past),
        ));
        editor_frame(&mut core2, "");
        assert_eq!(core2.access_tree().get(k).unwrap().caret, Some(2));
        k
    };
    assert_ne!(key2, Key::ROOT);
}

/// An app that owns its text: an `on_key` sink declared a multiline
/// editor, a decorative gutter, each drawn line a `role="line"` row made
/// of two text nodes, caret and anchor offsets on the lines holding them.
fn custom_frame(
    core: &mut Core,
    lines: &[&str],
    caret: (usize, u32),
    anchor: Option<(usize, u32)>,
) -> Key {
    let style = TextStyle::new(14.0);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let sink = ui.with_keyed(
        "sink",
        NodeSpec::column()
            .fill()
            .on_key("ed")
            .role(Role::MultilineTextInput)
            .label("Doc"),
        |ui| {
            ui.with(NodeSpec::row(), |ui| {
                ui.with(NodeSpec::column().role(Role::None), |ui| {
                    for i in 0..lines.len() {
                        ui.text(&format!("{}", i + 1), style);
                    }
                });
                ui.with(NodeSpec::column(), |ui| {
                    for (i, l) in lines.iter().enumerate() {
                        let mut spec = NodeSpec::row().role(Role::Line);
                        if caret.0 == i {
                            spec = spec.caret(caret.1);
                        }
                        if let Some(a) = anchor
                            && a.0 == i
                        {
                            spec = spec.selection_anchor(a.1);
                        }
                        ui.with_keyed(&format!("l{i}"), spec, |ui| {
                            let (a, b) = l.split_at(l.len() / 2);
                            ui.text(a, style);
                            ui.text(b, style);
                        });
                    }
                });
            });
        },
    );
    ui.take_key_focus(sink);
    ui.finish();
    sink
}

#[test]
fn a_custom_editor_declares_its_lines_caret_and_selection() {
    let mut core = Core::new();
    let lines = ["fn main() {", "    hi", "}"];
    let sink = custom_frame(&mut core, &lines, (1, 6), Some((0, 3)));
    let tree = core.access_tree().clone();
    let node = tree.get(sink).unwrap();
    assert_eq!(node.role, Role::MultilineTextInput);
    assert_eq!(node.name.as_deref(), Some("Doc"));
    assert!(node.focused, "the key sink holds focus");
    assert_eq!(tree.focus, Some(sink));
    assert_eq!(node.value.as_deref(), Some("fn main() {\n    hi\n}"));
    // The gutter and the line rows are not nodes: the editor is the leaf.
    assert_eq!(tree.nodes.len(), 2);

    // Two text nodes per line, so two runs; the second carries the
    // newline, and byte ranges continue across them.
    assert_eq!(node.runs.len(), 6);
    assert_eq!(node.runs[0].text, "fn ma");
    assert_eq!(
        (node.runs[0].line, node.runs[0].start, node.runs[0].end),
        (0, 0, 5)
    );
    assert_eq!(node.runs[1].text, "in() {\n");
    assert_eq!(
        (node.runs[1].line, node.runs[1].start, node.runs[1].end),
        (0, 5, 11)
    );
    assert!(node.runs[1].rect.x > node.runs[0].rect.x);
    assert_eq!(node.runs[5].text, "}");
    assert_eq!(node.runs[4].text, "");
    assert_eq!(node.runs[4].line, 2);

    // Caret at the end of line 1 (offset 6): the line's last run, at its
    // end; the anchor three bytes into line 0's first run.
    assert_eq!(
        node.focus,
        Some(TextPos {
            run: node.runs[3].key,
            character: 3
        })
    );
    assert_eq!(
        node.anchor,
        Some(TextPos {
            run: node.runs[0].key,
            character: 3
        })
    );
    assert_eq!(node.caret, Some(18));
    assert_eq!(node.selection, Some((3, 18)));
    assert!(node.supports(AccessAction::SetTextSelection));
    assert!(node.supports(AccessAction::Focus));

    // Requests the core cannot honour come back as data, in the app's
    // own terms: line ordinals and byte offsets.
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(sink, AccessAction::SetTextSelection).with_selection(
            TextPos {
                run: node.runs[0].key,
                character: 1,
            },
            TextPos {
                run: node.runs[3].key,
                character: 3,
            },
        ),
    ));
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, sink);
    let pos = |line: i64, offset: i64| {
        Value::map([("line", Value::Int(line)), ("offset", Value::Int(offset))])
    };
    assert_eq!(
        evs[0].payload,
        Value::map([
            ("kind", Value::str("access")),
            ("action", Value::str("setTextSelection")),
            ("anchor", pos(0, 1)),
            ("focus", pos(1, 6)),
            ("tag", Value::str("ed")),
        ])
    );
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(sink, AccessAction::ReplaceSelectedText).with_value("x"),
    ));
    assert_eq!(
        evs[0].payload,
        Value::map([
            ("kind", Value::str("access")),
            ("action", Value::str("replaceSelectedText")),
            ("text", Value::str("x")),
            ("tag", Value::str("ed")),
        ])
    );
    let evs = core.handle_input(InputEvent::Access(
        AccessRequest::new(sink, AccessAction::SetValue).with_value("all new"),
    ));
    assert_eq!(evs[0].payload.get_str("action"), Some("setValue"));
    assert_eq!(evs[0].payload.get_str("text"), Some("all new"));

    // No caret declared: no positions, value still there.
    let mut core = Core::new();
    let sink = custom_frame(&mut core, &lines, (99, 0), None);
    let node = core.access_tree().get(sink).unwrap().clone();
    assert_eq!(node.focus, None);
    assert_eq!(node.caret, None);
    assert_eq!(node.value.as_deref(), Some("fn main() {\n    hi\n}"));
}

#[test]
fn a_key_sink_is_a_focusable_group() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let sink = ui.leaf_keyed(
        "canvas",
        NodeSpec::column().fill().key_sink().label("Canvas"),
    );
    ui.finish();
    let tree = core.access_tree().clone();
    let node = tree.get(sink).expect("a key sink is in the tree");
    assert_eq!(node.role, Role::Group);
    assert!(node.supports(AccessAction::Focus));
    assert!(!node.focused);
    core.handle_input(InputEvent::Access(AccessRequest::new(
        sink,
        AccessAction::Focus,
    )));
    assert_eq!(core.key_focus(), Some(sink));
}
