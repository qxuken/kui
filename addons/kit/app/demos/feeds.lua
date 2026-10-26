-- The live feeds the chart and log cards draw: a sample of two series
-- every 100 ms, kept to the last minute, and a log line every 400 ms, each
-- published as a channel into the app's model (`model.chart`,
-- `model.log`) while the toolbar's Live switch is on. main.lua starts the
-- timers and calls these on each tick.

local M = {}

-- What the model starts with.
M.state = {
  live = true, t = 0,
  series = { t = {}, cpu = {}, mem = {} }, chart = nil,
  log_lines = { time = {}, level = {}, message = {} }, log = nil,
}

-- A sample a tick: CPU swings on a slow wave with noise on it, memory on a
-- slower one; the oldest goes once there are 600 (a minute).
function M.tick(m)
  if not m.live then return end
  m.t = m.t + 0.1
  local s = m.series
  local n = #s.t + 1
  s.t[n] = m.t
  s.cpu[n] = 40 + 25 * math.sin(m.t * 0.9) + math.random() * 12
  s.mem[n] = 55 + 10 * math.sin(m.t * 0.21) + math.random() * 4
  if n > 600 then
    table.remove(s.t, 1); table.remove(s.cpu, 1); table.remove(s.mem, 1)
  end
  m.chart = m.chart and m.chart:publish(s) or channel(s)
end

local LEVELS = { "debug", "info", "info", "info", "warn", "error" }
local WORDS = { "request handled", "cache miss", "retrying upstream", "token refreshed",
  "query planned", "batch flushed", "connection reused", "slow response" }

-- A log line a tick, its level and words drawn at random.
function M.log_line(m)
  if not m.live then return end
  local l = m.log_lines
  local n = #l.time + 1
  l.time[n] = os.date("%H:%M:%S")
  l.level[n] = LEVELS[math.random(#LEVELS)]
  l.message[n] = WORDS[math.random(#WORDS)] .. " #" .. math.random(1000, 9999)
  m.log = m.log and m.log:publish(l) or channel(l)
end

return M
