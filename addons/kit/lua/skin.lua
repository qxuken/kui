-- Generated from src/skin.tl by addons/kit/build.lua. Edit the .tl.







local Half = {}








































local Skin = {}































local Settings = {}






























local ROLES = {
   "bg", "side", "panel", "raised", "sunken", "hover", "press", "rule", "ink",
   "fg", "muted", "faint", "sel", "sel_fg", "accent", "on_accent", "focus",
   "ok", "warn", "danger", "on_danger", "scrim",
}






local SHADED = { "accent", "danger", "panel", "sunken", "raised" }



local TOWARD = { accent = "on_accent", danger = "on_danger" }

local M = { Half = Half, Skin = Skin, Settings = Settings, ROLES = ROLES, AREA_ALPHA = 0.14 }


local function camel(role)
   return (role:gsub("^%l", string.upper):gsub("_(%l)", string.upper))
end


function M.row_h(s)
   return s.density == "compact" and 20 or 22
end

function M.ctl_h(s)
   return s.density == "compact" and 22 or 26
end








local refs = setmetatable({}, { __mode = "k" })

function M.ref(s, role)
   local memo = refs[s]
   if not memo then
      memo = {}
      refs[s] = memo
   end
   local r = memo[role]
   if r then return r end
   local o = s.roles and s.roles[role]
   if type(o) == "string" then
      r = o
   else
      local fill = role:match("^(.-)_hover$") or role:match("^(.-)_press$")
      local named = fill and s.roles and s.roles[fill]
      r = type(named) == "string" and named or ("$kit" .. camel(role))
   end
   memo[role] = r
   return r
end

local function half_of(v, l, d)
   if math.type(v) == "integer" then return v, v end
   if type(v) == "table" then return v.light or l, v.dark or d end
   return l, d
end


local function channels(c)
   return (c >> 24) & 255, (c >> 16) & 255, (c >> 8) & 255, c & 255
end


local function mix(a, b, t)
   local ar, ag, ab = channels(a)
   local br, bg, bb = channels(b)
   local function m(x, y) return math.floor(x + (y - x) * t + 0.5) end
   return (m(ar, br) << 24) | (m(ag, bg) << 16) | (m(ab, bb) << 8) | 0xff
end


local function over(fill, wash)
   local _, _, _, a = channels(wash)
   return mix(fill, wash | 0xff, a / 255)
end


local function shades(h, fill, value)
   local toward = TOWARD[fill]
   if toward then
      local t = h[toward]
      return mix(value, t, 0.16), mix(value, t, 0.30)
   end
   return over(value, h.hover), over(value, h.press)
end


local AREA_ALPHA = M.AREA_ALPHA


function M.faded(color, a)
   local alpha = color & 0xff
   return (color & ~0xff) | math.floor(alpha * a + 0.5)
end


local function luminance(c)
   local r, g, b = channels(c)
   local function lin(x)
      local v = x / 255
      return v <= 0.04045 and v / 12.92 or ((v + 0.055) / 1.055) ^ 2.4
   end
   return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
end

local function contrast(a, b)
   local x, y = luminance(a), luminance(b)
   if x < y then x, y = y, x end
   return (x + 0.05) / (y + 0.05)
end


local function readable(fill, a, b)
   return contrast(fill, a) >= contrast(fill, b) and a or b
end






local FOLLOW = { "sel", "focus" }

local function follow(s, colors, half, h)
   local o = s.roles and s.roles.accent
   if o == nil or type(o) == "string" then return end
   local was = h.accent
   local now = colors.kitAccent[half]
   if now == was then return end
   local function same(c) return (c & ~0xff) == (was & ~0xff) end
   if not s.roles.on_accent then
      colors.kitOnAccent[half] = readable(now, h.fg, h.bg)
   end
   for _, role in ipairs(FOLLOW) do
      local own = h[role]
      if not s.roles[role] and same(own) then
         colors["kit" .. camel(role)][half] = (now & ~0xff) | (own & 0xff)
         if role == "sel" and not s.roles.sel_fg and (h.sel_fg) == (h.on_accent) then
            colors.kitSelFg[half] = colors.kitOnAccent[half]
         end
      end
   end
   local series = colors.kitSeries1
   if series and same(series[half]) then
      series[half] = now
      colors.kitArea1[half] = M.faded(now, AREA_ALPHA)
   end
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
   for _, fill in ipairs(SHADED) do

      local base = colors["kit" .. camel(fill)]
      local lh, lp = shades(l, fill, base.light)
      local dh, dp = shades(d, fill, base.dark)
      colors["kit" .. camel(fill) .. "Hover"] = { light = lh, dark = dh }
      colors["kit" .. camel(fill) .. "Press"] = { light = lp, dark = dp }
   end
   for i = 1, #skin.light.groups do
      colors["kitGroup" .. i] = { light = skin.light.groups[i], dark = skin.dark.groups[i] }
   end
   for i = 1, #skin.light.series do
      local ls, ds = skin.light.series[i], skin.dark.series[i]
      colors["kitSeries" .. i] = { light = ls, dark = ds }


      colors["kitArea" .. i] = { light = M.faded(ls, AREA_ALPHA), dark = M.faded(ds, AREA_ALPHA) }
   end
   follow(s, colors, "light", l)
   follow(s, colors, "dark", d)



   local sunken = colors.kitSunken
   local lit = { light = mix(sunken.light, 0xffffffff, 0.7), dark = mix(sunken.dark, colors.kitFg.dark, 0.12) }
   colors.kitShimmer = lit
   colors.kitShimmerClear = { light = lit.light & ~0xff, dark = lit.dark & ~0xff }

   if s.appearance == "light" or s.appearance == "dark" then
      local half = s.appearance
      for _, c in pairs(colors) do
         local v = c[half]
         c.light, c.dark = v, v
      end
   end
   local shadow = s.shadows and skin.shadow or 0
   return {
      colors = colors,
      lengths = {
         kitRow = M.row_h(s),
         kitCtl = M.ctl_h(s),
         kitRadius = s.radius or skin.radius,
         kitRuleW = s.rule_w or skin.rule_w,
         kitHair = s.hair or skin.hair,
         kitShadow = shadow,
         kitShadowBlur = s.shadows and skin.shadow_blur or 0,
         kitSize = skin.size,
         kitSizeLabel = skin.size_label,
         kitSizeTitle = skin.size_title,
      },
      families = { kitUi = s.font_ui or skin.font_ui, kitData = s.font_data or skin.font_data },
      export = true,
   }
end




local resolved = setmetatable({}, { __mode = "k" })
function M.color(s, name, half)
   local decl = resolved[s]
   if not decl then
      decl = M.tokens(s)
      resolved[s] = decl
   end
   local c = decl.colors[name]
   return c and c[half] or 0
end


function M.key(s)
   local parts = { s.skin.name, s.density, tostring(s.shadows), tostring(s.radius),
tostring(s.appearance), tostring(s.hair), tostring(s.rule_w), tostring(s.shimmer),
s.font_ui and table.concat(s.font_ui, ",") or "", s.font_data and table.concat(s.font_data, ",") or "", }
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