---
status: accepted
date: 2026-09-27
---

# A rounded background is joined by meeting

> **Accepted and built 2026-09-27**, the day it was proposed (backlog
> F101). Raised by kawoosh's rounded selection. The editor draws a
> selection as a translucent `bg` on the spans of each row it covers,
> and wanted it drawn as one rounded outline, the way an editor's
> selection usually is. Its first cut was a `fragment` under each row's
> text, told its own extent and its neighbours' by the app (the reason
> for a6092cc's square fragment and snapped fragment quad). That cut
> could not reach a markdown paragraph. A paragraph is one text whose
> lines wrap where layout breaks them, and a row of the view is not a
> line of the paragraph, so the app did not know the lines to draw
> under. And it was a frame behind, since the extents came from the last
> frame's layout.

## Context

- **What a span's background is.** One square quad per line a run of
  spans of one background covers (`build_decorations`, one rect per run
  since dfc635b), each edge on a whole pixel from where layout put the
  text (`on_pixels`). Two rows' backgrounds meet on one pixel line with
  nothing drawn twice. Only the corners were left to do.
- **What a rounded outline needs at a corner.** Where a line reaches
  past the line above, its corner is convex. Where it falls short, the
  corner is concave: a fillet past its end, filled in the line's own
  colour. So a line's corners depend on the lines above and below it.
  Those can be in the same text (a wrap), in another text (an editor's
  next row), or in another node beside it (the cell an editor draws for
  a line's newline).
- **When the lines are known.** A text's quads are emitted in its
  node's turn of `emit_frame`, in paint order, and at that point the
  node after it has not been laid out into quads. Every text's lines are
  known only once the frame's quads are all emitted, and that is still
  before the frame leaves the core. So a pass there sees every line,
  from the layout of the frame it draws.
- **What already draws a shape no quad kind has.** The polygon (ADR
  0025, decision 6) is a `fragment` quad painted by a WGSL source the
  core registers itself. Its params are the shape, and every backend
  that draws fragments draws it with nothing new.

## Decisions

1. **A span's background takes a radius, and nothing else is new on
   the surface.** `Span::bg_radius(r)` in Rust, `bgRadius` on a
   `<span>` in JSX, `bg_radius` in a Lua span table and on `KuiSpan` in
   C, in logical px. At 0, the default, the background is the square
   one it always was, byte for byte.
2. **Pieces are joined by meeting, not by a name.** Two rounded
   backgrounds belong to one shape when they have the same colour and
   radius and one's edge touches the other's exactly: on the line above
   or below, overlapping it sideways, or end to end on the same line. It
   does not matter which node drew either. Nothing groups them, so
   there is no id to declare, keep stable or clear, and two that touch
   are one. Meeting is exact because the pieces are on whole pixels
   already. Pieces that meet end to end on a line are made one extent
   first. That extent is what the lines above and below are told, and
   no corner is drawn where the pieces meet.
3. **The join is a pass after emission.** A rounded background is
   emitted square, like every background, and noted (`text::JoinBg`:
   its quad, radius, the clip its text was given, and its text's own
   box when that box clips, as a no-wrap line's does). At the end of
   `emit_frame`, `join::shape` finds each piece's neighbour above and
   below, the one overlapping it most, and rewrites its quad in place.
   Paint order and the quad count are unchanged, and no backend,
   adapter or wire learns a new kind. The shape uses the layout
   of the frame it is drawn in, so it is never a frame behind.
4. **Each piece is drawn by a stock fragment, `fragment::JOIN`.** The
   core registers it as it registers the polygon's. A piece's quad is as
   tall as its line and as wide as its extent plus the fillets its
   neighbours ask for. Its params are its own extent, the neighbours'
   extents, the radius and which neighbours there are, in physical px
   from the quad's left. A corner is convex where the piece reaches past
   its neighbour, a fillet where the neighbour reaches past it, square
   where the two end together, and round where nothing meets it. At a
   join the radius is at most half the step, and both pieces work it out
   from the same two ends, so the convex half above and the fillet below
   meet. The fill is the quad's colour. Near the ends and arcs it is
   sampled sixteen times a pixel, and elsewhere it is solid.
5. **A piece is clipped as its text's parent is.** A fillet past the
   end of a short no-wrap line lies outside that text's own box, beside
   it, so the piece takes the clip the text was given, not its own box.
   A piece is still no wider than its text's own box shows. An
   ancestor's clip, such as a scrolled pane, cuts the shape square where
   it cuts, as it cuts everything.

## Considered options

- **The app draws it (kawoosh's first cut).** A `fragment` a row, the
  extents from the last frame's layout. A frame behind on every edit
  and scroll, and blind to a wrapped paragraph's lines, which only
  layout knows. The square fragment and its snap (a6092cc) stay useful
  for other stacks. For a selection over text, this ADR replaces that
  cut.
- **A named group** (`bgGroup="sel"`). It would let two selections of
  one colour that touch stay apart. It also needs an id per selection,
  stable across frames, in four bindings. The pass would still have to
  find which pieces of a group are neighbours, and only geometry
  answers that. An editor's selections that touch are one selection.
  Kept in reserve, as an optional key on the same pass, if an app ever
  needs two touching shapes of one colour kept apart.
- **A new quad kind for rounded runs.** Every backend, the C header's
  `KUI_QUAD_*` and the Node decoder would learn it. The fragment path
  already carries sixteen params and a registered source, and the
  polygon showed that path is enough.
- **Joining in layout.** Layout sizes boxes and does not know a text's
  line rects in physical px, which is what the join compares. That
  knowledge would move into layout for a paint effect.

## Consequences

- The first paint that reads across nodes. The pass costs a scan of
  the frame's rounded pieces against each other. That is the pieces on
  screen, a few dozen for a selection, and nothing when no span has a
  radius.
- A rounded background is a `KUI_QUAD_FRAGMENT` on the wire, and its
  `color` carries the fill with its alpha, not only the group opacity
  as a node's fragment quad does. A host that draws fragments itself
  gets the source from `kui_fragment_source`, as for the polygon. A host
  that draws no fragments draws no rounded background, where it drew a
  square one.
- Two touching selections in one colour and radius are one shape, by
  design (decision 2).
- The C `KuiSpan` gains `bg_radius` under the unreleased ABI 20, and the
  Node frame goes to v17, one slot a span.
- Pinned by `tests/joined_backgrounds.rs` in kui-core (the pieces and
  their params across texts, a wrap, colours and radii that do not
  join, pieces meeting end to end on a line) and in kui-wgpu (the shape
  composited through `JOIN`'s mirror, `tests/wgsl::join_alpha`), and by
  the corpus's `joined-backgrounds` scene in four adapters.
