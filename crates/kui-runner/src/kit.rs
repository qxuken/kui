//! Kit, built into the runner (ADR 0064, decision 1): its extension, loaded
//! under `kit` from the Lua compiled into the binary, and its host module,
//! which the app's `require("kit.ui")` finds.

use mlua::Lua;

pub use kui_kit::{NAMESPACE, extension};

/// The host module the app places Kit through (generated, ADR 0066).
const UI: &str = include_str!("../../../addons/kit/bindings/lua/kit/ui.lua");

fn preload(lua: &Lua, name: &str, source: &'static str) -> mlua::Result<()> {
    let preload: mlua::Table = lua
        .globals()
        .get::<mlua::Table>("package")?
        .get("preload")?;
    let chunk = format!("@kit/{name}.lua");
    preload.set(
        name,
        lua.create_function(move |lua, ()| {
            lua.load(source)
                .set_name(chunk.as_str())
                .call::<mlua::Value>(())
        })?,
    )
}

/// Makes `require("kit.ui")` Kit's host module in the app's state.
pub fn install_ui(lua: &Lua) -> mlua::Result<()> {
    preload(lua, "kit.ui", UI)?;
    preload(
        lua,
        "kit.presentation",
        include_str!("../../../addons/kit/host/lua/kit/presentation.lua"),
    )
}
