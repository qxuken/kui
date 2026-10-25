---
status: accepted
date: 2026-09-13
---

# A selection follows the pointer past the edge, and Shift extends it

> **Accepted (2026-09-13), built the same day** — what the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed); backlog C39. Found by running `examples/rust/features/clipboard.rs`
> after C33–C38 landed: drag the log's rows and keep going below the
> card, and the selection stops at the last row the frame drew while the
> list stays put; roll the wheel with the button held and the rows move
> under a selection that does not; Shift-click anywhere and it is a plain
> click. Three of the four things every text UI does with a held button
> are missing, in all three places a selection can live — the stock
> editor, a `selectable` scope, a `cells` grid. ADR 0017 wrote
> "autoscroll is a call, not a mechanism" and left the call unmade. This
> document makes one mechanism of all three gaps: **while a press is
> held, the live end is re-resolved at the pointer whenever the layout
> under it moves, and the core is what moves it when the pointer is past
> the scroller's edge**; Shift-press is a press that keeps the anchor.
> A `cells` grid is the case that cannot be a nudge — its history is the
> app's, and today it hears no wheel either — so **`onScroll` is a row
> on any node: the wheel as a message, in pixels, with the whole lines
> beside on a grid, and a grid's edge drag goes through the same door**,
> the app's `originLine` being what the re-hit watches.

## Context

### What a held button does today, read from the code

There are three drag-selects and they are dispatched from the same three
arms of `Core::input` (`crates/kui-core/src/runtime/dispatch.rs`):

| | press (`MouseDown`, `:561`) | motion (`CursorMoved`, `:604`) | release (`MouseUp`, `:629`) |
|---|---|---|---|
| stock editor | `edit.click(key, local, clicks)` then `edit.dragging = Some((key, origin))` | `edit.drag(key, local)` with the origin the press recorded | `dragging = None` |
| `selectable` scope | `arm_select_drag(scope, p, clicks)` | `extend_select_drag(drag, p)` | `select_dragging = None` |
| `cells` grid | the same arm, told apart inside `arm_select_drag` | the same | the same |

Four facts about those arms are the gaps:

1. **The live end moves only when the pointer does.** `extend_select_drag`
   and `edit.drag` are called from the `CursorMoved` arm and nowhere else.
   A wheel event under a held press (`InputEvent::Scroll`, `:280`) moves
   the scroller's offset and the next frame moves the text; the selection's
   `focus` is an address (ADR 0017, decision 2), so it stays glued to the
   byte it named and *not* to the pointer, until the pointer moves a pixel.
   A virtual list re-slicing its rows under the scroll is the same thing
   one step further: the row under the pointer is a different row and the
   end still names the old one.

2. **A point past the edge resolves to the last thing drawn.**
   `TextSystem::scope_hit` (`crates/kui-core/src/text.rs:2112`) filters to
   `r.place.drawn` and takes the nearest run vertically, so a pointer 200
   px below a four-row log lands in the fourth row. Correct as a hit — an
   undrawn run is under no pointer — and it is also why nothing scrolls:
   nothing asks the scroller to move, and the hit has nothing to move
   toward. The editor is the same through cosmic-text's `Action::Drag`,
   which clamps to the buffer's laid-out lines; its enclosing scroller is
   never nudged because `EditStore::drag` (`edit.rs:1053`) never calls
   `touch_caret`, unlike every keyboard motion (`edit.rs:1027`). So a
   drag-select in a field does not even reveal the caret it is moving.

3. **Shift is in the core and unread by the press.** Every driver sends
   `InputEvent::Modifiers` (`crates/kui-native/src/lib.rs:1413`, `kui-node`'s
   `modifiers()`, `kui-ffi/src/input.rs:78`, Lua through the same door),
   `Interaction::modifiers()` (`input.rs:1103`) holds the state, and
   `arm_select_drag` (`select_api.rs:706`) already reads `.alt` from it to
   make a grid's selection rectangular. `.shift` is read by exactly one
   pointer path in the whole core — none; the keyboard reads it for the
   editor (`edit.rs:961`: Shift+arrow seeds `Selection::Normal(cursor)`
   and moves) and for Tab. So the wire is done and the decision is
   entirely which press keeps which anchor.

4. **ADR 0017 named the mechanism and did not build it.** Decision 1,
   `0017-selection-as-a-scope.md:221`: "Dragging past the scope's edge
   scrolls it: `Core::reveal` and `Core::set_scroll` already exist, so
   autoscroll is a call, not a mechanism." Neither is called from a drag.
   `reveal(key)` is the wrong shape anyway — it reveals a *node*, and the
   thing to reveal is a point that has no node yet — and `set_scroll` is
   an absolute offset with no rate. What exists that fits is
   `scroll_rect_into_view(i, rect, relayout)`
   (`runtime/scrolling.rs:156`): climb from node `i` to the nearest
   scrolling ancestor, nudge its offset by how far `rect` is outside, and
   re-run the positions pass so *this* frame draws it. The caret and a
   pending `reveal` both use it after layout (`:111`, `:140`).

5. **A `cells` grid hears no wheel, and has no scroller to nudge.**
   `InputEvent::Scroll` goes to `Interaction::scroll_target`
   (`input.rs:1120`), which knows only scroll containers; a grid is not
   one, so a wheel over a terminal pane scrolls whatever column it sits
   in, or nothing. There is no `scroll` event in `docs/props.md`'s table
   and no `onScroll` row on any element. The `cells` example scrolls its
   session with two buttons (`examples/rust/widgets/cells.rs:230`), and
   the `splitmux` pane has no scrollback at all. So for a grid the edge
   drag and the wheel are the *same* gap: the core cannot move a grid's
   history — a grid is one screenful and `originLine` says which
   (`schema.rs:1496`) — and nothing tells the app to. ADR 0017 decision
   4 already made a grid's selection survive a scroll (absolute lines);
   what is missing is any way for the pointer to cause one.

### What the clock allows

The core is clock-free by design; a driver sets `Core::set_time` before
each frame (`runtime.rs:1020`) and one that never does gets snapping.
`AnimStore` keeps only `now` (`anim.rs:310`) — there is no frame delta
today — and `Core::animating()` (`runtime.rs:1027`) is how the core asks
the driver for a frame without input, which is what a held pointer past
the edge is: nothing arrives, and the content has to keep moving. The
conformance corpus has a `Step::Time(ms)` (`conformance.rs:342`), so a
rate is pinnable exactly, and the pomodoro's field notes on
`animating()` (F42) say the Node runner already pumps on it.

### What the corpus can and cannot spell

`Step` (`conformance.rs:284`) has `Cursor`, `MouseDown`, `MouseUp`,
`Scroll`, `Time`, and `ShiftTab` as its own kind — one word per step,
integers only, because four adapters print and parse the list. It has
no modifier step: a Shift-press has no spelling, and the `selection`
scene's four steps (`:1923`) are a plain drag. Every decision below that
touches Shift costs one new step kind in four adapters, the way
`ShiftTab` did.

### How the three neighbours do it

- **A browser** (Blink's `AutoscrollController`): while a selection drag
  is held past the viewport or a scrolling box, scroll by an amount
  proportional to the distance past the edge, every animation frame, and
  re-hit the selection at the pointer after each scroll. Shift-click
  extends from the existing anchor at the character grain; a
  Shift-double-click is a Shift-click.
- **NSTextView**: `autoscroll(with:)` is called from the drag loop with a
  timer while the pointer is outside; the selection follows the pointer.
  Shift-click extends; after a double-click, Shift extends by words (the
  "selection granularity" is remembered).
- **A terminal** (Alacritty, kitty): dragging past the top scrolls the
  scrollback at a rate from the distance; the selection is in absolute
  lines so it survives the scroll (what ADR 0017 decision 4's
  `origin_line` already gives a grid).

All three agree on the shape — proportional rate, per frame, re-hit after
each move — and differ only on what Shift remembers.

## Decision

### 1. While a press is held, the live end is re-resolved at the pointer whenever the layout under it moved

A drag-select in flight — `edit.dragging`, `select_dragging`, in a grid or
a scope alike — records the scroll offset of its scroller *as the last
layout resolved it* (`ScrollStore::geometry`, not the value the store
holds — a wheel event writes the store at once, and the finished frame's
placement reflects it only after the next layout). At the start of every
frame (`begin_frame`, before the view builds, so the frame paints the
result), if the finished frame resolved a different offset, the live end
is resolved again at the last pointer position exactly as a
`CursorMoved` there would resolve it: `extend_select_drag(drag, cursor)`
for a scope or grid, `edit.drag` for an editor. Against the finished
frame, like every selection query.

This is the whole fix for the wheel: a `Scroll` under a held press
changes the offset, the next frame lays out and draws the moved text,
the frame after re-hits and paints the end under the pointer — one frame
of lag on the highlight, none on the text, invisible at 60 Hz.

For a grid the offset is `origin_line`: a cell drag records the
`origin_line` the finished frame's grid declared, and a frame whose grid
declares another one re-hits through `extend_cell_selection_grained` at
the pointer. Same gate, the grid's own number — which is what decision
4's event moves.
It is also the fix for a virtual list re-slicing its rows and for a
window resize under a held press. And it is what makes decision 2 a
one-liner: autoscroll is nothing but a reason for the offset to differ.

Gated on the offset so a held-but-still pointer costs nothing per frame
— `scope_hit` walks the scope's runs, and a drag that is not moving over
content that is not moving has nothing to re-resolve.

The release does the same once more, before dropping the drag: `MouseUp`
resolves the end at the pointer against the finished frame, so a release
right after a nudge lands where the pointer is and not a frame behind.

### 2. The core nudges the nearest scroller when the held pointer is past its edge, at a rate from the distance

> **Amended 2026-10-25** (backlog F196): the rate below is replaced by
> an edge zone inside the scroller, a ramp while held and a cap, with a
> pointer pinned against the window's edge counted as fully deep — see
> [*Amended 2026-10-25*](#amended-2026-10-25-an-edge-zone-a-ramp-and-the-windows-edge)
> at the end. The rest of the decision stands.

Every frame a drag-select is in flight, before layout: take the drag's
scroller — the nearest ancestor-or-self of the scope, grid or editor
node whose spec has `scroll_x` or `scroll_y`, found the way
`scroll_rect_into_view` climbs, except starting at the node itself,
because a `virtual_column` is both the scope and the scroller — and the
last pointer position. For each axis the scroller scrolls, the distance
the pointer is past that edge of the scroller's rect (zero inside) sets
a velocity:

```
past  = clamp(distance past the edge, 0, 100)      logical px
v     = past × 10                                  logical px / s
step  = v × dt                                     this frame
```

with `dt` the frame clock's delta since the last frame, and **1/60 s when
there is no clock** — a clockless core does not snap here, because
there is no end state to snap to; it takes a frame as a sixtieth. So a
pointer 60 px below the log scrolls it 600 px/s, 10 px a clockless
frame, and 100 px or further past is the cap at 1000 px/s. The numbers
are one `const` pair and not a corpus contract; the *rule* — proportional,
capped, per frame, zero inside — is.

The step goes through `ScrollStore::scroll_by`, so the layout clamps it
to the overflow like a wheel step, and a scroller at its end stops. It
does not climb to an outer scroller when the inner one is at its end;
the nearest one is the drag's, the way it is the caret's. A grid that
declares `onScroll` is its own scroller for this (decision 4); one that
does not gets the ancestor nudge like any node.

While the step was non-zero the core reports `animating()`, so the
driver comes back for the next frame with nothing in the queue. The
pointer's last position is kept for the drag across a `CursorLeft`
(`input.rs:1240` clears `cursor`, and a press dragged out of the window
gets one on some platforms): the select drag records the point, the way
`Interaction::drag.last` already does for an app's `onDrag`.

For the editor, two more things: `EditStore::drag` calls `touch_caret`,
so the drag's own caret is revealed by `scroll_caret_into_view` exactly
as a keyboard motion's is — which is what a single-line field needs,
since it scrolls its own text through `line_offset` and has no scroller
to nudge — and `edit.dragging`'s origin is re-read from the editor's
place each frame rather than kept from the press: after a nudge the
content origin moved by the step, and a caret placed against the stale
origin lands the step off.

### 3. A Shift-press keeps the anchor

On a primary press, if `interaction.modifiers().shift` is set:

- **In a `selectable` scope that is the current selection's scope** (or
  the grid that holds the current cell selection): the anchor stays, the
  live end becomes the hit, and a drag is armed from that anchor — at
  the character grain, whatever the click count, with no `DragAnchor`.
  So a Shift-click extends and a Shift-drag keeps extending, and the
  press clears nothing (`clear_selection` at `dispatch.rs:538` is
  skipped for it). A block selection stays block.
- **In the focused editor**: if the editor has no selection,
  `Selection::Normal(cursor)` is set first — the same seed Shift+arrow
  uses — then `Action::Drag{x, y}` places the caret at the point with the
  selection kept, and `edit.dragging` is set so the drag continues from
  there. cosmic-text has no Shift-click; `Drag` is the action that moves
  the cursor without touching the selection, which is the whole gesture.
- **Anywhere else** — a scope that is not the selection's, an unfocused
  editor, a plain node — a Shift-press is a press. There is no anchor to
  keep, and a new selection is what the press means.

What Shift does *not* remember is the grain: a Shift-click after a
double-click-word extends by characters, not words. NSTextView remembers
the granularity and a browser does not; the browser's rule is the one
that needs no state, and the word case can be added by keeping
`SelectDrag` past the release if an app asks. Declined for now, named
here so it is not rediscovered.

### 4. `onScroll` on any node: the wheel as a message, and a grid's edge drag through the same door

`onScroll` (`on_scroll`) is a new schema row, on every node, and the
event is

```
{ kind: "scroll", x, y, dx, dy, lines, tag }
```

`dx`/`dy` the delta in logical px as the driver reported it (positive
`dy` is the wheel rolling up, toward earlier content — `InputEvent::
Scroll`'s own sign), `x`/`y` the pointer, and `lines` the whole lines a
`cells` grid's `dy` covers — positive is later history (row 0 becomes a
higher absolute line, the content moves up), the sign `originLine` grows
in, the fraction carried per grid to the next notch so a trackpad's
3 px steps accumulate into a line rather than vanish — and `null` on
any other node. The app answers by re-declaring: a grid with a new
`originLine`, a canvas with a new zoom; the core moves nothing.

The routing question the draft named is answered by paint order, the
rule every scroll region already follows: a node declaring the row
pushes a `ScrollRegion` with `handler` set, the topmost region under the
pointer takes the notch, so a handler **takes the wheel from any
scroller above it and loses it to a scroller inside it**. A node that is
both — a scroller that also declared the row — is the handler's: the
app asked to hear the wheel, and hearing it *and* having the content
move would be two answers to one notch. A handler draws no bars.

Two things feed a grid's, and the app cannot tell which:

- **The wheel**, as above.
- **The edge drag.** A cell drag held past the grid's own top or bottom
  edge runs decision 2's rate against the grid's rect — same `past`,
  same `v`, same `dt` — divided by the row height, whole lines out once
  a frame, the fraction carried in the drag. `animating()` while it
  steps, as for a scroller. Past the left or right edge nothing happens:
  a grid has no columns to scroll to. A handler that is not a grid hears
  its edge drag the same way, both axes, in pixels.

An app that ignores the event loses nothing it has today; one that
moves `originLine` gets scrollback under the wheel and under a held
selection, and ADR 0017's absolute lines make the selection survive it
without the app doing anything else. The `cells` example drops its two
buttons for the row; `splitmux` is unchanged (its panes are numbers).

The draft kept the row to `cells`, in lines, and declined the general
one; the ask on 2026-09-13 was for `onScroll` outright, and the general
row costs nothing the grid's did not — the unit question is answered by
carrying both — so it is built whole.

### 5. The corpus gains `Shift` as a step, and three scenes

`Step::Modifiers(u32)` — the modifier state as `KeyMods::bits` (Shift 1,
Ctrl 2, Alt 4, Super 8), `InputEvent::Modifiers` — one step kind with an
integer the way `Arrow` is, so a chord is one line and every argument
stays an integer, in four adapters. (The draft had `ShiftDown` /
`ShiftUp`, two kinds per key; the review of the build replaced them
before anything outside the repo learned the names.) Two scenes:

- `selection-extend`: the `selection` card, a click in the first run, a
  Shift-click in the third, then a Shift-press-drag back into the second.
  Pins that the anchor survived both, as the highlight quad count and
  digest do for `selection`.
- `selection-scroll`: a scrolling card two rows tall over four runs; press
  in the first, move 60 px below the card, `Time` steps of 16 ms until
  the offset reaches its clamp, release. Pins the rate (the offset after
  N frames), the clamp, the re-hit (the end is in the last run), and that
  a `Scroll` step under the held press moves the end too.

- `cells-scroll`: the `cells` scene's grid with `onScroll`, a wheel
  step over it, then a press in row 1 and the pointer 60 px below the
  grid through `Time` steps. Pins the wheel's `lines` (whole, the
  fraction carried), the edge drag's per-frame lines, and — the view
  answering with a moved `originLine` per phase — that the selection's
  ends kept their absolute lines through it.

All three are behind the existing `conformance` feature and cost the
four adapters one parse arm.

### 6. Doors: one row, one reader

Shift is already on every wire. Autoscroll on a scroller is an internal
of a drag the app never sees. The new things on the wire are the
`onScroll` row and its `scroll` event — a row in `schema.rs`, the four
spellings, a `props.md` line, the payload typed in `index.d.ts` — and
one reader, `selection_ends`: the anchor and the focus as a virtual
row's index and a byte, the directed pair `selection_range` is not,
which is what a test or a mirroring model reads to say a Shift-press
kept the anchor (Rust `Core`/`Ui`, Node `selectionEnds()`, Lua
`env.selection_ends()`, C `kui_selection_ends`). The other observable changes — `selection()`,
`cell_selection()` and the editor's selection moving without a
`CursorMoved`, `animating()` true while a held pointer is past an edge —
are readable through every existing query. `Ui::modifiers` stays what
it is. No ABI bump, no protocol bump.

## Considered options

**A. Re-hit only on autoscroll, not on every offset change.** Cheaper to
describe, and it leaves the wheel gap open: the wheel is the case the
example shows first. Rejected; decision 1's gate makes the general rule
cost the same as the narrow one.

**B. Rate by a fixed step per frame, no clock.** `step = past / 6` per
frame gives the same numbers at 60 Hz and drifts at 120 Hz (twice as
fast) and under a slow frame (half). The clock is there; use it, with the
clockless case defined so a headless drive is exact.

**C. Autoscroll as an event the app answers everywhere**
(`{kind:"autoscroll", dy}` on the scope; the app calls `set_scroll`). It
puts the common case, a scrolling card, on every app. Rejected as the
default and *taken for the grid*, where it is the only shape that can
work — and made the wheel's door too, so a terminal does not get two
events for one scroll. The name is `scroll`, not `autoscroll`, because
the app cannot tell and should not care which it was.

**F. The grid scrolls its own history in the core.** A `cells` node
that kept the app's scrollback and sliced it would have to be handed
every line ever printed; the whole point of C20's grid is that the app
hands over one screenful a frame and the core keeps nothing. Rejected;
`originLine` stays the app's word.

**D. Shift remembers the grain (NSTextView).** Needs `SelectDrag` kept
past the release and cleared on the next non-Shift press; a third state
the drag has to carry. Declined in decision 3 with the door named.

**E. Do nothing; the app scrolls itself from `onDrag`.** The app's
`onDrag` never fires for a drag-select — the scope claims the press and
`onDrag` is the *alternative* to it (ADR 0017, decision 1). There is no
app-side hook and there should not be one.

## Consequences

- Every drag-select on every platform scrolls at the edge and follows
  the wheel, with the same rate rule, and every Shift-click extends.
  Three gaps, one mechanism, in the core once (`feedback-bindings-first`).
- `animating()` is true for as long as a pointer is held past an edge; a
  driver that pumps on it pays a frame per 16 ms for that long, which is
  what a browser pays. A driver that ignores it scrolls only when the
  pointer moves, which is still more than today.
- `begin_frame` gains one branch, taken only while a drag-select is in
  flight; a frame without one pays a `bool`.
- The `selection` and `cells` scenes are unchanged. Three scenes, one
  step kind and one row land in four adapters.
- A terminal built on `cells` gets scrollback under the wheel for the
  first time, as a side effect of the drag needing the same door. That
  is the larger of the two gains for the mux use case, and it was not
  what the example was opened to find.
- `EditStore::drag` revealing the caret changes a field's behaviour under
  a plain drag today: a single-line field now scrolls its text toward the
  pointer as it is dragged past its end, which is the missing half of
  F41's fix and what every native field does.
- The docs: `docs/props.md`'s `selectable` row and the clipboard howto
  gain the two sentences; the example's key list gains `⇧-click`.

## Open questions

1. **Should `onDrag` get the same nudge?** An app's `onDrag` over a
   scrolling list — a reorderable list dragging a row toward the edge —
   wants the same scroller nudge, and decision 2 is a function of
   `(node, pointer)` that could take the drag's node. Not asked for; the
   nudge is written so it can be, and it is not wired.
2. **A handler at its end.** A scroller at its end stops stepping —
   the layout clamps the offset — but an `onScroll` node cannot say it
   has nothing more to scroll to, so a drag held past a terminal's
   bottom edge keeps asking once a frame until the release (the screen
   check counted ~130 events/s at the end of the session). Cheap, and
   correct as far as the core can know; a reply on the event ("at the
   end") would let it stop, and nothing has asked.
3. **A held press past the window.** The core keeps the last point; the
   driver's mouse capture is what keeps sending it. winit captures on
   both platforms and the Node and C runners are the app's — whether
   theirs do is a runner-by-runner check, and a headless drive proves
   nothing about it.

## Action items

- [x] Decision 1: the drag records its scroller and that scroller's
      offset; `begin_frame` re-hits on a differing offset; `MouseUp`
      re-hits once before dropping the drag.
- [x] Decision 2: the nudge in `begin_frame`, the `const` pair, the frame
      delta kept beside `AnimStore::now`, `animating()` while stepping,
      the drag's last point kept across `CursorLeft`; `EditStore::drag`
      touches the caret; `edit.dragging` re-reads its origin.
- [x] Decision 3: the Shift arm in `MouseDown` for the three places.
- [x] Decision 4: the `onScroll` row in `schema.rs` and four bindings, a
      handler as a scroll region, `Scroll` ÷ the row height with the
      fraction carried per grid, the edge drag's lines per frame with the
      fraction carried in the drag, the re-hit gate on `origin_line`; the
      `cells` example on the wheel and the drag, buttons gone.
- [x] Decision 5: `Step::Modifiers(u32)` in four adapters; the
      `selection-extend`, `selection-scroll` and `cells-scroll` scenes.
- [x] The `clipboard` example's headless drive pins all three on the log
      (Shift-click, drag past the bottom, wheel under a held press), in
      Rust and Node, and the key list names `⇧-click`.
- [x] `docs/props.md`, the howto (two articles), CHANGELOG under alpha.12,
      backlog C39.

## What the building changed

Built 2026-09-13, the day it was written, as decided — with decision 4
widened from a row on `cells` to a row on every node at the user's ask,
and these things the draft could not know:

- **The gate is the *laid* offset, and it is new.** `ScrollStore::
  geometry().offset` is the retained offset clamped, which a wheel notch
  moves at once — a frame before the text does — so a re-hit gated on it
  would have placed the end against the old placement and landed on the
  same byte. `Entry::laid` is what `resolve` clamped at the last layout,
  read through `laid_offset`; the corpus's `selection-scroll` scene pins
  the one frame of lag the right gate has (the text moves on the frame
  after the notch, the highlight on the one after that).
- **The hook runs at the top of `begin_frame`, before the tree clears.**
  Everything it reads — the scroller's rect, a grid's `origin_line`, the
  editor's drawn origin — is the finished frame's, and `begin_frame`'s
  first act after the hook is to clear it. It also runs before
  `frame_requested = false` is reset, which is why the step sets its own
  `DragFollow::stepped` flag that `animating()` reads instead of
  `request_frame`.
- **Time set but not advanced is no time.** With a clock, a frame whose
  `set_time` equals the last one steps nothing; a headless drive that
  stamps every frame (`Drive`, Node's loop) autoscrolls only through
  `advance`, which is what both example drives do. A core with no clock
  at all takes a frame as a sixtieth. A frame over 100 ms is capped: a
  window that sat hidden must not scroll a second's worth on return.
- **cosmic-text's `Drag` is the whole Shift-click.** It seeds
  `Selection::Normal(cursor)` when there is none and moves the cursor
  keeping it, so the editor arm is `click(.., extend)` choosing `Drag`
  over `Click` and nothing else — still a click, so a live composition
  is abandoned and the typing unit ends (the review caught the first cut
  going to `edit.drag` directly and skipping both). `EditStore::drag`
  now `touch_caret`s when the cursor moved, which is what makes a
  single-line field scroll its text toward a drag past its end (the
  caret reveal F41 built runs for it) — and the review found the same
  mark putting a *document's* reveal under the held pointer: a caret 60
  px past the edge revealed by 60 px a frame, on top of the step, six
  times decision 2's rate and frame-bound. The reveal waits for the
  release while the editor is the one being dragged; the field's own
  text still settles at once.
- **One carry per grid.** The wheel's leftover fraction and the edge
  step's were two accumulators on the same grid; they are one
  (`Core::line_carry`, owned by `scroll_event`), so a notch's half line
  and a step's add up. And a step the clamp would undo is no step: a
  container at its end under a held pointer does not report
  `animating()` — the first cut did, a window at display rate until the
  release — and `DragFollow::stepped` is that flag, gone with the
  follow instead of reset by hand.
- **`set_scroll` from a view asks for no frame.** The two frame spins
  found on screen were each a view writing an offset every frame; the
  guards at both sites stay, and the mechanism no longer lets a third
  do it.
- **The event row prints the lines.** `event scroll term 2` — the
  corpus's report carries a scroll's `lines` the way it carries a drag's
  deltas, in all four writers, because the carry is the contract and a
  binding that lost it between two notches would agree on the kind.
  `Step::Modifiers(bits)` is the `Modifiers` input, and the modifier
  changing is itself an event on the root, so `selection-extend` pins two
  `modifiers -` rows too.
- **A grid without `onScroll` is unchanged**, and one with it is its own
  scroller for the drag: `scroller_of` walks self-first and prefers a
  handler over a container on the same node, the rule `ScrollRegion::
  handler` states for the wheel.
- **`selection_ends`** was not in the draft. The Node twin of the
  clipboard example needed to say a Shift-press kept the anchor on a
  virtual list whose anchor row was no longer built, which
  `selectionText` cannot (an unbuilt end reads as nothing) and
  `selection_range` cannot honestly (it is undirected, and — found on
  the way — unordered: filed as its own task). So the directed pair, as
  `(RangeEnd, RangeEnd)` in Rust and a `{anchor, focus}` of `{index,
  byte}` in the three bindings.
- **On screen** (macOS, CGEvent driver): the clipboard example's log
  scrolls from row 0 to row 35 in ~1.1 s with the pointer 60 px past its
  edge (134 frames — the runner pumps on `animating()`), the highlight
  follows, a release stops it, a Shift-click 1 s later ends the
  selection mid-row 37 and Cmd-C puts rows 0–37 on the OS clipboard; the
  cells example's edge drag reaches line 1215 with the anchor kept on
  1205 and a `scroll` event a frame, and the wheel moves the screen
  under a selection that stays. A held press *inside* the list draws
  one frame and idles. Found on the same screen and filed as its own
  task, not this branch's: a Cmd-C over the virtual log leaves the
  runner at ~130 frames/s on main too.
- **Not done:** a handler cannot say it is at its end (open question 2,
  new); a Shift-press does not remember a double-click's word grain
  (decision 3, declined as drafted); the `selection_range` ordering
  defect is a separate task.

## Amended 2026-10-25: an edge zone, a ramp and the window's edge

Raised by Carnet, which copied decision 2's rate for its own page drag
(backlog F196). The rate counts only the distance *past* the edge. In a
full-screen or maximised window the pointer stops at the monitor's
edge, a pixel or two from the scroller's, so an app's band of 24 px
inside the edge was all the depth there was: 240 px/s at most, and
selecting through a long document was painfully slow. kui's own drags
had it worse: no band, so a scroller filling the screen did not move at
all under a pointer pinned to its bottom.

- **`EdgeScrollFeel { zone_px, speed_px_s, curve, ramp_secs, ramp_max,
  cap_px_s, pull_back_px }`** (`crates/kui-core/src/edge_scroll.rs`),
  set by `Core::set_edge_scroll_feel` and read back by
  `Core::edge_scroll_feel`, shaped like ADR 0050's `WheelFeel`: the
  app's input feel, clamped, copied to every window the runner opens.
  Per axis the scroller scrolls, every frame a drag is held:

  ```
  zone   = min(zone_px, a quarter of the scroller's extent)
  depth  = how far the pointer is into the zone (0 at its inner edge,
           zone at the scroller's edge, more past it); pinned within
           2 px of the window's edge, at least zone
  base   = speed_px_s × (depth / zone)^curve              inside
         = speed_px_s × (1 + curve × (depth − zone) / zone) past
  ramp   = 1 + (ramp_max − 1) × min(held / ramp_secs, 1)
  step   = min(base × ramp, cap_px_s) × dt
  ```

  Past the edge the speed goes on along the curve's tangent, so in a
  smaller window the distance past still adds speed, within the cap.
  `held` is the time since the pointer entered the zone; pulling back
  toward the content by more than `pull_back_px` from the deepest point
  since then starts it again (the curve has already slowed the step at
  once), and leaving the zones resets it. `dt` is decision 2's: the
  frame clock's delta capped at 0.1 s, 1/60 with no clock, 0 for a
  clock set and not advanced. Not eased: the store's `scroll_by`, as
  before.
- **A press inside its click slop never scrolls.** With a zone inside
  the scroller a press near its edge is in the zone before it moves;
  the step waits for `press_slop` to clear, the same gate F170/F173 put
  on the live end.
- **Three presets**, slow to fast; `standard` is the default.

  | preset     | zone  | at the edge | curve | ramp        | cap       | pull back |
  |------------|-------|-------------|-------|-------------|-----------|-----------|
  | `gentle`   | 72 px | 360 px/s    | 2     | 5× in 2 s   | 1800 px/s | 8 px      |
  | `standard` | 60 px | 500 px/s    | 2     | 6× in 1.5 s | 3000 px/s | 8 px      |
  | `brisk`    | 48 px | 700 px/s    | 2     | 7× in 1 s   | 4900 px/s | 8 px      |

  Why `standard`'s numbers: the zone is about two lines of body text,
  as Carnet asked. 500 px/s at the edge is twice the 240 px/s a
  full-screen window topped out at, and half the old cap, about 25
  lines a second: fast, and still a speed a hand can stop on. The
  quadratic curve puts the zone's middle at a quarter of that, 125 px/s,
  and its inner third under 60 px/s, for walking a selection a line at
  a time. Six times over a second and a half reaches 3000 px/s, three
  screens a second of a 1000 px window, which is what holding still
  should come to; the cap is exactly the ramp's top at full depth, so
  only a pointer past the edge in a smaller window reaches it sooner.
  The pull-back is 8 px, past a hand's tremor and a trackpad's jitter
  (the click slop is 3), and short of any deliberate easing off.
- **An app's own drag** — Carnet's page selection, a reorder, a file
  dragged over a list — calls `Core::edge_scroll(key, pointer, rect) ->
  Option<Vec2>` (and `Ui::edge_scroll`) once a frame while it holds the
  drag: `rect` is the area whose edges scroll (usually the container's
  `scroll_geometry` rect), the container moves at once by the same
  step, the delta that moved comes back, and the next frame is asked
  for while it moves. The ramp is kept per container while the calls
  come every frame and starts again after a frame without one. Whether
  a press is a drag yet is the app's to know, so the click slop is
  its own. Doors: C `kui_edge_scroll`, Odin `edge_scroll`, Node
  `edgeScroll(key, x, y, rect)`, Lua `env.edge_scroll(key, x, y, rect)`.
- **Feel doors.** C `kui_set_edge_scroll_feel` (seven numbers),
  `kui_edge_scroll_feel` (seven out pointers), `kui_set_edge_scroll_preset`
  (new functions, no struct moves, no ABI bump); Odin
  `set_edge_scroll_feel`, `edge_scroll_feel`, `set_edge_scroll_preset`;
  Node `setEdgeScrollFeel(name | { zonePx, speedPxS, curve, rampSecs,
  rampMax, capPxS, pullBackPx })` and `edgeScrollFeel()`, and
  `edgeScrollFeel` to `runWindowed` (a window option) and `createApp`;
  Lua `No(GUEST)`. The launcher's `edge_scroll_feel(feel)`, and
  **`KUI_EDGE_SCROLL`** in the environment — a preset's name or up to
  seven numbers in the struct's order, the rest standard — over every
  way an app sets its feel before the window opens, as `KUI_WHEEL` is.
- **`AUTOSCROLL_RATE` and `AUTOSCROLL_REACH` are gone** from
  `runtime::follow`, and `edge_step` with them; `CLOCKLESS_FRAME`
  stays.

Tests: `edge_scroll.rs`'s unit tests (the curve, the tangent past the
edge, pinned is full depth, the ramp and the cap, pull back and reset, a
short scroller's still middle, clamping); `tests/edge_scroll.rs`, a
headless caret drag on a window-filling scroller (the ramp from 500 to
3000 px/s over 1.5 s and no further, pinned at the window's edge is full
depth, a pull back past 8 px drops to the curve and restarts the ramp,
leaving stops it, an unmoved press never scrolls, the content's ends
stop it, and an app's drag through `Ui::edge_scroll`); `tests/follow.rs`
on a flat feel that is the old rate; kui-ffi's `edge_scroll_headless`,
the C surface round, Node's `edgeScroll` test, Lua's
`a_script_scrolls_a_container_at_its_edge`, kui-native's
`kui_edge_scroll_reads_a_preset_or_numbers`, and the `edge_scroll`
example's self-check. Not yet felt by hand in a full-screen window.

## Amended 2026-10-25: a held drag hands on

Raised by the user after F196: a drag held at an edge scrolled the one
scroller the press found and no other (backlog F204). Selecting from a
code block that scrolls sideways down a page could not scroll the page;
a short log scrolled to its end and stopped with the page still; an
app's board-in-a-page drag needed an `edge_scroll` call per container.
The user asked whether handing on should be opt-in.

- **It is on, and `overscroll: contain` keeps it in.** Chromium's
  selection autoscroll scrolls through `ScrollRectToVisible`, which
  moves every scrolling box around the target; AppKit's `autoscroll:`
  goes to the superview when a clip view cannot move; and kui's own
  wheel chains, stopped by `contain`. F196 had not shipped, so nothing
  released moves under it, and an opt-in would leave the common case —
  a page with code blocks, Carnet's page selection — broken until each
  app found the switch. One rule for the wheel and the drag, and no new
  prop or door.
- **Per axis, from the scroller out.** Each frame, on each axis — over
  the finished frame's scroll regions, the wheel's, which a view being
  built leaves whole — the first container walking out from the
  innermost scroller under the pointer inside the press's (a code block
  a selection begun in the text around it has come into — one the hand
  moved the pointer into, not one the content carried under a pointer
  held still), or else from
  the press's scroller, that scrolls
  along it, has the pointer in its edge zone — measured against its own
  rect — and room the way that zone scrolls, is stepped on that axis.
  One that does not scroll that way, is at its end, or has the pointer
  clear of its zones hands the axis to the container around it; one
  that says `contain` keeps it whether or not it can move. The walk ends
  at an `on_scroll` handler, whose room the core cannot read, and at a
  modal's edge. A pointer held at a page's middle, below a short log at
  its end, scrolls nothing: the page's zones are at its edges.
- **A ramp per container**, kept while it is stepped every frame and
  started again after a frame it was not (`Core::step_edge_chain`, with
  the core's `edge_ramps` that `Ui::edge_scroll` already kept). The live
  end is placed again after any frame that stepped, since a container
  further out moved the text under the pointer without the scroller's
  own reading moving.
- **`Ui::edge_scroll(key, pointer, rect)` hands on the same way** from
  `key`, the containers around it measured by their laid rects, and
  returns what moved summed over them.

Tests: `tests/edge_scroll_chain.rs` (a log at its end hands a caret drag
to the page; `contain` keeps it in the log and idle at its end; a strip
that scrolls sideways hands down to the page; an app's `edge_scroll` on
the strip moves the page with one call), with `edge_scroll.rs` and
`follow.rs` unchanged. Not yet felt by hand.
