//! The application menu bar (`docs/adr/0018-a-menu-bar-the-app-declares.md`).
//! The frame declares it, in one call — `widgets::menu_bar` says what it
//! is and where its strip goes when it has to be drawn — and the host
//! decides who shows it: on macOS the same declaration goes to the OS
//! and is the bar at the top of the screen, and the strip in the window
//! is not there; everywhere else the core draws the strip. The dock's
//! `menus` row switches between the two on a host that has both.
//!
//! Its rows are the rows a context menu has, so View ▸ Wrap checks itself
//! (`checked`, rebuilt from the model every frame with nothing retained),
//! View ▸ Clear is `enabled` only when there is something to clear, Edit
//! ▸ Copy is the same Copy the core performs itself, and every choice —
//! standard or the app's own — arrives as the same one `menu` event.
//!
//! Run: cargo run -p kui --example menu_bar [-- --headless]

use kui::widgets;
use kui::{
    Align, App, BarMenu, Core, MenuBar, MenuItem, MenuRole, NodeSpec, Sizing, Span, TextStyle,
    TextWrap, Ui, UiEvent, Value,
};
use kui_harness::{Drive, Example};

#[derive(Default)]
struct Demo {
    /// A setting one of the bar's rows toggles: a `checked` row doing
    /// what a checked row is for.
    wrap: bool,
    /// Rows "About" was chosen, for a row that is plain.
    abouts: u32,
    /// Something to clear, for a row that is `enabled` conditionally.
    notes: Vec<String>,
    /// The last thing the bar reported, shown at the bottom.
    last: Option<String>,
}

impl Demo {
    /// The bar, rebuilt every frame from the model. Every row is an
    /// ordinary `MenuItem`: the app's own with `id`s, a `checked`
    /// setting, and the standard Edit rows the core performs itself.
    fn menu(&self) -> MenuBar {
        let mine = |what: &str| Value::map([("do", Value::str(what))]);
        MenuBar::new(vec![
            // First, which on macOS is the position the OS titles with the
            // app's own name whatever this label says.
            BarMenu::new(
                "Demo",
                vec![
                    MenuItem::new("About this example").id(mine("about")),
                    MenuItem::separator(),
                    MenuItem::new("Add a note").id(mine("note")).accel("mod+n"),
                ],
            ),
            BarMenu::new(
                "Edit",
                vec![
                    MenuItem::role(MenuRole::Cut),
                    MenuItem::role(MenuRole::Copy),
                    MenuItem::role(MenuRole::Paste),
                    MenuItem::separator(),
                    MenuItem::role(MenuRole::SelectAll),
                ],
            ),
            BarMenu::new(
                "View",
                vec![
                    MenuItem::new("Wrap the paragraph")
                        .id(mine("wrap"))
                        .accel("mod+shift+w")
                        .checked(self.wrap),
                    MenuItem::new("Clear the notes")
                        .id(mine("clear"))
                        .enabled(!self.notes.is_empty()),
                ],
            ),
        ])
    }
}

impl App for Demo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().bg(t.bg), |ui| {
            // The application menu: full width and flush with the top,
            // because that is where a menu bar goes — the padding the rest
            // of the page has moves inside. On macOS this draws nothing.
            widgets::menu_bar(ui, self.menu());
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .pad(24.0)
                    .gap(12.0)
                    .cross_align(Align::Center),
                |ui| {
                    ui.with(
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .max_width(560.0)
                            .pad(20.0)
                            .gap(10.0)
                            .bg(t.surface)
                            .radius(10.0)
                            .border(1.0, t.border),
                        |ui| {
                            ui.text(
                                "View ▸ Wrap the paragraph",
                                TextStyle::new(12.0).color(t.muted),
                            );
                            ui.rich_text(
                                &[
                                    Span::new("A paragraph whose wrapping is a "),
                                    Span::new("checked").bold(),
                                    Span::new(
                                        " row of the bar: the row reads the model, the model \
                                         reads the row, and nothing in between remembers \
                                         anything — the bar is rebuilt from `wrap` every frame.",
                                    ),
                                ],
                                TextStyle::new(15.0).line_height(22.0).wrap(if self.wrap {
                                    TextWrap::Word
                                } else {
                                    TextWrap::None
                                }),
                            );
                            ui.text(
                                &format!(
                                    "About chosen {} time{} · {} note{}",
                                    self.abouts,
                                    if self.abouts == 1 { "" } else { "s" },
                                    self.notes.len(),
                                    if self.notes.len() == 1 { "" } else { "s" },
                                ),
                                TextStyle::new(13.0).color(t.muted),
                            );
                            for note in &self.notes {
                                ui.text(note, TextStyle::new(13.0));
                            }
                        },
                    );
                    ui.with(
                        NodeSpec::row()
                            .width(Sizing::Grow(1.0))
                            .max_width(560.0)
                            .gap(8.0),
                        |ui| {
                            ui.text("last menu event —", TextStyle::new(12.0).color(t.muted));
                            ui.text(
                                self.last
                                    .as_deref()
                                    .unwrap_or("choose something from the bar"),
                                TextStyle::new(12.0).color(t.accent),
                            );
                        },
                    );
                },
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.get("kind").and_then(Value::as_str) != Some("menu") {
            return;
        }
        // Every chosen row, standard or not, arrives here with the role it
        // played; a custom row hands back whatever its `id` carried.
        let role = ev.payload.get("role").and_then(Value::as_str).unwrap_or("");
        let item = ev.payload.get("item").cloned().unwrap_or(Value::Null);
        let did = item.get("do").and_then(Value::as_str);
        match did {
            Some("about") => self.abouts += 1,
            Some("note") => self.notes.push(format!("note {}", self.notes.len() + 1)),
            Some("wrap") => self.wrap = !self.wrap,
            Some("clear") => self.notes.clear(),
            _ => {}
        }
        self.last = Some(match did {
            Some(d) => d.to_string(),
            None => role.to_string(),
        });
    }
}

impl Example for Demo {
    const KEYS: &'static [(&'static str, &'static str)] =
        &[("⌘N", "Demo ▸ Add a note"), ("⌘⇧W", "View ▸ Wrap")];

    fn window(&self) -> kui_harness::Window {
        kui_harness::Window::default().size(640.0, 360.0)
    }

    /// Drawn menus, so the strip is in the frame on every host — which
    /// is what makes the drive below the same everywhere.
    fn native_menus(&self) -> Option<bool> {
        Some(false)
    }

    /// The bar as data: the declaration the frame made, read back from
    /// the core, and one of its rows chosen.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 640.0, 360.0);
        d.core.set_native_menus(false);
        d.frame(self);
        let bar = d.core.menu_bar().ok_or("the frame declared no menu bar")?;
        d.check(bar.menus.len() == 3, "the bar has its three menus")?;
        let view = &bar.menus[2];
        d.check(
            view.items.iter().all(|i| !i.checked),
            "Wrap is a checked row, unchecked to start",
        )?;
        d.check(
            view.items.iter().any(|i| !i.enabled),
            "Clear is disabled with nothing to clear",
        )?;

        // Choosing a row, both ways. The drawn bar: open View (what a
        // press on its title does) and click its first row, keyed by
        // index under the panel. The platform's bar reports the same
        // choice as `activate_menu_bar_item`, and both land in the same
        // `menu` event — with the row checked on the frame after.
        d.core.set_menu_bar_open(Some(2));
        d.frame(self);
        let panel = d
            .key_of("kui.menubar.menu")
            .ok_or("the View menu did not open")?;
        d.click_key(self, panel.index(0));
        d.frame(self);
        d.check(
            self.wrap,
            "a click on the drawn row arrives as a `menu` event",
        )?;
        d.check(
            self.last.as_deref() == Some("wrap"),
            "with the row's own id",
        )?;
        let bar = d.core.menu_bar().ok_or("the bar went away")?;
        d.check(
            bar.menus[2].items.iter().any(|i| i.checked),
            "and the row is checked on the next frame",
        )?;
        let evs = d.core.activate_menu_bar_item(2, 0);
        for ev in evs {
            self.on_event(ev);
        }
        d.check(
            !self.wrap,
            "the platform's report of the same row is the same event",
        )
    }
}

kui_harness::main!(Demo::default());
