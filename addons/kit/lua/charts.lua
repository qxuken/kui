-- Generated from src/charts.tl by addons/kit/build.lua. Edit the .tl.





local ctx = require("ctx")
local data = require("data")



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


local function lower(xs, t)
   local lo, hi = 1, #xs + 1
   while lo < hi do
      local mid = (lo + hi) // 2
      if xs[mid] < t then lo = mid + 1 else hi = mid end
   end
   return lo
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
   return string.format("%.4g", v)
end



local function points(xs, ys, from, to, x0, x1,
   y0, y1, w, h)
   local out = {}
   if to < from then return out end
   local sx = w / math.max(x1 - x0, 1e-12)
   local sy = h / math.max(y1 - y0, 1e-12)
   local bucket, lo, hi, lo_x, hi_x = -1, 0.0, 0.0, 0.0, 0.0
   local function flush()
      if bucket < 0 then return end
      if lo_x <= hi_x then
         out[#out + 1] = { lo_x, h - (lo - y0) * sy }
         if hi ~= lo then out[#out + 1] = { hi_x, h - (hi - y0) * sy } end
      else
         out[#out + 1] = { hi_x, h - (hi - y0) * sy }
         if hi ~= lo then out[#out + 1] = { lo_x, h - (lo - y0) * sy } end
      end
   end
   for i = from, to do
      local x = xs and (xs[i] - x0) * sx or (i - from) * sx
      local y = ys[i] or 0
      local b = math.floor(x / 2)
      if b ~= bucket then
         flush()
         bucket, lo, hi, lo_x, hi_x = b, y, y, x, x
      else
         if y < lo then lo, lo_x = y, x end
         if y > hi then hi, hi_x = y, x end
      end
   end
   flush()
   return out
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
      box[1] = line({
         points = points(idx, ys, 1, n, 1, n, lo, hi, w, h - 2),
         width = 1.5,
         color = (p.color) or ref(c, "accent"),
      })
   end
   return column(box)
end

function M.line_chart(c, p)
   local ch = p.data
   local env = c.env
   local h = (p.height or 160)
   local series = (p.series or {})
   local xs = col(ch, p.x)
   local n = #xs
   local r = env.layout_of("plot")
   local w = r and r.w or 400

   local from, to = 1, n
   local x0, x1 = xs[1] or 0, xs[n] or 1
   if p.window and n > 0 then
      x0 = x1 - (p.window)
      from = lower(xs, x0)
   end
   if x1 <= x0 then x1 = x0 + 1 end
   local lo, hi = math.huge, -math.huge
   for _, s in ipairs(series) do
      local a, b = range_of(col(ch, s.field), from, to)
      if a < lo then lo = a end
      if b > hi then hi = b end
   end
   if lo > hi then lo, hi = 0, 1 end
   if lo > 0 and lo < (hi - lo) then lo = 0 end
   local step = nice((hi - lo) / 4)
   lo = math.floor(lo / step) * step
   hi = math.ceil(hi / step) * step

   local plot = { key = "plot", width = "grow", height = h, clip = true }

   local labels = { width = 44, height = h }
   local v = lo
   while v <= hi + step / 2 do
      local y = h - (v - lo) / (hi - lo) * h
      plot[#plot + 1] = line({ from = { 0, y }, to = { w, y }, width = 1, color = ref(c, "rule") })
      labels[#labels + 1] = column({
         float = { anchor = "parent", dx = 0, dy = math.max(0, math.min(h - 12, y - 6)) },
         ctx.data(c, short(v), { size = 10, color = ref(c, "faint") }),
      })
      v = v + step
   end
   local legend = { gap = 12, cross_align = "center" }
   for k, s in ipairs(series) do
      local ys = col(ch, s.field)
      local color = (s.color) or ("$kitSeries" .. ((k - 1) % 6 + 1))
      local pts = points(xs, ys, from, to, x0, x1, lo, hi, w, h)
      if k == 1 and p.area == true and #pts >= 2 then
         local d = { string.format("M%.1f %.1f", pts[1][1], h) }
         for _, pt in ipairs(pts) do d[#d + 1] = string.format("L%.1f %.1f", pt[1], pt[2]) end
         d[#d + 1] = string.format("L%.1f %.1fZ", pts[#pts][1], h)
         plot[#plot + 1] = path({ d = table.concat(d, " "), bg = color, opacity = 0.14 })
      end
      if #pts >= 2 then
         plot[#plot + 1] = line({ points = pts, width = 1.5, color = color })
      end
      legend[#legend + 1] = row({ gap = 4, cross_align = "center",
column({ width = 10, height = 2, bg = color }),
ctx.muted(c, (s.name or s.field)), })
   end

   if p.on_hover and to >= from then
      local bands = math.min(to - from + 1, 64)
      local bw = w / bands
      local hover = { width = "grow", height = h, float = { anchor = "parent" } }
      for b = 0, bands - 1 do
         local mid = from + math.floor((b + 0.5) / bands * (to - from + 1))
         hover[#hover + 1] = column({ key = "b" .. b, width = bw, height = h, hoverable = true,
on_hover = act(c, "line_chart.hover", { row = mid - 1 }), })
      end
      plot[#plot + 1] = row(hover)
   end
   return column({
      width = "grow",
      gap = 4,
      role = "image",
      label = "line chart",
      row(legend),
      row({ width = "grow", gap = 6, column(labels), column(plot) }),
      row({ width = "grow", pad = { l = 50 },
ctx.data(c, short(x0), { size = 10, color = ref(c, "faint") }),
ctx.grow(),
ctx.data(c, short(x1), { size = 10, color = ref(c, "faint") }), }),
   })
end


local function bars(c, h, values, names, kit)
   local top = 0.0
   for _, v in ipairs(values) do if v > top then top = v end end
   if top <= 0 then top = 1 end
   local plot = { width = "grow", height = h, gap = 2, cross_align = "end" }
   for i, v in ipairs(values) do
      local t = {
         key = "b" .. i,
         width = "grow",
         height = math.max(1, v / top * (h - 14)),
         bg = ref(c, "accent"),
         hover_bg = ref(c, "ink"),
         label = (names[i] or "") .. ": " .. short(v),
         tooltip = (names[i] or "") .. ": " .. short(v),
      }
      if kit then t.on_click = act(c, kit, { row = i - 1 }); t.cursor = "pointer" end
      plot[#plot + 1] = column(t)
   end
   return row(plot)
end

function M.bar_chart(c, p)
   local ch = p.data
   local n = ch and #ch or 0
   local values = col(ch, p.value)
   local names = {}
   for i = 0, n - 1 do names[i + 1] = data.cell_text(ch:get(i, p.label)) end
   local axis = { width = "grow", gap = 2 }
   for i = 1, n do
      axis[#axis + 1] = row({ width = "grow", main_align = "center",
ctx.muted(c, names[i], { size = 10, max_lines = 1, ellipsis = true }), })
   end
   return column({
      width = "grow", gap = 4, role = "group", label = "bar chart",
      bars(c, (p.height or 160), values, names, "bar_chart.select"),
      ctx.rule(c, "ink", "$kitHair"),
      row(axis),
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
      bars(c, (p.height or 120), b.counts, b.names, nil),
      ctx.rule(c, "ink", "$kitHair"),
      row({ width = "grow",
ctx.data(c, short(b.lo), { size = 10, color = ref(c, "faint") }),
ctx.grow(),
ctx.data(c, short(b.hi), { size = 10, color = ref(c, "faint") }), }),
   })
end

function M.event(m, kit)
   if kit == "line_chart.hover" then
      local hv = m.hover
      if hv and hv.phase == "enter" then return emit("on_hover", { row = m.row }) end
   elseif kit == "bar_chart.select" then
      return emit("on_select", { row = m.row })
   end
   return nil
end

return M