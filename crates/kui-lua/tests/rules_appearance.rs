//! A script sets the appearance its own `dark` / `light` conditions read
//! (backlog C113): Kit holding its look dark has its `when = { dark = true
//! }` entries agree with its colours, while the host's nodes read the
//! palette as before.

use kui_core::{Color, Cond, Core, Extensions, NodeSpec, Size, Value, testing};
use kui_lua::LuaExtension;

const SCRIPT: &str = r#"
    components = {
      card = function(env, props)
        env.set_rules_appearance(props.held)
        return row { width = 10, height = 10, bg = 0xffffffff,
                     when = { { dark = true, bg = 0x000000ff } } }
      end,
    }
"#;

#[test]
fn a_script_holds_its_rules_dark() {
    let mut exts = Extensions::new();
    let ext = LuaExtension::from_source("s.lua", SCRIPT).unwrap();
    exts.push_as("s", Box::new(ext)).unwrap();
    let mut core = Core::new();
    core.set_theme(kui_core::Theme::light());
    let mut frame = |core: &mut Core, props: &Value| {
        let mut ui = core.frame_with(Size::new(100.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::column().fill());
        ui.component("s/card", props);
        ui.leaf_keyed(
            "host",
            NodeSpec::row()
                .size(10.0, 10.0)
                .bg(Color::hex(0xff0000ff))
                .when(Cond::Dark, |p| p.bg(Color::hex(0x00ff00ff))),
        );
        ui.finish();
        testing::solids(core)
            .iter()
            .map(|q| q.color)
            .collect::<Vec<_>>()
    };
    let dark = Value::map([("held", Value::str("dark"))]);
    frame(&mut core, &dark);
    let colours = frame(&mut core, &dark);
    assert!(
        colours.contains(&Color::BLACK),
        "the script's rule reads dark: {colours:?}"
    );
    assert!(
        colours.contains(&Color::hex(0xff0000ff)),
        "the host's reads the light palette: {colours:?}"
    );
    let palette = Value::map([("held", Value::Null)]);
    frame(&mut core, &palette);
    let colours = frame(&mut core, &palette);
    assert!(
        colours.contains(&Color::WHITE),
        "nil gives the palette back: {colours:?}"
    );
    let w = core.take_warnings();
    assert!(w.is_empty(), "{w:?}");
}
