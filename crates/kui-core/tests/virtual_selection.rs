//! What a selection does when the rows under it are virtualised — built
//! for one scroll position and gone at the next (ADR 0017, decision 3,
//! tier 3).
//!
//! Two halves. The *ordering* half is the core's own: an end whose row is
//! not built is placed by that row's index in the data, so the part of a
//! selection a reader can still see keeps its highlight while the list
//! scrolls under it. The *filling* half is the app's: the rows behind a
//! gap were never handed to the core, so a copy over one is asked for
//! (`selectionrange`) and answered (`answer_selection_range`) rather than
//! guessed at.

use kui_core::{Core, InputEvent, Key, MouseButton, NodeSpec, Size, Sizing, TextStyle, Vec2};

fn style() -> TextStyle {
    TextStyle::new(14.0)
}

/// A `selectable` scroller that builds rows `range`, each at its own data
/// index, with spacers standing in for the rest — a virtual list.
fn frame(core: &mut Core, range: std::ops::Range<u64>) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let scope = ui.with_keyed(
        "list",
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Fixed(100.0))
            .scroll_y()
            .selectable(),
        |ui| {
            ui.with_keyed(
                "lead",
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Fixed(range.start as f32 * 20.0)),
                |_| {},
            );
            for i in range.clone() {
                ui.with_indexed(
                    i,
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Fixed(20.0)),
                    |ui| ui.text(&format!("row {i}"), style()),
                );
            }
        },
    );
    ui.finish();
    scope
}

#[test]
fn a_selection_over_unbuilt_rows_reads_as_nothing_and_comes_back() {
    let mut core = Core::new();
    let scope = frame(&mut core, 0..5);
    let _ = scope;
    // Drag across the first rows.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(2.0, 4.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 45.0)));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2"),
        "the rows it dragged over are built, so they select and copy"
    );

    // The view scrolls: rows 10..15 are built and neither end is.
    frame(&mut core, 10..15);
    assert!(
        core.selection().is_some(),
        "the selection itself survives — its ends are addresses, not places"
    );
    assert_eq!(
        core.selection_text(),
        None,
        "the core cannot answer for rows it never built"
    );
    let tint = kui_core::select::TINT;
    let painted = core
        .output()
        .0
        .quads
        .iter()
        .filter(|q| q.color == tint)
        .count();
    assert_eq!(painted, 0, "and nothing paints, which is the visible half");

    // Scrolled back, the same rows are built again — and because a row's
    // key comes from its *data index*, they are the same nodes, so the
    // selection is the one that was made.
    frame(&mut core, 0..5);
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2"),
        "no drift: `open_indexed` gives a row the same key wherever it sits"
    );
}

/// Scrolled *partway*: the anchor's row is gone but rows the selection
/// covers are still built, so the part a reader can see is still painted.
/// This is what the row index buys — an end that is not built can still be
/// placed against the ends that are (ADR 0017, decisions 2 and 3).
#[test]
fn the_built_middle_of_a_selection_still_paints() {
    let mut core = Core::new();
    frame(&mut core, 0..6);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(2.0, 4.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    // Down through rows 0, 1, 2.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 45.0)));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    let tint = kui_core::select::TINT;
    let painted = |core: &mut Core| {
        core.output()
            .0
            .quads
            .iter()
            .filter(|q| q.color == tint)
            .count()
    };
    frame(&mut core, 0..6);
    assert_eq!(painted(&mut core), 3, "three rows selected, three runs");

    // The list scrolls: row 0 is no longer built, rows 1 and 2 are.
    frame(&mut core, 1..7);
    assert_eq!(
        painted(&mut core),
        2,
        "the two rows still on screen keep their highlight"
    );

    // Past the selection entirely: nothing of it is built, nothing paints.
    frame(&mut core, 10..16);
    assert_eq!(painted(&mut core), 0);
}

/// The filling half: a copy that reaches unbuilt rows asks the app, and
/// the app's answer is what reaches the clipboard.
#[test]
fn a_copy_over_unbuilt_rows_asks_the_app_and_takes_its_answer() {
    use kui_core::{CopyRequest, MenuAction, Value};

    let mut core = Core::new();
    frame(&mut core, 0..6);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(2.0, 4.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 45.0)));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    // Everything selected is built, so the core answers it itself.
    assert_eq!(
        core.request_copy(),
        CopyRequest::Ready("row 0\nrow 1\nrow 2".into())
    );

    // Scroll so the anchor's row is gone.
    frame(&mut core, 2..8);
    assert_eq!(core.request_copy(), CopyRequest::Asked);
    let asked = core.take_pending_events();
    let ev = asked
        .iter()
        .find(|e| e.payload.get("kind").and_then(Value::as_str) == Some("selectionrange"))
        .expect("the app is asked, on the scope");
    let end = |name: &str| {
        let e = ev.payload.get(name).expect("an end");
        (
            e.get("index").and_then(Value::as_int),
            e.get("byte").and_then(Value::as_int),
        )
    };
    assert_eq!(end("from"), (Some(0), Some(0)), "row 0, byte 0");
    assert_eq!(end("to").0, Some(2), "row 2");

    // The app knows its own data; its answer is what gets copied.
    assert!(core.answer_selection_range("row 0\nrow 1\nrow 2"));
    assert_eq!(
        core.take_menu_actions(),
        vec![MenuAction::SetClipboard {
            text: "row 0\nrow 1\nrow 2".into(),
            html: None,
        }]
    );
    // And an answer nobody asked for changes nothing: a late reply cannot
    // overwrite whatever has been copied since.
    assert!(!core.answer_selection_range("stale"));
    assert!(core.take_menu_actions().is_empty());
}

/// A scope that holds a header beside its virtual rows: an end *below* the
/// built range must be placed below it. Comparing against the last built
/// run rather than the last built *row* put it at the top instead, and the
/// wrong half of the list highlighted.
#[test]
fn an_end_below_a_mixed_scope_is_placed_below_it() {
    let mut core = Core::new();
    let build = |core: &mut Core, range: std::ops::Range<u64>| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.configure_root(NodeSpec::column().fill());
        let scope = ui.with_keyed(
            "list",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(200.0))
                .scroll_y()
                .selectable(),
            |ui| {
                // A header, which is not a row and knows no index.
                ui.text("HEADER", style());
                for i in range.clone() {
                    ui.with_indexed(
                        i,
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .height(Sizing::Fixed(20.0)),
                        |ui| ui.text(&format!("row {i}"), style()),
                    );
                }
            },
        );
        ui.finish();
        scope
    };
    build(&mut core, 0..4);
    // Select from the header down into row 3.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(2.0, 4.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 85.0)));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    let all = core.selection_text().expect("a selection");
    assert!(all.starts_with("HEADER"), "{all:?}");

    // Now only rows 0..2 are built: the far end (row 3) is above nothing
    // and below everything, so the whole built scope stays highlighted.
    build(&mut core, 0..2);
    let tint = kui_core::select::TINT;
    let painted = core
        .output()
        .0
        .quads
        .iter()
        .filter(|q| q.color == tint)
        .count();
    assert_eq!(
        painted, 3,
        "the header and both built rows, not a single run at the top"
    );
}

/// A drag made backwards — pressed on a later row, released on an earlier
/// one — asks for the same range a forwards one does: `from` precedes `to`
/// in the ask, whichever end the press was, whether both rows are built,
/// neither is, or only one (backlog C39's finding).
#[test]
fn a_backwards_drag_asks_for_its_range_in_reading_order() {
    use kui_core::{CopyRequest, Value};
    let ask = |core: &mut Core| -> (i64, i64, i64, i64) {
        assert_eq!(core.request_copy(), CopyRequest::Asked);
        let ev = core
            .take_pending_events()
            .into_iter()
            .find(|e| e.payload.get("kind").and_then(Value::as_str) == Some("selectionrange"))
            .expect("a selectionrange ask");
        let end = |name: &str| {
            let e = ev.payload.get(name).unwrap();
            (
                e.get("index").and_then(Value::as_int).unwrap(),
                e.get("byte").and_then(Value::as_int).unwrap(),
            )
        };
        let (from, to) = (end("from"), end("to"));
        (from.0, from.1, to.0, to.1)
    };
    let mut core = Core::new();
    frame(&mut core, 0..5);
    // Press three bytes into row 2, release at the start of row 0.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 45.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(2.0, 4.0)));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    assert_eq!(
        core.selection_text().as_deref(),
        Some("row 0\nrow 1\nrow 2")
    );
    let (a, f) = core.selection_ends().unwrap();
    assert_eq!((a.row, f.row), (Some(2), Some(0)), "the ends are directed");

    // (Both rows built is never an ask: the core answers itself.) The
    // earlier row gone, the later still built: the unbuilt end is placed
    // before every built row.
    frame(&mut core, 1..6);
    let (from_i, from_b, to_i, to_b) = ask(&mut core);
    assert_eq!((from_i, from_b), (0, 0), "from is the earlier row");
    assert_eq!((to_i, to_b), (2, 5), "to is the later, at its byte");

    // Neither row built: ordered by row index.
    frame(&mut core, 10..15);
    assert_eq!(ask(&mut core), (0, 0, 2, 5));

    // The later row gone, the earlier still built: the unbuilt end is
    // placed after every built row.
    frame(&mut core, 0..2);
    assert_eq!(ask(&mut core), (0, 0, 2, 5));

    // And the mirror: a forwards drag asks for the same range.
    frame(&mut core, 0..5);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(2.0, 4.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(200.0, 45.0)));
    core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    frame(&mut core, 10..15);
    assert_eq!(ask(&mut core), (0, 0, 2, 5));
}
