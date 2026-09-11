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

-- Single-line input with chrome: input { label = "name", initial = "" }
function input(t)
  t.type = "input"
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

-- virtual_column(env, { key = "log", rows = 10000, row_h = 28, ... }, function(i)
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
-- puts its on_click on a `fill = true` child, as above.
--
-- The geometry is the previous frame's, so the first frame -- before any
-- layout has resolved the container -- slices by the viewport, and a resize
-- is one frame late and covered by `overscan` (two rows each side).
--
-- To reach a row that is not built, scroll to it:
-- `env.set_scroll(key, 0, i * row_h)` puts row i at the top.
function virtual_column(env, opts, row)
  local key = opts.key
  if type(key) ~= "string" or key == "" then
    error("virtual_column needs a string `key` -- its geometry is read back by that name", 2)
  end
  local row_h = opts.row_h
  if type(row_h) ~= "number" or row_h <= 0 then
    error("virtual_column needs a positive `row_h` -- one row's height is the whole stride", 2)
  end
  if type(row) ~= "function" then
    error("virtual_column needs a row builder -- virtual_column(env, opts, function(i) ... end)", 2)
  end
  local n = math.max(0, math.floor(opts.rows or 0))
  local overscan = opts.overscan or 2

  -- nil until a layout has resolved the container, which is the first
  -- frame: a screenful of the viewport is a safe over-build for one.
  local g = env.scroll_geometry(key)
  local vh = g and g.h or env.viewport_h
  -- Layout puts the flow's origin at pad_t - offset, so the band starts
  -- there; only the top padding shifts it.
  local pad_t = opts.pad_t or opts.pad_y or opts.pad or 0
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

  local t = {}
  for k, v in pairs(opts) do t[k] = v end
  t.rows, t.row_h, t.overscan = nil, nil, nil
  t.type = "column"
  t.scroll_y = true
  t.gap = 0

  local at = 1
  -- Keyed, not auto-keyed: an auto key *is* the sibling index, and the rows
  -- already occupy that namespace at their data indices.
  if first > 0 then
    t[at] = column { key = "kui:lead", width = "grow", height = first * row_h }
    at = at + 1
  end
  for i = first, last - 1 do
    t[at] = column { index = i, width = "grow", height = row_h, row(i) }
    at = at + 1
  end
  if last < n then
    t[at] = column { key = "kui:tail", width = "grow", height = (n - last) * row_h }
  end
  return t
end
