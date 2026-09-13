//! A held drag follows its scroller, and Shift extends
//! (`docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md`,
//! backlog C39): the live end is placed again when the layout under a
//! still pointer moves (a wheel notch, a nudge), the core steps the
//! scroller while the pointer is held past its edge, a Shift-press keeps
//! the anchor, and an `on_scroll` node hears the wheel — in lines on a
//! `cells` grid, where a held drag past the edge arrives the same way.

use kui_core::runtime::follow::{AUTOSCROLL_RATE, CLOCKLESS_FRAME};
use kui_core::testing::{press, release};
use kui_core::{
    Cell, CellGrid, Core, EditOptions, InputEvent, Key, KeyMods, NodeSpec, Size, Sizing, TextStyle,
    Value, Vec2,
};

const ROW_H: f32 = 20.0;
const VIEW_H: f32 = 60.0;

fn style() -> TextStyle {
    TextStyle::new(14.0)
}

/// A `selectable` scroller three rows tall over ten rows of text.
fn list(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let scope = ui.with_keyed(
        "list",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(VIEW_H))
            .scroll_y()
            .selectable(),
        |ui| {
            for i in 0..10 {
                ui.with_keyed(
                    &format!("r{i}"),
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Fixed(ROW_H)),
                    |ui| ui.text(&format!("row {i}"), style()),
                );
            }
        },
    );
    ui.finish();
    scope
}

fn shift(core: &mut Core, on: bool) {
    core.handle_input(InputEvent::Modifiers(KeyMods {
        shift: on,
        ..KeyMods::default()
    }));
}

#[test]
fn a_wheel_notch_under_a_held_press_moves_the_live_end() {
    let mut core = Core::new();
    let scope = list(&mut core);
    // Press in row 0, drag to row 1, hold.
    press(&mut core, Vec2::new(1.0, 6.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, ROW_H + 6.0)));
    assert_eq!(core.selection_text().as_deref(), Some("row 0\nrow 1"));
    // Two rows scroll under the still pointer.
    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -2.0 * ROW_H)));
    list(&mut core);
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1"),
        "the frame that lays the scroll out paints the old end — the text \
         moved, the highlight follows a frame later"
    );
    list(&mut core);
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2\nrow 3"),
        "the end is placed again where the pointer is: row 3 is under it now"
    );
    assert_eq!(core.scroll_offset(scope).y, 2.0 * ROW_H);
    // Release: nothing more moves.
    release(&mut core);
    list(&mut core);
    list(&mut core);
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2\nrow 3")
    );
}

#[test]
fn a_press_held_past_the_edge_scrolls_at_a_rate_from_the_distance() {
    let mut core = Core::new();
    let scope = list(&mut core);
    press(&mut core, Vec2::new(1.0, 6.0));
    // 60 px below the scroller's bottom edge: 600 px/s, 10 px a clockless
    // frame.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, VIEW_H + 60.0)));
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2"),
        "past the edge the hit clamps to the last drawn row"
    );
    let step = 60.0 * AUTOSCROLL_RATE * CLOCKLESS_FRAME as f32;
    assert!((step - 10.0).abs() < 1e-4);
    list(&mut core);
    assert!(
        core.animating(),
        "while the pointer is held past the edge the core asks for frames"
    );
    assert!(
        (core.scroll_offset(scope).y - step).abs() < 1e-3,
        "one step"
    );
    for _ in 0..5 {
        list(&mut core);
    }
    assert!(
        (core.scroll_offset(scope).y - 6.0 * step).abs() < 1e-3,
        "a step a frame: {}",
        core.scroll_offset(scope).y
    );
    // 60 px scrolled: rows 3..6 are on screen and the end followed.
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2\nrow 3\nrow 4\nrow 5"),
        "{:?}",
        core.selection_text()
    );
    // The clamp: the list stops at its end, and so does the request.
    for _ in 0..40 {
        list(&mut core);
    }
    assert_eq!(core.scroll_offset(scope).y, 10.0 * ROW_H - VIEW_H);
    release(&mut core);
    list(&mut core);
    assert!(
        !core.animating(),
        "released: nothing asks for a frame any more"
    );
    assert!(core.selection_text().unwrap().ends_with("row 9"));
}

#[test]
fn the_rate_is_the_clocks_when_there_is_one() {
    let mut core = Core::new();
    let scope = list(&mut core);
    core.set_time(1.0);
    list(&mut core);
    press(&mut core, Vec2::new(1.0, 6.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, VIEW_H + 60.0)));
    // Time has not passed: no step.
    list(&mut core);
    assert_eq!(core.scroll_offset(scope).y, 0.0);
    // 50 ms at 600 px/s.
    core.set_time(1.05);
    list(&mut core);
    assert!((core.scroll_offset(scope).y - 30.0).abs() < 1e-3);
    // A frame that took a second is capped: a hidden window coming back
    // must not scroll a second's worth.
    core.set_time(2.05);
    list(&mut core);
    assert!((core.scroll_offset(scope).y - 90.0).abs() < 1e-3);
    release(&mut core);
}

#[test]
fn a_shift_press_in_the_scope_keeps_the_anchor() {
    let mut core = Core::new();
    let _ = list(&mut core);
    // A click in row 0 places both ends.
    press(&mut core, Vec2::new(1.0, 6.0));
    release(&mut core);
    assert!(core.selection().is_some_and(|s| s.is_empty()));
    // Shift-click in row 2 extends from it.
    shift(&mut core, true);
    press(&mut core, Vec2::new(200.0, 2.0 * ROW_H + 6.0));
    release(&mut core);
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2")
    );
    // Shift-drag back into row 1 goes on from the same anchor, by
    // characters whatever the click count.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, ROW_H + 6.0)));
    core.handle_input(InputEvent::mouse_down(2));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, ROW_H + 6.0)));
    release(&mut core);
    assert_eq!(core.selection_text().as_deref(), Some("row 0\nrow"));
    // Without Shift a press starts over.
    shift(&mut core, false);
    press(&mut core, Vec2::new(200.0, 2.0 * ROW_H + 6.0));
    release(&mut core);
    assert!(core.selection().is_some_and(|s| s.is_empty()));
}

fn editor(core: &mut Core) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.child_key("field");
    ui.text_edit(
        "field",
        "one two three",
        &EditOptions::default(),
        NodeSpec::column().width(Sizing::Fixed(300.0)),
    );
    ui.finish();
    key
}

#[test]
fn a_shift_press_in_the_focused_editor_extends_from_its_caret() {
    let mut core = Core::new();
    let key = editor(&mut core);
    // Click after "one" places the caret there.
    let end_of_one = core.measure_text("one", &TextStyle::default(), None).width;
    press(&mut core, Vec2::new(end_of_one, 8.0));
    release(&mut core);
    assert_eq!(core.focus(), Some(key));
    assert!(core.copy_selection().is_none_or(|t| t.is_empty()));
    // Shift-click at the end selects from the caret to there.
    shift(&mut core, true);
    press(&mut core, Vec2::new(299.0, 8.0));
    release(&mut core);
    assert_eq!(
        core.copy_selection().as_deref(),
        Some(" two three"),
        "{:?}",
        core.copy_selection()
    );
    // In an editor that is not focused, Shift is a press.
    core.set_focus(None);
    press(&mut core, Vec2::new(end_of_one, 8.0));
    release(&mut core);
    assert!(core.copy_selection().is_none_or(|t| t.is_empty()));
}

/// A box that hears the wheel, holding a scroller that still wins inside
/// it, inside a scroller it takes the wheel from.
fn handlers(core: &mut Core) -> (Key, Key, Key) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let mut inner = Key::ROOT;
    let mut canvas = Key::ROOT;
    let outer = ui.with_keyed(
        "outer",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(100.0))
            .scroll_y(),
        |ui| {
            canvas = ui.with_keyed(
                "canvas",
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(80.0))
                    .on_scroll(Value::str("zoom")),
                |ui| {
                    inner = ui.with_keyed(
                        "inner",
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .height(Sizing::Fixed(30.0))
                            .scroll_y(),
                        |ui| {
                            ui.with(
                                NodeSpec::column()
                                    .width(Sizing::Grow(1.0))
                                    .height(Sizing::Fixed(200.0)),
                                |_| {},
                            );
                        },
                    );
                },
            );
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(200.0)),
                |_| {},
            );
        },
    );
    ui.finish();
    (outer, canvas, inner)
}

#[test]
fn an_on_scroll_node_takes_the_wheel_and_a_scroller_inside_it_still_wins() {
    let mut core = Core::new();
    let (outer, canvas, inner) = handlers(&mut core);
    // Over the canvas but under the inner scroller: an event, and the
    // outer scroller stays put.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 60.0)));
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(3.0, -12.0)));
    assert_eq!(out.len(), 1);
    let ev = &out[0];
    assert_eq!(ev.key, canvas);
    assert_eq!(
        ev.payload.get("kind").and_then(Value::as_str),
        Some("scroll")
    );
    assert_eq!(ev.payload.get("dx").and_then(Value::as_float), Some(3.0));
    assert_eq!(ev.payload.get("dy").and_then(Value::as_float), Some(-12.0));
    assert_eq!(ev.payload.get("x").and_then(Value::as_float), Some(50.0));
    assert_eq!(ev.payload.get("y").and_then(Value::as_float), Some(60.0));
    assert_eq!(
        ev.payload.get("lines"),
        Some(&Value::Null),
        "no grid, no lines"
    );
    assert_eq!(ev.payload.get("tag").and_then(Value::as_str), Some("zoom"));
    handlers(&mut core);
    assert_eq!(
        core.scroll_offset(outer).y,
        0.0,
        "the canvas took the notch"
    );
    // Over the inner scroller: it scrolls, and the canvas hears nothing.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 10.0)));
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -12.0)));
    assert!(out.is_empty());
    handlers(&mut core);
    assert_eq!(core.scroll_offset(inner).y, 12.0);
    // Below the canvas, still in the outer scroller: the outer scrolls.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 90.0)));
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -12.0)));
    assert!(out.is_empty());
    handlers(&mut core);
    assert_eq!(core.scroll_offset(outer).y, 12.0);
}

const COLS: usize = 10;
const ROWS: usize = 3;

fn screen() -> Vec<Cell> {
    let lines = ["hello", "brave", "bye"];
    let mut cells = vec![Cell::new(' ', 0xffffffff, 0); ROWS * COLS];
    for (r, line) in lines.iter().enumerate() {
        for (c, ch) in line.chars().enumerate() {
            cells[r * COLS + c] = Cell::new(ch, 0xffffffff, 0);
        }
    }
    cells
}

/// A three-row grid at `origin`, `selectable` and hearing the wheel.
fn grid(core: &mut Core, origin: u64) -> Key {
    let cells = screen();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let grid = CellGrid {
        rows: ROWS,
        cols: COLS,
        cells: &cells,
        style: TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        cursor: None,
        origin_line: origin,
    };
    let key = ui.child_key("term");
    ui.cells_keyed(
        "term",
        &grid,
        NodeSpec::column()
            .width(Sizing::Fixed(200.0))
            .height(Sizing::Fixed(3.0 * 17.0))
            .selectable()
            .on_scroll(Value::str("term")),
    );
    ui.finish();
    key
}

fn cell_h(core: &mut Core) -> f32 {
    core.measure_text(
        "M",
        &TextStyle::new(14.0).family(kui_core::FontFamily::Mono),
        None,
    )
    .height
}

#[test]
fn the_wheel_over_a_grid_is_whole_lines_with_the_fraction_carried() {
    let mut core = Core::new();
    let key = grid(&mut core, 100);
    let h = cell_h(&mut core);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 20.0)));
    // Down by two and a half lines: two lines out, a half carried.
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -2.5 * h)));
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].key, key);
    assert_eq!(out[0].payload.get("lines").and_then(Value::as_int), Some(2));
    assert_eq!(
        out[0].payload.get("tag").and_then(Value::as_str),
        Some("term")
    );
    // Another half: the carried half makes a whole line.
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -0.5 * h)));
    assert_eq!(out[0].payload.get("lines").and_then(Value::as_int), Some(1));
    // Up by a line and a bit: earlier history is negative.
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, 1.2 * h)));
    assert_eq!(
        out[0].payload.get("lines").and_then(Value::as_int),
        Some(-1)
    );
    // A notch too small for a line still arrives, with zero lines and the
    // pixels, so an app that wants the delta has it.
    let out = core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -1.0)));
    assert_eq!(out[0].payload.get("lines").and_then(Value::as_int), Some(0));
    assert_eq!(
        out[0].payload.get("dy").and_then(Value::as_float),
        Some(-1.0)
    );
}

#[test]
fn a_drag_held_past_a_grids_edge_asks_the_app_for_lines_and_the_ends_survive() {
    let mut core = Core::new();
    let key = grid(&mut core, 100);
    let h = cell_h(&mut core);
    // Press in row 0, drag 60 px below the grid: 10 px a clockless frame.
    press(&mut core, Vec2::new(4.0, 8.0));
    core.handle_input(InputEvent::CursorMoved(Vec2::new(60.0, 3.0 * 17.0 + 60.0)));
    let sel = core.cell_selection().expect("a cell selection");
    assert_eq!((sel.anchor.line, sel.focus.line), (100, 102));
    let mut lines = 0i64;
    let mut origin = 100u64;
    for _ in 0..(h as usize + 1) {
        grid(&mut core, origin);
        assert!(core.animating());
        for ev in core.take_pending_events() {
            assert_eq!(ev.key, key);
            assert_eq!(
                ev.payload.get("kind").and_then(Value::as_str),
                Some("scroll")
            );
            let n = ev.payload.get("lines").and_then(Value::as_int).unwrap();
            lines += n;
            // The app answers by moving its screen.
            origin = (origin as i64 + n) as u64;
        }
    }
    assert!(
        lines >= 1,
        "ten px a frame over {h} px rows adds up to lines: {lines}"
    );
    grid(&mut core, origin);
    grid(&mut core, origin);
    let sel = core.cell_selection().expect("still selected");
    assert_eq!(sel.anchor.line, 100, "the anchor's absolute line survived");
    assert_eq!(
        sel.focus.line,
        origin + 2,
        "the live end followed the pointer onto the last row of the moved screen"
    );
    release(&mut core);
    grid(&mut core, origin);
    assert!(!core.animating());
    assert!(core.take_pending_events().is_empty());
}

#[test]
fn a_shift_press_in_the_grid_keeps_the_anchor() {
    let mut core = Core::new();
    let _ = grid(&mut core, 100);
    press(&mut core, Vec2::new(4.0, 8.0));
    release(&mut core);
    shift(&mut core, true);
    press(&mut core, Vec2::new(20.0, 2.0 * 17.0 + 8.0));
    release(&mut core);
    let sel = core.cell_selection().expect("extended");
    assert_eq!((sel.anchor.line, sel.focus.line), (100, 102));
    assert!(
        core.cell_selection_text()
            .is_some_and(|t| t.starts_with("hello\nbrave"))
    );
}
