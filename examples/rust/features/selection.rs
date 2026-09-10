//! Selection as a scope (`docs/adr/0017-selection-as-a-scope.md`), over
//! every kind of text it touches. One `selectable` row on a container
//! makes everything inside it one selection — plain `text`, a rich
//! paragraph of spans, and whatever else is in the box — so a drag across
//! the card takes the heading, both paragraphs and the footer as one run;
//! double-click takes a word, triple-click a line; Cmd/Ctrl-C copies the
//! plain text with its bold and italic carried beside it. A `cells` grid
//! is a scope too, selecting in cells rather than bytes; an `edit` has a
//! selection of its own, the caret's, which the same keys and drags move.
//!
//! There is one selection per window: starting one in any scope clears
//! the others, and `Ui::selection_text` / `Ui::cell_selection` read back
//! whichever it is. The readouts sit *outside* the scopes on purpose:
//! inside, a label that reports the selection's length would be part of
//! what Select All selects, and the number would chase itself.
//!
//! Force-clicking a word (a Force Touch trackpad) selects it and opens
//! the system's Look Up panel.
//!
//! Run: cargo run -p kui --example selection [-- --headless]

use kui::{
    Align, App, Cell, CellGrid, Core, EditOptions, FontFamily, NodeSpec, Sizing, Span, TextStyle,
    Theme, Ui,
};
use kui_harness::{Drive, Example};

const LINES: [&str; 4] = [
    "~/kui $ cargo test -p kui-core",
    "running 23 tests ..............",
    "test result: ok. 23 passed; 0 failed",
    "~/kui $ ",
];
const COLS: usize = 40;

struct Selection;

fn card(t: &Theme) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .max_width(560.0)
        .pad(20.0)
        .gap(10.0)
        .bg(t.surface)
        .radius(12.0)
        .border(1.0, t.border)
}

impl App for Selection {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // What is selected, whichever scope holds it.
        let readout = match ui.cell_selection() {
            Some(sel) => {
                let (a, b) = sel.ordered();
                format!("cells: lines {}–{} selected", a.line, b.line)
            }
            None => ui
                .selection_text()
                .filter(|s| !s.is_empty())
                .map(|s| format!("text: {} characters selected", s.chars().count()))
                .unwrap_or_else(|| "drag across anything below".into()),
        };
        ui.with(
            NodeSpec::column()
                .fill()
                .cross_align(Align::Center)
                .pad(24.0)
                .gap(12.0)
                .scroll_y(),
            |ui| {
                // One scope over a heading, a rich paragraph and a plain
                // line: the row is on the container, and every run inside
                // selects as one.
                ui.with_keyed("article", card(&t).selectable(), |ui| {
                    ui.text("Selectable article", TextStyle::new(22.0));
                    ui.rich_text(
                        &[
                            Span::new("Drag across this paragraph and the whole card selects as "),
                            Span::new("one run of text").bold().color(t.accent),
                            Span::new(
                                " — three labels, one selection. Double-click a word, \
                                 triple-click a line; Cmd-C copies, with the ",
                            ),
                            Span::new("bold").bold(),
                            Span::new(" and the "),
                            Span::new("italic").italic(),
                            Span::new(" carried beside the plain text."),
                        ],
                        TextStyle::new(15.0).line_height(24.0),
                    );
                    ui.text(
                        "The footer is inside the scope too, so Select All takes it.",
                        TextStyle::new(13.0).color(t.muted),
                    );
                });

                // A grid is its own scope, in cells.
                let mut cells = vec![Cell::new(' ', t.muted.to_hex(), 0); LINES.len() * COLS];
                for (r, line) in LINES.iter().enumerate() {
                    for (c, ch) in line.chars().take(COLS).enumerate() {
                        cells[r * COLS + c] = Cell::new(ch, t.fg.to_hex(), 0);
                    }
                }
                let grid = CellGrid {
                    rows: LINES.len(),
                    cols: COLS,
                    cells: &cells,
                    style: TextStyle::new(13.0).family(FontFamily::Mono).color(t.fg),
                    cursor: None,
                    origin_line: 100,
                };
                ui.with(card(&t).pad(12.0), |ui| {
                    ui.text(
                        "A terminal selects in cells (Alt-drag a rectangle)",
                        TextStyle::new(12.0).color(t.muted),
                    );
                    ui.cells_keyed(
                        "term",
                        &grid,
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .pad(10.0)
                            .radius(6.0)
                            .bg(t.sunken)
                            .selectable(),
                    );
                });

                // An editor's selection is the caret's: the same drag, the
                // same double-click, and Shift with the arrows.
                ui.with(card(&t).pad(12.0), |ui| {
                    ui.text(
                        "An editor's selection is its own",
                        TextStyle::new(12.0).color(t.muted),
                    );
                    ui.text_edit(
                        "note",
                        "Select in here with a drag, a double-click, or Shift and the arrows.",
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

                ui.with(
                    NodeSpec::row()
                        .width(Sizing::Grow(1.0))
                        .max_width(560.0)
                        .main_align(Align::End),
                    |ui| {
                        ui.text(&readout, TextStyle::new(12.0).color(t.accent));
                    },
                );
            },
        );
    }
}

impl Example for Selection {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("drag", "select"),
        ("double / triple click", "a word / a line"),
        ("⌘A", "everything in the scope"),
        ("⌘C", "copy"),
    ];

    fn window(&self) -> kui_harness::Window {
        kui_harness::Window::default().size(620.0, 620.0)
    }

    /// Select All in the article takes all three runs as one; a drag in
    /// the grid clears it and selects cells instead — one selection per
    /// window.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 620.0, 620.0);
        d.frame(self);
        let article = d.key_of("article").ok_or("no article")?;
        d.core.select_all_in(article);
        d.frame(self);
        let text = d.core.selection_text().unwrap_or_default();
        d.check(
            text.contains("Selectable article") && text.contains("Select All takes it"),
            "Select All takes the heading, the paragraph and the footer as one",
        )?;
        let html = d.core.selection_html().unwrap_or_default();
        d.check(
            html.contains("<b>") || html.contains("bold"),
            "and the copy carries the bold",
        )?;

        let term = d.key_of("term").ok_or("no grid")?;
        let r = d.rect_of(term).ok_or("the grid has no rect")?;
        d.input(
            self,
            kui::InputEvent::CursorMoved(kui::Vec2::new(r.x + 20.0, r.y + 14.0)),
        );
        d.input(self, kui::InputEvent::mouse_down(1));
        d.input(
            self,
            kui::InputEvent::CursorMoved(kui::Vec2::new(r.x + 200.0, r.y + 40.0)),
        );
        d.input(self, kui::InputEvent::mouse_up());
        d.frame(self);
        d.check(
            d.core.cell_selection().is_some(),
            "a drag in the grid selects cells",
        )?;
        d.check(
            d.core.selection_text().is_none_or(|s| s.is_empty()),
            "and the article's selection is gone: one selection per window",
        )
    }
}

kui_harness::main!(Selection);
