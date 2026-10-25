-- Kit's entry: the script a host loads. It provides components (ADR 0060)
-- and nothing else: a host places `kit/<name>` as often as it likes, each
-- instance keyed by where it is and replayed while its props hold, and
-- hears Kit's events as its own messages. `schema.lua` beside `src/` is
-- the API these functions answer to; every binding's typed wrappers are
-- generated from it (ADR 0066).
--
-- `settings` draws nothing: it holds the window's skin, density, shadows
-- and brand roles, and declares Kit's tokens whenever they move — which
-- re-asks every instance that painted with them.
--
-- The modules beside this file are found through `package.path`, which
-- every host's loader starts with this file's folder (ADR 0059).

local skin = require("skin")
local ctx = require("ctx")
local ledger = require("ledger")
local phosphor = require("phosphor")
local neutral = require("neutral")
local layout = require("layout")
local inputs = require("inputs")
local data = require("data")
local overlays = require("overlays")
local charts = require("charts")
local waterfall = require("waterfall")

local type P = {string:any}
local type View = function(ctx.Ctx, P): kui.Node

local SKINS: {string:skin.Skin} = { ledger = ledger, phosphor = phosphor, neutral = neutral }

local settings: skin.Settings = { skin = ledger, density = "comfortable", shadows = true }
local declared: string = nil

local function ensure_tokens(env: kui.Env)
  local k = skin.key(settings)
  if declared ~= k then
    env.set_tokens(skin.tokens(settings))
    declared = k
  end
end

local function apply(p: P)
  local roles = p.roles
  local radius = p.radius
  settings = {
    skin = SKINS[p.skin as string] or ledger,
    density = p.density == "compact" and "compact" or "comfortable",
    shadows = p.shadows ~= false,
    radius = radius is number and radius or nil,
    roles = roles is {string:any} and roles or nil,
  }
end

-- Each component's view, by the module that draws it.
local VIEWS: {string:View} = {}
-- Each component's module's event handler, by component name.
local EVENTS: {string:function(P, string): any} = {}

local function provide(module: {string:any}, names: {string})
  for _, name in ipairs(names) do
    VIEWS[name] = module[name] as View
    EVENTS[name] = module.event as function(P, string): any
  end
end

provide(layout as {string:any}, { "panel", "toolbar", "statusbar", "split", "tabs", "section", "divider" })
provide(inputs as {string:any}, {
  "button", "icon_button", "text_field", "search", "select", "checkbox", "switch",
  "radio_group", "segmented", "slider", "text_area",
})
provide(data as {string:any}, { "table", "list", "inspector", "log", "badge", "stat", "code", "empty", "progress", "notice" })
provide(overlays as {string:any}, { "dialog", "popover", "menu", "toast" })
provide(charts as {string:any}, { "sparkline", "line_chart", "bar_chart", "histogram" })
provide(waterfall as {string:any}, { "waterfall" })

local type Component = function(kui.Env, P, kui.Instance): kui.Node

global components: {string:Component} = {
  settings = function(env: kui.Env, props: P, _inst: kui.Instance): kui.Node
    apply(props or {})
    ensure_tokens(env)
    return column { width = 0, height = 0 }
  end,
}

for name, view in pairs(VIEWS) do
  components[name] = function(env: kui.Env, props: P, inst: kui.Instance): kui.Node
    ensure_tokens(env)
    local c: ctx.Ctx = { env = env, s = settings, inst = inst.key }
    return view(c, props or {})
  end
end

global function on_event(ev: kui.Event): any
  local m = ev as P
  local kit = m.kit as string
  if not kit then return nil end
  local name = kit:match("^([%w_]+)%.")
  local handler = name and EVENTS[name]
  if not handler then return nil end
  return handler(m, kit)
end
