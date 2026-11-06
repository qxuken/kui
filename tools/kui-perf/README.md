# kui-perf

kui's trace inspector ([ADR 0075](../../docs/adr/0075-kui-perf-reads-a-recording-in-kit.md)):
a Teal app on the `kui` runner, built from Kit, that opens a `.kuitrace` — from
the devtools' Perf tab, `KUI_PERF_TRACE`, or `kui-trace synth` — whole, and
follows it as it grows.

```sh
kui perf session.kuitrace            # the runner carries it
kui run tools/kui-perf -- session.kuitrace
```

The design is [shared perf exploration in the viewer and
terminal](../../docs/adr/0078-perf-exploration-shares-one-query-model.md):
named queries, text/JSON results, stable frame/span references, scoped
comparisons and focused viewer startup.

## Explore from the terminal

Queries run once without a window or inspector event loop. Text is the
default; `--json` returns the same readings as structured data. Start with
a summary, rank frames or span groups, then inspect a recorded ID:

```sh
kui perf summary session.kuitrace
kui perf frames session.kuitrace --sort work --limit 10 --json
kui perf frame session.kuitrace 123
kui perf spans session.kuitrace --frames 120:180 --name layout --all
kui perf span session.kuitrace 456
kui perf counters session.kuitrace       # counters and gauges
kui perf marks session.kuitrace          # named instants, including repeated names
kui perf compare before.kuitrace after.kuitrace \
  --before-time 1s:2s --after-time 3s:4s --all --json
kui perf session.kuitrace --frame 123    # continue visually
kui perf session.kuitrace --span 456
kui perf session.kuitrace --frames 120:180
kui perf session.kuitrace --time 1s:2s
kui perf --help
```

| Query | Result |
| --- | --- |
| `summary FILE` | Complete overview, frame-work distributions and data limitations. |
| `frames FILE` | Ranked frames with recorded frame numbers, work, phases and verdicts. |
| `frame FILE FRAME_ID` | Complete frame detail, independent of active filters. |
| `spans FILE` | Span groups by name and origin, with self/inclusive costs and an example ID. |
| `span FILE SPAN_ID` | Complete recorded span, its owner, parent and children. |
| `counters FILE` | Counters and gauges over the selected scope. |
| `marks FILE` | Named instants; repeated names remain separate events. |
| `compare BEFORE AFTER` | Overview and span-group metrics for two independently scoped inputs. |

`FILE` is an absolute or relative `.kuitrace` path. A query reads the file
as it stands and exits; it does not wait for new records. To open a file
whose name is a query word, prefix its path, for example `kui perf ./frames`.

Frame numbers and span IDs belong to their recording; they do not change
under filtering or sorting. Span groups provide an `example_id` for
drilling into an actual span; span detail includes `parent` and `children`
IDs. The viewer opens with that item visible, overriding saved filters
for the initial focus. An explicit startup scope stays the measured scope
until a pan or zoom replaces it.

The whole recording is the default scope. `--frames FIRST:LAST` is
inclusive and uses recorded frame numbers. `--time FROM:TO` is half-open;
both endpoints carry `us`, `ms` or `s`. Time selections expand to complete
intersecting frames and their preceding gaps. Results report the requested
selection, effective extent, selected frame count, interval count and
accounted duration. A frame filter can select a noncontiguous population;
its accounted duration excludes omitted intervals while `elapsed` spans
the outer extent. An empty selection stays empty.

`--name`, `--origin`, `--hide-name` and `--min-us` restrict span attribution
in `spans` and `compare`. They preserve frame statistics, denominators and
self costs from the original nesting. Devtools spans are excluded by
default, as in the viewer; `--include-devtools` includes them. Frame
filters (`--verdict`, `--hide-unchanged`) change the frame population.
All active filters are reported.

| Option | Applies to | Meaning |
| --- | --- | --- |
| `--frames FIRST:LAST` or `--time FROM:TO` | Summary and lists | Choose one selection; default is the whole recording. |
| `--verdict KINDS` | Summary, lists and comparison | Comma-separated `late,blocked,slow,waited,wasted,burn,fine`. |
| `--hide-unchanged` | Summary, lists and comparison | Omit frames explicitly recorded as unchanged. |
| `--name TEXT` | Spans and comparison | Case-insensitive substring, including component fill names. |
| `--origin ORIGINS` | Spans and comparison | Comma-separated `kui`, `host` or extension names; repeatable. |
| `--hide-name NAME` | Spans and comparison | Exclude an exact recorded name; repeatable. |
| `--min-us N` | Spans and comparison | Minimum inclusive span duration in microseconds. |
| `--include-devtools` | Spans and comparison | Include the devtools spans hidden by default. |
| `--sort FIELD` | Lists | Frames: `work`, `frame`, `start`; spans: `self`, `total`, `worst`, `calls`, `name`; marks: `start`, `name`; counters: `name`. |
| `--asc` or `--desc` | Lists | Costs default descending; names, timestamps and frame numbers ascending. Ties are deterministic: frames use recorded numbers, span groups use name and origin. |
| `--limit N` or `--all` | Lists and comparison | Default 20 rows; `--all` removes the limit. |
| `--json` | Every query | Structured output instead of text. |

Comparison uses `--before-frames` or `--before-time` for its first input,
and `--after-frames` or `--after-time` for its second. An unspecified side
uses its whole recording. Frame and span filters apply to both inputs.
Comparison has a fixed overview-first metric order. Detail queries accept
only the file, recorded ID and optional `--json`; summaries are complete
and take no sorting or result limit.

Span ownership is half-open at the core frame's end: work starting exactly
there belongs to the following frame's gap. Attribution respects the
selected frame population even when present intervals overlap another
frame's gap. Whole-recording queries also retain unframed work.

Lists default to 20 results. `--limit N` changes that and `--all` removes
the limit; matched, returned and omitted counts are explicit. Span groups
rank by self cost by default and also expose inclusive cost. Comparison
shows absolute measurements, deltas and normalized costs, with overview
metrics first. It makes no overall winner or pass/fail judgment. Missing
measurements and percentages without a nonzero baseline stay unavailable.
Total frame work requires both a core end and a present measurement.
It is core duration plus rendering time; vsync wait is reported separately.
`complete_frames` and work distributions count those measured frames;
known core/phase timestamps remain available without a present, while
total work, its verdict and unsupported comparison deltas stay unavailable.
Missing work sorts after measured values in either direction.

JSON envelopes carry `schema_version: 1`, `operation`, `recording`,
`session`, `time_unit: "ms"`, `scope`, `filters`, sorting and limit metadata,
counts, limitations and `data`. A comparison includes separate `before`
and `after` envelopes. Times are milliseconds except explicitly named
quantities such as `seconds` and session `refresh_hz`; unavailable values
are `null` (`—` in text). IDs are recording-local integers; frame numbers
beyond the signed 64-bit range are decimal strings to preserve their value.
Diagnostics go to stderr on failure, with a nonzero exit code.
Trailing incomplete records are reported when detected; the format cannot
certify that no events were dropped.

For scripts, check the exit status, parse JSON and inspect `schema_version`,
`scope`, `matched`, `returned`, `omitted` and `limitations` before interpreting
`data` (comparison carries scope and limitations in `before` and `after`). Keep an
`example_id` with its recording path to request `span` detail or open the
viewer. IDs from different recordings do not identify the same work;
comparison matches span groups by name and origin.

Lua callers use the same Rust query module:

```lua
local rec = kui.trace.open("session.kuitrace", { watch = false })
rec:filter { hide_names = { "devtools" }, name = "layout" }
local result = rec:query("spans", { first_frame = 120, last_frame = 180, all = true })
local frame = rec:frame_id(123)
local span = rec:span_id(456)
local tables = rec:readings(1000, 2000) -- ms; complete tables, active filters
```

`rec:query` also accepts `from_ms`/`to_ms`, `sort`, `descending`, `limit`,
and `id` for a `frame` or `span` operation. Optional unavailable fields are
Lua `nil`. Legacy `rec:frame(row)` and `rec:span(row)` remain row adapters;
use the ID methods when retaining a reference. Span detail's `frame` is
the recorded frame number, resolved with `rec:frame_id`, not a table row.
`rec:stats(first, last)`
keeps its original timeline-index arguments, now applies active filters,
orders span groups by self cost and returns all groups without the old
40-group cap.

The low-level numeric `rec.frames` channels use NaN for unavailable work
and phase measurements. Kit displays these cells as `—`, skips them in
plots and sorts them last. Use `rec:query` for ordinary nullable values.

## The viewer

- **The frames** as an overview a frame a sample, the window the timeline shows
  marked on it (drag there to move it).
- **The timeline**: Kit's waterfall over the spans, as tall as its lanes up to
  twelve (their lanes read a window
  at a time in Rust, so a million spans pan as a hundred do), marks for input,
  events, notes and the components drawn fresh; counters and gauges as graph
  lanes under it taking the rest, their plots starting where its lanes do. The wheel
  or a pinch — over the waterfall, a graph lane or the overview — zooms about the
  pointer, a sideways swipe or ⇧ and the wheel pans,
  a drag anywhere on it pans — over a span too, which a click picks — and a drag
  along the time axis selects a range, its length beside the title, zoomed to by
  its button or `z`; a double click on a span, a frame's row or a burn's makes its
  time the range.
- **The frame or span selected**: its verdict, phases, input path, holders,
  components drawn fresh and why, notes and counters.
- **Tables over the window**: frames, spans, origins, components, counters and
  gauges, burns, gaps — read once the window is still; a header sorts its table
  (a number's greatest first, again the other way).
- **Filters**: a span name, a shortest span, only slow frames or no unchanged
  ones, origins; the devtools' own spans off unless asked for.
- **Live**: the file is watched; what it gains comes in, the window riding the
  end until you move it (↦ latest takes it back). Off, a refresh button counts
  what waits.
- **The dock** under the timeline, its rule dragged (neither under 140 px); in
  it, by its own width (a container query), the detail beside the tables at
  1180 px and over, one pane of tabs under that; under 700 px no lane picker.

Keys: ← → or `h` `l` the frame before and after; ↓ `j` the selected span's
first child (or the top lane's first span in the window), ↑ `k` its parent; `[` `]`
the slow frame, `{` `}` the burn; `w` `s` zoom, `a` `d` pan; `f` or `z` zoom to
the range, or fit the selection; `0` the whole recording; `/` the span search;
Esc clears the range, then the selection. ⌘O (Ctrl+O) opens, and `?` (Help ▸
Keys and gestures, the ⓘ in the toolbar) shows them all on a card — open by
itself the first time a recording opens. Preferences — splits, the dock, lanes
per app, filters, recent files, the card seen — are kept in the app's data
folder.

`state.tl` is the model and messages, `inspector.tl` everything about one
recording (what a devtools tab will mount), `main.tl` the shell. `test/` drives
it headless, its preferences in a folder of its own under `$TMPDIR`:
`kui run tools/kui-perf/test --headless -- part.kuitrace rest.bin`.
