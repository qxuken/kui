---
status: accepted
date: 2026-09-03
---

# Modal surfaces: one modal in effect, focus and input contained

`Role::Dialog` was declarable and meant nothing: a dialog announced itself
to a screen reader and then let Tab walk straight out of it, let the
pointer press the buttons behind it, and had no convention for being
dismissed. We decided that **modality is one more row** — `modal`, a
`Kind::Tag` like `onKey` and `onHover` — that a frame has **at most one
modal in effect** (the last node declaring it, in tree order), that the
Tab ring, hit testing, hover, the wheel and assistive-technology
activation are all confined to that node's subtree, that the focus
outside is remembered and given back when the modal goes away, and that
Escape or a press outside emits `{kind:"dismiss", reason}` on the modal
node — an event the app acts on, since the view is data and only the app
can stop declaring the dialog.

## Context

- ADR 0002 settled the focus model: one focus in the core, a Tab ring
  over every control, computed by `Core::focus_ring` from the last
  frame's tree. That ring walks from index 0 and skips only `role="none"`
  subtrees and disabled nodes. Nothing scopes it, and nothing exposes it,
  so no binding and no app can contain focus inside a dialog: the ring is
  core-private state, which is exactly the test `docs/BACKLOG.md` uses
  for "belongs in the core" (C1).
- The pointer had the same hole from the other side. Hit regions are
  emitted for the whole tree in paint order (`Core::finish_frame`), and
  `Interaction::hit_at` takes the topmost. A dialog drawn over an app
  covers it visually and blocks nothing: the buttons underneath still
  hover, still press, still take focus on click, and a screen reader's
  `Click` on one still resolves, because `click_node` searches the same
  hit list.
- An app can fake *some* of this and not the rest. A full-viewport
  `onClick` box behind the dialog swallows presses, and that is what
  every kui example that wants a scrim already does — but it does not
  contain Tab, does not stop the wheel, does not stop assistive
  technology, and does not tell the platform anything. The part that
  cannot be faked is the part that matters most to the users modality
  exists for.
- AccessKit has had the answer since we adopted it: `Node::set_modal`,
  ARIA's `aria-modal`, which is how a reader learns to keep its cursor
  inside a dialog. The core had nothing to set it from.
- Escape already had a meaning — `set_focus(None)`, "let go of this
  control" — and nothing above it. Every platform's dialog closes on
  Escape, and a menu closes on a press outside; both are conventions the
  core is in the only position to detect, and neither can be a core
  *action*, because closing a dialog means not declaring it next frame,
  which is the app's decision.
- The layout already has what a dialog needs to sit over the app:
  floats escape ancestor clips and paint (and hit-test) after all
  in-flow content. Modality does not need a new painting concept, only a
  new input and focus concept.

## Decision

1. **`modal` is a row, and its kind is `Tag`.** `modal` (id 64,
   `Kind::Tag`) joins `PROPS`, so JSX, Lua and Node get it from the
   schema and C gets a `KuiSpec.modal` field, pinned like every other row
   by `every_schema_prop_has_a_c_counterpart`. `Tag` rather than `Flag`
   because a modal needs a way to route its dismissal to an app whose
   messages are a typed union — and a null tag (`modal={null}` in JSX,
   `kui_value_null()` in C; in Lua any value, as for every tag row)
   declares the behaviour with no `tag` on its events, exactly as the
   null-tag rule already settled for `onKey` and `onHover`. One row, one
   concept — not a flag plus an `onDismiss` that can disagree with it.
2. **One modal in effect: the last one declared.** The frame's active
   modal is the last node in tree order whose spec declares `modal`; its
   subtree (`[i, subtree_end(i))`) is the modal scope. Tree order is
   declaration order, so a confirm nested inside a dialog and a confirm
   declared as a later sibling both resolve the same way — the newer one
   wins, and the dialog under it is as inert as the app under the dialog.
   That is a stack without a stack: nesting falls out of the tree the
   view already builds, and no push/pop API can drift out of step with
   what is on screen.
3. **The Tab ring is the modal scope.** `focus_ring` walks the modal
   subtree instead of the whole tree, skipping `role="none"` subtrees and
   disabled nodes as before; Tab and Shift-Tab wrap inside it. At the end
   of a frame, focus that is not inside the scope is pulled to the first
   focusable node in it — and dropped when the modal holds none, so an
   empty dialog contains focus by holding none. That entry keeps the
   visibility focus already had: a dialog opened by Tab-and-Enter shows
   its ring on its first control at once, one opened by a click shows
   nothing until the first Tab — the same rule browsers apply to
   `showModal()`, and the one that tells a keyboard user where they are
   without moving them off the first control. The access tree reports
   the moved focus either way, so a screen reader's cursor follows into
   the dialog immediately.
4. **The focus a modal displaced comes back.** The focus at the moment a
   node *starts* declaring `modal` is remembered with it; when that node
   stops being declared, focus returns exactly there — including to
   nothing, when it displaced nothing or when the key it remembered has
   left the tree, since the alternative is focus left on a button that is
   not on screen any more. Only the core can do this: in a view that is
   data there is no moment for the app to capture "what was focused
   before this dialog existed", and without it every dismissed dialog
   drops the keyboard user back at the top of the document.
5. **Everything outside the scope is inert.** Nodes outside emit no hit
   region, so they cannot be clicked, dragged, hovered, pressed, focused
   by a press, or activated by Enter, Space or an assistive-technology
   `Click` — all of which resolve against the same hit list. Their scroll
   containers keep their scrollbars *drawn* (a background that visibly
   loses its scrollbar reads as a bug) and refuse the wheel and thumb
   drags. **Window chrome is the exception**: a `window` node stays live,
   so a modal dialog does not trap the window itself — it can still be
   moved, minimized and closed, and a host mirroring hit regions into
   `WM_NCHITTEST` keeps seeing them. A press on it is the platform's, not
   the app's, so it leaves the modal's focus where it is rather than
   blurring it the way a press on the background otherwise would.
6. **Escape and an outside press emit `dismiss`.** While a modal is in
   effect, `Escape` emits `{kind:"dismiss", reason:"escape", tag}` on the
   modal node and does nothing else — no blur, no editor escape; and a
   press that lands on no hit region emits `{kind:"dismiss",
   reason:"outside", tag}` and neither moves nor drops focus. Declaring
   `modal` implies a hit region on the node itself, so its own background
   is not "outside". The core closes nothing: the app stops declaring the
   node, on the frame it decides to — which is what makes "Escape asks
   for confirmation" and "a menu closes, a dialog does not" the app's
   call rather than the core's. A focused `onKey` sink still receives
   Escape as a `KeyDown` the way it receives every key; the dismiss is
   the *modal's* event, not the focused node's.
7. **The access tree says modal, and derives the role.** `AccessNode`
   gains `modal`, set on the node in effect only (the dialog under a
   confirm is not the modal any more, and saying so twice would point a
   reader at the wrong one); `access_bridge` calls
   `Node::set_modal`, so AccessKit reports `aria-modal` to VoiceOver,
   Narrator and AT-SPI. A node declaring `modal` with no `role` derives
   `Role::Dialog`, the way `on_click` derives `Role::Button`. The tree is
   not pruned: it keeps mirroring the frame, and the modal flag is what
   the platform layers are built to act on.
8. **Painting is unchanged; a float is how a modal covers the app.**
   Modality is about input, not paint order. A dialog that must draw over
   the app is a float (`float="viewport"`), which already escapes clips
   and paints last. A modal that is *not* in a float and has content
   painting after it is a silent misconfiguration — inert underneath
   something the user can see — so it is a warning,
   `modal-behind-content`, not a reordering rule the layout would have to
   carry forever.

## Considered options

- **A `modal` flag plus an `onDismiss` row.** Rejected: two rows for one
  concept, which can be declared apart (`modal` with no handler is a
  dialog nothing can close from the keyboard; `onDismiss` without
  `modal` is dead), and `Kind::Tag` exists precisely so a behaviour and
  its tag are one declaration.
- **The core closes the modal itself.** Rejected: it would put retained
  "is the dialog open" state in the core, in a system where what is on
  screen is the frame the app declared. It also forecloses the cases that
  make dialogs worth having — a confirm before discarding, a form that
  refuses to close while it is saving.
- **An imperative modal stack (`ui.push_modal` / `pop_modal`).**
  Rejected: a second source of truth beside the tree, which then has to
  be reconciled with it every frame. The tree already orders the dialog
  and the confirm inside it; "the last one declared" reads that order.
- **The first modal wins instead of the last.** Rejected: it inverts
  nesting. A confirm raised from inside a dialog is declared inside it,
  so under a first-wins rule the confirm would be the inert one — the
  dialog would block the thing it just raised.
- **Every declared modal marked modal in the access tree.** Rejected for
  the same reason: two `aria-modal` nodes tell a reader to confine itself
  to the outer one, which is where the user is *not*.
- **Inertness by geometry — a rect that swallows what is under it.**
  Rejected: modality is a subtree, not a rectangle. A dialog's own menu
  or tooltip floats outside its rect and must stay live, and a dialog
  smaller than the content it blocks is the normal case, not the odd one.
- **Filtering in `Interaction` instead of at emission.** Rejected: it
  would need the same modal test at `hit_at`, `refresh_hover`, the drag
  path, `click_node` and the access path, and any new query would forget
  it. Not emitting the region is one place, and every consumer inherits
  it.
- **Pruning the background out of the access tree.** Rejected: the tree
  is a mirror of the frame (ADR 0001), the hash-based diffing depends on
  that, and AccessKit's modal flag is the supported way to say this. A
  reader that ignores the flag can still review the page, which is what
  its users expect of it.
- **Entering a modal always without a visible ring.** Rejected: the
  ring exists to tell a keyboard user where focus went, and a dialog they
  opened from the keyboard is exactly when it moved without them.
  Clearing it there would show nothing until a Tab press, which also
  moves them off the control the dialog opened on.
- **Containing focus only from the first Tab.** Rejected: it leaves the
  first Tab press free to leave the dialog, and leaves a screen reader
  cursor sitting outside a modal that has just opened.
- **Making window chrome inert too.** Rejected: the window's controls
  belong to the platform, not to the app's dialog. A modal that traps the
  window is a hung app to everyone outside it.
- **Restoring focus in the app instead of the core.** Rejected: see
  decision 4 — in a data view there is no moment to capture it, and the
  core is the only place that knows what focus was before the frame that
  declared the dialog.

## Consequences

- Dialogs, popovers and menus become buildable in every binding from one
  row: a floated subtree with `modal` on it contains Tab, blocks the app
  behind it, tells the platform it is modal and reports the two dismiss
  gestures as data. With C2 (a secondary mouse button) the same subtree
  is a context menu.
- Escape no longer blurs a focused control while a modal is up. Nothing
  in the examples relied on it, and outside a modal it behaves exactly as
  ADR 0002 left it.
- A modal that is not in a float will usually be wrong, and now says so
  (`modal-behind-content`) instead of looking like inert-behind is broken.
- `dismiss` joins `EVENTS`, `docs/props.md` and the TS `CoreMsg` union,
  so an app whose messages are a typed union gets the new variant checked
  rather than discovered.
- Headless tests pin the scope of the ring, containment and its return,
  the inert hit list (click, hover, wheel, Enter/Space, an
  assistive-technology `Click`), live window chrome, nesting, both
  dismiss reasons, the access flag and the derived role.
- The examples gain a modal: `cargo run -p kui --example accessibility`
  has a "Delete…" button that opens a confirm dialog, which is what a
  VoiceOver session and the pre-release macOS audit can be pointed at.
  `scripts/ax-audit.swift` has no modal section: what `set_modal` does to
  a reader's cursor is a session observation, and every check in that
  script is one we have actually asserted.
- Not done here, and each small: a scene in the binding-parity corpus
  (`modal` is a schema row, so all four transports lower it mechanically;
  a scene would pin the *behaviour* the way the `tooltip` scene does), a
  `modal-without-name` warning next to `control-without-name` (a dialog
  should be labelled), the ARIA composite patterns ADR 0002 deferred
  (arrow keys inside a menu), and initial focus placement (`autofocus` on
  a control, so a dialog can open on its Cancel button rather than its
  first).
