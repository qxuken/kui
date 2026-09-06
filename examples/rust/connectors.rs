//! The stroke primitive (`docs/adr/0010-a-segment-primitive.md`): a small
//! mind map whose links are `line` nodes — one curve per link, drawn in the
//! canvas's box space between the floats it connects, instead of the three
//! thin boxes a box-only vocabulary forces. Hover a card and its links
//! brighten: the stroke colour rides the `bg` slot, so a `transition` eases
//! it like any background. The links are declared before the cards, so
//! they paint under them (floats stack in tree order).
//!
//! Run: cargo run -p kui --example connectors

use kui::{App, Color, FloatConfig, NodeSpec, Sizing, Stroke, TextStyle, Ui, Vec2};

struct Card {
    label: &'static str,
    /// Top-left in the canvas, logical px.
    at: Vec2,
    parent: Option<usize>,
}

const W: f32 = 120.0;
const H: f32 = 36.0;

struct Map {
    cards: Vec<Card>,
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
        }
    }
}

impl App for Map {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::column().fill().pad(24.0).gap(12.0));
        kui::widgets::titlebar(ui, "kui — connectors");
        ui.text(
            "Links are `line` nodes: a curve through four points, in the canvas's box space. Hover a card.",
            TextStyle::new(12.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)),
        );
        let canvas = NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .height(Sizing::Grow(1.0))
            .bg(Color::rgb8(0x14, 0x16, 0x1e))
            .radius(12.0);
        ui.with_keyed("canvas", canvas, |ui| {
            // Which cards are hovered, read before anything is declared so
            // the links (declared first, painted under) can see it.
            let hovered: Vec<bool> = (0..self.cards.len())
                .map(|i| ui.is_hovered(ui.child_key(&format!("card{i}"))))
                .collect();
            for (i, card) in self.cards.iter().enumerate() {
                let Some(p) = card.parent else { continue };
                let from = Vec2::new(self.cards[p].at.x + W, self.cards[p].at.y + H / 2.0);
                let to = Vec2::new(card.at.x, card.at.y + H / 2.0);
                let mid = (from.x + to.x) / 2.0;
                let lit = hovered[i] || hovered[p];
                let color = if lit {
                    Color::rgb8(0x7f, 0x9c, 0xf5)
                } else {
                    Color::rgb8(0x3a, 0x3f, 0x52)
                };
                ui.polyline_keyed(
                    &format!("link{i}"),
                    &[from, Vec2::new(mid, from.y), Vec2::new(mid, to.y), to],
                    Stroke::new(if lit { 3.0 } else { 2.0 }, color).curve(),
                    NodeSpec::column().transition(160.0),
                );
            }
            for (i, card) in self.cards.iter().enumerate() {
                ui.with_keyed(
                    &format!("card{i}"),
                    NodeSpec::row()
                        .float(FloatConfig::parent().offset(card.at.x, card.at.y))
                        .width(Sizing::Fixed(W))
                        .height(Sizing::Fixed(H))
                        .pad_xy(12.0, 0.0)
                        .cross_align(kui::Align::Center)
                        .bg(Color::rgb8(0x24, 0x27, 0x33))
                        .hover_bg(Color::rgb8(0x30, 0x34, 0x4a))
                        .radius(8.0)
                        .transition(160.0)
                        .hoverable(),
                    |ui| {
                        ui.text(card.label, TextStyle::new(13.0));
                    },
                );
            }
        });
    }
}

fn main() {
    kui::app("kui — connectors").run(Map::new()).unwrap();
}
