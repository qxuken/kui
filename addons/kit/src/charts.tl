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

-- The first 1-based index whose x is at least `t` (x sorted ascending).
local function lower(xs: {number}, t: number): integer
  local lo, hi = 1, #xs + 1
  while lo < hi do
    local mid = (lo + hi) // 2
    if xs[mid] < t then lo = mid + 1 else hi = mid end
  end
  return lo
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

-- The ticks across [x0, x1] about `px` apart in a plot `w` wide, and each
-- one's label, as `fmt` says:
--   "number" — the value, shortened, with `unit`;
--   "ago"    — x counts back to now (`-15` … `0`): `15 min ago` … `now`;
--   "clock"  — x is Unix seconds: the local time, to the second when the
--              ticks are under a minute apart.
local function x_ticks(x0: number, x1: number, w: number, fmt: string, unit: string): {number}, {string}
  local want = math.max(2, math.floor(w / 90))
  local raw = (x1 - x0) / want
  local step: number
  if fmt == "clock" then
    step = CLOCK_STEPS[#CLOCK_STEPS]
    for _, st in ipairs(CLOCK_STEPS) do
      if st >= raw then step = st; break end
    end
  else
    step = nice(raw)
  end
  local at: {number} = {}
  local labels: {string} = {}
  local v = math.ceil(x0 / step - 1e-9) * step
  while v <= x1 + step * 1e-6 do
    local s: string
    if fmt == "clock" then
      s = os.date(step < 60 and "%H:%M:%S" or "%H:%M", math.floor(v)) as string
    elseif fmt == "ago" then
      local back = math.abs(v)
      s = back < step * 1e-6 and "now" or (with_unit(short(back), unit) .. " ago")
    else
      s = with_unit(short(v), unit)
    end
    at[#at + 1], labels[#labels + 1] = v, s
    v = v + step
  end
  return at, labels
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
-- baseline, the next starting where it ended. A path through them all
-- was rasterized again on every tick of a live chart, whole: 3–4 ms a
-- frame for 300 points at scale 2, where polygons cost a quad each.
local function area(plot: kui.Props, pts: {{number}}, h: number, bg: kui.Color, opacity: number)
  local n = #pts
  local i = 1
  while i < n do
    local j = math.min(i + 5, n)
    local poly: {{number}} = {}
    for k = i, j do poly[k - i + 1] = pts[k] end
    poly[#poly + 1] = { pts[j][1], h }
    poly[#poly + 1] = { pts[i][1], h }
    plot[#plot + 1] = polygon { points = poly, bg = bg, opacity = opacity }
    i = j
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

function M.sparkline(c: Ctx, p: P): kui.Node
  local w, h = (p.width or 80) as number, (p.height or 18) as number
  local ys = col(p.data as kui.Channel, p.field as string)
  local n = #ys
  local box: kui.Props = { width = w, height = h, label = str(p.field), role = "image" }
  if n >= 2 then
    local lo, hi = range_of(ys, 1, n)
    local idx: {number} = {}
    for i = 1, n do idx[i] = i end
    box[#box + 1] = line {
      points = points(idx, ys, 1, n, 1, n, lo, hi, w, h - 2),
      width = 1.5,
      color = (p.color as kui.Color) or ref(c, "accent"),
    }
  end
  return column(box)
end

function M.line_chart(c: Ctx, p: P): kui.Node
  local ch = p.data as kui.Channel
  local env = c.env
  local h = (p.height or 160) as number
  local series = (p.series or {}) as {P}
  local xs = col(ch, p.x as string)
  local n = #xs
  local r = env.layout_of("plot")
  local w = r and r.w or 400
  -- The x window: everything, or the last `window` of it.
  local from, to = 1, n
  local x0, x1 = xs[1] or 0, xs[n] or 1
  if p.window and n > 0 then
    x0 = x1 - (p.window as number)
    from = lower(xs, x0)
  end
  if x1 <= x0 then x1 = x0 + 1 end
  local lo, hi = math.huge, -math.huge
  for _, s in ipairs(series) do
    local a, b = range_of(col(ch, s.field as string), from, to)
    if a < lo then lo = a end
    if b > hi then hi = b end
  end
  if lo > hi then lo, hi = 0, 1 end
  if lo > 0 and lo < (hi - lo) then lo = 0 end
  local step = nice((hi - lo) / 4)
  lo = math.floor(lo / step) * step
  hi = math.ceil(hi / step) * step

  -- Its laid-out rect is kept for `layout_of` because it declares
  -- `on_layout`, and a width that moves is an event that redraws it.
  local plot: kui.Props = { key = "plot", width = "grow", height = h, clip = true,
    on_layout = act(c, "line_chart.layout") }
  -- Grid lines and their labels.
  local labels: kui.Props = { width = str(p.y_unit) and 56 or 44, height = h }
  local v = lo
  while v <= hi + step / 2 do
    local y = h - (v - lo) / (hi - lo) * h
    plot[#plot + 1] = line { from = { 0, y }, to = { w, y }, width = 1, color = ref(c, "rule") }
    labels[#labels + 1] = column {
      float = { anchor = "parent", clip = true, dx = 0, dy = math.max(0, math.min(h - 12, y - 6)) },
      ctx.data(c, with_unit(short(v), str(p.y_unit)), { size = 10, color = ref(c, "faint") }),
    }
    v = v + step
  end
  -- The x axis: ticks at round steps, each with its label, the end ones
  -- kept inside the plot.
  local fmt = str(p.x_format) or "number"
  local tick_at, tick_label = x_ticks(x0, x1, w, fmt, str(p.x_unit))
  local axis: kui.Props = { width = "grow", height = 14 }
  for i, tx in ipairs(tick_at) do
    local px = (tx - x0) / (x1 - x0) * w
    plot[#plot + 1] = line { from = { px, h - 4 }, to = { px, h }, width = 1, color = ref(c, "faint") }
    local lw = 80
    local left = math.max(0, math.min(w - lw, px - lw / 2))
    local align: kui.MainAlign = px - lw / 2 < 0 and "start" or (px + lw / 2 > w and "end" or "center")
    axis[#axis + 1] = row { key = "x" .. i, width = lw, main_align = align,
      float = { anchor = "parent", clip = true, dx = left, dy = 0 },
      ctx.data(c, tick_label[i], { size = 10, color = ref(c, "faint"), max_lines = 1 }) }
  end
  local legend: kui.Props = { gap = 12, cross_align = "center" }
  for k, s in ipairs(series) do
    local ys = col(ch, s.field as string)
    local color: kui.Color = (s.color as kui.Color) or ("$kitSeries" .. ((k - 1) % 6 + 1))
    local pts = points(xs, ys, from, to, x0, x1, lo, hi, w, h)
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
      area(plot, pts, h, fill, opacity)
    end
    if #pts >= 2 then
      plot[#plot + 1] = line { points = pts, width = 1.5, color = color }
    end
    legend[#legend + 1] = row { gap = 4, cross_align = "center",
      column { width = 10, height = 2, bg = color },
      ctx.muted(c, (s.name or s.field) as string) }
  end
  -- Hover bands: a few dozen across the plot, each naming its rows' middle.
  if p.on_hover and to >= from then
    local bands = math.min(to - from + 1, 64)
    local bw = w / bands
    local hover: kui.Props = { width = "grow", height = h, float = { anchor = "parent", clip = true } }
    for b = 0, bands - 1 do
      local mid = from + math.floor((b + 0.5) / bands * (to - from + 1))
      hover[#hover + 1] = column { key = "b" .. b, width = bw, height = h, hoverable = true,
        on_hover = act(c, "line_chart.hover", { row = mid - 1 }) }
    end
    plot[#plot + 1] = row(hover)
  end
  return column {
    width = "grow",
    gap = 4,
    role = "image",
    label = "line chart",
    row(legend),
    row { width = "grow", gap = 6, column(labels), column(plot) },
    row { width = "grow", pad = { l = (str(p.y_unit) and 56 or 44) + 6 }, row(axis) },
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
  if kit == "line_chart.hover" then
    local hv = m.hover as P
    if hv and hv.phase == "enter" then return emit("on_hover", { row = m.row }) end
  elseif kit == "bar_chart.select" then
    return emit("on_select", { row = m.row })
  end
  return nil
end

return M
