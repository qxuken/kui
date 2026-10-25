//! `kui.fs`: files, read and written at once (a local disk answers faster
//! than a frame). Relative paths are the app's folder's.

use mlua::{Lua, Table};

use super::{fail, shared};

fn io(path: &std::path::Path, e: std::io::Error) -> mlua::Error {
    fail(format!("{}: {e}", path.display()))
}

pub fn install(lua: &Lua, kui: &Table) -> mlua::Result<()> {
    let fs = lua.create_table()?;
    fs.set(
        "read",
        lua.create_function(|lua, path: String| {
            let p = shared(lua).path(&path);
            let bytes = std::fs::read(&p).map_err(|e| io(&p, e))?;
            lua.create_string(bytes)
        })?,
    )?;
    fs.set(
        "write",
        lua.create_function(|lua, (path, data): (String, mlua::LuaString)| {
            let p = shared(lua).path(&path);
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
            }
            std::fs::write(&p, &*data.as_bytes()).map_err(|e| io(&p, e))
        })?,
    )?;
    fs.set(
        "append",
        lua.create_function(|lua, (path, data): (String, mlua::LuaString)| {
            use std::io::Write;
            let p = shared(lua).path(&path);
            let mut f = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&p)
                .map_err(|e| io(&p, e))?;
            f.write_all(&data.as_bytes()).map_err(|e| io(&p, e))
        })?,
    )?;
    fs.set(
        "exists",
        lua.create_function(|lua, path: String| Ok(shared(lua).path(&path).exists()))?,
    )?;
    fs.set(
        "list",
        lua.create_function(|lua, path: Option<String>| {
            let p = shared(lua).path(path.as_deref().unwrap_or("."));
            let mut names: Vec<String> = std::fs::read_dir(&p)
                .map_err(|e| io(&p, e))?
                .filter_map(|e| e.ok())
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            lua.create_sequence_from(names)
        })?,
    )?;
    fs.set(
        "stat",
        lua.create_function(|lua, path: String| {
            let p = shared(lua).path(&path);
            let Ok(m) = std::fs::metadata(&p) else {
                return Ok(mlua::Value::Nil);
            };
            let t = lua.create_table()?;
            t.set("size", m.len())?;
            t.set("dir", m.is_dir())?;
            t.set("file", m.is_file())?;
            if let Ok(modified) = m.modified()
                && let Ok(d) = modified.duration_since(std::time::UNIX_EPOCH)
            {
                t.set("modified", d.as_secs_f64() * 1000.0)?;
            }
            Ok(mlua::Value::Table(t))
        })?,
    )?;
    fs.set(
        "mkdir",
        lua.create_function(|lua, path: String| {
            let p = shared(lua).path(&path);
            std::fs::create_dir_all(&p).map_err(|e| io(&p, e))
        })?,
    )?;
    fs.set(
        "remove",
        lua.create_function(|lua, path: String| {
            let p = shared(lua).path(&path);
            if p.is_dir() {
                std::fs::remove_dir_all(&p).map_err(|e| io(&p, e))
            } else {
                std::fs::remove_file(&p).map_err(|e| io(&p, e))
            }
        })?,
    )?;
    fs.set(
        "rename",
        lua.create_function(|lua, (from, to): (String, String)| {
            let s = shared(lua);
            let (a, b) = (s.path(&from), s.path(&to));
            std::fs::rename(&a, &b).map_err(|e| io(&a, e))
        })?,
    )?;
    fs.set(
        "lines",
        lua.create_function(|lua, path: String| {
            let p = shared(lua).path(&path);
            let text = std::fs::read_to_string(&p).map_err(|e| io(&p, e))?;
            lua.create_sequence_from(text.lines().map(str::to_string).collect::<Vec<_>>())
        })?,
    )?;
    kui.set("fs", fs)?;
    Ok(())
}
