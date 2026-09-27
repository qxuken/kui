//! A scroller takes the axes it scrolls on and passes the rest to the one
//! under it (backlog DX13). kawoosh kept its own horizontal offset and set
//! it every frame, because a vertical list inside a horizontal strip ate
//! the sideways swipe.

use kui_core::{Core, InputEvent, NodeSpec, Size, Vec2};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn frame(core: &mut Core) {
    let mut ui = core.frame(VIEW, 1.0);
    // A strip scrolling x, wider inside than its 300 px; in it a column
    // scrolling y, taller inside than its 200 px.
    ui.with_keyed(
        "strip",
        NodeSpec::row().size(300.0, 200.0).scroll_x(),
        |ui| {
            ui.with_keyed(
                "list",
                NodeSpec::column().size(200.0, 200.0).scroll_y(),
                |ui| {
                    ui.leaf(NodeSpec::column().size(200.0, 800.0));
                },
            );
            ui.leaf(NodeSpec::column().size(600.0, 200.0));
        },
    );
    ui.finish();
}

#[test]
fn a_sideways_notch_over_a_vertical_list_moves_the_strip_around_it() {
    let mut core = Core::new();
    frame(&mut core);
    frame(&mut core);
    let (strip, list) = (core.key_of("strip").unwrap(), core.key_of("list").unwrap());
    core.handle_input(InputEvent::CursorMoved(Vec2::new(50.0, 50.0)));

    core.handle_input(InputEvent::Scroll(Vec2::new(0.0, -40.0)));
    frame(&mut core);
    assert_eq!(
        core.scroll_offset(list),
        Vec2::new(0.0, 40.0),
        "y: the list's"
    );
    assert_eq!(core.scroll_offset(strip), Vec2::ZERO);

    core.handle_input(InputEvent::Scroll(Vec2::new(-30.0, 0.0)));
    frame(&mut core);
    assert_eq!(
        core.scroll_offset(strip),
        Vec2::new(30.0, 0.0),
        "x: passed up"
    );
    assert_eq!(core.scroll_offset(list), Vec2::new(0.0, 40.0));

    // A diagonal notch splits: each takes its axis.
    core.handle_input(InputEvent::Scroll(Vec2::new(-10.0, -10.0)));
    frame(&mut core);
    assert_eq!(core.scroll_offset(strip), Vec2::new(40.0, 0.0));
    assert_eq!(core.scroll_offset(list), Vec2::new(0.0, 50.0));
}
