# kui-value

kui's plain-data payload: `Value` (null, bool, int, float, string, list,
an ordered string-keyed map), a strict JSON reader into it, the
`MessageField` conversions a typed message is built from, and — behind
features — its Lua conversions (`lua`) and `#[derive(Message)]`
(`derive`).

It depends on no other kui crate, so what has no kui core — the data
channel (`kui-channel`), the recording's reader (`kui-trace`) — speaks
the same values. `kui-core` re-exports all of it where it always was:
`kui_core::Value`, `kui_core::json`, `kui_core::message`. See
[ADR 0075](../../docs/adr/0075-kui-perf-reads-a-recording-in-kit.md),
decision 3.
