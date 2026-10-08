//! The half of the turn contract shader validation cannot see (ADR 0043):
//! the core puts a node's turn and its inner clip on the clip entry its
//! quads name, and `vs_main` turns the corners through it while `shade`
//! tests the inner clip against the position before the turn. Both
//! compile and both draw whatever they were told; only evaluating one over
//! the other's output shows they agree — that a tilted card's corner lands
//! where the core says it does, and that a child inside a turned, rounded,
//! clipping card is cut by the card's corners in the card's own space and
//! not by an upright copy of them.

use kui_core::{Clip, Color, Core, NodeSpec, Quad, Size};

mod wgsl;
use wgsl::{inside, inside_inner, turned};

/// The last frame's solid quads with the clip entries they name.
fn solids(build: impl FnOnce(&mut kui_core::Ui<'_>), scale: f32) -> Vec<(Quad, Clip)> {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 200.0), scale);
    ui.configure_root(NodeSpec::column().pad(20.0));
    build(&mut ui);
    ui.finish();
    let dl = core.output().0;
    dl.quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid)
        .map(|q| (*q, dl.clip_of(q)))
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

#[test]
fn the_corners_land_where_the_core_says_at_any_scale() {
    for scale in [1.0, 2.0] {
        let qs = solids(
            |ui| {
                ui.leaf_keyed(
                    "card",
                    NodeSpec::column()
                        .size(100.0, 50.0)
                        .bg(Color::WHITE)
                        .rotate(0.25),
                );
            },
            scale,
        );
        let (q, clip) = qs[0];
        // A quarter turn clockwise about the centre, (70, 45) logical:
        // the top-left corner goes to the top-right of the drawn box.
        let (x, y) = turned(&clip, q.rect.x, q.rect.y);
        assert!(
            near(x, 95.0 * scale) && near(y, -5.0 * scale),
            "{x} {y} at {scale}"
        );
        // The centre stays.
        let (cx, cy) = turned(&clip, q.rect.x + q.rect.w / 2.0, q.rect.y + q.rect.h / 2.0);
        assert!(
            near(cx, 70.0 * scale) && near(cy, 45.0 * scale),
            "{cx} {cy}"
        );
        // An entry that turns nothing is the identity: `turned` is the
        // point itself.
        let (ix, iy) = turned(&Clip::NONE, 12.0, 34.0);
        assert!(near(ix, 12.0) && near(iy, 34.0));
    }
}

#[test]
fn a_child_of_a_turned_rounded_card_is_cut_by_the_cards_corners_in_its_own_space() {
    let qs = solids(
        |ui| {
            ui.with_keyed(
                "card",
                NodeSpec::column()
                    .size(100.0, 100.0)
                    .radius(16.0)
                    .clip()
                    .bg(Color::WHITE)
                    .rotate(0.1),
                |ui| {
                    ui.leaf_keyed(
                        "child",
                        NodeSpec::column().fill().bg(Color::rgb8(20, 20, 30)),
                    );
                },
            );
        },
        1.0,
    );
    let (_, child_clip) = qs[1];
    assert!(child_clip.turned());
    assert_eq!(child_clip.inner_radius, [16.0; 4]);
    // In the child's own space the card is at (20, 20), 100 square: the
    // middle is inside, the middle of an edge is inside, the corner's
    // dead zone is cut.
    assert_eq!(inside_inner(&child_clip, 70.0, 70.0), 1.0);
    assert_eq!(inside_inner(&child_clip, 70.0, 21.0), 1.0, "mid top edge");
    assert_eq!(inside_inner(&child_clip, 21.0, 21.0), 0.0, "the corner");
    assert!(
        inside_inner(&child_clip, 24.5, 24.5) > 0.1,
        "just inside the arc"
    );
    // The outer clip clips nothing here, wherever the turn put the pixel.
    let (x, y) = turned(&child_clip, 21.0, 21.0);
    assert_eq!(inside(&child_clip, x, y), 1.0);
}

#[test]
fn a_clipper_outside_the_turn_cuts_in_framebuffer_space() {
    let qs = solids(
        |ui| {
            ui.with_keyed(
                "scroller",
                NodeSpec::column().size(100.0, 100.0).clip(),
                |ui| {
                    ui.leaf_keyed(
                        "card",
                        NodeSpec::column()
                            .size(100.0, 50.0)
                            .bg(Color::WHITE)
                            .rotate(0.25),
                    );
                },
            );
        },
        1.0,
    );
    let (q, clip) = qs[0];
    // The scroller's box, upright, is the outer clip; nothing is inner.
    assert_eq!(clip.rect, kui_core::Rect::new(20.0, 20.0, 100.0, 100.0));
    assert_eq!(clip.inner, kui_core::NO_CLIP);
    // The card's top-left corner is turned to (95, -5), above the
    // scroller: cut. Its centre stays at (70, 45): shown.
    let (x, y) = turned(&clip, q.rect.x, q.rect.y);
    assert_eq!(inside(&clip, x, y), 0.0);
    let (cx, cy) = turned(&clip, 70.0, 45.0);
    assert_eq!(inside(&clip, cx, cy), 1.0);
    assert_eq!(
        inside_inner(&clip, q.rect.x, q.rect.y),
        1.0,
        "no inner clip"
    );
}
