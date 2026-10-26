-- The Design system pages: Kit describing itself from the files under
-- design/ — the brand book (design/README.md), every colour role's usage
-- note (design/tokens.json) and a guideline per component
-- (design/components/<Component>.md) — with every value read live from Kit
-- as the Theme panel sets it (`env.tokens`), and the components on the
-- other pages as the previews.

local kit = require("kit.ui")

local M = {}

-- A file of the app's own, read once (nil when it cannot be).
local read_cache = {}
local function read(path)
  local t = read_cache[path]
  if t == nil then
    local ok, s = pcall(kui.fs.read, path)
    t = ok and s or false
    read_cache[path] = t
  end
  return t or nil
end

local function muted(s)
  return text(s, { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

local function lead(s)
  return text(s, { size = "$kitSize", family = "$kitUi", color = "$kitFg", wrap = "word" })
end

-- tokens.json, decoded once: the colour roles by name, each with its
-- usage note, in the file's order.
local function tokens()
  local t = read_cache["design/tokens.json:decoded"]
  if t == nil then
    local s = read("design/tokens.json")
    local ok, d = pcall(kui.json.decode, s or "")
    t = ok and d or false
    read_cache["design/tokens.json:decoded"] = t
  end
  return t or nil
end
M.tokens = tokens

-- `kitSelFg` ↔ `sel-fg`, `kitGroup1` ↔ `group-1`: a Kit token's name as
-- tokens.json spells it.
function M.role_of(token)
  local s = token:gsub("^kit", "")
  s = s:gsub("(%l)(%u)", "%1-%2"):gsub("(%a)(%d)", "%1-%2"):lower()
  return s
end

-- The colour tokens Kit exports now, by role name, as integers.
function M.live_colors(env)
  local out = {}
  local t = env.tokens and env.tokens.colors or {}
  for name, v in pairs(t) do
    if name:match("^kit") then out[M.role_of(name)] = v end
  end
  return out
end

-- WCAG's contrast of two 0xRRGGBBAA colours.
local function lum(c)
  local function lin(x)
    local v = x / 255
    return v <= 0.04045 and v / 12.92 or ((v + 0.055) / 1.055) ^ 2.4
  end
  return 0.2126 * lin((c >> 24) & 255) + 0.7152 * lin((c >> 16) & 255) + 0.0722 * lin((c >> 8) & 255)
end

local function contrast(a, b)
  local x, y = lum(a), lum(b)
  if x < y then x, y = y, x end
  return (x + 0.05) / (y + 0.05)
end

-- `top` laid over the opaque `under` by its alpha.
local function over(under, top)
  local a = (top & 255) / 255
  local function ch(s) return math.floor(((under >> s) & 255) * (1 - a) + ((top >> s) & 255) * a + 0.5) end
  return (ch(24) << 24) | (ch(16) << 16) | (ch(8) << 8) | 255
end

local function hex(c) return string.format("#%08x", c) end

-- The roles drawn as text, and the grounds a note can name for them.
local TEXT = {
  fg = true, muted = true, faint = true, focus = true, ok = true, warn = true, danger = true,
  ["accent-text"] = true, ["sel-fg"] = true, ["on-accent"] = true, ["on-danger"] = true, ["chosen-fg"] = true,
}
local GROUNDS = { "bg", "side", "panel", "raised", "sunken", "hover", "press", "sel", "accent", "danger", "chosen" }

-- The grounds a text role's note names, each as Kit draws it now: a wash
-- or a translucent selection over the panel.
local function grounds_of(role, note, live)
  local out = {}
  local g = role:match("^group%-fg%-(%d)$")
  if g then return { { "group-" .. g, live["group-" .. g] } } end
  local words = {}
  for w in (note or ""):gmatch("[%w%-]+") do words[w] = true end
  if role == "sel-fg" then words = { sel = true } end
  if role == "on-accent" then words = { accent = true } end
  if role == "on-danger" then words = { danger = true } end
  if role == "chosen-fg" then words = { chosen = true } end
  for _, name in ipairs(GROUNDS) do
    local v = live[name]
    if words[name] and v then
      if (v & 255) < 255 then v = over(live.panel or 0xffffffff, v) end
      out[#out + 1] = { name, v }
    end
  end
  return out
end

-- A swatch: the colour over the panel, framed in the rule.
local function swatch(c)
  return column { width = 36, height = 24, radius = "$kitRadius", bg = c,
    border = { w = 1, color = "$kitRule" } }
end

-- One colour role: its swatch, name, live value and note, and for a text
-- role its contrast on each ground its note names.
local function colour_row(name, v, note, live)
  local t = { width = "grow", gap = 12, cross_align = "start", pad = { y = 4 },
    swatch(v),
    column { width = 150, gap = 2,
      text(name, { size = "$kitSize", family = "$kitData", color = "$kitFg", bold = true }),
      text(hex(v), { size = 11, family = "$kitData", color = "$kitMuted" }) },
  }
  local right = { width = "grow", min_width = 0, gap = 4, muted(note or "No usage note in design/tokens.json.") }
  if TEXT[name] or name:match("^group%-fg%-") then
    local chips = { gap = 6, cross_gap = 4, wrap_children = true }
    for _, g in ipairs(grounds_of(name, note, live)) do
      local k = contrast(v, g[2])
      chips[#chips + 1] = row { gap = 4, pad = { x = 6, y = 2 }, radius = "$kitRadius", bg = g[2], cross_align = "center",
        text(string.format("%.1f:1 on %s", k, g[1]), { size = 11, family = "$kitData", color = v }) }
    end
    if #chips > 0 then right[#right + 1] = row(chips) end
  end
  t[#t + 1] = column(right)
  return row(t)
end

function M.colours(m, env)
  local live = M.live_colors(env)
  local d = tokens()
  local notes, order, seen = {}, {}, {}
  for _, t in ipairs(d and d.color and d.color.tokens or {}) do
    notes[t.name] = t.usage
    if live[t.name] then order[#order + 1] = t.name; seen[t.name] = true end
  end
  -- A role Kit exports that the file does not name yet, at the end.
  local extra = {}
  for name in pairs(live) do if not seen[name] then extra[#extra + 1] = name end end
  table.sort(extra)
  for _, name in ipairs(extra) do order[#order + 1] = name end
  local held = m.theme.appearance ~= "system" and m.theme.appearance or "the OS's appearance"
  local rows = { width = "grow", gap = 2 }
  for _, name in ipairs(order) do rows[#rows + 1] = colour_row(name, live[name], notes[name], live) end
  return lead("Every colour role of the skin in force — " .. m.theme.skin .. ", " .. held ..
      " — read live from Kit: change it in the Theme panel and this page follows."),
    muted("Each text role shows its contrast on the grounds its note names; a wash or a translucent selection is laid over the panel first."),
    kit.panel({ title = "Colour roles", subtitle = #order .. " roles" }, { column(rows) })
end

-- The type styles, the scales and the shadows, as the skin sets them now.
function M.type(m, env)
  local lengths = env.tokens and env.tokens.lengths or {}
  local families = env.tokens and env.tokens.families or {}
  local function px(name) return lengths[name] and string.format("%g px", lengths[name]) or "—" end
  local function style(label, size, family, bold, said)
    return row { width = "grow", gap = 16, cross_align = "center", pad = { y = 4 },
      column { width = 180, gap = 2,
        text(label, { size = 11, family = "$kitData", color = "$kitMuted" }),
        text(said, { size = 11, family = "$kitData", color = "$kitFaint" }) },
      column { width = "grow", min_width = 0,
        text("Restart checkout-api — 412 ms, 3 errors", { size = size, family = family, bold = bold, color = "$kitFg", wrap = "word" }) } }
  end
  local ui, data = families.kitUi or "sans", families.kitData or "mono"
  local types = kit.panel({ title = "Type", subtitle = "the faces this machine resolved: " .. ui .. " and " .. data }, {
    style("$kitSizeTitle, bold", "$kitSizeTitle", "$kitUi", true, px("kitSizeTitle") .. " · $kitUi"),
    style("$kitSize", "$kitSize", "$kitUi", false, px("kitSize") .. " · $kitUi"),
    style("$kitSizeLabel", "$kitSizeLabel", "$kitUi", false, px("kitSizeLabel") .. " · $kitUi"),
    style("$kitSize, data", "$kitSize", "$kitData", false, px("kitSize") .. " · $kitData"),
    style("$kitSizeLabel, data", "$kitSizeLabel", "$kitData", false, px("kitSizeLabel") .. " · $kitData"),
  })
  -- Each length drawn to size, beside its name and value.
  local function bar(name, w, h)
    return row { width = "grow", gap = 16, cross_align = "center",
      column { width = 180, text(name, { size = 11, family = "$kitData", color = "$kitMuted" }) },
      column { width = w, height = h, bg = "$kitAccent" },
      text(string.format("%g px", type(w) == "number" and w or (type(h) == "number" and h or 0)),
        { size = 11, family = "$kitData", color = "$kitFaint" }) }
  end
  local sizes = { width = "grow", gap = 8 }
  for _, name in ipairs({ "kitRow", "kitCtl" }) do
    if lengths[name] then sizes[#sizes + 1] = bar("$" .. name .. " (a height)", 8, lengths[name]) end
  end
  for _, w in ipairs({ 2, 4, 6, 8, 10, 12, 16 }) do sizes[#sizes + 1] = bar("gap / pad " .. w, w, 8) end
  for _, name in ipairs({ "kitHair", "kitRuleW" }) do
    if lengths[name] then sizes[#sizes + 1] = bar("$" .. name .. " (a rule)", 120, lengths[name]) end
  end
  local radius = lengths.kitRadius or 0
  local corners = row { gap = 16, cross_align = "center",
    column { width = 64, height = 40, bg = "$kitSunken", radius = "$kitRadius", border = { w = 1, color = "$kitRule" } },
    text("$kitRadius: " .. string.format("%g px", radius) .. (radius == 0 and " (square)" or ""),
      { size = 11, family = "$kitData", color = "$kitMuted" }) }
  local shadow = row { gap = 24, cross_align = "center", pad = { y = 8, x = 4 },
    column { width = 120, height = 56, bg = "$kitRaised", radius = "$kitRadius", border = { w = 1, color = "$kitInk" },
      shadow_color = "$kitInk", shadow_x = "$kitShadow", shadow_y = "$kitShadow", shadow_blur = "$kitShadowBlur",
      main_align = "center", cross_align = "center",
      text("A float", { size = "$kitSizeLabel", family = "$kitUi", color = "$kitFg" }) },
    kit.button({ label = "Primary", variant = "primary" }),
    text("$kitShadow " .. px("kitShadow") .. " · $kitShadowBlur " .. px("kitShadowBlur"),
      { size = 11, family = "$kitData", color = "$kitMuted" }) }
  return lead("The type styles in the faces Kit resolved, the spacing and rule scales drawn to size, the radius and the shadow — as the Theme panel sets them now."),
    types,
    kit.panel({ title = "Spacing and rules" }, { column(sizes) }),
    kit.panel({ title = "Radius and shadow" }, { column { gap = 12, corners, shadow } })
end

-- Kit's icons, as the brand book lists them (design/README.md's "The
-- set:" line), each drawn by Kit and named.
function M.icon_names()
  local s = read("design/README.md") or ""
  local line = s:match("\n%- The set: ([^\n]*)") or ""
  local out = {}
  for name in line:gmatch("`([%w_]+)`") do out[#out + 1] = name end
  return out
end

function M.icons(m)
  local names = M.icon_names()
  local cells = { width = "grow", gap = 8, cross_gap = 8, min_column = 120 }
  for _, name in ipairs(names) do
    cells[#cells + 1] = column { gap = 6, pad = 8, cross_align = "center", radius = "$kitRadius", bg = "$kitPanel",
      border = { w = 1, color = "$kitRule" },
      kit.icon_button({ icon = name, label = name, variant = "ghost" }),
      text(name, { size = 11, family = "$kitData", color = "$kitMuted" }) }
  end
  return lead(#names .. " icons, drawn by Kit in a 24-unit frame as 2-unit strokes, each in the colour of the text beside it."),
    row(cells)
end

function M.foundations(m)
  local s = read("design/README.md")
  if not s then return muted("design/README.md could not be read from the app's folder.") end
  return kit.panel({ title = "Kit's brand book", subtitle = "design/README.md" }, { kit.markdown({ text = s }) })
end

-- A component's guideline (design/components/<Component>.md), nil when
-- it has none.
function M.guideline(component)
  return read("design/components/" .. component .. ".md")
end

-- `line_chart` ↔ `LineChart`.
function M.component_of(name)
  return (name:gsub("^%l", string.upper):gsub("_(%l)", string.upper))
end

-- The guidelines a card shows: one for each Kit component its call places,
-- in the order it places them.
function M.guides_of(call)
  local out, seen = {}, {}
  for name in (call or ""):gmatch("kit%.([%w_]+)%(") do
    local comp = M.component_of(name)
    if not seen[comp] and M.guideline(comp) then
      seen[comp] = true
      out[#out + 1] = comp
    end
  end
  return out
end

return M
