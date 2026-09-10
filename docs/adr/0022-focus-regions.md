---
status: accepted
date: 2026-09-10
---

# Focus regions: a ring of its own, entered on purpose

> **Accepted (2026-09-10), built the same day.** The devtools dock is the
> first region; what the building changed is under *Consequences*.

ADR 0002 gave kui one focus and one Tab ring over every control. That is
the right default for an app, and the wrong shape for the thing that sits
*beside* an app: a devtools dock, a command palette's shell, an inspector
panel. Those have controls of their own — tabs, buttons, a tree — and a
user pressing Tab in the app they are looking at must never land in them.
Today the only way to keep them out of the ring is `role="none"`, which
also hides them from assistive technology and leaves their controls
unreachable by keyboard at all. We decided that a node can declare its
subtree a **focus region**: a Tab ring of its own that the main ring never
enters, entered by a call (`focus_region`), a press, or an explicit focus,
and left the same ways — and, in the same round, that an `autofocus`
editor takes focus once, when it appears, rather than every time nothing
holds it, and that a key sink on the root hears the keys nothing claims
even when nothing is focused, so a shell no longer has to take focus to
have somewhere for its chords to land.

## Context

- `crates/kui-devtools` and `examples/node/devtools.tsx` draw a dock
  beside every example. The dock is `role = none`, with the comment "so
  neither the Tab ring nor assistive technology sees it — the example's
  access tree is the example's". The first half is the goal; the second
  is the price, and it is not a price anyone chose: a screen-reader user
  cannot reach the dock's tabs, and a keyboard user cannot reach them
  either, ring or no ring.
- Both devtools take focus on the root when nothing is focused ("somewhere
  for the chords to land"), from the second frame so an example's own
  autofocus gets the first, and the Rust one carries a `step: Option<bool>`
  field to hand a Tab that landed on the root back to the ring. Three
  workarounds for one missing rule: with no focus, `key_target` resolves
  nothing, so a root sink hears nothing.
- `autofocus` on an editor is checked every frame: `opts.autofocus &&
  self.focus.is_none()`. An app with an autofocus field cannot have
  *nothing* focused for longer than a frame — a click on the background
  blurs, and the field takes focus straight back. ADR 0002 decision 5
  described it as "takes focus only while nothing else holds it", which
  reads as a one-time courtesy and behaves as a standing claim. `keyFocus`
  was made edge-triggered by that same ADR for exactly this reason;
  `autofocus` was left as it was.
- The modal scope (ADR 0003) already narrows the ring to one subtree, and
  its machinery — a range, a remembered focus, an entry rule with
  `initialFocus`, a restore on the way out — is most of what a region
  needs. What a modal adds that a region must not is inertness: the app
  behind a dialog is dead to the pointer, and the example beside a dock is
  not.

## Decision

1. **`focusRegion` is a flag row on a container.** Its subtree is a Tab
   ring of its own, and the ring outside it (the *main* ring: the tree
   minus every region) skips the whole subtree. A region inside a region
   is skipped by the outer's ring the same way, so each ring is one
   region's subtree minus the regions nested in it. Nothing else about the
   node changes: it lays out, paints, takes the pointer and appears in the
   access tree exactly as before. Schema id 92; `InteractSpec::focus_region`
   (cold: read only when a frame declared one, gated by `Core::any_region`
   the way `any_modal` gates the modal scope).
2. **One region is in effect, and Tab walks only its ring.** `Core::region()`
   names it (`None` is main). Tab and Shift-Tab step through that ring and
   wrap inside it; no Tab press crosses a region boundary in either
   direction. With nothing focused, Tab enters the ring of the region in
   effect, not the main ring — the user who pressed Escape on a dock
   button and then Tab is still in the dock.
3. **The region follows focus, and a press settles it.** `set_focus(key)`
   makes the region the one enclosing `key` (main when none does), so an
   explicit `focus(key)`, an assistive-technology `Focus` request, a
   `keyFocus` declaration and a click on a control all move the ring to
   where focus went. A primary press that focuses nothing — dead space, or
   a press inside a key sink that keeps the keyboard — still settles the
   region on the one enclosing the pressed node, so a click into an empty
   part of the dock followed by Tab enters the dock's ring — and the
   settle *holds* while that focus sits still, since ADR 0011 gives the
   dead-space press to the enclosing sink, which may be the root outside
   every region; the next focus that moves takes the region with it
   again. Focus itself is never confined: a region scopes the *ring*, and
   nothing else.
4. **`focus_region(target)` enters one on purpose.** `Ui::focus_region(key)`
   (`ctx.focusRegion(name)`, `env.focus_region(name)`, `kui_focus_region`)
   moves focus into the region `key` names — or to main, for `None` /
   `null` / `nil` / `0` — landing on the focus that region last held if
   that node is still in the frame, else on its ring's `initialFocus`,
   else on the ring's first stop; a region with an empty ring is entered
   with nothing focused. The landing shows (`focus_visible`), like a Tab
   step: the call exists to be bound to a chord, and a chord that moves
   focus somewhere the user was not looking owes them the ring. Like
   `focus_next` it is **deferred to the end of the frame being built**, so
   the frame that first draws the dock and asks to enter it does both, and
   an `update` that runs between frames may name a node the last frame
   did not have. Node and the core take the *label* for that reason
   (`focusRegion('devtools')`, held until the frame resolves it, the way
   `setEditText` by label is); a frame that then declares no region under
   the name, or a key that is not a region, raises
   `focus-region-without-node` and moves nothing.
5. **A region that goes away hands focus back.** When the region in effect
   stops being declared — the dock toggled off with focus inside it — the
   region becomes main and focus becomes what main last held, if that node
   is still there, else nothing. The same shape as the modal restore, with
   one remembered focus per region rather than one per modal; the gone
   region keeps its memory, so a dock toggled off and on again is entered
   where the user was.
6. **A modal wins.** While a modal scope is in effect the ring is the
   modal's subtree, wherever that subtree sits, with nested regions skipped
   as the main ring skips them; the region in effect is not consulted. A
   modal opened from inside a region hands focus back into it when it
   closes (ADR 0003's restore), and decision 3 makes the region follow.
7. **Keys and the pointer do not know regions exist.** Unclaimed keys
   bubble to the nearest enclosing sink through a region boundary (ADR
   0011 is about sinks, not rings), so an app's root keymap still hears
   the chord that toggles the dock while focus is in the dock; a region
   that wants a keymap of its own declares `onKey` and is a sink like any
   other. The pointer, the wheel and assistive technology treat the region
   as the plain node it is.
8. **A root sink hears what nothing claims, focus or no focus.** With no
   node focused, a key had no target and was dropped; a shell that wanted
   its chords to work put focus on the root. The root encloses every node,
   so a sink on it is already the sink every unclaimed key bubbles to (ADR
   0011); with nothing focused there is nothing to claim, and the same
   sink hears the press. Not under a modal, where the root is inert. Tab
   is unaffected — with nothing focused it enters the ring, as before.
9. **`autofocus` is an edge.** An editor takes focus on the frame its
   `autofocus` declaration *starts* — a new editor, one back after a gap,
   or one whose flag just turned on — and only while nothing holds focus;
   redeclared every frame it asks nothing more. `EditStore::declare`
   reports the edge. What the user did with focus afterwards — blurred it,
   moved it — stands. The same edge `keyFocus` has had since ADR 0002,
   with the "only while nothing holds it" courtesy kept.

## Considered options

- **Keep `role="none"` on the dock and give it a keymap instead.** Rejected:
  it makes the dock's buttons unreachable by keyboard and invisible to a
  screen reader, which is the state the devtools are in today. A panel that
  is drawn is a panel that should be operable.
- **A region as a modal without inertness — a `modal` variant.** Rejected
  on the name: a modal is a claim about the rest of the surface (inert,
  dismiss on outside press, Escape), and a region makes none of those. The
  machinery is shared where it is the same thing (a range, a remembered
  focus, `initialFocus`, a restore), and the row is a different word
  because it is a different promise.
- **Regions reachable by Tab as one stop each (a "group" the ring enters
  with Enter).** Rejected: it puts the dock back in the app's ring, which
  is the thing the user asked to keep it out of. A region is entered on
  purpose — by a call bound to a chord, or by a click — and never by
  walking.
- **Make `focus_region` immediate, against the last frame.** Rejected for
  the same reason `focus_next` is deferred and for one more: the primary
  caller toggles the region on and enters it in the same `update`, and the
  last frame does not have it. The label form follows from that — a key
  for a node that does not exist yet is not something the caller can
  hold.
- **Stop keys at the region boundary, as a modal does.** Rejected: the
  chord that leaves the dock is the app's, on the root, and the dock is
  not a dialog whose shortcuts must not leak. An app that wants the
  boundary has `onKey` on the region.
- **Leave `autofocus` alone and let apps use `keyFocus`.** Rejected: the
  standing claim is a bug in the field — an app with an autofocus field
  cannot blur — and every editor example in the repo carries the row.
  `keyFocus` is the unconditional edge; `autofocus` is now the polite one.
- **Have the root sink hear keys with no focus only when it declares
  something.** Rejected: there is nothing to declare. A sink on the root
  is a sink that asked for every unclaimed key in the tree; the empty
  focus was the one case it did not get them, and no example wanted that.

## Consequences

- The devtools dock is a `focusRegion` named `devtools`, without
  `role = none`: a Ctrl+Shift+I chord toggles focus between the dock and
  the example, Tab inside the dock walks its icons, tabs and tree rows,
  and a screen reader can reach it. The root no longer takes focus, so
  `frames >= 2` and `take_key_focus(Key::ROOT)` are deleted — decision 8
  is what made them unnecessary. The Rust harness's `step` hand-off stays:
  a press on dead space still gives focus to the enclosing sink (ADR
  0011), and a sink holding focus keeps every key (ADR 0002), so the Tab
  it hears is still its to hand on — and the step walks the ring in
  effect, which decision 3 made the one under the pointer.
- **What you can delete**: a `keyFocus` / `take_key_focus` on the root
  whose only purpose was giving chords a target when nothing was focused;
  any guard that kept an `autofocus` editor from retaking focus after a
  blur.
- **What changes under you**: an app that relied on its `autofocus` field
  retaking focus after every blur now sees the blur stand, and calls
  `focus(key)` when it wants the field back. An app with a root sink and
  nothing focused now hears keys it used to lose; none of the examples
  minded, and a sink that heard nothing was not a sink anyone tested.
- `KuiSpec` grows `focus_region` at its tail, an [in] append — no ABI
  bump. `kui_focus_region(ctx, key)` and `kui_region(ctx)` are new
  functions; `ctx.focusRegion(name | null)` / `ctx.region()` in Node,
  `env.focus_region(key | nil)` and the `env.region` reading in Lua.
- Headless tests pin the ring's confinement in both directions, the entry
  order (remembered, `initialFocus`, first), the press settling the
  region, the restore when a region goes away, the modal winning, keys
  bubbling through the boundary, the root sink with no focus, and the
  autofocus edge. No conformance scene: the row lowers mechanically in
  every transport, and the behaviour is the core's alone.
