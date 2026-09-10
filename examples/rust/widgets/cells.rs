//! The `cells` element: one screenful of a terminal as a grid — a
//! character, a colour and a background per cell, laid out row-major in
//! a monospace style, with a block cursor and an `origin_line` that says
//! where this screenful sits in the session's own history. The grid is
//! data the app rebuilds every frame from its own screen model; nothing
//! is retained in the core but what a `cells` node keeps for itself.
//!
//! Its selection is in *cells*, not in bytes (ADR 0017): drag out a block
//! of the screen, hold Alt for a rectangle, double-click a word,
//! triple-click a row. What a copy takes is what a terminal copies — the
//! lines, each one's trailing blanks trimmed — and its ends are absolute
//! lines, so the readout says which lines of the session they are and
//! not which rows of the screen. The buttons scroll the screen under a
//! selection to show the ends staying put.
//!
//! Run: cargo run -p kui --example cells [-- --headless]

use kui::widgets;
use kui::{
    Align, App, Cell, CellCursor, CellGrid, Core, FontFamily, NodeSpec, Sizing, TextStyle, Theme,
    Ui, UiEvent, Value,
};
use kui_devtools::{Drive, Example};

/// What a line of the fake session is *for*. A terminal's palette is the
/// app's, but these three are roles the theme already names — so the
/// screen reads on a light desktop instead of staying the grey it was
/// picked to be on a dark one.
#[derive(Clone, Copy)]
enum Ink {
    /// A prompt, a command, ordinary output.
    Plain,
    /// The compiler's own chatter.
    Quiet,
    /// A passing test run.
    Good,
}

impl Ink {
    fn of(self, t: &Theme) -> u32 {
        match self {
            Ink::Plain => t.fg,
            Ink::Quiet => t.muted,
            Ink::Good => t.success,
        }
        .to_hex()
    }
}

/// The fake session: a prompt, a command, its output, twice over, so the
/// screen can scroll through it. Trailing blanks are the app's own
/// padding, which is exactly what a copy has to trim.
const SESSION: [(&str, Ink); 12] = [
    ("~/kui $ cargo test -p kui-core", Ink::Plain),
    ("   Compiling kui-core v0.1.0-alpha.10", Ink::Quiet),
    ("    Finished `test` profile in 3.42s", Ink::Quiet),
    ("running 23 tests ..............", Ink::Plain),
    ("test result: ok. 23 passed; 0 failed", Ink::Good),
    ("~/kui $ cargo bench -p kui-core -- layout", Ink::Plain),
    ("   Compiling kui-core v0.1.0-alpha.10", Ink::Quiet),
    ("    Finished `bench` profile in 6.10s", Ink::Quiet),
    ("deep_nesting        fastest │ 41.2 µs", Ink::Plain),
    ("list_10k_rows       fastest │ 2.98 ms", Ink::Plain),
    ("done                                   ", Ink::Good),
    ("~/kui $ ", Ink::Plain),
];
/// Where the session's first line sits in its history: an end of a
/// selection is an *absolute* line, so scrolling the screen under it does
/// not move it (`origin_line`).
const FIRST_LINE: u64 = 1_204;
const TERM_COLS: usize = 44;
/// Rows on screen: fewer than the session has, so it scrolls.
const TERM_ROWS: usize = 6;

#[derive(Default)]
struct Cells {
    /// The first session line on screen.
    top: usize,
}

impl App for Cells {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // One screenful of cells from the session at `top`. The app pads
        // its own lines, which is why a copy trims them.
        let mut cells = vec![Cell::new(' ', t.muted.to_hex(), 0); TERM_ROWS * TERM_COLS];
        for r in 0..TERM_ROWS {
            let Some((line, ink)) = SESSION.get(self.top + r) else {
                break;
            };
            let fg = ink.of(&t);
            for (c, ch) in line.chars().take(TERM_COLS).enumerate() {
                cells[r * TERM_COLS + c] = Cell::new(ch, fg, 0);
            }
        }
        // The cursor sits after the last prompt, when it is on screen.
        let cursor = (SESSION.len() - 1)
            .checked_sub(self.top)
            .filter(|r| *r < TERM_ROWS)
            .map(|r| (r, 8, CellCursor::Block, t.accent.with_alpha(0.7)));
        let grid = CellGrid {
            rows: TERM_ROWS,
            cols: TERM_COLS,
            cells: &cells,
            style: TextStyle::new(13.0).family(FontFamily::Mono).color(t.fg),
            cursor,
            origin_line: FIRST_LINE + self.top as u64,
        };
        // What is selected: which *lines* of the session, since the ends
        // are absolute and the rows are not.
        let selected = ui.cell_selection().map(|sel| {
            let (a, b) = sel.ordered();
            format!("lines {}–{} of the session selected", a.line, b.line)
        });
        ui.with(
            NodeSpec::column()
                .fill()
                .cross_align(Align::Center)
                .pad(24.0)
                .gap(12.0),
            |ui| {
                ui.with(
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .max_width(620.0)
                        .pad(12.0)
                        .gap(8.0)
                        .bg(t.surface)
                        .radius(10.0)
                        .border(1.0, t.border),
                    |ui| {
                        ui.text(
                            "a terminal selects in cells — double-click a word, Alt-drag a rectangle",
                            TextStyle::new(13.0).color(t.muted),
                        );
                        ui.cells_keyed(
                            "term",
                            &grid,
                            NodeSpec::column()
                                .width(Sizing::Grow(1.0))
                                .pad(10.0)
                                .radius(6.0)
                                .bg(t.sunken)
                                .border(1.0, t.border)
                                // A scope of one grid: the drag selects
                                // cells, and the stock menu's Select All
                                // takes the whole screen.
                                .selectable(),
                        );
                        ui.with(
                            NodeSpec::row()
                                .width(Sizing::Grow(1.0))
                                .gap(8.0)
                                .cross_align(Align::Center),
                            |ui| {
                                widgets::button(ui, "▲ earlier", Value::str("up"));
                                widgets::button(ui, "▼ later", Value::str("down"));
                                ui.text(
                                    &format!(
                                        "lines {}–{} on screen",
                                        FIRST_LINE + self.top as u64,
                                        FIRST_LINE + (self.top + TERM_ROWS) as u64 - 1
                                    ),
                                    TextStyle::new(12.0).color(t.muted),
                                );
                                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                                ui.text(
                                    selected.as_deref().unwrap_or("nothing selected"),
                                    TextStyle::new(12.0).color(t.accent),
                                );
                            },
                        );
                    },
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.as_str() {
            Some("up") => self.top = self.top.saturating_sub(1),
            Some("down") => self.top = (self.top + 1).min(SESSION.len() - TERM_ROWS),
            _ => {}
        }
    }
}

impl Example for Cells {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("drag", "select cells"),
        ("Alt-drag", "a rectangle"),
        ("double / triple click", "a word / a row"),
        ("⌘C", "copy, trailing blanks trimmed"),
    ];

    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(680.0, 300.0)
    }

    /// Selects a block by dragging across the grid, then scrolls the
    /// screen under it: the ends are absolute lines and stay where they
    /// were.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 680.0, 300.0);
        d.frame(self);
        let term = d.key_of("term").ok_or("no grid")?;
        let r = d.rect_of(term).ok_or("the grid has no rect")?;
        // A drag from the second row to the fourth.
        let (x0, y0) = (r.x + 30.0, r.y + 10.0 + 1.5 * 18.0);
        let (x1, y1) = (r.x + 200.0, r.y + 10.0 + 3.5 * 18.0);
        d.input(self, kui::InputEvent::CursorMoved(kui::Vec2::new(x0, y0)));
        d.input(self, kui::InputEvent::mouse_down(1));
        d.input(self, kui::InputEvent::CursorMoved(kui::Vec2::new(x1, y1)));
        d.input(self, kui::InputEvent::mouse_up());
        d.frame(self);
        let sel = d.core.cell_selection().ok_or("the drag selected nothing")?;
        let (a, b) = sel.ordered();
        d.check(a.line < b.line, "a drag selects a block of cells")?;
        d.check(
            a.line >= FIRST_LINE && b.line < FIRST_LINE + TERM_ROWS as u64,
            "and its ends are absolute session lines",
        )?;
        let copied = d.core.copy_selection().unwrap_or_default();
        d.check(
            !copied.is_empty() && !copied.lines().any(|l| l.ends_with(' ')),
            "a copy is the lines with their trailing blanks trimmed",
        )?;

        // Scroll the screen under the selection: the ends stay put.
        let down = d.key_of("▼ later").ok_or("no scroll button")?;
        d.click_key(self, down);
        d.frame(self);
        let after = d
            .core
            .cell_selection()
            .ok_or("scrolling lost the selection")?;
        let (a2, b2) = after.ordered();
        d.check(
            (a2.line, b2.line) == (a.line, b.line),
            "scrolling the screen leaves the selection on its lines",
        )
    }
}

kui_devtools::main!(Cells::default());
