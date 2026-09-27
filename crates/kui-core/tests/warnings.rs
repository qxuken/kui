//! Diagnostics as data: the misconfigurations that fail silently become
//! warnings a driver drains — each once — and the stock widgets raise none.

use kui_core::diag::{DUPLICATE_KEY, GROW_WEIGHT_IGNORED, TRANSITION_AUTO_KEY};
use kui_core::testing::codes;
use kui_core::{Core, Key, NodeSpec, Size, Sizing, Value, widgets};

/// A row root with the given children specs (auto-keyed).
fn row_of(core: &mut Core, children: &[NodeSpec]) -> Vec<Key> {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::row().fill());
    let keys = children.iter().map(|c| ui.leaf(c.clone())).collect();
    ui.finish();
    keys
}

#[test]
fn a_lone_weighted_grow_child_warns_once() {
    let mut core = Core::new();
    let keys = row_of(
        &mut core,
        &[
            NodeSpec::column().width(Sizing::Fixed(50.0)),
            NodeSpec::column().width(Sizing::Grow(2.0)),
        ],
    );
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [GROW_WEIGHT_IGNORED]);
    assert_eq!(ws[0].key, keys[1]);
    assert!(
        ws[0].message.contains("only grow child"),
        "{}",
        ws[0].message
    );
    // The same frame again says nothing new.
    row_of(
        &mut core,
        &[
            NodeSpec::column().width(Sizing::Fixed(50.0)),
            NodeSpec::column().width(Sizing::Grow(2.0)),
        ],
    );
    assert!(core.take_warnings().is_empty());
}

#[test]
fn weights_that_split_something_do_not_warn() {
    let mut core = Core::new();
    row_of(&mut core, &[NodeSpec::column().width(Sizing::Grow(1.0))]);
    row_of(
        &mut core,
        &[
            NodeSpec::column().width(Sizing::Grow(2.0)),
            NodeSpec::column().width(Sizing::Grow(1.0)),
        ],
    );
    assert!(core.take_warnings().is_empty());
}

#[test]
fn a_cross_axis_weight_warns() {
    let mut core = Core::new();
    // A column's children grow *across* it in width: the weight is moot.
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let key = ui.leaf(
        NodeSpec::row()
            .width(Sizing::Grow(3.0))
            .height(Sizing::Fixed(10.0)),
    );
    ui.finish();
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [GROW_WEIGHT_IGNORED]);
    assert_eq!(ws[0].key, key);
    assert!(ws[0].message.contains("width grow 3"), "{}", ws[0].message);
}

/// `n` auto-keyed children carrying a transition under a keyed parent.
fn list(core: &mut Core, n: usize, keyed: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.with_keyed("list", NodeSpec::column().fill(), |ui| {
        for i in 0..n {
            let spec = NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(20.0))
                .transition(100.0);
            if keyed {
                ui.leaf_keyed(&format!("item-{i}"), spec);
            } else {
                ui.leaf(spec);
            }
        }
    });
    let key = ui.child_key("list");
    ui.finish();
    key
}

#[test]
fn a_changing_child_count_under_auto_keyed_transitions_warns_on_the_parent() {
    let mut core = Core::new();
    let parent = list(&mut core, 3, false);
    assert!(core.take_warnings().is_empty(), "a stable list is fine");
    list(&mut core, 4, false);
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [TRANSITION_AUTO_KEY]);
    assert_eq!(ws[0].key, parent);
    assert!(ws[0].message.contains("3 to 4"), "{}", ws[0].message);
    // Once per parent.
    list(&mut core, 2, false);
    assert!(core.take_warnings().is_empty());
}

#[test]
fn keyed_list_items_never_warn() {
    let mut core = Core::new();
    list(&mut core, 3, true);
    list(&mut core, 4, true);
    list(&mut core, 1, true);
    assert!(core.take_warnings().is_empty());
}

#[test]
fn two_nodes_on_one_key_warn() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let key = ui.child_key("same");
    ui.leaf_keyed("same", NodeSpec::column());
    ui.leaf_keyed("same", NodeSpec::column());
    ui.finish();
    let ws = core.take_warnings();
    assert_eq!(codes(&ws), [DUPLICATE_KEY]);
    assert_eq!(ws[0].key, key);
}

#[test]
fn diagnostics_can_be_switched_off() {
    let mut core = Core::new();
    core.set_diagnostics(false);
    row_of(&mut core, &[NodeSpec::column().width(Sizing::Grow(2.0))]);
    assert!(core.take_warnings().is_empty());
    assert!(!core.diagnostics());
}

/// Every stock widget in one frame, each under its own parent (two
/// titlebars as siblings would share a key for real): none trips a check.
#[test]
fn the_stock_widgets_raise_no_warnings() {
    let mut core = Core::new();
    for _ in 0..3 {
        let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
        ui.with(NodeSpec::column(), |ui| widgets::titlebar(ui, "title"));
        ui.with(NodeSpec::column(), |ui| {
            widgets::titlebar_with(ui, |ui| {
                ui.text("custom", kui_core::TextStyle::new(13.0));
                widgets::window_buttons(ui);
            });
        });
        widgets::button(&mut ui, "ok", Value::str("ok"));
        widgets::text_input(&mut ui, "name", "");
        ui.with(NodeSpec::row().hoverable().pad(4.0), |ui| {
            widgets::tooltip(ui, "hint");
        });
        ui.with(NodeSpec::column(), widgets::latency_graph);
        ui.with(NodeSpec::column(), widgets::latency_hud);
        ui.finish();
    }
    let ws = core.take_warnings();
    assert!(ws.is_empty(), "{ws:#?}");
}

/// The check that just failed the first draft of the test above, kept as
/// the contract: two stock widgets with the same label under one parent
/// are a real duplicate and say so.
#[test]
fn two_titlebars_under_one_parent_are_a_real_duplicate() {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    widgets::titlebar(&mut ui, "one");
    widgets::titlebar(&mut ui, "two");
    ui.finish();
    assert!(codes(&core.take_warnings()).contains(&DUPLICATE_KEY));
}
