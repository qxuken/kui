-- Inputs: buttons, fields, choices and a slider. Every one is a box with
-- the role assistive technology reads it by, so Space, Enter and a
-- reader's press reach it as a click does, and every one is controlled:
-- what it shows is the host's prop, and an event proposes the next value.

local ctx = require("ctx")
local icons = require("icons")
local skin = require("skin")

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

local record Look
  -- The fill at rest, under the pointer and held; nil at rest for none.
  bg: string
  hover: string
  press: string
  -- The role the label, the icon and the shortcut are drawn in: one
  -- colour for all three, so an icon is never dark beside a white label.
  ink: string
  border: boolean
end

-- The look a button's variant paints with. Every variant has a visible
-- hover and press, and every shade is opaque (`accent_hover`, worked out
-- by the skin): a translucent wash over a fill showed what was under it —
-- under the primary button, Ledger's hard shadow.
local function look(c: Ctx, variant: string): Look
  local bordered = c.s.skin.ctl_border
  local quiet = bordered and "panel" or "sunken"
  if variant == "primary" then
    return { bg = ref(c, "accent"), hover = ref(c, "accent_hover"), press = ref(c, "accent_press"), ink = "on_accent", border = bordered }
  elseif variant == "danger" then
    return { bg = ref(c, "danger"), hover = ref(c, "danger_hover"), press = ref(c, "danger_press"), ink = "on_danger", border = bordered }
  elseif variant == "ghost" then
    -- No fill of its own: the washes over whatever it sits on.
    return { bg = nil, hover = ref(c, "hover"), press = ref(c, "press"), ink = "fg", border = false }
  elseif variant == "success" then
    -- Confirms rather than invites: the ok colour on the quiet fill.
    return { bg = ref(c, quiet), hover = ref(c, quiet .. "_hover"), press = ref(c, quiet .. "_press"), ink = "ok", border = bordered }
  end
  return { bg = ref(c, quiet), hover = ref(c, quiet .. "_hover"), press = ref(c, quiet .. "_press"), ink = "fg", border = bordered }
end

-- A pressable box at the control height: what button, icon_button and a
-- select's field share.
local function pressable(c: Ctx, variant: string, t: kui.Props): kui.Props
  -- Unavailable is quiet whatever the variant: a faded accent and its
  -- shadow read as a muddy call to action, not as "not now".
  if t.disabled == true and variant ~= "ghost" then variant = "default" end
  local lk = look(c, variant)
  t.role = t.role or "button"
  t.height = t.height or "$kitCtl"
  -- Never squeezed below its label: a crowded row gives elsewhere.
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
  -- The shadow stays where it is under hover and press: the fill shades,
  -- opaque, and nothing of it moves.
  if variant == "primary" then ctx.shadow(c, t, true) end
  return t
end

-- A button that casts a hard shadow (Ledger's primary) moves its face
-- with the pointer: lifted a pixel toward the light under it, the shadow
-- a pixel longer; pressed into the shadow's place, the shadow gone. The
-- shadow's far edge never moves, so the face reads as sliding on it.
-- Done as a still box that takes the pointer (so the face moving cannot
-- flicker the hover), and a face in it that casts the shadow, moved by
-- `dx` / `dy` and its shadow lengthened or dropped by rules on the pair's
-- hover group: lifted a pixel with the shadow a pixel longer, its far
-- edge stays put; pressed by the shadow's length with no shadow, it sits
-- where the shadow was. A hover or a press repaints and runs no view
-- (the face read `is_hovered` and `is_pressed`, and every hover re-ran
-- the button). Instant, not eased, as a pressed key is.
local function pressed_face(c: Ctx, variant: string, t: kui.Props): kui.Node
  local sk = c.s.skin
  if variant ~= "primary" or t.disabled == true or not c.s.shadows or sk.shadow <= 0 or sk.shadow_blur > 0 then
    return row(t)
  end
  local group = "kit.face." .. tostring(c.inst)
  local s = sk.shadow
  local face: kui.Props = {
    key = "face",
    hover_group = group,
    -- Its content's width in a box that fits it; the box's where it has one.
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
  local hit: kui.Props = {
    key = "hit",
    hover_group = group,
    radius = t.radius,
    -- Hover-tracked, so its group reads hovered (a box with only a click
    -- is not).
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

-- A toggle that is on: the selection's fill, read as selected.
local function pressed_look(c: Ctx, t: kui.Props): string
  t.selected = true
  t.bg = ref(c, "sel")
  t.hover_bg = ref(c, "sel")
  return ref(c, "sel_fg")
end

-- A button; `on`, for a button another component draws inside itself,
-- is its own action in place of `button.press`.
function M.button(c: Ctx, p: P, on?: {string:any}): kui.Node
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
  -- The icon takes the label's colour, whatever the variant.
  if str(p.icon) then t[#t + 1] = icons.node(c, p.icon as string, 13, ink_role) end
  t[#t + 1] = ctx.caption(c, label, { color = ref(c, ink_role), bold = variant == "primary" or c.s.skin.label_bold })
  if str(p.shortcut) then
    -- Faint beside a quiet label; on a filled one, the label's own ink.
    local quiet = ink_role == "fg" or ink_role == "ok"
    t[#t + 1] = ctx.data(c, p.shortcut as string, { size = "$kitSizeLabel", color = ref(c, quiet and "faint" or ink_role) })
  end
  return pressed_face(c, variant, t)
end

function M.icon_button(c: Ctx, p: P): kui.Node
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

-- A field's well: an underline in Ledger and Phosphor, a box in Neutral.
-- In error its edge is the danger colour, focused or not.
local function well(c: Ctx, t: kui.Props, focused: boolean, bad: boolean)
  if c.s.skin.field_box then
    t.bg = ref(c, "sunken")
    t.radius = "$kitRadius"
    t.border = { w = "$kitHair", color = ref(c, bad and "danger" or (focused and "focus" or "rule")) }
  else
    t.border = nil
  end
end

-- The underline under a field in the skins that draw one.
local function underline(c: Ctx, focused: boolean, bad: boolean): kui.Node
  if c.s.skin.field_box then return nil end
  -- At rest a hairline in the faint ink: soft beside the text, yet the
  -- 3:1 a field's edge needs to read as a field (the `rule` colour drew
  -- it at 1.5:1, a version field read as a label). Focused, the region
  -- rule in the ink: the accent is a highlighter on Ledger's paper and
  -- vanished there as a focus mark. In error, the danger colour.
  local role = bad and "danger" or (focused and "ink" or "faint")
  -- Always the region rule's height, the hairline at its foot: the
  -- thicker focused rule took a pixel more, and focusing a field moved
  -- everything under it (a params table's rows, a form).
  return column { width = "grow", height = "$kitRuleW", main_align = "end",
    ctx.rule(c, role, (focused or bad) and "$kitRuleW" or "$kitHair") }
end

-- What is wrong with a one-line field, at the end of its well in the
-- danger colour: under the field it made the field's row taller, and a
-- toolbar holding the field grew and moved its buttons off the field's
-- line. Cut short past half the field, the whole message its tooltip and
-- its description.
local function error_tag(c: Ctx, msg: string): kui.Node
  if not msg or msg == "" then return nil end
  return row { key = "error", max_width = "60%", min_width = 0, gap = 4, cross_align = "center",
    tooltip = msg,
    icons.node(c, "alert", 12, "danger"),
    row { min_width = 0,
      ctx.text(c, msg, { size = "$kitSizeLabel", color = ref(c, "danger"), max_lines = 1, ellipsis = true }) } }
end

-- What is wrong with a field of several lines, under it in the danger
-- colour. An empty message marks the field without a line: fields that
-- share one message.
local function error_line(c: Ctx, msg: string): kui.Node
  if not msg or msg == "" then return nil end
  return row { width = "grow", gap = 5, pad = { t = 4 }, cross_align = "center",
    icons.node(c, "alert", 12, "danger"),
    row { width = "grow", min_width = 0,
      ctx.text(c, msg, { size = "$kitSizeLabel", color = ref(c, "danger"), wrap = "word" }) } }
end

local record Asked
  focus: integer
  seen: boolean
  -- Choose… was pressed: the next fill asks for the dialog (a view's
  -- verb; an event handler holds no env).
  browse: boolean
end

-- `focus` is a request: each new number takes the keyboard to the field.
-- The first number seen is where it starts, not a request; and a field
-- that was not on screen last frame — new, or back after the number
-- moved while it was away — takes its number as where it starts too:
-- the request was for the field showing then, and there is no node of
-- this one's to move the keyboard to yet. `autofocus` is the way to ask
-- for a field as it first shows (the edit's `key_focus`).
local function focus_request(c: Ctx, p: P)
  local st = ctx.state(c, function(): Asked return { seen = false } end)
  local want = p.focus as integer
  if want ~= nil and st.seen and want ~= st.focus then
    -- Refused when no node was keyed "edit" here last frame: an arriving field.
    pcall(c.env.set_focus, "edit")
  end
  st.focus, st.seen = want, true
end

local function labelled(c: Ctx, label: string, body: kui.Node, width: any): kui.Node
  if not label then return body end
  return column(ctx.up_to({ gap = 3, ctx.muted(c, ctx.label(c, label), { bold = true }), body }, width, 80))
end

-- A field's Choose… button and its dialog, for `browse`: the press marks
-- the ask, this fill makes it, and the answer comes back to Kit as the
-- tag's message with `files` in it (`text_field.picked`).
local function browse_button(c: Ctx, p: P, value: string): kui.Node
  local mode = str(p.browse)
  if not mode then return nil end
  local st = ctx.state(c, function(): Asked return { seen = false } end)
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
    label = (str(p.browse_label) or "Choose…") .. (str(p.label) and (" " .. (p.label as string):lower()) or ""),
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

function M.text_field(c: Ctx, p: P): kui.Node
  focus_request(c, p)
  local focused = c.env.is_focused("edit")
  local value = str(p.value) or ""
  local err = str(p.error)
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
      -- The error is read after the name, as what to fix.
      description = err ~= "" and err or nil,
      key_focus = p.autofocus == true or nil,
      on_change = act(c, "text_field.change"),
      on_submit = act(c, "text_field.submit", { text = value }),
    },
    error_tag(c, err),
  }
  if err and err ~= "" then box.gap = 6 end
  well(c, box, focused, err ~= nil)
  local chooser = browse_button(c, p, value)
  -- The field takes what its row leaves, down to 80 px (its text scrolls
  -- in the editor); a Choose… button beside it keeps its whole label.
  local field: kui.Node = column(ctx.up_to({ gap = 0, row(box), underline(c, focused, err ~= nil) },
    not chooser and p.width or nil, 80))
  if chooser then
    -- Beside the field while the row is wide enough for both, under it
    -- when it is not (ADR 0069, the row's own width): the field keeps a
    -- width to type in and the button its whole label.
    field = row(ctx.up_to({ gap = 8, cross_align = "center", field, chooser,
      when = { { width = { below = 260, band = 16 }, dir = "column", cross_align = "start", gap = 6 } } }, p.width, 80))
  end
  if p.hide_label == true then return field end
  return labelled(c, str(p.label), field, p.width)
end

-- A field of several lines: a well in every skin (an underline under a
-- block of text reads as a rule, not a field), `rows` lines tall at the
-- least, growing with its text up to `max_rows` and scrolling past them.
function M.text_area(c: Ctx, p: P): kui.Node
  focus_request(c, p)
  local focused = c.env.is_focused("edit")
  local value = str(p.value) or ""
  local err = str(p.error)
  local line_h = 17
  local rows = math.max(1, (p.rows or 4) as number)
  local max_rows = p.max_rows as number
  local pad = 8
  local boxed = c.s.skin.field_box
  local edge = err and "danger" or (focused and (boxed and "focus" or "ink") or (boxed and "rule" or "faint"))
  local grow = p.grow == true
  -- A border takes no room, so the thicker edge focused moves nothing.
  local thick = (focused or err ~= nil) and not boxed
  -- The editor is the whole well, its padding inside it: a press anywhere
  -- in the box puts the caret there (in the padding or under the last
  -- line, at the nearest place), where only the lines of text took one.
  local box: kui.Props = {
    key = "box",
    width = "grow",
    height = grow and "grow" or nil,
    max_height = max_rows and (max_rows * line_h + 2 * pad) or nil,
    scroll_y = max_rows ~= nil,
    bg = ref(c, "sunken"),
    radius = "$kitRadius",
    border = { w = thick and "$kitRuleW" or "$kitHair", color = ref(c, edge) },
    edit {
      key = "edit",
      value = value,
      multiline = true,
      -- The core's multiline editor takes Tab by default now (backlog
      -- C99); Kit's prop keeps its meaning until Kit adopts the default.
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
    },
  }
  local field = column(ctx.up_to({ height = grow and "grow" or nil, gap = 0,
    column(box), error_line(c, err) }, p.width, 80))
  if p.hide_label == true or not str(p.label) then return field end
  return column(ctx.up_to({ height = grow and "grow" or nil, gap = 3,
    ctx.muted(c, ctx.label(c, p.label as string), { bold = true }), field }, p.width, 80))
end

function M.search(c: Ctx, p: P): kui.Node
  focus_request(c, p)
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
      label = str(p.label) or str(p.placeholder) or "Search",
      width = "grow",
      size = "$kitSize",
      family = "$kitUi",
      color = ref(c, "fg"),
      on_change = act(c, "search.change"),
      on_submit = act(c, "search.submit", { text = value }),
      on_cancel = act(c, "search.clear"),
      key_focus = p.autofocus == true or nil,
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
  well(c, box, focused, false)
  return column { width = "grow", row(box), underline(c, focused, false) }
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
    pad = { x = 8 },
    gap = 6,
    -- Opened on the press, as a native pop-up is, and the press carries
    -- on into the menu: held, dragged onto an option and let go there, it
    -- picks it; a quick click leaves the menu open. With the menu open a
    -- press here is outside it and arrives as `select.dismiss`.
    on_click = act(c, "select.open"),
    opens_on_press = true,
    ctx.text(c, current or str(p.placeholder) or "Choose…", {
      color = ref(c, current and "fg" or "faint"), max_lines = 1, ellipsis = true,
    }),
    ctx.grow(),
    icons.node(c, st.open and "chevron_up" or "chevron_down", 12, "muted"),
  })
  -- Up to its width, less in a narrow row, its choice ellipsized.
  ctx.up_to(field, p.width, 80)
  if st.open then
    -- The menu opens with the current option over the field, as a native
    -- pop-up's does: lifted by the rows above it (each `$kitRow` and a
    -- 1 px gap), its padding and its edge, and centred on the field's
    -- height; `fit` keeps it in the window. With nothing chosen it hangs
    -- under the field.
    local float: {any:any} = MENU_FLOAT
    local at = 0
    for i, o in ipairs(options) do
      if o.value == p.value then at = i end
    end
    if at > 0 then
      local row_h, ctl_h = skin.row_h(c.s), skin.ctl_h(c.s)
      local edge = c.s.hair or c.s.skin.hair
      float = { anchor = "parent", at = { "start", "start" }, self = { "start", "start" }, fit = true,
        dy = -(4 + edge + (at - 1) * (row_h + 1)) + (ctl_h - row_h) / 2 }
    end
    local list: kui.Props = {
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
    return labelled(c, str(p.label), column(ctx.up_to({ row(field), column(list) }, p.width, 80)), p.width)
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

-- A mark (a box, a dot, a track) `w`×`h` with the skin's ring 2 px round
-- it while the keyboard holds its row: the ring is always there, shown by
-- a rule on the hover group the row and it share, so focus moving
-- repaints and runs no view (Kit read the focus in Lua).
local function ringed_mark(c: Ctx, mark: kui.Node, group: string, w: number, h: number, radius: number): kui.Node
  local m = mark as kui.Props
  local ring = ctx.focus_ring(c, w, h, radius) as kui.Props
  ring.hover_group = group
  ring.opacity = 0
  ring.when = { { group_focus_visible = true, opacity = 1 } }
  m[#m + 1] = ring as kui.Node
  return mark
end

function M.checkbox(c: Ctx, p: P): kui.Node
  local on, mixed = p.checked == true, p.mixed == true
  local group = "kit.mark." .. tostring(c.inst)
  local mark = ringed_mark(c, tickbox(c, on, mixed, false), group, 14, 14, c.s.skin.radius > 0 and 3 or 0)
  return row {
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
  }
end

-- How long a switch's knob takes to cross, in ms: `transition` is ms (it
-- was 0.12, so the knob jumped and its colour swapped in one frame).
local SLIDE = 140

function M.switch(c: Ctx, p: P): kui.Node
  local on = p.on == true
  local bordered = c.s.skin.ctl_border
  local round = c.s.skin.radius > 0
  -- The knob is one thing in both states — the raised colour, outlined
  -- in the ink where controls are outlined — and only its place moves;
  -- the track fills with the accent behind it. A knob that changed colour
  -- with the state read as two different marks, not one that moved.
  -- Round skins: a raised disc, as a platform switch has. Square ones: a
  -- solid block of the ink (Ledger's printed mark), or, where the ink is
  -- the accent itself (Phosphor), the page colour edged in the ink, so it
  -- shows on the lit track as on the dark one. A hollow square read as a
  -- checkbox sitting in the track.
  local sk = c.s.skin
  local ink_is_accent = sk.dark.ink == sk.dark.accent
  -- Where the ink is the accent (Phosphor) the off switch was a hollow
  -- square in a lit frame, a checkbox in a box: off is now a dim track
  -- edged in the faint ink and a solid knob in the muted one, on the lit
  -- track and a page-coloured knob edged in the ink.
  local knob_bg = round and "raised" or (ink_is_accent and (on and "bg" or "muted") or "ink")
  local knob = column {
    key = "knob", width = 14, height = 14, radius = round and 7 or 0, transition = SLIDE, slide = true,
    when = { ctx.still() },
    bg = ref(c, knob_bg),
    border = (not round and ink_is_accent and on) and { w = "$kitHair", color = ref(c, "ink") } or nil,
  }
  local off_edge = ink_is_accent and "faint" or (bordered and "ink" or "rule")
  local track: kui.Props = {
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
  return row {
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
  }
end

function M.radio_group(c: Ctx, p: P): kui.Node
  local options = (p.options or {}) as {P}
  local t: kui.Props = { role = "radioGroup", label = str(p.label), gap = 2 }
  if str(p.label) and p.hide_label ~= true then t[#t + 1] = ctx.muted(c, ctx.label(c, p.label as string), { bold = true }) end
  for i, o in ipairs(options) do
    local here = o.value == p.value
    local group = "kit.mark." .. tostring(c.inst) .. "." .. i
    t[#t + 1] = row {
      key = "r" .. i,
      hover_group = group,
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
      focus_ring = 0xffffff00,
      ringed_mark(c, tickbox(c, here, false, true), group, 14, 14, 7),
      ctx.text(c, o.label as string),
    }
  end
  return column(t)
end

function M.segmented(c: Ctx, p: P): kui.Node
  local options = (p.options or {}) as {P}
  -- A track with the chosen segment filled, in every skin: an outline and
  -- a rule between each segment drew a cage of edges around three words.
  -- Where controls are outlined (Ledger, Phosphor) the chosen one is
  -- printed in the ink, the page's colour on it: the accent stays the one
  -- action's, and a view switch is not one. The track is the pressed wash,
  -- so it reads on any surface (Ledger's sunken was its toolbar's colour).
  local bordered = c.s.skin.ctl_border
  local on_bg = bordered and "ink" or "panel"
  local on_fg = bordered and "bg" or "fg"
  local t: kui.Props = {
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
    local seg: kui.Props = {
      key = "s" .. i,
      role = "radio",
      label = o.label as string,
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
      ctx.caption(c, o.label as string, { color = ref(c, here and on_fg or "fg") }),
      -- An alert count is in the danger colour off the chosen segment; on
      -- it, in its ink and bold (the danger colour has no contrast on ink).
      o.count ~= nil and ctx.data(c, tostring(o.count), { size = "$kitSizeLabel", bold = o.alert == true,
        color = ref(c, here and on_fg or (o.alert == true and "danger" or "muted")) }) or nil,
    }
    t[#t + 1] = row(seg)
  end
  return row(t)
end

function M.slider(c: Ctx, p: P): kui.Node
  local lo, hi = (p.min or 0) as number, (p.max or 1) as number
  local v = math.max(lo, math.min(hi, (p.value or lo) as number))
  local f = hi > lo and (v - lo) / (hi - lo) or 0
  local step = (p.step or (hi - lo) / 100) as number
  -- The value as the host wrote it (`20 MB/s`), or the number itself.
  local shown = str(p.value_label) or string.format("%g", v)
  -- The handle lights while the pointer is over the track and stays lit,
  -- a shade deeper, while it is held there and dragged: the track and the
  -- handle share a hover group, and the handle's look is rules on that
  -- group's state (ADR 0069), so a hover repaints and the slider replays.
  -- A press on the handle passes to the track (C91), so a drag can start
  -- on it. The whole track is taken hold of: an open hand over it, a
  -- closed one while it is held.
  local group = "kit-slider-" .. tostring(c.inst)
  local track = row {
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
    -- Half the handle each side: the core maps the pointer along the
    -- content box (ADR 0034), and the handle's centre sits at the value's
    -- place in it. The handle was laid out in the row after the fill, so
    -- its centre sat half a handle past the value's place and more toward
    -- the far end: a drag landed a step past the one under the pointer.
    pad = { x = 6 },
    cross_align = "center",
    cursor = "grab",
    when = { { pressed = true, cursor = "grabbing" } },
    column { key = "fill", width = { pct = f * 100 }, height = 4, radius = 2, bg = ref(c, "accent"),
      -- Centred on the fill's end: the value's place, over the rail.
      column {
        key = "handle",
        float = { anchor = "parent", at = { "end", "center" }, self = { "center", "center" } },
        hover_group = group,
        width = 12, height = 12, radius = c.s.skin.radius > 0 and 6 or 0,
        bg = ref(c, "panel"),
        border = { w = "$kitRuleW", color = ref(c, "ink") },
        cursor = "grab",
        when = {
          { hovered = true, bg = ref(c, "accent") },
          { pressed = true, bg = ref(c, "accent_press"), cursor = "grabbing" },
        },
      } },
    column { width = "grow", height = 4, radius = 2, bg = ref(c, "rule") },
  }
  if not str(p.label) then return track end
  return column {
    width = "grow",
    gap = 2,
    row { width = "grow", ctx.muted(c, ctx.label(c, p.label as string), { bold = true }), ctx.grow(),
      ctx.data(c, shown, { size = "$kitSizeLabel", color = ref(c, "fg") }) },
    track,
  }
end

-- The events: what each action proposes to the host.
function M.event(m: P, kit: string): any
  if kit == "button.press" or kit == "icon_button.press" then
    return emit("on_press", {})
  elseif kit == "text_field.change" or kit == "search.change" or kit == "text_area.change" then
    local ch = m.change as P
    return emit("on_change", { text = ch and ch.text or "" })
  elseif kit == "text_field.browse" then
    local st = ctx.peek(m.inst as integer) as Asked
    if st then st.browse = true end
  elseif kit == "text_field.picked" then
    local f = m.files as P
    local paths = f and f.paths as {string}
    if paths and paths[1] then return emit("on_change", { text = paths[1] }) end
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
