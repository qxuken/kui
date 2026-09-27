//! Hit-testing by shape (`docs/adr/0026-hit-testing-by-shape.md`): a
//! region's rect is the first test and its shape the second, so a press
//! in the corner of a rounded box, in the bounding box of a stroke but off
//! it, or in a polygon's box past its outline reaches what is under.
//! Every input path — hover, press, click, drag, the cursor shape — goes
//! through the one `contains`, so one set of shapes covers them all.

use kui_core::input::{HitShape, MIN_STROKE_GRAB, in_polygon, in_rounded_rect, segment_distance};
use kui_core::testing::{click_at, kinds};
use kui_core::{Color, Core, CursorShape, FloatConfig, InputEvent, NodeSpec, Size, Stroke, Vec2};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn frame(core: &mut Core, build: impl FnOnce(&mut kui_core::Ui<'_>)) {
    let mut ui = core.frame(VIEW, 1.0);
    build(&mut ui);
    ui.finish();
}

fn tag(events: &[kui_core::UiEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| e.payload.as_str().map(str::to_string))
        .collect()
}

// -- The geometry, on its own ---------------------------------------------

#[test]
fn a_rounded_rect_excludes_its_corners_and_nothing_else() {
    let r = [10.0; 4];
    // The dead corner: inside the box, outside the arc.
    assert!(!in_rounded_rect(Vec2::new(1.0, 1.0), 100.0, 50.0, r));
    // Just inside the arc.
    assert!(in_rounded_rect(Vec2::new(4.0, 4.0), 100.0, 50.0, r));
    // The middle of an edge, and the middle of the box.
    assert!(in_rounded_rect(Vec2::new(50.0, 0.5), 100.0, 50.0, r));
    assert!(in_rounded_rect(Vec2::new(50.0, 25.0), 100.0, 50.0, r));
    // Square: nothing is excluded.
    assert!(in_rounded_rect(Vec2::new(0.0, 0.0), 100.0, 50.0, [0.0; 4]));
    // Per corner: only the rounded one bites.
    assert!(in_rounded_rect(
        Vec2::new(1.0, 1.0),
        100.0,
        50.0,
        [0.0, 10.0, 0.0, 0.0]
    ));
    assert!(!in_rounded_rect(
        Vec2::new(99.0, 1.0),
        100.0,
        50.0,
        [0.0, 10.0, 0.0, 0.0]
    ));
    // An oversized radius is the pill it draws as, not a fold.
    assert!(in_rounded_rect(
        Vec2::new(50.0, 25.0),
        100.0,
        50.0,
        [1000.0; 4]
    ));
    assert!(!in_rounded_rect(
        Vec2::new(1.0, 1.0),
        100.0,
        50.0,
        [1000.0; 4]
    ));
}

#[test]
fn segment_distance_is_to_the_piece_not_the_line_through_it() {
    let (a, b) = (Vec2::new(0.0, 0.0), Vec2::new(10.0, 0.0));
    assert_eq!(segment_distance(Vec2::new(5.0, 3.0), a, b), 3.0);
    // Past the end: to the endpoint, not the extended line.
    assert!((segment_distance(Vec2::new(13.0, 4.0), a, b) - 5.0).abs() < 1e-5);
    // A degenerate piece is a point.
    assert_eq!(segment_distance(Vec2::new(3.0, 4.0), a, a), 5.0);
}

#[test]
fn a_polygon_test_is_even_odd_and_concave_is_fine() {
    let tri = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 0.0),
        Vec2::new(0.0, 10.0),
    ];
    assert!(in_polygon(Vec2::new(2.0, 2.0), &tri));
    assert!(
        !in_polygon(Vec2::new(8.0, 8.0), &tri),
        "past the hypotenuse"
    );
    // A concave chevron: the notch is outside.
    let chevron = [
        Vec2::new(0.0, 0.0),
        Vec2::new(10.0, 5.0),
        Vec2::new(0.0, 10.0),
        Vec2::new(4.0, 5.0),
    ];
    assert!(in_polygon(Vec2::new(6.0, 5.0), &chevron));
    assert!(!in_polygon(Vec2::new(2.0, 5.0), &chevron), "the notch");
    // Padding — the last point repeated, as a `polygon` node pads to
    // eight — changes nothing.
    let padded = [tri[0], tri[1], tri[2], tri[2], tri[2]];
    assert!(in_polygon(Vec2::new(2.0, 2.0), &padded));
    assert!(!in_polygon(Vec2::new(8.0, 8.0), &padded));
    assert!(
        !in_polygon(Vec2::new(1.0, 1.0), &tri[..2]),
        "two points are no shape"
    );
}

// -- Through the core ------------------------------------------------------

/// The case that asked for this: two wedges of a pie share a bounding
/// box, and a press in one must not reach the other — or, past both
/// outlines, anything but the canvas.
#[test]
fn two_wedges_sharing_a_box_are_told_apart() {
    let mut core = Core::new();
    let c = Vec2::new(100.0, 100.0);
    // Two quarter-wedges as triangles: upper-right and lower-right; their
    // bounding boxes both span x 100..180.
    let upper = [c, Vec2::new(180.0, 100.0), Vec2::new(100.0, 20.0)];
    let lower = [c, Vec2::new(180.0, 100.0), Vec2::new(100.0, 180.0)];
    frame(&mut core, |ui| {
        // A group by role: with its click alone the canvas would be a
        // button, which folds its children into its own name.
        let canvas = NodeSpec::column()
            .fill()
            .on_click("canvas")
            .role(kui_core::Role::Group);
        ui.with(canvas, |ui| {
            ui.polygon_keyed(
                "upper",
                &upper,
                NodeSpec::column()
                    .bg(Color::WHITE)
                    .on_click("upper")
                    .label("Upper"),
            );
            ui.polygon_keyed(
                "lower",
                &lower,
                NodeSpec::column()
                    .bg(Color::WHITE)
                    .on_click("lower")
                    .label("Lower"),
            );
        });
    });
    assert_eq!(tag(&click_at(&mut core, 130.0, 80.0)), ["upper"]);
    assert_eq!(tag(&click_at(&mut core, 130.0, 120.0)), ["lower"]);
    // In both boxes, in neither outline: the canvas.
    assert_eq!(tag(&click_at(&mut core, 170.0, 30.0)), ["canvas"]);
    // And to assistive technology they are two buttons.
    let roles: Vec<_> = core
        .access_tree()
        .nodes
        .iter()
        .filter_map(|n| n.name.clone())
        .collect();
    assert_eq!(roles, ["Upper", "Lower"]);
}

/// Hover follows the shape too, and the wedge's `hover_bg` with it.
#[test]
fn hover_is_by_shape() {
    let mut core = Core::new();
    let tri = [
        Vec2::new(10.0, 10.0),
        Vec2::new(110.0, 10.0),
        Vec2::new(10.0, 110.0),
    ];
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.polygon_keyed(
                "tri",
                &tri,
                NodeSpec::column()
                    .bg(Color::WHITE)
                    .hover_bg(Color::BLACK)
                    .on_hover("tri"),
            );
        });
    };
    frame(&mut core, build);
    let key = core.key_of("tri").unwrap();
    // The wedge's fill is the fragment quad's colour.
    let fill = |core: &mut Core| {
        let (dl, _) = core.output();
        dl.quads
            .iter()
            .find(|q| q.kind == kui_core::QuadKind::Fragment)
            .map(|q| q.color)
            .expect("the wedge drew")
    };
    assert_eq!(fill(&mut core), Color::WHITE);
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 20.0)));
    assert_eq!(kinds(&evs), ["hover"]);
    assert!(core.is_hovered(key));
    // AR16: hovered, the next frame paints `hover_bg` — the polygon door
    // ran the easing and skipped the hover resolve, so the wedge was
    // hovered and still white.
    frame(&mut core, build);
    assert_eq!(
        fill(&mut core),
        Color::BLACK,
        "the wedge paints its hover_bg"
    );
    // Across the hypotenuse, still in the box: left.
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(100.0, 100.0)));
    assert_eq!(kinds(&evs), ["hover"]);
    assert!(!core.is_hovered(key));
    frame(&mut core, build);
    assert_eq!(fill(&mut core), Color::WHITE);
}

/// A hairline is a 4 px target: within two px of a 1 px stroke hits, five
/// px away does not; a wide stroke is hit by its own width.
#[test]
fn a_stroke_is_hit_within_its_width_and_never_less_than_the_grab() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("canvas"), |ui| {
            ui.line_keyed(
                "thin",
                Vec2::new(10.0, 50.0),
                Vec2::new(200.0, 50.0),
                Stroke::new(1.0, Color::WHITE),
                NodeSpec::column().on_click("thin"),
            );
            ui.line_keyed(
                "wide",
                Vec2::new(10.0, 150.0),
                Vec2::new(200.0, 150.0),
                Stroke::new(20.0, Color::WHITE),
                NodeSpec::column().on_click("wide"),
            );
        });
    });
    assert_eq!(MIN_STROKE_GRAB, 4.0);
    assert_eq!(tag(&click_at(&mut core, 100.0, 51.5)), ["thin"]);
    // The thin line's box is 1 + 2×2 px tall (half the width plus two of
    // ramp padding each side), so 5 px away is outside it either way;
    // 2.5 px is inside the box and outside the grab.
    assert_eq!(tag(&click_at(&mut core, 100.0, 52.5)), ["canvas"]);
    assert_eq!(tag(&click_at(&mut core, 100.0, 158.0)), ["wide"]);
    assert_eq!(tag(&click_at(&mut core, 100.0, 162.5)), ["canvas"]);
}

/// A curve is hit along its pieces: on the arc, not across the chord.
#[test]
fn a_curve_is_hit_along_its_pieces() {
    let mut core = Core::new();
    let knots = [
        Vec2::new(20.0, 100.0),
        Vec2::new(100.0, 20.0),
        Vec2::new(180.0, 100.0),
    ];
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("canvas"), |ui| {
            ui.polyline_keyed(
                "arc",
                &knots,
                Stroke::new(3.0, Color::WHITE).curve(),
                NodeSpec::column().on_click("arc"),
            );
        });
    });
    // The middle knot is on the curve; the middle of the chord between
    // the outer knots is well below it, inside the bounding box.
    assert_eq!(tag(&click_at(&mut core, 100.0, 20.0)), ["arc"]);
    assert_eq!(tag(&click_at(&mut core, 100.0, 100.0)), ["canvas"]);
}

/// The corner of a rounded box: what the README used to name as the
/// standing caveat. A press in the dead corner reaches the row under.
#[test]
fn a_rounded_corner_is_not_a_hit() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("page"), |ui| {
            ui.leaf(
                NodeSpec::column()
                    .float(FloatConfig::parent().offset(10.0, 10.0))
                    .size(100.0, 60.0)
                    .radius(20.0)
                    .on_click("card"),
            );
        });
    });
    assert_eq!(
        tag(&click_at(&mut core, 60.0, 40.0)),
        ["card"],
        "the middle"
    );
    assert_eq!(
        tag(&click_at(&mut core, 12.0, 12.0)),
        ["page"],
        "the dead corner"
    );
    assert_eq!(
        tag(&click_at(&mut core, 30.0, 11.0)),
        ["card"],
        "the edge past the arc"
    );
}

/// The cursor shape reads the same test: the hand a clickable wedge
/// declares over it, the default beside it in the same box.
#[test]
fn the_cursor_follows_the_shape() {
    let mut core = Core::new();
    let tri = [
        Vec2::new(10.0, 10.0),
        Vec2::new(110.0, 10.0),
        Vec2::new(10.0, 110.0),
    ];
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.polygon(
                &tri,
                NodeSpec::column()
                    .bg(Color::WHITE)
                    .on_click("tri")
                    .cursor(CursorShape::Pointer),
            );
        });
    });
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 20.0)));
    assert_eq!(core.cursor_shape(), CursorShape::Pointer);
    core.handle_input(InputEvent::CursorMoved(Vec2::new(100.0, 100.0)));
    assert_eq!(core.cursor_shape(), CursorShape::Default);
}

/// A drag starts by shape as a click does; once captured it follows the
/// pointer anywhere, as every drag does.
#[test]
fn a_drag_starts_by_shape() {
    let mut core = Core::new();
    let tri = [
        Vec2::new(10.0, 10.0),
        Vec2::new(110.0, 10.0),
        Vec2::new(10.0, 110.0),
    ];
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.polygon_keyed(
                "tri",
                &tri,
                NodeSpec::column().bg(Color::WHITE).on_drag("tri"),
            );
        });
    });
    // Past the hypotenuse: no drag.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(100.0, 100.0)));
    assert!(kinds(&core.handle_input(InputEvent::mouse_down(1))).is_empty());
    core.handle_input(InputEvent::mouse_up());
    // Inside: a drag, which then follows the pointer out of the outline.
    core.handle_input(InputEvent::CursorMoved(Vec2::new(20.0, 20.0)));
    assert_eq!(
        kinds(&core.handle_input(InputEvent::mouse_down(1))),
        ["drag"]
    );
    assert_eq!(
        kinds(&core.handle_input(InputEvent::CursorMoved(Vec2::new(150.0, 150.0)))),
        ["drag"]
    );
    core.handle_input(InputEvent::mouse_up());
}

/// The shape list is rebuilt with the regions: a frame that stops
/// declaring a shaped node leaves no stale shape for a new region to
/// index.
#[test]
fn shapes_are_the_frames_own() {
    let mut core = Core::new();
    let tri = [
        Vec2::new(10.0, 10.0),
        Vec2::new(110.0, 10.0),
        Vec2::new(10.0, 110.0),
    ];
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.polygon(&tri, NodeSpec::column().bg(Color::WHITE).on_click("tri"));
        });
    });
    assert_eq!(tag(&click_at(&mut core, 20.0, 20.0)), ["tri"]);
    // The same box, plain: the corner that missed the triangle hits it.
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.leaf(
                NodeSpec::column()
                    .float(FloatConfig::parent().offset(9.0, 9.0))
                    .size(102.0, 102.0)
                    .on_click("box"),
            );
        });
    });
    assert_eq!(tag(&click_at(&mut core, 100.0, 100.0)), ["box"]);
    let _ = HitShape::Rect;
}

/// A region whose shape indexes points the interaction does not hold —
/// installed through `set_hits` without its shapes — misses rather than
/// panics on the next pointer move.
#[test]
fn a_shape_without_its_points_misses() {
    use kui_core::input::{HitRegion, Interaction};
    let mut it = Interaction::default();
    let mut hits = Vec::new();
    // Built the way a host mirroring regions would: every field spelled.
    let base = {
        let mut core = Core::new();
        frame(&mut core, |ui| {
            ui.leaf(NodeSpec::column().fill().on_click("box"));
        });
        core.interaction.hits()[0].clone()
    };
    hits.push(HitRegion {
        shape: HitShape::Polygon { first: 40, len: 8 },
        ..base
    });
    it.set_hits(hits);
    let mut out = Vec::new();
    it.handle(InputEvent::CursorMoved(Vec2::new(10.0, 10.0)), &mut out);
    assert!(out.is_empty());
    assert!(it.hovered().is_none(), "a shape with no points is a miss");
}
