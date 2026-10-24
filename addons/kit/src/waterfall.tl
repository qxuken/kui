-- Trace waterfall: bars on lanes by depth over a time axis, zoomed with the
-- wheel and panned with a drag, a click selecting a bar.
--
-- Input, source-agnostic: each bar is a start and a length (ms), a depth
-- (its lane), a group (its colour), a name and a target (what a click
-- picks). It arrives once by revision, as the table's rows do.
--
-- Cost is bounded by the width, not the bar count: bars narrower than the
-- current pixel merge into one density strip per run, read from levels
-- built once when the bars arrive (each level merges at half the last
-- one's threshold), so a fully zoomed-out recording of 18,000 bars draws a
-- strip per lane rather than walking every bar.

local skin = require("skin")
local ctx = require("ctx")
local button = require("button")

local record Bar
  s: number
  l: number
  d: integer
  g: integer
  n: string
  t: string
end

local record Mark
  at: number
  label: string
end

local record Data
  rev: integer
  bars: {Bar}
  marks: {Mark}
end

local record Window
  start: number
  len: number
end

local record Params
  data: Data
  -- A window the host controls; without one the waterfall keeps its own.
  view: Window
  on_select: any
end

-- One level: runs of intervals sorted by start. `one[i]` is the bar's
-- index in the lane when interval i is a single bar, 0 for a strip.
local record Level
  thr: number
  s: {number}
  e: {number}
  one: {integer}
end

local record Lane
  bars: {Bar}
  s: {number}
  e: {number}
  levels: {Level}
end

local record Index
  rev: integer
  lanes: {Lane}
  marks: {Mark}
  lo: number
  hi: number
  count: integer
end

local record State
  index: Index
  v0: number
  v1: number
  sel_lane: integer
  sel_bar: integer
  -- The lanes' rect last frame, for wheel zoom about the pointer.
  x: number
  w: number
end

local M = { Params = Params }

local LANE_H = 20
local MIN_BAR = 2
-- Levels merge at extent / 2^k for k in FINEST..COARSEST; finer than the
-- last, the raw bars are drawn.
local COARSEST, FINEST = 4, 22

local states: {string:State} = {}

-- The level for threshold `thr`: a bar at least `thr` long stays a bar; a
-- run of shorter ones whose gaps are under `thr` becomes one strip.
local function level(bars: {Bar}, thr: number): Level
  local lv: Level = { thr = thr, s = {}, e = {}, one = {} }
  local n = 0
  local i = 1
  while i <= #bars do
    local b = bars[i]
    if b.l >= thr then
      n = n + 1
      lv.s[n], lv.e[n], lv.one[n] = b.s, b.s + b.l, i
      i = i + 1
    else
      local s, e = b.s, b.s + b.l
      local j = i + 1
      while j <= #bars and bars[j].l < thr and bars[j].s - e < thr do
        local ej = bars[j].s + bars[j].l
        if ej > e then e = ej end
        j = j + 1
      end
      n = n + 1
      lv.s[n], lv.e[n], lv.one[n] = s, e, (j == i + 1) and i or 0
      i = j
    end
  end
  return lv
end

local function build(d: Data): Index
  local lanes: {Lane} = {}
  local lo, hi = math.huge, -math.huge
  for _, b in ipairs(d.bars or {}) do
    local k = b.d + 1
    local lane = lanes[k]
    if not lane then
      for m = #lanes + 1, k do lanes[m] = { bars = {}, s = {}, e = {}, levels = {} } end
      lane = lanes[k]
    end
    lane.bars[#lane.bars + 1] = b
    if b.s < lo then lo = b.s end
    if b.s + b.l > hi then hi = b.s + b.l end
  end
  if lo > hi then lo, hi = 0, 1 end
  local extent = hi - lo
  for _, lane in ipairs(lanes) do
    table.sort(lane.bars, function(a: Bar, b: Bar): boolean return a.s < b.s end)
    for i, b in ipairs(lane.bars) do lane.s[i], lane.e[i] = b.s, b.s + b.l end
    for k = COARSEST, FINEST do
      lane.levels[#lane.levels + 1] = level(lane.bars, extent / (2 ^ k))
    end
  end
  return { rev = d.rev, lanes = lanes, marks = d.marks or {}, lo = lo, hi = hi, count = #(d.bars or {}) }
end

local function state(c: ctx.Ctx, p: Params): State
  local st = states[c.id]
  local d = p.data
  if d and d.bars and (not st or st.index.rev ~= d.rev) then
    local idx = build(d)
    st = { index = idx, v0 = idx.lo, v1 = idx.hi, x = 0, w = 0 }
    states[c.id] = st
  end
  if not st then
    st = { index = build({ rev = -1, bars = {} }), v0 = 0, v1 = 1, x = 0, w = 0 }
    states[c.id] = st
  end
  if p.view then
    st.v0, st.v1 = p.view.start, p.view.start + p.view.len
  end
  return st
end

-- The first index whose end reaches `t`, by binary search over starts
-- (ends are monotone within a level: runs never overlap, and a lane's
-- bars at one depth do not nest).
local function first_at(s: {number}, e: {number}, t: number): integer
  local lo, hi = 1, #s
  while lo < hi do
    local mid = (lo + hi) // 2
    if e[mid] < t then lo = mid + 1 else hi = mid end
  end
  return lo
end

local function fmt(ms: number, step: number): string
  if step >= 1000 then return string.format("%.1f s", ms / 1000) end
  if step >= 1 then return string.format("%.0f ms", ms) end
  return string.format("%.2f ms", ms)
end

local function nice(raw: number): number
  local p = 10 ^ math.floor(math.log(raw, 10))
  for _, m in ipairs({ 1, 2, 5, 10 }) do
    if m * p >= raw then return m * p end
  end
  return 10 * p
end

local function lane_row(lane: Lane, k: integer, st: State, c: ctx.Ctx, width: number): kui.Node
  local scale = width / (st.v1 - st.v0)
  local px = 1 / scale
  local extent = st.index.hi - st.index.lo
  -- The coarsest level whose threshold is still under two pixels; finer
  -- than every level, the raw bars.
  local lv: Level = nil
  local kk = math.floor(math.log(extent / (px * MIN_BAR), 2))
  if kk <= FINEST then
    lv = lane.levels[math.max(kk, COARSEST) - COARSEST + 1]
  end
  local ss = lv and lv.s or lane.s
  local ee = lv and lv.e or lane.e
  local t: kui.Props = { height = LANE_H, width = "grow", cross_align = "center", clip = true }
  if #ss == 0 then return row(t) end
  local cursor = 0.0
  local i = first_at(ss, ee, st.v0)
  while i <= #ss and ss[i] <= st.v1 do
    local x0 = math.max(0, (ss[i] - st.v0) * scale)
    local x1 = math.min(width, (ee[i] - st.v0) * scale)
    local w = x1 - x0
    if w > 0 then
      if x0 > cursor then
        t[#t + 1] = column { width = x0 - cursor }
      end
      local one = lv and lv.one[i] or i
      if one ~= 0 and w >= MIN_BAR then
        local b = lane.bars[one]
        local sel = st.sel_lane == k and st.sel_bar == one
        local bar: kui.Props = {
          -- Named whether or not its text fits: a narrow bar is still a control.
          label = b.n,
          width = w,
          height = LANE_H - 4,
          bg = sel and "$kitSel" or ("$kitGroup" .. b.g),
          clip = true,
          pad = { x = 4 },
          cross_align = "center",
          on_click = { kit = "pick", id = c.id, lane = k, index = one },
        }
        if w >= 4 then bar.border = { w = 1, color = "$kitInk" } end
        if w >= 48 then
          bar[#bar + 1] = text(b.n, { size = 11, max_lines = 1, ellipsis = true, family = "$kitData",
            color = sel and "$kitSelFg" or "$kitFg" })
        end
        t[#t + 1] = row(bar)
      else
        t[#t + 1] = column { width = math.max(w, 1), height = LANE_H - 10, bg = "$kitRule" }
      end
      cursor = x0 + math.max(w, 1)
    end
    i = i + 1
  end
  return row(t)
end

function M.view(p: Params, c: ctx.Ctx): kui.Node
  local st = state(c, p)
  local env = c.env
  local r = env.layout_of("lanes")
  if r then st.x, st.w = r.x, r.w end
  local width = st.w > 0 and st.w or math.max(100, env.viewport_w - 40)
  local scale = width / (st.v1 - st.v0)

  -- The axis: a tick about every 100 px at a round step.
  local step = nice(100 / scale)
  local axis: kui.Props = { height = 18, width = "grow", clip = true, cross_align = "end" }
  local t0 = math.ceil(st.v0 / step) * step
  local lead = (t0 - st.v0) * scale
  if lead > 0 then axis[#axis + 1] = column { width = lead } end
  local tick = t0
  while tick <= st.v1 do
    axis[#axis + 1] = column { width = step * scale, text(fmt(tick, step), { size = 10.5, family = "$kitData", color = "$kitMuted" }) }
    tick = tick + step
  end

  local lanes: kui.Props = {
    key = "lanes",
    width = "grow",
    gap = 2,
    on_drag = { kit = "pan", id = c.id },
    on_scroll = { kit = "zoom", id = c.id },
  }
  for k, lane in ipairs(st.index.lanes) do
    lanes[#lanes + 1] = lane_row(lane, k, st, c, width)
  end

  local head: kui.Props = {
    width = "grow",
    gap = 8,
    cross_align = "center",
    text(skin.label(c.skin, "Trace"), { size = 11, bold = true, family = "$kitUi", color = "$kitFg" }),
    text(string.format("%s – %s · %d bars", fmt(st.v0, step), fmt(st.v1, step), st.index.count),
      { size = 11, family = "$kitData", color = "$kitMuted" }),
    row { width = "grow" },
    button.view({ label = "−" }, c, 1),
    button.view({ label = "+" }, c, 2),
    button.view({ label = "Fit" }, c, 3),
  }

  return column {
    width = "grow",
    gap = 4,
    pad = 10,
    bg = "$kitPanel",
    row(head),
    row(axis),
    column { width = "grow", height = "$kitRuleW", bg = "$kitInk" },
    column(lanes),
  }
end

local function zoom(st: State, factor: number, about: number)
  local span = (st.v1 - st.v0) * factor
  span = math.max(span, 0.01)
  local f = (about - st.v0) / (st.v1 - st.v0)
  st.v0 = about - span * f
  st.v1 = st.v0 + span
end

function M.event(ev: kui.Event, p: Params, id: string): {string:any}
  local st = states[id]
  if not st then return nil end
  local m = ev as {string:any}
  if m.kit == "pan" then
    local d = m.drag as {string:number}
    local px = (st.v1 - st.v0) / math.max(st.w, 1)
    st.v0 = st.v0 - d.dx * px
    st.v1 = st.v1 - d.dx * px
  elseif m.kit == "zoom" then
    local s = m.scroll as {string:number}
    local about = st.v0 + (s.x - st.x) / math.max(st.w, 1) * (st.v1 - st.v0)
    zoom(st, math.exp(-s.dy * 0.005), about)
  elseif m.kit == "press" then
    local i = m.index as integer
    local mid = (st.v0 + st.v1) / 2
    if i == 1 then zoom(st, 2, mid)
    elseif i == 2 then zoom(st, 0.5, mid)
    else st.v0, st.v1 = st.index.lo, st.index.hi end
  elseif m.kit == "pick" then
    local lane = st.index.lanes[m.lane as integer]
    local b = lane and lane.bars[m.index as integer]
    if b then
      st.sel_lane, st.sel_bar = m.lane as integer, m.index as integer
      return ctx.emit(p.on_select, { target = b.t, name = b.n, start = b.s, len = b.l })
    end
  end
  return nil
end

return M
