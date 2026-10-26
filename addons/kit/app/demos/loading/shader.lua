-- A shader box: a fragment shader (WGSL) painting a box, rounded by the
-- skin. The host registers the shader and passes its handle — a Rust, C,
-- Odin or Node host can; a Lua app on the runner has no door to register
-- one yet, so here the box is the skin's well, as it is for any host that
-- passes no handle. Kit's skeletons sweep with addons/kit/shaders/shimmer.wgsl
-- the same way, once a host hands it to `kit.settings({ shimmer = handle })`.

local kit = require("kit.ui")

local demo = {
  title = "Shader box",
  note = "a fragment shader in a box, rounded by the skin; the host registers it",
  call = '-- Rust: let id = core.add_fragment(include_str!("noise.wgsl")).unwrap();\n-- Node: const id = ctx.addFragment(wgsl);\nkit.shader({ id = handle, params = { 0.2, 0.4, 0.9, 1 }, animate = true, height = 120 })',
}

function demo.view(m)
  return kit.shader({ height = 96, label = "A shader's box" }),
    text("With no handle the box is the skin's well and keeps its place. Given one, `animate` asks for a frame every frame for a shader that reads `time`, and holds still under reduced motion.",
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
