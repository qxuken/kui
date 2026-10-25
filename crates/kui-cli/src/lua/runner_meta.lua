---@meta kui-runner
-- The kui runner's standard library (ADR 0064) for lua-language-server;
-- `kui types` writes it beside kui's DSL and Kit's module. A function
-- marked "waits" yields inside a task (`kui.spawn`) and blocks anywhere
-- else.

---@class kui.Response
---@field status integer
---@field ok boolean
---@field headers table<string, string>
---@field body string
---@field json any Decoded when the response says it is JSON.

---@class kui.RequestOpts
---@field url? string
---@field method? string
---@field headers? table<string, string>
---@field json? any A body sent as JSON.
---@field body? string
---@field timeout? number Milliseconds (30,000 by default).

---@class kui.Request
---@field method string
---@field path string
---@field query table<string, string>
---@field headers table<string, string>
---@field body string
---@field json any
---@field params table<string, string> The route's `:name` parts.

---@alias kui.Handler fun(req: kui.Request): any

---@class kui.Server
---@field port integer
local Server = {}
---@param path string `/api/items/:id`, `/static/*`
---@param handler kui.Handler A string, a table (sent as JSON) or `{ status, headers, body | json | html }`.
---@return kui.Server
function Server:get(path, handler) end
---@return kui.Server
function Server:post(path, handler) end
---@return kui.Server
function Server:put(path, handler) end
---@return kui.Server
function Server:patch(path, handler) end
---@return kui.Server
function Server:delete(path, handler) end
---@return kui.Server
function Server:route(method, path, handler) end
function Server:stop() end

---@class kui.ProcessResult
---@field code integer
---@field ok boolean
---@field stdout string
---@field stderr string

---@class kui.Process
---@field pid integer
local Process = {}
---The next line of its output, nil at the end (waits).
---@param stream? "stdout"|"stderr"
---@return string?
function Process:read_line(stream) end
---@return fun(): string?
function Process:lines(stream) end
---@param data string
function Process:write(data) end
function Process:close() end
---Its exit code (waits).
---@return integer
function Process:wait() end
---Ends it, or sends `sig` ("term", "int", "hup", "kill"); its group's whole when spawned `group`. False when it had already exited.
---@param sig? string
---@return boolean
function Process:kill(sig) end

---@class kui.Db
local Db = {}
---Rows as tables by column name (waits). `params`: a list for `?`, a table for `:name`.
---@return table<string, any>[]
function Db:query(sql, params) end
---@return table<string, any>?
function Db:first(sql, params) end
---@return { changes: integer, last_id: integer }
function Db:exec(sql, params) end
---@param fn fun(db: kui.Db)
function Db:transaction(fn) end

---@class kui.Timer
local Timer = {}
function Timer:cancel() end

---@class kui.Worker
---@field id integer
---@field path string
---@field running boolean
local Worker = {}
---Sends `msg` to the worker's `on_message`.
function Worker:dispatch(msg) end
function Worker:stop() end

kui = {}

---Runs `fn(...)` as a task: its waiting calls yield to the frame.
---@param fn function
---@return thread
function kui.spawn(fn, ...) end
---Sends `msg` to the app's `update` before the next frame (in a worker: to the app).
function kui.dispatch(msg) end
---Waits `ms` milliseconds (waits).
---@param ms number
function kui.sleep(ms) end
---Milliseconds since the Unix epoch.
---@return number
function kui.now() end
---Tasks waiting on something.
---@return integer
function kui.pending() end
---Ends the app with `code` (0 by default): the window closes and `on_exit` runs. Ctrl-C in the terminal does the same.
---@param code? integer
function kui.exit(code) end
---Runs `fn(env)` at the start of the next frame, before `view`: for what only a frame can do (`env.set_clipboard`, `env.set_focus`, `env.measure_text`) from `update`, a task or a timer.
---@param fn fun(env: kui.Env)
function kui.next_frame(fn) end
---What followed `--` on the command line: `kui run app -- --port 3001`.
---@type string[]
kui.args = {}
---The folder `kui` was started from (relative paths are the app's).
---@type string
kui.cwd = ""
---"macos", "windows", "linux", or the OS's own name.
---@type string
kui.platform = ""
---The cores to share work between.
---@type integer
kui.cpus = 1
---The process environment, after `.env` and `.env.local`.
---@type table<string, string>
kui.env = {}
---@return string?
function kui.getenv(name) end
---The app's folder.
---@type string
kui.dir = ""
---@type string
kui.version = ""

---Runs `path` in a Lua state of its own on a thread of its own.
---@param path string A module beside the app.
---@param handler? fun(msg: any, w: kui.Worker) What the worker dispatches; `update` without one.
---@return kui.Worker
function kui.worker(path, handler) end

kui.timer = {}
---@return kui.Timer
function kui.timer.after(ms, fn) end
---@return kui.Timer
function kui.timer.every(ms, fn) end

kui.json = {}
---@return any
function kui.json.decode(text) end
---@param opts? { pretty?: boolean }
---@return string
function kui.json.encode(value, opts) end
kui.toml = {}
---@return table
function kui.toml.decode(text) end
---@return string
function kui.toml.encode(value) end
kui.yaml = {}
---@return any
function kui.yaml.decode(text) end
---@return string
function kui.yaml.encode(value) end

kui.fs = {}
---@return string
function kui.fs.read(path) end
function kui.fs.write(path, data) end
function kui.fs.append(path, data) end
---@return boolean
function kui.fs.exists(path) end
---@return string[]
function kui.fs.list(path) end
---@return string[]
function kui.fs.lines(path) end
---@return { size: integer, dir: boolean, file: boolean, modified: number }?
function kui.fs.stat(path) end
---Writes a file that does not exist yet; false when it does.
---@return boolean
function kui.fs.create(path, data) end
function kui.fs.mkdir(path) end
function kui.fs.remove(path) end
function kui.fs.rename(from, to) end

kui.process = {}
---Runs a command to its end (waits).
---@param args? string[]
---@param opts? { cwd?: string, env?: table<string,string>, stdin?: string }
---@return kui.ProcessResult
function kui.process.run(cmd, args, opts) end
---`group`: leads a process group of its own, so `kill` reaches all it started. `detach`: outlives the app (spawned processes end with it otherwise).
---@param opts? { cwd?: string, env?: table<string,string>, group?: boolean, detach?: boolean }
---@return kui.Process
function kui.process.spawn(cmd, args, opts) end
---A signal to any process, or to the group it leads; `0` asks whether it is alive. Whether it arrived.
---@param sig? string|integer
---@param group? boolean
---@return boolean
function kui.process.kill(pid, sig, group) end
---This process's id.
---@type integer
kui.process.pid = 0

kui.http = {}
---Each waits.
---@param target string|kui.RequestOpts
---@param opts? kui.RequestOpts
---@return kui.Response
function kui.http.request(target, opts) end
---@param url string
---@param opts? kui.RequestOpts
---@return kui.Response
function kui.http.get(url, opts) end
---@return kui.Response
function kui.http.post(url, opts) end
---@return kui.Response
function kui.http.put(url, opts) end
---@return kui.Response
function kui.http.patch(url, opts) end
---@return kui.Response
function kui.http.delete(url, opts) end
---@return kui.Response
function kui.http.head(url, opts) end
---Serves HTTP; each request runs as a task.
---@param opts? { port?: integer, host?: string, static?: string }
---@return kui.Server
function kui.http.serve(opts) end

kui.sqlite = {}
---A database beside the app, or `:memory:`.
---@param path? string
---@return kui.Db
function kui.sqlite.open(path) end

---@type any The app's state: `init()`'s result, kept across a --watch reload.
model = nil
