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

use super::{Shared, fail, shared, try_fn};

/// What the app holds of a worker it started.
pub struct Handle {
    ops: Ops,
    stop: Arc<AtomicBool>,
    join: Option<std::thread::JoinHandle<()>>,
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    kui.set(
        "__try_worker_start",
        try_fn(lua, |lua, path: String| {
            let app = shared(lua);
            let file = app.path(&path);
            let file = if file.extension().is_none() && !file.exists() {
                file.with_extension("lua")
            } else {
                file
            };
            if !file.exists() {
                return Err(super::fail_as(
                    "not_found",
                    format!("kui.worker: no file {}", file.display()),
                ));
            }
            let id = app.ops.id();
            let dir = file
                .parent()
                .map(|d| d.to_path_buf())
                .unwrap_or_else(|| app.dir.clone());
            let inner = Shared::in_run(dir, Some((id, app.ops.clone())), app.run.clone());
            let stop = Arc::new(AtomicBool::new(false));
            app.workers.lock().unwrap().insert(
                id,
                Handle {
                    ops: inner.ops.clone(),
                    stop: stop.clone(),
                    join: None,
                },
            );
            let to_app = app.ops.clone();
            let join = std::thread::Builder::new()
                .name(format!(
                    "kui-worker-{}",
                    file.file_stem().unwrap_or_default().to_string_lossy()
                ))
                .spawn(move || {
                    let result = run(&file, inner, &stop);
                    let error = if stop.load(Ordering::Relaxed) {
                        None
                    } else {
                        result.err()
                    };
                    to_app.send(Inbound::WorkerExit { id, error });
                })
                .map_err(|e| fail(format!("kui.worker: {e}")))?;
            if let Some(handle) = app.workers.lock().unwrap().get_mut(&id) {
                handle.join = Some(join);
            }
            Ok(id)
        })?,
    )?;
    kui.set(
        "__try_worker_send",
        try_fn(lua, |lua, (id, msg): (u64, mlua::Value)| {
            let msg = kui_lua::lua_to_value(&msg)?;
            match shared(lua).workers.lock().unwrap().get(&id) {
                Some(w) => w.ops.send(Inbound::Message(msg)),
                None => return Err(super::fail_as("closed", "the worker has stopped")),
            }
            Ok(())
        })?,
    )?;
    kui.set(
        "__worker_stop",
        lua.create_function(|lua, id: u64| {
            if let Some(w) = shared(lua).workers.lock().unwrap().remove(&id) {
                w.stop.store(true, Ordering::Relaxed);
                w.ops.close();
                if let Some(join) = w.join {
                    let _ = join.join();
                }
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
fn run(file: &std::path::Path, shared: Arc<Shared>, stop: &Arc<AtomicBool>) -> Result<(), String> {
    struct Descendants<'a>(&'a Shared);
    impl Drop for Descendants<'_> {
        fn drop(&mut self) {
            stop_all(self.0);
        }
    }
    let _descendants = Descendants(&shared);
    let lua = Lua::new();
    let stop_hook = stop.clone();
    lua.set_global_hook(
        mlua::HookTriggers {
            every_nth_instruction: Some(10000),
            ..Default::default()
        },
        move |_, _| {
            if stop_hook.load(Ordering::Relaxed) {
                Err(mlua::Error::runtime("worker stopped"))
            } else {
                Ok(mlua::VmState::Continue)
            }
        },
    )
    .map_err(|e| e.to_string())?;
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
    lua.globals()
        .get::<Table>("kui")
        .and_then(|k| k.get::<mlua::Function>("__instrument"))
        .and_then(|f| f.call::<()>(()))
        .map_err(|e| e.to_string())?;
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

/// Stop and join owned workers before a case's resources disappear.
pub fn stop_all(shared: &Shared) {
    let workers = shared
        .workers
        .lock()
        .unwrap()
        .drain()
        .map(|(_, w)| w)
        .collect::<Vec<_>>();
    for w in &workers {
        w.stop.store(true, Ordering::Relaxed);
        w.ops.close();
    }
    // Run shutdown cancels shared clients; a worker's normal exit must not
    // close clients still used by its app and siblings.
    if shared.run.closed.load(Ordering::Relaxed) {
        super::http::shutdown(&shared.run.http);
        for client in shared.run.clients.lock().unwrap().iter() {
            super::http::shutdown(client);
        }
    }
    for w in workers {
        if let Some(join) = w.join {
            let _ = join.join();
        }
        if shared.run.testing && !w.ops.wait_idle(Duration::from_secs(5)) {
            shared.error("worker operations did not stop during teardown".into());
        }
    }
}
