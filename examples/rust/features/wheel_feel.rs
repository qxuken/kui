//! The wheel feel, tried by hand: how far a mouse wheel's notch goes and
//! how quickly its glide lands (ADR 0050, `WheelFeel`).
//!
//! - a select picks a preset, slow to fast: `gentle`, `standard` (the
//!   default), `brisk` (a touch faster), `swift`;
//! - three sliders set the numbers themselves — the line in px, the
//!   glide's response in seconds and its kick — and the select reads
//!   `custom` once they match no preset;
//! - a switch turns the glide off, where a notch jumps its line.
//!
//! Turn the wheel over the list on the right to feel it. The line under
//! the sliders is the same feel as `KUI_WHEEL`, to run any app with it.
//!
//! The view holds no copy of the feel: it reads the core's, and the
//! handler writes the core's (`App::on_event_with`), so `KUI_WHEEL=brisk`
//! in the environment opens the example on `brisk`.
//!
//! Run: cargo run -p kui-native --example wheel_feel [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{
    Align, App, Core, Event, InputEvent, KeyMods, NodeSpec, Role, TextStyle, Ui, UiEvent, Value,
    Vec2, WheelFeel,
};

/// The list's rows: enough to spin through for a while.
const ROWS: usize = 400;
const ROW_H: f32 = 28.0;

#[derive(Default)]
struct WheelFeelDemo;

/// The `{kind}` message every control here declares.
fn message(kind: &str) -> Value {
    Value::map([("kind", Value::str(kind))])
}

/// The preset `feel` is, by its index in [`WheelFeel::PRESETS`]; `None`
/// for numbers no preset has.
fn preset_of(feel: WheelFeel) -> Option<usize> {
    WheelFeel::PRESETS.iter().position(|(_, f)| *f == feel)
}

impl App for WheelFeelDemo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let m = ui.metrics();
        let feel = ui.core().wheel_feel();
        let glides = ui.core().smooth_wheel();
        let heading = |ui: &mut Ui<'_>, s: &str| {
            ui.text(s, TextStyle::new(12.0).color(t.muted));
        };
        // A labelled slider row: the name, the slider, the number.
        let slider = |ui: &mut Ui<'_>,
                      name: &str,
                      kind: &str,
                      now: f32,
                      range: (f32, f32, f32),
                      shown: String| {
            ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                ui.text_in(
                    NodeSpec::row().width(72.0),
                    name,
                    TextStyle::new(13.0).color(t.fg),
                );
                widgets::slider_with(
                    ui,
                    name,
                    widgets::slider_spec(&m)
                        .width(200.0)
                        .value_now(now)
                        .value_min(range.0)
                        .value_max(range.1)
                        .value_step(range.2)
                        .value_text(shown.clone())
                        .on_change(message(kind)),
                    None,
                );
                ui.text(&shown, TextStyle::new(13.0).color(t.fg).mono());
            });
        };
        ui.with(NodeSpec::row().fill().pad(24.0).gap(24.0).bg(t.bg), |ui| {
            // The settings.
            ui.with(
                NodeSpec::column()
                    .width(400.0)
                    .grow_height()
                    .gap(12.0)
                    .cross_align(Align::Start),
                |ui| {
                    heading(ui, "a preset, slow to fast");
                    ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                        ui.text_in(
                            NodeSpec::row().width(72.0),
                            "Preset",
                            TextStyle::new(13.0).color(t.fg),
                        );
                        let names: Vec<&str> = WheelFeel::PRESETS.iter().map(|(n, _)| *n).collect();
                        widgets::select(ui, "preset", &names, preset_of(feel));
                        if preset_of(feel).is_none() {
                            ui.text("custom", TextStyle::new(13.0).color(t.muted));
                        }
                    });

                    heading(ui, "or the numbers");
                    slider(
                        ui,
                        "Line",
                        "line",
                        feel.line_px,
                        (10.0, 120.0, 2.0),
                        format!("{:.0} px", feel.line_px),
                    );
                    slider(
                        ui,
                        "Response",
                        "response",
                        feel.response,
                        (0.10, 0.60, 0.01),
                        format!("{:.2} s", feel.response),
                    );
                    slider(
                        ui,
                        "Kick",
                        "kick",
                        feel.kick,
                        (0.0, 1.0, 0.05),
                        format!("{:.2}", feel.kick),
                    );

                    heading(ui, "whether a notch glides at all");
                    widgets::switch(ui, "Glide", glides, message("glide"));

                    ui.leaf(NodeSpec::column().height(8.0));
                    heading(ui, "the same feel for any app");
                    ui.text(
                        &format!("KUI_WHEEL={},{},{}", feel.line_px, feel.response, feel.kick),
                        TextStyle::new(12.0).color(t.fg).mono(),
                    );
                    heading(
                        ui,
                        "a line is how far one notch goes; the response is how long \
                             its glide takes to land; the kick is how hard each notch \
                             starts — more lands sooner, less runs notches together",
                    );
                },
            );

            // What to try it on.
            ui.with(
                NodeSpec::column().grow_width().grow_height().gap(8.0),
                |ui| {
                    heading(ui, "turn the wheel here");
                    ui.with_keyed(
                        "list",
                        NodeSpec::column()
                            .grow_width()
                            .grow_height()
                            .scroll_y()
                            .bg(t.sunken)
                            .radius(8.0)
                            .label("Rows to scroll"),
                        |ui| {
                            for i in 0..ROWS {
                                let bg = if i % 2 == 0 { t.sunken } else { t.bg };
                                ui.with(
                                    NodeSpec::row()
                                        .grow_width()
                                        .height(ROW_H)
                                        .pad_xy(12.0, 0.0)
                                        .cross_align(Align::Center)
                                        .bg(bg),
                                    |ui| {
                                        ui.text(
                                            &format!("row {i}"),
                                            TextStyle::new(13.0).color(t.fg),
                                        );
                                    },
                                );
                            }
                        },
                    );
                },
            );
        });
    }

    fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
        let mut feel = core.wheel_feel();
        let (msg, event) = ev.split::<Value>();
        match (msg.as_ref().and_then(|m| m.get_str("kind")), event) {
            // The select's choice: the preset's name, on the field's key.
            (_, Event::Menu(choice)) => {
                if let Some(f) = choice.item.as_str().and_then(WheelFeel::preset) {
                    feel = f;
                }
            }
            (Some("line"), Event::SliderChange(c)) => feel.line_px = c.value as f32,
            (Some("response"), Event::SliderChange(c)) => feel.response = c.value as f32,
            (Some("kick"), Event::SliderChange(c)) => feel.kick = c.value as f32,
            (Some("glide"), _) => {
                let on = core.smooth_wheel();
                core.set_smooth_wheel(!on);
                return;
            }
            _ => return,
        }
        core.set_wheel_feel(feel);
    }
}

impl Example for WheelFeelDemo {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(820.0, 520.0)
    }

    /// The drawn menu, so the select's rows are in the frame to click.
    fn native_menus(&self) -> Option<bool> {
        Some(false)
    }

    /// A notch glides its line and lands there; a preset chosen in the
    /// select is the core's feel and the next notch goes its line; a
    /// slider's step makes the feel custom; the switch off jumps.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 820.0, 520.0);
        d.frame(self);
        d.frame(self);
        d.check(
            d.core.wheel_feel() == WheelFeel::STANDARD,
            "the standard feel to start",
        )?;
        let list = d.key_of("list").ok_or("no list")?;
        let over = d.rect_of(list).ok_or("the list has no rect")?.center();
        d.move_to(self, over.x, over.y);
        // One notch down, then frames until the glide lands.
        let notch = |d: &mut Drive<'_>, app: &mut WheelFeelDemo| -> f32 {
            d.input(
                app,
                InputEvent::Wheel {
                    lines: Vec2::new(0.0, -1.0),
                    begins: true,
                },
            );
            for _ in 0..60 {
                d.advance(1.0 / 60.0);
                d.frame(app);
                if !d.core.animating() {
                    break;
                }
            }
            d.core.scroll_geometry(list).map_or(-1.0, |g| g.offset.y)
        };
        let y = notch(&mut d, self);
        d.check(
            y == 40.0,
            &format!("a standard notch lands 40 px down: {y}"),
        )?;

        // The select: open it, choose `swift`.
        let field = d.key_of("preset").ok_or("no preset select")?;
        d.click_key(self, field);
        d.frame(self);
        let row = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::MenuItem && n.name.as_deref() == Some("swift"))
            .map(|n| n.rect)
            .ok_or("the preset rows are not drawn")?;
        d.click(self, row.x + row.w / 2.0, row.y + row.h / 2.0);
        d.frame(self);
        d.check(
            d.core.wheel_feel() == WheelFeel::SWIFT,
            "the select's choice is the core's feel",
        )?;
        d.move_to(self, over.x, over.y);
        let y = notch(&mut d, self);
        d.check(y == 104.0, &format!("a swift notch goes 64 px more: {y}"))?;

        // A slider's step: the line, two px further; no preset any more.
        let line = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == Role::Slider && n.name.as_deref() == Some("Line"))
            .map(|n| n.key)
            .ok_or("no Line slider")?;
        d.focus(self, line);
        d.key(self, "right", KeyMods::default());
        d.frame(self);
        d.check(
            d.core.wheel_feel().line_px == 66.0,
            "Right is one 2 px step on the line",
        )?;
        d.check(
            preset_of(d.core.wheel_feel()).is_none(),
            "and the feel is no preset now",
        )?;

        // The switch off: the next notch is there on the next frame.
        let glide = d.key_of("Glide").ok_or("no Glide switch")?;
        d.click_key(self, glide);
        d.frame(self);
        d.check(!d.core.smooth_wheel(), "the switch turns the glide off")?;
        d.move_to(self, over.x, over.y);
        d.input(
            self,
            InputEvent::Wheel {
                lines: Vec2::new(0.0, -1.0),
                begins: true,
            },
        );
        d.advance(1.0 / 60.0);
        d.frame(self);
        let y = d.core.scroll_geometry(list).map_or(-1.0, |g| g.offset.y);
        d.check(y == 170.0, &format!("a jump of the line, at once: {y}"))?;
        let warned = d.warnings();
        d.check(warned.is_empty(), &format!("nothing warned: {warned:?}"))
    }
}

kui_devtools::main!(WheelFeelDemo);
