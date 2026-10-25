//! `on_range` (ADR 0065): the data rows in a node's view, reported after
//! the frame that first lays it out and whenever they change — what a
//! component pages an unbounded source by.

use kui_core::{Core, Edges, EventKind, NodeSpec, Size, TextStyle, Value, Vec2};

const VIEW: Size = Size { w: 200.0, h: 100.0 };

/// A 100 px scroller over 40 rows of 20 px, each at its data index.
fn frame(core: &mut Core) -> Vec<kui_core::UiEvent> {
    let mut ui = core.frame(VIEW, 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.with_keyed(
        "rows",
        NodeSpec::column()
            .size(200.0, 100.0)
            .scroll_y()
            .padding(Edges::all(0.0))
            .on_range(Value::str("rows")),
        |ui| {
            for i in 0..40u64 {
                ui.open_indexed(i, NodeSpec::row().size(200.0, 20.0));
                ui.text(&format!("row {i}"), TextStyle::new(12.0));
                ui.close();
            }
        },
    );
    ui.finish();
    core.take_pending_events()
}

fn ranges(events: &[kui_core::UiEvent]) -> Vec<(i64, i64)> {
    events
        .iter()
        .filter(|e| e.event == EventKind::Range)
        .map(|e| {
            let r = e.payload.get("range").expect("range data");
            (
                r.get("first").and_then(Value::as_int).unwrap(),
                r.get("last").and_then(Value::as_int).unwrap(),
            )
        })
        .collect()
}

#[test]
fn the_rows_in_view_are_reported_first_and_on_change_only() {
    let mut core = Core::new();
    assert_eq!(ranges(&frame(&mut core)), [(0, 4)]);
    // Nothing moved: nothing reported.
    assert_eq!(ranges(&frame(&mut core)), []);
    // Scrolled by 50 px: rows 2 (half) through 7 (half) are in view.
    let rows = core.key_of("rows").expect("the scroller");
    core.set_scroll(rows, Vec2::new(0.0, 50.0));
    let after = ranges(&frame(&mut core));
    assert_eq!(after, [(2, 7)], "{after:?}");
    let e = frame(&mut core);
    assert_eq!(ranges(&e), []);
}

#[test]
fn the_message_is_the_one_declared() {
    let mut core = Core::new();
    let events = frame(&mut core);
    let e = events
        .iter()
        .find(|e| e.event == EventKind::Range)
        .expect("a range");
    assert_eq!(e.payload.get_str("tag"), Some("rows"));
}
