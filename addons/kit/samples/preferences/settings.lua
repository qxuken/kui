-- Tidewater's settings: what there is, where it shows, what it defaults to,
-- and what makes a value wrong. Tidewater is a (pretend) photo and file
-- sync client; this sample is its preferences window.

local M = {}

M.SECTIONS = {
  { value = "general", label = "General", detail = "This device, language and look." },
  { value = "sync", label = "Sync", detail = "What is kept in step with your other devices, and how." },
  { value = "network", label = "Network", detail = "Bandwidth, metered connections and the proxy." },
  { value = "notifications", label = "Notifications", detail = "What Tidewater tells you, and when it stays quiet." },
  { value = "advanced", label = "Advanced", detail = "For when something needs looking into." },
}

M.DEFAULTS = {
  device_name = "Maren’s MacBook Pro",
  language = "en-GB",
  appearance = "system",
  open_at_login = true,
  menu_bar = true,
  folder = "~/Tidewater",
  mode = "two-way",
  photos = true, documents = true, desktop = false, downloads = false,
  versions = 30,
  conflicts = "keep-both",
  upload_limit = 0,
  download_limit = 0,
  metered = true,
  proxy = "system",
  proxy_host = "",
  proxy_port = "3128",
  notify_done = false,
  notify_conflicts = true,
  notify_errors = true,
  notify_quota = true,
  quiet = true,
  quiet_from = "22:00",
  quiet_to = "07:00",
  transfers = 4,
  verbose = false,
  ignore = ".DS_Store, *.tmp, node_modules/",
}

M.LANGUAGES = {
  { value = "en-GB", label = "English (UK)" }, { value = "en-US", label = "English (US)" }, { value = "de-DE", label = "Deutsch" },
  { value = "fr-FR", label = "Français" }, { value = "es-ES", label = "Español" }, { value = "ja-JP", label = "日本語" },
}

M.CONFLICTS = {
  { value = "keep-both", label = "Keep both copies" },
  { value = "newest", label = "Keep the newest" },
  { value = "ask", label = "Ask me each time" },
}

M.FOLDERS = { { "photos", "Photos" }, { "documents", "Documents" }, { "desktop", "Desktop" }, { "downloads", "Downloads" } }

local function trim(s) return (s:gsub("^%s+", ""):gsub("%s+$", "")) end

local function hhmm(s)
  local h, m = s:match("^(%d%d?):(%d%d)$")
  return h and tonumber(h) < 24 and tonumber(m) < 60
end

-- What is wrong with `v`, by key: errors block saving, warnings do not.
function M.check(v)
  local errors, warnings = {}, {}
  local name = trim(v.device_name)
  if name == "" then errors.device_name = "Give this device a name: it is how your other devices list it."
  elseif #name > 40 then errors.device_name = string.format("At most 40 characters (this is %d).", utf8.len(name) or #name)
  elseif name:find("[/\\]") then errors.device_name = "A name cannot hold / or \\." end

  local folder = trim(v.folder)
  if folder == "" then errors.folder = "Choose where synced files live."
  elseif not folder:match("^[/~]") then errors.folder = "Use a full path, starting with / or ~."
  else
    local path = folder:gsub("^~", kui.env.HOME or "~")
    if not kui.fs.exists(path) then warnings.folder = "This folder does not exist yet: Tidewater will create it." end
  end

  if v.proxy == "manual" then
    if trim(v.proxy_host) == "" then errors.proxy_host = "A manual proxy needs a host."
    elseif not trim(v.proxy_host):match("^[%w%.%-]+$") then errors.proxy_host = "Only letters, digits, dots and dashes." end
    local port = tonumber(v.proxy_port)
    if not port or port % 1 ~= 0 or port < 1 or port > 65535 then errors.proxy_port = "A port is a whole number from 1 to 65535." end
  end

  if v.quiet then
    if not hhmm(v.quiet_from) then errors.quiet_from = "Write the time as HH:MM, 24-hour." end
    if not hhmm(v.quiet_to) then errors.quiet_to = "Write the time as HH:MM, 24-hour." end
  end

  local any = v.photos or v.documents or v.desktop or v.downloads
  if not any then warnings.photos = "Nothing is ticked: this device will sync nothing." end
  if v.transfers > 8 and v.upload_limit > 0 and v.upload_limit < 512 then
    warnings.transfers = "Many transfers over a tight upload limit: each crawls."
  end
  return errors, warnings
end

-- The keys whose values differ between two settings tables.
function M.diff(a, b)
  local out = {}
  for k in pairs(M.DEFAULTS) do
    if a[k] ~= b[k] then out[#out + 1] = k end
  end
  table.sort(out)
  return out
end

function M.copy(t)
  local out = {}
  for k in pairs(M.DEFAULTS) do out[k] = t[k] end
  return out
end

-- Reads the TOML file at `path` over the defaults; missing is defaults.
function M.load(path)
  local out = M.copy(M.DEFAULTS)
  if path and kui.fs.exists(path) then
    local ok, t = pcall(kui.toml.decode, kui.fs.read(path))
    if not ok then return out, "The settings file could not be read (" .. tostring(t):match("^[^\n]*") .. "); showing the defaults." end
    for k, v in pairs(t) do
      if M.DEFAULTS[k] ~= nil and type(v) == type(M.DEFAULTS[k]) then out[k] = v end
    end
  end
  return out
end

function M.encode(v)
  return kui.toml.encode(M.copy(v))
end

function M.save(path, v)
  local dir = path:match("^(.*)/[^/]+$")
  if dir and not kui.fs.exists(dir) then kui.fs.mkdir(dir) end
  kui.fs.write(path, "# Tidewater settings, written by the preferences window.\n" .. M.encode(v))
end

return M
