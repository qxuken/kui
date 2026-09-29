//! Step 8 — motion. Builds on step 7 by letting the core ease what
//! changes: a panel whose width eases between two values, and toasts
//! that slide in when declared and out when they stop being declared.
//! Chapter 9 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_08_motion

use kui_native::widgets;
use kui_native::{Align, App, Easing, Enter, Message, NodeSpec, TextStyle, Ui, UiEvent};

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Toggle,
    Notify,
    Dismiss { id: u64 },
}
// ANCHOR_END: message

#[derive(Default)]
struct Motion {
    wide: bool,
    toasts: Vec<(u64, String)>,
    next_id: u64,
}

impl App for Motion {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(20.0)
                .gap(12.0)
                .cross_align(Align::Start)
                .bg(t.bg),
            |ui| {
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    widgets::button(ui, "Toggle width", Msg::Toggle);
                    widgets::button(ui, "Notify", Msg::Notify);
                });

                // ANCHOR: transition
                // `transition(ms)`: when a value on this node changes
                // between frames, the core eases from the old to the new
                // over that long. The view just declares the target.
                ui.with_keyed(
                    "panel",
                    NodeSpec::column()
                        .width(if self.wide { 360.0 } else { 160.0 })
                        .height(80.0)
                        .center()
                        .bg(t.accent_soft)
                        .radius(10.0)
                        .transition(400.0)
                        .easing(Easing::Spring),
                    |ui| {
                        ui.text(
                            if self.wide { "wide" } else { "narrow" },
                            TextStyle::new(14.0).color(t.accent),
                        );
                    },
                );
                // ANCHOR_END: transition

                // ANCHOR: enter_exit
                // A toast enters from the right and leaves the same way.
                // The key is what makes this work: the core matches this
                // frame's `toast N` to last frame's and eases between
                // them. On the frame it is gone, `exit` replays it out.
                ui.with(
                    NodeSpec::column().gap(6.0).cross_align(Align::Start),
                    |ui| {
                        for (id, text) in &self.toasts {
                            ui.with_indexed(
                                *id,
                                NodeSpec::row()
                                    .pad_xy(12.0, 8.0)
                                    .gap(8.0)
                                    .cross_align(Align::Center)
                                    .bg(t.raised)
                                    .radius(8.0)
                                    .border(1.0, t.border)
                                    .transition(250.0)
                                    .enter(Enter::from(240.0, 0.0))
                                    .exit(Enter::from(240.0, 0.0))
                                    .on_click(Msg::Dismiss { id: *id }),
                                |ui| {
                                    ui.text(text, TextStyle::new(13.0).color(t.fg));
                                    ui.text(
                                        "click to dismiss",
                                        TextStyle::new(11.0).color(t.faint),
                                    );
                                },
                            );
                        }
                    },
                );
                // ANCHOR_END: enter_exit
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Toggle) => self.wide = !self.wide,
            Some(Msg::Notify) => {
                self.next_id += 1;
                self.toasts
                    .push((self.next_id, format!("Saved #{}", self.next_id)));
            }
            Some(Msg::Dismiss { id }) => self.toasts.retain(|(i, _)| *i != id),
            None => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Motion")
        .size(480.0, 360.0)
        .run(Motion::default())
}
