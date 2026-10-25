-- A picture of Watchtower: the fleet mid-incident, staged.
--
--   kui run test/shot.lua --shot shots/watchtower.png --after 1500 --size 1320x880 -- [skin] [state]
--
-- `skin` is ledger, neutral or phosphor; `state` one of overview,
-- attention, restart, scale, errors, spread, empty, live (the timer left on).
package.path = kui.dir .. "/../?.lua;" .. package.path
local skin, state = kui.args[1] or "ledger", kui.args[2] or "overview"
require("main")
model = init()
model.skin = skin
model.live = state == "live"

if state == "attention" then
  update({ kind = "filter", change = { value = "attention" } })
  update({ kind = "select", select = { row = 0 } })
  update({ kind = "range", change = { value = "1h" } })
elseif state == "restart" then
  update({ kind = "select", select = { row = 1 } })
  update({ kind = "restart" })
elseif state == "scale" then
  update({ kind = "sort", sort = { field = "rps" } })
  update({ kind = "select", select = { row = 0 } })
  update({ kind = "scale" })
  update({ kind = "scale_to", change = { value = 8 } })
elseif state == "errors" then
  update({ kind = "events_tab", change = { value = "errors" } })
  update({ kind = "range", change = { value = "5m" } })
elseif state == "spread" then
  update({ kind = "events_tab", change = { value = "spread" } })
elseif state == "empty" then
  update({ kind = "query", change = { text = "billing" } })
  model.selected = nil
end
