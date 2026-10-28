-- kui-perf's spike (ADR 0075, decision 11; backlog F252): Kit's waterfall
-- and table over a long recording, driven a step a tick through one phase,
-- the runner recording itself (KUI_PERF_TRACE) for `analyze.lua` to read.
--
--   kui run tools/kui-perf/spike --headless -- PHASE RECORDING [STEPS]
--
-- PHASE: `still` (nothing moves), `pan` (a 2 s window across the whole),
-- `zoom` (the window halved a step, sixteen times over), `full` (zoomed
-- fully out, the view built anew each step), `table` (the frame table's
-- selection 25 rows on a step), `idle` (nothing, for STEPS ms; 2000 by
-- default).

local kit = require("kit.ui")

local T0 = kui.now()
local PHASE = kui.args[1] or "pan"
local PATH = kui.args[2] or "spike.kuitrace"
local STEPS = tonumber(kui.args[3] or "240")
local WARM = 20

local rec = kui.trace.open(PATH, { watch = false })
local OPENED = kui.now() - T0
local session = rec:session()
local ALL = session.finish - session.start

local COLUMNS = {
  { field = "frame", title = "frame", width = 70, mono = true, align = "end" },
  { field = "work", title = "work", width = 80, mono = true, align = "end" },
  { field = "build", title = "build", width = 80, mono = true, align = "end" },
  { field = "layout", title = "layout", width = 80, mono = true, align = "end" },
  { field = "verdict", title = "verdict", width = 90 },
  { field = "causes", title = "why it ran" },
}

function init()
  return { step = 0, first = true }
end

local function window(m)
  if PHASE == "pan" then
    local len = 2000
    return session.start + (ALL - len) * m.step / STEPS, len
  elseif PHASE == "zoom" then
    local len = ALL / 2 ^ (m.step % 16)
    return session.start + (ALL - len) / 2, len
  elseif PHASE == "full" then
    -- The whole, nudged a hair a step so the view is asked again.
    return session.start - (m.step % 2) * 0.001, ALL
  end
  return nil, nil
end

function update(msg)
  if msg.kind == "tick" then model.step = model.step + 1 end
end

function view()
  local m = model
  if m.first then
    m.first = false
    kui.next_frame(function()
      print(string.format("open\t%.1f\tfirst_frame\t%.1f", OPENED, kui.now() - T0))
    end)
  end
  local start, length = window(m)
  local selected = nil
  if PHASE == "table" then selected = (m.step * 25) % #rec.frames end
  return column { key = "root", width = "grow", height = "grow", gap = 8, pad = 8,
    kit.settings({ skin = "phosphor", density = "compact" }),
    text(string.format("%s · %d frames · %d spans", PHASE, #rec.frames, #rec.spans)),
    column { key = "wf", width = "grow", height = 320,
      kit.waterfall({ key = "waterfall", bars = rec.spans, marks = rec.instants,
        start = start, length = length, grow = true }) },
    column { key = "tbl", width = "grow", height = "grow",
      kit.table({ key = "frames", rows = rec.frames, columns = COLUMNS, selected = selected, grow = true }) },
  }
end

local function mark(name)
  kui.next_frame(function(env) env.perf_mark(name) end)
end

if PHASE == "idle" or PHASE == "still" then
  -- No timer ticking: what is measured is what the app draws on its own.
  local IDLE_MS = tonumber(kui.args[3] or "2000")
  kui.timer.after(300, function()
    mark("measure")
    kui.timer.after(IDLE_MS, function()
      mark("done")
      kui.timer.after(50, function() kui.exit(0) end)
    end)
  end)
else
  kui.timer.every(1, function()
    if model.step == WARM then mark("measure") end
    if model.step == WARM + STEPS then mark("done") end
    if model.step > WARM + STEPS + 2 then kui.exit(0) end
    kui.dispatch({ kind = "tick" })
  end)
end
