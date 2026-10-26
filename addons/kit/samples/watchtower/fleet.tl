-- The fleet Watchtower watches, simulated: Larder's twelve services, the
-- edge's latency and throughput a sample a second, and an incident under
-- way (checkout slow since a deploy, the inventory sync down with the
-- warehouse). The same seed draws the same hour every run.

local record fleet
  enum Status
    "healthy" "degraded" "down" "restarting"
  end

  record Service
    name: string
    team: string
    version: string
    deployed: string
    instances: integer
    rps: number
    p95: number
    errors: number
    cpu: number
    status: Status
    note: string
    -- The last minute of requests a second, oldest first.
    trend: {number}
  end

  record Line
    time: string
    level: string
    message: string
  end

  -- The edge's samples, a second apart, oldest first: an hour of them.
  record Series
    t: {number}
    p50: {number}
    p95: {number}
    p99: {number}
    rps: {number}
    errors: {number}
  end

  services: {Service}
  series: Series
  lines: {Line}
  -- Seconds since the hour the series starts at.
  now: number

  clock: function(t: number): string
  tick: function(): boolean
  latencies: function(n: integer): {number}
  find: function(name: string): Service
  -- Its new numbers are its level now (a restart, a scale).
  rebase: function(name: string)
end

local HOUR = 3600

-- A small, repeatable generator.
local seed = 0x5EED1234
local function rand(): number
  seed = (seed * 1103515245 + 12345) % 2147483648
  return seed / 2147483648
end

local function gauss(): number
  return (rand() + rand() + rand() + rand() - 2) / 0.58
end

local function round(x: number, places: integer): number
  local f = 10 ^ places
  return math.floor(x * f + 0.5) / f
end

local function svc(name: string, team: string, version: string, deployed: string, instances: integer, rps: number,
  p95: number, errors: number, cpu: number, status: fleet.Status, note?: string): fleet.Service
  local trend: {number} = {}
  for i = 1, 60 do trend[i] = math.max(0, rps * (1 + 0.08 * gauss())) end
  return { name = name, team = team, version = version, deployed = deployed, instances = instances, rps = rps, p95 = p95,
    errors = errors, cpu = cpu, status = status, note = note, trend = trend }
end

fleet.services = {
  svc("edge-gateway", "platform", "1.25.4", "Oct 21 09:12", 6, 1840, 48, 0.21, 41, "healthy"),
  svc("checkout-api", "checkout", "3.18.2", "Oct 25 13:47", 4, 212, 1180, 2.9, 87, "degraded",
    "p95 above 800 ms since the 13:47 deploy (3.18.2)"),
  svc("catalog-api", "catalog", "2.40.0", "Oct 23 16:05", 5, 690, 62, 0.08, 38, "healthy"),
  svc("search-svc", "catalog", "1.9.3", "Oct 22 11:30", 3, 410, 95, 0.12, 52, "healthy"),
  svc("payments-adapter", "checkout", "0.31.7", "Oct 19 08:44", 2, 96, 310, 0.64, 29, "healthy"),
  svc("inventory-sync", "fulfilment", "4.2.1", "Oct 24 18:20", 2, 0, 0, 100, 4, "down",
    "wms-eu-2 not answering: 503 upstream_timeout on every sync"),
  svc("orders-worker", "fulfilment", "3.18.2", "Oct 25 13:47", 3, 148, 220, 0.4, 61, "healthy"),
  svc("notifications", "growth", "1.4.0", "Oct 18 10:02", 2, 64, 140, 0.05, 18, "healthy"),
  svc("auth-svc", "platform", "2.2.9", "Oct 20 14:55", 3, 520, 22, 0.02, 33, "healthy"),
  svc("pricing-engine", "catalog", "5.0.0-rc3", "Oct 25 10:31", 2, 260, 410, 1.3, 74, "degraded",
    "rc build: cache hit rate 61% (was 94%)"),
  svc("media-proxy", "platform", "1.1.12", "Oct 17 12:40", 4, 930, 34, 0.01, 22, "healthy"),
  svc("postgres-primary", "platform", "16.4", "Sep 30 03:00", 1, 2210, 6, 0.0, 58, "healthy"),
}

-- The incident's shape over the hour: checkout's deploy at minute 47.
local DEPLOY = 47 * 60

-- Slow noise: each sample leans on the last, as real metrics do.
local drift: {number} = { 0, 0, 0, 0, 0, 0, 0, 0 }
local function wander(k: integer, keep: number): number
  drift[k] = drift[k] * keep + gauss() * (1 - keep)
  return drift[k]
end

local function sample(s: fleet.Series, t: number)
  local i = #s.t + 1
  -- the deploy's effect ramps in over two minutes
  local hot = math.max(0, math.min(1, (t - DEPLOY) / 120))
  local wave = math.sin(t / 420) * 0.07 + wander(1, 0.995) * 1.2
  local base = 44 * (1 + wave)
  s.t[i] = t
  s.p50[i] = round(base + wander(2, 0.8) * 3, 1)
  s.p95[i] = round(base * 2.6 + hot * (190 + wander(3, 0.97) * 120) + wander(6, 0.9) * 4, 1)
  s.p99[i] = round(base * 4.4 + hot * (540 + wander(4, 0.97) * 300) + math.abs(wander(7, 0.9)) * 18, 1)
  s.rps[i] = round(1840 * (1 + wave * 1.4) + wander(8, 0.9) * 18, 0)
  s.errors[i] = round(math.max(0, 0.22 + hot * 1.05 + wander(5, 0.85) * 0.9), 2)
end

local function clock(t: number): string
  local h = 13 + math.floor(t / HOUR)
  local m = math.floor(t / 60) % 60
  local sec = math.floor(t) % 60
  return string.format("%02d:%02d:%02d", h % 24, m, sec)
end
fleet.clock = clock

fleet.series = { t = {}, p50 = {}, p95 = {}, p99 = {}, rps = {}, errors = {} }
for t = 0, HOUR - 1 do sample(fleet.series, t) end
fleet.now = HOUR - 1

fleet.lines = {}
local function log(t: number, level: string, message: string)
  fleet.lines[#fleet.lines + 1] = { time = clock(t), level = level, message = message }
end

log(2 * 60 + 14, "info", "deploy notifications 1.4.0 → 1.4.0 (config only) finished in 41 s")
log(9 * 60 + 3, "info", "autoscaler: media-proxy 3 → 4 instances (cpu 71%)")
log(18 * 60 + 40, "warn", "pricing-engine: cache hit rate 78% (threshold 85%)")
log(23 * 60 + 11, "info", "certificate *.larder.app renewed, expires 2027-01-23")
log(31 * 60 + 2, "warn", "pricing-engine: cache hit rate 61% (threshold 85%)")
log(36 * 60 + 55, "error", "inventory-sync: POST wms-eu-2/v2/stock → 503 upstream_timeout (900 ms)")
log(37 * 60 + 25, "error", "inventory-sync: 3 consecutive failures, circuit open for 60 s")
log(38 * 60 + 0, "info", "paged #oncall-fulfilment: \27[1minventory-sync down\27[0m (INC-2291)")
log(41 * 60 + 18, "info", "INC-2291 acknowledged by j.weber")
log(44 * 60 + 9, "error", "inventory-sync: circuit half-open probe failed: 503")
log(47 * 60 + 2, "info", "deploy checkout-api 3.18.1 → 3.18.2 started by ci (pipeline #18842)")
log(47 * 60 + 51, "info", "deploy checkout-api 3.18.2 finished: 4/4 instances healthy")
log(49 * 60 + 30, "warn", "checkout-api: p95 812 ms over 5 min (SLO 400 ms)")
log(52 * 60 + 4, "warn", "checkout-api: p95 1.1 s, error rate 2.4%")
log(52 * 60 + 40, "error", "checkout-api: POST /v1/checkout 504 from payments-adapter in 3 of 120 requests")
log(53 * 60 + 15, "info", "opened \27[1mINC-2292\27[0m checkout latency after 3.18.2 — severity 2")
log(55 * 60 + 47, "info", "INC-2292: m.okafor investigating; suspect N+1 query in basket totals")
log(58 * 60 + 2, "warn", "checkout-api: cpu 87% on 4/4 instances")

local MESSAGES: {{string, string}} = {
  { "info", "edge-gateway: 0 5xx in the last minute" },
  { "warn", "checkout-api: p95 %d ms over 1 min" },
  { "info", "autoscaler: checkout-api at 4/8 instances, holding (cooldown)" },
  { "error", "inventory-sync: probe → 503 upstream_timeout" },
  { "info", "orders-worker: queue depth %d" },
  { "warn", "pricing-engine: cache hit rate %d%%" },
}

-- Each service's level, as it first ticked: rate, p95, cpu.
local baseline: {string:{number}} = {}

-- One more second: a sample for the edge and the services, and now and
-- then a log line. Returns whether a line was added.
function fleet.tick(): boolean
  fleet.now = fleet.now + 1
  local s = fleet.series
  sample(s, fleet.now)
  for _, k in ipairs({ "t", "p50", "p95", "p99", "rps", "errors" }) do
    table.remove((s as {string:{number}})[k], 1)
  end
  for _, v in ipairs(fleet.services) do
    if v.status ~= "down" then
      -- Each pulled back toward where it sat, with a little noise: a
      -- service's numbers drift about their level, as real ones do, and
      -- do not walk off (a walk with no pull ran a service's rate and p95
      -- anywhere in an hour).
      local b = baseline[v.name]
      if not b then
        b = { v.rps, v.p95, v.cpu }
        baseline[v.name] = b
      end
      v.rps = math.max(0, v.rps + (b[1] - v.rps) * 0.05 + b[1] * 0.006 * gauss())
      v.p95 = math.max(1, v.p95 + (b[2] - v.p95) * 0.05 + b[2] * 0.01 * gauss())
      v.cpu = math.min(99, math.max(1, v.cpu + (b[3] - v.cpu) * 0.05 + 0.6 * gauss()))
    end
    table.remove(v.trend, 1)
    v.trend[#v.trend + 1] = v.status == "down" and 0 or v.rps
  end
  if rand() < 0.2 then
    local pick = MESSAGES[1 + math.floor(rand() * #MESSAGES)]
    local n = pick[2]:find("%d", 1, true) and math.floor(800 + rand() * 600) or 0
    if pick[2]:find("queue", 1, true) then n = math.floor(rand() * 40) end
    if pick[2]:find("cache", 1, true) then n = math.floor(55 + rand() * 10) end
    log(fleet.now, pick[1], (pick[2]:format(n)))
    if #fleet.lines > 500 then table.remove(fleet.lines, 1) end
    return true
  end
  return false
end

-- Latency samples for the histogram: the last 5 minutes' requests, roughly.
function fleet.latencies(n: integer): {number}
  local out: {number} = {}
  for i = 1, n do
    local slow = rand() < 0.18
    out[i] = round(math.exp(3.7 + 0.5 * gauss()) + (slow and 400 + 300 * rand() or 0), 0)
  end
  return out
end

function fleet.rebase(name: string)
  baseline[name] = nil
end

function fleet.find(name: string): fleet.Service
  for _, v in ipairs(fleet.services) do if v.name == name then return v end end
end

return fleet
