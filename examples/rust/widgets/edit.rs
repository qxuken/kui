//! The `edit` element: a multiline document and the single-line
//! `widgets::text_input` beside it, both the same widget under two
//! options. Cursor motion, selection (shift+arrows, drag, double-click a
//! word), clipboard (Cmd/Ctrl C/X/V/A), undo, and scrolling as the
//! document grows are the core's; the view declares the editor and reads
//! its text back (`ui.edit_text`) — the buffer lives in the core keyed by
//! the widget's identity, so it survives every rebuild of the tree.
//!
//! What arrives in `on_event` is what each editor declared: the
//! document's `on_change` message on every edit, with the whole text
//! beside it under `change`, and from the single-line field its own
//! change message and, on Enter, its submit message as declared. An
//! editor that declares none of them sends nothing.
//!
//! Run: cargo run -p kui-native --example edit [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::widgets::{self, InputMessages};
use kui_native::{
    Align, App, Core, EditOptions, Event, FontFamily, Key, Message, NodeSpec, TextStyle, Ui,
    UiEvent,
};

const INITIAL: &str = "\
# kui edit

Type here. Everything works the way you'd expect:

  - arrows, home/end, page up/down (+shift to select)
  - alt+arrows for word motion, cmd+arrows for line/document
  - click to place the caret, drag to select, double-click a word
  - cmd+c / cmd+x / cmd+v / cmd+a, cmd+z / cmd+shift+z
  - enter, tab, backspace, delete

The buffer lives in the core keyed by widget identity, so this
text survives every rebuild of the UI tree - the view below is
regenerated from scratch every frame, like any other kui view.
";

#[derive(Message, Clone, Debug, PartialEq)]
enum Msg {
    /// The document changed: its text arrives beside, under `change`.
    Doc,
    /// The title field changed, the same way.
    Title,
    /// Enter in the title field: the message as declared, nothing more.
    Submit,
}

#[derive(Default)]
struct Edit {
    doc: Option<Key>,
    chars: usize,
    lines: usize,
    edited: bool,
    /// The title field's text, as its last `change` said.
    title: String,
    /// What the single-line field last submitted.
    submitted: Option<String>,
}

impl App for Edit {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.with(NodeSpec::column().fill(), |ui| {
            // The single-line field: `widgets::text_input` is the same
            // element with `multiline: false`, chrome, and a focus ring.
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .pad(12.0)
                    .gap(12.0)
                    .cross_align(Align::Center)
                    .bg(t.surface)
                    .border(1.0, t.border),
                |ui| {
                    ui.text("title", TextStyle::new(12.0).color(t.muted));
                    widgets::text_input_with(
                        ui,
                        "title",
                        "",
                        false,
                        &InputMessages {
                            change: Some(Msg::Title.into()),
                            submit: Some(Msg::Submit.into()),
                            cancel: None,
                        },
                    );
                    ui.text(
                        &match &self.submitted {
                            Some(s) => format!("submitted: {s:?}"),
                            None => "Enter submits".into(),
                        },
                        TextStyle::new(12.0).color(t.muted),
                    );
                },
            );

            // The document: the edit node grows its height with content
            // inside a scroll container, so the document scrolls as it
            // grows.
            ui.with(NodeSpec::column().fill().scroll_y(), |ui| {
                let key = ui.text_edit(
                    "doc",
                    INITIAL,
                    &EditOptions {
                        style: TextStyle::new(14.0)
                            .family(FontFamily::Mono)
                            .line_height(22.0),
                        multiline: true,
                        autofocus: true,
                        ..Default::default()
                    },
                    // Nothing inside an editor names it, so
                    // `control-without-name` is right to ask: a screen
                    // reader would say "text input". `on_change` is what
                    // makes it report its edits at all.
                    NodeSpec::column()
                        .grow_width()
                        .pad(20.0)
                        .label("document")
                        .on_change(Msg::Doc),
                );
                self.doc = Some(key);
                // The first count is of the seed, read back once; after
                // that every edit brings its text with it.
                if self.chars == 0
                    && let Some(text) = ui.edit_text(key)
                {
                    self.recount(&text);
                }
            });

            // Status bar.
            ui.with(
                NodeSpec::row()
                    .grow_width()
                    .pad_xy(12.0, 6.0)
                    .gap(16.0)
                    .bg(t.surface)
                    .border(1.0, t.border)
                    .cross_align(Align::Center),
                |ui| {
                    let muted = TextStyle::new(12.0).color(t.muted);
                    ui.text(&format!("{} lines", self.lines), muted);
                    ui.text(&format!("{} chars", self.chars), muted);
                    ui.leaf(NodeSpec::row().grow_width());
                    ui.text(
                        if self.edited { "edited" } else { "saved" },
                        TextStyle::new(12.0).color(if self.edited { t.warning } else { t.success }),
                    );
                },
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.split::<Msg>() {
            (Some(Msg::Doc), Event::EditChange(c)) => {
                self.edited = true;
                self.recount(&c.text);
            }
            (Some(Msg::Title), Event::EditChange(c)) => self.title = c.text,
            (Some(Msg::Submit), _) => self.submitted = Some(self.title.clone()),
            _ => {}
        }
    }
}

impl Edit {
    fn recount(&mut self, text: &str) {
        self.chars = text.chars().count();
        self.lines = text.lines().count().max(1);
    }
}

impl Example for Edit {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("Shift-arrows", "select"),
        ("⌥/⌘ arrows", "word / line motion"),
        ("⌘C ⌘X ⌘V", "clipboard"),
        ("⌘Z", "undo"),
        ("Enter", "submits the field"),
    ];

    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 640.0, 400.0);
        d.frame(self);
        let doc = self.doc.ok_or("no document")?;
        let lines = self.lines;
        d.check(
            lines == INITIAL.lines().count(),
            "the initial text is counted",
        )?;

        // Typing lands in the document (it autofocused) and the status
        // bar sees the change through its `on_change` message.
        d.text(self, "x");
        d.frame(self);
        d.check(
            self.edited && self.chars == INITIAL.chars().count() + 1,
            "a keystroke reports `change`, with the text",
        )?;
        let text = d.core.edit_text(doc).unwrap_or_default();
        d.check(text.contains('x'), "and the text read back has it")?;

        // The single-line field submits on Enter.
        let title = d.key_of("title").ok_or("no title field")?;
        d.focus(self, title);
        d.text(self, "hello");
        d.key(self, "enter", Default::default());
        d.frame(self);
        d.check(
            self.submitted.as_deref() == Some("hello"),
            "Enter submits the field's text",
        )
    }
}

kui_devtools::main!(Edit::default());
