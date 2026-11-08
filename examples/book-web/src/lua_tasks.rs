//! Browser host for the runner chapter's Lua application.
use crate::{App, Ui, Window};
use mlua::{Function, Lua, LuaSerdeExt, Table, Value};

#[path = "../../../crates/kui-cli/src/kit.rs"]
mod kit;

const SOURCE: &str = include_str!("../../lua/tutorial/tasks/main.lua");
const CONFIG: &str = include_str!("../../lua/tutorial/tasks/kui.config.lua");
const ARGS: &str = include_str!("../../../crates/kui-cli/src/lua/args.lua");

thread_local! {
    static TASKS: std::cell::OnceCell<Function> = const { std::cell::OnceCell::new() };
}

pub fn launch() -> Result<(), Box<dyn std::error::Error>> {
    let app = kui_lua::LuaExtension::from_source_with("@tasks/main.lua", SOURCE, install)?;
    let lua = app.lua();
    let globals = lua.globals();
    globals.set("model", globals.get::<Function>("init")?.call::<Value>(())?)?;
    TASKS.with(|tasks| {
        let drain = globals.get::<Function>("__book_run_tasks")?;
        tasks
            .set(drain)
            .map_err(|_| mlua::Error::runtime("tasks already launched"))
    })?;
    let size: Table = lua
        .load(CONFIG)
        .set_name("@tasks/kui.config.lua")
        .eval::<Table>()?
        .get("size")?;
    Window::default()
        .size(size.get("w")?, size.get("h")?)
        .extension_as(kit::NAMESPACE, kit::extension()?)
        .extension_as("app", app)
        .run(Tasks)
}

struct Tasks;

impl App for Tasks {
    fn view(&mut self, _ui: &mut Ui<'_>) {
        // The extension fills the root, as it does in the CLI runner.
    }
}

fn install(lua: &Lua) -> mlua::Result<()> {
    kit::install_ui(lua)?;
    let kui = lua.create_table()?;
    let fs = lua.create_table()?;
    fs.set(
        "exists",
        lua.create_function(|_, path: String| Ok(std::path::Path::new(&path).exists()))?,
    )?;
    fs.set(
        "try_read",
        lua.create_function(|_, path: String| Ok(answer(std::fs::read_to_string(path))))?,
    )?;
    fs.set(
        "write",
        lua.create_function(|_, (path, text): (String, String)| {
            std::fs::write(path, text).map_err(mlua::Error::external)
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
    kui.set("data_dir", "/data")?;
    kui.set("fs", fs)?;
    kui.set("json", json)?;
    lua.globals().set("kui", kui)?;
    lua.load(ARGS).set_name("@kui:args").exec()?;
    lua.load(include_str!("lua_tasks.lua"))
        .set_name("@kui:book")
        .exec()
}

fn answer(value: std::io::Result<String>) -> (bool, String) {
    match value {
        Ok(text) => (true, text),
        Err(error) => (false, error.to_string()),
    }
}

#[unsafe(no_mangle)]
pub extern "C" fn demo_run_tasks() -> u32 {
    TASKS.with(|tasks| {
        tasks.get().map_or(0, |run| match run.call::<bool>(()) {
            Ok(worked) => u32::from(worked),
            Err(error) => {
                eprintln!("{error}");
                0
            }
        })
    })
}
