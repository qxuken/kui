//! Where a frame drew (backlog F235): `DisplayList::bounds`, the union of
//! the quads' footprints in physical px. From Replicator Bloom, whose
//! full-window transparent layer draws two small tags and paid for a
//! 2560×1600 clear, draw and composite every frame.

use kui_core::{Align, Color, Core, FloatConfig, NodeSpec, Rect, Shadow, Size};

const VIEW: Size = Size { w: 400.0, h: 300.0 };
const INK: Color = Color {
    r: 0.9,
    g: 0.9,
    b: 0.9,
    a: 1.0,
};

/// A `w`×`h` box at (`x`, `y`) on the transparent root.
fn tag(x: f32, y: f32, w: f32, h: f32) -> NodeSpec {
    NodeSpec::column().size(w, h).bg(INK).float(
        FloatConfig::parent()
            .inside(Align::Start, Align::Start)
            .offset(x, y),
    )
}

fn bounds(core: &mut Core, scale: f32, f: impl FnOnce(&mut kui_core::Ui<'_>)) -> Option<Rect> {
    let mut ui = core.frame(VIEW, scale);
    ui.configure_root(NodeSpec::column().fill());
    f(&mut ui);
    ui.finish();
    let (dl, _) = core.output();
    dl.bounds()
}

#[test]
fn a_frame_that_draws_nothing_has_no_bounds() {
    let mut core = Core::new();
    assert_eq!(bounds(&mut core, 1.0, |_| {}), None);
}

#[test]
fn two_tags_are_bounded_by_both_and_nothing_else_in_physical_px() {
    let mut core = Core::new();
    let b = bounds(&mut core, 2.0, |ui| {
        ui.leaf_keyed("gate", tag(20.0, 30.0, 40.0, 10.0));
        ui.leaf_keyed("planet", tag(300.0, 200.0, 50.0, 20.0));
    });
    assert_eq!(b, Some(Rect::new(40.0, 60.0, 660.0, 380.0)));
}

#[test]
fn a_shadow_reaches_past_its_box_and_a_clip_cuts_what_it_holds() {
    let mut core = Core::new();
    let b = bounds(&mut core, 1.0, |ui| {
        ui.leaf_keyed(
            "card",
            tag(100.0, 100.0, 50.0, 50.0).shadow(Shadow {
                color: Color {
                    r: 0.0,
                    g: 0.0,
                    b: 0.0,
                    a: 0.5,
                },
                dx: 0.0,
                dy: 0.0,
                blur: 10.0,
                spread: 0.0,
            }),
        );
    })
    .expect("drawn");
    assert!(b.x < 100.0 && b.y < 100.0, "{b:?}");
    assert!(b.x + b.w > 150.0 && b.y + b.h > 150.0, "{b:?}");

    let b = bounds(&mut core, 1.0, |ui| {
        let strip = NodeSpec::column().size(50.0, 50.0).clip().float(
            FloatConfig::parent()
                .inside(Align::Start, Align::Start)
                .offset(10.0, 10.0),
        );
        ui.with_keyed("strip", strip, |ui| {
            ui.leaf_keyed("wide", NodeSpec::column().size(300.0, 50.0).bg(INK));
        });
    });
    assert_eq!(
        b,
        Some(Rect::new(10.0, 10.0, 50.0, 50.0)),
        "the strip's clip"
    );
}

#[test]
fn a_turned_box_is_bounded_where_it_is_drawn() {
    let mut core = Core::new();
    // A 100×10 bar a quarter turn about its centre at (150, 105): drawn
    // 10 wide and 100 tall.
    let b = bounds(&mut core, 1.0, |ui| {
        ui.leaf_keyed("bar", tag(100.0, 100.0, 100.0, 10.0).rotate(0.25));
    })
    .expect("drawn");
    assert!(
        (b.w - 10.0).abs() <= 2.0 && (b.h - 100.0).abs() <= 2.0,
        "{b:?}"
    );
    assert!(
        (b.x - 145.0).abs() <= 1.0 && (b.y - 55.0).abs() <= 1.0,
        "{b:?}"
    );
}
