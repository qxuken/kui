-- App-specific assertions; the runner owns startup, time and cleanup.
return function()

local fleet = require("fleet")
local view = require("view")


local function names()
  local out = {}
  for i, v in ipairs(view.visible(model)) do out[i] = v.name end
  return out
end

  -- the table: worst first
  local n = names()
  assert(#n == 12 and n[1] == "inventory-sync" and n[2] == "checkout-api", "12 services, the down one first: " .. n[1])

  update({ kind = "filter", change = { value = "attention" } })
  assert(#names() == 3, "three need attention")
  update({ kind = "filter", change = { value = "down" } })
  assert(#names() == 1 and names()[1] == "inventory-sync", "one is down")
  update({ kind = "filter", change = { value = "all" } })
  update({ kind = "query", change = { text = "catalog" } })
  assert(#names() == 3, "the search reads teams too: " .. table.concat(names(), ","))
  update({ kind = "query", change = { text = "billing" } })
  assert(#names() == 0, "nothing named billing")
  update({ kind = "reset_filters" })
  assert(#names() == 12 and model.query == "" and model.filter == "all", "show all resets the search and the filter")

  -- sorting: a number sorts high to low first, again flips it
  update({ kind = "sort", sort = { field = "rps" } })
  assert(model.sort == "rps" and model.descending and names()[1] == "postgres-primary", "busiest first")
  update({ kind = "sort", sort = { field = "rps" } })
  assert(not model.descending and names()[1] == "inventory-sync", "again: quietest first")
  update({ kind = "sort", sort = { field = "name" } })
  assert(names()[1] == "auth-svc", "by name")
  update({ kind = "select", select = { row = 1 } })
  assert(model.selected == "catalog-api", "a row picked selects its service: " .. tostring(model.selected))

  -- the clock: an hour of samples, a second at a time
  local now, rev = fleet.now, model.chart.rev
  update({ kind = "tick" })
  assert(fleet.now == now + 1 and #fleet.series.t == 3600 and model.chart.rev ~= rev, "a tick adds a sample and drops the oldest")
  assert(#model.chart == 3600 and model.chart:get(3599, "ago") == 0 and model.chart:get(0, "ago") < -59.9, "x is minutes from now")
  update({ kind = "live", toggle = { on = false } })
  update({ kind = "tick" })
  assert(fleet.now == now + 1, "paused: ticks do nothing")
  update({ kind = "live", toggle = { on = true } })

  -- a rolling restart, confirmed
  update({ kind = "select", select = { row = 2 } })  -- by name: auth, catalog, checkout
  assert(model.selected == "checkout-api", "checkout picked")
  update({ kind = "restart" })
  assert(model.confirm == "checkout-api", "restart asks first")
  update({ kind = "cancel" })
  assert(model.confirm == nil and fleet.find("checkout-api").status == "degraded", "kept running")
  update({ kind = "restart" })
  update({ kind = "confirm_restart" })
  assert(fleet.find("checkout-api").status == "restarting" and model.toast:find("Rolling restart", 1, true), "restarting, said in a toast")
  kui.sleep(4200)
  local c = fleet.find("checkout-api")
  assert(c.status == "healthy" and c.p95 < 400 and model.toast_tone == "ok", "back healthy after the restart")

  -- scaling from the popover
  update({ kind = "scale" })
  assert(model.scaling == "checkout-api" and model.scale_to == 4, "the scale opens at the current count")
  update({ kind = "scale_to", change = { value = 6.4 } })
  update({ kind = "apply_scale" })
  assert(c.instances == 6 and model.scaling == nil and model.toast:find("4 → 6", 1, true), "scaled 4 → 6")

  -- the events tab and the skin
  update({ kind = "events_tab", change = { value = "errors" } })
  assert(model.events == "errors" and model.errors:get(0, "service") == "inventory-sync", "errors by service, worst first")
  for _, s in ipairs({ "neutral", "phosphor", "ledger" }) do update({ kind = "skin", change = { value = s } }) end
  kui.sleep(200)

  -- a window smaller than the dashboard scrolls it: the split wide, the
  -- halves stacked narrow, both in a page that scrolls down and sideways
  local function find(node, key)
    if type(node) ~= "table" then return nil end
    if node.key == key then return node end
    for _, child in ipairs(node) do
      local hit = find(child, key)
      if hit then return hit end
    end
  end
  local wide = view.window(model, { viewport_w = 1320, viewport_h = 600 })
  local page = find(wide, "page")
  assert(page and page.scroll_y and page.scroll_x and find(page, "split-box") ~= nil, "wide: the split, in a page that scrolls")
  local narrow = view.window(model, { viewport_w = 760, viewport_h = 560 })
  assert(find(narrow, "stacked") ~= nil and find(narrow, "split-box") == nil, "narrow: the halves one over the other")

  -- the numbers drift, they do not jump: ten minutes of ticks
  local s = fleet.series
  local worst_step, auth = 0, fleet.find("auth-svc")
  local lo, hi = auth.rps, auth.rps
  for _ = 1, 600 do
    fleet.tick()
    local n = #s.p95
    worst_step = math.max(worst_step, math.abs(s.p95[n] - s.p95[n - 1]))
    lo, hi = math.min(lo, auth.rps), math.max(hi, auth.rps)
  end
  assert(worst_step < 20, ("a second's step of the edge's p95 is small: %.1f ms"):format(worst_step))
  assert(lo > 520 * 0.8 and hi < 520 * 1.2, ("a service stays about its level: %.0f..%.0f req/s"):format(lo, hi))

end
