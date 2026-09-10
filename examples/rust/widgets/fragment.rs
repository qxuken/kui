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
//!   children paint over it and take input normally.
//!
//! The click target is the card's button, and the fragments themselves are
//! hoverable, so hovering one lifts its ring — a fragment takes input like
//! any box.
//!
//! Run: cargo run -p kui --example fragment

use kui::{App, Color, FragmentId, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value, widgets};
use kui_devtools::Example;

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

#[derive(Default)]
struct Demo {
    shaders: Option<Shaders>,
    /// 0..1, what the ring shows. The button nudges it.
    progress: f32,
}

#[derive(Clone, Copy)]
struct Shaders {
    gradient: FragmentId,
    ring: FragmentId,
    shimmer: FragmentId,
    wash: FragmentId,
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
            }
        });

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

impl Example for Demo {}

kui_devtools::main!(Demo::default());
