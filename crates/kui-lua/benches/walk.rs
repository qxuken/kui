//! The walk from a view's tables to the tree (backlog F143): a settings
//! pane's worth of tables — 120 settings, each a column holding a name, a
//! described line of spans, and a row of four chips with a background, a
//! radius, padding and a click payload, about the 1,561 tables kawoosh's
//! settings pane returned when its walk was measured at 3.0 ms a frame —
//! lowered every frame.
//!
//! `walk` returns one tree built at load, so it times kui-lua's half
//! alone: reading the tables and pushing the nodes, layout and emission
//! included as every frame has them. `build_and_walk` builds the tables
//! in the view each frame, as a view does, so the gap between the two is
//! the script's own time. Each with the core's diagnostics on (the
//! default, which checks every key of every table against the schema)
//! and off.
//!
//! Run: cargo bench -p kui-lua --bench walk

use kui_core::{Core, Extension, OriginId, Size, Slot};
use kui_lua::LuaExtension;

const TREE: &str = r#"
local function setting(i)
  local chips = row { gap = 4, cross_gap = 4, wrap_children = true }
  for c = 1, 4 do
    chips[#chips + 1] = row { key = "chip" .. c, pad = { x = 8, y = 2 }, radius = 4,
      bg = c == 2 and 0x7d5bbfff or 0x00000010, hover_bg = 0x7d5bbf40,
      on_click = { kind = "pick", key = "layout.k" .. i, value = c },
      text("choice " .. c, { size = 12, color = c == 2 and 0xffffffff or 0x404040ff }) }
  end
  return column { key = "s" .. i, width = "grow", gap = 4, pad = { x = 12, y = 8 },
    text({ { "layout.", color = 0x404040ff }, { "setting_" .. i, bold = true } },
      { size = 13, family = "mono" }),
    text({ { "what the setting does, " }, { "in a few", bold = true }, { " words" } },
      { size = 12, color = 0x606060ff, wrap = "word" }),
    chips,
  }
end

local function tree()
  local body = column { key = "body", width = "grow", height = "grow", scroll_y = true, gap = 2 }
  for i = 1, 120 do body[#body + 1] = setting(i) end
  return column { width = "grow", height = "grow", body }
end
"#;

fn source(cached: bool) -> String {
    if cached {
        format!("{TREE}\nlocal T = tree()\nfunction view(env) return T end\n")
    } else {
        format!("{TREE}\nfunction view(env) return tree() end\n")
    }
}

fn frame(core: &mut Core, ext: &mut LuaExtension) {
    let mut ui = core.frame(Size::new(900.0, 700.0), 1.0);
    ui.set_origin(OriginId(1));
    ext.view(&Slot::root(), &mut ui).unwrap();
    ui.finish();
}

fn run(bencher: divan::Bencher, cached: bool, diagnostics: bool) {
    let mut core = Core::new();
    core.set_diagnostics(diagnostics);
    let mut ext = LuaExtension::from_source("walk", &source(cached)).unwrap();
    for _ in 0..3 {
        frame(&mut core, &mut ext);
    }
    assert!(core.take_warnings().is_empty(), "the fixture is clean");
    bencher.bench_local(|| frame(&mut core, &mut ext));
}

#[divan::bench(args = [true, false])]
fn walk(bencher: divan::Bencher, diagnostics: bool) {
    run(bencher, true, diagnostics);
}

#[divan::bench(args = [true, false])]
fn build_and_walk(bencher: divan::Bencher, diagnostics: bool) {
    run(bencher, false, diagnostics);
}

fn main() {
    divan::main();
}
