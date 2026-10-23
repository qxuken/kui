//! A faded subtree drawn as one picture (backlog V11, ADR 0056): what the
//! core hands a renderer that asked for groups. The pixels are kui-wgpu's
//! (`tests/opacity_groups.rs` there); this pins the markers, the fades
//! inside them, and the groups undone into the fade per quad.

use kui_core::{Color, Core, FloatConfig, NodeSpec, Quad, QuadKind, Size, TextStyle, widgets};

const VIEW: Size = Size { w: 200.0, h: 100.0 };

fn frame(core: &mut Core, build: impl FnOnce(&mut kui_core::Ui<'_>)) -> Vec<Quad> {
    let mut ui = core.frame(VIEW, 2.0);
    ui.configure_root(NodeSpec::column().pad(20.0));
    build(&mut ui);
    ui.finish();
    core.output().0.quads.clone()
}

fn grouping() -> Core {
    let mut core = Core::new();
    core.set_opacity_groups(true);
    core
}

/// A dark card at `opacity` with a white label on it: the case that
/// washes out when each piece fades on its own.
fn card(ui: &mut kui_core::Ui<'_>, opacity: f32) {
    ui.with_keyed(
        "card",
        NodeSpec::column()
            .size(80.0, 30.0)
            .bg(Color::hex(0x202020ff))
            .opacity(opacity),
        |ui| ui.text("Go", TextStyle::new(14.0).color(Color::WHITE)),
    );
}

fn kinds(qs: &[Quad]) -> Vec<QuadKind> {
    qs.iter().map(|q| q.kind).collect()
}

#[test]
fn off_by_default_every_quad_fades_on_its_own() {
    let mut core = Core::new();
    assert!(!core.opacity_groups());
    let qs = frame(&mut core, |ui| card(ui, 0.5));
    assert!(
        !kinds(&qs).iter().any(|k| matches!(k, QuadKind::GroupBegin | QuadKind::GroupEnd)),
        "no markers for a renderer that did not ask"
    );
    assert!(qs.iter().all(|q| q.color.a <= 0.5 + 1e-6), "faded per quad");
}

#[test]
fn a_faded_subtree_is_one_group_faded_once() {
    let mut core = grouping();
    let qs = frame(&mut core, |ui| card(ui, 0.5));
    let b = qs.iter().position(|q| q.kind == QuadKind::GroupBegin).expect("a begin");
    let e = qs.iter().position(|q| q.kind == QuadKind::GroupEnd).expect("an end");
    assert!(b < e);
    assert_eq!((b, e), (0, qs.len() - 1), "around the card and its label");
    assert_eq!((qs[b].uv[0], qs[e].uv[0]), (e as u32, b as u32), "each names the other");
    assert_eq!(qs[e].color.a, 0.5, "the fade, once");
    assert_eq!(qs[b].color.a, 0.5);
    // Inside, nothing of the card's fade: the fill and the glyphs are
    // drawn whole into the picture.
    for q in &qs[b + 1..e] {
        assert_eq!(q.color.a, 1.0, "{:?} unfaded inside", q.kind);
    }
    // The bounds are the card's, in whole physical px.
    let r = qs[e].rect;
    assert_eq!((r.x, r.y, r.w, r.h), (40.0, 40.0, 160.0, 60.0));
    assert_eq!(qs[b].rect, r);
    assert_eq!(qs[e].clip, 0, "clipped by nothing: its pieces were");
}

#[test]
fn a_group_of_one_thing_is_undone_into_its_fade() {
    let mut core = grouping();
    let qs = frame(&mut core, |ui| {
        ui.leaf_keyed(
            "one",
            NodeSpec::column().size(40.0, 20.0).bg(Color::WHITE).opacity(0.4),
        );
    });
    assert!(
        !kinds(&qs).iter().any(|k| matches!(k, QuadKind::GroupBegin | QuadKind::GroupEnd)),
        "one quad fades the same on its own"
    );
    let drawn: Vec<&Quad> = qs.iter().filter(|q| q.rect.w > 0.0).collect();
    assert_eq!(drawn.len(), 1);
    assert!((drawn[0].color.a - 0.4).abs() < 1e-6, "the fade back on the quad");
    // The markers are left as nothing at all.
    assert!(qs.iter().filter(|q| q.rect.w <= 0.0).all(|q| q.color.a == 0.0));
}

#[test]
fn a_group_inside_a_group_carries_its_own_fade_relative_to_the_outer() {
    let mut core = grouping();
    let qs = frame(&mut core, |ui| {
        ui.with_keyed(
            "panel",
            NodeSpec::column().pad(4.0).bg(Color::hex(0xeeeeeeff)).opacity(0.5),
            |ui| card(ui, 0.8),
        );
    });
    let ends: Vec<usize> = qs
        .iter()
        .enumerate()
        .filter(|(_, q)| q.kind == QuadKind::GroupEnd)
        .map(|(i, _)| i)
        .collect();
    assert_eq!(ends.len(), 2, "the panel's and the card's");
    let (inner, outer) = (ends[0], ends[1]);
    assert!((qs[inner].color.a - 0.8).abs() < 1e-6, "the card's own, not 0.4");
    assert_eq!(qs[outer].color.a, 0.5);
    assert!(qs[outer].rect.w >= qs[inner].rect.w, "the outer holds the inner");
    for q in qs.iter().filter(|q| !matches!(q.kind, QuadKind::GroupBegin | QuadKind::GroupEnd)) {
        assert_eq!(q.color.a, 1.0, "{:?}: each picture fades it, not the quad", q.kind);
    }
}

#[test]
fn a_node_inside_a_group_fades_by_what_lies_between_it_and_the_group() {
    let mut core = grouping();
    let qs = frame(&mut core, |ui| {
        ui.with_keyed(
            "panel",
            NodeSpec::column().pad(4.0).bg(Color::hex(0xeeeeeeff)).opacity(0.5),
            |ui| {
                // A child at 0.5 of one quad: no group of its own, faded
                // by its own 0.5 inside the panel's picture, not 0.25.
                ui.leaf_keyed(
                    "dot",
                    NodeSpec::column().size(10.0, 10.0).bg(Color::BLACK).opacity(0.5),
                );
                ui.leaf_keyed("bar", NodeSpec::column().size(40.0, 4.0).bg(Color::BLACK));
            },
        );
    });
    assert_eq!(qs.iter().filter(|q| q.kind == QuadKind::GroupEnd).count(), 1);
    let solids: Vec<f32> = qs
        .iter()
        .filter(|q| q.kind == QuadKind::Solid && q.rect.w > 0.0)
        .map(|q| q.color.a)
        .collect();
    assert_eq!(solids, vec![1.0, 0.5, 1.0], "the panel, the dot, the bar");
}

#[test]
fn a_float_under_a_faded_node_fades_on_its_own_outside_the_group() {
    let mut core = grouping();
    let qs = frame(&mut core, |ui| {
        ui.with_keyed(
            "card",
            NodeSpec::column().size(80.0, 30.0).bg(Color::BLACK).opacity(0.5),
            |ui| {
                ui.leaf(NodeSpec::column().size(20.0, 10.0).bg(Color::WHITE));
                ui.leaf_keyed(
                    "tip",
                    NodeSpec::column()
                        .size(30.0, 10.0)
                        .bg(Color::hex(0xff0000ff))
                        .float(FloatConfig::below()),
                );
            },
        );
    });
    let e = qs.iter().position(|q| q.kind == QuadKind::GroupEnd).expect("the card's group");
    let tip = qs
        .iter()
        .position(|q| q.color.r == 1.0 && q.color.g == 0.0 && q.kind == QuadKind::Solid)
        .expect("the float");
    assert!(tip > e, "the float's layer paints after the group");
    assert_eq!(qs[tip].color.a, 0.5, "faded by the whole of its opacity, per quad");
}

#[test]
fn a_backdrop_blur_inside_undoes_the_group() {
    let mut core = grouping();
    let qs = frame(&mut core, |ui| {
        ui.with_keyed(
            "glass",
            NodeSpec::column()
                .size(80.0, 30.0)
                .bg(Color::hex(0xffffff40))
                .backdrop_blur(8.0)
                .opacity(0.5),
            |ui| ui.text("Go", TextStyle::new(14.0)),
        );
    });
    assert!(
        !kinds(&qs).iter().any(|k| matches!(k, QuadKind::GroupBegin | QuadKind::GroupEnd)),
        "a picture of its own has no frame under it to blur"
    );
    let b = qs.iter().find(|q| q.kind == QuadKind::Backdrop).expect("the blur");
    assert_eq!(b.color.a, 0.5, "faded as before");
}

#[test]
fn a_disabled_stock_button_is_a_group() {
    let mut core = grouping();
    let qs = frame(&mut core, |ui| {
        widgets::button_with(ui, "go", "Go", NodeSpec::default().disabled(true), None);
    });
    let e = qs.iter().position(|q| q.kind == QuadKind::GroupEnd).expect("a group");
    let o = core.theme().disabled_opacity;
    assert!((qs[e].color.a - o).abs() < 1e-6, "faded by the theme's disabled opacity");
}

#[test]
fn turning_the_switch_off_brings_the_per_quad_fade_back() {
    let mut core = grouping();
    let on = frame(&mut core, |ui| card(ui, 0.5));
    core.set_opacity_groups(false);
    let off = frame(&mut core, |ui| card(ui, 0.5));
    let drawn = |qs: &[Quad]| -> Vec<QuadKind> {
        qs.iter()
            .filter(|q| !matches!(q.kind, QuadKind::GroupBegin | QuadKind::GroupEnd))
            .map(|q| q.kind)
            .collect()
    };
    assert_eq!(drawn(&on), drawn(&off), "the same pieces either way");
    assert_eq!(on.len(), off.len() + 2);
}
