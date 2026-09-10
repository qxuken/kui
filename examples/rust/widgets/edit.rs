//! The `edit` element: a multiline document and the single-line
//! `widgets::text_input` beside it, both the same widget under two
//! options. Cursor motion, selection (shift+arrows, drag, double-click a
//! word), clipboard (Cmd/Ctrl C/X/V/A), undo, and scrolling as the
//! document grows are the core's; the view declares the editor and reads
//! its text back (`ui.edit_text`) — the buffer lives in the core keyed by
//! the widget's identity, so it survives every rebuild of the tree.
//!
//! What arrives in `on_event`: `changed` on every edit and `submit` from
//! the single-line field on Enter, both carrying the editor's key and
//! nothing else — the text is the core's, read back with `edit_text`.
//!
//! Run: cargo run -p kui --example edit [-- --headless]

use kui::{
    Align, App, Core, EditOptions, FontFamily, Key, NodeSpec, Sizing, TextStyle, Ui, UiEvent, Value,
};
use kui_harness::{Drive, Example};

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

#[derive(Default)]
struct Edit {
    doc: Option<Key>,
    chars: usize,
    lines: usize,
    edited: bool,
    seen_version: u64,
    /// The field a `submit` arrived on, read back on the next view.
    submit: Option<Key>,
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
                    .width(Sizing::Grow(1.0))
                    .pad(12.0)
                    .gap(12.0)
                    .cross_align(Align::Center)
                    .bg(t.surface)
                    .border(1.0, t.border),
                |ui| {
                    ui.text("title", TextStyle::new(12.0).color(t.muted));
                    let title = kui::widgets::text_input(ui, "title", "");
                    if self.submit.take() == Some(title) {
                        self.submitted = ui.edit_text(title);
                    }
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
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .scroll_y(),
                |ui| {
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
                        // reader would say "text input".
                        NodeSpec::column()
                            .width(Sizing::Grow(1.0))
                            .pad(20.0)
                            .label("document"),
                    );
                    self.doc = Some(key);
                    // Recount only when the document actually changed —
                    // pulling the full text out every frame would be
                    // O(doc) per keystroke.
                    let version = ui.core().edit.version(key);
                    if version != self.seen_version || self.chars == 0 {
                        self.recount(ui.edit_text(key));
                        self.seen_version = version;
                    }
                },
            );

            // Status bar.
            ui.with(
                NodeSpec::row()
                    .width(Sizing::Grow(1.0))
                    .pad_xy(12.0, 6.0)
                    .gap(16.0)
                    .bg(t.surface)
                    .border(1.0, t.border)
                    .cross_align(Align::Center),
                |ui| {
                    let muted = TextStyle::new(12.0).color(t.muted);
                    ui.text(&format!("{} lines", self.lines), muted);
                    ui.text(&format!("{} chars", self.chars), muted);
                    ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                    ui.text(
                        if self.edited { "edited" } else { "saved" },
                        TextStyle::new(12.0).color(if self.edited { t.warning } else { t.success }),
                    );
                },
            );
        });
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get("kind").and_then(Value::as_str) {
            Some("changed") if Some(ev.key) == self.doc => {
                self.edited = true;
                self.chars = 0; // recount next view
            }
            Some("submit") => self.submit = Some(ev.key),
            _ => {}
        }
    }
}

impl Edit {
    fn recount(&mut self, text: Option<String>) {
        if let Some(t) = text {
            self.chars = t.chars().count();
            self.lines = t.lines().count().max(1);
        }
    }
}

impl Example for Edit {
    const KEYS: &'static [(&'static str, &'static str)] = &[
        ("⇧ arrows", "select"),
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
        // bar sees the change through `changed`.
        d.text(self, "x");
        d.frame(self);
        d.check(self.edited, "a keystroke reports `changed`")?;
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

kui_harness::main!(Edit::default());
