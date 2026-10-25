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












local function hand_on(c, seen)
   local dir = seen.tab
   if not dir then return end
   seen.tab = nil
   if dir > 0 then c.env.focus_next() else c.env.focus_prev() end
end



local function focus_edge(c, t)
   local ok, here = pcall(c.env.is_focused, "rows")
   if ok and here then t.border = { w = "$kitRuleW", color = ref(c, "focus") } end
end



local TONE_ROLE = { ok = "ok", warn = "warn", danger = "danger", accent = "accent", neutral = "muted" }

local function tone_of(v)
   return type(v) == "string" and TONE_ROLE[v] or nil
end






local function pill(c, label, tone, dot, sel)
   local role = tone_of(tone)
   local accent = role == "accent" and not sel
   local ink = ref(c, sel and "sel_fg" or (accent and "on_accent" or (role or "muted")))
   local spot = sel and "sel_fg" or (accent and "on_accent" or (role == "muted" and "faint" or (role or "faint")))
   return row({
      height = 18, min_width = "fit", pad = { x = 6 }, gap = 5, cross_align = "center",
      radius = c.s.skin.radius > 0 and 9 or 0,
      bg = accent and ref(c, "accent") or (not sel and ref(c, "press") or nil),
      dot and column({ width = 6, height = 6, radius = 3, bg = ref(c, spot) }) or nil,
      ctx.data(c, label, { size = "$kitSizeLabel", bold = c.s.skin.label_bold, color = ink, max_lines = 1 }),
   })
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
   hand_on(c, seen)
   if selected ~= seen.selected then
      if selected then reveal(env, "rows", selected, row_h) end
      seen.selected = selected
   end



   local framed = p.border == true
   local head = { width = "grow", height = row_h + 4, pad = { x = 10 }, gap = 10, cross_align = "center",
bg = ref(c, framed and "panel" or "sunken"), }
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
   local rows_t = {
      height = "grow",
      focusable = true,
      on_key = act(c, "table.key", { selected = selected, n = n, page = math.max(1, math.floor((env.viewport_h or 400) / row_h) - 1) }),
      on_range = act(c, "table.range"),
      label = "rows",
   }
   focus_edge(c, rows_t)
   local view = body(c, "rows", n, row_h, rows_t, function(i)
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



         local tone = str(col.tone) and cell_text(ch:get(i, col.tone)) or nil
         local cell
         if col.badge == true and s ~= "" then
            cell = pill(c, s, tone, true, sel)
         else
            local role = not sel and tone_of(tone) or nil
            cell = text(s, {
               size = "$kitSize",
               color = role and ref(c, role == "accent" and "fg" or role) or ink,
               bold = role ~= nil and role ~= "muted" and c.s.skin.label_bold or nil,
               family = col.mono == true and "$kitData" or "$kitUi",
               max_lines = 1,
               ellipsis = true,
            })
         end
         line[#line + 1] = row({
            width = (col.width) or "grow",
            min_width = 0,
            cross_align = "center",
            main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
            cell,
         })
      end
      line.label = table.concat(name, ", ")
      return row(line)
   end)

   return column({
      width = "grow",
      height = p.grow == true and "grow" or nil,
      bg = ref(c, "panel"),



      border = framed and { w = "$kitHair", color = ref(c, "rule") } or nil,
      pad = framed and "$kitHair" or nil,
      row(head),
      framed and ctx.rule(c, "rule", "$kitHair") or nil,
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
   hand_on(c, seen)
   if selected ~= seen.selected then
      if selected then reveal(c.env, "rows", selected, row_h) end
      seen.selected = selected
   end
   local rows_t = {
      height = p.grow == true and "grow" or nil,
      role = "list",
      label = str(p.label),
      focusable = true,
      on_key = act(c, "list.key", { selected = selected, n = n, page = 8 }),
   }
   focus_edge(c, rows_t)
   return body(c, "rows", n, row_h, rows_t, function(i)
      local sel = selected == i
      local title = cell_text(ch:get(i, p.title))
      local t = {
         width = "grow",
         min_width = 0,
         main_align = "center",
         gap = 1,
         ctx.text(c, title, { color = ref(c, sel and "sel_fg" or "fg"), max_lines = 1, ellipsis = true }),
      }
      if two then
         t[#t + 1] = ctx.muted(c, cell_text(ch:get(i, p.detail)), { max_lines = 1, ellipsis = true,
color = ref(c, sel and "sel_fg" or "muted"), })
      end


      local badge = str(p.badge) and cell_text(ch:get(i, p.badge)) or ""
      local tone = str(p.tone) and cell_text(ch:get(i, p.tone)) or nil
      return row({
         index = i,
         role = "listItem",
         label = badge ~= "" and (badge .. " " .. title) or title,
         selected = sel,
         height = row_h,
         width = "grow",
         pad = { x = 10 },
         gap = 8,
         cross_align = "center",
         bg = sel and ref(c, "sel") or nil,
         hover_bg = sel and ref(c, "sel") or ref(c, "hover"),
         on_click = act(c, "list.select", { row = i }),
         badge ~= "" and row({ width = p.badge_width, min_width = "fit",
pill(c, badge, tone, false, sel), }) or nil,
         column(t),
      })
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




local ANSI = {
   [0] = "faint", [1] = "danger", [2] = "ok", [3] = "warn",
   [4] = "$kitSeries2", [5] = "$kitSeries5", [6] = "$kitSeries6", [7] = "fg",
}

local function ansi_colour(c, n)
   local role = ANSI[n % 8]
   if role:sub(1, 1) == "$" then return role end
   return ref(c, role)
end

local function rgba(r, g, b)
   return ((r & 255) << 24) | ((g & 255) << 16) | ((b & 255) << 8) | 0xff
end


local function colour256(c, n)
   if n < 16 then return ansi_colour(c, n) end
   if n >= 232 then
      local v = 8 + (n - 232) * 10
      return rgba(v, v, v)
   end
   local function level(v) return v == 0 and 0 or 55 + v * 40 end
   n = n - 16
   return rgba(level(n // 36), level(n // 6 % 6), level(n % 6))
end








local function apply_sgr(c, st, params)
   local i = 1
   while i <= #params do
      local q = params[i]
      if q == 0 then st.color, st.bold, st.italic, st.underline = nil, false, false, false
      elseif q == 1 then st.bold = true
      elseif q == 3 then st.italic = true
      elseif q == 4 then st.underline = true
      elseif q == 22 then st.bold = false
      elseif q == 23 then st.italic = false
      elseif q == 24 then st.underline = false
      elseif q >= 30 and q <= 37 then st.color = ansi_colour(c, q - 30)
      elseif q >= 90 and q <= 97 then st.color = ansi_colour(c, q - 90)
      elseif q == 39 then st.color = nil
      elseif q == 38 then
         if params[i + 1] == 5 then
            st.color = colour256(c, params[i + 2] or 0)
            i = i + 2
         elseif params[i + 1] == 2 then
            st.color = rgba(params[i + 2] or 0, params[i + 3] or 0, params[i + 4] or 0)
            i = i + 4
         end
      end
      i = i + 1
   end
end




local function message(c, raw)
   raw = raw:match("[^\r]*$")
   if not raw:find("\27", 1, true) then return raw end
   local spans = {}
   local st = { bold = false, italic = false, underline = false }
   local at = 1
   while at <= #raw do
      local s, e, params, final = raw:find("\27%[([%d;:?]*)(%a)", at)
      local chunk = raw:sub(at, (s or #raw + 1) - 1)
      if chunk ~= "" then
         if st.color or st.bold or st.italic or st.underline then
            spans[#spans + 1] = { chunk, color = st.color, bold = st.bold or nil, italic = st.italic or nil,
underline = st.underline or nil, }
         else
            spans[#spans + 1] = chunk
         end
      end
      if not s then break end
      if final == "m" then
         local list = {}
         for n in (params ~= "" and params or "0"):gmatch("%d+") do list[#list + 1] = math.tointeger(tonumber(n)) end
         apply_sgr(c, st, list)
      end
      at = e + 1
   end
   return spans
end




















local KEYED = 20000




local function dropped_front(st, n, key_of)
   local keys = st.keys
   if not keys or #keys == 0 or n == 0 then return nil end
   local first = key_of(0)
   for j = 1, math.min(#keys, 1000) do
      if keys[j] == first then
         local k = j - 1
         local overlap = #keys - k
         if overlap <= n and key_of(overlap - 1) == keys[#keys] then return k end
         return nil
      end
   end
   return nil
end





local function following(st, g)
   if st.follow == nil then st.follow = true end
   if not g then return end
   local y = g.offset.y


   if y >= g.max_offset.y - 1 then
      st.follow = true
   elseif st.last_y and y < st.last_y - 0.5 then
      st.follow = false
   end
   st.last_y = y
end



local function jump(c)
   return row({
      key = "jump",
      role = "button",
      label = "Jump to the latest line",
      float = { anchor = "parent", at = { "end", "end" }, self = { "end", "end" }, dx = -12, dy = -8 },
      height = 22,
      pad = { x = 8 },
      gap = 5,
      cross_align = "center",
      radius = "$kitRadius",
      bg = ref(c, "raised"),
      hover_bg = ref(c, "raised_hover"),
      pressed_bg = ref(c, "raised_press"),
      border = { w = "$kitHair", color = ref(c, c.s.skin.ctl_border and "ink" or "rule") },
      cursor = "pointer",
      on_click = act(c, "log.end"),
      icons.node(c, "chevron_down", 12, "fg"),
      ctx.caption(c, "Latest", { size = "$kitSizeLabel" }),
   })
end

function M.log(c, p)
   local ch = p.rows
   local n = count(ch)
   local row_h = 18
   local env = c.env
   local wrap = p.wrap == true
   local with_time = p.time ~= false
   local st = ctx.state(c, function() return { n = 0, rev = "" } end)
   local function line(i)
      local level = cell_text(ch:get(i, "level"))
      local text_opts = { size = "$kitSizeLabel" }
      if wrap then text_opts.wrap = "word" else text_opts.max_lines, text_opts.ellipsis = 1, true end
      return row({
         index = i,
         height = not wrap and row_h or nil,
         width = "grow",
         pad = { x = 8, y = wrap and 1 or 0 },
         gap = 10,
         cross_align = wrap and "start" or "center",
         hover_bg = ref(c, "hover"),
         on_click = act(c, "log.select", { row = i }),
         with_time and ctx.data(c, cell_text(ch:get(i, "time")), { size = "$kitSizeLabel", color = ref(c, "faint") }) or nil,
         row({ width = 44, ctx.data(c, level:upper(), { size = "$kitSizeLabel", bold = true, color = ref(c, LEVEL_ROLE[level] or "muted") }) }),
         row({ width = "grow", min_width = 0,
ctx.data(c, message(c, cell_text(ch:get(i, "message"))), text_opts), }),
      })
   end
   local wants = ctx.flag(p.follow, true)
   following(st, env.scroll_geometry("rows"))



   local rev = ch and (tostring(ch.id) .. ":" .. tostring(ch.rev)) or ""
   local dropped = nil
   if rev ~= st.rev then
      local function key_of(i)
         return cell_text(ch:get(i, "time")) .. "\31" .. cell_text(ch:get(i, "message"))
      end
      dropped = dropped_front(st, n, key_of)
      if n <= KEYED then
         local keys = {}
         for i = 0, n - 1 do keys[i + 1] = key_of(i) end
         st.keys = keys
      else
         st.keys = nil
      end
   end










   local tails = wants and st.follow and n > 0
   local function framed(t)
      local away = wants and not st.follow and n > 0
      return column({ width = "grow", height = p.grow == true and "grow" or nil, t, away and jump(c) or nil })
   end
   if not wrap then
      if wants and st.follow and n > 0 then

         env.set_scroll("rows", 0, 1e9)
      elseif dropped and dropped > 0 then

         env.shift_scroll("rows", -dropped * row_h, -dropped * row_h)
      end
      st.rev = rev
      st.n = n
      return framed(body(c, "rows", n, row_h, {
         height = p.grow == true and "grow" or nil,
         bg = ref(c, "sunken"),
         pad = { y = 4 },
         selectable = true,
         anchor = tails or nil,
      }, line))
   end



   if not st.heights then st.heights = row_heights(n, row_h) end
   local g = env.scroll_geometry("rows")
   local hold = nil
   if rev ~= st.rev then
      if dropped == 0 then



         st.heights:set_len(n)
      elseif dropped then


         local old = st.heights
         local fresh = row_heights(n, old:estimate())
         for i = 0, old:len() - dropped - 1 do
            local h = old:measured(i + dropped)
            if h and h > 0 then fresh:set(i, h) end
         end
         st.heights = fresh
         if not (wants and st.follow) and g then


            local top = math.max(0, g.offset.y - 4)
            local at = old:row_at(top)
            hold = { row = at - dropped, into = top - old:offset_of(at), top = top }
         end
      else
         st.heights:clear()
         st.heights:set_len(n)
      end
   end
   local label_w = (with_time and 70 or 0) + 54 + 16
   local function measure(i, width)
      local m = env.measure_text(message(c, cell_text(ch:get(i, "message"))),
      { size = "$kitSizeLabel", family = "$kitData" }, math.max(40, width - label_w))
      return math.max(row_h, m.height + 2)
   end
   local pin = wants and st.follow and n > 0
   if pin and g and g.w > 0 then


      local filled, i = 0.0, n - 1
      while i >= 0 and filled < g.h + row_h do
         local h = st.heights:measured(i)
         if not h or h <= 0 then
            h = measure(i, g.w)
            st.heights:set(i, h)
         end
         filled = filled + h
         i = i - 1
      end
   end
   local t = list(env, {
      key = "rows",
      heights = st.heights,
      width = "grow",
      height = p.grow == true and "grow" or nil,
      bg = ref(c, "sunken"),
      pad = { y = 4 },
      selectable = true,
      anchor = tails or nil,
   }, measure, line)
   if hold and hold.row >= 0 then



      local want = st.heights:offset_of(math.floor(hold.row)) + hold.into
      local at = env.scroll_offset("rows").y - 4
      local by = want - math.max(0, at)
      if math.abs(by) > 0.01 then env.shift_scroll("rows", by, by) end
   end
   if pin then





      env.set_scroll("rows", 0, 1e9)
   end
   st.n, st.rev = n, rev
   return framed(t)
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
   local fade = p.fade == true
   local t = {
      label = label,
      tooltip = str(p.tooltip),
      height = 18,

      min_width = "fit",


      enter = fade and { opacity = 0 } or nil,
      exit = fade and { opacity = 0 } or nil,
      transition = fade and 140 or nil,
      pad = { x = 6 },
      gap = 5,
      cross_align = "center",
      radius = c.s.skin.radius > 0 and 9 or 0,
      bg = ref(c, accent and "accent" or "sunken"),


      p.dot == true and column({ width = 6, height = 6, radius = 3,
bg = ref(c, tone == "neutral" and "faint" or (accent and "on_accent" or tone)), }) or nil,
      ctx.text(c, label, { size = "$kitSizeLabel", bold = c.s.skin.label_bold, color = ref(c, tone_ink(tone)), max_lines = 1 }),
   }


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

local LINE = 16

function M.code(c, p)
   local wrap = p.wrap == true
   local lines = p.max_lines
   local body = ctx.data(c, str(p.text) or "", { wrap = wrap and "glyph" or "none", line_height = LINE })


   local inner = { key = "lines", width = "grow", selectable = true, body }
   if lines then
      inner.max_height = lines * LINE
      inner.scroll_y = true
   end
   inner.scroll_x = not wrap
   return column({
      width = "grow",
      bg = ref(c, "sunken"),
      radius = "$kitRadius",
      pad = 8,
      column(inner),
   })
end

local NOTICE_ROLE = { danger = "danger", warn = "warn", ok = "ok", accent = "accent", neutral = "muted" }

function M.notice(c, p)
   local tone = str(p.tone) or "neutral"
   local role = NOTICE_ROLE[tone] or "muted"


   local body = { width = "grow", gap = 2, live = tone == "danger" and "assertive" or "polite" }
   if str(p.title) then
      body[#body + 1] = ctx.text(c, p.title, { bold = true, color = ref(c, role == "muted" and "fg" or role), wrap = "word" })
   end
   body[#body + 1] = ctx.text(c, str(p.message) or "", { color = ref(c, role == "muted" and "fg" or role), wrap = "word" })




   body.width, body.min_width = "grow", 160
   local t = {
      role = "group",

      label = str(p.title),
      width = "grow",
      pad = { x = 12, y = 8 },
      gap = 10,
      cross_align = "center",
      radius = "$kitRadius",
      bg = ref(c, "sunken"),
      border = { w = "$kitHair", color = ref(c, role == "muted" and "rule" or role) },
      icons.node(c, tone == "ok" and "check" or (tone == "neutral" or tone == "accent") and "info" or "alert", 14, role),
      row({ key = "wrap", width = "grow", min_width = 0, gap = 10, cross_align = "center", wrap_children = true,
column(body),
row({ key = "actions", min_width = "fit", gap = 8, cross_align = "center", children("") }), }),
   }
   return row(t)
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


      bar = { key = "bar", width = { pct = v * 100 }, height = "grow", bg = ref(c, "accent"),
transition = 320, easing = "snappy", }
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
   elseif kit == "log.end" then
      local st = ctx.peek(m.inst)
      if st then st.follow, st.last_y = true, nil end
   elseif kit == "table.sort" then
      return emit("on_sort", { field = m.field })
   elseif kit == "table.range" then
      local r = m.range
      if r then return emit("on_range", { first = r.first, last = r.last }) end
   elseif kit == "table.key" or kit == "list.key" then
      local k = m.key
      if not k or k.phase ~= "down" then return nil end
      if k.code == "tab" then
         local seen = ctx.peek(m.inst)
         if seen then seen.tab = k.shift == true and -1 or 1 end
         return nil
      end
      local n = (m.n or 0)
      if n == 0 then return nil end
      local s = step(k.code, m.selected, n, (m.page or 8))
      if s and s ~= m.selected then return emit("on_select", { row = s }) end
   end
   return nil
end

return M