//! A clipping node with a radius rounds what it clips, so the children of a
//! rounded card stay inside its corners instead of poking out square. It is
//! not a prop: the radius is the clipping node's own, exactly as CSS rounds
//! `overflow: hidden` under a `border-radius`.
//!
//! What is pinned here is the propagation — which quads carry which radii,
//! at which scale, and what nesting does to a corner. The other half, that
//! the shader draws the shape the core described, is
//! `crates/kui-wgpu/tests/rounded_clip_coverage.rs`.

use kui_core::{Clip, Color, Core, NodeSpec, Quad, Rect, Size, Vec2};

const CARD: f32 = 100.0;
const CHILD: f32 = 60.0;

/// A card at (10, 10) with one child inside it, and whatever the caller
/// wants on each. Returns the emitted quads — the card's, then the
/// child's — each paired with the clip it names, resolved out of
/// `DisplayList::clips`.
fn card_and_child(
    core: &mut Core,
    card: NodeSpec,
    child: NodeSpec,
    scale: f32,
) -> (Vec<Quad>, Vec<Clip>) {
    let mut ui = core.frame(Size::new(300.0, 200.0), scale);
    ui.configure_root(NodeSpec::column().pad(10.0));
    ui.with_keyed("card", card.size(CARD, CARD).bg(Color::WHITE), |ui| {
        ui.leaf_keyed(
            "child",
            child.size(CHILD, CHILD).bg(Color::rgb8(20, 20, 30)),
        );
    });
    ui.finish();
    let dl = core.output().0;
    dl.quads.iter().map(|q| (*q, dl.clip_of(q))).unzip()
}

#[test]
fn a_rounded_clipper_rounds_the_clip_its_children_inherit() {
    let mut core = Core::new();
    let (q, c) = card_and_child(
        &mut core,
        NodeSpec::column().radius(12.0).clip(),
        NodeSpec::column(),
        1.0,
    );
    // The card draws itself rounded, as it always did, and is not clipped
    // by its own clip: `clips[i]` is ancestors only.
    assert_eq!(q[0].radius, [12.0; 4]);
    assert_eq!(c[0].radius, [0.0; 4], "the card is not its own clip");
    // The child is square, and the clip it inherits is not.
    assert_eq!(q[1].radius, [0.0; 4]);
    assert_eq!(c[1].radius, [12.0; 4]);
    assert_eq!(c[1].rect, Rect::new(10.0, 10.0, CARD, CARD));
}

#[test]
fn a_scrolling_container_rounds_what_it_scrolls() {
    // `scroll_y` clips too, and is the case the symptom was reported as:
    // rows sliding out of a rounded card with square corners.
    let mut core = Core::new();
    let (_q, c) = card_and_child(
        &mut core,
        NodeSpec::column().radius(8.0).scroll_y(),
        NodeSpec::column(),
        1.0,
    );
    assert_eq!(c[1].radius, [8.0; 4]);
}

#[test]
fn per_corner_radii_reach_the_clip_per_corner() {
    // The shape a uniform clip radius could not express: a card rounded at
    // the top only, the way a header sits over a flush-bottomed panel.
    let mut core = Core::new();
    let (_q, c) = card_and_child(
        &mut core,
        NodeSpec::column().radius_top(14.0).clip(),
        NodeSpec::column(),
        1.0,
    );
    assert_eq!(c[1].radius, [14.0, 14.0, 0.0, 0.0]);
}

#[test]
fn a_clipper_without_a_radius_still_clips_square() {
    let mut core = Core::new();
    let (_q, c) = card_and_child(
        &mut core,
        NodeSpec::column().clip(),
        NodeSpec::column(),
        1.0,
    );
    assert_eq!(c[1].rect, Rect::new(10.0, 10.0, CARD, CARD));
    assert_eq!(c[1].radius, [0.0; 4]);
}

#[test]
fn a_rounded_node_that_does_not_clip_rounds_nothing() {
    // Rounding the clip is what *clipping* does; a radius on a node that
    // lets its children overflow changes only its own corners.
    let mut core = Core::new();
    let (q, c) = card_and_child(
        &mut core,
        NodeSpec::column().radius(12.0),
        NodeSpec::column(),
        1.0,
    );
    assert_eq!(q[0].radius, [12.0; 4]);
    assert_eq!(c[1].radius, [0.0; 4]);
}

#[test]
fn clip_radii_are_physical_pixels() {
    let mut core = Core::new();
    let (_q, c) = card_and_child(
        &mut core,
        NodeSpec::column().radius(12.0).clip(),
        NodeSpec::column(),
        2.0,
    );
    assert_eq!(c[1].radius, [24.0; 4], "scaled like every other length");
    assert_eq!(c[1].rect, Rect::new(20.0, 20.0, 200.0, 200.0));
}

#[test]
fn glyphs_and_images_inherit_the_rounded_clip_too() {
    // Text and images do not go through `emit_node`'s own quad push, so
    // they are the two that could silently keep a square clip.
    let mut core = Core::new();
    let img = core.resources.add_image(2, 2, vec![0xff; 16]);
    let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().pad(10.0));
    ui.with_keyed(
        "card",
        NodeSpec::column().size(CARD, CARD).radius(9.0).clip(),
        |ui| {
            ui.text("hello", kui_core::TextStyle::new(14.0));
            ui.image(img, NodeSpec::column().width(20.0));
        },
    );
    ui.finish();
    let (dl, _) = core.output();
    assert!(dl.quads.len() > 2, "text and image both drew");
    for q in &dl.quads {
        assert_eq!(
            dl.clip_of(q).radius,
            [9.0; 4],
            "{:?} kept a square clip",
            q.kind
        );
    }
}

#[test]
fn a_float_escapes_the_rounded_clip_with_the_rest_of_it() {
    // Floats already escape ancestor clips; the radii have to leave with
    // the rect or a tooltip gets clipped to a shape that is not there.
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
    ui.configure_root(NodeSpec::column().pad(10.0));
    ui.with_keyed(
        "card",
        NodeSpec::column()
            .size(CARD, CARD)
            .radius(12.0)
            .clip()
            .bg(Color::WHITE),
        |ui| {
            ui.leaf_keyed(
                "tip",
                NodeSpec::column()
                    .float(kui_core::FloatConfig::below())
                    .size(40.0, 20.0)
                    .bg(Color::rgb8(255, 0, 0)),
            );
        },
    );
    ui.finish();
    let (dl, _) = core.output();
    let tip = dl.quads.last().expect("the float draws last");
    assert_eq!(dl.clip_of(tip).radius, [0.0; 4]);
}

// -- Nesting ---------------------------------------------------------------
// One rounded rect cannot name the intersection of two, so `Clip::intersect`
// decides per corner. These pin the rule directly, where a tree would only
// reach a few of its cases.

const BOX: Rect = Rect {
    x: 0.0,
    y: 0.0,
    w: 100.0,
    h: 100.0,
};

#[test]
fn a_corner_an_ancestor_cut_away_is_square() {
    // The outer clip keeps the left half. The rounded box's right-hand
    // corners are gone — a straight edge made them square — and its
    // left-hand ones survive untouched.
    let outer = Clip::rect(Rect::new(0.0, 0.0, 50.0, 100.0));
    let c = outer.intersect(BOX, [10.0; 4]);
    assert_eq!(c.rect, Rect::new(0.0, 0.0, 50.0, 100.0));
    assert_eq!(c.radius, [10.0, 0.0, 0.0, 10.0]);
}

#[test]
fn a_square_clipper_inside_a_rounded_one_keeps_the_corners_it_did_not_move() {
    // A plain clipper flush with the rounded card's top-left, cutting its
    // bottom-right: the two corners it did not touch stay rounded.
    let rounded = Clip::rect(BOX).intersect(BOX, [10.0; 4]);
    assert_eq!(rounded.radius, [10.0; 4]);
    let inner = rounded.intersect(Rect::new(0.0, 0.0, 60.0, 60.0), [0.0; 4]);
    assert_eq!(inner.rect, Rect::new(0.0, 0.0, 60.0, 60.0));
    assert_eq!(inner.radius, [10.0, 0.0, 0.0, 0.0]);
}

#[test]
fn two_rounded_clippers_on_one_corner_take_the_tighter_cut() {
    // Same box, two radii: the intersection of two rounded corners is the
    // one that cuts more, so the larger radius wins.
    let outer = Clip::rect(BOX).intersect(BOX, [4.0, 20.0, 0.0, 0.0]);
    let both = outer.intersect(BOX, [12.0, 6.0, 0.0, 0.0]);
    assert_eq!(both.radius, [12.0, 20.0, 0.0, 0.0]);
}

#[test]
fn nothing_rounds_a_clip_that_rounds_nothing() {
    let c = Clip::NONE.intersect(BOX, [0.0; 4]);
    assert_eq!(c.rect, BOX);
    assert_eq!(c.radius, [0.0; 4]);
}

#[test]
fn a_scrolled_child_keeps_the_containers_corners_not_its_own_position() {
    // Scrolling moves the child, not the clip: the rounded corners belong
    // to the container's box and stay where the container is.
    let mut core = Core::new();
    let frame = |core: &mut Core| {
        let mut ui = core.frame(Size::new(300.0, 200.0), 1.0);
        ui.configure_root(NodeSpec::column().pad(10.0));
        ui.with_keyed(
            "list",
            NodeSpec::column().size(CARD, CARD).radius(12.0).scroll_y(),
            |ui| {
                for i in 0..8 {
                    ui.leaf_indexed(
                        i,
                        NodeSpec::column()
                            .grow_width()
                            .height(30.0)
                            .bg(Color::rgb8(40, 40, 60)),
                    );
                }
            },
        );
        ui.finish();
    };
    frame(&mut core);
    core.set_scroll(kui_core::Key::ROOT.str("list"), Vec2::new(0.0, 45.0));
    frame(&mut core);
    let (dl, _) = core.output();
    let rows: Vec<&Quad> = dl
        .quads
        .iter()
        .filter(|q| q.color == Color::rgb8(40, 40, 60))
        .collect();
    assert!(rows.len() > 2, "several rows visible");
    for r in rows {
        let clip = dl.clip_of(r);
        assert_eq!(clip.rect, Rect::new(10.0, 10.0, CARD, CARD));
        assert_eq!(clip.radius, [12.0; 4]);
    }
}
