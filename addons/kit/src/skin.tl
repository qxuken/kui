-- A skin is data: the colour roles for each appearance and the shape it
-- draws with. `tokens` lowers a skin plus the app's settings to the token
-- table Kit declares with `env.set_tokens`, so every component paints from
-- `"$kit…"` names and light/dark is the core's to resolve (ADR 0027). A
-- role the brand overrides with a token name (`"$brandSurface"`) is painted
-- with that name instead, so it keeps the host's own light and dark.

local record Half
  -- The window, a sidebar, a panel on it, and a float above both.
  bg: integer
  side: integer
  panel: integer
  raised: integer
  -- A well cut into a surface: a field, a track, a code block.
  sunken: integer
  -- Washes over a hovered and a pressed row or control.
  hover: integer
  press: integer
  -- A hairline between rows, and the rule that marks a region.
  rule: integer
  ink: integer
  -- Text: body, secondary, and the quietest — counts, times, shortcuts —
  -- which still reads at 4.5:1 on `panel` (WCAG AA for small text).
  fg: integer
  muted: integer
  faint: integer
  -- A selected row and its text.
  sel: integer
  sel_fg: integer
  -- The one loud colour and what reads on it.
  accent: integer
  on_accent: integer
  -- What reads on `danger` as a fill (a destructive button's label): the
  -- page's colour where danger is dark, the ink where it is light.
  on_danger: integer
  -- The keyboard focus ring.
  focus: integer
  ok: integer
  warn: integer
  danger: integer
  -- What a dialog dims the window with.
  scrim: integer
  -- The waterfall's group fills, pale enough for `fg` on each.
  groups: {integer}
  -- A chart's series, in order: strokes that read on `panel`.
  series: {integer}
end

local record Skin
  name: string
  light: Half
  dark: Half
  -- Corner radius, region rule and hairline widths, in px.
  radius: number
  rule_w: number
  hair: number
  -- The shadow under the primary action and a float: an offset and a
  -- blur (0 for Ledger's hard one).
  shadow: number
  shadow_blur: number
  -- Labels drawn in capitals (written into the string: kui has no text-transform).
  label_upper: boolean
  -- Labels on controls in bold; Neutral's are regular, its primary action aside.
  label_bold: boolean
  -- Controls carry a border; Neutral's sit on a sunken fill instead.
  ctl_border: boolean
  -- A text field: an underline (Ledger, Phosphor) or a box (Neutral).
  field_box: boolean
  font_ui: {string}
  font_data: {string}
  -- Text sizes: body, the labels on controls and headers, a title.
  size: number
  size_label: number
  size_title: number
end

local enum Density
  "comfortable" "compact"
end

local record Settings
  skin: Skin
  density: Density
  shadows: boolean
  -- The brand's corner radius in px; the skin's when nil.
  radius: number
  -- Roles the brand overrides: a colour, `{ light, dark }`, or a token name.
  roles: {string:any}
  -- "light" or "dark" holds Kit's colours to that half whatever the OS
  -- says; nil follows it (the core resolves each token's halves).
  appearance: string
  -- The hairline and region rule widths in px; the skin's when nil.
  hair: number
  rule_w: number
  -- The faces, as family lists; the skin's when nil.
  font_ui: {string}
  font_data: {string}
end

local record Decl
  colors: {string:{string:integer}}
  lengths: {string:number}
  families: {string:{string}}
  -- Readable from every origin, so an app paints its own nodes with
  -- `$kitPanel` and follows the skin.
  export: boolean
end

local ROLES: {string} = {
  "bg", "side", "panel", "raised", "sunken", "hover", "press", "rule", "ink",
  "fg", "muted", "faint", "sel", "sel_fg", "accent", "on_accent", "focus",
  "ok", "warn", "danger", "on_danger", "scrim",
}

-- The fills a pointer changes: each gets an opaque hover and press shade
-- (`accent_hover`, `panel_press`), worked out from the skin's colours. A
-- translucent wash over a fill let what is under it show through: under
-- the primary button that was Ledger's hard shadow, so a press looked as
-- if the shadow had jumped into the button.
local SHADED: {string} = { "accent", "danger", "panel", "sunken", "raised" }

-- The colour a fill moves toward on hover and press: what reads on it,
-- for the loud fills; the washes, laid over it, for the quiet ones.
local TOWARD: {string:string} = { accent = "on_accent", danger = "on_danger" }

local M = { Half = Half, Skin = Skin, Settings = Settings, ROLES = ROLES, AREA_ALPHA = 0.14 }

-- `sel_fg` → `SelFg`, the tail of a `kit…` token name.
local function camel(role: string): string
  return (role:gsub("^%l", string.upper):gsub("_(%l)", string.upper))
end

-- Rows and controls by density: the only lengths that move with it.
function M.row_h(s: Settings): number
  return s.density == "compact" and 20 or 22
end

function M.ctl_h(s: Settings): number
  return s.density == "compact" and 22 or 26
end

-- The name a role paints under: the brand's token where it named one,
-- `"$kitSelFg"` otherwise. A shade of a fill the brand named a token for
-- is that token: Kit cannot shade a colour it does not know.
-- Each settings' answers, kept: a chart asks for a role per tick and per
-- label, and the answer is two pattern matches and two substitutions.
-- Settings are replaced whole when they change, never edited, so a table
-- of them is the key; weak, so old settings go.
local refs: {Settings:{string:string}} = setmetatable({}, { __mode = "k" })

function M.ref(s: Settings, role: string): string
  local memo = refs[s]
  if not memo then
    memo = {}
    refs[s] = memo
  end
  local r = memo[role]
  if r then return r end
  local o = s.roles and s.roles[role]
  if o is string then
    r = o
  else
    local fill = role:match("^(.-)_hover$") or role:match("^(.-)_press$")
    local named = fill and s.roles and s.roles[fill]
    r = named is string and named or ("$kit" .. camel(role))
  end
  memo[role] = r
  return r
end

local function half_of(v: any, l: integer, d: integer): integer, integer
  if v is integer then return v, v end
  if v is {string:integer} then return v.light or l, v.dark or d end
  return l, d
end

-- 0xRRGGBBAA's channels, and back.
local function channels(c: integer): integer, integer, integer, integer
  return (c >> 24) & 255, (c >> 16) & 255, (c >> 8) & 255, c & 255
end

-- `a` moved `t` of the way to `b`, opaque.
local function mix(a: integer, b: integer, t: number): integer
  local ar, ag, ab = channels(a)
  local br, bg, bb = channels(b)
  local function m(x: integer, y: integer): integer return math.floor(x + (y - x) * t + 0.5) end
  return (m(ar, br) << 24) | (m(ag, bg) << 16) | (m(ab, bb) << 8) | 0xff
end

-- A wash (with its own alpha) laid over an opaque fill.
local function over(fill: integer, wash: integer): integer
  local _, _, _, a = channels(wash)
  return mix(fill, wash | 0xff, a / 255)
end

-- A fill's hover and press shades in one half of a skin.
local function shades(h: {string:any}, fill: string, value: integer): integer, integer
  local toward = TOWARD[fill]
  if toward then
    local t = h[toward] as integer
    return mix(value, t, 0.16), mix(value, t, 0.30)
  end
  return over(value, h.hover as integer), over(value, h.press as integer)
end

-- How opaque the area under a chart's line is.
local AREA_ALPHA = M.AREA_ALPHA

-- 0xRRGGBBAA with its alpha scaled by `a`.
function M.faded(color: integer, a: number): integer
  local alpha = color & 0xff
  return (color & ~0xff) | math.floor(alpha * a + 0.5)
end

-- A channel's share of the light, 0 to 1 (WCAG's relative luminance).
local function luminance(c: integer): number
  local r, g, b = channels(c)
  local function lin(x: integer): number
    local v = x / 255
    return v <= 0.04045 and v / 12.92 or ((v + 0.055) / 1.055) ^ 2.4
  end
  return 0.2126 * lin(r) + 0.7152 * lin(g) + 0.0722 * lin(b)
end

local function contrast(a: integer, b: integer): number
  local x, y = luminance(a), luminance(b)
  if x < y then x, y = y, x end
  return (x + 0.05) / (y + 0.05)
end

-- Of `a` and `b`, the one that reads better on `fill`.
local function readable(fill: integer, a: integer, b: integer): integer
  return contrast(fill, a) >= contrast(fill, b) and a or b
end

-- The roles a skin paints in its accent — Ledger's selection is its
-- yellow, Neutral's focus ring its blue, Phosphor's first series its
-- green — follow a brand's accent there, as do what reads on it, unless
-- the brand names them too. A blue accent left Ledger's selection yellow
-- and its primary label black on blue.
local FOLLOW: {string} = { "sel", "focus" }

local function follow(s: Settings, colors: {string:{string:integer}}, half: string, h: {string:any})
  local o = s.roles and s.roles.accent
  if o == nil or o is string then return end
  local was = h.accent as integer
  local now = colors.kitAccent[half]
  if now == was then return end
  local function same(c: integer): boolean return (c & ~0xff) == (was & ~0xff) end
  if not s.roles.on_accent then
    colors.kitOnAccent[half] = readable(now, h.fg as integer, h.bg as integer)
  end
  for _, role in ipairs(FOLLOW) do
    local own = h[role] as integer
    if not s.roles[role] and same(own) then
      colors["kit" .. camel(role)][half] = (now & ~0xff) | (own & 0xff)
      if role == "sel" and not s.roles.sel_fg and (h.sel_fg as integer) == (h.on_accent as integer) then
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

function M.tokens(s: Settings): Decl
  local skin = s.skin
  local colors: {string:{string:integer}} = {}
  local l = skin.light as {string:any}
  local d = skin.dark as {string:any}
  for _, role in ipairs(ROLES) do
    local lv, dv = half_of(s.roles and s.roles[role], l[role] as integer, d[role] as integer)
    colors["kit" .. camel(role)] = { light = lv, dark = dv }
  end
  for _, fill in ipairs(SHADED) do
    -- From the colours in force, the brand's integers included.
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
    -- The area under a series' line: its colour at AREA_ALPHA, so a chart
    -- fills it with plain polygons and no opacity of their own.
    colors["kitArea" .. i] = { light = M.faded(ls, AREA_ALPHA), dark = M.faded(ds, AREA_ALPHA) }
  end
  follow(s, colors, "light", l)
  follow(s, colors, "dark", d)
  -- The shimmer a skeleton sweeps with: the well lit toward white in the
  -- light half and toward the text in the dark one, and the same with no
  -- alpha, for the band's soft edges.
  local sunken = colors.kitSunken
  local lit = { light = mix(sunken.light, 0xffffffff, 0.7), dark = mix(sunken.dark, colors.kitFg.dark, 0.12) }
  colors.kitShimmer = lit
  colors.kitShimmerClear = { light = lit.light & ~0xff, dark = lit.dark & ~0xff }
  -- An appearance held: both halves of every colour are the one asked for.
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

-- The colour `role` resolves to under these settings in `half` ("light"
-- or "dark"), from the token table they lower to: what a fragment's
-- params carry, since a shader reads numbers, not tokens.
local resolved: {Settings:Decl} = setmetatable({}, { __mode = "k" })
function M.color(s: Settings, name: string, half: string): integer
  local decl = resolved[s]
  if not decl then
    decl = M.tokens(s)
    resolved[s] = decl
  end
  local c = decl.colors[name]
  return c and c[half] or 0
end

-- What the token table depends on: declared again only when it moves.
function M.key(s: Settings): string
  local parts = { s.skin.name, s.density, tostring(s.shadows), tostring(s.radius),
    tostring(s.appearance), tostring(s.hair), tostring(s.rule_w),
    s.font_ui and table.concat(s.font_ui, ",") or "", s.font_data and table.concat(s.font_data, ",") or "" }
  if s.roles then
    local names: {string} = {}
    for k in pairs(s.roles) do names[#names + 1] = k end
    table.sort(names)
    for _, k in ipairs(names) do
      local v = s.roles[k]
      if v is {string:integer} then
        parts[#parts + 1] = k .. "=" .. tostring(v.light) .. "/" .. tostring(v.dark)
      else
        parts[#parts + 1] = k .. "=" .. tostring(v)
      end
    end
  end
  return table.concat(parts, ";")
end

-- A label as the skin draws it.
function M.label(s: Settings, text: string): string
  return s.skin.label_upper and text:upper() or text
end

return M
