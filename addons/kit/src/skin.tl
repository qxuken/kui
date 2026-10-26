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

-- TEXT_CONTRAST: what body text and a tone's text read at on what they
-- sit on (WCAG AA); GRAPHIC_CONTRAST: a chart's line, which is no text.
local M = { Half = Half, Skin = Skin, Settings = Settings, ROLES = ROLES, AREA_ALPHA = 0.14,
  TEXT_CONTRAST = 4.5, GRAPHIC_CONTRAST = 3.0 }

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

-- A fill's hover and press shades in one half, from the colours in force.
local function shades(colors: {string:{string:integer}}, half: string, fill: string, value: integer): integer, integer
  local toward = TOWARD[fill]
  if toward then
    local t = colors["kit" .. (toward == "on_accent" and "OnAccent" or "OnDanger")][half]
    return mix(value, t, 0.16), mix(value, t, 0.30)
  end
  return over(value, colors.kitHover[half]), over(value, colors.kitPress[half])
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

-- 0xRRGGBBAA as hue (degrees), saturation and lightness (0 to 1), and back.
local function hsl(c: integer): number, number, number
  local r, g, b = channels(c)
  local R, G, B = r / 255, g / 255, b / 255
  local hi, lo = math.max(R, G, B), math.min(R, G, B)
  local l = (hi + lo) / 2
  if hi == lo then return 0, 0, l end
  local d = hi - lo
  local s = l > 0.5 and d / (2 - hi - lo) or d / (hi + lo)
  local h: number
  if hi == R then h = (G - B) / d + (G < B and 6 or 0)
  elseif hi == G then h = (B - R) / d + 2
  else h = (R - G) / d + 4 end
  return h * 60, s, l
end

local function from_hsl(h: number, s: number, l: number, alpha: integer): integer
  local function f(n: number): integer
    local k = (n + h / 30) % 12
    local x = l - s * math.min(l, 1 - l) * math.max(-1, math.min(k - 3, 9 - k, 1))
    return math.floor(math.max(0, math.min(1, x)) * 255 + 0.5)
  end
  return (f(0) << 24) | (f(8) << 16) | (f(4) << 8) | alpha
end

-- How colourful: the spread of the channels, 0 to 1.
local function chroma(c: integer): number
  local r, g, b = channels(c)
  return (math.max(r, g, b) - math.min(r, g, b)) / 255
end

local function hue_gap(a: number, b: number): number
  local d = math.abs(a - b) % 360
  return d > 180 and 360 - d or d
end

local function rgb(c: integer): integer return c & ~0xff end

-- `c` moved in lightness away from `ground` until it reads at `target`
-- on it, its hue and saturation kept, so a tone stays recognisable.
local function legible(c: integer, ground: integer, target: number): integer
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

-- `c` legible on every one of `grounds`.
local function legible_on(c: integer, grounds: {integer}, target: number): integer
  for _ = 1, 3 do
    for _, g in ipairs(grounds) do c = legible(c, g, target) end
  end
  return c
end

-- Of `options`, the one that reads best on `fill`.
local function best_on(fill: integer, options: {integer}): integer
  local pick, most = options[1], -1.0
  for _, o in ipairs(options) do
    local k = contrast(fill, o)
    if k > most then pick, most = o, k end
  end
  return pick
end

-- The roles a skin draws in or near its brand colour, which a brand's
-- accent re-hues, and the surfaces it tints: Ledger's selection is its
-- yellow; Phosphor's text, rules and ground are all its green.
local REHUE: {string} = {
  "bg", "side", "panel", "raised", "sunken", "rule", "ink", "fg", "muted", "faint",
  "sel", "sel_fg", "focus", "hover", "press",
}
-- How far each surface leans toward the accent: a tint, not a wash.
local TINT: {string:number} = { bg = 0.05, side = 0.06, panel = 0.03, raised = 0.03, sunken = 0.07, rule = 0.12 }

-- An accent given as a colour derives the palette around it in one half:
-- what the skin drew in its own accent takes the new one, what it drew in
-- the accent's hue takes the new hue at its own lightness, the surfaces
-- lean a few percent toward it, the washes take its hue; what reads on it
-- is worked out, and every text role is moved, in lightness alone, until
-- it reads at 4.5:1 on the surfaces it is drawn on. Roles the brand names
-- are left as named. A yellow accent left Ledger's info notice yellow on
-- paper, and Phosphor's text, knobs and rules green.
local function derive(s: Settings, colors: {string:{string:integer}}, half: string, h: {string:any})
  local o = s.roles and s.roles.accent
  if o == nil or o is string then return end
  local named = s.roles
  local function set(role: string, v: integer)
    if not named[role] then colors["kit" .. camel(role)][half] = v end
  end
  local A = rgb(colors.kitAccent[half]) | 0xff
  local A0 = h.accent as integer
  local ah, as = hsl(A)
  local h0, s0 = hsl(A0)
  if rgb(A) ~= rgb(A0) then
    for _, role in ipairs(REHUE) do
      local c = h[role] as integer
      local alpha = c & 0xff
      local v: integer = nil
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
  local function get(role: string): integer return colors["kit" .. camel(role)][half] end
  local panel, bg, side, sunken, raised = get("panel"), get("bg"), get("side"), get("sunken"), get("raised")
  local grounds: {integer} = { panel, bg, side, sunken, raised }
  local black, white = 0x111111ff, 0xffffffff
  -- What reads on the accent, and on a selection that is the accent.
  set("on_accent", best_on(A, { h.fg as integer, h.bg as integer, black, white }))
  local sel = get("sel")
  local sel_ground = (sel & 0xff) == 0xff and sel or over(panel, sel)
  if (h.sel_fg as integer) == (h.on_accent as integer) and (sel & 0xff) == 0xff then
    set("sel_fg", best_on(sel_ground, { get("on_accent"), h.fg as integer, h.bg as integer, black, white }))
  else
    set("sel_fg", legible(get("sel_fg"), sel_ground, M.TEXT_CONTRAST))
  end
  -- The first chart series and the first waterfall group in the accent:
  -- the line at 3:1 on the panel, the group a pale fill the text reads on.
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
  -- The chosen segment of a view switch: the accent, and what reads on it.
  colors.kitChosen[half] = A
  colors.kitChosenFg[half] = get("on_accent")
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
    local ls, ds = skin.light.series[i], skin.dark.series[i]
    colors["kitSeries" .. i] = { light = ls, dark = ds }
    -- The area under a series' line: its colour at AREA_ALPHA, so a chart
    -- fills it with plain polygons and no opacity of their own.
    colors["kitArea" .. i] = { light = M.faded(ls, AREA_ALPHA), dark = M.faded(ds, AREA_ALPHA) }
  end
  -- A view switch's chosen segment: printed in the ink where controls are
  -- outlined, a raised chip in Neutral — until an accent is given.
  local outlined = skin.ctl_border
  colors.kitChosen = { light = colors[outlined and "kitInk" or "kitPanel"].light, dark = colors[outlined and "kitInk" or "kitPanel"].dark }
  colors.kitChosenFg = { light = colors[outlined and "kitBg" or "kitFg"].light, dark = colors[outlined and "kitBg" or "kitFg"].dark }
  derive(s, colors, "light", l)
  derive(s, colors, "dark", d)
  -- Text, and the tones and the focus colour where they are text — a
  -- notice on the well, a badge's label, a toast's mark — read at 4.5:1
  -- on every surface, accent or none: moved in lightness alone, so a tone
  -- stays itself. Ledger's blue info text sat at 4:1 on its well. Roles
  -- the brand names are kept as named.
  for _, half in ipairs({ "light", "dark" }) do
    local function at(name: string): integer return colors[name][half] end
    local grounds: {integer} = { at("kitPanel"), at("kitBg"), at("kitSide"), at("kitSunken"), at("kitRaised") }
    for _, role in ipairs({ "fg", "muted", "faint", "focus", "ok", "warn", "danger" }) do
      if not (s.roles and s.roles[role]) then
        local slot = colors["kit" .. camel(role)]
        slot[half] = legible_on(slot[half], grounds, M.TEXT_CONTRAST)
      end
    end
  end
  -- The accent as text (an accent notice, its toast's mark): the accent
  -- moved in lightness until it reads on the panel and the well. Ledger's
  -- yellow was its own notice's text on paper.
  local function accent_text(half: string): integer
    local function at(name: string): integer return colors[name][half] end
    return legible_on(rgb(at("kitAccent")) | 0xff, { at("kitPanel"), at("kitBg"), at("kitSide"), at("kitSunken"), at("kitRaised") }, M.TEXT_CONTRAST)
  end
  colors.kitAccentText = { light = accent_text("light"), dark = accent_text("dark") }
  local text_role = s.roles and s.roles.accent_text
  if text_role is integer or text_role is {string:integer} then
    local lv, dv = half_of(text_role, colors.kitAccentText.light, colors.kitAccentText.dark)
    colors.kitAccentText = { light = lv, dark = dv }
  end
  for _, fill in ipairs(SHADED) do
    -- From the colours in force, the brand's integers and what an accent
    -- derived included.
    local base = colors["kit" .. camel(fill)]
    local lh, lp = shades(colors, "light", fill, base.light)
    local dh, dp = shades(colors, "dark", fill, base.dark)
    colors["kit" .. camel(fill) .. "Hover"] = { light = lh, dark = dh }
    colors["kit" .. camel(fill) .. "Press"] = { light = lp, dark = dp }
  end
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
