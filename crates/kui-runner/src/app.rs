//! Lua facilities for the embedded native and browser hosts.
use std::{
    io,
    path::{Path, PathBuf},
};

use mlua::{Function, IntoLuaMulti, Lua, LuaSerdeExt, MultiValue, Table, Value};

use crate::kit;

const ARGS: &str = include_str!("../../kui-cli/src/lua/args.lua");
pub const WEB_APP_DIR: &str = "/app";

pub(crate) struct Metadata<'a> {
    pub name: &'a str,
    pub id: &'a str,
    pub version: &'a str,
    pub platform: &'a str,
}

#[derive(Clone)]
pub(crate) struct Paths {
    pub app: PathBuf,
    data: PathBuf,
    cache: PathBuf,
}

impl Paths {
    pub fn new(
        app: impl Into<PathBuf>,
        data: impl Into<PathBuf>,
        cache: impl Into<PathBuf>,
    ) -> Self {
        Self {
            app: app.into(),
            data: data.into(),
            cache: cache.into(),
        }
    }

    fn resolve(&self, path: &str) -> io::Result<PathBuf> {
        let relative = Path::new(path);
        let full = self.app.join(relative);
        // WASI already confines and normalizes paths through its virtual mount.
        if cfg!(not(target_os = "wasi"))
            && (relative
                .components()
                .any(|part| part == std::path::Component::ParentDir)
                || ![&self.app, &self.data, &self.cache]
                    .iter()
                    .any(|root| full.starts_with(root)))
        {
            return Err(io::Error::new(
                io::ErrorKind::PermissionDenied,
                "path is outside the packed app",
            ));
        }
        Ok(full)
    }
}

pub fn initialize(app: &kui_lua::LuaExtension) -> mlua::Result<Function> {
    let globals = app.lua().globals();
    if let Some(init) = globals.get::<Option<Function>>("init")? {
        globals.set("model", init.call::<Value>(())?)?;
    }
    globals.get("__kui_run_tasks")
}

pub fn install_web(lua: &Lua, name: &str, id: &str, version: &str) -> mlua::Result<()> {
    install(
        lua,
        Metadata {
            name,
            id,
            version,
            platform: "web",
        },
        &Paths::new(WEB_APP_DIR, "/data", "/cache"),
    )
}

pub(crate) fn install(lua: &Lua, app: Metadata<'_>, paths: &Paths) -> mlua::Result<()> {
    kit::install_ui(lua)?;
    kui_lua::set_script_dir(lua, &paths.app)?;
    let kui = lua.create_table()?;
    let fs = lua.create_table()?;
    let root = paths.clone();
    fs.set(
        "exists",
        lua.create_function(move |_, path: String| {
            Ok(root.resolve(&path).is_ok_and(|path| path.exists()))
        })?,
    )?;
    let root = paths.clone();
    fs.set(
        "try_read",
        lua.create_function(move |lua, path: String| {
            let path = match root.resolve(&path) {
                Ok(path) => path,
                Err(error) => return file_failure(lua, Path::new(&path), error),
            };
            match std::fs::read(&path) {
                Ok(bytes) => (true, lua.create_string(bytes)?).into_lua_multi(lua),
                Err(error) => file_failure(lua, &path, error),
            }
        })?,
    )?;
    let root = paths.clone();
    fs.set(
        "try_write",
        lua.create_function(move |lua, (path, bytes): (String, mlua::LuaString)| {
            modify_file(lua, &root, &path, |path| write(path, &bytes.as_bytes()))
        })?,
    )?;
    let root = paths.clone();
    fs.set(
        "try_mkdir",
        lua.create_function(move |lua, path: String| {
            modify_file(lua, &root, &path, |path| std::fs::create_dir_all(path))
        })?,
    )?;
    let root = paths.clone();
    fs.set(
        "try_remove",
        lua.create_function(move |lua, path: String| {
            modify_file(lua, &root, &path, |path| {
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
    kui.set("name", app.name)?;
    kui.set("id", app.id)?;
    kui.set("app_version", app.version)?;
    kui.set("version", env!("CARGO_PKG_VERSION"))?;
    kui.set("platform", app.platform)?;
    kui.set("dir", paths.app.to_string_lossy().as_ref())?;
    kui.set("data_dir", paths.data.to_string_lossy().as_ref())?;
    kui.set("cache_dir", paths.cache.to_string_lossy().as_ref())?;
    let packed = lua.create_table()?;
    packed.set("runner", env!("CARGO_PKG_VERSION"))?;
    packed.set("bytecode", false)?;
    kui.set("packed", packed)?;
    kui.set("fs", fs)?;
    kui.set("json", json)?;
    let globals = lua.globals();
    globals.set("kui", kui.clone())?;
    let package: Table = globals.get("package")?;
    let loaded: Table = package.get("loaded")?;
    loaded.set("kui", kui)?;
    let root = paths.app.to_string_lossy();
    let mut path = format!("{root}/?.lua;{root}/?/init.lua");
    if cfg!(target_os = "wasi") {
        path.push(';');
        path.push_str(&package.get::<String>("path")?);
    } else {
        // The app's file writes go through kui.fs on a native filesystem.
        for name in ["io", "os"] {
            globals.set(name, Value::Nil)?;
            loaded.set(name, Value::Nil)?;
        }
        globals.set("dofile", Value::Nil)?;
        globals.set("loadfile", Value::Nil)?;
    }
    package.set("path", path)?;
    package.set("cpath", "")?;
    package.get::<Table>("searchers")?.set(3, Value::Nil)?;
    package.get::<Table>("searchers")?.set(4, Value::Nil)?;
    lua.load(ARGS).set_name("@kui:args").exec()?;
    lua.load(include_str!("app.lua"))
        .set_name("@kui:app")
        .exec()
}

fn write(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path.parent().ok_or(io::ErrorKind::InvalidInput)?;
    std::fs::create_dir_all(dir)?;
    #[cfg(feature = "embedded")]
    {
        use std::io::Write;
        let mut temp = tempfile::NamedTempFile::new_in(dir)?;
        temp.write_all(bytes)?;
        temp.as_file().sync_all()?;
        temp.persist(path)?;
        Ok(())
    }
    #[cfg(not(feature = "embedded"))]
    std::fs::write(path, bytes)
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
    paths: &Paths,
    path: &str,
    modify: impl FnOnce(&Path) -> io::Result<()>,
) -> mlua::Result<MultiValue> {
    let file = match paths.resolve(path) {
        Ok(file) => file,
        Err(error) => return file_failure(lua, Path::new(path), error),
    };
    if file.starts_with(&paths.app) {
        let message = format!(
            "{path}: a packed app's own files are read-only — write under kui.data_dir ({})",
            paths.data.display(),
        );
        return (false, message, "denied").into_lua_multi(lua);
    }
    match modify(&file) {
        Ok(()) => true.into_lua_multi(lua),
        Err(error) => file_failure(lua, &file, error),
    }
}
