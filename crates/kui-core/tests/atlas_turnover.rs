//! The glyph atlas under a working set that turns over (backlog F99):
//! a frame whose emit fills the page must still present quads that each
//! sample what they were emitted for. The page used to reset mid-emit —
//! every slot dropped, the pixels zeroed and packed over — so the quads
//! emitted before the fill sampled blanks and scraps for that one frame:
//! kawoosh's fonts pane, scrolling through six hundred families, showed
//! a frame with nearly every glyph in the window gone now and then.

use kui_core::atlas::GlyphAtlas;
use kui_core::{Core, NodeSpec, Quad, QuadKind, Size, TextStyle};

const LINE: &str = "Sphinx of black quartz, judge my vow 0123456789";

/// A distinct RGBA pattern per image, so a slot holding another image's
/// pixels (or none) cannot pass for this one's.
fn pattern(seed: u32, w: u32, h: u32) -> Vec<u8> {
    let mut v = Vec::with_capacity((w * h * 4) as usize);
    for i in 0..w * h {
        let x = i
            .wrapping_mul(2_654_435_761)
            .wrapping_add(seed.wrapping_mul(40_503));
        v.extend_from_slice(&[(x >> 8) as u8, (x >> 16) as u8, (x >> 24) as u8, 255]);
    }
    v
}

/// A frame of the turnover: `sizes` of the line that no earlier frame
/// drew — new glyph keys, the way a list scrolling through fonts brings
/// new faces — and one new image, over a page that still holds the
/// frames before.
fn draw(core: &mut Core, sizes: std::ops::Range<u32>, image: kui_core::resources::ImageId) {
    let mut ui = core.frame(Size::new(900.0, 400.0), 1.0);
    ui.with(NodeSpec::column(), |ui| {
        for k in sizes {
            ui.text(LINE, TextStyle::new(11.0 + k as f32 * 0.75));
        }
        ui.image(image, NodeSpec::column());
    });
    ui.finish();
}

/// The texels a quad samples, row by row.
fn texels(atlas: &GlyphAtlas, q: &Quad) -> Vec<u8> {
    let [x, y, w, h] = q.uv;
    let mut out = Vec::with_capacity((w * h * 4) as usize);
    for row in y..y + h {
        let at = ((row * atlas.size + x) * 4) as usize;
        out.extend_from_slice(&atlas.pixels[at..at + (w * 4) as usize]);
    }
    out
}

fn samples_atlas(kind: QuadKind) -> bool {
    matches!(
        kind,
        QuadKind::GlyphMask | QuadKind::GlyphColor | QuadKind::GlyphSubpixel | QuadKind::Image
    )
}

#[test]
fn a_frame_that_fills_the_page_presents_every_quad_on_what_it_was_emitted_for() {
    let mut core = Core::new();
    // A small page, so a few frames of new sizes fill it mid-emit.
    core.atlas = GlyphAtlas::with_size(256);
    let (mut fills, mut grew) = (0, 0);
    let mut next = 0;
    for n in 0..24 {
        // Two new sizes a frame, which the page's rows see coming and
        // empty it for between frames, and every sixth frame a burst of
        // eight, which fills it mid-emit.
        let count = if n % 6 == 5 { 8 } else { 2 };
        let sizes = next..next + count;
        next += count;
        let rgba = pattern(n, 24, 24);
        let image = core.resources.add_image(24, 24, rgba.clone());
        let (epoch, size) = (core.atlas.epoch, core.atlas.size);
        draw(&mut core, sizes.clone(), image);
        if core.atlas.epoch != epoch {
            fills += 1;
        }
        if core.atlas.size > size {
            grew += 1;
        }

        // The same frame on a page with room to spare: what each quad
        // should sample.
        let mut reference = Core::new();
        let image = reference.resources.add_image(24, 24, rgba);
        draw(&mut reference, sizes, image);

        let (dl, atlas) = core.output();
        let quads: Vec<Quad> = dl.quads.clone();
        let (want_dl, want_atlas) = reference.output();
        assert_eq!(
            quads.len(),
            want_dl.quads.len(),
            "frame {n}: the same quads"
        );
        let mut checked = 0;
        for (i, (q, want)) in quads.iter().zip(&want_dl.quads).enumerate() {
            assert_eq!(q.kind, want.kind, "frame {n}, quad {i}");
            if !samples_atlas(q.kind) {
                continue;
            }
            assert_eq!(&q.uv[2..], &want.uv[2..], "frame {n}, quad {i}: slot size");
            assert!(
                texels(atlas, q) == texels(want_atlas, want),
                "frame {n}, quad {i} ({:?} at {:?}): its slot no longer holds what it \
                 was emitted for — the page was repacked under it",
                q.kind,
                q.uv,
            );
            checked += 1;
        }
        assert!(checked > 0, "frame {n} sampled the atlas");
    }
    assert!(fills >= 3, "the turnover filled the page: {fills}");
    assert!(grew >= 1, "a burst filled it mid-frame: {grew}");
}
