# kui-perf's spike

ADR 0075, decision 11 (backlog F252): before `kui-perf` is built, a bare Lua
app on the `kui` runner — Kit's waterfall over a recording's spans and its
table over its frames, from `kui.trace` — is measured over a recording the
size of a long session, against the budgets the decision names.

```sh
cargo build --release -p kui-cli -p kui-trace
target/release/kui-trace synth --frames 72000 big.kuitrace          # 937,000 spans
KUI_PERF_TRACE=run.kuitrace target/release/kui run tools/kui-perf/spike --headless -- pan big.kuitrace 120
target/release/kui run tools/kui-perf/spike/analyze --headless -- run.kuitrace pan
```

`main.lua` runs one phase a run — `pan`, `zoom`, `full`, `table`, `still`,
`idle` — stepping its model on a timer and marking where the measured
frames begin and end; the runner records itself (`KUI_PERF_TRACE`), and
`analyze/main.lua` reads that recording back through `kui.trace`: the
frames between the marks and their work. Idle is checked in a window too,
by the process's CPU time over a few still seconds. The numbers are in
ADR 0075's amendment.
