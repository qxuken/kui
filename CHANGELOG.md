# Changelog

Every release lists what it adds and, separately, **what you can delete**:
the workaround, the model field or the arithmetic the release made
unnecessary. The second list is the point of the first — a library whose
upgrades remove code from the apps on it is doing the job. A "what you can
delete" line names the *behaviour* the release removed the need for, not the
workaround it guesses an app wrote — a workaround that accreted two purposes
only sheds the one the release addressed, and only the app's own tests know
which of its lines that was.

From 0.1.0-alpha.9 on, a release section opens with **What breaks.** as a
bullet list — one line per break, naming the symbol — and then the paragraphs
that argue each one. The list is for the reader with a build to fix, who
needs to grep for a name before reading 30 KB of prose; the paragraphs are
for the reader deciding whether to upgrade. Earlier sections keep the shape
they shipped with and are not retrofitted (backlog F31, from the alpha.8
field reports).

## Unreleased

**What breaks.**

- **A single-line `<edit>` no longer wraps.** It takes one line whatever
  its box, sizes to its text when its width is `fit`, and scrolls that
  line under the caret when it is not — what `multiline: false` has always
  said it is, and what the `<edit>` row already called "a native field".
  A view that was relying on a field folding onto a second line wants
  `multiline` on it. A multiline editor is unchanged.
- **A `Fit`-width `<edit>` is now as wide as its text.** It was as wide as
  the widest line of its text wrapped at whatever width it had last frame,
  which is a number that only ever went down. Any arithmetic built on the
  old one — a hand-measured width, headroom past the widest character —
  can go; see **What you can delete**.

### Added

- **`{kind: "system"}`, when an OS setting changes** (backlog F40, from
  the two alpha.10 field reports). The appearance, the accent, reduced
  motion or the locale changing while the app is open now arrives on the
  root as an event carrying the whole of `env.system` — same spellings,
  same nulls — one per window that noticed, and the first frame
  establishes the reading rather than reporting it. Exactly the
  bookkeeping a changed viewport already got as `resize`.

  It exists because a redraw is not a re-render for three of the four
  bindings. alpha.10 said the appearance is re-read from `ThemeChanged`,
  "which carries the new theme and also requests a redraw", and that is
  true of a Rust `App`, whose `view` is what the runner calls every frame.
  Node, C and Lua hand the core a tree and keep it: their `view` runs when
  a message changes the model, and there was no message — so a palette
  picked from `env.system.appearance` was the one the first frame picked,
  for the life of the window. Now:

  ```ts
  if (msg.kind === 'system') return { ...m, dark: msg.appearance === 'dark' };
  ```

  A Rust app can keep reading `env` and ignore it. The runner also asks
  for a redraw when taking focus back finds the settings changed, which is
  the one path that had no event behind it.

- `KUI_ABI_VERSION` is 11, and this one the size handshake cannot absorb.
  `KuiQuad` lost `clip[4]` and `clip_radius[4]` and gained a `uint32_t clip`
  in their place: the struct is 96 bytes where it was 124, and every field
  after `kind` moved. `KuiQuad` travels as an array you stride with your own
  `sizeof`, so an un-recompiled host reads element 1 at the wrong offset
  whatever element 0 says — recompile, and read `dd.clips[q.clip]` where you
  read `q.clip` and `q.clip_radius`. `KuiDrawData` gained `clips` and
  `clip_count` to point at.
- `kui_core::Quad::clip` is a `ClipId` (a `u32` index into
  `DisplayList::clips`) and `Quad::clip_radius` is gone. `DisplayList::clip_of(&quad)`
  resolves one; `Clip` is unchanged and still carries the rect and the four
  radii. A backend or a test that read `q.clip.x` reads `dl.clip_of(q).rect.x`.
- `conformance::quad_digest` takes the clip table as a second argument. The
  number it produces is unchanged — the clip is digested resolved, so a
  report from this version compares byte for byte against one from
  alpha.10 — but a binding mirroring the walk (the C and Node adapters both
  do) has to follow the index.
- `decodeQuads` returns `clip` as a number, the index, and no longer returns
  `clipRadii`. `ctx.clips()` and `decodeClips` are the other half;
  `clipStride()` sizes them, as `quadStride()` does quads.

### Changed

- **`Quad` is 96 bytes, down from 124.** The clip — a rect and four corner
  radii, 32 bytes — was on every quad, and nearly every quad of a frame
  shares one with its neighbours: a clip is inherited, and only a clipping
  node makes a new one. It rides in `DisplayList::clips` now with a 4-byte
  index on the quad, the way a fragment's sixteen parameters have ridden in
  `DisplayList::fragments` since ADR 0015 and for the same reason. The 28
  bytes come off the struct emission writes once per quad and then walks
  again in the fade pass, in the backend's upload, and in the whole previous
  frame `depart` keeps for a diff.

  What it is worth, measured interleaved against alpha.10 on an M3 Pro. The
  win is where the quads are, and it is not everywhere: `cells_200x50_warm`,
  which is 94% `CellStore::emit` and therefore almost entirely quad writes,
  goes **57.3 µs → 47.6 µs (−17%)** with under 1% run-to-run spread on
  either side, and `cells_200x50_streaming` **59.0 → 54.0 µs (−11%)**. The
  frame benches move much less, because emission is about a seventh of what
  they do: `frame_10k_rects_with_text_and_hits` 1.24 ms → 1.17 ms (−5.7%,
  against ±3.2% run-to-run), `frame_1k_curves` −4.5% (±3.6%),
  `frame_1k_typical` −3.6% (±3.4%), and `frame_10k_rects` itself −0.7%,
  which on a ±1.7% floor is nothing.

  **It costs a rounded clip about 2.5%**, and that is not noise: measured
  again for the whole branch against `main`, on a quieter machine,
  `frame_10k_rects_rounded_clip` reads **802 → 821 µs, +2.5% on a ±0.3%
  run-to-run floor**. It is the one place the change adds work rather than
  removing it — a frame where every row clips interns a hundred entries and
  every quad under one carries an index the backend then resolves — and it
  is the trade the shrink is: 28 bytes off every quad of every frame
  against a few percent on the frames that clip most. `frame_10k_rects_square_clip`
  is unchanged (−0.1%), so what costs is the rounding, not the clipping.

  Nothing renders differently. The corpus report — every quad digest of
  every scene, across all four bindings — is byte-identical to alpha.10's,
  which is the property the change was built to keep.

  Interning is a constant-time append with a run-length check rather than a
  real intern, and the callers avoid most of the calls: a node whose clip is
  its parent's reuses the index the parent interned without comparing
  anything, so a 10,000-node frame under one clipper interns twice. The
  alternative — scanning the table — is quadratic on the frame shape that
  makes many clips, a screen of width-clamped labels, which narrows the clip
  once per label.

- **A transitioning node hashes its key once a frame, not nine times.**
  `AnimStore::drive` opened with `tweens.entry(key)`, and
  `ease_transitioning` calls it seven to nine times in a row for one node —
  width, height, bg, border, shadow colour, shadow geometry, opacity,
  radius. `AnimStore::node(key)` does the lookup once and hands back a
  `NodeAnim` holding the node's slot array, the clock and the store's
  "another frame is owed" flag; every slot drives through that.
  `AnimStore::drive` stays as a one-slot wrapper, which is what
  `ease_positions` wants.

  Worth less than the profile suggested, and the honest number is the small
  one. `drive` was 31% of `frame_10k_rects_all_transitioning` under
  `sample`(1) — the largest single entry anywhere — but the lookup was only
  about a fifth of that, since after the first probe the entry is in L1 and
  the other eight are a hash and a hit. Interleaved on an M3 Pro:
  `drop_1k_rows_declaring_exit` **157 → 147 µs (−6.1%** against ±0.8%
  run-to-run**)**, `drop_500_rows_declaring_exit` 139 → 134 µs (−3.5%,
  ±0.4%), and `frame_10k_rects_all_transitioning` itself 1.74 → 1.70 ms
  (−2.6%), which on a ±4.4% floor that row cannot resolve.

- **A `Tween` carries its leg's curve, not a whole `Transition`.** 96 bytes
  a slot rather than 104, so a transitioning node's nine slots are 864 and
  not 936. `Transition::repeat` and `delay_ms` belong to the keyframe
  cycle, which is sampled straight off the clock and never reaches a
  `Tween`; they were being copied into every slot of every node and read by
  nobody. A leg keeps `duration_ms` and `easing` alone. `anim.rs` carries a
  size assertion on `Option<Tween>` now, the way `spec.rs` does on
  `NodeSpec`, since a field added here is paid nine times per node forever.

  **It is not measurably faster, and the measurement is why the shrink
  stops here.** Interleaved on an M3 Pro the transitioning frame moves
  between −7% and −2.5% depending on which of four rounds is read, on a row
  whose own run-to-run spread reached ±19%: unreadable, which is the honest
  answer rather than the favourable one. The arithmetic says why. `drive`
  is about 27% of that frame, but it is called 90,000 times in it — ten
  thousand nodes by nine slots — which is **~5 ns, about 15 cycles, per
  call**, for a function that reads and writes a 96-byte struct, branches
  four or five times and eases four lanes. There is no fat left in it; the
  cost is the call count.

  So the two hoists that looked obvious from the profile are not available
  either, and for reasons worth writing down. `last_used` cannot move to
  the node: a node does not drive all nine slots every frame —
  `ease_positions` drives `Slot::Pos` alone, and a keyframed slot is
  sampled instead of driven — so per-slot staleness is what makes a skipped
  slot snap rather than resume. `transition` cannot move either: what a
  `Tween` holds is the curve the *running leg* started under, read in
  `eased_at` one line before the new one replaces it, which is what makes
  retargeting a live tween continuous. Only the two fields no leg reads
  were ever redundant.

- **The access tree is derived only when it would come out different**
  (ADR 0016, decision 3). `Core::access_tree()` rebuilt from scratch on
  every frame a screen reader was attached — about **480 µs on a
  10,000-node frame**, +40% on top of it — although most frames change
  nothing it can see: a pointer crossing hover backgrounds, a colour
  transition, a caret blink. It now hashes what deriving it reads and keeps
  the tree it had when the hash matches.
  `frame_10k_rects_with_access_tree` goes **1.77 ms → 1.39 ms (−21.7%)**
  against a ±3.0% run-to-run floor, the only row in the table that moved.

  The reason it can be both safe and cheap is that the walk is the cheap
  quarter of the work: the same traversal calling the same `semantic`,
  `focusable` and `orientation` costs 105 µs against `build`'s 480, because
  three quarters of that function is constructing nodes and pushing them.
  So the hash calls the real helpers rather than reimplementing what they
  decide, and what could drift is only the fields `build` reads directly —
  gated by 21 cases in `runtime::dispatch::access_cache`, one per input,
  each mutating that input alone and asserting the tree was derived again
  *and* came out different. A view whose editor is a custom one made of
  `line` children answers "rebuild" rather than a hash, because reading its
  inputs amounts to building its node.

  Nothing about the tree changes — only how often it is computed. `WindowRole`
  and `WindowButton` derive `Hash` so that a variant added later is covered
  without anyone remembering to cover it.

### Fixed

- **A field that hugs its text no longer ratchets down to one character**
  (backlog F38). An `<edit>` with a `Fit` width was measured off its text
  buffer *as it stood* — still carrying the wrap width the last frame set
  on it — so the box took the widest wrapped line, the next frame wrapped
  to that, and the fixed point of that loop is one character per line:
  `hello`, typed into a field that hugs its text, was four lines and 11 px
  wide. The intrinsic measurement takes the wrap off first and is cached
  against the text *and its metrics*, which closes a second, quieter half
  of the same bug: a style or scale change re-metrics the buffer without
  touching the text, and a cache keyed on the text alone answered the new
  frame with the old font's size.

  Nothing in this repository hit it, because every example and every test
  gives its editor a `Fixed` or `Grow` width. It was the natural spelling
  that was broken — and, with the field change below, it is the spelling
  that makes a rename field size itself: the core re-lays out the tree it
  was handed when it echoes a keystroke, so a `fit` field grows on the
  frame the character arrives, with no second render from the app.

- **A single-line `<edit>` is a field, not a short document** (backlog
  F41). `multiline` decided whether Enter inserts a newline, what the up
  and down arrows do, and where a seed leaves the caret — and nothing
  about layout: every editor was wrapped to its content width. So a name
  that outgrew its field was drawn on two lines, or four, inside a box
  measured for one. A field now lays out on one line and scrolls it under
  the caret, clipping horizontally to its own content box (and only
  horizontally — the ancestors own the vertical clip, and a descender is
  not what a field is cutting off). Clicks, drag-selection and the access
  tree's character rects all read the same offset emission drew with, and
  a field that scrolls itself no longer makes a scrolling ancestor scroll
  for it.

  A field inside a `scrollX` box already worked, and still does; that is
  a composition an app builds, and this is what the element does alone.

- **A window's `env` is filled in before the first view, not before the
  first frame** (backlog F39). The refresh rate, the four OS settings and
  the window facts were written into the core in the runner's redraw — but
  a host that drives its own loop runs its view as soon as the window
  exists, which is earlier. So every Node app's first `view` read
  `appearance: "unknown"`, no refresh rate and — the sharp one —
  `customChrome: false` in an app launched with `chrome: 'custom'`, which
  is a titlebar drawn wrong on the frame the user sees. For an app that
  only redraws on input that frame is also the last one, which is what the
  `system` event above is for.

- **`<button accent>` typechecks** (backlog F37, from an alpha.10 field
  report). alpha.10 gave the stock button the accent row, and said so in
  its own entry above and in `howto.md`; every binding read it and
  `docs/props.md` documented it. `ButtonProps` in `jsx-runtime.d.ts` did
  not name it, so a TypeScript app got `Property 'accent' does not exist
  on type 'ButtonProps'` for the spelling the release was about, and the
  app that found it moved the colour onto a node beside the button
  instead. The runtime was never wrong — the encoder admitted the row and
  painted it, which is how the report could verify the three backgrounds
  before filing.

  This is the second time that interface fell behind the list it is meant
  to mirror: alpha.9 added `description` to it by hand (F24) and left the
  hand-written list in place. So the fix is the list. `ButtonProps` now
  `extends` what `gen-types.mjs` generates from `BUTTON_ROWS_JSX` — the
  same rows `ElementDef::jsx_rows` hands the encoder — between markers of
  its own, beside the prop types that file already writes. A row added in
  `schema.rs` is a prop on `<button>` one `npm run gen` later, and the CI
  step that reruns the generator and diffs `jsx-runtime.d.ts` is what
  catches the next drift. The button is no wider than it was: it still
  takes exactly `onClick`, `key`, `label`, `description`, `tooltip`,
  `disabled` and `accent`, and `<button bg="…">` is still a type error and
  an `unknown-prop` warning.

  What let it ship: nothing typechecked the spelling. CI does run
  `npm run typecheck` over `examples/node`, on the stated grounds that the
  example "uses every app-facing type" — but no example wrote `accent`, so
  the one check that could have seen the gap had nothing to look at. The
  counter's `+1` carries it now.

### What you can delete

The cast, or the node you moved the accent onto: `accent` is a row on the
stock button in TypeScript as it already was on the wire, and a button that
carries it needs nothing standing beside it.

The width you measured for a rename field, and the headroom you added past
it. A field that hugs its text is `width: "fit"` now, and it grows on the
frame the keystroke arrives — the core re-lays out the tree it was handed,
so there is no frame where the text is wider than the box it is drawn in
and nothing for a margin to absorb. If the box is a fixed size, the second
line the text used to fold onto is gone too: it scrolls.

The palette branch that could only read `"unknown"`. `env.system` is filled
in before your first view, and a change to it is a message — so a view that
picks its colours from the OS can be written the way it reads, rather than
against a reading that never arrived.

Nothing for the two performance changes above, and that is the point: a
view declares no clip and no tween slot, so the only code either one asks
to change is a backend or a test reading the clip off a quad — the
**What breaks** list, rather than a workaround this release made
unnecessary.

## 0.1.0-alpha.10 (2026-09-09)

**What breaks.**

- `KUI_ABI_VERSION` is 10, over two appends, and the Node binary frame is
  version 5 (`slot`). `KuiDrawData` gained
  `fragments`, `fragment_count` and `time` (ABI 9), and `KuiEvent` gained
  `reply_sink` (ABI 10). Both are [out] structs whose leading `size` means
  an un-recompiled host keeps the prefix it knows and only has to bump the
  constant it checks against.
- `kui_reply` reads its sink off the event rather than from a `thread_local`.
  Nothing in a host's or a plugin's *source* changes — recompile both. What
  the bump is really for is a plugin **binary** built against ABI 9: it must
  not be handed an ABI 10 event, and the version check is what refuses it.
  The reason it matters is Windows: a plugin there links against an import
  library, so it may be calling a *different copy* of this library than its
  host, and one `thread_local` per copy meant every `kui_reply` landed where
  nobody read it. A plugin that imports `kui_ffi.dll` now loads into any host
  shipping that DLL, which is the shape a precompiled plugin wants.
- `Fragment` from `@qxuken/kui/jsx-runtime` is a `Symbol` rather than the
  string `'fragment'`, because `<fragment>` is now a real element and the
  two would be the same `type`. `jsx-runtime.d.ts` has always declared it a
  `unique symbol`, so no typed caller could have depended on the string;
  a JS caller comparing `type === 'fragment'` must compare against
  `Fragment` (or `Symbol.for('kui.jsx.fragment')`).
- `Ui::fragment` and `Core::fragment_node` return the node's `Key` rather
  than `()`. Nothing to fix unless a caller bound the result. `Ui::slot_with`
  likewise now answers whether the slot was declared (false for a
  duplicate), so the C context can be the same code as the Rust runner.
- **The C examples build into `target/<profile>/`**, not beside their
  sources: `./target/debug/counter` where alpha.9 had `./examples/c/counter`,
  and `target/debug/panel.so` (`panel.dll`) for the plugin. It is where the
  library they link already is, which is what a Windows host needs and what
  ELF needed an rpath for; the source tree stays clean and `.gitignore` names
  no artifact. On Windows the two plugin shapes swapped names to match what
  each is for: `panel.dll` is the portable one that imports `kui_ffi.dll`
  (the shape you hand to somebody, and both hosts' default), and
  `panel-host.dll` the one that imports the Rust host's executable.
- `KUI_SMOKE_FRAMES` is honoured by a dev build, and by a release build only
  with `--features smoke` — an app you ship should not close its own window
  because the environment it was launched from had a variable set. Nothing
  changes for `cargo run`; `scripts/smoke-windows.ps1` passes the feature
  when it is not smoking dev.
- `Extensions::push_as("", ext)` loads under the extension's own name
  rather than refusing: the "empty means the plugin's own" rule every host
  had was spelling for itself is the core's, once, and a Lua
  `env.add_extension("", path)` — the one host that did not — now agrees
  with the others. The refusal that is left is an extension that names
  itself nothing.
- `widgets::button_with` now colours its label by the luminance of the spec's
  background instead of always white. The stock button and every accent are
  unchanged (white, as before); a caller that passed `button_spec().bg(...)`
  with a *light* colour of its own gets a black label where it used to get an
  illegible white one.
- `Env` gained `system` (a `SystemEnv`), so a Rust host that builds one as a
  struct literal has a field to add; `..Default::default()`, which is how
  every example writes it, is unaffected. No ABI break — the four facts
  arrive through a new `kui_env_set_system`, and a C host that never calls
  it reads them as unknown.
- `setEditText`'s first argument is a name, not only a key: anything that
  is not sixteen hex digits is read as the label an editor's `key` prop
  declares. A call that used to throw `bad id "…"` now holds the text for
  a frame and, if nothing declares that name, warns instead — a suite
  asserting on the throw sees a warning.
- `editText`, `setScroll`, `scrollOffset` and `scrollGeometry` take a
  declared label too, and with it `focus`'s stricter reading of a key:
  exactly sixteen hex digits. A short hex string (`editText('ff')`) used
  to parse as a key and now resolves as a label, which throws when no node
  declared it.
- An extension's own `ui.slot(…)` / `kui_slot` used to declare nothing; it
  now declares a slot like anyone's, filled from the frame's one extension
  list. Nothing could have depended on the old silence except a test
  asserting it, but the name an extension declares is now taken frame-wide,
  so a host declaring the same name sees `duplicate-slot`.
- An extension's replies go to whoever declared its slot rather than always
  to the host. For every extension a host loaded and placed itself — which
  is every extension before this release — that is the host, unchanged.
- `kui-lua` depends on `kui-ffi` (without its `runner` feature), for
  `CExtension` alone: `env.add_extension` needs something to open a shared
  library with, and that loader is the C API's.
- `layout::compute` and `layout::positions` take the scale factor as a last
  argument, so a scroll offset can be snapped to whole physical pixels. Only
  a caller driving the solver directly — which is the crate's own tests and
  nothing else that ships — has anything to pass.
- **Anything moving now moves by whole physical pixels**: a `slide`, an
  `enter`/`exit` offset, a scroll. What a view declares is unchanged and
  where a *still* node sits is unchanged; the last frame of an animation
  still lands exactly on the layout position. What differs is the frames in
  between, which snap to the pixel grid instead of passing through it — at
  100% that is the same number the animation had, at 150% it is a third of a
  logical pixel of stepping, traded for text that stops wobbling (below).

### Added

- **`kui-ffi` has a `runner` feature**, on by default, holding the only two
  entry points that need the GUI runtime — `kui_run` and `kui_run_with`.
  They are 2 of 135 and more than half the weight: the release cdylib is
  **12.3 MB with them and 5.1 MB without** (x86_64-pc-windows-msvc). A host
  that already owns a window and draws the display list itself builds
  `--no-default-features` and ships less than half.

  Not a crate split, because there is nothing to split along: the other 133
  functions are one surface — build, lay out, hit-test, read the access
  tree, take the draw data — and a plugin links against none of it anyway.
  It imports from the host or from `kui_ffi.dll`, and `panel.dll` is 147 KB
  either way.

- **A Node host can load a C extension too**, and place it. `<slot
  name="todos/panel" params={…}/>` is a position an extension fills, in
  place — the JSX spelling of what Rust calls `ui.slot` and C calls
  `kui_slot`. `ctx.addExtension(namespace, path)` loads one into a headless
  context; a window takes `extensions: [{path, namespace?}]` in its options.
  `ctx.extensionNamespaces()` turns an event's `origin` back into the name
  you gave the plugin.

  A plugin's events reach the plugin and its replies reach the app, carrying
  that plugin's origin, exactly as under the Rust runner — a headless `Ctx`
  routes them itself, a window's runner already did. With nothing loaded a
  `<slot>` still places an empty node, so a view can declare its layout
  before it has a plugin to put in it.

  **The mechanism is C shared libraries and only that.** A plugin is a
  `.so` / `.dylib` / `.dll` exporting the seven `kui_ext_*` entry points
  `kui.h` describes, and the *same binary* loads into a Rust, C or Node
  host. There is no script-loads-script path: a Lua *extension* is loaded by
  a Rust host or not at all, and nothing loads a Node one. (A Lua script may
  load a C plugin — see the next entry, added after this one.)

  `slot` is a protocol op rather than a schema element, like `richText`: it
  is a host-side placement call in every binding that has one, not a
  drawable node. The binary frame version is 5.

- **A Lua script can load a C extension and place it**, which makes an
  extension something that can host extensions — the question ADR 0014
  left open ("whether one may offer slots is a decision for the day one
  asks") answered in [its amendment](docs/adr/0014-slots-an-extension-fills-in-place.md).
  `env.add_extension(namespace, path)` opens a plugin and
  `fill { name = "ns/slot", params = … }` is the position it draws in,
  among the script's own children:

  ```lua
  function view(env, slot)
    env.add_extension("todos", "target/debug/panel.dll")
    return column {
      text("the script"),
      fill { name = "todos/panel", params = { title = "todos", on_toggle = { kind = "toggled" } } },
    }
  end
  function on_event(ev)
    if ev.from == "todos" then ... end   -- a reply from the plugin it loaded
  end
  ```

  It is the same mechanism one level down, not a second one. The plugin
  joins the frame's **one** extension list under a namespace of its own, so
  `todos/panel` means one thing to everybody and a name the host already
  took is refused; keys nest, so moving the script moves the plugin; and
  **replies go to whoever declared the slot** rather than always to the
  host, which is ADR 0014's decision 6 read as written. What a script
  answers is what the host hears. The one thing refused is an extension
  filling its own slot, since it is out of the list while it draws: that is
  the new `recursive-slot` warning and an empty position, not a hang.

  `fill` and not `slot` because `slot` is `view`'s second argument and would
  shadow the constructor in the one function that needs it. A reply from a
  plugin the script loaded arrives with `from` naming the namespace; the
  script's own events have none. C plugins can declare a slot from
  `kui_ext_view` too, but cannot load one — a plugin's context has no list.
  Nothing here is a new capability in the security sense: `Lua::new` has
  `package`, so a script could always `package.loadlib`; what it could not
  do is put what it loaded in its own tree.

  [`examples/lua/panel.lua`](examples/lua/panel.lua) is the example, and it
  is now three languages deep — a Rust host, the Lua panel, and
  [`examples/c/panel.c`](examples/c/panel.c) inside it once
  `examples/c/build.ps1` has run. Without the plugin the example is exactly
  what it was.

- **A C host can load a C extension** (ADR 0014's other half, which was
  Rust's alone). `kui_ctx_add_extension(ctx, namespace, path)` loads a
  plugin into a context, `kui_run_with(ctx, …)` opens a window with what a
  context loaded (so a refusal is read the one way, before any window), and
  `kui_slot` fills in place from either — the same call a host already made
  to declare the position. `kui_ctx_extension_error` says why a load was
  refused, `kui_ctx_extension_count` how many are loaded, and
  `kui_ctx_extension_namespace` turns an event's `origin` back into the name
  you gave it. A plugin's events reach the plugin and its replies reach the
  host, exactly as under the Rust runner; a context unloads what it loaded.
  A context that borrows a frame — a plugin's own `kui_ext_view`, or a view
  callback under `kui_run_with` — refuses the call rather than answering it,
  which is what the header has always said and what the code now does too
  (below).

  [`examples/c/host.c`](examples/c/host.c) is the example, and it loads the
  *same plugin binary* the Rust host does: an extension does not know what
  language its host is written in, which is the point of ADR 0014's contract
  being C. Its `--headless` asserts the whole round trip — the slot filled,
  the click reached the plugin and not the host, the reply came back under
  the plugin's origin — and the build scripts run it.

- **A Windows smoke round that opens a real window**, and
  `KUI_SMOKE_FRAMES=n` to make it possible: the runner quits once the main
  window has presented n frames, so every example is a self-terminating
  check with an exit code. `scripts/smoke-windows.ps1` runs all 14 windowed
  examples for 120 frames each (~1.5 s apiece), fails on a non-zero exit or
  a hang, and prints the stderr and any `kui: warning` lines; it is wired
  into `smoke-windows` in `.forgejo/workflows/smoke.yml` after the
  `cargo test` step.

  It covers the layer `cargo test --workspace` cannot: surfaces, swapchains,
  pipelines and everything wgpu validates about them, on the platform's own
  GPU. The first time it ran it found three crashes and a dead feature — all
  four under `### Fixed` below, all four invisible to every headless test in
  the repo. It does not judge what was drawn, only that drawing it did not
  fail; the conformance corpus is still what checks the pixels.

- **A `fragment` element: a box a WGSL function paints**
  ([ADR 0015](docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md)).
  `add_fragment(wgsl)` validates one function and returns a handle;
  `<fragment src params animate>` — `fragment { id=, params=, animate= }`,
  `kui_fragment`, `ui.fragment` — draws a box with it. The app writes

  ```wgsl
  fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32>
  ```

  and reads `in.local`, `in.size`, `in.time`, `in.scale` and sixteen
  positional `params`. kui owns the vertex stage, the node's rounded box,
  the inherited clip (rounded when an ancestor rounds it), the group
  opacity and the blend, so a fragment cannot look unlike a kui node and an
  app cannot get any of them wrong. It lays out, sizes from its spec,
  clips, fades, takes input and **holds children, which paint over it** —
  a gradient card is a `fragment` with a title and a button inside it.
  A source that does not compile is refused at registration with a
  `fragment-rejected` warning carrying the compiler's message *in the app's
  own line numbers*, so a bad shader is a warning a headless test sees and
  never a blank box in a window. Registration is idempotent by source.

  The renderer splits its single instanced draw around each fragment quad.
  Measured, worst case, on an M3 Pro: **about 0.6 µs of CPU per fragment
  and nothing the GPU can see**, so a hundred fragment boxes is 0.7% of a
  120 Hz frame and what a fragment actually costs is its own fill.
  `crates/kui-wgpu/benches/split.rs` is that measurement, and it reads a
  pixel back before reporting so it cannot measure a pipeline that draws
  nothing.

- **`animate`**, a plain prop on any node: ask for another frame after this
  one, every frame the node is declared. What a fragment reading `time`
  needs, and what anything driven by the clock rather than by input needs.
  Opt-in like `exit`, for the same reason — it takes the loop off
  input-driven — and one node asking is enough for the window.

- **What the user set in the OS, as a reading: `env.system`.** Four facts
  a view has had no way to ask for — `appearance` (light/dark), `accent`
  (the OS highlight colour as `0xRRGGBBAA`), `motion` (the reduce-motion
  setting) and `locale` (a BCP-47 tag) — under `env.system` in Node and
  Lua, `ui.env().system` in Rust, `kui_env_set_system` in C.

  **Every one of them can be unknown, and unknown is the default.** The two
  enums spell it (`"unknown"`, `KUI_APPEARANCE_UNKNOWN`) rather than
  leaving it to a null, because "nobody asked the OS" is a third answer and
  not a missing second one: a `reduceMotion` boolean would have had to
  invent a `false` for it, and a view branching on that false would animate
  for a user who asked it not to. `motion` is `"reduced"` / `"full"` /
  `"unknown"` for the same reason — test `=== "reduced"`, not for
  truthiness. The two values are `null` in Node and an absent key in Lua,
  which is the shape `refreshHz` already had.

  **The Rust runner asks the OS itself.** The appearance comes from winit,
  which answers on macOS and Windows and returns nothing on X11 and on
  Wayland without an override; it is read when a window is created and
  thereafter from `WindowEvent::ThemeChanged`, which carries the new theme
  and also requests a redraw, so an input-driven app repaints. The other three
  are `kui`'s own `system_env`: on macOS `NSColor.controlAccentColor`,
  `NSWorkspace.accessibilityDisplayShouldReduceMotion` and
  `NSLocale.preferredLanguages`; on Windows `DwmGetColorizationColor`,
  `SPI_GETCLIENTAREAANIMATION` and `GetUserDefaultLocaleName`; elsewhere the
  locale from `LC_ALL` / `LC_MESSAGES` / `LANG` (`en_US.UTF-8` → `en-US`;
  `C` and `POSIX` name no language and answer unknown) and nothing else
  until a desktop-portal query lands. It costs 6.5 µs measured, so it runs
  on the main thread at startup and again whenever the user has plainly
  been elsewhere — the app taking focus back, or the theme changing —
  rather than on a thread that would have to hop back to AppKit's anyway.
  The macOS half adds no crates: `objc2-app-kit` and `objc2-foundation` are
  the versions `arboard` already pulls in.

  A host that owns its own window — the C, Node and Lua drivers — pushes
  what it can ask the OS for itself, which is why the setter exists
  separately from `kui_env_set`: these change when the user opens a settings
  app, not when a window moves.

  **The core acts on none of it.** A dark appearance repaints nothing and a
  reduced motion shortens no animation, because only the view knows which
  of its colours is the background and which of its animations carries
  meaning rather than decoration. The reading is the feature; the policy is
  the app's.

  The `locale` is carried inline (31 ASCII bytes) so `Env` stays `Copy` and
  a view reading `ui.env()` every frame allocates nothing; a tag that does
  not fit reads back as unknown rather than as a truncated one, and kui
  never parses it.

  **`accent`, the one prop the environment paints.** A row on any node:
  the background comes from `env.system.accent` where the host knows it and
  stays the declared `bg` where it does not. On the stock button it does the
  whole job — `<button accent>` (`button { accent = true }`,
  `KuiSpec.accent`) takes the OS colour, derives its hover and pressed
  shades from it, and picks a black or white label by luminance, so a yellow
  accent is a readable button and not a white-on-yellow one. Opt-in, because
  a prop whose colour depends on the machine is the point here and a
  surprise anywhere else; a host that was never told what the accent is
  paints exactly the button it painted before. The arithmetic is public and
  on `Color`: `mix` (straight sRGB, alpha kept) and `luminance` (WCAG), with
  `widgets::button_palette` and `widgets::readable_on` as the stock button's
  answers built from them.

  **What you can delete:** the model field a host was threading its own
  appearance answer through to reach the view — a driver that asks the OS
  now has somewhere to put the answer that every binding already reads, and
  a windowed Rust app can delete the question too. And the hand-picked
  hover and pressed shades beside any colour you were theming a button
  with: `Color::mix` is what picked the stock ones.

- **`examples/rust/fragments.rs`**: a gradient, a progress ring drawn from
  the prelude's own distance field, a shimmer that reads `time`, and a card
  with a button on a wash.

**What you can delete:** the image you were shipping to fake a gradient, and
the stack of solids you were laying over each other to fake a ramp. Both are
what ADR 0005 pointed at when it declined gradient props, and a fragment is
five lines instead of either. Also the CPU-side animation you were running to
make a shimmer or a spinner move: `animate` plus `in.time` moves it on the
GPU without rebuilding a node.

- **A one-shot cut off says so.** An `audio` node whose sound is still playing
  when the node goes away — or when its `src` changes — is a truncated sound,
  and it was silent: the view guessed a duration, the guess was short, and
  nothing said which. `truncated-playback` names the node and where in the
  sound it was cut ("removed 0.50s into its sound"). It is raised from the only
  place that can know: the core queues the `stop` as data and remembers which
  stops could have cut a one-shot off, the driver applies it and answers
  `Core::audio_truncated` for the handles it found still playing. `finish` is
  both the fix and the opt-out — a released playback is never stopped — and so
  is keeping the node declared until its `ended` event; a `looped` playback and
  an imperative `Core::stop` are never reported. A headless `Ctx` never raises
  it, because nothing there plays: the same fact from that end is
  `audioCommands()` holding a `stop` for the node, which is where a suite
  asserts. (Backlog F34, from the alpha.9 field reports.)

  **What you can delete:** the duration constant a view kept only to hold an
  `audio` node declared for a sound's length — `finish`, or the `ended` event,
  replaces the arithmetic, and the warning now finds the ones that were wrong.

- **A refused play is data, not a stderr line.** The audio device holds 128
  concurrent sounds — kira's number, from `AudioManagerSettings::default()` —
  and a playback released by `finish` holds one of them until its file ends,
  released or not. Past that the device refuses the play, and it used to
  refuse it into `eprintln!`: the playback was never started, so it never
  ended, so a `tag`ged node waiting for `{kind:"sound", phase:"ended"}` — the
  pattern `howto.md` recommends — waited forever, and a `finish` node was
  released to nothing. The refusal now comes back through the same seam the
  end does (`Core::audio_refused`, from the driver): a tagged playback gets
  `{kind:"sound", phase:"refused", playback, tag}` so the view is unstuck and
  can tell a refusal from an end, and a `playback-refused` warning is raised
  on the node that asked either way, so an untagged one is not silent. A
  sound that fails to decode takes the same path, for the same reason. What
  costs a voice is now written where the cost is paid: the `audio` row, the
  `finish` doc and the howto answer.

  There is no kui budget on top of kira's — 128 is the device's number, and
  `MainTrackBuilder::sound_capacity` is where a setting would go if a view
  asks for one.

  **What you can delete:** Any timeout an app kept behind a `finish`ed one-shot's `ended` message to
  cover the case where the message never arrived. The refusal that used to
  drop the message now sends one.
- **`PumpRunner::route_events`**, for a host that drives the core itself.
  A driver that calls `core_mut().press`, answers an access action, or
  drains `take_pending_events` produces events its loop never saw, so
  nothing has given an extension's event to the extension yet. Handing
  those straight to the app delivers a plugin's clicks to the host and
  leaves the plugin deaf to them; this delivers them the way the runner's
  own loop does (`Extensions::route`, ADR 0014 decision 6), calling back
  with what should reach the app. It exists because the extension list and
  the app have to be borrowed at once, which only this crate can do — and
  it is what the Node window driver's own bug was fixed with, below.

### Changed

- **`setEditText` can be given the name the view declares** (backlog F32,
  from the mind map's alpha.9 report). alpha.9 made the call reach the
  editor the *next* frame declares; what it could not reach was the
  *name*. The key is sixteen hex digits, those come from an event the node
  fired, and an editor a rename is opening for the first time has fired
  none — so the one call the release told apps to make was the one call
  they could not spell, and `keyOf`, which the new warning named as the way
  out, existed in Rust and C but not in Node. It would not have helped:
  `keyOf` resolves through the frame being built and then the last
  finished one, and an editor that is closed was in neither — not on a
  first open, and not on a second one either.

  So the name defers the way the text already did. `setEditText('note',
  text)` — Node, either spelling, the way `focus` takes either;
  `Core::set_edit_text_by_label`; Lua's new `env.set_edit_text(key_or_label,
  text)`, which the binding had no form of at all; C's appended
  `kui_edit_set_text_label` (no ABI bump). A label some frame declared
  resolves at once. One nothing has declared is held for the frame that
  declares an editor under it, and that frame's editor takes the text over
  its `initial`. An editor that was retained while its key sat off screen
  — the second open, the abandoned draft — takes it as a `setEditText`
  rather than as a seed, so the model's text wins over what the user typed
  and walked away from, which is the case a seed by key could not express.
  A name nothing declares still drops its text with an
  `edit-text-without-editor` warning, now naming the label it was given.
  Node also gains `keyOf(label)` (the hex key, or null), so the resolver
  the warning names exists wherever a binding hands out hex keys — Lua's
  verbs take the label itself and need none — and `editText`, `setScroll`,
  `scrollOffset`, `scrollGeometry`, `isHovered` and `isPressed` take a
  label like the focus verbs.

  This corrects alpha.9's F25 entry below, the way that entry corrected
  alpha.8's F20. Its "what you can delete" — "Set the text in the `update`
  that opens the editor; the frame that draws it takes the text with it" —
  was true of the holding and false of the naming: the `update` that opens
  an editor had no key for it, so the latch that waited for one was still
  load-bearing, and an app that deleted it on that advice got `bad id`.
  The advice is true now, with the label the view declares in place of the
  key it cannot have.

  **What you can delete:** the frame of waiting, for real this time — the
  `onLayout` latch, the `editReady` flag, the per-node key cache kept only
  so a later `setEditText` would have something to name. Set the text by
  the editor's own `key` prop from the `update` that opens it.

### Fixed

- **Text no longer wobbles inside a box that is moving** (backlog W7). A
  glyph run is placed at whole physical pixels — that is what keeps one
  raster per glyph and text crisp — and its background was placed wherever
  the arithmetic landed, so during a slide the two moved by different
  amounts and the text swung ±0.5 px inside its own card, every frame, for
  the length of the animation. Measured at 150% over a 260 ms `enter`, the
  gap between a card's left edge and its first glyph swung between 21.52 and
  22.49 px.

  Every displacement that moves a *subtree* is now rounded to whole physical
  pixels — a `slide`, an `enter` or `exit` offset, a scroll — so the box and
  the text step together. Three things it deliberately does not do. It does
  not round *positions*: a card laid out at a fractional x stays there, and
  keeps the gap it had, because rounding the position would move every still
  node the moment it stopped animating. It does not place glyphs at
  fractional offsets, which is the other way to make the two agree and costs
  both sharpness and the one-raster-per-glyph atlas. And it does not touch
  the retained scroll offset, which stays exact — a 0.3 px trackpad notch is
  not lost, it accumulates, and a virtual list reading `scroll_offset` sees
  what it always did; only what the children are *moved* by is whole.

  The visible cost is at the low scale factors, which is where the bug was
  worst: one physical pixel is 0.67 logical at 150% and a whole one at 100%,
  against half on a 2× display. A slide steps in those units now instead of
  gliding through them. That is the trade the backlog entry named, taken in
  that direction because a wobble inside a moving card reads as broken and a
  third of a pixel of stepping does not.

  Underneath, the placement rounding is `floor(v + 0.5)` rather than
  `round`, which breaks a .5 tie *away from zero* and so would have jumped a
  pixel where a moving run crossed the origin — the same wobble surviving at
  one line on the screen. It carries a thousandth-of-a-pixel bias with it,
  because at 150% every other whole logical pixel is a half physical one, so
  exact ties are ordinary rather than a corner, and float noise either side
  of one would otherwise pick a different pixel each frame.

- **The C examples build and run on Windows** (backlog W8), from
  `examples/c/build.ps1` — `build.sh`'s round in the same order with the same
  checks: `kui.h` against the Rust layout, `counter.exe` (C as the host),
  `panel.dll` (C as an extension) and the `kui_ext_abi`-deleted mutant the
  host must refuse. It finds `cl` or `clang-cl`, importing the VS environment
  through vswhere if neither is on PATH. Both scripts take `--run` / `-Run`,
  which runs the round they otherwise print — so what CI invokes is one line
  and a local repro is the same line.

  Two things had to change for the plugin half, because a DLL may not leave a
  symbol undefined. `kui-ffi`'s build script now hands link.exe a `/DEF:`
  naming all 135 `kui_*`, so the host exports them and link.exe writes the
  `c_panel.lib` a plugin links against — the list is read from the crate's
  own `pub extern "C" fn kui_*` set, so it cannot go stale. And `kui.h` puts
  a new `KUI_EXT_EXPORT` on the seven `kui_ext_*` declarations
  (`__declspec(dllexport)` on Windows, empty everywhere else), so a plugin
  that includes the header and defines them the ordinary way is exported
  without saying so: `panel.c` is unchanged and builds on every platform.

  One limit is Windows' and stays: an import library names the module it
  imports from, so a plugin built against a host's `.lib` loads into that
  host and no other, where the same `panel.so` loads into either. A host of
  your own exports `kui_*` the same way and ships the `.lib` its plugins
  link to; `kui.h` says so beside the contract.

- **Windows: `fragment` drew one box per frame and crashed on the second**
  (backlog W4). `kui-wgpu` read `min_uniform_buffer_offset_alignment` from
  the *adapter*, which reports 64 on DX12, while the device is opened with
  `Limits::default()` and its 256 — and wgpu validates a dynamic offset
  against what the device asked for, not against what the hardware could
  have done. The frame's second fragment then sat at an offset validation
  refused, and the app panicked on its first paint. It is read from the
  device now. macOS never saw it because Metal's adapter reports 256 and
  the two agreed; the corpus never saw it because one fragment in a frame
  needs no second offset. **What you can delete:** nothing — but an app
  that shipped one fragment per view to keep `fragments` from crashing on
  Windows can stop.

- **Windows: a resize could hand the swapchain a size that panicked wgpu**
  (backlog W5). Moving a window arrived at `Surface::configure` as
  2578×32711 — Windows hands out a nonsense size mid-resize — and a surface
  larger than the device's `max_texture_dimension_2d` panics inside wgpu,
  taking the app with it. Both ends are clamped now (`clamp(1, max)`, where
  only the `1` was there before, for the minimized window that reports
  zero). One frame at the wrong size, and the next real one fixes it.

- **Windows: a plugin's own directory is searched for what it imports.**
  `CExtension::open` uses `LoadLibraryExW` with
  `LOAD_WITH_ALTERED_SEARCH_PATH` against an absolute path, so a panel next
  to the `kui_ffi.dll` it was linked against loads. Without it the search
  starts from the *process* directory — `node.exe`'s, for a Node host — and
  a plugin whose sibling DLL is right there fails with error 126. Found
  loading the C panel from Node. Unix is untouched: a bare `libc.so.6` is a
  name for `dlopen` to search, and resolving it against the working
  directory could load a different library than the one meant.

- **Windows: no C extension could load, ever** (backlog W6). `path_arg`
  encoded the path as UTF-16 and packed the code units into a `CString`'s
  bytes; every ASCII character puts a zero byte in its pair, so
  `CString::new` refused every path there has ever been and
  `CExtension::open` returned `path contains a NUL` before it opened
  anything. ADR 0014's C-extension half has been dead on Windows for as
  long as it has existed. The path is now the platform's own type all the
  way through — `Vec<u16>` for `LoadLibraryW`, `CString` for `dlopen` —
  with nothing smuggled through the other. An interior NUL is still
  refused, which is what the old comment wanted.

- **Windows: dragging a window whose app is animating no longer lurches,
  and holding its title bar no longer freezes the animation** (backlog W3,
  reported twice and now measured on the platform). Windows runs a window
  move inside its own modal loop, where `about_to_wait` — kui's animation
  pacing — never runs, so a transition held still for 2 s drew 0 frames.
  alpha.9 answered that by asking for the next frame from `RedrawRequested`,
  which cures the freeze and costs the drag: every answer blocks on vsync
  *inside* the modal loop with the coalesced `WM_MOUSEMOVE` waiting behind
  it, so a dragged window followed the pointer in 18 px steps 8 ms apart
  where a still one manages 5 px steps 2 ms apart, with stalls to 38 ms and
  jumps to 86 px. It is a timer now (`SetTimer` with a `TIMERPROC`, armed
  while the pane animates): `WM_TIMER` is dispatched wherever the loop is
  running, and like `WM_PAINT` it loses to real input. Measured on the same
  rig, the drag is back to the static window's numbers and a held title bar
  animates at ~64 fps, which is Windows' timer granularity. No subclass, so
  it holds under `Chrome::Native`; the other platforms are untouched.
- **`howto.md` no longer describes the tree it was written against**
  (backlog F33, from both alpha.9 field reports, which found it
  independently). Three of its answers were written in the alpha.8
  worktree that built the page and shipped unchanged in the release that
  closed what they called open: "find out a release happened" said the
  package publishes no `latest` in the tarball whose own `### Changed`
  added one, "reset an editor's text" called `setEditText` before the
  declare "a silent no-op today", and "get the settled frame" said a
  windowed loop can only be polled. All three say what alpha.9 shipped,
  and the version-pinning answer cites that release rather than the
  backlog entry it closed. A new *Test it* answer — "How do I use one
  font in every headless core of a suite?" — retires a wish that has been
  on four reports and answered in a backlog entry no app reads: register
  in `setup`, read the id in `init(surface)`, never a module global.
  A `#[test]` in `kui-core/tests/docs.rs` is what keeps the page honest:
  a backlog id cited as an open gap that the "Closed — index" lists fails
  the workspace test run, as does a `props.md#anchor` no heading answers.
  Prose in `howto.md` cites a backlog id only for work that has not
  shipped.
- **Linux: a C extension could not load into a Node host at all**, which is
  half of what this release added (the other half, `kui_ctx_add_extension`
  from C, was fine). A plugin leaves the whole `kui_*` API undefined and the
  dynamic linker resolves it against what the process has already loaded —
  but only against what is in the *global* scope, and Node loads an addon
  with glibc's default, `RTLD_LOCAL`. So the addon carried all 138 symbols
  and offered none of them: `ctx.addExtension` failed with `undefined
  symbol: kui_slot_params`. `native.cjs` asks for `RTLD_GLOBAL` now
  (`RTLD_LAZY` stays Node's default, so it is one bit). This is the same
  fact `crates/kui-ffi/build.rs` passes `--export-dynamic` for, one loader
  down: being in the image was never the problem, being reachable from the
  plugin is. macOS and Windows are unchanged — Darwin's `dlopen` defaults to
  `RTLD_GLOBAL`, which is exactly why the round trip passed here and only
  CI saw it, and Windows resolves through the import table instead. A test
  reads the flags rather than the load, because on this platform the load
  it governs succeeds either way.
- **An event a Node window's own driver injects reaches the plugin that
  owns it.** `KuiWindow` gained the `extensions` window option in this
  release, and its `take_events` assumed the runner had already routed
  what it was handed. That is true of what comes back out of
  `TreeApp::on_event` — `Shell::route_events` gave an extension its event
  and queued the replies on the way in — and false of everything this
  driver asks the core for itself: a `press`, an access action, the
  pending events `pollEvents` drains. Those never pass the loop, so an
  event whose `origin` was a plugin went straight to JS and the plugin
  never heard its own click. They go through `Extensions::route` now, the
  same one the other three hosts use, reached through a new
  `PumpRunner::route_events` (below) because the list and the app have to
  be borrowed at once.
- **`kui_ctx_add_extension` says no instead of a silent yes.** On a
  context that borrows a frame — a plugin's `kui_ext_view`, or a view
  callback under `kui_run_with` — it loaded the library into a list
  `kui_slot` never consults on such a context, answered true, and
  unloaded it again when the call returned; the `kui_slot` after it drew
  an empty node and said nothing. The header and three doc comments
  already promised a plugin cannot host a guest of its own, so the code
  says so too: the call is refused, with the reason readable through
  `kui_ctx_extension_error`. Refused rather than forwarded to
  `Ui::add_extension`, which would have worked — letting C nest a host
  the way Lua does is a feature to decide on, not a bug to fix here.
- **Windows: `panel.c`'s ` · %d left` compiled as mojibake**, and had
  since `build.ps1` was written. `cl` reads a source with no BOM in the
  machine's ANSI code page unless told otherwise; `/utf-8` joins the flags
  both scripts share, so the two compile the same bytes. It surfaced
  through the mutant the round builds to check a host refuses a plugin
  with no `kui_ext_abi`: that file was written `-Encoding ASCII`, which
  turned the same `·` into `?`, so the file whose whole claim is
  "`panel.c` with one line deleted" differed by more than that — on
  Windows and only there. It is `utf8NoBOM` now.
### What you can delete

Beside the lines under the entries above:

- The `kui_ffi.dll` copied next to a C example on Windows, and the second
  copy beside the plugin. Both are built into `target/<profile>/` now,
  which is where the library they link already is; on ELF the rpath that
  did the same job goes with them.
- A host's own loop giving an injected event to its extensions before
  handing it to the app — `PumpRunner::route_events` is that loop, and it
  is the one the runner uses.
- The AppKit or Win32 query an app made for the four things the OS knows
  about the user: appearance, accent, reduced motion, locale. `env.system`
  carries them, with an explicit unknown for a host that never answers.
- The whole-pixel rounding a view did to its own `slide` offsets to stop
  text swimming inside a moving card. Every displacement that moves a
  subtree is snapped in the core now, scrolling included.
- On Linux, whatever a Node app did instead of loading a C extension —
  there was no working shape to write, so this is more "you can now" than
  "you can delete".

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag on
2026-09-09 on `main`. What follows is what executed on what.

**macOS 26.6.2 (arm64), rustc 1.98.0, Node 26.8.1.** `cargo test
--workspace` passes: **766 tests over 71 suites, 0 failed, 0 ignored**, with
no display, no installed fonts and no GPU. `cargo fmt --all --check` and
`cargo clippy --workspace --all-targets -- -D warnings` are clean. The scene
corpus runs in all four adapters against one reference report: **23 scenes**
— `fragment` new — Rust and Lua through `cargo test`, C through
`examples/c/counter --conformance` (the header at **259 fields and 89 enum
members**, matched against the Rust side by the parity assert), Node through
`npm test` with `KUI_CONFORMANCE_REQUIRED=1` (**100 Node tests**, 0 failed,
0 skipped). The C plugin dlopens into a Rust host and into Node, routes
clicks both ways, fills the slot it is given and delivers its reply; the same
plugin with `kui_ext_abi` deleted is refused against **ABI 10**. `npm run
gen` leaves the three generated files unchanged, `examples/node` installs,
typechecks, builds and runs headless, and `scripts/check-version.sh
0.1.0-alpha.10` passes — after the script that writes the version was taught
about `kui-ffi`, which was added to the workspace after its list of names was
written and so would have shipped one requirement at alpha.9.

**Linux (glibc), in `node:24-bookworm`.** The one platform check that is not
macOS's, and it is here because CI found a defect no test on this machine
could: a C extension in a Node host. The plugin leaves **18 `kui_*` symbols
undefined**, and with the addon loaded `RTLD_LOCAL` — glibc's default —
none of them resolve. Both halves pass there now: the round trip through
`ctx.addExtension`, and the flags check that guards it. macOS cannot fail
this one, which is why the test reads the flags rather than the load.

`scripts/ax-audit.swift` against `examples/rust/accessibility.rs` passes
**106/106 checks**, three times over, the same 106 alpha.9 had. Compile it
once with `swiftc -O` before running it: interpreted, each attribute read
waits on the app's run loop and the menu-focus checks fail on timing alone
(104/106 and 99/106 on two such runs here, 106/106 on every compiled one).

Every host opened a window, presented **120 frames** under `KUI_SMOKE_FRAMES`
and exited 0: the **fourteen** Rust examples (`accessibility`, `bulk_exit`,
`connectors`, `counter`, `editor`, `fragments` — new, and the one that draws
what ADR 0015 added — `gallery`, `modal_editor`, `popup`, `rich_text`,
`splitmux`, `syntax_view`, `toasts`, `waker`), `examples/c/counter`,
`c_panel`, `lua_panel`, and `counter-window.mjs` on Node — seventeen windows
over five hosts, eighteen with the mind map below.

**The round reads the warnings now, not just the exit code**, which is what
found the last four things in this release. `KUI_SMOKE_FRAMES` judges that
drawing did not fail, not what was said while it drew, and
`examples/lua/panel.lua` was drawing nothing at all down one branch while
exiting 0. Every example is clean of warnings as of this tag.

**The drag check on the by-hand list.** `npm run mindmap`, then a synthetic
drag posted through the HID tap with `screencapture` run *while the button
was down*: at "move pan −90,−55" all four cards and their connectors had
moved together by exactly that offset — 180 physical px at 2× — with each
label square in its own card. That is F15 still absent, and it is also W7's
claim under the gesture that would show it worst.

**What this round did not run**, and alpha.9 did: ADR 0009's press-drag-release
into a popup. Nothing on this platform's popup or retarget path changed in
this release — W3 and W5 are Windows' window code — so it is the alpha.9
result that stands, not a fresh one. Windows' own round is
`scripts/smoke-windows.ps1` and ran on the platform for W3–W12; its mixed-DPI
popup case is still unrun.

**The benchmark table below the README's is not refreshed**, for the third
release running and for alpha.7's reason. `scripts/bench-check.sh
v0.1.0-alpha.9` reports **all four guarded rows clean** — `frame_10k_rects`
−1.6%, `frame_10k_rects_with_text_and_hits` −0.6%, `frame_1k_typical` +2.8%
and `deep_nesting_64_levels` −1.1% — but the first run of the four could not
resolve the last of them (±58.5% against itself, which the script reads as
"unreadable" rather than as either answer) and it took a second run of that
row alone, on a quieter machine, to land at ±4.0%. A guarded row is a
*difference* measured back to back and survives that; the README's absolute
medians do not, so they stay at alpha.8's numbers.

The one unguarded row that read slow, `frame_10k_rects_with_access_tree` at
+4.5% against its own ±3.0%, does not reproduce: alone it came back −7.3% at
±9.8%, and the *base* side — alpha.9's code, unchanged between the two runs —
measured 1.63 ms and then 1.71 ms, a 4.9% swing on identical code. The row
cannot resolve a difference that size here, which is the same verdict
`deep_nesting_64_levels` got with the label on it. Nothing in this release
regressed a bench.


## 0.1.0-alpha.9 (2026-09-08)

**What breaks.**

- `Extension::view` takes the `Slot` it is filling, and `Extension::on_event`
  returns `Vec<Value>` — every Rust extension's two methods change shape.
- Lua's `on_event` return value is now read as a reply to the host; a script
  that returned something incidental now sends it.
- `kui::run` and `Launcher::extensions` refuse two extensions of one name —
  give one a namespace with `extension_as`.
- `KUI_ABI_VERSION` is 8: `KuiSpan` gained `bg`, and spans travel as an
  array, so the stride moved — a C host recompiles against the new `kui.h`
  and changes no source (a zeroed `bg` is none).
- An extension's node keys are derived from the slot's full name, so they all
  move once at the upgrade: its tweens restart and its editors are seeded
  from `initial` again on the first frame after it.
- `<button>` / `button { }` / `kui_button_with`: a row the stock button does
  not read is an `unknown-prop` warning naming the rows it does, where it used
  to vanish silently — a suite that asserts `warnings` is empty will see it.
- `setEditText(key, text)` for a key nothing declares by the end of that
  frame is an `edit-text-without-editor` warning where it was silence — the
  same kind of suite sees this one.
- Editor and scroll state kept for keys nothing declares any more has a
  ceiling (256 editors, 1024 scroll entries; longest-undeclared evicted
  first). A draft parked in an undeclared editor under thousands of others
  can be gone when its key returns; a declared one is never evicted.
- `InputEvent` has a new variant, `Commit(String)`, and `Role` a new
  `Terminal` — a Rust `match` on either without a wildcard stops compiling.
  C's `KUI_ROLE_TERMINAL` is appended, so no existing value moved.
- A plain text of 4096 bytes or more with no line breaks is shaped in
  chunks whatever its `wrap` — alpha.8 chunked `wrap: none` only. A long
  wrapped paragraph now breaks its rows from its chunks' positions rather
  than through cosmic-text, so a row can end one break opportunity from
  where it did, and its height is an estimate until its chunks show;
  `maxLines` and `ellipsis` keep the whole-text path.

**An extension is a different shape in all three host languages.** `view`
receives the `Slot` it is filling (Lua's `view(env)` may keep its one
argument; Rust's cannot), `on_event` returns replies rather than nothing, and
an extension names the slots it fills. Two extensions of one name both
loaded before and both drew after the host's view; loading refuses the second
now, because a namespace is what a slot name addresses. See `### Changed`
below.

**An extension's keys move once.** They were `root.index(n)` for whatever `n`
the host happened to leave at its root — which is the defect the slot work
closed, since a host adding a conditional child there rekeyed the whole panel
— and they are the slot's full name now. Everything retained per key crosses
the upgrade once: a tween restarts, a scroll offset returns to zero, an editor
that was holding a draft is seeded from `initial` again.

**A prop the stock button does not read now warns.** `<button>` grew the four
access rows (`label`, `description`, `tooltip`, `disabled`) and, with them, a
declared row list; anything outside that list — `<button radius={4}>` — is
dropped with an `unknown-prop` warning that names the rows the button does
read. It was dropped in silence before, on any element, because the encoder's
allow-list was one flat set of every schema row: a real row on the wrong
element passed the check and vanished. Nothing renders differently; a test
that asserts on an empty warning list is what notices.

### Added

- **The shaped-text cache has a byte budget** (backlog C16).
  `Core::set_text_cache_budget(bytes)` — `setTextCacheBudget` in Node,
  `kui_set_text_cache_budget` in C — with `DEFAULT_TEXT_CACHE_BYTES`
  (64 MB) as the default, and `text_cache_bytes()` / `textCacheBytes()`
  / `kui_text_cache_bytes` reading what the cache holds. Every text a
  frame draws is shaped once and kept; the cache used to empty only on a
  300-frame clock, so a view whose text is new every frame — a terminal
  streaming, a log tailing, a file scrolled through fast — held five
  seconds of everything it had shown: measured at 3.6 GB resident for
  fifty new 200-column lines a frame, and 2.8 GB for log-like ones. Now,
  past the budget, the least recently drawn entries go at the start of
  the next frame, down to three quarters of it, and cosmic-text's
  shape-run cache — the words those lines shaped, with no budget of its
  own — is trimmed with them. **What the last frame drew is never
  evicted**, whatever the budget says: a screenful that does not fit is
  kept whole and re-shapes nothing, the way F26's declared editors are
  never evicted. The clock is unchanged, so an idle cache still empties.
  The budget is charged an estimate per entry (4 KB plus 480 bytes a
  glyph, calibrated against a counting allocator: cosmic-text's own
  `Buffer` is 284 B/glyph of it), and `tests/text_budget.rs` holds a 2 MB
  budget against the allocator — the same stream settles at ~1.6 MB live
  — while `benches/stream.rs` is the frame it costs: fifty new log-like
  lines a frame at ~25 ms, fifty random ones at ~64 ms, and that number
  is cosmic-text's (the same fifty lines shaped through it alone measure
  the same), which is C20's entry and not this one's.

- **A cell grid: a terminal's screen as one node, in every binding**
  (backlog C20). `ui.cells(&grid, spec)` / `cells_keyed` in Rust,
  `<cells rows cols cells={Uint32Array} cursorAt cursorShape cursorColor
  …style rows/>` in JSX (four entries a cell — codepoint, fg, bg, flags —
  in a `Uint32Array` an app keeps and mutates; the stream carries three
  slots a cell), `cells { rows=, cols=, lines={…}, runs={{row, col, len,
  fg, bg, flags}, …}, cursor_at= }` in Lua (a string per row, colour runs
  over it), and `kui_cells` in C with a `KuiCell` array (a new [in-array]
  struct, no ABI bump since nothing existing moved). The node's own rows
  apply: an `onKey` makes it the terminal's sink, and a click or drag on
  it carries `cell: {row, col}` — from the event's own point for a drag,
  the cursor for a click — so the app never divides by a cell size it did
  not choose. Its access role is the new `terminal` (AccessKit's
  `Terminal`, `KUI_ROLE_TERMINAL` appended at the tail), the rows joined
  with trailing blanks trimmed as its value. The cursor prop is `cursorAt`
  / `cursor_at`, since `cursor` is the pointer shape. The grid itself: a `Cell` is a character, `fg` and
  `bg` as `0xRRGGBBAA`, and attribute bits (bold, italic, underline,
  strikethrough, wide) in sixteen bytes; the node lays out at `cols ×
  cell_w` by `rows × cell_h`, the cell width being `M`'s advance snapped
  to whole pixels and the height the style's line height. Its emission
  is a table walk: a glyph is shaped once per character and style
  variant — ASCII by direct index, the rest by map — rasterized into the
  same atlas, and thereafter placed at `col × cell_w` without shaping, so
  a screen whose every cell is new costs what an unchanged one costs.
  Backgrounds coalesce per run of one colour per row, underlines and
  strikethroughs per run of one flag, and the cursor (`Block`, `Bar`,
  `Underline`, in its own colour) paints under the glyph it sits on.
  Measured with `benches/cells.rs` at 200 × 50 cells: **~58 µs warm,
  ~60 µs with every character new each frame**, against ~2.2 ms for the
  same cells as one text node each — the gate was 0.2 ms and flat under
  streaming, and it clears both by a wide margin. A departing grid ghosts
  as its box. Four core tests (the grid, the cursor and lines, a new
  screen shaping nothing new, the click's cell and the terminal's value),
  one each in Node, Lua and the C self-test, and a `cells` corpus scene
  every adapter rebuilds from its own transport and reports identically
  — the same glyphs at `col × cell_w`, the run's one background quad, the
  cursor, the click, the terminal's value.

- **A long line is shaped in chunks, on demand** (backlog C19). A plain
  text with `wrap: none` and no line breaks of its own that is
  `LONG_LINE_BYTES` (4096) or longer — a minified bundle, a log line with
  a JSON blob in it, a base64 field — used to be shaped whole on first
  sight, kept whole, and walked whole at every emission: 662 ms, 47 MB
  and 102 ms a keystroke for 100k characters, measured. Now it is cut
  into ~1 KB chunks after the last whitespace in each window (cosmic-text
  shapes per word and caches per word, so a cut at whitespace loses
  nothing the whole line kept; a whitespace-free run cuts at a grapheme
  boundary), each chunk an ordinary cache entry under the byte budget,
  shaped when emission, a hit-test or a caret query lands in it — the
  first at once, for the line height and the mean advance the others are
  estimated by until they shape. So the width the scrollbar sees can move
  a little as chunks fill in, exact under monospace, which is the
  tolerance a virtual list's uniform rows already accept. Measured with
  the new `long_line` bench on the same 100k characters: **first frame
  ~18 ms** (the screenful, hashing the content, and the chunk cuts),
  **a keystroke ~160 µs** — the
  chunk it lands in reshapes, every other chunk hits by its own content
  — and **a viewport of scrolling ~160 µs**, or a few milliseconds on the
  frame that brings a new chunk on screen. `text_hit` and `caret_rect`
  answer through the chunks (exact in a shaped one, by the mean advance
  in one that never showed); the access tree carries the line's value
  without its runs, the way a tall document's off-screen lines are not
  walked; `measure_text` answers the estimate. Rich text and anything that
  wraps take the path they always took, and so does every text under the
  line, so nothing else moves — `Core::long_lines()` counts what is held.
  Pinned by `tests/long_line.rs`: the first frame shapes the screenful
  and not the line, scrolling shapes what scrolls in, a keystroke
  reshapes one chunk, the queries answer, a short line is untouched, and
  measurement matches layout. **The follow-up the entry named landed the
  next day: a long line wraps.** `wrap: word` or `glyph` on a line past
  the threshold keeps the chunks exactly as they are — shaped once,
  unwrapped, keyed by their content — and breaks the rows itself from
  their glyph positions: each chunk's first row starts where the previous
  chunk's last row ended, so wrapping is a one-direction prefix
  computation over positions and never a reshape, at the break
  opportunities UAX #14 gives (the ones cosmic-text's word wrap takes),
  a piece wider than a row breaking by glyph, trailing whitespace hanging
  as it does elsewhere. A chunk that never showed contributes the rows
  its estimated width makes, corrected when it shapes, so the paragraph's
  height can move a little as chunks fill in, the same tolerance the
  width had. Emission draws the chunks whose rows touch the clip, plus
  one either side, row by row from the unwrapped templates; `text_hit`
  names the row it landed on in `line`, `caret_rect` the row and the x on
  it; `measure_text` with a width answers the rows. A line budget stays a
  property of the whole, so `max_lines` and `ellipsis` keep the whole
  path however long the text. Measured on the same 100k characters as a
  paragraph: **first frame ~36 ms** (a screenful of rows is seventeen
  times the text a single row shows), **a viewport of scrolling ~180
  µs**, ~9 ms on the frame that brings a chunk in. `tests/long_line.rs`
  gained the wrapped half: rows and the screenful, glyph wrap filling
  every row, scrolling down, the queries on rows, and measurement
  matching layout.

- **Underline, strikethrough, and a background per span** (backlog C22).
  `Span::underline()` / `strikethrough()` / `bg(color)` and
  `TextStyle::underline()` / `strikethrough()` — `<span underline
  strikethrough bg>` and `<text underline strikethrough>` in JSX, the same
  keys on a Lua span table and text options, `KUI_SPAN_UNDERLINE` /
  `KUI_SPAN_STRIKETHROUGH` / `KuiSpan.bg` and `KuiTextStyle.decoration`
  in C. Paint, not shape: the decorated text measures like the plain one
  and is a second cache entry, and the rects are built beside the glyph
  templates from the layout — one per run of the span per line, so a
  span's background follows it across a wrap, which is the thing a `bg`
  box around a run could never do. A background paints under the glyphs
  and a line over them, where the face puts it: swash's
  `underline_offset`, `strikeout_offset` and `stroke_size` scaled to the
  glyph's size, read from the run's first glyph. Each span's index rides
  its glyphs as cosmic-text metadata, which is how a rect finds its span
  after layout. `tests/decorations.rs` pins the two lines (spanning the
  glyphs, the underline below the strikethrough, painted after the
  glyphs), a span background covering the span alone and painted before
  them, the wrapped span's two backgrounds and two underlines, and that
  measurement does not move; the Node, Lua and C tests drive their spans
  through their own doors. Node's encoder writes four slots per span now
  (text, flags, colour, bg) with flags 8/16/32 for the three — an
  internal change, since the encoder is the addon's only door.

- **Font features: ligatures off, tabular figures on** (backlog C23).
  `TextStyle::features(FontFeatures)` — `features` on `<text>` / `<edit>`
  in JSX, `features =` in Lua, `KuiTextStyle.features` in C — takes
  OpenType features in one spelling every binding shares: `tag=value`
  pairs separated by spaces or commas, a bare tag meaning 1 and `-tag`
  0, so `"liga=0 calt=0"` keeps a coding font from joining `->` and `!=`
  (what a terminal built on text runs needs to hold its grid) and
  `"tnum"` lines figures up in a gutter. `FontFeatures::parse` is the
  reader, `set(tag, value)` the builder, eight at most, `Copy` like the
  style it rides. It is part of what a text is shaped as — in the cache
  key and on every `Attrs` the core builds, spans and the stock editor
  included — so two texts differing only here are two entries. The C
  field is appended to `KuiTextStyle` the way `KuiSpec.tooltip` was: a
  host predating it passes the shorter struct, no `KUI_ABI_VERSION`
  bump. Pinned by `tests/font_features.rs` (the spelling round-trips, the
  cache key splits) and, where a ligature font is installed, that `->`
  shapes as one glyph by default and two with the features off — the
  Node test does the same and both skip on a runner without the font;
  the Lua test and the C self-test drive the row through their own doors.

- **A thread can wake the loop** (backlog C21). The windowed loop parks
  between events (`ControlFlow::Wait`), and nothing outside the main
  thread could reach it: `Launcher::run` kept its event-loop proxy to
  itself, and `PumpRunner::pump` never blocked, so a Rust app whose data
  arrives on another thread — a PTY reader, a file watcher, an LSP client,
  a socket — either could not redraw or polled on a timer. Now
  `App::setup(&mut self, waker: Waker)` runs once before the window opens
  (a default no-op, so nothing changes shape), `kui::Waker` is `Clone +
  Send`, and `waker.wake()` from any thread asks every window for a frame
  through the loop's own user-event channel, coalesced by its queue. A
  host that owns the loop has `PumpRunner::waker()` and
  `pump_until(deadline)`, which parks on OS events, wakes and the deadline
  together. Measured with the new `waker` example — a thread pushing a
  line every 40 ms and nothing else touching the window: 30 lines, 28
  frames drawn, then the app closed itself (`KUI_WAKER_LINES=30`). Not
  headless-testable, since the wake is the driver's; it joins the by-hand
  round. Node needs none of it: libuv is its timer, by design.

- **IME reaches an editor the app owns** (backlog C17). A custom editor —
  an `onKey` sink drawing `line` rows with `caret` — heard nothing of a
  composition: the preedit went to the stock editor only, the commit is
  the one committed text the platform never reports as a key press with
  `text`, and the OS candidate window opened at the window's origin. Now
  both arrive on the focused sink (or the nearest sink above the focused
  control) as data, tagged like a `key` event: `{kind:"preedit", text,
  cursor:[start,end]|null, tag}` while composing — an empty `text` is the
  composition ending without a commit — and `{kind:"text", text, tag}` on
  the commit. Plain typing is not a commit: the `key` event already
  carries what the press would insert, and the text channel every driver
  sends beside a press stays away from sinks, so nothing is typed twice —
  which is why the commit has its own input, `InputEvent::Commit`
  (`ctx.commit(text)` in Node beside the new `ctx.preedit(text, cursor)`,
  `kui_input_commit` in C, `Ime::Commit` in the winit driver), and a
  focused `<edit>` takes it exactly as it took `Text`. The candidate
  window is anchored for you: `ime_rect` reads a custom editor's caret
  from the `line` carrying `caret` through C18's `caret_rect`, so it
  follows the caret across runs and lines; `imeRect()` in Node and the
  new `kui_ime_rect` in C read it back, the second because a C host
  driving its own window had no way to place the window at all, stock
  editor or not. `TextMsg` and `PreeditMsg` join `CoreMsg`; the two rows
  are in the events table. `tests/ime.rs` pins the sink route, the
  once-only typing, the moving anchor and the stock editor's unchanged
  path; the Node test drives all of it headless. The conformance corpus
  gained `preedit <codepoint>` and `commit <codepoint>` steps (`preedit
  0` ends a composition) and an `ime` scene that composes into a custom
  editor and a stock one in turn, so every binding's adapter — Rust,
  Node, C, Lua — pins the same two routes.

- **A point on text the app owns is a byte offset, and a byte offset is a
  caret rect** (backlog C18). `Core::text_hit(key, point)` /
  `ui.text_hit` — `textHit` in Node, `env.text_hit` in Lua, `kui_text_hit`
  in C — answers `{byte, line}` for the text a keyed node drew, `point`
  being the logical viewport `x`/`y` a click or drag event carries; and
  `caret_rect(key, byte)` — `caretRect`, `env.caret_rect`,
  `kui_caret_rect` — the zero-wide, one-line-tall rect a caret, a
  selection edge or an IME candidate window goes in, a byte past the text
  meaning the end. A node holding several text runs answers across them
  in tree order — a `line` row of syntax runs with a selection box around
  some of them is one line with one byte offset, the way the access tree
  already reads it — so a custom editor turns a click into a caret with
  one call. Before this the only doors were `measure_text` on prefixes
  (shaping and caching each one, O(n) a query, O(n²) a drag) and a
  monospace cell-width assumption, which drifts on the first fallback
  glyph. Answered from the frame that finished — the layout the pointer
  was over — and from the cache entry it shaped, so a query is a lookup:
  the text system records where each live text node was drawn, with the
  keys above it, and swaps that list every frame, so a Rust view
  resolving a click it stashed in `on_event` gets the frame the click
  was made against. `KuiTextHit` and `KuiCaretRect` are new `[out]`
  structs (size-led, no ABI bump). Pinned in all four:
  `tests/text_hit.rs` (a point either side of a glyph's middle, a wrapped
  node per visual line, a paragraph break, scale 2, rich text, the line of
  runs by the row's key and by the wrapper's, and nothing after the node
  stops being drawn), the Node test, the Lua test and the C self-test.

- **Slots: an extension fills a place the host declares**
  ([ADR 0014](docs/adr/0014-slots-an-extension-fills-in-place.md)).
  `ui.slot("fs/panel")` — or `slot_with(name, &params)` — is a position
  among the host's own children, filled then and there by the extension
  the name addresses, as children of the node the host is inside; the
  tree stays preorder because nothing is appended to a closed node. Slot
  names are namespaced and **the host decides the namespace**, the way an
  importer picks an alias: `kui::app(..).extension_as("fs", ext)` loads
  `ext` as `fs`, `extension(ext)` uses its own name, and an extension
  lists the slots it fills in its own vocabulary (`slots = { "panel" }` in
  Lua, `kui_ext_slots` in C, `Extension::slots` in Rust). The same plugin
  loaded twice is two namespaces, two sets of slots and two sets of
  params. Parameters are a `Value` declared every frame and never
  retained — `slot.params` in Lua's `view(env, slot)`, `kui_slot_params`
  in C, `Slot::params` in Rust — and never part of a key. An extension
  **replies** as data: what Lua's `on_event` returns, what a C plugin
  passes to `kui_reply(ev, value)`, what `Extension::on_event` returns;
  each reaches the host's `on_event` with the extension's origin and the
  event's window and key. An extension listing no slots fills `ns/root`
  after the host's view, which is the sequence every extension got
  before; a host may declare `ui.slot("ns/root")` to move it. Four
  warnings: `unknown-slot`, `duplicate-slot`, `unbalanced-extension` (an
  extension that returns with nodes open is closed at the fill's depth,
  so the rest of the host's view lands where the host put it) and
  `extension-view-error` (once per slot, in place of the runner's
  per-frame stderr line). Rust hosts: `Core::frame_with`, `Extensions`,
  `Fill`, `Slot`. C: `kui_slot`, `kui_slot_name`, `kui_slot_namespace`,
  `kui_slot_params`, `kui_reply`, `kui_ext_slots` — functions only, so
  `KUI_ABI_VERSION` stays 7 under ADR 0006's rule. Both panel examples
  place the panel in a slot, hand it a title and a reply template, and
  count the replies.

- **A window says when it has painted, and when it has stopped moving.**
  The loop `runWindowed` builds (`WindowLoop`) gains two promises:
  `await app.frame()` resolves after the next pump has painted, and
  `await app.settled(maxMs = 10_000)` resolves the first time a pump —
  `win.pump()` and the `step()` after it — leaves `animating()` false with
  no effect unflushed, with the wall-clock milliseconds it waited. It is
  `runOut` for a window, and it is a promise rather than a loop for the
  reason `advance` refuses a wall clock: a window's clock is the wall's, so
  a test cannot step time forward itself and the driver's pump is the only
  thing that can say a frame happened. Both are answered from inside that
  pump — a list of waiters it drains at the end of every turn — so a throw
  in the pump rejects them instead of hanging, as does the window closing.
  A cap resolves rather than throws, with `animating()` still true, exactly
  as `runOut` returns `maxMs`: a window whose view holds a `repeat`
  keyframe never settles, and the number says so. Headless is unchanged and
  keeps `runOut`; asking a loop that holds its own clock for either promise
  is refused by name, since nothing would ever answer it.

  **What you can delete:** the sleeps around a windowed test.
  `await new Promise(r => setTimeout(r, 400))` before reading `win.quads()`
  or `app.accessTree()`, and the helper that wrapped it. The app that
  reported this (backlog F30) had three, of 400, 300 and 300 ms; they are
  three `await`s now, and its `npm run smoke` finishes about a second
  sooner with the same assertions passing.
- **An `<audio>` one-shot can be told to finish, so a view need not guess
  the asset's length.** Presence was the whole of playback: a declared
  `audio` node plays, a removed one stops. That is right for a bed of
  music and wrong for a chime — the view decides how long the node stays
  declared, and only the file knows how long the sound is, so an app
  played its alarm behind `m.now - m.alarmAt < CHIME_MS` with
  `CHIME_MS = 6_000` "picked by guessing at the asset's length": too short
  cut the sound off, too long replayed it on an unrelated re-declare, and
  nothing checked either way. `finish` changes what *gone* means and
  nothing else — `<audio src={id} finish/>`, `audio { finish = true }`,
  `KuiAudio.finish`, `AudioSpec::finish()`. The node's removal **releases**
  the playback instead of stopping it: no `stop` reaches the driver and the
  sound plays itself out. Declaring the node for one frame is now enough to
  hear a sound whole. Three edges, each pinned: a `looped` playback is
  still stopped on removal, since release is meaningless where there is no
  end to reach; a changed `src` still restarts, because that is a
  replacement rather than a departure; and a `tag` still reports
  `{kind:"sound", phase:"ended"}` after the release, which a `stop` would
  have cancelled. (A playback that is `paused` when its node goes has
  nothing to finish, so pause and release do not combine — stop it
  instead.) `KuiAudio` grew the field at its end, where a C host that
  predates it writes nothing and reads the old behaviour out of the zeroed
  tail, so `KUI_ABI_VERSION` stays 7. The corpus `media` scene declares
  three playbacks and drops two of them in a second phase, and the report
  gained an `audio` line per command, so all four bindings are diffed on a
  command list that shows a `stop` for the plain removal and nothing at all
  for the released one.

  **What you can delete:** the guessed duration and the window around it —
  a `CHIME_MS` constant and the `now - alarmAt` comparison gating the
  `<audio>` node, plus whatever kept `now` fresh for it. Declare the node
  when the sound should start and stop declaring it on the next frame. If
  the view needs to know when the sound is over — to re-arm, or to clear a
  flag — that is what a `tag` and its `ended` message have always been, and
  it survives the release; it is a real message rather than a constant that
  was right on one machine.

### Changed

- **An extension's keys are stable across what the host builds around
  it.** They were `root.index(n)` for whatever `n` the host happened to
  leave at the root, so a host adding a conditional child there rekeyed
  the whole panel: tweens restarted and an editor keyed under the panel's
  unkeyed root was handed `initial` again. Under a slot — the reserved
  `ns/root` included — they are keyed by the slot's full name and nothing
  else. An existing extension's keys move once, at the upgrade.
- `Extension::view` takes the `Slot` it is filling; `Extension::on_event`
  returns `Vec<Value>`; Lua's `view` receives the slot as a second
  argument (a `view(env)` script never sees it) and its `on_event`'s
  return value is now read; `kui::run` and `Launcher::extensions` refuse
  two extensions of one name — give one a namespace with `extension_as`.
- `quads()`, `access()` and the README's window example now say how a real
  window is driven: `access(key, action)` is the one synthetic input it
  takes, and `click`, `type` and `key` are refused there. The answer was
  only on the two methods that refuse, which is not where a smoke test
  reading `win.quads()` looks (backlog F28).
- **A release is discoverable: every alpha also takes the `latest`
  dist-tag.** The npm registry held one tag, `alpha`, because the publish
  step gave a prerelease its identifier and `latest` only to a plain
  version — which has never existed. `npm view` defaults to `latest`, and
  against a package without one it prints nothing and exits 0, so `npm
  view @qxuken/kui version` answered nothing, `npm outdated` omitted the
  package and a bare `npm install @qxuken/kui` had no version to resolve:
  an app could be running a release it had no way to learn it was running.
  The publish step now follows the tagged publish with `npm dist-tag add
  @qxuken/kui@<ver> latest`, guarded on that version sorting highest among
  the ones the registry already holds (npm's own semver does the sort), so
  a stable release, once there is one, keeps `latest` to itself. The tag
  itself appears with the first publish that runs the new step. Both
  READMEs also say what a range does and does not do: `^0.1.0-alpha.8` and
  `~0.1.0-alpha.8` admit every later alpha of the same `0.1.0` — that is
  npm's semver, not a difference between the two spellings — so a range is
  a floor, and an app that wants the version it tested writes it exactly
  and commits its lockfile.

- **`setEditText` reaches the editor the next frame declares** (backlog
  F25, from the mind map's alpha.8 report). `EditStore::set_text`
  was `if let Some(state) = …` over the editors that exist, and the only
  thing that creates one is `text_edit` in the frame builder. So an editor
  a rename opens does not exist until the frame *after* the `update` that
  opened it, and `setEditText` called from that `update` — the natural
  place, the turn that puts the name in the model — matched nothing and
  fell through: no error, no warning, no text. The field opened with
  `initial` and the app was left concluding that `setEditText` is
  unreliable. Now the text is held for one frame and seeds the editor the
  next frame declares under that key, over its `initial`. The caret lands
  where the call leaves it — at the end, document or not, because a held
  call is that call arriving where it can land and not a second kind of
  `initial`. Held for that one frame only: a key nothing declares on it
  drops its text and raises the new `edit-text-without-editor` warning, so
  a view that spells the key differently is a line rather than a field
  that quietly opens with the wrong text. The code is declared in
  `diag.rs` like every other, so `props.md` and `index.d.ts`'s
  `WarningCode` carry it; `Core::set_edit_text`, the `<edit>` row,
  `kui_edit_set_text` and the Node method all say what the call reaches.

  This also corrects alpha.8's F20 entry below. Its "what you can delete"
  said the `onLayout` latch could go whole and the draft be written "with
  `setEditText` when the editor opens" — but the half of that latch that
  waited for the editor to exist before calling was still load-bearing,
  because until this release the call one frame earlier reached nothing.
  An app that deleted the latch on that advice lost its reset. The advice
  is true now, and the release it was written for is the one it was not.

  **What you can delete:** the frame of waiting — the `onLayout` (or
  first-`changed`, or "is it there yet" re-render) an app kept only so its
  `setEditText` would land after the declare. Set the text in the `update`
  that opens the editor; the frame that draws it takes the text with it.

- **The stock button takes the access rows.** `<button>`, `button { }`
  and `kui_button` were closed composites: three fields on the wire — text,
  key, click — and no prop list at all, so `<button description="…">` was
  dropped without a warning, `tsc` rejected it first, and a Lua or C button
  could not be disabled. That was an odd place for the seam. Every other
  element an app builds content from takes the prop list; the only closed
  ones besides the button were chrome and diagnostics (`titlebar`,
  `windowButtons`, the latency HUD), where nothing needs a description. And
  the alpha.8 `description` entry's own example was a hand-rolled
  `<box role="button">` because the stock one could not say the sentence
  — the button being the element a reader most needs it on ("what a button
  will do"). A field report from `kui-node-template` made it concrete: a
  `reset` button whose action wanted a sentence, and with its context menu
  open two buttons named `reset` in the tree, and the scaffold left the
  sentence out rather than give up the composite for it.

  The button now admits exactly the rows a reader hears and nothing that
  changes its look: `label` (the name, when the text is not it),
  `description`, `tooltip` (the description plus the float while hovered,
  as on a box) and `disabled` (inert, and dimmed to half — the core drops
  the hover and pressed backgrounds of a disabled node, and nothing else
  would show a sighted user the state a reader is told). The list is one
  place, `schema::BUTTON_ROWS_JSX` / `_LUA`, and every binding's button is
  one lowering, `widgets::button_with(ui, key, text, spec, hint)`, which
  `widgets::button` itself now calls. In JSX the four are props on
  `<button>` and `ButtonProps` types them; in Lua `label` stays the name
  *and* the text, and a new `text` key takes the text when the two differ
  (`button { label = "Stop the run", text = "stop" }`); in C
  `kui_button_with(ctx, text, spec, payload)` reads the four fields off a
  `KuiSpec` and ignores the rest, since a zeroed `KuiSpec` is the schema
  default and not "unset" — a new function, no `KUI_ABI_VERSION` bump.

  Why not the whole prop list: the button's look *is* its spec —
  `widgets::button_spec`'s padding, colours and radius — and a general
  props pass over it has a real hazard, `dir` alone rebuilds the spec from
  nothing (the decoder's `P_DIR` is `NodeSpec::row()`, not a flag on the
  spec it has). So `ElementDef` gained `jsx_rows` / `lua_rows`: an element
  that reads only some rows names them, `known_prop` admits those and the
  element's own, the JS encoder writes only those over the wire, and a row
  outside the list — `<button radius={4}>` — is an `unknown-prop` warning
  whose message says which rows the element does read, rather than hunting
  for a nearer spelling of a name that is spelled right. That closes the
  other half of the report: the encoder's allow-list was one flat set of
  every schema row across every element, so a real row on the wrong
  element passed the check and vanished. It now cannot, on any element
  that names its rows. The corpus `controls` scene builds both stock
  buttons through each binding's own button — the clicked one with a
  `description`, a second one disabled, named past its text, with a
  `tooltip` whose string reaches the access row while its float never
  draws — so the four parsers are pinned to one access dump.

  **What you can delete:** the hand-rolled button. A view that wrote
  `<box role="button" bg hoverBg pressedBg radius pad center onClick>`
  with a white 15px `<text>` inside it, to get a description, a distinct
  name or a disabled state onto a button that looked like the stock one,
  is `<button description="…">` again — seven props and a text node per
  button. And `AccessNode.description`'s doc, which still said "what the
  `tooltip` prop sets" a release after `description` began writing that
  slot too.

- **Retained state has a ceiling: the editors and scroll offsets nobody
  declares.** `EditStore::states` and `ScrollStore::entries` only ever
  grew. `declare` is `entry(key).or_insert_with` and no path in the
  runtime removed an entry, so the idiom the retained-by-key model
  invites — a fresh key per opening (`edit-${id}-${session}`, the way
  React remounts on a changed key, which is also the shortest answer to
  "reopen this editor clean") — leaked one shaped `cosmic_text::Buffer`
  per opening, and nothing outside the core could drop it:
  `setEditText(key, "")` shrinks the buffer and keeps the entry. What
  one costs, measured with a counting allocator around `Core`: 3.4 KB
  for a fresh empty editor, 4.9 KB holding `"hello"`, 22 KB holding a
  39-character line, the shaped glyphs being most of it. No shipped app
  has felt it, because today's cost is bounded by the number of distinct
  keys an app ever declares — but that bound is the app's discipline,
  not the library's.

  Pruning on undeclare was not available: "a key declared again keeps
  the draft the user typed" is what the `<edit>` row promises, a tabbed
  form that undeclares a field and returns to it relies on it, and a
  test pins it. So the bound is the exit store's shape instead
  ([ADR 0012](docs/adr/0012-the-exit-budget.md)): a budget on the
  *undeclared* states, the longest-undeclared evicted first, and a
  declared one never evicted however many there are. **256 editors**
  (~1.2 MB of short fields, ~5.6 MB of the 39-character one) and **1024
  scroll entries** (an entry is 56 bytes, 64 with its key, so ~64 KB) —
  sized so no ordinary view meets them: a view with 256 editors it no
  longer declares is already generating keys, which is the idiom this
  makes safe. Declared means declared by the frame that just ended, so a
  key declared every frame never ages, and a scroll offset written
  between two frames (`setScroll` at a key the next frame will build)
  counts as a declaration too. A store inside its budget pays one length
  comparison per frame and never walks itself, which is why this runs
  every frame rather than every 240th like the anim store's cutoff
  sweep. Both counts are readable from Rust (`core.edit.len()`,
  `core.scroll.len()`) and the constants are public
  (`MAX_UNDECLARED_EDITS`, `MAX_UNDECLARED_SCROLLS`); nothing was added
  to `FrameStats`, which is the driver's timing ring and has no way to
  see either store. Backlog F26.

  **What you can delete:** the discipline an app kept in order not to
  generate editor keys — a pool of reused field keys, a "session" suffix
  dropped again because it grew without limit, a comment saying the key
  must be stable *for memory reasons* (it is still what keeps the draft).
  Keying an editor per opening is now bounded by the library.

### Fixed

- **The first click sound stalled the frame for the audio device to
  open.** Opening the output device takes about 90 ms on macOS — six
  frames — and the runner did it on the loop's thread the first time a
  sound played, so the counter's first click (its buttons carry a click
  sound) lagged and every later one flew. The open now runs on its own
  thread, started the frame a session first holds a sound rather than
  the frame one plays, so a click that comes after the first frame finds
  the device open; a command that arrives while it is still opening waits
  in order and starts the moment it is, a few ms late rather than the
  frame being. The driver polls while anything waits, as it does while
  anything plays. A machine with no device degrades as before: one
  warning, commands dropped.

- **A departing subtree drew past its own clippers.** A ghost draws
  outside every clip its ancestors held, since they may be gone — but it
  also drew outside the clips *inside* the picture, so a `virtual_column`
  with `exit` on the container showed the two overscan rows it had built
  past its bottom edge for the frames it slid out (the `bulk_exit`
  example's *clear list*). A ghost now inherits clips the way a live
  subtree does, from its own root down: a scroll box or `clip` node in
  the picture bounds what it held, moved with the picture and rounded by
  its radius; a float inside it escapes them as a live float would; a
  node the clip leaves nothing of is culled. The root itself stays
  unclipped, and `tests/exit.rs` pins both halves. It costs a ghost what a
  clip costs a live node: `replay_a_full_depart_store` reads 15.8 µs
  against alpha.8's 13.3 — about 5 ns a ghost node.

- **Windows: an animation should keep moving while the title bar is
  held** (backlog W3, unverified). Reported as a regression: a transition
  mid-flight froze for as long as the window was grabbed and resumed on
  release. Windows moves and resizes a window inside its own modal loop,
  where the runner's `about_to_wait` — which asks an animating pane for
  its next frame — does not run; winit's Windows backend only marks the
  loop (`WM_ENTERSIZEMOVE` sets a flag) and arms no timer. The modal loop
  does dispatch `WM_PAINT`, so on Windows an animating pane now asks for
  its next frame from `RedrawRequested` as well, which keeps the chain
  alive through the hold; vsync paces it as before, and nothing changes
  on the other platforms. Built against the report and `cargo check`ed
  for the target, not run: no Windows machine here, and the smoke job
  that would (P8) is still waiting for a runner, so it stays on the
  by-hand round — `toasts`, grab the title bar mid-spring.

- **`npm run gen` describes the addon it just built, not the one before.**
  The generator loaded the addon first — for the prop, element, event and
  warning tables `docs/props.md` and the TS unions are written from — and
  built it afterwards, for the `#[napi]` type defs `index.d.ts`'s addon
  half comes from. So a schema row added in Rust reached `props.md` one
  `gen` late, while the method that landed beside it was current, and CI's
  `git diff --exit-code` on the generated files was checking the previous
  build's tables. The build step now runs before the load. Found while
  building C17, whose two event rows did not appear the first time.

- **Lua's `text(s, opts)` copies its options.** It wrote `type` and
  `value` into the table it was handed and returned that table, so a
  script that hoisted a style — `local mono = { size = 14 }` — and passed
  it to three texts built one table three times, and every text showed
  the last string. Found by C18's Lua test, whose three runs share one
  style table and came out as `" = 1;"` three times; the prelude copies
  now, and `a_style_table_shared_by_three_texts_is_three_texts` pins it.
  **What you can delete:** the per-call `{ size = 14 }` literal a script
  spelled out because a shared one misbehaved.
- **`examples/lua/panel.lua` drew nothing when the C plugin was not built.**
  Its fallback branch passed `wrap = true`, and `wrap` is the text one
  (`"word"` / `"glyph"` / `"none"`) — the boolean is `wrap_children`, on a
  row. Lua reported it the way it should, as a runtime error from the
  extension's `view`, but an error there takes the whole panel with it:
  `extension-view-error`, then `unbalanced-extension` for the two nodes the
  failed call left open. Nothing caught it because the branch only runs
  before `examples/c/build.sh` has been run, and because a warning is not a
  non-zero exit — `KUI_SMOKE_FRAMES` judges that drawing did not fail, not
  what was said while it drew. The pre-tag round reads the warnings now, and
  every example is clean of them: three more that were not (`gallery`'s three
  unlabelled images, `editor`'s unnamed document field, and this panel's
  filter box) have the `label` the library was asking for.

### What you can delete

- The `configure_root(NodeSpec::row())` a host set only so that an
  extension appended after its view would land beside it, and the
  comment saying why: declare the slot where the panel goes.
- A `key` on an extension's root node added to keep its editors and
  tweens from resetting when the host's root changed.
- The stash a plugin kept so the host could learn what it chose: reply.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag on
2026-09-08 on the branch that carried the editor-and-mux work. What
follows is what executed on what.

**macOS 26.6.2 (arm64), rustc 1.98.0.** `cargo test --workspace` passes:
**710 tests over 69 suites, 0 failed, 0 ignored**, with no display, no
installed fonts and no GPU. `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets -- -D warnings` are clean. The scene corpus runs
in all four adapters against one reference report: **22 scenes** — `cells`
and `ime` new, the second driving a composition into an app-owned editor
and a stock one in turn — Rust and Lua through `cargo test`, C through
`examples/c/counter --conformance` (the header at 251 fields and 82 enum
members, matched against the Rust side by the parity assert), Node
through `npm test` with `KUI_CONFORMANCE_REQUIRED=1` (**93 Node tests**,
0 failed). The C plugin dlopens into a Rust host, routes clicks both ways,
fills the slot it is given and delivers its reply; the same plugin with
`kui_ext_abi` deleted is refused against ABI 8. `npm run gen` leaves the
three generated files unchanged, `examples/node` installs, typechecks,
builds and runs headless, and `scripts/check-version.sh 0.1.0-alpha.9`
passes.

`scripts/ax-audit.swift` against `examples/rust/accessibility.rs` passes
**106/106 checks**, the same 106 alpha.8 had. The one new role,
`Terminal`, is derived from a `cells` node, which that example does not
draw; the corpus `cells` scene is what reads it back.

Every host opened a window and drew, each captured with `screencapture -l`
and looked at: the **thirteen** Rust examples (`accessibility`,
`bulk_exit`, `connectors`, `counter`, `editor`, `gallery`, `modal_editor`,
`popup`, `rich_text`, `splitmux`, `syntax_view`, `toasts`, and `waker`,
which is new and reports its 42 lines drawn over 43 frames with no input),
`examples/c/counter`, `c_panel`, `lua_panel`, and `counter-window.mjs` on
Node — seventeen windows, eighteen with the mind map below.

**The two drag checks on the by-hand list.** `npm run mindmap`, then a
synthetic drag posted through the HID tap with `screencapture` run *while
the button was down*: at "move pan −45,−27" the four cards and their
connectors had moved together to where the cursor was, and at "end pan
−90,−55" they sat at the full offset — F15's symptom, still absent. ADR
0009's gesture: press on the `popup` example's field, four legs down into
the menu captured as a screen region so the second window is in the shot
— the menu open throughout and *Beryllium* highlighted under the cursor
on the third leg — and a release 76 pt below on the third item, which set
the field to **Cadmium** without a second click. Windows' mixed-DPI case
and W3 (an animation under a held title bar) stay unrun: nothing here has
a Windows machine, which is why W3 ships marked blind.

**Two of this release's fixes were watched where they show.** The ghost
clip: `bulk_exit`'s *clear list* pressed through the AX API and captured
straight after, the list's picture sliding out with rows 0–8 inside its
box and nothing below it, where the report's capture had rows 9–11
hanging under the panel. The counter's first click: the audio device
takes **92 ms** to open on this machine, measured, and the device test
now asserts that the first play returns in under 20 ms — the open is on
its own thread — and that the queued blip still plays and reports ended.

**The performance guard passed; the table was not refreshed from it.**
`scripts/bench-check.sh` benches HEAD against `v0.1.0-alpha.8` back to
back on one machine: **none of the four guarded rows is slower** —
`deep_nesting_64_levels` +3.4%, `frame_10k_rects` −1.9%,
`frame_10k_rects_with_text_and_hits` −0.3%, `frame_1k_typical` +0.3% —
with a worst run-to-run spread of 2.5% on a guarded row. The run
overlapped the window round (the captures and the AX audit ran beside
it), and the noisy rows show it: `frame_10k_rects_all_transitioning`
read 2.34 ms for alpha.8's own commit where the README records 1.93 for
it, its two runs ±12% apart. So this is a run that says "no regression"
and not one to take absolute numbers from — alpha.7's rule — and the
README's table keeps alpha.8's medians for the rows both releases have.
Two unguarded rows moved: `replay_a_full_depart_store` +19.9% (13.2 →
15.8 µs) is the ghost clip, real and explained under `### Fixed`;
`list_10k_rows_naive` +14.1% at ±9.5% spread is unreadable at that size.
The rows this release added — the `stream`, `long_line` and `cells`
benches — carry the numbers their entries measured when they landed.

## 0.1.0-alpha.8 (2026-09-07)

**What breaks.** **A press that dismisses a popup no longer reaches the
window it landed in.** It reports `{kind:"dismiss", reason:"outside"}` as
before and then stops there, and its release stops with it — which is what
every native menu does, and what an in-window modal already did by accident
(everything outside a modal emits no hit region, so the outside press lands
on nothing). A popup's owner is live, so there the press used to dismiss
*and* act. Only non-activating popups are affected: an `activates: true`
panel keeps the pass-through, since someone working in a panel beside the
app expects a click in the app to act. The `### Added` entry below is why
it had to change — with the press passed through, a field that opens on
mouse-down would dismiss and reopen its own menu in one gesture.

**A single-line `<edit>` seeded with `initial` opens with the caret after
its text.** It opened at offset 0, so typing into a value meant to be
extended prepended to it; see `### Changed`. A multiline editor still opens
at its top.

**`ctx.setTime` throws under `createApp`.** It was overwritten by the loop's
own stamp before every frame since alpha.7, so it already did nothing; now
it says so. See `### Changed` — the two alpha.7 field reports are why.

**A frame that removes more than 512 nodes declaring `exit` animates none
of them.** It used to animate the first 512 and blink the rest — a 1000-row
list dropped with rows 0–511 sliding out and rows 512–999 vanishing, the
split set by how many nodes a row's markup happened to contain. Now the
removal is judged whole: past the budget every departing node vanishes at
once, as a node with no `exit` does, and the `exit-budget` warning names
the frame's count. A view that was getting the half-animation loses it;
that is the intended trade, and the `### Changed` entry says why
(`docs/adr/0012-the-exit-budget.md`).

### Added

- **`minWidth: "fit"` / `minHeight: "fit"`: a node's own fit size as its
  floor.** The first view to ask was an i3-style tab bar — every tab
  `grow`, so the tabs split the bar evenly while they fit, and once they
  do not, each at its label's width with the bar scrolling — and the
  numeric half of that already worked: `grow` + `minWidth={80}` +
  `scrollX` gives 200/200/200 for three tabs in a 600 bar and 80 × 10
  with 200 to scroll for ten, because the grow pass clamps each share to
  its min and a scroll axis skips the shrink pass. What could not be
  said was the floor *as the content*: `grow` contributes nothing to fit
  and had no floor of its own, so ten grow tabs each around an 80 px
  label got 60 px apiece, the label cut, nothing to scroll. That is
  CSS's `flex: 1 0 auto`, and `Sizing` had no word for it. Now `minWidth`
  and `minHeight` take a number or `"fit"`: the fit pass of that axis
  measures the node's content once, writes the number into the spec, and
  every later clamp reads it — so the grow pass, the wrap breaker and the
  shrink pass need no second form. Both axes, under any sizing (a 50%
  height floored at its child's is in the corpus scene), one measurement
  shared with a `fit` sizing when both apply, and no cost on a node that
  declares neither. In Lua it is `min_width = "fit"`; in C, `.min_w =
  KUI_MIN_FIT` — a negative in a slot that was already clamped to zero,
  so `KuiSpec` did not move and neither did the ABI; Rust's
  `.min_width(Min::Fit)` beside the `f32` it always took. The new `tabs`
  scene runs the bar both ways in one frame, in four bindings.
  **Not the default, on purpose.** CSS floors every flex item at its
  min-content and every CSS author has typed `min-width: 0` to undo it;
  kui's fit width is the *unwrapped* one — a text node's intrinsic line —
  so a default floor would stop every paragraph inside a `grow` column
  from wrapping. A tab bar's labels do not wrap, which is exactly why it
  is the view that wants the floor and can say so.
  **What you can delete:** the `measureText` in `view` that sized a
  `minWidth` to a label plus padding, and the cache in front of it.

- **Effects as data: `withEffects(model, ...effects)`** (backlog F23, from
  the pomodoro's alpha.7 report; `docs/adr/0013-effects-as-data.md`). The
  report's chime "costs a model field plus a module global purely so the
  driver can notice a counter move and call `win.play`", and asked for an
  Elm-style `[model, effects]` return. Half of that premise was wrong and
  the entry says so: `update` already has the surface as its fourth
  argument, `<audio key src>` is a declarative one-shot, and
  `audioCommands()` is the headless drain the report said was missing —
  the chime is one `<audio key={\`chime-${m.alarmCount}\`}/>` in the view
  and no API. What survives is every effect kui does *not* own — a file
  write, a request, the clipboard — which had three homes and each was
  wrong in a way a test can feel: done in `update` (impure, and nothing
  records it), recorded in the model for a driver to diff (a field that is
  not state, and a shadow of it), or keyed off a message in the driver
  (duplicated or absent headless). Now `update` — and a function `init` —
  may return `withEffects(model, ...effects)`: a branded value, because
  the model is untyped by the loop and an array model *is* a tuple, so a
  tuple could not be told from one. An effect is the app's own value (`E`
  on `createApp` / `runWindowed`, default `never`); kui defines no
  vocabulary for it, and kui's own effects stay where they are. The loop
  sets the model, queues the effects, and after the next frame hands each
  to the `effects(effect, dispatch, surface)` handler the app was created
  with — after the frame, so an effect that dispatches its result lands in
  the next turn and one that reads the surface sees the frame its cause
  produced. Headless, `app.effects()` drains them for a test whether or
  not a handler ran, the way `audioCommands()` answers without a device;
  a window drains them every pump. Node only; nothing in the core, the IR
  or any binding moves.
  **What you can delete:** the counter-and-shadow. A model field that
  exists only so the driver can notice it move, the module-level `let`
  that remembers the last value it acted on, and the `update` wrapper in
  the windowed entry point that performed the effect on the way past —
  for an effect kui does not own, the effect is the return value now, and
  the test reads it from `app.effects()`. (For the chime specifically:
  the `<audio>` line, and none of this.)

- **Press the field, drag into the menu, release on an item** — the native
  select gesture, in one gesture with no second click
  (`docs/adr/0009-press-drag-release-into-a-popup.md`, backlog W2). It could
  not be built by an app, and the reason is measurable: the OS gives a
  captured drag to the window that received the mouse-down, so a press on
  the field and a release over the popup deliver **nothing** to the popup
  and everything to the owner, in coordinates that mean nothing to it —
  `93,-1`, `95,-5`, `97,-10` walking off the top edge. That is what
  `NSMenu`'s tracking loop, Win32's menu message loop and a GTK pointer grab
  exist to do, and none of them is reachable through winit. The runner does
  it instead: a non-activating popup that opens while the primary button is
  down **joins that press**, the owner's moves are translated through the
  screen into the popup's own coordinates and fed to its core as ordinary
  `CursorMoved`s, and the release is classified by where it lands — over the
  popup, the press-and-release it never saw are synthesised into it; on the
  anchor, nothing, so the two-click interaction stays whole; anywhere else,
  `dismiss`. The mapping is in physical pixels, so it holds on a mixed-DPI
  desktop where two windows do not share a logical origin.
  **Nothing new to declare.** A popup's core is never told it was
  retargeted, there is no `onPress` row, and an app opens on `onDrag`'s
  `start` phase — a press by another name, with a capture and an `end` a
  bare press event would not have. The protocol is *everything opens, only a
  dismissal or a choice closes*: `start` sets `open`, `click` sets it too
  (that is the keyboard, assistive technology, and a stationary release),
  `dismiss` and choosing clear it. Nothing toggles, so nothing has to know
  which press it is answering — which is what the consumed press above buys.
  `examples/rust/popup.rs` is the whole of it: `on_drag`, `on_click` and
  `cursor: "pointer"` over the `grab` a draggable node would derive.
  **What you can delete:** the guard against the click you were dismissed
  by. A view that answered `{kind:"dismiss"}` by clearing a flag and then
  had to survive the press arriving in the owner a moment later — the
  "was this the press that closed me?" timestamp, the one-frame latch, the
  re-entrancy check around a toggle — can drop it: that press does not
  arrive any more. And the toggle itself, if the flag was one: assignment
  is enough now.

- **`app.runOut()`: the settled frame, by name** (backlog F22, from both
  alpha.7 field reports). Since the loop owns the clock, the frame that
  applies a change is frame 0 of its transitions, and a capture taken
  straight after a dispatch shows exactly that — the mind map's preview
  rendered every overlay panel fully transparent with its labels
  mid-slide, and stayed green, because the access tree was complete and
  correctly named while only the paint was gone. Both apps then wrote the
  same loop, `for (let i = 0; i < 40 && app.ctx.animating(); i += 1)
  app.advance(16)`. `runOut(maxMs?, stepMs?)` is that loop: it draws once
  so a pending change is frame 0 rather than a stale `animating()`, then
  advances in frame steps until nothing moves, returns the milliseconds it
  took, and stops at the cap with `animating()` still true if something
  never settles — a looping keyframe. Headless only, like `advance`.
  **What you can delete:** the loop.

- **`win.quads()`: the display list a real window drew** (backlog F19,
  from the pomodoro's report). Headless coverage was the whole of the
  app's coverage — "the window path has none" — and the ask was small:
  `quads()` on the window, so the smoke test can read the frame the
  shipping driver painted. It was on `Ctx` alone for no reason the code
  could give: `accessTree()`, `stats()` and `animating()` already answer
  for a window through the same core. It is in the shared surface now,
  reads what the last pump drew, and `decodeQuads` takes the buffer as
  before. Pixels — a `capture()` — are a wgpu readback and wait for a smoke
  test that needs them rather than the display list.

- **`description`: the sentence a reader says after the name** (backlog P1,
  open since the 2026-09-03 architecture review). The accessible
  description had one spelling — `tooltip` — which also hover-tracks the
  node and floats the string under it, so a hint that should be *spoken and
  never drawn* could not be said at all, and in C it could not be said even
  with a tooltip until the shorthand shipped. `description` is now a row in
  the shared prop schema like any other: `<box role="button" label="Save"
  description="Nothing to save yet">`, `description` in a Lua table,
  `KuiSpec.description` in C, `.description()` in Rust. `tooltip` is
  unchanged and is still the shorthand for all three effects at once.
  Being a schema row rather than a tenth hand-written composite is the
  point of it: Lua looks it up by name, the JS encoder writes it by kind,
  `npm run gen` put it in `jsx-runtime.d.ts` and `docs/props.md`, and the
  parity test forced the `KuiSpec` field, its `include/kui.h` mirror and
  its application in `spec_of` rather than a reviewer having to notice
  they were missing. `KuiSpec` is host-allocated and read by the library,
  so appending the field costs no ABI bump: `KUI_ABI_VERSION` stays at 7.
  Two things worth knowing. `description` and `tooltip` write one slot —
  in C the explicit field is applied second and wins; in the parsers the
  later declaration wins, as with `center` and `mainAlign`. And a
  description only reads on a node that reaches the access tree: an
  unlabelled plain box is elided and takes its description with it, so put
  it where the `role` or the `label` is. The corpus `tooltip` scene now
  builds both nodes side by side in all four bindings.
  **What you can delete:** the tooltip you did not want. A view that wanted
  a reader to explain a disabled control, a format, or what a button is
  about to do, and had to accept a floating hint over the pointer to get
  it — or gave up and stuffed the sentence into `label`, which renames the
  control instead of describing it.

### Changed

- **A frame's removal animates whole or not at all, and a new removal
  outranks the ghosts already in flight** (ADR 0012, decisions 2, 3 and 6;
  decision 5 is the entry below). The exit store's budget is 512 nodes and
  it used to be applied one departing subtree at a time — admit, admit,
  admit, refuse — so a mass removal over the budget got a third behaviour
  neither policy produces: the top of the list sliding out and the bottom
  blinking, with the boundary moving whenever an author added a label to a
  row. And the store protected its oldest ghosts, so an unrelated
  dismissal 100 ms behind a big one was truncated to whatever was left.
  Now `collect_departures` counts the frame's departing roots before it
  copies any of them and asks the store once (`DepartStore::admit`): a
  removal that fits an empty store is admitted whole, taking the room it
  needs from the **oldest** ghosts first — they are furthest through their
  own fade, and the removal the user just caused is the one they are
  looking at — and a removal larger than the budget on its own is refused
  whole, with one `exit-budget` warning for the frame that says how many
  nodes it removed. A view under the budget cannot tell any of this
  happened: six toasts and a "clear" are what they were.
  The corpus `exit` scene pins it in all four bindings: `bulk` (one
  subtree, a node past the budget) now leaves in a frame of its own, and
  600 one-node rows leave in the frame after — refused whole, where
  per-subtree admission kept 512 of them and the solid count would read
  522. Building the scene found the one thing the ADR's consequences had
  wrong: `bulk` used to leave with `fade`, and under the new rule that
  frame of 517 nodes would have refused `fade` too. The departing frame is
  cheaper as well as different, since a refused removal copies nothing:
  `drop_1k_rows_declaring_exit` 217 → 157 µs, and its meaning changed with
  it (it is the refused frame now), so `drop_500_rows_declaring_exit`
  measures the admitted one at 139 µs. `examples/rust/bulk_exit.rs` is the
  boundary with buttons on it. Nothing in the IR, the ABI or any binding
  moves; `props.md`, `index.d.ts`, `kui.h` and the README carry the
  warning's new sentence.
  **What you can delete:** the model-side stagger. A view that dropped a
  big list in batches across several frames to keep each batch under the
  budget — or capped how many rows could leave with `exit` at once — can
  drop the whole list in one frame and let the store decide; and a view
  that wrapped a long list's departure in its own count to avoid the
  half-animation can stop counting. A long list still wants its `exit` on
  the list rather than on every row, and a *virtualised* list
  (`widgets::virtual_column`) is what makes that picture small enough to
  fit: the ghost is the built slice, not the thousand rows.

- **A seeded single-line editor opens with the caret after its text**
  (backlog F20, from the mind map's report). A rename field seeded with
  the current name and `autofocus` opened with the caret at 0, "so typing
  into a name you meant to extend prepends to it", and nothing declared
  otherwise: `caret` is a row for an editor the app draws itself, and
  `key('end')` is headless-only by design (F6), so a fix built on it
  passed every headless check and did nothing in the window. The app's
  way through was `setEditText(key, text)` with the unchanged text from an
  `onLayout` latch, because that call happens to leave the caret at the
  end. Now a single-line field seeds with the caret after its text — what
  a native field does with a prefilled value — and a multiline editor,
  being a document, still opens at its top as a native text view does.
  Placed rather than moved, so nothing scrolls to reveal it before the
  user has touched it. The `<edit>` row also says the two things the
  report had to find out: `initial` seeds a new editor only — a key
  declared again keeps the draft the user typed — and `setEditText` is
  what resets one. The corpus `controls` scene's editor pins the new
  caret in all four bindings.
  **What you can delete:** the `onLayout` latch that wrote the editor's
  own text back to move its caret, and the `initial`-does-not-reset
  surprise on reopen — write the model's draft with `setEditText` when
  the editor opens, which places the caret in the same move.

- **The `line` row says what a curve costs** (backlog F21, from the mind
  map's report): one quad per piece, a curve flattened at one piece per
  6 logical px of chord and at most 32 per span, fixed rather than
  tolerance-driven so all four bindings get the same pieces and the corpus
  can pin them. The report's nine-point curve came back as 63 segments and
  took its frame from 68 quads to 477 — exactly what the constant
  predicts, and nothing visible slowed, but a `quadCount` budget will
  notice and now the row says so.

- **The npm package ships this file and the ADRs its types cite** (backlog
  F18, from the pomodoro's report). The tarball was the runtime, the types
  and `props.md`; learning what alpha.7 changed "meant downloading both
  tarballs and diffing three files", and the doc comments in `index.d.ts`
  and `jsx-runtime.d.ts` point at `docs/adr/0003-modal-surfaces.md`,
  `0004`, `0008` and `0010` eleven times — paths relative to a repository a
  `node_modules` reader does not have. `prepack` now copies `CHANGELOG.md`
  and `docs/adr/` into the package under the same paths, so every citation
  resolves where it points, and the package README says so. Nothing in the
  comments moves.

- **A frame clock set by hand under a loop is refused rather than ignored**
  (backlog F16, from both alpha.7 field reports). alpha.7 gave the loop
  the clock: `createApp` stamps `setTime` before every frame it draws, and
  `advance(ms)` is what moves the hands. What that left behind is the
  alpha.6 idiom — `ctx.setTime(t)` around each `render()` — which still
  compiled, still ran, and did something worse than snap: the stamp that
  overwrote it reads a clock only `advance` moves, so a tween driven that
  way never left its baseline and `animating()` never fell. One report's
  test "stopped observing motion"; the other's hung on `while
  (ctx.animating())`, "not at t=0.3s, not at t=1000s". The changelog line
  under alpha.7's "what you can delete" was the only signal. Now the loop
  keeps the surface's native `setTime` for its own stamping and replaces
  the method on the instance with one that throws and names `advance` —
  the mirror of `advance()` refusing a wall clock, and the same shape as
  the surface saying it has no `click()` to drive it with. A bare `Ctx`
  keeps the method: that is still the one place to set the clock by hand,
  and `setTime`'s doc says so in both bindings' words.
  **What you can delete:** nothing new — the alpha.7 entry already listed
  those calls. What changes is that a test that kept one fails at the call
  with the fix in the message, instead of at a timeout.

- **A plain frame is ~11% cheaper, and C15 is closed** (backlog C15, the
  half left open after alpha.6's boxing fix). The second profile the entry
  asked for was taken, and it says where the remaining ~1.5× against the
  2026-08-31 table went: about a third is still the struct's size, now
  paid in the app's own builder chain and the moves down the open chain,
  and the rest was per-node reads that features added to passes a plain
  frame runs anyway. Those are gone. `Tree` carries `any_float`, `any_wrap`
  and `any_text`, set as nodes are pushed, and the layout passes skip the
  float, wrap and text-clamp walks wholesale behind them — the same shape as
  `any_exit` skipping the depart diff; emission's float check sits behind
  the flag it already had; the hover-style and transition early-outs are
  branches at the call site rather than calls that return; `hover_tracked`
  tests each boxed group once; and `Ui::open` → `Core::open` →
  `open_with_key` → `Tree::push` are inlined so a spec is copied once, into
  the tree. Against alpha.7, interleaved on one machine: `frame_10k_rects`
  816 → 725 µs (−11%), `frame_1k_typical` 117 → 110 µs, text-and-hits
  1.22 → 1.14 ms, the chip grids −12 to −14%, the clip and one-exit grids
  −12%, with `deep_nesting_64_levels` unchanged (+1%, inside its noise).
  Nothing in the IR, the ABI or any binding moves. The entry also measures
  what it declines: `Quad` at 124 bytes is an ABI field and costs ~5% on
  the 20k-quad bench, and the one pass left that a flag could skip — the
  drop walk over 10k specs in `begin_frame`, ~3% — needs kui-core's first
  `unsafe` block, which is a decision and not an optimisation.

- **A mass removal's departing frame stops being quadratic** (ADR 0012,
  decision 5). `DepartStore::depart` retires any ghost already holding the
  departing key — two pictures of one node are never right — and that
  retire is a pass over the whole store, taken unconditionally, so a frame
  that dropped N subtrees with an `exit` paid N walks. It now keeps an
  exact set of the keys it holds and takes the walk only when the key is
  actually in it. Dropping a thousand rows that declare an `exit` costs
  ~217 µs where it cost ~256 µs (four interleaved rounds, no overlap), and
  the shape matters more than the frame it saves today: past the 512-node
  budget the old path ran away — 10 000 subtrees cost 32 ms against 1.05 ms
  — so the budget could not have been raised without this regardless of
  what it was raised to. No behaviour changes, and nothing in the IR, the
  ABI or any binding moves.

  Worth recording because the obvious fix does not work: a 64-bit
  membership mask over the key, which is what the store already uses for
  the neighbouring `before` keys, saves **nothing at any size**. A mass
  removal is hundreds of distinct keys, 64 bits saturate after about
  sixty-four of them, and the mask then answers "maybe" for every row. A
  membership mask wants a field that is sparse, and a departing key never
  is.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag on
2026-09-07. What follows is what executed on what.

The round was run twice, because the `"fit"` minimums were folded into this
release after the first one. Everything headless was re-run against the
folded tree and the numbers below are that second run: the workspace tests,
fmt and clippy, all four corpus adapters, both C extension loads, the Node
addon tests, `npm run gen`, the `examples/node` leg, `check-version.sh` and
`bench-check.sh`. What was **not** re-run is everything needing a logged-in
GUI session and the Accessibility grant — `ax-audit.swift`, the per-host
window captures, the ADR 0012 boundary watch and the two drag checks. Those
results stand from the first run, one commit earlier, and the reason they
are allowed to is that a `"fit"` minimum is a layout number: it moves no
role, no name, no announcement and no window, and none of the examples
those checks drive declares one.

**macOS 26.6.2 (arm64), rustc 1.98.0.** `cargo test --workspace` passes:
**638 tests over 60 suites, 0 failed, 0 ignored**, with no display, no
installed fonts and no GPU. `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets -- -D warnings` are clean. The scene corpus runs
in all four adapters against one reference report: **20 scenes** — the
`exit` scene two phases longer than alpha.7's, and `tabs` new for the
`"fit"` minimums — Rust and Lua through `cargo test`, C through
`examples/c/counter --conformance` (which also reports the header at 235
fields and 81 enum members), Node through `npm test` with
`KUI_CONFORMANCE_REQUIRED=1` (**82 Node tests**, 0 failed; three are
ADR 0013's). The C header still compiles against the generated ABI asserts,
the C plugin dlopens into a Rust host and routes clicks both ways, and the
same plugin with `kui_ext_abi` deleted is refused against ABI 7. `npm run
gen` leaves the three generated files unchanged, `examples/node` builds,
typechecks against the new loop types and runs headless, and
`scripts/check-version.sh 0.1.0-alpha.8` passes.

`scripts/ax-audit.swift` against `examples/rust/accessibility.rs` passes
**106/106 checks**, the same 106 alpha.7 had: nothing in this release
touched a role, a name or an announcement.

Every host opened a window and drew, each captured with `screencapture -l`
and looked at: the **twelve** Rust examples (`accessibility`, `connectors`,
`counter`, `editor`, `gallery`, `modal_editor`, `popup`, `rich_text`,
`splitmux`, `syntax_view`, `toasts`, and `bulk_exit`, which is new),
`examples/c/counter`, `c_panel`, `lua_panel`, and `counter-window.mjs` on
Node.

**ADR 0012 was watched at its boundary, in the frames no headless test
reads.** `bulk_exit` driven through the macOS AX API with a compiled press
driver, captured straight after each press: *clear both* removed 600 nodes
in one frame and **nothing animated** — both grids gone in the first
capture, the `exit-budget` warning on stderr naming the frame's 600 nodes
against the store's 512 — where alpha.7 would have slid 512 out and blinked
88. *clear A* then *clear B* **3 ms apart**: A's first 88 cells gone, its
remaining 212 sliding down together, every one of B's 300 sliding with
them, and nothing on stderr, since eviction is the policy and not a warning.
*clear list* on the thousand-row virtual column with the `exit` on the
container: the built slice — rows 0–11, the overscan included — slid out as
one picture, unclipped, as a ghost is.

**The two drag checks on the by-hand list.** `npm run mindmap`, then a
synthetic drag posted through the HID tap with `screencapture` run *while
the button was down*: at "move pan −45,−27" the four cards and their three
connectors had moved together to where the cursor was, and at "end pan
−90,−55" they sat at the full offset — F15's symptom, still absent. And
ADR 0009's gesture, run for the first time by hand since W2 built it:
press on the `popup` example's field, four drag legs down into the menu
captured as a screen region so the second window is in the shot — the menu
open and tracking throughout — and a release 76 pt below on the third item,
which set the field from Aluminium to **Cadmium** without a second click.
Windows' mixed-DPI case stays unrun; nothing here has a Windows machine.

**The performance guard passed, on a quiet machine this time.**
`scripts/bench-check.sh v0.1.0-alpha.7` benches HEAD against the previous
tag back to back on one machine: **none of the four guarded rows is slower
than alpha.7** — `frame_10k_rects` −9.9%, `frame_10k_rects_with_text_and_hits`
−6.4%, `frame_1k_typical` −3.9%, `deep_nesting_64_levels` −0.3% — with a
worst run-to-run spread of 6.1% on a guarded row and no load warning. This
is the run that covers the `"fit"` minimums as well, which is why it is
this one and not the earlier one in the round: `minWidth: "fit"` adds a
read to the fit pass, and `frame_1k_typical` and `deep_nesting_64_levels`
are the rows that would show it. Neither does, because a node declaring
neither minimum pays nothing.
The README's table is this run's medians throughout, re-measured when the
`"fit"` work was folded in: most rows moved a percent or two, and two
moved more than the guard's tolerance without either being a regression —
`frame_10k_rects_all_declaring_exit` (2.20 → 2.51 ms) and
`list_10k_rows_naive` (3.57 → 3.94 ms), both unguarded rows measured
against alpha.7 at −9.4% and −1.7% in the same run, so the gap is between
two README measurements and not between two releases. The
gains are C15's, landed after alpha.7's tag: the chip, clip and one-exit
grids are −10 to −13%. Three rows are marked *touched* because the bench
file changed, and each says why: `drop_1k_rows_declaring_exit` −41.2%
(278 → 163 µs) is ADR 0012 refusing a 1000-row removal whole and copying
nothing, so the row now measures the refused frame and the new
`drop_500_rows_declaring_exit` (~138 µs) measures the admitted one;
`replay_a_full_depart_store` −4.7% fills the store to exactly the budget
rather than through a refusal; `drop_1k_rows_plain` −8.6% shares the
builder and is C15's. The row alpha.7 found would not hold still held
still this time: `frame_10k_rects_all_declaring_exit` read ±2.7% between
its own two runs rather than ±12.5%, and reports −9.4% — but one quiet run
does not retire a row's history, so the table keeps the stated ±10% on it.

## 0.1.0-alpha.7 (2026-09-06)

**What breaks.** **A drag's `dx`/`dy` are measured from the press point
now, in every phase.** They were the step since the previous event on
`move` and always zero on `end`, and the doc said neither — so a handler
that accumulated on `move` and committed on `end` snapped whatever it moved
back to where it started, and lost up to the 3 px click slop from every sum
besides (backlog F2, from a field report). Now `start` carries zero, a
`move` how far the pointer is from where it pressed, `end` the whole
distance: `value = start + dx` replaces `value += dx`, and `end` is a phase
an app can commit from. A handler that adds deltas will now move things
quadratically fast; the `drag` scene of the corpus pins the new numbers in
all four bindings. The click slop is measured from the press too, so a slow
pointer that never covers 3 px between two events still starts its drag.

**And a key sink hears presses only again** — alpha.6 delivered both halves
of every key to one sink, which made press-only, the shape every keymap has,
the case that needed a guard. A sink that wants releases says `keyUp`. The
`### Changed` entry below has the whole of it, including what an alpha.4 app
bumped past alpha.6 should delete.

### Fixed

- **`Quad` declares what `decodeQuads` has been returning since alpha.7**
  (backlog F17, from the mind map's report). The decoder gained `blur` with
  ADR 0005's shadow quad and `ends` with ADR 0010's segment, and the
  interface in `index.d.ts` — the hand-written half — got neither, with a
  `kind` comment that still stopped at 4. The report's preview script
  filtered the display list to `kind === 0`, which is what the type
  suggested a display list held, and silently dropped every connector; the
  app declared the field itself to get on. `Quad` now carries `blur`,
  `ends` (`null` off a segment), and the seven kinds in `QuadKind`'s
  order, and a test decodes one of each.
  **What you can delete:** the `Quad` augmentation an app wrote to read a
  stroke's endpoints (`src/kui.d.ts` in the mind map).

### Native verification

The same by-hand round alpha.6 introduced (backlog R4), run before this tag.
What follows is what executed on what, and what did not.

**macOS 26.6.2 (arm64), rustc 1.98.0, 2026-09-06.** `cargo test --workspace`
passes: **629 tests over 60 suites, 0 failed, 0 ignored** — still with no
display, no installed fonts and no GPU. `cargo fmt --all --check` and
`cargo clippy --workspace --all-targets -- -D warnings` are clean. The scene
corpus runs in all four adapters against one reference report: **19 scenes**,
Rust and Lua through `cargo test`, C through `examples/c/counter
--conformance`, Node through `npm test` with `KUI_CONFORMANCE_REQUIRED=1`
(75 Node tests, 0 failed). The C header still matches the Rust structs — 234
fields and 81 enum members — the C plugin dlopens into a Rust host and routes
clicks both ways, and the same plugin with `kui_ext_abi` deleted is refused
against ABI 7. `npm run gen` leaves the three generated files unchanged.

`scripts/ax-audit.swift` against `examples/rust/accessibility.rs` passes
**106/106 checks**, up from alpha.6's 88 — the eighteen new ones are ADR
0008's live regions and announcements, driven through the real macOS AX API:
a live region's text is exposed and re-announced when it changes, an
announcement with no node reaches the OS, the same message twice in a row is
said twice (the fresh-node-id rule the ADR records), and the last
announcement stays readable in the tree afterwards.

Every host opened a window and drew, each one captured with `screencapture
-l` and looked at: the **eleven** Rust examples (`accessibility`,
`connectors`, `counter`, `editor`, `gallery`, `modal_editor`, `popup`,
`rich_text`, `splitmux`, `syntax_view`, `toasts` — `popup` is new this
release), `examples/c/counter`, `c_panel`, `lua_panel`, and
`counter-window.mjs` on Node.

**F15 was checked in the one frame no headless test reads** — between a press
and a release. `npm run mindmap`, then a synthetic drag posted through the
HID tap and `screencapture` run *while the button was down*: at "move pan
180,110" the four cards and their three connectors have moved together to
where the cursor is, rather than standing still and jumping into place on
release. That is the whole of the F15 symptom, and it is on the by-hand list
rather than in a test because the model was always right — only the screen
was wrong.

**The performance guard passed; the README's table took a second run to
refresh.** `scripts/bench-check.sh` benches HEAD against the previous tag
back to back on one machine: **none of the four guarded rows is more than 10%
slower than alpha.6** (`frame_10k_rects` +1.4%, `frame_1k_typical` +1.2%,
`frame_10k_rects_with_text_and_hits` +2.6%, `deep_nesting_64_levels` −4.2%),
with a worst run-to-run spread of 4.6% on a guarded row. That verdict is a
*difference* measured under identical conditions and it stands. The absolute
table under "Performance" could not be taken from that run, and the reason is
worth recording: this machine was not idle, and benching the alpha.6 tag
*itself* gave `frame_10k_rects_all_transitioning` at 2.80 ms against the
1.73 ms the README recorded for that same commit, with a ±27% spread between
that row's own two runs. Those medians would have made the table less true,
not fresher.

**The table is now a quiet machine's**, and the re-measure found something
the guard cannot: two rows do not reproduce. `frame_10k_rects_all_transitioning`
spans **1.69–1.94 ms** and `frame_10k_rects_all_declaring_exit` **2.15–2.61 ms**
across four full-suite passes of one binary, while every other row holds to ~3%.
Their table entries are the median of those passes and carry a stated ±10%;
quoting them to three figures was never meaningful. What was ruled out: power
source (the same binary on mains and on battery agrees to within 1%) and a
regression (alpha.6 benched beside it moves the same way). What is left is
unexplained, so nothing here names a cause. Protocol matters too — a row
measured alone after an idle reads below the same row inside the whole suite
(`frame_10k_rects` ~765 µs against ~807 µs) — so the table quotes what
`cargo bench -p kui-core` gives, since that is the command it names.

`frame_10k_segments` and `frame_1k_curves` joined the table and the ratios
under it were re-derived. The editing-latency table was re-measured four
times over: a first attempt taken straight after a long benching session read
the 100k row ~18% high, and four rested runs put it back at 1.79 / 2.08 ms —
where the table already had it. `replay_a_full_depart_store` is the one row
where alpha.7 is consistently slower than alpha.6 (13.1 against 12.4 µs,
five comparisons out of five), which is ~1 µs and below the guard, but it is
the only row that always leans one way.

**Not run, and so not claimed.** Nothing has executed on **Windows** — no
machine or VM was reachable, so `crates/kui/src/windows_nc.rs` has still
never run, and neither has the win32-x64 prebuild. The **published prebuilds
are untested as artifacts** on every platform: what ran here was built from
source on this machine, not the cross-compiled binary the release ships.
`SMOKE_MACOS` / `SMOKE_WINDOWS` stay unset, because turning either on without
a matching runner registered leaves the job queued on every push instead of
reporting anything. **W2's driver half is unbuilt**, so press-drag-release
into a popup was not run: ADR 0009 settled it and only the arithmetic
(`crates/kui/src/retarget.rs`) shipped.

### Added

- **`valueText`: a slider says what its position reads as** (backlog F8,
  from the pomodoro field report). Without it a screen reader has only
  `valueNow` and the range and turns the position into a percentage — 25
  minutes in [5..60] is announced "36 percent", which is what the report
  hit on all three of its sliders. `valueText` is the reading itself:
  `<box role="slider" valueText="25 minutes">`, `value_text` in Lua and C,
  `.value_text()` in Rust. It is ARIA's `aria-valuetext`, and it
  **replaces** the number rather than joining it — settled by driving the
  real macOS accessibility API before the row was designed:
  `accesskit_macos` gives a node one value slot, and a string in it wins
  over the number. The range still travels (F10's check reads the numbers,
  not the reading), `AXIncrement` and `AXDecrement` still work, and a nudge
  announces the new *text*
  (`docs/adr/0008-live-regions-and-announcements.md`). It arrives back as
  the access node's `value`, the same field an editor's text uses, so
  nothing on the readback side grew and `KUI_ABI_VERSION` stays at 7.
  Meaningful on the slider role alone, like the three numbers.
  **What you can delete:** the name that carries the reading. An app
  working around this baked the value into `label` — `"FOCUS LENGTH, 25
  MINUTES"` — which is the wrong attribute twice over: it renames the
  control on every nudge, so a reader announces a *new control* rather than
  a new value, and the name is what a voice-control user has to say to
  reach it. Move the number to `valueText` and let `label` go back to being
  the control's name.

- **Node's `init` and `view` are handed the surface** (backlog F11, from
  two field reports). The function form of `init` takes it —
  `init: (surface) => model`, called after `setup`, so a first model
  measures against the fonts `setup` registered and, under a window, reads
  the size the window really opened at — and `view` takes it third:
  `view(model, window, surface)`. Both are additive and typed by the `S`
  that `LoopConfig` already carried (`Ctx` headless, `KuiWindow` under
  `runWindowed`), so a value `init` and a two-argument `view` are unchanged.
  This is what makes the README's own suggestion for `measureText` — size a
  column to its widest label — something a view can do, rather than
  something only `update` could.
  **What you can delete:** the module-level variable an app parked the
  surface in from `setup`, the hardcoded window size its first model was
  built against, and the `resize` handler that existed only to correct it —
  with it, the frames between the first draw and that correction, where the
  camera and every hit test were off. `examples/node/counter-window.tsx`
  deletes its `openedAt` exactly this way. Note that a headless `Ctx` still
  has no `size()`: the size it draws at is the one you hand `createApp`.

- **`press` and `release`: one key, both channels** (backlog F6, from the
  mind-map and pomodoro field reports). A key press has always been two
  events, and a window has always sent both: the raw press to whatever
  holds key focus, and then what the *core* is asked to do with that key —
  Escape dismisses a modal, Tab walks the focus ring, an arrow nudges a
  focused slider, Space presses a focused control, a printable character
  reaches the focused editor. Headless, the two were separate calls with
  nothing saying so, so a test picked one and got half a keyboard: six of
  the mind map's first-run failures were `keyDown('escape')` reaching the
  editor's keymap and leaving the modal it sat in open, and the pomodoro's
  "the arrows do nothing on a focused slider" is the same split from the
  other side. `ctx.press(code, mods)` / `app.press(code, mods)` in Node,
  `Core::press` / `Core::release` in Rust (which is how a Lua extension's
  host drives it), and `kui_input_press` / `kui_input_release` in C now do
  what the window does, in the window's order. **The old calls stay as the
  halves** and say so: `key()` drives the second channel alone, `keyDown()`
  / `kui_input_key_down` the first, for a test that means to drive one and
  not the other. The table that maps a key to its second event moved into
  the core (`KeyPress::edit_event`) and the winit runner reads it from
  there, so there is one copy of it rather than one per driver — which is
  what let the two drift in the first place. No behaviour changed in a
  window, and C gains two functions without an ABI bump.

- **A stroke primitive: `QuadKind::Segment` and the `line` element**
  (`docs/adr/0010-a-segment-primitive.md`, backlog F12 from the mind-map
  field report, whose every connector was three thin boxes). A segment is a
  round-capped line between two endpoints, drawn by an SDF capsule in the
  same über-pipeline and the same draw call as everything else; `Quad`
  did not grow — the endpoints ride in the `uv` slot glyphs use, the width
  in `border_w`. `<line from to width color/>` draws one,
  `<line points curve/>` a polyline through the points or a smooth curve
  through them, **flattened in the core** so every binding gets the same
  segments and the corpus pins the count (`line { … }` in Lua, `kui_line` /
  `kui_polyline` in C, `ui.line` / `ui.polyline` with a `Stroke` in Rust).
  A line is **never in layout**: it floats, sized to its own bounding box,
  in its parent's box space (`float="viewport"` for viewport space), takes
  no room in a row or column, and paints in the float pass in tree order,
  so a connector declared before two cards sits under them. Its colour is
  the node's `bg` slot, so `transition` eases it and `enter` / `exit`
  reach it; `exit` replays a line's points like any ghost. It takes **no
  pointer input** and has no access row (`line-ignores-input` says so when
  a line declares a click). Not in it, each with its reason in the ADR:
  paths, fills, dashes, arrowheads, a tweening width, and a shape-aware
  hit test — the last is the same unbuilt change rounded hit-testing waits
  on. Measured: a 10k-segment frame costs about 8% more than the 10k-rect
  frame with the same quad count, and the plain frame is unchanged.
  `examples/rust/connectors.rs` is a mind map whose links are curves.

- **Popup windows** (`docs/adr/0004-multi-window.md`, decision 9 — the last
  of ADR 0004's five build steps). A `windows` entry can say
  `kind: "popup"`, and what opens is a menu surface rather than a window
  with the app's chrome: borderless, off the taskbar, above and owned by
  the window that declared it, and placed in **screen** coordinates against
  an `anchor` — the `{x, y, w, h}` an `onLayout` node already reports for
  the field or button the menu belongs to, so nothing new has to be
  queried. It does not take OS focus, so the field that opened it keeps its
  focus ring while the driver routes that window's keys to the popup, and
  the arrows walk the list with the ring still on the field.
  A press outside it or Escape arrives as **the same `dismiss` event a
  `modal` node gets** — `{kind: "dismiss", reason, name, id}`, on the root
  — and closes nothing: the app stops declaring the window, on the frame it
  decides to. So a dropdown that outgrows its window is a change to a
  declaration and not to a handler.
  Reach for one only where an in-window float cannot go: a list taller than
  the window, a menu near an edge with nowhere in-window to sit, a panel
  beside the app. Everything else stays `float` with `fit` plus a `modal`
  node, which costs one tree and one draw call where this costs an OS
  surface, a swapchain, a `Core` and an accessibility adapter.
  `examples/rust/popup.rs` is a combobox in a 360x150 window whose 300-tall
  list draws well past the frame.
- **`kui_window_dismissed`** in C, `ctx.windowDismissed(id, reason)` in
  Node: a host driving its own windows reports a press outside a popup or
  an Escape, and the app hears the `dismiss`. `kui::app(…).run(…)` and
  `runWindowed` do it themselves.

- **Live regions and one-off announcements**
  (`docs/adr/0008-live-regions-and-announcements.md`), the last thing ADR
  0001 named and did not build. Two shapes, because the problem has two
  halves.

  **The row.** `live="polite" | "assertive"` marks a node a live region:
  when the text inside it changes, a screen reader reads the change
  without being asked. It is a plain schema row in all four bindings
  (`live` in JSX and Lua, `KuiSpec.live` / `KUI_LIVE_*` in C). A live box
  is never elided — a region that vanished from the access tree could not
  carry liveness to its text — and it reads as **one message**: the text
  inside becomes its name, and that name is what moves when the message
  does.

  **The verb.** `ui.announce(text, live)` in Rust, `ctx.announce(text,
  live)` on both Node classes, `env.announce(text, live)` in Lua,
  `kui_announce` in C: something to say once, with no node behind it
  ("Saved", "3 results"). Queued and drained like every other channel the
  core has — `Core::take_announcements` / `ctx.announcements()` /
  `kui_take_announcements` — so a headless test asserts on what an app
  asked to say, and the corpus's `live` scene pins it in all four
  bindings. The windowed runners drain every frame whether or not
  assistive technology is attached, so nothing is spoken late.

  Two diagnostics come with it: `live-region-without-name` (a region with
  no `label` and no text, which can never announce anything) and
  `announcement-repeated` (the same text on two consecutive frames —
  what an unguarded `announce` in a frame builder looks like).

  Verified against the macOS accessibility API, not only against kui's
  types: `scripts/ax-audit.swift` now observes the
  `AXAnnouncementRequested` notification VoiceOver listens for, and
  checks that a region's changed text announces, politely, that a
  node-less announcement arrives, and that the same message twice in a
  row is said twice. 96/96.

- **A node is named by the label its `key` declared** (backlog F5). Node's
  `focus`, `isFocused`, `reveal` and `access` took only the hex key an
  event carried, so a node the user had never touched — "focus the editor
  I just created" — could not be named at all. They now take either
  spelling: `focus('note')` resolves the label through the last frame
  (`Core::key_of`, `Ui::key_of` in Rust; the build records every keyed
  node's label as it goes, into a buffer cleared between frames). Lua's
  `env.set_focus`, `env.is_focused` and `env.reveal` take a string beside
  the integer, and C gets `kui_key_of(ctx, label)` for its `uint64_t`
  callers. Labels are unique among siblings, not across a tree, so two
  nodes on one label resolve to the first in tree order with a new
  `ambiguous-key` warning; a label nothing declared is an error naming
  both spellings, where `bad id "beta"` named neither.

- **`slider-value-out-of-range`** (backlog F10, from the pomodoro field
  report). A `slider` whose `valueNow` is outside its own `valueMin` /
  `valueMax`, or whose `valueMin` is above its `valueMax`, was advertised
  verbatim and warned nothing — the app that clamps in `update` keeps the
  range in two places, and the drift is visible only to a screen-reader
  user. Now the diagnostics walk compares the rows a slider declares
  (only those: a slider with no `valueMin` has no floor) and reports the
  node once, beside `image-without-label`. One more row in the single
  warnings table, so the Node `WarningCode` union and `docs/props.md`
  regenerated from it.

### Fixed

- **A tween whose target moves every frame now moves too** (backlog F15,
  the mind map's canvas). Retargeting starts a fresh leg at progress zero,
  and the new leg began from the value sampled on the *previous* frame — so
  each frame put a frame's worth of time in and got no motion out, and a
  target the view moved again next frame never got a frame that did not
  retarget. The value sat exactly where it started for the whole gesture
  and jumped only once the target held still. That is a canvas of `slide`
  floats panned by a drag: every card's position changes every frame, so
  the map froze on screen while `pan` in the model was correct the whole
  time. A retarget now reads where the running leg stands *at that instant*
  and continues from there, the way CSS does, so the value trails its
  target by a fixed distance — about one transition's worth of travel —
  instead of standing still. Springs were never affected: they carry
  velocity across a retarget and integrate per frame.
  Nothing about it was window-specific — a real window and `ctx.mouse`
  emit the same `drag` events for the same gesture — but only a window was
  stamping a real clock, which is why every headless assertion passed and
  the report said "not in the window".
  **What you can delete:** the transition an app took *off* its canvas to
  make dragging work, and any pan reimplemented outside the view to dodge
  it. `examples/node/mindmap.tsx` is the shape, with its connectors easing
  beside its cards — `slide` is on the `line` element's props now, which it
  was missing from in TypeScript while the runtime had always honoured it,
  so a canvas that eases everything (the rule `slide`'s own doc states) is
  finally the one that type-checks.
- **A modal gives focus back unless the closing frame says otherwise**
  (backlog F4, the mind map's rename editor). The focus a modal displaces
  comes back when it goes away (ADR 0003, decision 4) — but a rename
  editor opened on a node created in the *same* frame displaced the node
  the user came from, so dismissing it put focus back there and the next
  Enter added a sibling in the wrong place. The app had no way out:
  `focus()` needed a key it did not have, and `keyFocus` on the new node
  had spent its edge. Now a `keyFocus` edge on the frame a modal stops
  being declared — a node declared focused there and not on the frame
  before — stands, and the remembered focus is dropped. That is
  `initialFocus`'s missing half: `initialFocus` says which control a
  dialog opens on, `keyFocus` on the closing frame says where the
  keyboard lands on the way out, and neither needed a new row. An app
  that declares nothing, or that repeats one declaration every frame (a
  key sink owning its keyboard), is untouched — a redeclaration is no
  edge, so the restore still lands where it always did. The corpus's
  `modal` scene drops its dialog in a second phase and declares the node
  it was renaming focused, so all four bindings agree on where the
  keyboard ends up.
- **A `transition` eases under `createApp`, so it is testable from Node**
  (backlog F1, the mind-map field report's #8). The loop set the core's
  frame clock only inside `advance(ms)`; `render()`, `click()`, `type()`
  and every event-driven frame ran with the clock unset, where the core
  snaps by design — so a keyed box going 100 → 400 under `transition: 200`
  was already at 400 in the frame that applied the change, and
  `animating()` said true for 200 ms while nothing moved. The loop now
  stamps the clock when it is built and before every frame it draws, so
  the baseline is taken at the loop's time and the first `advance` is
  mid-flight. A test that wants only the end state advances past the
  duration. Nothing changed for a window: its runner already stamps each
  frame from its own epoch, and `KuiWindow` has no `setTime`.
- **The accessibility example's Actions menu was drawn half outside the
  window.** `FloatConfig::below()` centres a float on its anchor, so a
  180-wide menu under a 90-wide button near the left edge hung off it, and
  without `fit` nothing pulled it back — the item labels read "name",
  "plicate", "hive". It is left-aligned under the button and `fit` now,
  which is the placement a menu wants and the in-window answer ADR 0004
  decision 11 keeps `fit` for. The example, not the library: no float
  behaviour changed.
- **A `Chrome::Borderless` window is no longer dead to the mouse on macOS**
  (backlog W1). `with_decorations(false)` produces an `NSWindow` with the
  borderless style mask, and AppKit never sends `mouseUp:` to one — so every
  press in such a window landed and never released: no click, no drag end,
  no pressed style cleared. Chromeless is now spelled as a hidden titlebar
  over a fullsize content view, which is what `Chrome::Custom` already asked
  for and what a Mac app that draws its own chrome actually wants. Such a
  window also **gains rounded corners, a drop shadow and native edge
  resizing**, none of which a borderless one had; if you were drawing your
  own square corners under the old behaviour, that is the one thing to look
  at.
- **A window no longer greys out while you press an item in its popup.**
  AppKit makes a window key when you press it, and the *platform's own*
  titlebar dims under whatever is no longer key — however `env.focused`
  reads, since that is kui's answer and not the OS's. A non-activating
  popup that ends up with the keyboard now asks for it to go back
  immediately (once per acquisition, since the request is advisory), so the
  window behind keeps its key styling for the whole press. Nothing else
  notices this: it is not a state any frame is drawn from.
- **A window no longer flashes unfocused while a popup opens or closes.**
  What a view reads as `focused` is now derived from what the platform said
  about *every* window, once per event batch, rather than written per event
  — because focus moving between two windows is two events (three around a
  new window, which winit announces with a `Focused(false)` of its own), and
  in the middle of any ordering of them no window claims the keyboard.
  Reading that moment was the flicker. An owner and its non-activating popup
  also read as focused *together*, since between them the keyboard is being
  routed rather than lost, and a popup that is destroyed while holding it
  hands it back to its owner instead of leaving the owner to wait for the
  platform. `env.focused` is unchanged for a single-window app.
- **A window's first frame no longer waits for a mouse move.** A window that
  has just been ordered front reports its surface *occluded* for a frame or
  two while the platform catches up, and `render` answering "nothing to
  present, try next frame" was scheduling no next frame — so nothing else
  being on the event loop, the window sat blank until a stray input woke it.
  A window that has not yet presented now asks again, one frame apart and at
  most sixty times; in practice a popup lands on the third try, about 50ms
  in. Visible on any second window, popup or not.

- **`AccessMsg` takes the app's union** (backlog F9, pomodoro 2.2). It was
  the one core message with `tag?: unknown` while `DragMsg<T = AppMsg>`
  and `KeyMsg<T>` carried the app's messages, so handling a slider nudge
  needed `p.tag as PomoMsg` in a library whose pitch is one union and no
  casts. Now `AccessMsg<T = AppMsg>` with `tag?: T`; `CoreMsg` picks up
  the default, and `examples/node/counter.tsx` reads a step off a nudge's
  tag with no cast.

### Changed

- **`curve` is a centripetal Catmull-Rom spline, not a uniform one**
  (`docs/adr/0010-a-segment-primitive.md`, amendment). A uniform parameter
  is a claim that the knots are evenly spaced: every span gets one unit of
  curve however long or short its chord, so the stroke has to move fast
  through the tight ones. On an elbow that is a bow out of the wrong side
  of the corner — `examples/rust/connectors.rs` had three links leaving one
  card 10.3 px below the edge they start on and crossing each other doing
  it — and on knots spaced unevenly enough it is a loop: over 20,000 random
  four-knot sets the uniform middle span crossed itself in 82 and the
  centripetal one in none, which is a theorem and not a sample. Each span's
  parameter is `sqrt(chord)` now, the end knots are mirrored rather than
  doubled — a repeated knot is a zero-length chord and centripetal has no
  parameter for one, so a duplicated point in a list is a kink instead of a
  division by zero — and a span is still cut into `ceil(chord / 6)` pieces,
  so **no scene's segment count moved**. It costs 11% on `frame_1k_curves`:
  7.7 ns a segment against 6.9, flattening included.
  **What breaks:** the same `points` draw a different curve. Nothing about
  the count, the box or the endpoints changes — a run still starts, ends
  and passes through every knot exactly — but a test that pins a curve's
  interior, or a digest over one, will see new numbers.
  **And what it does not fix:** a spline passes *through* its knots, so it
  still leans into a corner it has to arrive at; centripetal halves that
  lean rather than removing it. A shape whose middle points should only
  *pull* is a Bézier, which is what the connectors example samples for its
  links now — the four points it used to hand to `curve` were a Bézier's
  control points and never knots.
- **Keys a focused control does not claim bubble to the nearest enclosing
  key sink** (`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, backlog
  F7, from both field reports). An app could have a keyboard shortcut or a
  Tab ring, never both: a sink hears a press only while it *holds* focus,
  so a root `onKey` box either kept focus through every Tab — the
  pomodoro's three sliders were reachable by a screen reader and not by
  the keyboard — or handed the ring on with `focusNext` and went deaf, and
  Space stopped starting the timer. Now a focused control keeps the keys
  the core presses it with (**Enter** and **Space** where there is
  something to activate, a **slider's arrows**, a **composite's** arrows,
  Home, End and type-ahead) and **Tab stays the ring's wherever focus
  is**; every other press — a letter, a function key, Escape, and any
  chord, since ⌘ and ⌥ are what a shortcut layer is made of — walks up to
  the first non-disabled `onKey` ancestor and arrives there as the same
  `{kind:"key"}` payload a sink already handles. The nearest sink wins, a
  bubbled release follows its press under the same `keyUp` opt-in, and the
  walk stops at a modal boundary, so a shell under its own dialog is as
  inert as the app it wraps. **No new prop:** a shell is an `onKey` box
  around its content, which is what both reports already wrote. A sink
  that holds focus still keeps every key, Tab included (ADR 0002, decision
  3, unchanged, and now half of a pattern rather than a whole answer).
  **What breaks:** a chord that used to press the focused control — ⌘Enter
  on a button — reaches the shell instead, and a focused control's Escape
  now goes to a shell that is listening rather than blurring. The corpus's
  `keys` scene grew a shell over a ring, so all four bindings reproduce it.
- **A key sink hears presses only, unless it says `keyUp`** (backlog F3,
  from the pomodoro field report). alpha.6 made `onKey` deliver both
  halves of every key to one sink as `{kind:"key", phase:"down"|"up"}` —
  the right shape, and the guarantee behind it (a key only comes up where
  it went down, which two sinks could not promise) stands — but it made
  press-only, which is what every keymap is, the case that needed a guard.
  An alpha.4 app bumped to alpha.6 type-checked, ran, and toggled every
  shortcut back: Space started and paused the timer, `m` and `a` flipped
  twice, and nothing pointed at the one line (`phase !== 'down'`) that
  fixed it. Nothing *could*: a sink that ignores `phase` looks exactly
  like a sink that wants both, so no warning tells them apart. So the
  default is presses again, and releases are one flag away. **`keyUp`**
  (`key_up` in Lua and C, `.key_up()` in Rust) beside `onKey` delivers
  both halves, the payload shape unchanged and the guarantee kept: a
  release whose press the sink never got is still dropped, and focus
  leaving still lets go first — to a sink that asked. A sink without it
  hears nothing on the way up, synthetic or real; the key is still tracked
  as held, so a stray release resolves silently rather than to a second
  event, and a sink that opts in mid-hold hears the release it is owed.
  One schema row, so all four bindings got it mechanically, and the ABI
  parity test forced the C field (`KuiSpec.key_up`, an [in] append — no
  version bump). The corpus's new **`keys`** scene pins both behaviours
  across the four transports, with `keydown` / `keyup` steps to drive
  them, and `packages/kui/test.mjs` has the fixture the report was
  missing: a keymap that presses *and* releases, and toggles once.
  **What breaks:** a held-key binding written against alpha.6 — WASD,
  press-and-hold, a key that arms a mode — stops hearing its `up` until
  the sink adds `keyUp`. Nothing else changes.
- **The C ABI went 6 → 7, and this is the one bump the `size` handshake
  cannot absorb.** `KuiWindowConfig` gained four `anchor_*` floats and
  `KuiWindowCommand` gained `owner` — each the compatible kind of change on
  its own, but `KuiWindowCommand` embeds a `KuiWindowConfig` *by value*, so
  appending inside the config moved every field after it and lifted the
  floor `kui_take_window_command` accepts past the whole size of the ABI-6
  struct. An un-recompiled ABI-6 binary is **refused** rather than
  short-written: no corruption, but its drain loop sees an empty queue
  instead of its windows. Check `kui_abi_version()` before your first call
  — this is the release where that check is the difference between a
  message and a mystery. Recompiling changes no source.
- **`unknown-window-kind` now means a kind neither `KUI_WINDOW_KIND_NORMAL`
  nor `KUI_WINDOW_KIND_POPUP`.** JSX and Lua still refuse an unknown kind
  where it is written rather than warning a frame later.
- **A handler that runs now redraws every window**, not only the one the
  input landed in — one app and one model means a press in one window can
  change what another declares, which is exactly how choosing an item in a
  popup closes it. Only when more than one window is open; a single-window
  app draws what it always drew.
- **Drag deltas are displacements from the press point** (backlog F2, the
  "what breaks" above). `DragMsg`, the `EVENTS` row (so `docs/props.md`),
  `kui_open_draggable`'s comment and the README now say what `dx`/`dy` are
  relative to; `crates/kui-core/tests/drag.rs` pins press, 2 px, 2 px, 4
  px, release as `move 4`, `move 8`, `end 8`, and the corpus's new `drag`
  scene carries the deltas on its event rows — `event drag split move 8
  0` — so a binding that summed steps would fail to match the reference
  rather than agree on the kind and disagree on the number. Of the
  consumers this repo ships, only `examples/rust/splitmux.rs`'s tab
  reorder read a per-move `dx` (for its sign); it now differences two
  `dx`es. The others already anchored to the absolute `x`/`y`, which is
  the shape the report recommended and is unchanged.

- `KuiAccessNode` reports liveness as two new `flags` bits
  (`KUI_ACCESS_LIVE_POLITE`, `KUI_ACCESS_LIVE_ASSERTIVE`) rather than a
  new field, so the [out-array] struct's layout is unchanged. **No ABI
  bump**: `KuiSpec.live` is an [in] append and `kui_announce` /
  `kui_take_announcements` are new functions (ADR 0006, decision 2).
- `examples/rust/accessibility.rs` scrolls its controls instead of
  sizing the window around them, and gained a live status line and a
  Copy button that announces.

### What you can delete

- **The knots you added to talk a curve out of its overshoot.** The extra
  point either side of an elbow, the corner nudged off the grid, the second
  copy of a knot that a uniform span was looping around: a centripetal span
  does not loop, and a link that wants *handles* rather than waypoints is a
  Bézier the caller samples into `points` — `examples/rust/connectors.rs`
  is that, in one function.
- **The three-box connector.** The stub, the vertical run and the second
  stub a diagram drew for every link, the colour arithmetic that decided
  which of two overlapping stubs won, and the elbow-only layout the boxes
  forced: one `<line points curve/>` per link, in the same coordinates the
  cards already float in, replaces all of it.
- **The alias a shortcut was hidden behind, and the `focus()` after every
  keypress.** A node that is both a control and a sink never had its
  Enter, Space or Tab claimed by the core — the mind map's `insert` alias
  hedged against a collision that was not there, and the README now says
  so in a sentence. And a shell that took focus back after handing the
  ring on, to keep hearing its own keys, can stop: what its controls do
  not claim arrives on its own.
- **The `phase` guard in every keymap.** `if (msg.phase !== 'down')
  return` — or the alpha.4 → alpha.6 migration line, `phase !== 'down' ||
  repeat` — is what a sink without `keyUp` does by itself now. The four
  Rust examples that carried it lost it. `repeat` is still yours to
  filter: an auto-repeat is a press.
- **The accumulator behind a drag.** The model field that summed `dx` over
  `move`s so that something could be committed on `end`, and the `start`
  handler that zeroed it: `end` carries the total now, and `x - x0` computed
  by hand from the absolute coordinates is what `dx` is.

- **The second call after every simulated key.** The `ctx.keyDown('escape')`
  followed by `ctx.key('escape')` that a test needed to press one key — and,
  more often, the missing half of that pair and the assertion written around
  it (a modal asserted still-open because it was, a slider whose arrows were
  believed not to work). `press('escape')` is the whole key. Any test that
  really did want one channel keeps the call it was already using.

- **The workaround for a list that did not fit.** Whatever a view did to
  keep a long dropdown inside the window — a scroll container sized to the
  space left below the field, a menu that opened upwards past a hand-rolled
  threshold, a "show 6 of 40" that existed because 40 did not fit — is a
  `kind: "popup"` declaration and an `anchor` now. The handler does not
  change: it was already answering `dismiss`.

- The `app.ctx.setTime(s)` calls a test wrapped around each `render()`,
  or the `app.advance(0)` it made before a change, to see a `transition`
  move at all. The loop keeps the clock now; `advance(ms)` alone moves it.

- The state field that held a status message *only* so a screen reader
  would see it change, and the code that cleared it a frame later. A
  message with a place on screen takes `live` on the node it is already
  in; one without takes `announce` and needs no node at all.

- **The key harvest.** An `onLayout` on every node whose only job was to
  learn its hex key so `focus()` could be called later — an event per node
  per frame — and the model field that stored them. A node is `focus('its
  label')` now, and the access-tree lookup that found an editor's rect just
  to click it into focus is `focus('note')` too.

## 0.1.0-alpha.6 (2026-09-05)

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

**What breaks.** **The C ABI has a version now, and it is 6**: alpha.5 had
none at all, and 1 through 5 came and went inside this release's own
development (bumps are per change, not per release — ADR 0006 decision 8),
so 6 is the first number anything shipped. Every C host recompiles and
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
`docs/backlog/closed-2026-09.md` carries the reasoning and the alternatives
declined, and `docs/BACKLOG.md` what is still open.

### Native verification

The five prebuilds are cross-compiled on one Linux runner, and P8's
`smoke-macos` / `smoke-windows` jobs are gated on repository variables no
runner satisfies — so nothing had executed on a platform since alpha.5.
This is what was run by hand before the tag (backlog R4), and what was not.
Re-run in full on 2026-09-05 after the exit-layering fix below landed; the
numbers are that run's.

**macOS 26.6.2 (arm64), rustc 1.98.0, 2026-09-05.** `cargo test --workspace`
passes: **552 tests over 55 suites, 0 failed, 0 ignored**. The workspace
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
and `c_panel` — the C plugin dlopened into a Rust host. On the re-run every one
of those eleven windows was captured with `screencapture -l` and looked at,
and the toasts example was driven through the AX API (press *toggle panel*,
two *notify*, *toggle panel*) with the window captured twice inside the
420ms exit: the panel's ghost slides out **under** the latency HUD, where
the live panel sat, instead of jumping over it — the layering bug the fix
below is for, confirmed on screen and not only in `tests/exit.rs`.

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
  space, which is also how CSS's exit transitions work), **in its place and
  outside every clip** — the pass it painted in, just under the node that
  painted after it, so a side panel that sat under the HUD leaves under it
  rather than jumping to the top of the window for its last few frames;
  and outside every clip because its ancestors may be gone — and **inert**
  — no hit region, no Tab stop, no access row, because it is a picture of
  a node rather than a node. It is dropped when its transition ends, and immediately if the key
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
  that all declare one costs about 190 µs once, and a full 512-node store
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

- **The Rust counter has a context menu** (backlog R4). `on_context_menu`
  shipped with a core test and a corpus scene, and Node's and C's counters
  both demo it — but no Rust example did, so the one binding most people
  read first had no worked example of the feature. `examples/rust/counter.rs`
  now declares it on the root and floats a `modal` menu at the press, the
  same shape `counter.tsx` and `counter.c` use: the core reports
  `{kind="contextmenu", x, y}` and opens nothing, the view puts the menu
  there next frame, and `modal` does the rest — the buttons behind it stop
  taking clicks, Tab is scoped to the menu, and Escape or a press outside
  comes back as `{kind="dismiss"}`. `FloatConfig::fit` keeps it in the window
  when the press lands near an edge. Driven natively through the OS rather
  than asserted headlessly, which is also how the float-ordering bug in the
  first draft was found: the menu was declared before `latency_hud`, and the
  HUD — a float too — drew over it.
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
  **P3 in `docs/backlog/closed-2026-09.md` is why both exist today**:
  `env.focused` was spent on the window fact before the node reading needed
  it, and neither name can move inside 0.1 — renaming either is breaking,
  and simply dropping `env.focus` would leave the node key unreadable from
  Lua altogether. So this is the announcement, not the change; converging
  the two, and giving the window fact its own unambiguous name, is 0.2's.
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
