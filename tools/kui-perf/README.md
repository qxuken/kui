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
- **The timeline**: Kit's waterfall over the spans (their lanes read a window
  at a time in Rust, so a million spans pan as a hundred do), marks for input,
  events, notes and the components drawn fresh; counters and gauges as graph
  lanes on the same window.
- **The frame or span selected**: its verdict, phases, input path, holders,
  components drawn fresh and why, notes and counters.
- **Tables over the window**: frames, spans, origins, components, counters and
  gauges, burns, gaps — read once the window is still.
- **Filters**: a span name, a shortest span, only slow frames or no unchanged
  ones, origins; the devtools' own spans off unless asked for.
- **Live**: the file is watched; what it gains comes in, the window riding the
  end until you move it (↦ latest takes it back). Off, a refresh button counts
  what waits.
- **Bands** by the inspector's own width (container queries): the detail
  beside the tables at 1200 px and over, one pane of tabs under that, and under
  700 px the timeline the height with a short pane.

Keys: ← → or `h` `l` the frame before and after, ↑ ↓ or `j` `k` too; `[` `]`
the slow frame, `{` `}` the burn; `w` `s` zoom, `a` `d` pan; `f` fit the
selection, `0` the whole recording; `/` the span search; Esc clears. ⌘O
(Ctrl+O) opens. Preferences — splits, lanes per app, filters, recent files —
are kept in the app's data folder.

`state.tl` is the model and messages, `inspector.tl` everything about one
recording (what a devtools tab will mount), `main.tl` the shell. `test/` drives
it headless: `kui run tools/kui-perf/test --headless -- part.kuitrace rest.bin`.
