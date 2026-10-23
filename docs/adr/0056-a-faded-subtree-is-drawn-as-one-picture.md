---
status: accepted
date: 2026-10-23
---

# A faded subtree is drawn as one picture

> **Accepted and built (2026-10-23); backlog V11.** From the group-opacity
> measurement that filed V10 and V11: `opacity` multiplied into every
> quad of a subtree, so where two of them overlapped the upper one
> blended over the lower one already faded. V10 took the node's own box
> out of its shadow; this takes the rest.

## Context

`opacity` on a node fades it and everything under it. The emitter did
that by multiplying the product of the opacities down the tree into the
alpha of every quad (`emit.rs`'s `fade`). That is right for one quad
and wrong wherever two overlap: a label on its own fill, a child box on
its card, two overlapping avatars. The upper one blends over the lower
one already faded, and what lies under the subtree shows through both:
the fill shows through the glyphs of its own label, and overlapping
circles show a lens where they meet. A browser, Flutter, Android and
Core Animation all draw such a subtree into a picture of its own and
fade the picture.

The common case is not a transition. Every stock control that is
disabled fades its whole subtree by the theme's `disabled_opacity` for
as long as it is disabled, so every disabled button's label was drawn
the wrong way.

ADR 0054 made the parts this needs: the renderer draws into a texture,
and draws a texture-backed image back. The C55 measurement priced a
picture drawn before the frame's own pass at about 5–12 µs of GPU and
15 µs of encoding.

## Decision

1. **The core brackets a faded subtree.** With the renderer's switch on
   (`Core::set_opacity_groups(true)`), a node whose own `opacity` is
   under 1 opens a group as the walk reaches it: a `QuadKind::GroupBegin`
   marker, then every quad its subtree emits in that walk faded only by
   what lies between it and the group, then a `QuadKind::GroupEnd`
   carrying the group's own fade, relative to the group around it. A
   group is closed by the first node the walk emits outside the node's
   subtree, so an `overSiblings` child painted after its siblings and a
   departing subtree painted under a live one land in the group they
   belong to (`runtime/groups.rs`).

2. **A float under the node is outside its group.** It is emitted with
   its own layer, later (ADR 0023), and keeps the fade on every quad by
   the whole of its opacity. A tooltip or a menu over a faded panel
   overlaps nothing of the panel's own.

3. **Not every group is worth a picture.** After the frame is emitted
   (and rounded spans are joined, which reshapes quads in place), each
   group's bounds come from what it drew, through each quad's clip and
   turn, in whole physical px inside the viewport. A group of fewer than
   two things drawn fades the same on its own, a group holding a
   backdrop blur would blur a picture with nothing under it, and a group
   with nothing on screen draws nothing: each is undone, its fade folded
   back into its quads and its markers made into quads that draw
   nothing. A group that is kept carries its bounds in both markers'
   `rect`, its fade in `color.a`, and the other marker's index in
   `uv[0]`.

4. **Off by default; the runner turns it on.** A host that draws the
   quads itself sees today's list until it asks. The native runner turns
   it on for every window it opens, in every binding; a headless driver
   turns it on to capture what a window shows (the book's pictures do).
   The door is `set_opacity_groups` in every binding but Lua, whose
   script is a guest in the host's frame.

5. **The renderer draws each group into a texture, deepest first,**
   before the frame's own pass: one pass per depth, each group through a
   viewport moved so its bounds land on its place in the texture, cut to
   that place with a scissor. Nothing about a quad is rewritten. The
   shader tests the clip against the fragment's place in the frame, a
   new varying (`screen`, the vertex's position before it becomes NDC),
   rather than `frag_pos`, which in a group's pass is the texture's
   place. Groups at even depths share one texture and odd depths the
   other, shelf-packed, so a pass never samples the texture it draws
   into: it reads only the pictures one depth deeper. The textures grow
   in steps of 256 px and are dropped by the first frame with no group.

6. **The end marker draws the picture** (`shade`, kind 8): the texel at
   the pixel's place in the frame moved by the group's offset, which is
   premultiplied (drawn over nothing), straightened for the blend, and
   faded once. Where the end marker sits in the run of the group around
   it, the picture is drawn into that group's picture.

7. **Glyphs in a group are alpha masks.** LCD coverage blends each
   channel against the colour behind it, and a picture over nothing has
   none. A device that blends per channel draws groups with the
   single-source variant of the quad pipeline, made the first time it
   draws one; elsewhere that is the frame's own pipeline already.

8. **Groups too many for the device's textures** are drawn as they would
   be with the switch off: each group's fade folded into the instances
   of its run, its picture drawn as nothing.

## Considered options

- **A fade per quad, as before.** Free, and wrong wherever pieces
  overlap, which a faded subtree of more than one quad almost always
  does: text lies on its own background.
- **Proving a group unneeded by overlap.** Android's
  `hasOverlappingRendering` and Chromium's render-surface rules skip a
  layer when no two pieces overlap. The cheap test here — two things
  drawn — keeps every case that matters (a label on its fill) and skips
  the one-quad case; an overlap test across a run is quadratic for
  little more.
- **A texture per group, at its own size.** No packing, and a texture
  made or found per group per frame. Two shared textures, packed, are
  made once and kept while groups are drawn.
- **Drawing each group at its place in a frame-sized texture.** No
  offset and no new varying, but groups at one depth that overlap on
  screen would draw into each other, and a frame-sized texture is 31 MB
  at 3456×2234 where the bounds of ten disabled buttons are a few.
- **Breaking the frame's pass at each group,** drawing the group into a
  texture and compositing it in place. C55 measured a broken pass at
  twice the cost of a picture drawn first, and every group breaks it
  twice.
- **Subpixel text inside a group** against a guessed backdrop (the
  group's own opaque fill, where it has one). Fragile, and grayscale is
  what a device without dual-source blending draws everywhere.
- **On by default in the core.** Every host renderer that predates the
  two kinds would draw a faded subtree unfaded.

## Consequences

- A disabled stock control, and any faded subtree in a window, looks as
  a browser draws it: the label sits on its fill and fades with it, and
  overlapping pieces show no seam. On a page darker than a control's
  fill, the label of a disabled control reads a little dimmer than it
  did, since the fill no longer shows through its glyphs.
- A group's text is grayscale, where the window draws subpixel text.
- Cost, measured on an M3 Pro on battery (`benches/groups.rs` in
  kui-wgpu): a grid of half-opacity cards, a fill and a label each,
  covering a 3456×2234 frame, groups off against on —

  | cards | emit | encode (`render_to`, CPU) | GPU per frame |
  | --- | --- | --- | --- |
  | 100 | 0.029 → 0.032 ms | 0.041 → 0.085 ms | 1.22 → 1.64 ms |
  | 1000 | 0.285 → 0.312 ms | 0.118 → 0.480 ms | 1.25 → 1.74 ms |

  The GPU's share goes with the area grouped, not the count: here the
  groups cover the frame, so it is a screenful drawn a second time and
  read back, about 0.45 ms. A disabled button is a sliver of that. The
  CPU's is about 0.4 µs a group to encode and 30 ns to emit. The frame
  benches with the switch off read as alpha.65's within the battery's
  noise (`frame_10k_rects` 758 against 765 µs, `deep_nesting_64_levels`
  83.7 against 80.7 µs, the guard green).
- The frame's walk pays nothing for groups unless the switch is on and
  the frame fades something.
- `docs/props.md`'s `opacity` row, `kui.h`, `NodeSpec::opacity` and
  `docs/status.md` say a faded subtree is one picture in a window.
