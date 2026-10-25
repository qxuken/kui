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
  "ok", "warn", "danger", "scrim",
}

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
-- `"$kitSelFg"` otherwise.
function M.ref(s: Settings, role: string): string
  local o = s.roles and s.roles[role]
  if o is string then return o end
  return "$kit" .. camel(role)
end

local function half_of(v: any, l: integer, d: integer): integer, integer
  if v is integer then return v, v end
  if v is {string:integer} then return v.light or l, v.dark or d end
  return l, d
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
