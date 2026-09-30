//! A spring, tuned by eye: the two numbers a person means when they say a
//! motion feels right, and nothing from the physics lesson.
//!
//! - `transition(ms)`: how long the spring takes to get there;
//! - `bounce`: how far it overshoots — 0 glides in, 0.5 visibly bounces,
//!   0.9 rings a while (the most there is: a spring at 1 never settles).
//!
//! Two sliders set them, and four buttons set the bounce each spring
//! easing is named for — `smooth` 0, `snappy` 0.15, `spring` 0.25,
//! `bouncy` 0.5. Click the stage and a bar and a racer spring across it
//! with those numbers; click again mid-flight and they turn around with
//! the momentum they had, which is what a spring is for.
//!
//! Nothing here computes a curve: the view declares a width and an anchor
//! and the spring's two numbers, and the core is between the last frame
//! and this one.
//!
//! Run: cargo run -p kui-native --example spring [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{
    Align, App, Core, Easing, FloatConfig, KeyMods, NodeSpec, Role, TextStyle, Ui, UiEvent, Value,
};

/// The spring easings, each a named bounce.
const PRESETS: [(&str, Easing); 4] = [
    ("smooth", Easing::Smooth),
    ("snappy", Easing::Snappy),
    ("spring", Easing::Spring),
    ("bouncy", Easing::Bouncy),
];

/// The bar's two widths, and the lane both springs cross.
const NARROW: f32 = 80.0;
const WIDE: f32 = 360.0;
const LANE: f32 = 420.0;

struct Spring {
    duration_ms: f32,
    bounce: f32,
    far: bool,
}

impl Default for Spring {
    fn default() -> Self {
        Self {
            duration_ms: 500.0,
            bounce: Easing::Spring.bounce().unwrap_or(0.0),
            far: false,
        }
    }
}

fn tag(kind: &str) -> Value {
    Value::map([("kind", Value::str(kind))])
}

impl App for Spring {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let m = ui.metrics();
        let heading = |ui: &mut Ui<'_>, s: &str| {
            ui.text(s, TextStyle::new(12.0).color(t.muted));
        };
        // Every spring on the page is this one: a duration and a bounce.
        let spring = |spec: NodeSpec| {
            spec.transition(self.duration_ms)
                .easing(Easing::Spring)
                .bounce(self.bounce)
        };
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(12.0)
                .cross_align(Align::Start)
                .bg(t.bg),
            |ui| {
                heading(ui, "how long it takes to get there");
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    widgets::slider_with(
                        ui,
                        "Duration",
                        widgets::slider_spec(&m)
                            .width(240.0)
                            .value_now(self.duration_ms)
                            .value_min(100.0)
                            .value_max(1500.0)
                            .value_step(50.0)
                            .value_text(format!("{:.0} ms", self.duration_ms))
                            .on_change(tag("duration")),
                        None,
                    );
                    ui.text(
                        &format!("{:.0} ms", self.duration_ms),
                        TextStyle::new(13.0).color(t.fg),
                    );
                });

                heading(ui, "how far it overshoots");
                ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                    widgets::slider_with(
                        ui,
                        "Bounce",
                        widgets::slider_spec(&m)
                            .width(240.0)
                            .value_now(self.bounce)
                            .value_min(0.0)
                            .value_max(kui_native::MAX_BOUNCE)
                            .value_step(0.05)
                            .value_text(format!("bounce {:.2}", self.bounce))
                            .on_change(tag("bounce")),
                        None,
                    );
                    ui.text(
                        &format!("{:.2}", self.bounce),
                        TextStyle::new(13.0).color(t.fg),
                    );
                });
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    for (name, _) in PRESETS {
                        widgets::button(ui, name, tag(name));
                    }
                });

                heading(
                    ui,
                    "click the stage — and again mid-flight: it turns around with its momentum",
                );
                ui.with_keyed(
                    "stage",
                    NodeSpec::column()
                        .width(LANE + 16.0)
                        .pad(8.0)
                        .gap(12.0)
                        .bg(t.sunken)
                        .radius(8.0)
                        .on_click(tag("go"))
                        .label("Stage: spring the bar and the racer across"),
                    |ui| {
                        // The width is a layout input, so it springs: past
                        // its target and back, by as much as the bounce.
                        ui.leaf_keyed(
                            "bar",
                            spring(
                                NodeSpec::row()
                                    .width(if self.far { WIDE } else { NARROW })
                                    .height(20.0)
                                    .radius(4.0)
                                    .bg(t.accent),
                            ),
                        );
                        // A float between two anchors, its position eased
                        // with `slide` — the same spring.
                        ui.with(NodeSpec::row().width(LANE).height(24.0), |ui| {
                            let side = if self.far { Align::End } else { Align::Start };
                            ui.leaf_keyed(
                                "racer",
                                spring(
                                    NodeSpec::row()
                                        .float(FloatConfig::parent().inside(side, Align::Center))
                                        .size(24.0, 24.0)
                                        .radius(12.0)
                                        .bg(t.success),
                                )
                                .slide(),
                            );
                        });
                    },
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        let tag = ev
            .payload
            .get("tag")
            .and_then(|t| t.get("kind"))
            .and_then(Value::as_str);
        match (ev.kind(), tag) {
            // A slider's proposal: store it, and the next frame draws it.
            (Some("change"), Some("duration")) => {
                self.duration_ms = ev.payload.get_float("value").unwrap_or(500.0) as f32;
            }
            (Some("change"), Some("bounce")) => {
                self.bounce = ev.payload.get_float("value").unwrap_or(0.0) as f32;
            }
            (Some("go"), _) => self.far = !self.far,
            (Some(kind), _) => {
                if let Some((_, e)) = PRESETS.iter().find(|(name, _)| *name == kind) {
                    self.bounce = e.bounce().unwrap_or(0.0);
                }
            }
            _ => {}
        }
    }
}

impl Example for Spring {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(520.0, 420.0)
    }

    /// The bounce is what the eye reads: `smooth` carries the bar to its
    /// width and no further, `bouncy` carries it past and back, and a
    /// retarget mid-flight keeps moving the way it was going before it
    /// turns. The sliders store what they propose.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 520.0, 420.0);
        d.frame(self);
        d.frame(self);
        let press = |d: &mut Drive<'_>, app: &mut Spring, label: &str| -> Result<(), String> {
            let key = d.key_of(label).ok_or(format!("no {label}"))?;
            d.click_key(app, key);
            d.frame(app);
            Ok(())
        };
        let bar = d.key_of("bar").ok_or("no bar")?;
        let width = |d: &Drive<'_>| d.rect_of(bar).map_or(0.0, |r| r.w);
        // The widest the bar gets over `secs` of 60 Hz frames.
        let widest = |d: &mut Drive<'_>, app: &mut Spring, secs: f64| {
            let mut max = 0.0f32;
            for _ in 0..(secs * 60.0) as usize {
                d.advance(1.0 / 60.0);
                d.frame(app);
                max = max.max(width(d));
            }
            max
        };

        press(&mut d, self, "smooth")?;
        d.check(self.bounce == 0.0, "smooth is no bounce")?;
        press(&mut d, self, "stage")?;
        let smooth = widest(&mut d, self, 2.0);
        d.check(
            smooth <= WIDE + 0.5 && smooth > WIDE - 0.5,
            &format!("smooth reaches {WIDE} and stops there: {smooth}"),
        )?;
        d.check(!d.core.animating(), "and comes to rest")?;

        press(&mut d, self, "bouncy")?;
        press(&mut d, self, "stage")?;
        widest(&mut d, self, 2.0);
        press(&mut d, self, "stage")?;
        let bouncy = widest(&mut d, self, 2.0);
        d.check(
            bouncy > WIDE + 20.0,
            &format!("bouncy carries it past {WIDE}: {bouncy}"),
        )?;

        // Mid-flight, turned around: the bar keeps growing for a moment.
        press(&mut d, self, "stage")?;
        widest(&mut d, self, 2.0);
        press(&mut d, self, "stage")?;
        d.advance(0.1);
        d.frame(self);
        let going = width(&d);
        press(&mut d, self, "stage")?;
        d.advance(1.0 / 60.0);
        d.frame(self);
        d.check(
            width(&d) > going,
            "a retarget keeps the momentum it had before turning",
        )?;

        let node = |d: &mut Drive<'_>, name: &str| {
            d.core
                .access_tree()
                .nodes
                .iter()
                .find(|n| n.role == Role::Slider && n.name.as_deref() == Some(name))
                .map(|n| n.key)
                .ok_or(format!("no {name} slider"))
        };
        let b = node(&mut d, "Bounce")?;
        d.focus(self, b);
        d.key(self, "end", KeyMods::default());
        d.frame(self);
        d.check(
            self.bounce == kui_native::MAX_BOUNCE,
            "the bounce slider tops out at the most a spring takes",
        )?;
        let dur = node(&mut d, "Duration")?;
        d.focus(self, dur);
        d.key(self, "right", KeyMods::default());
        d.frame(self);
        d.check(self.duration_ms == 550.0, "Right is one 50 ms step")?;
        let warned = d.warnings();
        d.check(warned.is_empty(), &format!("nothing warned: {warned:?}"))
    }
}

kui_devtools::main!(Spring::default());
