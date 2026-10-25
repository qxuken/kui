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
   local quiet = bordered and "panel" or "sunken"
   if variant == "primary" then
      return { bg = ref(c, "accent"), hover = ref(c, "accent_hover"), press = ref(c, "accent_press"), ink = "on_accent", border = bordered }
   elseif variant == "danger" then
      return { bg = ref(c, "danger"), hover = ref(c, "danger_hover"), press = ref(c, "danger_press"), ink = "on_danger", border = bordered }
   elseif variant == "ghost" then

      return { bg = nil, hover = ref(c, "hover"), press = ref(c, "press"), ink = "fg", border = false }
   elseif variant == "success" then

      return { bg = ref(c, quiet), hover = ref(c, quiet .. "_hover"), press = ref(c, quiet .. "_press"), ink = "ok", border = bordered }
   end
   return { bg = ref(c, quiet), hover = ref(c, quiet .. "_hover"), press = ref(c, quiet .. "_press"), ink = "fg", border = bordered }
end



local function pressable(c, variant, t)


   if t.disabled == true and variant ~= "ghost" then variant = "default" end
   local lk = look(c, variant)
   t.role = t.role or "button"
   t.height = t.height or "$kitCtl"

   t.min_width = t.min_width or "fit"
   t.cross_align = "center"
   t.radius = "$kitRadius"
   t.bg = lk.bg
   t.hover_bg = lk.hover
   t.pressed_bg = lk.press
   t.cursor = "pointer"
   if lk.border then
      local edge = (variant == "danger" or variant == "success") and (variant == "danger" and "danger" or "ok") or "ink"
      t.border = { w = "$kitRuleW", color = ref(c, edge) }
   end


   if variant == "primary" then ctx.shadow(c, t, true) end
   return t
end










local function pressed_face(c, variant, t)
   local sk = c.s.skin
   if variant ~= "primary" or t.disabled == true or not c.s.shadows or sk.shadow <= 0 or sk.shadow_blur > 0 then
      return row(t)
   end
   local function state(f)
      local ok, v = pcall(f, "hit")
      return ok and v == true
   end
   local pressed = state(c.env.is_pressed)
   local over = pressed or state(c.env.is_hovered)
   local d = pressed and sk.shadow or (over and -1 or 0)
   local face = {
      key = "face",

      width = t.width and "grow" or nil,
      height = "grow",
      pad = t.pad,
      gap = t.gap,
      main_align = t.main_align,
      cross_align = "center",
      radius = t.radius,
      bg = pressed and t.pressed_bg or (over and t.hover_bg or t.bg),
      border = t.border,
      shadow_color = t.shadow_color,
      shadow_x = sk.shadow - d,
      shadow_y = sk.shadow - d,
      keyframes = d ~= 0 and { { dx = d, dy = d }, { dx = d, dy = d } } or nil,
      iterations = d ~= 0 and 1 or nil,

      transition = d ~= 0 and 1 or nil,
   }
   for i = 1, #t do face[i] = t[i] end
   local hit = {
      key = "hit",


      hoverable = true,
      role = t.role,
      label = t.label,
      tooltip = t.tooltip,
      selected = t.selected,
      on_click = t.on_click,
      cursor = t.cursor,
      width = t.width,
      height = t.height,
      min_width = t.min_width,
      row(face),
   }
   return row(hit)
end


local function pressed_look(c, t)
   t.selected = true
   t.bg = ref(c, "sel")
   t.hover_bg = ref(c, "sel")
   return ref(c, "sel_fg")
end

function M.button(c, p)
   local variant = str(p.variant) or "default"
   if p.disabled == true and variant ~= "ghost" then variant = "default" end
   local ink_role = look(c, variant).ink
   local label = str(p.label) or ""
   local t = pressable(c, variant, {
      label = str(p.name) or label,
      tooltip = str(p.tooltip),
      disabled = p.disabled == true,
      pad = { x = 10 },
      gap = 6,
      on_click = act(c, "button.press"),
   })
   if p.pressed == true then
      pressed_look(c, t)
      ink_role = "sel_fg"
   end
   if p.disabled == true then t.opacity = "$disabledOpacity" end

   if str(p.icon) then t[#t + 1] = icons.node(c, p.icon, 13, ink_role) end
   t[#t + 1] = ctx.caption(c, label, { color = ref(c, ink_role), bold = variant == "primary" or c.s.skin.label_bold })
   if str(p.shortcut) then

      local quiet = ink_role == "fg" or ink_role == "ok"
      t[#t + 1] = ctx.data(c, p.shortcut, { size = "$kitSizeLabel", color = ref(c, quiet and "faint" or ink_role) })
   end
   return pressed_face(c, variant, t)
end

function M.icon_button(c, p)
   local variant = str(p.variant) or "ghost"
   if p.disabled == true and variant ~= "ghost" then variant = "default" end
   local label = str(p.label) or ""
   local t = pressable(c, variant, {
      label = label,
      tooltip = str(p.tooltip) or label,
      disabled = p.disabled == true,
      width = "$kitCtl",
      main_align = "center",
      on_click = act(c, "icon_button.press"),
   })
   local role = look(c, variant).ink
   if p.pressed == true then
      pressed_look(c, t)
      role = "sel_fg"
   end
   if p.disabled == true then t.opacity = "$disabledOpacity" end
   t[#t + 1] = icons.node(c, str(p.icon) or "", 14, role)
   return pressed_face(c, variant, t)
end



local function well(c, t, focused, bad)
   if c.s.skin.field_box then
      t.bg = ref(c, "sunken")
      t.radius = "$kitRadius"
      t.border = { w = "$kitHair", color = ref(c, bad and "danger" or (focused and "focus" or "rule")) }
   else
      t.border = nil
   end
end


local function underline(c, focused, bad)
   if c.s.skin.field_box then return nil end





   local role = bad and "danger" or (focused and "ink" or "faint")



   return column({ width = "grow", height = "$kitRuleW", main_align = "end",
ctx.rule(c, role, (focused or bad) and "$kitRuleW" or "$kitHair"), })
end






local function error_tag(c, msg)
   if not msg or msg == "" then return nil end
   return row({ key = "error", max_width = "60%", min_width = 0, gap = 4, cross_align = "center",
tooltip = msg,
icons.node(c, "alert", 12, "danger"),
row({ min_width = 0,
ctx.text(c, msg, { size = "$kitSizeLabel", color = ref(c, "danger"), max_lines = 1, ellipsis = true }), }), })
end




local function error_line(c, msg)
   if not msg or msg == "" then return nil end
   return row({ width = "grow", gap = 5, pad = { t = 4 }, cross_align = "center",
icons.node(c, "alert", 12, "danger"),
row({ width = "grow", min_width = 0,
ctx.text(c, msg, { size = "$kitSizeLabel", color = ref(c, "danger"), wrap = "word" }), }), })
end













local function focus_request(c, p)
   local st = ctx.state(c, function() return { seen = false } end)
   local want = p.focus
   if want ~= nil and st.seen and want ~= st.focus then

      pcall(c.env.set_focus, "edit")
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
   local err = str(p.error)
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

         description = err ~= "" and err or nil,
         key_focus = p.autofocus == true or nil,
         on_change = act(c, "text_field.change"),
         on_submit = act(c, "text_field.submit", { text = value }),
      }),
      error_tag(c, err),
   }
   if err and err ~= "" then box.gap = 6 end
   well(c, box, focused, err ~= nil)
   local field = column({ width = (p.width) or "grow", gap = 0, row(box), underline(c, focused, err ~= nil) })
   if p.hide_label == true then return field end
   return labelled(c, str(p.label), field, p.width)
end




function M.text_area(c, p)
   focus_request(c, p)
   local focused = c.env.is_focused("edit")
   local value = str(p.value) or ""
   local err = str(p.error)
   local line_h = 17
   local rows = math.max(1, (p.rows or 4))
   local max_rows = p.max_rows
   local pad = 8
   local boxed = c.s.skin.field_box
   local edge = err and "danger" or (focused and (boxed and "focus" or "ink") or (boxed and "rule" or "faint"))
   local grow = p.grow == true

   local thick = (focused or err ~= nil) and not boxed



   local box = {
      key = "box",
      width = "grow",
      height = grow and "grow" or nil,
      max_height = max_rows and (max_rows * line_h + 2 * pad) or nil,
      scroll_y = max_rows ~= nil,
      bg = ref(c, "sunken"),
      radius = "$kitRadius",
      border = { w = thick and "$kitRuleW" or "$kitHair", color = ref(c, edge) },
      edit({
         key = "edit",
         value = value,
         multiline = true,
         tab_inserts = p.tab_inserts == true or nil,
         width = "grow",
         height = grow and "grow" or nil,
         min_height = rows * line_h + 2 * pad,
         pad = pad,
         cursor = "text",
         placeholder = str(p.placeholder),
         label = str(p.label) or str(p.placeholder),
         description = err ~= "" and err or nil,
         size = "$kitSize",
         line_height = line_h,
         family = p.mono == true and "$kitData" or "$kitUi",
         color = ref(c, "fg"),
         disabled = p.disabled == true,
         key_focus = p.autofocus == true or nil,
         on_change = act(c, "text_area.change"),
      }),
   }
   local field = column({ width = (p.width) or "grow", height = grow and "grow" or nil, gap = 0,
column(box), error_line(c, err), })
   if p.hide_label == true or not str(p.label) then return field end
   return column({ width = (p.width) or "grow", height = grow and "grow" or nil, gap = 3,
ctx.muted(c, ctx.label(c, p.label), { bold = true }), field, })
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
         label = str(p.label) or str(p.placeholder) or "Search",
         width = "grow",
         size = "$kitSize",
         family = "$kitUi",
         color = ref(c, "fg"),
         on_change = act(c, "search.change"),
         on_submit = act(c, "search.submit", { text = value }),
         on_cancel = act(c, "search.clear"),
         key_focus = p.autofocus == true or nil,
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
   well(c, box, focused, false)
   return column({ width = "grow", row(box), underline(c, focused, false) })
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
   local mark = tickbox(c, on, mixed, false)
   if ctx.focus_shows(c, "ctl") then
      (mark)[#(mark) + 1] = ctx.focus_ring(c, 14, 14, c.s.skin.radius > 0 and 3 or 0)
   end
   return row({
      key = "ctl",
      focus_bg = 0x00000000,
      role = "checkbox",
      label = str(p.label),
      checked = on,
      mixed = mixed,
      disabled = p.disabled == true,
      opacity = p.disabled == true and "$disabledOpacity" or nil,
      width = p.grow == true and "grow" or nil,
      min_width = p.grow == true and 0 or nil,
      height = p.grow == true and "grow" or "$kitCtl",
      min_height = "$kitCtl",
      gap = 8,
      cross_align = "center",
      cursor = "pointer",
      on_click = act(c, "checkbox.toggle", { checked = not on }),
      mark,
      p.hide_label ~= true and ctx.text(c, str(p.label) or "", { max_lines = 1, ellipsis = true }) or nil,
      children(""),
   })
end



local SLIDE = 140

function M.switch(c, p)
   local on = p.on == true
   local bordered = c.s.skin.ctl_border
   local round = c.s.skin.radius > 0









   local sk = c.s.skin
   local ink_is_accent = sk.dark.ink == sk.dark.accent




   local knob_bg = round and "raised" or (ink_is_accent and (on and "bg" or "muted") or "ink")
   local knob = column({
      key = "knob", width = 14, height = 14, radius = round and 7 or 0, transition = SLIDE, slide = true,
      bg = ref(c, knob_bg),
      border = (not round and ink_is_accent and on) and { w = "$kitHair", color = ref(c, "ink") } or nil,
   })
   local off_edge = ink_is_accent and "faint" or (bordered and "ink" or "rule")
   local track = {
      width = 32, height = 18, pad = 2,
      radius = round and 9 or 0,
      main_align = on and "end" or "start",
      cross_align = "center",
      bg = ref(c, on and "accent" or "sunken"),
      border = { w = "$kitHair", color = ref(c, on and "accent" or off_edge) },
      transition = SLIDE,
      knob,
   }
   if ctx.focus_shows(c, "ctl") then track[#track + 1] = ctx.focus_ring(c, 32, 18, round and 9 or 0) end
   return row({
      key = "ctl",
      focus_bg = 0x00000000,
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
   if str(p.label) and p.hide_label ~= true then t[#t + 1] = ctx.muted(c, ctx.label(c, p.label), { bold = true }) end
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
   local on_bg = bordered and "ink" or "panel"
   local on_fg = bordered and "bg" or "fg"
   local t = {
      role = "radioGroup",
      label = str(p.label),
      height = "$kitCtl",
      min_width = "fit",
      radius = "$kitRadius",
      pad = 2,
      gap = 2,
      bg = ref(c, "press"),
      clip = true,
   }
   for i, o in ipairs(options) do
      local here = o.value == p.value
      t[#t + 1] = row({
         key = "s" .. i,
         role = "radio",
         label = o.label,
         checked = here,
         disabled = o.disabled == true,
         height = "grow",
         min_width = "fit",
         pad = { x = 10 },
         cross_align = "center",
         radius = "$kitRadius",
         bg = here and ref(c, on_bg) or nil,
         hover_bg = here and ref(c, on_bg) or ref(c, "hover"),
         cursor = "pointer",
         on_click = act(c, "segmented.pick", { value = o.value }),
         gap = 6,
         ctx.caption(c, o.label, { color = ref(c, here and on_fg or "fg") }),


         o.count ~= nil and ctx.data(c, tostring(o.count), { size = "$kitSizeLabel", bold = o.alert == true,
color = ref(c, here and on_fg or (o.alert == true and "danger" or "muted")), }) or nil,
      })
   end
   return row(t)
end

function M.slider(c, p)
   local lo, hi = (p.min or 0), (p.max or 1)
   local v = math.max(lo, math.min(hi, (p.value or lo)))
   local f = hi > lo and (v - lo) / (hi - lo) or 0
   local step = (p.step or (hi - lo) / 100)

   local shown = str(p.value_label) or string.format("%g", v)









   local okp, held = pcall(c.env.is_pressed, "track")
   held = okp and held
   local okh, over = pcall(c.env.is_hovered, "track")
   over = okh and over
   local track = row({
      key = "track",
      hoverable = true,
      role = "slider",
      label = str(p.label),
      value_now = v,
      value_text = str(p.value_label),
      value_min = lo,
      value_max = hi,
      value_step = step,
      on_change = act(c, "slider.change"),
      width = "grow",
      height = 18,
      cross_align = "center",
      cursor = held and "grabbing" or "grab",
      column({ width = { pct = f * 100 }, height = 4, radius = 2, bg = ref(c, "accent") }),
      column({
         key = "handle",
         width = 12, height = 12, radius = c.s.skin.radius > 0 and 6 or 0,
         bg = ref(c, held and "accent_press" or (over and "accent" or "panel")),
         border = { w = "$kitRuleW", color = ref(c, "ink") },
      }),
      column({ width = "grow", height = 4, radius = 2, bg = ref(c, "rule") }),
   })
   if not str(p.label) then return track end
   return column({
      width = "grow",
      gap = 2,
      row({ width = "grow", ctx.muted(c, ctx.label(c, p.label), { bold = true }), ctx.grow(),
ctx.data(c, shown, { size = "$kitSizeLabel", color = ref(c, "fg") }), }),
      track,
   })
end


function M.event(m, kit)
   if kit == "button.press" or kit == "icon_button.press" then
      return emit("on_press", {})
   elseif kit == "text_field.change" or kit == "search.change" or kit == "text_area.change" then
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