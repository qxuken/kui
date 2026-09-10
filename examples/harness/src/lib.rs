//! The harness every example in this repository runs inside
//! (`docs/adr/0021-one-subject-per-example.md`, decision 6).
//!
//! An example is one subject — a widget, a feature, an app — and nothing
//! else. What every example used to carry beside its subject lives here
//! once: the window title, the `--headless` parsing, the latency HUD, the
//! theme keys one example had, and a **dock** the example never draws
//! into. The dock shows what the runtime is doing while the example runs:
//!
//! - the latency graph and the frame counter (`n / KUI_SMOKE_FRAMES` when
//!   one is set, so a smoke run is legible on screen);
//! - the **event stream** — every `UiEvent` handed to the example, with
//!   the frame it arrived on and its payload printed as the data it is,
//!   plus every `kui: warning` the core raised, marked;
//! - the **status block** — what the runtime believes right now, each
//!   row read from the door it comes from: `env.system`, `env.window`,
//!   the viewport, the focused node and whether its ring shows, the
//!   modifiers, native menus, every open window, and `env.audio`;
//! - the **controls** — the theme base and accent, native menus on or
//!   off, where the dock sits, and clear — the same handler the chords
//!   below reach;
//! - the **key legend**, from [`Example::KEYS`].
//!
//! The chords are `Ctrl+Shift+<letter>` on every platform, a family no
//! example keymap uses: `T` cycles the base (follow the OS → light →
//! dark), `A` the accent, `M` toggles native menus, `D` moves the dock
//! (side → bottom → off), `C` clears the stream.
//!
//! The dock is a sibling of the example's tree, so the example's root is
//! a child of the harness's. Two rules follow, and both are what an
//! extension in a slot already lives under (ADR 0014): an example
//! addresses its nodes from the key its `open` returned, never from
//! `Key::ROOT`; and it opens its own container rather than configuring
//! the root, which is the harness's. The dock carries `role = none`, so
//! neither the Tab ring nor assistive technology sees it — the example's
//! access tree is the example's. `--dock off` also declares no key sink
//! and takes no focus, which is what the accessibility audit runs under.
//!
//! One `main` per example: `kui_harness::main!(Counter::default())`.

use std::collections::VecDeque;

use kui::widgets;
use kui::{
    Align, App, Appearance, Chrome, Color, Core, Extensions, Key, Min, NodeSpec, Role, Sizing,
    TextStyle, Theme, ThemeSource, Ui, UiEvent, Value, Vec2, Waker,
};

mod drive;
pub use drive::Drive;

/// Width of the side dock, logical px.
pub const DOCK_SIDE_W: f32 = 330.0;
/// Height of the bottom dock, logical px.
pub const DOCK_BOTTOM_H: f32 = 280.0;
/// The window height a side dock needs to show all of itself, and the
/// width a bottom one does: a smaller example's window is raised to it.
const DOCK_SIDE_MIN_H: f64 = 600.0;
const DOCK_BOTTOM_MIN_W: f64 = 640.0;
/// How many stream entries the dock keeps.
const STREAM_CAP: usize = 64;
/// The tag on the harness's own key sink: events carrying it are the
/// harness's and never reach the example.
const SINK_TAG: &str = "harness";
/// What `Example::headless` answers by default, and the message the exit
/// code 2 carries: listed in a smoke round without a drive to run.
pub const NO_HEADLESS: &str = "no headless drive";

/// The accents `Ctrl+Shift+A` walks: kui's own, then four the OS might
/// report. `None` is the OS's.
const ACCENTS: [(&str, u32); 5] = [
    ("kui blue", 0x3b5bd4ff),
    ("macOS blue", 0x007affff),
    ("macOS yellow", 0xffc409ff),
    ("macOS pink", 0xf74f9eff),
    ("forest", 0x2f7d4fff),
];

/// Where the dock sits.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Dock {
    /// A column on the right, [`DOCK_SIDE_W`] wide.
    #[default]
    Side,
    /// A strip along the bottom, [`DOCK_BOTTOM_H`] tall.
    Bottom,
    /// No dock, no key sink, no focus taken: the example alone, under the
    /// harness's title and CLI.
    Off,
}

impl Dock {
    fn name(self) -> &'static str {
        match self {
            Dock::Side => "side",
            Dock::Bottom => "bottom",
            Dock::Off => "off",
        }
    }

    fn parse(s: &str) -> Option<Dock> {
        match s {
            "side" => Some(Dock::Side),
            "bottom" => Some(Dock::Bottom),
            "off" => Some(Dock::Off),
            _ => None,
        }
    }

    fn next(self) -> Dock {
        match self {
            Dock::Side => Dock::Bottom,
            Dock::Bottom => Dock::Off,
            Dock::Off => Dock::Side,
        }
    }
}

/// The window an example asks for. The harness adds the dock's extent to
/// `size`, so the example's content gets exactly what it asked for.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// Initial inner size of the example's area, logical px.
    pub size: (f64, f64),
    pub min_size: Option<(f64, f64)>,
    pub max_size: Option<(f64, f64)>,
    pub chrome: Chrome,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            size: (960.0, 640.0),
            min_size: None,
            max_size: None,
            chrome: Chrome::Native,
        }
    }
}

impl Window {
    pub fn size(mut self, w: f64, h: f64) -> Self {
        self.size = (w, h);
        self
    }

    pub fn min_size(mut self, w: f64, h: f64) -> Self {
        self.min_size = Some((w, h));
        self
    }

    pub fn max_size(mut self, w: f64, h: f64) -> Self {
        self.max_size = Some((w, h));
        self
    }

    pub fn chrome(mut self, chrome: Chrome) -> Self {
        self.chrome = chrome;
        self
    }

    /// Shorthand for `.chrome(Chrome::Custom)`.
    pub fn custom_titlebar(self) -> Self {
        self.chrome(Chrome::Custom)
    }
}

/// An example: an [`App`] with what the harness asks beside it. Every
/// method has a default, so the smallest example is `impl Example for X {}`.
pub trait Example: App {
    /// The key legend the dock shows: `(keys, what they do)`.
    const KEYS: &'static [(&'static str, &'static str)] = &[];
    /// Flags of the example's own, for the usage text and so the harness
    /// does not refuse them: `(flag, what it does)`. The example reads
    /// them from `std::env::args()` itself.
    const FLAGS: &'static [(&'static str, &'static str)] = &[];

    /// The window the example wants.
    fn window(&self) -> Window {
        Window::default()
    }

    /// Where the dock goes unless `--dock` says.
    fn dock(&self) -> Dock {
        Dock::Side
    }

    /// The extensions the window loads, each under the namespace the host
    /// chose for it (ADR 0014) — taken once, before the window opens. A
    /// headless drive loads its own, since it builds its own frames.
    fn extensions(&mut self) -> Extensions {
        Extensions::new()
    }

    /// Whether the platform's own menus are shown, to start with: `None`
    /// leaves the core's default (the platform's, where it has one),
    /// `Some(false)` asks for the core's drawn menu. The dock's `menus`
    /// row toggles it either way.
    fn native_menus(&self) -> Option<bool> {
        None
    }

    /// The self-check `--headless` runs: drive the example through a bare
    /// `Core` and return `Err` on a wrong answer. [`Drive`] is the
    /// scaffolding for it. The default is the refusal the harness exits 2
    /// on — a name in a `headless = [...]` list without a drive fails,
    /// rather than passing for having printed nothing.
    fn headless(&mut self, _core: &mut Core) -> Result<(), String> {
        Err(NO_HEADLESS.into())
    }
}

/// What the harness parsed off the command line.
#[derive(Clone, Debug, Default)]
pub struct Cli {
    pub headless: bool,
    pub dock: Option<Dock>,
    pub base: Option<Appearance>,
    pub accent: Option<Color>,
    pub size: Option<(f64, f64)>,
}

impl Cli {
    /// Parses the harness's flags out of `args` (the program name
    /// excluded), leaving the example's own (`flags`) alone. Anything else
    /// is an error with the usage text.
    pub fn parse(
        name: &str,
        args: impl IntoIterator<Item = String>,
        flags: &[(&str, &str)],
    ) -> Result<Cli, String> {
        let mut cli = Cli::default();
        let mut args = args.into_iter();
        while let Some(arg) = args.next() {
            // `--dock=side` and `--dock side` both.
            let (flag, inline) = match arg.split_once('=') {
                Some((f, v)) => (f.to_string(), Some(v.to_string())),
                None => (arg.clone(), None),
            };
            let mut value = |what: &str| -> Result<String, String> {
                inline
                    .clone()
                    .or_else(|| args.next())
                    .ok_or_else(|| format!("{flag} takes {what}\n\n{}", usage(name, flags)))
            };
            match flag.as_str() {
                "--headless" => cli.headless = true,
                "--dock" => {
                    let v = value("side, bottom or off")?;
                    cli.dock = Some(
                        Dock::parse(&v)
                            .ok_or_else(|| format!("--dock: {v:?} is not side, bottom or off"))?,
                    );
                }
                "--light" => cli.base = Some(Appearance::Light),
                "--dark" => cli.base = Some(Appearance::Dark),
                "--accent" => {
                    let v = value("#rrggbb")?;
                    cli.accent = Some(
                        parse_color(&v).ok_or_else(|| format!("--accent: {v:?} is not #rrggbb"))?,
                    );
                }
                "--size" => {
                    let v = value("WxH")?;
                    let (w, h) = v
                        .split_once('x')
                        .and_then(|(w, h)| Some((w.parse::<f64>().ok()?, h.parse::<f64>().ok()?)))
                        .ok_or_else(|| format!("--size: {v:?} is not WxH"))?;
                    cli.size = Some((w, h));
                }
                "-h" | "--help" => return Err(usage(name, flags)),
                other if flags.iter().any(|(f, _)| *f == other) => {}
                other if other.starts_with('-') => {
                    return Err(format!("unknown flag {other}\n\n{}", usage(name, flags)));
                }
                _ => {}
            }
        }
        Ok(cli)
    }
}

fn usage(name: &str, flags: &[(&str, &str)]) -> String {
    let mut s = format!(
        "usage: {name} [--headless] [--dock side|bottom|off] [--light|--dark] [--accent #rrggbb] [--size WxH]"
    );
    for (f, doc) in flags {
        s.push_str(&format!(" [{f}]"));
        let _ = doc;
    }
    s.push_str("\n\n  --headless       drive the example through a bare Core and exit 0 on the right answer\n");
    s.push_str("  --dock WHERE     where the harness dock sits (default: what the example asks)\n");
    s.push_str("  --light, --dark  pin the theme base instead of following the OS\n");
    s.push_str("  --accent COLOUR  the accent, instead of the OS's\n");
    s.push_str("  --size WxH       the example's area, logical px (the dock is added)\n");
    if !flags.is_empty() {
        s.push('\n');
        for (f, doc) in flags {
            s.push_str(&format!("  {f:<16} {doc}\n"));
        }
    }
    s
}

fn parse_color(s: &str) -> Option<Color> {
    let hex = s.strip_prefix('#')?;
    let n = u32::from_str_radix(hex, 16).ok()?;
    match hex.len() {
        6 => Some(Color::hex((n << 8) | 0xff)),
        8 => Some(Color::hex(n)),
        _ => None,
    }
}

/// Runs `example` under the harness: parses the command line, drives it
/// headlessly when asked, and otherwise opens the window the example asked
/// for with the dock beside it. `name` is what the window title and the
/// usage text call it — [`main!`] passes `CARGO_BIN_NAME`.
///
/// Exits the process: 0 when the window closed or the headless drive
/// passed, 1 on a failed drive or a runner error, 2 on a bad command line
/// or an example listed as headless without a drive.
pub fn run<E: Example>(name: &str, example: E) {
    let args = std::env::args().skip(1);
    let cli = match Cli::parse(name, args, E::FLAGS) {
        Ok(cli) => cli,
        Err(msg) => {
            eprintln!("{msg}");
            std::process::exit(2);
        }
    };
    std::process::exit(run_with(name, example, cli));
}

/// [`run`] with the command line already parsed; returns the exit code.
pub fn run_with<E: Example>(name: &str, mut example: E, cli: Cli) -> i32 {
    if cli.headless {
        let mut core = Core::new();
        core.set_diagnostics(true);
        return match example.headless(&mut core) {
            Ok(()) => {
                println!("{name}: headless drive OK");
                0
            }
            Err(e) if e == NO_HEADLESS => {
                eprintln!("{name}: {e}");
                2
            }
            Err(e) => {
                eprintln!("{name}: FAILED: {e}");
                1
            }
        };
    }
    let dock = cli.dock.unwrap_or_else(|| example.dock());
    let window = example.window();
    let (mut w, mut h) = cli.size.unwrap_or(window.size);
    // The dock's extent is added, so the example's area is what it asked
    // for — and the window is given the floor the dock needs to be read,
    // which a small example is otherwise under.
    match dock {
        Dock::Side => {
            w += DOCK_SIDE_W as f64;
            h = h.max(DOCK_SIDE_MIN_H);
        }
        Dock::Bottom => {
            h += DOCK_BOTTOM_H as f64;
            w = w.max(DOCK_BOTTOM_MIN_W);
        }
        Dock::Off => {}
    }
    let extensions = example.extensions();
    let harness = Harness::new(name, example, dock, cli.base, cli.accent);
    let mut launcher = kui::app(&format!("kui — {name}"))
        .size(w, h)
        .chrome(window.chrome)
        .with_extensions(extensions);
    if let Some((mw, mh)) = window.min_size {
        launcher = launcher.min_size(mw, mh);
    }
    if let Some((mw, mh)) = window.max_size {
        launcher = launcher.max_size(mw, mh);
    }
    match launcher.run(harness) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{name}: {e}");
            1
        }
    }
}

/// `fn main` for an example: `kui_harness::main!(Counter::default());`.
#[macro_export]
macro_rules! main {
    ($example:expr) => {
        fn main() {
            $crate::run(env!("CARGO_BIN_NAME"), $example);
        }
    };
}

/// One line of the dock's stream.
struct Entry {
    frame: u64,
    kind: EntryKind,
    text: String,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum EntryKind {
    Event,
    Warning,
    Note,
}

/// The [`App`] the runner sees: the example's, with the dock beside it.
pub struct Harness<E> {
    example: E,
    name: String,
    dock: Dock,
    /// `None` = follow the OS. Otherwise the base the harness pinned.
    base: Option<Appearance>,
    /// `Some(i)` walks [`ACCENTS`]; `None` is the OS's, or `custom`.
    accent: Option<usize>,
    custom_accent: Option<Color>,
    /// `None` leaves the core's default (the platform's own where there
    /// is one); `Some` is what the user toggled to.
    native_menus: Option<bool>,
    stream: VecDeque<Entry>,
    /// Frames of the main window built so far.
    frames: u64,
    /// How many of `Core::warnings_raised` are already in the stream.
    warnings_seen: usize,
    /// Whether the stream grew since it was last scrolled to its end.
    stream_dirty: bool,
    smoke_frames: Option<u64>,
}

impl<E: Example> Harness<E> {
    pub fn new(
        name: &str,
        example: E,
        dock: Dock,
        base: Option<Appearance>,
        accent: Option<Color>,
    ) -> Self {
        let native_menus = example.native_menus();
        Harness {
            example,
            name: name.to_string(),
            dock,
            base,
            accent: None,
            custom_accent: accent,
            native_menus,
            stream: VecDeque::new(),
            frames: 0,
            warnings_seen: 0,
            stream_dirty: false,
            smoke_frames: std::env::var("KUI_SMOKE_FRAMES")
                .ok()
                .and_then(|s| s.parse().ok()),
        }
    }

    /// The example, for a test or a drive that holds the harness.
    pub fn example(&mut self) -> &mut E {
        &mut self.example
    }

    /// The source the base and the accent add up to — the same three
    /// `ThemeSource`s `theme.rs` used to walk.
    fn source(&self, os_accent: Option<Color>) -> ThemeSource {
        let accent = self
            .accent
            .map(|i| Color::hex(ACCENTS[i].1))
            .or(self.custom_accent);
        match (self.base, accent) {
            (None, None) => ThemeSource::Derived,
            (None, Some(c)) => ThemeSource::DerivedWithAccent(c),
            (Some(app), c) => ThemeSource::Pinned(Theme::derive(app, c.or(os_accent))),
        }
    }

    fn base_name(&self) -> &'static str {
        match self.base {
            None => "OS",
            Some(Appearance::Light) => "light",
            Some(_) => "dark",
        }
    }

    fn accent_name(&self) -> String {
        match (self.accent, self.custom_accent) {
            (Some(i), _) => ACCENTS[i].0.to_string(),
            (None, Some(c)) => format!("#{:06x}", c.to_hex() >> 8),
            (None, None) => "OS".to_string(),
        }
    }

    fn push(&mut self, kind: EntryKind, text: String) {
        if self.stream.len() >= STREAM_CAP {
            self.stream.pop_front();
        }
        self.stream.push_back(Entry {
            frame: self.frames,
            kind,
            text,
        });
        self.stream_dirty = true;
    }

    fn note(&mut self, text: impl Into<String>) {
        self.push(EntryKind::Note, text.into());
    }

    /// One of the harness's own actions, by the name its button and its
    /// chord share. Returns whether `what` was one.
    fn act(&mut self, what: &str) -> bool {
        match what {
            "base" => {
                self.base = match self.base {
                    None => Some(Appearance::Light),
                    Some(Appearance::Light) => Some(Appearance::Dark),
                    Some(_) => None,
                };
                self.note(format!("theme base: {}", self.base_name()));
            }
            "accent" => {
                self.custom_accent = None;
                self.accent = match self.accent {
                    None => Some(0),
                    Some(i) if i + 1 < ACCENTS.len() => Some(i + 1),
                    Some(_) => None,
                };
                self.note(format!("accent: {}", self.accent_name()));
            }
            "menus" => {
                // Toggled from what the core is doing now, which is the
                // platform's default until the first press.
                self.native_menus = Some(!self.native_menus.unwrap_or(cfg!(target_os = "macos")));
                self.note(format!(
                    "menus: {}",
                    if self.native_menus == Some(true) {
                        "native"
                    } else {
                        "drawn"
                    }
                ));
            }
            "dock" => {
                self.dock = self.dock.next();
                self.note(format!("dock: {}", self.dock.name()));
            }
            "clear" => {
                self.stream.clear();
            }
            _ => return false,
        }
        true
    }

    /// Whether `ev` is one of the harness's chords, acted on if so.
    fn chord(&mut self, ev: &UiEvent) -> bool {
        let p = &ev.payload;
        let flag = |k: &str| p.get(k).and_then(Value::as_bool).unwrap_or(false);
        if p.get("kind").and_then(Value::as_str) != Some("key")
            || p.get("phase").and_then(Value::as_str) != Some("down")
            || !flag("ctrl")
            || !flag("shift")
            || flag("alt")
        {
            return false;
        }
        // `code` is what the layout produced with Shift applied; the
        // physical position is the fallback for a layout that produced
        // something else under Control.
        let letter = |k: &str| {
            p.get(k)
                .and_then(Value::as_str)
                .and_then(|s| s.chars().next())
                .map(|c| c.to_ascii_lowercase())
        };
        let what = match letter("code").or_else(|| letter("physical")) {
            Some('t') => "base",
            Some('a') => "accent",
            Some('m') => "menus",
            Some('d') => "dock",
            Some('c') => "clear",
            _ => return false,
        };
        self.act(what)
    }

    fn dock_panel(&mut self, ui: &mut Ui<'_>, t: &Theme) {
        let side = self.dock == Dock::Side;
        let spec = if side {
            NodeSpec::column()
                .width(Sizing::Fixed(DOCK_SIDE_W))
                .height(Sizing::Grow(1.0))
        } else {
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Fixed(DOCK_BOTTOM_H))
        }
        .bg(t.surface)
        .border(1.0, t.border)
        .pad(10.0)
        .gap(10.0)
        // Decorative to assistive technology and skipped by the Tab ring,
        // subtree and all: the example's access tree stays the example's.
        .role(Role::None);
        ui.open_keyed("dock", spec.clip());
        if side {
            // Everything but the stream refuses to shrink: when the
            // window is short the stream gives way, and what is left
            // is clipped at the dock's bottom rather than overlapped.
            fixed(ui, |ui| self.header(ui, t));
            fixed(ui, widgets::latency_graph);
            self.status(ui, t);
            self.stream_panel(ui, t, Sizing::Grow(1.0));
            fixed(ui, |ui| self.legend(ui, t));
        } else {
            // Two columns: the facts, then what happens. The stream grows
            // into whatever width the window has left, which on a narrow
            // example is most of it.
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(290.0))
                    .height(Sizing::Grow(1.0))
                    .gap(8.0)
                    .scroll_y(),
                |ui| {
                    self.header(ui, t);
                    self.status(ui, t);
                },
            );
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Grow(1.0))
                    .height(Sizing::Grow(1.0))
                    .gap(8.0),
                |ui| {
                    fixed(ui, widgets::latency_graph);
                    self.stream_panel(ui, t, Sizing::Grow(1.0));
                    fixed(ui, |ui| self.legend(ui, t));
                },
            );
        }
        ui.close();
    }

    fn header(&self, ui: &mut Ui<'_>, t: &Theme) {
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .cross_align(Align::Center)
                .gap(8.0),
            |ui| {
                ui.text(&self.name, TextStyle::new(13.0).color(t.fg));
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                let frames = match self.smoke_frames {
                    Some(n) => format!("frame {} / {n}", self.frames),
                    None => format!("frame {}", self.frames),
                };
                ui.text(&frames, TextStyle::new(11.0).color(t.muted).mono());
            },
        );
    }

    /// The runtime's facts right now, each from the door it comes from —
    /// and, beside each one the harness can change, the small button that
    /// changes it (the chord in its tooltip). One row per fact, so a
    /// control sits next to the thing it controls rather than restating it.
    fn status(&self, ui: &mut Ui<'_>, t: &Theme) {
        let env = ui.env();
        let vp = ui.viewport();
        let scale = ui.core().scale();
        let core = ui.core();
        let focus = core.focus();
        let focus_label = focus.map(|k| match core.label_of(k) {
            _ if k == Key::ROOT => "root".to_string(),
            Some(l) => l.to_string(),
            None => format!("{:08x}", k.0 as u32),
        });
        let focus_visible = core.focus_visible();
        let mods = core.modifiers();
        let native_menus = core.native_menus();
        let windows: Vec<String> = core
            .windows()
            .iter()
            .map(|(id, name)| format!("{name}#{}", id.0))
            .collect();
        let source = match core.theme_source() {
            ThemeSource::Derived => "derived",
            ThemeSource::DerivedWithAccent(_) => "derived + accent",
            ThemeSource::Pinned(_) => "pinned",
        };
        let theme_appearance = ui.theme().appearance.name();
        let opt = |s: Option<String>| s.unwrap_or_else(|| "—".into());
        let hex = |c: Color| format!("#{:06x}", c.to_hex() >> 8);
        // `(label, value, control)`: the control is the harness action the
        // small button posts, with its tooltip.
        let rows: Vec<Row> = vec![
            ("appearance", env.system.appearance.name().into(), None),
            ("motion", env.system.motion.name().into(), None),
            (
                "locale",
                opt(env.system.locale.map(|l| l.to_string())),
                None,
            ),
            (
                "base",
                format!("{theme_appearance} · {source}"),
                Some((
                    "base",
                    "cycle",
                    "Ctrl+Shift+T · follow the OS → light → dark",
                )),
            ),
            (
                "accent",
                format!("{} · {}", hex(t.accent), self.accent_name()),
                Some((
                    "accent",
                    "cycle",
                    "Ctrl+Shift+A · the OS's, kui's, four the OS might report",
                )),
            ),
            (
                "menus",
                if native_menus { "native" } else { "drawn" }.into(),
                Some((
                    "menus",
                    "toggle",
                    "Ctrl+Shift+M · the platform's own menu, or the core's drawn one",
                )),
            ),
            (
                "dock",
                self.dock.name().into(),
                Some(("dock", "move", "Ctrl+Shift+D · side → bottom → off")),
            ),
            (
                "window",
                format!(
                    "#{}{}{}{} · {}",
                    env.window.id.0,
                    if env.window.custom_chrome {
                        " custom-chrome"
                    } else {
                        ""
                    },
                    if env.window.maximized {
                        " maximized"
                    } else {
                        ""
                    },
                    if env.window.fullscreen {
                        " fullscreen"
                    } else {
                        ""
                    },
                    windows.join(" "),
                ),
                None,
            ),
            (
                "viewport",
                format!(
                    "{}×{} @{scale} · {}",
                    vp.w.round(),
                    vp.h.round(),
                    opt(env.refresh_hz.map(|hz| format!("{hz:.0} Hz"))),
                ),
                None,
            ),
            (
                "keyboard",
                if env.focused {
                    "this window"
                } else {
                    "elsewhere"
                }
                .into(),
                None,
            ),
            (
                "focus",
                match focus_label {
                    Some(l) if focus_visible => format!("{l} · ring"),
                    Some(l) => l,
                    None => "—".into(),
                },
                None,
            ),
            (
                "modifiers",
                {
                    let mut m = Vec::new();
                    if mods.shift {
                        m.push("shift");
                    }
                    if mods.ctrl {
                        m.push("ctrl");
                    }
                    if mods.alt {
                        m.push("alt");
                    }
                    if mods.super_key {
                        m.push("super");
                    }
                    if m.is_empty() {
                        "—".into()
                    } else {
                        m.join("+")
                    }
                },
                None,
            ),
            (
                "audio",
                format!("{} · {} live", env.audio.device.name(), env.audio.live),
                None,
            ),
        ];
        ui.with(
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                // Never squeezed to make room: a fact half-drawn is worse
                // than a stream a line shorter.
                .min_height(Min::FIT)
                .gap(3.0),
            |ui| {
                for (k, v, control) in rows {
                    ui.with(
                        NodeSpec::row()
                            .width(Sizing::Grow(1.0))
                            .gap(8.0)
                            .cross_align(Align::Center),
                        |ui| {
                            ui.with(NodeSpec::row().width(Sizing::Fixed(70.0)), |ui| {
                                ui.text(k, TextStyle::new(11.0).color(t.muted));
                            });
                            ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |ui| {
                                ui.text(&v, TextStyle::new(11.0).color(t.fg).mono().nowrap());
                            });
                            if let Some((what, text, hint)) = control {
                                small_button(ui, t, what, text, hint);
                            }
                        },
                    );
                }
            },
        );
    }

    fn stream_panel(&mut self, ui: &mut Ui<'_>, t: &Theme, height: Sizing) {
        // The core's warnings since the last frame, into the stream: the
        // runner drains and prints them, so this reads the log it leaves.
        let raised = ui.core().warnings_raised();
        if raised.len() > self.warnings_seen {
            let new: Vec<String> = raised[self.warnings_seen..]
                .iter()
                .map(|w| format!("warning [{}] {}", w.code, w.message))
                .collect();
            self.warnings_seen = raised.len();
            for text in new {
                self.push(EntryKind::Warning, text);
            }
        }
        ui.with(
            NodeSpec::row()
                .width(Sizing::Grow(1.0))
                .min_height(Min::FIT)
                .cross_align(Align::Center)
                .gap(8.0),
            |ui| {
                ui.text("events", TextStyle::new(11.0).color(t.muted));
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)), |_| {});
                small_button(ui, t, "clear", "clear", "Ctrl+Shift+C · empty the stream");
            },
        );
        let key = ui.open_keyed(
            "stream",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(height)
                .min_height(60.0)
                .bg(t.sunken)
                .radius(6.0)
                .pad(6.0)
                .gap(1.0)
                .scroll_y(),
        );
        if self.stream.is_empty() {
            ui.text(
                "events arrive here as data",
                TextStyle::new(11.0).color(t.faint),
            );
        }
        for e in &self.stream {
            let color = match e.kind {
                EntryKind::Event => t.fg,
                EntryKind::Warning => t.warning,
                EntryKind::Note => t.muted,
            };
            ui.text(
                &format!("{:>4} {}", e.frame, e.text),
                TextStyle::new(11.0).color(color).mono(),
            );
        }
        ui.close();
        if self.stream_dirty {
            // Follow the newest line; a wheel on the panel takes over
            // until the next entry.
            ui.set_scroll(key, Vec2::new(0.0, f32::MAX));
            self.stream_dirty = false;
        }
    }

    fn legend(&self, ui: &mut Ui<'_>, t: &Theme) {
        if E::KEYS.is_empty() {
            return;
        }
        ui.with(NodeSpec::column().width(Sizing::Grow(1.0)).gap(2.0), |ui| {
            for (keys, what) in E::KEYS {
                ui.with(NodeSpec::row().width(Sizing::Grow(1.0)).gap(8.0), |ui| {
                    ui.with(NodeSpec::row().width(Sizing::Fixed(74.0)), |ui| {
                        ui.text(keys, TextStyle::new(11.0).color(t.accent).mono());
                    });
                    ui.text(what, TextStyle::new(11.0).color(t.muted));
                });
            }
        });
    }
}

impl<E: Example> App for Harness<E> {
    fn view(&mut self, ui: &mut Ui<'_>) {
        // Only the main window gets the dock; a popup or a second window
        // the example declares is the example's alone.
        if &*ui.window_name() != "main" {
            self.example.view(ui);
            return;
        }
        self.frames += 1;
        let os_accent = ui.env().system.accent;
        ui.core().set_theme_source(self.source(os_accent));
        if let Some(m) = self.native_menus {
            ui.core().set_native_menus(m);
        }
        let t = ui.theme();
        let title = format!("kui — {}", self.name);
        ui.window_title(&title);

        if self.dock == Dock::Off {
            // Bare: the example's tree under the harness's root, nothing
            // else — no sink, no focus, nothing an audit could see.
            ui.configure_root(NodeSpec::column().fill().bg(t.bg));
            self.example.view(ui);
            return;
        }

        // The root is the harness's key sink: it encloses everything, so
        // a key nothing below claims bubbles here (ADR 0011) whatever the
        // example focused, and the ring never stops on it. Tagged, so
        // `on_event` can tell its keys from the example's own.
        let root = |dock: Dock| {
            match dock {
                Dock::Bottom => NodeSpec::column(),
                _ => NodeSpec::row(),
            }
            .fill()
            .bg(t.bg)
            .on_key(Value::str(SINK_TAG))
        };
        ui.configure_root(root(self.dock));
        ui.with_keyed(
            "example",
            NodeSpec::column()
                .width(Sizing::Grow(1.0))
                .height(Sizing::Grow(1.0))
                .clip(),
            |ui| self.example.view(ui),
        );
        // An example that configured the root anyway configured ours;
        // the last write wins, and it is this one.
        ui.configure_root(root(self.dock));

        self.dock_panel(ui, &t);

        // Somewhere for the chords to land when the example has nothing
        // focused. From the second frame, so an `initial_focus` of the
        // example's own gets the first; edge-triggered in the core, so a
        // Tab or a click afterwards moves it freely, and a blur brings it
        // back here.
        if self.frames >= 2 && ui.key_focus().is_none() {
            ui.take_key_focus(Key::ROOT);
        }
    }

    fn on_event(&mut self, ev: UiEvent) {
        if let Some(what) = ev.payload.get("harness").and_then(Value::as_str) {
            let what = what.to_string();
            self.act(&what);
            return;
        }
        if self.chord(&ev) {
            return;
        }
        // The harness's own sink is the root, so whatever lands on the
        // root key — a key nothing claimed, a `modifiers` change — is the
        // harness's: the example declared nothing there and would never
        // have heard it. Logged as data, and not forwarded.
        if ev.payload.get("tag").and_then(Value::as_str) == Some(SINK_TAG)
            || (self.dock != Dock::Off && ev.key == Key::ROOT)
        {
            self.push(
                EntryKind::Note,
                format!("(root) {}", fmt_value(&ev.payload)),
            );
            return;
        }
        self.push(EntryKind::Event, fmt_event(&ev));
        self.example.on_event(ev);
    }

    fn setup(&mut self, waker: Waker) {
        self.example.setup(waker);
    }
}

/// One status row: its label, its value, and the control beside it —
/// `(action, button text, tooltip)` — where the harness can change it.
type Row = (
    &'static str,
    String,
    Option<(&'static str, &'static str, &'static str)>,
);

/// A section of the dock that keeps its height whatever the window's.
fn fixed(ui: &mut Ui<'_>, f: impl FnOnce(&mut Ui<'_>)) {
    ui.with(
        NodeSpec::column()
            .width(Sizing::Grow(1.0))
            .min_height(Min::FIT),
        f,
    );
}

/// A control the size of the row it sits in: the harness's own, drawn
/// off the palette, posting `{harness: what}` like the chords act on.
fn small_button(ui: &mut Ui<'_>, t: &Theme, what: &str, text: &str, hint: &str) {
    let key = ui.child_key(what);
    ui.with_keyed(
        what,
        NodeSpec::row()
            .pad_xy(7.0, 1.0)
            .radius(4.0)
            .bg(t.raised)
            .hover_bg(t.hover)
            .pressed_bg(t.pressed)
            .border(1.0, t.border)
            .on_click(Value::map([("harness", Value::str(what))]))
            .apply_tooltip(hint),
        |ui| {
            ui.text(text, TextStyle::new(10.0).color(t.fg));
            if ui.is_hovered(key) {
                widgets::tooltip(ui, hint);
            }
        },
    );
}

/// `key payload`, with the key as a label where one names it.
fn fmt_event(ev: &UiEvent) -> String {
    let key = if ev.key == Key::ROOT {
        "root".to_string()
    } else {
        format!("{:08x}", ev.key.0 as u32)
    };
    let window = if ev.window.0 == 0 {
        String::new()
    } else {
        format!(" w{}", ev.window.0)
    };
    format!("{key}{window} {}", fmt_value(&ev.payload))
}

/// A `Value` as one line of data: `{kind: click, n: 3}`.
pub fn fmt_value(v: &Value) -> String {
    let mut out = String::new();
    write_value(v, &mut out);
    out
}

fn write_value(v: &Value, out: &mut String) {
    match v {
        Value::Null => out.push_str("null"),
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => out.push_str(&i.to_string()),
        Value::Float(f) => {
            if f.fract() == 0.0 && f.abs() < 1e9 {
                out.push_str(&format!("{f:.0}"));
            } else {
                out.push_str(&format!("{f:.2}"));
            }
        }
        Value::Str(s) => {
            if s.chars().any(|c| c.is_whitespace() || c == ',' || c == '}') || s.is_empty() {
                out.push_str(&format!("{s:?}"));
            } else {
                out.push_str(s);
            }
        }
        Value::List(items) => {
            out.push('[');
            for (i, item) in items.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                write_value(item, out);
            }
            out.push(']');
        }
        Value::Map(entries) => {
            out.push('{');
            for (i, (k, v)) in entries.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(k);
                out.push_str(": ");
                write_value(v, out);
            }
            out.push('}');
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct Blank {
        events: Vec<UiEvent>,
    }
    impl App for Blank {
        fn view(&mut self, ui: &mut Ui<'_>) {
            ui.with(NodeSpec::column().fill(), |ui| {
                widgets::button(ui, "press", Value::map([("kind", Value::str("pressed"))]));
            });
        }
        fn on_event(&mut self, ev: UiEvent) {
            self.events.push(ev);
        }
    }
    impl Example for Blank {}

    fn frame(h: &mut Harness<Blank>, core: &mut Core) {
        let mut ui = core.frame(kui::Size::new(800.0, 600.0), 1.0);
        h.view(&mut ui);
        ui.finish();
    }

    #[test]
    fn the_cli_parses_its_flags_and_leaves_the_examples() {
        let cli = Cli::parse(
            "x",
            [
                "--headless",
                "--dock=bottom",
                "--dark",
                "--accent",
                "#ff0000",
                "--size",
                "300x200",
                "--mine",
            ]
            .map(String::from),
            &[("--mine", "the example's own")],
        )
        .unwrap();
        assert!(cli.headless);
        assert_eq!(cli.dock, Some(Dock::Bottom));
        assert_eq!(cli.base, Some(Appearance::Dark));
        assert_eq!(cli.accent.map(|c| c.to_hex()), Some(0xff0000ff));
        assert_eq!(cli.size, Some((300.0, 200.0)));
        assert!(Cli::parse("x", ["--nope".to_string()], &[]).is_err());
        assert!(Cli::parse("x", ["--dock".to_string(), "left".to_string()], &[]).is_err());
    }

    /// The dock builds beside the example, the example's tree is intact,
    /// and nothing the dock adds is in the access tree or the ring.
    #[test]
    fn the_dock_is_beside_the_example_and_invisible_to_the_ring() {
        let mut core = Core::new();
        let mut h = Harness::new("blank", Blank::default(), Dock::Side, None, None);
        frame(&mut h, &mut core);
        frame(&mut h, &mut core);
        assert!(core.key_of("dock").is_some());
        assert!(core.key_of("stream").is_some());
        // The example's button is the only control Tab reaches: the dock's
        // five buttons are under `role = none`.
        let button = core.key_of("press").unwrap();
        core.focus_next(true);
        assert_eq!(core.focus(), Some(button));
        core.focus_next(true);
        assert_eq!(core.focus(), Some(button), "the ring has one member");
    }

    /// A click on the example reaches it and lands in the stream; a click
    /// on a dock button is the harness's and does not.
    #[test]
    fn events_flow_to_the_example_and_into_the_stream() {
        let mut core = Core::new();
        let mut h = Harness::new("blank", Blank::default(), Dock::Side, None, None);
        frame(&mut h, &mut core);
        let press = core.key_of("press").unwrap();
        let base = core.key_of("base").unwrap();
        assert_ne!(h.base, Some(Appearance::Light));
        for ev in core.handle_input(kui::InputEvent::Access(kui::AccessRequest::new(
            base,
            kui::AccessAction::Click,
        ))) {
            h.on_event(ev);
        }
        assert_eq!(h.base, Some(Appearance::Light), "the dock button acted");
        assert!(h.example.events.is_empty(), "and the example never saw it");
        for ev in core.handle_input(kui::InputEvent::Access(kui::AccessRequest::new(
            press,
            kui::AccessAction::Click,
        ))) {
            h.on_event(ev);
        }
        assert_eq!(h.example.events.len(), 1);
        assert!(
            h.stream
                .iter()
                .any(|e| e.kind == EntryKind::Event && e.text.contains("pressed"))
        );
    }

    #[test]
    fn a_chord_is_the_harness_s_whatever_sink_heard_it() {
        let mut h = Harness::new("blank", Blank::default(), Dock::Side, None, None);
        let chord = |c: char| UiEvent {
            origin: kui::OriginId::HOST,
            window: kui::WindowId::MAIN,
            key: Key::ROOT,
            payload: Value::map([
                ("kind", Value::str("key")),
                ("phase", Value::str("down")),
                ("code", Value::str(c.to_string())),
                ("physical", Value::str(c.to_string().to_lowercase())),
                ("ctrl", Value::Bool(true)),
                ("shift", Value::Bool(true)),
                ("alt", Value::Bool(false)),
            ]),
        };
        h.on_event(chord('D'));
        assert_eq!(h.dock, Dock::Bottom);
        h.on_event(chord('M'));
        assert!(h.native_menus.is_some());
        assert!(h.example.events.is_empty(), "no chord reached the example");
        // A bare key on the root is the harness's too — the root is its
        // sink, and the example declared nothing there.
        let mut plain = chord('d');
        if let Value::Map(m) = &mut plain.payload {
            m.retain(|(k, _)| k != "ctrl");
        }
        h.on_event(plain.clone());
        assert!(
            h.example.events.is_empty(),
            "a root key is not the example's"
        );
        assert!(h.stream.iter().any(|e| e.kind == EntryKind::Note));
        // The same key from the example's own sink is the example's.
        plain.key = Key::ROOT.str("pane");
        h.on_event(plain);
        assert_eq!(h.example.events.len(), 1);
    }

    #[test]
    fn values_print_as_one_line_of_data() {
        let v = Value::map([
            ("kind", Value::str("click")),
            ("n", Value::Int(3)),
            (
                "at",
                Value::List(vec![Value::Float(1.5), Value::Float(2.0)]),
            ),
            ("text", Value::str("two words")),
        ]);
        assert_eq!(
            fmt_value(&v),
            "{kind: click, n: 3, at: [1.50, 2], text: \"two words\"}"
        );
    }
}

/// The two pins ADR 0021 asks for, in the pattern ADR 0020's header audit
/// set: the manifests are read, and every mirror of them is checked
/// against them, so a file moved without its index line — or a headless
/// list naming an example that no longer exists — fails `cargo test`
/// rather than going quietly stale.
#[cfg(test)]
mod pins {
    use std::path::{Path, PathBuf};

    fn root() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
    }

    /// The `[[example]]` names and the `headless = [...]` list of one
    /// manifest, read by line: a manifest is small and this needs no
    /// parser to stay honest about.
    fn manifest(path: &Path) -> (Vec<String>, Vec<String>) {
        let text =
            std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        let mut examples = Vec::new();
        let mut headless = Vec::new();
        let mut in_example = false;
        let mut in_metadata = false;
        let mut in_headless = false;
        for line in text.lines() {
            let line = line.trim();
            if line.starts_with('[') {
                in_example = line == "[[example]]";
                in_metadata = line == "[package.metadata.kui]";
                continue;
            }
            if in_example && let Some(rest) = line.strip_prefix("name = \"") {
                examples.push(rest.trim_end_matches('"').to_string());
            }
            if in_metadata && line.starts_with("headless = [") {
                in_headless = !line.ends_with(']');
                for name in line["headless = [".len()..]
                    .trim_end_matches(']')
                    .split(',')
                {
                    let name = name.trim().trim_matches('"');
                    if !name.is_empty() {
                        headless.push(name.to_string());
                    }
                }
                continue;
            }
            if in_headless {
                if line.starts_with(']') {
                    in_headless = false;
                } else {
                    let name = line.trim_end_matches(',').trim_matches('"');
                    if !name.is_empty() {
                        headless.push(name.to_string());
                    }
                }
            }
        }
        (examples, headless)
    }

    const CRATES: &[&str] = &["kui", "kui-core", "kui-ffi", "kui-lua"];

    /// Every name in a `headless = [...]` list is an `[[example]]` of the
    /// same crate: `scripts/smoke-headless.sh` runs what the list says.
    #[test]
    fn every_headless_name_is_an_example() {
        for krate in CRATES {
            let (examples, headless) =
                manifest(&root().join("crates").join(krate).join("Cargo.toml"));
            for name in &headless {
                assert!(
                    examples.contains(name),
                    "crates/{krate}/Cargo.toml lists `{name}` as headless, and has no such example"
                );
            }
        }
    }

    /// Every `[[example]]` in the workspace is a row in `examples/README.md`,
    /// linked by its file — the index a reader looks in, pinned to the
    /// manifests so a move cannot leave it pointing at a file that is not
    /// there.
    #[test]
    fn every_example_is_in_the_readme() {
        let readme = std::fs::read_to_string(root().join("examples/README.md")).unwrap();
        for krate in CRATES {
            let path = root().join("crates").join(krate).join("Cargo.toml");
            let text = std::fs::read_to_string(&path).unwrap();
            // The path each `[[example]]` names, relative to examples/.
            for line in text.lines().map(str::trim) {
                let Some(rest) = line.strip_prefix("path = \"../../examples/") else {
                    continue;
                };
                let rel = rest.trim_end_matches('"');
                assert!(
                    readme.contains(&format!("]({rel})")),
                    "examples/README.md has no link to `{rel}` (an example of {krate})"
                );
                assert!(
                    root().join("examples").join(rel).exists(),
                    "crates/{krate}/Cargo.toml names `{rel}`, which does not exist"
                );
            }
        }
        // The Node examples too: what package.json builds is what the
        // README lists.
        let pkg = std::fs::read_to_string(root().join("examples/node/package.json")).unwrap();
        for dir in ["apps", "features", "widgets", "tools"] {
            assert!(
                pkg.contains(&format!("{dir}/*.tsx")),
                "package.json builds {dir}/*.tsx"
            );
            for entry in std::fs::read_dir(root().join("examples/node").join(dir)).unwrap() {
                let name = entry.unwrap().file_name().to_string_lossy().to_string();
                if name.ends_with(".tsx") || name.ends_with(".mjs") {
                    assert!(
                        readme.contains(&format!("](node/{dir}/{name})")),
                        "examples/README.md has no link to `node/{dir}/{name}`"
                    );
                }
            }
        }
    }
}
