-- Generated from src/inputs.tl by addons/kit/build.lua. Edit the .tl.





local ctx = require("ctx")
local icons = require("icons")



local ref, act = ctx.ref, ctx.act

local M = {}




local MENU_FLOAT = { anchor = "below", at = { "start", "end" }, self = { "start", "start" }, fit = true }

local function str(v)
   return type(v) == "string" and v or nil
end



local function look(c, variant)
   local bordered = c.s.skin.ctl_border
   if variant == "primary" then
      return ref(c, "accent"), ref(c, "accent"), ref(c, "on_accent"), bordered
   elseif variant == "danger" then
      return ref(c, "danger"), ref(c, "danger"), ref(c, "on_accent"), bordered
   elseif variant == "ghost" then
      return nil, ref(c, "hover"), ref(c, "fg"), false
   elseif variant == "success" then

      return bordered and ref(c, "panel") or ref(c, "sunken"), ref(c, "hover"), ref(c, "ok"), bordered
   end
   return bordered and ref(c, "panel") or ref(c, "sunken"), ref(c, "hover"), ref(c, "fg"), bordered
end



local function pressable(c, variant, t)
   local bg, hover, _, border = look(c, variant)
   t.role = t.role or "button"
   t.height = t.height or "$kitCtl"
   t.cross_align = "center"
   t.radius = "$kitRadius"
   t.bg = bg
   t.hover_bg = hover
   t.pressed_bg = ref(c, "press")
   t.cursor = "pointer"
   if border then
      local edge = (variant == "danger" or variant == "success") and (variant == "danger" and "danger" or "ok") or "ink"
      t.border = { w = "$kitRuleW", color = ref(c, edge) }
   end
   if variant == "primary" then ctx.shadow(c, t, true) end
   return t
end


local function pressed_look(c, t)
   t.selected = true
   t.bg = ref(c, "sel")
   t.hover_bg = ref(c, "sel")
   return ref(c, "sel_fg")
end

function M.button(c, p)
   local variant = str(p.variant) or "default"
   local _, _, ink, _ = look(c, variant)
   local label = str(p.label) or ""
   local t = pressable(c, variant, {
      label = str(p.name) or label,
      tooltip = str(p.tooltip),
      disabled = p.disabled == true,
      pad = { x = 10 },
      gap = 6,
      on_click = act(c, "button.press"),
   })
   if p.pressed == true then ink = pressed_look(c, t) end
   if p.disabled == true then t.opacity = "$disabledOpacity" end
   local icon_role = p.pressed == true and "sel_fg" or (variant == "primary" and "on_accent" or (variant == "success" and "ok" or "fg"))
   if str(p.icon) then t[#t + 1] = icons.node(c, p.icon, 13, icon_role) end
   t[#t + 1] = ctx.caption(c, label, { color = ink, bold = variant == "primary" or c.s.skin.label_bold })
   if str(p.shortcut) then
      t[#t + 1] = ctx.data(c, p.shortcut, { size = "$kitSizeLabel", color = ref(c, variant == "primary" and "on_accent" or "faint") })
   end
   return row(t)
end

function M.icon_button(c, p)
   local variant = str(p.variant) or "ghost"
   local label = str(p.label) or ""
   local t = pressable(c, variant, {
      label = label,
      tooltip = str(p.tooltip) or label,
      disabled = p.disabled == true,
      width = "$kitCtl",
      main_align = "center",
      on_click = act(c, "icon_button.press"),
   })
   local role = variant == "primary" and "on_accent" or (variant == "success" and "ok" or "fg")
   if p.pressed == true then
      pressed_look(c, t)
      role = "sel_fg"
   end
   if p.disabled == true then t.opacity = "$disabledOpacity" end
   t[#t + 1] = icons.node(c, str(p.icon) or "", 14, role)
   return row(t)
end


local function well(c, t, focused)
   if c.s.skin.field_box then
      t.bg = ref(c, "sunken")
      t.radius = "$kitRadius"
      t.border = { w = "$kitHair", color = ref(c, focused and "focus" or "rule") }
   else
      t.border = nil
   end
end


local function underline(c, focused)
   if c.s.skin.field_box then return nil end
   return ctx.rule(c, focused and "accent" or "ink", focused and "$kitRuleW" or "$kitHair")
end








local function focus_request(c, p)
   local st = ctx.state(c, function() return { seen = false } end)
   local want = p.focus
   if want ~= nil and st.seen and want ~= st.focus then
      c.env.set_focus("edit")
   end
   st.focus, st.seen = want, true
end

local function labelled(c, label, body, width)
   if not label then return body end
   return column({ width = (width) or "grow", gap = 3, ctx.muted(c, ctx.label(c, label), { bold = true }), body })
end

function M.text_field(c, p)
   focus_request(c, p)
   local focused = c.env.is_focused("edit")
   local value = str(p.value) or ""
   local box = {
      height = "$kitCtl",
      width = "grow",
      pad = { x = c.s.skin.field_box and 8 or 2 },
      cross_align = "center",
      edit({
         key = "edit",
         value = value,
         placeholder = str(p.placeholder),
         label = str(p.label) or str(p.placeholder),
         width = "grow",
         size = "$kitSize",
         family = p.mono == true and "$kitData" or "$kitUi",
         color = ref(c, "fg"),
         disabled = p.disabled == true,
         on_change = act(c, "text_field.change"),
         on_submit = act(c, "text_field.submit", { text = value }),
      }),
   }
   well(c, box, focused)
   local field = column({ width = (p.width) or "grow", gap = 0, row(box), underline(c, focused) })
   if p.hide_label == true then return field end
   return labelled(c, str(p.label), field, p.width)
end

function M.search(c, p)
   focus_request(c, p)
   local focused = c.env.is_focused("edit")
   local value = str(p.value) or ""
   local box = {
      height = "$kitCtl",
      width = "grow",
      gap = 6,
      pad = { x = c.s.skin.field_box and 8 or 2 },
      cross_align = "center",
      icons.node(c, "search", 13, "muted"),
      edit({
         key = "edit",
         value = value,
         placeholder = str(p.placeholder) or "Search",
         label = str(p.placeholder) or "Search",
         width = "grow",
         size = "$kitSize",
         family = "$kitUi",
         color = ref(c, "fg"),
         on_change = act(c, "search.change"),
         on_submit = act(c, "search.submit", { text = value }),
         on_cancel = act(c, "search.clear"),
      }),
   }
   if str(p.count) then box[#box + 1] = ctx.data(c, p.count, { size = "$kitSizeLabel", color = ref(c, "muted") }) end
   if value ~= "" then
      box[#box + 1] = row({
         role = "button", label = "Clear", width = 18, height = 18, main_align = "center",
         cross_align = "center", radius = "$kitRadius", hover_bg = ref(c, "hover"), cursor = "pointer",
         on_click = act(c, "search.clear"),
         icons.node(c, "x", 12, "muted"),
      })
   end
   well(c, box, focused)
   return column({ width = "grow", row(box), underline(c, focused) })
end


local function chosen(options, value)
   for _, o in ipairs(options) do
      if o.value == value then return o.label end
   end
   return nil
end





function M.select(c, p)
   local options = (p.options or {})
   local st = ctx.state(c, function() return { open = false } end)
   local current = chosen(options, p.value)
   local field = pressable(c, "default", {
      label = (str(p.label) or "choice") .. ": " .. (current or "none"),
      expanded = st.open and "expanded" or "collapsed",
      width = (p.width) or "grow",
      pad = { x = 8 },
      gap = 6,
      on_click = act(c, "select.open"),
      ctx.text(c, current or str(p.placeholder) or "Choose…", {
         color = ref(c, current and "fg" or "faint"), max_lines = 1, ellipsis = true,
      }),
      ctx.grow(),
      icons.node(c, st.open and "chevron_up" or "chevron_down", 12, "muted"),
   })
   if st.open then
      local list = {
         key = "menu",
         role = "menu",
         float = MENU_FLOAT,
         modal = act(c, "select.dismiss"),
         min_width = 160,
         pad = 4,
         gap = 1,
         bg = ref(c, "raised"),
         radius = "$kitRadius",
         border = { w = "$kitHair", color = ref(c, "ink") },
      }
      ctx.shadow(c, list)
      for i, o in ipairs(options) do
         local here = o.value == p.value
         list[#list + 1] = row({
            key = "o" .. i,
            role = "menuItem",
            label = o.label,
            selected = here,
            disabled = o.disabled == true,
            initial_focus = here,
            height = "$kitRow",
            width = "grow",
            pad = { x = 8 },
            gap = 6,
            cross_align = "center",
            radius = "$kitRadius",
            bg = here and ref(c, "sel") or nil,
            hover_bg = here and ref(c, "sel") or ref(c, "hover"),
            cursor = "pointer",
            opacity = o.disabled == true and "$disabledOpacity" or nil,
            on_click = act(c, "select.pick", { value = o.value }),
            ctx.text(c, o.label, { color = ref(c, here and "sel_fg" or "fg") }),
         })
      end


      return labelled(c, str(p.label), column({ width = (p.width) or "grow", row(field), column(list) }), p.width)
   end
   return labelled(c, str(p.label), row(field), p.width)
end


local function tickbox(c, on, mixed, round)
   local filled = on or mixed
   local t = {
      width = 14, height = 14, main_align = "center", cross_align = "center",
      radius = round and 7 or (c.s.skin.radius > 0 and 3 or 0),
      bg = filled and not round and ref(c, "accent") or ref(c, "panel"),
      border = { w = c.s.skin.ctl_border and "$kitRuleW" or "$kitHair", color = ref(c, filled and "accent" or "ink") },
   }
   if round and on then
      t[1] = column({ width = 6, height = 6, radius = 3, bg = ref(c, "accent") })
   elseif filled then
      t[1] = icons.node(c, mixed and "minus" or "check", 11, "on_accent")
   end
   return row(t)
end

function M.checkbox(c, p)
   local on, mixed = p.checked == true, p.mixed == true
   return row({
      role = "checkbox",
      label = str(p.label),
      checked = on,
      mixed = mixed,
      disabled = p.disabled == true,
      opacity = p.disabled == true and "$disabledOpacity" or nil,
      height = "$kitCtl",
      gap = 8,
      cross_align = "center",
      cursor = "pointer",
      on_click = act(c, "checkbox.toggle", { checked = not on }),
      tickbox(c, on, mixed, false),
      ctx.text(c, str(p.label) or ""),
   })
end

function M.switch(c, p)
   local on = p.on == true
   local track = {
      width = 30, height = 16, radius = 8, pad = 2,
      main_align = on and "end" or "start",
      cross_align = "center",
      bg = ref(c, on and "accent" or "sunken"),
      border = { w = "$kitHair", color = ref(c, on and "accent" or "ink") },

      column({ key = "knob", width = 12, height = 12, radius = 6, transition = 0.12,
bg = ref(c, on and "on_accent" or "muted"), }),
   }
   if c.s.skin.radius == 0 then
      track.radius = 0
      track[1] = column({ key = "knob", width = 12, height = 12, transition = 0.12, bg = ref(c, on and "on_accent" or "muted") })
   end
   return row({
      role = "switch",
      label = str(p.label),
      checked = on,
      disabled = p.disabled == true,
      opacity = p.disabled == true and "$disabledOpacity" or nil,
      height = "$kitCtl",
      gap = 8,
      cross_align = "center",
      cursor = "pointer",
      on_click = act(c, "switch.toggle", { on = not on }),
      row(track),
      ctx.text(c, str(p.label) or ""),
   })
end

function M.radio_group(c, p)
   local options = (p.options or {})
   local t = { role = "radioGroup", label = str(p.label), gap = 2 }
   if str(p.label) then t[#t + 1] = ctx.muted(c, ctx.label(c, p.label), { bold = true }) end
   for i, o in ipairs(options) do
      local here = o.value == p.value
      t[#t + 1] = row({
         key = "r" .. i,
         role = "radio",
         label = o.label,
         checked = here,
         disabled = o.disabled == true,
         opacity = o.disabled == true and "$disabledOpacity" or nil,
         height = "$kitRow",
         gap = 8,
         cross_align = "center",
         cursor = "pointer",
         on_click = act(c, "radio_group.pick", { value = o.value }),
         tickbox(c, here, false, true),
         ctx.text(c, o.label),
      })
   end
   return column(t)
end

function M.segmented(c, p)
   local options = (p.options or {})
   local bordered = c.s.skin.ctl_border
   local t = {
      role = "radioGroup",
      height = "$kitCtl",
      radius = "$kitRadius",
      pad = bordered and 0 or 2,
      gap = bordered and 0 or 2,
      bg = ref(c, bordered and "panel" or "sunken"),
      clip = true,
   }
   if bordered then t.border = { w = "$kitRuleW", color = ref(c, "ink") } end
   for i, o in ipairs(options) do
      local here = o.value == p.value
      if bordered and i > 1 then t[#t + 1] = ctx.rule(c, "ink", "$kitHair", true) end
      t[#t + 1] = row({
         key = "s" .. i,
         role = "radio",
         label = o.label,
         checked = here,
         disabled = o.disabled == true,
         height = "grow",
         pad = { x = 10 },
         cross_align = "center",
         radius = bordered and 0 or "$kitRadius",
         bg = here and ref(c, bordered and "accent" or "panel") or nil,
         hover_bg = here and ref(c, bordered and "accent" or "panel") or ref(c, "hover"),
         cursor = "pointer",
         on_click = act(c, "segmented.pick", { value = o.value }),
         gap = 6,
         ctx.caption(c, o.label, { color = ref(c, here and bordered and "on_accent" or "fg") }),
         o.count ~= nil and ctx.data(c, tostring(o.count), { size = "$kitSizeLabel", bold = o.alert == true,
color = ref(c, o.alert == true and "danger" or (here and bordered and "on_accent" or "muted")), }) or nil,
      })
   end
   return row(t)
end

function M.slider(c, p)
   local lo, hi = (p.min or 0), (p.max or 1)
   local v = math.max(lo, math.min(hi, (p.value or lo)))
   local f = hi > lo and (v - lo) / (hi - lo) or 0
   local step = (p.step or (hi - lo) / 100)
   local track = row({
      role = "slider",
      label = str(p.label),
      value_now = v,
      value_min = lo,
      value_max = hi,
      value_step = step,
      on_change = act(c, "slider.change"),
      width = "grow",
      height = 18,
      cross_align = "center",
      cursor = "pointer",
      column({ width = { pct = f * 100 }, height = 4, radius = 2, bg = ref(c, "accent") }),
      column({
         width = 12, height = 12, radius = c.s.skin.radius > 0 and 6 or 0,
         bg = ref(c, "panel"), border = { w = "$kitRuleW", color = ref(c, "ink") },
      }),
      column({ width = "grow", height = 4, radius = 2, bg = ref(c, "rule") }),
   })
   if not str(p.label) then return track end
   return column({
      width = "grow",
      gap = 2,
      row({ width = "grow", ctx.muted(c, ctx.label(c, p.label), { bold = true }), ctx.grow(),
ctx.data(c, string.format("%g", v), { size = "$kitSizeLabel", color = ref(c, "muted") }), }),
      track,
   })
end


function M.event(m, kit)
   if kit == "button.press" or kit == "icon_button.press" then
      return emit("on_press", {})
   elseif kit == "text_field.change" or kit == "search.change" then
      local ch = m.change
      return emit("on_change", { text = ch and ch.text or "" })
   elseif kit == "text_field.submit" or kit == "search.submit" then
      return emit("on_submit", { text = m.text or "" })
   elseif kit == "search.clear" then
      return emit("on_change", { text = "" })
   elseif kit == "select.open" then
      local st = ctx.peek(m.inst)
      if st then st.open = not st.open end
   elseif kit == "select.dismiss" then
      local st = ctx.peek(m.inst)
      if st then st.open = false end
   elseif kit == "select.pick" then
      local st = ctx.peek(m.inst)
      if st then st.open = false end
      return emit("on_change", { value = m.value })
   elseif kit == "checkbox.toggle" then
      return emit("on_toggle", { checked = m.checked })
   elseif kit == "switch.toggle" then
      return emit("on_toggle", { on = m.on })
   elseif kit == "radio_group.pick" or kit == "segmented.pick" then
      return emit("on_change", { value = m.value })
   elseif kit == "slider.change" then
      local ch = m.change
      return emit("on_change", { value = ch and ch.value })
   end
   return nil
end

return M