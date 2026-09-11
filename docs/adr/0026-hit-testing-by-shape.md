---
status: accepted
date: 2026-09-11
---

# Hit-testing by shape: a region's rect, then its outline

> **Accepted (2026-09-11), built the same day.** ADR 0010 declined to
> settle this and named the change; ADR 0025's `polygon` made it due — a
> pie's wedges share a bounding box, and the example that shipped floated
> a hover box over each wedge because the wedge itself could not be hit.
> Two storage options were built and measured; the one the draft
> predicted would cost 7% cost nothing, and won on simplicity.

Hit-testing has been rectangular since the first commit: a `HitRegion` is
a rect and a clip, and `hit_at` is `rect.contains(p)`. ADR 0010 gave a
`line` no input at all rather than a rectangular approximation of a
diagonal, and wrote down what would change it: *"a `HitRegion` that
carries a shape and a `hit_at` that evaluates the SDF instead of
`rect.contains` — built once, for both, when a view wants to click a
connector or is bitten by a rounded corner."* ADR 0025 carried the same
rule over to `polygon`, and the README has said since ADR 0005 that a
click in the corner of a rounded scroll container still reaches the row
under it.

We are giving every hit region a **shape** — the plain rect it always
was, a rounded rect, a stroke's pieces, or a fill's outline — and making
the one point-in-region test every input path runs evaluate it after the
rect. A `line` or a `polygon` that declares input is hit by its stroke or
its outline and derives an access role as a box would; one that declares
none is elided as before. The two `*-ignores-input` warnings go, because
nothing is ignored any more.

## Context

- **One door.** `Interaction::hit_at`, `refresh_hover` and `target_at`
  are the three readers of the hit list (`crates/kui-core/src/input.rs`),
  and everything — hover, press, click, drag start, the cursor shape, the
  secondary press — goes through them. One `contains` covers every path,
  which is what makes this a small change with a wide effect.
- **The view that wants it.** `examples/rust/widgets/polygon.rs` shipped
  with a `hoverable` box floated over each wedge's middle, because a
  wedge's bounding box overlaps its neighbours' and the topmost-region
  rule would light whichever was declared last. A fill is the natural
  click target a stroke never was: a pie, a map's regions, a node
  editor's ports, a treemap.
- **The geometry is already in the frame.** A polygon's normalised
  vertices are its `FragmentDraw.params`; a line's flattened pieces are
  in the `LineStore`; a box's radii are on its style. None outlives the
  frame, and a press lands between frames, so what a region needs has to
  be copied out at emission — for the regions that ask, which is the
  few that declare input.
- **`HitRegion` is 304 bytes** and is built once per hover-tracked node
  per frame (2,500 of them in `frame_10k_rects_with_text_and_hits`, every
  one with a 2 px radius). C15's rule is that a field added to a
  per-node struct is paid forever, so where the shape lives is the
  question this ADR measures rather than assumes.
- **Access.** `access::derived_role` returned `None` for a line or a
  polygon before anything else was asked, so an `on_click` on one could
  not make it a button. With a shape hit-test that early return is
  wrong: a clickable wedge *is* a button, and a screen reader should
  hear it.

## Decisions

1. **`HitRegion::shape: HitShape`**, an enum inline on the region:
   `Rect` (what every region was), `Rounded([f32; 4])` (the node's
   radii, logical px), `Segments { first, len, width }` and `Polygon {
   first, len }` — the last two indexing `Interaction::shape_points`, a
   `Vec<Vec2>` of points relative to the region's top-left in logical
   px, rebuilt with the hits and empty on a frame of plain boxes.
2. **One `contains`.** `rect.contains(p) && clip.contains(p)` first, as
   before, then the shape: a rounded rect excludes a point in a corner's
   square past its arc (radii clamped to the half extents, as the shader
   clamps them, so an oversized radius is the pill it draws as); a stroke
   hits within half its width of any piece — **at least
   `MIN_STROKE_GRAB` (4 px) wide**, so a hairline is a target the way a
   1 px splitter's handle is wider than its line; a fill hits by the
   even-odd crossing test. All three are analytic and take the rect's
   fast reject first, so the scan every pointer move pays is the rect
   test on every region and the shape on the few under the pointer.
3. **Emission builds the shape** where it builds the region
   (`runtime/emit.rs`): a `line`'s run copied from the `LineStore`, a
   `polygon`'s eight vertices denormalised from its params, a
   `Rounded` for any region whose style has a radius (an `edit` field's
   included), `Rect` otherwise. The `is_line` exclusion that kept strokes
   and fills out of the hit list is gone: they get a region when they
   are hover-tracked, exactly as a box does.
4. **A line or polygon with input derives a role** as a box would
   (`access::derived_role` no longer returns early for them): `on_click`
   is a button, `on_key` or `focusable` a group. Without input they are
   elided as before. So a clickable wedge wants a `label`, and the
   corpus scenes give theirs one.
5. **The two warnings go.** `line-ignores-input` and
   `polygon-ignores-input` said the declaration did nothing; now it
   does, and the codes are removed from `diag`, the `WARNINGS` table and
   the generated union.
6. **The C doors take the payloads.** `kui_polyline` and `kui_polygon`
   gain `on_click`, `on_drag`, `on_hover` as `kui_open_with` takes them;
   `kui_line` stays the two-point convenience with none. Both are in the
   unreleased ABI 14, so the signature change rides that bump.
7. **The corpus pins it in four bindings.** `lines` and `polygon` each
   gain two presses: on the stroke / inside the outline (an event), and
   in the bounding box off the shape (nothing) — and their clickable
   nodes gain labels and so access rows.

## Options considered, and measured

Two places the shape could live were both built and run on the same
machine (M3 Pro, `cargo bench -p kui-core --bench frame`):

| variant | `frame_10k_rects_with_text_and_hits` | `hover_over_10k_regions` |
|---|---|---|
| base, before this ADR | 1.181 ms | — |
| **B** — `shape: u32` index into a side `Vec<HitShape>`, `NO_SHAPE` sentinel | 1.197 ms | 3.62 µs |
| **A** — `shape: HitShape` inline (chosen) | 1.199 ms | 3.71 µs |

- **B, the side list**, was the draft's choice: `HitRegion` stays 304
  bytes and only shaped regions pay an entry. Measured against A it was
  within noise on both benches (the ±3% floor is 36 µs on the build
  bench; the difference is 2). It costs an id, a sentinel, a second
  list and an indirection on every test, for a saving nothing can see.
- **A, inline**, grows the struct by the enum (about 20 bytes) on every
  region, which the draft priced at +7% and the bench does not show:
  the struct is already 304 bytes and is written once and read by a
  linear scan, so twenty more is not where the time is. Chosen.
- **`hover_over_10k_regions`** is the whole scan on a pointer move —
  2,500 rounded, clickable regions, the hovered node changing every
  move — at 3.6 µs, one and a half nanoseconds a region. The shape test
  runs only for the regions whose rect passed, which is one or two per
  move; it is not measurable inside that.
- **The build cost** is +16 µs on a frame that makes 2,500 shaped
  regions: the `Rounded` copy of four floats each. A frame of square
  boxes pays nothing new.
- **An SDF instead of analytic tests.** The shader's rounded-box
  distance transcribed to the CPU would round corners the same way the
  paint does; the analytic corner test is the same set to within the
  antialiasing ramp and needs no transcription to keep in sync. A
  stroke's capsule *is* the segment distance, so that one is the SDF.
- **Winding instead of even-odd for the fill.** This draft believed the
  paint filled by winding and argued even-odd was an acceptable
  mismatch; the review after the build read the stock WGSL again — its
  sign flips on *every* edge crossing, which is parity — so the paint is
  even-odd too, and the hit test is the fill exactly, a self-intersecting
  outline's unfilled overlaps included. Had they differed, the crossing
  test would have taken a winding counter in place of a boolean; they do
  not, and it does not.
- **Copying points versus referencing the frame's stores.** The line
  store and the fragment list are the frame's and are swapped at the
  next `begin_frame`; input arrives between frames. Referencing them
  would have tied `Interaction`'s lifetime to the core's stores;
  copying costs a `Vec` extend for the strokes and fills that declare
  input, which is a handful.
- **A minimum grab of zero.** Rejected: a 1 px connector with a 1 px
  grab is a target nobody can hit, and every drawing tool pads a
  hairline's hit width. 4 px total is the smallest that reads as
  hittable at 1×; it does not scale with `env.scale` because it is in
  logical px, as every other size here is.

## Consequences

- **What you can delete:** the hover or click box floated over a wedge,
  a connector or a shape's middle to make it a target — the polygon
  example's own, and any app's; and the `role="none"` or the absent
  label that kept a "button" that could not be pressed out of a screen
  reader's way.
- **Two warning codes are gone** from the union: a match on
  `'line-ignores-input'` or `'polygon-ignores-input'` no longer
  compiles.
- **`HitRegion` has a new public field**, so a host that constructs
  regions by hand — the tests in `input.rs` are the only such code —
  adds `shape: HitShape::Rect`. A host that only reads `hits()` (the
  Windows non-client hit test) sees a rect that still means what it
  did, and may read the shape.
- **A line or a polygon with input is in the access tree**, so an
  access-row snapshot of a view whose connectors take clicks grows by
  those rows, and they want names.
- **The README's standing caveat** — a click in a rounded corner reaches
  the row under it — becomes what the paint always looked like.
- **Not done here:** a `cells` grid's cell-level shape (a grid is
  rectangular), text glyph hit-testing (the editor has its own), and
  hit-testing a fragment by its alpha (a fragment's edge is its box,
  and only a readback could say otherwise — the painter's domain).

## Action items — all done 2026-09-11

1. [x] `HitShape`, `contains`, the three point-in-shape functions with
   their own tests (`tests/hit.rs`).
2. [x] Emission builds the shapes; the `is_line` exclusion removed.
3. [x] `derived_role` reaches strokes and fills; the two warnings
   removed; `WARNINGS`, `props.md`, `index.d.ts` regenerated.
4. [x] `kui_polyline` / `kui_polygon` take the payloads; header updated.
5. [x] Corpus `lines` and `polygon` scenes: labels, presses on and off
   the shape, in four adapters.
6. [x] The polygon example loses its hover boxes and its drive asserts
   the outline.
7. [x] Both storage variants benched; `hover_over_10k_regions` added.
