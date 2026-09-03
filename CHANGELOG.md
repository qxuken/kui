# Changelog

Every release lists what it adds and, separately, **what you can delete**:
the workaround, the model field or the arithmetic the release made
unnecessary. The second list is the point of the first — a library whose
upgrades remove code from the apps on it is doing the job.

## 0.1.0-alpha.5 (unreleased)

### Added

- **Modal surfaces** (`docs/adr/0003-modal-surfaces.md`). One row,
  `modal`, makes a node the frame's modal surface in every binding
  (`modal` in JSX, `modal = true` in Lua, `KuiSpec.modal` in C,
  `NodeSpec::modal` in Rust): the Tab ring becomes its subtree and wraps
  inside it, focus enters it when it appears and returns exactly where it
  was when it goes away, and everything outside it stops taking
  input — no click, drag, hover, press, wheel, scrollbar thumb, Enter /
  Space, or assistive-technology activation, all of which resolve against
  the hit list the modal now scopes. Scrollbars behind it still draw;
  window chrome stays live, so a dialog never traps the window. The
  access tree reports `modal` (AccessKit's `set_modal`, ARIA's
  `aria-modal`, so VoiceOver keeps its cursor inside) and derives
  `role="dialog"` when the view declares none. Escape, and a press that
  lands outside, emit `{kind:"dismiss", reason:"escape"|"outside", tag}`
  on the modal node — the core closes nothing, because only the app can
  stop declaring the dialog (or ask first). The last modal declared in
  tree order is the one in effect, so a confirm inside a dialog stacks
  without a stack API, and a modal that must cover the app is a float —
  one that is not says so, as a new `modal-behind-content` warning. The
  accessibility example grew a "Delete…" button and the confirm dialog it
  opens, which is what a VoiceOver session can be pointed at.
- **The binding-parity table is a build failure now.** `CUSTOM` and
  `ELEMENTS` in `crates/kui-core/src/schema.rs` name the ten props and ten
  elements every frontend lowers by hand, and until now that agreement was
  about *names* only — nothing checked that four bindings did the same
  thing with them. `kui_core::conformance` is a **scene corpus**: small
  named scenes with the input to replay and the semantics to expect, each
  declaring which `CUSTOM` and `ELEMENTS` rows it covers (a test fails
  when a row has no scene). `conformance::drive` runs one and
  `conformance::report` renders it as a line-oriented block with no
  formatted floats in it — quad geometry travels as one FNV-1a digest —
  so four languages can produce the same bytes. Four thin adapters read
  the one corpus: Rust asserts it natively
  (`crates/kui-core/tests/conformance.rs`), Lua re-expresses each scene as
  the table a script returns (`crates/kui-lua/tests/conformance.rs`), C
  rebuilds them through the C API alone (`./counter --conformance`), and
  Node runs each scene on all three transports beside its existing
  `assertParity` (`packages/kui/test.mjs`). The reference report is
  generated per run (`cargo run -p kui-core --example conformance-dump`),
  never checked in: its digests cover real glyph geometry, so it holds
  only for the machine and fonts that made it — which is why all four
  adapters run in CI's single `check` job.
- **Keyboard focus as data** (`docs/adr/0002-keyboard-focus-as-data.md`).
  One focus in the core for every node, and every control the access
  tree knows is a Tab stop: editors, `onKey` sinks, `onClick` boxes, the
  control roles (`button`, `checkbox`, `radio`, `switch`, `slider`,
  `tab`, `link`) and any node declaring the new `focusable` flag — in
  tree order, wrapping, never a `disabled` node, `role="none"` decoration
  or window chrome. Enter and Space press the focused control (the same
  event a click emits), the arrows nudge a focused slider (the same
  `increment` / `decrement` access events), Escape lets go. Keyboard
  focus scrolls its node into view and draws a ring on top of the frame,
  in every binding, unless the node declares `focusBg` (pressed wins over
  focus wins over hover); a click's focus draws nothing, like the web's
  `:focus-visible`. `disabled` is a row: an inert node that keeps its
  tooltip and tells a screen reader so. The access tree reports `focused`
  and `disabled` on every node, every focusable node advertises `focus`
  / `blur`, and a reader's focus request lands where Tab would — which is
  what makes VoiceOver's cursor follow into buttons instead of stopping
  at editors. New calls: `ui.focus(key)` / `blur()` / `focus_next()` /
  `focus_prev()` / `focused()` / `focus_visible()` (`ctx.focus(key)`,
  `ctx.focused()`, ... in Node; `kui_focus`, `kui_focus_next`,
  `kui_focused`, `kui_focus_visible` in C; `env.focus` /
  `env.focus_visible` in Lua). `scripts/ax-audit.swift` gained a
  keyboard-focus section: a reader focusing a button, and a real Tab
  keystroke moving on.
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
- **A platform audit.** `examples/accessibility` puts every prop in one
  window, and `scripts/ax-audit.swift` drives it through the macOS
  accessibility API — the one VoiceOver calls — checking roles, names,
  values, the text protocol and the actions, each by the change it
  makes. It found the two things headless tests could not: a drawn
  titlebar announced the window title twice (the strip is now unnamed;
  the window carries it), and the latency HUD was read out between the
  controls (`widgets::latency_graph` is now `role="none"`).
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

- `KuiSpec` gained `tooltip` (appended; a zeroed struct means what it
  meant): the C spelling of the `tooltip` prop the other bindings have —
  it makes the node hover-tracked, becomes its accessible description,
  and `kui_close` floats the hint below it while hovered. `kui_tooltip` /
  `kui_tooltip_with` stay what they were, a hint that always draws.
  `Ctx.windowTitle()` in Node reports the title a frame declared.
- `ui.take_key_focus(key)` / `<box keyFocus>` / `key_focus = true` /
  `kui_set_key_focus` are edge-triggered: the node takes focus on the
  first frame it is declared, and a declaration repeated every frame no
  longer wins focus back from a Tab press or a click. Apps that relied on
  that call `focus(key)` when they mean it. `is_focused` (`isFocused`,
  `kui_is_focused`, `env.is_focused`) answers for any node, not editors
  only; `key_focus()` answers the same unified focus. An `autofocus`
  editor no longer takes focus from a focused control.
- `KuiSpec` gained `focusable`, `disabled` and `focus_bg` (appended; a
  zeroed struct means what it meant); `KuiAccessNode.flags` gained
  `KUI_ACCESS_DISABLED`.
- `KuiMsg`: the docs now say to register only the app's own messages.
  `CoreMsg` is typed in terms of the registration, so registering
  `MyMsg | CoreMsg` made the alias circular.
- `KuiSpec` gained `on_layout` (appended; a zeroed struct means what it
  meant).

### What you can delete

- **The full-viewport `onClick` scrim behind a dialog** — and the half of
  modality it never bought: a `modal` node blocks Tab, the wheel and
  assistive technology too, and tells the platform it is a dialog.
- **The "what was focused before this dialog?" field in the model**, and
  the `focus(key)` call on the way out: the core remembers what the modal
  displaced and gives it back.
- **A hand-rolled Escape binding on every dialog**, and the outside-click
  hit test under a menu: both arrive as `{kind:"dismiss", reason}` on the
  node that asked to be modal.
- **"Does the C build do what the JSX build does?"** — the question, and
  the hand-written probe app written to answer it. One corpus, four
  adapters, one CI job: a binding that lowers a prop differently names
  the scene and the line.
- **A `kui_is_hovered` round trip around every C tooltip**, and the
  accessible description it silently dropped: `KuiSpec.tooltip` is the
  prop the other three bindings already had.
- **A "keyboard users can't reach the buttons" issue**, and the custom
  Tab handling an app wrote around it: every control is a Tab stop, Enter
  and Space press it, and the ring draws itself.
- **A `focused` field in the model** mirrored into styling by hand, and
  the per-frame `take_key_focus` that pinned it: `is_focused` answers for
  any node, `focus_visible` says whether to show it, and `focusBg` does
  the styling with no query at all.
- **A "disabled" bool that only changed a colour** while the click still
  fired: `disabled` makes the node inert everywhere at once — pointer,
  keyboard, screen reader.
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
