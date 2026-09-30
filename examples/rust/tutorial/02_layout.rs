//! Step 2 — rows, columns and sizes. Builds on step 1 by putting the text
//! inside boxes: a header row, a body with a fixed sidebar beside a
//! growing content area, and a footer. Chapter 3 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_02_layout

use kui_native::{Align, App, NodeSpec, TextStyle, Ui};

struct Layout;

impl App for Layout {
    // ANCHOR: view
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();

        // ANCHOR: root
        // The root: a column that fills the window. `with` opens a box,
        // runs the closure for its children, and closes it.
        ui.with(NodeSpec::column().fill().bg(t.bg), |ui| {
            // ANCHOR_END: root
            // ANCHOR: header
            // A row: children go left to right. `SpaceBetween` pushes the
            // title and the badge to opposite ends.
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .pad(16.0)
                    .main_align(Align::SpaceBetween)
                    .cross_align(Align::Center)
                    .bg(t.surface)
                    .border(1.0, t.border),
                |ui| {
                    ui.text("Layout", TextStyle::new(18.0).color(t.fg));
                    ui.text_in(
                        NodeSpec::row()
                            .pad_xy(10.0, 4.0)
                            .radius(10.0)
                            .bg(t.accent_soft),
                        "step 2",
                        TextStyle::new(12.0).color(t.accent),
                    );
                },
            );
            // ANCHOR_END: header

            // ANCHOR: body
            // The body takes whatever height the header and footer leave.
            ui.with(NodeSpec::row().grow_width().grow_height(), |ui| {
                // A fixed width...
                ui.with(
                    NodeSpec::column()
                        .width(160.0)
                        .grow_height()
                        .pad(16.0)
                        .gap(8.0)
                        .bg(t.sunken),
                    |ui| {
                        for item in ["Inbox", "Drafts", "Sent"] {
                            ui.text(item, TextStyle::new(14.0).color(t.muted));
                        }
                    },
                );
                // ...and the rest. `grow_width` shares the leftover space;
                // with one grower it takes all of it.
                ui.with(
                    NodeSpec::column()
                        .grow_width()
                        .grow_height()
                        .center()
                        .gap(8.0),
                    |ui| {
                        ui.text("Sizes are three words", TextStyle::new(20.0).color(t.fg));
                        ui.text(
                            "fit (the default), fixed, or grow",
                            TextStyle::new(14.0).color(t.muted),
                        );
                    },
                );
            });
            // ANCHOR_END: body

            // ANCHOR: footer
            ui.text_in(
                NodeSpec::row().grow_width().pad_xy(16.0, 8.0).bg(t.surface),
                "a footer, as tall as its text",
                TextStyle::new(12.0).color(t.faint),
            );
            // ANCHOR_END: footer
        });
    }
    // ANCHOR_END: view
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Layout").size(560.0, 360.0).run(Layout)
}
