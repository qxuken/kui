-- Generated from src/charts.tl by addons/kit/build.lua. Edit the .tl.





local ctx = require("ctx")
local data = require("data")
local skin = require("skin")



local ref, act = ctx.ref, ctx.act

local M = {}

local function str(v)
   return type(v) == "string" and v or nil
end







local held = {}
local held_n = 0

local function col(ch, name)
   if ch == nil or name == nil then return {} end
   local h = held[ch.id]
   if not h or h.rev ~= ch.rev then
      if not h then

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
         c[i] = type(v) == "number" and v or (tonumber(v) or 0)
      end
      h.cols[name] = c
   end
   return c
end

local function nice(raw)
   if raw <= 0 then return 1 end
   local p = 10 ^ math.floor(math.log(raw, 10))
   local steps = { 1, 2, 2.5, 5, 10 }
   for _, m in ipairs(steps) do
      if m * p >= raw then return m * p end
   end
   return 10 * p
end

local function short(v)
   local a = math.abs(v)
   if a >= 1e9 then return string.format("%.3gG", v / 1e9) end
   if a >= 1e6 then return string.format("%.3gM", v / 1e6) end
   if a >= 1e4 then return string.format("%.3gk", v / 1e3) end
   if a > 0 and a < 1e-9 then return "0" end
   return string.format("%.4g", v)
end


local function with_unit(s, unit)
   if not unit or unit == "" then return s end
   if unit:match("^[%%°]") then return s .. unit end
   return s .. " " .. unit
end


local CLOCK_STEPS = { 1, 2, 5, 10, 15, 30, 60, 120, 300, 600, 900, 1800, 3600, 7200, 10800, 21600, 43200, 86400 }


local NICE = { 1, 2, 2.5, 5, 10 }
local function nicer(step, down)
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


local function clock_step(step, down)
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




local function tick_number(v, step, top)
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




local LABEL = { size = 10, family = "$kitData" }
local widths = setmetatable({}, { __mode = "k" })
local widths_n = setmetatable({}, { __mode = "k" })

local function label_w(c, s)
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







local function tick_labels(x0, x1, step, fmt, unit)
   local at = {}
   local labels = {}
   local top = math.max(math.abs(x0), math.abs(x1))
   local v = math.ceil(x0 / step - 1e-9) * step
   while v <= x1 + step * 1e-6 do
      local s
      if fmt == "clock" then
         s = os.date(step < 60 and "%H:%M:%S" or "%H:%M", math.floor(v + 0.5))
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




local X_GAP = 12
local X_TICK = 50






local function x_ticks(c, x0, x1, w, fmt, unit)
   local span = x1 - x0
   local clock = fmt == "clock"


   local loose = span / math.max(2, math.floor(w / 90))
   local guess_step = clock and clock_step(loose, true) or nice(loose)
   local _, guess = tick_labels(x1 - guess_step, x1, guess_step, fmt, unit)
   local est = guess[#guess] and label_w(c, guess[#guess]) or 40
   local raw = span * math.max(est + X_GAP, X_TICK) / math.max(w, 1)
   local step
   if clock then
      step = CLOCK_STEPS[#CLOCK_STEPS]
      for _, st in ipairs(CLOCK_STEPS) do
         if st >= raw then step = st; break end
      end
      step = clock_step(step, true)
   else
      step = nicer(nice(raw), true)
   end
   local at, labels = {}, {}
   local lw = {}
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








local function points(xs, ys, from, to, x0, x1,
   y0, y1, w, h)
   local out = {}
   if to < from then return out end
   local sx = w / math.max(x1 - x0, 1e-12)
   local sy = h / math.max(y1 - y0, 1e-12)

   local base = h + y0 * sy
   local step = 2 / sx
   local n = 0
   local i = from
   while i <= to do

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


















local function area(plot, pts, h, bg, opacity, ox, scale)
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
      local poly = { start }
      for k = i + 1, j - 1 do poly[k - i + 1] = pts[k] end
      poly[#poly + 1] = stop
      poly[#poly + 1] = { stop[1], h }
      poly[#poly + 1] = { start[1], h }
      plot[#plot + 1] = polygon({ points = poly, bg = bg, opacity = opacity })
      start, i = stop, j
   end
end

local function range_of(ys, from, to)
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




























function M.sparkline(c, p)
   local w, h = (p.width or 80), (p.height or 18)
   local ys = col(p.data, p.field)
   local n = #ys
   local box = { width = w, height = h, label = str(p.field), role = "image" }
   if n >= 2 then
      local lo, hi = range_of(ys, 1, n)
      local idx = {}
      for i = 1, n do idx[i] = i end
      local color = (p.color) or ref(c, "accent")
      box[#box + 1] = line({
         points = points(idx, ys, 1, n, 1, n, lo, hi, w, h - 2),
         width = 1.5,
         color = color,
      })



      if p.readout ~= false then
         local st = ctx.state(c, function() return {} end)
         local layer = { key = "over", width = w, height = h, hoverable = true,
float = { anchor = "parent" }, on_hover = act(c, "sparkline.over"), }
         if st.over then



            local bands = math.max(1, math.floor(w / 2))
            local bw = w / bands
            local run_i, run_from = 0, 0
            local function flush(b)
               if run_i > 0 then
                  layer[#layer + 1] = column({ key = "b" .. run_from, width = (b - run_from) * bw, height = h, hoverable = true,
tooltip = with_unit(string.format("%.4g", ys[run_i]), str(p.unit)),
on_hover = act(c, "sparkline.hover", { row = run_i }), })
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
               box[#box + 1] = column({ float = { anchor = "parent", dx = px - 3, dy = py - 3 },
width = 6, height = 6, radius = 3, bg = color, border = { w = 1, color = ref(c, "panel") }, })
            end
         end
         box[#box + 1] = row(layer)
      end
   end
   return column(box)
end


local function num(v)
   if type(v) == "number" then return v end
   return tonumber(v)
end



local function lower_row(ch, field, lo, hi, t)
   while lo < hi do
      local mid = (lo + hi) // 2
      local x = num(ch:get(mid, field)) or 0
      if x < t then lo = mid + 1 else hi = mid end
   end
   return lo
end

























local function segments(ch, xf, from, to, w)
   local segs = {}
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

local function reduced_runs(ch, segs, field, x0, x1,
   y0, y1, w, h)
   local runs = {}
   local sx = w / math.max(x1 - x0, 1e-12)
   local sy = h / math.max(y1 - y0, 1e-12)
   local base = h + y0 * sy
   local run = {}
   local n = 0
   for _, seg in ipairs(segs) do
      local rx = seg.rx
      local ry = ch:reduce(field, seg.from, seg.to, seg.buckets)


      local xfs, xls, xfr, xlr = rx.first, rx.last, rx.first_row, rx.last_row
      local yfr, ymnr, ymxr, ylr = ry.first_row, ry.min_row, ry.max_row, ry.last_row
      local yf, ymn, ymx, yl = ry.first, ry.min, ry.max, ry.last
      for i = 1, #yf do
         local fr, lr = xfr[i], xlr[i]
         local xa, xb = xfs[i], xls[i]
         local k = lr > fr and (xb - xa) / (lr - fr) or 0


         local r1, v1 = yfr[i], yf[i]
         local r2, v2, r3, v3 = ymnr[i], ymn[i], ymxr[i], ymx[i]
         if r3 < r2 then r2, v2, r3, v3 = r3, v3, r2, v2 end
         local r4, v4 = ylr[i], yl[i]
         for j = 1, 4 do
            local row, v
            if j == 1 then row, v = r1, v1
            elseif j == 2 then row, v = r2, v2
            elseif j == 3 then row, v = r3, v3
            else row, v = r4, v4 end
            local again = (j > 1 and row == r1) or (j > 2 and row == r2) or (j > 3 and row == r3)
            if not again then
               if v ~= v then

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


local GLIDE_MS = 240

















local function glide(c, lo, hi, h)
   local st = ctx.state(c, function() return {} end)
   local now = c.env.now or 0
   local a, b = -h / (hi - lo), h + lo * h / (hi - lo)
   st.ygen = st.ygen or 0
   if not st.ga or now <= 0 or ctx.reduced(c) then
      st.ga, st.gb, st.ga0, st.gb0, st.t0 = a, b, a, b, -math.huge
      return "glide" .. st.ygen, nil, h
   end
   if a ~= st.ga or b ~= st.gb then
      local k = math.min(1, (now - st.t0) * 1000 / GLIDE_MS)
      local e = 1 - (1 - k) ^ 3
      st.ga0, st.gb0 = st.ga0 + (st.ga - st.ga0) * e, st.gb0 + (st.gb - st.gb0) * e
      st.ga, st.gb, st.t0, st.ygen = a, b, now, st.ygen + 1
   end
   local key = "glide" .. st.ygen
   if (now - st.t0) * 1000 >= GLIDE_MS then return key, nil, h end




   local sy = st.ga0 / st.ga
   local dy = st.gb0 - sy * st.gb
   local foot = h
   if sy > 0 and sy < 1 then foot = math.max(h, (h - dy) / sy) end
   return key, { scale_y = sy, dy = dy }, foot
end


local function nearest_row(ch, xf, lo, hi, t)
   local r = lower_row(ch, xf, lo, hi, t)
   if r >= hi then return hi - 1 end
   if r > lo and t - (num(ch:get(r - 1, xf)) or t) < (num(ch:get(r, xf)) or t) - t then return r - 1 end
   return r
end


local function said_of(ch, xf, at, series, fmt, x_unit,
   y_unit)
   local xv = num(ch:get(at, xf)) or 0
   local title
   if fmt == "clock" then
      title = os.date("%H:%M:%S", math.floor(xv + 0.5))
   elseif fmt == "ago" then
      title = math.abs(xv) < 1e-9 and "now" or (with_unit(string.format("%.4g", math.abs(xv)), x_unit) .. " ago")
   else
      title = with_unit(string.format("%.4g", xv), x_unit)
   end
   local said = { title }
   for _, s in ipairs(series) do
      local v = num(ch:get(at, s.field))
      if v and v == v then
         said[#said + 1] = ((s.name or s.field)) .. " " .. with_unit(string.format("%.4g", v), y_unit)
      end
   end
   return title, said
end

function M.line_chart(c, p)
   local ch = p.data
   local env = c.env
   local h = (p.height or 160)
   local series = (p.series or {})
   local xf = p.x
   local n = ch and #ch or 0
   local r = env.layout_of("plot")
   local w = r and r.w or 400


   local from, to = 0, n
   local x0 = n > 0 and (num(ch:get(0, xf)) or 0) or 0
   local x1 = n > 0 and (num(ch:get(n - 1, xf)) or 1) or 1
   if p.window and n > 0 then
      x0 = x1 - (p.window)
      from = lower_row(ch, xf, 0, n, x0)
   end



   if (p.x_min ~= nil or p.x_max ~= nil) and n > 0 then
      x0 = (p.x_min or x0)
      x1 = (p.x_max or x1)
      from = math.max(0, lower_row(ch, xf, 0, n, x0) - 1)
      to = math.min(n, lower_row(ch, xf, from, n, x1) + 1)
   end
   if x1 <= x0 then x1 = x0 + 1 end
   local st = ctx.state(c, function() return {} end)











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
      if st.step_x and st.every and not ctx.reduced(c) then
         slide = st.step_x / (x1 - x0) * w


         slide_ms = st.every * 1000 * 0.95
      end
   end











   local drawn_from = math.max(0, from - 1)
   if slide > 0 then drawn_from = math.max(0, lower_row(ch, xf, 0, n, x0 - st.step_x) - 1) end

   local lo, hi = math.huge, -math.huge
   for _, s in ipairs(series) do
      if to > from then
         local whole = ch:reduce(s.field, from, to, 1)
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

   local gutter = 0.0
   do
      local v = lo
      while v <= hi + step / 2 do
         gutter = math.max(gutter, label_w(c, with_unit(tick_number(v, step, math.max(math.abs(lo), math.abs(hi))), y_unit)))
         v = v + step
      end
   end


   local glide_key, glide_from, foot = glide(c, lo, hi, h)










   local plot = { key = "plot", width = "grow", height = h, clip = true,
on_layout = act(c, "line_chart.layout"), }






   local function sliding(key)
      local t = { width = w, height = h, float = { anchor = "parent" } }
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


   local marks = sliding("marks")


   local gliding = { key = glide_key, width = w, height = h, float = { anchor = "parent" },
pivot_y = 0, }
   if glide_from then
      gliding.transition, gliding.enter = GLIDE_MS, glide_from
   end


   local ys = {}
   local ylabels = {}
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

   gutter = p.gutter ~= nil and p.gutter or (math.ceil(gutter) + 2)
   local labels = { key = "gutter", width = gutter, height = h, transition = 160, when = { ctx.still() } }



   local fade = glide_from and { opacity = 0 } or nil
   for i, v in ipairs(ys) do
      local y = h - (v - lo) / (hi - lo) * h
      local vk = string.format("%.12g", v)


      plot[#plot + 1] = column({ key = "g" .. vk, float = { anchor = "parent", dy = y },
slide = true, transition = GLIDE_MS, enter = fade, exit = { opacity = 0 }, when = { ctx.still() },
line({ from = { 0, 0 }, to = { w, 0 }, width = 1, color = rule }), })
      labels[#labels + 1] = row({
         key = "l" .. vk,
         float = { anchor = "parent", dx = 0, dy = math.max(0, math.min(h - 12, y - 6)) },
         width = "grow", main_align = "end",
         slide = true, transition = GLIDE_MS, enter = fade, exit = { opacity = 0 }, when = { ctx.still() },
         ctx.data(c, ylabels[i], { size = 10, color = ref(c, "faint") }),
      })
   end






   local fmt = str(p.x_format) or "number"
   local tick_at, tick_label, tick_w = x_ticks(c, x0, x1, w, fmt, str(p.x_unit))
   if slide > 0 and #tick_at >= 2 then
      tick_at, tick_label = tick_labels(x0 - st.step_x, x1, tick_at[2] - tick_at[1], fmt, str(p.x_unit))
      tick_w = {}
      for i, s in ipairs(tick_label) do tick_w[i] = label_w(c, s) end
   end
   local axis = { width = "grow", height = 14, clip = slide > 0 }
   local axis_moving = sliding("axis")
   local placed = {}
   local shown = {}
   for i, tx in ipairs(tick_at) do
      local px = (tx - x0) / (x1 - x0) * w
      marks[#marks + 1] = line({ from = { px, h - 4 }, to = { px, h }, width = 1, color = faint })
      placed[i] = slide > 0 and px - tick_w[i] / 2 or math.max(0, math.min(w - tick_w[i], px - tick_w[i] / 2))
      shown[i] = true
   end
   local nt = #tick_at
   local function touch(a, b)
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
         axis_moving[#axis_moving + 1] = row({ key = "x" .. i, width = tick_w[i],
float = { anchor = "parent", dx = placed[i], dy = 0 },
text(tick_label[i], { size = 10, family = "$kitData", color = faint, max_lines = 1 }), })
      end
   end
   axis[#axis + 1] = column(axis_moving)
   local legend = { gap = 12, cross_align = "center" }

   local segs = segments(ch, xf, drawn_from, to, w)
   for k, s in ipairs(series) do
      local color = (s.color) or ("$kitSeries" .. ((k - 1) % 6 + 1))
      local runs = reduced_runs(ch, segs, s.field, x0, x1, lo, hi, w, h)
      for _, pts in ipairs(runs) do
         if k == 1 and p.area == true and #pts >= 2 then



            local fill = "$kitArea" .. ((k - 1) % 6 + 1)
            local opacity = nil
            local own = s.color
            if math.type(own) == "integer" then
               fill = skin.faded(own, skin.AREA_ALPHA)
            elseif own then
               fill, opacity = color, skin.AREA_ALPHA
            end
            area(moving, pts, foot, fill, opacity, r and r.x or 0, c.env.scale or 1)
         end
         if #pts >= 2 then
            moving[#moving + 1] = line({ points = pts, width = 1.5, color = color })
         end
      end
      legend[#legend + 1] = row({ gap = 4, cross_align = "center",
column({ width = 10, height = 2, bg = color }),
ctx.muted(c, (s.name or s.field)), })
   end









   local readout = p.readout ~= false
   local focusable = p.focusable == true
   local px_at = nil
   if (readout or p.on_hover) and to > from then
      local layer = { key = "over", width = "grow", height = h, hoverable = true,
float = { anchor = "parent" }, on_hover = act(c, "line_chart.over"), }
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



            local bands = math.max(1, math.floor(w / 3))
            local bw = w / bands
            local lo_row, run_row, run_from = drawn_from, -1, 0
            local function flush(b)
               if run_row >= 0 then
                  layer[#layer + 1] = column({ key = "b" .. run_row, width = (b - run_from) * bw, height = h,
hoverable = true, float = { anchor = "parent", dx = run_from * bw },
on_hover = act(c, "line_chart.hover", { row = run_row }), })
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



   local said_box = nil
   if readout and at_row and at_row >= drawn_from and at_row < to then
      local xv = num(ch:get(at_row, xf)) or x0


      local px = px_at or (xv - x0) / (x1 - x0) * w
      local layer = px_at and plot or marks
      layer[#layer + 1] = line({ from = { px, 0 }, to = { px, h }, width = 1, color = faint })
      local title, said = said_of(ch, xf, at_row, series, fmt, str(p.x_unit), y_unit)
      local box = { key = "readout", gap = 2, pad = { x = 8, y = 6 }, radius = "$kitRadius",
bg = ref(c, "raised"), border = { w = "$kitHair", color = ref(c, "ink") },
live = "polite", min_width = "fit",
ctx.data(c, title, { size = "$kitSizeLabel", color = ref(c, "muted"), max_lines = 1 }), }

      ctx.shadow(c, box)



      local box_w = label_w(c, title)
      local pys = {}
      for k, s in ipairs(series) do
         local v = num(ch:get(at_row, s.field))
         local color = (s.color) or ("$kitSeries" .. ((k - 1) % 6 + 1))
         if v and v == v then
            local py = h - (v - lo) / (hi - lo) * h
            pys[#pys + 1] = py
            layer[#layer + 1] = column({ float = { anchor = "parent", dx = px - 3.5, dy = py - 3.5 },
width = 7, height = 7, radius = 3.5, bg = color, border = { w = 1, color = ref(c, "panel") }, })
            local value = with_unit(string.format("%.4g", v), y_unit)
            local name = (s.name or s.field)
            box_w = math.max(box_w, label_w(c, name) + label_w(c, value) + 32)
            box[#box + 1] = row({ gap = 6, cross_align = "center",
column({ width = 8, height = 3, bg = color }),
ctx.text(c, name, { size = "$kitSizeLabel", max_lines = 1 }),
ctx.grow(),
ctx.data(c, value, { size = "$kitSizeLabel", bold = true, max_lines = 1 }), })
         end
      end
      box.label = table.concat(said, ", ")




      box_w = box_w * 1.25 + 16
      local box_h = 12 + 17 * (1 + #pys)
      local right = px + 8 + box_w <= w or px - 8 - box_w < 0



      local x = right and px + 8 or px - 8 - box_w
      x = math.max(0, math.min(x, w - box_w))
      local dy, best = 4.0, -1.0
      for _, top in ipairs({ 4.0, (h - box_h) / 2, h - 4 - box_h }) do
         local clear = math.huge
         for _, y in ipairs(pys) do
            clear = math.min(clear, y < top and top - y or (y > top + box_h and y - top - box_h or 0.0))
         end
         if clear > best + 0.5 then dy, best = top, clear end
      end


      box.float = { anchor = "parent", dx = x, dy = math.max(4, math.min(dy, h - 4 - box_h)) }
      said_box = column(box)
   end


   gliding[1] = column(moving)
   table.insert(plot, #ys + 1, column(gliding))
   table.insert(plot, #ys + 2, column(marks))
   return column({
      width = "grow",
      gap = 4,
      role = focusable and "group" or "image",
      label = "line chart",
      row(legend),





      row({ width = "grow", gap = 6, column(labels),
column({ width = "grow", height = h, floats_in_order = true, column(plot), said_box }), }),


      row({ width = "grow", gap = 6,
column({ key = "under", width = gutter, transition = 160, when = { ctx.still() } }), row(axis), }),
   })
end



local function bars(c, h, values, names, kit, top, ph,
   shown, unit)
   if not top then
      top = 0.0
      for _, v in ipairs(values) do if v > top then top = v end end
   end
   if top <= 0 then top = 1 end





   local plot = { width = "grow", height = h, gap = 2, cross_align = "end",
role = kit and "list" or nil, label = kit and "bars" or nil, }
   for i, v in ipairs(values) do
      local said = with_unit(short(v), unit)
      local t = {
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
         plot[#plot + 1] = column({ key = "c" .. i, width = "grow", min_width = 0, gap = 2, cross_align = "center",
ctx.data(c, said, { size = 10, color = ref(c, "muted"), max_lines = 1 }),
column(t), })
      else
         plot[#plot + 1] = column(t)
      end
   end
   return row(plot)
end



local function lying(c, values, names, kit, unit, label_w)
   local top = 0.0
   for _, v in ipairs(values) do if v > top then top = v end end
   if top <= 0 then top = 1 end
   local t = { width = "grow", gap = 4 }
   for i, v in ipairs(values) do
      local said = with_unit(short(v), unit)
      local bar = {
         key = "bar", width = { pct = math.max(0.5, v / top * 100) }, height = 12, bg = ref(c, "accent"),
         hover_bg = ref(c, "ink"),
      }
      local r = {
         key = "b" .. i, width = "grow", height = 18, gap = 8, cross_align = "center",
         label = (names[i] or "") .. ": " .. said, tooltip = (names[i] or "") .. ": " .. said,
         row({ width = label_w, min_width = 0, ctx.text(c, names[i] or "", { size = "$kitSizeLabel", max_lines = 1, ellipsis = true }) }),
         row({ width = "grow", min_width = 0, cross_align = "center", gap = 6, row(bar),
ctx.data(c, said, { size = 10, color = ref(c, "muted"), max_lines = 1 }), }),
      }
      if kit then r.on_click = act(c, kit, { row = i - 1 }); r.cursor = "pointer"; r.hover_bg = ref(c, "hover") end
      t[#t + 1] = row(r)
   end
   return column(t)
end

function M.bar_chart(c, p)
   local ch = p.data
   local n = ch and #ch or 0
   local values = col(ch, p.value)
   local names = {}
   for i = 0, n - 1 do names[i + 1] = data.cell_text(ch:get(i, p.label)) end
   local unit = str(p.unit)
   if p.horizontal == true then
      return column({ width = "grow", role = "group", label = "bar chart",
lying(c, values, names, p.on_select ~= nil and "bar_chart.select" or nil, unit, (p.label_width or 120)), })
   end
   local h = (p.height or 160)


   local top = 0.0
   for _, v in ipairs(values) do if v > top then top = v end end
   local step = nice(math.max(top, 1e-9) / 4)
   local top_tick = math.max(step, math.ceil(top / step - 1e-9) * step)
   local plot_h = h - 16


   local labels = {}
   local label_w = 0.0
   do
      local v = 0.0
      while v <= top_tick + step / 2 do
         local s = with_unit(short(v), unit)
         labels[#labels + 1] = s
         local m = c.env.measure_text(s, { size = 10, family = "$kitData" }, nil)
         if m and m.width > label_w then label_w = m.width end
         v = v + step
      end
   end
   local scale_w = math.ceil(label_w) + 2
   local scale = { width = scale_w, height = h }


   local plot = { width = "grow", height = h }
   local v = 0.0
   local k = 0
   while v <= top_tick + step / 2 do
      local y = h - v / top_tick * plot_h
      k = k + 1
      scale[#scale + 1] = row({
         float = { anchor = "parent", dx = 0, dy = math.max(0, math.min(h - 12, y - 6)) },
         width = scale_w, main_align = "end",
         ctx.data(c, labels[k], { size = 10, color = ref(c, "faint") }),
      })
      if v > 0 then
         plot[#plot + 1] = line({ from = { 0, y }, to = { 8192, y }, width = 1, color = ref(c, "rule") })
      end
      v = v + step
   end


   local axis = { width = "grow", gap = 2, cross_align = "start" }
   for i = 1, n do
      axis[#axis + 1] = row({ width = "grow", min_width = 0, main_align = "center",
ctx.muted(c, names[i], { size = 10, max_lines = 2, ellipsis = true, wrap = "word" }), })
   end
   local picks = p.on_select ~= nil and "bar_chart.select" or nil
   plot[#plot + 1] = bars(c, h, values, names, picks, top_tick, plot_h, p.values ~= false, unit)
   return column({
      width = "grow", gap = 4, role = "group", label = "bar chart",
      row({ width = "grow", gap = 6, column(scale), column({ width = "grow", gap = 0, column(plot), ctx.rule(c, "ink", "$kitHair") }) }),
      row({ width = "grow", pad = { l = scale_w + 6 }, row(axis) }),
   })
end








local binned = {}

function M.histogram(c, p)
   local ch = p.data
   local nbins = math.max(1, math.floor((p.bins or 24)))
   local key = ch and string.format("%d/%d/%s/%d", ch.id, ch.rev, p.field, nbins) or ""
   local b = binned[c.inst]
   if not b or b.key ~= key then
      local vs = col(ch, p.field)
      local lo, hi = range_of(vs, 1, #vs)
      local counts = {}
      for i = 1, nbins do counts[i] = 0 end
      local width = (hi - lo) / nbins
      for _, v in ipairs(vs) do
         local k = math.min(nbins, math.floor((v - lo) / width) + 1)
         counts[k] = counts[k] + 1
      end
      local names = {}
      for i = 1, nbins do
         names[i] = short(lo + (i - 1) * width) .. "–" .. short(lo + i * width)
      end
      b = { key = key, counts = counts, names = names, lo = lo, hi = hi }
      binned[c.inst] = b
   end
   return column({
      width = "grow", gap = 4, role = "group", label = "histogram of " .. tostring(p.field),
      bars(c, (p.height or 120), b.counts, b.names, nil, nil, nil, false, nil),
      ctx.rule(c, "ink", "$kitHair"),
      row({ width = "grow",
ctx.data(c, short(b.lo), { size = 10, color = ref(c, "faint") }),
ctx.grow(),
ctx.data(c, short(b.hi), { size = 10, color = ref(c, "faint") }), }),
   })
end

function M.event(m, kit)
   local st = ctx.peek(m.inst)
   if kit == "line_chart.over" or kit == "sparkline.over" then
      local hv = m.hover
      if st and hv then
         st.over = hv.phase == "enter"
         if not st.over then st.row = nil end
      end
   elseif kit == "line_chart.hover" or kit == "sparkline.hover" then



      local hv = m.hover
      if hv and st then st.over = hv.phase == "enter" end
      if hv and hv.phase == "enter" then
         if kit == "sparkline.hover" then
            if st then st.row = m.row end
         else
            return emit("on_hover", { row = m.row })
         end
      end
   elseif kit == "line_chart.focus" then
      local f = m.focus
      if st and f and f.phase == "out" then st.key_row = nil end
   elseif kit == "line_chart.step" then

      local ch = m.change
      if st and ch and ch.value then st.key_row = math.floor(ch.value + 0.5) end
   elseif kit == "bar_chart.select" then
      return emit("on_select", { row = m.row })
   end
   return nil
end

return M