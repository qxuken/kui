//! A faded card against the exact group result, on the machine's GPU: a
//! card fading in or out with `opacity` must look like the card drawn
//! whole and then faded, not like its pieces faded one by one with its
//! shadow showing through its middle.
//!
//! The exact result needs no layer to compute. Over a backdrop `B`, a
//! group `L` faded by `α` is `α·L + B·(1 − α·L.a)`; the frame with the
//! card opaque is `L + B·(1 − L.a)` and the frame with it gone is `B`, so
//! the exact result is the two frames mixed by `α`. Three captures, per
//! pixel. Skipped, saying so, where there is no adapter.
//!
//! Before the shadow was knocked out of its node, the card here differed
//! from that by up to 50 levels over most of its body (measured with the
//! same three captures through Node, 2026-10-23): the shadow under the
//! card, faded with it, darkened everything the card covered.

use kui_core::{Color, Core, NodeSpec, Shadow, Size};
use kui_wgpu::{Capture, Gpu, Renderer, wgpu};

fn renderer() -> Option<Renderer> {
    let gpu = match pollster::block_on(Gpu::windowless()) {
        Ok(g) => g,
        Err(e) => {
            eprintln!("skipped: no adapter ({e})");
            return None;
        }
    };
    let mut r = Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm).expect("a renderer");
    r.clear_color = wgpu::Color::WHITE;
    Some(r)
}

/// A 200x120 page at 2x, beige, with a card at (30, 30): white, rounded,
/// bordered, casting a soft shadow down, at `opacity`, and `bg` its fill.
fn shot(r: &mut Renderer, opacity: f32, bg: Color, shadow: bool) -> Capture {
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(200.0, 120.0), 2.0);
    ui.configure_root(NodeSpec::column().fill().pad(30.0).bg(Color::hex(0xf2efe6ff)));
    let mut card = NodeSpec::column()
        .size(120.0, 60.0)
        .bg(bg)
        .radius(10.0)
        .border(1.0, Color::hex(0xc8c2b4ff))
        .opacity(opacity);
    if shadow {
        card = card.shadow(Shadow {
            color: Color::hex(0x00000066),
            dx: 0.0,
            dy: 6.0,
            blur: 18.0,
            spread: 0.0,
        });
    }
    ui.leaf_keyed("card", card);
    ui.finish();
    let (dl, atlas) = core.output();
    r.capture(dl, atlas).expect("a capture")
}

/// The most any channel of any pixel of `a` differs from `b`.
fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    a.iter().zip(b).map(|(x, y)| x.abs_diff(*y)).max().unwrap_or(0)
}

#[test]
fn a_faded_card_with_a_shadow_is_the_card_faded_whole() {
    let Some(mut r) = renderer() else { return };
    let white = Color::hex(0xffffffff);
    for alpha in [0.25f32, 0.5, 0.8] {
        let faded = shot(&mut r, alpha, white, true);
        let whole = shot(&mut r, 1.0, white, true);
        let gone = shot(&mut r, 0.0, white, true);
        let exact: Vec<u8> = gone
            .rgba
            .iter()
            .zip(&whole.rgba)
            .map(|(&b, &l)| (b as f32 * (1.0 - alpha) + l as f32 * alpha).round() as u8)
            .collect();
        // Rounding aside (two 8-bit blends against a mix of two 8-bit
        // frames), every pixel is within 2 levels, but for the card's own
        // antialiased edge: there the shadow and the card each cover part
        // of one pixel and are blended one after the other, which no
        // per-quad fade makes the same as one blend of the group. A few
        // levels, on a one-pixel ring; a layer is what would close it.
        let w = faded.width as usize;
        let (mut over, mut worst) = (0usize, 0u8);
        for (a, b) in faded.rgba.chunks(4).zip(exact.chunks(4)) {
            let m = (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap_or(0);
            worst = worst.max(m);
            over += usize::from(m > 2);
        }
        let ring = 2 * (240 + 120) + 64;
        assert!(worst <= 6, "at {alpha}: {worst} levels off the card faded whole");
        assert!(
            over <= ring,
            "at {alpha}: {over} pixels more than 2 levels off, more than its edge ({ring}); {w} wide"
        );
    }
}

/// Not only a fade: a card whose own fill is translucent shows the page
/// through it, not its shadow — the same card with no shadow, inside it.
#[test]
fn a_translucent_card_does_not_show_its_own_shadow() {
    let Some(mut r) = renderer() else { return };
    let glass = Color::hex(0xffffff80);
    let with = shot(&mut r, 1.0, glass, true);
    let without = shot(&mut r, 1.0, glass, false);
    // The card is (60, 60)..(300, 180) in physical px; inside its border
    // and corners the two frames are the same.
    let w = with.width as usize;
    for y in 70..170 {
        let span = (y * w + 70) * 4..(y * w + 290) * 4;
        let d = max_diff(&with.rgba[span.clone()], &without.rgba[span]);
        assert!(d <= 1, "row {y}: the shadow shows through by {d}");
    }
    // And below the card, the shadow is still there.
    let at = |c: &Capture, x: usize, y: usize| c.rgba[(y * w + x) * 4];
    assert!(at(&with, 180, 192) + 20 < at(&without, 180, 192), "a shadow under the card");
}
