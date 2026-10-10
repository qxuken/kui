//! The runner's image decoder (backlog F138): the `image` crate the
//! wallpaper behind a `Tinted` window already links (`ground.rs`), behind
//! two functions, so an app that ships a photo or an animated GIF needs no
//! decoder of its own.
//!
//! [`decode_image`] turns PNG, JPEG, WebP or GIF bytes into straight RGBA,
//! the shape `Core::add_image`, `update_image` and `Launcher::icon` take.
//! [`decode_animation`] keeps every frame of an animated GIF, PNG (APNG) or
//! WebP with the time it shows for, and [`Animation::at`] says which frame
//! a moment of the frame clock shows and when the next is due — what a
//! view hands to `update_image` and `Ui::request_frame_at`, so a GIF plays
//! with no thread and no frame owed between its changes.

use std::io::Cursor;

use image::{AnimationDecoder, ImageDecoder, ImageFormat};

/// Decoded pixels: `width` by `height`, four bytes each (RGBA), row by row
/// from the top left, alpha not premultiplied.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Pixels {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

/// Every frame of an animated image, each the whole canvas (the
/// file's own partial frames already composited, as a browser shows
/// them), with how long each shows.
#[derive(Clone, Debug, PartialEq)]
pub struct Animation {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<AnimationFrame>,
    /// How many times the whole sequence plays; `None` for ever, which is
    /// what most GIFs say.
    pub loops: Option<u32>,
}

/// One frame of an [`Animation`]: `width * height * 4` bytes of RGBA, and
/// the seconds it shows for.
#[derive(Clone, Debug, PartialEq)]
pub struct AnimationFrame {
    pub rgba: Vec<u8>,
    pub delay: f64,
}

/// Which frame of an [`Animation`] a moment shows, from [`Animation::at`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Showing {
    /// The frame's index in [`Animation::frames`].
    pub index: usize,
    /// Seconds after the start when the next frame is due; infinity once
    /// a finite animation has played out, or for a single frame.
    pub next: f64,
}

/// The shortest delay a GIF frame is shown for as written: browsers show
/// a frame that says 0 or 10 ms for 100 ms, and GIFs are made for that.
const GIF_FLOOR: f64 = 0.011;
const GIF_SLOW: f64 = 0.1;

fn format_of(bytes: &[u8]) -> Result<ImageFormat, String> {
    let format = image::guess_format(bytes)
        .map_err(|_| "not an image kui decodes: PNG, JPEG, WebP or GIF".to_string())?;
    match format {
        ImageFormat::Png | ImageFormat::Jpeg | ImageFormat::WebP | ImageFormat::Gif => Ok(format),
        other => Err(format!(
            "{other:?} is not a format kui decodes: PNG, JPEG, WebP or GIF"
        )),
    }
}

/// Decodes PNG, JPEG, WebP or GIF bytes to straight RGBA — an animated
/// file's first frame. A failure names the format it could not read, or
/// that the bytes are none of the four.
pub fn decode_image(bytes: &[u8]) -> Result<Pixels, String> {
    let format = format_of(bytes)?;
    // The `image` crate's own ceiling holds its decode, which for a WebP
    // or a JPEG is RGB: the RGBA it is turned into is a third again, past
    // it (backlog FZ3: 176 bytes of WebP made 576 MiB). The header says
    // what the RGBA will be, and that is held to the frame's ceiling.
    let (w, h) = image::ImageReader::with_format(Cursor::new(bytes), format)
        .into_dimensions()
        .map_err(|e| format!("decoding a {format:?}: {e}"))?;
    if frame_bytes(w, h) > MAX_FRAME_BYTES {
        return Err(too_big(format, w, h));
    }
    let img = image::load_from_memory_with_format(bytes, format)
        .map_err(|e| format!("decoding a {format:?}: {e}"))?
        .into_rgba8();
    Ok(Pixels {
        width: img.width(),
        height: img.height(),
        rgba: img.into_raw(),
    })
}

/// Decodes every frame of an animated GIF, PNG (APNG) or WebP, each the
/// whole canvas with its delay; a still image — a JPEG, a PNG with one
/// frame — is one frame that shows for ever. A GIF frame that asks for
/// 10 ms or less shows for 100 ms, as browsers show it.
///
/// Every frame is kept decoded, so the whole is held to
/// [`MAX_ANIMATION_BYTES`], and any one buffer to the `image` crate's
/// own ceiling (512 MiB) — what [`decode_image`] is held to: past either
/// it is an error, never an allocation of what a file's header claims
/// (backlog FZ3: a 130-byte GIF asked for 11 GB).
pub fn decode_animation(bytes: &[u8]) -> Result<Animation, String> {
    let format = format_of(bytes)?;
    let err = |e: image::ImageError| format!("decoding a {format:?}: {e}");
    let (width, height, loops, frames) = match format {
        ImageFormat::Gif => {
            let mut d = image::codecs::gif::GifDecoder::new(Cursor::new(bytes)).map_err(err)?;
            d.set_limits(image::Limits::default()).map_err(err)?;
            let (w, h) = d.dimensions();
            let loops = loops_of(d.loop_count());
            (w, h, loops, d.into_frames())
        }
        ImageFormat::Png => {
            let d = image::codecs::png::PngDecoder::with_limits(
                Cursor::new(bytes),
                image::Limits::default(),
            )
            .map_err(err)?;
            if !d.is_apng().map_err(err)? {
                return still(bytes);
            }
            let (w, h) = d.dimensions();
            let a = d.apng().map_err(err)?;
            let loops = loops_of(a.loop_count());
            (w, h, loops, a.into_frames())
        }
        ImageFormat::WebP => {
            let d = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).map_err(err)?;
            if !d.has_animation() {
                return still(bytes);
            }
            let (w, h) = d.dimensions();
            // The WebP decoder takes no limits: its canvas is held here.
            if frame_bytes(w, h) > MAX_FRAME_BYTES {
                return Err(too_big(format, w, h));
            }
            let loops = loops_of(d.loop_count());
            (w, h, loops, d.into_frames())
        }
        _ => return still(bytes),
    };
    if frame_bytes(width, height) > MAX_FRAME_BYTES {
        return Err(too_big(format, width, height));
    }
    let gif = format == ImageFormat::Gif;
    let mut out: Vec<AnimationFrame> = Vec::new();
    for f in frames {
        let f = f.map_err(err)?;
        if (out.len() as u64 + 1) * frame_bytes(width, height) > MAX_ANIMATION_BYTES {
            return Err(format!(
                "a {format:?} of {width}x{height} with more frames than {} MiB holds",
                MAX_ANIMATION_BYTES >> 20
            ));
        }
        let (num, den) = f.delay().numer_denom_ms();
        let mut delay = f64::from(num) / f64::from(den.max(1)) / 1000.0;
        if gif && delay < GIF_FLOOR {
            delay = GIF_SLOW;
        }
        let (left, top) = (f.left(), f.top());
        let buf = f.into_buffer();
        let rgba = if (buf.width(), buf.height(), left, top) == (width, height, 0, 0) {
            buf.into_raw()
        } else {
            // A decoder that hands back the changed rectangle alone: laid
            // over the frame before it, which is what "not disposed" means.
            let mut canvas = out
                .last()
                .map(|p| p.rgba.clone())
                .unwrap_or_else(|| vec![0; width as usize * height as usize * 4]);
            blit(&mut canvas, width, height, &buf, left, top);
            canvas
        };
        out.push(AnimationFrame { rgba, delay });
    }
    if out.is_empty() {
        return Err(format!("a {format:?} with no frames"));
    }
    Ok(Animation {
        width,
        height,
        frames: out,
        loops,
    })
}

/// The most every frame of one [`decode_animation`] may hold together:
/// 1 GiB, some 125 frames at 1080p, or a minute of a 640x480 GIF at 15
/// fps.
pub const MAX_ANIMATION_BYTES: u64 = 1 << 30;

/// The most one frame's buffer may hold: the `image` crate's own default
/// ceiling for an allocation, the one `decode_image` is held to.
const MAX_FRAME_BYTES: u64 = 512 << 20;

fn frame_bytes(w: u32, h: u32) -> u64 {
    u64::from(w) * u64::from(h) * 4
}

fn too_big(format: ImageFormat, w: u32, h: u32) -> String {
    format!(
        "a {format:?} of {w}x{h} is more than one frame may hold ({} MiB)",
        MAX_FRAME_BYTES >> 20
    )
}

fn still(bytes: &[u8]) -> Result<Animation, String> {
    let p = decode_image(bytes)?;
    Ok(Animation {
        width: p.width,
        height: p.height,
        frames: vec![AnimationFrame {
            rgba: p.rgba,
            delay: f64::INFINITY,
        }],
        loops: Some(1),
    })
}

fn loops_of(count: image::metadata::LoopCount) -> Option<u32> {
    match count {
        image::metadata::LoopCount::Infinite => None,
        image::metadata::LoopCount::Finite(n) => Some(n.get()),
    }
}

fn blit(canvas: &mut [u8], w: u32, h: u32, src: &image::RgbaImage, left: u32, top: u32) {
    for (y, row) in src.rows().enumerate() {
        let cy = top as usize + y;
        if cy >= h as usize {
            break;
        }
        for (x, px) in row.enumerate() {
            let cx = left as usize + x;
            if cx >= w as usize {
                break;
            }
            if px.0[3] != 0 {
                let at = (cy * w as usize + cx) * 4;
                canvas[at..at + 4].copy_from_slice(&px.0);
            }
        }
    }
}

impl Animation {
    /// The seconds one pass through every frame takes; infinity when a
    /// frame shows for ever.
    pub fn duration(&self) -> f64 {
        self.frames.iter().map(|f| f.delay).sum()
    }

    /// The frame `elapsed` seconds after the animation started shows, and
    /// when the next is due, in the same seconds: with the frame clock,
    /// `at(ui.now() - started)`, then `update_image` when the index moved
    /// and `request_frame_at(started + next)`. Before the start is the
    /// first frame; after a finite animation has played out, the last,
    /// with nothing due.
    pub fn at(&self, elapsed: f64) -> Showing {
        let last = self.frames.len().saturating_sub(1);
        let total = self.duration();
        if self.frames.len() < 2 || !total.is_finite() || total <= 0.0 {
            return Showing {
                index: 0,
                next: f64::INFINITY,
            };
        }
        let elapsed = elapsed.max(0.0);
        let pass = (elapsed / total).floor();
        if let Some(n) = self.loops
            && pass >= f64::from(n)
        {
            return Showing {
                index: last,
                next: f64::INFINITY,
            };
        }
        let mut t = pass * total;
        for (i, f) in self.frames.iter().enumerate() {
            t += f.delay;
            if elapsed < t {
                return Showing { index: i, next: t };
            }
        }
        Showing {
            index: last,
            next: (pass + 1.0) * total + self.frames[0].delay,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gif::Repeat;
    use image::{Rgba, RgbaImage};

    /// A 4x2 GIF of three frames, red, green, then a 2x1 blue patch over
    /// the green at (1, 1), at 50, 100 and 0 ms; `repeat` is its loop count.
    fn gif(repeat: gif::Repeat) -> Vec<u8> {
        let mut out = Vec::new();
        {
            let mut enc = gif::Encoder::new(&mut out, 4, 2, &[]).unwrap();
            enc.set_repeat(repeat).unwrap();
            let mut frame = |w: u16, h: u16, left: u16, top: u16, c: [u8; 4], cs: u16| {
                let mut px: Vec<u8> = c.repeat(usize::from(w * h));
                let mut f = gif::Frame::from_rgba(w, h, &mut px);
                (f.left, f.top, f.delay) = (left, top, cs);
                enc.write_frame(&f).unwrap();
            };
            frame(4, 2, 0, 0, [255, 0, 0, 255], 5);
            frame(4, 2, 0, 0, [0, 255, 0, 255], 10);
            frame(2, 1, 1, 1, [0, 0, 255, 255], 0);
        }
        out
    }

    fn png() -> Vec<u8> {
        let img = RgbaImage::from_fn(3, 2, |x, y| Rgba([x as u8 * 80, y as u8 * 100, 7, 200]));
        let mut out = Cursor::new(Vec::new());
        img.write_to(&mut out, ImageFormat::Png).unwrap();
        out.into_inner()
    }

    /// A header's canvas is not an allocation (backlog FZ3, from the first
    /// fuzz round): fifteen bytes of GIF claiming 52428 by 17356 asked
    /// `decode_animation` for 3.6 GB, which no limit stood between.
    #[test]
    fn a_canvas_past_the_ceiling_is_refused() {
        let gif = b"GIF89a\xcc\xcc\xcc\x43\x3c\x00\x3c\x00\x3c";
        assert!(decode_animation(gif).is_err());
        assert!(decode_image(gif).is_err());
        assert_eq!(frame_bytes(52428, 17356), 3_639_761_472);
        assert!(frame_bytes(52428, 17356) > MAX_FRAME_BYTES);
    }

    #[test]
    fn a_png_decodes_to_straight_rgba() {
        let p = decode_image(&png()).unwrap();
        assert_eq!((p.width, p.height), (3, 2));
        assert_eq!(p.rgba.len(), 3 * 2 * 4);
        assert_eq!(&p.rgba[4..8], &[80, 0, 7, 200], "not premultiplied");
    }

    #[test]
    fn a_gif_decodes_to_its_first_frame_and_to_every_frame() {
        let bytes = gif(Repeat::Infinite);
        let first = decode_image(&bytes).unwrap();
        assert_eq!((first.width, first.height), (4, 2));
        assert_eq!(&first.rgba[..4], &[255, 0, 0, 255]);
        let a = decode_animation(&bytes).unwrap();
        assert_eq!(a.frames.len(), 3);
        assert_eq!(a.loops, None);
        let delays: Vec<f64> = a.frames.iter().map(|f| f.delay).collect();
        assert_eq!(delays, vec![0.05, 0.1, 0.1], "a 0 ms frame shows for 100");
        for f in &a.frames {
            assert_eq!(f.rgba.len(), 4 * 2 * 4, "each frame the whole canvas");
        }
        let third = &a.frames[2].rgba;
        assert_eq!(
            &third[..4],
            &[0, 255, 0, 255],
            "the green kept under the patch"
        );
        assert_eq!(&third[(4 + 1) * 4..(4 + 1) * 4 + 4], &[0, 0, 255, 255]);
    }

    #[test]
    fn at_walks_the_frames_and_loops() {
        let a = decode_animation(&gif(Repeat::Infinite)).unwrap();
        let near = |s: Showing, i: usize, next: f64| s.index == i && (s.next - next).abs() < 1e-9;
        assert!(near(a.at(0.0), 0, 0.05));
        assert!(near(a.at(0.07), 1, 0.15));
        assert!(near(a.at(0.2), 2, 0.25));
        assert!(near(a.at(0.26), 0, 0.30), "{:?}", a.at(0.26));
        assert!(near(a.at(-1.0), 0, 0.05), "before the start");
    }

    #[test]
    fn a_finite_gif_rests_on_its_last_frame() {
        let a = decode_animation(&gif(Repeat::Finite(1))).unwrap();
        assert_eq!(a.loops, Some(1));
        assert_eq!(a.at(0.1).index, 1);
        let end = a.at(0.3);
        assert_eq!(end.index, 2);
        assert!(end.next.is_infinite());
    }

    #[test]
    fn a_still_is_one_frame_for_ever() {
        let a = decode_animation(&png()).unwrap();
        assert_eq!(a.frames.len(), 1);
        let s = a.at(100.0);
        assert_eq!(s.index, 0);
        assert!(s.next.is_infinite());
    }

    #[test]
    fn what_is_not_an_image_is_named() {
        let e = decode_image(b"hello, world").unwrap_err();
        assert!(e.contains("PNG, JPEG, WebP or GIF"), "{e}");
        let mut cut = png();
        cut.truncate(40);
        let e = decode_image(&cut).unwrap_err();
        assert!(e.contains("decoding a Png"), "{e}");
    }
}
