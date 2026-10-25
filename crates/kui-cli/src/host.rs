//! The window's side of a run: the app's script is an extension under
//! `app` that fills the root, Kit one under `kit`, and this host drains
//! the runtime before each frame (answers, requests, workers' messages,
//! dispatches), reloads under `--watch`, closes on `kui.exit`, and shows
//! what went wrong over the window.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use kui_core::{Color, NodeSpec, TextStyle, Value, WindowCommand, WindowId};
use kui_lua::LuaExtension;
use kui_native::{App, Ui, UiEvent};
use mlua::Lua;

use crate::modules::{self, Shared};
use crate::project::Project;

/// What the app's state looked like before its script ran: the modules a
/// reload must not forget.
type Baseline = Rc<RefCell<Vec<String>>>;

/// Builds the app's extension: the runner's modules, Kit's host module and
/// the `.tl` loader installed, then the entry run. With `fallback`, an
/// entry that fails to load still gives an extension (an empty script),
/// and the error back, for a `--watch` run to show until it is fixed.
pub fn load_app(
    project: &Project,
    shared: Arc<Shared>,
    fallback: bool,
) -> Result<(LuaExtension, Baseline, Option<String>), String> {
    let baseline: Baseline = Rc::default();
    let setup = {
        let (shared, baseline, dir) = (shared.clone(), baseline.clone(), project.dir.clone());
        move |lua: &Lua| -> mlua::Result<()> {
            crate::project::add_path(lua, &dir)?;
            modules::install_loader(lua)?;
            crate::teal::install(lua)?;
            crate::kit::install_ui(lua)?;
            modules::install(lua, shared.clone())?;
            let loaded: mlua::Table = lua.globals().get::<mlua::Table>("package")?.get("loaded")?;
            let mut names = Vec::new();
            for pair in loaded.pairs::<String, mlua::Value>() {
                names.push(pair?.0);
            }
            *baseline.borrow_mut() = names;
            Ok(())
        }
    };
    let entry = &project.entry;
    let source = std::fs::read_to_string(entry).map_err(|e| format!("{}: {e}", entry.display()))?;
    let attempt = crate::teal::source_for(entry, &source).and_then(|src| {
        let mut s = setup.clone();
        LuaExtension::from_source_with(format!("@{}", entry.display()), &src, &mut s)
    });
    let ext = match attempt {
        Ok(ext) => ext,
        Err(e) if fallback => {
            let mut s = setup.clone();
            let ext = LuaExtension::from_source_with("kui-app", "", &mut s).map_err(|e| e.to_string())?;
            return Ok((ext, baseline, Some(e.to_string())));
        }
        Err(e) => return Err(e.to_string()),
    };
    init(ext.lua()).map_err(|e| e.to_string())?;
    Ok((ext, baseline, None))
}

/// `model = init()` when the app defines `init` and has no model yet; and
/// the app's `view` wrapped so `kui.next_frame` callbacks run first.
fn init(lua: &Lua) -> mlua::Result<()> {
    let g = lua.globals();
    g.get::<mlua::Table>("kui")?.get::<mlua::Function>("__wrap_view")?.call::<()>(())?;
    if g.get::<mlua::Value>("model")?.is_nil()
        && let Ok(f) = g.get::<mlua::Function>("init")
    {
        let m: mlua::Value = f.call(())?;
        g.set("model", m)?;
    }
    Ok(())
}

/// Files that changed since the last look, by modification time: a thread
/// that polls the app's folder and rings the loop.
pub struct Watch {
    changed: Arc<AtomicBool>,
}

impl Watch {
    pub fn start(project: &Project, shared: Arc<Shared>) -> Watch {
        let changed = Arc::new(AtomicBool::new(false));
        let flag = changed.clone();
        let dir = project.dir.clone();
        let include = project.watch_include.clone();
        let exclude = project.watch_exclude.clone();
        std::thread::Builder::new()
            .name("kui-watch".into())
            .spawn(move || {
                let mut last = snapshot(&dir, &include, &exclude);
                loop {
                    std::thread::sleep(Duration::from_millis(200));
                    let now = snapshot(&dir, &include, &exclude);
                    if now != last {
                        last = now;
                        flag.store(true, Ordering::Relaxed);
                        shared.ops.poke();
                    }
                }
            })
            .expect("spawn the watcher");
        Watch { changed }
    }

    fn take(&self) -> bool {
        self.changed.swap(false, Ordering::Relaxed)
    }
}

/// A file the watcher follows: the app's code, and what the manifest adds.
fn watched(path: &Path, include: &[String], exclude: &[String]) -> bool {
    let name = path.to_string_lossy();
    if exclude.iter().any(|g| crate::project::glob(g, &name)) {
        return false;
    }
    name.ends_with(".lua") || name.ends_with(".tl") || include.iter().any(|g| crate::project::glob(g, &name))
}

fn snapshot(dir: &Path, include: &[String], exclude: &[String]) -> Vec<(PathBuf, SystemTime)> {
    let mut out = Vec::new();
    crate::project::walk(dir, &mut |p| {
        if watched(p, include, exclude)
            && let Ok(m) = std::fs::metadata(p).and_then(|m| m.modified())
        {
            out.push((p.to_path_buf(), m));
        }
    });
    out.sort();
    out
}

pub struct Runner {
    lua: Lua,
    shared: Arc<Shared>,
    entry: PathBuf,
    baseline: Baseline,
    watch: Option<Watch>,
    load_error: Option<String>,
    closing: bool,
    /// How many of `shared.errors` the user dismissed.
    dismissed: usize,
    /// `--shot`: the picture to take, after how long, and how far along.
    shot: Option<Shot>,
    /// The devtools' Log tab's rows, and the log's revision they hold.
    log: Option<kui_core::channel::Channel>,
    log_rev: u64,
    /// `kui.devtools("log")`: the tab to choose once declared. The Log
    /// tab to start with, so the panel opens on what the app printed
    /// (ADR 0068, decision 21).
    devtools_tab: Option<String>,
    /// The macOS menu bar the runner gives an app that declares none
    /// (ADR 0068, decision 22): its revision once declared, and whether
    /// the app has since declared its own.
    menu: DefaultMenu,
}

/// The runner's own menu bar, until the app declares one.
#[derive(Default)]
struct DefaultMenu {
    declared_rev: Option<u64>,
    app_owns: bool,
    devtools: bool,
}

/// `kui run --shot out.png`: a frame `after` seconds in, written to `path`,
/// then the run ends. With `--record MS`, every frame drawn for that long
/// from then on, written as an animation (`.gif`, `.png`/`.apng`).
pub struct Shot {
    pub path: PathBuf,
    pub after: f64,
    at: Option<f64>,
    /// Frames since the capture was asked: it is written once drawn.
    since: u32,
    /// `--record`: how long to record, in seconds, and when it stops.
    record: Option<f64>,
    stop_at: Option<f64>,
}

impl Shot {
    pub fn new(path: PathBuf, after_ms: f64) -> Self {
        Shot { path, after: after_ms / 1000.0, at: None, since: 0, record: None, stop_at: None }
    }

    /// Records `ms` of frames instead of taking one.
    pub fn recording(mut self, ms: Option<f64>) -> Self {
        self.record = ms.map(|ms| ms / 1000.0);
        self
    }
}

impl Runner {
    pub fn new(
        lua: Lua,
        shared: Arc<Shared>,
        project: &Project,
        baseline: Baseline,
        watch: Option<Watch>,
        load_error: Option<String>,
        shot: Option<Shot>,
    ) -> Self {
        Runner {
            shot,
            lua,
            shared,
            entry: project.entry.clone(),
            baseline,
            watch,
            load_error,
            closing: false,
            dismissed: 0,
            log: None,
            log_rev: u64::MAX,
            devtools_tab: Some("kui:log".into()),
            menu: DefaultMenu { devtools: project.devtools_reachable, ..Default::default() },
        }
    }

    /// The menu bar an app gets when it declares none, on macOS where the
    /// system owns the bar: the standard menus (ADR 0052) and a View menu
    /// with the devtools. Declared every frame like any bar, until a frame
    /// shows the app declared its own: then it is the app's for good.
    fn default_menu(&mut self, ui: &mut Ui<'_>) {
        if !cfg!(target_os = "macos") || self.menu.app_owns {
            return;
        }
        let core = ui.core();
        if let Some(rev) = self.menu.declared_rev
            && core.menu_bar_revision() != rev
        {
            self.menu.app_owns = true;
            return;
        }
        let mut items = Vec::new();
        if self.menu.devtools {
            items.push(Value::map([
                ("label", "Toggle Developer Tools".into()),
                ("id", "kui:devtools".into()),
                ("accel", "alt+mod+i".into()),
            ]));
        }
        let menus = Value::List(vec![Value::map([
            ("label", "View".into()),
            ("items", Value::List(items)),
        ])]);
        if let Ok(mut bar) = kui_core::MenuBar::from_value(&menus) {
            bar = bar.on_menu(Value::map([("kui", "menu".into())]));
            bar.standard_menus = true;
            core.declare_menu_bar(bar);
            self.menu.declared_rev = Some(core.menu_bar_revision());
        }
    }

    /// Runs the entry again in the same state: the app's own modules
    /// forgotten so they load fresh, its globals — `model` among them —
    /// kept. A load that fails keeps the code that ran before.
    fn reload(&mut self) {
        let result = (|| -> mlua::Result<()> {
            let loaded: mlua::Table = self.lua.globals().get::<mlua::Table>("package")?.get("loaded")?;
            let keep = self.baseline.borrow();
            let mut drop = Vec::new();
            for pair in loaded.pairs::<String, mlua::Value>() {
                let name = pair?.0;
                if !keep.contains(&name) {
                    drop.push(name);
                }
            }
            for name in drop {
                loaded.set(name, mlua::Value::Nil)?;
            }
            let source = std::fs::read_to_string(&self.entry)
                .map_err(|e| mlua::Error::runtime(format!("{}: {e}", self.entry.display())))?;
            let source = crate::teal::source_for(&self.entry, &source)?;
            self.lua
                .load(&source)
                .set_name(format!("@{}", self.entry.display()))
                .exec()?;
            init(&self.lua)
        })();
        match result {
            Ok(()) => {
                if self.load_error.take().is_some() {
                    eprintln!("kui: reloaded");
                }
            }
            Err(e) => {
                eprintln!("kui: {e}");
                self.load_error = Some(e.to_string());
            }
        }
    }

    /// `kui.devtools(on)`, and the panel's Log tab: what the app printed
    /// and logged, and the runner's errors, in Kit's log.
    fn devtools(&mut self, ui: &mut Ui<'_>) {
        if let Some((want, tab)) = self.shared.devtools.lock().unwrap().take() {
            let core = ui.core();
            let shown = core.devtools() && core.devtools_dock() != kui_core::DevtoolsDock::Off;
            core.set_devtools(want.unwrap_or(!shown));
            if let Some(tab) = tab {
                // The runner's own tab by its short name; the panel's by theirs.
                let name = if tab == "log" { "kui:log".to_string() } else { tab };
                self.devtools_tab = Some(name);
            }
        }
        // Hidden (on, docked nowhere, waiting for F12): no tab to fill.
        if !ui.core().devtools() || ui.core().devtools_dock() == kui_core::DevtoolsDock::Off {
            return;
        }
        let rev = crate::modules::log::revision();
        if rev != self.log_rev {
            use kui_core::channel::{Channel, Column};
            let (time, level, message) = crate::modules::log::columns();
            let cols = vec![
                ("time".to_string(), Column::strs(time)),
                ("level".to_string(), Column::strs(level)),
                ("message".to_string(), Column::strs(message)),
            ];
            self.log = match &self.log {
                Some(ch) => ch.publish(cols).ok(),
                None => Channel::table(cols).ok(),
            };
            self.log_rev = rev;
        }
        let rows = self.log.as_ref().map_or(Value::Null, |c| c.handle());
        // A tab asked for is chosen once the panel knows it — the runner's
        // own from the frame after its first declaration.
        if let Some(tab) = &self.devtools_tab
            && ui.core().set_devtools_tab(tab)
        {
            self.devtools_tab = None;
        }
        ui.devtools_tab_with("kui:log", "Log", |ui| {
            ui.with_keyed(
                "kui:log",
                NodeSpec::column().width(kui_core::Sizing::Grow(1.0)).height(kui_core::Sizing::Grow(1.0)),
                |ui| {
                    ui.component(
                        "kit/log",
                        &Value::map([
                            ("rows", rows),
                            ("follow", Value::Bool(true)),
                            ("grow", Value::Bool(true)),
                            ("wrap", Value::Bool(true)),
                        ]),
                    );
                },
            );
        });
    }

    fn overlay(&mut self, ui: &mut Ui<'_>) {
        let errors = self.shared.errors.lock().unwrap();
        let fresh = errors.len().saturating_sub(self.dismissed);
        let text = match (&self.load_error, fresh) {
            (Some(e), _) => format!("The app failed to load; the last code that loaded is running.\n\n{e}"),
            (None, 0) => return,
            (None, n) => {
                let last = errors.last().cloned().unwrap_or_default();
                if n > 1 { format!("{last}\n\n(and {} more, printed to the terminal)", n - 1) } else { last }
            }
        };
        drop(errors);
        let theme = ui.theme();
        // Along the window's foot, over the app.
        let float = kui_core::FloatConfig::viewport().inside(kui_core::Align::Start, kui_core::Align::End);
        ui.with_keyed(
            "kui:errors",
            NodeSpec::column()
                .float(float)
                .width(kui_core::Sizing::Grow(1.0))
                .pad(14.0)
                .gap(8.0)
                .bg(Color::hex(0x2a0f0fee))
                .label("kui error"),
            |ui| {
                ui.text(
                    &text,
                    TextStyle::new(12.5).color(Color::hex(0xffd6d6ff)).mono(),
                );
                if self.load_error.is_none() {
                    ui.leaf(
                        NodeSpec::row()
                            .pad(6.0)
                            .bg(theme.surface)
                            .on_click(Value::map([("kui", Value::str("dismiss"))]))
                            .label("Dismiss"),
                    );
                }
            },
        );
    }
}

impl App for Runner {
    fn setup(&mut self, waker: kui_native::Waker) {
        let waker = Mutex::new(waker);
        self.shared
            .ops
            .set_wake(Box::new(move || waker.lock().unwrap().wake()));
    }

    // A request the app's own server queued, or a reply a task is waiting
    // on, is answered as the wake arrives rather than at the frame it asks
    // for, which waits for the display: a task requesting its own server
    // took two refreshes a round trip.
    fn on_wake(&mut self) {
        if let Err(e) = modules::drain(&self.lua) {
            self.shared.error(e.to_string());
        }
    }

    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.configure_root(NodeSpec::column().fill());
        // Kit's palette before the app's first node: the app's root is
        // lowered before the `kit.settings` inside it runs, so its
        // `$kitBg` would find nothing on the first frame. Kit is the
        // second extension (`main` loads the app first).
        if !ui.core().tokens_declared(kui_core::OriginId(2)) {
            ui.component("kit/settings", &Value::Null);
        }
        if self.watch.as_ref().is_some_and(Watch::take) {
            self.reload();
        }
        if let Err(e) = modules::drain(&self.lua) {
            self.shared.error(e.to_string());
        }
        if let Some(shot) = &mut self.shot
            && let Some(len) = shot.record
        {
            // A recording: started at `after`, stopped `len` later, and the
            // run ends when the file is written (its `capture` event).
            let now = ui.now();
            let at = *shot.at.get_or_insert(now + shot.after);
            match shot.stop_at {
                None if now >= at => {
                    ui.record_to(
                        shot.path.to_string_lossy().into_owned(),
                        Some(Value::map([("kui", Value::str("recorded"))])),
                    );
                    shot.stop_at = Some(now + len);
                    ui.request_frame();
                }
                None => ui.request_frame_at(at),
                Some(stop) if shot.since == 0 && now >= stop => {
                    ui.stop_recording();
                    shot.since = 1;
                    ui.request_frame();
                }
                Some(stop) if shot.since == 0 => ui.request_frame_at(stop),
                Some(_) => {}
            }
        } else if let Some(shot) = &mut self.shot {
            let now = ui.now();
            let at = *shot.at.get_or_insert(now + shot.after);
            if shot.since > 0 {
                shot.since += 1;
                if shot.since > 3 {
                    eprintln!("kui: wrote {}", shot.path.display());
                    self.shared.exit.lock().unwrap().get_or_insert(0);
                } else {
                    ui.request_frame();
                }
            } else if now >= at {
                ui.capture_to(shot.path.to_string_lossy().into_owned(), None);
                shot.since = 1;
                ui.request_frame();
            } else {
                ui.request_frame_at(at);
            }
        }
        if self.shared.exit.lock().unwrap().is_some() && !self.closing {
            self.closing = true;
            ui.window_command(WindowCommand::Close(WindowId::MAIN));
        }
        self.default_menu(ui);
        self.devtools(ui);
        self.overlay(ui);
    }

    fn on_event(&mut self, ev: UiEvent) {
        match ev.payload.get_str("kui") {
            Some("dismiss") => self.dismissed = self.shared.errors.lock().unwrap().len(),
            // The default menu's Toggle Developer Tools: on at the Log tab
            // when hidden, away when shown.
            Some("menu") if ev.data().and_then(|d| d.get_str("item")) == Some("kui:devtools") => {
                *self.shared.devtools.lock().unwrap() = Some((None, None));
                self.shared.ops.poke();
            }
            _ => {}
        }
        if ev.payload.get_str("kui") == Some("recorded")
            && let Some(shot) = &self.shot
        {
            eprintln!("kui: wrote {}", shot.path.display());
            self.shared.exit.lock().unwrap().get_or_insert(0);
        }
    }

    /// The app's `on_exit()` first — it may save, or stop what it started
    /// gently — then whatever it spawned that still runs, then its servers.
    fn teardown(&mut self) {
        if let Ok(f) = self.lua.globals().get::<mlua::Function>("on_exit")
            && let Err(e) = f.call::<()>(())
        {
            eprintln!("kui: on_exit: {e}");
        }
        self.shared.children.kill_all();
        for (_, s) in self.shared.servers.lock().unwrap().drain() {
            drop(s);
        }
    }
}
