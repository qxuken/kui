//! `modal` (`docs/adr/0003-modal-surfaces.md`): a dialog over a form, and
//! a confirm over the dialog — the two things a page of its own can show
//! that a menu cannot.
//!
//! - **Entry.** The dialog opens with focus on its *Save* button, because
//!   that button declares `initial_focus`; without it focus would land on
//!   the first stop, the field. The ring is the dialog: Tab wraps inside
//!   it and never reaches the form.
//! - **Nesting.** *Discard* opens a confirm *inside* the dialog. The last
//!   `modal` declared wins, so the dialog goes as inert as the form under
//!   it — a stack without a stack (decision 2).
//! - **Escape is the app's call.** On the confirm it closes the confirm;
//!   on the dialog it closes the dialog when the field is untouched and
//!   asks the confirm otherwise (decision 6: the core asks, the app
//!   decides).
//! - **Restore.** When the dialog goes, focus returns to the button that
//!   opened it (decision 4), so a keyboard user is where they were.
//!
//! Run: cargo run -p kui --example modal [-- --headless]

use kui::widgets;
use kui::{Align, App, Core, FloatConfig, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};
use kui_devtools::{Drive, Example};

#[derive(Default)]
struct Page {
    dialog: bool,
    confirm: bool,
    /// What the last dialog saved, shown on the form.
    saved: Option<String>,
    /// The dialog field's text, read off the editor every frame (a
    /// `changed` event says *that* it changed; the text is the core's to
    /// read), which is what "untouched" is judged by.
    draft: String,
}

impl App for Page {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column().fill().pad(24.0).gap(14.0).bg(t.bg),
            |ui| {
                ui.text("modal", TextStyle::new(22.0).color(t.fg));
                ui.text(
                    "the form is the main ring; a dialog confines it, a confirm over the dialog confines it again",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with(NodeSpec::row().gap(10.0), |ui| {
                    widgets::button(ui, "open dialog", Value::str("open"));
                    widgets::button(ui, "another button", Value::str("noop"));
                });
                ui.with(NodeSpec::column().width(Sizing::Fixed(260.0)), |ui| {
                    widgets::text_input(ui, "form field", "");
                });
                if let Some(s) = &self.saved {
                    ui.text(&format!("saved: {s:?}"), TextStyle::new(12.0).color(t.muted));
                }
            },
        );
        if self.dialog {
            let mut draft = None;
            ui.with_keyed(
                "dialog",
                NodeSpec::column()
                    .float(
                        FloatConfig::viewport()
                            .at(Align::Center, Align::Center)
                            .self_at(Align::Center, Align::Center),
                    )
                    .modal(Value::str("dialog"))
                    .role(kui::Role::Dialog)
                    .label("Edit")
                    .width(Sizing::Fixed(320.0))
                    .pad(18.0)
                    .gap(12.0)
                    .bg(t.raised)
                    .border(1.0, t.border_strong)
                    .radius(10.0),
                |ui| {
                    ui.text("edit", TextStyle::new(16.0).color(t.fg));
                    let name = widgets::text_input(ui, "name", "");
                    draft = ui.edit_text(name);
                    ui.with(
                        NodeSpec::row()
                            .width(Sizing::Grow(1.0))
                            .gap(8.0)
                            .main_align(Align::End),
                        |ui| {
                            widgets::button(ui, "discard", Value::str("discard"));
                            // `initial_focus`: the dialog opens here, not on the
                            // field — the entry precedence of ADR 0003 / 0007.
                            let spec = widgets::button_spec(&ui.metrics())
                                .accent()
                                .on_click(Value::str("save"))
                                .initial_focus();
                            widgets::button_with(ui, "save", "save", spec, None);
                        },
                    );
                    // The confirm, declared inside the dialog: the later
                    // `modal` is the one in effect.
                    if self.confirm {
                        ui.with_keyed(
                            "confirm",
                            NodeSpec::column()
                                .float(
                                    FloatConfig::viewport()
                                        .at(Align::Center, Align::Center)
                                        .self_at(Align::Center, Align::Center),
                                )
                                .modal(Value::str("confirm"))
                                .role(kui::Role::Dialog)
                                .label("Discard changes?")
                                .width(Sizing::Fixed(240.0))
                                .pad(16.0)
                                .gap(10.0)
                                .bg(t.raised)
                                .border(1.0, t.danger)
                                .radius(8.0),
                            |ui| {
                                ui.text("discard changes?", TextStyle::new(14.0).color(t.fg));
                                ui.with(
                                    NodeSpec::row()
                                        .width(Sizing::Grow(1.0))
                                        .gap(8.0)
                                        .main_align(Align::End),
                                    |ui| {
                                        widgets::button(ui, "keep editing", Value::str("keep"));
                                        widgets::button(ui, "discard them", Value::str("really"));
                                    },
                                );
                            },
                        );
                    }
                },
            );
            if let Some(text) = draft {
                self.draft = text;
            }
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        let kind = ev.payload.get("kind").and_then(Value::as_str);
        if kind == Some("dismiss") {
            // Which modal asked: the tag is the node's.
            match ev.payload.get("tag").and_then(Value::as_str) {
                Some("confirm") => self.confirm = false,
                Some("dialog") if self.draft.is_empty() => self.dialog = false,
                Some("dialog") => self.confirm = true,
                _ => {}
            }
            return;
        }
        match ev.payload.as_str() {
            Some("open") => {
                self.dialog = true;
                self.confirm = false;
                self.draft.clear();
            }
            Some("save") => {
                self.saved = Some(self.draft.clone());
                self.dialog = false;
            }
            Some("discard") => self.confirm = true,
            Some("keep") => self.confirm = false,
            Some("really") => {
                self.confirm = false;
                self.dialog = false;
            }
            _ => {}
        }
    }
}

impl Example for Page {
    /// Entry on `initial_focus`, the ring confined, the confirm over the
    /// dialog making it inert, Escape routed by tag, and focus restored to
    /// the opener.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 700.0, 500.0);
        d.frame(self);
        let open = d.key_of("open dialog").unwrap();
        let other = d.key_of("another button").unwrap();
        d.focus(self, open);
        d.frame(self);
        d.click_key(self, open);
        d.frame(self);
        let save = d.key_of("save").unwrap();
        let discard = d.key_of("discard").unwrap();
        let name = d.key_of("name").unwrap();
        d.check(
            d.core.focus() == Some(save),
            "the dialog opens on save (initial_focus)",
        )?;
        // Tab wraps inside the dialog: name → discard → save → name.
        d.key(self, "tab", Default::default());
        d.check(d.core.focus() == Some(name), "Tab wraps to the field")?;
        d.key(self, "tab", Default::default());
        d.check(d.core.focus() == Some(discard), "then discard")?;
        d.key(self, "tab", Default::default());
        d.check(
            d.core.focus() == Some(save),
            "then save again, never the form",
        )?;
        // The form is inert: a click on its other button reaches nobody.
        let evs = d.click_key(self, other);
        d.check(evs.is_empty(), "the form under the dialog is inert")?;
        // Escape on an untouched dialog closes it, and focus comes back
        // to the button that opened it.
        d.key(self, "escape", Default::default());
        d.frame(self);
        d.check(!self.dialog, "Escape closes an untouched dialog")?;
        d.check(d.core.focus() == Some(open), "focus is back on the opener")?;
        // Open again, type, then Escape asks the confirm instead.
        d.click_key(self, open);
        d.frame(self);
        let name = d.key_of("name").unwrap();
        d.focus(self, name);
        d.text(self, "hi");
        d.frame(self);
        d.key(self, "escape", Default::default());
        d.frame(self);
        d.check(
            self.dialog && self.confirm,
            "Escape on a dirty dialog asks the confirm",
        )?;
        let keep = d.key_of("keep editing").unwrap();
        let really = d.key_of("discard them").unwrap();
        d.check(
            d.core.focus() == Some(keep),
            "the confirm opens on its first stop, and holds focus",
        )?;
        // The dialog is now as inert as the form: its save button reaches nobody.
        let save = d.key_of("save").unwrap();
        let inert = d.click_key(self, save).is_empty();
        d.check(inert, "the dialog under the confirm is inert")?;
        d.key(self, "escape", Default::default());
        d.frame(self);
        d.check(
            self.dialog && !self.confirm,
            "Escape on the confirm closes only it",
        )?;
        d.check(
            d.core.focus() == Some(name),
            "and focus returns to the field the confirm displaced",
        )?;
        // Discard for real: both go, focus back on the opener.
        d.click_key(self, discard);
        d.frame(self);
        d.click_key(self, really);
        d.frame(self);
        d.check(!self.dialog && !self.confirm, "discarding closes both")?;
        d.check(
            d.core.focus() == Some(open),
            "focus is back on the opener again",
        )?;
        Ok(())
    }
}

kui_devtools::main!(Page::default());
