//! The runner's image encoder (`kui_native::encode_image`,
//! `AnimationEncoder`, ADR 0054): straight RGBA a capture hands out, as a
//! still in each of the four formats and as an animated PNG or GIF, read
//! back through the runner's own decoder.
//!
//! Checked: what the docs promise of each format. A PNG and a WebP read
//! back pixel for pixel; a JPEG and a GIF at the size written; a GIF's
//! pixel is opaque, or transparent where its alpha was zero, and exact
//! where the picture holds 256 colours or fewer (the encoder quantizes
//! only past them). An animated PNG reads back as the frames added, in
//! order — a frame that changed nothing, or too short to show, folded
//! away, one shown past what a delay holds read back as more than one, the
//! last always shown — every frame's time a millisecond or more and the
//! whole the frames' own to the millisecond each.

use arbitrary::{Arbitrary, Result, Unstructured};
use kui_native::{
    AnimationEncoder, AnimationEncoding, ImageEncoding, decode_animation, decode_image,
    encode_image,
};

#[derive(Arbitrary, Debug)]
struct Input {
    size: Dims,
    /// Which still format, and a JPEG's quality.
    still: u8,
    /// Whether the animation is a GIF, rather than an animated PNG.
    gif: bool,
    loops: Option<u16>,
    /// The first frame's pixels, as many as there are (zeros past them).
    first: Vec<u8>,
    frames: Vec<Frame>,
}

/// A picture's size: small most of the time, so an input is quick, and a
/// strip now and then.
#[derive(Debug, Clone, Copy)]
struct Dims(u32, u32);

impl<'a> Arbitrary<'a> for Dims {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        Ok(match u.int_in_range(0u8..=7)? {
            0 => Dims(1 + u.int_in_range(0..=999)?, 1 + u.int_in_range(0..=3)?),
            1 => Dims(1 + u.int_in_range(0..=3)?, 1 + u.int_in_range(0..=999)?),
            _ => Dims(1 + u.int_in_range(0..=31)?, 1 + u.int_in_range(0..=31)?),
        })
    }
}

/// A frame: the one before with rectangles painted on it, and its time.
#[derive(Arbitrary, Debug)]
struct Frame {
    paint: Vec<Rect>,
    secs: Secs,
}

#[derive(Arbitrary, Debug)]
struct Rect {
    x: u16,
    y: u16,
    w: u8,
    h: u8,
    rgba: [u8; 4],
}

/// A frame's time: the ones around the formats' shortest, a frame at
/// 60 Hz, past what a 16-bit delay holds, and a few no recorder sends.
#[derive(Debug, Clone, Copy)]
struct Secs(f64);

impl<'a> Arbitrary<'a> for Secs {
    fn arbitrary(u: &mut Unstructured<'a>) -> Result<Self> {
        const SOME: &[f64] = &[
            0.0,
            0.0004,
            0.0005,
            0.001,
            0.0015,
            0.005,
            0.01,
            0.0149,
            0.015,
            0.019,
            0.02,
            1.0 / 60.0,
            0.1,
            1.0,
            65.5,
            700.0,
            -0.001,
            f64::NAN,
            f64::INFINITY,
        ];
        Ok(Secs(if u.ratio(3u8, 4u8)? {
            SOME[u.choose_index(SOME.len())?]
        } else {
            f64::from(u.arbitrary::<u16>()?) / 1000.0
        }))
    }
}

pub fn run(data: &[u8]) {
    let Ok(input) = Input::arbitrary_take_rest(Unstructured::new(data)) else {
        return;
    };
    let Dims(w, h) = input.size;
    let len = w as usize * h as usize * 4;
    let mut px = input.first.clone();
    px.resize(len, 0);
    still(&input, w, h, &px);
    animation(&input, w, h, px);
}

fn still(input: &Input, w: u32, h: u32, px: &[u8]) {
    let format = match input.still % 4 {
        0 => ImageEncoding::Png,
        1 => ImageEncoding::Jpeg {
            quality: input.still / 4,
        },
        2 => ImageEncoding::WebP,
        _ => ImageEncoding::Gif,
    };
    let bytes = encode_image(format, w, h, px).expect("a picture of the size encodes");
    let back = decode_image(&bytes).unwrap_or_else(|e| panic!("a {format:?} reads back: {e}"));
    assert_eq!((back.width, back.height), (w, h), "a {format:?}'s size");
    match format {
        ImageEncoding::Png | ImageEncoding::WebP => {
            if let Some(i) = first_difference(&back.rgba, px) {
                panic!(
                    "a {format:?} {w}x{h} reads back {:?} for {:?} at pixel {i}",
                    &back.rgba[i * 4..i * 4 + 4],
                    &px[i * 4..i * 4 + 4]
                );
            }
        }
        ImageEncoding::Gif => gif_pixels(&back.rgba, px, colours(px) <= 256, "a still GIF"),
        ImageEncoding::Jpeg { .. } => {}
    }
}

fn animation(input: &Input, w: u32, h: u32, first: Vec<u8>) {
    let format = if input.gif {
        AnimationEncoding::Gif
    } else {
        AnimationEncoding::Png
    };
    let loops = input.loops.map(u32::from);
    let mut enc = AnimationEncoder::new(format, w, h, loops).expect("an encoder of the size");
    // Every frame added, with its time.
    let mut added: Vec<(Vec<u8>, f64)> = Vec::new();
    let mut px = first;
    for (i, f) in input.frames.iter().take(12).enumerate() {
        if i > 0 {
            for r in f.paint.iter().take(4) {
                paint(&mut px, w, h, r);
            }
        }
        let secs = f.secs.0;
        let ok = enc.add(&px, secs);
        assert_eq!(
            ok.is_ok(),
            secs.is_finite() && secs >= 0.0,
            "a frame of {secs} s: {ok:?}"
        );
        if ok.is_ok() {
            added.push((px.clone(), secs));
        }
    }
    let Ok(bytes) = enc.finish() else {
        assert!(added.is_empty(), "{} frames encode", added.len());
        return;
    };
    assert!(!added.is_empty(), "no frame encoded");
    let a = decode_animation(&bytes).unwrap_or_else(|e| panic!("a {format:?} reads back: {e}"));
    assert_eq!((a.width, a.height), (w, h), "a {format:?}'s size");
    if format == AnimationEncoding::Gif {
        // Each frame read back is one added, in order, as a GIF shows it;
        // a frame shown long may read back as more than one.
        let mut from = 0;
        for (n, f) in a.frames.iter().enumerate() {
            let Some(i) = (from..added.len()).find(|&i| gif_like(&f.rgba, &added[i].0)) else {
                // Past 256 colours the frame is quantized: nothing to match.
                if added[from..].iter().any(|(p, _)| colours(p) > 256) {
                    return;
                }
                panic!(
                    "GIF frame {n} of {} is no frame added from {from}",
                    a.frames.len()
                );
            };
            from = i;
            assert!(f.delay >= 0.02, "GIF frame {n} shows for {} s", f.delay);
        }
        return;
    }
    if a.frames.len() == 1 {
        // A still PNG: no time of its own.
        assert_eq!(a.frames[0].rgba, added.last().unwrap().0, "a one-frame PNG");
        return;
    }
    let mut from = 0;
    for (n, f) in a.frames.iter().enumerate() {
        let Some(i) = (from..added.len()).find(|&i| added[i].0 == f.rgba) else {
            panic!(
                "APNG frame {n} of {} is no frame added from {from} of {}",
                a.frames.len(),
                added.len()
            );
        };
        from = i;
        assert!(
            f.delay >= 0.001 - 1e-9,
            "APNG frame {n} shows for {} s",
            f.delay
        );
    }
    assert_eq!(
        a.frames.last().unwrap().rgba,
        added.last().unwrap().0,
        "the last APNG frame is the last added"
    );
    // The whole, to the millisecond a frame, while it is under the 4.6
    // hours one frame may show for.
    let want: f64 = added.iter().map(|(_, s)| s).sum();
    if want < 16_000.0 {
        let got: f64 = a.frames.iter().map(|f| f.delay).sum();
        let slack = 0.001 * a.frames.len() as f64 + 0.0005;
        assert!(
            got >= want - 0.0005 - 1e-9 && got <= want + slack + 1e-9,
            "{} APNG frames show for {got} s, the {} added for {want} s",
            a.frames.len(),
            added.len()
        );
    }
}

fn paint(px: &mut [u8], w: u32, h: u32, r: &Rect) {
    let (x0, y0) = (u32::from(r.x) % w, u32::from(r.y) % h);
    let x1 = (x0 + 1 + u32::from(r.w)).min(w);
    let y1 = (y0 + 1 + u32::from(r.h)).min(h);
    for y in y0..y1 {
        for x in x0..x1 {
            let at = (y * w + x) as usize * 4;
            px[at..at + 4].copy_from_slice(&r.rgba);
        }
    }
}

fn first_difference(a: &[u8], b: &[u8]) -> Option<usize> {
    a.as_chunks::<4>()
        .0
        .iter()
        .zip(b.as_chunks::<4>().0.iter())
        .position(|(p, q)| p != q)
}

/// How many colours a GIF would see: every transparent pixel one.
fn colours(px: &[u8]) -> usize {
    let mut seen: std::collections::BTreeSet<[u8; 3]> = Default::default();
    let mut clear = false;
    for p in px.as_chunks::<4>().0.iter() {
        if p[3] == 0 {
            clear = true;
        } else {
            seen.insert([p[0], p[1], p[2]]);
        }
    }
    seen.len() + usize::from(clear)
}

/// Whether `got` is `want` as a GIF holds it: a pixel opaque in its colour,
/// or transparent where its alpha was zero.
fn gif_like(got: &[u8], want: &[u8]) -> bool {
    got.as_chunks::<4>()
        .0
        .iter()
        .zip(want.as_chunks::<4>().0.iter())
        .all(|(g, p)| {
            if p[3] == 0 {
                g[3] == 0
            } else {
                *g == [p[0], p[1], p[2], 255]
            }
        })
}

fn gif_pixels(got: &[u8], want: &[u8], exact: bool, what: &str) {
    for (i, (g, p)) in got
        .as_chunks::<4>()
        .0
        .iter()
        .zip(want.as_chunks::<4>().0.iter())
        .enumerate()
    {
        let opaque = p[3] != 0;
        assert_eq!(
            g[3],
            if opaque { 255 } else { 0 },
            "{what}: pixel {i} reads back {g:?} for {p:?}"
        );
        if exact && opaque {
            assert_eq!(
                &g[..3],
                &p[..3],
                "{what}: pixel {i} reads back {g:?} for {p:?}"
            );
        }
    }
}
