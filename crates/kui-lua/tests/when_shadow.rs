//! A `when` entry that moves a shadow (backlog C114): it sets the lanes it
//! names and keeps the rest — the colour, the blur — so a face lifted on
//! hover keeps its shadow, written from Lua as Kit writes it.

use kui_core::{Core, Extensions, NodeSpec, QuadKind, Size, Value};
use kui_lua::LuaExtension;

const SCRIPT: &str = r#"
    components = {
      card = function(env, props)
        return row {
          key = "card", width = 80, height = 40, bg = 0xffffffff,
          shadow_color = 0x00000080, shadow_blur = 6, shadow_x = 2, shadow_y = 2,
          when = {
            -- Every platform: the entry always holds.
            { platform = { "macos", "windows", "linux" }, shadow_x = 3, shadow_y = 5 },
          },
        }
      end,
    }
"#;

#[test]
fn an_entry_that_moves_the_shadow_keeps_its_colour_and_blur() {
    let mut exts = Extensions::new();
    exts.push_as(
        "s",
        Box::new(LuaExtension::from_source("s.lua", SCRIPT).unwrap()),
    )
    .unwrap();
    let mut core = Core::new();
    for _ in 0..2 {
        let mut ui = core.frame_with(Size::new(200.0, 100.0), 1.0, &mut exts);
        ui.configure_root(NodeSpec::column().pad(20.0));
        ui.component("s/card", &Value::Null);
        ui.finish();
    }
    let (dl, _) = core.output();
    let shadow = dl
        .quads
        .iter()
        .find(|q| q.kind == QuadKind::Shadow)
        .expect("the shadow is drawn");
    assert!(
        (shadow.color.a - 0x80 as f32 / 255.0).abs() < 0.01,
        "its colour kept"
    );
    assert_eq!(shadow.blur, 6.0, "its blur kept");
    // The node at (20, 20), moved by (3, 5), inflated by the blur.
    assert_eq!(
        (shadow.rect.x, shadow.rect.y),
        (20.0 + 3.0 - 6.0, 20.0 + 5.0 - 6.0)
    );
}
