-- Inputs: buttons, fields, choices and a slider. Every one is a box with
-- the role assistive technology reads it by, so Space, Enter and a
-- reader's press reach it as a click does, and every one is controlled:
-- what it shows is the host's prop, and an event proposes the next value.

local ctx = require("ctx")
local icons = require("icons")

local type Ctx = ctx.Ctx
local type P = {string:any}
local ref, act = ctx.ref, ctx.act

local M = {}

-- Under what opened it, its left edge on the opener's: a menu reads down
-- from where the eye already is (the `below` preset centres, as a tooltip
-- does). `fit` flips and clamps it onto the screen.
local MENU_FLOAT: {any:any} = { anchor = "below", at = { "start", "end" }, self = { "start", "start" }, fit = true }

local function str(v: any): string
  return v is string and v or nil
end

-- The look a button's variant paints with: its fill, hover fill, label
-- colour and whether it carries a border.
local function look(c: Ctx, variant: string): string, string, string, boolean
  local bordered = c.s.skin.ctl_border
  if variant == "primary" then
    return ref(c, "accent"), ref(c, "accent"), ref(c, "on_accent"), bordered
  elseif variant == "danger" then
    return ref(c, "danger"), ref(c, "danger"), ref(c, "on_accent"), bordered
  elseif variant == "ghost" then
    return nil, ref(c, "hover"), ref(c, "fg"), false
  end
  return bordered and ref(c, "panel") or ref(c, "sunken"), ref(c, "hover"), ref(c, "fg"), bordered
end

-- A pressable box at the control height: what button, icon_button and a
-- select's field share.
local function pressable(c: Ctx, variant: string, t: kui.Props): kui.Props
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
    t.border = { w = "$kitRuleW", color = ref(c, variant == "danger" and "danger" or "ink") }
  end
  if variant == "primary" then ctx.shadow(c, t, true) end
  return t
end

function M.button(c: Ctx, p: P): kui.Node
  local variant = str(p.variant) or "default"
  local _, _, ink, _ = look(c, variant)
  local label = str(p.label) or ""
  local t = pressable(c, variant, {
    label = label,
    disabled = p.disabled == true,
    pad = { x = 10 },
    gap = 6,
    on_click = act(c, "button.press"),
  })
  if p.disabled == true then t.opacity = "$disabledOpacity" end
  if str(p.icon) then t[#t + 1] = icons.node(c, p.icon as string, 13, variant == "primary" and "on_accent" or "fg") end
  t[#t + 1] = ctx.caption(c, label, { color = ink })
  if str(p.shortcut) then
    t[#t + 1] = ctx.data(c, p.shortcut as string, { size = "$kitSizeLabel", color = ref(c, variant == "primary" and "on_accent" or "faint") })
  end
  return row(t)
end

function M.icon_button(c: Ctx, p: P): kui.Node
  local variant = str(p.variant) or "ghost"
  local label = str(p.label) or ""
  local t = pressable(c, variant, {
    label = label,
    tooltip = label,
    disabled = p.disabled == true,
    width = "$kitCtl",
    main_align = "center",
    on_click = act(c, "icon_button.press"),
  })
  if p.disabled == true then t.opacity = "$disabledOpacity" end
  t[#t + 1] = icons.node(c, str(p.icon) or "", 14, variant == "primary" and "on_accent" or "fg")
  return row(t)
end

-- A field's well: an underline in Ledger and Phosphor, a box in Neutral.
local function well(c: Ctx, t: kui.Props, focused: boolean)
  if c.s.skin.field_box then
    t.bg = ref(c, "sunken")
    t.radius = "$kitRadius"
    t.border = { w = "$kitHair", color = ref(c, focused and "focus" or "rule") }
  else
    t.border = nil
  end
end

-- The underline under a field in the skins that draw one.
local function underline(c: Ctx, focused: boolean): kui.Node
  if c.s.skin.field_box then return nil end
  return ctx.rule(c, focused and "accent" or "ink", focused and "$kitRuleW" or "$kitHair")
end

local function labelled(c: Ctx, label: string, body: kui.Node, width: any): kui.Node
  if not label then return body end
  return column { width = (width as number) or "grow", gap = 3, ctx.muted(c, ctx.label(c, label), { bold = true }), body }
end

function M.text_field(c: Ctx, p: P): kui.Node
  local focused = c.env.is_focused("edit")
  local value = str(p.value) or ""
  local box: kui.Props = {
    height = "$kitCtl",
    width = "grow",
    pad = { x = c.s.skin.field_box and 8 or 2 },
    cross_align = "center",
    edit {
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
    },
  }
  well(c, box, focused)
  local field = column { width = (p.width as number) or "grow", gap = 0, row(box), underline(c, focused) }
  return labelled(c, str(p.label), field, p.width)
end

function M.search(c: Ctx, p: P): kui.Node
  local focused = c.env.is_focused("edit")
  local value = str(p.value) or ""
  local box: kui.Props = {
    height = "$kitCtl",
    width = "grow",
    gap = 6,
    pad = { x = c.s.skin.field_box and 8 or 2 },
    cross_align = "center",
    icons.node(c, "search", 13, "muted"),
    edit {
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
    },
  }
  if str(p.count) then box[#box + 1] = ctx.data(c, p.count as string, { size = "$kitSizeLabel", color = ref(c, "muted") }) end
  if value ~= "" then
    box[#box + 1] = row {
      role = "button", label = "Clear", width = 18, height = 18, main_align = "center",
      cross_align = "center", radius = "$kitRadius", hover_bg = ref(c, "hover"), cursor = "pointer",
      on_click = act(c, "search.clear"),
      icons.node(c, "x", 12, "muted"),
    }
  end
  well(c, box, focused)
  return column { width = "grow", row(box), underline(c, focused) }
end

-- The label of the option chosen, nil for none.
local function chosen(options: {P}, value: any): string
  for _, o in ipairs(options) do
    if o.value == value then return o.label as string end
  end
  return nil
end

local record Open
  open: boolean
end

function M.select(c: Ctx, p: P): kui.Node
  local options = (p.options or {}) as {P}
  local st = ctx.state(c, function(): Open return { open = false } end)
  local current = chosen(options, p.value)
  local field = pressable(c, "default", {
    label = (str(p.label) or "choice") .. ": " .. (current or "none"),
    expanded = st.open and "expanded" or "collapsed",
    width = (p.width as number) or "grow",
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
    local list: kui.Props = {
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
      list[#list + 1] = row {
        key = "o" .. i,
        role = "menuItem",
        label = o.label as string,
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
        ctx.text(c, o.label as string, { color = ref(c, here and "sel_fg" or "fg") }),
      }
    end
    -- Beside the field, not in it: a button's children are presentational
    -- to assistive technology, and the options must be read.
    return labelled(c, str(p.label), column { width = (p.width as number) or "grow", row(field), column(list) }, p.width)
  end
  return labelled(c, str(p.label), row(field), p.width)
end

-- The square a checkbox draws: ticked, a dash for mixed, or empty.
local function tickbox(c: Ctx, on: boolean, mixed: boolean, round: boolean): kui.Node
  local filled = on or mixed
  local t: kui.Props = {
    width = 14, height = 14, main_align = "center", cross_align = "center",
    radius = round and 7 or (c.s.skin.radius > 0 and 3 or 0),
    bg = filled and not round and ref(c, "accent") or ref(c, "panel"),
    border = { w = c.s.skin.ctl_border and "$kitRuleW" or "$kitHair", color = ref(c, filled and "accent" or "ink") },
  }
  if round and on then
    t[1] = column { width = 6, height = 6, radius = 3, bg = ref(c, "accent") }
  elseif filled then
    t[1] = icons.node(c, mixed and "minus" or "check", 11, "on_accent")
  end
  return row(t)
end

function M.checkbox(c: Ctx, p: P): kui.Node
  local on, mixed = p.checked == true, p.mixed == true
  return row {
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
  }
end

function M.switch(c: Ctx, p: P): kui.Node
  local on = p.on == true
  local track: kui.Props = {
    width = 30, height = 16, radius = 8, pad = 2,
    main_align = on and "end" or "start",
    cross_align = "center",
    bg = ref(c, on and "accent" or "sunken"),
    border = { w = "$kitHair", color = ref(c, on and "accent" or "ink") },
    -- The knob slides: the same node, keyed, moving with a transition.
    column { key = "knob", width = 12, height = 12, radius = 6, transition = 0.12,
      bg = ref(c, on and "on_accent" or "muted") },
  }
  if c.s.skin.radius == 0 then
    track.radius = 0
    track[1] = column { key = "knob", width = 12, height = 12, transition = 0.12, bg = ref(c, on and "on_accent" or "muted") }
  end
  return row {
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
  }
end

function M.radio_group(c: Ctx, p: P): kui.Node
  local options = (p.options or {}) as {P}
  local t: kui.Props = { role = "radioGroup", label = str(p.label), gap = 2 }
  if str(p.label) then t[#t + 1] = ctx.muted(c, ctx.label(c, p.label as string), { bold = true }) end
  for i, o in ipairs(options) do
    local here = o.value == p.value
    t[#t + 1] = row {
      key = "r" .. i,
      role = "radio",
      label = o.label as string,
      checked = here,
      disabled = o.disabled == true,
      opacity = o.disabled == true and "$disabledOpacity" or nil,
      height = "$kitRow",
      gap = 8,
      cross_align = "center",
      cursor = "pointer",
      on_click = act(c, "radio_group.pick", { value = o.value }),
      tickbox(c, here, false, true),
      ctx.text(c, o.label as string),
    }
  end
  return column(t)
end

function M.segmented(c: Ctx, p: P): kui.Node
  local options = (p.options or {}) as {P}
  local bordered = c.s.skin.ctl_border
  local t: kui.Props = {
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
    t[#t + 1] = row {
      key = "s" .. i,
      role = "radio",
      label = o.label as string,
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
      ctx.caption(c, o.label as string, { color = ref(c, here and bordered and "on_accent" or "fg") }),
    }
  end
  return row(t)
end

function M.slider(c: Ctx, p: P): kui.Node
  local lo, hi = (p.min or 0) as number, (p.max or 1) as number
  local v = math.max(lo, math.min(hi, (p.value or lo) as number))
  local f = hi > lo and (v - lo) / (hi - lo) or 0
  local step = (p.step or (hi - lo) / 100) as number
  local track = row {
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
    column { width = { pct = f * 100 }, height = 4, radius = 2, bg = ref(c, "accent") },
    column {
      width = 12, height = 12, radius = c.s.skin.radius > 0 and 6 or 0,
      bg = ref(c, "panel"), border = { w = "$kitRuleW", color = ref(c, "ink") },
    },
    column { width = "grow", height = 4, radius = 2, bg = ref(c, "rule") },
  }
  if not str(p.label) then return track end
  return column {
    width = "grow",
    gap = 2,
    row { width = "grow", ctx.muted(c, ctx.label(c, p.label as string), { bold = true }), ctx.grow(),
      ctx.data(c, string.format("%g", v), { size = "$kitSizeLabel", color = ref(c, "muted") }) },
    track,
  }
end

-- The events: what each action proposes to the host.
function M.event(m: P, kit: string): any
  if kit == "button.press" or kit == "icon_button.press" then
    return emit("on_press", {})
  elseif kit == "text_field.change" or kit == "search.change" then
    local ch = m.change as P
    return emit("on_change", { text = ch and ch.text or "" })
  elseif kit == "text_field.submit" or kit == "search.submit" then
    return emit("on_submit", { text = m.text or "" })
  elseif kit == "search.clear" then
    return emit("on_change", { text = "" })
  elseif kit == "select.open" then
    local st = ctx.peek(m.inst as integer) as Open
    if st then st.open = not st.open end
  elseif kit == "select.dismiss" then
    local st = ctx.peek(m.inst as integer) as Open
    if st then st.open = false end
  elseif kit == "select.pick" then
    local st = ctx.peek(m.inst as integer) as Open
    if st then st.open = false end
    return emit("on_change", { value = m.value })
  elseif kit == "checkbox.toggle" then
    return emit("on_toggle", { checked = m.checked })
  elseif kit == "switch.toggle" then
    return emit("on_toggle", { on = m.on })
  elseif kit == "radio_group.pick" or kit == "segmented.pick" then
    return emit("on_change", { value = m.value })
  elseif kit == "slider.change" then
    local ch = m.change as P
    return emit("on_change", { value = ch and ch.value })
  end
  return nil
end

return M
