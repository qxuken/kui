//! An image drawn smaller is drawn from a level the core halves
//! (`docs/adr/0044-an-image-drawn-smaller-is-drawn-from-a-level.md`,
//! backlog V6): which level a quad names, through `fit`, the display's
//! scale and a node's `scale`, and which images never take one.

use kui_core::{Core, ImageFit, ImageId, ImageOpts, NodeSpec, Quad, QuadKind, Sampling, Size};

fn rgba(w: u32, h: u32) -> Vec<u8> {
    vec![0x80; (w * h * 4) as usize]
}

/// One frame drawing `id` in a `w × h` box under `spec`'s extra rows,
/// at display scale `scale`; the image's quad, `Image` or `Texture`.
fn draw(core: &mut Core, id: ImageId, opts: ImageOpts, spec: NodeSpec, scale: f32) -> Quad {
    let mut ui = core.frame(Size::new(800.0, 600.0), scale);
    ui.image_with(id, opts, spec);
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .find(|q| matches!(q.kind, QuadKind::Image | QuadKind::Texture))
        .copied()
        .expect("the image's quad")
}

fn texels(q: &Quad) -> (u32, u32) {
    (q.uv[2], q.uv[3])
}

/// 1024 texels in 100 px is 10.24 a pixel: level 3, 128 texels, the
/// deepest still at least one a pixel. The page holds level 3 and never
/// level 0.
#[test]
fn a_photo_drawn_small_names_the_deepest_level_that_covers_it() {
    let mut core = Core::new();
    let id = core.resources.add_image(1024, 1024, rgba(1024, 1024));
    let q = draw(
        &mut core,
        id,
        ImageOpts::default(),
        NodeSpec::column().size(100.0, 100.0),
        1.0,
    );
    assert_eq!(q.kind, QuadKind::Image);
    assert_eq!(texels(&q), (128, 128));
    assert_eq!((q.rect.w, q.rect.h), (100.0, 100.0));
    assert!(core.atlas.has_image_level(id, 3));
    assert!(!core.atlas.has_image_level(id, 0), "level 0 never went in");

    // At no more than two texels a pixel it is level 0, as before.
    let q = draw(
        &mut core,
        id,
        ImageOpts::default(),
        NodeSpec::column().size(600.0, 600.0),
        1.0,
    );
    assert_eq!(texels(&q), (1024, 1024));
}

/// The ratio is in physical px: at 2× the same box is 200 px, level 2;
/// and a node `scale` it is drawn through counts as the display's does.
#[test]
fn the_display_scale_and_a_node_scale_both_count() {
    let mut core = Core::new();
    let id = core.resources.add_image(1024, 1024, rgba(1024, 1024));
    let q = draw(
        &mut core,
        id,
        ImageOpts::default(),
        NodeSpec::column().size(100.0, 100.0),
        2.0,
    );
    assert_eq!(texels(&q), (256, 256));

    // 400 px drawn at a quarter is 100 px on screen: level 3.
    let q = draw(
        &mut core,
        id,
        ImageOpts::default(),
        NodeSpec::column().size(400.0, 400.0).scale(0.25),
        1.0,
    );
    assert_eq!(texels(&q), (128, 128));
}

/// `cover` crops on the level's own size: a 1000×500 image in a 100 px
/// square shows a 500-texel square of level 0, five a pixel, so level 2
/// (250×125) and its 125-texel square, centred.
#[test]
fn cover_crops_the_level() {
    let mut core = Core::new();
    let id = core.resources.add_image(1000, 500, rgba(1000, 500));
    let opts = ImageOpts {
        fit: ImageFit::Cover,
        ..ImageOpts::default()
    };
    let q = draw(
        &mut core,
        id,
        opts,
        NodeSpec::column().size(100.0, 100.0),
        1.0,
    );
    assert_eq!(texels(&q), (125, 125));
}

/// `nearest` asked for the texels as they are, and an updated image is a
/// stream: both draw level 0 however small.
#[test]
fn nearest_and_a_stream_draw_level_zero() {
    let mut core = Core::new();
    let id = core.resources.add_image(512, 512, rgba(512, 512));
    let nearest = ImageOpts {
        sampling: Sampling::Nearest,
        ..ImageOpts::default()
    };
    let q = draw(
        &mut core,
        id,
        nearest,
        NodeSpec::column().size(50.0, 50.0),
        1.0,
    );
    assert_eq!(texels(&q), (512, 512));

    assert!(core.update_image(id, 512, 512, rgba(512, 512)));
    let q = draw(
        &mut core,
        id,
        ImageOpts::default(),
        NodeSpec::column().size(50.0, 50.0),
        1.0,
    );
    assert_eq!(q.kind, QuadKind::Texture, "a stream is texture-backed");
    let (dl, _) = core.output();
    assert_eq!(dl.textures[0].uv, [0, 0, 512, 512], "and drawn whole");
}

/// An image past a page is texture-backed, but drawn small and never
/// updated its level fits a page and is drawn from the atlas: a 4100-
/// texel strip in 100 px takes level 5, 129 texels, and no texture.
#[test]
fn an_image_past_a_page_drawn_small_is_drawn_from_the_page() {
    let mut core = Core::new();
    let id = core.resources.add_image(4100, 64, rgba(4100, 64));
    let q = draw(
        &mut core,
        id,
        ImageOpts::default(),
        NodeSpec::column().size(100.0, 1.0),
        1.0,
    );
    assert_eq!(q.kind, QuadKind::Image);
    assert_eq!(texels(&q), (129, 2));
    let (dl, _) = core.output();
    assert!(dl.textures.is_empty());
}

/// A level is the image halved: the page holds averaged texels. A 2×2
/// image of two greys and two whites drawn in one pixel is one texel of
/// their linear-light average.
#[test]
fn the_page_holds_the_halved_pixels() {
    let mut core = Core::new();
    let px = [
        [0, 0, 0, 255],
        [255, 255, 255, 255],
        [0, 0, 0, 255],
        [255, 255, 255, 255],
    ]
    .concat();
    let id = core.resources.add_image(2, 2, px);
    let q = draw(
        &mut core,
        id,
        ImageOpts::default(),
        NodeSpec::column().size(1.0, 1.0),
        1.0,
    );
    assert_eq!(texels(&q), (1, 1));
    let at = ((q.uv[1] * core.atlas.size + q.uv[0]) * 4) as usize;
    let texel = &core.atlas.pixels[at..at + 4];
    assert!((187..=189).contains(&texel[0]), "{texel:?}");
    assert_eq!(texel[3], 255);
}
