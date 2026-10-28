---@meta kui-runner
-- The kui runner's standard library (ADR 0064) for lua-language-server;
-- `kui types` writes it beside kui's DSL and Kit's module. A function
-- marked "waits" yields inside a task (`kui.spawn`) and blocks anywhere
-- else. A call that can fail on its input or on the world (a file, text
-- read in, the network, a process, a database) has a `try_` form that
-- returns `true` and what the call returns, or `false` and the error's
-- message: `local ok, text = kui.fs.try_read(path)`. The plain call raises
-- the message at its caller's line.

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
---@return boolean ok
---@return string? line_or_err
function Process:try_read_line(stream) end
---@return boolean ok
---@return string? err
function Process:try_write(data) end
---@return boolean ok
---@return integer|string code_or_err
function Process:try_wait() end

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
---@return boolean ok
---@return table<string, any>[]|string rows_or_err
function Db:try_query(sql, params) end
---@return boolean ok
---@return table<string, any>|string|nil row_or_err
function Db:try_first(sql, params) end
---@return boolean ok
---@return { changes: integer, last_id: integer }|string result_or_err
function Db:try_exec(sql, params) end
---@param fn fun(db: kui.Db)
---@return boolean ok
---@return string? err
function Db:try_transaction(fn) end

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
---@return boolean ok
---@return string? err
function Worker:try_dispatch(msg) end

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
---@param fn fun(env: table)
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
---The app's name (the manifest's, else its folder's).
---@type string
kui.name = ""

---@class kui.ArgOption
---@field type? "string"|"integer"|"number"|"boolean" The default's type when left out, else "string".
---@field default? any
---@field short? string
---@field help? string
---@field required? boolean
---@field many? boolean Repeatable: a list of every value given.
---@field choices? string[]
---@field env? string An environment variable read when the line leaves it out.
---@field value? string The value's name in the help.

---@class kui.ArgPositional
---@field name string
---@field type? "string"|"integer"|"number"|"boolean"
---@field help? string
---@field required? boolean
---@field many? boolean
---@field default? any
---@field choices? string[]

---@class kui.ArgSpec
---@field name? string
---@field about? string
---@field usage? string
---@field epilog? string
---@field options? table<string, kui.ArgOption>
---@field args? kui.ArgPositional[]
---@field rest? string|boolean Keep arguments past the positionals under this name (true: "rest").

---The command line (`kui.args`, or `argv`) read against `spec`: each option under its key, `--help` printed from the spec, a mistake said with what to type instead (both end the app before its window).
---@param spec kui.ArgSpec
---@param argv? string[]
---@return table<string, any>
function kui.parse_args(spec, argv) end

---@class kui.Log
---@overload fun(...: any)
kui.log = {}
---@param ... any
function kui.log.debug(...) end
---@param ... any
function kui.log.info(...) end
---@param ... any
function kui.log.warn(...) end
---@param ... any
function kui.log.error(...) end

---The devtools panel on, off, or (with nothing) toggled.
---@param on? boolean
function kui.devtools(on) end

---What a window gets from the OS, sent into a headless run (`kui run
-----headless`): a test drives the app as a user does, through the core's
---key routing, the Tab ring and hit testing. Each is handled before the
---next frame, in the order sent. A windowed run raises an error.
kui.input = {}
---A key pressed and released, by the name `on_key` reports it under:
---"tab", "down", "a", "f2".
---@param name string
---@param mods? { shift?: boolean, ctrl?: boolean, alt?: boolean, super?: boolean }
function kui.input.key(name, mods) end
---The pointer to a point of the window, nothing pressed.
---@param x number
---@param y number
function kui.input.move(x, y) end
---The pointer there, the primary button pressed and released.
---@param x number
---@param y number
function kui.input.click(x, y) end
---Typed text, as the OS delivers it to the focused editor.
---@param s string
function kui.input.text(s) end

---Digests as lowercase hex.
kui.hash = {}
---@return string
function kui.hash.sha1(data) end
---@return string
function kui.hash.sha256(data) end
---@return string
function kui.hash.sha512(data) end
---64-bit FNV-1a: a quick name, not a secret.
---@return string
function kui.hash.fnv(data) end
---The process environment, after `.env` and `.env.local`.
---@type table<string, string>
kui.env = {}
---@return string?
function kui.getenv(name) end
---The app's folder.
---@type string
kui.dir = ""
---The app's id: the manifest's `id` (reverse DNS), else its folder's name.
---@type string
kui.id = ""
---The app's own version, the manifest's `version`.
---@type string?
kui.app_version = nil
---The app's own data folder (Application Support, $XDG_DATA_HOME, %APPDATA%), named by its id.
---@type string
kui.data_dir = ""
---The app's own cache folder, named by its id.
---@type string
kui.cache_dir = ""
---@class kui.Packed
---@field runner string The kui it was packed for.
---@field bytecode boolean
---@field hash string
---What `kui pack` wrote, in a packed app; nil under `kui run`.
---@type kui.Packed?
kui.packed = nil
---@type string
kui.version = ""

---Runs `path` in a Lua state of its own on a thread of its own.
---@param path string A module beside the app.
---@param handler? fun(msg: any, w: kui.Worker) What the worker dispatches; `update` without one.
---@return kui.Worker
function kui.worker(path, handler) end
---@param path string
---@param handler? fun(msg: any, w: kui.Worker)
---@return boolean ok
---@return kui.Worker|string worker_or_err
function kui.try_worker(path, handler) end

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
---`t` (or a new table) marked a list: empty, it encodes as `[]`.
---@generic T
---@param t? T[]
---@return T[]
function kui.json.array(t) end
---@return boolean ok
---@return any value_or_err
function kui.json.try_decode(text) end
---@param opts? { pretty?: boolean }
---@return boolean ok
---@return string text_or_err
function kui.json.try_encode(value, opts) end
kui.toml = {}
---@return table
function kui.toml.decode(text) end
---@return string
function kui.toml.encode(value) end
---@return boolean ok
---@return table|string value_or_err
function kui.toml.try_decode(text) end
---@return boolean ok
---@return string text_or_err
function kui.toml.try_encode(value) end
kui.yaml = {}
---@return any
function kui.yaml.decode(text) end
---@return string
function kui.yaml.encode(value) end
---@return boolean ok
---@return any value_or_err
function kui.yaml.try_decode(text) end
---@return boolean ok
---@return string text_or_err
function kui.yaml.try_encode(value) end

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
---The files under `dir` (waits), relative to it.
---@param dir? string
---@param opts? { skip?: string[], match?: string, dirs?: boolean }
---@return string[]
function kui.fs.walk(dir, opts) end
---Reads `paths` (waits): their texts in order, false for one that could not be read.
---@param paths string[]
---@return (string|false)[]
function kui.fs.read_all(paths) end
function kui.fs.mkdir(path) end
function kui.fs.remove(path) end
function kui.fs.rename(from, to) end
---@return boolean ok
---@return string text_or_err
function kui.fs.try_read(path) end
---@return boolean ok
---@return string? err
function kui.fs.try_write(path, data) end
---@return boolean ok
---@return string? err
function kui.fs.try_append(path, data) end
---@return boolean ok
---@return string[]|string names_or_err
function kui.fs.try_list(path) end
---@return boolean ok
---@return string[]|string lines_or_err
function kui.fs.try_lines(path) end
---@return boolean ok
---@return boolean|string created_or_err
function kui.fs.try_create(path, data) end
---@param dir? string
---@param opts? { skip?: string[], match?: string, dirs?: boolean }
---@return boolean ok
---@return string[]|string files_or_err
function kui.fs.try_walk(dir, opts) end
---@return boolean ok
---@return string? err
function kui.fs.try_mkdir(path) end
---@return boolean ok
---@return string? err
function kui.fs.try_remove(path) end
---@return boolean ok
---@return string? err
function kui.fs.try_rename(from, to) end

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
---The command `pid` was started with (waits), nil when none.
---@return string?
function kui.process.command_line(pid) end
---@param args? string[]
---@param opts? { cwd?: string, env?: table<string,string>, stdin?: string }
---@return boolean ok
---@return kui.ProcessResult|string result_or_err
function kui.process.try_run(cmd, args, opts) end
---@param opts? { cwd?: string, env?: table<string,string>, group?: boolean, detach?: boolean }
---@return boolean ok
---@return kui.Process|string process_or_err
function kui.process.try_spawn(cmd, args, opts) end

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
---A refused connection or a timeout is `false`; a 404 is a response.
---@param target string|kui.RequestOpts
---@param opts? kui.RequestOpts
---@return boolean ok
---@return kui.Response|string response_or_err
function kui.http.try_request(target, opts) end
---@return boolean ok
---@return kui.Response|string response_or_err
function kui.http.try_get(url, opts) end
---@return boolean ok
---@return kui.Response|string response_or_err
function kui.http.try_post(url, opts) end
---@return boolean ok
---@return kui.Response|string response_or_err
function kui.http.try_put(url, opts) end
---@return boolean ok
---@return kui.Response|string response_or_err
function kui.http.try_patch(url, opts) end
---@return boolean ok
---@return kui.Response|string response_or_err
function kui.http.try_delete(url, opts) end
---@return boolean ok
---@return kui.Response|string response_or_err
function kui.http.try_head(url, opts) end
---@param opts? { port?: integer, host?: string, static?: string }
---@return boolean ok
---@return kui.Server|string server_or_err
function kui.http.try_serve(opts) end

kui.sqlite = {}
---A database beside the app, or `:memory:`.
---@param path? string
---@return kui.Db
function kui.sqlite.open(path) end
---@param path? string
---@return boolean ok
---@return kui.Db|string db_or_err
function kui.sqlite.try_open(path) end

---A recording (ADR 0075): Rust reads and analyses it, a script draws its channels — `spans` in Kit's waterfall's shape, `frames` a row a finished frame, `instants` the waterfall's marks. Times are ms; rows 0-based.
---@class kui.Recording
---@field spans kui.Channel
---@field frames kui.Channel
---@field instants kui.Channel
---@field moved boolean The watched file moved since the last poll.
---@field ended boolean
---@field budget number The frame budget, ms.
---@field filtered boolean
---@field poll fun(self: kui.Recording): string, integer "more" and how many records, "idle", "ended", or "reset".
---@field session fun(self: kui.Recording): table<string, any>
---@field frame fun(self: kui.Recording, row: integer): table<string, any>
---@field span fun(self: kui.Recording, row: integer): table<string, any>
---@field stats fun(self: kui.Recording, first?: integer, last?: integer): table<string, any>
---@field series fun(self: kui.Recording, name: string): kui.Channel?
---@field series_names fun(self: kui.Recording): string[]
---@field filter_error string? Why a `where` stopped keeping what a poll brought, if it threw.
---@field filter fun(self: kui.Recording, f?: { origins?: string[], name?: string, min_us?: number, verdicts?: string[], hide_unchanged?: boolean, where?: fun(span: { name: string, origin: string, start: number, len: number, self: number, depth: integer, track: integer, frame: integer }): boolean }): integer, integer

kui.trace = {}
---A recording read whole, then watched: `{ watch = false }` reads it once; `message` is what `update` gets when the file moves.
---@param path string
---@param opts? { watch?: boolean, message?: any }
---@return kui.Recording
function kui.trace.open(path, opts) end
---@param path string
---@param opts? { watch?: boolean, message?: any }
---@return boolean ok
---@return kui.Recording|string rec_or_err
function kui.trace.try_open(path, opts) end
---A recording from its bytes.
---@param bytes string
---@return kui.Recording
function kui.trace.read(bytes) end
---@param bytes string
---@return boolean ok
---@return kui.Recording|string rec_or_err
function kui.trace.try_read(bytes) end

---@type any The app's state: `init()`'s result, kept across a --watch reload.
model = nil
