---
status: accepted
date: 2026-09-10
---

# One subject per example, and the smoke round that reads the tree

> **Accepted (2026-09-10), built the same day** — every phase, with the
> deltas the building found listed under *What the building changed*.
> The examples grew one at a time, each written
> for whatever was being built or verified that day, and the tree shows
> it: a file is named for the app it happens to be rather than the thing
> it shows, several show five things, three stock widgets are the subject
> of no example and the side dish of nine, and four features exist only
> inside the C header walk. The bindings' directories follow no rule at
> all beyond "what a round once needed". At the same time the examples are
> the repo's only windowed check and half of its by-hand round, and any
> reshuffle that forgets that loses coverage nobody will notice missing.
>
> This ADR fixes the axis: an example has **one subject**, the subject is
> a **widget**, a **feature** or an **app**, the file is named for it and
> sits in the directory for its kind; a binding carries the same taxonomy
> and a subset chosen by one rule. It then makes the smoke role explicit —
> three channels, each enrolled automatically from something a script can
> read, so that moving a file cannot drop it from a round — and closes the
> gaps the audit found while it is there. The scaffolding every example
> repeats today (the HUD, the title, the `--headless` parsing, the theme
> keys one example has) moves into a **harness** the example runs inside,
> which is also what lets the windowed round open every example on both
> theme bases without any example knowing.

## Context

### What the tree holds today

Twenty-nine files under `examples/`, one directory per binding, every
Rust one registered by `path` in its crate's `Cargo.toml`
(`examples/README.md` says why). Read by what each one is *about*:

**Rust, 18 files.** Six are apps that own their keyboard and touch
everything — `counter`, `splitmux` (1,198 lines), `modal_editor` (1,007),
`syntax_view`, `editor`, and `context_menu`, whose own header opens with
"five things to try, and each one is a different rule" (a stock menu over
selectable text, the app's menu over a row, a `cells` grid selecting in
cells, an editor's clipboard menu, and the menu bar of ADR 0018). Seven
are one feature each and say so — `toasts` (enter/exit), `bulk_exit` (the
budget), `waker`, `popup`, `theme`, `accessibility`, `fragments`. Four are
one element each without saying so — `gallery` is `image`, `connectors`
is `line`, `rich_text` is `text` spans (and `selectable`, as a second
subject), `virtual_list` is the widget of that name. One is a tool
(`conformance-dump`).

**C, 4 files.** `counter.c` is 2,590 lines and three programs: a counter
app with a `--headless` click round-trip, a 2,000-line `surface()` that
walks every prototype in `kui.h` (the FFI self-test CI runs), and the C
corpus adapter (`--conformance`). The other three are one story — C as an
extension (`panel.c`), the Rust host that loads it (`panel.rs`), the C
host that loads it (`host.c`).

**Lua, 3 files.** The same panel in Lua with its Rust host, and a
lowering bench. There is no Lua *app* example because there cannot be one:
a Lua extension has no window of its own (ADR 0014).

**Node, 4 files.** `counter.tsx` (the headless self-check CI runs),
`counter-window.tsx` (the same app under the pumped winit loop, plus
images, sounds and an editor), `mindmap.tsx` (floats, `line`, `onDrag`,
every tween retargeted per frame — the by-hand check for F15), and
`virtual-list.tsx`.

### What the mixing costs

Grepping the tree for what shows what, the axis problem is concrete:

- **Subject of none, side dish of many.** `titlebar`/`titlebar_with` is
  called in nine examples and demonstrated by none; `tooltip` in seven,
  none; `latency_hud` in fourteen. A reader looking for "how do I do
  custom chrome" has nowhere to go, and the `kui::widgets` doc says custom
  widgets should follow the pattern the stock ones set.
- **Only the C header walk has it.** `keyframes`, `hover_group`,
  `window_buttons`, `latency_graph` and the span decorations of C22 (ABI 8)
  are exercised by `counter.c`'s `surface()` and nowhere else. They are
  features with a corpus scene and a `props.md` row, and no Rust example
  — the walk proves the C prototype exists, not what the feature is for.
- **One file, several subjects.** `counter` is "minimal" and shows the Elm
  loop, `click_sound`, `hover_sound`, an `audio` node, `on_context_menu`
  and a `modal` float. `context_menu` is the five above. `rich_text` is
  spans and selection. A change to any one subject edits a file named for
  another, and its header grows a bullet list.
- **The bindings follow no rule.** Node has four examples chosen by what
  once needed a check; C has an app that is really a test; Lua has the
  extension story and a bench. The only cross-binding correspondences are
  accidental: three counters share a shape (Rust, C, Node), two panels
  share a contract (C, Lua), two virtual lists share a widget (Rust,
  Node). Nothing says which of those was meant.

### What the examples already are, besides examples

This is the half a reshuffle must not lose. Five rounds read the tree:

1. **`scripts/smoke-windows.ps1`** — every `[[example]]` of the `kui`
   crate, read from `cargo metadata`, opened for 120 frames under
   `KUI_SMOKE_FRAMES`, exit 0 or fail. Automatic enrolment: a new example
   is smoked the day it is added. Windows only, and only when a runner
   exists; the macOS equivalent is by hand with `screencapture`
   (`kui-macos-window-quirks`).
2. **`check` in `ci.yml`** — `conformance-dump` writes the reference
   report; `counter --headless` (the header walk) and `counter
   --conformance` (the C adapter); `build.sh --run` (the plugin into a
   Rust host, into a C host, and the `kui_ext_abi`-less mutant refused);
   `npm run typecheck` in `examples/node`, which is what exercises the
   shipped `.d.ts` ("the example app is the one that uses every
   app-facing type"), then `node dist/counter.mjs`, which exits 1 on
   `MISMATCH`.
3. **The by-hand round** (`### Native verification` in the CHANGELOG):
   `accessibility` under `scripts/ax-audit.swift` (106 checks); the
   `mindmap` drag (F15); `toasts` under a Windows title-bar drag (W3);
   `popup` press-drag-release (ADR 0009); `waker` under
   `KUI_WAKER_LINES`; the `bulk_exit` boundary watch; every host's window
   captured.
4. **The benches** — `lua/bench.rs` and `node/bench.mjs`, the shootout.
5. **Nothing.** `lua_panel` is built by `clippy --all-targets` and run by
   no round. `c_panel` runs only inside `build.sh --run`. `virtual_list
   --headless` prints four lines and asserts none of them; `splitmux`'s
   headless drive is a test module. Only two examples in the repo can
   fail on a wrong *answer* — Node's `counter` and C's `counter
   --headless`. The rest can fail only on a crash.

Enrolment is therefore automatic for one binding on one platform, a
hand-written list for Node (`package.json`'s build script plus one line
in `ci.yml`), a script's own list for C, and absent for Lua.

### What the codebase already believes

Three conventions the rest of the repo holds and the examples do not:

- **One list, read by a test.** `schema::ELEMENTS`, `THEME_ROLES`,
  `ENV_FIELDS`, the `abi_fn!` table (ADR 0020): a surface is one table,
  and a test pins every mirror of it. The examples' index
  (`examples/README.md`'s tables) is a mirror of `Cargo.toml` and
  `package.json` with nothing pinning it.
- **A script owns its round and prints it when not running it**
  (`build.sh --run`, `smoke-windows.ps1`). The Node and headless-Rust
  halves are lines in `ci.yml` instead.
- **A generated name is the name.** `docs/props.md` is generated from
  the schema; the widgets module names the stock widgets. An example
  called `gallery` for `image`, or `connectors` for `line`, or `toasts`
  for `enter`/`exit`, is the one place the repo names a thing twice.

## Decisions

### 1. An example has one subject, and the subject is one of three kinds

| Kind | Shows | May use | Directory |
|---|---|---|---|
| **widget** | one element or one `widgets::` helper, in every state it has | any feature the widget needs to be shown (a button needs hover; nothing else) | `widgets/` |
| **feature** | one cross-cutting behaviour, with exactly the widgets it touches | the stock widgets, as props not as subjects | `features/` |
| **app** | how it composes: an app owning its state, keymap or pane tree | everything | `apps/` |

A fourth directory, `tools/`, holds what is registered as an example
because cargo has no better slot for it and is not one: `conformance-dump`,
the C header walk, the benches.

The test for "one subject" is the header: its first sentence names the
subject, and there is no "N things to try" list. A feature example lists
the widgets it touches in that sentence — selection "over `text`, spans,
`cells` and `edit`" is one subject with four surfaces, which is what the
kind is for.

### 2. The file is named for the subject, with the name the repo already uses

A widget example is named for its `ELEMENTS` row or `widgets::` function;
a feature example for its `props.md` prop or its ADR's noun. So `gallery`
becomes `widgets/image`, `connectors` becomes `widgets/line`, `toasts`
becomes `features/enter_exit`, `rich_text` becomes `widgets/text` (spans,
decorations, wrap) and hands `selectable` to `features/selection`. Apps
keep their names — an app is named for what it is.

The cargo target name stays the file's basename: every example binary in
the workspace lands in one flat `target/debug/examples/`, so
`widgets/tooltip.rs` is `--example tooltip`, and the `c_`/`lua_` prefixes
keep doing what they do. Subdirectories cost nothing because every
`[[example]]` already carries an explicit `path`.

### 3. A binding carries the same taxonomy, and a subset chosen by one rule

The corpus already proves the four bindings lower the same tree to the
same quads, so a binding example that re-shows a core feature is a copy
of the Rust one with a different syntax. A binding gets an example when
one of three things is true:

- **it is `apps/counter`.** The counter is the one example every binding
  has — the Rosetta stone. Its contents are one list in
  `examples/README.md` (the Elm loop, a button, an `edit`, an
  `on_context_menu` → `modal` menu, `--headless` driving all of those and
  exiting non-zero on a wrong answer), and the four implement the list.
  Sounds leave it for `features/audio`.
- **the binding has its own door for the subject.** Node's `virtualColumn`
  and the `index` row, its pumped window loop, `ctx.addExtension`; C's
  `kui_ctx_add_extension` and the plugin entry points; Lua's `fill` and
  `env.add_extension`. The example shows the binding's spelling, not the
  feature again.
- **a round needs it.** `mindmap` exists because F15 was a driver bug
  only a Node window showed; it stays, as `features/slide` — its subject
  is a canvas of floats that eases everything or nothing.

Which gives: **Node** `apps/counter`, `features/window` (the runner:
pumped loop, images, sounds — was `counter-window`), `features/slide`
(was `mindmap`), `widgets/virtual_list`, `tools/bench`. **C**
`apps/counter` (the app and its headless round-trip, nothing else),
`features/slots/` (`panel.c`, `panel.rs`, `host.c` — one story, one
directory), `tools/surface.c` (the header walk, named for what it is),
`tools/conformance.c` (the adapter). **Lua** `features/slots/`
(`panel.lua`, `panel.rs`), `tools/bench.rs`. The `build.sh`/`build.ps1`
round builds all four C targets from a small shared `common.h` where the
counter, the walk and the adapter share a `check()`.

### 4. Every example is in a smoke channel, and enrolment is read, not written

Three channels, each discovered from something a script already parses:

| Channel | Enrolled by | Run by |
|---|---|---|
| **windowed** | being an `[[example]]` of `kui` — `cargo metadata`, as today | `smoke-windows.ps1` and a new `scripts/smoke-examples.sh` with the same contract (120 frames, exit 0, `kui: warning` lines reported), so the macOS half of the by-hand round is one command |
| **headless** | `[package.metadata.kui] headless = [...]` in the crate's `Cargo.toml`; `"smoke"` in `examples/node/package.json`; the round in `build.sh` | `check` in `ci.yml`, one step per binding, each a script that prints its round when not running it |
| **by hand** | the *By hand* column of `examples/README.md`, naming the check and its backlog id | the round before a tag; results into `### Native verification` as today |

`--headless` becomes a contract rather than a flag: an example that
accepts it drives itself through `Core` and **exits non-zero on a wrong
answer**. The harness (decision 6) parses the flag and calls
`Example::headless`; `virtual_list --headless` today prints and passes,
and under this rule it asserts the row range, the geometry and the
click, or it is not listed. `lua_panel` and `c_panel` gain the same
(`host.c` already has it), which is what puts Lua into a round for the
first time.

The windowed round runs every example **twice, once per base**
(`--light`, `--dark`), which the harness makes possible without any
example knowing. That is T3's "every example migrated onto the palette"
turned from a migration into a check: a literal colour that reads fine
on one base and not the other is opened on both, every run.

Two tests pin the mirrors, in the pattern ADR 0020 set: every
`[[example]]` in the workspace appears in `examples/README.md`, and every
name in a `headless` list is an `[[example]]` of that crate. A file moved
without its index line, or a list naming a binary that no longer exists,
fails `cargo test`.

### 5. The gaps are filled in the order the audit ranks them

Splitting and renaming adds no coverage; the new files do. In priority:

1. **Features only the C walk has.** `features/hover` (`hoverBg`,
   `hoverGroup`, `onHover`), `features/transition` (`transition`,
   `easing`, `slide`, `keyframes`, `repeat`, `delay`), and the
   decorations into `widgets/text`. These have corpus scenes and no
   picture.
2. **Widgets that are the side dish of nine.** `widgets/titlebar`
   (`titlebar`, `titlebar_with`, `window_buttons`, custom chrome on both
   platforms, the macOS traffic-light inset), `widgets/tooltip`,
   `widgets/button` (every state, `disabled`, the access rows F24 admits).
3. **Subjects split out of the five-things files.** `widgets/menu_bar`,
   `widgets/cells`, `widgets/context_menu` (stock over selectable, the
   app's own, the editor's), `features/selection`, `features/modal`,
   `features/audio`, `features/focus` (the Tab ring of ADR 0002, `focusBg`,
   `initialFocus` — today shown only inside `accessibility`).
4. **`features/drag`** (`onDrag`, today a side dish of `splitmux`,
   `popup` and `accessibility`).

`latency_hud` gets no example: it is in the harness's dock (decision 6),
which is its documentation. `ime` keeps its corpus scene only — there is
nothing to show without a real IME, and `accessibility` is where the
editor's text protocol is checked.

### 6. Every example runs inside one harness, and the harness owns the scaffolding

Fourteen examples call `latency_hud`, nine draw a titlebar, six parse
`--headless` by hand, every `main` spells its own `kui — <name>` title,
and exactly one — `theme` — can be flipped to the other base. All of that
is scaffolding, none of it is a subject, and each copy is a place a
convention can drift. It moves into `examples/harness` (now `examples/devtools`, see *What the
building changed*, 11), an unpublished
workspace crate that `kui`, `kui-ffi` and `kui-lua` take as a
dev-dependency, with a `Harness<A: Example>` that implements `App` and
wraps the example's.

**The dock.** The harness lays the frame out as the example's tree plus
a dock beside it — `Dock::Side` (a fixed-width column on the right) or
`Dock::Bottom` (a fixed-height strip), or `Dock::Off`. The dock holds:

- the **latency HUD** and the frame counter (`n / KUI_SMOKE_FRAMES` when
  one is set, so a smoke run is legible on screen);
- the **event stream**: every `UiEvent` the harness handed to `on_event`,
  newest at the bottom, each with the frame it arrived on and its
  `Value` printed as data — which is the whole thesis of the library
  made visible. `kui: warning` lines (`take_warnings`, with diagnostics
  on) land in the same stream, marked. A ring of the last 64; the wheel
  scrolls it; a chord clears it;
- the **status block**: what the runtime believes *now*, as opposed to
  what just happened. Every row is a fact an app can already read, each
  drawn from the door it comes from, so the block doubles as a live
  index of the readback surface: `env.system` (appearance, accent,
  motion, locale, each with its explicit unknown), `env.window`
  (custom chrome, maximized, fullscreen, native controls), viewport and
  scale, `refresh_hz`, `env.focused`; `Core::focus` + `focus_visible`
  (the focused node's label and whether the ring shows), `modifiers`,
  `native_menus`, `windows()` (every open window by name — `popup`'s
  second one appears and disappears here); and **audio** — device
  closed / opening / open, and how many playbacks are live. That last
  row is the one no app can read today (decision 6a);
- the **controls**: a row of stock buttons for what the harness can
  change, so a pointer does what the chords do — the theme **base**
  (follow the OS / light / dark) and **accent** (the OS's / kui's own /
  the four the OS might report / none), with which of the three
  `ThemeSource`s that amounts to shown beside them; **native menus**
  on/off (`Core::set_native_menus`, read back by `native_menus()`), so
  any example's context menu and menu bar can be seen as the platform's
  `NSMenu` and as the core's drawn one from the same item list — the
  check `theme.rs` had to opt out of native menus to make, available in
  every example; the **dock** position; and **clear** for the stream.
  They are the stock `widgets::button` on the palette, which is the one
  control every example then shows on both bases without trying;
- the **key legend**, from `Example::KEYS` (a `&[(&str, &str)]` the
  example declares instead of a `//! Keys:` paragraph nobody can see
  while the window is up).

The controls are controls, and two rules follow. They declare
`focusable(false)`, so the Tab ring stays the example's — `features/focus`
and `accessibility` would otherwise be showing the harness's buttons in
their ring — and the chords remain the keyboard path to the same
handler, which is also what `--dock off` leaves. And they sit outside
any `modal` the example opens, so while one is up the pointer cannot
reach them (ADR 0003's boundary), which is correct and, in passing, a
demonstration.

A dock rather than a float: a float covers what it reports on, and a
stream needs a column. The context menu stays the frame's topmost float
exactly as `Ui::finish` draws it now; the dock is ordinary layout under
it. The dock is theme-drawn, so it is also the one control every
example shows on both bases without trying.

**The chords.** The harness reserves `Ctrl+Shift+<letter>` on every
platform — a family no example keymap uses (`splitmux` chords on Alt and
rejects Ctrl; `modal_editor` and `syntax_view` read bare keys): `T` cycles
the base (follow the OS → light → dark), `A` the accent (the OS's →
kui's own → the four the OS might report → none), `M` toggles native
menus, `D` the dock (side → bottom → off), `C` clears the stream — the
same handler the dock's buttons call. The harness declares a root
`on_key` sink around the example's tree; ADR 0011 bubbles unclaimed
keys out to it, and for an example that claims its whole keyboard the
harness intercepts its chords in `on_event` before delegating, so the
family works whether or not the example listens.

**The CLI.** One parser, in the harness: `--headless`, `--dock
side|bottom|off`, `--light` / `--dark` / `--accent #rrggbb`, `--size
WxH`. The window title is `kui — $CARGO_BIN_NAME`. `--headless` calls
`Example::headless(&mut self, &mut Core) -> Result<(), String>`; the
default returns `Err("no headless drive")` and the harness exits 2, so
"listed in `headless = [...]` but never wrote one" is a failure with a
message rather than a pass. What a headless drive prints is the same
event stream the dock shows, which is what its assertions read.

**What the harness may not do.** It is not a subject and shows no
feature of its own: no `modal`, no sounds, no animation on the dock.
Its buttons are the only nodes it adds with a role, and they are why
`accessibility` runs under `--dock off` in the audit: `ax-audit.swift`'s
106 checks walk exactly the window they walk today, and a dock with
five buttons in the AX tree would be a different fixture. It claims no
key outside its family.

**The rule it imposes.** Because the dock is a sibling, the example's
root is no longer the tree's root, and `Key::ROOT.str("log")`
(`virtual_list` today) stops naming the log. An example addresses its
nodes from the key its `open` returned, never from `Key::ROOT` — the
same discipline an extension in a slot already lives under (ADR 0014),
and the reason a harnessed example can be dropped into one unchanged.
An example that wants a specific window size (`popup` is 360×150 on
purpose) declares it through the harness, which adds the dock's own
extent to the request, so what the example sees is what it asked for.

**Per binding.** The contract is the flags, the chords and the dock's
contents; the implementation is per binding and small — `harness.tsx`
for Node, `harness.h`/`harness.c` for C — since each is a wrapper over
that binding's `App` shape and a ring of `Value`s. Lua scripts get
nothing: their host is a Rust example and already has it.

`theme.rs` keeps its swatch page and loses its key handling and its
`set_native_menus(false)`, both of which were the harness's job done in
one file. The chords are the same ones, relettered.

### 6a. One door for the status block: `env.audio`

`Audio::holds_device()` and `Audio::active()` exist
(`crates/kui/src/audio.rs`) and answer exactly the status question — but
on the runner's private `Audio`, where the driver asks them to decide
when to let the device go. No app can. And it is the row that matters
most: an open output stream is a real-time thread at ~94 callbacks a
second whether or not anything plays, which is the whole of an idle
app's CPU once a session has held a sound (`kui-idle-cpu`). An example
that shows the device still open ten seconds after its last blip is
showing a bug, and today the only way to see it is `top`.

The driver already writes host facts into `Env` every frame
(`focused`, `system.*`, `window.*`); it writes one more.
`Env.audio: AudioEnv { device: Closed | Opening | Open, live: u32 }`,
set from the two readers the driver holds. Headless drivers leave it at
`Closed`/0, like every other default in `Env`. It is a row in
`schema::ENV_FIELDS`, so the four bindings pin it the way S7 pins the
rest (Lua `env.audio.device`, Node `env.audio`, C a `kui_env_set_audio`
setter beside `kui_env_set_system` — additive, pinned by `abi_fn!`, so
ABI stays where it is), and the corpus reads it back with the others. It is a
*fact*, not a verb: nothing here lets an app close the device, which
stays the driver's decision for the reasons `close()` documents.

Everything else the block shows is readable now and needs no door.

## What the building changed

Everything above was built as written, with these exceptions and
additions, each found by running it:

1. **The dock hides by `role = none`, not `focusable(false)`.** There is
   no opt-out from the ring — `focusable` is opt-in only — and
   `Role::None` was already the rule that takes a subtree out of the ring
   *and* the access tree. So the whole dock declares it, and its five
   buttons are neither Tab stops nor AX nodes; `--dock off` for the audit
   is belt and braces, and the audit passes 106/106 either way.
2. **The root is never a Tab stop.** The harness's sink is the root (it
   encloses everything, so ADR 0011 bubbles every unclaimed key to it
   whatever the example focused), and a root sink was in the ring and
   drew the ring around the window. A rule in `focus_ring` now skips
   index 0; `tests/focus.rs` pins it. No test or scene depended on Tab
   reaching a root.
3. **Two readback doors the dock needed and no app had:** `Core::label_of`
   (the inverse of `key_of`, through the same `LabelIndex`), so a key
   from an event or from `focus()` reads back as the name the view gave
   it; and `Core::warnings_raised`, the log `take_warnings` leaves behind,
   because the runner drains and prints after every frame and a view
   would otherwise never see one.
4. **`env.audio` is pinned by the Lua and Node key-set tests and kui-ffi's
   setter parity**, as the rest of `ENV_FIELDS` is — not by the corpus,
   which reads only `WindowEnv` back. Decision 6a said "the corpus"; that
   was the wrong pin named, and the right ones were already there.
5. **The controls are small buttons beside the fact they change**, in the
   status rows, rather than a row of stock buttons restating the values —
   `base`, `accent`, `menus`, `dock`, and `clear` on the stream's header.
   The chords stay as the keyboard path to the same handler.
6. **Two hooks the trait needed:** `Example::extensions` (the two panel
   hosts load a plugin the launcher must carry) and `Example::native_menus`
   (`theme`, `context_menu` and `menu_bar` ask for the drawn menu to start
   with, which `theme.rs` used to do by hand). And a `Drive` — the
   scaffolding a headless drive is built from (frames, clicks by key or
   point, keys through both channels, a `rect_of` off the hit list) — so
   a drive is a list of checks and not fifty lines of setup.
7. **The Node addon has no dev build to honour `KUI_SMOKE_FRAMES`:**
   `native.cjs` loads the release cdylib first, so the Node windows never
   closed under the round. `kui-node` gained a `smoke` feature forwarding
   to `kui/smoke`, `scripts/smoke-examples.sh --node` builds with it and
   smokes the four Node windows, and both scripts open every example on
   both bases. `--smoke` timeouts in the Node examples are gone.
8. **C got `common.h`, not a dock.** The C examples are one app and three
   tools; the counter's `--headless` is the Rosetta drive and the header
   walk is a test, and neither wants a dock. A C twin of the dock is
   *Not done here*.
9. **Names the table did not have:** `features/audio` took the old
   counter's sounds; `tools/types.tsx` is where the Node `.d.ts` coverage
   went (the old headless counter's second half); `features/window.tsx`
   is the Node runner's own example.
10. **Phase 3 built whole**, in the order given: `transition` (with the
    `keyframes` / `repeat` / `delay` chase the C walk alone had), `hover`
    (with `hover_group`), `focus`, `drag`, `titlebar` (with
    `window_buttons`), `tooltip`, `button` — each with a drive, seven
    more names in `headless = [...]`.

11. **A round of the by-hand smoke on the built thing** (2026-09-10)
    renamed and reshaped the harness. The crate is `examples/devtools`
    (`kui-devtools`, `kui_devtools::Example`, `devtools.tsx`): it had
    grown a tree view, so "harness" undersold it. The dock's header is
    the example's name, the frame counter and an **icon strip** in the
    top-right — base ◐/☀/☾, accent ●, menus ☰, dock ▐/▄/✕ — each icon's
    tooltip saying what it is set to and which chord does the same; the
    small buttons beside the status rows (5) are gone. Under the header,
    **three tabs** (`Ctrl+Shift+N` cycles them): *facts* is the latency
    graph, the status block and the key legend; *events* the stream;
    *tree* the last frame's nodes, indented, with its label, role and
    flags, and a click on one draws the node's rect on the example and
    opens an **inspector** below — key, kind, label, role, rect, sizing,
    direction, background, flags, text. That tab is what
    `Core::set_inspect` / `Core::nodes()` exist for: a per-frame
    `NodeInfo` snapshot taken at the end of `finish_frame`, off unless a
    tool asked (the copy is O(nodes)), with the *derived* role — the
    access tree's reading, so a box with a click says `button`. It is a
    `KuiWindow.nodes()` / `setInspect()` / `warningsRaised()` in Node,
    which gives the Node dock the warnings line *Not done here* had
    declined. Six things the round found and fixed on the way:
    - the runner presented the platform's context menu *and* drew the
      core's — `pump_native_menu` now asks `core.native_menus()` first —
      and it kept the macOS bar up over a core told to draw its own, so
      `pump_menu_bar` hands AppKit an empty bar for that core (the
      devtools' menus toggle sets both `set_native_menus` and
      `set_native_menu_bar`, which is how the drawn bar is seen on macOS
      at all);
    - the harness's root sink kept Tab (a focused sink keeps every key)
      and swallowed the example's menu-bar choices (root-keyed events
      were all read as its own): it now owns only events tagged with its
      sink and `modifiers` on the root, and forwards a Tab it heard to
      `focus_next` / `focus_prev` on the next view;
    - a `dispatch` the Node loop did not make itself — an effect
      handler's, a timer's — changed the model and drew nothing until the
      next OS event; `step()` now draws on it (`dirty`), which ADR 0013's
      "lands in the next turn" had implied and never pinned;
    - `apps/counter` had an `edit` field (that is `widgets/edit`'s
      subject); `widgets/image` still carried a list; `features/drag`'s
      card was clamped only at the top-left; `enter_exit`'s toasts floated
      in the window rather than the example's viewport; `widgets/text`'s
      mono run overflowed its card; `titlebar` and `virtual_list` posted
      payloads without a `kind`; `⇧` had no glyph in the UI font
      (spelled "Shift" now); `transition` gained the `Bouncy` lane.

Two things the drives corrected in the *prose* of this ADR: `on_hover` is
per node — a `hover_group` lights together, it does not report together —
and a `drag` event's `dx`/`dy` are the displacement since the press, not
the step since the last event, which is what "no delta is ever summed"
in the Node mindmap always meant.

## Considered options

### Option A: keep the tree, fix the index

Re-sort `examples/README.md`'s tables by topic and add a "shows" column
per subject.

| Dimension | Assessment |
|---|---|
| Complexity | Low — one markdown file |
| Coverage gained | None |
| Smoke risk | None |
| Drift | The index already drifts from the manifests; a richer index drifts more |

**Pros:** no renames, no CHANGELOG churn, `-Only` names stable.
**Cons:** `context_menu` still shows five things; `titlebar` still has
no example; the C counter is still three programs; a reader still learns
the taxonomy from the README rather than from the tree. It answers the
complaint about the *index*, not the one about the examples.

### Option B: one subject per file, three kinds, a subset rule for bindings (chosen)

What the decisions above say.

| Dimension | Assessment |
|---|---|
| Complexity | Medium — ~15 renames, 3 splits, ~13 new files, a harness crate (+ a Node and a C twin), one `Env` field, two pin tests, one script |
| Coverage gained | Four features and three widgets get a picture; three bindings' examples become self-checking; Lua enters a round |
| Smoke risk | Low, once enrolment is read from the manifests — the pin tests exist to make a dropped file a red test |
| Drift | The taxonomy is the directory; the index is pinned |

**Pros:** the tree answers "where is X" without a README; the smoke role
is declared per example and cannot be forgotten; the C counter stops
lying about its size; every example gets the event stream, the status
block and both bases for free.
**Cons:** every name in the by-hand round, the CHANGELOG's verification
sections, `smoke-windows.ps1 -Only` and the memory notes changes once;
thirteen new files are real work, and each is a windowed target that
adds to the Windows round's ~1.5 s apiece.

### Option C: one showcase app per binding, a page per subject

A single `showcase` binary with a sidebar — `storybook`'s shape — and the
Node and C equivalents.

| Dimension | Assessment |
|---|---|
| Complexity | High — a navigation shell, a page registry, and a page per subject anyway |
| Coverage gained | The same pictures, one process |
| Smoke risk | High — `KUI_SMOKE_FRAMES` counts the main window's frames, so one process is one check; a crash on page 12 is not reached by a 120-frame round that never leaves page 1 |
| Drift | A page is not a file a reader can copy whole |

**Pros:** one window to open, one binary to build, the theme page's
"every widget under every role" comes free.
**Cons:** loses the property the examples have now — each one is a
complete program a user copies — and the two by-hand fixtures that need a
specific window (`ax-audit.swift` walks *the* accessibility window;
`popup` needs its own 360×150 frame). `theme.rs` is already the page that
wants this shape, and it stays one file.

### Option D: per-binding parity — every subject in every binding

Rejected outright. It multiplies ~30 subjects by four, and the corpus is
the thing that already proves the bindings agree; an example is for the
binding's own doors (decision 3).

## Consequences

- **Where is X** is answered by `ls examples/rust/widgets
  examples/rust/features`. The README table stays as the run column, one
  line per file, one subject per line — and a test says every target is
  in it.
- **Four features get their first Rust example** and three widgets their
  first anywhere; `counter.c` becomes ~400 lines and the header walk is
  named as the test it is.
- **A moved example cannot fall out of a round**: windowed enrolment was
  already read from `cargo metadata`; headless and by-hand enrolment now
  are too. The macOS by-hand round gains the same 120-frame script
  Windows has, so "every host opened a window" in a `Native verification`
  section becomes one command's output.
- **Lua is in a round.** `lua_panel --headless` asserts the slot, the
  click, the reply — and, when `panel.so` is built, the three-languages
  load — and `check` runs it.
- **Names churn once.** `-Only fragments,toasts` becomes `-Only
  fragments,enter_exit`; the CHANGELOG's next `Native verification`
  section uses the new names and says so; the memory notes that name
  `toasts`, `gallery`, `connectors`, `mindmap` and `counter --headless`
  are updated with the ADR.
- **The Windows round grows** by roughly thirteen targets and then
  doubles for the two bases — from 14 runs to about 54, roughly 80 s at
  the measured ~1.5 s apiece, and thirteen more link steps under
  `CARGO_BUILD_JOBS=2`. Acceptable; the round is `continue-on-error` and
  on no release path, and `-Only` still narrows it by hand.
- **Every example is a debugging tool.** The dock shows the events as
  the data they are, the runtime's facts as the readback doors they come
  from, and the warnings a frame raised — on screen, not on stderr. A
  field report ("the click never arrived", "the device stays open")
  becomes "open the example, read the dock".
- **One example's scaffolding is every example's.** Fourteen HUD calls,
  nine titlebars, six `--headless` parsers and one set of theme keys
  become one crate; an example file is its subject and a `main` of one
  line. The cost is that an example is no longer a self-contained
  program a reader copies whole — it copies the example *and* the
  harness's `run`, which the README says in one sentence.
- **One new host fact, `env.audio`**, across four bindings and the
  corpus readback — the price of the one status row no app could read.
- **Two more pin tests in `cargo test`**, both reading files — the
  pattern ADR 0020's header audit set, and as cheap.
- **What to revisit:** whether `apps/` wants a fourth member showing
  multi-window composition once ADR 0004's remaining steps land; whether
  the Node subset rule should admit `features/theme` when Node gains a
  theme door of its own (T-series); and whether the by-hand `By hand:`
  line should become a `[package.metadata.kui] by_hand` list once there
  are enough of them that a script wants to print the round.

## Action items

Phase 0 — the harness, so that every move in phase 1 lands in it:

0. [x] `examples/harness` crate: `Harness<A: Example>`, the dock (side /
   bottom / off) with HUD, event stream, status block, the controls
   row (base, accent, native menus, dock, clear — `focusable(false)`)
   and key legend; the `Ctrl+Shift` chords; the CLI; `Example::headless` and
   `Example::KEYS`. `env.audio` in `Env`, `ENV_FIELDS`, the driver, the
   four bindings and the corpus readback (6a). `harness.tsx` and
   `harness.c` with the same flags and chords. `theme.rs` drops its keys.

Phase 1 — move and split, no new coverage, every round green after each
step:

1. [x] Create `examples/rust/{apps,widgets,features,tools}`; move the 18
   files per decisions 1–2; update every `[[example]]` path; keep target
   names.
2. [x] Split `context_menu.rs` into `widgets/context_menu`,
   `widgets/menu_bar`, `widgets/cells`, `features/selection`; split
   `counter.rs`'s sounds into `features/audio` and its menu into
   `features/modal`; move `selectable` from `widgets/text` into
   `features/selection`.
3. [x] Split `examples/c/counter.c` into `apps/counter.c`,
   `tools/surface.c`, `tools/conformance.c` over a `common.h`; move the
   panel trio to `features/slots/`; update `build.sh`/`build.ps1` and the
   two `ci.yml` lines that name `counter --headless` / `--conformance`.
4. [x] Rename Node's four to `apps/counter`, `features/window`,
   `features/slide`, `widgets/virtual_list`; move `bench.mjs` to
   `tools/`; `package.json`'s build script globs the three directories.
5. [x] Move Lua's to `features/slots/` and `tools/bench.rs`.
6. [x] Rewrite `examples/README.md`: the four tables become one per kind
   per binding, one line each, with a `Smoke` column naming the channel.
7. [x] Update `smoke-windows.ps1`'s header examples, the README's release
   section, the `kui-review-rounds` / `kui-release-pipeline` /
   `kui-architecture` memory notes, and the next CHANGELOG heading.

Phase 2 — the smoke contract:

8. [x] `--headless` asserts, exits non-zero: `virtual_list`, `lua_panel`,
   `c_panel`. Add `[package.metadata.kui] headless = [...]` to `kui`,
   `kui-lua`, `kui-ffi`; `"smoke"` script to `examples/node/package.json`.
9. [x] `scripts/smoke-examples.sh` (unix twin of `smoke-windows.ps1`,
   same flags, same contract), both rounds running each example under
   `--light` and `--dark`; and a `scripts/smoke-headless.sh` that reads
   the metadata lists and prints its round when not running it; `check`
   calls the second. *Both scripts were replaced on 2026-09-11 (backlog
   AR4) by one program, `cargo run -p kui-devtools --bin smoke --
   --headless` / `--windowed`, which reads the same rosters the pin tests
   read; the contract is the scripts', the shell is gone.*
10. [x] The two pin tests: every workspace `[[example]]` is in
    `examples/README.md`; every `headless` name is an `[[example]]`.
11. [x] The by-hand checks named per example — as the *By hand* column of
    `examples/README.md` rather than a header line, since the index is
    what the pin test reads and where the round is read from.

Phase 3 — the gaps, in decision 5's order:

12. [x] `features/hover`, `features/transition`, decorations into
    `widgets/text`.
13. [x] `widgets/titlebar`, `widgets/tooltip`, `widgets/button`.
14. [x] `features/focus`, `features/drag`.

### Not done here

- No storybook shell (option C); `theme.rs` is the one page that wants
  it and already is one. The harness is not one either: it wraps one
  example per process, which is what keeps `KUI_SMOKE_FRAMES` and the
  AX audit meaning what they mean.
- No verb on `env.audio`: the harness shows the device's state and
  cannot close it. If a status row ever wants a button, that is a
  different ADR.
- No C dock: `common.h` is the whole of the C harness (see *What the
  building changed*, 8).
- No popup window for the tree view: it is a tab in the dock, which
  is a column already and shares the example's frame, so the outline
  it draws on a selected node is ordinary layout in the same tree. A
  second window would need the snapshot to cross windows; the day an
  inspector wants to be beside a full-screen example, that is the
  reason to build it.
- `features/modal` — the dialog with `initial_focus`, Tab confined,
  `dismiss` — is still shown inside `apps/counter` and `features/focus`
  rather than as its own page; the counter's menu is the Rosetta copy of
  it, and a second page would be that menu again with a title.
- No Lua app example — there is no Lua window to put one in, and that is
  ADR 0014's decision, not this one's.
- No change to what a windowed smoke judges: it still judges that drawing
  did not fail, and the corpus still judges the pixels.
- The `macos`/`windows` runners stay unregistered (P8); this ADR gives the
  macOS round a script to run the day one exists, and nothing more.
