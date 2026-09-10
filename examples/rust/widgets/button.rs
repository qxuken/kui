//! The stock button, in every state it has. `widgets::button(ui, text,
//! payload)` is the whole of it for most uses: a keyed row with the
//! theme's three backgrounds (rest, hover, pressed), a label readable on
//! whichever it is, a click payload, and a `button` role named by its
//! text. `button_with` hands the spec back to the caller for the rest —
//! `accent` (the whole family off the theme's accent, not just the rest
//! colour), `disabled` (dimmed, inert, out of the Tab ring), a `label`
//! when the text is not the name, a `description`, a `tooltip` hint — and
//! `button_spec` / `button_palette` / `readable_on` are the pieces for a
//! button of the app's own colour that still reads as the same control.
//!
//! Every binding's `button` lowers to this one function, so none of them
//! can end up with a button of its own; the access rows it admits are
//! `schema::BUTTON_ROWS_JSX`.
//!
//! Run: cargo run -p kui --example button [-- --headless]

use kui::widgets;
use kui::{Align, App, Color, Core, NodeSpec, TextStyle, Ui, UiEvent, Value};
use kui_devtools::{Drive, Example};

#[derive(Default)]
struct Buttons {
    last: Option<String>,
    presses: u32,
}

impl App for Buttons {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(18.0)
                .cross_align(Align::Start),
            |ui| {
                ui.text("`widgets::button` · rest, hover, pressed, and the ring on Tab", TextStyle::new(12.0).color(t.muted));
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    widgets::button(ui, "plain", Value::str("plain"));
                    widgets::button_with(
                        ui,
                        "accent",
                        "accent",
                        widgets::button_spec().accent().on_click(Value::str("accent")),
                        None,
                    );
                    widgets::button_with(
                        ui,
                        "disabled",
                        "disabled",
                        widgets::button_spec().disabled(true).on_click(Value::str("disabled")),
                        None,
                    );
                });

                ui.text("`button_with` · the access rows: a label, a description, a hint", TextStyle::new(12.0).color(t.muted));
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    // The text is a glyph; the name is what a reader says.
                    widgets::button_with(
                        ui,
                        "add",
                        "＋",
                        widgets::button_spec().on_click(Value::str("add")).label("add a row"),
                        None,
                    );
                    // A `description` is what a reader says after the
                    // name; a tooltip hint *is* one, so a button says it
                    // once and both the float and the reader have it.
                    widgets::button_with(
                        ui,
                        "delete",
                        "delete",
                        widgets::button_spec()
                            .on_click(Value::str("delete"))
                            .apply_tooltip("removes the row for good — no undo"),
                        Some("removes the row for good — no undo"),
                    );
                });

                ui.text("`button_spec` + `button_palette` · a button in the app's own colour, still the same control", TextStyle::new(12.0).color(t.muted));
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    for (name, base) in [
                        ("forest", Color::hex(0x2f7d4fff)),
                        ("plum", Color::hex(0x7d2f6bff)),
                        ("sand", Color::hex(0xe0c070ff)),
                    ] {
                        let (rest, hover, pressed) = widgets::button_palette(base);
                        let spec = widgets::button_spec()
                            .bg(rest)
                            .hover_bg(hover)
                            .pressed_bg(pressed)
                            .on_click(Value::str(name));
                        // Readable on the base, whichever it is — the rule
                        // the stock button applies to its own.
                        let fg = widgets::readable_on(rest);
                        ui.with_keyed(name, spec, |ui| {
                            ui.text(name, TextStyle::new(widgets::BUTTON_TEXT).color(fg));
                        });
                    }
                });

                ui.text(
                    &format!(
                        "{} presses · last: {}",
                        self.presses,
                        self.last.as_deref().unwrap_or("nothing")
                    ),
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if let Some(s) = ev.payload.as_str() {
            self.presses += 1;
            self.last = Some(s.to_string());
        }
    }
}

impl Example for Buttons {
    const KEYS: &'static [(&'static str, &'static str)] =
        &[("Tab", "the ring"), ("Enter / Space", "press")];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(560.0, 320.0)
    }

    /// A click by label presses; a disabled button does not, and is not
    /// in the ring; the access rows come back as declared.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 560.0, 320.0);
        d.frame(self);
        let plain = d.key_of("plain").ok_or("no plain button")?;
        d.click_key(self, plain);
        d.check(
            self.last.as_deref() == Some("plain") && self.presses == 1,
            "a click presses",
        )?;
        let disabled = d.key_of("disabled").ok_or("no disabled button")?;
        d.click_key(self, disabled);
        d.check(self.presses == 1, "a disabled button is inert")?;
        let mut ring = Vec::new();
        for _ in 0..8 {
            d.input(
                self,
                kui::InputEvent::Key(kui::EditKey::Tab, Default::default()),
            );
            if let Some(k) = d.core.focus() {
                ring.push(d.core.label_of(k).unwrap_or("?").to_string());
            }
        }
        d.check(!ring.iter().any(|n| n == "disabled"), "and not in the ring")?;
        d.check(
            ring.iter().any(|n| n == "forest"),
            "the app-coloured button is a button like the rest",
        )?;

        let node = |d: &mut Drive<'_>, name: &str| {
            d.core
                .access_tree()
                .nodes
                .iter()
                .find(|n| n.name.as_deref() == Some(name))
                .map(|n| (n.role, n.description.clone()))
        };
        let add = node(&mut d, "add a row");
        d.check(
            add.is_some(),
            "a `label` is the name a reader says, not the glyph",
        )?;
        let delete = node(&mut d, "delete");
        d.check(
            delete.and_then(|(_, desc)| desc).as_deref()
                == Some("removes the row for good — no undo"),
            "and a tooltip hint is the `description` a reader says after the name",
        )
    }
}

kui_devtools::main!(Buttons::default());
