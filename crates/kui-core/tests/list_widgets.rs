//! DX12's widgets: a divider that drags, a virtual list's row revealed
//! before it is built, and a row spec on `uniform_list`. kawoosh wrote
//! the divider three times and the reveal three times, and nested a
//! second node in every row of three lists to hang a click on.

use kui_core::testing::{click, hover, press, release};
use kui_core::{Core, Dir, NodeSpec, Size, Vec2, widgets};

const VIEW: Size = Size { w: 300.0, h: 100.0 };

#[test]
fn a_splitter_drags_to_its_parent_s_ratio_and_keeps_the_keyboard_where_it_was() {
    let frame = |core: &mut Core| {
        let mut ui = core.frame(VIEW, 1.0);
        ui.with(NodeSpec::row().size(300.0, 100.0), |ui| {
            ui.leaf_keyed("left", NodeSpec::column().fill().on_key("keys"));
            widgets::splitter(ui, "bar", Dir::Row, 4.0, "split");
            ui.leaf(NodeSpec::column().fill());
        });
        ui.finish();
    };
    let mut core = Core::new();
    frame(&mut core);
    let left = core.key_of("left");
    core.set_key_focus(left);
    frame(&mut core);

    let mut evs = press(&mut core, Vec2::new(150.0, 50.0));
    evs.extend(hover(&mut core, Vec2::new(225.0, 50.0)));
    evs.extend(release(&mut core));
    let ratio = evs.iter().rev().find_map(|e| e.drag()).unwrap().ratio();
    assert_eq!(ratio.x, 0.75);
    assert_eq!(core.key_focus(), left, "the editor keeps the keyboard");
}

/// A 100 px list of 50 rows of 20 px, each row clickable by its spec.
fn list(ui: &mut kui_core::Ui<'_>, reveal: Option<usize>) -> Option<bool> {
    let scrolled = reveal.map(|i| widgets::reveal_row(ui, "rows", i, 20.0));
    widgets::uniform_list_with(
        ui,
        "rows",
        NodeSpec::column().size(300.0, 100.0),
        50,
        20.0,
        |i| NodeSpec::row().on_click(i),
        |_, _| {},
    );
    scrolled
}

#[test]
fn reveal_row_centres_a_row_the_list_has_not_built_and_leaves_a_shown_one() {
    let mut core = Core::new();
    let run = |core: &mut Core, reveal| {
        let mut ui = core.frame(VIEW, 1.0);
        let r = list(&mut ui, reveal);
        ui.finish();
        r
    };
    assert_eq!(run(&mut core, Some(40)), Some(false), "no geometry yet");
    run(&mut core, None);
    let rows = core.key_of("rows").unwrap();
    assert_eq!(run(&mut core, Some(40)), Some(true));
    // Row 40 is 800..820; the middle of a 100 px list puts it at 760.
    assert_eq!(core.scroll_offset(rows), Vec2::new(0.0, 760.0));
    assert_eq!(run(&mut core, Some(41)), Some(false), "already shown");

    // And the rows were built for the offset the same frame scrolled to:
    // the row under the pointer is 40, clickable through its own spec.
    let evs = click(&mut core, Vec2::new(10.0, 45.0));
    assert_eq!(evs.iter().find_map(|e| e.payload.as_int()), Some(40));
    let mut ui = core.frame(VIEW, 1.0);
    assert_eq!(widgets::rows_in_view(&mut ui, "rows", 20.0), 5);
    ui.finish();
}
