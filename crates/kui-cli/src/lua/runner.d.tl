  -- The kui runner's standard library (ADR 0064), spliced into the
  -- `kui` record by `kui check`. A call marked "waits" yields inside a
  -- task (`kui.spawn`) and blocks anywhere else.

  -- Runs `fn` as a task: its waiting calls yield to the frame.
  spawn: function(fn: function(...: any), ...: any): thread
  -- Sends a message to the app's `update` before the next frame (in a
  -- worker: to the app).
  dispatch: function(msg: any)
  -- Waits `ms` milliseconds (waits).
  sleep: function(ms: number)
  -- Milliseconds since the Unix epoch.
  now: function(): number
  -- Tasks waiting on something.
  pending: function(): integer
  -- Ends the app with `code` (0 by default): the window closes and
  -- `on_exit` runs. Ctrl-C in the terminal does the same.
  exit: function(code?: integer)
  -- Runs `fn(env)` at the start of the next frame, before `view`: for
  -- what only a frame can do (`env.set_clipboard`, `env.set_focus`,
  -- `env.measure_text`) from `update`, a task or a timer.
  next_frame: function(fn: function(env: kui.Env))
  -- What followed `--` on the command line: `kui run app -- --port 3001`.
  args: {string}
  -- The folder `kui` was started from (relative paths are the app's).
  cwd: string
  -- "macos", "windows", "linux", or the OS's own name.
  platform: string
  -- The cores to share work between.
  cpus: integer
  -- The process environment, after `.env` and `.env.local`.
  env: {string:string}
  getenv: function(name: string): string
  -- The app's folder, and the runner's version.
  dir: string
  version: string

  record Timer
    cancel: function(self: Timer)
  end
  record timer
    after: function(ms: number, fn: function()): Timer
    every: function(ms: number, fn: function()): Timer
  end

  record json
    decode: function(text: string): any
    encode: function(value: any, opts?: {string:boolean}): string
  end
  record toml
    decode: function(text: string): {string:any}
    encode: function(value: {string:any}): string
  end
  record yaml
    decode: function(text: string): any
    encode: function(value: any): string
  end

  record Stat
    size: integer
    dir: boolean
    file: boolean
    modified: number
  end
  record fs
    read: function(path: string): string
    write: function(path: string, data: string)
    append: function(path: string, data: string)
    exists: function(path: string): boolean
    list: function(path?: string): {string}
    lines: function(path: string): {string}
    stat: function(path: string): Stat
    -- Writes a file that does not exist yet; false when it does.
    create: function(path: string, data: string): boolean
    mkdir: function(path: string)
    remove: function(path: string)
    rename: function(from: string, to: string)
  end

  record ProcessOpts
    cwd: string
    -- Added to the runner's environment.
    env: {string:string}
    stdin: string
    -- Leads a process group of its own: `kill` reaches all it started.
    group: boolean
    -- Outlives the app (spawned processes end with it otherwise).
    detach: boolean
  end
  record ProcessResult
    code: integer
    ok: boolean
    stdout: string
    stderr: string
  end
  record Process
    pid: integer
    read_line: function(self: Process, stream?: string): string
    lines: function(self: Process, stream?: string): function(): string
    write: function(self: Process, data: string)
    close: function(self: Process)
    -- The exit code; nil when a signal ended it.
    wait: function(self: Process): integer
    -- Ends it, or sends `sig` ("term", "int", "hup", "kill"); its group's
    -- whole when spawned `group`. False when it had already exited.
    kill: function(self: Process, sig?: string): boolean
  end
  record process
    -- Runs a command to its end (waits).
    run: function(cmd: string, args?: {string}, opts?: ProcessOpts): ProcessResult
    spawn: function(cmd: string, args?: {string}, opts?: ProcessOpts): Process
    -- A signal to any process, or to the group it leads; `0` asks whether
    -- it is alive. Whether it arrived.
    kill: function(pid: integer, sig?: string | integer, group?: boolean): boolean
    -- This process's id.
    pid: integer
  end

  record RequestOpts
    url: string
    method: string
    headers: {string:string}
    json: any
    body: string
    timeout: number
  end
  record Response
    status: integer
    ok: boolean
    headers: {string:string}
    body: string
    json: any
  end
  record Request
    method: string
    path: string
    query: {string:string}
    headers: {string:string}
    body: string
    json: any
    params: {string:string}
  end
  type Handler = function(req: Request): any
  record Server
    port: integer
    get: function(self: Server, path: string, handler: Handler): Server
    post: function(self: Server, path: string, handler: Handler): Server
    put: function(self: Server, path: string, handler: Handler): Server
    patch: function(self: Server, path: string, handler: Handler): Server
    delete: function(self: Server, path: string, handler: Handler): Server
    route: function(self: Server, method: string, path: string, handler: Handler): Server
    stop: function(self: Server)
  end
  record ServeOpts
    port: integer
    host: string
    static: string
  end
  record http
    -- Each waits.
    request: function(target: string | RequestOpts, opts?: RequestOpts): Response
    get: function(url: string, opts?: RequestOpts): Response
    post: function(url: string, opts?: RequestOpts): Response
    put: function(url: string, opts?: RequestOpts): Response
    patch: function(url: string, opts?: RequestOpts): Response
    delete: function(url: string, opts?: RequestOpts): Response
    head: function(url: string, opts?: RequestOpts): Response
    serve: function(opts?: ServeOpts): Server
  end

  record ExecResult
    changes: integer
    last_id: integer
  end
  record Db
    -- Each waits.
    query: function(self: Db, sql: string, params?: any): {{string:any}}
    first: function(self: Db, sql: string, params?: any): {string:any}
    exec: function(self: Db, sql: string, params?: any): ExecResult
    transaction: function(self: Db, fn: function(db: Db))
  end
  record sqlite
    open: function(path?: string): Db
  end

  record Worker
    id: integer
    path: string
    running: boolean
    dispatch: function(self: Worker, msg: any)
    stop: function(self: Worker)
  end
  worker: function(path: string, handler?: function(msg: any, w: Worker)): Worker
