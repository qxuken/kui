-- A trace waterfall: 18,000 spans the app's worker built
-- (jobs/generate.lua; main.lua keeps them as `model.spans`). Wheel or
-- pinch to zoom about the pointer, swipe sideways or drag to pan, drag
-- along the time axis to select a range, click a span or walk them with
-- the arrows.

-- Source also: jobs/generate.lua

local kit = require("kit.ui")

local demo = {
  title = "Waterfall",
  note = "18,000 spans: wheel or pinch to zoom about the pointer, swipe sideways or drag to pan, drag along the time axis to select a range, click a span or Tab to the spans and use the arrows",
  call = 'kit.waterfall({ bars = model.spans, start = model.start, length = model.length,\n  on_zoom = { kind = "zoom" }, on_select = { kind = "span" } })',
  state = { trace_start = nil, trace_length = nil, span = nil },
}

function demo.update(m, msg)
  if msg.kind == "zoom_trace" then m.trace_start, m.trace_length = msg.zoom.start, msg.zoom.length
  elseif msg.kind == "span" then m.span = msg.select.row
  end
end

function demo.view(m)
  local detail
  if m.spans and m.span then
    detail = kit.inspector({ entries = {
      { key = "Span", value = tostring(m.spans:get(m.span, "name")) },
      { key = "Start", value = string.format("%.2f ms", m.spans:get(m.span, "start")), mono = true },
      { key = "Length", value = string.format("%.2f ms", m.spans:get(m.span, "len")), mono = true },
      { key = "Frame", value = tostring(m.spans:get(m.span, "target")) },
    } })
  end
  return column { width = "grow", height = 300,
      m.spans and kit.waterfall({ bars = m.spans, start = m.trace_start, length = m.trace_length or 2000, selected = m.span,
        grow = true, on_zoom = { kind = "zoom_trace" }, on_select = { kind = "span" } })
        or kit.progress({ label = "The worker is building the trace…" }) },
    detail or text("Click a span to see it here.", { size = 11, family = "$kitUi", color = "$kitMuted" })
end

return demo
