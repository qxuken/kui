---
status: proposed
date: 2026-09-11
---

# The image is the canvas: pixels an app replaces, a texture of their own, and a polygon

> **Proposed (2026-09-11).** Written out of the question "should kui have
> a canvas — raw GPU commands, declarative or callback-shaped — and if not,
> what primitives make an app not need one?" The answer this document
> gives is that nine times out of ten *canvas* means "I will draw it, you
> show it", and that is an image whose pixels the app can replace; the
> tenth time it is data the app wants drawn cheaply, which is the fragment
> reading that image. Neither needs a command list or a device handle.
> The four measurements it waits on are at the end, unrun.

## Context

- **Raw GPU access has a name and a reason to be parked.** ADR 0015
  decision 10 names the `painter` — a Rust trait or a C extension's
  function pointers over the shared `Gpu` and the frame's encoder — and
  does not build it, because it would be the first thing in a frame that
  is not data: Node cannot author one, the corpus digests only a marker,
  headless draws nothing. The backlog keeps it under *Design, wanting an
  ADR* with the condition "a view a fragment cannot serve: a 3D viewport,
  a simulation". Nothing in this document moves it. A *declarative*
  command list (`<canvas ops={[…]}>`) is not the painter, and it has been
  measured here rather than assumed: the core builds 10k nodes in
  ~0.8 ms (ADR 0010's table, ~80 ns a node), Node ~290 ns a node
  (`examples/node/tools/bench.mjs`, 0.259 ms for ~900 nodes, 89% at the
  boundary), so an op list would buy 4–5× in Rust and ~25× in Node for a
  scene of 50k primitives — and no app on kui has one. What it would not
  buy is anything new to draw: an op is a box, a segment or a glyph, the
  things a node already emits, and a *fill* is the path primitive ADR
  0010 rejected as a triangle soup beside the quad list in every backend.
- **Images are immutable, atlas-bound, and silently dropped past 4096.**
  `Resources::add_image(w, h, rgba)` mints a handle and stores the bytes
  (`crates/kui-core/src/resources.rs:345`); there is no update. At
  emission the pixels go into the *glyph* atlas
  (`runtime/emit.rs:1567`, `atlas.rs:207`), whose page doubles to
  `MAX_ATLAS_SIZE = 4096` and no further; an image that does not fit is
  recorded as `None` and **draws nothing, with no warning**
  (`atlas.rs:216`). Sampling is `Linear` for both filters, no mips
  (`crates/kui-wgpu/src/lib.rs:891`). So every "binary stream" — a video
  frame, a camera, an emulator's framebuffer, a plot the app rasterised, a
  PDF page, a map tile, a terminal recording — has exactly one path today:
  `add_image` + `remove_image` every frame, through the atlas, competing
  with text for the page and evicting glyphs to make room. A 12-megapixel
  photo cannot be shown at all.
- **The fragment is the data-visual primitive, one input short.** ADR 0015
  built a box a WGSL function paints from sixteen floats, the clock and
  its own size, and deferred an *image input* (decision 9) "only because
  it is the first thing that binds a resource to another resource, and
  the removal order needs a test before it needs a feature". With a
  texture the app replaces, that input is a waveform, a heatmap, a 50k-
  point line and every image effect, from one quad — which is the whole of
  the "many points" case above without an op list.
- **A fill is the one shape the vocabulary cannot make.** `line` draws
  strokes (ADR 0010); arrowheads, pie slices, the area under a curve and
  any polygonal marker are fills, and ADR 0010 deferred arrowheads to a
  `cap` vocabulary for want of one. A fragment can already fill any
  signed-distance shape, and a polygon of up to eight vertices is
  sixteen floats — exactly the params a fragment takes.
- **Zoom is the app's, and images are the reason it stays so for now.**
  The one canvas app on kui (the mind map) zooms by re-declaring every
  size × zoom and measuring text at the drawn size, which is what the
  core would do; `fragment` gets `size` in physical px and re-renders
  sharp. Raster cannot "adapt" under any design — the core can only
  resample the texture it has; only the app can produce more pixels. So
  whether the camera lives in the app or in the core, an image under zoom
  needs the same three things: a way to replace pixels, the pixel density
  it will be drawn at, and a say in how it is sampled and fitted. A core
  `zoom` row would still need all three, so they are the no-regret step
  and it is declined below with the condition that builds it.
- **`onLayout` reports logical px and nothing about density.** The
  payload is `{x, y, w, h, parent}` in viewport logical px
  (`runtime/emit.rs:1022`); `resize` carries `scale` and `layout` does
  not, so an app sizing pixels for a box multiplies by a number it read
  elsewhere.
- **The frame's side lists are the pattern.** ADR 0015 put a fragment's
  sixteen floats in `DisplayList::fragments`, indexed from `uv[0]`, so
  `Quad` stayed 124 bytes (C15); the renderer splits the instanced draw
  around such a quad at ~0.6 µs of CPU each (`benches/split.rs`), and
  `KuiDrawData` appends the list with an ABI bump the `size` handshake
  lets an old host ignore. A texture-backed image is the same shape: a
  side entry, a split, a bind.
- **Every element and every row costs four bindings and a scene.** The
  design is judged, as ADR 0010's and 0015's were, by how little each
  binding invents: a byte buffer and dims for the update, a string per
  row, a point list the `line` row already spells.

## Decision

1. **`update_image` replaces an image's pixels in place.**
   `Core::update_image(id, w, h, rgba)` in Rust (`Vec<u8>`, taken);
   `kui_image_update(ctx, id, w, h, rgba)` in C; `surface.updateImage(id,
   w, h, buf)` in Node, a `Uint8Array`/`Buffer` handed to the addon (not
   the encoder — it is bytes, not a frame); `kui.update_image(id, w, h,
   s)` in Lua. Dimensions may change. The entry's CPU copy is replaced
   (headless keeps it, as today), a per-entry revision moves, and the
   handle is unchanged, so every node declaring `src={id}` shows the new
   pixels next frame with no view change. An unknown or foreign handle
   raises `foreign-resource` through the ordinary path and changes
   nothing. Pixels are not digested by the corpus, for the reason atlas
   `uv` is not: the quad is the contract, the bytes are the app's.
2. **An image is backed by the atlas or by a texture of its own, and the
   core decides which.** Atlas-backed as today — icons, thumbnails, the
   corpus fixture — until one of two things: it does not fit a
   `MAX_ATLAS_SIZE` page, in which case it is texture-backed from
   registration and the silent drop of today becomes a draw; or it
   receives its first `update_image`, from which frame on it is
   texture-backed for life (evicted from the atlas if it had been drawn
   from there — one eviction, once). An app that streams calls `add_image`
   then `update_image` before the first draw and never touches the atlas.
   The renderer uploads a texture-backed image at the frame's flush when
   its revision moved since the last upload (`queue.write_texture`, the
   whole image; a dirty rect is deferred until a view has one), caches
   textures on `Gpu` by handle shared across every window on the device
   as fragment pipelines are, and drops one when the handle is removed.
3. **On the wire: one quad kind, a side list.** `QuadKind::Texture = 8`.
   The quad's `rect`, `clip`, `clip_radius`, `radius` and `color` are what
   an `Image` quad's are; `uv[0]` indexes `DisplayList::textures`, a
   `Vec<TextureDraw { id: ImageId, uv: [u32; 4] }>` cleared with the quads
   — the texel rect is in *that* texture's texels, and the bind group for
   it carries the texture's size where the atlas's is, so `vs_main`'s
   divide is unchanged. A frame drawing no texture-backed image pays an
   empty vector. `Quad` stays 124 bytes. The renderer splits the
   instanced draw at a texture quad exactly as at a fragment: the run so
   far, then this run with group 0 rebound to that texture (consecutive
   quads of one texture are one run), then the rest. `RenderReport`
   counts the runs. `KuiDrawData` appends `textures` and `texture_count`
   — an ABI bump (the header's size handshake covers it) — and a C host
   that walks the list itself reads the bytes through a new
   `kui_image_pixels(ctx, id, KuiBytes *out)`, the reader that has been
   missing since `kui_image_add`.
4. **Two rows on `image`: `sampling` and `fit`.** `sampling` is
   `"linear"` (today, the default) or `"nearest"` — pixel art, an
   emulator, a zoomed data grid — and rides the `Image`/`Texture` quad's
   `border_w` (0 or 1), a slot those kinds do not use, with the header
   comment saying so; group 0 gains a second sampler and `shade` selects
   by the flag. `fit` is `"fill"` (today, the default: the pixels stretch
   to the box), `"contain"` (the largest rect of the image's aspect that
   fits, centred; the node's box, hit region and layout are unchanged —
   only the painted rect shrinks) or `"cover"` (the box is filled and the
   texel rect is cropped, centred). Both are resolved in the core, so the
   corpus pins them on the quad — `contain` moves `rect`, `cover` moves
   `uv` — and a C host rendering the list gets them for free.
5. **The `layout` payload carries `scale`.** Physical px per logical px
   at that node — today the frame's `Core::scale()`, the same number
   `resize` carries, and the number a view multiplies `w`/`h` by to know
   how many pixels to render before it calls `update_image`. It is a
   promise as much as a field: if the core ever composes a zoom into a
   subtree, this is where the app reads it and nothing else in the loop
   changes. A `Value` map key in four bindings; no ABI change.
6. **A `polygon` element, filled by a stock fragment.** `<polygon
   points={[[x,y],…]} bg/>` in JSX; `polygon { points = {{x,y},…}, bg = }`
   in Lua; `kui_polygon(ctx, points, n, spec)` in C; `ui.polygon(&points,
   spec)` in Rust. Up to **eight** points; a ninth raises
   `polygon-points-truncated` once per key; fewer than three raises
   nothing and draws nothing. The fill is the `bg` row — a fill is a
   background — so it **tweens** through the slot backgrounds already
   ease, and `enter`/`exit` fade it. Placement is `line`'s (ADR 0010
   decision 5): always a float, sized to its bounding box inflated by one
   logical px for the antialiasing ramp, points in the parent's box space
   (`float="viewport"` for viewport space), no room taken in a row or
   column; `slide`, `enter`, `exit` move it as a float. Like a line it
   takes **no input** and has **no access row** (`polygon-ignores-input`
   for the same six keys, a `role` and `label` honoured if declared), and
   ghosts carry its points through the point store the line's use. On
   the wire it is a **`Fragment` quad** — no new kind, no ABI change —
   whose fragment is a WGSL source the core ships and registers once per
   session on first use through the ordinary idempotent-by-source
   `add_fragment`, so `kui_fragment_source` hands a C host the same
   function kui validated. Its sixteen params are the eight points
   normalised to the node's box, the last point repeated to pad; the
   function is the winding-number polygon SDF (concave and simple
   polygons fill correctly, a self-intersecting one fills by winding,
   which is defined and harmless), zero-length padding edges skipped,
   the distance in physical px so the edge is the same one-pixel ramp
   every box gets, and the colour is the quad's `color` — which is what
   puts `bg` through the fragment's `color` slot rather than `params`, so
   a tween is one float less per frame and the stock source never
   changes. A stroked polygon is a closed `line`; a polygon with more
   than eight vertices is two polygons, which the app can see, or the
   path primitive this document does not build.
7. **Fragment image input is the next step, named and not built here.**
   `image={id}` on `fragment`; the prelude exposes `FragmentIn::image`
   (the texel rect) and a `sample(uv)` helper; a texture-backed image
   binds through decision 3's split, an atlas-backed one through the
   atlas the fragment already has. Decision 2 is the test ADR 0015 said
   its removal order needed: a removed image under a live fragment draws
   the "draws nothing" fallback, and the corpus can pin it. It is filed
   in the backlog, to be built the round after this document's
   decisions are, so that the resource-to-resource binding lands on a
   texture path that already exists.

## Considered options

- **A drawing-ops `canvas` element.** Rejected, with the condition that
  reopens it: a view with more than ~10k primitives *from Node* (where
  the boundary is ~290 ns a node and an op would be ~10), or a fill that
  eight points cannot make. Until then it is the node vocabulary again
  in a second encoding, in four bindings, with fills still missing.
- **The painter now.** Rejected; ADR 0015 decision 10 stands, and its
  condition has not been met. Everything in this document is data.
- **A path primitive** (`d`-style paths, lyon). Rejected as ADR 0010
  rejected it: a second vertex stream beside the quad list in every
  backend, and nothing for the C host. The polygon is eight points in a
  fragment because that is the largest fill that costs no new stream.
- **Polygon as a new `QuadKind`.** Rejected: a ninth kind, a shader
  branch and an ABI bump, for a shape one registered fragment already
  draws. The fragment route costs one split (~0.6 µs) per run of
  polygons and nothing else new.
- **Polygon as a library WGSL source the app registers itself.** Rejected
  as the *only* form: it leaks the sixteen-float encoding and the
  eight-point cap into every app, and a fill that cannot be tweened
  through `bg` or placed by its points is half an element. The source is
  still reachable through `kui_fragment_source`.
- **Every image gets its own texture.** Rejected: an icon is 32 px and an
  app has a hundred of them; a bind per icon is a split per icon, and
  the atlas is the right home for what does not change. The core
  decides, on the two facts (size, ever updated) that matter.
- **The app chooses the backing** (`add_image_streaming`). Rejected for
  v0: the two facts above are the whole rule and both are visible to the
  core; a knob would ask the app to predict what it will do with the
  handle. Revisit if the one-time eviction of an atlas-drawn image on its
  first update ever bites, which it would only for an image drawn from
  the atlas first and then streamed.
- **Dirty rects on `update_image`.** Deferred until a view has one; the
  whole-image `write_texture` at 1080p is the first measurement below,
  and the API can grow a rect without changing its shape.
- **`sampling` as a per-image resource property** rather than a row.
  Rejected: the same pixels are a crisp map tile at one zoom and a smooth
  one at another; it is the node's to say, like `radius`.
- **Mipmaps.** Deferred: `contain`/`cover` plus an app that renders at
  `w × scale` pixels needs none; a photo viewer minifying a 12-megapixel
  texture does, and that is the view that files it.
- **A core `zoom` row** — a per-subtree scale as scroll's sibling, layout
  in the subtree's own logical space, `env.scale × zoom` composed at emit
  (`runtime/emit.rs:418` is the one seam), local-space payloads and
  `Core::project/unproject(key, point)` as its doors. Declined **with a
  condition**: a second app that writes the mind map's camera, or a
  request for UI zoom (Cmd+/Cmd−). What it buys is deleting `× zoom`
  from every row; what it costs is the composed transform through
  hit-testing, `onLayout`, access bounds, caret and IME rects, popup
  anchors, ghosts and W7's whole-pixel snapping. Decision 5 is written so
  that the day it lands, an image loop already reads the right number.
  One fact for either design: continuous zoom re-shapes text at every
  fractional size and churns the shape cache (C16); the mind map steps
  by ×1.45 for this reason and a core zoom would want the same
  quantisation.
- **Rotation on `Quad`.** Rejected, C15, as ADR 0010 rejected it; a
  fragment rotates its own fill and rotated *text* stays out.
- **`dash`, `cap` (arrow), `backdrop`, `blend`.** Each already has its
  shape written (ADR 0010 for the first two, ADR 0015 decision 9 for the
  last two) and each waits for a view; they go to the backlog as entries
  rather than being built blind here.

## Consequences

- **`Quad` is still 124 bytes**; `QuadKind` gains a ninth variant;
  `DisplayList` gains a vector empty in every frame that draws no
  texture-backed image; `KuiDrawData` grows two fields under one ABI
  bump; `KuiQuad`'s `border_w` comment gains a sentence for the two
  image kinds.
- **What you can delete:** the `add_image` + `remove_image` pair an app
  wrote around every streamed frame; the "keep it under 4096" check in
  front of `add_image`, and the blank box an app could not explain; the
  aspect arithmetic in front of an `image` that wanted `contain`; the
  three-segment closed `line` that stood in for a filled arrowhead; and
  the `env.scale` a view carried into its `layout` handler.
- **Streaming is a copy per frame, twice in Node.** The core takes the
  bytes (one copy from a `Buffer` in Node, none in Rust) and the renderer
  uploads them; at 1080p that is 8 MB each. The first measurement says
  what it costs; the entry keeps a CPU copy so headless and
  `kui_image_pixels` can read it, which is the second copy's reason.
- **Two corpus scenes move and one is new.** `media` gains a
  texture-backed image, an update between two frames, `nearest`,
  `contain` and `cover`, so every binding agrees on `rect` and `uv` to
  the bit; `fragments` gains nothing; **`polygon`** is a new scene
  claiming the element row — a triangle, a concave arrow, an eight-point
  star, one with nine points and the warning, one faded — pinning the
  params to the bit the way `fragments` does.
- **The mind map keeps its camera**, and gets `contain`, `nearest` and
  the density it needs to put an image on a card.
- **The painter's condition is unchanged**, and a little further away:
  every "canvas" this document has heard of is an image the app draws or
  a fragment that reads one.

## Measurements to run before accepting

1. **`update_image` at 1920×1080 every frame** — the core's copy (Rust
   `Vec` handoff vs Node `Buffer` → `Vec`) and the renderer's
   `write_texture`, on the M3 Pro and on the Windows box; the number the
   doc quotes for "what a stream costs".
2. **`benches/split.rs` gains texture runs** — 0, 1, 8, 32 texture-backed
   images among 10k quads, so the split's bill is known for images as it
   is for fragments.
3. **`frame_1k_typical` unchanged** with an empty `textures` list, and
   `frame_1k_typical_with_8_textures` beside it.
4. **`frame_1k_polygons`** — a thousand six-point polygons declared per
   frame, against a thousand closed six-point `line`s: the point-store
   and normalisation cost, and the one split against six segment quads.

## Action items

1. [ ] `Resources::update_image` + revision; `ImageBacking` decided by the
   two facts; `remove_image` drops the texture.
2. [ ] `QuadKind::Texture`, `DisplayList::textures`, emission; the wgpu
   split, texture cache on `Gpu`, second sampler; `kui-wgpu` conformance
   transcription untouched (no new SDF).
3. [ ] `sampling` and `fit` rows in `schema.rs`, resolved in `emit`.
4. [ ] `scale` on the `layout` payload; `docs/props.md` regenerated.
5. [ ] `polygon`: `ELEMENTS` row, four lowerings, the stock WGSL in
   `fragment.rs`, point store reuse, ghosts, the two warnings.
6. [ ] Doors: `kui_image_update`, `kui_image_pixels`, `kui_polygon`;
   `updateImage` in Node, `update_image` in Lua; `index.d.ts` regenerated.
7. [ ] ABI bump; header audit; `abi_parity`.
8. [ ] Corpus: `media` extended, `polygon` new; `target/conformance.txt`
   regenerated; the coverage test satisfied.
9. [ ] Benches 1–4 above; the numbers written into this document's status
   block.
10. [ ] `docs/howto.md`: "How do I show a video frame / a plot I drew
    myself?" and "How many pixels should I render for this box?"
11. [ ] Backlog: fragment image input, `dash`, `cap`, `backdrop`, `blend`,
    mipmaps, dirty rects, the core `zoom` row and the drawing-ops canvas,
    each with its condition.
