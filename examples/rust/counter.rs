//! Minimal Elm-ish counter: `view(&mut self)` rebuilds the tree from state,
//! clicks arrive as data in `on_event`. Sound is data too: the buttons
//! declare a `click_sound`, the badge a `hover_sound`, and the hum is an
//! `audio` node the view keeps declaring while it is on.
//!
//! Right-click anywhere for a context menu — the same shape the Node and C
//! counters use. The core opens nothing: a secondary press on a node that
//! declared `on_context_menu` emits `{kind="contextmenu", x, y}` and stops
//! there, and the view declares a `modal` float at those coordinates on the
//! next frame. `modal` is what makes it behave like a menu — Tab is scoped
//! to it, the pointer cannot reach the buttons behind it, and Escape or a
//! press outside comes back as `{kind="dismiss"}` for the app to act on.
//!
//! Run: cargo run --example counter

use kui::audio::{blip, wav_pcm16};
use kui::widgets;
use kui::{
    Align, App, AudioSpec, FloatConfig, NodeSpec, Sizing, SoundId, TextStyle, Ui, UiEvent, Value,
};

#[derive(Default)]
struct Counter {
    count: i64,
    hum: bool,
    /// Where the last right-click landed, while its menu is open. The
    /// position is app state like any other: the core reported it once and
    /// kept nothing.
    menu: Option<(f32, f32)>,
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
///
/// `name` is the node's key and `label` is what it says, and they are two
/// arguments because for one of these buttons they are two things: `hum`
/// reads `hum: off` and then `hum: on`. Keyed by its label it would be a
/// *different node* the frame after it is pressed — and everything the
/// core keeps per node is keyed too, so the focus would be left on a key
/// nothing declares any more and the ring would vanish under the press
/// that caused it. Hover and any tween would go the same way. The key is
/// what the button *is*; the label is a view of the state it toggles.
fn sound_button(ui: &mut Ui<'_>, name: &str, label: &str, payload: Value, sound: SoundId) {
    // `on_accent` rather than white: the label has to be readable on
    // whatever the button's background is, and under a light OS accent
    // that is black (`kui::Theme`).
    let fg = ui.theme().on_accent;
    ui.with_keyed(
        name,
        widgets::button_spec().on_click(payload).click_sound(sound),
        |ui| ui.text(label, TextStyle::new(widgets::BUTTON_TEXT).color(fg)),
    );
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Every colour below is a token, not a literal: the card, the
        // caption, the badge and the menu all come off the palette kui
        // derived from what the OS said (`kui::Theme`), so this window
        // follows a light appearance and an accent without one branch.
        let t = ui.theme();
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
        // The whole window answers a secondary press. Any node can: the
        // topmost one that declared `on_context_menu` is the one asked, so a
        // row inside could offer its own menu and win over this one.
        ui.configure_root(
            NodeSpec::column()
                .fill()
                .center()
                .gap(24.0)
                .on_context_menu(Value::map([("kind", "menu".into())])),
        );
        if self.hum {
            ui.audio_keyed("hum", AudioSpec::new(sounds.hum).looped().volume(0.3));
        }

        ui.with(
            NodeSpec::column()
                .pad(32.0)
                .gap(20.0)
                .bg(t.surface)
                .radius(12.0)
                .border(1.0, t.border)
                .cross_align(Align::Center),
            |ui| {
                ui.text("kui counter", TextStyle::new(14.0).color(t.muted));
                ui.text(&self.count.to_string(), TextStyle::new(56.0));
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    sound_button(
                        ui,
                        "dec",
                        "-1",
                        Value::map([("kind", "dec".into())]),
                        sounds.click,
                    );
                    sound_button(
                        ui,
                        "inc",
                        "+1",
                        Value::map([("kind", "inc".into())]),
                        sounds.click,
                    );
                    sound_button(
                        ui,
                        "hum",
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
                            .bg(t.raised)
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
                    TextStyle::new(13.0).color(t.faint),
                );
            },
        );
        kui::widgets::latency_hud(ui);

        // Declared after the HUD, not before it: floats stack in
        // declaration order, and the HUD is a float too — declared second it
        // would draw over the menu. One `modal` row is the whole difference
        // between this and a plain float.
        if let Some((x, y)) = self.menu {
            ui.with_keyed(
                "menu",
                NodeSpec::column()
                    .float(
                        FloatConfig::viewport()
                            .at(Align::Start, Align::Start)
                            .self_at(Align::Start, Align::Start)
                            .offset(x, y)
                            // Opened near the right or bottom edge, the menu
                            // would hang off the window; `fit` mirrors it
                            // back across the press instead.
                            .fit(),
                    )
                    .modal(Value::str("menu"))
                    .label("Actions")
                    .width(Sizing::Fixed(120.0))
                    .pad(4.0)
                    .gap(4.0)
                    .bg(t.raised)
                    .border(1.0, t.border_strong)
                    .radius(6.0),
                |ui| {
                    sound_button(
                        ui,
                        "add10",
                        "+10",
                        Value::map([("kind", "add10".into())]),
                        sounds.click,
                    );
                    sound_button(
                        ui,
                        "reset",
                        "reset",
                        Value::map([("kind", "reset".into())]),
                        sounds.click,
                    );
                },
            );
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("inc") => self.count += 1,
            Some("dec") => self.count -= 1,
            Some("hum") => self.hum = !self.hum,
            // A right-click: the core reports where it landed and opens
            // nothing. The next frame's view is what puts a menu there.
            Some("contextmenu") => {
                let at = |k| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
                self.menu = Some((at("x"), at("y")));
            }
            // Escape, or a press outside the menu. The core asks; the app
            // decides — this one just closes.
            Some("dismiss") => self.menu = None,
            // Choosing an item closes the menu, the way it does everywhere.
            Some("add10") => {
                self.count += 10;
                self.menu = None;
            }
            Some("reset") => {
                self.count = 0;
                self.menu = None;
            }
            _ => {}
        }
    }
}

fn main() {
    kui::run("kui — counter", Counter::default(), vec![]).unwrap();
}
