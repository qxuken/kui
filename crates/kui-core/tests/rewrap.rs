//! End-to-end resize regression: the same text through a live `Core` (real
//! shaping caches) must rewrap when consecutive frames shrink the viewport —
//! the path a window resize exercises.

use kui_core::{Core, NodeSpec, Size, TextStyle};

const PARA: &str = "The whole paragraph is shaped together, which means wrapping \
crosses style boundaries correctly instead of breaking at every run.";

fn glyph_bottom(core: &mut Core, viewport_w: f32) -> f32 {
    let mut ui = core.frame(Size::new(viewport_w, 600.0), 1.0);
    ui.configure_root(NodeSpec::column().grow_width().pad(10.0));
    ui.text(PARA, TextStyle::new(16.0));
    ui.finish();
    let (dl, _) = core.output();
    dl.quads
        .iter()
        .filter(|q| !matches!(q.kind, kui_core::QuadKind::Solid))
        .map(|q| q.rect.y + q.rect.h)
        .fold(0.0f32, f32::max)
}

#[test]
fn text_rewraps_across_frames_when_viewport_shrinks() {
    let mut core = Core::new();
    let wide = glyph_bottom(&mut core, 800.0);
    let narrow = glyph_bottom(&mut core, 300.0);
    let wide_again = glyph_bottom(&mut core, 800.0);

    assert!(wide > 0.0, "no glyphs emitted");
    assert!(
        narrow > wide * 1.5,
        "narrow viewport should wrap to more lines: wide={wide}, narrow={narrow}"
    );
    assert!(
        (wide_again - wide).abs() < 1.0,
        "growing back should restore the original wrap: {wide} vs {wide_again}"
    );
}
