-- Layout: the regions a tool's window is made of. They hold the host's
-- children in named groups (`children("actions")`), so a panel's header
-- carries the host's own buttons, keyed and heard as the host's.

local ctx = require("ctx")
local icons = require("icons")

local type Ctx = ctx.Ctx
local type P = {string:any}
local ref, act = ctx.ref, ctx.act

local M = {}

local function str(v: any): string
  return v is string and v or nil
end

-- A region's frame: the skin's rule around it (Ledger's 2px ink, a
-- hairline in Neutral and Phosphor) on the panel colour.
local function region(c: Ctx, t: kui.Props): kui.Props
  t.bg = ref(c, "panel")
  t.radius = "$kitRadius"
  t.clip = true
  t.border = { w = "$kitRuleW", color = ref(c, c.s.skin.ctl_border and "ink" or "rule") }
  return t
end

function M.panel(c: Ctx, p: P): kui.Node
  local grow = p.grow == true
  local t = region(c, { width = "grow", height = grow and "grow" or nil, label = str(p.title), role = "group" })
  if str(p.title) then
    local head: kui.Props = {
      width = "grow", height = "$kitCtl", pad = { x = 10 }, gap = 8, cross_align = "center",
      ctx.caption(c, p.title as string, { size = "$kitSizeLabel", bold = true }),
    }
    if str(p.subtitle) then head[#head + 1] = ctx.muted(c, p.subtitle as string, { max_lines = 1, ellipsis = true }) end
    head[#head + 1] = ctx.grow()
    head[#head + 1] = row { gap = 4, cross_align = "center", children("actions") }
    t[#t + 1] = row(head)
    t[#t + 1] = ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", c.s.skin.ctl_border and "$kitRuleW" or "$kitHair")
  end
  t[#t + 1] = column {
    key = "body",
    width = "grow",
    height = (grow or p.scroll == true) and "grow" or nil,
    scroll_y = p.scroll == true,
    pad = (p.pad as number) or 10,
    gap = 8,
    children(""),
  }
  t[#t + 1] = column {
    width = "grow",
    children("footer"),
  }
  return column(t)
end

function M.toolbar(c: Ctx, _p: P): kui.Node
  return column {
    width = "grow",
    bg = ref(c, "side"),
    row {
      width = "grow",
      height = 38,
      pad = { x = 8 },
      gap = 6,
      cross_align = "center",
      -- The unnamed group takes the room: a host's own spacer in it pushes
      -- what follows to the group's end, where a spacer of Kit's beside it
      -- split the room with it and left that midway.
      row { width = "grow", gap = 6, cross_align = "center", children("") },
      row { gap = 6, cross_align = "center", children("trailing") },
    },
    ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair"),
  }
end

function M.statusbar(c: Ctx, _p: P): kui.Node
  return column {
    width = "grow",
    bg = ref(c, "side"),
    ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair"),
    row {
      width = "grow", height = 22, pad = { x = 10 }, gap = 14, cross_align = "center",
      row { width = "grow", gap = 14, cross_align = "center", children("") },
      row { gap = 14, cross_align = "center", children("trailing") },
    },
  }
end

function M.split(c: Ctx, p: P): kui.Node
  local column_axis = p.axis == "column"
  local f = math.max(0, math.min(1, (p.fraction or 0.5) as number))
  -- A 6 px strip to grab, drawing the region rule down its middle; the
  -- strip lights in the accent while hovered or held.
  local line: kui.Props = { bg = ref(c, c.s.skin.ctl_border and "ink" or "rule") }
  local handle: kui.Props = {
    key = "rule",
    role = "none",
    keep_focus = true,
    main_align = "center",
    cross_align = "center",
    hover_bg = ref(c, "hover"),
    cursor = column_axis and "nsResize" or "ewResize",
    on_drag = act(c, "split.drag", { column = column_axis, min = (p.min or 80) as number }),
  }
  local first: kui.Props = { key = "first", clip = true, children("first") }
  local second: kui.Props = { key = "second", clip = true, children("second") }
  if column_axis then
    handle.width, handle.height = "grow", 6
    line.width, line.height = "grow", "$kitRuleW"
    handle[1] = column(line)
    first.width, first.height = "grow", { pct = f * 100 }
    second.width, second.height = "grow", "grow"
    return column { width = "grow", height = "grow", column(first), column(handle), column(second) }
  end
  handle.width, handle.height = 6, "grow"
  line.width, line.height = "$kitRuleW", "grow"
  handle[1] = column(line)
  first.width, first.height = { pct = f * 100 }, "grow"
  second.width, second.height = "grow", "grow"
  return row { width = "grow", height = "grow", column(first), row(handle), column(second) }
end

function M.tabs(c: Ctx, p: P): kui.Node
  local tabs = (p.tabs or {}) as {P}
  local strip: kui.Props = { role = "tabList", width = "grow", height = "$kitCtl", gap = 2, cross_align = "end" }
  for i, tb in ipairs(tabs) do
    local here = tb.value == p.value
    local t: kui.Props = {
      key = "t" .. i,
      role = "tab",
      label = tb.label as string,
      selected = here,
      height = "grow",
      pad = { x = 10 },
      gap = 6,
      cross_align = "center",
      cursor = "pointer",
      hover_bg = not here and ref(c, "hover") or nil,
      on_click = act(c, "tabs.change", { value = tb.value }),
      ctx.caption(c, tb.label as string, { color = ref(c, here and "fg" or "muted") }),
    }
    if tb.count ~= nil then
      t[#t + 1] = ctx.data(c, tostring(tb.count), { size = "$kitSizeLabel", color = ref(c, "faint") })
    end
    strip[#strip + 1] = column {
      key = "tab" .. i,
      height = "grow",
      row(t),
      column { width = "grow", height = "$kitRuleW", bg = here and ref(c, "accent") or nil },
    }
  end
  -- What acts on the chosen tab — close it, run it — at the strip's far side.
  strip[#strip + 1] = ctx.grow()
  strip[#strip + 1] = row { height = "grow", gap = 4, cross_align = "center", pad = { r = 4 }, children("trailing") }
  return column {
    width = "grow",
    height = "grow",
    row(strip),
    ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair"),
    column { key = "body", width = "grow", height = "grow", children("") },
  }
end

local record Folded
  collapsed: boolean
end

function M.section(c: Ctx, p: P): kui.Node
  -- Folded is the host's when it passes `collapsed`, Kit's otherwise.
  local st = ctx.state(c, function(): Folded return { collapsed = false } end)
  local collapsed: boolean
  if p.collapsed ~= nil then collapsed = p.collapsed == true else collapsed = st.collapsed end
  local head: kui.Props = {
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
  local t: kui.Props = { width = "grow", gap = 4, row(head) }
  if not collapsed then
    t[#t + 1] = column { key = "body", width = "grow", gap = 6, pad = { l = 18 }, children("") }
  end
  return column(t)
end

function M.divider(c: Ctx, _p: P): kui.Node
  return ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", "$kitHair")
end

function M.event(m: P, kit: string): any
  if kit == "split.drag" then
    local d = m.drag as P
    local parent = d and d.parent as P
    if not parent then return nil end
    local column_axis = m.column == true
    local at = (column_axis and d.y or d.x) as number
    local start = (column_axis and parent.y or parent.x) as number
    local size = (column_axis and parent.h or parent.w) as number
    if size <= 0 then return nil end
    local min = math.min((m.min or 80) as number, size / 2)
    local px = math.max(min, math.min(size - min, at - start))
    return emit("on_resize", { fraction = px / size })
  elseif kit == "tabs.change" then
    return emit("on_change", { value = m.value })
  elseif kit == "section.toggle" then
    local st = ctx.peek(m.inst as integer) as Folded
    if st then st.collapsed = m.collapsed == true end
    return emit("on_toggle", { collapsed = m.collapsed == true })
  end
  return nil
end

return M
