//! `kui.worker(path)` (ADR 0064, decision 7): a module run in a Lua state
//! of its own on a thread of its own, with every module but the UI. The
//! app's `w:dispatch(msg)` reaches the worker's `on_message`; the worker's
//! `kui.dispatch(msg)` reaches the app. Messages cross as plain values; a
//! channel handle inside one crosses without its rows (ADR 0065).

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use mlua::{Lua, Table};

use crate::ops::{Inbound, Ops};

use super::{Shared, fail, shared};

/// What the app holds of a worker it started.
pub struct Handle {
    ops: Ops,
    stop: Arc<AtomicBool>,
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    kui.set(
        "__worker_start",
        lua.create_function(|lua, path: String| {
            let app = shared(lua);
            let file = app.path(&path);
            let file = if file.extension().is_none() && !file.exists() {
                file.with_extension("lua")
            } else {
                file
            };
            if !file.exists() {
                return Err(fail(format!("kui.worker: no file {}", file.display())));
            }
            let id = app.ops.id();
            let dir = file
                .parent()
                .map(|d| d.to_path_buf())
                .unwrap_or_else(|| app.dir.clone());
            let inner = Shared::new(dir, Some((id, app.ops.clone())));
            let stop = Arc::new(AtomicBool::new(false));
            app.workers.lock().unwrap().insert(
                id,
                Handle {
                    ops: inner.ops.clone(),
                    stop: stop.clone(),
                },
            );
            let to_app = app.ops.clone();
            std::thread::Builder::new()
                .name(format!(
                    "kui-worker-{}",
                    file.file_stem().unwrap_or_default().to_string_lossy()
                ))
                .spawn(move || {
                    let error = run(&file, inner, &stop).err();
                    to_app.send(Inbound::WorkerExit { id, error });
                })
                .map_err(|e| fail(format!("kui.worker: {e}")))?;
            Ok(id)
        })?,
    )?;
    kui.set(
        "__worker_send",
        lua.create_function(|lua, (id, msg): (u64, mlua::Value)| {
            let msg = kui_lua::lua_to_value(&msg)?;
            match shared(lua).workers.lock().unwrap().get(&id) {
                Some(w) => w.ops.send(Inbound::Message(msg)),
                None => return Err(fail("the worker has stopped".into())),
            }
            Ok(())
        })?,
    )?;
    kui.set(
        "__worker_stop",
        lua.create_function(|lua, id: u64| {
            if let Some(w) = shared(lua).workers.lock().unwrap().remove(&id) {
                w.stop.store(true, Ordering::Relaxed);
                w.ops.poke();
            }
            Ok(())
        })?,
    )?;
    kui.set(
        "__to_app",
        lua.create_function(|lua, msg: mlua::Value| {
            let s = shared(lua);
            let Some((id, app)) = &s.worker else {
                return Err(fail("kui.__to_app outside a worker".into()));
            };
            app.send(Inbound::Dispatch {
                from: Some(*id),
                msg: kui_lua::lua_to_value(&msg)?,
            });
            Ok(())
        })?,
    )?;
    Ok(())
}

/// The worker's life: its state built, its module run, then its inbox
/// drained until it is stopped, or has nothing left to do — no
/// `on_message` to wait for and no task waiting.
fn run(file: &std::path::Path, shared: Arc<Shared>, stop: &AtomicBool) -> Result<(), String> {
    let lua = Lua::new();
    let setup = || -> mlua::Result<()> {
        kui_lua::install_data(&lua)?;
        crate::project::add_path(&lua, &shared.dir)?;
        super::install_loader(&lua)?;
        crate::teal::install(&lua)?;
        super::install(&lua, shared.clone())?;
        let source =
            std::fs::read_to_string(file).map_err(|e| fail(format!("{}: {e}", file.display())))?;
        let source = crate::teal::source_for(file, &source)?;
        lua.load(&source)
            .set_name(format!("@{}", file.display()))
            .exec()
    };
    setup().map_err(|e| e.to_string())?;
    let listens = lua
        .globals()
        .get::<mlua::Value>("on_message")
        .map(|v| !v.is_nil())
        .unwrap_or(false);
    let pending = |lua: &Lua| -> usize {
        lua.globals()
            .get::<Table>("kui")
            .and_then(|k| k.get::<mlua::Function>("pending"))
            .and_then(|f| f.call::<usize>(()))
            .unwrap_or(0)
    };
    loop {
        super::drain(&lua).map_err(|e| e.to_string())?;
        if stop.load(Ordering::Relaxed) || shared.exit.lock().unwrap().is_some() {
            return Ok(());
        }
        if !listens && pending(&lua) == 0 {
            return Ok(());
        }
        shared.ops.wait_any(Duration::from_millis(250));
    }
}
