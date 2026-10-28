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
-- pixel merge into one density strip per run, read from the channel's
-- lanes (`bars:lanes`, ADR 0075) — built once per channel revision in
-- Rust, each lane's levels made as a window first asks for them, each
-- level merging at half the last one's threshold — so a zoomed-out
-- recording of a million spans draws a strip per lane, and a frame reads
-- the runs in view and the names of the bars it draws, never every span.
-- Marks merge the same way: one rule a pixel, its tooltip saying how many.

local ctx = require("ctx")
local data = require("data")
local icons = require("icons")

local type Ctx = ctx.Ctx
local type P = {string:any}
local ref, act = ctx.ref, ctx.act

local record Index
  key: string
  bars: kui.Channel
  marks: kui.Channel
  lanes: kui.ChannelLanes
  ticks: kui.ChannelLanes
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
local function build(bars: kui.Channel, marks: kui.Channel, key: string): Index
  local lanes = bars and bars:lanes("start", "len", "depth") or nil
  local ticks = (marks and #marks > 0) and marks:lanes("at") or nil
  local lo, hi = 0.0, 1.0
  if lanes and lanes.len > 0 then lo, hi = lanes.lo, lanes.hi end
  return { key = key, bars = bars, marks = marks, lanes = lanes, ticks = ticks,
    lo = lo, hi = hi, count = lanes and lanes.len or 0 }
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

local function num(v: any): number
  return v is number and v or (tonumber(v) or 0)
end

local function lane_row(c: Ctx, lane: integer, st: State, v0: number, v1: number, width: number, selected: integer, picks: boolean): kui.Node
  local scale = width / (v1 - v0)
  local px = 1 / scale
  local t: kui.Props = { height = LANE_H, width = "grow", cross_align = "center", clip = true }
  local runs = st.index.lanes:window(lane, v0, v1, px * MIN_BAR)
  local bars = st.index.bars
  local cursor = 0.0
  for i = 1, #runs.s do
    local x0 = math.max(0, (runs.s[i] - v0) * scale)
    local x1 = math.min(width, (runs.e[i] - v0) * scale)
    local w = x1 - x0
    if w > 0 then
      if x0 > cursor then
        t[#t + 1] = column { width = x0 - cursor }
      end
      local r = runs.row[i]
      if r >= 0 and w >= MIN_BAR then
        local name = data.cell_text(bars:get(r, "name"))
        local g = math.floor(num(bars:get(r, "group"))) % 6 + 1
        local sel = selected == r
        local bar: kui.Props = {
          -- Named whether or not its text fits: a narrow bar is still a control.
          label = name,
          width = w,
          height = LANE_H - 4,
          bg = sel and ref(c, "sel") or ("$kitGroup" .. g),
          clip = true,
          pad = { x = 4 },
          cross_align = "center",
          -- One of the lanes' list (ADR 0007) when the host picks spans:
          -- the arrows walk the bars, Enter picks one. Every bar was a Tab
          -- stop of its own, and a stop that did nothing where no one
          -- picked.
          key = picks and ("span" .. r) or nil,
          role = picks and "listItem" or nil,
          focusable = picks or nil,
          on_click = picks and act(c, "waterfall.pick", { row = r }) or nil,
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
          bar[#bar + 1] = text(name, { size = 11, max_lines = 1, ellipsis = true, family = "$kitData",
            color = sel and ref(c, "sel_fg") or ("$kitGroupFg" .. g) })
        end
        t[#t + 1] = row(bar)
      else
        t[#t + 1] = column { width = math.max(w, 1), height = LANE_H - 10, bg = ref(c, "faint") }
      end
      cursor = x0 + math.max(w, 1)
    end
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
  local n_lanes = st.index.lanes and #st.index.lanes or 0
  for lane = 0, n_lanes - 1 do
    lanes[#lanes + 1] = lane_row(c, lane, st, v0, v1, width, selected, picks)
  end
  -- Marks: a rule down the lanes, those a pixel apart one rule saying how
  -- many it stands for.
  if st.index.ticks then
    local runs = st.index.ticks:window(0, v0, v1, 1 / scale)
    for i = 1, #runs.s do
      local x = (runs.s[i] - v0) * scale
      local r = runs.row[i]
      local label = r >= 0 and data.cell_text(st.index.marks:get(r, "label"))
        or string.format("%d marks", runs.n[i])
      lanes[#lanes + 1] = column {
        key = "m" .. math.floor(runs.s[i] * 1000),
        float = { anchor = "parent", dx = x, dy = 0 },
        -- The accent as text reads at 4.5:1 on the panel; the accent itself
        -- may not reach the 3:1 a mark needs (Ledger's yellow on paper).
        width = 1, height = n_lanes * (LANE_H + 2), bg = ref(c, "accent_text"),
        tooltip = label,
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
