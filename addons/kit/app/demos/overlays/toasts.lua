-- Toasts: stacked in a toaster in the window's corner, each in its tone.
-- The stack lives here, and every page raises toasts through it: main.lua
-- places `demo.toaster(model)` over everything and calls `demo.push` for
-- its own news (what a button pressed, a worker that stopped).

local kit = require("kit.ui")

local demo = {
  title = "Toasts",
  note = "stacked in a toaster in the corner, each in its tone; they go by themselves, an error stays, Undo answers",
  call = 'kit.toaster({}, {\n  kit.toast({ message = "Saved.", tone = "ok", inline = true, on_dismiss = { kind = "toast_off", id = 1 } }),\n  kit.toast({ message = "Deleted 3 rows.", action = "Undo", inline = true, on_action = { kind = "undo" }, ... }),\n})',
  state = { toasts = {}, toast_n = 0 },
}

-- A toast on the stack: `t` has its message and, as it likes, a tone, an
-- action and a duration.
function demo.push(m, t)
  m.toast_n = m.toast_n + 1
  t.id = m.toast_n
  m.toasts[#m.toasts + 1] = t
end

local function drop(m, id)
  for i, t in ipairs(m.toasts) do
    if t.id == id then table.remove(m.toasts, i); return end
  end
end

function demo.update(m, msg)
  local k = msg.kind
  if k == "toast" then
    demo.push(m, { message = msg.message, tone = msg.tone, action = msg.action, duration = msg.duration })
  elseif k == "toast_all" then
    demo.push(m, { message = "A new version is ready." })
    demo.push(m, { message = "Saved to disk.", tone = "ok" })
    demo.push(m, { message = "The upstream is slow.", tone = "warn" })
    demo.push(m, { message = "Deleted 3 rows.", action = "Undo", duration = 8000 })
  elseif k == "toast_off" then
    drop(m, msg.id)
  elseif k == "undo" then
    drop(m, msg.id)
    demo.push(m, { message = "Restored the 3 rows.", tone = "ok" })
  end
end

-- The toasts, stacked in the corner while there are any.
function demo.toaster(m)
  if #m.toasts == 0 then return nil end
  local t = {}
  for i, s in ipairs(m.toasts) do
    t[i] = kit.toast({ key = "toast" .. s.id, message = s.message, tone = s.tone, action = s.action, duration = s.duration,
      inline = true, on_dismiss = { kind = "toast_off", id = s.id }, on_action = { kind = "undo", id = s.id } })
  end
  return kit.toaster({}, t)
end

function demo.view()
  return row { gap = 8, wrap_children = true, cross_gap = 8,
      kit.button({ label = "Info", on_press = { kind = "toast", message = "A new version is ready." } }),
      kit.button({ label = "Saved", on_press = { kind = "toast", message = "Saved to disk.", tone = "ok" } }),
      kit.button({ label = "Warning", on_press = { kind = "toast", message = "The upstream is slow.", tone = "warn" } }),
      kit.button({ label = "Error", on_press = { kind = "toast", message = "The request failed: 500.", tone = "danger", duration = 0 } }),
      kit.button({ label = "Delete, with Undo", icon = "trash",
        on_press = { kind = "toast", message = "Deleted 3 rows.", action = "Undo", duration = 8000 } }),
      kit.button({ label = "All of them", variant = "ghost", on_press = { kind = "toast_all" } }) },
    text("A toast with no tone is info. An error is kept (duration = 0) until it is dismissed; the others go after five seconds, counted again from when the pointer leaves them.",
      { size = 11, family = "$kitUi", color = "$kitMuted", wrap = "word" })
end

return demo
