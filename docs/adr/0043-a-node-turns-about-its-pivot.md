---
status: accepted
date: 2026-10-08
---

# A node turns about its pivot: `rotate` and `scale` on any node, paint-only, carried by the clip entry

> **Accepted and built 2026-10-08**, the day it was proposed; the
> *Amendment* at the end records what the building changed. Asked by
> berainder (backlog F131): a card that tilts a few degrees towards the
> side it is being dragged to is the swipe's whole look, and the app
> tweened `width` and `height` instead and called it depth. [ADR
> 0041](0041-a-mask-turns-about-its-centre.md) gave `rotate` to a
> `path` alone and listed "a transform on any node" under *Considered
> options* as "the general thing, and a different document" — every
> quad kind turns, glyphs leave the pixel grid, the clip stops being a
> rect in framebuffer space, a scroller inside a turned box scrolls
> along a tilted axis, hit-testing inverts a matrix per ancestor, the
> access tree's rects become bounding boxes. This is that document. It
> takes each of those costs, decides which to pay and which to decline,
> and lands on a turn and a uniform scale about a pivot, **paint-only**:
> layout is what it was, and everything after layout — the quads, the
> clip, the hits, the access rect — is drawn, cut, hit and read through
> the transform. [ADR 0005](0005-the-paint-vocabulary.md)'s paint
> vocabulary gains two slots and no quad kind; ADR 0041's path turn
> composes under this one and is otherwise untouched.

## Context

- **What a turning box costs today: everything.** No node has a
  transform. `slide`, `enter` and `exit` move and fade; ADR 0025 gave
  layout a scale factor (the DPI), not a rotation; backlog V7 declined
  a layout `zoom` — a subtree laid out in its own logical space — until
  a second app asks for it, and berainder did not: it asked for the
  *look* of a tilt, with the card taking the room its upright self
  takes. A view that wants one today fakes it (width and height eased
  against each other), or draws the card as a `path` (which cannot
  hold a photo or text), or goes to a framework with CSS transforms.
- **The vertex stage already turns.** ADR 0041 put a path's turn in the
  quad: the `blur` slot carries radians for kinds 1 and 3, and
  `vs_main` turns the four corners about the quad's centre
  (`shader.wgsl`). `local` and `uv` stay the quad's own, so the fragment
  stage's SDF, border, radii and image sampling come out right on a
  turned quad without knowing it turned. The one thing that does not
  turn is the clip, tested against the framebuffer position.
- **The clip is already a table.** Since ABI 11 a quad names a
  [`Clip`] entry in `DisplayList::clips` rather than carrying its rect
  (`display.rs`): one entry per *distinct* clip a frame reaches, a
  handful, interned by run length as emission walks paint order. Every
  quad under one card names the same entry. An entry is the space a
  quad is painted in; today that space is "framebuffer, cut to this
  rounded rect".
- **Hits are by shape.** ADR 0026 made a region's hit test the rect and
  then a shape inside it — rounded corners, a stroke's pieces, a
  polygon, a path's outline — in the region's own coordinates
  (`input.rs`, `contains`). A region's test is local already; what it
  lacks is the step from the pointer to local.
- **The slots are a list.** `anim::Slot` names nine things a transition
  tweens; an entrance and a keyframe stop name five of them through
  `Slots` (`slots.rs`), parsed in one place for every binding. A tenth
  slot is a row in that enum, a lane in that struct and a branch in
  `ease_transitioning`.
- **What a turn is for, in the apps this repo has seen.** A card that
  tilts with a drag and springs back; a pulse (`scale` 1 → 1.05 → 1 on
  a cycle); a wobble (`rotate` ±0.01 on a cycle); a spinner that is a
  box and not an arc; a chip that enters at `scale: 0.8` and settles;
  an icon that flips. Every one is a turn or a uniform scale about a
  point in the box, and every one wants to **tween**. None is a skew,
  a matrix or a 3D flip.

## Decisions

1. **`rotate`, `scale`, `pivotX`, `pivotY` on any node.** `rotate` in
   turns, clockwise with y down, 0 for none — the unit a `path`'s
   `rotate` and a gradient's `angle` already take. `scale` a uniform
   factor, 1 for none. `pivotX`/`pivotY` fractions of the node's box,
   `0.5, 0.5` — the centre — by default, the point the turn and the
   scale are about. In Rust `NodeSpec::rotate(turns)`, `scale(f)`,
   `pivot(fx, fy)`; JSX `rotate`, `scale`, `pivotX`, `pivotY`; Lua
   their snake case; C `rotate`, `scale` (0 is 1), `pivot_set`,
   `pivot_x`, `pivot_y` on `KuiSpec`; Odin the generated `Spec` fields.
   Container rows — a box, an image, an edit, a `line`, a `polygon`;
   on a `text` they are dropped with the `unknown-prop` warning every
   container row on a text raises (put them on the box around it). On
   a `path`, `rotate` and `pivot` keep ADR 0041's meaning — the path's
   own turn about a point in `d`'s coordinates, which does not tween —
   and `scale` is the node's; a path that wants a tweened turn is put
   in a box. The two should become one; see *Open questions*.
2. **Paint-only.** Layout does not see the transform: the node takes
   the room its upright self takes, its siblings do not move, a scroller
   around it scrolls the untransformed content size, `onLayout` reports
   the layout rect, `rect_of` and `layout_of` answer it. A transform is
   `opacity`'s kind of row — something applied after layout to
   everything the subtree draws — and not `width`'s. V7's layout zoom
   stays declined on its own terms; nothing here is it.
3. **A node's whole subtree turns with it, about the node's pivot.**
   Every quad the subtree emits — backgrounds, borders, shadows,
   glyphs, images, fragments, segments, masks — is drawn through the
   same similarity: `pixel = R(angle) · scale · p + t`, with `t` chosen
   so the pivot stays put. Transforms **nest** by composition: a node
   turned inside a turned node turns about its own pivot in its
   parent's turned space. A float anchored to its parent turns with the
   parent as its content does; a viewport float does not; a float
   anchored to a node by key (the devtools' tabs) does not.
4. **The clip entry carries the space.** [`Clip`] gains three fields:
   `transform` — angle in radians, scale, `tx`, `ty`, the similarity
   every quad naming the entry is drawn through — and `inner` with
   `inner_radius`, a second clip in the quad's own space, before the
   transform. The entry's `rect` and `radius` stay what they were: the
   clip in framebuffer space. A clipping node *outside* any turn
   narrows `rect`; one *inside* a turn narrows `inner`, since its box is
   in the turned space. Between two nested turns a clip is approximated
   by its bounding box in the inner space (its corners drop to square);
   a tilted card inside a tilted card inside a scroller is the one
   shape that pays, by a sliver at a corner. A frame with no transform
   interns exactly the entries it interned before, with the identity
   transform and no inner clip, so the run-length intern and every
   backend that ignores the new fields are unchanged.
5. **The shader turns the corners and tests two clips.** `vs_main`
   applies the entry's transform after a path's own turn (decision 1 of
   ADR 0041 composes under it, as that document said it would), and
   passes the pre-transform position down as a varying. The fragment
   stage tests the framebuffer clip against the framebuffer position,
   as before, and the inner clip against the pre-transform position —
   an affine function of position, so the interpolated varying is
   exact, and no inverse is computed per pixel. A segment's capsule SDF
   reads the pre-transform position too, so a turned stroke is a
   stroke. The instance grows by three `vec4`s (48 bytes); the quad on
   the wire does not change. A fragment pipeline's epilogue
   (`fragment::EPILOGUE`) tests the inner clip the same way, so a
   `fragment` inside a turned card is cut by the card.
6. **Hit where drawn.** A region under a transform carries it and the
   inner clip; `contains` unprojects the pointer into the node's space
   and tests the rect, the shape and the inner clip there, and the
   framebuffer clip with the pointer as it is. A tilted card is grabbed
   on its tilted edge; its rounded corners still miss; a press in its
   box off its outline falls through. Drag payloads stay in viewport
   px — what a drag means to the app that moves the card is where the
   pointer is on screen. A scroll region inside a turned node is hit
   the same way, so the wheel over a tilted list scrolls it; what moves
   is the content along the node's own axes, since the offset is
   applied in layout and the turn after.
7. **The access rect is the bounding box.** Assistive technology gets
   an axis-aligned rect, so a turned node's is the bounding box of its
   turned layout rect, cut to the framebuffer clip as every rect is.
   Reading order and actions are untouched.
8. **A turn and a scale are one slot, and it tweens.** `Slot::Transform`
   carries `[rotate, scale, 0, 0]`: with a `transition` a card follows
   the pointer while a drag holds the transition off and springs back
   when it is on, as `slide` does for position. An entrance names
   `rotate` and `scale` (`enter: { scale: 0.8 }` settles a chip in), an
   exit names them (`exit: { rotate: 0.1, scale: 0 }` spins a card
   away), a keyframe stop names them (`keyframes: [{ scale: 1.05, at:
   0.5 }]` pulses; `[{ rotate: 0 }, { rotate: 1 }]` spins a box) —
   through `Slots`, so every binding gets them from the one parser. A
   lane a stop or an entrance leaves out is the node's own value,
   which for `scale` is 1 and not 0: a stop that names only `rotate`
   does not shrink the box.
9. **A ghost keeps its turn.** A departing subtree is replayed with the
   transform its nodes declared, the root's eased toward the exit's;
   the turns of ancestors outside the picture — which may be gone — are
   not replayed, as their clips are not.
10. **What is declined.** A matrix, skew, 3D, a per-axis scale, a
    transform origin outside the box (`pivot` past 0..1 is allowed and
    means what it says; it is not clamped). CSS's `transform` is the
    general thing; a turn and a uniform scale are what every app has
    asked for, and each is one float that tweens. A `backdropBlur`
    under a turn blurs its upright box (the blur reads the framebuffer
    region); `pixelSnap` snaps the layout rect before the turn, so a
    turned snapped box is a turned box. Both are said in `props.md`.

## Considered options

- **The transform on the quad.** Four floats on every quad of every
  frame, for a feature almost no quad uses; and the inner clip has no
  room there at all. The clip table exists for exactly this reason
  (ABI 11), and a quad under a turned card already names the entry its
  siblings do. Not taken.
- **Rasterize the subtree offscreen and draw the texture turned.** One
  quad, exact clipping, and the way a browser composites a transformed
  layer. It costs a render target per turned subtree per frame, a copy
  of every pixel under it, and text rendered to a texture and
  resampled — blurry at any angle, which is the one thing a glyph
  turned by its own quad is not. The backdrop pass (F129) does this
  for a blur because a blur needs the pixels; a turn does not.
- **A transform on the clip only, hits and access left upright.** Half
  the build and a lie: a tilted card hit on its upright rect is grabbed
  in the air beside its corner. ADR 0026's shapes make the honest
  version cheap.
- **Unify the path's `rotate` with this one now.** The path's turn
  would become a node transform, its `pivot` a point in the box, the
  square box ADR 0041 sweeps unnecessary, and its turn would tween for
  free. It is the right end state and the wrong day: `path` is
  released, its `pivot` is in `d`'s coordinates by a decision this
  repo argued, and `path_coverage.rs` pins the sweep. Left open below.
- **A per-axis scale (`scaleX`/`scaleY`).** A flip (`scaleX: -1`) is
  the one use, and a flip of a box with text in it mirrors the text.
  The slot's lanes have room for it if an app asks.

## Consequences

- **A tilt is one row and it tweens.** berainder's card is
  `rotate(lean * 0.03)` with its transition held off while dragging,
  and the spring back is the transition it already has.
- **The instance grows.** Three `vec4`s per quad in `kui-wgpu`: half a
  megabyte more upload on a 10,000-quad frame, a few microseconds.
  Nothing on the core's side of the wire grows per quad; the clip
  table's entries grow from 32 to 80 bytes, and a frame has a handful.
- **The C ABI bumps to 27.** `KuiClip` grows (a `[lib]` struct a host
  strides with its own `sizeof`), `KuiSpec`, `KuiEnter` and
  `KuiKeyframe` gain fields, and the conformance digest hashes the
  whole clip entry. The Node `clips()` buffer's stride grows with it;
  `decodeClips` reads the new fields.
- **Every binding gets the four rows and the two slot lanes** from the
  schema and the one slot parser; the Odin layer regenerates its
  `Spec`, `Enter`, `Keyframe` and `Clip`.
- **Glyphs leave the pixel grid under a turn**, as a turned path's
  mask does: a turned text is resampled at its angle and reads
  softer. A text that must stay crisp is not turned. Under a scale, the
  SDF ramps scale with the box, so a box scaled up has an edge
  `scale` times as soft; a UI scale stays near 1.
- **The devtools' facts show `rotate` and `scale`** beside `opacity`.

## Open questions

- **The path's own turn.** When a release can break `path`, its
  `rotate` should be this one — tweened, composed, about a pivot in
  the box — and `pivot` should take the box's fractions as every other
  node's does; ADR 0041's square sweep goes with it. Condition: the
  first app that wants a path's turn to tween.
- **Text sharpness under a turn.** A glyph mask is bilinear-sampled at
  its angle. If a turned paragraph is ever asked for — not a card with
  a word on it — the answer is a supersampled mask or a rasterization
  at the angle, both of which the atlas's turn-bin rejection in ADR
  0041 already priced.

## Measurements to take

- `frame_10k_rects` and the guarded `frame` rows: the transform walk is
  behind `Tree::any_transform`, so a frame with none should read as it
  did (the guard allows the noise floor).
- A frame of 1,000 turned boxes, each its own clip entry, against the
  same frame upright: the cost of the per-node compose and the
  bounding-box cull.

## Amendment: what the building changed (2026-10-08)

- **`KuiSpec` is 744 bytes, not 728.** The five appended fields land
  after the 8-byte-aligned tail, and the parity test said so.
- **`pivot_set` is two bits.** A C host setting one axis — the parity
  test's one-row-at-a-time check was the first — needs to say which, so
  `KUI_PIVOT_X` and `KUI_PIVOT_Y` are bits as `KUI_VALUE_*` are, and the
  Odin lowering ORs them in. `scale` stays "0 is 1".
- **The slot bits are `KUI_KF_ROTATE` 64, `KUI_KF_SCALE` 128,
  `KUI_ENTER_ROTATE` 64, `KUI_ENTER_SCALE` 128.**
- **A glyph cull reads the clip in the quad's space.** The text, cell
  and editor painters compared glyph positions against the entry's
  framebuffer rect; under a turn that is the wrong space. They read
  `Clip::visible` — the inner clip narrowed by the outer one pulled
  back through the transform — which is the rect itself when nothing
  turns.
- **The pre-transform position is a varying, not an inverse.** `pre` at
  location 10 of `VsOut`; the fragment epilogue takes it at the same
  location, with `inner` and `inner_radii` at 11 and 12. A fragment
  entry point lists a subset of the vertex outputs, so an app's
  fragment compiled against the old prelude still links.
- **A scrollbar's thumb under a turn is hit upright.** The bar is drawn
  turned (its quads name the scroller's entry) but its track is tested
  as an upright rect. A turned scroller is a demonstration shape, and
  the wheel over it is right; the thumb waits for an app that drags one.
- **The Odin layer was regenerated by hand.** No Odin ran on the
  building machine (its binary wants an LLVM the machine does not have),
  so `kui_c.odin`, `layout.odin`, `generated.odin`, `types.odin` and the
  generator's policy were written as the generator writes them, and
  CI's `odin.nu gen --check` is what says whether they match.
- **Lua's `path` strips the row.** Lua reads every key of a table as a
  schema row before an element reads its own, so `path { rotate = }`
  turned the node as well as the mask and the `path` scene's digest
  moved; the arm now drops the node transform the generic walk read.
  Node's encoder keeps an element's own names off the rows already, and
  C and Odin take a path's turn as an argument.
- **The conformance digest hashes twenty words a clip** in all five
  adapters, and the `transform` scene is the first whose digest moves
  if a turn, a scale or an inner clip does.
- **Measured** with `bench-check.nu` against `v0.1.0-alpha.44`, fastest
  samples, base then head, on the building Mac. The guard passed (no
  guarded row 10% slower), but the frames that turn nothing did not
  come out free, against *Measurements to take*'s expectation:

  | row | alpha.44 | this build | change |
  | --- | --- | --- | --- |
  | `frame_10k_rects` | 762.6 µs | 803 µs | +5.3% |
  | `frame_1k_typical` | 124.9 µs | 130 µs | +4.1% |
  | `frame_10k_rects_all_transitioning` | 1.516 ms | 1.621 ms | +6.9% |
  | `frame_1k_curves` | 241.8 µs | 246.7 µs | +2.0% |

  The likely costs, not yet bisected: `Clip` grew from 32 to 80 bytes
  and `Paint` carries it by value through `emit_node` and `paint_box`;
  `clip.visible()` is a call where `clip.rect` was a field read; and a
  tenth tween slot is walked for every transitioning node, which is the
  largest of the three. Owed: bisect the three, and if `Paint` is it,
  carry the clip id and look the entry up on the rare turned path.
