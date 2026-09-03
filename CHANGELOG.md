# Changelog

Every release lists what it adds and, separately, **what you can delete**:
the workaround, the model field or the arithmetic the release made
unnecessary. The second list is the point of the first — a library whose
upgrades remove code from the apps on it is doing the job.

## 0.1.0-alpha.6 (unreleased)

### Added

- **Selection and disclosure state** (`docs/adr/0001-accessibility-as-data.md`).
  `checked` covered checkbox / radio / switch and stopped there, so a row
  of tabs read out with no way to hear which one was open — AccessKit and
  ARIA keep "toggled" and "selected" apart, and kui only had the first.
  Two rows close it in every binding. `selected` marks the current one of
  a set: a `tab` reports the state either way, so its siblings read as
  "not selected", while a `listItem` or a `link` reports it only where the
  view sets it (an ordinary list is not a selection, and a reader saying
  "not selected" on each of its rows is noise). `expanded` names its state
  — `"collapsed"` or `"expanded"` — instead of being a flag, because a
  flag cannot say "collapsed": absent has to keep meaning "this node does
  not expand", and a shut disclosure that says nothing never tells you it
  opens. It lands wherever it is declared, since a twisty, an accordion
  header and a menu button share no role. The bridge maps them to
  AccessKit's `set_selected` and `set_expanded`. **"3 of 7" is not
  declared at all**: a `list` already holds its rows and a `tabList` its
  tabs, so the core numbers them itself — the zero-based ordinal on each
  item, the count on the container, the way AccessKit models a set — and
  reports them as `pos_in_set` / `set_size` (`posInSet` / `setSize` in
  Node, `KUI_ACCESS_HAS_POS_IN_SET` / `HAS_SET_SIZE` in C). The
  accessibility example grew a tab list and a disclosure, and
  `scripts/ax-audit.swift` grew 25 checks over them against the real macOS
  accessibility API (70 in total, all passing). That pass found two gaps
  that are AccessKit's, not kui's: a tab's state arrives as its `AXValue`
  rather than `AXSelected` (so the audit pins both halves, including that
  pressing a tab moves the state *off* the one that had it), and
  `accesskit_macos` maps no disclosure state and no set position at all —
  so `expanded` reaches UIA and AT-SPI but not VoiceOver, which the audit
  asserts so it fails the day that changes. It also found one gap that
  *is* kui's: because an unpicked `listItem` carries no selected state at
  all, macOS treats it as unselectable and drops a reader's request to
  select it — `AXPress` works, `AXSelected` does not. The audit pins both
  halves and ADR 0001 records the fix that would close it. Live regions and announcements
  stay out: an announcement is an event on a timeline, not a property of
  a tree, and it wants its own design (noted in ADR 0001's follow-ups,
  along with `required` / `invalid` and a heading's `level`).
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
- **The secondary mouse button, and `on_context_menu`.** `InputEvent` was
  built around one button — `MouseDown(u8)` carried the *click count*, not
  which button — so nothing in kui could right-click. It is now
  `MouseDown { button, clicks }` / `MouseUp { button }` over a
  driver-facing `MouseButton` (`Primary` / `Secondary` / `Middle` /
  `Other(n)`, so back and forward reach a driver without a vocabulary
  change), and every binding carries it: `kui_input_mouse_button` in C
  (`kui_input_mouse` still means the primary button, unchanged ABI),
  `ctx.mouse(down, clicks, "secondary")` in Node, `InputEvent::mouse_down`
  / `mouse_up` for the primary spellings in Rust. A new `onContextMenu`
  row (a `Kind::Tag`, so a null tag declares the behaviour without a
  payload, like `onKey`) emits `{kind:"contextmenu", x, y, tag}` on the
  node the press landed on, at the logical viewport point to open the menu
  at. **The press does nothing else**: it moves no keyboard focus, places
  no caret, starts no drag and produces no click, so right-clicking a
  selection still has that selection when the menu opens — that is what
  every platform does, and it is what makes a "Copy" item possible.
  Routing is a click's: the topmost node under the pointer answers, a
  disabled one answers nothing, and a modal scopes it like every other
  input (a press outside still dismisses, which is how a menu closes when
  you right-click elsewhere). With modal surfaces, that is a context menu
  end to end — the C counter example now opens one, and Escape or a press
  outside takes it away. Nothing routes the middle button or the ones past
  it yet; they arrive as data so a driver need not drop them.
- **Cursor shapes, derived rather than declared.** The one `set_cursor`
  in the tree used to be the synthesized resize band, so an editor showed
  an arrow instead of an I-beam, a button an arrow instead of a hand, a
  drag handle an arrow instead of a grab. The core is the only thing that
  knows what is under the pointer, so it is the thing that answers:
  `Core::cursor_shape()` resolves a `CursorShape` each frame from the
  topmost node under the pointer — the same node a click would go to — and
  the driver applies it. Nothing declares a cursor for the ordinary
  cases: an editor is `text`, an `onClick` or `focusable` node is
  `pointer`, an `onDrag` node is `grab` and `grabbing` for as long as the
  drag holds the pointer, window chrome and a plain box are the arrow.
  A captured drag keeps the dragged node's shape however far the cursor
  wanders off it, and a scrollbar drawn over content is the arrow rather
  than whatever is under it — both the way the press already resolves. A
  `cursor` row covers what the derivation cannot know: a splitter that
  resizes (`ewResize` / `nsResize`) rather than moves, a `disabled`
  control that would rather say `notAllowed` than stay quiet. One row, so
  JSX, Lua, C and Rust all got it; `kui_cursor_shape` reads the resolved
  shape back in C. It is a query, not a queue — a state a driver applies
  when it changes — and a headless driver simply never asks, so the core
  is still device-free.
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
- **Programmatic scrolling.** Scroll offsets have always been retained by
  the core and keyed by node; nothing handed them out, so the wheel, the
  scrollbars, Tab and the caret could move them and an app could not.
  Three calls now do, in every binding: `reveal(key)` scrolls whatever
  contains a node so the node shows — what Tab already does to the control
  it lands on, asked for by name — while `scroll_offset(key)` and
  `set_scroll(key, offset)` read and write a container's offset directly
  (`ui.reveal` / `ui.scroll_offset` / `ui.set_scroll` in Rust,
  `ctx.reveal` / `scrollOffset` / `setScroll` and the same three on
  `KuiWindow` in Node, `kui_reveal` / `kui_scroll_offset` /
  `kui_set_scroll` in C, `env.reveal` / `env.scroll_offset` /
  `env.set_scroll` in Lua). A written offset is clamped by the next
  layout, so `(0, 0)` is "jump to the top" and a large value is "jump to
  the end" with no content height in the app. A `reveal` resolves against
  the *next* frame's layout rather than the last one's — a frame is
  requested, so one comes — which is what lets a view reveal a row it is
  declaring for the first time, and what makes `env.reveal` work at all in
  Lua, where `view(env)` runs while the tree is being rebuilt. A key that
  frame does not declare is a no-op, and is not held for a later frame.

- **Affordable long lists.** Glyphs have always been culled by viewport;
  the nodes around them never were, so a view that declared ten thousand
  rows paid for ten thousand rows of build and layout whether or not they
  could be seen — a bench of a 10k-row log frame costs ~4.8 ms here, most
  of it rows nobody sees. An app could not fix this itself: slicing its
  own data needs the container's scroll offset *and* its resolved height
  during the build, and the height existed only inside a layout pass that
  cleared its tree before the next view ran.
  `scroll_geometry(key)` retains and hands out the whole of it — the
  container's box, its laid-out content size, where it is scrolled to and
  how far it can travel — in every binding (`ui.scroll_geometry` in Rust,
  `ctx.scrollGeometry` / `KuiWindow.scrollGeometry` in Node,
  `kui_scroll_geometry` in C, `env.scroll_geometry` in Lua). The four
  numbers describe one moment: the offset is the retained one already
  clamped to that container's travel, so a `set_scroll(key, huge)`
  meaning "the end" reads back as the end rather than as a row index a
  million past the data. `None` (`null`, `nil`, `false`) until a layout
  has resolved the key as a scroll container — writing an offset at a key
  does not invent one.
  `widgets::virtual_column` is the uniform-row case done: it declares the
  rows crossing the window, two rows of overscan and two spacers holding
  the space of the rest, so the content height, the scrollbar and
  `set_scroll` all behave as if the whole list were there. The same 10k
  rows through it cost **~19 µs instead of ~4.8 ms**, and 100k rows cost
  the same ~19 µs — the frame stops growing with the data. Rows are
  opened at their *data* index (`Ui::open_indexed` / `with_indexed`, and
  `child_key_index` for the key before the node), so a row keeps its key,
  and with it its hover, focus, edit buffer and tweens, as the built range
  slides over it; `widgets::visible_rows` is the slice arithmetic alone,
  for views that build their own container.
  This is (a) of the two options the backlog listed. A `virtual` flag in
  the core — (b) — stays unbuilt, and now needs a case this does not
  serve.

### Changed

- `KuiSpec` gained `tooltip` (appended; a zeroed struct means what it
  meant): the C spelling of the `tooltip` prop the other bindings have —
  it makes the node hover-tracked, becomes its accessible description,
  and `kui_close` floats the hint below it while hovered. `kui_tooltip` /
  `kui_tooltip_with` stay what they were, a hint that always draws.
  `Ctx.windowTitle()` in Node reports the title a frame declared.
- The Node counter example clicks into its editor before typing, and CI
  runs it instead of only typechecking it. It had been failing since
  0.1.0-alpha.5 made a click take the keyboard (ADR 0002): `autofocus`
  only claims the keyboard while nothing holds it, so after the example
  clicked a button its typing went nowhere and its own self-check said
  MISMATCH — to nobody, because the CI step stopped at `tsc`. The example
  is a self-checking headless drive; it is worth a `node` run.

### What you can delete

- **The label that spelled out the state of a tab** — `label="General
  (current)"`, or the "selected" suffix an app appended so a reader would
  say *something*: `selected` is the state, and the name stays the name.
- **The `aria-posinset` equivalent an app was computing** — the index and
  the total it threaded through every row of a list so it could put "3 of
  7" into a label. The core counts the items it already holds.
- **The two-props-for-one-state workaround on a disclosure**: a `label`
  that changed between "Advanced (collapsed)" and "Advanced (expanded)",
  or a `checked` misused as an open flag on a node that is not a
  checkbox. One `expanded` row, and a reader that says the right word.
- **"kui can't right-click"**, and every workaround under it: the
  Ctrl-click convention an app invented, the long-press timer on a
  desktop UI, the "menu" button bolted next to a row that should have had
  a context menu. A `onContextMenu` tag and the point to open at.
- **A blur-then-restore dance around a right-click** — the focus and
  selection an app saved before opening its own menu and put back after:
  a secondary press never moved them.
- **The full-viewport `onClick` scrim behind a dialog** — and the half of
  modality it never bought: a `modal` node blocks Tab, the wheel and
  assistive technology too, and tells the platform it is a dialog.
- **The "what was focused before this dialog?" field in the model**, and
  the `focus(key)` call on the way out: the core remembers what the modal
  displaced and gives it back.
- **A hand-rolled Escape binding on every dialog**, and the outside-click
  hit test under a menu: both arrive as `{kind:"dismiss", reason}` on the
  node that asked to be modal.
- **Every `set_cursor` an app made from the outside** — the hit test it
  re-ran against its own layout to guess what the pointer was over, and
  the arrow it settled for when it could not. The core already knew;
  `Core::cursor_shape()` says so.
- **"Does the C build do what the JSX build does?"** — the question, and
  the hand-written probe app written to answer it. One corpus, four
  adapters, one CI job: a binding that lowers a prop differently names
  the scene and the line.
- **A `kui_is_hovered` round trip around every C tooltip**, and the
  accessible description it silently dropped: `KuiSpec.tooltip` is the
  prop the other three bindings already had.
- **The "scroll to the selected row" issue that closed as "can't"**, and
  every approximation under it: the `autofocus` on an off-screen row that
  was really a scroll request, the fixed row height an app multiplied by an
  index to guess an offset it had no way to apply, the "press End" hint in
  a tooltip. `reveal(key)` is the one call, and it needs no geometry.
- **A scroll position saved by listening to the wheel** — the running
  total an app kept beside the core's, wrong the moment content changed
  size and re-clamped the real one. `scroll_offset(key)` is the number the
  last layout actually used, and `set_scroll` puts it back.
- **The row cap on a log view, a table or a chat history** — the "last
  500 lines" an app truncated to because the frame could not afford the
  rest, the paging buttons under a list that should have scrolled, and
  the `onLayout` handler bolted to a container only to copy its height
  into the model so the next frame could slice by it. `scroll_geometry`
  answers during the build, from the frame before, with no event and no
  model field.
- **A virtual list's home-made row identity** — the `key={"row" + i}`
  string built for every row so that hover and focus would not slide when
  the window scrolled. `open_indexed(i)` is the key auto-keying would
  have given row `i` anyway, so a virtualized list and a full one agree.

## 0.1.0-alpha.5 (2026-09-03)

### Added

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
