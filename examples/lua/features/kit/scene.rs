//! The host scene of Kit's spike, shared by the example (`spike.rs`) and
//! its benches (`crates/kui-lua/benches/kit.rs`): a toolbar, a table of
//! 100,000 rows and a trace waterfall of 18,000 spans, each a Kit
//! component placed through the wrappers Kit's schema generates
//! (`kui-kit`, ADR 0066), the rows and spans handed over as channels
//! (ADR 0065).
//!
//! Every Kit event reaches the app as one of its own messages: the scene
//! passes `Msg` values as the components' `on_*` props, and the event's
//! data rides beside it (`select: {row}`), read with the wrapper's
//! `from_payload`.

use kui_kit as kit;
use kui_lua::LuaExtension;
use kui_core::channel::{Channel, Column};
use kui_native::{Message, NodeSpec, Ui, UiEvent, widgets};

/// What Kit's components say to the app.
#[derive(Message, Clone, Debug, PartialEq)]
pub enum Msg {
    Run,
    Pause,
    Clear,
    Shadows,
    Density,
    Press { n: i64 },
    Select,
    Pick,
}

/// How the toolbar's buttons are placed: what one instance costs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Buttons {
    /// kui's stock button from Rust, no Kit at all: the floor a button
    /// costs in a frame.
    Native,
    /// One `kit/button` per button in a plain row, replayed by default
    /// (ADR 0060), its props built every frame.
    Components,
    /// The same buttons as the host's children of a `kit/toolbar`: Kit
    /// inside Kit, through a second handle on the script.
    Toolbar,
}

pub struct Scene {
    pub rows: Channel,
    row_cols: Vec<(String, Column)>,
    pub spans: Channel,
    pub selected: Option<i64>,
    pub picked: Option<String>,
    pub runs: i64,
    pub presses: i64,
    pub shadows: bool,
    pub compact: bool,
    pub buttons: Buttons,
    pub n_buttons: usize,
    /// The waterfall's window in ms, `None` for its own.
    pub window: Option<(f64, f64)>,
    pub table: bool,
    pub waterfall: bool,
}

impl Scene {
    pub fn new(rows: usize, frames: usize) -> Self {
        let row_cols = make_rows(rows);
        Scene {
            rows: Channel::table(row_cols.clone()).expect("rows"),
            row_cols,
            spans: Channel::table(make_spans(frames)).expect("spans"),
            selected: None,
            picked: None,
            runs: 0,
            presses: 0,
            shadows: true,
            compact: false,
            buttons: Buttons::Toolbar,
            n_buttons: 8,
            window: None,
            table: true,
            waterfall: true,
        }
    }

    /// The rows again, as a new revision of the same channel: what a host
    /// pays when its data moves.
    pub fn republish(&mut self) {
        self.rows = self.rows.publish(self.row_cols.clone()).expect("rows");
    }

    fn button(&self, i: usize) -> kit::Button {
        let (label, variant, msg) = match i {
            0 => ("Run".to_string(), kit::Variant::Primary, Msg::Run),
            1 => ("Pause".into(), kit::Variant::Default, Msg::Pause),
            2 => ("Clear".into(), kit::Variant::Default, Msg::Clear),
            3 => ("Shadows".into(), kit::Variant::Default, Msg::Shadows),
            4 => ("Density".into(), kit::Variant::Default, Msg::Density),
            n => (format!("Tool {n}"), kit::Variant::Ghost, Msg::Press { n: n as i64 }),
        };
        kit::Button::new(label).variant(variant).on_press(msg)
    }

    pub fn view(&mut self, ui: &mut Ui<'_>) {
        let t = ui.theme();
        ui.open(NodeSpec::column().fill().bg(t.bg).gap(8.0).pad(8.0));
        kit::Settings::new()
            .shadows(self.shadows)
            .density(if self.compact {
                kit::Density::Compact
            } else {
                kit::Density::Comfortable
            })
            .place(ui);

        match self.buttons {
            Buttons::Toolbar => {
                kit::Toolbar::new().place_with(ui, |group, ui| {
                    if group.is_empty() {
                        for i in 0..self.n_buttons {
                            self.button(i).place(ui);
                        }
                    }
                });
            }
            Buttons::Components => {
                ui.open(NodeSpec::row().gap(6.0));
                for i in 0..self.n_buttons {
                    self.button(i).place(ui);
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
            let col = |field: &str, title: &str, width: f64, end: bool, mono: bool| {
                let c = kit::Column::new(field, title).mono(mono);
                let c = if width > 0.0 { c.width(width) } else { c };
                if end { c.align(kit::Align::End) } else { c }
            };
            let mut table = kit::Table::new(
                vec![
                    col("n", "#", 64.0, true, true),
                    col("time", "Time", 104.0, false, true),
                    col("level", "Level", 56.0, false, false),
                    col("source", "Source", 72.0, false, false),
                    col("size", "Size", 64.0, true, true),
                    col("message", "Message", 0.0, false, false),
                ],
                self.rows.clone(),
            )
            .grow(true)
            .on_select(Msg::Select);
            if let Some(i) = self.selected {
                table = table.selected(i);
            }
            ui.open(NodeSpec::column().fill());
            table.place(ui);
            ui.close();
        }

        if self.waterfall {
            let mut w = kit::Waterfall::new(self.spans.clone()).on_select(Msg::Pick);
            if let Some((start, len)) = self.window {
                w = w.start(start).length(len);
            }
            w.place(ui);
        }
        ui.close();
    }

    pub fn on_event(&mut self, ev: &UiEvent) {
        let Some(msg) = ev.message::<Msg>() else {
            return;
        };
        match msg {
            Msg::Run => self.runs += 1,
            Msg::Pause | Msg::Clear => {}
            Msg::Shadows => self.shadows = !self.shadows,
            Msg::Density => self.compact = !self.compact,
            Msg::Press { .. } => self.presses += 1,
            Msg::Select => {
                if let Some(s) = kit::TableSelect::from_payload(&ev.payload) {
                    self.selected = Some(s.row);
                }
            }
            Msg::Pick => {
                if let Some(s) = kit::WaterfallSelect::from_payload(&ev.payload) {
                    self.picked = self
                        .spans
                        .col("target")
                        .and_then(|c| self.spans.get(s.row as usize, c).to_value().as_str().map(str::to_string));
                }
            }
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

/// `n` log rows, a column a field.
pub fn make_rows(n: usize) -> Vec<(String, Column)> {
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
    let h = |i: usize| hash(i as u64);
    vec![
        ("n".into(), Column::I64((0..n as i64).collect())),
        (
            "time".into(),
            Column::strs((0..n).map(|i| {
                let ms = i as u64 * 37;
                format!(
                    "{:02}:{:02}:{:02}.{:03}",
                    12 + ms / 3_600_000 % 12,
                    ms / 60_000 % 60,
                    ms / 1000 % 60,
                    ms % 1000
                )
            })),
        ),
        (
            "level".into(),
            Column::strs((0..n).map(|i| {
                LEVELS[match h(i) % 20 {
                    0..=11 => 0,
                    12..=15 => 1,
                    16..=18 => 2,
                    _ => 3,
                }]
            })),
        ),
        ("source".into(), Column::strs((0..n).map(|i| SOURCES[(h(i) >> 8) as usize % 5]))),
        (
            "size".into(),
            Column::strs((0..n).map(|i| format!("{:.1} KB", ((h(i) >> 16) % 900) as f64 / 100.0 + 0.2))),
        ),
        (
            "message".into(),
            Column::strs((0..n).map(|i| format!("{} #{}", WORDS[(h(i) >> 24) as usize % 8], (h(i) >> 32) % 10_000))),
        ),
    ]
}

/// A recording of `frames` frames at 60 fps, six spans a frame: the frame,
/// its view, layout and paint, and the walk and shaping under them.
pub fn make_spans(frames: usize) -> Vec<(String, Column)> {
    let (mut start, mut len, mut depth, mut group) = (Vec::new(), Vec::new(), Vec::new(), Vec::new());
    let (mut name, mut target) = (Vec::new(), Vec::new());
    let mut span = |s: f64, l: f64, d: i64, g: i64, n: &'static str, f: usize| {
        start.push(s);
        len.push(l);
        depth.push(d);
        group.push(g);
        name.push(n);
        target.push(format!("f{f}/{n}"));
    };
    for f in 0..frames {
        let base = f as f64 * 1000.0 / 60.0;
        let j = (hash(f as u64) % 1000) as f64 / 1000.0;
        let view = (base + 0.2, 4.0 + 3.0 * j);
        let layout = (view.0 + view.1 + 0.1, 1.5 + j);
        let paint = (layout.0 + layout.1 + 0.1, 2.5 + 1.5 * j);
        span(base, 1000.0 / 60.0, 0, 0, "frame", f);
        span(view.0, view.1, 1, 1, "view", f);
        span(layout.0, layout.1, 1, 2, "layout", f);
        span(paint.0, paint.1, 1, 3, "paint", f);
        span(view.0 + 0.3, view.1 - 0.8, 2, 4, "walk", f);
        span(layout.0 + 0.1, layout.1 * 0.6, 2, 5, "shape", f);
    }
    vec![
        ("start".into(), Column::F64(start)),
        ("len".into(), Column::F64(len)),
        ("depth".into(), Column::I64(depth)),
        ("group".into(), Column::I64(group)),
        ("name".into(), Column::strs(name)),
        ("target".into(), Column::strs(target)),
    ]
}
