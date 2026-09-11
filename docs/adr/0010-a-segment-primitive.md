---
status: accepted
date: 2026-09-06
---

# A segment primitive: lines, polylines and curves as capsule quads

> **Decisions 7 and 8 are superseded by
> [ADR 0026](0026-hit-testing-by-shape.md) (2026-09-11):** a line that
> declares input is hit by its stroke and derives an access role as a box
> would; the `line-ignores-input` warning is gone. The rest stands.

`QuadKind` is six rounded-rect variants, and the only thing a view can draw
is a box. That is enough for a control and not for a diagram: the mind map
in the second field report (backlog F12) draws every connector as **three
thin boxes** — a stub out of the parent, a vertical run, a stub into the
child — and says plainly that this is the ceiling. No diagonal, no curve,
three nodes per link, and where two children share a parent's stub the stub
is drawn twice in two branch colours, so the overlap is whichever won. ADR
0005 widened the paint vocabulary by group opacity and a shadow, declined
gradients with reasons, and never considered a stroke.

We are shipping **one new quad kind, `QuadKind::Segment`** — a round-capped
line between two endpoints, drawn by an SDF capsule in the same über-pipeline
and the same draw call as everything else — and **one new element, `line`**,
which emits one segment per straight piece of what it was given: a segment
from `from` to `to`, a polyline through `points`, or a curve through the same
`points` when `curve` is set. Polylines and curves are flattened **in the
core**, so the four bindings and the app never see the flattening. A line is
**never in layout**: it is a float positioned by its own endpoints, in its
parent's coordinate space, and it takes **no pointer input**. Everything the
element is not — arbitrary paths, fills, dashes, arrowheads, a shape-aware hit
test — is declined or deferred below, each with the reason.

## Context

- **`Quad` must not grow.** C15 (`docs/backlog/closed-2026-09.md`) is the
  record of a frame getting 2.5× slower one small field at a time, and the
  note it ends on is that `Quad` went 92 → 124 bytes over the same period,
  20,000 copies a frame, without a profile ever noticing. A segment needs
  two endpoints (four floats) and a width. `Quad` has a 16-byte slot only
  glyphs and images use — `uv: [u32; 4]`, atlas texels — and a `border_w`
  a segment has no other use for. The new kind fits in what is already
  there, and the struct stays 124 bytes.
- **`Shadow` is the pattern.** ADR 0005 added a kind by giving the existing
  rounded-rect SDF one more branch; the shader grew about fifteen lines and
  the backend nothing. A capsule SDF is shorter than that. The one thing a
  segment needs that a shadow did not is the raw endpoints in the fragment
  shader, which the `uv` varying does not carry (the vertex shader divides
  it by the atlas size) — so one more varying, flat across the quad.
- **The renderer boundary is a list, not a tree.** ADR 0005 rejected
  compositing because it would make `DisplayList` a tree; a path primitive
  (SVG's `d`, tessellated) would make it a triangle soup beside the quad
  list, in every backend including the C host that has only `KuiQuad` to
  walk. One quad per segment keeps the boundary what it is.
- **A connector's coordinates belong to the things it connects.** The mind
  map is a canvas of floats: every card is `float` with an offset in the
  canvas's space, and every link is drawn between two of those offsets.
  Giving a line a place in a row or column would move its box and not its
  meaning. The one coordinate space a line can honestly draw in is the one
  its siblings are floated in.
- **Hit-testing is rectangular everywhere.** `HitRegion` is a rect and a
  clip rect, and `hit_at` is `rect.contains`. The README's Status section
  already names the consequence for rounded clippers ("a click in the
  corner of a rounded scroll container still reaches the row under it").
  A diagonal segment's rect is its whole bounding box, which for a long
  diagonal is mostly not the line; a shape-aware hit test is the same
  question for both, and it is not a smaller question for lines.
- **Every element costs four bindings.** An `ELEMENTS` row is hand-lowered
  in Rust, Lua, C and Node, the corpus's coverage test refuses a row no
  scene exercises, and the C and Node adapters mirror the report's quad
  digest byte for byte. So the design below is judged partly by how little
  each binding has to invent: a list of points, a width, a colour and a
  flag, and nothing about how a curve becomes segments.

## Decision

1. **`QuadKind::Segment`.** A round-capped stroke from one endpoint to
   another, in physical pixels. The endpoints ride in `uv` as
   `[x0, y0, x1, y1]`, each an `f32` stored through `to_bits` in the `u32`
   slot the atlas texels use; `Quad::segment_ends` reads them back. The
   stroke width is `border_w`, the colour is `color`, and `rect` is the
   bounding box — the endpoints inflated by half the width and two logical
   pixels more, so the antialiasing ramp is never cut by the quad's own
   edge, at scale 1 included. `radius`, `border_color` and `blur` are zero and ignored. `Quad`
   stays 124 bytes and `KuiQuad`'s layout does not change; C gains
   `KUI_QUAD_SEGMENT = 6` and the header says what `uv` holds for it. A
   backend that predates the kind draws a segment as a solid box of its
   bounding rect, which is wrong and harmless; a C host cannot reach that
   state without also calling `kui_line`, which it does not have.
2. **The shader draws a capsule.** One more branch in `shade`, before the
   rounded-rect path: the signed distance from the fragment to the segment
   minus half the width, ramped over the same `AA` as every other edge, and
   multiplied by the same clip coverage. A tenth varying carries the
   endpoints untouched. `instance_of` decodes the bits for this kind only;
   every other kind's `uv` is converted as it always was. Still one
   pipeline, one instance buffer, one draw call. The CPU transcription in
   `tests/wgsl` gains `sd_segment`, and `segment_coverage.rs` evaluates the
   branch over a quad the core actually emitted, the way `shadow_coverage`
   does: the half the core computes and the half the shader computes are
   checked against each other rather than each against itself.
3. **The `line` element.** `<line from={[x0,y0]} to={[x1,y1]} width color/>`
   in JSX; `line { from = {x, y}, to = {x, y}, width =, color = }` in Lua;
   `kui_line(ctx, x0, y0, x1, y1, width, color, spec)` in C;
   `ui.line(from, to, Stroke::new(width, color), spec)` in Rust. `width`
   is the stroke width in logical pixels, default 1 — a line has no box to
   size, so the sizing name is free, and it is the name every drawing API
   uses (`lineWidth`, `stroke-width`). `color` is the schema's own `color`
   row — the text colour row — read off the parsed style, so it defaults
   to the foreground and every binding parses it mechanically. Inside the
   core the stroke colour lives in the node's `bg` slot, which is what
   makes it **tween**: a `transition` eases it through the slot backgrounds
   already ease, and `enter` / `exit` can start or end it there. The node
   emits no background box; `bg` on a line *is* the stroke.
4. **Polylines and curves are the same element, flattened in the core.**
   `points` is a list of at least two; `<line points curve/>` draws a
   smooth curve *through* them (a Catmull-Rom spline, each span flattened
   into a fixed number of pieces by its chord length, capped at 32) and
   without `curve` the straight polyline. *This shipped uniformly
   parameterized and is centripetal now — see the amendment below.* Two points with `curve` is a
   straight segment. The flattening is one function in the core, the same
   in every binding, so a curve's segment count is reproducible and the
   corpus can pin it. Consecutive segments overlap at their round caps,
   which is the join: exact for an opaque stroke, and a translucent one
   double-blends at each join the way a faded subtree shows its seams (ADR
   0005) — said in the doc comment, not discovered.
5. **A line is a float, always, positioned by its endpoints.** The core
   sizes the node to its padded bounding box, sets `float` to attach that
   box's top-left to the parent's at the box's own offset, and stores the
   points relative to it. Endpoints are therefore in the **parent's box
   space** — the space a sibling floated with an offset is placed in — and
   a line consumes no room in a row or column. The one piece of a declared
   `float` a line keeps is its anchor: `float="viewport"` reads the
   endpoints in viewport space instead. `width` / `height` / `min` / `max`
   as sizing, `pad`, `gap`, `dir` and alignment are meaningless on it and
   ignored. Because it is a float it paints in the float pass, on top of
   its parent's in-flow content and in tree order among the other floats:
   a connector meant to sit under two cards is declared before them.
   `slide`, `enter` and `exit` offsets move it as they move any float.
6. **`on_layout` reports the bounding box.** The rect is the node's — the
   endpoints inflated by half the width plus two — with `parent` beside
   it, exactly like any other node. Nothing inside a line has a rect of
   its own.
7. **A line takes no input and has no access row.** It emits no hit
   region, so it is never hovered, pressed, dragged or focused, and pointer
   events fall through to whatever is under its bounding box. A line
   declaring `onClick`, `onDrag`, `onKey`, `onHover`, `hoverable` or
   `focusable` raises `line-ignores-input`, once per key, through the
   ordinary diagnostics path. It derives no role, so it is elided from the
   access tree like plain structure; a connector that means something is
   named on the nodes it joins, or a `role` and `label` declared on it are
   honoured like any node's.
8. **This ADR does not settle rounded hit-testing**, and does not build a
   shape-aware one for lines either. Both are the same change — a
   `HitRegion` that carries a shape (a rounded rect's radii, a segment's
   endpoints and width) and a `hit_at` that evaluates the SDF the CPU test
   transcription already has instead of `rect.contains` — and it should be
   built once, for both, when a view wants to click a connector or is bitten
   by a rounded corner. Until then a line is inert, which is the honest
   behaviour rather than a rectangular approximation of a diagonal.
9. **Not in this ADR.** Arbitrary paths, fills, dashes and arrowheads. Each
   has a reason in the Considered options below and a shape of what would
   change it.
10. **What it costs, measured.** `frame_10k_segments` joins
    `benches/frame.rs`: a 100×100 grid of one-segment lines, so it has the
    same quad count as `frame_10k_rects` and the difference between the two
    is what a segment costs over a box. `frame_1k_curves` is a thousand
    eight-point curves flattened per frame, which is the flattening's own
    bill. The numbers are under **Measurement** at the end.

## Considered options

- **A rotation on `Quad`** (a thin box turned by an angle). Rejected: four
  more bytes on the hot struct for every quad (C15's exact mistake), a
  rotate in the vertex shader for every quad, square ends where a join
  needs round ones, and clip and antialiasing math in a rotated space. The
  capsule needs none of it and has caps for free.
- **`uv` becomes `[f32; 4]` for every kind.** The clean version of the
  bit-cast. Rejected: it changes the type of a field every backend reads
  for every glyph and image quad, and `KuiQuad.uv` is `uint32_t[4]` in a
  header shipped in alpha.6 — a C renderer reading it as before would
  misplace every glyph. The bit-cast changes nothing for the six kinds that
  exist and is read by exactly one function per backend.
- **A path primitive** (`d`-style paths tessellated to triangles, lyon or
  hand-rolled). Rejected for v0: the display list stops being one flat
  instanced quad list — a second vertex stream beside it in every backend,
  and a C host that has only `KuiQuad` to walk gets nothing — and
  tessellation is a per-frame CPU cost proportional to the path, paid in
  the frame model that rebuilds everything each frame. A diagram is
  strokes, a stroke is segments, and a segment is one quad.
- **One quad per polyline** (the SDF over N points read from a storage
  buffer). Rejected as the starting point, and the named follow-up: it needs
  a per-frame point buffer beside the instance buffer in every backend, and
  the C ABI has no such buffer. What it buys is an exact join — no
  double-blend where round caps overlap — which matters only for
  translucent strokes. If a view needs those, this is the change.
- **A `stroke` prop on a box** (draw the box's diagonal). Rejected: a box
  has no endpoints to speak of, and the mind map's three-box hack is what
  this looks like when written out.
- **Flatten curves in the bindings** (each binding turns `points` +
  `curve` into segments before the core sees them). Rejected: four
  implementations of one function that all have to agree to the bit for
  the corpus digest to match, and an app-visible segment count that differs
  by binding. In the core it is one function and the digest is one number.
- **A line in flow** (an ordinary child with a `Fit` size). Rejected: its
  endpoints are in the coordinate space of what it connects, which is the
  parent's box, not a slot in the parent's run; and a `Fit` size cannot be
  known before the endpoints are, which are only known relative to that
  box. Float-only is the one placement that means what the view said.
- **Dashes now**, as a `dash` array. Deferred: a dash is a pattern along
  arc length and has to keep its phase across the joins of a polyline
  (`params.w` is free to carry a phase per quad, and the core knows the
  cumulative length), but an app that declares `dash` today gets a pattern
  that restarts at every join of a curve, which reads as a bug. It is a
  small change once a view wants it; it is not free to get right blind.
- **Arrowheads now.** Deferred: a filled arrowhead is a fill, which the
  vocabulary does not have, and an open one is two short segments the app
  can already draw from the endpoint and the direction it has. A `cap`
  vocabulary (`round | butt | arrow`) is where it would go.
- **Fills** (a closed polygon). Rejected: a fill is a path primitive by
  another name — see above.

## Consequences

- **`Quad` is still 124 bytes** and `KuiQuad`'s layout, stride and ABI
  version are unchanged. `QuadKind` gains a seventh variant, `KuiQuad.uv`'s
  comment gains a sentence, and the JS decoder reads `uv` back as endpoints
  for this kind. The conformance report's `kinds` line grows a seventh
  column, `Expect` a `segments` count, and the C and Node adapters mirror
  both; the quad digest, which skips `uv` because atlas coordinates follow
  glyph insertion order, **includes it for segments**, because there it is
  geometry — two diagonals of one bounding box would otherwise digest the
  same.
- **One scene, `lines`**, claims the `line` row and is what the coverage
  test demands: a diagonal, an orthogonal three-point polyline, a curve
  through four points, and a faded one, so all four bindings reproduce the
  flattening's segment count and the endpoints' bits.
- **Ghosts carry their points.** A departing `line` is replayed like any
  node: `GhostContent::Line` indexes a point list the depart store copies
  at departure and drops when its last ghost goes, and the stroke colour
  eases through the ghost's `bg` slot like a box's background. The line
  store keeps the previous frame's points by the same gated buffer swap the
  text list uses, so a frame with no `exit` pays nothing for it.
- **The stroke's width does not tween.** Colour does (through `bg`); width
  is stored with the points and snaps. A view that animates a connector's
  thickness will notice, and the fix is a tween slot, not a redesign.
- **A translucent polyline shows its joints**, and a translucent stroke
  crossing itself double-blends. Both are the caps overlapping, and both are
  in the doc comment.
- **`width` on a line is a stroke width and `bg` is its colour**, which are
  the same names meaning something else on a box. The TypeScript type for
  `line` says so (`width?: number`, no sizing union), Lua and C take the
  number, and the element table row says it in one line.
- **No input, no access row, no hit region.** The `line-ignores-input`
  warning is the whole surface of the limitation; decision 8 is where it
  would be lifted.
- **What this does not do:** paths, fills, dashes, arrowheads, a shape-aware
  hit test, and a tweening width — each above with its reason and the
  change that would add it.
- **Measured** on `crates/kui-core/benches/frame.rs`, on the same machine,
  against the commit before, in the section below.

## Measurement

Fastest of 100 samples, one machine, one run of each build; the previous
commit was built and run from its own worktree the way C15's bisect did it.
Not interleaved rounds, so the noise floor is not measured here — the
shadows row, which this change does not touch, moved 6% between the two
runs, and that is the number every row below has to beat to mean anything.

| frame | before | after | |
|---|---|---|---|
| `frame_10k_rects` (10k boxes) | 789.6 µs | 792.7 µs | +0.4% |
| `frame_1k_typical` | 114.7 µs | 114.4 µs | −0.3% |
| `frame_10k_rects_with_text_and_hits` | 1.189 ms | 1.193 ms | +0.3% |
| `frame_10k_rects_with_shadows_and_opacity` | 861.1 µs | 810.2 µs | −5.9% |
| `frame_10k_segments` (10k one-segment lines, 10k quads) | — | 852.9 µs | +7.6% over `frame_10k_rects` |
| `frame_1k_curves` (1k eight-knot curves, 35k quads) | — | 234.9 µs | |

Read it as two numbers. **A frame that draws no line pays nothing it can
measure**: `Quad` did not grow, the line store's frame start is two
`clear`s on empty vectors, and the one new branch in `emit_node` reads a
byte that is loaded a few lines later anyway. **A segment costs about 8%
more than a box**, which is the float placement (a segment is never in
flow, so every one goes through `place_float`), the point list, and the
bounding-box arithmetic per piece. The curve bench says the flattening is
cheap: 35,000 segments in 235 µs is under 7 ns a segment, flattening
included — a curve's cost is its segment count, not its being a curve.

Not measured here: the GPU side. What is checked is that both preprocessed
shader variants still validate (`shader_variants_validate`) and that the
capsule the shader cuts is the capsule the core described, by the CPU
transcription in `segment_coverage.rs`.

## Amendment: the spline is centripetal, not uniform

Decision 4 said "a Catmull-Rom spline" and meant the textbook one, which is
uniformly parameterized: every span gets one unit of curve parameter,
however long or short its chord is. The first view drawn on top of it —
`examples/rust/connectors.rs`, a mind map whose links are elbows — showed
what that costs, and the flattening is now **centripetal** (Lee's α = ½:
each span's parameter is `√chord`).

### What uniform gets wrong

A uniform parameter is a claim that the knots are evenly spaced, and when
they are not, the curve has to move fast through the tight ones to spend
the parameter they were given. Two symptoms, both measured rather than
recalled:

- **It bows out of the wrong side of a corner.** The connectors example's
  first link runs 60px flat and then turns 140px upward. The uniform
  spline leaves the card **10.3px below** the edge it starts on before it
  turns — three links out of one card, each hooking the wrong way and
  crossing each other just past its edge. Centripetal halves that to 5.3px.
- **It ties loops.** Over 20,000 random four-knot sets, the middle span of
  the uniform spline crossed itself in **82**; the centripetal one in
  **none**, which is the theorem (Yuksel, Schaefer and Keyser) and not
  luck. `a_tight_knot_between_two_long_ones_neither_loops_nor_cusps` pins
  one of them: chords of 671, 36 and 328, where uniform ties a visible
  knot at `CURVE_STEP`'s own sampling and turns 95° in one piece.

### What it does not fix, and what does

Centripetal **reduces** the lean into a corner; it does not remove it,
because a spline through its knots must arrive at each one. A shape whose
middle points should only *pull* is a Bézier, and that is what the
connectors example now samples for its links — the four points it used to
hand to `curve` are the Bézier's own control points, which were never
knots. The two are both in the tree: `curve` is the primitive for a run of
points a stroke should visit, and a caller who wants handles builds them.

### What it cost

Three things changed inside `flatten_curve`, none of them visible through
any binding:

- **The evaluation is Barry and Goldman's pyramid**, not a cubic in `t`,
  because the knot times are no longer evenly spaced. Six interpolations
  instead of one polynomial per axis.
- **The end knots are no longer doubled.** A repeated knot is a zero-length
  chord, and centripetal has no parameter for it; each end gets a mirrored
  phantom (`2·p1 − p2`) instead, and a *coincident pair of real knots*
  takes the same branch — so a duplicated point in a caller's list is a
  kink and not a division by zero.
- **Chords are rolled forward** rather than recomputed per neighbour (two
  square roots a span, not six), and a span's five denominators are
  reciprocated once and reused by every piece. Both were needed: without
  them the bench was +35%.

Interleaved A/B, three rounds each on the same machine, `divan` medians:

| Bench | uniform | centripetal | Δ |
| --- | --- | --- | --- |
| `frame_1k_curves` (1k eight-knot curves, 35k quads) | 241.5 µs | 268.6 µs | +11.2% |

Which is 7.7 ns a segment against 6.9, flattening included. Nothing else
moved: the piece count is still `ceil(chord / CURVE_STEP)`, so every scene's
segment count, every `Expect`, and every binding's mirror of it are
unchanged — the corpus's quad digests shifted only in the point values, and
all four adapters shifted together, which is what the shared reference
report is for.
