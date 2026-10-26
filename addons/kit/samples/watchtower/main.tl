-- Watchtower: an ops dashboard on Kit and the kui runner, in Teal. Larder's
-- services in a sortable table, the edge's latency and throughput live,
-- the selected service's detail with a rolling restart and a scale, and
-- the incident log — all fed by a timer from a simulated fleet (fleet.tl).
--
--   kui run addons/kit/samples/watchtower
--   kui check addons/kit/samples/watchtower

local fleet = require("fleet")
local state = require("state")
local view_of = require("view")

local type Model = state.Model
local type Msg = state.Msg

global model: Model

-- Whether two lists hold the same values.
local function same(a: {any}, b: {any}): boolean
  if #a ~= #b then return false end
  for i = 1, #a do if a[i] ~= b[i] then return false end end
  return true
end

-- The edge's series, the log, the errors and each service's trend as
-- channels, published again each tick — each only when it moved, so the
-- components that draw one that did not are kept as they were. `logged`
-- says whether the tick added a log line.
local function publish(m: Model, logged: boolean)
  local s = fleet.series
  -- x is minutes from now, so the chart's ends read "-15" and "0"
  local ago: {any} = {}
  for i, t in ipairs(s.t) do ago[i] = (t - fleet.now) / 60 end
  local cols: {string:{any}} = { ago = ago, p50 = s.p50, p95 = s.p95, p99 = s.p99, rps = s.rps, errors = s.errors }
  m.chart = m.chart and m.chart:publish(cols) or channel(cols)
  if logged or not m.log then
    local time, level, message: {any}, {any}, {any} = {}, {}, {}
    for i, l in ipairs(fleet.lines) do time[i], level[i], message[i] = l.time, l.level, l.message end
    local lc: {string:{any}} = { time = time, level = level, message = message }
    m.log = m.log and m.log:publish(lc) or channel(lc)
  end
  -- the services erring at all, worst first
  local erring: {fleet.Service} = {}
  for _, v in ipairs(fleet.services) do if v.errors > 0.1 then erring[#erring + 1] = v end end
  table.sort(erring, function(a: fleet.Service, b: fleet.Service): boolean return a.errors > b.errors end)
  local label, value: {any}, {any} = {}, {}
  for i, v in ipairs(erring) do
    label[i] = v.name
    value[i] = math.min(v.errors, 5)
  end
  if not m.errors or not same(m.errors:column("service") as {any}, label) or not same(m.errors:column("rate") as {any}, value) then
    local ec: {string:{any}} = { service = label, rate = value }
    m.errors = m.errors and m.errors:publish(ec) or channel(ec)
  end
  m.trends = m.trends or {}
  for _, v in ipairs(fleet.services) do
    local ch = m.trends[v.name]
    local tc: {string:{any}} = { rps = v.trend as {any} }
    m.trends[v.name] = ch and ch:publish(tc) or channel(tc)
  end
  m.updated = fleet.clock(fleet.now)
end

global function init(): Model
  local m: Model = {
    skin = "ledger",
    env = "prod-eu",
    range = "15m",
    live = true,
    query = "",
    filter = "all",
    sort = "status",
    descending = false,
    selected = "checkout-api",
    events = "log",
    split = 0.64,
    tick = 0,
  }
  m.latencies = channel({ ms = fleet.latencies(2400) as {any} })
  publish(m, true)
  view_of.publish_services(m)
  return m
end

-- A second of the fleet; a global so a --watch reload keeps one timer.
global ticker: kui.Timer
if not ticker then
  ticker = kui.timer.every(1000, function() kui.dispatch({ kind = "tick" }) end)
end

local function toast(m: Model, text: string, tone: string)
  m.toast, m.toast_tone = text, tone
end

global function update(msg: Msg)
  local m = model
  local k = msg.kind
  if k == "tick" then
    if m.live then
      local logged = fleet.tick()
      m.tick = m.tick + 1
      publish(m, logged)
    end
  elseif k == "skin" then m.skin = msg.change.value as string
  elseif k == "env" then m.env = msg.change.value as string
  elseif k == "range" then m.range = msg.change.value as string
  elseif k == "events_tab" then m.events = msg.change.value as string
  elseif k == "live" then m.live = msg.toggle.on as boolean
  elseif k == "query" then m.query = msg.change.text as string
  elseif k == "filter" then m.filter = msg.change.value as string
  elseif k == "reset_filters" then m.filter, m.query = "all", ""
  elseif k == "split" then m.split = msg.resize.fraction
  elseif k == "sort" then
    local f = msg.sort.field
    if m.sort == f then m.descending = not m.descending else m.sort, m.descending = f, f ~= "name" and f ~= "status" end
  elseif k == "select" then
    local v = view_of.visible(m)[msg.select.row + 1]
    if v then m.selected = v.name end
  elseif k == "restart" then m.confirm = m.selected
  elseif k == "cancel" then m.confirm = nil
  elseif k == "confirm_restart" then
    local v = fleet.find(m.confirm)
    m.confirm = nil
    if v then
      v.status = "restarting"
      toast(m, ("Rolling restart of %s started: %d instances, one at a time."):format(v.name, v.instances), "accent")
      kui.timer.after(4000, function() kui.dispatch({ kind = "restarted", name = v.name }) end)
    end
  elseif k == "restarted" then
    local v = fleet.find(msg.name)
    if v then
      v.status = v.name == "inventory-sync" and "down" or "healthy"
      if v.name == "checkout-api" then v.p95, v.errors, v.cpu = 210, 0.3, 48 end
      fleet.rebase(v.name)
      toast(m, v.status == "down" and v.name .. " restarted but is still down: the warehouse is not answering."
        or v.name .. " restarted: all instances healthy.", v.status == "down" and "warn" or "ok")
    end
  elseif k == "scale" then
    local v = fleet.find(m.selected)
    m.scaling, m.scale_to = m.selected, v and v.instances or 1
  elseif k == "scale_to" then m.scale_to = math.floor((msg.change.value as number) + 0.5)
  elseif k == "scale_off" then m.scaling = nil
  elseif k == "apply_scale" then
    local v = fleet.find(m.scaling)
    if v then
      local from = v.instances
      v.instances = m.scale_to
      v.cpu = math.min(99, v.cpu * from / v.instances)
      fleet.rebase(v.name)
      toast(m, ("%s scaled %d → %d instances."):format(v.name, from, v.instances), "ok")
    end
    m.scaling = nil
  elseif k == "toast_off" then m.toast = nil
  end
  -- The table's rows, as this message left the services, the filter and
  -- the sort.
  view_of.publish_services(m)
end

global function view(env: kui.Env): kui.Node
  return view_of.window(model, env)
end
