//! `env.tokens` lists what a script can paint with (backlog C112): its own
//! tokens over the host's over every other origin's exported ones — so an
//! app reads `$kitAccent`'s value as it resolves in a prop — and colours
//! are `0xRRGGBBAA` integers, as Teal types them.

use kui_core::{Core, Extensions, NodeSpec, Size, Value};
use kui_lua::LuaExtension;

const KIT: &str = r#"
    tokens = { export = true, colors = { kitAccent = 0x3366ffff, shared = 0x111111ff },
               lengths = { kitRow = 28 } }
    components = { panel = function(env, props) return row {} end }
"#;

const APP: &str = r#"
    tokens = { colors = { shared = 0x222222ff } }
    components = {
      read = function(env, props)
        seen = env.tokens
        accent_type = math.type(env.tokens.colors.kitAccent)
        return row { bg = "$kitAccent", width = 10, height = 10 }
      end,
    }
"#;

#[test]
fn a_script_reads_another_origins_exported_tokens() {
    let mut exts = Extensions::new();
    exts.push_as(
        "kit",
        Box::new(LuaExtension::from_source("kit.lua", KIT).unwrap()),
    )
    .unwrap();
    let app = LuaExtension::from_source("app.lua", APP).unwrap();
    let lua = app.lua().clone();
    exts.push_as("app", Box::new(app)).unwrap();
    let mut core = Core::new();
    for _ in 0..2 {
        let mut ui = core.frame_with(Size::new(200.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.component("kit/panel", &Value::Null);
        ui.component("app/read", &Value::Null);
        ui.finish();
    }
    let seen: mlua::Table = lua.globals().get("seen").unwrap();
    let colors: mlua::Table = seen.get("colors").unwrap();
    assert_eq!(
        colors.get::<Option<u32>>("kitAccent").unwrap(),
        Some(0x3366ffff)
    );
    assert_eq!(
        colors.get::<Option<u32>>("shared").unwrap(),
        Some(0x222222ff),
        "the script's own wins"
    );
    let lengths: mlua::Table = seen.get("lengths").unwrap();
    assert_eq!(lengths.get::<Option<f32>>("kitRow").unwrap(), Some(28.0));
    let ty: String = lua.globals().get("accent_type").unwrap();
    assert_eq!(ty, "integer");
    let w = core.take_warnings();
    assert!(w.is_empty(), "{w:?}");
}
