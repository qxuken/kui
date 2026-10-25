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
        })
    }

    /// A path the app named, against its folder when relative.
    pub fn path(&self, p: &str) -> PathBuf {
        let p = std::path::Path::new(p);
        if p.is_absolute() { p.to_path_buf() } else { self.dir.join(p) }
    }

    pub fn error(&self, msg: String) {
        eprintln!("kui: {msg}");
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
    let _ = std::thread::Builder::new().name("kui-signals".into()).spawn(move || {
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

/// Installs `kui` and every module into `lua`, for the app (`ui`) or a
/// worker.
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
            shared_of(lua).error(msg);
            Ok(())
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
    if let Ok(cwd) = std::env::current_dir() {
        kui.set("cwd", cwd.to_string_lossy().as_ref())?;
    }
    kui.set("platform", PLATFORM)?;
    kui.set("cpus", std::thread::available_parallelism().map_or(1, |n| n.get()))?;
    if shared.worker.is_some() {
        kui.set("__worker", true)?;
    }
    install_env(lua, &kui)?;
    formats::install(lua, &kui)?;
    fs::install(lua, &kui)?;
    timer::install(lua, &kui)?;
    process::install(lua, &kui)?;
    http::install(lua, &kui)?;
    sqlite::install(lua, &kui)?;
    worker::install(lua, &kui)?;
    lua.globals().set("kui", kui.clone())?;
    let loaded: Table = lua.globals().get::<Table>("package")?.get("loaded")?;
    loaded.set("kui", kui)?;
    lua.load(RUNTIME).set_name("@kui:runtime").exec()
}

fn shared_of(lua: &Lua) -> Arc<Shared> {
    shared(lua)
}

/// An operation's answer as the `(ok, value)` pair `kui.__await` reads.
pub fn answer_to_lua(lua: &Lua, a: crate::ops::Answer) -> (bool, mlua::Value) {
    match a {
        Ok(v) => match kui_lua::value_to_lua(lua, &v) {
            Ok(v) => (true, v),
            Err(e) => (false, mlua::Value::String(lua.create_string(e.to_string()).unwrap())),
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
        dispatch_drain.call::<()>(())?;
        any |= fresh;
        if !fresh {
            break;
        }
    }
    Ok(any)
}
