//! Presentation and debugger control. The app's logical surface belongs to
//! the test driver; the native window never changes its viewport or time.
use super::{Options, artifact};
use kui_core::{Color, Core, InputEvent, NodeSpec, Value};
use kui_native::{App, Capture, PumpRunner, Ui};
use serde_json::{Value as J, json};
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::{Duration, Instant};

#[derive(Default)]
struct Surface {
    capture: Option<Capture>,
    revision: u64,
    interact: bool,
    input: Vec<InputEvent>,
}
struct Presenter {
    surface: Rc<RefCell<Surface>>,
    image: Option<kui_core::ImageId>,
    revision: u64,
    transform: (f32, f32, f32),
}
impl App for Presenter {
    fn on_input(&mut self, event: &InputEvent) -> bool {
        let mut surface = self.surface.borrow_mut();
        if surface.interact {
            let mut event = event.clone();
            if let InputEvent::CursorMoved(v) = &mut event {
                v.x = (v.x - self.transform.0) / self.transform.2;
                v.y = (v.y - self.transform.1) / self.transform.2;
            }
            surface.input.push(event);
        }
        true
    }
    fn view(&mut self, ui: &mut Ui<'_>) {
        let surface = self.surface.borrow();
        ui.configure_root(
            NodeSpec::column()
                .fill()
                .bg(Color::hex(0x171a20ff))
                .main_align(kui_core::Align::Center)
                .cross_align(kui_core::Align::Center),
        );
        if let Some(capture) = &surface.capture {
            if self.revision != surface.revision || self.image.is_none() {
                if let Some(id) = self.image {
                    ui.core()
                        .update_image(id, capture.width, capture.height, capture.rgba.clone());
                } else {
                    self.image = Some(ui.core().resources.add_image(
                        capture.width,
                        capture.height,
                        capture.rgba.clone(),
                    ));
                }
                self.revision = surface.revision;
            }
            let viewport = ui.core().viewport();
            let logical = (
                capture.width as f32 / capture.scale,
                capture.height as f32 / capture.scale,
            );
            let scale = (viewport.w / logical.0)
                .min(viewport.h / logical.1)
                .max(0.001);
            let (w, h) = (logical.0 * scale, logical.1 * scale);
            self.transform = ((viewport.w - w) / 2.0, (viewport.h - h) / 2.0, scale);
            ui.image(
                self.image.unwrap(),
                NodeSpec::row().size(w, h).label("App under test"),
            );
        }
    }
}
pub struct DebugControl {
    dir: PathBuf,
    headed: bool,
    enabled: bool,
    paused: bool,
    next: bool,
    interact: bool,
    cancelled: bool,
    waiting: bool,
    bootstrapping: bool,
    app_ready: bool,
    failure: bool,
    paused_for: Duration,
    status_last: std::cell::Cell<Option<(bool, bool, bool, bool)>>,
    surface: Rc<RefCell<Surface>>,
    window: Option<PumpRunner<Presenter>>,
    pending: Vec<J>,
    manual: u64,
}
impl DebugControl {
    pub fn new(options: &Options, dir: &Path) -> Self {
        let s = Self {
            dir: dir.to_path_buf(),
            headed: options.headed,
            enabled: options.ui,
            paused: options.debug,
            next: false,
            interact: false,
            cancelled: false,
            waiting: false,
            bootstrapping: options.debug,
            app_ready: false,
            failure: false,
            paused_for: Duration::ZERO,
            status_last: std::cell::Cell::new(None),
            surface: Rc::default(),
            window: None,
            pending: Vec::new(),
            manual: 0,
        };
        let _ = s.status();
        s
    }
    fn status(&self) -> Result<(), String> {
        let state = (
            self.paused && self.waiting,
            self.interact,
            self.app_ready,
            self.failure,
        );
        if self.status_last.get() == Some(state) {
            return Ok(());
        }
        artifact::atomic_json(
            &self.dir.join("status.json"),
            &json!({"paused":state.0,"interact":state.1,"app_ready":state.2,"failed":state.3,"reason":if state.3 {"Test failed. Inspect Errors, state, I/O and Output. Resume to finish this test."} else if state.2 {"App is ready. Resume to run the test, or Step to execute one action."} else {"Paused before an app is available. Resume to continue, or inspect Errors."}}),
        )?;
        self.status_last.set(Some(state));
        Ok(())
    }
    pub fn paused_seconds(&self) -> f64 {
        self.paused_for.as_secs_f64()
    }
    pub fn present(&mut self, core: &mut Core) -> Result<(), String> {
        if !self.headed {
            return Ok(());
        }
        {
            let mut s = self.surface.borrow_mut();
            s.capture = Some(kui_native::capture(core)?);
            s.revision += 1;
        }
        if self.window.is_none() {
            self.window = Some(
                kui_native::app("kui test · app under test")
                    .size(900.0, 650.0)
                    .open(Presenter {
                        surface: self.surface.clone(),
                        image: None,
                        revision: 0,
                        transform: (0.0, 0.0, 1.0),
                    })
                    .map_err(|e| e.to_string())?,
            );
        }
        if let Some(window) = &mut self.window {
            window.waker().wake();
            if !window.pump() {
                return Err("test window closed".into());
            }
        }
        Ok(())
    }
    pub fn poll(&mut self) -> Result<(), String> {
        if let Some(window) = &mut self.window
            && !window.pump()
        {
            return Err("test window closed".into());
        }
        let dir = self.dir.join("commands");
        if let Ok(entries) = std::fs::read_dir(dir) {
            let mut paths = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().is_some_and(|e| e == "json"))
                .collect::<Vec<_>>();
            paths.sort();
            for p in paths {
                let bytes = std::fs::read(&p).map_err(|e| e.to_string())?;
                let command = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
                self.pending.push(command);
                std::fs::remove_file(p).map_err(|e| e.to_string())?;
            }
        }
        Ok(())
    }
    pub fn cancelled(&self) -> bool {
        self.cancelled
    }
    pub fn clear_failure(&mut self) {
        self.failure = false;
    }
    pub fn pause_failure(&mut self) -> Result<(), String> {
        self.failure = true;
        self.pause()
    }
    pub fn pause(&mut self) -> Result<(), String> {
        if !self.enabled {
            return Err("test.pause requires --ui or --debug".into());
        }
        self.paused = true;
        self.status()
    }
    pub fn bootstrapping(&self) -> bool {
        self.bootstrapping
    }
    pub fn app_started(&mut self) {
        self.bootstrapping = false;
    }
    pub fn boundary(&mut self, app: Option<&mut super::session::AppSession>) -> Result<(), String> {
        self.checkpoint(app)
    }
    pub fn after_boundary(
        &mut self,
        app: Option<&mut super::session::AppSession>,
    ) -> Result<(), String> {
        if self.next {
            self.paused = true;
            self.next = false;
        }
        self.checkpoint(app)
    }
    pub fn checkpoint(
        &mut self,
        mut app: Option<&mut super::session::AppSession>,
    ) -> Result<(), String> {
        if self.cancelled {
            return Err("test cancelled in inspector".into());
        }
        self.app_ready = app.is_some();
        loop {
            let start = Instant::now();
            self.waiting = true;
            self.poll()?;
            for command in std::mem::take(&mut self.pending) {
                match command["command"].as_str().unwrap_or("") {
                    "resume" => {
                        self.paused = false;
                        self.interact = false;
                    }
                    "pause" => self.paused = true,
                    "next" => {
                        self.paused = false;
                        self.next = true;
                        self.interact = false;
                    }
                    "interact" => {
                        self.paused = true;
                        self.interact = command["enabled"].as_bool().unwrap_or(!self.interact);
                        if self.interact {
                            self.headed = true;
                            if let Some(app) = app.as_deref_mut() {
                                self.present(app.runner.core_mut())?;
                            }
                        }
                    }
                    "frame" => {
                        if let Some(app) = app.as_deref_mut() {
                            self.manual_action(app, "debug frame", |app| app.frame(1000.0 / 60.0))?;
                        }
                    }
                    "cancel" => {
                        self.cancelled = true;
                        self.paused = false;
                        self.status()?;
                        return Err("test cancelled in inspector".into());
                    }
                    _ => {}
                }
            }
            self.surface.borrow_mut().interact = self.paused && self.interact;
            let input = std::mem::take(&mut self.surface.borrow_mut().input);
            if self.paused
                && self.interact
                && let Some(app) = app.as_deref_mut()
            {
                for event in input {
                    let description = format!("{event:?}");
                    self.manual_action(app, &description, |app| {
                        app.runner.input().send(event);
                        app.frame(0.0)
                    })?;
                }
            }
            self.status()?;
            if !self.paused {
                break;
            }
            std::thread::sleep(Duration::from_millis(10));
            self.paused_for += start.elapsed();
        }
        self.waiting = false;
        self.status()?;
        Ok(())
    }
    fn manual_action(
        &mut self,
        app: &mut super::session::AppSession,
        name: &str,
        action: impl FnOnce(&mut super::session::AppSession) -> Result<(), String>,
    ) -> Result<(), String> {
        self.manual += 1;
        let parent = app.shared.run.step.swap(
            (1u64 << 63) + self.manual,
            std::sync::atomic::Ordering::Relaxed,
        );
        app.shared.run.record(
            "step",
            None,
            json!({"name":name,"phase":"before","parent":parent,"manual":true}),
        );
        let result = (|| {
            app.debug_capture(
                &self.dir,
                &format!("manual-{}-before", self.manual),
                name,
                "before",
            )?;
            app.shared.run.record("manual", None, json!({"input":name}));
            let result = action(app);
            app.debug_capture(
                &self.dir,
                &format!("manual-{}-after", self.manual),
                name,
                "after",
            )?;
            result
        })();
        app.shared.run.record("step",None,json!({"name":name,"phase":"after","parent":parent,"manual":true,"error":result.as_ref().err()}));
        app.shared
            .run
            .step
            .store(parent, std::sync::atomic::Ordering::Relaxed);
        result?;
        self.present(app.runner.core_mut())
    }
}

const FILES: &[(&str, &str)] = &[
    (
        "kui.config.lua",
        include_str!("../../../../tools/kui-test/kui.config.lua"),
    ),
    (
        "main.lua",
        include_str!("../../../../tools/kui-test/main.lua"),
    ),
    (
        "inspector.lua",
        include_str!("../../../../tools/kui-test/inspector.lua"),
    ),
];
fn folder() -> Result<PathBuf, String> {
    let dir = crate::paths::kui_cache()
        .join("test-inspector")
        .join(env!("CARGO_PKG_VERSION"));
    for (name, source) in FILES {
        artifact::atomic(&dir.join(name), source.as_bytes())?;
    }
    Ok(dir)
}
pub fn open_artifact(path: &Path) -> Result<i32, String> {
    open(path, None)
}
pub fn launch(options: Options, selected: Option<PathBuf>, junit: bool) -> Result<i32, String> {
    open(&options.output.clone(), Some((options, selected, junit)))
}
fn spawn_run(
    options: Options,
    selected: Option<PathBuf>,
    junit: bool,
    activity: std::sync::Arc<std::sync::atomic::AtomicBool>,
) -> std::thread::JoinHandle<Result<i32, String>> {
    activity.store(true, std::sync::atomic::Ordering::Release);
    std::thread::spawn(move || {
        struct Done(std::sync::Arc<std::sync::atomic::AtomicBool>);
        impl Drop for Done {
            fn drop(&mut self) {
                self.0.store(false, std::sync::atomic::Ordering::Release);
            }
        }
        let _done = Done(activity);
        let result = super::run(&options, selected.as_deref(), junit);
        if let Err(error) = &result {
            let _ = artifact::atomic_json(
                &options.output.join("run.json"),
                &json!({"version":1,"root":options.root,"status":"error","error":error}),
            );
        }
        result
    })
}
type RunRequest = (Options, Option<PathBuf>, bool);
type SharedLaunch = Rc<RefCell<Option<RunRequest>>>;
type SharedRun = Rc<RefCell<Option<std::thread::JoinHandle<Result<i32, String>>>>>;
fn open(output: &Path, launch: Option<(Options, Option<PathBuf>, bool)>) -> Result<i32, String> {
    let output = std::path::absolute(output).map_err(|e| e.to_string())?;
    let launch = Rc::new(RefCell::new(launch));
    let running: SharedRun = Rc::default();
    let activity = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
    if let Some((options, selected, junit)) = launch.borrow().clone() {
        let handle = spawn_run(options, selected, junit, activity.clone());
        *running.borrow_mut() = Some(handle);
    }
    // The inspector is a normal runner app. The UI bridge is installed by
    // modules::install when this thread-local request is present.
    BRIDGE.with(|b| {
        *b.borrow_mut() = Some(Bridge {
            output: output.clone(),
            activity,
            launch,
            running,
        })
    });
    let mut argv = vec![
        "run".into(),
        folder()?.to_string_lossy().into_owned(),
        "--no-rename".into(),
    ];
    // Integration checks can render the real inspector without leaving a window.
    if let Some(path) = std::env::var_os("KUI_TEST_INSPECTOR_SHOT") {
        if let Some(size) = std::env::var_os("KUI_TEST_INSPECTOR_SIZE") {
            argv.extend(["--size".into(), size.to_string_lossy().into_owned()]);
        }
        argv.extend([
            "--shot".into(),
            path.to_string_lossy().into_owned(),
            "--after".into(),
            "1000".into(),
        ]);
    }
    argv.extend(["--".into(), output.to_string_lossy().into_owned()]);
    let result = crate::run(&crate::parse(argv)?, None);
    BRIDGE.with(|b| {
        if let Some(bridge) = b.borrow_mut().take()
            && let Some(handle) = bridge.running.borrow_mut().take()
        {
            if !handle.is_finished() {
                let _ = std::fs::write(output.join("cancel"), b"cancel");
            }
            let _ = handle.join();
        }
    });
    result
}
#[derive(Clone)]
struct Bridge {
    output: PathBuf,
    activity: std::sync::Arc<std::sync::atomic::AtomicBool>,
    launch: SharedLaunch,
    running: SharedRun,
}
thread_local! {static BRIDGE:RefCell<Option<Bridge>>=const{RefCell::new(None)};}
pub fn install(lua: &mlua::Lua, kui: &mlua::Table) -> mlua::Result<()> {
    let bridge = BRIDGE.with(|b| b.borrow().clone());
    let Some(bridge) = bridge else {
        return Ok(());
    };
    if crate::modules::shared(lua).run.testing {
        return Ok(());
    }
    let shared = crate::modules::shared(lua);
    let weak = std::sync::Arc::downgrade(&shared.run);
    let ops = shared.ops.clone();
    let output = bridge.output.clone();
    let activity = bridge.activity.clone();
    let watch = bridge
        .launch
        .borrow()
        .as_ref()
        .map(|v| (v.0.root.clone(), v.0.watch));
    let thread = std::thread::spawn(move || {
        let mut last = output_stamp(&output);
        let mut last_busy = activity.load(std::sync::atomic::Ordering::Acquire);
        let mut sources = watch
            .as_ref()
            .filter(|v| v.1)
            .map(|v| super::source_stamp(&v.0, &output));
        loop {
            std::thread::sleep(Duration::from_millis(150));
            if weak
                .upgrade()
                .is_none_or(|r| r.closed.load(std::sync::atomic::Ordering::Relaxed))
            {
                break;
            }
            let current = output_stamp(&output);
            let current_busy = activity.load(std::sync::atomic::Ordering::Acquire);
            if current != last || current_busy != last_busy {
                last_busy = current_busy;
                last = current;
                ops.send(crate::ops::Inbound::Dispatch {
                    from: None,
                    msg: Value::map([("kind", Value::str("refresh"))]),
                });
            }
            if let (Some(old), Some((root, _))) = (&mut sources, &watch) {
                let current = super::source_stamp(root, &output);
                if *old != current {
                    *old = current;
                    ops.send(crate::ops::Inbound::Dispatch {
                        from: None,
                        msg: Value::map([("kind", Value::str("source_changed"))]),
                    });
                }
            }
        }
    });
    shared.run.background.lock().unwrap().push(thread);
    let module = lua.create_table()?;
    let ops = shared.ops.clone();
    module.set(
        "path_action",
        lua.create_function(
            move |_, (action, path, line): (String, String, Option<u32>)| {
                let command = super::paths::command(&action, Path::new(&path), line.unwrap_or(1))
                    .map_err(mlua::Error::runtime)?;
                let ops = ops.clone();
                super::paths::spawn(command, move |result| {
                    if let Err(error) = result {
                        ops.send(crate::ops::Inbound::Dispatch {
                            from: None,
                            msg: Value::map([
                                ("kind", Value::str("path_error")),
                                ("error", Value::str(error)),
                            ]),
                        });
                    }
                })
                .map_err(mlua::Error::runtime)
            },
        )?,
    )?;
    if std::env::var_os("KUI_TEST_INSPECTOR_SHOT").is_some() {
        module.set("initial_tab", std::env::var("KUI_TEST_INSPECTOR_TAB").ok())?;
    }

    let b = bridge.clone();
    module.set(
        "read",
        lua.create_function(move |lua, ()| {
            let mut value = read_run(&b.output).map_err(mlua::Error::runtime)?;
            let busy = b.activity.load(std::sync::atomic::Ordering::Acquire);
            value["busy"] = json!(busy);
            value["can_run"] = json!(b.launch.borrow().is_some() && !busy);
            kui_lua::value_to_lua(
                lua,
                &kui_core::json::parse(&value.to_string())
                    .map_err(|e| mlua::Error::runtime(e.to_string()))?,
            )
        })?,
    )?;
    let b = bridge.clone();
    module.set(
        "detail",
        lua.create_function(move |lua, (file, frame): (String, String)| {
            let dir = artifact::safe_join(&b.output, &file).map_err(mlua::Error::runtime)?;
            let path = artifact::safe_join(&dir, &frame).map_err(mlua::Error::runtime)?;
            let mut value = artifact::read_frame(&path).map_err(mlua::Error::runtime)?;
            if value["shot"].is_null()
                && let Some(drawing) = value["drawing"].as_str()
            {
                let capture =
                    super::drawing::render(&dir, drawing).map_err(mlua::Error::runtime)?;
                let shot = artifact::image(
                    &dir,
                    &artifact::png(&capture).map_err(mlua::Error::runtime)?,
                )
                .map_err(mlua::Error::runtime)?;
                value["shot"] = json!(shot);
                value["reconstructed"] = json!(true);
            }
            value["path"] = json!(frame);
            value["file"] = json!(file);
            value["state_view"] = artifact::app_state_view(&value);
            value["kit_view"] = artifact::graph_view(&value["kit"]);
            if let Some(shot) = value["shot"].as_str() {
                value["shot_path"] = json!(
                    artifact::safe_join(&dir, shot)
                        .map_err(mlua::Error::runtime)?
                        .to_string_lossy()
                );
            }
            kui_lua::value_to_lua(
                lua,
                &kui_core::json::parse(&value.to_string())
                    .map_err(|e| mlua::Error::runtime(e.to_string()))?,
            )
        })?,
    )?;
    let b = bridge.clone();
    module.set("output",lua.create_function(move|lua,file:String| {
        let dir=artifact::safe_join(&b.output,&file).map_err(mlua::Error::runtime)?;
        let path=dir.join("output.log");let bytes=std::fs::read(&path).unwrap_or_default();let start=bytes.len().saturating_sub(128*1024);
        let value=json!({"path":path,"text":String::from_utf8_lossy(&bytes[start..]),"truncated":start>0,"bytes":bytes.len()});
        kui_lua::value_to_lua(lua,&kui_core::json::parse(&value.to_string()).map_err(|e|mlua::Error::runtime(e.to_string()))?)
    })?)?;
    let b = bridge.clone();
    module.set(
        "changes",
        lua.create_function(
            move |lua, (file, before, after, field): (String, String, String, String)| {
                let dir = artifact::safe_join(&b.output, &file).map_err(mlua::Error::runtime)?;
                let read = |name: &str| -> Result<J, String> {
                    let v = artifact::read_frame(&artifact::safe_join(&dir, name)?)?;
                    Ok(if field == "state" {
                        artifact::app_state_view(&v)
                    } else if field == "kit" {
                        artifact::graph_view(&v["kit"])
                    } else {
                        v[field.as_str()].clone()
                    })
                };
                let result = artifact::changes(
                    &read(&before).map_err(mlua::Error::runtime)?,
                    &read(&after).map_err(mlua::Error::runtime)?,
                );
                kui_lua::value_to_lua(
                    lua,
                    &kui_core::json::parse(&result.to_string())
                        .map_err(|e| mlua::Error::runtime(e.to_string()))?,
                )
            },
        )?,
    )?;
    let b = bridge.clone();
    module.set(
        "perf",
        lua.create_function(move |_, (file, frame): (String, Option<String>)| {
            let dir = artifact::safe_join(&b.output, &file).map_err(mlua::Error::runtime)?;
            let path = if let Some(frame) = frame {
                let frame = artifact::read_frame(
                    &artifact::safe_join(&dir, &frame).map_err(mlua::Error::runtime)?,
                )
                .map_err(mlua::Error::runtime)?;
                artifact::safe_join(
                    &dir,
                    frame["perf"].as_str().ok_or_else(|| {
                        mlua::Error::runtime("run tests with --perf to record performance")
                    })?,
                )
                .map_err(mlua::Error::runtime)?
            } else {
                std::fs::read_dir(&dir)
                    .map_err(mlua::Error::external)?
                    .flatten()
                    .map(|e| e.path())
                    .find(|p| p.extension().is_some_and(|e| e == "kuitrace"))
                    .ok_or_else(|| {
                        mlua::Error::runtime("run tests with --perf to record performance")
                    })?
            };
            if !path.is_file() {
                return Err(mlua::Error::runtime(
                    "run tests with --perf to record performance",
                ));
            }
            std::process::Command::new(std::env::current_exe().map_err(mlua::Error::external)?)
                .arg("perf")
                .arg(path)
                .spawn()
                .map_err(mlua::Error::external)?;
            Ok(())
        })?,
    )?;
    let b = bridge.clone();
    module.set(
        "snapshot",
        lua.create_function(move |lua, (file, index): (String, usize)| {
            let dir = artifact::safe_join(&b.output, &file).map_err(mlua::Error::runtime)?;
            let events =
                std::fs::read_to_string(dir.join("events.jsonl")).map_err(mlua::Error::external)?;
            let mut snapshot = events
                .lines()
                .filter_map(|l| serde_json::from_str::<J>(l).ok())
                .filter(|e| e["kind"] == "snapshot")
                .nth(index.saturating_sub(1))
                .ok_or_else(|| mlua::Error::runtime("snapshot no longer exists"))?["data"]
                .clone();
            for variant in ["expected", "actual", "diff"] {
                if let Some(path) = snapshot[variant].as_str() {
                    let path = artifact::safe_join(&dir, path).map_err(mlua::Error::runtime)?;
                    snapshot[format!("{variant}_path")] = json!(path);
                    if snapshot["kind"] != "expect_shot" {
                        snapshot[format!("{variant}_text")] =
                            json!(std::fs::read_to_string(path).unwrap_or_default());
                    }
                }
            }
            kui_lua::value_to_lua(
                lua,
                &kui_core::json::parse(&snapshot.to_string())
                    .map_err(|e| mlua::Error::runtime(e.to_string()))?,
            )
        })?,
    )?;
    let b = bridge.clone();
    module.set(
        "command",
        lua.create_function(
            move |_, (file, command, enabled): (String, String, Option<bool>)| {
                let dir = artifact::safe_join(&b.output, &file).map_err(mlua::Error::runtime)?;
                static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                artifact::atomic_json(
                    &dir.join("commands").join(format!("{n:020}.json")),
                    &json!({"command":command,"enabled":enabled}),
                )
                .map_err(mlua::Error::runtime)
            },
        )?,
    )?;
    let b = bridge.clone();
    module.set(
        "rerun",
        lua.create_function(
            move |_,
                  (filter, headed, debug, update, file): (
                Option<String>,
                bool,
                bool,
                bool,
                Option<String>,
            )| {
                let mut running = b.running.borrow_mut();
                if running.as_ref().is_some_and(|h| !h.is_finished()) {
                    return Ok(false);
                }
                if let Some(handle) = running.take() {
                    let _ = handle.join();
                }
                let Some((mut options, mut selected, junit)) = b.launch.borrow().clone() else {
                    return Err(mlua::Error::runtime(
                        "open this app's tests to rerun; this is a recorded artifact",
                    ));
                };
                options.exact = file.is_some() && filter.is_some();
                if let Some(file) = file {
                    selected = Some(PathBuf::from(file));
                }
                options.filter = filter;
                options.headed = headed || debug;
                options.debug = debug;
                options.update = update;
                artifact::atomic_json(&options.output.join("options.json"), &options)
                    .map_err(mlua::Error::runtime)?;
                *running = Some(spawn_run(options, selected, junit, b.activity.clone()));
                Ok(true)
            },
        )?,
    )?;
    let b = bridge.clone();
    module.set(
        "drawing",
        lua.create_function(move |_, (file, frame): (String, String)| {
            let dir = artifact::safe_join(&b.output, &file).map_err(mlua::Error::runtime)?;
            let bytes =
                std::fs::read(artifact::safe_join(&dir, &frame).map_err(mlua::Error::runtime)?)
                    .map_err(mlua::Error::external)?;
            let frame: J = serde_json::from_slice(&bytes).map_err(mlua::Error::external)?;
            let drawing = frame["drawing"]
                .as_str()
                .ok_or_else(|| mlua::Error::runtime("frame has no drawing packet"))?;
            let capture = super::drawing::render(&dir, drawing).map_err(mlua::Error::runtime)?;
            let path = dir.join("playback.png");
            artifact::atomic(
                &path,
                &artifact::png(&capture).map_err(mlua::Error::runtime)?,
            )
            .map_err(mlua::Error::runtime)?;
            Ok(path.to_string_lossy().into_owned())
        })?,
    )?;
    let b = bridge.clone();
    module.set(
        "render_state",
        lua.create_function(move |_, (file, frame): (String, String)| {
            let dir = artifact::safe_join(&b.output, &file).map_err(mlua::Error::runtime)?;
            let path = artifact::safe_join(&dir, &frame).map_err(mlua::Error::runtime)?;
            let options: Options = serde_json::from_slice(
                &std::fs::read(b.output.join("options.json")).map_err(mlua::Error::external)?,
            )
            .map_err(mlua::Error::external)?;
            let capture =
                super::session::render_state(&options.root, &path).map_err(mlua::Error::runtime)?;
            let target = dir.join("state-render.png");
            artifact::atomic(
                &target,
                &artifact::png(&capture).map_err(mlua::Error::runtime)?,
            )
            .map_err(mlua::Error::runtime)?;
            Ok(target.to_string_lossy().into_owned())
        })?,
    )?;
    kui.set("test_inspector", module)
}
fn read_run(output: &Path) -> Result<J, String> {
    let read = |p: PathBuf| {
        std::fs::read(p)
            .ok()
            .and_then(|b| serde_json::from_slice::<J>(&b).ok())
            .unwrap_or(J::Null)
    };
    let mut files = Vec::new();
    let dirs = artifact::file_dirs(output)?;
    for (index, dir) in dirs.into_iter().enumerate() {
        let (collection, result) = super::report::file_results(output, &dir, index);
        let events = std::fs::read_to_string(dir.join("events.jsonl"))
            .unwrap_or_default()
            .lines()
            .filter_map(|l| serde_json::from_str::<J>(l).ok())
            .map(|mut e| {
                if e["data"]["version"] == 1 {
                    e["data"] = artifact::graph_view(&e["data"]);
                }
                if e["data"]["args"]["version"] == 1 {
                    e["data"]["args"] = artifact::graph_view(&e["data"]["args"]);
                }
                e
            })
            .collect::<Vec<_>>();
        let mut frames = events
            .iter()
            .filter(|e| e["kind"] == "frame")
            .filter_map(|e| {
                let path = e["data"]["path"].as_str()?;
                let p = artifact::safe_join(&dir, path).ok()?;
                let mut frame = read(p);
                frame["path"] = json!(path);
                frame["file"] = json!(dir.strip_prefix(output).ok()?.to_string_lossy());
                if let Some(shot) = frame["shot"].as_str() {
                    frame["shot_path"] =
                        json!(artifact::safe_join(&dir, shot).ok()?.to_string_lossy());
                }
                if let Some(map) = frame.as_object_mut() {
                    for field in ["state", "tree", "core", "kit"] {
                        map.remove(field);
                    }
                }
                Some(frame)
            })
            .collect::<Vec<_>>();
        annotate_frames(&mut frames, &events);
        files.push(json!({"name":dir.strip_prefix(output).unwrap_or(&dir).to_string_lossy(),"status":read(dir.join("status.json")),"results":result,"collection":collection,"frames":frames,"diagnostics":super::report::diagnostics(&events,&result),"output_path":dir.join("output.log"),"events":events}));
    }
    Ok(
        json!({"artifact_path":output,"options":read(output.join("options.json")),"run":read(output.join("run.json")),"results":read(output.join("results.json")),"files":files}),
    )
}

fn annotate_frames(frames: &mut [J], events: &[J]) {
    let mut steps = std::collections::HashMap::<String, J>::new();
    let mut snapshot_index = 0;
    for event in events {
        let key = format!("{}:{}", event["case"], event["step"]);
        let meta = steps.entry(key).or_insert_with(|| json!({}));
        let data = &event["data"];
        match event["kind"].as_str().unwrap_or("") {
            "step" if data["phase"] == "before" => {
                if let Some(source) = data["source"].as_array() {
                    meta["source"] = json!({"file":source.first(),"line":source.get(1)});
                }
            }
            "target" => meta["target"] = data.clone(),
            "snapshot" => {
                snapshot_index += 1;
                meta["expectation"] = data.clone();
                meta["snapshot_index"] = json!(snapshot_index);
            }
            "assertion" => {
                meta["assertion"] = data.clone();
                if data["ok"] == false && data["source"].is_object() {
                    meta["source"] = data["source"].clone();
                }
            }
            _ => {}
        }
    }
    for frame in frames {
        if let Some(meta) = steps.get(&format!("{}:{}", frame["case"], frame["step_id"])) {
            for (field, value) in meta.as_object().unwrap() {
                frame[field] = value.clone();
            }
        }
    }
}

/// Native image decoding and picking for the Kit inspector's frame component.
/// The component owns its image handles; Lua receives immutable metadata.
pub struct Media {
    components: Vec<String>,
    images: std::collections::HashMap<String, (kui_core::ImageId, u32, u32, std::time::SystemTime)>,
    bounds: std::collections::HashMap<kui_core::Key, (kui_core::Rect, f32, f32)>,
}
pub fn media() -> Option<Media> {
    BRIDGE.with(|b| {
        b.borrow().as_ref().map(|_| Media {
            components: vec!["image".into()],
            images: Default::default(),
            bounds: Default::default(),
        })
    })
}
impl kui_core::Extension for Media {
    fn name(&self) -> &str {
        "testmedia"
    }
    fn components(&self) -> &[String] {
        &self.components
    }
    fn view(&mut self, inst: &kui_core::Instance<'_>, ui: &mut Ui<'_>) -> Result<(), String> {
        let Some(path) = inst.params.get_str("path") else {
            return Ok(());
        };
        let width = inst.params.get_f32("width").unwrap_or(480.0);
        let height = inst.params.get_f32("height").unwrap_or(280.0);
        let modified = std::fs::metadata(path)
            .and_then(|m| m.modified())
            .map_err(|e| format!("{path}: {e}"))?;
        let highlight = inst.params.get("highlight");
        let target = inst.params.get("target");
        let expectation = inst.params.get("expectation");
        let cache_key = format!("{path}:{highlight:?}:{target:?}:{expectation:?}");
        if self.images.get(&cache_key).is_none_or(|v| v.3 != modified) {
            let bytes = std::fs::read(path).map_err(|e| e.to_string())?;
            let mut pixels = kui_native::decode_image(&bytes)?;
            paint_markers(&mut pixels, inst.params, highlight, target, expectation);
            let id = ui
                .core()
                .resources
                .add_image(pixels.width, pixels.height, pixels.rgba);
            if let Some((old, ..)) = self.images.insert(
                cache_key.clone(),
                (id, pixels.width, pixels.height, modified),
            ) {
                ui.core().remove_image(old);
            }
            if self.images.len() > 40
                && let Some(key) = self.images.keys().find(|k| **k != cache_key).cloned()
                && let Some((old, ..)) = self.images.remove(&key)
            {
                ui.core().remove_image(old);
            }
        }
        let (id, w, h, _) = self.images[&cache_key];
        let scale = (width / w as f32).min(height / h as f32);
        let (draw_w, draw_h) = (w as f32 * scale, h as f32 * scale);
        let mut key = None;
        ui.with_keyed(
            "preview",
            NodeSpec::column()
                .size(width, height)
                .bg(Color::hex(0x111419ff))
                .center(),
            |ui| {
                key = Some(
                    ui.with_keyed(
                        "pixels",
                        NodeSpec::column()
                            .size(draw_w, draw_h)
                            .label("Pick a recorded node")
                            .on_button(Value::str("pick")),
                        |ui| {
                            ui.image(
                                id,
                                NodeSpec::row()
                                    .size(draw_w, draw_h)
                                    .label("Recorded app frame"),
                            );
                        },
                    ),
                );
            },
        );
        let nodes = ui.core().nodes();
        if let Some(node) = nodes.iter().find(|n| Some(n.key) == key) {
            self.bounds.insert(
                node.key,
                (
                    node.rect,
                    inst.params.get_f32("logical_width").unwrap_or(w as f32) / draw_w,
                    inst.params.get_f32("logical_height").unwrap_or(h as f32) / draw_h,
                ),
            );
        }
        Ok(())
    }
    fn on_event(&mut self, event: &kui_core::UiEvent) -> Vec<Value> {
        let Some(data) = event.data() else {
            return Vec::new();
        };
        if data.get_str("phase") != Some("up") {
            return Vec::new();
        }
        let Some((rect, sx, sy)) = self.bounds.get(&event.key) else {
            return Vec::new();
        };
        let (Some(x), Some(y)) = (data.get_f32("x"), data.get_f32("y")) else {
            return Vec::new();
        };
        vec![kui_core::slot::emit(
            "on_pick",
            Value::map([(
                "pick",
                Value::map([
                    ("x", Value::float((x - rect.x) * sx)),
                    ("y", Value::float((y - rect.y) * sy)),
                ]),
            )]),
        )]
    }
}

fn paint_markers(
    pixels: &mut kui_native::Pixels,
    params: &Value,
    highlight: Option<&Value>,
    target: Option<&Value>,
    expectation: Option<&Value>,
) {
    if highlight.is_none() && target.is_none() && expectation.is_none() {
        return;
    }
    let sx = pixels.width as f32
        / params
            .get_f32("logical_width")
            .unwrap_or(pixels.width as f32)
            .max(1.0);
    let sy = pixels.height as f32
        / params
            .get_f32("logical_height")
            .unwrap_or(pixels.height as f32)
            .max(1.0);
    let rect = highlight.or_else(|| target.and_then(|t| t.get("rect")));
    let point = target.and_then(|t| t.get("point"));
    let border = expectation.map(|e| {
        if e.get_bool("ok") == Some(true) {
            [55, 210, 130, 255]
        } else {
            [255, 90, 90, 255]
        }
    });
    for py in 0..pixels.height {
        for px in 0..pixels.width {
            let (x, y) = (px as f32 / sx, py as f32 / sy);
            let mut color = None;
            if let Some(r) = rect {
                let (rx, ry, rw, rh) = (
                    r.get_f32("x").unwrap_or(0.0),
                    r.get_f32("y").unwrap_or(0.0),
                    r.get_f32("w").unwrap_or(0.0),
                    r.get_f32("h").unwrap_or(0.0),
                );
                if x >= rx
                    && x < rx + rw
                    && y >= ry
                    && y < ry + rh
                    && (x < rx + 2.0 || x >= rx + rw - 2.0 || y < ry + 2.0 || y >= ry + rh - 2.0)
                {
                    color = Some(if highlight.is_some() {
                        [255, 212, 0, 255]
                    } else {
                        [90, 180, 255, 255]
                    });
                }
            }
            if let Some(p) = point {
                let dx = x - p.get_f32("x").unwrap_or(0.0);
                let dy = y - p.get_f32("y").unwrap_or(0.0);
                let distance = dx * dx + dy * dy;
                if (64.0..=100.0).contains(&distance)
                    || (dx.abs() < 1.0 && dy.abs() < 5.0)
                    || (dy.abs() < 1.0 && dx.abs() < 5.0)
                {
                    color = Some([90, 180, 255, 255]);
                }
            }
            if let Some(border) = border
                && (px < 3
                    || py < 3
                    || px >= pixels.width.saturating_sub(3)
                    || py >= pixels.height.saturating_sub(3))
            {
                color = Some(border);
            }
            if let Some(color) = color {
                let at = (py as usize * pixels.width as usize + px as usize) * 4;
                pixels.rgba[at..at + 4].copy_from_slice(&color);
            }
        }
    }
}

fn output_stamp(output: &Path) -> Vec<(PathBuf, u64, std::time::SystemTime)> {
    let mut paths = vec![output.join("run.json"), output.join("results.json")];
    if let Ok(dirs) = artifact::file_dirs(output) {
        for dir in dirs {
            for name in [
                "events.jsonl",
                "results.json",
                "status.json",
                "collection.json",
                "output.log",
            ] {
                paths.push(dir.join(name));
            }
        }
    }
    let mut stamp = paths
        .into_iter()
        .filter_map(|p| {
            let m = p.metadata().ok()?;
            Some((p, m.len(), m.modified().ok()?))
        })
        .collect::<Vec<_>>();
    stamp.sort();
    stamp
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn action_annotations_join_by_case_and_step_including_failed_comparisons() {
        let mut frames = vec![
            json!({"case":"one","step_id":2}),
            json!({"case":"two","step_id":2}),
        ];
        let events = vec![
            json!({"case":"one","step":2,"kind":"step","data":{"phase":"before","source":["/app/test.lua",12]}}),
            json!({"case":"one","step":2,"kind":"target","data":{"action":"click","label":"Save","point":{"x":40,"y":50}}}),
            json!({"case":"one","step":2,"kind":"snapshot","data":{"kind":"expect_shot","name":"saved","ok":false}}),
            json!({"case":"two","step":2,"kind":"assertion","data":{"ok":false,"source":{"file":"/app/helper.lua","line":21}}}),
        ];
        annotate_frames(&mut frames, &events);
        assert_eq!(frames[0]["target"]["label"], "Save");
        assert_eq!(frames[0]["source"]["line"], 12);
        assert_eq!(frames[0]["expectation"]["ok"], false);
        assert_eq!(frames[0]["snapshot_index"], 1);
        assert!(frames[1]["target"].is_null());
        assert_eq!(frames[1]["source"]["file"], "/app/helper.lua");
    }
    #[test]
    fn recorded_targets_scale_with_screenshot_pixels_and_preserve_unmarked_content() {
        let mut pixels = kui_native::Pixels {
            width: 200,
            height: 160,
            rgba: vec![0; 200 * 160 * 4],
        };
        let value = |s: &str| kui_core::json::parse(s).unwrap();
        let params = value(r#"{"logical_width":100,"logical_height":80}"#);
        let target = value(r#"{"rect":{"x":10,"y":20,"w":20,"h":20},"point":{"x":20,"y":30}}"#);
        paint_markers(&mut pixels, &params, None, Some(&target), None);
        let pixel = |p: &kui_native::Pixels, x: usize, y: usize| {
            p.rgba[(y * 200 + x) * 4..(y * 200 + x) * 4 + 4].to_vec()
        };
        assert_eq!(pixel(&pixels, 20, 40), [90, 180, 255, 255]);
        assert_eq!(pixel(&pixels, 40, 60), [90, 180, 255, 255]);
        assert_eq!(pixel(&pixels, 160, 100), [0, 0, 0, 0]);
        let expectation = value(r#"{"ok":false}"#);
        paint_markers(&mut pixels, &params, None, None, Some(&expectation));
        assert_eq!(pixel(&pixels, 0, 0), [255, 90, 90, 255]);
    }
    #[test]
    fn file_controls_copy_literal_paths_and_report_launcher_errors() {
        inspector(
            r#"
local I=assert(load(inspector_source))();local m=I.new()
local path='/tmp/a b/test.lua'
I.update(m,{kind='copy_path',path=path});assert(m.clipboard==path)
kui.test_inspector.path_action=function(action,file,line)assert(action=='editor' and file==path and line==21);error('editor not found')end
I.update(m,{kind='path_action',action='editor',path=path,line=21});assert(m.error:find('editor not found',1,true))
I.update(m,{kind='path_error',error='editor exited 1'});assert(m.error=='editor exited 1')
kui.test_inspector.snapshot=function()return {name='visual'}end
I.update(m,{kind='snapshot',file='file',index=1});assert(m.tab=='assertions' and m.snapshot.name=='visual')
I.update(m,{kind='markers'});assert(m.hide_markers)
"#,
        );
    }
    #[test]
    fn completed_failure_selects_the_failed_comparison_instead_of_the_final_frame() {
        inspector(
            r#"
local frames={{file='file',path='start',step='start',step_id=1,phase='after'},{file='file',path='compare',step='expect_shot',step_id=2,phase='before',expectation={name='checkout',ok=false}},{file='file',path='end',step='case end',step_id=0,phase='after'}}
kui.test_inspector.read=function()return {run={id='failed',status='failed'},files={{name='file',frames=frames}}}end
kui.test_inspector.detail=function()return {tree={}}end
local I=assert(load(inspector_source))();local m=I.new();I.update(m,{kind='refresh'});assert(m.selected==2)
I.update(m,{kind='latest'});assert(m.selected==3)
"#,
        );
    }
    fn inspector(script: &str) {
        let lua = mlua::Lua::new();
        lua.load("package.preload['kit.ui']=function()return {}end;kui={test_inspector={}};kui.test_inspector.changes=function()return {}end").exec().unwrap();
        lua.globals()
            .set(
                "inspector_source",
                include_str!("../../../../tools/kui-test/inspector.lua"),
            )
            .unwrap();
        lua.load(script).exec().unwrap();
    }
    #[test]
    fn reruns_do_not_submit_while_busy_or_discard_the_selected_frame() {
        inspector(
            r#"
local calls=0
local data={can_run=true,busy=true,run={status='running'},files={}}
kui.test_inspector.read=function()return data end
kui.test_inspector.rerun=function()calls=calls+1;error('a test run is already active')end
local I=assert(load(inspector_source))();local m=I.new();I.update(m,{kind='refresh'})
m.detail={shot_path='selected.png'};I.update(m,{kind='run_all'})
assert(calls==0,'busy run button submitted another run')
assert(m.detail and m.detail.shot_path=='selected.png','busy click discarded selected history')
assert(not m.error,'busy click displayed an exception')
"#,
        );
    }
    #[test]
    fn reruns_refresh_reused_frame_names_instead_of_retaining_stale_image_paths() {
        inspector(
            r#"
local data={run={id='old',status='complete'},files={{name='file-0000',frames={{file='file-0000',path='frames/1.json',case='case',step='start',phase='after',step_id=1}}}}}
local image='old.png'
kui.test_inspector.read=function()return data end
kui.test_inspector.detail=function()return {shot_path=image,tree={}}end
local I=assert(load(inspector_source))();local m=I.new();I.update(m,{kind='refresh'})
assert(m.detail.shot_path=='old.png')
data.run.id='new';image='new.png';I.update(m,{kind='refresh'})
assert(m.detail.shot_path=='new.png','rerun retained a stale image path')
"#,
        );
    }
}
