---
status: accepted
date: 2026-10-26
---

# A test runner for kui apps

> **Accepted (2026-10-26); backlog C122.** Settled by a grilling round
> with the user. `kui test` runs a runner app's tests headless and
> reports them; a `kui.test` module gives tests a vocabulary, assertions
> and four kinds of snapshot that read the same on every machine.
> **Amended (2026-11-06)** after investigation and a second grilling:
> the first release includes a Kit inspector, headed/debug execution,
> durable state and drawing playback, temporary resources and mocks.

## Context

- A runner app's tests run today as `kui run test --headless`, a
  convention the book teaches: `test/main.lua` drives the app, reads
  nodes, presses by label and asserts with `assert`. Kit's samples and
  the book's task list carry such folders; there is no `kui test`, so
  ADR 0071's agent block had to name the convention.
- The runner already drives an app headless with a frame clock under
  its control, and draws pictures without a window (`kui run --shot`,
  `--record`).
- Rust, Node and C apps have their language's test runners and kui's
  `testing` helpers in each binding.

## Decision

1. **`kui test [path]`** finds an app's tests — `test/**/*.lua`,
   `test/**/*.tl` and `*_test.lua` / `*_test.tl` beside the sources —
   and runs each file in a fresh headless runner: no window, the frame
   clock the test's, the app started from the folder's manifest. It
   prints each test's result, the failing assertion with its file and
   line, a summary with the slowest tests, and exits non-zero on any
   failure.
2. **`kui.test`**, a documented module:
   - `describe(name, fn)` / `it(name, fn)` grouping, `before_each` /
     `after_each`;
   - the driving API the samples already use, named and documented:
     start the app, `press` / `click` / `type` / `key` by label or key,
     `frames(n)`, `wait_for(cond)`, read nodes and text, `dispatch` a
     message;
   - `expect(x)` with a small set of matchers — `to_be`, `to_equal`
     (deep), `to_be_near`, `to_contain`, `to_match`, `to_throw`,
     `never` — each failure printing what was expected and what came.
3. **`--filter <text>`** runs the tests whose `describe` / `it` path
   contains it; **`--watch`** reruns on save.
4. **Four kinds of snapshot**, stored beside the test in
   `test/__snapshots__/`, accepted with **`--update`**:
   - `expect_tree(name)`: the node tree as text — roles, labels, keys,
     text, rects rounded to whole pixels — diffed line by line;
   - `expect_quads(name)`: the draw list as text — kind, rect, colour;
   - `expect_shot(name)`: a PNG compared with a per-pixel tolerance; a
     failure writes `name.actual.png` and `name.diff.png` beside it;
   - `expect_table(name, value)`: a Lua value — a model, a message, a
     reply — written deterministically (keys sorted, numbers in a fixed
     format, cycles named) and diffed as text.
5. **The same on every machine.** Tests run with kui's bundled test
   fonts and a fixed scale, theme, locale and clock, so a snapshot taken
   on a Mac matches on Linux CI; `--system-fonts` opts out.
6. **Output for CI**: human output by default; **`--reporter junit`**
   writes JUnit XML to a file, with each test's time.
7. **Scope**: runner apps, Lua and Teal. Rust, Node and C keep their own
   test runners and kui's `testing` helpers. Kit's samples and the
   book's task list move onto `kui test`, and ADR 0071's agent block
   names `kui test`.

## Considered options

- **Keep the `kui run test --headless` convention**: no discovery, no
  report, no snapshots, and every app writes its own driving helpers.
- **A Lua test framework from the ecosystem** (busted and the like):
  not built into the runner, knows nothing of driving a kui app, and a
  dependency a one-binary runner does not take.
- **Pictures only**: a shot is brittle across fonts and machines and
  says nothing about why it changed; the text snapshots say which node
  or quad moved, the shot what it looks like.
- **System fonts by default**: snapshots would differ between a
  developer's machine and CI.

## Consequences

- An app gets `kui test` with discovery, a report, snapshots and CI
  output, and writes no harness of its own.
- The runner gains a test mode and bundles an isolated, licensed test font set; the book's testing chapter is rewritten around
  `kui test`.

## Amendment: execution, recording and the inspector

The following decisions extend the first-release scope above, including
the previously deferred C128 inspector. They were agreed on 2026-11-06.

### Execution and isolation

- Run files sequentially, each in a fresh child process. The parent
  reports a crash or timeout and continues with the other files.
- Collect test definitions before starting the app. `test.start { args =
  {...} }` resolves the app's manifest and configures arguments before
  entry loading and `init`; `before_each` may start it. Each case begins
  without a running app and automatically tears its app down afterward.
- Each start has fresh app and framework state. Each case owns temporary
  data/cache directories, retained across starts to test reopening. App-run contexts own clients, secrets, input, timers,
  mocks and resources; a fresh Lua VM alone does not reset the current
  process-wide defaults. Stop workers, child processes and outstanding
  operations before removing their temporary resources.
- One virtual clock drives frames, `kui.now`, sleeps and timers. Test
  advancement does not sleep in real time. External I/O and stalled
  execution have separate real-time deadlines.
- Default execution is headless. `--headed` shows the same controlled
  test session without pausing. `--debug` opens it with inspector
  pause/step/resume controls. `--ui` opens the inspector and can launch
  either execution mode. Debug pauses suspend test deadlines while the
  native windows remain responsive.
- Headed execution presents the driver-owned logical surface. Its
  viewport, scale, fonts, theme and environment match headless execution;
  physical window resizing and monitor scaling change presentation only.
  Changing the test viewport is an explicit driving/debugger action.
- Automated execution accepts scripted input. An explicit **Interact**
  mode while paused accepts manual gestures, records them as manual test
  steps, and lets the test resume from the modified state. Native IME,
  OS chrome and compositor integration remain a separate test boundary.

### Temporary resources and mocks

- Reusable `kui.fs.temp_dir(opts)` and `temp_file(opts)` return absolute
  paths, work outside tests, and clean up when the app ends. `keep = true`
  retains a resource. Tests allocate underneath a case-owned temporary
  root that the parent can also clean after a crashed file process.
- Real I/O remains the default. First-release mocks include Lua function
  spies/stubs; HTTP responses, errors, delays and progress; scripted
  subprocess results and streams; a case-local memory secrets store;
  and targeted filesystem/SQLite failure injection. Normal filesystem
  and SQLite tests use real temporary files or memory databases.
- Workers remain real and inherit the case's mock context. A complete
  virtual filesystem and scripted fake workers are later facilities.
- Hook native effects beneath the public Lua wrappers: replacing a
  public function alone misses captured aliases and other Lua states.
  Preserve validation, try/plain errors, yielding and resource lifecycle.
- Within an explicitly mocked service, unmatched calls fail unless
  passthrough was enabled. Unmocked services keep real I/O. Record calls,
  arguments, results/errors, virtual timing and app/worker identity.

### Recorded history and state rendering

- A versioned test-run artifact survives its processes and opens from
  CI. It contains cases, source locations, outcomes and test steps;
  inputs, events, messages, operations and causal relationships; captured
  frames, node trees, app/Kit/core inspection state; and assertion and
  snapshot expected/actual/diff data. Binary bodies remain bytes.
- Each driving action and explicit named test step captures before/after
  state. Events and intermediate frames belong to that interval;
  screenshots default to step boundaries, with every-frame capture
  available for animation investigation. This does not instrument every
  Lua assignment.
- Preserve full drawing packets (display lists, clips, shader inputs,
  textures, glyph atlas, viewport, scale, background and clock), so a
  historical frame can render without executing app code. Deduplicate
  shared resources and unchanged captures. Original PNGs preserve the
  actual pixels when another GPU/backend renders a packet differently.
- Save app state with a snapshot-specific value graph that preserves
  aliases, cycles and mixed tables, and materializes channel contents at
  the captured revision. Functions, threads and unsupported resources
  remain explicit opaque values with restorability information. The
  ordinary event-value converter is not a state serializer.
- Global `model` is the default app-state root, not all application
  state. Explicit capture/render adapters include module data and
  supported resources and reconstruct a fresh view in isolation. Kit
  supplies its own state provider. Core inspection includes focus,
  editors, scrolling and animation state where available.
- Historical browsing executes no app code. Fresh state rendering uses
  the explicit adapter without running ordinary app startup, `init`,
  `update`, pending callbacks or real effects. Continue execution by
  rerunning the test from startup. Full Lua/core runtime checkpoints and
  recorded-response execution replay remain later capabilities.

### Kit inspector

- Build the inspector as a Kit app carried by the runner, following
  `kui-perf`'s shell/inspector separation. Keep its native event loop in
  the parent process, separate from the test file's child.
- List/filter/run tests, select a test step, inspect before/after frames,
  pick historical nodes, compare app/framework state, follow input to
  messages and I/O, and inspect assertions and snapshot diffs. Link an
  optional performance recording rather than overloading `.kuitrace`,
  which deliberately omits payloads and state.
- Pause before an action, advance an action or frame, resume, rerun and
  enter manual Interact mode. Selection of recorded history and live
  execution controls are distinct operations.

### Investigation consequences

The current headless and windowed loops use real time, and headed layout
imports monitor size/scale. A driver-owned clock and presentation seam
are required. No bundled production test-font assets or isolated font
database constructor currently exist; add both rather than appending
test fonts to the OS database. Existing sample `test/shot.lua` files are
picture-staging scripts; move them outside discovery. Prefer a `.tl`
file over its generated `.lua` sibling and exclude declarations and
snapshot artifacts.

## Building order

1. Shared execution/lifecycle foundations: app-run contexts, virtual
   time, deterministic environment/fonts and fresh case state.
2. Runner discovery/process supervision, assertions/hooks/driving,
   filtering/watch/reporters and snapshot comparison.
3. Reusable temporary resources and the agreed mock facilities.
4. Durable recording, value graphs, framework state providers, drawing
   playback and explicit app-state capture/render adapters.
5. The Kit inspector, headed presentation and debug/manual controls.
6. Sample/tutorial migration, book/declarations/generated guidance and
   validation of the complete headless/headed/inspector workflow.

## Implementation (2026-11-06)

Built in the order above. The controller and artifact protocol live in
`crates/kui-cli/src/testing/`; the bundled inspector is `tools/kui-test`.
The sample migrations preserve their domain assertions in `fixtures/`,
and the task tutorial drives its actual fields and controls. Integration
coverage exercises isolation, timeouts, debug stepping, mocks and worker
inheritance, temporary cleanup, typed declarations and snapshot diffs.
Drawing and state-render tests compare reconstructed pixels without
reexecuting app startup. Headless and headed fixture runs produce the
same boundary pixels.

### Inspector and output refinement (2026-11-06)

Reruns retain separate `runs/<id>/` directories. The top-level manifest
selects the latest run; selection and media reference immutable run paths.
Native execution activity controls availability of Run actions, including
startup and completion races. Busy clicks are ignored without discarding
history or presenting an exception.

The inspector follows kui-perf's theme settings. Test selection, action
selection, the preview, the screenshot filmstrip and detailed evidence
have distinct regions. Before/After/Compare is a preview choice. Output
and Errors expose stdout/stderr and structured failures directly.
`kui test --json` and the windowless `test report`, `output` and `events`
queries make that evidence available to agents, with nonzero status for
failed/incomplete results and explicit paths to full logs.

The inspector's Tests, Actions and preview/detail dividers are draggable,
with saved proportions and a Reset panes action. Debug initializes the
app and records/presents its first frame before the initial pause. A
visible prompt explains Resume and Step; one action executes before Step
pauses again. A missing earlier frame uses the initial app state in the
Before preview instead of an unexplained empty panel.
