//! Size expressions from Lua (backlog F109), end to end: a view of 1000
//! rows each declaring a `width`, built, lowered and laid out per frame,
//! by the form the width is written in. The difference against `px` is
//! what the form costs a node — `"clamp(…)"` spelled (parsed once, then
//! found by its text), the same as a table built in the view each frame
//! (`{ clamp = { 400, { pct = 80 }, 1000 } }`, what a view literally
//! writes), and the table built once and reused.
//!
//! Run: cargo bench -p kui-lua --bench sizes

use kui_core::{Core, Extension, OriginId, Size, Slot};
use kui_lua::LuaExtension;

fn view(width: &str) -> String {
    format!(
        r#"
        local W = {{ clamp = {{ 400, {{ pct = 80 }}, 1000 }} }}
        function view(env)
          local t = {{ width = "grow" }}
          for i = 1, 1000 do
            t[#t + 1] = row {{ width = {width}, height = 4 }}
          end
          return column(t)
        end
        "#
    )
}

fn frame(core: &mut Core, ext: &mut LuaExtension) {
    let mut ui = core.frame(Size::new(1600.0, 1000.0), 1.0);
    ui.set_origin(OriginId(1));
    ext.view(&Slot::root(), &mut ui).unwrap();
    ui.finish();
}

#[divan::bench(args = [
    "px",
    "percent",
    "spelled",
    "table per frame",
    "table reused",
])]
fn lua_1000_rows(bencher: divan::Bencher, form: &str) {
    let width = match form {
        "px" => "720",
        "percent" => "\"80%\"",
        "spelled" => "\"clamp(400px, 80%, 1000px)\"",
        "table per frame" => "{ clamp = { 400, { pct = 80 }, 1000 } }",
        _ => "W",
    };
    let mut ext = LuaExtension::from_source("bench", &view(width)).unwrap();
    let mut core = Core::new();
    frame(&mut core, &mut ext);
    bencher.bench_local(|| frame(&mut core, &mut ext));
}

fn main() {
    divan::main();
}
