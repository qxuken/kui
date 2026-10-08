//! A node's backdrop blur (backlog F129): one `QuadKind::Backdrop` quad
//! before everything the node paints, carrying the region's shape — the
//! node's rect, radii and clip — the radius in physical px and the group
//! opacity. The core paints nothing for it; this pins what a renderer is
//! handed, which is all a headless frame has.

use kui_core::{Color, Core, NodeSpec, Quad, QuadKind, Size};

const VIEW: Size = Size { w: 200.0, h: 100.0 };

fn frame(core: &mut Core, scale: f32, build: impl FnOnce(&mut kui_core::Ui<'_>)) -> Vec<Quad> {
    let mut ui = core.frame(VIEW, scale);
    ui.configure_root(NodeSpec::column().pad(20.0));
    build(&mut ui);
    ui.finish();
    core.output().0.quads.clone()
}

fn glass() -> NodeSpec {
    NodeSpec::column()
        .size(80.0, 40.0)
        .radius(6.0)
        .bg(Color::hex(0xffffff40))
        .backdrop_blur(12.0)
}

#[test]
fn the_blur_comes_before_the_node_s_own_paint_in_physical_px() {
    let mut core = Core::new();
    let qs = frame(&mut core, 2.0, |ui| {
        ui.leaf_keyed("glass", glass());
    });
    let i = qs
        .iter()
        .position(|q| q.kind == QuadKind::Backdrop)
        .expect("a backdrop quad");
    let b = &qs[i];
    let bg = &qs[i + 1];
    assert_eq!(bg.kind, QuadKind::Solid, "the bg paints over the blur");
    assert_eq!(b.rect, bg.rect, "the same box");
    assert_eq!(
        (b.rect.x, b.rect.y, b.rect.w, b.rect.h),
        (40.0, 40.0, 160.0, 80.0)
    );
    assert_eq!(b.radius, [12.0; 4], "6 logical px at scale 2");
    assert_eq!(b.blur, 24.0, "12 logical px at scale 2");
    assert_eq!(
        (b.color.r, b.color.g, b.color.b, b.color.a),
        (0.0, 0.0, 0.0, 1.0),
        "rgb zero, the opacity in alpha"
    );
    assert_eq!(
        qs.iter().filter(|q| q.kind == QuadKind::Backdrop).count(),
        1
    );
}

#[test]
fn no_radius_no_quad() {
    let mut core = Core::new();
    let none = frame(&mut core, 1.0, |ui| {
        ui.leaf_keyed(
            "plain",
            NodeSpec::column().size(80.0, 40.0).bg(Color::WHITE),
        );
        ui.leaf_keyed(
            "zero",
            NodeSpec::column().size(80.0, 40.0).backdrop_blur(0.0),
        );
        ui.leaf_keyed(
            "negative",
            NodeSpec::column().size(80.0, 40.0).backdrop_blur(-4.0),
        );
    });
    assert!(none.iter().all(|q| q.kind != QuadKind::Backdrop));
    assert_eq!(
        NodeSpec::column()
            .backdrop_blur(-4.0)
            .interact()
            .backdrop_blur,
        0.0
    );
}

#[test]
fn a_faded_group_fades_the_blur_and_a_clip_shapes_it() {
    let mut core = Core::new();
    let qs = frame(&mut core, 1.0, |ui| {
        ui.with_keyed(
            "panel",
            NodeSpec::column()
                .size(60.0, 30.0)
                .radius(10.0)
                .clip()
                .opacity(0.5),
            |ui| {
                ui.leaf_keyed("glass", glass());
            },
        );
    });
    let b = qs
        .iter()
        .find(|q| q.kind == QuadKind::Backdrop)
        .expect("a backdrop quad");
    assert!((b.color.a - 0.5).abs() < 1e-6, "{}", b.color.a);
    let clip = core.output().0.clip_of(b);
    assert_eq!(
        (clip.rect.x, clip.rect.y, clip.rect.w, clip.rect.h),
        (20.0, 20.0, 60.0, 30.0),
        "the panel clips it"
    );
    assert_eq!(clip.radius, [10.0; 4], "rounded as the panel is");
}

#[test]
fn the_blur_lies_under_what_was_drawn_after_and_over_what_was_drawn_before() {
    let mut core = Core::new();
    let qs = frame(&mut core, 1.0, |ui| {
        ui.with(NodeSpec::column().bg(Color::hex(0x202020ff)), |ui| {
            ui.leaf_keyed("glass", glass());
            ui.leaf_keyed("after", NodeSpec::row().size(10.0, 10.0).bg(Color::WHITE));
        });
    });
    let kinds: Vec<QuadKind> = qs.iter().map(|q| q.kind).collect();
    let b = kinds.iter().position(|k| *k == QuadKind::Backdrop).unwrap();
    assert!(b >= 1, "the parent's bg is under it: {kinds:?}");
    assert_eq!(
        kinds.len(),
        b + 3,
        "the glass's bg, then the sibling: {kinds:?}"
    );
}

#[test]
fn an_opaque_background_over_the_blur_is_a_warning() {
    let mut core = Core::new();
    frame(&mut core, 1.0, |ui| {
        ui.leaf_keyed(
            "hidden",
            NodeSpec::column()
                .size(80.0, 40.0)
                .bg(Color::WHITE)
                .backdrop_blur(8.0),
        );
        ui.leaf_keyed("frosted", glass());
    });
    let warnings = core.take_warnings();
    let hidden: Vec<_> = warnings
        .iter()
        .filter(|w| w.code == kui_core::diag::BACKDROP_BLUR_HIDDEN)
        .collect();
    assert_eq!(hidden.len(), 1, "{warnings:?}");
    assert!(
        hidden[0].message.contains("backdropBlur 8"),
        "{}",
        hidden[0].message
    );
}

#[test]
fn the_inspector_reads_the_radius() {
    let mut core = Core::new();
    core.set_inspect(true);
    frame(&mut core, 1.0, |ui| {
        ui.leaf_keyed("glass", glass());
    });
    let nodes = core.nodes();
    assert!(nodes.iter().any(|n| n.backdrop_blur == 12.0));
    assert!(nodes.iter().filter(|n| n.backdrop_blur == 0.0).count() >= 1);
}
