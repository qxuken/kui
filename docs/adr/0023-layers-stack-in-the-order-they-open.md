---
status: accepted
date: 2026-09-10
---

# Layers stack in the order they open, and chrome belongs to its layer

> **Accepted (2026-09-10), built the same day** — all nine action items,
> with what the building changed under
> [*What the building changed*](#what-the-building-changed). Out of one
> bug and the family it belongs to.
> The bug: in `features/theme`, right-click the page and the drawn menu
> comes up *under* the page's scrollbar — a translucent thumb across the
> menu's right edge, exactly where "Nothing doing" ends. The family: kui
> has no z-order. It has a fixed sequence of passes, hard-coded in
> `Core::emit_frame`, and three of its four members are in the wrong
> place for something. Every one of the four bugs below is that sequence,
> and none of them is a widget's.
>
> This ADR gives kui a z-order and keeps it out of the schema. A frame is
> a stack of **layers** — the in-flow tree, then one per floating subtree
> — and **the stack is ordered by when each layer opened**, not by where
> its float sits in the tree; **chrome** (a scroller's bars, the focus
> ring) **paints at the end of the layer that owns it**, not at the end of
> the frame; and input reads the same order, so what is on top is what is
> pressed. There is no `zIndex` row, and the reason is written down under
> *What was declined*.

The paint order of a frame was decided once, when floats were added:
in-flow content in preorder, then the floats in preorder, then the
scrollbars, then the ring. Each later addition — ghosts, the menu bar's
dropdown, the drawn context menu, the devtools' outline — went into one of
those slots and inherited its place. The order was right for a tooltip
over a card and has been wrong, quietly, for everything that overlaps a
scroller's edge or another float.

## Context

### The order today, and where it is written

`crates/kui-core/src/runtime/emit.rs`, `emit_frame`, is the whole of it:

1. **Pass 1** (`emit.rs:603`): every node in tree order, skipping those
   inside a float. Quads and hit regions both, so the hit list is the
   paint list.
2. **Pass 2** (`emit.rs:727`): every node inside a float, in tree order.
   A float declared earlier in the tree paints under one declared later,
   whatever either of them is.
3. **Scrollbars** (`emit.rs:766`): one thumb per scroll region, in-flow
   and floating alike, after every float. The pass also finds each
   region's node by a linear scan of `tree.keys`, once per scroller.
4. **The ring** (`emit.rs:863`): last, over everything.

Input is the same order read backwards, with one exception written in by
hand: `dispatch.rs:340` asks `scrollbar_at` *before* `hit_at`, so a bar
wins a press over anything, and `input.rs:1082` gives the cursor shape
the same priority. The comment on both says why — "scrollbars draw on
top" — which is true, and is the bug.

`depart::Pass` (`depart.rs:136`) already knows there are two layers, and
`PaintOrder` (`emit.rs:1442`) already places a ghost "just under the next
live node in its pass". That is a z-order for ghosts alone.

### Four bugs, one sequence

Measured on this tree with a throwaway `Core` test (a 400×200 window, a
20-row scroller filling it, a 200×100 viewport float at `x=250` so it
covers the bar at `x≈394`):

- **A bar over a float, always.** The float's background is solid quad
  #7 and the scroller's thumb is #8. Declaring the float `modal` changes
  nothing about paint. This is the theme example's menu.
- **A press on a non-modal float over the bar goes to the bar.** Cursor at
  `(395, 100)`, primary down and up: no event, the scroller's offset
  jumps from 0 to 200, and `cursor_shape()` is the arrow rather than the
  float's pointer. The float's `on_click` region is under the pointer and
  above the bar in the hit list, and is never asked. A tooltip, a
  popover, a menu bar's dropdown and the devtools' inspector outline are
  all non-modal floats. The context menu escapes this half only because
  it is `modal` and a bar behind a modal is `inert` (`emit.rs:127`) — the
  paint half it does not escape.
- **Floats stack in declaration order.** A "menu bar" first in the tree
  with a dropdown float, a body with a tooltip float: dropdown #1,
  tooltip #2. The dropdown is under the tooltip. An app cannot fix this
  from the view — the bar's place in the tree is its place in the layout.
- **The ring is over every float.** A focused button and two floats: ring
  #3, after both. A popover opened over a focused control shows the
  control's ring through it.

None of these is a widget doing something wrong. `widgets::context_menu`
declares a viewport float with `fit` and `modal`, which is the right
thing; the sequence puts it under a bar.

### What the codebase already believes

- **The hit list is the paint list.** `Interaction::hits` is documented
  "in paint order: later entries are on top", `hit_at` is `rev().find`,
  and every ordering decision below is made once, at emission, and read
  by both channels. The scrollbar exception is the one place the two
  orders were allowed to differ, and it is where the input bug is.
- **Keys are path hashes** (`key.rs`): a child's key is derived from its
  parent's. A node inside a float cannot outlive the float, and a float
  inside a float cannot have existed before its enclosing one. That is
  what makes "opened order" well-defined without a rule for nesting.
- **Retained state is keyed and bounded** — `AnimStore`, `ScrollStore`,
  `EditStore`, `DepartStore`, each a map from `Key` with a budget. A
  retained float order is one more of those, and the smallest: it holds
  only what this frame declares.
- **ADR 0003 warns rather than fixes** when content paints over a modal
  (`modal-behind-content`): "everything the user can see over the modal
  is inert, which looks like inert-behind is broken". The rule there is
  that paint order and interaction order are the same thing and a
  mismatch is a bug the diag names. This ADR makes them the same thing
  for the bars too.
- **ADR 0005 declined `z-index`** in a bake-off note ("'no z-index' and
  'one shadow' are the same ADR's paint vocabulary", backlog, from the
  bake-off) — declined as a *paint prop*, which it still is not. This ADR
  is not a paint prop: no row, no field on `Quad`, no sort in the
  renderer.

## Decisions

### 1. A frame is a stack of layers, and the hit list is the same stack

A **layer** is the in-flow tree, or one floating subtree without the
floating subtrees nested inside it (each of which is a layer of its own).
`emit_frame` paints layers bottom to top: the in-flow layer, then the
float layers in the order decision 3 gives. Within a layer the order is
what it always was — preorder, parents under children — and hit regions
are pushed in the same order, so `hit_at`'s `rev().find` reads the stack
without knowing it exists.

Mechanically, pass 1's `in_float: Vec<bool>` becomes `float_root:
Vec<u32>` — the index of the nearest floating ancestor-or-self, `NIL`
in-flow — computed in the same loop, and pass 2's flat walk over
`in_float` becomes one subtree walk per float root (`Tree::subtree_end`
bounds it) that skips nodes whose `float_root` is a different root.

### 2. Chrome paints at the end of the layer that owns it

A scroller's bars paint after its layer's content and before the next
layer. The focused node's ring paints at the end of the layer the node is
in. So: the page's bar is above the page and under the menu; a menu's
own bar (a long one scrolls) is above the menu's rows and under a
tooltip opened over it; a ring around a control behind a popover is
under the popover.

Why the end of the *layer* and not right after the node: a ring is drawn
2 px outside its node (`FOCUS_RING_GAP`), and a sibling at `gap: 0` would
cover half of it — the reason it was hoisted out of `emit_node` in the
first place. The layer's end is the nearest place that is above every
sibling and below every layer that could cover the control anyway.

The scrollbar pass's per-region key scan goes with this: a layer knows
the indices of the scroll regions it pushed (the `scroll_regions` length
before and after), and the node index is on the region, so the bars are
emitted from what the layer has in hand.

### 3. Float layers stack in the order they opened

The core keeps a **float stack**: the keys of last frame's float roots,
bottom to top. Each frame, after layout: keys the frame no longer
declares as float roots are removed; keys it declares for the first time
are appended, in tree order; the rest keep their places. The float layers
paint in stack order.

Consequences of the rule, each of which is the behaviour a user expects
and none of which the tree order gives:

- A tooltip that appears over an open menu is above the menu; a menu that
  opens while a tooltip is showing is above the tooltip.
- The menu bar's dropdown, opened after the page it sits above, is above
  the page's tooltips — unless one appears later, and then that tooltip
  is above the dropdown, which is where a tooltip for a menu item goes.
- A dialog and its scrim declared in the same frame stack by tree order,
  scrim under dialog; a menu opened from the dialog is above both.
- A HUD declared from the first frame is under everything opened after
  it. That is where a HUD goes.
- **Re-keying a float re-opens it**, so a view that wants one raised
  declares it under a fresh key. This is the only "raise" and it is not a
  row; see *What was declined*.

A nested float is above its enclosing float without a rule: it opened
the same frame (appended after its parent, in tree order) or a later one
(appended above). The key scheme means the reverse cannot happen, and a
`debug_assert` says so where the stack is rebuilt.

The steady state — the same float roots as last frame — is one
comparison of two short key lists and costs nothing more; the first
frame a float opens or closes does an `O(F)` merge over `F` floats, which
is tens on the busiest screen kui has. A frame with no floats does not
touch the stack (`any_float` gates it, as it gates everything about
floats today). The stack is per window, like every store the session
holds per `Core`, and OS popups (`WindowKind::Popup`, ADR 0004) are above
every layer of their owner by being windows.

### 4. Input reads the stack: a bar wins only what is under it

`ScrollbarRegion` records the length of the hit list at the moment its
thumb was painted. A press at `p` asks for the topmost hit region under
`p` and the topmost bar under `p`; **the bar wins only if the hit region
is below the bar's watermark** — that is, painted in the same or a lower
layer. One function, `Interaction::target_at(p)`, answers for the press
(`dispatch.rs:340`) and the cursor shape (`input.rs:1082`), so the two
cannot disagree again. The wheel never asked the bars — `scroll_target`
reads the scroll regions, which are in paint order already — and still
does not. `inert` stays as it is: a bar behind
a modal is still not a target.

Nothing changes for the common case — a bar with nothing floating over
it wins the press as it always has, a thumb press still starts a drag,
a track press still jumps.

### 5. Ghosts keep their place in the new order

`depart::Place` names the pass and the next live node painted after the
departed subtree. For the in-flow layer nothing changes. For a float
layer, "the next live node in the pass" becomes "the next live float root
in the stack": a departed float's ghost paints just under the float that
was above it, and at the top of the stack once that is gone too. A ghost
of a node *inside* a float paints inside that float's layer as it does
now. `PaintOrder::of` reads the stack for its float half instead of the
previous tree's order.

### 6. The access tree and the modal scope do not move

Paint order is not reading order. The access tree stays in tree order
(ADR 0001), the Tab ring stays in tree order (ADR 0002), and the modal
scope is still the subtree that declared `modal` (ADR 0003). A layer
above the modal's that is *not* inside the modal's scope is inert, as
content painted after a modal is today, and `modal-behind-content`
extends to name a float layer above the modal's whose root is outside
its scope — it is the same defect wearing a float.

## What was declined, and why

- **A `zIndex` row, CSS's.** An integer on every node with stacking
  contexts is the single most misunderstood part of CSS, and the number
  every app writes is `9999`. In an immediate tree it is also a sort per
  frame over every node that declares one. What it buys over decision 3
  is a float that stays above things opened later, and no view kui has
  wants that; the one that might — a HUD — wants the opposite. Declined
  with the condition that a view needs a float that stays on top through
  everything opened after it, and re-keying is not enough. The row, if
  it comes, is on floats only and sorts *within* the stack's frame of
  opening, so it never puts a nested float under its parent.
- **A `layer` enum — tooltip, popover, menu, modal — instead of the
  stack.** Every real ordering has a counter-case in the enum: a tooltip
  on a menu item must be above the menu; a menu from a dialog above the
  dialog; a tooltip showing when a menu opens must go under it. The enum
  is a static answer to a question whose answer is temporal. Opening
  order is the answer, and the enum falls out as its common case.
- **Keep tree order among floats; fix only the chrome.** Closes the
  reported bug and the input half, leaves the menu bar under the body's
  tooltips, and leaves an app no way out but restructuring its tree.
  Half the price for half the bugs, and the retained stack is the small
  half of the price.
- **A z key on `Quad`, sorted in the renderer.** Four bytes on the hot
  struct, a sort per frame over every quad (20k on the bench), and the
  hit list would need the same sort or a second truth. The stack costs a
  `Vec<Key>` and nothing on the quad.
- **A `raise` edge, the way `keyFocus` is edge-triggered.** Wants a view.
  Re-keying does it today; a row that does it without the re-key is a
  four-binding cost for a case nobody has.

## Consequences

- **The theme example draws right without a change to it**, and so does
  every stock widget: `context_menu`, `tooltip`, the menu bar's dropdown,
  the perf HUD and the devtools' outline are floats already, and each
  goes where a user would put it.
- **Two channels agree by construction.** After decision 4 there is one
  function that says what is under the pointer, and the paint list is the
  only ordering either channel reads.
- **A frame with floats retains one more thing**: a `Vec<Key>` of its
  float roots. It is bounded by the frame's own float count, so there is
  no budget and nothing to evict — a closed float leaves the stack the
  frame it closes.
- **The float pass changes shape**, from one flat walk to one walk per
  root; `float_root` replaces `in_float`. Pinned on the bench: the 10k-rect
  frame declares no float and must not move; a new float-heavy bench
  (a hundred tooltips) is the number the per-root walk is checked
  against.
- **Ghost placement moves with it** (decision 5). `exit`'s corpus scene
  has no float, so the check is a core test rather than a scene change.
- **Quad digests move** for every corpus scene with a ring and a float, or
  a bar and a float — `float`, `tooltip`, `menubar`, `modal`, `menu`,
  `popup` at least. The digest is not checked in (`conformance.rs`, tier
  two), so nothing in the repo drifts; the four bindings' reports are
  compared to the dump on the same machine and share the core, so they
  move together.
- **Behaviour an app might have leaned on**: a float declared *later* in
  the tree is no longer guaranteed above one declared earlier — only one
  that *opened* later is. A view that declared two floats every frame and
  relied on tree order for their stacking gets tree order still (same
  frame, tree-order append); one that opened them on different frames
  gets opening order. `CHANGELOG`'s "what you can delete" says which
  re-key hacks are no longer needed, and its "what changed" says this.

## What the building changed

Built whole on 2026-09-10 (`crates/kui-core/src/runtime/emit.rs`,
`depart.rs`, `input.rs`, `dispatch.rs`, `diag.rs`; `tests/layers.rs`; the
`layers` corpus scene in four bindings). Five things came out differently
from the text above, none against it:

1. **`Place` is three-shaped, not pass-plus-key.** Decision 5 kept
   `depart::Place { pass, before }` and changed what `before` meant for a
   float. Built, a ghost's place is `InFlow { before }`, `InLayer { layer,
   before }` or `Layer { before }` — a ghost *inside* a float needs the
   layer's key as well as its neighbour's, so that when the neighbour
   goes the ghost ends that layer rather than the frame. `Replay::paint`
   takes an `At` (six asks: under a node, the end of the in-flow layer,
   under a layer, under a node inside one, the end of one, the top) in
   place of `(Pass, Option<Key>)`, and a ghost whose whole layer is gone
   ends on top — the only place left that is under nothing it was under.
2. **The placer scans for the in-layer case.** `PaintOrder` keeps its
   `next_live` array for the in-flow layer, where a removal can be a
   thousand rows, and finds a departing node's neighbour inside a float
   by a scan bounded by that float's subtree — a departure is rare and a
   float is small, and one array per layer would be one allocation per
   float per departing frame.
3. **The stack remembers ranks.** Decision 3's steady-state check is one
   comparison of two key lists; built, each stack entry carries its root's
   rank in tree order, so the check and the answer are the same read
   (`stack_floats`): a frame with the same floats as the last does one
   pass over the stack and allocates the order alone. The rebuild, on a
   frame that opened or closed one, goes through an `FxHashMap` of this
   frame's roots.
4. **The modal check runs from emission.** Decision 6 extended
   `modal-behind-content`, which `diag::check_modal` raises over the tree
   before emission — where the stack does not exist yet. The float case
   is raised from `emit_frame` after the stack is built
   (`check_layers_over_modal`, `diag::modal_under_layer`), under the same
   code and the same once-per-key gate.
5. **The corpus pins the input half, not only the digest.** The `layers`
   scene clicks where two floats overlap in phase 0 (the toast, later in
   the tree), on the scroller's track under the popover (the popover), and
   at the same overlap in phase 2 after the popover closed and reopened
   (the popover) — three events checked in, in every binding, that the
   old order would have reported as `toast`, nothing, `toast`.

Measured: the guarded bench rows did not move (`frame_10k_rects` +1.6%,
`frame_10k_rects_with_text_and_hits` +1.4%, `frame_1k_typical` −0.6%,
against a ±3% run-to-run spread, `scripts/bench-check.sh main`). A new
row, `frame_1k_typical_with_100_floats`, is ~145 µs against ~114 µs for
the same frame without the hundred tooltips: ~0.3 µs a layer at the
steady state, most of it the hundred extra nodes. `features/theme`'s menu
paints over the page's bar in a real window, checked by screenshot.

## Action items — all done 2026-09-10

1. `float_root` per node; per-root float layers in `emit_frame`; bars and
   the ring at the end of their layer (decisions 1, 2). The scrollbar
   key scan goes.
2. The float stack in `Core`, rebuilt after layout, gated on `any_float`,
   with the nesting `debug_assert` (decision 3).
3. `ScrollbarRegion::above`, `Interaction::target_at`, and the three
   callers (decision 4). `tests/drag.rs`'s two scrollbar tests still
   pass; a third presses a non-modal float over a bar and gets the click.
4. `PaintOrder::of` reads the stack for floats; `tests/exit.rs` gains a
   departing float under a live one (decision 5).
5. `modal-behind-content` names a float layer above the modal from
   outside its scope (decision 6).
6. A corpus scene, `layers`: a scroller, a modal float over its bar, a
   tooltip opened on a later phase, a focused control under the float.
   Counts checked in; the digest carries the order to the four bindings.
7. A `Core` test that pins the order by quad index — the probe above,
   kept: bar under float, tooltip above dropdown, ring under a later
   float, press on the float over the bar reaching the float.
8. The bench: `frame.rs` unchanged on 10k rects; a hundred-float case
   added and its number recorded here when built.
9. `README.md:653` and `FloatConfig`'s doc comment (`spec.rs:119`):
   "draws on top of in-flow content" becomes "a layer of its own, above
   the in-flow tree and every float that opened before it"; the `float`
   schema row (and so `docs/props.md`) gains the same sentence.
