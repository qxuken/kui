-- A skin is data: the colour roles for each appearance and the shape
-- settings. `tokens` lowers a skin plus the app's settings to the token
-- table Kit declares with `env.set_tokens`, so every component paints from
-- `"$kit…"` names and light/dark is the core's to resolve (ADR 0027).

local record Half
  bg: integer
  side: integer
  panel: integer
  hover: integer
  rule: integer
  ink: integer
  fg: integer
  muted: integer
  sel: integer
  sel_fg: integer
  accent: integer
  on_accent: integer
  ok: integer
  warn: integer
  -- The waterfall's group colours, in order.
  groups: {integer}
end

local record Skin
  name: string
  light: Half
  dark: Half
  -- Corner radius, region rule width and hard shadow offset, in px.
  radius: number
  rule_w: number
  shadow: number
  -- Labels drawn in capitals (written into the string: kui has no text-transform).
  label_upper: boolean
  font_ui: {string}
  font_data: {string}
end

local enum Density
  "comfortable" "compact"
end

local record Settings
  shadows: boolean
  density: Density
end

local record Decl
  colors: {string:{string:integer}}
  lengths: {string:number}
  families: {string:{string}}
end

local M = { Half = Half, Skin = Skin, Settings = Settings }

local ROLES: {string} = {
  "bg", "side", "panel", "hover", "rule", "ink", "fg", "muted",
  "sel", "sel_fg", "accent", "on_accent", "ok", "warn",
}

-- `sel_fg` → `SelFg`, the tail of a `kit…` token name.
local function camel(role: string): string
  return (role:gsub("^%l", string.upper):gsub("_(%l)", string.upper))
end

-- The token name a role paints under: `"$kitSelFg"`.
function M.ref(role: string): string
  return "$kit" .. camel(role)
end

-- Rows and controls by density: the only lengths that move with it.
function M.row_h(s: Settings): number
  return s.density == "compact" and 20 or 22
end

function M.ctl_h(s: Settings): number
  return s.density == "compact" and 22 or 24
end

function M.tokens(skin: Skin, s: Settings): Decl
  local colors: {string:{string:integer}} = {}
  local l = skin.light as {string:any}
  local d = skin.dark as {string:any}
  for _, role in ipairs(ROLES) do
    colors["kit" .. camel(role)] = { light = l[role] as integer, dark = d[role] as integer }
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

-- A label as the skin draws it.
function M.label(skin: Skin, text: string): string
  return skin.label_upper and text:upper() or text
end

return M
