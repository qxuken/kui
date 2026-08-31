//! Minimal Elm-ish counter: `view(&mut self)` rebuilds the tree from state,
//! clicks arrive as data in `on_event`.
//!
//! Run: cargo run --example counter

use kui::widgets;
use kui::{App, Align, Color, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};

#[derive(Default)]
struct Counter {
    count: i64,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::column().fill().center().gap(24.0));

        ui.with(
            NodeSpec::column()
                .pad(32.0)
                .gap(20.0)
                .bg(Color::rgb8(0x16, 0x18, 0x20))
                .radius(12.0)
                .border(1.0, Color::rgb8(0x2a, 0x2d, 0x3a))
                .cross_align(Align::Center),
            |ui| {
                ui.text("kui counter", TextStyle::new(14.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)));
                ui.text(&self.count.to_string(), TextStyle::new(56.0));
                ui.with(NodeSpec::row().gap(12.0), |ui| {
                    widgets::button(ui, "-1", Value::map([("kind", "dec".into())]));
                    widgets::button(ui, "+1", Value::map([("kind", "inc".into())]));
                });
            },
        );

        ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).main_align(Align::Center), |ui| {
            ui.text(
                "clicks are data: view() never sees a callback",
                TextStyle::new(13.0).color(Color::rgb8(0x5c, 0x61, 0x74)),
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("inc") => self.count += 1,
            Some("dec") => self.count -= 1,
            _ => {}
        }
    }
}

fn main() {
    kui::run("kui — counter", Counter::default(), vec![]).unwrap();
}
