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

-- Each skin's own faces, as its font lists name them (addons/kit/src/
-- ledger.tl, phosphor.tl, neutral.tl): listed first where installed, and
-- named where not, since a family that is not installed draws as the
-- system's sans.
local SKIN_FACES = {
  ledger = { "Space Grotesk", "IBM Plex Sans", "IBM Plex Mono" },
  phosphor = { "IBM Plex Mono", "JetBrains Mono" },
  neutral = { "Inter", "JetBrains Mono", "IBM Plex Mono" },
}

-- The system's own faces, by the names a family list takes.
local GENERIC = { sans = "System sans", serif = "System serif", mono = "System mono" }

-- Every family the core can see, installed or loaded from a file, sorted:
-- asked once a run, and again after a file adds one (the door reads the
-- font database each call, and this view runs on every message).
local all_families = nil
-- The families a file loaded here brought, for their badge, and how often
-- that list changed.
local from_file, files_rev = {}, 0
-- What the list showed last, by row: a pick's row is read back against it.
local shown = {}
local shown_key, shown_rows = nil, nil

local function families(env)
  if not all_families then all_families = env.system_font_families() end
  return all_families
end

-- The families as the list shows them, filtered by `filter` (a part of the
-- name, any case): the skin's installed faces first, then those a file
-- brought, then the rest in order; and as a channel, made again only when
-- the filter or the families change.
local function family_rows(env, skin, filter)
  local all = families(env)
  local key = table.concat({ skin, filter, #all, files_rev }, "\0")
  if key == shown_key then return shown_rows end
  local have = {}
  for _, f in ipairs(all) do have[f] = true end
  local want = filter:lower()
  local function match(f) return want == "" or f:lower():find(want, 1, true) ~= nil end
  local rows = { family = {}, badge = {}, tone = {} }
  local placed = {}
  local function add(f, badge, tone)
    if placed[f] or not match(f) then return end
    placed[f] = true
    local n = #rows.family + 1
    rows.family[n], rows.badge[n], rows.tone[n] = f, badge, tone
  end
  for _, f in ipairs(SKIN_FACES[skin] or {}) do
    if have[f] then add(f, "skin", "accent") end
  end
  for _, f in ipairs(all) do
    if from_file[f] then add(f, "file", "info") end
  end
  -- macOS's own interface faces (`.SF NS Mono`) are the system's to use,
  -- not an app's: left out, as a font menu leaves them.
  for _, f in ipairs(all) do
    if f:sub(1, 1) ~= "." then add(f, "", "neutral") end
  end
  local listed = 0
  for _, f in ipairs(all) do
    if f:sub(1, 1) ~= "." then listed = listed + 1 end
  end
  shown, shown_key, shown_rows = rows.family, key, { ch = channel(rows), n = #rows.family, of = listed }
  return shown_rows
end

-- The skin's faces this machine has not got.
local function missing_faces(env, skin)
  local have = {}
  for _, f in ipairs(families(env)) do have[f] = true end
  local out = {}
  for _, f in ipairs(SKIN_FACES[skin] or {}) do
    if not have[f] then out[#out + 1] = f end
  end
  return out
end

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

-- A face's family list as `kit.settings` takes it: nil for the skin's, a
-- generic alone, or a family with the generic after it, which draws where
-- the family is not installed.
local function face_list(value, generic)
  if value == nil or value == "skin" then return nil end
  if GENERIC[value] then return { value } end
  return { value, generic }
end

-- A face select's options: the skin's (named by the family it resolved
-- to), the system's generic ones, and the family picked from the list.
local function choices(generics, skin_picked, chosen)
  local out = { { value = "skin", label = skin_picked and ("The skin's (" .. skin_picked .. ")") or "The skin's" } }
  for _, g in ipairs(generics) do out[#out + 1] = { value = g, label = GENERIC[g] } end
  if chosen ~= "skin" and not GENERIC[chosen] then out[#out + 1] = { value = chosen, label = chosen } end
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
    -- The family list: which face a pick sets, the filter, and a font file
    -- loaded from disk (its path as typed or chosen).
    font_target = "ui", font_filter = "", font_file = nil,
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
    font_ui = face_list(t.font_ui, "sans"), font_data = face_list(t.font_data, "mono"),
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
  local ui, data = face_list(t.font_ui, "sans"), face_list(t.font_data, "mono")
  if ui then add("font_ui = " .. lua_list(ui) .. ",") end
  if data then add("font_data = " .. lua_list(data) .. ",") end
  -- A family a file brought is the app's to load too, from its view.
  if t.font_file and (from_file[t.font_ui] or from_file[t.font_data]) then
    table.insert(lines, 1, string.format("-- env.load_font_file(%q) first, in the view", t.font_file))
  end
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
  elseif k == "theme_font_filter" then
    t.font_filter = msg.change.text
  elseif k == "theme_font_pick" then
    local f = shown[(msg.select.row or -1) + 1]
    if f then t[t.font_target == "data" and "font_data" or "font_ui"] = f end
  elseif k == "theme_font_file" then
    t.font_file = msg.change.text ~= "" and msg.change.text or nil
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
local function color_row(t, c, half, colors)
  local key = c.role .. "/" .. half
  local own = t.colors[c.role] and t.colors[c.role][half]
  local typed = t.typed[key]
  local now = colors[token(c.role)]
  return row { width = "grow", gap = 8, cross_align = "center",
    column { width = 18, height = 18, radius = "$kitRadius", border = { w = 1, color = "$kitRule" }, bg = "$" .. token(c.role) },
    column { width = 84, text(c.label, { size = "$kitSizeLabel", family = "$kitUi", color = "$kitFg", max_lines = 1 }) },
    column { width = "grow", min_width = 0,
      kit.text_field({ label = c.label .. " (" .. half .. ")", hide_label = true, mono = true,
        value = typed or (now and hex(now)) or "",
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
  -- Kit's tokens as they resolve now: its colours (0xRRGGBBAA) and lengths.
  local tokens = env.tokens or {}
  local now, lengths, fams = tokens.colors or {}, tokens.lengths or {}, tokens.families or {}
  -- A font file named here is loaded (the same file again is the same
  -- family), and the family it brought joins the list.
  local file_family, file_err = nil, nil
  if t.font_file then
    file_family, file_err = env.load_font_file(t.font_file)
    if file_family and not from_file[file_family] then
      from_file[file_family] = true
      files_rev = files_rev + 1
      all_families = nil
    end
  end
  local list = family_rows(env, t.skin, t.font_filter)
  local target = t.font_target == "data" and t.font_data or t.font_ui
  local picked = nil
  for i, f in ipairs(shown) do
    if f == target then picked = i - 1 end
  end
  local missing = missing_faces(env, t.skin)
  local swatches = { swatch(nil, "The skin's accent", t.accent == nil, { kind = "theme_accent" }) }
  for _, a in ipairs(ACCENTS) do
    swatches[#swatches + 1] = swatch(a.color, "Accent: " .. a.label, t.accent == a.color,
      { kind = "theme_accent", color = a.color })
  end
  local colors = {}
  for _, c in ipairs(COLORS) do colors[#colors + 1] = color_row(t, c, half, now) end
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
        value = t.typed.accent or (now.kitAccent and hex(now.kitAccent)) or "",
        error = t.typed.accent and not parse(t.typed.accent) and "#rrggbb" or nil,
        on_change = { kind = "theme_accent_text" } }) }),
    length(t, "radius", "Radius", 0, 12, 1, lengths.kitRadius or 0),
    length(t, "hair", "Hairline", 0.5, 3, 0.5, lengths.kitHair or 1),
    length(t, "rule", "Region rule", 0, 4, 0.5, lengths.kitRuleW or 1),
    kit.select({ label = "UI face", value = t.font_ui, on_change = { kind = "theme", field = "font_ui" },
      options = choices({ "sans", "serif" }, t.font_ui == "skin" and fams.kitUi or nil, t.font_ui) }),
    kit.select({ label = "Data face", value = t.font_data, on_change = { kind = "theme", field = "font_data" },
      options = choices({ "mono" }, t.font_data == "skin" and fams.kitData or nil, t.font_data) }),
    setting("Families", column { width = "grow", gap = 6,
      kit.segmented({ label = "A pick sets", value = t.font_target, on_change = { kind = "theme", field = "font_target" },
        options = { { value = "ui", label = "UI face" }, { value = "data", label = "Data face" } } }),
      kit.search({ label = "Filter the families", placeholder = "Filter families", value = t.font_filter,
        count = list.n < list.of and string.format("%d of %d", list.n, list.of) or tostring(list.of),
        on_change = { kind = "theme_font_filter" } }),
      column { width = "grow", height = 200,
        kit.list({ rows = list.ch, title = "family", badge = "badge", tone = "tone", badge_width = 36,
          label = "Families", selected = picked, grow = true, on_select = { kind = "theme_font_pick" } }) },
      kit.text_field({ label = "Font file", hide_label = true, mono = true, browse = "open", browse_label = "Load…",
        placeholder = "A .ttf, .otf or .ttc", value = t.font_file or "", error = file_err,
        on_change = { kind = "theme_font_file" } }),
      file_family and caption("Loaded " .. file_family .. ": pick it above.") or nil,
      text(#missing > 0
          and string.format("Kit ships no font files: the skin's %s %s not installed here, and draws in the system's face instead.",
            table.concat(missing, ", "), #missing > 1 and "are" or "is")
          or "Kit ships no font files: these are the families installed here, and any a file loaded.",
        { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" }) }),
    kit.section({ title = "Colours, " .. half, collapsed = not m.theme_colors, on_toggle = { kind = "theme_colors" } },
      { column { width = "grow", gap = 6, table.unpack(colors) } }),
    kit.code({ label = "Your theme", text = M.lua(t), copy = true }),
  }
end

return M

