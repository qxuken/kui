-- Data: views over a channel (ADR 0065) and the small pieces a tool shows
-- numbers with. The channel views build only the rows in view, read cell
-- by cell from the channel, so a hundred thousand rows cost a screenful;
-- an unchanged channel revision and scroll offset replay without asking.

local ctx = require("ctx")
local icons = require("icons")

local type Ctx = ctx.Ctx
local type P = {string:any}
local ref, act = ctx.ref, ctx.act

local M = {}

local function str(v: any): string
  return v is string and v or nil
end

-- A cell as text: integers bare, other numbers to six figures.
function M.cell_text(v: any): string
  if v == nil then return "" end
  if v is integer then return tostring(v) end
  if v is number then
    if v == math.floor(v) and math.abs(v) < 1e15 then return string.format("%d", math.floor(v)) end
    return string.format("%.6g", v)
  end
  return tostring(v)
end
local cell_text = M.cell_text

local function count(ch: kui.Channel): integer
  if ch == nil then return 0 end
  return #ch
end

-- The rows [first, last) of `n` uniform rows of `row_h` a scroller keyed
-- `key` shows, two rows over each side: last frame's geometry, the
-- viewport on the first.
local function slice(env: kui.Env, key: string, n: integer, row_h: number): integer, integer
  local g = env.scroll_geometry(key)
  local vh = g and g.h or env.viewport_h
  local top = g and g.offset.y or 0
  local first = math.min(n, math.max(0, math.floor(top / row_h) - 2))
  local last = math.min(n, math.ceil((top + math.max(vh, 0)) / row_h) + 2)
  if last < first then last = first end
  return first, last
end

-- Scrolls `key` so row `i` shows, when it does not.
local function reveal(env: kui.Env, key: string, i: integer, row_h: number)
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

-- A virtual body: `build(i)` makes row i's node, keyed by its index; the
-- spacers keep the height of the rest.
local function body(c: Ctx, key: string, n: integer, row_h: number, t: kui.Props, build: function(i: integer): kui.Node): kui.Node
  local first, last = slice(c.env, key, n, row_h)
  t.key = key
  t.width = t.width or "grow"
  t.scroll_y = true
  t.row_count = n
  if first > 0 then t[#t + 1] = column { key = "kit:lead", width = "grow", height = first * row_h } end
  for i = first, last - 1 do t[#t + 1] = build(i) end
  if last < n then t[#t + 1] = column { key = "kit:tail", width = "grow", height = (n - last) * row_h } end
  return column(t)
end

local record Seen
  selected: integer
  n: integer
end

-- Up, Down, Home, End and the pages from `selected` in `n` rows.
local function step(code: string, selected: integer, n: integer, page: integer): integer
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

function M.table(c: Ctx, p: P): kui.Node
  local ch = p.rows as kui.Channel
  local cols = (p.columns or {}) as {P}
  local n = count(ch)
  local row_h = c.s.density == "compact" and 20 or 22
  local env = c.env
  local selected = p.selected as integer
  local seen = ctx.state(c, function(): Seen return {} end)
  if selected ~= seen.selected then
    if selected then reveal(env, "rows", selected, row_h) end
    seen.selected = selected
  end

  local head: kui.Props = { width = "grow", height = row_h + 4, pad = { x = 10 }, gap = 10, cross_align = "center", bg = ref(c, "panel") }
  for i, col in ipairs(cols) do
    local field = col.field as string
    local sorted = p.sort == field
    local h: kui.Props = {
      key = "h" .. i,
      role = "button",
      label = (col.title as string) .. (sorted and (p.descending == true and ", sorted descending" or ", sorted ascending") or ""),
      width = (col.width as number) or "grow",
      height = "grow",
      gap = 4,
      cross_align = "center",
      main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
      cursor = "pointer",
      on_click = act(c, "table.sort", { field = field }),
      ctx.caption(c, col.title as string, { color = ref(c, sorted and "fg" or "muted") }),
    }
    if sorted then h[#h + 1] = icons.node(c, p.descending == true and "chevron_down" or "chevron_up", 11, "fg") end
    head[#head + 1] = row(h)
  end

  local striped = p.striped == true
  local view = body(c, "rows", n, row_h, {
    height = "grow",
    focusable = true,
    on_key = act(c, "table.key", { selected = selected, n = n, page = math.max(1, math.floor((env.viewport_h or 400) / row_h) - 1) }),
    on_range = act(c, "table.range"),
    label = "rows",
  }, function(i: integer): kui.Node
    local sel = selected == i
    local line: kui.Props = {
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
    local name: {string} = {}
    for j, col in ipairs(cols) do
      local s = cell_text(ch:get(i, col.field as string))
      name[#name + 1] = s
      line[#line + 1] = row {
        width = (col.width as number) or "grow",
        main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
        text(s, {
          size = "$kitSize",
          color = ink,
          family = col.mono == true and "$kitData" or "$kitUi",
          max_lines = 1,
          ellipsis = true,
        }),
      }
    end
    line.label = table.concat(name, ", ")
    return row(line)
  end)

  return column {
    width = "grow",
    height = p.grow == true and "grow" or nil,
    bg = ref(c, "panel"),
    row(head),
    ctx.rule(c, c.s.skin.ctl_border and "ink" or "rule", c.s.skin.ctl_border and "$kitRuleW" or "$kitHair"),
    n == 0 and column { width = "grow", pad = 16, ctx.muted(c, "No rows") } or view,
  }
end

function M.list(c: Ctx, p: P): kui.Node
  local ch = p.rows as kui.Channel
  local n = count(ch)
  local two = str(p.detail) ~= nil
  local row_h = two and 40 or (c.s.density == "compact" and 22 or 26)
  local selected = p.selected as integer
  local seen = ctx.state(c, function(): Seen return {} end)
  if selected ~= seen.selected then
    if selected then reveal(c.env, "rows", selected, row_h) end
    seen.selected = selected
  end
  return body(c, "rows", n, row_h, {
    height = p.grow == true and "grow" or nil,
    role = "list",
    focusable = true,
    on_key = act(c, "list.key", { selected = selected, n = n, page = 8 }),
  }, function(i: integer): kui.Node
    local sel = selected == i
    local title = cell_text(ch:get(i, p.title as string))
    local t: kui.Props = {
      index = i,
      role = "listItem",
      label = title,
      selected = sel,
      height = row_h,
      width = "grow",
      pad = { x = 10 },
      main_align = "center",
      gap = 1,
      bg = sel and ref(c, "sel") or nil,
      hover_bg = sel and ref(c, "sel") or ref(c, "hover"),
      on_click = act(c, "list.select", { row = i }),
      ctx.text(c, title, { color = ref(c, sel and "sel_fg" or "fg"), max_lines = 1, ellipsis = true }),
    }
    if two then
      t[#t + 1] = ctx.muted(c, cell_text(ch:get(i, p.detail as string)), { max_lines = 1, ellipsis = true,
        color = ref(c, sel and "sel_fg" or "muted") })
    end
    return column(t)
  end)
end

function M.inspector(c: Ctx, p: P): kui.Node
  local entries = (p.entries or {}) as {P}
  local kw = (p.key_width or 140) as number
  local t: kui.Props = { width = "grow", gap = 2, selectable = true }
  for i, e in ipairs(entries) do
    t[#t + 1] = row {
      key = "e" .. i,
      width = "grow",
      gap = 10,
      row { width = kw, ctx.muted(c, e.key as string, { max_lines = 1, ellipsis = true }) },
      row { width = "grow",
        (e.mono == true and ctx.data or ctx.text)(c, cell_text(e.value), { wrap = "word" }) },
    }
  end
  return column(t)
end

local LEVEL_ROLE: {string:string} = { debug = "faint", info = "muted", warn = "warn", error = "danger" }

local record Follow
  n: integer
end

function M.log(c: Ctx, p: P): kui.Node
  local ch = p.rows as kui.Channel
  local n = count(ch)
  local row_h = 18
  local env = c.env
  local st = ctx.state(c, function(): Follow return { n = 0 } end)
  if ctx.flag(p.follow, true) and n > st.n then
    -- At the end before the rows arrived (or on the first frame): stay there.
    local g = env.scroll_geometry("rows")
    local at_end = not g or g.offset.y + g.h >= st.n * row_h - row_h
    if at_end then env.set_scroll("rows", 0, n * row_h) end
  end
  st.n = n
  return body(c, "rows", n, row_h, {
    height = p.grow == true and "grow" or nil,
    bg = ref(c, "sunken"),
    pad = { y = 4 },
    selectable = true,
  }, function(i: integer): kui.Node
    local level = cell_text(ch:get(i, "level"))
    return row {
      index = i,
      height = row_h,
      width = "grow",
      pad = { x = 8 },
      gap = 10,
      cross_align = "center",
      hover_bg = ref(c, "hover"),
      on_click = act(c, "log.select", { row = i }),
      ctx.data(c, cell_text(ch:get(i, "time")), { size = "$kitSizeLabel", color = ref(c, "faint") }),
      row { width = 44, ctx.data(c, level:upper(), { size = "$kitSizeLabel", bold = true, color = ref(c, LEVEL_ROLE[level] or "muted") }) },
      ctx.data(c, cell_text(ch:get(i, "message")), { size = "$kitSizeLabel", max_lines = 1, ellipsis = true }),
    }
  end)
end

-- A tone's ink: what a badge's text and border are drawn in.
local function tone_ink(tone: string): string
  if tone == "ok" or tone == "warn" or tone == "danger" then return tone end
  if tone == "accent" then return "on_accent" end
  return "muted"
end

function M.badge(c: Ctx, p: P): kui.Node
  local tone = str(p.tone) or "neutral"
  local accent = tone == "accent"
  local label = str(p.label) or ""
  local t: kui.Props = {
    label = label,
    height = 18,
    pad = { x = 6 },
    cross_align = "center",
    radius = c.s.skin.radius > 0 and 9 or 0,
    bg = ref(c, accent and "accent" or "sunken"),
    -- As written: a badge carries data (a status, a version, a count),
    -- which the skin's label casing would garble.
    ctx.data(c, label, { size = 10.5, bold = true, color = ref(c, tone_ink(tone)), max_lines = 1 }),
  }
  if tone ~= "neutral" and not accent then
    t.border = { w = "$kitHair", color = ref(c, tone) }
  end
  return row(t)
end

function M.stat(c: Ctx, p: P): kui.Node
  local tone = str(p.tone)
  local t: kui.Props = {
    role = "group",
    label = (str(p.label) or "") .. ": " .. (str(p.value) or ""),
    gap = 2,
    ctx.muted(c, ctx.label(c, str(p.label) or ""), { bold = true }),
    ctx.data(c, str(p.value) or "", { size = 22, bold = true }),
  }
  if str(p.delta) then
    local role = (tone == "ok" or tone == "warn" or tone == "danger") and tone or "muted"
    t[#t + 1] = ctx.data(c, p.delta as string, { size = "$kitSizeLabel", color = ref(c, role) })
  end
  return column(t)
end

function M.code(c: Ctx, p: P): kui.Node
  local wrap = p.wrap == true
  return column {
    width = "grow",
    bg = ref(c, "sunken"),
    radius = "$kitRadius",
    pad = 8,
    scroll_x = not wrap,
    selectable = true,
    ctx.data(c, str(p.text) or "", { wrap = wrap and "word" or "none" }),
  }
end

function M.empty(c: Ctx, p: P): kui.Node
  local t: kui.Props = {
    width = "grow",
    height = "grow",
    -- Its content at least, in a parent that only fits its children.
    min_height = "fit",
    main_align = "center",
    cross_align = "center",
    gap = 6,
    pad = 24,
    text(str(p.title) or "", { size = "$kitSizeTitle", bold = true, family = "$kitUi", color = ref(c, "fg") }),
  }
  if str(p.detail) then
    t[#t + 1] = ctx.muted(c, p.detail as string, { size = "$kitSize", wrap = "word" })
  end
  t[#t + 1] = row { gap = 8, pad = { t = 6 }, children("") }
  return column(t)
end

function M.progress(c: Ctx, p: P): kui.Node
  local v = p.value as number
  local bar: kui.Props
  if v then
    v = math.max(0, math.min(1, v))
    bar = { width = { pct = v * 100 }, height = "grow", bg = ref(c, "accent") }
  else
    -- Under way: a third of the track sliding across it.
    bar = {
      width = { pct = 30 }, height = "grow", bg = ref(c, "accent"),
      keyframes = { { dx = -80 }, { dx = 400 } }, transition = 1400,
    }
  end
  local track = row {
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
  }
  if not str(p.label) then return track end
  return column { width = "grow", gap = 4, ctx.muted(c, p.label as string), track }
end

function M.event(m: P, kit: string): any
  if kit == "table.select" or kit == "list.select" or kit == "log.select" then
    return emit("on_select", { row = m.row })
  elseif kit == "table.sort" then
    return emit("on_sort", { field = m.field })
  elseif kit == "table.range" then
    local r = m.range as P
    if r then return emit("on_range", { first = r.first, last = r.last }) end
  elseif kit == "table.key" or kit == "list.key" then
    local k = m.key as P
    if not k or k.phase ~= "down" then return nil end
    local n = (m.n or 0) as integer
    if n == 0 then return nil end
    local s = step(k.code as string, m.selected as integer, n, (m.page or 8) as integer)
    if s and s ~= m.selected then return emit("on_select", { row = s }) end
  end
  return nil
end

return M
