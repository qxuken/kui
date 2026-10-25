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
    toast = nil,            -- the newest toast, the last of `toasts`
    toasts = {},            -- every toast showing, oldest first: they stack
    next_toast = 1,
    saved_at = nil,
    show_toml = false,
  }
end

-- A toast joins the stack in the corner; it goes by itself (Kit's
-- `duration`) or by its close button. `undo` is the draft its Undo brings
-- back, for a revert or a reset.
local function toast(m, message, tone, undo)
  local t = { id = m.next_toast, message = message, tone = tone, undo = undo }
  m.next_toast = m.next_toast + 1
  m.toasts[#m.toasts + 1] = t
  m.toast = t
end

local function toast_off(m, id)
  for i, t in ipairs(m.toasts) do
    if t.id == id then table.remove(m.toasts, i) break end
  end
  m.toast = m.toasts[#m.toasts]
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
      toast(m, "Fix the highlighted settings first.", "danger")
      return
    end
    local ok, err = pcall(settings.save, boot.config, m.draft)
    if ok then
      local n = #settings.diff(m.saved, m.draft)
      m.saved = settings.copy(m.draft)
      m.saved_at = kui.now()
      m.problem = nil
      toast(m, string.format("Saved %d change%s.", n, n == 1 and "" or "s"), "ok")
    else
      toast(m, "Could not save: " .. tostring(err):match("^[^\n]*"), "danger")
    end
  elseif k == "revert" then
    local before = settings.copy(m.draft)
    m.draft = settings.copy(m.saved)
    toast(m, "Changes reverted.", "neutral", before)
  elseif k == "reset" then m.confirm = "reset"
  elseif k == "cancel" then m.confirm = nil
  elseif k == "confirm_reset" then
    m.confirm = nil
    local before = settings.copy(m.draft)
    m.draft = settings.copy(settings.DEFAULTS)
    toast(m, "Every setting is back to its default. Save to keep them.", "accent", before)
  elseif k == "undo" then
    -- A toast's Undo: the draft from before its revert or reset.
    for _, t in ipairs(m.toasts) do
      if t.id == msg.id and t.undo then m.draft = t.undo end
    end
    toast_off(m, msg.id)
  elseif k == "menu" then
    -- The menu bar (and its accelerators, ⌘S / Ctrl+S among them).
    local item = msg.menu and msg.menu.item
    if item == "save" or item == "revert" then return update({ kind = item })
    elseif item == "find" then m.focus_search = m.focus_search + 1 end
  elseif k == "show_toml" then m.show_toml = not m.show_toml
  elseif k == "skin" then m.skin = msg.change.value
  elseif k == "side" then m.side = msg.resize.fraction
  elseif k == "toast_off" then toast_off(m, msg.id)
  end
end

function view(env)
  return view_of(model, env)
end
