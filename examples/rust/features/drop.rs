//! A drop zone, declared rather than tracked
//! (`docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md`).
//! Drag files in from the Finder:
//!
//! - `on_drop`: the box is a zone, and the files over it arrive as four
//!   phases of one event — `enter`, `move`, `leave`, `drop` — with their
//!   paths and the pointer's position;
//! - `drop_bg`: the zone lights while they hover, swapped by the core the
//!   way `hover_bg` is, with no state in the view;
//! - a button inside the zone is the zone's: files over it land here;
//! - the banner the view shows in answer to `enter` is a float over the
//!   zone that is no zone, and the files look past it — the flicker
//!   HTML's `dragleave` is known for cannot happen;
//! - the second box below takes nothing: over it the cursor shows the
//!   not-allowed circle and a release there slides the icon home;
//! - "Open…" asks for the platform's Open dialog instead (backlog C51):
//!   `ui.request_files`, answered by one `files` event whose `paths` are
//!   what a drop's are, so the same list takes both.
//!
//! Run: cargo run -p kui-native --example drop [-- --headless]

use kui_devtools::{Drive, Example};
use kui_native::{
    Align, App, Core, FileDialog, FloatConfig, NodeSpec, TextStyle, Ui, UiEvent, Value, Vec2,
};

#[derive(Default)]
struct Drop {
    /// What landed, newest last.
    landed: Vec<String>,
    /// The files over the zone right now, from `enter` / `leave`.
    hovering: Vec<String>,
    /// Where they are, from `move`.
    at: Option<(f32, f32)>,
    events: u32,
    cleared: u32,
    /// "Open…" was clicked: the next view asks for the dialog. `on_event`
    /// has no `Ui` to ask with, so the ask is the view's.
    open: bool,
}

impl App for Drop {
    fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        if std::mem::take(&mut self.open) {
            // One ask at a time: a click while the dialog is up asks for
            // nothing more.
            ui.request_files(
                FileDialog::open()
                    .multiple()
                    .title("Add files")
                    .tag(Value::str("add")),
            );
        }
        ui.with(
            NodeSpec::column()
                .fill()
                .pad(24.0)
                .gap(14.0)
                .cross_align(Align::Start),
            |ui| {
                ui.text(
                    "on_drop + drop_bg · drag files from the Finder onto the zone",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.with_keyed(
                    "zone",
                    NodeSpec::column()
                        .size(400.0, 180.0)
                        .pad(14.0)
                        .gap(8.0)
                        .radius(10.0)
                        .bg(t.surface)
                        .drop_bg(t.accent.with_alpha(0.35))
                        .border(1.0, t.border)
                        .on_drop("zone"),
                    |ui| {
                        let heading = if self.hovering.is_empty() {
                            "drop files here".to_string()
                        } else {
                            format!("{} file(s) over the zone", self.hovering.len())
                        };
                        ui.text(&heading, TextStyle::new(14.0));
                        if let Some((x, y)) = self.at {
                            ui.text(
                                &format!("pointer at {x:.0}, {y:.0}"),
                                TextStyle::new(12.0).color(t.faint),
                            );
                        }
                        // Inside the zone: a button, and files over it are
                        // the zone's — no `on_drop` of its own.
                        ui.text_in_keyed(
                            "clear",
                            NodeSpec::row()
                                .pad_xy(12.0, 6.0)
                                .radius(6.0)
                                .bg(t.raised)
                                .hover_bg(t.hover)
                                .on_click(Value::map([("kind", Value::str("clear"))]))
                                .label("Clear"),
                            "Clear the list",
                            TextStyle::new(12.0),
                        );
                        ui.text_in_keyed(
                            "open",
                            NodeSpec::row()
                                .pad_xy(12.0, 6.0)
                                .radius(6.0)
                                .bg(t.raised)
                                .hover_bg(t.hover)
                                .on_click(Value::map([("kind", Value::str("open"))]))
                                .label("Open…"),
                            "Open…",
                            TextStyle::new(12.0),
                        );
                        // What an app shows in answer to `enter`: a banner
                        // floated over the zone. It takes no files, so
                        // the files look past it (decision 2) — the zone
                        // stays lit and hears `move`, not `leave`.
                        if !self.hovering.is_empty() {
                            ui.text_in_keyed(
                                "banner",
                                NodeSpec::row()
                                    .float(
                                        FloatConfig::parent()
                                            .inside(Align::Center, Align::End)
                                            .offset(0.0, -8.0),
                                    )
                                    .pad_xy(14.0, 8.0)
                                    .radius(8.0)
                                    .bg(t.accent)
                                    .hoverable(),
                                "release to add",
                                TextStyle::new(12.0)
                                    .color(kui_native::widgets::readable_on(t.accent)),
                            );
                        }
                    },
                );
                ui.text_in_keyed(
                    "nowhere",
                    NodeSpec::row()
                        .width(400.0)
                        .pad(12.0)
                        .radius(10.0)
                        .bg(t.surface)
                        .border(1.0, t.border)
                        .hoverable(),
                    "not a zone · the cursor says no, a release slides home",
                    TextStyle::new(12.0).color(t.muted),
                );
                ui.text(
                    &format!(
                        "landed: {}",
                        if self.landed.is_empty() {
                            "nothing yet".to_string()
                        } else {
                            self.landed.join(", ")
                        }
                    ),
                    TextStyle::new(12.0),
                );
                ui.text(
                    &format!("{} drop events · cleared {}×", self.events, self.cleared),
                    TextStyle::new(12.0).color(t.faint),
                );
            },
        );
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.kind() {
            Some("clear") => {
                self.landed.clear();
                self.cleared += 1;
                return;
            }
            Some("open") => {
                self.open = true;
                return;
            }
            // The dialog's answer: the same paths a drop carries, none when
            // it was cancelled.
            Some("files") => {
                let paths = ev.payload.get("paths").and_then(Value::as_list);
                self.landed.extend(
                    paths
                        .unwrap_or(&[])
                        .iter()
                        .filter_map(|p| p.as_str().map(str::to_string)),
                );
                return;
            }
            Some("drop") => {}
            _ => return,
        }
        self.events += 1;
        let paths: Vec<String> = ev
            .payload
            .get("paths")
            .and_then(Value::as_list)
            .map(|l| {
                l.iter()
                    .filter_map(|p| p.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default();
        let at = match (ev.payload.get_float("x"), ev.payload.get_float("y")) {
            (Some(x), Some(y)) => Some((x as f32, y as f32)),
            _ => None,
        };
        match ev.payload.get_str("phase") {
            Some("enter") => {
                self.hovering = paths;
                self.at = at;
            }
            Some("move") => self.at = at,
            Some("leave") => {
                self.hovering.clear();
                self.at = None;
            }
            Some("drop") => {
                self.hovering.clear();
                self.at = None;
                self.landed.extend(paths);
            }
            _ => {}
        }
    }
}

impl Example for Drop {
    fn window(&self) -> kui_devtools::Window {
        kui_devtools::Window::default().size(460.0, 440.0)
    }

    /// Files in from the OS, headlessly: over the zone, over its button,
    /// over the banner the view showed, off every zone, and landed.
    fn headless(&mut self, core: &mut Core) -> Result<(), String> {
        let mut d = Drive::new(core, 460.0, 440.0);
        d.frame(self);
        let zone = d.key_of("zone").ok_or("no zone")?;
        let zone_rect = d.rect_of(zone).ok_or("the zone has no rect")?;
        let paths = vec!["/tmp/a.txt".to_string(), "/tmp/b.png".to_string()];
        let inside = Vec2::new(zone_rect.x + 30.0, zone_rect.y + 30.0);
        d.input(
            self,
            kui_native::InputEvent::DragFiles {
                paths: paths.clone(),
                at: inside,
            },
        );
        d.check(
            self.hovering.len() == 2 && d.core.drop_target() == Some(zone),
            "files over the zone: `enter`, and the zone is the target",
        )?;
        d.frame(self);
        d.check(d.core.is_drop_target(zone), "the zone reads lit")?;
        // Over the button inside it: the zone's still.
        let button = d.key_of("clear").ok_or("no button")?;
        let br = d.rect_of(button).ok_or("the button has no rect")?;
        d.input(
            self,
            kui_native::InputEvent::DragFiles {
                paths: paths.clone(),
                at: Vec2::new(br.x + 4.0, br.y + 4.0),
            },
        );
        d.check(
            self.hovering.len() == 2 && self.events == 2,
            "over the button inside the zone: a `move` on the zone, not a leave",
        )?;
        // Over the banner the view showed on enter: looked past.
        d.frame(self);
        let banner = d.key_of("banner").ok_or("the view showed no banner")?;
        let bn = d.rect_of(banner).ok_or("the banner has no rect")?;
        d.input(
            self,
            kui_native::InputEvent::DragFiles {
                paths: paths.clone(),
                at: Vec2::new(bn.x + 4.0, bn.y + 4.0),
            },
        );
        d.check(
            self.hovering.len() == 2 && d.core.drop_target() == Some(zone),
            "over the banner: the files look past it to the zone",
        )?;
        // Off every zone: leave, nothing lit.
        let nowhere = d.key_of("nowhere").ok_or("no second box")?;
        let nr = d.rect_of(nowhere).ok_or("the second box has no rect")?;
        d.input(
            self,
            kui_native::InputEvent::DragFiles {
                paths: paths.clone(),
                at: Vec2::new(nr.x + 4.0, nr.y + 4.0),
            },
        );
        d.check(
            self.hovering.is_empty() && d.core.drop_target().is_none(),
            "over the box that is no zone: `leave`, and no target",
        )?;
        // Back in, and released: landed, no leave after.
        d.input(
            self,
            kui_native::InputEvent::DragFiles {
                paths: paths.clone(),
                at: inside,
            },
        );
        let before = self.events;
        d.input(
            self,
            kui_native::InputEvent::DropFiles { paths, at: inside },
        );
        d.check(
            self.landed.len() == 2 && self.hovering.is_empty() && self.events == before + 1,
            "released over the zone: `drop` with both paths, and no `leave` after it",
        )?;
        d.frame(self);
        d.check(!d.core.is_drop_target(zone), "and nothing is lit")?;
        d.click_key(self, button);
        d.check(
            self.landed.is_empty() && self.cleared == 1,
            "the button inside still clicks",
        )?;

        // "Open…": the view asks, the host (this drive) takes the ask and
        // answers it, and the answer lands in the same list.
        let open = d.key_of("open").ok_or("no Open button")?;
        d.click_key(self, open);
        d.frame(self);
        let asks = d.core.take_file_requests();
        d.check(
            asks.len() == 1 && asks[0].multiple && asks[0].tag == Value::str("add"),
            "Open… asks the host for one multiple-file dialog, tagged",
        )?;
        d.check(
            d.core.awaiting_files(),
            "and the ask stays out until it is answered",
        )?;
        d.click_key(self, open);
        d.frame(self);
        let again = d.core.take_file_requests();
        d.check(
            again.is_empty(),
            "a second click while it is up asks for nothing more",
        )?;
        d.input(
            self,
            kui_native::InputEvent::Files(vec!["/tmp/c.md".to_string()]),
        );
        d.check(
            self.landed == ["/tmp/c.md"] && !d.core.awaiting_files(),
            "the answer lands like a drop, and the ask is spent",
        )?;
        d.click_key(self, open);
        d.frame(self);
        d.core.take_file_requests();
        d.input(self, kui_native::InputEvent::Files(Vec::new()));
        d.check(
            self.landed.len() == 1,
            "a cancelled dialog answers with no paths, and nothing lands",
        )
    }
}

kui_devtools::main!(Drop::default());
