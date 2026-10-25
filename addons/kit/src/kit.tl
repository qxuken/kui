-- Kit's entry: the script a host loads. Every slot under its namespace is
-- an instance, named `<component>` or `<component>#<n>` (`button#3`), so
-- one extension serves all of them (`slots = { "*" }`). That naming is
-- M0's stand-in for position-keyed instances (ADR B, decision 1).
--
-- `settings` is a slot of its own that draws nothing: the app's Kit
-- settings ride in its params, and the skin's tokens are declared from
-- them whenever they change.
--
-- The modules beside this file are found through `package.path`, which
-- every host's loader starts with this file's folder (ADR 0059).

local skin = require("skin")
local ledger = require("ledger")
local ctx = require("ctx")
local button = require("button")
local toolbar = require("toolbar")
local datatable = require("datatable")
local waterfall = require("waterfall")

global slots: {string} = { "*" }

local settings: skin.Settings = { shadows = true, density = "comfortable" }
local declared: string = nil
local params: {string:any} = {}

local function settings_key(s: skin.Settings): string
  return tostring(s.shadows) .. "/" .. tostring(s.density)
end

local function ensure_tokens(env: kui.Env)
  local k = settings_key(settings)
  if declared ~= k then
    env.set_tokens(skin.tokens(ledger, settings))
    declared = k
  end
end

local function component(id: string): string
  return id:match("^([^#]+)") or id
end

global function view(env: kui.Env, slot: kui.Slot): kui.Node
  local id = slot.name
  local p = slot.params or {}
  local kind = component(id)
  if kind == "settings" then
    settings = {
      shadows = p.shadows ~= false,
      density = p.density == "compact" and "compact" or "comfortable",
    }
  end
  ensure_tokens(env)
  params[id] = p
  local c: ctx.Ctx = { env = env, skin = ledger, settings = settings, id = id }
  if kind == "button" then
    return button.view(p as button.Params, c)
  elseif kind == "toolbar" then
    return toolbar.view(p as toolbar.Params, c)
  elseif kind == "table" then
    return datatable.view(p as datatable.Params, c)
  elseif kind == "waterfall" then
    return waterfall.view(p as waterfall.Params, c)
  end
  return column { width = 0, height = 0 }
end

global function on_event(ev: kui.Event): {string:any}
  local m = ev as {string:any}
  local id = m.id as string
  if not id then return nil end
  local kind = component(id)
  local p = params[id] or {}
  if kind == "button" then
    return button.event(ev, p as button.Params)
  elseif kind == "toolbar" then
    return toolbar.event(ev, p as toolbar.Params)
  elseif kind == "table" then
    return datatable.event(ev, p as datatable.Params)
  elseif kind == "waterfall" then
    return waterfall.event(ev, p as waterfall.Params, id)
  end
  return nil
end
