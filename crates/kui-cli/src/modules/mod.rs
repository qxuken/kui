//! The standard library under `kui.*` (ADR 0064, decision 3), each module a
//! table of Rust functions in the Lua state, and what they share: the
//! state's inbox, the app's folder, and the servers and workers it runs.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use mlua::{Lua, Table};

use crate::ops::Ops;

pub mod formats;
pub mod fs;
pub mod http;
pub mod input;
pub mod log;
pub mod process;
pub mod sqlite;
pub mod timer;
pub mod worker;

/// What every module of one Lua state reaches: kept as the state's app
/// data, an `Arc` so the threads it starts can hold it.
pub struct Shared {
    pub ops: Ops,
    /// The app's folder: what a relative path is read against.
    pub dir: PathBuf,
    /// The servers this state runs, by id.
    pub servers: Mutex<HashMap<u64, http::Server>>,
    /// The workers this state started, by id.
    pub workers: Mutex<HashMap<u64, worker::Handle>>,
    /// Errors raised in tasks, handlers and `update`, newest last, for the
    /// window to show; each was printed as it came.
    pub errors: Mutex<Vec<String>>,
    /// `kui.exit(code)` was called.
    pub exit: Mutex<Option<i32>>,
    /// Inside a worker: its id and the app's inbox, where its dispatches go.
    pub worker: Option<(u64, Ops)>,
    pub timers: timer::Timers,
    /// Processes `kui.process.spawn` started that end with the app.
    pub children: process::Children,
    /// `kui.devtools(on)` asked the panel on (`Some(true)`), off, or
    /// toggled (`None`) — and on a tab, given one — for the next frame.
    pub devtools: Mutex<Option<(Option<bool>, Option<String>)>>,
}

impl Shared {
    pub fn new(dir: PathBuf, worker: Option<(u64, Ops)>) -> Arc<Self> {
        let ops = Ops::default();
        Arc::new(Shared {
            timers: timer::Timers::new(ops.clone()),
            ops,
            dir,
            servers: Mutex::default(),
            workers: Mutex::default(),
            errors: Mutex::default(),
            exit: Mutex::default(),
            worker,
            children: process::Children::default(),
            devtools: Mutex::default(),
        })
    }

    /// A path the app named, against its folder when relative.
    pub fn path(&self, p: &str) -> PathBuf {
        let p = std::path::Path::new(p);
        if p.is_absolute() {
            p.to_path_buf()
        } else {
            self.dir.join(p)
        }
    }

    pub fn error(&self, msg: String) {
        // `kui.parse_args` stopping the script after `--help`.
        if msg.contains(EXIT_MARK) {
            return;
        }
        eprintln!("kui: {msg}");
        log::push("error", msg.clone());
        let mut errors = self.errors.lock().unwrap();
        errors.push(msg);
        let n = errors.len();
        if n > 20 {
            errors.drain(..n - 20);
        }
    }
}

/// The app's own arguments, what followed `--` on the command line.
static ARGS: std::sync::OnceLock<Vec<String>> = std::sync::OnceLock::new();

pub fn set_args(args: Vec<String>) {
    let _ = ARGS.set(args);
}

/// `--headless`: a task that throws ends the run with 1, so a headless
/// test that fails says so instead of waiting for a `kui.exit` that the
/// throw skipped.
static TASK_ERROR_EXITS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

pub fn set_task_error_exits(on: bool) {
    TASK_ERROR_EXITS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// The app's name, the manifest's: what `kui.parse_args` calls it.
static NAME: std::sync::OnceLock<String> = std::sync::OnceLock::new();

pub fn set_name(name: String) {
    let _ = NAME.set(name);
}

/// Who the app is and where it writes (ADR 0068): `kui.id`,
/// `kui.app_version`, `kui.data_dir`, `kui.cache_dir`, and `kui.packed`
/// when it runs from a packed archive.
#[derive(Clone, Default)]
pub struct AppInfo {
    pub id: String,
    pub version: Option<String>,
    pub data_dir: std::path::PathBuf,
    pub cache_dir: std::path::PathBuf,
    /// What `kui.pack.lua` said, for a packed app: (runner, bytecode, hash).
    pub packed: Option<(String, bool, String)>,
}

static APP: std::sync::OnceLock<AppInfo> = std::sync::OnceLock::new();

pub fn set_app(info: AppInfo) {
    let _ = APP.set(info);
}

pub fn app() -> Option<&'static AppInfo> {
    APP.get()
}

/// Lua loads bytecode as readily as source, and does not check it: a
/// crafted chunk can crash the process. So the runner loads source only —
/// `load`, `loadfile`, `dofile` and `require`'s file searcher — except from
/// a packed app's own folder, which `kui pack --bytecode` wrote (ADR 0068,
/// decision 12).
///
/// Installed before the `.tl` searcher, which goes in ahead of it.
pub fn install_loader(lua: &Lua) -> mlua::Result<()> {
    let loadfile = lua.create_function(|lua, (path, env): (String, Option<Table>)| {
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => return Ok((None, Some(format!("cannot open {path}: {e}")))),
        };
        let packed =
            crate::packed::dir().is_some_and(|d| std::path::Path::new(&path).starts_with(d));
        let mut chunk = lua.load(bytes).set_name(format!("@{path}"));
        if !packed {
            chunk = chunk.set_mode(mlua::chunk::ChunkMode::Text);
        }
        if let Some(env) = env {
            chunk = chunk.set_environment(env);
        }
        match chunk.into_function() {
            Ok(f) => Ok((Some(f), None)),
            Err(e) => Ok((None, Some(e.to_string()))),
        }
    })?;
    lua.load(LOADER_LUA)
        .set_name("@kui:loader")
        .call::<()>(loadfile)
}

const LOADER_LUA: &str = r##"
local loadfile_ = ...
local load_ = load
-- An `env` given, even nil, is the chunk's `_ENV`; one left out is the
-- globals. So the arguments are passed on as many as they came.
function load(...)
  local chunk, name, _, env = ...
  if select("#", ...) >= 4 then return load_(chunk, name, "t", env) end
  return load_(chunk, name, "t")
end
function loadfile(path, _mode, env)
  return loadfile_(path, env)
end
function dofile(path)
  local f, err = loadfile_(path)
  if not f then error(err, 2) end
  return f()
end
-- `require`'s searcher for Lua files, the same as Lua's own but through
-- the loader above.
package.searchers[2] = function(name)
  local path, err = package.searchpath(name, package.path)
  if not path then return "\n\t" .. err end
  local f, lerr = loadfile_(path)
  if not f then error(("error loading module '%s' from file '%s':\n\t%s"):format(name, path, lerr), 2) end
  return f, path
end
"##;

/// What `kui.parse_args` raises to stop the script once it has printed
/// help or a mistake and set the exit: no failure to report.
pub const EXIT_MARK: &str = "kui: exit requested";

/// The OS, as `kui.platform` names it.
const PLATFORM: &str = match std::env::consts::OS.as_bytes() {
    b"macos" => "macos",
    b"windows" => "windows",
    b"linux" => "linux",
    _ => std::env::consts::OS,
};

/// Ctrl-C, or a `kill`, in the terminal the app runs from: a `kui.exit`
/// with the signal's code, so the window closes the way it closes from
/// its button and `on_exit` runs. A second one ends the process at once.
/// The handler only writes a byte to a pipe; a thread waits on the other
/// end, so nothing polls.
#[cfg(unix)]
pub fn on_interrupt(shared: Arc<Shared>) {
    use std::sync::atomic::{AtomicI32, Ordering};
    static WRITE: AtomicI32 = AtomicI32::new(-1);
    extern "C" fn handler(sig: libc::c_int) {
        let fd = WRITE.load(Ordering::Relaxed);
        let b = sig as u8;
        // SAFETY: write(2) is async-signal-safe, the buffer is one byte.
        unsafe { libc::write(fd, (&b as *const u8).cast(), 1) };
    }
    let mut fds = [0; 2];
    // SAFETY: a fresh pipe; its ends live for the process.
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        return;
    }
    WRITE.store(fds[1], Ordering::Relaxed);
    for sig in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP] {
        // SAFETY: the handler only writes to the pipe.
        unsafe { libc::signal(sig, handler as *const () as libc::sighandler_t) };
    }
    let read = fds[0];
    let _ = std::thread::Builder::new()
        .name("kui-signals".into())
        .spawn(move || {
            let mut seen = false;
            loop {
                let mut b = 0u8;
                // SAFETY: reads one byte into `b`.
                let n = unsafe { libc::read(read, (&mut b as *mut u8).cast(), 1) };
                if n <= 0 {
                    return;
                }
                if seen {
                    shared.children.kill_all();
                    std::process::exit(128 + b as i32);
                }
                seen = true;
                shared.exit.lock().unwrap().get_or_insert(128 + b as i32);
                shared.ops.poke();
            }
        });
}

#[cfg(not(unix))]
pub fn on_interrupt(_shared: Arc<Shared>) {}

/// The state's shared half, from inside a Lua function.
pub fn shared(lua: &Lua) -> Arc<Shared> {
    lua.app_data_ref::<Arc<Shared>>()
        .expect("the kui runner's state is installed")
        .clone()
}

/// A Lua error carrying `msg` as it is.
pub fn fail(msg: String) -> mlua::Error {
    mlua::Error::runtime(msg)
}

/// The runtime's Lua half (tasks, dispatch, the router).
const RUNTIME: &str = include_str!("../lua/runtime.lua");
const ARGS_LUA: &str = include_str!("../lua/args.lua");

/// Installs `kui` and every module into `lua`, for the app (`ui`) or a
/// worker.
/// A runner call's `try_` form, the runner's convention for a call that
/// can fail on its input or on the world: `true` and what `f` returns, or
/// `false` and the error's message. Only an argument of the wrong type
/// raises, before `f` runs. runtime.lua makes the plain call from it,
/// raising the message at its caller's line.
pub fn try_fn<A, R, F>(lua: &Lua, f: F) -> mlua::Result<mlua::Function>
where
    A: mlua::FromLuaMulti,
    R: mlua::IntoLuaMulti,
    F: Fn(&Lua, A) -> mlua::Result<R> + mlua::MaybeSend + 'static,
{
    lua.create_function(move |lua, a: A| tried(lua, f(lua, a)))
}

/// `f`'s result as a `try_` form answers it (see `try_fn`): for a method.
pub fn tried(
    lua: &Lua,
    r: mlua::Result<impl mlua::IntoLuaMulti>,
) -> mlua::Result<mlua::MultiValue> {
    match r {
        Ok(v) => {
            let mut out = v.into_lua_multi(lua)?;
            out.push_front(mlua::Value::Boolean(true));
            Ok(out)
        }
        Err(e) => mlua::IntoLuaMulti::into_lua_multi((false, message(&e)), lua),
    }
}

fn message(e: &mlua::Error) -> String {
    match e {
        mlua::Error::CallbackError { cause, .. } => message(cause),
        mlua::Error::RuntimeError(m) => m.clone(),
        other => other.to_string(),
    }
}

pub fn install(lua: &Lua, shared: Arc<Shared>) -> mlua::Result<()> {
    lua.set_app_data(shared.clone());
    let kui = lua.create_table()?;
    kui.set(
        "__block",
        lua.create_function(|lua, id: u64| {
            let s = shared_of(lua);
            Ok(answer_to_lua(lua, s.ops.block_on(id)))
        })?,
    )?;
    kui.set(
        "__poke",
        lua.create_function(|lua, ()| {
            shared_of(lua).ops.poke();
            Ok(())
        })?,
    )?;
    kui.set(
        "__task_error",
        lua.create_function(|lua, msg: String| {
            let s = shared_of(lua);
            s.error(msg);
            if s.worker.is_none() && TASK_ERROR_EXITS.load(std::sync::atomic::Ordering::Relaxed) {
                s.exit.lock().unwrap().get_or_insert(1);
                s.ops.poke();
            }
            Ok(())
        })?,
    )?;
    // The message alone of an error `try_transaction` caught from the
    // app's function, where a Rust call's error arrives as "runtime
    // error: …" with a traceback.
    kui.set(
        "__message",
        lua.create_function(|_, v: mlua::Value| {
            Ok(match v {
                mlua::Value::Error(e) => message(&e),
                mlua::Value::String(s) => s.to_string_lossy(),
                other => other.to_string()?,
            })
        })?,
    )?;
    kui.set(
        "exit",
        lua.create_function(|lua, code: Option<i32>| {
            *shared_of(lua).exit.lock().unwrap() = Some(code.unwrap_or(0));
            shared_of(lua).ops.poke();
            Ok(())
        })?,
    )?;
    kui.set(
        "now",
        lua.create_function(|_, ()| {
            Ok(std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_secs_f64() * 1000.0)
                .unwrap_or(0.0))
        })?,
    )?;
    kui.set("dir", shared.dir.to_string_lossy().as_ref())?;
    kui.set("version", env!("CARGO_PKG_VERSION"))?;
    kui.set("args", ARGS.get().cloned().unwrap_or_default())?;
    if let Some(name) = NAME.get() {
        kui.set("name", name.as_str())?;
    }
    // Spelled as `dir` is (`paths::canonical`), so the two compare: on
    // Windows `current_dir` keeps a short name (`RUNNER~1`) the app's
    // folder has expanded.
    if let Ok(cwd) = std::env::current_dir() {
        let cwd = crate::paths::canonical(&cwd).unwrap_or(cwd);
        kui.set("cwd", cwd.to_string_lossy().as_ref())?;
    }
    kui.set("platform", PLATFORM)?;
    if let Some(app) = APP.get() {
        kui.set("id", app.id.as_str())?;
        if let Some(v) = &app.version {
            kui.set("app_version", v.as_str())?;
        }
        kui.set("data_dir", app.data_dir.to_string_lossy().as_ref())?;
        kui.set("cache_dir", app.cache_dir.to_string_lossy().as_ref())?;
        if let Some((runner, bytecode, hash)) = &app.packed {
            let t = lua.create_table()?;
            t.set("runner", runner.as_str())?;
            t.set("bytecode", *bytecode)?;
            t.set("hash", hash.as_str())?;
            kui.set("packed", t)?;
        }
    }
    kui.set(
        "cpus",
        std::thread::available_parallelism().map_or(1, |n| n.get()),
    )?;
    if shared.worker.is_some() {
        kui.set("__worker", true)?;
    }
    // `kui.devtools(on)`: the panel on, off, or (with nothing) toggled;
    // `kui.devtools("log")` on, at that tab.
    kui.set(
        "devtools",
        lua.create_function(|lua, on: mlua::Value| {
            let s = shared_of(lua);
            let ask = match on {
                mlua::Value::Boolean(b) => (Some(b), None),
                mlua::Value::String(t) => (Some(true), Some(t.to_str()?.to_string())),
                _ => (None, None),
            };
            *s.devtools.lock().unwrap() = Some(ask);
            s.ops.poke();
            Ok(())
        })?,
    )?;
    install_env(lua, &kui)?;
    log::install(lua, &kui)?;
    formats::install(lua, &kui)?;
    fs::install(lua, &kui)?;
    timer::install(lua, &kui)?;
    process::install(lua, &kui)?;
    http::install(lua, &kui)?;
    input::install(lua, &kui)?;
    sqlite::install(lua, &kui)?;
    worker::install(lua, &kui)?;
    lua.globals().set("kui", kui.clone())?;
    let loaded: Table = lua.globals().get::<Table>("package")?.get("loaded")?;
    loaded.set("kui", kui)?;
    lua.load(RUNTIME).set_name("@kui:runtime").exec()?;
    lua.load(ARGS_LUA).set_name("@kui:args").exec()
}

fn shared_of(lua: &Lua) -> Arc<Shared> {
    shared(lua)
}

/// An operation's answer as the `(ok, value)` pair `kui.__await` reads.
pub fn answer_to_lua(lua: &Lua, a: crate::ops::Answer) -> (bool, mlua::Value) {
    match a {
        Ok(v) => match kui_lua::value_to_lua(lua, &v) {
            Ok(v) => (true, v),
            Err(e) => (
                false,
                mlua::Value::String(lua.create_string(e.to_string()).unwrap()),
            ),
        },
        Err(e) => (false, mlua::Value::String(lua.create_string(e).unwrap())),
    }
}

/// `kui.env`: the process environment as a table, read once (after the
/// `.env` files, which the runner loads before the app runs), and
/// `kui.getenv(name)` for a live read.
fn install_env(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let env = lua.create_table()?;
    for (k, v) in std::env::vars() {
        env.set(k, v)?;
    }
    kui.set("env", env)?;
    kui.set(
        "getenv",
        lua.create_function(|_, name: String| Ok(std::env::var(name).ok()))?,
    )?;
    Ok(())
}

/// Runs everything the state's inbox holds through its Lua half: answers
/// resume their tasks, requests start their handlers, workers' messages
/// reach their handlers, then the dispatched messages reach `update`.
/// Returns whether anything ran.
pub fn drain(lua: &Lua) -> mlua::Result<bool> {
    let s = shared(lua);
    let kui: Table = lua.globals().get("kui")?;
    let resume: mlua::Function = kui.get("__resume")?;
    let request: mlua::Function = kui.get("__request")?;
    let from_worker: mlua::Function = kui.get("__from_worker")?;
    let worker_exit: mlua::Function = kui.get("__worker_exit")?;
    let dispatch_drain: mlua::Function = kui.get("__drain")?;
    // A dispatch queued while this runs is delivered by it (`Ops::poke`).
    let _draining = s.ops.draining();
    let mut any = false;
    for _ in 0..64 {
        let (answers, inbound) = s.ops.take();
        let fresh = !answers.is_empty() || !inbound.is_empty();
        for (id, a) in answers {
            let (ok, v) = answer_to_lua(lua, a);
            resume.call::<()>((id, ok, v))?;
        }
        for msg in inbound {
            match msg {
                crate::ops::Inbound::Dispatch { from: Some(w), msg } => {
                    from_worker.call::<()>((w, kui_lua::value_to_lua(lua, &msg)?))?;
                }
                crate::ops::Inbound::Dispatch { from: None, msg } => {
                    let dispatch: mlua::Function = kui.get("dispatch")?;
                    dispatch.call::<()>(kui_lua::value_to_lua(lua, &msg)?)?;
                }
                crate::ops::Inbound::Request { server, id, req } => {
                    request.call::<()>((server, id, kui_lua::value_to_lua(lua, &req)?))?;
                }
                crate::ops::Inbound::WorkerExit { id, error } => {
                    s.workers.lock().unwrap().remove(&id);
                    worker_exit.call::<()>((id, error))?;
                }
                crate::ops::Inbound::Message(msg) => {
                    if let Ok(f) = lua.globals().get::<mlua::Function>("on_message") {
                        let v = kui_lua::value_to_lua(lua, &msg)?;
                        if let Err(e) = f.call::<()>(v) {
                            s.error(format!("on_message: {e}"));
                        }
                    }
                }
            }
        }
        let delivered: u64 = dispatch_drain.call(())?;
        any |= fresh || delivered > 0;
        if !fresh {
            break;
        }
    }
    Ok(any)
}
