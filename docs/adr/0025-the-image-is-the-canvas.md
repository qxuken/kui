---
status: accepted
date: 2026-09-11
---

# The image is the canvas: pixels an app replaces, a texture of their own, and a polygon

> **Accepted (2026-09-11), built the same day.** Written out of the
> question "should kui have a canvas — raw GPU commands, declarative or
> callback-shaped — and if not, what primitives make an app not need
> one?" The answer this document gives is that nine times out of ten
> *canvas* means "I will draw it, you show it", and that is an image whose
> pixels the app can replace; the tenth time it is data the app wants
> drawn cheaply, which is the fragment reading that image. Neither needs
> a command list or a device handle. Decisions 1–6 are built in four
> bindings with the two corpus scenes; decision 7 is filed as backlog V1.
> The four measurements are run and under *Measurements* at the end, and
> what the building changed from the draft is under *Amendment* after
> them — including the per-node lock that made the first polygon cost
> more than six segments, and the Lua door this draft got wrong.

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

## Measurements

Run on 2026-09-11, M3 Pro, `cargo bench -p kui-core --bench frame` and
`cargo bench -p kui-wgpu --bench split` (fragments, then `TEX=1`), after
the amendment's lock fix. Windows is, as for every round, the platform this
machine cannot run.

**The core** (divan medians, a hundred samples):

| bench | median | what it says |
|---|---|---|
| `frame_1k_typical` | 115.3 µs | unchanged from before the branch: an empty side list costs a frame nothing |
| `frame_1k_typical_with_8_textures` | 122.4 µs | +7 µs for eight texture-backed images — eight float image nodes, eight `fit` resolutions, eight `Arc` clones, eight side entries |
| `frame_1k_closed_lines` | 94.2 µs | a thousand six-point outlines as closed strokes: 6,000 segment quads |
| `frame_1k_polygons` | 98.5 µs | the same outlines as fills: 1,000 fragment quads — parity with the stroke, see the amendment |
| `copy_1080p_frame` | 129.5 µs | the app's own `Vec::clone` of an 8 MB frame, measured alone |
| `update_image_1080p_and_frame` | 109.2 µs | that copy, the handoff, the revision bump, and a frame that draws it — no slower than the copy alone, so the core's share is inside the copy's own noise |

The two 1080p rows say what a stream costs the core: the app's copy and
nothing measurable beyond it. The hot path did not move for the two-field
`NodeContent::Image` or the emission's extra arm: `frame_10k_rects` 714 µs
against 729 at the base commit and `frame_10k_rects_with_text_and_hits`
1.18 against 1.21 ms — an interleaved A/B on the same machine, inside
noise. In Node there is one more copy (`Buffer` →
`Vec`, decision 1), which is the same memcpy again.

**The renderer** (`split.rs`, 10k quads at 2560×1440 offscreen, 320×180
split boxes; `cpu_med` is CPU ms per frame, `saturated` GPU ms per frame
with the queue kept full):

| split quads | fragments: cpu_med | saturated | textures: cpu_med | saturated |
|---|---|---|---|---|
| 0 | 0.148 | 0.259 | 0.145 | 0.256 |
| 1 | 0.151 | 0.259 | 0.140 | 0.265 |
| 8 | 0.153 | 0.272 | 0.142 | 0.283 |
| 32 | 0.167 | 0.296 | 0.152 | 0.355 |
| 100 | 0.212 | 0.360 | 0.184 | 0.590 |

A texture run costs about 0.4 µs of CPU — a bind-group swap under the
same pipeline, cheaper than a fragment's pipeline swap. The GPU column is
the one to read carefully: at a hundred boxes the texture case is slower
than the fragment case not because of the split but because each box
samples a 1920×1080 texture minified into 320×180 pixels with no mips —
that is the fill's bill, and the mipmap deferral (V6) is what it argues
for once a view minifies a large stream. At the sizes an app draws a
stream (one box, near its own size) the row that matters is 1: within
noise of no split at all.

## Amendment: what the building changed

- **A polygon cost more than six segments, at first.** The first
  `frame_1k_polygons` was 108 µs against 95 for the closed stroke — one
  quad slower than six. The cause was `stock_polygon()` taking the
  session lock per node to check the cached handle was still registered.
  The handle is now checked once (registered on first use) and forgotten
  by `remove_fragment` if a host removes it, which is the only way it can
  die; that took the bench to 98.5 µs, parity. What remains over a box is
  the fragment path's own per-quad session lookup in `push_fragment`,
  shared with `fragment`, and worth hoisting the day a view draws ten
  thousand of either.
- **Lua has no `update_image`, because it has no `add_image`.** The draft
  named `kui.update_image(id, w, h, s)` for Lua; Lua has never registered
  an image — the host does, and hands the script an integer — so the door
  belongs beside `kui_image_add` on the host and Lua gets the rows
  (`sampling`, `fit`) and the element (`polygon`) only. The C host that
  embeds a Lua panel streams through `kui_image_update` as it always
  registered through `kui_image_add`.
- **The fill's colour goes through the quad, not the params.** Decision 6
  said `bg` rides the quad's `color` slot; building it meant the prelude's
  `FragmentIn` gaining `color: vec4<f32>` — the quad colour, white on a
  `fragment` and the fill on a `polygon` — and the epilogue multiplying
  the fill alpha in with the group opacity it already multiplied. Any
  fragment may now read `in.color`, which is one more thing a gradient
  can take from the view without spending params on it; the `fragments`
  corpus scene did not move, since its quads were white already.
- **`fit` is the row's name and `"fit"` is a sizing value.** `width="fit"`
  means the box meets the pixels; `fit="contain"` means the pixels meet
  the box. The ELEMENTS doc says both in one sentence rather than renaming
  the row `objectFit`; the collision is between a value and a name, not
  two names, and every binding spelled it without a clash.
- **The `cover` crop is whole texels.** A half-texel crop edge samples the
  neighbour under linear filtering, so the crop's size is rounded and its
  origin floored; the corpus `media` scene pins the 8×2 stream in a 12×12
  box as `texture 2 3 0 2 2`.
- **`scale` on the payload is a `Value` key, not a new struct field**, so
  no ABI moved for it; the one test that deep-compared a `layout` payload
  gained the key, which is the "what breaks" line.
- **The corpus report grew a column and a line, not a scene per row.** The
  ninth `kinds` column and the `texture <i> <x> <y> <w> <h>` line are what
  all four adapters mirror; the digest mixes a texture quad's `uv` (the
  side-list index) as it mixes a segment's endpoints, and the texel rect
  is pinned by the line rather than the digest, the way a fragment's
  params are.
- **The texture is shared across windows; the bind group is not.** The
  device's `Gpu` caches `ImageTexture`s by handle, as it caches fragment
  pipelines, and re-uploads when the revision moved or makes a new one
  when the size changed; each `Renderer` keeps its own group-0 bind group
  per texture — with a private globals copy whose `atlas_size` is the
  texture's, rewritten each frame the image is drawn — and rebuilds it
  when the `Arc` it holds is no longer the cache's. A removal reaches the
  renderer as `DisplayList::dropped_textures`, carried across frames by
  the core because `remove_image` can land between them.
- **Consecutive quads of one texture are not merged into a run.** Each
  takes a bind and a draw, as consecutive fragments of one handle take a
  pipeline set each (ADR 0015's amendment says the same); at 0.4 µs a
  draw it is not the thing to optimise first.
- **`RenderReport` still does not count runs.** ADR 0015 said it would and
  it did not either; nothing reads it, so the sentence went rather than
  the field arriving.
- **The image example is the loop, with a drive.** `image.rs` gained a
  plasma stream rendered at the size its `layout` event reports,
  `nearest` beside `linear`, and the three fits in one box size, and a
  headless drive that pins the loop closing (no render before the report,
  one after, the texture at the box's size) and the fits differing in
  paint and not in box; `polygon.rs` is the element's own example with its
  drive (every fill one fragment quad, no hit region, a hovered wedge's
  fill eased to the accent through the `bg` slot).

## Action items — all done 2026-09-11

1. [x] `Resources::update_image` + revision; `ImageBacking` decided by the
   two facts; `remove_image` drops the texture (via `dropped_textures`).
2. [x] `QuadKind::Texture`, `DisplayList::textures` + `texture_pixels`,
   emission; the wgpu split, texture cache on `Gpu`, second sampler.
3. [x] `sampling` and `fit` as image-own rows, resolved in `emit`
   (`fit_image`).
4. [x] `scale` on the `layout` payload; `docs/props.md` regenerated.
5. [x] `polygon`: `ELEMENTS` row, four lowerings through `Content::Polygon`,
   `fragment::POLYGON`, ghosts, the two warnings, `FragmentIn::color`.
6. [x] Doors: `kui_image_update`, `kui_image_pixels`, `kui_image_with`,
   `kui_polygon`; `updateImage` and `textureDraws` in Node; Lua's rows and
   element (no `update_image`, see the amendment); `index.d.ts` and
   `jsx-runtime.d.ts` regenerated / extended.
7. [x] ABI 14; header audit; `abi_parity` rows for the struct and the two
   enums.
8. [x] Corpus: `media` extended (the `stream` fixture), `polygon` new;
   `target/conformance.txt` regenerated; every adapter replays.
9. [x] Benches 1–4, run; the tables above.
10. [x] `docs/howto.md`: three answers (a stream, how many pixels, a fill);
    README's vocabulary and limits paragraphs.
11. [x] Backlog V1–V8, filed with the proposal; V1's precondition is now
    met.
