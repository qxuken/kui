-- Trace waterfall: spans as bars on lanes by depth over a time axis,
-- zoomed by the wheel about the pointer and panned by a drag, a click
-- selecting a span.
--
-- Input is a channel, source-agnostic: each row a span's `start` and `len`
-- (ms), its `depth` (its lane), `group` (its colour), `name` and `target`.
-- The window is the host's when it passes `start`/`length` (and keeps what
-- `zoom` proposes), Kit's own otherwise.
--
-- Cost is bounded by the width, not the span count: spans narrower than a
-- pixel merge into one density strip per run, read from levels built once
-- per channel revision (each level merges at half the last one's
-- threshold), so a zoomed-out recording of 18,000 spans draws a strip per
-- lane rather than walking every span.

local ctx = require("ctx")
local data = require("data")
local icons = require("icons")

local type Ctx = ctx.Ctx
local type P = {string:any}
local ref, act = ctx.ref, ctx.act

local record Bar
  s: number
  l: number
  d: integer
  g: integer
  -- The span's row in the channel.
  row: integer
  n: string
end

local record Mark
  at: number
  label: string
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
  key: string
  lanes: {Lane}
  marks: {Mark}
  lo: number
  hi: number
  count: integer
end

local record State
  index: Index
  -- Kit's own window, while the host keeps none.
  v0: number
  v1: number
  -- The lanes' rect last frame, for zoom about the pointer.
  x: number
  w: number
end

local M = {}

local LANE_H = 20
local MIN_BAR = 2
-- Levels merge at extent / 2^k for k in COARSEST..FINEST; finer than the
-- last, the raw bars are drawn.
local COARSEST, FINEST = 4, 22

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

local function num(v: any): number
  return v is number and v or (tonumber(v) or 0)
end

local function build(bars: kui.Channel, marks: kui.Channel, key: string): Index
  local lanes: {Lane} = {}
  local lo, hi = math.huge, -math.huge
  local n = bars and #bars or 0
  for r = 0, n - 1 do
    local row_t = bars:row(r)
    local b: Bar = {
      s = num(row_t.start), l = num(row_t.len), d = math.floor(num(row_t.depth)),
      g = math.floor(num(row_t.group)) % 6 + 1, row = r, n = tostring(row_t.name or ""),
    }
    local k = b.d + 1
    if k < 1 then k = 1 end
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
  local ms: {Mark} = {}
  for r = 0, (marks and #marks or 0) - 1 do
    ms[#ms + 1] = { at = num(marks:get(r, "at")), label = data.cell_text(marks:get(r, "label")) }
  end
  return { key = key, lanes = lanes, marks = ms, lo = lo, hi = hi, count = n }
end

local function key_of(ch: kui.Channel): string
  if not ch then return "-" end
  return ch.id .. "@" .. ch.rev
end

local function state(c: Ctx, p: P): State
  local st = ctx.state(c, function(): State return { v0 = 0, v1 = 1, x = 0, w = 0 } end)
  local bars, marks = p.bars as kui.Channel, p.marks as kui.Channel
  local key = key_of(bars) .. "/" .. key_of(marks)
  if not st.index or st.index.key ~= key then
    local fresh = not st.index or st.index.count == 0
    st.index = build(bars, marks, key)
    if fresh then st.v0, st.v1 = st.index.lo, st.index.hi end
  end
  return st
end

-- The window in force: the host's when it keeps one.
local function window(st: State, p: P): number, number
  if p.start ~= nil or p.length ~= nil then
    local v0 = (p.start or st.index.lo) as number
    local len = (p.length or (st.index.hi - st.index.lo)) as number
    return v0, v0 + math.max(len, 1e-6)
  end
  return st.v0, st.v1
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

local function lane_row(c: Ctx, lane: Lane, st: State, v0: number, v1: number, width: number, selected: integer, picks: boolean): kui.Node
  local scale = width / (v1 - v0)
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
  local i = first_at(ss, ee, v0)
  while i <= #ss and ss[i] <= v1 do
    local x0 = math.max(0, (ss[i] - v0) * scale)
    local x1 = math.min(width, (ee[i] - v0) * scale)
    local w = x1 - x0
    if w > 0 then
      if x0 > cursor then
        t[#t + 1] = column { width = x0 - cursor }
      end
      local one = lv and lv.one[i] or i
      if one ~= 0 and w >= MIN_BAR then
        local b = lane.bars[one]
        local sel = selected == b.row
        local bar: kui.Props = {
          -- Named whether or not its text fits: a narrow bar is still a control.
          label = b.n,
          width = w,
          height = LANE_H - 4,
          bg = sel and ref(c, "sel") or ("$kitGroup" .. b.g),
          clip = true,
          pad = { x = 4 },
          cross_align = "center",
          -- One of the lanes' list (ADR 0007) when the host picks spans:
          -- the arrows walk the bars, Enter picks one. Every bar was a Tab
          -- stop of its own, and a stop that did nothing where no one
          -- picked.
          key = picks and ("span" .. b.row) or nil,
          role = picks and "listItem" or nil,
          focusable = picks or nil,
          on_click = picks and act(c, "waterfall.pick", { row = b.row }) or nil,
        }
        -- Each group's fill reads at 3:1 on the panel and its label on it
        -- at 4.5:1 (`$kitGroupFg<n>`, skin.tl). The selected bar is the
        -- selection's fill in a heavier edge in the text colour, so it stands out by its
        -- outline as well as its colour while the rest stay as they are.
        if sel then
          bar.border = { w = 2, color = ref(c, "fg") }
        elseif w >= 4 then
          bar.border = { w = 1, color = ref(c, "panel") }
        end
        if w >= 48 then
          bar[#bar + 1] = text(b.n, { size = 11, max_lines = 1, ellipsis = true, family = "$kitData",
            color = sel and ref(c, "sel_fg") or ("$kitGroupFg" .. b.g) })
        end
        t[#t + 1] = row(bar)
      else
        t[#t + 1] = column { width = math.max(w, 1), height = LANE_H - 10, bg = ref(c, "faint") }
      end
      cursor = x0 + math.max(w, 1)
    end
    i = i + 1
  end
  return row(t)
end

local function tool(c: Ctx, icon: string, label: string, action: string, v0: number, v1: number): kui.Node
  local t: kui.Props = {
    role = "button", label = label, tooltip = label, width = 22, height = 22,
    main_align = "center", cross_align = "center", radius = "$kitRadius",
    hover_bg = ref(c, "hover"), cursor = "pointer",
    on_click = act(c, "waterfall." .. action, { v0 = v0, v1 = v1 }),
    icons.node(c, icon, 13, "fg"),
  }
  return row(t)
end

function M.waterfall(c: Ctx, p: P): kui.Node
  local st = state(c, p)
  local env = c.env
  local r = env.layout_of("lanes")
  if r then st.x, st.w = r.x, r.w end
  local width = st.w > 0 and st.w or math.max(100, env.viewport_w - 40)
  local v0, v1 = window(st, p)
  local scale = width / (v1 - v0)
  local selected = p.selected as integer

  -- The axis: a tick about every 100 px at a round step.
  local step = nice(100 / scale)
  local axis: kui.Props = { height = 18, width = "grow", clip = true, cross_align = "end" }
  local t0 = math.ceil(v0 / step) * step
  local lead = (t0 - v0) * scale
  if lead > 0 then axis[#axis + 1] = column { width = lead } end
  local tick = t0
  while tick <= v1 do
    axis[#axis + 1] = column { width = step * scale,
      ctx.data(c, fmt(tick, step), { size = 10.5, color = ref(c, "muted") }) }
    tick = tick + step
  end

  local picks = p.on_select ~= nil
  local lanes: kui.Props = {
    key = "lanes",
    role = picks and "list" or nil,
    label = picks and "spans" or nil,
    width = "grow",
    height = p.grow == true and "grow" or nil,
    scroll_y = p.grow == true,
    gap = 2,
    on_drag = act(c, "waterfall.pan", { v0 = v0, v1 = v1, w = width }),
    -- Kept for `layout_of` (the width the window maps onto), and a resize
    -- redraws.
    on_layout = act(c, "waterfall.layout"),
    on_scroll = act(c, "waterfall.wheel", { v0 = v0, v1 = v1, x = st.x, w = width }),
  }
  for _, lane in ipairs(st.index.lanes) do
    lanes[#lanes + 1] = lane_row(c, lane, st, v0, v1, width, selected, picks)
  end
  -- Marks: a rule down the lanes and a label at the top.
  for i, m in ipairs(st.index.marks) do
    if m.at >= v0 and m.at <= v1 then
      local x = (m.at - v0) * scale
      lanes[#lanes + 1] = column {
        key = "m" .. i,
        float = { anchor = "parent", dx = x, dy = 0 },
        -- The accent as text reads at 4.5:1 on the panel; the accent itself
        -- may not reach the 3:1 a mark needs (Ledger's yellow on paper).
        width = 1, height = #st.index.lanes * (LANE_H + 2), bg = ref(c, "accent_text"),
        tooltip = m.label,
      }
    end
  end

  return column {
    width = "grow",
    height = p.grow == true and "grow" or nil,
    gap = 4,
    pad = 10,
    bg = ref(c, "panel"),
    row {
      width = "grow", gap = 8, cross_align = "center",
      ctx.caption(c, "Trace"),
      ctx.data(c, string.format("%s – %s · %d spans", fmt(v0, step), fmt(v1, step), st.index.count),
        { size = "$kitSizeLabel", color = ref(c, "muted") }),
      ctx.grow(),
      tool(c, "zoom_out", "Zoom out", "out", v0, v1),
      tool(c, "zoom_in", "Zoom in", "in", v0, v1),
      tool(c, "fit", "Fit", "fit", v0, v1),
    },
    row(axis),
    ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", c.s.skin.ctl_border and "$kitRuleW" or "$kitHair"),
    column(lanes),
  }
end

-- The window `span` long about `about`, keeping `about`'s place in it.
local function zoomed(v0: number, v1: number, factor: number, about: number): number, number
  local span = math.max((v1 - v0) * factor, 0.01)
  local f = (about - v0) / (v1 - v0)
  local s = about - span * f
  return s, s + span
end

function M.event(m: P, kit: string): any
  local st = ctx.peek(m.inst as integer) as State
  if not st then return nil end
  local v0, v1: number, number
  if kit == "waterfall.pick" then
    return emit("on_select", { row = m.row })
  elseif kit == "waterfall.pan" then
    local d = m.drag as P
    local px = ((m.v1 as number) - (m.v0 as number)) / math.max(m.w as number, 1)
    -- dx is from the press: the window the drag began on, moved.
    v0 = (m.v0 as number) - (d.dx as number) * px
    v1 = (m.v1 as number) - (d.dx as number) * px
  elseif kit == "waterfall.wheel" then
    local s = m.scroll as P
    local a, b = m.v0 as number, m.v1 as number
    local about = a + ((s.x as number) - (m.x as number)) / math.max(m.w as number, 1) * (b - a)
    v0, v1 = zoomed(a, b, math.exp(-(s.dy as number) * 0.005), about)
  elseif kit == "waterfall.in" or kit == "waterfall.out" then
    local a, b = m.v0 as number, m.v1 as number
    v0, v1 = zoomed(a, b, kit == "waterfall.in" and 0.5 or 2, (a + b) / 2)
  elseif kit == "waterfall.fit" then
    v0, v1 = st.index.lo, st.index.hi
  else
    return nil
  end
  st.v0, st.v1 = v0, v1
  return emit("on_zoom", { start = v0, length = v1 - v0 })
end

return M
