-- A shader box: a fragment shader (WGSL) painting a box, rounded by the
-- skin. Kit's presets take the skin's colours; your own WGSL goes in as
-- `source`, and Kit registers either (env.add_fragment), so the same
-- source is the same fragment every frame. Under reduced motion it holds
-- still.

local kit = require("kit.ui")

local demo = {
  title = "Shader box",
  note = "a fragment shader in a box, rounded by the skin: Kit's presets, or your own WGSL",
  call = 'kit.shader({ preset = "noise", height = 120 })\nkit.shader({ source = RINGS, params = { 0.18, 0.43, 0.93, 1 }, animate = true, height = 120 })',
  state = { shader = "noise" },
}

-- Your own: rings spreading from the middle in a colour you pass, its
-- alpha falling off toward the edge. `in.time` moves it; the four params
-- are the colour.
local RINGS = [[
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let p = (in.local - in.size * 0.5) / in.scale;
    let d = length(p);
    let wave = 0.5 + 0.5 * sin(d * 0.15 - in.time * 2.0);
    let fade = clamp(1.0 - d / (0.5 * length(in.size / in.scale)), 0.0, 1.0);
    return vec4<f32>(params[0].rgb, params[0].a * wave * fade);
}
]]

function demo.update(m, msg)
  if msg.kind == "shader" then m.shader = msg.change.value end
end

function demo.view(m)
  local box
  if m.shader == "rings" then
    box = kit.shader({ source = RINGS, params = { 0.18, 0.43, 0.93, 1 }, animate = true, height = 120, label = "Rings" })
  else
    box = kit.shader({ preset = m.shader, height = 120, label = m.shader == "noise" and "Noise" or "Shimmer" })
  end
  return kit.segmented({ label = "Shader", value = m.shader, on_change = { kind = "shader" },
      options = { { value = "noise", label = "Noise" }, { value = "shimmer", label = "Shimmer" }, { value = "rings", label = "Your own" } } }),
    box,
    text("Noise drifts between the skin's well and its accent; Shimmer is a skeleton's sweep; Your own is the WGSL in this card's source.",
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
