//! An image drawn smaller is drawn from a level the core halves
//! (`docs/adr/0044-an-image-drawn-smaller-is-drawn-from-a-level.md`,
//! backlog V6).
//!
//! Level 0 is the image; level *n* + 1 is level *n* with each 2×2 block
//! averaged into one texel, `⌈w/2⌉ × ⌈h/2⌉`, the last row and column of
//! an odd size repeated. The average is taken in linear light with the
//! alpha premultiplied, so a level is neither darker than its source nor
//! fringed where a transparent edge meets colour. Which level a quad
//! draws is [`level_for`]: the deepest still at least as many texels as
//! pixels on both axes.

use std::sync::OnceLock;

/// sRGB byte to linear light, `0..=1`.
fn to_linear() -> &'static [f32; 256] {
    static T: OnceLock<[f32; 256]> = OnceLock::new();
    T.get_or_init(|| {
        std::array::from_fn(|i| {
            let c = i as f32 / 255.0;
            if c <= 0.04045 {
                c / 12.92
            } else {
                ((c + 0.055) / 1.055).powf(2.4)
            }
        })
    })
}

/// Entries in [`to_srgb`]: linear light quantized to 12 bits, fine
/// enough that every sRGB byte above the darkest few has its own step.
const ENCODE_STEPS: usize = 4096;

/// Linear light quantized to `ENCODE_STEPS` back to an sRGB byte.
fn to_srgb() -> &'static [u8; ENCODE_STEPS] {
    static T: OnceLock<[u8; ENCODE_STEPS]> = OnceLock::new();
    T.get_or_init(|| {
        std::array::from_fn(|i| {
            let l = i as f32 / (ENCODE_STEPS - 1) as f32;
            let c = if l <= 0.003_130_8 {
                l * 12.92
            } else {
                1.055 * l.powf(1.0 / 2.4) - 0.055
            };
            (c * 255.0).round().clamp(0.0, 255.0) as u8
        })
    })
}

/// The size of the level below a `w × h` one.
pub fn halved(w: u32, h: u32) -> (u32, u32) {
    (w.div_ceil(2).max(1), h.div_ceil(2).max(1))
}

/// How many levels below level 0 a `w × h` image has: halving stops at
/// 1×1.
pub fn depth(w: u32, h: u32) -> u8 {
    let mut n = 0;
    let (mut w, mut h) = (w, h);
    while w > 1 || h > 1 {
        (w, h) = halved(w, h);
        n += 1;
    }
    n
}

/// Level *n* + 1 from level *n*'s straight RGBA, `w × h × 4` bytes: each
/// 2×2 block averaged in linear light with the alpha premultiplied, the
/// colour taken back to straight alpha. Returns the new size and pixels.
pub fn halve(w: u32, h: u32, rgba: &[u8]) -> (u32, u32, Vec<u8>) {
    debug_assert_eq!(rgba.len(), w as usize * h as usize * 4);
    let (nw, nh) = halved(w, h);
    let (w, h) = (w as usize, h as usize);
    let lin = to_linear();
    let enc = to_srgb();
    let scale = (ENCODE_STEPS - 1) as f32;
    let mut out = vec![0u8; nw as usize * nh as usize * 4];
    let quarter = scale / 4.0;
    for y in 0..nh as usize {
        let rows = [2 * y, (2 * y + 1).min(h - 1)];
        let (r0, r1) = (
            &rgba[rows[0] * w * 4..][..w * 4],
            &rgba[rows[1] * w * 4..][..w * 4],
        );
        let out_row = &mut out[y * nw as usize * 4..][..nw as usize * 4];
        for (x, o) in out_row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
            let (c0, c1) = (8 * x, (8 * x + 4).min(w * 4 - 4));
            let block = [
                &r0[c0..c0 + 4],
                &r0[c1..c1 + 4],
                &r1[c0..c0 + 4],
                &r1[c1..c1 + 4],
            ];
            // Opaque, as a photo is: the plain mean, no weights.
            if block.iter().all(|p| p[3] == 255) {
                for c in 0..3 {
                    let sum: f32 = block.iter().map(|p| lin[p[c] as usize]).sum();
                    o[c] = enc[(sum * quarter) as usize];
                }
                o[3] = 255;
                continue;
            }
            let (mut r, mut g, mut b, mut a) = (0.0f32, 0.0f32, 0.0f32, 0u32);
            for p in block {
                let pa = p[3] as f32;
                r += lin[p[0] as usize] * pa;
                g += lin[p[1] as usize] * pa;
                b += lin[p[2] as usize] * pa;
                a += p[3] as u32;
            }
            if a > 0 {
                // Back to straight alpha: the premultiplied sums over the
                // alpha sum, the weights of the texels that had any.
                let inv = scale / a as f32;
                o[0] = enc[(r * inv) as usize];
                o[1] = enc[(g * inv) as usize];
                o[2] = enc[(b * inv) as usize];
                o[3] = ((a + 2) / 4) as u8;
            }
        }
    }
    (nw, nh, out)
}

/// The level a quad draws: the deepest whose texels still cover the
/// pixels on both axes. `texels` is the texel rect `fit` picked at level
/// 0, `pixels` the drawn size in physical px (any scale the quad is
/// drawn through multiplied in), `deepest` the chain's last level.
/// `⌊log₂ r⌋` for `r` the smaller of the two ratios, so a bilinear
/// sample inside the level minifies by less than two and a level never
/// magnifies.
pub fn level_for(texels: (f32, f32), pixels: (f32, f32), deepest: u8) -> u8 {
    let (tw, th) = texels;
    let (pw, ph) = pixels;
    if !(pw > 0.0 && ph > 0.0 && tw > 0.0 && th > 0.0) {
        return 0;
    }
    let r = (tw / pw).min(th / ph);
    // NaN is not at least 2 either.
    if r.partial_cmp(&2.0).is_none_or(|o| o.is_lt()) {
        return 0;
    }
    // `r` is at most a texture's side over a sliver of a pixel; the cast
    // saturates, and `deepest` bounds it.
    (r.log2().floor() as u32).min(deepest as u32) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn px(c: [u8; 4], n: usize) -> Vec<u8> {
        c.iter().copied().cycle().take(n * 4).collect()
    }

    #[test]
    fn a_flat_image_halves_to_itself() {
        let (w, h, out) = halve(4, 2, &px([200, 100, 30, 255], 8));
        assert_eq!((w, h), (2, 1));
        assert_eq!(out, px([200, 100, 30, 255], 2));
    }

    /// Black and white averaged in linear light is sRGB 188, not 128: a
    /// level in gamma space would darken every edge a photo has.
    #[test]
    fn the_average_is_in_linear_light() {
        let rgba = [0, 0, 0, 255, 255, 255, 255, 255].repeat(2);
        let (_, _, out) = halve(2, 2, &rgba);
        assert!((187..=189).contains(&out[0]), "{out:?}");
        assert_eq!(out[3], 255);
    }

    /// A transparent texel's colour does not bleed into its neighbour's:
    /// red beside clear is red at half alpha, not dark red.
    #[test]
    fn the_alpha_is_premultiplied() {
        let rgba = [255, 0, 0, 255, 0, 0, 0, 0].repeat(2);
        let (_, _, out) = halve(2, 2, &rgba);
        assert_eq!(&out[..3], &[255, 0, 0]);
        assert_eq!(out[3], 128);
    }

    /// An odd side repeats its last row and column: a 3×1 image is two
    /// texels, the second its last texel alone.
    #[test]
    fn an_odd_side_repeats_its_edge() {
        let rgba = [10, 10, 10, 255, 10, 10, 10, 255, 250, 250, 250, 255];
        let (w, h, out) = halve(3, 1, &rgba);
        assert_eq!((w, h), (2, 1));
        assert_eq!(&out[4..], &[250, 250, 250, 255]);
    }

    #[test]
    fn the_chain_ends_at_one_texel() {
        assert_eq!(depth(1, 1), 0);
        assert_eq!(depth(2, 1), 1);
        assert_eq!(depth(4032, 3024), 12);
        assert_eq!(halved(1, 7), (1, 4));
    }

    #[test]
    fn the_level_never_magnifies() {
        // At most 2:1 is level 0.
        assert_eq!(level_for((1000.0, 1000.0), (600.0, 600.0), 10), 0);
        assert_eq!(level_for((1000.0, 1000.0), (500.0, 500.0), 10), 1);
        assert_eq!(level_for((1000.0, 1000.0), (260.0, 260.0), 10), 1);
        assert_eq!(level_for((1000.0, 1000.0), (250.0, 250.0), 10), 2);
        // The smaller ratio decides: stretched wide, the width is
        // barely minified and keeps its texels.
        assert_eq!(level_for((1000.0, 1000.0), (900.0, 100.0), 10), 0);
        // Bounded by the chain, and nothing for an empty draw.
        assert_eq!(level_for((1000.0, 1000.0), (0.5, 0.5), 3), 3);
        assert_eq!(level_for((1000.0, 1000.0), (0.0, 10.0), 10), 0);
        assert_eq!(level_for((1000.0, 1000.0), (f32::NAN, 10.0), 10), 0);
    }
}
