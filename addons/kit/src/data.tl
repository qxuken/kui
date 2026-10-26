-- Data: views over a channel (ADR 0065) and the small pieces a tool shows
-- numbers with. The channel views build only the rows in view, read cell
-- by cell from the channel, so a hundred thousand rows cost a screenful;
-- an unchanged channel revision and scroll offset replay without asking.

local ctx = require("ctx")
local icons = require("icons")
local inputs = require("inputs")
local skin = require("skin")

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

-- What a list or a table keeps between fills: a type-ahead the core
-- passed on, answered by the next fill (an event handler holds no env),
-- and what the search reads.
local record Seen
  ask: string
  ch: kui.Channel
  field: string
end

-- A list's or a table's keyboard is the core's (ADR 0070): a `list` of
-- focusable `listItem`s, or a `table` of focusable `cell`s, under the
-- `row_count` its body declares, steps past its window, pages, enters on
-- the selected row (`active_index`), and draws the ring (in Kit's focus
-- colour, `focus_ring` on every Kit root). It was a key sink that moved
-- the selection by index, scrolled it into view and handed Tab on.
-- Selection is the host's: a click, or the keyboard landing on a row,
-- proposes it. A type-ahead the built rows cannot answer comes here and
-- is answered from the channel.
local function typeahead(c: Ctx, seen: Seen, ch: kui.Channel, field: string)
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

-- A tone a channel cell names, as the role its text is drawn in; nil for
-- none or a word Kit has no tone for.
local TONE_ROLE: {string:string} = { ok = "ok", warn = "warn", danger = "danger", accent = "accent", info = "focus", neutral = "muted" }

local function tone_of(v: any): string
  return v is string and TONE_ROLE[v] or nil
end

-- A small pill for a cell or a row: its text in the tone, a dot before it
-- when `dot`. What a badge draws, sized to sit in a row.
-- Its ground is the pressed wash, which reads on a panel and a sidebar
-- alike; the accent tone fills it, as a badge's does. On a selected row
-- it keeps its chip, on the panel's colour — what its tone's text is made
-- to read on — rather than losing it to the selection's fill, where the
-- tone's colour may not read (a red 500 on a yellow selection).
local function pill(c: Ctx, label: string, tone: string, dot: boolean, sel: boolean): kui.Node
  local role = tone_of(tone)
  local accent = role == "accent"
  local ink = ref(c, accent and "on_accent" or (role or "muted"))
  local spot = accent and "on_accent" or (role == "muted" and "faint" or (role or "faint"))
  return row {
    height = 18, min_width = "fit", pad = { x = 6 }, gap = 5, cross_align = "center",
    radius = c.s.skin.radius > 0 and 9 or 0,
    bg = accent and ref(c, "accent") or ref(c, sel and "panel" or "press"),
    dot and column { width = 6, height = 6, radius = 3, bg = ref(c, spot) } or nil,
    ctx.data(c, label, { size = "$kitSizeLabel", bold = c.s.skin.label_bold, color = ink, max_lines = 1 }),
  }
end

-- Selection in a list or a table: `selection` says which kind — `single`
-- (one row, `selected`), `multiple` (any rows, `selected_ranges`) or
-- `none` — and `select_on_focus` whether the keyboard's moves select (the
-- default) or only move the cursor, Space then selecting and Enter
-- activating. The selection is the host's: Kit proposes it, by
-- `on_select`, and draws what the host says. The cursor — the row the
-- keyboard is on — is the core's focus (ADR 0070), reported by
-- `on_cursor`; Enter and a double click are `on_activate`. A range is
-- Kit's: the anchor it extends from is kept here, the modifiers a click or
-- a step was made under coming with its event (C101 would make ranges
-- the core's).
local record Range
  first: integer
  last: integer
end

local record Sel
  -- `single`, `multiple` or `none`, and whether a move selects.
  mode: string
  on_focus: boolean
  -- What the host holds: a row, or ranges, sorted and apart.
  row: integer
  ranges: {Range}
  -- The rows, the anchor a range extends from, the cursor.
  n: integer
  anchor: integer
  cursor: integer
  -- Whether the host hears activations (else Enter selects, as it did).
  activates: boolean
  mac: boolean
end

-- `ranges` sorted, each inclusive, touching ones joined.
local function merged(ranges: {Range}): {Range}
  local rs: {Range} = {}
  for _, r in ipairs(ranges) do
    local a, b = math.min(r.first, r.last), math.max(r.first, r.last)
    rs[#rs + 1] = { first = a, last = b }
  end
  table.sort(rs, function(x: Range, y: Range): boolean return x.first < y.first end)
  local out: {Range} = {}
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

local function holds(ranges: {Range}, i: integer): boolean
  for _, r in ipairs(ranges) do
    if i >= r.first and i <= r.last then return true end
    if r.first > i then return false end
  end
  return false
end

-- `ranges` with row `i` in it if it was not, out of it if it was.
local function toggled(ranges: {Range}, i: integer): {Range}
  if not holds(ranges, i) then
    local rs: {Range} = { { first = i, last = i } }
    for _, r in ipairs(ranges) do rs[#rs + 1] = r end
    return merged(rs)
  end
  local out: {Range} = {}
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

-- The rows a selection event lists, up to this many; past it, the ranges
-- alone, so ⌘A on 100,000 rows sends one range.
local LISTED = 1000

-- The `on_select` a multiple selection proposes: the row the change was
-- about, the ranges, their count, and the rows when there are few.
local function proposed(st: Sel, row: integer, ranges: {Range}): any
  st.ranges = ranges
  local count = 0
  for _, r in ipairs(ranges) do count = count + r.last - r.first + 1 end
  local rows: {integer} = nil
  if count <= LISTED then
    rows = {}
    for _, r in ipairs(ranges) do
      for i = r.first, r.last do rows[#rows + 1] = i end
    end
  end
  local out: {Range} = {}
  for _, r in ipairs(ranges) do out[#out + 1] = { first = r.first, last = r.last } end
  return emit("on_select", { row = row, ranges = out, count = count, rows = rows })
end

-- What a list or a table holds of the host's selection this fill, kept for
-- the events, and whether row `i` is selected.
local function selection_of(c: Ctx, p: P, n: integer): Sel
  local st = ctx.state(c, function(): Sel return {} end)
  local mode = str(p.selection) or "single"
  st.mode, st.n = mode, n
  st.on_focus = p.select_on_focus ~= false
  st.row = p.selected as integer
  st.activates = p.on_activate ~= nil
  st.mac = c.env.platform == "macos"
  local given = (p.selected_ranges or {}) as {Range}
  st.ranges = mode == "multiple" and merged(given) or {}
  return st
end

local function selected_in(st: Sel, i: integer): boolean
  if st.mode == "single" then return st.row == i end
  if st.mode == "multiple" then return holds(st.ranges, i) end
  return false
end

-- Where Tab enters the rows: the cursor, else the selection's first row.
local function entry_of(st: Sel): integer
  if st.cursor and st.cursor < st.n then return st.cursor end
  if st.mode == "single" then return st.row end
  if st.mode == "multiple" and st.ranges[1] then return st.ranges[1].first end
  return nil
end

-- The keyboard landed on row `i` (an arrow, a page, a type-ahead, Tab
-- back in): the cursor moves, and the selection with it when moves
-- select; Shift held, a multiple selection extends from its anchor.
local function on_cursor_moved(st: Sel, i: integer, mods: P): {any}
  local out: {any} = {}
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

-- Row `i` was clicked or pressed: by Enter (activate), by Space (select,
-- or toggle in a multiple selection), or by the pointer — a double click
-- activates, ⌘ (Ctrl off a Mac) toggles a row of a multiple selection,
-- Shift extends it from the anchor, a plain click selects the row alone.
local function on_row_pressed(st: Sel, i: integer, click: P): {any}
  local out: {any} = {}
  local key = click and click.key as string
  local mods = click and click.mods as P
  local clicks = click and (click.clicks as integer) or 1
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

-- A key the focused row left to its list (`on_item_key`, C120): ⌘A (Ctrl+A
-- off a Mac) selects every row of a multiple selection — the whole count
-- as one range, built rows or not.
local function on_row_key(st: Sel, k: P): {any}
  local out: {any} = {}
  if st.mode ~= "multiple" or st.n == 0 then return out end
  local code = k.code as string
  local primary = (st.mac and k.super == true) or (not st.mac and k.ctrl == true)
  if (code == "a" or code == "A") and primary then
    out[#out + 1] = proposed(st, st.cursor or 0, { { first = 0, last = st.n - 1 } })
  end
  return out
end

function M.table(c: Ctx, p: P): kui.Node
  local ch = p.rows as kui.Channel
  local cols = (p.columns or {}) as {P}
  local n = count(ch)
  local row_h = c.s.density == "compact" and 20 or 22
  local seen = ctx.state(c, function(): Seen return {} end)
  local st = selection_of(c, p, n)
  local cells = p.cells == true
  typeahead(c, seen, ch, cols[1] and cols[1].field as string or "")

  -- Narrow: the columns with a `priority` hide lowest first as the table
  -- narrows past what they need, header and cells alike — each row's own
  -- width rules hide its keyed cells (ADR 0069), so a row and the header
  -- above it, as wide as each other, hide the same ones. What still does
  -- not fit scrolls sideways: the header and the rows are as wide as the
  -- columns left need, in a scroller.
  local function need(col: P): number
    return ((col.width or col.min_width or 80) as number)
  end
  local function width_of(shown: {boolean}): number
    local w, k = 20.0, 0
    for i, col in ipairs(cols) do
      if shown[i] then w, k = w + need(col), k + 1 end
    end
    return w + 10 * math.max(k - 1, 0) + 2
  end
  local shown: {boolean} = {}
  local hides: {integer} = {}
  for i, col in ipairs(cols) do
    shown[i] = true
    if col.priority ~= nil then hides[#hides + 1] = i end
  end
  -- Lowest priority first; of two alike, the later column.
  table.sort(hides, function(a: integer, b: integer): boolean
    local pa, pb = cols[a].priority as number, cols[b].priority as number
    if pa ~= pb then return pa < pb end
    return a > b
  end)
  local head_rules: {{string:any}} = {}
  local cell_rules: {{string:any}} = {}
  local gone_h, gone_c: {string}, {string} = {}, {}
  for _, i in ipairs(hides) do
    local below = width_of(shown)
    shown[i] = false
    gone_h[#gone_h + 1], gone_c[#gone_c + 1] = "h" .. i, "c" .. i
    head_rules[#head_rules + 1] = { width = { below = below, band = 8 }, hidden = { table.unpack(gone_h) } }
    cell_rules[#cell_rules + 1] = { width = { below = below, band = 8 }, hidden = { table.unpack(gone_c) } }
  end
  local least = width_of(shown)

  -- The header on a plane of its own (the sunken colour) rather than over
  -- a rule; `border` frames the whole table and rules the header instead.
  local framed = p.border == true
  -- A table nobody sorts or selects in takes no keyboard: its headers are
  -- captions and its rows no stop. Both were stops that did nothing.
  local sorts = p.on_sort ~= nil
  local selects = p.on_select ~= nil or p.on_cursor ~= nil or p.on_activate ~= nil
  local entry = selects and entry_of(st) or nil
  local keys = (selects and st.mode == "multiple") and act(c, "table.key") or nil
  local head: kui.Props = { width = "grow", height = row_h + 4, pad = { x = 10 }, gap = 10, cross_align = "center",
    bg = ref(c, framed and "panel" or "sunken"), role = cells and "row" or nil,
    when = #head_rules > 0 and head_rules or nil }
  for i, col in ipairs(cols) do
    local field = col.field as string
    local sorted = p.sort == field
    local h: kui.Props = {
      key = "h" .. i,
      -- In a table of cells, a header cell; sorting it is its action.
      role = cells and "columnHeader" or (sorts and "button" or nil),
      focusable = (cells and selects) or nil,
      label = (col.title as string) .. (sorted and (p.descending == true and ", sorted descending" or ", sorted ascending") or ""),
      width = (col.width as number) or "grow",
      height = "grow",
      gap = 4,
      cross_align = "center",
      main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
      cursor = sorts and "pointer" or nil,
      on_click = sorts and act(c, "table.sort", { field = field }) or nil,
      ctx.caption(c, col.title as string, { color = ref(c, sorted and "fg" or "muted") }),
    }
    if sorted then h[#h + 1] = icons.node(c, p.descending == true and "chevron_down" or "chevron_up", 11, "fg") end
    head[#head + 1] = row(h)
  end

  local striped = p.striped == true
  local rows_t: kui.Props = {
    height = "grow",
    on_range = act(c, "table.range"),
    -- A list of rows (the arrows move whole rows), or with `cells` the
    -- body of a table whose cells the arrows walk.
    role = not cells and "list" or nil,
    label = "rows",
    active_index = not cells and entry or nil,
    on_typeahead = selects and act(c, "table.typeahead") or nil,
    on_item_key = not cells and keys or nil,
  }
  local view = body(c, "rows", n, row_h, rows_t, function(i: integer): kui.Node
    local sel = selected_in(st, i)
    local line: kui.Props = {
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
    local name: {string} = {}
    for j, col in ipairs(cols) do
      local s = cell_text(ch:get(i, col.field as string))
      name[#name + 1] = s
      -- A column with a `tone` field draws each cell in the tone its row
      -- names there (a status, an HTTP method); with `badge`, as a pill
      -- with a dot. On the selected row the selection's ink wins.
      local tone = str(col.tone) and cell_text(ch:get(i, col.tone as string)) or nil
      local cell: kui.Node
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
      line[#line + 1] = row {
        key = "c" .. j,
        role = cells and "cell" or nil,
        label = cells and s or nil,
        focusable = (cells and selects) or nil,
        on_focus = (cells and selects) and act(c, "table.focus", { row = i }) or nil,
        -- A focusable cell takes the press, so it carries the row's click:
        -- a click selects as arrowing onto the cell does.
        on_click = (cells and selects) and act(c, "table.select", { row = i }) or nil,
        width = (col.width as number) or "grow",
        min_width = 0,
        cross_align = "center",
        main_align = col.align == "end" and "end" or (col.align == "center" and "center" or "start"),
        cell,
      }
    end
    line.label = table.concat(name, ", ")
    return row(line)
  end)

  return column {
    width = "grow",
    height = p.grow == true and "grow" or nil,
    bg = ref(c, "panel"),
    role = cells and "table" or nil,
    label = cells and "table" or nil,
    active_index = cells and entry or nil,
    on_typeahead = (cells and selects) and act(c, "table.typeahead") or nil,
    on_item_key = cells and keys or nil,
    -- Framed: a hairline in the soft rule colour around the table, inside
    -- which the header sits (the border's width is padding, so the rule
    -- under the header stops at the frame and does not cross it).
    border = framed and { w = "$kitHair", color = ref(c, "rule") } or nil,
    pad = framed and "$kitHair" or nil,
    column { key = "wide", width = "grow", height = p.grow == true and "grow" or nil, scroll_x = true,
      column { key = "sheet", width = "grow", min_width = least, height = p.grow == true and "grow" or nil,
        row(head),
        framed and ctx.rule(c, "rule", "$kitHair") or nil,
        n == 0 and column { width = "grow", pad = 16, ctx.muted(c, "No rows") } or view } },
  }
end

function M.list(c: Ctx, p: P): kui.Node
  local ch = p.rows as kui.Channel
  local n = count(ch)
  local two = str(p.detail) ~= nil
  local row_h = two and 40 or (c.s.density == "compact" and 22 or 26)
  local seen = ctx.state(c, function(): Seen return {} end)
  local st = selection_of(c, p, n)
  typeahead(c, seen, ch, p.title as string)
  -- A list nobody selects in takes no keyboard: no stop that does nothing.
  local selects = p.on_select ~= nil or p.on_cursor ~= nil or p.on_activate ~= nil
  local rows_t: kui.Props = {
    height = p.grow == true and "grow" or nil,
    role = "list",
    label = str(p.label),
    active_index = selects and entry_of(st) or nil,
    on_typeahead = selects and act(c, "list.typeahead") or nil,
    on_item_key = (selects and st.mode == "multiple") and act(c, "list.key") or nil,
  }
  return body(c, "rows", n, row_h, rows_t, function(i: integer): kui.Node
    local sel = selected_in(st, i)
    local title = cell_text(ch:get(i, p.title as string))
    local t: kui.Props = {
      width = "grow",
      min_width = 0,
      main_align = "center",
      gap = 1,
      ctx.text(c, title, { color = ref(c, sel and "sel_fg" or "fg"), max_lines = 1, ellipsis = true }),
    }
    if two then
      t[#t + 1] = ctx.muted(c, cell_text(ch:get(i, p.detail as string)), { max_lines = 1, ellipsis = true,
        color = ref(c, sel and "sel_fg" or "muted") })
    end
    -- A `badge` field puts a pill before the title (a method, a state),
    -- in the tone the row's `tone` field names.
    local badge = str(p.badge) and cell_text(ch:get(i, p.badge as string)) or ""
    local tone = str(p.tone) and cell_text(ch:get(i, p.tone as string)) or nil
    local item: kui.Props = {
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
      badge ~= "" and row { width = p.badge_width as number, min_width = "fit",
        pill(c, badge, tone, false, sel) } or nil,
      column(t),
    }
    return row(item)
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

-- ANSI's eight colours in the skin's: red is danger, green ok, yellow warn,
-- the rest series colours, so a tool's output reads in every skin, light
-- and dark alike. Black and white are the skin's faint and its ink.
local ANSI: {integer:string} = {
  [0] = "faint", [1] = "danger", [2] = "ok", [3] = "warn",
  [4] = "$kitSeries2", [5] = "$kitSeries5", [6] = "$kitSeries6", [7] = "fg",
}

local function ansi_colour(c: Ctx, n: integer): any
  local role = ANSI[n % 8]
  if role:sub(1, 1) == "$" then return role end
  return ref(c, role)
end

local function rgba(r: integer, g: integer, b: integer): integer
  return ((r & 255) << 24) | ((g & 255) << 16) | ((b & 255) << 8) | 0xff
end

-- The 256-colour palette: the sixteen as the skin's, then the cube and the grays.
local function colour256(c: Ctx, n: integer): any
  if n < 16 then return ansi_colour(c, n) end
  if n >= 232 then
    local v = 8 + (n - 232) * 10
    return rgba(v, v, v)
  end
  local function level(v: integer): integer return v == 0 and 0 or 55 + v * 40 end
  n = n - 16
  return rgba(level(n // 36), level(n // 6 % 6), level(n % 6))
end

local record Style
  color: any
  bold: boolean
  italic: boolean
  underline: boolean
end

local function apply_sgr(c: Ctx, st: Style, params: {integer})
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

-- A message as spans in the skin's colours: SGR read, every other escape
-- dropped, and a line a progress bar rewrote with `\r` kept from its last
-- rewrite. A message with no escape is itself.
local function message(c: Ctx, raw: string): string | {any}
  raw = raw:match("[^\r]*$")
  if not raw:find("\27", 1, true) then return raw end
  local spans: {any} = {}
  local st: Style = { bold = false, italic = false, underline = false }
  local at = 1
  while at <= #raw do
    local s, e, params, final = raw:find("\27%[([%d;:?]*)(%a)", at)
    local chunk = raw:sub(at, (s or #raw + 1) - 1)
    if chunk ~= "" then
      if st.color or st.bold or st.italic or st.underline then
        spans[#spans + 1] = { chunk, color = st.color, bold = st.bold or nil, italic = st.italic or nil,
          underline = st.underline or nil }
      else
        spans[#spans + 1] = chunk
      end
    end
    if not s then break end
    if final == "m" then
      local list: {integer} = {}
      for n in (params ~= "" and params or "0"):gmatch("%d+") do list[#list + 1] = math.tointeger(tonumber(n)) end
      apply_sgr(c, st, list)
    end
    at = e + 1
  end
  return spans
end

local record Follow
  n: integer
  -- The channel and its revision drawn last: a new channel is a new revision too.
  rev: string
  width: number
  heights: kui.RowHeights
  -- Kept at the end: on until the reader scrolls up, on again when they
  -- come back to the end or press the jump.
  follow: boolean
  -- Where the scroll stood when the log was last drawn.
  last_y: number
  -- Each row's identity (its time and message) as of the last revision:
  -- what tells a log that only dropped its oldest rows from one whose
  -- rows changed. Nil past `KEYED` rows.
  keys: {string}
end

-- The most rows a log keeps identities for; past it, a new revision is
-- taken as new rows throughout.
local KEYED = 20000

-- How many rows a new revision of a log dropped from its front, when the
-- rest are the rows it had (the newest appended after them); nil when the
-- rows are not the ones it had. 0 is the same rows, or rows only added.
local function dropped_front(st: Follow, n: integer, key_of: function(integer): string): integer
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

-- Whether a log keeps to its end, from where its scroller stands: any
-- move up is the reader's and stops it (a wheel's first notch, not a
-- screenful later — the old test, "within a row of the end", pinned a
-- glide that had only begun back down); reaching the end starts it again.
local function following(st: Follow, g: kui.ScrollGeometry)
  if st.follow == nil then st.follow = true end
  if not g then return end
  local y = g.offset.y
  -- At the end first: a log that grew taller clamps its scroll up to a
  -- new, nearer end, which is still the end.
  if y >= g.max_offset.y - 1 then
    st.follow = true
  elseif st.last_y and y < st.last_y - 0.5 then
    st.follow = false
  end
  st.last_y = y
end

-- What a log shows while it is not keeping to its end: a button over its
-- foot that takes it back there and keeps it.
local function jump(c: Ctx): kui.Node
  local t: kui.Props = {
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

function M.log(c: Ctx, p: P): kui.Node
  local ch = p.rows as kui.Channel
  local n = count(ch)
  local row_h = 18
  local env = c.env
  local wrap = p.wrap == true
  local with_time = p.time ~= false
  local st = ctx.state(c, function(): Follow return { n = 0, rev = "" } end)
  local function line(i: integer): kui.Node
    local level = cell_text(ch:get(i, "level"))
    local text_opts: {string:any} = { size = "$kitSizeLabel" }
    if wrap then text_opts.wrap = "word" else text_opts.max_lines, text_opts.ellipsis = 1, true end
    return row {
      index = i,
      height = not wrap and row_h or nil,
      width = "grow",
      pad = { x = 8, y = wrap and 1 or 0 },
      gap = 10,
      cross_align = wrap and "start" or "center",
      hover_bg = ref(c, "hover"),
      -- A line, not a control: a click and no role made every line a Tab stop.
      role = "listItem",
      on_click = act(c, "log.select", { row = i }),
      with_time and ctx.data(c, cell_text(ch:get(i, "time")), { size = "$kitSizeLabel", color = ref(c, "faint") }) or nil,
      row { width = 44, ctx.data(c, level:upper(), { size = "$kitSizeLabel", bold = true, color = ref(c, LEVEL_ROLE[level] or "muted") }) },
      row { width = "grow", min_width = 0,
        ctx.data(c, message(c, cell_text(ch:get(i, "message"))) as string, text_opts) },
    }
  end
  local wants = ctx.flag(p.follow, true)
  following(st, env.scroll_geometry("rows"))
  -- A new revision: the same rows (a feed republished each tick), rows
  -- added at the end, or the oldest dropped from the front as the newest
  -- came — or other rows altogether. Worked out from the rows' identities.
  local rev = ch and (tostring(ch.id) .. ":" .. tostring(ch.rev)) or ""
  local dropped: integer = nil
  if rev ~= st.rev then
    local function key_of(i: integer): string
      return cell_text(ch:get(i, "time")) .. "\31" .. cell_text(ch:get(i, "message"))
    end
    dropped = dropped_front(st, n, key_of)
    if n <= KEYED then
      local keys: {string} = {}
      for i = 0, n - 1 do keys[i + 1] = key_of(i) end
      st.keys = keys
    else
      st.keys = nil
    end
  end
  -- The log and, while it is not at its end, the jump back there.
  -- Always the same box around the rows, so the scroller keeps its key
  -- (and its offset) whether or not the jump is in it.
  -- Following: the scroller is `anchor`ed, which keeps an anchored node
  -- left at its end at its end in the core's own layout, the frame its
  -- box or its rows change size. The pin below came a frame late when
  -- only the box moved (a panel above it grew): this view was replayed,
  -- not asked, and the rows sat a band short of the end for that frame.
  -- Only while following: a list re-anchors its window itself when its
  -- estimate moves, and the core's anchoring would add the move again.
  local tails = wants and st.follow and n > 0
  local function framed(t: kui.Node): kui.Node
    local away = wants and not st.follow and n > 0
    return column { width = "grow", height = p.grow == true and "grow" or nil, t, away and jump(c) or nil }
  end
  if not wrap then
    if wants and st.follow and n > 0 then
      -- Past the end: the core clamps it to the end as laid out.
      env.set_scroll("rows", 0, 1e9)
    elseif dropped and dropped > 0 then
      -- Read where it was: the rows in view stay put as the oldest go.
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
  -- Wrapped: rows of their own heights, measured as they come into view.
  -- New rows at the end keep what was measured; rows that moved (a log
  -- that drops its oldest) or a new width measure again.
  if not st.heights then st.heights = row_heights(n, row_h) end
  local g = env.scroll_geometry("rows")
  local hold: {string:number} = nil
  if rev ~= st.rev then
    if dropped == 0 then
      -- The same rows, or rows only added: what was measured stands. It
      -- was cleared on every revision of no new rows, and every row came
      -- back at the estimate and was measured again: the log blinked.
      st.heights:set_len(n)
    elseif dropped then
      -- The oldest gone: the measured heights move up with their rows, and
      -- a reader scrolled away keeps the rows they read where they are.
      local old = st.heights
      local fresh = row_heights(n, old:estimate())
      for i = 0, old:len() - dropped - 1 do
        local h = old:measured(i + dropped)
        if h and h > 0 then fresh:set(i, h) end
      end
      st.heights = fresh
      if not (wants and st.follow) and g then
        -- The row at the top of the window and how far into it, to put
        -- back where it was once the list has measured (below).
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
  local function measure(i: integer, width: number): number
    local m = env.measure_text(message(c, cell_text(ch:get(i, "message"))) as string,
      { size = "$kitSizeLabel", family = "$kitData" } as kui.Props, math.max(40, width - label_w))
    return math.max(row_h, m.height + 2)
  end
  local pin = wants and st.follow and n > 0
  if pin and g and g.w > 0 then
    -- Following: the last screenful measured first, so the estimate the
    -- rows above stand on has already moved for them when the list slices.
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
  } as kui.List, measure, line)
  if hold and hold.row >= 0 then
    -- After the list has measured: the row the reader had at the top goes
    -- back there, whatever the rows above it (and the estimate they stand
    -- on) came to once the oldest left.
    local want = st.heights:offset_of(math.floor(hold.row)) + hold.into
    local at = env.scroll_offset("rows").y - 4
    local by = want - math.max(0, at)
    if math.abs(by) > 0.01 then env.shift_scroll("rows", by, by) end
  end
  if pin then
    -- After the list, and past the end: the list keeps its window on the
    -- row it starts with when measuring moves the estimate (its own
    -- scroll shift), which pinned the window mid-log, above a blank band
    -- where the end should be. Asked last, and clamped by the core to the
    -- end as laid out, the pin wins.
    env.set_scroll("rows", 0, 1e9)
  end
  st.n, st.rev = n, rev
  return framed(t)
end

-- A tone's ink: what a badge's text and border are drawn in.
local function tone_ink(tone: string): string
  if tone == "ok" or tone == "warn" or tone == "danger" then return tone end
  if tone == "accent" then return "on_accent" end
  if tone == "info" then return "focus" end
  return "muted"
end

function M.badge(c: Ctx, p: P): kui.Node
  local tone = str(p.tone) or "neutral"
  local accent = tone == "accent"
  local label = str(p.label) or ""
  local fade = p.fade == true
  local t: kui.Props = {
    label = label,
    tooltip = str(p.tooltip),
    height = 18,
    -- Whole or not at all: a status cut to half a word says nothing.
    min_width = "fit",
    -- `fade`: in as it comes and out as it goes, over the time a switch
    -- takes to cross, so a status that comes with an edit does not pop.
    enter = fade and { opacity = 0 } or nil,
    exit = fade and { opacity = 0 } or nil,
    transition = fade and 140 or nil,
    when = fade and { ctx.still() } or nil,
    pad = { x = 6 },
    gap = 5,
    cross_align = "center",
    radius = c.s.skin.radius > 0 and 9 or 0,
    bg = ref(c, accent and "accent" or "sunken"),
    -- As written: a badge carries data (a status, a version, a count),
    -- which the skin's label casing would garble.
    p.dot == true and column { width = 6, height = 6, radius = 3,
      bg = ref(c, tone == "neutral" and "faint" or (accent and "on_accent" or tone_ink(tone))) } or nil,
    ctx.text(c, label, { size = "$kitSizeLabel", bold = c.s.skin.label_bold, color = ref(c, tone_ink(tone)), max_lines = 1 }),
  }
  -- No outline: the dot and the text carry the tone, and a row of badges
  -- beside fields and buttons is not one more set of edges.
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

local LINE = 16

-- A code block's Copy: asked by a press, done in the next view (the
-- clipboard is the env's), and said for a moment after.
local record Copied
  ask: boolean
  at: number
end

-- How long a code block's button says Copied, in s.
local COPIED_S = 1.5

function M.code(c: Ctx, p: P): kui.Node
  local text = str(p.text) or ""
  local wrap = p.wrap == true
  local lines = p.max_lines as integer
  local body = ctx.data(c, text, { wrap = wrap and "glyph" or "none", line_height = LINE })
  -- The padding on an outer frame and the scroll on an inner box of whole
  -- lines, so a scrolled code block shows no half line at either end.
  local inner: kui.Props = { key = "lines", width = "grow", selectable = true }
  if p.line_numbers == true and not wrap then
    -- The numbers in a gutter of their own, outside the selection and the
    -- sideways scroll, in one text so they keep the code's line height.
    local n = 1
    for _ in text:gmatch("\n") do n = n + 1 end
    local nums: {string} = {}
    for i = 1, n do nums[i] = tostring(i) end
    inner.selectable = nil
    inner[1] = row { width = "grow", gap = 12,
      column { cross_align = "end",
        ctx.data(c, table.concat(nums, "\n"), { line_height = LINE, color = ref(c, "faint") }) },
      column { key = "text", width = "grow", min_width = 0, selectable = true, scroll_x = true, body } }
  else
    inner[1] = body
    inner.scroll_x = not wrap
  end
  if lines then
    inner.max_height = lines * LINE
    inner.scroll_y = true
  end
  local frame: kui.Props = {
    width = "grow",
    role = str(p.label) and "group" or nil,
    label = str(p.label),
    bg = ref(c, "sunken"),
    radius = "$kitRadius",
    pad = 8,
    gap = 6,
  }
  if p.copy == true or str(p.label) then
    local head: kui.Props = { width = "grow", cross_align = "center", gap = 8 }
    if str(p.label) then head[#head + 1] = ctx.muted(c, p.label as string, { max_lines = 1, ellipsis = true }) end
    head[#head + 1] = ctx.grow()
    if p.copy == true then
      local env = c.env
      local st = ctx.state(c, function(): Copied return {} end)
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

-- Kit's own shaders, as Kit registers them: `addons/kit/shaders/*.wgsl`,
-- embedded so every host that loads Kit has them (Kit's tests check the
-- copies match the files).
local SHIMMER_WGSL = [[
{{wgsl:shimmer}}]]

local NOISE_WGSL = [[
{{wgsl:noise}}]]

-- A skeleton's sweep, in ms: one period for every bar, so the bars of a
-- skeleton (and the skeletons placed together) shimmer in step.
local SWEEP_MS = 1500

-- 0xRRGGBBAA as four numbers 0 to 1, onto `out`.
local function rgba(out: {number}, color: integer)
  out[#out + 1] = ((color >> 24) & 255) / 255
  out[#out + 1] = ((color >> 16) & 255) / 255
  out[#out + 1] = ((color >> 8) & 255) / 255
  out[#out + 1] = (color & 255) / 255
end

-- Which half of Kit's colours is on screen: the one held, or the palette's.
local function half(c: Ctx): string
  local held = c.s.appearance
  if held then return held end
  local th = c.env.theme as {string:any}
  return th and th.appearance == "light" and "light" or "dark"
end

-- A fragment's id for `wgsl`, registered in this session — idempotent by
-- source, so asked for every fill — or nil where it does not compile.
local function fragment_id(c: Ctx, wgsl: string): integer
  local ok, id = pcall(c.env.add_fragment, wgsl)
  return ok and id or nil
end

-- A preset's params from the skin: the shimmer's well and its light, its
-- period and whether it moves; the noise's two colours, its drift and
-- whether it moves, and its scale in px.
local function preset_params(c: Ctx, preset: string, moving: boolean): {number}
  local h = half(c)
  local params: {number} = {}
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

-- A box a fragment shader paints (ADR 0015): one of Kit's presets, WGSL
-- the caller hands over (Kit registers both), or a fragment the host
-- registered (`id`). Kit rounds the box by the skin and holds a shader
-- that reads `time` still under reduced motion. With no shader, or one
-- that does not compile, it is the skin's well and keeps its place.
function M.shader(c: Ctx, p: P): kui.Node
  local label = str(p.label)
  local t: kui.fragment = {
    key = "shader",
    role = label and "image" or nil,
    label = label,
    height = p.grow == true and "grow" or (p.height or 80) as number,
    radius = "$kitRadius",
  }
  local pw = p.width
  t.width = pw is number and pw or "grow"
  local moving = not ctx.reduced(c)
  local preset = str(p.preset)
  local id: integer = nil
  local raw = p.id
  if raw is number then
    id = math.tointeger(raw)
  elseif str(p.source) then
    id = fragment_id(c, p.source as string)
  elseif preset then
    id = fragment_id(c, preset == "noise" and NOISE_WGSL or SHIMMER_WGSL)
  end
  if id then
    t.id = id
    -- A preset reads `time` whatever `animate` says; the caller's own
    -- shader does when it says so.
    t.params = p.params or (preset and not p.source and not raw and preset_params(c, preset, moving) or nil)
    local img = p.image
    t.image = img is number and math.tointeger(img) or nil
    t.animate = (p.animate == true or (preset ~= nil and p.animate ~= false)) and moving or nil
    return fragment(t)
  end
  t.bg = ref(c, "sunken")
  return column(t)
end

-- One bar of a skeleton: Kit's shimmer fragment, the well with a band of
-- light crossing it per pixel; or, where the shimmer cannot be registered,
-- the well and a gradient band slid across it by keyframes, from just off
-- its start (so under reduced motion, which holds a loop still, no band
-- shows) to just off its end. `travel` is the bar's laid-out width.
local function bone(c: Ctx, key: string, w: kui.Sizing, h: number, radius: kui.Length, travel: number, shimmer: integer, moving: boolean): kui.Node
  if shimmer then
    return fragment { key = key, id = shimmer, width = w, height = h, radius = radius,
      params = preset_params(c, "shimmer", moving), animate = moving or nil }
  end
  local t: kui.Props = { key = key, width = w, height = h, radius = radius, bg = ref(c, "sunken"), clip = true }
  if travel > 0 then
    local band = math.max(40, math.min(200, travel * 0.4))
    t[1] = column { width = band, height = h, float = { anchor = "parent", dx = -band, clip = true },
      gradient = { to = "right", stops = { "$kitShimmerClear", "$kitShimmer", "$kitShimmerClear" } },
      keyframes = { { dx = 0 }, { dx = travel + band } }, transition = SWEEP_MS, easing = "linear" }
  end
  return column(t)
end

-- A placeholder for what is still loading: lines a text high (the last of
-- several shorter), a block, or a circle, in the skin's well and rounded
-- as its controls are, with a shimmer crossing them that holds still
-- under reduced motion. Read as one image named by `label`.
function M.skeleton(c: Ctx, p: P): kui.Node
  local shape = str(p.shape) or "text"
  local moving = not ctx.reduced(c)
  local shimmer = fragment_id(c, SHIMMER_WGSL)
  local t: kui.Props = { key = "skeleton", role = "image", label = str(p.label) or "Loading" }
  if shape == "circle" then
    local d = (p.width or 32) as number
    t.width, t.height = d, d
    t[1] = bone(c, "b1", d, d, d / 2, d, shimmer, moving)
    return column(t)
  end
  -- Without the fragment, the bars' laid-out width is the band's travel
  -- (kept for `layout_of` by `on_layout`, and a new width redraws).
  t.on_layout = not shimmer and act(c, "skeleton.layout") or nil
  local pw = p.width
  t.width = pw is number and pw or "grow"
  local w = 0.0
  if not shimmer then
    local r = c.env.layout_of("skeleton")
    w = r and r.w or 0
  end
  if shape == "block" then
    local h = (p.height or 80) as number
    t[1] = bone(c, "b1", "grow", h, "$kitRadius", w, shimmer, moving)
    return column(t)
  end
  local n = math.max(1, math.floor((p.lines or 1) as number))
  t.gap = 6
  for i = 1, n do
    local last = n > 1 and i == n
    local width: kui.Sizing = last and { pct = 60 } or "grow"
    t[i] = bone(c, "b" .. i, width, 10, "$kitRadius", last and w * 0.6 or w, shimmer, moving)
  end
  return column(t)
end

local NOTICE_ROLE: {string:string} = { danger = "danger", warn = "warn", ok = "ok", accent = "accent_text", info = "focus", neutral = "muted" }

function M.notice(c: Ctx, p: P): kui.Node
  local tone = str(p.tone) or "neutral"
  local role = NOTICE_ROLE[tone] or "muted"
  -- Live on the message alone: a live region reads as one message, so
  -- the actions inside one would vanish from assistive technology.
  local body: kui.Props = { width = "grow", gap = 2, live = tone == "danger" and "assertive" or "polite" }
  if str(p.title) then
    body[#body + 1] = ctx.text(c, p.title as string, { bold = true, color = ref(c, role == "muted" and "fg" or role), wrap = "word" })
  end
  body[#body + 1] = ctx.text(c, str(p.message) or "", { color = ref(c, role == "muted" and "fg" or role), wrap = "word" })
  -- The message takes the room, never narrower than a short line
  -- (`min_width`); where the actions do not fit beside that, the row wraps
  -- them onto a line of their own under the message, lined up with its
  -- text, instead of squeezing it to a word a line.
  body.width, body.min_width = "grow", 160
  local t: kui.Props = {
    role = "group",
    -- Named by its title at most: the live message inside reads itself.
    label = str(p.title),
    width = "grow",
    pad = { x = 12, y = 8 },
    gap = 10,
    cross_align = "center",
    radius = "$kitRadius",
    bg = ref(c, "sunken"),
    border = { w = "$kitHair", color = ref(c, role == "muted" and "faint" or role) },
    icons.node(c, tone == "ok" and "check" or (tone == "neutral" or tone == "accent" or tone == "info") and "info" or "alert", 14, role),
    row { key = "wrap", width = "grow", min_width = 0, gap = 10, cross_align = "center", wrap_children = true,
      column(body),
      row { key = "actions", min_width = "fit", gap = 8, cross_align = "center", children("") } },
  }
  return row(t)
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
    -- Eased to each new value on a short spring: a checklist ticked off
    -- moves its bar, it does not jump.
    bar = { key = "bar", width = { pct = v * 100 }, height = "grow", bg = ref(c, "accent"),
      transition = 320, easing = "snappy", when = { ctx.still() } }
  else
    -- Under way: a third of the track sliding across all of it, in from
    -- past its start and out past its end. It went from -80 px to 400 px
    -- whatever the track's width, so on a wide track it turned back a
    -- third of the way along. The track's width is its laid-out rect's
    -- (`on_layout` keeps it for `layout_of`, and a new width redraws).
    local r = c.env.layout_of("track")
    local w = r and r.w or 400
    bar = {
      width = { pct = 30 }, height = "grow", bg = ref(c, "accent"),
      -- Under reduced motion the core holds the loop still: a third of
      -- the track lit, at its start.
      keyframes = { { dx = -0.3 * w }, { dx = w } },
      transition = math.max(1400, w * 2.5),
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
    key = "track",
    on_layout = not v and act(c, "progress.layout") or nil,
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
  if kit == "log.select" then
    return emit("on_select", { row = m.row })
  elseif kit == "table.select" or kit == "list.select" then
    -- A row clicked, or pressed by Enter or Space (the click says which).
    local st = ctx.peek(m.inst as integer) as Sel
    if not st or not st.mode then return emit("on_select", { row = m.row }) end
    return on_row_pressed(st, m.row as integer, m.click as P)
  elseif kit == "table.focus" or kit == "list.focus" then
    -- Focus landed on a row: the cursor, and the selection with it when
    -- moves select. By the keyboard, by a screen reader, or at a frame's
    -- end (`program`: a step to a row the window had not built lands
    -- there); a click moves both by its own message.
    local f = m.focus as P
    if f and f.phase == "in" and f.by ~= "pointer" then
      local st = ctx.peek(m.inst as integer) as Sel
      if not st or not st.mode then return emit("on_select", { row = m.row }) end
      return on_cursor_moved(st, m.row as integer, f.mods as P)
    end
  elseif kit == "table.key" or kit == "list.key" then
    local st = ctx.peek(m.inst as integer) as Sel
    local k = m.itemkey as P
    if st and st.mode and k then return on_row_key(st, k) end
  elseif kit == "table.typeahead" or kit == "list.typeahead" then
    local seen = ctx.peek(m.inst as integer) as Seen
    local t = m.typeahead as P
    if seen and t then seen.ask = t.text as string end
  elseif kit == "log.end" then
    local st = ctx.peek(m.inst as integer) as Follow
    if st then st.follow, st.last_y = true, nil end
  elseif kit == "code.copy" then
    local st = ctx.peek(m.inst as integer) as Copied
    if st then st.ask = true end
  elseif kit == "table.sort" then
    return emit("on_sort", { field = m.field })
  elseif kit == "table.range" then
    local r = m.range as P
    if r then return emit("on_range", { first = r.first, last = r.last }) end
  end
  return nil
end

return M
