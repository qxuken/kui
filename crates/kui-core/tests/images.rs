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

/// One frame drawing `id` as a texture-backed image; what it returns is
/// the first byte of the pixels the display list hands a backend.
fn draw(core: &mut Core, id: kui_core::ImageId) -> u8 {
    let mut ui = core.frame(Size::new(64.0, 64.0), 1.0);
    ui.image(id, NodeSpec::column().fill());
    ui.finish();
    core.output().0.texture_pixels[0].rgba[0]
}

/// Where an image's bytes live, for telling a reused buffer from a new one.
fn buffer_of(core: &Core, id: kui_core::ImageId) -> usize {
    core.image_pixels(id).unwrap().2.as_ptr() as usize
}

/// `update_image_with` between frames, the way a Node or C stream calls
/// it: the last frame's display list still holds the current buffer, so
/// the update writes into the one the update before replaced. From the
/// third frame on, a stream alternates two buffers and allocates nothing
/// (backlog W20) — and each frame still draws its own pixels.
#[test]
fn a_stream_updated_between_frames_alternates_two_buffers() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    let mut seen = Vec::new();
    for i in 1..=8u8 {
        assert!(core.update_image_with(id, 4, 4, |px| {
            assert_eq!(px.len(), 4 * 4 * 4);
            px.fill(i);
        }));
        assert_eq!(draw(&mut core, id), i, "frame {i} draws its own pixels");
        seen.push(buffer_of(&core, id));
    }
    let (_, _, px) = core.image_pixels(id).unwrap();
    assert!(px.iter().all(|&b| b == 8));
    let mut steady = seen[2..].to_vec();
    steady.sort();
    steady.dedup();
    assert_eq!(steady.len(), 2, "two buffers, taken in turn: {seen:x?}");
    assert_ne!(seen[6], seen[7]);
    assert_eq!(seen[5], seen[7]);
}

/// Inside the frame's build the display list has been cleared, so nothing
/// else holds the pixels and the update writes where they already are.
#[test]
fn a_stream_updated_inside_the_frame_writes_in_place() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    let mut seen = Vec::new();
    for i in 1..=4u8 {
        let mut ui = core.frame(Size::new(64.0, 64.0), 1.0);
        assert!(ui.core().update_image_with(id, 4, 4, |px| px.fill(i)));
        ui.image(id, NodeSpec::column().fill());
        ui.finish();
        assert_eq!(core.output().0.texture_pixels[0].rgba[0], i);
        seen.push(buffer_of(&core, id));
    }
    assert!(seen[1..].iter().all(|&p| p == seen[1]), "{seen:x?}");
}

/// A reused buffer is only one nobody reads: pixels a backend (or the last
/// display list) still holds are never written under it.
#[test]
fn an_update_never_writes_into_pixels_still_held() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    for i in 1..=3u8 {
        core.update_image_with(id, 4, 4, |px| px.fill(i));
        draw(&mut core, id);
    }
    // A backend mid-upload of frame 3, and the display list of frame 3.
    let held = core.output().0.texture_pixels[0].rgba.clone();
    let older = core.image_pixels(id).unwrap().2;
    assert!(std::sync::Arc::ptr_eq(&held, &older));
    core.update_image_with(id, 4, 4, |px| px.fill(4));
    core.update_image_with(id, 4, 4, |px| px.fill(5));
    assert!(
        held.iter().all(|&b| b == 3),
        "the held frame kept its pixels"
    );
    assert_eq!(draw(&mut core, id), 5);
}

/// The size may change on any update, and the handed slice follows it; a
/// dead handle takes nothing and never calls `fill`.
#[test]
fn update_image_with_resizes_and_refuses_a_dead_handle() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    for (w, h) in [(8, 2), (16, 16), (2, 2), (16, 16)] {
        assert!(core.update_image_with(id, w, h, |px| {
            assert_eq!(px.len(), (w * h * 4) as usize);
            px.fill(0x11);
        }));
        draw(&mut core, id);
        let (pw, ph, px) = core.image_pixels(id).unwrap();
        assert_eq!((pw, ph, px.len()), (w, h, (w * h * 4) as usize));
    }
    core.remove_image(id);
    assert!(!core.update_image_with(id, 4, 4, |_| panic!("fill called for a dead handle")));
    let live = core.resources.add_image(1, 1, rgba(1, 1));
    assert!(!core.update_image_with(live, u32::MAX, u32::MAX, |_| {
        panic!("fill called for a size that overflows")
    }));
}

/// Weak handles on every buffer an image's pixels were seen in, and how
/// many of them are still alive.
fn live(seen: &[std::sync::Weak<Vec<u8>>]) -> usize {
    seen.iter().filter(|w| w.upgrade().is_some()).count()
}

/// One frame drawing `id`, whatever its backing.
fn frame(core: &mut Core, id: kui_core::ImageId) {
    let mut ui = core.frame(Size::new(64.0, 64.0), 1.0);
    ui.image(id, NodeSpec::column().fill());
    ui.finish();
}

/// An image updated once keeps one buffer, not the pixels it was added
/// with as a spare for a stream that never comes (W20's review: an 8K
/// image updated once held ~264 MB).
#[test]
fn an_image_updated_once_keeps_no_spare() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    frame(&mut core, id);
    // A backend still reading the added pixels, so the update cannot
    // write over them.
    let held = core.image_pixels(id).unwrap().2;
    let added = std::sync::Arc::downgrade(&held);
    assert!(core.update_image_with(id, 4, 4, |px| px.fill(1)));
    drop(held);
    assert_eq!(draw(&mut core, id), 1);
    assert!(added.upgrade().is_none(), "the added pixels were kept");
}

/// A stream that stops gives its spare back once `SPARE_FRAMES` frames
/// pass with no update; while it runs, it keeps two buffers.
#[test]
fn a_stopped_stream_gives_its_spare_back() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    frame(&mut core, id);
    let mut seen = Vec::new();
    for i in 1..=6u8 {
        core.update_image_with(id, 4, 4, |px| px.fill(i));
        draw(&mut core, id);
        seen.push(std::sync::Arc::downgrade(&core.image_pixels(id).unwrap().2));
    }
    assert_eq!(live(&seen), 2, "a running stream keeps two buffers");
    // The loop's last frame was the first after the last update.
    for _ in 1..kui_core::resources::SPARE_FRAMES {
        draw(&mut core, id);
    }
    assert_eq!(live(&seen), 2, "the spare outlives the stream a while");
    draw(&mut core, id);
    assert_eq!(live(&seen), 1, "a stopped stream holds one buffer");
    // And starting again is a stream like any other.
    for i in 1..=4u8 {
        core.update_image_with(id, 4, 4, |px| px.fill(i));
        assert_eq!(draw(&mut core, id), i);
    }
}

/// A stream that shrinks keeps neither its old size's buffers nor their
/// capacity.
#[test]
fn a_shrunk_stream_drops_its_old_size() {
    let mut core = Core::new();
    let id = core.resources.add_image(64, 64, rgba(64, 64));
    frame(&mut core, id);
    for i in 1..=4u8 {
        core.update_image_with(id, 64, 64, |px| px.fill(i));
        draw(&mut core, id);
    }
    // Taken after the stream, since a `Weak` keeps a buffer from reuse.
    let last_big = std::sync::Arc::downgrade(&core.image_pixels(id).unwrap().2);
    for i in 1..=4u8 {
        core.update_image_with(id, 2, 2, |px| px.fill(i));
        assert_eq!(draw(&mut core, id), i);
        let (w, h, px) = core.image_pixels(id).unwrap();
        assert_eq!((w, h, px.len()), (2, 2, 16));
        assert!(
            px.capacity() <= 32,
            "update {i}: capacity {}",
            px.capacity()
        );
    }
    assert!(
        last_big.upgrade().is_none(),
        "a 64×64 buffer outlived the shrink"
    );
}

/// A host that keeps a `Weak` on pixels it was handed does not make the
/// next update panic: a spare with a live `Weak` is not reused.
#[test]
fn a_weak_on_the_pixels_does_not_break_the_stream() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    frame(&mut core, id);
    let mut weaks = Vec::new();
    for i in 1..=6u8 {
        core.update_image_with(id, 4, 4, |px| px.fill(i));
        assert_eq!(draw(&mut core, id), i);
        weaks.push(std::sync::Arc::downgrade(&core.image_pixels(id).unwrap().2));
    }
}

/// A `fill` that panics leaves the image at its new size with bytes of
/// that length, so the next frame draws (stale pixels) instead of reading
/// past a buffer its size does not match.
#[test]
fn a_panicking_fill_leaves_a_consistent_image() {
    let mut core = Core::new();
    let id = core.resources.add_image(4, 4, rgba(4, 4));
    core.update_image_with(id, 4, 4, |px| px.fill(1));
    draw(&mut core, id);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        core.update_image_with(id, 8, 8, |_| panic!("the app's fill"));
    }));
    assert!(caught.is_err());
    draw(&mut core, id);
    let (w, h, px) = core.image_pixels(id).unwrap();
    assert_eq!((w, h, px.len()), (8, 8, 8 * 8 * 4));
}

/// The corpus fixture that samples its `image` (backlog V1).
const SAMPLER: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    return kui_sample(in.local / max(in.size, vec2<f32>(1.0)));
}";

fn fragment_draws(core: &mut Core) -> Vec<kui_core::FragmentDraw> {
    core.output().0.fragments.clone()
}

/// A fragment reading an atlas-backed image carries the atlas slot; one
/// reading a texture-backed image takes a `textures` entry — the same
/// entry an `image` node of it would — and carries its index. The frame
/// draws no texture *quad* for it: the fragment's own quad is the draw.
#[test]
fn a_fragment_reads_an_image_from_the_atlas_or_its_texture() {
    use kui_core::FragmentImage;
    let mut core = Core::new();
    let icon = core.resources.add_image(4, 4, rgba(4, 4));
    let stream = core.resources.add_image(4, 4, rgba(4, 4));
    assert!(core.update_image(stream, 8, 2, rgba(8, 2)));
    let f = core.add_fragment(SAMPLER).unwrap();
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    let spec = || {
        NodeSpec::column()
            .width(Sizing::Fixed(16.0))
            .height(Sizing::Fixed(16.0))
    };
    ui.fragment(f.with_image(icon), &[], spec());
    ui.fragment(f.with_image(stream), &[], spec());
    ui.fragment(f, &[], spec());
    ui.finish();

    let draws = fragment_draws(&mut core);
    assert_eq!(draws.len(), 3);
    assert!(matches!(draws[0].image, FragmentImage::Atlas([_, _, 4, 4])));
    assert_eq!(
        draws[1].image,
        FragmentImage::Texture {
            index: 0,
            uv: [0, 0, 8, 2]
        }
    );
    assert_eq!(draws[2].image, FragmentImage::None);
    let (dl, _) = core.output();
    assert_eq!(dl.textures.len(), 1);
    assert_eq!(dl.textures[0].id, stream);
    assert_eq!(dl.texture_pixels[0].rev, 1);
    assert_eq!(
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::Fragment)
            .count(),
        3
    );
    assert_eq!(
        dl.quads
            .iter()
            .filter(|q| q.kind == QuadKind::Texture)
            .count(),
        0
    );
}

/// The removal order ADR 0015 decision 9 asked to see pinned before an
/// image input existed: an image removed under a live fragment makes the
/// fragment draw nothing — the fallback every resource has — from the
/// next frame on, and putting the image back (a new handle) draws again.
#[test]
fn a_removed_image_under_a_live_fragment_draws_nothing() {
    let mut core = Core::new();
    let img = core.resources.add_image(4, 4, rgba(4, 4));
    let f = core.add_fragment(SAMPLER).unwrap();
    let spec = || {
        NodeSpec::column()
            .width(Sizing::Fixed(16.0))
            .height(Sizing::Fixed(16.0))
    };
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.fragment(f.with_image(img), &[], spec());
    ui.finish();
    assert_eq!(fragment_draws(&mut core).len(), 1);

    core.remove_image(img);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.fragment(f.with_image(img), &[], spec());
    ui.finish();
    assert!(fragment_draws(&mut core).is_empty(), "the image is gone");
    assert!(
        core.output()
            .0
            .quads
            .iter()
            .all(|q| q.kind != QuadKind::Fragment)
    );

    // The other order too: the fragment goes, the image stays.
    let img = core.resources.add_image(4, 4, rgba(4, 4));
    core.remove_fragment(f);
    let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
    ui.fragment(f.with_image(img), &[], spec());
    ui.finish();
    assert!(fragment_draws(&mut core).is_empty(), "the function is gone");
    assert!(core.output().0.textures.is_empty());
}

/// AR19: a working set larger than the atlas page — three atlas-backed
/// images that do not fit one page together — grows the page within the
/// frame instead of resetting it on every overflow. Before, the page
/// reset mid-emit every frame. The growth keeps every slot where it was
/// (F99), so the frame that grew is right as presented and asks for no
/// other; until F99 it reset first, the quads emitted before sampled the
/// page packed over them, and the frame asked for the one that rebuilt
/// them.
#[test]
fn a_set_larger_than_the_atlas_page_grows_it_within_the_frame() {
    let mut core = Core::new();
    core.atlas = kui_core::atlas::GlyphAtlas::with_size(128);
    let ids: Vec<_> = (0..3)
        .map(|_| core.resources.add_image(80, 80, rgba(80, 80)))
        .collect();
    let draw = |core: &mut Core| {
        let mut ui = core.frame(Size::new(400.0, 300.0), 1.0);
        ui.with(NodeSpec::row(), |ui| {
            for id in &ids {
                ui.image(*id, NodeSpec::column());
            }
        });
        ui.finish();
    };
    draw(&mut core);
    assert!(
        core.atlas.size >= 256,
        "grown for the set: {}",
        core.atlas.size
    );
    assert!(
        !core.animating(),
        "the frame that grew is right as it is: it asks for no other"
    );
    let epoch = core.atlas.epoch;
    draw(&mut core);
    assert_eq!(core.atlas.epoch, epoch, "the second frame is still");
    assert!(!core.animating(), "and asks for nothing");
    assert_eq!(image_quads(&mut core).len(), 3);
}
