//! Sound as data. Three ways a view asks for one, and none of them is a
//! call: a button declares a `click_sound` and the core plays it on the
//! click; a badge declares a `hover_sound` for the pointer's arrival; and a
//! loop is an `audio` node the view keeps declaring while it is on and
//! stops declaring to stop — the same way a toast exists while the model
//! holds it. The sounds are synthesized here (`kui_native::audio::blip`,
//! `wav_pcm16`) and registered once as resources, since a resource is
//! long-lived core state and not per-frame data.
//!
//! The dock's `audio` row is the driver's side of the same story: the
//! device opens off-thread the first time a session holds a sound, stays
//! open while anything is live, and is let go after a while idle — an open
//! output stream is a real-time thread whether or not anything plays, so
//! `open · 0 live` ten seconds after the last click would be a bug
//! (`env.audio`, ADR 0021).
//!
//! Run: cargo run -p kui-native --example audio [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::audio::{blip, wav_pcm16};
use kui_native::widgets;
use kui_native::{Align, App, AudioSpec, Core, NodeSpec, SoundId, TextStyle, Ui, UiEvent, Value};

#[derive(Default)]
struct Audio {
    clicks: u32,
    hovers: u32,
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
    let spec = widgets::button_spec(&ui.theme(), &ui.metrics())
        .on_click(payload)
        .click_sound(sound);
    // Readable on whatever this button's background *is*, which is the
    // rule `widgets::button_with` applies to the stock one.
    let fg = widgets::readable_on(spec.style.bg);
    ui.with_keyed(name, spec, |ui| {
        ui.text(label, TextStyle::new(widgets::BUTTON_TEXT).color(fg))
    });
}

impl App for Audio {
    fn view(&mut self, ui: &mut Ui<'_>) {
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
        ui.with(NodeSpec::column().fill().center().gap(24.0), |ui| {
            // The loop: declared while on, and that is the whole of
            // "playing". Stop declaring it and the core stops it.
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
                    ui.text("sound is data", TextStyle::new(14.0).color(t.muted));
                    ui.text(
                        &format!("{} clicks · {} hovers", self.clicks, self.hovers),
                        TextStyle::new(24.0),
                    );
                    ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                        sound_button(
                            ui,
                            "blip",
                            "blip",
                            Value::map([("kind", "click".into())]),
                            sounds.click,
                        );
                        sound_button(
                            ui,
                            "hum",
                            if self.hum { "hum: on" } else { "hum: off" },
                            Value::map([("kind", "hum".into())]),
                            sounds.click,
                        );
                        // The hover sound: a `hoverable` badge with a
                        // `hover_sound` — the tick plays when the pointer
                        // arrives, and `on_hover` reports the same arrival
                        // as data so the count above can follow it.
                        ui.with_keyed(
                            "badge",
                            NodeSpec::column()
                                .pad_xy(12.0, 6.0)
                                .bg(t.raised)
                                .radius(10.0)
                                .hoverable()
                                .hover_sound(sounds.tick)
                                .on_hover(Value::map([("kind", "hover".into())])),
                            |ui| {
                                ui.text("hover me", TextStyle::new(13.0));
                            },
                        );
                    });
                },
            );
            ui.text(
                "click_sound · hover_sound · an audio node declared while on",
                TextStyle::new(12.0).color(t.faint),
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("click") => self.clicks += 1,
            Some("hum") => self.hum = !self.hum,
            Some("hover") if ev.payload.get("phase").and_then(Value::as_str) == Some("enter") => {
                self.hovers += 1
            }
            _ => {}
        }
    }
}

impl Example for Audio {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(520.0, 360.0)
    }

    /// A headless core has no device, so what this checks is the data:
    /// the commands a click and a loop queue for a driver to apply.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 520.0, 360.0);
        d.frame(self);
        let blip = d.key_of("blip").ok_or("no blip button")?;
        let hum = d.key_of("hum").ok_or("no hum button")?;
        let _ = d.core.take_audio_commands();
        d.click_key(self, blip);
        let cmds = d.core.take_audio_commands();
        d.check(self.clicks == 1, "the click counts")?;
        d.check(!cmds.is_empty(), "and queues a play for the driver")?;

        d.click_key(self, hum);
        d.frame(self);
        let cmds = d.core.take_audio_commands();
        d.check(
            self.hum && !cmds.is_empty(),
            "declaring the loop queues its play",
        )?;
        d.click_key(self, hum);
        d.frame(self);
        let cmds = d.core.take_audio_commands();
        d.check(
            !self.hum && !cmds.is_empty(),
            "and undeclaring it queues the stop",
        )?;

        // The hover: the pointer arriving over the badge is `on_hover`'s
        // `enter`, and the tick is queued beside it.
        let badge = d.key_of("badge").ok_or("no badge")?;
        d.hover(self, badge);
        let cmds = d.core.take_audio_commands();
        d.check(
            self.hovers == 1,
            "the pointer's arrival is reported as data",
        )?;
        d.check(!cmds.is_empty(), "and the hover sound is queued")
    }
}

kui_devtools::main!(Audio::default());
