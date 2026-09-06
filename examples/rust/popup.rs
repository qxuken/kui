//! A combobox whose list is taller than the window: the case
//! `FloatConfig::fit` cannot place, and so the reason `WindowKind::Popup`
//! exists (`docs/adr/0004-multi-window.md`, decision 9). The window is 360
//! x 150; the list is twelve rows and 300 tall, so opening it visibly
//! extends past the frame instead of being clamped inside it.
//!
//! Everything here is the declaration. The field carries `on_layout`, so
//! the app is told the rect it occupies — that rect *is* the anchor, which
//! is why a popup needs no new geometry query. While `open` is true the
//! view declares a second window, `"menu"`, of kind popup; when it is false
//! the declaration stops and the window closes. Nothing calls a "close
//! popup" function, because there isn't one.
//!
//! Three things to watch with it running:
//!
//! - The **field keeps its focus ring** while the list is up, and the arrow
//!   keys walk the list. A popup does not take OS focus, so the owner still
//!   believes it has the keyboard; the runner routes the keys on.
//! - **Escape and a press outside** produce `{kind:"dismiss"}` — the same
//!   event a `modal` node gets — and close nothing by themselves. This app
//!   answers by clearing `open`, which is the whole of "closing" a popup.
//! - The **list draws past the bottom edge** of the main window, which an
//!   in-window float cannot do. Shrink the window and it still does.
//!
//! Run: cargo run --example popup

use kui::{App, Color, NodeSpec, Rect, Sizing, TextStyle, Ui, UiEvent, Value, WindowConfig};

/// The list is twelve rows of 24 plus the panel's padding — deliberately
/// twice the window's height, so "taller than the window" is not a detail
/// of how it was resized.
const ITEMS: [&str; 12] = [
    "Aluminium",
    "Beryllium",
    "Cadmium",
    "Chromium",
    "Iridium",
    "Lithium",
    "Magnesium",
    "Osmium",
    "Palladium",
    "Rhodium",
    "Titanium",
    "Vanadium",
];
const ROW_H: f32 = 24.0;
const MENU_W: f32 = 200.0;
const MENU_H: f32 = ITEMS.len() as f32 * ROW_H + 12.0;

#[derive(Default)]
struct Combo {
    /// Which item is chosen, and which the arrows are sitting on. Both are
    /// app state: the core stores no selection for a list an app draws.
    chosen: usize,
    cursor: usize,
    /// Whether the list is declared this frame. The one flag that opens and
    /// closes an OS window.
    open: bool,
    /// The field's rect, as `on_layout` last reported it — the anchor the
    /// popup is placed against, in this window's own coordinates.
    field: Rect,
}

impl App for Combo {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Every window of the app runs this same `view`; `window_name`
        // says which one is being drawn (ADR 0004 decision 12).
        if &*ui.window_name() == "menu" {
            self.list(ui);
            return;
        }

        // The list exists exactly while the field says so. The config —
        // kind, size and anchor — is read on the frame it opens and never
        // again, so a popup that must follow a moving anchor stops being
        // declared and starts again.
        if self.open {
            ui.window("menu", WindowConfig::popup(self.field, MENU_W, MENU_H));
        }

        ui.with(
            NodeSpec::column()
                .pad(16.0)
                .gap(10.0)
                // The window's whole surface, not just the content box: a
                // `Fit` column would paint its background around the text
                // and leave the rest of the window whatever the renderer
                // cleared it to.
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .bg(Color::hex(0x14161eff)),
            |ui| {
                ui.text("Alloy", TextStyle::new(12.0).color(Color::hex(0x8b90a0ff)));
                // The field. `on_layout` is the only thing here that has
                // anything to do with the popup: it reports this rect, and
                // the rect is the anchor.
                ui.with_keyed(
                    "field",
                    NodeSpec::row()
                        .pad_xy(10.0, 6.0)
                        .gap(8.0)
                        .width(Sizing::Fixed(MENU_W))
                        .bg(Color::hex(0x202030ff))
                        .hover_bg(Color::hex(0x272839ff))
                        .radius(5.0)
                        .focusable()
                        .label("Alloy")
                        .on_layout(Value::str("field"))
                        .on_click(Value::map([("kind", Value::str("toggle"))])),
                    |ui| {
                        ui.text(ITEMS[self.chosen], TextStyle::new(14.0).color(Color::WHITE));
                        ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                        ui.text("v", TextStyle::new(11.0).color(Color::hex(0x8b90a0ff)));
                    },
                );
                ui.text(
                    if self.open {
                        "Escape or a press outside asks it to go away"
                    } else {
                        "Click the field, or Tab to it and press Space"
                    },
                    TextStyle::new(11.0).color(Color::hex(0x6c7180ff)),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        let kind = ev.payload.get("kind").and_then(Value::as_str);
        match kind {
            // The field's rect, every time layout changes it. Stored, not
            // acted on: it is what the *next* declaration will carry.
            Some("layout") => {
                let n = |k: &str| ev.payload.get(k).and_then(Value::as_float).unwrap_or(0.0) as f32;
                self.field = Rect::new(n("x"), n("y"), n("w"), n("h"));
            }
            Some("toggle") => {
                self.open = !self.open;
                self.cursor = self.chosen;
            }
            // The same event a `modal` float would have given, on the
            // window instead of on a node — which is the whole of ADR 0004
            // decision 9's "changes its declaration and not its handler".
            // The core closed nothing; this line is what closes it.
            Some("dismiss") => self.open = false,
            Some("choose") => {
                if let Some(i) = ev.payload.get("i").and_then(Value::as_int) {
                    self.chosen = i as usize;
                }
                self.open = false;
            }
            // The arrows arrive on the popup's core, because the runner
            // routes the owner's keyboard there while a non-activating
            // popup is up. The field keeps its ring throughout.
            // Presses only: the list's sink never asked for releases.
            Some("key") => match ev.payload.get("code").and_then(Value::as_str) {
                Some("down") => self.cursor = (self.cursor + 1) % ITEMS.len(),
                Some("up") => self.cursor = (self.cursor + ITEMS.len() - 1) % ITEMS.len(),
                Some("enter") => {
                    self.chosen = self.cursor;
                    self.open = false;
                }
                _ => {}
            },
            _ => {}
        }
    }
}

impl Combo {
    /// The popup window's own frame: a plain list, drawn by the same
    /// `view`. Nothing about it says "popup" — the declaration did that.
    fn list(&mut self, ui: &mut Ui<'_>) {
        let root = ui.with(
            NodeSpec::column()
                .pad(6.0)
                // The popup's window *is* the panel, so its root fills it:
                // the rows grow across, and there is nothing else to share
                // the height with.
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .bg(Color::hex(0x1b1d27ff))
                .border(1.0, Color::hex(0x2f3342ff))
                // The list owns its keyboard: `on_key` makes it a sink, so
                // the arrows walk the list rather than moving a focus ring
                // the user cannot see — the visible ring is over in the
                // window that opened this one.
                .on_key(Value::map([("kind", Value::str("key"))])),
            |ui| {
                for (i, item) in ITEMS.iter().enumerate() {
                    let on = i == self.cursor;
                    ui.with_keyed(
                        item,
                        NodeSpec::row()
                            .pad_xy(10.0, 4.0)
                            .height(Sizing::Fixed(ROW_H))
                            .width(Sizing::Grow(1.0))
                            .bg(if on {
                                Color::hex(0x3b5bd4ff)
                            } else {
                                Color::hex(0x00000000)
                            })
                            .hover_bg(Color::hex(0x2a2d3aff))
                            .radius(4.0)
                            .selected(i == self.chosen)
                            .on_click(Value::map([
                                ("kind", Value::str("choose")),
                                ("i", Value::Int(i as i64)),
                            ])),
                        |ui| {
                            ui.text(
                                item,
                                TextStyle::new(13.0).color(if on {
                                    Color::WHITE
                                } else {
                                    Color::hex(0xc7cbd6ff)
                                }),
                            );
                        },
                    );
                }
            },
        );
        // The sink has to hold key focus to be handed anything, and this
        // window's own ring is invisible to the user — the ring they see
        // belongs to the field. Edge-triggered, so asking every frame is
        // asking once.
        ui.take_key_focus(root);
    }
}

fn main() {
    // Small on purpose: the list is twice this tall, so it cannot be an
    // in-window float however `fit` is asked to place it.
    kui::app("kui — popup")
        .size(360.0, 150.0)
        .run(Combo::default())
        .unwrap();
}
