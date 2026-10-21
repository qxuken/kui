//! A click in a selection scope selects nothing, however the layout moves
//! under the pointer before the release (backlog F173): a press that
//! never leaves the click slop is a click, as an editor's is (F170). A
//! pane revealed by the press — a strip scrolling it into view while the
//! button is held — slides its text under the still pointer, and the
//! release, the frames while it was held and the pointer's shake used to
//! place the selection's live end on whatever text slid there.

use kui_core::{
    Cell, CellGrid, Core, FontFamily, InputEvent, Key, MouseButton, NodeSpec, Size, TextStyle, Vec2,
};

const COLS: usize = 30;
const ROWS: usize = 3;
const PANE_W: f32 = 300.0;

fn mono() -> TextStyle {
    TextStyle::new(14.0).family(FontFamily::Mono)
}

/// A strip two panes wide in a window one and a third wide: the second
/// pane's first 100 px show. It holds a `selectable` grid when `grid`,
/// a `selectable` column of three texts otherwise. The strip's key and
/// the scope's.
fn strip(core: &mut Core, grid: bool) -> (Key, Key) {
    let lines: Vec<String> = (0..ROWS)
        .map(|r| {
            (0..COLS)
                .map(|c| char::from(b'a' + ((r + c) % 26) as u8))
                .collect()
        })
        .collect();
    let mut cells = vec![Cell::new(' ', 0xffffffff, 0); ROWS * COLS];
    for (r, line) in lines.iter().enumerate() {
        for (c, ch) in line.chars().enumerate() {
            cells[r * COLS + c] = Cell::new(ch, 0xffffffff, 0);
        }
    }
    let mut ui = core.frame(Size::new(PANE_W + 100.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut scope = Key::ROOT;
    let strip = ui.with_keyed("strip", NodeSpec::row().fill().scroll_x(), |ui| {
        ui.leaf_keyed("a", NodeSpec::column().size(PANE_W, 200.0));
        ui.with_keyed("b", NodeSpec::column().size(PANE_W, 200.0), |ui| {
            if grid {
                let g = CellGrid {
                    rows: ROWS,
                    cols: COLS,
                    cells: &cells,
                    style: mono(),
                    cursor: None,
                    origin_line: 0,
                };
                scope = ui.child_key("term");
                ui.cells_keyed("term", &g, NodeSpec::column().fill().selectable());
            } else {
                scope = ui.with_keyed("text", NodeSpec::column().fill().selectable(), |ui| {
                    for (r, line) in lines.iter().enumerate() {
                        ui.text_in_keyed(
                            &format!("r{r}"),
                            NodeSpec::column().grow_width(),
                            line,
                            mono(),
                        );
                    }
                });
            }
        });
    });
    ui.finish();
    (strip, scope)
}

/// A press at `at` in the second pane, the strip scrolled 200 px under
/// the held button — the pane brought into view — with frames between,
/// the pointer moved by each of `shake` (from the press), and the
/// release.
fn click_while_revealed(core: &mut Core, grid: bool, shake: &[Vec2]) -> Key {
    let (strip_key, scope) = strip(core, grid);
    let at = Vec2::new(PANE_W + 50.0, 8.0);
    core.handle_input(InputEvent::CursorMoved(at));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.set_scroll(strip_key, Vec2::new(200.0, 0.0));
    strip(core, grid);
    strip(core, grid);
    for d in shake {
        core.handle_input(InputEvent::CursorMoved(Vec2::new(at.x + d.x, at.y + d.y)));
        strip(core, grid);
    }
    assert_eq!(core.scroll_offset(strip_key).x, 200.0, "the pane moved");
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    strip(core, grid);
    scope
}

#[test]
fn a_click_in_a_grid_that_slides_under_the_pointer_selects_nothing() {
    for shake in [
        &[][..],
        &[Vec2::new(1.0, 0.0), Vec2::new(-1.0, 1.0)][..],
        &[Vec2::new(2.5, 0.0), Vec2::new(0.0, -2.5)][..],
    ] {
        let mut core = Core::new();
        let scope = click_while_revealed(&mut core, true, shake);
        let sel = core.cell_selection().expect("the press placed both ends");
        assert_eq!(sel.node, scope);
        assert_eq!(
            sel.anchor, sel.focus,
            "shaken {shake:?}: the click left the ends together, not 200 px of cells"
        );
        assert_eq!(core.copy_selection(), None, "shaken {shake:?}");
    }
}

#[test]
fn a_click_in_text_that_slides_under_the_pointer_selects_nothing() {
    for shake in [&[][..], &[Vec2::new(2.0, 1.0)][..]] {
        let mut core = Core::new();
        click_while_revealed(&mut core, false, shake);
        assert_eq!(
            core.selection_text().unwrap_or_default(),
            "",
            "shaken {shake:?}: nothing selected"
        );
    }
}

#[test]
fn a_drag_past_the_slop_still_selects_from_the_press() {
    let mut core = Core::new();
    click_while_revealed(&mut core, true, &[Vec2::new(4.0, 0.0)]);
    let sel = core.cell_selection().expect("a cell selection");
    assert_ne!(
        sel.anchor, sel.focus,
        "once the pointer left the slop the press is a drag, and its end \
         is where the pointer is over the text that moved"
    );
    assert_eq!(sel.anchor.line, 0);
    assert!(
        sel.focus.col > sel.anchor.col,
        "the text slid left under the pointer: the end is after the anchor"
    );
}
