-- Generated from src/kit.tl by addons/kit/build.lua. Edit the .tl.












local skin = require("skin")
local ledger = require("ledger")
local ctx = require("ctx")
local button = require("button")
local toolbar = require("toolbar")
local datatable = require("datatable")
local waterfall = require("waterfall")

slots = { "*" }

local settings = { shadows = true, density = "comfortable" }
local declared = nil
local params = {}

local function settings_key(s)
   return tostring(s.shadows) .. "/" .. tostring(s.density)
end

local function ensure_tokens(env)
   local k = settings_key(settings)
   if declared ~= k then
      env.set_tokens(skin.tokens(ledger, settings))
      declared = k
   end
end

local function component(id)
   return id:match("^([^#]+)") or id
end

function view(env, slot)
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
   local c = { env = env, skin = ledger, settings = settings, id = id }
   if kind == "button" then
      return button.view(p, c)
   elseif kind == "toolbar" then
      return toolbar.view(p, c)
   elseif kind == "table" then
      return datatable.view(p, c)
   elseif kind == "waterfall" then
      return waterfall.view(p, c)
   end
   return column({ width = 0, height = 0 })
end

function on_event(ev)
   local m = ev
   local id = m.id
   if not id then return nil end
   local kind = component(id)
   local p = params[id] or {}
   if kind == "button" then
      return button.event(ev, p)
   elseif kind == "toolbar" then
      return toolbar.event(ev, p)
   elseif kind == "table" then
      return datatable.event(ev, p)
   elseif kind == "waterfall" then
      return waterfall.event(ev, p, id)
   end
   return nil
end