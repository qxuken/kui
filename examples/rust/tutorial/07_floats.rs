//! Step 7 — floats and a modal. Builds on step 6 by drawing over the
//! page: a tooltip the core floats under a hovered node, and a confirm
//! dialog the app declares as a `modal` float and stops declaring when
//! it is answered or dismissed. Chapter 8 of the book.
//!
//! Run: cargo run -p kui-native --example tutorial_07_floats

use kui_native::widgets;
use kui_native::{Align, App, FloatConfig, Message, NodeSpec, TextStyle, Ui, UiEvent};

// ANCHOR: message
#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Ask,
    Confirm,
    Cancel,
    /// The modal's tag: a `dismiss` (Escape, or a press outside) arrives
    /// under it.
    Dialog,
}
// ANCHOR_END: message

#[derive(Default)]
struct Floats {
    asking: bool,
    deleted: u32,
}

impl App for Floats {
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
                // ANCHOR: tooltip
                // `tooltip` is a prop: the node tracks its own hover, and
                // the core floats the hint below it while hovered. The
                // same text is what a screen reader says after the name.
                ui.text_in(
                    NodeSpec::row()
                        .pad_xy(10.0, 6.0)
                        .radius(8.0)
                        .bg(t.raised)
                        .border(1.0, t.border)
                        .tooltip("A float: out of the flow, on top, unclipped"),
                    "hover me",
                    TextStyle::new(13.0).color(t.fg),
                );
                // ANCHOR_END: tooltip

                ui.with(NodeSpec::row().gap(8.0).cross_align(Align::Center), |ui| {
                    widgets::button(ui, "Delete…", Msg::Ask);
                    ui.text(
                        &format!("deleted {} times", self.deleted),
                        TextStyle::new(12.0).color(t.muted),
                    );
                });

                // ANCHOR: modal
                // Declared last, so it stacks on top. `modal` scopes Tab
                // to it, keeps the pointer from what is behind, and turns
                // Escape or a press outside into a `dismiss` event.
                if self.asking {
                    ui.with_keyed(
                        "dialog",
                        NodeSpec::column()
                            .float(FloatConfig::viewport().inside(Align::Center, Align::Center))
                            .modal(Msg::Dialog)
                            .label("Delete?")
                            .width(260.0)
                            .pad(16.0)
                            .gap(12.0)
                            .bg(t.raised)
                            .radius(10.0)
                            .border(1.0, t.border_strong),
                        |ui| {
                            ui.text("Delete the thing?", TextStyle::new(15.0).color(t.fg));
                            ui.text(
                                "This cannot be undone.",
                                TextStyle::new(12.0).color(t.muted),
                            );
                            ui.with(NodeSpec::row().gap(8.0).main_align(Align::End), |ui| {
                                widgets::button(ui, "Cancel", Msg::Cancel);
                                widgets::button(ui, "Delete", Msg::Confirm);
                            });
                        },
                    );
                }
                // ANCHOR_END: modal
            },
        );
    }

    // ANCHOR: on_event
    fn on_event(&mut self, ev: UiEvent) {
        match ev.message::<Msg>() {
            Some(Msg::Ask) => self.asking = true,
            Some(Msg::Confirm) => {
                self.deleted += 1;
                self.asking = false;
            }
            // The core closes nothing. Both a Cancel click and a dismiss
            // end the same way: the view stops declaring the dialog.
            Some(Msg::Cancel) | Some(Msg::Dialog) => self.asking = false,
            None => {}
        }
    }
    // ANCHOR_END: on_event
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    kui_native::app("Floats")
        .size(420.0, 280.0)
        .run(Floats::default())
}
