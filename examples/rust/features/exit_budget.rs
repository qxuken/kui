//! The exit budget at its boundary (`docs/adr/0012-the-exit-budget.md`).
//!
//! `exit` copies a departing subtree out of the last frame that had it and
//! replays the copy, and the store that holds those copies is bounded at
//! 4096 nodes (backlog DX23; 512 until a 1,800-node pane had to fade). A
//! toast or a pane never gets near it. This one does, on purpose, so the
//! three rules the budget follows are each a button:
//!
//!   - **whole or not at all.** Two grids of 2100 cells, every cell with
//!     its own `exit`. *clear A* drops 2100 nodes in one frame: under the
//!     budget, so every cell slides out. *clear both* drops 4200 in one
//!     frame: over it, so **nothing** animates — the grids vanish at once,
//!     exactly as they would with no `exit` declared — and the runner
//!     prints the `exit-budget` warning on stderr, naming the frame's count.
//!     Before ADR 0012 that frame animated the budget's worth and blinked
//!     the rest.
//!   - **a new removal outranks the ghosts in flight.** *clear A*, then
//!     *clear B* while A is still fading: B fits the budget but not the
//!     room beside A's 2100 ghosts, so A's oldest give way and every cell
//!     of B animates. The removal you just caused is the one you are
//!     looking at.
//!   - **put `exit` on the list, and make the list small.** The right-hand
//!     column is a `uniform_list` of 5000 rows with the `exit` on the
//!     container. *clear list* removes five thousand rows and the picture
//!     the store keeps is the built slice — the rows on screen and two
//!     spacers — so it slides out whole. A plain 5000-row column with the
//!     same `exit` would be 5001 nodes and refused: the budget counts what
//!     is copied, and virtualisation is what keeps that count small.
//!
//! Run: cargo run -p kui-native --example exit_budget

use kui_devtools::Example;
use kui_native::{Align, App, Color, Enter, NodeSpec, TextStyle, Ui, UiEvent, Value, widgets};

/// Cells per grid: 2100. One grid is under the budget (4096); both
/// together are over it.
const CELLS_PER_ROW: usize = 60;
const ROWS: usize = 35;
/// A cell and the gap after it, px: small, so 2100 fit on screen.
const CELL: f32 = 5.0;
const STEP: f32 = 6.0;
const LIST_ROWS: usize = 5000;
const ROW_H: f32 = 22.0;

struct BulkExit {
    a: bool,
    b: bool,
    list: bool,
}

impl BulkExit {
    /// A grid of cells that each carry their own `exit`. The rows are plain
    /// containers that stay declared when the grid clears, so what departs
    /// is the 2100 one-node cells inside them: an exit plays where its
    /// parent is still declared, and a cell whose row went would go with it
    /// at once (backlog DX19).
    fn grid(&self, ui: &mut Ui<'_>, key: &str, filled: bool, color: Color) {
        ui.with_keyed(
            key,
            NodeSpec::column()
                .size(
                    CELLS_PER_ROW as f32 * STEP - (STEP - CELL),
                    ROWS as f32 * STEP - (STEP - CELL),
                )
                .gap(STEP - CELL),
            |ui| {
                for r in 0..ROWS {
                    ui.with_indexed(
                        r as u64,
                        NodeSpec::row().gap(STEP - CELL).height(CELL),
                        |ui| {
                            if !filled {
                                return;
                            }
                            for c in 0..CELLS_PER_ROW {
                                ui.leaf_indexed(
                                    c as u64,
                                    NodeSpec::column()
                                        .size(CELL, CELL)
                                        .bg(color)
                                        .radius(1.0)
                                        .transition(360.0)
                                        .exit(Enter::from(0.0, 28.0).opacity(0.0)),
                                );
                            }
                        },
                    );
                }
            },
        );
    }

    /// Five thousand rows, virtualised, with the `exit` on the container.
    fn list(&self, ui: &mut Ui<'_>) {
        if !self.list {
            return;
        }
        let t = ui.theme();
        widgets::uniform_list(
            ui,
            "list",
            NodeSpec::column()
                .size(200.0, ROWS as f32 * STEP - (STEP - CELL))
                .bg(t.raised)
                .radius(6.0)
                .transition(360.0)
                .exit(Enter::from(0.0, 40.0).opacity(0.0)),
            LIST_ROWS,
            ROW_H,
            |ui, i| {
                ui.text_in(
                    NodeSpec::row().pad(4.0).cross_align(Align::Center),
                    &format!("row {i}"),
                    TextStyle::new(12.0),
                );
            },
        );
    }
}

impl App for BulkExit {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.window_title("kui — the exit budget");
        ui.with(NodeSpec::column().fill().pad(24.0).gap(16.0), |ui| {
            ui.text("the exit budget: a removal animates whole or not at all", {
                TextStyle::new(20.0)
            });
            ui.text(
                "Two grids of 2100 cells, each cell with its own `exit`, and a \
             5000-row list with the `exit` on the list. The store keeps \
             4096 nodes of departing pictures. Clear A: 2100 fit, every cell \
             slides out. Clear both: 4200 in one frame do not fit, so none of \
             them animate — the warning is on stderr — where alpha.7 slid \
             the budget's worth out and blinked the rest. Clear A and then B \
             while A is still fading: B animates whole and A's oldest ghosts \
             give way. Clear the list: five thousand rows go, but the picture \
             is the built slice, so it slides out as one.",
                TextStyle::new(13.0).color(t.muted),
            );
            ui.with(NodeSpec::row().gap(12.0).cross_align(Align::Center), |ui| {
                widgets::button(ui, "fill", Value::map([("kind", "fill".into())]));
                widgets::button(ui, "clear A", Value::map([("kind", "a".into())]));
                widgets::button(ui, "clear B", Value::map([("kind", "b".into())]));
                widgets::button(ui, "clear both", Value::map([("kind", "both".into())]));
                widgets::button(ui, "clear list", Value::map([("kind", "list".into())]));
            });
            ui.with(NodeSpec::row().gap(24.0), |ui| {
                self.grid(ui, "a", self.a, t.accent);
                self.grid(ui, "b", self.b, t.success);
                self.list(ui);
            });
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.kind() {
            Some("fill") => {
                self.a = true;
                self.b = true;
                self.list = true;
            }
            Some("a") => self.a = false,
            Some("b") => self.b = false,
            Some("both") => {
                self.a = false;
                self.b = false;
            }
            Some("list") => self.list = false,
            _ => {}
        }
    }
}

impl Example for BulkExit {
    /// The three rules, by the warning each frame raises or does not.
    fn headless(&mut self, core: &mut kui_native::Core) -> Result<(), String> {
        let mut d = kui_devtools::Drive::new(core, 1200.0, 700.0);
        let budget = |d: &mut kui_devtools::Drive<'_>| {
            d.warnings().iter().any(|w| w.starts_with("exit-budget"))
        };
        d.frame(self);
        d.frame(self);
        d.warnings();
        let step = |d: &mut kui_devtools::Drive<'_>, app: &mut BulkExit, kind: &str| {
            app.on_event(UiEvent::on(
                kui_native::OriginId::HOST,
                kui_native::Key::ROOT,
                Value::map([("kind", kind.into())]),
            ));
            d.advance(0.016);
            d.frame(app);
        };
        step(&mut d, self, "a");
        let refused = budget(&mut d);
        d.check(!refused, "2100 cells in one frame fit the budget")?;
        step(&mut d, self, "fill");
        d.advance(1.0);
        d.frame(self);
        step(&mut d, self, "both");
        let refused = budget(&mut d);
        d.check(refused, "4200 in one frame are refused whole")?;
        step(&mut d, self, "fill");
        step(&mut d, self, "list");
        let refused = budget(&mut d);
        d.check(!refused, "a 5000-row list leaves as its built slice")
    }
}

kui_devtools::main!(BulkExit {
    a: true,
    b: true,
    list: true,
});
