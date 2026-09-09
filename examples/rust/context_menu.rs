//! Context menus, and the selection they act on
//! (`docs/adr/0017-selection-as-a-scope.md`).
//!
//! Four things to try, and each one is a different rule:
//!
//!   * **Right-click the article.** Nothing declares a menu there, so the
//!     core offers the standard one for a `selectable` scope — Copy, and
//!     Select All, with Copy dimmed until something is selected. Drag
//!     across the text first and it lights up.
//!   * **Right-click a row of the list.** The row declares
//!     `on_context_menu`, so the app's own menu wins: the standard items
//!     *and* two of the app's, opened with `Ui::open_menu`. Whichever is
//!     chosen arrives as one `menu` event on the row.
//!   * **Right-click the field.** An editor gets Cut / Copy / Paste /
//!     Select All without asking for anything.
//!   * **Right-click the footer.** A plain box: no menu, because a
//!     right-click on nothing has never opened one.
//!
//! On macOS all of them are the platform's own `NSMenu`, because the
//! runner says it can draw one; everywhere else the core draws the same
//! items itself. The app's code is identical either way, which is the
//! point of the items being data.
//!
//! Run: cargo run -p kui --example context_menu
//!
//! Force-clicking a word (a Force Touch trackpad) selects it and opens the
//! system's Look Up panel; Cmd-C copies the selection, with its bold and
//! italic carried beside the plain text.

use kui::{
    Align, App, Color, Key, Menu, MenuItem, MenuRole, NodeSpec, Sizing, Span, TextStyle, Ui,
    UiEvent, Value, Vec2,
};

const BG: Color = Color {
    r: 0.055,
    g: 0.063,
    b: 0.086,
    a: 1.0,
};
const CARD: Color = Color {
    r: 0.086,
    g: 0.094,
    b: 0.125,
    a: 1.0,
};
const EDGE: Color = Color {
    r: 0.165,
    g: 0.176,
    b: 0.212,
    a: 1.0,
};
const TEXT: Color = Color {
    r: 0.851,
    g: 0.863,
    b: 0.898,
    a: 1.0,
};
const MUTED: Color = Color {
    r: 0.541,
    g: 0.561,
    b: 0.639,
    a: 1.0,
};
const ACCENT: Color = Color {
    r: 0.42,
    g: 0.62,
    b: 1.0,
    a: 1.0,
};

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
        ui.configure_root(
            NodeSpec::column()
                .fill()
                .bg(BG)
                .pad(28.0)
                .gap(16.0)
                .cross_align(Align::Center),
        );
        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .max_width(620.0)
                .gap(16.0),
            |ui| {
                self.article(ui);
                self.list(ui);
                self.field(ui);
                self.footer(ui);
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        let kind = ev.payload.get("kind").and_then(Value::as_str);
        match kind {
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
    fn article(&mut self, ui: &mut Ui<'_>) {
        ui.with(card().selectable().pad(20.0).gap(10.0), |ui| {
            ui.text("Selectable article", TextStyle::new(20.0).color(TEXT));
            ui.rich_text(
                &[
                    Span::new("Drag across this paragraph and the whole card selects as "),
                    Span::new("one run of text").bold().color(ACCENT),
                    Span::new(
                        " — three labels, one selection. Right-click for the standard \
                             menu; nothing here declares one, so the core offers what it \
                             can do: Copy, and Select All.",
                    ),
                ],
                TextStyle::new(15.0).line_height(24.0).color(TEXT),
            );
            ui.text(
                "Copy carries the bold with it, beside the plain text.",
                TextStyle::new(13.0).color(MUTED),
            );
        });
    }

    fn list(&mut self, ui: &mut Ui<'_>) {
        ui.with(card().pad(12.0).gap(2.0), |ui| {
            ui.text(
                "Rows with a menu of their own",
                TextStyle::new(13.0).color(MUTED),
            );
            for name in ROWS {
                let archived = self.archived.iter().any(|a| a == name);
                ui.with_keyed(
                    name,
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .pad_xy(10.0, 8.0)
                        .radius(6.0)
                        .hover_bg(EDGE)
                        // The declaration that wins: the core stands back
                        // and this reaches `on_event` instead.
                        .on_context_menu(Value::str(name)),
                    |ui| {
                        let style = TextStyle::new(14.0).color(if archived { MUTED } else { TEXT });
                        ui.text(name, style);
                        if archived {
                            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                            ui.text("archived", TextStyle::new(12.0).color(MUTED));
                        }
                    },
                );
            }
        });
    }

    fn field(&mut self, ui: &mut Ui<'_>) {
        ui.with(card().pad(12.0).gap(8.0), |ui| {
            ui.text(
                "An editor gets the four a field has",
                TextStyle::new(13.0).color(MUTED),
            );
            ui.text_edit(
                "note",
                "Cut, Copy, Paste, Select All — none of it declared.",
                &kui::EditOptions::default(),
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .pad(10.0)
                    .radius(6.0)
                    .bg(BG)
                    .border(1.0, EDGE),
            );
        });
    }

    fn footer(&mut self, ui: &mut Ui<'_>) {
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
                ui.text("last menu event —", TextStyle::new(12.0).color(MUTED));
                ui.text(&said, TextStyle::new(12.0).color(ACCENT));
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

fn card() -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .bg(CARD)
        .radius(10.0)
        .border(1.0, EDGE)
}

fn num(ev: &UiEvent, name: &str) -> f32 {
    ev.payload
        .get(name)
        .and_then(Value::as_float)
        .unwrap_or(0.0) as f32
}

fn main() {
    kui::run("kui — context menus", Demo::default(), vec![]).unwrap();
}
