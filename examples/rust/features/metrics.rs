//! The sizes the stock widgets are built from, and an app's own controls
//! agreeing with them (`kui::Metrics`, backlog T2): the palette's other
//! axis. Three densities of the same page — the stock set, `compact`, and
//! the stock set scaled up — switched by the buttons at the top, and a
//! card of the app's own that reads `ui.metrics()` for its radius and its
//! padding, so it changes with the stock button beside it without a
//! number of its own.
//!
//! Nothing here scales by itself: `env.scale` is the renderer's and comes
//! after; a density is the app's choice, made here by a click.
//!
//! Run: cargo run -p kui --example metrics [-- --headless]

use kui::menu::MenuItem;
use kui::widgets;
use kui::{Align, App, Core, Metrics, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_devtools::{Drive, Example};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Density {
    Comfortable,
    Compact,
    Large,
}

impl Density {
    fn metrics(self) -> Metrics {
        match self {
            Density::Comfortable => Metrics::comfortable(),
            Density::Compact => Metrics::compact(),
            Density::Large => Metrics::comfortable().scaled(1.4),
        }
    }

    fn name(self) -> &'static str {
        match self {
            Density::Comfortable => "comfortable",
            Density::Compact => "compact",
            Density::Large => "large",
        }
    }
}

struct Page {
    density: Density,
    menu: Option<(f32, f32)>,
}

impl App for Page {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // The choice is made before the frame's widgets are built, so
        // every one of them — including the buttons that choose — is in
        // the chosen density.
        ui.core().set_metrics(self.density.metrics());
        let t = ui.theme();
        let m = ui.metrics();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(20.0)
                .bg(t.bg)
                .on_context_menu(Value::map([("kind", "contextmenu".into())])),
            |ui| {
                ui.text("metrics", TextStyle::new(22.0).color(t.fg));
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    for d in [Density::Comfortable, Density::Compact, Density::Large] {
                        // The chosen one in the accent, the others in the
                        // surface: `accent` asks for the whole family, so
                        // it is declared only where it is wanted.
                        let spec = widgets::button_spec(&m).on_click(Value::str(d.name()));
                        let spec = if d == self.density {
                            spec.accent()
                        } else {
                            spec.bg(t.surface).hover_bg(t.hover).pressed_bg(t.pressed)
                        };
                        widgets::button_with(ui, d.name(), d.name(), spec, None);
                    }
                });
                ui.text(
                    &format!(
                        "control {}px in {}×{} · radius {} · menu {} wide · titlebar {}",
                        m.control_text,
                        m.control_pad_x,
                        m.control_pad_y,
                        m.radius,
                        m.menu_width,
                        m.titlebar_h
                    ),
                    TextStyle::new(12.0).color(t.muted).mono(),
                );

                // The stock widgets, as the metrics build them.
                ui.with(NodeSpec::row().gap(16.0).cross_align(Align::Center), |ui| {
                    let spec = widgets::button_spec(&m).on_click(Value::str("noop"));
                    widgets::button_with(
                        ui,
                        "a stock button",
                        "a stock button",
                        spec,
                        Some("a stock tooltip, in the hint metrics"),
                    );
                    ui.with(NodeSpec::column().width(Sizing::Fixed(200.0)), |ui| {
                        widgets::text_input(ui, "field", "a stock field");
                    });
                });

                // A control of the app's own, built from the same numbers:
                // it agrees with the stock button on its corner and its
                // padding at every density, and copies nothing.
                ui.with_keyed(
                    "own",
                    NodeSpec::row()
                        .pad_xy(m.control_pad_x, m.control_pad_y)
                        .radius(m.radius)
                        .gap(m.control_pad_x)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .cross_align(Align::Center)
                        // Hoverable so the drive can read its box off the
                        // hit list; it lights on hover, which is harmless.
                        .hoverable()
                        .hover_bg(t.hover),
                    |ui| {
                        ui.text(
                            "a card of the app's own",
                            TextStyle::new(m.control_text).color(t.fg),
                        );
                        ui.text(
                            "same radius, same padding",
                            TextStyle::new(m.hint_text).color(t.muted),
                        );
                    },
                );

                ui.text(
                    "right-click for the menu; hover the button for a tooltip",
                    TextStyle::new(11.0).color(t.faint),
                );
                if let Some((x, y)) = self.menu {
                    widgets::context_menu(
                        ui,
                        kui::Vec2::new(x, y),
                        &[MenuItem::new("A row"), MenuItem::new("Another")],
                    );
                }
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.as_str() {
            Some("comfortable") => self.density = Density::Comfortable,
            Some("compact") => self.density = Density::Compact,
            Some("large") => self.density = Density::Large,
            _ => {}
        }
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("contextmenu") => {
                let at = |k| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
                self.menu = Some((at("x"), at("y")));
            }
            Some("dismiss") => self.menu = None,
            _ => {}
        }
    }
}

impl Example for Page {
    /// Compact shrinks the stock button and the app's own card together,
    /// large grows them, and the corpus's stock set is what "comfortable"
    /// draws.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 800.0, 500.0);
        let boxes = |d: &mut Drive<'_>, app: &mut Page| {
            d.frame(app);
            let stock = d.key_of("a stock button").unwrap();
            let own = d.key_of("own").unwrap();
            (d.rect_of(stock).unwrap(), d.rect_of(own).unwrap())
        };
        let (stock, own) = boxes(&mut d, self);
        d.check(
            d.core.metrics() == &Metrics::default(),
            "comfortable is the stock set",
        )?;
        let compact = d.key_of("compact").unwrap();
        d.click_key(self, compact);
        let (stock2, own2) = boxes(&mut d, self);
        d.check(stock2.h < stock.h, "compact shrinks the stock button")?;
        d.check(own2.h < own.h, "and the app's own card with it")?;
        let large = d.key_of("large").unwrap();
        d.click_key(self, large);
        let (stock3, own3) = boxes(&mut d, self);
        d.check(stock3.h > stock.h, "large grows the stock button")?;
        d.check(own3.h > own.h, "and the card")?;
        d.check(
            d.core.metrics().titlebar_h == Metrics::default().titlebar_h * 1.4,
            "scaled multiplies every length, the titlebar included",
        )?;
        Ok(())
    }
}

kui_devtools::main!(Page {
    density: Density::Comfortable,
    menu: None,
});
