-- Watchtower end to end, without a window: the Teal app loaded as it runs,
-- driven through `update`, its state checked after each step.
--
--   kui run test --headless          (from the sample's folder)
--
-- The exit code is the number of failures.

package.path = kui.dir .. "/../?.lua;" .. package.path
require("main")
---@type any
model = init()
local fleet = require("fleet")
local view = require("view")

local failed = 0
local function check(ok, what)
  if ok then print("ok    " .. what) else failed = failed + 1; print("FAIL  " .. what) end
end

local function names()
  local out = {}
  for i, v in ipairs(view.visible(model)) do out[i] = v.name end
  return out
end

kui.spawn(function()
  -- the table: worst first
  local n = names()
  check(#n == 12 and n[1] == "inventory-sync" and n[2] == "checkout-api", "12 services, the down one first: " .. n[1])

  update({ kind = "filter", change = { value = "attention" } })
  check(#names() == 3, "three need attention")
  update({ kind = "filter", change = { value = "down" } })
  check(#names() == 1 and names()[1] == "inventory-sync", "one is down")
  update({ kind = "filter", change = { value = "all" } })
  update({ kind = "query", change = { text = "catalog" } })
  check(#names() == 3, "the search reads teams too: " .. table.concat(names(), ","))
  update({ kind = "query", change = { text = "billing" } })
  check(#names() == 0, "nothing named billing")
  update({ kind = "reset_filters" })
  check(#names() == 12 and model.query == "" and model.filter == "all", "show all resets the search and the filter")

  -- sorting: a number sorts high to low first, again flips it
  update({ kind = "sort", sort = { field = "rps" } })
  check(model.sort == "rps" and model.descending and names()[1] == "postgres-primary", "busiest first")
  update({ kind = "sort", sort = { field = "rps" } })
  check(not model.descending and names()[1] == "inventory-sync", "again: quietest first")
  update({ kind = "sort", sort = { field = "name" } })
  check(names()[1] == "auth-svc", "by name")
  update({ kind = "select", select = { row = 1 } })
  check(model.selected == "catalog-api", "a row picked selects its service: " .. tostring(model.selected))

  -- the clock: an hour of samples, a second at a time
  local now, rev = fleet.now, model.chart.rev
  update({ kind = "tick" })
  check(fleet.now == now + 1 and #fleet.series.t == 3600 and model.chart.rev ~= rev, "a tick adds a sample and drops the oldest")
  check(#model.chart == 3600 and model.chart:get(3599, "ago") == 0 and model.chart:get(0, "ago") < -59.9, "x is minutes from now")
  update({ kind = "live", toggle = { on = false } })
  update({ kind = "tick" })
  check(fleet.now == now + 1, "paused: ticks do nothing")
  update({ kind = "live", toggle = { on = true } })

  -- a rolling restart, confirmed
  update({ kind = "select", select = { row = 2 } })  -- by name: auth, catalog, checkout
  check(model.selected == "checkout-api", "checkout picked")
  update({ kind = "restart" })
  check(model.confirm == "checkout-api", "restart asks first")
  update({ kind = "cancel" })
  check(model.confirm == nil and fleet.find("checkout-api").status == "degraded", "kept running")
  update({ kind = "restart" })
  update({ kind = "confirm_restart" })
  check(fleet.find("checkout-api").status == "restarting" and model.toast:find("Rolling restart", 1, true), "restarting, said in a toast")
  kui.sleep(4200)
  local c = fleet.find("checkout-api")
  check(c.status == "healthy" and c.p95 < 400 and model.toast_tone == "ok", "back healthy after the restart")

  -- scaling from the popover
  update({ kind = "scale" })
  check(model.scaling == "checkout-api" and model.scale_to == 4, "the scale opens at the current count")
  update({ kind = "scale_to", change = { value = 6.4 } })
  update({ kind = "apply_scale" })
  check(c.instances == 6 and model.scaling == nil and model.toast:find("4 → 6", 1, true), "scaled 4 → 6")

  -- the events tab and the skin
  update({ kind = "events_tab", change = { value = "errors" } })
  check(model.events == "errors" and model.errors:get(0, "service") == "inventory-sync", "errors by service, worst first")
  for _, s in ipairs({ "neutral", "phosphor", "ledger" }) do update({ kind = "skin", change = { value = s } }) end
  kui.sleep(200)

  print(("%s: %d failure(s)"):format(failed == 0 and "PASS" or "FAIL", failed))
  kui.exit(failed)
end)
