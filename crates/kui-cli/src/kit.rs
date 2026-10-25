//! Kit, built into the runner (ADR 0064, decision 1): its extension, loaded
//! under `kit` from the Lua compiled into the binary, and its host module,
//! which the app's `require("kit.ui")` finds.

use kui_lua::LuaExtension;
use mlua::Lua;

/// Kit's modules, as `addons/kit/build.lua` writes them.
pub const MODULES: &[(&str, &str)] = &[
    ("charts", include_str!("../../../addons/kit/lua/charts.lua")),
    ("ctx", include_str!("../../../addons/kit/lua/ctx.lua")),
    ("data", include_str!("../../../addons/kit/lua/data.lua")),
    ("icons", include_str!("../../../addons/kit/lua/icons.lua")),
    ("inputs", include_str!("../../../addons/kit/lua/inputs.lua")),
    ("layout", include_str!("../../../addons/kit/lua/layout.lua")),
    ("ledger", include_str!("../../../addons/kit/lua/ledger.lua")),
    ("neutral", include_str!("../../../addons/kit/lua/neutral.lua")),
    ("overlays", include_str!("../../../addons/kit/lua/overlays.lua")),
    ("phosphor", include_str!("../../../addons/kit/lua/phosphor.lua")),
    ("skin", include_str!("../../../addons/kit/lua/skin.lua")),
    ("waterfall", include_str!("../../../addons/kit/lua/waterfall.lua")),
];
const ENTRY: &str = include_str!("../../../addons/kit/lua/kit.lua");
/// The host module the app places Kit through (generated, ADR 0066).
const UI: &str = include_str!("../../../addons/kit/bindings/lua/kit/ui.lua");

/// The namespace Kit is loaded under.
pub const NAMESPACE: &str = "kit";

fn preload(lua: &Lua, name: &str, source: &'static str) -> mlua::Result<()> {
    let preload: mlua::Table = lua.globals().get::<mlua::Table>("package")?.get("preload")?;
    let chunk = format!("@kit/{name}.lua");
    preload.set(
        name,
        lua.create_function(move |lua, ()| lua.load(source).set_name(chunk.as_str()).call::<mlua::Value>(()))?,
    )
}

/// Kit's extension, its modules found in the binary.
pub fn extension() -> mlua::Result<LuaExtension> {
    LuaExtension::from_source_with("kit.lua", ENTRY, |lua| {
        for (name, source) in MODULES {
            preload(lua, name, source)?;
        }
        Ok(())
    })
}

/// Makes `require("kit.ui")` Kit's host module in the app's state.
pub fn install_ui(lua: &Lua) -> mlua::Result<()> {
    preload(lua, "kit.ui", UI)
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_kit_module_is_in_the_binary() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../addons/kit/lua");
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| e.file_name().to_str()?.strip_suffix(".lua").map(str::to_string))
            .filter(|n| n != "kit")
            .collect();
        on_disk.sort();
        let embedded: Vec<String> = super::MODULES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(embedded, on_disk, "add the new module to kit::MODULES");
    }
}
