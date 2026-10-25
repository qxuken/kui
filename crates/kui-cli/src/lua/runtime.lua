-- The runner's Lua half (ADR 0064): tasks, dispatch and the server's router,
-- installed in the app's state (and each worker's) before the app's script
-- runs. The Rust half has already put its functions in `kui`; the ones
-- spelled `kui.__name` are its doors and not the app's API.

local kui = kui

-- Tasks: coroutines waiting on an operation, by the operation's id.
local waiting = {}
-- The task being resumed right now, so a waiting call can tell a task's
-- yield from a coroutine of the app's own.
local current = nil

local function step(co, ...)
  local outer = current
  current = co
  local ok, id = coroutine.resume(co, ...)
  current = outer
  if not ok then
    kui.__task_error(tostring(id))
    return
  end
  if coroutine.status(co) == "dead" then return end
  if math.type(id) == "integer" then
    waiting[id] = co
  else
    kui.__task_error("a task yielded something other than a kui call's wait; use coroutines of your own inside it, not around kui calls")
  end
end

--- Runs `fn(...)` as a task: a waiting call inside it yields to the frame
--- rather than blocking it, and the task goes on when the answer arrives.
function kui.spawn(fn, ...)
  assert(type(fn) == "function", "kui.spawn takes a function")
  local co = coroutine.create(fn)
  step(co, ...)
  return co
end

-- What every waiting call ends in: inside a task it yields the operation's
-- id; anywhere else it blocks until the answer is there.
function kui.__await(id)
  local co = coroutine.running()
  local ok, value
  if current ~= nil and co == current then
    ok, value = coroutine.yield(id)
  else
    ok, value = kui.__block(id)
  end
  if not ok then error(value, 0) end
  return value
end

function kui.__resume(id, ok, value)
  local co = waiting[id]
  if not co then return end
  waiting[id] = nil
  step(co, ok, value)
end

--- How many tasks are waiting on something.
function kui.pending()
  local n = 0
  for _ in pairs(waiting) do n = n + 1 end
  return n
end

--- Waits `ms` milliseconds.
function kui.sleep(ms)
  return kui.__await(kui.__sleep(ms))
end

kui.timer = {}

--- Calls `fn` once after `ms`; `:cancel()` on what it returns stops it.
function kui.timer.after(ms, fn)
  local t = { live = true }
  function t:cancel() self.live = false end
  kui.spawn(function()
    kui.sleep(ms)
    if t.live then fn() end
  end)
  return t
end

--- Calls `fn` every `ms` until `:cancel()`.
function kui.timer.every(ms, fn)
  local t = { live = true }
  function t:cancel() self.live = false end
  kui.spawn(function()
    while t.live do
      kui.sleep(ms)
      if t.live then fn() end
    end
  end)
  return t
end

-- Dispatch: messages for `update`, delivered before the next frame.
local queue = {}

--- Sends `msg` to the app's `update` (in a worker: to the app, through the
--- handler it gave the worker).
function kui.dispatch(msg)
  if kui.__worker then
    kui.__to_app(msg)
    return
  end
  queue[#queue + 1] = msg
  if #queue == 1 then kui.__poke() end
end

local function deliver(msg)
  local f = rawget(_G, "update") or rawget(_G, "on_event")
  if f then
    local ok, err = pcall(f, msg)
    if not ok then kui.__task_error(tostring(err)) end
  end
end

function kui.__drain()
  local n = 0
  while #queue > 0 do
    local q = queue
    queue = {}
    for _, m in ipairs(q) do deliver(m) end
    n = n + #q
    if n > 100000 then
      kui.__task_error("update dispatched messages in a loop for 100,000 rounds; stopped")
      queue = {}
    end
  end
end

-- An event the app's nodes raised: to `update` unless the app defines its
-- own `on_event`, which replaces this one.
function on_event(ev)
  local f = rawget(_G, "update")
  if f then f(ev) end
end

-- What wants the frame's env: `update` hears events without one, so
-- clipboard writes, focus moves and measurements wait for the next view.
local frame_fns = {}

--- Runs `fn(env)` at the start of the next frame, before `view`: for what
--- only a frame can do — `env.set_clipboard`, `env.set_focus`,
--- `env.measure_text` — from `update`, a task or a timer.
function kui.next_frame(fn)
  frame_fns[#frame_fns + 1] = fn
  if #frame_fns == 1 then kui.__poke() end
end

local wrapped
function kui.__wrap_view()
  local inner = rawget(_G, "view")
  if type(inner) ~= "function" or inner == wrapped then return end
  wrapped = function(env, ...)
    if #frame_fns > 0 then
      local fns = frame_fns
      frame_fns = {}
      for _, f in ipairs(fns) do
        local ok, err = pcall(f, env)
        if not ok then kui.__task_error("next_frame: " .. tostring(err)) end
      end
    end
    return inner(env, ...)
  end
  _G.view = wrapped
end

-- Workers the app started, by id: their handlers.
local workers = {}

function kui.__from_worker(id, msg)
  local w = workers[id]
  local f = w and w.handler
  if f then
    local ok, err = pcall(f, msg, w)
    if not ok then kui.__task_error(tostring(err)) end
  else
    deliver(msg)
  end
end

function kui.__worker_exit(id, err)
  local w = workers[id]
  if not w then return end
  w.running = false
  local msg = { kind = "worker_exit", worker = w.path, error = err }
  if w.handler then
    pcall(w.handler, msg, w)
  else
    deliver(msg)
  end
end

--- Runs `path` (a module beside the app) in a Lua state of its own on a
--- thread of its own. `w:dispatch(msg)` reaches its `on_message`; what it
--- dispatches reaches `handler(msg, w)`, or `update` without one.
function kui.worker(path, handler)
  local id = kui.__worker_start(path)
  local w = { id = id, path = path, handler = handler, running = true }
  function w:dispatch(msg) kui.__worker_send(self.id, msg) end
  function w:stop() kui.__worker_stop(self.id); self.running = false end
  workers[id] = w
  return w
end

-- The server's routes: method → list of { pattern, names, handler }.
local servers = {}

local function compile(path)
  local names = {}
  local pat = "^" .. path:gsub("[%-%.%+%[%]%(%)%$%^%%%?%*]", "%%%0"):gsub("/:([%w_]+)", function(name)
    names[#names + 1] = name
    return "/([^/]+)"
  end):gsub("/%%%*$", "/(.*)") .. "$"
  return pat, names
end

local function route(s, method, path, handler)
  local pat, names = compile(path)
  local list = s.routes[method] or {}
  s.routes[method] = list
  list[#list + 1] = { pat = pat, names = names, handler = handler }
  return s
end

--- Serves HTTP: `kui.http.serve { port = 3050, static = "public" }`, then
--- `s:get("/api/items/:id", function(req) ... end)`. Each request is a
--- task; a handler returns a string, a table (sent as JSON), or `{ status,
--- headers, body | json | html }`.
function kui.http.serve(opts)
  opts = opts or {}
  local id, port = kui.__serve(opts)
  local s = { id = id, port = port, routes = {} }
  function s:get(p, f) return route(self, "GET", p, f) end
  function s:post(p, f) return route(self, "POST", p, f) end
  function s:put(p, f) return route(self, "PUT", p, f) end
  function s:patch(p, f) return route(self, "PATCH", p, f) end
  function s:delete(p, f) return route(self, "DELETE", p, f) end
  function s:route(m, p, f) return route(self, m:upper(), p, f) end
  function s:stop() kui.__serve_stop(self.id) end
  servers[id] = s
  return s
end

local function respond(server, id, res)
  if res == nil then
    kui.__respond(server, id, { status = 204 })
  elseif type(res) == "string" then
    kui.__respond(server, id, { status = 200, body = res })
  elseif type(res) == "table" and (res.status or res.body or res.json ~= nil or res.html) then
    kui.__respond(server, id, res)
  else
    kui.__respond(server, id, { status = 200, json = res })
  end
end

function kui.__request(server, id, req)
  local s = servers[server]
  local found
  for _, r in ipairs(s and s.routes[req.method] or {}) do
    local caps = { req.path:match(r.pat) }
    if caps[1] ~= nil or req.path:match(r.pat) then
      req.params = {}
      for i, name in ipairs(r.names) do req.params[name] = caps[i] end
      found = r
      break
    end
  end
  if not found then
    kui.__respond(server, id, { status = 404, body = "no route for " .. req.method .. " " .. req.path })
    return
  end
  kui.spawn(function()
    local ok, res = pcall(found.handler, req)
    if ok then
      respond(server, id, res)
    else
      kui.__task_error(req.method .. " " .. req.path .. ": " .. tostring(res))
      kui.__respond(server, id, { status = 500, body = tostring(res) })
    end
  end)
end

-- The waiting calls: each starts an operation and waits on it.

--- `kui.http.request{ url, method, headers, json | body, timeout }` or
--- `kui.http.request(url, opts)`: `{ status, ok, headers, body, json? }`.
function kui.http.request(target, opts)
  return kui.__await(kui.http.__request(target, nil, opts))
end
for _, m in ipairs({ "get", "post", "put", "patch", "delete", "head" }) do
  local method = m:upper()
  kui.http[m] = function(url, opts)
    return kui.__await(kui.http.__request(url, method, opts))
  end
end

--- Runs a command to its end: `{ code, ok, stdout, stderr }`.
function kui.process.run(cmd, args, opts)
  return kui.__await(kui.process.__run(cmd, args, opts))
end

--- Starts a command: `p:read_line()` (nil at the end), `p:lines()`,
--- `p:write(s)`, `p:close()`, `p:wait()` (the exit code), `p:kill(sig)`.
function kui.process.spawn(cmd, args, opts)
  local ud = kui.process.__spawn(cmd, args, opts)
  local p = { pid = ud.pid }
  function p:read_line(stream) return kui.__await(ud:__read(stream)) end
  function p:lines(stream) return function() return p:read_line(stream) end end
  function p:wait() return kui.__await(ud:__wait()) end
  function p:write(s) ud:write(s) end
  function p:close() ud:close() end
  function p:kill(sig) return ud:kill(sig) end
  return p
end

local open_db = kui.sqlite.open

--- Opens a database (a path beside the app, or `:memory:`): `db:query(sql,
--- params)` → rows, `db:first(...)` → a row or nil, `db:exec(sql, params)`
--- → `{ changes, last_id }`, `db:transaction(fn)`.
function kui.sqlite.open(path)
  local ud = open_db(path)
  local db = {}
  function db:query(sql, params) return kui.__await(ud:__query(sql, params)) end
  function db:first(sql, params) return self:query(sql, params)[1] end
  function db:exec(sql, params) return kui.__await(ud:__exec(sql, params)) end
  function db:transaction(fn)
    self:exec("BEGIN")
    local ok, err = pcall(fn, self)
    if ok then
      self:exec("COMMIT")
    else
      self:exec("ROLLBACK")
      error(err, 0)
    end
  end
  return db
end
