---
status: accepted
date: 2026-10-27
---

# Paint stays in the box it is in

> **Accepted and built (2026-10-27); backlog C151–C154.** The fifth
> report of one bug: Kit's live line chart drew over the window's bar
> and smeared into its y gutter. A float anchored to its parent escaped
> every clip unless it said `clip`, so each component that floated
> something had to remember to say it, and each one that forgot was a
> report. The default turns over: what a node paints is cut by the
> nearest box above it that clips or scrolls, and a float that must
> leave says `escape`. A paint audit checks the rule over the whole Kit
> gallery.

## Context

The reports, in order: Kit's slider handle drew over the app's bar; a
checkbox's focus ring escaped its scroller (C123 made `clip` take the
clip a child takes, not the parent's own rect); the skeleton's shimmer
band escaped; the chart's readout painted under the slide layers (a
paint-order bug, ADR 0053); and now the live line chart, whose lines,
area and x tick labels drew above the plot, over the card above and
the window's bar, and left of the plot into its y gutter as the window
slid. Four of the five were one mechanism, each patched in the
component by adding `clip = true`.

What lets a node's paint leave its ancestors' clips, read from the
code at d900a911 (`runtime/emit.rs` unless named):

1. **A float anchored to its parent without `clip`.** The walk gave it
   `Clip::NONE` (`emit.rs:1272`, the `floats_here && !drawn_in_parent`
   arm): every ancestor's clip and scroll rect dropped. `clip` was off
   by default (`spec.rs:392`, `:410`); `FloatConfig::clipped_by_parent`
   (`spec.rs:494`) read it. The chart's sliding box was such a float
   (`charts.tl:682`, `float = { anchor = "parent" }`) and so was every
   stroke and polygon in it, which take their parent's clip — the
   sliding box's, which was none. Its x tick labels said
   `clip = slide == 0` (`charts.tl:756`): off exactly while sliding.
   The y range glides 240 ms behind the data, so the line runs past the
   plot's top and foot before any escape; the slide reaches a sample
   back past the plot's left edge; nothing cut either.
2. **A float anchored to the viewport or to a node** leaves by design:
   it is placed against something other than its parent.
3. **A departing subtree** (an `exit`) was drawn outside every clip
   (`emit.rs:1975–1980`: the ghost's root took `Clip::NONE`, its
   ancestors "may be gone"): a row sliding out of a scroller drew over
   the bar beside it.

And what does not leave, checked: a node moved by `dx`/`dy`
(`move_declared`, `:2293`), by a keyframe stop (`offset_stops`,
`:2244`) or by `slide` (`ease_positions`, `:2139`) is moved in
`tree.pos` before the walk, and the walk cuts it by its clippers' rects
as laid; a turn or a scale (`:1330`, `display.rs` `Clip::turned_by`)
keeps the outer clip and adds an inner one; an `over_siblings` subtree
is painted late from the per-node clips the walk already wrote
(`flush_over_siblings`, `:1636`; `emit_stored`, `:1714`); floats in
tree order (`group_in_tree_order`, `:4309`) only reorder layers; the
focus ring (`:2735`, `:2754`) and a scroller's bars (`:2460`) use their
node's clip. Those were held already; only 1 and 3 leave without
saying so.

## Decision

1. **The rule.** What a node and its subtree paint — quads, glyphs,
   masks, shadows, its focus ring and its bars — is cut by the nearest
   box above it that clips or scrolls, whatever its float, move,
   keyframes, turn or sibling order, unless the node declares it leaves.
   Its hit region is cut with it, as the walk already did for a clipped
   float.
2. **Who leaves.** A float anchored to the viewport or to a node (as
   before), and a float anchored to its parent that says **`escape`**:
   a menu, a popover, a tooltip, a select's list. `FloatConfig::escape`
   in Rust (`.escaping()`), `escape` in the float table in Lua and JSX,
   `float_escape` appended to `KuiSpec` (ABI 39, into the tail padding;
   888 bytes still), `Float.escape` in Odin, and bit 4 of the Node float
   stanza's flags word (protocol 27). The core's own floats that must
   leave say it: the hint under a hovered control, a submenu, the menu
   bar's panel, the devtools' "⋮" menu. A `line`, `polygon` or `path`
   in its parent's box never escapes.
3. **`clip` on a float** is what a parent-anchored float does now.
   Lua and JSX still read the key and drop it, C keeps `float_clip` in
   its place and reads it no more, and Rust's `FloatConfig::clipped()`
   is deprecated and does nothing; the Rust and Odin structs lose the
   field (a typed literal naming it is the one compile error).
4. **A departing subtree keeps the clip it was held by** on the frame
   it left (`depart::held_by`): its root is cut where it was, so an
   exit plays inside the scroller it left from. Under a turn it is
   drawn as before, unclipped.
5. **The paint audit** (`Core::set_paint_audit`, `KUI_PAINT_AUDIT=1`
   under kui-native's drivers) checks the rule from outside the walk:
   emission notes which node each run of quads belongs to, and at the
   frame's end every quad, cut by the clip it names, is compared with
   the rect the boxes above its node leave it — by what the node
   declares, never by what the walk did — raising `paint-escape` per
   node that drew outside. A headless run prints its counts. Off by
   default; the walk pays one predictable branch a node for it.

## Considered options

- **Keep `clip` opt-in and fix the chart.** Four components fixed that
  way already; the fifth is this report. Every new float is a chance to
  forget, and the code reviews cannot see a missing key.
- **Cut viewport floats too.** A toast, a dialog's scrim and the
  devtools' panels are placed against the window: the box they sit in
  in the tree is not where they are, and cutting them by it would draw
  nothing.
- **A tri-state `clip` (unset, true, false)** in place of a new name.
  The bindings OR booleans into presets (`FloatConfig::build`), and the
  C struct and the Node flags word have no "unset" for a bool; `escape`
  says what it does.
- **An audit that reads the walk's own clips.** It would agree with
  the walk by construction. Reading the declared floats and the laid
  rects catches a walk that is wrong, which is the bug.

## Consequences

- The chart's sliding layer, its axis labels and every other float a
  component hangs off a child are cut by the box they are in, without a
  key. Kit drops `clip` from thirteen of its floats and says `escape` on
  its menus, popovers, the select's list, its dialogs and toasts.
- **What breaks:** a float anchored to its parent (`parent`, `below`,
  `above`) that showed past a clipping or scrolling ancestor — a menu,
  a popover, a tooltip of an app's own — is now cut there, and not hit
  past it, until it says `escape`. A row's exit slides out under its
  scroller's edge. Rust's `FloatConfig { clip, .. }` and Odin's
  `Float{clip = ..}` no longer compile.
- The gallery walk `nothing_the_kit_app_draws_leaves_the_box_it_is_in`
  (kui-cli, at scale 1 and 1.5) fails on the old rule — the chart's
  slide, its axis labels, the menu and the popover — and passes on
  this one; `tests/clip_rule.rs` checks each mechanism.
