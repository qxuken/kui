---
status: accepted
date: 2026-10-25
---

# An `svg` element

> **Accepted and built (2026-10-25); backlog V12.** Kit's icons, a
> brand's logo and a chart's legend glyphs are SVG files, and every binding
> had to turn one into `path`s by hand. What the building changed is at
> the end.

An `svg` element draws an SVG document in a box. When every shape is one
kui's `path` draws exactly, it lowers to a box holding one `path` per
shape — crisp at any size and scale, recoloured live by `color`. When one
is not, it lowers to one image the document is rasterized into at the
drawn size and scale. Either way it is made of nodes the core already
has, so it adds nothing for a backend to learn.

## Context

- `path` (ADR 0040) draws any outline a `d` string spells, filled and
  stroked with round caps and joins, dashed (V2), turned (ADR 0041). An
  icon set's file is a handful of those plus the document around them:
  a viewBox, `<circle>`, `<rect rx>`, `<g transform>`, `currentColor`.
- Everything else SVG can say — gradients, clips, masks, filters, text,
  embedded pictures, butt and square caps, miter joins — no `path` draws.
- Lucide (ISC), which Kit's icon tool pulls, is 24×24 viewBoxes stroked
  `currentColor`, width 2, round caps and joins: all of it a `path` draws.
- `usvg` parses and normalizes a document into absolute paths in user
  space (shapes, transforms, `use`, styles resolved); `resvg` rasterizes
  that tree with `tiny-skia`, which kui-core already links (0.11).

## Decision

1. **One element, `svg`, everywhere.** Rows of its own: `data` (the
   markup) or `src` (a file); `mode`; `fit`. `color` is the text-colour
   row it already is: what `currentColor` paints, the theme's `fg` when
   unset, so an icon takes the ink of the text beside it. Every other row
   is the box's — it lays out, clips, fades, takes input and has a tooltip
   like any box.
2. **Paths when it can, an image when it cannot.** `mode`:
   - `auto` (the default) draws paths when every shape is one a `path`
     draws exactly, and an image otherwise;
   - `paths` always draws paths, approximating what a path cannot draw
     (a gradient's first stop, a clip left out) and raising
     `svg-approximated` once with what it changed;
   - `image` always rasterizes.

   What sends `auto` to an image: a gradient or pattern paint, a clip, a
   mask, a filter, an embedded picture, text, a group opacity over more
   than one child, a butt or square cap on an open subpath, a join that is
   not round, a dash of more than two pairs.
3. **It lowers onto nodes that exist.** The element is a box with the
   spec; inside, a `path` per shape (its fill in `bg`, its stroke in the
   path's stroke) floated where the viewBox puts it, or one `image` the
   size of the box. Replay (ADR 0045) journals what the doors push, so a
   path-drawn svg in a kept slot replays; an image-drawn one, or one sized
   by its layout, marks the fill unkept.
4. **`currentColor` is resolved in the core.** The document is parsed
   once with a style sheet that sets the root's `color` to a sentinel no
   design uses; a paint of that colour is the element's `color` at draw
   time, so recolouring costs nothing. A translucent `color` over several
   shapes draws them opaque in a box faded by the colour's alpha (ADR
   0056's group), so overlapping strokes do not darken where they cross.
5. **Sizing.** No width or height: the document's own size. One axis
   given: the other keeps the aspect. `grow` or a percentage: the size the
   box was laid out at last frame, and a frame owed when that changes.
   `fit` places the viewBox: `contain` (whole, centred; the default),
   `cover` (filled, clipped), `fill` (stretched).
6. **Caches.** Documents are parsed once by content hash; a `src` file is
   stat'ed once a frame and read again when its size or modification time
   moves, so a picture an editor saves redraws. Rasters are shared by
   content, size, scale, fit and colour, so a list of a thousand identical
   icons holds one; they are swept with the core's by-last-use cadence and
   early past 64.
7. **Access.** The box is an image to assistive technology unless it
   takes a click (then it derives as a button, as a clickable path does)
   or declares a role; `label` names it, `role="none"` makes it
   decoration, and an unnamed one raises `image-without-label`.
8. **No I/O but the one file.** `usvg` is built without its text and
   system-font features and given no resources folder: an `<image>` the
   document names by path is left out; only a `data:` one is drawn.
9. **Doors.**
   - Rust: `Ui::svg(SvgSource, SvgOpts, spec)`, `Ui::svg_keyed`;
     `Content::Svg` through `Core::open_from`; `kui_core::svg::Svg` for a
     host that wants the parse itself (`shapes`, `raster`).
   - Node: `<svg data src mode fit color>` (wire v26, op `svg`).
   - Lua: `svg { data=, src=, mode=, fit=, color= }`, a relative `src`
     read from the script's folder.
   - C: `kui_svg(ctx, label, data, src, mode, fit, color, spec,
     on_click)` with `KUI_SVG_AUTO…` and `KUI_SVG_CONTAIN…`.
   - Odin: `kui.svg`, generated, with `Svg_Mode` and `Svg_Fit`.

## Considered options

- **A node kind of its own.** Every backend, the hit tester, the access
  tree and the devtools would learn it, for what a box of paths already
  is.
- **Always rasterize.** Simplest, but an icon would be a texture per size
  and colour, blurred under a transform and re-uploaded on every theme
  flip; the common case is exactly what `path` draws crisply.
- **Convert in each binding.** Four parsers, four sets of bugs; the
  conversion belongs where `path` already is.
- **Kit's icon tool converts ahead of time and nothing else.** The tool
  does convert (it emits `d` strings), but an app with a logo file has no
  tool in between; both use this element's core.

## Consequences

- kui-core depends on `usvg` and `resvg` 0.46 (no default features),
  sharing the `tiny-skia` it had, behind a default-on `svg` feature: they
  add 1.10 MB to the release `libkui_ffi` (19.17 → 20.28 MB on macOS
  arm64). Built without it, the element and its doors stay and every
  document is refused with `svg-malformed`.
- A new parsing surface, fuzzed: target `svg` (parse, shapes, raster, and
  the element under each mode in a frame), seeded with an icon, a
  gradient and a dashed group. Its first five minutes found a panic in
  tiny-skia's anti-aliased scan converter on a stroke reaching `8e8`
  units; a picture that reaches past 30,000 device px is now drawn as
  nothing, with `svg-too-large` (regression `huge-rotated-stroke`).
- Four warning codes: `svg-malformed`, `svg-unreadable`,
  `svg-approximated`, `svg-too-large`.
- A conformance scene, `svg`, in all five adapters; the element is
  `UNDERIVED`, since the tree holds a box and paths, not an `svg`.

## What the building changed

- The draft had a `NodeContent::Svg`; lowering onto `open`, `path` and
  `image` made it unnecessary (decision 3).
- A clickable svg was an image with a click; it is the button a clickable
  path is (decision 7), which the conformance scene's access tree pins.
- A laid-out size is read from the core's own record of the box's last
  size, not from `layout_of`, which only reports keys that declared
  `onLayout`.
