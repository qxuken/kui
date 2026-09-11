//! Images end to end: registration, layout sizing (intrinsic + aspect),
//! atlas placement/growth, and display-list emission.

use kui_core::{Core, NodeSpec, QuadKind, Size, Sizing};

fn rgba(w: u32, h: u32) -> Vec<u8> {
    vec![0x80; (w * h * 4) as usize]
}

fn image_quads(core: &mut Core) -> Vec<kui_core::Quad> {
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| q.kind == QuadKind::Image)
        .cloned()
        .collect()
}

#[test]
fn fit_sizing_takes_pixel_dimensions() {
    let mut core = Core::new();
    let id = core.resources.add_image(40, 20, rgba(40, 20));
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(id, NodeSpec::column());
    ui.finish();

    let quads = image_quads(&mut core);
    assert_eq!(quads.len(), 1);
    assert_eq!((quads[0].rect.w, quads[0].rect.h), (40.0, 20.0));
    assert_eq!((quads[0].uv[2], quads[0].uv[3]), (40, 20));
}

#[test]
fn fit_height_preserves_aspect_at_fixed_width() {
    let mut core = Core::new();
    let id = core.resources.add_image(100, 50, rgba(100, 50));
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(id, NodeSpec::column().width(Sizing::Fixed(80.0)));
    ui.finish();

    let quads = image_quads(&mut core);
    assert_eq!((quads[0].rect.w, quads[0].rect.h), (80.0, 40.0));
}

#[test]
fn removed_image_emits_nothing() {
    let mut core = Core::new();
    let id = core.resources.add_image(10, 10, rgba(10, 10));
    core.remove_image(id);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(
        id,
        NodeSpec::column()
            .width(Sizing::Fixed(10.0))
            .height(Sizing::Fixed(10.0)),
    );
    ui.finish();
    assert!(image_quads(&mut core).is_empty());
}

#[test]
fn oversized_image_grows_the_atlas() {
    let mut core = Core::new();
    assert_eq!(core.atlas.size, kui_core::atlas::ATLAS_SIZE);
    let id = core.resources.add_image(1500, 100, rgba(1500, 100));
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(id, NodeSpec::column());
    ui.finish();

    let quads = image_quads(&mut core);
    assert_eq!(
        quads.len(),
        1,
        "wide image should render after atlas growth"
    );
    assert!(
        core.atlas.size >= 2048,
        "atlas should have grown, is {}",
        core.atlas.size
    );
    // Second frame reuses the slot without another reset.
    let epoch = core.atlas.epoch;
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(id, NodeSpec::column());
    ui.finish();
    assert_eq!(core.atlas.epoch, epoch);
    assert_eq!(image_quads(&mut core).len(), 1);
}

#[test]
fn scale_two_doubles_physical_rect() {
    let mut core = Core::new();
    let id = core.resources.add_image(40, 20, rgba(40, 20));
    let mut ui = core.frame(Size::new(400.0, 300.0), 2.0);
    ui.image(id, NodeSpec::column());
    ui.finish();
    let quads = image_quads(&mut core);
    // Logical 40x20 -> physical 80x40; uv stays the stored pixels.
    assert_eq!((quads[0].rect.w, quads[0].rect.h), (80.0, 40.0));
    assert_eq!((quads[0].uv[2], quads[0].uv[3]), (40, 20));
}

// The two below pin what every leaf door owes the frame: the flags that let
// emission skip a pass are noted where the node is pushed, not by the door
// that pushed it. Before they were, an `image` set two of nine, so an
// image that was the frame's only translucent node painted opaque and one
// that was its only float was culled under its parent's clip.

#[test]
fn a_lone_translucent_image_fades() {
    let mut core = Core::new();
    let id = core.resources.add_image(10, 10, rgba(10, 10));
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.image(
        id,
        NodeSpec::column()
            .width(Sizing::Fixed(10.0))
            .height(Sizing::Fixed(10.0))
            .opacity(0.5),
    );
    ui.finish();
    let quads = image_quads(&mut core);
    assert_eq!(quads.len(), 1);
    assert_eq!(
        quads[0].color.a, 0.5,
        "the image is the only fade in the frame"
    );
}

#[test]
fn a_lone_floating_image_escapes_its_parent_clip() {
    use kui_core::FloatConfig;
    let mut core = Core::new();
    let id = core.resources.add_image(10, 10, rgba(10, 10));
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.open(
        NodeSpec::column()
            .width(Sizing::Fixed(20.0))
            .height(Sizing::Fixed(20.0))
            .clip(),
    );
    ui.image(
        id,
        NodeSpec::column()
            .width(Sizing::Fixed(10.0))
            .height(Sizing::Fixed(10.0))
            .float(FloatConfig::parent().offset(100.0, 100.0)),
    );
    ui.close();
    ui.finish();
    let quads = image_quads(&mut core);
    assert_eq!(quads.len(), 1, "a float paints outside the clip it escaped");
    assert_eq!((quads[0].rect.x, quads[0].rect.y), (100.0, 100.0));
}

/// `update_image` with a buffer that is not `w × h × 4` bytes takes
/// nothing (a backend would refuse the short upload with a validation
/// error, which wgpu turns into a panic); the entry keeps its pixels and
/// its backing (`docs/adr/0025-the-image-is-the-canvas.md`).
#[test]
fn an_update_of_the_wrong_length_is_refused() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, vec![0xff; 4 * 4 * 4]);
    assert!(!core.update_image(id, 8, 2, vec![0; 3]));
    let (w, h, px) = core.image_pixels(id).unwrap();
    assert_eq!((w, h, px.len()), (4, 4, 64));
    assert!(core.update_image(id, 8, 2, vec![0x80; 8 * 2 * 4]));
    let (w, h, _) = core.image_pixels(id).unwrap();
    assert_eq!((w, h), (8, 2));
}
