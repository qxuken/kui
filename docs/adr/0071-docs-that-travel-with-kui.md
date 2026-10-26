---
status: accepted
date: 2026-10-26
---

# Docs that travel with kui

> **Accepted and built (2026-10-26); backlog C118.** Settled by a grilling round
> with the user. What each channel kui reaches a machine by (ADR 0068)
> carries of its docs, and how a developer — or the coding agent they
> work with — finds the docs that match the kui they run. Writing docs
> or agent files into a project is opt-in.

## Context

- **crates.io** packages a crate's own folder. `docs/` sits at the
  repository's root, so props.md, the howto, the ADRs and the book never
  reach a crate; kui-core ships its README, docs.rs builds rustdoc.
- **npm** `@qxuken/kui` ships `props.md`, `howto.md`, `CHANGELOG.md`,
  `docs/adr/` and `kit/`.
- **The `kui` runner** ships the editor declarations already: `kui
  types`, which `kui new` calls, writes `.kui/types` (`kui.d.tl`,
  `kit/ui.d.tl`, the LuaLS `kui.lua` with its doc comments,
  `kui_runner.lua`, `kit/ui.lua`), a `.luarc.json` and a `tlconfig.lua`.
  It ships no prose, and the declarations go stale after `kui upgrade`
  until `kui types` is run again.
- The prose — the ADRs, props.md, the howto and the book's chapters
  without their pictures — is about 630 KB gzipped against a 28 MB
  runner.
- Kit's components are described once, in `addons/kit/schema.lua`
  (ADR 0066), but no Markdown reference is generated from it.
- The book is hosted on a home server; it is not something an offline
  developer can rely on.

## Decision

### The runner

1. **`kui docs`, always there.** `kui docs` opens the book for this
   runner's version; `kui docs <query>` searches the docs it carries and
   prints the matching props.md rows, howto entries and Kit component
   entries in the terminal; `kui docs --adr <n>` prints an ADR. Running
   it is the request, so nothing about it is opt-in.
2. **The runner carries the docs**, compressed: props.md, howto.md,
   Kit's component reference, the book's chapters as text and the ADRs,
   for its own version.
3. **Writing them into a project is opt-in.** `kui new` and `kui types`
   take `--docs`, which writes `.kui/docs/` (gitignored with the rest
   of `.kui/`), and `--agents`, which writes the agent block (decision
   5) and implies `--docs`. Without either, a project gets what it gets
   today. `--docs` writes everything listed in decision 2; there is no
   lean variant.
4. **The choice is the app's, in its manifest**: `tooling = { docs =
   true, agents = true }` in `kui.config.lua` (or `.tl`), written by
   the flags, so it is visible and committed and later refreshes keep
   it.
5. **The agent block**: a short Markdown block — the kui version, where
   the types and docs are, the runner's commands (`kui run --watch`,
   `kui check`, `kui test`), and a few house rules (Lua 5.5, Kit's
   components, events as data). It is written into `AGENTS.md`, or
   appended between markers to an `AGENTS.md` or `CLAUDE.md` the app
   already has; kui rewrites only what is between its markers.
6. **Nothing goes stale.** `.kui/` records the runner version that
   wrote it. `kui run` and `kui check` rewrite `.kui/types` when the
   version differs — and `.kui/docs` and the agent block when the app
   opted into them — saying so in one line. They never create what the
   app did not opt into.

### The packages

7. **crates.io**: the `kui` crate gains `kui::docs::props` and
   `kui::docs::howto` as rustdoc pages and an ADR index linking to the
   book; `scripts/gen.nu` copies the files into the crate's folder so
   `cargo package` takes them. The ADRs themselves stay out of the
   crate. Nothing there is opt-in: rustdoc is how a crate is read.
8. **npm** adds the book's text and Kit's component reference beside
   what it ships, so the three channels carry the same set. Nothing
   agent-specific is added to npm.
9. **Kit's component reference is generated** from its schema by
   `kui-addon-schema` (a Markdown emitter beside the C, Odin, Rust and
   Teal ones), checked by `gen.nu --check` like every other generated
   file.

## What the building changed

- **There is no `kui` crate**: the pages are `kui_native::docs::props`,
  `::howto` and `::adr` (`crates/kui-native/src/lib.rs`), in the crate
  most apps depend on, which re-exports all of kui-core. The copies in
  `crates/kui-native/docs` are not verbatim: a bare fence is `text` and a
  `rust` one `rust,ignore`, so rustdoc runs none as a doctest; links to
  props.md and howto.md are the crate's own pages, links into the
  repository go to GitHub (`main`). The ADR index links each record on
  GitHub — the book does not carry them — and the book from its head.
- **The book has no copy per version.** `book.yml` publishes each
  release to the one address, so `kui docs` opens its root (the
  workspace's `homepage`) and says it is the latest release's, pointing
  at `kui docs <query>` and `--adr` for this version's own. The opener is
  `kui_native::open_external`, or the command `KUI_BROWSER` names, which
  is how a test reads the address without a browser.
- **Kit's reference is `addons/kit/bindings/reference.md`**: the
  generator writes into the folder `gen` is given, beside the bindings.
  It is `kit.md` in the runner's bundle and in `.kui/docs`, and
  `kit/reference.md` on npm, beside Kit's wrappers there.
- **The book's text is its chapters with their includes expanded**: each
  mdBook `{{#include file:anchor}}` / `{{#rustdoc_include …}}` is the
  lines it names (`crates/kui-cli/build.rs`, `packages/kui/prepack.mjs`),
  so a chapter reads whole without `examples/`. The chapters are
  `docs/book/src/*.md`; the generated Examples part, all pictures, stays
  out.
- **`kui docs <query>`** searches the props.md rows (every table, not
  only props), the howto's questions and Kit's components and types; an
  exact name in either spelling (`hoverBg`, `hover_bg`, `kit.table`,
  `table`) ranks first, ten print (`--all` for every one), and the ADRs
  whose title holds the query's words are named after them. No match
  exits 1. `kui docs --adr` alone lists the records.
- **There is no `kui test`.** The agent block names `kui run test
  --headless`, the command the book's tests run with.
- **The stamp is `.kui/version`**, written with `.kui/types`. A refresh
  writes `.kui/types` where the app has them *or* its manifest opted into
  docs or agents — a fresh clone of an opted-in app has no `.kui/`, and
  the types came with the flag — and says so on stderr. `kui types`
  without flags keeps what the manifest opted into, as any refresh does.
- **Placing `tooling` in an app's manifest**: its `tooling = { … }` is
  replaced in place if it has one, else a line opens the table after
  `return`, indented as the line after it; the manifest is read back and
  put back if it does not say what was meant, the error asking for the
  line by hand. A folder with no manifest gets one of `tooling` alone.

## Considered options

- **Agent files by default**: what a project carries is the developer's
  choice; an `AGENTS.md` nobody asked for is noise in a repository, and
  the user asked for opt-in.
- **`.kui/config` for the choice**: hidden in a gitignored folder, so a
  fresh clone forgets it; the manifest is where an app's settings are.
- **A lean `--docs`** without the ADRs and the book: about 200 KB of
  text instead of 2 MB, which no one has needed.
- **Only the online book**: it is on a home server, and an offline
  developer or an agent reads files.
- **Full ADRs on docs.rs**: a poor reader for seventy design records;
  the index links to the book.

## Consequences

- A developer offline, or an agent in their editor, reads the docs of
  the kui they run, at its version, after opting in once.
- The runner grows by about 0.7 MB; `kui-addon-schema` gains an emitter;
  `gen.nu` copies docs into the `kui` crate and the npm package.
- `kui upgrade` no longer leaves a project's types behind.

## Building order

1. Kit's reference from the schema; the docs bundle embedded in the
   runner; `kui docs` (open, search, `--adr`).
2. `--docs` / `--agents`, `tooling` in the manifest, the agent block
   with markers, the version stamp and the refresh in `kui run` /
   `kui check`.
3. The `kui` crate's rustdoc pages and npm's added files.
