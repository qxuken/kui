//! Motion as data: a node declares what it eases and how, and the core
//! tweens whatever changes between frames. Six things, each one prop:
//!
//! - `transition(ms)`: a value that changes eases to its new one over that
//!   long — the bar's width follows the buttons;
//! - `easing`: the curve it takes, one racer per `Easing` — the two
//!   springs overshoot, `bouncy` more — racing on a click;
//! - `slide`: a float whose *position* eases too, so a card jumps between
//!   two anchors along a path rather than appearing at the other;
//! - `keyframes`: a cycle of stops the node walks by itself — width, colour,
//!   radius, opacity — with `repeat` saying which way (normal, reverse,
//!   alternate) and `delay` holding siblings out of phase, which is what a
//!   chase light is;
//!
//! Nothing here calls an animation; each frame declares the value it wants
//! and the core is between the last frame and this one.
//!
//! Run: cargo run -p kui-native --example transition [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{
    Align, App, Core, Easing, FloatConfig, Keyframe, NodeSpec, Repeat, Sizing, TextStyle, Ui,
    UiEvent, Value,
};

const EASINGS: [(&str, Easing); 6] = [
    ("linear", Easing::Linear),
    ("ease-out", Easing::EaseOut),
    ("ease-in", Easing::EaseIn),
    ("ease-in-out", Easing::EaseInOut),
    ("spring", Easing::Spring),
    ("bouncy", Easing::Bouncy),
];

#[derive(Default)]
struct Motion {
    /// The bar's width, as a fraction: what the buttons set and the
    /// transition follows.
    level: f32,
    /// Whether the racers are at the far end.
    far: bool,
    /// Which anchor the sliding card sits at.
    right: bool,
}

impl App for Motion {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(18.0)
                .cross_align(Align::Start)
                .scroll_y(),
            |ui| {
                // transition: the width follows the level, eased.
                ui.text("transition · the bar eases to whatever the buttons set", TextStyle::new(12.0).color(t.muted));
                ui.with(NodeSpec::row().gap(8.0).cross_align(Align::Center), |ui| {
                    for (label, level) in [("0%", 0.0), ("40%", 0.4), ("100%", 1.0)] {
                        kui_native::widgets::button(
                            ui,
                            label,
                            Value::map([("kind", Value::str("level")), ("to", Value::Float(level))]),
                        );
                    }
                });
                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Fixed(420.0))
                        .height(Sizing::Fixed(14.0))
                        .bg(t.sunken)
                        .radius(7.0),
                    |ui| {
                        ui.with_keyed(
                            "bar",
                            NodeSpec::row()
                                .width(Sizing::Percent(self.level))
                                .height(Sizing::Grow(1.0))
                                .bg(t.accent)
                                .radius(7.0)
                                .transition(600.0),
                            |_| {},
                        );
                    },
                );

                // easing: the same travel on five curves.
                ui.text("easing · click a lane and the six race on their own curves", TextStyle::new(12.0).color(t.muted));
                ui.with_keyed(
                    "lanes",
                    NodeSpec::column()
                        .width(Sizing::Fixed(420.0))
                        .gap(6.0)
                        .pad(8.0)
                        .bg(t.surface)
                        .radius(8.0)
                        .border(1.0, t.border)
                        .on_click(Value::str("race")),
                    |ui| {
                        for (name, easing) in EASINGS {
                            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).gap(8.0).cross_align(Align::Center), |ui| {
                                ui.with(NodeSpec::row().width(Sizing::Fixed(80.0)), |ui| {
                                    ui.text(name, TextStyle::new(11.0).color(t.muted));
                                });
                                ui.with(
                                    NodeSpec::row().width(Sizing::Grow(1.0)).height(Sizing::Fixed(16.0)),
                                    |ui| {
                                        // The racer floats inside its lane; `slide`
                                        // is what makes its *position* ease.
                                        ui.with_keyed(
                                            name,
                                            NodeSpec::row()
                                                .float(
                                                    FloatConfig::parent()
                                                        .at(if self.far { Align::End } else { Align::Start }, Align::Center)
                                                        .self_at(if self.far { Align::End } else { Align::Start }, Align::Center),
                                                )
                                                .width(Sizing::Fixed(16.0))
                                                .height(Sizing::Fixed(16.0))
                                                .radius(8.0)
                                                .bg(t.accent)
                                                .transition(900.0)
                                                .easing(easing)
                                                .slide(),
                                            |_| {},
                                        );
                                    },
                                );
                            });
                        }
                    },
                );

                // slide: a float that eases between two anchors.
                ui.text("slide · the card eases between its two anchors instead of appearing at the other", TextStyle::new(12.0).color(t.muted));
                ui.with_keyed(
                    "stage",
                    NodeSpec::row()
                        .width(Sizing::Fixed(420.0))
                        .height(Sizing::Fixed(64.0))
                        .bg(t.sunken)
                        .radius(8.0)
                        .on_click(Value::str("flip")),
                    |ui| {
                        let side = if self.right { Align::End } else { Align::Start };
                        ui.with_keyed(
                            "card",
                            NodeSpec::column()
                                .float(FloatConfig::parent().at(side, Align::Center).self_at(side, Align::Center).offset(0.0, 0.0))
                                .pad_xy(14.0, 10.0)
                                .bg(t.raised)
                                .radius(8.0)
                                .border(1.0, t.border)
                                .transition(500.0)
                                .easing(Easing::EaseInOut)
                                .slide(),
                            |ui| {
                                ui.text("click the stage", TextStyle::new(13.0));
                            },
                        );
                    },
                );

                // keyframes, repeat, delay: a cycle each node walks alone.
                ui.text("keyframes · repeat · delay — a cycle, a direction, and siblings out of phase", TextStyle::new(12.0).color(t.muted));
                ui.with(NodeSpec::row().gap(10.0).cross_align(Align::End), |ui| {
                    for (name, repeat) in [
                        ("normal", Repeat::Normal),
                        ("reverse", Repeat::Reverse),
                        ("alternate", Repeat::Alternate),
                        ("alt-reverse", Repeat::AlternateReverse),
                    ]
                    .into_iter()
                    {
                        ui.with(NodeSpec::column().gap(4.0).cross_align(Align::Center), |ui| {
                            ui.with_keyed(
                                name,
                                NodeSpec::row()
                                    .width(Sizing::Fixed(48.0))
                                    .height(Sizing::Fixed(48.0))
                                    .bg(t.accent)
                                    .radius(6.0)
                                    .transition(1200.0)
                                    .repeat(repeat)
                                    .keyframes(vec![
                                        Keyframe::default().at(0.0).bg(t.accent).radius(6.0).opacity(1.0),
                                        Keyframe::default().at(0.5).bg(t.success).radius(24.0).opacity(0.6),
                                        Keyframe::default().at(1.0).bg(t.danger).radius(6.0).opacity(1.0),
                                    ]),
                                |_| {},
                            );
                            ui.text(name, TextStyle::new(10.0).color(t.muted));
                        });
                    }
                    // The chase: one cycle, five delays.
                    ui.with(NodeSpec::row().gap(4.0).cross_align(Align::Center), |ui| {
                        for i in 0..5 {
                            ui.with_keyed(
                                &format!("chase{i}"),
                                NodeSpec::row()
                                    .width(Sizing::Fixed(12.0))
                                    .height(Sizing::Fixed(12.0))
                                    .radius(6.0)
                                    .bg(t.faint)
                                    .transition(800.0)
                                    .delay(i as f32 * 160.0)
                                    .keyframes(vec![
                                        Keyframe::default().at(0.0).bg(t.faint).opacity(0.4),
                                        Keyframe::default().at(0.5).bg(t.accent).opacity(1.0),
                                        Keyframe::default().at(1.0).bg(t.faint).opacity(0.4),
                                    ]),
                                |_| {},
                            );
                        }
                    });
                });
                ui.text(
                    "every value above is what this frame declared; the core is between the last frame and this one",
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.as_str() {
            Some("race") => self.far = !self.far,
            Some("flip") => self.right = !self.right,
            _ => {
                if let Some(to) = ev.payload.get("to").and_then(Value::as_float) {
                    self.level = to as f32;
                }
            }
        }
    }
}

impl Example for Motion {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("click a lane", "race the easings"),
        ("click the stage", "slide the card"),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(520.0, 560.0)
    }

    /// The bar's width is tweened: one frame after the level changes the
    /// core is animating on its account, and a click that changes nothing
    /// starts nothing. (The keyframe cycles run forever, so "settled" is
    /// not a state this frame has; a tween's end is checked in
    /// `kui-core`'s own tests.)
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 520.0, 560.0);
        d.frame(self);
        d.frame(self);
        let full = d.key_of("100%").ok_or("no 100% button")?;
        d.click_key(self, full);
        d.check(self.level == 1.0, "the click sets the level")?;
        d.advance(0.016);
        d.frame(self);
        d.check(d.core.animating(), "one frame in, the bar is still moving")?;
        let stage = d.key_of("stage").ok_or("no stage")?;
        d.click_key(self, stage);
        d.frame(self);
        d.check(self.right, "the stage flips the card's anchor")?;
        d.check(
            d.core.animating(),
            "and the float slides rather than jumping",
        )
    }
}

kui_devtools::main!(Motion::default());
