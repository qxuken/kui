//! The exit budget at its boundary (`docs/adr/0012-the-exit-budget.md`).
//!
//! `exit` copies a departing subtree out of the last frame that had it and
//! replays the copy, and the store that holds those copies is bounded at
//! 512 nodes. `toasts` never gets near it. This one does, on purpose, so
//! the three rules the budget follows are each a button:
//!
//!   - **whole or not at all.** Two grids of 300 cells, every cell with its
//!     own `exit`. *clear A* drops 300 nodes in one frame: under the budget,
//!     so every cell slides out. *clear both* drops 600 in one frame: over
//!     it, so **nothing** animates — the grids vanish at once, exactly as
//!     they would with no `exit` declared — and the runner prints the
//!     `exit-budget` warning on stderr, naming the frame's count. Before
//!     ADR 0012 that frame animated 512 of the 600 and blinked the rest.
//!   - **a new removal outranks the ghosts in flight.** *clear A*, then
//!     *clear B* while A is still fading: B fits the budget but not the
//!     room beside A's 300 ghosts, so A's oldest give way and every cell of
//!     B animates. The removal you just caused is the one you are looking
//!     at.
//!   - **put `exit` on the list, and make the list small.** The right-hand
//!     column is a `uniform_list` of 1000 rows with the `exit` on the
//!     container. *clear list* removes a thousand rows and the picture the
//!     store keeps is the built slice — the dozen rows on screen and two
//!     spacers — so it slides out whole. A plain thousand-row column with
//!     the same `exit` would be 1001 nodes and refused: the budget counts
//!     what is copied, and virtualisation is what keeps that count small.
//!
//! Run: cargo run -p kui-native --example exit_budget

use kui_devtools::Example;
use kui_native::{
    Align, App, Color, Enter, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value, widgets,
};

/// Cells per grid. One grid is under the budget; both together are over it.
const CELLS_PER_ROW: usize = 20;
const ROWS: usize = 15;
const LIST_ROWS: usize = 1000;
const ROW_H: f32 = 22.0;

struct BulkExit {
    a: bool,
    b: bool,
    list: bool,
}

impl BulkExit {
    /// A grid of cells that each carry their own `exit`. The rows are plain
    /// containers: when the grid goes they vanish, and what departs is the
    /// 300 one-node cells inside them.
    fn grid(&self, ui: &mut Ui<'_>, key: &str, filled: bool, color: Color) {
        ui.with_keyed(
            key,
            NodeSpec::column()
                .width(Sizing::Fixed(CELLS_PER_ROW as f32 * 14.0 - 2.0))
                .height(Sizing::Fixed(ROWS as f32 * 14.0 - 2.0))
                .gap(2.0),
            |ui| {
                if !filled {
                    return;
                }
                for r in 0..ROWS {
                    ui.with_indexed(r as u64, NodeSpec::row().gap(2.0), |ui| {
                        for c in 0..CELLS_PER_ROW {
                            ui.with_indexed(
                                c as u64,
                                NodeSpec::column()
                                    .width(Sizing::Fixed(12.0))
                                    .height(Sizing::Fixed(12.0))
                                    .bg(color)
                                    .radius(2.0)
                                    .transition(360.0)
                                    .exit(Enter::from(0.0, 28.0).opacity(0.0)),
                                |_| {},
                            );
                        }
                    });
                }
            },
        );
    }

    /// A thousand rows, virtualised, with the `exit` on the container.
    fn list(&self, ui: &mut Ui<'_>) {
        if !self.list {
            return;
        }
        let t = ui.theme();
        widgets::uniform_list(
            ui,
            "list",
            NodeSpec::column()
                .width(Sizing::Fixed(200.0))
                .height(Sizing::Fixed(ROWS as f32 * 14.0 - 2.0))
                .bg(t.raised)
                .radius(6.0)
                .transition(360.0)
                .exit(Enter::from(0.0, 40.0).opacity(0.0)),
            LIST_ROWS,
            ROW_H,
            |ui, i| {
                ui.with(NodeSpec::row().pad(4.0).cross_align(Align::Center), |ui| {
                    ui.text(&format!("row {i}"), TextStyle::new(12.0));
                });
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
                "Two grids of 300 cells, each cell with its own `exit`, and a \
             thousand-row list with the `exit` on the list. The store keeps \
             512 nodes of departing pictures. Clear A: 300 fit, every cell \
             slides out. Clear both: 600 in one frame do not fit, so none of \
             them animate — the warning is on stderr — where alpha.7 slid \
             512 out and blinked 88. Clear A and then B while A is still \
             fading: B animates whole and A's oldest ghosts give way. Clear \
             the list: a thousand rows go, but the picture is the built \
             slice, so it slides out as one.",
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
        match ev.payload.get("kind").and_then(Value::as_str) {
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

impl Example for BulkExit {}

kui_devtools::main!(BulkExit {
    a: true,
    b: true,
    list: true,
});
