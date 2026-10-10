//! A character that ends a bidi paragraph without ending a line (backlog
//! FZ6, from the first fuzz round): U+001C–U+001E, NEL and U+2029 are
//! paragraph separators to Unicode's bidi algorithm and nothing to
//! cosmic-text's line breaking, which asserts that every paragraph in a
//! line runs the way the first does. One between left-to-right and
//! right-to-left text panicked the shaper — in a text node, a span, a
//! cell, an editor's seed or a paste. Each is shaped as a character that
//! ends nothing, the same length in UTF-8, and an editor takes U+2029 as
//! the newline it names.

use kui_core::{
    Cell, CellGrid, Core, EditKey, EditOptions, InputEvent, Mods, NodeSpec, Size, Span, TextStyle,
};

const MIXED: &[&str] = &[
    "abc\u{1c}\u{5d0}\u{5d1}",
    "abc\u{1d}\u{5d0}",
    "abc\u{1e}\u{5d0}\u{5d1}",
    "\u{5d0}\u{5d1}\u{85}abc",
    "abc\u{2029}\u{627}\u{644}",
    "\u{2029}\u{5d0}\u{2029}a\u{1e}\u{5d1}",
];

#[test]
fn a_paragraph_end_inside_a_line_shapes() {
    let mut core = Core::new();
    for text in MIXED {
        let style = TextStyle::new(14.0);
        let cells: Vec<Cell> = text.chars().map(|c| Cell::new(c, 0xffffffff, 0)).collect();
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        ui.text(text, style);
        ui.rich_text(&[Span::new(text), Span::new("\u{5d2}\u{2029}x")], style);
        let grid = CellGrid {
            rows: 1,
            cols: cells.len(),
            cells: &cells,
            style,
            cursor: None,
            origin_line: 0,
        };
        ui.cells_keyed("g", &grid, NodeSpec::column());
        let opts = EditOptions {
            style,
            multiline: true,
            ..Default::default()
        };
        ui.text_edit("many", text, &opts, NodeSpec::column().width(200.0));
        ui.text_edit(
            "one",
            text,
            &EditOptions {
                style,
                placeholder: Some((*text).to_string()),
                ..Default::default()
            },
            NodeSpec::column().width(200.0),
        );
        ui.finish();
        let _ = core.output();
    }
}

#[test]
fn an_editor_takes_the_paragraph_separator_as_a_newline() {
    let mut core = Core::new();
    let frame = |core: &mut Core, multiline: bool, seed: &str| {
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        let opts = EditOptions {
            style: TextStyle::new(14.0),
            multiline,
            autofocus: true,
            ..Default::default()
        };
        let k = ui.text_edit("e", seed, &opts, NodeSpec::column().width(200.0));
        ui.finish();
        k
    };
    let k = frame(&mut core, true, "ab\u{2029}\u{5d0}");
    assert_eq!(core.edit_text(k).as_deref(), Some("ab\n\u{5d0}"));
    // A multiline editor opens with the caret at the start: to the end.
    core.handle_input(InputEvent::Key(EditKey::End, Mods::NONE.with_doc()));
    core.handle_input(InputEvent::Text("\u{2029}\u{5d1}".into()));
    frame(&mut core, true, "");
    assert_eq!(core.edit_text(k).as_deref(), Some("ab\n\u{5d0}\n\u{5d1}"));
    // The IME composing one is shaped too, and committing it is typing.
    core.handle_input(InputEvent::Preedit("x\u{2029}\u{5d2}".into(), Some((1, 1))));
    frame(&mut core, true, "");
    core.handle_input(InputEvent::Commit("x\u{2029}\u{5d2}".into()));
    frame(&mut core, true, "");
    assert_eq!(
        core.edit_text(k).as_deref(),
        Some("ab\n\u{5d0}\n\u{5d1}x\n\u{5d2}")
    );

    // A field is one line: the separator goes where a newline does.
    let mut core = Core::new();
    let k = frame(&mut core, false, "ab\u{2029}\u{5d0}");
    assert_eq!(core.edit_text(k).as_deref(), Some("ab\u{5d0}"));
    assert!(core.set_edit_text(k, "c\u{1e}\u{5d1}"));
    frame(&mut core, false, "");
    assert_eq!(core.edit_text(k).as_deref(), Some("c\u{1f}\u{5d1}"));
}
