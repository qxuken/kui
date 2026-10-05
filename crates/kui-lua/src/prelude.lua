-- kui prelude: injected before every extension script.
-- Views are plain tables — the same IR the Rust builders lower into.

function row(t)
  t = t or {}
  t.type = "row"
  return t
end

function column(t)
  t = t or {}
  t.type = "column"
  return t
end

-- grid { row { text("name"), text("value") }, row { ... } }: a table
-- (docs/adr/0033) -- a column whose rows' children line up in columns,
-- each column as wide as its widest cell, so a label column sits at its
-- longest label with no width picked by hand. A cell's `width` sizes its
-- column (fit or a number is content, "grow" grows the column, a percent
-- takes its cut), a bare text is a cell held to its column, and the rows
-- are rows -- give them width = "grow" for the columns to grow into, and
-- their own gap, pad, bg, hover_bg, on_click. Every other key is the
-- column's. Named `grid` here because `table` is Lua's own.
function grid(t)
  t = t or {}
  t.type = "grid"
  return t
end

-- text("plain", opts) or text({ "plain ", { "bold", bold = true, color = 0x.. } }, opts)
-- The options are copied, not written into: a style table a script hoists
-- (`local mono = { size = 14 }`) and hands to several texts is several
-- nodes, not one table that ends up holding the last string.
function text(s, opts)
  local t = {}
  if opts then
    for k, v in pairs(opts) do t[k] = v end
  end
  t.type = "text"
  if type(s) == "table" then
    t.spans = s
  else
    t.value = tostring(s)
  end
  return t
end

function button(t)
  t.type = "button"
  return t
end

-- The stock toggles (docs/adr/0034): checkbox { label = "Mute", checked =
-- muted, on_click = "mute" }, radio { ... }, switch { ... }. The state is
-- the model's: a press posts on_click and the view flips it. A checkbox
-- also takes mixed = true, the select-all box over a partial selection.
-- `label` is the name and the text both unless `text` says otherwise.
function checkbox(t)
  t.type = "checkbox"
  return t
end

function radio(t)
  t.type = "radio"
  return t
end

function switch(t)
  t.type = "switch"
  return t
end

-- radio_group { label = "Theme", radio { ... }, radio { ... } }: one Tab
-- stop whose arrows move the choice and press the radio they land on.
-- Takes every box row; a column by default.
function radio_group(t)
  t.type = "radio_group"
  return t
end

-- slider { label = "Volume", value_now = v, value_min = 0, value_max = 100,
-- value_step = 5, on_change = "vol" }: the core turns the pointer and the
-- keys into {kind = "change", value, phase, tag} events; store `value` and
-- draw the slider at it.
function slider(t)
  t.type = "slider"
  return t
end

-- Single-line input with chrome: input { label = "name", initial = "" }
function input(t)
  t.type = "input"
  return t
end

-- The stock select: dropdown { label = "language", options = { "English",
-- "Deutsch", { label = "Latin", id = "la", enabled = false } }, current = 2 }.
-- A field showing the choice in force that, clicked, opens the core's own
-- menu of the options under it; the choice arrives as the {kind = "menu"}
-- event a menu row posts, on the field's key, its `item` the option's label
-- or id. `current` counts from 1, as a Lua list does; nil for none. Named
-- `dropdown` here because `select` is Lua's own.
function dropdown(t)
  t.type = "dropdown"
  return t
end

-- Bare editor: edit { key = "note", initial = "", multiline = true,
-- autofocus = true, size = 14, ...spec props }. Read it back in view(env)
-- with env.edit_text(key) after a {kind = "changed"} event (ev.node_key).
function edit(t)
  t.type = "edit"
  return t
end

-- image { id = handle, width = 16, sampling = "nearest", fit = "contain" }:
-- a registered image (the host registers it and passes the handle in as a
-- plain integer). `sampling` is "linear" (default) or "nearest"; `fit` is
-- "fill" (default), "contain" or "cover" -- how the pixels meet the box,
-- which is itself the same in every mode (docs/adr/0025).
function image(t)
  t.type = "image"
  return t
end

-- fragment { id = handle, params = {0.1, 0.2, ...}, animate = true, ... }: a
-- box the registered WGSL `id` paints (the host registers it with
-- Core::add_fragment and passes the handle in as a plain integer). An
-- ordinary node otherwise -- it lays out, rounds, clips, fades, takes input
-- and may hold children, which paint over it -- but it has no intrinsic
-- size, so give it a width and height or `fill`. `params` is up to sixteen
-- numbers the shader reads as four vec4<f32>; `animate` asks for a frame
-- every frame, which a fragment reading `time` needs.
function fragment(t)
  t.type = "fragment"
  return t
end

-- line { from = {x, y}, to = {x, y}, width = 2, color = 0x7f9cf5ff } or
-- line { points = {{x, y}, ...}, curve = true, ... }: a round-capped stroke
-- in the parent's box space, never in layout (it floats, sized to its own
-- bounding box). `width` is the stroke width, `color` the stroke colour;
-- `key`, `transition`, `opacity`, `on_layout` apply, and `on_click`,
-- `on_drag`, `on_hover`, `hoverable` hit by the stroke (docs/adr/0026).
function line(t)
  t.type = "line"
  return t
end

-- polygon { points = {{x, y}, ...}, bg = 0xd8863bff }: a filled polygon
-- through up to eight points in the parent's box space, the fill in `bg`,
-- placed like a line (never in layout; floats, sized to its own bounding
-- box). The outline may be concave. `key`, `transition`, `opacity`,
-- `on_layout` apply, and `on_click`, `on_drag`, `on_hover`, `hoverable`
-- hit by the outline (docs/adr/0025 decision 6, docs/adr/0026).
function polygon(t)
  t.type = "polygon"
  return t
end

-- path { d = "M60 60 L100 60 A40 40 0 0 1 60 100 Z", bg = 0xd8863bff,
-- fill_rule = "nonzero", width = 2, color = 0xffffffff }: any outline as SVG
-- path data (or `ops`, the flat op form), filled with `bg` by `fill_rule`
-- and stroked `width` wide in `color` when `width` is given, placed like a
-- line. Hit by its outline under the rule (docs/adr/0040).
function path(t)
  t.type = "path"
  return t
end

-- cells { rows=, cols=, lines={"row text", ...}, runs={{row, col, len, fg,
-- bg, flags}, ...}, cursor_at={row, col}, cursor_shape="block", cursor_color=,
-- size=, family= }: a terminal's screen as one node (backlog C20). Rows and
-- columns are 0-based in runs and the cursor; a run's fg or bg of 0 keeps
-- the default.
function cells(t)
  t.type = "cells"
  return t
end

-- audio { src = id, loop = true, volume = 0.5, paused = false, tag = {...},
-- key = "music" }: a playback retained by key while the view declares it
-- (present = playing, gone = stopped; volume/paused apply live). Draws
-- nothing. The host registers the sound and hands the id to the script.
function audio(t)
  t.type = "audio"
  return t
end

-- titlebar { title = "app" } draws the standard title; with children it
-- hosts custom content between the drag inset and the window buttons.
function titlebar(t)
  t = t or {}
  t.type = "titlebar"
  return t
end

function window_buttons()
  return { type = "window_buttons" }
end

-- menu_bar { menu = { { label = "File", items = { ... } } } }: the
-- application menu (docs/adr/0018-a-menu-bar-the-app-declares.md). `menu`
-- is what it is; where this sits is where its titles go when they have to
-- be drawn in the window. Nothing is drawn where the platform owns the bar
-- (macOS), so a view writes it once and is portable.
function menu_bar(t)
  t = t or {}
  t.type = "menu_bar"
  return t
end

-- tooltip("hint") or tooltip { children }: a float hanging below the parent.
-- For the common hover-gated case use the `tooltip = "hint"` prop on a
-- row/column instead; the node form always draws.
function tooltip(s, t)
  if type(s) == "table" then
    t = s
  else
    t = t or {}
    t.value = tostring(s)
  end
  t.type = "tooltip"
  return t
end

function latency_graph()
  return { type = "latency_graph" }
end

-- latency_hud { at = { "end", "end" } }
function latency_hud(t)
  t = t or {}
  t.type = "latency_hud"
  return t
end

-- fill { name = "todos/panel", params = { ... } }: the position a plugin
-- this script loaded draws in -- a place among its siblings, not a box
-- around anything, and keyed inside this script's own fill (ADR 0014).
-- It draws nothing itself. `name` is the full "namespace/slot" -- the
-- namespace given to env.add_extension and the slot in the plugin's own
-- vocabulary. Draws an empty position when nothing answers to the name.
--
-- Named `fill` and not `slot` because `slot` is view's second argument,
-- and a script that took it would shadow the constructor in the one
-- function that needs it. A fill is what the core calls this anyway: the
-- thing an extension draws.
function fill(t)
  t.type = "fill"
  return t
end

-- devtools_tab { name = "syntax", label = "Tree-sitter", slot = "ts/panel" }
-- devtools_tab { name = "syntax", label = "Tree-sitter", view = function()
--   return column { text("identifier 12:4") }
-- end }
--
-- A tab in the core's devtools panel (ADR 0032), beside facts, events and
-- tree. The first form names a slot a plugin fills: the panel declares it
-- in the tab's body while the tab is on show. The second is the script's
-- own content: `view` is called only while the tab is on show -- so a tab
-- nobody looks at costs its declaration and nothing else -- and the tree
-- it returns is drawn over the tab's body, the script's to hear as any of
-- its nodes. `view` takes no arguments; close over the `env` the script's
-- view received. Not a node: declare it anywhere in the tree, every frame.
function devtools_tab(t)
  t.type = "devtools_tab"
  return t
end

-- uniform_list(env, { key = "log", rows = 10000, row_h = 28, ... }, function(i)
--   return column { fill = true, on_click = { kind = "pick", row = i }, text("line " .. i) }
-- end)
--
-- A scrolling column of `rows` uniform rows that declares only the visible
-- ones, plus the two spacers that hold the height of the rest -- so the
-- frame costs a screenful however long the list is. `env.scroll_geometry`
-- is what it slices by, so the container needs the `key` this reads it back
-- under. Every other key of `opts` is the container's own; `scroll_y` and
-- `gap` are the widget's (put a row's spacing inside `row_h`).
--
-- `row(i)` returns row i's *contents*: the widget owns the row's own node,
-- `row_h` tall and keyed by the row's data `index`, so a row keeps its
-- hover, focus, edit buffer and tweens as the built range slides over it,
-- and a virtualised list and a full one agree on identity. A clickable row
-- puts its on_click on a `fill = true` child, as above -- or on the row's
-- own node: `row_props = function(i) return { on_click = ..., bg = ... } end`
-- in `opts` gives each row's node its props (backlog DX22), its `index`
-- and `height` staying the widget's.
--
-- The geometry is the previous frame's, so the first frame -- before any
-- layout has resolved the container -- slices by the viewport, and a resize
-- is one frame late and covered by `overscan` (two rows each side).
--
-- To reach a row that is not built, `reveal_row(env, key, i, row_h)`
-- before the list scrolls it to the middle when it does not show.
function uniform_list(env, opts, row)
  local key = opts.key
  if type(key) ~= "string" or key == "" then
    error("uniform_list needs a string `key` -- its geometry is read back by that name", 2)
  end
  local row_h = opts.row_h
  if type(row_h) ~= "number" or row_h <= 0 then
    error("uniform_list needs a positive `row_h` -- one row's height is the whole stride", 2)
  end
  if type(row) ~= "function" then
    error("uniform_list needs a row builder -- uniform_list(env, opts, function(i) ... end)", 2)
  end
  local n = math.max(0, math.floor(opts.rows or 0))
  local overscan = opts.overscan or 2

  -- nil until a layout has resolved the container, which is the first
  -- frame: a screenful of the viewport is a safe over-build for one.
  local g = env.scroll_geometry(key)
  local vh = g and g.h or env.viewport_h
  -- Layout puts the flow's origin at the top padding - offset, so the
  -- band starts there; only the top padding shifts it. `pad` is a number
  -- or a table of edges; a `"$token"` is not resolved here and counts as
  -- none, which `overscan` covers.
  local pad = opts.pad
  local pad_t = 0
  if type(pad) == "table" then pad = pad.t or pad.y or pad.all end
  if type(pad) == "number" then pad_t = pad end
  local top = (g and g.offset.y or 0) - pad_t
  -- Both ends are clamped to the list, `first` included: the geometry is the
  -- previous frame's, so a list that shrank under its own scroll offset
  -- slices past its new end. Left unclamped that builds a lead spacer taller
  -- than the whole list and no rows at all, and the oversized spacer keeps
  -- the offset legal, so it unwinds one viewport a frame instead of landing
  -- in one.
  local first = math.min(n, math.max(0, math.floor(top / row_h) - overscan))
  local last = math.min(n, math.ceil((top + math.max(vh, 0)) / row_h) + overscan)
  if last < first then last = first end

  local row_props = opts.row_props
  local t = {}
  for k, v in pairs(opts) do t[k] = v end
  t.rows, t.row_h, t.overscan, t.row_props = nil, nil, nil, nil
  t.type = "column"
  t.scroll_y = true
  t.gap = 0
  -- The whole list's size, built or not: what Select All inside a
  -- selectable list spans.
  t.row_count = n

  local at = 1
  -- Keyed, not auto-keyed: an auto key *is* the sibling index, and the rows
  -- already occupy that namespace at their data indices.
  if first > 0 then
    t[at] = column { key = "kui:lead", width = "grow", height = first * row_h }
    at = at + 1
  end
  for i = first, last - 1 do
    local r = { width = "grow" }
    if row_props then
      for k, v in pairs(row_props(i)) do
        if type(k) == "string" then r[k] = v end
      end
    end
    r.index, r.height, r[1] = i, row_h, row(i)
    t[at] = column(r)
    at = at + 1
  end
  if last < n then
    t[at] = column { key = "kui:tail", width = "grow", height = (n - last) * row_h }
  end
  return t
end

-- reveal_row(env, "log", i, 28)
--
-- Scrolls the `uniform_list` keyed "log" so row i shows, when it does
-- not: to the middle, so a jump lands with rows on both sides (backlog
-- DX22, Rust's `widgets::reveal_row`). Call it before the list, so the
-- frame that scrolls builds the rows it scrolled to; `env.reveal` finds
-- nothing for a row the list has not built. True when it scrolled; the
-- first frame, before the list has laid out, scrolls nothing. Assumes the
-- rows start at the list's content top, as they do without top padding.
function reveal_row(env, key, i, row_h)
  local g = env.scroll_geometry(key)
  if not g then return false end
  local y = i * row_h
  -- A row past the content, or no stride, scrolls nothing (backlog RG75).
  if not (row_h > 0) or y + row_h > g.content_h + 0.5 then return false end
  if g.offset.y <= y and y + row_h <= g.offset.y + g.h then return false end
  env.set_scroll(key, g.offset.x, math.max(0, y + row_h / 2 - g.h / 2))
  return true
end

-- How many whole rows of `row_h` the list keyed `key` shows as of the last
-- layout -- a page's stride; 0 before it has laid out.
function rows_in_view(env, key, row_h)
  local g = env.scroll_geometry(key)
  if not g or not (row_h > 0) then return 0 end
  return math.max(0, math.floor(g.h / row_h))
end

-- splitter(env, { key = "divider", dir = "row", thickness = 4, on_drag = { kind = "split" } })
--
-- A divider between two panes that the pointer drags (backlog DX22, Rust's
-- `widgets::splitter`): `thickness` px across (4 unless said) and growing
-- along the rest of its parent, in the theme's border colour and its accent
-- while hovered or held, with the resize arrows, and `keep_focus`, so a
-- press on it leaves the keyboard with the editor beside it. `dir` is the
-- parent's: "row" (the default) for panes side by side, "column" for panes
-- stacked. The split is the handler's: a `drag` event carries `x`, `y` and
-- `parent`, and `(x - parent.x) / parent.w` is a row's new fraction.
function splitter(env, t)
  local th = env.theme
  local across = (t.dir or "row") == "row"
  local n = t.thickness or 4
  return column {
    key = t.key,
    width = across and n or "grow",
    height = across and "grow" or n,
    cursor = across and "ewResize" or "nsResize",
    bg = th.border,
    hover_bg = th.accent,
    pressed_bg = th.accent,
    on_drag = t.on_drag,
    keep_focus = true,
  }
end

-- local heights = row_heights(#lines, 20)
-- list(env, { key = "chat", heights = heights },
--   function(i, width) return env.measure_text(lines[i + 1], { size = 14 }, width).height + 8 end,
--   function(i) return text { pad = 4, lines[i + 1] } end)
--
-- A scrolling column of rows of *different* heights that declares only the
-- visible ones -- `uniform_list` where no single stride describes the list.
-- `heights` is a `row_heights(rows, estimate)` the script makes once and
-- keeps: the core's own prefix sums over the rows measured so far, with the
-- mean of those standing in for every other row. `set_len` it when rows are
-- appended, `clear` it when they change under the same indices.
--
-- `measure(i, width)` returns row i's height at that content width, and is
-- called only for rows the frame is about to build that `heights` has no
-- number for; `env.measure_text(s, style, width)` is layout's own answer for
-- a text row. What it returns is the height the row gets: the widget fixes
-- each row's node to it. `row(i)` returns row i's contents, as
-- `uniform_list`'s does, keyed by the row's data `index` (0-based).
--
-- Measuring moves the estimate, and with it every row above the window, so
-- the widget puts the row the window starts in back where it was
-- (`env.shift_scroll`) -- and, mid-glide on a container with a
-- `transition`, the row the glide is going to. The arithmetic is the core's
-- (`widgets::list` in Rust); this is the loop around the script's callback.
--
-- `env.set_scroll(key, 0, heights:offset_of(i))` puts row i at the top.
function list(env, opts, measure, row)
  local key = opts.key
  if type(key) ~= "string" or key == "" then
    error("list needs a string `key` -- its geometry is read back by that name", 2)
  end
  local heights = opts.heights
  if type(heights) ~= "userdata" then
    error("list needs `heights` -- a row_heights(rows, estimate) the script keeps between views", 2)
  end
  if type(measure) ~= "function" then
    error("list needs a measure function -- list(env, opts, function(i, width) return h end, row)", 2)
  end
  if type(row) ~= "function" then
    error("list needs a row builder -- list(env, opts, measure, function(i) ... end)", 2)
  end

  -- The padding the rows sit inside: its top shifts where the window
  -- starts, its sides narrow what a row is measured at. A `"$token"` is
  -- not resolved here and counts as none.
  local pad = opts.pad
  local pad_t, pad_x = 0, 0
  if type(pad) == "number" then
    pad_t, pad_x = pad, 2 * pad
  elseif type(pad) == "table" then
    local function n(v) return type(v) == "number" and v or nil end
    local all = n(pad.all)
    pad_t = n(pad.t) or n(pad.y) or all or 0
    pad_x = (n(pad.l) or n(pad.x) or all or 0) + (n(pad.r) or n(pad.x) or all or 0)
  end

  local width, pending = heights:slice_begin {
    geometry = env.scroll_geometry(key),
    scroll_y = env.scroll_offset(key).y,
    viewport_w = env.viewport_w,
    viewport_h = env.viewport_h,
    pad_t = pad_t,
    pad_x = pad_x,
    overscan = opts.overscan or 2,
  }
  while #pending > 0 do
    local measured = {}
    for n, i in ipairs(pending) do
      measured[n] = measure(i, width)
    end
    pending = heights:slice_measured(measured)
  end
  local plan = heights:slice_finish()
  if plan.shift then
    env.shift_scroll(key, plan.shift.drawn, plan.shift.target)
  end

  local t = {}
  for k, v in pairs(opts) do t[k] = v end
  t.heights, t.overscan = nil, nil
  t.type = "column"
  t.scroll_y = true
  t.gap = 0
  t.row_count = heights:len()

  local at = 1
  if plan.lead > 0 then
    t[at] = column { key = "kui:lead", width = "grow", height = plan.lead }
    at = at + 1
  end
  for i = plan.first, plan.last - 1 do
    t[at] = column { index = i, width = "grow", height = plan.heights[i - plan.first + 1], row(i) }
    at = at + 1
  end
  if plan.tail > 0 then
    t[at] = column { key = "kui:tail", width = "grow", height = plan.tail }
  end
  return t
end
