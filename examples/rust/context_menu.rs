//! Context menus, and the selection they act on
//! (`docs/adr/0017-selection-as-a-scope.md`).
//!
//! Five things to try, and each one is a different rule:
//!
//!   * **Right-click the article.** Nothing declares a menu there, so the
//!     core offers the standard one for a `selectable` scope — Copy, and
//!     Select All, with Copy dimmed until something is selected. Drag
//!     across the text first and it lights up.
//!   * **Right-click a row of the list.** The row declares
//!     `on_context_menu`, so the app's own menu wins: the standard items
//!     *and* two of the app's, opened with `Ui::open_menu`. Whichever is
//!     chosen arrives as one `menu` event on the row.
//!   * **Right-click the terminal.** A `cells` grid selects in *cells*,
//!     not in bytes: drag out a block of the screen (hold Alt for a
//!     rectangle), double-click a word, triple-click a row, and its menu
//!     copies what a terminal copies — the lines, with each one's trailing
//!     blanks trimmed. Its ends are absolute lines, so the readout says
//!     which lines of the session they are and not which rows of the
//!     screen.
//!   * **Right-click the field.** An editor gets Cut / Copy / Paste /
//!     Select All without asking for anything.
//!   * **Right-click the footer.** A plain box: no menu, because a
//!     right-click on nothing has never opened one.
//!
//! And one that is not a right-click at all:
//!
//!   * **The menu bar across the top.** The frame declares it
//!     (`docs/adr/0018-a-menu-bar-the-app-declares.md`) and, on this
//!     machine, `widgets::menu_bar` draws it — on macOS the same
//!     declaration goes to the OS and the strip in the window is not
//!     there. Its rows are the rows a context menu has, so View ▸ Wrap
//!     checks itself and Edit ▸ Copy is the same Copy, and every choice
//!     arrives as the same one event.
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
    Align, App, BarMenu, Cell, CellCursor, CellGrid, Color, FontFamily, Key, Menu, MenuBar,
    MenuItem, MenuRole, NodeSpec, Sizing, Span, TextStyle, Ui, UiEvent, Value, Vec2,
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

/// The fake session the `cells` panel shows: a prompt, a command, its
/// output. Trailing blanks are the app's own padding, which is exactly
/// what a copy has to trim.
const SESSION: [(&str, u32); 6] = [
    ("~/kui $ cargo test -p kui-core", 0xd6d8e0ff),
    ("   Compiling kui-core v0.1.0-alpha.10", 0x8a90a3ff),
    ("    Finished `test` profile in 3.42s", 0x8a90a3ff),
    ("running 23 tests ..............", 0xd6d8e0ff),
    ("test result: ok. 23 passed; 0 failed", 0x6bd08aff),
    ("~/kui $ ", 0xd6d8e0ff),
];
/// Where this screenful sits in the session's own history: an end of a
/// selection is an *absolute* line, so scrolling the screen under it does
/// not move it (`origin_line`).
const FIRST_LINE: u64 = 1_204;
const TERM_COLS: usize = 44;

#[derive(Default)]
struct Demo {
    /// The last thing a menu reported, shown at the bottom — the whole
    /// point of the items being data is that this is an ordinary event.
    last: Option<String>,
    /// Rows the app's own menu archived, to show a custom item doing
    /// something the core could not have done for it.
    archived: Vec<String>,
    /// A setting one of the menu bar's rows toggles, to show a `checked`
    /// row doing what a checked row is for.
    wrap: bool,
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
        ui.configure_root(NodeSpec::column().fill().bg(BG).gap(16.0));
        // The application menu, in one call: what it is, and where its
        // strip goes when it has to be drawn. Full width and flush with
        // the top, because that is where a menu bar goes — the padding the
        // rest of the page had moves inside. On macOS this draws nothing
        // and the same menu is the one at the top of the screen.
        kui::widgets::menu_bar(ui, self.menu());
        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .padding(kui::Edges {
                    l: 28.0,
                    r: 28.0,
                    t: 0.0,
                    b: 28.0,
                })
                .cross_align(Align::Center),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .max_width(620.0)
                        .gap(16.0),
                    |ui| {
                        self.article(ui);
                        self.list(ui);
                        self.terminal(ui);
                        self.field(ui);
                        self.footer(ui);
                    },
                );
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
                // The menu bar's own rows arrive here too, on the same
                // event: one handler for both menus is the whole point.
                match did {
                    Some("wrap") => self.wrap = !self.wrap,
                    Some("clear") => self.archived.clear(),
                    _ => {}
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

    fn terminal(&mut self, ui: &mut Ui<'_>) {
        // One screenful of cells: a character, a colour and a background
        // each, laid out row-major. The app pads its own lines, which is
        // why a copy trims them.
        let mut cells = vec![Cell::new(' ', 0x8a90a3ff, 0); SESSION.len() * TERM_COLS];
        for (r, (line, fg)) in SESSION.iter().enumerate() {
            for (c, ch) in line.chars().take(TERM_COLS).enumerate() {
                cells[r * TERM_COLS + c] = Cell::new(ch, *fg, 0);
            }
        }
        let grid = CellGrid {
            rows: SESSION.len(),
            cols: TERM_COLS,
            cells: &cells,
            style: TextStyle::new(13.0).family(FontFamily::Mono).color(TEXT),
            // The block cursor after the last prompt.
            cursor: Some((
                SESSION.len() - 1,
                8,
                CellCursor::Block,
                Color {
                    r: 0.42,
                    g: 0.62,
                    b: 1.0,
                    a: 0.7,
                },
            )),
            origin_line: FIRST_LINE,
        };
        ui.with(card().pad(12.0).gap(8.0), |ui| {
            ui.text(
                "A terminal selects in cells — double-click a word, Alt-drag a rectangle",
                TextStyle::new(13.0).color(MUTED),
            );
            ui.cells_keyed(
                "term",
                &grid,
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .pad(10.0)
                    .radius(6.0)
                    .bg(BG)
                    .border(1.0, EDGE)
                    // A scope of one grid: the drag selects cells, and the
                    // stock menu's Select All takes the whole screen.
                    .selectable(),
            );
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
        // What is selected, whichever kind of selection it is — and for
        // the grid, which *lines* of the session, since its ends are
        // absolute and its rows are not.
        let selected = match ui.cell_selection() {
            Some(sel) => {
                let (a, b) = sel.ordered();
                Some(format!("lines {}–{} of the session", a.line, b.line))
            }
            None => ui
                .selection_text()
                .filter(|t| !t.is_empty())
                .map(|t| format!("{} characters selected", t.chars().count())),
        };
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .pad_xy(4.0, 2.0)
                .gap(8.0),
            |ui| {
                ui.text("last menu event —", TextStyle::new(12.0).color(MUTED));
                ui.text(&said, TextStyle::new(12.0).color(ACCENT));
                if let Some(selected) = &selected {
                    ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                    ui.text(selected, TextStyle::new(12.0).color(MUTED));
                }
            },
        );
    }

    /// The menu one row gets: the standard items worth having on it, and
    /// two the app invented. A custom row carries its own payload, which
    /// is what comes back in the event — so nothing here has to work out
    /// afterwards which row the menu was about.
    /// The application menu, rebuilt every frame from the model — which is
    /// what lets View ▸ Wrap carry its own state without anything retained
    /// anywhere. Every row is an ordinary `MenuItem`: two of the app's own
    /// with `id`s, a `checked` setting, and the standard Edit rows the core
    /// performs itself.
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
                    MenuItem::new("Close").id(mine("close")).accel("mod+w"),
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
                    MenuItem::new("Wrap the article")
                        .id(mine("wrap"))
                        .accel("mod+shift+w")
                        .checked(self.wrap),
                    MenuItem::new("Clear the archive")
                        .id(mine("clear"))
                        .enabled(!self.archived.is_empty()),
                ],
            ),
        ])
    }

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
    kui::app("kui — context menus")
        .size(700.0, 820.0)
        .run(Demo::default())
        .unwrap();
}
