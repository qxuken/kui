//! The `gradient` row (`docs/adr/0042-a-gradient-is-an-image-the-core-paints.md`):
//! one `Image` quad from the atlas over the box's background and under
//! its border and its children, keyed by what the gradient is and not by
//! the box that draws it.

use kui_core::{Color, Core, Gradient, NodeSpec, Quad, QuadKind, Side, Size, TextStyle};

const VIEW: Size = Size { w: 400.0, h: 300.0 };
const RED: Color = Color {
    r: 1.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};
const BLUE: Color = Color {
    r: 0.0,
    g: 0.0,
    b: 1.0,
    a: 1.0,
};

fn frame(core: &mut Core, scale: f32, build: impl FnOnce(&mut kui_core::Ui<'_>)) -> Vec<Quad> {
    let mut ui = core.frame(VIEW, scale);
    build(&mut ui);
    ui.finish();
    core.output().0.quads.clone()
}

fn fade() -> Gradient {
    Gradient::to(Side::Right, [RED, BLUE])
}

#[test]
fn a_gradient_is_one_image_quad_over_the_box() {
    let mut core = Core::new();
    let quads = frame(&mut core, 2.0, |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(200.0, 100.0)
                .radius(8.0)
                .gradient(fade()),
        );
    });
    assert_eq!(quads.len(), 1);
    let q = quads[0];
    assert_eq!(q.kind, QuadKind::Image);
    assert_eq!((q.rect.w, q.rect.h), (400.0, 200.0));
    assert_eq!(q.radius, [16.0; 4]);
    assert_eq!(q.color, Color::WHITE);
    // The whole strip, sampled linearly.
    assert_eq!((q.uv[2], q.uv[3]), (256, 1));
    assert_eq!(q.border_w, 0.0);
}

/// Over the background, under the border — which becomes a ring of its
/// own so the gradient does not cover the inside of it — and under the
/// children; after the shadow.
#[test]
fn it_paints_over_the_bg_and_under_the_border_and_the_children() {
    let mut core = Core::new();
    let bg = Color::hex(0x202020ff);
    let edge = Color::hex(0xffffffff);
    let quads = frame(&mut core, 1.0, |ui| {
        ui.with(
            NodeSpec::column()
                .size(200.0, 100.0)
                .bg(bg)
                .border(2.0, edge)
                .shadow_color(Color::BLACK)
                .shadow_blur(8.0)
                .gradient(fade()),
            |ui| {
                ui.leaf(NodeSpec::column().size(10.0, 10.0).bg(RED));
            },
        );
    });
    let kinds: Vec<QuadKind> = quads.iter().map(|q| q.kind).collect();
    assert_eq!(
        kinds,
        [
            QuadKind::Shadow,
            QuadKind::Solid,
            QuadKind::Image,
            QuadKind::Solid,
            QuadKind::Solid
        ]
    );
    // The background alone, then the gradient, then the ring.
    assert_eq!((quads[1].color, quads[1].border_w), (bg, 0.0));
    assert_eq!(quads[3].color, Color::TRANSPARENT);
    assert_eq!((quads[3].border_w, quads[3].border_color), (2.0, edge));
    assert_eq!(quads[4].color, RED);

    // A border and no background: the gradient, then the ring.
    let quads = frame(&mut core, 1.0, |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(200.0, 100.0)
                .border(2.0, edge)
                .gradient(fade()),
        );
    });
    let kinds: Vec<QuadKind> = quads.iter().map(|q| q.kind).collect();
    assert_eq!(kinds, [QuadKind::Image, QuadKind::Solid]);
    assert_eq!(quads[1].color, Color::TRANSPARENT);
}

/// A text's glyphs are the box's content: the gradient goes under them.
#[test]
fn it_paints_under_a_leafs_content() {
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.with(NodeSpec::column().pad(8.0).gradient(fade()), |ui| {
            ui.text("label", TextStyle::new(14.0));
        });
    });
    assert_eq!(quads[0].kind, QuadKind::Image);
    assert!(quads.len() > 1);
    assert!(quads[1..].iter().all(|q| q.kind != QuadKind::Image));
}

/// The key is the gradient and not the box: two boxes of different
/// sizes, and the same box at another scale, draw one slot; a different
/// gradient draws another; an angle draws the square.
#[test]
fn equal_gradients_share_a_slot_at_any_size() {
    let mut core = Core::new();
    let build = |ui: &mut kui_core::Ui<'_>| {
        ui.leaf(NodeSpec::column().size(200.0, 100.0).gradient(fade()));
        ui.leaf(NodeSpec::column().size(40.0, 300.0).gradient(fade()));
        ui.leaf(
            NodeSpec::column()
                .size(50.0, 50.0)
                .gradient(Gradient::to(Side::Right, [BLUE, RED])),
        );
        ui.leaf(
            NodeSpec::column()
                .size(50.0, 50.0)
                .gradient(Gradient::angle(0.1, [RED, BLUE])),
        );
    };
    let quads = frame(&mut core, 1.0, build);
    assert_eq!(quads[0].uv, quads[1].uv);
    assert_ne!(quads[0].uv, quads[2].uv);
    assert_eq!((quads[3].uv[2], quads[3].uv[3]), (128, 128));
    let again = frame(&mut core, 2.0, build);
    assert_eq!(quads[0].uv, again[0].uv, "the scale is not in the key");
}

/// The group opacity fades it with the box, and a gradient with nothing
/// to paint paints nothing.
#[test]
fn it_fades_with_the_box_and_one_stop_draws_nothing() {
    let mut core = Core::new();
    let quads = frame(&mut core, 1.0, |ui| {
        ui.leaf(
            NodeSpec::column()
                .size(50.0, 50.0)
                .opacity(0.5)
                .gradient(fade()),
        );
        ui.leaf(
            NodeSpec::column()
                .size(50.0, 50.0)
                .gradient(Gradient::to(Side::Right, [RED])),
        );
    });
    assert_eq!(quads.len(), 1);
    assert_eq!(quads[0].color.a, 0.5);
}

/// A departing box keeps its gradient while it fades.
#[test]
fn a_ghost_keeps_its_gradient() {
    let mut core = Core::new();
    core.set_time(0.0);
    let mut ui = core.frame(VIEW, 1.0);
    ui.with_keyed(
        "card",
        NodeSpec::column()
            .size(100.0, 50.0)
            .transition(100.0)
            .exit(kui_core::Enter::from(0.0, 20.0).opacity(0.0))
            .gradient(fade()),
        |_| {},
    );
    ui.finish();
    let live = core.output().0.quads.clone();
    core.set_time(0.05);
    let ui = core.frame(VIEW, 1.0);
    ui.finish();
    // The frame that notices it gone starts the exit; the next is in it.
    core.set_time(0.1);
    let ui = core.frame(VIEW, 1.0);
    ui.finish();
    let ghost = core.output().0.quads.clone();
    assert_eq!(ghost.len(), 1);
    assert_eq!(ghost[0].kind, QuadKind::Image);
    assert_eq!(ghost[0].uv, live[0].uv);
    assert!(ghost[0].color.a < 1.0);
}
