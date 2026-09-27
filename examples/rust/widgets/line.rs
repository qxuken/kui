//! The `line` element (`docs/adr/0010-a-segment-primitive.md`): a small
//! mind map whose links are `line` nodes — one curve per link, drawn in the
//! canvas's box space between the floats it connects, instead of the three
//! thin boxes a box-only vocabulary forces. Each link is the cubic Bézier
//! every flow chart draws, sampled here into the polyline the element
//! takes (see `link`). Hover a card and its links
//! brighten: the stroke colour rides the `bg` slot, so a `transition` eases
//! it like any background. The links are declared before the cards, so
//! they paint under them (floats stack in tree order).
//!
//! Run: cargo run -p kui-native --example line

use kui_devtools::Example;
use kui_native::{App, FloatConfig, NodeSpec, Stroke, TextStyle, Ui, Vec2};

struct Card {
    label: &'static str,
    /// Top-left in the canvas, logical px.
    at: Vec2,
    parent: Option<usize>,
}

const W: f32 = 120.0;
const H: f32 = 36.0;

/// Samples the link from `from` to `to` into `out`: the cubic Bézier whose
/// two handles sit halfway across the gap, level with the edge each end
/// leaves, so the stroke departs the parent and meets the child dead
/// horizontal.
///
/// The four points are deliberately *not* handed to [`Stroke::curve`]. A
/// spline passes **through** its knots, so the two handles would become
/// places the line has to visit, and a curve that has to arrive at a
/// corner leans into it: the link out of `kui` bows about 5px below the
/// card's edge before it turns up, and the three links out of that one
/// card cross each other doing it. (It used to bow 10px — the core's
/// spline was uniformly parameterized until this example was drawn. It is
/// centripetal now, which halves the lean without removing it.) As Bézier
/// handles the same four points are only *pulled* towards, so the curve
/// stays inside them and leaves each card level.
///
/// One piece per [`kui_native::line::CURVE_STEP`] of control polygon, so the
/// sampling is as fine as the flattening the core would have done.
fn link(from: Vec2, to: Vec2, out: &mut Vec<Vec2>) {
    let h = (to.x - from.x) * 0.5;
    let (c1, c2) = (Vec2::new(from.x + h, from.y), Vec2::new(to.x - h, to.y));
    let span = h.abs() * 2.0 + (to.y - from.y).abs();
    let n = ((span / kui_native::line::CURVE_STEP).ceil() as usize).clamp(1, 64);
    out.clear();
    for i in 0..=n {
        let t = i as f32 / n as f32;
        let u = 1.0 - t;
        let (a, b, c, d) = (u * u * u, 3.0 * u * u * t, 3.0 * u * t * t, t * t * t);
        out.push(Vec2::new(
            a * from.x + b * c1.x + c * c2.x + d * to.x,
            a * from.y + b * c1.y + c * c2.y + d * to.y,
        ));
    }
}

struct Map {
    cards: Vec<Card>,
    /// Refilled per link per frame, so a frame allocates nothing.
    points: Vec<Vec2>,
}

impl Map {
    fn new() -> Self {
        let c = |label, x, y, parent| Card {
            label,
            at: Vec2::new(x, y),
            parent,
        };
        Map {
            cards: vec![
                c("kui", 60.0, 200.0, None),
                c("layout", 300.0, 60.0, Some(0)),
                c("paint", 300.0, 200.0, Some(0)),
                c("input", 300.0, 340.0, Some(0)),
                c("wrapping", 540.0, 30.0, Some(1)),
                c("floats", 540.0, 100.0, Some(1)),
                c("shadows", 540.0, 170.0, Some(2)),
                c("segments", 540.0, 240.0, Some(2)),
                c("focus", 540.0, 340.0, Some(3)),
            ],
            points: Vec::new(),
        }
    }
}

impl App for Map {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().bg(t.bg).pad(24.0).gap(12.0), |ui| {
        ui.text(
            "Links are `line` nodes: a Bézier sampled into a polyline, in the canvas's box space. Hover a card.",
            TextStyle::new(12.0).color(t.muted),
        );
        let canvas = NodeSpec::column()
            .fill()
            // The canvas is a panel on the page, not the page.
            .bg(t.surface)
            .border(1.0, t.border)
            .radius(12.0);
        ui.with_keyed("canvas", canvas, |ui| {
            // Which cards are hovered, read before anything is declared so
            // the links (declared first, painted under) can see it.
            let hovered: Vec<bool> = (0..self.cards.len())
                .map(|i| ui.is_hovered(ui.child_key(&format!("card{i}"))))
                .collect();
            for i in 0..self.cards.len() {
                let Some(p) = self.cards[i].parent else {
                    continue;
                };
                let from = Vec2::new(self.cards[p].at.x + W, self.cards[p].at.y + H / 2.0);
                let to = Vec2::new(self.cards[i].at.x, self.cards[i].at.y + H / 2.0);
                let lit = hovered[i] || hovered[p];
                // A lit link is the accent; a resting one is the strong
                // border, which is what every other hairline on the page is.
                let color = if lit { t.focus_ring } else { t.border_strong };
                link(from, to, &mut self.points);
                ui.polyline_keyed(
                    &format!("link{i}"),
                    &self.points,
                    Stroke::new(if lit { 3.0 } else { 2.0 }, color),
                    NodeSpec::column().transition(160.0),
                );
            }
            for (i, card) in self.cards.iter().enumerate() {
                ui.with_keyed(
                    &format!("card{i}"),
                    NodeSpec::row()
                        .float(FloatConfig::parent().offset(card.at.x, card.at.y))
                        .size(W, H)
                        .pad_xy(12.0, 0.0)
                        .cross_align(kui_native::Align::Center)
                        .bg(t.raised)
                        // An opaque step toward the accent rather than the
                        // translucent `accent_soft`: a `hover_bg` replaces
                        // the background, it does not composite over it.
                        .hover_bg(t.raised.mix(t.accent, 0.18))
                        // The border is what makes the card a card on the
                        // light base, where `raised` and the canvas it
                        // floats over are the same white (ADR 0019).
                        .border(1.0, t.border)
                        .radius(8.0)
                        .transition(160.0)
                        .hoverable(),
                    |ui| {
                        ui.text(card.label, TextStyle::new(13.0).color(t.fg));
                    },
                );
            }
        });
        });
    }
}

impl Example for Map {}

kui_devtools::main!(Map::new());
