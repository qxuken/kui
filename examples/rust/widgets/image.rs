//! The `image` element: host-registered RGBA pixels drawn through the
//! same atlas and draw call as everything else. Shows intrinsic (Fit)
//! sizing, aspect-preserving responsive width, rounded corners, and alpha
//! — and, since ADR 0025 (`docs/adr/0025-the-image-is-the-canvas.md`),
//! the image as the canvas: a *stream* whose pixels the app replaces
//! every frame with `update_image_with`, rendered at exactly the pixel count
//! the `layout` event's `scale` says the box covers, shown `nearest`
//! beside `linear`; and `contain` / `cover` against a box of another
//! aspect.
//!
//! Run: cargo run -p kui-native --example image [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{
    App, Core, ImageFit, ImageId, ImageOpts, NodeSpec, Sampling, Sizing, TextStyle, Ui, UiEvent,
};

/// A procedural "photo": vertical sky gradient with a sun disc.
fn sky(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    let (cx, cy, r) = (w as f32 * 0.72, h as f32 * 0.3, h as f32 * 0.16);
    for y in 0..h {
        let t = y as f32 / h as f32;
        for x in 0..w {
            let d = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
            let sun = (1.0 - ((d - r) / 6.0).clamp(0.0, 1.0)).powi(2);
            let base = [
                (30.0 + 160.0 * t) as u8,
                (60.0 + 120.0 * t) as u8,
                (120.0 + 80.0 * t) as u8,
            ];
            px.push(base[0].saturating_add((sun * 220.0) as u8));
            px.push(base[1].saturating_add((sun * 180.0) as u8));
            px.push(base[2].saturating_add((sun * 60.0) as u8));
            px.push(0xff);
        }
    }
    px
}

/// Checkerboard with transparent squares — alpha compositing over the bg.
fn checker(w: u32, h: u32) -> Vec<u8> {
    let mut px = Vec::with_capacity((w * h * 4) as usize);
    for y in 0..h {
        for x in 0..w {
            let on = ((x / 12) + (y / 12)) % 2 == 0;
            px.extend_from_slice(if on {
                &[0xd8, 0x86, 0x3b, 0xff]
            } else {
                &[0, 0, 0, 0]
            });
        }
    }
    px
}

/// A frame of plasma at `w`×`h`, `phase` along, written into `out`
/// (`w × h × 4` bytes): what a video decoder, a camera or a plot library
/// would hand back — pixels the app made.
fn plasma(w: u32, h: u32, phase: f32, out: &mut [u8]) {
    for (i, px) in out.as_chunks_mut::<4>().0.iter_mut().enumerate() {
        let (x, y) = (i as u32 % w, i as u32 / w);
        let (u, v) = (x as f32 / w as f32, y as f32 / h as f32);
        let a = ((u * 6.0 + phase).sin()
            + (v * 5.0 - phase * 0.7).sin()
            + ((u + v) * 4.0 + phase * 1.3).sin())
            / 3.0;
        *px = [
            (128.0 + 100.0 * a) as u8,
            (128.0 + 100.0 * (a + 2.1).sin()) as u8,
            (128.0 + 100.0 * (a + 4.2).sin()) as u8,
            0xff,
        ];
    }
}

struct Gallery {
    sky: Option<ImageId>,
    checker: Option<ImageId>,
    /// The stream: registered once at a token size, then replaced every
    /// frame at the size the layout event last reported.
    stream: Option<ImageId>,
    /// Physical pixels the stream's box covers, from `on_layout`'s `w`,
    /// `h` and `scale`: what the next frame renders to. Zero until the
    /// first layout arrives, which is the frame model — one frame late.
    stream_px: (u32, u32),
    phase: f32,
    /// How many frames were rendered at the reported size — the
    /// headless drive's evidence that the loop closed.
    rendered: u32,
}

impl App for Gallery {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // Register once, lazily, through the escape hatch — resources are
        // long-lived core state, not per-frame data.
        let sky_id = *self
            .sky
            .get_or_insert_with(|| ui.core().resources.add_image(480, 270, sky(480, 270)));
        let checker_id = *self
            .checker
            .get_or_insert_with(|| ui.core().resources.add_image(96, 96, checker(96, 96)));
        let stream_id = *self
            .stream
            .get_or_insert_with(|| ui.core().resources.add_image(16, 9, vec![0; 16 * 9 * 4]));
        // The loop: render at the size the box covers, replace the pixels,
        // keep the handle. Before the first `layout` event the token
        // 16×9 shows, stretched — one frame. `update_image_with` hands
        // over a buffer the core recycles, so the plasma is rendered
        // straight into it: no buffer of the app's own, no copy, and no
        // allocation after the second frame (backlog W20).
        let (pw, ph) = self.stream_px;
        if pw > 0 && ph > 0 {
            let phase = self.phase;
            ui.core()
                .update_image_with(stream_id, pw, ph, |px| plasma(pw, ph, phase, px));
            self.rendered += 1;
        }
        self.phase += 0.04;

        ui.with(
            NodeSpec::column().fill().pad(24.0).gap(16.0).scroll_y(),
            |ui| {
                let muted = TextStyle::new(12.0).color(t.muted);

                ui.text(
                    "Grow width + Fit height: rescales with the window, keeps aspect",
                    muted,
                );
                ui.image(
                    sky_id,
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .max_width(720.0)
                        .radius(12.0)
                        .label("a generated sky gradient"),
                );

                ui.text(
                    "Intrinsic size, rounded, over a colored card (alpha shows through)",
                    muted,
                );
                ui.with(
                    NodeSpec::row()
                        .pad(16.0)
                        .gap(16.0)
                        .bg(t.surface)
                        .radius(10.0),
                    |ui| {
                        ui.image(
                            checker_id,
                            NodeSpec::column()
                                .radius(8.0)
                                .label("a checkerboard, at its intrinsic size"),
                        );
                        ui.image(
                            checker_id,
                            NodeSpec::column()
                                .width(Sizing::Fixed(48.0))
                                .height(Sizing::Fixed(96.0))
                                .radius(8.0)
                                .label("the same checkerboard, stretched to 48x96"),
                        );
                        ui.text("same image, intrinsic and stretched", muted);
                    },
                );

                ui.text(
                    "A stream: pixels replaced every frame at the size the box covers (`layout.scale`), linear and nearest",
                    muted,
                );
                ui.with(NodeSpec::row().gap(16.0), |ui| {
                    // The box that reports its size; the stream node
                    // itself sits inside it so the report is the box's.
                    ui.with_keyed(
                        "stream-box",
                        NodeSpec::column()
                            .width(Sizing::Fixed(240.0))
                            .height(Sizing::Fixed(135.0))
                            .on_layout(kui_native::Value::str("stream"))
                            .animate(),
                        |ui| {
                            ui.image_with(
                                stream_id,
                                ImageOpts::default(),
                                NodeSpec::column().fill().radius(8.0).label("a plasma, rendered at the box's size"),
                            );
                        },
                    );
                    // The same pixels through `nearest`, at a size the
                    // texels are bigger than the pixels: each one a square.
                    ui.image_with(
                        stream_id,
                        ImageOpts {
                            sampling: Sampling::Nearest,
                            ..ImageOpts::default()
                        },
                        NodeSpec::column()
                            .width(Sizing::Fixed(240.0))
                            .height(Sizing::Fixed(135.0))
                            .radius(8.0)
                            .label("the same stream, nearest-sampled"),
                    );
                });

                ui.text(
                    "The 16:9 sky in a square box: `contain` letterboxes, `cover` crops, the box is the same",
                    muted,
                );
                ui.with(NodeSpec::row().gap(16.0), |ui| {
                    for (fit, label) in [
                        (ImageFit::Fill, "fill: stretched"),
                        (ImageFit::Contain, "contain: letterboxed"),
                        (ImageFit::Cover, "cover: cropped"),
                    ] {
                        ui.with(
                            NodeSpec::column()
                                .width(Sizing::Fixed(140.0))
                                .height(Sizing::Fixed(140.0))
                                .bg(t.surface)
                                .border(1.0, t.border)
                                .radius(10.0),
                            |ui| {
                                ui.image_with(
                                    sky_id,
                                    ImageOpts {
                                        fit,
                                        ..ImageOpts::default()
                                    },
                                    NodeSpec::column().fill().radius(10.0).label(label),
                                );
                            },
                        );
                    }
                });
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.get("kind").and_then(|v| v.as_str()) == Some("layout")
            && ev.payload.get("tag").and_then(|v| v.as_str()) == Some("stream")
        {
            let f = |k: &str| ev.payload.get(k).and_then(|v| v.as_float()).unwrap_or(0.0);
            // `scale` is the number the ADR put on the payload for exactly
            // this multiply: physical px per logical px at the node.
            self.stream_px = (
                (f("w") * f("scale")).round() as u32,
                (f("h") * f("scale")).round() as u32,
            );
        }
    }
}

impl Example for Gallery {
    /// The loop closes: after the first frame's layout report the stream
    /// is re-rendered at the box's pixel count and drawn from a texture
    /// of its own; the three fits share one box size and differ in what
    /// is painted.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        // Tall enough that the scrolling page culls nothing.
        let mut d = Drive::new(core, 900.0, 1100.0);
        d.frame(self);
        d.check(
            self.rendered == 0,
            "before the layout report nothing is rendered",
        )?;
        d.check(
            self.stream_px == (240, 135),
            "the report says how many pixels the box covers",
        )?;
        d.frame(self);
        d.check(
            self.rendered == 1,
            "the frame after renders once at that size",
        )?;
        // Read everything off the frame first; `check` wants the drive.
        let (textures, size, nearest, sky) = {
            let dl = &d.core.output().0;
            let texture = |q: &&kui_native::Quad| q.kind == kui_native::QuadKind::Texture;
            (
                dl.quads.iter().filter(texture).count(),
                dl.texture_pixels.first().map(|p| (p.width, p.height)),
                dl.quads
                    .iter()
                    .filter(texture)
                    .filter(|q| q.border_w == 1.0)
                    .count(),
                dl.quads
                    .iter()
                    .filter(|q| q.kind == kui_native::QuadKind::Image && q.rect.w >= 100.0)
                    .copied()
                    .collect::<Vec<_>>(),
            )
        };
        d.check(
            textures == 2,
            "the stream draws from a texture of its own, twice",
        )?;
        d.check(
            size == Some((240, 135)),
            "and the texture is the box's size",
        )?;
        d.check(nearest == 1, "one of the two is nearest-sampled")?;
        // The sky three ways: same box, `contain` paints a shorter rect,
        // `cover` shows fewer texels.
        d.check(sky.len() == 4, "the sky is drawn four times as an image")?;
        let fits = &sky[1..];
        d.check(
            fits[0].rect.h > fits[1].rect.h,
            "contain paints a shorter rect than fill",
        )?;
        d.check(
            fits[2].uv[2] < fits[0].uv[2],
            "cover shows fewer texels than fill",
        )?;
        Ok(())
    }
}

kui_devtools::main!(Gallery {
    sky: None,
    checker: None,
    stream: None,
    stream_px: (0, 0),
    phase: 0.0,
    rendered: 0,
});
