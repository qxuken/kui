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

-- The runner's convention for a call that can fail on its input or on the
-- world: its `try_` form answers `true` and the value, or `false` and the
-- message, and the plain call is made from it, raising the message at its
-- caller's line with no traceback. Each such call has one value.
local function raising(try_f)
  return function(...)
    local ok, v = try_f(...)
    if not ok then error(v, 2) end
    return v
  end
end

local function plain(t, names)
  for _, name in ipairs(names) do t[name] = raising(t["try_" .. name]) end
end

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
-- id; anywhere else it blocks until the answer is there. `ok, value`.
function kui.__try_await(id)
  local co = coroutine.running()
  if current ~= nil and co == current then
    return coroutine.yield(id)
  end
  return kui.__block(id)
end

function kui.__await(id)
  local ok, value = kui.__try_await(id)
  if not ok then error(value, 0) end
  return value
end

-- A waiting call's `try_` form: `start(...)`, a Rust `try_` form that
-- answers an operation's id, then the operation's answer.
local function waits(start, ...)
  local ok, id = start(...)
  if not ok then return false, id end
  return kui.__try_await(id)
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
  return n
end

-- An event the app's nodes raised: to `update` unless the app defines its
-- own `on_event`, which replaces this one.
function on_event(ev)
  local f = rawget(_G, "update")
  if f then f(ev) end
end

-- `kui.log(...)`, `kui.log.warn(...)` and `print(...)`: printed, and kept
-- for the devtools' Log tab.
kui.log = setmetatable({}, { __call = function(_, ...) kui.__log("info", ...) end })
for _, level in ipairs({ "debug", "info", "warn", "error" }) do
  kui.log[level] = function(...) kui.__log(level, ...) end
end
function print(...) kui.__log("info", ...) end

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
function kui.try_worker(path, handler)
  local ok, id = kui.__try_worker_start(path)
  if not ok then return false, id end
  local w = { id = id, path = path, handler = handler, running = true }
  function w:try_dispatch(msg) return kui.__try_worker_send(self.id, msg) end
  function w:stop() kui.__worker_stop(self.id); self.running = false end
  plain(w, { "dispatch" })
  workers[id] = w
  return true, w
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
function kui.http.try_serve(opts)
  local ok, id, port = kui.__try_serve(opts or {})
  if not ok then return false, id end
  local s = { id = id, port = port, routes = {} }
  function s:get(p, f) return route(self, "GET", p, f) end
  function s:post(p, f) return route(self, "POST", p, f) end
  function s:put(p, f) return route(self, "PUT", p, f) end
  function s:patch(p, f) return route(self, "PATCH", p, f) end
  function s:delete(p, f) return route(self, "DELETE", p, f) end
  function s:route(m, p, f) return route(self, m:upper(), p, f) end
  function s:stop() kui.__serve_stop(self.id) end
  servers[id] = s
  return true, s
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

--- `kui.http.request{ url, method, headers, json | body, timeout (ms) }` or
--- `kui.http.request(url, opts)`: `{ status, ok, headers, body, json? }`.
function kui.http.try_request(target, opts)
  return waits(kui.http.__try_request, target, nil, opts)
end
for _, m in ipairs({ "get", "post", "put", "patch", "delete", "head" }) do
  local method = m:upper()
  kui.http["try_" .. m] = function(url, opts)
    return waits(kui.http.__try_request, url, method, opts)
  end
end

--- The files under `dir` (waits): `{ skip = { ".git", "node_modules" },
--- match = "**/src/*.ts", dirs = false }`, paths relative to `dir`.
function kui.fs.try_walk(dir, opts)
  return waits(kui.fs.__try_walk, dir, opts)
end

--- Reads `paths` (waits): their texts in order, false for one that could
--- not be read.
function kui.fs.read_all(paths)
  return kui.__await(kui.fs.__read_all(paths))
end

--- The command `pid` was started with (waits), nil when no such process.
function kui.process.command_line(pid)
  return kui.__await(kui.process.__command_line(pid))
end

--- Runs a command to its end: `{ code, ok, stdout, stderr }`.
function kui.process.try_run(cmd, args, opts)
  return waits(kui.process.__try_run, cmd, args, opts)
end

--- Starts a command: `p:read_line()` (nil at the end), `p:lines()`,
--- `p:write(s)`, `p:close()`, `p:wait()` (the exit code), `p:kill(sig)`.
function kui.process.try_spawn(cmd, args, opts)
  local ok, ud = kui.process.__try_spawn(cmd, args, opts)
  if not ok then return false, ud end
  local p = { pid = ud.pid }
  function p:try_read_line(stream) return kui.__try_await(ud:__read(stream)) end
  function p:lines(stream)
    return function()
      local read, line = p:try_read_line(stream)
      if not read then error(line, 2) end
      return line
    end
  end
  function p:try_wait() return kui.__try_await(ud:__wait()) end
  function p:try_write(s) return ud:try_write(s) end
  function p:close() ud:close() end
  function p:kill(sig) return ud:kill(sig) end
  plain(p, { "read_line", "wait", "write" })
  return true, p
end

local open_db = kui.sqlite.try_open

--- Opens a database (a path beside the app, or `:memory:`): `db:query(sql,
--- params)` → rows, `db:first(...)` → a row or nil, `db:exec(sql, params)`
--- → `{ changes, last_id }`, `db:transaction(fn)`.
--- `db:transaction(fn)` runs `fn(db)` between BEGIN and COMMIT, and rolls
--- back when it raises: the plain call raises `fn`'s error again, as it
--- was; `try_transaction` answers `false` and its message.
function kui.sqlite.try_open(path)
  local ok, ud = open_db(path)
  if not ok then return false, ud end
  local db = {}
  function db:try_query(sql, params) return waits(ud.__try_query, ud, sql, params) end
  function db:try_first(sql, params)
    local found, rows = self:try_query(sql, params)
    if not found then return false, rows end
    return true, rows[1]
  end
  function db:try_exec(sql, params) return waits(ud.__try_exec, ud, sql, params) end
  local function run(self, fn)
    local began, err = self:try_exec("BEGIN")
    if not began then return false, err end
    local ran, failure = pcall(fn, self)
    if not ran then
      self:try_exec("ROLLBACK")
      return false, failure, true
    end
    local done, cerr = self:try_exec("COMMIT")
    if not done then return false, cerr end
    return true
  end
  function db:try_transaction(fn)
    local done, err = run(self, fn)
    if done then return true end
    return false, kui.__message(err)
  end
  function db:transaction(fn)
    local done, err, raised = run(self, fn)
    if done then return end
    if raised then error(err, 0) end
    error(err, 2)
  end
  plain(db, { "query", "first", "exec" })
  return true, db
end

-- The plain calls, each made from its `try_` form.
plain(kui, { "worker" })
plain(kui.json, { "decode", "encode" })
plain(kui.toml, { "decode", "encode" })
plain(kui.yaml, { "decode", "encode" })
plain(kui.fs, { "read", "write", "append", "create", "list", "lines", "walk", "mkdir", "remove", "rename" })
plain(kui.process, { "run", "spawn" })
plain(kui.http, { "request", "get", "post", "put", "patch", "delete", "head", "serve" })
plain(kui.sqlite, { "open" })
plain(kui.trace, { "open", "read" })
