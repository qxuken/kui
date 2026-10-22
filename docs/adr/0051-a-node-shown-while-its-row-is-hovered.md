---
status: accepted
date: 2026-10-23
---

# A node shown while its row is hovered

> **Accepted and built 2026-10-23; backlog F183.** What the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed). Raised by the Cookie Mapping
> Builder, a Node app on alpha.63, during a redesign: a row's or a group
> header's secondary actions (rename, delete, export) should show only
> while that row is under the pointer, as in a file list or an editor's
> tab strip. kui could not say so. The app drew the buttons always,
> quietly, and asked for the rest in kui.

## Context

- **What kui resolves for hover today.** `hoverBg`, `pressedBg` and
  `focusBg` are swapped into a node's `bg` by the core as the node opens
  (`Core::resolve_hover_style`), from what the last hit test found. A
  `hoverGroup` makes nodes that share a name light together. The view
  never asks `isHovered`, so a Node or Lua view, which the binding
  re-lowers from its stored tree for a pointer move rather than running
  again, still repaints its hover. Only a background changes.

- **Hover is one node.** The hovered node is the topmost hit region
  under the pointer (`Interaction::hovered`). Over a stock button inside
  a row, the hovered node is the button, not the row. That is right for
  `hoverBg`. It is wrong for "is the row hovered", which is CSS's
  `:hover` on an ancestor: the pointer over anything inside it. F179
  met the same thing for a box's `tooltip` and answered it there by
  scanning the box's subtree when the box closes.

- **What an app had to do.** Re-render on `onHover` enter and leave of
  every row (a handler per row and a model field for the hovered one),
  or show the actions always. A `hoverGroup` cannot carry it: the stock
  button's rows are closed, so the button inside cannot join the row's
  group, and a group lights backgrounds only.

## Decision

1. **A flag on the node that hides: `revealOnHover`** (`reveal_on_hover`
   in Lua and Rust, `KuiSpec.reveal_on_hover` in C, Odin's generated
   `Spec.reveal_on_hover`). The node is drawn only while its *anchor* is
   hovered, and hidden otherwise. One flag on the actions box, nothing on
   the row but what it already has.

2. **The anchor is the nearest ancestor with a hit region of its own**:
   the first ancestor that tracks the pointer (`hover_tracked`) — an
   `onClick` row, a `hoverBg` card, a `focusable` control, a group header
   marked `hoverable`. No name to keep in step, and the row that lights
   on hover is the row that reveals. With no such ancestor nothing could
   ever show the node, so it is **drawn always**, and
   `reveal-without-anchor` says why. A hidden control nobody can find is
   a worse failure than a visible one.

3. **"Hovered" is CSS's `:hover`, not kui's one hovered node**: the
   anchor or anything inside it, so the pointer moving from the row onto
   the button it revealed keeps it shown. And **a press held inside the
   anchor** keeps it shown while the pointer is dragged off, until the
   release.

4. **Keyboard parity: focus inside shows it.** The node is drawn while
   keyboard-visible focus is anywhere inside the anchor — Tab onto the
   row's checkbox and its actions appear — and while focus of any kind
   is inside the node itself, so a control in it that has the keyboard
   never vanishes (an inline field, a button clicked and the pointer
   moved away). A click's focus elsewhere in the row does not count:
   the pointer that clicked has its own rule, and leaving the row should
   hide its actions.

5. **Hidden is paint: opacity 0.** The node keeps its room, so the row
   does not reflow as the pointer crosses it. It keeps its hit regions,
   so the pointer that reaches the hidden button finds it (and has shown
   it by then, since the button is inside the anchor). It keeps its
   place in the access tree and in the Tab ring: a screen reader reads
   the actions and acts on them, and the focus that lands there shows
   them. A `transition` on the node eases the opacity, as it eases a
   `hoverBg`.

6. **Resolved by the core as the node opens**, beside the hover
   backgrounds, from the hover, the press and the focus of the frame
   that finished: no `onHover`, no model field and no re-render of the
   view. At `begin_frame`, when the outgoing tree declared one,
   the core takes the keys from the hovered node, the pressed one and
   the focused one up to the root (`note_reveal_anchors`). Those are
   the nodes the last hit test found in that very tree, so the walk is
   exact. A node opening asks whether its anchor's key is among them. A
   frame that declares none walks nothing (`Tree::any_reveal`).

## What was declined, and why

- **A named group (`revealGroup: "row-3"`, shown while that hover group
  is hovered).** It would reach a node that is not inside the row, but
  every member of the group has to join it by name. The stock button
  cannot (its rows are closed), and a name per row is model the app has
  to keep. The condition that would build it: a design whose revealed
  controls live outside the row they belong to.
- **`display: none` when hidden.** The row would reflow under the
  pointer as it crossed, the access tree would lose the controls, and
  Tab could not reach them. A design that wants the room back can
  declare the actions as a float.
- **A `hoverStyle` group — any paint row swapped on hover.** That is
  CSS's `:hover` in general and a larger design. Opacity is the one row
  this needs, and the flag leaves the rest open.

## Consequences

- An app's secondary actions are one row on their box. For the Cookie
  Mapping Builder, `revealOnHover` on the group header's button row and
  on each mapping row's actions, and the quiet always-visible buttons
  go.
- A screen-reader user and a keyboard user meet the controls as before:
  in the tree, in the ring, shown when focused.
- Every binding gets it from the schema: a row (id 141) Node and Lua
  read automatically, `KuiSpec.reveal_on_hover` (ABI 35, 800 bytes on
  64-bit), Odin generated, the corpus scene's `Stop` control in all
  five adapters.

## What the building changed

- The anchor test is `hover_tracked`, the predicate that already decides
  whether a node gets a hit region, so "an ancestor that tracks the
  pointer" and "an ancestor the hit test can find" are one rule, and the
  `reveal-without-anchor` check walks the same way.
- The first tests used a clickable row (`onClick`, no role) and found
  the stock button inside it missing from the access tree. That is not
  the reveal: a box with `onClick` reads as a button named by its
  content, and a control inside a button is part of its name. A row
  that carries controls of its own wants `hoverable` (or a role of its
  own) instead, and the howto entry says so.
- With a `transition`, the frame that first sees the hover starts the
  tween from hidden and the frames after it draw the way up, as a
  `hoverBg`'s tween does; nothing about the reveal needed its own
  easing.
