# Testing without a window

`kui test` runs Lua and Teal app tests with no window. Input still passes
through the real core and Kit: routing, focus, editing and layout are the
same paths the app normally uses.

## A runner app's tests

Put cases in `test/**/*.lua` or `test/**/*.tl`, or in `*_test.lua` /
`*_test.tl` beside the sources. Keep helper modules in `fixtures/` and
snapshot baselines in `__snapshots__/`. A Teal file wins over its generated
Lua sibling; `.d.tl` declarations are not tests.

```lua
local test = require('kui.test')

test.describe('counter', function()
  test.before_each(function()
    test.start { args = {} }
  end)

  test.it('increments through the actual button', function()
    test.press('Add one')
    test.expect(test.model().count):to_be(1)
    test.expect_tree('one-click')
    test.expect_shot('one-click', { tolerance = 2 })
  end)
end)
```

Each file runs in a fresh process. Each case starts with no running app;
`test.start` loads its manifest and entry after setting its arguments and
temporary data/cache directories. A case's app, workers and operations
are cleaned up afterward. Start again within a case to test reopening
its files with fresh app and framework state.

```sh
kui test
kui test --filter counter --watch
kui test --update
kui test --reporter junit --output test-results
```

Failures include the test path and traceback. The exit code is nonzero
when a case fails, a file crashes/times out, or no cases match. JUnit XML
is written to the output directory with each case's duration.

## Driving and assertions

`test.start` returns an app handle with the same driving operations:

- `press(label_or_key)` invokes a node's accessible action;
  `click(label_or_key)` clicks its centre, and `click(x, y)` uses coordinates.
- `type(label, text)` focuses a field and types; `type(text)` uses current
  focus. `key('tab')` or `key('a', {ctrl = true})` sends a key gesture.
- `dispatch(message)` sends an app message. `frames(n)`, `advance(ms)` and
  `wait_for(predicate, timeout_ms)` drive virtual time, including sleeps
  and timers, without waiting that long in real time.
- `nodes()` reads the current tree, `text(key_label)` reads visible text
  beneath a node, and `model()` returns a detached inspection copy.
- `eval(source)` executes a short expression in the app state. `task(source)`
  runs app-specific assertions as a task and drives frames until it ends.
  These are useful for testing domain helpers and asynchronous workflows.
- `step(name, fn)` groups actions into a named interval in the recording.

`expect(value)` supports `to_be`, deep `to_equal`, `to_be_near`,
`to_contain`, Lua-pattern `to_match`, `to_throw`, and `.never`. Function
`spy`/`stub` helpers keep calls, arguments, returns and errors; assertions
include `to_have_been_called` and `to_have_been_called_with`.

## Snapshots and reproducibility

`expect_tree(name)` records roles, labels, text and rounded rectangles.
`expect_quads(name)` records the drawing as text. `expect_table(name, value)`
preserves mixed table keys, aliases and cycles in a deterministic value
graph. `expect_shot(name, {tolerance = 2})` compares RGBA pixels with an
absolute channel tolerance from 0 to 255. Image failures leave actual and
difference PNGs beside the baseline. Filenames include the test case and
snapshot type, so tree and drawing snapshots can share a name.

Baselines live beside the test under `__snapshots__/`; review them in
source control. `--update` accepts current output. Tests use bundled Noto
fonts, scale 1, Linux platform conventions, light appearance, en-US and a
fixed clock origin. `--system-fonts` uses the machine's fonts instead.
Original captured pixels remain available when drawing playback on another
GPU/backend differs slightly.

## Temporary resources and mocks

Temporary paths also work in ordinary apps:

```lua
local dir = kui.fs.temp_dir { prefix = 'export-' }
local file = kui.fs.temp_file { suffix = '.csv', keep = true }
```

They are absolute paths usable by filesystem calls, SQLite and subprocesses.
A temporary file is created empty and closed. App shutdown removes owned
resources; `keep = true` retains them. Tests also provide fresh app
data/cache directories.

Real I/O is the default. Register mocks inside a case or hook, including
before startup:

```lua
test.mock_http {
  method = 'GET', url = 'https://example.test/count',
  response = { status = 200, body = '42' }, delay = 50,
}
test.mock_process {
  op = 'spawn', command = 'example-tool',
  response = { stdout = { 'first', 'last' }, stderr = '', code = 0 },
}
test.mock_fs { op = 'write', error = 'disk full', code = 'full', passthrough = true }
```

Rules can use `match` for matching argument fields and `once = true` for
one response. Within a mocked service, unmatched calls fail unless
`passthrough = true`; unmocked services remain real. `calls(service)`
returns their call history. HTTP can script errors, delays and progress;
processes can script results, output streams, EOF, stdin and exit codes.
For timed streams use `{text = "line", at = 20}` entries; delays and
`at` offsets are virtual milliseconds. HTTP progress uses a `progress`
array of native progress events with `at` offsets, and `delay` sets the
final response time.
Secrets use a case-local memory store. Filesystem and SQLite mocks inject
failures; real temporary files or memory databases exercise normal storage.
Real workers inherit the case's mocks.

## The inspector and visible execution

```sh
kui test --headed
kui test --ui --perf
kui test --debug
kui test test-results --ui
```

The Kit inspector lists tests and their results, runs a selection, and
browses recorded before/after frames, nodes, app/Kit/core state, messages,
I/O and assertions. `--perf` attaches a performance recording to each app
start, opened with the inspector’s Perf button. Drawing playback renders captured resources without
running app code. Test-run artifacts remain usable after the processes
exit and can be retained from CI.

`--headed` shows automated execution. `--debug` launches the app and shows
its initial frame, then pauses with a Resume/Step prompt. Step executes one
action and pauses again; next-frame advances the app without executing a
test action; `test.pause()` sets another
pause point under `--ui`/`--debug`. Pauses suspend test deadlines. Interact
mode accepts manual input while paused, records it, and resumes the test
from the changed state. Ordinary automated execution accepts scripted input.

The driver owns the logical viewport, scale and clock in both modes.
Resizing the visible window scales its presentation; `resize(w, h)`
explicitly changes the test viewport. Native IME, OS chrome and compositor
integration are a separate testing boundary.

## Reading results as an agent

Every run writes `report.json`, `results.json` and a `run.json` manifest
in the output folder. Each run owns an immutable `runs/<id>/` directory;
rerunning keeps earlier screenshots, state and logs available to an open
inspector. Open either the latest output folder or a particular run with
`kui test PATH --ui`.

```sh
kui test my-app --json
kui test report my-app/test-results --json
kui test output my-app/test-results --file file-0000 --json
kui test events my-app/test-results --case 'counter / increments' --step 2 --json
```

`--json` (also `--reporter json`) writes one JSON report to stdout,
including `status`, `ok`, pass/fail counts, each failure's source and
traceback, diagnostics, and paths to the complete output and event logs.
A failed test still exits nonzero. `test report` reads saved results and
returns nonzero for a failed or incomplete run. `test output` reads the
full stdout/stderr capture, including output from passing tests;
`test events` follows structured messages, logged errors, assertions and
I/O, optionally filtered by file, case and causal step.

The inspector exposes the same evidence in **Output** and **Errors**.
Logged errors remain distinguishable from failed test assertions. The
header reports execution state and pass/fail counts; run controls are
unavailable while a run is active. Select an action to inspect it, or use
the screenshot filmstrip and arrow keys to move through recorded frames.
Before/After/Compare changes the preview without changing live execution.
The inspector uses kui-perf's skin, font stack, density, line width and
accent settings. Drag the dividers between Tests, Actions, preview and
details to resize those panes. Sizes are saved between sessions; **Reset
panes** restores the defaults.

File rows offer **Copy path**, **Reveal** in the file manager, and **Open
in editor** for test source, assertion failures, screenshots, logs and
snapshot artifacts. Editor preference is `KUI_EDITOR`, then `VISUAL`, then
`EDITOR`; without one, the platform's text editor opens the file. For
example, `KUI_EDITOR='code --reuse-window' kui test --ui` jumps to source
lines in VS Code. Quoted editor arguments and paths containing spaces are
supported. Launch errors appear in the inspector.

New recordings retain click/press target bounds and coordinates before
the action changes the app. Selecting an action shows a blue target outline
and crosshair in the preview, including controls that have disappeared.
**Hide markers** restores the unmarked preview. Screenshot/drawing/tree
expectations show pass/fail badges; **View comparison** opens their saved
expected, actual and difference artifacts. Screenshot borders mark the
comparison's outcome. These annotations never change captured pixels or
snapshot baselines.

## Try a deliberate failure

The checkout demo has a passing default test and an opt-in discount fault:

```sh
kui test examples/lua/test-runner-demo/fixtures/discount_failure.lua --ui
# Or step through the actions:
kui test examples/lua/test-runner-demo/fixtures/discount_failure.lua --debug
```

The service replies with a 20% discount, but fault mode charges $20 instead
of $16. Use Errors, before/after app state, Messages & I/O and Output to
follow the request and find the mismatch. Debug publishes the failed
assertion while paused, so it can be inspected before finishing the test.
The fixture stays outside normal discovery; the demo's ordinary test passes.

Large state payloads build only visible rows and cache their formatted text.
They remain complete: scroll to any row, select/copy across offscreen rows,
or use **Copy all** to copy the entire payload.

## Rendering saved app state

Historical browsing reads recorded data and executes no app code. Apps
that want to reconstruct a fresh view register a state adapter:

```lua
require('kui.test').replay {
  capture = function() return { model = model, extra = module_state } end,
  renderer = 'replay_view',
}
```

`replay_view.lua` returns a function `(state, env) -> tree`, or a table
with that `render` function. It imports only the rendering code it needs;
the inspector does not run ordinary app startup, `init`, pending tasks or
real I/O. Captured channels are reconstructed at their saved revision.
Opaque functions, threads and unsupported resources require the adapter
to capture a supported representation.

This creates a fresh rendering of app state. Continuing execution uses a
rerun from startup; it does not restore a running Lua/core checkpoint.

## Rust apps

Rust, Node and C apps keep their language's test runners and binding
helpers. The Rust driver is `kui_native::testing::Drive`.

## The app under test

A smaller cousin of chapter 6's list: an Add button, a Clear button,
a row per item, and a summary line. Two boxes are given keys — `list`
and `summary` — so the test can find them by name; the buttons and the
checkboxes are found by their text, since the stock widgets key
themselves that way.

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/10_testing.rs:view}}
```

## The tests

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/10_testing.rs:tests}}
```

Run them:

```bash
cargo test
```

## What the drive does

- **`Drive::new(core, w, h)`** — a core and a viewport. `.framing()`
  makes it build a frame after every gesture, so the view has caught
  up before the next line.
- **`d.frame(&mut app)`** — one frame, as the runner would build it.
- **`d.key_of("Add")`** — the key of the node declared under that
  name. Buttons are keyed by their text; your own boxes by what you
  gave `with_keyed` or `text_in_keyed`.
- **`d.click_key(&mut app, key)`** — a click by key, the way a screen
  reader presses. No coordinates. `d.click(app, x, y)` is the pointer
  version, and `d.rect_of(key)` gives you the rect to aim at.
- **`d.key(&mut app, "a", KeyMods::NONE)`** — a key press and release,
  by name. `d.keys(app, "jj ww")` types a sequence.
- **`d.texts_under("list")`** — every text inside the node with that
  key, in order. What the user would read.
- **`d.warnings()`** — every warning the core raised. A test that
  asserts it empty catches an unnamed button or a duplicate key before
  a user does.

There is more — `hover`, `drag`, `wheel`, `advance(secs)` to move the
clock for a transition — in the module's docs.

## What to assert on

Two things, and both are cheap here:

- the **model** — `app.items.len()` — because that is what the app
  believes;
- the **frame** — `texts_under`, `rect_of` — because that is what the
  user sees, and a view that ignores its model would pass the first
  assert and fail this one.

## Try this

- Assert that the second row's checkbox is *not* ticked after ticking
  the first. (`app.items[1].done`, and the frame: the checkbox's
  `checked` is in the node's access tree — `d.core.access_tree()`.)
- Add `d.advance(1.0)` and a frame, then assert something about a
  transition. Chapter 9's toasts are a good subject.

## Where this is decided

- The headless driver:
  [`kui_native::testing`](https://docs.rs/kui-native/latest/kui_native/testing/index.html).
- The Node version of the same test:
  [`docs/guide.md`](https://github.com/qxuken/kui/blob/main/docs/guide.md#testing-without-a-window),
  *Testing without a window*.
- A larger example with its own `mod tests`:
  [`apps/splitmux.rs`](examples/apps/splitmux.md).
