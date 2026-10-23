//! The runner's image encoder (ADR 0054, decision 7): straight RGBA — a
//! [`crate::capture`], a decoded image — out as a file's bytes, through
//! the crates the decoder already links, so a test or a probe keeps what
//! it saw with no encoder of its own.
//!
//! [`encode_image`] writes one picture as PNG, JPEG, WebP or GIF.
//! [`AnimationEncoder`] writes a sequence as an animated PNG or GIF, a
//! frame at a time: a recording of a UI at retina size is 30 MB a frame,
//! so the encoder keeps only the rectangle each frame changed and folds a
//! frame that changed nothing into the time of the one before. Both
//! formats draw a frame over the last at an offset, which is what makes
//! the rectangles enough.

use crate::decode::{MAX_FRAME_BYTES, frame_bytes};

/// What [`encode_image`] writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageEncoding {
    /// Lossless, 8 bits a channel with alpha, at the encoder's fast
    /// setting: a text-heavy 3456 by 2234 capture took about 12 ms and
    /// 1.2 MB on an M3 Pro.
    Png,
    /// Lossy, at `quality` from 1 to 100 (clamped); alpha is dropped, each
    /// pixel's straight colour kept.
    Jpeg { quality: u8 },
    /// Lossless WebP with alpha: the runner has no lossy WebP encoder.
    WebP,
    /// 256 colours, quantized per picture; a pixel is opaque, or
    /// transparent where its alpha is zero.
    Gif,
}

/// What an [`AnimationEncoder`] writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AnimationEncoding {
    /// An animated PNG (APNG): every colour and alpha exact, a frame's
    /// time to the millisecond. One frame is a still PNG.
    Png,
    /// An animated GIF: 256 colours quantized per frame, a pixel opaque or
    /// transparent, a frame's time in hundredths of a second and at least
    /// two of them, which is the shortest browsers show as asked.
    Gif,
}

/// Encodes straight RGBA — `width` by `height` pixels, four bytes each,
/// row by row from the top left, what [`crate::capture`] and
/// [`crate::decode_image`] hand out — as `format`. Fails, saying why, for
/// a degenerate size, pixels that are not that size, or a GIF past 65535
/// pixels a side.
pub fn encode_image(
    format: ImageEncoding,
    width: u32,
    height: u32,
    rgba: &[u8],
) -> Result<Vec<u8>, String> {
    use image::ImageEncoder;
    check_size(width, height)?;
    check_pixels(width, height, rgba)?;
    let mut out = Vec::new();
    let done = match format {
        ImageEncoding::Png => {
            use image::codecs::png::{CompressionType, FilterType, PngEncoder};
            PngEncoder::new_with_quality(&mut out, CompressionType::Fast, FilterType::Adaptive)
                .write_image(rgba, width, height, image::ExtendedColorType::Rgba8)
        }
        ImageEncoding::Jpeg { quality } => {
            let rgb: Vec<u8> = rgba
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            image::codecs::jpeg::JpegEncoder::new_with_quality(&mut out, quality.clamp(1, 100))
                .write_image(&rgb, width, height, image::ExtendedColorType::Rgb8)
        }
        ImageEncoding::WebP => image::codecs::webp::WebPEncoder::new_lossless(&mut out)
            .write_image(rgba, width, height, image::ExtendedColorType::Rgba8),
        ImageEncoding::Gif => {
            check_gif(width, height)?;
            let whole = Patch {
                x: 0,
                y: 0,
                w: width,
                h: height,
                rgba: rgba.to_vec(),
                secs: 0.0,
            };
            write_gif(&mut out, width, height, None, &[whole], &[0])?;
            Ok(())
        }
    };
    done.map_err(|e| format!("encoding a {format:?}: {e}"))?;
    Ok(out)
}

/// Writes an animated PNG or GIF a frame at a time; [`Self::finish`]
/// hands back the file's bytes.
///
/// Each [`Self::add`] is the whole canvas and how long it shows. What is
/// kept of it is the rectangle that differs from the frame kept before; a
/// frame that differs in nothing adds its time to that one, and a frame
/// shorter than the format can show (a millisecond for PNG, 20 ms for
/// GIF) gives its time to the next frame and is not shown. So a recording
/// holds two whole frames and the changed parts, and a UI that changes a
/// corner costs a corner.
///
/// A headless recording is exact: frame, [`crate::capture`], `add` it for
/// the step, move the clock by the step, again.
pub struct AnimationEncoder {
    format: AnimationEncoding,
    width: u32,
    height: u32,
    loops: Option<u32>,
    /// The canvas after the last frame kept.
    shown: Vec<u8>,
    /// The frame added last, not kept yet, and how long it shows so far.
    pending: Option<(Vec<u8>, f64)>,
    kept: Vec<Patch>,
}

/// One kept frame: the rectangle it changed, its pixels there, and how
/// long it shows.
struct Patch {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
    rgba: Vec<u8>,
    secs: f64,
}

/// A GIF frame's least time: 2 hundredths. Browsers show 1 (and 0) as 10.
const GIF_SHORTEST: f64 = 0.02;
/// An APNG frame's: a millisecond, its delay's unit here.
const PNG_SHORTEST: f64 = 0.001;

impl AnimationEncoder {
    /// An encoder for `width` by `height` frames that play `loops` times,
    /// `None` for ever. Fails, saying why, for a degenerate size, a frame
    /// past what the decoder holds one to (512 MiB), or a GIF past 65535
    /// pixels a side.
    pub fn new(
        format: AnimationEncoding,
        width: u32,
        height: u32,
        loops: Option<u32>,
    ) -> Result<Self, String> {
        check_size(width, height)?;
        if frame_bytes(width, height) > MAX_FRAME_BYTES {
            return Err(format!(
                "a {width}x{height} frame is more than one may hold ({} MiB)",
                MAX_FRAME_BYTES >> 20
            ));
        }
        if format == AnimationEncoding::Gif {
            check_gif(width, height)?;
        }
        Ok(Self {
            format,
            width,
            height,
            loops,
            shown: Vec::new(),
            pending: None,
            kept: Vec::new(),
        })
    }

    /// Adds a frame — the whole canvas, straight RGBA — that shows for
    /// `seconds`. Fails, saying why, for pixels that are not the size, or
    /// a time that is negative or not finite.
    pub fn add(&mut self, rgba: &[u8], seconds: f64) -> Result<(), String> {
        check_pixels(self.width, self.height, rgba)?;
        if !(seconds.is_finite() && seconds >= 0.0) {
            return Err(format!(
                "a frame shows for a finite number of seconds, not {seconds}"
            ));
        }
        let shortest = match self.format {
            AnimationEncoding::Png => PNG_SHORTEST,
            AnimationEncoding::Gif => GIF_SHORTEST,
        };
        self.pending = Some(match self.pending.take() {
            // Too short to show: this frame takes its place and its time.
            Some((mut px, secs)) if secs < shortest => {
                px.copy_from_slice(rgba);
                (px, secs + seconds)
            }
            Some((px, secs)) => {
                self.keep(px, secs);
                (rgba.to_vec(), seconds)
            }
            None => (rgba.to_vec(), seconds),
        });
        Ok(())
    }

    /// How many frames are kept so far, the one added last not counted:
    /// what the file will hold, less one.
    pub fn kept(&self) -> usize {
        self.kept.len()
    }

    /// The file's bytes. Fails, saying why, with no frame added.
    pub fn finish(mut self) -> Result<Vec<u8>, String> {
        if let Some((px, secs)) = self.pending.take() {
            self.keep(px, secs);
        }
        if self.kept.is_empty() {
            return Err("an animation is at least one frame".into());
        }
        let mut out = Vec::new();
        match self.format {
            AnimationEncoding::Png => {
                let delays = ticks(&self.kept, 1000.0, 1);
                write_apng(
                    &mut out,
                    self.width,
                    self.height,
                    self.loops,
                    &self.kept,
                    &delays,
                )
                .map_err(|e| format!("encoding an animated PNG: {e}"))?;
            }
            AnimationEncoding::Gif => {
                let delays = ticks(&self.kept, 100.0, 2);
                write_gif(
                    &mut out,
                    self.width,
                    self.height,
                    Some(self.loops),
                    &self.kept,
                    &delays,
                )?;
            }
        }
        Ok(out)
    }

    /// Keeps `px` as the rectangle it changed over what shows, or gives its
    /// time to the frame before when it changed nothing.
    fn keep(&mut self, px: Vec<u8>, secs: f64) {
        let (w, h) = (self.width, self.height);
        if self.kept.is_empty() {
            self.kept.push(Patch {
                x: 0,
                y: 0,
                w,
                h,
                rgba: px.clone(),
                secs,
            });
            self.shown = px;
            return;
        }
        let Some((x, y, cw, ch)) = changed(&self.shown, &px, w) else {
            if let Some(last) = self.kept.last_mut() {
                last.secs += secs;
            }
            return;
        };
        let row = w as usize * 4;
        let mut rgba = Vec::with_capacity(cw as usize * ch as usize * 4);
        for r in y..y + ch {
            let at = r as usize * row + x as usize * 4;
            rgba.extend_from_slice(&px[at..at + cw as usize * 4]);
        }
        self.kept.push(Patch {
            x,
            y,
            w: cw,
            h: ch,
            rgba,
            secs,
        });
        self.shown = px;
    }
}

/// The smallest rectangle `b` differs from `a` in, `w` pixels a row:
/// `(x, y, width, height)`, `None` where they are the same.
fn changed(a: &[u8], b: &[u8], w: u32) -> Option<(u32, u32, u32, u32)> {
    let row = w as usize * 4;
    let (mut x0, mut x1) = (usize::MAX, 0);
    let mut ys: Option<(usize, usize)> = None;
    for (y, (ra, rb)) in a.chunks_exact(row).zip(b.chunks_exact(row)).enumerate() {
        if ra == rb {
            continue;
        }
        let (pa, pb) = (ra.as_chunks::<4>().0, rb.as_chunks::<4>().0);
        let first = pa.iter().zip(pb).position(|(p, q)| p != q).unwrap_or(0);
        let last = pa.iter().zip(pb).rposition(|(p, q)| p != q).unwrap_or(0);
        x0 = x0.min(first);
        x1 = x1.max(last + 1);
        ys = Some((ys.map_or(y, |(y0, _)| y0), y + 1));
    }
    ys.map(|(y0, y1)| (x0 as u32, y0 as u32, (x1 - x0) as u32, (y1 - y0) as u32))
}

/// Each frame's time in `per_sec` units, rounded on the running total so
/// the sum keeps to the frames' own, and never under `least`.
fn ticks(kept: &[Patch], per_sec: f64, least: u16) -> Vec<u16> {
    let (mut t, mut at) = (0.0, 0i64);
    kept.iter()
        .map(|p| {
            t += p.secs;
            let end = (t * per_sec).round() as i64;
            let d = (end - at).clamp(least as i64, u16::MAX as i64);
            at += d;
            d as u16
        })
        .collect()
}

fn write_apng(
    out: &mut Vec<u8>,
    width: u32,
    height: u32,
    loops: Option<u32>,
    kept: &[Patch],
    delays: &[u16],
) -> Result<(), png::EncodingError> {
    let mut e = png::Encoder::new(out, width, height);
    e.set_color(png::ColorType::Rgba);
    e.set_depth(png::BitDepth::Eight);
    e.set_compression(png::Compression::Fast);
    let animated = kept.len() > 1;
    if animated {
        e.set_animated(kept.len() as u32, loops.unwrap_or(0))?;
    }
    let mut w = e.write_header()?;
    for (p, &d) in kept.iter().zip(delays) {
        if animated {
            // Back to the origin first: a size and a place are each
            // checked against the other's current value.
            w.reset_frame_position()?;
            w.set_frame_dimension(p.w, p.h)?;
            w.set_frame_position(p.x, p.y)?;
            w.set_frame_delay(d, 1000)?;
            w.set_blend_op(png::BlendOp::Source)?;
            w.set_dispose_op(png::DisposeOp::None)?;
        }
        w.write_image_data(&p.rgba)?;
    }
    w.finish()
}

/// `loops`: `None` for a still picture, `Some(None)` for ever, `Some(n)`
/// that many times.
fn write_gif(
    out: &mut Vec<u8>,
    width: u32,
    height: u32,
    loops: Option<Option<u32>>,
    kept: &[Patch],
    delays: &[u16],
) -> Result<(), String> {
    let err = |e: gif::EncodingError| format!("encoding a GIF: {e}");
    let mut e = gif::Encoder::new(out, width as u16, height as u16, &[]).map_err(err)?;
    if let Some(loops) = loops {
        e.set_repeat(match loops {
            None => gif::Repeat::Infinite,
            Some(n) => gif::Repeat::Finite(n.min(u16::MAX as u32) as u16),
        })
        .map_err(err)?;
    }
    for (p, &d) in kept.iter().zip(delays) {
        let mut px = p.rgba.clone();
        let mut f = gif::Frame::from_rgba_speed(p.w as u16, p.h as u16, &mut px, 10);
        (f.left, f.top, f.delay) = (p.x as u16, p.y as u16, d);
        f.dispose = gif::DisposalMethod::Keep;
        e.write_frame(&f).map_err(err)?;
    }
    Ok(())
}

fn check_size(width: u32, height: u32) -> Result<(), String> {
    if width == 0 || height == 0 {
        return Err(format!(
            "a picture is at least one pixel, not {width}x{height}"
        ));
    }
    Ok(())
}

fn check_pixels(width: u32, height: u32, rgba: &[u8]) -> Result<(), String> {
    if rgba.len() as u64 != frame_bytes(width, height) {
        return Err(format!(
            "{width}x{height} RGBA is {} bytes, not {}",
            frame_bytes(width, height),
            rgba.len()
        ));
    }
    Ok(())
}

fn check_gif(width: u32, height: u32) -> Result<(), String> {
    if width > u16::MAX as u32 || height > u16::MAX as u32 {
        return Err(format!(
            "a GIF is at most 65535 pixels a side, not {width}x{height}"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{decode_animation, decode_image};

    /// `w` by `h` of one colour.
    fn fill(w: u32, h: u32, c: [u8; 4]) -> Vec<u8> {
        c.repeat((w * h) as usize)
    }

    #[test]
    fn a_png_and_a_webp_come_back_exact_and_a_jpeg_close() {
        let px: Vec<u8> = (0..3 * 2 * 4).map(|i| (i * 11) as u8).collect();
        let png = encode_image(ImageEncoding::Png, 3, 2, &px).expect("png");
        assert_eq!(&png[..8], b"\x89PNG\r\n\x1a\n");
        assert_eq!(
            decode_image(&png).expect("decoded").rgba,
            px,
            "straight both ways"
        );
        let webp = encode_image(ImageEncoding::WebP, 3, 2, &px).expect("webp");
        assert_eq!(decode_image(&webp).expect("decoded").rgba, px, "lossless");
        let flat = fill(16, 16, [200, 40, 90, 128]);
        let jpeg = encode_image(ImageEncoding::Jpeg { quality: 95 }, 16, 16, &flat).expect("jpeg");
        let back = decode_image(&jpeg).expect("decoded");
        let p = &back.rgba[..4];
        assert!(p[0].abs_diff(200) <= 3 && p[1].abs_diff(40) <= 3 && p[2].abs_diff(90) <= 3);
        assert_eq!(p[3], 255, "the alpha dropped");
        let gif =
            encode_image(ImageEncoding::Gif, 16, 16, &fill(16, 16, [0, 0, 255, 255])).expect("gif");
        assert_eq!(
            &decode_image(&gif).expect("decoded").rgba[..4],
            &[0, 0, 255, 255]
        );
    }

    #[test]
    fn a_wrong_size_is_refused_saying_why() {
        let e = encode_image(ImageEncoding::Png, 0, 2, &[]).unwrap_err();
        assert!(e.contains("at least one pixel"), "{e}");
        let e = encode_image(ImageEncoding::Png, 3, 2, &[0; 23]).unwrap_err();
        assert!(e.contains("is 24 bytes, not 23"), "{e}");
        let e = AnimationEncoder::new(AnimationEncoding::Gif, 70_000, 1, None)
            .err()
            .expect("too wide");
        assert!(e.contains("65535"), "{e}");
        let mut a = AnimationEncoder::new(AnimationEncoding::Png, 2, 2, None).unwrap();
        assert!(a.add(&[0; 16], f64::NAN).unwrap_err().contains("finite"));
        let e = AnimationEncoder::new(AnimationEncoding::Png, 2, 2, None)
            .unwrap()
            .finish()
            .unwrap_err();
        assert!(e.contains("at least one frame"), "{e}");
    }

    /// Red, then a blue square in its corner, then the same again, then
    /// green: the repeat folds into the square's time, the square is kept
    /// as its rectangle, and both formats decode to the whole canvases at
    /// the times given.
    fn sequence(a: &mut AnimationEncoder) {
        let red = fill(8, 6, [255, 0, 0, 255]);
        let mut square = red.clone();
        for y in 1..3 {
            for x in 2..4 {
                let o = (y * 8 + x) * 4;
                square[o..o + 4].copy_from_slice(&[0, 0, 255, 255]);
            }
        }
        a.add(&red, 0.1).unwrap();
        a.add(&square, 0.05).unwrap();
        a.add(&square, 0.15).unwrap();
        a.add(&fill(8, 6, [0, 255, 0, 255]), 0.3).unwrap();
    }

    #[test]
    fn an_animation_keeps_what_changed_and_plays_back_whole() {
        for format in [AnimationEncoding::Png, AnimationEncoding::Gif] {
            let mut a = AnimationEncoder::new(format, 8, 6, Some(2)).unwrap();
            sequence(&mut a);
            assert_eq!(
                a.kept(),
                2,
                "{format:?}: the repeat folded in, green pending"
            );
            assert_eq!(
                a.kept[1].rgba.len(),
                2 * 2 * 4,
                "{format:?}: the square alone"
            );
            let bytes = a.finish().unwrap();
            let back = decode_animation(&bytes).expect("decoded");
            assert_eq!((back.width, back.height), (8, 6), "{format:?}");
            assert_eq!(back.frames.len(), 3, "{format:?}");
            let delays: Vec<f64> = back.frames.iter().map(|f| f.delay).collect();
            assert_eq!(
                delays,
                [0.1, 0.2, 0.3],
                "{format:?}: the square holds both its times"
            );
            assert_eq!(back.loops, Some(2), "{format:?}");
            let at = |f: usize, x: usize, y: usize| &back.frames[f].rgba[(y * 8 + x) * 4..][..4];
            assert_eq!(at(1, 2, 1), &[0, 0, 255, 255], "{format:?}: the square");
            assert_eq!(
                at(1, 6, 4),
                &[255, 0, 0, 255],
                "{format:?}: the red kept under it"
            );
            assert_eq!(at(2, 2, 1), &[0, 255, 0, 255], "{format:?}");
        }
    }

    /// 60 frames a second: a GIF cannot show a frame under 20 ms, so every
    /// other one gives its time to the next; the total is kept.
    #[test]
    fn a_gif_at_60_fps_shows_every_other_frame_for_the_same_total() {
        let mut a = AnimationEncoder::new(AnimationEncoding::Gif, 4, 4, None).unwrap();
        for i in 0..60u8 {
            a.add(&fill(4, 4, [i * 4, 0, 0, 255]), 1.0 / 60.0).unwrap();
        }
        let back = decode_animation(&a.finish().unwrap()).unwrap();
        assert_eq!(back.frames.len(), 30);
        let total: f64 = back.frames.iter().map(|f| f.delay).sum();
        assert!((total - 1.0).abs() < 0.011, "{total}");
        assert!(back.frames.iter().all(|f| f.delay >= 0.02));
        assert_eq!(back.loops, None, "for ever");
    }

    #[test]
    fn one_frame_is_a_still_png() {
        let mut a = AnimationEncoder::new(AnimationEncoding::Png, 2, 2, None).unwrap();
        a.add(&fill(2, 2, [1, 2, 3, 4]), 0.5).unwrap();
        let bytes = a.finish().unwrap();
        assert_eq!(decode_image(&bytes).unwrap().rgba, fill(2, 2, [1, 2, 3, 4]));
    }
}
