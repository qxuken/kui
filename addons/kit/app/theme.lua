-- The Theme panel: Kit's settings, tweaked live. Everything here lowers
-- to one `kit.settings({...})` call, the one an app of your own places —
-- so every card shows what that app's theme would look like, and the
-- panel prints the call to copy.

local kit = require("kit.ui")

local M = {}

local SKINS = {
  { value = "ledger", label = "Ledger" },
  { value = "phosphor", label = "Phosphor" },
  { value = "neutral", label = "Neutral" },
}

-- Accents to pick from; the skin's own is the first swatch.
local ACCENTS = {
  { label = "Blue", color = 0x2f6fedff },
  { label = "Indigo", color = 0x4f46e5ff },
  { label = "Violet", color = 0x8b5cf6ff },
  { label = "Pink", color = 0xdb2777ff },
  { label = "Red", color = 0xdc2626ff },
  { label = "Orange", color = 0xea580cff },
  { label = "Yellow", color = 0xffd400ff },
  { label = "Green", color = 0x16a34aff },
  { label = "Teal", color = 0x0d9488ff },
}

-- The faces, each a family list; "skin" is the skin's own.
local UI_FACES = {
  { value = "skin", label = "The skin's" },
  { value = "Inter", label = "Inter", list = { "Inter", "sans" } },
  { value = "IBM Plex Sans", label = "IBM Plex Sans", list = { "IBM Plex Sans", "sans" } },
  { value = "Space Grotesk", label = "Space Grotesk", list = { "Space Grotesk", "sans" } },
  { value = "Helvetica Neue", label = "Helvetica Neue", list = { "Helvetica Neue", "Helvetica", "sans" } },
  { value = "sans", label = "System sans", list = { "sans" } },
  { value = "serif", label = "Serif", list = { "serif" } },
}

local DATA_FACES = {
  { value = "skin", label = "The skin's" },
  { value = "IBM Plex Mono", label = "IBM Plex Mono", list = { "IBM Plex Mono", "mono" } },
  { value = "JetBrains Mono", label = "JetBrains Mono", list = { "JetBrains Mono", "mono" } },
  { value = "Menlo", label = "Menlo", list = { "Menlo", "mono" } },
  { value = "mono", label = "System mono", list = { "mono" } },
}

-- Each skin's own lengths (src/ledger.tl, phosphor.tl, neutral.tl), for a
-- slider to start from. A script reads Kit's values in no other way:
-- `env.tokens` lists its own origin's tokens and the host's, not another
-- origin's exported ones, though `$kitRadius` resolves in any prop.
local SKIN_LENGTHS = {
  ledger = { radius = 0, hair = 1, rule = 2 },
  phosphor = { radius = 0, hair = 1, rule = 1 },
  neutral = { radius = 6, hair = 1, rule = 1 },
}

-- The colour roles worth setting by hand, and what each paints.
local COLORS = {
  { role = "bg", label = "Window" },
  { role = "side", label = "Sidebar" },
  { role = "panel", label = "Panel" },
  { role = "sunken", label = "Well" },
  { role = "fg", label = "Text" },
  { role = "muted", label = "Muted text" },
  { role = "rule", label = "Hairline" },
  { role = "ink", label = "Rule" },
  { role = "sel", label = "Selection" },
  { role = "focus", label = "Focus ring" },
  { role = "ok", label = "OK" },
  { role = "warn", label = "Warning" },
  { role = "danger", label = "Danger" },
}

local function list_of(faces, value)
  for _, f in ipairs(faces) do
    if f.value == value then return f.list end
  end
  return nil
end

-- The faces as a select's options.
local function choices(faces)
  local out = {}
  for i, f in ipairs(faces) do out[i] = { value = f.value, label = f.label } end
  return out
end

-- `sel_fg` → `$kitSelFg`: the token a role paints under.
local function token(role)
  return "kit" .. role:gsub("^%l", string.upper):gsub("_(%l)", string.upper)
end

-- The theme as it starts, and as Reset leaves it: the skin's own all
-- through.
function M.defaults()
  return {
    skin = "ledger", appearance = "system", density = "comfortable", shadows = true,
    accent = nil, radius = nil, hair = nil, rule = nil, font_ui = "skin", font_data = "skin",
    -- Colours by role, each `{ light, dark }` with either half left out;
    -- and what is typed in a colour's field, valid or not yet.
    colors = {}, typed = {},
  }
end

-- `#rrggbb` (or without the `#`) as 0xRRGGBBff; nil for anything else.
local function parse(s)
  local hex = s:match("^%s*#?(%x%x%x%x%x%x)%s*$")
  return hex and (tonumber(hex, 16) << 8 | 0xff) or nil
end

local function hex(c)
  return string.format("#%06x", (c >> 8) & 0xffffff)
end

-- What `kit.settings` takes for this theme.
function M.settings(t)
  local roles = nil
  for role, c in pairs(t.colors) do
    roles = roles or {}
    roles[role] = { light = c.light, dark = c.dark }
  end
  if t.accent then
    roles = roles or {}
    roles.accent = t.accent
  end
  return {
    skin = t.skin, appearance = t.appearance, density = t.density, shadows = t.shadows,
    radius = t.radius, hair = t.hair, rule = t.rule,
    font_ui = list_of(UI_FACES, t.font_ui), font_data = list_of(DATA_FACES, t.font_data),
    roles = roles,
  }
end

local function lua_list(l)
  local q = {}
  for i, s in ipairs(l) do q[i] = string.format("%q", s) end
  return "{ " .. table.concat(q, ", ") .. " }"
end

-- The theme as the Lua that places it, with only what differs from the
-- skin's own.
function M.lua(t)
  local lines = { "kit.settings({", string.format("  skin = %q,", t.skin) }
  local function add(s) lines[#lines + 1] = "  " .. s end
  if t.appearance ~= "system" then add(string.format("appearance = %q,", t.appearance)) end
  if t.density ~= "comfortable" then add(string.format("density = %q,", t.density)) end
  if not t.shadows then add("shadows = false,") end
  if t.radius then add(string.format("radius = %g,", t.radius)) end
  if t.hair then add(string.format("hair = %g,", t.hair)) end
  if t.rule then add(string.format("rule = %g,", t.rule)) end
  local ui, data = list_of(UI_FACES, t.font_ui), list_of(DATA_FACES, t.font_data)
  if ui then add("font_ui = " .. lua_list(ui) .. ",") end
  if data then add("font_data = " .. lua_list(data) .. ",") end
  local roles = {}
  if t.accent then roles[#roles + 1] = string.format("    accent = 0x%08x,", t.accent) end
  for _, c in ipairs(COLORS) do
    local v = t.colors[c.role]
    if v and (v.light or v.dark) then
      local halves = {}
      if v.light then halves[#halves + 1] = string.format("light = 0x%08x", v.light) end
      if v.dark then halves[#halves + 1] = string.format("dark = 0x%08x", v.dark) end
      roles[#roles + 1] = string.format("    %s = { %s },", c.role, table.concat(halves, ", "))
    end
  end
  if #roles > 0 then
    add("roles = {")
    for _, r in ipairs(roles) do lines[#lines + 1] = r end
    add("},")
  end
  lines[#lines + 1] = "})"
  return table.concat(lines, "\n")
end

function M.update(m, msg)
  local t = m.theme
  local k = msg.kind
  if k == "theme_open" then
    m.theme_open = not m.theme_open
  elseif k == "theme_reset" then
    m.theme = M.defaults()
  elseif k == "theme" then
    local f = msg.field
    if msg.toggle then t[f] = msg.toggle.on
    elseif msg.change and msg.change.value ~= nil then t[f] = msg.change.value
    end
  elseif k == "theme_number" then
    -- A slider at the skin's own value says nothing.
    local v = msg.change.value
    t[msg.field] = v
  elseif k == "theme_skin_value" then
    t[msg.field] = nil
  elseif k == "theme_accent" then
    t.accent = msg.color
    t.typed.accent = nil
  elseif k == "theme_accent_text" then
    t.typed.accent = msg.change.text
    local c = parse(msg.change.text)
    if c then t.accent = c end
  elseif k == "theme_color" then
    local key = msg.role .. "/" .. msg.half
    t.typed[key] = msg.change.text
    local c = parse(msg.change.text)
    if c then
      t.colors[msg.role] = t.colors[msg.role] or {}
      t.colors[msg.role][msg.half] = c
    end
  elseif k == "theme_colors" then
    m.theme_colors = not msg.toggle.collapsed
  elseif k == "theme_color_reset" then
    local c = t.colors[msg.role]
    if c then c[msg.half] = nil end
    t.typed[msg.role .. "/" .. msg.half] = nil
  end
end

local function caption(s)
  return text(s, { size = 11, family = "$kitUi", color = "$kitMuted" })
end

-- A setting: its name over its control.
local function setting(name, control)
  return column { width = "grow", gap = 4, caption(name), control }
end

-- A colour square to pick; `chosen` rings it. `color` nil is the skin's
-- own, drawn as an empty square with a mark in it.
local function swatch(color, name, chosen, msg)
  return row { role = "radio", label = name, checked = chosen, tooltip = name,
    width = 22, height = 22, radius = "$kitRadius", bg = color or "$kitPanel", cursor = "pointer", focusable = true,
    main_align = "center", cross_align = "center",
    border = { w = chosen and 2 or 1, color = chosen and "$kitFocus" or "$kitRule" },
    on_click = msg,
    not color and text("A", { size = 10, bold = true, family = "$kitUi", color = "$kitMuted" }) or nil }
end

-- A slider over a length the skin has its own value of (`own`, what the
-- token says while nothing is set): the skin's until it is moved, and a
-- button that gives it back.
local function length(t, field, name, lo, hi, step, own)
  local v = t[field]
  return row { width = "grow", gap = 8, cross_align = "end",
    column { width = "grow",
      kit.slider({ label = name, value = v or own, min = lo, max = hi, step = step,
        value_label = v and string.format("%g px", v) or string.format("the skin's, %g px", own),
        on_change = { kind = "theme_number", field = field } }) },
    kit.icon_button({ icon = "reset", label = "The skin's " .. name:lower(), disabled = v == nil,
      on_press = { kind = "theme_skin_value", field = field } }) }
end

-- One colour role: its swatch (the colour in force, by its token), its
-- name, the colour set for the half on screen as a field to type into,
-- and a button that gives back the skin's.
local function color_row(t, c, half)
  local key = c.role .. "/" .. half
  local own = t.colors[c.role] and t.colors[c.role][half]
  local typed = t.typed[key]
  return row { width = "grow", gap = 8, cross_align = "center",
    column { width = 18, height = 18, radius = "$kitRadius", border = { w = 1, color = "$kitRule" }, bg = "$" .. token(c.role) },
    column { width = 84, text(c.label, { size = "$kitSizeLabel", family = "$kitUi", color = "$kitFg", max_lines = 1 }) },
    column { width = "grow", min_width = 0,
      kit.text_field({ label = c.label .. " (" .. half .. ")", hide_label = true, mono = true,
        value = typed or (own and hex(own)) or "", placeholder = "the skin's",
        error = typed and not parse(typed) and "#rrggbb" or nil,
        on_change = { kind = "theme_color", role = c.role, half = half } }) },
    kit.icon_button({ icon = "reset", label = "The skin's " .. c.label:lower(), disabled = own == nil,
      on_press = { kind = "theme_color_reset", role = c.role, half = half } }) }
end

-- The panel. `env` says which half of the colours is on screen: the half
-- a colour typed here is set for.
function M.view(m, env)
  local t = m.theme
  local half = t.appearance ~= "system" and t.appearance
    or (env.theme and env.theme.appearance == "light" and "light" or "dark")
  local lengths = SKIN_LENGTHS[t.skin] or SKIN_LENGTHS.ledger
  local swatches = { swatch(nil, "The skin's accent", t.accent == nil, { kind = "theme_accent" }) }
  for _, a in ipairs(ACCENTS) do
    swatches[#swatches + 1] = swatch(a.color, "Accent: " .. a.label, t.accent == a.color,
      { kind = "theme_accent", color = a.color })
  end
  local colors = {}
  for _, c in ipairs(COLORS) do colors[#colors + 1] = color_row(t, c, half) end
  return column { width = "grow", gap = 14,
    row { width = "grow", cross_align = "center", gap = 8,
      text("Theme", { size = "$kitSizeTitle", bold = true, family = "$kitUi", color = "$kitFg" }),
      row { width = "grow" },
      kit.button({ label = "Reset", icon = "reset", variant = "ghost", on_press = { kind = "theme_reset" } }),
      kit.icon_button({ icon = "x", label = "Close the theme", on_press = { kind = "theme_open" } }) },
    text("Every card follows what is set here. It is one kit.settings call, printed at the end to copy into an app.",
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" }),
    setting("Skin", kit.segmented({ label = "Skin", options = SKINS, value = t.skin,
      on_change = { kind = "theme", field = "skin" } })),
    setting("Appearance", kit.segmented({ label = "Appearance", value = t.appearance,
      on_change = { kind = "theme", field = "appearance" },
      options = { { value = "system", label = "System" }, { value = "light", label = "Light" }, { value = "dark", label = "Dark" } } })),
    setting("Density", kit.segmented({ label = "Density", value = t.density, on_change = { kind = "theme", field = "density" },
      options = { { value = "comfortable", label = "Comfortable" }, { value = "compact", label = "Compact" } } })),
    kit.switch({ label = "Shadows", on = t.shadows, on_toggle = { kind = "theme", field = "shadows" } }),
    setting("Accent", column { width = "grow", gap = 8,
      row { role = "radioGroup", label = "Accent", gap = 6, cross_gap = 6, wrap_children = true, table.unpack(swatches) },
      kit.text_field({ label = "Accent colour", hide_label = true, mono = true,
        value = t.typed.accent or (t.accent and hex(t.accent)) or "", placeholder = "the skin's, or #rrggbb",
        error = t.typed.accent and not parse(t.typed.accent) and "#rrggbb" or nil,
        on_change = { kind = "theme_accent_text" } }) }),
    length(t, "radius", "Radius", 0, 12, 1, lengths.radius),
    length(t, "hair", "Hairline", 0.5, 3, 0.5, lengths.hair),
    length(t, "rule", "Region rule", 0, 4, 0.5, lengths.rule),
    kit.select({ label = "UI face", value = t.font_ui, options = choices(UI_FACES),
      on_change = { kind = "theme", field = "font_ui" } }),
    kit.select({ label = "Data face", value = t.font_data, options = choices(DATA_FACES),
      on_change = { kind = "theme", field = "font_data" } }),
    kit.section({ title = "Colours, " .. half, collapsed = not m.theme_colors, on_toggle = { kind = "theme_colors" } },
      { column { width = "grow", gap = 6, table.unpack(colors) } }),
    kit.code({ label = "Your theme", text = M.lua(t), copy = true }),
  }
end

return M

