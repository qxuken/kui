---
status: accepted
date: 2026-09-04
amended: 2026-09-04
---

# The paint vocabulary: group opacity and shadows in, gradients out, exit animations designed

> **Amended 2026-09-04, twice.** Exit animations are built, as
> `crates/kui-core/src/depart.rs` and one `exit` schema row. The design
> below held; two things changed, both recorded in
> [Amendment: exit animations, built](#amendment-exit-animations-built) —
> where the departing subtree is copied *from*, and the answer to the open
> question about `animating()`. And **rounded clipping is built too**: the
> consequence below prices it as "a bigger bill than either of these" on an
> estimate, and
> [Amendment: rounded clipping, built](#amendment-rounded-clipping-built)
> replaces the estimate with the measurement, which is about 1% of a frame
> that never clips. Both amendments are at the end of this document.

`crates/kui-core/src/display.rs` was the whole renderer contract — fill,
border, four radii, glyph, image — and three things a real UI wants were
missing from it: **group opacity**, **shadows** and **gradients**. A fourth,
**exit animations**, was blocked on the first: `Enter` can fade a node's own
`bg` but not a subtree, so a panel could fade in and never out, and the
README said so ("a removed node vanishes at once").

We shipped **group opacity** as a `VisualStyle::opacity` multiplied into
every quad a subtree emits, and **drop shadows** as a new
`QuadKind::Shadow` carrying a blur radius, rendered by one more branch in
the existing SDF. We are **not** shipping gradients in v0. We are **not**
building exit animations yet, and the design below is what building them
would mean, written down so the next person starts from the shape of the
problem rather than from scratch.

## Context

- **The quad struct is hot.** `benches/frame.rs` runs 10k-node frames;
  anything added to `Quad` is paid 20,000 times a frame. That argues for
  fields that many quads use, and against fields one kind uses — and it is
  why `opacity` is not on the quad at all.
- **Every addition costs four bindings.** A plain `PROPS` row is nearly
  free: Lua looks it up by snake\_name, Node parses it by kind, the TS types
  and `docs/props.md` regenerate, and only `KuiSpec` needs a field, which
  `every_schema_prop_has_a_c_counterpart` then forces. A *composite* is
  hand-written four times. Six plain rows beat one composite here.
- **The frame model is immediate.** `Tree` is rebuilt every frame from what
  the view declares; the only thing that survives a frame is keyed retained
  state (`AnimStore`, `ScrollStore`, `EditStore`), and none of it holds a
  *node*. A node the view stops declaring is gone before `finish_frame`
  runs. That is the whole difficulty of exit animations.
- **`Enter` already proves the pattern.** A first-sighted node's slots start
  somewhere other than where the view declares them, stated as data. Its
  mirror image — where a *last*-sighted node's slots end — needs the node to
  still exist, which is a different kind of change.

## Decision

### Group opacity: a spec slot, multiplied at emission

`VisualStyle::opacity` (0..=1, default 1) is inherited multiplicatively down
the tree in `finish_frame`'s existing pass 1, beside the clip propagation
and gated the same way (`any_opacity`, so a frame that fades nothing pays
one branch). `emit_node` records where its quads begin and multiplies the
inherited product into `color.a` and `border_color.a` of everything it
pushed. Scrollbars and the focus ring fade with the node they belong to.

Consequences we accept:

- **It is not a composite.** Overlapping pieces of one subtree double-blend
  where a real offscreen composite would not — the classic seam through a
  faded stack of translucent boxes. Compositing means a render target per
  faded subtree, a second pass, and a `DisplayList` that stops being one
  flat list; that is a renderer redesign, not a paint prop. The doc comment
  and `docs/props.md` say this outright rather than letting someone discover
  it.
- **Nothing but paint changes.** A faded subtree still lays out, still takes
  clicks, and is still read out by assistive technology — CSS's rule for
  `opacity: 0`, and the one that makes a fade-*out* usable at all, since the
  thing being faded out is normally still interactive on the way.
- **It animates.** `Slot::Opacity` joins the tween slots, `Keyframe::opacity`
  and `Enter::opacity` join their parse tables, so `enter: { opacity: 0 }`
  with a `transition` fades a whole panel in. `KuiEnter` / `KuiKeyframe`
  gain a `set` bit each.
- **C needs `opacity_set`.** A zeroed `KuiSpec` is the schema default, and
  the default is 1, so 0 cannot double as "unset" the way every color field
  does. The bit is the price of keeping a fully transparent subtree
  expressible from C.

### Shadows: a `QuadKind`, not a nine-slice

`QuadKind::Shadow` plus one `blur: f32` on `Quad`. The core emits the
shadow as the node's rect moved by `dx`/`dy`, grown by `spread`, inflated by
`blur` on every side; the shader insets by `blur` again to recover the
shape, and ramps the edge over it with the `sd_rounded_box` it already has
for rounded rects. Five plain schema rows (`shadowColor`, `shadowBlur`,
`shadowX`, `shadowY`, `shadowSpread`) and two tween slots (the geometry as
one four-vector, the color as another).

Against the nine-slice: it needs a blurred texture per (radius, blur) pair
in the glyph atlas, an atlas eviction policy for something that is not a
glyph, nine quads where this needs one, and seams to get right at
fractional scales — all to avoid about fifteen lines of WGSL. The ramp is a
smoothstep rather than a true Gaussian, which at UI blur radii does not
read; if it ever does, the fix is a better curve in one shader function,
not a different architecture.

Scope kept deliberately small: **outer shadows only** (no inset), **one per
node**, and the shape is **not knocked out of the middle** — CSS clips the
box out of its own shadow, and a translucent background here shows the
shadow through itself. Each of those is a note in the doc comment, not a
silent surprise.

### Gradients: out of scope for v0

Two colors and a direction sound like one more `PROPS` row and are not. A
gradient needs a stop list (so: a `Keyframes`-shaped parse, in five
bindings), a type (linear, radial, conic), a geometry (angle or two points,
in the node's own space), and a decision about interpolation space that
looks arbitrary in every choice. On the quad it is at minimum a second
color, a direction, and a discriminant; via the atlas it is a texture per
distinct gradient. Neither is a paint prop's worth of work, and the
workaround — an image, or a stack of solids — exists. The README says v0
has no gradients rather than leaving a reader to infer it.

### Exit animations: the design, not the build

Not built. What building it means:

1. **A departing node is retained, not re-declared.** `finish_frame` diffs
   this frame's keys against the last frame's. A key that was present, is
   absent now, and whose last spec declared both a `transition` and an
   `exit` becomes a *departing subtree*: its specs, contents and laid-out
   rects are copied out of the old `Tree` into a `DepartStore` keyed by
   `Key`, alongside the frame time it left.
2. **It is replayed, not re-laid-out.** On every later frame the departing
   subtree is appended after the live tree with its rects frozen at the
   moment it left, its `exit` slots eased from where they were toward what
   `exit` declares (`Enter` in reverse — `dx`/`dy`, size, `bg`, `radius`,
   `opacity`), and emitted like a float: on top, escaping ancestor clips,
   because its ancestors may be gone. Freezing is the key simplification —
   a dying node must not fight the live layout for space, and CSS's own
   exit transitions work the same way (the element is out of flow the
   moment it is removed).
3. **It is inert.** No hit region, no place in the Tab ring, no access row —
   it is a picture of a node, not a node. A departing subtree that held
   focus has already given it up, by the same path a removed modal does.
4. **It is dropped when its transition ends**, or when the same key comes
   back (the live node wins immediately and the ghost is discarded, so a
   toast dismissed and re-shown does not double), or after a frame budget —
   `AnimStore`'s existing `EVICT_AFTER_FRAMES` sweep is the model, and the
   store must be bounded, since a view that removes a thousand rows at once
   must not retain a thousand subtrees indefinitely.
5. **What it costs.** `Tree` grows a way to be sliced and copied, which it
   does not have today; `finish_frame` grows a key diff over the previous
   frame (a `FxHashSet` of the last frame's keys, or a generation stamp per
   key in the existing map); and every emission pass has to tolerate nodes
   whose parents do not exist. The spec grows one `exit: Option<Enter>`
   field, which needs no new parse code — an exit *is* an `Enter`, read the
   other way — so the four bindings cost one row.

The open question, and the reason this is written down rather than
built: **what a departing subtree does to `animating()`**. The driver
schedules frames while anything is mid-flight; a ghost keeps that true for
its whole duration, which is correct but means removing a node now costs
frames. That is fine for a dialog and wrong for a list that drops a
thousand rows, and the policy — cap the ghosts, cap the duration, or make
`exit` opt-in per node as designed here — should be decided with a real
view in front of us. **Answered in the amendment below: all three of the
first and none of the second.**

## Considered options

- **Opacity as a `Quad` field** rather than pre-multiplied at emission.
  Rejected: four bytes on the hot struct and a multiply in every backend, to
  express something the alpha channel already expresses. Pre-multiplying
  costs one pass over the quads a *faded* node emitted, and nothing at all
  otherwise.
- **Opacity as a true composite** (render target per faded subtree).
  Rejected for v0: it is the correct rendering and the wrong amount of
  work, and it would make `DisplayList` a tree instead of a list, which is
  the one thing the renderer boundary promises it is not.
- **Shadows as a nine-slice** out of the glyph atlas. Rejected above.
- **Shadows as a composite prop** (`shadow = { color, blur, x, y, spread }`).
  Rejected: five plain rows are free in three of four bindings; one
  composite is hand-written in all four and needs a conformance scene to
  keep them honest. The Rust builder still offers `shadow(Shadow { .. })`
  for the whole thing at once.
- **Exit animations by keeping the whole previous `Tree`** and diffing
  frame to frame. Rejected as the starting point: it doubles the retained
  frame state to serve the rare case, where a keyed `DepartStore` pays only
  for nodes that actually declared an `exit`.

## Consequences

- `Quad` grows one `f32` (104 → 108 bytes) and `QuadKind` one variant. Both
  are ABI: `KuiQuad` mirrors the layout field for field, so the C example's
  digest mirror and the JS decoder shifted with it. Measured on the 10k-node
  benches, the growth is not visible above run-to-run noise, and a 10k-node
  frame where *every* node casts a shadow under a faded root
  (`frame_10k_rects_with_shadows_and_opacity`, 20k quads) costs about what
  the plain 10k-quad frame costs.
- The `kinds` line of the conformance report grew a sixth column, and
  `Expect` a `shadows` count; the `layout` scene's card now carries both new
  props, so all four bindings reproduce them or fail.
- `docs/props.md` gains six rows and `KuiSpec` seven fields (appended, so
  the C ABI stays additive).
- Rounded clipping is not addressed here: `Quad::clip` is a rect, so a
  rounded scroll container does not round its children. Four more floats on
  the hot struct plus a second SDF per fragment looked like a bigger bill
  than either of these, and it was left in the backlog with that price on
  it. **Superseded by
  [Amendment: rounded clipping, built](#amendment-rounded-clipping-built)**,
  which measured the bill instead of estimating it: the four floats are
  invisible, the plumbing around them is about 1% of a frame that never
  clips, and it is built.

## Amendment: exit animations, built

Built as designed, with two corrections and one answer. The view they were
decided against is `examples/rust/toasts.rs`, which now has an `exit`
on every toast and on the panel, and a "clear" button that drops the whole
stack in one frame.

### The answer: `exit` is opt-in, the store is bounded, and the duration is the view's

The open question was what a ghost does to `animating()`, with three
candidate policies. The answer is that two of them are needed and the third
is not:

- **`exit` is opt-in per node, and needs a `transition` as well.** This is
  the primary control and it costs nothing: a node that declares neither is
  never a candidate, is never copied, and never keeps a frame owed. It also
  puts the decision where the knowledge is — a toast wants to leave
  visibly, a table row does not, and only the view knows which it is
  looking at.
- **The store is bounded, over nodes.** `depart::MAX_NODES` is 512;
  a departure that would take the store past it is refused, and what is
  refused vanishes at once — which is *exactly* what a node with no `exit`
  does, so the failure mode is the old behaviour rather than a wrong one.
  The budget is over nodes rather than subtrees because a node is what a
  replayed frame pays for: one dialog of forty nodes and forty toasts of
  one are the same bill.
  Silently, though, "only some of my rows animated" reads as a bug, so the
  first refusal of a frame raises an `exit-budget` warning through the
  ordinary diagnostics path, with the sentence that fixes it (put `exit` on
  the list, not on every row).
- **The duration is not capped.** A ghost runs for its own node's
  `transition`, and a view that declares a ten-second transition gets ten
  seconds of frames — which is already true of every live transition, so a
  cap here would be a second, inconsistent rule for the same declaration.
  What *is* bounded is the store's patience with a driver that stops
  advancing its clock: an unreplayed ghost is swept after
  `EVICT_AFTER_FRAMES`, the same backstop `AnimStore` keeps. And a driver
  with no clock at all gets no ghosts: every transition snaps without one,
  and an exit that snaps is the plain disappearance it always was.

So `animating()` stays honestly true while a ghost is in flight, and the
frames that costs are bounded by 512 nodes times whatever the view asked
for. The 1000-row list of the original worry animates its first 512 nodes'
worth of rows out and drops the rest at once, having said so.

### Correction: the previous frame is kept, not re-copied

Step 1 above says `finish_frame` copies the departing subtree "out of the
old `Tree`". There is no old `Tree` at that point: `begin_frame` clears it
before the view builds, so by the time a frame can notice a node missing,
the node is gone. The "Considered options" list rejected *keeping the whole
previous `Tree`* as the starting point, on the grounds that it doubles the
retained frame state to serve the rare case.

It does not have to be doubled unconditionally. `Core` holds two tree
buffers and `begin_frame` **swaps** them instead of clearing one — but only
when the frame that just ended declared an `exit` somewhere (`any_exit`,
alongside `any_clip`, `any_float` and the rest). A frame with no exits
clears the spare, so a stale tree can never be diffed against, and the cost
of the feature to an app that does not use it is one bool. To an app that
does, it is an allocation that already existed, reused a frame later
instead of immediately.

The text list goes with it. A text node holds a `TextId` into the frame's
text list, which is rebuilt every frame, so a subtree copied out of the
previous tree carries ids that only the previous list can resolve —
resolving them against the current one draws whatever text happens to sit at
that index. `TextSystem` keeps its previous list on the same condition and
in the same way, and the ghost stores the shaped buffer's cache key rather
than the id, so what it replays is its own text for as long as the text
cache still has it (and nothing, rather than someone else's, once it does
not).

The diff itself is then two flat `Vec<Key>` compared. Almost every frame
declares the same nodes in the same order, so that comparison is the whole
diff; only a frame that actually changed shape pays for the set of watched
keys (the previous frame's exit-declaring roots, plus the roots already
departing) and the per-node walk, which is one AND against a 64-bit
membership mask and a hash lookup only for the few keys that collide with
it.

### Correction: a ghost eases itself

Step 2 says the exit slots are "eased from where they were", which reads
like an `AnimStore` tween. It is one lerp instead. Nothing can retarget a
ghost — the view has stopped talking about it, so its target cannot change
— and a retained tween exists to survive retargets. The ghost keeps the
clock reading it left at and the duration and easing its last spec
declared, and interpolates. Spring easings sample as ease-out, the
substitution `AnimStore::sample` already makes for a keyframed slot, for
the same reason: there is no leg to carry momentum across.

One limit falls out of freezing: `exit`'s `width` / `height` resize the
departing *root's* own box, and nothing inside it moves, because re-laying
out a frozen subtree is the one thing the design rules out. It is a note in
the doc comment and in `docs/props.md` rather than a surprise.

### What it cost

Measured on `crates/kui-core/benches/frame.rs`, fastest of 100 samples on one
machine (the medians drift 15% with thermal state; the fastest does not),
against the same benches at the commit before.

**What everybody pays**, whether or not they use it:

| frame | before | after |
|---|---|---|
| `frame_10k_rects` (no `exit` anywhere) | 1.20 ms | 1.26 ms |
| `frame_1k_typical` | 157 µs | 164 µs |

That ~5% is not the diff, which never runs there — it is `NodeSpec` growing
an `Option<Enter>`, 60 bytes on a struct the tree holds one of per node
(668 → 728 bytes, so 600 KB more tree at 10k nodes). It is the one cost here
that lands on frames that declare nothing, and it is worth naming what would
buy it back if `NodeSpec` grows another of these: boxing `enter` and `exit`
together behind one pointer, which recovers about half of it and puts an
allocation per declaring node per frame on the views that use them. Measured,
not guessed — it was tried. Not worth it for one slot; the next slot is where
it becomes worth it.

**What the feature itself costs**, from the new benches:

| frame | fastest |
|---|---|
| `frame_10k_rects_one_exit` — one dialog with an `exit` in a 10k-node frame | 1.28 ms (vs 1.26 ms with none) |
| `frame_10k_rects_all_transitioning` — 10k nodes with a `transition` | 2.65 ms |
| `frame_10k_rects_all_declaring_exit` — the same 10k, each also with an `exit` | 3.2 ms |
| `drop_1k_rows_plain` — 1000 rows built, then dropped | 96 µs |
| `drop_1k_rows_declaring_exit` — the same, every row with an `exit` | 274 µs |
| `replay_a_full_depart_store` — a full 512-node store replayed over an empty frame | 12 µs |

The realistic shape — a dialog with an `exit` inside a big app — costs under
2% of the frame it is in, even though keeping the previous frame is per frame
rather than per exit. The pathological one — `exit` on all 10k cells of a grid
— costs about 21% over the same grid's transitions, and is the declaration
the `exit-budget` warning exists to argue with. A mass removal costs about
180 µs over the plain one for the budget's worth of subtree copies, once, on
the frame it happens; replaying them afterwards is 12 µs a frame, which is
what makes "keep drawing until the exit finishes" affordable at all.

### No corpus scene, and why

`exit` is a plain `PROPS` row of the kind three of the four bindings lower
by table lookup: Lua reads it by snake\_name, Node's encoder takes its id
straight off the protocol, and C — the one that maps by hand — is forced by
`every_schema_prop_has_a_c_counterpart`, which fails the build for a row
with no `KuiSpec` field. Node's own sweep ("every generic schema prop
reaches the stream and lowers") covers the wire. What is left is the ghost,
and that is entirely `kui-core`: there is no per-binding behaviour for a
scene to catch four of.

Pinning it in the corpus anyway would need two changes to the corpus
protocol that serve nothing else. A **clock step**, because without one
every transition snaps and there is no ghost to see, in all four adapters.
And a **per-frame builder** in the Node adapter, which builds its scene tree
once and reuses it for every frame — a departure is a tree that changed, so
a scene that never changes its tree cannot express one. Both are
worthwhile if a second cross-binding behaviour ever wants a clock; neither
is worth doing for a row that is already forced twice. The behaviour is
pinned in `crates/kui-core/tests/exit.rs` instead, including the toast-stack
shape the policy was decided against.

## Amendment: rounded clipping, built

The consequence above left rounded clipping in the backlog with a price on
it: "four more floats on the hot struct plus a second SDF per fragment is a
bigger bill than either of these". The price was an estimate. It has now
been measured, and it is smaller than the estimate implied — smaller, in
particular, than the 5% every frame already pays for `NodeSpec::exit` — so
it is built.

`Quad` grows `clip_radius: [f32; 4]` (108 → 124 bytes) and `display::Clip`
is the rect-plus-radii pair that the clip propagation, `Paint`, and the text
and editor emitters now carry instead of a bare `Rect`. A node that clips
(`clip`, `scroll_x`, `scroll_y`) and has a `radius` rounds the clip its
descendants inherit — no new prop, in any of the four bindings, because the
radius is the clipping node's own and the rule is CSS's.

### What it cost

Measured on `crates/kui-core/benches/frame.rs`, fastest of 100 samples over
14 interleaved rounds on one machine, against the same benches built from
the commit before — the *same bench file* on both sides, including the two
benches added for this, so the ~5% a bench binary moves by gaining functions
is not being read as a result. Two copies of the baseline binary ran in the
same rounds; they differ by −0.6% to +0.4%, which is the method's noise floor
and the number every row below has to beat to mean anything.

| frame | before | after | |
|---|---|---|---|
| `frame_10k_rects` (nothing clips) | 1.269 ms | 1.281 ms | +0.9% |
| `frame_10k_rects_with_text_and_hits` | 1.663 ms | 1.682 ms | +1.1% |
| `frame_10k_rects_with_shadows_and_opacity` | 1.384 ms | 1.398 ms | +1.0% |
| `frame_1k_typical` | 163 µs | 166 µs | +1.5% |
| `list_10k_rows_naive` (a real scroll container) | 4.93 ms | 4.98 ms | +1.0% |
| `frame_10k_rects_square_clip` (100 clipping rows) | 1.289 ms | 1.334 ms | +3.5% |
| `frame_10k_rects_rounded_clip` (the same, rounded) | 1.284 ms | 1.346 ms | +4.8% |

Read it as three numbers. **About 1% is what everybody pays**, whether or
not anything clips: sixteen more bytes on `Quad` and one more field written
per quad, 20,000 times a frame. **About 3.5% is what a frame that clips
pays**, because a `Clip` is 32 bytes where a `Rect` was 16 and it travels
through the propagation pass, `Paint`, and every emitter. And **the rounding
itself is the last 1.3 points** — inside the new build, the rounded frame is
0.9% slower than the square one (1.346 vs 1.334 ms), which is the per-corner
[`Clip::intersect`] running on all 10k nodes in the shape a real view never
declares: it rounds the card, not each of its rows.

The four floats, on their own, were measured first and separately, against a
one-float `clip_radius` that could not express a per-corner radius: at
108 → 112 vs 108 → 124 bytes the two were indistinguishable from each other
and from the baseline. The generality is free; it was the plumbing around it
that cost the 1%.

The shader side is one more `sd_rounded_box` per fragment, taken **instead
of** the rect test rather than after it, and only on the branch where
`clip_radius` is not all zero — which it is on every quad of every frame
that has no rounded clipper. The value comes off the instance, so the branch
is uniform across a quad's fragments. That is not measured on a GPU here;
what is checked is that both preprocessed variants of the shader still
validate (`shader_variants_validate`) and that the shape the shader cuts is
the shape the core described, by mirroring `shade`'s clip block on the CPU
and evaluating it over quads a real `Core` emitted
(`crates/kui-wgpu/tests/rounded_clip_coverage.rs`, beside the shadow work's
`shadow_coverage.rs` — both now share one transcription of the WGSL in
`tests/wgsl`).

### One rounded rect cannot name the intersection of two

The clip a node inherits is one rect and four radii, and clippers nest, so
the intersection of two rounded rects has to collapse into that shape.
`Clip::intersect` decides **per corner**: a corner takes whichever of the
two shapes rounds it more — the intersection of two rounded corners is the
tighter one, so that is exact — and keeps a radius at all only while that
corner of the result is still the same point as that corner of the shape it
came from. A corner an ancestor's straight edge has already cut away is
square, which is what the ancestor made it, so that is exact too.

The one case it approximates: an ancestor edge that cuts *partway* into a
rounded corner moves that corner, so the radius drops to zero there and a
sliver at the very corner goes unclipped. It takes two rounded clippers
offset from each other to build. The alternative is carrying the rounding
shape's own rect alongside the intersection — eight more floats rather than
four, to serve a shape nobody has drawn yet.

### What is deliberately not rounded

- **Hit testing.** `HitRegion::clip` stays a rect: a click in the corner of
  a rounded scroll container still reaches the row under it. Browsers do the
  same for descendants of a rounded `overflow: hidden`, and the alternative
  is a per-corner test in the hot input path for a few pixels.
- **Culling.** The "entirely clipped away" test in `finish_frame` is still
  the rect intersection, so a node that survives only inside a corner's arc
  is emitted and clipped rather than dropped. Cheaper, and never wrong in
  the visible direction.
- **A clipper's own quads.** `clips[i]` is ancestors only, as it always was,
  so a rounded card is not clipped by itself — it draws its own rounded
  shape through `radius`, which is the same shape.
- **Scrollbars**, for the same reason: a container does not round away its
  own bar.

### The option not taken, and what would buy it back

The cheapest version of this feature makes `Quad` *smaller*: replace
`clip: Rect` with a `clip_id: u32` into a `DisplayList::clips` side table
(there is one entry per clipping node, not per quad), which is 96 bytes with
full rounded clipping instead of 124 without. It was not taken because it
breaks what the renderer boundary promises: a quad stops being
self-contained, every backend grows a second array to bind, and a C host
reading `KuiDrawData.quads` has to resolve an index. If `Quad` ever has to
grow again, that is the trade to reopen — and it would pay for the growth
twice over.

### ABI

`KuiQuad` gained the field and `KUI_ABI_VERSION` went 1 → 2, which is
exactly the rule ADR 0006 wrote down: `KuiQuad` is a **[lib]** struct, the
host strides the array with its own `sizeof`, and there is no in-band
handshake to catch a mismatch. `conformance::quad_digest` hashes the new
words (0..=18 and 23..=30 of a now 31-word struct), so a binding whose
mirror of `KuiQuad` missed the field fails every scene rather than none —
and the corpus's `overflow` scene now rounds its scrolling list, so those
words are not all zero.
