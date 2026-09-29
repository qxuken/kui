//! Step 10 — a test without a window. Builds on step 5's list by giving
//! it a `mod tests` that drives the real `view` and `on_event` headless:
//! clicks by label, a key, and assertions on what the frame drew.
//! Chapter 11 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_10_testing
//! Test: cargo test -p kui-native --example tutorial_10_testing

use kui_native::widgets;
use kui_native::{Align, App, KeyCode, KeyPhase, Message, NodeSpec, TextStyle, Ui, UiEvent};

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Add,
    Toggle {
        id: u64,
    },
    Clear,
    /// The root sink: `a` adds, so a test can press a key too.
    Key,
}

struct Item {
    id: u64,
    text: String,
    done: bool,
}

#[derive(Default)]
struct Todo {
    items: Vec<Item>,
    next_id: u64,
}

impl Todo {
    fn add(&mut self) {
        self.next_id += 1;
        self.items.push(Item {
            id: self.next_id,
            text: format!("Task {}", self.next_id),
            done: false,
        });
    }
}

impl App for Todo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
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
                ui.with(NodeSpec::row().gap(8.0), |ui| {
                    widgets::button(ui, "Add", Msg::Add);
                    widgets::button(ui, "Clear", Msg::Clear);
                });
                // ANCHOR: labels
                // A keyed box has a name the frame remembers. A test reads
                // the frame by those names, as it clicks a button by its
                // text.
                ui.with_keyed("list", NodeSpec::column().gap(4.0), |ui| {
                    for item in &self.items {
                        ui.with_indexed(item.id, NodeSpec::row(), |ui| {
                            widgets::checkbox(
                                ui,
                                &item.text,
                                item.done,
                                Msg::Toggle { id: item.id },
                            );
                        });
                    }
                });
                let done = self.items.iter().filter(|i| i.done).count();
                ui.text_in_keyed(
                    "summary",
                    NodeSpec::row(),
                    &format!("{done} of {} done", self.items.len()),
                    TextStyle::new(12.0).color(t.muted),
                );
                // ANCHOR_END: labels
            },
        );
        ui.take_key_focus(sink);
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Add) => self.add(),
            Some(Msg::Toggle { id }) => {
                if let Some(item) = self.items.iter_mut().find(|i| i.id == id) {
                    item.done = !item.done;
                }
            }
            Some(Msg::Clear) => self.items.clear(),
            Some(Msg::Key) => {
                if let Some((KeyPhase::Down, key)) = ev.key_press()
                    && key.code == KeyCode::Char('a')
                {
                    self.add();
                }
            }
            None => {}
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Testing")
        .size(360.0, 300.0)
        .run(Todo::default())
}

// ANCHOR: tests
#[cfg(test)]
mod tests {
    use super::*;
    use kui_native::testing::Drive;
    use kui_native::{Core, KeyMods};

    /// A drive over a core and a viewport: the same `view` and `on_event`
    /// the window runs, with no window. `framing` builds a frame after
    /// every gesture, so the view has caught up before the next assert.
    fn drive() -> Drive {
        Drive::new(Core::new(), 360.0, 300.0).framing()
    }

    #[test]
    fn a_click_adds_and_a_click_ticks() {
        let mut app = Todo::default();
        let mut d = drive();
        d.frame(&mut app);
        assert_eq!(d.texts_under("summary"), ["0 of 0 done"]);

        // Click by label: how a screen reader presses, so no geometry.
        let add = d.key_of("Add").expect("an Add button");
        d.click_key(&mut app, add);
        d.click_key(&mut app, add);
        assert_eq!(app.items.len(), 2);
        assert_eq!(d.texts_under("list"), ["Task 1", "Task 2"]);

        let first = d.key_of("Task 1").expect("the first row's checkbox");
        d.click_key(&mut app, first);
        assert!(app.items[0].done);
        assert_eq!(d.texts_under("summary"), ["1 of 2 done"]);
    }

    #[test]
    fn a_key_adds_too() {
        let mut app = Todo::default();
        let mut d = drive();
        d.frame(&mut app);
        d.key(&mut app, "a", KeyMods::NONE);
        assert_eq!(app.items.len(), 1);
        // Nothing the core saw was misdeclared.
        assert!(d.warnings().is_empty());
    }
}
// ANCHOR_END: tests
