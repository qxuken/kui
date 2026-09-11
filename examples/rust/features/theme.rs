//! The token reference: every colour a [`kui::Theme`] names, drawn in the
//! theme that names it, over every stock widget that reads it
//! (`docs/adr/0019-a-theme-derived-from-appearance-and-accent.md`).
//!
//! It is the page you check a palette against — the swatches say what the
//! roles *are*, and the widgets below them say what the roles *do*, so a
//! value that reads fine as a chip and badly as a menu row has nowhere to
//! hide. Nothing here paints a literal colour: every box on the screen is
//! a token, which is also the point.
//!
//! The base and the accent are the harness's to change — the dock's
//! `base` and `accent` rows, or `Ctrl+Shift+T` / `Ctrl+Shift+A` — which is
//! what every example gets, and this page is where to look while doing
//! it. Right-click for the menu: this page asks for the core's *drawn*
//! one, since on macOS the platform's own would be showing what AppKit
//! paints instead; the dock's `menus` row switches it back.
//!
//! Run: cargo run -p kui --example theme

use kui::widgets;
use kui::{Align, App, Color, NodeSpec, Sizing, TextStyle, Theme, Ui, UiEvent, Value};
use kui_devtools::Example;

/// What the demo buttons post: they are here to be looked at, not to say
/// anything, and the menu's rows post their own text.
const NOOP: &str = "noop";

#[derive(Default)]
struct Gallery {
    menu: Option<(f32, f32)>,
    said: Option<String>,
}

/// One token: a chip of the colour, its name, and the hex it resolved to.
/// The label sits *outside* the chip, because half of these are
/// translucent washes with nothing legible to write on them.
fn swatch(ui: &mut Ui<'_>, name: &str, c: Color, t: &Theme) {
    ui.with(NodeSpec::row().gap(8.0).cross_align(Align::Center), |ui| {
        ui.with(
            NodeSpec::row()
                .width(Sizing::Fixed(34.0))
                .height(Sizing::Fixed(20.0))
                .bg(c)
                .radius(4.0)
                .border(1.0, t.border),
            |_| {},
        );
        ui.with(NodeSpec::column().width(Sizing::Fixed(112.0)), |ui| {
            ui.text(name, TextStyle::new(12.0).color(t.fg));
            ui.text(
                &format!("#{:08x}", c.to_hex()),
                TextStyle::new(10.0).color(t.faint),
            );
        });
    });
}

/// A titled card, in the theme's own card colour.
fn card(ui: &mut Ui<'_>, title: &str, body: impl FnOnce(&mut Ui<'_>)) {
    let t = ui.theme();
    ui.with(
        NodeSpec::column()
            .gap(12.0)
            .pad(16.0)
            .bg(t.surface)
            .radius(10.0)
            .border(1.0, t.border),
        |ui| {
            ui.text(title, TextStyle::new(11.0).color(t.muted));
            body(ui);
        },
    );
}

impl App for Gallery {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.open_keyed(
            "page",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .gap(16.0)
                .pad(20.0)
                .bg(t.bg)
                // A window shorter than the swatches scrolls rather than
                // over-committing its column: without this the two rows of
                // cards shrink into each other.
                .scroll_y()
                // A secondary press asks the *topmost* node under the
                // pointer and stops there — it does not bubble the way a
                // key does — and this column covers the page.
                .on_context_menu(Value::map([("kind", "menu".into())])),
        );

        ui.text("design tokens", TextStyle::new(22.0));
        ui.text(
            "the dock's base and accent rows change the palette · right-click for the drawn menu",
            TextStyle::new(12.0).color(t.muted),
        );

        ui.with(NodeSpec::row().gap(16.0).width(Sizing::Grow(1.0)), |ui| {
            card(ui, "SURFACES", |ui| {
                for (n, c) in [
                    ("bg", t.bg),
                    ("surface", t.surface),
                    ("raised", t.raised),
                    ("sunken", t.sunken),
                    ("border", t.border),
                    ("border_strong", t.border_strong),
                ] {
                    swatch(ui, n, c, &t);
                }
            });
            card(ui, "TEXT & ACCENT", |ui| {
                for (n, c) in [
                    ("fg", t.fg),
                    ("muted", t.muted),
                    ("faint", t.faint),
                    ("accent", t.accent),
                    ("accent_soft", t.accent_soft),
                    ("on_accent", t.on_accent),
                ] {
                    swatch(ui, n, c, &t);
                }
            });
            card(ui, "STATE & STATUS", |ui| {
                for (n, c) in [
                    ("focus_ring", t.focus_ring),
                    ("selection", t.selection),
                    ("hover", t.hover),
                    ("success", t.success),
                    ("warning", t.warning),
                    ("danger", t.danger),
                ] {
                    swatch(ui, n, c, &t);
                }
            });
        });

        ui.with(NodeSpec::row().gap(16.0).width(Sizing::Grow(1.0)), |ui| {
            card(ui, "BUTTONS", |ui| {
                ui.with(NodeSpec::row().gap(10.0).cross_align(Align::Center), |ui| {
                    widgets::button(ui, "stock", Value::str(NOOP));
                    widgets::button_with(
                        ui,
                        "accented",
                        "accent",
                        widgets::button_spec(&ui.metrics())
                            .accent()
                            .on_click(Value::str(NOOP)),
                        Some("bg, hover and pressed all come off the accent"),
                    );
                    widgets::button_with(
                        ui,
                        "off",
                        "disabled",
                        widgets::button_spec(&ui.metrics()).disabled(true),
                        None,
                    );
                });
                ui.text(
                    "Tab to see the focus ring; hover the middle one for a tooltip.",
                    TextStyle::new(11.0).color(t.faint),
                );
            });
            card(ui, "TEXT & FIELDS", |ui| {
                widgets::text_input(ui, "search", "select me for the tint");
                ui.text("body text — theme.fg", TextStyle::default());
                ui.text(
                    "secondary — theme.muted",
                    TextStyle::new(13.0).color(t.muted),
                );
                ui.text(
                    "tertiary — theme.faint",
                    TextStyle::new(13.0).color(t.faint),
                );
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    for (n, c) in [("ok", t.success), ("warn", t.warning), ("fail", t.danger)] {
                        ui.text(n, TextStyle::new(13.0).color(c));
                    }
                });
            });
        });

        // Always declared, never conditional: a line that appears when a
        // menu row is chosen would reflow everything above it, and a
        // reference page that jumps while you use it is its own bug report.
        ui.text(
            &match &self.said {
                Some(said) => format!("last menu choice — {said}"),
                None => "last menu choice — none yet".to_string(),
            },
            TextStyle::new(12.0).color(if self.said.is_some() {
                t.accent
            } else {
                t.faint
            }),
        );

        ui.close();

        // The stock menu, drawn by `widgets::context_menu` in the theme's
        // own colours — the float that made `raised` a role of its own.
        if let Some((x, y)) = self.menu {
            widgets::context_menu(
                ui,
                kui::Vec2::new(x, y),
                &[
                    kui::MenuItem::new("Copy").accel("⌘C"),
                    kui::MenuItem::new("Paste"),
                    kui::MenuItem::separator(),
                    kui::MenuItem::new("Nothing doing").enabled(false),
                ],
            );
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("contextmenu") => {
                let at = |k| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
                self.menu = Some((at("x"), at("y")));
            }
            Some("dismiss") => self.menu = None,
            // Every other string payload here is a menu row's own id,
            // which the stock menu posts as the row's text — except the
            // one the demo buttons post, which is not a menu choice and
            // must not be reported as the last one.
            _ => match ev.payload.as_str() {
                Some(NOOP) | None => {}
                Some(s) => {
                    self.said = Some(s.to_string());
                    self.menu = None;
                }
            },
        }
    }
}

impl Example for Gallery {
    const KEYS: &'static [(&'static str, &'static str)] = &[("right-click", "the drawn menu")];

    /// The drawn menu, not the platform's: this page is showing what the
    /// theme paints, and on macOS the platform's own menu would be
    /// showing what AppKit paints instead.
    fn native_menus(&self) -> Option<bool> {
        Some(false)
    }
}

kui_devtools::main!(Gallery::default());
