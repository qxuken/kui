//! The tooltip: a float below a hovered node, and the same fact told to
//! assistive technology. Three spellings of one thing:
//!
//! - `widgets::tooltip(ui, text)` inside a node the view knows is hovered
//!   (`hoverable` + `is_hovered`) — the float, drawn by the view when the
//!   view decides, which is also how a tooltip shows on a click or a
//!   first frame;
//! - `NodeSpec::apply_tooltip(hint)`: the prop half — the node tracks hover
//!   and the hint is its accessible `description`, what a screen reader
//!   says after the name — and the stock button's `hint` argument, which
//!   floats it while hovered;
//! - `tooltip_with`: the same chrome around anything — a legend, a
//!   shortcut hint with its own layout.
//!
//! A tooltip is a float with `fit`: near the window's bottom edge it
//! mirrors above the node instead of hanging off the frame.
//!
//! Run: cargo run -p kui-native --example tooltip [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{Align, App, Core, NodeSpec, TextStyle, Ui, UiEvent};

#[derive(Default)]
struct Tooltip {
    clicks: u32,
    /// The badge's tooltip is pinned open until the first click, so it can
    /// be seen without a pointer.
    pinned: bool,
}

impl App for Tooltip {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(20.0)
                .cross_align(Align::Start),
            |ui| {
                ui.text("the view's: `hoverable`, `is_hovered`, `widgets::tooltip`", TextStyle::new(12.0).color(t.muted));
                let badge = ui.child_key("badge");
                let show = ui.is_hovered(badge) || !self.pinned;
                ui.with_keyed(
                    "badge",
                    NodeSpec::row()
                        .pad_xy(12.0, 6.0)
                        .radius(10.0)
                        .bg(t.raised)
                        .border(1.0, t.border)
                        .hoverable(),
                    |ui| {
                        ui.text("?", TextStyle::new(13.0));
                        if show {
                            widgets::tooltip(ui, "a float: out of flow, on top, unclipped — pinned until the first click");
                        }
                    },
                );

                ui.text("the prop's: `apply_tooltip` — hover tracking and the accessible description in one", TextStyle::new(12.0).color(t.muted));
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    widgets::button_with(
                        ui,
                        "save",
                        "save",
                        widgets::button_spec(&ui.theme(), &ui.metrics()).on_click("save").apply_tooltip("⌘S · write the file"),
                        Some("⌘S · write the file"),
                    );
                    widgets::button_with(
                        ui,
                        "run",
                        "run",
                        widgets::button_spec(&ui.theme(), &ui.metrics()).accent().on_click("run").apply_tooltip("⌘R · run the current file"),
                        Some("⌘R · run the current file"),
                    );
                });

                ui.text("`tooltip_with`: the chrome around anything", TextStyle::new(12.0).color(t.muted));
                let legend = ui.child_key("legend");
                let over = ui.is_hovered(legend);
                ui.with_keyed(
                    "legend",
                    NodeSpec::row()
                        .pad_xy(12.0, 6.0)
                        .radius(8.0)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .hoverable(),
                    |ui| {
                        ui.text("hover for the shortcuts", TextStyle::new(13.0));
                        if over {
                            widgets::tooltip_with(ui, |ui| {
                                ui.with(NodeSpec::column().gap(4.0), |ui| {
                                    for (k, what) in [("⌘S", "save"), ("⌘R", "run"), ("⌘-Shift-P", "the palette")] {
                                        ui.with(NodeSpec::row().gap(10.0), |ui| {
                                            ui.text_in(NodeSpec::row().width(80.0), k, TextStyle::new(12.0).color(t.accent).mono());
                                            ui.text(what, TextStyle::new(12.0).color(t.muted));
                                        });
                                    }
                                });
                            });
                        }
                    },
                );
                ui.leaf(NodeSpec::column().grow_height());
                let low = ui.child_key("low");
                let over_low = ui.is_hovered(low);
                ui.with_keyed(
                    "low",
                    NodeSpec::row()
                        .pad_xy(12.0, 6.0)
                        .radius(8.0)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .hoverable(),
                    |ui| {
                        ui.text("near the bottom: the float mirrors above", TextStyle::new(13.0));
                        if over_low {
                            widgets::tooltip(ui, "fit: mirrored back across the node, not off the frame");
                        }
                    },
                );
                ui.text(&format!("{} clicks", self.clicks), TextStyle::new(12.0).color(t.faint));
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.as_str().is_some() {
            self.clicks += 1;
            self.pinned = true;
        }
    }
}

impl Example for Tooltip {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(520.0, 360.0)
    }

    /// The badge's tooltip is in the frame until a click, then only while
    /// hovered; a button's hint is its accessible description whether or
    /// not it is hovered.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 520.0, 360.0);
        d.frame(self);
        let has_text = |d: &mut Drive<'_>, s: &str| {
            d.core
                .access_tree()
                .nodes
                .iter()
                .any(|n| n.name.as_deref().is_some_and(|x| x.contains(s)))
        };
        let shown = has_text(&mut d, "a float: out of flow");
        d.check(shown, "the badge's tooltip shows before any click")?;
        let save = d.key_of("save").ok_or("no save button")?;
        d.click_key(self, save);
        d.frame(self);
        let shown = has_text(&mut d, "a float: out of flow");
        d.check(!shown, "and hides after the first click")?;
        let badge = d.key_of("badge").ok_or("no badge")?;
        d.hover(self, badge);
        d.frame(self);
        let shown = has_text(&mut d, "a float: out of flow");
        d.check(shown, "hovering the badge shows it again")?;

        let desc = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.name.as_deref() == Some("save"))
            .and_then(|n| n.description.clone());
        d.check(
            desc.as_deref() == Some("⌘S · write the file"),
            "a button's hint is its accessible description, hovered or not",
        )
    }
}

kui_devtools::main!(Tooltip::default());
