-- Generated from src/inputs.tl by addons/kit/build.lua. Edit the .tl.





local ctx = require("ctx")
local icons = require("icons")
local skin = require("skin")



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
   local group = "kit.face." .. tostring(c.inst)
   local s = sk.shadow
   local face = {
      key = "face",
      hover_group = group,

      width = t.width and "grow" or nil,
      height = "grow",
      pad = t.pad,
      gap = t.gap,
      main_align = t.main_align,
      cross_align = "center",
      radius = t.radius,
      bg = t.bg,
      border = t.border,
      shadow_color = t.shadow_color,
      shadow_x = s,
      shadow_y = s,
      when = {
         { group_hovered = true, bg = t.hover_bg, dx = -1, dy = -1, shadow_x = s + 1, shadow_y = s + 1 },
         { group_pressed = true, bg = t.pressed_bg, dx = s, dy = s, shadow_x = 0, shadow_y = 0 },
      },
   }
   for i = 1, #t do face[i] = t[i] end
   local hit = {
      key = "hit",
      hover_group = group,
      radius = t.radius,


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



function M.button(c, p, on)
   local variant = str(p.variant) or "default"
   if p.disabled == true and variant ~= "ghost" then variant = "default" end
   local ink_role = look(c, variant).ink
   local label = str(p.label) or ""
   local t = pressable(c, variant, {
      key = on and str(p.key) or nil,
      label = str(p.name) or label,
      tooltip = str(p.tooltip),
      disabled = p.disabled == true,
      pad = { x = 10 },
      gap = 6,
      on_click = on or act(c, "button.press"),
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
   return column(ctx.up_to({ gap = 3, ctx.muted(c, ctx.label(c, label), { bold = true }), body }, width, 80))
end




local function browse_button(c, p, value)
   local mode = str(p.browse)
   if not mode then return nil end
   local st = ctx.state(c, function() return { seen = false } end)
   if st.browse then
      st.browse = false
      local dir = value:match("^/") and value or nil
      pcall(c.env.request_files, {
         mode = mode,
         title = str(p.label),
         directory = mode ~= "save" and dir or nil,
         file_name = mode == "save" and (value:match("[^/]+$")) or nil,
         tag = act(c, "text_field.picked"),
      })
   end
   local t = pressable(c, "default", {
      key = "browse",
      label = (str(p.browse_label) or "Choose…") .. (str(p.label) and (" " .. (p.label):lower()) or ""),
      disabled = p.disabled == true,
      pad = { x = 10 },
      gap = 6,
      on_click = act(c, "text_field.browse"),
      icons.node(c, mode == "folder" and "folder" or "file", 13, "fg"),
      ctx.caption(c, str(p.browse_label) or "Choose…", { color = ref(c, "fg") }),
   })
   if p.disabled == true then t.opacity = "$disabledOpacity" end
   return pressed_face(c, "default", t)
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
   local chooser = browse_button(c, p, value)


   local field = column(ctx.up_to({ gap = 0, row(box), underline(c, focused, err ~= nil) },
   not chooser and p.width or nil, 80))
   if chooser then



      field = row(ctx.up_to({ gap = 8, cross_align = "center", field, chooser,
when = { { width = { below = 260, band = 16 }, dir = "column", cross_align = "start", gap = 6 } }, }, p.width, 80))
   end
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


         tab_moves_focus = p.tab_moves_focus == true or nil,
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
   local field = column(ctx.up_to({ height = grow and "grow" or nil, gap = 0,
column(box), error_line(c, err), }, p.width, 80))
   if p.hide_label == true or not str(p.label) then return field end
   return column(ctx.up_to({ height = grow and "grow" or nil, gap = 3,
ctx.muted(c, ctx.label(c, p.label), { bold = true }), field, }, p.width, 80))
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











local function neighbours(options, value)
   local at = 0
   for i, o in ipairs(options) do
      if o.value == value then at = i end
   end
   local before, after = nil, nil
   for i = at - 1, 1, -1 do
      if options[i].disabled ~= true then before = options[i].value; break end
   end
   for i = at + 1, #options do
      if options[i].disabled ~= true then after = options[i].value; break end
   end
   return before, after
end

function M.select(c, p)
   local options = (p.options or {})
   local st = ctx.state(c, function() return { open = false } end)
   if st.hand then
      if st.hand == "prev" then c.env.focus_prev() else c.env.focus_next() end
      st.hand = nil
   end
   local current = chosen(options, p.value)
   local before, after = neighbours(options, p.value)
   local field = pressable(c, "default", {
      label = (str(p.label) or "choice") .. ": " .. (current or "none"),
      expanded = st.open and "expanded" or "collapsed",
      pad = { x = 8 },
      gap = 6,




      on_click = act(c, "select.open"),
      opens_on_press = true,







      on_key = act(c, "select.key", { mac = c.env.platform == "macos", before = before, after = after }),
      ctx.text(c, current or str(p.placeholder) or "Choose…", {
         color = ref(c, current and "fg" or "faint"), max_lines = 1, ellipsis = true,
      }),
      ctx.grow(),
      icons.node(c, st.open and "chevron_up" or "chevron_down", 12, "muted"),
   })

   ctx.up_to(field, p.width, 80)
   if st.open then





      local float = MENU_FLOAT
      local at = 0
      for i, o in ipairs(options) do
         if o.value == p.value then at = i end
      end
      if at > 0 then
         local row_h, ctl_h = skin.row_h(c.s), skin.ctl_h(c.s)
         local edge = c.s.hair or c.s.skin.hair
         float = { anchor = "parent", at = { "start", "start" }, self = { "start", "start" }, fit = true,
dy = -(4 + edge + (at - 1) * (row_h + 1)) + (ctl_h - row_h) / 2, }
      end
      local list = {
         key = "menu",
         role = "menu",
         float = float,
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


      return labelled(c, str(p.label), column(ctx.up_to({ row(field), column(list) }, p.width, 80)), p.width)
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





local function ringed_mark(c, mark, group, w, h, radius)
   local m = mark
   local ring = ctx.focus_ring(c, w, h, radius)
   ring.hover_group = group
   ring.opacity = 0
   ring.when = { { group_focus_visible = true, opacity = 1 } }
   m[#m + 1] = ring
   return mark
end


















function M.checklist(c, p)
   local items = (p.items or {})
   local removable = p.removable == true
   local st = ctx.state(c, function() return {} end)
   local focus = st.focus
   if focus and focus >= #items then
      focus = #items > 0 and #items - 1 or nil
      st.focus = focus
   end
   local t = {
      key = "checklist",
      role = "list",
      label = str(p.label) or "checklist",
      width = "grow",
      gap = 0,
      active_index = focus or (#items > 0 and 0 or nil),
      on_item_key = removable and act(c, "checklist.key") or nil,
   }
   for i, it in ipairs(items) do
      local on = it.checked == true
      local off = it.disabled == true
      local group = "kit.check." .. tostring(c.inst) .. "." .. i
      local mark = tickbox(c, on, false, false)
      local r = {
         index = i - 1,
         role = "listItem",
         label = it.label,


         checked = on,
         description = on and "done" or "not done",
         disabled = off or nil,
         focusable = not off or nil,
         hover_group = group,
         focus_ring = 0xffffff00,
         on_focus = act(c, "checklist.focus", { index = i - 1 }),
         on_click = not off and act(c, "checklist.toggle", { index = i - 1, checked = not on }) or nil,
         width = "grow",
         height = "$kitCtl",
         pad = { x = 2 },
         gap = 8,
         cross_align = "center",
         radius = "$kitRadius",
         hover_bg = ref(c, "hover"),
         cursor = not off and "pointer" or nil,
         opacity = off and "$disabledOpacity" or nil,
         ringed_mark(c, mark, group, 14, 14, c.s.skin.radius > 0 and 3 or 0),
         ctx.text(c, it.label, { max_lines = 1, ellipsis = true, color = ref(c, on and "muted" or "fg") }),
         ctx.grow(),
      }
      if removable then



         r[#r + 1] = row({
            key = "rm", width = 20, height = 20, main_align = "center", cross_align = "center",
            radius = "$kitRadius", hover_bg = ref(c, "hover"), tooltip = "Remove",
            on_hover = act(c, "checklist.over", { index = i - 1 }),
            icons.node(c, "x", 12, "muted"),
         })
      end
      t[#t + 1] = row(r)
   end
   return column(t)
end

function M.checkbox(c, p)
   local on, mixed = p.checked == true, p.mixed == true
   local group = "kit.mark." .. tostring(c.inst)
   local mark = ringed_mark(c, tickbox(c, on, mixed, false), group, 14, 14, c.s.skin.radius > 0 and 3 or 0)
   return row({
      key = "ctl",
      hover_group = group,
      focus_ring = 0xffffff00,
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
      when = { ctx.still() },
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
      when = { ctx.still() },
      knob,
   }
   local group = "kit.mark." .. tostring(c.inst)
   return row({
      key = "ctl",
      hover_group = group,
      focus_ring = 0xffffff00,
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
      ringed_mark(c, row(track), group, 32, 18, round and 9 or 0),
      ctx.text(c, str(p.label) or ""),
   })
end

function M.radio_group(c, p)
   local options = (p.options or {})
   local t = { role = "radioGroup", label = str(p.label), gap = 2 }
   if str(p.label) and p.hide_label ~= true then t[#t + 1] = ctx.muted(c, ctx.label(c, p.label), { bold = true }) end
   for i, o in ipairs(options) do
      local here = o.value == p.value
      local group = "kit.mark." .. tostring(c.inst) .. "." .. i
      t[#t + 1] = row({
         key = "r" .. i,
         hover_group = group,
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
         focus_ring = 0xffffff00,
         ringed_mark(c, tickbox(c, here, false, true), group, 14, 14, 7),
         ctx.text(c, o.label),
      })
   end
   return column(t)
end

function M.segmented(c, p)
   local options = (p.options or {})







   local on_bg, on_fg = "chosen", "chosen_fg"
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
      local seg = {
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
      }
      t[#t + 1] = row(seg)
   end
   return row(t)
end







function M.slider(c, p)
   local lo, hi = (p.min or 0), (p.max or 1)
   local v = math.max(lo, math.min(hi, (p.value or lo)))
   local f = hi > lo and (v - lo) / (hi - lo) or 0





   local st = ctx.state(c, function() return { moves = 0 } end)
   local dragged = st.moves >= 2
   local step = (p.step or (hi - lo) / 100)

   local shown = str(p.value_label) or string.format("%g", v)







   local group = "kit-slider-" .. tostring(c.inst)
   local track = row({
      key = "track",
      hover_group = group,
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





      pad = { x = 6 },
      cross_align = "center",
      cursor = "grab",
      when = { { pressed = true, cursor = "grabbing" } },
      column({ key = "fill", width = { pct = f * 100 }, height = 4, radius = 2, bg = ref(c, "accent"),
      transition = dragged and 0 or 260, easing = "snappy", when = { ctx.still() },

      column({
         key = "handle",
         float = { anchor = "parent", at = { "end", "center" }, self = { "center", "center" }, clip = true },
         hover_group = group,
         width = 12, height = 12, radius = c.s.skin.radius > 0 and 6 or 0,
         bg = ref(c, "panel"),
         border = { w = "$kitRuleW", color = ref(c, "ink") },
         cursor = "grab",
         when = {
            { hovered = true, bg = ref(c, "accent") },
            { pressed = true, bg = ref(c, "accent_press"), cursor = "grabbing" },
         },
      }), }),
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
   elseif kit == "text_field.browse" then
      local st = ctx.peek(m.inst)
      if st then st.browse = true end
   elseif kit == "text_field.picked" then
      local f = m.files
      local paths = f and f.paths
      if paths and paths[1] then return emit("on_change", { text = paths[1] }) end
   elseif kit == "text_field.submit" or kit == "search.submit" then
      return emit("on_submit", { text = m.text or "" })
   elseif kit == "search.clear" then
      return emit("on_change", { text = "" })
   elseif kit == "select.open" then
      local st = ctx.peek(m.inst)
      if st then st.open = not st.open end
   elseif kit == "select.key" then
      local k = m.key
      local code = k and k.code
      local st = ctx.peek(m.inst)
      if not st or st.open or k.phase ~= "down" then return nil end
      if code == "tab" then

         st.hand = k.shift == true and "prev" or "next"
      elseif code == "space" or code == "enter" or code == "f4" then
         st.open = true
      elseif code == "up" or code == "down" then
         if m.mac == true or k.alt == true then
            st.open = true
         elseif not (k.ctrl == true or k.super == true or k.shift == true) then
            local to = m.after
            if code == "up" then to = m.before end
            if to ~= nil then return emit("on_change", { value = to }) end
         end
      end
   elseif kit == "select.dismiss" then
      local st = ctx.peek(m.inst)
      if st then st.open = false end
   elseif kit == "select.pick" then
      local st = ctx.peek(m.inst)
      if st then st.open = false end
      return emit("on_change", { value = m.value })
   elseif kit == "checklist.focus" then
      local st = ctx.peek(m.inst)
      local f = m.focus
      if st and f and f.phase == "in" then st.focus = m.index end
   elseif kit == "checklist.toggle" then
      local st = ctx.peek(m.inst)
      if st and st.over == m.index then return emit("on_remove", { index = m.index }) end
      return emit("on_toggle", { index = m.index, checked = m.checked })
   elseif kit == "checklist.key" then

      local k = m.itemkey
      if k and (k.code == "delete" or k.code == "backspace") then
         return emit("on_remove", { index = k.index })
      end
   elseif kit == "checklist.over" then
      local st = ctx.peek(m.inst)
      local h = m.hover
      if st and h then
         if h.phase == "enter" then st.over = m.index
         elseif st.over == m.index then st.over = nil end
      end
   elseif kit == "checkbox.toggle" then
      return emit("on_toggle", { checked = m.checked })
   elseif kit == "switch.toggle" then
      return emit("on_toggle", { on = m.on })
   elseif kit == "radio_group.pick" or kit == "segmented.pick" then
      return emit("on_change", { value = m.value })
   elseif kit == "slider.change" then
      local ch = m.change

      local st = ctx.peek(m.inst)
      if st and ch then
         if ch.phase == "end" then st.moves = 0 else st.moves = st.moves + 1 end
      end
      return emit("on_change", { value = ch and ch.value })
   end
   return nil
end

return M