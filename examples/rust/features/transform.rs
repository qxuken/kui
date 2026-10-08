//! A node turns about its pivot (ADR 0043): `rotate`, `scale`, `pivot`.
//! Paint-only — the card takes the room its upright self takes — and
//! everything after layout turns with it: the photo it clips inside its
//! rounded corners, the badge floating on its corner, the text, the hit
//! region (grab the card by its tilted edge) and the access rect.
//!
//! The card tilts as it is dragged, towards the side it is leaving — the
//! transition is off while the pointer holds it, so it follows at once —
//! and springs back when let go. The buttons tilt it by a notch, pick its
//! pivot (the centre, or the top-left corner, which swings it like a
//! sign), and toggle a chip that enters at `scale: 0.8`. Beside it, a
//! spinner is a box with `keyframes: [{ rotate: 0 }, { rotate: 1 }]` and
//! a pulse is one with `[{ scale: 1.08, at: 0.5 }]`: no `path`, no
//! `request_frame`.
//!
//! Run: cargo run -p kui-native --example transform [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{
    Align, App, Color, Core, CursorShape, Easing, Enter, FloatConfig, Keyframe, NodeSpec, Repeat,
    TextStyle, Transition, Ui, UiEvent, Value,
};

/// A notch of tilt, in turns.
const NOTCH: f32 = 0.02;
/// The most a drag tilts the card, in turns, reached at `DRAG_FULL` px.
const TILT_MAX: f32 = 0.05;
const DRAG_FULL: f32 = 200.0;
const CARD: (f32, f32) = (180.0, 240.0);

#[derive(Default)]
struct Turn {
    /// The tilt the buttons set, in turns.
    tilt: f32,
    /// The pointer's travel since the press, while a drag holds the card.
    drag: Option<f32>,
    corner: bool,
    chip: bool,
}

fn tag(kind: &str) -> Value {
    Value::map([("kind", Value::str(kind))])
}

impl Turn {
    /// What the card is turned by this frame: the buttons' tilt, plus the
    /// drag's lean while one holds it.
    fn turn(&self) -> f32 {
        let lean = self
            .drag
            .map_or(0.0, |dx| (dx / DRAG_FULL).clamp(-1.0, 1.0) * TILT_MAX);
        self.tilt + lean
    }
}

impl App for Turn {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let heading = |ui: &mut Ui<'_>, s: &str| {
            ui.text(s, TextStyle::new(12.0).color(t.muted));
        };
        ui.with(
            NodeSpec::column().fill().pad(24.0).gap(16.0).bg(t.bg),
            |ui| {
                ui.with(NodeSpec::row().gap(8.0).cross_align(Align::Center), |ui| {
                    widgets::button(ui, "◀ tilt", tag("left"));
                    widgets::button(ui, "tilt ▶", tag("right"));
                    widgets::button(ui, "upright", tag("reset"));
                    widgets::button(
                        ui,
                        if self.corner {
                            "pivot: corner"
                        } else {
                            "pivot: centre"
                        },
                        tag("pivot"),
                    );
                    widgets::button(
                        ui,
                        if self.chip { "hide chip" } else { "show chip" },
                        tag("chip"),
                    );
                    ui.text(
                        &format!("{:+.3} turn", self.turn()),
                        TextStyle::new(13.0).color(t.fg),
                    );
                });
                heading(
                    ui,
                    "drag the card sideways; it tilts towards where it is going and springs back",
                );
                ui.with(
                    NodeSpec::row()
                        .grow_width()
                        .gap(40.0)
                        .cross_align(Align::Start),
                    |ui| {
                        self.card(ui);
                        ui.with(NodeSpec::column().gap(24.0), |ui| {
                            heading(ui, "a spinner: keyframes on rotate");
                            ui.leaf_keyed(
                                "spinner",
                                NodeSpec::column()
                                    .size(48.0, 48.0)
                                    .radius(10.0)
                                    .bg(t.accent)
                                    .transition_with(
                                        Transition::ms(1600.0)
                                            .easing(Easing::Linear)
                                            .repeat(Repeat::Normal),
                                    )
                                    .keyframes(vec![
                                        Keyframe::default().rotate(0.0),
                                        Keyframe::default().rotate(1.0),
                                    ]),
                            );
                            heading(ui, "a pulse: keyframes on scale");
                            ui.leaf_keyed(
                                "pulse",
                                NodeSpec::column()
                                    .size(48.0, 48.0)
                                    .radius(24.0)
                                    .bg(Color::hex(0xe8505bff))
                                    .transition_with(
                                        Transition::ms(900.0)
                                            .easing(Easing::EaseInOut)
                                            .repeat(Repeat::Alternate),
                                    )
                                    .keyframes(vec![Keyframe::default().scale(1.08)]),
                            );
                            if self.chip {
                                heading(ui, "a chip that enters at scale 0.8");
                                ui.text_in_keyed(
                                    "chip",
                                    NodeSpec::row()
                                        .pad(8.0)
                                        .radius(14.0)
                                        .bg(t.accent)
                                        .transition(260.0)
                                        .easing(Easing::Spring)
                                        .enter(Enter::default().scale(0.8).opacity(0.0))
                                        .exit(Enter::default().scale(0.6).opacity(0.0)),
                                    "new",
                                    TextStyle::new(13.0).color(Color::WHITE),
                                );
                            }
                        });
                    },
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.kind() {
            Some("left") => self.tilt -= NOTCH,
            Some("right") => self.tilt += NOTCH,
            Some("reset") => self.tilt = 0.0,
            Some("pivot") => self.corner = !self.corner,
            Some("chip") => self.chip = !self.chip,
            Some("drag") => {
                let dx = ev
                    .payload
                    .get("dx")
                    .and_then(Value::as_float)
                    .unwrap_or(0.0) as f32;
                match ev.payload.get_str("phase").unwrap_or("") {
                    "start" | "move" => self.drag = Some(dx),
                    _ => self.drag = None,
                }
            }
            _ => {}
        }
    }
}

impl Turn {
    /// The card: rounded, clipping, holding a photo bigger than itself
    /// and a badge floating on its corner, turned by `turn()` about the
    /// pivot. The transition is off while a drag holds it, so the tilt
    /// follows the pointer; on, it springs back.
    fn card(&self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let (fx, fy) = if self.corner { (0.0, 0.0) } else { (0.5, 0.5) };
        let mut spec = NodeSpec::column()
            .size(CARD.0, CARD.1)
            .radius(18.0)
            .clip()
            .bg(Color::hex(0x20639bff))
            .shadow_color(Color::hex(0x00000060))
            .shadow_blur(16.0)
            .shadow_y(8.0)
            .on_drag("card")
            .on_click(tag("tap"))
            .label("Card")
            .cursor(if self.drag.is_some() {
                CursorShape::Grabbing
            } else {
                CursorShape::Grab
            })
            .rotate(self.turn())
            .pivot(fx, fy);
        if self.drag.is_none() {
            spec = spec.transition(420.0).easing(Easing::Spring);
        }
        ui.with_keyed("card", spec, |ui| {
            // The photo: a gradient box wider and taller than the card,
            // which the card's rounded clip cuts in its own turned space.
            ui.leaf_keyed(
                "photo",
                NodeSpec::column()
                    .size(CARD.0 + 60.0, CARD.1 * 0.62)
                    .gradient(kui_native::Gradient::to(
                        kui_native::Side::BottomRight,
                        [Color::hex(0xf9a03fff), Color::hex(0x8e5bd6ff)],
                    )),
            );
            ui.with(NodeSpec::column().pad(14.0).gap(4.0), |ui| {
                ui.text("Honey, 4", TextStyle::new(18.0).color(Color::WHITE));
                ui.text(
                    "tilts with the drag, by one row",
                    TextStyle::new(12.0).color(Color::hex(0xffffffb0)),
                );
            });
            ui.text_in_keyed(
                "badge",
                NodeSpec::row()
                    .pad(6.0)
                    .radius(12.0)
                    .bg(t.accent)
                    .float(FloatConfig::parent().offset(CARD.0 - 36.0, -10.0)),
                "★",
                TextStyle::new(13.0).color(Color::WHITE),
            );
        });
    }
}

impl Example for Turn {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("drag", "tilt the card"),
        ("buttons", "tilt by a notch, pick the pivot"),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(620.0, 420.0)
    }

    /// The card's clip entry carries its turn; a tilt button turns it
    /// and the spring brings a reset back; a drag leans it at once; and
    /// a press on the drawn card past its layout box reaches it.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 620.0, 420.0);
        d.frame(self);
        d.frame(self);
        let card = d.key_of("card").ok_or("no card")?;
        let rect = d.rect_of(card).ok_or("no card rect")?;
        // The clip entry of the card's background quad, in physical px.
        let angle = |d: &mut Drive<'_>| -> f32 {
            let (dl, _) = d.core.output();
            dl.quads
                .iter()
                .find(|q| {
                    q.kind == kui_native::QuadKind::Solid
                        && q.rect.x == rect.x
                        && q.rect.w == rect.w
                })
                .map(|q| dl.clip_of(q).transform.angle)
                .unwrap_or(f32::NAN)
        };
        let upright = angle(&mut d);
        d.check(upright == 0.0, "upright to start with")?;

        let right = d.key_of("tilt ▶").ok_or("no tilt button")?;
        d.click_key(self, right);
        d.frame(self);
        for _ in 0..90 {
            d.advance(1.0 / 60.0);
            d.frame(self);
        }
        let turned = angle(&mut d);
        let want = NOTCH * std::f32::consts::TAU;
        d.check(
            (turned - want).abs() < 1e-3,
            &format!("a notch turns the card {want} rad: {turned}"),
        )?;
        // The spinner and the pulse cycle for good, so the frame is always
        // owed; what settles is the transition.
        let owed = d.core.owed();
        d.check(!owed.transition && owed.cycle, "and the spring has settled")?;

        // A press on the drawn card past its layout box: the top-right
        // corner swings up and out when the card turns clockwise, so a
        // point just outside the layout box's right edge near the top is
        // on the drawn card.
        let x = rect.x + rect.w + 2.0;
        let y = rect.y + 20.0;
        let evs = d.click(self, x, y);
        d.check(
            evs.iter().any(|e| e.kind() == Some("tap")),
            "the card is hit where it is drawn",
        )?;

        // A drag leans it at once (no transition while held) and letting
        // go springs it back to the notch.
        let from = kui_native::Vec2::new(rect.x + 40.0, rect.y + 120.0);
        d.drag(self, from, kui_native::Vec2::new(from.x - 100.0, from.y));
        d.frame(self);
        d.check(self.drag.is_none(), "the drag ended")?;
        for _ in 0..120 {
            d.advance(1.0 / 60.0);
            d.frame(self);
        }
        let back = angle(&mut d);
        d.check(
            (back - want).abs() < 1e-3,
            &format!("back at the notch after the drag: {back}"),
        )?;
        let warnings = d.warnings();
        d.check(warnings.is_empty(), &format!("no warnings: {warnings:?}"))
    }
}

fn main() {
    kui_devtools::run("transform", Turn::default());
}
