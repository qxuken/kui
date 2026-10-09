//! A node turns about its pivot (`docs/adr/0043-a-node-turns-about-its-pivot.md`):
//! `rotate` and `scale` on any node, paint-only, carried by the clip entry
//! its quads name. Layout is what it was; the entry's `transform` is what
//! the quads are drawn through, its `inner` clip is what a clipper inside
//! the turn cuts, a hit region is tested where it is drawn, the access
//! rect is the bounding box, and the turn tweens as one slot.

use std::f32::consts::TAU;

use kui_core::testing::{click_at, hover, kinds, solids};
use kui_core::{
    Clip, Color, Core, Easing, Enter, FloatConfig, Keyframe, NO_CLIP, NodeSpec, Quad, Rect, Size,
    Transform, Transition, Vec2,
};

const VIEW: Size = Size { w: 300.0, h: 200.0 };

fn frame(core: &mut Core, build: impl FnOnce(&mut kui_core::Ui<'_>)) {
    let mut ui = core.frame(VIEW, 1.0);
    build(&mut ui);
    ui.finish();
}

/// The solid quads of the last frame, each with the clip entry it names.
fn solids_with_clips(core: &mut Core) -> Vec<(Quad, Clip)> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == kui_core::QuadKind::Solid)
        .map(|q| (*q, dl.clip_of(q)))
        .collect()
}

fn near(a: f32, b: f32) -> bool {
    (a - b).abs() < 1e-3
}

fn card() -> NodeSpec {
    NodeSpec::column().size(100.0, 50.0).bg(Color::WHITE)
}

// -- The entry -------------------------------------------------------------

#[test]
fn a_turned_box_names_an_entry_with_its_transform_and_keeps_its_layout_rect() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed("card", card().rotate(0.25));
    });
    let qs = solids_with_clips(&mut core);
    assert_eq!(qs.len(), 1);
    let (q, clip) = qs[0];
    // Layout is untouched: the quad's rect is where the upright box is.
    assert_eq!(q.rect, Rect::new(20.0, 20.0, 100.0, 50.0));
    // The entry turns it a quarter turn about its centre, (70, 45).
    let want = Transform::about(Vec2::new(70.0, 45.0), 0.25, 1.0);
    assert!(
        near(clip.transform.angle, TAU / 4.0),
        "{:?}",
        clip.transform
    );
    assert!(near(clip.transform.scale, 1.0));
    assert!(near(clip.transform.tx, want.tx) && near(clip.transform.ty, want.ty));
    // Nothing clips it, inside the turn or out.
    assert_eq!(clip.rect, NO_CLIP);
    assert_eq!(clip.inner, NO_CLIP);
    assert!(clip.turned());
    // The corner lands where the turn says: the top-left of the box at
    // (20, 20) goes to (95, -5), the top-right of the drawn box.
    let c = clip.transform.apply(Vec2::new(20.0, 20.0));
    assert!(near(c.x, 95.0) && near(c.y, -5.0), "{c:?}");
}

#[test]
fn a_frame_with_no_turn_interns_the_entries_it_always_did() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed("card", card());
        ui.leaf_keyed("other", card().scale(1.0).pivot(0.0, 0.0));
    });
    let (dl, _) = core.output();
    assert_eq!(dl.clips.len(), 1, "entry zero and nothing else");
    assert_eq!(dl.clips[0], Clip::NONE);
    assert!(!dl.clips[0].turned());
}

#[test]
fn a_pivot_moves_what_stays_put() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed("card", card().rotate(0.25).pivot(0.0, 0.0));
    });
    let (_, clip) = solids_with_clips(&mut core)[0];
    // The top-left corner is the pivot: it does not move.
    let c = clip.transform.apply(Vec2::new(20.0, 20.0));
    assert!(near(c.x, 20.0) && near(c.y, 20.0), "{c:?}");
    // The top-right corner swings down below it.
    let r = clip.transform.apply(Vec2::new(120.0, 20.0));
    assert!(near(r.x, 20.0) && near(r.y, 120.0), "{r:?}");
}

#[test]
fn a_scale_is_about_the_pivot_too() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed("card", card().scale(2.0));
    });
    let (_, clip) = solids_with_clips(&mut core)[0];
    assert!(near(clip.transform.scale, 2.0) && near(clip.transform.angle, 0.0));
    // The centre stays, the corners move out by the box's half size.
    let c = clip.transform.apply(Vec2::new(70.0, 45.0));
    assert!(near(c.x, 70.0) && near(c.y, 45.0));
    let tl = clip.transform.apply(Vec2::new(20.0, 20.0));
    assert!(near(tl.x, -30.0) && near(tl.y, -5.0), "{tl:?}");
}

// -- The two clips -----------------------------------------------------------

/// A clipping box around a turned, rounded, clipping card that holds one
/// child filling it: the shape a photo inside a tilted card is.
fn clipped_card_with_child(ui: &mut kui_core::Ui<'_>, turn: f32) {
    ui.configure_root(NodeSpec::column().pad(20.0));
    ui.with_keyed(
        "scroller",
        NodeSpec::column().size(120.0, 120.0).clip(),
        |ui| {
            ui.with_keyed(
                "card",
                NodeSpec::column()
                    .size(100.0, 60.0)
                    .radius(8.0)
                    .clip()
                    .bg(Color::WHITE)
                    .rotate(turn),
                |ui| {
                    ui.leaf_keyed(
                        "photo",
                        NodeSpec::column().fill().bg(Color::rgb8(20, 20, 30)),
                    );
                },
            );
        },
    );
}

#[test]
fn a_clip_outside_the_turn_stays_outer_and_one_inside_it_is_inner() {
    let mut core = Core::new();
    frame(&mut core, |ui| clipped_card_with_child(ui, 0.1));
    let qs = solids_with_clips(&mut core);
    assert_eq!(qs.len(), 2, "the card and the photo");
    let (card, card_clip) = qs[0];
    let (photo, photo_clip) = qs[1];
    // The card is cut by the scroller, from outside the turn, in
    // framebuffer space; nothing inside the turn clips it yet.
    assert_eq!(card_clip.rect, Rect::new(20.0, 20.0, 120.0, 120.0));
    assert_eq!(card_clip.inner, NO_CLIP);
    assert!(near(card_clip.transform.angle, 0.1 * TAU));
    // The photo is cut by the card, from inside the turn, in the card's
    // own space — its rounded corners included — and by the scroller
    // from outside it, through the same transform.
    assert_eq!(photo.rect, card.rect);
    assert_eq!(
        photo_clip.rect, card_clip.rect,
        "the outer clip is inherited"
    );
    assert_eq!(
        photo_clip.inner, card.rect,
        "the inner clip is the card's box"
    );
    assert_eq!(photo_clip.inner_radius, [8.0; 4]);
    assert_eq!(photo_clip.transform, card_clip.transform);
}

#[test]
fn nested_turns_compose() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.with_keyed(
            "outer",
            NodeSpec::column().size(100.0, 100.0).rotate(0.25),
            |ui| {
                ui.leaf_keyed("inner", card().rotate(0.25));
            },
        );
    });
    let (_, clip) = solids_with_clips(&mut core)[0];
    assert!(
        near(clip.transform.angle, TAU / 2.0),
        "{:?}",
        clip.transform
    );
    // The inner box's centre, (70, 45), goes through its own quarter turn
    // (about itself: stays) and then the outer's about (70, 70): to (95, 70).
    let c = clip.transform.apply(Vec2::new(70.0, 45.0));
    assert!(near(c.x, 95.0) && near(c.y, 70.0), "{c:?}");
}

#[test]
fn a_clip_between_two_turns_is_its_bounding_box_in_the_inner_space() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.with_keyed(
            "outer",
            NodeSpec::column().size(100.0, 100.0).rotate(0.1).clip(),
            |ui| {
                ui.leaf_keyed("inner", card().rotate(0.1));
            },
        );
    });
    let (q, clip) = solids_with_clips(&mut core)[0];
    // The outer's clip was in the outer's space; pulled back through the
    // inner's own turn it is the bounding box of a turned square, bigger
    // than the square and square-cornered, holding the inner box.
    assert!(clip.inner != NO_CLIP);
    assert_eq!(clip.inner_radius, [0.0; 4]);
    assert!(
        clip.inner.w > 100.0 && clip.inner.h > 100.0,
        "{:?}",
        clip.inner
    );
    assert!(clip.inner.x < q.rect.x && clip.inner.y < q.rect.y);
}

#[test]
fn a_parent_float_turns_with_its_parent_and_a_viewport_float_does_not() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.with_keyed("card", card().rotate(0.25), |ui| {
            ui.leaf_keyed(
                "badge",
                NodeSpec::column()
                    .size(10.0, 10.0)
                    .bg(Color::WHITE)
                    .float(FloatConfig::parent()),
            );
            ui.leaf_keyed(
                "tooltip",
                NodeSpec::column()
                    .size(10.0, 10.0)
                    .bg(Color::WHITE)
                    .float(FloatConfig::viewport()),
            );
        });
    });
    let qs = solids_with_clips(&mut core);
    assert_eq!(qs.len(), 3);
    let (_, card_clip) = qs[0];
    let (_, badge_clip) = qs[1];
    let (_, tip_clip) = qs[2];
    assert_eq!(badge_clip.transform, card_clip.transform);
    assert_eq!(
        badge_clip.rect, NO_CLIP,
        "a float escapes the clip as before"
    );
    assert!(!tip_clip.turned());
}

// -- Hits, scrolling and the access rect ---------------------------------------

#[test]
fn a_turned_box_is_hit_where_it_is_drawn() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        frame(core, |ui| {
            ui.configure_root(NodeSpec::column().pad(20.0));
            ui.leaf_keyed("card", card().rotate(0.25).on_click("card"));
        });
    };
    build(&mut core);
    // Drawn as 50 wide and 100 tall about (70, 45): x 45..95, y -5..95.
    // Inside the drawn box but outside the layout rect (y 20..70).
    assert_eq!(kinds(&click_at(&mut core, 85.0, 10.0)), vec!["card"]);
    // Inside the layout rect but outside the drawn box.
    assert!(click_at(&mut core, 25.0, 25.0).is_empty());
    // Hover reads the same geometry.
    let key = core.key_of("card").unwrap();
    hover(&mut core, Vec2::new(85.0, 90.0));
    assert!(core.is_hovered(key));
    hover(&mut core, Vec2::new(115.0, 25.0));
    assert!(!core.is_hovered(key));
}

#[test]
fn a_rounded_turned_box_still_misses_its_corner() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed("card", card().radius(20.0).rotate(0.25).on_click("card"));
    });
    // The drawn box's top-left corner is the layout box's top-right
    // corner turned: (120, 20) → (95, -5)... just inside it, in the arc's
    // dead zone, misses; the middle of that edge hits.
    assert!(click_at(&mut core, 93.0, -3.0).is_empty());
    assert_eq!(kinds(&click_at(&mut core, 93.0, 45.0)), vec!["card"]);
}

#[test]
fn a_child_inside_a_turned_clipper_is_hit_inside_the_inner_clip_only() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.with_keyed(
            "card",
            NodeSpec::column().size(100.0, 50.0).clip().rotate(0.25),
            |ui| {
                // Wider than the card: its right half is cut.
                ui.leaf_keyed(
                    "wide",
                    NodeSpec::column()
                        .size(200.0, 50.0)
                        .bg(Color::WHITE)
                        .on_click("wide"),
                );
            },
        );
    });
    // In the card's space the child spans x 20..220; the card cuts it at
    // 120. A point of the child's at (70, 45) is in the card; one at
    // (170, 45) is past it. Both go through the card's turn to screen.
    let (_, clip) = solids_with_clips(&mut core)[0];
    let inside = clip.transform.apply(Vec2::new(70.0, 45.0));
    let past = clip.transform.apply(Vec2::new(170.0, 45.0));
    assert_eq!(
        kinds(&click_at(&mut core, inside.x, inside.y)),
        vec!["wide"]
    );
    assert!(click_at(&mut core, past.x, past.y).is_empty());
}

#[test]
fn a_scroller_inside_a_turn_takes_the_wheel_where_it_is_drawn() {
    let mut core = Core::new();
    let build = |core: &mut Core| {
        frame(core, |ui| {
            ui.configure_root(NodeSpec::column().pad(20.0));
            ui.with_keyed(
                "list",
                NodeSpec::column()
                    .size(100.0, 50.0)
                    .scroll_y()
                    .rotate(0.25)
                    .bg(Color::WHITE),
                |ui| {
                    for i in 0..20 {
                        ui.leaf_keyed(&format!("row{i}"), NodeSpec::column().size(100.0, 20.0));
                    }
                },
            );
        });
    };
    build(&mut core);
    let list = core.key_of("list").unwrap();
    // Over the drawn list (x 45..95, y -5..95), off the layout rect.
    hover(&mut core, Vec2::new(85.0, 85.0));
    core.handle_input(kui_core::InputEvent::Scroll(Vec2::new(0.0, -40.0)));
    build(&mut core);
    assert!(
        core.scroll_offset(list).y > 0.0,
        "{:?}",
        core.scroll_offset(list)
    );
    // Over the layout rect but off the drawn list: nothing to scroll.
    let before = core.scroll_offset(list);
    hover(&mut core, Vec2::new(25.0, 25.0));
    core.handle_input(kui_core::InputEvent::Scroll(Vec2::new(0.0, -40.0)));
    build(&mut core);
    assert_eq!(core.scroll_offset(list), before);
}

#[test]
fn the_access_rect_is_the_bounding_box_of_the_turned_box() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed("card", card().rotate(0.25).on_click("card").label("Card"));
    });
    let key = core.key_of("card").unwrap();
    let r = core.access_tree().get(key).expect("an access node").rect;
    assert!(near(r.x, 45.0) && near(r.y, -5.0), "{r:?}");
    assert!(near(r.w, 50.0) && near(r.h, 100.0), "{r:?}");
}

#[test]
fn the_region_s_rect_is_the_upright_one_and_its_turn_rides_beside_it() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed("card", card().rotate(0.25).on_click("card"));
    });
    let key = core.key_of("card").unwrap();
    let h = core
        .interaction
        .hits()
        .iter()
        .find(|h| h.key == key)
        .expect("a hit region");
    assert_eq!(h.rect, Rect::new(20.0, 20.0, 100.0, 50.0));
    let turn = h.turn.as_deref().expect("the region carries its turn");
    assert!(near(turn.transform.angle, TAU / 4.0));
    assert_eq!(turn.inner, NO_CLIP);
}

// -- The slot -------------------------------------------------------------------

#[test]
fn a_turn_tweens_with_transition() {
    let mut core = Core::new();
    let turn = |core: &mut Core, now: f64, turns: f32| {
        core.set_time(now);
        frame(core, |ui| {
            ui.configure_root(NodeSpec::column().pad(20.0));
            ui.leaf_keyed(
                "card",
                card()
                    .rotate(turns)
                    .transition_with(Transition::ms(100.0).easing(Easing::Linear)),
            );
        });
        solids_with_clips(core)[0].1.transform.angle
    };
    assert!(near(turn(&mut core, 0.0, 0.0), 0.0));
    assert!(
        near(turn(&mut core, 0.0, 0.5), 0.0),
        "a retarget starts from where it was"
    );
    assert!(core.animating());
    assert!(near(turn(&mut core, 0.05, 0.5), TAU / 4.0), "halfway");
    assert!(near(turn(&mut core, 0.1, 0.5), TAU / 2.0));
    assert!(!core.animating());
}

/// A turn that comes and goes as a prop does, the way C's zeroed field
/// and an omitted JSX or Lua prop say "none": it eases in from upright and
/// back out to it, as a declared 0 does, rather than snapping.
#[test]
fn a_turn_that_comes_and_goes_eases_from_and_to_upright() {
    let mut core = Core::new();
    let turn = |core: &mut Core, now: f64, turns: Option<f32>| {
        core.set_time(now);
        frame(core, |ui| {
            ui.configure_root(NodeSpec::column().pad(20.0));
            let mut spec = card().transition_with(Transition::ms(100.0).easing(Easing::Linear));
            if let Some(t) = turns {
                spec = spec.rotate(t);
            }
            ui.leaf_keyed("card", spec);
        });
        solids_with_clips(core)[0].1.transform.angle
    };
    assert!(near(turn(&mut core, 0.0, None), 0.0));
    assert!(
        near(turn(&mut core, 0.0, Some(0.25)), 0.0),
        "it starts upright"
    );
    assert!(core.animating());
    assert!(
        near(turn(&mut core, 0.05, Some(0.25)), TAU / 8.0),
        "halfway in"
    );
    assert!(near(turn(&mut core, 0.2, Some(0.25)), TAU / 4.0));
    assert!(!core.animating());
    assert!(
        near(turn(&mut core, 0.2, None), TAU / 4.0),
        "dropped, it starts where it was"
    );
    assert!(core.animating());
    assert!(near(turn(&mut core, 0.25, None), TAU / 8.0), "halfway out");
    assert!(near(turn(&mut core, 0.4, None), 0.0));
    assert!(!core.animating(), "and rests upright");
    // A node new to the frame with a turn still appears turned.
    let mut fresh = Core::new();
    assert!(near(turn(&mut fresh, 0.0, Some(0.25)), TAU / 4.0));
}

/// A turn or a scale that is not a finite number — a NaN from a binding's
/// raw field — is none: it draws upright and never reaches a tween.
#[test]
fn a_turn_or_scale_that_is_not_a_number_is_none() {
    let mut core = Core::new();
    core.set_time(0.0);
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.leaf_keyed(
            "card",
            card()
                .rotate(f32::NAN)
                .scale(f32::INFINITY)
                .transition(100.0)
                .enter(Enter::default().scale(f32::NAN)),
        );
    });
    let (q, clip) = solids_with_clips(&mut core)[0];
    assert!(q.rect.x.is_finite() && q.rect.w.is_finite());
    assert!(near(clip.transform.angle, 0.0) && near(clip.transform.scale, 1.0));
    assert!(!core.animating());
    let tree = core.access_tree();
    assert!(
        tree.nodes
            .iter()
            .all(|n| n.rect.x.is_finite() && n.rect.w.is_finite())
    );
}

#[test]
fn an_entrance_scales_in_and_a_stop_naming_only_rotate_keeps_the_scale() {
    let mut core = Core::new();
    let entering = |core: &mut Core, now: f64| {
        core.set_time(now);
        frame(core, |ui| {
            ui.configure_root(NodeSpec::column().pad(20.0));
            ui.leaf_keyed(
                "chip",
                card()
                    .transition_with(Transition::ms(100.0).easing(Easing::Linear))
                    .enter(Enter::default().scale(0.5)),
            );
        });
        solids_with_clips(core)[0].1.transform.scale
    };
    assert!(near(entering(&mut core, 0.0), 0.5));
    assert!(near(entering(&mut core, 0.05), 0.75));
    assert!(near(entering(&mut core, 0.1), 1.0));

    let mut core = Core::new();
    let spinning = |core: &mut Core, now: f64| {
        core.set_time(now);
        frame(core, |ui| {
            ui.configure_root(NodeSpec::column().pad(20.0));
            ui.leaf_keyed(
                "spinner",
                card()
                    .scale(2.0)
                    .transition_with(Transition::ms(1000.0).easing(Easing::Linear))
                    .keyframes(vec![
                        Keyframe::default().rotate(0.0),
                        Keyframe::default().rotate(1.0),
                    ]),
            );
        });
        solids_with_clips(core)[0].1.transform
    };
    let t = spinning(&mut core, 0.25);
    assert!(near(t.angle, TAU / 4.0), "{t:?}");
    assert!(
        near(t.scale, 2.0),
        "the scale the stops did not name: {t:?}"
    );
    assert!(core.animating(), "a cycle owes a frame");
}

#[test]
fn a_ghost_keeps_its_turn_and_eases_toward_the_exit() {
    let mut core = Core::new();
    let frame_at = |core: &mut Core, now: f64, show: bool| {
        core.set_time(now);
        frame(core, |ui| {
            ui.configure_root(NodeSpec::column().pad(20.0));
            if show {
                ui.leaf_keyed(
                    "card",
                    card()
                        .rotate(0.25)
                        .transition_with(Transition::ms(100.0).easing(Easing::Linear))
                        .exit(Enter::default().rotate(0.5).scale(0.0)),
                );
            }
        });
    };
    frame_at(&mut core, 0.0, true);
    frame_at(&mut core, 0.0, false);
    let ghost = solids_with_clips(&mut core);
    assert_eq!(ghost.len(), 1, "the ghost, where the card was");
    assert!(
        near(ghost[0].1.transform.angle, TAU / 4.0),
        "{:?}",
        ghost[0].1.transform
    );
    frame_at(&mut core, 0.05, false);
    let mid = solids_with_clips(&mut core)[0].1.transform;
    assert!(near(mid.angle, 0.375 * TAU), "{mid:?}");
    assert!(near(mid.scale, 0.5), "{mid:?}");
    frame_at(&mut core, 0.1, false);
    assert!(solids(&mut core).is_empty());
}

#[test]
fn a_path_s_own_turn_composes_under_the_node_s() {
    let mut core = Core::new();
    frame(&mut core, |ui| {
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.with_keyed(
            "card",
            NodeSpec::column().size(100.0, 100.0).rotate(0.25),
            |ui| {
                ui.path(
                    &kui_core::Path::parse("M10 10 H50 V30 H10 Z")
                        .unwrap()
                        .rotated(0.25),
                    NodeSpec::column().bg(Color::WHITE),
                );
            },
        );
    });
    let (dl, _) = core.output();
    let mask = dl
        .quads
        .iter()
        .find(|q| q.kind == kui_core::QuadKind::GlyphMask)
        .expect("the path's mask");
    // The path's own quarter turn rides in the quad (ADR 0041) and the
    // card's in the entry (ADR 0043); the shader applies the first, then
    // the second.
    assert!(near(mask.blur, TAU / 4.0), "{}", mask.blur);
    assert!(near(dl.clip_of(mask).transform.angle, TAU / 4.0));
}
