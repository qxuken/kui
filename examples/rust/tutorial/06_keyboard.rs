//! Step 6 — keys and focus. Builds on step 5 by listening to the
//! keyboard: a sink at the root hears every key as data, the arrows move
//! a cursor over a grid, Tab walks the buttons, and the view reads where
//! focus is. Chapter 7 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_06_keyboard

use kui_native::widgets;
use kui_native::{Align, App, KeyCode, KeyPhase, Message, NodeSpec, TextStyle, Ui, UiEvent};

const SIDE: usize = 4;

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    /// The tag on the key sink: every key press arrives under it.
    Key,
    Pick {
        cell: usize,
    },
    Clear,
}
// ANCHOR_END: message

#[derive(Default)]
struct Grid {
    cursor: usize,
    lit: [bool; SIDE * SIDE],
    last_key: String,
}

impl App for Grid {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        // ANCHOR: sink
        // `on_key` makes this box a key sink. A key goes to the focused
        // node and bubbles up to the nearest sink above it, so the root
        // hears the arrows even while a cell or a button has focus.
        // `take_key_focus` below gives the sink focus on its first frame,
        // so it hears keys before anything is clicked.
        let sink = ui.with_keyed(
            "app",
            NodeSpec::column()
                .fill()
                .pad(20.0)
                .gap(12.0)
                .cross_align(Align::Start)
                .bg(t.bg)
                .on_key(Msg::Key),
            |ui| {
                // ANCHOR_END: sink
                // ANCHOR: readout
                // Where focus is, as the core knows it, by the node's label.
                let focused = ui
                    .focused()
                    .and_then(|k| ui.core().label_of(k).map(str::to_string))
                    .unwrap_or_else(|| "nothing".into());
                let by_keyboard = ui.focus_visible();
                ui.text(
                    &format!(
                        "focus: {focused}{} · last key: {}",
                        if by_keyboard { " (by keyboard)" } else { "" },
                        self.last_key
                    ),
                    TextStyle::new(12.0).color(t.muted),
                );
                // ANCHOR_END: readout

                // ANCHOR: grid
                ui.with(NodeSpec::column().gap(4.0), |ui| {
                    for row in 0..SIDE {
                        ui.with(NodeSpec::row().gap(4.0), |ui| {
                            for col in 0..SIDE {
                                let i = row * SIDE + col;
                                let bg = match (self.lit[i], i == self.cursor) {
                                    (true, _) => t.accent,
                                    (false, true) => t.accent_soft,
                                    (false, false) => t.sunken,
                                };
                                // `focusable` puts a plain box in the Tab
                                // ring; `focus_bg` is what it shows there.
                                // A clickable box is a button to a screen
                                // reader, so `label` gives it a name.
                                ui.leaf_keyed(
                                    &format!("cell {i}"),
                                    NodeSpec::row()
                                        .size(36.0, 36.0)
                                        .radius(6.0)
                                        .bg(bg)
                                        .label(format!("cell {i}"))
                                        .focusable()
                                        .focus_bg(t.accent_hover)
                                        .on_click(Msg::Pick { cell: i }),
                                );
                            }
                        });
                    }
                });
                // ANCHOR_END: grid

                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    widgets::button(ui, "Clear", Msg::Clear);
                    ui.text(
                        "arrows move · space lights · Tab walks · Enter presses",
                        TextStyle::new(12.0).color(t.faint),
                    );
                });
            },
        );
        ui.take_key_focus(sink);
    }

    // ANCHOR: on_event
    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Key) => {
                // The message says which sink; `key_press` reads the key
                // event's own fields off the same payload, typed.
                let Some((KeyPhase::Down, key)) = ev.key_press() else {
                    return;
                };
                self.last_key = format!("{:?}", key.code);
                let (row, col) = (self.cursor / SIDE, self.cursor % SIDE);
                match key.code {
                    KeyCode::Left => self.cursor = row * SIDE + col.saturating_sub(1),
                    KeyCode::Right => self.cursor = row * SIDE + (col + 1).min(SIDE - 1),
                    KeyCode::Up => self.cursor = row.saturating_sub(1) * SIDE + col,
                    KeyCode::Down => self.cursor = (row + 1).min(SIDE - 1) * SIDE + col,
                    KeyCode::Space => self.lit[self.cursor] = !self.lit[self.cursor],
                    _ => {}
                }
            }
            Some(Msg::Pick { cell }) => {
                self.cursor = cell;
                self.lit[cell] = !self.lit[cell];
            }
            Some(Msg::Clear) => self.lit = [false; SIDE * SIDE],
            None => {}
        }
    }
    // ANCHOR_END: on_event
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Keyboard")
        .size(420.0, 320.0)
        .run(Grid::default())
}
