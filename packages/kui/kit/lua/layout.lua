-- Generated from src/layout.tl by addons/kit/build.lua. Edit the .tl.




local ctx = require("ctx")
local icons = require("icons")



local ref, act = ctx.ref, ctx.act

local M = {}

local function str(v)
   return type(v) == "string" and v or nil
end



local function region(c, t)
   t.bg = ref(c, "panel")
   t.radius = "$kitRadius"
   t.clip = true
   t.border = { w = "$kitRuleW", color = ref(c, c.s.skin.ctl_border and "ink" or "rule") }
   return t
end

function M.panel(c, p)
   local grow = p.grow == true
   local t = region(c, { width = "grow", height = grow and "grow" or nil, label = str(p.title), role = "group" })
   if str(p.title) then
      local head = {
         width = "grow", height = "$kitCtl", pad = { x = 10 }, gap = 8, cross_align = "center",
         ctx.caption(c, p.title, { size = "$kitSizeLabel", bold = true }),
      }
      if str(p.subtitle) then head[#head + 1] = ctx.muted(c, p.subtitle, { max_lines = 1, ellipsis = true }) end
      head[#head + 1] = ctx.grow()
      head[#head + 1] = row({ gap = 4, cross_align = "center", children("actions") })
      t[#t + 1] = row(head)
      t[#t + 1] = ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", c.s.skin.ctl_border and "$kitRuleW" or "$kitHair")
   end
   t[#t + 1] = column({
      key = "body",
      width = "grow",
      height = (grow or p.scroll == true) and "grow" or nil,
      scroll_y = p.scroll == true,
      pad = (p.pad) or 10,
      gap = 8,
      children(""),
   })
   t[#t + 1] = column({
      width = "grow",
      children("footer"),
   })
   return column(t)
end

function M.toolbar(c, _p)
   return column({
      width = "grow",
      bg = ref(c, "side"),
      row({
         width = "grow",
         height = 38,
         pad = { x = 8 },
         gap = 6,
         cross_align = "center",
         children(""),
         ctx.grow(),
         row({ gap = 6, cross_align = "center", children("trailing") }),
      }),
      ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair"),
   })
end

function M.statusbar(c, _p)
   return column({
      width = "grow",
      bg = ref(c, "side"),
      ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair"),
      row({
         width = "grow", height = 22, pad = { x = 10 }, gap = 14, cross_align = "center",
         children(""),
      }),
   })
end

function M.split(c, p)
   local column_axis = p.axis == "column"
   local f = math.max(0, math.min(1, (p.fraction or 0.5)))


   local line = { bg = ref(c, c.s.skin.ctl_border and "ink" or "rule") }
   local handle = {
      key = "rule",
      role = "none",
      keep_focus = true,
      main_align = "center",
      cross_align = "center",
      hover_bg = ref(c, "hover"),
      cursor = column_axis and "nsResize" or "ewResize",
      on_drag = act(c, "split.drag", { column = column_axis, min = (p.min or 80) }),
   }
   local first = { key = "first", clip = true, children("first") }
   local second = { key = "second", clip = true, children("second") }
   if column_axis then
      handle.width, handle.height = "grow", 6
      line.width, line.height = "grow", "$kitRuleW"
      handle[1] = column(line)
      first.width, first.height = "grow", { pct = f * 100 }
      second.width, second.height = "grow", "grow"
      return column({ width = "grow", height = "grow", column(first), column(handle), column(second) })
   end
   handle.width, handle.height = 6, "grow"
   line.width, line.height = "$kitRuleW", "grow"
   handle[1] = column(line)
   first.width, first.height = { pct = f * 100 }, "grow"
   second.width, second.height = "grow", "grow"
   return row({ width = "grow", height = "grow", column(first), row(handle), column(second) })
end

function M.tabs(c, p)
   local tabs = (p.tabs or {})
   local strip = { role = "tabList", width = "grow", height = "$kitCtl", gap = 2, cross_align = "end" }
   for i, tb in ipairs(tabs) do
      local here = tb.value == p.value
      local t = {
         key = "t" .. i,
         role = "tab",
         label = tb.label,
         selected = here,
         height = "grow",
         pad = { x = 10 },
         gap = 6,
         cross_align = "center",
         cursor = "pointer",
         hover_bg = not here and ref(c, "hover") or nil,
         on_click = act(c, "tabs.change", { value = tb.value }),
         ctx.caption(c, tb.label, { color = ref(c, here and "fg" or "muted") }),
      }
      if tb.count ~= nil then
         t[#t + 1] = ctx.data(c, tostring(tb.count), { size = "$kitSizeLabel", color = ref(c, "faint") })
      end
      strip[#strip + 1] = column({
         key = "tab" .. i,
         height = "grow",
         row(t),
         column({ width = "grow", height = "$kitRuleW", bg = here and ref(c, "accent") or nil }),
      })
   end
   return column({
      width = "grow",
      height = "grow",
      row(strip),
      ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair"),
      column({ key = "body", width = "grow", height = "grow", children("") }),
   })
end





function M.section(c, p)

   local st = ctx.state(c, function() return { collapsed = false } end)
   local collapsed
   if p.collapsed ~= nil then collapsed = p.collapsed == true else collapsed = st.collapsed end
   local head = {
      role = "button",
      label = str(p.title),
      expanded = collapsed and "collapsed" or "expanded",
      width = "grow",
      height = "$kitRow",
      gap = 6,
      cross_align = "center",
      cursor = "pointer",
      hover_bg = ref(c, "hover"),
      on_click = act(c, "section.toggle", { collapsed = not collapsed }),
      icons.node(c, collapsed and "chevron_right" or "chevron_down", 12, "muted"),
      ctx.caption(c, str(p.title) or "", { size = "$kitSizeLabel" }),
   }
   if p.count ~= nil then head[#head + 1] = ctx.data(c, tostring(p.count), { size = "$kitSizeLabel", color = ref(c, "faint") }) end
   local t = { width = "grow", gap = 4, row(head) }
   if not collapsed then
      t[#t + 1] = column({ key = "body", width = "grow", gap = 6, pad = { l = 18 }, children("") })
   end
   return column(t)
end

function M.divider(c, _p)
   return ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair")
end

function M.event(m, kit)
   if kit == "split.drag" then
      local d = m.drag
      local parent = d and d.parent
      if not parent then return nil end
      local column_axis = m.column == true
      local at = (column_axis and d.y or d.x)
      local start = (column_axis and parent.y or parent.x)
      local size = (column_axis and parent.h or parent.w)
      if size <= 0 then return nil end
      local min = math.min((m.min or 80), size / 2)
      local px = math.max(min, math.min(size - min, at - start))
      return emit("on_resize", { fraction = px / size })
   elseif kit == "tabs.change" then
      return emit("on_change", { value = m.value })
   elseif kit == "section.toggle" then
      local st = ctx.peek(m.inst)
      if st then st.collapsed = m.collapsed == true end
      return emit("on_toggle", { collapsed = m.collapsed == true })
   end
   return nil
end

return M