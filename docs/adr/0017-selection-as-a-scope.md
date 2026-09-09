---
status: proposed
date: 2026-09-09
---

# Selection is a scope, and the menu is what acts on it

> **Proposed (2026-09-09).** Out of a field question with a small surface
> and a large back: on macOS, force-clicking a word selects it and shows
> the system Look Up panel, and a kui app does none of that. Answering it
> honestly needs three things kui has never had — a selection that is not
> the editor's, a selection that reaches past the viewport, a selection
> inside a `cells` grid — and one it nearly has, a context menu whose
> items the core decides. The force click itself is the last and smallest
> decision here, six paragraphs at the end.
>
> The claim this document defends is that **all of it fits without a
> document model**. A selection scope is a node that declares `selectable`;
> within it, the frame's own emission order *is* the document order,
> because `places` is already recorded in tree order. What that cannot
> reach — a virtual list's unbuilt rows, a terminal's scrollback — is
> reached by asking the app, not by retaining a tree.

kui is immediate mode with one retained selection: the editor's. Everything
else about text is a function of the frame, and every app that has wanted
to select a label has been told to build it out of `text_hit` and its own
state. Two of the apps in the field round did exactly that, differently,
and neither can copy across two labels or offer Look Up.

This ADR takes the position that selection is not a widget feature but a
**window-level mode with a scope**, in the same family as keyboard focus
(ADR 0002) and modal containment (ADR 0003): one at a time, owned by the
core, addressed by keys, reported as data.

## Context

### The editor owns the only selection there is

`Editor::click` maps the driver-counted multi-click onto cosmic-text's
actions — 2 selects the word, 3 the line (`crates/kui-core/src/edit.rs:994`)
— and the selection paints as quads under the glyphs, run by run, through
cosmic-text's `run.highlight` (`crates/kui-core/src/edit.rs:1306`).
`Core::copy_selection` is documented as what it is: "Selected text of the
focused editor" (`crates/kui-core/src/runtime/dispatch.rs:890`).

For a `text` node there is nothing. The word `selectable` does not occur
anywhere in the crates today. A press on a label moves focus if the label
is focusable and otherwise does nothing at all.

### The text system was already built to answer by an enclosing key

This is the fact the whole design rests on, and it predates the design.

`TextPlace` records, for every text run the frame drew, the keys **above**
it — as many as `PLACE_ANCESTORS`, which is 4 — so a query by an ancestor
finds the runs inside. The constant's comment names the intended callers:
"a `line` row, **a selection wrapper around a run**, and two to spare"
(`crates/kui-core/src/text.rs:521`). Whoever wrote that was describing this
ADR.

On top of it:

- `runs_of(key)` returns every run answering to `key`, **in tree order**,
  each with the byte offset its content starts at in the concatenation
  (`crates/kui-core/src/text.rs:1835`).
- `hit_at(key, point)` picks the nearest run by vertical-then-horizontal
  distance and then the byte inside it, so a point outside every glyph box
  still resolves — which is what a drag past the end of a line needs
  (`crates/kui-core/src/text.rs:2031`).
- `caret_at(key, byte)` maps back the other way.

So "a point inside a subtree → a byte offset in that subtree's
concatenated text" exists and ships. What is missing is only who holds
*two* of those offsets, what paints between them, and what copies them.

### A place exists only for text that was drawn — and that is the whole viewport problem

Emission skips a node whose rect misses its clip:

```rust
// Entirely clipped away: skip drawing and hit-testing.
let visible = rect.intersect(&clip.rect);
if visible.w <= 0.0 || visible.h <= 0.0 { continue; }
```

(`crates/kui-core/src/runtime/emit.rs:502`) — before `emit_node`, so a
label scrolled out of a scroller has no place, no hit region and answers
no query. `TextPlace`'s own doc states it as a property: "a node the frame
culled — scrolled out of its clip — has no place and answers nothing,
which is also true of a point nobody can click".

**But the shaping already happened.** `TextSystem::add` interns and shapes
when the node is *opened*, during the app's view
(`crates/kui-core/src/text.rs:925`), because layout needs the measurement.
A culled text node therefore has a live cache entry with its content in it;
what it lacks is one `places.push`. The distance between "selection stops
at the edge of the viewport" and "selection reaches the whole scroller" is
that push.

### A selection cannot be evicted out from under itself

The obvious fear about a selection that reaches off-screen is that it
points into the shaped-text cache, which has a byte budget (64 MB by
default, `crates/kui-core/src/text.rs:158`) and evicts to three quarters of
it when over. It does not, and cannot, for two independent reasons.

The first is the eviction rule itself: `evict_to_budget` filters to entries
with `last_used < drawn_last_frame` before it sorts — "**Never an entry the
frame that just finished drew**: what is on screen stays shaped whatever
the budget says" (`crates/kui-core/src/text.rs:700`). An entry's `last_used`
is stamped at `intern`, which runs when the node is *built*, not when it is
drawn — so tier 2's off-screen text is protected by that guard exactly as
strongly as the text on screen is. A frame whose built text exceeds the
budget does not evict live entries; it goes over budget and evicts nothing.
Content that stops being built is content in tier 3, which was never in the
cache to lose.

The second is decision 2: an endpoint is an address — a key and an offset —
never a pointer, an index into a frame vector, or a cache handle. Anything
the core cannot resolve resolves to nothing and says so, rather than
resolving to whatever moved into that slot.

**What is genuinely proportional to the selection is the copy.** Assembling
a selection over a built 10,000-row log means one `String` the size of the
selection, and then the clipboard's own copy of it. That is unavoidable —
the clipboard takes a string — but it is worth stating so nobody looks for
a leak: the selection *state* is two addresses and O(1); the *highlight* is
O(visible runs); only *copy* is O(selected bytes), paid once, on Cmd-C.
Since each run's content length is known before assembly, the copy reserves
the exact total once rather than growing a buffer — no cap, no truncation,
no `selection-truncated` warning to explain to someone selecting a log
file on purpose.

### What no cheap trick reaches

Content the frame never built. `widgets::virtual_column` builds rows
900..930 of ten thousand — the other 9,970 were never opened, never shaped,
and their text was never handed to the core in any form. A terminal is the
same shape one level down: the app hands `cells` one screenful of
`Cell { ch, fg, bg, flags }` (`crates/kui-core/src/cells.rs:44`) and keeps
its scrollback to itself.

A browser answers this by retaining the document. That is precisely the
purchase kui declines everywhere else, and it would be strange to buy it
here, for selection, having declined it for layout (ADR 0016 declined even
caching the *frame*).

### Menus are entirely the app's, and every app writes the same one

`onContextMenu` emits `{kind:"contextmenu", x, y, tag}` at the point to
open a menu at, moves no focus, places no caret and produces no click
(`crates/kui-core/src/schema.rs:736`). Everything after that — items,
layout, theming, keyboard navigation, dismissal, and the fact that Cut,
Copy, Paste and Select All are the same four items in every app that has
text — is the app's to write. On macOS, Look Up and the Services menu are
not the app's to write at all: they are the system's, and a drawn
imitation would be both wrong and worse.

### The frame has a defined moment for content the app did not build

`Ui::finish()` runs a filler before layout — the `"root"` fill, "unless the
view declared it" (`crates/kui-core/src/ui.rs:613`), which is how ADR 0014's
extensions put their nodes into a host's frame. So there is already an
answer to "who builds a subtree the app did not write", and it is not a new
overlay layer.

### The gesture is already in the driver's hands, unread

winit delivers `WindowEvent::TouchpadPressure { pressure, stage }` straight
from AppKit's `pressureChangeWithEvent:`
(`winit-0.30.13/src/platform_impl/macos/view.rs:758`). It is the only
platform_impl in winit that mentions pressure: this is a macOS-only event,
and on macOS a trackpad-only one that the user can switch off entirely
(System Settings → Trackpad → Force Click and haptic feedback).

The rest is available too. `objc2-app-kit` 0.3 is already a dependency of
`kui` on macOS (for `system_env`), and carries both
`NSView::showDefinitionForAttributedString_atPoint` and
`setPressureConfiguration:`. winit's content view answers `isFlipped =
true`, so kui's logical coordinates *are* the view's points — no flip
arithmetic between a caret rect and the popover that points at it.

What is not free: the "select the word and show the panel" behaviour
belongs to `NSTextView`, which implements it by calling `showDefinition`. A
custom view gets the pressure event and nothing else.

## Decision

### 1. `selectable` declares a selection scope; the runs inside it are one selection

A node declaring `selectable` becomes a **selection scope**. Every text run
emitted beneath it belongs to that scope's single selection, concatenated
in emission order — which is tree order, which is reading order for every
layout that reads in order.

- Scopes do not nest: the innermost enclosing scope wins, and a warning
  (`nested-selection-scope`) fires on the outer one, in the family of the
  existing `diag` warnings.
- An `edit` node is its own scope and keeps its current behaviour exactly.
  A `cells` node is its own scope (decision 4).
- **One selection per window.** A press that starts a selection in one
  scope clears every other, the way focus moves. The window's selection is
  a `(scope, anchor, focus)` triple in the core, not a per-node flag.

Two mechanical changes carry it:

- `TextPlace` gains `scope: Option<Key>`, threaded down the emit walk from
  whichever ancestor declared `selectable`. This **replaces the four-deep
  ancestor guess** for this purpose: the walk in
  `crates/kui-core/src/runtime/emit.rs:133` climbs at most
  `PLACE_ANCESTORS = 4` levels, so a `selectable` card wrapping rows
  wrapping labels would already be out of reach. One `Option<Key>` per
  place is also less memory than the array it stands beside.
- `HitRegion` gains `select_scope: Option<Key>` beside the existing
  `edit_origin: Option<Vec2>` (`crates/kui-core/src/input.rs:571`), which
  is how the pointer model already owns a caret drag the app never
  declared. A selection drag is owned the same way: press sets the anchor,
  motion moves the focus, release ends it, and none of it is an `onDrag`
  the app has to declare or can intercept.

Dragging past the scope's edge scrolls it: `Core::reveal` and
`Core::set_scroll` already exist (`crates/kui-core/src/runtime/scrolling.rs:38`,
`:78`), so autoscroll is a call, not a mechanism.

### 2. An endpoint is an address, not a position

The selection's two endpoints are `(node, offset)` pairs against the run's
**own** key, not `(scope, concatenated byte)` pairs. The concatenation is
how a *hit* resolves in a frame; it is not how an endpoint is stored,
because the concatenation shifts whenever the built set changes — one
virtual row scrolling into existence would silently move both ends of a
selection stored against it.

Ordering between two endpoints is the frame's place order when both are
placed, and the data index when they are not (decision 3).

For that, the core keeps one more thing: **the data index of a node inside
a selection scope**, when it was opened with one. `open_indexed` hashes the
index into the key and does not retain it
(`crates/kui-core/src/runtime/builder.rs:353`), and a hash does not invert.
This is a side map filled only for indexed nodes inside a scope — a `u64`
per selectable row, not per node.

### 3. Reach is three tiers, and only the third asks the app

**Tier 1 — drawn.** As today.

**Tier 2 — built but clipped away.** Record a place for a culled text node
too, flagged `drawn: false`. It answers selection, ordering and copy; it
answers **no** hit test and no `text_hit`, which must keep meaning "where a
click can land" — a query about a point the user cannot reach should stay
`None`. The content is already in the cache (it was shaped at open), so
this tier costs one push per off-screen text node and nothing else.

**And only inside a scope.** The place for a culled node is recorded when
that node is inside a `selectable` (or `edit`, or `cells`) scope, which
emission knows at the moment it culls, because the scope came down the walk
with it (decision 1). An app that declares no scope pays nothing at all —
not a branch it can measure — and an app that wraps one card in
`selectable` pays for that card's off-screen text, not for its 10,000-row
log beside it. This is deliberate: tier 2's cost is proportional to what
the app asked to be selectable, and the app says so by declaring it.

This is the tier that makes "select past the bottom of a scroller and keep
going" work, and it covers every non-virtualized app.

**Tier 3 — never built. Described here, and not built: see
`crates/kui-core/tests/virtual_selection.rs`, which pins what a selection
over virtualised rows does today — the ends stop resolving, so nothing
paints and nothing copies until the rows are built again. It is not
*wrong* (the ends are addresses, the state survives, and a row keeps its
key because `open_indexed` derives one from the data index), but a reader
who selects a screenful of a log, scrolls, and presses Cmd-C gets
nothing.** The core reports the endpoints and the app fills
the middle. A copy over a range whose interior was never built emits a
`{kind:"selectionrange", from:{index, byte}, to:{index, byte}}` event that
the app answers with the text, through the reply channel ADR 0014 decision
6 established (`reply_sink` has been on `KuiEvent` since ABI 10 —
`crates/kui-ffi/src/abi.rs:116`). What the core has it fills in itself;
what it does not it asks for; what the app declines to answer is a copy of
the built part, not a lie about the rest.

The rule this makes explicit, and which the docs must state plainly: **the
core never invents content it was not given.** Selection over a virtual
list is exactly as complete as the app's answer.

### 4. Cells select in cell space, and address by absolute line

A `cells` grid is a selection scope of its own with its own geometry:
endpoints are `(line, col)`, the default is linewise (wrapping at the
right edge, the way a terminal selects), and a modifier gives the
rectangular block selection every terminal also has. Copy reads `ch` out of
the grid the app handed us and trims trailing blanks per line — the one
rule that makes a copied terminal screen paste like text rather than like a
rectangle of spaces.

The grid is one screenful, so a row number is not an address: it means a
different line after every scroll. `cells` therefore gains **`originLine`**,
the absolute line number of row 0, stamped by the app that owns the
scrollback. An anchor is `(absolute line, col)`; inside the grid the core
paints and copies; outside it, tier 3 asks the app for the lines by number.
An app that never sets `originLine` gets 0 and a selection that is only
correct while it does not scroll, which is the honest behaviour for an app
that did not say where its screen sits in its history.

Clicks and drags already carry `cell: {row, col}`
(`crates/kui-core/src/schema.rs:1392`), so an app that wants to own terminal
selection entirely still can; this makes it unnecessary rather than
impossible.

### 5. The menu is IR the driver may render natively

A context menu becomes **data the core produces and a host consumes**,
drained like every other host-facing consequence
(`take_window_commands`, `take_audio_commands`, `take_announcements`): a
`MenuCommand` carrying the anchor point, the window, the node, and the
items — each an id, a label, an enabled flag, an optional accelerator, and
a **role** from a closed set (`Cut`, `Copy`, `Paste`, `SelectAll`,
`LookUp`, `Separator`, `Custom`). Choosing an item posts an event on the
node; dismissing posts nothing.

Who draws it, in order of preference:

1. **The host, natively.** The Rust runner renders an `NSMenu` on macOS and
   a `TrackPopupMenu` on Windows. This is what buys the system look, the
   system keyboard behaviour, and — the reason it matters here — Look Up
   and Services, which cannot be drawn.
2. **The core, into the frame.** With no native renderer (Linux today,
   every binding whose host has not implemented one, every headless test),
   the core builds a stock menu subtree through the filler that already
   runs after the app's view and before layout
   (`crates/kui-core/src/ui.rs:613`). It is `widgets::` code building
   ordinary nodes into an ordinary frame — floated, focus-ringed,
   arrow-navigable, dismissed by the modal rules ADR 0003 already defines.
   No new overlay layer, no core-owned window, and it lands in the
   conformance corpus like any other scene.

**Automatic, with the app's declaration winning.** A secondary press or a
force click inside an `edit` or a `selectable` scope produces the default
items for that scope — Cut/Copy/Paste/Select All for an editor, Copy/Select
All/Look Up for static text, all correctly disabled when the selection is
empty or the clipboard has nothing. A node that declares `onContextMenu`
gets today's event and no menu: the app asked to own it, and one of the two
must win by declaration rather than by luck.

**The stock menu is exported, not hidden.** The fallback renderer is a
public widget — `widgets::context_menu(ui, &items)` and a `context_menu_with`
beside it for content the item vocabulary cannot express — and the core's
automatic path calls exactly that function. This is F24's pattern for the
stock button (`ElementDef.jsx_rows` + `widgets::button_with`), and it earns
the same three things it earned there: an app that declares
`onContextMenu` to add two items of its own gets our layout, our keyboard
navigation, our dismissal and our access rows instead of reimplementing
them; the corpus tests one menu rather than two; and the core's menu cannot
drift from the one apps see, because it is the one apps see.

~~For the bindings, the same widget is a **`menu` element**~~ —
**amended while building step 2 (2026-09-09): it is a verb, not an
element.** The bindings get `openMenu(target, at, items)` /
`open_menu(...)` / `kui_open_menu(...)`, mirroring `Core::open_menu`, and
the items' choices come back as the `menu` event on the target.

The reason is that this decision was written before the core held the
menu. Once it does — and it must, for the automatic path to open one —
an element is the wrong shape twice over. It would make the app re-declare
an open menu on every frame and own "is it open" in its own model, which
is a second copy of state the core is already keeping; and it would cost,
per binding, an opcode, an encoder, a decoder, a parser arm and a C
function, against a verb's handful of lines. What it would buy over the
verb is nothing an app can see: the item list is plain data through either
door, which is all a native renderer needs.

The Rust widget stays a widget — `widgets::context_menu(ui, at, &items)`
is what the core's own path calls and what a Rust app placing a menu
itself calls — so the "one menu, not two" property the paragraph above
argues for is unaffected. What changed is only how the other three
languages ask for one.

### 6. Force click is one input event, and Look Up is a menu role

`InputEvent::ForceClick(Vec2)`, produced by the driver on the rising edge
into stage 2, at the last cursor position. Routed the way a secondary press
is: the topmost node under the pointer, no focus move, no caret, no click,
no interference with the primary press that is still down. The ordinary
click that follows still happens — that is what macOS does, and an app that
wants to suppress it can, having heard the force click first.

What the core does with it, in order: over an `edit` or a `selectable`
scope, select the word under the pointer (the editor path is already
`Action::DoubleClick`) and emit a `MenuCommand` whose single item has role
`LookUp`, carrying the word and the caret rect; the macOS runner answers
that role with `showDefinitionForAttributedString:atPoint:` rather than by
opening a menu. Elsewhere, emit `{kind:"forceclick", x, y, tag}` for a node
declaring `onForceClick`, so a force click on a chart or a map is available
to apps that want the gesture for something else.

Two platform details go in the driver, not the core: the rising-edge
detection with its hysteresis, and — if measurement shows winit's default
view configuration does not deliver stage 2 —
`NSPressureConfiguration(pressureBehavior: .primaryDeepClick)` set on the
content view at window creation, reached through the raw window handle the
way `windows_nc.rs:69` already reaches a Win32 one.

### 8. A drag moves by what the press counted

**Added 2026-09-09, from the field.** A press arms its drag with a
granularity: one click drags by characters, two by words, three by whole
runs. Both ends round outwards to that unit, not just the live one — a
double-click-drag turned back on itself keeps the word it started in
whole, which is the difference between a gesture that selects words and
one that snaps to them.

This is not novel; it is what every text UI does, and what the stock
`<edit>` already did without being asked, since cosmic-text's
`Selection::Word` expands both ends as the cursor moves. Only a
`selectable` scope had to be taught, and the editor's half now has a test
so it cannot quietly stop working.

A `cells` grid drags in cells at every click count. A word in a terminal
is the app's idea rather than the grid's, and nothing has needed one yet.

### 7. A copy carries the formatting the text declared — and not the theme

**Added 2026-09-09, while building steps 4 and 5.** A selection knows more
than its characters: the core shaped it, so it still holds the weight, the
slant and the per-span colours the view asked for. Throwing that away at
the clipboard makes "copy" mean less than it does in every other app, and
keeping it is a second flavour beside the plain text, not instead of it —
`set_html(html, alt_text)` on macOS, Windows and Linux alike, so an editor
that understands HTML takes the formatting and every plain-text field
takes the words. A clipboard whose only flavour is HTML pastes markup into
half the machine, which is why the two always travel together.

What travels is **bold, italic, and a span's own colour**. What does not
is the node's colour. That distinction is the whole decision: a paragraph
drawn light grey on a dark card is grey because of the app's theme, and
pasting it into a white document as grey-on-white is exactly how "copy
with formatting" earns its bad name. A `rich_text` span that declared a
colour is the other case — that colour is authored, the way a highlighted
keyword is — and it travels.

The mechanism is that cosmic-text keeps the attributes it shaped with, so
`AttrsList` still answers for the run long after the spans that declared
it are gone. One catch worth writing down: `spans_iter` lists only the
ranges something *changed*, so a plain paragraph has none at all and reads
its line's defaults — a walk that asks per character and coalesces is what
covers both.

Not carried: a `cells` selection, whose colours are the app's grid rather
than the text's, and a long line, which holds one style throughout.

## Considered options

**A document model over the whole tree.** Retain enough of every frame to
order and address all text everywhere, and selection works uniformly with
no tiers. Rejected: it is a retained-mode purchase for one feature's
benefit, it costs on every frame including the ones with no selection, and
ADR 0016 has just finished declining a far cheaper retention (the frame
cache) on the same grounds. Tiers 1 and 2 cover every non-virtualized app,
and tier 3 gets the rest with the app's help.

**Per-node selection only** — a selection cannot leave the text node it
started in. Rejected: the complaint that started this is that a paragraph
built from three labels cannot be selected, and per-node selection answers
none of it. It is also barely cheaper than scoped: the machinery that makes
a scope work (`runs_of`, the concatenation, `hit_at`) is machinery that
already exists precisely because a node's text can be several runs.

**Native menus only.** Rejected: nothing on Linux, nothing in a headless
test, nothing in the conformance corpus, and nothing an app can theme. A UI
toolkit whose menus cannot be tested by the corpus that tests everything
else has a hole where its most-clicked surface is.

**Drawn menus only.** Rejected for the opposite reason: Look Up, the
Services menu and the system's own keyboard conventions are not drawable,
and on the platform that started this conversation they are the point.

**Leave it to apps** (today's answer). Rejected by evidence: two field apps
built two different context menus, neither has Look Up, and both had to
invent word selection out of `text_hit`.

## Consequences

**What an app can delete.** Hand-rolled context menus over `onContextMenu`;
per-app word selection built on `text_hit`; per-app selection highlighting
built out of overlay rects; per-app Cut/Copy/Paste keyboard handling for
static text. A terminal app can delete its own selection layer entirely if
it wants ours, or keep its own on `cell` payloads if it does not. An app
that keeps its own menu keeps only its *items*: the column of `onClick`
rows, the float, the dismissal and the keyboard navigation under them all
become one `menu` node.

**What stays impossible, and should be documented as such.**

- Selecting *between* two scopes. Two `selectable` cards are two
  selections; the second press clears the first.
- Selection inside a `fragment` — a WGSL function paints pixels and the
  core knows no text there (ADR 0015).
- A long line's chunked path (backlog C19) does not join `runs_of`'s
  concatenation, by construction: a 100k-character line is queried alone.
  Either it joins in step 1 or a selection touching one is refused with a
  warning rather than silently truncated. **This is the open question
  below.**
- A copy whose interior was never built and whose app declines to answer:
  the built part, and nothing pretending to be the rest.

**Perf.** Three costs, each to be measured rather than asserted: one
`Option<Key>` per `TextPlace`; one extra `places.push` per *culled* text
node **inside a scope** (tier 2), which no app pays until it declares one
and which is then proportional to what it declared — `frame_10k_chips_wrapped`
is the bench that would see it, with a `selectable` wrapper added; and the
selection highlight, which is per *visible* run only and mirrors a loop the
editor already runs. A frame with no `selectable` in it should measure
identically to today's, and that is the assertion the bench has to defend.

One second-order cost worth watching, and not one this ADR introduces: an
app whose built text exceeds the cache budget is over budget on *every*
frame, and `evict_to_budget` then collects and sorts a filtered vector each
frame to evict nothing, since every live entry is protected
(`crates/kui-core/src/text.rs:705`). Tier 2 does not change what is built,
so it does not change that — but a `selectable` log viewer is exactly the
app that would meet it first, and it should be measured in the same pass
rather than discovered in the field.

**ABI: nothing moves.** `KuiSpec` gains trailing rows (`selectable`,
`onForceClick`, `originLine`), and the crate's own rule is explicit that an
[in] struct's append is *not* a bump — a host built against the older
header passes the shorter struct, the library reads the prefix it was given
and takes the rest as zero (`crates/kui-ffi/src/abi.rs:52`, and `accent`
did exactly this after ABI 9). The menu drain is a **new symbol** with a
new [out] struct (`kui_take_menu_command`), which by the same rule is not a
bump either: a host that never calls it is unaffected. So the version moves
only if some [out] struct a host already reads has to change shape, and
nothing here says it must — a claim step 2 should re-check rather than
inherit.

**Testing, honestly.** The core half — scopes, ordering, painting, copy,
the stock menu, cells selection — goes in the conformance corpus as two
scenes (`selection`, `cells-selection`) replayed by all four adapters,
which needs no new `Step` kinds at all: `Cursor`, `MouseDown`, `Cursor`,
`MouseUp` already spell a press-drag-release, and the corpus has had them
since the pointer scenes. The native menu renderers and the pressure arm are **eyes-only,
permanently**: `AXPress` cannot produce a trackpad press, the smoke round
cannot see an `NSMenu`, and no CI machine has a Force Touch trackpad. That
is a real and permanent hole in a project that tests everything else, and
it argues for keeping the platform layer as thin as it can be — detection
and one AppKit call — with every decision above it in the core, where the
corpus can see it.

**Staging.** Five steps, each shippable alone:

1. Scopes: `selectable`, the scope on `TextPlace`, hit and drag ownership,
   the highlight, copy, tier 2. This alone answers "selection outside the
   viewport". **Built 2026-09-09**, corpus scene `selection` included: a
   drag across a `selectable` card, replayed by all four adapters, whose
   three highlight quads and their digest are what pin the ends.
2. The menu as data: `widgets::context_menu` and the verb the bindings
   open one with, the stock renderer through the filler, the default item
   sets, `onContextMenu` still winning. The widget lands *before* the
   automatic path is wired, so the thing apps get and the thing the core
   opens are the same code from the first commit. **Built 2026-09-09**,
   with the amendment above (a verb, not an element) and a corpus scene
   `menu` — the `selection` tree under a secondary press, since the menu
   is the core's and no binding declares it.
3. Native menu renderers in the Rust runner: macOS first (it is the one
   with Look Up), Windows second. **macOS built 2026-09-09**; Windows
   still open. The core half is a seam any host can use —
   `set_native_menus` makes the core hold the menu and draw none of it,
   and the host answers with `activate_menu_item` — so a C host on any
   platform can render one too.

   The one thing this step turned out to be *about* is re-entrancy.
   `popUpMenuPositioningItem:atLocation:inView:` runs a nested modal run
   loop, winit's macOS event handler `panic!`s when re-entered, and its
   run-loop observers fire in `NSEventTrackingRunLoopMode` — so a menu
   popped from inside `window_event` kills the app the first time the
   pointer moves over it. The menu is therefore *scheduled*
   (`performSelector:withObject:afterDelay:` at zero delay) and shown from
   the run loop with no winit handler on the stack, which also means the
   answer arrives a turn later and is collected in `about_to_wait`.
4. Cells: grid selection, `originLine`, linewise and block, copy with
   trailing-blank trimming. **Built 2026-09-09.** A grid selects only when
   it declares `selectable` — the ADR said "a `cells` node is its own
   scope", which it is, but making every terminal selectable without
   asking would surprise an app that draws its own selection over `cell`
   payloads. It never joins the text scope around it either way.
5. Force click: the pressure arm, the `forceclick` event, `onForceClick`,
   and Look Up as a menu role.
6. **Tier 3, unbuilt.** Two halves, and the first is worth having on its
   own. *Ordering:* an endpoint whose node is not built cannot be placed
   among the ones that are, which is why nothing paints — the fix is the
   data index decision 2 already calls for, recorded for the indexed nodes
   inside a scope, so the ends can be ordered against built rows and the
   built middle can be painted. *Filling:* the `selectionrange` event the
   app answers, so a copy over a gap is the app's text rather than a
   silent hole. Until then a virtual list selects only what it has built,
   and says so by going blank rather than by copying half a document. **Built 2026-09-09**, and the least
   verifiable thing in the repo: the core routing has tests, the Look Up
   row and its `MenuAction` reach the driver through the drain Copy and
   Paste already use, and the last two inches — `showDefinition` putting a
   panel on screen, and stage 2 ever arriving — need a Force Touch
   trackpad and a finger. Both are one call each, and both are written
   down here rather than left to be discovered.

Steps 1 and 2 are the ADR; 3, 4 and 5 are each independently declinable
without stranding the others.

## Open questions

1. ~~**Long lines (C19).** Join the concatenation, or refuse a selection
   that touches one?~~ **Decided (2026-09-09): they join.** A line being
   long is an implementation detail of how it was shaped — chunked past
   `LONG_LINE_BYTES` so a 100k-character line does not reshape as a unit —
   and it must not be a detail the user can feel by dragging across it. So
   `runs_of` learns about chunks: a long line contributes its chunks in
   order, at their own offsets within the line, and the line contributes
   its content length to the running base like any other run. The cost is
   that `long_place`'s "queried alone" shortcut stops being the whole story
   and the two paths have to agree about offsets — which is exactly the
   sort of thing the corpus scene is for.
2. ~~**Does winit's default view deliver stage 2?**~~ **Answered by
   declaring it (2026-09-09).** winit never calls
   `setPressureConfiguration:`, and the question of what its default does
   is not one this project can observe — nothing synthesises a trackpad
   press, so the answer would be a guess either way. So the driver asks
   for what it wants: `NSPressureBehaviorPrimaryDeepClick`, the behaviour
   Quick Look and every force-clickable text view use, set on the content
   view before the window is shown. That is one call, it cannot be wrong
   about a default that may change, and it is the same call the answer
   "no" would have required.
   Measurable in an afternoon on real hardware, and worth measuring before
   the step is planned rather than during it.
3. **Selection in the access tree.** AccessKit models a text selection;
   a screen reader ought to hear ours. Deferred to a follow-up rather than
   guessed at here — ADR 0001's rule is that the tree is derived from what
   the core knows, and after this it knows something new.
4. **Copy's join rule between runs.** Newline between runs whose origins
   differ vertically, space otherwise, is the obvious first answer and is
   wrong for inline runs that wrap. It should be decided against a real
   paragraph of mixed styles, not in the abstract.
