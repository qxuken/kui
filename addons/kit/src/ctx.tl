-- What every component is handed besides its props: the frame's env, the
-- settings in force and the instance it draws, with the helpers every
-- component paints through, so a role, a label's casing and a text style
-- are spelled once.

local skin = require("skin")

local record Ctx
  env: kui.Env
  s: skin.Settings
  -- The instance's key: what its own events carry back to find its state.
  inst: integer
end

local M = { Ctx = Ctx }

-- The colour a role paints in: `c:ref("panel")`.
function M.ref(c: Ctx, role: string): string
  return skin.ref(c.s, role)
end

-- A label as the skin draws it.
function M.label(c: Ctx, text: string): string
  return skin.label(c.s, text)
end

-- An action of this instance, as a node's message: the event comes back
-- to `on_event` with `kit` naming the component and its action and `inst`
-- the instance, plus whatever `fields` say.
function M.act(c: Ctx, kit: string, fields?: {string:any}): {string:any}
  local m: {string:any} = { kit = kit, inst = c.inst }
  if fields then
    for k, v in pairs(fields) do m[k] = v end
  end
  return m
end

-- Body text in the UI face.
function M.text(c: Ctx, s: string, o?: {string:any}): kui.Node
  local t: {string:any} = { size = "$kitSize", family = "$kitUi", color = M.ref(c, "fg") }
  if o then for k, v in pairs(o) do t[k] = v end end
  return text(s, t as kui.text)
end

-- Text in the data face: ids, timings, numbers.
function M.data(c: Ctx, s: string, o?: {string:any}): kui.Node
  local t: {string:any} = { size = "$kitSize", family = "$kitData", color = M.ref(c, "fg") }
  if o then for k, v in pairs(o) do t[k] = v end end
  return text(s, t as kui.text)
end

-- A label on a control or a header, cased as the skin says.
function M.caption(c: Ctx, s: string, o?: {string:any}): kui.Node
  local t: {string:any} = {
    size = "$kitSizeLabel", family = "$kitUi", bold = c.s.skin.label_bold, color = M.ref(c, "fg"),
    max_lines = 1, ellipsis = true,
  }
  if o then for k, v in pairs(o) do t[k] = v end end
  return text(M.label(c, s), t as kui.text)
end

-- A muted line: a subtitle, a hint, a count.
function M.muted(c: Ctx, s: string, o?: {string:any}): kui.Node
  local t: {string:any} = { size = "$kitSizeLabel", family = "$kitUi", color = M.ref(c, "muted") }
  if o then for k, v in pairs(o) do t[k] = v end end
  return text(s, t as kui.text)
end

-- A rule across its parent: `w` px of `role` (the region rule by default).
function M.rule(c: Ctx, role?: string, w?: kui.Length, vertical?: boolean): kui.Node
  local t: kui.Props = { bg = M.ref(c, role or "ink") }
  if vertical then
    t.width, t.height = w or "$kitRuleW", "grow"
  else
    t.width, t.height = "grow", w or "$kitRuleW"
  end
  return column(t)
end

-- A spacer that takes the room its row leaves.
function M.grow(): kui.Node
  return row { width = "grow" }
end

-- The shadow a float or the primary action carries, onto `t`.
-- Ledger's is hard and offset in the ink, Neutral's soft and dropped,
-- Phosphor's a glow in the ink with no offset (on `strong` things only:
-- a primary button, the focused float).
function M.shadow(c: Ctx, t: kui.Props, strong?: boolean)
  local sk = c.s.skin
  if sk.shadow == 0 then
    if strong then
      t.shadow_color, t.shadow_blur = M.ref(c, "ink"), "$kitShadowBlur"
    end
  elseif sk.shadow_blur == 0 then
    t.shadow_color = M.ref(c, "ink")
    t.shadow_x, t.shadow_y = "$kitShadow", "$kitShadow"
  else
    t.shadow_color = 0x0000002e
    t.shadow_x, t.shadow_y, t.shadow_blur = 0, "$kitShadow", "$kitShadowBlur"
  end
end

-- Each instance's state between frames, for what the host does not keep:
-- a menu open, a waterfall's own window. Made by `init` on first use.
local states: {integer:any} = {}

function M.state<T>(c: Ctx | integer, init: function(): T): T
  local k: integer = c is integer and c or (c as Ctx).inst
  local st = states[k]
  if st == nil then
    st = init()
    states[k] = st
  end
  return st as T
end

-- A state if the instance has one, without making it.
function M.peek<T>(inst: integer): T
  return states[inst] as T
end

-- A host's `true`/`false` with a default for nil.
function M.flag(v: any, default: boolean): boolean
  if v == nil then return default end
  return v == true
end

return M
