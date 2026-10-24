-- Generated from src/button.tl by addons/kit/build.lua. Edit the .tl.




local skin = require("skin")
local ctx = require("ctx")





local Params = {}







local M = { Params = Params }

function M.view(p, c, index)
   local primary = p.variant == "primary"
   local ghost = p.variant == "ghost"
   local t = {
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

function M.event(_ev, p)
   return ctx.emit(p.on_press, {})
end

return M