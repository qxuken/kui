---
status: accepted
date: 2026-10-23
---

# Floats under a node paint in tree order

> **Accepted and built (2026-10-23); backlog F186.** From kawoosh's
> diagram pane: a `.kdraw` file's items are floats on a canvas, and the
> file's order is the drawing's — the last line on top, and reordering is
> moving lines. kui stacks floats in the order they opened, which cannot
> say that. This adds one flag row, `floatsInOrder`, on the node that
> holds the floats. It keeps [ADR 0023](0023-layers-stack-in-the-order-they-open.md)'s
> stack for everything outside such a node and its refusal of a `zIndex`.

## Context

A float is a layer of its own. Layers stack in the order they opened:
the stack is kept from frame to frame by key, a root it knows keeps its
place, a root it does not know goes on top (ADR 0023, decision 3;
`stack_floats`). That is right for what floats were for — a menu opened
while a tooltip shows is over the tooltip, whatever the tree says — and
it is the only order a popup can have, since nothing in the tree says
which of two popups the user opened last.

A drawing canvas is the other case. Its items are floats because they
are placed by coordinates (`float={{ anchor: 'parent', dx, dy }}`), not
because they open. Their z-order is a document's: the item declared
later is on top. Under the opening order:

- An item that pans into view — culled the frame before, so its key is
  new to the stack — lands on top of every other item, wherever it is
  in the file. So does an item the user adds in the middle of the file.
- Moving an item in the tree moves nothing. The stack knows both keys,
  and a known key keeps its place.

The only workaround was to re-key the whole canvas whenever the drawn
set changed, which reopens every float in tree order and throws away
every node's state with it (transitions, hover, focus, an editor's
caret). The view cannot fix this from outside: the order is the core's.

## Decision

1. **A `floatsInOrder` row** — a flag: `NodeSpec::floats_in_order`,
   `floatsInOrder` in JSX, `floats_in_order` in Lua,
   `KuiSpec.floats_in_order` in C at ABI 36, `Spec.floats_in_order` in
   Odin. Declared on the node that holds the floats: the canvas.
2. **Every float under it, at any depth, paints in tree order among
   them, worked out again every frame.** The last declared is on top. A
   float nested in an item is later in the tree than the item and
   earlier than the next one, so it paints over the item it is in and
   under the items after it — the order the canvas's in-flow children
   would paint in. A float the stack opens and a float it already knew
   are treated alike, so an item that pans in or is added goes to its
   place in the order, and two items swapped in the tree swap on the
   paint and the press on that frame.
3. **As a group they keep their place in the stack.** The stack still
   runs over every float in the window, as before. Then each group — the
   floats whose outermost `floatsInOrder` ancestor is one node — is put
   where its lowest member was, in tree order, and whatever lay between
   its members moves up over it, in the order it had. So a menu opened
   after the canvas's items stays over all of them, a newcomer included,
   and a menu that opened before them stays under. Only the group's own
   order changes. Everything moved goes down to the group's lowest place,
   so a float nested in a float is still over the float it is in, as
   the stack guarantees (RG151).
4. **The hover hint a `tooltip` floats is not a member.** The core puts
   it under the hovered box, as the box's last child, and it explains
   the box to the user: in tree order it would sit under every item
   after the hovered one, and the app cannot move it. It keeps the
   stack's rule (it opens last, so it is on top), and so does anything
   inside it.
5. **A popup the app declares under the node is a member.** The core
   cannot tell a menu from an item; both are floats. A menu declared
   inside an item paints by its place, under the items after it. An app
   that wants a popup over the canvas declares it outside the node — as
   a sibling after the canvas, or anywhere the stack opens it — where it
   opens on top as every popup does.
6. **Input reads the same order.** Hit regions are pushed as the layers
   paint, so where two items overlap the later one takes the press (ADR
   0023's "the hit list is the paint list", kept).
7. **Behind the frame's flag, and cheap when nothing moved.**
   `Tree::any_floats_in_order` is set by `Tree::note`; a frame without
   the row runs `stack_floats` as before. A frame with it refills each
   root's group into a scratch vector the core keeps (no allocation once
   it has grown), checks that each group is one run in tree order —
   which the order the stack wrote back last frame already is — and
   returns. Only a frame that changed the order allocates, as a frame
   that opened or closed a float already did. The stack is written back
   in the new order, so the departure ghosts and the inspector, which
   read it, see the order that was painted.

## Considered options

- **The core decides: a float anchored to its parent is a canvas item,
  one anchored to the viewport or a node is a popup.** No row, but
  anchoring says where a float is placed, not what it is: a tooltip is
  anchored to its parent, and a canvas's labels can be anchored to a
  node. Declined.
- **Tree order for every float, everywhere.** It would make the canvas
  right and every popup wrong: a menu bar's dropdown declared before the
  body would open under a tooltip the body shows. ADR 0023 chose the
  opening order for this reason, and it still holds outside a canvas.
- **Only the floats whose nearest float ancestor is the node (direct
  items), nested ones left to the stack.** A label floated in an item
  would open on top of every item and stay there; the canvas would be
  in order only one level deep. Tree order at every depth is the
  document's, and the stack's nested-above rule is kept by it.
- **The nearest `floatsInOrder` ancestor names the group, so a canvas
  in an item of another canvas orders its own floats apart.** In tree
  order both answers are the same — an inner node's floats are a
  contiguous stretch of the outer node's — so the outermost names the
  group and an inner one changes nothing.
- **A `zIndex` number, on the item.** ADR 0023 and ADR 0046 declined it:
  the number every app writes is `9999`, and it is a second order to keep
  in step with the file's, which the tree already is.
- **Re-key the canvas when the drawn set changes.** The workaround this
  replaces, for the cost under *Context*.
- **Names.** `zOrder` and `stackOrder` promise a number or a mode.
  `layersInOrder` uses the core's word for what an app calls a float.
  `treeOrder` says the order and not whose. `floatsInTreeOrder` says it
  all and is long, and "in order" with nothing else means declaration
  order in every binding's surface, the order children are written in.
  `floatsInOrder` names the subject — the floats under the node, the
  `float` prop's word — and what happens to them, beside `overSiblings`,
  which names what a node is over and how far it reaches.

## Consequences

- `KuiSpec` takes a `uint32_t floats_in_order` in the tail padding after
  `reveal_on_hover`. Its 64-bit size stays 800 and the layout moves, so
  ABI 36. A zeroed spec stacks floats as before.
- `overSiblings` is unrelated and composes: inside an item it raises a
  child over its siblings in the item's layer, as before. On an item it
  does not move the item among the floats, since a float is a layer and
  `overSiblings` does not escape its layer (ADR 0046, decision 2).
- A popup declared under an ordered node paints by its place. The props
  row, the howto and the CHANGELOG say so.
- Pinned in `crates/kui-core/tests/floats_in_order.rs`: a newcomer in
  tree order (and on top without the row), a swap on the next frame for
  paint and press, the group's place under a menu opened after it, a
  nested float between its item and the next, the row declared on floats
  the stack already holds, `overSiblings` beside it, and the hint over
  the items. The `floats-in-order` conformance scene carries it in all
  five adapters.
