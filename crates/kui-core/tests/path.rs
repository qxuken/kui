//! The `path` element through the core (ADR 0040): a pie whose wedges are
//! hit by their arcs, the mask quads on the wire and the slots they keep,
//! the two roads to a texture of its own, the warning a bad `d` raises,
//! and a ghost that keeps its masks.

use kui_core::testing::{click_at, kinds};
use kui_core::{
    Color, Core, Enter, FillRule, InputEvent, NodeSpec, Path, QuadKind, Size, Stroke, Vec2,
};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn frame(core: &mut Core, build: impl FnOnce(&mut kui_core::Ui<'_>)) {
    frame_at(core, 1.0, build);
}

fn frame_at(core: &mut Core, scale: f32, build: impl FnOnce(&mut kui_core::Ui<'_>)) {
    let mut ui = core.frame(VIEW, scale);
    build(&mut ui);
    ui.finish();
}

fn tag(events: &[kui_core::UiEvent]) -> Vec<String> {
    events
        .iter()
        .filter_map(|e| e.payload.as_str().map(str::to_string))
        .collect()
}

fn masks(core: &mut Core) -> Vec<kui_core::Quad> {
    core.output()
        .0
        .quads
        .iter()
        .filter(|q| q.kind == QuadKind::GlyphMask)
        .copied()
        .collect()
}

/// The case the ADR opens with: a pie's wedges, round, and a press in
/// one wedge's bounding box past its arc is the neighbour's or nothing.
#[test]
fn a_pies_wedges_are_hit_by_their_arcs() {
    let mut core = Core::new();
    let (cx, cy, r) = (100.0, 100.0, 80.0);
    frame(&mut core, |ui| {
        let canvas = NodeSpec::column()
            .fill()
            .on_click("canvas")
            .role(kui_core::Role::Group);
        ui.with(canvas, |ui| {
            // East to south, then south to west: both bounding boxes hold
            // the centre's quadrant corners.
            ui.path_keyed(
                "a",
                &Path::sector(cx, cy, r, 0.0, 0.0, 0.25),
                NodeSpec::column().bg(Color::WHITE).on_click("a").label("A"),
            );
            ui.path_keyed(
                "b",
                &Path::sector(cx, cy, r, 0.0, 0.25, 0.25),
                NodeSpec::column().bg(Color::WHITE).on_click("b").label("B"),
            );
        });
    });
    // Halfway out along each wedge's middle.
    assert_eq!(tag(&click_at(&mut core, 140.0, 140.0)), ["a"]);
    assert_eq!(tag(&click_at(&mut core, 60.0, 140.0)), ["b"]);
    // In A's bounding box, past its arc: the canvas, not A.
    assert_eq!(tag(&click_at(&mut core, 175.0, 175.0)), ["canvas"]);
    // Two buttons to assistive technology.
    let names: Vec<_> = core
        .access_tree()
        .nodes
        .iter()
        .filter_map(|n| n.name.clone())
        .collect();
    assert_eq!(names, ["A", "B"]);
}

/// A wedge is one glyph-mask quad tinted with its fill; hovered, the
/// next frame paints `hover_bg` on the same slot — nothing re-rasterizes
/// for a colour.
#[test]
fn a_fill_is_one_mask_quad_and_a_hover_recolours_it() {
    let mut core = Core::new();
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.path_keyed(
                "w",
                &Path::sector(100.0, 100.0, 60.0, 0.0, 0.0, 0.3),
                NodeSpec::column()
                    .bg(Color::WHITE)
                    .hover_bg(Color::BLACK)
                    .on_hover("w"),
            );
        });
    };
    frame(&mut core, build);
    let first = masks(&mut core);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].color, Color::WHITE);
    assert_eq!(core.path_texture_count(), 0, "it fits the atlas");
    let evs = core.handle_input(InputEvent::CursorMoved(Vec2::new(130.0, 120.0)));
    assert_eq!(kinds(&evs), ["hover"]);
    frame(&mut core, build);
    let second = masks(&mut core);
    assert_eq!(second.len(), 1);
    assert_eq!(second[0].color, Color::BLACK);
    assert_eq!(second[0].uv, first[0].uv, "the same slot, re-tinted");
    assert_eq!(second[0].rect, first[0].rect);
}

/// Fill and stroke are two masks of one node, the stroke over the fill in
/// the border's colour; a stroke alone is hit as a line is.
#[test]
fn a_stroke_is_a_second_mask_and_hits_by_its_width() {
    let mut core = Core::new();
    let square = Path::parse("M50 50 H150 V150 H50 Z").unwrap();
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("canvas"), |ui| {
            ui.path(
                &square
                    .clone()
                    .stroked(Stroke::new(4.0, Color::hex(0xff0000ff))),
                NodeSpec::column().bg(Color::WHITE),
            );
            ui.path_keyed(
                "ring",
                &Path::parse("M200 50 H280 V150 H200 Z")
                    .unwrap()
                    .stroked(Stroke::new(2.0, Color::hex(0x00ff00ff))),
                NodeSpec::column().on_click("ring"),
            );
        });
    });
    let m = masks(&mut core);
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].color, Color::WHITE);
    assert_eq!(m[1].color, Color::hex(0xff0000ff));
    assert_eq!(m[2].color, Color::hex(0x00ff00ff));
    // The outline is the target, the inside is not.
    assert_eq!(tag(&click_at(&mut core, 200.0, 100.0)), ["ring"]);
    assert_eq!(tag(&click_at(&mut core, 240.0, 100.0)), ["canvas"]);
}

/// Even-odd leaves a ring's hole open to the press beneath; nonzero over
/// the same two contours (wound the same way) fills it.
#[test]
fn the_fill_rule_decides_the_hole() {
    let mut core = Core::new();
    let d = "M20 20 H180 V180 H20 Z M60 60 H140 V140 H60 Z";
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("canvas"), |ui| {
            ui.path_keyed(
                "ring",
                &Path::parse(d).unwrap().fill_rule(FillRule::EvenOdd),
                NodeSpec::column().bg(Color::WHITE).on_click("ring"),
            );
        });
    });
    assert_eq!(tag(&click_at(&mut core, 100.0, 100.0)), ["canvas"]);
    assert_eq!(tag(&click_at(&mut core, 40.0, 100.0)), ["ring"]);
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("canvas"), |ui| {
            ui.path_keyed(
                "disc",
                &Path::parse(d).unwrap(),
                NodeSpec::column().bg(Color::WHITE).on_click("disc"),
            );
        });
    });
    assert_eq!(tag(&click_at(&mut core, 100.0, 100.0)), ["disc"]);
}

/// A `d` that does not parse raises the warning, once, and draws nothing.
#[test]
fn a_malformed_d_warns_and_draws_nothing() {
    let mut core = Core::new();
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.core().path_d_node_keyed(
                "bad",
                "M10 10 L20",
                FillRule::NonZero,
                None,
                None,
                NodeSpec::column().bg(Color::WHITE),
            );
        });
    };
    frame(&mut core, build);
    assert!(masks(&mut core).is_empty());
    let w = core.take_warnings();
    assert_eq!(w.len(), 1);
    assert_eq!(w[0].code, "path-malformed");
    assert!(w[0].message.contains("byte 10"), "{}", w[0].message);
    frame(&mut core, build);
    assert!(core.take_warnings().is_empty(), "once per key");
}

/// A mask a quarter of the biggest page or more is drawn from a texture
/// of its own, as a `texture` quad; one past what a texture holds draws
/// nothing and says so.
#[test]
fn a_big_mask_takes_a_texture_and_a_huge_one_warns() {
    let mut core = Core::new();
    let big = Path::parse("M0 0 H2100 V2100 H0 Z").unwrap();
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.path_keyed("big", &big, NodeSpec::column().bg(Color::WHITE));
        });
    };
    frame(&mut core, build);
    let dl = core.output().0;
    assert_eq!(
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::Texture)
            .count(),
        1
    );
    assert_eq!(dl.textures.len(), 1);
    assert_eq!(dl.texture_pixels[0].width, 2105);
    assert_eq!(core.path_texture_count(), 1);
    // Still, the next frame draws the same texture: nothing is dropped.
    frame(&mut core, build);
    assert!(core.output().0.dropped_textures.is_empty());
    assert_eq!(core.path_texture_count(), 1);
    // Gone from the view: the frame after drops it.
    frame(&mut core, |_| {});
    frame(&mut core, |_| {});
    assert_eq!(core.output().0.dropped_textures.len(), 1);
    assert_eq!(core.path_texture_count(), 0);

    let huge = Path::parse("M0 0 H9000 V10 H0 Z").unwrap();
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.path_keyed("huge", &huge, NodeSpec::column().bg(Color::WHITE));
        });
    });
    let w = core.take_warnings();
    assert_eq!(
        w.iter().map(|w| w.code).collect::<Vec<_>>(),
        ["path-too-large"]
    );
    assert!(
        core.output()
            .0
            .quads
            .iter()
            .all(|q| q.kind != QuadKind::Texture)
    );
}

/// A path whose ops change twice within a few frames leaves the atlas for a
/// texture of its own and stays there once still again.
#[test]
fn an_animating_path_leaves_the_atlas_and_stays_out() {
    let mut core = Core::new();
    let wedge = |sweep: f32| Path::sector(100.0, 100.0, 60.0, 0.0, 0.0, sweep);
    let show = |core: &mut Core, sweep: f32| {
        frame(core, |ui| {
            ui.with(NodeSpec::column().fill(), |ui| {
                ui.path_keyed("w", &wedge(sweep), NodeSpec::column().bg(Color::WHITE));
            });
        });
    };
    show(&mut core, 0.2);
    assert_eq!(masks(&mut core).len(), 1);
    // One change is a new shape, not an animation.
    show(&mut core, 0.21);
    assert_eq!(masks(&mut core).len(), 1);
    assert_eq!(core.path_texture_count(), 0);
    // A second change on the next frame is.
    show(&mut core, 0.22);
    assert!(masks(&mut core).is_empty());
    assert_eq!(core.path_texture_count(), 1);
    // And still again, it stays texture-backed.
    show(&mut core, 0.22);
    show(&mut core, 0.22);
    assert!(masks(&mut core).is_empty());
    assert_eq!(core.path_texture_count(), 1);
    let dl = core.output().0;
    assert_eq!(
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::Texture)
            .count(),
        1
    );
}

/// At scale 2 the mask is rasterized at scale 2 — twice the texels, on
/// whole physical pixels — and the quad is placed on them.
#[test]
fn the_mask_follows_the_scale() {
    let mut core = Core::new();
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.path(
                &Path::parse("M10.5 10 H50.5 V50 H10.5 Z").unwrap(),
                NodeSpec::column().bg(Color::WHITE),
            );
        });
    };
    frame_at(&mut core, 1.0, build);
    let one = masks(&mut core)[0];
    frame_at(&mut core, 2.0, build);
    let two = masks(&mut core)[0];
    assert_eq!(one.rect.x.fract(), 0.0);
    assert_eq!(two.rect.x.fract(), 0.0);
    assert!(
        two.uv[2] >= one.uv[2] * 2 - 2,
        "{:?} vs {:?}",
        one.uv,
        two.uv
    );
    assert_ne!(one.uv, two.uv, "two slots, one per scale");
}

/// A departing path keeps painting its masks while it fades: the ghost
/// carries the ops and the hash.
#[test]
fn a_ghost_keeps_its_masks() {
    let mut core = Core::new();
    core.set_time(0.0);
    let show = |core: &mut Core, on: bool| {
        frame(core, |ui| {
            ui.with(NodeSpec::column().fill(), |ui| {
                if on {
                    ui.path_keyed(
                        "w",
                        &Path::sector(100.0, 100.0, 60.0, 0.0, 0.0, 0.3),
                        NodeSpec::column()
                            .bg(Color::WHITE)
                            .transition(400.0)
                            .exit(Enter::default().opacity(0.0)),
                    );
                }
            });
        });
    };
    show(&mut core, true);
    let live = masks(&mut core);
    assert_eq!(live.len(), 1);
    core.set_time(0.1);
    show(&mut core, false);
    core.set_time(0.3);
    show(&mut core, false);
    let ghost = masks(&mut core);
    assert_eq!(ghost.len(), 1, "the ghost still draws");
    assert_eq!(ghost[0].uv, live[0].uv, "from the same slot");
    assert!(ghost[0].color.a < 1.0, "faded");
    core.set_time(1.0);
    show(&mut core, false);
    assert!(masks(&mut core).is_empty(), "and then it is gone");
}

/// An open stroked curve is hit where it is drawn and not along the
/// chord from its end back to its start; a closed one is hit on the
/// edge its `Z` drew.
#[test]
fn an_open_stroke_is_not_hit_along_its_chord() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("canvas"), |ui| {
            ui.path_keyed(
                "open",
                &Path::parse("M50 50 L150 50 L150 150")
                    .unwrap()
                    .stroked(Stroke::new(2.0, Color::WHITE)),
                NodeSpec::column().on_click("open"),
            );
            ui.path_keyed(
                "closed",
                &Path::parse("M180 50 L280 50 L280 150 Z")
                    .unwrap()
                    .stroked(Stroke::new(2.0, Color::WHITE)),
                NodeSpec::column().on_click("closed"),
            );
        });
    });
    assert_eq!(tag(&click_at(&mut core, 100.0, 50.0)), ["open"]);
    assert_eq!(tag(&click_at(&mut core, 150.0, 100.0)), ["open"]);
    assert_eq!(tag(&click_at(&mut core, 100.0, 100.0)), ["canvas"]);
    assert_eq!(tag(&click_at(&mut core, 230.0, 100.0)), ["closed"]);
}

/// A shape that changes every other frame is animating as surely as one
/// that changes every frame; two changes far apart are two shapes.
#[test]
fn changes_a_frame_apart_are_an_animation_and_far_apart_are_not() {
    let wedge = |sweep: f32| Path::sector(100.0, 100.0, 60.0, 0.0, 0.0, sweep);
    let show = |core: &mut Core, sweep: f32| {
        frame(core, |ui| {
            ui.with(NodeSpec::column().fill(), |ui| {
                ui.path_keyed("w", &wedge(sweep), NodeSpec::column().bg(Color::WHITE));
            });
        });
    };
    let mut core = Core::new();
    show(&mut core, 0.2);
    show(&mut core, 0.21);
    show(&mut core, 0.21);
    assert_eq!(core.path_texture_count(), 0);
    show(&mut core, 0.22);
    assert_eq!(core.path_texture_count(), 1, "a frame between two changes");

    let mut core = Core::new();
    show(&mut core, 0.2);
    show(&mut core, 0.21);
    for _ in 0..kui_core::path::ANIMATING_WINDOW {
        show(&mut core, 0.21);
    }
    show(&mut core, 0.22);
    assert_eq!(core.path_texture_count(), 0, "two shapes, not a motion");
    assert_eq!(masks(&mut core).len(), 1);
}

/// A `d` handed over every frame draws what it drew, and a changed
/// string under the same key draws the new shape.
#[test]
fn a_d_string_is_kept_until_it_changes() {
    let mut core = Core::new();
    let show = |core: &mut Core, d: &str| {
        frame(core, |ui| {
            ui.with(NodeSpec::column().fill(), |ui| {
                ui.path_d_keyed(
                    "p",
                    d,
                    FillRule::NonZero,
                    None,
                    None,
                    NodeSpec::column().bg(Color::WHITE),
                );
            });
        });
    };
    show(&mut core, "M10 10 H50 V50 H10 Z");
    let first = masks(&mut core);
    show(&mut core, "M10 10 H50 V50 H10 Z");
    let again = masks(&mut core);
    assert_eq!(again.len(), 1);
    assert_eq!(again[0].uv, first[0].uv);
    assert_eq!(again[0].rect, first[0].rect);
    show(&mut core, "M10 10 H90 V50 H10 Z");
    let wide = masks(&mut core);
    assert_eq!(wide.len(), 1);
    assert!(wide[0].rect.w > first[0].rect.w);
    // A string that stops parsing warns and draws nothing, kept ops or not.
    show(&mut core, "M10 10 H");
    assert!(masks(&mut core).is_empty());
    assert_eq!(core.take_warnings().len(), 1);
}

/// A turning path is one mask (ADR 0041): the same slot at every angle,
/// never animating, the angle in the quad's `blur`, the quad a square
/// on whole pixels about the pivot.
#[test]
fn a_turning_path_keeps_its_mask_and_carries_the_angle() {
    let mut core = Core::new();
    // An arc of a circle about (100, 100): its own box's centre is not
    // the pivot.
    let arc = |turns: f32| {
        Path::new()
            .move_to(130.0, 100.0)
            .arc_to(30.0, 30.0, 0.0, false, true, 100.0, 130.0)
            .stroked(Stroke::new(4.0, Color::WHITE))
            .pivot(100.0, 100.0)
            .rotated(turns)
    };
    let show = |core: &mut Core, turns: f32| {
        frame(core, |ui| {
            ui.with(NodeSpec::column().fill(), |ui| {
                ui.path_keyed("arc", &arc(turns), NodeSpec::column());
            });
        });
    };
    show(&mut core, 0.0);
    let first = masks(&mut core);
    assert_eq!(first.len(), 1);
    assert_eq!(first[0].blur, 0.0);
    let mut seen = Vec::new();
    for i in 1..=12 {
        let turns = i as f32 / 12.0;
        show(&mut core, turns);
        let m = masks(&mut core);
        assert_eq!(m.len(), 1, "still an atlas mask at {turns}");
        assert_eq!(m[0].uv, first[0].uv, "the same slot");
        assert_eq!(m[0].rect, first[0].rect, "the same quad");
        assert!((m[0].blur - turns * std::f32::consts::TAU).abs() < 1e-5);
        seen.push(m[0].blur);
    }
    assert_eq!(core.path_texture_count(), 0, "turning is not animating");
    // The quad is a square about the pivot, on whole pixels.
    let r = first[0].rect;
    assert_eq!(r.w, r.h);
    assert_eq!((r.x.fract(), r.y.fract()), (0.0, 0.0));
    assert_eq!((r.x + r.w * 0.5, r.y + r.h * 0.5), (100.0, 100.0));
}

/// A turned path is hit where it is drawn, not where its ops say.
#[test]
fn a_turned_path_is_hit_where_it_is_drawn() {
    let mut core = Core::new();
    // A bar to the east of the pivot, turned a quarter: it points south.
    let bar = Path::parse("M110 95 H160 V105 H110 Z")
        .unwrap()
        .pivot(100.0, 100.0)
        .rotated(0.25);
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill().on_click("canvas"), |ui| {
            ui.path_keyed(
                "bar",
                &bar,
                NodeSpec::column().bg(Color::WHITE).on_click("bar"),
            );
        });
    });
    assert_eq!(tag(&click_at(&mut core, 100.0, 135.0)), ["bar"]);
    assert_eq!(tag(&click_at(&mut core, 135.0, 100.0)), ["canvas"]);
}

/// Without a pivot the path turns about the centre of its outline's box.
#[test]
fn the_default_pivot_is_the_centre_of_the_box() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.with(NodeSpec::column().fill(), |ui| {
            ui.path(
                &Path::parse("M40 60 H80 V70 H40 Z").unwrap().rotated(0.1),
                NodeSpec::column().bg(Color::WHITE),
            );
        });
    });
    let r = masks(&mut core)[0].rect;
    assert_eq!((r.x + r.w * 0.5, r.y + r.h * 0.5), (60.0, 65.0));
}
