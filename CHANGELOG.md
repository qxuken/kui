# Changelog

Every release lists what it adds and, separately, **what you can delete**:
the workaround, the model field or the arithmetic the release made
unnecessary. The second list is the point of the first — a library whose
upgrades remove code from the apps on it is doing the job.

## 0.1.0-alpha.5 (unreleased)

### Added

- **Accessibility as data** (`docs/adr/0001-accessibility-as-data.md`).
  Two props on every node in every binding, `role` and `label` (with
  `checked` for checkbox / radio / switch roles and `valueNow` /
  `valueMin` / `valueMax` for a slider; `role="none"` hides decoration),
  and the core derives an **access tree** from them and from what nodes
  already do: an `onClick` box is a button named by the text inside it,
  an editor a text input carrying its text and caret, a scrolling box a
  scroll view, the root the window named by `title`, window chrome its
  buttons and title bar; the `tooltip` prop is the description. Plain
  boxes are elided, so ten thousand rects yield a handful of nodes.
  `Core::access_tree()` (`ctx.accessTree()` / `app.accessTree()` in Node,
  `kui_access_tree` in C) returns it as data, hashed so a driver sends
  it on only when it changed. Requests from assistive technology come
  back in as `InputEvent::Access` (`ctx.access(key, action, value)`,
  `kui_input_access`) and resolve the way pointer input would: a click
  emits the node's payload, focus lands on an editor, `setValue` writes
  it and raises `changed`, scroll requests move the scroll view; a slider
  nudge reaches the app as `{ kind: "access", action, tag }`. The Rust
  runner (and so `runWindowed`) hands the tree to screen readers through
  AccessKit (UIA, NSAccessibility, AT-SPI; cargo feature `accesskit`, on
  by default) and costs nothing until one attaches. Missing names are
  warnings: `image-without-label`, `control-without-name`.
- **Text a screen reader can walk.** An editor's access node carries its
  laid-out lines as *runs* (every character's byte length, position and
  width, word starts, the trailing newline as a zero-width character),
  and its caret and selection as positions in them, so a reader moves by
  character, word and line and hears where the caret went. Two more
  requests, `setTextSelection` and `replaceSelectedText`, resolve in the
  core for the built-in editors (`app.access(key, 'setTextSelection', {
  anchor, focus })`, `kui_input_access_text`). An app that owns its text
  gets the same tree: `role="multilineTextInput"` on its `onKey` sink,
  `role="line"` on each row it draws (the text nodes inside are that
  line; a `role="none"` gutter does not count), `caret` /
  `selectionAnchor` byte offsets on the lines holding them — and the text
  requests it alone can honour arrive as `{ kind: "access", action,
  text, anchor: { line, offset }, focus }` messages. The modal editor
  example is wired up this way. A key sink with no role is now a
  focusable group instead of nothing.
- **Text measurement as a query.** `Core::measure_text` and
  `measure_rich_text` (`ui.measure_text` in Rust, `ctx.measureText` /
  `win.measureText(content, style, maxWidth)` in Node, `env.measure_text(s,
  opts, max_w)` in Lua, `kui_measure_text` / `kui_measure_rich_text` in C)
  return `{ width, height, lines }` in logical px: what layout gives a text
  node with that content and style, wrapped to a width when asked, `wrap` /
  `maxLines` / `ellipsis` included. Works before the first frame and shapes
  through the same cache, so measuring a string and then drawing it shapes
  once.
- **Layout as data.** An `onLayout` tag (`on_layout` in Rust and Lua,
  `KuiSpec.on_layout` in C) brings the node's laid-out rect back as
  `{ kind: "layout", x, y, w, h, parent, tag }` on its first frame and
  whenever it changes — never on a frame that left it alone. Rects are
  final (after scrolling and `slide` easing) in viewport coordinates;
  `parent` rides along as on drags.
- **Diagnostics as data.** The core notices three misconfigurations that
  used to fail silently and raises each distinct (code, node) pair once as
  a `Warning { code, key, message }`: `grow-weight-ignored` (a grow weight
  with nothing to split against: the only grow child, or across the
  parent's main axis), `transition-auto-key` (a node's child count changed
  while an unkeyed child carries a transition, so the shifted children
  snapped), `duplicate-key` (two nodes on one key in a frame).
  `Core::take_warnings` drains them; the Rust runner and `runWindowed`
  print them; `createApp` collects them on `app.warnings` (and prints
  unless `warnings: false`); C drains `kui_take_warnings`. The checks run
  on the first two frames and every 16th after (a persisting condition
  surfaces within that; measured cost otherwise ~10 ns per node per
  frame). A development aid: the Rust runner runs them in debug builds
  (`Launcher::diagnostics` overrides), the Node loops unless
  `NODE_ENV=production` (`diagnostics` option overrides), a standalone C
  context only after `kui_set_diagnostics(ctx, true)`; a bare `Core` /
  `Ctx` has them on for headless tests.
- **Null tags.** `onDrag`, `onHover`, `onKey` and `onLayout` are `Tag`
  rows in the schema: `null` declares the behaviour with no `tag` on the
  events (JSX `onKey={null}`, C `kui_value_null()`), and the generated TS
  props type them `AppMsg | null`.
- The tick re-render contract is documented next to the `tick` option.

### Changed

- `KuiMsg`: the docs now say to register only the app's own messages.
  `CoreMsg` is typed in terms of the registration, so registering
  `MyMsg | CoreMsg` made the alias circular.
- `KuiSpec` gained `on_layout` (appended; a zeroed struct means what it
  meant).

### What you can delete

- **The "we'll do accessibility later" ticket.** A screen reader sees the
  buttons, editors and lists a view already declares; what it cannot
  name, the warnings list by node.
- **A hand-run screen-reader pass** to know a control is reachable:
  `app.access(key, 'click')` in a headless test emits what the pointer
  would, and `app.accessTree()` is what the reader would see.
- **The "our editor is a canvas to screen readers" caveat.** A row role,
  two offsets, and the buffer stays yours.
- **Breakpoint tables found by screenshot.** Digit-cell widths, elbow
  widths, "does this label fit at this tier": measure the strings in the
  style you draw them and compare against the width you have. The numbers
  follow the font.
- **Geometry re-derived in `update`** to place something against a node:
  read the node's `layout` event.
- **An inert message in the union** so a root key sink has something to
  say: `onKey={null}`.
- **A regression report** for the lone grow child that ignored its
  weight: it now says so itself.

## 0.1.0-alpha.4 (2026-09-03)

### Added

- Node messages typed as the app's own union: `KuiMsg` registration,
  `CoreMsg` with every core payload spelled out, `createApp` /
  `runWindowed` inferring the union from `update`.
- A captured drag stays pressed while the cursor is off the node.
- Window size as a `resize` event and as `win.size()`, answerable before
  the first frame; minimum and maximum window size as launch options.
- CSS-style `keyframes` on transitions, with `repeat` and `delay`.
- Line breaking on text: `wrap`, `maxLines`, `ellipsis`.
- Entrance transitions (`enter`) and frame timing on `KuiWindow`.
- Audio as data: sound resources, playback commands, the `<audio>` tag,
  `clickSound` / `hoverSound`.
- `props.md` ships in the npm tarball.

### What you can delete

- **A pressed flag in the model** for a drag strip that must stay lit
  while the cursor leaves it: `pressedBg` holds through the captured drag.
- **A cascade's model field, its tick computation and its change-detection
  line**: `keyframes` plus `delay={i * n}` per sibling, and nothing wakes
  up to flip a target.
- **Per-tier label strings** swapped in so a header never wraps:
  `wrap="none"` with `ellipsis` clips it instead.
- **A staging frame for a slide-in** (draw off-screen, request a frame,
  draw in place): `enter={{ dx }}`.
- **A stored window size** kept in sync by hand: it arrives as a `resize`
  message and `win.size()` answers before the first frame.
- **Casts and "is this an object" preambles in `update`**: annotate it
  with `MyMsg | CoreMsg` and switch on `msg.kind`.

## 0.1.0-alpha.3 (2026-09-02)

### Added

- Declarative hover: `hoverBg`, `pressedBg`, `hoverGroup`, `onHover`
  events; `<button>` is that data.
- Per-corner radii.
- Fonts as registered resources: `addFont`, `addSystemFont`,
  `loadFontFile`, `loadFontsDir`.
- Schema-driven docs: `docs/props.md` generated from the prop table.

### What you can delete

- **`isHovered` queries in the view** and the re-render they forced:
  `hoverBg` / `pressedBg` resolve in the core, and ease with `transition`.
- **Two boxes glued together** to round a tab or an elbow: one box, four
  radii.

## 0.1.0-alpha.1 and .2 (2026-09-02)

The first published builds: the core, the wgpu backend, the Rust runner,
the Lua, C and Node bindings, and the npm package under `@qxuken/kui`.
