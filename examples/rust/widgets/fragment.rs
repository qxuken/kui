//! The `fragment` element: a box a WGSL function paints
//! (`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`).
//!
//! Four of them, each one function the app wrote:
//!
//! - a **gradient**, which is the thing ADR 0005 declined to build as props
//!   and this is the answer to;
//! - a **ring**, drawn from the prelude's `kui_sd_rounded_box` — a gauge
//!   with no geometry and no second pass;
//! - a **shimmer**, which reads `time` and so declares `animate`;
//! - a **card**, a fragment holding a title and a button, to show that
//!   children paint over it and take input normally;
//! - a **heatmap**, a fragment reading an `image` the app rewrites every
//!   frame with `update_image` — 64×16 values in a texture, one cell per
//!   texel through `kui_sample_nearest`, coloured between two theme roles
//!   (backlog V1, ADR 0025 decision 7): the "many points" case, where the
//!   data is a texture and the nodes are one;
//! - a **ripple**, the same input as an image effect — a registered icon
//!   read through `kui_sample` with a time-driven offset, so the pixels
//!   come from the atlas the icon lives in and the function never knows.
//!
//! The click target is the card's button, and the fragments themselves are
//! hoverable, so hovering one lifts its ring — a fragment takes input like
//! any box.
//!
//! Run: cargo run -p kui --example fragment [-- --headless]

use kui::{
    App, Color, Core, FragmentId, ImageId, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value, widgets,
};
use kui_devtools::{Drive, Example};

/// One colour as the four floats a `params` slot is. Fragment parameters
/// are where a theme meets a shader: the WGSL says *what* a gradient or a
/// ring is, and the palette says which colours it is made of — so every
/// fragment below follows the OS without a line of its shader changing
/// (`docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`).
fn rgba(c: Color) -> [f32; 4] {
    [c.r, c.g, c.b, c.a]
}

/// The four params a fragment takes, flattened: colours as `rgba`, plain
/// numbers as themselves.
fn params(slots: [[f32; 4]; 4]) -> [f32; 16] {
    let mut out = [0.0; 16];
    for (i, s) in slots.iter().enumerate() {
        out[i * 4..i * 4 + 4].copy_from_slice(s);
    }
    out
}

/// A vertical gradient between `params[0]` and `params[1]`.
const GRADIENT: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let t = clamp(in.local.y / max(in.size.y, 1.0), 0.0, 1.0);
    return mix(params[0], params[1], t);
}";

/// A progress ring: `params[1].x` of the way round, in `params[0]`, on a
/// track of `params[2]`. The arc is an angle test against the same
/// distance field the renderer draws every rounded box with.
const RING: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let c = in.size * 0.5;
    let p = in.local - c;
    let r = min(c.x, c.y) - params[3].x;
    let d = abs(length(p) - r) - params[3].y;
    let cov = 1.0 - smoothstep(-KUI_AA, KUI_AA, d);
    // Angle from twelve o'clock, clockwise, in turns.
    let turn = fract(atan2(p.x, -p.y) / 6.2831853 + 1.0);
    let on = step(turn, clamp(params[1].x, 0.0, 1.0));
    let col = mix(params[2].rgb, params[0].rgb, on);
    return vec4<f32>(col, cov);
}";

/// A diagonal sheen sliding across a dim base — the skeleton-loading
/// shimmer, as one function of `time`.
const SHIMMER: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let u = (in.local.x + in.local.y) / max(in.size.x + in.size.y, 1.0);
    let sweep = fract(in.time * params[2].x);
    let d = abs(fract(u - sweep + 0.5) - 0.5);
    let band = 1.0 - smoothstep(0.0, params[2].y, d);
    return vec4<f32>(mix(params[0].rgb, params[1].rgb, band), 1.0);
}";

/// A soft radial wash for the card to sit on.
const WASH: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let c = in.size * 0.5;
    let d = length((in.local - c) / max(c.x, 1.0));
    return vec4<f32>(mix(params[0].rgb, params[1].rgb, clamp(d, 0.0, 1.0)), 1.0);
}";

/// One cell per texel of the `image`, its red channel the value, coloured
/// between `params[0]` (cold) and `params[1]` (hot). `in.image.zw` is the
/// grid's size, which is what draws the cell borders without a second
/// input.
const HEATMAP: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let uv = in.local / max(in.size, vec2<f32>(1.0));
    let v = kui_sample_nearest(uv).r;
    let cell = fract(uv * in.image.zw);
    let edge = min(min(cell.x, 1.0 - cell.x), min(cell.y, 1.0 - cell.y));
    let border = smoothstep(0.0, 0.08, edge);
    return vec4<f32>(mix(params[0].rgb, params[1].rgb, v) * (0.6 + 0.4 * border), 1.0);
}";

/// An image effect: the `image` sampled through a horizontal ripple that
/// travels with `time`, desaturated by `params[0].y`.
const RIPPLE: &str = "\
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let uv = in.local / max(in.size, vec2<f32>(1.0));
    let wobble = vec2<f32>(sin(uv.y * 18.0 + in.time * 3.0) * params[0].x, 0.0);
    let c = kui_sample(uv + wobble);
    let grey = dot(c.rgb, vec3<f32>(0.299, 0.587, 0.114));
    return vec4<f32>(mix(vec3<f32>(grey), c.rgb, params[0].y), c.a);
}";

const HEAT_W: u32 = 64;
const HEAT_H: u32 = 16;

/// The heatmap's frame: a field of two travelling waves, one value per
/// texel in the red channel — what a sensor grid, a spectrogram column or
/// a 50k-point series would hand back.
fn heat(phase: f32, out: &mut Vec<u8>) {
    out.clear();
    out.reserve((HEAT_W * HEAT_H * 4) as usize);
    for y in 0..HEAT_H {
        for x in 0..HEAT_W {
            let (u, v) = (x as f32 / HEAT_W as f32, y as f32 / HEAT_H as f32);
            let a = ((u * 9.0 + phase).sin() * (v * 5.0 - phase * 0.6).cos() + 1.0) * 0.5;
            out.extend_from_slice(&[(a * 255.0) as u8, 0, 0, 0xff]);
        }
    }
}

/// A 48×48 icon for the ripple: a ring on a disc, opaque in the middle
/// and transparent past the disc, so the effect has an alpha to keep.
fn icon() -> Vec<u8> {
    let n = 48u32;
    let mut px = Vec::with_capacity((n * n * 4) as usize);
    for y in 0..n {
        for x in 0..n {
            let d = ((x as f32 - 23.5).powi(2) + (y as f32 - 23.5).powi(2)).sqrt();
            let (r, g, b, a) = if d > 22.0 {
                (0, 0, 0, 0)
            } else if (d - 15.0).abs() < 3.0 {
                (0xf4, 0xd0, 0x6f, 0xff)
            } else if d < 8.0 {
                (0x3b, 0x5b, 0xd4, 0xff)
            } else {
                (0x2a, 0x2e, 0x3c, 0xff)
            };
            px.extend_from_slice(&[r, g, b, a]);
        }
    }
    px
}

#[derive(Default)]
struct Demo {
    shaders: Option<Shaders>,
    /// 0..1, what the ring shows. The button nudges it.
    progress: f32,
    /// The heatmap's data texture: registered once, replaced every frame.
    heat: Option<ImageId>,
    /// The ripple's icon: registered once, never replaced, so it lives in
    /// the atlas and the fragment reads it from there.
    icon: Option<ImageId>,
    phase: f32,
    pixels: Vec<u8>,
}

#[derive(Clone, Copy)]
struct Shaders {
    gradient: FragmentId,
    ring: FragmentId,
    shimmer: FragmentId,
    wash: FragmentId,
    heatmap: FragmentId,
    ripple: FragmentId,
}

fn label(ui: &mut Ui<'_>, text: &str) {
    let muted = ui.theme().muted;
    ui.text(text, TextStyle::new(12.0).color(muted));
}

/// One labelled tile.
fn tile(ui: &mut Ui<'_>, name: &str, f: impl FnOnce(&mut Ui<'_>)) {
    ui.with_keyed(name, NodeSpec::column().gap(6.0), |ui| {
        f(ui);
        label(ui, name);
    });
}

impl App for Demo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // Registration is idempotent by source, so calling it every frame
        // costs a comparison. A real app would still do this once.
        let s = *self.shaders.get_or_insert_with(|| {
            let core = ui.core();
            Shaders {
                gradient: core.add_fragment(GRADIENT).expect("gradient"),
                ring: core.add_fragment(RING).expect("ring"),
                shimmer: core.add_fragment(SHIMMER).expect("shimmer"),
                wash: core.add_fragment(WASH).expect("wash"),
                heatmap: core.add_fragment(HEATMAP).expect("heatmap"),
                ripple: core.add_fragment(RIPPLE).expect("ripple"),
            }
        });
        // The data texture: a handle once, then new pixels every frame
        // through the same handle — the image is the canvas (ADR 0025),
        // and the fragment is what draws it as cells rather than pixels.
        let heat_id = *self.heat.get_or_insert_with(|| {
            ui.core()
                .resources
                .add_image(HEAT_W, HEAT_H, vec![0; (HEAT_W * HEAT_H * 4) as usize])
        });
        self.phase += 0.05;
        heat(self.phase, &mut self.pixels);
        ui.core()
            .update_image(heat_id, HEAT_W, HEAT_H, self.pixels.clone());
        let icon_id = *self
            .icon
            .get_or_insert_with(|| ui.core().resources.add_image(48, 48, icon()));

        ui.with(
            NodeSpec::column().fill().pad(28.0).gap(24.0).bg(t.bg),
            |ui| {
                ui.text("fragments", TextStyle::new(22.0).color(t.fg));

                ui.with(NodeSpec::row().gap(20.0), |ui| {
                    tile(ui, "gradient", |ui| {
                        ui.fragment(
                            s.gradient,
                            &params([
                                rgba(t.accent),  // top
                                rgba(t.surface), // bottom
                                [0.0; 4],
                                [0.0; 4],
                            ]),
                            NodeSpec::column()
                                .width(Sizing::Fixed(150.0))
                                .height(Sizing::Fixed(96.0))
                                .radius(10.0),
                        );
                    });

                    tile(ui, "ring", |ui| {
                        ui.fragment(
                            s.ring,
                            &params([
                                rgba(t.accent),                 // the arc
                                [self.progress, 0.0, 0.0, 0.0], // how far round
                                rgba(t.border),                 // the track
                                [10.0, 5.0, 0.0, 0.0],          // inset, half-width
                            ]),
                            NodeSpec::column()
                                .width(Sizing::Fixed(96.0))
                                .height(Sizing::Fixed(96.0)),
                        );
                    });

                    tile(ui, "shimmer (animate)", |ui| {
                        ui.fragment(
                            s.shimmer,
                            &params([
                                rgba(t.sunken),         // base
                                rgba(t.border_strong),  // sheen
                                [0.35, 0.22, 0.0, 0.0], // turns per second, width
                                [0.0; 4],
                            ]),
                            NodeSpec::column()
                                .width(Sizing::Fixed(150.0))
                                .height(Sizing::Fixed(96.0))
                                .radius(10.0)
                                .animate(),
                        );
                    });
                });

                ui.with(NodeSpec::row().gap(20.0), |ui| {
                    // The image input: a texture the app replaces every
                    // frame, read as data — one node for a thousand cells.
                    tile(ui, "heatmap (an image as data)", |ui| {
                        ui.fragment_keyed(
                            "heatmap",
                            s.heatmap.with_image(heat_id),
                            &params([
                                rgba(t.sunken), // cold
                                rgba(t.accent), // hot
                                [0.0; 4],
                                [0.0; 4],
                            ]),
                            NodeSpec::column()
                                .width(Sizing::Fixed(256.0))
                                .height(Sizing::Fixed(64.0))
                                .radius(6.0)
                                .animate(),
                        );
                    });

                    // The same input as an effect over an atlas-backed icon.
                    tile(ui, "ripple (an image effect)", |ui| {
                        ui.fragment_keyed(
                            "ripple",
                            s.ripple.with_image(icon_id),
                            &params([
                                [0.02, 0.35, 0.0, 0.0], // ripple depth, colour kept
                                [0.0; 4],
                                [0.0; 4],
                                [0.0; 4],
                            ]),
                            NodeSpec::column()
                                .width(Sizing::Fixed(96.0))
                                .height(Sizing::Fixed(96.0))
                                .animate(),
                        );
                    });
                });

                // A fragment with children: they lay out inside it and paint over
                // it, and the button takes input exactly as it would anywhere.
                tile(ui, "a fragment with children", |ui| {
                    ui.fragment_with(
                        s.wash,
                        &params([
                            rgba(t.surface.mix(t.accent, 0.30)), // centre
                            rgba(t.surface),                     // edge
                            [0.0; 4],
                            [0.0; 4],
                        ]),
                        NodeSpec::column()
                            .width(Sizing::Fixed(320.0))
                            .pad(18.0)
                            .gap(12.0)
                            .radius(12.0),
                        |ui| {
                            ui.text("on a wash", TextStyle::new(16.0).color(t.fg));
                            widgets::button(ui, "advance", Value::str("advance"));
                        },
                    );
                });
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.as_str() == Some("advance") {
            self.progress = (self.progress + 0.125) % 1.125;
        }
    }
}

impl Example for Demo {
    /// The image input lands on the right binding: the heatmap's data
    /// texture takes the frame's one `textures` entry and its draw names
    /// it, the ripple's icon is read from the atlas, no texture *quad* is
    /// drawn for either, and the next frame's replacement moves the
    /// revision the backend re-uploads on.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        use kui::{FragmentImage, QuadKind};
        let mut d = Drive::new(core, 900.0, 700.0);
        d.frame(self);
        let (images, textures, texture_quads, rev) = {
            let dl = &d.core.output().0;
            (
                dl.fragments.iter().map(|f| f.image).collect::<Vec<_>>(),
                dl.textures.len(),
                dl.quads
                    .iter()
                    .filter(|q| q.kind == QuadKind::Texture)
                    .count(),
                dl.texture_pixels.first().map(|p| p.rev),
            )
        };
        d.check(images.len() == 6, "six fragments drawn")?;
        d.check(
            images
                .iter()
                .filter(|i| matches!(i, FragmentImage::None))
                .count()
                == 4,
            "four of them read no image",
        )?;
        d.check(
            images.contains(&FragmentImage::Texture {
                index: 0,
                uv: [0, 0, HEAT_W, HEAT_H],
            }),
            "the heatmap reads the data texture, whole",
        )?;
        d.check(
            images
                .iter()
                .any(|i| matches!(i, FragmentImage::Atlas([_, _, 48, 48]))),
            "the ripple reads the icon from the atlas",
        )?;
        d.check(textures == 1, "one texture entry, the heatmap's")?;
        d.check(
            texture_quads == 0,
            "and no texture quad: the fragment draws",
        )?;
        d.frame(self);
        let rev2 = d.core.output().0.texture_pixels.first().map(|p| p.rev);
        d.check(
            rev2 > rev,
            "the next frame's update_image moved the revision",
        )?;
        Ok(())
    }
}

kui_devtools::main!(Demo::default());
