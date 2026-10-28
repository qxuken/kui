-- Generated from src/waterfall.tl by addons/kit/build.lua. Edit the .tl.



















local ctx = require("ctx")
local data = require("data")
local icons = require("icons")



local ref, act = ctx.ref, ctx.act



























local M = {}

local LANE_H = 20
local MIN_BAR = 2
local function build(bars, marks, key)
   local lanes = bars and bars:lanes("start", "len", "depth") or nil
   local ticks = (marks and #marks > 0) and marks:lanes("at") or nil
   local lo, hi = 0.0, 1.0
   if lanes and lanes.len > 0 then lo, hi = lanes.lo, lanes.hi end
   return { key = key, bars = bars, marks = marks, lanes = lanes, ticks = ticks,
lo = lo, hi = hi, count = lanes and lanes.len or 0, }
end

local function key_of(ch)
   if not ch then return "-" end
   return ch.id .. "@" .. ch.rev
end

local function state(c, p)
   local st = ctx.state(c, function() return { v0 = 0, v1 = 1, x = 0, w = 0 } end)
   local bars, marks = p.bars, p.marks
   local key = key_of(bars) .. "/" .. key_of(marks)
   if not st.index or st.index.key ~= key then
      local fresh = not st.index or st.index.count == 0
      st.index = build(bars, marks, key)
      if fresh then st.v0, st.v1 = st.index.lo, st.index.hi end
   end
   return st
end


local function range(st, p)
   if p.on_range ~= nil then
      local len = (p.range_length or 0)
      if p.range_start == nil or len <= 0 then return nil, nil end
      return p.range_start, p.range_start + len
   end
   return st.r0, st.r1
end


local function window(st, p)
   if p.start ~= nil or p.length ~= nil then
      local v0 = (p.start or st.index.lo)
      local len = (p.length or (st.index.hi - st.index.lo))
      return v0, v0 + math.max(len, 1e-6)
   end
   return st.v0, st.v1
end



local function fmt(ms, step)
   if step >= 1000 then return string.format("%.1f s", ms / 1000) end
   if step >= 1 then return string.format("%.0f ms", ms) end
   local d = math.min(6, math.max(1, math.ceil(-math.log(step, 10) - 1e-9)))
   if step * 10 ^ d % 1 > 1e-6 then d = math.min(6, d + 1) end
   return string.format("%." .. d .. "f ms", ms)
end

local function nice(raw)
   local p = 10 ^ math.floor(math.log(raw, 10))
   for _, m in ipairs({ 1, 2, 5, 10 }) do
      if m * p >= raw then return m * p end
   end
   return 10 * p
end

local function num(v)
   return type(v) == "number" and v or (tonumber(v) or 0)
end

local function lane_row(c, lane, st, v0, v1, width, selected, picks)
   local scale = width / (v1 - v0)
   local px = 1 / scale
   local t = { height = LANE_H, width = "grow", cross_align = "center", clip = true }
   local runs = st.index.lanes:window(lane, v0, v1, px * MIN_BAR)
   local bars = st.index.bars
   local cursor = 0.0
   for i = 1, #runs.s do
      local x0 = math.max(0, (runs.s[i] - v0) * scale)
      local x1 = math.min(width, (runs.e[i] - v0) * scale)
      local w = x1 - x0
      if w > 0 then
         if x0 > cursor then
            t[#t + 1] = column({ width = x0 - cursor })
         end
         local r = runs.row[i]
         if r >= 0 and w >= MIN_BAR then
            local name = data.cell_text(bars:get(r, "name"))
            local g = math.floor(num(bars:get(r, "group"))) % 6 + 1
            local sel = selected == r
            local bar = {

               label = name,
               width = w,
               height = LANE_H - 4,
               bg = sel and ref(c, "sel") or ("$kitGroup" .. g),
               clip = true,
               pad = { x = 4 },
               cross_align = "center",




               key = picks and ("span" .. r) or nil,
               role = picks and "listItem" or nil,
               focusable = picks or nil,
               on_click = picks and act(c, "waterfall.pick", { row = r }) or nil,



               on_drag = act(c, "waterfall.pan", { v0 = v0, v1 = v1, w = width }),
            }




            if sel then
               bar.border = { w = 2, color = ref(c, "fg") }
            elseif w >= 4 then
               bar.border = { w = 1, color = ref(c, "panel") }
            end
            if w >= 48 then
               bar[#bar + 1] = text(name, { size = 11, max_lines = 1, ellipsis = true, family = "$kitData",
color = sel and ref(c, "sel_fg") or ("$kitGroupFg" .. g), })
            end
            t[#t + 1] = row(bar)
         else
            t[#t + 1] = column({ width = math.max(w, 1), height = LANE_H - 10, bg = ref(c, "faint") })
         end
         cursor = x0 + math.max(w, 1)
      end
   end
   return row(t)
end

local function tool(c, icon, label, action, v0, v1)
   local t = {
      role = "button", label = label, tooltip = label, width = 22, height = 22,
      main_align = "center", cross_align = "center", radius = "$kitRadius",
      hover_bg = ref(c, "hover"), cursor = "pointer",
      on_click = act(c, "waterfall." .. action, { v0 = v0, v1 = v1 }),
      icons.node(c, icon, 13, "fg"),
   }
   return row(t)
end

function M.waterfall(c, p)
   local st = state(c, p)
   local env = c.env
   local r = env.layout_of("lanes")
   if r then st.x, st.w = r.x, r.w end
   local width = st.w > 0 and st.w or math.max(100, env.viewport_w - 40)
   local v0, v1 = window(st, p)
   local scale = width / (v1 - v0)
   local selected = p.selected


   local step = nice(100 / scale)

   local axis = { key = "axis", height = 18, width = "grow", clip = true, cross_align = "end",
cursor = "ewResize", tooltip = "Drag to select a range",
on_drag = act(c, "waterfall.range", { v0 = v0, v1 = v1, x = st.x, w = width }), }
   local t0 = math.ceil(v0 / step) * step
   local lead = (t0 - v0) * scale
   if lead > 0 then axis[#axis + 1] = column({ width = lead }) end
   local tick = t0
   while tick <= v1 do
      axis[#axis + 1] = column({ width = step * scale,
ctx.data(c, fmt(tick, step), { size = 10.5, color = ref(c, "muted") }), })
      tick = tick + step
   end

   local picks = p.on_select ~= nil
   local lanes = {
      key = "lanes",
      role = picks and "list" or nil,
      label = picks and "spans" or nil,
      width = "grow",
      gap = 2,
      on_drag = act(c, "waterfall.pan", { v0 = v0, v1 = v1, w = width }),


      on_layout = act(c, "waterfall.layout"),
      on_scroll = act(c, "waterfall.wheel", { v0 = v0, v1 = v1, x = st.x, w = width }),
   }
   local n_lanes = st.index.lanes and #st.index.lanes or 0
   for lane = 0, n_lanes - 1 do
      lanes[#lanes + 1] = lane_row(c, lane, st, v0, v1, width, selected, picks)
   end


   local a, b = range(st, p)
   local band = nil
   if a then
      local x0 = math.max(-2, (a - v0) * scale)
      local x1 = math.min(width + 2, (b - v0) * scale)
      if x1 > x0 then

         local function shade(key, h)
            local edge = ref(c, "accent_text")
            return {
               column({ key = key, float = { anchor = "parent", dx = x0, dy = 0 }, width = math.max(1, x1 - x0),
height = h, bg = ref(c, "accent"), opacity = 0.16, }),
               column({ key = key .. "-a", float = { anchor = "parent", dx = x0, dy = 0 }, width = 1, height = h, bg = edge }),
               column({ key = key .. "-b", float = { anchor = "parent", dx = x1 - 1, dy = 0 }, width = 1, height = h, bg = edge }),
            }
         end
         for _, n in ipairs(shade("range", math.max(n_lanes, 1) * (LANE_H + 2))) do lanes[#lanes + 1] = n end
         band = shade("range-axis", 18)
      end
   end
   for _, n in ipairs(band or {}) do axis[#axis + 1] = n end


   if st.index.ticks then
      local runs = st.index.ticks:window(0, v0, v1, 1 / scale)
      for i = 1, #runs.s do
         local x = (runs.s[i] - v0) * scale
         local r = runs.row[i]
         local label = r >= 0 and data.cell_text(st.index.marks:get(r, "label")) or
         string.format("%d marks", runs.n[i])
         lanes[#lanes + 1] = column({
            key = "m" .. math.floor(runs.s[i] * 1000),
            float = { anchor = "parent", dx = x, dy = 0 },


            width = 1, height = n_lanes * (LANE_H + 2), bg = ref(c, "accent_text"),
            tooltip = label,
         })
      end
   end



   local gutter = p.gutter
   local function beside(n, grow)
      if not gutter or gutter <= 0 then return n end
      return row({ width = "grow", height = grow and "grow" or nil, gap = 6, column({ width = gutter }), n })
   end
   local head = {
      width = "grow", gap = 8, cross_align = "center",
      ctx.caption(c, "Trace"),
      ctx.data(c, string.format("%s – %s · %d spans", fmt(v0, step), fmt(v1, step), st.index.count),
      { size = "$kitSizeLabel", color = ref(c, "muted") }),
   }
   if a then
      head[#head + 1] = ctx.data(c, string.format("range %s", fmt(b - a, nice((b - a) / 4))),
      { size = "$kitSizeLabel", color = ref(c, "accent_text") })
      head[#head + 1] = tool(c, "fit", "Zoom to the range", "to_range", a, b)
      head[#head + 1] = tool(c, "x", "Clear the range", "clear", v0, v1)
   end
   head[#head + 1] = ctx.grow()
   head[#head + 1] = tool(c, "zoom_out", "Zoom out", "out", v0, v1)
   head[#head + 1] = tool(c, "zoom_in", "Zoom in", "in", v0, v1)
   head[#head + 1] = tool(c, "fit", "Fit", "fit", v0, v1)



   return column({
      width = "grow",
      height = p.grow == true and "grow" or nil,
      gap = 4,
      pad = 10,
      bg = ref(c, "panel"),


      on_scroll = act(c, "waterfall.wheel", { v0 = v0, v1 = v1, x = st.x, w = width }),
      on_drag = act(c, "waterfall.pan", { v0 = v0, v1 = v1, w = width }),
      row(head),
      beside(row(axis), false),
      ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", c.s.skin.ctl_border and "$kitRuleW" or "$kitHair"),
      beside(column({ key = "lanes-box", width = "grow", height = p.grow == true and "grow" or nil,
max_height = p.max_height, scroll_y = p.grow == true or p.max_height ~= nil,
column(lanes), }), p.grow == true),
   })
end


local function zoomed(v0, v1, factor, about)
   local span = math.max((v1 - v0) * factor, 0.01)
   local f = (about - v0) / (v1 - v0)
   local s = about - span * f
   return s, s + span
end

function M.event(m, kit)
   local st = ctx.peek(m.inst)
   if not st then return nil end
   local v0, v1
   if kit == "waterfall.pick" then


      local click = m.click
      if click and ((click.clicks or 1)) >= 2 then
         local bars = st.index.bars
         local s0 = num(bars:get(m.row, "start"))
         local len = num(bars:get(m.row, "len"))
         st.r0, st.r1 = s0, s0 + len
         return emit("on_range", { start = s0, length = len })
      end
      return emit("on_select", { row = m.row })
   elseif kit == "waterfall.pan" then
      local d = m.drag
      local px = ((m.v1) - (m.v0)) / math.max(m.w, 1)

      v0 = (m.v0) - (d.dx) * px
      v1 = (m.v1) - (d.dx) * px
   elseif kit == "waterfall.wheel" then


      local s = m.scroll
      local a, b = m.v0, m.v1
      local wpx = math.max(m.w, 1)
      local about = a + ((s.x) - (m.x)) / wpx * (b - a)
      local factor = math.exp(-((s.dy or 0)) * 0.005 - ((s.zoom or 0)))
      v0, v1 = zoomed(a, b, factor, about)
      local shift = ((s.dx or 0)) / wpx * (v1 - v0)
      v0, v1 = v0 - shift, v1 - shift
   elseif kit == "waterfall.range" then
      local d = m.drag
      local a, b = m.v0, m.v1
      local at = a + ((d.x) - (m.x)) / math.max(m.w, 1) * (b - a)
      if d.phase == "start" then
         st.from = at
         return nil
      end
      local from = st.from or at
      local r0, r1 = math.min(from, at), math.max(from, at)

      if d.phase == "end" and math.abs(d.dx) < 3 then r0, r1 = nil, nil end
      st.r0, st.r1 = r0, r1
      return emit("on_range", { start = r0 or 0, length = r0 and (r1 - r0) or 0 })
   elseif kit == "waterfall.clear" then
      st.r0, st.r1 = nil, nil
      return emit("on_range", { start = 0, length = 0 })
   elseif kit == "waterfall.to_range" then
      v0, v1 = m.v0, m.v1
      st.r0, st.r1 = nil, nil
   elseif kit == "waterfall.in" or kit == "waterfall.out" then
      local a, b = m.v0, m.v1
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