//! App icons for `kui pack` (ADR 0068, decision 13): one PNG from the app
//! becomes the macOS `.icns`, the Windows `.ico` and the Linux PNGs, so an
//! app names a single `icon.png` and every target gets its own format.
//!
//! The formats are written by hand (both are a short header and a table of
//! PNGs), and resizing is a Lanczos-3 filter here rather than the `image`
//! crate's, which kui does not otherwise build with resizing. Each size is
//! filtered from the source, not from the next size up, and in linear light
//! with premultiplied alpha, so edges neither darken nor fringe.
//!
//! An app that names no icon gets [`default_icon`], drawn here rather than
//! shipped as a file.

// Some of what is here (a reader's conveniences, the parsing back) only
// the tests and `kui pack` itself use; it stays as the format's whole API.
#![cfg_attr(not(test), allow(dead_code))]

// `kui pack` is wired to this module in a later step; until then its API
// is only reached from the tests.

use std::collections::HashMap;

/// An 8-bit RGBA image with straight (not premultiplied) alpha.
#[derive(Clone, Debug, PartialEq)]
pub struct Rgba {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

impl Rgba {
    pub fn new(width: u32, height: u32) -> Rgba {
        Rgba {
            width,
            height,
            pixels: vec![0; width as usize * height as usize * 4],
        }
    }
}

/// A decoded icon and what is worth telling the user about it.
pub struct Loaded {
    /// Always square: a non-square source is centred on a transparent square.
    pub image: Rgba,
    /// Not errors — the icon is used anyway — but `kui pack` prints them:
    /// a non-square source, and one under 1024 px, whose largest macOS
    /// sizes are scaled up and come out soft.
    pub warnings: Vec<String>,
}

/// The size an icon should be: the largest the `.icns` holds.
pub const BEST_SIZE: u32 = 1024;

/// Decodes a PNG of any colour type and depth into a square RGBA icon.
pub fn load_png(bytes: &[u8]) -> Result<Loaded, String> {
    let mut dec = png::Decoder::new(std::io::Cursor::new(bytes));
    dec.set_transformations(png::Transformations::normalize_to_color8());
    let mut reader = dec.read_info().map_err(|e| format!("not a PNG: {e}"))?;
    let size = reader.output_buffer_size().ok_or("the PNG is too large")?;
    let mut buf = vec![0; size];
    let info = reader
        .next_frame(&mut buf)
        .map_err(|e| format!("the PNG does not decode: {e}"))?;
    let (w, h) = (info.width, info.height);
    if w == 0 || h == 0 {
        return Err("the PNG is empty".into());
    }
    let n = w as usize * h as usize;
    let mut px = Vec::with_capacity(n * 4);
    let rows = buf.chunks(info.line_size).take(h as usize);
    for row in rows {
        match info.color_type {
            png::ColorType::Rgba => px.extend_from_slice(&row[..w as usize * 4]),
            png::ColorType::Rgb => row[..w as usize * 3]
                .chunks(3)
                .for_each(|c| px.extend_from_slice(&[c[0], c[1], c[2], 255])),
            png::ColorType::GrayscaleAlpha => row[..w as usize * 2]
                .chunks(2)
                .for_each(|c| px.extend_from_slice(&[c[0], c[0], c[0], c[1]])),
            png::ColorType::Grayscale => row[..w as usize]
                .iter()
                .for_each(|&g| px.extend_from_slice(&[g, g, g, 255])),
            png::ColorType::Indexed => return Err("the PNG's palette was not expanded".into()),
        }
    }
    let mut image = Rgba {
        width: w,
        height: h,
        pixels: px,
    };
    let mut warnings = Vec::new();
    if w != h {
        warnings.push(format!(
            "the icon is {w}x{h}; it is centred on a transparent square"
        ));
        image = squared(&image);
    }
    if image.width < BEST_SIZE {
        warnings.push(format!(
            "the icon is {} px; sizes above that are scaled up and look soft (1024 px is best)",
            image.width
        ));
    }
    Ok(Loaded { image, warnings })
}

fn squared(src: &Rgba) -> Rgba {
    let side = src.width.max(src.height);
    let mut out = Rgba::new(side, side);
    let (dx, dy) = ((side - src.width) / 2, (side - src.height) / 2);
    let row = src.width as usize * 4;
    for y in 0..src.height as usize {
        let to = ((y + dy as usize) * side as usize + dx as usize) * 4;
        out.pixels[to..to + row].copy_from_slice(&src.pixels[y * row..(y + 1) * row]);
    }
    out
}

fn srgb_to_linear(c: u8) -> f32 {
    let c = c as f32 / 255.0;
    if c <= 0.04045 {
        c / 12.92
    } else {
        ((c + 0.055) / 1.055).powf(2.4)
    }
}

fn linear_to_srgb(c: f32) -> u8 {
    let c = c.clamp(0.0, 1.0);
    let s = if c <= 0.003_130_8 {
        c * 12.92
    } else {
        1.055 * c.powf(1.0 / 2.4) - 0.055
    };
    (s * 255.0 + 0.5) as u8
}

fn lanczos3(x: f32) -> f32 {
    let x = x.abs();
    if x < 1e-6 {
        1.0
    } else if x < 3.0 {
        let px = std::f32::consts::PI * x;
        3.0 * px.sin() * (px / 3.0).sin() / (px * px)
    } else {
        0.0
    }
}

/// For each output pixel along one axis: the first source index and the
/// normalised weights from there on.
fn weights(src: u32, dst: u32) -> Vec<(usize, Vec<f32>)> {
    let scale = src as f32 / dst as f32;
    // Downscaling widens the kernel so every source pixel counts;
    // upscaling keeps it at its natural width.
    let filter_scale = scale.max(1.0);
    let support = 3.0 * filter_scale;
    (0..dst)
        .map(|i| {
            let center = (i as f32 + 0.5) * scale;
            let lo = ((center - support).floor() as i64).max(0) as usize;
            let hi = ((center + support).ceil() as i64).min(src as i64) as usize;
            let mut w: Vec<f32> = (lo..hi)
                .map(|j| lanczos3((j as f32 + 0.5 - center) / filter_scale))
                .collect();
            let sum: f32 = w.iter().sum();
            if sum.abs() > 1e-8 {
                w.iter_mut().for_each(|v| *v /= sum);
            }
            (lo, w)
        })
        .collect()
}

/// `src` scaled to `size`×`size` (a square source stays undistorted).
pub fn resize(src: &Rgba, size: u32) -> Rgba {
    resize_to(src, size, size)
}

fn resize_to(src: &Rgba, dw: u32, dh: u32) -> Rgba {
    if src.width == dw && src.height == dh {
        return src.clone();
    }
    let (sw, sh) = (src.width as usize, src.height as usize);
    // Premultiplied linear light.
    let lut: Vec<f32> = (0..=255u8).map(srgb_to_linear).collect();
    let lin: Vec<[f32; 4]> = src
        .pixels
        .as_chunks::<4>()
        .0
        .iter()
        .map(|p| {
            let a = p[3] as f32 / 255.0;
            [
                lut[p[0] as usize] * a,
                lut[p[1] as usize] * a,
                lut[p[2] as usize] * a,
                a,
            ]
        })
        .collect();
    // Horizontal pass: sw×sh → dw×sh.
    let wx = weights(src.width, dw);
    let mut mid = vec![[0f32; 4]; dw as usize * sh];
    for y in 0..sh {
        let row = &lin[y * sw..(y + 1) * sw];
        for (x, (lo, w)) in wx.iter().enumerate() {
            let mut acc = [0f32; 4];
            for (k, &wk) in w.iter().enumerate() {
                let p = row[lo + k];
                for c in 0..4 {
                    acc[c] += p[c] * wk;
                }
            }
            mid[y * dw as usize + x] = acc;
        }
    }
    // Vertical pass: dw×sh → dw×dh.
    let wy = weights(src.height, dh);
    let mut out = Rgba::new(dw, dh);
    for (y, (lo, w)) in wy.iter().enumerate() {
        for x in 0..dw as usize {
            let mut acc = [0f32; 4];
            for (k, &wk) in w.iter().enumerate() {
                let p = mid[(lo + k) * dw as usize + x];
                for c in 0..4 {
                    acc[c] += p[c] * wk;
                }
            }
            // Lanczos rings past 0 and 1; clamp, and keep colour within alpha.
            let a = acc[3].clamp(0.0, 1.0);
            let o = (y * dw as usize + x) * 4;
            if a > 0.0 {
                for (dst, v) in out.pixels[o..o + 3].iter_mut().zip(acc) {
                    *dst = linear_to_srgb(v.clamp(0.0, a) / a);
                }
            }
            out.pixels[o + 3] = (a * 255.0 + 0.5) as u8;
        }
    }
    out
}

/// A PNG of `img` as it is.
pub fn encode_png(img: &Rgba) -> Vec<u8> {
    let mut out = Vec::new();
    let mut enc = png::Encoder::new(&mut out, img.width, img.height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.set_compression(png::Compression::High);
    // Writing RGBA8 of the declared size into a Vec cannot fail.
    let mut w = enc.write_header().expect("png header");
    w.write_image_data(&img.pixels).expect("png data");
    w.finish().expect("png end");
    out
}

/// A PNG of `img` scaled to `size`: the Linux icons (256 and 512).
pub fn png_of(img: &Rgba, size: u32) -> Vec<u8> {
    encode_png(&resize(img, size))
}

/// PNGs of each size, each filtered once from the source.
fn pngs(img: &Rgba, sizes: &[u32]) -> HashMap<u32, Vec<u8>> {
    let mut out = HashMap::new();
    for &s in sizes {
        out.entry(s).or_insert_with(|| png_of(img, s));
    }
    out
}

/// The `.icns` types and their pixel sizes. The 16 and 32 px entries are
/// `ic04`/`ic05` in run-length ARGB, as Apple's `iconutil` writes them:
/// the `icp4`/`icp5` types also allow PNG, but `iconutil` on macOS 27 reads
/// a PNG there as raw pixels and shows noise. Every other entry is a PNG.
const ICNS: [(&[u8; 4], u32); 10] = [
    (b"ic04", 16),
    (b"ic05", 32),
    (b"ic11", 32), // 16@2x
    (b"ic12", 64), // 32@2x
    (b"ic07", 128),
    (b"ic13", 256), // 128@2x
    (b"ic08", 256),
    (b"ic14", 512), // 256@2x
    (b"ic09", 512),
    (b"ic10", 1024), // 512@2x
];

/// The icns run-length code for one channel plane: a control byte below
/// 0x80 copies the next `c + 1` bytes; from 0x80 it repeats the next byte
/// `c - 0x80 + 3` times.
fn icns_rle(plane: &[u8], out: &mut Vec<u8>) {
    let mut i = 0;
    let mut lit: Vec<u8> = Vec::new();
    let flush = |lit: &mut Vec<u8>, out: &mut Vec<u8>| {
        for chunk in lit.chunks(128) {
            out.push(chunk.len() as u8 - 1);
            out.extend_from_slice(chunk);
        }
        lit.clear();
    };
    while i < plane.len() {
        let run = plane[i..]
            .iter()
            .take(130)
            .take_while(|&&b| b == plane[i])
            .count();
        if run >= 3 {
            flush(&mut lit, out);
            out.push((run - 3 + 0x80) as u8);
            out.push(plane[i]);
            i += run;
        } else {
            lit.push(plane[i]);
            i += 1;
        }
    }
    flush(&mut lit, out);
}

/// `ARGB` and the alpha, red, green and blue planes, each run-length coded.
/// The colour is straight, as `iconutil -c icns` writes it. (Its reverse,
/// `iconutil -c iconset`, divides the colour by alpha again, so soft edges
/// come back too bright from Apple's own files as from ours.)
fn icns_argb(img: &Rgba) -> Vec<u8> {
    let mut out = b"ARGB".to_vec();
    for c in [3, 0, 1, 2] {
        let plane: Vec<u8> = img.pixels.as_chunks::<4>().0.iter().map(|p| p[c]).collect();
        icns_rle(&plane, &mut out);
    }
    out
}

/// A macOS `.icns`: `icns` and the big-endian total length, then for each
/// entry its type, its length (header included) and its image.
pub fn icns(img: &Rgba) -> Vec<u8> {
    let mut scaled: HashMap<u32, Rgba> = HashMap::new();
    let mut body = Vec::new();
    for (ty, size) in ICNS {
        let small = scaled.entry(size).or_insert_with(|| resize(img, size));
        let data = if size <= 32 && ty[..3] == *b"ic0" {
            icns_argb(small)
        } else {
            encode_png(small)
        };
        body.extend_from_slice(ty);
        body.extend_from_slice(&(8 + data.len() as u32).to_be_bytes());
        body.extend_from_slice(&data);
    }
    let mut out = Vec::with_capacity(8 + body.len());
    out.extend_from_slice(b"icns");
    out.extend_from_slice(&(8 + body.len() as u32).to_be_bytes());
    out.extend(body);
    out
}

/// The `.ico` sizes; Windows picks among them for the taskbar, Explorer
/// views and high-DPI scales.
pub const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// A Windows `.ico` of PNG-compressed images (Vista and later read PNG
/// entries). The directory writes 256 as 0, as the format requires.
pub fn ico(img: &Rgba) -> Vec<u8> {
    let pngs = pngs(img, &ICO_SIZES);
    let n = ICO_SIZES.len();
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes());
    out.extend_from_slice(&1u16.to_le_bytes()); // 1 = icon
    out.extend_from_slice(&(n as u16).to_le_bytes());
    let mut offset = 6 + 16 * n as u32;
    for s in ICO_SIZES {
        let len = pngs[&s].len() as u32;
        let dim = if s >= 256 { 0 } else { s as u8 };
        out.extend_from_slice(&[dim, dim, 0, 0]);
        out.extend_from_slice(&1u16.to_le_bytes()); // planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&len.to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += len;
    }
    for s in ICO_SIZES {
        out.extend_from_slice(&pngs[&s]);
    }
    out
}

/// Ledger's ink, paper and accent (addons/kit's light skin).
const INK: [f32; 3] = [0x14 as f32, 0x14 as f32, 0x13 as f32];
const PAPER: [f32; 3] = [0xf6 as f32, 0xf3 as f32, 0xea as f32];
const ACCENT: [f32; 3] = [0xff as f32, 0xd4 as f32, 0x00 as f32];

fn sd_box(px: f32, py: f32, cx: f32, cy: f32, hw: f32, hh: f32) -> f32 {
    let (dx, dy) = ((px - cx).abs() - hw, (py - cy).abs() - hh);
    let (ox, oy) = (dx.max(0.0), dy.max(0.0));
    (ox * ox + oy * oy).sqrt() + dx.max(dy).min(0.0)
}

fn sd_round_box(px: f32, py: f32, cx: f32, cy: f32, half: f32, r: f32) -> f32 {
    sd_box(px, py, cx, cy, half - r, half - r) - r
}

/// A bar of half-thickness `t` from `a` to `b`, square-ended.
fn sd_bar(p: (f32, f32), a: (f32, f32), b: (f32, f32), t: f32) -> f32 {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = (dx * dx + dy * dy).sqrt();
    let (ux, uy) = (dx / len, dy / len);
    let (qx, qy) = (p.0 - (a.0 + b.0) / 2.0, p.1 - (a.1 + b.1) / 2.0);
    let along = qx * ux + qy * uy;
    let across = -qx * uy + qy * ux;
    sd_box(along, across, 0.0, 0.0, len / 2.0, t)
}

/// The icon an app gets when it names none: a lowercase `k` in paper on an
/// ink rounded square, with Ledger's yellow as a cursor block beside it.
/// Drawn from distance fields at 1024 px, so its edges are antialiased at
/// any size it is scaled to. The square fills 824 of 1024 px, the margin
/// macOS's icon grid leaves.
pub fn default_icon() -> Rgba {
    const S: u32 = 1024;
    let mut img = Rgba::new(S, S);
    let cov = |d: f32| (0.5 - d).clamp(0.0, 1.0);
    // The k: a stem from the ascender to the baseline, and two arms
    // clipped to the x-height band so they end level with it.
    let (x_height, baseline) = (452.0, 724.0);
    let t = 46.0;
    for y in 0..S {
        for x in 0..S {
            let (px, py) = (x as f32 + 0.5, y as f32 + 0.5);
            let body = cov(sd_round_box(px, py, 512.0, 512.0, 412.0, 184.0));
            if body <= 0.0 {
                continue;
            }
            let stem = sd_box(px, py, 372.0, 487.0, t, 237.0);
            let upper = sd_bar((px, py), (380.0, 690.0), (660.0, 400.0), t);
            let lower = sd_bar((px, py), (470.0, 560.0), (690.0, 790.0), t);
            let band = sd_box(
                px,
                py,
                512.0,
                (x_height + baseline) / 2.0,
                1000.0,
                (baseline - x_height) / 2.0,
            );
            let k = cov(stem.min(upper.max(band)).min(lower.max(band)));
            let cursor = cov(sd_box(px, py, 704.0, 700.0, 34.0, 24.0));
            let mut c = INK;
            for i in 0..3 {
                c[i] = c[i] + (PAPER[i] - c[i]) * k;
                c[i] = c[i] + (ACCENT[i] - c[i]) * cursor;
            }
            let o = (y * S + x) as usize * 4;
            img.pixels[o] = (c[0] + 0.5) as u8;
            img.pixels[o + 1] = (c[1] + 0.5) as u8;
            img.pixels[o + 2] = (c[2] + 0.5) as u8;
            img.pixels[o + 3] = (body * 255.0 + 0.5) as u8;
        }
    }
    img
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;
    use std::process::Command;

    fn decode(png_bytes: &[u8]) -> Rgba {
        load_png(png_bytes).unwrap().image
    }

    fn be32(b: &[u8], at: usize) -> u32 {
        u32::from_be_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    fn le32(b: &[u8], at: usize) -> u32 {
        u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
    }

    fn solid(w: u32, h: u32, p: [u8; 4]) -> Rgba {
        Rgba {
            width: w,
            height: h,
            pixels: p.repeat(w as usize * h as usize),
        }
    }

    #[test]
    fn png_round_trip_and_colour_types() {
        let img = default_icon();
        let back = decode(&encode_png(&img));
        assert_eq!(back, img);
        // Greyscale and RGB sources come out as opaque RGBA.
        for (ct, chans) in [(png::ColorType::Grayscale, 1), (png::ColorType::Rgb, 3)] {
            let mut out = Vec::new();
            let mut enc = png::Encoder::new(&mut out, 4, 4);
            enc.set_color(ct);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&vec![200u8; 16 * chans]).unwrap();
            w.finish().unwrap();
            let l = load_png(&out).unwrap();
            assert_eq!(l.image.pixels[..4], [200, 200, 200, 255]);
            assert!(l.warnings.iter().any(|w| w.contains("4 px")));
        }
        assert!(load_png(b"not a png").is_err());
    }

    #[test]
    fn non_square_is_centred() {
        let l = load_png(&encode_png(&solid(40, 20, [255, 0, 0, 255]))).unwrap();
        assert_eq!((l.image.width, l.image.height), (40, 40));
        assert!(l.warnings.iter().any(|w| w.contains("40x20")));
        let at = |x: usize, y: usize| &l.image.pixels[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
        assert_eq!(at(20, 0), [0, 0, 0, 0]);
        assert_eq!(at(20, 20), [255, 0, 0, 255]);
        assert_eq!(at(20, 39), [0, 0, 0, 0]);
        assert!(
            load_png(&encode_png(&default_icon()))
                .unwrap()
                .warnings
                .is_empty()
        );
    }

    #[test]
    fn resize_keeps_flat_colour_and_alpha_edges() {
        let r = resize(&solid(300, 300, [10, 120, 250, 255]), 37);
        assert_eq!((r.width, r.height), (37, 37));
        for p in r.pixels.chunks(4) {
            assert!(
                p[0].abs_diff(10) <= 1
                    && p[1].abs_diff(120) <= 1
                    && p[2].abs_diff(250) <= 1
                    && p[3] == 255,
                "{p:?}"
            );
        }
        // A transparent pixel's colour never bleeds into its neighbours.
        let mut half = solid(64, 64, [255, 255, 255, 255]);
        for y in 0..64 {
            for x in 0..32 {
                half.pixels[(y * 64 + x) * 4..(y * 64 + x) * 4 + 4].copy_from_slice(&[0, 0, 0, 0]);
            }
        }
        let r = resize(&half, 16);
        for p in r.pixels.chunks(4).filter(|p| p[3] > 0) {
            assert!(p[0] >= 250, "a dark fringe: {p:?}");
        }
        // Upscaling works too.
        assert_eq!(
            resize(&solid(16, 16, [1, 2, 3, 255]), 64).pixels.len(),
            64 * 64 * 4
        );
    }

    /// The inverse of `icns_argb`, for a square image.
    fn unrle_argb(mut data: &[u8]) -> Rgba {
        let mut all = Vec::new();
        while !data.is_empty() {
            let c = data[0] as usize;
            if c < 0x80 {
                all.extend_from_slice(&data[1..2 + c]);
                data = &data[2 + c..];
            } else {
                all.extend(std::iter::repeat_n(data[1], c - 0x80 + 3));
                data = &data[2..];
            }
        }
        let n = all.len() / 4;
        let side = (n as f64).sqrt() as u32;
        assert_eq!((side * side) as usize * 4, all.len());
        let mut img = Rgba::new(side, side);
        for i in 0..n {
            img.pixels[i * 4..i * 4 + 4].copy_from_slice(&[
                all[n + i],
                all[2 * n + i],
                all[3 * n + i],
                all[i],
            ]);
        }
        img
    }

    #[test]
    fn icns_rle_round_trips() {
        let img = resize(&default_icon(), 32);
        assert_eq!(unrle_argb(&icns_argb(&img)[4..]), img);
        let flat = solid(16, 16, [9, 200, 77, 255]);
        assert_eq!(unrle_argb(&icns_argb(&flat)[4..]), flat);
        // Long literal and run stretches split at 128 and 130.
        let mut ramp = Rgba::new(32, 32);
        for (i, p) in ramp.pixels.chunks_mut(4).enumerate() {
            p.copy_from_slice(&[i as u8, if i < 600 { 7 } else { i as u8 }, 0, 255]);
        }
        assert_eq!(unrle_argb(&icns_argb(&ramp)[4..]), ramp);
    }

    #[test]
    fn icns_parses_back() {
        let bytes = icns(&default_icon());
        assert_eq!(&bytes[..4], b"icns");
        assert_eq!(be32(&bytes, 4) as usize, bytes.len());
        let mut p = 8;
        let mut seen = Vec::new();
        while p < bytes.len() {
            let ty = std::str::from_utf8(&bytes[p..p + 4]).unwrap().to_string();
            let len = be32(&bytes, p + 4) as usize;
            let data = &bytes[p + 8..p + len];
            let img = if data.starts_with(b"ARGB") {
                unrle_argb(&data[4..])
            } else {
                decode(data)
            };
            assert_eq!(img.width, img.height);
            seen.push((ty, img.width));
            p += len;
        }
        assert_eq!(p, bytes.len());
        let want: Vec<(String, u32)> = ICNS
            .iter()
            .map(|(t, s)| (String::from_utf8(t.to_vec()).unwrap(), *s))
            .collect();
        assert_eq!(seen, want);
    }

    #[test]
    fn ico_directory_is_consistent() {
        let bytes = ico(&default_icon());
        assert_eq!(&bytes[..6], &[0, 0, 1, 0, 7, 0]);
        let mut end = 6 + 16 * 7;
        for (i, want) in ICO_SIZES.iter().enumerate() {
            let e = &bytes[6 + 16 * i..6 + 16 * (i + 1)];
            let dim = if e[0] == 0 { 256 } else { e[0] as u32 };
            assert_eq!(
                (dim, e[1] as u32 + if e[1] == 0 { 256 } else { 0 }),
                (*want, *want)
            );
            let (len, off) = (le32(e, 8) as usize, le32(e, 12) as usize);
            assert_eq!(off, end, "entries are packed in order");
            let img = decode(&bytes[off..off + len]);
            assert_eq!((img.width, img.height), (*want, *want));
            end = off + len;
        }
        assert_eq!(end, bytes.len());
    }

    #[test]
    fn linux_pngs() {
        for s in [256, 512] {
            let img = decode(&png_of(&default_icon(), s));
            assert_eq!((img.width, img.height), (s, s));
        }
    }

    #[test]
    fn default_icon_looks_right() {
        let img = default_icon();
        let at = |x: u32, y: u32| {
            img.pixels[((y * 1024 + x) * 4) as usize..((y * 1024 + x) * 4 + 4) as usize].to_vec()
        };
        assert_eq!(at(10, 10)[3], 0, "outside the square is clear");
        assert_eq!(at(150, 512), [0x14, 0x14, 0x13, 255], "the square is ink");
        assert_eq!(at(372, 300), [0xf6, 0xf3, 0xea, 255], "the stem is paper");
        assert_eq!(
            at(704, 700),
            [0xff, 0xd4, 0x00, 255],
            "the cursor is yellow"
        );
    }

    #[test]
    fn iconutil_accepts_our_icns() {
        let iconutil = Path::new("/usr/bin/iconutil");
        if !iconutil.exists() {
            eprintln!("skipped: no iconutil");
            return;
        }
        let dir = std::env::temp_dir().join(format!("kui-icons-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let file = dir.join("app.icns");
        std::fs::write(&file, icns(&default_icon())).unwrap();
        let set = dir.join("app.iconset");
        let out = Command::new(iconutil)
            .arg("-c")
            .arg("iconset")
            .arg(&file)
            .arg("-o")
            .arg(&set)
            .output()
            .unwrap();
        assert!(
            out.status.success(),
            "iconutil: {}",
            String::from_utf8_lossy(&out.stderr)
        );
        let big = std::fs::read(set.join("icon_512x512@2x.png")).unwrap();
        let img = decode(&big);
        assert_eq!(img.width, 1024);
        // The small ARGB entries come back as what was written.
        for (name, size) in [("icon_16x16.png", 16), ("icon_32x32.png", 32)] {
            let back = decode(&std::fs::read(set.join(name)).unwrap());
            let ours = resize(&default_icon(), size);
            assert_eq!((back.width, back.height), (size, size));
            // Alpha everywhere, colour where opaque: `iconutil` brightens
            // soft edges on the way out (see `icns_argb`).
            let worst = back
                .pixels
                .chunks(4)
                .zip(ours.pixels.chunks(4))
                .flat_map(|(a, b)| {
                    let n = if b[3] == 255 { 4 } else { 0 };
                    a[..n]
                        .iter()
                        .zip(&b[..n])
                        .map(|(x, y)| x.abs_diff(*y))
                        .chain([a[3].abs_diff(b[3])])
                })
                .max()
                .unwrap();
            assert!(worst <= 2, "{name} differs by up to {worst}");
        }
    }
}
