---
status: proposed
date: 2026-10-25
---

# A recording is a timeline of what happened, in frames and between them

> **Proposed (2026-10-25); backlog F198. Not built.** From the user,
> after the devtools' Perf tab (F197), over one conversation: a bar of
> the timeline should open into *why that frame lagged*; an app should
> put its own measurements beside kui's — "instruments for
> integration"; a recording should be kui's own format, from which
> Chrome's trace or any other is an adapter, read by an inspector that
> need not live in the panel; what happens *between* frames should be
> recorded too; the C and Odin bindings should have the waker Rust
> has; and the inspector should be built with the kui tools design
> system, whose trace waterfall it shares with the BFF testing tool. kawoosh built most of the first two for itself — a Frames tab
> (why each frame ran, the runs of frames with no input) and a Perf tab
> (spans by phase, system, pane and plugin, their CPU time, the
> process's memory), some 2,300 lines and a Lua profiler — and every app
> that asks the question would build them again. This decides the
> recording's shape, what kui records on its own and through which
> doors an app adds to it, how it leaves kui, the inspector that reads
> it, and the rule that none of it costs anything until it is asked for.

## Context

What a frame costs and why it ran are spread over five places today:
`Core::stats` (the driver's `FrameSample`: input, view, layout, render,
vsync's wait — pushed by the native runner and Node, by no C, Odin or
headless host); `Core::frame_cause` (why it ran, as kinds: a `wake`
names no system); `Core::owed_by` under `set_frame_trace` (F111: who
held an owed frame — a request by its source line, not what it was
for); `Core::frame_unchanged` (it drew what the one before drew); and
F197's Perf tab, which records those four per frame and draws a bar
each.

What none of them says:

- **Where inside a phase the time went.** A 14 ms view is one number;
  the app's breakdown (kawoosh's `rows`, its `status: lua`, a Lua
  plugin's fill) lives in the app's thread-locals and its own tab.
- **Whether the thread worked or waited.** kawoosh reads the thread's
  CPU clock beside the wall clock: a lap far over its CPU time was a
  lock, a page read back in, the core given to another program.
- **Which frames were a burn**: a run of frames with no input between
  them, kawoosh's ledger's main finding.
- **What happened between frames.** A frame can be fast and still reach
  the screen 40 ms after the key that asked for it — the input waited
  for a loop that was asleep, or busy outside kui, or the handler that
  read it was slow. An app can burn CPU while idle in handlers and
  timers that draw nothing, so no bar shows it. A frame-shaped record
  has nowhere to put any of this: the time is in the gaps.

And two things outside the panel: the tools that already exist —
Perfetto, speedscope, the Firefox Profiler, Tracy, Apple's Instruments —
which a recording should reach without kui knowing each; and a way to
read a recording made somewhere the panel is not — a Node app, a C host,
a headless run on CI, a session too long to watch, or two recordings
side by side, before a change and after.

Last, the waker. C21 gave Rust `Waker` (`App::setup`), ADR 0057 let it
ring a headless run as well as a window; C and Odin have none, so a
C host under `kui_run` whose data arrives on a thread — a socket, a file
watcher, a pty — can only redraw on a timer. Node does not need one
(libuv is its timer, D4, and a Worker cannot hold a native handle), and
Lua is a guest that wakes nothing.

## Decision

1. **Nothing is recorded unless something listens.** *Measuring* is on
   while the Perf tab records or is on show, while `KUI_PERF_TRACE`
   names a file (decision 11), or while a `tracing` subscriber listens
   for target `kui` under the `tracing` feature (decision 12). Off, every
   door below is one branch on a flag kept on the core — no clock read,
   no allocation, no lock — and `Core::perf_measuring()` says which, so
   an app skips reading its own clocks too. The core's records and the
   driver's are behind the same flag. The frame trace (F111) is on while
   measuring and put back as the app had it after, as F197 does.

2. **The recording is a timeline, and kui's own format.** One clock —
   µs since the recording began, monotonic — and records placed on it,
   each on a track: `main` (the thread the core runs on), `loop` (the
   driver's), and one per name given to `perf_measure` (work timed off
   the main thread). Frames are intervals on the timeline, not its unit:
   everything after a frame ends and before the next begins is that next
   frame's *gap*, and what it holds is the next frame's to answer for.
   The records:

   | record | what | from |
   |---|---|---|
   | `session` | format version, kui version, app, platform, refresh rate and budget, scale, the wall time at 0 | core |
   | `name` | an id for a string, once per session (every name below is one) | core |
   | `frame` | begin and end, its number, its causes (kinds and named ones), its holders, unchanged, nodes | core |
   | `present` | a frame drawn: render and vsync's wait | driver |
   | `span` | a name, an origin, a parent, start and length, CPU time, the node it began under | core, app |
   | `input` | a kind and the node it reached, at its arrival — the OS's stamp too where the platform gives one; pointer moves as one record per run with a count | core |
   | `event` | an event handed to the app: kind and node | core |
   | `wake`, `request` | a wake and a frame asked for, with their names | core, driver |
   | `loop` | the driver went to sleep (until when), woke (by what), or handled an OS event that never became input | driver |
   | `counter`, `gauge`, `mark`, `note` | a count, a reading, an instant, a line | core, app |

   No record holds a payload, typed text or a concealed field's
   anything: kinds, keys, labels and times. A recording goes into bug
   reports, which the events stream does not.

3. **`kui-trace` holds the format, and kui writes it.** A crate with no
   dependencies outside `std`: the records as types, a writer that
   appends them, a reader, and a text form (`kui-trace dump`). The bytes
   are `KUITRACE` and a version, then records, each a tag and a length
   before its fields (LEB128 varints, times as deltas on their track),
   so a reader skips a tag it does not know and a file written as it
   records is read up to where it stopped. `kui-core` depends on it for
   the types and the writer; the recording in memory is these records,
   so the Perf tab, the inspector and every adapter read one model and
   cannot drift apart.

4. **What the core records on its own.** Spans under origin `kui`:
   `build` (from `begin_frame`'s end to `finish`'s start — the view, for
   a Rust app, a C host under `kui_run`, a Lua guest; the encoded tree's
   replay for Node), `fill <namespace>` for each extension's fill (a Lua
   plugin's cost with no line of the app's), `devtools` (the panel's own
   build, so its cost is taken out of what it measures), `layout`,
   `emit`. Counters where the work happens: texts shaped (the text
   cache's misses), glyphs rasterized, atlas uploads, nodes. The
   `input`, `event`, `request` and `frame` records. So a C, Odin or
   headless host records view and layout with no `FrameSample` pushed;
   render and vsync are the driver's.

5. **What the drivers record.** The native runner and ADR 0057's
   headless runner: `loop` sleep and wake (and what woke it), `present`,
   OS events that never became input, the named wakes, and a `handler`
   span around each call into the app's `on_event` / `on_event_with`.
   Node's loop: the pump, and a `handler` span around `update`.
   `kui_run` for C and Odin: the same as the native runner, its callback
   timed as `handler`. A host on its own loop records what it knows
   through the doors (decision 13).

6. **Spans.** A name, a start and a length on the timeline, nested by a
   stack the core keeps while measuring; each closed one carries its
   parent, its CPU time (`CLOCK_THREAD_CPUTIME_ID` on unix; none on
   Windows, whose thread times tick every 15.6 ms), its origin (the
   host, an extension's namespace, `kui`) and the key of the node the
   builder had open when it began — so a span begun inside a pane's
   `with_keyed` lights that pane from the panel with no key passed. A
   span opened between frames (a handler, Node's view, which runs before
   its frame begins) sits at its own time in the gap and is the next
   frame's. A span left open when its frame ends is closed there and
   marked so; an end with no begin is nothing. At most 1024 spans a
   frame are kept, the rest counted.

7. **Counters, gauges, marks, measures.** `perf_count(name, n)` adds to
   a counter for the frame; `perf_gauge(name, value)` sets a reading,
   held until set again (a footprint, a buffer's bytes); `perf_mark(name)`
   puts an instant on the timeline (a file opened, a command run);
   `perf_measure(name, ms)` records a span timed elsewhere — a parse on
   another thread, a job — ending as it is reported, on the track named
   for it.

8. **Causes have names.** `request_frame_for(why)` asks for a frame as
   `request_frame` does and names it (`FrameRequest::why`, interned per
   session as `&'static str` so the type stays `Copy`: at most 256
   distinct names, then `"request_frame"` and the warning
   `perf-names-exhausted` once). `Core::note_wake(name)` notes
   `FrameCause::WAKE` with a name, for a driver or a host whose thread
   woke the loop; a named waker calls it (decision 14). The frame's
   causes read `wake: tree-sitter`, `request: caret blink`.

9. **A bar opens into the frame, and the frame says why it lagged.**
   Above F197's detail:
   - **The path from input to screen**: the input's arrival, its
     handler, the frame's begin, its end, its present — each leg's time,
     so "it felt slow" is a leg.
   - **A verdict**, the first that holds: *late* (an input waited more
     than the budget for its frame — and whether the loop slept through
     it or was busy); *blocked* (a gap the driver recorded no sleep in:
     the thread was busy outside kui); *slow* (over the budget: the
     phase and the span with the most self time in it); *waited* (wall
     over CPU time by more than 2 ms and a third); *wasted* (drew what
     the frame before drew: its cause and top holder); *burn* (frame k
     of a run, decision 10); else *fine*, as a share of the budget.
   - **The gap**, drawn: its inputs, handlers, wakes, sleep and marks.
   - **The spans as a flame chart**, a row per depth, the core's in one
     colour and the app's and each extension's in theirs, self time on
     hover, a span with a node lighting it in the app; and the same
     spans as a list, the most self time first.
   - **The frame's counters and gauges.**

10. **The recording is grouped, as kawoosh groups it.** Spans by name
    (calls, average, worst, total, self, on CPU) and by origin (the
    host, each extension, `kui`); counters (average, most); gauges
    (last, least, most); the gaps (time in handlers, asleep, and busy
    outside kui); and **burns** — 30 frames or more, each within 100 ms
    of the one before, no input among them — each with its length, the
    input before it, its causes and holders counted, opening its first
    frame; the timeline marks a burn with a band under its bars.

11. **A recording leaves kui as a file.** `Core::perf_trace()` returns
    the last recording's bytes; the Perf tab's *save trace* writes a
    `.kuitrace` beside the screenshots (`KUI_CAPTURE_DIR`, the desktop,
    the temporary directory) and notes the path. `KUI_PERF_TRACE=path`
    records from the first frame and *streams* to `path` as it goes —
    the file is the ask, so it is not a ring — closed when the app exits
    or `Core::perf_flush()` is called; the native runner, the headless
    runner, `kui_run` and Node's loop read it. kawoosh's
    `KAWOOSH_PERF_LOG` and `KAWOOSH_FRAME_LOG`, as one file.

12. **Adapters and a diff, in `kui-trace`.** `kui-trace convert IN
    --to chrome|perfetto|speedscope|firefox OUT`, and the same as a
    library: Chrome's trace-event JSON (`chrome://tracing`, Perfetto's
    viewer), Perfetto's protobuf (its tracks and counters natively, the
    encoder written for the few messages it needs), speedscope's, and the
    Firefox Profiler's. An adapter maps what its format has and drops
    what it has not (a holder becomes an annotation, a verdict nothing).
    `kui-trace diff A B` compares two recordings — each span's and
    phase's spread, burns, gaps, counters — and with `--fail-over 10%`
    exits non-zero past the threshold, so a CI job holds a scripted
    session's cost as `bench-check.nu` holds a bench's. And live, beside
    the files: under `kui-core`'s `tracing` feature (off by default;
    `tracing` is in the dependency tree already, through `winit` and
    `zbus`) every span enters a `tracing` span named `kui` with its name,
    origin and node as fields, and counters go as events, while a
    subscriber listens — the door to Tracy (`tracing-tracy`), to
    Instruments (`tracing-oslog`, os_signposts) and to Perfetto live.

13. **`kui-perf`, the inspector, is a tool built on the kui tools
    design system.** It opens a `.kuitrace`: the whole timeline, zoomed
    and panned across frames and gaps, its tracks, the flame chart, the
    grouped tables, the burns, and two recordings side by side
    (`kui-perf a.kuitrace b.kuitrace`, the diff of decision 12 drawn).
    - **Its parts are the design system's** (the Lua/Teal add-on every
      host loads, Ledger's look): the frame and span tables, tabs,
      toolbar, resizable panes, key-value detail, stats and status bar
      are its v1 components, and the timeline and flame chart are its
      **trace waterfall** — a composite that design system lists for
      after v1 and that `kui-perf` and the BFF testing tool's request
      inspector both need, so it comes early. Its input is
      source-agnostic — a name, a start, a length, a depth, a group, and
      what a click selects — so `kui-trace`'s spans map onto it one to
      one, and an HTTP exchange's phases do too.
    - **It ships twice**: an app that opens files, and an extension that
      mounts as a devtools tab (ADR 0032) in any app that loads it,
      reading the live recording. The read door that tab needs — an
      extension seeing the session's recording — is decided when the
      tab is built.
    - **What it asks of kui** (a canvas of thousands of bars, zoom,
      virtual lists of frames) and of the design system is filed with
      whichever answers it, as kawoosh's reports are.

    The Perf tab stays the look inside the app — the recent frames,
    record and stop, a frame opened — drawn by the core with no
    dependency (ADR 0024: the panel is the core's, in every binding, a
    kiosk build and a C host among them), and hands a long or a shared
    recording to the inspector by saving it. Attaching to a running app
    over a socket is left for when a file is not enough.

14. **C and Odin get a waker.** `KuiWaker *kui_waker(KuiCtx *)` under
    `kui_run` or a headless run — the runner's `Waker` (ADR 0057),
    reference-counted, `Send` — and `NULL` for a context on a host's own
    loop, which has its own way to wake and `kui_note_wake` to name it.
    `kui_waker_wake(w)`, `kui_waker_wake_at(w, ms)`,
    `kui_waker_wake_named(w, name)` (also notes the name, decision 8),
    `kui_waker_clone(w)`, `kui_waker_free(w)`; any thread may call them.
    Odin's are generated from them. Rust's `Waker` gains `named(name)`,
    a waker whose wakes carry the name. Node gets `noteWake(name)` and no
    handle: its completions run on libuv between pumps, and that is
    where it names them. Lua none.

15. **The doors** (rows of `schema::DOORS`):

    | Rust | C | Odin | Node | Lua |
    |---|---|---|---|---|
    | `Ui::span(name, \|ui\| …)` | — | — | `ctx.span(name, fn)` | `env.span(name, fn)` |
    | `Core::span_begin` / `span_end` | `kui_span_begin` / `kui_span_end` | `span_begin` / `span_end` | `spanBegin` / `spanEnd` | the closure form |
    | `Core::perf_measure` | `kui_perf_measure` | `perf_measure` | `perfMeasure` | `env.perf_measure` |
    | `Core::perf_count` | `kui_perf_count` | `perf_count` | `perfCount` | `env.perf_count` |
    | `Core::perf_gauge` | `kui_perf_gauge` | `perf_gauge` | `perfGauge` | `env.perf_gauge` |
    | `Core::perf_mark` | `kui_perf_mark` | `perf_mark` | `perfMark` | `env.perf_mark` |
    | `Core::perf_measuring` | `kui_perf_measuring` | `perf_measuring` | `perfMeasuring` | `env.perf_measuring` |
    | `Ui::request_frame_for` | `kui_request_frame_for` | `request_frame_for` | `requestFrameFor` | `env.request_frame_for` |
    | `Core::note_wake` | `kui_note_wake` | `note_wake` | `noteWake` | no: a guest wakes nothing |
    | `Waker::named` | `kui_waker`, `kui_waker_wake`, `_wake_at`, `_wake_named`, `_clone`, `_free` | `waker`, … | no: libuv wakes it (D4) | no |
    | `Core::set_perf_recording` / `perf_recording` | `kui_set_perf_recording` / `kui_perf_recording` | as C | `setPerfRecording` / `perfRecording` | no: the host's |
    | `Core::perf_trace` / `perf_flush` | `kui_perf_trace` (borrowed bytes) / `kui_perf_flush` | as C | `perfTrace` (a `Buffer`) / `perfFlush` | no: the host's |

    `span_begin` returns a `SpanId` (`0` while not measuring, which
    `span_end` ignores), so a span crosses a callback in C and Node with
    no closure. Node's `ctx.span` times in Rust, so it nests with the
    core's. Names are interned for the session (decision 8's table).

## Considered options

- **Frames as the format's unit** (F197's model, kawoosh's): what a
  frame holds is easy to draw, and the gaps — a late input, a busy
  thread, a handler's cost — have nowhere to go, so the format would
  change the day someone asks why a fast frame felt slow. Declined:
  frames are intervals on the timeline.
- **Chrome's trace JSON as the format.** Every viewer reads it — and it
  is text a frame's worth at a time, slow to stream, with no place for a
  cause, a holder or "drew nothing new" except as free-form arguments
  the panel would parse back. Declined as the format, kept as an adapter.
- **Perfetto's protobuf as the format.** Tracks and counters natively
  and a fine viewer — and a large schema to carry in a crate with no
  dependencies, for semantics kui would still add as its own fields.
  Declined as the format, kept as an adapter.
- **Leave it to apps** (kawoosh's thread-locals and two tabs): every app
  pays again, and none can see the core's half — the fills, the layout,
  the panel, the text shaped, the loop. Declined.
- **Only `tracing`**: Rust only, a subscriber per binding, nothing for
  the panel to draw. Kept beside the format (decision 12).
- **The inspector in the panel only.** The docked pane is 280 px at its
  narrowest and lives in the app it measures: a five-minute recording, a
  recording from CI or a Node app, two side by side, do not fit it.
  Declined; the panel keeps the quick look.
- **The inspector's timeline its own, not the design system's.** Quicker
  to start, and the BFF tool's inspector would draw a second waterfall
  beside it. Declined: one composite, two tools.
- **The devtools panel on the design system.** One look everywhere — and
  a debugging tool that loads the add-on it might be debugging, missing
  from every host that cannot load a `.lua` extension or chose not to.
  Declined; the panel borrows the look, not the code.
- **os_signpost from `kui-native` directly**: a signpost's name and
  format must be literals in the image; `tracing-oslog` does the
  encoding behind the feature.
- **Measure always, cheaply**: a clock read is ~20 ns and a frame has a
  few dozen spans — and the rule for F197 was that nothing regresses
  until it is used. Declined.
- **A sampling profiler in kui**: Instruments' Time Profiler, `samply`
  and `perf` do it with symbols and better; the trace lines up with them
  by the clock.
- **A waker for Node and Lua.** Node's completions are libuv's and a
  Worker cannot hold a native handle; its need is the name, not the
  wake. Lua is a guest. Declined; `noteWake` for Node.

## Consequences

- **A lag is read off the bar**: the path says which leg, the verdict
  which kind, the flame chart where, the node lights up; the gap shows
  what the frames never did.
- **Every host records**: C, Odin and headless hosts get view and layout
  from the core and the loop from `kui_run` or the headless runner.
- **Two new crates**: `kui-trace` (published, so a tool reads and writes
  the format without kui) and `kui-perf` (an app, published as a binary
  to `cargo install`). Both go into `publish-crates.nu`, and crates.io's
  limit on new crates in a burst applies to the first release that
  carries them.
- **The order it lands in.** First, with no design system needed: the
  format and `kui-trace` (decision 3), what the core and the drivers
  record (4–8), the doors (15), the waker (14), the Perf tab's verdict,
  path, gap, flame chart and tables (9–10), the files out and the
  adapters and diff (11–12). Then `kui-perf`, once the design system's
  v1, its trace waterfall and the core changes it rests on (a `.lua`
  extension in every host, a slot that carries the host's children,
  cheap instances) are built — the inspector is the first heavy user of
  all three.
- **The panel and the tools look like one family**, without the panel
  depending on the design system: once Ledger's look settles, the
  devtools take its spacing, type scale, monospace labels and rules as
  values of the core's own, in one restyle pass.
- **What an app can delete**, kawoosh first: its span and lap
  thread-locals and the Perf tab that reads them, its frame ledger,
  burn detection and Frames tab, `frames::request` (now
  `request_frame_for`), its named wakes' ledger (now `Waker::named`),
  and its two log files (now `KUI_PERF_TRACE`). It keeps what only it
  knows — its process's memory and its buffers — as gauges. The port is
  kawoosh's, after this is built, in its own session.
- **Costs.** Off: a branch per door. On: two clock reads and a record a
  span, a record an input (a pointer move adds to its run's count), a
  record per handler and per loop turn; the ring in memory, or the
  stream to a file. The panel's own build is a span, so its cost shows
  and is left out of the verdict.
- **Breaks.** None for an app that does not use it. `FrameRequest`
  keeps its shape (decision 8). The Perf tab grows sections; a declared
  tab named `perf` keeps its name (F197).
- **Bindings.** Twelve rows in `DOORS`, pinned by each binding's test;
  `kui.h` gains the C doors and `KuiWaker` (new functions, no struct
  moved: no ABI bump), and the generated Odin package follows.
- **Not decided here**: a span on another thread with its own timeline
  (a worker's track — `perf_measure` reports its result for now), GPU
  timestamps for the render phase (wgpu's timestamp queries, where the
  adapter has them), the process's memory read by kui itself
  (kawoosh reads footprint and resident per platform, and kui may adopt
  it as built-in gauges), and the inspector attaching live.
