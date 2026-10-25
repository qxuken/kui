-- Preferences: a settings window on Kit and the kui runner — sections in a
-- sidebar, switches, sliders, selects, radios, segments and fields with
-- validation, a search across every setting, and save / revert over a
-- TOML file. The settings are those of Tidewater, a pretend sync client.
--
--   kui run addons/kit/samples/preferences
--   kui run addons/kit/samples/preferences -- --config /tmp/tidewater.toml

local settings = require("settings")
local view_of = require("view")

if not boot then
  local opts = kui.parse_args({
    name = "preferences",
    about = "Tidewater's preferences window: a settings sample for Kit",
    options = {
      config = { default = (kui.env.HOME or ".") .. "/.config/kui-samples/tidewater.toml", value = "path",
        help = "the settings file it reads and saves" },
    },
  })
  boot = { config = opts.config }
end

function init()
  local saved, problem = settings.load(boot.config)
  return {
    skin = "ledger",
    section = "general",
    query = "",
    focus_search = 0,
    side = 0.22,
    saved = saved,
    draft = settings.copy(saved),
    problem = problem,
    confirm = nil,          -- "reset" while the reset waits on the dialog
    toast = nil,
    saved_at = nil,
    show_toml = false,
  }
end

-- The rows a search leaves are the view's to know; it hands them back.
function update(msg)
  local m = model
  local k = msg.kind
  if k == "set" then
    -- every control proposes through this: the key, and the event's value
    local v
    if msg.change then v = msg.change.value ~= nil and msg.change.value or msg.change.text
    elseif msg.toggle then
      if msg.toggle.on ~= nil then v = msg.toggle.on else v = msg.toggle.checked end
    end
    if msg.integer and type(v) == "number" then v = math.floor(v + 0.5) end
    if v ~= nil then m.draft[msg.key] = v end
  elseif k == "all_folders" then
    for _, f in ipairs(settings.FOLDERS) do m.draft[f[1]] = msg.toggle.checked end
  elseif k == "section" then m.section, m.query = msg.section, ""
  elseif k == "query" then m.query = msg.change.text
  elseif k == "save" then
    local errors = settings.check(m.draft)
    if next(errors) then
      m.toast = { message = "Fix the highlighted settings first.", tone = "danger" }
      return
    end
    local ok, err = pcall(settings.save, boot.config, m.draft)
    if ok then
      local n = #settings.diff(m.saved, m.draft)
      m.saved = settings.copy(m.draft)
      m.saved_at = kui.now()
      m.problem = nil
      m.toast = { message = string.format("Saved %d change%s.", n, n == 1 and "" or "s"), tone = "ok" }
    else
      m.toast = { message = "Could not save: " .. tostring(err):match("^[^\n]*"), tone = "danger" }
    end
  elseif k == "revert" then
    m.draft = settings.copy(m.saved)
    m.toast = { message = "Changes reverted.", tone = "neutral" }
  elseif k == "reset" then m.confirm = "reset"
  elseif k == "cancel" then m.confirm = nil
  elseif k == "confirm_reset" then
    m.confirm = nil
    m.draft = settings.copy(settings.DEFAULTS)
    m.toast = { message = "Every setting is back to its default. Save to keep them.", tone = "accent" }
  elseif k == "show_toml" then m.show_toml = not m.show_toml
  elseif k == "skin" then m.skin = msg.change.value
  elseif k == "side" then m.side = msg.resize.fraction
  elseif k == "toast_off" then m.toast = nil
  end
end

function view(env)
  return view_of(model, env)
end
