-- Generated from src/data.tl by addons/kit/build.lua. Edit the .tl.





local ctx = require("ctx")
local icons = require("icons")



local ref, act = ctx.ref, ctx.act

local M = {}

local function str(v)
   return type(v) == "string" and v or nil
end


function M.cell_text(v)
   if v == nil then return "" end
   if math.type(v) == "integer" then return tostring(v) end
   if type(v) == "number" then
      if v == math.floor(v) and math.abs(v) < 1e15 then return string.format("%d", math.floor(v)) end
      return string.format("%.6g", v)
   end
   return tostring(v)
end
local cell_text = M.cell_text

local function count(ch)
   if ch == nil then return 0 end
   return #ch
end




local function slice(env, key, n, row_h)
   local g = env.scroll_geometry(key)
   local vh = g and g.h or env.viewport_h
   local top = g and g.offset.y or 0
   local first = math.min(n, math.max(0, math.floor(top / row_h) - 2))
   local last = math.min(n, math.ceil((top + math.max(vh, 0)) / row_h) + 2)
   if last < first then last = first end
   return first, last
end


local function reveal(env, key, i, row_h)
   local g = env.scroll_geometry(key)
   if not g then return end
   local top = g.offset.y
   local y = i * row_h
   if y < top then
      env.set_scroll(key, 0, y)
   elseif y + row_h > top + g.h then
      env.set_scroll(key, 0, y + row_h - g.h)
   end
end



local function body(c, key, n, row_h, t, build)
   local first, last = slice(c.env, key, n, row_h)
   t.key = key
   t.width = t.width or "grow"
   t.scroll_y = true
   t.row_count = n
   if first > 0 then t[#t + 1] = column({ key = "kit:lead", width = "grow", height = first * row_h }) end
   for i = first, last - 1 do t[#t + 1] = build(i) end
   if last < n then t[#t + 1] = column({ key = "kit:tail", width = "grow", height = (n - last) * row_h }) end
   return column(t)
end







local function step(code, selected, n, page)
   local s = selected or -1
   if code == "down" then s = s + 1
   elseif code == "up" then s = s - 1
   elseif code == "home" then s = 0
   elseif code == "end" then s = n - 1
   elseif code == "pagedown" then s = s + page
   elseif code == "pageup" then s = s - page
   else return nil end
   return math.max(0, math.min(n - 1, s))
end

function M.table(c, p)
   local ch = p.rows
   local cols = (p.columns or {})
   local n = count(ch)
   local row_h = c.s.density == "compact" and 20 or 22
   local env = c.env
   local selected = p.selected
   local seen = ctx.state(c, function() return {} end)
   if selected ~= seen.selected then
      if selected then reveal(env, "rows", selected, row_h) end
      seen.selected = selected
   end

   local head = { width = "grow", height = row_h + 4, pad = { x = 10 }, gap = 10, cross_align = "center", bg = ref(c, "panel") }
   for i, col in ipairs(cols) do
      local field = col.field
      local sorted = p.sort == field
      local h = {
         key = "h" .. i,
         role = "button",
         label = (col.title) .. (sorted and (p.descending == true and ", sorted descending" or ", sorted ascending") or ""),
         width = (col.width) or "grow",
         height = "grow",
         gap = 4,
         cross_align = "center",
         main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
         cursor = "pointer",
         on_click = act(c, "table.sort", { field = field }),
         ctx.caption(c, col.title, { color = ref(c, sorted and "fg" or "muted") }),
      }
      if sorted then h[#h + 1] = icons.node(c, p.descending == true and "chevron_down" or "chevron_up", 11, "fg") end
      head[#head + 1] = row(h)
   end

   local striped = p.striped == true
   local view = body(c, "rows", n, row_h, {
      height = "grow",
      focusable = true,
      on_key = act(c, "table.key", { selected = selected, n = n, page = math.max(1, math.floor((env.viewport_h or 400) / row_h) - 1) }),
      on_range = act(c, "table.range"),
      label = "rows",
   }, function(i)
      local sel = selected == i
      local line = {
         index = i,
         height = row_h,
         width = "grow",
         pad = { x = 10 },
         gap = 10,
         cross_align = "center",
         bg = sel and ref(c, "sel") or ((striped and i % 2 == 1) and ref(c, "hover") or nil),
         hover_bg = sel and ref(c, "sel") or ref(c, "hover"),
         on_click = act(c, "table.select", { row = i }),
      }
      local ink = ref(c, sel and "sel_fg" or "fg")
      local name = {}
      for j, col in ipairs(cols) do
         local s = cell_text(ch:get(i, col.field))
         name[#name + 1] = s
         line[#line + 1] = row({
            width = (col.width) or "grow",
            main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
            text(s, {
               size = "$kitSize",
               color = ink,
               family = col.mono == true and "$kitData" or "$kitUi",
               max_lines = 1,
               ellipsis = true,
            }),
         })
      end
      line.label = table.concat(name, ", ")
      return row(line)
   end)

   return column({
      width = "grow",
      height = p.grow == true and "grow" or nil,
      bg = ref(c, "panel"),
      row(head),
      ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", c.s.skin.ctl_border and "$kitRuleW" or "$kitHair"),
      n == 0 and column({ width = "grow", pad = 16, ctx.muted(c, "No rows") }) or view,
   })
end

function M.list(c, p)
   local ch = p.rows
   local n = count(ch)
   local two = str(p.detail) ~= nil
   local row_h = two and 40 or (c.s.density == "compact" and 22 or 26)
   local selected = p.selected
   local seen = ctx.state(c, function() return {} end)
   if selected ~= seen.selected then
      if selected then reveal(c.env, "rows", selected, row_h) end
      seen.selected = selected
   end
   return body(c, "rows", n, row_h, {
      height = p.grow == true and "grow" or nil,
      role = "list",
      focusable = true,
      on_key = act(c, "list.key", { selected = selected, n = n, page = 8 }),
   }, function(i)
      local sel = selected == i
      local title = cell_text(ch:get(i, p.title))
      local t = {
         index = i,
         role = "listItem",
         label = title,
         selected = sel,
         height = row_h,
         width = "grow",
         pad = { x = 10 },
         main_align = "center",
         gap = 1,
         bg = sel and ref(c, "sel") or nil,
         hover_bg = sel and ref(c, "sel") or ref(c, "hover"),
         on_click = act(c, "list.select", { row = i }),
         ctx.text(c, title, { color = ref(c, sel and "sel_fg" or "fg"), max_lines = 1, ellipsis = true }),
      }
      if two then
         t[#t + 1] = ctx.muted(c, cell_text(ch:get(i, p.detail)), { max_lines = 1, ellipsis = true,
color = ref(c, sel and "sel_fg" or "muted"), })
      end
      return column(t)
   end)
end

function M.inspector(c, p)
   local entries = (p.entries or {})
   local kw = (p.key_width or 140)
   local t = { width = "grow", gap = 2, selectable = true }
   for i, e in ipairs(entries) do
      t[#t + 1] = row({
         key = "e" .. i,
         width = "grow",
         gap = 10,
         row({ width = kw, ctx.muted(c, e.key, { max_lines = 1, ellipsis = true }) }),
         row({ width = "grow",
(e.mono == true and ctx.data or ctx.text)(c, cell_text(e.value), { wrap = "word" }), }),
      })
   end
   return column(t)
end

local LEVEL_ROLE = { debug = "faint", info = "muted", warn = "warn", error = "danger" }





function M.log(c, p)
   local ch = p.rows
   local n = count(ch)
   local row_h = 18
   local env = c.env
   local st = ctx.state(c, function() return { n = 0 } end)
   if ctx.flag(p.follow, true) and n > st.n then

      local g = env.scroll_geometry("rows")
      local at_end = not g or g.offset.y + g.h >= st.n * row_h - row_h
      if at_end then env.set_scroll("rows", 0, n * row_h) end
   end
   st.n = n
   return body(c, "rows", n, row_h, {
      height = p.grow == true and "grow" or nil,
      bg = ref(c, "sunken"),
      pad = { y = 4 },
      selectable = true,
   }, function(i)
      local level = cell_text(ch:get(i, "level"))
      return row({
         index = i,
         height = row_h,
         width = "grow",
         pad = { x = 8 },
         gap = 10,
         cross_align = "center",
         hover_bg = ref(c, "hover"),
         on_click = act(c, "log.select", { row = i }),
         ctx.data(c, cell_text(ch:get(i, "time")), { size = "$kitSizeLabel", color = ref(c, "faint") }),
         row({ width = 44, ctx.data(c, level:upper(), { size = "$kitSizeLabel", bold = true, color = ref(c, LEVEL_ROLE[level] or "muted") }) }),
         ctx.data(c, cell_text(ch:get(i, "message")), { size = "$kitSizeLabel", max_lines = 1, ellipsis = true }),
      })
   end)
end


local function tone_ink(tone)
   if tone == "ok" or tone == "warn" or tone == "danger" then return tone end
   if tone == "accent" then return "on_accent" end
   return "muted"
end

function M.badge(c, p)
   local tone = str(p.tone) or "neutral"
   local accent = tone == "accent"
   local label = str(p.label) or ""
   local t = {
      label = label,
      height = 18,
      pad = { x = 6 },
      cross_align = "center",
      radius = c.s.skin.radius > 0 and 9 or 0,
      bg = ref(c, accent and "accent" or "sunken"),


      ctx.data(c, label, { size = 10.5, bold = true, color = ref(c, tone_ink(tone)), max_lines = 1 }),
   }
   if tone ~= "neutral" and not accent then
      t.border = { w = "$kitHair", color = ref(c, tone) }
   end
   return row(t)
end

function M.stat(c, p)
   local tone = str(p.tone)
   local t = {
      role = "group",
      label = (str(p.label) or "") .. ": " .. (str(p.value) or ""),
      gap = 2,
      ctx.muted(c, ctx.label(c, str(p.label) or ""), { bold = true }),
      ctx.data(c, str(p.value) or "", { size = 22, bold = true }),
   }
   if str(p.delta) then
      local role = (tone == "ok" or tone == "warn" or tone == "danger") and tone or "muted"
      t[#t + 1] = ctx.data(c, p.delta, { size = "$kitSizeLabel", color = ref(c, role) })
   end
   return column(t)
end

function M.code(c, p)
   local wrap = p.wrap == true
   return column({
      width = "grow",
      bg = ref(c, "sunken"),
      radius = "$kitRadius",
      pad = 8,
      scroll_x = not wrap,
      selectable = true,
      ctx.data(c, str(p.text) or "", { wrap = wrap and "word" or "none" }),
   })
end

function M.empty(c, p)
   local t = {
      width = "grow",
      height = "grow",

      min_height = "fit",
      main_align = "center",
      cross_align = "center",
      gap = 6,
      pad = 24,
      text(str(p.title) or "", { size = "$kitSizeTitle", bold = true, family = "$kitUi", color = ref(c, "fg") }),
   }
   if str(p.detail) then
      t[#t + 1] = ctx.muted(c, p.detail, { size = "$kitSize", wrap = "word" })
   end
   t[#t + 1] = row({ gap = 8, pad = { t = 6 }, children("") })
   return column(t)
end

function M.progress(c, p)
   local v = p.value
   local bar
   if v then
      v = math.max(0, math.min(1, v))
      bar = { width = { pct = v * 100 }, height = "grow", bg = ref(c, "accent") }
   else

      bar = {
         width = { pct = 30 }, height = "grow", bg = ref(c, "accent"),
         keyframes = { { dx = -80 }, { dx = 400 } }, transition = 1400,
      }
   end
   local track = row({
      role = "slider",
      label = str(p.label) or "progress",
      value_now = v and v * 100 or nil,
      value_min = 0,
      value_max = 100,
      value_text = v and string.format("%d%%", math.floor(v * 100 + 0.5)) or "under way",
      disabled = true,
      width = "grow",
      height = 4,
      clip = true,
      bg = ref(c, "rule"),
      radius = c.s.skin.radius > 0 and 2 or 0,
      row(bar),
   })
   if not str(p.label) then return track end
   return column({ width = "grow", gap = 4, ctx.muted(c, p.label), track })
end

function M.event(m, kit)
   if kit == "table.select" or kit == "list.select" or kit == "log.select" then
      return emit("on_select", { row = m.row })
   elseif kit == "table.sort" then
      return emit("on_sort", { field = m.field })
   elseif kit == "table.range" then
      local r = m.range
      if r then return emit("on_range", { first = r.first, last = r.last }) end
   elseif kit == "table.key" or kit == "list.key" then
      local k = m.key
      if not k or k.phase ~= "down" then return nil end
      local n = (m.n or 0)
      if n == 0 then return nil end
      local s = step(k.code, m.selected, n, (m.page or 8))
      if s and s ~= m.selected then return emit("on_select", { row = s }) end
   end
   return nil
end

return M