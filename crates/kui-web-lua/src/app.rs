//! Lua facilities supplied by the browser's WASI filesystem.
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};

#[path = "../../kui-cli/src/kit.rs"]
pub mod kit;

const ARGS: &str = include_str!("../../kui-cli/src/lua/args.lua");

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
        lua.create_function(|lua, path: String| match std::fs::read(app_path(&path)) {
            Ok(bytes) => Ok((true, lua.create_string(bytes)?)),
            Err(error) => Ok((false, lua.create_string(error.to_string())?)),
        })?,
    )?;
    fs.set(
        "write",
        lua.create_function(|_, (path, bytes): (String, mlua::LuaString)| {
            std::fs::write(app_path(&path), bytes.as_bytes()).map_err(mlua::Error::external)
        })?,
    )?;
    fs.set(
        "mkdir",
        lua.create_function(|_, path: String| {
            std::fs::create_dir_all(app_path(&path)).map_err(mlua::Error::external)
        })?,
    )?;
    fs.set(
        "remove",
        lua.create_function(|_, path: String| {
            std::fs::remove_file(app_path(&path)).map_err(mlua::Error::external)
        })?,
    )?;
    let json = lua.create_table()?;
    json.set(
        "try_decode",
        lua.create_function(|lua, text: String| {
            match serde_json::from_str::<serde_json::Value>(&text) {
                Ok(value) => Ok((true, lua.to_value(&value)?)),
                Err(error) => Ok((false, Value::String(lua.create_string(error.to_string())?))),
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
        lua.create_function(|lua, table: Table| {
            table.set_metatable(Some(lua.array_metatable()))?;
            Ok(table)
        })?,
    )?;
    kui.set("args", lua.create_table()?)?;
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
    kui.set("dir", "/app")?;
    kui.set("data_dir", "/data")?;
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
    package.set("path", format!("/app/?.lua;/app/?/init.lua;{path}"))?;
    package.set("cpath", "")?;
    package.get::<Table>("searchers")?.set(3, Value::Nil)?;
    package.get::<Table>("searchers")?.set(4, Value::Nil)?;
    lua.load(ARGS).set_name("@kui:args").exec()?;
    lua.load(include_str!("app.lua"))
        .set_name("@kui:web")
        .exec()
}

fn app_path(path: &str) -> std::path::PathBuf {
    std::path::Path::new("/app").join(path)
}
