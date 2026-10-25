---
status: proposed
date: 2026-10-25
---

# The core's I/O asks: files read, written and listed off the frame

> **Proposed (2026-10-25), grilled, not built; backlog C-io. Its
> number and its backlog id are given when it lands, after Kit.** From
> the grilling of [draft ADR S](the-core-reaches-the-system-through-kui-native.md):
> the devtools' "save trace" should not write from the core, and a
> generic asynchronous save, its result routed back to whoever asked,
> grew into a small I/O layer the user wanted complete — read, write,
> delete, list and stat, streams, progress and cancelling — "for a user
> who wants to edit a large CSV and save it using kui". **Waits on**
> draft ADR S being built, and on Kit landing on main, whose data channel
> ([ADR 0065](../0065-a-data-channel.md), on that branch) carries the bytes.

## Context

An app on kui can reach files only through its own language, and some
hosts have none to reach with: a Lua extension is a guest, the Kit
runner's `kui.fs` is synchronous (backlog C83 named it), and a headless C
host writes its own. The devtools write a trace with `std::fs::write` on
the frame. Captures (ADR 0055) already show the shape that works: the
core queues an ask as data, a driver serves it after the frame, and the
result comes back as the message the ask named.

ADR 0065's channel — typed columns or a run of bytes, immutable per
revision, `Send + Sync`, named by a handle that is a plain value — lets
megabytes cross threads and bindings without a copy.

## Decision

1. **Seven verbs, by path.** On `Core`, each returning an `IoId`:
   - `read_file(path, opts, message)` — the whole file; `{ offset, len }`
     for a part; `{ chunk: n }` (or `true`, 1 MB) to stream.
   - `write_file(path, data, opts, message)` — replace, atomic by
     default; `{ offset }` or `{ append: true }` in place.
   - `delete_file(path, opts, message)`.
   - `read_dir(path, opts, message)`.
   - `stat(path, opts, message)`.
   - `rename(from, to, message)`.
   - `create_dir(path, opts, message)`.

   No open-file handles: a handle's lifetime, leak and close-on-drop would
   be in every binding, and it is the OS's abstraction, not kui's. Each
   call opens and closes; at a megabyte a chunk that is noise. `copy`,
   watching, symlinks, permissions and the network are each their own ADR
   when an app needs one.
2. **One result kind.** An `io` event carrying the caller's message with
   `id`, `op`, `path`, and `data` (a bytes channel for a read, a channel
   of entries for a listing, a record for `stat`) or
   `error: { code, message }`. The codes are a closed list pinned in a
   test across bindings — `not_found`, `no_folder`, `exists`,
   `permission`, `too_large`, `cancelled`, `no_trash`, `busy`, `other` —
   and `message` is the OS's text, for a person. A new code is an
   amendment to this ADR.
3. **Reads.** A whole read refuses a file over `max_bytes` (256 MB by
   default, per call to change) with `too_large` and "read it in chunks";
   `{ offset, len }` is bounded by `len`; a streamed read is one command
   that answers in several `io` events, each with `data` and `offset`, the
   last with `done: true`, and has no limit.
4. **Writes.** `data` is a channel, the binding's own bytes, or a string
   written as UTF-8. Replace writes a temporary file beside the target and
   renames it over, so a crash or a full disk never leaves half a file;
   `atomic: false` writes in place. `offset` and `append` write in place,
   and either with `atomic: true` is an error rather than ignored; a write
   past the end extends the file. `create_dirs` (off) makes missing
   parents; without it a missing folder is `no_folder`.
5. **Delete.** Files by default. `recursive` takes folders and goes on
   past what fails, listing it in `failed: [{ path, error }]`; `trash`
   moves to the system Trash through `system::shell`, and where there is
   none reports `no_trash` rather than deleting for good; `missing_ok`
   counts a missing path as done. All off by default.
6. **Listing and stat.** `read_dir` answers a channel with columns
   `path` (relative to the folder asked), `name`, `kind`, `size` and
   `modified`; `recursive`, `max_depth`, `follow_symlinks` (off, so a
   link loop cannot happen) and `batch: n`, which answers in several
   events of up to `n` entries, the last `done`. Hidden entries are
   listed and nothing is sorted — both are the app's. A folder it cannot
   read goes in `failed`. `stat` answers `kind` (`file`, `dir`,
   `symlink`, `other`), `size`, `modified`, `created` where the platform
   keeps it, and `readonly`, in milliseconds since the epoch;
   `follow_symlinks` is on; a missing path is `not_found`.
7. **No order is promised.** Commands may run in the order they were
   asked or at the same time — the server is free to do either, and an
   app relies on neither. An app that needs an order waits for a result,
   or chains with `{ after: id }`: start once that command has answered,
   however it answered. A streamed write is appends, each `after` the
   one before. Two safeguards hold whatever the order: a mutation —
   `write`, `delete`, `rename`, `create_dir` — whose path overlaps one
   still running (the same path, or one inside the other, after relative
   paths are resolved, symlinks not followed; a rename touches both, a
   recursive delete or listing everything under it) fails at once with
   `busy` unless it was chained; and reads are never held, since an
   atomic replace shows the old file or the new one.
8. **Cancelling.** `cancel_io(id)`, best effort: a command not started
   is dropped, a stream stops at its next chunk, a write already running
   finishes (an atomic one lands whole or not at all). A cancelled command
   answers once, with `cancelled`.
9. **Progress.** Opt in with `{ progress: true }`: `io` events with
   `done` and `total` — bytes for reads and writes, entries for a
   listing, items for a recursive delete — at most one per command per
   frame. The final event always comes; a stream's chunks are progress of
   their own.
10. **Pending, and the end.** `Core::io_pending()` counts ids issued
    whose last result has not come back, for an app's close handler to
    say "still saving". On exit kui-native's runner keeps serving until
    nothing is pending or the exit budget (ADR 0012) is spent. A core
    dropped with commands queued drops them; one running finishes, and
    its result is dropped with the core.
11. **The core queues; one implementation serves.** The core holds the
    queue (`take_io`) and the way back (`io_done(id, result)`) and touches
    no file. kui-native's `system::serve_io(&mut core)` starts what is
    new on a pool of blocking threads (four for now) and hands back what
    finished since the last call, waking the loop through the waker when
    a command finishes; `serve_io_blocking` waits, for tests. Every host
    serves through it: the runner each frame; headless C through
    `kui_serve_io`; Node through the addon, its results waking Node's loop
    through a threadsafe function — libuv's own wakeup, as asynchronous
    from JavaScript as `fs` — rather than a second implementation in JS
    that would drift; and the Kit runner on its I/O thread, its `kui.fs`
    rebuilt on these verbs (closing C83's synchronous fs). No async
    runtime: tokio's file I/O is `spawn_blocking` on a pool, as libuv's
    is, and a large save is bound by the disk, which one thread writing a
    megabyte at a time already fills; a runtime is the network ADR's
    question. Relative paths resolve against the process's working
    directory, in kui-native; the core passes paths through untouched,
    and there is no `~`, sandbox or allowed-folder list.
12. **Doors.** `read_file`, `write_file`, `delete_file`, `read_dir`,
    `stat`, `rename`, `create_dir`, `cancel_io`, `io_pending` and
    `serve_io` in C (`kui_*`), Odin (generated) and Node (`readFile`, …,
    `serveIo()`), with a `text()` helper to decode a bytes channel as
    UTF-8 in each binding (`channel.text()`, `ch:text()`,
    `kui_channel_text`); none for a Lua guest, whose host's runner has
    `kui.fs`. All in DOORS, one ABI bump.
13. **The devtools use it first.** "Save trace" becomes `write_file` with
    the trace's bytes as a channel and the panel's own marker message, the
    way its pictures pass `action("captured")`; the "saved: path" note
    comes back by the same road. `perf_trace_to` stays draft ADR S's direct
    writer: it streams while recording, and its last write is when the
    core drops, when nothing is left to serve an ask.

## Considered options

- **An async runtime for Rust (tokio, smol, io_uring).** No faster for
  files, a large dependency, and nothing a pool of four does not do.
- **Node serving with its own `fs`.** Atomic replace, the Trash,
  best-effort recursive delete, throttled progress, the `busy` check,
  streams and cancelling written twice, and a corpus to keep them equal.
- **Handles (`open`, `read_at`, `write_at`, `close`).** Closer to nbio;
  lifetimes and leaks in five bindings for a cost per call nobody has
  measured.
- **Ordering per path, kept by the server.** The user's call: order is
  the app's, said with `after`; the server only refuses to corrupt.
- **A separate event kind per verb.** Seven kinds for one shape.
- **A text mode on reads.** Not every file is text; one decode is one
  call in every binding.

## Consequences

- A new core module (the queue, `IoId`, the `io` kind and its EVENTS
  row), `kui_native::system::io` (the pool, the `busy` check, atomic
  replace, the Trash per platform), doors in three bindings and the Kit
  runner's `kui.fs`, one ABI bump, a fuzz target over the options'
  parsing, and tests in temporary folders on all three platforms (Linux
  CI answers `no_trash` where it has none).
- C83's synchronous `kui.fs` is closed by the Kit runner moving onto it.
- The book gets a chapter on reading and writing files from any binding.

## Build order

After draft ADR S is built, on main with Kit in it: the core's queue and
event; the system module's server and its tests; the devtools on it;
the doors and the Kit runner; the fuzz target, the changelog and the
book.
