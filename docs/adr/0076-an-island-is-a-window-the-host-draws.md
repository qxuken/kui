---
status: accepted
date: 2026-10-28
---

# An island is a window the host draws

> **Accepted and built (2026-10-28), backlog F236.** Settled by a
> grilling round with the user on F236 (with F237), after F233–F235,
> F237's door, F238 and F239 were built for Replicator Bloom. It answers
> F236 with a kind of declared window rather than a row on a subtree, and
> closes F237's atlas half. Built the same day as written: decisions 1–7
> needed no change.

## Context

- Replicator Bloom draws kui over a game world. Its wishlist (item 5)
  wants UI *in* that world: a tag on the gate ring, warped by the
  camera's perspective, behind the swarm. The host needs that subtree's
  quads apart, drawn into a texture of its own at a size of its own, and
  the pointer taken in the subtree's coordinates once the host has
  un-projected it.
- The backlog entry sketched a `surface` row: the subtree laid out at its
  own size in the one context, its quads a separate range of the one
  `KuiDrawData` with its own origin and bounds, and a
  `kui_input_cursor_in(ctx, surface, x, y)`. That is a layout pass for
  surface roots, emission split into ranges, ranges in the draw data (an
  ABI change), and a hit space per surface beside the window's.
- kui already draws subtrees in targets of their own: declared windows
  (ADR 0004). A view declares a window by name (`windows` in every
  binding); a host without the runner hears `KUI_CMD_OPEN` with the
  window's id and config through `kui_take_window_command`, runs a context
  for it that builds the same view with `env.window.id` set to that id,
  and draws that context's draw data. Since F237's door, that context
  can join the main one's session (`kui_ctx_new_in`): the same fonts,
  images, sounds and shaping caches.
- `KuiWindowConfig.kind` already has a second kind: `KUI_WINDOW_KIND_POPUP`,
  a menu surface placed against its owner and closed with it. An unknown
  kind degrades to a normal window with a warning.
- Every context has its own glyph atlas (ADR 0004 step 1 hoisted fonts,
  images and sounds into the session and left the atlas: `Core::output`
  hands it out beside the list it was drawn for, and `CachedText` keys
  its runs by the atlas's epoch).

## Decision

1. **An island is a declared window of a new kind,
   `KUI_WINDOW_KIND_SURFACE`** (Rust `WindowKind::Surface`, `kind:
   "surface"` in the `windows` row). It is never an OS window: the host
   draws it, wherever and however its world wants. Everything a declared
   window has, it has: an id, `on_window` opened and closed, its own
   context and draw data and bounds (F235), its own frame in the one
   view, owned by the window that declares it and closed when that one
   closes (as a popup is).
2. **The host runs it as it runs any declared window**: on
   `KUI_CMD_OPEN` with a surface kind, a context made with
   `kui_ctx_new_in(main)`, `kui_env_set_window` with the id, built each
   turn at the size the declaration says and a scale the host picks —
   its texture's pixels per logical px, larger for a surface the camera
   comes close to. Its draw data is drawn into the host's texture with
   kui's shader and packing (F234), and the host maps that texture into
   its world.
3. **Its size follows its declaration.** A normal window's config is
   read once, because the user owns its geometry after it opens; nobody
   drags a surface, so its width and height are read every frame it is
   declared, and a change arrives as `KUI_CMD_SET_SIZE` for the host to
   resize its texture. Placement is the host's: nothing in the config
   says where in the world it goes.
4. **Input goes in through its context**, in its own logical px: the
   host un-projects the pointer onto the surface (a ray against the gate
   ring) and calls `kui_input_cursor` and the rest on the surface's
   context, or `kui_input_cursor_left` when the ray misses. `kui_hit`,
   `kui_pointer_wants` and `kui_keyboard_wants` (F233) answer per
   context, so the host asks the main one first, then the surface under
   the pointer, then gives the input to the world.
5. **Keys are the host's to route, by a documented rule and no new
   door**: to the context the last press landed in — a surface's, while
   its `kui_keyboard_wants` says so — and to the main one otherwise.
   Focus is per context, so a surface is a Tab ring of its own.
6. **The windowed runner does not open one.** `kui_run`, Rust's
   `Launcher` and Node's `runWindowed` have no world to draw a surface
   into: a declared surface raises a `surface-without-host` warning once
   and opens nothing, and the rest of the app runs.
7. **The atlas stays per context; F237's atlas half closes.** An
   island's atlas holds only the glyphs it drew, so its upload is small;
   sharing one atlas across a session means taking it out of
   `Core::output` and re-keying `CachedText`, a large refactor for a cost
   nobody has measured. Reopened if a measurement shows one.

## Consequences

- No layout, emission or hit-testing change in the core, and no change
  to `KuiDrawData`: the work is the kind (schema, `KuiWindowConfig`,
  every binding's `windows` row), the size that follows the declaration,
  the runner's warning, kui.h's host loop saying how to run one, and a
  test that drives a surface headless — declared, opened, built at its
  size in a context of the main one's session, clicked in its own
  coordinates, resized by its declaration, closed with its owner.
- State between the HUD and an island travels through the app's model,
  as it does between windows: a click on the ring is a message, and both
  frames read the model. A focus ring, a selection or a drag does not
  cross from one to the other.
- A surface costs a core: its own tree, layout and atlas. A game with a
  handful of islands pays a handful; one with hundreds of world labels
  would want them in one surface laid out as floats, or in the main
  layer pinned with `float: viewport` as Replicator Bloom's tags already
  are.
- The reference hosts (F234) draw one context's draw data into a
  texture; an island is that, with a world transform the host owns.
- A screen reader does not reach an island (backlog F269). The
  accessibility bridge a host attaches (`kui_access_attach`) is the main
  context's, on the host's window; an island's context is not bridged,
  because its nodes' rects are in the host's world and have no place in
  the window until the host projects them. Bridging one later takes the
  host giving the four corners of the island's quad in window px each
  frame, and the bridge grafting the island's tree under a node of the
  main one with its rects projected through them.

## Considered and turned down

- **A `surface` row in one context** (the backlog's sketch): shared focus
  and atlas, declared inline beside the HUD. Turned down for the core
  work it needs — a layout pass per surface root, ranged emission, a hit
  space per surface, ranges in the draw data — when declared windows give
  each piece already, and for the ABI change in `KuiDrawData`.
- **Opening a surface as a normal window under the runner**, to debug an
  island's UI without the game: a surface's size and placement are the
  host's world's, and an OS window of that size at the OS's place shows
  something the game never draws. The warning says what happened.
- **The core forwarding keys to a focused surface**, as a popup's owner
  routes its keys: a focus handoff between contexts in the core, when the
  host already knows which surface it last pressed.
- **One atlas per session now** (ADR 0004 step 2): see decision 7.
