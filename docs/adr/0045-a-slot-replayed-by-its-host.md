---
status: accepted
date: 2026-10-09
---

# A slot replayed by its host: the extension is spared when the host says nothing it feeds it changed, and the core checks everything else

> **Accepted and built (2026-10-09), for alpha.48; backlog F142.** This
> amends [ADR 0016](0016-caching-against-the-last-frame.md) — decision 2
> there refuses a `memo` element "in the form that asks the view to
> declare its own dependencies", and this is not that form, but it is the
> same move one level up and the refusal's reasons are answered here one
> by one rather than waved at. What changed between the two documents is
> a measurement: ADR 0016 priced the app's own building at 40% of a
> 717 µs frame of Rust pushes, about 0.03 µs a node; a Lua extension's
> fill costs about 2 µs a node *before* it reaches `Tree::push`, in the
> binding's walk from its tables to the tree, and that is the half of the
> bill no primitive of kui's can reduce, because the nodes are not the
> cost — the conversion is.

A host that puts a plugin's pane beside its own content runs the plugin's
`view` every frame the window draws, whatever brought the frame: a key in
the host's own editor, a caret blinking in a field, a pointer moving over
a title bar. For a Rust extension that is the same tenth of a microsecond
a node the host pays for its own tree. For a Lua one it is not: kawoosh's
settings pane — 1,561 tables for a dozen settings on show — spends
1.25 ms running the view and **3.0 ms** in kui-lua turning the tables it
returned into nodes, on frames where the pane did nothing, against 0.3 ms
for kui's whole layout and 0.25 ms for the editor beside it; a frame after
a pause runs cold and the same pane costs 14–22 ms at the caret's 2 Hz.
(The kawoosh perf log of 2026-10-09, release build, median over forty
frames each: settings 5.9 ms a frame against 1.1 ms with no pane; themes
6.2; theme lab 4.6; a native pane would pay the C ABI's per-node calls the
same way, smaller.) We decided that **the host may declare a slot as
unchanged** (`Ui::slot_replay`), that **the core then checks everything
it can see itself** — the slot's params, every fact of the frame the kept
fill read, where the slot sits — and **pushes the kept fill's nodes again
through the doors they came in by** without asking the extension, or
runs the extension and says why; that **a fill is kept only when the host
asked** (`Ui::slot_kept`) and forgotten the frame it is not declared;
that **a nested slot is declared again and filled fresh on a replay**;
and that **a fill the journal cannot vouch for is refused whole**, by a
count of the nodes it pushed against the nodes it journaled.

## Context

- **ADR 0016's decision 2 and its reasons.** "An element that means
  'trust me, nothing under here changed' moves the correctness obligation
  from the core to every app, and the class of bug it admits — a view that
  forgot a dependency, so the screen shows something that stopped being
  true — is exactly the class immediate mode exists to make impossible."
  And: "kui's answer to 'my view is too expensive to run every frame'
  stays *build less*." Both reasons stand and both are addressed below;
  neither is overruled.
- **What a slot already is.** A position the host declares with params
  "declared every frame and never retained" (ADR 0014), filled then and
  there by an extension the host loaded, its nodes keyed under the slot's
  own key and tagged with the extension's origin, its events routed back
  by that origin. The host is already the one who says what the extension
  reads from it; the core already trusts the host for every other
  per-frame fact (the title, the focus, the declared windows).
- **What the core can see of a fill's inputs, and what it cannot.** A
  fill reads two kinds of things: facts of the frame, through the core's
  own doors — `is_hovered`, `focus`, `scroll_offset`, `layout_of`,
  `caret_visible`, `now`, `measure_text`, the env reading — and facts of
  the host, through the host's own API (an editor's buffers, a settings
  table), which the core never sees. The first kind can be *recorded with
  the value read* and compared next frame, exactly; the second kind is
  the host's, and only the host can say whether it moved.
- **What a replay can and cannot re-issue.** A fill pushes nodes, and may
  also declare things of the frame — a window title, a frame it wants
  next, a devtools tab, a loaded extension — or draw through a door that
  carries a picture rather than a spec (`cells`). The nodes come back from
  a journal; the declarations do not, and a fill that makes them is one
  whose next frame is its own business.
- **Keys are the slot's, not the frame's.** Every node under a fill is
  keyed from the slot's key (`fill_within` sets the namespace), so a
  replay of the same ops under the same slot key yields the same keys —
  which is what keeps hover, focus, scroll offsets, transitions, edit
  buffers and exit diffs where they were, since all of those are keyed
  state the core already retains outside the spec.
- **Hover, accent and easing are resolved at push, not in the view.**
  `prepare_spec` folds the declared `hover_bg`, the OS accent and a
  transition's eased values into the spec on the way into the tree. A
  journal that records the spec *before* that step and replays it through
  the same door resolves all three for the frame being built, so a
  replayed button lights under the pointer and a replayed card keeps
  easing, with no refill and nothing stale.

## Decision

1. **Two doors on `Ui`, the second a claim.** `slot_kept(name, params)`
   is `slot_with` and keeps what the fill built. `slot_replay(name,
   params)` is the host's claim that nothing *it* feeds the extension has
   changed since; it answers [`SlotFill`]: `Replayed`, or why it ran the
   extension instead — `NotKept`, `Params`, `Reads`, `NotReplayable`,
   `Moved` — having filled and kept it either way, so the next frame may
   replay. `None` when the slot was not declared. The C side is
   `kui_slot_kept` / `kui_slot_replay` / `kui_slot_fill` with
   `KUI_SLOT_*` codes; Lua's `fill { keep = true }` / `fill { replay =
   true }` and `env.slot_fill(name)`; Node's `<slot keep>` / `<slot
   replay>` and `ctx.slotFill(name)`. New symbols only: the C ABI version
   stays, the Node stream version moves (v23) because `slot` gained a
   word.

2. **The host vouches for its side only; the core checks the rest.**
   Before a replay the core compares the params (`Value` equality), every
   fact of the frame the kept fill read against its value now — a hover,
   a press, a focus, the caret phase, a scroll offset or geometry, a
   layout rect, a text hit, an editor's text, the modifiers, the pointer,
   the fonts' revision behind a measurement, the env reading less its
   clock and caret phase — and that the slot's key is the kept one. The
   clock is never the same twice, and the selection's text is not
   compared: a fill that read either runs every frame. The read is noted
   where the door is, in `Core`, so a stock widget's `is_hovered` inside
   the fill is caught as surely as the script's `env.is_hovered`.

3. **What is kept is the fill's ops, as the doors saw them.** Every node
   push records the key, the spec *as declared* (before `prepare_spec`),
   and what the door needs to push it again — a text's string and style,
   a stroke's points, a path's ops, an editor's label and initial, a
   fragment's handle and params, an image's handle and fit — beside the
   labels, data indices, row counts, hints and the `key_focus` asks the
   fill made, and each nested slot it declared with its params. A replay
   pushes these through the same `*_with_key` doors under the kept
   origin, inside `fill_within` as a fresh fill would be, so the tree's
   fill ranges (`ev.slot`), the key labels, the hints and the event
   routing come out as they would have.

4. **A nested slot is declared again, and filled fresh.** `Op::Slot`
   replays as `slot_with`, so whoever fills it runs. This is what makes
   the engine's field inside a script's pane blink its caret at the
   field's cost while the pane around it is not rebuilt. A `slot_kept` or
   `slot_replay` *inside* a fill being kept is a plain slot: the outer
   journal holds the slot, not the inner's nodes.

5. **A fill the journal cannot vouch for is refused whole, by count.**
   Every node a fill pushes is journaled by its door, pushed by a nested
   fill, or pushed by the core on its own behalf (a hover hint) while the
   journal is paused; at the end the nodes the tree gained less the
   nested and the core's own must equal the nodes journaled, or the kept
   fill is `NotReplayable`. So is one that declared something of the frame
   — a title, always-on-top, secure input, option-as-alt, the input
   method, a window, a devtools tab, a frame now or at a time, a widget's
   owed frame, audio, a loaded extension — or drew a `cells` grid, or
   whose view failed. The valve is what lets the journal start narrow:
   a door it does not know is a fresh fill, never a wrong one.

6. **Nothing of the frame is cached.** The tree is built from scratch,
   laid out and emitted as it always was; a replayed node is a node, hit,
   read and departed like any other. What is skipped is the extension's
   `view` and the binding's work before the first push. ADR 0016's
   decision 1 — no subtree cache of layout and emission — is untouched,
   and its list of what a digest cannot see does not apply, because
   nothing downstream of the push is reused.

## How this answers ADR 0016

- **"Moves the correctness obligation to every app."** It moves *half* of
  it, and the half it moves is the half only the app has: what the
  extension reads from the host. The other half — everything read through
  kui — the core carries, with values compared rather than inputs
  enumerated, which is the form ADR 0016's own measurement section
  recommends ("compare, do not hash"). An app that vouches wrongly draws
  last frame's pane until the next frame it vouches rightly; that is a
  real bug and a quiet one, and the answers `slot_replay` gives and
  `slot_fill` reads back are what make it a line in a ledger rather than a
  feeling. A host that cannot track what its extension reads has
  `slot_with`, and nothing changed for it.
- **"Build less."** The host did: the settings pane is a dozen rows on
  show. The cost is not the rows, it is that a Lua table becomes a node
  at 2 µs each, and `virtual_column` does not make a table cheaper to
  read. The binding's walk can be made cheaper (backlog F143), by a
  factor; a replay removes it, on the frames where the view would have
  built the same thing.
- **"The frame stays honest."** It does: no part of the core keeps a copy
  of a frame it might serve instead. It keeps what a fill *declared*,
  which is what the host would have declared again, and declares it
  again on the host's word — the same word the core takes for the title
  and the focus.

## Consequences

- A host with a plugin pane gets the pane's steady frames for the price
  of its own, and a view that reads nothing of the frame is never asked
  twice for the same picture. Measured in the bench
  (`slot_1500_nodes_fresh` against `slot_1500_nodes_replayed`): the
  Rust stand-in's fill of 1,500 nodes is 287 µs a frame fresh and 214 µs
  replayed (medians, M3 Pro), the 73 µs between them the extension's own
  pushes and nothing else — the two are close, which is the point: the
  saving is the extension's own time and the binding's, and a Rust
  extension has little of either. kawoosh's numbers are in its own log.
- **The hole, written down.** A binding that hands the env reading out
  whole (Lua's `env` table, Node's `ctx.env()`) cannot say which fields
  the script used, so the reading's clock and caret phase are left out of
  the comparison; a script that draws from `env.now` or
  `env.caret_visible` is one its host must not replay. The explicit doors
  (`Ui::now`, `Ui::caret_visible`, Lua's functions, C's) still note the
  read. A host that lends the script a proxy over `env` can close the
  hole on its own side.
- **What a kept fill costs.** One clone of each declared spec into the
  journal (a `NodeSpec` is 224 bytes plus its boxed parts) and a second
  into the tree on replay; a `Value` clone of the params; and a `Read`
  per fact read. The journal is dropped the frame its slot is not
  declared, so a pane closed costs nothing after.
- **A door added later must journal or taint.** A new way to push a node
  that does neither is caught by the count — the fill is refused, never
  wrong — but it is caught as a slot that stopped replaying, which a
  host will report as a regression. The doors are listed in
  `runtime/replay.rs`'s `Op`.
- The devtools do not yet say which slots were replayed; `slot_fill` does,
  `slot_fill_why` names the fact that moved or what the fill declared, and
  a host's own ledger can carry both.

## Considered options

**A. Make the binding's walk cheaper and leave the contract alone.**
Honest, bounded, and worth doing (F143); it does not reach the frames
where the view would have produced the same tree, which are most frames.

**B. A `memo` element the view declares with its dependencies.** ADR
0016's decision 2, refused there and refused here for the same reason:
the view is the party least able to enumerate what it read, and the core
can enumerate most of it.

**C. A core-side digest of the extension's output.** The extension would
have run to produce the thing digested; nothing is saved.

**D. The host declares the slot unchanged; the core checks what it can.
(Chosen.)** The claim sits with the party that owns the facts the core
cannot see, and nowhere else.

## Amendment to ADR 0016

A note at the head of ADR 0016 points here. Decision 2 there stands as
written for the view-declared form; the host-declared form, with the
core's own checks, is this document.
