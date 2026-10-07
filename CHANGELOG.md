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
field reports). A fix that changes what an existing input draws is a break
too — for whoever wrote to the old behaviour, documented or not — and is
listed under both (backlog F61, from the alpha.12 field reports: the list
is what the release knows it broke, and a fix it did not think of as one
was the first bare bump to break an app in five releases).

## Unreleased

### Added

- **An Odin binding, experimental** (`packages/odin`). `kui/c` mirrors
  kui.h declaration by declaration, pinned to the C compiler's layout by
  848 generated `#assert`s. `kui` is the typed layer, generated from
  kui.h and the prop schema:
  - `Spec` and `Text_Style`, one field per schema row;
  - an enum or bit set per family of constants;
  - a door per C function: out-params as results, arrays as slices,
    messages as plain Odin values with `#[derive(Message)]`'s `kind`
    rule.

  The elements are written by hand: containers close at the end of their
  `if` through `@(deferred_in)`. A C function the hand-written files do
  not call gets a generated door, so the binding covers kui.h whole: 219
  generated, 51 by hand, 2 skipped with their reason.
  `examples/odin/tools/surface.odin` is `surface.c` through it, and calls
  every door, and `examples/odin/tools/conformance.odin` rebuilds the
  scene corpus through `Spec` and the doors, matching the reference report
  byte for byte, as the Rust, Lua, C and Node adapters do.
  `polyline` and `polygon` take points (`[][2]f32`), the count kui.h
  means.

  It covers extensions both ways. An Odin host loads a plugin with
  `ctx_add_extension` and `slot`. An Odin plugin is a shared library whose
  seven `kui_ext_*` exports are one line each over `extension.odin`.
  Built with `KUI_PLUGIN`, it links no kui and loads into any host:
  `nu scripts/odin.nu slots` drives the Odin panel in the C and Rust
  hosts and the C panel in the Odin host.

  Run it with `nu scripts/odin.nu gen | test | slots | run counter`. It is
  not published. CI's `check` installs a pinned Odin release (`ODIN_VERSION`)
  and runs `gen --check`, `check`, `test` and `slots`, so a header or schema
  change that was not regenerated goes red there. All four also run green
  by hand on Windows x64 and Linux x64. On Windows the binding links
  `kui_ffi.dll.lib`, odin.nu puts `kui_ffi.dll` beside the programs, and
  `gen --check` reads a CRLF checkout as current.
- `scripts/pack-ffi.nu`: libkui_ffi for every platform kui ships, dynamic
  and static, with `kui.h`, rustc's `native-static-libs` as `link.txt`, a
  tarball each and `SHA256SUMS`. It is for a C or Odin host that links a
  library instead of building the workspace. The machine's own platform
  is built natively. Linux x64 and arm64 (glibc 2.28) and Windows x64 are
  built in a pinned Docker image with the release workflow's
  cargo-zigbuild and cargo-xwin. Windows also asks for
  `--accept-msvc-license`, for the CRT and SDK xwin downloads. macOS is
  built on a Mac only. A Windows pack carries windows-targets' import
  libraries (`windows.0.53.0.lib`, `windows.0.52.0.lib`), which its
  `link.txt` names and no SDK has, so a static link finds them.
- `examples/rust/tools/schema-dump.rs`: the prop schema (props,
  composites, elements, events, the verb table, the name lists) as JSON,
  for a binding generated outside Rust.

## 0.1.0-alpha.41 (2026-10-07)

**What breaks.**

- A leaf that declares `tooltip` — `line`, `polygon`, `path`, `cells`,
  `image`, `edit`, in any binding, or a Rust leaf whose spec says
  `NodeSpec::tooltip` — draws its hint while hovered, where it drew
  nothing; so does a C `kui_fragment*` node with `KuiSpec.tooltip`.
- A `dash` gap the round-capped marks overlap closes: `dash = {2, 2}` at
  a width of 8 draws a solid line where it drew 8 px dots every 4 px.
- With a fallback list set, `Mono` text asks the app's fallback faces
  before the machine's other monospaced ones, so a character may draw in
  another face, and a single-weight monospaced face draws its bold
  synthesized instead of in the platform's proportional face.
- A cell grid whose family has no `M` draws its own glyphs where its face
  puts them, no longer centred as a fallback's.
- On a line past 4 KB, a tab after more than ~512 bytes with none moves
  onto the line's tab stops, up to a tab's width, and the rest of the
  line with it.
- Rust: `schema::PropsOut` gains `ime_off` (under Added, F125), so a
  struct literal of it needs the field or `..Default::default()`; so do
  `conformance::Expect` and `conformance::Output`, behind the
  `conformance` feature.
- npm: `@qxuken/kui` bundles no darwin-x64 prebuild. On an Intel Mac (or
  an x64 Node under Rosetta),
  build the addon (`cargo build -p kui-node --release`) and point
  `KUI_NODE_LIB` at it; the Rust crates are unchanged there.

C stays at ABI 25 (`kui_set_ime_off` / `kui_ime_off_get` are new
functions) and the Node wire at v21 (a root prop, no frame version).

A leaf's hint is drawn now, and the four paint changes after it are each
what the code always meant to draw: a hint the docs told you to put on a
box around the leaf, a dotted line that was a lumpy solid one at a quad a
dot, a fallback list that `Mono` text never asked, a grid's own glyphs
treated as a stranger's, and a tab measured from where its chunk began
instead of where the line did. Each moves pixels an app may have written
to, so each is listed.

### Added

- **A window can take the keyboard as keys, with the input method off**
  (backlog F125, from kawoosh's wish list). `ui.ime_off(true)` / a root
  `imeOff` / `ime_off = true` / `kui_set_ime_off(ctx, true)` turns the
  platform's input method off in the window: no composition and no
  candidate window, and on a Mac no dead key waiting for the next and no
  press-and-hold — which is an input method too, so a held `j` repeats
  and a held `e` opens no accent picker, whatever the user's
  `ApplePressAndHoldEnabled` says. A `key` event's `text` is still the
  layout's character. What a modal editor's normal mode wants; its
  insert mode stops declaring it and gets accents, dead keys and the IME
  back. Frame state in `optionAsAlt`'s shape, default off; the runner
  applies it on change through winit's `set_ime_allowed`, so winit's
  `keyDown:` stops calling `interpretKeyEvents:`, where macOS composes. A
  key pressed while it was off keeps it off until it comes up, so the
  `i` that switches to insert mode, held, keeps typing `iiii` rather
  than opening the picker over its own repeats; the next fresh press in
  insert mode gets the picker. A composition open when it turns off is
  dropped, and ends in the view as an empty `preedit` — as one does now
  when the user switches input source mid-composition, which used to
  leave the preedit drawn. On Windows and Linux the window's IME is
  disabled the same way, and only that: a dead key there is the
  layout's, which winit composes whatever the IME says, and it still
  composes. `Ctx.imeOff()` / `kui_ime_off_get` read the ask
  back; `modal_editor` declares it in normal mode. *What you can
  delete:* the README line telling a Mac user of a modal editor to run
  `defaults write -g ApplePressAndHoldEnabled -bool false`, and any
  per-mode IME switching an app did by hand.

### Changed

- **Intel Macs lose their Node prebuild.** The package ships the addon
  for linux-x64, linux-arm64, darwin-arm64 and win32-x64; the release
  workflow's darwin-x64 leg and `release-local.nu`'s are gone. On an
  Intel Mac `require('@qxuken/kui')` fails with the loader's not-found
  error, which now says the prebuild was dropped and how to build one.
  `kui-native`, `kui-ffi` and the rest still build and run there; only
  the bundled `.node` is gone.

### Fixed

- **A leaf draws its tooltip** (backlog RG113). The hint floats as its
  node's last child, and a leaf has none, so a hovered wedge, path, line,
  grid, image or editor with a `tooltip` showed nothing — it was tracked
  and spoken, not drawn. It now floats beside the leaf, anchored to it
  (`FloatAnchor::Node`), and lands below the leaf's box as a box's hint
  lands below the box: outside every clip, outside layout, flipped above
  near the window's bottom, and only while the pointer is on the stroke
  or inside the outline. In all four bindings — `NodeSpec::tooltip`, the
  JSX and Lua prop, `KuiSpec.tooltip` on C's leaf doors. A float anchored
  to a node now honours `fit` as a parent-anchored one does. On a leaf
  the drawn string is its description, so a leaf that declares a
  `description` after its `tooltip` draws that.
- **An animating window whose surface skips its frames waits between
  tries** (backlog F103). A skipped frame returns at once, with no
  drawable to wait for and no vsync behind it, so where the platform
  never says a window is covered — Windows behind other windows, Wayland,
  an acquire that times out — an animation asked for its next frame on
  the loop's very next turn: 82,000 views a second at a core, measured on
  a Mac with every frame forced to skip and no display link. It waits a
  retry (16 ms) after a skip now — 60 a second at 2% — and runs at the
  display's rate again from the first frame that lands. A window macOS
  calls covered still asks for nothing at all. On Windows the timer that
  keeps an animation going through a title-bar drag paces the same tries
  at its own interval, 10 to 16 ms.
- **A tab on a long line stops where the line's stops are** (backlog
  RG76). On a line past 4 KB, a tab that followed more than half a chunk
  without one measured from its chunk's start. Such a chunk now ends
  after that tab, and the line gives the tab the advance that reaches its
  next stop — exact once the text since the previous tab has been on
  screen, and as near as the rest of the line's estimate before that.
  Wrapped rows carry the same advance.
- **`measure_text` leaves a drawn text's rows alone** (backlog RG76).
  Measuring, between frames, a text a node drew at another width
  re-wrapped the run they share, so that node's `caret_rect` and
  `text_hit` answered from the measured rows until the next frame wrapped
  it back. The measure lays out a copy at its own width, kept for the
  next measure there.
- **`Mono` text asks the app's fallback fonts straight after its own
  face** (backlog RG118). cosmic-text's generic monospace family walks
  every installed monospaced face that has the character before the
  lists `set_fallback_fonts` joins — on a Mac, Hebrew in `Mono` came out
  in Courier New *Italic* with Arial named. While a list is set, `Mono`
  is shaped as the face it is pinned to, by name, so the order is that
  face, the app's names, then the platform's. With no list nothing
  changes.
- **A grid tells its family's glyphs by the family's name** (backlog
  RG118). It read the family's face off the face `M` shaped with, and a
  symbols-only or CJK-only family has no `M`, so its own glyphs were
  taken for a fallback's — centred, and asked of a monospaced face first.
- **A new fallback list or a font rescan draws every window of the
  session** (backlog RG118). `set_fallback_fonts` and
  `reload_system_fonts` asked for a frame of the window they were called
  through alone; another window shaped again whenever something else drew
  it. Each window that has drawn now owes a frame (`animating()`) while
  the session's fonts are newer than the text it shaped.
- **Dash dots that overlap make one mark** (backlog RG118). A gap no
  wider than the stroke is closed by the round caps either side of it,
  and `{2, 2}` at a width of 8 was 8 px dots every 4 px — a solid line
  drawn lumpy, at a quad a dot. Such a gap now closes and the marks
  either side are one mark; a pattern with no gap left draws solid. A
  pattern whose gaps show draws exactly as before.
- **A long line's placed tab answers the caret and the click where it
  is drawn** (backlog RG122, from the pre-tag pass). A line ending in such
  a tab has its end caret, and a selection's end, at the line's width —
  it fell short of the width in a monospaced face and past it in a
  proportional one, and wrapped rows used the chunk's own narrower tab —
  and a click answers the tab before the middle of the drawn tab and the
  byte after it from there, where both halves answered the byte after.
- **A pinned `Mono` keeps to its face at every weight, and follows the
  fonts the app loads and removes** (backlog RG123, from the pre-tag
  pass). A single-weight monospaced face was passed over for bold and set
  in the platform's proportional face; it now draws in itself, with bold
  synthesized. A pinned family whose only face the app removed left
  `Mono` naming nothing, and one loaded after the list was set was never
  pinned; both now move the pin, and every window shapes again.
- **A C fragment draws its tooltip** (backlog RG124, from the pre-tag
  pass). The four `kui_fragment*` doors tracked and spoke
  `KuiSpec.tooltip` and floated nothing, where JSX and Lua drew it.
- The docs of a `gradient` stop whose `$token` misses say what it does:
  `unknown-token` is raised and the stop is left out, so a gradient left
  with fewer than two stops draws nothing over its `bg` — no error, and
  no fade nobody wrote. ADR 0042's amendment says why it stays (backlog
  RG118).

**What you can delete.** A box wrapped around a `line`, `polygon` or
`path` only to carry its `tooltip`; a pattern stretched by hand so a
thick stroke's dots would not touch; a second `set_fallback_fonts` (or a
redraw) sent through every other window of the session.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-07, over
the rounds after the alpha.40 tag — `imeOff` (F125), F103's skipped
half, RG76, RG113 and RG118, and the darwin-x64 prebuild dropped — with
a regression pass over them first: the mechanical round, then three
read-only reviews in worktrees (text and fonts; core paint, layout and
bindings; the runner, the release machinery and the docs), each claim
probed with a test before anything changed. They filed five: RG122–RG125,
built before the tag (a long line's placed tab answering the caret and
the click where its run had it, a pinned `Mono` asked at weights its face
lacks and not re-pinned when the app's fonts move, a C fragment's
tooltip never drawn, and `imeOff`'s docs promising no dead keys on
Windows and Linux, where winit composes them whatever the IME says), and
RG126, open: `cells_200x50_warm`, unguarded, 3 to 5% slower, bisected to
the grid's family-by-name check and not yet explained. F103 itself was
measured in a window first: on macOS a hidden animating window already
built nothing, and with every frame forced to skip and no display link
the probe went from 82,000 frames a second at a core to 60 at 2%. This
round ran on the Mac alone; Windows and Linux did not run it for this
tag, so F125's `set_ime_allowed` there and F103 under Windows' drag
timer are by reading.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 26.10.0, nu 0.116.1**, on the release commit's tree.
`cargo fmt --all --check` and `cargo clippy --workspace --all-targets
--features kui-core/conformance -- -D warnings` are clean, and so is
`cargo audit --deny warnings`. `cargo test --workspace --features
kui-core/conformance`: **1844 tests over 138 suites, 0 failed** (4
ignored). The C round, `cbuild --run`, passes its five checks; the
corpus passes its **57 scenes** in four adapters, the `path` scene now
resting on a tooltip wedge; the ABI is **25**. Node's `node --test
test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`: **212 of 212**. `npm run
gen` leaves no diff, the examples typecheck and their lockfile installs,
the headless round passes all **35 drives**, the book builds and
`scripts/book-examples.nu --check` passes.

**The windowed round**, `smoke -- --node`, twice — before the fixes and
on the release tree: **51 Rust examples and the eleven Node examples,
each on both bases, 120 frames each, every one exiting 0** — 124
windows, eight at a time, in 34.2 and 52.3 s, the second beside another
session's builds — and `counter`, `host`, `c_panel` and `lua_panel` by
hand under `KUI_SMOKE_FRAMES=120`, each exiting 0 with nothing on
stderr: **128 windows over five hosts.** `target/debug/examples` was
pruned first (12,280 files), the alpha.36 trap. The AX audit:
**106/106** on both trees, the audited window raised to the front by its
pid first, and no warning on the fixture's stderr.

**The bench guard** against the alpha.40 tag: **green**, none of the 8
guarded rows more than 10% slower — every one between −1.0% and +0.6%
(the worst guarded run-to-run spread 2.8%), `frame_10k_segments` +0.3%
with RG118's dash — beside other processes using ~500% CPU, so the
medians are not the README's and its table is kept as it was. Of the
unguarded rows the frame bench's moved by at most +1.7%; the `cells`
bench's `warm` row read +6.0% (±1.4%) against the tag, +3.8% against
alpha.40 with F125 merged, and is RG126.

## 0.1.0-alpha.40 (2026-10-07)

**What breaks.**

- Rust: `InputEvent` gains `Open(Vec<String>)`, so an exhaustive `match`
  on it needs an arm.

C stays at ABI 25 (`kui_input_open` is a new function) and the Node wire
at v21.

### Added

- **The documents the OS asks the app to open reach it** (backlog F124,
  from kawoosh). The Finder's Open With, a file dropped on the Dock icon,
  `open -a YourApp file`, a double-click on a document type the bundle's
  `Info.plist` declares (`CFBundleDocumentTypes`): AppKit hands every one
  to the application delegate's `application:openURLs:`, which winit's
  delegate does not answer, so they went nowhere. The macOS runner adds
  it to winit's delegate class once per process, and the app hears
  `{kind:"open", paths}` on the root — file-system paths, one event per
  request, asked for or not. A launch's documents, which AppKit sends
  before any window exists and leaves out of the arguments, wait for the
  main window and arrive on its first turn; while the app runs they
  arrive at once. A delegate class that already answers documents (an
  app's own override) is left alone. Headless and for a host with its
  own window: `InputEvent::Open(paths)`, C `kui_input_open`, Node
  `ctx.openDocuments(paths)` (`OpenMsg` in `CoreMsg`). Windows and Linux
  hand documents over in the process's arguments, and nothing sends it
  there.

**What you can delete.** An `NSApplicationDelegate` method added or
swizzled by hand onto winit's delegate to hear a Finder open, and the
queue that held a launch's paths until a window existed to show them.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-07 — the
one round after the alpha.39 tag: the documents the OS asks the app to
open (F124) — with a regression pass over it first: the diff read whole,
each claim probed before anything changed. The probes ran in a window,
since the half that can break is AppKit's: a scratch app on the
release tree's `kui-native`, bare and in an ad-hoc-signed bundle
declaring `public.data` (`LSHandlerRank` `Alternate`). `open -a` at
launch handed over both documents in one `open` event, `argv` the
executable alone; two `open -a` calls into the running instance arrived
as two events, in order, each with its request's paths in their order.
And the one thing an added `application:openURLs:` could have started
that F124's own checks did not look at: AppKit taking the process's
arguments for documents once the delegate answers them
(`NSTreatUnknownArgumentsAsOpen`). A binary run from a terminal with a
path, a subcommand, a missing file and a flag, bare and as the bundle's
own executable, even with `-NSTreatUnknownArgumentsAsOpen YES`, heard
no `open` — so an app that reads its arguments and hears `open` too
does not open a file twice. The non-macOS side (`pump_open_documents`
without `macos_open`) is clean under clippy for `x86_64-pc-windows-msvc`.
Nothing was filed. This round ran on the Mac alone; Windows and Linux
did not run it for this tag.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 25.6.0, nu 0.116.1**, on the release commit's tree.
`cargo fmt --all --check` and `cargo clippy --workspace --all-targets
--features kui-core/conformance -- -D warnings` are clean, and so is
`cargo audit --deny warnings`. `cargo test --workspace --features
kui-core/conformance`: **1822 tests over 137 suites, 0 failed** (4
ignored). The C round, `cbuild --run`, passes its five checks; the
corpus passes its **57 scenes** in four adapters, the `drop` scene now
ending with two documents opened; the ABI is **25**. Node's `node --test
test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`: **211 of 211**. `npm run
gen` leaves no diff, the examples typecheck and their lockfile installs,
the headless round passes all **35 drives**, the book builds and
`scripts/book-examples.nu --check` passes.

**The windowed round**, `smoke -- --node`, three times over — twice
before the version bump, once on the release tree: **51 Rust examples
and the eleven Node examples, each on both bases, 120 frames each, every
one exiting 0** — 124 windows, eight at a time, in 32.3, 32.5 and
38.8 s — and `counter`, `host`, `c_panel` and `lua_panel` by hand under
`KUI_SMOKE_FRAMES=120`, each exiting 0 with nothing on stderr: **128
windows over five hosts.** The AX audit: **106/106**, the audited window
raised to the front by its pid first, and no warning on the fixture's
stderr. F124 itself was seen in kawoosh's window before the merge, built
against the branch by path.

**The bench guard** against the alpha.39 tag: **green**, none of the 8
guarded rows more than 10% slower — every one between −3.2% and +3.5%
(the worst guarded run-to-run spread 1.3%) — and no row of the run more
than 4.7% slower (`frame_1k_paths_animating`, a frame F124 does not
reach). `README.md`'s table is kept as it was.

## 0.1.0-alpha.39 (2026-10-06)

**What breaks.**

- A `line`, `polygon` or `path` in its parent's box space paints in its
  parent's layer at its place in the tree — over the siblings declared
  before it, under those declared after — where it was a float layer of
  its own, above every in-flow node and every float that had opened
  before it (under Fixed). A stroke declared before a sibling it
  overlaps is under that sibling now; declare it after to keep it on
  top. One anchored `float="viewport"` is placed as it was.

### Fixed

- **A stroke is its parent's content, not a layer over the window**
  (backlog F123, from kawoosh). The core floats every `line`, `polygon`
  and `path` so it takes no room in a row or column, and since ADR 0023
  every float is a layer stacked by when it opened — so a glyph drawn as
  polylines into a pane's title bar, in a pane opened after the first
  frame, painted over a toast the app had floated on every frame since
  the window opened: kawoosh's "a new Kawoosh is installed: relaunch to
  run it" with an `⌥` key cap through its border. A stroke in its
  parent's box space now paints in the parent's layer, at its place in
  the tree, as a child that takes no room does, and is held by the
  parent's clip as before; a stroke anchored to the viewport keeps a
  layer of its own. `Tree::opens_layer` is the one reading, for the live
  pass and for a departing stroke's ghost.
- **`modal-behind-content` reads the paint order, not the float bit**
  (backlog RG120, from this release's pre-tag pass). With a stroke in its
  parent's layer, the check for content over a modal the view did not
  float still took any node under a `float` for a layer over it — and
  every `line`, `polygon` and `path` is one for the room alone — so an
  icon drawn with strokes before the modal's declaration raised the
  warning on a modal nothing painted over. It reads `Tree::opens_layer`
  now; a viewport-anchored stroke, a layer of its own, still warns.

**What you can delete.** A toast or popover redeclared under a fresh key
so it stacks over the icons an app draws with `line` or `polygon`, and
a stroke moved out of its row into a float of its own so a card could
cover it.

- **`scripts/npm-approve.nu` asks npmjs whatever the npm it runs under
  is configured with.** An npm with `@qxuken:registry` pointing at the
  Forgejo copy asks that registry for a scoped package whatever
  `--registry` says. alpha.38 was published by hand during a GitHub
  Actions outage, Forgejo's npm copy first, and the script then read
  Forgejo's dist-tags and said "live on npmjs already" and "latest is
  0.1.0-alpha.38 already" while npmjs held the version staged. Every
  call names npmjs for the scope as well now. A release script, so
  nothing an app sees.

**What you can delete.** Nothing.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-06 — the
two commits after the alpha.38 tag: a stroke painting in its parent's
layer (F123) and the approval script naming npmjs for the scope — with a
regression pass over them first: the diff read whole, each claim probed
with a test before anything changed. Six probes were written; five
passed and are kept in `tests/layers.rs` for what F123's own tests did
not pin (a `polygon` and a `path` under the same rule, a departing
stroke's ghost in its parent's layer, a declared float with `clip` still
a layer, a viewport-anchored stroke still escaping its clip, a clickable
stroke hit at its place), and the sixth found RG120, which is in this
release. This round ran on the Mac alone; Windows and Linux did not run
it for this tag.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 25.6.0, nu 0.116.0**, on the release commit's tree, the
workspace's own artifacts pruned and rebuilt. `cargo fmt --all --check`
and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1816 tests over 136 suites, 0
failed** (4 ignored). The C round, `cbuild --run`, passes its five
checks; the corpus passes its **57 scenes** in four adapters; the ABI
is **25**. Node's `node --test test.mjs` under
`KUI_CONFORMANCE_REQUIRED=1`: **210 of 210**. `npm run gen` leaves no
diff, the examples typecheck and their lockfile installs, the headless
round passes all **35 drives**, the book builds and
`scripts/book-examples.nu --check` passes.

**The windowed round**, `smoke -- --node`, twice over: **51 Rust
examples and the eleven Node examples, each on both bases, 120 frames
each, every one exiting 0** — 124 windows, eight at a time, in
37.6 and 33.4 s — and `counter`, `host`, `c_panel` and `lua_panel` by
hand under `KUI_SMOKE_FRAMES=120`, each exiting 0 with nothing on
stderr: **128 windows over five hosts.** The AX audit: **106/106**, the
audited window raised to the front by its pid first, and no warning on
the fixture's stderr. F123 itself was seen in kawoosh's window before
the merge, built against the branch by path; RG120's case is pinned in
the core's tests alone.

**The bench guard** against the alpha.38 tag, on the tree the release
commit was cut from: **green**, none of the 8 guarded rows more than
10% slower — every one between −5.3% and +1.4% (the worst guarded
run-to-run spread 2.7%), and no row of the run more than 2.2% slower.
Three rows got faster by more than the noise, and that is F123:
`frame_10k_segments` 881 → 834 µs (−5.3%), `frame_1k_closed_lines`
103 → 97.1 µs (−5.9%) and `frame_1k_polygons` 102 → 97.2 µs (−4.5%) —
ten thousand strokes that were ten thousand layers for the float stack
to sort are their parents' content now. `README.md`'s table is kept as
it was.

## 0.1.0-alpha.38 (2026-10-05)

**What breaks.**

- C: `KuiSpec` gains `scroll_mods` at its end (64-bit size 704), so
  `KUI_ABI_VERSION` is 25; recompile. A zeroed spec is what it was.
- Rust: `event::Scroll` gains a `mods` field and `EventSpec` a
  `scroll_mods` one (under Added), so a struct literal of either needs
  them.

The Node wire stays v21: `scrollMods` is one more string row.

### Added

- **An `onScroll` node can be for a modified wheel** (backlog F122,
  from kawoosh). `scrollMods` — `"ctrl super"`, any of `shift`, `ctrl`,
  `alt`, `super`; Rust `NodeSpec::scroll_mods(KeyMods)`, C
  `KuiSpec.scroll_mods` in `KUI_KMOD_*` bits, Lua `scroll_mods` — and
  the node hears only a scroll gesture that began with one of them
  held, and hears it first: ahead of every scroll container and every
  `onScroll` that names none, wherever under the pointer the gesture
  began, the innermost such node winning. A Ctrl-wheel zoom declared
  on the window's root is heard over a list, and the list does not
  scroll; a canvas inside can name the same key and take it for
  itself. A wheel with none of them held passes the node by — and
  scrolls it, if it is a scroll container too: a list can name a key
  for its own zoom and scroll for every other wheel (backlog RG119,
  from this release's pre-tag pass; as merged, such a list stood still
  for a plain wheel). Its
  `scroll` events carry `mods` (`Scroll::mods`), the modifiers held
  when the gesture began: a gesture stays what it began as to the end
  of its glide, whatever is let go or pressed meanwhile.

**What you can delete.** An `onScroll` handler that read the modifiers
to tell a zoom from a scroll, and the scroll it then had to do itself
for the plain wheel — and the knowledge that it only ever worked over
that handler's own node, every scroll container inside it taking the
same wheel first.

### Fixed

- **`scripts/npm-approve.nu` no longer fails a release it has just
  made.** Approving alpha.37 went through, and the script then waited
  a minute for `npm view` to list the version, gave up with "approved,
  but npmjs does not list it yet; run this again", and on the second
  run — no stage left, the cached packument still without the
  version — said nothing was staged and asked whether the release
  workflow had run. `latest` was set by hand. What is live is now read
  off the dist-tags, which a version takes the moment it is approved,
  and `latest` follows the approval at once. A release script, so
  nothing an app sees.

**What you can delete.** Nothing.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-05 — the
two commits after the alpha.37 tag: `scrollMods` (F122) and the
approval script — with a regression pass over them first: the diff
read whole, each claim probed with a test before anything changed.
RG119 came of it and is in this release. Before any of it the last
`check` on main was read on both hosts, green at `3d4fff6` — the step
alpha.37's first tag went without. This round ran on the Mac alone;
Windows and Linux did not run it for this tag.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 26.10.0, nu 0.116.0**, on the release commit's tree,
the workspace's own artifacts pruned and rebuilt. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets -- -D warnings`
are clean. `scripts/test.nu`, the workspace's tests with the
conformance feature: **1808 tests over 136 suites, 0 failed** (4
ignored). The C round, `cbuild --run`, passes its five checks; the
corpus passes its **57 scenes** in four adapters, `scroll-gestures`
now holding a Ctrl-wheel over a contained list at its limit; the ABI
is **25**. Node's `node --test test.mjs` under
`KUI_CONFORMANCE_REQUIRED=1`: **210 of 210**. `npm run gen` leaves no
diff, the examples typecheck and their lockfile installs, the headless
round passes all **35 drives**, the book builds and
`scripts/book-examples.nu --check`
passes.

**The windowed round**, `smoke -- --node`, twice over: **51 Rust
examples and the eleven Node examples, each on both bases, 120 frames
each, every one exiting 0** — 124 windows, eight at a time, in 34 and
35 s — and `counter`, `host`, `c_panel` and `lua_panel` by hand under
`KUI_SMOKE_FRAMES=120`, each exiting 0 with nothing on stderr: **128
windows over five hosts.** The AX audit: **106/106**, the audited
window raised to the front by its pid first, and no warning on the
fixture's stderr. `scrollMods` itself was not driven in a window in
this round: it was tried in kawoosh's before the merge, and RG119's
case is pinned in the core's tests alone.

**The bench guard** against the alpha.37 tag, on the release commit's
tree: **green**, none of the 8 guarded rows more than 10% slower —
every one between −4.1% and +0.6% (the worst guarded run-to-run spread
4.3%), and no row of the run more than 5% slower. `README.md`'s table
is kept as it was.

## 0.1.0-alpha.37 (2026-10-05)

**What breaks.**

- Rust: `HitShape::Path` gains a `stroke` field and `Content` a
  `PathFlat` variant (under Fixed), so a pattern over either needs
  them; `HitShapes::path` takes the stroke's width after the rule.
- A `path` with a stroke over a fill is hit out to the stroke's edge,
  where the half of the stroke outside the outline fell through; and a
  stroked path with no `bg` but a `hover_bg`, `pressed_bg` or
  `focus_bg` is hit inside, where it was hit along its stroke only
  (under Fixed).
- Lua: `path { ops = }` whose numbers are not the flat form draws
  nothing with a `path-malformed` warning, where it was an error that
  failed the view (under Fixed).
- A `path` that animated and then held still for 120 frames draws from
  the atlas again — a `GlyphMask` quad where it stayed a `Texture`
  (under Fixed). Nothing an app sees; a host that counts quads by kind
  does.
- C: `kui_polyline`, `kui_path` and `kui_path_d` take a `dash` before
  `spec` — five floats, or NULL for the solid stroke they drew (under
  Added). `KUI_ABI_VERSION` is 24; pass NULL and recompile. The same
  step appends `gradient` to `KuiSpec` (64-bit size 696).
- Rust: `schema::Kind`, `Apply` and `Parsed` gain a `Gradient` variant
  and `InteractSpec` a `gradient` field (under Added).
- Rust: `Stroke` gains a `dash` field, so a struct literal needs it
  (`Stroke::new` does not); `path::MaskPaint` gains `Dashed`.
- A cell grid's glyph for a character its family lacks is drawn from a
  monospaced face where one has it, in the middle of its cell, and no
  wider than it (F120, under Fixed), where it was the platform's first
  fallback at its own width from the cell's left edge.
- Rust: `Tree` gains an `any_gradient` field.

The Node wire moves to v21 for the dash's five floats on a line and a
path, which an encoder and addon of one release never see apart.

### Added

- **`gradient` on a box, in every binding**
  ([ADR 0042](docs/adr/0042-a-gradient-is-an-image-the-core-paints.md),
  which supersedes ADR 0005's "no gradients"): `gradient={{ to:
  'bottom', stops: ['#1e2030', '#14161e'] }}`, `{ angle: 0.125, stops }`
  in turns clockwise from east, `{ radial: true, at: [0.5, 0], stops }`;
  the same table in Lua; `NodeSpec::gradient(Gradient::to(Side::Bottom,
  [...]))` with `Gradient::angle` and `Gradient::radial_at`; and a
  `const KuiGradient *` on `KuiSpec`. A stop is a colour — a `$token`
  too — or a colour and a position.
  - **Over `bg`, under the border and the children.** `bg` stays a
    colour and keeps its tweens, tokens and state backgrounds; a
    transparent stop shows it through.
  - **An image the core paints.** Each distinct gradient is rasterized
    once into the glyph atlas — a 256-texel strip along an axis, a
    128-texel square otherwise, each with a gutter of the gradient
    carried a texel past its edges, since a stretched quad samples
    there — and drawn as one `Image` quad, so no
    renderer changes and a host that draws an image draws a gradient.
    The key is the gradient and not the box: a resize, another scale
    and a thousand boxes sharing one rasterize nothing.
  - **On the box's unit square.** A side or a corner is CSS's; any
    other `angle` runs corner to corner at an eighth of a turn at any
    aspect, where CSS's pixel-measured `45deg` does not. Stops mix in
    straight sRGB with the alpha premultiplied.
  - **What it does not do.** It does not tween and the state
    backgrounds do not replace it; a hard stop is as soft as the raster
    stretched (a 256th of the box along a strip); there is no conic
    one, and none on a `path`, a `line`, a border or a text. Those, and
    anything animated, stay a `fragment`'s.
  - Measured (the ADR's *Measured*): a linear gradient is within half
    an 8-bit level of the mix computed per pixel at every pixel of the
    box, a radial within 1.2; a gradient box costs about 52 ns over a
    flat one, half of it what any `hoverBg` pays; a raster is 4 µs for
    a strip and 25 to 42 for a square.
  - The corpus gains a `gradients` scene; `examples/rust/apps/loaders.rs`
    gains a rainbow — one gradient two tracks long slid under a clip,
    one strip in the atlas for good.

- **`dash` on a `line` and on a `path`'s stroke, in every binding**
  (backlog V2, the amendment in
  [ADR 0010](docs/adr/0010-a-segment-primitive.md)): `<line dash={[6, 4]}
  dashOffset/>`, `line { dash = {6, 4}, dash_offset = }`,
  `Stroke::new(w, c).dash(6.0, 4.0).dash_offset(n)` (`Dash::of` for a
  pattern read from data), and a `const float *dash` on `kui_polyline`,
  `kui_path` and `kui_path_d`. One length is marks and gaps alike, two
  are a mark and a gap, four a dash-dot.
  - **The lengths are the ones seen.** Every mark is a short stroke
    with the stroke's own round caps, so `6, 4` is 6 px of ink and 4 px
    of nothing at any width and a mark no longer than the stroke is
    wide is a dot. SVG's `stroke-dasharray` measures the centre line,
    which with round caps makes `4 4` at a width of 4 a solid line;
    this pattern is SVG's `mark − width, gap + width`.
  - **The pattern keeps its phase** along the whole stroke — round the
    corners of a polyline and across the pieces of a curve, which is
    what ADR 0010 would not ship without — and restarts at each subpath
    of a `path`, as SVG's does. `dashOffset` starts that far into it;
    growing it moves the marks towards the first point, a marquee's
    marching ants. Neither tweens.
  - **No renderer changes.** A dashed line is one `Segment` quad per
    mark per piece the mark lies on, cut in the core, so a host that
    draws a stroke draws a dashed one; a dashed path stroke is cut by
    the rasterizer into the mask it already was. A path whose offset
    changes every frame is a shape that changes every frame, and
    leaves the atlas for a texture of its own while it marches.
  - A pattern with no gap, a mark and gap under a physical pixel
    together, or more than 16384 marks draws solid. A dashed stroke is
    hit along its whole length, gaps included.
  - The corpus's `lines` and `paths` scenes carry one each, so the
    segment count pins the cut in four bindings;
    `examples/rust/widgets/line.rs` hangs its planned cards off dashed
    links that march under the pointer.

- **The fallback fonts are the app's to name** (backlog F121, from
  kawoosh). `Core::set_fallback_fonts(&[FontId])` — C
  `kui_font_set_fallback`, Node `ctx.setFallbackFonts(ids)` — lists the
  fonts asked, in order, for a character the text's own family has no
  glyph for, before the platform's list, whose first choice on macOS is
  the system's proportional face. For every text in the session — an
  editor already open and a cell grid too — and kept across
  `reload_system_fonts`; an empty list is the platform's alone.
  `Core::fallback_fonts` reads the families back.

### Fixed

- **A cell grid's fallback glyph stays in its cell** (backlog F120, from
  kawoosh: Russian text in a terminal whose family has no Cyrillic, `Ю`
  drawn across the letter after it). A character the grid's family has
  no glyph for is asked of a monospaced face before the platform's
  fallback list — on macOS that list opens with the system's
  proportional face; a glyph still wider than its cells, two under
  `wide`, is shaped at the size it fits at; and the room a narrower one
  leaves is shared either side. The family's own glyphs and the private
  use area's icons draw as they did.

- **What the `path` reviews left** (backlog RG112, the second review in
  [ADR 0040](docs/adr/0040-a-path-is-a-mask-in-the-atlas.md)):
  - A window closed while it showed an animating or a big path left the
    path's texture with the renderer for the life of the process; the
    core now hands it to the session as it goes, and the next frame any
    window draws drops it. A frame built and never read keeps its
    `dropped_textures` and `dropped_fragments` for the next one.
  - A path that stopped moving stops costing a texture and a draw of
    its own: after 120 still frames its mask is the atlas's again. A
    row of icons keyed by position, whose neighbours came and went
    twice within eight frames, read as animating and stayed so for
    good.
  - A stroke over a fill is hit as far as it is painted
    (`HitShape::Path`'s `stroke`).
  - Ops that are not the flat form raise `path-malformed` under the
    node's key in every binding (`Core::path_flat_node`,
    `Content::PathFlat`); C and Node drew nothing in silence.
  - JSX types `width` on `line` and `path` as a number or a `$length`,
    as the encoder reads it; `Core::image_pixels` answers for the
    handle a path's `Texture` quad names.

- **What this release's own pre-tag pass found** (backlog RG114–RG117;
  RG118 holds what it read and left), none of it in a release before
  this one:
  - An editor open when `set_fallback_fonts` was called kept the faces
    its lines were shaped in until they were edited (RG114).
  - The root's `gradient` was dropped while the devtools panel was
    docked (RG115).
  - A dashed `line` whose points coincide drew nothing and was still
    hit; it is the dot its solid one is (RG116).
  - A gradient's `radial` that is not a boolean is an error, where it
    read as linear; its row says that fewer than two stops fail the
    view in JSX and Lua and draw nothing in Rust and C; `kui.h` says
    what a zeroed `KuiGradient` is (RG117).

**What you can delete.** A family chosen only because it covers a
script the preferred one lacks, which can give way to the preferred
one with the fallback list named; the key an app gave a still icon only so a
neighbour's coming and going would not move it to a texture; the
invisible wider path laid under a thick-stroked one to catch the press
on its edge; the WGSL a card's two-colour fade was written in, and the
`fragment` that held its children; the loop that walked a connector's points and declared a
`line` per dash, and the arithmetic that kept its phase round a corner.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-05 — the
nine commits after the alpha.36 tag: `dash`, the `gradient` row, F120,
F121 and RG112 — with a regression pass over them first: three
read-only reviews (the dash, the gradient, the fonts with this
release's docs), each claim probed with a test before anything
changed. RG114–RG117 came of it and are in this release; RG118 holds
what was read and left. This round ran on the Mac alone; Windows and
Linux did not run it for this tag.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 26.10.0, nu 0.116.0**, on the release commit's tree,
the workspace's own artifacts pruned and rebuilt. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets -- -D warnings`
are clean. `scripts/test.nu`, the workspace's tests with the
conformance feature: **1802 tests over 135 suites, 0 failed** (4
ignored). The C round, `cbuild --run`, passes its five checks; the
corpus passes its **57 scenes** in four adapters, `gradients` the new
one; the ABI is **24**. Node's `node --test test.mjs` under
`KUI_CONFORMANCE_REQUIRED=1`: **209 of 209**. `npm run gen` leaves no
diff, the examples typecheck and their lockfile installs, the headless
round passes all **35 drives**, the book builds and
`scripts/book-examples.nu --check` passes.

**The windowed round**, `smoke -- --node`, three times over: **51 Rust
examples and the eleven Node examples, each on both bases, 120 frames
each, every one exiting 0** — 124 windows, eight at a time, in 28 to
32 s — and `counter`, `host`, `c_panel` and `lua_panel` by hand under
`KUI_SMOKE_FRAMES=120`, each exiting 0 with nothing on stderr: **128
windows over five hosts.** In each of the three runs one window of the
first eight took the whole round to draw its frames — `cells`, then
`audio`, then `accessibility`, each a second and a half when run
alone — which reads as a window covered by the seven launched over it
and not drawn until they had gone; not compared against alpha.36's
build. The AX audit: **106/106**, the audited window raised to the
front by its pid first, and no warning on the fixture's stderr.

**The bench guard** against the alpha.36 tag, on the release commit's
tree: **green**, none of the 8 guarded rows more than 10% slower —
every one between −0.9% and +2.8% (the worst guarded run-to-run spread
2.1%). It was not so at first, twice over, and both are fixed in this
tree:

- `frame_10k_segments` read **+7.7%** (863 → 929 µs, ±1.0%) on solid
  lines, and that was the dash (backlog C52): bisected to V2's commit
  by building the bench at each one. The `Stroke`, twenty bytes wider
  with its `Dash`, was copied at each call from `Ui::line` down to the
  line store, and the dash's cut ran its whole test on every line. The
  stroke is lent and a solid pattern is told by its zeroes; the row
  reads +2.5% in the guard and 875 → 880 µs with the two binaries
  alternated.
- `frame_10k_rects` read +9.3% and every row built on the bench's grid
  5 to 9% slower, and that was the bench: bisected to the commit that
  gave its grid the gradient rows' arms in one `match` per cell, with
  no line of the core between it and the commit before. The gradient
  cell is a function of its own now, the plain rows read as
  alpha.36's, and the gradient rows were taken again — 52 ns a node
  over a flat box where the first reading said 55
  ([ADR 0042](docs/adr/0042-a-gradient-is-an-image-the-core-paints.md),
  *Measured*; [docs/performance.md](docs/performance.md) carries the
  four rows from that run, the rest of its table kept as it was).

**This tag was cut twice.** The first, at `c7e9793`, published
nothing: both hosts' `check` failed on F121's own test, which asserted
that the platform's list draws `字` differently from the app's — and
on an image with DejaVu alone, both pipelines', the fixture is the one
face that has it, list or no list. Forgejo's `check` had been red on
it since F121 was merged, which this round did not read before
tagging; cargo stops at the first failing suite, so the suites after
`fonts` had not run there either. The test now tells which machine it
is on and asserts the platform's half only where there is one, and
`cargo test -p kui-core` passes its 105 suites in `rust:1-bookworm`
with `fonts-dejavu-core` alone (the whole workspace would not fit that
container). With no crate and no package carrying the version, the tag
was moved to the commit that has the fix and C52's; the mechanical
round, the windowed round and the guard above are of that commit.

## 0.1.0-alpha.36 (2026-10-05)

**What breaks.**

- Rust: `NodeKind::Path`, `HitShape::Path` and `NodeContent::Path` (under
  Added), so an exhaustive match on any of the three needs the arm.
- Node: `NodeInfo.kind` gains `'path'` and `WarningCode` gains
  `'path-malformed'` and `'path-too-large'`, so an exhaustive `switch`
  over either wants the cases.
- C: `kui_polyline` and `kui_polygon` with a NULL `spec` and a payload
  are hit, where the payload was dropped (RG109, under Fixed).
- A host that draws the list itself (C, or a renderer of your own):
  `KuiQuad.blur` / `Quad::blur` on a `GlyphMask` or `Texture` quad is
  the quad's turn in radians about its centre — 0 on every glyph and
  image, a path's `rotate` on a path's quads (under Added). A renderer
  that ignores it draws turned paths upright; nothing else changes.

No C build breaks: `KUI_ABI_VERSION` stays 23 — the C API gains three
functions and two enums and changes no struct or signature, which the
ABI rule does not bump for — and the Node wire moves to v20 for the one
new op, which an encoder and addon of one release never see apart.

### Added

- **`path` is a new element in every binding**
  ([ADR 0040](docs/adr/0040-a-path-is-a-mask-in-the-atlas.md)): any
  outline as SVG path data (`<path d="M … Z" bg width color fillRule/>`,
  `path { d = }`, `kui_path_d` / `kui_path` with `kui_path_parse`,
  `ui.path(&Path::parse(d)?, spec)` with `Path::sector` for a wedge, a
  donut's segment or a ring), filled with `bg` by `nonzero` or `evenodd`
  and stroked by `width` and `color` over the fill, placed like a `line`
  and hit by its outline under the rule. The core parses `d` once, for
  every binding; flattens the outline for the hit test; rasterizes it
  through zeno (swash's rasterizer, already in the tree) at the node's
  physical scale and quarter-pixel position into the glyph atlas, under
  its existing reset-and-copy policy, as a fourth kind of slot; and draws
  one `GlyphMask` quad per paint, so a host that draws text draws paths
  and nothing is re-rasterized for a hover, a colour tween or a slide. A
  fill bleeds half a pixel, so two paths sharing an edge meet without the
  background showing through — the hairline the `polygon` pie showed. A
  mask a quarter of the biggest atlas page or more, or a path whose ops
  change twice within a few frames, draws from a texture of its own
  (`QuadKind::Texture`, dropped through `dropped_textures` when no frame
  draws it), and one past 8192 px on a side draws nothing with
  `path-too-large`; a `d` that does not parse raises `path-malformed`
  with the byte. `HitShape::Path` carries the rule; `NodeKind::Path` in
  the devtools. A draw after `Z` starts where the subpath began, in the
  mask as in the hit outline; a stroke with no fill is hit by the pieces
  it paints and not along the chord of an open curve; and a `d` string
  handed over every frame is parsed when it changes, not every frame.
  A fill with no area — a ring's sector at a sweep of 0 — paints
  nothing; a path holding a NaN or an infinity (`1e99` in `d` is one)
  among its coordinates or in its turn draws nothing with
  `path-malformed`; and ops in the flat form whose code is not a whole
  0 to 5 are refused.
  The `path` corpus scene in four bindings; the `path`
  example; `frame_1k_paths_cached`, `frame_1k_paths_fresh`,
  `frame_1k_paths_animating` and `raster_pie_wedge_220px` benches.
- **`rotate` and `pivot` on a `path`**
  ([ADR 0041](docs/adr/0041-a-mask-turns-about-its-centre.md)):
  `<path d rotate={0.25} pivot={[x, y]}/>`, `path { d =, rotate =, pivot
  = {x, y} }`, `Path::rotated(turns)` and `Path::pivot(x, y)`, and two
  arguments on `kui_path` and `kui_path_d`. The turn is in turns,
  clockwise, about the pivot — the centre of the path's box without one
  — and it is the quad's, not the mask's: a path that only turns is
  boxed by the square the turn sweeps, rasterized once upright, kept in
  the atlas at every angle, and its `GlyphMask` (or `Texture`) quad
  carries the angle in `blur`, radians about the quad's centre, which a
  renderer turns the four corners by. Measured in a window, a turning
  arc drawn by changing its ops costs about 20 µs a frame — a raster, a
  texture and a draw of its own — and one drawn by `rotate` costs what a
  still one does; `frame_1k_paths_rotating` is the bench. A turned edge
  is resampled, so it softens by about a tenth of a pixel's coverage,
  and two fills that share an edge and turn together show a faint seam
  (0.94 where an upright seam is whole). `rotate` does not tween. Hit
  where it is drawn.
- **`examples/rust/apps/loaders.rs`**: ten loaders over one job — a pie, a
  ring and a bar that draw its progress, seven that draw only the time —
  with the clock in the model, a frame asked for only while the job
  runs, the round fills as `path`s that change every frame, and reduced
  motion honoured. Its arc turns by `rotate`: one atlas mask, turned
  by its quad. The job's button holds the keyboard, so Space starts it.
- **`polygon` is unchanged**: the eight-point fill whose SDF costs nothing per
  frame however it moves, for an arrowhead on a tweening connector or a
  strip under a live sparkline.

### Changed

- **The release workflow's npm step runs**: `release.yml` installs npm
  11 with `sudo`, where alpha.35's tag failed on `EACCES` under
  `/usr/local` after the crates were out and the stage was finished by
  hand; and `scripts/npm-approve.nu` asks nu for npm's global root, so
  its `latest` step runs on Windows. [docs/releasing.md](docs/releasing.md)
  has what the first two-host release found out, and pushes the tag to
  GitHub by name: the Forgejo mirror carried a tag there once without
  starting its pipeline.

### Fixed

- **An animating window no longer turns its run loop between frames**
  (macOS, backlog F103's held half). While anything animated — a
  transition, a spinner, a view asking for frames — the loop asked for
  the next frame on every turn, the pacer held it for the display, and
  the held frame was asked for again at once: a whole core for as long
  as it lasted, whatever was being drawn. The loop now waits for the
  display link it had already armed. An empty view asking for a frame
  every frame at 120 Hz went from 102–104% of a core to about 40%, the
  rest being the frame and the platform's own presenting.

- **C: `kui_polygon` and `kui_polyline` consume their payloads on every
  way out** (backlog RG109, as the new `kui_path` and `kui_path_d` do):
  a call that drew nothing — a NULL context, too few points — leaked
  the three `KuiValue`s it was handed, and one with a NULL `spec`
  dropped them unfreed. A NULL `spec` now keeps its handlers, so a
  stroke with no spec of its own takes a click.

**What you can delete.** The trigonometry that rebuilt a spinner's `d`
every frame to turn it; the half-pixel overlap or same-colour hairline
stroke a chart drew between adjacent wedges to hide the seam; the two
polygons a shape of more than eight points was split into; the flattening
an app did to draw an arc, a bezier or an icon's `d` as a polyline.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-05 — the
fifteen commits after the alpha.35 tag: the `path` element, `rotate` on
it, the loaders example, F103's held half and the release workflow's
fixes — with a regression pass over them first: three read-only
reviews (the core half of `path`, its bindings with the pacer, this
release's docs), each claim probed with a test before anything
changed. RG107–RG111 came of it and are in this release; RG112 holds
what was read and left. This round ran on the Mac alone; Windows and
Linux did not run it for this tag.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 26.10.0, nu 0.116.0**, on the release commit's tree,
the workspace's own artifacts pruned and rebuilt. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets -- -D warnings`
are clean. `scripts/test.nu`, the workspace's tests with the
conformance feature: **1764 tests over 133 suites, 0 failed** (4
ignored). The C round, `cbuild --run`, passes its five checks; the
corpus passes its **56 scenes** in four adapters, `path` the new one;
the ABI is **23**. Node's `node --test test.mjs` under
`KUI_CONFORMANCE_REQUIRED=1`: **207 of 207**. `npm run gen` leaves no
diff, the examples typecheck and their lockfile installs, the headless
round passes all **35 drives**, the book builds and
`scripts/book-examples.nu --check` passes.

**The windowed round**, `smoke -- --node`: **51 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 124 windows, eight at a time, in 32 s — and `counter`,
`host`, `c_panel` and `lua_panel` by hand under `KUI_SMOKE_FRAMES=120`,
each exiting 0 with nothing on stderr: **128 windows over five hosts.**
The AX audit: **106/106**, the audited window raised to the front by
its pid first. Before the prune the same round took a minute and
twice timed out its first eight windows straight after a relink, and
that was the directory, not the code: `target/debug/examples` held
47 796 files of older builds, a process's start walks the directory
its executable is in, and the same binary copied to an empty one
started as alpha.35's does (eight `counter`s side by side: 3.6 s
each from the crowded directory, 1.8 s from the empty one, 1.6 s for
alpha.35's).

**The bench guard** against the alpha.35 tag, on `004d49a`, before the
pass's fixes (which touch a path's gate and its first raster, and no
guarded row's code): **green**, none of the 8 guarded rows more than
10% slower — every one between −2.8% and +0.1% (the worst guarded
run-to-run spread 2.5%) — and every row both sides have reading
"same". The five `path` rows are new and in
[docs/performance.md](docs/performance.md) from this run; the rest of
its table is kept at alpha.22's numbers.

## 0.1.0-alpha.35 (2026-10-04)

**What breaks.** No build: the crates' code is alpha.34's — doc
comments, READMEs and manifests' `readme` / `repository` keys are what
moved — the ABI stays 23 and the Node wire is unchanged.

- npm, `npm publish` from `packages/kui`: the package's `publishConfig`
  names registry.npmjs.org where it named the Forgejo registry, so a
  bare publish from a checkout goes to npmjs (under Changed). Nothing an
  app that installs the package sees.

### Changed

- **A release is cut on two hosts.** The `v*` tag runs GitHub's
  `release.yml` — fmt, clippy and the workspace tests, then the Node
  addon built on each platform it ships for (linux-x64 and linux-arm64
  through cargo-zigbuild for the glibc 2.28 floor, darwin-arm64 and
  darwin-x64 on `macos-15`, win32-x64 on `windows-2025`), then the
  crates to crates.io and the npm package staged on npmjs — and
  Forgejo's `ci.yml`, which checks the same commit in full, waits for
  crates.io to list the version and publishes the crates to the
  `drydock9` cargo registry. This is the first release cut that way;
  [docs/releasing.md](docs/releasing.md) has the secrets and the order.
- **`@qxuken/kui` is on npmjs**, from this version: `npm install
  @qxuken/kui@alpha` with no registry configured. The runner's token is
  stage-only, so the version is staged and a maintainer with 2FA makes
  it live — `nu scripts/npm-approve.nu`, which also gives an alpha
  `latest` when it is the highest version the registry holds. The
  Forgejo npm registry still gets every version, by hand through
  `scripts/release-local.nu`. *What you can delete:* the
  `@qxuken:registry` line in an app's `.npmrc`, once the version it
  wants is on npmjs — 0.1.0-alpha.35 is the first there, and earlier
  ones stay on Forgejo only.
- **The repository is public** at
  [github.com/qxuken/kui](https://github.com/qxuken/kui), and the
  crates' and the npm package's `repository` names it.
- **The docs are written for a reader arriving from crates.io.** The
  root README is a front door — what kui is, the crates, install, the
  hello program, where to go next — and what it held moved unchanged
  into `docs/guide.md`, `docs/design.md`, `docs/performance.md`,
  `docs/releasing.md` and `docs/status.md`. Each of `kui-core`,
  `kui-native`, `kui-derive`, `kui-wgpu`, `kui-ffi`, `kui-lua` and
  `kui-node` has a README and a crate root that says what the crate is,
  who reaches it and through what, with compile-checked examples
  (`kui-core` alone runs 44 doctests where it ran none). The rendered
  docs and the book cite no ADRs or backlog items; each citation became
  the fact it supported. `docs/props.md` and `packages/kui/index.d.ts`
  are regenerated with the same text.

### Fixed

- **`kui-derive`'s docs name the crate the generated code reaches**:
  `::kui_native` by default, where they said `::kui`. The macro is
  unchanged.

### Native verification

On 2026-10-04, the commits after the alpha.34 tag: doc comments,
READMEs, the release workflows and their scripts. This round ran on
Windows alone, and short — the code is alpha.34's; macOS and Linux did
not run it for this tag, and the C round, the Node parity tests, the
corpus and the book are left to Forgejo's `check` on the tag.

**Windows 11 (build 26300) on a Ryzen 9 9950X3D with an RTX 5080,
rustc 1.99.0, nu 0.116.0**, on `7522bd4`, in a cold worktree. `cargo
fmt --all --check` and `cargo clippy --workspace --all-targets -- -D
warnings` are clean. `cargo test --workspace`: **1724 tests over 132
suites, 0 failed** (5 ignored). **The windowed round**, `smoke`: **49
Rust examples, each on both bases, 120 frames each, every one exiting
0** — 98 windows in 24 s. The Node windows, the C hosts, the AX audit
and the bench guard were not run.

## 0.1.0-alpha.34 (2026-10-03)

**What breaks.**

- Cargo, from the drydock9 registry: an app that depends on more than
  one `kui-*` crate with `registry = "drydock9"` does not build — its
  own `kui-core` and the one drydock9's `kui-native` resolves from
  crates.io are two crates, and their types do not meet (under
  Changed; corrected 2026-10-03, after kawoosh hit it). Take kui from
  crates.io: drop the key.
- macOS menus, the context menu and the bar: a row's accelerator that
  binds nothing — one kui does not parse, or one with no modifier — is
  drawn beside the row where it was drawn nowhere (F119, under Fixed).

### Changed

- **The crates are on crates.io**: `kui-derive`, `kui-core`, `kui-wgpu`,
  `kui-native`, `kui-lua` and `kui-ffi`, from 0.1.0-alpha.33 (published
  from this release's branch on 2026-10-03, the tag's code with only the
  manifests changed), so a Rust project depends on
  `kui-native = "0.1.0-alpha.34"` with no `[registries]` table and
  `cargo add kui-native` writes it: the bake-offs' first
  recommendation, taken at an alpha rather than at the beta the backlog
  had held it for. A tag publishes to
  crates.io first and then, as before, to the Forgejo registry
  (`drydock9`), through `scripts/publish-crates.nu`, which skips a crate a
  registry already holds at the version, so a release cut short by
  crates.io's rate limit on new crates, or anything else, is finished by
  running it again; the release workflow needs a `CRATES_IO_TOKEN`
  secret beside `PACKAGES_TOKEN`. The workspace's `kui-*` dependencies no
  longer name `drydock9` (crates.io refuses a dependency from another
  registry, and one manifest cannot name two), so the copies on Forgejo
  resolve their own `kui-*` dependencies from crates.io too. A project
  that keeps `registry = "drydock9"` builds only while it names one
  `kui-*` crate: name two — `kui-native` and `kui-core`, as an app that
  uses the core's types does — and the graph holds a `kui_core` from
  each registry, the same version and checksum but two crates to
  cargo, and every type passed between them is a mismatch
  (`expected kui_native::Cell, found kui_core::cells::Cell`, from
  kawoosh). Dropping the key is the move, and for such an app the
  only one; this paragraph said "builds as before" when the release
  was cut, and is corrected after it. The crates' `homepage` is the
  book, the one public link — the repository stays private. npm is
  unchanged.

### Fixed

- **A macOS menu row draws the keys it does not bind** (F119, from
  kawoosh's menus seen on a Mac): an accelerator AppKit cannot take as a
  key equivalent — `"gd"`, an app's own multi-key hint, or a bare
  `"space"` — is drawn in the row's title, right-aligned in a column
  the menu shares, in the secondary label colour, as ADR 0018 decision
  7 always said and the drawn menu on Windows and Linux always did; the
  `NSMenu` adapter dropped it. *What you can delete:* a hint folded into
  a row's label (`"Go to Definition (gd)"`) for the Mac's sake.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-03 — the
commits after the alpha.33 tag: the crates published to crates.io
(manifests and the release workflow only) and F119 (a macOS menu row
drawing the keys it does not bind), merged from their branches. This
round ran on the Mac alone; F119 is the macOS `NSMenu` adapter's and
was seen there in kawoosh's editor menu, against its branch by path,
before it merged. Windows and Linux did not run the round for this tag.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 25.6.0, nu 0.116.0**, on `679ee76` with the version set,
in a cold worktree. `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets --features kui-core/conformance -- -D
warnings` are clean. `cargo test --workspace --features
kui-core/conformance`: **1667 tests over 131 suites, 0 failed** (3
ignored). The C round, `cbuild --run`, passes its five checks; the
corpus passes its **55 scenes** in four adapters; the ABI is **23**.
Node's `node --test test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`:
**207 of 207**. `npm run gen` leaves no diff, the examples typecheck
and their lockfile installs, the headless round passes all **33
drives**, and `scripts/book-examples.nu --check` passes.

**The windowed round**, `smoke -- --node`: **49 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 120 windows, eight at a time, in 35 s — and `counter`,
`host`, `c_panel` and `lua_panel` by hand under `KUI_SMOKE_FRAMES=120`,
each exiting 0 with nothing on stderr: **124 windows over five hosts.**
The AX audit: **106/106**, the audited window raised to the front by
its pid first.

**The bench guard** against the alpha.33 tag, on `679ee76`: **green**,
none of the 8 guarded rows more than 10% slower — the worst
`frame_10k_rects_with_text_and_hits` at +0.4% (the worst guarded
run-to-run spread 1.1%) — and every one of the 38 rows' sources
"same". README's table is kept at alpha.22's numbers.

## 0.1.0-alpha.33 (2026-10-02)

**What breaks.** No build.

- Input, every binding: an `onScroll` node that also scrolls an axis as
  a container (`scrollX` / `scrollY`) no longer takes a gesture that
  starts with it at its limit on that axis the way the gesture goes;
  the scroller around it does (F118, under Fixed). A view that wanted
  every such gesture says `overscroll: "contain"`.
- Layout, every binding: a fit box across a column is no wider than the
  column's box (F116, under Fixed). One that ran past a parent that does
  not scroll x — a wrapper in a card with a `maxWidth`, anything wider
  than the window — is held to it, and what is in it wraps, gives or is
  cut to its ellipsis. A layout that meant the overflow declares
  `minWidth: "fit"`.

### Fixed

- **The text in a card with a `maxWidth` wraps at the card, however deep
  it sits** (F116, from kawoosh's fit-content review): across a column,
  a fit child is its content's width but no wider than the column's
  content box — CSS's `fit-content` — where it was its content's width
  whatever room there was. A 400 px text straight in a column capped at
  200 wrapped to it; the same text in a fit row or column inside that
  card was 400 wide on one line, drawn 200 past it, since only a text
  straight under a box was held to it. Held in the width pass, top down,
  the child's own row then gives along it and its text wraps, however
  many wrappers deep. A fit child that fits keeps its width — nothing
  stretches — and it gives down to its declared `minWidth`, 0 when none
  is declared, so a title with `ellipsis` is cut to the card (a CSS
  `min-width: auto` floor would have kept it whole). A column that
  scrolls x, a fixed child, a width a ratio derives, a `cells` grid and
  an image keep their width, and heights are not held. A table in such a card lays its columns
  across the card, the fit columns compressed into it. ADR 0033,
  decision 10, as amended; the corpus's `fit-across` scene in all four
  bindings. *What you can delete:* a `width="grow"` (`grow_width()`)
  on a wrapper, written so the text in it would wrap at a capped parent.

- **A wheel handler at its edge passes the swipe on** (F118, from
  kawoosh's sticky-swipe report): an `onScroll` node that scrolls an
  axis as a container too — a code view or a table whose sideways
  offset the app sets from what it hears — is answered on that axis by
  its room, as any container is. At its right edge a swipe further
  right moves the scroller around it, and back left is still its event;
  a node that is only a handler takes its axes as before, the core
  having nothing to ask. ADR 0038, decision 3, as amended; the corpus's
  `scroll-handler-room` scene in all four bindings. *What you can
  delete:* a `scrollAxes` an app flipped each frame by its own offset
  so a swipe at the edge would reach the scroller around.
- **A swipe begun while the last one still glides goes where it is
  aimed** (F117, from kawoosh's sticky-swipe report): kui-native begins
  a scroll gesture when a finger comes down, not only after a 200 ms
  pause, so a swipe started over another scroller mid-glide latches that
  scroller and chooses its own axis, where it went on moving the
  glide's target. A glide's start and a finger's are both winit's
  `Started`; one within 100 ms of a finger's lift is the glide's. And a
  slow turn within one swipe turns the axis lock: it asks for 10 px of
  recent travel, not 24 (2.5 px an event, not 6). ADR 0038, decision
  2, as amended. *What you can delete:* nothing an app could have
  written; the runner's gestures were not the app's to draw.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-02 — the
commits after the alpha.32 tag: F116 (a fit box across a column held to
the column's box), F117 (a finger put down begins a scroll gesture of
its own) and F118 (a wheel handler at its edge passes a new touch on),
merged from their branches. This round ran on the Mac alone. F116 was
built and tested on Windows on its branch (1662 tests over 132 suites,
Node 205/207, the C round and 54 scenes); F117 reads the trackpad
phases macOS reports, and F118 is the core's, the same on every
platform. Windows and Linux did not run the round for this tag.

**macOS 27.0.1 on an M3 Pro MacBook Pro, rustc 1.99.0 (the toolchain
CI runs), Node 25.6.0, nu 0.116.0**, on `c1109f9` with the version set,
in a cold worktree. `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets --features kui-core/conformance -- -D
warnings` are clean. `cargo test --workspace --features
kui-core/conformance`: **1665 tests over 131 suites, 0 failed** (3
ignored). The C round, `cbuild --run`, passes its five checks; the
corpus passes its **55 scenes** in four adapters; the ABI is **23**.
Node's `node --test test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`:
**207 of 207**. `npm run gen` leaves no diff, the examples typecheck
and their lockfile installs, the headless round passes all **33
drives**, and `scripts/book-examples.nu --check` passes.

**The windowed round**, `smoke -- --node`: **49 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 120 windows, eight at a time, in 33 s — and `counter`,
`host`, `c_panel` and `lua_panel` by hand under `KUI_SMOKE_FRAMES=120`,
each exiting 0 with nothing on stderr: **124 windows over five hosts.**
The AX audit: **106/106**, the audited window raised to the front by
its pid first.

**The bench guard** against the alpha.32 tag, on `c1109f9`: **green**,
none of the 8 guarded rows more than 10% slower — the worst
`frame_10k_rects` at +2.3% (the worst guarded run-to-run spread
±2.6%) — and every one of the 38 rows' sources "same". README's table
is kept at alpha.22's numbers.

## 0.1.0-alpha.32 (2026-10-02)

**What breaks.** No build.

- A `Session` made after the first in a process starts from the fonts the
  first one found: a font installed on the system while the process runs
  is seen by none of them until something calls `reload_system_fonts`
  (under Added), where a later session used to rescan and see it. Fonts
  an app loads itself are unchanged.
- The repository's scripts are Nushell (`scripts/*.nu`), each where its
  `.sh` was; `nu` 0.116 runs them on macOS, Linux and Windows. CI
  installs it. Nothing a consumer of the crates or the package touches.

### Changed

- **The system's fonts are scanned once a process.** Every
  `Session::new` (so every `Core::new`, and every Node `Ctx`) opened
  every font file on the system twice — fontdb's scan, then the check
  that drops faces the shaper cannot measure (F98) — 20–70 ms on a Mac's
  1312 faces, most of it `open`. The first session does that; the rest
  copy its checked database. Test suites that make a core a test are
  where it showed: `cargo test --workspace` went from ~100 s to ~33 s on
  an M3 Pro, `npm test` from 13 s to 8 s, and one coverage test that
  builds 54 cores from 10 s to 2 s (it now also runs its cases on
  threads). *What you can delete:* a `Session` kept alive only so later
  windows would not pay for the scan.

### Added

- **`Core::reload_system_fonts`** — C `kui_font_reload_system`, Node
  `ctx.reloadSystemFonts()` / `win.reloadSystemFonts()`; Lua has none, a
  script being a guest: scans the system's fonts again and brings the
  session's font database up to it, a face installed since joining and
  one uninstalled leaving, and returns how many faces came and went.
  Faces still installed keep their handles and their place in every
  window's caches, fonts the app loaded itself are not touched, every
  window of the session shapes its text again on its next frame (and
  the calling one is asked for it), and sessions made afterwards start
  from the new scan. The winit runner calls it itself when the OS says
  the installed fonts changed — CoreText's
  `kCTFontManagerRegisteredFontsChangedNotification` on macOS (checked:
  a font copied into `~/Library/Fonts` and removed again reached a
  running example as one face each way), `WM_FONTCHANGE` on Windows (checked:
  Inter installed per user and removed again reached a running Node
  window as one `fonts` event each way, its text reshaped in Inter and
  back) — coalescing a burst into one rescan, and every window
  draws; so every Rust, C `kui_run` and Node window app follows an
  install with no code. Each window's next frame then raises a `fonts`
  event (`{ kind: "fonts" }`, Node `FontsMsg` in `CoreMsg`, on the root),
  for an app that keeps `systemFonts()` in its model — kawoosh's fonts
  pane — to read it again; a font the app loads itself raises none. Linux's fontconfig has no such signal, and a host
  with its own windowing has no runner: those call it when the set may
  have changed — a fonts pane opening, the window taking focus back — not
  every frame, since it opens every font file on the system. A new
  function, so the ABI stays 23. *What you can delete:* a restart, or a
  `Session` made anew, to pick up a font the user just installed.
- **`KUI_WINDOW_AT=X,Y`**, beside `KUI_WINDOW=WxH`: where the runner opens
  the main window, logical px from the screen's top left.
- **`smoke --jobs N`.** The smoke round runs in parallel: eight windows
  at once by default, cascaded through `KUI_WINDOW_AT` so none is wholly
  covered (a covered window is `Occluded` on macOS and presents nothing),
  and every headless drive at once, started as built binaries rather
  than a `cargo run` each. On an M3 Pro the windowed round went from
  143 s to 24 s and the headless one from 44 s to 6 s. `--jobs 1` is the
  old round. On a Windows desktop (RTX 5080) eight at a time is clean,
  the 120 windows in 31 s; CI's Windows runner keeps `--jobs 1` until it
  is run there.
- **`scripts/test.nu`**, the local test run: what `cargo test --workspace
  --features kui-core/conformance` runs, with the test binaries started
  side by side, slowest first, instead of one after another — 11–16 s
  where `cargo test` takes ~33 s. On Windows ~6 s. `--node` adds `npm test`, `-p` takes a
  comma-separated list, a filter narrows it and libtest's own flags go
  after `--`. nextest was measured and is slower here: a process per
  test pays the font scan per test.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-02 — the
commits after the alpha.31 tag: the system's fonts scanned once a
process and again when the OS says they changed, with a `fonts` event,
`KUI_WINDOW_AT`, `smoke --jobs` and the scripts in Nushell. This round
ran on Windows and on Linux under WSL, not on the Mac, where the fonts
work and alpha.31 were checked: what was owed was their Windows halves,
`WM_FONTCHANGE` and F115's `ToUnicodeEx`, and the Linux `check` that
took the last tag. The AX audit (a Mac's) was not run. It found two
things, fixed before the tag: the rescan test failed on Linux, where
fontdb reads Ubuntu's variable `Ubuntu[wdth,wght].ttf` as two faces at
one index and the test uninstalled one of them — the rescan, keyed on
the file, was right — and `scripts/test.nu` lost the toolchain's
libraries on Windows, nu spelling the variable `PATH`.

**Windows 11 Pro 26300 on an RTX 5080, rustc 1.99.0, Node 25.2.1, nu
0.115.0**, on the round's head in its worktree. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1652 tests over 131 suites, 0
failed** (4 ignored); `scripts/test.nu` counts the same in ~6 s. The C
round, `cbuild --run`, passes its six checks; the corpus passes its
**53 scenes**; the ABI is **23**. Node's `node --test test.mjs` under
`KUI_CONFORMANCE_REQUIRED=1`: **206 of 207**, the RTLD test skipped.
`npm run gen` leaves no diff, the examples typecheck and their lockfile
installs, the headless round passes all **33 drives**, and
`scripts/book-examples.nu --check` passes — on this CRLF checkout too,
where the bash one reported every page stale.

**Ubuntu 24.04 under WSL 2, rustc 1.99.0, Node 25.2.1**, on a clean
clone with the round's fixes: clippy as above clean; **1648 tests over
130 suites, 0 failed** (3 ignored); the C round passes its five checks
and the corpus its 53 scenes; Node **207 of 207**; `gen` no diff; the
examples typecheck; the headless round passes.

**The windowed round**, `smoke -- --node`: **49 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 120 windows, eight at a time, in 31 s — and `counter`,
`host`, `c_panel` and `lua_panel` by hand under `KUI_SMOKE_FRAMES=120`,
each exiting 0 with nothing on stderr: **124 windows over five hosts.**

**The Windows halves, in a window.** `WM_FONTCHANGE`: a Node window
naming `Inter`, absent, while Inter-Regular was installed for the user
(the file in `%LOCALAPPDATA%\Microsoft\Windows\Fonts`, its HKCU value,
`AddFontResource` and the broadcast) and removed again — one `fonts`
event each way, `systemFonts()` 80 → 81 → 80, the text reshaped in
Inter and back. F115: an `onKey` sink under Windows' Russian layout,
pressed by `keybd_event` — the key printed `` ` `` reads `` ` `` with
`text` "ё", ⇧4 `$` with ";", ⇧/ `?` with ",", ; and Q `;` and `q` with
"ж" and "й", 1 `1`; under US the same keys read as printed.

**The bench guard** against the alpha.31 tag, on Windows: **green**,
none of the 8 guarded rows slower beyond its noise — the worst
`frame_1k_curves` and `frame_1k_typical` at +0.2% — and every one of
the 38 rows' sources "same". README's table is kept at alpha.22's
numbers.

## 0.1.0-alpha.31 (2026-10-01)

**What breaks.** No build: `LayoutScript` and
`KeyPress::from_layout_in` are new, `from_layout` is unchanged, and
`KUI_KLAYOUT_NONLATIN` is a bit that was zero, so the ABI stays 23.

- The winit runner, on a layout that writes no Latin: a key whose
  layout character is ASCII but not the key US-QWERTY prints there now
  reports that US key as `code` (F115, under Fixed) — macOS Russian's
  `` ` `` key is `` ` `` where it was `]`, its ⇧5 `%` where it was `:`;
  Windows Russian's `/` key is `/` where it was `.`. `text` is the
  layout's as before; a keymap that matched the layout's ASCII there
  reads it.

### Fixed

- **A non-Latin layout's punctuation is the US key** (F115, from
  kawoosh's Russian-backtick report): on macOS's Russian layout the key
  printed `` ` `` types `]`, and a press was `]` — the layout fallback
  judged each key by itself and let the layout's ASCII win, which on a
  layout that writes no Latin is incidental, so a vim hand's `` ` ``,
  `$` (⇧4, the layout's `%`) and `%` (⇧5, its `:`) went to the wrong
  commands. The driver now says which alphabet the layout writes
  (`LayoutScript::{Latin, NonLatin}`, `KeyPress::from_layout_in`), and
  on a non-Latin one every key's `code` is the US-QWERTY key at
  `physical`, as Shift prints it — macOS's own rule for a ⌘ shortcut. A
  key that is already its position's character, or sits where US-QWERTY
  has no key, keeps the layout's; `text` is the layout's either way.
  The winit runner asks macOS whether the keyboard layout is
  ASCII-capable and Windows what its letter keys type, and elsewhere
  goes by what the letter keys last typed. A host with its own
  windowing says so with `KUI_KLAYOUT_NONLATIN` in C's kmods or
  `layout: "nonLatin"` in Node's key mods; without it, nothing changes.
  ADR 0002, decision 11, as amended. *What you can delete:* a table
  mapping a non-Latin layout's punctuation back to the US keys, written
  so a keymap's `` ` `` or `@` worked there.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-10-01 — the
commits after the alpha.30 tag: F115 (a non-Latin layout's punctuation
read as the US key), merged from its branch. This round ran on the Mac
alone; F115's Windows half (`ToUnicodeEx` over A–Z) was built and
clippy-clean for `x86_64-pc-windows-msvc` on its branch, and is not yet
run on Windows.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.1, Node 25.6.0**, on
`5438039` with the version set, in a cold worktree. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean — and clippy again on
rustc 1.99.0, which CI runs: its `needless_borrows_for_generic_args`
took the first tag's check (`text.rs`'s `.map(&class)`), fixed on main
and the tag moved before anything was published. `cargo test --workspace
--features kui-core/conformance`: **1647 tests over 130 suites, 0
failed** (3 ignored). The C round, `cbuild --run`, passes its five
checks; the corpus passes its **53 scenes** in four adapters; the ABI
is **23**. Node's `node --test test.mjs` under
`KUI_CONFORMANCE_REQUIRED=1`: **206 of 206**. `npm run gen` leaves no
diff, the examples typecheck and their lockfile installs, the headless
round passes all **33 drives**, and `scripts/book-examples.sh --check`
passes.

**The windowed round**, `smoke -- --node`: **49 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 120 windows — and `counter`, `host`, `c_panel` and
`lua_panel` by hand under `KUI_SMOKE_FRAMES=120`, each exiting 0 with
nothing on stderr: **124 windows over five hosts.** The AX audit:
**106/106**, the audited window raised to the front by its pid first.

**The bench guard** against the alpha.30 tag, on `5438039`: **green**,
no guarded row more than 10% slower — the worst `deep_nesting_64_levels`
at +2.0% inside its own ±2.6% spread — and every one of the 38 rows'
sources "same". F115 touches no frame: it is a key's reading in the
runner, the macOS layout asked once a press. README's table is kept at
alpha.22's numbers.

## 0.1.0-alpha.30 (2026-09-30)

**What breaks.** The ABI is 23: `KuiSpec` gains `bounce`. The Node wire
stays v19 — `bounce` is a schema row and `smooth` / `snappy` two more
names of one. Rust gains two `Easing` variants and a `Transition` field.

- C: `KuiSpec.bounce` appended (under Added); the 64-bit size is 688.
  Recompile — a zeroed field is the easing's own bounce, which is what
  every spring had. `KUI_EASE_SMOOTH` and `KUI_EASE_SNAPPY` are new
  values of `easing`.
- Rust: `Easing::Smooth` and `Easing::Snappy`, so an exhaustive match on
  `Easing` needs the arms.
- Rust: `Transition::bounce`, an `Option<Bounce>`, so a `Transition`
  built field by field names it (`Transition::ms` sets it to `None`).
- On Windows, `env.system.appearance` follows the OS's light/dark switch
  while the app runs (RG106), where it kept the base the window opened
  with; a `system` event reports it.
- Layout, every binding: a column of fit children too short for them
  no longer squeezes a child below its content (F114, under Fixed). A
  row that was squeezed now keeps the height of its text and the column
  overflows — clipped by whatever clips it, scrolled by a scroller. A
  layout that meant the squeeze declares `minHeight: 0`.

### Added

- **A spring takes a bounce, not physics.** A spring is two numbers a
  person tunes by eye: how long it takes to get there — the duration it
  always had — and how far it overshoots, a `bounce` from 0 (glides in,
  no overshoot) to 0.9 (rings a while; a spring at 1 would never
  settle, so more is held there). A bounce `b` is a damping ratio of
  `1 - b`, SwiftUI's `Spring(duration:bounce:)`; nothing asks for a
  stiffness or a damping. The spring easings are named bounces —
  `smooth` 0 and `snappy` 0.15 are new, `spring` 0.25 and `bouncy`
  0.5 are the ratios they had — and the `bounce` prop (`bounce` in Lua,
  `NodeSpec::bounce` / `Transition::bounce` in Rust, `KuiSpec.bounce`
  in C) replaces an easing's own. On a timed easing it makes the
  transition a spring, since a bounce is only a spring's to have:
  `transition={300} bounce={0.3}` is a 300 ms spring with that bounce.
  `Bounce` rides in the two bytes of padding `Transition` had spare, so
  `NodeSpec` does not grow. A new example, `spring`, puts a spring's
  duration and bounce on two sliders, with a button for each named
  bounce, and springs a bar and a racer across a stage with them; the
  `transition` example races all four springs and one with a bounce of
  its own, splitmux's tabs land on `snappy` and `enter_exit`'s side
  panel on `smooth`, which keeps it on the window's edge. *What you can
  delete:* nothing — a spring had two bounces before this and no way to
  ask for a third.

### Fixed

- **A Windows app follows the OS from light to dark and back** (RG106):
  winit asks uxtheme's `ShouldAppsUseDarkMode` on each setting change,
  and that answers from a policy cached per process until
  `RefreshImmersiveColorPolicyState` is called, which nothing called —
  so the switch in Settings reached winit as the answer it already had,
  no `ThemeChanged` went out, and a window kept its base until the app
  was started again (focus coming back re-read winit's record, the same
  stale answer). Each window is subclassed to refresh the policy on
  `ImmersiveColorSet` before winit's own handler runs, which then sends
  `ThemeChanged` and sets the window's dark mode itself. *What you can
  delete:* a restart, or a registry read of `AppsUseLightTheme`, to pick
  up the switch.
- **A row keeps the height of its text in a column too short for it**
  (F114, from kawoosh's overflow hunt): down a column of fit children
  that overflows, an undeclared floor is the child's min-content — CSS's
  `min-height: auto` — where it was 0 and the children gave largest
  first toward it. Nothing in a box gives vertically but a scroller or
  a clip, so a squeezed row only ever drew its text over the rows under
  it: kawoosh's settings and fonts panes in a window 260 px tall put
  their headers' chips over the search field. A row that wraps keeps
  its lines stacked — its min-content down is each line's tallest and
  the cross gaps, as its fit height is. A child that scrolls or
  clips still gives to 0, so a list under a dialog's title takes what
  the title leaves it, and `min_content` now takes a clip for a box that
  needs nothing of what it holds on either axis, as CSS's automatic
  minimum does for overflow that is not visible. A `grow` child still
  gives first, a declared `minHeight` wins, and across nothing changed:
  a row of fit labels still squeezes them into their ellipses. ADR 0033,
  decision 10, as amended; the corpus's `column-squeeze` scene in all
  four bindings. *What you can delete:* a `minHeight: 'fit'` on a
  column's rows, written to stop them squeezing under their text.


### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-30 — the
commits after the alpha.29 tag: Cargo.lock's semver-compatible updates,
a spring's bounce, RG106 (Windows' light/dark switch reaching a running
app) and F114 (a column's rows kept at their text's height), each merged
from its branch. This round ran on the Mac alone; RG106 is Windows' and
was checked there when it was built, in the `theme` example on Windows
11, and is not re-run here.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.1, Node 25.6.0**, on
`f6d6259` with the version set, in a cold worktree. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1643 tests over 130 suites, 0
failed** (3 ignored). The C round, `cbuild --run`, passes its five
checks; the corpus passes its **53 scenes** in four adapters,
`column-squeeze` new; the ABI is **23**. Node's `node --test test.mjs`
under `KUI_CONFORMANCE_REQUIRED=1`: **205 of 205**. `npm run gen` leaves
no diff, the examples typecheck and their lockfile installs, the
headless round passes all **33 drives**, and `scripts/book-examples.sh
--check` passes.

**The windowed round**, `smoke -- --node`: **49 Rust examples — `spring`
new — and the eleven Node examples, each on both bases, 120 frames
each, every one exiting 0** — 120 windows — and `counter`, `host`,
`c_panel` and `lua_panel` by hand under `KUI_SMOKE_FRAMES=120`, each
exiting 0 with nothing on stderr: **124 windows over five hosts.** The
AX audit: **106/106**, the audited window raised to the front by its
pid first.

**The bench guard** against the alpha.29 tag, on `f6d6259`: **green**,
no guarded row more than 10% slower — the worst `deep_nesting_64_levels`
at +6.7% inside its own ±5.1% spread — and every one of the 38 rows'
sources "same". What F114 costs where a column overflows without
scrolling, isolated against `d279ac9`, the merge before it: the frame
benches' grids are 100 rows taller than their 1080 px viewport, so
every row is now measured for its min-content where it was squeezed —
`frame_10k_chips_unwrapped` 670 → 693 µs (+3.4%), `frame_10k_rects`
767 → 777 µs (+1.3%), `frame_1k_typical` level — and those rows now
keep their 14 px cells where they were squeezed under them. A column
that fits, or scrolls, measures nothing. `sizes/squeeze_1000_rows_down`,
new, reads a thousand labelled rows overflowing a column at 320.7 µs
against 319.3 with the squeeze. README's table is kept at alpha.22's
numbers.

## 0.1.0-alpha.29 (2026-09-30)

**What breaks.** No build: `Core::holds_key` is new, the ABI stays 22
and the Node wire v19. Five readings change on Windows and Linux, each
a fix:

- A key held as a window gains focus is not pressed there (RG100): on
  Windows and X11 winit hands the window a press of every key already
  down, and kui delivered them. A sink hears no `down` for a key pressed
  in another window or app; the `up` still arrives.
- On Windows, the second Shift pressed while the first is held is a
  first press (`repeat: false`), and letting go of both delivers a
  release for each, the one Windows never sent first (RG102).
- On X11, `caps_lock` and `num_lock` are the X server's (RG104): a lock
  set before the app started, or turned in another app, reads as it is.
  Wayland still tracks the lock keys' own presses.
- On X11, the left Alt pressed after Shift says `alt: true` on its own
  press, and AltGr says `location: "right"` (RG101).
- A key pressed in a window and let go of while a popup borrows its
  keyboard is released in that window at once (RG103), where it came
  up when the window next lost focus.

### Fixed

- **A key that closed a window is not pressed again in the one
  beneath** (RG100): winit makes up a press of every held key as a
  window gains focus on Windows and X11, so F3 answered by closing a
  window pressed F3 in the window that took focus, and opened it again.
  The made-up presses are dropped; the made-up releases as a window
  loses focus are kept, since they let go of what a sink held.
- **Two Shifts on Windows are two presses and two releases** (RG102):
  Windows keeps one "was down" bit for both Shifts, so the second said
  it was a repeat, and sends no release for the first let go while the
  other is held. The record kept that Shift held, and the next Shift's
  release said `shift: true`. A modifier's repeat is now one only while
  its key is held, and a Shift's release lets go of its twin first, as
  GLFW reads it.
- **An X11 modifier the layout leaves unnamed is the key it is**
  (RG101): ⇧ then the left Alt reads `Meta_L`, which winit names
  nothing, so its press carried the state before it; AltGr
  (`ISO_Level3_Shift`) has no side in its keysym and said `standard`.
  Both now take where the key is.
- **A release goes where its press went** (RG103): the owner of a
  popup held a key pressed before the popup opened until it lost focus,
  and let go of it then, after keys pressed later. `Core::holds_key`
  says which core delivered a press.
- **X11 reads the lock state off the server** (RG104), on a connection
  of its own, as macOS and Windows ask theirs: tracking the lock keys
  knew nothing of a lock turned while another app had the keyboard, and
  under XWayland it disagreed with the text the server typed after a
  focus change.

### Docs

- **The kui book is published** at https://kui-book.qxuken.dev, built
  on a `v*` tag by the `book` workflow. It carries an Examples part — a
  page for each of the 39 Rust examples outside `tutorial/`, each
  including its source when the book is built
  (`scripts/book-examples.sh`, checked in CI) — and no link leaves it
  for the repository. Its setup page tells a reader to copy a program
  into their own project, and its dependency line names the release
  (`scripts/set-version.sh` writes it; `check-version.sh` refuses a tag
  whose book names another).

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-30 — the
commits after the alpha.28 tag: the book's examples part and its
publishing, and RG99's window round with what it found (RG100–RG104
built, RG105 withdrawn), merged from its branch. This round ran on
Windows and on Linux under WSLg, not on the Mac: what the release
changes is the key model's Windows and X11 halves, and RG103, the one
change that runs on macOS too, is pinned by a core test. The AX audit
(a Mac's) and the four hosts by hand were not run.

**Windows 11 Pro 26200 on an RTX 5080, rustc 1.98.1, Node 25.2.1**, on
`0530ef3`, the branch's head before the version was set, in its
worktree (not cold). `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets --features kui-core/conformance -- -D
warnings` are clean. `cargo test --workspace --features
kui-core/conformance`: **1634 tests over 130 suites, 0 failed** (4
ignored). The C round, `cbuild --run`, passes its six checks. Node's
`node --test test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`: **204 of
205**, the RTLD test skipped (win32 has no dlopen flags). `npm run gen`
leaves no diff but line endings, the examples typecheck and their
lockfile installs, and the headless round passes all **32 drives** once
the C round has built `panel.dll` (before it, `c_panel` says so and
fails). `scripts/book-examples.sh --check` reports every page stale on
this CRLF checkout, by its line endings alone; it passes on Linux.

**Ubuntu 24.04 under WSLg (X11 on llvmpipe), rustc 1.96.1, Node
25.2.1**, on a clean clone of `0530ef3`: clippy as above clean; **1632
tests over 129 suites, 0 failed** (3 ignored); the C round passes its
five checks and the no-ABI panel is refused as "this build is 22";
Node **205 of 205**; `gen` no diff; the book's examples check passes;
the headless round passes all **32 drives**.

**The windowed round**, `smoke -- --node`: **48 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — **118 windows on Windows and 118 on X11**. RG99's probe
checked the release's own changes in a window on both (RG100–RG104).

**The bench guard** against the alpha.28 tag, on Windows at `0530ef3`:
**green**, none of the 8 guarded rows more than 10% slower — the worst
`frame_10k_rects_with_access_tree` at +7.9% inside its own ±8.4%
run-to-run spread — and every one of the 38 rows' sources "same"; the
release touches no frame path. README's table is kept at alpha.22's
numbers.

## 0.1.0-alpha.28 (2026-09-30)

**What breaks.** No build: `Ctx.beginFrameCause()`,
`Core::begin_frame_cause`, `Min::AUTO`, `TextMeasure::min_content` (with
a default), `KUI_MIN_NONE`, `calc::FULL`, `calc::is_full`,
`calc::refused` and the `size-expressions-full` warning code (a new
member of Node's `WarningCode`) are new, the ABI stays 22 and the Node
wire v19. One layout rule changes:

- A row or column holding a share of the room — a percentage or a size
  expression — gives as CSS's flex items do when it overflows (under
  Changed, RG92): its children give in proportion to their sizes where
  the largest gave first, and stop at their content where they went to
  0. Such a row can draw its children at other widths and overflow
  where it squeezed; `minWidth: 0` on a child is the old floor. A row
  of fit children alone is as it was.
- Rust: `Min::default()` and `LayoutSpec`'s default min are `Min::AUTO`
  (negative zero, `==` to `Min::px(0.0)`); a floor of 0 declared is
  `Min::px(0.0)`. C: a `KuiSpec.min_w` of 0 stays undeclared, and
  `KUI_MIN_NONE` declares 0.

Eight readings change, each a fix:

- A negative px `maxWidth` / `maxHeight` is a ceiling of 0 again, in
  every binding (under Fixed, RG78): since alpha.25 it could read as
  another view's size expression.
- A size expression nested past 32, or holding a `NaN` or an infinity,
  is refused — "bad size: nested past 32", "bad size: NaN is not a
  finite number" — where it was taken, and a nesting deep enough
  aborted the process (RG79, RG80). `-0` is `0`.
- A table cell's size-expression clamps resolve against the columns'
  room, as its width does, and a column is its largest expression, not
  its first (RG90, RG91): such a table can lay out differently.
- A size spelling reads as CSS reads it (RG94): a space between a
  number and its unit or a function and its parenthesis is refused
  (`"80 %"`, `"100 px"`, `"min (1, 2)"`, taken before), and names and
  `px` are taken in any case (`"MIN(10PX, 50%)"`, refused before).
  Node's encoder refuses `"50px%"` (it sent 50%), `"1.2.3%"` (1.2%) and
  a table naming two functions (`{ min, max }`, which it read as the
  `min`), and a keyframe or entrance stop's `{ percent: 50 }` is 50%,
  where it was 5000%.
- Caps Lock's own press says `caps_lock: true` as it turns the lock on,
  on Linux too (RG96): it said what it found there, where macOS and
  Windows said what it made. `Accel::parse` refuses a lock key
  (`"ctrl+capslock"`), which it read since alpha.24 named them.
- An animation — a transition, a keyframe cycle, a node declaring
  `animate` — asks for no frames while its window is covered or
  minimized, on macOS and X11 as on Windows since RG45 (RG98). A view
  that did work once a frame through `animate` does none until the
  window shows again; a waker or the host still gets its frames.
- A modifier key let go of because the window lost the keyboard says
  its own bit is off (`shift: false` for Shift), as a real release does
  (RG85).
- `owed_by().requests` names a `reveal`, `set_scroll`, `focus_region`
  or `request_files` by its door (`why: "reveal"`) at the app's line,
  not `"request_frame"` at kui's (RG82).

### Added

- **The kui book, and the tutorial it reads** (backlog DX27,
  [ADR 0039](docs/adr/0039-a-tutorial-is-a-sequence.md)). `docs/book`
  is an mdBook — fourteen chapters, one concept each, from a window
  with text to a headless test — built with `mdbook build docs/book`
  (CI does), and `examples/rust/tutorial/` is the ten programs it
  reads, `01_hello.rs` to `10_testing.rs`, each the last plus one
  concept, registered as `tutorial_NN_<name>` and run on the shipped
  launcher with no harness around them. A chapter's code is included
  from its step by anchor, so the book cannot say what the step does
  not do; step 10's `mod tests` runs under `cargo test`. The README
  opens with a *Start here* that is step 1, and `howto.md` points a
  reader with no question yet at the book. `apps/counter.rs`, the
  Rosetta example, spells its messages as a `#[derive(Message)]` enum
  now, as the book's step 3 does; its Node, Lua and C twins build the
  same maps by hand as before. *What you can delete:* nothing — this
  is the release that adds the first page.

### Changed

- **A row holding a share of the room gives as CSS's flex items do**
  (backlog RG92, from the regression run of 2026-09-30, asked for as a
  CSS mirror). F110 let a `Percent` or a size expression give in an
  overflowing row by the fit children's rule, the largest first and each
  down to 0: 30% and 70% squeezed by 300 came out 150 and 150, and a
  `50%` column holding a 120 px button went to 50 with the button
  spilling out. Now every child of such a row that can give — the
  shares and the fit children beside them — gives in proportion to its
  size (90 and 210) and stops at its min-content, CSS's `min-width:
  auto`: the widest thing in it that cannot wrap, which for text is its
  longest word, measured off the shaped glyphs with `unicode-linebreak`'s
  break opportunities and kept with the text, so asking lays nothing out;
  a row's fixed children and gaps add up, a column's widest counts, and
  its padding is on top. A declared `minWidth` is the floor instead, `0`
  included (`min-width: 0`), and a child that scrolls that axis has
  none. The height axis the same way in a column. A row of fit children
  alone keeps the largest-first rule, a `Fixed` child still keeps its
  size, and a table's percent columns still never shrink. The
  min-content is measured by the shrink, only for the children of a row
  that overflowed with a share in it, so a frame whose shares fit pays
  nothing for it; the bench's `squeeze_1000_rows` (new, in
  `benches/sizes.rs`) reads a thousand squeezed rows of two `"50%"`
  labels at 779 µs against 757 under the old rule, +2.9%, and its rows
  of fit boxes and of shares that fit unchanged. Tests: `relative_shrink.rs` (the ratio, the content
  floor with a button, a longest word from the stub and from a real
  font, `minWidth: 0`, a scroller, a fit child beside a share, the
  height axis) and kui-ffi's `a_min_of_zero_is_undeclared_and_min_none_declares_zero`.
  *What you can delete:* a `minWidth` a share carried to keep its
  content from spilling out of it, and a percentage written so its
  neighbour keeps its proportion when the row squeezes.

- **The cargo registry is named `drydock9`, after its host, not
  `forgejo`**: the `[registries.drydock9]` table in `.cargo/config.toml`,
  `publish = ["drydock9"]` in each crate, the workspace dependencies,
  `--registry drydock9` in `ci.yml` and `scripts/release-local.nu`, and
  the token variable `CARGO_REGISTRIES_DRYDOCK9_TOKEN`. Nothing an app
  builds changes: the name is local to whoever reads the config, and
  `cargo publish` writes the index URL into a packaged manifest, not the
  name, so a project whose own config says `[registries.forgejo]` keeps
  resolving kui through it. A token stored by `cargo login --registry
  forgejo` is under the old name in `~/.cargo/credentials.toml`; rename
  the table there or log in again as `drydock9`.

### Fixed

From the regression run of 2026-09-30 over F108–F113 (backlog
RG77–RG91 and RG93–RG98; RG99 open).

- **A node-anchored float's `maxWidth "50%"` takes its anchor** (RG77):
  its size-expression clamps were resolved against 0 before the anchor
  was placed, and a devtools tab's content laid out 0 wide.
- **A negative px max is no size expression** (RG78).
- **A size expression nests at most 32 deep and holds finite numbers**
  (RG79, RG80): a view declaring a `NaN` each frame no longer fills the
  process-wide expression table.
- **A Node view reads the frame it is for** (RG81): `createApp`'s loop
  takes the frame's cause before the views run
  (`Ctx.beginFrameCause()`), so `frameCause()` inside a view says `key`
  on the key's frame, not the frame before's. A `KuiWindow`'s view runs
  only when the model changes, so there the two answer the last frame
  drawn, as their docs now say.
- **kui's own doors that ask for a frame name the line that called
  them** (RG82).
- **An Option let go of in a popup no longer swallows every key after
  it** (RG83): which Option is Alt is read from winit's own flags, the
  ones it rewrote the press by, and the held modifier keys are the
  keyboard's window's, not the popup's. A window coming back with
  Option already held no longer types the key.
- **Lua `option_as_alt = "none"` leaves a side the host declared**, as
  Node's does (RG84).
- **A forced release of a modifier key says the state after** (RG85).
- **Node's key doors take `caps_lock` / `num_lock` as a `key` event
  spells them**, so a heard press handed back keeps its locks (RG86).
- **A wheel between two frames no longer empties
  `owed_by().scrolls`** while the frame is still owed (RG89).
- **A table cell's clamps and its column take the columns' room and
  the largest expression** (RG90, RG91).
- **A frame no longer fails whole once the process has spelled 65 536
  size expressions** (RG93): the table every window shares never lets
  one go, so a `{ max: [dragX, { percent: 30 }] }` fed a splitter's
  fractional drag filled it, and the next new expression failed the
  Node or Lua frame it was in. It now leaves that one prop at its
  default — a width fit, a clamp none — and the core warns
  `size-expressions-full` once, naming the last refused; the
  expressions kept still resolve, and a bad spelling still fails as
  before. C's `kui_size_parse` writes `KUI_FIT` and says true for one,
  as its builders already reduced it. The cap is in the `width` docs,
  the LuaLS `kui.Size`, howto and kui.h now: declare one expression per
  layout, with the part that moves a px size of its own.
- **The Node encoder, the parser and the core read one size grammar**
  (RG94): a spelled function inside data — `{ min: ['max(1px, 2%)',
  30] }`, which the type allows and Lua and the core took — is read by
  the encoder's own copy of the core's grammar and crosses as numbers,
  where the encoder refused it. The spellings every binding is run
  through are one table, `crates/kui-core/tests/fixtures/size_spellings.json`,
  read by the core's, Lua's and Node's tests; the readings it changed
  are under What breaks.
- **The lock keys are one keyboard's, and AltGr is held** (RG96): Caps
  Lock and Num Lock are tracked for the app where the OS is not asked
  (Linux), so a popup reads what its owner turned and a toggle in one
  window reaches another; AltGr is the right Alt, recorded as held with
  its bit on, where it was `unknown` there; and a held modifier is
  recorded by where it is, so an X11 left Alt read as `Meta_L` going
  down and `Alt_L` coming up leaves no Super held. Read off the Mac and
  pinned in unit tests; the window round on Linux and Windows is RG99.
- **A minimized or covered window stops drawing its animation** (RG98):
  every frame its surface skipped went unpaced, since only a presented
  frame paced the next, and an animating macOS window drew about 2 800
  skipped frames a second from the minimize until AppKit stopped
  asking. An animation now asks for no frames while the window is
  covered or minimized, and picks up where its clock has got to when it
  comes back; a skipped frame paces the next as a presented one does,
  so a waker or the host asking meanwhile is held for the display. A
  covered window's animation pauses, as a hidden browser tab's does.
  Five seconds minimized cost an animating release build 4.5 s of CPU,
  now 0.2 s; visible, the same.
- **The frame that brings a minimized window back says `occlusion`,
  not what was asked while it was down** (RG97): a minimized window gets
  no frames, and what the runner noted for it meanwhile — another
  window's input (`elsewhere`), an appearance change — rode into the
  restore frame's `frameCause()` from long before. What was noted
  before the window went dark, and what arrives with the restore (a
  waker's queued wakes), still count.
- **A Lua value or view that holds itself, or nests too deep, is an
  error, not an abort** (RG95): `t = {}; t[1] = t` as a handler's reply, a
  message or a prop said "a table that holds itself", and a view holding
  itself "a view node that holds itself", where both recursed off the
  stack. A value nests at most 64 tables, a view 128 nodes. A debug
  build's Lua view overflowed its stack at 32 nested columns on a test
  thread and 128 on the main thread, each level 64 KB; it now nests past
  the cap either way.

### Docs

- alpha.25's "as a CSS flex item does" for a share of the room was
  wrong until RG92 above made it so (RG87): the rule was the fit
  children's, largest first and down to 0.
- alpha.24's and alpha.27's "What breaks" gained the struct field each
  left out (`EventSpec::modifier_keys`, `schema::PropsOut::option_as_alt`;
  RG88), and ten built backlog entries left in the open list went to
  the archive.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-30 — the
commits after the alpha.27 tag: the kui book (DX27, merged from its
branch), and the regression run over F108–F113 with everything it
filed, RG77–RG98, built the day they were filed. This round ran on the
Mac alone; Windows and Linux were not in it. What the run could not
reach on a Mac is RG96, the key model's readings on Linux and Windows,
which were answered by reading and pinned in unit tests; RG99 holds the
window round on each, and RG98's pause of a covered window's animation
was probed on macOS alone (Windows' minimize was already RG45's).

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.1, Node 26.8.1**, on
`f40bee3` with the version set, in the regression run's worktree (not
cold). `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets --features kui-core/conformance -- -D warnings` are clean.
`cargo test --workspace --features kui-core/conformance`: **1630 tests
over 129 suites, 0 failed** (3 ignored). The C round, `cbuild --run`,
passes its five checks, and the no-ABI panel is refused as "this build
is 22"; the corpus passes its **52 scenes** in four adapters; the ABI is
**22**. Node's `node --test test.mjs` under
`KUI_CONFORMANCE_REQUIRED=1`: **205 of 205**, the frame **v19**. `npm
run gen` leaves no diff, the examples typecheck and their lockfile
installs, and the headless round passes all **32 drives**.

**The windowed round**, `smoke -- --node`: **48 Rust examples — the
book's ten steps among them for the first time — and the eleven Node
examples, each on both bases, 120 frames each, every one exiting 0** —
118 windows — and `counter`, `host`, `c_panel` and `lua_panel` by hand
under `KUI_SMOKE_FRAMES=120`, each exiting 0 with nothing on stderr:
**122 windows over five hosts.** The AX audit: **106/106**, the audited
window raised to the front first.

**The bench guard** against the alpha.27 tag, run on `f40bee3` (the
same code without the version): **green**, no guarded row more than
3.1% slower (`deep_nesting_64_levels`, which read between −1.7% and
+3.4% over the day's five runs with no core change between them), the
worst guarded spread 2.8%, and every one of the 38 rows "same" — the
largest move `update_image_1080p_recycled_and_frame` at +35.6% inside
its own ±35.9% spread, as it was noise in the last round too. Beside
it, what the round's own changes cost where no guarded row reaches:
RG92's CSS squeeze reads a thousand squeezed rows at 779 µs against 757
(`sizes/squeeze_1000_rows`, new), `layout_1000_rows` level after a
second walk it first added was taken out; RG95's value path first cost
kui-lua's `lua_1000_rows/table per frame` 2.7% and now reads within
1.2%; core's `sizes/spelled_cached` reads about 1 ns slower with its
path's source unchanged, left as layout drift. RG98 was measured in a
window: five seconds minimized cost the release `fragment` example
4.5 s of CPU before and 0.2 s now, and visible the same. README's table
is kept at alpha.22's numbers.

## 0.1.0-alpha.27 (2026-09-29)

**What breaks.** New functions and a root prop, the ABI at 22 and the
Node wire at v19; one Rust struct literal:

- Rust: `schema::PropsOut` gains `option_as_alt` (under Added, F113), so
  a struct literal of it needs the field or `..Default::default()`.
  *Listed 2026-09-30*: this section said "No build" until the
  regression run found it (backlog RG88).

One drawing changes:

- A `cells` node draws U+E0B4–U+E0BF — the Powerline Extra half circles
  and wedges — from the cell box, not the font (under Fixed, F112):
  what those twelve characters draw changes in every grid, and a
  screenshot pinned against the fallback font's glyphs needs re-taking.

### Added

- **The Option keys as Alt on a Mac** (backlog F113, from kawoosh's
  settings-pane report). `ui.option_as_alt(OptionAsAlt::Left)` / a root
  `optionAsAlt="left"` / `option_as_alt = "left"` /
  `kui_set_option_as_alt(ctx, KUI_OPTION_AS_ALT_LEFT)` — `none`, `left`,
  `right` or `both` — makes that Option key Alt in the window: ⌥u, ⌥e,
  ⌥i, ⌥n and ⌥`, dead keys on a Mac that started an accent and never
  arrived as a key, now arrive as `<A-u>` like every other ⌥ chord, and
  a key under that Option types nothing, as under Control. One side
  leaves the other composing, so a user keeps `ü` on the right Option.
  Frame state in `alwaysOnTop`'s shape, default `none` — every app that
  does not declare it keeps the Mac's own Option — and the frame that
  stops declaring it gives the keys back; the runner applies it to the
  window on change through winit's `set_option_as_alt`, never per frame.
  Nothing on Windows or Linux, whose Alt composes nothing.
  `Ctx.optionAsAlt()` / `kui_option_as_alt_get` read the ask back.
  *What you can delete:* a keymap's second spelling of an Alt binding
  as the character the Mac composes (`µ` for `<A-m>`), and the
  alternative key a Mac user was told to press because `<A-u>` never
  fired.

### Fixed

- **A rounded row in a terminal is rounded** (backlog F112, from
  kawoosh, 2026-09-29; every binding). yazi ends its hovered row with
  U+E0B6 before it and U+E0B4 after, the Powerline Extra half circles a
  starship prompt's rounded segments use too; `cells` drew U+E0B0–U+E0B3
  from the cell box (F66) and sent these to the font, and a face without
  them fell back to one whose glyph at that codepoint was a squiggle the
  size of its own line box beside each end of the row. The whole
  Powerline Extra separator row now comes from the cell box as the
  arrows do: the filled half circles (U+E0B4, U+E0B6) a half ellipse the
  cell wide and tall on its flat edge, so it meets the run beside it
  with no seam, and their arcs (U+E0B5, U+E0B7) in the light stroke; the
  corner wedges (U+E0B8, U+E0BA, U+E0BC, U+E0BE) a right triangle on
  the cell's diagonal, two of which tile the cell, and the diagonals
  between them (U+E0B9, U+E0BB, U+E0BD, U+E0BF). The cells' text and
  access value are unchanged. **What you can delete:** a Nerd Font
  symbols face bundled or required so that a TUI's rounded rows do not
  draw a stray glyph at each end.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-29 — the
commits after the alpha.26 tag: F112 from the kawoosh rounded-row
report and F113 from the kawoosh settings-pane report, each with its
merge. This round ran on the Mac alone; Windows and Linux were not in
it. F112 is the core's drawing of twelve codepoints from the cell box,
the same on every platform, and its tests hold it (no corpus scene, as
for F66: the corpus compares trees, not the atlas). F113 acts on a Mac
alone: its frame state and its four doors are what the tests, the C
round and the corpus's `chrome` scenes hold, and what it does to a key
was checked by hand — presses posted to a scratch window logged winit
rewriting a left ⌥u to `u` with `alt` and leaving a right one `¨`, and
in a focused window from kawoosh, on kui by path, a left ⌥u arrived as
the chord. What the round could not reach is the other half of the
claim: that Windows and Linux, whose Alt composes nothing, are
unchanged by it.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.0, Node 25.6.0**, on
`ef6a542` with the version set, in a cold worktree. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1599 tests over 124 suites, 0
failed** (3 ignored), kui-native's `plays_through_a_device_or_degrades_gracefully`
among the passes. The C round, `cbuild --run`, passes its five checks,
and the no-ABI panel is refused as "this build is 22"; the corpus
passes its **52 scenes** in four adapters, C's through
`target/debug/conformance`; the ABI is **22**. Node's `node --test
test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`: **202 of 202**, the frame
**v19**. `npm run gen` leaves no diff, the examples typecheck and their
lockfile installs, and the headless round passes all **32 drives**.

**The windowed round**, `smoke -- --node`: **38 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 98 windows — and `counter`, `host`, `c_panel` and
`lua_panel` by hand under `KUI_SMOKE_FRAMES=120`, each exiting 0 with
nothing on stderr: **102 windows over five hosts.** The AX audit:
**106/106**, with the audited window raised to the front first —
launched from a shell while another app was frontmost it never became
key, and the eight checks that need a key window read nothing (98/106).

**The bench guard** against the alpha.26 tag, the machine otherwise
quiet (the script's busy check silent): **green**, no guarded row more
than 4.0% slower (`deep_nesting_64_levels`, inside its ±5.5%), the
worst guarded spread 5.8% (`frame_1k_curves`), and no row of the 38
more than 7.2% slower (`frame_10k_rects_one_exit`, ±7.1%;
`frame_10k_rects_all_declaring_exit` next at +6.6%, outside its ±2.8%
but inside the tolerance); every row reads "same". What the two add to
a frame the benches run is F113's one field that `begin_frame` clears;
F112 is in the cells' drawing, which no row here reaches. README's
table is kept at alpha.22's numbers.

## 0.1.0-alpha.26 (2026-09-29)

**What breaks.** Nothing: readings and doors added, the ABI at 22 and
the Node wire at v19. `Core::request_frame` and `Ui::request_frame`
track their caller, which changes no call.

### Added

- **Why a frame runs** (backlog F111, from kawoosh, 2026-09-29; Rust,
  Node, C). `Core::frame_cause()` is every reason that reached the
  window since the last frame began, as a `FrameCause` set: the input
  the core was handed, by kind, recorded by the core for every driver;
  what the driver saw and noted with `note_frame_cause` — kui-native
  notes the wake, the resize, the scale, focus, occlusion, the
  appearance, the caret's blink, a surface retry, the pacer's overdue
  frame, the device reopened, a frame's own events answered after it,
  another window's input, a menu, a sound; and `owed` when the frame
  before left one owed. `set_frame_trace(true)` adds two readings:
  `owed_by()`, who held that owed frame (`OwedBy`: the nodes mid-
  transition by label path and slot, the cycles, the departures, the
  easing scrollers, the autoscroller, the `animate` nodes, and each
  `request_frame` line as a `Location` — kui's own asks named for what
  they are), and `frame_unchanged()`, whether the last frame drew
  exactly what the one before drew. Read from a view through
  `ui.core()`. Node: `frameCause()`, `owedBy()`, `frameUnchanged()`,
  `setFrameTrace()`. C: `kui_frame_cause`, `kui_note_frame_cause`,
  `kui_set_frame_trace`, `kui_frame_unchanged` and the
  `KUI_FRAME_CAUSE_*` bits. Off, the trace costs nothing; the reasons
  are an OR per input. **What you can delete:** the guesswork behind a
  window that draws with nothing moving — bisecting transitions and
  `request_frame` calls to find the one that keeps `animating()` true,
  and wondering which OS event the runner answered that your app never
  heard.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-29 — the
commits after the alpha.25 tag: F111 from the kawoosh frame-ledger
round with its merge. This round ran on the Mac alone; Windows and
Linux were not in it. F111's readings are the core's, the same on every
platform, and its C and Node doors are what the tests and the C round
hold; what the round could not reach there is kui-native's notes of a
frame's cause as the Windows and Linux event loops deliver them.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.0, Node 25.6.0**, on
`81a4bad` with the version set, in a cold worktree. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1591 tests over 124 suites, 0
failed** (3 ignored), kui-native's `plays_through_a_device_or_degrades_gracefully`
among the passes. The C round, `cbuild --run`, passes its five checks,
and the no-ABI panel is refused as "this build is 22"; the corpus
passes its **52 scenes** in four adapters, C's through
`target/debug/conformance`; the ABI is **22**. Node's `node --test
test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`: **201 of 201**, the frame
**v19**. `npm run gen` leaves no diff, the examples typecheck and their
lockfile installs, and the headless round passes all **32 drives**.

**The windowed round**, `smoke -- --node`: **38 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 98 windows — and `counter`, `host`, `c_panel` and
`lua_panel` by hand under `KUI_SMOKE_FRAMES=120`, each exiting 0 with
nothing on stderr: **102 windows over five hosts.** The AX audit:
**106/106**.

**The bench guard** against the alpha.25 tag, the machine otherwise
quiet (the script's busy check silent): **green**, no guarded row more
than 0.2% slower (`deep_nesting_64_levels`, ±0.1%, and
`frame_10k_rects`, ±1.0%), the worst guarded spread 6.2%
(`frame_1k_curves`), and no row of the 38 more than 3.7% slower
(`copy_1080p_frame`, inside its ±13.5%); every row reads "same". The
trace is off in every bench, so what F111 adds to a frame at rest is
the OR per input and the one `owed()` per frame `begin_frame` takes.
README's table is kept at alpha.22's numbers.

## 0.1.0-alpha.25 (2026-09-29)

**What breaks.** A build no longer writes `libkui_ffi.a`. The ABI is
22: `KuiSpec` gains four fields. The Node wire is v19. Rust gains a
`Sizing` variant, and the clamp props take a `Bound`.

- C: `kui-ffi` builds a cdylib and an rlib; the staticlib is no longer
  one of its crate types (under Changed). A host that links
  `target/<profile>/libkui_ffi.a` asks for it:
  `cargo rustc -p kui-ffi --lib --release --crate-type staticlib`.
- C: `KuiSpec.min_w_size`, `max_w_size`, `min_h_size` and `max_h_size`
  appended (under Added, F109); the 64-bit size is 680. Recompile — a
  zeroed field is the float clamp as before. `KuiSizing` takes a fifth
  tag, `KUI_CALC`.
- Node: the wire is v19 — `maxWidth` / `maxHeight` are two slots, and a
  sizing, a min or a max has two new modes (under Added, F109). The
  encoder and the addon move together; one without the other is refused
  at the version check.
- Rust: `Sizing::Calc` (under Added, F109), so an exhaustive match on
  `Sizing` needs the arm.
- Rust: `NodeSpec::min_width` / `min_height` take `impl Into<Bound>`
  and `max_width` / `max_height` too, where the maxes took an `f32` —
  a number and a `Min` still convert. `schema::Parsed::Min` is
  `Parsed::Bound`, `Apply::SpecMin` is `Apply::SpecBound`, `min_str` and
  `min_num` return a `Bound`, and `Kind::Max` is the maxes' kind.
- Rust: `LayoutSpec::max_w` / `max_h` hold a negative for a calc clamp
  until layout resolves it (as `min_w` holds `Min::FIT`): a reader
  before layout wants `max_w_px()`. `Min::is_fit` is exactly `FIT`
  where it was any negative.

One reading changes:

- A percentage child of a row that overflows gives, where it kept its
  size (under Changed, F110): two `"50%"` children and a gap now fit
  their row, and whatever overflowed by a share of the room now draws
  narrower. A `Fixed` child and a scrolling row are as they were.

### Added

- **Size expressions** (backlog F109, from kawoosh, 2026-09-29; every
  binding). `width`, `height` and the four clamps take CSS's `min()`,
  `max()` and `clamp()` over lengths and percentages, nested —
  `"clamp(400px, 80%, 1000px)"` — resolved by layout against the
  parent's content box, the box a percentage takes its cut of. As data
  it is never parsed: `{ clamp: [400, { percent: 80 }, 1000] }` (Node,
  sent as numbers), `{ clamp = { 400, { pct = 80 }, 1000 } }` (Lua),
  `kui_size_clamp(kui_size_px(400), kui_size_pct(80), kui_size_px(1000))`
  (C, with `kui_size_min`, `kui_size_max` and `kui_size_parse`), and
  `calc::Expr` with `calc::sizing_of` (Rust). A spelling is parsed once
  and found by its text after that; `calc::parse` is public, so a host
  that validates its own settings holds them to the grammar kui draws.
  **What you can delete:** a view's own arithmetic for a size bounded by
  its parent — reading the parent's width back (`onLayout`, the window's
  size) to compute a clamped px for a child, a frame late and wrong by
  the parent's padding.

### Changed

- **A share of the room gives when the room is spent** (backlog F110,
  from kawoosh, 2026-09-29; ADR 0033's decision 10, amended). A
  `Percent` or a size expression is cut from the content box before the
  gaps between the children take theirs, so two `"50%"` children and a
  20 px gap overflowed their row by 20. They now shrink with the `Fit`
  children when a row overflows, largest first and down to each one's
  floor, as a CSS flex item does (`flex-shrink: 1`): 90 and 90 in a row
  of 200, the numbers a table's percent columns already had. A `Fixed`
  child keeps its size, and a scrolling row overflows on purpose.
  **What you can delete:** a percentage written short of its share to
  leave room for the gaps (`"48%"` for half a row with a gap), or a
  `calc`-style subtraction a view did by hand.
- **The static archive is built on request.** Nothing in the workspace
  links it, and it was the largest file any build wrote. An archive is
  every object file of the dependency tree, std's included, not yet
  dead-stripped and carrying its symbol tables for the link to come:
  545 MB in debug, 442 MB of it DWARF against 18 MB of machine code, and
  73 MB in release, where the cdylib linked from the same objects is
  27 MB and 13.7 MB (aarch64-apple-darwin). Cargo cannot choose a crate
  type per profile, so the manifest drops it for both and the command
  above leaves the archive where it was.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-29 — the
commits after the alpha.24 tag: the static archive built on request,
and F109 and F110 from the kawoosh launcher-sizes review with their
merges. This round ran on the Mac alone; Windows and Linux were not in
it. F109 and F110 are layout, the same on every platform, and their
doors — the C builders, the wire's new modes — are what the corpus
holds in four adapters; what the round could not reach there is the
Windows runner's own build of the new `size.rs` exports.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.0, Node 25.6.0**, on
`8a42ba1` with the version set, in a cold worktree. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1583 tests over 123 suites, 0
failed** (3 ignored) — kui-native's `plays_through_a_device_or_degrades_gracefully`
among them, which had failed earlier the same day on main as well as on
F110's branch (the 30 ms blip reporting no end) and passed here: the
machine's audio device, not a change. The C round, `cbuild --run`,
passes its five checks, and the no-ABI panel is refused as "this build
is 22"; the corpus passes its **52 scenes** in four adapters,
`size-expressions` and `relative-shrink` the new ones, C's through
`target/debug/conformance`; the ABI is **22**. Node's `node --test
test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`: **200 of 200**, the frame
**v19**. `npm run gen` leaves no diff, the examples typecheck and their
lockfile installs, and the headless round passes all **32 drives**.

**The windowed round**, `smoke -- --node`: **38 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0** — 98 windows — and `counter`, `host`, `c_panel` and
`lua_panel` by hand under `KUI_SMOKE_FRAMES=120`, each exiting 0 with
nothing on stderr: **102 windows over five hosts.** The AX audit:
**106/106**.

**The bench guard** against the alpha.24 tag, the machine otherwise
quiet (the script's busy check silent): **green**, no guarded row more
than 5.9% slower (`frame_10k_segments`, ±3.0%), the worst guarded spread
3.0%, and no row of the 38 more than 8.5% slower
(`frame_10k_rects_square_clip`, inside its ±5.3%); every row reads
"same". No bench declares a size expression or overflows a row with a
share, so what F109 and F110 add to a frame at rest is the flag each
`distribute_axis` reads and the wider match `shrink_axis` makes; what a
calc costs when declared is in F109's entry (`benches/sizes.rs`, 69.4
against 61.8 µs for 1000 rows). README's table is kept at alpha.22's
numbers.

## 0.1.0-alpha.24 (2026-09-28)

**What breaks.** The ABI is 21: `KuiSpec` gains `modifier_keys`. Rust
gains fields on two structs and variants on one enum.

- C: `KuiSpec.modifier_keys` appended (under Added, F108). On a 64-bit
  target it takes what was the struct's tail padding, so the size stays
  648, but a host that did not recompile leaves those bytes to chance;
  on a 32-bit one the size moved. Recompile.
- Rust: `KeyPress` gains `location` and `locks` (under Added, F108), so
  a struct literal of it needs the fields or `..KeyPress::new(..)`.
- Rust: `EventSpec` gains `modifier_keys` (under Added, F108), so a
  struct literal of it needs the field or `..Default::default()`.
  *Listed 2026-09-30*: the regression run found it missing (backlog
  RG88).
- Rust: `KeyCode` gains `PrintScreen`, `Pause`, `Menu`, `Clear`, the
  modifier and lock keys (`Shift`, `Ctrl`, `Alt`, `Super`, `CapsLock`,
  `NumLock`, `ScrollLock`) and twelve media keys, and `F(n)` reaches 35
  (under Added, F108), so an exhaustive match on it needs the arms.

Three readings change:

- The native runner delivers F13–F35, Print Screen, Pause, the menu
  key, Clear and the media keys to a key sink (under Added, F108): a
  keymap that heard nothing from them hears their names now.
- A press and its release are matched by location as well as position
  (under Added, F108): the keypad's `1` and the main block's, or the
  two Shifts, are two keys held, where a release of one let go of the
  other.
- The key payload carries `location`, `caps_lock` and `num_lock`
  (under Added, F108); a host that compared payloads whole sees three
  more fields.

### Added

- **Where a key is, the modifier keys as keys, the lock state, and the
  rest of the keyboard** (backlog F108, ADR 0002's amendment of
  2026-09-28, from kawoosh, 2026-09-28; `modifier_keys` in Lua and on
  `KuiSpec`, `modifierKeys` in Node, `NodeSpec::modifier_keys` in
  Rust). A key press says which of a key's twins it was —
  `location: "left" | "right" | "numpad" | "standard"` (`KeyLocation`,
  `KeyPress::location`) — so the keypad's `1`, still `code: "1"`, is
  told from the main block's, and the left Shift from the right; and
  what the lock keys held, `caps_lock` and `num_lock` (`KeyLocks`,
  `KeyPress::locks`): asked of the OS on macOS and Windows, tracked
  from the lock keys' own presses elsewhere. A sink that says
  `modifierKeys` hears the modifier and lock keys themselves —
  `shift`, `ctrl`, `alt`, `super`, `capslock`, `numlock`, `scrolllock`,
  the side in `location` — and every other sink still only ever sees a
  modifier held, so a keymap mid-sequence does not read a Shift as a
  key. F13–F35, `printscreen`, `pause`, `menu`, `clear` and the media
  keys (`mediaplaypause`, `mediastop`, `medianext`, `mediaprev`,
  `mediaplay`, `mediapause`, `mediarecord`, `mediafastforward`,
  `mediarewind`, `volumeup`, `volumedown`, `volumemute`) are keys like
  any other, where the runner dropped them. The doors take the place
  and the locks with the key: C in the `kmods` word beside the
  modifiers (`KUI_KLOC_LEFT` / `_RIGHT` / `_NUMPAD`, `KUI_KLOCK_CAPS` /
  `_NUM`), Node in the mods object (`location`, `capsLock`,
  `numLock`), Rust as `with_location` and `with_locks`. A terminal
  speaking kitty's keyboard protocol needs all of it; so does a game
  that binds a lone Shift. The corpus's `modifier-keys` scene, in all
  four bindings, presses a left Shift, the keypad's Enter and Caps
  Lock on a sink that asks and one that does not.
  *What you can delete:* nothing an app could have written: the runner
  dropped every key it had no name for, and said nothing of where a
  key was or what was locked.


### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-28 — the
three commits after the alpha.23 tag, F108 from the kawoosh
kitty-keyboard review, its check on a keyboard and its merge. This
round ran on the Mac alone; Windows and Linux were not in it, and
F108's halves there — the lock state asked of Windows, tracked from
the lock keys' own presses on Linux, and the keys past F12 and the
media keys as those runners name them — are what it could not reach.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.0, Node 25.6.0**, on
`c884d7b` with the version set, in a cold worktree. `cargo fmt --all
--check` and `cargo clippy --workspace --all-targets --features
kui-core/conformance -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1568 tests over 121 suites, 0
failed** (3 ignored). The C round, `cbuild --run`, passes its five
checks, and the no-ABI panel is refused as "this build is 21"; the
corpus passes its **50 scenes** in four adapters, `modifier-keys` the
new one, C's through `target/debug/conformance`; the ABI is **21**.
Node's `node --test test.mjs` under `KUI_CONFORMANCE_REQUIRED=1`: **199
of 199**, the frame still **v18**. `npm run gen` leaves no diff, the
examples typecheck and their lockfile installs, and the headless round
passes all **32 drives**.

**The windowed round**, `smoke -- --node`: **38 Rust examples and the
eleven Node examples, each on both bases, 120 frames each, every one
exiting 0 with no warning on stderr** — 98 windows — and `counter`,
`host`, `c_panel` and `lua_panel` by hand under `KUI_SMOKE_FRAMES=120`,
each exiting 0 with nothing on stderr: **102 windows over five hosts.**
The AX audit: **106/106**, the fixture silent. F108 itself was checked
on a keyboard before the merge (`be89259`), through kawoosh's terminal
speaking kitty's protocol to a program logging its bytes — the two
Shifts and Command with their sides, the keypad's `1` and Enter as keys
of their own, F13 — and the two readings that check found wrong were
fixed there; this round did not drive a keyboard again.

**The bench guard** against the alpha.23 tag, the machine otherwise
quiet (the script's busy check silent): **green**, no guarded row more
than 3.6% slower (`frame_10k_rects`, ±3.0%), the worst guarded spread
5.1% (`frame_1k_typical`), and no row of the 38 more than 4.4% slower
(`list_100k_rows_variable`, inside its ±6.5%). No bench drives a key,
so the dispatch F108 touched is measured by none of them; the frame
rows see only `EventSpec`'s one new `bool`. README's table is kept at
alpha.22's numbers.

## 0.1.0-alpha.23 (2026-09-28)

**What breaks.** No door changes; the ABI stays at 20 and the frame at
v18. Rust gains fields on one event view; Node's `ctx.mouse` takes a
number too.

- Rust: `ButtonEvent` gains `line` and `byte` (under Fixed, RG75), so a
  struct literal of it needs the fields.

Eight readings change:

- A paragraph past 4 KB whose first frame laid out on estimated rows
  asks for the frame that lays it out on its shaped ones (under Fixed,
  RG70): an idle view draws one or two more frames after first showing
  one, where its last rows stayed clipped until the next input.
- Copying a rich line past 4 KB, or a rich `break-spaces` text, puts
  its spans' formatting on the clipboard's HTML (under Fixed, RG71),
  where it put plain escaped text.
- Two identical texts drawn at two widths in one frame answer
  `textHit` and `caretRect` each at its own width (under Fixed, RG72),
  where both answered at the width laid out last.
- A `break-spaces` line under 4 KB is shaped whole, and a line past it
  is cut at a tab where one falls near a cut (under Fixed, RG75): a tab
  in it stops where the line's stops are, where one after the first
  kilobyte stopped up to a tab width off.
- A pixel scroll gesture no touch began — a mouse whose driver scrolls
  smoothly on macOS, a high-resolution wheel between notches on Wayland
  — ends when the pointer moves, as a wheel's does (under Fixed, RG73):
  spun over one pane and then over another within 200 ms, it scrolls the
  second, where it went on scrolling the first. A trackpad's swipe and
  its glide keep their target as before.
- The window losing the keyboard mid-drag ends the drag (under Fixed,
  RG75): an `onDrag` node hears `end` and a slider its slide's `end`
  where the pointer was, and a press held across the blur clicks
  nothing, where the drag went on after the window came back.
- `revealRow` for an index past the list's end, or with a stride that
  is not positive, scrolls nothing and answers false (under Fixed,
  RG75), where it scrolled to the end and answered true; `rowsInView`
  of such a stride is 0.
- Once a window's frame retries are spent, a skipped frame after half a
  second with none gets fresh ones (under Fixed, RG74), where nothing
  but a presented frame or being uncovered asked again.

### Fixed

- **A long paragraph's first frame kept its estimated rows** (backlog
  RG70). C19 lays a paragraph past 4 KB out on an estimate before its
  chunks are shaped, and asked for no frame when emission's rows came
  out otherwise, so an idle view kept its last rows clipped. Emission
  now asks for one when a chunk it shaped moved the rows, and only for
  a row count it has not asked for already, so a chunk cache over its
  budget cannot ask forever.

- **Copying a long rich line lost its formatting** (backlog RG71).
  `selection_html` took a long line for one style; each chunk the
  selection touches now goes through the run's HTML as a short text's
  does (a chunk never shown, or evicted, is still plain).

- **One text at two widths answered at one** (backlog RG72). Identical
  texts share a shaped run, re-wrapped for each node as it draws, so
  the hit test and the caret answered at the width last laid out. A
  second node asking another width in the frame takes its own copy
  (`own_wrap`), which its drawing and its queries read.

- **Spent frame retries never came back** (backlog RG74). F102 asks for
  a skipped frame again, sixty times, and a window away long enough to
  spend them — on a platform that says nothing about being covered,
  Windows and likely Wayland — came back behind a skipped frame and
  showed the stale one. A skip after a rest (`REST`, 500 ms) is a new
  frame asked for and gets tries of its own; a window skipping without
  pause (F103's animation) stays spent.
  *What you can delete:* a redraw an app asked for on its window's
  `focused` event to paint over a stale frame.

- **A smooth-scrolling mouse's gesture followed nothing the pointer
  did** (backlog RG73). The runner took every pixel delta for a
  trackpad's, whose glide a pointer move must not re-aim. winit does not
  say which device sent them but does say the phase, and a touch
  surface's stream opens with `Started` where a wheel's never does, so a
  pixel gesture is a swipe once it has said so and a wheel's until then.

- **Readings the regression pass filed** (backlog RG75).
  `revealRow` / `reveal_row` / `widgets::reveal_row` past the end and
  `rowsInView` of a zero stride (above). An app's `onClick` payload of
  kind `"button"` inside a line-drawing sink lost its `clicks`; the
  core's own `button` event is told by its fields. `UiEvent::button()`
  reads `line` and `byte`. Node's `ctx.mouse` takes a button's number
  (`3 + n`), so a test drives the buttons past the middle one.
  `testing::Drive` gains `button_click` and `scroll_gesture`. The window
  losing the keyboard ends a primary drag, slide, caret, selection or
  scrollbar drag, and clicks nothing (above). The `tooltip` docs say
  its hint floats on a box or a fragment and, on a leaf, is spoken and
  not drawn. Tab stops on a long line (see the readings): a line under
  4 KB is one chunk, and one past it is cut after a tab near the cut,
  so the next chunk starts on a stop; a tab after a tab-free stretch of
  more than half a window still measures from its chunk's start.
  *What you can delete:* ending a splitter's drag by hand on the
  window's `blurred` event.


### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-28 — the
twelve commits after the alpha.22 tag, RG70–RG75, the six the alpha.22
regression pass left open. This round ran on the Mac alone; Windows and
Linux were not in it, and RG73's Wayland half and RG74's are the
platforms it could not reach.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.0, Node 26.8.1**, on
`3a8d96e`. `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets -- -D warnings` are clean. `cargo test --workspace
--features kui-core/conformance`: **1561 tests over 121 suites, 0
failed** (3 ignored). The C round passes its five checks, the corpus
its **49 scenes** in four adapters, the ABI still **20**; Node's `npm
test` **198 of 198**, the frame still **v18**. `npm run gen` leaves no
diff, the examples typecheck, and the headless round passes all **32
drives**.

**The windowed round**: **38 Rust examples and the eleven Node
examples, each on both bases, 120 frames each, every one exiting 0 with
nothing on stderr** — 98 windows — and `counter`, `host`, `c_panel` and
`lua_panel` by hand the same way: **102 windows over five hosts.** The
AX audit: **106/106**, the fixture silent. RG73 was checked in a window
driven by CGEvents before the tag: phaseless precise pixels over a list
re-aimed at a terminal when the pointer moved, where a phased swipe the
same way kept the list.

**The bench guard** against the alpha.22 tag, alone on a quiet
machine: **green**, no guarded row more than 2.6% slower
(`deep_nesting_64_levels`), the worst guarded spread 1.4%.
`KUI_BENCH=long_line` against alpha.22: `long_line_100k_edit` +4.7%
(±1.7%), in pieces each inside its own noise — RG70–RG72 +1.7% (±3.0%)
and the tab cut +2.1% (±1.9%) after it was made one pass (it read +4.6%
walking each window twice) — about 55 µs on an edit frame of a
100k-character line; every other row within its noise. README's table
is kept at alpha.22's numbers.

## 0.1.0-alpha.22 (2026-09-28)

**What breaks.** The ABI is 20 and the Node frame v18. C gains one door,
`kui_input_scroll_gesture`, and `KuiSpec` grows to 648 bytes; Rust gains
fields on four spec structs, an `InputEvent` and a `TextWrap` variant.
Seventeen readings change.

- C: `KuiSpec` gains `pixel_snap`, `keep_focus`, `on_focus`, `rules`,
  `rule_w`, `on_button`, `buttons`, `overscroll` and `scroll_axes` at its
  end, so `KUI_ABI_VERSION` is 20: an [in] append (under Added,
  `pixelSnap`, DX10, DX18, DX21, F105 and F107); its 64-bit size is 648
  bytes. Recompile against the new header; zeroed, a box is drawn as
  before, a press focuses as it did, nothing hears focus, a table draws
  no rules, no node hears the middle button, a scroller at its limit
  passes a gesture on and an `on_scroll` node takes both axes.
- C: `KuiSpan` gains `bg_radius` at its end under the same ABI 20
  (under Added). On a 64-bit target it takes what was the struct's tail
  padding, so the stride holds at 40 bytes, but a host that did not
  recompile leaves those bytes to chance; on a 32-bit one the stride
  moves. Recompile; a zeroed field is the square background.
- C: `kui_input_scroll_gesture(ctx, dx, dy, begins)` is a new door
  (under Added, F107); `kui_input_scroll` is unchanged.
- Node: the binary frame is v18. A span carries one more slot, its
  background's radius (`bgRadius`, under Added), and `family` is a
  string, a stock name or an installed family's, where it was the index
  into `schema::FAMILIES` (under Added, ADR 0037); `pixelSnap` rides by
  its id. The encoder and the addon ship together, so this breaks only a
  stale prebuilt addon, which refuses the stream by its version.
- Rust: `Ui::child_key_index` and `Core::child_key_index` are
  `child_key_indexed`, the spelling of every other `_indexed` (under
  Added, DX5).
- Rust: `NodeSpec::width` / `height` / `modal`, and `width` / `height` on
  a `Keyframe`, `Enter` or `Exit`, take `impl Into<…>` (under Added,
  DX1 and DX3). A call that passed `x.into()` no longer infers its
  target: drop the `.into()`.
- Rust: `VisualStyle` gains `pixel_snap`, `AccessSpec` gains `tooltip`,
  `EventSpec` gains `keep_focus`, `on_focus`, `on_button`, `buttons` and
  `scroll_axes`, and `InteractSpec` gains `rules`, `rule_w` and
  `overscroll` (under Added, `pixelSnap`, DX6, DX10, DX18, DX21, F105
  and F107), so a struct literal of any of them without `..` needs the
  fields.
- Rust: `InputEvent` gains `ScrollGesture { delta, begins }` (under
  Added, F107), so an exhaustive match on it in a driver of your own
  needs the arm. `InputEvent::Scroll` is unchanged, and is a gesture of
  its own.
- Rust: `schema::Kind` gains `Family` and `schema::Parsed` gains
  `Family(FontFamily)`, so an exhaustive match on either in a binding of
  your own needs the arm; `TokenLookup` carries the session.
- Rust: `PumpRunner::route_events`' closure takes the core of the event's
  window as a third argument, `FnMut(&mut A, UiEvent, &mut Core)` (under
  Added, ADR 0036). Add the parameter; `|app, ev, _|` keeps what it did.
- Rust: `TextWrap` gains `BreakSpaces` (under Added, F106), so an
  exhaustive match on it needs the arm. C's `KUI_WRAP_BREAK_SPACES` is
  3, a new value of the `wrap` field the ABI already has.

The readings:

- A square-cornered quad (a `bg`, a span's background, an image with no
  `radius`, a solid underline, a `fragment` node's box) covers each pixel
  by the area of it inside the rect (under Fixed). On whole pixels it is solid to its edge, where
  its outermost pixels drew at 93%, so its edge is a pixel crisper. At a
  fractional edge the pixel it falls in is part-covered as before. A
  rounded quad keeps its ramp.
- A text's span backgrounds are one quad per line for each run of spans
  of one background, where they were one per span, and each edge is on a
  whole pixel (under Fixed). A background can draw up to half a pixel
  from where it did. Boxes are drawn where layout put them, as before.
- A family registered with no 400 face, one face lighter than 400 and
  one between 400 and 500 — a Light and a Medium — is asked for regular
  at the heavier one, where it was asked at the lighter one (under
  Fixed, RG59). That is the order CSS font matching tries them in. Regular
  text in such a family draws heavier and, when the faces differ in
  width, measures wider.
- Text already shaped when a face of its family is loaded or removed is
  shaped again at the family's new weights (under Fixed, RG59). Bold
  synthesized before the family's Bold was loaded draws in that Bold
  from the next frame, where it stayed synthesized until the text left
  the cache.
- A sink's `text` event that answers a paste carries `pasted: true`
  (under Added, DX14) — a host's paste reply, or any commit while a paste
  is outstanding. A handler that matches the payload exactly sees one more
  field.
- The wheel over a scroller that scrolls on one axis passes the other
  axis to the scroller or `onScroll` node under it (under Added, DX13),
  where the inner one swallowed it and nothing moved.
- A scroll gesture keeps the target it started on, and one that starts
  over a scroller already at its limit that way goes to the scroller
  around it (under Fixed, F107). The native runner's swipe and its glide,
  or a wheel spun without a pause or a pointer move, is one gesture: a
  swipe that moved the strip goes on moving it when a terminal or a list
  comes under the pointer, where the new node took the rest. A notch or
  swipe over a list at its end moves the page around it, where it moved
  nothing; `overscroll: "contain"` keeps the old stop. A gesture that
  reaches a limit midway still stops there. A bare `Scroll` from a
  driver of your own is a gesture of its own.
- A trackpad swipe keeps to one axis (under Fixed, F104): the other
  axis's delta is dropped while the swipe and its glide last, so a
  diagonal swipe moves a two-axis scroller one axis at a time, and an
  `onScroll` node hears `delta` with one component zero. A mouse wheel's
  notches are unchanged.
- An `exit` plays only when its node's parent is still declared (under
  Added, DX19). A node that goes because an ancestor went — a tab
  switched away, a panel closed around it — goes at once, where every
  node with an `exit` inside it faded out; put the `exit` on the node
  that goes, or keep its parent declared and empty it.
- The exit budget is 4096 nodes, where it was 512 (under Fixed, DX23).
  A removal of 513 to 4096 nodes declaring `exit` now animates, where it
  vanished at once with an `exit-budget` warning; one past 4096 is
  refused as before.
- The first font an app registers — by name, from bytes, from a file or
  a folder — maps the database's installed font files once (under Fixed,
  DX24): ~30 ms at that call on a Mac with 1,311 faces, after which a
  family's first shaping costs ~0.4 ms where it cost ~9.7 ms. Every file
  loaded after it maps its own faces the same way (DX25).
- A hover event carries `by: "pointer" | "content"` (under Added, DX20),
  and the window's `window` event has two more phases, `focused` and
  `blurred`, raised when `env.focused` changes (under Added, DX18). A
  handler matching either payload exactly sees the new field or events.
- `reveal` and `setScroll` by a label no frame has declared — Lua's
  `env.reveal` / `env.set_scroll`, Node's `reveal` / `setScroll` — wait
  for the frame to finish (under Added, DX15), where they threw "no node
  is keyed". A label that frame does not declare either is the
  `label-without-node` warning. `focus` and `access` still throw.
- `caret_rect` on a space a `word` break swallowed — the whitespace the
  row broke at, which has no glyph — answers the end of the row it
  broke (under Fixed, F106), where it answered the end of the
  paragraph's last row; `text_hit` on that row's end answers the same
  place.
- `setScroll` asking an eased container (one with a `transition`) for
  the offset its ease is already going to lets the ease go on (under
  Fixed, RG62), where it started it again from where it was drawn: a
  view that asks every frame until a row shows now scrolls to it, where
  the content stood still and frames were asked for without end.
- A `word` or `glyph` line past 4 KB, drawn in chunks, can start a row
  with a chunk's first word (under Fixed, RG68), where that word split
  after its first glyph and the glyph hung past the box.
- A `family` string other than `sans`, `serif` or `mono` — in JSX or a
  Lua view — draws the installed or loaded family of that name (under
  Added, ADR 0037), where it threw "bad value … for family". A name that
  matches nothing draws sans and raises `unknown-family` once.

### Added

- **`bgRadius`: a span's background rounded, and joined into one shape
  with the ones it meets** (backlog F101, from kawoosh, 2026-09-27;
  [ADR 0035](docs/adr/0035-a-rounded-background-is-joined-by-meeting.md)).
  `bgRadius` on a `<span>` in JSX, `bg_radius` in a Lua span table and
  on `KuiSpan` in C, `Span::bg_radius(r)` in Rust, in logical px. A
  rounded background is one shape with every rounded background of the
  same colour and radius it meets. That is a piece whose edge touches
  it exactly on the line above or below and overlaps it sideways, or one
  that meets it end to end on its own line, such as a line's text and
  the cell an editor draws for its newline. The piece can be in the same
  text (a wrapped paragraph's lines) or in another (an editor's rows).
  Its corners are convex where a line reaches past its neighbour, a
  fillet where it falls short, square where the two end together, and
  round where nothing meets it. A selection over rows, or over the
  wrapped lines of a markdown paragraph, is one rounded outline. Nothing
  names the shape, and two that touch are one. The join runs once the
  frame's quads are all emitted, so every text's lines are known, and it
  uses the layout of the frame it draws: the outline is never a frame
  behind the text. Each piece is a `fragment` quad painted by a source
  the core registers itself (`fragment::JOIN`, as the polygon's is), so
  no backend learned a kind. It is clipped as its text's parent is, so
  a fillet past a short no-wrap line's end shows. At 0, the default, the
  background is the square one it was. The corpus gains
  `joined-backgrounds`, re-expressed in Lua, C and Node: four texts of
  mono spans, three in one translucent colour joined, the fourth in
  another beside them. Pinned by `tests/joined_backgrounds.rs` in
  kui-core (the pieces' extents and neighbours across texts, a wrap,
  colours and radii that stay apart, pieces meeting end to end) and in
  kui-wgpu (the shape composited through the fragment's mirror), by a
  Node test and by the ffi's
  `a_span_s_bg_radius_joins_its_background_across_the_boundary`. Lua
  was seen going red with the radius dropped.
  *What you can delete:* a rounded selection drawn by the app as a
  `fragment` under each row, told its own extent and its neighbours'
  from the last frame's layout, and the line rects of a wrapped
  paragraph worked out to draw one under it.

- **`pixelSnap`: a box painted on whole pixels.** A flag on any node
  (`pixelSnap` in JSX, `pixel_snap` in Lua and on `KuiSpec`,
  `NodeSpec::pixel_snap()` in Rust). The node's background, border,
  shadow and, on a `fragment` node, its fragment's quad are painted with
  each edge on a whole physical pixel, each edge
  rounded on its own from where layout put it (`snap_px(x)` and
  `snap_px(x + w)`), the rounding a text's span backgrounds use. Boxes that
  share an edge in layout then meet on one pixel line. So do a box and a
  text's background, which is what an editor needs: its selection past a
  line's end is a box beside the line's text. So are bands and code-block
  rows stacked at a pitch that is not whole pixels. Unflagged, two such
  boxes each draw part of the pixel their join falls in, and it shows as
  a line. Opt-in because snapping every box closed a 1 px `gap` below 1×
  (see Fixed); a box that does not ask is drawn where layout put it, as
  every box was. Layout, hit-testing, the clip and the children are
  untouched. A snapped box can draw up to half a pixel from its layout
  edge and its size can differ by a pixel, so a snapped hairline is 1 or
  2 px thick by where it sits. A fragment's quad is covered as a solid's
  is, by area when square, so a stack of snapped fragments meets on
  pixel lines too: an editor that draws each row's part of a rounded
  selection as a fragment under its text gets one seamless shape (kawoosh,
  2026-09-27; `snapped_fragments_stack_on_whole_pixels_and_others_where_
  layout_put_them` pins the rects, and fails without the snap). The
  corpus gains `pixel-snap`, re-expressed
  in Lua, C and Node: three boxes at 40.5 by 20.25, two snapped (one with
  a shadow) and one not. Pinned in kui-wgpu by a column of snapped boxes
  at a 21.75 pitch covering every pixel exactly once at nine scales from
  0.5× to 3×, and by an unflagged column keeping its exact rects; both
  fail with the flag ignored.
  *What you can delete:* a box that stood in for a background as a text
  of its own (a space with the colour behind it), so its edge would round
  the way the text beside it does.

- **`Ui::leaf`: a node with no children, in Rust** (from kawoosh,
  2026-09-27). `ui.leaf(spec)`, `ui.leaf_keyed(label, spec)` and
  `ui.leaf_indexed(i, spec)` open a node and close it, returning its key,
  as `with` does with nothing inside. A spacer, a rule, a swatch or a hit
  area was `ui.with(spec, |_| {})`, the empty closure there only because
  `with` takes one; the repo's own trees had 452 of them and now have
  none. The other bindings already had it: `<box … />` in JSX, `row { }`
  in Lua, `kui_open` then `kui_close` in C. No door changes.
  *What you can delete:* the `|_| {}` on every childless `with`,
  `with_keyed` and `with_indexed`.

- **`NodeSpec::tooltip`: a Rust view's hint floats** (backlog DX6, from
  kawoosh, 2026-09-27). `apply_tooltip` is the `tooltip` prop's spec half
  — hover tracking and the accessible description — and floats nothing:
  the parsers and `kui_close` float the hint themselves, and a Rust view
  had no door for the third effect. So kawoosh's "reload settings" hint
  was never seen, and kui's own tooltip example passed each hint twice,
  once to the spec and once to `button_with`. `.tooltip(hint)` is the
  prop whole: the core floats the hint below the node as it closes,
  while it is hovered, as it does the prop's (`AccessSpec::tooltip`,
  read where every node opens; one pointer check for a node with no
  access group). A stock widget given a `hint` floats its own and clears
  the flag, so one hint shows. `apply_tooltip` is unchanged, for a
  caller that floats its own. Pinned by
  `a_spec_tooltip_floats_while_hovered_and_apply_tooltip_alone_does_not`
  and `a_button_given_a_hint_and_a_spec_tooltip_floats_one_hint`, each
  failing without its half.
  *What you can delete:* the `is_hovered` check and `widgets::tooltip`
  call a Rust view wrote beside `apply_tooltip` to float the hint.

- **Rust shorthands for what views spell most** (backlog DX1–DX5, the
  DX sweep of 2026-09-27 over kui's own code and kawoosh's). Each is
  pinned in `tests/conveniences.rs` to the long form it stands for, and
  the repo's own code moved onto them in the same round (about 2,100
  sites; `bench-check.sh` green against the commit before). Nothing reaches the other bindings: each is a spelling of a
  Rust call that already existed. No door changes.
  - *Sizing (DX1).* `width` and `height` take a number of px, as
    `min_width` did (`From<f32> for Sizing`); `size(w, h)` sets both;
    `grow_width()`, `grow_height()` and `Sizing::GROW` are `Grow(1.0)`.
    The same on a keyframe, entrance or exit. Before, `Sizing::Fixed(`
    and `Sizing::Grow(1.0)` were 1,046 of the repo's calls.
  - *A text in a box (DX2).* `ui.text_in(spec, s, style)`, `text_in_keyed`
    and `text_in_indexed`: a box holding one text, returning the box's
    key — the width, padding, background, click or role a text has no
    rows for. The same tree `with(spec, |ui| ui.text(…))` builds.
  - *Values (DX3).* `Value` from `i32`, `u32`, `usize` (saturating at
    `i64::MAX`) and `f32`; `get_str`, `get_int`, `get_float`, `get_f32`
    and `get_bool` for `get(k).and_then(Value::as_*)`; `UiEvent::kind()`
    for the payload's `kind`; `modal` takes `impl Into<Value>` as every
    other tag row did; `NodeSpec::key_sink()` for `on_key(Value::Null)`.
    `#[derive(Message)]` (C50) remains the answer for an app's own
    messages; these are for the core's payloads and a quick tag.
  - *Placement and input (DX4).* `FloatConfig::inside(x, y)` is
    `.at(x, y).self_at(x, y)`, which is how all 44 pairs in kui and
    kawoosh were written; `KeyMods::NONE.with_shift().with_ctrl()` (and
    `with_alt`, `with_super`, `with_primary`) and `Mods::NONE.with_word()`
    for the struct literals; `Rect::center()`.
  - *The node verbs (DX5).* `fragment_indexed`, `fragment_with_indexed`
    and `line_indexed`, which the core had and `Ui` did not;
    `child_key_indexed` (under What breaks); four doc comments in `ui.rs`
    that sat on the neighbouring method moved to their own.
  *What you can delete:* `Sizing::Fixed(` around a number, a
  `with(spec, |ui| ui.text(…))` wrapper, `x as i64` into a `Value`,
  `.and_then(Value::as_str)` after a `get`, `Value::str("…")` around a
  tag, and `.self_at(…)` repeating `.at(…)`.

- **Typed views of the core's own events, in Rust** (backlog DX7).
  `ev.drag()`, `ev.key_press()`, `ev.text()`, `ev.scroll()`, `ev.hover()`,
  `ev.modifiers()` and `ev.layout()` read a core event's payload into a
  struct (`Drag`, `(KeyPhase, KeyPress)`, `TextInput`, `Scroll`,
  `Hover`, `KeyMods`, `Layout`), `None` for an event of another
  kind; `ev.tag()` is the app's tag inside one, and `Drag::ratio()` the
  pointer's place across the parent, 0 to 1 — a divider's split. The
  payload stays the wire. `#[derive(Message)]` reads the app's half;
  these are the core's. splitmux reads its keymap, modifiers and three
  drags through them. Pinned against events real input produced.
  *What you can delete:* a `KeyPress` rebuilt from `code`, `shift`,
  `ctrl`, `alt`, `super` and `text`; `get("phase")` matched as strings;
  a ratio worked out from `x` and `parent`.

- **A key built from a label and an index** (backlog DX8).
  `ui.open_key(key, spec)`, `with_key` and `leaf_key` open a node under
  a key the caller built — `ui.child_key("gap").index(id)` — with no
  string formatted and no clash with the sibling-index keys `_indexed`
  gives. Two nodes under one key are `duplicate-key`, as two labels are;
  a built key has no label for `key_of`.
  *What you can delete:* `&format!("gap{id}")` labels, one of them
  built twice for `child_key` and `with_keyed`; index offsets (`2000 +
  i`, `1 << 32 | i`) kept to stay clear of the auto keys.

- **`keepFocus`: a press that leaves the keyboard where it was**
  (backlog DX10; `keep_focus` in Lua, Rust and on `KuiSpec`). A press on
  the node or anywhere inside it acts — the click, the drag, the hover —
  and keyboard focus stays with the editor or key sink that had it: a
  toolbar, a tab strip, a divider. The node stays a Tab stop. The
  corpus's chrome scene declares it on its window-button strip in all
  four bindings.
  *What you can delete:* taking focus back after a click on a toolbar
  button — kawoosh's `reclaim_focus`, set in ten handlers.

- **`kui_native::testing::Drive`: the headless driver, published**
  (backlog DX11). The driver every example's `--headless` runs on, for
  an app's own tests: it owns a `Core` (or borrows one), routes events
  through its `Extensions` as the runner does, turns the node snapshot
  on, and has `frame`, `click`, `click_key`, `double_click`, `drag`,
  `wheel`, `move_to`, `hover`, `key`, `keys`, `text`, `commit`,
  `focus`, `rect_of`, `texts_under`, `warnings`, `log` and `check`.
  `framing()` frames after every gesture and once while a drag is held.
  `key` gives a plain character or the space its text, as a keyboard
  does.
  *What you can delete:* a copy of kui-devtools' `Drive` (kawoosh's
  `harness.rs`, 477 lines) — keep only the readings of your own tree.

- **The wheel passes the axis a scroller does not scroll** (backlog
  DX13). A `scrollY` list inside a `scrollX` strip moves the strip on a
  sideways swipe; a diagonal notch moves each on its own axis. An
  `onScroll` node still takes the whole notch.
  *What you can delete:* a horizontal offset kept by the app and
  re-set every frame because the list under the pointer ate the swipe.

- **A paste's answer says it is one** (backlog DX14). The sink's `text`
  event carries `pasted: true` when it answers a paste the app asked
  for — the host's paste reply, or any commit while the ask is
  outstanding, since a bare commit is how some hosts answer; an IME
  commit that lands in that window is marked too. `TextInput::pasted`
  reads it in Rust.
  *What you can delete:* a flag of the app's own, set on the ask and
  cleared on the next `text`, to tell the clipboard's text from typing.

- **A divider, a revealed row and a row spec** (backlog DX12 in Rust,
  DX22 in Lua and Node).
  `widgets::splitter(ui, label, dir, thickness, tag)` is a divider
  between two panes: the theme's border colour, its accent while hovered
  or held, the resize arrows, `on_drag(tag)` and `keep_focus`; the
  handler's `ev.drag().ratio()` is the new split. `widgets::reveal_row(ui,
  label, i, row_h)` centres a `uniform_list`'s row `i` when it does not
  show — called before the list, so that frame builds the rows it
  scrolled to, where `reveal` finds nothing for a row the list has not
  built — and `rows_in_view` is a page's stride. `uniform_list_with`
  takes `row_spec(i)` for each row's own node. Lua's prelude has
  `reveal_row(env, key, i, row_h)`, `rows_in_view`, `splitter(env, { key,
  dir, thickness, on_drag })` and `row_props = function(i)` on
  `uniform_list`; Node exports `revealRow(ctx, key, i, rowH)`,
  `rowsInView`, `splitter(ctx, { key, dir, thickness, onDrag })` and takes
  `rowProps: (i) => props` on `uniformList`, typed in `index.d.ts` and
  typechecked through the examples' `types.tsx`.
  *What you can delete:* a divider's hover-or-pressed colouring and its
  ratio arithmetic; `scroll_geometry` plus an in-view test plus
  `set_scroll` to reach a list's row; a second node inside every row to
  carry its click.

- **A family by name** (ADR 0037, backlog DX17). `family = "Berkeley
  Mono"` in Lua and `family="Berkeley Mono"` in JSX draw an installed
  family, or one loaded with `loadFontsDir` / `loadFontFile`, in the frame
  that names it: the parser registers the name in the session (a query of
  the font database's scan, no file opened) and gives the text the handle
  `addSystemFont` gives. `sans`, `serif` and `mono` stay kui's own. A
  name nothing matches shapes as sans and raises the new
  `unknown-family`. `Ui::system_font(name)` is the Rust pass-through; C
  keeps `kui_font_add_system`. The TS type offers the stock three and
  takes any string.
  *What you can delete:* a host door that registers a family for a
  script and hands the handle down — kawoosh's `kawoosh.fonts.face` and
  its registration of all 613 families at once — and the `font =` a
  view passed where it meant a family.

- **An event handler gets its window's core** (ADR 0036, backlog DX9).
  `App::on_event_with(&mut self, ev, core: &mut Core)` is lent the core
  of the window the event came from, as Node's `update` has its surface,
  so a handler writes the clipboard, asks for a paste, moves focus,
  reveals or scrolls in answer to the event. It defaults to `on_event`,
  so an app that overrides only that one is unchanged. The runner, the
  pumped runner and `testing::Drive` all lend it. The clipboard example
  copies, pastes and answers the log's selection there.
  *What you can delete:* a model field parked for the next `view` to
  carry out — the text to copy, a paste asked for, a focus to take back,
  an answer owed — and the branch in `view` that did it.

- **Focus as events** (backlog DX18). `onFocus` (`on_focus` in Lua,
  Rust and on `KuiSpec`) hears keyboard focus entering and leaving the
  node's subtree as `{kind:"focus", phase:"in"|"out", by, tag}`, `by`
  being `pointer`, `keyboard`, `assistive` or `program` — reported after
  the input that moved it or at the end of the frame that declared it.
  The window raises `{kind:"window", phase:"focused"|"blurred"}` when it
  gains and loses the keyboard. The corpus's chrome scenes declare it.
  *What you can delete:* the focused key or `env.focused` kept from last
  frame to diff against.

- **An `exit` plays where its node was removed** (backlog DX19). A node
  whose parent left the frame too goes at once with it, unless that
  ancestor's own `exit` carries it — React's `AnimatePresence` rule. A
  tab switched away no longer fades out every column in it.
  *What you can delete:* an `exit` dropped from nodes because it played
  when their container went.

- **A hover says what moved** (backlog DX20): `by: "pointer"` when the
  pointer moved, `by: "content"` when a frame put something else under a
  still one. `ev.hover()` is a `Hover { phase, by }`.
  *What you can delete:* bookkeeping to tell a row sliding under a still
  pointer — the keys scrolling a picker — from the pointer moving.

- **A table's grid rules** (backlog DX21). `rules` (a colour) and
  `ruleWidth` on a table draw a line down each gap between its columns
  and across each gap between its rows, under its cells and on whole
  pixels. The corpus's table scene declares them.
  *What you can delete:* rule cells between a table's cells and an edge
  row, built to draw its grid.

- **`reveal` and `setScroll` by a label the frame has not declared
  yet** (backlog DX15). `Core::reveal_label` / `set_scroll_label` (and
  `Ui`'s) resolve when the frame finishes — the scroll before layout, so
  that frame lays out at it — and Lua's and Node's string forms fall
  back to them. A label the frame does not declare is the new
  `label-without-node` warning.
  *What you can delete:* `pcall` around `env.reveal` and a retry on the
  frames after.

- **`onButton`: the middle and secondary buttons, press to release**
  (backlog F105, from kawoosh, 2026-09-28; `on_button` and `buttons` in
  Lua, Rust and on `KuiSpec`). A node declaring `onButton` hears the
  non-primary buttons pressed over it, or over anything inside it that
  claims none: `{kind:"button", phase:"press", button, x, y, clicks,
  tag}`, and then the button is the node's until it comes up — every
  pointer move while it is held is `phase:"move"`, its release
  `phase:"release"`, on that node wherever the pointer went. `button` is
  `"secondary"`, `"middle"` or a further button's number; on a `cells`
  grid each event carries `cell: {row, col}`, clamped to the grid, as a
  click does. `buttons` narrows what it claims — `"middle"`, `"secondary
  middle"`; `Buttons::MIDDLE` in Rust, `KUI_BUTTONS_*` bits in C — and
  unset it is all three kinds. A claimed secondary press is the button
  event instead of `contextmenu` and the stock menu, so a terminal whose
  program asked for mouse reports takes the right button from the menu
  for as long as it says `buttons` with `secondary` in it. Several
  buttons can be held at once, each its own capture; a primary drag is
  untouched; and no non-primary press moves focus, the caret, a
  selection or a scrollbar, as before. A press nothing claims routes as
  it did. `UiEvent::button()` reads the event in Rust (`ButtonEvent`),
  `ButtonMsg` in TypeScript. The corpus's controls scene claims the
  middle button on its panel in all four bindings and holds it across a
  move, beside the secondary presses its menu still takes.
  *What you can delete:* nothing an app could have written: the middle
  button reached the core and went no further, so a terminal pane had no
  middle-click paste and reported only the primary button to a program
  that asked for the mouse.

- **`wrap="break-spaces"`: whitespace that takes its room** (backlog
  F106, from kawoosh, 2026-09-28; `TextWrap::BreakSpaces` in Rust,
  `KUI_WRAP_BREAK_SPACES` in C). CSS's `white-space: break-spaces`: a
  text wraps between words, and a space is a glyph like any other — one
  that does not fit starts the next row, before or after its word, so
  no space hangs past the edge and none vanishes at the break. Every
  byte has a caret place inside the box and a background on a space is
  drawn where the space is. An editor's wrapped line wants it: kawoosh
  typed a space after a full row's last word and saw its caret past the
  pane, then back on the full stop, then on the next row, because under
  `word` cosmic-text hangs one space over the edge and drops the next
  two with the break. The text is laid out by kui's own row breaker —
  the long line's (C19, C42) at any length — so it is shaped unwrapped
  and broken at UAX #14's opportunities, each trailing whitespace
  character a piece of its own; a text with line breaks of its own, a
  `maxLines` or an `ellipsis` wraps as `word`, and so does an `edit`,
  which lays its own buffer out. The corpus's `break-spaces` scene sets
  `ab  c` in a box 4 px wide in all four bindings: each character a row,
  the two spaces' background two solids.
  *What you can delete:* padding an app kept at a wrapped text's right
  edge so a hanging space's caret stayed inside it.

- **`overscroll` and `scrollAxes`: where a scroll gesture may go**
  (backlog F107, ADR 0038, from kawoosh, 2026-09-28; `overscroll` and
  `scroll_axes` in Lua and on `KuiSpec`, `NodeSpec::overscroll` and
  `NodeSpec::scroll_axes` in Rust). A scroll gesture picks its target
  when it starts: the innermost scroller under the pointer that can
  still move the way it goes (see the gesture entry under Fixed).
  `overscroll: "contain"` (`Overscroll::Contain`,
  `KUI_OVERSCROLL_CONTAIN`) keeps a gesture that starts over a scroller
  at its limit there instead of passing it to the one around it, on the
  axes the scroller scrolls: a popup's list or a sheet whose scrolling
  must never move what is behind it. `scrollAxes: "x" | "y"`
  (`ScrollAxes`, `KUI_SCROLL_AXES_*`) says which axes an `onScroll`
  node takes, both unless it narrows them; a gesture on the other
  passes it by — a terminal that scrolls its history says `"y"`, and a
  sideways swipe over it moves the strip it sits in. Drivers of their
  own say where gestures begin with `InputEvent::ScrollGesture { delta,
  begins }` (`kui_input_scroll_gesture(ctx, dx, dy, begins)` in C); a
  bare `Scroll` is a gesture of its own. The corpus's `scroll-gestures`
  scene holds a contained list at its end and a `y`-only handler met by
  a sideways notch in all four bindings.
  *What you can delete:* nothing an app could have written: the core
  picked the node under the pointer for every delta, and a node that
  takes the wheel could not decline an axis.

### Fixed

- **A pane of more than 512 nodes could not fade out** (backlog DX23,
  from kawoosh, 2026-09-27). `depart::MAX_NODES` is 4096. Measured: a
  departing subtree costs ~0.065 µs a node to depart and ~0.021 µs a node
  a frame to replay, so 4096 nodes are 267 µs and then 87 µs a frame,
  less than the same pane alive. kawoosh's fonts and themes panes
  (1,500–1,800 nodes) raised `exit-budget` and vanished; they fade. The
  corpus's exit scene moved with the number in four adapters, and
  `drop_5k_rows_declaring_exit` is the bench of the refusal.
  *What you can delete:* an `exit` dropped from a big pane because it
  tripped the budget.

- **A family's first shaping cost ~9.7 ms** (backlog DX24, from kawoosh,
  2026-09-27). cosmic-text ranks every installed face for a new family,
  weight or style, opening each face's file to read its weight axis while
  the file is unshared. The first font an app registers now maps the
  installed files once (~30 ms), and a family's first shaping costs ~0.4
  ms. An app on the stock families pays nothing new.
  *What you can delete:* warming families ahead of time to keep their
  first shaping off the frames (kawoosh's `warm`).

- **A font loaded from a file was left unshared** (backlog DX25, from
  kawoosh, 2026-09-28). DX24 shared the installed faces before the load
  that asked for it, so the file being loaded, and every one after,
  still opened and mapped itself each time a text shaped in a new
  family, weight or style. `load_font_file`, `add_font_data` and
  `load_fonts_dir` now share what they add. kawoosh loads 167 files it
  ships, and its fonts pane's frame that first shows a family went from
  6.6 ms to 2.0 ms (worst 16 → 7.4 ms, none over 8 ms).

- **The frame after the glyph atlas emptied rasterized every glyph on
  screen again** (backlog DX26, 2026-09-28). A view whose text turns
  over a little each frame — a list scrolling through fonts — fills the
  atlas every so often, and the page is emptied between frames; the
  frame after looked up its whole visible set on the empty page and
  rasterized all of it, ~600 glyphs and 2.5–5.3 ms in kawoosh's window,
  every ~40 frames of its fonts pane. The emptied page is now kept for
  that one frame, and what it held is copied, not rasterized. kawoosh's
  worst layout frame in that walk went from 5.5 ms to 2.0.

- **Text backgrounds that adjoin showed a seam at every join** (from
  kawoosh, 2026-09-26). An editor's selection is a translucent `bg` on
  each span of its syntax runs, row after row, and it drew as stripes.
  Three things made them. *The shader ramped every edge*, square corners
  included, over 1.5 px, so a rect on whole pixels still drew its
  outermost pixels at 93%, and two rects sharing an edge left a 2 px band
  of what lay under them. A square-cornered quad now covers each pixel by
  the area of it inside the rect. *A background was a rect per span*, its
  ends wherever the glyphs put them, so a join inside a pixel was drawn
  in two halves, and two halves of a translucent colour do not add up to
  the whole. Neighbouring spans of one background are now one rect, and
  a text's backgrounds (a span's, the ADR 0017 highlight) have each edge
  snapped to a whole pixel. *They are snapped from where layout put the
  text*, not from its drawn origin, which is rounded already: from there
  a row's rect reached a pixel into the next row wherever the rows'
  pitch was not whole pixels, and every other join was drawn twice.
  Snapping every box's edges the same way was tried and taken back. It
  put kawoosh's newline box on the text's grid, but below 1× it closed a
  1 px `gap` between two boxes (their edges rounded to the same pixel)
  and at 1.5× made it 1 or 2 px. A box is drawn where layout put it, so
  the gap is there at any scale, and one that has to meet a text's
  background or another box without a seam asks for it: `pixelSnap`
  (under Added). Pinned by
  `tests/seamless_backgrounds.rs` in kui-wgpu, which composites through
  the shader's coverage. A five-row selection from a fractional offset,
  at eight scales from 1× to 2.175×, is the selection's own alpha at
  every pixel. Editor-shaped rows (a line's text, then its newline as a
  `pixelSnap` box, an empty line's box alone), at three line heights,
  three offsets and six scales, have no pixel drawn twice and no gap. A
  1 px gap between boxes is drawn at twelve scales from 0.5× to 3×. Each
  was seen failing with its part taken out, and the gap with every box
  snapped.
  *What you can delete:* a selection or highlight drawn as one box behind
  a line to hide the seams between its spans' backgrounds.

- **A glyph the atlas refused for room on a page that began the frame
  empty stayed blank for as long as the page lived** (backlog RG56,
  from the regression pass of 2026-09-26). A frame whose own glyphs
  come to more than a 4096 page holds — large CJK or emoji at 2×, big
  box-drawing cells — draws without the ones that do not fit, and the
  next frame would too, so nothing asked for it. The refusal was cached,
  and so were the text templates and cell tables built on that frame.
  When the view then scrolled to a part of the set, nothing looked the
  refused glyphs up and nothing emptied the page, and they stayed
  blank. Before F99 a full 4096 page reset mid-frame, so the glyph drew
  after one wrong frame. Now the atlas keeps what it refused and, until
  the page is next emptied, measures each frame. Text templates and
  cell tables look their glyphs up again on each such frame (they key
  their slots on `GlyphAtlas::stamp`, which moves with `epoch` and on
  every measured frame), and the atlas counts the texels of the
  distinct slots the frame used. A frame that wanted a refused glyph,
  and whose glyphs fit in what the page was seen to hold, asks for the
  next frame, and that one begins on an empty page and draws them. A
  view that still shows the whole set keeps the page as it is, with
  nothing emptied or asked for. Cost: none on a page that has refused
  nothing. On one that has, every frame rebuilds the text templates and
  cell tables it draws, until the page is emptied.
  *What you can delete:* nothing an app could have written.

- **Four readings of F100's weights** (backlog RG59, from the
  regression pass of 2026-09-26).
  *A face's `OS/2` weight shadowed a named instance at the same
  weight*: a variable face whose `OS/2` weight is 400 and whose default
  instance is its Thin, with a Regular at 400, drew regular Thin. Named
  instances now go first, and the default stands for the `OS/2` weight
  only when no instance names it.
  *A face loaded or removed did not reach text already shaped*: the
  shaped-text cache, cell tables and editors were keyed by content and
  style, and a reweigh changed neither. Now a registration or removal
  that changes the weights of a family registered before it drops each
  window's shaped text and cell tables at its next frame, and gives
  every editor the new weights (text, caret and history kept).
  Registering a new family drops nothing, so a list that registers a
  family per row as it scrolls does not reshape the window each time.
  *The nearest face leaned light where CSS leans heavy*: regular with
  faces at 300 and 500 was asked at 300. It now follows CSS Fonts 4
  § 5.2, so that case is asked at 500. Regular looks up to 500 first,
  then down, then above 500. Bold looks up first, then down.
  *Limit, now written down*: Skia's `OS/2` weight is 5, off the CSS
  scale, so its regular is asked at 5. A glyph Skia lacks falls back at
  weight 5, and a variable fallback such as `.SF NS`, whose axis starts
  at 1, shapes and draws near hairline. Clamping the asked weight to
  100–900 would pass Skia itself over for another family: cosmic-text
  takes a family's face only at its exact weight or inside its `wght`
  axis, and Skia's axis runs from 0.48 to 3.2. The rasterizer cannot
  correct it alone either, since the fallback face was also shaped at
  that weight.
  *What you can delete:* nothing an app could have written.

- **A frame the surface skipped is asked for again, and a window
  uncovered draws** (backlog F102, from kawoosh, 2026-09-27). A window
  brought back with ⌘-Tab or Alt-Tab showed the frame from before it went
  away until something else asked for one: the frame its focus asked for
  was skipped, the surface still calling the window occluded, and only a
  window's *first* frame was ever asked for again. Now any skipped frame
  is owed. The next is asked for 16 ms later, up to 60 times, until one
  lands, as the first frame was (`mod retry` in kui-native). Where the
  platform says a window is covered (`WindowEvent::Occluded`, on macOS
  and X11), a skip there owes nothing, and when the platform says the
  window is uncovered a frame is asked for at once. Probed on macOS 27 with the
  `counter` example hidden and shown by AppleScript: before, two skips
  and no frame until the process was killed; after, the frame lands as
  the window is uncovered. Pinned by `retry.rs`'s four tests (a first
  frame, a later skip, the bound, covered and uncovered).
  *What you can delete:* a redraw an app asked for itself on regaining
  focus, a beat later, to get past the skip.

- **A trackpad swipe down nudged the strip sideways** (backlog F104,
  from kawoosh, 2026-09-28). A finger never moves along one axis alone,
  and the runner passed both axes of every trackpad delta on, so a
  vertical swipe's few pixels of drift — and its momentum's — reached
  whatever scrolls sideways: since DX13 the strip around a `scrollY`
  list, and an editor's own text column where an `onScroll` node takes
  the whole delta. A swipe now locks to the larger axis once it has
  travelled 4 px and drops the other while it and its glide last (`mod
  axis_lock` in kui-native); a 200 ms pause ends it, and the other axis
  carrying three times the locked one's recent travel (and at least
  24 px) turns the lock, so a hand that turns without lifting is
  followed. Pixel deltas only — a trackpad's, a Magic Mouse's; a wheel's
  line notches pass whole. Pinned by `axis_lock.rs`'s six tests.
  *What you can delete:* a dead zone or axis filter an app put on the
  `scroll` event's `delta` itself.

- **A caret on the space a `word` break swallowed went to the end of the
  paragraph** (backlog F106, from kawoosh, 2026-09-28). cosmic-text
  drops the whitespace a word-wrapped row breaks at, and `caret_rect`,
  finding no glyph for the byte, answered the paragraph's last row —
  rows below the space. A byte with no glyph now sits at the end of the
  last row that starts before it, the row it broke; the end of the
  paragraph is still its last row's end. The hit test's row (AR30) reads
  the same place. Pinned by
  `a_space_a_word_break_swallows_ends_the_row_it_broke`.
  *What you can delete:* nothing an app could have written.

- **A swipe across the strip stopped hard at a terminal, and a list at
  its end kept a swipe the scroller around it could use** (backlog F107,
  ADR 0038, from kawoosh, 2026-09-28). The wheel's delta went to
  whatever scroller was under the pointer when it came, and a two-finger
  swipe does not move the pointer: a swipe that moved kawoosh's pane
  strip carried a terminal under it, and the terminal, an `onScroll`
  node taking every delta for its history, took the rest of the swipe.
  A scroll gesture now latches: the native runner begins one after a
  200 ms pause (F104's gap), on a switch between a wheel's notches and a
  trackpad's pixels, and for a wheel when the pointer moves (`mod
  scroll_gesture` in kui-native), and the core keeps each axis's target
  from the gesture's first event on that axis to its end, found by key
  wherever the pointer or the content has gone (`runtime::gesture`). The
  pick chains: a container at its limit that way (within half a pixel)
  is passed for the one around it, unless it says `overscroll:
  "contain"`; an `onScroll` node takes the axes its `scrollAxes` names
  whether or not it can move. Nothing chains midway, as in a browser. A
  latched target gone from the frame or behind a modal is picked again.
  Programmatic scrolls, scrollbars, keys, `reveal` and a drag's edge
  scrolling are untouched. Pinned by `tests/scroll_gestures.rs` (10), the
  runner's `scroll_gesture.rs` (3), `surface.c` and the corpus.
  *What you can delete:* a check an app made on the `scroll` event for
  whether its node was where the swipe began.


- **The regression pass before the tag** (backlog RG60–RG69, found and
  built 2026-09-28, each pinned by a test seen failing without its
  fix). Five read-only reviews of the diff since alpha.21, every claim
  probed before it was fixed:
  - A gesture over a scroller at its limit chained to whatever scroller
    was painted under the pointer — a list at its end in a popover moved
    the page it floated over. It walks out through the scrollers around
    it in the tree now (RG60, F107; ADR 0038 amended). And the wheel read
    the tree by a region's index, so an event between `begin_frame` and
    `finish` panicked; a region carries what the pick needs (RG61).
  - `setScroll` for the offset an ease was already going to started the
    ease over, so `reveal_row` called every frame held an eased list
    still (RG62).
  - The atlas page DX26 keeps for one frame lived until the next
    `begin_frame`, which an idle window may never reach: up to 64 MiB
    held. It goes as its frame finishes (RG63).
  - A scrolled table drew its `rules` over what was above and below it;
    a departing table lost them on its first ghost frame; and a
    `column` section between rows had the rules drawn through it. The
    rules take the table's own clip, fade with its ghost, and go around
    a child that is no row, as a `colspan` cell (RG64, DX21).
  - An `onButton` capture outlived the window losing the keyboard — the
    owner heard every later move as a drag — and a second press of the
    held button dropped the first owner without a `release`. Both end
    in a `release` now (RG65, F105).
  - A focus move the program made was reported with the next input's
    `by`; it is `"program"`, ahead of that input's events (RG66, DX18).
  - A press on a `keepFocus` node cleared the text and cell selection
    it was meant to act on, and moved the Tab ring's region under the
    pointer; it leaves both (RG67, DX10).
  - `break-spaces` sent short texts down the long line's path and met
    its limits there: one text at two widths drew and answered at the
    width laid out last, a custom editor's lines had no accessibility
    runs or caret, a key holding several texts answered from the last,
    a word at a chunk edge could not start its row, a right-to-left
    paragraph drew almost empty, a paragraph past a chunk laid out its
    first frame on estimated rows, the text was clipped to its box, its
    selection box and baseline were a line's rather than the box's, and
    a hit past a row's end named that row with the next row's byte
    (RG68, F106). Each node now has its own copy of the line, the long
    line answers through the same queries as a run, and an RTL
    paragraph wraps as `word` does.
  - Node gains `Ctx.scrollGesture(dx, dy, begins)` beside C's
    `kui_input_scroll_gesture`; props.md names the C fields `rule_w`,
    `on_focus`, `overscroll` and `scroll_axes`; the `button` event's docs
    say the runner counts clicks for the primary button alone and that
    `ctx.mouse` takes names; `ButtonMsg` declares `cell`, `line` and
    `byte`; and the 512-node budget, `KUI_OVERSCROLL_AUTO`'s value and
    the `family` doc's "opens no file" are corrected (RG69).
  Six smaller findings are filed open, RG70–RG75.
  *What you can delete:* a `requestCopy` an app issued on the press
  rather than the click of a `keepFocus` button, to beat the clear.


### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-28 —
forty-three commits after the alpha.21 tag (F101–F107, the DX sweep,
RG56/RG59, ADRs 0036–0038) and the regression pass over them (backlog
RG60–RG69, built before the tag and folded in above; RG70–RG75 open).
This round ran on the Mac alone; Windows and Linux were not in it.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.0, Node 26.8.1**, in
a cold worktree. `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets -- -D warnings` are clean. `cargo test
--workspace --features kui-core/conformance`: **1549 tests over 121
suites, 0 failed** (3 ignored). The scene corpus runs in all four
adapters against one reference report: Rust and Lua through `cargo
test`, C through `target/debug/conformance` (**49 scenes**, the ABI
**20**), Node through `npm test` (**198 of 198**, the frame at
**version 18**). The C round, `cbuild --run`, passes its five checks,
and the no-ABI panel is refused as "this build is 20". `npm run gen`
leaves no diff; `npm run typecheck` on `examples/node` is clean and its
lockfile installs. The headless round, `smoke -- --headless`, passes
all **32 drives**.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **38 Rust examples and the eleven Node examples, each on both
bases, 120 frames each, every one exiting 0 with nothing on stderr** —
98 windows. The C and Lua hosts by hand under `KUI_SMOKE_FRAMES=120`:
`counter`, `host`, `c_panel` and `lua_panel` each opened a window and
exited 0 with nothing on stderr — **102 windows over five hosts.** The
AX audit (`scripts/ax-audit.swift`, compiled) against the
`accessibility` example: **106/106**, the fixture silent on stderr.

**The new input, in a window.** No example exercises most of it, so a
probe app driven by CGEvents (continuous pixel swipes with their phases
and momentum, line notches, the middle and secondary buttons) checked,
before and after the regression pass: a swipe with sideways drift keeps
to its axis (F104); a sideways swipe begun over a list keeps moving the
strip while a two-axis `onScroll` terminal comes under the pointer, and
a new swipe after a pause is the terminal's (F107); a middle press is
captured press–move–release, the release past the window's edge, and a
claimed secondary press is a `button` event, not a context menu (F105);
a press into a pane reports `focus in` by the pointer, a `keepFocus`
button clicks without moving it (DX18, DX10); hover says `content` when
rows scroll under a still pointer (DX20); `break-spaces` carries a run
of spaces onto the next row where `word` drops two at the break (F106);
a table's rules draw in its gaps (DX21).

**The bench guard** against the alpha.21 tag, alone on a quiet
machine: **green**, no guarded row more than 2.5% slower
(`list_10k_rows_virtual`), the worst guarded spread 6.2%.
`replay_a_full_depart_store` (+775%) and `drop_1k_rows_declaring_exit`
(+82%) measure DX23's larger budget — a full store is eight times the
nodes, and a thousand-row drop is now copied rather than refused.
`KUI_BENCH=long_line`, the path RG68 reworked, is flat within its
noise. README's table is refreshed from rows whose two runs agreed
within 5%.

## 0.1.0-alpha.21 (2026-09-26)

**What breaks.** No door changes, the ABI stays at 19 and the frame at
v16. Rust gains two methods and a type, and Node and C a door each.
Four readings change:

- A font face whose glyphs cannot be measured (no `head`, `hhea` or
  `hmtx`) is refused (under Fixed, F98). `addFontData` and
  `loadFontFile` (Rust `add_font_data`, `load_font_file`; C
  `kui_font_add`, `kui_font_load_file`) return no handle for a file
  holding only such faces, where they returned one. `loadFontsDir`
  does not count them. `systemFonts()` and `systemFontFamilies()` leave
  out a family of only such faces: GB18030 Bitmap on a Mac. A host that
  unwrapped the handle for such a file fails at load, where the text
  drawn with it used to fail.
- The glyph atlas's size can go down between frames (under Fixed, F99):
  a page that filled mid-frame is doubled for that frame and goes back
  to its size at the next. C's `KuiDrawData.atlas_size`, Node's
  `frameStats().atlasSize` and Rust's `GlyphAtlas::size` read the
  doubled size for one frame. A renderer of your own that sized its
  texture to the largest `atlas_size` it had seen samples the smaller
  page at the wrong scale. Size the texture to each frame's
  `atlas_size`, as `kui-wgpu` does. A set that turns over fast can now
  keep a bigger page than before: a page that has to be emptied twice
  within two frames, turning glyphs over in between, doubles, up to
  4096.
- A frame that grew the atlas no longer asks for another frame
  (`animating()` stays false): it is drawn right as it is.
- Bold of a family with no bold face draws in that family (under
  Fixed, F100), where it drew in whatever family cosmic-text's fallback
  found first: `.SF NS` on a Mac. Bold text in such a family now
  measures at the family's own advances, so a layout that sized itself
  to the fallback's bold (a button's width, a column) comes out at the
  family's width. Regular text in a variable face whose `wght` axis is
  not on the CSS scale (Berkeley Mono Variable) draws lighter: at its
  Regular, where it drew at its Bold. Plain text and editors in a
  family with no 400 face (a Light-only file) draw in that family,
  where they drew in the fallback's.

### Added

- **`Core::update_image_with(id, w, h, fill)`: a streamed image's
  pixels written into a buffer the core recycles** (backlog W20, from
  the Windows–Mac bench comparison). `fill` is handed `w × h × 4` bytes:
  the image's own when nothing else holds them, else the buffer the
  previous update replaced once no display list holds it, else a new
  one. It writes every byte, since what it is handed is an earlier
  frame's pixels. A stream settles on two buffers and allocates nothing
  from its fourth update. The second buffer is kept only for an image
  updated before at the same size, and is let go 30 frames after the
  last update, so an image updated once, a stream that stopped and one
  that shrank hold one buffer of their size. On Windows that was most of the cost: a fresh
  8 MB buffer a frame read ~870 µs there, 590 of it faulting the new
  block in page by page and 170 releasing the old one, against a 275 µs
  memcpy. `update_image_1080p_recycled_and_frame` reads 275 µs on that
  machine, against 875 for `update_image_1080p_and_frame`. The `image`
  example renders its plasma straight into the slice. `update_image`,
  which takes a `Vec`, is unchanged.
  *What you can delete:* a Rust stream's own frame buffer and its
  per-frame `clone()` into `update_image`. Render into `fill`'s slice.

- **`systemFonts()`: the installed families with what they are**
  (backlog F97, from kawoosh's fonts-pane report). kawoosh lists the
  installed families to pick an editor font from and wants the
  monospaced ones first. `systemFontFamilies()` handed out the names and
  dropped what the font database had already read off every face, so
  the app measured an `i` against an `M` in each family: about 5 s for
  613 families on a Mac, every file loaded to shape, and symbol fonts
  such as Webdings called monospaced. `ctx.systemFonts()` lists the same
  families in the same order as `{family, monospaced, weights, italic}`:
  `monospaced` when every face's `post` table says fixed-pitch (Cascadia,
  Fira Code, Iosevka Term, Menlo and a Nerd Font's "Mono" are; its
  "Propo" and Webdings are not), the weights the faces come in, sorted,
  and whether one is italic or oblique. Nothing is loaded or shaped to
  answer. Rust: `Core::system_fonts()` returning `Vec<SystemFont>`
  (`kui_native::SystemFont`); `system_font_families()` is now its names.
  C: `kui_system_fonts(ctx, out, cap)` filling `KuiSystemFont`s, as
  `kui_font_families` fills names, a new [out[]] struct and function,
  ABI still 19. Lua names no font by family, so it has none, as it has
  no `systemFontFamilies`.
  *What you can delete:* the glyph measuring that sorted a font list
  into monospaced and not, and the list of symbol fonts it had to skip.

### Changed

- **Node's `updateImage` and C's `kui_image_update` copy into the
  recycled buffer** (W20) instead of a new `Vec` per call. The signature
  and the copy are unchanged, and the allocation is gone. A 1080p
  `updateImage` in a real window on Windows went from 808 µs to ~335.
  `kui_image_pixels`' pointer is still valid only until the next update
  of the handle, as it says. `kui_image_add` and `kui_image_update`
  refuse a size whose byte count overflows before reading a byte.

### Fixed

- **Han text in `mono` fell back to a face with no metrics** (backlog
  F98, from kawoosh's Han-in-mono report). On a Mac, "字 a" in
  `FontFamily::Mono` panicked in a debug build with `attempt to add
  with overflow` in cosmic-text's `LayoutGlyph::physical`, and in a
  release build it drew every glyph after the ideograph at x = ∞. Menlo
  has no 字. `Mono`'s fallback then takes a monospaced face that maps
  it, and the first was GB18030 Bitmap. That is a bitmap-only face with
  no `head`, `hhea` or `hmtx`, so its units per em read 0 and its
  advances came out infinite. Named families, `sans` and `serif` never
  reached it. The font database now keeps out any face whose glyphs
  cannot be measured: one with no `head` whose units per em are in
  16–16384, no `hhea` with a horizontal metric, or no `hmtx` as long as
  that says. That holds for the installed fonts when the session starts
  and for every face `addFontData`, `loadFontFile` and `loadFontsDir`
  bring in. So such a face is never a fallback, a family to name or a
  row in `systemFonts()`, and 字 falls to a face that measures it. The
  check reads each file's table directory once, on a few threads, and
  adds about 4.5 ms to a Mac's ~20 ms font scan (1312 faces).
  *What you can delete:* a family list's filter for GB18030 Bitmap, and
  any code that kept Han out of `mono` or named a CJK family ahead of
  it to dodge the panic.

- **One frame in a scroll drew nearly every glyph blank or scrambled**
  (backlog F99, from kawoosh's fonts-pane report). kawoosh's font
  picker scrolls a list of cards, each drawing its family's name and
  two lines of code in that family. Now and then, while scrolling or
  filtering, one frame showed the chrome's text and every card blank
  or in scraps, and the next frame was right. Each card brings glyphs
  no earlier frame drew, so the atlas page filled every so often. When
  it filled mid-frame it dropped every slot and packed new glyphs over
  them. The quads emitted earlier in that frame still pointed at the
  old slots, and that frame was presented. Any app whose text turns
  over (new fonts, sizes, CJK or emoji as it scrolls) hit it. Now the
  page is never emptied while a frame is being drawn. `begin_frame`
  empties it before anything is emitted when the rows the last frames
  opened say it is about to fill, which catches the steady turnover. A
  fill it does not foresee doubles the page for that frame with every
  glyph where it was, since quads address the atlas in texels, and the
  next frame goes back to the page's size. A set too big for the page
  fills it on a frame that began it empty, and the page keeps the
  growth (AR19, F83). A page that has to be emptied again within two
  frames, once a frame since the last emptying has turned glyphs over
  on it, doubles for good: the set turns over faster than the page
  holds it. One burst of new glyphs that fits the page (a view of
  other fonts opened beside a steady chrome) does not count. At 4096,
  where the page cannot double, the glyph that does not fit is left
  out of that frame and the next frame, which is asked for, draws it,
  when the page held earlier frames' glyphs. A set bigger than a 4096
  page on its own leaves glyphs out for as long as it holds (backlog
  RG56, open). Cost: an ordinary
  frame does nothing new. Scrolling 400 frames of such a list at 2×,
  kawoosh-style: before, 59 frames were drawn wrong, and each was
  followed by a ~2.2 ms rebuild; after, 15 frames emptied the page
  first at ~2.5 ms each, and the page settled at 2048. A fill nothing
  foresaw costs the frame about 0.5–1 ms at 1024 (the 4 MB copied into
  a 16 MB page) and 2–3 ms at 2048.
  *What you can delete:* nothing an app could have written. The wrong
  frame came from inside the atlas, and nothing outside it could
  reach it.

- **Bold of a family with no bold face drew in another family, and a
  variable face off the CSS scale drew regular at its Bold** (backlog
  F100, from kawoosh's Berkeley-bold report). kawoosh set its editor
  and terminal font to Berkeley Mono Variable, a variable family of a
  Regular and an Italic file, each with a `wght` axis. Bold and bold
  italic in its terminal drew another font's glyphs one to a cell (no
  slash in `0`, a squeezed `m`, gaps round `i` and `.`). cosmic-text
  takes a family's face for a weight only when the face is that weight
  or its `wght` axis spans it, and otherwise goes to the platform's
  fallback list and then to every installed face. It also reads the
  CSS number as the axis coordinate. Berkeley's axis runs from 100
  (its Regular) to 150 (its Bold) with an `OS/2` weight of 400, so 700
  fell outside it and 400 clamped to the Bold. A static family with
  one regular face (Monaco, Andale Mono) sent bold to the fallback too.
  Now each registered family is asked at weights it has faces for, in
  text, rich text, cells and editors alike: bold
  asked of a family with none is asked at its nearest face's weight,
  and its glyphs are marked for the rasterizer to draw bold. A variable
  face draws at the coordinate its named instances give the weight
  ("Regular", "Bold"…) when its axis is off the CSS scale, a marked
  glyph at the axis's bold, and a marked glyph of a face with no
  heavier instance has its outline grown (Skia's fake-bold ratios).
  Italic of a family with no italic face was already drawn in the
  family, leaning. Faces whose axis is on the CSS scale (Cascadia Code,
  200–700) draw as before. Cost: registering a family reads its faces'
  `wght` axes once, ~40 µs a family (686 installed families in 38 ms
  against 11 ms); shaping and a cached glyph cost nothing new. Limit: a
  variable face off the CSS scale is still *shaped* at cosmic-text's
  clamped coordinate, so a proportional one's advances are its
  heaviest instance's (a monospaced one's do not vary).
  A copy of rich text reads bold against the family's regular, so a
  family whose bold is its SemiBold copies `<b>`.
  *What you can delete:* a style that avoided bold, or named another
  family for it, because bold of such a family drew elsewhere.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-26 —
five commits after the alpha.20 tag (W20 and F97–F100) and the
regression pass over them (backlog RG53–RG59: RG53–RG55, RG57 and RG58
built before the tag and folded into the entries above, RG56 and RG59
open). This round ran on the Mac alone; Windows and Linux were not in
it.

**macOS 27.0 on an M3 Pro MacBook Pro, rustc 1.98.0, Node 26.8.1**, on
`main` at `21428ca`. `cargo fmt --all --check` and `cargo clippy
--workspace --all-targets -- -D warnings` are clean. `cargo test
--workspace`: **1423 tests over 104 suites, 0 failed** (1 ignored doc
example). The scene corpus runs in all four adapters against one
reference report: Rust and Lua through `cargo test`, C through
`target/debug/conformance` (**45 scenes**, the header at **395 fields,
269 enum members and 251 prototypes** — `KuiSystemFont` and
`kui_system_fonts` are the difference — the ABI still **19**), Node
through `npm test` (**193 of 193**, the frame still at **version 16**).
The C round, `cbuild --run`, passes its five checks, and the no-ABI
panel is refused as "this build is 19". `npm run gen` leaves no diff;
`npm run typecheck` on `examples/node` is clean and its lockfile
installs. The headless round, `smoke -- --headless`, passes all **32
drives**.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **38 Rust examples and the eleven Node examples, each on both
bases, 120 frames each, every one exiting 0 with nothing on stderr** —
98 windows, the `image` example's stream among them through
`update_image_with`. The C and Lua hosts by hand under
`KUI_SMOKE_FRAMES=120`: `counter`, `host`, `c_panel` and `lua_panel`
each opened a window and exited 0 with nothing on stderr — **102
windows over five hosts.** The AX audit (`scripts/ax-audit.swift`,
compiled) against the `accessibility` example: **106/106**, and the
fixture said nothing on stderr. F97–F100 are pinned by headless tests
on font fixtures (`tests/fonts.rs`, `tests/atlas_turnover.rs`,
`atlas.rs`'s own); none was checked in kawoosh's windows for this tag.

**The bench guard** against the alpha.20 tag, alone on a quiet
machine: **green**, no guarded row more than 0.8% slower
(`frame_10k_segments`), the worst guarded run-to-run spread 2.4%. Every
unguarded row moved less than 5% but
`frame_10k_rects_all_declaring_exit`, −13.7% at ±11.7% noise. The new
`update_image_1080p_recycled_and_frame` reads ~119 µs here, level with
the `Vec` handoff (~114 µs): macOS's allocator hands an 8 MB block
straight back, which Windows' does not (W20). README's table is
refreshed from this run: 29 rows whose two runs agreed within 5%, four
noisier ones kept.

## 0.1.0-alpha.20 (2026-09-26)

**What breaks.** No door, the ABI at 19 and the frame at v16. One
reading changes, one warning is new, and `frameStats()` gains a field.

- An access node's rect, and an editor's text run's, is cut to the clip
  the node is drawn under, and a node wholly clipped away reads as a
  zero-size rect on the clip's edge (under Fixed, F93). A test that read
  the rect of a node on a `clip` canvas, a row half out of a scroller or
  anything inside a `clip` box reads the cut one. Nothing that is not
  clipped moves.
- A `radio` with no `radioGroup` above it, or a `tab` with no `tabList`,
  raises the new `item-outside-container` warning (under Added, F96). A
  test that asserts an app raises no warnings fails on such a view until
  the set is wrapped in its container, which is what the warning asks.
- The devtools' four dock placements are one Tab stop in a
  `radioGroup`, and its tabs one in a `tabList` (under Changed). A
  script that pressed Tab to reach a placement or a tab reaches the
  set's one stop and moves by arrows from there.

### Added

- **`frameStats().wokenPumps`: the pumps an OS event or a wake reached**
  (backlog F94, from both apps' alpha.19 reports). A third monotonic
  count beside `pumps` and `framesTotal`: the pumps whose batch carried
  a window event other than a redraw (a key, the pointer crossing the
  window, a focus change, a resize, the window moved or occluded) or
  something through the loop's proxy (a `Waker` wake, a screen reader
  asking, a file dialog's answer). It is the same signal that makes
  `nextDeadlineMs()` answer 0 and resets `runWindowed`'s backoff, now
  counted. Two readings a second apart with it unmoved are a second the
  desktop left the window alone, so every frame in that second was the
  app's own. Both apps' smoke test, "a stopped window paints about once
  a second", failed 2–3 runs in ten on a loaded machine. A focus change,
  a pointer crossing and a busy WindowServer each reset the backoff
  exactly as the F57 regression would, and the test could not tell them
  apart. In a real window on this Mac, a stopped app with a
  once-a-second tick read 30 pumps, 0 woken and 1 frame per idle
  second; the second the pointer crossed it read 115 pumps, 20 woken
  and 21 frames. In Rust, `PumpRunner::woken_pumps()`. C and Lua have
  no `pumps` either, since their hosts own the loop.
  *What you can delete:* the heuristic an idle-window test wrapped
  around a second the desktop touched (a median of five seconds, or a
  retry when `env().focused`, `owed()` or `cursorShape()` moved in it).
  Measure the second again when `wokenPumps` moved.

- **A radio or a tab outside its container is warned about** (backlog
  F96, from the LCARS pomodoro's alpha.19 report). The pomodoro declared
  `role="radio"` on its three mode buttons in alpha.6 and had three Tab
  stops, no arrows and no "2 of 3" for ten releases, with nothing to
  say why. A `radio` with no `radioGroup` above it and a `tab` with no
  `tabList` now raise `item-outside-container`, once per node, the stock
  radio (`<radio>`, `widgets::radio`, `kui_radio`, Lua's `radio`)
  included. The pairs are the composite ones (`composite::PAIRS`, ADR
  0007). A lone `menuItem` or `listItem` is left alone: the menus build
  their own container, and a row outside a list is only a looser
  reading. Every binding gets it from the core; Node's `WarningCode`
  union and `docs/props.md` list it. The conformance corpus's `sampler`
  scene, whose card is a lone `tab`, pins it in all four bindings.

### Changed

- **The devtools' radios and tabs are composites** (with F96, which
  found them). The four dock placements sit in a `radioGroup` labelled
  "Dock" and the tab strip is a `tabList`: one Tab stop each, the
  arrows, Home and End inside, and a reader's "2 of 4". Moving onto a
  placement docks the panel there, as a radio's choice follows focus.
  Close is not a placement, so it is a plain button after the group, and
  no arrow closes the panel. The header draws as it did.

### Fixed

- **A clipped node's access rect was its whole box** (backlog F93, from
  the mind map's alpha.19 report). A node panned 28 px under the
  toolbar was cut at its `clip` canvas's edge (F90) and a click there
  missed it, but its access node still read `y=17.7 h=36.6`, so a
  reader's highlight was drawn over the toolbar. It was every clipped
  node, not only a `clip` float: a row half out of a scroller and a
  child of a `clip` box too. The runner hands the rect to AccessKit as
  the node's bounds, and AccessKit's hit test reads raw bounds, so
  VoiceOver's mouse-over, Narrator's hover and a Windows
  `ElementFromPoint` could name a clipped node over whatever was drawn
  there. `access::build` now cuts every rect to the clip the node's hit
  region carries: a `clip` float takes its parent's, and a float that
  escapes keeps its whole box, as it draws and is hit. A node wholly
  clipped keeps its place in reading order and its actions (a reader's
  "scroll into view" goes by key) on a zero-size rect at the clip's
  edge nearest it, so it is never a hit. An editor's text runs are cut
  the same way, a field's also across its own content box as its
  glyphs are (F41), and each character's position moves with the run's
  x, so it still lands where the character is drawn. AccessKit's own
  `clips_children` was not used: an escaping float is the clipper's
  semantic child, and a plain `clip` box is elided from the access
  tree, so there is nothing to set it on. The access cache hashes the
  cut rect, so a clip that moved rebuilds the tree.
  The corpus's `node` lines carry the rect as f32 bits, which pins it in
  all four bindings over every scene, and the new `clip-access` scene
  has a node straddling a `clip` canvas's edge, one wholly past it, one
  that escapes, and a row half out of a scroller and one wholly out.
  *What you can delete:* nothing; a reader was the one misled.
- **Two reference docs behind the code** (backlog F95, from the mind
  map's and the pomodoro's alpha.19 reports). The `line` and `polygon`
  rows, and the `line` element's JSX doc, set a stroke against "a
  declared float, which escapes every ancestor's clip", which F90 made
  false: a float that declares `clip` with a parent anchor is held by
  the parent's clip too. They now say that. The `role` row says that a
  `radio` belongs in a `radioGroup` and a `tab` in a `tabList`, and what
  the pair buys: one Tab stop, the arrows, Home and End moving the
  choice, and "2 of 3" read to a screen reader (ADR 0007).

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-26 —
four commits after the alpha.19 tag, the alpha.19 upgrade reports'
F93–F96. This round ran on Windows and on Linux under WSL 2; the Mac
was not in it, so the AX audit was not run for this tag. What follows
is what executed on what.

**Windows 11 Pro (26200, RTX 5080, 239.76 Hz, 150%), rustc 1.98.1,
Node 25.2.1**, on `main` at `15120d5`. `cargo fmt --all --check` and
`cargo clippy --workspace --all-targets -- -D warnings` are clean.
`cargo test --workspace`: **1394 tests over 104 suites, 0 failed** (2
ignored, the devtools' `drive.rs` and kui-native's `windows_console`
doc examples), from alpha.19's 1381 over 103 on this machine. The
scene corpus runs in all four adapters against one reference report:
Rust and Lua through `cargo test`, C through `target/debug/conformance`
(**45 scenes**, the header at **390 fields, 269 enum members and 250
prototypes**, the ABI still **19**), Node through `npm test` (**190 of
191**, the RTLD test skipping on Windows as it does, the frame still at
**version 16**). The C round, `cbuild --run`, passes its six checks,
and the no-ABI panel is refused as "this build is 19". `npm run gen`
leaves no diff but the checkout's line endings; `npm run typecheck` on
`examples/node` is clean. The headless round, `smoke -- --headless`,
passes all **32 drives**.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **38 Rust examples and the eleven Node examples, each on both
bases, 120 frames each, every one exiting 0 with nothing on stderr** —
98 windows. The C and Lua hosts by hand under `KUI_SMOKE_FRAMES=120`,
after a fresh `cbuild` (W16): `counter`, `host`, `c_panel` and
`lua_panel` each opened a window and exited 0 with nothing on stderr —
**102 windows over five hosts.**

**The release's two runner-facing fixes, in a real window.** F94: a
`runWindowed` app ticking once a second read 21–24 pumps, **0 woken**
and 1 frame in every idle second; the two seconds in which
`SetCursorPos` swept the pointer across it read 48 and 52 pumps, 13 and
4 woken, 13 and 5 frames. F93, through UI Automation: in a 180 px
scroller of 40 px rows, the row half past its edge reads a
`BoundingRectangle` 30 px tall (20 logical at 150%), the rows wholly
past it read `Empty`, and the rows inside read whole.

**Ubuntu 24.04 under WSL 2 (WSLg, llvmpipe), rustc 1.96.1, Node
25.2.1**, on a clone of the same commit: `cargo test --workspace`,
**1392 tests over 103 suites, 0 failed** (1 ignored); **191 of 191
Node tests**; the C round's five checks; all **32 headless drives**;
and the windowed round on X11, **38 Rust examples and the eleven Node
examples on both bases** — 98 windows, every one exiting 0. Not
covered, as in alpha.19's round: a Wayland session other than WSLg's,
and the five prebuilds, which CI cross-compiles.

**The bench guard** against the alpha.19 tag, on this machine rather
than the M3 Pro the README's table comes from. Both runs ended
INCONCLUSIVE by the script's rule — the desktop's noise put four
guarded rows, then one, over the 10% run-to-run spread — but every
guarded row read clean in one of the two and none regressed:
`deep_nesting_64_levels` −0.6%, `frame_10k_rects` +3.2%,
`frame_10k_rects_with_access_tree` −1.6% (F93's cut is on this path),
`frame_10k_rects_with_text_and_hits` −0.8%, `frame_10k_segments`
−2.3%, `frame_1k_curves` +0.2% and `list_10k_rows_virtual` +2.4% from
the second, `frame_1k_typical` +0.7% (±2.2%) from the first. The
README's table keeps its macOS numbers.

## 0.1.0-alpha.19 (2026-09-26)

**What breaks.** The ABI at 19 and the frame at v16. Nothing an
existing input draws changes: a stroke is clipped as it was.

- The runner crate `kui` is `kui-native`, since `kui` on crates.io is
  another crate's: depend on `kui-native = { version = "…", registry =
  "forgejo" }` and write `kui_native::` where `kui::` was, or keep every
  path as it is with `kui = { package = "kui-native", version = "…",
  registry = "forgejo" }`. Its directory is `crates/kui-native`, and
  `cargo run -p kui-native --example …` runs an example. The other
  crates and the npm package keep their names.
- `KuiSpec` gains `float_clip` at its end (ABI 19). Recompile. A zeroed
  field is the float that escapes, which is what every float was.
- `FloatConfig::build` takes a seventh argument, `clip`, after `fit`.
- `FloatConfig` has a new public field, `clip`. A struct literal that
  names every field and has no `..` stops compiling.
- The binary frame's float stanza (v16): its last slot, `fit` as 0 or 1,
  is now a flags word, 1 `fit` and 2 `clip`. Only a hand-written
  encoder sees this; the package's encoder and addon move together.
- A control with a `tooltip` — the prop on any node, or the stock
  button's — keeps its accessible name while the pointer is over it,
  where the name gained the hint's text (`SKIP` read `SKIP KEY S`), and
  the hint is no longer a text node of its own in the access tree, where
  a group read it after its description (under Fixed, F88). A control
  named from its content no longer takes text from under a `role="none"`
  node inside it. A test that asserted the hovered name, or found the
  hint's text in the tree, reads the name and the description instead.
- `widgets::virtual_column` is `widgets::uniform_list`; Lua's
  `virtual_column` is `uniform_list` and its options class
  `kui.VirtualColumn` is `kui.UniformList`; Node's `virtualColumn` is
  `uniformList` and `VirtualColumnProps` is `UniformListProps`.
- `widgets::virtual_rows` is `widgets::list`.
- `KuiSpec` gains `aspect_ratio` after `float_clip`, and `mixed`,
  `value_step` and `on_change` after that, under the same ABI 19; the
  struct is 600 bytes on 64-bit targets, where 18's was 576. The frame's
  three new ops (`toggle`, `slider`, `radioGroup`) join v16.
- Rust: `AccessSpec` (`mixed`, `value_step`), `EventSpec` (`on_change`),
  `AccessNode` (`mixed`, `step`) and `HitRegion` (`slider`) have new
  public fields, so a struct literal that names every field stops
  compiling. `Core`'s internal `nudge` takes a `SliderMove`.
- Every window keeps two frames queued ahead of the one on screen where
  it kept one, and on macOS 14+ the Rust and C runners start frames that
  run back to back at the display's vsync (under Changed, C47): smoother
  at light load, and no slower. A Node window is not paced, so it has a
  frame of latency more while frames run back to back.
  `frame_latency(1)` (`frameLatency: 1`, `KuiRunConfig.frame_latency =
  1`, `KUI_FRAME_LATENCY=1`) with `KUI_FRAME_PACING=0` is the old
  behaviour. On Windows a window keeps one, as it did (RG46): there one
  already delivers every vsync.
- A slider's access node supports `setValue` beside `increment` and
  `decrement` (RG42), and a slider without `onChange` can hear an
  `access` event whose `action` is `setValue`, with the number as
  `value`. A handler that matched only the two nudges ignores it, as it
  ignored the request before.
- `conformance::Expect` (behind the `conformance` feature) has a new
  field, `segments_follow_text` (RG38).
- `KuiRunConfig` gains `frame_latency` under the same ABI 19 (40 bytes,
  was 36).
- Node: `CoreMsg` gains `ChangeMsg`, so an exhaustive `switch` over it
  wants a `'change'` arm; the access tree's nodes gain `mixed` and
  `valueStep`.
- Rust: `Align` has four more variants (`SpaceBetween`, `SpaceAround`,
  `SpaceEvenly`, `Baseline`), so a `match` on it without a wildcard arm
  stops compiling. `LayoutSpec` has an `aspect` field, which a struct
  literal must now name (`..Default::default()` covers it).
- A `KuiSpec.main_align` / `cross_align` of 3 to 6, a value past
  `KUI_END` that used to lay out as `KUI_START`, now means one of the new
  alignments.
- Rust: `InputEvent` has a new variant, `Files`, the answer to a file
  dialog, so a `match` on it without a wildcard arm stops compiling. Node:
  `CoreMsg` gains `FilesMsg`, so an exhaustive `switch` wants a `'files'`
  arm.

### Added

- **File dialogs** (backlog C51, from the second bake-off's table: gpui
  has them, iced and kui did not). An app asks for the platform's Open,
  Save or folder dialog and hears the answer as one event, `{kind:
  "files", paths, tag}`. `paths` is shaped as a `drop`'s, so one handler
  takes both, and it is empty when the user cancelled.
  - The asks: `ui.request_files(FileDialog::open().multiple().filter(…))`
    in Rust, `ctx.requestFiles({mode, multiple, title, filters,
    directory, fileName, tag})` in Node, `env.request_files{…}` in Lua,
    and `kui_request_files(ctx, &KuiFileDialog, tag)` in C.
  - One dialog at a time, as a paste ask is: a second while one is out
    is dropped. `awaiting_files` reads the state.
  - The answer goes to whoever asked: the host, or the extension whose
    fill asked.
  - The runner shows the dialog through rfd (the new default-on `dialogs`
    feature; on Linux through the XDG portal, no GTK). It is rfd's async
    panel, made on the loop's thread and waited on by a thread of its
    own, which posts the answer back through the event loop, so the loop
    never blocks in a modal. On macOS it is a sheet on the window that
    asked.
  - Without `dialogs`, every ask is answered at once with no paths.
  - A host driving its own window takes the ask with
    `take_file_requests` / `takeFileRequests` / `kui_take_file_request`
    (then `kui_file_request_filter` per filter) and answers with
    `InputEvent::Files` / `ctx.answerFiles` / `kui_input_files`.
  - C gains the [in] structs `KuiFileFilter` and `KuiFileDialog` under
    the pending ABI 19.
  - The drop example, Rust and Node, has an "Open…" button whose picks
    land in the same list as a drop. It was checked in a real window
    both ways, and its headless drive takes and answers the ask.

- **Typed messages in Rust: `#[derive(Message)]`** (backlog C50, from
  both bake-offs: "typed Rust messages: no, a `Value` payload"). A new
  crate, `kui-derive`, re-exported by `kui-native` behind a default
  `derive` feature. Derive it on an enum and each variant is a `{kind,
  …fields}` payload, its kind the variant's name in snake_case
  (`#[message(kind = "…")]` renames it). `on_click(Msg::Save)` builds
  the payload, and `ev.message::<Msg>()` reads it back for an exhaustive
  `match`, from a click's payload or from the `tag` inside a drag,
  change, scroll or drop event.
  - Fields may be the numbers, `bool`, `String`, `Option` (absent is
    `None`), `Vec`, `Value`, or other messages, through the new
    `MessageField` trait.
  - An enum of unit variants marked `#[message(string)]` is a bare
    string (`dir: SplitDir` is `"h"`).
  - `MessageError` says what did not fit.
  - The payload is the same plain data, so the other bindings read it
    unchanged.

  The syn it builds on was already in a windowed app's tree, so the
  derive adds its own few hundred lines to a cold build and no new crate
  besides itself. The splitmux example moved onto it, and its headless
  drive now clicks a pane, a tab and the `+`, and drags a divider.

- **The variable-height list in JSX and Lua** (backlog C46). `list(ctx,
  { key, heights }, measure, row)` in Node and `list(env, { key, heights },
  measure, row)` in Lua are `widgets::list`: rows of no fixed height (a
  chat, a log whose lines wrap), built a screenful at a time. The core's
  own `RowHeights` is the app's: `new RowHeights(rows, estimate)` /
  `row_heights(rows, estimate)`, kept in the model or a script global,
  with `setLen`/`set_len`, `clear`, `offsetOf`/`offset_of` and the rest.
  `measure(i, width)` runs only for the rows a frame builds that have no
  height yet, typically over `ctx.measureText` / `env.measure_text`.

  The arithmetic is not ported. `widgets::list`'s slicing became a
  stepping API on `RowHeights` (`slice`, `ListSlice::unmeasured` /
  `reslice` / `finish`, `ListReading`, `ListPlan`). `widgets::list`
  drives it in Rust, and each binding's `list` is the same loop around
  the app's callback. So the prefix sums, the moving estimate, the anchor
  that keeps the row under the pointer still, and RG18's second anchor
  for a glide exist once.

  The correction is a new verb in all four bindings: `Ui::shift_scroll`
  (now public), `Core::shift_scroll`, `shiftScroll`, `env.shift_scroll`
  and `kui_shift_scroll`. It moves a scroll by content that moved under
  it, with no ease asked or ended and no frame requested. A list composed
  by hand calls it too.

  Each port is pinned by the Rust suite's two tests, replayed: the row
  under the pointer stays put while the estimate moves, and a long glide
  lands on the row asked for. Both fail with the correction taken out.
  The Node `virtual_list` example has a `--variable` mode, and its
  headless drive checks the variable list on every smoke round.

- **Where the free space goes, and what lines up** (backlog C13, parked
  since 2026-09-03 and named by both bake-offs against gpui and iced).
  `mainAlign` takes `spaceBetween`, `spaceAround` and `spaceEvenly`
  beside start/center/end, CSS's `justify-content`: the main axis's free
  space is dealt out between the children, around each, or into equal
  gaps and ends, on top of `gap`, per line in a wrapping row, and not at
  all when nothing is free (a `grow` child took it, or the run
  overflows). A lone child starts under `spaceBetween` and centres under
  the other two. `crossAlign="baseline"` on a row lines up the first
  baselines of the children's text, so a 13 px label and a 32 px value
  read as one line: a child's baseline is the first line of the first
  text down its first-child chain, measured by the text system where the
  glyphs are drawn; a child with no text aligns by its bottom edge, a
  `grow` or percent height fills the line from its top, and a fit-height
  row grows to hold the aligned children. Baselines are measured only on
  a frame that declares a baseline row. The same values in every
  binding: the `ALIGNS` rows grew at their tail, so Lua's
  `main_align = "spaceBetween"`, Node's `mainAlign: 'spaceBetween'` and
  C's `KUI_SPACE_BETWEEN` .. `KUI_BASELINE` are the next indices. A value
  on the axis where it means nothing — a spread across, `baseline` along
  or on a column, either as a float's attach point — lays out as start
  (the centring spreads as centre) and warns `align-ignored`.
  *What you can delete:* the `<box width="grow"/>` spacers between
  children that were standing in for `space-between`, and the padding a
  view nudged onto a small label to sit it near a large one's baseline.
- **`aspectRatio`** (backlog C14, CSS's `aspect-ratio`). Width over
  height on any box, image or fragment, sizing the axis left `fit`: a fit
  height is the final width over the ratio, so `width="grow"
  aspectRatio={16/9}` keeps its shape as the window resizes, and a fit
  width under a fixed height is that height times it. The derived axis
  is neither shrunk nor fitted to the children (`minHeight="fit"` floors
  it at them); on an image it wins over the pixels' own aspect. With both
  axes declared, or a fit width under a `grow` or percent height, it has
  nothing to set and warns `aspect-ignored`. Lua `aspect_ratio`, C
  `KuiSpec.aspect_ratio`.
  *What you can delete:* the `layout` event round trip that read a box's
  width to set its height a frame later.

Both are in the new `align` corpus scene, run by all four adapters, and
in `examples/rust/features/align.rs`.

- **Stock checkbox, radio group, switch and slider** (backlog C45,
  [ADR 0034](docs/adr/0034-stock-controls-over-the-roles.md), from both
  bake-offs' "roles only, you draw"). Each is drawn from the state the
  view declares and holds none of its own. A toggle — `<checkbox>`,
  `<radio>`, `<switch>`, their Lua tables, `kui_checkbox` /
  `kui_radio` / `kui_switch`, `widgets::checkbox` / `radio` / `switch`
  / `toggle_with` — is a box, a circle or a track drawn from `checked`,
  its label beside it, and posts its `onClick` when the pointer,
  Space, Enter or a reader presses it. The new `mixed` row is the
  select-all box over a partial selection: a dash, read as mixed. A
  `<radioGroup>` (`radio_group`, `kui_radio_group_open` … `kui_close`,
  `widgets::radio_group` / `radio_group_with`) is one Tab stop whose
  arrows move the choice and press the radio they land on. A
  `<slider>` (`slider`, `kui_slider`, `widgets::slider` /
  `slider_with`) that declares the new `onChange` has the core do the
  arithmetic every app did half of. A press proposes the value under
  the pointer and a drag each new step. The arrows and a reader's
  increment move one `valueStep` (the new row, a hundredth of the range
  unset), PageUp / PageDown move ten, and Home / End go to the ends.
  Every value is clamped and snapped, in the decimal the step names,
  and arrives as `{kind:"change", value, phase:"move"|"end", tag}`.
  The view declares it back as `valueNow`. Sizes follow the metrics
  (the box is the control text plus one), colours the theme. The rows
  are closed, as the button's are: a paint row on a control is an
  `unknown-prop` warning. A slider without `onChange` keeps the
  `access` nudge unchanged. The new `stock-controls` corpus scene is
  replayed by all four adapters, and `examples/rust/widgets/controls.rs`
  and `examples/node/widgets/controls.tsx` show the controls. The
  accessibility and focus examples use them now in place of the ones
  they drew; the platform audit is unchanged at 106/106.
  *What you can delete:* a hand-drawn checkbox, radio, switch or
  slider; the `drag` arithmetic that turned `x` and the parent rect into
  a value; the increment / decrement handler that stepped, clamped and
  snapped it; and the float noise a step of `0.1` left in the model.

- **A float can take its parent's clip** (backlog F90, from the mind
  map's alpha.18 report). `float={{ anchor: 'parent', clip: true }}`
  (Lua `float = { anchor = "parent", clip = true }`, C `float_clip`,
  Rust `FloatConfig::parent().clipped()`) cuts a parent-anchored float
  at its parent's clip, as a child is cut, and its hit region with it.
  The case is a node on a `clip` canvas panned under the toolbar above
  it: it stops at the canvas's edge instead of drawing over the toolbar
  and taking the toolbar's clicks. alpha.17 had already cut the `<line>`
  connectors between the nodes there (F78), and there was no way to ask
  the same for the nodes. The bit is read only with the parent anchor
  (`below` and `above` anchor to the parent). A viewport-anchored float
  escapes whether it is set or not. Paint order does not change: a
  clipped float is still a layer over its in-flow siblings and is only
  cut. F78's rule is now this bit, which the core sets on every `line`
  and `polygon` in its parent's box (ADR 0010, amended).
  *What you can delete:* a hit test an app ran against its canvas's
  rect to ignore a press on a node past the edge, and a node hidden,
  or a pan clamped, so that nothing reached the toolbar.

- **Where the app is in its window, as a rect** (backlog F92, from the
  pomodoro's alpha.16 wish 5, carried in alpha.18). Node's
  `ctx.hostArea()` / `win.hostArea()`, Rust's `Core::host_rect()` and
  C's `kui_host_rect` answer `{x, y, w, h}` in logical px: the rect the
  last frame laid the app out in, which is `env().viewport` with its
  origin — `x` is the devtools pane's width under a left dock, `h` the
  window less the strip under a bottom one, and the whole window with
  the panel off, in its own window, or in any window but the main one.
  F43 made every size reading what the dock leaves; the origin reached
  only the Rust runner, through `devtools_inset`, and `quads()` is the
  whole display list, so a smoke test run with `KUI_DEVTOOLS=1` could
  fit itself to the host area and not say that nothing of its own left
  it. The quads are physical px, so the filter is the rect times the
  scale: a quad inside it is the app's, one outside the dock's — all
  but the root's `bg`, which is the window's background as well as the
  app's and under a dock fills the whole window beneath the pane. The
  frame's reading, like `env().viewport`: zeros before the first frame,
  where a window's `size()` answers. Lua has none — a script lays out in
  its own viewport and reads nothing back. A new C function, no break
  of its own under ABI 19, writing the `KuiLayoutRect` `kui_layout_of` already had.
  *What you can delete:* the dock's width, hard-coded or worked out
  from `size()` against the window's, wherever a test found where the
  app starts.

- **A headless `Ctx` knows its size before its first frame** (backlog
  F91, from the mind map's alpha.18 report). `ctx.size()` answers the
  `WindowSize` `win.size()` does, so the `S` that `init`, `view` and
  `update` are handed reads the same under `createApp` as under
  `runWindowed`. Before the first frame it is the `width` / `height` /
  `scale` `createApp` will frame at, 800×600 at 1 unless its options
  say otherwise; after a frame it is `env().viewport`, that frame's
  size less a docked devtools pane. `init`'s doc had promised the
  function form "the real `size()`" since alpha.14, and under
  `createApp` there was none: `Ctx` had no `size()`, the options'
  `width` / `height` reached only the frame call, and
  `env().viewport` is 0×0 until a frame establishes it, with no
  `resize` after the first. A `Ctx` the app framed itself before
  handing it to `createApp` answers that frame's size until the loop's
  first frame replaces it; a bare one nothing framed answers 0×0, as
  `env().viewport` does. `init`'s and `view`'s docs now say which
  answer a headless loop gets.
  *What you can delete:* a fallback size a headless test's `init` fitted
  its first model to because `size()` was not there, and the passing of
  `createApp`'s `width` / `height` into the model by hand to reach
  `view`.

### Fixed

- **A hovered control's accessible name read its tooltip** (backlog
  F88, from the LCARS pomodoro's alpha.16 and alpha.18 reports). The
  `tooltip` prop floats its hint as the node's last child while the
  pointer is over it, and a role named from its content — a button, a
  box with `onClick` — joins every text inside it, the hint's among
  them: a box reading `SKIP` was `SKIP KEY S` under the mouse, the
  stock `<button tooltip>` `Go Starts it`, and a screen reader re-read
  the changed name each time the pointer crossed a control. The report
  said focus; it was hover, which the headless `click()` leaves on the
  control. The hint is drawn under `role="none"` now, and a name from
  content skips what is under `role="none"`, as a custom editor's text
  already did. The hint's string is where it always was, the
  description, and nothing else reads it; a stock button handed a hint
  with no description on its spec takes the hint as one. The `tooltip`
  *element* (`<tooltip>`, `kui_tooltip`, `widgets::tooltip`) is
  unchanged: it is drawn with no description behind it, so its text
  stays content.
  *What you can delete:* a `label` an app set on a control only so a
  hover would not rename it.

- **Five Node and reference docs described what the code stopped doing
  a release or more ago** (backlog F89, from the alpha.14, alpha.16 and
  alpha.18 upgrade reports). `cursorShape()` and the npm README said an
  `onClick` or `focusable` node gets the hand and an `onDrag` node the
  grab; since alpha.14 only the I-beam is implied, and both now say so
  as `props.md`'s `cursor` row does. `frame()` said it resolves once
  the next pump has painted; it resolves after the next pump, drawn or
  not, and `win.frameStats().framesTotal` moving is the paint (the
  `index.d.ts` doc, `index.js` and two howto answers). `FrameTiming.frames`
  said it climbs to 120 in two seconds; it is the ring's fill, one per
  painted frame, so an idle window stays below it. The `line` and
  `polygon` rows, and their JSX docs, say a stroke in its parent's box
  is held by the parent's clip (F78) where they said only "always a
  float". The howto's one-font answer says why a baseline compared
  across machines needs it: headless text is shaped against the
  machine's installed fonts.

- **A frame of plain boxes cost 5.8% more than alpha.9's** (backlog
  C48, from the second bake-off: 60.0% against 56.8% CPU at 40,000
  boxes, on one OS). Bisected over every tag and then every tenth
  commit, `frame_10k_rects` rose in steps, 743 µs at alpha.9 to 786 µs at
  alpha.18 and 807 µs on this branch. Each step was an inlining decision
  flipped by a change elsewhere:
  - AR29/AR30's text-only code;
  - C13's layout additions, after which `shadow_quad` was inlined into
    `emit_node` and cost every node two more saved registers.

  Rebuilt with every function aligned to 64 bytes, the steps stayed,
  so it is not code placement.

  Three changes bring the row to 755–761 µs, and
  `frame_10k_rects_with_text_and_hits` from 1375 µs back to alpha.18's
  ~1300 µs:
  - `emit_node` keeps only a plain box's path, and the hit regions,
    what a leaf draws, and shadows are out-of-line calls;
  - `layout::wraps` is always inlined;
  - `set_axis_clamped` is inlined and borrows the spec it used to copy.

  The bench guard against alpha.18 passes on all eight guarded rows,
  and the conformance dump is byte-identical. Nothing changes in what a
  frame draws.

- **An app's edit-compile loop paid for the whole runner** (backlog
  C49, from the second bake-off: a release `touch main.rs` rebuild of
  the counter was 1.57 s against alpha.9's 1.20 s). The runner was
  generic over the app, `Shell<A>` and `PumpRunner<A>` alike. So every
  app crate compiled and optimised again the event loop and every
  feature added to it, and its drop glue, on every edit: the app
  crate's IR grew 26% between the two tags.

  The runner is now written against `Shell<dyn App>` and compiled once
  in kui. The one generic step boxes the app, and a pumped runner keeps
  its typed `app_mut` without a cast. The counter's release rebuild is
  0.85 s (alpha.9 1.22 s, alpha.18 1.59 s), and its debug rebuild
  0.54 s (0.63 / 0.65 s). The app crate's IR is 58% under alpha.9's.

  The public API is unchanged, and no `'static` bound was added: an
  app that borrows still runs.

- **Windows, from the regression round of 2026-09-26** (backlog
  RG38–RG46: the Windows halves since alpha.16, cross-compiled and
  linted on a Mac, run on a Windows machine for the first time):
  - *A Node or C app's windows did not get the executable's icon*
    (RG41). `icon_resource` was looked up in the module winit is linked
    into — `kui_node.dll`, `kui_ffi.dll` — which has no resources, so the
    windows fell back to the pixels however plainly `node.exe` or the
    host carried the icon. It is loaded from the executable now, for all
    three.
  - *A slider ignored UI Automation* (RG42). Windows' RangeValue
    pattern moves a slider only by setting it — it has no increment —
    and the request was dropped while the slider read as writable. A
    set value is now proposed as `change`, snapped and clamped, and a
    slider without `onChange` hears `{kind: "access", action:
    "setValue", value}`.
  - *A multi-select Open answered with a file that is not there* (RG43).
    rfd's multi-select and folder panels drop the options that make
    Windows refuse a typed name that does not exist; an Open or folder
    answer now carries only paths that exist.
  - *A Rust `FileDialog::filter` with a dotted extension listed nothing*
    on Windows (`*..txt`, RG44); the dot is dropped, as the other doors
    drop it.
  - *A window waiting for a device built 64 views a second* (RG40): the
    Windows animation timer kept asking while RG29 had stopped every
    other ask. It is off until the device is back.
  - *A minimized animating window drew every frame* at the display's
    rate into a surface nobody saw (RG45); on Windows it asks for none
    until it is restored, where the animation picks up at its clock.
  - Two tests that failed on Windows and nowhere else: the `underlines`
    scene's segment count, which follows the installed fonts and is a
    lower bound in the checked-in half now (RG38), and the Node F55 test
    under Git for Windows' CRLF checkout (RG39).

- **A main window that cannot open is an error, not a panic** (backlog
  RG47, from the Linux round under WSLg). `run` and `open` return why
  — `cannot open the window: …` or `cannot draw in the window: …` — and
  a Node window's constructor throws it, where a panic inside the addon
  aborted the process (`failed to initiate panic`). The loop is parked,
  so the next window can be tried. README now says what a Linux build
  and window need, the libraries loaded at run time among them (RG48).

- **`widgets::window_buttons` alone was 0 px tall** (backlog RG50). In
  a row that fits its content, the cluster's grow row and buttons added
  nothing to the row's height, and the glyphs hung out of a 12 px pill.
  The cluster is a titlebar tall where nothing gives it a height, and
  as tall as the strip inside `titlebar_with`, as before.
- **A custom-chrome window on Windows 11 is rounded and bordered** like
  every other window (RG51), where `Chrome::Custom` had drawn it square,
  flat and shadowless. Maximized, it fills the work area exactly, and
  the caption hit-testing is unchanged.
- The Node relaunch example's custom-chrome window draws its titlebar
  along the window's top edge, with the devtools docked below it (RG52).

### Changed

- **A window keeps two frames queued, and every vsync gets one**
  (backlog C47, from the second bake-off: "kui and iced miss vsync when
  there is little to draw", 105–119 fps where gpui held 120.0).
  Measured on an M3 Pro under macOS 27 on AC power:
  - With one queued frame, the old setting, runs of 10 s drew 112.5–118.6
    fps at 100 and 2,500 boxes (0.8–6.2% of vsyncs missed) and 119.7 at
    40,000. That is the report's
    pattern: the frame's work was 0.3–0.9 ms, and a thread that woke a
    little late after sleeping ~7.9 ms found no free drawable.
  - With two, every run delivered 1198–1201 of the ~1200 vsyncs, in
    three rounds and through the new default as well as the override.
  - gpui gets the same from `maximumDrawableCount(3)` at the rev the
    bake-off ran. wgpu makes kui's `desired_maximum_frame_latency` the
    Metal layer's drawable count less one, so 2 is gpui's setting.
  - The report's other lever, the present mode, does nothing here:
    wgpu's Metal `AutoVsync` is `Fifo`.
  - The second queued frame alone cost a frame of latency while frames
    run back to back. A frame built as soon as a drawable freed waited
    out a vsync in the queue. Measured from the moment a frame sampled
    its state to the moment it was on screen (a ScreenCaptureKit
    capture decoding a timestamp the frame drew), that is 27.5–27.9 ms,
    against 19.2–19.5 with one queued frame.
  - So on macOS 14+ such frames now start at the display's vsync, from a
    `CADisplayLink` on the window's view (`kui_native::pacer`), which is how
    gpui runs. The queued slot is slack and not a delay: 17.5–19.2 ms
    once the window has settled, with every vsync delivered. A frame
    asked for from idle, such as a keystroke, is drawn at once. The link
    runs only while frames are asked for, so an idle window stays at no
    CPU.
  - A Node window turns its loop from a timer, where the link cannot
    start frames (paced, it drew 50 frames a second against 95), so it
    is not paced and keeps the queued frame's cost; `frameLatency: 1`
    trades back.
  `Launcher::frame_latency`, `WindowOptions.frameLatency` and
  `KuiRunConfig.frame_latency` choose per app. `KUI_FRAME_LATENCY` and
  `KUI_FRAME_PACING=0` override without a rebuild. The report saw its
  misses on battery; this change was measured on AC.
  *What you can delete:* nothing an app could have written; a frame
  that missed its vsync was the runner's.

- **The two virtual lists are named for what sets them apart** (from
  the second bake-off against gpui and iced, 2026-09-25). gpui calls
  the pair `uniform_list` and `list`, and the names say the one thing
  a caller has to choose on: every row the same height, or each row
  its own. `virtual_column` said how the uniform list was built, and
  `virtual_rows` said nothing that told the two apart. Arguments,
  behaviour and keys are unchanged, so the rename is a find and
  replace; the examples keep their file names (`virtual_list.rs`,
  `virtual_list.tsx`) and the bench rows theirs, so the bench guard
  still compares against alpha.18. `RowHeights`, which the variable
  list slices by, keeps its name. No door and nothing of the ABI or
  the frame: C never had either list, and composes one from
  `kui_scroll_geometry` and `kui_row_count`.
  *What you can delete:* nothing; this one only costs a rename.

- **The runner crate is `kui-native`** (from the distribution decision
  of 2026-09-26). The name `kui` on crates.io is another crate's, and
  crates.io is where the runner goes at the beta, so it takes the name
  it can keep now, while an alpha may still rename. The crate, its
  directory and `cargo run -p kui-native --example …` change; its
  modules, types and behaviour do not. `kui-derive`, new this release,
  is published beside it. The Forgejo registry keeps `kui` at
  alpha.18 and earlier. The other crates and the npm package keep
  their names. How to move an app over is under What breaks.
  *What you can delete:* nothing; this one only costs a rename.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), on 2026-09-26 —
twenty-one commits after the alpha.18 tag: the upgrade reports'
F88–F92, the second bake-off's C13, C14 and C45–C51 with the two
renames, and the Windows and WSLg rounds' RG38–RG52. What follows is
what executed on what.

**Windows 11 (RTX 5080, 239.76 Hz, 150%) and Ubuntu 24.04 under WSL 2
(WSLg, llvmpipe)**, on the Windows round's branch (`e7b85c1`, merged
unchanged as `f1a4caa`). On Windows: **1381 tests over 103 suites**,
`cargo clippy -D warnings` and `cargo fmt` clean, the C round, **32
headless drives**, **189 of 190 Node tests** (the RTLD test skips on
Windows), `npm run gen` stable, and the windowed round, **38 examples
on both bases with the Node windows**. Probed by hand through user32 and
UI Automation: the accent against Settings, a Rust app's icon resource,
F84's markers both ways, RG31's fault line inside a window callback,
the device reopen, C51's dialogs and C45's controls. On Linux: **1378
tests over 102 suites**, **190 of 190 Node**, the C round and the X11
windowed round, 38 examples on both bases; the X11 icon, the KDE secret
hint and the portal-less dialog held. Not covered: a Wayland session
other than WSLg's (whose Weston crashes under winit's decoration
subsurfaces, filed as theirs), W19's drop position off macOS, and the
five prebuilds, which CI cross-compiles.

**macOS 27.0 (26A428, arm64, Apple M3 Pro), Xcode 27.0, rustc 1.98.0,
Node 26.8.1**, on `main` at `f1a4caa`. `cargo fmt --all --check` and
`cargo clippy --workspace --all-targets -- -D warnings` are clean.
`cargo test --workspace`: **1380 tests over 102 suites, 0 failed** (1
ignored, the devtools' `drive.rs` doc example), from alpha.18's 1324.
The scene corpus runs in all four adapters against one reference
report: Rust and Lua through `cargo test`, C through
`target/debug/conformance` (the header at **390 fields, 269 enum
members and 250 prototypes**, the ABI **19**), Node through `npm test`
(**190 Node tests, 190 passed, 0 skipped**, the frame at **version
16**). The C round, `cbuild --run`, passes its five checks and the
conformance replay, and the no-ABI panel is refused as "this build is
19". `npm run gen` leaves a zero-line diff; `npm run typecheck` on
`examples/node` is clean and the examples lockfile matches. The
headless round, `smoke -- --headless`, passes all **32 drives**.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **38 Rust examples and the eleven Node examples, each on both
bases, 120 frames each, every one exiting 0 with nothing on stderr** —
98 windows. The C and Lua hosts by hand under `KUI_SMOKE_FRAMES=120`:
`counter`, `host`, `c_panel` and `lua_panel` each opened a window and
exited 0 with nothing on stderr — **102 windows over five hosts.** The
AX audit against `accessibility`, whose controls are the stock ones
now: **106/106**.

**The bench guard** against the alpha.18 tag, on a quiet machine: worst
guarded spread 3.4%, every guarded row within its tolerance —
`deep_nesting_64_levels` +2.8%, `frame_10k_rects` −2.7% (791 → 770 µs,
C48), `frame_10k_rects_with_access_tree` −0.5%,
`frame_10k_rects_with_text_and_hits` +0.9%, `frame_10k_segments`
+3.1%, `frame_1k_curves` −1.7%, `frame_1k_typical` +0.6%,
`list_10k_rows_virtual` +1.9% (its bench touched by the rename). Of
the unguarded rows only `replay_a_full_depart_store` is slower by more
than its run-to-run spread, 15.2 → 16.1 µs (+5.5%, ±3.7%). The README's
table is refreshed from every row whose two runs agree within 5%; the
seven that do not keep their earlier numbers.

## 0.1.0-alpha.18 (2026-09-25)

**What breaks.** No door, the ABI at 18 and the frame at v15. ⌘V
(Ctrl+V) with no editor focused no longer puts the clipboard's text
into the window as typing when the window holds a selection — a
`selectable` scope's or a `cells` grid's (under Fixed, RG37); a focused
list whose type-ahead searched for it no longer does, and a key sink,
which never heard that text, still hears the chord. On Windows the
system accent, and every role a theme derives from it, is the colour
the user picked in Settings, where it was the darker one DWM tints
title bars with (under Fixed) — an app that lightened
`Env::accent` to make up for it now lightens the right colour.

### Added

- **Every window of the app carries its icon** (backlog F86, from
  kawoosh's window-icon report). `kui::app("t").icon(rgba, w, h)` gives
  every window the runner creates — the main one, a declared one, a
  popup — that picture, and `.icon_resource(1)` gives them, on Windows,
  the executable's own icon resource: the `1 ICON "app.ico"` line of
  its `.rc`, which Explorer already drew for the file while the window
  showed the default, because winit registers its window class with no
  icon. The resource wins over the pixels there and each of the title
  bar and the taskbar loads the `.ico`'s frame for its own size; X11
  takes the pixels. macOS draws the bundle's `.icns` and Wayland the
  `.desktop` file's, and neither has a window icon, so an app passes
  both and each platform takes its own. Node: `icon: {rgba, width,
  height, resource}` in `WindowOptions`, `rgba` a `Uint8Array` (a
  `Buffer` is one). C: `kui_set_icon(rgba, w, h, resource)` before
  `kui_run`, as `kui_on_teardown` is — a new function, ABI still 18.
  Pixels that are not the size are refused with the reason
  (`Launcher::try_icon`, the constructor, `false` and stderr).
  *What you can delete:* nothing an app could have written — kui
  created the window, so a window icon was out of an app's reach; a
  platform call reaching for the window's handle after the fact to set
  `WM_SETICON` or `_NET_WM_ICON` goes.

### Fixed

- **Windows' accent came out near black** (found in kawoosh on
  Windows 11). `system_env::accent` read `DwmGetColorizationColor`,
  the colour DWM tints title bars with, which Windows 11 darkens and
  blends on its own terms — `#0C2231` for a `#2C79AD` accent — so
  every role derived from it drew near black, and kawoosh's selection,
  a 40% wash of the accent, drew darker than the page under it. It
  reads DWM's `AccentColor` now, the colour Settings shows and WinRT's
  `UISettings` answers `Accent` with, and falls back to the
  colorization colour only on a profile that never set one. Compiled
  and linted for `x86_64-pc-windows-msvc`, not run here.
  *What you can delete:* a lightening or a hard-coded accent an app
  laid over `Env::accent` on Windows to get the user's colour back.
- **⌘V over a selection sent the clipboard nowhere, and gave a stock
  editor a paste without its markers** (backlog RG37, from the
  regression pass of 2026-09-25). The runner took ⌘V whenever the
  window held a selection and answered it by reading the clipboard and
  sending it as typing: with an editor focused that dropped F84's
  `concealed` and `transient`, and with none it reached no sink — a
  focused key sink hears the raw ⌘V first and pastes by its own
  binding — only a focused list's type-ahead. Now only a focused editor
  takes the runner's paste, asked through `request_paste`, so the
  answer is the menu's `Paste` with the pasteboard's markers; with no
  editor ⌘V is the sink's chord. The fix the backlog first wrote down,
  a `request_paste` for every ⌘V over a selection, pasted twice into a
  sink that binds ⌘V — found in a window before it shipped.
- **A stroke's segments cost 10% more to emit since alpha.14** (backlog
  C41, carried by three tags). The drop-zone commit grew `emit_node`,
  and the segment loop inlined into it lost its carried point to a
  register spill — a store and a reload on the stack for every segment,
  in a loop the commit never touched. `push_segments` is kept out of
  line now, where the loop keeps every value in a register:
  `frame_1k_curves` reads 285 → 254 µs against alpha.17's code, under
  the 261 µs it read before the drop zone, and the row joins
  `scripts/bench-check.sh`'s guard list, so the segment path has a row
  that fails. The bench guard against `main`: every guarded row within
  its noise (worst spread 3.9%).

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run on `main` on
2026-09-25 — the Windows accent fix, RG37, C41 and F86, six commits
after the alpha.17 tag. What follows is what executed on what.

**macOS 27.0 (26A428, arm64, Apple M3 Pro), Xcode 27.0, rustc 1.98.0,
Node 26.8.1.** `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets -- -D warnings` are clean. `cargo test --workspace`:
**1324 tests over 97 suites, 0 failed** (1 ignored, the devtools'
`drive.rs` doc example), from alpha.17's 1319. The scene corpus runs
in all four adapters against one reference report: **41 scenes**, Rust
and Lua through `cargo test`, C through `target/debug/conformance`
(the header at **374 fields, 260 enum members and 238 prototypes** —
`kui_set_icon` new — the ABI still **18**), Node through `npm test`
(**180 Node tests, 180 passed, 0 skipped**, the frame still at
**version 15**). The C round, `cbuild --run`, passes its five checks
and the conformance replay. `npm run gen` leaves a zero-line diff; `npm
run typecheck` on `examples/node` is clean and the examples lockfile
matches. The headless round, `smoke -- --headless`, passes all **31
drives**. The Windows code of the release — the accent read and F86's
window icon, pixels and resource — is compiled and clippy-clean for
`x86_64-pc-windows-msvc` through cargo-xwin (`kui`, `kui-node`,
`kui-ffi`, all targets) and **not run**: the icon in a title bar and
Alt-Tab, and the accent against Settings, are the hand checks left.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **36 Rust examples and the ten Node examples, each on both
bases, 120 frames each, every one exiting 0 with nothing on stderr** —
92 windows. The C and Lua hosts by hand under `KUI_SMOKE_FRAMES=120`:
`counter`, `host`, `c_panel` and `lua_panel` each opened a window and
exited 0, warning-free — **96 windows over five hosts.** The AX audit
against `accessibility`: **106/106**.

**The bench guard** against the alpha.17 tag: worst guarded spread
4.7%, every guarded row within its tolerance — `deep_nesting_64_levels`
−0.6%, `frame_10k_rects` −2.0%, `frame_10k_rects_with_access_tree`
−4.0%, `frame_10k_rects_with_text_and_hits` −1.9%, `frame_10k_segments`
+2.0%, `frame_1k_curves` **−7.9%** (272 → 251 µs, ±0.8%, C41's fix),
`frame_1k_typical` +0.8%, `list_10k_rows_virtual` +2.2% — and every
unguarded row "same". The run opened with the script's load warning,
so the README's table keeps alpha.17's numbers but for
`frame_1k_curves`, the row this release moved.

## 0.1.0-alpha.17 (2026-09-25)

**What breaks.** No door, the ABI at 18 and the frame at v15.
A focused `on_key` sink hears the keyboard while it is drawn outside
its container's clip, where it heard nothing (under Fixed, F79) — an
app that leaned on the silence to keep keys from an off-screen pane
now hears them there and decides for itself.
A `line` or `polygon` inside a clipping or scrolling container is cut
at that container's edge now, where it drew past it (under Fixed,
F78) — a view that relied on a stroke escaping its scroller anchors it
`float="viewport"` or declares it outside.
A windowed app on Windows that prints, launched from a shell, prints
there now where it printed nowhere (under Added) — an app that wrote
its own `AttachConsole` for that can delete it. `code` under Shift on a
layout that does not speak ASCII is the shifted US-QWERTY key — `J`,
`:`, `~` — where it was the unshifted one (under Fixed); a keymap that
matched `j` and read `shift` beside it to tell the two apart reads
`code` alone now.
Two `reveal`s asked before one frame, into different scroll
containers, both land now, where only the last did (under Fixed,
F82) — a view that revealed into one container and relied on a later
reveal elsewhere cancelling it drops the first ask instead.
The runner answers a paste with `InputEvent::Paste`, where it sent
`InputEvent::Commit`, and a view can queue
`MenuAction::SetClipboardSecret` (under Added, F84) — a Rust `match`
that named every variant of either gains an arm; a host that answers
pastes itself with `Commit` (`kui_input_commit`, Node's `commit`)
keeps working unchanged, and the ABI stays at 18.
A scroll container that already declared a `transition` — for its
colours, say — eases a `reveal` or a `set_scroll` now, where it jumped
(under Added, F80), and while it eases `scroll_geometry`'s offset is
where the content is drawn, not the target `scroll_offset` answers; a
view that wants the jump keeps the `transition` off the scroller.
`kui_owed` gains `KUI_OWED_SCROLL` (32) and Node's `owed()` a `scroll`
field for that leg — a host that compared `kui_owed` against a fixed
set of bits sees a new one. The Rust surface gains names a struct
literal or an exhaustive `match` spells out: `Owed::scroll`,
`PropsOut::secure_input`, `RenderError::DeviceLost`, and
`ScrollStore::resolve` takes the container's transition.
On Windows the device is D3D12 alone unless `WGPU_BACKEND` names
another (under Fixed), where wgpu enumerated every backend; and on
every platform a wgpu validation error is printed rather than raised
as a panic — `on_uncaptured_error` replaces wgpu's default handler —
so a check that relied on the panic to fail a run reads stderr.
A C or Node host that hands the core `physical` and no `text` gets the
layout's character typed, where it got the US stand-in's (under Fixed,
RG28): Node's `keyDown("Ж", {shift: true}, ";")` types `Ж`, which was
`;`.

### Added

- **Secure keyboard entry while a window asks for it** (backlog F85,
  from kawoosh's secrets report). `ui.secure_input(true)` / a root
  `secureInput` / `secure_input = true` / `kui_set_secure_input`,
  declared on every frame a password prompt is up, turns on macOS's
  Secure Keyboard Entry while that window has the keyboard, so no other
  process — an event tap, a keylogger — reads the password as it is
  typed. The runner owns the balance: `EnableSecureEventInput` is
  process-wide and counted, and it holds one count only while a window
  whose frame asked has the keyboard, giving it back when the window
  loses the keyboard, closes or stops asking, and at exit, so a frame
  that stops declaring it is all it takes to turn it off. Nothing on
  Windows or Linux, which have no such switch. `Ctx.secureInput()` /
  `kui_secure_input_get` read the ask back — a C host with its own
  window makes the call itself. No ABI change, no frame version.
  *What you can delete:* an app's own `EnableSecureEventInput` /
  `DisableSecureEventInput` pair and the bookkeeping that kept it
  balanced across focus changes, window closes and quitting.
- **A paste says whether the pasteboard marked it a secret, and a
  secret can be copied marked** (backlog F84, from kawoosh's secrets
  report). The answer to `requestPaste()` / `request_paste` /
  `kui_request_paste` — and to a menu's Paste — reaches a focused
  `onKey` sink as the same `{kind:"text", text, tag}`, now with
  `concealed: true` when a password manager marked the copy a secret
  and `transient: true` when it asked that no history keep it, after
  the nspasteboard.org convention 1Password, Bitwarden and KeePassXC
  follow (Windows: the clipboard's exclusion formats). A marker that
  is not set is absent, never false, so a sink that never heard of
  them is unchanged. The runner reads them on macOS and Windows; on
  Linux a paste arrives unmarked for now. In Rust the answer is
  `InputEvent::Paste { text, marks: ClipboardMarks }`, routed exactly
  as `Commit` is — which is still an answer, one nothing marked. The
  other way, `setClipboardSecret(text)` / `ui.set_clipboard_secret` /
  `env.set_clipboard_secret` / `kui_set_clipboard_secret` puts text
  there the way a password manager does — marked concealed and
  transient on macOS, excluded from monitoring, history and the cloud
  clipboard on Windows, KDE's password-manager hint on Linux — as a
  `setClipboardSecret` action (`KUI_MENU_ACTION_SET_CLIPBOARD_SECRET`);
  `setClipboard(text, html)` is unchanged. A C host answers with
  `kui_input_paste(ctx, text, KUI_PASTE_CONCEALED |
  KUI_PASTE_TRANSIENT)`, a Node test with `ctx.paste(text, {concealed,
  transient})`. The devtools' events tab keeps a concealed paste's
  markers and its length, never its text (RG34). The runner needs
  arboard 3.6 for the writes (RG35). No ABI change.
  *What you can delete:* a sink's own guess at which pastes were
  passwords (their length, their alphabet, the app they came from), and
  any platform code of the app's that read `NSPasteboard` types or
  wrote a copy through a second clipboard library to mark it.

- **The Lua DSL described for lua-language-server** (backlog F81,
  from kawoosh's Lua-types report). `kui_lua::luals_meta()` returns a
  `---@meta` file of every prelude constructor — `row`, `column`,
  `grid`, `text`, `edit`, `image`, `virtual_column`, the rest — each
  with the comment above it in the prelude and its parameters typed by
  its element, and a `kui.Props` class of every prop a node table
  takes: the schema's rows by their Lua spelling, each typed by its
  kind (an enum as its names, a sizing as `kui.Sizing`) with its doc,
  the composites (`pad`, `border`, `scroll_x`, …) and each element's
  own props. It is generated from kui-core's schema, the same tables
  `npm run gen` writes `index.d.ts` from, and tests run a value of
  every annotated type through the Lua parser and every value the
  parser takes through the annotation, so the two cannot drift apart
  unnoticed — the regression pass found the first cut offering
  `{ percent = n }` where the parser reads `{ pct = n }`, `repeat`
  (a Lua keyword) where a table writes `direction`, `key` as a number,
  and every constructor's table optional where thirteen index it
  unchecked (RG33). A table `pad` on `virtual_column` no longer
  crashes the prelude. A host writes the file into a directory and puts
  that on the server's `workspace.library`. No ABI change.
  *What you can delete:* a hand-kept `---@meta` file of the prelude,
  and the `diagnostics.globals` list that silenced `row` and `text`.
- **A scroll container eases where a `reveal` or a `set_scroll` takes
  it** (backlog F80, from kawoosh's scrolling-tab report). Declare a
  `transition` on the container and a programmatic scroll glides over
  it instead of jumping — a ribbon the keyboard walks, a list a
  shortcut jumps to, a row revealed from a search. The hand's own
  scrolling is untouched and always lands whole: the wheel, the
  scrollbar's thumb and a drag held past the edge, none of which may
  ever lag a finger, and any of which interrupts a leg in flight and
  takes the content where it stands. `Core::scroll_offset` answers
  where the container is *going*, so a view's arithmetic is unchanged;
  `Core::scroll_geometry` answers where the content *is*, which is what
  a virtual list must slice by, and `Owed::scroll` says a leg is
  mid-flight so a driver schedules the next frame. Mid-leg, a wheel
  notch and another reveal are measured from where the content stands;
  the thumb, the access tree and the inspector show the drawn place;
  `virtual_rows`' own height correction carries the leg with it, so one
  `set_scroll` to a far row lands on that row; and the leg is owed in
  every binding — `KUI_OWED_SCROLL`, Node's `owed().scroll`, which
  `quiet()` waits on (the regression pass, RG17–RG22, found each of
  these in the first cut). No new prop, no ABI change: `transition`
  gains a meaning on a node that scrolls.
  *What you can delete:* an app's own offset tween — the frame-by-frame
  `set_scroll` toward a target, its easing and its clock — and the
  guard that kept the wheel from fighting it.
- **A Windows app with no console window, and its terminal back.**
  A console-subsystem binary — what Rust builds unless told otherwise —
  gets a black console window beside its own the moment it is opened
  from the Explorer, made by the loader before a line of the app has
  run; nothing at runtime can prevent it, only close it after it has
  been seen. The one way not to have it is the app's own line,
  `#![cfg_attr(windows, windows_subsystem = "windows")]` at the top of
  its binary crate (or `/SUBSYSTEM:WINDOWS` on a C host's link), and
  its cost was the terminal: a windows-subsystem process started from a
  shell has no standard handles at all, so its `println!`, its panics,
  the runner's `kui:` lines and `kui_wgpu::report_faults` all went
  nowhere. The runner now attaches such a process to the console of the
  shell that launched it (`AttachConsole`, once, when the shell starts)
  when the process has no output handle of its own — a Rust app's own
  `println!` before it calls `run` still has nowhere to go, and a C
  host's `printf` needs its own `AttachConsole`, since its C runtime
  set stdio up at startup and does not look again; from the
  Explorer there is no parent console and nothing is shown, and a
  process whose launcher piped or redirected its output — cargo, the
  smoke round, `> log.txt` — keeps what it was given. The prompt is back
  before the app's first line, since a shell does not wait for a
  windowed process; a Ctrl+C typed at it is the shell's and the app
  ignores it; and closing that terminal ends the app, as it ends a
  console build — Windows terminates every process on a console it
  closes, and no handler prevents it. Checked with a windows-subsystem probe on kui: from
  a console, its report on the console; through a pipe, in the pipe; from
  a parent with no console, no window and no handles. The how-to's
  "How do I stop the console window on Windows?" has the line.
  *What you can delete:* a hand-written `AttachConsole` at the top of
  `main`, and a log file written only because a windowed build had
  nowhere else to print.

### Fixed

- **A glyph working set between one atlas page and two corrupted every
  frame** (backlog F83, from kawoosh's big-font report). The page
  grew when it filled twice in one frame (AR19), and a set that
  fills it once a frame — a code view at 66 px on a 2× display, some
  fifty glyphs to a 1024 page — never filled it twice: every frame
  reset once mid-emit, the quads emitted before the reset sampled the
  overwritten page, and the rows drawn first (the caret's, redrawn
  every frame) showed other glyphs' pixels. A glyph the last reset
  dropped coming back to a page that fills again on the very next
  frame is thrash, and the page grows; a set that turns over, all new
  keys, still resets, and so does a fill long after the reset that
  happens to want back a common glyph — which the first cut took for
  thrash, doubling the page on every later fill and never giving it
  back (RG23).
  *What you can delete:* a cap on the font size an app lets its user
  pick to keep the glyphs whole.
- **Two reveals in one frame, into different containers, dropped the
  first** (backlog F82, from kawoosh's tab-strip report). `reveal`
  kept one pending key and the last ask won, which is right inside
  one container — two asks there contradict — and wrong across two: a
  window whose tab strip and pane ribbon each reveal their selection on
  the frame a key changes both lost the strip's. The pending reveals
  are a list now, resolved after layout by the container each would
  move (its nearest scrolling ancestor): the last ask per container
  stands, and every container asked about moves.
  *What you can delete:* a reveal asked again on the next frame so it
  survives another one asked later in the same frame.
- **A focused key sink drawn outside its container's clip heard
  nothing** (backlog F79, from kawoosh's scrolling-tab report). A key
  reaches a node by holding focus; a hit region is where a *point*
  finds one. The delivery asked the hit list all the same, and a node
  the frame drew past its scroller's edge is under no point, so it has
  no region there — a pane scrolled off a ribbon, or one an `enter`
  offset or a `slide` had not finished moving, dropped every key typed
  at it until it came back, silently and with focus plainly on it.
  `route_key` and `deliver_to_sink` resolve the sink from the frame's
  tree now (`Core::sink_node`), which is what every other step of the
  walk already read; `disabled` and the modal boundary still refuse, and
  the pointer's rule is untouched — a click past the clip finds nothing,
  as before. ADR 0011 decision 9 says so. *What you can delete:* the
  reveal-before-you-type dance — a frame requested, or an animation
  shortened or dropped, so that a pane the keyboard was given would be
  on screen in time to hear it.
- **A virtual list sliced by a geometry that moved stayed a frame
  behind until the next event** (backlog F77, from kawoosh's
  virtual-list report). `virtual_column` slices by the frame before —
  by design, covered by two rows of overscan — but nothing asked for
  the frame that would close the lag, so a list whose container came
  out of layout taller than the slice assumed (a split sliding open, a
  resize) or scrolled elsewhere (a reveal, a clamp after the rows
  shrank) kept the stale rows on screen: five rows in a pane twenty
  tall, a blank band at the edge under the wheel. The core now
  records what `scroll_geometry` handed each build and, after layout,
  asks for one more frame when a container so read was placed
  otherwise (`ScrollStore::take_resliced`); the frame after builds
  against what is on screen and owes nothing. A read made before the
  frame begins counts for it — Node's view runs between two frames,
  which the first cut missed, so every binding's virtual list gets it
  now (RG24) — and a container read but not laid out, a pane in a
  hidden tab, owes nothing, where it asked for a frame every frame
  until the tab came back (RG25). *What you can delete:* a `request_frame` after a virtual list, or
  a frame requested on every resize, put there so the list would catch
  up.
- **A stroke or a polygon inside a scrolled row spilled past the
  scroller** (backlog F78, the same report). A `line` or `polygon` is a
  float the core makes (ADR 0010, decision 5), and a float escaped
  every ancestor's clip — right for a tooltip, wrong for a graph's lane
  drawn inside a list's row, which stayed on screen over the strip
  above once the row scrolled away while its text was cut. A stroke or
  polygon anchored in its parent's box takes the parent's clip now, as
  a child would, and its hit region with it; a declared float and a
  viewport-anchored stroke still escape. A departing subtree's strokes
  follow the same rule while its exit plays (RG26). *What you can delete:* a
  `clip` node wrapped around a graph, or a check that skips drawing a
  row's strokes once the row is out of view.
- **Shift was lost under the layout fallback: `J` on a Russian layout
  was `j`, and Shift on the key printed `;` was `;`** (backlog F76,
  from kawoosh's Russian-layout report). `KeyPress::from_layout` stands
  in the US-QWERTY key at the position when the layout's key is not
  ASCII, and a window reports that position from a table that never
  sees Shift (F65), so every shifted press on such a layout arrived as
  its unshifted key — a modal editor's `J`, `:`, `>`, `~` and `{` were
  `j`, `;`, `.`, `` ` `` and `[`, and the one key a vim hand on a
  Russian layout reaches for most, `:` on the `;` key, was the repeat
  of a find. The stand-in is now what US-QWERTY prints for the same
  press, Shift included: the upper-case letter, the symbol above the
  digit, the pair on the punctuation key. The layout's own ASCII was
  never touched and still is not (Russian's `:` on Shift+6 arrives as
  itself), and `physical` stays the unshifted position it always was.
  The winit runner's Alt path takes the logical key with every
  modifier stripped, Shift included, and now resolves the fallback the
  same way, so ⌥⇧ on the key printed J is `j` on a Russian layout as
  it is on a US one. That Alt rule lives in the core now, not in the
  runner alone, so a C or Node host that passes `physical` gets the
  same `j` for ⌥⇧ over `Ô` that the runner does (RG27), and a host that
  gives no `text` types the layout's character — `Ж`, not the
  stand-in's `:` (RG28). Caps Lock is not modelled: a Caps-Locked
  non-Latin key stands in as the lower-case letter. ADR 0002 decision
  11 says so. `crates/kui-core/tests/keys.rs` pins seven shifted keys
  of a Russian layout, a dead key under Shift, the Alt rule and the
  two that must not move.
  *What you can delete:* a keymap's second table of shifted symbols
  for non-Latin layouts, and the `shift` check beside a `j` arm that
  told `J` apart from it.

- **A lost GPU device is opened again, and the loss is a real one.** A
  driver update or a GPU reset removes the device under a running app;
  every frame after acquired nothing, the shell printed one validation
  error a frame and the window kept what the compositor last had. The
  device says what happened now (`on_uncaptured_error`, the lost
  callback), `Renderer::render` answers `RenderError::DeviceLost` for a
  dead device, and the shell drops every renderer, opens one new device
  and a renderer for each window on it — the cores stay, the next frame
  draws what the last would have. `KUI_LOSE_DEVICE=SECS` pretends the
  loss, and on Windows it is one: the D3D12 device itself is removed. A
  surface that will not configure is tried three times and then waits
  for the device's own retry, and a device that will not open is tried
  once a second with the loop asleep between tries rather than spinning
  a core (RG29, RG30); the subpixel decision is taken again for the new
  device (RG32). A window's first renderer at launch still panics when
  there is no GPU at all. On Windows `Gpu::new` asks for D3D12 alone
  unless `WGPU_BACKEND` names another — with every backend enumerated
  the process held an OpenGL context and a Vulkan instance it never
  drew with, and under a driver update the driver faulted in present
  rather than report the loss — and `kui_wgpu::report_faults` names the
  code and the module of a fault that would otherwise end the process
  with nothing but `0xC000041D`, from the unhandled-exception filter, so
  an exception a driver or V8 catches for itself is not reported and a
  host's own crash reporter is still called (RG31).
  *What you can delete:* a watchdog that restarted the app when a
  driver update left its window frozen.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run on `main` on
2026-09-25 — the Windows device-loss round, F76–F85 and the regression
pass over them (RG17–RG36, built the day they were filed; RG37 left
open), 22 commits after the alpha.16 tag. What follows is what executed
on what.

**macOS 27.0 (26A428, arm64, Apple M3 Pro), Xcode 27.0, rustc 1.98.0,
Node 26.8.1.** `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets -- -D warnings` are clean. `cargo test --workspace`:
**1319 tests over 97 suites, 0 failed** (1 ignored, the devtools'
`drive.rs` doc example), from alpha.16's 1283 — the pass's own
regressions tests among them, each checked to fail on the code before
its fix. The scene corpus runs in all four adapters against one
reference report: **41 scenes** (`paste` new), Rust and Lua through
`cargo test`, C through `target/debug/conformance` (the header at
**374 fields, 260 enum members and 237 prototypes**, the ABI still
**18**), Node through `npm test` (**179 Node tests, 179 passed, 0
skipped**, the frame still at **version 15**). The C round, `cbuild
--run`, passes its five checks and the conformance replay. `npm run
gen` leaves a zero-line diff; `npm run typecheck` on `examples/node` is
clean and the examples lockfile matches. The headless round, `smoke --
--headless`, passes all **31 drives**. The Windows half of the pass
(RG29–RG31) is compiled and clippy-clean for `x86_64-pc-windows-msvc`
through cargo-xwin and **not run**: a device lost on a real machine,
and whether a `0xC000041D` reaches the unhandled-exception filter
rather than a fail-fast, are the hand checks left.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **36 Rust examples and the ten Node examples, each on both
bases, 120 frames each, every one exiting 0 with nothing on stderr** —
92 windows. The C and Lua hosts by hand under `KUI_SMOKE_FRAMES=120`:
`counter`, `host`, `c_panel` and `lua_panel` each opened a window and
exited 0, warning-free — **96 windows over five hosts.** The AX audit
against `accessibility`: **106/106**.

**Driven by hand: F85 in a window.** A Node window whose root declared
`secureInput`, read through `ioreg`'s `kCGSSessionSecureInputPID`:
held while the window had the keyboard and asked, gone when a click on
its toggle stopped the ask and back when it asked again, gone on ⌘-Tab
to another app, and gone after ⌘Q. Two facts of the OS worth knowing:
while secure input is on, another app cannot take activation
programmatically (`open -a`, AppleScript's `activate` leave the window
frontmost; a click or ⌘-Tab does), and the pid the session reports can
name the app that was frontmost when the count was taken rather than
the one that took it.

**The bench guard**, run alone against the alpha.16 tag once another
session's build had finished (the first run, beside it, read a worst
guarded spread of 138% and was discarded): worst guarded spread 4.9%,
every guarded row within its tolerance — `deep_nesting_64_levels`
+0.3%, `frame_10k_rects` −0.6%, `frame_10k_rects_with_access_tree`
+0.6%, `frame_10k_rects_with_text_and_hits` −1.3%, `frame_10k_segments`
+6.4% (±4.9%), `frame_1k_typical` −2.5%, `list_10k_rows_virtual` +0.4%
— and every unguarded row "same"; `frame_1k_curves` reads 273 → 271 µs,
so **C41** is carried as it was. The README's table is refreshed from
this run except five rows whose two runs disagreed by more than 5%,
which keep alpha.16's numbers; its `long_line`, `stream` and `cells`
rows are not re-run.

## 0.1.0-alpha.16 (2026-09-20)

**What breaks.** Nothing: four doors, a prop, a widget, an element and
a `dir` value added, the ABI at 18 and the frame at v15 (a new op for
the select element; the addon and the JS ship together, and an older
addon refuses the newer encoder by version rather than misreading it;
the table is a value of a prop both already carry). One layout result
moves — a grow child beside a sibling whose `max` held it short now
takes that room (backlog F71 and RG5 under Fixed) — which a view that
padded the hole by hand will see. Two inputs answer differently: an
assistive-technology click naming a node behind a modal is the press
outside now — one `dismiss` with `reason: "outside"` on the modal,
nothing on the node — where it was dropped (RG13), so an app that
closes on any `dismiss` closes on it; and `activate_menu_item` /
`activateMenuItem` / `kui_activate_menu_item` on a row the menu
disabled returns false and posts nothing, where it posted the choice
(RG9).

### Added

- **A table** (backlog F75, from the devtools' own tabs and kawoosh's:
  six key/value lists, each lining its values up behind a label box of
  a width picked by hand — 70, 90, 52, 110, 180 px — each wrong the day
  a longer label arrived). `NodeSpec::table()` / `<box dir="table">` /
  `grid { }` in Lua (`table` is Lua's own) / `dir = KUI_TABLE` in C is
  a column whose rows' children line up in columns
  ([ADR 0033](docs/adr/0033-a-table-is-a-column-whose-cells-align.md)):
  the nth in-flow child of every row is column n (a float in a row is
  not a cell), and a column is as wide as
  its widest cell, so a label column sits at its longest label with
  nothing measured and no width in the view. A cell's `width` sizes its
  column — `fit` and a number are content, `grow` grows the column with
  the table, a percent takes its cut — and its `minWidth` / `maxWidth`
  clamp it; a bare text is a cell held to its column; the rows are rows,
  with their own `gap`, padding, background, hover, click and label; a
  row of a table never wraps (`wrap-ignored` says so). Fit columns that
  overflow the row are compressed largest first, as a row's children
  are, unless the table scrolls x. Resolved inside the five layout
  passes — the fit at the table in pass 1, the columns once in pass 2,
  written into the cells — so it costs a frame without a table nothing
  and needs no widget, which is why it reaches every binding as one
  value of `dir`, with the ABI and the frame version unchanged. A
  `table` scene in the conformance corpus, built in all four adapters;
  `examples/rust/widgets/table.rs` and `examples/node/widgets/table.tsx`;
  the `table` element row in props.md; the devtools' Facts, tokens,
  legend and inspector lists are tables now, and the Tree tab's
  inspector says `column · table` of one.

  *What you can delete:* the fixed-width box around a label that lines
  a list's values up, and the `measureText` that picked its width.

- **The app hears the window go** (backlog F74, from kawoosh closed
  with its red button and with ⌘Q: the session it saves on `:q` was
  not saved, since neither path is a key the app sees — the first
  returns from `run` with the app dropped, the second ends the process
  from `applicationWillTerminate` without `run` ever returning).
  `App::teardown(&mut self)`, called once as the main window goes for
  good — its close button, Quit from the menu or the dock,
  `WindowCommand::Close` on it, a pumped runner ended — before `run`
  returns or the process exits: from the loop's `exiting`, which the
  OS's Quit reaches too, and from a pumped runner's retirement,
  whichever comes first. The place to keep what the app would lose
  with the window; there is no `Ui` by then and nothing draws. The
  default does nothing. `kui-devtools`' `Harness` forwards it, so an
  example's `teardown` runs; `waker` prints once more as it goes.
  Rust first; the C and Node doors came two days later (backlog RG1,
  below) — this entry said the other hosts "own their loops and their
  `pump` returning false is the same moment", which was wrong: on a Mac
  a Quit ends the process inside the pump, and no pump returns.

  *What you can delete:* a `Drop` on the app that saved state as it
  went — which never ran under ⌘Q on a Mac.

- **The C and Node apps hear the window go too** (backlog RG1, from the
  regression pass over F74: a Node app quit with ⌘Q ran nothing after
  `await runWindowed(...)`, not even `process.on('exit')`, and a C host
  nothing after `kui_run` — `applicationWillTerminate` runs the loop's
  `exiting`, which reached a `teardown` neither host had, and the
  process ends there). Node: `teardown(model)` in `runWindowed`'s
  config beside `init`/`update`/`view`, run once with the model as it
  stands, from inside the pump that saw the window go and before
  `runWindowed` resolves — which under ⌘Q it never does, so this is the
  only thing a Node app runs on ⌘Q; `createApp` takes the same field
  and `app.teardown()` runs it, so a headless drive asserts on what the
  app would have kept; under the hood `KuiWindow.onTeardown(cb)` is the
  addon's door, for a driver of its own. C: `kui_on_teardown(fn)` before
  `kui_run` / `kui_run_with`, called once with the run's `user` — a free
  function, since `kui_run`'s app is three arguments and not a struct
  and `KuiRunConfig` is the window, so ABI stays 18. The verb table has the row (`App::teardown`); the C counter
  prints its count as it goes, and the Node harness forwards an
  example's `teardown` as the Rust one does. Both checked in the window
  on macOS: ⌘Q through System Events prints the teardown line and
  nothing after; the close button prints it once and then the line
  after the call. Also written down: on the pumped path a panic
  unwinding through the host drops the runner, and the drop retires it,
  so `teardown` *does* run for that crash (the F74 text said a crash
  does not reach it, which holds for `run`) — and a `teardown` that
  panics there is an abort, as any panic in a drop is.

  *What you can delete:* a `process.on('exit')` or `SIGTERM` handler
  meant to save on quit — the first never ran under ⌘Q, and the second
  is not what ⌘Q sends.

- **A select** (backlog F72, from kawoosh's Facts tab: three toggles that
  cycled a base, an accent and a menu mode with a click each, off in the
  tab strip and away from the facts they changed). `widgets::select(ui,
  label, &options, current)` is a field showing the choice in force that,
  clicked, drops the core's own menu of the choices with the current one
  checked — the same menu a right-click opens: drawn in the frame, or the
  platform's where the host shows menus itself; dismissed by Escape or a
  press outside; its rows walked by the arrows and read as a menu. The
  app holds no open state; what it hears is the choice, as the `menu`
  event a menu row posts, on the field's key: `{kind: "menu", role:
  "custom", item: <the option>}`. `select_items` is the same field over
  `MenuItem`s, whose choice posts each item's `id`; `select_with` takes
  the field's spec and text style, and `select_spec` is the stock one.
  A reader hears a button named by the field, described by its choice,
  expanded while the menu is open. `examples/rust/widgets/select.rs`.

- **The select in every binding** (backlog F73, the day after F72):
  `<select label options current/>` in JSX — `options` are strings or
  the `MenuItemInput` objects `openMenu` takes, `current` from 0 —
  `dropdown { label=, options=, current= }` in Lua (`select` is Lua's
  own; `current` from 1, as a Lua list counts), and `kui_select(ctx,
  label, items, count, current)` in C over the `KuiMenuItem`s
  `kui_open_menu` takes, `-1` for none. All three lower to the one Rust
  widget, and the choice arrives as the `menu` event each binding already
  reads. A `select` scene in the conformance corpus, built in all four
  adapters; a Node example beside the Rust one; the props.md row.

- **A caret that does not blink** (backlog F68, from kawoosh idling in
  normal mode: sixteen frames in eight seconds with nothing happening —
  the blink clock, armed on the `caret` row its block caret declared for
  the IME and the access tree, toggling a phase the block never read).
  `caretSolid` / `caret_solid = true` / `KUI_VALUE_CARET_SOLID` in
  `value_set` / `NodeSpec::caret_solid()` beside `caret` on a `line`
  says the caret is solid: the row still anchors the IME and is still
  the caret assistive technology hears, but `has_caret` leaves it out,
  so the runner's clock is not armed and an editor idling in normal mode
  asks for no frame at all. The phase stays `true` while it is declared
  — in a window without the keyboard too, where the runner hides a
  blinking caret: the clock owns only what it blinks, and a solid
  caret's unfocused look (hollow, as `modal_editor` now draws it;
  dimmed; gone) is the view's from `env.focused` (backlog RG14) — so a
  view that reads `caret_visible` draws its block; the bar of insert
  mode, declared without it, blinks as before. Node gains
  `hasCaret()` (the reading C had as `kui_has_caret`), so a headless
  test can see that a view arms no clock. `modal_editor` keeps its
  normal-mode block solid now, the way a modal editor does.

  *What you can delete:* dropping the `caret` row in the modes whose
  caret does not blink to keep the window quiet — and the screen-reader
  caret and the IME anchor that went with it.

- **A devtools tab is selected from the app** (backlog F67, from
  kawoosh's Syntax tab: a `:syntax_tree` command with no way to *show*
  the tab it had built — the strip's click and `Ctrl+Shift+N` were the
  only two, and both the user's). `Core::set_devtools_tab(name)` /
  `win.setDevtoolsTab(name)` / `kui_set_devtools_tab(ctx, name)` shows
  the tab named — one of the panel's own by its name (`facts`, `events`,
  `tree`) or a declared one's — as the strip's click would. A declared
  name the panel does not list yet is kept and shows once a frame
  declares it, so the call lands before the first frame; the return says
  whether the panel lists it now (a declared tab is listed from the
  panel's first frame on). A hidden dock comes back on the right, as the
  picker's raise does; `on` is not touched — `set_devtools(true)` is
  still the app's to call beside it. Once, not every frame: called each
  frame it would pin the strip against the user's own clicks.
  `Core::devtools_current_tab()` / `devtoolsCurrentTab()` /
  `kui_devtools_current_tab(ctx, &out)` reads the selection back by the
  same names — the strip's own reading, where `devtoolsShownTab()` stays
  the encoder's (a declared tab on show in the main window, `null` for
  one of the panel's own). Node gains a `DevtoolsTab` type for the three
  own names; the verb table has both rows; Lua stays a guest, as for
  every devtools verb. Both `devtools_tab` examples gained a page button
  that jumps to the Inspector and print which tab the panel is on.

### Fixed

- **A `minWidth`/`minHeight` of `"fit"` inside a devtools tab's content
  was the first measurement, not the fit** — found building F75's
  Settings tab: a toolbar row with `min_height: fit` stood 86 px tall
  for one line of text. The content of a host's tab is a float anchored
  to the tab's body by key, laid out twice (the five passes, then the
  sixth against the anchor), and a fit floor is written back into the
  spec as the number it resolved to — so the sixth pass read the first
  run's number, measured before the float had a width, where the text
  had folded into a column of one word. The floors a frame declared are
  remembered and declared again before the re-run. Any `"fit"` floor
  inside a node-anchored float was affected; nothing outside one.

- **A grow child's `max` left a hole its siblings could have filled**
  (backlog F71, from the devtools Tree tab: the node list and the
  inspector both `Grow(1)`, the inspector capped at 300, and a third of
  the panel empty under them). A grow child whose own `min` or `max`
  held it off its share kept its share's worth of the container anyway;
  the run now freezes such a child at its clamp and shares what is left
  among the others, as flexbox does, until a pass freezes nothing. A
  view that put a grow spacer or a fixed height where the hole was can
  take it out.

- **F71's freeze loop froze a `min` and a `max` violator in the same
  pass, so a plain sibling could get nothing** (backlog RG5, a
  regression of this cycle). A 600 px column of three grow rows, one
  capped at 100, one held to 500 and one plain, came out 100 / 500 / 0
  — the capped row at its cap and the plain one empty, where flexbox
  (CSS Flexible Box §9.7, step 6) gives 50 / 500 / 50. A pass now sums
  its violations and freezes only the violators of the dominant sign —
  the `min` ones when the sum is positive, the `max` ones when negative
  — and re-shares the rest, the other sign's violators included, until
  a pass's violations cancel. The layout result that moves: a run with
  a `min` violator and a `max` violator in the same pass. When the
  `min` side dominates, a capped sibling now takes a share of what the
  re-share leaves instead of sitting at its cap (the 100 above is 50);
  when the `max` side dominates, a `min` sibling whose re-share clears
  its floor gets that share instead of its floor (600 over `max 100`,
  `min 210` and a plain row is 100 / 250 / 250, was 100 / 210 / 290).
  A run with violators of one sign only — the F71 case, one capped
  child — lays out as it did, and no corpus scene moves. The frozen
  set is a byte per child of the run in a scratch the tree keeps,
  where it was a list searched per child per pass — quadratic in the
  frozen count; the new `frame_1k_grow_rows_capped` row (a column of
  1k grow rows under a staircase of `max_height`s, four passes, 348
  frozen) reads 80 µs against 115 µs before.

- **The drawn menu is fitted to the window, not the app's area** — found
  building F72: a float of the core's menu took the host viewport (the
  window less the dock) as every host float does, so a menu opened in
  the dock — the panel's select — was pushed left into the app. The
  core's menu now floats against the window, as the devtools' own nodes
  do and as a platform menu would; a context menu opened near a dock's
  edge may overlap the dock where it used to be squeezed beside it.

- **`platform` in the devtools' menus select put nothing back** — the
  override's `None` left the core in whatever mode was set last, so
  `drawn` then `platform` stayed drawn. The panel remembers the host's
  own mode (menus and bar) when the override goes on and restores it
  when it is lifted, or when the panel goes off.

- **A host's menu answered a devtools select to the app** — found
  building F72: `activate_menu_item` is not an input, and its events
  skipped the pass an input's take on the way out, so a row chosen from
  the platform's menu reached the app as a `menu` event on the panel's
  own node instead of acting. Every batch now leaves through the one
  `outbound` pass: the panel's controls taken back, the rest translated,
  stamped with their window and logged.

- **A devtools select's menu outlived the panel, and its rows reached
  the app** (backlog RG2, from the regression pass of 2026-09-19): with
  the Facts tab's `theme` menu open, the panel turned off — the app's
  `set_devtools(false)`, or the header's and `Ctrl+Shift+D`'s `off` —
  or popped into its own window left the menu open in the main window
  with nothing to hang under, its rows still drawn and in the access
  tree, and a click on one handed the app a `menu` event from origin
  65535 (`{kind: "menu", role: "custom", item: {dt: "base:light"}}`),
  an origin no app declared, with the override unchanged. The panel's
  own menu now goes when this window stops building the panel, and an
  event from the devtools origin is taken back whatever the panel's
  state — a control still in the tree between the door and the frame
  it owes is nobody's. An app's own menu is untouched.

- **`Ctrl+Shift+M` was a two-way toggle from a compile-time guess**
  (backlog RG12, from the same pass): the chord flipped `native_menus`
  from `cfg!(target_os = "macos")` when no override was set and never
  set none again, so on a drawn-menu host on macOS the first press
  "toggled" to drawn — nothing visible changed, the select read `drawn`
  where it read `platform` — and no number of presses came back to the
  host's own mode, which the select's `platform` row restores. The
  chord walks the select's three in the select's order, platform →
  native → drawn → platform, over the same state and the same restore
  path, and a choice made in either is where the other goes on from.
  The stream's note says `menus: platform` for the host's own.

- **A `Fit` table of `grow` rows was 0 wide** (backlog RG11, from the
  regression pass of 2026-09-19): `NodeSpec::table()` and `<box
  dir="table">` are `fit` wide, a `fit` parent counts a `grow` child
  as nothing, and the rows every example — and the howto's own
  snippet, which gives the table no width — writes are `grow`, so the
  table laid out 0 wide with every text in it folded to a glyph a
  line. A table's fit width is its columns' now, whatever its rows'
  sizing: the rows are the table's, and their `grow` says how the
  columns share the table, not that the table is nothing. A `min:
  fit` floor on a `grow` table reads the same number. A view that
  gave a key/value table a width or a `grow` parent it did not want
  can take it back.

- **A `scrollX` table of `grow` rows could not scroll** (backlog RG3,
  from the same pass): the table skips compressing its fit columns
  when it scrolls x, as documented — but a `grow` row is exactly the
  table's width, and a scroll container's content is measured from its
  children's boxes, so the overflow the table kept was clipped and
  `scroll_max.x` was 0; only a `fit` row, which nobody writes, could
  scroll. A row of a table that scrolls x is at least as wide as its
  columns now, its own padding and gaps counted, and the table scrolls
  to them; a table that does not scroll leaves its rows' boxes alone,
  as any row is left when fixed children overflow it.

- **A `column` straight under a table was a row, and its stacked
  children were cells** (backlog RG7, from the same pass): a row of a
  table was any in-flow container under it, so a `column` section — a
  heading text over a row, the natural shape for a grouped settings
  list — had its heading taken as cell 0 and its inner row as cell 1,
  widening the table's real columns to them (probed: 80 and 100 where
  the rows' cells were 30 and 20), and a table straight under a table
  had its rows taken as the outer's cells. Only a `row` is a row now;
  a `column`, a text or a nested table under the table is a child with
  its own width and its own children ([ADR 0033](docs/adr/0033-a-table-is-a-column-whose-cells-align.md)
  decisions 2 and 9). The flag itself is read on a column only, so a
  `LayoutSpec { dir: Row, table: true }` — buildable from Rust's public
  fields alone — is the row it says it is, to the solver, `NodeInfo`
  and the corpus alike.

- **An image cell was stretched to its column and re-aspected**
  (backlog RG8, from the same pass): a cell takes its column's width,
  and an image's `fit` height follows its width, so a 16 px icon in a
  column whose widest cell was 200 became a 200 × 200 box and its row
  200 tall. An image cell's height is its aspect at its *own* width
  now — the number a fixed image declared, the intrinsic one of a
  `fit` image — so the box is 200 × 16; how the pixels meet the wider
  box is the image's `fit` row (`fill` stretches, `contain` draws them
  at their size, centred), and an icon that must keep its width sits
  in a box, which holds the column with the icon at its size inside.

- **A node-anchored float lost its own `min: fit` floor** (backlog RG6,
  from the same pass — a regression of this round, from the `"fit"`
  floor fix at the top of this list): the sixth pass hands the float's subtree its `fit`
  floors again and then clamped the float's own size with a spec copied
  *before* the re-run resolved them, so a `width: grow` + `min_width:
  fit` float anchored to a 300 px node was 300 wide around 400 px of
  content, where alpha.15 gave 400. The spec is read after each fit
  pass now, where its floor is a number. The devtools' own tab float
  declares no floor, so only a Rust app's own node-anchored float saw
  it.

- **A devtools panel turned on from inside a frame sat in the bottom-left
  corner** (backlog F70, from kawoosh's `:kui_debugger` and
  `:syntax_tree`, which call `set_devtools(true)` from `view`): the root
  is wrapped for a dock at `begin_frame`, when the panel was still off,
  so `finish` built the dock into the plain column root — 340 wide, half
  the height, at the left — and it stayed there until something else
  drew a frame (with the blink clock gone quiet since F68, indefinitely).
  A panel turned on, or moved to another dock, since the frame began now
  waits for the next frame, which the frame asks for; nothing draws in
  between. The same for `set_devtools_dock` mid-frame.

- **A left-docked panel moved, turned off or put on another tab from
  inside a frame stayed as it was** (backlog RG4, from the regression
  pass of 2026-09-19): F70's deferral lived in `finish`, and a left dock
  is built at `begin_frame` — it precedes the app in the root row — so
  `finish` skipped it before the check. `set_devtools_dock(Right)` from
  `view` left the left panel drawn and asked for no frame;
  `set_devtools(false)` drew the panel although it was off, and asked
  for nothing; `set_devtools_tab` left the strip on the old tab. With
  the idle loop quiet, each stayed until the next input. The panel
  built before the app is this frame's, as it was; a door that moved
  it, turned it off or changed its tab since the frame began now asks
  for the next frame, which is right. The right and bottom docks were
  never affected, since theirs is built at `finish`.

- **A host's menu report could choose a row the menu had disabled**
  (backlog RG9, from the regression pass of 2026-09-19). The pointer
  never reaches a disabled row — it has no click — but
  `activateMenuItem(i)`, `kui_activate_menu_item` and
  `Core::activate_menu_item` performed whatever row `i` named: a select
  over `['English', {label: 'Latin', id: 'la', enabled: false}]` answered
  `activateMenuItem(1)` with `true` and the app heard `{kind: "menu",
  item: "la"}`, a choice it had disabled. A row that cannot be chosen —
  disabled, or a separator — is refused in the core, so every door
  agrees: `false`, nothing posted, the menu still open, as a native menu
  that never sends such a row leaves it. `Core::activate_menu_item`
  returns `Option<Vec<UiEvent>>` now — `None` for nothing taken, no menu
  open included — where it returned the events; an index past the end
  still closes the menu and posts nothing (`Some` and empty). The Rust
  runner closes the core's menu on a refusal, since the platform's is
  gone either way.

- **A select's options and `current` were checked by C's door and by
  nobody else's** (backlog RG10, the same pass). `options: []` framed a
  field whose click opened a menu of no rows that only Escape leaves;
  `current: 9` over two options described the field as `""`; `current`
  on a `{role: "separator"}` described it as `""` and checked the
  divider, so every row grew a check gutter for a mark never drawn; and
  `{label, disabled: true}` on an option was silently an enabled row —
  the key is `enabled` — which is how RG9 was first missed. Now the one
  reader every binding's options go through, `MenuItem::options_from_value`,
  refuses an empty list (Node's `frame` and Lua's `dropdown` throw "a
  select needs at least one option"; C's `count == 0` is refused at its
  door as before, since its rows never pass the reader); a `current`
  past the options or on a separator is none — described by nothing, no
  row checked — with a `select-current-ignored` warning on the field,
  once per field, raised in `widgets::select_with` so Rust, C, Lua and
  Node all get it (C's door used to clamp it to none before the core
  could see it); a key of an option object that no row reads is an
  `unknown-prop` warning naming the six a row takes (`label`, `role`,
  `enabled`, `checked`, `id`, `accel`), with "did you mean `enabled:
  false`?" for `disabled`, raised by the Node encoder and by Lua's
  `dropdown` (`MenuItem::KEYS` and `MenuItem::NAME` pin the list, and
  `protocol().menuItem` carries it to the encoder); and Lua's `dropdown`
  without a `label` says "dropdown needs a label" where mlua said "error
  converting Lua nil to String". `openMenu`'s items are not checked for
  unknown keys; only a select's options are.

- **An assistive-technology click on a control behind a modal was
  dropped where the pointer's press dismisses** (backlog RG13, the same
  pass). A reader can name a node behind a modal — the access tree is
  not pruned (ADR 0003 decision 7) — and its `Click` was refused at the
  gate every other request obeys (AR18) and nothing else happened: a
  select's field clicked a second time while its own menu was open left
  the menu open, where the pointer's press on the field — no region
  under it, the field inert behind the menu's modal — asks the modal to
  go away and the menu closes. A reader's click on a node outside the
  modal is now the press outside (decision 6): `{kind: "dismiss",
  reason: "outside", tag}` on the modal, nothing on the node, focus where
  it was; the menu it was a select's field closes, and a dialog's app
  decides as it does for the pointer. Window chrome behind the modal is
  the platform's as before, a disabled control behind it is outside like
  any other, and every other request behind a modal still does nothing.
  The finding read the dropped click as a re-open; it was a refusal, and
  the pointer's dismissal is what was missing.

- **A pick raised over a tab named but not declared yet showed in no
  tab, and `setDevtoolsTab("Tree")` was refused and kept as a declared
  name for good** (backlog RG14, the same pass; both in the devtools).
  `set_devtools_pick(true)` decided "keep the tab up" on a name being
  *set*, and since F67 a name no frame has declared yet is kept there
  with the strip falling back to the panel's own tab — so
  `set_devtools_tab("syntax")` before the first frame and then the pick
  left the panel on the events tab with the picker's landing meant for
  a tab that was not up. It decides on a name the panel *lists* now: a
  pending name is no tab up, the pick is the chord's and shows the
  tree, as `Ctrl+Shift+P` does. And the panel's own names are taken in
  any case — `tree`, `Tree`, `TREE` — since the strip labels them with a
  capital and a caller writes what it reads there; a declared tab's
  name is the app's spelling and is matched exactly, as before.

- **Ten doc and parity nits from the regression pass** (backlog RG14):
  `kui.h`'s ABI history names `kui_select`, `KUI_TABLE` and
  `KUI_VALUE_CARET_SOLID` among ABI 18's additions (and the drop zone's
  five functions, where `abi.rs` said four); the F73 archive says the
  encoder takes `key` for a missing `<select>` label, as `<input>` does,
  and refuses only both absent; ADR 0033 counts the five solver reads of
  the table flag by name, says "the nth *in-flow* child of every row"
  with the changelog, README, howto, JSX type and the `table` element
  row (a float in a row is not a cell), and records as decision 10 that
  a percent column's basis is the row's content less its gaps — what the
  columns are laid across, so two `50%` columns with a gap fill the row
  — where a percent child of a plain row is its cut of the content box
  with the gap on top, as CSS has it (a test pins both); the F74 archive
  carries RG1's correction that a crash on the pumped path does reach
  `teardown`; and the solid caret's unfocused phase is written down as
  the view's (F68, above). `NodeInfo.table` was never missing from C:
  `kui_nodes` serialises the same map Node and Lua read, and a test now
  pins `table: true` on a `dir: "column"` row there.

### Changed

- **The devtools tab strip wraps, and the overrides sit on the facts
  they change** (backlog F72). With five tabs in a narrow dock the strip
  broke *labels* — `Synta / x`, `Event / s` — where ADR 0032 said it
  wrapped tabs; a tab is now one unbreakable unit and the row wraps
  whole tabs onto another line. The base, accent and menu toggles are
  gone from the strip's end: the Facts tab's `theme`, `accent` and
  `menus` rows each carry a select of the choices beside the fact — the
  fact is what the app has, the select what the panel holds it to,
  `app` (or `platform`) leaving the app's own. `Ctrl+Shift+T` / `A` /
  `M` cycle the same choices, in the select's order (`M`'s was a toggle
  until RG12, under Fixed). The `kui-devtools/base`, `accent` and
  `menus` keys are the selects now, in the Facts tab only.

- **The panel's own tabs are labelled `Facts`, `Events`, `Tree`** in the
  strip and the access tree, as a declared tab's label is written
  (`Inspector`, `Tree-sitter`) — the strip read as two kinds of tab
  before. Their *names* — what `set_devtools_tab`, `devtools_current_tab`
  and the `kui-devtools/tab-tree` keys use — stay lowercase.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run on `main` at
`a2db30b` on 2026-09-20 — the F67–F75 round, the regression pass over
it (RG1–RG16, every entry built the day after it was filed) and the
verify round's own fix, 22 commits after the alpha.15 tag. What follows
is what executed on what.

**macOS 27.0 (26A428, arm64, Apple M3 Pro), Xcode 27.0, rustc 1.98.0,
Node 26.8.1.** `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets -- -D warnings` are clean. `cargo test --workspace`:
**1283 tests over 97 suites, 0 failed** (1 ignored, the devtools'
`drive.rs` doc example), from alpha.15's 1221 — the table's twenty-one
through `layout::compute`, the select's per binding, the freeze loop's,
the teardown's, RG13's two, RG10's per binding among them. The scene
corpus runs in all four adapters against one reference report: **40
scenes** (`table` and `select` new), Rust and Lua through `cargo test`,
C through `target/debug/conformance` (the header at **374 fields, 256
enum members and 233 prototypes** — `kui_select`, `kui_on_teardown`,
`kui_set_devtools_tab` and `kui_devtools_current_tab` added, `KUI_TABLE`
and `KUI_VALUE_CARET_SOLID` as values of fields the header had, the ABI
still **18** since nothing a host lays out moved), Node through `npm
test` (**176 Node tests, 176 passed, 0 skipped**, the frame at **version
15** for the select's op). The C round, `cbuild --run`, passes its five
checks and the conformance replay. `npm run gen` regenerated `props.md`,
`jsx-runtime.d.ts` and `index.d.ts` and the tree carries the result (a
zero-line diff). `npm run typecheck` on `examples/node` is clean, and
the examples lockfile matches the linked package. The headless round,
`smoke -- --headless`, passes all **31 drives** (`select`, `table` and
the Node `select`/`table` new) — after the one thing the verify round
found: the `modal` drive still asserted alpha.15's silence for a
reader's click behind the dialog, which RG13 made the press outside,
and the drives run through the smoke binary only; it asserts the
amended rule now.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **36 Rust examples and the ten Node examples, each on both
bases, 120 frames each, every one exiting 0 with nothing on stderr** —
92 windows. The C and Lua hosts by hand under `KUI_SMOKE_FRAMES=120`:
`counter`, `host`, `c_panel` and `lua_panel` each opened a window and
exited 0, warning-free — **96 windows over five hosts.** The AX audit
against `accessibility`: **106/106**.

**Driven by hand, the finding that headed the round.** RG1 — F74's
`teardown` reached Rust only — was checked the way it was found: a Node
window and the C counter, each quit with ⌘Q posted through System
Events. Both are gone afterwards, and both printed from their teardown
on the way — Node's `teardown(model)` with the model, the counter's
`kui_on_teardown` callback with its count — where on alpha.15 neither
ran a line after the loop. RG15 checked F69's press-and-hold pin in a
window and found it inert (HIToolbox reads the user's global domain
through the one CFPreferences call no volatile domain reaches), so
RG16 removed the door before it shipped; the howto says what does work.

**The bench guard**, run alone against the alpha.15 tag on a quiet
machine (worst guarded spread 3.5%): every guarded row within its own
noise — `deep_nesting_64_levels` +2.3%, `frame_10k_rects` +1.3%,
`frame_10k_rects_with_access_tree` +0.8%,
`frame_10k_rects_with_text_and_hits` −0.2%, `frame_10k_segments` +0.6%,
`frame_1k_typical` +1.6%, `list_10k_rows_virtual` +0.6% — and every
unguarded row "same" (`drop_1k_rows_plain` +8.6% at ±5.7% is the
widest, an unguarded row whose two runs disagree by more than the
change); `frame_1k_curves` reads 277 → 267 µs, so **C41** (alpha.14's
+10% on that row, bisected to the drop-zone commit) is neither worse
nor addressed and the tag carries it as alpha.15 did. RG5's
`frame_1k_grow_rows_capped` is new and only at HEAD, so it has no
column yet. The README's table is refreshed from this run; its
`long_line`, `stream` and `cells` rows are not re-run.

## 0.1.0-alpha.15 (2026-09-16)

**What breaks.**

- **A rich text past the long-line threshold is chunked** (backlog C42):
  a `rich_text` / `<text>` with `<span>` children / `text({ … })` /
  `kui_rich_text` of 4096 bytes or more with no line break (and no
  `maxLines` or `ellipsis`) now takes the path a plain text of that
  length has taken since alpha.9 — shaped in ~1 KB chunks as they come
  on screen. What that changes for such a paragraph, as it did for a
  plain one: the access tree carries its **value without its runs**
  (a screen reader reads the text, not word by word); `measureText` /
  `measure_rich_text` / `kui_measure_rich_text` answer an **estimate**
  for the chunks that never showed, exact under monospace; a **kern
  pair or ligature at a chunk cut** is lost, one per KB at most; and the
  width a `scrollX` container sees can move a little as chunks fill in.
  A rich paragraph shorter than that, or one with a line break of its
  own, draws exactly as before.

### Added

- **A long line in spans costs what a plain one does** (backlog C42,
  from kawoosh opening a `.ttf`: 17.9 ms a frame average, 229 ms at the
  worst, drawing thirty-four rows of a binary's newline-less "lines" as
  one `rich_text` each). `intern_rich` never asked `is_long`, so a 50 KB
  line with **one** styled span was shaped whole — 331 ms the frame it
  scrolled in, 24 MB of glyph templates kept, three such rows filling the
  64 MB text-cache budget so that a screenful reshaped what the last
  frame evicted (56 ms steady, measured) — and an editor's caret row,
  whose spans change with every caret move, was that again per
  keystroke. Now the rich paragraph is a `LongLine` like the plain one:
  the content concatenated once, the spans kept as byte ranges with their
  attributes beside it, and each chunk shaped as a rich run of **the
  spans that intersect it, sliced** — keyed by its content and its
  spans' attributes, so a caret span moving along the line re-keys one
  chunk and every other hits. The chunk's span backgrounds, underlines
  and strikethroughs are built with its glyphs as any rich run's are;
  under `wrap: word` / `glyph` they are drawn on the rows too, a rect a
  row break falls inside cut at the break, which the plain path never
  needed. Highlight, hit-test, caret and the wrapped rows were already
  per chunk. Measured: the 50 KB one-span line 331 → 20 ms cold and 0.13
  → 0.024 ms steady; thirty-four such rows 13.5 s → 0.66 s cold and 9.4 /
  56 ms → 0.40 ms steady; the caret span moving one character a frame
  along a 100k-character line, **1.29 ms, the same number as a plain
  keystroke into it**. Two bench rows, `long_line_100k_rich_first_frame`
  and `long_line_100k_rich_caret`, and four tests in `tests/long_line.rs`
  (a chunked rich line draws its marks where the spans put them and
  shapes the screenful; a moving span reshapes one chunk; a short rich
  text and one with a line break keep the whole path; a wrapped rich
  line draws a background on every row it covers). No API moved: the
  same calls, in four bindings, take the new path by length.
- The kawoosh report's own diagnosis — "thousands of spans, looks
  quadratic" — was measured before anything was built and is **not
  it**: at a fixed 16 KB, 250 → 8000 spans move the cold shape 161 →
  275 ms (four-byte spans cost ~1.5× — a factor, not a power), and at a
  fixed 2000 spans doubling the bytes doubles the time. Shaping is ~9 µs
  a byte whole-line, plain or rich alike; what made the rich row a
  thousand times the plain one was that it was shaped whole where the
  plain one was chunked. The backlog section records the table so the
  next round does not chase it.

### Changed

- **A text's cache key hashes its content eight bytes a round** (backlog
  C43). `style_key` and the rich key mixed the content through
  `key::fnv`, a byte a round with a multiply on the chain — a nanosecond
  a byte, so a long line's lookup cost its length every frame: thirty-four
  chunked rows of 500k characters, every chunk shaped and nothing
  changing, read 26.8 ms a frame drawing none of it; and the long-line
  decision scanned the content for a newline every frame besides, through
  a `str` pattern search that is per character. Content past 32 bytes now
  goes through `key::hash_bulk` — Fx's rotate-xor-multiply round over
  words, the length mixed first and murmur's finalizer after, so a zero
  tail and a shorter text differ and every bit reaches the key — and the
  eight bytes it returns are mixed by `fnv` as before; below 32 bytes the
  bytes are mixed as they were. And a text that could be long by its
  length and style is **looked up under both keys before its bytes are
  scanned**: the newline scan runs once, when the cache has not seen the
  text, and is a byte scan when it does (`intern_any`, `intern_rich_any`).
  The same screenful reads **3.0 ms**. `key::fnv` itself is untouched —
  keys, the access digest and the corpus digest are bit-stable as they
  were; a text cache key is not something anything keeps across versions,
  and nothing pinned one. A bench row, `long_rows_34x500k_steady`, and a
  unit test on the hash's tails and lengths. The entry's other line — a
  `key` the app supplies on the style, so an editor hands over `(rev,
  line)` instead of the content — is **not built**: the hash is the fix,
  and the door waits for a host with lines the hash is still too slow
  for.
- **`long_line_100k_edit` measured a cache hit.** The row alternated two
  letters at one byte, so from the third frame on both edited contents
  were in the text cache and the row read a lookup: ~160 µs, which the
  README's table and C19's outcome cited as the cost of a keystroke into
  a long line. The row now inserts two characters different every frame,
  and reads **~1.3 ms** — the chunk the keystroke lands in, reshaped, with
  cosmic-text's shape-run cache warm for every word but the edited one.
  The table says so; the alpha.9 outcome is left as written, with its
  number, since the archive is what was measured then.
- **The README's bench table is refreshed** for the first time since the
  machine moved to macOS 27 — the pre-tag guard run against alpha.14 was
  quiet (worst guarded spread 3.7%), so its HEAD medians are the table's;
  alpha.13's own code reads ~9% slower under 27.0 than the table recorded
  under 26.6.2, so the absolute numbers moved with the OS, not the code.

### Fixed

- **A long line with a multibyte character on a chunk edge panicked**
  (backlog C44, found by the pre-tag round's field check — kawoosh on
  this tree opening a 2.6 MB font — and fixed before the tag).
  `chunk_ranges` cut each ~1 KB window at a byte and sliced the content
  there to look for whitespace, so a character straddling byte 1024 (or
  2048, …) of a long line was a slice inside a `char`: `end byte index
  2942 is not a char boundary`, on the first frame, for any long line
  with a multibyte character at the wrong offset — a lossy-decoded binary
  every time (U+FFFD is three bytes, 1024 % 3 = 1), a log line with an
  arrow in it or prose past 4 KB with a curly quote when unlucky — since
  the chunked path shipped in alpha.9. The window's end is floored to a
  char boundary now; one test pins it, plain and rich.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag
in the night of 2026-09-16 on `main` after `29d4a3e` — the C42/C43
round, its review and its formatting, three commits after the alpha.14
tag, plus C44 found and fixed inside this round. What follows is what
executed on what.

**macOS 27.0 (26A428, arm64, Apple M3 Pro), Xcode 27.0, rustc 1.98.0,
Node 26.8.1.** `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets -- -D warnings` are clean. `cargo test --workspace`:
**1220 tests over 95 suites, 0 failed** (1 ignored, the devtools'
`drive.rs` doc example), from alpha.14's 1215 — before C44's test, which
makes 1221. The scene corpus runs in all four adapters against one
reference report: **38 scenes**, Rust and Lua through `cargo test`, C
through `target/debug/conformance` (the header at **374 fields, 255 enum
members and 229 prototypes**, unchanged — nothing in this release
touched the ABI, which stays **18**, or the Node frame, which stays
**version 14**), Node through `npm test` (**168 Node tests, 168 passed,
0 skipped**). The C round, `cbuild --run`, passes its five checks and the
conformance replay. `npm run gen` regenerated `props.md`,
`jsx-runtime.d.ts` and `index.d.ts` and the tree carries the result (a
zero-line diff). `npm run typecheck` on `examples/node` is clean, and
the examples lockfile matches the linked package. The headless round,
`smoke -- --headless`, passes all **28 drives**.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **34 Rust examples on both bases and the eight Node windows,
120 frames each, every one exiting 0 with nothing on stderr** — 84
windows. The C and Lua hosts by hand under `KUI_SMOKE_FRAMES=120`:
`counter`, `host`, `c_panel` and `lua_panel` each opened a window and
exited 0, warning-free — **88 windows over five hosts.** The AX audit
against `accessibility`: **106/106**.

**Driven by hand, the field case itself.** No example holds a long rich
line, so the round ran kawoosh — its `kui` by path on this tree, release
— on a 2.6 MB `.ttf`, the shape of file that filed C42, with its
`:kui_framerate_hud` on. The first attempt panicked on the first frame:
**C44**, above, fixed and re-run. On the fix: the file opened, `:400`
landed, and sixty caret moves over the rows (each an `ex` line through
the instance's socket) filled the HUD's ring at **2.68 ms a frame
average, 36.6 ms at the worst**, against the **17.9 / 229 ms** the
report that filed C42 read on alpha.14 — the frame captured from the
window, not read from the tree.

**The bench guard**, run alone against the alpha.14 tag on a quiet
machine (worst guarded spread 3.7%): every guarded row within its own
noise — `deep_nesting_64_levels` −0.8%, `frame_10k_rects` −1.8%,
`frame_10k_rects_with_access_tree` +0.2%,
`frame_10k_rects_with_text_and_hits` +1.0%, `frame_10k_segments` +3.9%
at ±3.7%, `frame_1k_typical` +2.1%, `list_10k_rows_virtual` −0.2% — and
every unguarded row "same"; `frame_1k_curves` reads 285 → 287 µs, so
**C41** (alpha.14's +10% on that row, bisected to the drop-zone commit)
is neither worse nor addressed and the tag carries it as alpha.14 did.
The README's table is refreshed from this run (the first since the OS
moved), and its `long_line` rows from the same day's `--bench
long_line`: the three new rows, and `long_line_100k_edit` at its honest
number.

## 0.1.0-alpha.14 (2026-09-15)

**What breaks.**

- **C: ABI 17 → 18.** `KuiSpec` gained `on_drop` and `drop_bg` at its
  end — the first [in] append under the amended rule (ADR 0006, backlog
  AR50), so it bumps: the library reads the whole struct, and a host
  that did not recompile would have the two read from past its end.
  Recompile; a zeroed tail is no zone and no colour. The same version
  adds `kui_input_drag_files`, `kui_input_drop_files`,
  `kui_input_drag_cancel`, `kui_is_drop_target` and `kui_drop_target`.
- The Node binary frame is **version 14**, from 13: `devtoolsTab` is a
  new op (ADR 0032, below). Encoder and addon ship together, so nothing
  to do unless you own an encoder.
- Under macOS custom chrome `widgets::titlebar` / `titlebar_with` (and
  the `titlebar` element in every binding) draw the strip **as tall as
  the OS's own titlebar** — `env.window.native_controls.h`, now measured
  from the window — instead of `Metrics::titlebar_h` (backlog W17). On
  macOS 27 that is 32 px where it was 34; on macOS 26, 28. A view that
  laid something out against the metric under the strip reads
  `widgets::titlebar_height(ui)` instead, which is the metric everywhere
  the OS keeps no controls over the strip.
- **The pointer shape is declared, not derived.** An `onClick` /
  `on_click`, `focusable` or `onDrag` / `on_drag` node with no `cursor`
  is now the plain arrow, where it was a hand (`pointer`) or an open hand
  (`grab`, `grabbing` while captured). The I-beam over an editor or a
  `selectable` scope is the one shape the core still implies, and the
  stock `button` (`widgets::button` / `button_with`, `<button>`, `button
  { }`, `kui_button_with`) declares `pointer` for itself, so a stock
  button looks as it did. Every other control that wants a hand says
  `cursor: "pointer"`; a handle says `grab`, and `grabbing` while its
  drag runs, from the drag state the model already keeps. A captured drag
  holds the dragged node's declared shape (or the arrow) wherever the
  pointer goes; nothing is promoted to `grabbing`. `Core::cursor_shape`,
  `ctx.cursorShape()` and `kui_cursor_shape` answer the new rule.
- A `cells` node draws **box drawing, block elements and the Powerline
  arrows from the cell box, not the font** (backlog F66): U+2500–U+259F
  and U+E0B0–U+E0B3 in every `cells` node — JSX, Lua, C and Rust — now
  render as cell-sized masks whose strokes sit on whole pixels, so what a
  `│`, a `▐` or a `═` draws changes for every grid, and a screenshot pinned
  against alpha.13's dashed frames needs re-taking. Bold no longer
  thickens a light line (the set has its heavy variants), italic is
  ignored, and a font's own box-drawing glyphs are never consulted.
- A headless press's default `physical` **folds a letter to lower case**
  (backlog F65): `press("Z", { shift: true })` with no fourth argument
  now reaches the sink with `physical: "z"`, the pair a window reports
  for ⇧Z, where alpha.13 delivered `"Z"`. In `KeyPress::new`, so Node's
  `keyDown` / `press` / `release`, C's `kui_input_key_down` with a NULL
  `physical`, Lua's and the Rust `testing` helpers all fold alike. A test
  asserting the old pair, or a keymap bound by position that only ever
  passed headless, reads the window's spelling now; a `physical` a
  caller spells is still delivered as spelled.

### Added

- **A drop zone: `onDrop` on any node, the files as an event, `dropBg`
  while they hover**
  ([ADR 0031](docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md),
  backlog C40). A node declaring `onDrop` (JSX), `on_drop` (Rust, Lua)
  or `KuiSpec.on_drop` (C) is a zone: files dragged in from the OS over
  it emit `{kind:"drop", phase:"enter"|"move"|"leave"|"drop", paths, x,
  y, tag}` — the OS paths as strings, the pointer in logical viewport
  coordinates (absent on `leave`), and no `leave` after a `drop`. The
  zone under the files is the topmost *zone* by paint order: a button
  or a field inside a zone is the zone's, and a node that is no zone
  and has none enclosing it is looked past — so the banner an app
  floats over the zone in answer to `enter` cannot make the zone lose
  the files, the flicker HTML's `dragleave` is known for. `dropBg` /
  `drop_bg` lights the zone while they hover, resolved in the same pick
  as `hoverBg` and winning over it, eased by `transition`, cleared on
  leave, drop and cancel. Nothing is re-resolved when a frame lands:
  only the driver's next report moves the files, and a zone the view
  stops declaring mid-drag hears its prepared `leave` then. Three
  `InputEvent`s for a driver (`DragFiles { paths, at }` — entering and
  moving alike, `DropFiles`, `DragCancel`; `Ctx.dragFiles` /
  `dropFiles` / `dragCancel` in Node, `kui_input_drag_files` /
  `drop_files` / `drag_cancel` in C), and one reader in every binding
  (`Core::drop_target` / `Ui::is_drop_target`, `dropTarget()` /
  `isDropTarget`, `env.drop_target()` / `is_drop_target`,
  `kui_drop_target` / `kui_is_drop_target`) — what a driver answers the
  OS with. The `drop` corpus scene pins the resolver's three cases in
  four adapters (`Step::DragFiles` / `DropFiles` / `DragCancel`, `n`
  files spelled `/drop/1.txt` …). `examples/rust/features/drop.rs` and
  `examples/node/features/drop.tsx` are the subject, headless and by
  hand. No access row: assistive technology has no drag protocol, and a
  screen-reader user's way in is a button beside the zone.
- **The Rust runner answers a file drag with where it is, on macOS.**
  winit 0.30's `HoveredFile` / `DroppedFile` / `HoveredFileCancelled`
  carry no position and winit never implements `draggingUpdated:`, so
  the runner overrides the five `NSDraggingDestination` selectors on
  winit's window delegate (`crates/kui/src/macos_drop.rs`, the
  mechanism `macos_text_input` uses) — replaced, not wrapped, since
  winit's would queue every file a second time — reads
  `draggingLocation` and the pasteboard's file names, and answers
  `draggingUpdated:` from a stamp the runner writes after each dispatch
  (`drop_target().is_some()`): the copy badge over a zone, none off it,
  and a release off every zone refused so the icon slides home, one
  update late. Verified with a file dragged from the Finder onto the
  example. On Windows and Linux the runner takes winit's events at the
  pane's last cursor, so a zone is found where the pointer was before
  the drag and `move` never fires (backlog W19).
- **A tab of the app's own in the devtools panel** (ADR 0032,
  `docs/adr/0032-a-devtools-tab-mounts-a-slot.md`). Beside facts, events
  and tree, an app or an extension declares tabs of its own — a
  tree-sitter inspector is the case that asked — in one of two forms.
  The *extension form* names a slot: `ui.devtools_tab("syntax",
  "Tree-sitter", "ts/panel")`, `<devtoolsTab name label slot/>`,
  `devtools_tab { name=, label=, slot= }`, `kui_devtools_tab`; while the
  tab is on show the panel declares that slot in the tab's body and the
  plugin draws there, and otherwise the plugin is not asked (and naming
  the slot raises no `unknown-slot`). The *host form* is the app's own
  content, **lazy in every binding**: `ui.devtools_tab_with(name, label,
  |ui| …)` runs its closure only while the tab is on show; C's `if
  (kui_devtools_tab_open(ctx, name, label)) { …; kui_close(ctx); }`
  answers the same rule; Node's `<devtoolsTab name label>{() =>
  …}</devtoolsTab>` function child is called by the encoder only for the
  tab `frame` / `setView` read as on show (`devtoolsShownTab()`); Lua's
  `devtools_tab { name=, label=, view = function() … end }` is called by
  the converter the same way. What the host builds is its own — its
  keys, its labels, its events reaching `update` untouched — laid out
  and painted as a **layer anchored to the tab's body**
  (`FloatAnchor::Node`, a sixth layout pass that runs only on a frame
  with one), clipped to it, and in the dock's focus region (Tab walks
  it after the panel's own stops, never from the app's ring). Both forms
  are declared every frame, panel on or off; a name declared twice warns
  `duplicate-tab` and keeps the first; a declaration Node or Lua cannot
  read as either form is a throw at the encoder and a `bad-devtools-tab`
  warning at the converter. The panel's facts are the tab's to read and
  drive: `devtools_selected` / `hovered` / `picked` (`devtoolsSelected()`
  …, `kui_devtools_selected` …), `set_devtools_selected` (select and
  reveal in the tree tab from outside it), and `set_devtools_pick` /
  `devtools_picking` — the picker raised from a tab lands its pick in
  `selected` and leaves the tab up, where the chord's pick shows the
  tree. `Ctrl+Shift+N` and the strip walk the declared tabs after the
  panel's three. `examples/rust/features/devtools_tab.rs` and
  `examples/node/features/devtools_tab.tsx` are the Inspector. Frame
  protocol v14 (`devtoolsTab` is a new op). Limits stated in the ADR: a
  host-form tab is docked-only — in `window` placement its body says so
  — and an extension-form fill's keys differ between docked and window
  placement.

- **The devtools chord is the app's to respell.** `Ctrl+Shift+I` — the
  chord that moves the keyboard into the panel and back out, and brings
  a hidden panel back — is now one door on every host:
  `Core::set_devtools_key(Accel)` and `kui::app("x").devtools_key(..)`
  in Rust, `setDevtoolsKey("f12")` on `Ctx` and `KuiWindow`,
  `kui_set_devtools_key(ctx, key)` in C, in any spelling a menu item's
  `accel` takes (`"f12"`, `"mod+shift+d"`, `"⌥⌘I"`), with
  `devtools_key` / `devtoolsKey()` / `kui_devtools_key` reading it back
  in the portable spelling `Accel::spelling` now gives
  (`"ctrl+shift+i"`, `"super+alt+d"`). The default stands; the panel's
  other chords stay `Ctrl+Shift+<letter>`, since they are reached once
  the keyboard is in; and a chord the app takes is the app's for good —
  with `F12` set, `Ctrl+Shift+I` reaches its sinks like any other press.
  The facts tab's `region` row names whichever chord is in force —
  handed into the collection, since the main window's build collects
  with the panel's state taken out of the session and a reader of the
  door there saw the default (a real window showed `^⇧I` over an `F12`
  that already answered; the test now reads the painted row). Lua has no
  cell, for the guest reason the verb table states. The examples harness
  takes `--key CHORD` in Rust and Node.
- **`owed()` — what the last frame left owed, by kind** (backlog F64,
  the pomodoro's third wish): `animating()` is one bool over five sources
  and a keyframe `repeat` cycle sets it on every frame, so under one
  `settled()` could only ever hit its cap — right for the driver, which
  must schedule the frame either way, and useless for a test asking
  whether the *transitions* have run out. `Core::owed()` returns `Owed
  { transition, cycle, depart, requested, autoscroll }` (`animating()`
  is its `any()`, `beyond_cycles()` the wait's predicate); the anim
  store keeps the two bits apart where it kept one. Doors: `owed()` on
  Node's `Ctx` and `KuiWindow` as `{transition, cycle, depart,
  requested, autoscroll}`, and `quiet(maxMs)` on the window loop —
  `settled` with a cycle allowed, resolving the first time a pump
  leaves nothing owed but a cycle and no effect unflushed; its own name
  rather than an option, since a wait that ignores something should
  say so. C: `kui_owed` returning `KUI_OWED_*` bits (`kui_animating` is
  `kui_owed != 0`). Not a new frame version or ABI: one function and
  five plain constants, pinned like the rest.
- **`env.audio.live` says what a play that waited on a refused open
  reads** (backlog F63, the pomodoro's second wish): counted from the
  frame it was asked until the open answers; if the device refuses, the
  play is refused on the next apply — `phase: "refused"` for a tagged
  one — and leaves the count with it, so a machine with no output device
  shows `opening`/1 then `failed`/0 with the refusal between. The
  behaviour was already this; the sentence is on the row in `index.d.ts`,
  `kui.h`, `props.md` and `AudioEnv`, and the existing refusal test now
  asserts `live` on both sides of the flush, so the pomodoro's guard can
  be an assertion.

- **`frameStats().framesTotal` and `.pumps`** (backlog F62, the
  pomodoro's first alpha.13 wish): two monotonic counts beside the ring —
  every frame the window has painted, and every `pump()` it has taken,
  the one that opened it included — so two readings a second apart are
  that second's frame and pump rates, and a smoke test can assert the
  driver's backoff (F57) where before only a process monitor could. The
  entry found `frames` was no counter either: it is the 120-sample
  ring's fill, climbs to 120 in the first two seconds and stays there,
  which its doc now says; it keeps its name, since a rename is a break
  for nothing. In Rust, `FrameStats::total` and `PumpRunner::pumps()`.
  Pinned over a fake surface on mocked timers: a stopped app with a
  once-a-second tick takes ~12 pumps in the 100 ms after a click and ~31
  over its second second — a lower rate, and the count to read it from.

### Changed

- **The cursor is what the view declares** (above). The C3 derivation
  made every clickable box a hand and every draggable one a grab, which
  is not what a desktop does: a native button, a tab, a list row, a menu
  item and a titlebar are all the arrow, and the hand is the Web's
  convention for a link. Deriving it meant a plain `onClick` card, a
  `focusable` list row and a resizable pane all pointed, and an app that
  wanted the arrow back had to declare `cursor: "default"` on each. Now
  `Interaction::implied_shape` reads only `edit_origin` and
  `select_scope` — the I-beam, which every toolkit derives because the
  text itself is what says it can be taken — and the hand is where it is
  written: on the stock button, on the devtools dock's own buttons, on
  the `popup` example's field and rows, and wherever an app puts it. The
  `drag` example declares `grab` on its track and card and `grabbing`
  while `dragging` names one, which is how a handle says it is held: the
  core does not promote a declared `grab` when the drag captures, since
  the closed hand is a state the view has already, and a shape the view
  did not write would be the derivation coming back one shape at a time.
  The core test, the Node test and the drag example's headless drive
  pin the new answers: an `on_click`, `focusable` or `on_drag` band that
  declared nothing is `default`, a captured drag on it stays `default`
  over the editor below, and the stock button in every binding is
  `pointer` — unless the caller declared its own cursor, or it is
  `disabled`, when it is the arrow like any inert control.
  **What you can delete:** `cursor: "default"` on a clickable or
  focusable node that wanted the arrow.

### Fixed

- **A `cells` node's `│` was a dash with a gap under it, every row**
  (backlog F66, kawoosh's terminal on alpha.13: lazygit's panel frames
  dashed, its `▐` scrollbar thumb a column of separate dashes). A font's
  box-drawing and block glyphs span *its* line box — the report read
  Iosevka's: 1.25 em, 16.25 px at 13 px — and a cell is `lineHeight` tall,
  20 px there, which no font can know; 3.75 px of every row was empty
  under the stroke, and a face without the codepoints was worse, since
  cosmic-text's fallback changed weight and width mid-frame. Every
  terminal that draws these from the cell (Alacritty, kitty, WezTerm,
  foot, Ghostty) does what `cells` does now: `shape_cell` — the one door
  every cell of the grid comes through once per table — hands
  U+2500–U+259F and U+E0B0–U+E0B3 to `cells/boxdraw.rs`, which
  rasterizes the character into an alpha mask of exactly `cell_w × cell_h`
  physical pixels. The light stroke is `max(1, round(cell_w / 8))` px, the
  heavy three times that, and every stroke sits on the row or column
  `(cell − stroke) / 2` computed from the cell size alone, so every
  character in a row lands on the same pixel column and every one in a
  column on the same pixel row: adjacent cells' strokes meet with no seam.
  The whole set: light and heavy lines, corners, tees and crosses in every
  mixed weight (U+2500–U+254B, U+2574–U+257F), the double, triple and
  quadruple dashes with the gap split at the cell's edges so two cells
  show the same gap as two dashes, single-and-double lines (U+2550–U+256C)
  as two light rails a light stroke apart with the outer corner closed at
  the far rail and the inner at the near, the rounded corners
  (U+256D–U+2570) as a quarter arc of the light stroke joining two stubs,
  the diagonals (U+2571–U+2573) anti-aliased corner to corner, the block
  elements (U+2580–U+259F) as rectangles on eighths of the cell with the
  edges snapped so `▀` over `▄` fills the cell and `▐` stacked is one bar —
  the shades `░▒▓` as flat alpha at 25 / 50 / 75 % — and the Powerline
  triangles and chevrons. The atlas keys them on `(character, cell_w,
  cell_h)` beside its glyphs and images (`get_or_insert_synth`), so one
  cell size shares one slot and another size does not, and a reset drops
  them with the rest; `lookup`'s per-table cache holds them the same way
  it holds a glyph. `text` nodes are untouched — a mono `text` row showing
  a box-drawn table has the same gap, but there the line box is not a
  cell contract. The `terminal` access value and the `cell` click payload
  are unchanged: drawing only. Pinned in `tests/cells.rs` by reading the
  atlas back — two rows of `│` (and `▐`, `█`) are solid down every
  scanline with the second quad starting where the first ends, `┌─┐` /
  `│ │` / `└─┘` at 2× puts the corner's strokes on `│`'s columns and `─`'s
  rows, 16 px and 20 px rows are different slots and the same size twice
  is one, and `a` / `é` still go through the font — and the `cells`
  example's fake session now ends in a box-drawn bench table with block
  bars, which the native round captured seamless.

- **The macOS keep-out rect is measured, not assumed** (backlog W17,
  found on the first macOS 27 machine). `env.window.native_controls`
  was `78×28` — gpui's traffic-light padding under the macOS 26 SDK over
  the 28 px titlebar macOS 26 drew — and macOS 27 draws 14 px buttons at
  (9, 9) in a 32 px titlebar, so the strip's 34 px sat 2 px under
  buttons centred at 16 and read as taller than the OS's own. The runner
  now asks the window when the pane is created — the frame's height less
  `contentLayoutRect`'s is the titlebar, the close button's `x` plus the
  zoom button's right edge the width (78, the same number by the symmetry
  of the gaps) — and the old pair is the fallback for a window with
  nothing to ask. The strip follows it (`widgets::titlebar_height`), the
  metric's doc says which of the two a strip is drawn from, and the
  corpus keeps `78×28` as the fixture that pins the rule.

- **A non-activating popup no longer becomes key or main on macOS**
  (backlog W18). Every press on a `WindowKind::Popup` window made it
  the key window — AppKit's `sendEvent:` does that for any window whose
  `canBecomeKeyWindow` says YES, which winit's does — and the runner
  asked for the keyboard back a batch later, so the owner's titlebar
  greyed for 20–50 ms on every open and every pick (measured with an
  `AXObserver`). The popup's `NSWindow` now answers NO to both
  (`macos_key.rs`, an override on winit's window class with a per-window
  registry, the way the text-input one works): the owner stays key and
  main throughout, the mouse still reaches the popup, and its keys come
  from the owner as they always did.

**What you can delete:** a strip height of your own under macOS custom
chrome, measured to sit level with the traffic lights; a delay or a
second frame you waited before reading the owner as focused after a
popup pick.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag on
the evening of 2026-09-15 on `main` at `8b2f35b` — thirteen commits after
the alpha.13 tag, the four rounds above (W17/W18, F62–F66, C40, ADR
0032) all in. What follows is what executed on what.

**macOS 27.0 (26A428, arm64, Apple M3 Pro), Xcode 27.0, rustc 1.98.0,
Node 26.8.1.** `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets -- -D warnings` are clean. `cargo test --workspace`: **1215
tests over 95 suites, 0 failed** (1 ignored, the devtools' `drive.rs`
doc example), from alpha.13's 1186. The scene corpus runs in all four
adapters against one reference report: **38 scenes** — `drop` new —
Rust and Lua through `cargo test`, C through `target/debug/conformance`
(the header at **374 fields, 255 enum members and 229 prototypes**, from
372 / 250 / 213), Node through `npm test` (**168 Node tests, 168 passed,
0 skipped**, from 163). The C round, `cbuild --run`, passes its five
checks and the conformance replay against **ABI 18**. `npm run gen`
regenerated `props.md`, `jsx-runtime.d.ts` and `index.d.ts` and the tree
carries the result (a zero-line diff). `npm run typecheck` on
`examples/node` is clean, and the examples lockfile matches the linked
package. The headless round, `smoke -- --headless`, passes all **28
drives** — `drop` and `devtools_tab` new. Xcode 27.0 is on the machine
now, and `cc` links against the 27.0 SDK with no `DEVELOPER_DIR`
override: the alpha.13 note about the Command Line Tools was the
toolchain of that day, and is gone with it.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **34 Rust examples on both bases and the eight Node windows,
120 frames each, every one exiting 0 with nothing on stderr** — 84
windows (`drop` and `devtools_tab` new on both sides). The C and Lua
hosts by hand under `KUI_SMOKE_FRAMES=120`: `counter`, `host`, `c_panel`
and `lua_panel` each opened a window and exited 0, warning-free — **88
windows over five hosts.** The AX audit against `accessibility`:
**106/106**.

**Driven by hand, each through a real `CGEvent` and read back from the
screen:** a file dragged from a Finder window onto the `drop` example
(ADR 0031) — over the zone it lit, the banner said `1 file(s) over the
zone` with the pointer at `220, 168`, and the cursor carried the copy
badge; dragged off it onto the box that is no zone, the badge left, the
zone unlit and the counter read the `leave`; dragged back and released,
`landed:` named the path and nothing was lit after — and the source
file stayed where it was. In `devtools_tab` (ADR 0032), Ctrl+Shift+N
walked facts → events → tree → **Inspector** with the source reading
`Inspector built 0 time(s)` until that tab showed and counting from
then, which is the laziness; hovering `println` in the source lit the
token, wrote its path and scrolled the tab to `identifier @macro
println`; hovering a tree row lit `tree.walk()` in the source; clicking
a token row read `selected identifier` in the panel; `pick a node`
raised from the tab and a click on `let` read `selected let` with the
tab kept up. With `--key f12` on `button`, Ctrl+Shift+I reached the app
as a plain `modifiers` event in the events stream and opened nothing;
F12 brought the panel back. The cursor rule read from captures with the
pointer in them: the hand over `plain`, `+` and the app-palette `sand`,
the arrow over `disabled` and over a paragraph.

**The bench guard**, run alone against the alpha.13 tag: every guarded
row within its own noise — `deep_nesting_64_levels` +4.8% at ±6.4% run
to run, `frame_10k_rects` −3.4%, `frame_10k_rects_with_access_tree`
−2.7%, `frame_10k_rects_with_text_and_hits` −0.2%, `frame_10k_segments`
+3.6%, `frame_1k_typical` −3.8%, `list_10k_rows_virtual` −0.2% — and
the README's table is **not** refreshed: the alpha.13 side of this run
read `frame_10k_rects` at 805 µs against the 740 µs the table records
for that same commit, measured under macOS 26.6.2 — the OS moved under
the machine, so this run's absolute numbers are not the table's and
only the guard's differences carry (the alpha.7 rule). Three unguarded
rows read past ±5% with a readable spread; re-run alone, two were
noise (`list_10k_rows_naive` +3.1% at ±3.4%,
`drop_500_rows_declaring_exit` +1.5%) and one is not: **`frame_1k_curves`
255 → 280 µs, +10.0% at ±1.4%**, reproducibly. Four probes of that one row
(`scripts/bench-check.sh <commit> frame_1k_curves`) bisect it to one
commit: `cc070bd` (F62–F65) and `db2ff82` (the cursor) read 260 µs
against HEAD's 280–284, `5e6711e` (ADR 0031, the drop zone) reads 286,
and nothing after it — ADR 0032, the chord — moves the row. It is not a
guarded row, so the guard is green; it is filed as **backlog C41** with
the suspects in that commit's emit path, and the tag carries it, as
alpha.11 carried C29.

**What the round found, besides the row.**

- **The examples harness sizes its window for the dock at launch
  only** (backlog E4): `--key f12` on `button` was tried with `--dock
  off`, and F12 brought the panel back into a window sized without it,
  so the example's column was 220 px wide with its paragraphs shrunk
  under the button rows. The default launch — and × then the chord —
  never shows it, since the window was sized for the dock; filed as a
  harness nit, not a core one.
- **Earlier the same day, the W17/W18 round** ran on the alpha.13 tag
  plus its fix — the strip dragged, double-clicked and pinned, the
  popup's press-drag-release with an `AXObserver` reporting no key or
  main change, the native context menu, the declared and standard menu
  bars through the AX API, an emoji and an accented letter typed, Paste,
  undo, redo, Emoji & Symbols, the audio device's hold — and its record
  is the backlog's "From the macOS 27 round". Two things it left for the
  next driver's author: `cc` did not link under Xcode 26.6 against the
  27.0 SDK (gone with Xcode 27.0, above), and **a synthetic keystroke
  carrying a lone UTF-16 surrogate aborts a debug build** in winit's
  `create_key_event` → objc2's `nsstring_to_str` — `-[NSString
  UTF8String]` returns NULL for a string UTF-8 cannot encode and objc2
  hands the pointer to `slice::from_raw_parts`. A real keyboard never
  produces one; the driver that did was fixed to post the pair. Theirs
  (objc2), and only reachable from a poster.

## 0.1.0-alpha.13 (2026-09-15)

**What breaks.**

- `KUI_ABI_VERSION` is **17**, from 16, for three [in] appends under the
  withdrawn rule (backlog K4): `KuiTextStyle` and `KuiSpan` gain
  `underline_color` and `underline_style` at their ends, `KuiCell` gains
  `ul` — two of them array elements, so the stride moved. A zeroed field
  is what the struct meant before; recompile. A positional `KuiSpan`
  initializer (`{text, color, flags, bg}`) still compiles and now warns
  under `-Wmissing-field-initializers`; the designated form does not.
  `KUI_UNDERLINE_SOLID` / `_WAVY` / `_DOTTED` and `KUI_CELL_WAVY` /
  `KUI_CELL_DOTTED` are new constants.
- The Node binary frame is **version 13**, from 12 (backlog K4): a span
  carries a third colour slot, its underline's, with flags bits 256, 512,
  1024 and 2048; a cell carries a fourth slot, its underline colour.
  Encoder and addon ship together, so nothing to do unless you own an
  encoder. `<cells cells>` takes four *or five* entries a cell.
- `kui_core::Cell` and `TextStyle` / `Span` gained fields
  (`ul`; `underline_color`, `underline_style`): a Rust struct literal adds
  them, or uses the constructors (`Cell::new`, `TextStyle::new`,
  `Span::new`) as everything in this repository does. Nothing else about
  the types moved, and `underline()` still means what it did.
- `UiEvent` gained a field, `slot: Option<Key>` (backlog K2): a Rust host
  or test that builds one as a struct literal adds `slot: None` (or uses
  `UiEvent::on`, which sets it). Nothing else about the type moved.
  `KuiEvent` grew `uint64_t slot` at its end under the [out] rule — no
  bump, a host reserving the older layout never sees it — and Node's
  `pollEvents()` objects and Lua's `on_event(ev)` tables carry it as
  `slot`, so a handler that pinned an event's exact key set adds one.
- A `PumpRunner` whose `pump()` has returned false has **dropped its
  windows** and parked the event loop for the next runner on the thread
  (backlog F58); its core stays readable. A host that kept a window on
  screen by holding a finished runner until process exit does not any
  more, and one that wanted a second `Launcher::open` in the same
  process — refused with `EventLoop can't be recreated` until now — has
  it. The pump path no longer asks winit to exit at all; `Launcher::run`
  is unchanged.
- The windowed Node driver's backoff no longer resets on a tick's own
  frame (backlog F57): a stopped app whose once-a-second tick returns a
  model idles at `idlePumpMs` between digits, where it used to spend the
  `quietMs` after every digit back at `pumpMs`. The next tick is a
  deadline the pump sleeps to regardless, so nothing ticks later; what an
  app paying for that busy half-second got from it was a click answered
  within 8 ms rather than 32 in the half-second after a digit. Measured
  on a real window: 67 → 29 pumps a second, the same tick.

### Added

- **An underline of its own colour and shape** (backlog K4, the wish
  parked since the third editor-and-mux round; kawoosh's M4 and M6 asked
  for it): `underlineColor` / `underline_color` and `underlineStyle` /
  `underline_style` — `solid`, `wavy`, `dotted` — on a text's style rows
  and on a `<span>`, each implying `underline`; on a cell, the underline
  colour as a fifth entry (JSX), a seventh run entry (Lua), `KuiCell.ul`
  (C) and `Cell::ul` (Rust), with flag bits 32 (wavy, a terminal's SGR
  4:3 undercurl) and 64 (dotted). In Rust, `TextStyle::underline_color`
  / `underline_style`, the same two on `Span`, `Cell::underline_color`
  and `cells::flags::WAVY` / `DOTTED`. A solid line is the one rect C22
  drew; a wave (three strokes tall, a six-stroke period) and dots (two
  strokes across, four apart) are runs of the segment primitive a `line`
  draws — `kui_core::deco` — so no backend, header or protocol learned a
  quad kind, and the cost is quads: two a period. The `underlines`
  corpus scene pins the shapes in four adapters by their segment count;
  `syntax_view` marks its counter's unread field with a red wave and
  `cells` undercurls a word. Not built: a `QuadKind` of its own, which
  the entry names as the next step if a screenful of diagnostics ever
  shows in a profile.
- **`slots = { "*" }`** — an extension whose slot names are not known
  when it loads (backlog K1, [ADR 0014's amendment of
  2026-09-15](docs/adr/0014-slots-an-extension-fills-in-place.md#amendment-a-wildcard-for-slots-not-known-at-load-2026-09-15)):
  `kui_core::ANY_SLOT` in `Extension::slots`, the same string in a Lua
  script's `slots` global and a C plugin's `kui_ext_slots`, matches every
  name the host declares under the namespace — a `root` declared
  explicitly among them, since under a wildcard it is one more name and
  not the auto-fill — and raises no `unknown-slot`, there being no list
  to check. The first consumer built on the Lua binding registers views
  from `init.lua` at runtime, one slot per (view, pane), and was
  bypassing the runner's list with a `Core::fill` of its own to do it.
- **An event says which slot its node was filled into** (backlog K2):
  `UiEvent::slot`, the slot's key — what `key_of` / `keyOf` /
  `kui_key_of` answer for the slot's full name — or none for a node the
  host drew; `ev.slot` in Lua is the full name the script was handed.
  One extension fills many slots, so `origin` could not say which pane a
  click was in, and the alternative was the extension stamping a pane id
  onto every handler payload it built. Stamped by the core on the way
  out, like `window`, from the node ranges each fill recorded
  (`Tree::fills`, a side list: a frame with no extension records none
  and pays one emptiness check per batch); a reply keeps the slot of the
  event it answers; a fill inside a fill reports the inner one.
- **A second `runWindowed` in one process** (backlog F58), and a second
  `Launcher::open`: winit builds one event loop per process, so a runner
  whose main window closed parks it and the next `open` on the thread
  takes it back. What made it work is not in the runner: the second
  window's view got no `NSTextInputContext`, because AppKit will not
  create one for a view answering `isEditable` NO, and the runner's
  override of that selector (W15) answered NO for a view it had not yet
  registered — the first window's view was born before the class was
  patched. It answers YES for an unregistered view now. Node's
  `features/relaunch.tsx` reopens its window under the other chrome;
  the harness gained `after`, and the windowed smoke round runs both.
- **`tick.msg(now, every)`** (backlog F59): the cadence the tick fired
  on, beside the time — the bound on how stale `model.now` may be by the
  next press. A bound, not the time: the pomodoro's answer, that a press
  anchors nothing and the next tick writes the anchor from its own
  `now`, stays the right one; this is for the model that wants to know
  which reading it holds.
- **`KeyMsg.physical`, `LayoutMsg.scale`, `ForceClickMsg`, and a `menu`
  row in the event table** (backlog F55). The first two were fields the
  runtime object carried and `index.d.ts` did not (the F37 class); the
  `menu` event (ADR 0017) and the `forceclick` one had no `EventDef` at
  all, so `props.md`'s event table and the Lua and C docs never listed
  them, and `forceclick` had no message type. `test.mjs` pins every
  message type against the payload shapes `schema::EVENTS` documents,
  both ways, and every `CoreMsg` member against a row of the table — the
  pin found the four before it was green.

### Changed

- A `KuiWindow.access()` hands the sound the click asked for to the
  device before the frame it asks for (backlog F56), the order the
  runner's own input path has always kept: `env.audio.live` used to read
  0 on that frame and the sound started a paint late — the "counted 35
  ms late" of the pomodoro's smoke trace, and a third of its race.
- A shifted letter is documented (backlog F60): `code` is the
  upper-case letter with `shift` set, as the OS spells it — `Z` with
  `physical` `z` for ⇧⌘Z — and a headless press is spelled the same way,
  since no door re-spells it. `press("z", { shift: true })` is a chord no
  keyboard produces; the mind map's redo was green over it for six
  releases and had never worked from a keyboard.
- `docs/adr/0014` amended (K1, above); alpha.12's **What breaks** list
  gained the `{ percent }` line after the fact, and this file's charter
  the rule it stands for (backlog F61): a fix that changes what an
  existing input draws is a break for whoever wrote to the old
  behaviour, documented or not, and is listed under both.

### Fixed

- The windowed Node driver's pacing is a value with a test
  (`runWindowed`'s `pacer`), where it was four variables in the pump.

### What you can delete

**The NBSP a monospace editor mapped its spaces to** — `s.replace(' ',
"\u{a0}")` in front of every run, and the byte arithmetic mapping a
pointer's `byte` back through the two-byte stand-in (backlog K3): a
run's spaces measure at the face's advance, leading, repeated and
trailing, unwrapped — `crates/kui-core/tests/measure.rs` pins `"ab  "`
at four cells and the NBSP spelling at the same width. What does drop a
trailing space is a *wrap*, which hangs it past the break; a row of runs
never meets that. `modal_editor` and `syntax_view` carried the mapping
since alpha.9 with a comment blaming fonts, and carry it no more.

The `_slot` an extension stamped onto every handler payload so its host
could route by pane, and the table walk that stamped it (K2); the
`Core::fill` under a private origin that placed a slot the runner's list
did not know (K1).

The child process a smoke test spawned to run a second windowed
configuration (F58).

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag on
2026-09-15 on `main`, on macOS — the platform alpha.12's round did not
run on. What follows is what executed on what.

**macOS 26.6.2 (arm64, Apple M3 Pro), rustc 1.98.0, Node 26.8.1.**
`cargo fmt --all --check` and `cargo clippy --workspace --all-targets`
are clean. `cargo test --workspace`: **1186 tests over 93 suites, 0
failed** (1 ignored, the devtools' `drive.rs` doc example), from
alpha.12's 1177. The scene corpus runs in all four adapters against one
reference report: **37 scenes** — `underlines` new — Rust and Lua
through `cargo test`, C through `target/debug/conformance` (the header
at **372 fields, 250 enum members and 213 prototypes**, from 366 / 245
/ 213), Node through `npm test` (**163 Node tests, 163 passed, 0
skipped** — the `RTLD_GLOBAL` pin runs here — from 158). The C round,
`cbuild --run`, passes its **six checks** against **ABI 17**, the C
host now asserting the slot a reply names. `npm run gen` regenerated
`props.md`, `jsx-runtime.d.ts` and `index.d.ts` and the tree carries
the result. `npm run typecheck` on `examples/node` is clean. The
headless round, `smoke -- --headless`, passes all **26 drives**.

**The windowed round**, `cargo run -p kui-devtools --bin smoke --
--node`: **32 Rust examples on both bases and the six Node windows, 120
frames each, every one exiting 0 with nothing on stderr** — 70 windows
(`relaunch` new, and it opens two). The C and Lua hosts by hand under
`KUI_SMOKE_FRAMES=120`: `counter`, `host`, `c_panel` and `lua_panel`
each opened a window and exited 0, warning-free — **74 windows over
five hosts.** The AX audit against `accessibility`: **106/106**. Two
things were looked at on screen rather than counted: `syntax_view`'s
red wave under `count` and `cells`' undercurl under `layout` (K4),
captured from real windows; and `relaunch`'s second window under
custom chrome after the first was closed through its close button
(F58).

**What the round found.** One hang, and it did not repeat.

- The windowed round's first run reported `accessibility/light` as
  **HUNG (killed at the timeout)** — the first window of the round —
  and every other of the 69 as ok. Alone, the example drew 120 and 300
  frames on both bases in ~2–3 s each; the full round rerun passed
  70/70. A run of the same round earlier in the session, killed by the
  harness mid-way with its output unread, had left nothing behind that
  `ps` could see. Not reproduced, not filed: the alpha.10 record has
  the same shape for a first window (F13's launch stall), and the
  round's timeout is the guard.
- **The bench guard**, run alone against the alpha.12 tag on the M3
  Pro: every guarded row within ±5% — `deep_nesting_64_levels` −4.9%,
  `frame_10k_rects` −0.8%, `frame_10k_rects_with_access_tree` −2.7%,
  `frame_10k_rects_with_text_and_hits` −1.2% (the row alpha.12's
  Windows round could not read; here ±1.3% run to run, and the answer
  the record deferred to this round is *no regression*),
  `frame_10k_segments` +1.7%, `frame_1k_typical` +0.7%,
  `list_10k_rows_virtual` +1.7% — and nothing unguarded past ±5% with a
  readable spread. The guard was readable (worst guarded spread 2.9%),
  so **the README's table is refreshed** to this run's medians for the
  `frame` rows; the `stream`, `long_line` and `cells` rows stay at
  2026-09-07's.

## 0.1.0-alpha.12 (2026-09-14)

**What breaks.**

- The Node binary frame is **version 12**, from 9 (backlog T4, AR14 and
  AR43):
  a prop id may carry the `0x8000` token tag and its value slot then
  holds a token index, the `pad` shorthand gains a second mask and
  `border` a flags word when tagged, and a span's flags gain bits 64 and
  128 (v10); a tagged `min` row is one slot, the token index, a `line`'s
  flags word bit 2 says its width slot is a length index, a `cells`'
  cursor-shape slot bit 4 says its colour slot is a colour index, the
  edit op's flags bit 4 is the stock field and `tooltip` is a new op
  (v11); the root's `windows` list rides as one JSON string read through
  `WindowConfig::from_value` (v12). Encoder and addon ship together, so nothing to do unless you own
  an encoder (`createEncoder(p).encode(tree, tokens)` takes the surface's
  token map as a second argument and returns `unknownTokens` beside
  `unknown`).
- `KUI_ABI_VERSION` is **16**, from 15, for one signature: `kui_run_with`
  takes a `const KuiRunConfig *` between the title and the view (backlog
  AR27) — `kui_run_with(ctx, title, NULL, view, on_event, user)` is what
  the five-argument call was, and `kui_run` is unchanged. The bump is
  ABI 12's case (a host that did not recompile passes one argument too
  few), not the struct's: `KuiRunConfig` is new. Everything else is
  additive — `KuiColorToken` / `KuiLengthToken` / `KuiColorOp` /
  `KuiDerivedToken` are new [in] arrays and `kui_tokens_set`,
  `kui_token_color`, `kui_token_length`, `kui_tokens_derive`,
  `kui_selection_ends`, `kui_env_set_always_on_top`, `kui_cell_selection`
  and the six devtools doors are new functions; `KuiSpec` grew
  `on_scroll` at its end (backlog C39) without a bump of its own, under
  the [in] rule as it then stood — ABI 16 covers it (below). Nothing the
  library writes moved.
- **The C ABI's [in] rule is withdrawn** (backlog AR50, ADR 0006's
  2026-09-14 amendment): a field appended to an [in] struct — `KuiSpec`,
  `KuiTextStyle`, `KuiWindowConfig` and the rest of the header's `[in]`
  paragraph — bumps `KUI_ABI_VERSION` from here on, like every other
  layout change. The header and `abi.rs` said such an append was safe
  for a host that did not recompile, "the library reads no further than
  the host wrote"; it reads the whole struct, so the appended field was
  read from past the end of that host's struct — a garbage `KuiStr` in
  `tooltip`'s case. No host in the tree was bitten (each is built
  against the header it links), and no number moves for the rule itself:
  ABI 16 is current and stands over every append the old rule let
  through, so a host that checks `kui_abi_version()` is served from now
  on. `abi_parity::an_in_struct_s_size_is_the_abi_s` pins every [in]
  struct's size, so the next append fails a test until the number moves.
  Nothing to do beyond the recompile ABI 16 already asks for.
- `Ui::token_color` and `Ui::token_length` return `Option` — `None` for
  a name nothing declared or of the other kind, where they answered
  transparent and zero. A Rust view writes `.bg(ui.token_color("peach")
  .unwrap_or(fallback))` or leaves the row alone, which is what a `$name`
  does in every other binding.
- `kui_lua::Refs` is a type alias for `kui_core::NameRefs` (same
  constructor, `take_missed()` where a caller read `.errors`), and a new
  second argument to `kui_lua::parse_props` and its siblings; a Rust host
  that called the Lua parser directly passes `&mut Refs::new(core.token_lookup())`.
- Every f32 prop in `jsx-runtime.d.ts` is `LengthProp` (`number |
  LengthToken`) and `SizingProp` admits a `LengthToken`, which widens and
  breaks nothing.
- The corpus's step vocabulary grew `modifiers N` (Shift 1, Ctrl 2, Alt
  4, Super 8, `KeyMods::bits`) and its event rows print a `scroll`'s
  lines — an adapter you maintain outside this repo needs both arms.
- The `system` event's payload grew a key; a handler that destructures
  the old four still reads.
- **The stock button paints from the theme's accent trio** (backlog
  AR41): `widgets::button_spec(&theme, &metrics)` takes the theme, and a
  plain `<button>` / `button { }` / `kui_button` is `theme.accent` with
  the theme's hover and pressed shades — byte-for-byte the stock blue on
  a host that reports nothing, the app's brand where it set one, and the
  OS's accent where the host reports one, which on a Mac is every stock
  button now, where before only one declaring `accent` was. A Rust
  caller adds `&ui.theme()` as the first argument.
- `Core::nodes()` returns `Vec<NodeInfo>` (rects in the host's
  viewport) where it returned `&[NodeInfo]` (backlog AR36), and
  `Core::finish_frame` is crate-private (AR37): a Rust host that called
  it finishes through `Ui::finish` — `Ui::wrap(core).finish()` from a
  bare core — which also runs the fills, the panel and the open menu it
  was skipping.
- **`{ percent: N }` in JSX is N%** — listed here on 2026-09-15, after
  the fact, from the pomodoro's alpha.12 report (backlog F61). It was
  under `### Fixed` (AR25, below) as the fix it is: the object form wrote
  the number raw where `"50%"` divides by 100, so `{ percent: 50 }` was
  5000% and no doc, example or test said so. But an app that had been
  passing *fractions* to it — `{ percent: 0.74 }` for a 74% row, the
  number the core reads — drew right by two wrongs cancelling, and draws
  0.74% under alpha.12: its cascade rows and slider fills collapsed, two
  headless assertions caught the rows, none the fills. Multiply by 100.
  The rule this adds to the list's charter: a fix that changes what an
  existing input draws is a break for whoever wrote to the old behaviour,
  documented or not, and goes here as well as under `### Fixed`.

### Added

- **A host driving its own audio device can report the other two
  answers** (backlog F36): `kui_audio_truncated(ctx, playback, at)` /
  `kui_audio_refused(ctx, playback)` beside `kui_audio_ended` in C, and
  `ctx.audioTruncated(playback, at)` / `ctx.audioRefused(playback)`
  beside `audioEnded` on a Node `Ctx`. A stop the device found still
  running names the one-shot `audio` node that went away in
  `truncated-playback`; a play it would not take is a `sound` event with
  phase `refused` for a tagged playback and `playback-refused` on the node
  either way — the two warnings that were in every binding's table and
  that only the Rust runner's own device could raise. Two rows in the
  verb table; no ABI bump, two new functions.
- **The verb table** (backlog B1a, on the condition ADR 0020 set and the
  second architecture review met): `schema::DOORS`, one row per verb an
  app or a host calls on its context — a resource registered, a focus
  moved, a selection read, a menu opened, a window sized, the driver's
  half — with its C, Node and Lua spelling, the same thing in another
  form (a prop, a reading, a callback), or the *reason* the binding has
  none, written once. A Lua script is a guest whose env is a reading; a
  Node host never paints and its `KuiWindow` is driven by the runner;
  those sentences were in seven heads and no file. Pinned both ways in
  each binding: kui-ffi holds every C cell to a header prototype and
  every prototype that is a verb to a row, the Node suite the two classes'
  methods, kui-lua `env`'s functions, and kui-core resolves every Rust
  spelling against the sources — so a verb added to one binding is a row
  with its three other cells or a red test. Rendered as the **Doors**
  section of `docs/props.md`. Building it closed the rows that were one
  line each: **`cellSelection()` / `env.cell_selection()` /
  `kui_cell_selection`** (a grid's ends as absolute lines and columns,
  the reading ADR 0017 §4 offered and only Rust had), the devtools
  readers and the inspector in C (**`kui_set_inspect`, `kui_nodes`,
  `kui_devtools`, `kui_devtools_dock`, `kui_set_devtools_theme`,
  `kui_set_devtools_legend`** — `KUI_ABI_VERSION` stays 15, seven
  functions and no struct), and the two element forms JSX lacked:
  **`<input label initial>`**, the stock field Lua's `input { }` and C's
  `kui_text_input` are, and **`<tooltip value>` / `<tooltip>…</tooltip>`**,
  the always-drawn node form beside the hover-gated prop (the edit op's
  flags bit 4 and a `tooltip` op, still frame v11). `protocol().doors`
  carries the table to JS.
- **C's runner takes a window, and the context's registrations reach
  it** (backlog AR27, ABI 16): `kui_run_with(ctx, title, &config, view,
  on_event, user)` takes a `KuiRunConfig` — `width`/`height`, the
  `min_*`/`max_*` bounds (a zero side unbounded, as Node's lone
  `minWidth` is), `chrome` (`KUI_CHROME_NATIVE` / `CUSTOM` /
  `BORDERLESS`, so a `kui_titlebar` no longer draws under a native
  bar), `text_aa` (`KUI_TEXT_AA_*`) and `diagnostics` (`KUI_DIAG_*`,
  the default the build's) — NULL for every default, `KUI_RUN_CONFIG_INIT`
  to start from, and a word this build lacks refused with its reason on
  stderr before a window opens. The context's core becomes the window's:
  a font, image, sound, token, theme, devtools door, `kui_set_native_menus`
  or text-cache budget registered before the call reaches the window and
  the handles keep drawing, where the runner used to open a fresh session
  and a C host that registered first got `foreign-resource` warnings
  from its first frame. The context is left a fresh core, still yours to
  free. In Rust that is `Launcher::core(core)` — open the main window on
  a core you made, its session the app's, with `diagnostics`,
  `KUI_DEVTOOLS` and every `setup_core` landing on top in that order.
  Node's `WindowOptions` gained `textAa: 'auto' | 'gray' | 'subpixel'`
  so the verb table's `Launcher::size` row says one thing in three
  bindings. The slots host opens at 720×480 with a 360 minimum.
- **A stock button takes `index`** (backlog AR40, from the second
  architecture review): `<button index={i}>` / `button { index = i }`
  in a virtual column keys the button by its row, as a box is keyed, so
  it keeps its focus and tweens as the range slides and two rows with
  the same text are two nodes — it warned `unknown-prop` and keyed by
  text before. Declared beside `key`, the index wins. Beside it, two
  refusals where a value was dropped: Lua's `cursor_shape` names the
  three shapes for a word that is none of them (Node's encoder reads
  the same list off the addon now, `protocol().cellCursors`), and `new
  KuiWindow` refuses a lone `width` or `height` and a `chrome` word it
  does not have, before it opens anything.
- **The standard menus, on macOS** ([ADR
  0030](docs/adr/0030-the-standard-menus-the-runner-keeps.md), raised by
  pressing fn+ctrl+F in a kui window and getting nothing). macOS's window
  shortcuts — Fill, Center, the tiling arrows, Return to Previous Size,
  fn+F for full screen — are rows AppKit adds to the application's
  *Window menu*, and fire through it; a process without one registered
  as `NSApp.windowsMenu` has none of them, and until now no kui process
  had one. A process that declares no bar now has the standard bar:
  winit's application menu (kept, Services and all), an **Edit** menu
  (Undo, Redo, Cut, Copy, Paste, Select All) and a **Window** menu
  (Minimize ⌘M, Zoom, Enter Full Screen, Bring All to Front), the latter
  registered so AppKit fills in Fill, Center, Move & Resize and the
  window list. A declared bar stays exactly what the app declared, and a
  menu it titles `Window` is registered as the platform's, the way a nib
  is read — it gains Enter Full Screen, the one row AppKit adds only at
  launch, and AppKit's tiling rows above the app's — and that one menu
  is validated like the standard bar's, so Enter Full Screen reads *Exit
  Full Screen* inside one and Remove Window from Set greys, while the
  app's own rows keep the enable state they declared (a review of the
  branch found the declared menu's rows never retitled). The Edit rows
  are *chords, not roles*: choosing Copy replays ⌘C — the press to the
  key-focused sink, the runner's clipboard half, the release — so an app
  that binds ⌘C itself hears it from the menu too, and an editor copies
  through the code the key takes. Popup windows stay out of the Window
  menu's list. The `menu_bar` example declares a `Window` menu with one
  checked row to show the rule.

- **Select All in a virtual list is the data, not the built rows**
  (`rowCount`, from running the standard bar: ⌘A in the clipboard
  example's log selected the four lines on screen of two thousand). A
  virtual list never told the core how big it is — the rows it built and
  two spacers were all the frame said — so Select All had nothing to
  span but the built rows. `rowCount` is the new row (`rowCount` /
  `row_count` / `kui_row_count` / `Ui::row_count`, prop id 100): how
  many `index`ed rows the list has, built or not, declared on the
  container by `virtualColumn`, `virtual_column`, `virtual_rows` and
  Lua's `virtual_column` without a line in the app. With it, Select All
  inside a `selectable` virtual list spans rows `0..rowCount`: an end in
  a built row is that row's run, one in an unbuilt row is placed by its
  index alone (the mechanism ADR 0017 tier 3 already had for a scrolled
  drag), the last row's end spelled `select::ROW_END` (`u32::MAX`) since
  the core never laid that row out — and once the list scrolls that row
  into the built window the end lands in the row's own runs, so the
  highlight at the bottom covers every built row (the review of the
  branch found it collapsing to nothing there) — and the copy is the `selectionrange`
  ask the app already answers, `to.byte` past the row's length meaning
  the whole row (both clipboard examples cut to the row already; the
  contract now says so). The corpus's `virtual` scene declares it in all
  four bindings, so an adapter that drops the row warns and fails.
  `KUI_ABI_VERSION` stays 15: one new symbol.

- **The standard Edit menu greys its rows** (backlog W14, filed and built
  the same day as ADR 0030 — the ADR's "not free" was measured and was
  wrong). The runner reads six facts off the front window's core after
  every event batch — a focused editor's history and selection
  (`EditStore::history` / `has_selection`, both new and allocation-free),
  the window's selection scope or cell grid, and whether a key sink would
  hear the chord (`Core::chord_sink`, new) — packs them into one byte on
  the bar's target, and `validateMenuItem:` answers from that byte when
  the menu opens or a key equivalent is matched. AppKit's callback never
  touches the core, which was the objection; the cost is a handful of
  `Option` reads a batch. A row stays lit while a sink would hear its
  chord, since a sink may bind it to anything.

- **The Emoji & Symbols palette and Dictation type into a kui window**
  (backlog W15, checked by hand on macOS 26.6 and built the same day).
  AppKit appends Writing Tools ▸, AutoFill ▸, Start Dictation and Emoji
  & Symbols to any bar's `Edit` menu — the standard one and a declared
  one alike — and ADR 0030 guessed two would type into a focused
  editor. Driven, none did: the palette opened at the caret and the
  emoji picked in it never arrived, because winit's view commits an
  `insertText:` only inside an IME composition; Start Dictation did
  nothing, because dictation starts only against a view answering
  `isEditable` and a real `selectedRange`, and winit's answers neither
  (measured on a bare `NSTextInputClient` view — with both it starts);
  Writing Tools opened a panel whose every tool did nothing, since the
  view cannot hand back the selected text; AutoFill greyed itself. The
  runner now overrides those three selectors on winit's view class at
  runtime and adds the fourth (`mod macos_text_input` — Apple's
  protocol selectors, not winit's names; installed once, on the class
  in the chain that defines each, which is not the view's own since
  AccessKit subclasses it): winit's implementation runs first, and an
  insert that arrives with no marked text and outside a `keyDown:` —
  the palette's, ⌃⌘Space's, dictation's — is dispatched as
  `InputEvent::Commit` to the window's key target, the channel a
  composition's commit already takes, so a stock editor types it and a
  sink hears a `text` event; `isEditable` and `selectedRange` answer
  from facts the runner stamps per frame (editable while the core has
  an IME anchor, the caret and selection as UTF-16 offsets into the
  focused stock editor's text). Verified by hand: an emoji picked from
  the menu lands in the field and in the document at the caret, and
  Start Dictation flips to Stop Dictation with the microphone popover
  over the caret. The two rows that cannot work are removed after every
  `setMainMenu:` (`macos_menu::trim_edit`) — a declared `Edit` is now
  its declaration plus the two rows that work. Still winit's: a
  replacement range is ignored (its own TODO), so a dictation revision
  of words already committed appends rather than replaces. This is the
  first place the runner reaches into winit rather than around it; a
  winit whose view stopped defining one of the selectors is skipped at
  install, not crashed.

- **A held drag follows its scroller, and Shift extends** (backlog C39,
  [ADR 0029](docs/adr/0029-a-selection-follows-the-pointer-past-the-edge.md),
  found by running the clipboard example after C38). Three gestures every
  text UI has and no selection in kui had — a stock `edit`'s, a
  `selectable` scope's or a `cells` grid's alike. *Past the edge:* a
  press held outside its scroller scrolls the nearest scrolling ancestor
  toward the pointer at 10 px/s per px past (capped 100 px out), a frame
  at a time on the driver's clock (a sixtieth a frame with none, so a
  headless drive steps by a knowable amount), and `animating()` is true
  while it does, so the runner keeps drawing with nothing in the queue.
  *Following:* the live end is placed again where the pointer is
  whenever the layout under it moved — that nudge, a wheel notch under
  the held press, a virtual list re-slicing its rows — against the
  finished frame, at the start of the next, and once more on the release
  so nothing lands a frame behind; the gate is the offset the last
  layout *placed the content at*, not the store's, since a notch writes
  the store a frame before the text moves. *Shift:* a press with Shift
  held inside the scope, grid or focused editor the selection is in keeps
  its anchor and moves the live end, by characters whatever the click
  count, and clears nothing; anywhere else Shift is a press. A caret
  drag marks the caret moved the way a keyboard motion does, so a
  single-line field scrolls its own text toward a drag past its end at
  once; a scroller above a document reveals the caret on the release,
  not under the held pointer, where the rate above is what moves it.
  The `selection-extend` and `selection-scroll` corpus scenes pin the
  anchor, the rate on both clocks, the cap, the clamp and the re-hit in
  four adapters; `Core::selection_ends` / `ctx.selectionEnds()` /
  `env.selection_ends()` / `kui_selection_ends` read the directed pair
  back — the anchor and the focus as a virtual row's index and a byte —
  which is what says a Shift-press kept it.

- **`onScroll`: the wheel as a message** (the same round, decision 4).
  A node declaring it hears `{kind:"scroll", x, y, dx, dy, lines, tag}`
  instead of anything scrolling — `dx`/`dy` the delta in logical px as
  the driver reported it, `x`/`y` the pointer — and takes the notch from
  any scroller above it, while a scroller inside it still wins, by paint
  order like any scroll region. On a `cells` grid `lines` is the whole
  lines the delta covers, positive toward later history (the sign
  `originLine` grows in), the fraction carried per grid to the next
  delta — a notch's or an edge step's, one carry — so a trackpad's small
  steps add up; on any other node it is null. A
  grid is one screenful of history the core does not hold, so this is
  also where its edge drag lands: a drag-select held past a grid's top or
  bottom edge arrives once a frame with the lines that frame scrolled by,
  the app re-declares `originLine`, and ADR 0017's absolute lines keep the
  selection's ends where they were. A terminal built on `cells` gets
  scrollback under the wheel for the first time by it — the larger gain,
  and not what the example was opened to find. The `cells-scroll` scene
  pins the lines and the carry; `examples/rust/widgets/cells.rs` scrolls
  its session through the row instead of two buttons.

  *What you can delete:* a `setScroll` an app called from an `onDrag`
  `move` when the pointer left the list; the buttons or key bindings a
  terminal pane offered because the wheel over it scrolled the column it
  sat in or nothing; and the "click again to extend" a list grew for want
  of Shift.

- **A key sink has a clipboard** (backlog C33, from the third
  editor-and-mux round of 2026-09-13). The runner performs the clipboard
  chords itself only while an editor or a selection scope has focus; a
  sink hears the raw `Ctrl-c` / `Ctrl-v` and brings its own bindings —
  and had nowhere to bind them to, since the only ways onto the system
  clipboard were a menu's Copy and Paste. Now the two actions those queue
  have a door: `ui.set_clipboard(text, html)` in Rust
  (`Core::set_clipboard`), `ctx.setClipboard(text, html?)` / `win.…` in
  Node, `env.set_clipboard(text, html?)` in Lua and `kui_set_clipboard`
  in C queue the `SetClipboard` a Copy does, and `ui.request_paste()` /
  `requestPaste()` / `env.request_paste()` / `kui_request_paste` the
  `Paste`. A paste comes back as `InputEvent::Commit`, the one committed
  text the core already routes to a focused editor as typing and to a
  focused sink as `{kind:"text", text, tag}` (alpha.9's IME commit) — so
  an app that owns its text inserts a paste with the arm it already has
  for an IME, never sees the clipboard any other way, and the read stays
  on the driver's side where the permission lives. The runner drains
  the two after every frame as well as after every input, since a view
  is where `Ui` is. Three things moved with it: the runner delivers a
  menu's Paste as a commit too (it was `Text`, which never reached a
  sink; an editor reads both the same), a commit with nothing focused
  reaches a root sink the way an unclaimed key does (ADR 0022, decision
  8), and `text_hit` clamps a point into the run it chose, so a press in
  the padding above a line's text no longer answers byte 0 whatever the
  x. `modal_editor`'s `y`, `dd` and `p` are on it — `pbpaste` reads the
  yank, and a paste from another app lands at the caret.

- **A custom editor has a mouse** (backlog C34, the same round). A press
  or drag inside an `onKey` sink that draws `role="line"` rows — the
  shape the access tree and the IME anchor already read — carries three
  more fields on its `drag` payload (and on a map `click` payload), the
  way a `cells` grid's carries `cell`: `line`, the ordinal among the
  sink's lines in the numbering its `access` events use; `byte`, where
  the point falls in that line's text, what `textHit` would have
  answered a frame later; and `clicks`, the press's driver-measured
  count. Click-to-caret, drag-select and double-click-word are then
  `on_event` arithmetic in every binding, with no query, no `Ui` and no
  frame of lag — and no timer the app keeps for the count. A point above
  the first line is the first, below the last the last, one in a
  `role="none"` gutter the line beside it; a sink with no lines adds
  nothing, and neither does a click the keyboard or assistive technology
  made (Enter on a focused row, an AT `click`), which has no point and
  no count — only a pointer's press carries the three. `modal_editor`
  gets all three: one click places the caret,
  two select the word, three the line, and a drag extends from the press.
  Pinned in four bindings (`tests/sink_pointer.rs`, `test.mjs`, the Lua
  and C suites); `access::lines_under` is the one walk both the access
  tree and the payload use, so the two numberings cannot drift.

- **A custom editor's caret blinks** (backlog C35, the same round). The
  blink clock armed only while a stock editor held focus, so a sink's
  caret — the node `modal_editor` drew as a bar or a block — was solid
  forever, and the two ways around it were both wrong (a `keyframes`
  loop asks for a frame every vsync and never stops; a thread on the
  `Waker` is Rust-only and blinks in the background). The core already
  knew the caret: the `line` under the focused sink that declares
  `caret`, which anchors the IME. It now remembers it across frames
  (`Core::has_caret`, `Core::caret_stamp` — bumped when the offset or
  the line changes, summed with the stock editor's), the runner arms the
  same clock on it and re-arms solid when it moves, and the phase is
  readable: `ui.caret_visible()`, Node `caretVisible()` (and
  `setCaretVisible` for a headless test), Lua `env.caret_visible`, C
  `kui_caret_visible` / `kui_set_caret_visible` / `kui_has_caret` /
  `kui_caret_stamp` for a host with its own window. A view draws its
  caret node on the on phase and skips it on the off, keeping the
  `caret` row either way — the stock editor and the custom one blink in
  step, and in a window without the keyboard neither does.
  `modal_editor` blinks. `caret_visible` is an `ENV_FIELDS` row, so the
  Lua env, `props.md` and the C parity carry it.

- **The three app-shaped examples drive themselves** (backlog C36, the
  same round). `modal_editor --headless`, `splitmux --headless` and
  `syntax_view --headless` printed "no headless drive" and exited 0; the
  smoke round ran them windowed for 120 frames, which pinned that they
  draw, not that `hjkl` moves the caret or `Alt-v` splits — that was
  checked by hand with real keystrokes each round. Each has a drive now,
  in the headless roster: the modal editor's keymap (`jjj ww v lll`,
  `dd` + `p` through the clipboard, `:help`), its mouse and its blink;
  the mux's chords (five panes on tab 1, a tab, a jump, a close); the
  code view's `j` / `G` / `k` / `tab`. And `cells.rs` no longer promises
  a grapheme cluster its `char` cannot hold (backlog C37): a cell is one
  scalar in every transport, and the doc names the side-table shape a
  cluster would take.

- **The clipboard has an example, and an article** (backlog C38, the
  same day). `examples/rust/features/clipboard.rs` and
  `examples/node/features/clipboard.tsx` show the four ways onto the
  host's one queue — the runner's chords in an `edit`, a `selectable`
  scope's stock Copy with the HTML beside, a selectable virtual list's
  `selectionrange` ask answered from the app's rows, and an `onKey`
  register's own `y` / `p` through `setClipboard` / `requestPaste` —
  and the two ways a paste lands, each pinned by a headless drive that
  reads the queue. `docs/howto.md` gained "How does copy and paste
  work?", the map of it. The `selectionrange` event, which only a test
  and the C header had ever named, is in the events table now and in
  Node's `CoreMsg` as `SelectionRangeMsg`.

- **Always on top** (backlog C30). A window an app wants kept above every
  other app's — a floating palette, a picture-in-picture player, a timer,
  a pinned note — can now ask for it, and the ask has the title's shape:
  a per-frame fact the driver diffs and applies on change, free on every
  frame it does not change. `ui.always_on_top(true)` in Rust
  (`Core::set_always_on_top`), a root `<box alwaysOnTop>` in JSX,
  `always_on_top = true` on a Lua root table, `kui_set_always_on_top(ctx,
  bool)` in C. Not a `WindowConfig` field: the thing every app that wants
  this draws next is a pin button, which is a toggle, and a config is read
  on the opening edge and never again. So the fact defaults to **false**
  rather than "leave as-is" — a frame that stops declaring it is what
  lowers the window, and the button toggles the app's own flag and undoes
  nothing. The runner applies it through `set_window_level` once per
  change (`NSWindowLevel` floating, `HWND_TOPMOST`, `_NET_WM_STATE_ABOVE`),
  never per frame, and never to a popup: a popup opened `AlwaysOnTop` by
  construction and keeps its level whatever its owner declares, so a pinned
  owner's dropdown stays over it. What the platform did is a new env fact,
  `env.window.always_on_top` (`WindowEnv::always_on_top`, Node
  `window.alwaysOnTop`, Lua `window.always_on_top`, C's own setter
  `kui_env_set_always_on_top`) — the runner's record of the level, and
  false on Wayland however often the app asks, since winit has no level
  there. That is the reading a pin button draws from, and the reason the
  fact is reported at all. Two readers for hosts driving their own window:
  `kui_always_on_top_get` and Node's `ctx.alwaysOnTop()`. The corpus's
  `chrome` scenes declare it and every report carries an `always-on-top`
  line, so the four bindings are held to one ask; the titlebar example
  gained the pin, checked on Windows (`WS_EX_TOPMOST` set on pin, cleared
  on unpin, the fact following both). **`KUI_ABI_VERSION` stays 15**: the
  three C doors are new functions, and a new function is off the version
  by the header's own rule (a host that never calls one is unaffected; one
  that does fails to link, loudly) — the entry had it moving to 16, which
  the rule does not ask for.

- **Tokens beside the theme**
  ([ADR 0027](docs/adr/0027-tokens-beside-the-theme.md)). The app's own
  named colours and lengths, beside the theme's twenty-three roles and
  the metrics' sixteen: a colour token has a light and a dark half the
  core picks by the appearance in effect (the same value twice for one
  that does not follow it), a length token is logical px. Declared whole
  — `ctx.setTokens({ colors: { peach: '#ffcc99', ink: { light, dark } },
  lengths: { sideW: 132 } })`, `Core::set_tokens(Tokens::new().color(..)
  .length(..))`, a `tokens = { colors = …, lengths = … }` global in a Lua
  script (or `env.set_tokens` from a view), `kui_tokens_set` in C — into
  a **table per origin**, so an extension's names are its own and a
  guest cannot shadow its host; a lookup reads the running origin's table
  then the host's. **Referenced by name in any colour or length prop**:
  `bg="$peach"`, `width="$sideW"`, a `pad` edge, a `border`'s width and
  colour, a text's `size`, a `<span>`'s `color` — resolved by the binding
  as it lowers the node, so the core's open path never sees a name; on
  Node's wire the prop id carries a tag and the index rides in the value
  slot, zero extra bytes. `defineTokens(decl)` returns the names as
  branded literal types, so `T.peech` does not compile and a colour token
  in a length slot is a type error (a length token in a colour slot is
  caught at encode time instead: `ColorProp` admits any string). The
  theme's and metrics' roles take the same spelling — `roles.surface` is
  `'$surface'`, `'$radius'` the metric — through a reserved range, so a
  declared token that takes a role's name is refused with
  **`reserved-token`**; a name nothing declared, or of the other kind,
  raises **`unknown-token`** once per name and the slot keeps its default
  (transparent, 0). Read back resolved: `ctx.tokens()`, `env.tokens`,
  `kui_token_color` / `kui_token_length`, `ui.token_color(name)`. C
  declares and reads but its props carry no reference (a bare `uint32_t`
  has no room for a tag). The devtools' facts tab lists every token with
  its swatches, grouped by origin, and the inspector prints a token's
  name after a value it painted — `#ffcc99ff · peach`, `7 · gap`.
  Measured on the way in: the core-side resolve the ADR prototyped read
  `frame_10k_rects` +1.6% at a ±4.8% floor unused and +0.3% used, and
  the built shape resolves in the binding, so the core's own path is
  untouched. The corpus gains the `tokens` scene and an **`appearance`
  step** — the OS appearance moving under the app, which pins the
  `system` event and a themed token's light half across all four
  bindings.

- **Derived tokens** ([ADR 0028](docs/adr/0028-derived-tokens.md);
  backlog T5, what building ADR 0027 left out). A colour token may be a
  **recipe over an earlier token or a theme role** — a source and a
  chain of operations folded over it in order, each a `[verb, …]` tuple:
  `['lift', t]` / `['darken', t]` toward white / black, `['raise', t]`
  toward the front of whichever base is in effect (`Theme::raise`),
  `['alpha', a]`, `['mix', token, t]` toward another token, and
  `['readable', token, ratio]` — `Color::toward_contrast` toward black or
  white, whichever reads on the named colour, until it clears the ratio.
  Declared beside the values — Node `peachHover: { from: 'peach', ops:
  [['lift', 0.3]] }` (a single op may be written bare, `ops: ['lift',
  0.3]`; no `ops` is an alias), Lua `peach_hover = { from = "peach", ops =
  { { "lift", 0.3 } } }`, C `KuiDerivedToken` arrays through
  **`kui_tokens_derive`** after `kui_tokens_set` (`KuiColorOp { op, t,
  other }`, `KUI_OP_*`; false with nothing added for a malformed op),
  Rust `Tokens::derive("peach_hover", "peach", [ColorOp::Lift(0.3)])` —
  and **resolved by the core on read**, so a recipe over a themed source
  runs on the half in effect and a role source follows the theme; a
  derived colour comes back **rounded to eight bits a channel**, so every
  binding paints the value `kui_token_color` reads. A source must be
  declared *before* the token that names it (so the chain is acyclic by
  construction); one that is not — undeclared, a length, a later token,
  the token itself — drops that token at the declaration with
  **`unknown-token`** naming both, and the rest of the table lands. Lua
  sorts its names, so its parser declares values first and derived
  tokens by dependency; Node's `setTokens` gives a dropped token no
  index, so the names after it stay in step with the core's. Nothing on
  the wire changes: a derived token is an index like any other,
  `NodeSpec` and the frame version are untouched. The devtools print a
  derived token's recipe after its hex — `peachHover #ffdbb8ff peach →
  lift 0.3` — and the inspector names it like any other. `ColorToken` is
  an enum now (`Value { light, dark }` / `Derived { from, ops }`);
  `ColorToken::resolve` is `Tokens::resolve_color(i, &theme)`. The
  corpus `tokens` scene gains six derived tokens: a lift, a two-step
  chain that does not commute, a `raise` off the `surface` *role*, a step
  off a derived token, a `readable` against the themed ink that has to
  move on one base only, and the dropped one. `KUI_ABI_VERSION` stays
  15: two new `[in]` arrays and one function.

- **A pin over `env.system`, at the launcher** (backlog F47; the pomodoro's
  `kui-alpha-11.md` wish 4, the third ask in three reports). A window's
  `env.system` is the OS's, written by the runner before every frame — which
  is why `setEnv` on a window was refused in alpha.10 and stays refused: a
  reading pushed at the core is gone by the next view. The reduced-motion
  branch of a view was therefore assertable headless and never *lookable
  at* on a machine whose owner had not asked for less motion. Now the app
  can say, in its own code, what part of the reading it wants pinned:
  Rust `kui::app(..).system(SystemEnv { motion: MotionPref::Reduced,
  ..Default::default() })`, Node `runWindowed(config, { system: { motion:
  'reduced' } })` (the same partial `Ctx.setEnv` takes, so the headless
  assertion and the window read one spelling; also on `new KuiWindow`'s
  options, and `windowOptions(opts)` is the exact object `runWindowed`
  constructs the window with), C by handing `kui_run_with` a context that
  `kui_env_set_system` was called on. The merge is `SystemEnv::over`,
  applied inside the runner's per-frame write: every field the pin knows
  wins, every field left unknown or null is *not pinned* and keeps
  following the OS — a real change to the accent still arrives, and the
  `system` event that reports it carries the pin like any other reading.
  Checked on a real window: the same machine reads `motion: 'full'` bare
  and `'reduced'` pinned, accent, appearance and locale unchanged. The
  two example harnesses take `--motion full|reduced`. An option, not an
  environment variable, for the reason `KUI_SMOKE_FRAMES` is kept out of
  a shipped build: an app you ship should not change its motion because
  of a variable in the environment it was launched from.

- **`env.system.assistive` — whether assistive technology is listening**
  (backlog F48; the pomodoro's `docs/kui-alpha-10.md` wish 4, carried to
  `docs/kui-alpha-11.md` wish 5). The fifth `system` row beside the four
  OS readings, with the same explicit unknown: `"listening"` once an
  accessibility client has asked this window for its tree, `"none"`
  while the bridge is up and nobody has, `"unknown"` where there is no
  bridge — a headless `Ctx`, a runner built without `accesskit`, a C
  host that never called the setter. The one reading that changes what a
  view *says* rather than what it draws: an alert that announces when
  something is listening and blinks when nothing is. The fact was always
  at the driver (AccessKit's `InitialTreeRequested` is what turns the
  bridge on, and ADR 0016 measures its cache from there); this carries
  it to `env` and reports it through the existing `system` event — whose
  payload gains `assistive` — so a retained-tree host hears the change
  without polling. Read it as `ui.env().system.assistive` in Rust
  (`Assistive`, with `is_listening()`), `env().system.assistive` in Node
  (`setEnv({ system: { assistive: 'listening' } })` declares it
  headless), `env.system.assistive` in Lua; push it from C with the new
  additive `kui_env_set_assistive(ctx, KUI_ASSISTIVE_*)` — its own
  setter rather than a fifth argument on `kui_env_set_system`, which
  keeps its prototype and leaves the field alone when a host re-pushes
  the four settings on an OS notification. Two limits, written into the
  row's doc: *any* client counts (a probe, an accessibility inspector, a
  test harness on the AX API and VoiceOver alike ask for the tree, and
  nothing tells them apart), and it falls back to `"none"` only where
  the adapter reports deactivation — which, of the AccessKit adapters
  pinned in `Cargo.lock`, is AT-SPI alone, on the session's accessibility
  bus going away; the macOS *and* Windows adapters in `accesskit_winit`
  0.34 take the deactivation handler and never call it, so there the
  reading rises once and stays for the window's life. The runner asks
  for a redraw when the bridge's activation changes, so the event lands
  on the next frame rather than with the next click. Guards: the schema
  pin (every restating site moved together), a core test that the
  `system` event carries the field on the rise and, where a platform
  reports it, the fall, the ffi setter test, a Node test through `setEnv`
  and the event, and the C header compiled against the Rust layout.

- **A field folds when it says `wrap`** (backlog F44, from the mind map's
  alpha.11 report). A single-line `<edit>` with `wrap="word"` or `"glyph"`
  declared lays its draft out to its width the way a document does — two
  lines in a 209 px box for a two-line label — and keeps a field's
  keyboard: Enter submits, a newline is never admitted by any door (seed,
  `setEditText`, paste), the caret opens at the end, and nothing scrolls
  under it. `wrap="none"` is a mode, not a request: that field takes one
  line and scrolls it, exactly as one that never said. In Rust it is
  `EditOptions::wrap: bool` beside `style.wrap`; in C `KUI_EDIT_WRAP` on
  `kui_text_edit`'s flags, since `KuiTextStyle.wrap`'s zero is `WORD` and
  the style alone cannot say it was declared. A `multiline` editor wraps
  between words as it always has, whatever its style says. With
  `width="fit"` and `maxWidth` the field hugs a short draft, stops growing
  sideways at the clamp and grows down from there — on the frame that lays
  out the keystroke, since the core measures the draft and not the width
  the app declared last frame. The corpus's `controls` scene seeds its
  editor past its box with `wrap` in all four adapters, so a binding that
  dropped the row would lay the draft on one line and fail the digest;
  `tests/field.rs` pins the fold, the submit, the three doors, the mode,
  the fit case, and the re-measure when a document turns field; each
  branch was mutation-tested. Not built, and still filed under F44: a
  `multiline` editor that submits on plain Enter (`submit="enter"`).

- **`tick.every` may be a function of the model.** `every: (m) =>
  m.endsAt ? 16 : 1000` ticks at frame rate while a countdown runs and
  once a second while it is stopped (backlog F46, from the pomodoro's
  alpha.11 report, wish 1). The loop reads it after `init` and after every
  `update`, a tick's own included; when the answer changes, the next tick
  moves to the last tick plus the new value — keeping the beat, and not
  letting a tick already queued a second out stand — or, when that is
  already past, to the new value from the change, the way a fresh loop
  counts from its start, so a cadence that shortens after a long quiet owes
  one tick `every` from now and not a burst. `0` or less means no tick, as
  the number does. Headless `advance(ms)` fires the same schedule (a
  change a tick makes is rescheduled from that tick's time, not from the
  end of the span), so it is testable; the windowed driver's budget is
  `max(pumpMs, min(gap, untilTick))` as before, and with the next tick a
  second out the backoff reaches `idlePumpMs` — the pomodoro's stopped
  tier, a pump every 16 ms (7–8% of a core in its report), gets the
  driver's own idle rate, which alpha.11 measured at ~3% for the default
  32 ms and under 1% at 250; not re-measured here. Two guards in
  `packages/kui/test.mjs`: the headless tick count over `advance(3000)`
  after the model stops, and over the injected fake surface, that the
  budget answers the idle gap once the model says 1000. Node-only, as
  `tick` is.

### Changed

- **An empty menu-bar declaration, and a core that draws its own bar,
  now leave the standard bar up on macOS** (ADR 0030) rather than no bar
  at all — `apply(&MenuBar::default())` was `setMainMenu(None)`, which
  took winit's Quit with it. An app that declared a bar and then declared
  none, or the devtools' `menus` toggle in its drawn setting, kept ⌘Q
  only by luck of never doing either.


- **Contrast is the colour's arithmetic, and public:
  `Color::contrast(other)`** is WCAG's ratio (1:1 to 21:1), and
  **`Color::toward_contrast(toward, on, ratio, from)`** is the loop
  behind "an accent that can be seen on this base" — `self` mixed toward
  `toward` in 0.05 steps from `from` until it clears `ratio` on `on`.
  The ratio was `theme::contrast`, crate-private, with a doc keeping it
  off `Color` because "a view that wants the number has
  `Color::luminance` to build it from"; that was true with one caller,
  and by F50 the loop on top of it had been written twice
  (`Theme::ring_for` and the devtools panel's `ink`), the pomodoro was
  computing hover shades by hand with no way to check them, and T5 was
  about to be a third spelling. `Theme` keeps the *decisions*:
  `Theme::front()` (white on a dark base, black on a light one — the one
  fact about contrast that is the theme's), `ring_for` (3:1 on `bg`,
  from 0.35 on dark), and the new **`Theme::ink_for(accent)`** (3:1 on
  `surface`, from 0 — F50's promise, which the panel now calls instead
  of carrying its own copy). `raise` mixes toward `front()`. No colour
  moves: both loops reduce to the one on `Color` with the same steps and
  the same floor, and the corpus and the devtools tests pin the ring and
  the panel's ink where they were.

- **`<audio finish>`'s cost, in the app's units.** The `finish` row said a
  released playback holds one of the device's 128 voices until its file
  ends and the 129th play is refused, and left the reader to work out how
  many releases a second that is (backlog F49, from the alpha.11 upgrade
  reports; carried from pomodoro's alpha.10 wish 3). The row, `props.md`
  and the howto's audio answer now carry the arithmetic: voices held =
  sound length × release rate, so a 1.4 s chime released four times a
  second holds 6 of the 128 at any moment, a 10 s ambience released once
  a second holds 10, every playback still declared (a loop included)
  counts beside them, and a `refused` `sound` event is what arriving at
  128 sounds like. 128 was checked against the driver before it was
  written down again: `AudioManagerSettings::default()` in kira 0.12.4
  gives the main track a `sound_capacity` of 128, and every play lands on
  the main track. The `audio` element's doc in `jsx-runtime.d.ts` is
  hand-written, not generated from the row, so it took one sentence of the
  same by hand. No code changed.

### Fixed

- **`libkui_ffi` names itself `@rpath/libkui_ffi.dylib`** (a soname on
  ELF; found by the QA round of 2026-09-14): rustc's default install
  name was the absolute path of `target/<profile>/deps/libkui_ffi.dylib`,
  which a host linking `-lkui_ffi` recorded as the file to load — a
  build machine's path in a shipped host, and in this workspace a file
  any build of kui-lua or kui-node (kui-ffi without `runner`) rewrote
  without `kui_run`, so `target/debug/counter` died at load with "Symbol
  not found: _kui_run" until `cbuild` ran again. A host now resolves the
  name through its rpath; `cbuild`'s is `target/<profile>/`, whose copy
  is only ever a root build's. A host you link yourself gives its rpath,
  as it would for any other dylib.
- **A window that closes takes its `audio` nodes with it** (found in the
  code review of the round): since AR7 a mount is reconciled against its
  own window's frames alone, so a popup's or a second window's looped
  `<audio>` played on after the window closed — by the OS or by the app
  no longer declaring it — with nothing left to stop it. Both close
  paths now reconcile the window against the nothing it declares: a loop
  stops, a one-shot with `finish` plays out, one without is reported
  `truncated-playback` if the device found it running.
- **A plugin's label lookup is its own** (found by the QA round of
  2026-09-14, on `lua_panel`): `key_of` / `keyOf` / `kui_key_of` and every
  verb that takes a label — Lua's `env.edit_text("filter")`, a focus, a
  scroll — asked from inside a fill are answered from the nodes that
  extension opened and no one else's, and the host from its own first
  (everyone's when it declared none). The Lua panel and the C plugin it
  loads both key an editor `filter`, and the script's read hit both from
  its second frame on — an `ambiguous-key` warning every frame, and the
  first in tree order, which was its own only by luck of the order. A
  guest cannot know what the host or another guest called its nodes;
  labels are unique among siblings, and now among an asker's own.
- **A keyboard selects in a `selectable` scope** (backlog AR28): Shift
  with Left / Right / Home / End on a focused node inside a scope moves
  the selection's focus the way the editor's Shift-motions move its
  caret — a character or a word at a time through the runs in order, to
  the scope's ends — and a scope with nothing selected anchors at its
  start, so Shift-End on a focused label selects it whole. Up and Down
  are not motions; under a key sink the press bubbles as before.
- **The caret follows the keys** (backlog AR29): a custom editor's
  caret — what arms the blink clock and anchors the IME — is the
  enclosing sink's wherever focus sits inside it (a pane button, an AT
  `Focus`, `setFocus`), the way keys and commits already went there; the
  candidates are the editor's lines as the access tree reads them, a
  gutter's line excluded, the last caret declared being the caret.
- **`textHit().line` is the visual row of the node asked about**
  (backlog AR30): counted across every run the key covers by where the
  rows sit — a `line` row of inline runs is one row, a wrapped run as
  many as it wrapped to — where it was the wrapped line within whichever
  run's buffer took the hit; it is not the ordinal `line` node a pointer
  event's `line` names, and the docs in every binding now say which is
  which. A `role="none"` subtree under a `line` (a gutter's number) is
  not the line's text for `textHit` and `caretRect`, as it is not for
  the access tree, so `byte` counts the characters an access event's
  `offset` counts. A text more than four levels below its `line` raises
  `text-beyond-line` instead of answering byte 0 in silence.
- **The enum lists are their enums** (backlog AR42): `Easing`, `Repeat`,
  `Live` and `FontFamily` carry `ALL`, `name()` and `from_index()`, the
  schema's name lists are pinned to them by a test, and an index a build
  lacks reads as the default rather than mapping one off.
- **`index.d.ts`'s `Theme`, `ThemeOverrides` and `Metrics` are
  generated** from the role tables (backlog AR44), docs included; `Env`
  and `NodeInfo` are held to the runtime objects' keys by a test.
- **One paste ask at a time** (backlog AR34): `request_paste` — and a
  menu's Paste row — queues one `Paste` while none is outstanding and
  drops a second until the `Commit` that answers it lands, so a view
  that asks on every frame until the answer comes asks once; the runner
  answers every ask, an empty commit for an empty clipboard. Readable as
  `Ui::awaiting_paste` / `awaitingPaste()` / `env.awaiting_paste()` /
  `kui_awaiting_paste`. The two Rust examples dropped their own guard.
- **A hovered scrollbar is not a hovered row** (backlog AR31): hover
  resolves through the same `target_at` the press and the cursor use,
  so over a bar's track the node beneath drops its `hoverBg` and hears
  `leave`, where a press there would have grabbed the thumb.
- **`Metrics::scaled` leaves the titlebar** (backlog AR35): the row the
  schema marks the platform's keeps its height under a density slider,
  as `compact()` always left it — a 1.5 slider drew a 51 px strip beside
  34 px traffic lights. `scale` in Node's `setMetrics` the same.
- **`nodes()` reads in the app's viewport** (backlog AR36), like
  `layoutOf` and every other readback: under a left dock its rects no
  longer disagree with `layoutOf` by the dock's width. `Core::nodes()`
  returns a `Vec<NodeInfo>` now, not a slice.
- **The devtools' node snapshot follows the tree tab** (backlog AR38):
  the panel asks for it while its tree tab shows or it is picking, and
  stops when neither — a closed panel no longer copies every node every
  frame, and a host's `setInspect(false)` no longer blanks the panel's
  tree for the session. The host's ask and the panel's are kept apart.
- **`Core::finish_frame` is crate-private** (backlog AR37): `Ui::finish`
  is the one door out of a frame; a driver holding a bare core finishes
  through `Ui::wrap(core).finish()`, which every driver did already.
- **A popup is a menu surface** (backlog AR32): borderless whatever
  chrome the launcher asked for, not resizable (AppKit and Win32 both
  kept edge resizing on an undecorated window), and without the
  non-client hit test or the synthesized resize band the app's own
  custom chrome gets.
- **Popup retargeting works in points on macOS** (backlog AR33): the
  drag-into-a-popup arithmetic's common frame is the platform's — points
  on macOS, where winit's `inner_position` is points times that window's
  own scale, physical pixels on Windows and X11 — so a popup on a Retina
  / non-Retina boundary retargets to the right rows; and a popup on
  Windows is positioned with a physical position, which resolves against
  the owner's monitor rather than the one the window is created on.
- **The runner re-finds a pane by id after an input's commands ran**
  (backlog AR39): a chrome close on pane *i* moved the cursor apply, the
  input-time charge and the redraw to the next pane, and a key that
  closed a window was one chrome-shaped key from an out-of-bounds.
- **A font the tests carry with them** (backlog AR48):
  `kui_core::testing::liga_font()` builds a small TrueType face — family
  "Kui Liga", every printable character a square, one `liga` ligature —
  table by table, so the font tests that used to hunt four system paths
  and pass green on a fontless machine register it instead, and the
  ligature effect (`fi` one glyph by default, two with `liga=0`) is
  pinned on every machine, CI's included. `testing` also gained
  `key_press` / `key_release` / `key_down` / `key_up` and `tags`, the
  helpers two test files had re-derived beside it.
- **The corpus declares every generic row, and `animate` has a test**
  (backlog AR47): a `sampler` scene in all four adapters carries the
  forty generic props no other scene spelled — the corner radii, the
  shadow offset, the hover / pressed / focus colours, a hover group, the
  cursor, `selected` / `expanded`, the hover / layout / force-click
  tags, the sounds, `animate`, an eased, delayed, alternating keyframe
  run with an entrance read mid-flight, a window-drag strip, a focus
  region, `max_lines` / `ellipsis` / the decorations / a feature string,
  a caret and a selection anchor — and the Node suite holds every
  generic prop to some scene's source (`font` exempt, with the reason).
  `tests/anim.rs` pins that an `animate` node owes a frame while
  declared and none after; the Node generic-prop test reads thirty-three
  props back off the node snapshot, so a value under a neighbour's id
  fails; `bench-check.sh` guards `frame_10k_rects_with_access_tree` and
  `list_10k_rows_virtual` and takes `KUI_BENCH=<file>`.
- **Every C prototype is called by a C program** (backlog AR46):
  `surface.c` opened with "every prototype in kui.h called once" and
  twenty-two were called by nothing in the tree — a door nobody calls
  can decode its arguments wrong for a release without failing
  anything. The sixty-two the walk lacked are in it now, each with
  something checked (the values' list half, the theme and metrics
  setters, the text cache, font families, image pixels, a fragment's
  whole life, the caret clock, layout and scroll readers, a focus
  region entered and left, a scope and a grid selected whole and read
  back, the owned clipboard, a host-shown context menu, the devtools
  doors, the node snapshot), and `abi_parity::every_entry_point_is_called`
  holds every declared prototype to a call in one of the six C sources
  `cbuild` compiles — no exempt list.
- **The documents agree with the code, and a backlog id names one
  entry** (backlog AR49, from the second architecture review). Three
  ids headed two entries each — `B1`, `D1`, `D2` — and code cited both
  senses; the newer holder of each is `B1a` (the verb surface, above),
  `D1a` (the devtools tree's keyboard) and `D2a` (its drawn icons) now,
  in the backlog, the index, ADR 0024 and every comment, and a test
  holds every heading id across the open list and the archive to one
  entry. The rest were words against code: ADR 0017 was `proposed` with
  its decisions 7 and 8 out of order and is accepted, in order; ADR 0006
  counted four `[out]` structs where the header has eleven; ADR 0021
  ticked two scripts the `smoke` binary replaced; ADR 0028 sketched an
  op over an index where the enum takes a name; `ENV_FIELDS`,
  `SystemEnv` and `docs/props.md` said the core acts on none of
  `system.*` forty lines from saying the stock widgets follow the OS —
  it derives the theme from two of them and acts on nothing else;
  `env.window.always_on_top` was "what the platform did" in four places
  and is the driver's record of the level it set, not a query, which is
  what the runner has; `KUI_WINDOW=WxH` was read by the runner and
  documented nowhere; and this file's own alpha.12 "What breaks" said
  the Node frame was 10, then 9, then "Nothing" — it is 11, in one list.
- **A `dispatch` made outside the loop no longer loses a `setEditText`
  seed to the redraw the call itself asked for** (backlog F42, from the
  mind map's alpha.11 report: `sync: views=1 field=absent`, `frame +1:
  views=2 field=absent`, `frame +2: field=present`, with
  `edit-text-without-editor` raised between, so a reopened editor came
  back with the abandoned draft over the model's text). Two halves. On
  the driver's side, `runWindowed` draws the model a foreign `dispatch`
  changed — from `setup`, a timer, a promise, an effect handler —
  **before** `win.pump()`, so no runner redraw (a caret blink, a hover, a
  resize, or one a call asked for) can re-lower a tree older than that
  `dispatch`; the pump-then-step order for events stays, and `dispatch`
  still does not draw synchronously. On the binding's side, `setEditText`
  asks for a redraw only when the text reached an editor: a seed the core
  held changed nothing on screen, and the frame that will is the app's.
  `Core::set_edit_text` / `set_edit_text_by_label` (and `Ui`'s two) now
  return whether the text landed, which is what the binding reads.
  `runWindowed` takes a `surface` option the way `createApp` does, so a
  test can run the driver itself — pump order and all — over a stand-in
  window; the guard does exactly that, over a stand-in whose `pump()`
  re-lowers its last tree through a real `Ctx`.

  *What you can delete:* an `app.frame()` (or a `setTimeout`) an app
  awaited between a foreign `dispatch` and a `setEditText` so the seed
  would land, and a `setEditText` repeated on the frame after a rename
  opened to paper over the draft that came back.

- **A second window's frame no longer stops the first window's `<audio>`
  node** (backlog AR7, from the second architecture review). The audio
  store is the session's — one device, one queue — but the `audio`
  nodes' mounts inside it were reconciled by *every* core's
  `finish_frame` against *its own* frame: a popup, a second window or
  the popped devtools window drawing a frame with no `<audio>` in it
  stopped the main window's loop (and raised `truncated-playback` for a
  one-shot), and main's next frame, finding it unmounted, started it
  from zero; a `finish` one-shot was released and replayed. A mount is
  now keyed by `(window, key)` and each window's frame reconciles its
  own slice; the same key in two windows is two playbacks. The `ended`
  and `refused` events, stamped `MAIN` by hand because the store did not
  know its window, now name the window whose frame declared the node,
  whichever core the driver folded them back through — the stamp every
  event takes on its way out leaves a window the producer wrote. The
  rule this was breaking is written into `session.rs`'s module doc: a
  session member is a registry keyed by a process-unique handle, a
  revision counter, or a queue any driver may drain; anything reconciled
  against a *frame* is one window's. `Core::playback_of(key)` is new,
  and `SharedAudio::playback_of` takes the window first; `tests/session.rs`
  has the two-window case in both directions.

- **A texture-backed image removed through a window that closes before
  its next frame is dropped from the device, and a removed fragment's
  pipelines are dropped at all** (backlog AR8). `remove_image` pushed
  onto the removing core's own list, which only that core's next frame
  forwarded as `dropped_textures`, and the other windows' atlases kept
  their slot for the handle until their own reset; `remove_fragment`
  freed nothing on the device — kui-wgpu's `fragment_pipelines` was keyed
  by handle and never evicted, so an app registering fragments over its
  life leaked one pipeline per handle per surface format. The removed
  ids are the session's now, beside `fonts_rev`, drained onto the next
  display list *any* core builds (the device is shared, so one window's
  list is enough), with `DisplayList::dropped_fragments` new beside
  `dropped_textures` and the backend dropping the pipelines it names;
  and an `images_rev` every core checks its own atlas against, so a
  removal through one window evicts the slot in every window's atlas at
  that window's next frame (`GlyphAtlas::retain_images` / `has_image`).
  A host rendering the display list itself through Node or C still
  hears of neither drop — the lists were never exported — which is filed
  rather than built here.

- **The glyph atlas grows for a working set larger than its page, and
  the frame that reset it asks for the next** (backlog AR19). On a full
  page the atlas reset, then grew only if the *one* item still did not
  fit an empty page — so a set of items that each fit (three 800×600
  atlas-backed images, a code view with many sizes plus CJK and emoji)
  never grew the page: every frame reset mid-emit, every text template
  was invalidated, the quads emitted before the reset sampled the
  overwritten page, and "rebuild next frame" had nothing asking for that
  frame — an input-driven app kept the corrupt frame until the next
  event. A page that fills twice in one frame doubles now (a page's
  worth of new glyphs a frame still resets and never grows), and a frame
  that moved the epoch sets the frame request. Pinned in `atlas.rs`
  (grown once, then still; turnover never grows) and `tests/images.rs`
  (three images past a 128 px page: grown, `animating()` for one frame,
  still after).

- **A device that failed to open refuses a play** (backlog AR20). The
  runner's `apply_one` dropped a `Play` on the floor when the device
  was `Failed` — CI, a container, a muted VM — so a `play(..).tag()` or
  `<audio tag>` neither ended nor was refused, a view sequenced on
  `sound ended` hung, and nothing polled. The play is refused like one
  the device would not take, so the core hears `refused`; the runner's
  audio tests force the state.

- **A submenu is not outside the menu it opened from** (backlog AR21).
  A sub-popup a popup's frame declared has the popup as its owner, but
  the runner's press-outside rule listed every popup but the pressed
  one, so a press in the child dismissed its parent and — for a
  non-activating parent — was consumed, the app stopped declaring the
  parent, the child closed with it, and the row pressed never heard
  the press; keys stopped at the first level and the sub-popup read
  `env.focused == false`. The runner follows the owner chain now: a
  pressed popup's ancestors are not "outside", keys go to the deepest
  non-activating popup, and the whole chain reads as focused together.
  Read, not run — a submenu example is on the by-hand list.

- **A window the OS refused is closed in the registry** (backlog AR22).
  A `create_window` or renderer failure printed and returned, leaving
  the id live: `windows()` listed a window that did not exist, the app
  had heard `opened` and never heard `closed`, declaring the name again
  was a no-op, and `dismiss` / `focus` / `setSize` on the id were silent
  forever. The failure is told to the registry as a close now, the way
  an OS close is, and the app hears `closed`.

- **The owner's modifiers stay current while a popup borrows the
  keyboard** (backlog AR23). `ModifiersChanged` was mirrored to the key
  target alone: hold Shift, open a menu, release Shift while it is up,
  choose — the owner's next key was a Shift chord until the next edge.
  Written to both panes now; the keyboard's state is one fact for the
  pair.

- **A keyboard-started editor selection takes the window's one
  selection with it** (backlog AR24, ADR 0017 decision 1). A scope's
  selection collapsed the editor's, but Select All, Shift+arrows or a
  reader's `setTextSelection` in an editor left the scope's (or a
  grid's) selection standing: drag-select a label, Tab into a field,
  Cmd-A — the runner read the scope first and select-alled *it*, Cmd-C
  copied the label, and two highlights drew. Cleared whenever the
  editor reports a non-collapsed selection after a key or a request;
  `tests/selection.rs` has the reverse case beside the forward one.

- **`{ percent: 50 }` in JSX is 50%** (backlog AR25). The object form
  wrote `v.percent` raw where `"50%"` divides by 100 and the core reads
  a fraction, so it was 5000% — in `SizingProp` and in no doc, example
  or test. Divided now; `test.mjs` pins the two forms to the same bytes.

- **Lua's `env.edit_text` takes a label, and its events say which
  window** (backlog AR26). `edit_text` took the integer key alone,
  against its own doc and every query beside it, so the one Lua example
  kept the key from a `changed` event to work around it — the shape F5
  and F32 removed everywhere else; and the event table set `node_key`
  and `from` but not `window`, where Node's `ev.window` and C's
  `KuiEvent.window` carry it, so a panel drawn into two windows could
  not tell which one clicked. `env.edit_text("filter")` works, the
  event carries `window`, and the example reads back by label.

- **An assistive-technology request obeys the modal and `disabled`**
  (backlog AR18). `handle_access` resolved only `Click` against the hit
  list: `SetValue` set an editor's text and posted `changed` with no
  modal or `disabled` check, `SetTextSelection` / `ReplaceSelectedText`
  the same, the scroll actions called `scroll_by` where the wheel path
  honours an inert region, and a nudge checked `disabled` alone — so
  with a dialog up a reader (or a headless test) edited, nudged and
  scrolled the inert page, and `Focus` on an editor behind the modal
  routed typed text there until the next frame's containment. One gate
  at the top now: a request naming a node outside the modal or a
  disabled one does nothing, which is what the access tree already
  refuses to advertise (ADR 0003 decision 5). `tests/access.rs` drives
  each action behind a dialog and on a disabled slider, red before.

- **`set_focus` on a modal's closing frame stands over the restore**
  (backlog AR17). The focus a modal displaced comes back when it closes
  unless a `keyFocus` edge on that frame says otherwise (ADR 0003
  decision 4, backlog F4) — and an imperative move said nothing: an app
  that closed a dialog from its handler and called `set_focus` /
  `focus('label')` / `ui.focus` to name where focus lands got the
  pre-dialog node instead. Three focus doors, three precedences, one
  frame-end pass. The app's door (`Core::set_focus`, which `Ui::focus`,
  Node's `focus`, `kui_focus` and Lua's `env.set_focus` are) stamps the
  move, the restore yields to the stamp as it yields to the edge, and
  the core's own moves — a press, a Tab, an autofocus, the restore
  itself — go through `move_focus` and stamp nothing, so a click on the
  dialog's own button still hands focus back where the dialog found it.
  The sentence is in decision 4 now; `tests/focus.rs` has the handler's
  move, the view's, and a core move that does not count.

- **A polygon's and a line's `hover_bg` paints, and `accent` reaches
  every door** (backlog AR16). `open_content` ran accent → hover → ease;
  `cells` and `image` ran hover + ease; `text_edit`, `line` and
  `polygon` ran ease alone — so a wedge with `hover_bg` was hit-tracked
  and `is_hovered` and painted its declared `bg` regardless (the test
  titled for it asserted `is_hovered` only), and `accent` was honoured
  on a box and nothing else. This is AR5's defect on the two doors AR5
  did not touch. One `prepare_spec` (accent, hover, ease) is what every
  door's spec goes through now, and one `float_box_for` is the float
  box a line and a polygon used to build in twenty identical lines each;
  `cells_keyed` records its label after the empty-tree check like every
  other keyed door. `tests/hit.rs`'s `hover_is_by_shape` asserts the
  wedge's fill under the pointer, red before.

- **Cut from a context menu posts the editor's `changed`** (backlog
  AR15). `MenuRole::Cut` deleted the selection and queued the clipboard
  action, and posted only `{kind:"menu", role:"cut"}` — every other
  mutation (`apply_text`, `apply_key`, a reader's `setValue`) posts a
  `changed`, and the runner knew it: its Cmd-X path built one by hand
  after `Core::cut_selection` with the comment "A cut is input too". The
  drawn context menu and a native menu's `activateMenuItem` took the
  core path and posted nothing, so a Node, Lua or C app mirroring a
  field through `changed` desynced after Cut. Both paths post it from
  the core now — `perform_menu_item` into the batch the chosen row
  returns, `cut_selection` into the pending events, like a `resize` —
  and the runner's copy is gone; `tests/menu.rs` asserts the event
  beside the clipboard, `tests/editing.rs` the pending one.

- **An unresolved `$token` means one thing everywhere** (backlog AR14).
  Three outcomes before: Rust's `ui.token_color` answered transparent and
  `token_length` zero; Node left the slot out and the core kept the row's
  default; Lua did the same and said why; `diag.rs` and `props.md`
  documented the Rust behaviour as everyone's — so `color="$typo"` on a
  text painted in the theme's fg in Node and Lua and invisibly in Rust,
  `width="$typo"` was `Fit` there and `Fixed(0)` here. Coverage differed
  too: `cursorColor` threw on a `$` in Node and resolved in Lua;
  `minWidth="$x"` and `<line width="$x">` were "bad" in both while
  `color` on the same line resolved; and a `$` in a `keyframes` /
  `enter` / `exit` stop errored the whole frame — the one place a token
  was fatal, against ADR 0027's "any colour or length prop". One policy
  now, in the core: a miss leaves the slot at the row's default as if
  the prop had not been written, with one `unknown-token` naming it.
  `NameRefs` is the by-name resolver every binding shares (Lua's `Refs`
  is it), `keyframes::parse_with` / `enter::parse_with` take one so a
  stop's `$name` resolves through the same lookup, `Ui::token_color` /
  `token_length` answer `Option`, and the three slots reach it: Node
  frame v11 (above) for `min`, the line's width and the cursor colour,
  Lua through its refs. The `unknown-token` row in `props.md` says the
  rule, and every kind's type text shows the `$` form. Tests in
  `tests/tokens.rs`, kui-lua and `test.mjs`; the corpus's `tokens` scene
  unchanged, since its Rust adapter now leaves the missed cell's `bg`
  undeclared the way the other three always did.

- **A container or access row on a `<text>` warns instead of vanishing**
  (backlog AR13). `text`'s element definition admitted every shared
  row, and all four doors lower a text as content plus a style and no
  spec — so `<text live="polite">` (what the `live` doc tells you to
  write), `<text role="heading">`, `<text label>`, `<text onClick>` and
  a Lua `key` on one reached no tree, raised no `unknown-prop`, and
  `live-region-without-name` could never fire for them. The element
  names its rows now (`TEXT_ROWS_JSX` / `TEXT_ROWS_LUA`: the
  `TextStyle` rows and `size`), pinned equal to the schema's
  `Target::Style` rows by a test, so a style row added later is a row
  here or a red test; anything else on a text is the `unknown-prop`
  warning naming the rows it does take, in Node (`checkProps` reads the
  same list) and Lua alike. The TypeScript `TextProps` already refused
  them; `types.tsx` now says so with an expected error. The Node
  examples' `typecheck` had one unrelated error under TypeScript 7 (a
  discriminant narrowed on an index expression in `clipboard.tsx`'s
  drive), fixed in passing.

- **Node's window surface addresses the window it is handed for**
  (backlog AR12). Every `KuiWindow` door but `setViewBinary` — `focus`,
  `editText`, `setEditText`, `isHovered`, `scrollGeometry`, `openMenu`,
  `selectionText`, `setTheme`, `setMetrics`, `setTokens`, the devtools
  setters, input injection — went to the main window's core, and
  `view(model, name, win)` handed every window the same surface: in an
  app with a second window, `editText('note')` for that window's editor
  was `null`, a `setEditText` from `update` landed on main and warned
  `edit-text-without-editor`, `focus('row')` moved the wrong window's
  focus, and every `$token` in the second window's tree missed (ADR
  0027 admitted the token case; nothing documented the rest — C's view
  ctx and Lua's env were already the drawing window's). `useWindow(name
  | id)` is new on both classes: the doors address the window it last
  named, and it answers whether that window is open now (main until it
  is; a headless `Ctx` answers only for main). The loop aims the surface
  itself, so an app that never calls it reads and writes the window it
  is being asked about: at the window whose view it is calling, at the
  window an event came from before `update` (main for a tick or an
  effect), and back at main after each. `PumpRunner::core_mut_of(id)`
  and `window_id(name)` are the Rust doors under it. Pinned in
  `test.mjs` on a recording surface; the two-window `editText` itself
  needs a display and is in the by-hand round.

- **A held key survives Shift moving under it** (backlog AR9). A held
  key was matched to its release — and a repeat to its press — on
  `code`, which the runner builds from the layout's logical key with no
  case folding: hold `w`, press Shift to run, and the OS repeat arrived
  as `W`, a second key held, with `w` stuck down until focus moved or
  the window blurred and a synthetic `up w` fired at the wrong time.
  The WASD case ADR 0002 sells `keyUp` for. `KeyPress::same_key` is new
  and matches by `physical` when the platform reported one, `code`
  otherwise, and both the repeat check and the release use it
  (`tests/keys.rs`: the Shift-mid-hold case, and an injected press with
  no position).

- **Both key channels agree about a chord, and Space under a modifier
  is one** (backlog AR10, ADR 0011 decision 3). `route_key` called any
  of Ctrl / Alt / Super a chord; `KeyPress::edit_event` then folded them
  to `word: alt, doc: primary()`, so the non-primary one — Ctrl on
  macOS, Super elsewhere — was gone by the time the `Key` arm asked
  whether the press bubbled: Ctrl+Enter on a focused button inside a
  sink bubbled to the sink as a chord *and* clicked the button. And
  Space was `Text(" ")` "whatever is held", with the `Text` arm passing
  `chord = false` unconditionally, so Ctrl+Space (an IME toggle, an
  Emacs mark) typed a space into an editor, or clicked a control and
  reached the sink both. The core now keeps the modifiers of the
  `KeyDown` the next input event is the second channel of
  (`Core::pressed_mods`, set by a `KeyDown`, read and cleared by
  whatever follows), and both arms ask that one bit; a `Key` or `Text`
  with no press before it — a test driving one channel, a host's
  editing-key door — falls back to its own `Mods` as before.
  `edit_event` answers `None` for a Space under Ctrl / Alt / Super, as
  for every other chord (Shift+Space is still a space). The
  `tests/key_bubbling.rs` helper derives `doc` the way `primary()` does
  instead of `ctrl || super`, a mapping no driver used — the F6 trap,
  where a test passes on a path the runner never takes.

- **A keystroke into an editor under a sink no longer carries the
  mouse's `line` / `byte` / `clicks`, a `{kind: "click"}` payload is no
  longer skipped, and a `cells` grid's `key` events no longer carry a
  `cell`** (backlog AR11). Two post-passes added fields to events after
  the fact by reading the payload's `kind` against the events table —
  whose editor row was spelled `"changed / submit"` and matched
  nothing, so a stock `<edit>` under a sink that draws `role="line"`
  rows (a shell with a minibuffer) got the three fields on every
  keystroke, resolved from wherever the mouse rested after a walk of
  the whole sink and a `hit_at`; an app whose click payload was
  `{kind: "click"}` got the opposite; and `attach_cells`, a day older
  than its twin, had neither the filter nor the no-press guard, so a
  grid with `onKey` got `cell: {row, col}` on every `key`, `text`,
  `preedit` and `access` event and on an Enter-made click, from the
  cursor's resting place, restating `cell_row_col` by hand. An event is
  pointer-made where it is built now: `Interaction::handle` returns how
  many of the events at the end of its output a press made — a drag in
  any phase, a click on the release — and one `Core::attach_pointer`
  runs on exactly those, giving a grid its `cell` through
  `cell_row_col` and a sink's line its `line` / `byte` / `clicks`
  through one `pointer_point`. The events table's row is two rows,
  `changed` and `submit` (`docs/props.md` regenerated). Pinned in
  `tests/sink_pointer.rs` and `tests/cells.rs`, each red on the old
  pass.

- **A window with one event in its devtools stream idled again** (found
  on screen building ADR 0029: after the first event reached the dock —
  a Cmd-C's `selectionrange` ask, a Shift key's `modifiers` — the runner
  drew ~130 frames/s until the window closed, on main as well). Two
  frame-requesters, each honest on its own: `widgets::virtual_rows`
  corrected its offset whenever `top + pad.t` differed from the laid
  offset, which on a padded list shorter than its box — offset 0,
  padding 6, `top` clamped at 0 — is every frame, and `Core::set_scroll`
  asks for a frame; and the events tab's *follow* pinned the list past
  its end every frame through the same call. Now the widget corrects
  only when a measurement moved the anchor row, and the follow pins only
  when the list's travel changed (the stream grew, a row opened, the
  pane resized) and reads an offset short of an *unchanged* travel as the
  user's wheel. Pinned by a devtools test that counts the frames one
  event asks for (at most two, then none) and a `virtual_rows` test a
  padded short list passes with zero; the on-screen count is flat after
  a Cmd-C. The review of the branch then moved the fix to the mechanism:
  `Core::set_scroll` from inside a view asks for no frame at all, since
  the frame being built is the one that lands the write (the positions
  pass reads the store after the view has run), so the next view that
  writes an offset every frame — a Lua tail-pin, a stock widget — does
  not spin either; between two frames it asks as before. The two guards
  stay as the cheaper path.

- **A held drag past the edge of a scroller at its end idles** (review
  of ADR 0029's branch). The step counted as one whenever the pointer
  was past the edge, so a scroller whose content fits, or one that had
  reached its clamp, kept `animating()` true — the runner at display
  rate — until the release. A step the clamp would undo is no step now,
  and the list at its end under a held pointer asks for nothing
  (`tests/follow.rs`). A window that loses focus with a drag held drops
  the follow too, since the release will not reach it — and its
  modifiers with the held keys, as the `modifiers` event it is, so a
  host that does not resend the state on the way back (winit does; a C
  loop may not) does not leave every later press an extending one.

- **A caret drag past a document's scroller moves at the drag's rate**
  (same review). `EditStore::drag` marking the caret moved put the
  caret's reveal under a held pointer: a caret placed 60 px past the
  edge was revealed by 60 px, every frame, on top of the step — ~3,600
  px/s at 60 Hz against the 600 the rate says, and twice that at 120.
  The reveal now waits for the release when the editor is the one being
  dragged (a field's own text still scrolls at once), which also keeps a
  click-and-hold on a half-clipped line from landing a line lower when
  the focus reveal moved the content under the still pointer. Pinned:
  three frames past a document's edge are three steps.

- **A Shift-press in the scope arms the drag whether or not it moved
  the end, and a Shift-click in an editor is a click** (same review).
  The editor's Shift arm went straight to cosmic's `Drag` and skipped
  what `click` does first — abandoning a live composition and ending the
  typing unit — so a Shift-click at the caret between "ab" and "c" made
  one undo of the three; and the scope's Shift arm left a focused
  editor's own selection standing beside the window's, which
  `set_selection` collapses. Both go through the one arming now
  (`arm_select_drag(.., extend)`, `click(.., extend)`), pinned in
  `tests/follow.rs`.

- **An unbuilt selection end is placed against its own list's rows**
  (same review, of the ordering fix below). The ask ordered a built end
  against an unbuilt row by the lowest row *any* virtual list on screen
  built, so a header selected down to a row that then scrolled away
  answered in the wrong order once a second list stood beside it — and by
  a different rule from the highlight's. One rule now
  (`select::unbuilt_row_is_after`, the scope's last built row), shared by
  `resolve_selection` and `selection_range`; the fallback with nothing
  to compare keeps the drag's order instead of swapping it. Pinned in
  `tests/virtual_selection.rs` against the old rule.

- **A backwards drag-select asks for its range in reading order** (found
  building ADR 0029 / C39). `Core::selection_range` handed the anchor as
  `from` and the focus as `to`, so a drag pressed on a later row and
  released on an earlier one posted a `selectionrange` ask whose `from`
  came after `to`, and an app iterating `from..=to` — both clipboard
  examples do — answered with nothing. The two ends are now ordered before
  the ask: by their place in the frame when both are built, by row when
  neither is, and an unbuilt row before or after every built one
  otherwise — the same placement the highlight uses. The directed pair is
  `selection_ends`, new in the same round. Pinned in
  `tests/virtual_selection.rs` and by a backwards drag in both clipboard
  drives whose answer is the forwards one's text.

- **`Mono` is an upright monospaced face on a machine without Noto Sans
  Mono** (backlog C32, observed in the first editor-and-mux round of
  2026-08-31 and unchanged since: every monospaced glyph — `modal_editor`,
  `syntax_view`, a `cells` grid, any `TextStyle::mono()` — drew in
  `BerkeleyMonoVariable-Italic` on the Windows box). cosmic-text names
  its own defaults, `Open Sans` / `DejaVu Serif` / `Noto Sans Mono`, none
  of which a stock Windows or macOS machine has; sans survives through
  the platform fallback list, but a missing monospace family falls to the
  *lowest-id* monospaced face in the database, and style is not in that
  ranking's key, so whichever face fontdb happened to load first won —
  Menlo Regular on one Mac, an italic instance on another machine. The
  session's font database now pins the three generic families to the
  first installed of a per-platform list — macOS `SF Mono` / `Menlo` /
  `Monaco`, Windows `Cascadia Mono` / `Consolas` / `Courier New`, Linux
  `DejaVu Sans Mono` / `Noto Sans Mono` / `Liberation Mono` / `Ubuntu
  Mono`, and sans and serif likewise — leaving cosmic-text's name in
  place when nothing on the list is present so its fallback still runs.
  The guard shapes `M` under each generic family and asserts the face is
  upright, and monospaced or not as its family says (skipped, with a
  message, on a machine with no face at all); the devtools' facts tab
  gained a `fonts` row naming the three resolved families, so the next
  machine this differs on says so on screen.

- **The devtools panel lives with what the app chose: its base, its
  chrome, its floor** — five defects from the LCARS pomodoro's devtools
  round of 2026-09-12 (backlog F50–F54), all of the class ADR 0024's
  consequences named: the panel is drawn into *the app's* window, and
  three of the five were it assuming the OS's choice over the app's.

  - *The accent is readable ink* (F50). The panel paints the accent as
    strokes, borders and short labels — the stock widgets only ever fill
    with it under `on_accent` — and an OS accent near the base (Windows'
    "automatic" accent off a dark wallpaper is a navy on `#1a1d27`)
    vanished: the accent dot, the lit placement, the picker's border.
    The panel's theme is the app's with the accent moved toward the front
    of the base until it clears 3:1 on `surface`, the promise
    `Theme::ring_for` already made for the ring; an accent that reads is
    painted verbatim, so the ones that were fine are unchanged. The
    accent *swatch* shows the accent in force inside a hairline, so it
    reads as a sample whatever it is, and the facts row prints it.
  - *"The app's own" base is the app's* (F51). With the base toggle at
    "app" and an accent chosen, the override went out as
    `DerivedWithAccent`, which follows `env.system` — so an app that had
    pinned dark on a light desktop flipped light on `Ctrl+Shift+A`. The
    override now keeps the half it leaves alone from the app's own
    source: a pinned palette is recoloured, a following one keeps
    following, and a base override on a brand accent keeps the brand. A
    theme source the app sets *under* an override is what the override
    is lifted back to, and the base toggle's hint names the app's base
    (`base: app (dark)`).
  - *The panel's window has the OS's chrome* (F52). Under
    `chrome: 'custom'` the popped-out panel opened undecorated and
    could not be moved: every window of the app took the launcher's
    chrome, and this is the one window the app did not declare. The
    runner reads the `Open`'s origin — `OriginId::DEVTOOLS` opens native
    — and chrome is per pane now (`custom_chrome`, the traffic-light
    rect, the Windows non-client hook and the synthesized resize band
    all follow the pane's).
  - *`pick` from the panel's window focuses the main window* (F53),
    the mirror of what `Ctrl+Shift+I` does the other way: the crosshair
    is in the main window, so the keyboard for Escape and the pointer
    go there. Docked, nothing is asked.
  - *The window's floor counts the dock* (F54). `minWidth` / `minHeight`
    reached the OS once, at launch, and the dock came out of the app's
    share — a 620×500 floor with a 280 px bottom dock drew the app into
    552 px it had said it could not fit. The new
    `Core::devtools_inset()` is what a docked pane takes, in the axis it
    takes it, and the runner adds it to the launcher's floor after every
    main-window frame, on change, capped by the maximum; popped out or
    off it is the app's floor again. Checked on Windows by asking the
    window `WM_GETMINMAXINFO` across the four placements.

  *What you can delete:* a `minWidth` padded by 340 to leave room for a
  dock the app does not always have, and a `setTheme` repeated on
  `system` because a devtools accent walk had flipped the base.

- **`env().viewport` and `win.size()` are what the dock leaves.** Under
  `KUI_DEVTOOLS=1` with the panel docked, a Node app read its 1040 px
  window from both — `win.size()` in `init`, `env().viewport` once a frame
  had run — sized its tiers to that and was squeezed into the ~700 px the
  right dock left, with everything `grow` absorbing the difference (backlog
  F43, from the pomodoro's alpha.11 report, `docs/kui-alpha-11.md` wish 2:
  "with the devtools docked at launch, the app draws for the whole
  window"). Two readings, one defect each: `Core::env_facts()` filled the
  `viewport` row from the window it was begun with while the row's own
  `ENV_FIELDS` entry said `Core::viewport()`, the frame's — the number
  alpha.11's "the app's viewport is what the dock leaves: `viewport()`
  says so" was about — and `KuiWindow.size()` was the window's inner size
  while the `resize` event's doc promised it "queries the same numbers".
  The getter reads the frame's viewport now, which fixes Node's, Lua's and
  C's `env` at once (and Rust's `ui.viewport()`, which read the same field
  and was not the `Core::viewport()` the entry took it for), and `size()`
  answers with the new `Core::host_area(window)` — what the dock leaves of
  a window that size, computable from the window and the dock's state
  alone, so it is right in `setup` and `init` too, before `env().viewport`
  is anything but 0×0. A dock present at launch still posts no `resize`,
  by the first-frame rule; it is moot now that the number an app read
  before its first view was the dock's. The readback tests that pin
  `ENV_FIELDS` could not see this because no corpus scene has a dock in
  its tree; the guards are a core test with a right dock and a 1040×720
  frame, and a headless `Ctx` in the Node suite under `setDevtools(true)`
  (`KUI_DEVTOOLS` is never read there, the call is). The two schema docs,
  the `WindowSize` and `Env.viewport` types and the README's window-size
  bullet say the same thing now.

- **`runWindowed`'s options type has `clock`.** The driver has read
  `opts.clock ?? Date.now` since the windowed tick bookkeeping was written,
  and `createApp`'s options declared it, but `runWindowed`'s did not — so
  the one line a test writes to move a real window's clock
  (`clock: () => Date.now() + ahead`, which is how a countdown is watched
  in seconds) carried a `@ts-expect-error` (backlog F45, from the
  pomodoro's alpha.11 report, wish 3). F37's class again: a `.d.ts`-only
  gap no test in this repo can see, because CI's typecheck of
  `examples/node` is the only guard and no example passed one. The type is
  fixed, and the examples harness (`examples/node/devtools.tsx`) now takes
  a `clock` on an `Example` and hands it to `runWindowed`, so the typecheck
  exercises the field — removing it from the type fails CI, which was
  checked by removing it. `startTime` stays absent from the windowed
  options: under a clock it is dead. The doc says what the clock moves in a
  window — the ticks and `tick.msg(now)`; the frame clock behind
  `transition` is the runner's own.

- **The corpus's dead image handle is one the fixtures removed, not raw
  1** (backlog C31). The `fragments` scene pins ADR 0025's dead-handle
  rule — a fragment naming an image live in no session draws nothing and
  warns nothing — and spelled that image as raw handle 1, with a comment
  saying no live slot ever has it. It does: `from_ffi` reads every handle
  at an odd generation, so raw 1 is index 1 at generation 1, the first
  key the process's mint hands out, live until the session that minted
  it drops. The reference, Lua and C adapters never noticed because their
  first session is gone by the time `fragments` builds; a Node process
  keeps every session the GC has not collected, so
  `--test-name-pattern` on the corpus test alone saw `warn
  foreign-resource` where the full suite (CI's run) did not. The
  fixtures now register a sixth image and remove it (`Fixtures::dead`,
  mirrored in `conf_fixtures` and `addFixtureDead`), which is what a dead
  handle is, in every process; and a `resources` test pins the half that
  *is* fixed — raw 0 is index 0, the slot the mint never fills, dead and
  nobody's whatever was minted before, which the scene's dead `src` and
  the doors' "no image" both rest on.

- **`npm run gen` writes the same `docs/props.md` on every platform**
  (backlog W13, from the alpha.11 Windows round). The metrics table's
  stock and compact columns were the addon's reading of
  `Metrics::default()` and `Metrics::compact()`, and `titlebar_h` is the
  platform's caption height — `cfg!(target_os = "windows") ? 32 : 34` —
  so the file said `34 | 34` from macOS or Linux and `32 | 32` from
  Windows, a one-line diff that was not staleness and either got
  committed (and failed CI's Linux `git diff --exit-code`) or had to be
  known about and reverted. The row now says what is true:
  `MetricRole::platform` is `Some(PlatformValue { windows, elsewhere })`
  for a metric that is the platform's rather than a density's, the two
  numbers live as `metrics::TITLEBAR_H_WINDOWS` / `TITLEBAR_H_ELSEWHERE`
  and `comfortable()` picks the running one from them, the addon's
  `protocol()` carries the pair beside `stock` and `compact`, and the
  generator prints `32 / 34` in both columns for such a row (both,
  because `compact()` leaves a platform row alone — the schema test pins
  that, and that the pair is what the struct reads here). Nothing shipped
  changes: `Metrics::default()` was right on every platform; only the
  printed table depended on where it was printed.

- **A float layer with no chrome no longer pays a chrome call** (backlog
  C29). The alpha.11 pre-tag round found `frame_10k_segments` +12% over
  alpha.10 with the guarded rows flat, and filed the bisect. A sweep of
  every core commit between the tags (63 of them, two runs each) puts
  the whole step at one commit, ADR 0023's: the row is ten thousand
  one-node `line` floats and each became a layer, with
  `emit_layer_chrome` called at the end of every one — fetching the
  cursor, walking an empty scroll-region slice, and asking
  `emit_focus_ring` to decline. The float pass now skips that call when
  the layer added no scroll region and `focus_visible` is off, which is
  every leaf float at rest: the row reads 796 → 759 µs on the sweep's
  machine (Ryzen 9 9950X3D), about half of what ADR 0023 added; the
  other half — the stack's steady check, the order vector, the
  per-layer loop, ~3 ns a float — is what the design costs and stays.
  `frame_1k_typical_with_100_floats` is unchanged (tooltips with content
  are ~250 ns each; the call was noise inside that), and no corpus
  scene moved. The other three rows the entry named were swept too:
  the clip rows' +3–5% is the clip shrink (priced in alpha.11) plus one
  large merge (`b30cc98`) that moved code rather than added a per-node
  line — `NodeSpec` is 224 bytes on both sides of it — and
  `drop_1k_rows_plain`'s +4% is a drift of a few hundred nanoseconds
  with no step in it. Both kept, and written down in the entry.
  `frame_10k_segments` is now the fifth guarded row in
  `scripts/bench-check.sh`, since it is the one row that sees a
  per-float cost.

- **A wrapped long line's edge glyphs are culled like everything
  else's** (found by the alpha.12 Windows round). `emit_entry_rows` — the
  wrapped long-line path C19 added — dropped a glyph horizontally when
  its ink merely touched the clip's edge (`x >= right`, `x + w <= left`)
  and vertically only when it crossed it (`y > bottom`, `y + h < top`),
  so a glyph whose ink ended exactly at the clip's top or began exactly
  at its bottom — no pixel inside — was still pushed. Which glyph that
  is depends on the face `Mono` resolves to and on where the scroll
  offset lands a row, which is why `long_line.rs`'s "every glyph drawn
  is inside the view" passed on every machine until C32 moved the face
  this one picks: at 12,000 px down, two of 417 glyphs sat on the
  boundary. Both vertical tests are inclusive now, as the horizontal
  pair always were. The unwrapped path (`emit_entry`) has the same
  asymmetry and is left alone for this release: every text takes it,
  and the corpus counts its quads.

- **`scripts/bench-check.sh` runs on Windows** (the same round). Two
  things a macOS shell never met: Git for Windows' `ps` has no `-o`, and
  under `set -eo pipefail` the CPU-load advisory's failed substitution
  ended the run before it built anything; and a CRLF checkout left every
  README row unmatched against the table regex's `$`, so the refreshed
  table said "(not in the README yet)" for all thirty-two. `|| true`
  on the advisory and `\r?` on the split. Repo tooling, not API.

### What you can delete

The `isDark ? light : dark` branch in front of every app colour that
had a light half, and the `system`-message plumbing that re-ran it; the
`hex → name` lookup a by-hand round kept in its head while reading the
inspector; and, for a Lua panel inside a host with a palette, the
`slot_with(params)` map that carried the host's colours to it
(ADR 0027).

**The hover and pressed arithmetic beside the palette** — the
`lift(c, t)` / `hoverOf` / `pressOf` a kit computed its lit shades with,
and the `chan` / `hex2` helpers under it: a shade is a derived token now
(`peachHover: { from: 'peach', ops: [['lift', 0.3]] }`), computed by the
core on the half in effect and named by the inspector; what stays in the
app is the map from a chosen colour to its shade (ADR 0028).

**The caret's `keyframes` loop, or the 500 ms thread on the `Waker`** —
whatever stood in for a blink on the caret you draw yourself, and the
frame-every-vsync it cost; the phase is one read now (C35).

**The in-process yank register and the arboard link beside it** — a
custom editor's `Vec<String>` that `y` and `p` went through because the
sink could not reach the system clipboard, and, for a Rust host, the
`arboard` it linked itself to reach around the runner; and **the
double-click timer and the stashed press point** — the `Instant` an
editor kept to turn two clicks into a word, and the `pending_click` it
stashed in `on_event` to resolve with `text_hit` in the next `view`, a
frame late: the count and the byte are on the event now (C33, C34).

**Whatever stood in for the OS setting you could not flip** — the
reduced-motion branch of a view is now reachable in a window from the
app's own code, on any machine, so nothing outside the view needs to
pretend the user asked for less motion, and a smoke test that skipped
that branch for lack of a way to open it can open it.

The `announce` an app made unconditionally because it could not tell
whether anyone would hear it, and the blink it ran beside it for the
case nobody would; the `KUI_SCREEN_READER=1` (or equivalent) environment
variable a test set to flip the app into announcing, now that
`setEnv({ system: { assistive: 'listening' } })` declares it.

The width an app declared by hand for a rename field because it could not
use `fit` — it wanted `wrap` — and the "headroom, sideways" margin past
the widest glyph it kept in front of the field so the echo frame's last
word had somewhere to go: the field is `width="fit" maxWidth={…}
wrap="word"` and the core sizes it to the wrapped draft on the keystroke
frame (F44).

- A `tick.every` chosen as the compromise between the cadence a running
  countdown needs and the one a stopped one can afford, and the
  `idlePumpMs` raised to compensate: `every` reads the model now.
- The `@ts-expect-error` on `clock` in a `runWindowed` call.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag on
2026-09-14 on `main` over the 102 commits since alpha.11. What follows is
what executed on what. One platform this time: the round ran on Windows
only, and the macOS half — `cargo test --workspace` against the real
SDK, the AX audit, the gestures on the by-hand list — was not run for
this tag.

**Windows 11 Pro 26200 (x64), rustc 1.98.1, Node 25.2.1, MSVC 14.52**,
Ryzen 9 9950X3D and an RTX 5080 at 3840×2160. `cargo fmt --all --check`
and `cargo clippy --workspace --all-targets -- -D warnings` are clean.
`cargo test --workspace`: **1177 tests over 93 suites, 0 failed** (1
ignored, the devtools' `drive.rs` doc example), from alpha.11's 1036
over 89. The scene corpus runs in all four adapters against one
reference report: **36 scenes** — `sampler`, `tokens`, `cells-scroll`,
`selection-extend` and `selection-scroll` new since alpha.11's 31 — Rust and Lua
through `cargo test`, C through `target/debug/conformance` (the header
at **366 fields, 245 enum members and 213 prototypes**, from 344 / 224 /
187, matched against the MSVC layout by the parity assert; the 213 are
the DLL's 211 exports and the two runner entry points), Node through
`npm test` with `KUI_CONFORMANCE_REQUIRED=1` (**158 Node tests, 157
passed, 0 failed, 1 skipped** — the `RTLD_GLOBAL` pin alpha.11 taught
to skip on a platform with no dlopen flags — from 124). The C round,
`cbuild --run`, passes its **six checks** against **ABI 16**: the C
counter's drive, the header walk with every prototype called, a C host
loading the C panel, the same plugin in a Rust host, the Windows plugin
importing from the Rust host, and the plugin with `kui_ext_abi` deleted
refused with the number. `npm run gen` leaves the three generated files
unchanged on Windows — W13's `titlebar_h` drift is gone, and the only
diff it makes is the line endings a CRLF checkout adds. `npm run
typecheck` on `examples/node` is clean. The headless round, `smoke --
--headless`, passes all **26 drives**, from 22: C36's three app-shaped
examples and C38's `clipboard` are new in it.

**The windowed round**, `cargo run -p kui-devtools --bin smoke -- --node`:
**32 Rust examples on both bases and the five Node windows, 120 frames
each, every one exiting 0 with nothing on stderr** — 69 windows, from
alpha.11's 66 (`clipboard` new on both sides). The C and Lua hosts by
hand under `KUI_SMOKE_FRAMES=120`: `counter.exe`, `host.exe`, `c_panel`
and `lua_panel` each opened a window and exited 0, warning-free — **73
windows over five hosts.**

**What the round found.** One test, one script, one ordering, one
bench row.

- `cargo test --workspace` failed on the first run, in one test:
  `long_line::scrolling_down_shapes_what_scrolls_in_and_draws_it_in_the_view`,
  "every glyph drawn is inside the view". Two of 417 glyphs sat exactly
  on the view's edge — ink ending at y = 0, ink starting at y = 100 —
  admitted by the wrapped long-line path's vertical cull, which was
  strict where its horizontal cull was inclusive. Which glyph lands
  there depends on the face `Mono` resolves to, and C32 moved that face
  on this machine between the tags; the test had passed on alpha.11's
  Windows round with the italic one. The cull is inclusive on both axes
  now (**Fixed** above); the second run is 1177 / 0.
- `scripts/bench-check.sh` exited before benching: Git for Windows'
  `ps` has no `-o`, and `set -eo pipefail` turned the CPU-load
  advisory's empty read into an exit. With that and the README table's
  `\r` fixed (**Fixed** above), it ran — for the first time on Windows;
  C29's sweep last release used `cargo bench` directly.
- The four hosts by hand failed to start on the first try, `counter.exe`
  and `host.exe` with `STATUS_ENTRYPOINT_NOT_FOUND`: `target/debug/
  kui_ffi.dll` had been rebuilt under the headless round as kui-lua's
  runner-less dependency (211 exports, no `kui_run`) over the one
  `cbuild` linked the hosts against. Rerunning `cbuild` — a fingerprint
  check — put it back and all four opened. Filed as **backlog W16**
  with the two fixes to choose between; the README's recipe says the
  order until then.
- **The bench guard, read honestly.** Against alpha.11 on this machine,
  the first run flagged one guarded row, `frame_10k_rects_with_text_and_hits`,
  at **+19.6%** (1.12 → 1.34 ms, ±2.8% run-to-run), and a second run of
  that row alone at +12.9% (1.27 → 1.43 ms). A bisect with the row as
  its oracle landed inside the noise (1.359 vs 1.367 ms), so the
  seventeen core commits from AR16 to HEAD were swept in tree order,
  two runs each in one worktree, the lower median kept: **every one
  reads between 1.33 and 1.39 ms, HEAD at 1.35 against the tag's 1.29
  in the same worktree (+4.6%), with no step at any commit.** A third
  guard run then read the row *unreadable* at ±12.0% — with the
  alpha.11 side itself at 1.41 ms, a tenth above what it read an hour
  earlier. The row is bimodal here by about 10%: two readings cluster
  at 1.27–1.34 and two at 1.41–1.43, on both sides of the tag, which is
  what a bench looks like when the scheduler moves it between a
  9950X3D's two CCDs (one carries the V-cache). The first run paired a
  low base with a high HEAD. The verdict is that **no regression is
  demonstrated** on this row and the machine cannot read it to 10%; on
  the M3 Pro, where the guard has run every release, the row reads to
  ±3% and the next macOS round is where it is settled. The other six
  guarded rows: `deep_nesting_64_levels` +1.7%, `frame_10k_rects`
  +1.7%, `frame_10k_rects_with_access_tree` +0.6% on the clean run
  (unreadable at ±16.6% on the first), `frame_10k_segments` **−4.3%**
  (C29's chrome skip, holding), `frame_1k_typical` +0.5% to +5.5%
  across runs, `list_10k_rows_virtual` +0.7%. Unguarded, nothing over
  ±6%. The README's table is left at its 2026-09-11 M3 Pro numbers
  rather than refreshed from a machine that reads one row two ways.

**Not run for this tag:** the macOS round — `scripts/ax-audit.swift`
(106/106 on alpha.11), the three gestures on the by-hand list captured
mid-press, `lua_panel`'s window looked at rather than counted, and the
AppKit half of ADR 0030's Edit menu (W14, W15) that was checked by hand
while it was built on 2026-09-14 and not again here. The by-hand list
in `docs/BACKLOG.md` ("After alpha.12") carries what a macOS session
should open first.


## 0.1.0-alpha.11 (2026-09-11)

**What breaks.**

- `KUI_ABI_VERSION` is **15**, over five bumps from alpha.10's 10, and the
  Node binary frame is **version 9**, over four from 5 (6 `originLine` on
  `cells`, 7 `measureText`, 8 `sampling`/`fit` on `image`, 9 the fragment
  image input; the encoder and the addon ship together, so nothing to do
  unless you own an encoder). Three of the five
  ABI bumps are appends the size handshake covers (13 `checked` on
  `KuiMenuItem`, 14 textures on `KuiDrawData`, 15 the image input on
  `KuiFragmentDraw`; each is a bullet below), and two are not: **ABI 11**
  moved every field of `KuiQuad` after `kind` — a host that walks the quad
  array itself must recompile, and reads `dd.clips[q.clip]` where it read
  `q.clip` and `q.clip_radius` — and **ABI 12** appended `origin_line` to
  `kui_cells`'s argument list, which is a compile error rather than a
  misread. Recompile every C host and plugin once against the new header;
  a plugin binary built against any of ABI 10–14 is refused by the version
  check rather than handed a frame it would misread.
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
- **Floats stack in the order they opened, not in tree order, and a
  scroller's bars are under the floats over it**
  ([ADR 0023](docs/adr/0023-layers-stack-in-the-order-they-open.md)). A
  float declared later in the tree is no longer guaranteed above one
  declared earlier — only one that *opened* later is; two floats declared
  every frame from the same first frame keep tree order. A scroller's
  bars and the focus ring paint at the end of the layer that owns them
  rather than at the end of the frame, so a menu over a page's bar covers
  it and takes the press there. The corpus report changes (`layers` is a
  new scene, and every scene with a bar or a ring beside a float moves),
  so `target/conformance.txt` wants regenerating.
- **A `line` or `polygon` that declares input takes it — by shape — and
  the two `*-ignores-input` warnings are gone**
  ([ADR 0026](docs/adr/0026-hit-testing-by-shape.md)). `onClick`,
  `onDrag`, `onHover` and `hoverable` on a stroke or a fill used to be
  ignored with a warning; now a press within the stroke's width (at
  least 4 px of grab) or inside the outline hits it, and one in the
  bounding box off the shape falls through. A match on
  `'line-ignores-input'` or `'polygon-ignores-input'` no longer
  compiles, and such a node is in the access tree (a clickable one is a
  button — name it), so an access-row snapshot grows. A rounded box's
  dead corners are no longer hits either: a click in the corner of a
  rounded card reaches what is under it. `HitRegion` gained a `shape`
  field, so code that constructs one by hand adds `HitShape::Rect`. In
  C, `kui_polyline` and `kui_polygon` take `on_click`, `on_drag` and
  `on_hover` as `kui_open_with` does (under the same unreleased ABI 14);
  `kui_line` is unchanged. The corpus `lines` and `polygon` scenes gain
  presses on and off the shape and their clickable nodes gain labels, so
  `target/conformance.txt` wants regenerating.
- **ABI 14: `KuiDrawData` appends `textures` and `texture_count`, and
  `KUI_QUAD_TEXTURE = 8` is a ninth quad kind**
  ([ADR 0025](docs/adr/0025-the-image-is-the-canvas.md)). An [out] append
  the size handshake covers — a host reserving the ABI-13 layout keeps
  working and never sees the side entry — so the bump is for the kind: a
  host's own renderer that predates it draws such a quad as a solid,
  wrongly and harmlessly, as a pre-segment host draws a segment. A host
  that renders the list itself reads `KuiDrawData.textures[q.uv[0]]`,
  fetches the bytes with `kui_image_pixels`, uploads when `rev` moved,
  binds that texture in the atlas's place and draws the quad as an
  image; on both image kinds `border_w` is now the `sampling` flag (0
  linear, 1 nearest), which was always zero before.
- **ABI 15: `KuiFragmentDraw` appends `image_source`, `image_texture`
  and `image_uv`** (backlog V1, [ADR 0025](docs/adr/0025-the-image-is-the-canvas.md)
  decision 7). An array element, so the append moves the stride — the
  `KuiSpan` / `KuiMenuItem` exception — and a host that walks
  `KuiDrawData.fragments` itself recompiles; one that does not has
  nothing to change. A host that renders the list binds, for a draw
  whose `image_source` is `KUI_FRAGMENT_IMAGE_TEXTURE`, the texture
  `textures[image_texture]` names in the atlas's place — exactly what it
  does for a `KUI_QUAD_TEXTURE` quad — and hands the shader `image_uv`.
- **`Content::Fragment` carries a `FragmentRef`, and `FragmentDraw` an
  `image`.** A binding that lowers `fragment` builds
  `FragmentRef { id, image }` where it passed a `FragmentId`; every
  `Core`/`Ui` fragment door takes `impl Into<FragmentRef>`, so a call
  passing a `FragmentId` compiles as before. Code that constructs a
  `FragmentDraw` by hand adds `image: FragmentImage::None`; the frame's
  `FragmentList` holds `fragment::Draw` (the declared form) rather than
  the wire one.
- **Node's binary protocol is v9, and `fragmentDraws()` returns
  twenty-four doubles a draw.** `<fragment>` carries its `image` as two
  slots after `src` (zero for none); a reader of `fragmentDraws()` that
  strided by 18 strides by 24 — the six new words are where the image is
  (0/1/2), the `textureDraws()` index, and the texel rect.
- **`widgets::button_spec` takes `&Metrics`, and `menu_panel_spec` takes
  it beside the theme** (backlog T2). `widgets::button_spec(&ui.metrics())`
  is the idiom, as `menu_panel_spec(&t, &m)` is; `button_spec(&Metrics::default())`
  is byte for byte what the no-argument form built. The public constants
  (`BUTTON_TEXT`, `MENU_WIDTH`, `MENU_TEXT`, `MENU_BAR_H`, `TITLEBAR_H`)
  stay, as the stock set's values — but the widgets read `ui.metrics()`
  now, so an app that sets its own metrics and still lays out against
  `TITLEBAR_H` is one constant behind its own titlebar; read
  `ui.metrics().titlebar_h`. The corpus report does not move: the stock
  set is the constants.
- **The corpus report grows a `fragment-image` line**, written for each
  fragment draw that names an image — `fragment-image <i> <atlas|texture>
  <texture index|-> <x> <y> <w> <h>` — and the `fragments` scene grows
  three such fragments (one over the atlas-backed icon, one over the
  texture-backed stream, one over an image live nowhere), so
  `target/conformance.txt` wants regenerating and every adapter registers
  a second fixture source, `FRAGMENT_IMAGE_WGSL`.
- **The corpus report's `kinds` line has nine columns, and `texture`
  lines join `fragment` lines.** Every scene's `kinds` moves by a column
  (`… <fragment> <texture>`), `media` gains three texture quads and a
  crop line, and `polygon` is a new scene; `target/conformance.txt` wants
  regenerating, and an adapter that counted eight kinds counts nine
  (`Expect` gained `textures`).
- **The `layout` event's payload carries `scale`.** A handler that
  deep-compares the whole payload sees one more key — `{kind, x, y, w, h,
  parent, scale, tag}` — which is physical px per logical px at the node,
  the number a view multiplies `w`/`h` by before `update_image`.
- **Node's binary protocol is v8.** `<image>` carries its `sampling` and
  `fit` as two slots before its props and `<polygon>` is a new op; the
  encoder and the addon ship together in the package, so nothing to do,
  and an encoder from an older package against this addon is refused
  by version rather than misread.
- **`ImageEntry.rgba` is an `Arc<Vec<u8>>`, and `NodeContent::Image`
  carries `ImageOpts` beside the id.** Rust code that read the bytes
  derefs one more level; a match on the variant takes two fields.
- **ABI 13: `KuiMenuItem` grew a `checked` field.** It is an [in] struct,
  whose appends are ordinarily free — but this one travels as an *array*,
  so the append moved the stride and a host that does not recompile reads
  every row after the first from the wrong bytes. Recompile; a zeroed tail
  is `checked = 0`, which is what every row had. The same exception
  `KuiSpan` was (ABI 8).
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
- **`TextStyle::color` is `Option<Color>`.** `None` is "the theme's
  foreground", which is what the schema row has said this prop means since
  it was written ("default foreground when omitted") and now what it does.
  `TextStyle::new(..).color(c)` is unchanged; code that *reads* the field
  wants `.color_or_default()`. Same for `EditOptions::accent`, whose
  `None` is `theme.selection`.
- **The stock widgets follow the OS appearance.** A host that reports a
  light `env.system.appearance` gets a light context menu, tooltip, field,
  titlebar, scrollbar and focus ring where it got dark ones. A host that
  reports nothing is unaffected, byte for byte. An app that wants the old
  behaviour on a light desktop pins it: `core.set_theme(Theme::dark())`.
- **The stock context menu's hovered row is a wash, not a fill.** It was
  the raw accent under a fixed light label, which is unreadable on a light
  base; it is now `theme.accent_soft` over `theme.raised` with `theme.fg`
  on top. Menu colours also consolidated onto the theme's roles, so the
  corpus report changes and `target/conformance.txt` wants regenerating.
- **The examples moved, and most were renamed** ([ADR 0021](docs/adr/0021-one-subject-per-example.md)).
  One subject per file, and the kind is the directory:
  `examples/rust/{apps,widgets,features,tools}`, the same under `c/`,
  `lua/` and `node/`. The old target names are gone — `gallery` is
  `image`, `connectors` is `line`, `fragments` is `fragment`, `rich_text`
  is `text`, `editor` is `edit`, `toasts` is `enter_exit`, `bulk_exit` is
  `exit_budget`; `context_menu` lost its terminal, its menu bar and its
  selection to `cells`, `menu_bar` and `selection`; the old `counter`'s
  sounds are `audio`, and every counter is now the same shape. In C,
  `counter.c` is three programs — `apps/counter.c`, `tools/surface.c` (the
  header walk) and `tools/conformance.c` — and the panel trio is under
  `features/slots/`; `./target/debug/counter --headless` is the counter's
  drive alone and `./target/debug/surface` the walk. In Node, `npm start`
  is `npm run counter`, `mindmap` is `slide`, the `.d.ts` fixture is
  `tools/types.tsx`, and every window closes under `KUI_SMOKE_FRAMES`
  only with an addon built `--features smoke` (the prebuilds never are).
- **The root is never a Tab stop.** A root that carried `on_key` was a
  stop, and Tab drew the ring around the whole window; the ring walks
  past index 0 now. A root sink still hears every unclaimed key (ADR 0011)
  and can be focused outright. Found by the examples' devtools, whose
  sink is the root.
- **`Env` grew `audio`**, and every binding's reading with it: a view
  that destructures `Env` exhaustively names one more field, and C hosts
  have one more setter to call (or not — zero is closed, which is the
  truth for a host with no device).
- **`autofocus` is an edge** ([ADR 0022](docs/adr/0022-focus-regions.md),
  decision 9). An editor takes focus on the frame its `autofocus`
  declaration starts — a new editor, one back after a gap, one whose flag
  just turned on — and only while nothing holds focus; it no longer takes
  it back on every frame nothing is focused. An app that relied on its
  field retaking focus after a click on the background now sees the blur
  stand, and calls `focus(key)` when it wants the field back.
- **A root key sink hears keys when nothing is focused** (ADR 0022,
  decision 8). A press with no focus used to go nowhere; it now reaches
  an `onKey` on the root, the way every unclaimed key already bubbled
  there from a focused control. An app with a root sink that counted on
  silence with nothing focused hears more than it did; none of the
  examples did, and the two devtools stop taking focus to get it.
- **`kui_devtools` is a harness, not a panel** ([ADR 0024](docs/adr/0024-the-devtools-are-the-cores.md)).
  `Harness::new(name, example)` takes two arguments; the dock, its
  state, `fmt_value` and `STREAM_CAP` moved to `kui_core::devtools`
  (`kui::devtools`), and `Dock` is `kui::DevtoolsDock` re-exported:
  `Left | Right | Bottom | Window | Off` — `Side` is `Right` now, and
  `--dock side` still means it. `examples/node/devtools.tsx` is the same
  wrapper for Node. `Core::nodes()` lists the panel's nodes too, under
  `key_of("kui-devtools")`; a reader that wants the app alone skips that
  subtree. `WindowCommand` gained `Redraw`; a `match` over it names one
  more arm.
- **`EditStore::declare` returns a `bool`** (the autofocus edge). Internal
  to the workspace — `pub(crate)` — and listed for anyone who copied it.
- **The Node binary frame is version 7**, and `measureText` crosses it.
  A `windows` entry's `activates` slot gained a third value for "unsaid",
  so the popup default is the core's (`WindowConfig::of_kind`) and not the
  encoder's; and `measureText(content, style, maxWidth)` is a method the
  JS package adds to `Ctx` and `KuiWindow` (like `frame` and `setView`)
  over the addon's `measureTextBinary` — the text is encoded like a
  frame's, so a measured label and a drawn one are flattened by the same
  code. A caller on the class itself calls the same name; only a caller
  of the raw addon's `measureText` with a JSON tree has to change. The
  encoder and the addon ship together and the version check says so if
  they do not.
- **`Enter` and `Keyframe` deref to `Slots`.** `enter.bg` still reads the
  slot and the builders are unchanged; code that *constructed* either by
  struct literal writes `Enter { dx, dy, slots: Slots { bg, .. } }` now.
  Nothing in the workspace did.
- **`kui::widgets::context_menu` and `menu_panel` return the root `Key`**
  rather than a `MenuNodes` with the row keys: the rows are known by
  `OriginId::MENU` / `OriginId::MENU_BAR` now, the way the devtools' nodes
  are by `OriginId::DEVTOOLS`, and an extension list can never reach any of
  the three. A Node menu row with `id: null` posts its label, as a row with
  no `id` does; it posted `null`.
- **`KuiSpec.tooltip` is floated by the core**, on `kui_close`, and
  `MouseButtonName` / `EditKeyName` in `index.d.ts` are generated from
  `EditKey::ALL` / `MouseButton::NAMED`. Neither changes a call.
- **The three smoke scripts are one program.** `scripts/smoke-examples.sh`,
  `scripts/smoke-windows.ps1` and `scripts/smoke-headless.sh` are gone;
  `cargo run -p kui-devtools --bin smoke` is the windowed round and
  `-- --headless` the headless one, with the flags the scripts took
  (`--frames`, `--only`, `--base`, `--release`, `--node`; `--headless
  --list` prints the round). Repo tooling, not API — listed because CI
  and the README named the scripts.
- **The C examples' two build scripts are one program too.**
  `examples/c/build.sh` and `examples/c/build.ps1` are gone; `cargo run -p
  kui-devtools --bin cbuild` builds kui_ffi, checks `kui.h` against the
  Rust layout, builds the four C programs and the plugin (both plugin
  shapes on Windows, the `kui_ext_abi`-less mutant everywhere) and prints
  the round; `-- --run` runs it, which CI does. `--release` as before. On
  Windows the MSVC compiler is found the way every `-sys` crate finds it
  (the `cc` crate's registry lookup), so `cl` need not be on PATH. The
  hosts' "build it first" hints name the new command.
- **`TextSystem`'s two maps are one.** Internal (`pub(crate)`), listed
  for anyone who copied the long-line cache: `Entry::{Run, Long}` in one
  map, and `FrameText` / `TextPlace` no longer carry a `long` flag.

### Added

- **Hit-testing by shape** ([ADR 0026](docs/adr/0026-hit-testing-by-shape.md)).
  Every hit region carries a `HitShape` — `Rect`, `Rounded(radii)`,
  `Segments` or `Polygon` — and the one `contains` every input path
  runs (hover, press, click, drag start, cursor shape, the secondary
  press) tests the rect first and the shape after, so a pie's wedges
  are their own hover targets, a connector takes a drag on its stroke,
  and a rounded card's corners are not hits. A stroke's grab is never
  under 4 px; a fill is tested even-odd, which for the simple outlines a
  `polygon` draws is the fill. Measured: two storage variants at
  `frame_10k_rects_with_text_and_hits` 1.199 vs 1.197 ms against 1.181
  before — the inline enum the draft priced at +7% cost nothing, and won
  on simplicity — and a full hover scan over 2,500 shaped regions at 3.6
  µs (`hover_over_10k_regions`). `tests/hit.rs` pins the geometry and
  every path; the polygon example deletes its hover boxes.
- **The image is the canvas**
  ([ADR 0025](docs/adr/0025-the-image-is-the-canvas.md)) — the answer to
  "should kui have a canvas, raw GPU commands or a painter callback" is
  the primitives that make an app not need one, and nine times out of
  ten *canvas* meant "I will draw it, you show it":
  - **`update_image(id, w, h, rgba)`** — `Core::update_image`,
    `surface.updateImage`, `kui_image_update` — replaces an image's
    pixels in place. The handle is unchanged, so every node showing it
    draws the new pixels next frame with no view change; the dimensions
    may change; a dead handle warns `foreign-resource`. A video frame, a
    camera, an emulator, a plot the app rasterised with whatever it
    likes: kui composes, clips, rounds, fades and hit-tests the box.
  - **Two backings, the core decides.** An image is atlas-backed as
    before until it is updated, or does not fit a 4096 page — then it
    draws from a texture of its own, as a `QuadKind::Texture` quad whose
    `uv[0]` indexes `DisplayList::textures` (the handle and the texel
    rect) beside `texture_pixels` (the bytes, shared by `Arc`, and a
    revision a backend uploads on). The wgpu renderer caches textures on
    the shared `Gpu` and splits the instanced draw around a texture quad
    the way it splits around a fragment, rebinding group 0 with that
    texture where the atlas was. Consecutive quads of one texture take
    one bind each, as consecutive fragments take one pipeline set each.
  - **`sampling` and `fit` on `image`** — `image_with(id, ImageOpts {..},
    spec)` in Rust, `<image sampling fit>` in JSX, `image { sampling=,
    fit= }` in Lua, `kui_image_with` in C. `sampling` is `linear` (the
    default) or `nearest` (pixel art, an emulator, a data grid); `fit` is
    `fill` (the default, and what every image did), `contain` (the
    largest rect of the image's aspect, centred; the painted rect
    shrinks) or `cover` (the box filled, the texels cropped, centred).
    Both resolved in the core, so the corpus pins them on the quad; the
    box — its layout, hit region and access rect — is the same in every
    mode.
  - **`scale` on the `layout` payload** — physical px per logical px at
    the node: `w × scale` by `h × scale` is how many pixels to render
    before `update_image`. The frame's today; where a zoom would compose
    in. The `image` example runs the loop: `onLayout` on the box,
    render at the reported size, replace, one frame late.
  - **`polygon`** — `ui.polygon(&points, spec)`, `<polygon points
    bg/>`, `polygon { points=, bg= }`, `kui_polygon`: a fill of up to
    eight points (a ninth warns `polygon-points-truncated`; fewer than
    three draw nothing; no `bg`, no fill), placed exactly as a `line` is
    — a float in the parent's box space sized to its own bounding box a
    pixel out on each side, no room taken, no input
    (`polygon-ignores-input`), no access row — with the fill in `bg` so
    `transition`, `enter` and `exit` reach it, and ghosts carry it. On
    the wire it is one `fragment` quad painted by `fragment::POLYGON`, a
    polygon-SDF source the core registers once per session through the
    same idempotent `add_fragment` an app's source takes, so a C host
    gets it from `kui_fragment_source` like any other; concave outlines
    fill correctly, self-intersecting ones even-odd (overlaps unfilled),
    the same rule the hit test uses. The prelude's
    `FragmentIn` gained `color` for it — the quad's colour, white on a
    `fragment` and the fill on a `polygon` — which any fragment may read
    rather than spend four params on one. `cargo run --example polygon`:
    a pie whose wedges light under a hover box, arrowheads on a graph,
    the area under a sparkline, a concave star.
  - **`kui_image_pixels`** for a C host that renders the list itself,
    `textureDraws()` in Node beside `fragmentDraws()`,
    `Core::image_pixels` in Rust; `KUI_SAMPLING_*` and `KUI_FIT_*`
    pinned by the ABI audit; the `media` corpus scene grown by an
    updated *stream* fixture, `nearest`, `contain` and a `cover` crop the
    report carries as a `texture` line; `polygon` a new scene pinning
    five fills' params to the bit in four bindings.
  - **Measured** (M3 Pro; the tables are in the ADR's status block):
    `frame_1k_typical` 115 → 122 µs with eight texture-backed images
    beside it and unchanged without; a thousand six-point polygons 98 µs
    against 94 µs for the same outlines as closed six-segment strokes —
    parity, after a per-node session lock the first build had was
    dropped; `update_image` at 1080p costs the app's own 8 MB copy
    (~110 µs here) and nothing the core adds measurably; the renderer's
    split around a texture run ~0.4 µs of CPU, and what the GPU pays at a
    hundred 320×180 boxes each sampling a 1080p texture is the minified
    sampling without mips (V6), not the split.
  - **What it declines, with the condition that reopens each**: a
    drawing-ops canvas (a view past ~10k primitives from Node, or a fill
    eight points cannot make), the painter (unchanged: a view a fragment
    cannot serve), a path primitive, rotation, a core `zoom` row (a
    second camera app, or UI zoom) — V7 and V8 in the backlog, and V1–V6
    for what it named and deferred (fragment image input next).
- **A fragment reads an image** (backlog V1, ADR 0025 decision 7, the
  deferral ADR 0015 decision 9 made). `<fragment src image params>`,
  `fragment { id=, image= }`, `ui.fragment(id.with_image(img), ..)`,
  `kui_fragment_with(ctx, label, id, image, ..)` and
  `kui_fragment_open_with`: the function reads a registered image
  through two prelude helpers — `kui_sample(uv)` bilinear,
  `kui_sample_nearest(uv)` texel-exact, `uv` in `[0,1]²` and clamped half
  a texel in from the edge so the atlas neighbour never bleeds — and
  `in.image` is the texel rect, `zw` the image's size in texels. **The
  core binds whichever holds the pixels**: an atlas-backed image goes
  into the atlas as an `image` node's would and the draw carries the
  slot; a texture-backed one — anything `update_image` has touched —
  takes the frame's `textures` entry an `image` node of it would take,
  and the renderer binds that texture at group 0 for the one quad, as
  it does for a texture quad. The function never knows which. A fragment
  whose image is not live draws nothing, as one whose `src` is not does
  — the removal order ADR 0015 said needed a test before it needed a
  feature, and `tests/images.rs` pins both orders. What it is for is the
  "many points" case every canvas request turned out to be: a
  spectrogram, a heatmap, a 50k-point line, an image effect — the data is
  a texture the app replaces, the nodes are one. The `fragment` example
  gains a heatmap (64×16 values rewritten every frame, drawn as cells
  through `kui_sample_nearest`) and a ripple over an atlas-backed icon,
  with a drive; `FragmentDraw.image` is a `FragmentImage` (`None`,
  `Atlas(uv)`, `Texture { index, uv }`), `KuiFragmentDraw` its three
  fields under ABI 15, Node's `fragmentDraws()` six more words, and the
  corpus a `fragment-image` line per such draw. The prelude's helpers are
  `kui_`-prefixed as `kui_sd_rounded_box` is, where the two ADRs wrote
  `sample(uv)` — the module's rule is that only `FragmentIn` and
  `fragment` go unprefixed. **Measured** (`scripts/bench-check.sh`,
  interleaved against the base commit, M3 Pro): the guarded rows moved
  +1.2–1.7% with a run-to-run spread of ±0.4–0.9% (`frame_1k_typical`
  115 → 117 µs, `frame_10k_rects` 721 → 733 µs), inside the tolerance
  and at the edge of the noise; `frame_1k_polygons` 96.1 → 101 µs
  (+4.9%, ±1.1%) is the one reading — a polygon is a fragment draw, and
  the draw grew by the image's slot on both sides of the wire. Five
  microseconds on a thousand fills; the hoist ADR 0025's amendment
  already names (the per-quad session lookup in `push_fragment`) is
  where the next round would take it back.
- **`Metrics`: the sizes the stock widgets are built from, beside the
  palette** (backlog T2 — the axis ADR 0019 scoped itself out of). Sixteen
  roles — `control_text`, `chrome_text`, `hint_text`, `radius`,
  `radius_inner`, the button's, the field's, the tooltip's and a menu
  row's padding, `menu_width`, `menu_bar_h`, `titlebar_h` — read as
  `ui.metrics()` in Rust, `env.metrics` in Lua, `ctx.metrics()` /
  `win.metrics()` in Node and `kui_metrics` in C, and set with
  `Core::set_metrics`, `ctx.setMetrics({..})` (overrides on the set in
  effect, or on `base: "compact"`, then `scale`) and `kui_metrics_set`.
  Every stock widget is built from it — the button, the field, the
  tooltip, the context menu, the menu bar and the titlebar — so an app's
  own control that reads `m.radius` and `m.control_pad_x` agrees with the
  stock button at every density without copying a number. The two
  questions the entry said to settle first are settled in
  `metrics.rs`'s doc: **a metric never scales by itself** — every field is
  logical px before `env.scale`, which is the renderer's, and density is
  the app's choice (`Metrics::compact()`, `Metrics::scaled(f)`) the way the
  palette is; and **the default is the contract** — `Metrics::default()`
  is byte for byte the constants the widgets had, the corpus runs with it
  and did not move by a byte, and a set metric changes what *that app*
  draws as `set_theme` does. `schema::METRIC_ROLES` pins the struct the
  way `THEME_ROLES` pins the theme (exhaustive destructure, both names
  both ways, C's `KuiMetrics` round-tripped through the table, Lua's
  `env.metrics` keys, the `## Metrics` table in `docs/props.md`).
  `cargo run --example metrics`: three densities by a click, the stock
  widgets rebuilt from each, and a card of the app's own that moves with
  them — with a drive.
- **Scroll anchoring: `anchor` on a scrolling node** (backlog C26 step 3,
  CSS's `overflow-anchor`) — `NodeSpec::anchor()`, `<box scrollY anchor>`,
  `anchor = true`, `KuiSpec.anchor`. The first child in view keeps its
  place on screen when the content before it changes size: a chat that
  prepends history, a log that inserts rows above the viewport, a list
  whose row heights are corrected as they are measured — with no
  `set_scroll` and no arithmetic in the view. The core remembers, per
  anchored container, which child was first in view and where its
  leading edge sat in the content (`ScrollStore`), and the next layout
  moves the retained offset by however far that edge moved, before the
  clamp; a wheel notch or a `set_scroll` between the frames is kept and
  the correction added to it. Children are found by key, so rows want
  stable keys; a child that is gone anchors nothing that frame; content
  after the anchor moves nothing, so a tailing log still asks for the end
  itself; and it is the scroll axis that is the node's main axis only. The
  `anchor` corpus scene pins one frame of it in four bindings — two
  scrollers wheeled alike, a taller row prepended to both, one still and
  one slid — and `tests/anchor.rs` the rest. This is the part of C5(b)
  that was worth building: the core keeping a row still, not the core
  skipping rows.
- **`layout_of(key)`: the `layout` event's numbers as a query** (backlog
  C26 step 2) — `Core::layout_of`, `ui.layout_of`, `ctx.layoutOf(key)`,
  `env.layout_of(key)`, `kui_layout_of(ctx, key, KuiLayoutRect *)`. The
  rect the last frame laid an `on_layout` node out at, read during the
  next build with no event, no tag and no model field; `None` for a key
  that did not declare `on_layout` last frame. Read during a build it
  describes the previous frame, like `scroll_geometry`.
- **The devtools tree walks by keyboard** (backlog D1a). The tree tab's
  list is one Tab stop and one key sink with a cursor of its own — the
  composite shape ADR 0007 describes, built in the panel rather than as
  a fifth container/item pair, since a tree's Left and Right fold and
  unfold where a list's step: Up/Down move the cursor by a row, Home/End
  to the ends, PageUp/Down by a screenful, Left folds the row (or goes to
  its parent when it is a leaf or already folded), Right unfolds it (or
  goes to its first child), Enter and Space select it as a click would,
  and the cursor's row stays in view with the list moving as little as it
  must. The rows are clickable and not Tab stops, as before; the ring is
  drawn on the list while it holds focus and on the cursor's row.
- **A `modal` example** (backlog E3): `cargo run --example modal` — a
  dialog over a form opening on its `initial_focus` button, Tab confined
  to it, a confirm nested inside it that makes the dialog as inert as the
  form, Escape routed by the modal's tag and answered by the app (close
  an untouched dialog, ask the confirm on a dirty one), and focus
  restored to the opener when the dialog goes — with a drive that pins
  all of it.
- **The devtools panel's buttons are drawn, not typed** (backlog D2a). The
  placement buttons, the base / accent / menus toggles and the tree tab's
  picker were Unicode blocks from whatever font the platform fell back to;
  they are now `runtime/devtools/icons.rs` — a `line` per stroke, a
  `polygon` per fill, a zero-length segment for a dot, in a 16-px box in
  the theme's colours, the lit placement's pane filled in the accent. The
  same on every platform, and they follow the appearance like the rest of
  the panel.
- **The devtools are the core's: one panel, drawn by the runtime, for
  every app** ([ADR 0024](docs/adr/0024-the-devtools-are-the-cores.md)).
  `Core::set_devtools(true)` — `kui::app(..).devtools(true)` in Rust,
  `win.setDevtools(true)` in Node, `kui_set_devtools` in C, or
  `KUI_DEVTOOLS=1` in the environment of a program that was never told —
  and the core draws the panel that was the examples' dock beside the
  app's own tree, in the main window (`left`, `right`, `bottom` — the
  pane's inner edge is a handle that resizes it), in a window of its own
  (`window`) or hidden with the chords live (`off`); a button per
  placement in the header, `set_devtools_dock`, `Ctrl+Shift+D`. **The
  app's viewport is what the dock leaves**: `viewport()` says so, a dock
  coming, moving or being dragged arrives as a `resize`, the app's
  viewport floats centre and clamp in it, and under a left dock every
  coordinate the app is handed or hands in is relative to its own
  origin. The app's `configure_root` is
  split between the root and an app container the core opens, and the
  app's keys do not move (`Key::ROOT.str("x")` names what it named). The
  panel's own clicks and its `Ctrl+Shift+<letter>` chords are acted on
  inside `handle_input` and never reach the host; every event that does
  is logged on its way out, from every window. What the round asked for
  and the dock never had: the **events** tab is a virtual list whose rows
  open into the payload as an indented tree, with a filter, a pause and
  a follow toggle; the **tree** tab is collapsible (a disclosure per row,
  `+N` on a folded one, fold/unfold all), filterable (a match with its
  ancestors dimmed), and has a **picker** (`Ctrl+Shift+P`, the crosshair)
  that outlines and names the node under the pointer over the app and
  selects it on a press; the **inspector** groups the node, its box,
  layout, paint, events and state, with the parent and every ancestor a
  click away. `NodeInfo` grew what the inspector reads — `layer`,
  `origin`, `children`, `padding`, `gap`, the alignments, `wrap`, the
  size floors and ceilings, `radius`, `border_w`, `border_color`,
  `opacity`, `scroll`, `events` (each handler with its payload) — and
  Node's `nodes()` rows the same. `WindowCommand::Redraw(id)`
  (`KUI_CMD_REDRAW`) is how one window asks another to draw. Behind a
  `devtools` feature of `kui-core`, on by default; off, the doors stay
  and do nothing. `Core::cursor()`, `Launcher::setup_core`,
  `Launcher::devtools`, `Core::set_devtools_theme` /
  `set_devtools_legend` came with it.
- **Scrollbars are per node: `scrollbar`, `scrollbarWidth`,
  `scrollbarColor`, `scrollbarActiveColor`** (rows 93–96; Lua
  `scrollbar`, `scrollbar_width`, …; C `KuiSpec.scrollbar` with
  `KUI_SCROLLBAR_VISIBLE` / `HIDDEN` / `AUTO`, `scrollbar_width`,
  `scrollbar_color`, `scrollbar_active_color`, appended after ABI 13 the
  compatible way). `hidden` draws no thumb and takes no press on the
  track while the wheel, the keyboard, `reveal` and the caret still
  scroll; `auto` shows the bar while the scroll state is changing — the
  offset or the extent moved, the pointer is on the track, a thumb is
  dragged — and for a second after, then fades it out over a quarter of
  one, asking for frames from the last change until it has faded, as a
  transition of that length would, and none while the pointer holds it
  (a node first seen shows it the same second; without a driver clock it
  is `visible`). The other
  three restyle the thumb: its width at rest (the active one is 2 px
  wider, and the track grows to fit), and its two colours over the
  theme's `scrollbar` / `scrollbar_active`. `ScrollbarMode` and
  `NodeSpec::scrollbar{,_width,_color,_active_color}` in Rust; the
  `scrollbar` corpus scene pins all three modes in four bindings.
- **Focus regions** ([ADR 0022](docs/adr/0022-focus-regions.md);
  `focusRegion` row, id 92; `Ui::focus_region` / `Ui::region`,
  `ctx.focusRegion(name | null)` / `ctx.region()`,
  `env.focus_region(key | nil)` and the `env.region` reading,
  `kui_focus_region` / `kui_region`, `KuiSpec.focus_region` appended the
  compatible way). A box declaring `focusRegion` is a Tab ring of its own:
  the ring outside it never enters it, and inside it Tab wraps over its
  controls alone. It is entered on purpose — `focusRegion('devtools')`
  from the chord that toggles a dock, a click on one of its controls or
  its dead space, an explicit `focus` on a node inside — landing on what
  the region last held, else its `initialFocus`, else its first stop, and
  showing the ring; `focusRegion(null)` comes back to what the main ring
  last held. The call is resolved by the frame it leads to, so the
  `update` that turns a dock on and enters it is one call, and a label the
  last frame did not have is fine (`focus-region-without-node` when the
  frame that follows has no such region either). A region that stops
  being declared hands focus back to main. Only the ring is scoped: keys
  bubble through the boundary to the sink above, the pointer and
  assistive technology see a plain node, and a `modal` is the ring
  wherever it sits. The devtools dock is the first one: `Ctrl+Shift+I`
  moves the keyboard into it and back out, Tab inside walks its icons,
  tabs and tree rows, and a screen reader reaches it as the group
  "Devtools" — it was `role="none"` before, invisible to both.

- **Every example runs inside the devtools, and the devtools have a dock**
  ([ADR 0021](docs/adr/0021-one-subject-per-example.md), decision 6 and
  *What the building changed*, 11; `examples/devtools`,
  `examples/node/devtools.tsx`). Beside the example's tree, a header —
  the frame counter and an icon strip for the theme base, the accent,
  native menus and where the dock sits — and three tabs: **facts** (the
  latency graph; the status block — `env.system`, the theme and its
  source, the window and every open one, the viewport and refresh rate,
  the focused node by its label and whether its ring shows, the
  modifiers, native menus, `env.audio`; the example's key legend),
  **events** (every `UiEvent` the example was handed, as the data it is,
  with the frame it arrived on, and every warning the core raised) and
  **tree** (the last frame's nodes with label, role and flags; click one
  and it is outlined on the example and an inspector opens: key, kind,
  label, role, rect, sizing, direction, background, flags, text). One
  CLI everywhere: `--headless`, `--dock side|bottom|off`, `--light` /
  `--dark`, `--accent`, `--size`. Chords `Ctrl+Shift+T/A/M/D/N/C` cycle
  the base, the accent, native menus (popups and bar), the dock, the
  tab, and clear the stream. The dock is `role = none`, so neither the
  Tab ring nor a screen reader sees it; the accessibility fixture runs
  `--dock off` under the audit and still reads 106/106.
- **`Core::set_inspect` / `Core::nodes`** — the frame as a list a tool
  can read back: every node the last finished frame laid out as a
  `NodeInfo` (key, parent, depth, kind, label, rect, sizing, direction,
  background, float, the *derived* role — the access tree's reading —
  and the declarations that make it interactive), taken at the end of
  `finish_frame` while a tool has asked and never otherwise. In Node,
  `win.setInspect()` / `win.nodes()`, beside `win.warningsRaised()`.
- **`--headless` is a contract**: an example that has one drives itself
  through a bare `Core` (`kui_devtools::Drive`) and exits non-zero on a
  wrong answer — twenty of them now, up from two. Which ones is read from
  `[package.metadata.kui] headless = [...]` in each crate's manifest;
  `scripts/smoke-headless.sh` prints the round and `--run` runs it, which
  CI does, and a test pins every listed name to an example. `lua_panel`
  is in a round for the first time.
- **The windowed round runs on every host and on both bases**:
  `scripts/smoke-examples.sh` is `smoke-windows.ps1`'s unix twin (same
  contract, 120 frames under `KUI_SMOKE_FRAMES`), both open every example
  under `--light` and `--dark` — T3's "every example on the palette" as a
  check rather than a migration — and `--node` adds the four Node windows.
  A test pins every `[[example]]` in the workspace to a row of
  `examples/README.md`.
- **Seven subjects that had no example**: `transition` (with the
  `keyframes` / `repeat` / `delay` chase that only the C header walk
  exercised), `hover` (with `hover_group`), `focus` (the Tab ring, who is
  in it, the verbs), `drag`, `titlebar` (with `window_buttons`, which no
  Rust example called), `tooltip`, `button` — each with a drive.
- **`env.audio`** (`docs/adr/0021`, decision 6a): what the driver's output
  device is doing — `device` closed / opening / open / failed, and `live`
  playbacks — as a row of `ENV_FIELDS` in every binding (`env.audio` in
  Lua and Node, `kui_env_set_audio` in C, additive). The reading no app
  had for the one fact that is an idle app's whole CPU once a session
  has held a sound.
- **`Core::label_of`**, the inverse of `key_of`: a key from an event or
  from `focus()` back to the label it was opened under.
- **`Core::warnings_raised`**: every warning a core has raised, drained
  or not — the log `take_warnings` leaves, for a reader that is not the
  driver.
- **`Example::extensions` / `native_menus`** on the devtools trait, and
  a `smoke` feature on `kui-node` forwarding to `kui/smoke`.
- **Every readback shape has a `to_value`** (backlog AR1): `ScrollGeometry`,
  `TextHit`, `Rect`, `Vec2`, `TextMetrics`, `NodeInfo`, `AccessTree` (and
  `AccessNode`, `AccessRun`, `TextPos`, `ScrollState`), `WindowCommand`
  (and `WindowConfig`), `Warning` and `AudioCommand` answer as a `Value`
  keyed in snake_case, the way an event already crosses. The shapes that
  hold a handle take a `Handles` — `Handles::HEX` spells a key or a
  resource id as sixteen hex digits (Node's way), `Handles::INT` as the
  integer (Lua's, and `ENV_FIELDS`'s). Node's and Lua's readers are one
  pass over these now; what a Node or Lua caller receives is byte for
  byte what it received (the key set is pinned per shape in kui-node,
  and tests/readback.rs pins the shapes in the core). A slider's numbers
  read as `value_now`/`value_min`/`value_max`, the rows that set them.
  With it, `Sizing::describe` (`fit`, `grow(1)`, `120px`, `50%` — the one
  spelling, shared with the devtools' inspector), `Dir::name`,
  `Align::name`, `WindowCommand::kind_name`, `AudioCommand::kind_name`.
- **`Core::select_word_under`**: the word under a point in a `selectable`
  scope, whichever geometry the scope has — what a double click and a
  force click take (backlog AR3); `Grain::of_clicks` beside it.
- **`cargo run -p kui-devtools --bin smoke`**, the smoke round as one
  program on every platform (backlog AR4), and `kui_devtools::manifest`,
  the rosters it and the pin tests read; **`--bin cbuild`** beside it, the
  C examples' build and round the same way.

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

- **An image larger than the atlas page drew nothing, silently.** A
  4097-px image was recorded as "does not fit" at emission and no quad
  was pushed, with no warning; it draws from a texture of its own now
  (ADR 0025, decision 2), and so does anything the page cannot take.
- **A secondary press on a node that offered no menu was swallowed, and
  the app looked like it had none.** `on_context_menu` was read off the
  topmost hit region alone, so a full-window `onKey` sink — the shell
  pattern `apps/splitmux` uses — over a root that declared the menu ate
  every right-click, silently (backlog T1, found building the theme
  example). Now the press is unclaimed at a node that offers nothing and
  reaches the nearest enclosing node that does, the rule ADR 0011 settled
  for keys: the event carries the *owner's* key and tag with the press
  point, a nested declaration still wins over its ancestor's, a disabled
  node's own is skipped (a disabled row in a list gets the list's menu —
  before, it got nothing), and the walk stops at the modal boundary. The
  owner is resolved at emission, where the tree is — `HitRegion::context_menu`
  is a `MenuOwner` now, not a bare tag — behind a `Tree::any_context_menu`
  flag so a frame that offers no menu pays nothing. The corpus `controls`
  scene gains the press on the button under the panel, so its report
  moves (one more `contextmenu menu`) and `target/conformance.txt` wants
  regenerating. `onForceClick` is unchanged and its doc now says so:
  topmost node only, no walk.

  **What you can delete:** the `onContextMenu` an app moved from its
  container onto every interactive child, or onto its key sink, to be
  heard at all.
- **A `cells` grid's `hoverBg` never lit, and its `transition` snapped.**
  `cells_at` pushed its spec untouched where every other leaf door
  resolves the hover style and eases first (backlog AR5). The tween is
  the box's — bg, opacity, size — and the cells inside stay the picture
  the app redraws. Pinned in tests/cells.rs.
- **The devtools' inspector rounded a node's sizing to whole px** —
  `12.5px` read as `12px`, `33.3%` as `33%` — where `nodes()` did not.
  Both spell it through `Sizing::describe` now.
- **A translucent `image` that was the frame's only fade painted opaque,
  and a floating `image` inside a clipped box vanished when it was the
  frame's only float.** The pass-skipping flags (C15) were set by hand at
  six builder doors and only the box door set all nine — a leaf that was
  the frame's only user of a feature turned the pass off. `Tree::note`
  sets them for every push and every root now; pinned in tests/images.rs.
- **The ghost of a departing `line` filled its box with the stroke
  colour.** The exit painter was a second copy of the live one, and only
  the live one knew a stroke's `bg` is its colour (ADR 0010); there is one
  painter now. Pinned in tests/line.rs.
- **The text cache's byte budget could not reach a long line's record**
  (`evict_to_budget` walked one of the two maps). Small — the chunks it
  shaped were always reachable — and the two maps now evict together.
- **A keyed `line`'s label resolves through `key_of` / `keyOf`** in every
  binding, as a box's and a `cells` grid's do; `line_node_keyed` never
  recorded it.
- **The drawn context menu came up under the page's scrollbar**, and so
  did every float that reached a scroller's edge; a press on a non-modal
  float over the track jumped the scroller instead of clicking the float;
  a menu bar's dropdown painted under a tooltip from the body; the focus
  ring painted over every float. One cause: `emit_frame` painted in-flow,
  then floats in tree order, then *every* scrollbar, then the ring, and
  the press asked the bars before the hit list. It paints a stack of
  layers now — the in-flow tree, then one per float in the order they
  opened, each with its own chrome at its end — and one
  `Interaction::target_at` answers the press and the cursor shape from
  that order ([ADR 0023](docs/adr/0023-layers-stack-in-the-order-they-open.md);
  `tests/layers.rs`, the `layers` corpus scene). A float from outside a
  modal's scope that opens over it raises `modal-behind-content`, as
  in-flow content after a modal already did. There is no `zIndex`;
  re-keying a float reopens it on top.
- **The macOS runner presented the platform's context menu over the
  core's drawn one** when a core had said `set_native_menus(false)` —
  both came up at once. `pump_native_menu` asks the core first. And a
  core told `set_native_menu_bar(false)` had the platform's bar left
  standing beside the strip it drew; the runner now hands AppKit an
  empty bar for such a core, so "drawn" on macOS shows one bar.
- **A Node `dispatch` the loop did not make itself drew nothing.** An
  effect handler's `dispatch('loaded')` — or the app's own from a timer
  or a promise — changed the model and waited for the next OS event to
  be seen. `step()` draws on it now; ADR 0013 said the result "lands in
  the next turn", and now the next turn is a frame.

- **The C ABI is pinned prototype by prototype, and constant by constant**
  ([ADR 0020](docs/adr/0020-the-surface-the-schema-does-not-cover.md)).
  `mod abi_parity` used to settle struct layout and three enum lists
  against `kui.h`; it now redeclares every one of the 173 entry points
  under its Rust signature in the generated translation unit, so a header
  prototype that drifts — a `const` dropped, an argument Rust gained
  (ABI 12's case) — fails the C build as `conflicting types` instead of
  reading a register nobody filled; and every plain constant the entry
  points read (`KUI_AUDIO_*`, `KUI_KMOD_*`, `KUI_ACCESS_*`, `KUI_KEY_*`,
  `KUI_MENU_*`, the span, cell and keyframe bits — 207 members, up from
  89) has a Rust name and a `KUI_ENUM` assert. The header's "Who writes
  what" audit is a test too, and was brought up to date: four `[out]`
  structs had the size handshake without being listed, and `KuiTheme` had
  it without the parity row. `./examples/c/build.sh` reports prototypes
  beside fields and enum members.

- **A C host can read every shape a payload takes.** `kui_value_as_bool`
  (a key event's `shift` / `ctrl` / `alt` / `super` / `repeat` were
  unreadable), `kui_value_as_float` (a drag's `dx`, a layout's rect and a
  resize's `scale` were truncated through `as_int`), `kui_value_len` /
  `kui_value_at` (a preedit's `cursor`, an `access` request's ends),
  `kui_value_entry` for a map whose keys you do not know,
  `kui_value_is_null`, and `kui_value_list` / `kui_value_list_push` for
  posting one.

- **The open menu reads back from C.** `kui_set_native_menus` said "read
  what is open" and offered nothing to read it with; `kui_menu_item_count`
  and `kui_menu_item` are that, spelled exactly as the menu bar's readers
  are and implemented by the same function, so a row cannot read two ways.

- **`kui_font_families`** lists what `kui_font_add_system` can take — what
  Node's `systemFontFamilies()` answers.

- **`Ctx.menu()` rows carry `checked` and the role's accelerator.** A
  `copy` row that declared no shortcut reported `accel: null` from the
  context menu and `⌘C` from the bar; both now read through one emitter,
  and `checked` is on every row (`openMenu` always accepted it).

- **`AccessRole`, `AccessAction` and `MenuItemRole` are generated.** The
  hand-written `AccessRole` union had missed `terminal` since backlog C20
  appended it; the three now come off `Role::ALL`, `AccessAction::ALL` and
  the new `MenuRole::ALL` through `npm run gen`, and `AccessNode.live` —
  emitted on every node since ADR 0008 — is declared.

- **A window losing the keyboard lets go of held keys in the core.**
  `Core::set_focused(false)` releases what the focused sink holds; the
  runner, `kui_env_set` and `setEnv({focused: false})` all go through it,
  so a headless test sees the `up`s a Cmd-Tab would have produced and a C
  host owes no `kui_release_held_keys` of its own (the call stays for a
  host with another reason).

- **An application menu bar the app declares**
  ([ADR 0018](docs/adr/0018-a-menu-bar-the-app-declares.md)). One call in
  the view says what the app's menu is and where its titles go when they
  have to be drawn: `<menuBar menu={[…]}/>`, `menu_bar { menu = {…} }`,
  `kui_menu_bar(ctx, menus, count)`, `widgets::menu_bar(ui, bar)`. On macOS
  it draws nothing and the same declaration becomes `NSApp.mainMenu`, with
  the OS drawing it, tracking it and binding the ⌘-shortcuts the rows
  declare; everywhere else it draws those menus as a strip in the window.
  One view is portable and no app writes its menu twice.

  Its rows are the rows a context menu has, deliberately: an Edit menu's
  `{ role: "copy" }` is the right-click Copy, performed by the core with
  the clipboard half handed to the host, and every choice arrives as the
  same `{kind:"menu", role, item}` event on both paths. The declaration is
  sticky and diffed the way the window title is, so re-declaring it every
  frame costs one comparison, an empty bar takes it away, and a window that
  declares none leaves the last one standing. `MenuItem` gains `checked`
  for the rows that are settings rather than commands (drawn as a
  checkmark, `NSMenuItem.state` where the platform draws, and reported to
  assistive technology), and `Accel` parses `"mod+shift+s"` — the portable
  spelling — into what the platform's own bar binds and what both bars
  draw (`⇧⌘S`, `Ctrl+Shift+S`).

- **`kui_core::Theme` — the colours a view paints with, as roles, derived
  from what the OS said** ([ADR
  0019](docs/adr/0019-a-theme-derived-from-appearance-and-accent.md)).
  `env.system.appearance` had been reported to every binding since the day
  it was plumbed and read by *nothing*: not the core, which is deliberate,
  and not the stock widgets or a single example, which was not. A view
  that wanted to honour it had no name for "the colour a card is", so it
  wrote the hex out again — 199 colour literals across the crates and the
  examples, 87 distinct values, and `#8a8fa3` by hand in sixteen files for
  one role nobody had a word for.

  Twenty-three roles in five groups — four surfaces, two borders, three
  text tiers, the accent family, and the state and status colours — chosen
  because four files in `examples/` had already converged on the same set
  under the same names. `ui.theme()` in Rust, `env.theme` in Lua,
  `ctx.theme()` in Node, `kui_theme` in C, all generated from one table
  (`schema::THEME_ROLES`) so a role added is a row added and nothing else.
  Every value is in the Theme table of [`docs/props.md`](docs/props.md),
  for both bases.

  Three sources, and the default follows the OS for both facts: `Derived`,
  `DerivedWithAccent(c)` — the OS's light/dark with the app's brand colour,
  which is what most apps with a colour of their own actually want — and
  `Pinned(theme)`, following nothing. `Core::set_accent`, `ctx.setAccent`
  and `kui_theme_set_accent` are the middle one; `set_theme` /
  `setTheme` / `kui_theme_set` the last. A Lua script reads and does not
  set: it is a guest in someone else's frame.

  An unknown appearance takes the **dark** base, which is what makes this
  safe to have on by default rather than behind a flag — every value in
  `Theme::dark()` is one this crate already painted, down to ADR 0002's
  focus ring, and a test asserts it colour by colour. The light base is
  new, mirrored rather than inverted (a float above a white page is not
  lighter than one; it separates by its border), and checked against WCAG
  AA by a test rather than by eye. That test failed on its first run
  against the *existing* palette: the `faint` grey four examples shared is
  2.43:1 on the page background, under the 3:1 floor for text a person is
  meant to read, and moved.

- **The window's ground follows the theme.** `kui-wgpu` cleared the
  surface to a constant near-black and nothing ever set it, so a view that
  paints no root background — most of them — showed that constant whatever
  the palette said. It is the one surface a `bg` prop cannot reach, since
  it is behind the tree. The runner writes `theme.bg` into the clear colour
  each frame; Node and C inherit it, both driving `kui::App` through the
  same `PumpRunner`.

- **Every example in the repo is on the theme** — sixteen Rust ones, the
  two Rust hosts that load a Lua and a C panel, the Lua panel itself, four
  JSX ones, and C on both sides of the FFI. The three `struct Pal`s survive
  as renames with a `From<Theme>` rebuilt each frame, because `pal.bg2`
  reads better than `theme.sunken` at twenty call sites. `fragments.rs`
  went too, and is where the idea reads best: fragment *parameters* are
  where a theme meets a shader — the WGSL says what a gradient, a ring or a
  shimmer is, and the palette says which colours it is made of, so all four
  follow the OS without a line of any shader changing. Three remainders
  keep colours of their own on purpose: `syntax_view`'s six syntax hues,
  authored the way ADR 0017 says a span's colour is (now in two sets,
  darkened for the light base, each checked past 4.5:1); the C self-test,
  whose checks assert exact colours; and `examples/lua/bench.rs`, whose two
  halves have to declare the same tree for the comparison to mean
  anything.

- **`examples/rust/theme.rs`** — the token reference page. Every role as a
  swatch beside its resolved hex, over every stock widget that reads it,
  with `1`/`2`/`3` switching between following the OS and pinning a base,
  `a` cycling an accent of the app's own, and the drawn context menu (it
  opts out of the platform's) to check a hovered row on both bases. It is
  where the next role gets looked at before it is added.

- **`examples/rust/context_menu.rs`**, which is the whole of ADR 0017's
  menu decision in one window: the stock menu over selectable text, an
  app's own menu over a row that declares `onContextMenu` (its two items
  beside the standard ones, opened with `Ui::open_menu`), a `cells` grid
  that selects in cells rather than in bytes, an editor's four, and nothing
  at all over a plain box. On macOS every one of them is the platform's
  `NSMenu`; elsewhere the core draws the same list. The app's code is
  identical either way.

- **`Ui::cell_selection`**, the grid half of `Ui::selection`: the window's
  selection when it lives in a `cells` grid, its ends as absolute lines and
  columns. `Ui::selection_text` already covered both kinds, but an app that
  wanted to *say* which lines of the session were selected had to reach for
  the `Core`.

- **Parity across the bindings for the selection and menu work.** C gains
  `kui_selection_text`, `kui_selection_html`, `kui_select_all_in` and
  `kui_clear_selection` — it could open a menu but not read or move the
  selection the menu acts on. Node gains the host seam it was missing:
  `takeMenuActions()`, `menu()`, `setNativeMenus`, `setLookupAvailable`
  and `activateMenuItem`, so a headless app or one driving its own window
  can render menus itself and drain what choosing a row left to do —
  before this, a Copy chosen in a headless Node app queued a clipboard
  write nothing could collect.
- **A checked menu row reads as checked.** `widgets::menu_panel` declared
  it and the access tree kept `checked` for checkbox / radio / switch
  alone, so a reader heard "Wrap" where the gutter drew "✓ Wrap". A
  `menuItem` that declares `checked` now reports it (and only then, the
  way a list row reports `selected` only when it is); the corpus's
  `menu_bar` report moves by one column on that row.

- **The macOS native context menu shows a row's own accelerator and its
  check state.** It kept a four-entry role table of its own and ignored
  both, while the native menu bar beside it parsed `accel_text()` and set
  the state; the two now build a row through one function.

- **Lua takes a menu role by its wire name.** `open_menu` took
  `"select_all"` and `"look_up"` and the `menu` event reported
  `"selectAll"` and `"lookUp"` back, the spelling every other enum value
  in Lua already used. Both spellings are accepted now, the snake ones as
  aliases the way `direction` is one for `repeat`.

- **A double click in a `cells` grid selects a word, a triple click the
  row.** A grid took one cell at every click count (ADR 0017 said a word
  in a terminal was the app's idea, which was wrong in practice: a
  terminal that does not select a word on a double click reads as broken).
  It now takes the same three grains a paragraph does, counted in cells,
  and both ends round outwards while the second click is held and dragged
  — so a word-drag turned back on itself keeps the word it started in
  whole. A force click on a grid takes the same word. The word is the run
  of like cells the pointer is in, classed the way a double click classes
  text; an app that wants a different rule still owns the gesture through
  `cell: {row, col}`.

- **Cmd-C and Cmd-A work over a `cells` grid.** The runner gated both on a
  focused editor or a *text* selection, so copying a selected terminal did
  nothing at all — unless some editor elsewhere in the window happened to
  hold focus, which is why it looked like it worked. Select All with a
  grid selection now takes the grid rather than falling through to the
  editor's.

- **A view can read a grid selection while it builds the next frame.** A
  host calling `selectionText()` from inside its own `view` — which is
  where Lua and Node hosts read it — got `null` over a `cells` grid: the
  grid was looked up in a tree that was still half-built. The cell store
  now keeps the frame before it, the way the text places already did, so
  the answer is last frame's grid rather than nothing.

- **A wide glyph copies as itself.** The `WIDE` flag is on the glyph and
  the blank the app leaves is the cell *after* it; the copy read the flag
  off the cell it was looking at, so it replaced every wide character with
  a space and left the spacer in. A line of CJK came back as blanks.

- **Copy over a `cells` grid copies the cells.** The stock menu lit its
  Copy row from the cell selection and then acted on the *text* one, which
  a grid does not have: choosing Copy over a terminal with half its screen
  selected posted the event, closed the menu and left the clipboard exactly
  as it was. Cmd-C was never affected — it reads the same selection the row
  was lit from, which is now what the row does too.

- **A `cells` grid honours its own padding.** Layout reserved it and the
  paint ignored it, so a padded grid drew from the box's corner with the
  padding hanging off the bottom-right, and a hit test — a `cell` payload
  on a click, or the end of a selection drag — named the cell above and
  left of the one under the pointer. All three now start from the same
  corner.

- **`<cells originLine>` no longer warns as an unknown prop.** The row was
  read by every binding and missing from the element's own list, so a grid
  that stamped where its screen sat in the scrollback got told the prop
  "is not a prop of cells: no binding reads it" — a warning that was both
  noise and wrong. `Ctx.openMenu`'s `items` parameter also reached the
  shipped `.d.ts` as a bare `Json`, which is not a TypeScript type: the
  examples' typecheck failed against it.

- **A selection survives a virtual list scrolling under it** (ADR 0017,
  tier 3). Two halves. The core places an end whose row is no longer built
  by that row's *data index*, so the part of the selection still on screen
  keeps its highlight while the list scrolls — before this the whole
  selection went blank as soon as an end left the built range. And a copy
  that reaches rows the core never saw now **asks the app**: `requestCopy()`
  answers `{text}` when the core has it all and `{asked: true}` otherwise,
  posting `{kind:"selectionrange", from:{index, byte}, to:{index, byte}}`
  on the scope; the app answers with `answerSelectionRange(text)` and that
  text is what reaches the clipboard.

  The alternative was copying the built rows and dropping the gap, which
  puts something on the clipboard that looks complete and is not. The rows
  behind the gap are the app's — it has the data, the core only ever had a
  screenful — so the app is who to ask.

  `env.request_copy()` / `env.answer_selection_range(text)` in Lua,
  `kui_request_copy` / `kui_answer_selection_range` in C, both new symbols.

- **Double-click and hold, then drag, selects by words** — and a third
  press by whole runs. The press arms the drag with what the click count
  says it moves by, and both ends round outwards: drag back over the word
  you started in and it stays whole, which is what makes the gesture feel
  like it is selecting words rather than snapping to them. A single press
  still drags by characters.

  A stock `<edit>` already did this — cosmic-text's `Selection::Word`
  expands both ends while the cursor moves — so this is a `selectable`
  scope learning the same trick, plus a test that pins the editor's half
  so it cannot quietly stop.

- **A terminal screen selects, in cells** (ADR 0017, step 4). A `cells`
  grid that declares `selectable` drags out a selection in cells rather
  than bytes: linewise by default, rectangular with Alt held, painted
  under the glyphs, and copied with each line's trailing blanks trimmed —
  the rule that makes a copied screenful paste like text instead of like a
  rectangle of spaces. A grid never joins a text scope around it, and the
  window still has exactly one selection: starting one in a grid clears the
  paragraph's, and the other way round.

  Its ends are **absolute lines**, not rows, so `cells` gains `originLine`
  / `origin_line` — the absolute line number of the grid's row 0. A screen
  is one screenful of an app's own history, so a row number means a
  different line after every scroll; stamping the origin is what lets a
  selection survive one. An app that says nothing gets 0 and a selection
  correct only while it does not scroll.

- **Copy carries formatting** (ADR 0017, decision 7). The clipboard now
  gets two flavours: the words, and the same words with the bold, the
  italic and the per-span colours the view declared. `selectionHtml()` /
  `env.selection_html()` / `KuiMenuAction.html` expose it, and the runner
  writes both so an editor that understands HTML takes the formatting and
  every plain-text field takes the words.

  What does not travel is the **node's** colour — that is the app's theme,
  not the text's, and a grey-on-dark paragraph pasted into a white document
  as grey-on-white is how this feature usually goes wrong. A span that
  declared its own colour is the other case, and it travels.

- **Force click, and Look Up** (ADR 0017, decision 6 — the question this
  whole ADR started from). A press that deepens past the second stage of a
  Force Touch trackpad is `InputEvent::ForceClick`, routed like a
  right-click: topmost node, no focus moved, no caret placed, no click, and
  the ordinary click the press is still producing arrives afterwards, as it
  does everywhere on macOS.

  Over an `<edit>` or a `selectable` scope it selects the word under the
  pointer and asks the host for the platform's definition panel — the same
  panel the standard **Look Up** row asks for, which is now offered in the
  stock menu on hosts that can show one. Elsewhere it reaches a node
  declaring `onForceClick`, for a force click on a chart or a map that
  means something the core cannot guess.

  Two host capabilities carry it, both usable by any host including C:
  `set_lookup_available` (offer the row, ask for the panel) and the
  `LookUp` menu action. The macOS driver answers with
  `showDefinitionForAttributedString:atPoint:` and asks the content view
  for `NSPressureBehaviorPrimaryDeepClick` at window creation — winit never
  sets a pressure configuration, and the deep-click stage is the gesture.

  Nothing synthesises a trackpad press, so the last two inches — the panel
  appearing, and stage 2 arriving — could not be tested here; they were
  confirmed by hand on a Force Touch trackpad, which is also where the
  remaining rough edges came from. The panel is anchored to the **baseline
  origin of the selection's first line** (a box's bottom draws the term a
  line low; a multi-run selection's union draws it under the last line
  while the panel shows the first), the gesture takes over the press that
  produced it rather than letting that press's drag take the word back, a
  force click in a run of spaces looks nothing up, and Look Up is offered
  only for a word or a short phrase — one line, at most a hundred
  characters. A dictionary handed a paragraph draws the whole thing back
  over the window and then says "No Results Found".

- **A standard menu row draws the shortcut its role has.** Copy reads
  `Ctrl+C` — `⌘C` on macOS, the same split `KeyMods::primary` makes for
  the key that produces it — without the app spelling one out.
  `MenuRole::default_accel()` supplies it the way `default_label()`
  supplies the wording, and `MenuItem::accel_text()` is what the stock
  menu draws: the row's own accelerator where it declares one, the role's
  otherwise. A `Custom` row still gets nothing, because only the app knows
  what key runs it, and `LookUp` gets nothing because the one platform
  with the panel draws its own menu.

  Display only, as `accel` has always been — the core binds no keys. It is
  also paint as far as a reader is concerned: `menuItem` is a
  name-from-content role and the widget names each row itself, so a screen
  reader hears "Copy", not "Copy Ctrl+C", and the accelerator adds no node
  of its own. It is not yet in AccessKit's `keyboard_shortcut` either, for
  want of a prop to carry it; a reader that would have announced the
  shortcut does not.

- **Windows draws the stock menu, not a Win32 one** (ADR 0017, decision 5,
  amended 2026-09-10). The `TrackPopupMenu` renderer that appeared earlier
  in this section was built, run, and taken out again: the drawn menu is
  what a Windows window gets, the same one Linux gets and the same one the
  conformance corpus tests.

  It worked. What it could not do is look like the app it was opened from.
  A popup `HMENU` is themed against the *process's* app mode and nothing
  else — the most it can be told is "follow the desktop", and even then
  the menu comes back in the desktop's grey, at the desktop's row height,
  in the desktop's typeface. A dark app on a light desktop gets a white
  menu and no call changes it. macOS keeps its `NSMenu` because Look Up
  and Services cannot be drawn at all; Windows has no such row, so there
  the native menu was charging the app's appearance for the system's
  metrics.

  What you give up on Windows, and Linux already had: the menu is a float
  inside the frame, so it is clamped to the window instead of spilling
  past its edge, and it casts no system shadow.

  Nothing moved in the seam. `set_native_menus` + `activate_menu_item` is
  still how a host renders menus itself, and a Windows host that wants an
  `HMENU` can still build one — the Rust runner has stopped asking for it.

- **macOS windows show the platform's own context menu** (ADR 0017, step
  3). The Rust runner declares `set_native_menus`, the core then holds the
  open menu without drawing it, and the driver shows an `NSMenu` at the
  press point: the platform's wording, the platform's keyboard, ⌘C / ⌘A
  beside the rows that have them, and a dimmed Copy when nothing is
  selected. Choosing a row runs the same code the drawn menu's row runs.

  The seam is not macOS's: `set_native_menus` + `activate_menu_item` is
  what any host with a menu of its own uses, C hosts included. macOS is
  the only platform the runner uses it on — see the entry above for why
  Windows does not — and everywhere else the core keeps drawing the menu
  it drew before.

  Worth knowing if you drive winit yourself: the menu is *scheduled* onto
  the run loop rather than shown where the press is handled.
  `popUpMenuPositioningItem` runs a nested modal loop, winit's macOS event
  handler panics when re-entered, and its run-loop observers fire in
  `NSEventTrackingRunLoopMode` — so popping a menu from inside
  `window_event` takes the app down the moment the pointer moves.

- **Context menus are data, and a right-click nobody claimed opens one**
  (`docs/adr/0017-selection-as-a-scope.md`, step 2). A menu is a list of
  items and a point: each item a label, an enabled flag, an optional
  payload and a *role* — `cut`, `copy`, `paste`, `selectAll`, `lookUp`,
  `separator`, or `custom` for one the app invented. `openMenu(key, x, y,
  items)` / `env.open_menu(...)` / `kui_open_menu(...)` opens one over a
  node; choosing a row posts `{kind:"menu", role, item}` on that node and
  closes it, and a press outside or Escape closes it with nothing posted.

  **The menu the core opens is the widget you can call.**
  `widgets::context_menu` is public, and the automatic path calls exactly
  it — so an app that answers its own `onContextMenu` to add two items of
  its own keeps the layout, the arrow keys, the dismissal and the access
  rows. It is a viewport-anchored float at the press point with `fit` (so
  it flips in off an edge), `modal` (so ADR 0003 dismisses it) and
  `menuItem` rows under a `menu` (so ADR 0007's keyboard and a screen
  reader both find it).

  **Automatic where it can be.** A right-click inside an `<edit>` offers
  Cut, Copy, Paste and Select All; one inside a `selectable` scope offers
  Copy and Select All; anywhere else it offers nothing, because a
  right-click on a plain box has never opened a menu. A node that declares
  `onContextMenu` wins and gets today's event unchanged. Rows that cannot
  act — Copy with nothing selected — are drawn dimmed and inert rather
  than left out, so the row a reader reaches for stays where it was.

  The clipboard remains the host's: Copy and Cut queue the text the core
  worked out, Paste asks for what is there, and the Rust runner answers
  both through the calls Cmd-C/V already use. C hosts drain the same queue
  with `kui_take_menu_action`. No ABI version moved — the new calls are new
  symbols, and `KuiMenuItem` / `KuiMenuAction` are new structs.

- **`selectable` — text outside an editor can be selected, and the
  selection reaches past the viewport** (`docs/adr/0017-selection-as-a-scope.md`,
  step 1). A node that declares it becomes a *selection scope*: the text of
  every node inside it is one run, in tree order, and a press-drag across
  three labels selects them as three lines of one text. A double press takes
  the word under it, a triple the whole run, and `kui_copy_text` /
  `Core::copy_selection` reads whichever selection the window has — a
  scope's or the focused editor's, never both, because starting either
  clears the other.

  The part that is not obvious: **a run the frame built but never drew is
  still part of the selection.** A label scrolled out of its scroller keeps
  its place in the order and its content reachable, so a drag that runs off
  the bottom of a list copies what the reader dragged over rather than what
  happened to be on screen. Hit-testing is unchanged — an off-screen run is
  under no pointer, and `textHit` still answers `null` for a point nobody
  can click. What the core never built (a virtualised list's unbuilt rows, a
  terminal's scrollback) it does not pretend to know; that is the next step
  of the ADR.

  A long line (the chunked path past 4096 bytes) joins the concatenation
  like any other run: being long is how it was shaped, not something a
  reader dragging across it should be able to feel.

  Scopes do not nest — the innermost owns the text under it, and the outer
  one is reported as `nested-selection-scope`. A control inside a scope
  still claims its own press, so a button in a selectable card is a button
  first. C hosts get a `selectable` field appended to `KuiSpec`, which the
  size handshake absorbs: no version moves, and a host that predates it
  reads zero.

- **`virtualColumn` / `virtual_column`, and `index`, so a long list is one
  call in every binding** (backlog C25). A JSX or Lua view that sliced a
  10,000-row list by `scrollGeometry` had three things a Rust one never
  needs, and this release is those three:

  ```jsx
  virtualColumn(ctx, { key: 'log', rows: lines.length, rowH: 28 }, (i) => (
    <box width="grow" height="grow" onClick={{ kind: 'pick', row: i }}>
      <text>{lines[i]}</text>
    </box>
  ))
  ```

  **The wheel raises no event, and a retained-tree binding redraws by
  re-lowering the tree it was handed.** So a JSX list that sliced by the
  geometry sliced by it *once*: `route_input` moves the container's offset
  and pushes nothing, and the Node loop runs `view` only when `update`
  returned a new model. Scrolling showed the spacers. The widget declares a
  zero-height node whose `onLayout` fires whenever the content moves, and
  the loop redraws on it **without handing it to `update`** — an app that
  had to add a `case 'layout'` for a widget to work has not been given a
  widget. Lua needs none of this (its view runs every frame) and gets the
  same call from the prelude.

  **`index`** is the second: a schema row, so all four bindings have it
  (`index={i}`, `index = i`, `kui_open_indexed`). It gives a node the key
  auto-keying would have given the `i`th child, wherever the node actually
  sits — so a row keeps its hover, focus, edit buffer and tweens as the
  built range slides over it, and a virtualised list and a full one agree
  on identity. Auto-keying is the *sibling* index, which a lead spacer
  shifts by one; `Ui::open_indexed` has been the Rust answer since alpha.7
  and had no spelling anywhere else. Also on a `line`, a `cells` and a
  `fragment`, wherever `key` names a node. The corpus scene `virtual` pins
  it in all four.

  A row's `index` is refused unless it is a whole non-negative number: folded
  to zero it would silently take row 0's key, and two of them in one frame
  would share it — the `duplicate-key` case, arrived at in silence.

  **A query now answers for a name no frame declared**, where a command
  still refuses. `scrollGeometry('log')` *threw* for a container the first
  frame has not built yet — which is every list's first frame — while
  `scrollGeometry(hexKey)` returned `null` for the same situation.
  `scrollOffset`, `scrollGeometry`, `isHovered`, `isPressed`, `isFocused`,
  `textHit`, `caretRect` and `editText` answer with their own "nothing"
  now; `focus`, `reveal`, `setScroll`, `setEditText` and `access` keep
  throwing, where a typo is a bug worth naming. In Lua the same call was
  worse than inconsistent: `scroll_geometry` and `scroll_offset` took an
  integer key *only*, so the container could not be named at all, and
  `is_hovered` / `is_pressed` were the same. All four take either spelling
  now, which is what the module doc always said.

- **`widgets::virtual_rows`: a virtual list whose rows are not one height**
  (backlog C26, steps 0 and 1). `virtual_column` takes a stride and every
  row must come out that tall. This one takes prefix sums over a
  `RowHeights` the app owns, and fills them from a `measure(ui, i, width)`
  it runs for the rows it is about to build and no others —
  `ui.measure_text(text, &style, Some(width))` is what layout would give
  that row, wrap, `max_lines` and the shaping cache included, so measuring
  a row and then drawing it shapes once. What `measure` returns is the
  height the row *gets*: each row's node is fixed to it, so the spacers can
  never disagree with the layout.

  Every row not measured stands at the mean of the ones that are, which
  means measuring a screenful changes the height of every row above the
  window too. Left alone that slides the content out from under the pointer
  on the frame the list learns anything, so the widget takes the row the
  window starts in and how far into it, measures, and puts that pair back:
  `Core::set_scroll` from inside a view lands on the frame being built (the
  positions pass reads the store after the view has run), so the corrected
  frame is the only one ever seen. What does move is the scrollbar, which
  is the honest thing to move. To *stay* at the end of a growing log, ask
  for it — one `set_scroll(key, huge)` after the widget, every frame.

  **It stays a second widget rather than replacing the first**, and the
  measurement is why it could have gone either way: at one height it costs
  what the uniform one costs. `list_10k_rows_variable_at_one_height` (the
  variable widget told, row by row, that every row is 24px) against
  `list_10k_rows_virtual` (the same 38 rows through the stride) is **16.79
  µs against 16.22 µs**, and three runs of these benches move their own
  medians by up to 10%, so that gap is not a reading. What differs is not
  the frame but the *state* — a `RowHeights` the app owns, an allocation
  per row, and a `set_len` when the data changes — and a caller who can
  name the stride should not have to carry any of it.

  Normalised per row, every virtualised frame here costs the same 427–455
  ns a row, and a row built naively costs 393 ns. That is the whole of the
  242× (`list_10k_rows_naive`, 3.93 ms): virtualisation does not make a row
  cheaper, it makes there be 38 of them instead of ten thousand. It is also
  why `list_10k_rows_variable` reads *under* the uniform bench — its rows
  average 32px against 24px, so eight fewer fit the window. Only the
  at-one-height pair holds that equal.

  Two numbers worth keeping from building it. The prefix sums are filled
  lazily and only as far as a query asks, and the search gallops out from
  where the last one landed rather than bisecting the whole list. Both were
  needed: laziness alone buys nothing, because a bisection from `0..len`
  probes the midpoint first and fills everything under it. The first cut
  had neither, and the same benches run against both implementations say
  what they were worth:

  | | first cut | shipped |
  |---|---|---|
  | a frame that learns nothing, 10k | 14.41 µs | 13.66 µs |
  | a frame that learns a row it is looking at, 10k | 21.24 µs | **13.20 µs** |
  | the same at 100k | 84.43 µs | **13.31 µs** |
  | a row nowhere near the window, 10k | 28.08 µs | 18.83 µs |

  So learning a row's height now costs nothing measurable, and is flat in
  the length of the list where it used to be six times the steady cost at
  100k rows. The one case that still pays is a list told about a row
  nowhere near its window — a list whose data changed under it — which
  pays for the distance between that row and the window, and is the honest
  cost of a prefix sum.

- **`KuiWindow.pumpUntil(timeoutMs)`, a pump that parks** — the Node door
  onto `PumpRunner::pump_until` (backlog C21), returning the moment an OS
  event or a `Waker` wake arrives and otherwise when the time is up. For a
  driver that owns its process; read the warning about libuv under
  **Fixed** before reaching for it. `runWindowed` does not use it.
- **`idlePumpMs` and `quietMs` on `runWindowed`** — the ceiling the gap
  between pumps grows to, and how long a window must have been quiet
  before it starts growing. See **Fixed**; the defaults (32 and 500) are
  what an app should want, and an app happy to answer a click into a deep
  idle more slowly can raise `idlePumpMs` — it is linear in both CPU and
  that first click.
- **`KuiWindow.nextDeadlineMs()` and `PumpRunner::next_deadline`** — when
  the runner next needs pumping, so a host driving from a foreign loop can
  sleep to it instead of guessing. Zero means "now": an OS event has been
  handled and its redraw is waiting, which is also the only way a driver
  hears about input the app itself never sees (see **Fixed**). The caret blink, a transition's next
  frame, the audio poll and a window's first-frame retry are deadlines the
  shell has already worked out, and a driver on a fixed interval hits each
  one a whole interval late: with a 200 ms interval and a sound in flight,
  a probe pumped every 203 ms while the shell was asking for 50 — asking
  brought it to 63. `runWindowed` uses it; it only ever asks for *sooner*,
  since whether an OS event is waiting is not something it can say.
- **`app.step()` reports whether the turn drew.** It returned nothing; it
  returns the boolean it already computed, which is what lets a driver
  pace itself from what the loop actually did.

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
- **A click in a Node window paints once, not twice.** Press, and the
  button went down; release, and the button came up in one frame and the
  count moved in the *next* — a two-frame release, plain to see at the
  8 ms pump. The release's frame was painted inside the pump that carried
  the release, from the tree JS had submitted before it, since `update`
  runs in JS after the pump returns; only then did `setView` ask for the
  frame with the count in it. Measured with a probe that samples the frame
  counter between pumps and inside `update`: before, the pump that
  delivered the `add` event painted a frame before `update` ran and the
  count's frame came 14 ms later; now that pump paints nothing, and the
  one frame after `update` shows the button let go and the count moved
  together.

  So **a frame can now wait one turn for the host's answer**.
  `Launcher::deferred_events()` — set by the runner behind `KuiWindow`,
  and not reachable by an app that does not drive the loop itself — says
  the host answers after `on_event` returns. Under it, a redraw waits when
  the app owes an answer to an input, and the host ends the wait by saying
  its view is current: `setView` already did, and a drained `pollEvents`
  now does too, so an input produces a frame even when the handler submits
  no view. Rust, C and Lua apps answer inside `on_event` and are
  untouched; the `App` trait is unchanged.

  **Two bounds are what make waiting safe, and the obvious version of this
  change has neither.** A wait that simply stands until the host speaks
  starves the window, because winit hands a pump its input *before* that
  pump's redraw: under a stream that reaches the app, each pump re-arms
  the wait before the frame the host just asked for is delivered. Measured
  on a node with `onDrag`, a 578 ms drag painted **one** frame. The same
  unbounded wait freezes a window for the length of a title-bar drag,
  since a platform's modal move loop never returns to the host that would
  end it. So:

  - **A frame never waits twice running.** Whatever happens, a window
    keeps painting — the worst case is half its frames, and a modal loop
    the host cannot interrupt costs one frame, not the whole gesture.
  - **Only a discrete input is worth waiting for.** A press, a release, a
    keystroke, an IME commit and an assistive-technology action finish
    something the core itself drew, and a frame showing the button let go
    with the count unchanged is a frame that lies. A pointer moving, a
    wheel turning and a preedit being revised are a stream, where content
    trailing by a frame is what every toolkit does. With both bounds the
    same drag paints 36 frames, which is what it painted before the
    change.
- **A control pressed from the keyboard shows its focus ring.** Click the
  `+1` button in `examples/rust/counter` and then press Space: the count
  goes up — Space has pressed the focused control since alpha.6 — and
  nothing on screen says *which* of the three buttons answered. Press Tab
  once first and the ring appears, and from then on Space looks like it
  works. So the report this came from reads as "Space does nothing until I
  press Tab", and what was actually missing is the ring.

  A click focuses without showing, deliberately (`focus_visible`, the
  web's `:focus-visible`) — a pointer user should not collect a ring
  around everything they touch. But that is a rule about how focus
  *arrives*, and the moment a key acts on it the user is owed the answer
  to "which node did that?". So each key the core acts on the focused
  control with now shows the focus first: Space and Enter pressing it, a
  slider's arrows, and the composite arrows that already did. Escape does
  not, because it acts by letting go; nor does a key the control does not
  claim, which went to the enclosing sink and never reached the control at
  all. The next mouse press hides the ring again, as before.
  `docs/adr/0002-keyboard-focus-as-data.md` carries it as decision 4a.

- **An idle window costs nothing again.** Three separate things kept a
  process that was doing nothing from settling at zero, and each was
  measured on its own before it was touched — a windowed app with none of
  them (`examples/rust/gallery`) already idles at 0.0% CPU and one context
  switch a second, which is what made the other three legible as defects
  rather than as the cost of having a window.

  **The audio device stayed open forever.** A session that holds a sound
  warms the output device at launch, so the first click does not stall for
  the ~90 ms open — that is alpha.9's fix and it stays. But an open device
  is a real-time thread the OS calls every buffer period whether or not
  anything is playing: 94 times a second at the usual 512 frames, which is
  0.3% of a core and *the whole* of `examples/rust/counter` sitting idle
  (16 threads, 92 wakeups a second). It is let go after five seconds with
  nothing playing and nothing to play — the decoded-sound cache stays, so
  what a re-warm pays is the open and not the decode — and any input at
  all warms it again, which is minutes of warning before a button is
  pressed. The counter now idles at 0.0% with 8 threads and 2 wakeups a
  second, and its click still sounds.

  A detail worth naming, because it is the case that mattered: a device
  warmed and never commanded sits in `Opening` forever, with the opened
  manager live in the channel and its stream running, since nothing else
  asks whether the open has landed. Closing has to settle that state
  before it can read it.

  **The caret blinked in windows nobody was typing into.** The blink is
  the one thing in an idle app that asks for a frame twice a second for
  ever, and it ran on widget focus alone — so `examples/rust/editor` left
  in the background drew 120 frames a minute, 0.7% of a core, for a caret
  no keystroke could reach. It now stops with the window's keyboard focus
  (`env.focused`), and the caret hides rather than parking solid, since
  solid is what a focused field looks like. Unfocused: 0.7% to 0.0%.

  **Node's driver polled the OS 125 times a second.** A pump costs the
  same empty as full — on macOS it runs a whole `NSApp` iteration and
  posts a synthetic event through the window server to break out again,
  about 1.2 ms of CPU — so the rate was the entire bill, and it was
  linear: 8 ms cost 8-15% of a core, 16 ms half of that, 100 ms a tenth.
  The gap between pumps now grows while nothing is happening — `pumpMs`
  (8) while the app is being used, doubling after `quietMs` (500) of
  silence up to `idlePumpMs` (32) — and anything at all puts it back at
  once. `examples/node/counter-window` idles at ~3%, from 8.7%.

  What "anything at all" has to mean was the trap, and it took clicking a
  real window with posted `CGEvent`s to find: **most OS input produces no
  app event.** A pointer crossing a window that declares no hover is a
  stream of events the shell acts on and the app never hears about, and a
  driver pacing itself on what `update` saw reads that as an idle window —
  so it went on backing off while the pointer was reaching for a button.
  Measured click-to-update was **520 ms** at worst, against 34 ms for a
  driver that never backs off. It is the runner that knows, so the runner
  says: after any OS event `nextDeadlineMs()` reports 0 — it has a redraw
  to present — and the driver takes that as work. With that, and the
  ceiling at 32 ms, a click into a fully idle window answers in ~55 ms
  (worst 72), and one into a window a pointer has approached in ~53 ms.

  The trade is `idlePumpMs` and it is linear both ways: 8 ms is 34 ms a
  click and ~9% of a core, 32 ms is ~55 ms and ~3%. Only the first event
  after half a second of silence pays it; the second is already back at
  `pumpMs`.

  What the driver deliberately does not do is park inside the pump, which
  is the obvious fix and looks strictly better: `KuiWindow.pumpUntil(ms)`
  wakes on the event itself, so there is no latency at all. It is exposed,
  and a driver that owns its whole process can use it — but a blocked main
  thread is a blocked libuv, and Node cannot be asked whether that is
  safe: `process.getActiveResourcesInfo()` reports nothing at all for an
  in-flight `fs.readFile`. Behind a 50 ms park, 200 sequential
  `await readFile` went from 6 ms to 4 s.

  Backing off turns out to be the best of the three options rather than a
  compromise, which is backlog C27 and its measurements. The cost is not the pump
  rate in the abstract: leaving and re-entering the platform's loop is
  ~1 ms of real main-thread CPU (987 of 1011 µs, awake, by
  `CLOCK_THREAD_CPUTIME_ID`) where a `WaitUntil` wake *inside* `run_app` is
  ~46 µs. A third of it is winit breaking out of `[NSApp run]` by posting a
  synthetic event and AppKit answering that event with a LaunchServices
  `sysctl`; the rest is the door itself. Which is why a Rust app has
  nothing to back off from — it never leaves the loop — and why parking is
  not the fix it looks like: winit 0.30 stops a parked pump on the first
  wake of any kind, so a 200 ms park runs 101 ms, and parking costs 4.59%
  against polling's 1.80% at a 32 ms period.

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

- **`lua_panel` loads the C panel on macOS and Linux**, not only on
  Windows. `examples/README.md` has said since 2026-09-08 that once
  `cbuild` has run the Lua panel is three languages deep, and on the
  unixes it never was: the third pane read "no native panel loaded" with
  `symbol not found '_kui_button'` under it, and the script's own comment
  called that the expected outcome. It was a missing link arg. A plugin
  there leaves every `kui_*` undefined and resolves it from the executable
  that loaded it, and rustc dead-strips an executable's unreferenced
  functions unless the linker is told to keep them — which is exactly what
  `kui-ffi/build.rs` passes for `c_panel` (`-export_dynamic`,
  `--export-dynamic`) and nothing passed for `kui-lua`'s example. So
  `kui-lua` has a `build.rs` doing the same for its examples, `lua_panel`
  exports 185 entry points, and the pane shows the panel; the window is
  1240 wide so the third pane is in it. The pane also reports the *first*
  real reason a load failed — the `.so` that exists — rather than the last
  path's "no such file". Windows is untouched: the shape that loads there
  imports `kui_ffi.dll` by name and needs no export from the host.
  Found by hand on the pre-tag round — read as a regression, and it was
  a gap that had been documented as a fact since the day it was written.

- **`announcement-repeated` no longer fires on two real presses in a
  row.** The rule was "the same text on two consecutive frames", and in a
  window that redraws only on input two consecutive Copy presses *are* two
  consecutive frames — `scripts/ax-audit.swift`'s "the same message twice
  in a row is said twice" check, which presses Copy twice on purpose,
  raised it on every run, and any app with a Copy button would have too.
  ADR 0008 always said a message repeated across frames the user did
  something between is not the case; the core now reads "did something"
  off the events it handed the app rather than the frame count, so the
  view-shouting-every-frame shape is still reported and a press between
  is not. Pinned in `tests/live.rs`, mutation-checked.

### What you can delete

The hover or click box floated over a wedge, a connector or a shape's
middle to give it a target — the polygon example's own went — and the
`role="none"` that kept an unpressable "button" out of a screen reader's
way (ADR 0026).

The `add_image` + `remove_image` pair an app wrote around every streamed
frame, and the handle it re-threaded through its view each time; the
"keep it under 4096" check in front of `add_image`, and the blank box an
app could not explain past it; the aspect arithmetic in front of an
`image` that wanted `contain`; the three-segment closed `line` that stood
in for a filled arrowhead, and the eight thin boxes that stood in for a pie
wedge; and the `env.scale` a view carried into its `layout` handler to
know how many pixels a box was (ADR 0025).

The `keyFocus` / `take_key_focus` on your root sink whose only job was
giving chords somewhere to land when nothing was focused, and the
`frames >= 2` guard in front of it so an editor's `autofocus` got the
first frame. A root sink hears every unclaimed key with nothing focused,
so the harness that carried both now carries neither. (Not the "hand Tab
back to the ring" branch: a press on dead space still gives focus to the
enclosing sink, ADR 0011, and a sink holding focus still keeps every key,
Tab included — that hand-off is the one a root sink still owes.) And the
`role="none"` on a panel
whose only purpose was keeping its buttons out of the app's Tab ring — it
also kept them from every keyboard and screen-reader user; `focusRegion`
keeps the ring out and nothing else.

The guard that kept an `autofocus` editor from retaking focus after a blur
— a `focused` flag beside the model, a `blur()` in the next frame: the flag
asks once now.

The order you declared floats in to get one over another, and the wrapper
you moved a menu bar's dropdown into so it would paint over the body's
tooltips: a float is above what opened before it wherever it sits in the
tree. And the gutter you kept clear of a scroller's right edge so a popover
would not come up under its bar — the bar is under the popover now.

The `pumpMs` you lowered to make a Node window feel responsive, and any
timer an app added beside `runWindowed` to keep it awake: the gap is 8 ms
whenever anything is happening and grows only into silence, so tuning it
down buys nothing but CPU. If you raised it instead, to stop the window
costing something while idle, that is now what the backoff does — and it
does it without the input latency you were paying for at every moment,
including the busy ones.

The cast, or the node you moved the accent onto: `accent` is a row on the
stock button in TypeScript as it already was on the wire, and a button that
carries it needs nothing standing beside it.

The width you measured for a rename field, and the headroom you added past
it. A field that hugs its text is `width: "fit"` now, and it grows on the
frame the keystroke arrives — the core re-lays out the tree it was handed,
so there is no frame where the text is wider than the box it is drawn in
and nothing for a margin to absorb. If the box is a fixed size, the second
line the text used to fold onto is gone too: it scrolls.

The sentinel, the `case 'layout'` beside it, the `key={String(i)}` on every
row and the `keyOf` guard in front of `scrollGeometry` — the four things a
JSX virtual list needed and a Rust one never did. `virtualColumn` is the
call; `examples/node/virtual-list.tsx` is that file with all four deleted,
printing what it printed before.

The row cap on a list whose rows are not all one height — the "one line
each" a log viewer settled for so a stride would describe it, the
`onLayout` on every row copying heights into a model, and the running total
kept beside them. `virtual_rows` measures what it builds and estimates the
rest, and the row you scrolled to stays where it is while it learns.

The palette branch that could only read `"unknown"`. `env.system` is filled
in before your first view, and a change to it is a message — so a view that
picks its colours from the OS can be written the way it reads, rather than
against a reading that never arrived.

**Your palette.** The `struct Pal`, the six `const BG/CARD/EDGE/TEXT/MUTED/
ACCENT`, the `#8a8fa3` you typed for the caption under the card — the whole
of it, if what it held were UI roles. `ui.theme()` names them, derives them
from the appearance and the accent the OS reported, and the stock widgets
paint from the same ones, so a menu, a tooltip and your own card agree
without you passing anything between them. What is worth keeping is the
half that was never a role: a syntax highlighter's keyword colour, a
chart's series, a brand illustration. Those are the app's, and always were.

**The `if dark { … } else { … }` you were about to write.** Following the
OS is the default rather than a mode, so the branch is a `Theme::derive`
you never call. What is left for a view to decide with `theme.is_dark()`
is the handful of things a palette cannot carry: which of two images, how
heavy a shadow.

**The readability arithmetic on an accent-painted control.** `on_accent`
is black or white by the accent's luminance, and the hover and pressed
shades come off it too — the trio a view was computing by hand every time
it wanted a second button in a colour of its own.

Nothing for the two performance changes above, and that is the point: a
view declares no clip and no tween slot, so the only code either one asks
to change is a backend or a test reading the clip off a quad — the
**What breaks** list, rather than a workaround this release made
unnecessary.

### Native verification

The by-hand round alpha.6 introduced (backlog R4), run before this tag on
2026-09-11 on `main`. What follows is what executed on what.

**macOS 26.6.2 (arm64), rustc 1.98.0, Node 26.8.1.** `cargo test
--workspace` passes: **1035 tests over 89 suites, 0 failed** (1 ignored: a
doc example in the devtools' `drive.rs`), with no display, no installed
fonts and no GPU. `cargo fmt --all --check` and `cargo clippy --workspace
--all-targets -- -D warnings` are clean. The scene corpus runs in all four
adapters against one reference report: **31 scenes** — `anchor`,
`layers`, `menu`, `menubar`, `polygon`, `scrollbar`, `selection` and `virtual`
new since alpha.10's 23 — Rust and Lua through `cargo test`, C through
`target/debug/conformance` (the header at **344 fields, 224 enum members
and 187 prototypes**, matched against the Rust side by the parity assert,
and every prototype now redeclared under its Rust signature by the same
test), Node through `npm test` with `KUI_CONFORMANCE_REQUIRED=1` (**124
Node tests**, 0 failed, 0 skipped). The C round, `cbuild --run`, passes
its five checks: the C counter's drive, the header walk, a C host loading
the C panel, the same plugin in a Rust host, and the plugin with
`kui_ext_abi` deleted refused against **ABI 15**. `npm run gen` leaves the
three generated files unchanged; `examples/node` installs, typechecks,
builds and runs its three headless drives; the headless round
(`smoke -- --headless`) passes all 22 drives.

**`scripts/check-version.sh 0.1.0-alpha.11` refused the tree first**, and
was wrong to: `kui`, `kui-lua` and `kui-ffi` dev-depend on `kui-devtools`
— the examples' harness, `publish = false`, added after alpha.10 — with a
path and no version, and the check read that as "requires kui-devtools
\*". `cargo package -p kui` ships an empty `[dev-dependencies]`, since
cargo strips a path-only dev-dependency, so there is nothing at the
registry to agree with; the check skips exactly that shape now. The same
class as alpha.10's `kui-ffi` miss, from the other side: a crate added
to the workspace that the version tooling had no rule for.

**The windowed round**, `cargo run -p kui-devtools --bin smoke -- --node`:
**31 Rust examples on both bases and the four Node windows, 120 frames
each, every one exiting 0 with nothing on stderr** — the round reads the
warnings, not just the exit code, since alpha.10. The C and Lua hosts are
not `[[example]]`s of `kui`, so by hand: `target/debug/counter`,
`target/debug/host`, `c_panel` and `lua_panel` each opened a window under
`KUI_SMOKE_FRAMES=120` and exited 0, warning-free — **39 windows over five
hosts.**

**What the hosts' windows found, read rather than counted.** `lua_panel`'s
third pane said "no native panel loaded", which the file called the
expected outcome on the unixes and the README called three languages deep.
It had never loaded on macOS: the host executable exported no `kui_*` —
`nm` shows 0 against `c_panel`'s 187 — because only `kui-ffi`'s build
script passed the linker the flag that keeps them. `kui-lua` has the same
build script now; the pane shows the C panel inside the Lua panel inside
the Rust host, and an AX press on the host's button counts (**Fixed**
above). Found by hand, reported as a regression, and a gap since the day
it was written.

`scripts/ax-audit.swift` against `examples/rust/features/accessibility.rs`
passes **106/106 checks**, compiled with `swiftc -O` (the README says so
now, and why). The first run passed 106/106 **and left one warning on the
fixture's stderr**: `announcement-repeated`, raised by the audit's own
"the same message twice in a row is said twice" check, which presses Copy
twice on purpose. The rule was frame-count-based, and in a window that
redraws only on input two presses are two consecutive frames — a false
positive any app with a Copy button would have hit. The rule reads the
events now (**Fixed** above, ADR 0008 amended); the second run is 106/106
with a clean stderr.

**The gestures on the by-hand list.** `node dist/features/slide.mjs`,
then a synthetic drag posted through the HID tap with `screencapture` run
*while the button was down*: at "move pan −22,−14" and "move pan −67,−41"
the four cards and their connectors had moved together to where the
cursor was, and at "end pan −90,−55" they sat at the full offset — F15
still absent. ADR 0009's gesture on `features/popup`: press on the
field, four legs down into the list, release — the list opened on the
press, the highlight followed the drag (Cadmium, then Lithium) and the
release chose Lithium, which the field then read. And new this release,
a secondary press on `widgets/context_menu`: the row's menu came up as
the platform's own `NSMenu` with the app's two items over Copy ⌘C /
Select All ⌘A, the article's as Look Up / Copy (both dimmed with nothing
selected) / Select All, and the footer's press opened nothing — each as
the example's own doc says.

**Windows 11 Pro 26200 (x64), rustc 1.98.1, Node 25.2.1, MSVC 14.51**,
run on 2026-09-12 before the tag, the same round as above minus the AX
audit. `cargo fmt` and `clippy` clean (on 1.98.1 — the 1.96.1 this
machine had refused `tests/conformance.rs` under a `nonminimal_bool`
that 1.98 no longer raises; CI runs stable, so the toolchain was moved
rather than the code). `cargo test --workspace`: **1036 tests over 89
suites, 0 failed**, one more than macOS for a Windows-only case. The
corpus's 31 scenes pass in all four adapters, the header at the same 344
/ 224 / 187 matched against the MSVC layout, `npm test` at **124 tests,
0 failed, 1 skipped** (below), `npm run typecheck` on `examples/node`
clean, the headless round's 22 drives green, `check-version.sh` content.
The windowed round, `smoke -- --node`: **31 examples on both bases and
the four Node windows, 120 frames each, every one exiting 0 with nothing
on stderr**; `counter.exe`, `host.exe`, `c_panel` and `lua_panel` by
hand the same, and `lua_panel`'s window looked at rather than counted:
its third pane reads "the same panel, in C", so the plugin loads three
languages deep on Windows too, through `panel.dll`'s import of
`kui_ffi.dll` rather than the export-dynamic the unixes needed above.

**What the Windows round found — two files cl had never compiled and a
test that had never run here.** `cbuild --run` had not met MSVC since
alpha.10: `examples/c/common.h` (new on 2026-09-10) declared its failure
counter `__attribute__((unused))`, which cl rejects outright — it is
gcc and clang's `-Wall` that names a static a program never reads, and
cl has neither the warning nor the syntax, so the attribute is behind
`__GNUC__`/`__clang__` now. `examples/c/features/slots/host.c` reset its
event with `(KuiEvent)KUI_EVENT_INIT`, a cast of a struct to its own
type that gcc and clang admit as an extension and cl calls C2440; the
macro is already a `KuiEvent` compound literal, so the cast is gone. And
`packages/kui/test.mjs`'s `RTLD_GLOBAL` pin, added in alpha.10 *after*
that release's Windows round, asserted a flag on a platform that has
none: `os.constants.dlopen` is `{}` on Windows and `native.cjs` then
calls `process.dlopen` without flags, as its own comment promises, so
the test skips there with the reason and asserts everywhere else. The
skip is the one in the count above. None of the three touches a shipped
binary — two examples and a test — and the round passes on all three
platforms it has now run on.

One thing seen and not fixed: `npm run gen` on Windows rewrites
`docs/props.md`'s `titlebar_h` default from 34 to 32, because the
generator reads `Metrics::default()` off the platform it runs on. CI
generates on Linux and the file agrees with Linux, so the check holds;
the generator is not platform-independent, which is backlog **W13**
rather than a release line.

**Not run**, and stated: W3 and the mixed-DPI popup case stay as alpha.10
left them, since nothing on the Windows window path changed here beyond
the Win32 context menu, which has no round on this machine; the bench
guard, whose record above is macOS's and whose README table is that
machine's; and Linux, which alpha.10 ran in docker for the `RTLD_GLOBAL`
case and this release did not, since `native.cjs`'s dlopen flags did
not move and the test that pins them ran on macOS.

**The bench guard, run alone and on a quiet machine.** `scripts/bench-check.sh
v0.1.0-alpha.10` reports **all four guarded rows clean** —
`frame_10k_rects` +0.9%, `frame_10k_rects_with_text_and_hits` +1.0%,
`frame_1k_typical` +3.1% and `deep_nesting_64_levels` −2.6% — with the
worst run-to-run spread on a guarded row at 6.8% and no load warning, so
for the first time since alpha.7 **the README's table is refreshed from
this run**, HEAD's medians over 2026-09-07's, on the rows both have.
`frame_10k_rects_with_access_tree` reads **−23.2%**, which is ADR 0016's
one built decision.

**Four unguarded rows read slower, and they reproduce**: run again alone,
`frame_10k_segments` +12.2% then +9.3%, `drop_1k_rows_plain` +7.4% then
+7.6%, `frame_10k_rects_square_clip` +6.7% then +3.8%,
`frame_10k_rects_rounded_clip` +6.3% then +5.5%, each on a spread under
3%. The clip shrink priced itself at +2.5% on the rounded clip and nothing
on the square one, so the rest arrived in the rounds after it — a bisect
over 87 commits, which is not a pre-tag job. Filed as **C29** in the
backlog with the numbers and the probe to run; the tag ships with them
stated rather than with a bench that read the same as the last one.

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
