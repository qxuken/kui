---
status: accepted
date: 2026-10-09
---

# An image drawn smaller is drawn from a level the core halves

> **Accepted and built 2026-10-09**, the day it was proposed; the
> *Amendment* at the end records what the building measured and
> changed. Backlog V6's second half, its condition met
> by berainder's alpha.47 upgrade: the app shows its bear photos on
> cards smaller than the files, and smaller again on the 0.9-scaled
> card underneath, and keeps a 25-line crop-and-resample in `bears.rs`
> to bring each photo down to card size before `add_image`, because the
> renderer samples the whole image with no mip chain. [ADR
> 0025](0025-the-image-is-the-canvas.md) deferred mipmaps until "a
> photo viewer minifying a 12-megapixel texture" filed them; this is
> that, from a card game. V6's first half, dirty rects on
> `update_image`, keeps its own condition and is not here.

## Context

- **What a minified image costs today.** An `image` node's quad samples
  the image's texel rect bilinearly — four texels around each pixel
  centre — whatever the ratio of texels to pixels. At 1:1 to 2:1 that
  is every texel read. At 4:1 three texels in four are skipped and the
  ones read are an accident of where the pixel centres land: a photo
  shrunk to a card shimmers as it moves, and its fine lines break up.
  ADR 0025's split bench shows the GPU bill as well: a hundred 320×180
  boxes each sampling a 1920×1080 texture ran at 0.59 ms against 0.36
  for fragments of the same size, the difference being cache misses
  from minified reads with no mip chain.
- **Where an image lives.** An image that fits a `MAX_ATLAS_SIZE`
  (4096) page and was never updated is blitted into the window's glyph
  atlas at first draw, whole, and drawn as an `Image` quad whose `uv` is
  its texel rect in the page (`atlas.rs`, `get_or_insert_image`). One
  past a page, or one ever updated, is texture-backed: its own texture,
  a `Texture` quad and an entry in `DisplayList::textures`
  (`ImageBacking`). A 4032×3024 phone photo is atlas-backed, and takes
  48 MB of a page that also holds every glyph the window draws.
- **Who draws.** `kui-wgpu` is the renderer this repo ships; a C host
  that draws `KuiDrawData` itself (ADR 0012's contract) uploads the
  atlas page and the texture side list and samples what the quads
  name. Whatever answers V6 has to reach that host too
  ([`feedback: bindings first`] — a capability goes where every binding
  and every backend gets it).
- **What the core knows at emission.** The image's size, the texel rect
  `fit` picked, the rect the quad is drawn at in physical px, and —
  since ADR 0043 — the clip entry's composed `Transform`, whose `scale`
  is the factor every ancestor's `scale` multiplied into. That is the
  whole input to "how many texels per pixel", per quad, on the CPU.

## Decisions

1. **A level is the image halved, and halved again.** Level 0 is the
   image; level *n* + 1 is level *n* with each 2×2 block averaged into
   one texel, `⌈w/2⌉ × ⌈h/2⌉`, the last row and column of an odd size
   repeated. The average is taken in linear light with the alpha
   premultiplied — sRGB decoded through a 256-entry table, the result
   encoded through a 4096-entry one — so a level is neither darker
   than its source nor fringed where a transparent edge meets colour.
   The chain stops at 1×1.
2. **The core picks the level per quad.** For an image drawn with
   `sampling: linear` (the default), the ratio is
   `r = min(texels_w / px_w, texels_h / px_h)`: the texel rect `fit`
   picked over the drawn rect in physical px, times the clip entry's
   composed `scale`. The level is `⌊log₂ r⌋`, at least 0, at most the
   chain's last — the deepest level still at least as many texels as
   pixels on both axes, so a level never magnifies and the bilinear
   sample inside it minifies by less than two. The smaller ratio of the
   two axes decides, so a `fill` stretched in one direction is never
   blurred in the other. `sampling: nearest` always draws level 0 — it
   asked for the texels as they are. Nothing new in any binding: no
   row, no door, no ABI. The level is a fact the core derives, as
   `ImageBacking` is.
3. **A level lives where level 0 would, in the atlas.** The page keys
   image slots by `(image, level)`; a level is blitted at the first
   draw that wants it and its slot named by the quad, an `Image` quad
   as before. An image only ever drawn small never puts level 0 in the
   page at all — the 48 MB photo drawn on a 400-point card at 2× takes
   the 1008×756 level's 3 MB. `fit`'s crop is computed on the level's own
   size, so `cover` crops whole texels of the level. A host that draws
   `KuiDrawData` gets the level in the page it already uploads, and
   changes nothing.
4. **A texture-backed image is levelled until it is updated.** One that
   is texture-backed because it is larger than a page, and was never
   updated, draws a level that fits a page from the atlas like any
   other, and level 0 from its texture as before. One ever updated —
   a stream — draws level 0, as today: halving a 1080p frame on every
   update is milliseconds a frame, and a stream minified on screen is
   V6's other half's condition, still unmet.
5. **Levels are made once per session, lazily, and shared.** The
   halved pixels are kept on the image's entry in the session's
   resources, so a second window, or the same window after its atlas
   page was reset, blits them again without halving again. Level *n* is
   made from level *n* − 1, which is kept too, so a chain costs a third
   of the image again at most, and only the levels some draw asked for
   (and those above them) are ever made. An `update_image` drops them;
   `remove_image` drops them with the entry.
6. **Fragments sample level 0.** A `fragment`'s `image` input is
   sampled at coordinates the shader computes, and the core does not
   know the ratio there; it keeps the whole image. A fragment that
   wants a smaller one is handed a smaller one.

## Consequences

- berainder deletes its resampler: a photo is registered as decoded and
  declared at the size it is shown.
- A photo that animates its size across a power of two switches level
  at the crossing — the image sharpens or softens by a step there,
  where trilinear sampling would blend the two. At the ratios a card's
  hover or its 0.9 under-scale move through, it does not cross; a
  pinch-zoom would, and is where trilinear comes back (see *Open
  questions*).
- The first draw of a large photo at a small size pays the halving on
  the frame that draws it: about a quarter of the image's pixels a
  level, so a 12-megapixel photo's first level is three million output
  texels. Measured below. berainder paid the same in its own
  resampler, before the first frame.
- A window's atlas holds less for a photo drawn small, and holds a
  second slot for a photo drawn at two sizes at once (a card and its
  thumbnail).
- Hit-testing, layout, `image_size`, access and the texture side list
  are unchanged: a level changes which texels a quad names and nothing
  else.

## Considered options

- **A mip chain on the GPU, made by the renderer.** The usual answer:
  `mip_level_count` on the texture, a blit pass per level at upload, a
  sampler with a mip filter, the hardware choosing per pixel from the
  derivatives. Declined as the answer for atlas images: mip levels of
  a page that packs glyphs, paths and images side by side bleed every
  neighbour into every item from the second level down, unless every
  item is padded to its level's alignment — and the C host that draws
  `KuiDrawData` would need the same chain built its own way. For a
  texture-backed *stream* it is still the right shape, and is what V6's
  stream half would build; it composes with this.
- **Trilinear, two levels blended in the shader.** Declined for now:
  the quad would carry two texel rects (the instance grows) and the
  fragment stage would sample twice for every image, to smooth a
  switch that only a size animated across a power of two shows.
- **A row, `mipmap` or `minify`, that the app sets.** Declined: there
  is no image an app wants aliased when it is drawn smaller, and
  `sampling: nearest` already says "these texels, as they are".
- **The app resamples, as berainder does.** The status quo. It works,
  and every app that shows a photo writes it, picks a filter, and gets
  sRGB wrong.

## Measurements to take

- The halving: a 4032×3024 level 0 to its first level, and the whole
  chain, in release, on the Mac.
- The frame: a frame drawing a 1920×1080 photo in a hundred 320×180
  boxes, before (level 0, every box) and after (level 2), CPU and GPU —
  the row of ADR 0025's split bench that argued for this.
- The frame bench guard, which draws no minified image, unchanged.

All three are taken; see the *Amendment*.

## Open questions

- Trilinear for a size that animates through a power of two, when an
  app pinch-zooms a photo.
- Making levels off the frame thread for a photo large enough that its
  first draw is a dropped frame.

## Amendment (2026-10-09, built)

Built the day it was proposed, as decided, in `kui-core` alone:
`mip.rs` (`halve`, `depth`, `level_for` and the two sRGB tables),
`ImageEntry::level(n)` holding the chain behind a mutex on the
session's entry (`levels_allowed` is `rev == 0`; both updates drop it),
the page's image slots keyed `(ImageId, level)` with
`get_or_insert_image_level` taking the pixels as a closure so a level
already in the page is never made, and `Emit::image_level` choosing per
quad from the `fit` rect, the display's scale and the clip entry's
composed `scale`. No binding, no door and no ABI moved; `kui-wgpu` did
not change. What the building measured and changed:

- **The halving has an opaque fast path.** Written first with the
  premultiplied weights for every block, a 4032×3024 photo's first
  level took 13.4 ms; a block whose four alphas are 255 now takes the
  plain mean, and the same level takes **10.1 ms**
  (`benches/levels.rs`, `halve_4032x3024`). The first frame that draws
  the photo on a 400×300 card at 2× — levels 1 to 3 made and level 3
  blitted — is **13.2 ms**, once per photo per session; every frame
  after reads **0.33 µs** for the whole one-image frame. That first
  frame drops at 120 Hz; *Open questions* keeps making levels off the
  frame thread.
- **The GPU half of the argument was smaller than ADR 0025 read it.**
  `benches/split.rs` had not run since ADR 0043 grew the instance (its
  mirror of `Instance` was 128 bytes against the renderer's 176, so the
  pipeline refused to build); it is mirrored again, with `LEVEL=n`
  (the 1080p texture halved `n` times) and `NOISE=1` (texels a GPU
  cannot compress — the flat orange one read the same at level 0 and
  level 2, 0.757 ms against 0.756, because a flat texture is free
  however it is minified). With noise, a hundred 320×180 boxes
  saturated at **0.786 ms from level 0 and 0.758 ms from level 2**:
  about 0.3 µs a box on an M-series GPU. The texture boxes' gap to
  fragments of the same size (0.46 ms) is mostly not minification. The
  case for this ADR is what is drawn, and the page's memory, not the
  GPU's time.
- **What is drawn.** The `image` example gained a 768-px zone plate
  drawn at 128 px, from a level beside `nearest`. Drawn whole and
  bilinear (levels switched off for the comparison), its outer rings
  alias into false rings as strong as `nearest`'s; from the level they
  fade to the grey they average to, with faint ghost rings left at the
  corners. Those are decision 2's choice showing: at 2× the plate is
  three texels a pixel, so it is drawn from level 1 at 1.5 texels a
  pixel, and bilinear sampling at 1.5 still folds the top half-octave
  of a pattern made of nothing else. Rounding the level instead, as a
  GPU's nearest-mip filter does, would draw level 2 magnified by 1.33
  — no ghosts, and every icon drawn a little smaller than its file
  blurred by the same factor. A UI has more icons than zone plates;
  the rule stays "never magnify", and trilinear is what removes the
  ghosts without the blur.
- **The guard.** `nu scripts/bench-check.nu` against alpha.47: the eight
  guarded rows within −0.6% to +0.8% (run-to-run ±2.8%). The two 1080p
  `update_image` rows read +4.4% and +6.9% in that run and −0.8% and
  −1.6% run alone; memcpy-bound rows, noise.
- **Tests.** `mip.rs`'s six (a flat image halves to itself, the linear
  light mean of black and white is sRGB 188, red beside clear stays red
  at half alpha, an odd side repeats its edge, the chain's depth, the
  level never magnifies) and `tests/image_levels.rs`'s six (the deepest
  level that covers, the display's scale and a node's `scale`, `cover`
  on the level, `nearest` and a stream at level 0, an image past a page
  drawn from the page, the page holding the halved texel). The `image`
  example's headless drive checks the zone plate's two quads.
