-- App-specific assertions; the runner owns startup, time and cleanup.
return function()

local path=kui.args[2]

local settings = require("settings")


local function set(key, value, how)
  local msg = { kind = "set", key = key }
  if how == "on" then msg.toggle = { on = value }
  elseif how == "checked" then msg.toggle = { checked = value }
  elseif type(value) == "string" and how ~= "value" then msg.change = { text = value }
  else msg.change = { value = value } end
  if how == "integer" then msg.integer = true end
  update(msg)
end

  assert(not kui.fs.exists(path) and #settings.diff(model.saved, model.draft) == 0, "no file yet: the defaults, nothing unsaved")
  assert(model.draft.device_name == "Maren’s MacBook Pro" and model.problem == nil, "the defaults read")

  -- every kind of control proposes through one message
  set("device_name", "Studio iMac")
  set("open_at_login", false, "on")
  set("downloads", true, "checked")
  set("appearance", "dark", "value")
  set("versions", 44.6, "integer")
  set("upload_limit", 2500, "integer")
  local d = model.draft
  assert(d.device_name == "Studio iMac" and d.open_at_login == false and d.downloads and d.appearance == "dark"
    and d.versions == 45 and d.upload_limit == 2500, "a field, a switch, a box, a radio and two sliders")
  assert(#settings.diff(model.saved, model.draft) == 6, "6 unsaved changes")

  -- validation blocks the save
  set("device_name", "   ")
  local errors = settings.check(model.draft)
  assert(errors.device_name ~= nil, "a blank name is an error")
  update({ kind = "save" })
  assert(not kui.fs.exists(path) and model.toast.tone == "danger", "and the save refuses, saying so")
  set("device_name", "Studio/iMac")
  assert(settings.check(model.draft).device_name:find("/", 1, true), "a slash is an error")
  set("device_name", string.rep("x", 41))
  assert(settings.check(model.draft).device_name:find("41", 1, true), "41 characters is too long")
  set("device_name", "Studio iMac")

  set("proxy", "manual", "value")
  errors = settings.check(model.draft)
  assert(errors.proxy_host and not errors.proxy_port, "a manual proxy needs a host (the port defaults to 3128)")
  set("proxy_host", "proxy.larder.internal")
  set("proxy_port", "80800")
  assert(settings.check(model.draft).proxy_port ~= nil, "port 80800 is out of range")
  set("proxy_port", "8080")
  assert(next((settings.check(model.draft))) == nil, "now nothing is wrong")

  update({ kind = "all_folders", toggle = { checked = false } })
  local _, warnings = settings.check(model.draft)
  assert(not d.photos and not d.documents and warnings.photos ~= nil, "nothing ticked: told, but not blocked")
  update({ kind = "all_folders", toggle = { checked = true } })
  assert(d.photos and d.documents and d.desktop and d.downloads, "all ticked at once")

  set("quiet_from", "9pm")
  assert(settings.check(model.draft).quiet_from ~= nil, "quiet hours want HH:MM")
  set("quiet", false, "on")
  assert(settings.check(model.draft).quiet_from == nil, "but not while quiet hours are off")
  set("quiet", true, "on")
  set("quiet_from", "21:30")

  -- save, and what is on disk: through File › Save, as ⌘S / Ctrl+S chooses it
  update({ kind = "menu", menu = { item = "save" } })
  assert(kui.fs.exists(path) and model.toast.tone == "ok" and model.toast.message:find("Saved", 1, true), "saved: " .. tostring(model.toast.message))
  local disk = kui.toml.decode(kui.fs.read(path))
  assert(disk.device_name == "Studio iMac" and disk.proxy == "manual" and disk.proxy_port == "8080" and disk.versions == 45, "the file holds the draft")
  assert(#settings.diff(model.saved, model.draft) == 0, "nothing unsaved after a save")

  -- revert and reset
  set("transfers", 12, "integer")
  set("verbose", true, "on")
  update({ kind = "revert" })
  assert(model.draft.transfers == 4 and not model.draft.verbose, "revert brings back what is saved")
  assert(#model.toasts >= 2, "the toasts stack: the save's and the revert's")
  update({ kind = "undo", id = model.toast.id })
  assert(model.draft.transfers == 12 and model.draft.verbose, "the revert's Undo brings the edits back")
  update({ kind = "revert" })
  update({ kind = "reset" })
  assert(model.confirm == "reset", "reset asks first")
  update({ kind = "cancel" })
  assert(model.confirm == nil and model.draft.device_name == "Studio iMac", "cancelled: nothing changed")
  update({ kind = "reset" })
  update({ kind = "confirm_reset" })
  assert(#settings.diff(settings.DEFAULTS, model.draft) == 0 and #settings.diff(model.saved, model.draft) > 0, "reset: the defaults, not yet saved")
  update({ kind = "revert" })

  -- the next run reads the file
  local again = init()
  assert(again.saved.device_name == "Studio iMac" and again.draft.quiet_from == "21:30", "a new window reads the saved file")
  kui.fs.write(path, "device_name = [oops")
  again = init()
  assert(again.problem ~= nil and again.saved.device_name == "Maren’s MacBook Pro", "a broken file: the defaults, and the window says why")
  kui.fs.write(path, 'device_name = 42\nversions = 7\nunknown = "x"\n')
  again = init()
  assert(again.saved.device_name == "Maren’s MacBook Pro" and again.saved.versions == 7 and again.saved.unknown == nil,
    "a value of the wrong type, or an unknown key, is ignored")

  -- sections and search
  update({ kind = "query", change = { text = "proxy" } })
  update({ kind = "section", section = "sync" })
  assert(model.section == "sync" and model.query == "", "a section clears the search")
  for _, s in ipairs({ "neutral", "phosphor", "ledger" }) do update({ kind = "skin", change = { value = s } }) end
  for _, s in ipairs(settings.SECTIONS) do update({ kind = "section", section = s.value }); kui.sleep(30) end

end
