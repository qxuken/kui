//! Frontend-lowering shootout: the same ~900-node view (300 rows of
//! text + swatch) built per frame through the Rust builder and through a
//! Lua view, timed headlessly. Compare with examples/node's bench for the
//! JSX/Node numbers. Run: cargo run -p kui-lua --example bench --release

use std::time::Instant;

use kui_core::{Color, Core, Extension, NodeSpec, OriginId, Size, Sizing, Slot, TextStyle};
use kui_lua::LuaExtension;

const ROWS: usize = 300;
const WARMUP: usize = 30;
const ITERS: usize = 200;

/// The two sides of the comparison declare the *same* tree, colours
/// included, so the literals here stay literal: a bench whose Lua half
/// read `env.theme` and whose Rust half read `ui.theme()` would still be
/// comparable, but a bench whose halves could drift is not a bench. The
/// apps are on the theme (ADR 0019); this is a measurement fixture.
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

// Same tree shape as the Lua view: an extension's table is a child of the
// frame's root (panels share the host's frame), so the Rust side nests its
// column under the default root too. Configuring the root instead would
// change layout, not just lowering — the nested column is a Fit child and
// the shrink pass squashes it to the viewport, dropping the row backgrounds.
fn rust_frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    ui.with(NodeSpec::column().pad(8.0).gap(2.0), |ui| {
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
    });
    ui.finish();
}

fn lua_frame(core: &mut Core, ext: &mut LuaExtension) {
    let mut ui = core.frame(Size::new(800.0, 600.0), 1.0);
    ui.set_origin(OriginId(1));
    ext.view(&Slot::root(), &mut ui).expect("lua view");
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
