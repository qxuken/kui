//! Context menus: who gets one, and what arrives when a row is chosen
//! (`docs/adr/0017-selection-as-a-scope.md`, decision 5). Four things to
//! right-click, each a different rule:
//!
//!   * **The article.** Nothing declares a menu there, so the core offers
//!     the standard one for a `selectable` scope — Copy, and Select All,
//!     with Copy dimmed until something is selected.
//!   * **A row of the list.** The row declares `on_context_menu`, so the
//!     app's own menu wins: the standard items *and* two of the app's,
//!     opened with `Ui::open_menu` on the next frame (an `on_event` has no
//!     `Ui`). Whichever is chosen arrives as one `menu` event on the row.
//!   * **The field.** An editor gets Cut / Copy / Paste / Select All
//!     without asking for anything.
//!   * **The footer.** A plain box: no menu, because a right-click on
//!     nothing has never opened one.
//!
//! On macOS all of them are the platform's own `NSMenu`, because the
//! runner says it can draw one; everywhere else the core draws the same
//! items itself — the dock's `menus` row switches between the two. The
//! app's code is identical either way, which is the point of the items
//! being data.
//!
//! Run: cargo run -p kui --example context_menu [-- --headless]

use kui::{
    Align, App, Core, EditOptions, Key, Menu, MenuItem, MenuRole, NodeSpec, Sizing, Span,
    TextStyle, Theme, Ui, UiEvent, Value, Vec2,
};
use kui_devtools::{Drive, Example};

const ROWS: [&str; 4] = ["alpha", "bravo", "charlie", "delta"];

#[derive(Default)]
struct Demo {
    /// The last thing a menu reported, shown at the bottom — the whole
    /// point of the items being data is that this is an ordinary event.
    last: Option<String>,
    /// Rows the app's own menu archived, to show a custom item doing
    /// something the core could not have done for it.
    archived: Vec<String>,
    /// A menu the app was asked for and has not opened yet. `on_event`
    /// has no `Ui` — an app changes its model there and builds from it —
    /// so the request waits one frame, which is the frame it opens in.
    pending: Option<(Key, Vec2, String)>,
}

impl App for Demo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // A menu the last frame's right-click asked for. Opened here
        // because this is where a `Ui` is; the frame after this one draws
        // it (or the platform shows it, on a host with menus of its own).
        if let Some((target, at, row)) = self.pending.take() {
            ui.open_menu(Menu::new(target, at, self.row_menu(&row)));
        }
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(28.0)
                .gap(16.0)
                .cross_align(Align::Center)
                .scroll_y(),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .max_width(620.0)
                        .gap(16.0),
                    |ui| {
                        self.article(ui, &t);
                        self.list(ui, &t);
                        self.field(ui, &t);
                        self.footer(ui, &t);
                    },
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            // Every chosen row, standard or not, arrives here — on the
            // node the menu was about, with the role it played.
            Some("menu") => {
                let role = ev.payload.get("role").and_then(Value::as_str).unwrap_or("");
                let item = ev.payload.get("item").cloned().unwrap_or(Value::Null);
                // A custom row says what it is *and* what it is about: the
                // core hands back whatever the item carried, so a menu
                // needs no lookup from the key to the thing.
                let did = item.get("do").and_then(Value::as_str);
                let row = item.get("row").and_then(Value::as_str).unwrap_or("");
                if did == Some("archive") && !self.archived.iter().any(|a| a == row) {
                    self.archived.push(row.to_string());
                }
                self.last = Some(match did {
                    Some(d) => format!("{d} {row}"),
                    None => format!("{role}{}", if role == "custom" { " item" } else { "" }),
                });
            }
            // A row asked to own its menu, so the app opens one: the
            // standard items it wants, plus its own two.
            Some("contextmenu") => {
                let at = Vec2::new(num(&ev, "x"), num(&ev, "y"));
                let tag = ev
                    .payload
                    .get("tag")
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                self.pending = Some((ev.key, at, tag));
            }
            _ => {}
        }
    }
}

impl Demo {
    fn article(&mut self, ui: &mut Ui<'_>, t: &Theme) {
        ui.with_keyed("article", card(t).selectable().pad(20.0).gap(10.0), |ui| {
            ui.text("Selectable article", TextStyle::new(20.0).color(t.fg));
            ui.rich_text(
                &[
                    Span::new("Right-click for the standard menu; nothing here declares one, so "),
                    Span::new("the core offers what it can do")
                        .bold()
                        .color(t.accent),
                    Span::new(
                        ": Copy, and Select All — Copy dimmed until a drag selects something.",
                    ),
                ],
                TextStyle::new(15.0).line_height(24.0).color(t.fg),
            );
        });
    }

    fn list(&mut self, ui: &mut Ui<'_>, t: &Theme) {
        ui.with(card(t).pad(12.0).gap(2.0), |ui| {
            ui.text(
                "Rows with a menu of their own",
                TextStyle::new(13.0).color(t.muted),
            );
            for name in ROWS {
                let archived = self.archived.iter().any(|a| a == name);
                ui.with_keyed(
                    name,
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .pad_xy(10.0, 8.0)
                        .radius(6.0)
                        .hover_bg(t.hover)
                        // The declaration that wins: the core stands back
                        // and this reaches `on_event` instead.
                        .on_context_menu(Value::str(name)),
                    |ui| {
                        let style =
                            TextStyle::new(14.0).color(if archived { t.muted } else { t.fg });
                        ui.text(name, style);
                        if archived {
                            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                            ui.text("archived", TextStyle::new(12.0).color(t.muted));
                        }
                    },
                );
            }
        });
    }

    fn field(&mut self, ui: &mut Ui<'_>, t: &Theme) {
        ui.with(card(t).pad(12.0).gap(8.0), |ui| {
            ui.text(
                "An editor gets the four a field has",
                TextStyle::new(13.0).color(t.muted),
            );
            ui.text_edit(
                "note",
                "Cut, Copy, Paste, Select All — none of it declared.",
                &EditOptions::default(),
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .pad(10.0)
                    .radius(6.0)
                    .bg(t.bg)
                    .border(1.0, t.border)
                    .label("note"),
            );
        });
    }

    fn footer(&mut self, ui: &mut Ui<'_>, t: &Theme) {
        let said = self
            .last
            .clone()
            .unwrap_or_else(|| "right-click anything above".into());
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .pad_xy(4.0, 2.0)
                .gap(8.0),
            |ui| {
                ui.text("last menu event —", TextStyle::new(12.0).color(t.muted));
                ui.text(&said, TextStyle::new(12.0).color(t.accent));
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                ui.text(
                    "a plain box: right-click here opens nothing",
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
    }

    /// The menu one row gets: the standard items worth having on it, and
    /// two the app invented. A custom row carries its own payload, which
    /// is what comes back in the event — so nothing here has to work out
    /// afterwards which row the menu was about.
    fn row_menu(&self, row: &str) -> Vec<MenuItem> {
        let about = |what: &str| Value::map([("do", Value::str(what)), ("row", Value::str(row))]);
        let archived = self.archived.iter().any(|a| a == row);
        vec![
            MenuItem::new("Copy name").id(about("copyname")),
            MenuItem::new(if archived {
                "Already archived"
            } else {
                "Archive"
            })
            .id(about("archive"))
            .enabled(!archived),
            MenuItem::separator(),
            // The standard ones still act: the core performs Select All
            // itself and turns Copy into a clipboard write, whoever put
            // the row in the list.
            MenuItem::role(MenuRole::Copy),
            MenuItem::role(MenuRole::SelectAll),
        ]
    }
}

/// A card in the theme's own surface and edge.
fn card(t: &Theme) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .bg(t.surface)
        .radius(10.0)
        .border(1.0, t.border)
}

fn num(ev: &UiEvent, name: &str) -> f32 {
    ev.payload
        .get(name)
        .and_then(Value::as_float)
        .unwrap_or(0.0) as f32
}

impl Example for Demo {
    const KEYS: &'static [(&'static str, &'static str)] =
        &[("right-click", "a menu, where one is offered")];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(700.0, 560.0)
    }

    /// Drawn menus, so the rows are nodes a drive can click on every host.
    fn native_menus(&self) -> Option<bool> {
        Some(false)
    }

    /// Right-clicks each of the four, and reads what the core offered:
    /// the standard scope menu, the app's own, the editor's four, nothing.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 700.0, 560.0);
        d.core.set_native_menus(false);
        d.frame(self);
        let right_click = |d: &mut Drive<'_>, app: &mut Demo, x: f32, y: f32| {
            d.input(app, kui::InputEvent::CursorMoved(Vec2::new(x, y)));
            d.input(
                app,
                kui::InputEvent::MouseDown {
                    button: kui::MouseButton::Secondary,
                    clicks: 1,
                },
            );
            d.input(
                app,
                kui::InputEvent::MouseUp {
                    button: kui::MouseButton::Secondary,
                },
            );
        };
        let roles = |menu: &Menu| -> Vec<MenuRole> { menu.items.iter().map(|i| i.role).collect() };

        // The article: the standard scope menu, offered by the core.
        let article = d.key_of("article").ok_or("no article")?;
        let bravo = d.key_of("bravo").ok_or("no row")?;
        let note = d.key_of("note").ok_or("no field")?;
        let article_rect = d.rect_of(article).ok_or("the article has no rect")?;
        let row_rect = d.rect_of(bravo).ok_or("the row has no rect")?;
        let note_rect = d.rect_of(note).ok_or("the field has no rect")?;
        right_click(
            &mut d,
            self,
            article_rect.x + 40.0,
            article_rect.y + article_rect.h / 2.0,
        );
        let menu = d.core.menu().cloned().ok_or("no menu over the article")?;
        d.check(
            roles(&menu).contains(&MenuRole::Copy) && roles(&menu).contains(&MenuRole::SelectAll),
            "the article gets Copy and Select All",
        )?;
        d.check(
            menu.items
                .iter()
                .any(|i| i.role == MenuRole::Copy && !i.enabled),
            "with Copy dimmed while nothing is selected",
        )?;
        d.core.close_menu();

        // A row: the app's own menu, one frame later, with a custom item
        // that comes back as data.
        right_click(
            &mut d,
            self,
            row_rect.x + 20.0,
            row_rect.y + row_rect.h / 2.0,
        );
        d.check(
            self.pending.is_some(),
            "a row's right-click reaches the app",
        )?;
        d.frame(self);
        let menu = d
            .core
            .menu()
            .cloned()
            .ok_or("the app's menu did not open")?;
        d.check(
            menu.items.iter().any(|i| i.label == "Archive"),
            "and the app's own rows are in it",
        )?;
        let archive = menu
            .items
            .iter()
            .position(|i| i.label == "Archive")
            .unwrap();
        for ev in d.core.activate_menu_item(archive) {
            self.on_event(ev);
        }
        d.check(
            self.archived == ["bravo"],
            "choosing Archive arrives as a `menu` event carrying the row",
        )?;
        d.frame(self);

        // The field: the editor's four.
        right_click(
            &mut d,
            self,
            note_rect.x + 20.0,
            note_rect.y + note_rect.h / 2.0,
        );
        let menu = d.core.menu().cloned().ok_or("no menu over the field")?;
        d.check(
            [
                MenuRole::Cut,
                MenuRole::Copy,
                MenuRole::Paste,
                MenuRole::SelectAll,
            ]
            .iter()
            .all(|r| roles(&menu).contains(r)),
            "an editor gets Cut, Copy, Paste and Select All",
        )?;
        d.core.close_menu();

        // The footer: nothing.
        right_click(&mut d, self, 200.0, 540.0);
        d.check(d.core.menu().is_none(), "a plain box opens nothing")
    }
}

kui_devtools::main!(Demo::default());
