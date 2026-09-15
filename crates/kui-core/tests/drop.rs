//! A drop zone through a live `Core`
//! (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`):
//! `drop_bg` resolved when the node opens, winning over `hover_bg`, and a
//! zone the view stops declaring mid-drag hearing its prepared `leave`.
//! The resolver's cases are the `drop` corpus scene's.

use kui_core::{Color, Core, InputEvent, Key, NodeSpec, Size, Sizing, Value, Vec2};

const BASE: Color = Color {
    r: 0.1,
    g: 0.1,
    b: 0.1,
    a: 1.0,
};
const HOVER: Color = Color {
    r: 0.2,
    g: 0.2,
    b: 0.2,
    a: 1.0,
};
const LIT: Color = Color {
    r: 0.3,
    g: 0.3,
    b: 0.6,
    a: 1.0,
};

/// A 100×100 zone at the origin, declared while `declare` is true.
fn frame(core: &mut Core, declare: bool) -> Key {
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let mut zone = Key(0);
    ui.with(NodeSpec::column(), |ui| {
        zone = ui.child_key("zone");
        if declare {
            ui.with_keyed(
                "zone",
                NodeSpec::column()
                    .width(Sizing::Fixed(100.0))
                    .height(Sizing::Fixed(100.0))
                    .bg(BASE)
                    .hover_bg(HOVER)
                    .drop_bg(LIT)
                    .on_drop(Value::str("files")),
                |_| {},
            );
        }
    });
    ui.finish();
    zone
}

fn zone_bg(core: &mut Core) -> Color {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| q.rect.w == 100.0)
        .map(|q| q.color)
        .expect("zone quad")
}

fn files() -> Vec<String> {
    vec!["/drop/1.txt".to_string()]
}

#[test]
fn drop_bg_lights_the_zone_while_files_hover_and_wins_over_hover() {
    let mut core = Core::new();
    let zone = frame(&mut core, true);
    assert_eq!(zone_bg(&mut core), BASE);

    // The pointer hovered the zone before the drag began: hover_bg.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)));
    frame(&mut core, true);
    assert_eq!(zone_bg(&mut core), HOVER);

    let evs = core.handle_input(InputEvent::DragFiles {
        paths: files(),
        at: Vec2::new(50.0, 50.0),
    });
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].payload.get("phase").unwrap().as_str(), Some("enter"));
    assert_eq!(
        evs[0].payload.get("paths").unwrap().as_list().unwrap()[0].as_str(),
        Some("/drop/1.txt")
    );
    assert_eq!(core.drop_target(), Some(zone));
    frame(&mut core, true);
    assert_eq!(zone_bg(&mut core), LIT, "drop_bg wins over hover_bg");
    assert!(core.is_drop_target(zone));

    // Landed: lit no more, and the stale hover is back — the pointer
    // never left it as far as the core was told.
    let evs = core.handle_input(InputEvent::DropFiles {
        paths: files(),
        at: Vec2::new(50.0, 50.0),
    });
    assert_eq!(evs.len(), 1, "a drop and no leave after it");
    assert_eq!(evs[0].payload.get("phase").unwrap().as_str(), Some("drop"));
    assert_eq!(core.drop_target(), None);
    frame(&mut core, true);
    assert_eq!(zone_bg(&mut core), HOVER);
}

#[test]
fn a_zone_the_view_stops_declaring_hears_its_prepared_leave() {
    let mut core = Core::new();
    let zone = frame(&mut core, true);
    let evs = core.handle_input(InputEvent::DragFiles {
        paths: files(),
        at: Vec2::new(50.0, 50.0),
    });
    assert_eq!(evs[0].key, zone);
    // The view answers `enter` by removing the zone. Nothing is
    // re-resolved by the frame (decision 2): the zone stays lit until
    // the driver's next report, which finds no zone under the point.
    frame(&mut core, false);
    assert_eq!(core.drop_target(), Some(zone));
    let evs = core.handle_input(InputEvent::DragFiles {
        paths: files(),
        at: Vec2::new(60.0, 60.0),
    });
    assert_eq!(evs.len(), 1);
    assert_eq!(evs[0].key, zone);
    assert_eq!(evs[0].payload.get("phase").unwrap().as_str(), Some("leave"));
    assert_eq!(evs[0].payload.get("tag").unwrap().as_str(), Some("files"));
    assert_eq!(core.drop_target(), None);
    // And a cancel with nothing lit is nothing.
    assert!(core.handle_input(InputEvent::DragCancel).is_empty());
}
