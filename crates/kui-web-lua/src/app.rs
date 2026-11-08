//! Lua facilities supplied by the browser's WASI filesystem.
use std::{io, path::Path};

use mlua::{Function, IntoLuaMulti, Lua, LuaSerdeExt, MultiValue, Table, Value};

#[path = "../../kui-cli/src/kit.rs"]
pub mod kit;

const ARGS: &str = include_str!("../../kui-cli/src/lua/args.lua");
pub const APP_DIR: &str = "/app";
const DATA_DIR: &str = "/data";

pub fn initialize(app: &kui_lua::LuaExtension) -> mlua::Result<Function> {
    let globals = app.lua().globals();
    if let Some(init) = globals.get::<Option<Function>>("init")? {
        globals.set("model", init.call::<Value>(())?)?;
    }
    globals.get("__web_run_tasks")
}

pub fn install(lua: &Lua, name: &str, id: &str, version: &str) -> mlua::Result<()> {
    kit::install_ui(lua)?;
    let kui = lua.create_table()?;
    let fs = lua.create_table()?;
    fs.set(
        "exists",
        lua.create_function(|_, path: String| Ok(app_path(&path).exists()))?,
    )?;
    fs.set(
        "try_read",
        lua.create_function(|lua, path: String| {
            let path = app_path(&path);
            match std::fs::read(&path) {
                Ok(bytes) => (true, lua.create_string(bytes)?).into_lua_multi(lua),
                Err(error) => file_failure(lua, &path, error),
            }
        })?,
    )?;
    fs.set(
        "try_write",
        lua.create_function(|lua, (path, bytes): (String, mlua::LuaString)| {
            modify_file(lua, &path, |path| {
                if let Some(dir) = path.parent() {
                    std::fs::create_dir_all(dir)?;
                }
                std::fs::write(path, bytes.as_bytes())
            })
        })?,
    )?;
    fs.set(
        "try_mkdir",
        lua.create_function(|lua, path: String| {
            modify_file(lua, &path, |path| std::fs::create_dir_all(path))
        })?,
    )?;
    fs.set(
        "try_remove",
        lua.create_function(|lua, path: String| {
            modify_file(lua, &path, |path| {
                if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                }
            })
        })?,
    )?;
    let json = lua.create_table()?;
    json.set(
        "try_decode",
        lua.create_function(|lua, text: String| {
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) => (true, lua.to_value(&value)?).into_lua_multi(lua),
                Err(error) => (false, error.to_string(), "parse").into_lua_multi(lua),
            }
        })?,
    )?;
    json.set(
        "encode",
        lua.create_function(|lua, value: Value| {
            let value: serde_json::Value = lua.from_value(value)?;
            serde_json::to_string(&value).map_err(mlua::Error::external)
        })?,
    )?;
    json.set(
        "array",
        lua.create_function(|lua, table: Option<Table>| {
            let table = match table {
                Some(table) => table,
                None => lua.create_table()?,
            };
            table.set_metatable(Some(lua.array_metatable()))?;
            Ok(table)
        })?,
    )?;
    kui.set("args", lua.create_table()?)?;
    kui.set("env", lua.create_table()?)?;
    kui.set(
        "__task_error",
        lua.create_function(|_, message: String| {
            eprintln!("{message}");
            Ok(())
        })?,
    )?;
    let started = std::time::Instant::now();
    kui.set(
        "now",
        lua.create_function(move |_, ()| Ok(started.elapsed().as_secs_f64() * 1000.0))?,
    )?;
    kui.set("name", name)?;
    kui.set("id", id)?;
    kui.set("app_version", version)?;
    kui.set("version", env!("CARGO_PKG_VERSION"))?;
    kui.set("platform", "web")?;
    kui.set("dir", APP_DIR)?;
    kui.set("data_dir", DATA_DIR)?;
    kui.set("cache_dir", "/cache")?;
    let packed = lua.create_table()?;
    packed.set("runner", env!("CARGO_PKG_VERSION"))?;
    packed.set("bytecode", false)?;
    kui.set("packed", packed)?;
    kui.set("fs", fs)?;
    kui.set("json", json)?;
    lua.globals().set("kui", kui.clone())?;
    let package: Table = lua.globals().get("package")?;
    package.get::<Table>("loaded")?.set("kui", kui)?;
    let path: String = package.get("path")?;
    package.set(
        "path",
        format!("{APP_DIR}/?.lua;{APP_DIR}/?/init.lua;{path}"),
    )?;
    package.set("cpath", "")?;
    package.get::<Table>("searchers")?.set(3, Value::Nil)?;
    package.get::<Table>("searchers")?.set(4, Value::Nil)?;
    lua.load(ARGS).set_name("@kui:args").exec()?;
    lua.load(include_str!("app.lua"))
        .set_name("@kui:web")
        .exec()
}

fn app_path(path: &str) -> std::path::PathBuf {
    Path::new(APP_DIR).join(path)
}

fn file_failure(lua: &Lua, path: &Path, error: io::Error) -> mlua::Result<MultiValue> {
    use io::ErrorKind as K;
    let code = match error.kind() {
        K::NotFound => "not_found",
        K::PermissionDenied | K::ReadOnlyFilesystem => "denied",
        K::AlreadyExists => "exists",
        K::DirectoryNotEmpty => "not_empty",
        K::IsADirectory | K::NotADirectory | K::InvalidInput | K::InvalidFilename => "invalid",
        K::InvalidData => "encoding",
        K::TimedOut => "timeout",
        K::StorageFull | K::QuotaExceeded | K::FileTooLarge => "full",
        _ => "failed",
    };
    (false, format!("{}: {error}", path.display()), code).into_lua_multi(lua)
}

fn modify_file(
    lua: &Lua,
    path: &str,
    modify: impl FnOnce(&Path) -> io::Result<()>,
) -> mlua::Result<MultiValue> {
    let file = app_path(path);
    if file.starts_with(APP_DIR) {
        let message = format!(
            "{path}: a packed app's own files are read-only — write under kui.data_dir ({DATA_DIR})"
        );
        return (false, message, "denied").into_lua_multi(lua);
    }
    match modify(&file) {
        Ok(()) => true.into_lua_multi(lua),
        Err(error) => file_failure(lua, &file, error),
    }
}
