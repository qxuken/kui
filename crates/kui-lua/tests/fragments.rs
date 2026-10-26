//! A script registers its own fragment (backlog C111): `env.add_fragment`
//! returns an id for `fragment { id = }`, the same one each time it is
//! asked, and nil with a `fragment-rejected` warning for source that does
//! not compile — so Kit's `shader` component, and an app run on the
//! runner, need no host to hand them a handle.

use kui_core::{Core, Extensions, NodeSpec, QuadKind, Size, Value, testing};
use kui_lua::LuaExtension;

const SCRIPT: &str = r#"
    local SRC = [[
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    return vec4<f32>(1.0, 0.0, 0.0, 1.0);
}
]]
    ids = {}
    components = {
      shine = function(env, props)
        local id = env.add_fragment(SRC)
        ids[#ids + 1] = id
        ids[#ids + 1] = env.add_fragment(SRC)
        bad = env.add_fragment("fn nope(")
        return fragment { id = id, width = 40, height = 20 }
      end,
    }
"#;

fn frame(core: &mut Core, exts: &mut Extensions) {
    let mut ui = core.frame_with(Size::new(200.0, 100.0), 1.0, exts);
    ui.configure_root(NodeSpec::column().fill());
    ui.component("s/shine", &Value::Null);
    ui.finish();
}

#[test]
fn a_script_registers_its_own_fragment() {
    let mut exts = Extensions::new();
    let ext = LuaExtension::from_source("s.lua", SCRIPT).unwrap();
    let lua = ext.lua().clone();
    exts.push_as("s", Box::new(ext)).unwrap();
    let mut core = Core::new();
    frame(&mut core, &mut exts);
    frame(&mut core, &mut exts);
    let ids: Vec<u64> = lua.globals().get("ids").unwrap();
    assert!(
        ids.len() >= 2 && ids.iter().all(|&i| i == ids[0] && i != 0),
        "{ids:?}"
    );
    let bad: Option<u64> = lua.globals().get("bad").unwrap();
    assert_eq!(bad, None, "source that does not compile is nil");
    assert!(
        testing::codes(&core.take_warnings()).contains(&"fragment-rejected"),
        "and warned"
    );
    let quads = testing::quads_of(&mut core, QuadKind::Fragment);
    assert_eq!(quads.len(), 1, "drawn by the fragment it registered");
}
