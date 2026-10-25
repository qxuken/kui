---
status: accepted
date: 2026-10-25
---

# A data channel: bulk data crosses by handle, never by copy

> **Accepted and built (2026-10-25); backlog C66.** Kit's M2, beside ADR
> 0060: what M0 found a component needs when its data is large. What the
> building changed is at the end.

The core gets one primitive for bulk data: a **channel**, a store of plain
data the core owns and a handle names. A host, an extension or a worker
writes it once; anyone holding the handle reads it by index — an
extension's fill, another Lua state, a Node worker thread through the
addon. The handle travels in props, in messages and between threads; the
data does not move.

## Context

- M0 measured the copy. Props are declared every frame, so the spike sent
  100,000 rows and 18,000 bars once by revision; converting them into Lua
  tables still took 110 ms, a hitch whenever the data changed.
- A virtual table draws about forty rows, yet today the whole dataset must
  be converted to draw them.
- The runner's workers (draft ADR F) run Lua states on other threads. Lua
  tables cannot be shared across states, so a worker's result would cross
  by copy — the same cost again.
- A scroll container emits no event when the rows it shows change, so an
  extension, which answers only from `on_event`, cannot page an unbounded
  source.

## Decision

1. **A channel is a table of typed columns, immutable per revision.**
   `kui_core::channel::Channel` holds named columns of the same length —
   numbers (`f64`), integers (`i64`), booleans, or strings (one buffer and
   offsets) — or a run of bytes. It is reference-counted and `Send + Sync`.
   A writer publishes a new revision under the same identity; a reader
   holding an older one keeps it until it lets go.
2. **A handle is a value.** `Channel::handle()` is the map
   `{"$channel": id, "rev": rev}`, so it rides wherever a value does: a
   component's props, a slot's params, a message, a worker's dispatch.
   `Channel::resolve(&value)` answers the latest live revision of that
   identity from a process-wide registry of weak references — so a channel
   lives as long as some side holds it, and a handle to one nobody holds
   resolves to nothing. Two handles compare as an integer pair, so a slot
   or an instance replay (ADR 0045, ADR 0060) checks one as cheaply as a
   number.
3. **Readers index; nothing converts it whole.**
   - Rust: `len`, `column(name)` → a typed slice, `get(row, col)` → a cell.
   - Lua: a handle in props or params arrives as userdata — `#ch`,
     `ch:get(i, col)`, `ch:row(i)` (a table of that row), `ch:column(name)`
     (the one column, as a list, when the script asks for it), `ch.rev`,
     `ch.names` — and a script makes one with `channel { name = {...},
     start = {...} }`.
   - Node: `new Channel({ name: [...], start: Float64Array })`, read with
     `get(row, col)`, `column(name)` (a typed array for numbers),
     `publish(columns)` for the next revision; it serializes as its handle,
     so `props={{ rows: channel }}` is all a host writes.
   - C: a builder — `kui_channel_new`, `kui_channel_add_f64`,
     `kui_channel_add_i64`, `kui_channel_add_str`, `kui_channel_finish`
     answering the handle value — and readers `kui_channel_len`,
     `kui_channel_get_f64`, `kui_channel_get_str`. Odin's are generated.
4. **A visible range for unbounded sources.** A new handler row, `onRange`
   (Lua `on_range`), on a node whose children carry data indices: after
   layout, when the indices of its children inside its box change, it
   posts its message with `range: { first, last }` — so an extension's
   table can page a source it does not hold whole.
5. **Workers talk in two layers.** Small messages by `dispatch`, plain
   values copied across; bulk results as channel handles inside them. The
   registry is process-wide, so a handle a worker's state made resolves in
   any other.

## Considered options

- **Bigger params with smarter diffing.** The data would still convert on
  every change and still cross threads by copy.
- **A `Value::Channel` variant.** Every binding's converters and every
  exhaustive match would learn it, for what a two-key map already says.
- **Raw shared byte buffers** (`SharedArrayBuffer`-like). Too low-level
  for rows and columns; bytes stay one kind of content.
- **Sharing Lua tables between states.** Not possible in Lua.

## Consequences

- A new core module and doors in five bindings, a fuzz target over the
  builder and the readers (`channel`), and a bench row: 100,000 rows
  handed to a Lua table component and its first frame, against M0's 110 ms.
- Kit's table and waterfall take a channel instead of `data = { rev,
  rows }` (M3).
- `EventKind::Range` and its `onRange` row; Node's protocol carries the
  row as it carries every handler row. C's `KuiSpec` gains `on_range`
  (ABI 38).
- The corpus's `virtual` scene declares `onRange`, so all five adapters
  report `range rows` for its two rows in view.

## What the building changed

- The draft's readers named `kui_channel_get_*` per kind; C has
  `kui_channel_get_f64` (an integer widened) and `kui_channel_get_str`,
  the two a reader in C asks for — a boolean column reads as a number in
  no binding, so it is left to the others for now.
- Rows are 0-based in every binding, Lua included, as a list's `index`
  and an `onRange` event's are; a column may be named or numbered.
- A handle names the identity, and resolves to its latest revision: a
  reader handed an old handle reads the newest data, and the revision it
  names is what makes two props differ for a replay.
- Measured with `cargo bench -p kui-lua --bench kit -- channel handover`
  (M3 Pro, AC power): a Lua table component that reads the forty rows it
  draws out of a 100,000-row channel, asked fresh every frame, is
  0.39 ms a frame; building that channel from nothing — formatting the
  strings included — is 4.0 ms once; M0's conversion of the same rows
  into Lua tables (`handover`) is 103 ms.
