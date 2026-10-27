//! A control inside a `live` region is unreachable: the region reads as
//! one message, its subtree presentational. `control-inside-live` says so.

use kui_core::access::Live;
use kui_core::testing::codes;
use kui_core::{Core, NodeSpec, Size, TextStyle, Value};

fn frame(core: &mut Core, live_on_row: bool) -> Vec<&'static str> {
    let mut ui = core.frame(Size::new(400.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    let row = NodeSpec::row().gap(8.0);
    let row = if live_on_row {
        row.live(Live::Polite)
    } else {
        row
    };
    ui.with(row, |ui| {
        let message = NodeSpec::column();
        let message = if live_on_row {
            message
        } else {
            message.live(Live::Polite)
        };
        ui.with(message, |ui| {
            ui.text("The port is busy.", TextStyle::new(12.0))
        });
        ui.leaf(
            NodeSpec::row()
                .size(60.0, 20.0)
                .on_click(Value::str("retry"))
                .label("Retry"),
        );
    });
    ui.finish();
    codes(&core.take_warnings()).into_iter().collect()
}

#[test]
fn a_button_inside_a_live_region_is_reported() {
    let mut core = Core::new();
    assert_eq!(
        frame(&mut core, true),
        [kui_core::diag::CONTROL_INSIDE_LIVE]
    );
}

#[test]
fn live_on_the_message_alone_leaves_the_button_reachable() {
    let mut core = Core::new();
    assert!(frame(&mut core, false).is_empty());
    let names: Vec<_> = core
        .access_tree()
        .nodes
        .iter()
        .filter_map(|n| n.name.clone())
        .collect();
    assert!(names.contains(&"Retry".to_string()), "{names:?}");
}
