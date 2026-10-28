# kui-channel

kui's data channel ([ADR 0065](../../docs/adr/0065-a-data-channel.md)):
a table of named, typed columns of one length, or a run of bytes, that a
small handle value names. Immutable per revision — `publish` makes the
next one under the same identity — and read by index from anywhere that
holds the handle: an extension's fill, another Lua state, a worker
thread.

It depends on `kui-value` alone, so what has no kui core — the
recording's reader, `kui-trace` — makes channels too
([ADR 0075](../../docs/adr/0075-kui-perf-reads-a-recording-in-kit.md),
decision 3). `kui-core` re-exports it as `kui_core::channel`. The `lua`
feature adds `kui_channel::lua`: the channel as a script's userdata and
the value conversions that carry a channel as its handle.
