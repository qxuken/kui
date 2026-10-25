-- Generated from src/ctx.tl by addons/kit/build.lua. Edit the .tl.





local skin = require("skin")

local Ctx = {}






local M = { Ctx = Ctx }


function M.ref(c, role)
   return skin.ref(c.s, role)
end


function M.label(c, text)
   return skin.label(c.s, text)
end




function M.act(c, kit, fields)
   local m = { kit = kit, inst = c.inst }
   if fields then
      for k, v in pairs(fields) do m[k] = v end
   end
   return m
end


function M.text(c, s, o)
   local t = { size = "$kitSize", family = "$kitUi", color = M.ref(c, "fg") }
   if o then for k, v in pairs(o) do t[k] = v end end
   return text(s, t)
end


function M.data(c, s, o)
   local t = { size = "$kitSize", family = "$kitData", color = M.ref(c, "fg") }
   if o then for k, v in pairs(o) do t[k] = v end end
   return text(s, t)
end


function M.caption(c, s, o)
   local t = {
      size = "$kitSizeLabel", family = "$kitUi", bold = c.s.skin.label_bold, color = M.ref(c, "fg"),
      max_lines = 1, ellipsis = true,
   }
   if o then for k, v in pairs(o) do t[k] = v end end
   return text(M.label(c, s), t)
end


function M.muted(c, s, o)
   local t = { size = "$kitSizeLabel", family = "$kitUi", color = M.ref(c, "muted") }
   if o then for k, v in pairs(o) do t[k] = v end end
   return text(s, t)
end


function M.rule(c, role, w, vertical)
   local t = { bg = M.ref(c, role or "ink") }
   if vertical then
      t.width, t.height = w or "$kitRuleW", "grow"
   else
      t.width, t.height = "grow", w or "$kitRuleW"
   end
   return column(t)
end


function M.grow()
   return row({ width = "grow" })
end





function M.shadow(c, t, strong)
   local sk = c.s.skin
   if sk.shadow == 0 then
      if strong then
         t.shadow_color, t.shadow_blur = M.ref(c, "ink"), "$kitShadowBlur"
      end
   elseif sk.shadow_blur == 0 then
      t.shadow_color = M.ref(c, "ink")
      t.shadow_x, t.shadow_y = "$kitShadow", "$kitShadow"
   else
      t.shadow_color = 0x0000002e
      t.shadow_x, t.shadow_y, t.shadow_blur = 0, "$kitShadow", "$kitShadowBlur"
   end
end



local states = {}

function M.state(c, init)
   local k = math.type(c) == "integer" and c or (c).inst
   local st = states[k]
   if st == nil then
      st = init()
      states[k] = st
   end
   return st
end


function M.peek(inst)
   return states[inst]
end


function M.flag(v, default)
   if v == nil then return default end
   return v == true
end

return M