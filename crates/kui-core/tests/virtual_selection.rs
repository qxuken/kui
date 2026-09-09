//! What a selection does when the rows under it are virtualised — built
//! for one scroll position and gone at the next (ADR 0017, decision 3,
//! tier 3).
//!
//! **This pins a gap, not a feature.** Tier 3 — the core reporting its
//! ends and the app filling in the rows it never built — is described in
//! the ADR and was never staged, so today a selection whose ends are not
//! built reads as nothing at all: no highlight, no copy. It is at least
//! not *wrong*: the ends are addresses, the state survives, and scrolling
//! the rows back brings the selection back intact. When tier 3 lands,
//! these assertions are the ones that change.

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
        "but nothing resolves, so nothing copies: tier 3 is what fills this in"
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
