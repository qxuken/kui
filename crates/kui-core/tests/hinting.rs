//! Glyphs at 2× are not hinted (backlog RG166): a face's hinting program
//! snaps each glyph by its own instructions, and Helvetica Neue's put `п`
//! a pixel above `н` in one word — two letters whose flat tops sit on the
//! same x-height in the outline.

use kui_core::geom::Size;
use kui_core::{Core, NodeSpec, QuadKind, TextStyle};

/// The top of every glyph's bitmap in `text`, `size` px at `scale`.
fn tops(text: &str, size: f32, scale: f32) -> Vec<f32> {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(400.0, 100.0), scale);
    ui.configure_root(NodeSpec::column().fill());
    ui.text(text, TextStyle::new(size));
    ui.finish();
    let (list, _) = core.output();
    list.quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
        .map(|q| q.rect.y)
        .collect()
}

#[test]
fn letters_of_one_x_height_share_a_top_at_2x() {
    // Flat-topped letters that rise to the x-height and no further: in
    // the outline one height, so one row. Round ones overshoot it by a
    // fraction a face chooses, which may cross a row on its own.
    for size in [11.0, 12.0, 13.0, 14.0] {
        for word in ["пнштки", "xvwzu"] {
            let t = tops(word, size, 2.0);
            assert_eq!(t.len(), word.chars().count(), "{size} {word}: {t:?}");
            assert!(t.iter().all(|&y| y == t[0]), "{size} {word}: {t:?}");
        }
    }
}

/// Every glyph's bitmap rect in `text` at 1×, drawn by a core told it runs
/// on `platform` (backlog F220).
fn rects_on(platform: kui_core::Platform, text: &str) -> Vec<(f32, f32, f32, f32)> {
    let mut core = Core::new();
    core.set_platform(platform);
    let mut ui = core.frame(Size::new(400.0, 100.0), 1.0);
    ui.configure_root(NodeSpec::column().fill());
    ui.text(text, TextStyle::new(13.0));
    ui.finish();
    let (list, _) = core.output();
    list.quads
        .iter()
        .filter(|q| matches!(q.kind, QuadKind::GlyphMask | QuadKind::GlyphSubpixel))
        .map(|q| (q.rect.x, q.rect.y, q.rect.w, q.rect.h))
        .collect()
}

/// Hinting is the core's platform's, not the build's: a run pinned to
/// macOS draws its 1× glyphs unhinted, as a Mac does, on any machine.
#[test]
fn hinting_at_1x_is_the_pinned_platform_s() {
    let text = "Hamburgefonstiv 0123456789";
    let mac = rects_on(kui_core::Platform::MacOs, text);
    let win = rects_on(kui_core::Platform::Windows, text);
    assert_eq!(mac.len(), win.len());
    assert_ne!(mac, win, "hinted and unhinted glyphs drew alike");
}
