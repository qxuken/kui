-- What every component is handed besides its own params: the frame's env,
-- the skin and settings in force, and the instance it is drawing.

local skin = require("skin")

local record Ctx
  env: kui.Env
  skin: skin.Skin
  settings: skin.Settings
  -- The instance: the slot name the host declared it under (`button#3`).
  id: string
end

local M = { Ctx = Ctx }

-- A component's event as the host declared it (ADR 0047's shape, done by
-- hand until core delivers it): a copy of the host's message with the
-- event's fields merged in. `nil` when the host declared none, so the
-- event is the component's alone.
function M.emit(declared: any, fields: {string:any}): {string:any}
  if declared == nil then return nil end
  local out: {string:any} = {}
  if declared is {string:any} then
    for k, v in pairs(declared) do out[k] = v end
  else
    out.tag = declared
  end
  for k, v in pairs(fields) do out[k] = v end
  return out
end

return M
