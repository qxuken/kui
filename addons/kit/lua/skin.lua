-- Generated from src/skin.tl by addons/kit/build.lua. Edit the .tl.





local Half = {}


















local Skin = {}

















local Settings = {}










local M = { Half = Half, Skin = Skin, Settings = Settings }

local ROLES = {
   "bg", "side", "panel", "hover", "rule", "ink", "fg", "muted",
   "sel", "sel_fg", "accent", "on_accent", "ok", "warn",
}


local function camel(role)
   return (role:gsub("^%l", string.upper):gsub("_(%l)", string.upper))
end


function M.ref(role)
   return "$kit" .. camel(role)
end


function M.row_h(s)
   return s.density == "compact" and 20 or 22
end

function M.ctl_h(s)
   return s.density == "compact" and 22 or 24
end

function M.tokens(skin, s)
   local colors = {}
   local l = skin.light
   local d = skin.dark
   for _, role in ipairs(ROLES) do
      colors["kit" .. camel(role)] = { light = l[role], dark = d[role] }
   end
   for i = 1, #skin.light.groups do
      colors["kitGroup" .. i] = { light = skin.light.groups[i], dark = skin.dark.groups[i] }
   end
   return {
      colors = colors,
      lengths = {
         kitRow = M.row_h(s),
         kitCtl = M.ctl_h(s),
         kitRadius = skin.radius,
         kitRuleW = skin.rule_w,
         kitShadow = s.shadows and skin.shadow or 0,
      },
      families = { kitUi = skin.font_ui, kitData = skin.font_data },
   }
end


function M.label(skin, text)
   return skin.label_upper and text:upper() or text
end

return M