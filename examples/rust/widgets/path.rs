//! The `path` element (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`):
//! any outline — SVG's `d`, or the `Path` builder — filled with `bg` by
//! its rule and stroked by its own stroke, placed like a `line` (a float
//! in its parent's box space sized to its own bounding box). Four things
//! an eight-point `polygon` could not draw: a pie whose wedges are round
//! and meet without a seam, lighting up under the pointer; a donut gauge
//! whose arc is a sector with an inner radius; an icon from SVG path
//! data, filled and stroked; and an even-odd ring whose hole the press
//! falls through. Every paint is one glyph-mask quad from the atlas,
//! rasterized once per shape and scale and re-tinted for free, so a hover
//! or a colour tween costs nothing a text's does not.
//!
//! A path is hit by its outline under its fill rule (ADR 0026): the
//! wedges are `hoverable` themselves, and a pointer in one wedge's
//! bounding box but past its arc is over the canvas, not it.
//!
//! Run: cargo run -p kui-native --example path [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{App, Color, Core, FillRule, NodeSpec, Path, QuadKind, Stroke, TextStyle, Ui};

/// A heart, as an icon editor would export it: SVG path data, in a 24 px
/// box, drawn at any scale by the one parser the core owns.
const HEART: &str = "M12 21 C12 21 3 14.5 3 8.5 A4.5 4.5 0 0 1 12 6 A4.5 4.5 0 0 1 21 8.5 C21 14.5 12 21 12 21 Z";

struct Demo {
    slices: Vec<(&'static str, f32, Color)>,
    gauge: f32,
}

impl App for Demo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().bg(t.bg).pad(24.0).gap(16.0), |ui| {
            let muted = TextStyle::new(12.0).color(t.muted);
            ui.text(
                "Outlines of any shape, one mask quad each: a pie, a gauge, an icon, a ring. Hover a wedge.",
                muted,
            );
            ui.with(NodeSpec::row().gap(24.0), |ui| {
                // The pie: each wedge is a sector with a round arc, hit by
                // its outline, and its fill eases toward the accent. Two
                // wedges share each radial edge and meet without a seam,
                // because a fill bleeds half a pixel (ADR 0040, decision 5).
                let pie = NodeSpec::column()
                    .size(220.0, 220.0)
                    .bg(t.surface)
                    .border(1.0, t.border)
                    .radius(12.0);
                ui.with_keyed("pie", pie, |ui| {
                    let mut from = 0.0;
                    for (i, &(label, frac, color)) in self.slices.iter().enumerate() {
                        let key = ui.child_key(&format!("wedge{i}"));
                        let lit = ui.is_hovered(key);
                        ui.path_keyed(
                            &format!("wedge{i}"),
                            &Path::sector(110.0, 110.0, 80.0, 0.0, from, frac),
                            NodeSpec::column()
                                .bg(if lit { t.accent } else { color })
                                .transition(160.0)
                                .hoverable()
                                .label(label),
                        );
                        from += frac;
                    }
                });

                ui.with(NodeSpec::column().gap(16.0), |ui| {
                    // The gauge: a track ring, and over it a sector whose
                    // sweep is the value. Both are one node each.
                    let card = NodeSpec::column()
                        .size(300.0, 102.0)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .radius(12.0);
                    ui.with(card, |ui| {
                        let (cx, cy) = (60.0, 70.0);
                        ui.path(
                            &Path::sector(cx, cy, 44.0, 32.0, 0.5, 0.5),
                            NodeSpec::column().bg(t.raised),
                        );
                        ui.path_keyed(
                            "gauge",
                            &Path::sector(cx, cy, 44.0, 32.0, 0.5, 0.5 * self.gauge),
                            NodeSpec::column().bg(t.accent).transition(300.0),
                        );
                        ui.with(
                            NodeSpec::column()
                                .float(kui_native::FloatConfig::parent().offset(130.0, 30.0))
                                .gap(4.0),
                            |ui| {
                                ui.text(
                                    &format!("{:.0}%", self.gauge * 100.0),
                                    TextStyle::new(22.0).color(t.fg),
                                );
                                ui.text("of the frame budget", muted);
                            },
                        );
                    });

                    // The icon: SVG path data, filled and stroked, at twice
                    // and four times its 24 px box — the same `d`, two
                    // masks of different sizes in the atlas.
                    let strip = NodeSpec::column()
                        .size(300.0, 102.0)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .radius(12.0);
                    ui.with(strip, |ui| {
                        let heart = Path::parse(HEART).expect("the icon parses");
                        for (i, (scale, x)) in [(2.0, 24.0), (4.0, 110.0)].into_iter().enumerate() {
                            let ops: Vec<kui_native::PathOp> = heart
                                .ops()
                                .iter()
                                .map(|op| scaled(*op, scale, x, 3.0 + (4.0 - scale) * 12.0))
                                .collect();
                            ui.path_keyed(
                                &format!("heart{i}"),
                                &Path::from_ops(ops).stroked(Stroke::new(2.0, t.fg)),
                                NodeSpec::column().bg(t.warning),
                            );
                        }
                    });
                });

                // The ring: two contours wound the same way, filled
                // even-odd, so the hole is a hole — to the eye and to the
                // pointer, which hovers the ring and not its middle.
                let card = NodeSpec::column()
                    .size(120.0, 220.0)
                    .bg(t.surface)
                    .border(1.0, t.border)
                    .radius(12.0);
                ui.with(card, |ui| {
                    let key = ui.child_key("ring");
                    let lit = ui.is_hovered(key);
                    ui.path_keyed(
                        "ring",
                        &Path::parse("M60 60 m-46 0 a46 46 0 1 0 92 0 a46 46 0 1 0 -92 0 M60 60 m-20 0 a20 20 0 1 0 40 0 a20 20 0 1 0 -40 0")
                            .expect("the ring parses")
                            .fill_rule(FillRule::EvenOdd),
                        NodeSpec::column()
                            .bg(if lit { t.accent } else { t.accent_soft })
                            .transition(160.0)
                            .hoverable()
                            .label("ring"),
                    );
                    ui.with(
                        NodeSpec::column()
                            .float(kui_native::FloatConfig::parent().offset(10.0, 130.0))
                            .size(100.0, 80.0),
                        |ui| ui.text("even-odd: the hole is the card's", muted),
                    );
                });
            });
        });
    }
}

/// `op` scaled by `s` and moved to `(x, y)`.
fn scaled(op: kui_native::PathOp, s: f32, x: f32, y: f32) -> kui_native::PathOp {
    use kui_native::{PathOp, Vec2};
    let at = |p: Vec2| Vec2::new(p.x * s + x, p.y * s + y);
    match op {
        PathOp::MoveTo(p) => PathOp::MoveTo(at(p)),
        PathOp::LineTo(p) => PathOp::LineTo(at(p)),
        PathOp::QuadTo(c, p) => PathOp::QuadTo(at(c), at(p)),
        PathOp::CubicTo(a, b, p) => PathOp::CubicTo(at(a), at(b), at(p)),
        PathOp::ArcTo {
            rx,
            ry,
            rotation,
            large,
            sweep,
            to,
        } => PathOp::ArcTo {
            rx: rx * s,
            ry: ry * s,
            rotation,
            large,
            sweep,
            to: at(to),
        },
        PathOp::Close => PathOp::Close,
    }
}

impl Example for Demo {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(760.0, 320.0)
    }

    /// Every paint is one glyph-mask quad and no fragment; a wedge is hit
    /// by its arc, so a pointer inside it lights it and one in its box past
    /// the arc does not; the ring's hole is not the ring's.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 760.0, 320.0);
        d.frame(self);
        let count = |core: &mut Core, kind: QuadKind| {
            core.output()
                .0
                .quads
                .iter()
                .filter(|q| q.kind == kind)
                .count()
        };
        // 4 wedges + the track and the gauge + 2 icons × 2 paints + the ring.
        let masks = count(d.core, QuadKind::GlyphMask);
        d.check(masks >= 11, "every paint is one mask quad")?;
        let fragments = count(d.core, QuadKind::Fragment);
        d.check(fragments == 0, "and none is a fragment")?;
        let warned = d.warnings();
        d.check(warned.is_empty(), "nothing warned")?;
        let w1 = d.key_of("wedge1").ok_or("no second wedge")?;
        let r = d.rect_of(w1).ok_or("the wedge has no region")?;
        // The pie's centre on the page: the wedge's region is its outline's
        // bounding box a pixel out, and the outline is what `sector`
        // computes, so the panel's origin follows from the two.
        let (from, frac) = (self.slices[0].1, self.slices[1].1);
        let mut outline = Vec::new();
        kui_native::path::flatten(
            Path::sector(110.0, 110.0, 80.0, 0.0, from, frac).ops(),
            &mut outline,
        );
        let b = kui_native::path::bounds(&outline).ok_or("no bounds")?;
        let c = kui_native::Vec2::new(r.x - (b.x - 2.0) + 110.0, r.y - (b.y - 2.0) + 110.0);
        let mid = (from + frac * 0.5) * std::f32::consts::TAU;
        let inside = kui_native::Vec2::new(c.x + 50.0 * mid.cos(), c.y + 50.0 * mid.sin());
        let outside = kui_native::Vec2::new(c.x + 84.0 * mid.cos(), c.y + 84.0 * mid.sin());
        d.input(self, kui_native::InputEvent::CursorMoved(inside));
        d.check(d.core.is_hovered(w1), "a pointer inside the wedge hovers it")?;
        d.input(self, kui_native::InputEvent::CursorMoved(outside));
        d.check(!d.core.is_hovered(w1), "a pointer in its box past its arc does not")?;
        // Back inside: the fill eases to the accent, from the same slot.
        d.input(self, kui_native::InputEvent::CursorMoved(inside));
        d.frame(self);
        d.advance(0.5);
        d.frame(self);
        let accent = d.core.theme().accent;
        let lit = d
            .core
            .output()
            .0
            .quads
            .iter()
            .any(|q| q.kind == QuadKind::GlyphMask && q.color == accent);
        d.check(lit, "a hovered wedge's fill is the accent")?;
        // The ring: its band hovers, its hole does not.
        let ring = d.key_of("ring").ok_or("no ring")?;
        let rr = d.rect_of(ring).ok_or("the ring has no region")?;
        let centre = kui_native::Vec2::new(rr.x + rr.w * 0.5, rr.y + rr.h * 0.5);
        d.input(self, kui_native::InputEvent::CursorMoved(centre));
        d.check(!d.core.is_hovered(ring), "the ring's hole is not the ring's")?;
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(centre.x + 33.0, centre.y)),
        );
        d.check(d.core.is_hovered(ring), "its band is")?;
        Ok(())
    }
}

kui_devtools::main!(Demo {
    slices: vec![
        ("layout", 0.34, Color::hex(0x7f9cf5ff)),
        ("paint", 0.26, Color::hex(0xd8863bff)),
        ("input", 0.22, Color::hex(0x9ad9a0ff)),
        ("text", 0.18, Color::hex(0xe07a8aff)),
    ],
    gauge: 0.62,
});
