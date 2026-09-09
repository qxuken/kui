---
status: accepted
date: 2026-09-09
---

# Caching against the last frame: the access tree yes, the frame no

> **Accepted (2026-09-09), and its one yes is built.** Out of the
> performance round that shipped the quad shrink and closed C24. It asks
> one question — may a frame reuse work from the frame before it, and
> immediate mode still mean what it says — and answers it three times,
> because the answer is different for the three places it could be asked.
> Decisions 1 and 2 decline, with a trigger written down for the first so
> the next person to want it has something to argue with. Decision 3 is
> built: `frame_10k_rects_with_access_tree` **1.77 ms → 1.39 ms, −21.7%**
> against a ±3.0% run-to-run floor, the only row in the table that moved.
> What the building changed is at the end, and the detection measurement
> below was wrong when this was written and is corrected in place —
> [that correction](#measurement-what-detection-costs-2026-09-09-corrected)
> is the more useful half of the document.

kui is immediate mode: the view runs every frame and what it builds is the
frame. Nothing is retained between frames except what has to be — scroll
offsets, tween state, the shaped-text cache, a departing subtree — and each
of those is retained by a named mechanism that says why. The question this
ADR asks is whether the *frame itself* should join that list: whether the
core may notice that a subtree is identical to last frame's and skip laying
it out and emitting it.

The honest short answer is that the win is real, smaller than it looks, and
lands almost entirely where two existing answers already land one to two
orders of magnitude harder. The dishonest failure mode is worse than the win
is good: a cache that misses an input does not crash, it draws last frame,
and it does that only in the app and not in the tests. So the proposal is to
**decline the general cache**, to **decline a `memo` element outright** on
contract grounds rather than performance ones, and to **accept one narrow
cache** — the derived access tree, which is 40% of a frame while a screen
reader is attached, is pure derived output, and has inputs you can write
down.

## Context

- **Where a frame's time goes.** `sample`(1) over `frame_10k_rects`
  (717 µs, 100×100 plain rects) puts about **40% in building** the tree —
  the app's own closure, `Core::open_content`, `Tree::push` — about **45%
  in the five layout passes** (`crates/kui-core/src/layout.rs:224`) and
  about **15% in emission**. A cache that skips layout and emission for an
  unchanged subtree therefore has a ceiling of **2.5×**, because the app
  rebuilt the tree either way and that half of the bill is not addressed by
  anything in this ADR.

- **The loop is already input-driven.** A driver schedules a frame on input,
  or because `Core::animating()` said a transition is mid-flight. There is
  no redundant-frame problem to solve: the frames that happen are frames
  where *something* changed. What a cache would exploit is that the
  something is usually small.

- **Two existing answers are far better where they apply.**
  `widgets::virtual_column` takes a 10,000-row list from **3.68 ms to
  16.9 µs** (218×), and 100,000 rows to the same 16.9 µs — it does not build
  what is off screen, so there is nothing to cache. The `cells` element
  (backlog C20) makes a terminal screen one node instead of ten thousand,
  about **37×**. Both are the same move: a better primitive, so the nodes
  never exist. Caching is the other move: let the nodes exist and pay less
  for them the second time.

- **Interaction and animation already fold into the spec before it is
  pushed**, which is what makes a digest over pushed specs viable at all.
  Hover, press and focus backgrounds are resolved in
  `Core::resolve_declared_hover_style`
  (`crates/kui-core/src/runtime/builder.rs:265`), the OS accent colour in
  the same chain (`:376`), and a transition's eased values in
  `Core::ease_spec` (`:89`) — so a hovered node, an accented node and a
  transitioning node all push a *different spec* and would fall out of a
  cache on their own, correctly and without being special-cased.

- **The core already retains and diffs a previous frame, and it is not
  cheap.** `depart` keeps the whole previous tree and diffs it by key to
  find removals (ADR 0012). `frame_10k_rects_all_declaring_exit` is 2.06 ms
  against `frame_10k_rects_all_transitioning`'s 1.70 ms: the retain-and-diff
  costs about **360 µs at 10,000 nodes**, half the base frame. That is a
  key-set diff rather than a digest fold and a cache would not do it that
  way — but it is the closest thing in the repo to the machinery being
  proposed, and it is evidence that "compare this frame to the last one" is
  not free at this granularity.

- **The access tree is the outlier.** `frame_10k_rects_with_access_tree`
  (1.63 ms) and `frame_10k_rects_with_text_and_hits` (1.16 ms) build the
  same grid; the difference is `Core::access_tree()`, about **470 µs, or
  +40% on that frame**, paid on every frame while assistive technology is
  attached and by nobody otherwise. It is derived output — the core does not
  act on it — and it changes far less often than the frame does.

- **C24 is the round's other lesson.** Three data-layout changes were
  measured against this profile and only the first paid; a microbenchmark of
  an access pattern mispredicted the real thing three times running. Nothing
  in this ADR should be built on a probe alone.

## Decision

1. **No general subtree cache**, for now. The design that would be built is
   written down under [Considered options](#considered-options) and its
   trigger under [Consequences](#consequences), so this is a "not yet with a
   condition" and not a "no forever".

2. **No `memo` element, ever, in the form that asks the view to declare its
   own dependencies.** This is a contract decision, not a performance one,
   and it does not have a trigger. An element that means "trust me, nothing
   under here changed" moves the correctness obligation from the core to
   every app, and the class of bug it admits — a view that forgot a
   dependency, so the screen shows something that stopped being true — is
   exactly the class immediate mode exists to make impossible. kui's answer
   to "my view is too expensive to run every frame" stays *build less*
   (`virtual_column`), not *run less of what you built*.

   The name is taken here so it is held for nobody, the way ADR 0015 holds
   `painter`.

3. **Cache the derived access tree on a digest**, subject to its own
   measurement. **Built 2026-09-09; see the amendment.** `Core::access_tree()` recomputes from scratch on every call;
   it should recompute only when its inputs changed. This is accepted in
   principle because it is the one place where the work is large (+40%),
   purely derived, and reached through a single function whose inputs can be
   enumerated: the tree's specs and content, the laid-out rects, focus, the
   edit store's text and selection, scroll offsets, and the viewport.

4. **Detection is by comparison against last frame, not by hashing, and it
   compares only what the consumer reads.** A serial digest of the specs
   costs 280 µs per 10,000 nodes — more than half of what deriving the
   access tree costs — while equality against a retained copy costs 52 µs.
   And whole-spec equality is the wrong granularity anyway: hover, press and
   the accent colour all land in `style.bg` before push, so comparing whole
   specs would invalidate on every frame a mouse moves, which is exactly the
   frame the cache exists for. See
   [Measurement: what detection costs](#measurement-what-detection-costs-2026-09-09-corrected),
   whose first version got this wrong by a factor of fifty.

5. **A cache key is never just the subtree.** Layout resolves against the
   box the parent gives, and emission against an inherited clip, group
   opacity and scale. Any cached result is keyed on the digest *and* the
   available size on both axes, the clip, the opacity and the scale, or it
   is wrong the first time a card is narrower than it was.

## Considered options

**A. Nothing. (Chosen for the frame.)** The frame stays immediate, and
"too many nodes" keeps being answered with a primitive that has fewer of
them. Costs: an app with ten thousand genuinely distinct visible nodes pays
717 µs a frame forever. Nobody has reported that app; the field reports that
looked like it produced `cells` and `virtual_column` instead.

**B. An invisible subtree cache in the core.** Digest each node at push, fold
at close, and when a subtree's digest and its layout context both match last
frame's, reuse its sizes, positions (translated), quads (translated), hit
regions and access rows. This is a pure optimisation in the sense that a
correct implementation is unobservable — which is also the problem, because
an *incorrect* one is unobservable too until it is on screen. Ceiling 2.5×;
detection about 10% of a frame once measured honestly; the list of inputs a
digest does not see is under
[What a digest cannot see](#what-a-digest-cannot-see) and is the real cost of
this option, cost being the lesser word for it.

**C. A `memo` / `static` element the view declares.** Skips the build as
well, so the ceiling is unbounded rather than 2.5×. Rejected on the contract
grounds in decision 2. Worth noting that it is also the *only* option that
addresses the 40% of the frame that is the app's own building, which is why
it will keep being proposed and why the refusal is written down rather than
left implied.

**D. Cache only the derived outputs, not the frame.** The access tree
(decision 3) is this option applied to the one output big enough to matter.
The display list is not a candidate — it is the frame, and option B is what
caching it means. The hit regions are not a candidate: they are already
cheap and they are rebuilt from the same walk that emits.

## Consequences

- **The frame stays honest.** What the view builds is what the frame is,
  every frame, and no part of the core keeps a copy it might serve instead.
  That property is worth more than 2.5× on a frame shape nobody has
  reported, and it is the property that lets every bug in this repo be
  reproduced by building one frame.

- **The trigger for revisiting option B** is a real app, with a real profile,
  whose frame is too slow at a node count that `virtual_column` and `cells`
  cannot reduce — that is, ten thousand or more nodes that are all
  simultaneously visible and all genuinely distinct. Until then the evidence
  points at a better primitive rather than at a cache, because that is what
  it pointed at the last three times.

- **The access tree cache changes what `access_tree()` costs, not what it
  returns.** If its digest misses an input the failure is a stale
  accessibility tree, which is a real bug and a quiet one; the measurement
  that gates decision 3 has to include a conformance scene that mutates each
  enumerated input in turn and asserts the tree moved.

- **The atlas epoch is a hard invalidation for anything that caches quads.**
  A glyph quad carries atlas texels in `uv`, and a repack bumps
  `GlyphAtlas::epoch` and invalidates every one of them. `CellStore::emit`
  already carries exactly this check
  (`crates/kui-core/src/cells.rs:299`); option B would need the same, and it
  is listed here so that a future implementation does not rediscover it on
  screen.

## What a digest cannot see

The argument against option B is not that it is slow, it is this list. A
digest over pushed specs covers the specs; a frame is not only its specs.
Each of these changes what a subtree draws without changing what the view
pushed for it, so each is a cache that serves last frame:

- **The glyph atlas epoch.** A repack moves every glyph, so every cached
  glyph quad's `uv` is stale.
- **Resources mutated between frames.** An image replaced under the same
  handle, a font added or dropped, a fragment's WGSL re-registered — the
  spec holds a handle, and the handle did not change.
- **Text measurement.** A text node's size depends on the width layout gave
  it, which is an output of the pass the cache is trying to skip; and its
  metrics depend on the font system's state, which is not in the spec.
- **Scroll offsets**, which are retained in `ScrollStore` and applied in
  `layout::positions`, not declared in the spec.
- **The focus ring**, drawn from the core's own focus state rather than from
  any node's spec.
- **A departing subtree** overlapping the cached one: ghosts are painted in
  the middle of the pass order (`depart::Place`), so a cached run of quads
  cannot simply be replayed as one block.
- **`env`**, beyond the accent colour that is already folded in: appearance,
  reduced motion, locale.

Every one of these is enumerable, and every one of them is a line of code
that someone has to remember to write. The cost of option B is not the 1% of
detection; it is that this list has to be complete, and stay complete as the
core grows, with nothing but a stale pixel to say when it is not.

## Measurement: what detection costs (2026-09-09, corrected)

**The first version of this section was wrong, and the correction is the
more interesting number.** It reported an FNV digest of a 224-byte spec at
about 6 µs per 10,000 nodes, and concluded detection was free. The probe
hashed near-constant data, so LLVM folded most of the chain away; 6 µs for
280,000 dependent multiply-xor steps was never physically possible and
should have been caught by that alone. Re-measured over specs the compiler
cannot see through, on an Apple M3 Pro, 10,000 nodes:

| | median |
|---|---|
| walking every spec and reading one word (the floor) | 4.5 µs |
| comparing each against a retained copy of last frame's | **52 µs** |
| FNV, four independent lanes combined at the end | 73 µs |
| FNV, one serial chain over every word | **280 µs** |

The serial digest — the obvious implementation, and the one the first
version of this section priced at 6 µs — costs **more than half** of what
deriving the access tree costs, and would have made a cache that loses on
its own detection. FNV's multiply is a four-cycle dependency and there are
28 words a node; nothing about it is free.

Three things follow, and they replace what decision 4 originally said:

- **Compare, do not hash.** Equality against a retained copy is five times
  cheaper than the serial digest and lowers to vector loads; the price is
  keeping last frame's state, which for the tree the core already knows how
  to do (`depart` swaps buffers rather than copying).
- **Compare only what the consumer reads.** Whole-spec equality is the wrong
  granularity for the access tree specifically: hover, press and accent all
  land in `style.bg` before push, so a mouse moving across a UI would
  invalidate on every frame over a field the access tree never looks at —
  which is precisely the frame where the cache would otherwise pay.
- **Detection is about 10% of a frame, not 1%.** That does not change the
  decision for option B, which was declined on the correctness list rather
  than on cost, but it removes the argument that its detection would be
  invisible.

## Measurement: what a cache could save (2026-09-09)

From the `sample`(1) profile of `frame_10k_rects` at `ce78116`, top-of-stack
samples, ~9,300 in the working thread:

| block | samples | share |
|---|---|---|
| building (the view's closure, `open_content`, `Tree::push`, key derivation) | ~3,780 | 40% |
| layout (`distribute_axis`, `positions`, `compute`, `distribute_run`, `set_axis_clamped`) | ~4,150 | 45% |
| emission (`emit_node`, `finish_frame`) | ~1,390 | 15% |

Option B addresses the second and third rows only, so its ceiling on a frame
where *nothing* changed is 717 µs → about 290 µs. For scale, the two answers
that already exist:

| | naive | with the existing answer |
|---|---|---|
| 10,000-row list | 3.68 ms | 16.9 µs (`virtual_column`) |
| 100,000-row list | — | 16.9 µs |
| a terminal screen | per-cell nodes | ~37× cheaper (`cells`, C20) |

And the frame the README calls typical, `frame_1k_typical`, is **114 µs**
today — under a tenth of a 60 Hz budget before any of this.

The access tree, by contrast, is 470 µs on a 1.16 ms frame that already
contains it, which is the whole reason decision 3 separates it from the
rest: it is the one number in the table that a cache would meaningfully
change, and the only one whose output the core never acts on itself.

## Amendment: what the building changed (2026-09-09)

Decision 3 is built, in `access::inputs_hash` and `Core::access_tree`.
Five things the design did not say, in the order they came up.

**The walk is a quarter of the work, which is what makes it pay.** The
design assumed deriving the tree was mostly walking it, and if it had been
there would have been nothing to gain — a hash walk that costs what the
build costs saves nothing, which is how C24 died. Measured before writing
any of it: `build` is **480 µs** over a 10,000-node frame and the same
traversal calling the same `semantic` is **105 µs**, so three quarters of
that function is constructing `AccessNode`s and pushing them. That is the
part a hit skips, and it is why the *safe* implementation — one that calls
the real `semantic`, `focusable` and `orientation` rather than
reimplementing what they decide — is also the cheap one. The drift surface
is therefore only the fields `build` reads directly.

**A custom editor answers `None`, not a hash.** `custom_editor` fills a node
from its `line` children's own text and runs, and there is no reading of
those inputs short of building the node. Such a view returns "cannot
answer, rebuild"; it is a view whose text is changing anyway, so it is the
case a cache would miss on regardless.

**An editor's shaped runs are not determined by its text and its box.** A
font arriving between frames reshapes them without necessarily moving the
rect. `EditStore::version` already bumps on every mutation that could, so it
is mixed in and stands in for reading the runs.

**`WindowRole` and `WindowButton` gained `Hash`** rather than being matched
by hand in the hash, so a variant added later is covered without anyone
remembering to cover it.

**Two of the gating tests failed first, and both were the test's fault** —
which is the outcome that should be reported, because it is evidence the
hash was right and the assertion wrong. `focusable` asserted that adding
`focusable()` to a node that already had `on_click` moved the tree; it does
not, because such a node was focusable already. `a_scroll_offset_moves_the_tree`
scrolled a box with nothing in it to scroll, so the offset clamped to zero
and nothing moved. Both now ask what they meant to. There are 21 cases in
`runtime::dispatch::access_cache`, one per input plus the two that assert a
frame the tree cannot see keeps the tree it had.

**What the bench measures, and what it does not.** The −21.7% is a 100% hit
rate: `run_frame` rebuilds an identical grid, so nothing the tree can see
ever changes. A real session hits on the frames the cache was argued for — a
pointer crossing hover backgrounds, a colour transition, a caret blink — and
misses whenever a rect moves, text changes, or focus does. The honest claim
is the one the decision made: this is the cost of *deriving* the tree, and
it is now paid only when the tree would come out different.
