-- Generated from src/kit.tl by addons/kit/build.lua. Edit the .tl.














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




local SKINS = { ledger = ledger, phosphor = phosphor, neutral = neutral }

local settings = { skin = ledger, density = "comfortable", shadows = true }
local declared = nil





local function ensure_tokens(env)
   local k = skin.key(settings)
   if declared ~= k then
      env.set_tokens(skin.tokens(settings))
      env.set_rules_appearance(settings.appearance)
      declared = k
   end
end

local function num(v)
   return type(v) == "number" and v or nil
end

local function faces(v)
   if type(v) == "string" then return { v } end
   if type(v) == "table" and #v > 0 then return v end
   return nil
end

local function apply(p)
   local roles = p.roles
   settings = {
      skin = SKINS[p.skin] or ledger,
      density = p.density == "compact" and "compact" or "comfortable",
      shadows = p.shadows ~= false,
      radius = num(p.radius),
      roles = type(roles) == "table" and roles or nil,
      appearance = (p.appearance == "light" or p.appearance == "dark") and p.appearance or nil,
      hair = num(p.hair),
      rule_w = num(p.rule),
      font_ui = faces(p.font_ui),
      font_data = faces(p.font_data),
   }
end


local VIEWS = {}

local EVENTS = {}

local function provide(module, names)
   for _, name in ipairs(names) do
      VIEWS[name] = module[name]
      EVENTS[name] = module.event
   end
end

provide(layout, { "panel", "toolbar", "statusbar", "split", "tabs", "section", "divider" })
provide(inputs, {
   "button", "icon_button", "text_field", "search", "select", "checkbox", "checklist", "switch",
   "radio_group", "segmented", "slider", "text_area",
})
provide(data, { "table", "list", "inspector", "log", "badge", "stat", "code", "empty", "progress", "notice", "skeleton", "shader" })
provide(overlays, { "dialog", "popover", "menu", "toast", "toaster" })
provide(charts, { "sparkline", "line_chart", "bar_chart", "histogram" })
provide(waterfall, { "waterfall" })



components = {
   settings = function(env, props, _inst)
      apply(props or {})
      ensure_tokens(env)
      return column({ width = 0, height = 0 })
   end,
}

for name, view in pairs(VIEWS) do
   components[name] = function(env, props, inst)
      ensure_tokens(env)
      local c = { env = env, s = settings, inst = inst.key }
      local node = view(c, props or {})



      if node and node.focus_ring == nil then node.focus_ring = ctx.ref(c, "focus") end
      return node
   end
end

function on_event(ev)
   local m = ev
   local kit = m.kit
   if not kit then return nil end
   local name = kit:match("^([%w_]+)%.")
   local handler = name and EVENTS[name]
   if not handler then return nil end
   return handler(m, kit)
end