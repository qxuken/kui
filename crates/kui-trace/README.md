# kui-trace

kui's recording format: a timeline of what happened in a kui app, in
frames and between them — why each frame ran and who held it, nested
spans with their CPU time, inputs, events handed to the app, wakes, the
driver's loop asleep and awake, counters, gauges and marks — with its
reader and writer, the analysis the devtools' Perf tab and `kui-perf`
share, and adapters out to Chrome's trace viewer, Perfetto, speedscope
and the Firefox Profiler.

`kui-core` writes it: the devtools' Perf tab records it, and
`KUI_PERF_TRACE=path` streams a session to a file. This crate reads it
without kui: it has no dependencies but behind its `lua` feature, which
hands a script a recording as channels (`kui-channel`, `kui-value`) —
the runner's `kui.trace`. A recording is read whole or as it arrives
(`Stream`, a `Transport` such as `FileTransport`, a `Watch` on the file).

```sh
kui-trace dump session.kuitrace                    # every record, a line each
kui-trace stats session.kuitrace                   # frames, work, burns, top spans, gaps
kui-trace convert session.kuitrace --to perfetto session.pftrace
kui-trace convert session.kuitrace session.json    # the format from the extension
kui-trace diff before.kuitrace after.kuitrace --fail-over 10%
kui-trace synth out.kuitrace                       # a made-up session, for a viewer's tests
```

`convert` writes:

| `--to` | extension | opens in | what it holds |
|---|---|---|---|
| `chrome` | `.json` | `chrome://tracing`, [Perfetto's UI](https://ui.perfetto.dev) | `X` slices a thread a track, `i` instants, `C` counters |
| `perfetto` | `.pftrace`, `.perfetto-trace` | Perfetto's UI, `trace_processor` | a track a thread, slices as begin/end track events, counter tracks |
| `speedscope` | `.speedscope.json` | [speedscope](https://www.speedscope.app) | an evented profile a track with slices |
| `firefox` | `.firefox.json` | [the Firefox Profiler](https://profiler.firefox.com) | a Gecko profile of markers, a thread a track |
| `kuitrace` | anything else | `kui-trace`, `kui-perf` | everything |

Every adapter draws the same rows: `frames` (each frame, then its
present's render and vsync wait), `main`, `loop` (asleep from sleep to
waking) and one per `perf_measure` track. A frame's causes and holders
become its arguments; a verdict, a burn or a phase is not written.
speedscope has no instants or counters, so it drops them.

`diff` compares what a frame costs — each phase's average, p95 and
maximum, frame work, each span's calls and µs a frame (matched by name
and origin, not id), the gaps, the counters — and `--fail-over N%` exits 1
when a counted row grew past N percent. Maxima, counts, shares and vsync's
wait are shown but not counted, nor a time under 50 µs or a rate under one
a frame before: those are noise, not regressions.

The design is [ADR 0058](https://github.com/qxuken/kui/blob/main/docs/adr/0058-a-recording-is-a-timeline.md).
