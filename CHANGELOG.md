# Changelog

Every release lists what it adds and, separately, **what you can delete**:
the workaround, the model field or the arithmetic the release made
unnecessary. The second list is the point of the first — a library whose
upgrades remove code from the apps on it is doing the job.

## 0.1.0-alpha.6 (unreleased)

### Added

- **Flex wrapping: `wrapChildren` and `crossGap`** (backlog C10). A row of
  tags, a toolbar of chips, a button row that has to survive a narrow
  window — none of them could be written. The solver's only answer to
  children that overflow the main axis was to squeeze them (or, on a
  scroll axis, to let them spill), and the userland workaround was to
  measure every item with `measure_text` and assemble the rows by hand.
  `wrapChildren` breaks them onto more lines instead; `crossGap` is the
  space between the lines, as `gap` is the space along one.
  **A wrapping row that happens to fit lays out identically to a plain
  one** — not approximately, node for node — so the flag is safe to leave
  on a row that only sometimes overflows, which is the whole point of it.
  That falls out of the two rules underneath: main alignment places each
  line in the content box the way it placed the single run, and the lines
  share the container's leftover cross space equally (CSS's
  `align-content: stretch`), so with one line the two compose back into
  the old placement exactly.
  **Wrapping answers overflow before shrinking does.** They are two
  answers to the same question and they now have an order: a child that
  can move to the next line moves, and its neighbours keep their size —
  where before, one wide chip cost every chip in the row its width.
  Greedy breaking leaves exactly one case a break cannot fix (a single
  child wider than the box, alone on its line), and that is where
  shrinking takes over, on that line alone. A text node too long for the
  row gets its own line and rewraps inside it.
  **Rows only, and the reason is the pass order rather than an unfinished
  half.** Breaking needs a definite main size, and a row's width is final
  one pass before the height that has to sum the lines; a column's main
  size is not resolved until two passes *after* the fit that would need
  them. `wrapChildren` on a column, or on a `scrollX` row (an axis with
  no bound has nothing to break against), lays out as if it were absent
  and says so — a new `wrap-ignored` warning, rather than the silence
  that reads as a broken feature.
  It is `wrapChildren` and not `wrap` because `wrap` is taken, by the
  text prop that picks where a line breaks inside one paragraph, and the
  two meet on `<edit>`. Two plain schema rows, so Lua (`wrap_children`,
  `cross_gap`), JSX and the generated TS types got them free, and
  `KuiSpec` gained two appended fields. A `wrap` scene joins the corpus,
  so all four bindings reproduce a two-line row byte for byte.
  Benched: 10k chips in 100 rows that each break into several lines cost
  1.24 ms against 1.06 ms for the same tree not wrapping — the same as
  the existing 10k-rect frame. A row that does not wrap pays nothing.
  **What you can delete:** every hand-rolled row-packer — the
  `measure_text` loop that accumulates widths, the running total, the
  "start a new row" branch and the outer column holding the rows — and
  the guesses that stood in for it: the fixed item width that made the
  arithmetic possible, the `maxWidth` chosen so N items always fit, and
  the horizontal scroll container used because a chip list had nowhere
  else to go.

- **`KeyUp`, and one payload shape for both halves of a key**
  (backlog C9). `InputEvent::KeyDown` had no counterpart, so a held-key
  interaction could not be written at all — WASD movement, press-and-hold
  to preview, a key that arms a mode while it is down — even though
  `KeyDown`'s own doc names "a game" as its audience. `Modifiers` covered
  the release of Shift and Command and nothing else. `InputEvent::KeyUp`
  closes it, routed to key focus exactly the way `KeyDown` is. Both halves
  arrive as **one** `{kind="key"}` payload with a `phase` of `"down"` or
  `"up"` — the shape a drag's three phases and a hover's two already use —
  so an app binds one handler and matches `phase`, and no binding grew a
  second event kind to plumb. `text` is null on every release (a release
  inserts nothing) and `repeat` false; the core normalizes both, so four
  drivers cannot disagree about it.
  Two rules make a stuck key impossible. **A key only comes up where it
  went down**: a release whose press the sink never got — pressed while an
  editor held focus, or already let go of — resolves nothing, so no sink
  hears an `up` it has no `down` for. And **focus moving lets go first**:
  `Core::set_focus` releases everything held, to the sink that took the
  presses, in press order, before the focus lands anywhere else. Drivers
  do the same when the window loses the keyboard, where the OS will send
  no release at all — the winit runner on `Focused(false)`,
  `kui_release_held_keys` for a C host, `Core::release_held_keys` for
  anyone else.
  Every binding drives it: `InputEvent::KeyUp` in Rust,
  `kui_input_key_down` / `kui_input_key_up` / `kui_release_held_keys` in
  C (which also closes a gap the C example had noted — a C host could not
  drive an `on_key` sink at all, `kui_input_key` carrying only the editing
  keys), `ctx.keyUp` beside a `ctx.keyDown` that now takes `repeat` in
  Node, and the payload as-is in Lua. `KeyCode::from_name` is the one
  parser they share, so `"pagedown"` cannot mean different keys in
  different languages. The winit driver stopped dropping releases on the
  floor and reports `repeat` from the OS.

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
  Node drives each scene through its encoder (`packages/kui/test.mjs`).
  The reference report is
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

- **Group opacity and drop shadows**
  (`docs/adr/0005-the-paint-vocabulary.md`). The renderer contract was fill,
  border, four radii, glyph and image, and two things a UI wants were
  missing from it. `opacity` (0..1) fades a node *and its whole subtree*: it
  multiplies down the tree and into the alpha of every quad the subtree
  emits — box, border, glyph, image, scrollbar, focus ring — and changes
  nothing else, so a faded subtree still lays out, still takes clicks and is
  still read out, which is CSS's rule for `opacity: 0` and the one that
  makes fading a *live* panel usable. It is a per-quad multiply rather than
  an offscreen composite, so overlapping pieces of one subtree show their
  seams through the fade; the prop doc says so rather than leaving it to be
  discovered. It eases with `transition` and joins `keyframes` and `enter`,
  so `enter={{ opacity: 0 }}` fades a whole panel in — the half of exit
  animations that could not be written before, since `enter` could fade a
  node's own `bg` and not a subtree.
  `shadowColor` with `shadowBlur` / `shadowX` / `shadowY` / `shadowSpread`
  casts one drop shadow behind a node. It is a new `QuadKind::Shadow` and
  one `blur` on the quad, not a nine-slice: the core emits the shape already
  offset, spread and inflated by the blur, and the shader ramps the same
  `sd_rounded_box` it already evaluates for rounded rects — so a shadow is
  one more instance in the same single draw call, with no atlas entry, no
  second pass and no seams. The color is the switch (nothing draws without
  one), the geometry and the color each ease with `transition`, and the
  scope is deliberately small: outer shadows only, one per node, and the
  shape is not knocked out of the middle, so a translucent background shows
  its own shadow through itself. Six plain schema rows, so Lua, JSX and the
  generated TS types got them for free; `KuiSpec` gained seven appended
  fields (`opacity` needs an `opacity_set` bit, because a zeroed struct is
  the schema default and this default is 1, so 0 cannot double as "unset").
  The `layout` conformance scene's card now carries both, so all four
  bindings reproduce them or fail. ADR 0005 also records the two decisions
  that produced no code: **no gradients in v0**, and the design for **exit
  animations** — a departing subtree retained by key, frozen where it was
  and replayed inert until its transition ends — which is a real change to
  the frame model and is written down rather than half-built.

### Changed

- `Quad` gained `blur` and `QuadKind` gained `Shadow`, which is ABI:
  `KuiQuad` mirrors the core quad field for field, so its `kind` moved from
  word 17 to word 18 and `uv` / `clip` shifted with it. A host reading
  quads by raw offset (the C example's digest, the JS `decodeQuads`) has to
  move with it; a host using the struct definitions recompiles and is done.
  The conformance report's `kinds` line grew a sixth column for shadows.
- `KuiEnter` and `KuiKeyframe` gained an appended `opacity` with a
  `KUI_ENTER_OPACITY` / `KUI_KF_OPACITY` bit.
- **`{kind="key"}` payloads carry a `phase`, and a sink now hears
  releases.** An app that took every `kind="key"` event as a press acts
  twice unless it filters: match `phase == "down"` (the `modal_editor`,
  `splitmux` and `syntax_view` examples each gained exactly that one
  line). `KeyPress::to_value` takes the phase as an argument — the only
  source-breaking signature in this — and `KeyPress::released` is the
  normalizing helper drivers reach for.
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
- **Node has one frame transport, not three** (backlog D2). `frameObject`
  (napi walking the JS object graph) and `frameJson` (a stringified tree
  through serde) are **removed**, with `setViewObject`, `setViewJson` and
  the `transport: 'json'` option on `createApp` / `runWindowed`. They were
  reference paths: two ~200-line element dispatchers in the addon, one
  carrying the comment "Mirrors the JSON path's button styling exactly",
  kept equal by a test that compared their quads. What made them
  unnecessary was P7 — the corpus scenes check each frame against a report
  `kui-core` generates, which says what the frame must *be* rather than
  only that two hand-written dispatchers agree. `binary.rs` is now the
  addon's only dispatcher; `frame()` and `setView()` are what they always
  were.
  Errors came out ahead. They now all come from the encoder, in JS, before
  anything crosses the boundary, and they name the value: `bad dir
  "diagonal" (row | column)`, `bad value "middle" for mainAlign (one of
  start | center | end)`. The one message only the JSON dispatcher had —
  "element without a type — did it come from kui/jsx-runtime?" — moved to
  the encoder. `measureText` and `play` still take plain JS objects; those
  are queries, not a transport, and the JSON prop parser stays for them.
  The suite that replaced `assertParity` was measured against it rather
  than argued for: 22 single-edit mutations of `encoder.js` — swapped
  slots in every hand-written composite, wrong mode numbers, dropped
  flags, swapped element operands — are caught 22/22 by the new one and
  were caught 15/22 by the old.

- **Two corpus scenes, from mutation-testing that comparison** (P7). Seven
  of those 22 mutations passed *both* suites, and the reason was the
  corpus, not the transport: the `float` scene attached with `at` equal to
  `self_at`, `dx` equal to `dy`, and landed inside the viewport anyway, so
  swapping either pair or dropping `fit` changed nothing anyone looked at.
  It now attaches off-screen on both axes with every value distinct, so
  all three are load-bearing. A new **`sizing`** scene puts one child of
  each mode in a row of known width — fixed 30, 25% of 200 = 50, fit
  around a 20-wide child, grow taking the remaining 100 — because the four
  modes were otherwise only ever exercised inside fit-sized parents, where
  a percent and a grow resolve alike. Its outer pad spells `padX`/`padY`
  unequally, which nothing covered: `layout` sets all four edges, so the
  shorthand pair never resolved. All four adapters gained the scene, so C
  and Lua are pinned on these too, not only Node. A scene built from
  symmetric values pins nothing about order — worth knowing before adding
  the next one.

- **One loop, two surfaces.** `createApp` and `runWindowed` were two loops
  that wrote the diagnostics gate, the warning formatter and the transport
  switch twice each — and then diverged where they should not have.
  `createApp` called `update(model, msg, event)`; `runWindowed` called
  `update(model, msg, event, win)`. `tick` existed only on the windowed
  side, so **an app with a clock could not be driven headless at all** —
  the README's central claim (the same app runs headless, and a test drives
  it the way a user would) failing on exactly the apps that most want a
  test. And the affordances that make a test read like a user — `click`,
  `type`, `key`, `settle`, `access`, the accumulated `warnings` — were the
  headless driver's, not the loop's, so watching the same script against a
  real window meant writing it a second time.
  There is one loop now, over an injected surface. `Ctx` and `KuiWindow`
  answer the same handful of calls — `pollEvents`, `warnings`,
  `setDiagnostics`, and a way to be shown a tree — so the loop is written
  once and handed one of them; the shared napi macro is where that stopped
  being a coincidence. `update`'s fourth argument is that surface,
  unconditionally, in both drivers, so one `update` serves both.
  `tick` moved into the shared loop with the clock injected: the wall clock
  under a window, and headless `app.advance(ms)`, which fires every tick
  that falls inside the span, moves the frame clock behind `transition`
  along with it (`ctx.setTime` — the core has no clock of its own, which is
  what makes this possible) and re-renders. A window still resyncs rather
  than firing a burst after real time jumps; `advance` fires the whole span,
  because a test asked for exactly that much time. `startTime` pins the
  origin so assertions on `tick.msg(now)` are exact.
  `settle` / `access` / `accessTree` / `dispatch` / `render` / `step` are
  the loop's, so `runWindowed`'s `setup(win, app)` hands a test the same
  object driving a real window. Synthetic input asks the surface for
  `mouse` / `text` / `key` and names the one it does not have, rather than
  failing as a missing method — a window takes its input from the OS.
  The two contracts that are real differences stayed exactly as they were:
  `runWindowed` resolves with the final model, `createApp` is synchronous.

- **The `.d.ts` for the addon is generated, not written** (backlog P5).
  `Ctx` and `KuiWindow` are 39 shared methods each, written once in Rust
  by `core_methods!` since D1 — and then a third time, by hand, in
  `packages/kui/index.d.ts`, the copy a JS user actually reads, with
  nothing checking that the two agreed. napi-rs derives a TypeScript
  signature for every `#[napi]` item; `crates/kui-node`'s build script now
  points it at `target/napi-type-defs`, and `npm run gen` renders that into
  a marked region of `index.d.ts`, beside the ones `jsx-runtime.d.ts` and
  `docs/props.md` already have. CI diffs the file. A method added to one
  class only — the failure D1 named and could not close — now fails the
  build.
  The derived output was checked before it was trusted, and the risk unique
  to this codebase is not one: `#[napi]` expands *after* `core_methods!`
  does, so both classes come out complete, doc comments and all. `Json` is
  the part that does not survive — every payload parameter and return would
  have been `any`. Rather than take the weaker fallback (a script asserting
  that each `#[napi]` method appears in a hand-written file), the Rust side
  now names the real TypeScript type at the definition:
  `#[napi(ts_return_type = "AccessTree")]`,
  `ts_args_type = "content: KuiNode, style?: TextProps, maxWidth?: number"`,
  `ts_generic_types = "A = AppMsg | CoreMsg"`. So `pollEvents<A>()`,
  `measureText`, `play`, `scrollGeometry` and the rest are typed exactly as
  precisely as they were — out of one copy instead of two.
  No signature changed. What stays hand-written is what has no `#[napi]`
  item behind it: the message and payload types (`CoreMsg`, `AccessTree`,
  `Warning`, `WindowOptions`, `LoopConfig`, ...), `createApp`,
  `runWindowed`, `decodeQuads` and `createEncoder` — plus `frame` and
  `setView`, which live on the prototypes in `index.js` rather than in the
  addon and reach the generated classes by declaration merging. `Protocol`
  is a new exported name for what `protocol()` already returned.
  The prose moved too: the doc comments on those 102 members are now the
  Rust ones, with whatever the `.d.ts` copy said better folded in.

### What you can delete

- **The clock you dispatched by hand in a headless test** — the loop
  calling `app.dispatch(tickMsg)` at intervals it made up, the
  `app.render()` after each one, and the `app.ctx.setTime(t)` you kept in
  step with a counter of your own, because `tick` was a windowed-only
  feature and a ticking app had no test driver. `app.advance(ms)` is those
  three, and it fires the app's own `tick` rather than a stand-in for it.
- **The second `update` written for the headless driver** — the one that
  took three arguments because `createApp` handed it no surface, next to
  the four-argument one the window got. Both drivers pass the surface now,
  so the two collapse back into one function.
- **The test script written twice** — once with `app.click` / `app.type`
  headless and once as hand-rolled `pollEvents` / `update` / `setView`
  against a window, to watch the same steps run. `runWindowed`'s
  `setup(win, app)` hands over the loop itself.

- **The alpha you were threading through a subtree by hand** — the
  `bg`, text `color` and border color an app recomputed at a fraction so a
  panel could look dimmed, and the "fade factor" it kept in its model to do
  it with. One `opacity` on the top of the subtree, and it eases.
- **The staging frame for a fade-in.** A panel that had to be drawn at a
  transparent `bg` for one frame and re-drawn opaque the next (and the
  `request_frame` that made the second frame come) is `enter={{ opacity: 0
  }}` — and it fades the panel's text and images too, which the `bg` trick
  never did.
- **The stack of translucent boxes standing in for a shadow** — the three
  or four nested rects at decreasing alpha and increasing radius under a
  card or a dialog, and the padding arithmetic that kept them centred.
- **The timer that stood in for a key release** — the `Instant` an app
  kept per held key, the "assume it was let go after 250 ms" heuristic,
  the tick handler that decayed a movement vector because nothing would
  ever tell it the key came up. There is a release now, and it arrives
  even when focus moves out from under the key.
- **The modifier-only workaround**: bindings shaped around `Modifiers`
  because it was the one release the core reported, so "hold to preview"
  had to be spelled as "hold Option".
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
