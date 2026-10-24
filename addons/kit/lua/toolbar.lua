-- Generated from src/toolbar.tl by addons/kit/build.lua. Edit the .tl.





local button = require("button")
local ctx = require("ctx")

local Params = {}



local M = { Params = Params }

function M.view(p, c)
   local t = { gap = 6, cross_align = "center", pad = { x = 10, y = 6 } }
   for i, item in ipairs(p.items or {}) do
      t[#t + 1] = button.view(item, c, i)
   end
   return row(t)
end

function M.event(ev, p)
   local i = (ev).index
   local item = p.items and p.items[i]
   if item then return button.event(ev, item) end
   return nil
end

return M