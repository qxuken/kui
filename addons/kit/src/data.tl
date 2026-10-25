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
  -- Tab heard by the list's key sink, to hand the focus on with when it
  -- is next drawn: 1 forward, -1 back.
  tab: integer
end

-- A list or a table is a key sink (its arrows move the selection), and a
-- sink owns Tab, so Tab went in and never came out. Its handler notes the
-- Tab and the next draw hands the focus on, as a sink is meant to.
local function hand_on(c: Ctx, seen: Seen)
  local dir = seen.tab
  if not dir then return end
  seen.tab = nil
  if dir > 0 then c.env.focus_next() else c.env.focus_prev() end
end

-- The focus, drawn: the sink's own edge in the focus colour while it holds
-- the keyboard (a scroller's ring was clipped by the pane around it).
local function focus_edge(c: Ctx, t: kui.Props)
  local ok, here = pcall(c.env.is_focused, "rows")
  if ok and here then t.border = { w = "$kitRuleW", color = ref(c, "focus") } end
end

-- A tone a channel cell names, as the role its text is drawn in; nil for
-- none or a word Kit has no tone for.
local TONE_ROLE: {string:string} = { ok = "ok", warn = "warn", danger = "danger", accent = "accent", neutral = "muted" }

local function tone_of(v: any): string
  return v is string and TONE_ROLE[v] or nil
end

-- A small pill for a cell or a row: its text in the tone, a dot before it
-- when `dot`. What a badge draws, sized to sit in a row.
-- Its ground is the pressed wash, which reads on a panel and a sidebar
-- alike; the accent tone fills it, as a badge's does. On a selected row
-- it takes the selection's ink.
local function pill(c: Ctx, label: string, tone: string, dot: boolean, sel: boolean): kui.Node
  local role = tone_of(tone)
  local accent = role == "accent" and not sel
  local ink = ref(c, sel and "sel_fg" or (accent and "on_accent" or (role or "muted")))
  local spot = sel and "sel_fg" or (accent and "on_accent" or (role == "muted" and "faint" or (role or "faint")))
  return row {
    height = 18, min_width = "fit", pad = { x = 6 }, gap = 5, cross_align = "center",
    radius = c.s.skin.radius > 0 and 9 or 0,
    bg = accent and ref(c, "accent") or (not sel and ref(c, "press") or nil),
    dot and column { width = 6, height = 6, radius = 3, bg = ref(c, spot) } or nil,
    ctx.data(c, label, { size = "$kitSizeLabel", bold = c.s.skin.label_bold, color = ink, max_lines = 1 }),
  }
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
  hand_on(c, seen)
  if selected ~= seen.selected then
    if selected then reveal(env, "rows", selected, row_h) end
    seen.selected = selected
  end

  -- The header on a plane of its own (the sunken colour) rather than over
  -- a rule; `border` frames the whole table and rules the header instead.
  local framed = p.border == true
  local head: kui.Props = { width = "grow", height = row_h + 4, pad = { x = 10 }, gap = 10, cross_align = "center",
    bg = ref(c, framed and "panel" or "sunken") }
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
  local rows_t: kui.Props = {
    height = "grow",
    focusable = true,
    on_key = act(c, "table.key", { selected = selected, n = n, page = math.max(1, math.floor((env.viewport_h or 400) / row_h) - 1) }),
    on_range = act(c, "table.range"),
    label = "rows",
  }
  focus_edge(c, rows_t)
  local view = body(c, "rows", n, row_h, rows_t, function(i: integer): kui.Node
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
    -- Framed: a hairline in the soft rule colour around the table, inside
    -- which the header sits (the border's width is padding, so the rule
    -- under the header stops at the frame and does not cross it).
    border = framed and { w = "$kitHair", color = ref(c, "rule") } or nil,
    pad = framed and "$kitHair" or nil,
    row(head),
    framed and ctx.rule(c, "rule", "$kitHair") or nil,
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
  hand_on(c, seen)
  if selected ~= seen.selected then
    if selected then reveal(c.env, "rows", selected, row_h) end
    seen.selected = selected
  end
  local rows_t: kui.Props = {
    height = p.grow == true and "grow" or nil,
    role = "list",
    label = str(p.label),
    focusable = true,
    on_key = act(c, "list.key", { selected = selected, n = n, page = 8 }),
  }
  focus_edge(c, rows_t)
  return body(c, "rows", n, row_h, rows_t, function(i: integer): kui.Node
    local sel = selected == i
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
    return row {
      index = i,
      role = "listItem",
      label = badge ~= "" and (badge .. " " .. title) or title,
      selected = sel,
      height = row_h,
      width = "grow",
      pad = { x = 10 },
      gap = 8,
      cross_align = "center",
      bg = sel and ref(c, "sel") or nil,
      hover_bg = sel and ref(c, "sel") or ref(c, "hover"),
      on_click = act(c, "list.select", { row = i }),
      badge ~= "" and row { width = p.badge_width as number, min_width = "fit",
        pill(c, badge, tone, false, sel) } or nil,
      column(t),
    }
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
  return row {
    key = "jump",
    role = "button",
    label = "Jump to the latest line",
    float = { anchor = "parent", at = { "end", "end" }, self = { "end", "end" }, dx = -12, dy = -8 },
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
    pad = { x = 6 },
    gap = 5,
    cross_align = "center",
    radius = c.s.skin.radius > 0 and 9 or 0,
    bg = ref(c, accent and "accent" or "sunken"),
    -- As written: a badge carries data (a status, a version, a count),
    -- which the skin's label casing would garble.
    p.dot == true and column { width = 6, height = 6, radius = 3,
      bg = ref(c, tone == "neutral" and "faint" or (accent and "on_accent" or tone)) } or nil,
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

function M.code(c: Ctx, p: P): kui.Node
  local wrap = p.wrap == true
  local lines = p.max_lines as integer
  local body = ctx.data(c, str(p.text) or "", { wrap = wrap and "glyph" or "none", line_height = LINE })
  -- The padding on an outer frame and the scroll on an inner box of whole
  -- lines, so a scrolled code block shows no half line at either end.
  local inner: kui.Props = { key = "lines", width = "grow", selectable = true, body }
  if lines then
    inner.max_height = lines * LINE
    inner.scroll_y = true
  end
  inner.scroll_x = not wrap
  return column {
    width = "grow",
    bg = ref(c, "sunken"),
    radius = "$kitRadius",
    pad = 8,
    column(inner),
  }
end

local NOTICE_ROLE: {string:string} = { danger = "danger", warn = "warn", ok = "ok", accent = "accent", neutral = "muted" }

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
    border = { w = "$kitHair", color = ref(c, role == "muted" and "rule" or role) },
    icons.node(c, tone == "ok" and "check" or (tone == "neutral" or tone == "accent") and "info" or "alert", 14, role),
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
      transition = 320, easing = "snappy" }
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
  elseif kit == "log.end" then
    local st = ctx.peek(m.inst as integer) as Follow
    if st then st.follow, st.last_y = true, nil end
  elseif kit == "table.sort" then
    return emit("on_sort", { field = m.field })
  elseif kit == "table.range" then
    local r = m.range as P
    if r then return emit("on_range", { first = r.first, last = r.last }) end
  elseif kit == "table.key" or kit == "list.key" then
    local k = m.key as P
    if not k or k.phase ~= "down" then return nil end
    if k.code == "tab" then
      local seen = ctx.peek(m.inst as integer) as Seen
      if seen then seen.tab = k.shift == true and -1 or 1 end
      return nil
    end
    local n = (m.n or 0) as integer
    if n == 0 then return nil end
    local s = step(k.code as string, m.selected as integer, n, (m.page or 8) as integer)
    if s and s ~= m.selected then return emit("on_select", { row = s }) end
  end
  return nil
end

return M
