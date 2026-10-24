-- Toolbar: the buttons it holds come in its params as data, one fill for
-- all of them. This is M0's stand-in for host children (ADR B, decision 2),
-- and the other half of the instance-cost comparison: twenty buttons in
-- one fill here, against twenty `button#i` slots of one each.

local button = require("button")
local ctx = require("ctx")

local record Params
  items: {button.Params}
end

local M = { Params = Params }

function M.view(p: Params, c: ctx.Ctx): kui.Node
  local t: kui.Props = { gap = 6, cross_align = "center", pad = { x = 10, y = 6 } }
  for i, item in ipairs(p.items or {}) do
    t[#t + 1] = button.view(item, c, i)
  end
  return row(t)
end

function M.event(ev: kui.Event, p: Params): {string:any}
  local i = (ev as {string:any}).index as integer
  local item = p.items and p.items[i]
  if item then return button.event(ev, item) end
  return nil
end

return M
