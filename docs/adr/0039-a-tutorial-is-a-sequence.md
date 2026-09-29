---
status: accepted
date: 2026-09-29
---

# A tutorial is a sequence, and the book reads it

> **Accepted and built 2026-09-29**, the day it was proposed (backlog
> DX27). Asked by the question "does kui have a human-friendly DX?",
> answered honestly: the mechanics are friendly — a headless test is a
> list of gestures, a warning names its fix, one model runs four
> bindings — and the front door is not. The README reaches "Examples"
> at line 122 with no hello-world before it; `howto.md` and `props.md`
> are reference prose, written for the reader who knows the name of
> what they want; and the counter, the one example every binding has,
> needs the devtools harness and a hand-built `Value` before a reader
> has met `App`. Nothing in the repo teaches kui in order.
>
> This ADR adds a fifth kind of example, the **tutorial step**, and a
> **book** that reads the steps. It amends ADR 0021 on two of its rules
> — a step's subject is a *concept added*, not a widget or a feature,
> and a step runs on the shipped launcher with no harness — and keeps
> the rest: a step is an `[[example]]`, in the windowed smoke round by
> being one, linked from `examples/README.md` by the pin test.

## Context

ADR 0021 fixed the axis of the reference gallery: one subject per
example, the subject a widget, a feature or an app, every example inside
the harness so the smoke round can open it on both bases. That is the
right shape for a gallery — a reader who wants to see `hover` opens
`features/hover.rs` and sees every spelling of it — and the wrong shape
for a first hour. A gallery is read by name; a beginner has no names
yet.

The three doors the repo offers a newcomer are the README (1,522 lines:
the pitch, the benchmarks, the release history and the design notes in
one file, no hello-world), `docs/howto.md` (a task index, which presumes
a task) and the examples (each complete, each inside `kui-devtools`,
each written for whatever round needed it). What is missing is the
fourth door: a sequence — a frame, then a box, then a message, then a
key, then a float, then motion, then a test — where each page adds one
thing to the last.

## Decisions

1. **`examples/rust/tutorial/` is a fifth kind.** Beside `apps/`,
   `widgets/`, `features/` and `tools/`. A file in it is a **step**: a
   whole program, named `NN_concept.rs`, whose header says which concept
   it adds to step `NN-1` and which chapter reads it. Ten steps at
   first: hello, layout, counter, controls, list, keyboard, floats,
   motion, effects, testing. Each is short — under 150 lines — and
   each compiles on its own, with no shared module, so a reader can
   copy one file out and have an app.

2. **The target is `tutorial_NN_concept`.** An explicit `[[example]]
   name` in `kui-native`'s manifest, the way `c_` and `lua_` prefix the
   two panel hosts (ADR 0021, decision 2): `tutorial_03_counter` beside
   `counter`, with no collision and a `cargo run --example` that sorts
   the sequence in order.

3. **A step runs on the shipped launcher alone.** `kui_native::app(..)
   .size(..).run(App)` and nothing else: no `kui_devtools::main!`, no
   `Example`, no `--headless`. The book teaches the API an app is
   written against, and the harness is not part of it. The windowed
   smoke round still opens every step, because `KUI_SMOKE_FRAMES` is
   honoured by `kui-native` itself in a debug build (and under
   `--features smoke`), not by the harness. A step that carries a `mod
   tests` is `test = true` in the manifest and runs under `cargo test`,
   which is its headless channel; none is in the `headless = [...]`
   list, since none has a `--headless` flag.

4. **The book is `docs/book`, an mdBook, and it includes the steps.**
   A chapter's code is `{{#include ../../../examples/rust/tutorial/
   NN_concept.rs:anchor}}` over `// ANCHOR:` regions in the step, never
   a pasted block, so the book cannot say what the step does not do. A
   step that stops compiling fails `cargo build --examples`; an anchor
   that goes missing fails `mdbook build`, which CI runs.

5. **The book's voice is not the repo's.** The repo's prose is written
   for its maintainer: long sentences, ADR and backlog ids inline, the
   history of a decision beside the decision. The book is written for
   the reader who has none of that: short sentences, one idea per
   paragraph, no ids in the body — the links to the ADR, the `howto`
   row and the reference example sit at the end of each chapter, under
   *Where this is decided*. Every chapter opens with what the reader
   will have on screen at its end and closes with something to try.

6. **The reference gallery does not contradict the book.** Where a
   gallery example spells something the book teaches another way —
   `apps/counter.rs` built its click payloads by hand where chapter 4
   uses `#[derive(Message)]` — the gallery moves to the book's spelling.
   The book is the path in; the gallery is what the path leads to.

## Considered options

- **Tags on the existing examples** (beginner / intermediate / advanced
  in `examples/README.md`). Cheap, and wrong: the gallery examples are
  complete on purpose — `hover.rs` shows four spellings of hover — and
  a beginner tag on one would not make it a first page. A sequence is a
  different artefact from a gallery, and the two want different rules.

- **The book as rustdoc** (`#![doc = include_str!("../book.md")]` on
  `kui-native`). It ties the text to the crate and publishes with `cargo
  doc`, and it is the wrong reading surface for a narrative — one page,
  no chapters, no sidebar, and every code block a doctest that would
  need a window to run or an `ignore` that says nothing.

- **The steps inside the harness**, like every other example. They
  would get the devtools panel for free, which the book wants the reader
  to open. But the first thing a reader would see is
  `kui_devtools::main!` and `impl Example`, neither of which their app
  will have, and the book would spend its first chapter explaining what
  to ignore. The chapter on the devtools shows `.devtools(true)` on the
  launcher instead, which every app has.

## Consequences

- Ten new `[[example]]`s in `kui-native`, in the windowed round; one
  with tests under `cargo test`. The pin test in the devtools crate
  wants a row for each in `examples/README.md`, which the *tutorial*
  table provides.
- `mdbook` is a build dependency of the docs and of CI's Linux job, not
  of any crate.
- `examples/README.md` gains a fifth kind; ADR 0021's "one subject"
  test — "the first sentence of the header names it" — holds for a
  step, whose subject is its concept.
- A change to a step's anchors is a change to the book; the two land
  together or `mdbook build` says so.

## Action items

- [x] `examples/rust/tutorial/01_hello.rs` … `10_testing.rs`, with
      anchors; registered as `tutorial_NN_*`; `10_testing` `test = true`.
- [x] `docs/book`: `book.toml`, `SUMMARY.md`, fourteen chapters.
- [x] `examples/README.md`: the kind, the table.
- [x] `apps/counter.rs` on `#[derive(Message)]`.
- [x] README: a *Start here* section, and the book under *Reference*.
- [x] CI: `mdbook build docs/book` on the Linux job.
