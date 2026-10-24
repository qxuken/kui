-- Generated from src/ctx.tl by addons/kit/build.lua. Edit the .tl.



local skin = require("skin")

local Ctx = {}







local M = { Ctx = Ctx }





function M.emit(declared, fields)
   if declared == nil then return nil end
   local out = {}
   if type(declared) == "table" then
      for k, v in pairs(declared) do out[k] = v end
   else
      out.tag = declared
   end
   for k, v in pairs(fields) do out[k] = v end
   return out
end

return M