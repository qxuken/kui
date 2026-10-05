---
status: accepted
date: 2026-10-04
---

# A path is a mask in the atlas: filled and stroked outlines of any shape, rasterized once and drawn as a glyph

> **Accepted (2026-10-04), built the next day (2026-10-05).** Asked by the pie in
> `examples/rust/widgets/polygon.rs`: four `polygon` wedges whose arcs are
> seven chords each and whose shared edges show the panel through them as
> a light hairline. ADR 0010 rejected a path primitive and ADR 0025 built
> an eight-point fill in its place; backlog V8 declined a drawing-ops
> canvas with the condition "a fill eight points cannot make", and a round
> wedge is that fill. This document takes the route the browsers take for
> most SVG — flatten on the CPU, rasterize to a coverage mask, cache it in
> a texture atlas, draw a textured quad — because kui already runs that
> pipeline for every glyph on screen, and the rasterizer it needs is
> already compiled into `kui-core`. What it decides beyond the element is
> where the masks live (the glyph atlas, under its existing policy), what
> happens past its limits (a texture of the mask's own, as a big image
> gets), and what a path that moves every frame does (leaves the atlas,
> as an updated image does). The measurements under *Measurements to
> take* are named and run under *Amendment*, which also records what the
> building changed from this draft: no ABI bump (a new function does not
> bump it), the stroke riding the border slots, a string-form C door, and
> "animating" meaning two changes running.

## Context

- **The pie is the symptom, not the problem.** A `polygon` is one
  `fragment` quad painted by the stock SDF in
  `crates/kui-core/src/fragment.rs`, covering a pixel by `0.5 − distance`
  to the outline. That is right for a shape alone and wrong for two that
  share an edge: each covers the edge's pixels by half, half over half is
  three quarters, and a quarter of what is under them shows along every
  shared edge. The area chart in the same example, a strip of quads, has
  the same seam at every join. Square boxes never do, because the epilogue
  covers them by the pixel's area, which sums to one across a shared
  edge. And eight points is seven chords on an arc: the widest wedge at
  radius 80 sags 1.4 px from the circle. Neither is fixable by an app
  from outside the element, short of overlapping its wedges by hand.
- **"Fill eight points cannot make" is the condition that was set.** ADR
  0010 rejected a `d`-style path tessellated to triangles for three
  reasons: the display list stops being one flat instanced quad list, a C
  host walking `KuiQuad` gets nothing, and tessellation is a per-frame CPU
  cost in a frame model that rebuilds everything. ADR 0025 decision 6
  built the eight-point `polygon` as the fill the vocabulary lacked and
  named "the path primitive this document does not build". Backlog V8
  declined a canvas and set the condition. The condition is met; the
  three reasons still hold, and this document has to answer them rather
  than wave them off.
- **What the browsers do.** Firefox draws no paths on the GPU: Gecko
  records an SVG into a blob, rasterizes it on the CPU into tiles, uploads
  them to a texture atlas, and WebRender composites quads. Chrome's Skia
  flattens a path to segments, rasterizes small and complex paths to an
  8-bit coverage mask cached in an atlas — the glyph path — and
  tessellates or stencils large simple ones with MSAA for the edges.
  Compute-shader rasterization (Vello) is the research direction and ships
  in no browser. One fact matters more than the survey: analytic coverage
  conflates at a shared edge in every one of these, and two adjacent SVG
  paths in a browser show the same hairline the pie shows. Only
  sample-based MSAA does not. The seam is the element's to close, by
  bleeding or by a gap; no rasterizer closes it for free.
- **The pipeline is already here.** `crates/kui-core/src/text.rs`
  rasterizes outline glyphs through swash, whose rasterizer is zeno: a
  general path renderer — move, line, quadratic, cubic, arc, close;
  nonzero and even-odd fills; strokes with joins, caps and dashes — that
  `kui-core` already depends on through `swash::zeno`. The result goes
  into the glyph atlas (`crates/kui-core/src/atlas.rs`), and a `GlyphMask`
  quad draws "atlas alpha times `color`" in the one instanced draw call
  every backend has, with no pipeline switch, which a `fragment` quad
  costs per run. The terminal's box-drawing and Powerline shapes already
  take this exact road: masks drawn from a cell box rather than a font,
  through `get_or_insert_synth`, keyed on what they were drawn from.
- **The atlas has a policy, and it is not per-slot eviction.** A slot
  handed out during a frame is never moved before that frame is
  presented, because emitted quads carry its texel rect. `begin_frame`
  watches how many shelf rows recent frames opened and empties the whole
  page before it would fill; the emptied page is kept for one frame and
  what that frame looks up is copied across rather than rasterized again;
  a page emptied again within two frames is too small for its turnover
  and grows instead (F83); the base never shrinks; a page at
  `MAX_ATLAS_SIZE` (4096) refuses and the next frame starts empty. An
  image that does not fit, or whose pixels the app replaced, leaves the
  page for a texture of its own and stays there (`ImageBacking::Texture`,
  ADR 0025). A path has to say which of these it is, in each case.
- **Hit-testing is by shape already.** ADR 0026 gave every region a
  `HitShape`; a polygon's is its outline in `Interaction::shape_points`,
  tested even-odd, with no limit on the point count — `in_polygon` takes
  a slice. A path's flattened outline is that slice.
- **The curve flattener is in the core, once.** ADR 0010 rejected
  flattening in the bindings so the corpus digest would be one number;
  `crates/kui-core/src/line.rs` flattens a curve by chord length. A
  path's own flattening — for the hit outline, and for the rasterizer's
  tolerance — belongs beside it for the same reason.

## Decisions

1. **A `path` element.** `<path d="…" bg color width fillRule/>` in JSX;
   `path { d = "…", bg =, color =, width =, fill_rule = }` in Lua;
   `kui_path(ctx, label, ops, count, spec, payloads)` in C; `ui.path(&ops,
   spec)` in Rust with a `Path` builder (`move_to`, `line_to`, `quad_to`,
   `cubic_to`, `arc_to`, `close`) and `Path::parse(d)`. Placement is
   `line`'s and `polygon`'s (ADR 0010 decision 5): always a float in its
   parent's box space, `float="viewport"` for viewport space, sized to its
   own bounding box inflated by one logical px, no room taken in a row or
   column; `slide`, `enter` and `exit` move and fade it as a float; a
   declared `clip` holds it as it holds a polygon.
2. **One wire form, and SVG's `d` parsed in the core.** On the wire a
   path is a flat `f32` list, an op code followed by its operands, every
   coordinate absolute and in the parent's box space: `M x y`, `L x y`,
   `Q cx cy x y`, `C c1x c1y c2x c2y x y`, `A rx ry rot large sweep x y`,
   `Z`. The ops live in a per-frame store beside the `LineStore`, and a
   node carries only where. A `d` string — JSX's and Lua's natural form,
   and what every chart library emits — is parsed by **one** function the
   core owns (`Path::parse`, `kui_path_parse`), which expands relative
   commands, `H`, `V`, `S` and `T` into the six ops above, so the four
   bindings cannot disagree on a path to the bit and the digest stays one
   number. Node passes the string through; it parses nothing. A string
   that does not parse raises `path-malformed` once per key, with the
   byte offset, and draws nothing.
3. **Fill in `bg`, stroke in `width` and `color`, as a line's rows are.**
   A path with `bg` is filled; one with `width` and `color` is stroked;
   one with both is two masks and two quads, the stroke over the fill. The
   fill is a background, so it **tweens** through the slot backgrounds
   already ease and `hover_bg` paints as ADR 0026 made it paint on a
   wedge; the stroke's colour tweens as a line's does. `fillRule` is
   `nonzero` (SVG's default; a self-intersecting outline fills its
   overlaps) or `evenodd` (the `polygon`'s rule, kept there unchanged).
   Joins and caps are round, as a `line`'s are; `dash` stays V2's.
4. **Rasterized on the CPU, through zeno, at the node's physical scale.**
   The ops are transformed to physical px and rasterized into an 8-bit
   coverage mask the size of the outline's bounding box plus a pixel on
   each side, with the bounding box's fractional offset baked in at the
   **quarter-pixel bins the glyph key already carries**, so a path that
   slides by a sub-pixel amount rasterizes at most sixteen variants and
   thereafter hits the cache, and one at rest is exact at its position.
   The arc flattening tolerance is the rasterizer's own, not
   `flatten_curve`'s chord length, which serves strokes.
5. **A fill bleeds half a pixel.** The fill mask is the outline filled
   plus the outline stroked one physical px wide, in the same mask by
   `max`. A shape alone gets half a pixel fatter with the same one-pixel
   ramp, which nothing can see; two fills that share an edge overlap by
   exactly the ramp, and the background never shows through. This is the
   decision that fixes the pie, and it is the element's, not the app's:
   a chart that wants visible separators between its wedges offsets its
   own geometry, as it does in a browser. `polygon` is left as it is; the
   amendment that would give it the same bleed is a one-line change to
   its coverage and a separate decision, since it changes ADR 0025's
   "the same one-pixel ramp every box gets".
6. **The masks live in the glyph atlas, as a fourth map.** Keyed on a
   hash of the ops, the scale bits, the bin, and whether it is the fill or
   the stroke with the stroke's width — the same shape as `synth`'s key,
   for the same reason: one cell size shares one slot, another does not.
   A refused path is a `Refusal::Path` entry, so the measuring that
   decides a reset counts paths with the glyphs; a looked-up path the
   emptied page held is **copied across** at a reset, not rasterized
   again, like a glyph. No per-slot eviction is added: a path's slot is
   reclaimed when the page is, which is the policy every other slot has.
   The quad is a `GlyphMask` tinted by the fill or stroke colour, `uv` in
   texels, so a C renderer that predates this document draws a path as it
   draws text, and the wire gains no kind.
7. **Past the page, a texture of its own; past the device, nothing.** A
   mask that would not fit a `MAX_ATLAS_SIZE` page on either side, or
   whose texels exceed a quarter of one (2048² — four of them would empty
   the biggest page every frame), does not enter the atlas: the core
   registers it as an internal image with `ImageBacking::Texture`, and it
   draws as a `Texture` quad through `DisplayList::textures`, the road an
   image too big for a page already takes. A mask past the device's
   texture limit raises `path-too-large` once per key with the size and
   the limit, and draws nothing; tiling is named and not built.
8. **A path that changes every frame leaves the atlas and stays out.** A
   path whose ops hash differed on two consecutive frames is **animating**
   and moves to a texture of its own, re-rasterized and re-uploaded each
   frame through the `rev` an updated image moves, and does not return to
   the page — the rule `ImageBacking::Texture` already states for an
   image whose pixels were replaced ("once here, an image stays here"). It
   never opens atlas rows, so it never trips the thrash rule that would
   grow the page to 4096 and hold 64 MiB for the session. The cost it
   pays is the raster each frame, which is the honest cost of a shape
   that is different each frame; a shape that must move cheaply every
   frame at any size stays a `polygon`, whose SDF costs the quad and
   nothing else. A path still for two frames after animating stays
   texture-backed; the rule is one-way, as the image's is. *(Amended:
   it returns after 120 still frames — see the second review below.)*
9. **Hit by its outline, nonzero or even-odd as it fills.** Emission
   flattens the ops once — the same flattening the rasterizer gets — into
   `Interaction::shape_points`, and `HitShape::Polygon` gains the fill
   rule, so a press inside a wedge is its own and one in its box past the
   arc is the neighbour's, with no point limit. A stroked path with no
   fill hits by its stroke as a `line` does, at least `MIN_STROKE_GRAB`
   wide. With input it derives a role as a box would (a clickable wedge is
   a button, give it a `label`); with none it is elided, as ADR 0026
   decided for its siblings. Ghosts carry the ops through the store as a
   line's points go.
10. **What does not re-rasterize.** Colour, the fill tween, `hover_bg`,
    group opacity, `enter`/`exit`, the clip, hover, and a `slide` that
    stays within a quarter-pixel bin. A scale change — a window moved
    between monitors — is a new key, as it is for a glyph, whose key
    carries its physical size: the old slots go stale and the page turns
    over by its own rule; nothing is cleared. `clear` stays what it is,
    the subpixel toggle's.
11. **The doors.** C: `kui_path` and `kui_path_parse` in ABI 24; `KuiOp`
    codes in the header. Node: `<path>` lowered as `<line>` is, `d` as a
    string or a flat number array. Lua: `path { d = }` or `ops = {…}`.
    `index.d.ts`, `jsx-runtime.d.ts` and `docs/props.md` regenerated from
    the schema row. `polygon` and `line` are unchanged.
12. **The corpus pins it.** A `path` scene: a pie of four arcs with a
    press inside a wedge and one past its arc, a cubic blob, an
    even-odd ring, a stroked open curve, a fill-and-stroke, and a
    malformed `d` raising its warning — in four bindings, the digest one
    number. The digest covers quads and warnings; the mask's texels are
    pinned by a `kui-wgpu` coverage test beside `segment_coverage.rs`,
    which is also where the half-pixel bleed is proved: two fills sharing
    an edge composite to full coverage along it.

## Considered options

- **Tessellation, or stencil-then-cover, with MSAA** (Chrome's GPU path,
  Flutter's). Rejected, for ADR 0010's reasons, which have not moved: a
  triangle stream beside the quad list in every backend; a C host with
  `KuiQuad` to walk gets nothing; MSAA on the frame's own target
  quadruples its bandwidth for every box and glyph to serve the one
  shape, or an offscreen MSAA pass per path. It is the one method with
  no shared-edge seam, and decision 5 buys that at half a pixel instead.
- **An N-point SDF per quad, points in a storage buffer** (ADR 0010's
  named follow-up). Keeps animation free and the quad list flat, but it
  needs a per-frame point buffer in every backend and in the C ABI, the
  per-pixel loop is linear in the edge count, and a curve is still
  chords, so the facets come back unless the count is large. Not taken;
  decision 8 keeps `polygon` for the shape that must be cheap to move.
- **More polygons, overlapped by hand.** What an app can do today — push
  each wedge's radial edges out half a pixel, split a wedge in two under
  two keys. It fixes this example and nothing else, and a general UI
  library cannot ask every chart to know it. Rejected as the answer; it
  remains the workaround until this is built.
- **A compute rasterizer** (Vello's architecture: flatten, bin, tile,
  fine-rasterize in compute shaders). The right end state for a renderer
  that is only paths; wrong here, where a C host draws the display list
  with whatever it has and no quad kind can mean "run a compute
  pipeline". Not taken.
- **A `sector` element** (centre, radii, angles, gap) with its own SDF
  fragment and `HitShape`. Exact arcs and cheap animation for the pie,
  the donut and the gauge — and nothing else, with the seam still to
  close. Not taken: a path with `A` draws a sector, and a convenience
  builder (`Path::sector`) gives the app the same four numbers.
- **A separate, alpha-only atlas page for masks.** An `R8` page is a
  quarter the memory of the RGBA one and would serve glyph masks just as
  well. Deferred: it is a second texture and a second bind for every
  backend, and it helps text more than it helps paths; it is its own
  decision and does not change this one, which is written so a mask's
  page can change under it.
- **Draw the whole pie as one app fragment.** Easy, and wrong for the
  example: a `hover` event carries enter and leave, not a position, so
  per-segment hover needs per-segment nodes.
- **Parse `d` in the bindings.** Rejected as flattening in the bindings
  was (ADR 0010): four parsers that must agree to the bit.

## Consequences

- **The three reasons ADR 0010 gave are answered, not overruled.** The
  display list stays one flat quad list, and the quad is a kind every
  backend has drawn since alpha.6. A C host walking `KuiQuad` draws a
  path as it draws a glyph. Rasterization is per *change*, not per
  frame: a still path costs a hash lookup after its first frame, and a
  path that changes every frame pays its raster openly, outside the
  atlas, by decision 8.
- **The pie is round and seamless, and so is anything else.** Arcs,
  beziers, holes, self-intersections, a map's regions, a treemap's cells,
  an icon from an SVG file with its `d` passed through: one element.
- **Memory.** A mask in an RGBA page costs four bytes a texel; a 220 px
  wedge is about 30k texels, 120 KiB, and a base page holds dozens beside
  a screen of text. A view of large still paths grows the page as a view
  of large glyphs does, to 4096 at most, and the base never shrinks —
  the policy that is there. Texture-backed paths cost their own texture
  each, bounded by decision 7's threshold on the atlas side and the
  device's limit on the other.
- **CPU on the frame thread.** The raster is where the glyph raster is:
  in the frame, when the slot misses. A view that shows a thousand new
  paths at once pays them in that frame, as a view that shows a thousand
  new glyphs does; the copy-across at a reset keeps the second time
  cheap. The *Measurements* name the numbers this is held to.
- **Three ramps, one width.** A box's edge, a polygon's, and now a path's
  mask ramp over one physical px; a path's fill is half a pixel fatter
  than its outline by decision 5, which the hit outline does not know,
  and nobody can press a half pixel.
- **The seam is a decision, not a surprise.** It is written here why two
  adjacent fills do not show the background, and `polygon`'s own seam is
  named as the one-line amendment it would take.
- **zeno is leaned on directly.** It arrives through `swash`, whose
  version the workspace pins; a swash bump that moves zeno's API moves
  this code, as it already moves the glyph raster.
- **The digest.** A path's quads and warnings digest as any node's; its
  texels do not, which is what the wgpu coverage test is for. The atlas
  epoch stays the hard invalidation ADR 0016 named for anything that
  caches a quad.

## Measurements to take

Named here so the build runs them and amends this document where they
disagree with it; the figures are what the decisions are held to.

| bench | what | held to |
|---|---|---|
| `raster_pie_wedge_220px` | one wedge of the example's pie, a cold mask | under 60 µs (a glyph is 4–9 µs for ~400 texels; this is ~30k) |
| `frame_1k_paths_cached` | 1k six-op fills, every slot a hit | within 2× of `frame_1k_polygons` (97.5 µs): the hash and the quad |
| `frame_1k_paths_fresh` | the same thousand on an empty page | the raster bound, for the record; sets the page it needs |
| `frame_1k_paths_animating` | the same thousand, every op moving | a thousand textures, re-uploaded each frame — the number that says where decision 8's cliff is and whether the threshold wants a count as well as a size |
| `frame_1k_polygons` | unchanged | unchanged, within noise |

## Amendment: what the building changed (2026-10-05)

Built in one day across the four bindings, with the corpus scene, the
example, the benches and the docs. Where the draft and the code differ,
the code is right and this says why.

- **No ABI bump.** The draft said ABI 24. ADR 0006's rule, restated in
  `crates/kui-ffi/src/abi.rs`, is that the version bumps for a struct
  layout or an existing signature and *not* for a new function; `kui_path`,
  `kui_path_d` and `kui_path_parse` are new functions and `KUI_PATH_*` and
  `KUI_FILL_*` new enums, so `KUI_ABI_VERSION` stays 23. The Node wire
  moves to v20 for the new op, as every new op has.
- **The stroke rides the border slots.** Decision 3 said the stroke's
  colour tweens as a line's does. A line has one colour and it rides `bg`;
  a path has two, and its fill is `bg`. The stroke's width and colour ride
  `border_w` and `border_color` — what a border is to a box — so the fill
  tweens, hovers and swaps through `bg` exactly as a polygon's does, and
  the stroke's colour tweens exactly as a box's border does, which today
  is not at all. The bindings spell it as `width` and `color`, as a line's
  rows, so nothing of this shows on the surface.
- **A string-form C door.** `kui_path` takes the flat op form and
  `kui_path_parse` makes it from a `d` string, as the draft said; but the
  corpus wanted the same `path-malformed` warning from every binding, and
  a C host that parses first has nothing to raise it on. `kui_path_d`
  takes the string and hands it to the core's parser under the node's key,
  which is what `<path d>` and `path { d = }` do; `conformance.c` calls
  all three.
- **"Animating" is two changes running** (widened to two within eight
  frames by the review below). Decision 8 said "whose ops hash
  differed on two consecutive frames". Built first as one change — a
  frame whose hash differs from the last — that banished every path a
  view reshaped once, a resize included. It is now two: a key whose ops
  changed this frame *and* the frame before is animating, and stays so.
  One change is a new shape and a new slot; a resize is a new slot per
  frame until it settles, as a glyph's is.
- **The hit outline of a stroke alone.** A stroked path with no fill is
  hit by its stroke as decision 9 said; the contours are closed back to
  their start in the hit list, with the contour breaks kept, so no piece
  runs from one contour to the next.
- **The quad is on whole pixels.** The box's fractional offset is rounded
  to the nearest quarter and baked into the mask; a bin that rounds up
  to a whole pixel moves the quad instead. `kui-wgpu/tests/path_coverage.rs`
  mirrors the shader's glyph-mask branch over the atlas and holds the
  bleed to its promise: two wedges sharing an edge composite to full
  coverage along it.
- **Fewer cliffs than the draft priced.** A texture-backed mask is keyed
  as the atlas would key it, so an animating path's texture is new each
  frame and the last frame's is dropped the frame after through
  `dropped_textures` — one upload and one free a frame, which is the cost
  decision 8 named, with no revision counter to keep.

### Measurements (2026-10-05, a 4-core cloud container, `cargo bench -p kui-core --bench frame`, medians)

| bench | result | held to | |
|---|---|---|---|
| `raster_pie_wedge_220px` | 13.7 µs | under 60 µs | a wedge of the example's pie, ~26k texels, cold: zeno at about half a nanosecond a texel |
| `frame_1k_paths_cached` | 294 µs | within 2× of `frame_1k_polygons` | 1.66×: the hash, the atlas lookup and the quad, with the box and the flattening for it, over a polygon's sixteen normalised floats |
| `frame_1k_paths_fresh` | 625 µs | the raster bound | a thousand 64 px hexagons rasterized on an empty page: ~330 µs of raster over the cached frame, a third of a microsecond each |
| `frame_1k_paths_animating` | 837 µs | where decision 8's cliff is | a thousand outlines moving every frame, each a raster and a texture of its own: the cliff is a thousand such paths at under a millisecond, and the threshold wants no count beside its size |
| `frame_1k_polygons` | 177 µs | unchanged | 97.5 µs on the M3 Pro the table in `docs/performance.md` was written on; this container is slower on every row, and the ratio is what the rows above are read against |

### Review (2026-10-05, the same day)

A review of the built branch found two defects and two rules worth
moving; all four are in the code and its tests.

- **A draw after `Z` starts where the subpath began.** The rasterizer
  kept the point before the close as the current point, where SVG, the
  parser and the hit outline all take the subpath's start: `M10 10 H30
  V30 Z L10 40 L30 40 Z` painted one triangle and was hit as another.
- **A stroke alone is hit by the pieces it paints.** The amendment above
  closed every contour back to its start in the hit list; an open curve
  — a gauge's arc — was then hit along a chord it never drew. Only a
  contour its `Z` closed runs back to its start (`path::flatten_stroke`).
- **"Animating" is two changes within eight frames**
  (`path::ANIMATING_WINDOW`), not two frames running. A shape driven at
  half the frame rate, or one with a frame between its changes that a
  hover or a tick asked for, never changed two frames running and took
  a new atlas slot per shape, which is the churn decision 8 exists to
  stop. One change is still a new shape and a new slot.
- **A `d` string is parsed once per string, not once per frame.** Node,
  Lua and `kui_path_d` hand the string over every frame; the core keeps
  the ops it last parsed under the node's key and parses again only
  when the string differs. The box is two logical px out, not the one
  decision 1 names: one for the bleed and one for the ramp (`b2c4986`).

The first run of the cached frame measured 647 µs, 3.3× the polygon
frame, with the bench building a `Path` per hexagon per frame; the
allocation, not the core, was the difference, and the bench now builds
its geometry once, as the polygon bench's points are. An app keeps its
geometry too.

### Second review (2026-10-05, before the alpha.36 tag and after it)

The pre-tag regression pass read the built element again (backlog
RG107–RG112). What it changed in the decisions above:

- **Decision 8 is not one-way.** An animating key whose shape has held
  for 120 frames (`path::SETTLED_AFTER`, a second at 120 Hz) is a shape
  again: its mask goes back to the atlas and its texture is dropped.
  The latch was for what moves; held for good it also caught a key from
  the tree position whose siblings came and went twice within the
  window, and a spinner that had stopped — each a texture, a bind group
  and a draw of its own for the life of the node. A pause shorter than
  that stays out, so a spinner that stutters does not churn the page,
  and moving again takes two changes, as the first time. The image's
  rule is unchanged: an image's backing is declared by its updates, a
  path's is inferred.
- **Decision 9, with a stroke over a fill.** A path that may paint a
  fill — a `bg`, or one a hover, a press or the focus brings — and has
  a stroke is hit by the fill *or* within half the stroke's width of
  the stroke's pieces (`HitShape::Path`'s `stroke`), so the outer half
  of a thick stroke is hit where it is painted. A stroke with no fill
  at all is still `HitShape::Segments`, with its minimum grab.
- **A texture of its own is the session's to drop when its window
  goes.** A core's `PathTextures` hands what it still holds to the
  session as it is dropped, so the next list any window builds carries
  the drop, as a removed image's does; and a frame whose list nobody
  read (`Core::output`) hands its drops to the next, since an animating
  path sweeps a texture every frame and a frame built twice before a
  render lost one each time.
- **The flat form is checked in the core.** `Core::path_flat_node`
  (and `Content::PathFlat` for a binding that lowers props) reads the
  floats and raises `path-malformed` under the node's key when they are
  not the form — what C and Node drew nothing for in silence and Lua
  failed the view for. A number that is not finite, in either form or
  in the turn, is `path-malformed` too (RG107).
- **The limit is the device's.** `kui-wgpu` opens its device with
  wgpu's default limits, so 8192 is what every device it draws on
  holds; the gate counts the mask's own margin (RG111).

## Action items — all done 2026-10-05

- [x] `crates/kui-core`: `path.rs` (ops, store, `Path` builder,
      `parse`), `NodeContent::Path`, the schema row, `diag` codes
      `path-malformed` and `path-too-large`, the atlas's fourth map and
      `Refusal::Path`, the texture route and the animating rule, the hit
      outline with the fill rule, ghosts.
- [x] `crates/kui-wgpu`: nothing on the wire; `tests/path_coverage.rs`
      for the mask and the bleed beside `segment_coverage.rs`.
- [x] `crates/kui-ffi`: `kui_path`, `kui_path_d`, `kui_path_parse`,
      `KUI_PATH_*`, `KUI_FILL_*`; no ABI bump (see the amendment); the
      header audit and the enum pins in `abi_parity`.
- [x] `crates/kui-node`, `packages/kui`: `<path>` lowered, `d` as string
      or array, wire v20, the `.d.ts` files regenerated.
- [x] `crates/kui-lua`: `path { d = }` and `ops`, the conformance
      binding.
- [x] Corpus: the `path` scene in four bindings.
- [x] `examples/rust/widgets/path.rs`: the pie, round and seamless, with
      the press past the arc in its headless drive; a gauge, an icon and a
      ring beside it. `polygon.rs` keeps its eight-point pie as the
      example of what `polygon` is, and its header says which element a
      round one wants (ADR 0021: one subject each).
- [x] `docs/props.md` regenerated; `docs/howto.md` "How do I fill a
      shape" rewritten around `path`, `polygon` kept for the eight-point
      case; backlog V8 answered against this document; `docs/status.md`
      and `docs/design.md`.
- [x] The benches above, run, and the amendment that records them.
