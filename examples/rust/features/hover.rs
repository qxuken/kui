//! Hover, declared rather than tracked. Four ways a view says what the
//! pointer's arrival means, and none of them is state the app keeps:
//!
//! - `hover_bg`: the background while the pointer is over the node — the
//!   core swaps it, the view declares both colours once;
//! - `hoverable` + `Ui::is_hovered`: a node the core tracks so the *view*
//!   can branch on it — a tooltip, a reveal, a badge that reads its own
//!   hover on the next frame;
//! - `hover_group`: siblings that light together — a row's icon and its
//!   label are one hover, whichever of them the pointer is on, asked by
//!   `is_group_hovered`;
//! - `on_hover`: the arrival and the leaving as events, `enter` and
//!   `leave`, per node, for a model that wants to know — the count below.
//!
//! The pointer leaving the window leaves everything; a hover survives a
//! rebuild because the node's key does.
//!
//! Run: cargo run -p kui-native --example hover [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{Align, App, Core, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};

const ROWS: [(&str, &str); 3] = [("◆", "inbox"), ("●", "drafts"), ("▲", "sent")];

#[derive(Default)]
struct Hover {
    enters: u32,
    leaves: u32,
    /// Which row the pointer is over, from `on_hover`.
    over: Option<String>,
}

impl App for Hover {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(18.0)
                .cross_align(Align::Start),
            |ui| {
                ui.text(
                    "hover_bg · the core swaps the background",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    for (name, bg, hover) in [
                        ("surface", t.surface, t.hover),
                        ("accent", t.accent, t.accent_hover),
                        ("danger", t.danger, t.danger.with_alpha(0.7)),
                    ] {
                        ui.with_keyed(
                            name,
                            NodeSpec::row()
                                .pad_xy(16.0, 10.0)
                                .radius(8.0)
                                .bg(bg)
                                .hover_bg(hover)
                                .border(1.0, t.border),
                            |ui| {
                                ui.text(
                                    name,
                                    TextStyle::new(13.0)
                                        .color(kui_native::widgets::readable_on(bg)),
                                );
                            },
                        );
                    }
                });

                ui.text(
                    "hoverable + is_hovered · the view branches on its own hover",
                    TextStyle::new(12.0).color(t.muted),
                );
                let badge = ui.child_key("badge");
                let hovered = ui.is_hovered(badge);
                ui.with_keyed(
                    "badge",
                    NodeSpec::row()
                        .pad_xy(14.0, 8.0)
                        .radius(8.0)
                        .bg(t.raised)
                        .border(1.0, if hovered { t.accent } else { t.border })
                        .hoverable(),
                    |ui| {
                        ui.text(
                            if hovered {
                                "hovered — the border is the view's doing"
                            } else {
                                "hover me"
                            },
                            TextStyle::new(13.0),
                        );
                    },
                );

                ui.text(
                    "hover_group · an icon and its label are one hover",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Fixed(260.0))
                        .gap(2.0)
                        .pad(6.0)
                        .bg(t.surface)
                        .radius(8.0)
                        .border(1.0, t.border),
                    |ui| {
                        for (icon, label) in ROWS {
                            let lit = ui.is_group_hovered(NodeSpec::hover_group_id(label));
                            let bg = if lit { t.hover } else { t.surface };
                            // Two nodes, one group: whichever the pointer is
                            // on, both read `lit`. `on_hover` stays per node
                            // — the group is who lights together, not who
                            // reports — so the count below moves as the
                            // pointer crosses from the icon to the label.
                            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).gap(0.0), |ui| {
                                ui.with_keyed(
                                    &format!("{label}-icon"),
                                    NodeSpec::row()
                                        .width(Sizing::Fixed(36.0))
                                        .pad_xy(0.0, 6.0)
                                        .main_align(Align::Center)
                                        .bg(bg)
                                        .radius(4.0)
                                        .hover_group(label)
                                        .on_hover(Value::str(label)),
                                    |ui| {
                                        ui.text(icon, TextStyle::new(13.0).color(t.accent));
                                    },
                                );
                                ui.with_keyed(
                                    &format!("{label}-text"),
                                    NodeSpec::row()
                                        .width(Sizing::Grow(1.0))
                                        .pad_xy(8.0, 6.0)
                                        .bg(bg)
                                        .radius(4.0)
                                        .hover_group(label)
                                        .on_hover(Value::str(label)),
                                    |ui| {
                                        ui.text(label, TextStyle::new(13.0));
                                    },
                                );
                            });
                        }
                    },
                );

                ui.text(
                    &format!(
                        "on_hover · {} enters, {} leaves · over: {}",
                        self.enters,
                        self.leaves,
                        self.over.as_deref().unwrap_or("nothing")
                    ),
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.get("kind").and_then(Value::as_str) != Some("hover") {
            return;
        }
        let tag = ev
            .payload
            .get("tag")
            .and_then(Value::as_str)
            .map(str::to_string);
        match ev.payload.get("phase").and_then(Value::as_str) {
            Some("enter") => {
                self.enters += 1;
                self.over = tag;
            }
            Some("leave") => {
                self.leaves += 1;
                self.over = None;
            }
            _ => {}
        }
    }
}

impl Example for Hover {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(460.0, 420.0)
    }

    /// The pointer moves onto a badge, a group, and away: what the frame
    /// reports and what the events say agree.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 460.0, 420.0);
        d.frame(self);
        let badge = d.key_of("badge").ok_or("no badge")?;
        d.hover(self, badge);
        d.check(
            d.core.is_hovered(badge),
            "a hoverable node reports the pointer",
        )?;
        d.frame(self);

        let icon = d.key_of("drafts-icon").ok_or("no drafts icon")?;
        let text = d.key_of("drafts-text").ok_or("no drafts label")?;
        d.hover(self, icon);
        d.check(
            d.core.is_group_hovered(NodeSpec::hover_group_id("drafts")),
            "the pointer on the icon lights the group",
        )?;
        d.check(
            self.over.as_deref() == Some("drafts") && self.enters == 1,
            "and `on_hover` reports one enter",
        )?;
        d.hover(self, text);
        d.check(
            self.enters == 2 && self.leaves == 1 && self.over.as_deref() == Some("drafts"),
            "moving to the label leaves the icon and enters the label: on_hover is per node",
        )?;
        d.check(
            d.core.is_group_hovered(NodeSpec::hover_group_id("drafts")),
            "and the group stays lit across the move",
        )?;
        d.input(self, kui_native::InputEvent::CursorLeft);
        d.check(
            self.leaves == 2 && self.over.is_none(),
            "the pointer leaving the window leaves the group",
        )?;
        d.frame(self);
        d.check(!d.core.is_hovered(badge), "and nothing is hovered")
    }
}

kui_devtools::main!(Hover::default());
