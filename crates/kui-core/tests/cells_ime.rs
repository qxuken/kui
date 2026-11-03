//! A terminal's cell cursor anchors the IME just as an editor's caret does.

use kui_core::cells::CellGrid;
use kui_core::{CellCursor, Color, Core, EditOptions, Key, NodeSpec, Rect, Size, TextStyle, Ui};

const CURSOR: Color = Color {
    r: 0.125,
    g: 0.875,
    b: 0.5,
    a: 1.0,
};

fn terminal(ui: &mut Ui<'_>, name: &str, cursor: Option<(usize, usize)>) -> Key {
    ui.with_keyed(
        name,
        NodeSpec::column().size(200.0, 120.0).pad(4.3).on_key(name),
        |ui| {
            ui.cells_keyed(
                "grid",
                &CellGrid {
                    rows: 4,
                    cols: 8,
                    cells: &[],
                    style: TextStyle::new(14.0).mono().line_height(20.0),
                    cursor: cursor.map(|(r, c)| (r, c, CellCursor::Block, CURSOR)),
                    origin_line: 900,
                },
                NodeSpec::column().pad(2.2),
            );
        },
    )
}

fn frame(core: &mut Core, focus: Key, cursor: Option<(usize, usize)>, scale: f32) {
    let mut ui = core.frame(Size::new(700.0, 300.0), scale);
    ui.configure_root(NodeSpec::row().fill().pad(10.3));
    ui.text_edit(
        "editor",
        "editor text",
        &EditOptions::default(),
        NodeSpec::column().size(130.0, 100.0),
    );
    terminal(&mut ui, "terminal", cursor);
    terminal(&mut ui, "other", Some((0, 0)));
    ui.focus(focus);
    ui.finish();
}

fn close(a: Rect, b: Rect) {
    for (a, b) in [(a.x, b.x), (a.y, b.y), (a.w, b.w), (a.h, b.h)] {
        assert!((a - b).abs() < 0.001, "{a} != {b}");
    }
}

#[test]
fn switching_from_an_editor_to_a_terminal_moves_the_ime_to_its_drawn_cursor() {
    for scale in [1.0, 1.5, 2.0] {
        let mut core = Core::new();
        let editor = Key::ROOT.str("editor");
        let term = Key::ROOT.str("terminal");
        frame(&mut core, editor, Some((1, 2)), scale);
        let old = core.ime_rect().expect("editor anchor");
        frame(&mut core, term, Some((1, 2)), scale);
        assert_eq!(core.focus(), Some(term));
        let anchor = core.ime_rect().expect("terminal cursor anchors the IME");
        assert_ne!(anchor, old, "the editor's anchor must not persist");
        let (display, _) = core.output();
        let drawn = display
            .quads
            .iter()
            .find(|q| q.color == CURSOR)
            .unwrap()
            .rect;
        close(anchor.scaled(scale), drawn);

        frame(&mut core, term, Some((2, 5)), scale);
        let moved = core.ime_rect().unwrap();
        close(
            moved,
            Rect::new(
                anchor.x + 3.0 * anchor.w,
                anchor.y + anchor.h,
                anchor.w,
                anchor.h,
            ),
        );
        assert!(!core.has_caret(), "the terminal still owns cursor blinking");

        frame(&mut core, term.str("grid"), Some((2, 5)), scale);
        close(core.ime_rect().unwrap(), moved);

        let other = Key::ROOT.str("other");
        frame(&mut core, other, Some((2, 5)), scale);
        assert_ne!(
            core.ime_rect(),
            Some(moved),
            "focus selects the other terminal"
        );
        frame(&mut core, editor, Some((2, 5)), scale);
        close(core.ime_rect().unwrap(), old);
    }
}

#[test]
fn the_terminal_anchor_follows_a_turned_pane() {
    let mut core = Core::new();
    let draw = |core: &mut Core, turn: f32| {
        let mut ui = core.frame(Size::new(700.0, 300.0), 1.5);
        ui.configure_root(NodeSpec::column().fill().pad(20.0));
        let card = ui.with_keyed(
            "card",
            NodeSpec::column().size(240.0, 160.0).rotate(turn),
            |ui| {
                terminal(ui, "terminal", Some((1, 2)));
            },
        );
        ui.focus(card.str("terminal"));
        ui.finish();
    };
    draw(&mut core, 0.0);
    let upright = core.ime_rect().unwrap();
    draw(&mut core, 0.5);
    let turned = core.ime_rect().unwrap();
    close(
        turned,
        Rect::new(
            280.0 - upright.x - upright.w,
            200.0 - upright.y - upright.h,
            upright.w,
            upright.h,
        ),
    );
}

#[test]
fn a_missing_or_out_of_bounds_cursor_drops_the_terminal_anchor() {
    let mut core = Core::new();
    let term = Key::ROOT.str("terminal");
    frame(&mut core, term, Some((1, 2)), 1.0);
    assert!(core.ime_rect().is_some());
    for cursor in [
        None,
        Some((4, 0)),
        Some((0, 8)),
        Some((usize::MAX, usize::MAX)),
    ] {
        frame(&mut core, term, cursor, 1.0);
        assert_eq!(
            core.ime_rect(),
            None,
            "{cursor:?}: no fallback to another pane"
        );
    }
    core.set_focus(None);
    let mut ui = core.frame(Size::new(700.0, 300.0), 1.0);
    terminal(&mut ui, "terminal", Some((1, 2)));
    ui.finish();
    assert_eq!(core.ime_rect(), None, "an unfocused cursor is no anchor");
}

#[test]
fn a_grid_can_be_the_sink_but_a_nested_sink_cannot_supply_its_parents_anchor() {
    let mut core = Core::new();
    for nested in [false, true] {
        let mut ui = core.frame(Size::new(700.0, 300.0), 1.0);
        let parent = ui.with_keyed("shell", NodeSpec::column().on_key("shell"), |ui| {
            ui.cells_keyed(
                "grid",
                &CellGrid {
                    rows: 2,
                    cols: 4,
                    cells: &[],
                    style: TextStyle::new(14.0).mono().line_height(20.0),
                    cursor: Some((1, 2, CellCursor::Underline, CURSOR)),
                    origin_line: 0,
                },
                NodeSpec::column().on_key("terminal"),
            );
        });
        ui.focus(if nested { parent } else { parent.str("grid") });
        ui.finish();
        if nested {
            assert_eq!(
                core.ime_rect(),
                None,
                "the nested terminal does not have focus"
            );
        } else {
            let anchor = core.ime_rect().expect("a grid can own its keys directly");
            assert_eq!(anchor.h, 20.0, "anchor the cell, not the thin underline");
        }
    }
}
