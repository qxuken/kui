---
status: accepted
date: 2026-09-15
---

# A drop zone is a row, and the files are an event

> **Accepted (2026-09-15), built the same day** — what the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed); backlog C40. Asked for as a feature: "you
> declare a drop zone on a box and you get events once it happens".
> Today nothing in the tree hears a file dragged in from the Finder —
> winit reports it, the runner drops it on the floor, and no binding has
> a spelling for it. This document makes it one row and one event: **any
> node declaring `onDrop` is a zone; the files hovering it, moving over
> it, leaving it and landing on it are four phases of one `drop` event
> on that node, and `dropBg` lights the zone while they hover, the way
> `hoverBg` lights a hovered node.** The part winit does not supply — the
> pointer's position during a drag — is the driver's to find, and on
> macOS the runner finds it by overriding winit's window delegate, the
> way it already overrides its view for force clicks and dictation.

## Context

### What winit reports, read from 0.30.13

`WindowEvent` has three file events (`src/event.rs:176`): `HoveredFile(PathBuf)`,
one per file when the drag enters the window; `HoveredFileCancelled`, once,
when it leaves; `DroppedFile(PathBuf)`, one per file on release. None
carries a position. The macOS backend (`window_delegate.rs:369–429`)
registers the *window delegate* for `NSFilenamesPboardType`, implements
`draggingEntered:` (queues the hovers, answers `NSDragOperationCopy`),
`performDragOperation:` (queues the drops) and `draggingExited:`, and does
not implement `draggingUpdated:` at all — so between entering and
releasing, the window hears nothing, and at no point does it hear
*where*. During an OS drag the pointer is the drag session's: no
`CursorMoved` arrives either, so the core's `cursor()` is wherever the
pointer was before the user picked the files up, possibly in another
window. The Windows backend (`drop_handler.rs`) implements `IDropTarget`
itself and discards the `POINTL` every method is handed; X11's XDnD
discards `XdndPosition`'s coordinates the same way.

A *zone* is a box, and a box is a place. Without a position there is no
zone — only "the window was dropped on". winit master's redesign
(`DragEnter { paths, position }`, `DragOver { position }`, `DragDrop`,
`DragLeave`) is the shape this document wants, and it is not in a release
kui can pin.

### What the core already has for "a place under the pointer"

Everything but the input is built. A node with an interaction prop
registers a `HitRegion` (`runtime/emit.rs:80`, `NodeSpec::hover_tracked`),
regions are in paint order, `Interaction::target_at` finds the topmost
one under a point by that order and nothing else (ADR 0023, decision 4),
a region behind a modal is not emitted (`emit_node`'s `interactive`), a
disabled node emits one with its payloads stripped. Two rows already
resolve "the nearest enclosing declaration" at emission:
`context_menu` walks up to the node that offers a menu
(`enclosing_menu`, ADR 0011's reasoning applied to the pointer), and a
key sink is found by the same walk. `hoverBg` is picked when the node
opens, from `is_hovered(key)` (`builder.rs:246`), so a data-only view
gets the swap and `transition` eases it. `on_hover` prepares its leave
event at enter because the region may be gone from the next frame's
hits (`hovered_leave`).

So the core needs: one row, one interaction state (the zone the files
are over), one resolver, three input events, and a background pick.

### How the neighbours do it, and the one trap

HTML: `dragenter`/`dragover`/`dragleave`/`drop` on any element, and the
best-known defect of the model — an element the app shows *in answer
to* `dragenter` (an overlay saying "drop here") is a new element under
the pointer, which fires `dragleave` on the zone, which removes the
overlay, which fires `dragenter`: the flicker every drop-zone library
exists to hide, with a counter of enters minus leaves. AppKit:
`NSDraggingDestination` on a view, `draggingUpdated:` answering an
operation that *is* the cursor (the green plus over a place that takes
the files, the not-allowed circle over one that does not), a refused
operation on release meaning the drop never happens and the icon slides
home. Qt: `setAcceptDrops`, then `dragEnterEvent`/`dragMoveEvent`/
`dropEvent` per widget, `event->acceptProposedAction()` per event.
GTK4: a `DropTarget` controller on a widget with the types it takes.

The trap to design against is HTML's; the thing to keep from AppKit is
that the answer *whether this place takes the files* is what the cursor
shows and what a release does.

## Decision

### 1. `onDrop` on any node makes it a zone; four phases of one event

A node declaring `onDrop` (Node/JSX), `on_drop` (Rust, Lua, and the
`on_drop` argument of C's `kui_open_with`) is a *drop zone*. It emits,
with the row's payload under `tag`:

- `{kind:"drop", phase:"enter", paths, x, y, tag}` — the dragged files
  came over it;
- `{kind:"drop", phase:"move", paths, x, y, tag}` — they moved over it
  (a list that shows an insertion mark between rows needs this; an app
  that does not wants it as much as it wants `drag`'s `move`);
- `{kind:"drop", phase:"leave", paths, tag}` — they left it, for another
  zone, for no zone, or out of the window;
- `{kind:"drop", phase:"drop", paths, x, y, tag}` — they landed on it.
  A drop ends the hover: **no `leave` follows a `drop`** (HTML's order,
  and the one an app keeping a "files are over me" flag does not have to
  special-case — the flag clears on either).

`paths` is a list of strings, the OS paths as the driver reported them
(a non-UTF-8 path is lossy; the driver may not turn a `file://` URL into
anything else). `x`, `y` are the pointer in the zone's viewport
coordinates, as `contextmenu`'s are. The row implies hover tracking
(the zone has to be a region to be found), like `onHover`.

### 2. The zone under the files is the topmost *zone*; what is no zone is transparent

The core resolves the zone at a point as: the topmost hit region under
the point (paint order, `target_at`'s rule) **whose resolved `drop` is
some** — where a region's `drop` is its own `onDrop` or the nearest
enclosing node's, resolved at emission exactly as `context_menu` is
(the walk stops at the modal boundary and skips a disabled node's own).
Two consequences, both on purpose:

- **A button inside a zone is the zone.** The topmost region under the
  pointer is the button's; its `drop` resolves to the enclosing zone.
  "A container declaring a zone for everything inside it" is the common
  case, as it was for menus.
- **An overlay the app shows on `enter` cannot make the zone lose the
  files.** A float with no `onDrop` of its own and none enclosing it
  has `drop: None`, and the resolver looks *past* it to the zone
  beneath. HTML's flicker cannot happen: what changes the target is
  another zone or the edge of every zone, never a node that takes no
  files. A modal over the zone is not looked past — the zone's region is
  not emitted while the modal is up.

Nothing is re-resolved when a frame lands. The zone is re-resolved on
the driver's next `DragFiles`, and the `leave` for the current zone is
prepared at `enter` (like `hovered_leave`) so a zone the app stops
declaring mid-drag still gets its leave. A new frame that removes the
zone under a still pointer keeps the state until the driver reports the
next position, which on macOS is within one `draggingUpdated:` — sent
periodically by AppKit while the drag is inside the window, whether or
not the pointer moved.

### 3. `dropBg`: the zone lights up without the view keeping state

`dropBg` (`drop_bg`) is the background while the dragged files are over
this node — resolved when the node opens, from
`Interaction::is_drop_target(key)`, in the same pick as `hoverBg`: it
wins over `pressedBg`, `focusBg` and `hoverBg` (a press cannot be held
while the OS holds a drag, and a lit focus ring under a drop highlight
would be noise), and `transition` eases it. It clears on `leave`,
`drop` and cancel. Implies hover tracking, like `hoverBg`. No
`dropGroup`: a zone is one box; a two-piece zone is a container
declaring the row.

### 4. Three input events; the position is the driver's, the resolution the core's

```rust
InputEvent::DragFiles { paths: Vec<String>, at: Vec2 }  // enter and move alike
InputEvent::DropFiles { paths: Vec<String>, at: Vec2 }
InputEvent::DragCancel
```

`DragFiles` is one event for entering and moving — the core tells enter
from move by whether the resolved zone changed, and a change from one
zone to another is that zone's `leave` then the new one's `enter`, in
that order. `DropFiles` resolves the zone at `at` and emits `drop` on
it; with no zone there, nothing — and with a zone lit that is not the
one under the point (a headless drive that never sent `DragFiles`),
the lit one gets its `leave` first. `DragCancel` is the files leaving
the window or the OS ending the drag elsewhere: the lit zone's
`leave`, then nothing lit.

The core keeps the paths of the current drag so `leave` can carry them
without the driver repeating them. It never keeps a path past the
event — a `Value` list on the wire, owned by whoever hears it.

After each of the three, `Core::drop_target() -> Option<Key>` reads
what is lit, which is what a driver answers the OS with (decision 5)
and what a test reads to say a zone was found.

### 5. The runner: macOS overrides winit's window delegate; the others take winit's events at the OS cursor

**macOS.** `crates/kui/src/macos_drop.rs` overrides, on winit's window
delegate class at runtime (the `macos_text_input` mechanism — once per
process, `method_setImplementation` where winit had the selector,
`class_addMethod` where it did not), the five `NSDraggingDestination`
selectors: `draggingEntered:`, `draggingUpdated:` (winit has none),
`draggingExited:`, `performDragOperation:`, `draggingEnded:`. Each reads
`draggingLocation` (window coordinates, bottom-left origin — converted
through the content view and flipped to the logical top-left the core
uses) and, on enter and perform, the pasteboard's file URLs; queues one
message per call in a per-window stash; wakes the loop through the
`Waker`. winit's own implementations are *replaced, not wrapped*: had
they run, `HoveredFile` and `DroppedFile` would arrive a second time
with no position, and the runner would have to know to drop them. On
macOS, the runner ignores winit's three file events outright.

The operation each method answers — `NSDragOperationCopy` over a zone,
`NSDragOperationNone` elsewhere — is read from a per-window stamp the
runner writes after every dispatch (`Core::drop_target().is_some()`),
because the delegate runs inside AppKit's callback where the `App` is
winit's to borrow. So the cursor and a refused release are **one
`draggingUpdated:` late** — the same latency as the hover itself, and
AppKit sends the update periodically, so a stale answer never outlives
a frame or two. `draggingEntered:` answers Copy before any stamp exists:
an optimistic first answer, corrected on the first update, is better
than a not-allowed cursor over a zone for a frame.

`draggingEnded:` fires after a refused release (no `performDragOperation:`)
and after an exit; the runner treats it as `DragCancel` when no drop
was delivered for this drag, so a release off every zone un-lights the
last one it was over. `registerForDraggedTypes` stays winit's: file
names are the one type this document takes.

**Windows and Linux.** winit's three events, as they are: hovers
collected per batch (they arrive one per file in one message) and
dispatched as one `DragFiles` at the end of the batch, drops the same,
the cancel as `DragCancel`. The point is the OS cursor asked at that
moment where the platform can be asked (`GetCursorPos` +
`ScreenToClient` on Windows) and the pane's last reported cursor where
it cannot (X11, Wayland) — with no `draggingUpdated:` equivalent, a zone
on those platforms is *the zone under the pointer when the drag entered
and when it was released*, and `move` never fires. Honest, and filed
(the true position from an own `IDropTarget` and from `XdndPosition`)
as the Windows round's next entry rather than built blind; W16's rule
applies, nothing Windows-only is verified from here.

### 6. Doors: three injections, one reader, one row, one scene

- **Rust**: the three `InputEvent`s through `Core::handle_input`;
  `Core::drop_target`; `NodeSpec::on_drop`, `NodeSpec::drop_bg`.
- **Node**: `Ctx.dragFiles(paths, x, y)`, `Ctx.dropFiles(paths, x, y)`,
  `Ctx.dragCancel()` (a `KuiWindow` refuses injection, as for every
  input); `dropTarget()` on both; `onDrop` and `dropBg` props in the
  encoder and `jsx-runtime.d.ts`; the `drop` event typed in
  `index.d.ts`. The `Core::handle_input` row of `DOORS` already reads
  "one per `InputEvent`" and stays true; `drop_target` is a new row.
- **Lua**: `on_drop` / `drop_bg` props; `env.drop_target()`; no
  injection (a guest, `GUEST`).
- **C**: `kui_input_drag_files(ctx, const KuiStr *paths, size_t count,
  float x, float y)`, `kui_input_drop_files(…)`, `kui_input_drag_cancel(ctx)`,
  `kui_drop_target(ctx) -> u64`; `on_drop` and `drop_bg` appended to
  `KuiSpec` — an [in] append, so **ABI 17 → 18** (AR50's rule).
- **Schema**: `onDrop` and `dropBg` rows in `PROPS` with the four
  spellings; `props.md` regenerated; the `drop` event documented beside
  `hover`.
- **Corpus**: `Step::DragFiles(n, x, y)`, `Step::DropFiles(n, x, y)`,
  `Step::DragCancel` — `n` files spelled `/drop/1.txt … /drop/n.txt` by
  every adapter (integer discipline), replayed in all four; a `drop`
  scene with a zone, a button inside it, an overlay the phase-2 view
  shows on enter, a second zone, and a modal over the first in phase 3,
  pinning decision 2's three cases and the lit background.
- **Devtools**: `"drop"` in the inspector's event-tag list; the events
  tab needs nothing.
- **Example**: `examples/rust/features/drop.rs` (ADR 0021: one
  subject) — a zone that lists what landed, its `dropBg`, and the
  overlay case, with a headless drive in the manifest; the Node twin.

## Considered options

**A. Resolve the zone as the topmost region, then walk up (the
context-menu rule alone).** Fails decision 2's second case: an overlay
with `hoverable` or `onClick` shown on enter is the topmost region, has
no zone above it, and the zone gets `leave`. Chosen instead: the walk
*and* looking past regions that resolve to no zone. The cost is one
more filter in a `rposition` that already runs once per pointer event.

**B. Re-resolve on every frame, as hover does.** Would make an overlay
the app shows on enter — even a plain box, no region — irrelevant,
since only regions are looked at; but a *zone-shaped* answer to enter
(a second zone appearing, say "drop as copy | drop as link") would
switch targets under a still pointer, and the leave/enter pair would
arrive with no driver event between them. Driver events only: the
core reports what it was told, when it was told.

**C. Wait for winit master's `DragEnter { position }`.** The right
shape, no release date, and a winit bump is a whole round on its own
(every override in the runner is against 0.30's class layout). Chosen:
the same override mechanism the runner has used twice, contained in one
file, replaced by winit's events when the bump comes.

**D. Wrap winit's delegate methods rather than replace them.** The
text-input override wraps; here the originals queue the very events the
override supersedes, and a runner that both hears the override's
message and winit's `HoveredFile` has two sources for one fact. Replace.

**E. `onDrop` carrying arbitrary pasteboard content — text, URLs,
images.** Files are what the request named and the one type winit
registers. A later `text` beside `paths` is an additive field on the
same event; declined now, not designed against.

**F. A drag *out* of the app, and between two kui windows.** Out: an
`NSDraggingSource` on the view and a file promise — a different feature
with a different row. Between windows: an in-app drag is `onDrag` and
ADR 0009's press-drag-release into a popup; it is not the OS's drag and
should not pretend to be.

## Consequences

- One row, one event, one background, three inputs, one reader, one
  ABI bump. The bindings' surfaces grow by the same five names each.
- `HitRegion` grows a `drop: Option<DropOwner>` beside `context_menu`
  (a `Key`, an origin, a `Value`); resolved only when
  `Tree::any_drop` — a frame with no zone pays a flag read.
- The runner's third override of a winit class. The dependence is on
  Apple's `NSDraggingDestination` selectors and winit registering the
  window delegate as the destination — checked at install; a winit that
  moved the destination to the view is skipped, and the runner falls
  back to winit's positionless events on macOS too.
- An app on Windows or Linux gets `enter` and `drop` at the right zone
  when the pointer is in the window on enter, and never `move`. Written
  in `props.md` beside the row.
- No access row: assistive technology has no drag protocol to map, and
  what a screen-reader user does instead — a button that opens the file
  dialog — is the app's to declare beside the zone. Written beside the
  row too.

## Open questions

1. **Does `draggingUpdated:` fire when winit's delegate answered
   `draggingEntered:` itself?** Only if the override is installed before
   the first drag reaches the window — installation is at window
   creation, so yes; to verify by dragging from the Finder onto the
   example and reading the stash.
2. **`draggingLocation` on a window with a titlebar strip** — window
   coordinates count from the content rect's bottom-left, so the
   conversion goes through the content view; the 32 px macOS 27 bar
   (W17) is outside the content view and needs no correction. To verify
   with the same drag, over the example's zone at its top edge.
3. **The Windows batch boundary.** `HoveredFile` events for one drag
   arrive in one `DragEnter` call, so "end of batch" is winit's
   `about_to_wait`; if a platform interleaves them with other events the
   collected paths are still one drag's. Unverified from here.

## Action items

- [x] Core: `on_drop`/`drop_bg` on `NodeSpec` (events box, interact
  box), `Tree::any_drop`, `DropOwner` on `HitRegion` resolved at
  emission, `Interaction::{drag_files, drop_files, drag_cancel,
  is_drop_target, drop_target}` with the prepared leave, the `dropBg`
  pick, the three `InputEvent`s dispatched.
- [x] Schema rows, `props.md`, the `drop` event doc, `DOORS` rows.
- [x] Corpus steps and the `drop` scene, four adapters.
- [x] Node, Lua, C doors; ABI 18; `index.d.ts`, `jsx-runtime.d.ts`
  regenerated.
- [x] Runner: `macos_drop.rs`; winit's events on the other platforms;
  the stamp.
- [x] Devtools tag; `examples/rust/features/drop.rs` and the Node twin
  with headless drives; CHANGELOG; C40 filed and closed; the Windows
  follow-up filed (W19).
- [x] Verified by hand: a Finder drag onto the example — the cursor over
  and off the zone, the overlay case, a refused release, the files
  leaving the window.

## What the building changed

Built on 2026-09-15, the day the document was accepted. What the code
settled that the text above had guessed:

- **The C struct is `KuiSpec`, not `KuiOpenArgs`** (decision 6 corrected
  above): `on_drop` and `drop_bg` are its last two fields, 560 → 576
  bytes, and `an_in_struct_s_size_is_the_abi_s` pins the new size at
  ABI 18. `kui_is_drop_target` came with `kui_drop_target` — the verb
  table's test refused a prototype with no row, and a reader that
  `is_hovered` has in three bindings has it in three bindings; the
  `Ui::is_drop_target` row is `Is` in every column, not the `As` the
  draft wrote.
- **`draggingDestinationWindow`, not the delegate's `window`.** The
  first real drag panicked in the override: a window *delegate* has no
  `window` selector. The destination window is the session's, from the
  `NSDraggingInfo` the protocol hands every method, and its content
  view (winit's, flipped) is what converts the point — the example read
  `220, 148` for a pointer 180 px below the window's top edge, the
  32 px macOS 27 bar (W17) already outside the content view, as open
  question 2 hoped.
- **The `leave` carries the paths of the `enter`,** not of the last
  report — it is built once, at enter, so the zone the view stops
  declaring mid-drag still gets it. The paths cannot change within one
  OS drag, so the corpus's `drop files leave 1` after a two-file move
  is the prepared event and not a defect; written into the scene's
  doc.
- **The corpus spells a drop as `drop <tag> <phase> <n>`,** the phase
  and the path count riding the tag column the way a drag's deltas do,
  so a binding that dropped the list on the wire would disagree there.
  The C adapter's step parser grew a third integer (`dragfiles n x y`),
  and its first run told `dragfiles` from `dropfiles` by their second
  letter — which is `r` in both.
- **`move` is suppressed for a repeated point in the core,** not the
  driver: AppKit's periodic `draggingUpdated:` with a still pointer
  would otherwise be a `move` every few frames. `DropHover::last` is
  the compare.
- **Off a zone, AppKit shows a plain arrow, not the not-allowed
  circle:** with `NSDragOperationNone` the green copy badge goes and
  the release is refused (the icon slid home in the test), which is
  what the stamp is for; the circle is the Finder's for a *move* it
  refuses. Decision 5's "not-allowed circle" reads as "no badge".
- **The C hosts had to be told about the drag in `surface.c`:** the
  header walk pins every prototype called, so the five new ones are
  called there with something checked — the card is a zone now, and
  `kui_drop_target` is checked against its key across an enter, a
  move, a leave, a drop and a cancel.
- **Windows and Linux got less than decision 5 promised.** The
  fallback dispatches at the pane's last reported cursor everywhere
  outside macOS; the `GetCursorPos` half was not written, because
  nothing here could run it (W16's rule), and it is one line of the
  W19 entry that replaces the whole fallback with an own
  `IDropTarget`.
