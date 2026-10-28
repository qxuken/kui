-- Reads what a spike run recorded of itself (KUI_PERF_TRACE) through
-- kui.trace: the frames between its `measure` and `done` marks, their
-- work's spread against the budget, and how many there were.
--
--   kui run tools/kui-perf/spike/analyze --headless -- RUN.kuitrace PHASE

local PATH, PHASE = kui.args[1], kui.args[2] or "?"
local rec = kui.trace.open(PATH, { watch = false })
local from, to
for i = 0, #rec.instants - 1 do
  if rec.instants:get(i, "kind") == "mark" then
    local label = rec.instants:get(i, "label")
    if label == "measure" then from = rec.instants:get(i, "at") end
    if label == "done" then to = rec.instants:get(i, "at") end
  end
end
local works = {}
local frames = rec.frames
for i = 0, #frames - 1 do
  local b = frames:get(i, "begin")
  if from and to and b >= from and b < to then works[#works + 1] = frames:get(i, "work") end
end
table.sort(works)
local n = #works
local sum = 0
for _, w in ipairs(works) do sum = sum + w end
local function at(q) return n > 0 and works[math.max(1, math.ceil(n * q))] or 0 end
print(string.format("%s\tframes %d\tavg %.3f\tp50 %.3f\tp95 %.3f\tmax %.3f ms\tover %.1fs",
  PHASE, n, n > 0 and sum / n or 0, at(0.5), at(0.95), n > 0 and works[n] or 0,
  ((to or 0) - (from or 0)) / 1000))
kui.exit(0)
