-- Generated from src/datatable.tl by addons/kit/build.lua. Edit the .tl.









local skin = require("skin")
local ctx = require("ctx")





local Column = {}












local Params = {}









local M = { Params = Params, Column = Column }


local held = {}

function M.rows(c, p)
   local d = p.data
   local h = held[c.id]
   if d and d.rows and (not h or h.rev ~= d.rev) then
      h = { rev = d.rev, rows = d.rows }
      held[c.id] = h
   end
   return h and h.rows or {}
end

local function cell(col, s, last, color, bold)
   local box = {
      width = last and "grow" or col.width,
      main_align = col.align == "end" and "end" or "start",
      text(s, {
         size = bold and 10.5 or 12,
         bold = bold,
         color = color,
         family = (col.mono and not bold) and "$kitData" or "$kitUi",
         max_lines = 1,
         ellipsis = true,
      }),
   }
   return row(box)
end

function M.view(p, c)
   local rows = M.rows(c, p)
   local cols = p.columns or {}
   local row_h = skin.row_h(c.settings)
   local env = c.env

   local head = { width = "grow", height = row_h + 2, pad = { x = 12 }, gap = 10, cross_align = "center" }
   for i, col in ipairs(cols) do
      head[#head + 1] = cell(col, skin.label(c.skin, col.title), i == #cols, "$kitFg", true)
   end



   local key = "rows"
   local n = #rows
   if p.scroll_top then env.set_scroll(key, 0, p.scroll_top) end
   local g = env.scroll_geometry(key)
   local vh = g and g.h or env.viewport_h
   local top = g and g.offset.y or 0
   local first = math.min(n, math.max(0, math.floor(top / row_h) - 2))
   local last = math.min(n, math.ceil((top + math.max(vh, 0)) / row_h) + 2)
   if last < first then last = first end

   local body = { key = key, width = "grow", height = "grow", scroll_y = true, row_count = n }
   if first > 0 then
      body[#body + 1] = column({ key = "kit:lead", width = "grow", height = first * row_h })
   end
   for i = first, last - 1 do
      local r = rows[i + 1]
      local sel = p.selected == i
      local line = {
         index = i,
         label = r and table.concat(r, " · ") or "…",
         height = row_h,
         width = "grow",
         pad = { x = 12 },
         gap = 10,
         cross_align = "center",
         hover_bg = "$kitHover",
         on_click = { kit = "select", id = c.id, index = i },
      }
      if sel then line.bg = "$kitSel" end
      local ink = sel and "$kitSelFg" or "$kitFg"
      for j, col in ipairs(cols) do
         line[#line + 1] = cell(col, r[j] or "", j == #cols, ink, false)
      end
      body[#body + 1] = row(line)
   end
   if last < n then
      body[#body + 1] = column({ key = "kit:tail", width = "grow", height = (n - last) * row_h })
   end

   return column({
      width = "grow",
      height = "grow",
      bg = "$kitPanel",
      row(head),

      column({ width = "grow", height = "$kitRuleW", bg = "$kitInk" }),
      column(body),
   })
end

function M.event(ev, p)
   local m = ev
   if m.kit == "select" then
      return ctx.emit(p.on_select, { index = m.index })
   end
   return nil
end

return M