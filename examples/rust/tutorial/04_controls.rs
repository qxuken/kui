//! Step 4 — the stock controls. Builds on step 3 by replacing the
//! buttons with a settings card: a switch, a checkbox, a slider, a text
//! field and a select, each drawn from the model and each reporting
//! through a message. Chapter 5 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_04_controls

use kui_native::widgets;
use kui_native::{Align, App, Key, Message, NodeSpec, TextStyle, Ui, UiEvent};

const LANGUAGES: [&str; 3] = ["English", "Deutsch", "日本語"];

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    /// A toggle flips: the click is the message, nothing else.
    Notify,
    Sound,
    /// A slider proposes a value: the message is a tag, the value rides
    /// beside it on the event.
    Volume,
}
// ANCHOR_END: message

// ANCHOR: model
struct Settings {
    notify: bool,
    sound: bool,
    volume: f32,
    language: usize,
    /// The select's key, kept so `on_event` knows its choice by key.
    language_key: Option<Key>,
}
// ANCHOR_END: model

impl Default for Settings {
    fn default() -> Self {
        Self {
            notify: true,
            sound: false,
            volume: 40.0,
            language: 0,
            language_key: None,
        }
    }
}

impl App for Settings {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().center().bg(t.bg), |ui| {
            ui.with(
                NodeSpec::column()
                    .width(320.0)
                    .pad(20.0)
                    .gap(14.0)
                    .cross_align(Align::Start)
                    .bg(t.surface)
                    .radius(12.0)
                    .border(1.0, t.border),
                |ui| {
                    ui.text("Settings", TextStyle::new(18.0).color(t.fg));

                    // ANCHOR: toggles
                    // Each control is drawn from the model: `self.notify`
                    // says whether the switch is on. The control keeps no
                    // state of its own.
                    widgets::switch(ui, "Notifications", self.notify, Msg::Notify);
                    widgets::checkbox(ui, "Play a sound", self.sound, Msg::Sound);
                    // ANCHOR_END: toggles

                    // ANCHOR: slider
                    ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                        widgets::slider(ui, "Volume", self.volume, 0.0, 100.0, 5.0, Msg::Volume);
                        ui.text(
                            &format!("{:.0}", self.volume),
                            TextStyle::new(13.0).color(t.muted),
                        );
                    });
                    // ANCHOR_END: slider

                    // ANCHOR: field
                    // The editor owns its text between frames. The view
                    // reads it back by key.
                    let name = widgets::text_input(ui, "Display name", "");
                    let typed = ui.edit_text(name).unwrap_or_default();
                    ui.text(
                        &format!(
                            "Hello, {}",
                            if typed.is_empty() { "stranger" } else { &typed }
                        ),
                        TextStyle::new(13.0).color(t.muted),
                    );
                    // ANCHOR_END: field

                    // ANCHOR: select
                    // A select posts the chosen row on its own key.
                    self.language_key = Some(widgets::select(
                        ui,
                        "Language",
                        &LANGUAGES,
                        Some(self.language),
                    ));
                    // ANCHOR_END: select
                },
            );
        });
    }

    // ANCHOR: on_event
    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Notify) => self.notify = !self.notify,
            Some(Msg::Sound) => self.sound = !self.sound,
            Some(Msg::Volume) => {
                // A `change` event: the message was its tag, the value is
                // a field beside it.
                if let Some(v) = ev.payload.get_float("value") {
                    self.volume = v as f32;
                }
            }
            None => {
                // The select speaks by key, not by message: a `menu`
                // event with the chosen label as `item`.
                if Some(ev.key) == self.language_key
                    && let Some(item) = ev.payload.get_str("item")
                    && let Some(i) = LANGUAGES.iter().position(|l| *l == item)
                {
                    self.language = i;
                }
            }
        }
    }
    // ANCHOR_END: on_event
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Settings")
        .size(480.0, 420.0)
        .run(Settings::default())
}
