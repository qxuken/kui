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



local M = { Half = Half, Skin = Skin, Settings = Settings, ROLES = ROLES, AREA_ALPHA = 0.14,
TEXT_CONTRAST = 4.5, GRAPHIC_CONTRAST = 3.0, }


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






local function shades(colors, half, fill, value)
   local toward = TOWARD[fill]
   if toward then
      local t = colors["kit" .. (toward == "on_accent" and "OnAccent" or "OnDanger")][half]
      if contrast(t, mix(value, t, 0.30)) < M.TEXT_CONTRAST then
         t = luminance(t) > 0.18 and 0x000000ff or 0xffffffff
      end
      return mix(value, t, 0.16), mix(value, t, 0.30)
   end
   return over(value, colors.kitHover[half]), over(value, colors.kitPress[half])
end


local function hsl(c)
   local r, g, b = channels(c)
   local R, G, B = r / 255, g / 255, b / 255
   local hi, lo = math.max(R, G, B), math.min(R, G, B)
   local l = (hi + lo) / 2
   if hi == lo then return 0, 0, l end
   local d = hi - lo
   local s = l > 0.5 and d / (2 - hi - lo) or d / (hi + lo)
   local h
   if hi == R then h = (G - B) / d + (G < B and 6 or 0)
   elseif hi == G then h = (B - R) / d + 2
   else h = (R - G) / d + 4 end
   return h * 60, s, l
end

local function from_hsl(h, s, l, alpha)
   local function f(n)
      local k = (n + h / 30) % 12
      local x = l - s * math.min(l, 1 - l) * math.max(-1, math.min(k - 3, 9 - k, 1))
      return math.floor(math.max(0, math.min(1, x)) * 255 + 0.5)
   end
   return (f(0) << 24) | (f(8) << 16) | (f(4) << 8) | alpha
end


local function chroma(c)
   local r, g, b = channels(c)
   return (math.max(r, g, b) - math.min(r, g, b)) / 255
end

local function hue_gap(a, b)
   local d = math.abs(a - b) % 360
   return d > 180 and 360 - d or d
end

local function rgb(c) return c & ~0xff end



local function legible(c, ground, target)
   if contrast(c, ground) >= target then return c end
   local h, s, l = hsl(c)
   local darker = luminance(ground) > 0.18
   local out = c
   for _ = 1, 60 do
      l = darker and math.max(0, l - 0.02) or math.min(1, l + 0.02)
      out = from_hsl(h, s, l, c & 0xff)
      if contrast(out, ground) >= target or l <= 0 or l >= 1 then break end
   end
   return out
end


local function legible_on(c, grounds, target)
   for _ = 1, 3 do
      for _, g in ipairs(grounds) do c = legible(c, g, target) end
   end
   return c
end


local function best_on(fill, options)
   local pick, most = options[1], -1.0
   for _, o in ipairs(options) do
      local k = contrast(fill, o)
      if k > most then pick, most = o, k end
   end
   return pick
end




local REHUE = {
   "bg", "side", "panel", "raised", "sunken", "rule", "ink", "fg", "muted", "faint",
   "sel", "sel_fg", "focus", "hover", "press",
}

local TINT = { bg = 0.05, side = 0.06, panel = 0.03, raised = 0.03, sunken = 0.07, rule = 0.12 }









local function derive(s, colors, half, h)
   local o = s.roles and s.roles.accent
   if o == nil or type(o) == "string" then return end
   local named = s.roles
   local function set(role, v)
      if not named[role] then colors["kit" .. camel(role)][half] = v end
   end
   local A = rgb(colors.kitAccent[half]) | 0xff
   local A0 = h.accent
   local ah, as = hsl(A)
   local h0, s0 = hsl(A0)
   if rgb(A) ~= rgb(A0) then
      for _, role in ipairs(REHUE) do
         local c = h[role]
         local alpha = c & 0xff
         local v = nil
         if rgb(c) == rgb(A0) then
            v = rgb(A) | alpha
         elseif chroma(c) >= 0.08 and hue_gap((hsl(c)), h0) <= 50 then
            local _, cs, cl = hsl(c)
            v = from_hsl(ah, math.min(1, cs * as / math.max(s0, 0.05)), cl, alpha)
         elseif TINT[role] then
            v = rgb(mix(c, A, TINT[role])) | alpha
         elseif role == "hover" or role == "press" then
            v = rgb(mix(c | 0xff, A, 0.5)) | alpha
         end
         if v then set(role, v) end
      end
   end
   local function get(role) return colors["kit" .. camel(role)][half] end
   local panel, bg, side, sunken, raised = get("panel"), get("bg"), get("side"), get("sunken"), get("raised")
   local grounds = { panel, bg, side, sunken, raised }
   local black, white = 0x111111ff, 0xffffffff

   set("on_accent", best_on(A, { h.fg, h.bg, black, white }))
   local sel = get("sel")
   local sel_ground = (sel & 0xff) == 0xff and sel or over(panel, sel)
   if (h.sel_fg) == (h.on_accent) and (sel & 0xff) == 0xff then
      set("sel_fg", best_on(sel_ground, { get("on_accent"), h.fg, h.bg, black, white }))
   else
      set("sel_fg", legible(get("sel_fg"), sel_ground, M.TEXT_CONTRAST))
   end


   local series = colors.kitSeries1
   if series and not named.series1 then
      series[half] = legible_on(A, { panel, bg }, M.GRAPHIC_CONTRAST)
      colors.kitArea1[half] = M.faded(series[half], AREA_ALPHA)
   end
   local group = colors.kitGroup1
   if group then
      local fg = get("fg")
      local t = luminance(panel) > 0.18 and 0.28 or 0.4
      local g = mix(panel, A, t)
      while contrast(fg, g) < M.TEXT_CONTRAST and t > 0.02 do
         t = t - 0.02
         g = mix(panel, A, t)
      end
      group[half] = g
   end

   colors.kitChosen[half] = A
   colors.kitChosenFg[half] = get("on_accent")
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
      local ls, ds = skin.light.series[i], skin.dark.series[i]
      colors["kitSeries" .. i] = { light = ls, dark = ds }


      colors["kitArea" .. i] = { light = M.faded(ls, AREA_ALPHA), dark = M.faded(ds, AREA_ALPHA) }
   end


   local outlined = skin.ctl_border
   colors.kitChosen = { light = colors[outlined and "kitInk" or "kitPanel"].light, dark = colors[outlined and "kitInk" or "kitPanel"].dark }
   colors.kitChosenFg = { light = colors[outlined and "kitBg" or "kitFg"].light, dark = colors[outlined and "kitBg" or "kitFg"].dark }
   derive(s, colors, "light", l)
   derive(s, colors, "dark", d)









   for _, half in ipairs({ "light", "dark" }) do
      local function at(name) return colors[name][half] end
      local grounds = { at("kitPanel"), at("kitBg"), at("kitSide"), at("kitSunken"), at("kitRaised") }
      local washed = {}
      for i = 1, 5 do
         local pressed = over(grounds[i], at("kitPress"))
         washed[#washed + 1] = grounds[i]
         washed[#washed + 1] = over(grounds[i], at("kitHover"))
         washed[#washed + 1] = pressed

         washed[#washed + 1] = over(pressed, at("kitHover"))
         washed[#washed + 1] = over(pressed, at("kitPress"))
      end
      for _, role in ipairs({ "fg", "muted", "faint", "focus", "ok", "warn", "danger" }) do
         if not (s.roles and s.roles[role]) then
            local slot = colors["kit" .. camel(role)]

            slot[half] = legible_on(slot[half], role == "focus" and grounds or washed, M.TEXT_CONTRAST)
         end
      end
   end



   local function accent_text(half)
      local function at(name) return colors[name][half] end
      local grounds = { at("kitPanel"), at("kitBg"), at("kitSide"), at("kitSunken"), at("kitRaised") }
      for i = 1, 5 do
         local pressed = over(grounds[i], at("kitPress"))
         grounds[#grounds + 1] = over(grounds[i], at("kitHover"))
         grounds[#grounds + 1] = pressed
         grounds[#grounds + 1] = over(pressed, at("kitHover"))
         grounds[#grounds + 1] = over(pressed, at("kitPress"))
      end
      return legible_on(rgb(at("kitAccent")) | 0xff, grounds, M.TEXT_CONTRAST)
   end
   colors.kitAccentText = { light = accent_text("light"), dark = accent_text("dark") }
   local text_role = s.roles and s.roles.accent_text
   if math.type(text_role) == "integer" or type(text_role) == "table" then
      local lv, dv = half_of(text_role, colors.kitAccentText.light, colors.kitAccentText.dark)
      colors.kitAccentText = { light = lv, dark = dv }
   end
   for _, fill in ipairs(SHADED) do


      local base = colors["kit" .. camel(fill)]
      local lh, lp = shades(colors, "light", fill, base.light)
      local dh, dp = shades(colors, "dark", fill, base.dark)
      colors["kit" .. camel(fill) .. "Hover"] = { light = lh, dark = dh }
      colors["kit" .. camel(fill) .. "Press"] = { light = lp, dark = dp }
   end



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
tostring(s.appearance), tostring(s.hair), tostring(s.rule_w),
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