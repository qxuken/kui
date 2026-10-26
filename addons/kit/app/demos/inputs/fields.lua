-- Fields: controlled — the value is the app's, a change proposes the next
-- one. An error sits at the field's end and takes no room.

local kit = require("kit.ui")

local demo = {
  title = "Fields",
  note = "controlled: the value is yours, change proposes the next; an error sits at the field's end and takes no room",
  call = 'kit.text_field({ label = "Name", value = model.name, on_change = { kind = "name" } })\nkit.text_field({ label = "Version", value = model.version, mono = true,\n  error = version_error(model.version), on_change = { kind = "version" } })',
  -- What the card keeps in the app's model.
  state = { name = "kit", version = "1.4", query = "", mode = "auto" },
}

-- What is wrong with a version, or nil.
local function version_error(v)
  if v:match("^%d+%.%d+%.%d+$") then return nil end
  return "Three numbers: 1.4.0"
end

function demo.update(m, msg)
  local k = msg.kind
  if k == "name" then m.name = msg.change.text
  elseif k == "version" then m.version = msg.change.text
  elseif k == "query" then m.query = msg.change.text
  elseif k == "mode" then m.mode = msg.change.value
  end
end

function demo.view(m)
  return row { width = "grow", gap = 16, cross_gap = 16, min_column = 220,
      kit.text_field({ label = "Name", value = m.name, on_change = { kind = "name" } }),
      kit.text_field({ label = "Version", value = m.version, mono = true, error = version_error(m.version),
        on_change = { kind = "version" } }),
      kit.search({ value = m.query, placeholder = "Search requests", count = m.query ~= "" and "12 of 340" or nil,
        on_change = { kind = "query" } }),
      kit.select({ label = "Mode", value = m.mode, on_change = { kind = "mode" },
        options = { { value = "auto", label = "Automatic" }, { value = "manual", label = "Manual" },
          { value = "off", label = "Off", disabled = true } } }) },
    text("name = " .. m.name .. "   version = " .. m.version .. "   query = " .. m.query .. "   mode = " .. m.mode,
      { size = 11, family = "$kitData", color = "$kitMuted" })
end

return demo
