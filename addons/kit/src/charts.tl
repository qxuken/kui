-- Charts over a channel. A column is read once per channel revision and
-- kept, then cut into at most two points a pixel (each bucket's least and
-- greatest), so a line's cost is its width however many rows it has, and
-- an unchanged revision at an unchanged size replays without asking.

local ctx = require("ctx")
local data = require("data")
local skin = require("skin")

local type Ctx = ctx.Ctx
local type P = {string:any}
local ref, act = ctx.ref, ctx.act

local M = {}

local function str(v: any): string
  return v is string and v or nil
end

-- Columns as Lua lists, kept by channel id, revision and name.
local record Held
  id: integer
  rev: integer
  cols: {string:{number}}
end
local held: {integer:Held} = {}
local held_n = 0

local function col(ch: kui.Channel, name: string): {number}
  if ch == nil or name == nil then return {} end
  local h = held[ch.id]
  if not h or h.rev ~= ch.rev then
    if not h then
      -- Channels come and go: past a few dozen, start again.
      held_n = held_n + 1
      if held_n > 64 then held, held_n = {}, 1 end
    end
    h = { id = ch.id, rev = ch.rev, cols = {} }
    held[ch.id] = h
  end
  local c = h.cols[name]
  if not c then
    local raw = ch:column(name)
    c = {}
    for i = 1, #raw do
      local v = raw[i]
      c[i] = v is number and v or (tonumber(v) or 0)
    end
    h.cols[name] = c
  end
  return c
end

local function nice(raw: number): number
  if raw <= 0 then return 1 end
  local p = 10 ^ math.floor(math.log(raw, 10))
  local steps: {number} = { 1, 2, 2.5, 5, 10 }
  for _, m in ipairs(steps) do
    if m * p >= raw then return m * p end
  end
  return 10 * p
end

local function short(v: number): string
  local a = math.abs(v)
  if a >= 1e9 then return string.format("%.3gG", v / 1e9) end
  if a >= 1e6 then return string.format("%.3gM", v / 1e6) end
  if a >= 1e4 then return string.format("%.3gk", v / 1e3) end
  if a > 0 and a < 1e-9 then return "0" end
  return string.format("%.4g", v)
end

-- A value with the host's unit after it: `12 ms`, `40%`.
local function with_unit(s: string, unit: string): string
  if not unit or unit == "" then return s end
  if unit:match("^[%%°]") then return s .. unit end
  return s .. " " .. unit
end

-- Steps a clock axis ticks at, in seconds: round minutes and hours.
local CLOCK_STEPS: {number} = { 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 21600, 43200, 86400 }

-- The nice step after `step` (1, 2, 2.5, 5, 10 a decade), or before it.
local NICE: {number} = { 1, 2, 2.5, 5, 10 }
local function nicer(step: number, down: boolean): number
  local p = 10 ^ math.floor(math.log(step, 10) + 1e-9)
  local m = step / p
  if down then
    for i = #NICE, 1, -1 do
      if NICE[i] < m - 1e-6 then return NICE[i] * p end
    end
    return p / 2
  end
  for _, u in ipairs(NICE) do
    if u > m + 1e-6 then return u * p end
  end
  return 20 * p
end

-- The clock step after `step`, or before it.
local function clock_step(step: number, down: boolean): number
  if down then
    for i = #CLOCK_STEPS, 1, -1 do
      if CLOCK_STEPS[i] < step then return CLOCK_STEPS[i] end
    end
    return CLOCK_STEPS[1]
  end
  for _, st in ipairs(CLOCK_STEPS) do
    if st > step then return st end
  end
  return step * 2
end

-- A tick's number for ticks `step` apart: as many decimals as the step
-- has and no more — ticks half a unit apart read `12.5`, not `12` twice —
-- in k, M or G by the axis's largest value, `top`.
local function tick_number(v: number, step: number, top: number): string
  if math.abs(v) < step * 1e-6 then v = 0 end
  local scale, suffix = 1.0, ""
  local a = math.abs(top)
  if a >= 1e9 then scale, suffix = 1e9, "G"
  elseif a >= 1e6 then scale, suffix = 1e6, "M"
  elseif a >= 1e4 then scale, suffix = 1e3, "k" end
  local s = step / scale
  local d = 0
  while d < 6 do
    local x = s * 10 ^ d
    if math.abs(x - math.floor(x + 0.5)) < 1e-6 * math.max(1, x) then break end
    d = d + 1
  end
  return string.format("%." .. d .. "f", v / scale) .. suffix
end

-- Label widths in a chart's label face, kept by text under the settings
-- in force — replaced whole when the skin moves, so its face is part of
-- the key: a live axis asks for the same few labels on every tick.
local LABEL: kui.Props = { size = 10, family = "$kitData" }
local widths: {skin.Settings:{string:number}} = setmetatable({}, { __mode = "k" })
local widths_n: {skin.Settings:integer} = setmetatable({}, { __mode = "k" })

local function label_w(c: Ctx, s: string): number
  local memo = widths[c.s]
  if not memo or widths_n[c.s] > 512 then
    memo = {}
    widths[c.s], widths_n[c.s] = memo, 0
  end
  local w = memo[s]
  if not w then
    local m = c.env.measure_text(s, LABEL, nil)
    w = m and math.ceil(m.width) or #s * 6
    memo[s] = w
    widths_n[c.s] = widths_n[c.s] + 1
  end
  return w
end

-- The ticks `step` apart across [x0, x1] and each one's label, as `fmt`
-- says:
--   "number" — the value, with `unit`;
--   "ago"    — x counts back to now (`-15` … `0`): `15 min ago` … `now`;
--   "clock"  — x is Unix seconds: the local time, to the second when the
--              ticks are under a minute apart.
local function tick_labels(x0: number, x1: number, step: number, fmt: string, unit: string): {number}, {string}
  local at: {number} = {}
  local labels: {string} = {}
  local top = math.max(math.abs(x0), math.abs(x1))
  local v = math.ceil(x0 / step - 1e-9) * step
  while v <= x1 + step * 1e-6 do
    local s: string
    if fmt == "clock" then
      s = os.date(step < 60 and "%H:%M:%S" or "%H:%M", math.floor(v + 0.5)) as string
    elseif fmt == "ago" then
      local back = math.abs(v)
      s = back < step * 1e-6 and "now" or (with_unit(tick_number(back, step, top), unit) .. " ago")
    else
      s = with_unit(tick_number(v, step, top), unit)
    end
    at[#at + 1], labels[#labels + 1] = v, s
    v = v + step
  end
  return at, labels
end

-- The least room between two x labels, and between two ticks, in px: a
-- tick is a place to read off, not a ruler's mark (thirty two-digit
-- labels across a wide plot fit, and cost a node and a line each).
local X_GAP = 12
local X_TICK = 50

-- The ticks across [x0, x1] in a plot `w` wide as close together as their
-- labels allow — each label measured, and two neighbours' ticks at least
-- their half widths and a gap apart — with each label and its width. The
-- step was a guess (a tick every 90 px, each label 80 px wide), which
-- dropped or squeezed labels long before they would have touched.
local function x_ticks(c: Ctx, x0: number, x1: number, w: number, fmt: string, unit: string): {number}, {string}, {number}
  local span = x1 - x0
  local clock = fmt == "clock"
  -- A first guess from the last label at a loose step, one step finer,
  -- so the search starts under the answer and climbs to it.
  local loose = span / math.max(2, math.floor(w / 90))
  local guess_step = clock and clock_step(loose, true) or nice(loose)
  local _, guess = tick_labels(x1 - guess_step, x1, guess_step, fmt, unit)
  local est = guess[#guess] and label_w(c, guess[#guess]) or 40
  local raw = span * math.max(est + X_GAP, X_TICK) / math.max(w, 1)
  local step: number
  if clock then
    step = CLOCK_STEPS[#CLOCK_STEPS]
    for _, st in ipairs(CLOCK_STEPS) do
      if st >= raw then step = st; break end
    end
    step = clock_step(step, true)
  else
    step = nicer(nice(raw), true)
  end
  local at, labels: {number}, {string} = {}, {}
  local lw: {number} = {}
  for _ = 1, 24 do
    local px = step / span * w
    local fits = px >= X_TICK
    if fits then
      at, labels = tick_labels(x0, x1, step, fmt, unit)
      lw = {}
      for i, s in ipairs(labels) do
        lw[i] = label_w(c, s)
        if i > 1 and px < (lw[i - 1] + lw[i]) / 2 + X_GAP then fits = false; break end
      end
      if fits then break end
    end
    step = clock and clock_step(step, false) or nicer(step, false)
  end
  if #labels == 0 then at, labels = tick_labels(x0, x1, step, fmt, unit) end
  for i = #lw + 1, #labels do lw[i] = label_w(c, labels[i]) end
  return at, labels, lw
end

-- The points of ys[from..to] against xs scaled into w×h, at most two a
-- pixel: each pixel bucket's least and greatest, in order.
-- One loop, no closure, no call and no arithmetic on x a row: it runs on
-- every tick of a live chart, once per series, over every row in view.
-- A bucket is two px; a row is in the current one while its raw x is
-- under the bucket's raw right edge, and the bucket keeps the rows of its
-- least and greatest y, placed when it is left.
local function points(xs: {number}, ys: {number}, from: integer, to: integer, x0: number, x1: number,
                      y0: number, y1: number, w: number, h: number): {{number}}
  local out: {{number}} = {}
  if to < from then return out end
  local sx = w / math.max(x1 - x0, 1e-12)
  local sy = h / math.max(y1 - y0, 1e-12)
  -- A value's y on screen is base - v * sy.
  local base = h + y0 * sy
  local step = 2 / sx
  local n = 0
  local i = from
  while i <= to do
    -- A new bucket at row i: the one its x falls in.
    local x = xs[i]
    local edge = x0 + ((x - x0) // step + 1) * step
    local y = ys[i] or 0
    local lo, hi, lo_i, hi_i = y, y, i, i
    i = i + 1
    while i <= to do
      local xi = xs[i]
      if xi >= edge then break end
      local yi = ys[i] or 0
      if yi < lo then
        lo, lo_i = yi, i
      elseif yi > hi then
        hi, hi_i = yi, i
      end
      i = i + 1
    end
    -- The bucket left: its least and greatest, in x order.
    local a_i, a_y, b_i, b_y = lo_i, lo, hi_i, hi
    if lo_i > hi_i then a_i, a_y, b_i, b_y = hi_i, hi, lo_i, lo end
    n = n + 1
    out[n] = { (xs[a_i] - x0) * sx, base - a_y * sy }
    if b_i ~= a_i then
      n = n + 1
      out[n] = { (xs[b_i] - x0) * sx, base - b_y * sy }
    end
  end
  return out
end

-- The area under a line through `pts` down to `h`, as polygons: a
-- polygon takes eight points, so each is six of the line's over the
-- baseline, the next starting where it ended. A path through them all is
-- rasterized again on every tick of a live chart, whole: with C96's
-- linear raster still about 0.6 ms a frame more for 300 points and 7 ms
-- for 100,000 rows (two points a pixel), where polygons cost a quad each.
--
-- Two polygons meeting along an edge each ramp it over a pixel, so a
-- pixel on it was covered three quarters, not whole: a faint seam at
-- every chunk's end, sliding as a live window moved. The point a chunk
-- ends on is moved to the nearest whole *device* pixel of the window,
-- where the ramp gives each pixel to one side whole — under half a pixel,
-- beneath the line's own width. `ox` is the plot's left edge in the
-- window and `scale` the display's (`env.scale`, C127): a whole logical
-- pixel is a whole device one at 1 and 2 only, and at 1.5 the seams were
-- fainter, not gone. A point with a neighbour within half a pixel stays
-- where it is.
local function area(plot: kui.Props, pts: {{number}}, h: number, bg: kui.Color, opacity: number, ox: number, scale: number)
  local n = #pts
  local start = pts[1]
  local i = 1
  while i < n do
    local j = math.min(i + 5, n)
    local stop = pts[j]
    if j < n then
      local x = math.floor((stop[1] + ox) * scale + 0.5) / scale - ox
      local prev = j - 1 == i and start or pts[j - 1]
      if x > prev[1] and x < pts[j + 1][1] then stop = { x, stop[2] } end
    end
    local poly: {{number}} = { start }
    for k = i + 1, j - 1 do poly[k - i + 1] = pts[k] end
    poly[#poly + 1] = stop
    poly[#poly + 1] = { stop[1], h }
    poly[#poly + 1] = { start[1], h }
    plot[#plot + 1] = polygon { points = poly, bg = bg, opacity = opacity }
    start, i = stop, j
  end
end

local function range_of(ys: {number}, from: integer, to: integer): number, number
  local lo, hi = math.huge, -math.huge
  for i = from, to do
    local y = ys[i]
    if y then
      if y < lo then lo = y end
      if y > hi then hi = y end
    end
  end
  if lo > hi then return 0, 1 end
  if lo == hi then return lo - 1, hi + 1 end
  return lo, hi
end

-- A chart's state between frames: a line chart's value range on its way to
-- a new one (where it set off from, where it is going, when it left), and
-- the readout — whether the pointer is over the plot, the row under it,
-- the row the keyboard put it on, and a Tab to hand on to the ring.
local record Glide
  lo: number
  hi: number
  from_lo: number
  from_hi: number
  t0: number
  over: boolean
  row: integer
  key_row: integer
  -- The slide (a live window's): the newest x the last frame drew, how
  -- far x moved when a sample last landed and when, the pace samples come
  -- at (s), and the slide's count, which keys its keyframe cycle.
  x_last: number
  step_x: number
  t_landed: number
  every: number
  gen: integer
end

function M.sparkline(c: Ctx, p: P): kui.Node
  local w, h = (p.width or 80) as number, (p.height or 18) as number
  local ys = col(p.data as kui.Channel, p.field as string)
  local n = #ys
  local box: kui.Props = { width = w, height = h, label = str(p.field), role = "image" }
  if n >= 2 then
    local lo, hi = range_of(ys, 1, n)
    local idx: {number} = {}
    for i = 1, n do idx[i] = i end
    local color = (p.color as kui.Color) or ref(c, "accent")
    box[#box + 1] = line {
      points = points(idx, ys, 1, n, 1, n, lo, hi, w, h - 2),
      width = 1.5,
      color = color,
    }
    -- The readout, as the line chart's: a hover layer while the pointer
    -- is off; bands two px wide once it is on, each a sample's value as
    -- its hint, and a dot on the sample under the pointer.
    if p.readout ~= false then
      local st = ctx.state(c, function(): Glide return {} end)
      local layer: kui.Props = { key = "over", width = w, height = h, hoverable = true,
        float = { anchor = "parent", clip = true }, on_hover = act(c, "sparkline.over") }
      if st.over then
        -- Each band names the sample nearest its middle where the line
        -- draws it (sample i at (i - 1) / (n - 1) of the width), bands
        -- naming the same sample one band.
        local bands = math.max(1, math.floor(w / 2))
        local bw = w / bands
        local run_i, run_from = 0, 0
        local function flush(b: integer)
          if run_i > 0 then
            layer[#layer + 1] = column { key = "b" .. run_from, width = (b - run_from) * bw, height = h, hoverable = true,
              tooltip = with_unit(string.format("%.4g", ys[run_i]), str(p.unit)),
              on_hover = act(c, "sparkline.hover", { row = run_i }) }
          end
        end
        for b = 0, bands - 1 do
          local i = 1 + math.floor((b + 0.5) / bands * (n - 1) + 0.5)
          if i ~= run_i then
            flush(b)
            run_i, run_from = i, b
          end
        end
        flush(bands)
        local i = st.row
        if i and ys[i] then
          local px = (i - 1) / math.max(n - 1, 1) * w
          local py = (h - 2) - (ys[i] - lo) / math.max(hi - lo, 1e-12) * (h - 2)
          box[#box + 1] = column { float = { anchor = "parent", dx = px - 3, dy = py - 3, clip = true },
            width = 6, height = 6, radius = 3, bg = color, border = { w = 1, color = ref(c, "panel") } }
        end
      end
      box[#box + 1] = row(layer)
    end
  end
  return column(box)
end

-- A number a channel cell holds, nil for none.
local function num(v: any): number
  if v is number then return v end
  return tonumber(v)
end

-- The first row in [lo, hi) whose x is at least `t`, x sorted ascending,
-- by `ch:get`: the x column never crosses into Lua whole.
local function lower_row(ch: kui.Channel, field: string, lo: integer, hi: integer, t: number): integer
  while lo < hi do
    local mid = (lo + hi) // 2
    local x = num(ch:get(mid, field)) or 0
    if x < t then lo = mid + 1 else hi = mid end
  end
  return lo
end

-- Series `field` over rows [from, to) as lines in a w×h plot, from the
-- channel's own reduction (C95): a bucket of rows per two px, and in each
-- the first, the least and the greatest in row order, and the last — the
-- line every row would draw. x for a row is placed along its bucket from
-- the x reduction's first and last (the buckets are equal row counts, not
-- equal x spans, so rows evenly spaced in x land where they would). A
-- value that is not a number (NaN) breaks the line: the result is a list
-- of runs, each two points or more.
-- A stretch of rows reduced in buckets of equal row counts, and its x.
local record Seg
  from: integer
  to: integer
  buckets: integer
  rx: kui.ChannelReduction
end

-- The stretches rows [from, to) are reduced in, a bucket per two px of a
-- plot `w` wide: buckets of `k` rows whose edges sit on whole multiples
-- of `k` — the first reaching back before `from`, drawn left of the plot
-- and cut by its clip — and the rows past the last full one as a bucket of
-- their own. Buckets counted from `from` moved with every row a live
-- window slid by, so each tick bucketed the line afresh and its spikes
-- came and went; on whole multiples, a slide by a row moves nothing but
-- the ends.
local function segments(ch: kui.Channel, xf: string, from: integer, to: integer, w: number): {Seg}
  local segs: {Seg} = {}
  if to <= from then return segs end
  local buckets = math.max(1, math.floor(w / 2))
  local k = math.max(1, (to - from + buckets - 1) // buckets)
  local start = (from // k) * k
  local full = start + ((to - start) // k) * k
  if full > start then
    local nb = (full - start) // k
    segs[#segs + 1] = { from = start, to = full, buckets = nb, rx = ch:reduce(xf, start, full, nb) }
  end
  if to > full then
    segs[#segs + 1] = { from = full, to = to, buckets = 1, rx = ch:reduce(xf, full, to, 1) }
  end
  return segs
end

local function reduced_runs(ch: kui.Channel, segs: {Seg}, field: string, x0: number, x1: number,
                            y0: number, y1: number, w: number, h: number): {{{number}}}
  local runs: {{{number}}} = {}
  local sx = w / math.max(x1 - x0, 1e-12)
  local sy = h / math.max(y1 - y0, 1e-12)
  local base = h + y0 * sy
  local run: {{number}} = {}
  local n = 0
  for _, seg in ipairs(segs) do
  local rx = seg.rx
  local ry = ch:reduce(field, seg.from, seg.to, seg.buckets)
  -- One loop, no closure, table or sort a bucket: it runs on every tick
  -- of a live chart, a bucket per two px (or a row, for fewer rows).
  local xfs, xls, xfr, xlr = rx.first, rx.last, rx.first_row, rx.last_row
  local yfr, ymnr, ymxr, ylr = ry.first_row, ry.min_row, ry.max_row, ry.last_row
  local yf, ymn, ymx, yl = ry.first, ry.min, ry.max, ry.last
  for i = 1, #yf do
    local fr, lr = xfr[i], xlr[i]
    local xa, xb = xfs[i], xls[i]
    local k = lr > fr and (xb - xa) / (lr - fr) or 0
    -- first, then the least and the greatest in row order, then last;
    -- a row met again (a bucket of one row) is not drawn twice.
    local r1, v1 = yfr[i], yf[i]
    local r2, v2, r3, v3 = ymnr[i], ymn[i], ymxr[i], ymx[i]
    if r3 < r2 then r2, v2, r3, v3 = r3, v3, r2, v2 end
    local r4, v4 = ylr[i], yl[i]
    for j = 1, 4 do
      local row, v: integer, number
      if j == 1 then row, v = r1, v1
      elseif j == 2 then row, v = r2, v2
      elseif j == 3 then row, v = r3, v3
      else row, v = r4, v4 end
      local again = (j > 1 and row == r1) or (j > 2 and row == r2) or (j > 3 and row == r3)
      if not again then
        if v ~= v then
          -- Not a number: the line breaks here.
          if n >= 2 then runs[#runs + 1] = run end
          run, n = {}, 0
        else
          n = n + 1
          run[n] = { (xa + (row - fr) * k - x0) * sx, base - v * sy }
        end
      end
    end
  end
  end
  if n >= 2 then runs[#runs + 1] = run end
  return runs
end

-- How long a range takes to glide to the next, in s.
local GLIDE_S = 0.24

-- The range to draw this frame on the way to [lo, hi]: the new one eased
-- in from wherever the last glide had got to, out of a cubic. A frame is
-- asked for while it moves, and the frame the chart is drawn in is not
-- replayed (`request_frame_at` keeps a fill from being kept). Under
-- reduced motion, and with no frame clock (a host that never sets one:
-- `now` stays 0), it jumps.
local function glide(c: Ctx, lo: number, hi: number): number, number
  local st = ctx.state(c, function(): Glide return {} end)
  local now = c.env.now or 0
  if not st.lo or now <= 0 or ctx.reduced(c) then
    st.lo, st.hi, st.from_lo, st.from_hi, st.t0 = lo, hi, lo, hi, -math.huge
    return lo, hi
  end
  local function at(k: number): number, number
    if k >= 1 then return st.lo, st.hi end
    local e = 1 - (1 - k) ^ 3
    return st.from_lo + (st.lo - st.from_lo) * e, st.from_hi + (st.hi - st.from_hi) * e
  end
  if lo ~= st.lo or hi ~= st.hi then
    st.from_lo, st.from_hi = at((now - st.t0) / GLIDE_S)
    st.lo, st.hi, st.t0 = lo, hi, now
  end
  local k = (now - st.t0) / GLIDE_S
  if k < 1 then c.env.request_frame_at(now) end
  return at(k)
end

-- The row of rows [lo, hi) whose x is nearest `t`.
local function nearest_row(ch: kui.Channel, xf: string, lo: integer, hi: integer, t: number): integer
  local r = lower_row(ch, xf, lo, hi, t)
  if r >= hi then return hi - 1 end
  if r > lo and t - (num(ch:get(r - 1, xf)) or t) < (num(ch:get(r, xf)) or t) - t then return r - 1 end
  return r
end

-- The readout's text for row `at`: the x, then each series' value.
local function said_of(ch: kui.Channel, xf: string, at: integer, series: {P}, fmt: string, x_unit: string,
                       y_unit: string): string, {string}
  local xv = num(ch:get(at, xf)) or 0
  local title: string
  if fmt == "clock" then
    title = os.date("%H:%M:%S", math.floor(xv + 0.5)) as string
  elseif fmt == "ago" then
    title = math.abs(xv) < 1e-9 and "now" or (with_unit(string.format("%.4g", math.abs(xv)), x_unit) .. " ago")
  else
    title = with_unit(string.format("%.4g", xv), x_unit)
  end
  local said: {string} = { title }
  for _, s in ipairs(series) do
    local v = num(ch:get(at, s.field as string))
    if v and v == v then
      said[#said + 1] = ((s.name or s.field) as string) .. " " .. with_unit(string.format("%.4g", v), y_unit)
    end
  end
  return title, said
end

function M.line_chart(c: Ctx, p: P): kui.Node
  local ch = p.data as kui.Channel
  local env = c.env
  local h = (p.height or 160) as number
  local series = (p.series or {}) as {P}
  local xf = p.x as string
  local n = ch and #ch or 0
  local r = env.layout_of("plot")
  local w = r and r.w or 400
  -- The x window: everything, or the last `window` of it, found by a
  -- binary search over the channel (rows [from, to), 0-based).
  local from, to = 0, n
  local x0 = n > 0 and (num(ch:get(0, xf)) or 0) or 0
  local x1 = n > 0 and (num(ch:get(n - 1, xf)) or 1) or 1
  if p.window and n > 0 then
    x0 = x1 - (p.window as number)
    from = lower_row(ch, xf, 0, n, x0)
  end
  if x1 <= x0 then x1 = x0 + 1 end
  local st = ctx.state(c, function(): Glide return {} end)
  -- The glide (a live window's): drawn one sample behind and slid left
  -- between samples, so it scrolls rather than steps. A sample landing
  -- starts a slide of the plot's content from one sample's width right
  -- of its place to its place, over the time samples have been coming
  -- in — a keyframe cycle of the core's, keyed by the sample, so the
  -- frames between two samples run no view and lay nothing out. The
  -- content the slide starts from is what the last one ended on: the
  -- window one sample back. Off under reduced motion, with no frame
  -- clock, while the plot is out of the viewport, and for a window that
  -- did not move (a paused feed, a fixed range).
  local now = env.now or 0
  local slide, slide_ms = 0.0, 0.0
  if p.window and n > 1 and now > 0 then
    if st.x_last and x1 > st.x_last then
      local gap = now - (st.t_landed or now)
      if st.t_landed and gap > 0 and gap < 5 then
        st.every = st.every and st.every * 0.7 + gap * 0.3 or gap
      end
      st.step_x, st.t_landed, st.gen = x1 - st.x_last, now, (st.gen or 0) + 1
    elseif st.x_last and x1 < st.x_last then
      st.step_x, st.every, st.t_landed = nil, nil, nil
    end
    st.x_last = x1
    local seen = r ~= nil and r.y + r.h > 0 and r.y < (env.viewport_h or 0) and r.x + r.w > 0
      and r.x < (env.viewport_w or 0)
    if st.step_x and st.every and seen and not ctx.reduced(c) then
      slide = st.step_x / (x1 - x0) * w
      -- A little under the measured pace: a sample early by a few ms
      -- would cut the slide short (a jump), where one late holds still.
      slide_ms = st.every * 1000 * 0.95
    end
  end
  -- What is drawn reaches a sample further back than the window while it
  -- slides: the slide uncovers it at the plot's left edge.
  local drawn_from = from
  if slide > 0 then drawn_from = lower_row(ch, xf, 0, n, x0 - st.step_x) end
  -- The value range: one bucket over the window per series.
  local lo, hi = math.huge, -math.huge
  for _, s in ipairs(series) do
    if to > from then
      local whole = ch:reduce(s.field as string, from, to, 1)
      local a, b = whole.min[1], whole.max[1]
      if a and a == a and a < lo then lo = a end
      if b and b == b and b > hi then hi = b end
    end
  end
  if lo == hi then lo, hi = lo - 1, hi + 1 end
  if lo > hi then lo, hi = 0, 1 end
  if lo > 0 and lo < (hi - lo) then lo = 0 end
  local step = nice((hi - lo) / 4)
  lo = math.floor(lo / step) * step
  hi = math.ceil(hi / step) * step
  local y_unit = str(p.y_unit)
  local faint, rule = ref(c, "faint"), ref(c, "rule")
  -- The gutter is as wide as the widest label of the range it is going
  -- to, and of the one in view while it glides there.
  local gutter = 0.0
  do
    local v = lo
    while v <= hi + step / 2 do
      gutter = math.max(gutter, label_w(c, with_unit(tick_number(v, step, math.max(math.abs(lo), math.abs(hi))), y_unit)))
      v = v + step
    end
  end
  -- What is drawn glides to the new range rather than jumping to it.
  lo, hi = glide(c, lo, hi)
  step = nice((hi - lo) / 4)

  -- Its laid-out rect is kept for `layout_of` because it declares
  -- `on_layout`, and a width that moves is an event that redraws it.
  local plot: kui.Props = { key = "plot", width = "grow", height = h, clip = true,
    on_layout = act(c, "line_chart.layout") }
  -- What slides: the lines, their areas and the x ticks, in a box over the
  -- plot keyed by the sample that started the slide, so each sample
  -- starts its own cycle; still, a plain box.
  local function sliding(key: string): kui.Props
    local t: kui.Props = { width = w, height = h, float = { anchor = "parent" } }
    if slide > 0 then
      t.key = key .. st.gen
      t.keyframes = { { at = 0, dx = slide }, { at = 1, dx = 0 } }
      t.transition, t.easing, t.iterations = slide_ms, "linear", 1
    else
      t.key = key
    end
    return t
  end
  local moving = sliding("slide")
  -- Grid lines and their labels: at the step's multiples in view,
  -- right-aligned against the plot.
  local ys: {number} = {}
  local ylabels: {string} = {}
  do
    local top = math.max(math.abs(lo), math.abs(hi))
    local v = math.ceil(lo / step - 1e-9) * step
    while v <= hi + step * 1e-6 do
      local s = with_unit(tick_number(v, step, top), y_unit)
      ys[#ys + 1], ylabels[#ylabels + 1] = v, s
      gutter = math.max(gutter, label_w(c, s))
      v = v + step
    end
  end
  gutter = math.ceil(gutter) + 2
  local labels: kui.Props = { key = "gutter", width = gutter, height = h, transition = 160, when = { ctx.still() } }
  for i, v in ipairs(ys) do
    local y = h - (v - lo) / (hi - lo) * h
    plot[#plot + 1] = line { from = { 0, y }, to = { w, y }, width = 1, color = rule }
    labels[#labels + 1] = row {
      float = { anchor = "parent", clip = true, dx = 0, dy = math.max(0, math.min(h - 12, y - 6)) },
      width = "grow", main_align = "end",
      ctx.data(c, ylabels[i], { size = 10, color = ref(c, "faint") }),
    }
  end
  -- The x axis: ticks at round steps as close as their labels allow, each
  -- label as wide as it measures, the end ones kept inside the plot. An
  -- end label that would touch its neighbour gives way to it, unless its
  -- tick is the plot's edge (`now`): then the neighbour does. Sliding,
  -- the ticks reach back a sample too and slide with the line, cut by
  -- the axis' edges rather than kept inside them.
  local fmt = str(p.x_format) or "number"
  local tick_at, tick_label, tick_w = x_ticks(c, x0, x1, w, fmt, str(p.x_unit))
  if slide > 0 and #tick_at >= 2 then
    tick_at, tick_label = tick_labels(x0 - st.step_x, x1, tick_at[2] - tick_at[1], fmt, str(p.x_unit))
    tick_w = {}
    for i, s in ipairs(tick_label) do tick_w[i] = label_w(c, s) end
  end
  local axis: kui.Props = { width = "grow", height = 14, clip = slide > 0 }
  local axis_moving = sliding("axis")
  local placed: {number} = {}
  local shown: {boolean} = {}
  for i, tx in ipairs(tick_at) do
    local px = (tx - x0) / (x1 - x0) * w
    moving[#moving + 1] = line { from = { px, h - 4 }, to = { px, h }, width = 1, color = faint }
    placed[i] = slide > 0 and px - tick_w[i] / 2 or math.max(0, math.min(w - tick_w[i], px - tick_w[i] / 2))
    shown[i] = true
  end
  local nt = #tick_at
  local function touch(a: integer, b: integer): boolean
    return placed[a] + tick_w[a] + 4 > placed[b]
  end
  if slide == 0 and nt >= 2 and touch(1, 2) then
    local edge = (tick_at[1] - x0) / (x1 - x0) * w < 1
    if edge and nt > 2 then shown[2] = false else shown[1] = false end
  end
  if slide == 0 and nt >= 3 and shown[nt - 1] and touch(nt - 1, nt) then
    local edge = (x1 - tick_at[nt]) / (x1 - x0) * w < 1
    if edge then shown[nt - 1] = false else shown[nt] = false end
  end
  for i = 1, nt do
    if shown[i] then
      axis_moving[#axis_moving + 1] = row { key = "x" .. i, width = tick_w[i],
        float = { anchor = "parent", clip = slide == 0, dx = placed[i], dy = 0 },
        text(tick_label[i], { size = 10, family = "$kitData", color = faint, max_lines = 1 }) }
    end
  end
  axis[#axis + 1] = column(axis_moving)
  local legend: kui.Props = { gap = 12, cross_align = "center" }
  -- Buckets on whole multiples of their row count, the x reductions shared by every series.
  local segs = segments(ch, xf, drawn_from, to, w)
  for k, s in ipairs(series) do
    local color: kui.Color = (s.color as kui.Color) or ("$kitSeries" .. ((k - 1) % 6 + 1))
    local runs = reduced_runs(ch, segs, s.field as string, x0, x1, lo, hi, w, h)
    for _, pts in ipairs(runs) do
    if k == 1 and p.area == true and #pts >= 2 then
      -- The series' colour faded: a skin's own as its area token, a
      -- host's integer with the alpha folded in, anything else (a token
      -- of the host's) at an opacity.
      local fill: kui.Color = "$kitArea" .. ((k - 1) % 6 + 1)
      local opacity: number = nil
      local own = s.color
      if own is integer then
        fill = skin.faded(own, skin.AREA_ALPHA)
      elseif own then
        fill, opacity = color, skin.AREA_ALPHA
      end
      area(moving, pts, h, fill, opacity, r and r.x or 0, c.env.scale or 1)
    end
    if #pts >= 2 then
      moving[#moving + 1] = line { points = pts, width = 1.5, color = color }
    end
    end
    legend[#legend + 1] = row { gap = 4, cross_align = "center",
      column { width = 10, height = 2, bg = color },
      ctx.muted(c, (s.name or s.field) as string) }
  end
  -- The readout: one hover layer over the plot. While the pointer is over
  -- it the view reads where (`pointer_in`, C119) on every frame it is
  -- drawn — a pointer move is a frame — and the crosshair stands at the
  -- pointer's x, the readout naming the sample under it as the data
  -- slides past: a still pointer keeps its crosshair still. The sample
  -- is found where the slide has the line at this frame. The 3 px bands
  -- that named rows did this by hovering, and a band that kept its key
  -- while the window slid kept its old row: the crosshair travelled with
  -- the data under a still pointer.
  local readout = p.readout ~= false
  local focusable = p.focusable == true
  local px_at: number = nil
  if (readout or p.on_hover) and to > from then
    local layer: kui.Props = { key = "over", width = "grow", height = h, hoverable = true,
      float = { anchor = "parent", clip = true }, on_hover = act(c, "line_chart.over") }
    if st.over then
      local at = env.pointer_in("over")
      if at and at.x >= 0 and at.x < at.w and at.y >= 0 and at.y < at.h then
        local off = 0.0
        if slide > 0 then
          off = slide * (1 - math.min(1, math.max(0, (now - st.t_landed) * 1000 / slide_ms)))
        end
        px_at = at.x
        st.row = nearest_row(ch, xf, drawn_from, to, x0 + (at.x - off) / w * (x1 - x0))
      end
      if p.on_hover then
        -- A host that hears the row: bands three px wide, each naming the
        -- row at its middle and keyed by it, so a band that comes to name
        -- another row under a still pointer is a new node there, entered.
        local bands = math.max(1, math.floor(w / 3))
        local bw = w / bands
        local lo_row, run_row, run_from = drawn_from, -1, 0
        local function flush(b: integer)
          if run_row >= 0 then
            layer[#layer + 1] = column { key = "b" .. run_row, width = (b - run_from) * bw, height = h,
              hoverable = true, float = { anchor = "parent", dx = run_from * bw },
              on_hover = act(c, "line_chart.hover", { row = run_row }) }
          end
        end
        for b = 0, bands - 1 do
          local row = nearest_row(ch, xf, lo_row, to, x0 + (b + 0.5) * bw / w * (x1 - x0))
          lo_row = row
          if row ~= run_row then
            flush(b)
            run_row, run_from = row, b
          end
        end
        flush(bands)
      end
    end
    plot[#plot + 1] = row(layer)
  end
  if focusable and to > from then
    -- A Tab stop that is a slider over the samples (2026-10-26): the
    -- arrows step the readout a sample, PageUp / PageDown ten, Home and
    -- End to the ends, and a reader hears the readout as its value. A
    -- slider is no key sink, so every other key — a chord the app binds,
    -- Tab — goes on as from any control.
    local at = st.key_row or (to - 1)
    if at < from then at = from end
    if at > to - 1 then at = to - 1 end
    local _, said = said_of(ch, xf, at, series, fmt, str(p.x_unit), y_unit)
    plot.role = "slider"
    plot.label = "line chart"
    plot.value_min, plot.value_max, plot.value_step, plot.value_now = from, to - 1, 1, at
    plot.value_text = table.concat(said, ", ")
    plot.on_change = act(c, "line_chart.step")
    plot.on_focus = act(c, "line_chart.focus")
  end
  local pointer_row = (st.over and px_at) and st.row or nil
  local at_row = pointer_row or st.key_row
  -- The readout box beside the plot rather than in it: a slider is one
  -- row to a screen reader, its children not read, and the box is a
  -- live region.
  local said_box: kui.Node = nil
  if readout and at_row and at_row >= drawn_from and at_row < to then
    local xv = num(ch:get(at_row, xf)) or x0
    -- The crosshair: at the pointer's x, still while the pointer is; at
    -- the keyboard's sample, sliding with it.
    local px = px_at or (xv - x0) / (x1 - x0) * w
    local layer = px_at and plot or moving
    layer[#layer + 1] = line { from = { px, 0 }, to = { px, h }, width = 1, color = faint }
    local title, said = said_of(ch, xf, at_row, series, fmt, str(p.x_unit), y_unit)
    local box: kui.Props = { key = "readout", gap = 2, pad = { x = 8, y = 6 }, radius = "$kitRadius",
      bg = ref(c, "raised"), border = { w = "$kitHair", color = ref(c, "ink") },
      live = "polite", min_width = "fit",
      ctx.data(c, title, { size = "$kitSizeLabel", color = ref(c, "muted"), max_lines = 1 }) }
    -- Raised over the plot as a float is: the skin's shadow under it.
    ctx.shadow(c, box)
    -- Its width, near enough to say which side it fits on: the labels'
    -- (measured at the axis' size, so scaled up a little) and the swatch,
    -- the gaps and the padding around them.
    local box_w = label_w(c, title)
    local pys: {number} = {}
    for k, s in ipairs(series) do
      local v = num(ch:get(at_row, s.field as string))
      local color: kui.Color = (s.color as kui.Color) or ("$kitSeries" .. ((k - 1) % 6 + 1))
      if v and v == v then
        local py = h - (v - lo) / (hi - lo) * h
        pys[#pys + 1] = py
        layer[#layer + 1] = column { float = { anchor = "parent", dx = px - 3.5, dy = py - 3.5, clip = true },
          width = 7, height = 7, radius = 3.5, bg = color, border = { w = 1, color = ref(c, "panel") } }
        local value = with_unit(string.format("%.4g", v), y_unit)
        local name = (s.name or s.field) as string
        box_w = math.max(box_w, label_w(c, name) + label_w(c, value) + 32)
        box[#box + 1] = row { gap = 6, cross_align = "center",
          column { width = 8, height = 3, bg = color },
          ctx.text(c, name, { size = "$kitSizeLabel", max_lines = 1 }),
          ctx.grow(),
          ctx.data(c, value, { size = "$kitSizeLabel", bold = true, max_lines = 1 }) }
      end
    end
    box.label = table.concat(said, ", ")
    -- Beside the crosshair, right of it unless it would not fit there
    -- (then left), and at the top, the middle or the bottom of the plot,
    -- whichever is furthest from the samples it names, so it keeps off
    -- the lines it reads out.
    box_w = box_w * 1.25 + 16
    local box_h = 12 + 17 * (1 + #pys)
    local right = px + 8 + box_w <= w or px - 8 - box_w < 0
    local dy, best = 4.0, -1.0
    for _, top in ipairs({ 4.0, (h - box_h) / 2, h - 4 - box_h }) do
      local clear = math.huge
      for _, y in ipairs(pys) do
        clear = math.min(clear, y < top and top - y or (y > top + box_h and y - top - box_h or 0.0))
      end
      if clear > best + 0.5 then dy, best = top, clear end
    end
    box.float = { anchor = "parent", at = { "start", "start" }, self = { right and "start" or "end", "start" },
      dx = right and px + 8 or px - 8, dy = math.max(4, dy), clip = true }
    said_box = column(box)
  end
  -- The sliding box under the hover layer and the readout, over the grid.
  table.insert(plot, #ys + 1, column(moving))
  return column {
    width = "grow",
    gap = 4,
    role = focusable and "group" or "image",
    label = "line chart",
    row(legend),
    -- The plot's floats and the readout in tree order (ADR 0053): floats
    -- stack in the order they opened by default, and each sample's slide
    -- opens a box of its own, which went over the readout box opened
    -- before it — the lines showed through the readout as if it were
    -- clear.
    row { width = "grow", gap = 6, column(labels),
      column { width = "grow", height = h, floats_in_order = true, column(plot), said_box } },
    -- The axis under the plot alone: a spacer as wide as the gutter and
    -- the gap, easing with it.
    row { width = "grow", gap = 6,
      column { key = "under", width = gutter, transition = 160, when = { ctx.still() } }, row(axis) },
  }
end

-- Bars standing on a baseline, `values[i]` against `top`, each labelled.
-- With `shown`, each bar carries its value over it.
local function bars(c: Ctx, h: number, values: {number}, names: {string}, kit: string, top: number, ph: number,
                    shown: boolean, unit: string): kui.Node
  if not top then
    top = 0.0
    for _, v in ipairs(values) do if v > top then top = v end end
  end
  if top <= 0 then top = 1 end
  -- Bars a host selects in are one Tab stop, a list of them (ADR 0007):
  -- the arrows walk the bars, Enter or Space picks one, and the bar the
  -- keyboard is on looks as the pointer's does — the hover colour and its
  -- tooltip. Each bar was a stop of its own, ringed apart from the look a
  -- bar under the pointer has.
  local plot: kui.Props = { width = "grow", height = h, gap = 2, cross_align = "end",
    role = kit and "list" or nil, label = kit and "bars" or nil }
  for i, v in ipairs(values) do
    local said = with_unit(short(v), unit)
    local t: kui.Props = {
      key = "b" .. i,
      width = "grow",
      height = math.max(1, v / top * (ph or h)),
      bg = ref(c, "accent"),
      hover_bg = ref(c, "ink"),
      label = (names[i] or "") .. ": " .. said,
      tooltip = (names[i] or "") .. ": " .. said,
    }
    if kit then
      t.role = "listItem"
      t.focusable = true
      t.on_click = act(c, kit, { row = i - 1 })
      t.cursor = "pointer"
      t.focus_bg = ref(c, "ink")
    end
    if shown then
      plot[#plot + 1] = column { key = "c" .. i, width = "grow", min_width = 0, gap = 2, cross_align = "center",
        ctx.data(c, said, { size = 10, color = ref(c, "muted"), max_lines = 1 }),
        column(t) }
    else
      plot[#plot + 1] = column(t)
    end
  end
  return row(plot)
end

-- Bars lying along rows, for labels too long to stand under a bar: the
-- label, the bar, its value; the longest bar takes the room.
local function lying(c: Ctx, values: {number}, names: {string}, kit: string, unit: string, label_w: number): kui.Node
  local top = 0.0
  for _, v in ipairs(values) do if v > top then top = v end end
  if top <= 0 then top = 1 end
  local t: kui.Props = { width = "grow", gap = 4 }
  for i, v in ipairs(values) do
    local said = with_unit(short(v), unit)
    local bar: kui.Props = {
      key = "bar", width = { pct = math.max(0.5, v / top * 100) }, height = 12, bg = ref(c, "accent"),
      hover_bg = ref(c, "ink"),
    }
    local r: kui.Props = {
      key = "b" .. i, width = "grow", height = 18, gap = 8, cross_align = "center",
      label = (names[i] or "") .. ": " .. said, tooltip = (names[i] or "") .. ": " .. said,
      row { width = label_w, min_width = 0, ctx.text(c, names[i] or "", { size = "$kitSizeLabel", max_lines = 1, ellipsis = true }) },
      row { width = "grow", min_width = 0, cross_align = "center", gap = 6, row(bar),
        ctx.data(c, said, { size = 10, color = ref(c, "muted"), max_lines = 1 }) },
    }
    if kit then r.on_click = act(c, kit, { row = i - 1 }); r.cursor = "pointer"; r.hover_bg = ref(c, "hover") end
    t[#t + 1] = row(r)
  end
  return column(t)
end

function M.bar_chart(c: Ctx, p: P): kui.Node
  local ch = p.data as kui.Channel
  local n = ch and #ch or 0
  local values = col(ch, p.value as string)
  local names: {string} = {}
  for i = 0, n - 1 do names[i + 1] = data.cell_text(ch:get(i, p.label as string)) end
  local unit = str(p.unit)
  if p.horizontal == true then
    return column { width = "grow", role = "group", label = "bar chart",
      lying(c, values, names, p.on_select ~= nil and "bar_chart.select" or nil, unit, (p.label_width or 120) as number) }
  end
  local h = (p.height or 160) as number
  -- A value axis: round steps from 0 to past the tallest bar, ruled
  -- across the plot faintly, the bars standing on the ink baseline.
  local top = 0.0
  for _, v in ipairs(values) do if v > top then top = v end end
  local step = nice(math.max(top, 1e-9) / 4)
  local top_tick = math.max(step, math.ceil(top / step - 1e-9) * step)
  local plot_h = h - 16
  -- The value labels' column as wide as the widest of them: a fixed 52 px
  -- for a unit left a gap between "6%" and the first bar wider than a bar.
  local labels: {string} = {}
  local label_w = 0.0
  do
    local v = 0.0
    while v <= top_tick + step / 2 do
      local s = with_unit(short(v), unit)
      labels[#labels + 1] = s
      local m = c.env.measure_text(s, { size = 10, family = "$kitData" } as kui.Props, nil)
      if m and m.width > label_w then label_w = m.width end
      v = v + step
    end
  end
  local scale_w = math.ceil(label_w) + 2
  local scale: kui.Props = { width = scale_w, height = h }
  -- The rules as lines in the plot's own box, drawn before the bars and
  -- so under them (a float would be a layer over them), clipped to the box.
  local plot: kui.Props = { width = "grow", height = h }
  local v = 0.0
  local k = 0
  while v <= top_tick + step / 2 do
    local y = h - v / top_tick * plot_h
    k = k + 1
    scale[#scale + 1] = row {
      float = { anchor = "parent", clip = true, dx = 0, dy = math.max(0, math.min(h - 12, y - 6)) },
      width = scale_w, main_align = "end",
      ctx.data(c, labels[k], { size = 10, color = ref(c, "faint") }),
    }
    if v > 0 then
      plot[#plot + 1] = line { from = { 0, y }, to = { 8192, y }, width = 1, color = ref(c, "rule") }
    end
    v = v + step
  end
  -- Names under their bars, two lines before an ellipsis: a hyphenated
  -- name breaks at its hyphen rather than being cut to its first word.
  local axis: kui.Props = { width = "grow", gap = 2, cross_align = "start" }
  for i = 1, n do
    axis[#axis + 1] = row { width = "grow", min_width = 0, main_align = "center",
      ctx.muted(c, names[i], { size = 10, max_lines = 2, ellipsis = true, wrap = "word" }) }
  end
  local picks = p.on_select ~= nil and "bar_chart.select" or nil
  plot[#plot + 1] = bars(c, h, values, names, picks, top_tick, plot_h, p.values ~= false, unit)
  return column {
    width = "grow", gap = 4, role = "group", label = "bar chart",
    row { width = "grow", gap = 6, column(scale), column { width = "grow", gap = 0, column(plot), ctx.rule(c, "ink", "$kitHair") } },
    row { width = "grow", pad = { l = scale_w + 6 }, row(axis) },
  }
end

local record Binned
  key: string
  counts: {number}
  names: {string}
  lo: number
  hi: number
end
local binned: {integer:Binned} = {}

function M.histogram(c: Ctx, p: P): kui.Node
  local ch = p.data as kui.Channel
  local nbins = math.max(1, math.floor((p.bins or 24) as number))
  local key = ch and string.format("%d/%d/%s/%d", ch.id, ch.rev, p.field as string, nbins) or ""
  local b = binned[c.inst]
  if not b or b.key ~= key then
    local vs = col(ch, p.field as string)
    local lo, hi = range_of(vs, 1, #vs)
    local counts: {number} = {}
    for i = 1, nbins do counts[i] = 0 end
    local width = (hi - lo) / nbins
    for _, v in ipairs(vs) do
      local k = math.min(nbins, math.floor((v - lo) / width) + 1)
      counts[k] = counts[k] + 1
    end
    local names: {string} = {}
    for i = 1, nbins do
      names[i] = short(lo + (i - 1) * width) .. "–" .. short(lo + i * width)
    end
    b = { key = key, counts = counts, names = names, lo = lo, hi = hi }
    binned[c.inst] = b
  end
  return column {
    width = "grow", gap = 4, role = "group", label = "histogram of " .. tostring(p.field),
    bars(c, (p.height or 120) as number, b.counts, b.names, nil, nil, nil, false, nil),
    ctx.rule(c, "ink", "$kitHair"),
    row { width = "grow",
      ctx.data(c, short(b.lo), { size = 10, color = ref(c, "faint") }),
      ctx.grow(),
      ctx.data(c, short(b.hi), { size = 10, color = ref(c, "faint") }) },
  }
end

function M.event(m: P, kit: string): any
  local st = ctx.peek(m.inst as integer) as Glide
  if kit == "line_chart.over" or kit == "sparkline.over" then
    local hv = m.hover as P
    if st and hv then
      st.over = hv.phase == "enter"
      if not st.over then st.row = nil end
    end
  elseif kit == "line_chart.hover" or kit == "sparkline.hover" then
    -- Hover events go to the topmost node alone: the layer is left as a
    -- band is entered, and a band left as the next one is entered, the
    -- leave first — so whichever came last says whether it is over.
    local hv = m.hover as P
    if hv and st then st.over = hv.phase == "enter" end
    if hv and hv.phase == "enter" then
      if kit == "sparkline.hover" then
        if st then st.row = m.row as integer end
      else
        return emit("on_hover", { row = m.row })
      end
    end
  elseif kit == "line_chart.focus" then
    local f = m.focus as P
    if st and f and f.phase == "out" then st.key_row = nil end
  elseif kit == "line_chart.step" then
    -- The slider's step: an arrow, a page, Home or End, a press.
    local ch = m.change as P
    if st and ch and ch.value then st.key_row = math.floor(ch.value as number + 0.5) end
  elseif kit == "bar_chart.select" then
    return emit("on_select", { row = m.row })
  end
  return nil
end

return M
