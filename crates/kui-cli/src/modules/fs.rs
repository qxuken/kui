//! `kui.fs`: files, read and written at once (a local disk answers faster
//! than a frame). Relative paths are the app's folder's.

use mlua::{Lua, Table};

use super::{fail, shared};

fn io(path: &std::path::Path, e: std::io::Error) -> mlua::Error {
    fail(format!("{}: {e}", path.display()))
}

/// A path the app may write: anywhere but a packed app's own files, which
/// are read-only (ADR 0068, decision 18).
fn writable(lua: &Lua, path: &str) -> mlua::Result<std::path::PathBuf> {
    let p = shared(lua).path(path);
    if let Some(dir) = crate::packed::dir()
        && p.starts_with(dir)
    {
        return Err(fail(format!(
            "{path}: a packed app's own files are read-only — write under kui.data_dir ({})",
            crate::modules::app()
                .map(|a| a.data_dir.display().to_string())
                .unwrap_or_default()
        )));
    }
    Ok(p)
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
            let p = writable(lua, &path)?;
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
            }
            std::fs::write(&p, &*data.as_bytes()).map_err(|e| io(&p, e))
        })?,
    )?;
    // Writes a file that does not exist yet: false, and nothing written,
    // when it does — a lock two runs started at once cannot share.
    fs.set(
        "create",
        lua.create_function(|lua, (path, data): (String, mlua::LuaString)| {
            use std::io::Write;
            let p = writable(lua, &path)?;
            if let Some(dir) = p.parent() {
                std::fs::create_dir_all(dir).map_err(|e| io(dir, e))?;
            }
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&p)
            {
                Ok(mut f) => f
                    .write_all(&data.as_bytes())
                    .map(|()| true)
                    .map_err(|e| io(&p, e)),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => Ok(false),
                Err(e) => Err(io(&p, e)),
            }
        })?,
    )?;
    fs.set(
        "append",
        lua.create_function(|lua, (path, data): (String, mlua::LuaString)| {
            use std::io::Write;
            let p = writable(lua, &path)?;
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
            let p = writable(lua, &path)?;
            std::fs::create_dir_all(&p).map_err(|e| io(&p, e))
        })?,
    )?;
    fs.set(
        "remove",
        lua.create_function(|lua, path: String| {
            let p = writable(lua, &path)?;
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
            let (a, b) = (writable(lua, &from)?, writable(lua, &to)?);
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
    // Waiting calls, off the frame: a folder's tree and a batch of files,
    // one hop each, where a task reading hundreds of files one `read` at a
    // time held the frame between them.
    fs.set(
        "__walk",
        lua.create_function(|lua, (dir, opts): (Option<String>, Option<Table>)| {
            let s = shared(lua);
            let root = s.path(dir.as_deref().unwrap_or("."));
            let mut skip: Vec<String> = vec![".git".into(), "node_modules".into()];
            let mut pattern: Option<String> = None;
            let mut dirs = false;
            if let Some(o) = opts {
                if let Some(list) = o.get::<Option<Vec<String>>>("skip")? {
                    skip = list;
                }
                pattern = o.get::<Option<String>>("match")?;
                dirs = o.get::<Option<bool>>("dirs")?.unwrap_or(false);
            }
            let id = s.ops.id();
            crate::ops::run(&s.ops, id, move || {
                let mut out = Vec::new();
                walk(&root, &root, &skip, pattern.as_deref(), dirs, &mut out)
                    .map_err(|e| format!("{}: {e}", root.display()))?;
                out.sort();
                Ok(kui_core::Value::List(
                    out.into_iter().map(kui_core::Value::Str).collect(),
                ))
            });
            Ok(id)
        })?,
    )?;
    fs.set(
        "__read_all",
        lua.create_function(|lua, paths: Vec<String>| {
            let s = shared(lua);
            let full: Vec<std::path::PathBuf> = paths.iter().map(|p| s.path(p)).collect();
            let id = s.ops.id();
            crate::ops::run(&s.ops, id, move || {
                Ok(kui_core::Value::List(
                    full.iter()
                        .map(|p| match std::fs::read(p) {
                            Ok(b) => kui_core::Value::Str(String::from_utf8_lossy(&b).into_owned()),
                            Err(_) => kui_core::Value::Bool(false),
                        })
                        .collect(),
                ))
            });
            Ok(id)
        })?,
    )?;
    kui.set("fs", fs)?;
    Ok(())
}

/// The files under `dir` (folders too with `dirs`), as paths relative to
/// `root` with `/` between names, past the names in `skip`, kept when
/// `pattern` matches the path's end (`*.json`, `**/src/*.ts`).
fn walk(
    root: &std::path::Path,
    dir: &std::path::Path,
    skip: &[String],
    pattern: Option<&str>,
    dirs: bool,
    out: &mut Vec<String>,
) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)?.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        if skip.iter().any(|s| *s == name) {
            continue;
        }
        let path = e.path();
        let rel = path
            .strip_prefix(root)
            .unwrap_or(&path)
            .components()
            .map(|c| c.as_os_str().to_string_lossy())
            .collect::<Vec<_>>()
            .join("/");
        let is_dir = e.file_type().is_ok_and(|t| t.is_dir());
        if (!is_dir || dirs) && pattern.is_none_or(|p| crate::project::glob(p, &rel)) {
            out.push(rel);
        }
        if is_dir {
            // A folder that cannot be read is passed over, not the walk's end.
            let _ = walk(root, &path, skip, pattern, dirs, out);
        }
    }
    Ok(())
}
