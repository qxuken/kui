  -- The kui runner's standard library (ADR 0064), spliced into the
  -- `kui` record by `kui check`. A call marked "waits" yields inside a
  -- task (`kui.spawn`) and blocks anywhere else. A call that can fail on
  -- its input or on the world (a file, text read in, the network, a
  -- process, a database) has a `try_` form that returns `true` and what
  -- the call returns, or `false`, the error's message and its code: `local
  -- ok, text, code = kui.fs.try_read(path)`. The codes are one set
  -- (the runner's README lists them): "not_found", "exists", "denied",
  -- "not_empty", "invalid", "parse", "encoding", "integrity", "timeout",
  -- "dns", "refused", "tls", "in_use", "closed", "busy", "constraint",
  -- "sql", "full", "too_long", "unavailable", "failed". The plain call
  -- raises the message at its caller's line.

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
  -- The app's name (the manifest's, else its folder's).
  name: string

  -- An option `kui.parse_args` reads: `{ type = "integer", default =
  -- 3000, short = "p", help = "…" }`. The type is the default's when left
  -- out, and "string" without one.
  record ArgOption
    ["type"]: string
    default: any
    short: string
    help: string
    required: boolean
    -- Repeatable: a list of every value given.
    many: boolean
    choices: {string}
    -- An environment variable read when the line leaves it out.
    env: string
    -- The value's name in the help (`<path>`).
    value: string
  end
  record ArgPositional
    name: string
    ["type"]: string
    help: string
    required: boolean
    many: boolean
    default: any
    choices: {string}
  end
  record ArgSpec
    name: string
    about: string
    usage: string
    epilog: string
    options: {string:ArgOption}
    args: {ArgPositional}
    -- Keep arguments past the positionals under this name (true: "rest").
    rest: string | boolean
  end
  -- The command line (`kui.args`, or `argv`) read against `spec`: each
  -- option under its key, `--help` printed from the spec, and a mistake
  -- said with what to type instead (both end the app before its window).
  parse_args: function(spec: ArgSpec, argv?: {string}): {string:any}

  -- `kui.log(...)`, `kui.log.warn(...)`: printed, and kept for the
  -- devtools' Log tab, as `print` is.
  record Log
    metamethod __call: function(self: Log, ...: any)
    debug: function(...: any)
    info: function(...: any)
    warn: function(...: any)
    error: function(...: any)
  end
  log: Log
  -- The devtools panel on, off, or (with nothing) toggled.
  devtools: function(on?: boolean)

  -- What a window gets from the OS, sent into a headless run (`kui run
  -- --headless`), for a test that drives the app as a user does: through
  -- the core's key routing, the Tab ring and hit testing, rather than
  -- dispatching the message a handler would post. Each is handled before
  -- the next frame, in the order sent. A windowed run raises an error.
  record InputMods
    shift: boolean
    ctrl: boolean
    alt: boolean
    super: boolean
  end
  record input
    -- A key pressed and released, by the name `on_key` reports it
    -- under: "tab", "down", "a", "f2".
    key: function(name: string, mods?: InputMods)
    -- The pointer to a point of the window, nothing pressed.
    move: function(x: number, y: number)
    -- The pointer there, the primary button pressed and released.
    click: function(x: number, y: number)
    -- Typed text, as the OS delivers it to the focused editor.
    text: function(s: string)
  end

  -- Digests: lowercase hex unless `enc` names "base64", "base64url" or "raw".
  record hash
    sha1: function(data: string, enc?: string): string
    sha256: function(data: string, enc?: string): string
    sha384: function(data: string, enc?: string): string
    sha512: function(data: string, enc?: string): string
    -- HMAC of `data` under `key`: "sha1", "sha256", "sha384", "sha512".
    hmac: function(alg: string, key: string, data: string, enc?: string): string
    -- 64-bit FNV-1a: a quick name, not a secret.
    fnv: function(data: string): string
  end
  record Base64Opts
    url: boolean
    pad: boolean
  end
  record base64
    encode: function(data: string, opts?: Base64Opts): string
    decode: function(text: string): string
    try_decode: function(text: string): boolean, string, string
  end
  record hex
    encode: function(data: string): string
    decode: function(text: string): string
    try_decode: function(text: string): boolean, string, string
  end
  record SealOpts
    -- "aes-256-gcm" (the default) or "chacha20-poly1305".
    alg: string
    aad: string
  end
  record Pbkdf2Opts
    iterations: integer
    length: integer
    hash: string
  end
  record Argon2Opts
    memory: integer
    iterations: integer
    parallelism: integer
    length: integer
  end
  record crypto
    random: function(n: integer): string
    equal: function(a: string, b: string): boolean
    encrypt: function(key: string, data: string, opts?: SealOpts): string
    try_encrypt: function(key: string, data: string, opts?: SealOpts): boolean, string, string
    decrypt: function(key: string, data: string, opts?: SealOpts): string
    try_decrypt: function(key: string, data: string, opts?: SealOpts): boolean, string, string
    -- Keys stretched from passwords, and password hashes: each waits.
    pbkdf2: function(password: string, salt: string, opts?: Pbkdf2Opts): string
    try_pbkdf2: function(password: string, salt: string, opts?: Pbkdf2Opts): boolean, string, string
    argon2: function(password: string, salt: string, opts?: Argon2Opts): string
    try_argon2: function(password: string, salt: string, opts?: Argon2Opts): boolean, string, string
    hash_password: function(password: string, opts?: Argon2Opts): string
    try_hash_password: function(password: string, opts?: Argon2Opts): boolean, string, string
    verify_password: function(password: string, hash: string): boolean
    try_verify_password: function(password: string, hash: string): boolean, boolean | string, string
  end
  record SecretOpts
    service: string
  end
  -- The OS's secret store, under the app's id unless `service` says.
  -- Every call waits.
  record secrets
    -- "keychain", "credential-manager", "secret-service" or "memory".
    store: string
    get: function(name: string, opts?: SecretOpts): string
    try_get: function(name: string, opts?: SecretOpts): boolean, string, string
    set: function(name: string, value: string, opts?: SecretOpts)
    try_set: function(name: string, value: string, opts?: SecretOpts): boolean, string, string
    delete: function(name: string, opts?: SecretOpts): boolean
    try_delete: function(name: string, opts?: SecretOpts): boolean, boolean | string, string
    -- true, or false, why and its code ("denied", "unavailable", …);
    -- never raises.
    probe: function(opts?: SecretOpts): boolean, string, string
  end
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
  -- The app's id, the manifest's (reverse DNS), else its folder's name.
  id: string
  -- The app's own version, the manifest's `version`.
  app_version: string
  -- The app's own folders for data and caches, the same packed and not.
  data_dir: string
  cache_dir: string
  -- What `kui pack` wrote, in a packed app; nil under `kui run`.
  record Packed
    runner: string
    bytecode: boolean
    hash: string
  end
  packed: Packed
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
    try_decode: function(text: string): boolean, any, string
    try_encode: function(value: any, opts?: {string:boolean}): boolean, string, string
    -- `t` (or a new table) marked a list: empty, it encodes as `[]`.
    array: function<T>(t?: {T}): {T}
  end
  record toml
    decode: function(text: string): {string:any}
    encode: function(value: {string:any}): string
    try_decode: function(text: string): boolean, {string:any} | string, string
    try_encode: function(value: {string:any}): boolean, string, string
  end
  record yaml
    decode: function(text: string): any
    encode: function(value: any): string
    try_decode: function(text: string): boolean, any, string
    try_encode: function(value: any): boolean, string, string
  end

  record WalkOpts
    skip: {string}
    match: string
    dirs: boolean
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
    -- The files under `dir` (waits), relative to it: `{ skip = { ".git",
    -- "node_modules" }, match = "**/src/*.ts", dirs = false }`.
    walk: function(dir?: string, opts?: WalkOpts): {string}
    -- Reads `paths` (waits): their texts in order, false for one that
    -- could not be read.
    read_all: function(paths: {string}): {string | boolean}
    mkdir: function(path: string)
    remove: function(path: string)
    rename: function(from: string, to: string)
    try_read: function(path: string): boolean, string, string
    try_write: function(path: string, data: string): boolean, string, string
    try_append: function(path: string, data: string): boolean, string, string
    try_list: function(path?: string): boolean, {string} | string, string
    try_lines: function(path: string): boolean, {string} | string, string
    try_create: function(path: string, data: string): boolean, boolean | string, string
    try_walk: function(dir?: string, opts?: WalkOpts): boolean, {string} | string, string
    try_mkdir: function(path: string): boolean, string, string
    try_remove: function(path: string): boolean, string, string
    try_rename: function(from: string, to: string): boolean, string, string
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
    try_read_line: function(self: Process, stream?: string): boolean, string, string
    try_write: function(self: Process, data: string): boolean, string, string
    try_wait: function(self: Process): boolean, integer | string, string
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
    -- The command `pid` was started with (waits), nil when none.
    command_line: function(pid: integer): string
    try_run: function(cmd: string, args?: {string}, opts?: ProcessOpts): boolean, ProcessResult | string, string
    try_spawn: function(cmd: string, args?: {string}, opts?: ProcessOpts): boolean, Process | string, string
  end

  record RequestOpts
    url: string
    method: string
    headers: {string:string}
    json: any
    body: string
    -- How long the whole request may take, in milliseconds; 30,000 (30 s) when left out.
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
    -- A refused connection or a timeout is `false`; a 404 is a Response.
    try_request: function(target: string | RequestOpts, opts?: RequestOpts): boolean, Response | string, string
    try_get: function(url: string, opts?: RequestOpts): boolean, Response | string, string
    try_post: function(url: string, opts?: RequestOpts): boolean, Response | string, string
    try_put: function(url: string, opts?: RequestOpts): boolean, Response | string, string
    try_patch: function(url: string, opts?: RequestOpts): boolean, Response | string, string
    try_delete: function(url: string, opts?: RequestOpts): boolean, Response | string, string
    try_head: function(url: string, opts?: RequestOpts): boolean, Response | string, string
    try_serve: function(opts?: ServeOpts): boolean, Server | string, string
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
    try_query: function(self: Db, sql: string, params?: any): boolean, {{string:any}} | string, string
    try_first: function(self: Db, sql: string, params?: any): boolean, {string:any} | string, string
    try_exec: function(self: Db, sql: string, params?: any): boolean, ExecResult | string, string
    try_transaction: function(self: Db, fn: function(db: Db)): boolean, string, string
  end
  record sqlite
    open: function(path?: string): Db
    try_open: function(path?: string): boolean, Db | string, string
  end

  -- A recording (ADR 0075): Rust reads and analyses it, a script draws
  -- its channels — `spans` in Kit's waterfall's shape, `frames` a row a
  -- finished frame, `instants` the waterfall's marks — and asks about one
  -- frame, one span or a range. Times are ms on the recording's clock;
  -- rows are 0-based.
  record TraceOpenOpts
    -- Watch the file and read what is added (on by default).
    watch: boolean
    -- What `update` is handed when the watched file moves: then poll.
    message: any
  end
  record TraceFilter
    -- Span origins kept: "kui", "host", an extension's namespace.
    origins: {string}
    -- A span name's part, any case.
    name: string
    -- Span names hidden whole: `{ "devtools" }`.
    hide_names: {string}
    -- Spans shorter than this many µs are hidden.
    min_us: number
    -- Frame verdicts kept: "late", "blocked", "slow", "waited",
    -- "wasted", "burn", "fine".
    verdicts: {string}
    -- Frames that drew what the one before drew are hidden.
    hide_unchanged: boolean
    -- A span's own test: handed one table per span (`name`, `origin`,
    -- `start`, `len`, `self`, `depth`, `track`, `frame`), kept when true.
    where: function(span: TraceSpanFields): boolean
  end
  record TraceSpanFields
    name: string
    origin: string
    start: number
    len: number
    self: number
    depth: integer
    track: integer
    frame: integer
  end
  -- Why a frame lagged (`kind`), and what its kind says: `late` wait,
  -- slept; `blocked` busy; `slow` work, phase, phase_ms, span, span_ms;
  -- `waited` wall, cpu; `burn` k of `of`; `fine` share of the budget.
  record TraceVerdict
    kind: string
    wait: number
    slept: boolean
    busy: number
    work: number
    phase: string
    phase_ms: number
    span: string
    span_ms: number
    wall: number
    cpu: number
    k: integer
    of: integer
    share: number
  end
  record TraceLeg
    what: string
    ms: number
  end
  record TracePath
    input: string
    total: number
    legs: {TraceLeg}
  end
  record TraceHolder
    what: string
    name: string
    node: string
  end
  record TraceFresh
    component: string
    answer: string
    why: string
    node: string
  end
  -- A frame opened: what the Perf tab's detail shows.
  record TraceFrame
    frame: integer
    index: integer
    begin: number
    finish: number
    gap: number
    work: number
    nodes: integer
    unchanged: boolean
    -- handler, build, devtools, layout, emit, render, "vsync wait".
    phases: {string:number}
    verdict: TraceVerdict
    causes: {string}
    holders: {TraceHolder}
    path: TracePath
    fresh: {TraceFresh}
    notes: {string}
    counters: {string:integer}
  end
  record TraceSpan
    name: string
    origin: string
    start: number
    len: number
    self: number
    cpu: number
    node: string
    cut: boolean
    track: string
    depth: integer
    frame: integer
    -- A component's fill: the component (`name` too), whether it
    -- replayed or drew fresh and why, and the span it was recorded as
    -- (`fill kit`).
    component: string
    answer: string
    why: string
    span: string
  end
  record Recording
    spans: Channel
    frames: Channel
    instants: Channel
    -- The watched file moved since the last poll.
    moved: boolean
    -- The file will grow no more.
    ended: boolean
    -- The frame budget, ms.
    budget: number
    filtered: boolean
    -- Why a `where` stopped keeping what a poll brought, if it threw.
    filter_error: string
    -- Reads what the file has: "more" and how many records, "idle",
    -- "ended", or "reset" for a file written again from its start.
    poll: function(self: Recording): string, integer
    session: function(self: Recording): {string:any}
    -- A row of `frames`: its verdict, phases, input path, holders,
    -- components drawn fresh, notes and counters.
    frame: function(self: Recording, row: integer): TraceFrame
    -- A row of `spans`.
    span: function(self: Recording, row: integer): TraceSpan
    -- The tables over frames `first..last` (the timeline's), or all.
    stats: function(self: Recording, first?: integer, last?: integer): {string:any}
    -- A counter's per-frame totals or a gauge's readings: `at`, `value`.
    series: function(self: Recording, name: string): Channel
    series_names: function(self: Recording): {string}
    -- Shows what the filter keeps (nothing: everything): the rows of
    -- `spans` and `frames` now.
    filter: function(self: Recording, f?: TraceFilter): integer, integer
  end
  record trace
    open: function(path: string, opts?: TraceOpenOpts): Recording
    read: function(bytes: string): Recording
    try_open: function(path: string, opts?: TraceOpenOpts): boolean, Recording | string, string
    try_read: function(bytes: string): boolean, Recording | string, string
  end

  record Worker
    id: integer
    path: string
    running: boolean
    dispatch: function(self: Worker, msg: any)
    stop: function(self: Worker)
    try_dispatch: function(self: Worker, msg: any): boolean, string, string
  end
  worker: function(path: string, handler?: function(msg: any, w: Worker)): Worker
  try_worker: function(path: string, handler?: function(msg: any, w: Worker)): boolean, Worker | string, string
