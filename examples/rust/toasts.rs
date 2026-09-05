//! Entrance and exit transitions: `enter` says where a node starts the
//! first frame it is seen and `exit` where it ends the frame after the view
//! stops declaring it, so a toast slides in from off screen and back out
//! again without the view staging a frame to animate from — or keeping a
//! dead toast in its model to animate it away.
//!
//! Four things worth watching:
//!   - a toast arrives from the right and fades up (`enter` with an offset
//!     and a bg), and it does that on the very frame it appears;
//!   - it leaves the same way (`exit`, which is an `enter` read the other
//!     way): the app drops it from `self.toasts` the moment it expires, and
//!     what slides back out is a *copy* the core kept — frozen where layout
//!     left it, in its place, and inert. Nothing in this file waits for it;
//!   - older toasts `slide` down as the newest one pushes into the stack,
//!     and back up as one goes — position eases whenever layout moves them,
//!     which is what `slide` buys on top of the entrance, and it happens
//!     while the departing toast is still on screen beside them;
//!   - "clear" drops the whole stack in one frame. Every toast departs at
//!     once, and they all animate out, because `exit` is opt-in per node
//!     and a handful of cards is nowhere near the store's budget. A list
//!     that dropped a thousand rows would be, and would say so.
//!
//! Expiry needs a clock the core doesn't have, so the app keeps its own
//! `Instant`s and asks for the next frame while any toast is still due to
//! go. With none left it stops asking — and the window keeps drawing anyway
//! until the last exit finishes, because a departing subtree is mid-flight
//! and `animating()` says so.
//!
//! Run: cargo run -p kui --example toasts

use std::time::{Duration, Instant};

use kui::{
    Align, App, Color, Easing, Enter, FloatConfig, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value,
    widgets,
};

const LIFETIME: Duration = Duration::from_millis(3200);
fn card() -> Color {
    Color::rgb8(0x1b, 0x1e, 0x28)
}

fn edge() -> Color {
    Color::rgb8(0x2f, 0x33, 0x42)
}

fn ink() -> Color {
    Color::rgb8(0x8a, 0x8f, 0xa3)
}

/// The same color with nothing behind it — what a toast fades up from.
fn clear(c: Color) -> Color {
    Color { a: 0.0, ..c }
}

struct Toast {
    /// Stable across frames: the node key the entrance and the slide are
    /// keyed by. Reusing an index here would make a dismissed toast hand
    /// its tween to the one that shuffled into its place.
    id: u64,
    text: String,
    born: Instant,
}

#[derive(Default)]
struct Toasts {
    toasts: Vec<Toast>,
    next_id: u64,
    sent: u64,
    panel: bool,
}

impl Toasts {
    /// The stack: a viewport float pinned to the bottom-right corner, with
    /// the toasts themselves in ordinary flow inside it.
    fn stack(&self, ui: &mut Ui<'_>) {
        ui.with(
            NodeSpec::column()
                .float(
                    FloatConfig::viewport()
                        .at(Align::End, Align::End)
                        .self_at(Align::End, Align::End)
                        .offset(-24.0, -24.0),
                )
                .gap(10.0)
                .cross_align(Align::End),
            |ui| {
                for t in &self.toasts {
                    ui.with_keyed(
                        &format!("toast-{}", t.id),
                        NodeSpec::column()
                            .width(Sizing::Fixed(268.0))
                            .pad(14.0)
                            .gap(3.0)
                            .bg(card())
                            .radius(10.0)
                            .border(1.0, edge())
                            .transition(260.0)
                            // In from beyond the right edge, fading up. The
                            // transparent part happens off screen, so what
                            // you see is a card that is already there.
                            .enter(Enter::from(340.0, 0.0).bg(clear(card())))
                            // And out the same way. The app has already
                            // forgotten this toast by the time this runs:
                            // what leaves is the core's copy of it.
                            .exit(Enter::from(340.0, 0.0).bg(clear(card())))
                            // And afterwards it keeps following layout, so
                            // the stack closes up when one of them goes.
                            .slide(),
                        |ui| {
                            ui.text(&t.text, TextStyle::new(13.0));
                            ui.text(
                                "clears itself in a moment",
                                TextStyle::new(11.0).color(ink()),
                            );
                        },
                    );
                }
            },
        );
    }

    /// The panel: same idea on the other axis, and with a spring, so it
    /// overshoots its edge slightly on the way in. It is declared before
    /// the latency HUD, so the HUD is painted over it — and its ghost
    /// leaves under the HUD too, where the panel was.
    fn side_panel(&self, ui: &mut Ui<'_>) {
        if !self.panel {
            return;
        }
        ui.with_keyed(
            "panel",
            NodeSpec::column()
                .float(
                    FloatConfig::viewport()
                        .at(Align::Start, Align::Start)
                        .self_at(Align::Start, Align::Start),
                )
                .width(Sizing::Fixed(240.0))
                .height(Sizing::Percent(1.0))
                .pad(20.0)
                .gap(12.0)
                .bg(Color::rgb8(0x14, 0x16, 0x1e))
                .border(1.0, edge())
                .transition(420.0)
                .easing(Easing::Spring)
                .enter(Enter::from(-240.0, 0.0))
                // A spring on the way in; on the way out the ghost samples
                // the spring as an ease-out, since nothing can retarget a
                // node the view has stopped talking about.
                .exit(Enter::from(-240.0, 0.0)),
            |ui| {
                ui.text("Panel", TextStyle::new(15.0));
                ui.text(
                    "Entered from one width to the left, and it leaves the \
                     same way. Close and open it again and it enters again: \
                     the ghost is discarded the moment the key comes back, \
                     so the two never overlap.",
                    TextStyle::new(12.0).color(ink()),
                );
            },
        );
    }
}

impl App for Toasts {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let now = Instant::now();
        self.toasts
            .retain(|t| now.duration_since(t.born) < LIFETIME);
        // Nothing else would wake the window on a toast's own deadline.
        if !self.toasts.is_empty() {
            ui.request_frame();
        }

        ui.window_title("kui — toasts");
        ui.configure_root(NodeSpec::column().fill().center().gap(20.0));

        ui.text("enter and exit: where a node starts, and where it ends", {
            TextStyle::new(20.0)
        });
        ui.text(
            "A transition never animates in from nowhere, so a node's first \
             sight snaps: `enter` gives it somewhere to come from. Nor out \
             into nowhere — a node the view stops declaring is gone before \
             the frame ends — so `exit` has the core keep a picture of it \
             and play that out instead.",
            TextStyle::new(13.0).color(ink()),
        );
        ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
            widgets::button(ui, "notify", Value::map([("kind", "notify".into())]));
            widgets::button(ui, "toggle panel", Value::map([("kind", "panel".into())]));
            widgets::button(ui, "clear", Value::map([("kind", "clear".into())]));
        });

        self.side_panel(ui);
        self.stack(ui);
        widgets::latency_hud_at(ui, Align::Start, Align::End);
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("notify") => {
                self.sent += 1;
                self.next_id += 1;
                self.toasts.push(Toast {
                    id: self.next_id,
                    text: format!("Saved change #{}", self.sent),
                    born: Instant::now(),
                });
            }
            Some("panel") => self.panel = !self.panel,
            Some("clear") => self.toasts.clear(),
            _ => {}
        }
    }
}

fn main() {
    kui::run("kui — toasts", Toasts::default(), vec![]).unwrap();
}
