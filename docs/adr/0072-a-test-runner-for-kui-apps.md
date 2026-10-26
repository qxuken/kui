---
status: accepted
date: 2026-10-26
---

# A test runner for kui apps

> **Accepted (2026-10-26); backlog C122.** Settled by a grilling round
> with the user. `kui test` runs a runner app's tests headless and
> reports them; a `kui.test` module gives tests a vocabulary, assertions
> and four kinds of snapshot that read the same on every machine.

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
- The runner gains a test mode and carries the test fonts it already
  builds with; the book's testing chapter is rewritten around
  `kui test`.

## Building order

1. `kui test` discovery, the fresh headless runner per file, the report
   and exit code; `kui.test` with `describe` / `it`, the driving API and
   `expect`.
2. The four snapshots, `--update`, the fixed environment and bundled
   fonts.
3. `--filter`, `--watch`, `--reporter junit`.
4. The samples' and the task list's tests moved onto it; the book's
   testing chapter; ADR 0071's agent block.
