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
| `--watch` | Reloads when the app's files change, keeping `model`; a load that fails keeps the old code and shows the error over the window. |
| `--headless` | No window: a server, a script, a test. |
| `--shot out.png [--after ms]` | No window: writes a frame after `ms` (800) and ends. |
| `--shot out.gif --record ms` | No window: records `ms` of frames from `--after` on into an animation (`.gif`, or `.png` for an animated PNG) and ends: a switch's slide, a toast's exit, to look at frame by frame. |
| `--size WxH` | The window's size (1000×700, or the manifest's). |
| `--scale S` | With `--headless` or `--shot`, the display's scale (1): `1.5` draws as a 150% display does, fractional pixels and all. A window takes its monitor's. |
| `KUI_PAINT_AUDIT=1` | In the environment: every frame checks that nothing a node draws leaves the box that clips or scrolls it (ADR 0074), a `paint-escape` warning per node that does; a headless run prints the counts at its end. A test aid. |
| `--appearance light\|dark` | Pins the appearance over the OS's, so a token with a light and a dark value takes that one; headless has no OS to ask, so a `--shot` in light needs it. |
| `-- args…` | What follows is the app's own, in `kui.args`: `kui run . -- --port 3001`. |
| `--devtools` | Opens with the devtools panel; its Log tab has what the app printed and logged. |
| `--no-rename` | Keeps the name "kui" in the Dock and ⌘Tab (macOS names an app by its binary, so a windowed run relaunches as a free clone named after the app). |
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
form that returns `true` and what the call returns, or `false` and the
error's message: `local ok, text = kui.fs.try_read(path)`,
`db:try_query(sql)`. The plain call is made from it and raises the
message at the line that called it, with no traceback. Only an argument
of the wrong type raises from a `try_` form. A `try_` form waits as its
call does; a decoded `null` is `true, nil`, not a failure; an HTTP 404
is a response, a refused connection is `false`. The `try_` forms:
`decode`/`encode` of `kui.json`, `kui.toml` and `kui.yaml`; `kui.fs`'s
`read`, `write`, `append`, `create`, `list`, `lines`, `walk`, `mkdir`,
`remove`, `rename`; `kui.process.run` and `spawn`, and a process's
`read_line`, `write`, `wait`; every `kui.http` request and `serve`;
`kui.sqlite.open` and a database's `query`, `first`, `exec`,
`transaction`; `kui.worker` and a worker's `dispatch`.

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
| `kui.hash.sha1/sha256/sha512/fnv(data)` | Digests as hex. |
| `kui.fs` | `read`, `write`, `append`, `create` (only if absent: a lock), `exists`, `list`, `lines`, `stat`, `mkdir`, `remove`, `rename` — at once; paths are the app's folder's. `walk(dir, { match, skip, dirs })` and `read_all(paths)` *wait*: a tree or a batch of files in one hop off the frame. |
| `kui.json.array(t)` | Marks a list, so that empty it encodes as `[]`; a decoded empty list comes back marked. |
| `kui.process.run(cmd, args, opts)` *waits* | `{ code, ok, stdout, stderr }`. |
| `kui.process.spawn(cmd, args, opts)` | `p:read_line(stream)` *waits*, `p:lines(stream)`, `p:write`, `p:wait()` *waits*, `p:kill(sig)`. `group = true` makes it lead a process group, and its `kill` reaches all it started; it ends with the app unless `detach = true`. |
| `kui.process.kill(pid, sig, group)` | A signal to any process (`0` asks whether it is alive); `kui.process.pid` is the app's. |
| `kui.process.command_line(pid)` *waits* | The command a process was started with, nil when none: what tells a stale pid that is still yours from a reused one, on Windows too. |
| `kui.http.get/post/...(url, opts)` *waits* | `{ status, ok, headers, body, json }`; `opts` takes `headers`, `json` or `body`, and `timeout` in milliseconds (30,000 by default) for the whole request. |
| `kui.http.serve{ port, static }` | `s:get("/api/items/:id", function(req) ... end)`; each request is a task. |
| `kui.sqlite.open(path)` | `db:query`, `db:first`, `db:exec` *wait*; `db:transaction(fn)`. |
| `kui.worker(path, handler)` | A module on a thread of its own: `w:dispatch(msg)` reaches its `on_message`; what it dispatches reaches `handler` (or `update`). A channel in a message crosses without a copy. |
| `kui.next_frame(fn)` | Runs `fn(env)` before the next `view`: `env.set_clipboard`, `env.set_focus`, `env.measure_text` from `update`, a task or a timer. |
| `kui.exit(code)` | Ends the app: the window closes, then `on_exit()` runs, then what it spawned is ended. Ctrl-C, `SIGTERM` and `SIGHUP` do the same. |

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
kui run addons/kit/app
```

is the Kit app, every component live (from the repository root).
