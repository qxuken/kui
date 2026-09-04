//! A mini text editor: multiline monospace editing with cursor motion,
//! selection (shift+arrows, drag, double-Home/End), clipboard (Cmd/Ctrl
//! C/X/V/A), scrolling, and a live status bar — all through the same
//! data-driven core as every other example.
//!
//! Run: cargo run -p kui --example editor

use kui::{
    Align, App, Color, EditOptions, FontFamily, Key, NodeSpec, Sizing, TextStyle, Ui, UiEvent,
    Value,
};

const INITIAL: &str = "\
# kui editor

Type here. Everything works the way you'd expect:

  - arrows, home/end, page up/down (+shift to select)
  - alt+arrows for word motion, cmd+arrows for line/document
  - click to place the caret, drag to select
  - cmd+c / cmd+x / cmd+v / cmd+a
  - enter, tab, backspace, delete

The buffer lives in the core keyed by widget identity, so this
text survives every rebuild of the UI tree - the view below is
regenerated from scratch every frame, like any other kui view.
";

#[derive(Default)]
struct Editor {
    key: Option<Key>,
    chars: usize,
    lines: usize,
    saved: bool,
    seen_version: u64,
}

impl App for Editor {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::column().fill());

        // Custom titlebar: drag strip + adaptive window controls (native
        // traffic lights on macOS, drawn buttons elsewhere).
        kui::widgets::titlebar(ui, "kui — editor");

        // Editor area: the edit node grows its height with content inside a
        // scroll container, so the document scrolls as it grows.
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
                    NodeSpec::column().width(Sizing::Grow(1.0)).pad(20.0),
                );
                self.key = Some(key);
                // Recount only when the document actually changed — pulling
                // the full text out every frame would be O(doc) per keystroke.
                let version = ui.core().edit.version(key);
                if version != self.seen_version || self.chars == 0 {
                    self.recount(ui.edit_text(key));
                    self.seen_version = version;
                }
            },
        );

        // Floating perf HUD, top-right, over the document.
        kui::widgets::latency_hud_at(ui, Align::End, Align::Start);

        // Status bar.
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .pad_xy(12.0, 6.0)
                .gap(16.0)
                .bg(Color::rgb8(0x14, 0x16, 0x1e))
                .border(1.0, Color::rgb8(0x22, 0x25, 0x30))
                .cross_align(Align::Center),
            |ui| {
                let muted = TextStyle::new(12.0).color(Color::rgb8(0x8a, 0x8f, 0xa3));
                ui.text(&format!("{} lines", self.lines), muted);
                ui.text(&format!("{} chars", self.chars), muted);
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                ui.text(
                    if self.saved { "saved" } else { "edited" },
                    TextStyle::new(12.0).color(if self.saved {
                        Color::rgb8(0x73, 0xd9, 0x8c)
                    } else {
                        Color::rgb8(0xf0, 0xb8, 0x59)
                    }),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        if ev.payload.get("kind").and_then(Value::as_str) == Some("changed") {
            self.saved = false;
            self.chars = usize::MAX; // recount next view
        }
    }
}

impl Editor {
    fn recount(&mut self, text: Option<String>) {
        if let Some(t) = text {
            self.chars = t.chars().count();
            self.lines = t.lines().count().max(1);
        }
    }
}

fn main() {
    kui::app("kui — editor")
        .custom_titlebar()
        .run(Editor {
            saved: true,
            ..Default::default()
        })
        .unwrap();
}
