# Changelog

Every release lists what it adds and, separately, **what you can delete**:
the workaround, the model field or the arithmetic the release made
unnecessary. The second list is the point of the first — a library whose
upgrades remove code from the apps on it is doing the job.

## 0.1.0-alpha.6 (unreleased)

The release that made a window something you can build a real app in.
`modal` is one row that scopes the Tab ring, the hit list and the access
tree, and hands focus back exactly where it found it; a tab list, a radio
group, a menu and a picker list are each one Tab stop with the arrows
moving inside, derived from roles a view already declares. Nodes can leave
as well as arrive (`exit`), a subtree dims as one (`opacity`), a card
casts a shadow and clips its children to its own corners, and a row of
them wraps (`wrapChildren` / `crossGap`). A frame declares which windows
exist and the runner opens them, over a `Session` that owns the device,
font database and registry they share — all of ADR 0004 except popups.
The C ABI has a version and a size handshake, C can be an *extension*
inside a Rust host and not only a host itself, and one scene corpus drives
all four bindings in CI.

**What breaks.** **The C ABI went 2 → 6**: every C host recompiles and
checks `kui_abi_version()`. `kui_take_window_commands`, which filled a
`uint32_t` array, is now `bool kui_take_window_command(ctx,
KuiWindowCommand *)` popping one at a time — renamed so an un-edited host
fails to link instead of passing the wrong pointer type — and
`kui_env_set_window` leads with a window id. `KuiEvent` gained `window`,
`KuiAccessNode` gained `orientation`, and `KuiWindowCommand` gained its
window, a config and `width`/`height`: appends under ADR 0006's `size`
handshake, so a host that kept its reservation keeps working, but only if
it asked. **Node's JSON and object frame transports are gone** (backlog
D2): `frameObject`, `frameJson`, `setViewObject`, `setViewJson` and the
`transport: 'json'` option are removed, and `setView` is the one door.
**Lua's `env.focus` and `env.focused` are one letter apart and are
different facts** — the focused node's key, and whether the window has the
keyboard at all — which breaks nothing today, and `env.focus` is
deprecated below.

Everything after this is the detail as each piece landed;
`docs/BACKLOG.md` carries the reasoning and the alternatives declined.

### Native verification

The five prebuilds are cross-compiled on one Linux runner, and P8's
`smoke-macos` / `smoke-windows` jobs are gated on repository variables no
runner satisfies — so nothing had executed on a platform since alpha.5.
This is what was run by hand before the tag (backlog R4), and what was not.

**macOS 26.6.2 (arm64), rustc 1.98.0, 2026-09-05.** `cargo test --workspace`
passes: **545 tests over 55 suites, 0 failed, 0 ignored**. The workspace
still needs no display, no installed fonts and no GPU — P8's note recorded
that on 2026-09-03 and 131 commits had not re-checked it. `scripts/ax-audit.swift`
against `examples/rust/accessibility.rs` passes **88/88 checks across all
nine sections** (roles and names, values, selection, disclosure, both
editors' text, actions, keyboard focus, composites), so alpha.6's access
work is confirmed against the real macOS AX API rather than against kui's
own types: `modal` arrives as an `AXWindow`/`AXDialog`, a radio group as an
`AXRadioGroup` and a menu as an `AXMenu` — each with the orientation its
`dir` derived — and `set_selected` / `set_expanded` move the selection and
the disclosure from the reader's side.

Watched in a real window, not asserted headlessly: focus enters a modal on
the node its `initial_focus` named (Cancel, not the first in tree order),
eight Tabs never leave its two controls, and Escape dismisses it and hands
focus back to the button that opened it; a secondary press opens a context
menu where it landed, Escape closes it, and the press never arrives as a
click (`examples/c/counter`); the pointer is an I-beam over an editor, a
hand over a button and an arrow over a plain box. `modal_editor` switches
NOR → INS on `i` and types into the buffer; `splitmux` splits and opens a
tab from its ⌥ chords. Every host opened a window and drew: the Rust
examples, `examples/c/counter`, `counter-window.mjs` on Node, `lua_panel`,
and `c_panel` — the C plugin dlopened into a Rust host.

**Not run, and so not claimed.** Nothing has executed on **Windows**: no
machine or VM was reachable, `crates/kui/src/windows_nc.rs` — 292 lines of
`WM_NCHITTEST` — has still never run since it was written, and neither has
the win32-x64 prebuild. The **published prebuilds are untested as
artifacts** on every platform: what ran here was built from source on the
machine, not the cross-compiled binary the release ships. `SMOKE_MACOS`
stays unset, because turning it on without a `macos`-labelled runner
registered would leave the job queued on every push instead of reporting
anything — the job is written and waits on a runner, not on an edit.

### Added

- **The warning codes are one table** (backlog S2). `kui_core::diag::WARNINGS`
  lists every code with the const's own doc comment, the addon's `protocol()`
  exports it as `warnings`, and `npm run gen` writes the `WarningCode` union
  into `index.d.ts` and a Warnings table into `docs/props.md` from it — the
  mechanism the props already use. The hand-written union had drifted to 11
  of 13 (`exit-budget` and `focusable-inside-item` were missing); a unit
  test now pins the table to the consts, and CI's `git diff --exit-code`
  pins the generated files to the table.
- **A resource handle used in the wrong session is caught** (backlog S5).
  Every `Session` now has a process-wide `SessionId` (`Session::id`), and
  the `FontId` / `ImageId` / `SoundId` handles its registry mints are
  unique to the process: one minting slotmap per kind hands them out and
  records the owner, and a session's registry is secondary to it. Before,
  two sessions each minted from zero, so an image registered in one and
  drawn through a core of another — two `Core::new()`s in one test is the
  ordinary way — drew whatever the other session had registered first,
  silently. Now it is a miss that behaves exactly as a removed handle does
  (draws nothing, shapes as sans-serif, plays nothing) and raises a
  `foreign-resource` warning naming the handle and both sessions, once per
  handle, on whichever core of the asked session drains `take_warnings`
  next. Noticed wherever a handle resolves — an image node, a text style,
  `play`, an `audio` node, a `remove_*`, the driver's audio backend —
  because the registry keeps the hits and the core reports them. The
  `u64` form of a handle and the C ABI are untouched; a removed handle is
  as silent as it always was.
- **The `env` reading is written down once, and every binding is pinned to
  it** (backlog S7). `schema::ENV_FIELDS` is the one statement of the host
  facts a view reads — `Env`, `WindowEnv`, the derived frame budget and the
  frame facts beside them — under each binding's spelling, and
  `docs/props.md` gained an *Env* section generated from it. The
  restatements are held to the table rather than trusted: `schema`'s tests
  hold the rows to the two structs (an exhaustive pattern, so a new field
  fails to compile until it has a row), kui-lua asserts `view(env)`'s keys
  against the Lua column, test.mjs asserts `ctx.env()`'s against the Node
  column recursively, and kui-ffi holds the header's `kui_env_set` /
  `kui_env_set_window` prototypes to the C column parameter for parameter.
  The corpus's Lua and Node adapters now also assert the env readback on
  every scene, so a frame under custom chrome reports `custom_chrome = true`
  the same way everywhere. Two divergences are recorded as deliberate rather
  than fixed: Node's `nativeControls` stays the `Rect` the core holds where
  Lua and C carry a width/height at the window origin, and
  `frame_budget_ms` stays in the reading, derived, in both bindings that
  have one. No shape changed. The C header's doc that pointed at a
  `kui_env_set_focused` that never existed now names `kui_env_set`.
- **There is exactly one window kind, and asking for another says so**
  (backlog R3, ADR 0004 step 4). This release ships multi-window *without*
  popups, deliberately: `WindowKind` has only `Normal`, so a dropdown taller
  than the window, a menu with nowhere in-window to go and a panel beside
  the app are still not buildable — `FloatConfig::fit` plus a `modal` float
  covers everything that fits inside the window, and its doc comment now
  says the popup it points at is a later release. The line is stated rather
  than left to be found: ADR 0004's Consequences carry a dated amendment and
  the README's Status / next has a Windows group. Nothing can ask for the
  kind that does not exist and quietly get the one that does — a JSX or Lua
  `windows` entry with a `kind` key is **refused** (an entry is plain data
  with a fixed shape, so a key that does nothing is an error, not a dropped
  declaration), and C's `KuiWindowConfig.kind`, which is a `uint32_t` a host
  can fill with anything, still opens a normal window but now raises the new
  **`unknown-window-kind`** warning while doing it. No ABI change.
- **An app can ask for a window's size and focus** (backlog C11 step 5, ADR
  0004 decision 5). `WindowCommand` gained `SetSize { window, size }` and
  `Focus(WindowId)`, queued by `Core::set_window_size` / `Core::focus_window`
  the way `reveal` and `play` queue theirs: the driver applies them on its
  next pump (winit's `request_inner_size` / `focus_window`), a headless one
  never drains, and the size the window actually becomes arrives as the
  ordinary `resize` event. Size is a **request, not a prop** — the user owns
  a window's size once it exists, and a declared size would fight every
  drag of its edge — so a frame that follows changes nothing. In every
  binding: `ui.set_window_size` / `ui.focus_window`, `setWindowSize` /
  `focusWindow` in Node, `kui_set_window_size` / `kui_focus_window` in C,
  `env.set_window_size` / `env.focus_window` in Lua.
  This is the other half of step 3's rule that a window's config is read on
  the opening edge only: a declaration cannot move a live window, so an app
  that wants to needs a verb, and now has one.
  **C's `KuiWindowCommand` gained `width`/`height`** for `KUI_CMD_SET_SIZE`
  (`KUI_CMD_FOCUS` needs only the `window` every command already carries),
  which is the second exercise of the [out] append `size` was put there for
  — the floor stays through `config`, so a host built against ABI 5 keeps
  its reservation, and no host that never calls `kui_set_window_size` can
  receive the verb at all. `KUI_ABI_VERSION` is 6 for the host that skipped
  the check. A `SetSize` carries a bare size rather than a `KuiWindowConfig`
  for the same reason the core's does: a config is what a window opens with,
  and this moves one that already exists.
- **A frame declares which windows exist, and the runner opens them**
  (backlog C11 step 3, `docs/adr/0004-multi-window.md` decisions 4-6).
  `ui.window("palette", WindowConfig::sized(400.0, 300.0))` in Rust,
  `kui_window_declare(ctx, name, &cfg)` in C, `windows: (model) => [...]`
  in a Node loop config (the root box's `windows` prop underneath, the way
  `title` is), a `windows` list beside `window_title` on a Lua root table.
  A window opens on the first frame any window's frame declares it and
  closes on the first frame none does; **the set in effect is the union of
  what every open window's frame declared, plus main** — so a window can
  declare its own child, and closing the parent takes the child with it.
  The core's diff queues `WindowCommand::Open { id, origin, config }` /
  `Close(id)` into the queue every driver already drains, and raises
  `{kind:"window", phase:"opened"|"closed", name, id}` for the app. The
  Rust runner opens a real window per `Open` — a `Pane`: its own surface
  on the shared device, its own `Core` on the shared `Session`, its own
  cursor, caret blink and accessibility adapter — and calls `App::view`
  once per open window per frame, with `ui.window_name()` saying which
  (`"main"` for the launcher's). Node's `runWindowed` does the same:
  `view(model, window)` runs once per name `win.windows()` lists, and a
  single-window app ignores the argument.
  **Config is read on the opening edge and never again.** Re-declaring a
  live window at another size resizes nothing, which is where "the user
  owns a window's geometry once it exists" is enforced: not by refusing
  the new size but by never looking at it. Where two declarations of one
  name disagree on that edge, the lowest declaring window's first one wins
  and **`duplicate-window-config`** says so. **An OS close is reported,
  not undone**: the driver calls `Core::window_closed` (`kui_window_closed`,
  `ctx.windowClosed(id)`), the app gets the `closed` event, and the window
  stays closed while it is still declared — a declaration reopens a window
  only when it *starts*, the same edge `keyFocus` uses. The first version
  of every multi-window app declares its window unconditionally and cannot
  reopen it, so that state has a name, **`window-declared-while-closed`**,
  whose message says the fix: handle the event, stop declaring the name,
  declare it again. Both warnings come from the diff, keyed by the window
  name (there is no node), on the core whose frame declared it.
  The conformance corpus gained a `windows` scene that pins all of it —
  the `Open`/`Close` sequence, the four events, both warnings, the id that
  changes across a lapse — reproduced byte-identically from Rust, Lua, C
  and Node; the report grew a `cmd` line per window command, which also
  pins, for the first time, the `drag` the modal scene's titlebar press
  always asked for. Chrome commands say which window they are about
  (`WindowCommand::Close(id)` and the rest gained a `WindowId`; the C
  header's `KuiWindowCommand` carries it); `Core::windows()` /
  `ctx.windows()` list what is open, `Core::window_name()` /
  `kui_ctx_window_name` / `ctx.windowName()` say which one a core draws.
  Popups (step 4) are not here.
- **Every event says which window it came from** (backlog C11 step 2, ADR
  0004). `WindowId` is an opaque integer the driver assigns — `WindowId::MAIN`
  is 0, apps never build one — and it now reaches every transport:
  `UiEvent::window` in Rust, `window` on a JSX `UiEvent`, `KuiEvent.window`
  in C. It is 0 everywhere until a frame can declare a second window (step
  3), so nothing existing changes meaning.
  It is a **new field and not a second reading of `origin`**, which says
  which *frontend* drew the node. The two were never the same question and
  are further apart since `CExtension` shipped: an extension draws into
  every window there is.
  `WindowEnv` gained `id`, so a view reads which window it is drawing the
  way it reads `maximized` — no new query — and that field is also where the
  event's answer comes from. Producers cannot fill it in (a hit test, the
  edit buffer and the audio queue are all below the level at which a window
  exists), so a `Core` stamps its own `env.window.id` onto everything
  leaving through `handle_input` and `take_pending_events`. One core is one
  window, so that is the whole of it, and step 3 has only to hand each core
  its id — no binding has to route by hand. Node reads and writes it as
  `env().window.id` / `setEnv({window: {id}})`, Lua reads it as
  `env.window.id`.
- **A tab list, a radio group, a menu and a picker list are one Tab stop,
  with the arrows moving inside**
  (`docs/adr/0007-composite-keyboard-patterns.md`, backlog A3). Every tab
  used to be its own Tab stop, which is neither the platform pattern nor
  what a screen reader user expects — the accessibility example alone made
  a keyboard user walk six stops where the platform describes two.
  **Nothing declares a composite.** A container role whose items are
  focusable *is* one: `radioGroup`/`radio`, `tabList`/`tab`,
  `menu`/`menuItem`, `list`/`listItem`, and no other pairing. Derived for
  the reason `pos_in_set` already is — a declared flag can be forgotten on
  something that is one, and set on something that is not, and then the
  keyboard and the platform disagree about the same node. So a tab list
  written before this release becomes one stop with no edit at all, and a
  `group` of buttons stays the ordinary ring it was.
  **The two things kui spells `list` are told apart by how they are already
  built**, with no prop between them: a navigation list is rows *containing*
  links, so the focusable node is the link and every link keeps its stop; a
  picker is rows that are themselves `focusable`, which is a composite.
  Inside one, Left / Up and Right / Down both move — the perpendicular pair
  costs nothing, while refusing it turns a mis-derived axis into a keyboard
  dead end that only a screen reader user finds — Home and End reach the
  ends, and printable characters search the items by name (a buffer aged by
  the frame clock, so input routing stays timeless; with no clock every
  keystroke starts a fresh search). A `radioGroup`, `tabList` and `menu`
  wrap past their ends; a `list` clamps, because a windowed list does not
  have its last row in the tree to wrap to. In a `wrapChildren` container
  the cross-axis pair moves by a wrap line.
  **The core moves the focus and never writes `selected`.** Focus is core
  state — `Core::set_focus` has been its one writer since ADR 0002 — and
  this is the motion Tab already performs with a narrower scope; `selected`
  is app state the view re-declares every frame, so a view that wants
  selection to follow focus writes `selected(ui.is_focused(key))` and it is
  true by construction. For `radio` and `tab`, whose patterns *define*
  selection as following focus, moving also emits the item's own
  `on_click` payload — the event Enter already emits — so an app that
  handles clicks on its tabs answers arrow keys with no new code and no new
  event kind.
  **Three roles and one field, and that is the whole surface.**
  `radioGroup`, `menu` and `menuItem` land at the tail of the role list
  (the only free position under ADR 0006; `menuItem` is a control, so it is
  focusable by its role and an unnamed one is reported by
  `control-without-name`), and `orientation` lands on the access node,
  derived from the container's own `dir` and reported to the platform
  through AccessKit — an announcement, never a gate. **No `PROPS` row, no
  parser in any binding, no new event kind.** C hosts need a rebuild:
  `KuiAccessNode` grew a field, which bumped `KUI_ABI_VERSION` (this
  release moved it twice — see Changed for where it ended up).
  A new `focusable-inside-item` warning names the one configuration this
  leaves impossible — a button inside a list row, which the roving stop
  puts out of reach. A focusable node inside the *container* but outside
  every item, a "+" at the end of a tab bar, keeps its own stop and is not
  reported: the line is drawn at the item, not at the container.
  The corpus gains a `composite` scene and four steps (`arrow`, `home`,
  `end`, `type`), and its node line gains `selected` and `orientation` —
  because "the core never wrote `selected`" is invisible without the first,
  and the C adapter is the only reader of the second. That adapter earned
  its keep on the first run: the three roles had gone in after `group`
  rather than at the tail, which renumbered every `KUI_ROLE_*` from
  `window` on. Rust's own tests were green.
  The accessibility example gains the radio group and a menu opened from a
  button, and `scripts/ax-audit.swift` a composite section — 88 checks on
  macOS 26, including a posted Right arrow moving the *checked* radio (both
  halves) and a posted Tab leaving the whole tab bar in one step.
  **What you can delete:** every `onKeyDown` handler that reimplemented
  arrow navigation over a tab bar or a picker list, and the index
  arithmetic behind it — the core walks a tree it computes and the app
  never had. Nothing else: a tab list already written needs no edit to
  become one Tab stop.

- **The corpus drives a frame under custom chrome, so `windowButtons` is
  covered by a tree instead of a claim** (backlog P9). A scene now declares
  the host window facts it runs under, beside its tree and its steps:
  `Scene::env` is a `WindowEnv`, `conformance::drive` takes it and assigns
  `core.env.window` **before the first frame** — which is why it is a
  parameter rather than something a driver pushes when it gets round to it.
  `titlebar_with`'s native-control inset and `window_buttons`'s early return
  are both read *while that frame builds*, so a fact pushed afterwards would
  compare a different tree.
  It reaches the other three adapters as one new report line —
  `env <customChrome> <maximized> <fullscreen> <controlsW> <controlsH>`,
  written only when a scene departs from `NATIVE_CHROME`, so every scene that
  is not about chrome carries no line and an adapter that sees none drives
  under the defaults it already had. The five numbers are
  `kui_env_set_window`'s arguments in its order, which also decides the
  shape: the controls rect travels as a `w`/`h` extent at the window origin
  rather than a free rect, because that is what all four bindings can
  express. Each adapter reads it back the way it already reads `step` lines.
  **`conformance::UNDERIVED` is now empty.** Its one entry was
  `windowButtons`, exempted because nothing could make
  `widgets::window_buttons` draw. The constant stays and so does the test
  over it: an empty list is a state to hold, not a constant to delete.
  **The `chrome` scene builds both ways an app gets the buttons**, which
  settles a question the old scene had been quietly answering wrong.
  `schema::ELEMENTS` says `windowButtons` is "just the min/max/close
  buttons, **for fully custom titlebars**", and `titlebar_with` appends its
  own cluster by contract — so the scene's `<titlebar>` with a
  `<windowButtons>` child inside it was asking for two clusters in one strip,
  invisibly, because the element drew nothing. The scene now has a
  `titlebar_with` (adaptive path) *and* a hand-laid plain row holding a
  second cluster through each binding's own element (the fully-custom path):
  six buttons in two clusters, compared byte-for-byte across Rust, Lua, C and
  Node. The strip is a plain row rather than a second `window_drag`, since a
  drag handle would derive a second `titleBar` role — a thing to tell a
  screen reader, not a side effect of where the corpus put a box.
  **And the traffic lights got the other scene.** A controls rect makes
  `window_buttons` return early, so one scene cannot show both halves — but
  the second one costs no second builder: `chrome-inset` is the *same tree*
  under `CUSTOM_CHROME_INSET` (custom chrome plus the 78x28 rect
  `MACOS_TRAFFIC_LIGHTS` reports), and each adapter points its existing
  chrome builder at the new name. The env being the only difference between
  the two scenes is exactly what is under test: both clusters go away and the
  title moves from the bare 12pt margin out to the controls' right edge —
  `widgets::titlebar` "adapting per platform by itself", pinned across four
  bindings instead of described.
  One term of that the corpus provably cannot reach, and it was found by
  mutating the widget rather than assumed: the inset is `r.x + r.w`, and
  changing it to `r.w` **does not move the digest**, because the protocol
  carries the controls as a `w`/`h` extent at the window origin (what
  `kui_env_set_window` can express), so `r.x` is always 0 in a scene. An
  inset wrong in the ordinary way *is* caught — a five-pixel error changes
  `chrome-inset`'s digest at an unchanged quad count. The `r.x` term is
  covered by `titlebar_insets_past_the_native_controls` instead, a new
  `kui-core` test asserting 12 / 78 / 82 across the widget's two branches:
  the report carries no coordinates, so a *position* is not a thing a
  checked-in `Expect` can hold, only a quad count is.
  **What you can delete:** the `windowButtons` line from `UNDERIVED`, and the
  habit of reading the `chrome` scene as if its buttons were checked.

- **Node can see `env`, and a headless one can declare it** (backlog B2,
  and the half of P9 that was blocking it). The other three bindings all
  read the host facts a frame driver pushes in — `ui.env()` in Rust, the
  whole `env.window` table in Lua, `kui_env_set` / `kui_env_set_window` in
  C — and Node exposed none of it, in either direction. So a JSX app could
  not write its *own* titlebar: `<titlebar>` papers over the common case by
  reading `env` in Rust on the app's behalf, but the whole design of
  `widgets::titlebar` is "adapt to `env.window` by yourself" — inset past
  the macOS traffic lights, pick the maximize or restore glyph, draw
  nothing under native decorations — and an app wanting tabs or a search
  box in its titlebar had `<titlebar>` with children and nothing else. It
  also could not dim its chrome on `focused`, pace itself on `refreshHz`,
  or lay out differently when `maximized`.
  **`ctx.env()` / `win.env()`** now return
  `{refreshHz, frameBudgetMs, focused, viewport, window}`, with `window`
  as `{customChrome, maximized, fullscreen, nativeControls}`. It is on both
  classes by construction — it went into the `core_methods!` list, so there
  is no second copy to forget. Two spellings differ from Lua's table on
  purpose: `refreshHz` is `null` where Lua omits the key (a stable shape is
  worth more to code that destructures it, and it types as
  `number | null`), and `nativeControls` is the whole rect where Lua
  flattens it to `controls_w` / `controls_h` — that flattening assumes the
  OS controls sit at the window origin, which is true of the traffic lights
  and of nothing in particular. `viewport` is the frame's `{width, height,
  scale}`, the `WindowSize` shape `runWindowed` already uses.
  **`ctx.setEnv({...})`** is the write side, one call where C has two, and
  merging rather than replacing: `setEnv({window: {customChrome: true}})`
  is the whole of "pretend this app draws its own titlebar", and
  `<windowButtons>` starts building the three buttons it has always
  returned early from. It is on `Ctx` only, and the test says so — a
  `KuiWindow`'s runner reports the real window every frame, so a fact set
  on one would be overwritten before the next view ran. Both calls' types
  are generated from the `#[napi]` attributes (P5), so `index.d.ts` needed
  no hand-editing beyond the `Env` / `EnvInput` shapes.
  **What you can delete:** any constant in a JSX app standing in for a host
  fact — a hardcoded 28px inset for the traffic lights, a 16.7ms frame
  budget, a "we're probably focused" assumption. And the reason the
  conformance corpus could not drive `windowButtons` from Node: the adapter
  can declare custom chrome now, which is what P9 was waiting on.

- **`Session`: windows that share a device, a font database and a registry**
  (`docs/adr/0004-multi-window.md` decision 2, backlog C11 step 1). A `Core`
  owned everything, which is fine while there is one of them and wrong the
  moment there are two: a second window would mean a second copy of every
  registered font, image and sound — so an `ImageId` from the main window
  would draw nothing in a palette window — a second audio device for a
  process that has one, and a second `wgpu::Device`, which is not how any of
  the three platforms want to be driven. **`kui_core::Session`** now owns the
  three things whose handles must mean the same in every window: the font
  database shaping resolves against, the resource registry behind `FontId` /
  `ImageId` / `SoundId`, and the audio store with its one command queue.
  `Core::new_in(&session)` builds a window against one; **`Core::new()` is
  unchanged**, and is sugar for a private session of one, so every headless
  test, the conformance corpus, `createApp` and `Ctx` are untouched — nothing
  in any binding had to move. `Core::resources` and `Core::audio` are now
  handles into the session rather than owned stores, with the same methods.
  **`kui_wgpu::Gpu`** is the same idea for the GPU: instance, adapter, device
  and queue behind one cloneable handle, `Renderer::new_in(&gpu, target, w, h)`
  beside the unchanged `Renderer::new` (which makes a private `Gpu` for its
  window), and `Renderer::gpu()` to hand it to the next one.
  Two of the four things ADR 0004 listed stayed per window, and the ADR is
  amended to say why: the shaped-text cache stamps its positioned glyphs with
  the atlas epoch they were packed against, so it is one unit with the
  window's glyph atlas — and the atlas cannot move while `Core::output` lends
  it out as `&mut GlyphAtlas`. That costs a duplicated page and repeated
  rasterization per window, and costs nothing in correctness.
  **What you can delete:** nothing yet — no binding can open a second window
  until C11 step 3. This is the piece that has to exist before one is worth
  opening.

- **A rounded card clips rounded** (`docs/adr/0005-the-paint-vocabulary.md`,
  amended). `Quad::clip` was a rect, so a card with a `radius` that also
  clipped or scrolled showed its children with square corners poking out of
  its rounded ones — a defect that reads as a rendering bug to anyone who
  has used CSS, and the last thing ADR 0005 left in the backlog. A node that
  clips (`clip`, `scroll_x`, `scroll_y`) and has a `radius` now rounds the
  clip its descendants inherit. **There is no new prop**, in any of the four
  bindings: the radius is the clipping node's own and the rule is CSS's
  (`overflow: hidden` under a `border-radius`), so nothing has to be
  declared and nothing has to be lowered.
  **The price was measured rather than estimated**, which is why it is built
  at all — the ADR had priced it as "a bigger bill" than group opacity or
  shadows. With the same bench file on both sides and two copies of the
  baseline binary interleaved to floor the noise at ±0.6%: a 10k-node frame
  that never clips pays about **1%** for `Quad` growing `clip_radius`
  (108 → 124 bytes); a frame where 100 rows clip pays about **3.5%**,
  because the inherited clip is now 32 bytes where it was 16; and the
  per-corner intersect itself is the last **0.9%** on top of that, in the
  shape a real view never declares (it rounds the card, not each row). The
  four floats were priced against a single uniform `clip_radius` first and
  were indistinguishable from it, so the per-corner generality is free — and
  it is needed: a card rounded only at the top is a real shape.
  Three limits, all deliberate and none silent. **Nesting approximates**: the
  inherited clip is one rect and four radii, so where two rounded clippers
  meet, a corner takes the tighter of the two and keeps a radius only while
  neither clipper moved it — an ancestor edge cutting partway into a rounded
  corner leaves a sliver there unclipped. **Hit testing stays rectangular**,
  so a click in the corner of a rounded scroll container still reaches the
  row under it, as it does in a browser. And **culling stays rectangular**,
  so a node that survives only inside a corner's arc is drawn and clipped
  rather than dropped.
  **What you can delete:** the extra inner wrapper with a matching `radius`
  you put inside every rounded scroll container so its first and last rows
  would not square off its corners, and the arithmetic that kept the two
  radii in step.

- **`exit`: a node can leave, not just arrive** (`docs/adr/0005-the-paint-vocabulary.md`,
  backlog C8). `enter` said where a node's slots start the first frame it is
  seen. The mirror image was the one animation the frame model could not
  express: a node the view stops declaring is gone before `finish_frame`
  runs, so a panel could fade in and never out, and views worked around it
  by keeping dead rows in the model with a `dying: bool` and a timer, so the
  app owned a clock and a lifetime that had nothing to do with the app.
  **`exit` is an `Enter` read the other way** — `{ dx, dy, width, height,
  bg, radius, opacity }`, the same shape and the same parser, so it is one
  plain schema row and Lua, JSX and the TS types got it for nothing.
  With a `transition`, the frame after the view stops declaring a node its
  subtree is copied out of the last frame that had it and replayed: **frozen**
  where layout left it (a dying node must not fight the live layout for
  space, which is also how CSS's exit transitions work), **on top and outside
  every clip** (its ancestors may be gone), and **inert** — no hit region, no
  Tab stop, no access row, because it is a picture of a node rather than a
  node. It is dropped when its transition ends, and immediately if the key
  comes back, so a toast dismissed and re-shown never doubles.
  **Bounded, and it says when the bound bites.** `exit` is opt-in per node
  and needs a `transition`; without both, a removed node vanishes at once as
  it always did. No more than 512 nodes may be departing at once — past
  that they vanish, which is exactly what a node with no `exit` does, and
  the first refusal raises an `exit-budget` warning with the sentence that
  fixes it (a list dropping a thousand rows wants `exit` on the list, not on
  every row). `animating()` stays honestly true while a ghost is in flight,
  so the window keeps drawing until the last exit finishes and then idles.
  A driver that never sets a clock gets no ghosts at all: every transition
  snaps without one, and an exit that snaps is a plain disappearance.
  Two limits are notes rather than surprises: `exit`'s `width`/`height`
  resize the departing node's own box only, since re-laying out a frozen
  subtree is the one thing the design rules out; and a spring easing plays
  out as an ease-out, because nothing can retarget a node the view has
  stopped talking about. Benched (`benches/frame.rs`): a 10k-node frame that
  declares no `exit` pays about 5% for `NodeSpec` growing an `Option<Enter>`,
  one exit inside such a frame costs under 2% more, dropping a thousand rows
  that all declare one costs about 180 µs once, and a full 512-node store
  replays in 12 µs a frame.
  **What you can delete:** the `dying` / `removing_at` flag on your model
  rows, the `Instant` beside it, the `retain` that could not run until the
  animation finished, and the frame requests that kept the window awake for
  it. Drop the item when it is gone; the core plays out the picture.

- **The corpus watches a node outlive its frame** (backlog B3,
  `docs/adr/0005-the-paint-vocabulary.md` amended again). `exit` shipped
  with ten corpus scenes and none of them exiting: the ADR argued a
  mechanically lowered row needs no scene, which is true about *lowering*
  and wrong about the thing the corpus actually compares. A ghost is the
  first node that draws while being absent from the hit regions, the Tab
  ring and the access tree at the same time, and nothing but a scene checks
  that four languages keep those three out of step in the same way.
  There is now an **`exit` scene**, and it ends its steps 80 ms into a
  400 ms exit because a report keeps one frame: `fade` is mid-flight and
  draws, text and all, from the previous frame's text list; a press on
  ground covered by both where the node was and where its ghost now is
  emits nothing, where the earlier press on the live node emitted `hit`;
  two Tabs walk past it from one live stop to the next; the access tree
  lists neither. `blink` ran 50 ms and is over,
  `flash` left and came back mid-exit so the frame holds one picture of it
  and not two, and `bulk` is one node past the 512-node budget, so it is
  refused whole with an `exit-budget` warning.
  **Two additions to the corpus protocol**, both of which the ADR had
  declined and neither of which is specific to exits. `Step::Phase(n)`
  (`step phase 1` in a report) is the view changing its mind — a builder is
  now a function of it, and every scene that never changes its tree ignores
  it — and `Step::Time(ms)` (`step time 80`) is the frame clock, without
  which every transition snaps and there is no ghost to see. Neither is an
  input, so each of the four adapters grew one arm that does not call
  `handle_input`. The Node adapter builds its tree per frame instead of once
  (a departure *is* a changed tree), with the corpus fixtures registered on
  the first build that asks for them so its handles are unchanged; the Lua
  adapter seeds the phase as a script global, for which `LuaExtension::lua()`
  now hands a host the interpreter.
  It caught nothing on the first run that compiled, which is the expected
  result for a row three bindings lower by table lookup — and the point is
  the next change to `depart.rs`, which now has four readers instead of one.

- **A C extension, not just a C host: `kui_ffi::CExtension`.** `kui-ffi`
  showed one direction only — C owns `main`, calls `kui_run`, links
  `libkui_ffi`, and the whole app is C. That is an all-or-nothing choice,
  and the interesting case is the other one: an app that already owns its
  window wanting *a panel* from somewhere else. Lua could do that
  (`kui-lua`), C could not, and nothing about `Extension` said why —
  the trait is `name`/`view`/`on_event` over a borrowed frame, and it had
  exactly one implementation, which made it look like a Lua feature rather
  than the binding contract.
  **A plugin is six C functions, two of them required.** `kui_ext_view`
  gets a `KuiCtx *` borrowing the host's frame and calls the ordinary
  `kui_open` / `kui_text` / `kui_close` builders on it; `kui_ext_init` /
  `kui_ext_free` own its state, `kui_ext_name` names it in logs, and
  `kui_ext_abi` is the version check a C host makes for itself - required,
  not optional, because a plugin without it is most likely one built
  against a header from before the symbol existed, which is the mismatch
  the check is for; the host refuses its absence the way it refuses a
  wrong number (backlog S1). The runner
  tags everything the plugin opens with the origin it assigned, which is
  the whole of the isolation: the plugin's clicks reach `kui_ext_on_event`
  and never `App::on_event`, and the host's reach the host.
  **The plugin links against nothing.** Every `kui_*` call is left
  undefined and resolved from the host executable at `dlopen`, the way a
  Lua C module resolves `lua_*`. That asks one linker flag of the host —
  `--export-dynamic`, since GNU ld gives an executable a dynamic symbol
  table holding only what it *imports*, and without it the plugin's load
  fails with `undefined symbol: kui_open`. `crates/kui-ffi/build.rs` passes
  it for this crate's examples; a host elsewhere passes its own, and
  `kui-ffi` gained an `rlib` so it can be one. Nothing else is needed:
  rustc links every object of the rlib, so all 100 entry points are in the
  binary whether the host calls them or not (measured both ways, on glibc
  and on dyld — the flag is the whole difference, and on dyld not even
  that, since Apple's linker exports an executable's globals already).
  **It is checked without a display.**
  `cargo run -p kui-ffi --example c_panel -- --headless` builds a frame the
  way the runner does, finds the plugin's rows through the access tree the
  frame produced, clicks one, and asserts the event carried the plugin's
  origin, that the toggle reached the plugin, and that the host's counter
  did not move — then the same in reverse. `examples/c/panel.c` is
  deliberately the same panel as `crates/kui-lua/examples/panel.lua`.
  **What you can delete:** the fork. Adding a C panel to a Rust app meant
  rewriting the app around `kui_run` — inverting who owns `main` for the
  sake of one subtree — or rebuilding and relinking the whole binary every
  time the panel changed. The panel is now a `.so` the host loads by path.

- **`unknown-prop`: a misspelled prop says so** (backlog P4). Both dynamic
  bindings ended their prop loop by ignoring names they could not place —
  deliberately, since an element's own props (`initial`, `src`, `multiline`)
  ride in the same list as the node's. The cost was that `hoverBg` in a Lua
  table and `onclick` in JSX did nothing and said nothing, which is a bigger
  silent misconfiguration than any the core already reported: it is the
  declaration you wrote being thrown away.
  **The allow-list is the schema tables themselves.** `CUSTOM` rows now
  carry every spelling of their composite (`jsx_names`, `lua_names` —
  `borderW` here, `border = { w=, color= }` there) and `ELEMENTS` rows the
  props each element lowers itself (`jsx_own`, `lua_own`), so
  `schema::known_prop` answers both bindings from one table instead of each
  binding restating what it accepts.
  **Per spelling, not one shared list.** JSX's camelCase and Lua's
  snake_case are separate, so `hover_bg` in JSX is as unknown as `hoverBgg`
  — which is the truth of it, since neither binding reads the other's
  names. That is also what makes the guess in the message reliable: the
  suggestion is the same word in the convention the binding actually takes
  (`did you mean \`hover_bg\`?`), and nothing fuzzier, so it is a confident
  suggestion or none.
  The warning comes from the binding rather than the tree walk, because a
  name nothing claims never becomes part of a node — `Core::warn` takes a
  built `Warning` behind the same `setDiagnostics` gate and the same
  once-per-(code, key) dedup as every check, keyed on (element, name) so a
  misspelling costs one line however many nodes carry it. For Node the
  encoder is the only side that ever sees such a name (without a wire id it
  cannot reach the stream), so it reports what it dropped and `frame` /
  `setView` pass it on.
  **What you can delete:** the JSX prop types kept around only to catch
  typos the runtime would not, and the "why is this prop doing nothing"
  bisect — commenting props out one at a time until the frame changes. The
  frame now names the prop.

- **`initialFocus`: a dialog says which control it opens on**
  (`docs/adr/0003-modal-surfaces.md`, backlog A2). ADR 0003 entered a modal
  at the *first* focusable node in its scope, which makes a destructive
  confirm open on Delete whenever Delete is declared first — and a keyboard
  user who opens it and presses Enter out of habit has confirmed the
  deletion. Platform convention is that such a dialog opens on its safe
  option; `initialFocus` is how a view says so. The first node in the
  modal's Tab ring declaring it is the entry, and **the ring's first when
  none does**, so every dialog that says nothing behaves exactly as it did.
  **It is entry-triggered, which is a third thing from the two rows nearby.**
  `keyFocus` fires when a node *starts being declared* and `<edit autofocus>`
  when nothing else holds the keyboard; this fires when the modal *scope is
  entered*. Only focus outside the scope reaches the rule, so a Tab press
  afterwards stands, a redeclaration every frame is not a second entry, and
  a nested confirm handing focus back into the dialog does not re-read the
  dialog's own entry.
  **`keyFocus` was tried first and does not serve**, though the edge lines
  up: it moves focus while the frame is being built, so the modal remembers
  *it* as the focus it displaced, and the dialog closing then drops the
  keyboard instead of returning it to the button that opened the dialog.
  The new row resolves after the scope is known, which is what lets it
  compose with that rule rather than defeat it.
  The accessibility example's confirm — what `scripts/ax-audit.swift` is
  driven against — now declares it on Cancel.
  **What you can delete:** the declaration-order juggling that kept a
  dialog's safe control first, and any comment explaining why the buttons
  in a confirm are in the order they are in.

- **A modal scene in the corpus, and a `modal-without-name` warning**
  (`docs/adr/0003-modal-surfaces.md`, backlog A1). The ADR named both as
  small things it had left, and they are two halves of one gap: `modal` is
  a schema row, so all four transports lowered it mechanically and nothing
  checked what it *does*. The new **`modal`** scene checks the part that is
  not mechanical. A floated dialog sits over an app with a titlebar, and
  the replay presses the button behind it (no click, one `dismiss` — the
  app is inert), then the titlebar (nothing at all: window chrome stays
  live under a modal, so it is not "outside" and asks for no dismissal),
  then the dialog's own button (a click, because the scope is live), then
  walks the ring, then Escape — so both dismiss reasons are in the event
  list with a live event between them. A report keeps one frame, so the
  three ring steps are chosen to land where only a *scoped* ring can:
  over the dialog's two stops Shift-Tab, Shift-Tab, Tab end on Cancel,
  over the whole tree's three they would end on the button behind. The
  corpus's step vocabulary grew `tab`, `shifttab` and `escape` for it, and
  all four adapters replay them.
  The warning is the other half. A dialog is not named by the text inside
  it — it is not one of ARIA's name-from-content roles — so an unlabelled
  modal is announced as an unnamed dialog to the user it has just moved
  focus to, which is the defect `control-without-name` already catches one
  node over. `modal-without-name` says so, from the same check and on the
  same terms. Its first two hits were our own context menus: the C and
  Node examples gained the `label` it asked for.

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

- **The C ABI has a version, and the structs the library writes carry
  their size** (`docs/adr/0006-c-abi-versioning.md`). `mod abi_parity`
  settles `include/kui.h` against the Rust layout at build time, but a C
  host does not build the library it links — it loads whatever
  `libkui_ffi` the system hands it, and nothing caught an old binary
  against a new one. That is not theoretical: `KuiEvent` is
  caller-allocated (`kui_poll_event(ctx, &ev)` writes into memory the host
  reserved), ADR 0004 appends a `window` to it, and a newer library
  writing the longer struct into an older host's shorter one is silent
  memory corruption in the hottest loop a C host has.
  Two guards, because they catch different things. **`kui_abi_version()`**
  returns the ABI the loaded library implements; **`KUI_ABI_VERSION`** is
  the one the host compiled against; a host compares them for equality
  before its first other call, and `examples/c/counter.c` now does exactly
  that in `main`. The number bumps when the layout of anything the library
  *writes or allocates* changes — never for a field appended to a struct
  the library only *reads*, which is why `KuiSpec` can keep growing a prop
  a release.
  And **the four structs the library writes into your memory now lead with
  a `uint32_t size`** you set to `sizeof` — `KuiEvent`, `KuiDrawData`,
  `KuiTextMetrics`, `KuiScrollGeometry`, each with a `KUI_*_INIT`
  initializer that sets it. The library writes no further than what you
  reserved, so the *next* append to one of them costs an un-recompiled
  host nothing. A `size` below the ABI-1 layout — what a zeroed or
  never-set one looks like — is refused rather than guessed at, and
  `kui_poll_event` refuses before it pops, so a rejected poll leaves the
  event queued instead of swallowing it.
  **Which structs those are is now written in the header**, as a "Who
  writes what" block and an `[in]` / `[out]` / `[out[]]` / `[lib]` tag on
  every struct. That distinction decides whether appending a field is free
  or fatal and it was previously implicit — `KuiSpec` was documented as
  "append-only: the layout is ABI" with nothing saying why the same did
  not hold for `KuiEvent`. The audit also names what a `size` field
  *cannot* fix: the four array out-params (`KuiAccessNode`,
  `KuiAccessRun`, `KuiWarning`, `KuiAudioCommand`) and the `KuiQuad` array
  you stride through `KuiDrawData`, where the stride is the problem and
  the version check is the whole guard. Growing one of those means an
  explicit stride argument, not a quiet append.
  **Nothing to delete**, and that is the honest entry: this is a guard,
  not a feature. It removes no line from any app — it removes a class of
  bug from the ones that upgrade a shared library without rebuilding.

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
  could be seen — a bench of a 10k-row log frame costs ~3.7 ms here, most
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
  rows through it cost **~16 µs instead of ~3.7 ms**, and 100k rows cost
  the same ~16 µs — the frame stops growing with the data. Rows are
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

- **The corpus's coverage is measured now, not declared.** A `Scene`
  carried `custom: &["float"]` as a hand-written list, and the assertion
  over it was bidirectional in the wrong dimension: it caught a `CUSTOM`
  row no scene named and a name no row answered, but nothing checked that
  the scene's *builder* still touched float. A scene could claim a row and
  never exercise it — the claim would pass, and the quad digests, which do
  catch real behavioural divergence, would have nothing to compare on that
  row. `conformance::observe` derives the two sets from the tree each frame
  actually built (a `float` claim needs a node with `layout.float`, a
  `border` claim a visible border width, an `edit` claim an `Edit` node),
  and the Rust adapter asserts derived ⊇ declared, so a stale claim fails
  the build. Three rows leave no mark on the tree and are read off the core
  instead: `title` from the declared window title, `keyFocus` from the
  frame's focus declarations (the focus it takes is indistinguishable from
  a click's), and the `audio` element from a mounted playback (it builds no
  node at all). Derived stays a superset on purpose — `widgets` helpers key
  their nodes, so most scenes exercise `key` without claiming it, and only
  the claims have to be true.
  Two rows the derivation cannot reach are now written down instead of
  silently uncovered. `size` is as far as the tree goes: the frame's text
  list keeps a cache key and a color, not the `TextStyle` it shaped, so the
  predicate is "the scene declared text" rather than "at that size". And
  **`windowButtons` turned out to be a genuinely vacuous claim** —
  `widgets::window_buttons` draws nothing unless `env.window.custom_chrome`
  is set, no binding can declare custom chrome to a headless core (C has
  `kui_env_set_window`, Lua only reads `env`, Node exposes neither), so the
  `chrome` scene calls it and builds nothing. It sits in
  `conformance::UNDERIVED` with that reason, and a test fails the moment a
  scene does exercise it, so the exemption cannot outlive the hole it
  documents. **It did not**: P9, later in this same release, gave every
  adapter a way to declare custom chrome, and `UNDERIVED` is empty.
- **Lua reaches the whole focus runtime** (backlog P3). `env.is_pressed(key)`
  joins `is_hovered` / `is_focused`, and the verbs Node and C always had
  arrived: `env.set_focus(key)`, `env.blur()`, `env.focus_next()`,
  `env.focus_prev()`. Lua was the last binding where focus was
  read-only — a script could style the focused node but not move focus,
  so a list that focuses the row you clicked, a dialog that focuses its
  first field, or a key sink that binds Tab and hands the keyboard on
  could not be written in Lua at all. `set_focus`, not `focus`, because
  alpha.5 shipped `env.focus` as the focused node's *key*, and a name that
  already means a value does not quietly become a verb.
  **`env.focused` and `env.focus` are different facts and both stay.**
  `env.focused` is the *window*'s keyboard focus (a bool, `Env::focused`);
  `env.focus` is the focused *node*'s key, which Node spells
  `ctx.focused()` and C `kui_focused`. Lua puts host facts and runtime
  queries on one table where Node has `env` and `ctx`, so `focused` was
  spent on the window fact before the node reading needed it. The module
  doc now says so at the top instead of leaving two names one letter
  apart to be guessed at.
  `env.focus` is a value the host writes before `view` runs, so it does
  not see a verb called in the same frame — `env.is_focused(key)` is the
  query that does.
- **Node reads the derived pointer shape** (backlog S6). `ctx.cursorShape()`
  / `win.cursorShape()` return what the core resolved for the node under
  the pointer, in the `cursor` prop's own words — `'text'` over an editor,
  `'pointer'` over a button or a `focusable` node, `'grab'` over an `onDrag`
  node (`'grabbing'` through the drag), `'default'` over a plain box or
  with no pointer at all — or whatever that node's `cursor` overrode it
  with. C3 shipped the derivation as `Core::cursor_shape` and
  `kui_cursor_shape`, and a `KuiWindow` was already applying it to the real
  cursor through its runner, but Node had no read, so a JavaScript test
  could not assert C3's deliverable. It went into the `core_methods!` list,
  so both classes have it by construction; `index.d.ts` types it as
  `CursorShape`, derived from the generated `cursor` prop union so the
  query and the prop cannot drift. Lua's `env` does not get it, on purpose:
  what `env` derives per frame — `focus`, `focus_visible`, `is_hovered`,
  `is_pressed`, the scroll offsets — is state a view draws *from*, and the
  cursor is the frame's output, derived from the view rather than fed to it.
  The Rust host applies it, and Lua's tests are Rust and read the core.

### Changed

- **A frame costs about a third less than in alpha.5, and `NodeSpec` is
  224 bytes instead of 728** (backlog C15). Re-measuring the README's
  benchmark table for this release found it 2.4-2.7x worse than the
  numbers it carried, on benches that declare none of what alpha.6 added:
  the frame had been getting more expensive a little at a time, across
  about ten feature commits, and nothing was watching. The cause was not
  any one feature but the struct they all grew — `NodeSpec` is moved by
  value for every node a frame builds, through the builder chain,
  `Core::open` and into `Tree::push`, so its width is a per-node cost paid
  whether or not a node declares the fields. At 728 bytes those moves were
  31% of a frame, in `memmove`.
  The cold fields now live behind four boxed groups — `EventSpec` (the
  seven event payloads), `AnimSpec` (`enter` / `exit` / `keyframes`),
  `AccessSpec` (the declared accessibility properties) and
  `InteractSpec` (hover, pressed and focus backgrounds, the hover group,
  the two sounds). **Nothing changes for a view**: `.on_click(v)`,
  `.role(r)`, `.hover_bg(c)` and the rest of the builders are untouched,
  and so is every binding. Rust code that read the *fields* directly reads
  them through a group accessor instead — `spec.on_click` becomes
  `spec.events().on_click`, `spec.role` becomes `spec.access().role` — and
  writes go through `events_mut()` and friends, which allocate on first
  use. `NodeSpec`'s `PartialEq` is hand-written so that a group left at its
  defaults still equals one that was never allocated.
  Measured on an M3 Pro: `frame_10k_rects` 1.37 ms -> 788 us,
  `frame_1k_typical` 175 -> 116 us, 10k rects with text and hits 1.81 ms ->
  1.20 ms, `deep_nesting_64_levels` 122 -> 79 us, `list_10k_rows_naive`
  5.59 -> 3.67 ms. Benches that touch none of the boxed fields moved just
  as far, which is the point. `size_of::<NodeSpec>()` now has a test with a
  bound on it so the next inline field fails a run rather than a release
  audit; the README's Performance section carries the whole table and what
  is still outstanding.

- **The scene corpus is behind `kui-core`'s `conformance` feature**, off
  by default (backlog S3). `kui_core::conformance` — the scene builders,
  the coverage derivation, the report and the fixture bytes — is test
  infrastructure, and it was compiled into every release build of every
  binding. Now it is on for `kui-core`'s and `kui-lua`'s own tests through
  a dev-dependency, so `cargo test` cannot skip the corpus, and off
  everywhere else; C and Node needed nothing, since their adapters go
  through the public APIs and read the reference as a file. The dump
  command is `cargo run -p kui-core --features conformance --example
  conformance-dump`. A crate that drives the corpus itself adds
  `features = ["conformance"]` to its `kui-core` dependency. Executables
  do not change (the linker already dropped the unreferenced module);
  the rlib is 10 % smaller and `kui-core` compiles in about half the time.
- **A key event carries two codes, and `code` no longer depends on the
  layout speaking Latin** (ADR 0002 decisions 11-12). `code` was whatever
  character the active layout produced, and every keymap in this repo — and
  every one an app would write — is spelled in Latin: `match code { "v" =>
  split }`. On a Cyrillic, Greek, Hebrew or Arabic layout the key US-QWERTY
  prints V on produces `м` / `ω` / `ה` / `ر`, so no arm matched and the app
  was silently keyboard-dead, with the events arriving normally and nothing
  to detect. `code` now follows the layout only while the layout speaks
  ASCII — a chord stays on the key the user can *see*, so Dvorak's `⌥v` is
  on the key printed V, AZERTY's `⌘a` on the one printed A — and falls back
  to the US-QWERTY letter at that position when it does not, which is how
  browsers keep `⌘C` copying on a Russian layout. `text` is untouched: the
  typing view is always the layout's own character.
  **`physical` is new on every key payload**: the US-QWERTY key at that
  position, in the same vocabulary as `code` (`"v"`, `"1"`, `"left"`,
  `"f5"`), so a keymap that wants the finger rather than the label — WASD,
  which `code` turns into ZQSD on AZERTY — switches its match arms by
  changing one word. `KeyPress` gained a `physical` field and
  `KeyPress::from_layout(layout, physical, mods)`, which is where the rule
  lives so no driver reimplements it; `KeyPress::new` sets `physical` to the
  code you named, so every existing construction keeps its meaning.
  **C and Node take the position where they take the code.**
  `kui_input_key_down(ctx, code, physical, kmods, text, repeat)` and
  `kui_input_key_up(ctx, code, physical, kmods)` gained the argument —
  `{NULL, 0}` means "the key I just named", the same NULL-means-derive
  convention `text` already uses, so a host that does not track positions
  edits one line per call and behaves exactly as before. A source break,
  caught by the compiler, and `KUI_ABI_VERSION` stays 6 because 6 has not
  shipped. In Node it is a trailing optional argument on `keyDown` /
  `keyUp`, so no existing call changes at all. Lua needed no change: it
  reads the payload, and `ev.physical` was there the moment the core sent
  it.
- **A press inside a key sink leaves the keyboard on the sink**, and a press
  on window chrome leaves focus alone (ADR 0002 decisions 2-3, refined).
  Making every box with `on_click` focusable put a whole class of app one
  click away from having no keyboard: an app that owns its keys through one
  `on_key` sink and draws clickable surfaces inside it — a multiplexer, a
  canvas with handles, a game with a HUD — lost every chord the first time
  the user clicked one of its own panes, and never got them back, because
  `take_key_focus` is edge-triggered and does not ask twice. A sink is by
  definition the app that owns its keyboard; its panes and buttons are that
  app's surface, focusable enough to be Tab stops without being entitled to
  take the keys away from the thing drawing them. So a primary press now
  resolves to the nearest enclosing sink unless it lands on a keyboard owner
  in its own right — an editor, or a nested sink — which still takes focus,
  caret and all. Separately, a press on window chrome (a `window_drag` strip,
  a window button) returns focus unchanged: moving a window is the
  platform's business, and chrome was already excluded from the Tab ring for
  the same reason. Nothing outside a sink changes: a click on a button in
  ordinary content focuses that button exactly as before.
  `examples/rust/splitmux.rs` was the app this broke, and it now carries the
  test — `cargo test -p kui --example splitmux` drives the real thing
  headlessly through the `App` trait and presses ⌥v after a pane click, a
  tab click and a titlebar grab.
- **`KUI_ABI_VERSION` is 5**, and the C drain loop is a source break (ADR
  0004 decision 12, built by C11 step 3). `kui_take_window_commands` filled
  a `uint32_t` array; a command now names its window and an open carries a
  config, so it is `bool kui_take_window_command(ctx, KuiWindowCommand *)`,
  popping one at a time the way `kui_poll_event` does — an [out] struct
  with the `size` handshake ADR 0006 gave the other four, so it can grow
  later without another break. The rename is deliberate: an un-edited
  host fails to link rather than passing the wrong pointer type through a
  warning. `kui_env_set_window` leads with the window id, which C had no
  way to set before. Both are compile-time breaks; a rebuilt host edits
  two lines, and `examples/c/counter.c` is the worked example.
  `KuiEvent.window` now means it: a context given an id by
  `kui_env_set_window` stamps it on every event.
- **`WindowCommand` carries its window.** `StartDrag`, `Close`, `Minimize`
  and `ToggleMaximize` gained a `WindowId`, and `WindowButton::command`
  takes the window the button was drawn in. A Rust keymap that wrote
  `WindowCommand::Close` writes `WindowCommand::Close(ui.env().window.id)`.
- **`KUI_ABI_VERSION` is 4** — 2 at the last release, moved twice in this
  one: to 3 by the access-node field the composite-keyboard work added, and
  to 4 by `KuiEvent` gaining a trailing `uint32_t window`. A C host
  rebuilds once for both.
  That `window` is **the first field ever appended to an [out] struct**, so
  it is the first real run of the growth story ADR 0006 shipped the size
  handshake for — and it works: `window` sits past `payload`, which is still
  the ABI-1 floor `out_accepts` measures a reservation against, so a host
  built against ABI 3 keeps polling events and simply never sees the new
  field. `size` comes back as the prefix that was filled.
  **A rebuilt C host needs no source change for it** — `KUI_EVENT_INIT`
  already sets `size`. The version still bumps, because a host that skipped
  its `kui_abi_version()` check would otherwise take that short write
  without ever having asked for it. `examples/c/counter.c --headless` now
  asserts both directions: an `offsetof(KuiEvent, window)` reservation still
  polls and keeps its own bytes past that point, and a current one is filled
  all the way. `KUI_WINDOW_MAIN` is the name for 0.
- **`KUI_ABI_VERSION` is 2** (was 1). `KuiQuad` gained `float
  clip_radius[4]`, and `KuiQuad` is a **[lib]** struct — the library
  allocates the array and the host strides it with its own `sizeof` — so per
  ADR 0006 that bumps the version rather than riding on a `size` handshake.
  A C host rebuilt against the new header needs no source change; one that
  is *not* rebuilt is caught by its own `kui_abi_version()` check before its
  first call. Node's `decodeQuads` gained `clipRadii`, and
  `conformance::quad_digest` hashes the four new words, so a binding whose
  mirror of `KuiQuad` missed the field now fails every corpus scene rather
  than none.
- **The Node addon resolver skips a library it cannot load**, instead of
  letting the raw `dlopen` failure escape. `native.cjs` picks the newest of
  the bundled prebuild and the two cargo profiles, and a candidate can exist
  without being loadable — most easily in this repo, where any build that
  also builds kui-node's tests (`--all-targets`, `--tests`, `cargo clippy
  --all-targets`) unifies its `napi/noop` dev-dependency feature into the
  cdylib and leaves an addon at `target/debug/` that Node refuses with
  "Module did not self-register". That file is the newest one, so it won,
  and `npm test` died before its first test with an error pointing at
  `native.cjs` rather than at the build that caused it. Now such a candidate
  is skipped with a warning naming it and the rebuild that fixes it, the
  next one down is used, and when nothing loads the error says which files
  were tried and why each failed rather than "not found". Newest-wins is
  unchanged: the only thing that overrides a newer artifact is that artifact
  being unusable.
- **`docs/props.md` says `window_title`, not `title`, for Lua's window
  title.** The root table has always been read for `window_title`; the
  `title` composite's Lua column claimed `title`, which does nothing. Found
  by the `unknown-prop` allow-list above, which had to write the real
  spelling down.
- **`KuiEvent`, `KuiDrawData`, `KuiTextMetrics` and `KuiScrollGeometry`
  gained a leading `size`**, which is a source break every C host fixes at
  the declaration: `KuiEvent ev;` becomes `KuiEvent ev = KUI_EVENT_INIT;`.
  It is the last time an appended field to one of those four costs a host
  anything, and it is strictly cheaper than the break ADR 0004 had already
  committed to — it just lands before it rather than after.
- **`kui_draw_data` returns `bool`** instead of `void`, so it can report a
  refused reservation like the other three. A host that ignores the result
  still compiles.
- **`Ui::focus_next` / `Ui::focus_prev` defer to the end of the frame.**
  The Tab ring is built from the finished tree and `begin_frame` clears
  it, so stepping from inside a build walked an empty ring and did
  nothing — which is every step a *view* can make, since a view is where
  the tree is still being declared. They now record the step
  (`Core::request_focus_step`, mirroring `pending_reveal`) and it applies
  at `finish`, after the modal scope resolves, so a view can step onto a
  control it is declaring right now rather than waiting a frame for it to
  exist. `Core::focus_next` is unchanged and still steps at once, which is
  what a driver handling a key press between frames wants; Node and C call
  it directly and are unaffected.
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
- **The ten composite props decide once, in kui-core.** `float`, the
  `pad` shorthand family, the overflow bits and `tooltip` were
  reimplemented per binding, line for line: `float_of` in Node's JSON prop
  parser, `parse_float` in Lua, a third at `P_FLOAT` in the binary
  decoder, a fourth spelled as `float_mode` / `float_anchor_x` / ... in
  `KuiSpec` — and that is how `kui_tooltip` in C came to set neither the
  description nor hover tracking, and how the docs named a Lua float key
  (`self`) the parser never read. The decisions moved down:
  `FloatConfig::build` takes the pieces a frontend can extract without
  interpreting them and applies only the ones it saw,
  `FloatConfig::preset` / `preset_at` resolve the four names ("parent",
  "viewport", "below", "above") for every binding including C's new
  `kui_spec_float_preset`, `PadShorthand::resolve` decides what `padX`
  falls back to, `NodeSpec::overflow_bits` decides that scrolling clips,
  and `PropsOut::apply_tooltip` owns all three of a hint's effects at
  once. The bindings keep the part that is genuinely different — reading
  a Lua table, a serde_json map, a binary stream, a C struct — and call
  one constructor with typed scalars.
  Three surfaces widened as a result, because a rule stated once applies
  everywhere: `anchor` takes any preset, not just `parent` / `viewport`
  (`{ anchor: "below", dx: 4 }` hangs below and keeps its 6px gap, since
  an override a frontend did not see leaves the preset's value alone);
  Lua's float table accepts `self` alongside the `self_at` it shipped
  with, so the docs and the parser name the same key; and Lua's `pad`
  table gained the `all` / `x` / `y` the other frontends had.
  The corpus grew with them — the `float` scene now carries a preset with
  one override declared, and `layout` a box whose whole size is what the
  pad fallback resolved to — and immediately earned it, catching a
  disagreement between two lowering paths that this change had introduced.
  The binary protocol is v4: the pad shorthand and each float offset now
  ride the wire as declared, with a set mask, so the JS encoder no longer
  decides what `padX` means, or whether a lone `dx` flattens a preset's
  `dy`, either. `kui.h` gained `kui_spec_float_preset`; `KuiSpec` is
  unchanged.
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
- **The Node corpus adapter can no longer skip where it is meant to run**
  (P7). Its comparison is conditional on a reference report that is
  generated, never checked in, so a missing one is a `t.skip` — which is
  green. That skip is there for the `publish` job, which has no cargo
  target dir and still runs the rest of the suite against the prebuilds;
  nothing asserted the adapter ran in `check`, the job whose whole point
  is that all four adapters meet over one report. A reordered step or a
  `$GITHUB_ENV` export that stopped propagating would have taken the
  check with it, silently. `check` now sets `KUI_CONFORMANCE_REQUIRED`
  and the skip is a failure under it. The C adapter never had the hole:
  it takes the path as an argument and exits 1 when it cannot read it.

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

- **Every example lives under `examples/` now**, one directory per binding:
  `examples/rust/`, `examples/c/`, `examples/lua/`, `examples/node/`. Half of
  them were already there (C and Node) and half were scattered a crate at a
  time under `crates/*/examples/`, so "where are the examples" had two
  answers depending on the language you came in through. A binding's
  directory holds the **whole** example, in whatever languages it takes:
  `examples/c/` is not the C files, it is the C story — C as the host
  (`counter.c`), C as an extension (`panel.c`), and `panel.rs`, the Rust host
  that `dlopen`s it, which used to sit three directories away from the plugin
  it loads. The two halves of the panel now differ only by extension, and the
  same in `examples/lua/` (`panel.lua` + `panel.rs`), where a Lua extension
  has no window of its own so both examples are Rust hosts.
  The cargo target names keep their prefixes — `--example c_panel` and
  `--example lua_panel` are unchanged — because every example binary lands in
  one flat `target/debug/examples/`, where two targets called `panel` would
  collide.
  `examples/README.md` is the map — every example, its `cargo run -p ...`
  line, and what it shows.
  No run command changed: cargo names a target by its `name`, not its path,
  so `cargo run -p kui --example counter` and
  `./target/debug/examples/accessibility` are what they were.
  Two things to know if you vendor the crates rather than the repo. Each
  crate's `Cargo.toml` now names its examples with an explicit `path`,
  because they sit outside the package; and cargo drops a target it cannot
  package, so **the published crates no longer carry their examples** and
  `cargo publish` says so once per example. The repo is where you read them,
  which is where the manifests' `repository` field already pointed.
- **CI audits `Cargo.lock` against RustSec.** A new workflow,
  `.forgejo/workflows/audit.yml`, runs `cargo audit --deny warnings` on
  pushes that touch the lockfile and once a week — weekly because an
  advisory is published against code that has not moved, so a
  commit-triggered check finds it only the next time someone happens to
  push. It is its own workflow rather than a step in `check`: it reads the
  lockfile and needs none of the target dir, fonts or Node that make that
  job expensive. `--deny warnings` means an unmaintained or unsound crate
  fails the same as a vulnerability, and the only way to accept one is an
  entry in
  the new `.cargo/audit.toml` naming what pulls the crate in and what would
  let the line be deleted. The tree is clean today apart from one such
  entry: `ttf-parser` (RUSTSEC-2026-0192) is unmaintained and arrives
  through cosmic-text's `fontdb`, which is not optional, so it is in every
  binding's shipped runtime and there is no version to move to. The job is
  deliberately not in `publish`'s `needs` — a release is cut from a tag, and
  an advisory landing after the last green `main` would otherwise block a
  release whose code nobody had touched. It also passes `--no-yanked`: that
  check costs one sparse-index request per crate and mostly times out on this
  runner, and a timed-out lookup is not a warning, so it was reporting
  success while not running. Off on purpose beats silently absent; a local
  `cargo audit` still does it.

- **C can name `role="line"`, and the header's enums are pinned to the
  lists they mirror.** `KUI_ROLE_LINE` was missing from `kui.h`: three of
  the header's own comments referred to it, `docs/props.md` listed `line`
  among the roles with `KUI_ROLE_*` as its C spelling, and `role_of_code`
  had accepted 22 all along — but no C app could write the constant, so an
  editor that draws its own text could mark the editor and not its lines,
  which is the half of ADR 0001's text outcome an app that owns its text
  needs. The constant is there now, and the enum's leading comment no
  longer says only the first fifteen roles can be declared (`textInput`,
  `multilineTextInput` and `line` are declarable too — that is what a
  custom editor is made of).
  Nothing caught the omission because nothing was looking: the header is
  hand-written, and `every_schema_prop_has_a_c_counterpart` pins prop
  *rows*, sampling every enum-valued row at index 1, so a list that grows
  is invisible to it. `mod abi_parity` — which already generates
  `_Static_assert`s pinning every struct field's offset, size and C type,
  compiled against the header by `examples/c/build.sh` in CI — now also
  emits one per member of the nine enums whose values are indices into a
  list the core owns (`KUI_ROLE_*`, `KUI_CURSOR_*`, `KUI_EXPANDED_*`,
  `KUI_WINDOW_*`, `KUI_START`/`CENTER`/`END`, `KUI_FONT_*`, `KUI_WRAP_*`,
  `KUI_EASE_*`, `KUI_REPEAT_*`). Both ends are pinned: the names are
  declared as a fixed-length array of the list's length, so a role appended
  to `Role::ALL` stops the crate compiling until the header's name for it
  is listed, and the generated C fails on the undeclared identifier if the
  header has not defined it. A new `every_role_round_trips_through_the_c_code`
  covers the third link — that the code C sends arrives as that role in a
  `Spec` — since the header agreeing on a number and the mapping agreeing
  on what it means are different facts.

- **The same pin for Lua, Node and JSX, whose version of that bug was
  worse.** The entry above pins C's role enum by construction; the dynamic
  bindings read `schema::ROLES` at run time and had no equivalent. `ROLES`
  is the declarable subset of `Role::ALL` — 18 of 22, the four omissions
  exactly the roles the core derives — and nothing asserted any of that.
  `role_idx` resolves a wire index through `ROLES` by *name*, so a name
  that no longer parses (a typo, or a role renamed in `access.rs`) did not
  fail and did not warn: every view declaring that role fell back to
  `Role::None`, which takes the node **and its whole subtree** out of the
  access tree. A screen reader would simply stop seeing part of the app,
  with nothing anywhere saying why.
  Two tests close it. `every_declarable_role_name_is_a_real_role` checks
  every name parses and that index `i` still means `ROLES[i]`, so the
  mapping is pinned and not just the spellings.
  `every_role_is_declarable_or_derived` checks the other direction: every
  `Role::ALL` variant is declarable or on the new **`schema::DERIVED_ONLY`**
  list, never neither and never both. That list is the written record of
  which roles a view cannot declare and what derives each, and the test
  keeps it honest the way `the_underived_rows_are_real_and_still_underived`
  keeps the corpus's exemptions honest — it builds a frame and asserts the
  core really does derive every role the list exempts, so a variant merely
  forgotten from `ROLES` cannot be parked there to quiet the failure.
  Mutation-tested, each failing with the role named: a typo'd `ROLES` name,
  a role dropped from `ROLES`, a forgotten role moved into `DERIVED_ONLY`,
  an exemption the core stopped deriving, and a `role_idx` rewritten to
  stop reading `ROLES`.
  **`role_idx`'s fallback is now `Role::Group`.** With the names pinned by
  construction, neither way of reaching it can happen: a drifted spelling
  fails the test rather than a frame, and no transport passes an index it
  has not bounds-checked (Node's binary reader rejects one past the list
  before it builds a value, Node's JSON and Lua resolve a *name*, C carries
  a bounded `Role::ALL` position). So this changes nothing that runs today.
  It changes what a future transport that forgets its check would get:
  hiding a subtree is a destructive answer to "an index I do not have",
  where a group is what the core already derives for a box that is merely
  somewhere focus can land — the node keeps its children, and a wrong role
  is recoverable where a missing subtree is not. The reasoning is in the
  function's doc comment, not only in this entry.

### Deprecated

- **Lua's `env.focus`** — the focused *node*'s key as a value — is
  deprecated. It stays for all of 0.1: nothing about it changes in this
  release, no warning fires, and a script that reads it needs no edit
  until 0.2, where Lua converges on the spelling Node and C already use
  (`ctx.focused()`, `kui_focused`) and `focused` becomes the node reading
  in every binding.
  The name is the whole of it. `env.focus` is a value, `env.set_focus(key)`
  is the verb beside it, and both sit one letter from `env.focused`, which
  is a **different fact** — the *window*'s keyboard focus as a bool
  (`Env::focused`), and the only thing `focused` has ever meant in `env`.
  Node keeps the two apart by having two objects, `env` and `ctx`; Lua has
  one table, so the collision is structural rather than a naming slip.
  **P3 in `docs/BACKLOG.md` is why both exist today**: `env.focused` was
  spent on the window fact before the node reading needed it, and neither
  name can move inside 0.1 — renaming either is breaking, and simply
  dropping `env.focus` would leave the node key unreadable from Lua
  altogether. So this is the announcement, not the change; converging the
  two, and giving the window fact its own unambiguous name, is 0.2's.
  Until then the module doc at the top of `crates/kui-lua/src/lib.rs`
  documents them against each other.

### What you can delete

- **Every keyboard-layout table an app carried to make its chords work** —
  the `match` arm listing `"v"` next to `"м"` and `"ω"`, the "detect the
  layout and pick a keymap" branch, and the per-locale keymap files behind
  it. `code` folds a non-Latin layout onto the position's US letter before
  the app sees it, so one Latin keymap is the whole story; an app that
  wanted the *shape* rather than the label reads `physical` and deletes its
  QWERTY position table too.
- **The `focus(sink)` call an app that owns its keyboard made on every
  pointer event it could see** — and the flag next to it tracking whether
  something inside the app had stolen the keyboard, which no app could
  actually maintain, because focus moving is not an event. A press inside a
  key sink leaves the keyboard on the sink now, and a press on window
  chrome leaves it alone; what remains is the one honest case, an app
  reclaiming focus from a real control outside its own sink.
- **The inner wrapper that re-rounded a rounded scroll container** — the
  extra node with a matching `radius` put inside every clipping card so its
  first and last rows would not square off its corners, and the arithmetic
  that kept the two radii in step when one of them changed. The clipper
  rounds what it clips now.
- **The Lua workarounds for focus you could see but not move** — the
  `key_focus` flag toggled through a script-side variable for one frame to
  simulate a `focus(key)` call, and the hand-rolled "which of my rows is
  next" arithmetic written next to it because `focus_next` did not exist.
  `env.set_focus(key)` and `env.focus_next()` are those, and the second
  one walks the core's ring — chrome, headings and `disabled` controls
  skipped — rather than a list the script maintained.
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
- **The float preset a C host was spelling out.** `float_anchor_x =
  KUI_CENTER, float_anchor_y = KUI_END, float_self_x = KUI_CENTER,
  float_self_y = KUI_START, float_dy = 6` — the five lines that were
  "below" written by hand, and the copy of them that drifted when the
  preset was tuned — are `kui_spec_float_preset(&spec, KUI_STR("below"))`.
  Set `float_dx` or `float_dy` after it to adjust one number without
  restating the other four.
- **The float config a view was completing to override one field of it.**
  `{ anchor: "parent", at: ["center", "end"], self: ["center", "start"],
  dx: 4, dy: 6 }`, written out because naming only `dx` used to flatten
  the gap to zero, is `{ anchor: "below", dx: 4 }`.
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
- **The `onKeyDown` handler that walked a tab bar, a radio group, a menu
  or a picker list** — the Left/Right arms, the index arithmetic that
  wrapped them, the Home and End cases, the type-ahead buffer with its own
  timer beside it, and the `focus(key)` call at the end of each branch —
  together with the `tabIndex`-style juggling underneath it, the one row
  left focusable while its siblings were held out of the ring so the whole
  bar would cost a single Tab stop. A container role whose items are
  focusable *is* a composite: the core walks the ring it already computes,
  and for a `radio` or a `tab` emits the click payload the app was
  handling anyway. Nothing declares it, so a tab list already written
  needs no edit to lose all of that.
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
- **The second copy of every font, image and sound a second window
  cost** — the `add_font_data` / `add_image` / `add_sound` calls replayed
  into the other window's `Core`, the table mapping one asset name to the two
  `FontId`s / `ImageId`s / `SoundId`s that came back, and the "which
  window am I drawing in" branch that picked between them. A `Core` owned
  its registry, so a handle minted in one meant nothing in the other, and
  the second audio device the second store opened was a second device for
  a process that has one. Windows opened against a `Session` share the
  font database, the registry and the one audio queue: register once, and
  a handle means the same thing in every one of them. The map was also the
  bug — a handle from the wrong registry silently drew whatever sat at
  that index — and that is a `foreign-resource` warning now rather than a
  wrong picture.
- **The second process an app split itself into to get a second window**,
  and the IPC it grew to keep one model in two of them: a frame declares
  which windows exist, `view` runs once per open window, and the set in
  effect is the union of what they declare.

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
