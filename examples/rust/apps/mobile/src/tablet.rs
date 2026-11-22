//! Two documents in a shared store, with a core and editor for every scene.
use kui_native::{
    App, Core, Event, Key, Message, NodeSpec, TextStyle, Ui, UiEvent, Vec2, mobile, widgets,
};
use std::{cell::RefCell, path::PathBuf, rc::Rc};

struct Store {
    drafts: [String; 2],
    dir: PathBuf,
    windows: std::collections::HashMap<String, mobile::Waker>,
}

struct Note {
    store: Rc<RefCell<Store>>,
    session: String,
    index: usize,
    session_file: String,
    saved: Position,
    restore: bool,
}

#[derive(Default, PartialEq)]
struct Position {
    scroll: f32,
    anchor: usize,
    caret: usize,
    focused: bool,
}

impl Position {
    fn read(record: &str) -> Self {
        let mut fields = record.split_whitespace().skip(1);
        let scroll = fields
            .next()
            .and_then(|v| v.parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v >= 0.)
            .unwrap_or_default();
        Self {
            scroll,
            anchor: fields
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or_default(),
            caret: fields
                .next()
                .and_then(|v| v.parse().ok())
                .unwrap_or_default(),
            focused: fields.next() == Some("true"),
        }
    }
}

impl Note {
    fn checkpoint(&mut self, core: &mut Core, field: Key, list: Key) {
        if self.restore {
            if self.saved.focused {
                core.set_focus(Some(field));
            }
            core.edit
                .set_selection(field, (0, self.saved.anchor), (0, self.saved.caret));
            core.set_scroll(list, Vec2::new(0., self.saved.scroll));
            self.restore = false;
            return;
        }
        let Some((caret, selection)) = core.edit.caret_and_selection(field) else {
            return;
        };
        let position = Position {
            scroll: core.scroll_offset(list).y,
            anchor: selection.map_or(caret, |(a, b)| if caret == a { b } else { a }),
            caret,
            focused: core.focus() == Some(field),
        };
        if position != self.saved {
            let note = ["A", "B"][self.index];
            let Position {
                scroll,
                anchor,
                caret,
                focused,
            } = position;
            super::save_draft(
                Some(&self.store.borrow().dir),
                &self.session_file,
                &format!("{note} {scroll} {anchor} {caret} {focused}"),
            );
            self.saved = position;
        }
    }
}

#[derive(Clone, Debug, PartialEq, Message)]
enum Msg {
    Edit,
    OpenOther,
}

impl App for Note {
    fn setup(&mut self, waker: mobile::Waker) {
        self.store
            .borrow_mut()
            .windows
            .insert(self.session.clone(), waker);
    }
    fn teardown(&mut self) {
        self.store.borrow_mut().windows.remove(&self.session);
    }

    fn view(&mut self, ui: &mut Ui<'_>) {
        let theme = ui.theme();
        ui.with(
            NodeSpec::column()
                .fill()
                .padding(ui.safe_area())
                .bg(theme.bg)
                .gap(12.),
            |ui| {
                let title = ["Note A", "Note B"][self.index];
                ui.with(NodeSpec::row().grow_width().center(), |ui| {
                    ui.text(title, TextStyle::new(28.).color(theme.fg));
                });
                widgets::button(
                    ui,
                    ["Open B in New Window", "Open A in New Window"][self.index],
                    Msg::OpenOther,
                );
                let field = widgets::text_input_with(
                    ui,
                    title,
                    &self.store.borrow().drafts[self.index],
                    true,
                    &widgets::InputMessages {
                        change: Some(Msg::Edit.into()),
                        ..Default::default()
                    },
                );
                let list = ui.with(NodeSpec::column().fill().scroll_y().gap(12.), |ui| {
                    for n in 1..=48 {
                        ui.text(
                            &format!("{title} paragraph {n}"),
                            TextStyle::new(18.).color(theme.fg),
                        );
                    }
                });
                self.checkpoint(ui.core(), field, list);
            },
        );
    }

    fn on_event_with(&mut self, event: UiEvent, _: &mut Core) {
        match event.split::<Msg>() {
            (Some(Msg::OpenOther), _) => mobile::open_scene(&self.session, ["B", "A"][self.index]),
            (Some(Msg::Edit), Event::EditChange(change)) => {
                let mut store = self.store.borrow_mut();
                store.drafts[self.index] = change.text;
                for (id, window) in &store.windows {
                    if id != &self.session {
                        window.wake();
                    }
                }
                super::save_draft(
                    Some(&store.dir),
                    ["A.txt", "B.txt"][self.index],
                    &store.drafts[self.index],
                );
            }
            _ => {}
        }
    }
}

pub fn start() {
    let dir = PathBuf::from(std::env::var_os("HOME").expect("sandbox home"))
        .join("Library/Application Support/KUI/tablet");
    std::fs::create_dir_all(&dir).expect("tablet draft directory");
    let drafts = ["A", "B"].map(|name| {
        std::fs::read_to_string(dir.join(format!("{name}.txt")))
            .unwrap_or_else(|_| format!("Draft {name}"))
    });
    let store = Rc::new(RefCell::new(Store {
        drafts,
        dir,
        windows: Default::default(),
    }));
    let fonts = super::session();
    mobile::run_scenes(move |session, document| {
        // UIKit can reconnect an existing session after abrupt termination
        // without delivering its restoration activity.
        let session_file = format!("scene-{}.txt", session.replace('/', "_"));
        let store_ref = store.borrow();
        let record = std::fs::read_to_string(store_ref.dir.join(&session_file)).unwrap_or_default();
        let previous = record.split_whitespace().next();
        let note = if document.is_empty() {
            previous.unwrap_or("A")
        } else {
            document
        };
        let saved = if previous == Some(note) {
            Position::read(&record)
        } else {
            super::save_draft(Some(&store_ref.dir), &session_file, note);
            Position::default()
        };
        let index = usize::from(note == "B");
        drop(store_ref);
        (
            Core::new_in(&fonts),
            Note {
                session: session.into(),
                index,
                session_file,
                saved,
                restore: true,
                store: store.clone(),
            },
        )
    })
    .expect("iPad scenes");
}
