# kui-perf

kui's trace inspector ([ADR 0075](../../docs/adr/0075-kui-perf-reads-a-recording-in-kit.md)):
a Teal app on the `kui` runner, built from Kit, that opens a `.kuitrace` — from
the devtools' Perf tab, `KUI_PERF_TRACE`, or `kui-trace synth` — whole, and
follows it as it grows.

```sh
kui perf session.kuitrace            # the runner carries it
kui run tools/kui-perf -- session.kuitrace
```

- **The frames** as an overview a frame a sample, the window the timeline shows
  marked on it (drag there to move it).
- **The timeline**: Kit's waterfall over the spans, as tall as its lanes up to
  twelve (their lanes read a window
  at a time in Rust, so a million spans pan as a hundred do), marks for input,
  events, notes and the components drawn fresh; counters and gauges as graph
  lanes under it taking the rest, their plots starting where its lanes do. The wheel
  or a pinch — over the waterfall, a graph lane or the overview — zooms about the
  pointer, a sideways swipe or ⇧ and the wheel pans,
  a drag on the lanes pans, and a drag along the time axis selects a range —
  its length beside the title, zoomed to by its button or `z`.
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
