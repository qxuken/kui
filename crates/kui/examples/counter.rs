//! Minimal Elm-ish counter: `view(&mut self)` rebuilds the tree from state,
//! clicks arrive as data in `on_event`.
//!
//! Run: cargo run --example counter

use kui::widgets;
use kui::{Align, App, Color, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};

#[derive(Default)]
struct Counter {
    count: i64,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // The title is frame state like everything else; the runner diffs.
        ui.window_title(&format!("kui — counter ({})", self.count));
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
                ui.text(
                    "kui counter",
                    TextStyle::new(14.0).color(Color::rgb8(0x8a, 0x8f, 0xa3)),
                );
                ui.text(&self.count.to_string(), TextStyle::new(56.0));
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    widgets::button(ui, "-1", Value::map([("kind", "dec".into())]));
                    widgets::button(ui, "+1", Value::map([("kind", "inc".into())]));
                    // Floating tooltip: shown on hover (or until first click,
                    // so you can see it without a mouse).
                    let badge = ui.child_key("help");
                    ui.with_keyed(
                        "help",
                        NodeSpec::column()
                            .pad_xy(9.0, 4.0)
                            .bg(Color::rgb8(0x24, 0x27, 0x33))
                            .radius(10.0)
                            .on_click(Value::Null),
                        |ui| {
                            ui.text("?", TextStyle::new(13.0));
                            if ui.is_hovered(badge) || self.count == 0 {
                                widgets::tooltip(ui, "floats: out-of-flow, on top, unclipped");
                            }
                        },
                    );
                });
            },
        );

        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .main_align(Align::Center),
            |ui| {
                ui.text(
                    "clicks are data: view() never sees a callback",
                    TextStyle::new(13.0).color(Color::rgb8(0x5c, 0x61, 0x74)),
                );
            },
        );
        kui::widgets::latency_hud(ui);
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
