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
                NodeSpec::column()
                    .bg(Color::WHITE)
                    .on_click("a")
                    .label("A"),
            );
            ui.path_keyed(
                "b",
                &Path::sector(cx, cy, r, 0.0, 0.25, 0.25),
                NodeSpec::column()
                    .bg(Color::WHITE)
                    .on_click("b")
                    .label("B"),
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
    assert_eq!(dl.quads.iter().filter(|q| q.kind == QuadKind::Texture).count(), 1);
    assert_eq!(dl.textures.len(), 1);
    assert_eq!(dl.texture_pixels[0].width, 2103);
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
    assert_eq!(w.iter().map(|w| w.code).collect::<Vec<_>>(), ["path-too-large"]);
    assert!(core.output().0.quads.iter().all(|q| q.kind != QuadKind::Texture));
}

/// A path whose ops change two frames running leaves the atlas for a
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
    assert_eq!(dl.quads.iter().filter(|q| q.kind == QuadKind::Texture).count(), 1);
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
    assert!(two.uv[2] >= one.uv[2] * 2 - 2, "{:?} vs {:?}", one.uv, two.uv);
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
