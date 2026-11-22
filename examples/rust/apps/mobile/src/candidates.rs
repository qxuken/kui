//! Manual software-keyboard fixture. Every trial starts inside the same word.
//! The readout uses KUI's committed model, not OCR of the editor or candidate bar.
use kui_native::{App, Core, Event, Key, NodeSpec, TextStyle, Ui, UiEvent, widgets};

#[derive(kui_native::Message, Clone, Debug, PartialEq)]
enum Msg {
    Start,
    Edit,
}

pub struct Candidates {
    value: String,
    field: Option<Key>,

    caret: usize,
}

impl Default for Candidates {
    fn default() -> Self {
        Self {
            value: "say drain now".into(),
            field: None,
            caret: 5,
        }
    }
}

impl App for Candidates {
    fn before_frame(&mut self, core: &mut Core) {
        if let Some(key) = self.field {
            if let Some((caret, _)) = core.edit.caret_and_selection(key) {
                self.caret = caret;
            }
        }
    }
    fn view(&mut self, ui: &mut Ui<'_>) {
        let color = ui.theme().fg;
        ui.with(
            NodeSpec::column()
                .fill()
                .padding(ui.safe_area())
                .gap(12.)
                .bg(ui.theme().bg),
            |ui| {
                self.field = Some(widgets::text_input_with(
                    ui,
                    "Candidate trial",
                    &self.value,
                    true,
                    &widgets::InputMessages {
                        change: Some(Msg::Edit.into()),
                        ..Default::default()
                    },
                ));
                ui.text(
                    &format!("Caret byte {}", self.caret),
                    TextStyle::new(17.).color(color),
                );
                // Show the exact committed scalars independently of the field's glyphs.
                let scalars = self
                    .value
                    .chars()
                    .map(|c| {
                        if c.is_ascii() {
                            c.to_string()
                        } else {
                            format!("[{:04X}]", c as u32)
                        }
                    })
                    .collect::<String>();
                ui.text(&scalars, TextStyle::new(16.).color(color));
                widgets::button(ui, "Start trial", Msg::Start);
            },
        );
    }
    fn on_event_with(&mut self, event: UiEvent, core: &mut Core) {
        if let Event::EditChange(change) = event.read() {
            self.value = change.text;
        } else if event.message::<Msg>() == Some(Msg::Start)
            && let Some(key) = self.field
        {
            self.value = "say drain now".into();
            core.set_edit_text(key, &self.value);
            core.set_focus(Some(key));
            core.edit.set_selection(key, (0, 5), (0, 5));
        }
    }
}
