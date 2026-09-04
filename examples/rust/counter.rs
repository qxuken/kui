//! Minimal Elm-ish counter: `view(&mut self)` rebuilds the tree from state,
//! clicks arrive as data in `on_event`. Sound is data too: the buttons
//! declare a `click_sound`, the badge a `hover_sound`, and the hum is an
//! `audio` node the view keeps declaring while it is on.
//!
//! Run: cargo run --example counter

use kui::audio::{blip, wav_pcm16};
use kui::widgets;
use kui::{Align, App, AudioSpec, Color, NodeSpec, Sizing, SoundId, TextStyle, Ui, UiEvent, Value};

#[derive(Default)]
struct Counter {
    count: i64,
    hum: bool,
    /// Registered on the first frame (synthesized, so no asset files).
    sounds: Option<Sounds>,
}

#[derive(Clone, Copy)]
struct Sounds {
    click: SoundId,
    tick: SoundId,
    hum: SoundId,
}

/// A button with a click sound: the stock button spec plus one prop.
fn sound_button(ui: &mut Ui<'_>, label: &str, payload: Value, sound: SoundId) {
    ui.with_keyed(
        label,
        widgets::button_spec().on_click(payload).click_sound(sound),
        |ui| {
            ui.text(
                label,
                TextStyle::new(widgets::BUTTON_TEXT).color(Color::WHITE),
            )
        },
    );
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let sounds = *self.sounds.get_or_insert_with(|| {
            let core = ui.core();
            // A 200-sample period at 44.1kHz loops seamlessly.
            let hum: Vec<f32> = (0..2000)
                .map(|i| (i as f32 / 200.0 * std::f32::consts::TAU).sin() * 0.25)
                .collect();
            Sounds {
                click: core.add_sound(blip(44_100, 880.0, 60.0, 0.4)),
                tick: core.add_sound(blip(44_100, 1760.0, 25.0, 0.2)),
                hum: core.add_sound(wav_pcm16(44_100, &hum)),
            }
        });
        // The title is frame state like everything else; the runner diffs.
        ui.window_title(&format!("kui — counter ({})", self.count));
        ui.configure_root(NodeSpec::column().fill().center().gap(24.0));
        if self.hum {
            ui.audio_keyed("hum", AudioSpec::new(sounds.hum).looped().volume(0.3));
        }

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
                    sound_button(ui, "-1", Value::map([("kind", "dec".into())]), sounds.click);
                    sound_button(ui, "+1", Value::map([("kind", "inc".into())]), sounds.click);
                    sound_button(
                        ui,
                        if self.hum { "hum: on" } else { "hum: off" },
                        Value::map([("kind", "hum".into())]),
                        sounds.click,
                    );
                    // Floating tooltip: shown on hover (or until first click,
                    // so you can see it without a mouse).
                    let badge = ui.child_key("help");
                    ui.with_keyed(
                        "help",
                        NodeSpec::column()
                            .pad_xy(9.0, 4.0)
                            .bg(Color::rgb8(0x24, 0x27, 0x33))
                            .radius(10.0)
                            .hoverable()
                            .hover_sound(sounds.tick),
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
            Some("hum") => self.hum = !self.hum,
            _ => {}
        }
    }
}

fn main() {
    kui::run("kui — counter", Counter::default(), vec![]).unwrap();
}
