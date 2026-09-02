//! LCD subpixel glyph rasterization through a live `Core`: off by default
//! (alpha masks), on request (per-channel coverage in the atlas, emitted as
//! `GlyphSubpixel` quads), and the atlas re-rasterizes when it flips.

use kui_core::{Core, NodeSpec, QuadKind, Size, TextStyle};

fn frame(core: &mut Core) -> Vec<QuadKind> {
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().pad(8.0));
    ui.text("Subpixel text", TextStyle::new(14.0));
    ui.finish();
    let (dl, _) = core.output();
    dl.quads.iter().map(|q| q.kind).collect()
}

#[test]
fn subpixel_glyphs_are_opt_in_and_carry_per_channel_coverage() {
    let mut core = Core::new();
    let kinds = frame(&mut core);
    assert!(kinds.contains(&QuadKind::GlyphMask), "default: alpha masks");
    assert!(!kinds.contains(&QuadKind::GlyphSubpixel));
    let epoch = core.atlas.epoch;

    core.set_subpixel_text(true);
    assert!(
        core.atlas.epoch > epoch,
        "flipping the mode resets the atlas"
    );
    let kinds = frame(&mut core);
    assert!(kinds.contains(&QuadKind::GlyphSubpixel));
    assert!(!kinds.contains(&QuadKind::GlyphMask), "no stale masks");

    // Somewhere on a glyph edge the three channels disagree — that is the
    // whole point — and alpha is their union.
    let (_, atlas) = core.output();
    let mut fringe = false;
    for px in atlas.pixels.as_chunks::<4>().0 {
        if px[3] == 0 {
            continue;
        }
        assert_eq!(px[3], px[0].max(px[1]).max(px[2]));
        if px[0] != px[1] || px[1] != px[2] {
            fringe = true;
        }
    }
    assert!(fringe, "per-channel coverage should differ at edges");

    core.set_subpixel_text(false);
    let kinds = frame(&mut core);
    assert!(kinds.contains(&QuadKind::GlyphMask));
    assert!(!kinds.contains(&QuadKind::GlyphSubpixel));
}
