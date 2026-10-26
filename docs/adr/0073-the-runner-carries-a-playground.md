---
status: proposed
date: 2026-10-26
---

# The runner carries a playground

> **Proposed (2026-10-26), for later; backlog C133.** Written down so the
> idea keeps its shape until it is picked up; not grilled yet — the open
> questions at the end are the grilling's first round. The `kui` runner
> carries an interactive app of its own: Kit's gallery, kui's primitives
> with their docs, the design system to try, and a place to craft a
> repro — `kui play`.

## Context

- **The gallery exists.** `addons/kit/app` is a runner app (about 190 KB
  of Lua) that shows every Kit component on a page, with a Source button
  per card (the card's real file), a Theme panel that tweaks skin,
  appearance, density, accent, radius, fonts and colour roles live and
  prints the `kit.settings({...})` call, a Keyboard page and a Loading
  page. It runs from the repository: a developer who installed the
  runner from a release has no copy.
- **The docs travel already** (ADR 0071): the runner carries props.md,
  the howto, Kit's generated reference, the book's text and the ADRs, and
  `kui docs <query>` searches them in the terminal. Nothing shows them
  next to something that runs.
- **Kit's design system** is written up as an artifact (tokens per skin
  and appearance, a guideline per component); the gallery's Theme panel
  is the same system, live.
- **kui's own primitives** — `row`, `column`, `text`, `scroll`, floats,
  `when` rules (ADR 0069), composites (ADRs 0007, 0070), editors,
  channels, the lazy form — have docs (props.md, the howto, the book) and
  examples under `examples/`, but nothing a developer pokes at.
- **A repro is a folder**: `kui new`, a `main.lua`, `kui run`. `kui pack`
  (ADR 0068) makes one file of it, `<name>.kui`, that `kui run` opens.
- The runner embeds Lua, Teal, Kit and the docs; it does not embed the
  gallery.

## Proposal

1. **`kui play`** opens the playground, an app the runner carries (packed
   like any app, ADR 0068, extracted to the cache), at the version of the
   runner that opens it. `kui play <topic>` opens it on a topic — a prop
   (`kui play minColumn`), a component (`kui play kit.table`), an ADR.
2. **Kit's gallery**, as today: every component, Source, Theme, Keyboard,
   Loading — the same app `addons/kit/app` is, so the repository's
   gallery and the shipped one are one source.
3. **Primitives with their docs.** A page per kui primitive and prop
   family: the doc text from the docs bundle (ADR 0071) beside a live
   example, the example's props editable (a number becomes a slider, an
   enum a segmented, a colour a field), and the code the example is
   updated as they change. `when` rules, composites and the lazy form get
   examples a reader drives (drag the width, Tab through, toggle reduced
   motion).
4. **The design system to try.** The Theme panel at its centre, every
   token named with its role and usage, contrast shown for the pairs that
   matter, and the `kit.settings` call to copy — the artifact's content,
   live.
5. **A scratchpad for repros.** A Lua (or Teal) editor beside a live
   pane: start from a blank app or from any card's source, edit, see it
   run (as `--watch` does), with the devtools a key away. Save it as a
   folder (`kui new`'s shape) or as one `<name>.kui` archive
   (`kui pack`'s), which `kui run` opens and a bug report can attach.
6. **Search across all of it** — the docs bundle's search, landing on the
   page that shows the thing.
7. **Opt-in, nothing written unasked.** `kui play` is a command; it
   writes into a project only when the developer saves there.

## Considered options

- **The gallery as a separate download**: one more thing to find and keep
  at the runner's version.
- **The docs site only** (the book): it reads, it does not run; and it is
  hosted on a home server (ADR 0071).
- **A web playground** (kui compiled to the web): the runner is native,
  and so are its windows, fonts, input and devtools — a web copy would
  show a different kui.

## Consequences

- A developer with only the runner finds Kit, the primitives and their
  docs, and a place to try them, at the runner's version.
- The runner grows by the gallery's Lua (about 0.2 MB) and the
  playground's own pages; the docs are carried already.
- The gallery moves from "an example in the repository" to "part of the
  runner": its pages become documentation that must stay true, and its
  test (the gallery walk) guards a shipped feature.

## Open questions (the grilling's first round)

1. Is it one app (gallery + primitives + design system + scratchpad) or
   the gallery plus a devtools tab for docs, opened from any running app?
2. Does the scratchpad edit with Kit's `text_area` (no syntax colouring
   today), or does kui grow a code editor component first?
3. How does a primitive's page get its editable example: written by hand
   per page, or generated from the schema (every prop row a control)?
4. Does a saved repro carry the runner version and its Kit version, so
   `kui run` on another machine says when they differ?
5. Should `kui play` work offline entirely (yes, as the docs do), and does
   it link out to the book for long chapters?
6. Is the playground also how Kit's design system is published, replacing
   the static artifact, or do both stay?
7. Where does it live in the repository — `addons/kit/app` grown, or a
   new `apps/play` that embeds the gallery?
