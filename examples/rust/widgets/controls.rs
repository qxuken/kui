//! The stock controls (`docs/adr/0034-stock-controls-over-the-roles.md`):
//! checkbox, radio group, switch and slider, each drawn from the state the
//! view declares and none holding any of its own.
//!
//! A settings page's worth of them: notifications on or off, a sound
//! checkbox beside a select-all box that goes mixed when only some of its
//! three rows are ticked, a theme chosen by radio (arrows move the choice),
//! and two sliders — a volume the core snaps to steps of 5, and a gain in
//! tenths — whose `change` events carry the value to store. Every change
//! is one line in the event log at the bottom.
//!
//! Run: cargo run -p kui-native --example controls [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{Align, App, Core, KeyMods, NodeSpec, Role, TextStyle, Ui, UiEvent, Value};

const THEMES: [&str; 3] = ["Light", "Dark", "System"];
const CHANNELS: [&str; 3] = ["Mail", "Calendar", "Chat"];

struct Controls {
    notify: bool,
    channels: [bool; 3],
    theme: usize,
    volume: f32,
    gain: f32,
    last: String,
}

impl Default for Controls {
    fn default() -> Self {
        Self {
            notify: true,
            channels: [true, false, true],
            theme: 2,
            volume: 40.0,
            gain: 0.5,
            last: "nothing yet — click, drag, or Tab and use the keys".into(),
        }
    }
}

fn tag(kind: &str) -> Value {
    Value::map([("kind", Value::str(kind))])
}

fn tagged(kind: &str, i: usize) -> Value {
    Value::map([("kind", Value::str(kind)), ("i", Value::Int(i as i64))])
}

impl App for Controls {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let m = ui.metrics();
        let heading = |ui: &mut Ui<'_>, s: &str| {
            ui.text(s, TextStyle::new(12.0).color(t.muted));
        };
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(12.0)
                .cross_align(Align::Start)
                .bg(t.bg),
            |ui| {
                heading(ui, "switch");
                widgets::switch(ui, "Notifications", self.notify, tag("notify"));

                heading(ui, "checkbox, with a select-all that can be mixed");
                let ticked = self.channels.iter().filter(|c| **c).count();
                widgets::toggle_with(
                    ui,
                    widgets::Toggle::Checkbox,
                    "All channels",
                    "All channels",
                    widgets::toggle_spec(&m)
                        .checked(ticked == CHANNELS.len())
                        .mixed(ticked > 0 && ticked < CHANNELS.len())
                        .on_click(tag("all"))
                        .disabled(!self.notify),
                    None,
                );
                ui.with(
                    NodeSpec::column()
                        .padding(kui_native::Edges {
                            l: 24.0,
                            r: 0.0,
                            t: 0.0,
                            b: 0.0,
                        })
                        .gap(8.0),
                    |ui| {
                        for (i, name) in CHANNELS.iter().enumerate() {
                            widgets::toggle_with(
                                ui,
                                widgets::Toggle::Checkbox,
                                name,
                                name,
                                widgets::toggle_spec(&m)
                                    .checked(self.channels[i])
                                    .on_click(tagged("channel", i))
                                    .disabled(!self.notify),
                                None,
                            );
                        }
                    },
                );

                heading(ui, "radio group — Tab to it, then the arrows");
                widgets::radio_group(ui, "Theme", &THEMES, Some(self.theme), |i| {
                    tagged("theme", i)
                });

                heading(
                    ui,
                    "slider — press, drag, or the arrows, Page keys, Home and End",
                );
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    widgets::slider(ui, "Volume", self.volume, 0.0, 100.0, 5.0, tag("volume"));
                    ui.text(
                        &format!("{:.0}", self.volume),
                        TextStyle::new(13.0).color(t.fg),
                    );
                });
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    widgets::slider_with(
                        ui,
                        "Gain",
                        widgets::slider_spec(&m)
                            .width(120.0)
                            .value_now(self.gain)
                            .value_min(0.0)
                            .value_max(1.0)
                            .value_step(0.1)
                            .value_text(format!("gain {:.1}", self.gain))
                            .on_change(tag("gain")),
                        Some("Steps of a tenth, proposed as the decimal"),
                    );
                    ui.text(
                        &format!("{:.1}", self.gain),
                        TextStyle::new(13.0).color(t.fg),
                    );
                });

                ui.leaf(NodeSpec::column().height(8.0));
                ui.text(&self.last, TextStyle::new(12.0).color(t.faint).mono());
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        let kind = ev.kind();
        let tag = ev
            .payload
            .get("tag")
            .and_then(|t| t.get("kind"))
            .and_then(Value::as_str);
        let i = |v: &Value| v.get_int("i").unwrap_or(0) as usize;
        match (kind, tag) {
            // A slider's proposal: store it, and the next frame draws it.
            (Some("change"), Some(which)) => {
                let v = ev.payload.get_float("value").unwrap_or(0.0) as f32;
                match which {
                    "volume" => self.volume = v,
                    "gain" => self.gain = v,
                    _ => {}
                }
            }
            (Some("notify"), _) => self.notify = !self.notify,
            (Some("all"), _) => {
                let all = !self.channels.iter().all(|c| *c);
                self.channels = [all; 3];
            }
            (Some("channel"), _) => {
                let k = i(&ev.payload);
                self.channels[k] = !self.channels[k];
            }
            (Some("theme"), _) => self.theme = i(&ev.payload),
            _ => return,
        }
        self.last = format!("last event: {:?}", ev.payload);
    }
}

impl Example for Controls {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(520.0, 560.0)
    }

    /// A toggle's press flips the model through its payload; the select-all
    /// reads mixed on a partial selection and ticks every row; the radio
    /// group's arrows move the choice; a slider's press, keys and ends
    /// propose values snapped to its step, which the app stores.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 520.0, 560.0);
        d.frame(self);
        let node = |d: &mut Drive<'_>, name: &str, role: Role| {
            d.core
                .access_tree()
                .nodes
                .iter()
                .find(|n| n.role == role && n.name.as_deref() == Some(name))
                .cloned()
                .ok_or(format!("no {name}"))
        };

        let all = node(&mut d, "All channels", Role::Checkbox)?;
        d.check(all.mixed, "two of three channels: the select-all is mixed")?;
        d.click_key(self, all.key);
        d.frame(self);
        d.check(self.channels == [true; 3], "and ticking it ticks every row")?;
        let all = node(&mut d, "All channels", Role::Checkbox)?;
        d.check(all.checked == Some(true), "then it reads checked")?;

        let notify = node(&mut d, "Notifications", Role::Switch)?;
        d.click_key(self, notify.key);
        d.frame(self);
        d.check(!self.notify, "the switch turns notifications off")?;
        let mail = node(&mut d, "Mail", Role::Checkbox)?;
        d.check(mail.disabled, "and the channel boxes go inert with them")?;

        let light = node(&mut d, "Light", Role::Radio)?;
        d.focus(self, light.key);
        d.key(self, "down", KeyMods::default());
        d.frame(self);
        d.check(self.theme == 1, "Down from Light chose Dark")?;

        let vol = node(&mut d, "Volume", Role::Slider)?;
        let r = vol.rect;
        let b = widgets::control_box(&kui_native::Metrics::default());
        // Three quarters along the track, which is the node less half the
        // thumb at each end.
        let x = r.x + b / 2.0 + (r.w - b) * 0.73;
        d.click(self, x, r.y + r.h / 2.0);
        d.frame(self);
        d.check(
            self.volume == 75.0,
            "a press three quarters along is 75, snapped to 5",
        )?;
        d.focus(self, vol.key);
        d.key(self, "right", KeyMods::default());
        d.frame(self);
        d.check(self.volume == 80.0, "Right is one step")?;
        d.key(self, "pagedown", KeyMods::default());
        d.frame(self);
        d.check(self.volume == 30.0, "PageDown is ten")?;
        d.key(self, "end", KeyMods::default());
        d.frame(self);
        d.check(self.volume == 100.0, "End is the top")?;

        let gain = node(&mut d, "Gain", Role::Slider)?;
        d.check(
            gain.value.as_deref() == Some("gain 0.5"),
            "the gain reads as its text",
        )?;
        d.focus(self, gain.key);
        d.key(self, "left", KeyMods::default());
        d.frame(self);
        d.check(self.gain == 0.4, "a tenth down is 0.4, not 0.39999998")?;
        let warned = d.core.take_warnings();
        d.check(warned.is_empty(), "nothing warned")?;
        Ok(())
    }
}

kui_devtools::main!(Controls::default());
