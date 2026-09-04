---
status: accepted
date: 2026-09-04
---

# The paint vocabulary: group opacity and shadows in, gradients out, exit animations designed

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
view in front of us.

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
- Rounded clipping is still not addressed: `Quad::clip` is a rect, so a
  rounded scroll container does not round its children. Four more floats on
  the hot struct plus a second SDF per fragment is a bigger bill than
  either of these, and it is left in the backlog with that price on it.
