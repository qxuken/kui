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

function text(s, opts)
  local t = opts or {}
  t.type = "text"
  t.value = tostring(s)
  return t
end

function button(t)
  t.type = "button"
  return t
end

function input(t)
  t.type = "input"
  return t
end
