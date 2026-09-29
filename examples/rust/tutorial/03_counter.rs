//! Step 3 — state and messages. Builds on step 2 by giving the app a
//! model, buttons that carry a message, and an `on_event` that moves the
//! model. Chapter 4 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_03_counter

use kui_native::widgets;
use kui_native::{App, Message, NodeSpec, TextStyle, Ui, UiEvent};

// ANCHOR: message
/// What the view can say to the app. `derive(Message)` turns each variant
/// into plain data (`{kind: "inc"}`) on the way out and back into `Msg`
/// on the way in.
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Inc,
    Dec,
    Reset,
}
// ANCHOR_END: message

// ANCHOR: model
#[derive(Default)]
struct Counter {
    count: i64,
}
// ANCHOR_END: model

impl App for Counter {
    // ANCHOR: view
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column().fill().center().gap(16.0).bg(t.bg),
            |ui| {
                ui.text(&self.count.to_string(), TextStyle::new(56.0).color(t.fg));
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    // A button takes the message it sends when clicked.
                    widgets::button(ui, "-1", Msg::Dec);
                    widgets::button(ui, "+1", Msg::Inc);
                    widgets::button(ui, "reset", Msg::Reset);
                });
            },
        );
    }
    // ANCHOR_END: view

    // ANCHOR: on_event
    /// Every event the frame produced comes here, one at a time, after
    /// the frame. A click carries the button's message.
    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Inc) => self.count += 1,
            Some(Msg::Dec) => self.count -= 1,
            Some(Msg::Reset) => self.count = 0,
            // Not ours: a resize, a focus change, a window event.
            None => {}
        }
    }
    // ANCHOR_END: on_event
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Counter")
        .size(360.0, 240.0)
        .run(Counter::default())
}
