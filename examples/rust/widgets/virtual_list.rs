//! A ten-thousand-row list that costs a screenful.
//!
//! The core builds every child a view declares, so a naive thousand-row
//! column pays for a thousand rows on every frame. `Core::scroll_geometry`
//! is the way out: it retains what the last layout resolved for a scroll
//! container — its box, its content size and the clamped offset — so the
//! *view* can decide which rows are worth declaring, and hold the space of
//! the rest with two spacers.
//!
//! Two spellings of the same thing:
//!
//!   - **`widgets::virtual_column`** is the uniform-row case done — visible
//!     range, two rows of overscan, the two spacers, and rows opened at
//!     their *data* index so a row keeps its hover, focus and tweens as the
//!     built range slides over it.
//!   - **`widgets::visible_rows` + `ui.scroll_geometry`** is the arithmetic
//!     alone, for a view that builds its own container (a header inside the
//!     scroller, a grid, rows that are not all one node). `--by-hand` runs
//!     that one; it is what the widget does, unrolled.
//!
//! And when the rows are *not* all one height — a log whose lines wrap —
//! `widgets::virtual_rows` is the same idea over prefix sums instead of a
//! stride: heights come from a `measure` callback it runs only for the rows
//! it is about to build, everything else stands at the mean of those, and
//! the row the window starts in is put back where it was after each frame
//! learns something, so the content never slides. `--variable` runs that.
//!
//! Run: cargo run -p kui --example virtual_list
//!      cargo run -p kui --example virtual_list -- --by-hand
//!      cargo run -p kui --example virtual_list -- --variable
//!      cargo run -p kui --example virtual_list -- --headless [--variable]

use kui::{
    Align, App, Color, Core, Key, NodeSpec, Role, Sizing, TextStyle, TextWrap, Theme, Ui, UiEvent,
    Value, widgets,
};
use kui_harness::{Drive, Example};

const ROWS: usize = 10_000;
const ROW_H: f32 = 28.0;
/// The geometry a view slices by is the previous frame's, so a resize (and
/// the frame a wheel jump lands on) is one frame late; two rows cover it.
const OVERSCAN: usize = 2;

struct VirtualList {
    selected: usize,
    mode: Mode,
    /// What the last frame built, for the header to report.
    built: std::ops::Range<usize>,
    /// `--variable` only: the row heights, which the app owns because the
    /// widget is composed from primitives and keeps nothing of its own.
    heights: widgets::RowHeights,
}

#[derive(Clone, Copy, PartialEq)]
enum Mode {
    Widget,
    ByHand,
    Variable,
}

/// `--variable`'s data: lines of very different lengths, so wrapping gives
/// the rows three or four different heights and no stride describes them.
fn line_of(i: usize) -> String {
    let words = 2 + (i * 7 + i / 3) % 22;
    let mut s = format!("{i:>5}  ");
    for w in 0..words {
        s.push_str(["log", "line", "of", "some", "length", "here"][w % 6]);
        s.push(' ');
    }
    s
}

const BODY: f32 = 13.0;

/// A row's background: the accent where it is selected, and otherwise the
/// two surfaces a striped list alternates between. Opaque rather than a
/// translucent wash, because a `hover_bg` replaces a background instead of
/// compositing over it.
fn row_bg(t: &Theme, i: usize, selected: usize) -> Color {
    if i == selected {
        t.surface.mix(t.accent, 0.28)
    } else if i.is_multiple_of(2) {
        t.surface
    } else {
        t.sunken
    }
}

/// One row. Whatever declares it, it has to come out exactly `ROW_H` tall:
/// that is the stride the arithmetic above and below it assumes.
fn row(ui: &mut Ui<'_>, i: usize, selected: usize) {
    let t = ui.theme();
    let bg = row_bg(&t, i, selected);
    ui.with(
        NodeSpec::row()
            .fill()
            .pad(6.0)
            .gap(8.0)
            .cross_align(Align::Center)
            .bg(bg)
            .hover_bg(bg.mix(t.accent, 0.12))
            .on_click(Value::Int(i as i64))
            .role(Role::ListItem)
            .label(format!("row {i} of {ROWS}")),
        |ui| {
            ui.text(&format!("{i:>5}"), TextStyle::new(13.0).color(t.faint));
            ui.text(&format!("log line {i}"), TextStyle::new(13.0).color(t.fg));
        },
    );
}

/// The style a `--variable` row's text is measured *and* drawn in. The
/// measurement is only worth anything if it is the same style: `measure_text`
/// is what layout would give a text node with this content and this style.
fn body(t: &Theme) -> TextStyle {
    TextStyle::new(BODY).color(t.fg).wrap(TextWrap::Word)
}

fn list_spec(t: &Theme) -> NodeSpec {
    NodeSpec::column()
        .fill()
        .bg(t.bg)
        .role(Role::List)
        .label("log")
}

impl VirtualList {
    /// The whole list, in one call.
    fn widget(&mut self, ui: &mut Ui<'_>) {
        let selected = self.selected;
        let spec = list_spec(&ui.theme());
        let (mut first, mut last) = (usize::MAX, 0usize);
        widgets::virtual_column(ui, "log", spec, ROWS, ROW_H, |ui, i| {
            first = first.min(i);
            last = i + 1;
            row(ui, i, selected);
        });
        self.built = if last == 0 { 0..0 } else { first..last };
    }

    /// The same list with the container in the app's hands: read the
    /// geometry, slice, and open each row at its data index.
    fn by_hand(&mut self, ui: &mut Ui<'_>) {
        // The key the container *will* have — `child_key` is the same hash
        // the open below computes, so the geometry can be read before the
        // node it belongs to is declared.
        let key: Key = ui.child_key("log");
        // `None` until a layout has resolved the container: on the first
        // frame slice by the viewport instead, and ask for the frame that
        // will know better.
        let (offset_y, vh) = match ui.scroll_geometry(key) {
            Some(g) => (g.offset.y, g.rect.h),
            None => {
                ui.request_frame();
                (ui.scroll_offset(key).y, ui.viewport().h)
            }
        };
        let range = widgets::visible_rows(offset_y, vh, 0.0, ROW_H, ROWS, OVERSCAN);
        self.built = range.clone();

        let selected = self.selected;
        // `scroll_y()` implies the clip; `gap(0)` because the stride is
        // `ROW_H` and nothing else.
        let spec = list_spec(&ui.theme()).scroll_y().gap(0.0);
        ui.with_keyed("log", spec, |ui| {
            // Keyed, not auto-keyed: an auto key *is* the sibling index, and
            // the rows already occupy that namespace at their data indices.
            let lead = range.start as f32 * ROW_H;
            if lead > 0.0 {
                ui.with_keyed("lead", spacer(lead), |_| {});
            }
            for i in range.clone() {
                // The key auto-keying would have given row `i` in a list
                // that built them all — so hover, focus and any tween stay
                // with the row as the window slides over it.
                ui.with_indexed(
                    i as u64,
                    NodeSpec::column()
                        .width(Sizing::Grow(1.0))
                        .height(Sizing::Fixed(ROW_H)),
                    |ui| row(ui, i, selected),
                );
            }
            let tail = (ROWS - range.end) as f32 * ROW_H;
            if tail > 0.0 {
                ui.with_keyed("tail", spacer(tail), |_| {});
            }
        });
    }
}

fn spacer(h: f32) -> NodeSpec {
    NodeSpec::column()
        .width(Sizing::Grow(1.0))
        .height(Sizing::Fixed(h))
}

impl VirtualList {
    /// Rows of different heights, from `measure_text` — the number layout
    /// itself would give the row's text at that width, so what the row
    /// measures is what the row gets.
    fn variable(&mut self, ui: &mut Ui<'_>) {
        let selected = self.selected;
        let t = ui.theme();
        let (mut first, mut last) = (usize::MAX, 0usize);
        widgets::virtual_rows(
            ui,
            "log",
            list_spec(&t).pad(6.0),
            &mut self.heights,
            |ui, i, w| {
                // The row pads itself by 6 on each side, and that padding is
                // part of the stride the arithmetic uses.
                ui.measure_text(&line_of(i), &body(&t), Some(w - 12.0))
                    .height
                    + 12.0
            },
            |ui, i| {
                first = first.min(i);
                last = i + 1;
                let bg = row_bg(&t, i, selected);
                ui.with(
                    NodeSpec::column()
                        .fill()
                        .pad(6.0)
                        .bg(bg)
                        .hover_bg(bg.mix(t.accent, 0.12))
                        .on_click(Value::Int(i as i64))
                        .role(Role::ListItem)
                        .label(format!("row {i} of {ROWS}")),
                    |ui| ui.text(&line_of(i), body(&t)),
                );
            },
        );
        self.built = if last == 0 { 0..0 } else { first..last };
    }
}

impl App for VirtualList {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill().bg(t.bg), |ui| {
            let built = self.built.len();
            let how = match self.mode {
                Mode::Widget => "virtual_column",
                Mode::ByHand => "by hand",
                Mode::Variable => "virtual_rows",
            };
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .pad(8.0)
                    .bg(t.surface),
                |ui| {
                    ui.text(
                        &format!(
                            "{ROWS} rows, {built} built ({how}) — row {} selected",
                            self.selected
                        ),
                        TextStyle::new(14.0).color(t.fg),
                    );
                },
            );

            match self.mode {
                Mode::Widget => self.widget(ui),
                Mode::ByHand => self.by_hand(ui),
                Mode::Variable => self.variable(ui),
            }
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        // A row's `on_click` payload arrives verbatim — it is the row's
        // index, and there is nothing else to decode.
        if let Some(i) = ev.payload.as_int() {
            self.selected = i as usize;
        }
    }
}

impl Example for VirtualList {
    const FLAGS: &'static [(&'static str, &'static str)] = &[
        (
            "--by-hand",
            "the same list unrolled from scroll_geometry + visible_rows",
        ),
        (
            "--variable",
            "rows of no fixed height, through virtual_rows",
        ),
    ];
    const KEYS: &'static [(&'static str, &'static str)] = &[("wheel", "scroll 10,000 rows")];

    fn window(&self) -> kui_harness::Window {
        kui_harness::Window::default().size(560.0, 480.0)
    }

    /// The same view against a bare `Core`. Two frames, because the first
    /// has only the viewport to slice by; then a wheel to row 300, which
    /// reaches no event handler — the next frame's view reads the new
    /// offset; then a click on a row the first frame never built.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 480.0, 300.0);
        d.frame(self);
        let screenful = (300.0 / ROW_H) as usize;
        d.check(
            !self.built.is_empty() && self.built.len() < 4 * screenful,
            "frame 1 slices by the viewport, having no geometry yet",
        )?;
        d.frame(self);
        d.check(
            self.built.start == 0
                && self.built.len() >= screenful
                && self.built.len() < 4 * screenful,
            "frame 2 builds a screenful and its overscan, not 10,000 rows",
        )?;
        let log = d.key_of("log").ok_or("no log container")?;
        let g = d
            .core
            .scroll_geometry(log)
            .ok_or("the log was not laid out")?;
        d.check(
            g.content.h >= (ROWS as f32) * ROW_H * 0.9,
            "the content is the whole list's height",
        )?;

        d.wheel(self, 240.0, 200.0, 0.0, -300.0 * ROW_H);
        d.frame(self);
        d.frame(self);
        // Uniform rows land within the overscan of row 300; rows of
        // varying height land wherever 8,400 px of them reach, which is
        // the point of measuring them.
        let landed = if self.mode == Mode::Variable {
            self.built.start > 100
        } else {
            self.built.start >= 290 && self.built.start <= 300
        };
        d.check(landed, "a wheel to row 300 re-slices the built range")?;
        if self.mode == Mode::Variable {
            let measured = (0..ROWS)
                .filter(|&i| self.heights.measured(i).is_some())
                .count();
            d.check(
                measured > 0 && measured < ROWS / 10,
                "only the rows built so far were measured",
            )?;
        }

        // A row that was never in the first frame's range is an ordinary
        // node: it hit-tests, and its payload comes back as the app wrote it.
        d.click(self, 240.0, 100.0);
        let clicked = self.selected;
        d.check(
            clicked > 100 && self.built.contains(&clicked),
            "a row the first frame never built is clickable",
        )
    }
}

fn main() {
    let mode = if std::env::args().any(|a| a == "--variable") {
        Mode::Variable
    } else if std::env::args().any(|a| a == "--by-hand") {
        Mode::ByHand
    } else {
        Mode::Widget
    };
    kui_harness::run(
        env!("CARGO_BIN_NAME"),
        VirtualList {
            selected: 0,
            mode,
            built: 0..0,
            // 28 is a guess, and all it decides is how wrong the scrollbar
            // is before anything has been measured.
            heights: widgets::RowHeights::new(ROWS, ROW_H),
        },
    );
}
