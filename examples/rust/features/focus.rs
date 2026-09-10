//! Keyboard focus as data (`docs/adr/0002-keyboard-focus-as-data.md`).
//! One focus for the window; Tab walks every control in tree order and
//! wraps; Shift-Tab walks back; Enter and Space press the focused button;
//! the ring the core draws shows for keyboard focus and not for a click
//! (`focus_visible`), or the node's own `focus_bg` replaces it.
//!
//! Who is in the ring: a button, an editor, a key sink, a control role,
//! and a plain box declaring `focusable` — and not a `disabled` one, a
//! `role = none` one, or the root. A `modal` scope confines the ring while
//! it is up and its `initial_focus` says where it opens (`apps/counter`'s
//! menu is one). The buttons at the bottom are the verbs — `focus`,
//! `blur`, `focus_next` — for a view that wants to move it itself.
//!
//! The dock's `focus` row reads the same fact this page draws.
//!
//! Run: cargo run -p kui --example focus [-- --headless]

use kui::widgets;
use kui::{Align, App, Core, Key, NodeSpec, Role, TextStyle, Ui, UiEvent, Value};
use kui_harness::{Drive, Example};

#[derive(Default)]
struct Focus {
    pressed: Option<String>,
    /// A verb the buttons asked for, applied on the next view.
    verb: Option<&'static str>,
    field: Option<Key>,
}

impl App for Focus {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // The verbs move focus from the view: `focus` by key, `blur`, and
        // `focus_next` from wherever it is.
        match self.verb.take() {
            Some("field") => {
                if let Some(k) = self.field {
                    ui.focus(k);
                }
            }
            Some("blur") => ui.blur(),
            Some("next") => ui.focus_next(),
            _ => {}
        }
        let focused = ui.focused();
        let visible = ui.focus_visible();
        let name = |ui: &mut Ui<'_>| -> String {
            focused
                .and_then(|k| ui.core().label_of(k).map(str::to_string))
                .unwrap_or_else(|| "nothing".into())
        };
        let readout = format!(
            "focus: {}{}",
            name(ui),
            if visible {
                " · ring shown (keyboard put it there)"
            } else {
                ""
            }
        );
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(16.0)
                .cross_align(Align::Start),
            |ui| {
                ui.text(
                    "Tab walks the ring in tree order; Enter or Space presses",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(NodeSpec::row().gap(10.0).cross_align(Align::Center), |ui| {
                    widgets::button(ui, "one", Value::str("one"));
                    widgets::button(ui, "two", Value::str("two"));
                    // Disabled: drawn dimmed, inert, and not a stop.
                    widgets::button_with(
                        ui,
                        "three",
                        "three (disabled)",
                        widgets::button_spec()
                            .disabled(true)
                            .on_click(Value::str("three")),
                        None,
                    );
                    widgets::button(ui, "four", Value::str("four"));
                });

                ui.text(
                    "an editor, a switch, a focusable row — all stops",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(NodeSpec::row().gap(10.0).cross_align(Align::Center), |ui| {
                    self.field = Some(widgets::text_input(ui, "field", ""));
                    let on = self.pressed.as_deref() == Some("mute");
                    ui.with_keyed(
                        "mute",
                        NodeSpec::row()
                            .pad_xy(12.0, 8.0)
                            .radius(8.0)
                            .bg(if on { t.accent_soft } else { t.raised })
                            .border(1.0, t.border)
                            .role(Role::Switch)
                            .label("mute")
                            .checked(on)
                            .on_click(Value::str("mute")),
                        |ui| {
                            ui.text(
                                if on { "mute: on" } else { "mute: off" },
                                TextStyle::new(13.0),
                            );
                        },
                    );
                    // A plain row, in the ring because it says so, with a
                    // `focus_bg` instead of the ring.
                    ui.with_keyed(
                        "row",
                        NodeSpec::row()
                            .pad_xy(12.0, 8.0)
                            .radius(8.0)
                            .bg(t.surface)
                            .border(1.0, t.border)
                            .focusable()
                            .focus_bg(t.accent_soft)
                            .on_click(Value::str("row")),
                        |ui| {
                            ui.text("focusable row · focus_bg", TextStyle::new(13.0));
                        },
                    );
                    // Decoration: `role = none` keeps it out of the ring
                    // and the access tree, `on_click` or not.
                    ui.with_keyed(
                        "deco",
                        NodeSpec::row()
                            .pad_xy(12.0, 8.0)
                            .radius(8.0)
                            .bg(t.sunken)
                            .role(Role::None)
                            .on_click(Value::str("deco")),
                        |ui| {
                            ui.text("role: none — skipped", TextStyle::new(13.0).color(t.muted));
                        },
                    );
                });

                ui.text(
                    "the verbs: focus moved from the view",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    widgets::button(ui, "focus the field", Value::str("verb:field"));
                    widgets::button(ui, "focus_next", Value::str("verb:next"));
                    widgets::button(ui, "blur", Value::str("verb:blur"));
                });

                ui.text(&readout, TextStyle::new(13.0).color(t.accent));
                ui.text(
                    &format!(
                        "last pressed: {}",
                        self.pressed.as_deref().unwrap_or("nothing")
                    ),
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.as_str() {
            Some("verb:field") => self.verb = Some("field"),
            Some("verb:next") => self.verb = Some("next"),
            Some("verb:blur") => self.verb = Some("blur"),
            Some(s) => self.pressed = Some(s.to_string()),
            None => {}
        }
    }
}

impl Example for Focus {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("Tab / ⇧Tab", "walk the ring"),
        ("Enter / Space", "press the focused control"),
    ];

    fn window(&self) -> kui_harness::Window {
        kui_harness::Window::default().size(760.0, 360.0)
    }

    /// Tab walks one, two, four (three is disabled), the field, the
    /// switch, the row, the three verbs, and wraps; Enter presses; a
    /// click focuses without the ring showing.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 760.0, 360.0);
        d.frame(self);
        let tab = |d: &mut Drive<'_>, app: &mut Focus, back: bool| {
            d.input(
                app,
                kui::InputEvent::Key(
                    kui::EditKey::Tab,
                    kui::Mods {
                        shift: back,
                        ..Default::default()
                    },
                ),
            );
        };
        let label = |d: &Drive<'_>| -> String {
            d.core
                .focus()
                .and_then(|k| d.core.label_of(k).map(str::to_string))
                .unwrap_or_default()
        };
        let mut walked = Vec::new();
        for _ in 0..9 {
            tab(&mut d, self, false);
            walked.push(label(&d));
        }
        d.check(
            walked
                == [
                    "one",
                    "two",
                    "four",
                    "field",
                    "mute",
                    "row",
                    "focus the field",
                    "focus_next",
                    "blur",
                ],
            "Tab walks every control in tree order and skips the disabled and the decorative",
        )?;
        d.check(d.core.focus_visible(), "and the ring shows after a Tab")?;
        tab(&mut d, self, false);
        d.check(label(&d) == "one", "and wraps")?;
        tab(&mut d, self, true);
        d.check(label(&d) == "blur", "Shift-Tab walks back")?;

        tab(&mut d, self, false);
        d.key(self, "enter", Default::default());
        d.check(
            self.pressed.as_deref() == Some("one"),
            "Enter presses the focused button",
        )?;

        let two = d.key_of("two").ok_or("no two")?;
        d.click_key(self, two);
        d.frame(self);
        let mute = d.key_of("mute").ok_or("no switch")?;
        let r = d.rect_of(mute).ok_or("no switch rect")?;
        d.click(self, r.x + r.w / 2.0, r.y + r.h / 2.0);
        d.frame(self);
        d.check(
            label(&d) == "mute" && !d.core.focus_visible(),
            "a click focuses, and the ring does not show",
        )?;

        let verb = d.key_of("focus the field").ok_or("no verb")?;
        d.click_key(self, verb);
        d.frame(self);
        d.check(label(&d) == "field", "the view's `focus` verb moves it")?;
        let blur = d.key_of("blur").ok_or("no blur")?;
        d.click_key(self, blur);
        d.frame(self);
        d.check(d.core.focus().is_none(), "and `blur` clears it")
    }
}

kui_harness::main!(Focus::default());
