---
status: proposed
date: 2026-10-27
---

# kui-perf reads a recording in Kit, over columns Rust keeps

> **Proposed (2026-10-27); backlog F200, F245–F251, and C147, C148,
> C149 and C102 built on the way.** Settled by a grilling round
> with the user, after Kit landed on main. It replaces ADR 0058's
> decision 13 with what `kui-perf`, the trace inspector, is and how it
> is built. It stays proposed until the spike (decision 11) reports:
> the spike's numbers accept it as written, or amend decision 1's
> language.

## Context

- ADR 0058 made the recording a timeline (`crates/kui-trace`) and put
  its reading in two places: the devtools' Perf tab, core-drawn and
  280 px wide at its narrowest, for the app it is docked in; and
  `kui-perf`, a tool on the kui tools design system, for everything
  else — a `KUI_PERF_TRACE` file, a CI run, a Node app, a long session.
  `kui-trace convert` opens a recording in Perfetto, speedscope or the
  Firefox Profiler, which draw spans and counters and know nothing of a
  frame's causes, holders, verdict, burns, or the components it drew
  fresh.
- Kit is on main (ADRs 0059–0067): Lua/Teal components, skins, a
  channel-fed trace waterfall (`addons/kit/src/waterfall.tl`) whose
  cost is bounded by its width through density levels, tables, charts,
  and the `kui` runner that runs a Lua app (ADR 0064).
- Kit's M0 spike measured a waterfall of 18,000 spans panned in 1.15 ms
  and a 100,000-row table scrolled in 1.26 ms, in Lua. A recording is
  bigger: ten minutes of `KUI_PERF_TRACE` at 120 Hz is about 72,000
  frames and up to a million spans, and nobody has drawn one in Lua.
- `kui-core` depends on `kui-trace`, so `kui-trace` cannot depend on
  `kui-core` — and the data channel (ADR 0065) lives in `kui-core`
  (`kui_core::channel`), though it uses nothing of the core's but
  `Value`, which uses nothing of the core's but `Key` in one helper.
- The recording grew three records kawoosh asked for (backlog F212,
  F215, F216): spans timed where no core is at hand, notes, and each
  component placed with why it was drawn fresh.

## Decision

1. **`kui-perf` is a Lua/Teal app on the `kui` runner, built from
   Kit** — if the spike (decision 11) meets its budgets. It lives in
   the kui repo, under `tools/kui-perf`, and is released with kui's
   tags. How it ships — an app the runner carries, or a binary — is
   decided with the spike's numbers.
2. **Rust parses, keeps and analyses; Lua lays out and draws.**
   `kui-trace` decodes the recording, builds the timeline, and answers
   every reading of it — phases, verdicts, burns, input paths, spans
   grouped, placements, filters — once, in Rust, as the Perf tab's
   readings are. What Lua draws from arrives as data channels; what it
   shows of one frame or one span arrives as a small table when asked
   for.
3. **`kui-value` and `kui-channel`, crates of their own**, so
   `kui-trace` can make channels without `kui-core`:
   - `kui-value`: `Value` and `json`, the Lua conversions
     (`lua_to_value` / `value_to_lua`, now in `kui-lua`) behind a `lua`
     feature, and the derives (`kui-derive`) behind a `derive`
     feature; no other kui crate under it. `Handles`, which spells a
     node key, stays in `kui-core`.
   - `kui-channel`: the store, columns, revisions and registry, on
     `kui-value`; a channel handle's Lua side behind its own `lua`
     feature.
   - `kui-core` re-exports both where they are today (`kui_core::Value`,
     `kui_core::json`, `kui_core::channel`): no binding changes.
4. **`kui-trace`'s Lua API is behind a `lua` feature.** It registers a
   module — `trace.open(path)` and the reader's other sources — whose
   recording hands Lua channels of typed columns: spans (start, length,
   depth, track, group, name, origin, node), frames (begin, end, work,
   phases, verdict, unchanged, burn), the lanes' instants (inputs,
   events, marks, notes, placements), counters and gauges. A filter
   (decision 8) and a new recording publish new revisions of the same
   channels. The runner enables the feature and registers the module;
   `kui-trace` stays free of dependencies without it.
5. **A recording is read from any byte stream.** `kui-trace` gains a
   decoder bytes are pushed into, `Stream::push(&[u8])`, which keeps a
   record cut between pushes and grows its timeline as records arrive;
   and a `Transport` trait — `poll(&mut self, buf) -> Polled`, one of
   `More`, `Idle` (nothing yet, more may come), `Ended` — for where the
   bytes come from. `Trace::from_bytes` is one push and the end. v1's
   transport is the file; a pipe or a socket, for attaching to a
   running app later, is another implementation behind the same trait.
6. **The open file is watched, always.** The file transport checks its
   size and modification time every 250 ms on a thread of its own, as
   the runner's `--watch` does, and wakes the loop only when one moved,
   so an idle inspector costs nothing. A file that grew is decoded on;
   one that shrank, or whose first bytes changed, is a new recording,
   read again whole, the selection kept where its frames still are. A
   **Live** switch in the toolbar, on by default: on, the new records
   show as they come, and a view showing the recording's end moves
   with it until the user pans or zooms away, when a "↦ latest" chip
   takes it back; off, a bar says how many frames came and a refresh
   brings them in.
7. **What v1 shows**, top to bottom:
   - **the overview**: a bar a frame over the whole recording, burns
     marked under them, a range dragged across it (the Perf tab's), and
     a minimap brushing the time window the timeline shows;
   - **the timeline**: Kit's waterfall with a time ruler, its tracks —
     main (spans by depth), the loop (asleep and woken), each declared
     track, a lane of inputs, events, marks, notes and components drawn
     fresh — and a long idle sleep drawn compressed;
   - **graph lanes** under it on the same axis: a counter a step graph,
     a gauge a line, each short by default, expanded, hidden or
     reordered, the choice remembered per recorded app's name;
   - **the frame selected**: its verdict, input to screen, phases,
     holders, components drawn fresh and why, notes;
   - **the tables over the range**, in tabs: spans, origins,
     components, counters and gauges, burns, gaps;
   - **a status bar**: the session — app, platform, Hz, budget, length.

   A span or a row selected in one is selected in the others.
8. **Filters against noise**, one bar for every view: origin toggles
   (kui, the app, each extension; the devtools' own spans off by
   default), a span name search, a minimum duration, and frames by
   verdict or cause ("only slow", "hide drew nothing new"). The bar
   builds a plain filter value that Rust applies over the columns; the
   views dim or hide, and a reading taken under a filter says so. No
   query language of kui's own: a Lua predicate over spans is the
   query box's candidate for v1.1, decided by the spike's measure of it.
9. **Adaptive by the window's size**, with ADR 0069's size conditions:
   wide (≥ 1200 px) the detail and the tables side by side under the
   timeline; medium, the two as tabs of one pane; narrow (< 700 px) the
   overview a strip, the timeline the height, one panel over it. The
   splits are dragged and remembered.
10. **Opening, keys, look, preferences.**
    - **Opening**: `kui-perf file.kuitrace`, File ▸ Open, a file dropped
      on the window, recent files; and the devtools' Perf tab gains
      *open in kui-perf*, which saves the trace and starts the tool on
      it.
    - **Keys**: ← → and `h` `l` the frame before and after; ↑ ↓ and `j`
      `k` a row in the focused table or lane list; `[` `]` the slow
      frame before and after; `{` `}` the burn; `W` `S` zoom about the
      pointer, `A` `D` pan (Perfetto's and Chrome's); `F` fit the
      selection; `0` the whole recording; `/` the name search; `Esc`
      clears the selection or the range. No vim mode: nothing else
      wants `hjkl`.
    - **Look**: Kit's Phosphor, compact, hairlines of 1.5, Berkeley Mono
      first in both families and the generic faces after it, its green
      roles — ink, text, selection, focus, accent, rules — replaced by a
      yellow set around `#ffd400`, through `kit.settings` as any Kit app
      sets them.
    - **Preferences** — lanes and their order per app, splits, look,
      recent files, the filters as last set — in one file in the app's
      own folder, `kui.data_dir` (ADR 0068: `~/Library/Application
      Support/<id>`, `%APPDATA%\<id>`, `$XDG_DATA_HOME/<id>`), written
      through the runner's `fs`. No new door.
11. **The spike decides decision 1.** Before the app: the two crates
    (decision 3), `kui-trace synth` (decision 12), the `lua` feature
    (decision 4), and a bare Lua app on the runner with the waterfall
    and the frame table over a synthetic recording of 72,000 frames
    and a million spans. Its budgets, on the machine Kit's M0 was measured
    on, in a release build:
    - **open** — the file read, decoded, analysed and the first frame
      drawn — in **1 s** or less;
    - **pan and zoom** across the whole recording, zoomed fully out
      included, **4 ms** a frame or less;
    - **the frame table's scroll** **1.5 ms** a frame or less;
    - **idle**, the window still, **0%** of a core;
    - and, measured for decision 8, a Lua predicate called from Rust
      on every span with one table reused: **200 ms** or less makes the
      query box cheap.

    A budget missed is first a generic primitive in Kit or the core —
    a level-of-detail pass over a channel, a canvas of bars — as Kit's
    own rule has it; the app moves to Rust only if no such primitive
    can meet it.
12. **`kui-trace synth`** writes realistic recordings — frames and
    their gaps, nested spans under the core's and the app's origins,
    extensions' fills, inputs and events, the loop asleep and woken,
    burns, counters, gauges, notes and placements — by frames, spans a
    frame and a seed: the spike's recording, a bench's, and the tests'.
13. **Built to be reused.** The inspector is a component,
    `perf/inspector`, whose props take a recording; `kui-perf` is its
    shell — the window, the menus, opening, recent files, preferences.
    A devtools tab mounts the same component later over the live
    recording, and the door that lets an extension read it is that
    tab's only new piece.
14. **What the inspector lacks is fixed where it belongs.** A component
    Kit does not have or draws wrongly, and a door the runner or the
    core does not have, is filed with Kit or kui and built there,
    generically; nothing is worked around inside `tools/kui-perf`.
15. **The open entries it needs are built on the way**, where the
    building order reaches them:
    - **C147, a channel appended to** — `append(rows)` with a cap that
      drops the oldest, in every binding: Live (decision 6) adds the
      records that came to the columns rather than publishing a million
      spans again. Built in `kui-channel` as it splits out.
    - **C148, a channel's filter, sort and take** — row indices by
      equality or substring on a column, an order by a column, a new
      channel of chosen rows, in Rust and every binding: what a table
      sorted by a column and a filter (decision 8) are made of, for any
      app and not only this one.
    - **C149, the line chart's y glide** as a transition rather than a
      frame a frame running the view: the graph lanes (decision 7)
      reframe as Live adds to them.
    - **C102, type-ahead over a channel column** — the tables' rows are
      a channel's, and typing a span's name should land on it with no
      handler.

## Considered options

- **A Rust app with Kit loaded as an extension**: what decision 11
  falls back to. It forgoes being the runner's and Kit's largest user,
  which is half of why `kui-perf` is built on them.
- **Decoding and analysing in Lua**: the analysis would exist twice —
  the Perf tab's in Rust, the inspector's in Lua — and drift; and
  decoding a million records in Lua spends the open budget alone.
- **Handing Lua the columns through a function the host passes**
  (`register(lua, make_channel)`), keeping the channel in `kui-core`:
  works, but leaves `kui-trace`'s Lua side unable to say what it
  makes, and the channel needed a crate of its own anyway.
- **A query language for filters**: a language to design, document
  and keep; the UI's filters cover v1, and Lua is already a query
  language the runner hosts.
- **A `--follow` flag**: a recording is a file that may still be
  written; watching costs nothing idle, and the Live switch says what
  the flag would have.
- **Attaching to a running app over a socket in v1**: the file covers
  every case ADR 0058 named; the transport trait leaves the socket a
  second implementation.
- **A hue setting in Kit's Phosphor**: Kit's settings already take
  roles; the yellow is the app's settings, not a skin.

## Consequences

- `kui-value` and `kui-channel` join the workspace and the release's
  crate list; the bindings and apps see no change, through the
  re-exports.
- `kui-trace` keeps its no-dependency default, and gains the
  stream decoder, the transport, `synth` and an optional `mlua`.
- The runner and Kit gain whatever the inspector finds missing, filed
  as it is found: Kit's waterfall at a million spans, its graph lanes,
  container-size layouts.
- Every app with a channel gains append, filter, sort and take, and a
  line chart that reframes without running its view (C147–C149).
- The devtools' Perf tab stays the look inside the app (ADR 0058,
  decision 13's last paragraph, unchanged) and gains the button that
  hands a recording to the inspector.

## Follow-up, later

- **Two recordings side by side** (`kui-perf a.kuitrace b.kuitrace`,
  `kui-trace diff` drawn) and **a folder of recordings over time** —
  each phase's p95 a run — together in v1.1, since both read several
  files.
- **The query box** (decision 8), if the spike's predicate meets its
  budget.
- **A devtools tab** mounting `perf/inspector` over the live recording,
  with the read door it needs.
- **Attaching to a running app** through a socket transport.

## Building order

1. `kui-value` and `kui-channel` split out, re-exported, every
   binding's tests green; C147's append and C148's filter, sort and
   take in `kui-channel`, in every binding.
2. `kui-trace`: the stream decoder and the transport trait with the
   file transport and its watching; `synth`; the `lua` feature.
3. The spike (decision 11): the bare app, the four budgets and the
   predicate's measure, reported; this ADR accepted or amended.
4. `tools/kui-perf`: the inspector component and its shell — the
   overview, the timeline and its lanes, the detail, the tables, the
   filters, the bands, the keys, the look, the preferences — and the
   Perf tab's *open in kui-perf*; C149 and C102 as the lanes and the
   tables reach them, and what else it finds, filed as found.
5. How it ships, decided with the spike's numbers; the book's devtools
   page and ADR 0058's pointers to it.
