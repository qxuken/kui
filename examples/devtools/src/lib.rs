//! The harness every example in this repository runs inside
//! (`docs/adr/0021-one-subject-per-example.md`, decision 6): the window
//! title, the `--headless` parsing, and the doors that open the core's
//! devtools panel around the example
//! (`docs/adr/0024-the-devtools-are-the-cores.md`).
//!
//! An example is one subject — a widget, a feature, an app — and nothing
//! else. What every example used to carry beside its subject lives here
//! once, and what the dock used to be lives in the core: `kui_core::devtools`
//! draws it, hears its own controls and chords, logs every event on its
//! way to the example, and can dock beside it or open a window of its own.
//! The harness asks for it with the launcher's [`kui_native::Launcher::devtools`]
//! and seeds it from the command line — `--dock`, `--light`, `--dark`,
//! `--accent`, `--key` — and from the example's [`Example::KEYS`] legend.
//!
//! The example's root is wrapped by the core while the panel is docked
//! (ADR 0024, decision 2), and its keys do not move for it: an example
//! addresses its nodes as it always did. `--dock off` keeps the panel's
//! chords live and draws nothing, which is what the accessibility audit
//! runs under.
//!
//! `--shot` and `--record` draw the example headlessly and keep it as a
//! PNG, or an animated GIF or PNG ([`picture`]): the pictures in the book's
//! Examples part.
//!
//! One `main` per example: `kui_devtools::main!(Counter::default())`.

use kui_native::{
    Accel, App, Appearance, BarMenu, Chrome, Color, Core, Extensions, MenuBar, MenuItem,
    MotionPref, SystemEnv, Ui, UiEvent, Value, Waker,
};

mod drive;
pub use drive::Drive;
pub mod manifest;
pub mod picture;
/// Where the panel sits: the core's own placement.
pub use kui_native::DevtoolsDock as Dock;
/// The dock's extents, for the window the harness sizes around them.
pub use kui_native::devtools::{DOCK_BOTTOM_H, DOCK_SIDE_W};
pub use picture::{Cue, Recording};

/// The window height a side dock needs to show all of itself, and the
/// width a bottom one does: a smaller example's window is raised to it.
const DOCK_SIDE_MIN_H: f64 = 600.0;
const DOCK_BOTTOM_MIN_W: f64 = 640.0;
/// What `Example::headless` answers by default, and the message the exit
/// code 2 carries: listed in a smoke round without a drive to run.
pub const NO_HEADLESS: &str = "no headless drive";

/// The window an example asks for. The harness adds the dock's extent to
/// `size`, so the example's content gets exactly what it asked for.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    /// Initial inner size of the example's area, logical px.
    pub size: (f64, f64),
    pub min_size: Option<(f64, f64)>,
    pub max_size: Option<(f64, f64)>,
    pub chrome: Chrome,
    /// What shows through the window (`Launcher::backdrop`).
    pub backdrop: kui_native::Backdrop,
    /// The macOS titlebar's height under custom chrome
    /// (`Launcher::titlebar`).
    pub titlebar: kui_native::Titlebar,
}

impl Default for Window {
    fn default() -> Self {
        Window {
            size: (960.0, 640.0),
            min_size: None,
            max_size: None,
            chrome: Chrome::Native,
            backdrop: kui_native::Backdrop::Opaque,
            titlebar: kui_native::Titlebar::Standard,
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

    pub fn backdrop(mut self, backdrop: kui_native::Backdrop) -> Self {
        self.backdrop = backdrop;
        self
    }

    pub fn titlebar(mut self, titlebar: kui_native::Titlebar) -> Self {
        self.titlebar = titlebar;
        self
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
    /// What `--record` plays, for an example whose subject is motion:
    /// the cues that start it and how long to keep the clock running.
    /// `None`, the default, records nothing.
    const RECORDING: Option<Recording> = None;

    /// The window the example wants.
    fn window(&self) -> Window {
        Window::default()
    }

    /// Where the dock goes unless `--dock` says.
    fn dock(&self) -> Dock {
        Dock::Right
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

    /// Whether the harness declares its menu bar — the platform's
    /// standard menus and a View menu that shows or hides the devtools
    /// panel. An example that declares a bar of its own says `false`:
    /// two declarations a frame would replace each other.
    fn devtools_menu(&self) -> bool {
        true
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
    /// `--motion`: pins `env.system.motion` over the OS's (backlog F47),
    /// to see what the example draws for a user who asked for less.
    pub motion: Option<MotionPref>,
    /// `--key`: respells the chord that moves the keyboard into the
    /// panel (`Core::set_devtools_key`), for an example whose own keymap
    /// wants `Ctrl+Shift+I`.
    pub key: Option<Accel>,
    /// `--shot`: where the settled frame goes, as a PNG.
    pub shot: Option<std::path::PathBuf>,
    /// `--record`: where the example's [`Example::RECORDING`] goes, as a
    /// GIF for a `.gif` path and an animated PNG otherwise.
    pub record: Option<std::path::PathBuf>,
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
                    let v = value("left, right, bottom, window or off")?;
                    cli.dock = Some(Dock::parse(&v).ok_or_else(|| {
                        format!("--dock: {v:?} is not left, right, bottom, window or off")
                    })?);
                }
                "--light" => cli.base = Some(Appearance::Light),
                "--dark" => cli.base = Some(Appearance::Dark),
                "--accent" => {
                    let v = value("#rrggbb")?;
                    cli.accent = Some(
                        parse_color(&v).ok_or_else(|| format!("--accent: {v:?} is not #rrggbb"))?,
                    );
                }
                "--motion" => {
                    let v = value("full or reduced")?;
                    cli.motion = Some(
                        MotionPref::parse(&v)
                            .filter(|m| *m != MotionPref::Unknown)
                            .ok_or_else(|| format!("--motion: {v:?} is not full or reduced"))?,
                    );
                }
                "--key" => {
                    let v = value("a chord, like f12 or mod+shift+d")?;
                    cli.key = Some(
                        Accel::parse(&v)
                            .ok_or_else(|| format!("--key: {v:?} is not a chord kui can name"))?,
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
                "--shot" => cli.shot = Some(value("a .png path")?.into()),
                "--record" => cli.record = Some(value("a .png or .gif path")?.into()),
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
        "usage: {name} [--headless] [--dock side|bottom|off] [--light|--dark] [--accent #rrggbb] [--motion full|reduced] [--key CHORD] [--size WxH] [--shot PNG] [--record PNG|GIF]"
    );
    for (f, doc) in flags {
        s.push_str(&format!(" [{f}]"));
        let _ = doc;
    }
    s.push_str("\n\n  --headless       drive the example through a bare Core and exit 0 on the right answer\n");
    s.push_str("  --dock WHERE     where the harness dock sits (default: what the example asks)\n");
    s.push_str("  --light, --dark  pin the theme base instead of following the OS\n");
    s.push_str("  --accent COLOUR  the accent, instead of the OS's\n");
    s.push_str("  --motion PREF    what env.system.motion reads, instead of the OS's\n");
    s.push_str(
        "  --key CHORD      the chord into the dock, instead of Ctrl+Shift+I (f12, mod+shift+d)\n",
    );
    s.push_str("  --size WxH       the example's area, logical px (the dock is added)\n");
    s.push_str(
        "  --shot PNG       draw the settled frame headlessly, light unless --dark, and save it\n",
    );
    s.push_str("  --record FILE    play the example's recording headlessly and save it, a GIF for .gif, else an APNG;\n");
    s.push_str("                   an example with none writes nothing and says so\n");
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
    if cli.shot.is_some() || cli.record.is_some() {
        return pictures(name, &mut example, &cli);
    }
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
        Dock::Left | Dock::Right => {
            w += DOCK_SIDE_W as f64;
            h = h.max(DOCK_SIDE_MIN_H);
        }
        Dock::Bottom => {
            h += DOCK_BOTTOM_H as f64;
            w = w.max(DOCK_BOTTOM_MIN_W);
        }
        Dock::Window | Dock::Off => {}
    }
    let extensions = example.extensions();
    let native_menus = example.native_menus();
    let devtools_menu = example.devtools_menu();
    let legend: Vec<(&'static str, &'static str)> = E::KEYS.to_vec();
    let (base, accent) = (cli.base, cli.accent);
    let harness = Harness::new(name, example);
    let mut launcher = kui_native::app(&format!("kui — {name}"))
        .size(w, h)
        .chrome(window.chrome)
        .backdrop(window.backdrop)
        .titlebar(window.titlebar)
        .with_extensions(extensions)
        .devtools(true)
        .setup_core(move |core| {
            Harness::<E>::setup_core(core, dock, base, accent, native_menus, &legend);
            // Drawn context and select menus, but the platform's bar kept
            // for the harness's View menu: only an example whose subject
            // is the bar draws one.
            if devtools_menu {
                core.set_native_menu_bar(true);
            }
        });
    if let Some(key) = cli.key {
        launcher = launcher.devtools_key(key);
    }
    if let Some((mw, mh)) = window.min_size {
        launcher = launcher.min_size(mw, mh);
    }
    if let Some((mw, mh)) = window.max_size {
        launcher = launcher.max_size(mw, mh);
    }
    if let Some(motion) = cli.motion {
        launcher = launcher.system(SystemEnv {
            motion,
            ..Default::default()
        });
    }
    match launcher.run(harness) {
        Ok(()) => 0,
        Err(e) => {
            eprintln!("{name}: {e}");
            1
        }
    }
}

/// `--shot` and `--record`, in that order when both are given; returns
/// the exit code: 0 when every file asked for was written, or there was
/// no recording to write, 1 when one failed.
fn pictures<E: Example>(name: &str, example: &mut E, cli: &Cli) -> i32 {
    if let Some(path) = &cli.shot {
        if let Err(e) = picture::shot(example, cli, path) {
            eprintln!("{name}: --shot: {e}");
            return 1;
        }
        println!("{name}: {}", path.display());
    }
    if let Some(path) = &cli.record {
        let Some(recording) = E::RECORDING else {
            println!("{name}: no recording");
            return 0;
        };
        if let Err(e) = picture::record(example, cli, recording, path) {
            eprintln!("{name}: --record: {e}");
            return 1;
        }
        println!("{name}: {}", path.display());
    }
    0
}

/// `fn main` for an example: `kui_devtools::main!(Counter::default());`.
#[macro_export]
macro_rules! main {
    ($example:expr) => {
        fn main() {
            $crate::run(env!("CARGO_BIN_NAME"), $example);
        }
    };
}

/// The [`App`] the runner sees: the example, with the window title the
/// harness gives it. The panel around it is the core's.
pub struct Harness<E> {
    example: E,
    name: String,
}

impl<E: Example> Harness<E> {
    pub fn new(name: &str, example: E) -> Self {
        Harness {
            example,
            name: name.to_string(),
        }
    }

    /// The example, for a test or a drive that holds the harness.
    pub fn example(&mut self) -> &mut E {
        &mut self.example
    }

    /// The doors the command line and the example reach the panel
    /// through, opened once on the main core before its first frame.
    pub fn setup_core(
        core: &mut Core,
        dock: Dock,
        base: Option<Appearance>,
        accent: Option<Color>,
        native_menus: Option<bool>,
        legend: &[(&str, &str)],
    ) {
        core.set_devtools(true);
        core.set_devtools_dock(dock);
        core.set_devtools_theme(base, accent);
        core.set_devtools_legend(legend);
        if let Some(m) = native_menus {
            core.set_native_menus(m);
            core.set_native_menu_bar(m);
        }
    }
}

/// The harness's menu bar choice: what its View menu's row posts.
const MENU_KIND: &str = "kui-devtools-menu";

/// The platform's standard menus kept beside a View menu whose one row
/// shows the devtools panel, or hides it while it is up — the way back
/// to a panel closed with its ×, beside `Ctrl+Shift+I`. Declared, not
/// drawn: the platform's bar shows it (macOS), and an app drawing its
/// own bar elsewhere has it in the frame.
fn devtools_bar(shown: bool) -> MenuBar {
    let row = if shown {
        "Hide Devtools"
    } else {
        "Show Devtools"
    };
    MenuBar::new(vec![BarMenu::new(
        "View",
        vec![MenuItem::new(row).id(Value::str("devtools"))],
    )])
    .standard_menus(true)
    .on_menu(Value::map([("kind", Value::str(MENU_KIND))]))
}

/// Whether the panel is up: on and not closed to `off`.
fn devtools_shown(core: &Core) -> bool {
    core.devtools() && core.devtools_dock() != Dock::Off
}

impl<E: Example> App for Harness<E> {
    fn view(&mut self, ui: &mut Ui<'_>) {
        if &*ui.window_name() == "main" {
            ui.window_title(&format!("kui — {}", self.name));
            if self.example.devtools_menu() {
                let shown = devtools_shown(ui.core());
                ui.core().declare_menu_bar(devtools_bar(shown));
            }
        }
        self.example.view(ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        if is_menu_choice(&ev) {
            return;
        }
        self.example.on_event(ev);
    }

    fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
        // The View menu's row: the panel up or down.
        if is_menu_choice(&ev) {
            if devtools_shown(core) {
                core.set_devtools_dock(Dock::Off);
            } else {
                core.set_devtools(true);
            }
            return;
        }
        self.example.on_event_with(ev, core);
    }

    fn setup(&mut self, waker: Waker) {
        self.example.setup(waker);
    }

    fn teardown(&mut self) {
        self.example.teardown();
    }
}

/// The harness's own View menu choice, which no example hears.
fn is_menu_choice(ev: &UiEvent) -> bool {
    ev.payload.get_str("kind") == Some(MENU_KIND)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kui_native::{Key, NodeSpec, Value, widgets};

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
    impl Example for Blank {
        const KEYS: &'static [(&'static str, &'static str)] = &[("Space", "press")];
    }

    fn frame(h: &mut Harness<Blank>, core: &mut Core) {
        let mut ui = core.frame(kui_native::Size::new(800.0, 600.0), 1.0);
        h.view(&mut ui);
        ui.finish();
    }

    /// The harness declares the platform's standard menus beside a View
    /// menu whose row hides the panel and shows it again — the way back
    /// to a panel closed with its × — and the example never hears it.
    #[test]
    fn the_view_menu_hides_and_shows_the_panel() {
        let mut core = Core::new();
        Harness::<Blank>::setup_core(&mut core, Dock::Right, None, None, None, &[]);
        let mut h = Harness::new("blank", Blank::default());
        frame(&mut h, &mut core);
        let bar = core.menu_bar().expect("the harness's bar").clone();
        assert!(bar.standard_menus, "beside the platform's own");
        let view = bar.menus.iter().position(|m| m.label == "View").unwrap();
        assert_eq!(bar.menus[view].items[0].label, "Hide Devtools");
        for shown in [false, true] {
            for ev in core.activate_menu_bar_item(view, 0) {
                h.on_event_with(ev, &mut core);
            }
            frame(&mut h, &mut core);
            assert_eq!(devtools_shown(&core), shown);
            let row = &core.menu_bar().unwrap().menus[view].items[0].label;
            assert_eq!(
                row,
                if shown {
                    "Hide Devtools"
                } else {
                    "Show Devtools"
                }
            );
        }
        assert!(
            h.example().events.is_empty(),
            "the example hears none of it"
        );
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
                "--motion=reduced",
                "--key",
                "f12",
                "--shot=a.png",
                "--record",
                "b.png",
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
        assert_eq!(cli.motion, Some(MotionPref::Reduced));
        assert_eq!(cli.key.map(|k| k.spelling()), Some("f12".into()));
        assert_eq!(cli.shot.as_deref(), Some(std::path::Path::new("a.png")));
        assert_eq!(cli.record.as_deref(), Some(std::path::Path::new("b.png")));
        assert!(Cli::parse("x", ["--key", "f99"].map(String::from), &[]).is_err());
        // `unknown` is a reading, not a pin: the flag takes the two answers.
        assert!(Cli::parse("x", ["--motion", "unknown"].map(String::from), &[]).is_err());
        assert_eq!(
            Cli::parse("x", ["--dock", "window"].map(String::from), &[])
                .unwrap()
                .dock,
            Some(Dock::Window)
        );
        assert!(Cli::parse("x", ["--nope".to_string()], &[]).is_err());
        assert!(Cli::parse("x", ["--dock".to_string(), "top".to_string()], &[]).is_err());
        assert_eq!(
            Cli::parse("x", ["--dock=left".to_string()], &[])
                .unwrap()
                .dock,
            Some(Dock::Left)
        );
    }

    /// The harness opens the doors and gets out of the way: the panel is
    /// the core's, the title is the harness's, the example's events are
    /// the example's, and the example's keys are what they are bare.
    #[test]
    fn the_harness_is_a_title_and_the_doors() {
        let mut bare = Core::new();
        let mut h = Harness::new("blank", Blank::default());
        frame(&mut h, &mut bare);
        let press_bare = bare.key_of("press").unwrap();

        let mut core = Core::new();
        Harness::<Blank>::setup_core(
            &mut core,
            Dock::Right,
            Some(Appearance::Dark),
            None,
            None,
            Blank::KEYS,
        );
        frame(&mut h, &mut core);
        frame(&mut h, &mut core);
        assert_eq!(core.window_title(), Some("kui — blank"));
        assert!(core.devtools());
        assert!(core.key_of(kui_native::devtools::DEVTOOLS_KEY).is_some());
        let press = core.key_of("press").unwrap();
        assert_eq!(
            press, press_bare,
            "the panel does not move the example's keys"
        );
        assert_eq!(
            core.theme().appearance,
            Appearance::Dark,
            "--dark reached the panel"
        );
        // A click on the example reaches it; a click on the panel's icon
        // is the core's and never arrives.
        let click = |core: &mut Core, key: Key| {
            core.handle_input(kui_native::InputEvent::Access(
                kui_native::AccessRequest::new(key, kui_native::AccessAction::Click),
            ))
        };
        let facts = core.key_of("kui-devtools/tab-facts").unwrap();
        for ev in click(&mut core, facts) {
            h.on_event(ev);
        }
        frame(&mut h, &mut core);
        let base = core.key_of("kui-devtools/base").unwrap();
        for ev in click(&mut core, base) {
            h.on_event(ev);
        }
        assert!(core.menu().is_some(), "the base select's menu is open");
        assert!(h.example.events.is_empty());
        for ev in click(&mut core, press) {
            h.on_event(ev);
        }
        assert_eq!(h.example.events.len(), 1);
        // `--dock off`: nothing drawn but the panel's holder, no stream.
        let mut off = Core::new();
        Harness::<Blank>::setup_core(&mut off, Dock::Off, None, None, None, &[]);
        frame(&mut h, &mut off);
        frame(&mut h, &mut off);
        assert!(off.key_of("kui-devtools/stream").is_none());
        assert_eq!(off.key_of("press"), Some(press_bare));
    }
}

/// The two pins ADR 0021 asks for, in the pattern ADR 0020's header audit
/// set: the manifests are read, and every mirror of them is checked
/// against them, so a file moved without its index line — or a headless
/// list naming an example that no longer exists — fails `cargo test`
/// rather than going quietly stale.
#[cfg(test)]
mod pins {
    use crate::manifest::{CRATES, crate_manifest, node_roster, root};

    /// Every name in a `headless = [...]` list is an `[[example]]` of the
    /// same crate: `smoke --headless` runs what the list says.
    #[test]
    fn every_headless_name_is_an_example() {
        for krate in CRATES {
            let (examples, headless) = crate_manifest(krate);
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
                // And every example is in a smoke roster (`kui.windowed`
                // or `kui.headless`), read the way `smoke` reads them, so
                // a new one cannot be left out of a round by forgetting a
                // second list; `bench.mjs` is a tool run by hand, not an
                // example.
                if let Some(stem) = name.strip_suffix(".tsx") {
                    let entry = format!("{dir}/{stem}");
                    assert!(
                        node_roster("windowed").contains(&entry)
                            || node_roster("headless").contains(&entry),
                        "examples/node/package.json's `kui` roster does not name {entry}"
                    );
                }
            }
        }
        // And the roster names nothing that is not there: every quoted
        // `<dir>/<name>` in the file (the rosters are the only place one
        // appears without a glob or a scheme).
        for quoted in pkg.split('"').skip(1).step_by(2) {
            let Some((dir, stem)) = quoted.split_once('/') else {
                continue;
            };
            if !["apps", "features", "widgets", "tools"].contains(&dir)
                || stem.contains(['*', '.', '/'])
            {
                continue;
            }
            assert!(
                root()
                    .join("examples/node")
                    .join(dir)
                    .join(format!("{stem}.tsx"))
                    .exists(),
                "package.json's kui roster names `{dir}/{stem}`, which does not exist"
            );
        }
    }
}
