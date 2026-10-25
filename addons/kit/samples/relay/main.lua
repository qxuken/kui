-- Relay: an HTTP client on Kit and the kui runner. It serves its own
-- dummy API (Larder, a grocery-delivery backend: api.lua) and talks to it —
-- a request bar, params and headers, a response with its status, body,
-- headers and a timing waterfall, and the history of what was sent.
--
--   kui run addons/kit/samples/relay
--   kui run addons/kit/samples/relay -- --port 4400

local api = require("api")
local view_of = require("view")

ENVS = {
  { value = "local", label = "Local mock" },
  { value = "staging", label = "Staging (offline)" },
}

-- What the history is seeded from, oldest last.
local SEEDS = {
  { method = "GET", path = "/api/orders", params = { { "status", "open" }, { "limit", "20" } } },
  { method = "GET", path = "/api/orders/ord_8F3K2" },
  { method = "GET", path = "/api/products", params = { { "category", "dairy" } } },
  { method = "POST", path = "/api/orders", body = '{\n  "customer": "cus_2210",\n  "slot": "16:00–18:00",\n  "items": [\n    { "sku": "VEG-0112", "qty": 2 },\n    { "sku": "BEV-0520", "qty": 1 }\n  ]\n}' },
  { method = "GET", path = "/api/customers/cus_9999" },
  { method = "POST", path = "/api/inventory/sync" },
  { method = "GET", path = "/api/reports/daily", params = { { "date", "2026-10-24" } } },
}

local function default_headers()
  return {
    { on = true, key = "Accept", value = "application/json" },
    { on = true, key = "Authorization", value = "Bearer lrd_test_7f3a9c21e4" },
    { on = false, key = "X-Debug-Trace", value = "1" },
  }
end

local function pairs_of(list)
  local out = {}
  for i, p in ipairs(list or {}) do out[i] = { on = true, key = p[1], value = p[2] } end
  return out
end

function init()
  return {
    skin = "ledger",
    side = 0.26,
    vsplit = 0.36,
    env = "local",
    method = "GET",
    url = "{{base}}/api/orders",
    params = pairs_of(SEEDS[1].params),
    headers = default_headers(),
    body = "",
    req_tab = "params",
    res_tab = "body",
    history = {},
    selected = nil,
    query = "",
    sending = false,
    response = nil,
    error = nil,
    hits = 0,
    toast = nil,
  }
end

-- ---------- the mock API ----------

local function on_hit(method, path, status)
  kui.dispatch({ kind = "hit", method = method, path = path, status = status })
end

if not boot then
  local opts = kui.parse_args({
    name = "relay",
    about = "an HTTP client, with the dummy API it talks to",
    options = { port = { type = "integer", default = 4317, short = "p", help = "the mock API's port (0: any free one)" } },
  })
  local ok, s = pcall(api.serve, opts.port, on_hit)
  if not ok then
    kui.log.warn("port " .. opts.port .. " is taken; serving the mock API on a free port")
    s = api.serve(0, on_hit)
  end
  boot = { server = s }
end

BASES = {
  ["local"] = "http://127.0.0.1:" .. boot.server.port,
  staging = "http://127.0.0.1:9",
}

-- ---------- sending ----------

local function resolve(url, env)
  return (url:gsub("{{base}}", BASES[env] or ""))
end

local function encode(s)
  return (s:gsub("[^%w%-%._~]", function(c) return string.format("%%%02X", c:byte()) end))
end

-- The URL as sent: the base filled in and the ticked params appended.
function full_url(m)
  local url = resolve(m.url, m.env)
  local q = {}
  for _, p in ipairs(m.params) do
    if p.on and p.key ~= "" then q[#q + 1] = encode(p.key) .. "=" .. encode(p.value) end
  end
  if #q > 0 then url = url .. (url:find("?", 1, true) and "&" or "?") .. table.concat(q, "&") end
  return url
end

local function header(h, name)
  if not h then return nil end
  local want = name:lower()
  for k, v in pairs(h) do if k:lower() == want then return v end end
end

-- `Server-Timing: auth;dur=0.8, db;dur=34.2` → { { name, ms } }
local function server_timing(h)
  local out = {}
  for name, dur in (h or ""):gmatch("([%w_%-]+);dur=([%d%.]+)") do out[#out + 1] = { name = name, ms = tonumber(dur) } end
  return out
end

-- The phases Relay draws: what the server said it did, placed after the
-- moment it said the request arrived; the rest is the network.
local function timing(t0, t1, headers)
  local total = t1 - t0
  local arrived = tonumber(header(headers, "x-arrived-at") or "")
  local phases = server_timing(header(headers, "server-timing"))
  local spans = { { name = "request", start = 0, len = total, depth = 0, group = 0 } }
  local at = arrived and math.max(0, math.min(arrived - t0, total)) or 0
  if arrived then spans[#spans + 1] = { name = "connect + send", start = 0, len = at, depth = 1, group = 1 } end
  if #phases > 0 then
    local server = { name = "server", start = at, len = 0, depth = 1, group = 2 }
    spans[#spans + 1] = server
    for _, ph in ipairs(phases) do
      spans[#spans + 1] = { name = ph.name, start = at + server.len, len = ph.ms, depth = 2, group = 3 }
      server.len = server.len + ph.ms
    end
    local after = at + server.len
    if total > after then spans[#spans + 1] = { name = "download", start = after, len = total - after, depth = 1, group = 4 } end
  end
  return spans, total
end

local REASONS = { [200] = "OK", [201] = "Created", [204] = "No Content", [304] = "Not Modified", [400] = "Bad Request",
  [401] = "Unauthorized", [403] = "Forbidden", [404] = "Not Found", [409] = "Conflict", [422] = "Unprocessable",
  [500] = "Server Error", [502] = "Bad Gateway", [503] = "Unavailable" }
function reason(status) return REASONS[status] or "" end

-- The request a model describes, sent and answered (waits: call it in a
-- task). The entry is what the history keeps.
local function perform(m)
  local req = { method = m.method, url = full_url(m), headers = {}, timeout = 10000 }
  for _, h in ipairs(m.headers) do if h.on and h.key ~= "" then req.headers[h.key] = h.value end end
  if m.method ~= "GET" and m.body ~= "" then
    req.body = m.body
    req.headers["Content-Type"] = req.headers["Content-Type"] or "application/json"
  end
  local entry = { method = m.method, url = m.url, params = m.params, headers = m.headers, body = m.body, env = m.env, at = kui.now() }
  local t0 = kui.now()
  local ok, r = pcall(kui.http.request, req)
  local t1 = kui.now()
  if not ok then
    entry.error = { message = (tostring(r):gsub("^.-:%d+: ", "")), ms = t1 - t0, url = req.url }
    return entry
  end
  local spans, total = timing(t0, t1, r.headers)
  entry.response = { status = r.status, headers = r.headers, body = r.body, json = r.json, ms = total, size = #(r.body or ""), spans = spans }
  return entry
end

local function copy_rows(list)
  local out = {}
  for i, p in ipairs(list) do out[i] = { on = p.on, key = p.key, value = p.value } end
  return out
end

local function send(m)
  m.sending, m.error = true, nil
  local snapshot = { method = m.method, url = m.url, params = copy_rows(m.params), headers = copy_rows(m.headers), body = m.body, env = m.env }
  kui.spawn(function()
    kui.dispatch({ kind = "answered", entry = perform(snapshot) })
  end)
end

local function load(m, e)
  m.method, m.url, m.env, m.body = e.method, e.url, e.env, e.body
  -- copies: editing a loaded request must not rewrite the history
  m.params, m.headers = copy_rows(e.params), copy_rows(e.headers)
  m.response, m.error = e.response, e.error
end

-- The seeds sent once, side by side, so the history opens with real
-- answers; their ages are made up (minutes apart) to read like a morning's.
if not boot.seeded then
  boot.seeded = true
  local entries, left = {}, #SEEDS
  for i, s in ipairs(SEEDS) do
    kui.spawn(function()
      local e = perform({ method = s.method, url = "{{base}}" .. s.path, params = pairs_of(s.params),
        headers = default_headers(), body = s.body or "", env = "local" })
      e.at = kui.now() - (i * 7 - 4) * 60000
      entries[i] = e
      left = left - 1
      if left == 0 then kui.dispatch({ kind = "seeded", entries = entries }) end
    end)
  end
end

local function copy(text, what)
  kui.next_frame(function(env) env.set_clipboard(text) end)
  model.toast = { message = what .. " copied to the clipboard.", tone = "ok" }
end

local function edit_row(list, i, field, value)
  local row = list[i]
  if row then row[field] = value end
end

-- The history the search leaves, newest first.
function visible_history(m)
  local q = m.query:lower()
  if q == "" then return m.history end
  local out = {}
  for _, e in ipairs(m.history) do
    if (e.method .. " " .. e.url):lower():find(q, 1, true) then out[#out + 1] = e end
  end
  return out
end

function curl(m)
  local parts = { "curl -X " .. m.method .. " '" .. full_url(m) .. "'" }
  for _, h in ipairs(m.headers) do
    if h.on and h.key ~= "" then parts[#parts + 1] = "-H '" .. h.key .. ": " .. h.value .. "'" end
  end
  if m.method ~= "GET" and m.body ~= "" then parts[#parts + 1] = "--data '" .. m.body:gsub("%s*\n%s*", " ") .. "'" end
  return table.concat(parts, " \\\n  ")
end

function update(msg)
  local m = model
  local k = msg.kind
  if k == "send" then
    if not m.sending then send(m) end
  elseif k == "answered" then
    table.insert(m.history, 1, msg.entry)
    m.sending = false
    m.response, m.error = msg.entry.response, msg.entry.error
    m.query, m.selected = "", 0
  elseif k == "seeded" then
    for _, e in ipairs(msg.entries) do m.history[#m.history + 1] = e end
    if not m.response and not m.error and not m.sending and m.history[1] then load(m, m.history[1]); m.selected = 0 end
  elseif k == "hit" then m.hits = m.hits + 1
  elseif k == "method" then m.method = msg.change.value
  elseif k == "url" then m.url = msg.change.text
  elseif k == "env" then m.env = msg.change.value
  elseif k == "req_tab" then m.req_tab = msg.change.value
  elseif k == "res_tab" then m.res_tab = msg.change.value
  elseif k == "body" then
    m.body = msg.change.text
    -- Said under the body as it is typed: the API reads JSON only.
    local ok, err = true, nil
    if m.body:match("%S") then ok, err = pcall(kui.json.decode, m.body) end
    m.body_error = not ok and ("Not JSON yet: " .. tostring(err):match("[^\n]*"):gsub("^.-: ", "")) or nil
  elseif k == "side" then m.side = msg.resize.fraction
  elseif k == "vsplit" then m.vsplit = msg.resize.fraction
  elseif k == "skin" then m.skin = msg.change.value
  elseif k == "query" then m.query = msg.change.text; m.selected = nil
  elseif k == "pick" then
    local e = visible_history(m)[msg.select.row + 1]
    if e then load(m, e); m.selected = msg.select.row end
  -- the params' and headers' rows: list = "params" | "headers"
  elseif k == "row_on" then edit_row(m[msg.list], msg.i, "on", msg.toggle.checked)
  elseif k == "row_key" then edit_row(m[msg.list], msg.i, "key", msg.change.text)
  elseif k == "row_value" then edit_row(m[msg.list], msg.i, "value", msg.change.text)
  elseif k == "row_add" then table.insert(m[msg.list], { on = true, key = "", value = "" })
  elseif k == "row_remove" then table.remove(m[msg.list], msg.i)
  elseif k == "copy_body" then if m.response then copy(m.response.body or "", "The body") end
  elseif k == "copy_curl" then copy(curl(m), "A curl command")
  elseif k == "clear_history" then m.history, m.selected = {}, nil
  elseif k == "toast_off" then m.toast = nil
  end
end

function view(env)
  return view_of(model, env)
end
