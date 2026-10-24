-- Button: a box with `role = "button"` (the stock button's look is closed
-- outside Rust), so the skin draws it in every binding. A primary button
-- carries the skin's hard shadow; the others are flat.

local skin = require("skin")
local ctx = require("ctx")

local enum Variant
  "default" "primary" "ghost"
end

local record Params
  label: string
  variant: Variant
  disabled: boolean
  -- The host's message for a press.
  on_press: any
end

local M = { Params = Params }

function M.view(p: Params, c: ctx.Ctx, index?: integer): kui.Node
  local primary = p.variant == "primary"
  local ghost = p.variant == "ghost"
  local t: kui.Props = {
    role = "button",
    label = p.label,
    disabled = p.disabled,
    height = "$kitCtl",
    pad = { x = 10 },
    cross_align = "center",
    bg = primary and "$kitAccent" or (ghost and 0x00000000 or "$kitPanel"),
    hover_bg = primary and "$kitAccent" or "$kitHover",
    radius = "$kitRadius",
    on_click = { kit = "press", id = c.id, index = index },
    text(skin.label(c.skin, p.label), {
      size = 11.5,
      bold = true,
      family = "$kitUi",
      color = primary and "$kitOnAccent" or "$kitFg",
    }),
  }
  if not ghost then
    t.border = { w = "$kitRuleW", color = "$kitInk" }
  end
  if primary then
    t.shadow_x, t.shadow_y, t.shadow_color = "$kitShadow", "$kitShadow", "$kitInk"
  end
  return row(t)
end

function M.event(_ev: kui.Event, p: Params): {string:any}
  return ctx.emit(p.on_press, {})
end

return M
