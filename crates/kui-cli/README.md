# kui — the runner

`kui` runs a Lua or Teal app with Kit and a standard library, and nothing
else to install ([ADR 0064](../../docs/adr/0064-the-kui-runner.md)).

Install it from a release — `install.sh` (macOS, Linux) and `install.ps1`
(Windows) are assets of each, beside the runners and their `SHA256SUMS`
— or from npm, and `kui upgrade` keeps it current:

```bash
curl -fsSL https://github.com/qxuken/kui/releases/download/v0.1.0-alpha.68/install.sh | sh
```

```bash
npm i -g @qxuken/kui-cli
```

```bash
kui new hello
```

```bash
kui run --watch hello
```

An app is a kui-lua script: `view(env)` returns its tree, `update(msg)`
hears every event its nodes raise and every `kui.dispatch`, and `init()`,
when it is defined, makes the `model` a `--watch` reload keeps.

```lua
local kit = require("kit.ui")

function init() return { count = 0, joke = nil } end

function update(msg)
  if msg.kind == "add" then
    model.count = model.count + 1
  elseif msg.kind == "fetch" then
    kui.spawn(function()                       -- a task: the window keeps drawing
      local r = kui.http.get("https://icanhazdadjoke.com/", { headers = { accept = "text/plain" } })
      kui.dispatch({ kind = "joke", text = r.body })
    end)
  elseif msg.kind == "joke" then
    model.joke = msg.text
  end
end

function view(env)
  return column { width = "grow",
    kit.toolbar({}, {
      kit.button({ label = "Add one", variant = "primary", on_press = { kind = "add" } }),
      kit.button({ label = "Fetch a joke", on_press = { kind = "fetch" } }),
    }),
    kit.stat({ label = "Count", value = tostring(model.count) }),
  }
end
```

## Commands

| Command | What |
| --- | --- |
| `kui run [path]` | Runs the app at `path`: a `.lua` or `.tl` file, or a folder (its `kui.config.lua` names the entry, else `main.lua`, `main.tl`, `app.lua`). `kui <path>` is the same. |
| `kui play` | Opens Kit's gallery, carried inside the runner: every component live, its source and guide, the design system and Theme panel. No repository checkout is needed; the run options apply. |
| `--watch` | Reloads when the app's files change, keeping `model`; a load that fails keeps the old code and shows the error over the window. |
| `--headless` | No window: a server, a script, a test. |
| `--shot out.png [--after ms]` | No window: writes a frame after `ms` (800) and ends. |
| `--shot out.gif --record ms` | No window: records `ms` of frames from `--after` on into an animation (`.gif`, or `.png` for an animated PNG) and ends: a switch's slide, a toast's exit, to look at frame by frame. |
| `--size WxH` | The window's size (1000×700, or the manifest's). |
| `--scale S` | With `--headless` or `--shot`, the display's scale (1): `1.5` draws as a 150% display does, fractional pixels and all. A window takes its monitor's. |
| `KUI_PAINT_AUDIT=1` | In the environment: every frame checks that nothing a node draws leaves the box that clips or scrolls it (ADR 0074), a `paint-escape` warning per node that does; a headless run prints the counts at its end. A test aid. |
| `--appearance light\|dark` | Pins the appearance over the OS's, so a token with a light and a dark value takes that one; headless has no OS to ask, so a `--shot` in light needs it. |
| `--platform macos\|windows\|linux` | With `--headless` or `--shot`, draws as that platform does rather than this machine's: a menu's shortcuts (`⌥⌘8` or `Ctrl+Alt+8`), the modifier a shortcut and an editing key mean (⌘ or Ctrl), `env.platform` and the `platform` conditions, and under custom chrome its titlebar — on `macos` the traffic lights' room at macOS 27's place for the manifest's `titlebar`. So a README's pictures match whichever machine takes them. |
| `-- args…` | What follows is the app's own, in `kui.args`: `kui run . -- --port 3001`. |
| `--devtools` | Opens with the devtools panel; its Log tab has what the app printed and logged. |
| `--no-rename` | Keeps the name "kui" in the Dock and ⌘Tab (macOS names an app by its binary, so a windowed run relaunches as a free clone named after the app). |
| `kui perf [recording]` | Opens a `.kuitrace` in kui-perf, the trace inspector (ADR 0075), carried inside the runner; the run options apply. |
| `kui check [path]` | Type-checks the app's `.tl` files against kui's, Kit's and the runner's declarations. |
| `kui new <dir>` | Starts an app, with its editor declarations. |
| `kui types [path]` | Writes `.kui/types` (lua-language-server's and Teal's declarations), `.luarc.json` and `tlconfig.lua`. |
| `kui pack [path]` | Builds the app for any platform from any machine (ADR 0068): `--target linux-x64,darwin-arm64,win32-x64,linux-arm64` (or `all`; default this machine's), `--format exe\|app\|bin\|npm\|kui` (default each target's own), `--out dist`. See [Shipping an app](#shipping-an-app). |
| `kui upgrade [version]` | Replaces this `kui` with the release's (the latest by default); one npm or Homebrew installed says the command to use instead. |
| `kui runners fetch <version> --to <dir>` | Downloads every platform's runner with its checksums, laid out as the release is, for a mirror (`latest` also writes `latest.txt`). |

## The standard library

A call marked *waits* yields inside a task (`kui.spawn`) and blocks
anywhere else, so a script's top level reads its config the plain way and
a task never stops a frame.

A call that can fail on its input or on the world — a file, text read
in, the network, a process, a database, a worker's script — has a `try_`
form that returns `true` and what the call returns, or `false`, the
error's message and a code to match on: `local ok, text, code =
kui.fs.try_read(path)`, `db:try_query(sql)`. The plain call is made from it and raises the
message at the line that called it, with no traceback. Only an argument
of the wrong type raises from a `try_` form. A `try_` form waits as its
call does; a decoded `null` is `true, nil`, not a failure; an HTTP 404
is a response, a refused connection is `false`. The `try_` forms:
`decode`/`encode` of `kui.json`, `kui.toml` and `kui.yaml`; `kui.fs`'s
`read`, `write`, `append`, `create`, `list`, `lines`, `walk`, `mkdir`,
`remove`, `rename`; `kui.process.run` and `spawn`, and a process's
`read_line`, `write`, `wait`; every `kui.http` request and `serve`;
`kui.sqlite.open` and a database's `query`, `first`, `exec`,
`transaction`; `kui.trace.open` and `read`; `kui.worker` and a worker's
`dispatch`; `decode` of `kui.base64` and `kui.hex`; `kui.crypto`'s
`encrypt`, `decrypt`, `pbkdf2`, `argon2`, `hash_password`,
`verify_password`;
`kui.secrets`' `get`, `set`, `delete`.

The codes are one set across the runner, named alike on every OS where
the messages are each OS's own:

| Code | When |
| --- | --- |
| `not_found` | No such file or folder (`kui.fs`, `kui.worker`), no such command (`kui.process`). |
| `exists` | It is there already: `kui.fs.mkdir` where a file is. |
| `denied` | Not allowed: permissions, a packed app's own files, a read-only database, a refused secrets prompt. |
| `not_empty` | `kui.fs.rename` onto a folder that holds something. |
| `invalid` | The input does not fit: a URL, a method, a key's length, a cipher's or a cost's name, TOML with no table at the top, a parameter a statement does not take. |
| `parse` | Text that does not read as its format: `json`, `toml`, `yaml`, `base64`, `hex` `decode`. |
| `encoding` | Bytes that are not the text expected: a file read as lines, a secret. |
| `integrity` | `kui.crypto.decrypt`: the wrong key or `aad`, or altered data. |
| `timeout` | A request past its `timeout`; an OS's own time-out. |
| `dns` | A request's host has no address. |
| `refused` | Nothing answered: refused, reset, unreachable. |
| `tls` | A request's secure connection failed. |
| `in_use` | `kui.http.serve` on a port that is taken. |
| `closed` | A process's stdin, a stopped worker, a closed database or HTTP client. |
| `busy` | The database is locked by another connection. |
| `constraint` | A database constraint: unique, not null, a foreign key, a check. |
| `sql` | The statement is wrong: its syntax, a table or column that is not there. |
| `full` | The disk or a quota is full. |
| `too_long` | Past a limit: a response's body, a secret, a database's value. |
| `unavailable` | The service is not there: no secret store, a database file that will not open. |
| `failed` | Anything else, and an error the app's own function raised in a `try_transaction`. |

A `try_transaction` whose function raised a database's error (a plain
`db:exec` that broke a constraint) answers that error's code.

| | |
| --- | --- |
| `kui.spawn(fn, ...)` | Runs `fn` as a task. |
| `kui.dispatch(msg)` | To `update`, before the next frame (in a worker: to the app). |
| `kui.sleep(ms)` *waits*, `kui.timer.after/every(ms, fn)` | Time. |
| `kui.json`, `kui.toml`, `kui.yaml` | `decode(text)`, `encode(value)`. |
| `kui.env`, `kui.getenv` | The environment, after `.env` and `.env.local` beside the app (a variable already set wins). |
| `kui.args`, `kui.cwd`, `kui.platform`, `kui.cpus`, `kui.name` | The app's arguments (after `--`), the folder `kui` started in, `"macos"`/`"windows"`/`"linux"`, the cores to share work between, the app's name. |
| `kui.parse_args(spec)` | The arguments read against `{ about, options = { port = { type = "integer", default = 3000, short = "p", help = … } }, args = { { name = "files", many = true } } }`: `--help` written from it, a mistake said with what to type instead (`--prot` → did you mean `--port`?), both ending the app before its window. Options also take `env`, `choices`, `required`, `many`. |
| `kui.log(...)`, `kui.log.warn/error/debug(...)`, `print` | Printed, and kept for the devtools' Log tab, with the runner's errors and every worker's lines. |
| `kui.devtools(on)` | The panel on, off, or toggled; `kui.devtools("log")` opens it at the Log tab. |
| `kui.hash.sha1/sha256/sha384/sha512(data, enc)` | Digests, as hex unless `enc` says `"base64"`, `"base64url"` or `"raw"`; `kui.hash.hmac("sha256", key, data, enc)` signs; `fnv(data)` is a quick name, not a secret. |
| `kui.base64`, `kui.hex` | `encode(bytes)` and `decode(text)`. `kui.base64.encode(s, { url = true })` is a JWT's alphabet, unpadded; `decode` reads either alphabet, padded or not, past line breaks. |
| `kui.crypto` | `random(n)` bytes; `equal(a, b)` in constant time (a MAC's check); `encrypt(key, data, { alg, aad })` and `decrypt` with a 32-byte key, AES-256-GCM or `alg = "chacha20-poly1305"`, the nonce kept in front of what it returns. Keys from passwords *wait*: `pbkdf2(password, salt, { iterations = 600000 })`, `argon2(password, salt, { memory, iterations, parallelism })` (Argon2id); `hash_password(pw)` makes a PHC string to keep and `verify_password(pw, hash)` checks one. |
| `kui.secrets` | The OS's own store — the Keychain, the Credential Manager, the Secret Service — under the app's `id` (or `{ service = … }`): `get(name)` (nil when absent), `set(name, text)`, `delete(name)`, each *waits*. `probe()` answers `true`, or `false`, why and the code — `denied` (a Keychain prompt refused, a locked collection), `unavailable` (no Secret Service on the bus) — never raising. Nothing falls back to a file: what to do without a store is the app's choice. `kui.secrets.store` names the store; `KUI_SECRETS=memory` keeps secrets in the process, for a test. |
| `kui.fs` | `read`, `write`, `append`, `create` (only if absent: a lock), `exists`, `list`, `lines`, `stat`, `mkdir`, `remove`, `rename` — at once; paths are the app's folder's. `walk(dir, { match, skip, dirs })` and `read_all(paths)` *wait*: a tree or a batch of files in one hop off the frame. |
| `kui.json.array(t)` | Marks a list, so that empty it encodes as `[]`; a decoded empty list comes back marked. |
| `kui.process.run(cmd, args, opts)` *waits* | `{ code, ok, stdout, stderr }`. |
| `kui.process.spawn(cmd, args, opts)` | `p:read_line(stream)` *waits*, `p:lines(stream)`, `p:write`, `p:wait()` *waits*, `p:kill(sig)`. `group = true` makes it lead a process group, and its `kill` reaches all it started; it ends with the app unless `detach = true`. |
| `kui.process.kill(pid, sig, group)` | A signal to any process (`0` asks whether it is alive); `kui.process.pid` is the app's. |
| `kui.process.command_line(pid)` *waits* | The command a process was started with, nil when none: what tells a stale pid that is still yours from a reused one, on Windows too. |
| `kui.http.client(opts)` | An independent pooled client with colon request methods and composed middleware. |
| `kui.http.configure(opts)` | Replaces process-wide client defaults (`base_url`, `headers`, `timeout`, `ca_file`, `pins` or `pin_file`); no options resets them. |
| `kui.http.get/post/...(url, opts)` *waits* | `{ status, ok, headers, body, json }`; `opts` takes `headers`, `json` or `body`, and `timeout` in milliseconds (30,000 by default) for the whole request; `pins = { "sha256/<base64 SPKI digest>", ... }` restricts HTTPS to the listed server public keys; `pin_file` reads a certificate from a path instead; `ca_file` adds trusted CAs from a PEM bundle (see below). |
| `kui.http.serve{ port, static }` | `s:get("/api/items/:id", function(req) ... end)`; each request is a task. |
| `kui.sqlite.open(path)` | `db:query`, `db:first`, `db:exec` *wait*; `db:transaction(fn)`. |
| `kui.trace.open(path, { message })` | A recording (`.kuitrace`, ADR 0075) as channels: `rec.spans` (Kit's waterfall's `bars`), `rec.frames`, `rec.instants` (its `marks`); `rec:frame(row)`, `rec:span(row)`, `rec:stats(first, last)`, `rec:series(name)`; `rec:filter{ origins, name, hide_names, min_us, verdicts, hide_unchanged, where }` (`where`, a function handed each span). The file is watched: as it grows, `update` gets `message`, and `rec:poll()` appends what came. |
| `kui.worker(path, handler)` | A module on a thread of its own: `w:dispatch(msg)` reaches its `on_message`; what it dispatches reaches `handler` (or `update`). A channel in a message crosses without a copy. |
| `kui.next_frame(fn)` | Runs `fn(env)` before the next `view`: `env.set_clipboard`, `env.set_focus`, `env.measure_text` from `update`, a task or a timer. |
| `kui.exit(code)` | Ends the app: the window closes, then `on_exit()` runs, then what it spawned is ended. Ctrl-C, `SIGTERM` and `SIGHUP` do the same. |

Set defaults for the default client, or create independent pooled clients:

```lua
kui.http.configure { timeout = 15000 }
local api = kui.http.client {
  base_url = "https://internal.example.com/v1/",
  ca_file = "certs/company-ca-bundle.pem",
  timeout = 15000,
  headers = { accept = "application/json" },
}
local response = api:get("/profile") -- https://internal.example.com/v1/profile
```

Clients have `request`, `get`, `post`, `put`, `patch`, `delete`, `head`,
`configure`, `compose`, `close`, their `try_` forms, and `destroy`, called
with `:`. `kui.http.try_client` returns `true, client` or `false, message,
code`. Each client owns a connection pool and starts with standard defaults
plus its supplied configuration. It does not inherit `kui.http.configure`
settings. `KUI_EXTRA_CA_CERTS` applies to every client.

`base_url` uses prefix joining. With `https://example.com/v1/`, both
`"/profile"` and `"profile"` produce `https://example.com/v1/profile`.
Trailing base slashes and leading request slashes are joined with one slash;
query strings on request paths are preserved. An empty path selects the
base prefix with a trailing slash. Absolute HTTP(S) request URLs override
the prefix and may target another origin; configured CA/pin checks still
apply. The base must be an absolute HTTP(S) URL without user info, query or
fragment. Scheme-relative `//host/path` requests are rejected. Relative
requests without a base return `invalid`.

Configuration replaces all defaults: `base_url`, `headers`, `timeout`,
`ca_file`, and either `pins` or `pin_file`. `api:configure()` or
`api:configure {}` resets that client's defaults; `kui.http.configure()`
resets the default client's. The default client's configuration and pool
are shared across workers and watch reloads. Independent clients retain
their own configuration. Runner release downloads use the environment's
trust settings independently.

Request options override configured values. Headers merge by
case-insensitive name; unrelated default headers remain. A request's
`pins` replaces a configured `pin_file`, and vice versa. Nil or omitted
options inherit defaults. Each request snapshots defaults when it starts.
Relative configured certificate paths resolve from the configuring app or
worker's folder; request-specific certificate paths resolve from the
calling app or worker's folder. Invalid configuration leaves previous
defaults intact. Timeouts are positive finite milliseconds and cover each
network attempt, including its response body.

Connections are reused within a client, with separate transport pools for
different effective CA contents, public-key pins and timeouts. Certificate
files are read off the UI thread for each network attempt; changing a file
selects a matching transport pool, so an existing connection cannot bypass
changed trust or pin requirements. Concurrent calls may open additional
connections. File errors surface when making the network request.

Compose middleware around a client's dispatch:

```lua
local logged = api:compose(function(next)
  return function(request)
    kui.log(request.method, request.url)
    local ok, response_or_error, code = next(request)
    if ok then kui.log(response_or_error.status) end
    return ok, response_or_error, code
  end
end)
local response = logged:get("/profile")
logged:close() -- also closes api's shared pool
```

A middleware takes a dispatch function and returns a dispatch function.
Dispatch accepts merged request options and returns `true, response` or
`false, message, code`. It can yield inside a task. Middleware can change
options, transform a response, recover from a failure, return a cached
response without calling `next`, or explicitly call `next` again for another
attempt. Every layer normalizes thrown errors into failed try-results for
the outer layer. Invalid dispatch return values produce `invalid`.

`api:compose(a, b)` and `api:compose { a, b }` return a new view: request
flow is `b -> a -> transport`, and response flow is `transport -> a -> b`.
The source view is unchanged. Chained composition wraps the existing
chain; it does not rebuild its middleware. `try_compose` reports constructor
or validation failures. `kui.http.compose` supports the same pattern for
the default client. URL prefix joining happens before middleware runs and
again before network dispatch if middleware changes the URL to a relative
path. Body serialization and TLS validation happen at network dispatch.
HTTP status codes, including 4xx/5xx, remain responses. Body-read failures,
truncated bodies, timeouts and response-limit errors propagate as failures
through middleware and the usual `try_`/raising calls.

Composed views share their source's configuration, pool and lifecycle.
Reconfiguring a view affects its siblings, while each request retains its
snapshot. Middleware functions belong to their Lua state; construct a view
inside a worker when needed. `configure()` keeps existing middleware.

`api:close()` rejects new calls with `closed`, waits for dispatched network
operations to finish, and closes idle connections. `api:destroy()` rejects
new calls and immediately shuts down open sockets; interrupted operations
return `closed`. DNS lookup or a TCP connect already underway remains
bounded by its timeout. Closing or destroying any composed view closes the
shared client. Repeated close/destroy is harmless; closed clients cannot
be reopened by `configure`. Create a fresh client instead. Unreferenced
independent clients release their pools after outstanding operations finish.

`kui.http` accepts optional certificate pinning on every request method:

```lua
local response = kui.http.get("https://api.example.com/profile", {
  pins = { "sha256/<current key digest>", "sha256/<backup key digest>" },
})
```

Replace the placeholders with standard padded Base64 encodings of the
SHA-256 digest of the leaf certificate's DER SubjectPublicKeyInfo (SPKI).
Obtain these from the server operator through a trusted channel. Several
pins allow a key rotation: any one may match. Renewing a certificate with
the same key keeps its pin.

Alternatively, pin the public key from a local PEM or DER certificate:

```lua
local response = kui.http.get("https://api.example.com/profile", {
  pin_file = "certs/api.pem",
})
```

`pin_file` accepts an absolute path or one relative to the app's folder.
It reads the file for each request, off the UI thread; PEM uses the first
`CERTIFICATE` block. Use either `pins` or `pin_file`. A missing file returns
`not_found`, an unreadable file its filesystem error code, and an invalid
certificate returns `invalid`. The file supplies the expected public key;
it does not add a trusted CA or bypass certificate validation.

Pins add to normal CA, hostname and expiry verification. They are checked
before sending HTTP headers or a body, on the same connection that carries
the request. Every HTTPS redirect destination must match a supplied pin;
HTTP URLs and redirects to HTTP are refused. A CONNECT proxy retains its
own normal TLS validation; the pins apply to the server inside the tunnel.
An empty or malformed pin list (including a digest other than 32 bytes) or
an initial HTTP URL returns `invalid`; a mismatch returns `tls` through
`try_get`, `try_post`, or `try_request` (and raises through the ordinary
calls). Leaving both options out of the request and client defaults uses normal HTTPS verification.

To trust a private or company CA, set a bundle for the runner:

```sh
KUI_EXTRA_CA_CERTS=/path/to/company-ca-bundle.pem kui run my-app
```

The file contains one or more PEM `CERTIFICATE` blocks. These CAs add to
the built-in public roots for `kui.http` and the runner's release/mirror
downloads, including TLS to HTTPS proxies. The path is captured at process
startup, relative to the launch directory when not absolute; setting it
in an app's `.env` has no effect. The bundle is loaded once, on the first
HTTP operation, and kept across requests, workers and watch reloads.
Restart the runner after changing it. An unset or empty variable keeps
the normal roots. An unreadable, empty or malformed bundle fails requests
with `tls` and a message naming `KUI_EXTRA_CA_CERTS`.

For additional CAs on one request, use `ca_file`:

```lua
local response = kui.http.get("https://internal.example.com", {
  ca_file = "certs/company-ca-bundle.pem",
  -- Optional public-key restriction, verified after normal TLS validation:
  pin_file = "certs/internal-server.pem",
})
```

`ca_file` accepts the same PEM bundle format. Relative paths are resolved
from the app's folder; absolute paths work too. It adds to both the
built-in and environment-provided roots for this request and its redirects,
including HTTPS proxies, without affecting other requests. The file is
read for each request, off the UI thread. Missing/unreadable files return
their filesystem error code; empty or malformed bundles return `invalid`.
CA bundles preserve hostname and expiry validation, and neither CA option
bypasses `pins` or `pin_file`.

A packed macOS app reads `PATH` and `SHELL` from the user's Nu, Zsh, Bash,
or PowerShell login/interactive configuration in the background. Its first
process waits if that read is still running; an explicit `opts.env` wins.

State a `--watch` reload must keep goes in `model` or a global: the app's
modules load afresh, so a module's locals start over while the processes,
servers and workers they held run on.

## The manifest

`kui.config.lua` beside the app (or `kui.config.tl`) returns a table:

| | |
| --- | --- |
| `name` | The app's name (the folder's when left out). |
| `entry` | The file to run. |
| `size`, `min_size` | `{ w = 900, h = 600 }`: the window's first and least size. |
| `chrome` | `"native"` (the default), `"custom"` (the app draws its titlebar), `"borderless"`. |
| `titlebar` | `"standard"`, `"medium"`, `"tall"`: the macOS strip under custom chrome. |
| `icon` | A PNG for the Dock, the taskbar and the window. |
| `id` | Reverse DNS, as `"dev.example.notes"`: names `kui.data_dir`, `kui.cache_dir` and the macOS bundle. `kui pack` needs one; `kui run` takes the folder's name. |
| `version` | The app's own version: `kui.app_version`, the bundle's, the file names `kui pack` writes. |
| `kui` | The runner the app was written against, as `"0.1.0-alpha.68"`: `kui run` warns when it is not this runner or one it lists as compatible; `kui pack` packs that runner. |
| `pack` | `{ exclude = { "fixtures/**" }, console = true, devtools = false, npm = { name = "@company/app" } }`: what `kui pack` leaves out, a Windows `exe` with a console, devtools off in the packed app, the npm package's name. |
| `devtools` | `true` opens with the devtools panel. Without it they are there, hidden: F12 brings them up at the Log tab, as does View › Toggle Developer Tools on macOS. |
| `devtools_key` | The chord that brings them up: F12 by default; another as a menu item spells one (`"mod+alt+i"`), or `false` to leave F12 to the app — the core's own Ctrl+Shift+I and the macOS menu still reach them. |
| `rename` | `false` keeps the name "kui" in the Dock (as `--no-rename`). |
| `watch` | `{ include = { "*.json" }, exclude = { "data/**" } }` beside the app's `.lua` and `.tl`. |

The app also reads who it is and where it may write: `kui.id`,
`kui.app_version`, `kui.data_dir` and `kui.cache_dir` (its own folders,
in each system's place, the same packed and not), and `kui.packed` (what
`kui pack` wrote, in a packed app). Lua bytecode loads only from a
packed app's own files; `load`, `loadfile`, `dofile` and `require` take
source everywhere else.

## Shipping an app

`kui pack` takes the target's prebuilt runner and adds the app to it — a
zip with the app's files under `app/` and a generated `kui.pack.lua` — so
one machine builds for every platform, with nothing to compile:

| Format | What |
| --- | --- |
| `exe` | Windows: one file, the archive appended (`unzip -l app.exe` lists it), GUI unless `pack.console`. |
| `app` | macOS: a zipped `.app`, the archive in `Resources/app.zip`, the icon as `.icns`; signed ad hoc when packed on a Mac. |
| `bin` | Linux: one file, the archive appended, and its icon, in a `.tar.gz`. |
| `npm` | A package of the app's sources with a `bin` and an exact dependency on `@qxuken/kui-cli`, for a registry of your own (`npm publish` it). |
| `kui` | The archive alone, which `kui run app.kui` runs. |

- Sources by default; `--bytecode` compiles them (`--strip` drops their
  debug information), tied to the runner it packs.
- `--installer https://host/app/` also writes `install.sh` and
  `install.ps1` that fetch the builds from there, check them against the
  `SHA256SUMS` beside them, and install into the user's own folders.
- Runners for other versions or platforms come from the GitHub release,
  cached; `--runner-mirror` (or `KUI_RUNNER_MIRROR`) names a mirror with
  the release's layout, which `kui runners fetch` fills, and `--runner`
  a file.
- A packed app gets every argument (`app --port 3001`); the runner's own
  flags come from `KUI_FLAGS` (`KUI_FLAGS=--devtools`). A packed GUI app
  logs to `kui.data_dir/logs/<name>.log` (the last few runs); `KUI_LOG`
  names another file, or `-` for none. Its own files are read-only.
- Signing is yours: `rcodesign` signs and notarizes a `.app` from Linux,
  `osslsigncode` an `.exe`.

Kit is built in: `require("kit.ui")`. Its palette is readable from the app
too — `bg = "$kitPanel"` follows the skin.

```bash
kui play
```

opens the Kit app, every component live. From the repository root,
`kui run --watch addons/kit/app` opens the same app for editing.
