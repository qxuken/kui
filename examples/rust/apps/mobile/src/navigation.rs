//! One note and its editor exercise mobile Back, nested scroll and interruption.
use kui_native::{
    Align, App, Core, Edges, Event, FloatConfig, Key, KeyCode, Message, NodeSpec, Role, TextStyle,
    Ui, UiEvent, widgets,
};
use std::path::PathBuf;

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    Open,
    Back,
    ShowEditor,
    CloseEditor,
    HideKeyboard,
    Edit,
    Increment,
    Slide,
}

#[derive(Default)]
pub struct Trial {
    detail: bool,
    dialog: bool,
    draft: String,
    directory: Option<PathBuf>,
    taps: u32,
    slider: f32,
    outer: Option<Key>,
    inner: Option<Key>,
    hide_keyboard: Option<Key>,
}

impl Trial {
    pub fn restore(dir: Option<PathBuf>) -> Self {
        let directory = dir.filter(|path| match std::fs::create_dir_all(path) {
            Ok(()) => true,
            Err(err) => {
                eprintln!("opening navigation draft directory: {err}");
                false
            }
        });
        let draft = directory
            .as_ref()
            .and_then(
                |path| match std::fs::read_to_string(path.join("note.txt")) {
                    Ok(text) => Some(text),
                    Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
                    Err(err) => {
                        eprintln!("reading navigation draft: {err}");
                        None
                    }
                },
            )
            .unwrap_or_default();
        Self {
            directory,
            draft,
            ..Self::default()
        }
    }

    fn detail(&mut self, ui: &mut Ui<'_>) {
        let theme = ui.theme();
        let label = TextStyle::new(16.).color(theme.fg);
        let offset = |key: Option<Key>| key.map_or(0., |key| ui.scroll_offset(key).y);
        let (outer, inner) = (offset(self.outer), offset(self.inner));
        ui.with(NodeSpec::row().gap(8.), |ui| {
            widgets::button(ui, "Back", Msg::Back);
            ui.text("Note detail", label.clone());
            widgets::button(ui, "Open editor", Msg::ShowEditor);
        });
        ui.text(&format!("Draft: {}", self.draft), label.clone());
        ui.text(
            &format!("Taps: {}  Slider: {:.0}", self.taps, self.slider),
            label.clone(),
        );
        ui.text(
            &format!("Outer: {outer:.0}  Inner: {inner:.0}"),
            label.clone(),
        );
        self.outer = Some(ui.with_keyed(
            "outer",
            NodeSpec::column().fill().scroll_y().gap(12.),
            |ui| {
                widgets::button(ui, "Increment", Msg::Increment);
                widgets::slider(ui, "Slider", self.slider, 0., 100., 1., Msg::Slide);
                self.inner = Some(
                    ui.with_keyed(
                        "inner",
                        NodeSpec::column()
                            .grow_width()
                            .height(156.)
                            .scroll_y()
                            .gap(8.)
                            .bg(theme.sunken),
                        |ui| {
                            for row in 1..=8 {
                                ui.text(&format!("Inner row {row}"), label.clone());
                            }
                        },
                    ),
                );
                for row in 1..=28 {
                    ui.text(&format!("Outer row {row}"), label.clone());
                }
            },
        ));
    }
}

impl App for Trial {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let theme = ui.theme();
        let safe = ui.safe_area();
        ui.with(
            NodeSpec::column()
                .fill()
                .padding(Edges {
                    l: safe.l + 12.,
                    r: safe.r + 12.,
                    ..safe
                })
                .gap(12.)
                .bg(theme.bg)
                .on_key(Msg::Back),
            |ui| {
                if self.detail {
                    self.detail(ui);
                } else {
                    ui.text("Notes", TextStyle::new(22.).color(theme.fg));
                    widgets::button(ui, "Open note", Msg::Open);
                }
            },
        );
        if self.dialog {
            ui.with_keyed(
                "editor",
                NodeSpec::column()
                    .width(280.)
                    .pad(16.)
                    .gap(12.)
                    .bg(theme.raised)
                    .float(FloatConfig::viewport().inside(Align::Center, Align::Center))
                    .modal(Msg::CloseEditor)
                    .role(Role::Dialog)
                    .label("Edit draft"),
                |ui| {
                    ui.text("Edit draft", TextStyle::new(20.).color(theme.fg));
                    widgets::text_input_with(
                        ui,
                        "Draft",
                        &self.draft,
                        true,
                        &widgets::InputMessages {
                            change: Some(Msg::Edit.into()),
                            submit: Some(Msg::HideKeyboard.into()),
                            ..Default::default()
                        },
                    );
                    self.hide_keyboard = Some(ui.child_key("Hide keyboard"));
                    widgets::button(ui, "Hide keyboard", Msg::HideKeyboard);
                    widgets::button(ui, "Close editor", Msg::CloseEditor);
                },
            );
        }
    }

    fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
        match ev.split::<Msg>() {
            (Some(Msg::Open), Event::Click(_)) => self.detail = true,
            (Some(Msg::Back), Event::Click(_)) => self.detail = false,
            (Some(Msg::Back), Event::Key(_, key))
                if key.code == KeyCode::Escape && !self.dialog =>
            {
                self.detail = false;
            }
            (Some(Msg::ShowEditor), Event::Click(_)) => self.dialog = true,
            (Some(Msg::CloseEditor), Event::Click(_) | Event::Dismiss(_)) => self.dialog = false,
            (Some(Msg::HideKeyboard), _) => core.set_focus(self.hide_keyboard),
            (Some(Msg::Edit), Event::EditChange(change)) => {
                self.draft = change.text;
                super::save_draft(self.directory.as_deref(), "note.txt", &self.draft);
            }
            (Some(Msg::Increment), Event::Click(_)) => self.taps += 1,
            (Some(Msg::Slide), Event::SliderChange(change)) => self.slider = change.value as f32,
            _ => {}
        }
    }
}
