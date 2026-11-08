//! Browser host for the book's presentation controls.
use crate::{App, Ui, Window};
use mlua::Function;

#[path = "../../../crates/kui-web-lua/src/app.rs"]
mod app_host;
use app_host::kit;

thread_local! {
    static TASKS: std::cell::OnceCell<Function> = const { std::cell::OnceCell::new() };
}

pub fn launch_presentation() -> Result<(), Box<dyn std::error::Error>> {
    let app = kui_lua::LuaExtension::from_source_with(
        "@presentations/main.lua",
        include_str!("../../lua/tutorial/presentations/main.lua"),
        |lua| app_host::install(lua, "Presentations", "dev.kui.book.presentations", "1.0.0"),
    )?;
    app.lua().load(include_str!("lua_app.lua")).exec()?;
    app.lua()
        .globals()
        .get::<Function>("__book_presentation_controls")?
        .call::<()>(())?;
    run(app)
}

fn run(app: kui_lua::LuaExtension) -> Result<(), Box<dyn std::error::Error>> {
    let drain = app_host::initialize(&app)?;
    TASKS.with(|tasks| {
        tasks
            .set(drain)
            .map_err(|_| mlua::Error::runtime("a Lua app is already running"))
    })?;
    Window::default()
        .size(640.0, 420.0)
        .min_size(320.0, 420.0)
        .extension_as(kit::NAMESPACE, kit::extension()?)
        .extension_as("app", app)
        .run(LuaApp)
}

struct LuaApp;

impl App for LuaApp {
    fn view(&mut self, _ui: &mut Ui<'_>) {
        // The extension fills the root, as it does in the CLI runner.
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
