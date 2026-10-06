---
status: accepted
date: 2026-10-05
---

# A gradient is an image the core paints: `gradient` on a box, rasterized once into the atlas

> **Accepted and built 2026-10-05**, the day it was proposed; the
> *Amendment* at the end records what the building changed — a malformed
> gradient is an error where it is declared and not a warning, the
> atlas's texels are straight alpha, the slot needs a gutter, and the
> ABI did not bump a second time — and *Measured* what the table of
> measurements read: the square's size holds, and the bound on a
> gradient box's cost did not. Asked the
> day `dash` was (backlog V2): "the next would be gradient backgrounds".
> [ADR 0005](0005-the-paint-vocabulary.md) declined gradients for v0 and
> wrote down why — a stop list to parse in five bindings, a type, a
> geometry, an interpolation space "that looks arbitrary in every
> choice", and on the wire either a second colour, a direction and a
> discriminant on the quad or a texture per distinct gradient. Since
> then [ADR 0015](0015-a-fragment-element-and-the-painter-it-is-not.md)
> gave the escape hatch (a `fragment` holding children is a gradient
> card today), [ADR 0025](0025-the-image-is-the-canvas.md) made an image
> a thing the core stretches over a box with linear sampling, and
> [ADR 0040](0040-a-path-is-a-mask-in-the-atlas.md) put a shape the core
> rasterizes into the atlas beside the glyphs. This document takes the
> second of ADR 0005's two wire shapes and argues it is no longer
> expensive: the gradient is rasterized on the CPU, once per distinct
> gradient, into a small image in the atlas, and the box draws it with
> the `Image` quad every backend already has. No quad kind, no shader
> change. It supersedes ADR 0005's *Gradients: out of scope for v0* when
> accepted, and nothing else of that document.

## Context

- **What a gradient costs an app today.** `add_fragment(wgsl)` and a
  `<fragment>` box: the app writes WGSL, the stops ride in sixteen
  `params`, and on the wire it is a `Fragment` quad — one pipeline
  switch per run of them, and nothing at all in a host whose renderer
  has no WGSL. That is the right price for a ring, noise or a shimmer,
  which nothing else can draw. It is a high one for "this card fades
  from one colour to another", which is what was asked for, and it is
  why `docs/status.md` has to say "there are no gradient props".
- **An `Image` quad already does the drawing.** It samples atlas RGBA
  over its `rect`, linearly unless told `nearest`, tinted by `color`
  (which is where group opacity goes) and rounded by `radius` "like a
  solid". A horizontal two-stop gradient *is* a strip of texels
  stretched over a box. Nothing about the quad or the backend needs to
  know the texels were computed and not decoded.
- **The atlas is `Rgba8Unorm` and filtered linearly**, so what the
  sampler interpolates between two texels is the straight sRGB mix —
  the interpolation `Color::lerp` does for a colour transition and the
  one CSS does by default. A stretched ramp and a tween agree.
- **The core already rasterizes for the atlas.** Paths are masks keyed
  by a hash, made on first sight, shared by every node that draws the
  same one, under the page's reset-and-copy policy. A gradient is the
  same arrangement with four channels.
- **`bg` is a colour everywhere.** `VisualStyle::bg` is a `Color` on a
  struct every node copies; it is what `transition`, `enter`, `exit`
  and `keyframes` ease, what `hoverBg`, `pressedBg`, `focusBg` and
  `dropBg` replace, what a `$token` resolves into, and a `uint32_t` in
  `KuiSpec`. Widening it to "a colour or a paint" touches all of that
  and the hot struct the frame benches guard.
- **Stops are a list, and one list-shaped row exists.** `keyframes` is
  a list of stops parsed once in the core from the shared `Value`
  (`keyframes::parse`), spelled as an array in JSX, a table in Lua and
  a struct array in C. ADR 0005 named exactly this as the cost of a
  stop list; it has been paid once.

## Decisions

1. **`gradient` is a row of its own, on anything that paints a `bg`
   box.** It is not a value of `bg`. The gradient is painted **over**
   `bg` and under the border and the children: `bg` stays a colour,
   shows through transparent stops, and keeps everything it has —
   tweens, state backgrounds, tokens. A box with only a `gradient` is
   one quad; with a visible `bg` under it, two; with a border, one more
   for the ring. `hoverBg` and its siblings replace `bg` as they do
   now and do not touch the gradient: a gradient button's hover is a
   second `gradient` chosen by the view, or a translucent `hoverBg`
   under translucent stops.
2. **Linear and radial.** A gradient is a type, a geometry and two or
   more stops:
   - linear — a direction, as `to` (`right`, `bottom`, `bottom right`,
     the eight sides and corners) or as `angle` in turns, 0 pointing
     east and running clockwise, the convention `Path::sector` and
     `rotate` already have;
   - radial — a centre `at` (fractions of the box, default the middle)
     and an ellipse that reaches the farthest corner, CSS's default
     `ellipse farthest-corner`.

   Conic is not in this document (see *Considered options*).
3. **Stops are colours with optional positions.** `[colour, at]` with
   `at` in 0–1; a stop without one is spaced evenly between its
   neighbours that have, the first defaulting to 0 and the last to 1,
   as CSS spaces them. A position less than the one before it is
   raised to it. A colour is whatever the binding's `bg` takes,
   `$tokens` included, resolved where the row is lowered — so a token
   stop follows the theme because the view is rebuilt when it changes,
   like every other token.
4. **Geometry is in the box's unit square.** The gradient is defined
   on (0,0)–(1,1) and stretched to the box. For `to right` and
   `to bottom` that is CSS exactly. For a corner it is CSS's corner
   keyword exactly (`to bottom right` puts the 50% line through the
   other two corners, which is what a diagonal in the unit square
   stretched to the box does). For any other `angle` it is **not**
   CSS's `<angle>`, which is measured in pixels and so depends on the
   box's aspect: a kui `angle` of an eighth of a turn runs corner to
   corner at every aspect, where CSS's `45deg` does not. This is the
   decision that makes the rest cheap — see 6 — and the one place the
   spelling means something a web developer has to be told.
5. **Rasterized on the CPU, into the atlas, as an RGBA image.** A
   linear gradient along an axis is a strip, 256 × 1 or 1 × 256. Every
   other one — an angle, a corner, a radial — is a square, 128 × 128
   (the sizes are the measurement's to confirm). The box draws it with
   one `Image` quad: the box's rect, its radii, linear sampling, the
   group opacity in `color`. No `QuadKind`, no field with a new
   meaning, nothing for `kui-wgpu` or a C host's renderer to learn — a
   host that draws an image draws a gradient.
6. **Keyed by what it is, not where it is.** The key is a hash of the
   type, the geometry and the resolved stops. Because of 4 the box's
   size, aspect and scale are not in it: a gradient is rasterized once
   and then every node that declares it, at any size, on any monitor,
   through any resize or layout animation, draws the same slot. A
   thousand rows with one gradient are one slot; a heat-mapped list
   with a thousand different ones is a thousand strips, 256 texels
   each.
7. **Stops interpolate in straight sRGB, alpha premultiplied.** The
   same mix as `Color::lerp`, and CSS's default. The raster computes
   each texel from the stops; the sampler's linear filter between
   texels is the same mix, so the two agree. Alpha is interpolated
   premultiplied, as CSS does, so `transparent` to a colour does not
   pass through grey. Because the raster is the core's and not a
   shader's, another space (`oklab`) is later a value on the row and
   thirty lines in one function, not a branch in every backend — ADR
   0005's "arbitrary in every choice" stops being a wire decision.
8. **It does not tween.** `transition` eases `bg` under it and
   `opacity` fades it with the box; a `gradient` that differs from last
   frame's is simply a different gradient, drawn at once. One that
   changes every frame is a raster a frame (256 texels, or 16k) and a
   slot a frame, which the page's reset policy absorbs and which is not
   what this is for: a shimmer, a moving sheen and anything driven by
   time stay a `fragment`'s, which reads `time` and re-rasterizes
   nothing.
9. **Spelled like `keyframes`, parsed once in the core.**
   - JSX: `gradient={{ to: 'bottom', stops: ['#1e2030', '#14161e'] }}`,
     `gradient={{ angle: 0.125, stops: [['$accent', 0], ['#0000', 0.8]] }}`,
     `gradient={{ radial: true, at: [0.5, 0], stops: [...] }}`.
   - Lua: the same table, `at` and stops as `{colour, position}`.
   - Rust: `NodeSpec::gradient(Gradient::linear_to(Side::Bottom, &[...]))`,
     `Gradient::angle(turns, ...)`, `Gradient::radial(...)`, each stop a
     `Color` or `(Color, f32)`.
   - C: `const KuiGradient *gradient` on `KuiSpec` — a type, an angle,
     a centre and a `KuiGradientStop` array, as `keyframes` is a
     `KuiKeyframe` array — NULL for none.

   A list with fewer than two stops, a `to` nobody spells or a number
   that is not finite draws no gradient and raises
   `gradient-malformed` once per key, as a malformed `d` does.
10. **Boxes only.** `gradient` is honoured wherever a `bg` paints a
    box. It is not a `path`'s fill, a `polygon`'s, a `line`'s stroke, a
    border's colour or a text's: a path's fill is a one-channel mask
    tinted by one colour, and a gradient under a mask is two textures
    in one quad, which is a different document. Each is listed under
    *Open questions* with what would build it.
11. **Ghosts, hit-testing, access.** A departing box's ghost carries
    its gradient's key and draws the same slot. The hit region and the
    access row are the box's and do not change. The devtools' node
    inspector shows the row as declared.

## Considered options

- **Leave it: a gradient is a fragment.** What is built, and still the
  answer for anything animated or procedural. Kept beside this, not
  replaced by it. Rejected as the *only* answer because the commonest
  paint after a flat colour should not need a shader language, a
  pipeline switch per run, or a renderer that compiles WGSL.
- **A stock gradient fragment**, as `polygon` is a stock fragment: the
  stops in the sixteen params, the WGSL registered by the core. No new
  quad kind, exact at any angle and any stop. But three stops fill the
  params (four floats each, plus geometry), it is still a pipeline
  switch per run — a list of gradient rows interleaved with text is a
  switch per row — and a host without WGSL draws nothing. Not taken.
- **A `Gradient` quad kind, shaded by the backend.** A ramp strip in
  the atlas, its texels in `uv`, the geometry in the slots a solid does
  not use, and the fragment stage computes `t` per pixel: linear,
  radial and conic are three branches, hard stops are exact, and a CSS
  `<angle>` in pixels is free. This is the better picture. It is also a
  new branch in every renderer of the list, a kind an older host
  silently does not draw, and a second texture read on a path it turns
  hot. Not taken now; it is what *Measurements to take* is deciding
  against, and the row's spelling is the same either way, so it can
  replace decision 5 without changing an app.
- **A second colour and an angle on the solid quad.** ADR 0005's first
  shape: two stops only, in `blur` and a word of `uv`. Free in quads
  and exact — and three stops are a different mechanism, which is two
  mechanisms. Rejected.
- **`bg` takes a gradient.** `bg="linear-gradient(…)"`, CSS's own
  spelling. One row instead of two and nothing to learn. But `bg` is a
  `Color` in the style every node carries, the slot five other rows
  replace and four animations ease, and a `uint32_t` in C; every one of
  those then has to say what it does with a paint that is not a colour.
  Rejected for decision 1, which costs a row and leaves them alone.
- **A CSS string, parsed in the core** as `d` and the size expressions
  are. Attractive for paste-from-a-design-tool, and `$tokens` and the
  four bindings' own colour spellings would each need an answer inside
  the string. Not taken; a `Gradient::parse_css` is an addition the day
  someone pastes one, and does not change the row.
- **A registered resource**, `addGradient(spec) → id` and
  `gradient={id}`, like a sound or an image. One `u64` a frame and no
  parse. But a gradient is five numbers and two colours, not a megabyte
  of pixels; a handle makes the view name something it could have
  said, makes token stops stale at the next theme change, and makes a
  data-driven list register and remove a thousand of them. Rejected;
  the hash in decision 6 is the handle, and the core holds it.
- **Rasterize at the box's size**, as a path's mask is, keyed by size
  and scale. Exact hard stops and CSS's pixel angles — and megabytes
  for a window-sized background, a raster on every frame of a resize,
  and the atlas's thrash rule one hero banner away. Rejected for 4
  and 6.
- **Conic.** A conic gradient's seam is a hard edge at an angle, which
  a 128-texel raster stretched to a box draws as a soft staircase; and
  a conic stretched to a non-square box is not what anyone means. Its
  uses — a colour wheel, a pie, a spinner — are a `path`'s sectors, a
  `rotate` or a fragment already. Not built here; it is the first thing
  the quad-kind option would be reopened for.

## Consequences

- **A gradient costs an image quad.** No pipeline switch, no texture of
  its own, the same batch as the glyphs around it. The raster is paid
  the first frame a gradient is seen and after an atlas reset.
- **No renderer changes and no new quad meaning.** `kui-wgpu` is
  untouched; a C host draws gradients the day it links the new
  library. The wire grows a row, not a kind.
- **Hard stops are soft.** Two stops at one position are an edge in the
  raster, and the raster is stretched: along a strip the edge is the
  box's length over 256 wide (4 px on a 1000 px box), on a square it
  is a 128th of the box each way. A smooth gradient cannot show this;
  stripes can. Stripes are boxes, or a fragment, and the row's doc
  says so.
- **`angle` is not CSS's.** Decision 4. The doc comment, `props.md` and
  the how-to each say it in one sentence, with the corner case as the
  example.
- **8-bit banding is what it is everywhere.** A ramp of 8 levels over
  1000 px bands in a browser and bands here; the atlas holds what the
  framebuffer holds. No dither.
- **Up to three quads for a bordered gradient box over a `bg`.** The
  ring is a solid with a transparent fill, drawn after the image so
  the gradient does not cover the border's inner edge. A `quadCount`
  budget should expect it.
- **A second row to explain beside `bg`.** "The gradient paints over
  the background" is the sentence; the state backgrounds not replacing
  it is the surprise, and gets a how-to entry.
- **ABI and wire.** `KuiSpec` gains a pointer and two structs are new,
  so `KUI_ABI_VERSION` bumps; the Node wire bumps for the row's value
  kind. No quad or draw-data layout changes.
- **ADR 0005 is partly superseded**, and `docs/status.md` loses "no
  gradient props" for "linear and radial on a box; conic, animated and
  anything on a shape are a fragment's".

## Open questions

- **The atlas's alpha convention.** Decision 7 wants premultiplied
  interpolation; whether the image slots hold straight or premultiplied
  RGBA, and so what the sampler's own filter does between a transparent
  and an opaque texel, is to be read in `atlas.rs` and the shader before
  building, and decides whether the raster stores premultiplied texels
  or straight ones with the colour bled into the transparent side.
- **The half-texel at each end.** An `Image` quad maps its rect to the
  slot's whole texels, so the outer half-texel of a ramp is flat: a
  512th of the length on a strip, a 256th on a square. Probably
  invisible; if not, the raster extrapolates its end texels by half a
  step rather than the quad learning fractional `uv`.
- **Does the slot bleed.** Slots are padded a texel; a stretched image
  samples at its edge. Images already do this, so it should hold — to
  confirm with a coverage test, not by reading.
- **A gradient under a mask**: a `path`'s fill, and with it a
  polygon's, a text's and a stroke's. One quad sampling a mask and a
  ramp is the quad-kind option again. **Condition:** a view that wants
  a gradient-filled shape and cannot stack a fragment — a chart's area
  fill is the likely first.
- **`gradient` in `keyframes`, `enter` and `exit`.** Not eased
  (decision 8). A crossfade between two slots is two quads and an
  alpha, cheap and not a true interpolation of stops; worth doing only
  if a view asks for a gradient that changes on hover without popping.
- **`oklab`.** Decision 7 leaves the door; whether it should have been
  the default is a taste question the default-is-CSS answer sidesteps.
- **Repeating gradients and `bgImage`.** A box whose background is any
  image, with `fit` — CSS's `background-image` — falls out of the same
  three-quad stack and is a row this document deliberately does not
  add; `repeating-linear-gradient` would want it to tile.

## Measurements to take

| bench or test | what | held to |
|---|---|---|
| `frame_10k_rects_with_gradient` | the plain 10k grid, every cell declaring one shared gradient | within 10% of `frame_10k_rects`: the row's parse, a hash and an image quad for a solid one |
| `frame_1k_distinct_gradients` | 1k rows, each its own strip, steady state | within noise of the above per node: all hits |
| `raster_gradient_strip`, `raster_gradient_square` | one 256 × 1 and one 128 × 128 raster, three stops | recorded; the square is the number that says what a changing gradient costs a frame |
| `frame_10k_rects` and the other guarded rows | unchanged trees | unchanged: the row must cost nothing where it is not declared (`NodeSpec` and `emit_node` are the risk, as C41 and C48 found) |
| `kui-wgpu/tests/gradient_coverage.rs`, two stops | a strip stretched to 1000 px against the mix computed per pixel | every pixel within one 8-bit level |
| the same, a corner and a radial on a 3:1 box | the 128² square against the per-pixel reference | largest difference recorded; proposed bound two levels for a smooth ramp — the number that confirms or reopens the square's size |
| the same, a hard stop | the width of the soft edge | recorded, and quoted in the row's doc |
| the same, transparent to opaque | the midpoint's colour | the premultiplied mix, not grey |

If the corner and radial rows cannot be held with a square the atlas
can afford, the answer is the `Gradient` quad kind under *Considered
options*, and decisions 1–4 and 6–11 stand as written.

## Amendment: what the building changed (2026-10-05)

- **A malformed gradient is an error, not a warning.** Decision 9
  proposed `gradient-malformed`, once per key. The row is carried and
  parsed exactly as `keyframes` is, and a `keyframes` list that does not
  parse fails the view in Node and Lua where it is declared; a second
  convention for the row beside it would be the surprise. So: a `to`
  nobody spells, an unknown field, fewer than two stops in the list or
  a number that is not one is an error from `gradient::parse_with`,
  with the field named. What still draws nothing in silence is a
  gradient that *parsed* and has nothing to paint — fewer than two
  stops left once those whose token missed are taken out (each raised
  as `unknown-token`), or one built in Rust or C with one stop or a
  NaN. No warning code was added.

  *Weighed again (2026-10-07, backlog RG118).* The count is of the
  list, so `[$peach, $peech]` parses and draws nothing. A missed stop
  could instead keep its place painted transparent, so the gradient
  still draws; it stays left out. A missed token leaves any other slot
  as if it were not declared (a `bg`, a keyframe's stop, an
  entrance), and a stand-in here would paint a colour nobody wrote — a
  fade to nothing that reads as intended, where a box with no gradient
  over its `bg` reads as the mistake it is beside the `unknown-token`
  that names it. Tokens have no fallback of their own to paint instead.
  The docs of the row say so.
- **The atlas holds straight alpha** (open question 1). An `Image`
  quad's shader multiplies the texel's rgb by the tint and its alpha by
  the coverage separately, so the texels are straight. The raster mixes
  the stops premultiplied, as decision 7 says, and stores the result
  un-premultiplied; a texel with no alpha keeps the straight mix of its
  neighbours' colours, so the sampler does not darken the texel beside
  it. `a_fade_to_transparent_keeps_its_colour` pins the mix.
- **The slot has a gutter, and it is not empty** (open questions 2
  and 3; see *Measured*). The raster is the strip or the square with
  one texel more all round, each the gradient carried on past the edge,
  and the quad's `uv` is the rect inside. As first built there was no
  gutter, and the coverage test's arithmetic said why there must be
  before the test was written: a stretched quad's sampler reads half a
  texel past the rect it is given, and a strip one texel high, drawn
  over a box forty pixels high, was the gradient only along its middle
  row and faded into whatever the atlas held above and below it.
- **One table in the atlas, not two.** A gradient's slot lives in the
  keyed table a path's mask does (`get_or_insert_gradient` beside
  `get_or_insert_path`), copied across a reset the same way. The keys
  are hashes of different things; nothing else tells them apart, and
  nothing needs to.
- **Where the quad goes.** `paint_box` is unchanged — it is on every
  node's path and the frame benches guard it. A cold `paint_gradient`
  runs after it, on frames whose tree has a gradient at all
  (`Tree::any_gradient`), and puts the image where it belongs among the
  quads the box just pushed: after the shadow and the background,
  before the content, with the border lifted off the background's solid
  into a transparent-filled ring above. A ghost reads the same row off
  the spec it kept.
- **C's stops place themselves with a negative `at`.** `KuiGradientStop`
  is a colour and an `at`; less than zero is "spaced between its
  neighbours", since 0 is a position.
- **The ABI is 24, not 25.** `dash` had already bumped it in the same
  unreleased section; `gradient` on `KuiSpec` (64-bit size 696) and the
  two structs are in the same step. The Node wire stays v21: the row
  rides as a string reference like `keyframes`, and a new row is not a
  new layout.
- **`gradient` on a stroke or a fill is ignored**, in the live pass and
  the ghost's: a `line`, a `polygon` and a `path` paint no box.

### Measured (2026-10-05, an M3 Pro; backlog V9)

`kui-wgpu/tests/gradient_coverage.rs` mirrors the shader's image branch
on the CPU — the atlas sampled linearly, nothing clamped to the slot —
over the quads a core emitted, with other gradients in the atlas on
either side, and compares every pixel of the box with the gradient
computed at that pixel, in 8-bit levels on the worst channel:

| what | box | worst pixel |
|---|---|---|
| a strip, each of the four sides | 1000 × 40 | 0.49 |
| a strip, black to white | 1000 × 40 | 0.50 |
| the square, a corner | 600 × 200 | 0.48 |
| the square, `angle` 0.07 | 600 × 200 | 0.52 |
| the square, a corner, black to white | 600 × 200 | 0.50 |
| the square, radial from the middle | 600 × 200 | 1.20 |
| the square, radial from the top edge | 600 × 200 | 0.75 |
| three stops, strip and corner | as above | 0.50 |

Half a level is the rounding of the texels themselves, so a linear
gradient through 128 texels is exact — a linear ramp is what a linear
filter reproduces — and **the square's size holds**: the proposed bound
was two levels, and the worst case, a radial's curvature between
texels, is 1.2. A hard stop on a strip over 1000 px is 4 px wide. A
fade to transparent is its colour at every alpha. Without the gutter
the same tests read 127 and 169 levels at the edges, which is the bug
the amendment above records.

`benches/frame.rs`, medians:

| bench | what | median |
|---|---|---|
| `frame_10k_rects` | the plain grid | 770 µs |
| `frame_10k_rects_with_gradient` | every cell's solid a gradient instead, all the same | 1.29 ms |
| `frame_1k_rects` | 32 × 32, plain | 78.3 µs |
| `frame_1k_shared_gradient` | the same, one gradient | 131 µs |
| `frame_1k_distinct_gradients` | the same, a gradient each | 134 µs |
| `raster_gradient_strip` | 258 × 3 texels, three stops | 4.0 µs |
| `raster_gradient_square` | 130 × 130, a corner | 24.8 µs |
| `raster_gradient_radial` | 130 × 130 | 41.9 µs |

(These five were taken again by the alpha.37 pre-tag pass, the same
day. As first measured they read 831 µs, 1.38 ms, 84.1, 140 and 143 µs:
the bench's grid chose a cell's paint in one `match` with the gradient
arms in it, and that cost every row built on the grid about 6 ns a
cell — `frame_10k_rects` 8% over alpha.36's with no line of the core
between them. The gradient cell is a function of its own now and the
plain rows are what they were.)

**The proposed bound on the first row was not held, and was the wrong
bound.** "Within 10% of `frame_10k_rects`" assumed a gradient box costs
what a solid one does. It costs about **52 ns more a node**: half of it
is the boxed group of rare rows the gradient lives in, which a
`hoverBg` pays as well (28 ns, measured by giving the plain grid a row
from that group), and the rest is the stops built and hashed by the
view every frame, the atlas lookup and the call out of `emit_node`. As
first built it was 105 ns; the direction is now read off a table for
the sides and corners instead of a cosine and a sine three times a
node, and up to four stops are held in place instead of in two lists.
The rasters were 90 µs and are 25: a square's texels read a ramp mixed
once instead of each mixing its own.

What it means: a card, a header and a row of buttons are microseconds,
and ten thousand gradient boxes are half a millisecond over ten
thousand flat ones — not free, and not what the row is for. Distinct
gradients cost 3 ns a node over a shared one. None of it is the wire
shape's doing: the quad-kind option would build, hash and box the same
row. What would lower it is the registered handle *Considered options*
set aside, or a `Gradient` an app builds once and clones; neither is
built, and the condition is a view that declares thousands.

The guarded rows are within noise of alpha.36 (`bench-check`): a tree
with no gradient pays one flag test a node.

## Action items — all done 2026-10-05

1. Read the atlas's alpha convention and its image sampling at a slot's
   edge; settle the first three open questions.
2. `gradient.rs` in the core: the type, `parse(&Value)`, the stop
   spacing, the two rasters, the hash; unit tests against a per-pixel
   reference.
3. The atlas slot, the `Image` quad and the three-quad order in
   `emit`; the ghost.
4. The row in `schema::PROPS` and its four doors; `KuiGradient` and the
   ABI bump; the Node wire bump; `gradient-malformed`.
5. A `gradients` scene in the corpus; the benches and coverage tests
   above.
6. The rainbow in `examples/rust/apps/loaders.rs` — a gradient two
   tracks long slid under a clip, one strip for good — in place of the
   `features/gradient.rs` first proposed; the how-to
   entry rewritten ("a gradient: the row; a ring, noise or a shimmer:
   a fragment"); `status.md`, ADR 0005's note, the changelog.
