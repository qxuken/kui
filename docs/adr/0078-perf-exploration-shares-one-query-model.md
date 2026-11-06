---
status: accepted
date: 2026-11-06
---

# Perf exploration shares one query model between the viewer and terminal

Build a great perf exploration tool, with agent accessibility as an
important additional use. The viewer and terminal share the same analysis,
selection, filters and references, so an investigation can move between
them without changing what its measurements mean.

This design was accepted after an interview on 2026-11-06. It extends
[ADR 0075](0075-kui-perf-reads-a-recording-in-kit.md).

Implemented on 2026-11-06: the shared Rust query module, CLI operations,
viewer readings and frame tables over the same scope, stable detail
references, independently scoped comparison and focused viewer startup.
The [tool documentation](../../tools/kui-perf/README.md) describes the
implemented interface. Scope metadata reports a compact effective extent and
interval count, keeping a filtered population's provenance bounded even
when many disjoint intervals are selected.

## Context

Windowless analysis already works. The separate `kui-trace` binary offers
`stats`, `dump`, `diff`, `convert` and `synth`; a Lua script can read a
recording through `kui.trace` on the headless runner. The
[spike analyzer](../../tools/kui-perf/spike/analyze/main.lua) does this.
`kui perf RECORDING --headless` runs the inspector without a window, but
does not provide a query interface.

The missing capability is a discoverable exploration interface: get an
overview, find expensive frames or span groups, inspect their evidence,
compare recordings, and continue in the viewer. People should get useful
terminal output, and agents should get the same results as structured data
without writing a script or starting a server.

`kui-trace` already owns decoding and analysis. However, its rich recording
facade, frame/span details and filtering live partly in `src/lua.rs` and
produce Lua tables. Current detail lookups use filtered row positions;
`rec:stats` ignores that filter and silently caps span groups at 40. The
native `stats` command has another fixed top list. Those conventions are
insufficient for a shared exploration interface.

## Decision

### One shared analysis module

Put the query module in `kui-trace`, with ordinary Rust request and result
types. The CLI and the viewer's Lua interface consume that module. Parsing,
selection, filtering, aggregation, sorting and result limits have one
implementation. Presentation remains in the terminal formatter or Kit
viewer. The query module must be usable without initializing a window or
running the inspector app.

The viewer and CLI use the same operation names and measurement semantics.
Their layouts and interaction patterns can differ: a timeline does not
become a twenty-row display merely because a terminal query defaults to
twenty results. Both must make any limits on their results explicit.

### Saved recordings and named operations first

Use one-shot `kui perf` commands. Preserve `kui perf RECORDING` as the
viewer entry point. Named operations cover summary, frame lists and
details, span groups and individual span details, counters and marks,
plus comparison of two recordings. Filters, sorting and limits compose
with those operations. Existing Lua scripts remain the escape hatch for
custom analysis; no new query language is introduced.

The following examples describe the target interface:

```sh
kui perf recording.kuitrace
kui perf summary recording.kuitrace
kui perf frames recording.kuitrace --sort work --limit 20
kui perf frame recording.kuitrace 123
kui perf spans recording.kuitrace --frames 120:180 --sort self
kui perf spans recording.kuitrace --frames 120:180 --name layout --json
kui perf span recording.kuitrace 456
kui perf counters recording.kuitrace
kui perf marks recording.kuitrace
kui perf compare before.kuitrace after.kuitrace
kui perf compare before.kuitrace after.kuitrace \
  --before-frames 120:180 --after-frames 240:300 --json
```

Frame and time ranges are available wherever an operation measures a
scope. Time selectors state their units; their exact flag grammar is an
implementation detail to document alongside the CLI. Comparison accepts
independent scopes for its two inputs rather than assuming matching frame
numbers or elapsed times.

### Scope precedes span filtering

The whole recording is the default measurement scope. A frame range names
recorded frame numbers, not positions in a filtered list. A time range
resolves to complete intersecting frames. Results report both the
requested selection and the effective frame/time range used; they must
not imply that a sub-frame selection was measured precisely. Frame-based
analysis preserves its existing frame/gap accounting rather than cutting
frames or nested costs into arbitrary fragments.

A span filter narrows attribution within the scope. It does not redefine
frame statistics or their denominators. For example, selecting 100 frames
and filtering to `layout` reports layout costs over those 100 frames;
frame pacing still describes those 100 frames. Span self time is computed
from the original nesting, before a name or origin filter hides children.
Otherwise hiding a child would falsely turn its time into its parent's
self time.

Frame filters, such as verdict, explicitly select a frame population and
report the resulting frame count. They are distinguished from span
filters in the shared requests and results. Active filters, including any
default origin exclusions, are visible in both presentations.

Marks are discoverable as named instants. Repeated names are legitimate;
v1 does not infer regions, pair start/end marks, or invent a workload
identity from their names.

### Stable references and visual continuation

Use the recorded frame number and the recorded span ID, qualified by
their recording. A sort, filter, limit or viewer row position must not
change their meaning. Expose span parent IDs so details can lead through
the original nesting. IDs are local to a recording, not identifiers that
match entities between two runs.

A terminal investigation can open the viewer focused on the same frame,
span or scope:

```sh
kui perf recording.kuitrace --frame 123
kui perf recording.kuitrace --span 456
kui perf recording.kuitrace --frames 120:180
```

The focused item and its surrounding timeline are visible on arrival.
Scope-based continuation uses the same selection resolver as the query,
including time-range expansion.

### Useful text and structured results

Human-readable output is the default. Every query command supports
`--json`, representing the same measurements, scope, filters and result
limits rather than a separate analysis path. Machine output must not
require parsing a formatted table. Times and other quantities have
explicit units.

List queries return 20 results by default, allow an explicit limit and an
option to request all matches, and report the matched count, returned
count and whether results were omitted. Sorting is explicit and
deterministic, including tie handling. Detail queries return the selected
item in full. An explicit request for all results must not meet a hidden
cap.

Span group results expose both self and inclusive time and rank by self
cost by default. Individual spans retain stable references for further
inspection. A grouped result is not presented as if its aggregate were
one recorded span.

Results carry enough context to reproduce a reading: the recording,
requested and effective scope, relevant counts, active filters, sort and
limits. Session metadata and known data limitations remain visible.
Unavailable metrics are represented as unavailable, not zero. In
particular, a reader cannot certify that no events were dropped: the
current file format does not encode the recorder's dropped-event count.

### Comparison presents evidence

Comparison is in v1. It reports each input's scope, duration and frame
count alongside absolute measurements, deltas and normalized costs.
Frame pacing and cost per frame are shown together so a recording with
more frames or more startup work cannot quietly look like a regression.
Use the recording's actual refresh/budget metadata where available.

Span groups can be matched by name and origin, as in the existing diff;
recorded IDs do not match across recordings. Missing measurements remain
unavailable. Percentage changes must not disguise a missing or zero
baseline. Available session context is shown without claiming that the
two workloads are equivalent.

The command makes no overall faster/slower verdict and adds no automatic
pass/fail policy. The existing `kui-trace diff --fail-over` is a separate
existing policy, not an implicit default for `kui perf compare`.

## Consequences and alternatives

- A CLI-specific analysis path would be quicker to add, but would preserve
  discrepancies in selection, attribution and limits. Extracting the
  shared Rust results is part of this work, not a later cleanup.
- Wrapping the existing Lua facade would retain filtered-row references,
  Lua table coupling and the current statistics/filter mismatch. Keep Lua
  as a caller of the shared module and as the advanced scripting option.
- One-shot commands suit terminal exploration and agent tool calls without
  a process lifecycle or server protocol. A persistent process or MCP
  adapter can be considered when a concrete workflow needs one.
- Expanding time selections to whole frames sacrifices sub-frame precision
  in exchange for intact frame measurements and explainable nested costs.
  Showing the effective scope is essential to that trade-off.
- Comparison moves into this exploration increment from ADR 0075's later
  work. A side-by-side comparison layout in the viewer is not required to
  ship the shared comparison operation and terminal results.
- Existing recording capture, viewer file watching and Lua scripting
  continue to work. This increment adds no live capture/control protocol.

## Building order and acceptance

1. Extract the shared query requests/results and selection resolver in
   `kui-trace`. Pin scope, filter, attribution and identity semantics
   through that interface, including equal-sort ties and boundary frames.
2. Move the viewer's relevant readings onto it. Preserve the existing
   timeline interaction and large-recording behavior while replacing row
   references and silent caps at the shared interface.
3. Add the one-shot commands, text formatting and JSON output. Expose the
   operations, filters, units, sorting and limits through CLI help.
4. Add independently scoped comparison and focused viewer startup.

The increment is complete when these investigations work:

- A person or agent gets a summary, ranks expensive frames, inspects one,
  follows its spans, and opens that exact frame or span in the viewer.
- The viewer and CLI return the same measurements for the same scope and
  filters; sorting or filtering cannot change what a detail reference names.
- A time selection intersecting part of a frame shows its expanded scope.
  Filtering to one span name preserves the frame population and self-time
  attribution. Empty selections do not silently fall back to the whole file.
- More than twenty matches produce visible omission counts; requesting all
  returns all matches. JSON and text agree on measurements and completeness.
- Two differently sized selections can be compared with their counts,
  absolute values and normalized costs visible, without an overall winner
  or missing measurements turned into zero.
- Headless queries exit without creating a window or needing an inspector
  event loop. Existing viewer panning and table behavior remain usable on
  large recordings.

Live session control, a persistent server/MCP interface, named-marker
regions, exact sub-frame accounting, a new query language and automatic
regression verdicts are outside this increment. A folder-of-recordings
history view remains later work from ADR 0075.
