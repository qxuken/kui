-- Generated from src/skin.tl by addons/kit/build.lua. Edit the .tl.







local Half = {}




































local Skin = {}





























local Settings = {}
















local ROLES = {
   "bg", "side", "panel", "raised", "sunken", "hover", "press", "rule", "ink",
   "fg", "muted", "faint", "sel", "sel_fg", "accent", "on_accent", "focus",
   "ok", "warn", "danger", "scrim",
}

local M = { Half = Half, Skin = Skin, Settings = Settings, ROLES = ROLES }


local function camel(role)
   return (role:gsub("^%l", string.upper):gsub("_(%l)", string.upper))
end


function M.row_h(s)
   return s.density == "compact" and 20 or 22
end

function M.ctl_h(s)
   return s.density == "compact" and 22 or 26
end



function M.ref(s, role)
   local o = s.roles and s.roles[role]
   if type(o) == "string" then return o end
   return "$kit" .. camel(role)
end

local function half_of(v, l, d)
   if math.type(v) == "integer" then return v, v end
   if type(v) == "table" then return v.light or l, v.dark or d end
   return l, d
end

function M.tokens(s)
   local skin = s.skin
   local colors = {}
   local l = skin.light
   local d = skin.dark
   for _, role in ipairs(ROLES) do
      local lv, dv = half_of(s.roles and s.roles[role], l[role], d[role])
      colors["kit" .. camel(role)] = { light = lv, dark = dv }
   end
   for i = 1, #skin.light.groups do
      colors["kitGroup" .. i] = { light = skin.light.groups[i], dark = skin.dark.groups[i] }
   end
   for i = 1, #skin.light.series do
      colors["kitSeries" .. i] = { light = skin.light.series[i], dark = skin.dark.series[i] }
   end
   local shadow = s.shadows and skin.shadow or 0
   return {
      colors = colors,
      lengths = {
         kitRow = M.row_h(s),
         kitCtl = M.ctl_h(s),
         kitRadius = skin.radius,
         kitRuleW = skin.rule_w,
         kitHair = skin.hair,
         kitShadow = shadow,
         kitShadowBlur = s.shadows and skin.shadow_blur or 0,
         kitSize = skin.size,
         kitSizeLabel = skin.size_label,
         kitSizeTitle = skin.size_title,
      },
      families = { kitUi = skin.font_ui, kitData = skin.font_data },
      export = true,
   }
end


function M.key(s)
   local parts = { s.skin.name, s.density, tostring(s.shadows) }
   if s.roles then
      local names = {}
      for k in pairs(s.roles) do names[#names + 1] = k end
      table.sort(names)
      for _, k in ipairs(names) do
         local v = s.roles[k]
         if type(v) == "table" then
            parts[#parts + 1] = k .. "=" .. tostring(v.light) .. "/" .. tostring(v.dark)
         else
            parts[#parts + 1] = k .. "=" .. tostring(v)
         end
      end
   end
   return table.concat(parts, ";")
end


function M.label(s, text)
   return s.skin.label_upper and text:upper() or text
end

return M