//! The select: a field that shows the choice in force and, clicked, drops
//! the core's own menu of the choices with the current one checked
//! (`widgets::select`). The app holds no open state — the menu is the
//! same one a right-click opens, drawn in the frame or shown by the
//! platform, dismissed by Escape or a press outside, walked by the
//! arrows — and hears one event: the choice, as the `menu` event a menu
//! row posts, on the field's key. Drawing the field again with the new
//! `current` is the whole loop.
//!
//! Two fields: a `select` over plain labels, whose choice posts the label,
//! and a `select_items` over `MenuItem`s, whose choice posts each item's
//! `id` — here the size in points, so the app parses nothing.
//!
//! Run: cargo run -p kui-native --example select [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets;
use kui_native::{Align, App, Core, MenuItem, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value};

const LANGUAGES: [&str; 4] = ["English", "Deutsch", "Français", "日本語"];
const SIZES: [u32; 4] = [11, 13, 15, 18];

struct Select {
    language: usize,
    size: usize,
    /// What the last choice posted, verbatim: the whole of what an app sees.
    last: Option<Value>,
}

impl Default for Select {
    fn default() -> Self {
        Self {
            language: 0,
            size: 1,
            last: None,
        }
    }
}

impl App for Select {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let row = |ui: &mut Ui<'_>, label: &str, f: &mut dyn FnMut(&mut Ui<'_>)| {
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .gap(12.0)
                    .cross_align(Align::Center),
                |ui| {
                    ui.with(NodeSpec::row().width(Sizing::Fixed(90.0)), |ui| {
                        ui.text(label, TextStyle::new(13.0).color(t.muted));
                    });
                    f(ui);
                },
            );
        };
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(14.0)
                .cross_align(Align::Start),
            |ui| {
                ui.text(
                    "`select`: the choice posts its label",
                    TextStyle::new(12.0).color(t.muted),
                );
                row(ui, "language", &mut |ui| {
                    widgets::select(ui, "language", &LANGUAGES, Some(self.language));
                });
                ui.text(
                    "`select_items`: the choice posts the item's `id` — the size itself",
                    TextStyle::new(12.0).color(t.muted),
                );
                row(ui, "size", &mut |ui| {
                    let items: Vec<MenuItem> = SIZES
                        .iter()
                        .map(|s| MenuItem::new(format!("{s} pt")).id(Value::Int(i64::from(*s))))
                        .collect();
                    widgets::select_items(ui, "size", &items, Some(self.size));
                });
                ui.leaf(NodeSpec::column().height(Sizing::Fixed(10.0)));
                ui.text(
                    &format!("{} at {} pt", LANGUAGES[self.language], SIZES[self.size]),
                    TextStyle::new(SIZES[self.size] as f32),
                );
                let last = match &self.last {
                    Some(v) => format!("last event: {v:?}"),
                    None => {
                        "no choice yet — click a field, or Tab to it and press Space".to_string()
                    }
                };
                ui.text(&last, TextStyle::new(12.0).color(t.faint).mono());
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        // `{kind: "menu", role: "custom", item: …}` on the field's key.
        if ev.payload.get("kind").and_then(Value::as_str) != Some("menu") {
            return;
        }
        let Some(item) = ev.payload.get("item") else {
            return;
        };
        self.last = Some(item.clone());
        if let Some(name) = item.as_str()
            && let Some(i) = LANGUAGES.iter().position(|l| *l == name)
        {
            self.language = i;
        }
        if let Some(pt) = item.as_int()
            && let Some(i) = SIZES.iter().position(|s| i64::from(*s) == pt)
        {
            self.size = i;
        }
    }
}

impl Example for Select {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(520.0, 300.0)
    }

    /// The drawn menu, so the rows are in the frame to click.
    fn native_menus(&self) -> Option<bool> {
        Some(false)
    }

    /// A click on the field opens its menu under it and reaches the app
    /// as nothing; a row chosen is one event naming the option, and the
    /// field then shows it.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 520.0, 300.0);
        d.frame(self);
        let field = d.key_of("language").ok_or("no language field")?;
        let rect = d.rect_of(field).ok_or("the field is not in the hit list")?;
        let evs = d.click_key(self, field);
        d.check(evs.is_empty(), "the field's click never reaches the app")?;
        let menu = d.core.menu().ok_or("no menu opened")?;
        d.check(menu.target == field, "the menu is about the field")?;
        d.check(
            menu.at.y == rect.y + rect.h && menu.at.x == rect.x,
            "and hangs under it",
        )?;
        d.check(menu.items[0].checked, "the current choice is checked")?;
        d.frame(self);
        let row = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == kui_native::Role::MenuItem && n.name.as_deref() == Some("Deutsch"))
            .map(|n| n.rect)
            .ok_or("the rows are not drawn")?;
        let evs = d.click(self, row.x + row.w / 2.0, row.y + row.h / 2.0);
        d.check(
            evs.len() == 1 && evs[0].key == field,
            "one event, on the field",
        )?;
        d.check(self.language == 1, "the app took the choice")?;
        d.check(d.core.menu().is_none(), "and the menu closed")?;
        d.frame(self);
        let shown = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == kui_native::Role::Button && n.name.as_deref() == Some("language"))
            .and_then(|n| n.description.clone());
        d.check(
            shown.as_deref() == Some("Deutsch"),
            "the field describes itself by the new choice",
        )?;
        // The item form: the id, not the label.
        let size = d.key_of("size").ok_or("no size field")?;
        d.click_key(self, size);
        d.frame(self);
        let row = d
            .core
            .access_tree()
            .nodes
            .iter()
            .find(|n| n.role == kui_native::Role::MenuItem && n.name.as_deref() == Some("18 pt"))
            .map(|n| n.rect)
            .ok_or("the size rows are not drawn")?;
        d.click(self, row.x + row.w / 2.0, row.y + row.h / 2.0);
        d.check(
            self.last == Some(Value::Int(18)),
            "the choice posted the item's id",
        )?;
        d.check(self.size == 3, "and the app took it")
    }
}

kui_devtools::main!(Select::default());
