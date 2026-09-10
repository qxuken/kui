//! The counter: the smallest app that is the whole pattern, and the one
//! example every binding has in the same shape (`docs/adr/0021`, decision
//! 3). `view` rebuilds the tree from the model; a click arrives in
//! `on_event` as data and moves the model; nothing else happens.
//!
//! What every counter holds, so the four stay one shape: the count with
//! `+1`, `-1` and `reset`; a `name` field whose text the greeting reads
//! back (`ui.edit_text`); a right-click that declares a `modal` menu on
//! the next frame — the core opens nothing, it reports the press and the
//! view puts a float there — with `+10` and `reset` in it; and a headless
//! drive that clicks all of it and exits non-zero on a wrong answer.
//!
//! Run: cargo run -p kui --example counter [-- --headless]

use kui::widgets;
use kui::{Align, App, Core, FloatConfig, Key, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_harness::{Drive, Example};

#[derive(Default)]
struct Counter {
    count: i64,
    /// Where the menu is, in viewport px, while it is open.
    menu: Option<(f32, f32)>,
    /// The name field's key from the last frame, for the greeting.
    name: Option<Key>,
    greeting: String,
}

impl App for Counter {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // The whole example answers a secondary press. Any node can: the
        // topmost one that declared `on_context_menu` is the one asked, so
        // a row inside could offer its own menu and win over this one.
        ui.with(
            NodeSpec::column()
                .fill()
                .center()
                .gap(24.0)
                .on_context_menu(Value::map([("kind", "menu".into())])),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .pad(32.0)
                        .gap(20.0)
                        .bg(t.surface)
                        .radius(12.0)
                        .border(1.0, t.border)
                        .cross_align(Align::Center)
                        .width(Sizing::Fixed(320.0)),
                    |ui| {
                        ui.text(&self.greeting, TextStyle::new(14.0).color(t.muted));
                        ui.text(&self.count.to_string(), TextStyle::new(56.0));
                        ui.with(NodeSpec::row().gap(12.0), |ui| {
                            widgets::button(ui, "-1", Value::map([("kind", "dec".into())]));
                            widgets::button(ui, "+1", Value::map([("kind", "inc".into())]));
                            widgets::button(ui, "reset", Value::map([("kind", "reset".into())]));
                        });
                        // The field's text is the core's; the view reads
                        // it back rather than keeping a copy.
                        self.name = Some(widgets::text_input(ui, "name", ""));
                    },
                );
                ui.text(
                    "right-click for a menu · clicks are data: view() never sees a callback",
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
        // Declared last: floats stack in declaration order. One `modal`
        // row is the whole difference between this and a plain float —
        // Tab is scoped to it, the pointer cannot reach what is behind
        // it, and Escape or a press outside comes back as `dismiss`.
        if let Some((x, y)) = self.menu {
            ui.with_keyed(
                "menu",
                NodeSpec::column()
                    .float(
                        FloatConfig::viewport()
                            .at(Align::Start, Align::Start)
                            .self_at(Align::Start, Align::Start)
                            .offset(x, y)
                            // Opened near an edge, the menu would hang off
                            // the window; `fit` mirrors it back instead.
                            .fit(),
                    )
                    .modal(Value::str("menu"))
                    .label("Actions")
                    .width(Sizing::Fixed(120.0))
                    .pad(4.0)
                    .gap(4.0)
                    .bg(t.raised)
                    .border(1.0, t.border_strong)
                    .radius(6.0),
                |ui| {
                    widgets::button(ui, "+10", Value::map([("kind", "add10".into())]));
                    widgets::button(ui, "reset", Value::map([("kind", "reset".into())]));
                },
            );
        }
        if let Some(name) = self.name {
            let who = ui.edit_text(name).unwrap_or_default();
            self.greeting = if who.trim().is_empty() {
                "kui counter".into()
            } else {
                format!("{}'s counter", who.trim())
            };
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("inc") => self.count += 1,
            Some("dec") => self.count -= 1,
            Some("reset") => {
                self.count = 0;
                self.menu = None;
            }
            // A right-click: the core reports where it landed and opens
            // nothing. The next frame's view is what puts a menu there.
            Some("contextmenu") => {
                let at = |k| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
                self.menu = Some((at("x"), at("y")));
            }
            // Escape, or a press outside the menu. The core asks; the app
            // decides — this one just closes.
            Some("dismiss") => self.menu = None,
            Some("add10") => {
                self.count += 10;
                self.menu = None;
            }
            _ => {}
        }
    }
}

impl Example for Counter {
    const KEYS: &'static [(&'static str, &'static str)] =
        &[("right-click", "the modal menu"), ("Esc", "dismiss it")];

    fn window(&self) -> kui_harness::Window {
        kui_harness::Window::default().size(560.0, 400.0)
    }

    /// The Rosetta drive: every counter clicks its buttons by label,
    /// types into its field, opens and dismisses its menu, and checks the
    /// model after each.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 560.0, 400.0);
        d.frame(self);
        let inc = d.key_of("+1").ok_or("no +1")?;
        let dec = d.key_of("-1").ok_or("no -1")?;
        let name = d.key_of("name").ok_or("no name field")?;
        d.click_key(self, inc);
        d.click_key(self, inc);
        d.click_key(self, dec);
        d.frame(self);
        d.check(self.count == 1, "two +1 and a -1 count to 1")?;

        // The field: focus it, type, and the greeting reads it back.
        d.focus(self, name);
        d.text(self, "Ada");
        d.frame(self);
        d.frame(self);
        d.check(
            self.greeting == "Ada's counter",
            "the greeting reads the field back",
        )?;

        // The menu: a secondary press anywhere in the example, then the
        // frame that declares the modal, then its +10.
        d.input(
            self,
            kui::InputEvent::CursorMoved(kui::Vec2::new(40.0, 40.0)),
        );
        d.input(
            self,
            kui::InputEvent::MouseDown {
                button: kui::MouseButton::Secondary,
                clicks: 1,
            },
        );
        d.input(
            self,
            kui::InputEvent::MouseUp {
                button: kui::MouseButton::Secondary,
            },
        );
        d.check(self.menu.is_some(), "a right-click asks for the menu")?;
        d.frame(self);
        let add10 = d.key_of("+10").ok_or("the menu did not open")?;
        d.click_key(self, add10);
        d.frame(self);
        d.check(
            self.count == 11 && self.menu.is_none(),
            "+10 counts and closes the menu",
        )?;

        // Escape dismisses a reopened one without choosing.
        self.menu = Some((40.0, 40.0));
        d.frame(self);
        d.key(self, "escape", Default::default());
        d.check(self.menu.is_none(), "escape dismisses the menu")?;
        d.frame(self);
        let gone = d.key_of("+10").is_none();
        d.check(gone, "and the frame after has no menu")
    }
}

kui_harness::main!(Counter::default());
