//! The `line` element (`docs/adr/0010-a-segment-primitive.md`): one
//! `QuadKind::Segment` per straight piece, endpoints in `uv`, the node a
//! float sized to the stroke's box, and nothing else — no place in its
//! parent's flow, and no hit region or access row unless it takes input,
//! when it is hit by its stroke (`docs/adr/0026-hit-testing-by-shape.md`;
//! the shape tests themselves are in `tests/hit.rs`).

use kui_core::line::{CURVE_STEP, flatten_curve};
use kui_core::{
    Color, Core, FloatAnchor, FloatConfig, InputEvent, MouseButton, NodeSpec, Quad, QuadKind, Size,
    Stroke, Value, Vec2,
};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn segments(quads: &[Quad]) -> Vec<&Quad> {
    quads
        .iter()
        .filter(|q| q.kind == QuadKind::Segment)
        .collect()
}

fn frame(core: &mut Core, scale: f32, build: impl FnOnce(&mut kui_core::Ui<'_>)) -> Vec<Quad> {
    let mut ui = core.frame(VIEW, scale);
    build(&mut ui);
    ui.finish();
    core.output().0.quads.clone()
}

#[test]
fn a_line_is_one_segment_quad_with_its_endpoints_in_uv() {
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.with(NodeSpec::column().pad(20.0), |ui| {
            ui.line(
                Vec2::new(10.0, 10.0),
                Vec2::new(90.0, 70.0),
                Stroke::new(4.0, Color::hex(0x7f9cf5ff)),
                NodeSpec::column(),
            );
        });
    });
    let segs = segments(&quads);
    assert_eq!(segs.len(), 1);
    let s = segs[0];
    // Points are in the parent's box space: the parent sits at the origin,
    // so its padding does not move them.
    assert_eq!(s.segment_ends(), [10.0, 10.0, 90.0, 70.0]);
    assert_eq!(s.border_w, 4.0, "the stroke width rides in border_w");
    assert_eq!(s.color, Color::hex(0x7f9cf5ff));
    // The box: the endpoints padded by half the width plus two.
    assert_eq!(
        (s.rect.x, s.rect.y, s.rect.w, s.rect.h),
        (6.0, 6.0, 88.0, 68.0)
    );
    assert_eq!(s.radius, [0.0; 4]);
    assert_eq!(s.blur, 0.0);
    // No box was filled for it: the parent has no bg, so the segment is
    // the only quad.
    assert_eq!(quads.len(), 1);
}

#[test]
fn everything_on_the_quad_is_physical() {
    let mut core = Core::new();
    let quads = frame(&mut core, 2.0, |ui| {
        ui.line(
            Vec2::new(10.0, 10.0),
            Vec2::new(50.0, 10.0),
            Stroke::new(3.0, Color::WHITE),
            NodeSpec::column(),
        );
    });
    let s = segments(&quads)[0];
    assert_eq!(s.segment_ends(), [20.0, 20.0, 100.0, 20.0]);
    assert_eq!(s.border_w, 6.0);
    // pad = 3 * 0.5 + 2 = 3.5 logical = 7 physical.
    assert_eq!(
        (s.rect.x, s.rect.y, s.rect.w, s.rect.h),
        (13.0, 13.0, 94.0, 14.0)
    );
}

#[test]
fn a_polyline_is_one_segment_per_piece_and_a_curve_is_flattened_in_the_core() {
    let knots = [
        Vec2::new(20.0, 100.0),
        Vec2::new(60.0, 80.0),
        Vec2::new(100.0, 110.0),
    ];
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.polyline(&knots, Stroke::new(2.0, Color::WHITE), NodeSpec::column());
        ui.polyline(
            &knots,
            Stroke::new(2.0, Color::WHITE).curve(),
            NodeSpec::column(),
        );
    });
    let segs = segments(&quads);
    // The polyline: two pieces, joined at the middle knot.
    assert_eq!(segs[0].segment_ends(), [20.0, 100.0, 60.0, 80.0]);
    assert_eq!(segs[1].segment_ends(), [60.0, 80.0, 100.0, 110.0]);
    // The curve: what `flatten_curve` says, and nothing the app did.
    let mut flat = Vec::new();
    flatten_curve(&knots, &mut flat);
    assert_eq!(segs.len(), 2 + flat.len() - 1);
    let chord = |a: Vec2, b: Vec2| ((b.x - a.x).powi(2) + (b.y - a.y).powi(2)).sqrt();
    let pieces = |a, b| (chord(a, b) / CURVE_STEP).ceil() as usize;
    assert_eq!(
        flat.len() - 1,
        pieces(knots[0], knots[1]) + pieces(knots[1], knots[2])
    );
    let first = segs[2].segment_ends();
    let last = segs.last().unwrap().segment_ends();
    assert_eq!(
        [first[0], first[1]],
        [20.0, 100.0],
        "starts at the first knot"
    );
    assert_eq!([last[2], last[3]], [100.0, 110.0], "ends at the last");
}

#[test]
fn a_line_takes_no_room_and_paints_over_its_siblings() {
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.with(NodeSpec::column().gap(4.0), |ui| {
            ui.leaf_keyed("a", NodeSpec::column().size(50.0, 20.0).bg(Color::WHITE));
            ui.line(
                Vec2::new(0.0, 0.0),
                Vec2::new(50.0, 50.0),
                Stroke::new(1.0, Color::WHITE),
                NodeSpec::column(),
            );
            ui.leaf_keyed("b", NodeSpec::column().size(50.0, 20.0).bg(Color::WHITE));
        });
    });
    let solids: Vec<&Quad> = quads.iter().filter(|q| q.kind == QuadKind::Solid).collect();
    assert_eq!(solids.len(), 2);
    // `b` sits right under `a` plus the gap: the line took no slot.
    assert_eq!(solids[1].rect.y, 24.0);
    // Floats paint after in-flow content.
    assert_eq!(quads.last().unwrap().kind, QuadKind::Segment);
}

#[test]
fn a_viewport_anchor_reads_the_points_in_viewport_space() {
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.with(
            NodeSpec::column()
                .pad(30.0)
                .float(FloatConfig::parent().offset(100.0, 50.0)),
            |ui| {
                ui.line(
                    Vec2::new(0.0, 0.0),
                    Vec2::new(10.0, 0.0),
                    Stroke::new(1.0, Color::WHITE),
                    NodeSpec::column(),
                );
                ui.line(
                    Vec2::new(0.0, 0.0),
                    Vec2::new(10.0, 0.0),
                    Stroke::new(1.0, Color::WHITE),
                    NodeSpec::column().float(FloatConfig::viewport()),
                );
            },
        );
    });
    let segs = segments(&quads);
    assert_eq!(
        segs[0].segment_ends(),
        [100.0, 50.0, 110.0, 50.0],
        "parent box space"
    );
    assert_eq!(
        segs[1].segment_ends(),
        [0.0, 0.0, 10.0, 0.0],
        "viewport space"
    );
    let _ = FloatAnchor::Viewport;
}

/// A line with a click is hit by its stroke: on the diagonal the line
/// answers, in the corner of its bounding box the box under it does
/// (ADR 0026). No warning either way.
#[test]
fn a_line_with_input_is_hit_by_its_stroke() {
    let mut core = Core::new();
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.with(
            // A group, not the button its click would make it: a button
            // folds its children into its name, and the point here is
            // the line's own row.
            NodeSpec::column()
                .size(100.0, 100.0)
                .on_click("under")
                .role(kui_core::Role::Group)
                .label("under"),
            |ui| {
                ui.line_keyed(
                    "l",
                    Vec2::new(0.0, 0.0),
                    Vec2::new(100.0, 100.0),
                    Stroke::new(8.0, Color::WHITE),
                    NodeSpec::column().on_click("line").label("the diagonal"),
                );
            },
        );
    };
    frame(&mut core, 1.0, build);
    assert!(core.take_warnings().is_empty());
    let click = |core: &mut Core, x: f32, y: f32| {
        core.handle_input(InputEvent::CursorMoved(Vec2::new(x, y)));
        core.handle_input(InputEvent::MouseDown {
            button: MouseButton::Primary,
            clicks: 1,
        });
        core.handle_input(InputEvent::MouseUp {
            button: MouseButton::Primary,
        })
    };
    // On the stroke: the line.
    let events = click(&mut core, 50.0, 50.0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload, Value::str("line"));
    // In its box, off the stroke: the box under it.
    let events = click(&mut core, 90.0, 10.0);
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload, Value::str("under"));
    // And it is a control to assistive technology now.
    assert_eq!(
        core.access_tree().nodes.len(),
        3,
        "the window, the box and the line"
    );
}

#[test]
fn a_transition_eases_the_colour_and_an_exit_replays_the_points() {
    let mut core = Core::new();
    core.set_time(0.0);
    let line = |ui: &mut kui_core::Ui<'_>, color: Color| {
        ui.line_keyed(
            "l",
            Vec2::new(10.0, 10.0),
            Vec2::new(50.0, 30.0),
            Stroke::new(2.0, color),
            NodeSpec::column()
                .transition(100.0)
                .exit(kui_core::Enter::default().opacity(0.0)),
        );
    };
    let black = Color::rgba8(0, 0, 0, 255);
    let white = Color::rgba8(255, 255, 255, 255);
    frame(&mut core, 1.0, |ui| line(ui, black));
    frame(&mut core, 1.0, |ui| line(ui, white));
    core.set_time(0.05);
    let quads = frame(&mut core, 1.0, |ui| line(ui, white));
    let s = segments(&quads)[0];
    assert!(
        s.color.r > 0.0 && s.color.r < 1.0,
        "mid-tween: {:?}",
        s.color
    );
    assert_eq!(
        s.segment_ends(),
        [10.0, 10.0, 50.0, 30.0],
        "the points do not tween"
    );
    // Gone: the ghost draws the same segment, fading.
    core.set_time(0.2);
    frame(&mut core, 1.0, |ui| line(ui, white));
    core.set_time(0.25);
    frame(&mut core, 1.0, |_| {});
    core.set_time(0.3);
    let quads = frame(&mut core, 1.0, |_| {});
    let ghost = segments(&quads);
    assert_eq!(ghost.len(), 1, "the ghost carries its points");
    assert_eq!(ghost[0].segment_ends(), [10.0, 10.0, 50.0, 30.0]);
    assert!(ghost[0].color.a < 1.0 && ghost[0].color.a > 0.0);
    core.set_time(1.0);
    let quads = frame(&mut core, 1.0, |_| {});
    assert!(
        segments(&quads).is_empty(),
        "and is dropped when its exit ends"
    );
}

#[test]
fn fewer_than_two_points_draw_nothing() {
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.polyline(
            &[Vec2::new(1.0, 1.0)],
            Stroke::new(1.0, Color::WHITE),
            NodeSpec::column(),
        );
        ui.polyline(&[], Stroke::new(1.0, Color::WHITE), NodeSpec::column());
    });
    assert!(quads.is_empty());
}

/// A stroke's `bg` is its colour, not a box to fill — for its ghost too.
/// The ghost painter used to be a second copy of the live one, and the
/// copy filled the departing stroke's whole box with the stroke colour.
#[test]
fn a_departing_line_leaves_no_box_behind() {
    let mut core = Core::new();
    let stroke = Stroke::new(4.0, Color::hex(0x7f9cf5ff));
    let spec = || {
        NodeSpec::column()
            .transition(100.0)
            .exit(kui_core::Enter::from(20.0, 0.0))
    };
    core.set_time(0.0);
    let mut ui = core.frame(VIEW, 1.0);
    ui.line_keyed(
        "l",
        Vec2::new(10.0, 10.0),
        Vec2::new(90.0, 70.0),
        stroke,
        spec(),
    );
    ui.finish();
    assert_eq!(core.output().0.quads.len(), 1, "live: the segment only");

    // Gone from the view: the ghost is the segment, still, and nothing else.
    core.set_time(0.05);
    let ui = core.frame(VIEW, 1.0);
    ui.finish();
    let quads = core.output().0.quads.clone();
    assert_eq!(segments(&quads).len(), 1, "the ghost's stroke");
    assert!(
        quads.iter().all(|q| q.kind == QuadKind::Segment),
        "no box was filled for the departing stroke: {:?}",
        quads.iter().map(|q| q.kind).collect::<Vec<_>>()
    );
}

/// A dashed stroke (backlog V2) is one segment per mark: a mark is a
/// short round-capped stroke, so the lengths declared are the ones seen —
/// a 6 px mark at a width of 2 is a 4 px centre line and its two caps.
#[test]
fn a_dashed_line_is_one_segment_per_mark() {
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.line(
            Vec2::new(10.0, 50.0),
            Vec2::new(110.0, 50.0),
            Stroke::new(2.0, Color::WHITE).dash(6.0, 4.0),
            NodeSpec::column(),
        );
    });
    let segs = segments(&quads);
    assert_eq!(segs.len(), 10, "a mark every 10 px of 100");
    assert_eq!(segs[0].segment_ends(), [10.0, 50.0, 14.0, 50.0]);
    assert_eq!(segs[1].segment_ends(), [20.0, 50.0, 24.0, 50.0]);
    assert_eq!(segs[9].segment_ends(), [100.0, 50.0, 104.0, 50.0]);
    assert!(segs.iter().all(|s| s.border_w == 2.0));
    // Each mark's quad is its own box, padded as a stroke's is.
    assert_eq!(
        (
            segs[1].rect.x,
            segs[1].rect.y,
            segs[1].rect.w,
            segs[1].rect.h
        ),
        (17.0, 47.0, 10.0, 6.0)
    );
}

/// The pattern runs along the stroke's whole length: a gap that a corner
/// interrupts is finished after it, and a mark that turns the corner is
/// two segments meeting there — ADR 0010's reason for not shipping dashes
/// that restart at every piece of a curve.
#[test]
fn a_dash_pattern_keeps_its_phase_across_a_corner() {
    let mut core = Core::new();
    // Centre lengths 4 and 6. The first piece is 7 long: a mark, then 3
    // of the gap's 6.
    let quads = frame(&mut core, 1.0, |ui| {
        ui.polyline(
            &[
                Vec2::new(10.0, 10.0),
                Vec2::new(17.0, 10.0),
                Vec2::new(17.0, 40.0),
            ],
            Stroke::new(2.0, Color::WHITE).dash(6.0, 4.0),
            NodeSpec::column(),
        );
    });
    let segs = segments(&quads);
    assert_eq!(segs[0].segment_ends(), [10.0, 10.0, 14.0, 10.0]);
    assert_eq!(segs[1].segment_ends(), [17.0, 13.0, 17.0, 17.0]);

    // The first piece is 2 long: half the mark before the corner, half
    // after, sharing the corner point.
    let quads = frame(&mut core, 1.0, |ui| {
        ui.polyline(
            &[
                Vec2::new(10.0, 10.0),
                Vec2::new(12.0, 10.0),
                Vec2::new(12.0, 40.0),
            ],
            Stroke::new(2.0, Color::WHITE).dash(6.0, 4.0),
            NodeSpec::column(),
        );
    });
    let segs = segments(&quads);
    assert_eq!(segs[0].segment_ends(), [10.0, 10.0, 12.0, 10.0]);
    assert_eq!(segs[1].segment_ends(), [12.0, 10.0, 12.0, 12.0]);
    assert_eq!(segs[2].segment_ends(), [12.0, 18.0, 12.0, 22.0]);

    // A curve's pieces are 6 px or less, shorter than the period: the
    // marks are still spaced by the pattern and not by the pieces.
    let quads = frame(&mut core, 1.0, |ui| {
        ui.polyline(
            &[
                Vec2::new(10.0, 100.0),
                Vec2::new(60.0, 60.0),
                Vec2::new(110.0, 100.0),
            ],
            Stroke::new(2.0, Color::WHITE).curve().dash(12.0, 8.0),
            NodeSpec::column(),
        );
    });
    let starts: Vec<[f32; 4]> = segments(&quads).iter().map(|s| s.segment_ends()).collect();
    let gaps = starts
        .windows(2)
        .filter(|w| (w[0][2], w[0][3]) != (w[1][0], w[1][1]))
        .count();
    // ~135 px of curve at a 20 px period.
    assert!((6..=7).contains(&gaps), "{gaps} gaps in {starts:?}");
}

/// A mark no longer than the stroke is wide is a dot; `dash_offset` moves
/// the marks towards the first point; a four-length pattern is a
/// dash-dot.
#[test]
fn dots_offsets_and_a_dash_dot() {
    let mut core = Core::new();
    let line = |core: &mut Core, stroke: Stroke| {
        let quads = frame(core, 1.0, |ui| {
            ui.line(
                Vec2::new(0.0, 50.0),
                Vec2::new(40.0, 50.0),
                stroke,
                NodeSpec::column(),
            );
        });
        segments(&quads)
            .iter()
            .map(|s| {
                let e = s.segment_ends();
                (e[0], e[2])
            })
            .collect::<Vec<_>>()
    };
    let white = |w| Stroke::new(w, Color::WHITE);
    // Dots 4 across, 10 apart.
    assert_eq!(
        line(&mut core, white(4.0).dash(4.0, 6.0)),
        [
            (0.0, 0.0),
            (10.0, 10.0),
            (20.0, 20.0),
            (30.0, 30.0),
            (40.0, 40.0)
        ]
    );
    // 3 px into a 4 + 6 pattern: the last of the first mark, then on.
    assert_eq!(
        line(&mut core, white(2.0).dash(6.0, 4.0).dash_offset(3.0)),
        [
            (0.0, 1.0),
            (7.0, 11.0),
            (17.0, 21.0),
            (27.0, 31.0),
            (37.0, 40.0)
        ]
    );
    // And it wraps, backwards too.
    assert_eq!(
        line(&mut core, white(2.0).dash(6.0, 4.0).dash_offset(-7.0)),
        line(&mut core, white(2.0).dash(6.0, 4.0).dash_offset(13.0)),
    );
    // A dash, a gap, a dot, a gap: centre lengths 8, 6, 0, 6.
    let dash_dot = kui_core::Dash::of(&[10.0, 4.0, 2.0, 4.0]).unwrap();
    assert_eq!(
        line(&mut core, white(2.0).dashed(dash_dot)),
        [(0.0, 8.0), (14.0, 14.0), (20.0, 28.0), (34.0, 34.0)]
    );
    assert!(kui_core::Dash::of(&[1.0, 2.0, 3.0]).is_none());
}

/// What is not a pattern draws solid: no gap, a length that is not a
/// number, a mark and its gap under a physical pixel together. And the lengths are logical —
/// the marks scale with the frame.
#[test]
fn a_pattern_with_no_gap_is_solid_and_the_lengths_are_logical() {
    let mut core = Core::new();
    let count = |core: &mut Core, scale: f32, stroke: Stroke| {
        let quads = frame(core, scale, |ui| {
            ui.line(
                Vec2::new(10.0, 50.0),
                Vec2::new(110.0, 50.0),
                stroke,
                NodeSpec::column(),
            );
        });
        segments(&quads).len()
    };
    let white = Stroke::new(2.0, Color::WHITE);
    assert_eq!(count(&mut core, 1.0, white.dash(6.0, 0.0)), 1);
    assert_eq!(count(&mut core, 1.0, white.dash(f32::NAN, 4.0)), 1);
    assert_eq!(count(&mut core, 1.0, white.dash(0.3, 0.3)), 1);
    assert!(white.dash(6.0, 0.0).dash.is_solid());
    assert!(!white.dash(6.0, 4.0).dash.is_solid());
    // 0.3 + 0.3 is over a pixel at a scale of 4.
    assert!(count(&mut core, 4.0, white.dash(0.3, 0.3)) > 1);

    let quads = frame(&mut core, 2.0, |ui| {
        ui.line(
            Vec2::new(10.0, 50.0),
            Vec2::new(110.0, 50.0),
            white.dash(6.0, 4.0),
            NodeSpec::column(),
        );
    });
    let segs = segments(&quads);
    assert_eq!(segs.len(), 10);
    assert_eq!(segs[1].segment_ends(), [40.0, 100.0, 48.0, 100.0]);
    assert_eq!(segs[1].border_w, 4.0);
}

/// A dashed stroke is hit along its whole length, gaps included: the
/// target is the stroke, not the ink.
#[test]
fn a_dashed_line_is_hit_in_its_gaps() {
    let mut core = Core::new();
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.line_keyed(
            "l",
            Vec2::new(10.0, 50.0),
            Vec2::new(110.0, 50.0),
            Stroke::new(4.0, Color::WHITE).dash(8.0, 12.0),
            NodeSpec::column().on_click("line"),
        );
    };
    frame(&mut core, 1.0, build);
    // x = 25 is in the first gap (8..20 past the start).
    core.handle_input(InputEvent::CursorMoved(Vec2::new(25.0, 50.0)));
    core.handle_input(InputEvent::MouseDown {
        button: MouseButton::Primary,
        clicks: 1,
    });
    let events = core.handle_input(InputEvent::MouseUp {
        button: MouseButton::Primary,
    });
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].payload, Value::str("line"));
}

/// A departing dashed stroke's ghost is dashed.
#[test]
fn a_dashed_line_leaves_dashed() {
    let mut core = Core::new();
    core.set_time(0.0);
    let mut ui = core.frame(VIEW, 1.0);
    ui.line_keyed(
        "l",
        Vec2::new(10.0, 50.0),
        Vec2::new(110.0, 50.0),
        Stroke::new(2.0, Color::WHITE).dash(6.0, 4.0),
        NodeSpec::column()
            .transition(100.0)
            .exit(kui_core::Enter::from(20.0, 0.0)),
    );
    ui.finish();
    core.set_time(0.05);
    let ui = core.frame(VIEW, 1.0);
    ui.finish();
    assert_eq!(segments(&core.output().0.quads).len(), 10);
}

/// RG116: a dashed line of no length is the dot its solid one is — a
/// connector being dragged out of a point was hit and not seen.
#[test]
fn a_dashed_line_of_no_length_is_a_dot() {
    let at = Vec2::new(40.0, 50.0);
    let mut core = Core::new();
    let solid = frame(&mut core, 1.0, |ui| {
        ui.line(at, at, Stroke::new(4.0, Color::WHITE), NodeSpec::column());
    });
    let dashed = frame(&mut core, 1.0, |ui| {
        ui.line(
            at,
            at,
            Stroke::new(4.0, Color::WHITE).dash(6.0, 4.0),
            NodeSpec::column(),
        );
    });
    assert_eq!(segments(&solid).len(), 1);
    assert_eq!(
        segments(&dashed)
            .iter()
            .map(|s| s.segment_ends())
            .collect::<Vec<_>>(),
        segments(&solid)
            .iter()
            .map(|s| s.segment_ends())
            .collect::<Vec<_>>()
    );
}
