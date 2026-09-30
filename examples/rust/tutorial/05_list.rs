//! Step 5 — a list with keys. Builds on step 4 by drawing a list of
//! items from a `Vec`, each row under a key of its own so it keeps its
//! hover and focus when rows above it come and go, inside a box that
//! scrolls. Chapter 6 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_05_list

use kui_native::widgets;
use kui_native::{Align, App, Message, NodeSpec, TextStyle, Ui, UiEvent};

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Add,
    /// A message can carry data: which item.
    Toggle {
        id: u64,
    },
    Remove {
        id: u64,
    },
    Filter {
        done: bool,
    },
    ShowAll,
}
// ANCHOR_END: message

// ANCHOR: model
struct Item {
    id: u64,
    text: String,
    done: bool,
}

struct Todo {
    items: Vec<Item>,
    next_id: u64,
    /// `None` shows everything; `Some(done)` only those.
    filter: Option<bool>,
}
// ANCHOR_END: model

impl Default for Todo {
    fn default() -> Self {
        let mut todo = Self {
            items: Vec::new(),
            next_id: 1,
            filter: None,
        };
        for _ in 0..3 {
            todo.add();
        }
        todo
    }
}

impl Todo {
    fn add(&mut self) {
        self.items.push(Item {
            id: self.next_id,
            text: format!("Task {}", self.next_id),
            done: false,
        });
        self.next_id += 1;
    }
}

impl App for Todo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(20.0)
                .gap(12.0)
                .cross_align(Align::Start)
                .bg(t.bg),
            |ui| {
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    widgets::button(ui, "Add", Msg::Add);
                    widgets::button(ui, "All", Msg::ShowAll);
                    widgets::button(ui, "Active", Msg::Filter { done: false });
                    widgets::button(ui, "Done", Msg::Filter { done: true });
                });

                // ANCHOR: list
                // A fixed height and `scroll_y`: the rows inside can be
                // taller than the box, and the wheel moves them.
                ui.with(
                    NodeSpec::column()
                        .grow_width()
                        .height(220.0)
                        .scroll_y()
                        .gap(4.0)
                        .pad(8.0)
                        .bg(t.surface)
                        .radius(8.0)
                        .border(1.0, t.border),
                    |ui| {
                        let shown = self
                            .items
                            .iter()
                            .filter(|i| self.filter.is_none_or(|d| i.done == d));
                        for item in shown {
                            // The row's key is the item's id, not its
                            // position. Remove the row above and this one
                            // is still itself: same hover, same focus.
                            ui.with_indexed(
                                item.id,
                                NodeSpec::row()
                                    .grow_width()
                                    .gap(8.0)
                                    .pad_xy(8.0, 4.0)
                                    .radius(6.0)
                                    .cross_align(Align::Center)
                                    .hover_bg(t.sunken),
                                |ui| {
                                    widgets::checkbox(
                                        ui,
                                        &item.text,
                                        item.done,
                                        Msg::Toggle { id: item.id },
                                    );
                                    ui.leaf(NodeSpec::row().grow_width());
                                    widgets::button(ui, "×", Msg::Remove { id: item.id });
                                },
                            );
                        }
                    },
                );
                // ANCHOR_END: list

                let done = self.items.iter().filter(|i| i.done).count();
                ui.text(
                    &format!("{done} of {} done", self.items.len()),
                    TextStyle::new(12.0).color(t.muted),
                );
            },
        );
    }

    // ANCHOR: on_event
    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Add) => self.add(),
            Some(Msg::Toggle { id }) => {
                if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
                    item.done = !item.done;
                }
            }
            Some(Msg::Remove { id }) => self.items.retain(|i| i.id != id),
            Some(Msg::Filter { done }) => self.filter = Some(done),
            Some(Msg::ShowAll) => self.filter = None,
            None => {}
        }
    }
    // ANCHOR_END: on_event
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Todo")
        .size(420.0, 360.0)
        .run(Todo::default())
}
