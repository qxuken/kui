//! The half of the rounded-clip contract shader validation cannot see: the
//! core decides which rect and which four radii a quad's clip is, and the
//! shader decides where that shape actually cuts. Both compile and both
//! draw whatever they were told; only evaluating one over the other's
//! output shows they agree.
//!
//! `shade`'s clip block is mirrored in `tests/wgsl` and run here over quads
//! a real `Core` emitted. What it has to show is the symptom: a child of a
//! rounded, clipping card is fully painted in the middle of an edge and
//! gone in the corner it used to poke out of.

use kui_core::{Color, Core, NodeSpec, Quad, Size, Sizing};

mod wgsl;
use wgsl::inside;

const CARD: f32 = 100.0;
/// The card sits at (20, 20) — the root's padding — and is 100 square.
const X0: f32 = 20.0;
const Y0: f32 = 20.0;
const R: f32 = 16.0;

/// A rounded, clipping card with one child that fills it completely, so the
/// child's own quad reaches into all four corners. Returns the child's quad.
fn child_of_rounded_card(radius: NodeSpec) -> Quad {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    ui.with_keyed(
        "card",
        radius
            .width(Sizing::Fixed(CARD))
            .height(Sizing::Fixed(CARD))
            .clip(),
        |ui| {
            ui.with_keyed(
                "child",
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .bg(Color::rgb8(20, 20, 30)),
                |_| {},
            );
        },
    );
    ui.finish();
    let q = core.output().0.quads[0];
    assert_eq!(
        q.color,
        Color::rgb8(20, 20, 30),
        "the child is the only fill"
    );
    q
}

#[test]
fn the_clip_cuts_the_corner_the_child_used_to_poke_out_of() {
    let q = child_of_rounded_card(NodeSpec::column().radius(R));
    assert_eq!(q.clip_radius, [R; 4]);

    // Dead centre, and a pixel in from the middle of each straight edge:
    // entirely inside, exactly as a rect clip would have been.
    assert_eq!(inside(&q, X0 + CARD / 2.0, Y0 + CARD / 2.0), 1.0);
    assert_eq!(inside(&q, X0 + CARD / 2.0, Y0 + 1.0), 1.0, "mid top edge");
    assert_eq!(inside(&q, X0 + 1.0, Y0 + CARD / 2.0), 1.0, "mid left edge");

    // *On* a straight edge the rounded clip feathers over the same AA width
    // the card's own edge is drawn with, rather than cutting hard. That is
    // the point: the two edges have to blend into each other or a rounded
    // card shows a seam where its child stops.
    let straight = inside(&q, X0 + CARD / 2.0, Y0);
    assert!(
        (straight - 0.5).abs() < 0.05,
        "half on the edge: {straight}"
    );
    assert_eq!(
        inside(&q, X0 + CARD / 2.0, Y0 - 1.0),
        0.0,
        "and out past it"
    );

    // The corner pixel itself — the one that read as a rendering bug.
    for (cx, cy) in [
        (X0 + 0.5, Y0 + 0.5),
        (X0 + CARD - 0.5, Y0 + 0.5),
        (X0 + CARD - 0.5, Y0 + CARD - 0.5),
        (X0 + 0.5, Y0 + CARD - 0.5),
    ] {
        assert_eq!(inside(&q, cx, cy), 0.0, "corner ({cx}, {cy}) still painted");
    }

    // Half coverage on the arc itself: the clip antialiases rather than
    // stepping, which is the whole reason it is an SDF and not a mask.
    let d = R - R / 2f32.sqrt(); // the 45° point of the top-left arc
    let edge = inside(&q, X0 + d, Y0 + d);
    assert!((edge - 0.5).abs() < 0.2, "soft on the arc: {edge}");
}

#[test]
fn a_square_clip_is_the_rect_it_always_was() {
    let q = child_of_rounded_card(NodeSpec::column());
    assert_eq!(q.clip_radius, [0.0; 4]);
    // Hard in, hard out, no ramp — the fast path is unchanged.
    assert_eq!(inside(&q, X0 + 0.5, Y0 + 0.5), 1.0, "corner still painted");
    assert_eq!(inside(&q, X0 - 0.5, Y0 - 0.5), 0.0);
}

#[test]
fn per_corner_radii_cut_only_their_own_corners() {
    // A card rounded at the top: the top corners go, the bottom ones stay.
    let q = child_of_rounded_card(NodeSpec::column().radius_top(R));
    assert_eq!(q.clip_radius, [R, R, 0.0, 0.0]);
    assert_eq!(inside(&q, X0 + 0.5, Y0 + 0.5), 0.0, "top-left is cut");
    assert_eq!(
        inside(&q, X0 + CARD - 0.5, Y0 + 0.5),
        0.0,
        "top-right is cut"
    );
    assert_eq!(
        inside(&q, X0 + 1.0, Y0 + CARD - 1.0),
        1.0,
        "bottom-left is square"
    );
    assert_eq!(
        inside(&q, X0 + CARD - 1.0, Y0 + CARD - 1.0),
        1.0,
        "bottom-right is square"
    );
}

#[test]
fn the_clip_shape_is_the_clippers_box_not_the_quad_being_clipped() {
    // A child smaller than the card, offset inside it: the arc must stay on
    // the card's corner. Reading the radii against the quad's own rect —
    // the easy mistake — would put the cut somewhere in the middle.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    ui.with_keyed(
        "card",
        NodeSpec::column()
            .width(Sizing::Fixed(CARD))
            .height(Sizing::Fixed(CARD))
            .radius(R)
            .clip()
            .pad(30.0),
        |ui| {
            ui.with_keyed(
                "inner",
                NodeSpec::column()
                    .width(Sizing::Fixed(20.0))
                    .height(Sizing::Fixed(20.0))
                    .bg(Color::rgb8(200, 30, 30)),
                |_| {},
            );
        },
    );
    ui.finish();
    let q = core.output().0.quads[0];
    // Well inside the card, nowhere near a corner: nothing is cut.
    assert_eq!(inside(&q, X0 + 50.0, Y0 + 50.0), 1.0);
    assert_eq!(inside(&q, X0 + 51.0, Y0 + 51.0), 1.0);
    // And the card's corner is still where the cut is.
    assert_eq!(inside(&q, X0 + 0.5, Y0 + 0.5), 0.0);
}

#[test]
fn an_oversized_clip_radius_degrades_to_a_pill() {
    // `sd_rounded_box` clamps each radius to the half extents, so a radius
    // past half the box is a capsule rather than a folded-inside-out shape.
    let q = child_of_rounded_card(NodeSpec::column().radius(500.0));
    assert_eq!(q.clip_radius, [500.0; 4]);
    assert_eq!(inside(&q, X0 + CARD / 2.0, Y0 + CARD / 2.0), 1.0, "centre");
    assert_eq!(inside(&q, X0 + 0.5, Y0 + 0.5), 0.0, "corner");
    // A square box clamped to a circle: the middle of an edge is the one
    // point of it the arc still touches.
    let edge = inside(&q, X0 + CARD / 2.0, Y0 + 1.0);
    assert!(edge > 0.9, "the middle of an edge survives: {edge}");
}
