# Kit (M0 spike)

Kit is kui's UI kit: components written once in Teal, loaded as a Lua
extension, painted by a skin. This folder is its **M0 spike**: three
components on a Rust host, to measure the component shape, the event path
and the perf budgets before the core ADRs that Kit depends on are accepted.
Nothing in core changed for it. [`M0.md`](M0.md) has the findings and the
numbers: every budget met in Lua, and one new requirement, a retained data
channel into an extension.

| Path | What |
| --- | --- |
| `src/*.tl` | The Teal source: `kit` (the entry a host loads), `skin` and `ledger` (the default skin), `ctx`, and the components `button`, `toolbar`, `datatable`, `waterfall`. |
| `lua/*.lua` | What a host loads, generated from `src/`. Edit the `.tl`, never these. |
| `build.lua` | Type-checks `src/` against `packages/lua/kui.d.tl` with the Teal compiler kui-lua vendors, and writes `lua/`. `--check` fails on a stale `lua/` instead. |

```bash
lua addons/kit/build.lua
```

```bash
cargo run -p kui-lua --example lua_kit_spike
```

```bash
cargo bench -p kui-lua --bench kit
```

The host is `examples/lua/features/kit/spike.rs`, and its scene, shared
with the benches, is `scene.rs` beside it.

What the spike works around until core has it:

- **Instances** are slots named `kit/<component>#n`, served by one
  extension with `slots = { "*" }` (ADR B, decision 1).
- **Host children:** the toolbar's buttons come as data in its params
  (ADR B, decision 2).
- **Declared messages:** the host passes its own messages in the params,
  and Kit replies with a copy that has the event's fields merged in, so
  `ev.message::<Msg>()` decodes it (ADR B, decision 3).
- **Large data** crosses once by revision: `data = { rev, rows }` when it
  changes, `{ rev }` after.
- **Finding its own modules:** the host extends `package.path` with
  `lua/` before loading `kit.lua` (ADR A, decision 3).
