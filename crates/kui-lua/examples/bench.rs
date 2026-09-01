//! Frontend-lowering shootout: the same ~900-node view (300 rows of
//! text + swatch) built per frame through the Rust builder and through a
//! Lua view, timed headlessly. Compare with examples/node's bench for the
//! JSX/Node numbers. Run: cargo run -p kui-lua --example bench --release

use std::time::Instant;

use kui_core::{Color, Core, Extension, NodeSpec, OriginId, Size, Sizing, TextStyle};
use kui_lua::LuaExtension;

const ROWS: usize = 300;
const WARMUP: usize = 30;
const ITERS: usize = 200;

const LUA_VIEW: &str = r#"
function view(env)
  local t = { pad = 8, gap = 2 }
  for i = 1, 300 do
    t[#t + 1] = row { gap = 4, bg = 0x202030ff,
      text("row " .. (i - 1), { size = 14 }),
      column { width = 40, height = 12, bg = 0x3b5bd4ff },
    }
  end
  return column(t)
end
"#;

fn rust_frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    ui.configure_root(NodeSpec::column().pad(8.0).gap(2.0));
    for i in 0..ROWS {
        ui.with(NodeSpec::row().gap(4.0).bg(Color::hex(0x202030ff)), |ui| {
            ui.text(&format!("row {i}"), TextStyle::new(14.0));
            ui.with(
                NodeSpec::column()
                    .width(Sizing::Fixed(40.0))
                    .height(Sizing::Fixed(12.0))
                    .bg(Color::hex(0x3b5bd4ff)),
                |_| {},
            );
        });
    }
    ui.finish();
}

fn lua_frame(core: &mut Core, ext: &mut LuaExtension) {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    ui.set_origin(OriginId(1));
    ext.view(&mut ui).expect("lua view");
    ui.finish();
}

fn bench(name: &str, mut frame: impl FnMut()) {
    for _ in 0..WARMUP {
        frame();
    }
    let t0 = Instant::now();
    for _ in 0..ITERS {
        frame();
    }
    let ms = t0.elapsed().as_secs_f64() * 1e3 / ITERS as f64;
    println!("{name:<12} {ms:>7.3} ms/frame");
}

fn main() {
    let mut core = Core::new();
    bench("rust", || rust_frame(&mut core));
    let quads = core.output().0.quads.len();

    let mut lua_core = Core::new();
    let mut ext = LuaExtension::from_source("bench", LUA_VIEW).expect("load lua");
    bench("lua", || lua_frame(&mut lua_core, &mut ext));
    let lua_quads = lua_core.output().0.quads.len();

    println!("quads: rust {quads}, lua {lua_quads}");
}
