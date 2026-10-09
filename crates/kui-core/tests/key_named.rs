//! A lookup by accessible name (backlog F137): `Core::key_named` finds a
//! node by what a reader hears it called — its `label` row, else the text
//! inside a control — where `key_of` reads the key label the view opened
//! it under, which a reader never hears.

use kui_core::{Core, NodeSpec, Size, TextStyle};

fn deck(core: &mut Core, two_deletes: bool) {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.with(NodeSpec::row(), |ui| {
        ui.leaf_keyed(
            "like",
            NodeSpec::column()
                .size(40.0, 40.0)
                .on_click("like")
                .label("Like"),
        );
        ui.with_keyed("nope", NodeSpec::row().on_click("nope"), |ui| {
            ui.text("Nope", TextStyle::new(14.0));
        });
        let deletes = if two_deletes { 2 } else { 1 };
        for i in 0..deletes {
            ui.with_keyed(
                &format!("delete-{i}"),
                NodeSpec::row().on_click("delete"),
                |ui| ui.text("Delete", TextStyle::new(14.0)),
            );
        }
    });
    ui.finish();
}

#[test]
fn a_label_row_names_a_node_and_the_key_label_does_not() {
    let mut core = Core::new();
    deck(&mut core, false);
    let like = core.key_of("like").expect("keyed like");
    assert_eq!(core.key_named("Like"), Some(like));
    assert_eq!(
        core.key_named("like"),
        None,
        "the key label is not the name"
    );
    assert_eq!(core.key_of("Like"), None, "nor the name the key label");
}

#[test]
fn a_button_is_named_by_the_text_inside_it() {
    let mut core = Core::new();
    deck(&mut core, false);
    assert_eq!(core.key_named("Nope"), core.key_of("nope"));
    assert!(core.take_warnings().is_empty());
}

#[test]
fn two_nodes_with_one_name_resolve_to_the_first_and_warn() {
    let mut core = Core::new();
    deck(&mut core, true);
    assert_eq!(core.key_named("Delete"), core.key_of("delete-0"));
    let codes: Vec<_> = core.take_warnings().iter().map(|w| w.code).collect();
    assert_eq!(codes, vec!["ambiguous-name"]);
}

#[test]
fn no_node_with_the_name_is_none() {
    let mut core = Core::new();
    assert_eq!(core.key_named("Like"), None, "before any frame");
    deck(&mut core, false);
    assert_eq!(core.key_named("Superlike"), None);
}
