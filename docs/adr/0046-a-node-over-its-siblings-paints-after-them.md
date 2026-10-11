---
status: accepted
date: 2026-10-11
---

# A node over its siblings paints after them

> **Accepted and built (2026-10-11); backlog F163.** From kawoosh's tab
> strip: a tab moved left glided under the neighbour making room for it,
> and one moved right glided over it. This adds one flag row,
> `overSiblings`, which changes the order of siblings and nothing else. It
> does not revisit [ADR 0023](0023-layers-stack-in-the-order-they-open.md)'s
> refusal of a `zIndex`. That refusal was about floats and layers, and
> `overSiblings` opens no layer and carries no number.

## Context

Inside a layer, kui paints in tree order: preorder, a parent under its
children, a later sibling over an earlier one (ADR 0023, decision 1). The
hit list is the paint list, so the press goes to whatever is drawn on top.
A row's tree order is its places' order. So when two `slide` siblings
cross on their way to new places, the one *landing* in the later place is
on top. Moved right, the tab the user moved lands later and passes over
its neighbour. Moved left, it lands earlier and passes under it: covered
from one side and then the other, it looks like a block shrinking to its
middle and growing back out of the next place's middle.

The core cannot tell which tab the user moved. In the common case, a tab
moved one place, both tabs travel exactly one width, so no rule read off
the motion can choose between them.

The recipe written for kawoosh on 2026-10-11 made the moved tab a float
anchored to a slot in the row, but only while it glides. That works, and
it costs the view three things. It has to track when a glide starts and
ends, which the core knows and the app does not. It needs an extra node
per tab. And a float is a layer: it goes over every float opened before
it, so a tooltip from a bar above the strip would sit under the gliding
tab, which is why the recipe had to turn the float off when the glide
ended.

## Decision

1. **An `overSiblings` row** — a flag: `NodeSpec::over_siblings`,
   `overSiblings` in JSX, `over_siblings` in Lua, `KuiSpec.over_siblings`
   in C at ABI 31, `Spec.over_siblings` in Odin. A node that declares it
   paints, with its subtree, after its siblings and their subtrees: at the
   point the walk leaves its parent, not at its own place.
2. **The sibling order is all it changes.** The parent still paints under
   it. Whatever follows the parent in its layer still paints over it. It
   keeps the parent's clip and opacity, which the walk computes in tree
   order as before. A float still opens above it, and it does not escape
   its layer. Two siblings that both declare it paint in tree order
   between themselves. One declared inside a sibling paints before one
   declared beside that sibling, because it is part of the sibling, which
   is under it. On the root it does nothing: the root has no siblings.
3. **Input reads the same order.** Hit regions are pushed as the node
   paints, so where it overlaps a sibling it takes the hover and the press.
   This is ADR 0023's "the hit list is the paint list", kept.
4. **Reading order does not move.** Text runs are recorded as they paint,
   and a selection reads them in recording order, which until now was
   tree order (`TextSystem::scope_runs`). A frame that declares
   `overSiblings` puts its runs back in tree order after each layer
   (`TextSystem::order_places`), so a selection across such a card reads
   it in its place. The access tree and the Tab ring were always tree
   order and are untouched.
5. **A departing ghost keeps its place.** A ghost whose place was "just
   under node *k*" paints at *k*'s place in the tree, even when *k* is
   over its siblings and paints later. It was under *k* and stays under
   it, and it does not jump over *k*'s siblings.
6. **Behind the frame's flag.** `Tree::any_over_siblings` is set by
   `Tree::note`. A frame that declares no `overSiblings` runs the walks it
   ran before, and the float pass keeps its flat loop. With the flag set,
   pass 1 skips such a subtree's emission and queues it, and the queue is
   drained at the index where its parent's subtree ends. Pass 2 walks each
   layer with the same rule.

The view that motivated this is now one attribute:
`overSiblings={t.id === selected}`, left on. A tab that overlaps nothing
changes nothing, so there is no glide to track.

## Considered options

- **The core decides: whatever glides is on top, and the farther travel
  wins.** No row and no binding cost, but the reported case is a tie. Two
  adjacent tabs that swap places both travel one width, and the tie falls
  back to tree order, which is the bug. A drag that moves a tab one place
  at a time hits the tie on every step. Declined. A view that wants it can
  compute it and declare `overSiblings` from that.
- **Keep the float recipe and document it.** Written on 2026-10-11 and
  replaced the same day, for the three costs under *Context*.
- **A `zIndex` number.** ADR 0023 declined it, and those reasons still
  hold: the number every app writes is `9999`, and it implies a sort per
  frame. `overSiblings` is the one bit a sibling order needs, and its cost
  is paid only on frames that declare it.
- **Other names.** `raised` reads well, but the theme already has a colour
  role `raised` (the surface of a tooltip or a menu, `bg="$raised"`), and
  a boolean prop with the same name would be read as a colour. `front`
  says where the node ends up but not what it is in front of. `onTop` is
  `on_top` in Rust and C, which reads as an event handler beside
  `on_click`. `topmost` sounds global, and a float still opens above the
  node. `paintLast` names the mechanism and leaves out the press.
  `overSiblings` names both what the node is over and how far it reaches.

## Consequences

- `KuiSpec` grows by a `uint32_t over_siblings` past `iterations`. Its
  64-bit size goes from 744 to 752, so ABI 31. A zeroed spec paints in
  tree order, as before.
- Each text run records its node's index (`TextPlace::node`) so the
  re-sort needs no lookup.
- Pinned in `crates/kui-core/tests/over_siblings.rs`: the order of such a
  child among its siblings and its parent's sibling; nesting; the flag
  inside a float; the reported glide, drawn and pressed both ways round;
  and the reading order of such a run. The conformance sampler declares
  it on its cut-short text, so all five adapters carry the row.
