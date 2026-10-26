-- The live feeds the chart and log cards draw: a sample of two series
-- every 100 ms, kept to the last minute, and a log line every 400 ms, kept
-- to the last 500, each published as a channel into the app's model
-- (`model.chart`, `model.log`).
--
-- A feed ticks only while the Live switch is on and the page in view
-- reads it (`M.PACE`): a timer that ran on behind another page cost the
-- app a view a tick for nothing anyone saw. main.lua calls `M.pace` after
-- every message, which starts and stops the timers, and these on each
-- tick.

local M = {}

-- What the model starts with.
M.state = {
  live = true, t = 0,
  series = { t = {}, cpu = {}, mem = {} }, chart = nil,
  log_lines = { time = {}, level = {}, message = {} }, log = nil,
}

-- Which feeds each page reads, and how often they tick there (ms): the
-- Charts page draws the series live; the Data page's log card the log;
-- Overview only counts the samples, so a second a tick does (ten samples
-- at once, the series keeping its pace).
M.PACE = {
  charts = { tick = 100 },
  data = { log = 400 },
  overview = { tick = 1000 },
}

-- The last minute of samples, and the last 500 log lines.
M.SAMPLES, M.LINES = 600, 500

-- Starts the timers the page in view reads and stops the rest; `timers`
-- is the app's (a global, so a --watch reload keeps them), by feed. A
-- feed that starts ticks at once too, so the page it was started for
-- draws it in its first frame rather than a period later.
function M.pace(m, timers)
  local want = m.live and M.PACE[m.page] or {}
  for _, feed in ipairs({ "tick", "log" }) do
    local ms, now = want[feed], timers[feed]
    if now and now.ms ~= ms then
      now.timer:cancel()
      timers[feed], now = nil, nil
    end
    if ms and not now then
      local steps = math.max(1, ms // 100)
      timers[feed] = { ms = ms, timer = kui.timer.every(ms, function() kui.dispatch({ kind = feed, steps = steps }) end) }
      kui.dispatch({ kind = feed })
    end
  end
end

-- Drops the first `n` of a list in place, in one pass.
local function drop(list, n)
  table.move(list, n + 1, #list + n, 1)
end

-- `steps` samples (one by default): CPU swings on a slow wave with noise
-- on it, memory on a slower one; the oldest go past 600 (a minute).
function M.tick(m, steps)
  if not m.live then return end
  local s = m.series
  for _ = 1, steps or 1 do
    m.t = m.t + 0.1
    local n = #s.t + 1
    s.t[n] = m.t
    s.cpu[n] = 40 + 25 * math.sin(m.t * 0.9) + math.random() * 12
    s.mem[n] = 55 + 10 * math.sin(m.t * 0.21) + math.random() * 4
  end
  local over = #s.t - M.SAMPLES
  if over > 0 then drop(s.t, over); drop(s.cpu, over); drop(s.mem, over) end
  m.chart = m.chart and m.chart:publish(s) or channel(s)
end

local LEVELS = { "debug", "info", "info", "info", "warn", "error" }
local WORDS = { "request handled", "cache miss", "retrying upstream", "token refreshed",
  "query planned", "batch flushed", "connection reused", "slow response" }

-- A log line a tick, its level and words drawn at random; the oldest go
-- past 500. The whole log is published again each line (a channel has no
-- append), so it is kept short: an hour's 9,000 lines were most of what
-- the app did.
function M.log_line(m)
  if not m.live then return end
  local l = m.log_lines
  local n = #l.time + 1
  l.time[n] = os.date("%H:%M:%S")
  l.level[n] = LEVELS[math.random(#LEVELS)]
  l.message[n] = WORDS[math.random(#WORDS)] .. " #" .. math.random(1000, 9999)
  local over = n - M.LINES
  if over > 0 then drop(l.time, over); drop(l.level, over); drop(l.message, over) end
  m.log = m.log and m.log:publish(l) or channel(l)
end

return M
