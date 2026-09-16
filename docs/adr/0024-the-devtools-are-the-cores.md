---
status: accepted
date: 2026-09-11
---

# The devtools are the core's: one panel, drawn by the runtime, for every app

> **Accepted (2026-09-11), built the same day**, with what the building
> changed under [*What the building changed*](#what-the-building-changed).
> Out of the examples' dock (ADR 0021) having become useful and being
> reachable by exactly one kind of program: a Rust example that wraps
> itself in `kui_devtools::Harness`. The Node examples have a twin of it,
> `examples/node/devtools.tsx`, written a second time in TypeScript, and
> the two have already drifted. A Node *app* — not an example — that wants
> to see its own event stream has to copy one of them.
>
> This ADR moves the panel into `kui-core`, where every binding already
> is. **The core draws the dock into the frame itself**, beside whatever
> the host built, the way it already draws the context menu; **it takes
> the dock's events back before they leave `handle_input`**, the way it
> takes the menu's; **it logs every event it does hand out**; and its
> state lives in the session, so the panel can be **docked in the main
> window or popped out into a window of its own**. One door per binding
> turns it on (`Core::set_devtools`, `setDevtools`, `kui_set_devtools`),
> and `KUI_DEVTOOLS=1` in the environment turns it on for a program that
> was never told about it. The two example harnesses keep their CLI and
> their `Example` trait and lose the thousand lines they each had of
> panel.

## Context

- **What the dock is, today.** `examples/devtools/src/lib.rs` is 2,000
  lines: a `Harness<E: Example>` that is the `App` the runner sees. Its
  `view` builds a row (or column) with the example's tree in one cell and
  the dock in the other, and its `on_event` acts on the dock's clicks,
  matches the `Ctrl+Shift+<letter>` chords, logs everything else into a
  `VecDeque<Entry>` and passes it on. Every fact the dock shows comes
  from a `Core` door that already exists — `nodes()`, `focus()`,
  `region()`, `modifiers()`, `windows()`, `warnings_raised()`,
  `theme_source()`, `env()` — and every control it offers calls one:
  `set_theme_source`, `set_native_menus`, `set_inspect`, `focus_region`.
  Nothing in it needs to be outside the core except the one thing it
  cannot see from inside: the events the host is handed.
- **The core already builds nodes of its own into the host's frame.**
  `Ui::finish` calls `Core::build_menu`, which opens the drawn context
  menu with a `Ui` over the core itself (`runtime/menu_api.rs:176`), and
  `handle_input` calls `consume_menu_events`, which takes the rows'
  clicks back out of the batch before the host sees them
  (`menu_api.rs:287`). The drawn menu bar does the same. The pattern —
  build at `finish`, consume at `handle_input`, keep the keys so the two
  agree — is exactly what a dock needs, and it is written twice already.
- **The core sees every event.** `handle_input` returns the batch; the
  driver routes it. A stream logged at that return, plus the frame's own
  pending events at `take_pending_events`, is complete by construction,
  which the harness's never was — the harness saw what reached the
  example and nothing an extension or another window was handed.
- **Isolation is by origin.** A hit region carries the `OriginId` of the
  node that declared it, and an event carries the region's
  (`input.rs:566`). A reserved origin for the dock's nodes makes "is this
  the dock's?" one compare, with no list of keys to keep in step, and
  `Extensions::route` hands an unknown origin to the host rather than
  panicking (`slot.rs:340`) — so an event that slipped past would be a
  visible oddity and not a crash.
- **Keys are derived from the parent chain**, and a node's auto key is
  `parent.index(n)` (`builder.rs:180`). Wrapping the host's tree in a
  container moves every one of its keys unless something says otherwise;
  ADR 0014 built the thing that says otherwise, `ns_depth` / `ns_key`,
  so a slot fill's children are keyed by the slot and not by what the
  host built around it. The same two fields, set at the container, key
  the host's children as if the container were not there.
- **A window is a `Core`**, and the session is what the cores share
  (`session.rs`). A panel in its own window is built by that window's
  core, from state the main window's core wrote — the way the window
  registry already crosses that line. What the driver does for a window
  it did not ask for is nothing special: an `Open` command is an `Open`
  command, and the pane it opens calls the host's `view` like any other.
- **The harness's keyboard rules** (ADR 0022, decision 8; the `step`
  field) exist because the harness's root sink held focus after a press
  on dead space and then had to hand Tab back to the ring. A core that
  matches its chords *before* routing a key press needs no sink for
  them, and the whole of that machinery goes.
- **What the by-hand rounds asked for** (this branch's brief): the event
  stream as a virtual list whose rows open into a viewer of the payload;
  a picker that finds a node in the tree from the app, the way a browser's
  inspector does; more of the node in the inspector; a collapsible tree;
  and the panel in a window of its own.

## Decisions

1. **The panel is `kui_core::devtools`, and it is on when a door says
   so.** `Core::set_devtools(bool)`; Node's `ctx.setDevtools(on)` /
   `win.setDevtools(on)`; C's `kui_set_devtools(ctx, bool)`. Lua is an
   extension language and has no host of its own, so its host's door is
   its door. The windowed runners also read `KUI_DEVTOOLS`
   (`Core::devtools_from_env`, before the first frame — *built:* not
   `Core::new`, so a headless core never grows a dock from a variable
   left exported): `1`, `true`, `side`,
   `bottom`, `window` turn it on (the last three say where), so a Node
   app run as `KUI_DEVTOOLS=1 node app.mjs` has the panel with no code.
   Off by default and off costs one bool per hook. Behind a `devtools`
   cargo feature of `kui-core`, on by default like `accesskit` — a kiosk
   build turns it off and the doors become no-ops.
2. **The main window's core wraps the host's tree.** With the panel
   docked (side or bottom), `begin_frame` opens an **app container**
   under the root before the host draws, at the same key namespace as
   the root (`ns_key = Key::ROOT`), so `Key::ROOT.str("x")` and
   `Key::ROOT.index(n)` name what they named. The host's
   `configure_root(spec)` is **split**: what lays out and paints its
   children — direction, padding, gap, alignment, wrap, clip, scroll,
   background, border, radius, transition — goes to the container, whose
   width and height are forced to grow into what the dock leaves; what
   is *addressed* — `on_key`, `on_click`, `on_hover`, `on_layout`,
   `modal`, `focusable`, `hoverable`, `cursor`, the access rows — stays
   on the root, so a root key sink still hears keys on `Key::ROOT` and a
   root `on_layout` still reports the window. `Ui::finish` closes the
   container (a host that left nodes open is truncated, as
   `finish_frame` already does) and opens the dock as the root's next
   child. Popped out or hidden, the root is not wrapped and the frame is
   what it always was. *(Built: and the host's viewport is what the dock
   leaves — see What the building changed, 12.)*
3. **Everything the devtools declare is under one key and one origin.**
   The dock, the outline floats and the picker's overlay are children of
   `Key::ROOT.str("kui-devtools")`, opened under `OriginId::DEVTOOLS`
   (`u16::MAX`). `Core::nodes()` lists them like any node — a tool that
   reads the tree sees the truth — and the tree tab skips that subtree
   and folds the app container out (its children shown at the root's
   depth plus one). Their labels are prefixed `kui-devtools/` so
   `key_of("filter")` still finds the app's field and never raises
   `ambiguous-key` against the panel's.
4. **`handle_input` acts on the panel before the host hears anything.**
   In order: a `KeyDown` that is one of the chords (`Ctrl+Shift` +
   `T A M D C N I P`) or `Escape` while picking is acted on and the batch
   is empty; every event whose origin is `DEVTOOLS` is acted on and
   dropped; a `window` event about the devtools' own window is dropped;
   what remains is **logged** and returned. `take_pending_events` logs
   too, so the stream has the `resize`, `system` and hover-from-layout
   events a frame raises. Nothing the host declared is ever consumed,
   and the host never sees a chord — the same family the harness kept.
5. **State is the session's.** `SessionState::devtools` holds what every
   core reads and one core writes: the switch, the dock placement, the
   tab, the stream (a ring of `STREAM_CAP` entries, each with its frame,
   window, key, label at the time, origin and the payload *as a
   `Value`*), the theme and menu overrides, the selection, the collapsed
   set, the filters, the picker, the legend, and the main window's last
   **facts** and **nodes** snapshot. A core takes the state out of the
   session for the length of a build and puts it back, so the panel's
   own text measuring can borrow the session underneath.
6. **The panel has four placements** (five, after the building: `left`,
   `right`, `bottom`, `window`, `off`, with a button each — see *What the
   building changed*, 11): `side`, `bottom`, `window`, `off`
   — `Ctrl+Shift+D` walks them in that order, and `off` is the panel
   hidden with the chords still live, which is how `Ctrl+Shift+D` brings
   it back. `window` has the main core declare `"kui-devtools"`, a
   `Normal` window, under the devtools origin, every frame it is wanted;
   the driver opens it like any declared window. **In that window's core
   the root is not opened until `finish`**, so every node the host's
   `view` builds there is a no-op (the builder ignores a frame with no
   root), the extensions are not asked to fill it, and the panel is the
   whole tree. Closing the window sets the placement to `off` — the user
   closed it — and the `closed` event never reaches the host. The
   panel's window reads the *main* window's facts and nodes from the
   session, written at the end of the main core's `finish_frame`; the
   outline over a selected node is drawn by the main core, whichever
   window the row was clicked in.
7. **A window may ask another to redraw.** `WindowCommand::Redraw(id)`,
   the sixth command. The devtools window's core pushes `Redraw(MAIN)`
   when a row is hovered or picked there, and any core that logs an
   event while the panel is a window pushes `Redraw(devtools)` — the
   stream should move when the app is clicked. The runner applies it as
   `request_redraw`; a C host that ignores an unknown kind has a panel
   that catches up on its next input, and nothing worse.
8. **The events tab is a `virtual_rows` list with a viewer.** Every
   entry is one line — the frame, the window when not the main, the
   node's label or key, the payload flattened — and a click opens it to
   its payload as an indented tree of `key: value` lines, nested maps
   and lists included, and closes it again. Heights are exact and kept
   in step with the entries (a line per row, plus one per line of an
   open payload), so `RowHeights` never estimates. A filter field
   narrows the list by substring over the line; **pause** stops logging
   without clearing; **follow** keeps the newest row in view and is what
   a wheel up turns off. Warnings and the panel's own notes are entries
   of their kinds, as before.
9. **The tree tab is collapsible, filterable, and has a picker.** A row
   with children carries a disclosure; a collapsed row shows how many it
   hides; `expand all` / `collapse all` are one click. The filter matches
   label, kind, role, text and flags, and shows a match with its
   ancestors (dimmed) so the path is legible. The **picker**
   (`Ctrl+Shift+P`, or the crosshair) lays a float over the app's area;
   while it is up the deepest node under the pointer is outlined and
   named in a badge, a click selects it — expanding its ancestors and
   scrolling the list to its row — and `Escape` or a second press on the
   crosshair leaves. "Deepest" is the last node in paint order whose
   rect contains the point and whose scrolling or clipping ancestors all
   do too, floats above flow by their layer's rank — which `NodeInfo`
   now carries.
10. **`NodeInfo` says more.** Beside what it had: `layer` (0 in flow,
    else the float layer's rank from the bottom), `origin`, `children`,
    `padding`, `gap`, `main_align`, `cross_align`, `wrap`, `min_w`,
    `min_h`, `max_w`, `max_h`, `radius`, `border_w`, `border_color`,
    `opacity`, `scroll` (the offset, for a scroller) and `events` — each
    handler the node declared with its payload, so the inspector can
    say what a click on it would post. Node's `nodes()` rows grow the
    same fields; C exposes no `NodeInfo`, so nothing moves in the ABI.
    The inspector shows them in groups — identity, box, layout, paint,
    events, state (hovered, pressed, focused, in the region) — with the
    parent as a row that selects it and the ancestors as a breadcrumb
    that does the same.
11. **The harnesses keep what is theirs and drop the rest.**
    `kui_devtools` (Rust) keeps `Example`, `Cli`, `run`, `main!`,
    `Drive` and a `Harness` that is now a title and two calls; its
    `--dock`, `--light`, `--dark`, `--accent` flags reach the panel
    through `set_devtools_dock` and `set_devtools_theme`, and
    `Example::KEYS` through `set_devtools_legend`. `examples/node/
    devtools.tsx` keeps the same CLI and becomes `setDevtools` plus the
    same three. The smoke scripts run unchanged; the `KUI_SMOKE_FRAMES`
    counter the header shows is read by the core now.

## What was declined, and why

- **A `Devtools` struct in the `kui` crate, behind a feature.** The
  first answer, and Rust-only: Node, C and Lua could not see it, and
  the TypeScript twin would have stayed. The core is where every
  binding already is.
- **The panel as an extension (ADR 0014).** An extension draws in a
  slot and hears its own events; it cannot see the host's stream, and
  only a Rust host loads one. Two of the three tabs would work and the
  bindings would get none of it.
- **A per-driver "skip the host's view for the devtools window"
  branch.** Four drivers, four ifs, and a C host's loop is the host's
  own code. Deferring the root push in that window's core does the same
  in one place and needs no driver to know.
- **Keyboard walking of the tree rows (arrows, Left/Right to
  collapse).** Wanted, and not in this round: a key sink per row is the
  wrong shape, and one sink for the list with a cursor of its own is a
  composite pattern (ADR 0007) that deserves its own reading. Filed.
- **A `zIndex`-aware hit test through the core's own `target_at`.** It
  answers only for hit regions, and a picker wants every node. The
  layer rank on `NodeInfo` gets the ordinary cases right; the
  `float_stack` is the same order the core paints, so it can be made
  exact later without changing what the picker asks.

## Consequences

- Any program on any binding can open the panel, with a call or an
  environment variable, and what it sees is the same panel.
- The harnesses shrink by ~1,900 lines between them and stop drifting.
- `kui-core` grows a module of roughly the size the harness lost. Off,
  it is a bool per hook — `begin_frame`, `configure_root`,
  `Ui::finish`, `handle_input`, `take_pending_events`, `finish_frame`'s
  end — and a feature turns even that off.
- `WindowCommand` gains a variant; ADR 0006's rule says a new constant
  is not a bump, and `KuiWindowCommand`'s layout is untouched.
- The tree tab's rows and the outline use `NodeInfo` from the frame
  before, as they always did; the picker's overlay is one more float in
  the frames it is up.
- `Core::nodes()` includes the panel. A tool that read it to count the
  app's nodes should skip `key_of("kui-devtools")`'s subtree, as the
  tree tab does.

## What the building changed

1. **The module is `runtime/devtools`, not `devtools`.** Every hook
   reads `Core`'s private fields — the tree, the stack, the session, the
   namespace pair — and those are private to `runtime`, where every
   other `impl Core` child lives (S4). `kui_core::devtools` re-exports
   it; nothing outside sees the move.
2. **A builder door on an empty tree derives no key and indexes no
   label.** `parent_key` indexed `tree.keys[0]` before `open_content`
   checked for a frame, which the deferred root of decision 6 made a
   panic on the first `open` in the panel's window; and `open_keyed`
   pushed its label whether or not a node was opened, so `key_of` in
   that window answered for nodes that were never built. Both guarded.
3. **The dock's placement `off` still declares one node**: a zero-size
   holder under `kui-devtools`, so the outlines and the picker have a
   parent whatever the placement, and everything the panel declares
   stays under one key (decision 3). `key_of("kui-devtools")` answers
   in every placement; `kui-devtools/stream` only when docked.
4. **The theme override remembers the app's own source.** Cycling the
   base back to "app" restores what the core had before the first
   override, rather than leaving the last pinned palette in place; the
   panel's own window mirrors the main window's app source through the
   facts when no override is in force. Amended 2026-09-12 (backlog F51):
   the core keeps the app's source *beside the override it applied*
   (`dt_theme`), so a source the app sets under the override is told
   from it and is what the override is lifted back to; and the override
   keeps the half it leaves alone from the *app's* source — "the app's
   base" is the base the app chose, which for a pinned palette is not
   the OS's. Before that an accent override went out as
   `DerivedWithAccent`, and a pinned-dark app on a light desktop flipped
   light on `Ctrl+Shift+A`.
5. **A text node is not picked.** The picker's "deepest node" skips
   `NodeKind::Text`: a text node's box is its parent's business, and
   picking the label of a button selected the label.
6. **Stream row heights are measured, not pre-set.** `virtual_rows`
   drops every cached height when the container's width first arrives,
   which threw away heights set at build time; the measure closure is
   the arithmetic now — exact for every row it is asked about, which is
   what decision 8 meant.
7. **The hovered row in the panel's own window asks the main window to
   draw.** A hover is styling, not an event, so nothing queued a
   `Redraw(MAIN)` for it; the tree tab marks the change and the build
   pushes one (decision 7's third case).
8. **The chords are matched on `KeyDown`, and the picker's `Escape` on
   both channels.** A driver sends a press as the raw `KeyDown` and, for
   an editor key, `InputEvent::Key(EditKey)` too; Tab walks the ring on
   the second, which the tests had to send to see it, and `Escape`
   while picking is taken on either so neither reaches a modal.
9. **`Launcher::setup_core` and `Launcher::devtools`.** The harness
   opens the doors before the first frame, and the launcher had no place
   for what a core is told rather than declared; a general hook, and the
   one-word sugar an app wants.
10. **The tree tab's count is the app's.** "13 of 136 nodes" counted the
    panel's own; the total skips the `kui-devtools` subtree and the
    container, and the root's row is named `root`.
11. **A button per placement, a left dock, and a handle.** The round
    on the built thing asked for the placements as buttons rather than
    one cycling icon, and for a docked pane that can be resized. So
    `Dock` is `Left | Right | Bottom | Window | Off` (`"side"` still
    parses as the right), the tab row ends in five radio-style buttons
    — left, right, bottom, undock, close — the current one lit, and
    `Ctrl+Shift+D` walks the same list. A docked pane's inner edge is a
    6 px handle with `on_drag`; `devtools_consume` reads the drag's
    pointer and sets the session's `side_w` / `bottom_h` (floors of 220
    and 120 px, and the app keeps 160). A **left** dock precedes the app
    in the root row, so it is built at `begin_frame` before the container
    (from the facts the last frame left — the title is not declared yet
    at that point) and `finish` skips it (`dt_built`).
12. **The host's viewport is the window less the dock.** The round on
    the built thing asked for it: an app should hear a `resize` when the
    dock comes, moves or is dragged, and think of its viewport as what is
    left. So the core keeps `dt_area`, the host's viewport in window
    coordinates, computed at the top of `begin_frame` (before the resize
    check, which now compares areas); `Core::viewport()` answers its
    size; `Tree::host_area` is what a host node's `FloatConfig::viewport()`
    float resolves and `fit`-clamps against (a devtools node's still uses
    the window); and under a **left** dock, where the area's origin is
    not the window's, every coordinate that crosses the boundary is
    translated — drag, layout, context-menu and force-click payloads on
    the way out (`devtools_translate`), `cursor()`, `caret_rect`,
    `scroll_geometry` likewise, and `text_hit`, `open_menu` and a
    popup's anchor on the way in. `NodeInfo` rects stay in window
    coordinates: they are the panel's, and the picker reads them
    against the window's pointer. Right and bottom docks have a zero
    origin, so for them this is size alone.
13. **A pane has a floor the window cannot squeeze past.** `SIDE_MIN_W`
    280 and `BOTTOM_MIN_H` 160: the handle clamps to them, the pane's
    node declares them as `min_width` / `min_height`, and `pane_w` /
    `pane_h` give the area and the pane the same number — the pane's
    floor wins over the app's 160 px when the window has room for
    neither.
14. **A placement chosen in the panel's own window used to hide the
    panel.** The main window stopped declaring the window, the diff
    closed it, and the `closed` event was read as the user closing it —
    `Dock::Off`. It is the user's only while the panel is still
    `Window` when the close arrives.
15. **`KUI_DEVTOOLS` is the launcher's to read, not the session's.**
    Read at `SessionState::new`, a variable left exported would have put
    the dock into every headless core — the conformance corpus and every
    `createApp` test with it. `Core::devtools_from_env` is what
    `Launcher::shell` calls before the first frame, and Node's and C's
    windowed paths go through the same launcher.
16. **The header holds the placements; the tab row the toggles; the
    picker sits before the find field**, as `⊕ pick`. *Amended
    2026-09-16 (backlog F72): the toggles left the tab row for the Facts
    rows they change — `theme`, `accent`, `menus` each carry a
    `widgets::select` of the choices — and the strip wraps whole tabs.* Real icons for all
    of them were backlog D2a, built 2026-09-11: `runtime/devtools/icons.rs`
    draws each from the core's own vocabulary — a `line` per stroke, a
    `polygon` per fill, a zero-length segment for a dot — in a 16-px box
    in the theme's colours, the lit placement's pane filled in the accent.
17. **`kui_devtools::Harness::new` takes two arguments**, and the crate
    is 630 lines from 2,048; `examples/node/devtools.tsx` is 203 from
    599. The four Rust examples that returned `Dock::Off` / `Bottom`
    compile unchanged through a re-export.
18. **The panel paints the accent as ink, and holds it to a contrast**
    (2026-09-12, backlog F50). The stock widgets fill with the accent
    under `on_accent`; the panel strokes, borders and labels with it,
    and an OS accent near the base — Windows' automatic accent off a
    dark wallpaper — was invisible. `panel::ink` moves the accent toward
    the front of the base until it clears 3:1 on `surface`, the way
    `Theme::ring_for` does for the ring, and leaves one that reads
    alone; the accent swatch shows the accent in force inside a
    hairline, and the facts row prints it.
19. **The panel's own window has the OS's chrome** (2026-09-12, backlog
    F52). The runner gave every window the launcher's chrome; under
    `Chrome::Custom` the panel's opened undecorated with nothing drawing
    a titlebar into it. `WindowCommand::Open` carries the origin, and
    a window declared under `OriginId::DEVTOOLS` opens native; chrome is
    a `Pane`'s, not the shell's.
20. **`pick` from the panel's window focuses the main window**
    (2026-09-12, backlog F53), the mirror of `inspect`'s `focus_window`.
21. **The window's floor counts the dock** (2026-09-12, backlog F54).
    Decision 1's "the pane squeezes the app" is right for a window the
    user shrank past both floors and wrong for the floor the app asked
    the OS to hold: the launcher's `min_size` reached the OS once, and
    the dock came out of the app's share. `Core::devtools_inset()` is
    what a docked pane takes, in its axis, and the runner adds it to the
    floor after every main-window frame, on change.
22. **The inspect chord is the app's to respell** (2026-09-15). Decision
    4's family is `Ctrl+Shift+<letter>`, and an app whose keymap wants
    `Ctrl+Shift+I` had no way to move the panel off it. The one chord an
    app names in its own help — the way in — is a door now:
    `Core::set_devtools_key(Accel)` (`Launcher::devtools_key`,
    `setDevtoolsKey`, `kui_set_devtools_key`), read back by
    `devtools_key` in `Accel::spelling`'s portable form. The other seven
    stay where they are: they are reached once the keyboard is in, so
    they collide with nothing. A chord the app takes is the app's for
    good — `chord()` asks the configured `Accel` first and no longer
    knows the letter `I` — and the facts row names the chord in force.

## Action items — all done 2026-09-11

- [x] `kui-core`: `devtools` module, feature, session state, hooks, the
      `DEVTOOLS` origin, `Core::cursor`, `WindowCommand::Redraw`,
      `NodeInfo` fields, doors.
- [x] `kui` runner: apply `Redraw`; `Launcher::setup_core` / `devtools`.
- [x] Node: `setDevtools` / `setDevtoolsDock` / `setDevtoolsTheme` /
      `setDevtoolsLegend` on `Ctx` and `KuiWindow`, the new `nodes()`
      fields, `runWindowed` not asking the app's `view` for the devtools'
      window, `devtools.tsx` reduced.
- [x] C: `kui_set_devtools`, `kui_set_devtools_dock`, `KUI_CMD_REDRAW`,
      header and parity.
- [x] `examples/devtools`: reduced to the harness; tests moved into the
      core.
- [x] Docs: CHANGELOG, READMEs, BACKLOG (E2 closed, D1 filed).
