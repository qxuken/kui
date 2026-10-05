//! The `polygon` element (`docs/adr/0025-the-image-is-the-canvas.md`,
//! decision 6): a filled outline of up to eight points, placed like a
//! `line` — a float in its parent's box space, sized to its own bounding
//! box — with the fill in `bg`, so a `transition` eases it like any
//! background. Four things a box-and-stroke vocabulary could not draw:
//! a pie whose wedges light up under the pointer, arrowheads on the
//! links of a small graph, the area under a sparkline, and a concave
//! star. Every one is a `fragment` quad on the wire, painted by the stock
//! source the core registers itself.
//!
//! A polygon is hit by its outline (`docs/adr/0026-hit-testing-by-shape.md`):
//! the wedges are `hoverable` themselves, and a pointer in one wedge's
//! bounding box but past its arc is over the neighbour, not it. Before
//! that ADR this example floated a hover box over each wedge's middle.
//!
//! What this pie shows that a chart would not want: an arc of seven
//! chords, and a hairline of the panel through every shared edge, since
//! two signed-distance fills each cover the edge's pixels by half. A
//! round pie that meets without a seam is the `path` element's
//! (`docs/adr/0040-a-path-is-a-mask-in-the-atlas.md`, `--example path`);
//! what `polygon` keeps is a fill that costs nothing per frame however it
//! moves — the arrowheads and the area strip below.
//!
//! Run: cargo run -p kui-native --example polygon [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{App, Color, Core, FloatConfig, NodeSpec, QuadKind, Stroke, TextStyle, Ui, Vec2};

/// A wedge of `frac` of the circle starting at `from` turns: the centre,
/// then the arc flattened into as many points as eight allows. Coarse for
/// a big wedge — a polygon is eight points, and a rounder pie is more
/// wedges, not more points — but the edge ramp hides the facets at this
/// radius.
fn wedge(center: Vec2, r: f32, from: f32, frac: f32, out: &mut Vec<Vec2>) {
    out.clear();
    out.push(center);
    let steps = 6;
    for i in 0..=steps {
        let a = (from + frac * i as f32 / steps as f32) * std::f32::consts::TAU;
        out.push(Vec2::new(center.x + r * a.cos(), center.y + r * a.sin()));
    }
    // Eight at most: the centre, seven on the arc.
    out.truncate(8);
}

/// A filled arrowhead at `tip` pointing along `dir`, `len` long.
fn arrowhead(tip: Vec2, dir: Vec2, len: f32) -> [Vec2; 3] {
    let n = Vec2::new(-dir.y, dir.x);
    let base = Vec2::new(tip.x - dir.x * len, tip.y - dir.y * len);
    let half = len * 0.45;
    [
        tip,
        Vec2::new(base.x + n.x * half, base.y + n.y * half),
        Vec2::new(base.x - n.x * half, base.y - n.y * half),
    ]
}

struct Demo {
    slices: Vec<(&'static str, f32, Color)>,
    points: Vec<Vec2>,
    samples: Vec<f32>,
}

impl App for Demo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().bg(t.bg).pad(24.0).gap(16.0), |ui| {
            let muted = TextStyle::new(12.0).color(t.muted);
            ui.text(
                "Fills the stroke vocabulary could not draw: a pie, arrowheads, an area, a star. Hover a wedge.",
                muted,
            );
            ui.with(NodeSpec::row().gap(24.0), |ui| {
                // The pie: each wedge is hoverable in its own outline, and
                // its fill eases toward the accent.
                let pie = NodeSpec::column()
                    .size(220.0, 220.0)
                    .bg(t.surface)
                    .border(1.0, t.border)
                    .radius(12.0);
                ui.with_keyed("pie", pie, |ui| {
                    let center = Vec2::new(110.0, 110.0);
                    let mut from = 0.0;
                    for (i, &(label, frac, color)) in self.slices.iter().enumerate() {
                        let key = ui.child_key(&format!("wedge{i}"));
                        let lit = ui.is_hovered(key);
                        wedge(center, 80.0, from, frac, &mut self.points);
                        ui.polygon_keyed(
                            &format!("wedge{i}"),
                            &self.points,
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
                    // Arrowheads: a stroke to the base, a triangle at the
                    // tip, both in the panel's box space.
                    let graph = NodeSpec::column()
                        .size(300.0, 102.0)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .radius(12.0);
                    ui.with(graph, |ui| {
                        let nodes = [
                            Vec2::new(40.0, 50.0),
                            Vec2::new(150.0, 24.0),
                            Vec2::new(150.0, 78.0),
                            Vec2::new(260.0, 50.0),
                        ];
                        for &(a, b) in &[(0, 1), (0, 2), (1, 3), (2, 3)] {
                            let (from, to) = (nodes[a], nodes[b]);
                            let d = Vec2::new(to.x - from.x, to.y - from.y);
                            let len = (d.x * d.x + d.y * d.y).sqrt();
                            let dir = Vec2::new(d.x / len, d.y / len);
                            let tip = Vec2::new(to.x - dir.x * 14.0, to.y - dir.y * 14.0);
                            let base = Vec2::new(tip.x - dir.x * 10.0, tip.y - dir.y * 10.0);
                            ui.line(from, base, Stroke::new(2.0, t.border_strong), NodeSpec::column());
                            ui.polygon(&arrowhead(tip, dir, 12.0), NodeSpec::column().bg(t.border_strong));
                        }
                        for n in nodes {
                            ui.leaf(
                                NodeSpec::column()
                                    .float(FloatConfig::parent().offset(n.x - 12.0, n.y - 12.0))
                                    .size(24.0, 24.0)
                                    .bg(t.raised)
                                    .border(1.0, t.border)
                                    .radius(12.0));
                        }
                    });

                    // The area under a sparkline: a strip of quads, one
                    // per span, so no polygon needs more than four points
                    // and the outline is the stroke over it.
                    let chart = NodeSpec::column()
                        .size(300.0, 102.0)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .radius(12.0);
                    ui.with(chart, |ui| {
                        let (w, h, pad) = (300.0, 102.0, 14.0);
                        let n = self.samples.len();
                        let x = |i: usize| pad + (w - 2.0 * pad) * i as f32 / (n - 1) as f32;
                        let y = |v: f32| h - pad - (h - 2.0 * pad) * v;
                        for i in 0..n - 1 {
                            let quad = [
                                Vec2::new(x(i), y(self.samples[i])),
                                Vec2::new(x(i + 1), y(self.samples[i + 1])),
                                Vec2::new(x(i + 1), y(0.0)),
                                Vec2::new(x(i), y(0.0)),
                            ];
                            ui.polygon(&quad, NodeSpec::column().bg(t.accent_soft));
                        }
                        self.points.clear();
                        self.points
                            .extend((0..n).map(|i| Vec2::new(x(i), y(self.samples[i]))));
                        ui.polyline(&self.points, Stroke::new(2.0, t.accent), NodeSpec::column());
                    });
                });

                // A concave outline fills correctly: the polygon SDF, not
                // a convex hull.
                let card = NodeSpec::column()
                    .size(120.0, 220.0)
                    .bg(t.surface)
                    .border(1.0, t.border)
                    .radius(12.0);
                ui.with(card, |ui| {
                    let c = Vec2::new(60.0, 110.0);
                    self.points.clear();
                    for i in 0..8 {
                        let a = i as f32 / 8.0 * std::f32::consts::TAU - std::f32::consts::FRAC_PI_2;
                        let r = if i % 2 == 0 { 46.0 } else { 20.0 };
                        self.points.push(Vec2::new(c.x + r * a.cos(), c.y + r * a.sin()));
                    }
                    ui.polygon(&self.points, NodeSpec::column().bg(t.warning));
                });
            });
        });
    }
}

impl Example for Demo {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(760.0, 320.0)
    }

    /// Every fill is one fragment quad; a wedge is hit by its outline, so a
    /// pointer inside it lights it and one in its bounding box past its
    /// arc lights the neighbour instead (ADR 0026).
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
        // 4 wedges + 4 arrowheads + 9 area quads + the star.
        let fills = count(d.core, QuadKind::Fragment);
        d.check(fills == 18, "every fill is one fragment quad")?;
        let wedges = d
            .core
            .interaction
            .hits()
            .iter()
            .filter(|h| {
                d.core
                    .label_of(h.key)
                    .is_some_and(|l| l.starts_with("wedge"))
            })
            .count();
        d.check(wedges == 4, "each wedge is a hit region of its own")?;
        let before = count(d.core, QuadKind::Fragment);
        let w1 = d.key_of("wedge1").ok_or("no second wedge")?;
        let r = d.rect_of(w1).ok_or("the wedge has no region")?;
        // Where the pie's centre is on the page: the wedge's region is its
        // outline's bounding box a pixel out, and the outline is what
        // `wedge` computes, so the panel's origin follows from the two.
        let (from, frac) = (self.slices[0].1, self.slices[1].1);
        let mut pts = Vec::new();
        wedge(Vec2::new(110.0, 110.0), 80.0, from, frac, &mut pts);
        let (min_x, min_y) = pts
            .iter()
            .fold((f32::MAX, f32::MAX), |(x, y), p| (x.min(p.x), y.min(p.y)));
        let c = kui_native::Vec2::new(r.x - (min_x - 1.0) + 110.0, r.y - (min_y - 1.0) + 110.0);
        let mid_angle = (from + frac * 0.5) * std::f32::consts::TAU;
        // Halfway out along the wedge's middle: inside it. Past the rim
        // along the same line, still inside its bounding box: not it.
        let inside =
            kui_native::Vec2::new(c.x + 50.0 * mid_angle.cos(), c.y + 50.0 * mid_angle.sin());
        let outside =
            kui_native::Vec2::new(c.x + 84.0 * mid_angle.cos(), c.y + 84.0 * mid_angle.sin());
        d.input(self, kui_native::InputEvent::CursorMoved(inside));
        d.check(
            d.core.is_hovered(w1),
            "a pointer inside the wedge hovers it",
        )?;
        d.input(self, kui_native::InputEvent::CursorMoved(outside));
        d.check(
            !d.core.is_hovered(w1),
            "a pointer in its box past its arc does not",
        )?;
        // Back inside: the fill eases to the accent, and stays one quad.
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
            .any(|q| q.kind == QuadKind::Fragment && q.color == accent);
        d.check(lit, "a hovered wedge's fill is the accent")?;
        let after = count(d.core, QuadKind::Fragment);
        d.check(after == before, "and it is still one quad")?;
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
    points: Vec::new(),
    samples: vec![0.2, 0.5, 0.35, 0.8, 0.6, 0.9, 0.55, 0.7, 0.4, 0.65],
});
