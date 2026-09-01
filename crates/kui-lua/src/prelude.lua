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
function text(s, opts)
  local t = opts or {}
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

function image(t)
  t.type = "image"
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
