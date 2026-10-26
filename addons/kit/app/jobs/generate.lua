-- A worker (kui.worker): builds the gallery's big data off the window's
-- thread and hands it back as channels, which cross without copying.
--
--   { kind = "rows", n }    → { kind = "rows", rows = <channel>, ms }
--   { kind = "view", query, sort, descending } → { kind = "rows", ... }
--   { kind = "trace", n }   → { kind = "trace", spans = <channel>, ms }

local METHODS = { "GET", "GET", "GET", "POST", "PUT", "DELETE" }
local PATHS = { "/api/items", "/api/users", "/api/search", "/health", "/api/orders", "/static/app.js" }
local STATUS = { 200, 200, 200, 200, 201, 204, 304, 404, 500 }

-- A small, repeatable generator: the same rows every run.
local seed = 0x2545F491
local function rand()
  seed = (seed * 1103515245 + 12345) % 2147483648
  return seed / 2147483648
end

-- The rows as built, kept for the views the app asks for: the channel
-- itself, read a column at a time while a view is made. (A copy as Lua
-- lists, kept beside it, was 21 MB the worker held for good.)
---@type any
local all = nil

local NAMES = { "id", "method", "path", "status", "tone", "ms", "size" }

-- A status's tone, for the table's badge column.
local function tone_of(status)
  if status >= 500 then return "danger" end
  if status >= 400 then return "warn" end
  if status >= 300 then return "neutral" end
  return "ok"
end

local function rows(n)
  local id, method, path, status, tone, ms, size = {}, {}, {}, {}, {}, {}, {}
  for i = 1, n do
    id[i] = i - 1
    method[i] = METHODS[1 + math.floor(rand() * #METHODS)]
    path[i] = PATHS[1 + math.floor(rand() * #PATHS)] .. "/" .. math.floor(rand() * 9000 + 1000)
    status[i] = STATUS[1 + math.floor(rand() * #STATUS)]
    tone[i] = tone_of(status[i])
    -- A long tail: most requests are quick, a few are slow.
    ms[i] = math.floor((rand() ^ 3) * 1800 + 2) + 0.0
    size[i] = math.floor(rand() * 64000)
  end
  all = channel({ id = id, method = method, path = path, status = status, tone = tone, ms = ms, size = size })
  return all
end

-- The rows whose path holds `query`, sorted by `field`: a new channel.
-- The columns it reads are read out of the rows' channel for the while,
-- and let go once the view is made.
local function view(query, field, descending)
  local keep = {}
  local q = query and query ~= "" and query:lower() or nil
  local n = #all
  if q then
    local path, method = all:column("path"), all:column("method")
    for i = 1, n do
      if path[i]:lower():find(q, 1, true) or method[i]:lower() == q then keep[#keep + 1] = i end
    end
  else
    for i = 1, n do keep[i] = i end
  end
  local sortable = false
  for _, name in ipairs(NAMES) do sortable = sortable or name == field end
  local col = sortable and all:column(field) or nil
  if col then
    table.sort(keep, function(a, b)
      if col[a] == col[b] then return a < b end
      if descending then return col[a] > col[b] end
      return col[a] < col[b]
    end)
  end
  local out = {}
  for _, name in ipairs(NAMES) do
    local src, dst = all:column(name), {}
    for j, i in ipairs(keep) do dst[j] = src[i] end
    out[name] = dst
  end
  local ch = channel(out)
  out, keep, col = nil, nil, nil
  collectgarbage()
  return ch
end

local function trace(frames)
  local name, start, len, depth, group, target = {}, {}, {}, {}, {}, {}
  local function span(s, l, d, g, n, f)
    local i = #name + 1
    name[i], start[i], len[i], depth[i], group[i], target[i] = n, s, l, d, g, "frame " .. f
  end
  for f = 0, frames - 1 do
    local base = f * 1000 / 60
    local j = rand()
    local build = { base + 0.2, 4 + 3 * j }
    local layout = { build[1] + build[2] + 0.1, 1.5 + j }
    local paint = { layout[1] + layout[2] + 0.1, 2.5 + 1.5 * j }
    span(base, 1000 / 60, 0, 0, "frame", f)
    span(build[1], build[2], 1, 1, "view", f)
    span(layout[1], layout[2], 1, 2, "layout", f)
    span(paint[1], paint[2], 1, 3, "paint", f)
    span(build[1] + 0.3, build[2] - 0.8, 2, 4, "walk", f)
    span(layout[1] + 0.1, layout[2] * 0.6, 2, 5, "shape", f)
  end
  return channel { name = name, start = start, len = len, depth = depth, group = group, target = target }
end

function on_message(msg)
  local t0 = kui.now()
  if msg.kind == "rows" then
    local ch = rows(msg.n)
    kui.dispatch({ kind = "rows", rows = ch, ms = kui.now() - t0 })
  elseif msg.kind == "view" and all then
    local ch = view(msg.query, msg.sort, msg.descending)
    kui.dispatch({ kind = "rows", rows = ch, ms = kui.now() - t0, filtered = true })
  elseif msg.kind == "trace" then
    local ch = trace(msg.n)
    kui.dispatch({ kind = "trace", spans = ch, ms = kui.now() - t0 })
  end
end
