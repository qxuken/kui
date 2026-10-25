//! The host scene of Kit's M0 spike, shared by the example (`spike.rs`)
//! and its benches (`crates/kui-lua/benches/kit.rs`): a toolbar, a table
//! of 100,000 rows and a trace waterfall of 18,000 bars, all filled by the
//! Kit script, with the app's settings in a `kit/settings` slot.
//!
//! Every Kit event reaches the app as one of its own messages: the scene
//! passes `Msg` values in the params (`on_press`, `on_select`) and Kit
//! returns a copy with the event's fields merged in, so `on_event` decodes
//! it with `ev.message::<Msg>()` like any stock control's message. That is
//! ADR B's declared-message path, done by the script until core does it.

use kui_lua::LuaExtension;
use kui_native::{Message, NodeSpec, Ui, Value, widgets};

/// What Kit's components say to the app.
#[derive(Message, Clone, Debug, PartialEq)]
pub enum Msg {
    Run,
    Pause,
    Clear,
    Shadows,
    Density,
    Press {
        n: i64,
    },
    Select {
        index: i64,
    },
    Pick {
        target: String,
        name: String,
        start: f64,
        len: f64,
    },
}

/// How the toolbar's buttons are placed: the cost comparison M0 asks for.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Buttons {
    /// One `kit/toolbar` fill holding every button.
    OneFill,
    /// One `kit/button#i` slot per button.
    Slots,
    /// The same slots, each declared with `slot_replay`.
    SlotsReplayed,
    /// Replayed, with each slot's name and params built once and kept:
    /// what replay costs core, without the host's building.
    SlotsReplayedKept,
    /// kui's stock button from Rust, no Kit at all: the floor a button
    /// costs in a frame.
    Native,
}

pub struct Scene {
    pub rows: Value,
    pub bars: Value,
    pub rev: i64,
    sent: i64,
    pub selected: Option<i64>,
    pub picked: Option<String>,
    pub runs: i64,
    pub presses: i64,
    pub shadows: bool,
    pub compact: bool,
    pub buttons: Buttons,
    pub n_buttons: usize,
    kept: Vec<(String, Value)>,
    /// Declare `kit/settings` with `slot_replay`, so an unchanged frame fills nothing fresh.
    pub settings_replayed: bool,
    pub scroll_top: Option<f64>,
    /// The waterfall's window in ms, `None` for its own.
    pub window: Option<(f64, f64)>,
    pub table: bool,
    pub waterfall: bool,
}

impl Scene {
    pub fn new(rows: usize, frames: usize) -> Self {
        Scene {
            rows: make_rows(rows),
            bars: make_bars(frames),
            rev: 1,
            sent: 0,
            selected: None,
            picked: None,
            runs: 0,
            presses: 0,
            shadows: true,
            compact: false,
            buttons: Buttons::OneFill,
            n_buttons: 8,
            kept: Vec::new(),
            settings_replayed: false,
            scroll_top: None,
            window: None,
            table: true,
            waterfall: true,
        }
    }

    /// The data, or only its revision once Kit has it.
    fn data(&self, key: &'static str, v: &Value) -> Value {
        if self.sent == self.rev {
            Value::map([("rev", self.rev.into())])
        } else {
            Value::map([("rev", self.rev.into()), (key, v.clone())])
        }
    }

    fn item(&self, i: usize) -> Value {
        let (label, variant, msg): (String, &str, Msg) = match i {
            0 => ("Run".into(), "primary", Msg::Run),
            1 => ("Pause".into(), "default", Msg::Pause),
            2 => ("Clear".into(), "default", Msg::Clear),
            3 => ("Shadows".into(), "default", Msg::Shadows),
            4 => ("Density".into(), "default", Msg::Density),
            n => (format!("Tool {n}"), "ghost", Msg::Press { n: n as i64 }),
        };
        Value::map([
            ("label", label.into()),
            ("variant", variant.into()),
            ("on_press", msg.into()),
        ])
    }

    pub fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.open(NodeSpec::column().fill().bg(t.bg).gap(8.0).pad(8.0));
        let settings = Value::map([
            ("shadows", self.shadows.into()),
            (
                "density",
                if self.compact {
                    "compact"
                } else {
                    "comfortable"
                }
                .into(),
            ),
        ]);
        if self.settings_replayed {
            ui.slot_replay("kit/settings", &settings);
        } else {
            ui.slot_with("kit/settings", &settings);
        }

        match self.buttons {
            Buttons::OneFill => {
                let items = (0..self.n_buttons)
                    .map(|i| self.item(i))
                    .collect::<Vec<_>>();
                ui.slot_with("kit/toolbar", &Value::map([("items", Value::list(items))]));
            }
            Buttons::Slots | Buttons::SlotsReplayed => {
                ui.open(NodeSpec::row().gap(6.0));
                for i in 0..self.n_buttons {
                    let name = format!("kit/button#{i}");
                    let p = self.item(i);
                    if self.buttons == Buttons::SlotsReplayed {
                        ui.slot_replay(&name, &p);
                    } else {
                        ui.slot_with(&name, &p);
                    }
                }
                ui.close();
            }
            Buttons::SlotsReplayedKept => {
                if self.kept.len() != self.n_buttons {
                    self.kept = (0..self.n_buttons)
                        .map(|i| (format!("kit/button#{i}"), self.item(i)))
                        .collect();
                }
                ui.open(NodeSpec::row().gap(6.0));
                for (name, p) in &self.kept {
                    ui.slot_replay(name, p);
                }
                ui.close();
            }
            Buttons::Native => {
                ui.open(NodeSpec::row().gap(6.0));
                for i in 0..self.n_buttons {
                    widgets::button(ui, &format!("Tool {i}"), Msg::Press { n: i as i64 });
                }
                ui.close();
            }
        }

        if self.table {
            let col = |title: &str, width: f64, align: &str, mono: bool| {
                Value::map([
                    ("title", title.into()),
                    ("width", width.into()),
                    ("align", align.into()),
                    ("mono", mono.into()),
                ])
            };
            let mut p = vec![
                (
                    "columns",
                    Value::list([
                        col("#", 64.0, "end", true),
                        col("Time", 104.0, "start", true),
                        col("Level", 56.0, "start", false),
                        col("Source", 72.0, "start", false),
                        col("Size", 64.0, "end", true),
                        col("Message", 0.0, "start", false),
                    ]),
                ),
                ("data", self.data("rows", &self.rows)),
                ("on_select", Msg::Select { index: 0 }.into()),
            ];
            if let Some(i) = self.selected {
                p.push(("selected", i.into()));
            }
            if let Some(y) = self.scroll_top {
                p.push(("scroll_top", y.into()));
            }
            ui.open(NodeSpec::column().fill());
            ui.slot_with("kit/table", &Value::map(p));
            ui.close();
        }

        if self.waterfall {
            let mut p = vec![
                ("data", self.data("bars", &self.bars)),
                (
                    "on_select",
                    Msg::Pick {
                        target: String::new(),
                        name: String::new(),
                        start: 0.0,
                        len: 0.0,
                    }
                    .into(),
                ),
            ];
            if let Some((start, len)) = self.window {
                p.push((
                    "view",
                    Value::map([("start", start.into()), ("len", len.into())]),
                ));
            }
            ui.slot_with("kit/waterfall", &Value::map(p));
        }
        ui.close();
        self.sent = self.rev;
    }

    pub fn on_message(&mut self, msg: Msg) {
        match msg {
            Msg::Run => self.runs += 1,
            Msg::Pause | Msg::Clear => {}
            Msg::Shadows => self.shadows = !self.shadows,
            Msg::Density => self.compact = !self.compact,
            Msg::Press { .. } => self.presses += 1,
            Msg::Select { index } => self.selected = Some(index),
            Msg::Pick { target, .. } => self.picked = Some(target),
        }
    }
}

/// Kit's entry script. `from_file` puts its folder first on
/// `package.path`, so its `require`s find the modules beside it (ADR 0059).
pub fn load_kit() -> LuaExtension {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../addons/kit/lua/kit.lua");
    LuaExtension::from_file(path).expect("load addons/kit/lua/kit.lua")
}

fn hash(i: u64) -> u64 {
    let mut x = i.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// `n` log rows, each a list of cell strings in column order.
pub fn make_rows(n: usize) -> Value {
    const LEVELS: [&str; 4] = ["info", "debug", "warn", "error"];
    const SOURCES: [&str; 5] = ["api", "worker", "db", "cache", "auth"];
    const WORDS: [&str; 8] = [
        "request handled",
        "cache miss for key",
        "retrying after timeout",
        "token refreshed",
        "query planned",
        "batch flushed",
        "connection reused",
        "slow response from upstream",
    ];
    Value::list((0..n).map(|i| {
        let h = hash(i as u64);
        let ms = i as u64 * 37;
        Value::list([
            Value::str(i.to_string()),
            Value::str(format!(
                "{:02}:{:02}:{:02}.{:03}",
                12 + ms / 3_600_000 % 12,
                ms / 60_000 % 60,
                ms / 1000 % 60,
                ms % 1000
            )),
            Value::str(
                LEVELS[match h % 20 {
                    0..=11 => 0,
                    12..=15 => 1,
                    16..=18 => 2,
                    _ => 3,
                }],
            ),
            Value::str(SOURCES[(h >> 8) as usize % 5]),
            Value::str(format!("{:.1} KB", ((h >> 16) % 900) as f64 / 100.0 + 0.2)),
            Value::str(format!(
                "{} #{}",
                WORDS[(h >> 24) as usize % 8],
                (h >> 32) % 10_000
            )),
        ])
    }))
}

/// A recording of `frames` frames at 60 fps, six bars a frame: the frame,
/// its view, layout and paint, and the walk and shaping under them.
pub fn make_bars(frames: usize) -> Value {
    let mut out = Vec::with_capacity(frames * 6);
    let bar = |s: f64, l: f64, d: i64, g: i64, n: &str, f: usize| {
        Value::map([
            ("s", s.into()),
            ("l", l.into()),
            ("d", d.into()),
            ("g", g.into()),
            ("n", n.into()),
            ("t", format!("f{f}/{n}").into()),
        ])
    };
    for f in 0..frames {
        let base = f as f64 * 1000.0 / 60.0;
        let j = (hash(f as u64) % 1000) as f64 / 1000.0;
        let view = (base + 0.2, 4.0 + 3.0 * j);
        let layout = (view.0 + view.1 + 0.1, 1.5 + j);
        let paint = (layout.0 + layout.1 + 0.1, 2.5 + 1.5 * j);
        out.push(bar(base, 1000.0 / 60.0, 0, 1, "frame", f));
        out.push(bar(view.0, view.1, 1, 2, "view", f));
        out.push(bar(layout.0, layout.1, 1, 3, "layout", f));
        out.push(bar(paint.0, paint.1, 1, 4, "paint", f));
        out.push(bar(view.0 + 0.3, view.1 - 0.8, 2, 5, "walk", f));
        out.push(bar(layout.0 + 0.1, layout.1 * 0.6, 2, 6, "shape", f));
    }
    Value::list(out)
}
