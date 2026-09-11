---
status: accepted
date: 2026-09-08
---

# A `fragment` element: a box a WGSL function paints, and the `painter` it is not

> **Accepted and built (2026-09-08).** Proposed, measured twice and accepted
> the same day. The two measurements it was waiting on are the sections at
> the end, and neither disqualified it: naga costs a cold build of
> `kui-core` alone and nothing a windowed binding does not already pay, and
> the draw-call split costs about 0.6 µs of CPU per fragment with nothing
> the GPU can see. What building it changed from the draft below is
> recorded in [Amendment: what the building changed](#amendment-what-the-building-changed)
> at the end.

The bake-off against iced and gpui (2026-09-08) has one row kui loses on
paint alone: "canvas / custom shader escape hatch — quads + lines only".
iced's answer is `widget::shader`, a trait the app implements with the
wgpu device in hand; gpui's is a custom element. Both are closures over a
GPU, and neither crosses a language boundary or a headless test. ADR 0005
declined gradients because a stop list, a type, a geometry and an
interpolation space are a design problem in five bindings, and the same
reasoning refuses every other visually rich thing one at a time: noise, a
ring, a sparkline, an image effect.

We are proposing **one new resource, a fragment**, WGSL source registered
like a font and validated at registration, and **one new element,
`fragment`**, an ordinary laid-out box whose pixels that function paints
from sixteen floats, the frame clock and its own size. It emits one quad
of a new kind; the renderer draws the run before it, that quad with the
fragment's own pipeline, and the run after. `Quad` does not grow, every
binding gets the element as data, the corpus digests its parameters, and
a headless test asserts them. We are **not** building the iced-shaped
hatch, and we are naming it anyway — **`painter`**, a Rust-only trait or
a C extension's function pointers, split around by the renderer the same
way — so that the word `shader` is held for nobody and the reserved
`PainterId` in `resources.rs` has a purpose. What a fragment cannot do
is listed with the reason, and the painter is what would do it.

## Context

- **The frame is data, all the way to the pixels.** `DisplayList` is one
  `Vec<Quad>` (`crates/kui-core/src/display.rs:206`) and the wgpu backend
  draws it as one instanced draw with one pipeline
  (`crates/kui-wgpu/src/lib.rs:1`). Lua, C and Node all render through
  that backend by way of the `kui` runner; the C ABI's `kui_draw_data`
  hands the same list to a host that wants to walk it headlessly. A
  closure over a `wgpu::Device` has no place in that list: it cannot be
  encoded by Node, cannot be a `KuiQuad`, and cannot be digested by the
  corpus.
- **`Quad` must not grow.** C15 is the record of a frame getting 2.5×
  slower one small field at a time; ADR 0010 fitted a segment's endpoints
  into the `uv` slot for that reason and the struct stayed at 124 bytes.
  A fragment needs a handle and sixteen floats. Neither fits, and neither
  belongs on a struct 20,000 copies of which carry nothing of the kind.
- **The renderer is already an SDF shader.** `shader.wgsl` computes a
  rounded-box distance for the shape, a second for the rounded clip, ramps
  both over `AA`, multiplies group opacity into the colour and blends
  premultiplied. Every one of those is a thing a custom fragment must get
  right to look like a kui node, and every one is a thing an app author
  would get wrong first. The contract that keeps them in kui's hands is
  the one worth designing.
- **Resources have a shape.** `add_font_data` returns `Option<FontId>`,
  `None` when the bytes hold no face; `add_system_font` is idempotent so a
  view can call it every frame; handles come from a process-wide mint so a
  foreign one warns rather than aliases
  (`crates/kui-core/src/resources.rs:1`). A `PainterId` key type has been
  in that file since the first commit and nothing uses it.
- **The clock and the frame request exist.** `Core::set_time` feeds the
  transition clock every frame; `Core::request_frame` asks for one more
  and `animating()` reports it (`crates/kui-core/src/runtime.rs:576`). A
  fragment that moves needs both and nothing new.
- **Every element costs four bindings and a scene.** An `ELEMENTS` row is
  hand-lowered in Rust, Lua, C and Node; the corpus's coverage test
  refuses a row no scene exercises; the C and Node adapters mirror the
  quad digest word for word. The design is judged by how little each
  binding invents: a handle, a float list, a flag.
- **ADR 0006's bump rule.** An [out] struct that changes layout, appends
  included, bumps `KUI_ABI_VERSION`; the leading `size` on `KuiDrawData`
  is what lets an un-recompiled host keep the prefix it knows. A new
  function bumps nothing.
- **The name.** Elements are named for what they put on screen — `box`,
  `text`, `image`, `line`, `cells` — not for what they lack. `shader`
  would promise iced's hatch and deliver less; `paint` is the core's own
  `Paint` type and ADR 0005's title; `canvas` promises drawing commands;
  `effect` is ADR 0013. What the app writes is a fragment function.

## Decision

1. **A fragment is a resource.** `Core::add_fragment(wgsl: &str) ->
   Option<FragmentId>`; `kui_fragment_add(ctx, KuiStr) -> u64` in C;
   `surface.addFragment(src)` in Node; `kui.add_fragment(src)` in Lua.
   The core prepends its prelude, appends its epilogue, and validates the
   whole with naga's WGSL front end at registration. Source that fails
   returns `None` and raises `fragment-rejected` through the ordinary
   diagnostics path, carrying naga's message with the line renumbered to
   the app's own source. A frame never sees an invalid fragment.
   Registration is **idempotent by source**: the same text gets the same
   handle, so a view that registers in its `view` pays a hash, not a
   compile. `ResourceKind::Fragment` joins the mint with the fallback
   "draws nothing", so a removed or foreign handle paints a transparent
   box and the foreign one warns.
2. **The contract is one function, fragment-only.** The app writes:

   ```wgsl
   fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32>
   ```

   `FragmentIn` carries `local` (the pixel in the node's own space, physical
   px, y down), `size` (the node's size in physical px), `time` (the frame
   clock in seconds, the same one transitions read) and `scale`. The
   return is a straight colour with alpha. The prelude's `fs_main`
   computes the same rounded-box coverage, rounded-clip coverage and
   group-opacity product every other quad gets, multiplies them into that
   alpha, premultiplies and blends as `shade` does. The vertex shader is
   kui's `vs_main`, unchanged. An app never writes a pipeline, a bind
   group, a clip or a blend.
3. **The `fragment` element is a box.** `<fragment src={id} params={[…]}
   animate/>` in JSX; `fragment { id =, params = {…}, animate = true }` in
   Lua; `kui_fragment(ctx, id, params, n, spec)` in C; `ui.fragment(id,
   &params, spec)` in Rust. It lays out like an `image`: sized by the
   spec, in flow or floated, clipped, rounded by its own `radius`, faded by
   group opacity, and it takes input like any node — `onClick`, hover,
   focus, drag — because it is one. Children are allowed and paint after
   it, so a gradient card with a title and two buttons is a `fragment`
   with children. `params` is up to sixteen numbers, positional, missing
   ones zero; a seventeenth raises `fragment-params-truncated` once per
   key. `animate` calls `request_frame` every frame the node is declared,
   opt-in like `exit`, so a still fragment costs the loop nothing.
4. **On the wire: one quad, a side list.** `QuadKind::Fragment = 7`. The
   quad's `rect`, `clip`, `clip_radius` and `radius` are the node's;
   `color` is unused and zero; `uv[0]` indexes `DisplayList::fragments`, a
   `Vec<FragmentDraw { id: FragmentId, params: [f32; 16] }>` cleared with
   the quads. A frame that draws no fragment has an empty vector and pays
   nothing. `Quad` stays 124 bytes.
5. **The renderer splits the draw.** `Renderer::render` walks the
   instances in order. At a fragment quad it draws the run so far with the
   über-pipeline, binds the fragment's pipeline, and draws that instance
   from the same instance buffer with the same bind group 0; consecutive
   quads of the same fragment draw as one instanced run. Params are
   uploaded once per frame into a uniform buffer with a dynamic offset per
   draw, each entry padded to the device's alignment. Pipelines are cached
   on `Gpu` by handle, shared across every window on the device, compiled
   on first sight and dropped when the handle is removed. The count of
   draw calls a frame makes is `1 + 2 × fragment runs`, and `RenderReport`
   says how many.
6. **Headless and corpus.** A headless `Core` emits the quad and the side
   entry and draws nothing, as for every kind. The conformance report's
   `kinds` grows an eighth column and `Expect` a `fragments` count; the
   quad digest mixes the sixteen params as bits, in word order, and skips
   the handle for the reason it skips atlas `uv` — a handle is minted, not
   declared. One scene, `fragments`, claims the row: a still gradient, an
   animated one at a fixed `Time`, one with children, and one with a
   removed handle, so all four bindings agree on the params to the bit and
   on what a dead handle emits.
7. **The C ABI.** `KUI_QUAD_FRAGMENT = 7`; `KuiDrawData` appends
   `fragments: *const KuiFragmentDraw` and `fragment_count`, a bump to
   ABI 9 that the `size` handshake lets an older host ignore. A host that
   renders the list itself and wants the source gets it from
   `kui_fragment_source(ctx, id, KuiStr *out)`, the prelude included, so
   the contract it compiles is the one kui validated.
8. **What a fragment can do,** so the doc says it rather than the reader
   guessing: any procedural fill (gradients with as many stops as the
   params hold, noise, patterns), any signed-distance shape (rings, arcs,
   gauges, dashed and animated borders), any animation that is a function
   of time, small data visuals from the params, image effects and large
   data visuals once decision 9's image input lands, and composition with
   text and controls as children.
9. **Deferred, with the shape of each.**
   - **An image input.** *Built 2026-09-11 (backlog V1, ADR 0025
     decision 7): `image={id}`, `FragmentIn::image`, `kui_sample(uv)` and
     `kui_sample_nearest(uv)`; the removal order is pinned both ways.*
     `image={id}` on the element; the prelude exposes
     the atlas and the image's texel rect as `FragmentIn::image` and a
     `sample(uv)` helper. This is what turns a fragment into an image
     effect and a data texture into a waveform or a heatmap. It is deferred
     only because it is the first thing that binds a resource to another
     resource, and the removal order needs a test before it needs a
     feature. The atlas grows to 4096 px square, so a data texture
     competes with glyphs; a dedicated texture per fragment is the change
     if that bites.
   - **A pointer input.** `FragmentIn::pointer` for a hover-tracked node.
     Deferred because the app already receives hover and drag positions as
     events and can fold them into `params` one frame late; a uniform is a
     shortcut, not a capability.
   - **Named parameters** by naga reflection of a `Params` struct.
     Deferred: sixteen positional floats are the same 64 bytes in every
     binding with no per-binding schema; names come when a view has more
     than one fragment and a wrong-index bug to show for it.
   - **A backdrop.** `backdrop` on the element copies the box's region of
     the framebuffer to a texture at the flush before it, and the prelude
     exposes it. Frosted glass. One copy per box, bounded and data. Not in
     v0 because nothing asked for it.
   - **Blend modes.** One blend, premultiplied over. `blend="add"` is a
     second pipeline variant per fragment; deferred until a glow stacks.
10. **What a fragment cannot do, and the `painter` that would.** No
    multi-pass or feedback (trails, fluid, life), no geometry (meshes, 3D
    viewports, particle systems), no compute, no storage buffers, no
    readback, no text, no per-pixel hit test. Each needs either a texture
    that outlives the pass, a vertex stream beside the quad list, or the
    device itself. That is a **`painter`**: a Rust trait with `prepare`
    and `paint` over the shared `Gpu` and the frame's encoder, or a C
    extension's pair of function pointers over the same, registered under
    the `PainterId` that already exists, placed by a `painter` node that
    emits a marker quad the renderer splits around exactly as it splits
    around a fragment. It is the first thing in a frame that is not data:
    Node cannot author one, the corpus digests only the marker, and
    headless draws nothing. **It is not built by this ADR** and it is not
    proposed by it; it is named so that the fragment does not have to
    pretend to be it, and so that the day a view needs a 3D viewport or a
    simulation, the door has a name, a handle and a shape rather than a
    word borrowed from iced.
11. **What it costs.** The renderer's bill is the draw-call split, which
    no core bench sees, so it has its own: `crates/kui-wgpu/benches/split.rs`
    draws this crate's own pipeline offscreen and splits it around 0, 1, 8,
    32 and 100 fragments. **Measured** in the second section at the end:
    about 0.6 µs of CPU per fragment and nothing measurable on the GPU, so
    what a fragment costs is its own fill. The core's bill — one more
    `NodeContent` arm and a side vector — is `frame_1k_typical` unchanged
    plus a `frame_1k_typical_with_8_fragments` beside it, and is still to
    be measured, because the core code does not exist yet.

## Considered options

- **Do nothing, and add gradients as props.** ADR 0005 already declined
  that, and the reasons have not moved: a stop list is a `Keyframes`-shaped
  parse in five bindings and every interpolation choice looks arbitrary.
  A fragment makes a gradient the app's five lines, in the app's own
  space, with no schema at all.
- **Ship iced's hatch and call it `shader`.** Rejected as the first thing
  built: it is a closure over the device, so it is Rust-only, invisible
  to the corpus, and inert headlessly — the opposite of every property the
  rest of the frame has. It is the `painter` of decision 10, under its own
  name, when a view needs what only it can do.
- **A full pipeline authored by the app** (vertex and fragment, its own
  instance layout). Rejected: geometry is a second vertex stream beside
  the quad list in every backend, and the C host gets nothing; and every
  app would reimplement the clip, the rounding and the opacity, each
  slightly wrong.
- **Params on the quad.** Rejected; C15. Sixty-four bytes on 20,000 quads
  a frame for a kind almost none of them are.
- **Params in a storage buffer indexed from the shader.** Rejected for the
  GLES-class backends wgpu still targets, where storage buffers are not a
  given; a uniform buffer with a dynamic offset is everywhere.
- **Validate lazily, in the renderer.** Rejected: the failure would be a
  blank box on the first frame in a window, reported through
  `RenderReport` a frame late, and a headless test would never see it.
  naga's front end is already in the dependency tree through wgpu; the
  core takes it directly with the `wgsl-in` feature and nothing else.
- **A dedicated texture per fragment now.** Deferred behind the atlas
  (decision 9): one bind group and the texture the renderer already has
  is what lets an image input land in a day.
- **Reserve `shader` for the painter.** Rejected: the data element is the
  first-class thing, and giving the plain word to the Rust-only one tells
  readers the opposite; and the painter is not a shader, it is a render
  callback, as egui's `PaintCallback` and gpui's custom element say
  plainly.

## Consequences

- **`Quad` is still 124 bytes**, `QuadKind` gains an eighth variant, and
  `DisplayList` gains a vector that is empty in every frame that draws no
  fragment. `KuiDrawData` appends two fields under ABI 9 and the `size`
  handshake keeps every older host whole.
- **The one-draw-call frame becomes one draw call per run.** A frame with
  no fragment is unchanged. A frame with ten is twenty-one draws, which is
  nothing; a grid of ten thousand fragment boxes is the wrong tool and the
  README's Status section says so beside the number.
- **Registration compiles.** The first frame that shows a new fragment
  pays a shader module and a pipeline build once per device. Measured
  below at about 0.2 ms together, not the "few milliseconds" this ADR
  assumed before it was run — with one 5 ms outlier in seven process runs,
  so the doc still says to register at startup. Idempotence by source is
  what makes the per-frame form safe, and it is worth having: the core's
  own validation is 55–73 µs, which a view calling `add_fragment` every
  frame would pay every frame.
- **A fragment's edge is a rectangle to input.** A circle drawn in a
  fragment is clicked in its corners; this is ADR 0010's decision 8 and
  the same rounded-hit-test change lifts both.
- **The corpus grows a scene and a column**, the JS decoder reads
  `uv[0]` as an index for this kind, and the C and Node adapters mix
  sixteen more words into the digest for it.
- **One new dependency in the core**, naga with WGSL input, pure Rust,
  already in the lock file. Measured below: about a quarter more on a cold
  build of the core alone, nothing on an incremental one, and nothing at
  all for any windowed binding, which already links this naga through
  wgpu. Not worth a feature gate; the section says why.
- **The painter has a name and no code.** `PainterId` stays reserved;
  `docs/BACKLOG.md` carries "painter" under design wanting an ADR, with
  decision 10 as the shape and "a 3D viewport, a simulation, a backdrop
  that a copy cannot serve" as what would justify writing it.
- **What this does not do:** multi-pass, geometry, compute, readback,
  text, a per-pixel hit test, an image input, a pointer input, named
  params, a backdrop, blend modes — each above with its reason and, where
  there is one, the change that adds it.

## Measurement: naga on the core (2026-09-08)

The question was whether validating WGSL in `kui-core` — rather than in the
renderer, a frame late — costs enough to want a feature gate. Measured on
the M3 Pro the bake-off ran on, `kui-core` built alone in a fresh target
directory, twice per configuration, with the dependency real: `naga = "30"`
with `default-features = false` and `wgsl-in`, and a
`crates/kui-core/src/fragment.rs` that parses and validates a source
between the prelude and the epilogue, with three tests, so nothing is
dead code the linker could drop. `cargo tree` counts distinct crates on the
normal edge.

| | before | with naga | |
|---|---|---|---|
| cold `--release`, real | 17.2 s, 16.6 s | 21.0 s, 21.6 s | **+4.4 s, +26%** |
| cold debug, real | 21.3 s, 22.7 s | 26.3 s, 29.3 s | **+5.8 s, +26%** |
| touch `lib.rs`, `--release` | 3.22 s | 3.25 s | unchanged |
| `libkui_core.rlib`, release | 4.54 MB | 4.87 MB | +334 KB, +7% |
| `libkui_core.rlib`, debug | 30.7 MB | 34.3 MB | +12% |
| crates in the tree | 47 | 72 | +25 |
| copies of naga in `Cargo.lock` | 1 | 1 | — |

The two cold runs of one configuration differ by 0.6 s in release and up
to 3 s in debug; the delta is four to seven times that.

Three things the table says:

- **The whole cost lands on a build of the core by itself.** Every
  windowed binding — the Rust runner, Lua, Node, the C `kui_run` — links
  wgpu, and wgpu already enables this naga's `wgsl-in` (and `msl-out`) on
  the one copy the lock file holds. The `Cargo.lock` diff is one line:
  `naga` added to `kui-core`'s dependency list. A shipped binary gains
  the validation module and nothing else.
- **The 25 crates are a proc-macro stack and an error printer**, not a
  compiler: `thiserror-impl` and `zerocopy-derive` bring `syn`,
  `proc-macro2` and `quote` (two `syn` majors, 2 and 3, resolved
  separately), and `codespan-reporting` with `unicode-width` is what
  `emit_to_string` uses to render the message with the app's line and a
  caret. The rest are `indexmap`, `bit-set`, `arrayvec`, `half` and their
  trivia. None is new to the workspace; `kui-core` alone simply did not
  build them before.
- **The incremental loop does not move.** A touched `lib.rs` rebuilds the
  core in the same 3.2 s, because naga is a finished artefact by then.
  What a developer of the core feels is the +5 s on a clean checkout and
  in CI's cold cache, which the workspace pays once per run anyway when
  it builds `kui-wgpu`.

**Decision on the gate: none.** A `fragment` feature would let a
headless-only consumer of the core save five seconds of cold build, at
the price of a second behaviour for `add_fragment` when the feature is
off — it cannot validate, so it either refuses every source or trusts
every source, and both are a warning code and a doc paragraph nobody
would read until the renderer failed. No consumer of the core is
headless-only today; the corpus adapters and the core's own tests are,
and they are the ones paying the five seconds, in CI, behind a cache.
The renderer's split — the other number decision 11 asks for — is still
unmeasured.

## Measurement: the renderer's draw-call split (2026-09-08)

The other number decision 11 asks for. `crates/kui-wgpu/benches/split.rs`
builds this crate's own pipeline — the same `shader.wgsl`, the same
descriptor, the same dual-source blend — draws a 10,000-quad frame into a
2560×1440 offscreen texture, and then draws the same frame as runs
interrupted by N fragment quads, each with its own pipeline and a
dynamic-offset uniform. The fragments are spread evenly through the quads,
which is the **worst case**: nothing batches, so N fragments cost 2N+1
draws. Same machine as the bake-off, best and median of 300 frames after 60
warm-up frames.

Three columns, because they answer different questions. **cpu** is upload +
encode + submit, which is what the runner records as `render_ms`.
**saturated** is 60 frames submitted back to back and waited on once, so
the GPU's own cost has nowhere to hide. A pixel is read back before the
table runs, and the bench asserts the gradient is actually there — without
it this would happily measure a pipeline that draws nothing.

### The split alone

Fragment boxes at 8×8, so their fill is negligible and what is left is the
state change: two draws, a pipeline set, two bind-group sets.

| fragments | draws | cpu best | cpu med | saturated |
|---|---|---|---|---|
| 0 | 1 | 0.118 ms | 0.150 | 0.257 ms |
| 1 | 3 | 0.122 | 0.143 | 0.259 |
| 8 | 17 | 0.128 | 0.157 | 0.260 |
| 32 | 65 | 0.132 | 0.167 | 0.262 |
| 100 | 201 | 0.176 | 0.204 | 0.264 |

**A hundred fragments cost 0.058 ms of CPU and 0.007 ms of GPU.** Per
fragment that is about 0.6 µs on the CPU and something too small to
separate from noise on the GPU. Against a 120 Hz budget of 8.33 ms, a
hundred fragment boxes is **0.7% of the frame**.

The same run at 1,000 quads gives 0.031 ms at zero fragments and 0.092 ms
at a hundred: **+0.061 ms, the same bill as at 10,000 quads**, which is the
check that the cost is per draw and not per quad. It also means the split
is a bigger *fraction* of a small frame — a hundred fragments triple the
encode cost of a 1k-quad frame — while remaining the same 0.7% of the
budget that matters.

### What a fragment actually costs: its own fill

The same table with the boxes at card size (320×180) and at the full
viewport, where the gradient is the whole bill:

| fragments | saturated, 8×8 | saturated, 320×180 | saturated, full viewport |
|---|---|---|---|
| 0 | 0.257 ms | 0.257 ms | 0.257 ms |
| 8 | 0.260 | 0.267 | 0.502 |
| 32 | 0.262 | 0.296 | 0.949 |
| 100 | 0.264 | 0.358 | 2.354 |

A hundred cards is +0.101 ms, of which the split is 0.007 and **fill is
0.094** — 5.8 M pixels of gradient, about 1.5 screens of overdraw. A
hundred full-viewport fragments is 369 M pixels and 2.35 ms, and that is
not the split failing, it is a hundred full-screen overdraws costing what a
hundred full-screen overdraws cost in any renderer. The doc comment says
this: **the split is free and the shader is the app's bill**, which is the
right way round, because the app can see its own fragment's area and
cannot see kui's draw calls.

### First sight

Distinct fragment sources, so nothing caches, measured before any drawing:

| | module | pipeline |
|---|---|---|
| first in the process | 0.06–0.12 ms | 0.13–0.21 ms |
| each after | 0.05–0.08 ms | 0.11–0.19 ms |

Five process runs. One earlier run produced a 5.1 ms first pipeline, which
is why decision 1's advice to register at startup stays even though the
typical number is a fifth of a millisecond. Beside it, the core's own naga
validation from the first measurement section: **55 µs** for a four-line
gradient, **57 µs** for a seven-line ring, **73 µs** for a ten-line noise
function. So a fragment is validated twice on its way to the screen, once
by the core at registration and once by wgpu at module creation, for about
0.13 ms total — and the second one is unavoidable, since wgpu validates
what it is given.

### What this measurement does not cover

- **The run-finding walk.** The bench is handed its runs precomputed; the
  real `render` has to notice a fragment quad while it walks `dl.quads`
  into instances. That is one comparison per quad on a loop that already
  exists, and it is unmeasured. It is the one part of decision 5 that
  costs something on frames with **no** fragments at all, so it is the
  first thing the implementation should bench.
- **Metal only.** One adapter, one backend. A backend where a pipeline
  switch is dearer — a tiler with a costly state change, or a driver that
  re-validates bind groups — would move the CPU column and possibly the
  GPU one. The bench runs anywhere wgpu does; the numbers above are the
  M3 Pro's.
- **Pipeline layout compatibility.** The fragment pipeline declares a
  second bind group, so its layout differs from the quad pipeline's, and
  the bench conservatively re-sets group 0 on every switch. Making the two
  layouts compatible would let that set be skipped. Not worth doing at
  0.6 µs, and worth remembering if a backend ever says otherwise.
- **Nothing about the core.** No `FragmentDraw` vector, no schema row, no
  binding. That code does not exist; decision 11 says what will measure it.

**Neither measurement disqualifies the design.** naga costs a cold build of
the core and nothing else; the split costs 0.6 µs a fragment and nothing
the GPU can see. What the numbers change in this ADR is one sentence — the
first-sight cost was overestimated by an order of magnitude — and what they
add is a ceiling worth writing in the README next to the element: a
hundred fragment boxes is free, and ten thousand is 20,001 draw calls and
the wrong tool.

## Amendment: what the building changed

Built the same day it was accepted. The design above held; seven things
changed, and one of them was a collision the draft could not have seen.

1. **`Fragment` in the JSX runtime was the string `'fragment'`.**
   `packages/kui/jsx-runtime.js` exported it as a string and `jsx()` spliced
   any element whose `type` matched, so `<fragment src=…/>` would have been
   silently treated as a `<>…</>` and had its children spliced into the
   parent — the element would have vanished with no error. It is now
   `Symbol.for('kui.jsx.fragment')`, which is what `jsx-runtime.d.ts` had
   always declared it to be (`unique symbol`), so the types were right and
   the implementation was wrong. This is the one breaking change the
   element's *name* forced, and it is in the changelog.

2. **`animate` is a plain schema row, not an element-own prop.** Decision 3
   listed it beside `src` and `params` as something each binding lowers
   itself. It is more useful and much cheaper as `PROPS` row 87 on any
   node: one `apply`, one `KuiSpec` field, four bindings for free, and it
   generalises to anything driven by the clock instead of by input. The
   builder sets `frame_requested`, which is the flag `animating()` already
   reports.

3. **A fragment opens like a box, so children work.** Decision 3 promised
   children; the draft's Rust signature `ui.fragment(id, params, spec)` had
   nowhere to put them. `open_with_key` is now `open_content(key, spec,
   content)` with the container path passing `NodeContent::Container`, so a
   fragment earns every `any_*` flag a box earns — clip, opacity, float,
   modal, slide — rather than a hand-copied subset that would have drifted.
   `Ui::fragment_with` and `Ui::fragment_with_keyed` take the closure;
   `kui_fragment_open` and `kui_close` are the C pair; Lua and JSX get
   children for nothing because their nodes are trees already.

4. **The WGSL contract lives in the core, and the renderer calls it.**
   `fragment::module_source(app)` is the one function that assembles
   prelude + app + epilogue, so the text the core validates at registration
   is character for character the text wgpu compiles. The prelude also
   declares `KuiGlobals`, which must describe the same bytes as
   `kui-wgpu`'s `Globals` — one buffer, two modules, and nothing at runtime
   would catch a drift because the size would still be right and only the
   numbers wrong. `globals_layout_matches` compares the two declarations
   field by field. `Globals` gained `time` and `scale` (and padding to the
   sixteen a uniform must be a multiple of), which is where a fragment's
   `time` comes from; `DisplayList` gained `time` to carry it there.

5. **The display list carries the source, not just the handle.** Decision 4
   gave the renderer a handle and sixteen floats; it also needs the WGSL
   the first time it sees a handle, and `Renderer::render` has no registry.
   `DisplayList::fragment_sources` is an `Arc<str>` per draw, beside
   `fragments` — a refcount bump per fragment quad, and neither vector
   exists on a frame that draws none. `FragmentDraw` stays `Copy` and
   digestible, which is what decision 6 needs.

6. **A departing fragment keeps painting, and needed a previous-frame
   list.** The draft said nothing about ghosts; `cells` is the precedent
   for declining them, and a departing `cells` draws as its box. A fragment
   usually *is* its box's paint, so a ghost that drew nothing would pop.
   `GhostContent::Fragment` holds the draw by value — the fixed sixteen is
   what makes that free — but the copy comes from the *previous* frame's
   list, because a ghost is cut out of the previous tree, so `FragmentList`
   has the same gated buffer swap `LineStore` and the text list have.

7. **The corpus report carries the parameters, because the digest cannot.**
   Decision 6 said the quad digest would mix the sixteen params. It cannot:
   the params are on the side list and `quad_digest` walks quads. A quad's
   `uv[0]` is only an index, and digesting an index pins the order and not
   the values. So the report grew one `fragment <i> <16 hex words>` line
   per draw — bits, so no adapter has to agree on how a float prints — and
   both mirrors grew a way to see the side list: `KuiDrawData.fragments`
   (which C needed anyway, decision 7) and `ctx.fragmentDraws()` in Node.
   The `fragments` scene then does what the corpus is for: four fragments,
   one with a child painted over it, one with a handle that is live in no
   session, and one with eighteen params so the truncation warning is
   pinned in four languages. All four adapters agreed on the first run that
   compiled, which is the expected result for a mechanically lowered row
   and not the reason to keep the scene.

**What did not change:** the name, the contract's shape, the one-function
rule, the sixteen positional floats, the side list, `Quad` at 124 bytes, the
draw-call split, the deferred list in decision 9, and the `painter` in
decision 10 — still named, still unbuilt, still the door for multi-pass,
geometry and compute.

**Verified in a window**, not only headlessly: `cargo run --example
fragments` draws the gradient, the ring, the shimmer and the card; three
accessibility presses on the button inside the card advance the ring, which
is input reaching a child painted over a fragment; and two captures a second
apart differ in the shimmer and are identical in the gradient, which is
`animate` driving the loop and `time` reaching the shader while a still
fragment stays still. The access tree lists the button and elides the
fragments, which is decision 3's derivation working: paint is not a control.
