//! Step 4 — the stock controls. Builds on step 3 by replacing the
//! buttons with a settings card: a switch, a checkbox, a slider, a text
//! field and a select, each drawn from the model and each reporting
//! through a message. Chapter 5 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_04_controls

use kui_native::widgets;
use kui_native::widgets::InputMessages;
use kui_native::{Align, App, Event, Key, Message, NodeSpec, TextStyle, Ui, UiEvent};

const LANGUAGES: [&str; 3] = ["English", "Deutsch", "日本語"];

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    /// A toggle flips: the click is the message, nothing else.
    Notify,
    Sound,
    /// A slider proposes a value: the message is the one declared, and
    /// the value rides inside it, under `change`.
    Volume,
    /// The text field changed: its whole text rides under `change` too.
    Name,
}
// ANCHOR_END: message

// ANCHOR: model
struct Settings {
    notify: bool,
    sound: bool,
    volume: f32,
    /// The text field's text: the model's, declared back as its value.
    name: String,
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
            name: String::new(),
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
                    // The field reports each edit as `Msg::Name`, the
                    // model keeps the text, and the view declares it
                    // back as the field's value (`true`).
                    widgets::text_input_with(
                        ui,
                        "Display name",
                        &self.name,
                        true,
                        &InputMessages {
                            change: Some(Msg::Name.into()),
                            ..Default::default()
                        },
                    );
                    let name = if self.name.is_empty() {
                        "stranger"
                    } else {
                        &self.name
                    };
                    ui.text(
                        &format!("Hello, {name}"),
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
        // The message, and the event it came with, in one match.
        match ev.split::<Msg>() {
            (Some(Msg::Notify), _) => self.notify = !self.notify,
            (Some(Msg::Sound), _) => self.sound = !self.sound,
            // A slider's `change`: the value the core proposes.
            (Some(Msg::Volume), Event::SliderChange(c)) => self.volume = c.value as f32,
            // An editor's `change`: the whole text after the edit.
            (Some(Msg::Name), Event::EditChange(c)) => self.name = c.text,
            // The select declared no message: its choice arrives as a
            // `menu` event on its key, with the chosen label as `item`.
            (None, Event::Menu(choice)) if Some(ev.key) == self.language_key => {
                if let Some(i) = choice
                    .item
                    .as_str()
                    .and_then(|item| LANGUAGES.iter().position(|l| *l == item))
                {
                    self.language = i;
                }
            }
            _ => {}
        }
    }
    // ANCHOR_END: on_event
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Settings")
        .size(480.0, 420.0)
        .run(Settings::default())
}
