//! A scrollable notes list and a stock multiline editor in the browser.

#![cfg(target_arch = "wasm32")]

mod host;

use kui_core::{
    EditOptions, Event, Live, Message, NodeSpec, Role, TextStyle, Ui, UiEvent, widgets,
};
use kui_web::{App, Runner};

type AppModel = Notes;

fn make_runner() -> Runner<Notes> {
    Runner::new(Notes::new(), &host::session())
}

#[derive(Message, Clone, Debug, PartialEq)]
#[message(crate = "kui_core")]
enum Msg {
    Open(usize),
    Edit(usize),
    Save,
}

struct Note {
    draft: String,
    saved: String,
}

struct Notes {
    notes: Vec<Note>,
    active: usize,
    saved: bool,
}

impl Notes {
    fn new() -> Self {
        Self {
            notes: (1..=32)
                .map(|n| {
                    let draft = if n == 1 {
                        "A little room for ideas.\n\nTry typing, selecting text, or pasting a line here.".into()
                    } else {
                        format!("This is note {n:02}.\n\nMake it yours.")
                    };
                    Note { saved: draft.clone(), draft }
                })
                .collect(),
            active: 0,
            saved: false,
        }
    }

    fn editor(&self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let note = &self.notes[self.active];
        let title = format!("Note {:02}", self.active + 1);
        ui.text_in(
            NodeSpec::row().role(Role::Heading),
            &title,
            TextStyle::new(24.0),
        );
        ui.with_indexed(self.active as u64, NodeSpec::column().fill(), |ui| {
            let options = EditOptions {
                multiline: true,
                tab_moves_focus: true,
                style: TextStyle::new(16.0).line_height(25.0),
                ..Default::default()
            };
            ui.text_edit_value(
                "draft",
                &note.draft,
                &options,
                NodeSpec::column()
                    .fill()
                    .pad(16.0)
                    .radius(8.0)
                    .bg(t.sunken)
                    .border(1.0, t.border)
                    .label("Note text")
                    .on_change(Msg::Edit(self.active)),
            );
        });
        ui.with(NodeSpec::row().gap(12.0).center(), |ui| {
            widgets::button(ui, "Save note", Msg::Save);
            let status = if note.draft != note.saved {
                "Unsaved changes"
            } else if self.saved {
                "Saved in this tab"
            } else {
                "Changes stay in this tab"
            };
            ui.text_in(
                NodeSpec::row().live(Live::Polite),
                status,
                TextStyle::new(13.0).color(t.muted),
            );
        });
    }

    fn list(&self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.text_in(
            NodeSpec::row().role(Role::Heading),
            "Notes",
            TextStyle::new(18.0),
        );
        ui.with(
            NodeSpec::column()
                .fill()
                .gap(4.0)
                .scroll_y()
                .role(Role::List)
                .label("Notes"),
            |ui| {
                for i in 0..self.notes.len() {
                    ui.with_indexed(
                        i as u64,
                        NodeSpec::column()
                            .grow_width()
                            .role(Role::ListItem)
                            .selected(i == self.active),
                        |ui| {
                            let label = format!("Note {:02}", i + 1);
                            let active = i == self.active;
                            let color = if active {
                                t.accent.with_alpha(0.28)
                            } else {
                                t.surface
                            };
                            ui.with_keyed(
                                "open",
                                NodeSpec::row()
                                    .grow_width()
                                    .height(38.0)
                                    .pad_xy(10.0, 6.0)
                                    .radius(6.0)
                                    .bg(color)
                                    .hover_bg(t.raised)
                                    .role(Role::Button)
                                    .label(label.clone())
                                    .cursor(kui_core::CursorShape::Pointer)
                                    .focusable()
                                    .on_click(Msg::Open(i)),
                                |ui| ui.text(&label, TextStyle::new(14.0).color(t.fg)),
                            );
                        },
                    );
                }
            },
        );
    }
}

impl App for Notes {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        let compact = ui.viewport().w < 520.0;
        let split = if compact {
            NodeSpec::column()
        } else {
            NodeSpec::row()
        };
        let list = if compact {
            NodeSpec::column().grow_width().height(152.0)
        } else {
            NodeSpec::column().width(190.0).grow_height()
        };
        ui.with(split.fill().pad(16.0).gap(12.0), |ui| {
            ui.with(
                NodeSpec::column()
                    .fill()
                    .pad(16.0)
                    .gap(12.0)
                    .radius(10.0)
                    .bg(t.surface),
                |ui| self.editor(ui),
            );
            ui.with(list.pad(12.0).gap(8.0).radius(10.0).bg(t.surface), |ui| {
                self.list(ui)
            });
        });
    }

    fn on_event(&mut self, event: UiEvent) {
        match event.split::<Msg>() {
            (Some(Msg::Open(i)), _) => {
                self.active = i;
                self.saved = false;
            }
            (Some(Msg::Edit(i)), Event::EditChange(change)) => {
                self.notes[i].draft = change.text;
                if i == self.active {
                    self.saved = false;
                }
            }
            (Some(Msg::Save), _) => {
                let note = &mut self.notes[self.active];
                note.saved.clone_from(&note.draft);
                self.saved = true;
            }
            _ => {}
        }
    }
}
