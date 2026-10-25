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

local M = { Half = Half, Skin = Skin, Settings = Settings, ROLES = ROLES }

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
function M.ref(s: Settings, role: string): string
  local o = s.roles and s.roles[role]
  if o is string then return o end
  local fill = role:match("^(.-)_hover$") or role:match("^(.-)_press$")
  local named = fill and s.roles and s.roles[fill]
  if named is string then return named end
  return "$kit" .. camel(role)
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
    colors["kitSeries" .. i] = { light = skin.light.series[i], dark = skin.dark.series[i] }
  end
  local shadow = s.shadows and skin.shadow or 0
  return {
    colors = colors,
    lengths = {
      kitRow = M.row_h(s),
      kitCtl = M.ctl_h(s),
      kitRadius = s.radius or skin.radius,
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

-- What the token table depends on: declared again only when it moves.
function M.key(s: Settings): string
  local parts = { s.skin.name, s.density, tostring(s.shadows), tostring(s.radius) }
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
