---
status: accepted
date: 2026-10-05
---

# A mask turns about its centre: `rotate` on a `path`, the turn in the quad and not in the raster

> **Accepted and built 2026-10-05**, the day it was proposed, after the
> measurement under *Measured before building* said the win was real
> (20 µs a turning arc a frame, most of it a texture and a draw of its
> own). *Amendment* at the end records what the building changed and
> what the coverage tests measured: `rotate` does not tween, the hit
> outline is turned at emission, C spells "no turn" as 0 and NULL, and a
> turned seam composites to 0.94 where an upright one is whole. Asked by the arc spinner in
> `examples/rust/apps/loaders.rs`, the day ADR 0040 was built: an open
> stroked arc that turns once a second. Its shape never changes — only
> where it points — and yet every frame is new ops, a new hash, a raster
> and a texture of its own, because the only way to turn a `path` today
> is to declare a different one. ADR 0040's decision 8 priced exactly
> this ("the honest cost of a shape that is different each frame") and
> the price is small for one spinner; the question put was whether a
> turned shape has to *be* a different shape. This document says it does
> not: the mask is rasterized once, upright, and the quad that draws it
> carries the turn.

## Context

- **What a turning path costs today.** `path_with_key` hashes the ops as
  stored; a turned arc is different ops, so the key's hash changes every
  frame, the key is animating after its second change
  (`path::ANIMATING_WINDOW`), and from then on each frame rasterizes the
  mask, mints an image handle, uploads a texture and frees the last
  frame's. ADR 0040 measured the raster at about half a nanosecond a
  texel (13.7 µs for a 26k-texel wedge) and a thousand animating
  hexagons at 837 µs a frame against 294 µs cached — in the core. In a
  window it is about 20 µs an arc a frame, most of it the texture and
  the draw of its own (see *Measured before building*): nothing for one
  spinner, half a 120 Hz frame for two hundred, and a spinner in each
  row of a list is the case this is for.
- **Caching the steps does not work as the rules stand.** Stepping the
  angle — sixty positions a turn — makes sixty shapes that recur, but an
  animating key's masks are texture-backed, and `PathTextures::sweep`
  drops a texture the frame after it is last drawn. The sixty never
  meet. Keeping them would be a budgeted cache of its own (see
  *Considered options*).
- **A quad cannot turn.** `Quad` is a `rect`, and every backend draws it
  axis-aligned; the clip is a rect with radii in framebuffer space. No
  node has a transform: `slide`, `enter` and `exit` move and fade, and
  ADR 0025 gave layout a scale, not a rotation. The one tilted thing in
  the display list is `QuadKind::Segment`, which carries its two
  endpoints in `uv` and is shaded as a capsule inside an axis-aligned
  box — tilted by its *shader*, not its geometry.
- **Two slots of a mask quad are idle.** A `GlyphMask` or `Texture`
  quad a path emits has `radius` square, `border_w` zero (the `Texture`
  kind reads it as the sampling flag) and `blur` zero. `blur` means
  something only to `QuadKind::Shadow`.
- **The path's box is the outline's.** `PathStore::push` boxes the
  flattened outline two logical px out and stores the ops relative to
  it; the mask is that box at physical scale, placed on whole pixels
  with the fractional offset baked in at a quarter-pixel bin. A turned
  outline has a different box at every angle.
- **`path` is unreleased.** The element, `kui_path`'s signature, the
  Node op and the Lua rows are all under *Unreleased* in the changelog.
  What this adds to them is a change to something no one has built
  against yet.

## Decisions

1. **`rotate` on a `path`, in turns.** `<path d rotate={0.25}/>`,
   `path { d =, rotate = 0.25 }`, `Path::rotated(turns)`, and a `rotate`
   argument on `kui_path` and `kui_path_d`. A turn is the unit
   `Path::sector` already takes, clockwise with y down, 0 for none.
   `rotate` is the path's and no other element's: a box, a text and an
   image do not turn (see *Considered options*, a transform on any
   node).
2. **About a pivot the path names, the centre of its box by default.**
   `pivot = [x, y]` in the path's own coordinates — the space `d` is
   written in. A spinner's arc turns about the circle's centre, which is
   not the centre of the arc's own bounding box, so the default alone
   would wobble it; the default serves the shapes that are symmetric
   about their box (a ring, a gear, a chevron).
3. **The box is the square the turn sweeps.** With `rotate` or `pivot`
   declared, the node's box is the square centred on the pivot whose
   half side is the farthest outline point's distance from it (plus
   ADR 0040's pad). It is the same box at every angle, so the ops as
   stored, the hash, the mask and its slot are the same at every angle,
   and the quad turns about its own centre with nothing else to say
   where. A path that declares neither keeps the tight box it has: the
   square is paid by the paths that turn. The cost is texels — a thin
   arc's mask is the whole circle's square — and it is paid once.
4. **The turn rides in the quad; the raster is upright.** The mask quad
   carries the angle in radians in `blur`, the slot a mask never used,
   and the backend turns the quad's four corners about the rect's centre
   in the vertex stage. The mask is sampled linearly across the turned
   quad. A quad with a zero angle is drawn exactly as today, on whole
   pixels, texel for texel. The clip is unchanged: it is a rect in
   framebuffer space tested per fragment, and a turned quad is clipped
   by it as an upright one is.
5. **`rotate` is not a change of shape.** It is not in the ops and not
   in the hash, so a path that only turns never becomes animating, stays
   in the atlas, and costs per frame what `frame_1k_paths_cached`
   measures. It is a `transition` lane beside `bg` — a float, eased by
   the node's own transition and its easing — so a view that declares
   `rotate = 0.5` after `rotate = 0` gets a half turn over the
   transition, and a spring overshoots as it does anywhere. A spinner is
   the app's clock in `rotate`, as it is the app's clock in the ops
   today (see *Open questions*, a `spin` the core drives).
6. **Hit by the outline, turned.** `HitShape::Path` gains the angle and
   the pivot; the test turns the pointer back by the angle about the
   pivot and asks `in_path` (or, for a stroke alone, the segment
   distance) of the upright outline. Nothing is re-flattened.
7. **The seam survives.** Two fills that share an edge and turn together
   about one pivot — a pie spun as a whole — still overlap by the bleed,
   since the bleed is in each mask. Their edges are resampled, so the
   sum along the seam is no longer exact; the coverage test that pins
   ADR 0040's decision 5 gains the turned case and the tolerance it
   needs (see *Measurements*).
8. **Ghosts and the digest.** A ghost carries the angle with the hash,
   as it carries the rule and the stroke width. The corpus digest covers
   quads, and the angle is in the quad: the `path` scene gains a turned
   path, the digest stays one number in four bindings.

## Considered options

- **Leave it: a turned path is a different path.** What is built. Right
  for one spinner and for any shape that turns rarely; wrong at a
  spinner per row, where the frame makes and frees a texture each. Kept
  as the answer if the measurements say the quad's turn costs quality a
  loader cannot afford.
- **Keep the stepped masks.** Let texture-backed masks outlive the
  frame under a budget (an LRU by bytes, as the text cache has), so a
  cycle of N shapes rasterizes once each and then hits. No wire change
  and no backend change, and it serves *any* cyclic animation, not only
  a turn — a pulsing shape, a morph on a loop. But a 100 px spinner at
  scale 2 is 160 KiB a step in RGBA, 9.6 MiB for sixty steps, per
  distinct spinner size; the motion is stepped; and the app must
  quantise its own angles for it to work at all. Not taken as the
  answer to turning; worth its own decision for cycles that are not
  turns.
- **A transform on any node** (`rotate`, `scale`, a matrix; CSS's
  `transform`). The general thing, and a different document: every quad
  kind turns, glyphs leave the pixel grid, a clip is no longer a rect in
  framebuffer space, a scroller inside a turned box scrolls along a
  tilted axis, hit-testing inverts a matrix per ancestor, and the access
  tree's rects become bounding boxes. Not built here, and this does not
  rule it out: a path's own `rotate` would compose under it. *Built as
  [ADR 0043](0043-a-node-turns-about-its-pivot.md) on 2026-10-08, with
  the path's turn composing under the node's as said.*
- **A stock arc fragment** (an SDF sector, angles in its params, as
  `polygon` is a stock fragment). A turn is then two floats a frame and
  no raster, exact at any angle. ADR 0040 declined a `sector` element
  for covering the pie and nothing else, and it costs a pipeline switch
  per run where a mask quad costs none. Not taken.
- **A new quad kind for a turned mask.** Explicit — a renderer that
  does not know the kind draws nothing rather than drawing it upright.
  But it is the same shader branch with one more input, twice (atlas and
  texture), and ADR 0040 was written so the wire gains no kind. Not
  taken; see the ABI question below for what it would have bought.
- **Rasterize turned, key on the angle's bin.** The glyph cache's
  answer for subpixel position: N angle bins, a mask per bin in the
  atlas. Exact edges per bin, no backend change — and N masks per shape,
  every one in the page the text lives in, with the thrash rule one
  spinner away. Rejected.

## Consequences

- **A spinner is one mask.** Rasterized once per scale and bin, drawn
  from the atlas for good, turned by four vertices. Two hundred of one
  size share one slot.
- **Every backend's vertex stage changes.** `kui-wgpu` turns the corners
  for the two mask kinds; a C host that draws the list has one more
  field to read for the kinds it already draws, and one that does not
  read it draws a turned path upright — a wrong picture and not a
  crash, which is what the ABI question is about.
- **A turned mask is resampled.** Upright, a mask is copied texel for
  texel. Turned, its one-pixel ramp is filtered again: edges soften by a
  fraction of a pixel and a stroke under two physical px loses contrast
  at odd angles. That is what every browser's cached, composited layer
  does when it is rotated, and it is the price this document proposes to
  pay; the measurement says how much it is.
- **The atlas's sampler matters.** A turned quad samples between texels
  at its edge. Slots are a texel apart and a path's mask is padded two
  logical px inside itself, so the bleed is into the mask's own empty
  margin — to be confirmed against the atlas's filter in the build, not
  assumed.
- **The box grows for the paths that turn.** A layout or a clip that
  was tight around an arc is now the circle's square around it. It is a
  float and takes no room, so what moves is its hit box (still tested by
  shape) and what a parent's clip cuts.

## Open questions

- **The ABI.** No struct changes layout; `kui_path` and `kui_path_d`
  gain an argument, which is a signature change — free today, since the
  functions are unreleased, and an ABI bump the day after they ship.
  Separately, `blur` gains a meaning for two existing kinds. ADR 0006
  bumps for layout and signatures and says nothing of a field's meaning;
  a host built against the old header would draw turned paths upright
  and never know. Proposed: land this before `path` is released, so
  there is no old header with `path` in it, and say in the header that
  `blur` is the angle for the mask kinds.
- **A `spin` the core drives.** `rotate` from the app's clock rebuilds
  the view every frame to change one float. A `spin = turns per second`
  row, the angle computed from the frame clock as a fragment's `animate`
  reads `time`, would let a spinner idle the app and still turn — and
  would be the first node whose picture changes with no view. It also
  has to answer reduced motion itself. Left out of the decisions; worth
  deciding with this rather than after.
- **Degrees or turns.** SVG's `rotate()` and its arc's `rotation` are
  degrees; `Path::sector` is turns. Proposed as turns, for the one unit
  inside one element's builder; a `d` string's own arc rotation stays
  degrees because it is SVG's.
- **Does `polygon` want it.** Its SDF is evaluated per pixel from eight
  points; turning the points is free and exact already, in the app. No.

## Measurements to take

| bench or test | what | held to |
|---|---|---|
| `frame_1k_paths_rotating` | 1k hexagons, each turning every frame by `rotate` | within noise of `frame_1k_paths_cached` (294 µs on the container ADR 0040 used): the hash, the slot and the quad, and no raster |
| `frame_1k_paths_animating` | unchanged | unchanged (837 µs): what `rotate` is measured against |
| `kui-wgpu/tests/path_coverage.rs`, turned | a wedge turned by the quad against the same wedge rasterized turned, at 7°, 45° and 90° | the largest coverage difference at any pixel, recorded; proposed bound a quarter of full coverage on an edge pixel and none inside |
| the same, a 1.5 px stroke | a thin arc turned by the quad against one rasterized turned | recorded: the number that says whether a hairline spinner should rasterize instead |
| the same, two wedges sharing an edge | the seam after a turn | composite coverage along the edge at least 0.97 |

## Measured before building (2026-10-05)

What a turning arc costs today, against the same arcs standing still —
which is what `rotate` would cost, give or take four vertices: the same
masks, from the atlas, in the one instanced draw. A scratch example drew
N arcs (a 40 px box, radius 14, a 3 px stroke), asked for a frame every
frame, and either turned each arc by its ops or left them alone; a
release build in a real window on an Apple-silicon Mac at 120 Hz, the
main thread sampled for four seconds with `sample` (1 ms), the time
under `Shell::redraw` divided by the 480 frames and the wait for the
drawable (`Surface::get_current_texture`) taken out. Milliseconds a
frame:

| arcs | still | turning | core, of the turning | GPU side, of the turning |
|---|---|---|---|---|
| 1 | 0.12 | 0.13 | 0.006 | 0.12 |
| 50 | 0.11 | 1.37 | 0.37 (0.27 the raster) | 1.00 (0.47 making, filling and dropping textures) |
| 200 | 0.07 | 4.02 | 0.89 (0.58 the raster) | 3.13 (1.55 making, filling and dropping textures) |

- **A turning arc is about 20 µs a frame; a still one is nothing that
  can be measured.** The still column is the frame's floor, 0.1 ms
  whatever N is, and its differences are the sampler's noise.
- **The raster is the small part.** Three microseconds an arc. Eight are
  the texture — `create_texture`, `write_texture` and the drop of last
  frame's — and the rest is what a texture of its own drags behind it: a
  bind group and a draw apiece, where 200 atlas masks are one draw. The
  core-only benches in ADR 0040 could not see this, and the half
  microsecond a path they suggest is a fortieth of what a window pays.
- **Where it starts to matter.** One spinner: never. Fifty: 1.4 ms, a
  sixth of a 120 Hz frame. Two hundred: 4 ms, half of one, a quarter of
  a 60 Hz one. A list with a spinner a row is the case, and it is real.
- **What this does not say.** One machine, one size of arc, a sampler
  rather than a timer, and "still" standing in for "turned by the quad",
  whose resampling cost on the GPU is not in it. It says the win is
  there to be had; the coverage tests below say what it costs in edges.

One thing the first attempt at this found instead: the process used a
whole core at any N, zero arcs included — the run loop goes round
`request_redraw` and `CFRunLoopWakeUp` without blocking while a view
asks for frames — so CPU time a frame measured the loop and not the
arcs. Not this document's; noted because it hid the answer. It was
backlog F103's visible half, and it was built the same day.

## Amendment: what the building changed (2026-10-05)

Built the same day, in four bindings, with the corpus scene, the
loaders' arc, the bench and the coverage tests. Where the draft and the
code differ, the code is right and this says why.

- **The open questions, answered.** The ABI: built before `path` ships,
  so `kui_path` and `kui_path_d` gain `rotate` and `pivot` with no old
  header to break, `KUI_ABI_VERSION` stays 23, and the header says what
  `blur` is on a mask quad. `spin`: not built — it is the first node
  whose picture changes with no view, and a spin is one line of an
  app's own clock in `rotate`. F103 (the run loop spinning while
  anything animated) was the larger cost of a spinner, and was fixed the
  same day. The unit: turns.
- **`rotate` does not tween.** Decision 5 made it a transition lane. A
  lane is a slot on every transitioning node — nine today, each held
  across frames, on the path `all_transitioning` guards — and a tenth
  paid by every box that eases a colour, for the paths that turn, is the
  wrong trade without a view that asks for an eased turn. `rotate` is
  drawn as declared; a spinner is the app's clock, as it was.
- **The mask is centred on a whole pixel.** Decision 4 left the quad
  where the box was. A turn about the quad's centre is only the turn
  about the pivot if the two are the same point, and a box on a
  quarter-pixel bin with a pixel of slack is up to a pixel off it — a
  wobble. A turning path's mask is the square on an even number of
  pixels, the pivot at its centre, the centre rounded to a whole
  physical pixel: no bins (one mask a scale, not sixteen), exact about
  the pivot, texel for texel at no angle, and placed to the nearest
  pixel rather than the nearest quarter.
- **The hit outline is turned at emission.** Decision 6 put the angle on
  `HitShape::Path` and undid it per test. Emission flattens the outline
  every frame already; it turns the points there, and the hit shapes are
  unchanged — a stroke's segments included, which had no angle to gain.
- **C spells no turn as 0 and NULL.** The other bindings have a prop
  that is there or not; C has two arguments. `rotate` 0 with a NULL
  `pivot` is the tight box, so a C path that turns through 0 names its
  pivot, or its box changes for that frame. The header says so.
- **The wire.** Node's `path` op gains three floats and two flag bits,
  inside v20, which is unreleased. The corpus's `path` scene has a
  seventh path, a bar turned an eighth about a pivot it names.

### Measured (2026-10-05, an M3 Pro)

| bench or test | result | held to | |
|---|---|---|---|
| `frame_1k_paths_rotating` | 225 µs | `frame_1k_paths_cached` | cached is 215 µs on the same run: the turn costs the trig for the angle and nothing of the raster |
| `frame_1k_paths_animating` | 456 µs | what `rotate` is measured against | twice the rotating frame in the core alone; the window's half is under *Measured before building* |
| a wedge turned by its quad against one rasterized turned | 0.28 at 7°, 0.30 at 45°, 0.20 at 90° on the worst edge pixel; 0.003 at most where the wedge is flat | the draft proposed 0.25 | 90° resamples nothing — texel lands on texel — so its 0.20 is the floor of the comparison (two rasters a quarter-pixel bin apart), and the turn adds about a tenth of a pixel's coverage on an edge. The test holds 0.35. |
| a 1.5 px arc, the same | 0.31 at 7°, 0.28 at 45°, 0.08 at 90° | recorded | a hairline loses up to a quarter of its coverage at an odd angle: visible as a slightly lighter line, not a broken one. The test holds 0.5. |
| two wedges sharing an edge, turned together | 0.937 at the worst pixel | the draft proposed 0.97 | **Not met.** The bleed is resampled with the edge, so six hundredths of what is under a turned seam shows, where an upright seam shows none. A pie that spins as a whole has a faint seam while it is at an odd angle; a chart that cannot have one turns its ops, and pays the raster. The test holds 0.9. |

The loaders' arc in a window: one atlas mask from its first frame, its
quad's `blur` the phase, checked in its headless drive and looked at
turning under the real shader.

## Action items — all done 2026-10-05, but the lane and `spin`

- [x] Decide the open questions: the ABI line, `spin`, the unit.
- [x] `kui-core`: `Path::rotated` and `pivot`, the swept square in
      `PathStore::push`, the angle on the run and in the mask quad, the
      hit outline turned, ghosts. Not the transition lane (see the
      amendment).
- [x] `kui-wgpu`: the corners turned for the two mask kinds; linear
      sampling confirmed at a slot's edge; the turned coverage tests.
- [x] `kui-ffi`, `kui-node`, `kui-lua`, `packages/kui`: the row in four
      bindings, the header's line on `blur`, the generated files.
- [x] Corpus: a turned path in the `path` scene.
- [x] `examples/rust/apps/loaders.rs`: the arc by `rotate`, its drive
      checking that it stays one atlas mask while it turns.
- [x] The benches and tests above, run, and the amendment that records
      them.
