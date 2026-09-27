//! `reveal` and `set_scroll` by a label no frame has declared yet
//! (backlog DX15): resolved when the frame finishes. kawoosh's Lua panes
//! called `env.reveal("row")` on a pane's first frame, got an error, and
//! wrapped every call in `pcall` and a retry over frames.

use kui_core::{Core, NodeSpec, Size, Vec2};

const VIEW: Size = Size { w: 300.0, h: 100.0 };

/// A 100 px list of 50 rows of 20 px, `row{i}` each.
fn list(ui: &mut kui_core::Ui<'_>) {
    ui.with_keyed(
        "rows",
        NodeSpec::column().size(300.0, 100.0).scroll_y(),
        |ui| {
            for i in 0..50 {
                ui.leaf_keyed(&format!("row{i}"), NodeSpec::column().size(300.0, 20.0));
            }
        },
    );
}

#[test]
fn a_label_named_before_its_node_resolves_when_the_frame_finishes() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    // First frame, before the list is declared: neither name exists yet,
    // in this frame or any before it.
    ui.reveal_label("row30");
    list(&mut ui);
    ui.finish();
    let rows = core.key_of("rows").unwrap();
    let y = core.scroll_offset(rows).y;
    assert!((520.0..=600.0).contains(&y), "row 30 (600..620) shows: {y}");
    assert!(core.take_warnings().is_empty());

    let mut ui = core.frame(VIEW, 1.0);
    ui.set_scroll_label("rows", Vec2::new(0.0, 40.0));
    list(&mut ui);
    ui.finish();
    assert_eq!(
        core.scroll_offset(rows),
        Vec2::new(0.0, 40.0),
        "this frame's layout"
    );
}

#[test]
fn a_label_its_frame_never_declares_is_a_warning_and_moves_nothing() {
    let mut core = Core::new();
    let mut ui = core.frame(VIEW, 1.0);
    ui.reveal_label("row99");
    list(&mut ui);
    ui.finish();
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(codes, ["label-without-node"]);
    let rows = core.key_of("rows").unwrap();
    assert_eq!(core.scroll_offset(rows), Vec2::ZERO);
}
