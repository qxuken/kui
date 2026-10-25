# kui — the runner

`kui` runs a Lua or Teal app with Kit and a standard library, and nothing
else to install ([ADR 0064](../../docs/adr/0064-the-kui-runner.md)).

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
| `--size WxH` | The window's size (1000×700, or the manifest's). |
| `kui check [path]` | Type-checks the app's `.tl` files against kui's, Kit's and the runner's declarations. |
| `kui new <dir>` | Starts an app, with its editor declarations. |
| `kui types [path]` | Writes `.kui/types` (lua-language-server's and Teal's declarations), `.luarc.json` and `tlconfig.lua`. |

## The standard library

A call marked *waits* yields inside a task (`kui.spawn`) and blocks
anywhere else, so a script's top level reads its config the plain way and
a task never stops a frame.

| | |
| --- | --- |
| `kui.spawn(fn, ...)` | Runs `fn` as a task. |
| `kui.dispatch(msg)` | To `update`, before the next frame (in a worker: to the app). |
| `kui.sleep(ms)` *waits*, `kui.timer.after/every(ms, fn)` | Time. |
| `kui.json`, `kui.toml`, `kui.yaml` | `decode(text)`, `encode(value)`. |
| `kui.env`, `kui.getenv` | The environment, after `.env` and `.env.local` beside the app (a variable already set wins). |
| `kui.fs` | `read`, `write`, `append`, `exists`, `list`, `lines`, `stat`, `mkdir`, `remove`, `rename`; paths are the app's folder's. |
| `kui.process.run(cmd, args, opts)` *waits* | `{ code, ok, stdout, stderr }`; `spawn` streams lines. |
| `kui.http.get/post/...(url, opts)` *waits* | `{ status, ok, headers, body, json }`. |
| `kui.http.serve{ port, static }` | `s:get("/api/items/:id", function(req) ... end)`; each request is a task. |
| `kui.sqlite.open(path)` | `db:query`, `db:first`, `db:exec` *wait*; `db:transaction(fn)`. |
| `kui.worker(path, handler)` | A module on a thread of its own: `w:dispatch(msg)` reaches its `on_message`; what it dispatches reaches `handler` (or `update`). A channel in a message crosses without a copy. |
| `kui.exit(code)` | Ends the app. |

Kit is built in: `require("kit.ui")`. Its palette is readable from the app
too — `bg = "$kitPanel"` follows the skin.

```bash
kui run addons/kit/app
```

is the Kit app, every component live (from the repository root).
