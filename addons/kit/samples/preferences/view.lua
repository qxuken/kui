-- The preferences window: sections on the left; the chosen section's
-- groups of settings on the right, each a label, a line of help and its
-- control; a bar at the foot with what is unsaved, Revert and Save.

local kit = require("kit.ui")
local settings = require("settings")

local SKINS = {
  { value = "ledger", label = "Ledger" }, { value = "neutral", label = "Neutral" }, { value = "phosphor", label = "Phosphor" },
}

local function words(s, o)
  local t = { size = "$kitSizeLabel", family = "$kitUi", color = "$kitMuted", wrap = "word" }
  for k, v in pairs(o or {}) do t[k] = v end
  return text(s, t)
end

local function rate(kb)
  if kb == 0 then return "unlimited" end
  if kb >= 1000 then return string.format("%.1f MB/s", kb / 1000) end
  return string.format("%d KB/s", kb)
end

local function set(key, extra)
  local msg = { kind = "set", key = key }
  for k, v in pairs(extra or {}) do msg[k] = v end
  return msg
end

-- This frame's validation, by setting: a field marks itself with it.
local ERRORS = {}

-- A text field for one setting. `says` puts its error under it; a field
-- sharing a row with another is only marked, the row saying why once.
local function field(v, key, o)
  local err = ERRORS[key]
  local p = { key = key, label = o.label, hide_label = true, value = v[key], mono = o.mono, width = o.width,
    placeholder = o.placeholder, disabled = o.disabled, on_change = set(key),
    error = err and (o.says and err or "") or nil }
  return kit.text_field(p)
end

local function switch(v, key, label)
  return kit.switch({ key = key, label = label, on = v[key], on_toggle = set(key) })
end

-- ---------- what there is ----------
--
-- A setting: its section and group, what it reads as, a line of help,
-- words a search also finds it by, and its control.

local ROWS = {
  -- General
  { "general", "This device", "device_name", "Device name", "How your phone and other computers list this one.", "computer",
    function(v) return field(v, "device_name", { label = "Device name", width = 320, says = true }) end, inline = true },
  { "general", "This device", "open_at_login", "Open at login", "Start syncing as soon as you sign in.", "startup launch",
    function(v) return switch(v, "open_at_login", "Open Tidewater when I log in") end },
  { "general", "This device", "menu_bar", "Menu bar", "A small icon with the sync state and the last few files.", "tray icon",
    function(v) return switch(v, "menu_bar", "Show in the menu bar") end },
  { "general", "Look", "language", "Language", "The language of Tidewater's windows and notifications.", "locale",
    function(v) return row { width = 220, kit.select({ key = "language", options = settings.LANGUAGES, value = v.language, on_change = set("language") }) } end },
  { "general", "Look", "appearance", "Appearance", "System follows macOS as it switches between light and dark.", "theme dark light mode",
    function(v) return kit.radio_group({ key = "appearance", label = "Appearance", hide_label = true, value = v.appearance, on_change = set("appearance"), options = {
      { value = "system", label = "Match the system" }, { value = "light", label = "Light" }, { value = "dark", label = "Dark" } } }) end },

  -- Sync
  { "sync", "Folder", "folder", "Sync folder", "Everything under it is kept in step with your other devices.", "path location directory",
    function(v) return field(v, "folder", { label = "Sync folder", mono = true, width = 360, says = true }) end, inline = true },
  { "sync", "Folder", "mode", "Direction", "Upload only keeps the cloud a backup; nothing deleted there reaches this device.", "two-way backup upload download",
    function(v) return kit.segmented({ key = "mode", label = "Sync direction", value = v.mode, on_change = set("mode"), options = {
      { value = "two-way", label = "Two-way" }, { value = "upload", label = "Upload only" }, { value = "download", label = "Download only" } } }) end },
  { "sync", "What syncs", "photos", "Folders", "Tick the folders in your home folder to sync as well.", "photos documents desktop downloads",
    function(v)
      local n = 0
      for _, f in ipairs(settings.FOLDERS) do if v[f[1]] then n = n + 1 end end
      local boxes = {}
      for _, f in ipairs(settings.FOLDERS) do
        boxes[#boxes + 1] = kit.checkbox({ key = f[1], label = f[2], checked = v[f[1]], on_toggle = set(f[1]) })
      end
      return column { width = "grow", gap = 2,
        kit.checkbox({ key = "all", label = "All of them", checked = n == #settings.FOLDERS, mixed = n > 0 and n < #settings.FOLDERS,
          on_toggle = { kind = "all_folders" } }),
        column { width = "grow", gap = 2, pad = { l = 22 }, table.unpack(boxes) },
      }
    end },
  { "sync", "History", "versions", "Keep old versions", "Earlier versions of a changed file stay restorable this long.", "history retention days",
    function(v) return column { width = 320, kit.slider({ key = "versions", label = "Kept for", value_label = string.format("%d days", v.versions), value = v.versions,
      min = 1, max = 90, step = 1, on_change = set("versions", { integer = true }) }) } end },
  { "sync", "History", "conflicts", "When a file changed in two places", "Keep both renames the other copy “(conflict, device)”.", "conflict merge",
    function(v) return row { width = 240, kit.select({ key = "conflicts", options = settings.CONFLICTS, value = v.conflicts, on_change = set("conflicts") }) } end },

  -- Network
  { "network", "Bandwidth", "upload_limit", "Upload limit", "Leave room for video calls on a slow line.", "speed throttle",
    function(v) return column { width = 320, kit.slider({ key = "upload_limit", label = "Upload", value_label = rate(v.upload_limit), value = v.upload_limit,
      min = 0, max = 10000, step = 250, on_change = set("upload_limit", { integer = true }) }) } end },
  { "network", "Bandwidth", "download_limit", "Download limit", "0 is unlimited.", "speed throttle",
    function(v) return column { width = 320, kit.slider({ key = "download_limit", label = "Download", value_label = rate(v.download_limit), value = v.download_limit,
      min = 0, max = 10000, step = 250, on_change = set("download_limit", { integer = true }) }) } end },
  { "network", "Bandwidth", "metered", "Metered connections", "A phone hotspot counts as metered.", "hotspot cellular data",
    function(v) return switch(v, "metered", "Pause syncing on metered connections") end },
  { "network", "Proxy", "proxy", "Proxy", "Most networks need none; a company network may.", "http socks",
    function(v) return kit.radio_group({ key = "proxy", label = "Proxy", hide_label = true, value = v.proxy, on_change = set("proxy"), options = {
      { value = "none", label = "No proxy" }, { value = "system", label = "Use the system's settings" }, { value = "manual", label = "Manual" } } }) end },
  { "network", "Proxy", "proxy_host", "Proxy host and port", "Only with a manual proxy.", "address server",
    function(v)
      local off = v.proxy ~= "manual"
      return row { width = "grow", gap = 8, cross_align = "center",
        field(v, "proxy_host", { label = "Proxy host", mono = true, width = 240, placeholder = "proxy.example.com", disabled = off }),
        text(":", { size = "$kitSize", family = "$kitData", color = "$kitMuted" }),
        field(v, "proxy_port", { label = "Proxy port", mono = true, width = 80, placeholder = "3128", disabled = off }),
      }
    end, also = { "proxy_port" } },

  -- Notifications
  { "notifications", "Tell me when", "notify_errors", "Errors", "A file could not sync, or the account needs attention.", "alert",
    function(v) return switch(v, "notify_errors", "Tell me about errors") end },
  { "notifications", "Tell me when", "notify_conflicts", "Conflicts", "A file changed in two places at once.", "",
    function(v) return switch(v, "notify_conflicts", "Tell me about conflicts") end },
  { "notifications", "Tell me when", "notify_quota", "Storage", "You are using more than 90% of your plan.", "quota space full",
    function(v) return switch(v, "notify_quota", "Warn me when storage runs low") end },
  { "notifications", "Tell me when", "notify_done", "Finished", "Each time a big upload completes; most people leave this off.", "complete done",
    function(v) return switch(v, "notify_done", "Tell me when uploads finish") end },
  { "notifications", "Quiet hours", "quiet", "Quiet hours", "Notifications wait until the morning, except errors.", "do not disturb night",
    function(v)
      return column { width = "grow", gap = 8,
        switch(v, "quiet", "Hold notifications overnight"),
        row { width = "grow", gap = 8, cross_align = "center",
          words("from", { wrap = nil }),
          field(v, "quiet_from", { label = "Quiet from", mono = true, width = 70, disabled = not v.quiet }),
          words("to", { wrap = nil }),
          field(v, "quiet_to", { label = "Quiet until", mono = true, width = 70, disabled = not v.quiet }),
        },
      }
    end, also = { "quiet_from", "quiet_to" } },

  -- Advanced
  { "advanced", "Transfers", "transfers", "Parallel transfers", "More is faster on a fast line and slower on a slow one.", "concurrency threads",
    function(v) return column { width = 320, kit.slider({ key = "transfers", label = "Transfers", value_label = string.format("%d at a time", v.transfers), value = v.transfers,
      min = 1, max = 16, step = 1, on_change = set("transfers", { integer = true }) }) } end },
  { "advanced", "Transfers", "ignore", "Ignore", "Files and folders never synced, comma-separated; * matches anything.", "exclude pattern glob",
    function(v) return field(v, "ignore", { label = "Ignore patterns", mono = true, says = true }) end, inline = true },
  { "advanced", "Diagnostics", "verbose", "Verbose logging", "Writes every transfer to the log; support may ask for it.", "debug log",
    function(v) return switch(v, "verbose", "Log everything") end },
}

local function matches(r, q)
  if q == "" then return true end
  for i = 4, 6 do if r[i]:lower():find(q, 1, true) then return true end end
  return false
end

local function changed(m, r)
  if m.saved[r[3]] ~= m.draft[r[3]] then return true end
  for _, k in ipairs(r.also or {}) do if m.saved[k] ~= m.draft[k] then return true end end
  return false
end

-- ---------- a setting, a group, a section ----------

local function setting(m, r, errors, warnings)
  local errs, warn = { errors[r[3]] }, warnings[r[3]]
  for _, k in ipairs(r.also or {}) do
    if errors[k] and errors[k] ~= errs[#errs] then errs[#errs + 1] = errors[k] end
    warn = warn or warnings[k]
  end
  local err = #errs > 0 and table.concat(errs, " ") or nil
  local dirty = changed(m, r)
  return row { key = "row." .. r[3], width = "grow", gap = 24, pad = { y = 10 },
    column { width = 250, gap = 3,
      -- As tall as the badge with or without it: a "changed" that appears
      -- does not push the help line and the rows under it down.
      row { height = 18, gap = 6, cross_align = "center",
        text(r[4], { size = "$kitSize", family = "$kitUi", bold = true, color = "$kitFg" }),
        dirty and row { tooltip = "Changed, not saved yet", role = "image", label = "changed",
          kit.badge({ label = "changed", tone = "accent" }) } or nil,
      },
      words(r[5]),
    },
    column { width = "grow", gap = 6,
      r[7](m.draft),
      (err and not r.inline) and words(err, { color = "$kitDanger" }) or nil,
      (not err and warn) and words(warn, { color = "$kitWarn" }) or nil,
    },
  }
end

local function group(m, title, rows, errors, warnings)
  local kids = {}
  for i, r in ipairs(rows) do
    if i > 1 then kids[#kids + 1] = kit.divider({}) end
    kids[#kids + 1] = setting(m, r, errors, warnings)
  end
  return kit.panel({ key = "group." .. title, title = title, pad = 0 }, {
    column { width = "grow", pad = { x = 16, y = 4 }, table.unpack(kids) },
  })
end

-- Rows grouped by their group, in order.
local function groups(m, rows, errors, warnings)
  local order, by = {}, {}
  for _, r in ipairs(rows) do
    if not by[r[2]] then by[r[2]] = {}; order[#order + 1] = r[2] end
    table.insert(by[r[2]], r)
  end
  local out = {}
  for _, g in ipairs(order) do out[#out + 1] = group(m, g, by[g], errors, warnings) end
  return out
end

local function section_of(value)
  for _, s in ipairs(settings.SECTIONS) do if s.value == value then return s end end
end

local function heading(title, detail)
  return column { width = "grow", gap = 4,
    text(title, { size = 20, family = "$kitUi", bold = true, color = "$kitFg" }),
    detail and words(detail) or nil,
  }
end

local function advanced_extras(m)
  return {
    kit.panel({ key = "group.file", title = "The file", subtitle = boot.config:gsub("^" .. (kui.env.HOME or "~"), "~") }, {
      actions = { kit.button({ key = "toml", label = m.show_toml and "Hide" or "Show what is saved", variant = "ghost", icon = "code",
        pressed = m.show_toml, on_press = { kind = "show_toml" } }) },
      m.show_toml and kit.code({ key = "toml-text", text = settings.encode(m.draft), max_lines = 14 })
        or words("Your settings are kept as TOML; the preview shows them as Save would write them."),
    }),
    kit.panel({ key = "group.danger", title = "Start over" }, {
      row { width = "grow", gap = 16, cross_align = "center",
        column { width = "grow", gap = 3,
          text("Reset every setting", { size = "$kitSize", family = "$kitUi", bold = true, color = "$kitFg" }),
          words("Back to how Tidewater came. Your files are not touched; nothing is saved until you press Save.") },
        kit.button({ key = "reset", label = "Reset all…", icon = "reset", variant = "danger", on_press = { kind = "reset" } }),
      },
    }),
  }
end

local function content(m, errors, warnings)
  local q = m.query:lower():match("^%s*(.-)%s*$")
  local kids = {}
  if q ~= "" then
    local found = 0
    for _, s in ipairs(settings.SECTIONS) do
      local rows = {}
      for _, r in ipairs(ROWS) do if r[1] == s.value and matches(r, q) then rows[#rows + 1] = r end end
      if #rows > 0 then
        found = found + #rows
        kids[#kids + 1] = heading(s.label)
        for _, g in ipairs(groups(m, rows, errors, warnings)) do kids[#kids + 1] = g end
      end
    end
    if found == 0 then
      kids[1] = column { width = "grow", height = 320,
        kit.empty({ title = "No setting matches “" .. m.query .. "”", detail = "Try a word from its help, like “proxy” or “hotspot”." },
          { kit.button({ label = "Clear the search", on_press = { kind = "query", change = { text = "" } } }) }) }
    else
      table.insert(kids, 1, words(string.format("%d setting%s match “%s”", found, found == 1 and "" or "s", m.query)))
    end
  else
    local s = section_of(m.section)
    kids[#kids + 1] = heading(s.label, s.detail)
    local rows = {}
    for _, r in ipairs(ROWS) do if r[1] == s.value then rows[#rows + 1] = r end end
    for _, g in ipairs(groups(m, rows, errors, warnings)) do kids[#kids + 1] = g end
    if s.value == "advanced" then
      for _, n in ipairs(advanced_extras(m)) do kids[#kids + 1] = n end
    end
  end
  return column { key = "content." .. (q ~= "" and "search" or m.section), width = "grow", height = "grow", scroll_y = true,
    pad = { x = 24, y = 20 }, gap = 16,
    column { width = "grow", max_width = 860, gap = 16, table.unpack(kids) } }
end

-- ---------- the sidebar ----------

local function sidebar(m, errors)
  local rows = {}
  for _, s in ipairs(settings.SECTIONS) do
    local here = m.query == "" and m.section == s.value
    local dirty, bad = 0, 0
    for _, r in ipairs(ROWS) do
      if r[1] == s.value then
        if changed(m, r) then dirty = dirty + 1 end
        if errors[r[3]] then bad = bad + 1 end
        for _, k in ipairs(r.also or {}) do if errors[k] then bad = bad + 1 end end
      end
    end
    rows[#rows + 1] = row {
      key = "nav." .. s.value, role = "tab", label = s.label, selected = here,
      width = "grow", height = "$kitCtl", pad = { x = 10 }, gap = 6, cross_align = "center",
      radius = "$kitRadius", bg = here and "$kitSel" or nil, hover_bg = here and "$kitSel" or "$kitHover", cursor = "pointer",
      on_click = { kind = "section", section = s.value },
      text(s.label, { size = "$kitSize", family = "$kitUi", bold = here, color = here and "$kitSelFg" or "$kitFg" }),
      row { width = "grow" },
      bad > 0 and kit.badge({ label = tostring(bad), tone = "danger" })
        or dirty > 0 and kit.badge({ label = tostring(dirty), tone = "accent" }) or nil,
    }
  end
  return column { key = "sidebar", width = "grow", height = "grow", bg = "$kitSide", pad = 8, gap = 8,
    column { role = "tabList", width = "grow", gap = 2, table.unpack(rows) },
  }
end

-- ---------- the foot ----------

local function foot(m, errors)
  local diff = settings.diff(m.saved, m.draft)
  local bad = 0
  for _ in pairs(errors) do bad = bad + 1 end
  local file = boot.config:match("[^/]+$")
  if #diff == 0 then
    return kit.statusbar({ key = "foot" }, {
      words(m.saved_at and ("Saved to " .. file .. " at " .. os.date("%H:%M", math.floor(m.saved_at / 1000))) or ("All settings saved in " .. file), { wrap = nil }),
    })
  end
  return row { key = "foot", width = "grow", pad = { x = 16, y = 8 }, gap = 10, cross_align = "center", bg = "$kitPanel",
    kit.badge({ label = string.format("%d unsaved change%s", #diff, #diff == 1 and "" or "s"), tone = "accent", dot = true }),
    bad > 0 and words(string.format("%d to fix before saving", bad), { color = "$kitDanger", wrap = nil }) or nil,
    row { width = "grow" },
    kit.button({ key = "revert", label = "Revert", variant = "ghost", on_press = { kind = "revert" } }),
    kit.button({ key = "save", label = "Save", variant = "primary", icon = "check", shortcut = "⌘S", disabled = bad > 0,
      tooltip = bad > 0 and "Fix the settings marked in red first" or nil, on_press = { kind = "save" } }),
  }
end

return function(m, env)
  local errors, warnings = settings.check(m.draft)
  ERRORS = errors
  local changed_from_default = #settings.diff(settings.DEFAULTS, m.draft)
  return column { window_title = "Tidewater Settings", width = "grow", height = "grow", bg = "$kitBg",
    kit.settings({ skin = m.skin }),
    kit.toolbar({ key = "top" }, {
      text("Tidewater Settings", { size = "$kitSizeTitle", family = "$kitUi", bold = true, color = "$kitFg" }),
      trailing = {
        row { key = "search", width = 260,
          kit.search({ key = "query", value = m.query, placeholder = "Search settings", label = "Search the settings", focus = m.focus_search,
            on_change = { kind = "query" } }) },
        row { key = "skin", width = 120, kit.select({ key = "skin-select", options = SKINS, value = m.skin, on_change = { kind = "skin" } }) },
      },
    }),
    m.problem and column { key = "problem", width = "grow", pad = { x = 12, y = 8 },
      kit.notice({ tone = "warn", message = m.problem }) } or nil,
    kit.split({ key = "split", fraction = m.side, min = 170, on_resize = { kind = "side" } }, {
      first = { sidebar(m, errors) },
      second = { content(m, errors, warnings) },
    }),
    foot(m, errors),
    m.confirm == "reset" and kit.dialog({ key = "reset-dialog", title = "Reset every setting?", on_dismiss = { kind = "cancel" } }, {
      words(string.format("%d setting%s differ from the defaults. They go back now; nothing is written until you press Save, so Revert still brings them back.",
        changed_from_default, changed_from_default == 1 and "" or "s"), { size = "$kitSize", color = "$kitFg" }),
      actions = {
        kit.button({ label = "Cancel", on_press = { kind = "cancel" } }),
        kit.button({ label = "Reset all", variant = "danger", icon = "reset", on_press = { kind = "confirm_reset" } }),
      },
    }) or nil,
    m.toast and kit.toast({ key = "toast", message = m.toast.message, tone = m.toast.tone,
      action = m.undo and "Undo" or nil, action_name = m.undo and ("Undo: " .. m.toast.message) or nil,
      on_action = { kind = "undo" }, on_dismiss = { kind = "toast_off" } }) or nil,
  }
end
