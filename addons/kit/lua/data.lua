-- Generated from src/data.tl by addons/kit/build.lua. Edit the .tl.





local ctx = require("ctx")
local icons = require("icons")
local inputs = require("inputs")
local skin = require("skin")



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



















local function typeahead(c, seen, ch, field)
   seen.ch, seen.field = ch, field
   local text = seen.ask
   if not text then return end
   seen.ask = nil
   local want = text:lower()
   for i = 0, count(ch) - 1 do
      if cell_text(ch:get(i, field)):lower():sub(1, #want) == want then
         pcall(c.env.answer_typeahead, "rows", i)
         return
      end
   end
end



local TONE_ROLE = { ok = "ok", warn = "warn", danger = "danger", accent = "accent", info = "focus", neutral = "muted" }

local function tone_of(v)
   return type(v) == "string" and TONE_ROLE[v] or nil
end








local function pill(c, label, tone, dot, sel)
   local role = tone_of(tone)
   local accent = role == "accent"
   local ink = ref(c, accent and "on_accent" or (role or "muted"))
   local spot = accent and "on_accent" or (role == "muted" and "faint" or (role or "faint"))
   return row({
      height = 18, min_width = "fit", pad = { x = 6 }, gap = 5, cross_align = "center",
      radius = c.s.skin.radius > 0 and 9 or 0,
      bg = accent and ref(c, "accent") or ref(c, sel and "panel" or "press"),
      dot and column({ width = 6, height = 6, radius = 3, bg = ref(c, spot) }) or nil,
      ctx.data(c, label, { size = "$kitSizeLabel", bold = c.s.skin.label_bold, color = ink, max_lines = 1 }),
   })
end


































local function merged(ranges)
   local rs = {}
   for _, r in ipairs(ranges) do
      local a, b = math.min(r.first, r.last), math.max(r.first, r.last)
      rs[#rs + 1] = { first = a, last = b }
   end
   table.sort(rs, function(x, y) return x.first < y.first end)
   local out = {}
   for _, r in ipairs(rs) do
      local top = out[#out]
      if top and r.first <= top.last + 1 then
         if r.last > top.last then top.last = r.last end
      else
         out[#out + 1] = { first = r.first, last = r.last }
      end
   end
   return out
end

local function holds(ranges, i)
   for _, r in ipairs(ranges) do
      if i >= r.first and i <= r.last then return true end
      if r.first > i then return false end
   end
   return false
end


local function toggled(ranges, i)
   if not holds(ranges, i) then
      local rs = { { first = i, last = i } }
      for _, r in ipairs(ranges) do rs[#rs + 1] = r end
      return merged(rs)
   end
   local out = {}
   for _, r in ipairs(ranges) do
      if i >= r.first and i <= r.last then
         if r.first < i then out[#out + 1] = { first = r.first, last = i - 1 } end
         if i < r.last then out[#out + 1] = { first = i + 1, last = r.last } end
      else
         out[#out + 1] = r
      end
   end
   return out
end



local LISTED = 1000



local function proposed(st, row, ranges)
   st.ranges = ranges
   local count = 0
   for _, r in ipairs(ranges) do count = count + r.last - r.first + 1 end
   local rows = nil
   if count <= LISTED then
      rows = {}
      for _, r in ipairs(ranges) do
         for i = r.first, r.last do rows[#rows + 1] = i end
      end
   end
   local out = {}
   for _, r in ipairs(ranges) do out[#out + 1] = { first = r.first, last = r.last } end
   return emit("on_select", { row = row, ranges = out, count = count, rows = rows })
end



local function selection_of(c, p, n)
   local st = ctx.state(c, function() return {} end)
   local mode = str(p.selection) or "single"
   st.mode, st.n = mode, n
   st.on_focus = p.select_on_focus ~= false
   st.row = p.selected
   st.activates = p.on_activate ~= nil
   st.mac = c.env.platform == "macos"
   local given = (p.selected_ranges or {})
   st.ranges = mode == "multiple" and merged(given) or {}
   return st
end

local function selected_in(st, i)
   if st.mode == "single" then return st.row == i end
   if st.mode == "multiple" then return holds(st.ranges, i) end
   return false
end


local function entry_of(st)
   if st.cursor and st.cursor < st.n then return st.cursor end
   if st.mode == "single" then return st.row end
   if st.mode == "multiple" and st.ranges[1] then return st.ranges[1].first end
   return nil
end




local function on_cursor_moved(st, i, mods)
   local out = {}
   local moved = st.cursor ~= i
   st.cursor = i
   if moved then out[#out + 1] = emit("on_cursor", { row = i }) end
   local shift = mods ~= nil and mods.shift == true
   if st.mode == "single" and st.on_focus then
      out[#out + 1] = emit("on_select", { row = i })
   elseif st.mode == "multiple" then
      if shift then
         out[#out + 1] = proposed(st, i, { { first = math.min(st.anchor or i, i), last = math.max(st.anchor or i, i) } })
      elseif st.on_focus then
         st.anchor = i
         out[#out + 1] = proposed(st, i, { { first = i, last = i } })
      end
   end
   return out
end





local function on_row_pressed(st, i, click)
   local out = {}
   local key = click and click.key
   local mods = click and click.mods
   local clicks = click and (click.clicks) or 1
   if st.cursor ~= i then
      st.cursor = i
      out[#out + 1] = emit("on_cursor", { row = i })
   end
   if key == "enter" or clicks >= 2 then
      if st.activates then
         out[#out + 1] = emit("on_activate", { row = i })
         return out
      end
      if clicks >= 2 then return out end
   end
   if st.mode == "single" then
      out[#out + 1] = emit("on_select", { row = i })
   elseif st.mode == "multiple" then
      local primary = mods ~= nil and ((st.mac and mods.super == true) or (not st.mac and mods.ctrl == true))
      local shift = mods ~= nil and mods.shift == true
      if key == "space" or primary then
         st.anchor = i
         out[#out + 1] = proposed(st, i, toggled(st.ranges, i))
      elseif shift then
         out[#out + 1] = proposed(st, i, { { first = math.min(st.anchor or i, i), last = math.max(st.anchor or i, i) } })
      else
         st.anchor = i
         out[#out + 1] = proposed(st, i, { { first = i, last = i } })
      end
   end
   return out
end




local function on_row_key(st, k)
   local out = {}
   if st.mode ~= "multiple" or st.n == 0 then return out end
   local code = k.code
   local primary = (st.mac and k.super == true) or (not st.mac and k.ctrl == true)
   if (code == "a" or code == "A") and primary then
      out[#out + 1] = proposed(st, st.cursor or 0, { { first = 0, last = st.n - 1 } })
   end
   return out
end

function M.table(c, p)
   local ch = p.rows
   local cols = (p.columns or {})
   local n = count(ch)
   local row_h = c.s.density == "compact" and 20 or 22
   local seen = ctx.state(c, function() return {} end)
   local st = selection_of(c, p, n)
   local cells = p.cells == true
   typeahead(c, seen, ch, cols[1] and cols[1].field or "")







   local function need(col)
      return ((col.width or col.min_width or 80))
   end
   local function width_of(shown)
      local w, k = 20.0, 0
      for i, col in ipairs(cols) do
         if shown[i] then w, k = w + need(col), k + 1 end
      end
      return w + 10 * math.max(k - 1, 0) + 2
   end
   local shown = {}
   local hides = {}
   for i, col in ipairs(cols) do
      shown[i] = true
      if col.priority ~= nil then hides[#hides + 1] = i end
   end

   table.sort(hides, function(a, b)
      local pa, pb = cols[a].priority, cols[b].priority
      if pa ~= pb then return pa < pb end
      return a > b
   end)
   local head_rules = {}
   local cell_rules = {}
   local gone_h, gone_c = {}, {}
   for _, i in ipairs(hides) do
      local below = width_of(shown)
      shown[i] = false
      gone_h[#gone_h + 1], gone_c[#gone_c + 1] = "h" .. i, "c" .. i
      head_rules[#head_rules + 1] = { width = { below = below, band = 8 }, hidden = { table.unpack(gone_h) } }
      cell_rules[#cell_rules + 1] = { width = { below = below, band = 8 }, hidden = { table.unpack(gone_c) } }
   end
   local least = width_of(shown)



   local framed = p.border == true


   local sorts = p.on_sort ~= nil
   local selects = p.on_select ~= nil or p.on_cursor ~= nil or p.on_activate ~= nil
   local entry = selects and entry_of(st) or nil
   local keys = (selects and st.mode == "multiple") and act(c, "table.key") or nil
   local head = { width = "grow", height = row_h + 4, pad = { x = 10 }, gap = 10, cross_align = "center",
bg = ref(c, framed and "panel" or "sunken"), role = cells and "row" or nil,
when = #head_rules > 0 and head_rules or nil, }
   for i, col in ipairs(cols) do
      local field = col.field
      local sorted = p.sort == field
      local h = {
         key = "h" .. i,

         role = cells and "columnHeader" or (sorts and "button" or nil),
         focusable = (cells and selects) or nil,
         label = (col.title) .. (sorted and (p.descending == true and ", sorted descending" or ", sorted ascending") or ""),
         width = (col.width) or "grow",
         height = "grow",
         gap = 4,
         cross_align = "center",
         main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
         cursor = sorts and "pointer" or nil,
         on_click = sorts and act(c, "table.sort", { field = field }) or nil,
         ctx.caption(c, col.title, { color = ref(c, sorted and "fg" or "muted") }),
      }
      if sorted then h[#h + 1] = icons.node(c, p.descending == true and "chevron_down" or "chevron_up", 11, "fg") end
      head[#head + 1] = row(h)
   end

   local striped = p.striped == true
   local rows_t = {
      height = "grow",
      on_range = act(c, "table.range"),


      role = not cells and "list" or nil,
      label = "rows",
      active_index = not cells and entry or nil,
      on_typeahead = selects and act(c, "table.typeahead") or nil,
      on_item_key = not cells and keys or nil,
   }
   local view = body(c, "rows", n, row_h, rows_t, function(i)
      local sel = selected_in(st, i)
      local line = {
         index = i,
         role = cells and "row" or "listItem",
         focusable = (selects and not cells) or nil,
         on_focus = (selects and not cells) and act(c, "table.focus", { row = i }) or nil,
         selected = sel,
         height = row_h,
         width = "grow",
         pad = { x = 10 },
         gap = 10,
         cross_align = "center",
         bg = sel and ref(c, "sel") or ((striped and i % 2 == 1) and ref(c, "hover") or nil),
         hover_bg = selects and (sel and ref(c, "sel") or ref(c, "hover")) or nil,
         on_click = selects and act(c, "table.select", { row = i }) or nil,
         when = #cell_rules > 0 and cell_rules or nil,
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
            key = "c" .. j,
            role = cells and "cell" or nil,
            label = cells and s or nil,
            focusable = (cells and selects) or nil,
            on_focus = (cells and selects) and act(c, "table.focus", { row = i }) or nil,


            on_click = (cells and selects) and act(c, "table.select", { row = i }) or nil,
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
      role = cells and "table" or nil,
      label = cells and "table" or nil,
      active_index = cells and entry or nil,
      on_typeahead = (cells and selects) and act(c, "table.typeahead") or nil,
      on_item_key = cells and keys or nil,



      border = framed and { w = "$kitHair", color = ref(c, "rule") } or nil,
      pad = framed and "$kitHair" or nil,
      column({ key = "wide", width = "grow", height = p.grow == true and "grow" or nil, scroll_x = true,
column({ key = "sheet", width = "grow", min_width = least, height = p.grow == true and "grow" or nil,
row(head),
framed and ctx.rule(c, "rule", "$kitHair") or nil,
n == 0 and column({ width = "grow", pad = 16, ctx.muted(c, "No rows") }) or view, }), }),
   })
end

function M.list(c, p)
   local ch = p.rows
   local n = count(ch)
   local two = str(p.detail) ~= nil
   local row_h = two and 40 or (c.s.density == "compact" and 22 or 26)
   local seen = ctx.state(c, function() return {} end)
   local st = selection_of(c, p, n)
   typeahead(c, seen, ch, p.title)

   local selects = p.on_select ~= nil or p.on_cursor ~= nil or p.on_activate ~= nil
   local rows_t = {
      height = p.grow == true and "grow" or nil,
      role = "list",
      label = str(p.label),
      active_index = selects and entry_of(st) or nil,
      on_typeahead = selects and act(c, "list.typeahead") or nil,
      on_item_key = (selects and st.mode == "multiple") and act(c, "list.key") or nil,
   }
   return body(c, "rows", n, row_h, rows_t, function(i)
      local sel = selected_in(st, i)
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
      local item = {
         index = i,
         role = "listItem",
         focusable = selects or nil,
         on_focus = selects and act(c, "list.focus", { row = i }) or nil,
         label = badge ~= "" and (badge .. " " .. title) or title,
         selected = sel,
         height = row_h,
         width = "grow",
         pad = { x = 10 },
         gap = 8,
         cross_align = "center",
         bg = sel and ref(c, "sel") or nil,
         hover_bg = selects and (sel and ref(c, "sel") or ref(c, "hover")) or nil,
         on_click = selects and act(c, "list.select", { row = i }) or nil,
         badge ~= "" and row({ width = p.badge_width, min_width = "fit",
pill(c, badge, tone, false, sel), }) or nil,
         column(t),
      }
      return row(item)
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
   local t = {
      key = "jump",
      role = "button",
      label = "Jump to the latest line",
      float = { anchor = "parent", clip = true, at = { "end", "end" }, self = { "end", "end" }, dx = -12, dy = -8 },
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
   }
   return row(t)
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

         role = "listItem",
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
   if tone == "info" then return "focus" end
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
      when = fade and { ctx.still() } or nil,
      pad = { x = 6 },
      gap = 5,
      cross_align = "center",
      radius = c.s.skin.radius > 0 and 9 or 0,
      bg = ref(c, accent and "accent" or "sunken"),


      p.dot == true and column({ width = 6, height = 6, radius = 3,
bg = ref(c, tone == "neutral" and "faint" or (accent and "on_accent" or tone_ink(tone))), }) or nil,
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









local COPIED_S = 1.5

function M.code(c, p)
   local text = str(p.text) or ""
   local wrap = p.wrap == true
   local lines = p.max_lines
   local body = ctx.data(c, text, { wrap = wrap and "glyph" or "none", line_height = LINE })


   local inner = { key = "lines", width = "grow", selectable = true }
   if p.line_numbers == true and not wrap then


      local n = 1
      for _ in text:gmatch("\n") do n = n + 1 end
      local nums = {}
      for i = 1, n do nums[i] = tostring(i) end
      inner.selectable = nil
      inner[1] = row({ width = "grow", gap = 12,
column({ cross_align = "end",
ctx.data(c, table.concat(nums, "\n"), { line_height = LINE, color = ref(c, "faint") }), }),
column({ key = "text", width = "grow", min_width = 0, selectable = true, scroll_x = true, body }), })
   else
      inner[1] = body
      inner.scroll_x = not wrap
   end
   if lines then
      inner.max_height = lines * LINE
      inner.scroll_y = true
   end
   local frame = {
      width = "grow",
      role = str(p.label) and "group" or nil,
      label = str(p.label),
      bg = ref(c, "sunken"),
      radius = "$kitRadius",
      pad = 8,
      gap = 6,
   }
   if p.copy == true or str(p.label) then
      local head = { width = "grow", cross_align = "center", gap = 8 }
      if str(p.label) then head[#head + 1] = ctx.muted(c, p.label, { max_lines = 1, ellipsis = true }) end
      head[#head + 1] = ctx.grow()
      if p.copy == true then
         local env = c.env
         local st = ctx.state(c, function() return {} end)
         if st.ask then
            st.ask = false
            env.set_clipboard(text)
            st.at = env.now or 0
         end
         local said = st.at ~= nil and (env.now or 0) < st.at + COPIED_S
         if said then env.request_frame_at(st.at + COPIED_S) end
         head[#head + 1] = inputs.button(c, {
            key = "copy",
            label = said and "Copied" or "Copy",
            name = said and "Copied" or "Copy the code",
            icon = said and "check" or "copy",
            variant = said and "success" or "ghost",
         }, act(c, "code.copy"))
      end
      frame[#frame + 1] = row(head)
   end
   frame[#frame + 1] = column(inner)
   return column(frame)
end




local SHIMMER_WGSL = [[
// Kit's skeleton shimmer, and `kit.shader({ preset = "shimmer" })`: a soft
// band of light crossing a bar from left to right, once a period, over the
// bar's own colour. Kit registers it itself (env.add_fragment; build.lua
// embeds this file) and draws each skeleton bar as a `fragment` with:
//
//   params[0]    the bar's colour, straight RGBA 0 to 1 (the skin's well)
//   params[1]    the band's colour at its middle
//   params[2].x  the period in seconds, the same for every bar, so bars of
//                any width cross in step
//   params[2].y  1 to sweep, 0 to hold still (reduced motion)
//
// The band is 40% of the bar's width, between 40 and 200 logical px. The
// rounding is the node's: Kit gives the fragment the skin's radius.
fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let w = in.size.x / in.scale;
    let band = clamp(w * 0.4, 40.0, 200.0);
    let period = max(params[2].x, 0.1);
    let x = in.local.x / in.scale;
    let at = -band * 0.5 + fract(in.time / period) * (w + band);
    let k = (1.0 - smoothstep(0.0, band * 0.5, abs(x - at))) * params[2].y;
    return mix(params[0], params[1], k);
}
]]

local NOISE_WGSL = [[
// Kit's noise preset (`kit.shader({ preset = "noise" })`): gradient noise in
// three octaves drifting slowly, between two colours. Kit registers it
// itself (env.add_fragment) and fills its params from the skin:
//
//   params[0]    the first colour, straight RGBA 0 to 1 (the skin's well)
//   params[1]    the second (the skin's accent)
//   params[2].x  the drift, in cells a second
//   params[2].y  1 to drift, 0 to hold still (reduced motion)
//   params[2].z  a cell's size in logical px
fn kit_hash(p: vec2<f32>) -> vec2<f32> {
    let q = vec2<f32>(dot(p, vec2<f32>(127.1, 311.7)), dot(p, vec2<f32>(269.5, 183.3)));
    return fract(sin(q) * 43758.5453) * 2.0 - 1.0;
}

fn kit_noise(p: vec2<f32>) -> f32 {
    let i = floor(p);
    let f = fract(p);
    let u = f * f * (3.0 - 2.0 * f);
    let a = dot(kit_hash(i), f);
    let b = dot(kit_hash(i + vec2<f32>(1.0, 0.0)), f - vec2<f32>(1.0, 0.0));
    let c = dot(kit_hash(i + vec2<f32>(0.0, 1.0)), f - vec2<f32>(0.0, 1.0));
    let d = dot(kit_hash(i + vec2<f32>(1.0, 1.0)), f - vec2<f32>(1.0, 1.0));
    return mix(mix(a, b, u.x), mix(c, d, u.x), u.y);
}

fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let cell = max(params[2].z, 1.0);
    let p = in.local / in.scale / cell;
    let t = in.time * params[2].x * params[2].y;
    let n = kit_noise(p + vec2<f32>(t, t * 0.6)) * 0.6
        + kit_noise(p * 2.0 - vec2<f32>(t * 0.8, 0.0)) * 0.3
        + kit_noise(p * 4.0 + vec2<f32>(0.0, t)) * 0.1;
    let k = smoothstep(0.2, 0.8, clamp(n * 0.5 + 0.5, 0.0, 1.0));
    return mix(params[0], params[1], k);
}
]]



local SWEEP_MS = 1500


local function rgba(out, color)
   out[#out + 1] = ((color >> 24) & 255) / 255
   out[#out + 1] = ((color >> 16) & 255) / 255
   out[#out + 1] = ((color >> 8) & 255) / 255
   out[#out + 1] = (color & 255) / 255
end


local function half(c)
   local held = c.s.appearance
   if held then return held end
   local th = c.env.theme
   return th and th.appearance == "light" and "light" or "dark"
end



local function fragment_id(c, wgsl)
   local ok, id = pcall(c.env.add_fragment, wgsl)
   return ok and id or nil
end




local function preset_params(c, preset, moving)
   local h = half(c)
   local params = {}
   if preset == "noise" then
      rgba(params, skin.color(c.s, "kitSunken", h))
      rgba(params, skin.color(c.s, "kitAccent", h))
      params[9], params[10], params[11] = 0.12, moving and 1 or 0, 56
   else
      rgba(params, skin.color(c.s, "kitSunken", h))
      rgba(params, skin.color(c.s, "kitShimmer", h))
      params[9], params[10] = SWEEP_MS / 1000, moving and 1 or 0
   end
   return params
end






function M.shader(c, p)
   local label = str(p.label)
   local t = {
      key = "shader",
      role = label and "image" or nil,
      label = label,
      height = p.grow == true and "grow" or (p.height or 80),
      radius = "$kitRadius",
   }
   local pw = p.width
   t.width = type(pw) == "number" and pw or "grow"
   local moving = not ctx.reduced(c)
   local preset = str(p.preset)
   local id = nil
   local raw = p.id
   if type(raw) == "number" then
      id = math.tointeger(raw)
   elseif str(p.source) then
      id = fragment_id(c, p.source)
   elseif preset then
      id = fragment_id(c, preset == "noise" and NOISE_WGSL or SHIMMER_WGSL)
   end
   if id then
      t.id = id


      t.params = p.params or (preset and not p.source and not raw and preset_params(c, preset, moving) or nil)
      local img = p.image
      t.image = type(img) == "number" and math.tointeger(img) or nil
      t.animate = (p.animate == true or (preset ~= nil and p.animate ~= false)) and moving or nil
      return fragment(t)
   end
   t.bg = ref(c, "sunken")
   return column(t)
end






local function bone(c, key, w, h, radius, travel, shimmer, moving)
   if shimmer then
      return fragment({ key = key, id = shimmer, width = w, height = h, radius = radius,
params = preset_params(c, "shimmer", moving), animate = moving or nil, })
   end
   local t = { key = key, width = w, height = h, radius = radius, bg = ref(c, "sunken"), clip = true }
   if travel > 0 then
      local band = math.max(40, math.min(200, travel * 0.4))
      t[1] = column({ width = band, height = h, float = { anchor = "parent", dx = -band, clip = true },
gradient = { to = "right", stops = { "$kitShimmerClear", "$kitShimmer", "$kitShimmerClear" } },
keyframes = { { dx = 0 }, { dx = travel + band } }, transition = SWEEP_MS, easing = "linear", })
   end
   return column(t)
end





function M.skeleton(c, p)
   local shape = str(p.shape) or "text"
   local moving = not ctx.reduced(c)
   local shimmer = fragment_id(c, SHIMMER_WGSL)
   local t = { key = "skeleton", role = "image", label = str(p.label) or "Loading" }
   if shape == "circle" then
      local d = (p.width or 32)
      t.width, t.height = d, d
      t[1] = bone(c, "b1", d, d, d / 2, d, shimmer, moving)
      return column(t)
   end


   t.on_layout = not shimmer and act(c, "skeleton.layout") or nil
   local pw = p.width
   t.width = type(pw) == "number" and pw or "grow"
   local w = 0.0
   if not shimmer then
      local r = c.env.layout_of("skeleton")
      w = r and r.w or 0
   end
   if shape == "block" then
      local h = (p.height or 80)
      t[1] = bone(c, "b1", "grow", h, "$kitRadius", w, shimmer, moving)
      return column(t)
   end
   local n = math.max(1, math.floor((p.lines or 1)))
   t.gap = 6
   for i = 1, n do
      local last = n > 1 and i == n
      local width = last and { pct = 60 } or "grow"
      t[i] = bone(c, "b" .. i, width, 10, "$kitRadius", last and w * 0.6 or w, shimmer, moving)
   end
   return column(t)
end

local NOTICE_ROLE = { danger = "danger", warn = "warn", ok = "ok", accent = "accent_text", info = "focus", neutral = "muted" }

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
      border = { w = "$kitHair", color = ref(c, role == "muted" and "faint" or role) },
      icons.node(c, tone == "ok" and "check" or (tone == "neutral" or tone == "accent" or tone == "info") and "info" or "alert", 14, role),
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
transition = 320, easing = "snappy", when = { ctx.still() }, }
   else





      local r = c.env.layout_of("track")
      local w = r and r.w or 400
      bar = {
         width = { pct = 30 }, height = "grow", bg = ref(c, "accent"),


         keyframes = { { dx = -0.3 * w }, { dx = w } },
         transition = math.max(1400, w * 2.5),
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
      key = "track",
      on_layout = not v and act(c, "progress.layout") or nil,
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










local function inline(c, s, bold)
   local spans = {}
   local function plain(t, b, it)
      if t == "" then return end
      if b or it then
         spans[#spans + 1] = { t, bold = (b or bold) or nil, italic = it or nil }
      elseif bold then
         spans[#spans + 1] = { t, bold = true }
      else
         spans[#spans + 1] = t
      end
   end
   local at = 1
   local n = #s
   local buf = ""
   while at <= n do
      local ch = s:sub(at, at)
      if ch == "`" then
         local e = s:find("`", at + 1, true)
         if e then
            plain(buf, false, false)
            buf = ""
            spans[#spans + 1] = { s:sub(at + 1, e - 1), family = "$kitData", bold = bold or nil }
            at = e + 1
         else
            buf = buf .. ch
            at = at + 1
         end
      elseif s:sub(at, at + 1) == "**" then
         local e = s:find("**", at + 2, true)
         if e then
            plain(buf, false, false)
            buf = ""
            plain(s:sub(at + 2, e - 1), true, false)
            at = e + 2
         else
            buf = buf .. "**"
            at = at + 2
         end
      elseif (ch == "*" or ch == "_") and s:sub(at + 1, at + 1) ~= " " and s:sub(at + 1, at + 1) ~= "" then
         local e = s:find(ch, at + 1, true)

         local inword = ch == "_" and at > 1 and s:sub(at - 1, at - 1):match("%w") ~= nil
         if e and e > at + 1 and not inword then
            plain(buf, false, false)
            buf = ""
            plain(s:sub(at + 1, e - 1), false, true)
            at = e + 1
         else
            buf = buf .. ch
            at = at + 1
         end
      elseif ch == "[" then
         local label, close = s:match("^%[([^%]]*)%]%(([^%)]*)%)", at)
         if label then
            buf = buf .. label
            at = at + #label + #close + 4
         else
            buf = buf .. ch
            at = at + 1
         end
      else
         buf = buf .. ch
         at = at + 1
      end
   end
   plain(buf, false, false)
   return spans
end

local function prose(c, s, o)
   local t = { size = "$kitSize", family = "$kitUi", color = ref(c, "fg"), wrap = "word" }
   if o then for k, v in pairs(o) do t[k] = v end end
   return text(inline(c, s, t.bold == true), t)
end


local function cells(line)
   local s = line:match("^%s*|?(.-)|?%s*$")
   local out = {}
   for cell in (s .. "|"):gmatch("([^|]*)|") do out[#out + 1] = cell:match("^%s*(.-)%s*$") end
   return out
end

local HEADING_SIZE = { 1.6, 1.3, 1.12, 1.0 }








function M.markdown(c, p)
   local src = (str(p.text) or ""):gsub("\r\n", "\n")
   local lines = {}
   for l in (src .. "\n"):gmatch("([^\n]*)\n") do lines[#lines + 1] = l end
   local base = c.s.skin.size


   local out = { width = "grow", gap = 10 }
   local seg = { width = "grow", gap = 10, selectable = true }
   local function close_seg()
      if #seg > 0 then out[#out + 1] = column(seg) end
      seg = { width = "grow", gap = 10, selectable = true }
   end
   local i = 1
   local para = {}
   local function flush()
      if #para > 0 then
         seg[#seg + 1] = prose(c, table.concat(para, " "))
         para = {}
      end
   end
   while i <= #lines do
      local l = lines[i]
      local fence = l:match("^%s*```")
      local level, head = l:match("^(#+)%s+(.*)$")
      if fence then
         flush()
         local body = {}
         i = i + 1
         while i <= #lines and not lines[i]:match("^%s*```") do
            body[#body + 1] = lines[i]
            i = i + 1
         end
         close_seg()
         out[#out + 1] = M.code(c, { text = table.concat(body, "\n") })
      elseif level and #level <= 6 then
         flush()
         local k = math.min(#level, #HEADING_SIZE)
         seg[#seg + 1] = row({ width = "grow", pad = { t = k <= 2 and 6 or 2 },
prose(c, head, { size = math.floor(base * HEADING_SIZE[k] + 0.5), bold = true }), })
         if k == 1 then seg[#seg + 1] = ctx.rule(c, "rule", "$kitHair") end
      elseif l:match("^%s*%-%-%-+%s*$") or l:match("^%s*%*%*%*+%s*$") then
         flush()
         seg[#seg + 1] = ctx.rule(c, "rule", "$kitHair")
      elseif l:match("^%s*|") then
         flush()
         local tbl = { width = "grow", gap = 4 }
         local first = true
         while i <= #lines and lines[i]:match("^%s*|") do
            local cs = cells(lines[i])
            local rule = lines[i]:match("^[%s|:%-]+$") ~= nil
            if not rule then
               local r = { width = "grow", gap = 16 }
               for _, cell in ipairs(cs) do
                  r[#r + 1] = column({ min_width = 0, width = "grow",
prose(c, cell, { size = "$kitSizeLabel", bold = first or nil }), })
               end
               tbl[#tbl + 1] = row(r)
               if first then tbl[#tbl + 1] = ctx.rule(c, "rule", "$kitHair") end
               first = false
            end
            i = i + 1
         end
         seg[#seg + 1] = grid(tbl)
         i = i - 1
      elseif l:match("^%s*>") then
         flush()
         local q = {}
         while i <= #lines and lines[i]:match("^%s*>") do
            q[#q + 1] = lines[i]:match("^%s*>%s?(.*)$")
            i = i + 1
         end
         i = i - 1
         seg[#seg + 1] = row({ width = "grow", gap = 10,
ctx.rule(c, "rule", 2, true),
column({ width = "grow", min_width = 0, prose(c, table.concat(q, " "), { color = ref(c, "muted") }) }), })
      elseif l:match("^%s*[%-%*]%s+") or l:match("^%s*%d+[.)]%s+") then
         flush()
         local items = {}
         while i <= #lines do
            local li = lines[i]
            local indent, mark, body = li:match("^(%s*)([%-%*])%s+(.*)$")
            if not mark then indent, mark, body = li:match("^(%s*)(%d+[.)])%s+(.*)$") end
            if mark then
               items[#items + 1] = { depth = #indent // 2, mark = mark:match("^%d") and mark or "•", text = body }
               i = i + 1
            else

               local more = li:match("^%s%s+(%S.*)$")
               if not more or #items == 0 then break end
               items[#items].text = items[#items].text .. " " .. more
               i = i + 1
            end
         end
         i = i - 1
         local list = { width = "grow", gap = 4 }
         for _, it in ipairs(items) do
            list[#list + 1] = row({ width = "grow", gap = 6, pad = { l = 14 * it.depth },
column({ width = 14, ctx.muted(c, it.mark, { size = "$kitSize" }) }),
column({ width = "grow", min_width = 0, prose(c, it.text) }), })
         end
         seg[#seg + 1] = column(list)
      elseif l:match("^%s*$") then
         flush()
      else
         para[#para + 1] = l:match("^%s*(.-)%s*$")
      end
      i = i + 1
   end
   flush()
   close_seg()
   return column(out)
end

function M.event(m, kit)
   if kit == "log.select" then
      return emit("on_select", { row = m.row })
   elseif kit == "table.select" or kit == "list.select" then

      local st = ctx.peek(m.inst)
      if not st or not st.mode then return emit("on_select", { row = m.row }) end
      return on_row_pressed(st, m.row, m.click)
   elseif kit == "table.focus" or kit == "list.focus" then




      local f = m.focus
      if f and f.phase == "in" and f.by ~= "pointer" then
         local st = ctx.peek(m.inst)
         if not st or not st.mode then return emit("on_select", { row = m.row }) end
         return on_cursor_moved(st, m.row, f.mods)
      end
   elseif kit == "table.key" or kit == "list.key" then
      local st = ctx.peek(m.inst)
      local k = m.itemkey
      if st and st.mode and k then return on_row_key(st, k) end
   elseif kit == "table.typeahead" or kit == "list.typeahead" then
      local seen = ctx.peek(m.inst)
      local t = m.typeahead
      if seen and t then seen.ask = t.text end
   elseif kit == "log.end" then
      local st = ctx.peek(m.inst)
      if st then st.follow, st.last_y = true, nil end
   elseif kit == "code.copy" then
      local st = ctx.peek(m.inst)
      if st then st.ask = true end
   elseif kit == "table.sort" then
      return emit("on_sort", { field = m.field })
   elseif kit == "table.range" then
      local r = m.range
      if r then return emit("on_range", { first = r.first, last = r.last }) end
   end
   return nil
end

return M