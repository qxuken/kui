---
status: accepted
date: 2026-10-25
---

# The `kui` runner: a complete kit to make an app in Lua

> **Accepted (2026-10-25); backlog C61.** Kit's M4. Kit (ADR 0066) makes
> a tool's window cheap to write; this makes the whole tool cheap to
> write, with nothing to install beside one binary.

A new `kui` command runs a Lua or Teal app with no other runtime: `kui run
app.lua`. It brings Kit and a small Rust-backed standard library — data
formats, files, processes, timers, an HTTP client and server, SQLite and
workers — whose I/O runs as tasks that never block a frame.

## Context

- kui has no standalone way to run a `.lua` app: a script is an extension
  a Rust, Node, C or Odin host loads (ADR 0059).
- The windowed runner and the headless one (ADR 0057) exist, and a
  `Waker` rings either loop from any thread.
- `packages/lua/kui.d.tl` types the Lua DSL for Teal (F194); Teal's
  compiler is one MIT Lua file kui-lua already vendors for its tests.
- ADR 0013 made effects data. Plain Lua has no sockets, processes or
  async, so a tool written in it needs these from its host.
- A channel (ADR 0065) carries bulk data across threads by handle.

## Decision

1. **A `kui` binary** (crate `crates/kui-cli`): `kui run [path]`, `kui
   check [path]` and `kui new <dir>`; `kui pack` is reserved. `path` is a
   `.lua` or `.tl` file, or a folder holding `kui.config.lua` (or `.tl`),
   else `main.lua`, `main.tl`, `app.lua`; the default is the current
   folder. Kit is built in: its extension is loaded under `kit`, and
   `require("kit.ui")` finds its host module.
2. **The app is a kui-lua script, with an update.** It defines `view(env)`
   as any script does. Every event its nodes raise and every message
   dispatched reaches `update(msg)` when it defines one (`on_event(ev)`
   still takes events where it is defined instead). State lives in globals
   the app owns; `init()`, when defined, runs once and its result is the
   global `model`, which a reload keeps.
3. **Modules under `kui.*`**, each Rust-backed: `json`, `toml`, `yaml`
   (`decode` / `encode` with plain tables), `env` (the process
   environment, after `.env` and `.env.local` from the app's folder,
   which never overwrite a variable already set), `fs`, `process`,
   `timer`, `http` (client and server), `sqlite` (rusqlite, SQLite
   bundled), `worker`, and `exit`.
4. **Tasks are the async primitive.** `kui.spawn(fn, ...)` runs `fn` as a
   coroutine in the app's own state. A call that waits — `kui.sleep`, an
   HTTP request, a process's output, a query — yields, and the runner
   resumes the task between frames when the Rust side has the answer,
   woken through the loop's `Waker`. Code reads top to bottom (`local r =
   kui.http.get(url)`) and the window keeps drawing. Outside a task the
   same call blocks until it is answered, so a script's top level reads a
   config file or opens a database the plain way; inside one it never
   blocks the frame. Local, fast calls (`kui.fs`, the formats) never
   yield.
5. **`kui.dispatch(msg)` reaches `update`**, before the next frame. A task
   may read the app's state, and a model change belongs in `update`, so
   the one place state moves stays one place.
6. **The server uses handlers.** `local s = kui.http.serve { port = 3050,
   static = "public" }`, then `s:get("/api/state", function(req) ...
   return { status = 200, json = {...} } end)`. Each request runs as a
   task, so a handler can wait on a query or a request of its own.
7. **Workers are Lua states on threads.** `local w = kui.worker("jobs/parse.lua")`
   runs a module with every module but the UI. One verb both ways:
   `w:dispatch(msg)` reaches the worker's `on_message`, and inside it
   `kui.dispatch(msg)` reaches the app's `update` (or the handler the app
   gave). Messages are plain values, copied; bulk data goes as channel
   handles inside them, which cross without copying. A worker that throws
   arrives as a message; `w:stop()` ends it.
8. **Teal is built in and optional.** `require` finds `.tl` modules and
   compiles them as they load; an entry may be `main.tl`. `kui check`
   type-checks the app against `kui.d.tl` and Kit's declarations. Plain
   `.lua` always works.
9. **Run modes.** `--headless` runs on the headless runner, for a server
   or a test with no display. `--watch` re-runs the app when its files
   change, keeping `model`; a load that fails keeps the old code and shows
   the error over the window until the next save fixes it.
10. **The manifest is Lua, and optional.** `kui.config.lua` returns `{
    entry, name, size = { w, h }, watch = { include, exclude } }`; `pack`
    is reserved. It is not `kui.lua`, which is what `require "kui"`
    finds and lua-language-server's meta file is called.

## Considered options

- **Node as the app host.** It needs Node installed, and splits Kit's
  showcase from its own runtime.
- **Requests as messages to `update`.** Handlers read better, and
  `kui.dispatch` keeps the model's change in one place anyway.
- **A separate Lua state per server.** Parallel, but it shares nothing:
  every read of the app's state becomes a round-trip. Workers are that,
  on purpose.
- **Every waiting call an error outside a task.** Honest about where a
  frame could stall, but a script's top level could not read its own
  config; blocking there is what a script expects and costs no frame,
  since the window is not open yet.
- **TOML for the manifest.** The app is already written in a language.

## Consequences

- A new crate and binary; its tests run apps headless.
- The standard library is full trust — files, processes, network — as in
  Node. Sandboxing is out of scope.
- Bundled fonts (IBM Plex) and `kui pack` come later; Kit falls back to
  the stock faces when Plex is not installed.
- Two commands beyond the draft, for the editor and for pictures: `kui
  types` writes lua-language-server's and Teal's declarations (kui's DSL,
  Kit's module, the runner's `kui.*`) with a `.luarc.json`, and `kui new`
  runs it; `kui run --shot out.png` draws headless and writes a frame.
- What the runner found in the layers under it, fixed beside it: a Lua
  host's children built inside another script's component were taken for
  that script's own (backlog C73); an app could not paint with an add-on's
  palette, so tokens can be exported (C74, `Tokens::exported`, Lua's
  `export = true`); and Kit's chart and waterfall now declare `on_layout`,
  which is what keeps a rect for `layout_of`.
- What a real app's port found (Cookie Mapping, rebuilt on the runner in a
  repository of its own with the Node app's features, backlog C79–C80):
  the manifest names the window (`min_size`, `chrome`, `titlebar`,
  `icon`); an app reads its arguments after `--` and the folder it was
  started in (`kui.args`, `kui.cwd`, `kui.platform`, `kui.cpus`); a
  spawned process can lead a process group, takes a signal, and ends with
  the app unless `detach`; `kui.next_frame(fn(env))` gives `update` what
  only a frame has (the clipboard, focus); `on_exit()` runs on every way
  out, Ctrl-C included; `kui.fs.create` writes only what is absent. And
  under it, in kui-lua: a child left out with `nil` in the middle of a
  list dropped every sibling after it, because Lua 5.5's `#t` stops at the
  first hole (C79).
- The same app's second round (backlog C83, C84): `kui.parse_args` (help
  and mistakes from a spec, ending the app before its window — the runner
  now opens no window for an app that ended while it loaded); `kui.log`
  and `print` kept for a devtools Log tab the runner draws with Kit's log;
  `kui.devtools(on | "log")`; `kui.hash`; `kui.fs.walk` and `read_all` as
  waiting calls; `kui.json.array`; `kui.process.command_line`; and on
  macOS a windowed run relaunches as an APFS clone named after the app, so
  the Dock does not say "kui". Under it, in the core: a fill that asked
  for its own node by label before declaring it replayed its first frame
  for good (C84).
