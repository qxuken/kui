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
//! Keys: `1` follow the OS · `2` light · `3` dark · `a` cycle an accent of
//! the app's own · `0` back to the OS accent. Right-click for the menu —
//! this window opts out of the platform's own (`set_native_menus(false)`)
//! so the drawn one, the one the theme paints, is what you see.
//!
//! Run: cargo run -p kui --example theme

use kui::widgets;
use kui::{
    Align, App, Appearance, Color, NodeSpec, Sizing, TextStyle, Theme, ThemeSource, Ui, UiEvent,
    Value,
};

/// The accents `a` walks: kui's own, then four the OS might report.
const ACCENTS: [(&str, u32); 5] = [
    ("kui blue", 0x3b5bd4ff),
    ("macOS blue", 0x007affff),
    ("macOS yellow", 0xffc409ff),
    ("macOS pink", 0xf74f9eff),
    ("forest", 0x2f7d4fff),
];

#[derive(Default)]
struct Gallery {
    /// `None` = follow the OS. Otherwise the base the app pinned.
    base: Option<Appearance>,
    /// Index into [`ACCENTS`], or `None` for the OS's own.
    accent: Option<usize>,
    menu: Option<(f32, f32)>,
    said: Option<String>,
}

impl Gallery {
    /// The source the two keys add up to. Every combination of "who picks
    /// the base" and "who picks the accent" is one of the three sources —
    /// which is the argument that there need only be three.
    fn source(&self, os_accent: Option<Color>) -> ThemeSource {
        let accent = self.accent.map(|i| Color::hex(ACCENTS[i].1));
        match (self.base, accent) {
            (None, None) => ThemeSource::Derived,
            (None, Some(c)) => ThemeSource::DerivedWithAccent(c),
            // Pinning the base is not a reason to stop honouring the OS
            // accent: what the app chose here is the light or the dark,
            // and the accent is still whoever's it was.
            (Some(app), c) => ThemeSource::Pinned(Theme::derive(app, c.or(os_accent))),
        }
    }

    fn accent_name(&self) -> &'static str {
        self.accent.map_or("the OS's", |i| ACCENTS[i].0)
    }
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
        // The drawn menu, not the platform's: this window is showing what
        // the theme paints, and on macOS the platform's own menu would be
        // showing what AppKit paints instead.
        ui.core().set_native_menus(false);
        let os_accent = ui.env().system.accent;
        ui.core().set_theme_source(self.source(os_accent));
        let t = ui.theme();

        ui.window_title(&format!(
            "kui — theme ({}, accent {})",
            match self.base {
                None => "following the OS",
                Some(Appearance::Light) => "light",
                _ => "dark",
            },
            self.accent_name()
        ));
        ui.configure_root(NodeSpec::column().fill().bg(t.bg));
        // One `on_key` sink holding the keyboard, the way `splitmux` does
        // it: keys route from whatever holds focus, so a shell that wants
        // the whole keyboard takes focus for its sink and lets ADR 0011
        // bubble back whatever a focused control does not claim — Tab
        // still walks the buttons, and `2` still switches the base while
        // one of them has the ring.
        let sink = ui.open_keyed(
            "keys",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .gap(16.0)
                .pad(20.0)
                // A window shorter than the swatches scrolls rather than
                // over-committing its column: without this the two rows of
                // cards shrink into each other.
                .scroll_y()
                .on_key(Value::Null)
                // On the sink and not on the root: a secondary press asks
                // the *topmost* node under the pointer and stops there —
                // it does not bubble the way a key does — and this sink
                // covers the window.
                .on_context_menu(Value::map([("kind", "menu".into())])),
        );
        ui.take_key_focus(sink);

        ui.text("design tokens", TextStyle::new(22.0));
        ui.text(
            "1 follow the OS · 2 light · 3 dark · a cycle an accent · 0 the OS accent · right-click for a menu",
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
                    widgets::button(ui, "stock", Value::str("noop"));
                    widgets::button_with(
                        ui,
                        "accented",
                        "accent",
                        widgets::button_spec().accent().on_click(Value::str("noop")),
                        Some("bg, hover and pressed all come off the accent"),
                    );
                    widgets::button_with(
                        ui,
                        "off",
                        "disabled",
                        widgets::button_spec().disabled(true),
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
            Some("key") => {
                let Some(code) = ev.payload.get("code").and_then(Value::as_str) else {
                    return;
                };
                match code {
                    "1" => self.base = None,
                    "2" => self.base = Some(Appearance::Light),
                    "3" => self.base = Some(Appearance::Dark),
                    "0" => self.accent = None,
                    "a" => self.accent = Some(self.accent.map_or(0, |i| (i + 1) % ACCENTS.len())),
                    _ => {}
                }
            }
            Some("contextmenu") => {
                let at = |k| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
                self.menu = Some((at("x"), at("y")));
            }
            Some("dismiss") => self.menu = None,
            _ => {
                // Every other payload here is a menu row's own id, which
                // the stock menu posts as the row's text.
                if let Some(s) = ev.payload.as_str() {
                    self.said = Some(s.to_string());
                    self.menu = None;
                }
            }
        }
    }
}

fn main() {
    kui::run("kui — theme", Gallery::default(), vec![]).unwrap();
}
