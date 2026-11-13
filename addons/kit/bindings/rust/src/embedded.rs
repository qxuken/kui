use kui_lua::LuaExtension;
use mlua::Lua;

/// Kit's modules, as `addons/kit/build.lua` writes them.
const MODULES: &[(&str, &str)] = &[
    ("charts", include_str!("../../../lua/charts.lua")),
    ("content", include_str!("../../../lua/content.lua")),
    ("ctx", include_str!("../../../lua/ctx.lua")),
    ("data", include_str!("../../../lua/data.lua")),
    ("icons", include_str!("../../../lua/icons.lua")),
    ("inputs", include_str!("../../../lua/inputs.lua")),
    ("layout", include_str!("../../../lua/layout.lua")),
    ("ledger", include_str!("../../../lua/ledger.lua")),
    ("neutral", include_str!("../../../lua/neutral.lua")),
    ("overlays", include_str!("../../../lua/overlays.lua")),
    ("phosphor", include_str!("../../../lua/phosphor.lua")),
    ("skin", include_str!("../../../lua/skin.lua")),
    ("syntax", include_str!("../../../lua/syntax.lua")),
    ("themes", include_str!("../../../lua/themes.lua")),
    ("waterfall", include_str!("../../../lua/waterfall.lua")),
];
const ENTRY: &str = include_str!("../../../lua/kit.lua");

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

/// Loads Kit with all its Lua modules embedded in the binary.
///
/// Register it under [`crate::NAMESPACE`] (`"kit"`) to use the default
/// component builders. Each call creates an independent Lua interpreter.
/// No Lua files, Lua installation or code generation are needed at runtime.
///
/// ```no_run
/// # struct MyApp;
/// # impl kui_native::App for MyApp {
/// #     fn view(&mut self, _: &mut kui_native::Ui<'_>) {}
/// # }
/// kui_native::app("Kit")
///     .extension_as(kui_kit::NAMESPACE, kui_kit::extension()?)
///     .run(MyApp)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn extension() -> mlua::Result<LuaExtension> {
    LuaExtension::from_source_with("kit.lua", ENTRY, |lua| {
        for (name, source) in MODULES {
            preload(lua, name, source)?;
        }
        Ok(())
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn unavailable_numeric_cells_render_as_missing() {
        let lua = mlua::Lua::new();
        for (name, source) in super::MODULES {
            super::preload(&lua, name, source).unwrap();
        }
        lua.load(
            r#"
            local cell_text = require("data").cell_text
            assert(cell_text(0/0) == "—")
            assert(cell_text(math.huge) == "—")
            assert(cell_text(-math.huge) == "—")
            assert(cell_text(0) == "0")
            assert(cell_text(0.25) == "0.25")
            "#,
        )
        .exec()
        .unwrap();
    }

    #[test]
    fn every_kit_module_is_in_the_binary() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("lua");
        let mut on_disk: Vec<String> = std::fs::read_dir(&dir)
            .unwrap()
            .filter_map(|e| e.ok())
            .filter_map(|e| {
                e.file_name()
                    .to_str()?
                    .strip_suffix(".lua")
                    .map(str::to_string)
            })
            .filter(|n| n != "kit")
            .collect();
        on_disk.sort();
        let embedded: Vec<String> = super::MODULES.iter().map(|(n, _)| n.to_string()).collect();
        assert_eq!(embedded, on_disk, "add the new module to embedded::MODULES");
    }
}
