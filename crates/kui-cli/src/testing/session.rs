use super::{CaseResult, Options, Results, artifact};
use crate::{context::RunContext, host, kit, modules, project};
use kui_core::{AccessAction, AccessRequest, Core, InputEvent, Key, Value, Vec2};
use kui_native::HeadlessRunner;
use mlua::{FromLua, Lua, MultiValue, Table};
use serde_json::json;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

const SNAPSHOT: &str = include_str!("../lua/snapshot.lua");
fn error(e: impl ToString) -> mlua::Error {
    mlua::Error::runtime(e.to_string())
}

pub struct AppSession {
    pub runner: HeadlessRunner<host::Runner>,
    pub lua: Lua,
    pub kit: Lua,
    pub shared: Arc<modules::Shared>,
    recording: Option<(PathBuf, usize, String, bool)>,
    serial: u64,
    errors_seen: usize,
    warnings: Vec<String>,
    tree_cache: Option<(Vec<kui_core::NodeInfo>, String)>,
    model_cache: Option<(u64, String)>,
    perf: Option<String>,
}
impl AppSession {
    fn open(
        project: &project::Project,
        run: Arc<RunContext>,
        system_fonts: bool,
        size: Option<(f32, f32)>,
    ) -> Result<Self, String> {
        let shared = modules::Shared::in_run(project.dir.clone(), None, run);
        let (ext, baseline, load_error) = host::load_app(project, shared.clone(), false)?;
        let lua = ext.lua().clone();
        let kit = kit::extension().map_err(|e| e.to_string())?;
        let kit_lua = kit.lua().clone();
        let utilities = kit_lua.create_table().map_err(|e| e.to_string())?;
        modules::formats::install(&kit_lua, &utilities).map_err(|e| e.to_string())?;
        modules::crypto::install(&kit_lua, &utilities).map_err(|e| e.to_string())?;
        crate::snapshot::install(&kit_lua, &utilities).map_err(|e| e.to_string())?;
        kit_lua
            .globals()
            .set("kui", utilities)
            .map_err(|e| e.to_string())?;
        kit_lua.load("for _,n in ipairs({'encode','decode'}) do local f=kui.json['try_'..n];kui.json[n]=function(...) local ok,v=f(...);if not ok then error(v,2) end;return v end end").exec().map_err(|e|e.to_string())?;
        kui_lua::set_script_dir(kit.lua(), &project.dir).map_err(|e| e.to_string())?;
        let host = host::Runner::new(
            lua.clone(),
            shared.clone(),
            project,
            baseline,
            None,
            load_error,
            None,
        );
        let core = if system_fonts {
            Core::new()
        } else {
            let session = kui_core::Session::with_fonts(
                "en-US",
                [
                    include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec(),
                    include_bytes!("../../assets/fonts/NotoSans-Bold.ttf").to_vec(),
                    include_bytes!("../../assets/fonts/NotoSansMono-Regular.ttf").to_vec(),
                ],
            );
            Core::new_in(&session)
        };
        let (w, h) = size.unwrap_or((project.size.0 as f32, project.size.1 as f32));
        if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
            return Err("test.start size needs positive finite dimensions".into());
        }
        let mut runner = kui_native::app(&project.name)
            .core(core)
            .chrome(project.chrome)
            .titlebar(project.titlebar)
            .platform(kui_native::Platform::Linux)
            .system(kui_native::SystemEnv {
                appearance: kui_native::Appearance::Light,
                locale: kui_core::Locale::new("en-US"),
                ..Default::default()
            })
            .extension_as("app", ext)
            .extension_as(kit::NAMESPACE, kit)
            .setup_core(|core| core.set_inspect(true))
            .open_headless(host, w, h, 1.0);
        modules::input::set(&shared, runner.input());
        for _ in 0..3 {
            shared.run.clock.advance(0.0)?;
            runner.frame_at(shared.run.clock.virtual_time().unwrap() / 1000.0)?;
        }
        let mut session = Self {
            runner,
            lua,
            kit: kit_lua,
            shared,
            recording: None,
            serial: 0,
            errors_seen: 0,
            warnings: Vec::new(),
            tree_cache: None,
            model_cache: None,
            perf: None,
        };
        session.pump(0.0)?;
        Ok(session)
    }
    pub(super) fn frame(&mut self, ms: f64) -> Result<(), String> {
        self.drive_frame(ms, true)
    }
    fn pump(&mut self, ms: f64) -> Result<(), String> {
        let now = self.shared.run.clock.virtual_time().unwrap();
        let ms = self
            .shared
            .run
            .clock
            .next_due()
            .map_or(ms, |due| ms.min((due - now).max(0.0)));
        self.drive_frame(ms, false)
    }
    fn drive_frame(&mut self, ms: f64, force: bool) -> Result<(), String> {
        self.shared.run.clock.advance(ms)?;
        loop {
            let now = self.shared.run.clock.virtual_time().unwrap() / 1000.0;
            let before = self.runner.frames();
            if force {
                self.runner.frame_at(now)?;
            } else {
                self.runner.turn_at(now)?;
            }
            if let Some((dir, index, case, images)) = self
                .recording
                .clone()
                .filter(|_| self.runner.frames() != before)
            {
                self.serial += 1;
                self.save_frame(
                    &dir,
                    &format!("{index}-intermediate-{}", self.serial),
                    &case,
                    "frame",
                    "intermediate",
                    images,
                )?;
            }
            if self.shared.run.clock.virtual_time().unwrap() / 1000.0 == now {
                break;
            }
        }
        for warning in self.runner.take_warnings() {
            let fatal = matches!(
                warning.code,
                "extension-view-error" | "unbalanced-extension" | "invalid-lua-node"
            );
            let message = format!("{}: {}", warning.code, warning.message);
            self.warnings.push(message.clone());
            self.shared
                .run
                .record("warning", None, json!({"message":message}));
            if fatal {
                return Err(message);
            }
        }
        let errors = self.shared.errors.lock().unwrap();
        if errors.len() > self.errors_seen {
            let message = errors[self.errors_seen..].join("\n");
            self.errors_seen = errors.len();
            return Err(message);
        }
        if self.runner.ended() {
            return Err("app exited during the test".into());
        }
        Ok(())
    }
    fn shutdown(&mut self) -> Result<(), String> {
        self.runner.input().close();
        self.runner
            .frame_at(self.shared.run.clock.virtual_time().unwrap() / 1000.0)?;
        if !self.shared.ops.wait_idle(Duration::from_secs(5)) {
            return Err("app operations did not stop during teardown".into());
        }
        let errors = self.shared.errors.lock().unwrap();
        if errors.len() > self.errors_seen {
            return Err(errors[self.errors_seen..].join("\n"));
        }
        Ok(())
    }
    fn graph(&self, v: mlua::Value) -> mlua::Result<String> {
        let module = graph_module(&self.lua)?;
        module.get::<mlua::Function>("encode")?.call(v)
    }
    pub(super) fn model_graph(&self) -> mlua::Result<String> {
        self.graph(self.lua.globals().get::<mlua::Value>("model")?)
    }
    fn nodes(&mut self) -> Value {
        Value::List(
            self.runner
                .core_mut()
                .nodes()
                .iter()
                .map(|n| n.to_value(kui_core::value::Handles::INT))
                .collect(),
        )
    }
    fn key(&mut self, target: &mlua::Value) -> mlua::Result<Key> {
        let core = self.runner.core_mut();
        match target {
            mlua::Value::Integer(k) => Ok(Key(*k as u64)),
            mlua::Value::String(s) => {
                let s = s.to_str()?;
                core.key_named(&s)
                    .or_else(|| core.key_of(&s))
                    .ok_or_else(|| error(format!("no node labelled or keyed {s:?}")))
            }
            _ => Err(error("target must be a label or node key")),
        }
    }
    pub(super) fn debug_capture(
        &mut self,
        dir: &std::path::Path,
        prefix: &str,
        name: &str,
        phase: &str,
    ) -> Result<(), String> {
        let case = self
            .recording
            .as_ref()
            .map(|r| r.2.clone())
            .unwrap_or_default();
        self.save_frame(dir, prefix, &case, name, phase, true)
    }
    fn save_frame(
        &mut self,
        dir: &std::path::Path,
        prefix: &str,
        case: &str,
        name: &str,
        phase: &str,
        image: bool,
    ) -> Result<(), String> {
        if let Some(error) = self.shared.run.journal_error.lock().unwrap().as_ref() {
            return Err(error.clone());
        }
        let nodes = self.runner.core_mut().nodes_full();
        let tree_blob = if let Some((last, name)) = &self.tree_cache {
            if *last == nodes {
                name.clone()
            } else {
                let tree = artifact::value(&Value::List(
                    nodes
                        .iter()
                        .map(|n| n.to_value(kui_core::value::Handles::HEX))
                        .collect(),
                ));
                artifact::blob(dir, &serde_json::to_vec(&tree).map_err(|e| e.to_string())?)?
            }
        } else {
            let tree = artifact::value(&Value::List(
                nodes
                    .iter()
                    .map(|n| n.to_value(kui_core::value::Handles::HEX))
                    .collect(),
            ));
            artifact::blob(dir, &serde_json::to_vec(&tree).map_err(|e| e.to_string())?)?
        };
        self.tree_cache = Some((nodes, tree_blob.clone()));
        let core_state = artifact::value(&self.runner.core_mut().inspection_state());
        let drawing = super::drawing::save(dir, self.runner.core_mut())?;
        let revision = kui_lua::root_rev(&self.lua);
        let model_blob = if let Some((rev, name)) = &self.model_cache {
            if Some(*rev) == revision {
                name.clone()
            } else {
                artifact::blob(
                    dir,
                    self.model_graph().map_err(|e| e.to_string())?.as_bytes(),
                )?
            }
        } else {
            artifact::blob(
                dir,
                self.model_graph().map_err(|e| e.to_string())?.as_bytes(),
            )?
        };
        if let Some(rev) = revision {
            self.model_cache = Some((rev, model_blob.clone()));
        }
        let adapter: Option<Table> = self
            .lua
            .globals()
            .get::<Table>("kui")
            .and_then(|k| k.get::<Table>("test"))
            .and_then(|t| t.get("__adapter"))
            .ok()
            .flatten();
        let adapter_blob = if let Some(adapter) = adapter {
            let capture: mlua::Function = adapter.get("capture").map_err(|e| e.to_string())?;
            let v = capture.call::<mlua::Value>(()).map_err(|e| e.to_string())?;
            let value = json!({"renderer":adapter.get::<String>("renderer").map_err(|e|e.to_string())?,"graph":serde_json::from_str::<serde_json::Value>(&self.graph(v).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?});
            Some(artifact::blob(
                dir,
                &serde_json::to_vec(&value).map_err(|e| e.to_string())?,
            )?)
        } else {
            None
        };
        // Kit exports its state without running component handlers.
        let kit_state:mlua::Value=self.kit.load("local c=package.loaded.ctx; return c and c.capture_state and c.capture_state() or {}").eval().map_err(|e|e.to_string())?;
        let kit_module = graph_module(&self.kit).map_err(|e| e.to_string())?;
        let kit_encoded = kit_module
            .get::<mlua::Function>("encode")
            .and_then(|f| f.call::<String>(kit_state))
            .ok();
        let screenshot = if image {
            let shot = kui_native::capture(self.runner.core_mut())?;
            Some(artifact::image(dir, &artifact::png(&shot)?)?)
        } else {
            None
        };
        let filename = format!("frames/{prefix}.json");
        artifact::atomic_json(
            &dir.join(&filename),
            &json!({"version":1,"case":case,"step":name,"step_id":self.shared.run.step.load(std::sync::atomic::Ordering::Relaxed),"phase":phase,"tree_blob":tree_blob,"core_blob":artifact::blob(dir,&serde_json::to_vec(&core_state).map_err(|e|e.to_string())?)?,"model_blob":model_blob,"adapter_blob":adapter_blob,"kit_blob":artifact::blob(dir,kit_encoded.as_deref().unwrap_or("null").as_bytes())?,"drawing":drawing,"perf":self.perf,"shot":screenshot,"logical_width":self.runner.size().0.w,"logical_height":self.runner.size().0.h,"time":self.shared.run.clock.virtual_time()}),
        )?;
        if self.perf.is_some() {
            self.runner
                .core_mut()
                .perf_flush()
                .map_err(|e| e.to_string())?;
        }
        self.shared.run.record(
            "frame",
            None,
            json!({"name":name,"phase":phase,"path":filename,"shot":screenshot}),
        );
        Ok(())
    }
}

type StepRecord = (u64, String, Option<(String, u32)>);

struct Controller {
    options: Options,
    file: PathBuf,
    dir: PathBuf,
    tmp: PathBuf,
    app: Option<AppSession>,
    run: Option<Arc<RunContext>>,
    case: String,
    case_index: usize,
    frame: u64,
    results: Results,
    started: Instant,
    steps: Vec<StepRecord>,
    next_step: u64,
    location: Option<(String, u32)>,
    debug: super::ui::DebugControl,
}
impl Controller {
    fn begin(&mut self, lua: &Lua, name: String) -> mlua::Result<()> {
        self.app.take();
        self.run.take();
        self.debug.clear_failure();
        self.case = name;
        self.case_index += 1;
        self.frame = 0;
        self.steps.clear();
        self.next_step = 0;
        let root = self.tmp.join(format!("case-{}", self.case_index));
        std::fs::create_dir_all(root.join("data")).map_err(error)?;
        std::fs::create_dir_all(root.join("cache")).map_err(error)?;
        let project = project::find(&self.options.root).map_err(error)?;
        let run = RunContext::test(
            Vec::new(),
            project.name.clone(),
            modules::AppInfo {
                id: project.id.clone(),
                version: project.version.clone(),
                data_dir: root.join("data"),
                cache_dir: root.join("cache"),
                packed: None,
            },
            root,
        );
        *run.journal.lock().unwrap() = Some(self.dir.join("events.jsonl"));
        *run.case_name.lock().unwrap() = Some(self.case.clone());
        run.record(
            "case",
            None,
            json!({"name":self.case,"index":self.case_index}),
        );
        lua.set_app_data(modules::Shared::in_run(
            project.dir.clone(),
            None,
            run.clone(),
        ));
        modules::install_context(
            lua,
            &lua.globals().get::<Table>("kui")?,
            &modules::shared(lua),
        )?;
        self.run = Some(run);
        Ok(())
    }
    fn capture(&mut self, name: &str, phase: &str, image: bool) -> Result<(), String> {
        if let Some(app) = self.app.as_mut() {
            self.frame += 1;
            app.save_frame(
                &self.dir,
                &format!("{}-{}", self.case_index, self.frame),
                &self.case,
                name,
                phase,
                image,
            )?;
            self.debug.present(app.runner.core_mut())?;
        }
        Ok(())
    }
    fn boundary(&mut self, name: &str, phase: &str) -> Result<(), String> {
        let parent = self.steps.last().map(|s| s.0);
        if phase == "before" {
            self.next_step += 1;
            self.steps
                .push((self.next_step, name.into(), self.location.clone()));
        }
        if let Some(run) = &self.run {
            let step = self.steps.last().map_or(0, |s| s.0);
            run.step.store(step, std::sync::atomic::Ordering::Relaxed);
            let source = self.steps.last().and_then(|s| s.2.clone());
            run.record("step",None,json!({"name":name,"phase":phase,"parent":if phase=="before"{parent}else{self.steps.iter().rev().nth(1).map(|s|s.0)},"source":source}));
        }
        let captured = self.capture(name, phase, true);
        if phase == "after" {
            self.steps.pop();
            if let Some(run) = &self.run {
                run.step.store(
                    self.steps.last().map_or(0, |s| s.0),
                    std::sync::atomic::Ordering::Relaxed,
                );
            }
        }
        captured?;
        if phase == "before" {
            if !(self.debug.bootstrapping() && self.app.is_none()) {
                self.debug.boundary(self.app.as_mut())?;
            }
        } else {
            if name == "start" {
                self.debug.app_started();
            }
            self.debug.after_boundary(self.app.as_mut())?;
        }
        Ok(())
    }
    fn drive(&mut self, lua: &Lua, action: &str, mut args: MultiValue) -> mlua::Result<MultiValue> {
        if !matches!(action, "nodes" | "model" | "text" | "calls") {
            self.boundary(action, "before").map_err(error)?;
        }
        if action == "expect_table" {
            let name = String::from_lua(args.pop_front().unwrap_or_default(), lua)?;
            let value = args.pop_front().unwrap_or_default();
            let path = artifact::snapshot_path(&self.file, &self.case, &name, "table", "txt")
                .map_err(error)?;
            let graph = snapshot(lua, value)?;
            let text = serde_json::to_string_pretty(
                &serde_json::from_str::<serde_json::Value>(&graph).map_err(error)?,
            )
            .map_err(error)?
                + "\n";
            let result = artifact::text_snapshot(&path, &text, self.options.update);
            if let Some(run) = &self.run {
                artifact::snapshot_event(run, &name, action, &path, &result, None);
            }
            result.map_err(error)?;
            self.boundary(action, "after").map_err(error)?;
            return Ok(MultiValue::new());
        }
        if !matches!(action, "nodes" | "model" | "text" | "warnings" | "calls") {
            let values = lua.create_table()?;
            values.set("n", args.len())?;
            for (i, value) in args.iter().enumerate() {
                values.raw_set(i + 1, value.clone())?;
            }
            if let Some(run) = &self.run {
                run.record("input",None,json!({"action":action,"args":serde_json::from_str::<serde_json::Value>(&snapshot(lua,mlua::Value::Table(values))?).map_err(error)?}));
            }
        }
        let app = self
            .app
            .as_mut()
            .ok_or_else(|| error("start the app with test.start before driving it"))?;
        let mut out = MultiValue::new();
        match action {
            "frames" => {
                let n = u32::from_lua(args.pop_front().unwrap_or(mlua::Value::Integer(1)), lua)?;
                for _ in 0..n {
                    app.frame(1000.0 / 60.0).map_err(error)?;
                    self.debug.present(app.runner.core_mut()).map_err(error)?;
                    self.debug.checkpoint(Some(app)).map_err(error)?;
                }
            }
            "advance" => {
                let ms = f64::from_lua(args.pop_front().unwrap_or(mlua::Value::Number(0.0)), lua)?;
                if !ms.is_finite() || ms < 0.0 {
                    return Err(error("advance needs finite nonnegative milliseconds"));
                }
                let mut left = ms;
                if left == 0.0 {
                    app.frame(0.0).map_err(error)?;
                }
                while left > 0.000000001 {
                    let before = app.shared.run.clock.virtual_time().unwrap();
                    let step = left.min(1000.0 / 60.0);
                    app.pump(step).map_err(error)?;
                    self.debug.present(app.runner.core_mut()).map_err(error)?;
                    self.debug.checkpoint(Some(app)).map_err(error)?;
                    left -= (app.shared.run.clock.virtual_time().unwrap() - before).max(0.0);
                }
            }
            "dispatch" => {
                let v = args.pop_front().unwrap_or_default();
                let v = copy(lua, &app.lua, v)?;
                app.lua
                    .globals()
                    .get::<Table>("kui")?
                    .get::<mlua::Function>("dispatch")?
                    .call::<()>(v)?;
                app.frame(0.0).map_err(error)?;
            }
            "press" | "click" => {
                let target = args
                    .pop_front()
                    .ok_or_else(|| error("press/click needs a target"))?;
                if action == "click"
                    && matches!(target, mlua::Value::Number(_) | mlua::Value::Integer(_))
                    && !args.is_empty()
                {
                    let x = f32::from_lua(target, lua)?;
                    let y = f32::from_lua(args.pop_front().unwrap(), lua)?;
                    app.shared.run.record("target",None,json!({"action":action,"point":{"x":x,"y":y},"label":format!("({x:.0}, {y:.0})")}));
                    app.runner
                        .input()
                        .send(InputEvent::CursorMoved(Vec2::new(x, y)));
                    app.runner.input().send(InputEvent::mouse_down(1));
                    app.runner.input().send(InputEvent::mouse_up());
                } else {
                    let key = app.key(&target)?;
                    let n = app
                        .runner
                        .core_mut()
                        .nodes()
                        .into_iter()
                        .find(|n| n.key == key)
                        .ok_or_else(|| error("node has no rectangle"))?;
                    app.shared.run.record("target",None,json!({"action":action,"key":format!("{:016x}",key.0),"label":match &target {mlua::Value::String(s)=>Some(s.to_str()?.to_string()),_=>n.label.or(n.text)},"rect":{"x":n.rect.x,"y":n.rect.y,"w":n.rect.w,"h":n.rect.h},"point":{"x":n.rect.x+n.rect.w/2.0,"y":n.rect.y+n.rect.h/2.0}}));
                    if action == "press" {
                        app.runner
                            .input()
                            .send(InputEvent::Access(AccessRequest::new(
                                key,
                                AccessAction::Click,
                            )));
                    } else {
                        let n = app
                            .runner
                            .core_mut()
                            .nodes()
                            .into_iter()
                            .find(|n| n.key == key)
                            .ok_or_else(|| error("node has no rectangle"))?;
                        app.runner.input().send(InputEvent::CursorMoved(Vec2::new(
                            n.rect.x + n.rect.w / 2.0,
                            n.rect.y + n.rect.h / 2.0,
                        )));
                        app.runner.input().send(InputEvent::mouse_down(1));
                        app.runner.input().send(InputEvent::mouse_up());
                    }
                }
                app.frame(0.0).map_err(error)?;
            }
            "type" => {
                let first = args.pop_front().ok_or_else(|| error("type needs text"))?;
                let text = if let Some(text) = args.pop_front() {
                    let key = app.key(&first)?;
                    let n = app
                        .runner
                        .core_mut()
                        .nodes()
                        .into_iter()
                        .find(|n| n.key == key)
                        .ok_or_else(|| error("node has no rectangle"))?;
                    app.runner.input().send(InputEvent::CursorMoved(Vec2::new(
                        n.rect.x + n.rect.w / 2.0,
                        n.rect.y + n.rect.h / 2.0,
                    )));
                    app.runner.input().send(InputEvent::mouse_down(1));
                    app.runner.input().send(InputEvent::mouse_up());
                    app.frame(0.0).map_err(error)?;
                    String::from_lua(text, lua)?
                } else {
                    String::from_lua(first, lua)?
                };
                app.runner.input().send(InputEvent::Text(text));
                app.frame(0.0).map_err(error)?;
            }
            "key" => {
                let mut converted = MultiValue::new();
                for v in args {
                    converted.push_back(copy(lua, &app.lua, v)?);
                }
                app.lua
                    .globals()
                    .get::<Table>("kui")?
                    .get::<Table>("input")?
                    .get::<mlua::Function>("key")?
                    .call::<()>(converted)?;
                app.frame(0.0).map_err(error)?;
            }
            "resize" => {
                let w = f32::from_lua(args.pop_front().unwrap_or_default(), lua)?;
                let h = f32::from_lua(args.pop_front().unwrap_or_default(), lua)?;
                if !w.is_finite() || !h.is_finite() || w <= 0.0 || h <= 0.0 {
                    return Err(error("resize needs positive finite dimensions"));
                }
                app.runner.input().resize(w, h, 1.0);
                app.frame(0.0).map_err(error)?;
            }
            "warnings" => out.push_back(mlua::Value::Table(
                lua.create_sequence_from(app.warnings.clone())?,
            )),
            "nodes" => out.push_back(kui_lua::value_to_lua(lua, &app.nodes())?),
            "model" => out.push_back(restore(lua, &app.model_graph()?, true)?),
            "text" => {
                let label = String::from_lua(args.pop_front().unwrap_or_default(), lua)?;
                let nodes = app.runner.core_mut().nodes();
                let texts = if let Some(i) = nodes
                    .iter()
                    .position(|n| n.label.as_deref() == Some(&label))
                {
                    nodes[i..]
                        .iter()
                        .take_while(|n| n.key == nodes[i].key || n.depth > nodes[i].depth)
                        .filter_map(|n| app.runner.core_mut().text_of(n.key))
                        .collect::<Vec<_>>()
                } else {
                    Vec::new()
                };
                out.push_back(mlua::Value::Table(lua.create_sequence_from(texts)?));
            }
            "task" => {
                let code = String::from_lua(args.pop_front().unwrap_or_default(), lua)?;
                let timeout = args
                    .pop_front()
                    .map(|v| f64::from_lua(v, lua))
                    .transpose()?
                    .unwrap_or(10_000.0);
                app.lua
                    .load(
                        r#"
local source=...
local fn=assert(load(source,'@test:app-task'))
kui.__test_result=nil
kui.spawn(function() kui.__test_result=table.pack(pcall(fn)) end)
"#,
                    )
                    .call::<()>(code)?;
                kui_lua::root_moved(&app.lua);
                let start = app.shared.run.clock.virtual_time().unwrap();
                loop {
                    if let Some(result) = app
                        .lua
                        .globals()
                        .get::<Table>("kui")?
                        .get::<Option<Table>>("__test_result")?
                    {
                        if !result.get::<bool>(1)? {
                            return Err(error(result.get::<mlua::Value>(2)?.to_string()?));
                        }
                        out.push_back(restore(
                            lua,
                            &app.graph(result.get::<mlua::Value>(2)?)?,
                            true,
                        )?);
                        break;
                    }
                    if app.shared.run.clock.virtual_time().unwrap() - start > timeout {
                        return Err(error("app task timed out"));
                    }
                    app.pump(1000.0 / 60.0).map_err(error)?;
                    self.debug.present(app.runner.core_mut()).map_err(error)?;
                    self.debug.checkpoint(Some(app)).map_err(error)?;
                    std::thread::sleep(Duration::from_millis(1));
                }
                kui_lua::root_moved(&app.lua);
                app.frame(0.0).map_err(error)?;
            }
            "eval" => {
                let code = String::from_lua(args.pop_front().unwrap_or_default(), lua)?;
                let value = app
                    .lua
                    .load(&code)
                    .set_name("@test:app-eval")
                    .eval::<mlua::Value>()?;
                out.push_back(restore(lua, &app.graph(value)?, true)?);
                kui_lua::root_moved(&app.lua);
                app.frame(0.0).map_err(error)?;
            }
            "expect_tree" | "expect_quads" | "expect_shot" | "expect_table" => {
                let name = String::from_lua(args.pop_front().unwrap_or_default(), lua)?;
                let extension = if action == "expect_shot" {
                    "png"
                } else {
                    "txt"
                };
                let path = artifact::snapshot_path(
                    &self.file,
                    &self.case,
                    &name,
                    action.trim_start_matches("expect_"),
                    extension,
                )
                .map_err(error)?;
                let actual = if action == "expect_shot" {
                    Some(kui_native::capture(app.runner.core_mut()).map_err(error)?)
                } else {
                    None
                };
                let result = if action == "expect_shot" {
                    let tolerance = if let Some(mlua::Value::Table(opts)) = args.pop_front() {
                        opts.get::<Option<u8>>("tolerance")?.unwrap_or(2)
                    } else {
                        2
                    };
                    artifact::image_snapshot(
                        &path,
                        actual.as_ref().unwrap(),
                        tolerance,
                        self.options.update,
                    )
                } else {
                    let text=match action {
                        "expect_tree"=>app.runner.core_mut().nodes_full().iter().map(|n|format!("{}{} key={:016x} role={} label={:?} text={:?} rect={:.0},{:.0},{:.0},{:.0}","  ".repeat(n.depth as usize),n.label.as_deref().unwrap_or("_"),n.key.0,n.role.map(|r|r.name()).unwrap_or("_"),n.label,n.text,n.rect.x,n.rect.y,n.rect.w,n.rect.h)).collect::<Vec<_>>().join("\n")+"\n",
                        "expect_quads"=>app.runner.core_mut().drawn().0.quads.iter().map(|q|format!("{:?} {:.0},{:.0},{:.0},{:.0} #{:08x}",q.kind,q.rect.x,q.rect.y,q.rect.w,q.rect.h,q.color.to_hex())).collect::<Vec<_>>().join("\n")+"\n",
                        _=>{let v=args.pop_front().unwrap_or_default();let s=snapshot(lua,v)?;serde_json::to_string_pretty(&serde_json::from_str::<serde_json::Value>(&s).map_err(error)?).map_err(error)?+"\n"},
                    };
                    artifact::text_snapshot(&path, &text, self.options.update)
                };
                artifact::snapshot_event(
                    &app.shared.run,
                    &name,
                    action,
                    &path,
                    &result,
                    actual.as_ref(),
                );
                result.map_err(error)?;
            }
            _ => return Err(error(format!("unknown driving action {action}"))),
        }
        if !matches!(action, "nodes" | "model" | "text" | "calls") {
            self.boundary(action, "after").map_err(error)?;
        }
        Ok(out)
    }
}
fn graph_module(lua: &Lua) -> mlua::Result<Table> {
    if let Ok(module) = lua.named_registry_value::<Table>("kui.state-graph") {
        return Ok(module);
    }
    let module: Table = lua.load(SNAPSHOT).eval()?;
    lua.set_named_registry_value("kui.state-graph", module.clone())?;
    Ok(module)
}
fn snapshot(lua: &Lua, value: mlua::Value) -> mlua::Result<String> {
    graph_module(lua)?
        .get::<mlua::Function>("encode")?
        .call(value)
}
fn restore(lua: &Lua, text: &str, inspection: bool) -> mlua::Result<mlua::Value> {
    graph_module(lua)?
        .get::<mlua::Function>("decode")?
        .call((text, inspection))
}
fn copy(from: &Lua, to: &Lua, value: mlua::Value) -> mlua::Result<mlua::Value> {
    restore(to, &snapshot(from, value)?, false)
}

pub fn run_file(
    options: Options,
    file: PathBuf,
    dir: PathBuf,
    tmp: PathBuf,
) -> Result<i32, String> {
    // Test files are trusted local programs; debug supplies source locations and tracebacks.
    let lua = unsafe { Lua::unsafe_new() };
    kui_lua::install_data(&lua).map_err(|e| e.to_string())?;
    project::add_path(&lua, &options.root).map_err(|e| e.to_string())?;
    modules::install_loader(&lua).map_err(|e| e.to_string())?;
    crate::teal::install(&lua).map_err(|e| e.to_string())?;
    modules::install(&lua, modules::Shared::new(options.root.clone(), None))
        .map_err(|e| e.to_string())?;
    let debug = super::ui::DebugControl::new(&options, &dir);
    let state = Rc::new(RefCell::new(Controller {
        options: options.clone(),
        file: file.clone(),
        dir: dir.clone(),
        tmp,
        app: None,
        run: None,
        case: String::new(),
        case_index: 0,
        frame: 0,
        results: Results {
            version: 1,
            ..Default::default()
        },
        started: Instant::now(),
        steps: Vec::new(),
        next_step: 0,
        location: None,
        debug,
    }));
    let t: Table = lua
        .globals()
        .get::<Table>("kui")
        .and_then(|k| k.get("test"))
        .map_err(|e| e.to_string())?;
    macro_rules! install {
        ($name:expr,$body:expr) => {{
            let state = state.clone();
            t.set(
                $name,
                lua.create_function(move |lua, args| $body(&mut state.borrow_mut(), lua, args))
                    .map_err(|e| e.to_string())?,
            )
            .map_err(|e| e.to_string())?;
        }};
    }
    install!(
        "__begin",
        |s: &mut Controller, lua: &Lua, (name, _file, _line): (String, String, u32)| s
            .begin(lua, name)
    );
    install!("__source", |s: &mut Controller,
                          _: &Lua,
                          (file, line): (String, u32)|
     -> mlua::Result<()> {
        s.location = Some((file, line));
        Ok(())
    });
    install!("__failed", |s: &mut Controller,
                          _: &Lua,
                          (name, message): (String, String)|
     -> mlua::Result<()> {
        if let Some(run) = &s.run {
            run.record("step_error", None, json!({"name":name,"error":message,"source":s.location.as_ref().map(|(file,line)|json!({"file":file,"line":line}))}));
        }
        if s.steps.last().is_some_and(|step| step.1 == name) {
            let _ = s.boundary(&name, "after");
        }
        Ok(())
    });
    install!("__mock", |s: &mut Controller,
                        _: &Lua,
                        (service, rule): (String, Table)|
     -> mlua::Result<()> {
        let run = s
            .run
            .as_ref()
            .ok_or_else(|| error("configure mocks inside it or a hook"))?;
        let rule = artifact::value(&kui_lua::lua_to_value(&mlua::Value::Table(rule))?);
        run.effects.add(&service, rule).map_err(error)
    });
    install!("__calls", |s: &mut Controller,
                         lua: &Lua,
                         service: Option<String>|
     -> mlua::Result<mlua::Value> {
        let run = s
            .run
            .as_ref()
            .ok_or_else(|| error("calls belongs inside a case"))?;
        let value = kui_core::json::parse(&run.effects.calls(service.as_deref()).to_string())
            .map_err(error)?;
        kui_lua::value_to_lua(lua, &value)
    });
    install!("__start", |s: &mut Controller,
                         lua: &Lua,
                         opts: Option<Table>|
     -> mlua::Result<()> {
        s.boundary("start", "before").map_err(error)?;
        if let Some(app) = s.app.as_mut() {
            app.shutdown().map_err(error)?;
        }
        s.app.take();
        let previous = s
            .run
            .as_ref()
            .ok_or_else(|| error("test.start belongs inside it or a hook"))?;
        let arguments = opts
            .as_ref()
            .map(|o| o.get::<Option<Vec<String>>>("args"))
            .transpose()?
            .flatten()
            .unwrap_or_default();
        let test_shared = modules::shared(lua);
        test_shared.shutdown();
        let run = previous.restart(arguments);
        previous.cleanup();
        s.run = Some(run.clone());
        lua.set_app_data(modules::Shared::in_run(
            s.options.root.clone(),
            None,
            run.clone(),
        ));
        modules::install_context(
            lua,
            &lua.globals().get::<Table>("kui")?,
            &modules::shared(lua),
        )?;
        let size = opts
            .as_ref()
            .map(|o| o.get::<Option<Table>>("size"))
            .transpose()?
            .flatten()
            .map(|t| Ok::<_, mlua::Error>((t.get::<f32>("w")?, t.get::<f32>("h")?)))
            .transpose()?;
        let project = project::find(&s.options.root).map_err(error)?;
        let mut app =
            AppSession::open(&project, run, s.options.system_fonts, size).map_err(error)?;
        if s.options.perf {
            let name = format!("perf-{}-{}.kuitrace", s.case_index, s.next_step);
            app.runner
                .core_mut()
                .perf_trace_to(&s.dir.join(&name).to_string_lossy())
                .map_err(error)?;
            app.perf = Some(name);
            app.frame(0.0).map_err(error)?;
        }
        app.recording = Some((
            s.dir.clone(),
            s.case_index,
            s.case.clone(),
            s.options.every_frame,
        ));
        s.app = Some(app);
        s.boundary("start", "after").map_err(error)
    });
    install!(
        "__drive",
        |s: &mut Controller, lua: &Lua, (name, args): (String, MultiValue)| s
            .drive(lua, &name, args)
    );
    install!("__pause", |s: &mut Controller,
                         _: &Lua,
                         (): ()|
     -> mlua::Result<()> {
        s.debug.pause().map_err(error)?;
        s.debug.boundary(s.app.as_mut()).map_err(error)
    });
    install!(
        "__boundary",
        |s: &mut Controller, _: &Lua, (name, phase): (String, String)| s
            .boundary(&name, &phase)
            .map_err(error)
    );
    install!("__wall_time", |s: &mut Controller, _: &Lua, (): ()| Ok(s
        .started
        .elapsed()
        .as_secs_f64()
        - s.debug.paused_seconds()));
    install!("__yield_io", |s: &mut Controller,
                            _: &Lua,
                            (): ()|
     -> mlua::Result<()> {
        std::thread::sleep(Duration::from_millis(1));
        s.debug.poll().map_err(error)
    });
    install!("__finish", |s: &mut Controller,
                          lua: &Lua,
                          (
        name,
        mut ok,
        mut err,
        seconds,
        file,
        line,
    ): (
        String,
        bool,
        Option<String>,
        f64,
        String,
        u32
    )|
     -> mlua::Result<()> {
        if let Err(e) = s.capture("case end", "after", true) {
            ok = false;
            err = Some(format!("{}\n{e}", err.unwrap_or_default()));
        }
        let pending = if !ok && s.options.debug && !s.debug.cancelled() {
            let index = s.results.cases.len();
            s.results.cases.push(CaseResult {
                name: name.clone(),
                ok,
                error: err.clone(),
                seconds,
                file: file.clone(),
                line,
            });
            artifact::atomic_json(&s.dir.join("results.json"), &s.results).map_err(error)?;
            s.debug.pause_failure().map_err(error)?;
            s.debug.boundary(s.app.as_mut()).map_err(error)?;
            Some(index)
        } else {
            None
        };
        if let Some(app) = s.app.as_mut()
            && let Err(e) = app.shutdown()
        {
            ok = false;
            err = Some(format!("{}\n{e}", err.unwrap_or_default()));
        }
        s.app.take();
        let test_shared = modules::shared(lua);
        test_shared.shutdown();
        if !test_shared.ops.wait_idle(Duration::from_secs(5)) {
            ok = false;
            err = Some(format!(
                "{}\ntest operations did not stop during teardown",
                err.unwrap_or_default()
            ));
        }
        if let Some(run) = s.run.take() {
            run.record("result", None, json!({"name":name,"ok":ok,"error":err}));
            if let Some(error) = run.journal_error.lock().unwrap().as_ref() {
                ok = false;
                err = Some(format!("{}\n{error}", err.unwrap_or_default()));
            }
            run.cleanup();
        }
        let result = CaseResult {
            name,
            ok,
            error: err,
            seconds,
            file,
            line,
        };
        if let Some(index) = pending {
            s.results.cases[index] = result;
        } else {
            s.results.cases.push(result);
        }
        artifact::atomic_json(&s.dir.join("results.json"), &s.results).map_err(error)
    });
    lua.load(r#"
local t=kui.test
function t.mock(service,rule) t.__mock(service,rule) end
function t.calls(service) return t.__calls(service) end
for _,service in ipairs({'http','process','fs','sqlite','secrets'}) do t['mock_'..service]=function(rule) t.mock(service,rule) end end
function t.pause() t.__pause() end
local function source() local info=debug.getinfo(3,'Sl');if info then t.__source(info.source:gsub('^@',''),info.currentline) end end
function t.start(a,b)
  source();local ok,err=pcall(t.__start,a==t and b or a)
  if not ok then t.__failed('start',tostring(err));error(err,0) end;return t
end
for _,name in ipairs({'frames','advance','dispatch','press','click','type','key','resize','warnings','nodes','model','text','eval','task','expect_tree','expect_quads','expect_shot','expect_table'}) do
  t[name]=function(...)
    source();local a=table.pack(...);local first=a[1]==t and 2 or 1
    local out=table.pack(pcall(t.__drive,name,table.unpack(a,first,a.n)))
    if not out[1] then t.__failed(name,tostring(out[2]));error(out[2],0) end
    return table.unpack(out,2,out.n)
  end
end
function t.replay(adapter) assert(type(adapter.capture)=='function' and type(adapter.renderer)=='string','replay needs capture and renderer'); t.__adapter=adapter end
"#).exec().map_err(|e|e.to_string())?;
    let source = std::fs::read_to_string(&file).map_err(|e| e.to_string())?;
    let source = crate::teal::source_for(&file, &source).map_err(|e| e.to_string())?;
    lua.load(source)
        .set_name(format!("@{}", file.display()))
        .exec()
        .map_err(|e| e.to_string())?;
    let collected = t
        .get::<mlua::Function>("__list")
        .and_then(|f| f.call::<mlua::Value>(()))
        .map_err(|e| e.to_string())?;
    let cases = artifact::value(&kui_lua::lua_to_value(&collected).map_err(|e| e.to_string())?);
    artifact::atomic_json(
        &dir.join("collection.json"),
        &json!({"file":file,"cases":cases}),
    )?;
    let count = t
        .get::<mlua::Function>("__run")
        .and_then(|f| f.call::<usize>((options.filter.clone(), options.exact)))
        .map_err(|e| e.to_string())?;
    if count == 0 && options.filter.is_none() {
        return Err("test file collected no test cases".into());
    }
    let mut s = state.borrow_mut();
    s.results.complete = true;
    artifact::atomic_json(&dir.join("results.json"), &s.results)?;
    Ok(if s.results.cases.iter().any(|c| !c.ok) {
        1
    } else {
        0
    })
}

/// Reconstructs only the registered renderer, never the ordinary app entry.
pub(super) fn render_state(
    root: &std::path::Path,
    frame: &std::path::Path,
) -> Result<kui_native::Capture, String> {
    let value = artifact::read_frame(frame)?;
    let adapter = value["state"]["adapter"]
        .as_object()
        .ok_or("this app has no capture/render adapter")?;
    let renderer = adapter
        .get("renderer")
        .and_then(serde_json::Value::as_str)
        .ok_or("adapter has no renderer module")?
        .to_string();
    if renderer.is_empty()
        || renderer.contains(['/', '\\'])
        || renderer
            .split('.')
            .any(|s| s.is_empty() || !s.chars().all(|c| c.is_alphanumeric() || c == '_'))
    {
        return Err("invalid replay renderer module name".into());
    }
    let graph = adapter
        .get("graph")
        .ok_or("adapter has no captured state")?
        .to_string();
    let project = project::find(root)?;
    let temporary = tempfile::tempdir().map_err(|e| e.to_string())?;
    let run = RunContext::test(
        Vec::new(),
        project.name.clone(),
        modules::AppInfo {
            id: project.id.clone(),
            data_dir: temporary.path().join("data"),
            cache_dir: temporary.path().join("cache"),
            ..Default::default()
        },
        temporary.path().to_path_buf(),
    );
    run.clock.advance(value["time"].as_f64().unwrap_or(0.0))?;
    let shared = modules::Shared::in_run(project.dir.clone(), None, run.clone());
    let root = project.dir.clone();
    let setup_shared = shared.clone();
    let ext=kui_lua::LuaExtension::from_source_with("@test:state-render",r#"
local adapter=require(__renderer)
local render=type(adapter)=='function' and adapter or adapter.render
assert(type(render)=='function','state renderer must return a render function or a table with render')
function view(env) return render(__saved_state,env) end
"#,move|lua| {
        project::add_path(lua,&root)?;kit::install_ui(lua)?;modules::install(lua,setup_shared.clone())?;
        kui_lua::set_script_dir(lua,&root)?;
        lua.globals().set("__renderer",renderer.clone())?;
        lua.globals().set("__saved_state",restore(lua,&graph,false)?)?;
        let source_root=root.clone();
        let searcher=lua.create_function(move|lua,name:String| {
            if name.split('.').any(|s|s.is_empty()||!s.chars().all(|c|c.is_alphanumeric()||c=='_')) {return Err(error("invalid renderer import"));}
            let module=source_root.join(name.replace('.',"/"));
            for path in [module.with_extension("tl"),module.with_extension("lua"),module.join("init.tl"),module.join("init.lua")] {
                if path.is_file() {
                    let canonical=crate::paths::canonical(&path).map_err(error)?;
                    if !canonical.starts_with(&source_root){return Err(error("renderer imports must stay in the app"));}
                    let source=std::fs::read_to_string(&canonical).map_err(error)?;
                    let source=crate::teal::source_for(&canonical,&source)?;
                    return lua.load(source).set_name(format!("@{}",canonical.display())).into_function().map(Some);
                }
            }Ok(None)
        })?;
        let package:Table=lua.globals().get("package")?;let existing:Table=package.get("searchers")?;
        package.set("searchers",lua.create_sequence_from([existing.get::<mlua::Function>(1)?,searcher])?)?;
        lua.load(r#"
local function denied() error('state rendering cannot perform I/O, start tasks or dispatch messages',2) end
for _,name in ipairs({'fs','http','process','sqlite','secrets','timer'}) do
  kui[name]=setmetatable({}, {__index=function() return denied end})
end
kui.spawn=denied; kui.worker=denied; kui.sleep=denied; kui.dispatch=denied; kui.next_frame=denied
kui.test_inspector=nil; io=nil; loadfile=nil; dofile=nil
if os then os.execute=denied;os.remove=denied;os.rename=denied;os.exit=denied;os.tmpname=denied end
"#).exec()
    }).map_err(|e|e.to_string())?;
    let lua = ext.lua().clone();
    let kit = kit::extension().map_err(|e| e.to_string())?;
    if let Some(graph) = value.get("kit").filter(|v| !v.is_null()) {
        let utilities = kit.lua().create_table().map_err(|e| e.to_string())?;
        modules::formats::install(kit.lua(), &utilities).map_err(|e| e.to_string())?;
        modules::crypto::install(kit.lua(), &utilities).map_err(|e| e.to_string())?;
        crate::snapshot::install(kit.lua(), &utilities).map_err(|e| e.to_string())?;
        kit.lua()
            .globals()
            .set("kui", utilities)
            .map_err(|e| e.to_string())?;
        kit.lua().load("for _,n in ipairs({'encode','decode'}) do local f=kui.json['try_'..n];kui.json[n]=function(...) local ok,v=f(...);if not ok then error(v,2) end;return v end end").exec().map_err(|e|e.to_string())?;
        let state = restore(kit.lua(), &graph.to_string(), false).map_err(|e| e.to_string())?;
        kit.lua().load("local state=...;local ctx=require('ctx');if ctx.restore_state then ctx.restore_state(state) end").call::<()>(state).map_err(|e|e.to_string())?;
    }
    let core_session = kui_core::Session::with_fonts(
        "en-US",
        [
            include_bytes!("../../assets/fonts/NotoSans-Regular.ttf").to_vec(),
            include_bytes!("../../assets/fonts/NotoSans-Bold.ttf").to_vec(),
            include_bytes!("../../assets/fonts/NotoSansMono-Regular.ttf").to_vec(),
        ],
    );
    let host = host::Runner::new(lua, shared, &project, Rc::default(), None, None, None);
    let mut runner = kui_native::app("State rendering")
        .core(Core::new_in(&core_session))
        .platform(kui_native::Platform::Linux)
        .system(kui_native::SystemEnv {
            appearance: kui_native::Appearance::Light,
            locale: kui_core::Locale::new("en-US"),
            ..Default::default()
        })
        .extension_as("app", ext)
        .extension_as("kit", kit)
        .open_headless(
            host,
            value["logical_width"].as_f64().unwrap_or(project.size.0) as f32,
            value["logical_height"].as_f64().unwrap_or(project.size.1) as f32,
            1.0,
        );
    for _ in 0..3 {
        runner.frame_at(run.clock.virtual_time().unwrap() / 1000.0)?;
    }
    for warning in runner.take_warnings() {
        if matches!(
            warning.code,
            "extension-view-error" | "unbalanced-extension"
        ) {
            return Err(warning.message);
        }
    }
    kui_native::capture(runner.core_mut())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn state_adapter_renders_without_reexecuting_startup_and_rejects_effects() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("kui.config.lua"),
            "return {entry='main.lua',size={w=200,h=100}}",
        )
        .unwrap();
        std::fs::write(
            root.path().join("main.lua"),
            r#"
local t=require('kui.test')
function init() kui.fs.append('startup.txt','start\n'); return {count=7} end
function view() return column { text('Count '..model.count) } end
t.replay {capture=function() return model end,renderer='replay'}
"#,
        )
        .unwrap();
        std::fs::write(
            root.path().join("replay.lua"),
            "return function(state,env) return column {text('Count '..state.count)} end",
        )
        .unwrap();
        let project = project::find(root.path()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let run = RunContext::test(
            Vec::new(),
            "state adapter".into(),
            modules::AppInfo::default(),
            tmp.path().to_path_buf(),
        );
        let mut app = AppSession::open(&project, run, false, None).unwrap();
        let original = kui_native::capture(app.runner.core_mut()).unwrap();
        app.save_frame(root.path(), "saved", "case", "saved", "after", true)
            .unwrap();
        let replay = render_state(root.path(), &root.path().join("frames/saved.json")).unwrap();
        assert_eq!(original, replay);
        assert_eq!(
            std::fs::read_to_string(root.path().join("startup.txt")).unwrap(),
            "start\n"
        );
        std::fs::write(
            root.path().join("replay.lua"),
            "return function() kui.http.get('https://example.test');return column{} end",
        )
        .unwrap();
        assert!(
            render_state(root.path(), &root.path().join("frames/saved.json"))
                .unwrap_err()
                .contains("cannot perform I/O")
        );
    }
    #[test]
    fn large_inspector_payload_keeps_scroll_work_bounded() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(
            root.path().join("kui.config.lua"),
            "return {entry='main.lua',size={w=1440,h=980}}",
        )
        .unwrap();
        std::fs::write(
            root.path().join("inspector.lua"),
            include_str!("../../../../tools/kui-test/inspector.lua"),
        )
        .unwrap();
        std::fs::write(root.path().join("main.lua"),r#"
local animations={}
for i=1,227 do animations[i]={key=tostring(i),slot='size',duration_ms=140,from={32,0,0,0},to={0,0,0,0},value={0,0,0,0},velocity={0,0,0,0},start=0} end
local core={animations=animations,editors={},time=0.15}
local frames={{file='file',path='before',case='payload',step='task',step_id=1,phase='before'},{file='file',path='after',case='payload',step='task',step_id=1,phase='after'}}
kui.test_inspector={read=function()return {run={id='fixture',status='passed'},files={{name='file',collection={file='test.lua',cases={{name='payload'}}},results={cases={}},frames=frames}}}end,detail=function()return {core=core,state_view={count=1},tree={}}end,changes=function()return {}end}
local I=require('inspector')
function init()local m=I.new();I.update(m,{kind='refresh'});I.update(m,{kind='tab',change={value='core'}});return m end
function update(msg)I.update(model,msg)end
function view(env)return I.view(model,env)end
"#).unwrap();
        let project = project::find(root.path()).unwrap();
        let tmp = tempfile::tempdir().unwrap();
        let run = RunContext::test(
            Vec::new(),
            "payload scrolling".into(),
            modules::AppInfo::default(),
            tmp.path().to_path_buf(),
        );
        let mut app = AppSession::open(&project, run, false, None).unwrap();
        let nodes = app.runner.core_mut().nodes();
        let count = nodes.len();
        let target = Vec2::new(620.0, 880.0);
        app.runner.input().send(InputEvent::CursorMoved(target));
        app.frame(0.0).unwrap();
        let start = Instant::now();
        for _ in 0..30 {
            app.runner
                .input()
                .send(InputEvent::Scroll(Vec2::new(0.0, -24.0)));
            app.pump(1000.0 / 60.0).unwrap();
        }
        let elapsed = start.elapsed();
        let moved = app
            .runner
            .core_mut()
            .nodes()
            .iter()
            .any(|n| n.scroll.is_some_and(|v| v.y > 0.0));
        eprintln!(
            "core payload: {count} nodes, 30 scroll frames in {:.1} ms, moved={moved}",
            elapsed.as_secs_f64() * 1000.0
        );
        assert!(moved, "scroll probe never moved the detail pane");
        let source: String = app
            .lua
            .load("return model.payloads['before-core'].source")
            .eval()
            .unwrap();
        app.lua
            .load("kui.dispatch{kind='copy_payload',key='before-core'}")
            .exec()
            .unwrap();
        app.frame(0.0).unwrap();
        assert_eq!(
            app.runner.clipboard(),
            source,
            "Copy all lost rows outside the viewport"
        );
        let last_line:String=app.lua.load(r#"local p=model.payloads['before-core'];for i,line in ipairs(p.lines)do if line:find('"key": "227"',1,true) then kui.dispatch{kind='copy_payload_range',key='before-core',selectionrange={from={index=i-1,byte=0},to={index=i-1,byte=#line}}};return line end end;error('missing final animation')"#).eval().unwrap();
        app.frame(0.0).unwrap();
        assert_eq!(
            app.runner.clipboard(),
            last_line,
            "Selection lost a row outside the viewport"
        );
        app.lua.load(r#"local p=model.payloads['before-core'];for i,line in ipairs(p.lines)do if line:find('"key": "227"',1,true)then kui.next_frame(function(env)env.set_scroll('payload-before-core',0,(i-1)*18)end);break end end"#).exec().unwrap();
        for _ in 0..3 {
            app.frame(0.0).unwrap();
        }
        assert!(
            app.runner
                .core_mut()
                .nodes_full()
                .iter()
                .any(|n| n.text.as_deref() == Some(last_line.as_str())),
            "virtual payload did not render the final animation after scrolling to it"
        );

        assert!(
            count < 4000,
            "inspector materialized {count} nodes for a 227-animation payload; scrolling must cost the visible content"
        );
    }
}
