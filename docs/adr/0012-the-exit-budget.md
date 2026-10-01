---
status: accepted
date: 2026-09-07
---

# The exit budget: a frame's removal animates whole or not at all

> **The number moved, 2026-09-27: 4096 nodes, where it was 512**
> (backlog DX23). The view this ADR waited for came: kawoosh's fonts and
> themes panes, 1,500–1,800 nodes, could not fade out. Measured again:
> ~0.065 µs a node to depart and ~0.021 µs a node a frame to replay,
> linear, so 4096 nodes are 267 µs and then 87 µs a frame, less than the
> pane cost alive. Every decision below holds as written; only the
> constant and the corpus's numbers changed.

> **Accepted and built (2026-09-07).** Decision 5 landed first, on its
> own; decisions 2, 3 and 6 followed the same day: `DepartStore::admit`
> judges a frame's removal whole before anything is copied
> (`collect_departures` counts the departing roots first), evicts the
> oldest ghosts until the removal fits, refuses one larger than the budget
> outright, and the `exit-budget` warning names the frame's count. The
> corpus `exit` scene gained the phase the consequences ask for — `bulk`
> leaves in a frame of its own, then 600 one-node rows leave together and
> are refused whole where per-subtree admission kept 512 — pinned in all
> four bindings. Nothing here changed a schema row, an ABI struct or a
> binding, as predicted. The one thing the consequences got wrong is
> recorded there: the scene's `bulk` was *not* the boundary case that
> stays as it is, because it left in the same frame as `fade`.
>
> **Decision 5 changed shape when it was built**, and the correction is
> recorded in it: a 64-bit membership mask — the mechanism this ADR first
> specified, by analogy with the store's existing `before_mask` — saves
> **nothing at any size**, because a mass removal is hundreds of distinct
> keys and 64 bits saturate. It takes an exact set to work.

ADR 0005 shipped exit animations (backlog C8, 2026-09-04) and deliberately
left the policy as a placeholder: `exit` opt-in per node, a flat 512-node
store, no duration cap, and an open question — *what a departing subtree
should do to `animating()`* — to be "decided with a real view in front of
us". The backlog has carried it under "Design, wanting an ADR" ever since.

The view is now in front of us, and it moves the question. **`animating()`
is not the problem and does not change**: a full store owes the driver
twelve frames at 15.5 µs each, which is 0.09% of one frame's budget. What
is wrong is the *boundary*. A 1000-row list drops today with rows 0–511
sliding out and rows 512–999 blinking away, the fraction that animates set
by how many nodes a row's markup happens to contain, and an unrelated
dismissal 100 ms later truncated to twelve rows because the first one is
still in flight. So: **a frame's departures animate whole or not at all**,
**a new removal outranks ghosts already in flight**, and the O(N²) in
`depart()` that quietly defends the current budget is guarded, so 512 is a
number somebody chose rather than the largest one that happened to work.

## Context

- **What shipped.** `exit` is an `Enter` read backwards. A node that
  declared both a `transition` and an `exit`, was in the last frame's tree
  and is not in this one, becomes a **ghost**: its specs, contents and
  laid-out rects copied out of the previous frame — kept, tree and text list
  together, by a buffer swap gated on `any_exit` — and replayed frozen,
  inert, self-easing, in the paint place its node had (`depart::Place` is a
  pass plus the live node painted after it; the passes paint ghosts under
  that node, and the diff runs before the passes so nothing departs between
  `begin_replay` and `end_replay`).
- **What the policy is.** `depart::MAX_NODES` is 512. A departure that
  would take the store past it is refused, the store remembers the first
  refusal of a frame in `refused`, and `runtime/emit.rs` drains it into an
  `exit-budget` warning. There is no duration cap; `EVICT_AFTER_FRAMES`
  (300, swept every 240th frame) is the backstop for a driver whose clock
  stopped.
- **Two things the backlog entry says about this are not true of the code,
  and the ADR argues from the code.** First, refused ghosts are *not*
  silently dropped — `diag::EXIT_BUDGET` has existed since C8, it is
  documented in `docs/props.md`, it is in `index.d.ts`, `kui.h` and the
  README, and the corpus `exit` scene asserts it. The choice between
  silence and a warning was made two years' worth of rounds ago; what is
  open is whether the warning is telling the truth, and under decision 2 it
  is not, so it changes. Second, `examples/node/mindmap.tsx` is not a view
  that removes many nodes: its four cards and three links are static and it
  declares no `exit` at all — it exists for backlog F15, a pan that froze.
  The only shipped views with an `exit` on them are
  `examples/rust/toasts.rs` (a toast is three nodes, and "clear" drops a
  stack of a few dozen at most) and four one-off nodes in
  `examples/c/counter.c`. **No view in this repo has ever reached the
  boundary**, or come within an order of magnitude of it, which is exactly
  why the placeholder survived three days of rounds looking reasonable.
- **`animating()` has one shape of consumer.** `Core::animating()` is
  `anim || depart || frame_requested`. The winit driver calls it once per
  `about_to_wait` and turns it into `request_redraw`
  (`crates/kui-native/src/lib.rs:1894`); `kui_animating` hands the same bool to a C
  host; Node re-exports it as `ctx.animating()`. Node's `settle()` loops on
  the event queue, not on this, so no headless driver spins on a ghost. Every
  consumer asks the same yes/no question.

## What a real removal actually does (2026-09-07)

Measured on one machine, this worktree at `b89f461`. The bench rows are
divan, fastest of 100. The curve and boundary rows are a probe built against
`kui-core` and deleted after the run — a single timed frame per point,
fastest of 7, so read the *shape* rather than the last microsecond.

**The shipped benches first**, because ADR 0005's table is three days and
one C15 stale, and the drift is itself a result:

| bench | ADR 0005 (2026-09-04) | today | |
|---|---|---|---|
| `frame_10k_rects` | 1.26 ms | 797 µs | −37% |
| `frame_1k_typical` | 163 µs | 107 µs | −34% |
| `frame_10k_rects_one_exit` | 1.28 ms | 814 µs | −36% |
| `frame_10k_rects_all_transitioning` | 2.65 ms | 1.593 ms | −40% |
| `frame_10k_rects_all_declaring_exit` | 3.2 ms | 1.947 ms | −39% |
| `drop_1k_rows_plain` | 96 µs | 60 µs | −38% |
| `drop_1k_rows_declaring_exit` | 274 µs | 265 µs | **−3%** |
| `replay_a_full_depart_store` | 12 µs | 13.7 µs | **+14%** |

C15's boxing took about a third off every frame in the project. It took
almost nothing off the two rows that are the depart store's own work. The
exit surcharge on a mass removal was 178 µs against a 96 µs baseline and is
now 205 µs against a 60 µs one: the store went from 2.9× the cost of a plain
drop to 4.4× it, without anybody touching `depart.rs`. It is the slow part
now, and it was not before.

**The boundary.** A 1000-row list, every row declaring an `exit`, dropped in
one frame — varying only how many nodes a row contains:

| nodes per row | rows that animate | rows that vanish |
|---|---|---|
| 1 | 512 | 488 |
| 2 | 256 | 744 |
| 3 | 170 | 830 |
| 5 | 102 | 898 |
| 12 | 42 | 958 |

And the split is positional, not scattered: rows 0, 1, 2, 499, 510 and 511
animate; rows 512, 513, 900 and 999 do not. The top half of the list slides
out and the bottom half blinks. Adding one `<text>` to a row halves how many
rows animate, which is a property of the row's markup and of nothing the
author was thinking about when they wrote `exit`.

**The interference.** Two unrelated removals, 400 ms exits:

| first removal | second | apart | second animated |
|---|---|---|---|
| 400 rows | 200 rows | 100 ms | 112 of 200, `exit-budget` |
| 400 rows | 200 rows | 300 ms | 112 of 200, `exit-budget` |
| 500 rows | 200 rows | 100 ms | **12 of 200**, `exit-budget` |
| 200 rows | 200 rows | 100 ms | 200 of 200, no warning |

The budget is one global pool with a time dimension: how much of it a
feature gets depends on what a different feature did a fifth of a second
ago, and for how long that feature declared its transition. A 500-node panel
with a 5 s exit holds 500 of the 512 for five seconds.

**The cost curve**, with `MAX_NODES` temporarily raised to 32768 so the
shape past the budget is visible at all. The two departing-frame columns
are one binary with the guard behind a runtime switch and the arms
interleaved, fastest of 15 — not two builds compared, because a 40 µs
difference on a 90 µs frame is well inside what two compilations of the
same code differ by, and reading that as a result is the trap ADR 0005's
rounded-clipping amendment wrote down:

| ghosts | departing frame | departing frame, guarded | replay, per frame |
|---|---|---|---|
| 512 | 89 µs | 50 µs | 14.0 µs |
| 1 024 | 242 µs | 97 µs | 27.8 µs |
| 2 048 | 942 µs | 195 µs | 58.8 µs |
| 4 096 | 4 667 µs | 410 µs | 118.1 µs |
| 10 000 | 31 962 µs | 1 050 µs | 280.5 µs |

Three readings, and the whole of this ADR is in them.

**Replay is linear and nearly free** — 28 ns per ghost node per frame, at
every size, and unaffected by the guard because `retire` is not in that
path. Even a hypothetical 10,000-node store replays in 280 µs, 1.7% of a
16.6 ms budget. Against a real removal: a 512-node store at a 200 ms exit
owes the driver **12 frames at 15.5 µs each, 0.09% of a frame's budget
apiece**. ADR 0005's worry — "removing a node now costs frames… fine for a
dialog and wrong for a list that drops a thousand rows" — is measurably not
a thing. The frames were never the price.

**The departing frame is quadratic**, and that is the price. `depart()`
calls `self.retire(key)` unconditionally on every departure, and `retire`
is a `Vec::retain` over the entire store, so N departing subtrees cost
O(N²): 10 000 ghosts go from 32.0 ms to 1.05 ms once the call is skipped,
and the guarded column is straight-line linear at ~100 ns per node from
512 to 10 000. It is not free at today's budget either — 39 of the 89 µs
departing frame, 44%, and visible end to end on
`drop_1k_rows_declaring_exit` as 256 µs → 217 µs across four interleaved
rounds with no overlap between the two sets. The call is there for a real
case — a key that departs, returns and departs again inside one exit must
not leave two pictures — but that case is rare and the check is
unconditional.

**So 512 is not the number the ADR 0005 reasoning produced.** That
reasoning priced the *replay*, which is linear and cheap and would happily
carry several thousand. What actually stops the budget rising is the
quadratic, and 512 is roughly the largest value at which it stays under a
frame's budget. The budget's value has been defended by an accident.

**Memory, for completeness**, since it is the other thing a budget could be
about and turns out not to be binding: `GhostNode` is 280 bytes
(`NodeSpec` is 224 after C15), so the whole 512-node store is 140 KB and
even 10 000 nodes is 2.7 MB.

## Decision

**1. `animating()` is unchanged: one bool, meaning the driver owes another
frame.** No deadline, no per-source breakdown, no separate "a ghost is in
flight" channel. The three consumers — `request_redraw` in the winit
driver, `kui_animating`, `ctx.animating()` — all ask the same question and
none can act on a richer answer; a breakdown would be ABI surface for a
question nobody asks. The open question ADR 0005 wrote down is closed with
a measurement rather than a change: 12 frames at 0.09% of a budget each is
not a cost worth a policy. This decision is the reason the rest of the ADR
is about the one frame that *starts* the ghosts rather than the frames that
replay them.

**2. A frame's departures animate whole or not at all.** The budget is
tested once per frame, against the total the frame wants to add. If it
fits, every departing subtree of that frame becomes a ghost. If it does
not, **none of them do** and the frame raises `exit-budget`. No partial
credit, and in particular no positional split.

This is what ADR 0005 already claimed and did not deliver. It said the
failure mode is "exactly what a node with no `exit` does, so the failure
mode is the old behaviour rather than a wrong one" — true of a subtree,
false of a view, because a view that gets 512 of its 1000 rows animated
gets a third behaviour that neither policy produces and that nothing in the
UI explains. "The list vanished" is a thing a user has seen before. "The
top half of the list slid out and the bottom half blinked" is a rendering
bug with no bug in it. The measured dependence on nodes-per-row is the
sharpest form of the same complaint: the boundary moves when an author adds
a label, and it should not be possible to change how a removal *reads* by
editing markup that has nothing to do with the removal.

**3. Across frames, a new removal outranks ghosts already in flight.** When
a frame's departures do not fit beside what the store is holding, the store
drops its **oldest** ghosts until they do, and only refuses (per decision 2,
wholesale) if the frame's own removal cannot fit into an empty store.

Today the store refuses the newest and protects the oldest, which is why an
unrelated dismissal 100 ms behind a big one gets 12 rows of 200. Newest-wins
is the right way round on the evidence: the oldest ghosts are the ones
furthest through their own fade, so truncating them is the least visible
thing the store can do, and the removal the user just caused is the one they
are looking at. It also bounds the interference to something an author can
reason about — a removal is judged against the budget, not against the
budget minus whatever else the app happened to be doing.

**4. There is still no duration cap.** ADR 0005 declined one on the grounds
that it would be a second, inconsistent rule for a declaration that already
means what it says everywhere else, and that argument survives. What has
changed is the *reason it is safe*: not "frames are cheap" as a hope but as
a measurement (decision 1), and — the part 0005 could not have said —
duration's real cost is not frames at all but **occupancy**, the term that
converts a removal's size into how long it blocks other removals. Decision
3 is what pays that, so a cap would be solving a problem that now has an
owner. `EVICT_AFTER_FRAMES` stays as the clock-stopped backstop, and a
driver with no clock still gets no ghosts.

**5. `depart()`'s unconditional `retire` is guarded** — built 2026-09-07.
The store keeps `held`, an exact `FxHashSet` of its ghosts' keys in step
with `ghosts`, and takes the full-store scan only when that set says the
departing key is actually present. The rare case it exists for — depart,
return, depart again inside one exit — still works; it just stops charging
every other departure for it.

**It has to be an exact set, and this ADR first said a mask.** The obvious
mechanism was a 64-bit membership mask over `key.0 & 63`, by analogy with
the `before_mask` the store already keeps two fields away. Built and
measured that way, in one binary with the arms interleaved, it saves
**0% at 512 nodes and 0% at 10 000** — because the thing it guards is a
mass removal, a mass removal is hundreds of *distinct* keys, and 64 bits
saturate after about 64 of them, so the mask answers "maybe" for every row
and the walk happens anyway. `before_mask` survives the same argument only
by accident: in a mass removal almost every row's `place.before` is `None`,
so almost nothing sets a bit there. The lesson is narrow and worth
keeping — a membership mask is for a field that is *sparse*, and a
departing key is the one field in this store that never is.

This is not an optimisation for its own sake. It is what makes decisions 2
and 3 implementable at a budget anybody might choose later: under decision
2 a frame either takes its whole removal or none of it, and "whole" for a
1000-row list is a number the quadratic currently makes unaffordable. With
the guard, the departing frame is linear at ~99 ns per node, and where the
budget should sit becomes a question about the 16.6 ms it is allowed to
spend rather than a question about an accident.

**6. `MAX_NODES` stays 512, and the warning tells the truth.** The
measurement does not support raising it yet — with the guard in, 2048 nodes
cost 187 µs to depart and 59 µs a frame to replay, which is affordable, but
no view in this repo or in either field report has asked for more than six
toasts, and this project's rule for a number nobody has hit is to wait for
the view. What changes is that 512 becomes a decision with a curve behind
it, recorded here, rather than the largest number that happened to work; and
that `EXIT_BUDGET`'s message stops saying "this subtree was dropped" — under
decision 2 it is the frame's whole removal that did not animate, and the
sentence that fixes it (put `exit` on the list, not on every row) is the
same one.

## Considered options

- **Raise the budget instead of changing its shape.** Rejected as a
  first move: it relocates the boundary without changing what happens at
  it, and every measured complaint above is about the boundary's shape.
  1000 rows of 3 nodes would still split positionally at whatever the new
  number is. Worth revisiting after decision 5, on a view that asks.
- **Make the budget per departing root rather than global** ("each subtree
  may keep up to N nodes"). Rejected: it removes the cross-feature
  interference, but a 1000-row list is a thousand roots and the store's real
  constraint is the total, so it is the one shape that does not bound
  anything.
- **Keep the partial split but make it read as deliberate** — animate a
  spread sample of the rows rather than the first 512. Rejected: it is more
  code to produce a worse artefact. Rows disappearing in a comb pattern is
  not more explicable than the top half sliding, and both are less
  explicable than the list vanishing.
- **A duration cap after all**, clamping a ghost to, say, 1 s. Rejected per
  decision 4, and now with the measurement that would have justified it
  pointing elsewhere: the cost of a long exit is that it holds budget, which
  decision 3 addresses directly and without overriding a number the view
  declared.
- **Report a wake deadline from `animating()`** so a driver could sleep to
  the end of an exit instead of redrawing at vsync. Rejected: a ghost that
  is mid-fade needs every frame in between — the deadline is when it *stops*
  owing frames, which the bool already says by going false — and the frames
  cost 0.09% of budget each.
- **Split `animating()` into sources** (`anim` / `depart` / `requested`) for
  headless tests. Rejected: `crates/kui-core/tests/exit.rs` already asserts
  `depart.node_count()` and `depart.is_empty()` directly, which is a sharper
  observation than a flag, and the C and Node surfaces would carry a
  distinction no driver acts on.
- **Fix the quadratic and leave the policy alone.** Rejected as
  insufficient: it makes the wrong boundary cheaper to reach. It is decision
  5 rather than a competing option because both are wanted, in that order.

## Consequences

- **What breaks, stated plainly.** A view that drops more than the budget at
  once animates *less* after this than before: the 1000-row list that today
  gets its first 512 rows sliding out gets nothing. That is the intended
  trade and it is a real loss of motion — the argument is only that half a
  list animating is not worth having, not that nobody was getting it. The
  release that carries this says so under "what you can delete", and the
  `exit-budget` warning, which such a view is already raising today, is what
  tells an author it applies to them.
- `depart.rs` grows a per-frame admission step: the diff counts the nodes a
  frame wants to add before adding any, which it does not do today —
  `depart()` decides one subtree at a time. The natural shape is for
  `runtime/emit.rs` to collect the frame's departing roots and their subtree
  lengths (it already walks them, to compute each one's `Place`), sum, and
  then either replay the eviction of decision 3 and admit them all or admit
  none.
- `refused: Option<Key>` becomes a per-frame boolean plus, for the message,
  the node count that did not fit. The warning code and its `docs/props.md`
  row keep their names; the sentence changes, and so do the two places that
  restate it (`diag.rs`'s doc comment and the README's exit paragraph, whose
  "past that they vanish" becomes "past that the frame's removal does not
  animate").
- **The corpus needs one more phase, and the reason is precise.** The `exit`
  scene's `bulk` is a *single* subtree of `MAX_NODES + 1` nodes, refused
  whole — which is the one boundary case decisions 2 and 3 leave exactly as
  it is. Nothing anywhere pins the many-small-subtrees split, which is the
  case that changes and the case that reads as a bug. A phase that drops,
  say, 600 one-node rows would today produce 512 ghosts and after this ADR
  produce none, in all four bindings; that difference is the whole change,
  observable as a solid count, and it belongs in the scene beside `bulk`.
  **Built, with one correction (2026-09-07).** `bulk` was not left as it
  was, because it did not leave alone: the scene dropped it in the same
  frame as `fade`, `blink` and `flash`, and under decision 2 a frame of
  517 nodes is refused whole — `fade` would have lost its ghost and the
  scene its subject. `bulk` now leaves in a phase of its own, and the 600
  rows in the phase after, each refused whole; the expectation carries two
  `exit-budget` warnings and a solid count that would read 522 under
  per-subtree admission.
- On whether C8 was right to ship without a scene: moot, and it was not.
  Backlog B3 built one the same day, and ADR 0005's own superseding note
  concedes the argument — "there is no per-binding behaviour for a scene to
  catch" was wrong, because the corpus compares quads, access rows and
  events over driven steps and `exit` is the first feature where those
  disagree on purpose. The scene caught nothing on its first run, which is
  the expected result for a mechanically lowered row and not the reason to
  have it: the reason is that this ADR's change has four readers instead of
  one, which is the situation the note predicted and this is the first item
  to be in it.
- `examples/rust/toasts.rs` is unaffected. A toast is three nodes (the card
  and its two lines of text) and the stack is however many the user has
  clicked up before they expire, so "clear", which drops the lot in one
  frame, is a removal of a few dozen nodes against a budget of 512 — and
  the whole point of decisions 2 and 3 is that a view under the budget
  cannot tell they happened. **Nothing in the repo exercises the change**,
  which is a statement about the repo: the boundary wants an example that
  reaches it, and a 1000-row list with a "clear" button is a small addition
  to `toasts.rs` or a new one beside it. Until one exists, the corpus phase
  above is the only thing that would notice a regression.
- The guard in decision 5 was behaviour-preserving with a measurable result
  and no user-visible surface, which is why it landed on its own, first.
  `drop_1k_rows_declaring_exit` fell from ~256 µs to ~217 µs (four
  interleaved rounds, fastest of 100 each, no overlap);
  `scripts/bench-check.nu` reports that row without judging it. Its cover is
  `a_second_departure_of_one_key_replaces_the_first`, which is new — the
  `retire` inside `depart` had none, because the frame path cannot reach it
  (`collect_departures` retires a returning key before it can depart again)
  — plus a `debug_assert` in `end_replay` that `held` and `ghosts` are the
  same size, since a key missing from the set is a retire that will not
  happen. Both were mutation-tested: breaking the guard fails the first,
  breaking the set's maintenance fails the second.
- What is deliberately not settled: where `MAX_NODES` should sit once
  decision 5 is in. The curve says 2048 is affordable and says nothing about
  whether anyone wants it. That is a question for the first view that drops
  more than 512 nodes and minds.
