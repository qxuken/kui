//! `on_drag`: a press-move-release on a node arrives as `drag` events —
//! `start` at the press, `move` once the pointer has travelled past the
//! slop, `end` at the release — with the pointer in viewport px (`x`,
//! `y`), the displacement *since the press* (`dx`, `dy`, absolute against
//! the press point so no delta is ever summed), and the node's `parent`
//! rect, so a handler can turn a position into a fraction of its
//! container without a geometry query. The core captures the pointer for
//! the drag: it keeps reporting after the pointer leaves the node, and
//! the release lands wherever it lands.
//!
//! Two things it is for: a **slider** whose value is what it was at the
//! press plus `dx` over the track — drawn by hand here because the drag
//! is the subject; an app's slider is the stock one
//! (`widgets::slider`, ADR 0034), which does this and the keys — and a
//! **card** whose float offset is
//! where it was at the press plus the displacement, kept inside the stage
//! (its own size from `on_layout`, the stage's from the event's `parent`).
//! A drag that started on the card is the card's until it ends, whatever
//! the pointer crosses.
//!
//! The cursor is declared, not derived: an `on_drag` node with no `cursor`
//! is the plain arrow, so both declare `grab` at rest and `grabbing`
//! while their drag runs — the model already knows which drag is on, and
//! the shape is one more thing the view says from it. The core holds
//! whichever shape the dragged node declared for as long as the pointer
//! is captured, wherever it goes.
//!
//! Run: cargo run -p kui-native --example drag [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{
    Align, App, Core, CursorShape, FloatConfig, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value,
};

const TRACK_W: f32 = 320.0;

#[derive(Default)]
struct Drag {
    /// The slider's value, 0..1.
    value: f32,
    /// The card's offset in its stage.
    card: (f32, f32),
    /// What was true at the press, for the displacement to add to.
    at_press: (f32, (f32, f32)),
    /// The card's own size, from its `on_layout`, so it stays inside the
    /// stage whose rect the drag event carries.
    card_size: (f32, f32),
    dragging: Option<String>,
    log: String,
}

impl App for Drag {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // The open hand over a handle at rest, the closed one while its
        // drag runs: `dragging` names which.
        let hand = |tag: &str| {
            if self.dragging.as_deref() == Some(tag) {
                CursorShape::Grabbing
            } else {
                CursorShape::Grab
            }
        };
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(18.0)
                .cross_align(Align::Start),
            |ui| {
                ui.text(
                    &format!("a slider: the drag's x is the value · {:.0}%", self.value * 100.0),
                    TextStyle::new(12.0).color(t.muted),
                );
                // The track owns the drag, and its fill is the value: a
                // drag anywhere on it moves the value by the travel.
                ui.with_keyed(
                    "track",
                    NodeSpec::row()
                        .size(TRACK_W, 24.0)
                        .bg(t.sunken)
                        .radius(12.0)
                        .on_drag("slider")
                        .cursor(hand("slider")),
                    |ui| {
                        ui.leaf(
                            NodeSpec::row()
                                .width(Sizing::Percent(self.value))
                                .grow_height()
                                .bg(t.accent)
                                .radius(12.0));
                    },
                );

                ui.text(
                    "a card: the drag's steps move its float offset · the pointer is captured until the release",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with_keyed(
                    "stage",
                    NodeSpec::row()
                        .size(420.0, 200.0)
                        .bg(t.surface)
                        .radius(10.0)
                        .border(1.0, t.border),
                    |ui| {
                        let grabbing = self.dragging.as_deref() == Some("card");
                        ui.with_keyed(
                            "card",
                            NodeSpec::column()
                                .float(
                                    FloatConfig::parent()
                                        .inside(Align::Start, Align::Start)
                                        .offset(self.card.0, self.card.1),
                                )
                                .pad_xy(16.0, 12.0)
                                .bg(if grabbing { t.accent_soft } else { t.raised })
                                .radius(8.0)
                                .border(1.0, if grabbing { t.accent } else { t.border })
                                .on_drag("card")
                                .cursor(hand("card"))
                                .on_layout("card"),
                            |ui| {
                                ui.text("drag me", TextStyle::new(14.0));
                                ui.text(
                                    &format!("at {:.0}, {:.0}", self.card.0, self.card.1),
                                    TextStyle::new(11.0).color(t.muted),
                                );
                            },
                        );
                    },
                );
                ui.text(&self.log, TextStyle::new(12.0).color(t.faint));
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        let num = |k: &str| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
        match ev.kind() {
            Some("layout") => {
                self.card_size = (num("w"), num("h"));
                return;
            }
            Some("drag") => {}
            _ => return,
        }
        let phase = ev.payload.get_str("phase").unwrap_or("");
        let tag = ev.payload.get_str("tag").unwrap_or("");
        self.log = format!(
            "{tag} {phase} x {:.0} y {:.0} dx {:.0} dy {:.0}",
            num("x"),
            num("y"),
            num("dx"),
            num("dy")
        );
        match (tag, phase) {
            ("slider", "start") => {
                self.dragging = Some("slider".into());
                self.at_press.0 = self.value;
            }
            ("slider", "end") => self.dragging = None,
            ("slider", _) => {
                // The value at the press plus the displacement over the
                // track's width: absolute, so nothing accumulates.
                self.value = (self.at_press.0 + num("dx") / TRACK_W).clamp(0.0, 1.0);
            }
            ("card", "start") => {
                self.dragging = Some("card".into());
                self.at_press.1 = self.card;
            }
            ("card", "move") => {
                // The offset at the press plus the displacement, kept
                // inside the stage: the event carries the parent's rect,
                // and the card's own size came from its `on_layout`.
                let (ox, oy) = self.at_press.1;
                let parent = ev.payload.get("parent");
                let dim = |k: &str| {
                    parent
                        .and_then(|p| p.get(k))
                        .and_then(Value::as_float)
                        .unwrap_or(0.0) as f32
                };
                let max_x = (dim("w") - self.card_size.0).max(0.0);
                let max_y = (dim("h") - self.card_size.1).max(0.0);
                self.card = (
                    (ox + num("dx")).clamp(0.0, max_x),
                    (oy + num("dy")).clamp(0.0, max_y),
                );
            }
            ("card", "end") => self.dragging = None,
            _ => {}
        }
    }
}

impl Example for Drag {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(500.0, 380.0)
    }

    /// A drag along the track moves the value by the travel over the
    /// track's width; a drag across the card moves it by the pointer's
    /// travel and keeps reporting after the pointer has left it.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 500.0, 380.0);
        d.frame(self);
        let track = d.key_of("track").ok_or("no track")?;
        let r = d.rect_of(track).ok_or("no track rect")?;
        let (x0, y0) = (r.x + 10.0, r.y + 12.0);
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(x0, y0)),
        );
        d.check(
            d.core.cursor_shape() == CursorShape::Grab,
            "the track declares the open hand at rest",
        )?;
        let evs = d.input(self, kui_native::InputEvent::mouse_down(1));
        d.check(
            evs.iter()
                .any(|e| e.payload.get_str("phase") == Some("start")),
            "a press on the track starts a drag",
        )?;
        d.frame(self);
        d.check(
            d.core.cursor_shape() == CursorShape::Grabbing,
            "and the frame after the start declares the closed one",
        )?;
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(x0 + TRACK_W * 0.5, y0)),
        );
        d.check(
            (self.value - 0.5).abs() < 0.02,
            "half the track's width of travel is half the value",
        )?;
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(x0 + TRACK_W * 0.9, y0)),
        );
        d.check(
            (self.value - 0.9).abs() < 0.02,
            "and the value follows the pointer, not the steps",
        )?;
        d.input(self, kui_native::InputEvent::mouse_up());
        d.frame(self);
        d.check(
            d.core.cursor_shape() == CursorShape::Grab,
            "the release opens the hand again",
        )?;

        let card = d.key_of("card").ok_or("no card")?;
        let c = d.rect_of(card).ok_or("no card rect")?;
        let (x0, y0) = (c.x + c.w / 2.0, c.y + c.h / 2.0);
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(x0, y0)),
        );
        d.input(self, kui_native::InputEvent::mouse_down(1));
        d.check(
            self.dragging.is_some(),
            "a press on the card starts its drag",
        )?;
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(x0 + 60.0, y0 + 30.0)),
        );
        d.frame(self);
        d.check(
            (self.card.0 - 60.0).abs() < 1.0 && (self.card.1 - 30.0).abs() < 1.0,
            "a move of 60,30 moves the card by 60,30",
        )?;
        // Far outside the card and the stage: the capture keeps the drag,
        // and the card stops at the stage's edge.
        d.input(
            self,
            kui_native::InputEvent::CursorMoved(kui_native::Vec2::new(x0 + 600.0, y0 + 30.0)),
        );
        d.frame(self);
        d.check(
            self.card.0 > 200.0 && self.card.0 + self.card_size.0 <= 420.0 + 0.5,
            "the pointer leaving the card does not end the drag, and the card stops at the edge",
        )?;
        d.input(self, kui_native::InputEvent::mouse_up());
        d.check(
            self.dragging.is_none(),
            "the release ends it, wherever it lands",
        )
    }
}

kui_devtools::main!(Drag::default());
