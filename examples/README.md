# Examples

Every example in the repo lives here, one directory per binding, and
inside a binding one directory per **kind** of example
([ADR 0021](../docs/adr/0021-one-subject-per-example.md)):

| | |
|---|---|
| `apps/` | how it composes — an app owning its state, keymap or pane tree, touching whatever it needs |
| `widgets/` | one element or one stock widget, in every state it has |
| `features/` | one cross-cutting behaviour, with exactly the widgets it touches |
| `tools/` | registered as an example for want of a better slot, and not one: a corpus dump, a header walk, a bench |
| `tutorial/` | the book's steps ([ADR 0039](../docs/adr/0039-a-tutorial-is-a-sequence.md)): a whole program each, every one the last plus one concept, read in order by [`docs/book`](../docs/book) — and the one kind that runs on the shipped launcher with no harness around it |

An example has **one subject**, and the file is named for it with the
name the repo already uses — the `ELEMENTS` row, the `widgets::` function,
the `props.md` prop or the ADR's noun. The target name is the file's
basename (every example binary in the workspace lands in one flat
`target/debug/examples/`, so `widgets/tooltip.rs` is `--example tooltip`);
the `c_` and `lua_` prefixes on the two panel hosts keep them from
colliding.

The Rust files belong to four different crates but share this one tree,
so each crate's `Cargo.toml` names its examples with an explicit `path`,
and `cargo run` needs the right `-p` — the tables below have it. The
published crates do not carry their examples in the tarball (cargo drops
a target whose source sits outside the package); read them here.

## The devtools

Every example runs inside [`devtools/`](devtools) (Rust; `node/devtools.tsx`
is its twin for Node, and `c/common.h` what the C programs share). It owns
what is not the subject: the window title and the command line — and it
opens the **core's devtools panel** around the example
([ADR 0024](../docs/adr/0024-the-devtools-are-the-cores.md)), the same
panel any app gets from `Core::set_devtools(true)`, `win.setDevtools(true)`,
`kui_set_devtools` or `KUI_DEVTOOLS=1`. Its header is the window's title,
the frame counter (`n / KUI_SMOKE_FRAMES` when one is set) and an icon
strip with a button per placement — a little window with its left, right
or bottom pane shaded, two windows for undock, a cross for close — the
current one lit; the tab row ends in the app-state toggles — a half disc,
a sun or a crescent for the base, a dot in the accent, three bars for the
menus — each with a tooltip saying what it is set to. The icons are drawn
by the core from its own `line` and `polygon` elements, not glyphs from
a fallback font, so they are the same on every platform. A docked pane's inner edge is a handle that resizes it, and the
example's viewport is what the pane leaves (a change is a `resize`).
Under it, three tabs:

- **facts**: the latency graph; the **status block** — what the runtime
  believes right now, each row read from the door it comes from:
  `env.system`, the theme and its source, the window and every open one,
  the viewport and refresh rate, the focused node, the modifiers,
  `env.audio`, the node count; and the **key legend** the example declares;
- **events**: every `UiEvent` handed to the example from any window, as a
  virtual list — the frame it arrived on, the node by its label, the
  payload as data — each row opening into the payload as an indented
  tree; a filter, **pause**, **follow** (a wheel up turns it off) and
  **clear**; plus every `kui: warning` the core raised;
- **tree**: the last frame's nodes (`Core::set_inspect` / `nodes()`,
  `win.nodes()` in Node), collapsible — a disclosure per row, `+N` on a
  folded one, fold/unfold all — and filterable by label, kind, role,
  text or flag (a match with its ancestors dimmed). The **picker** (the crosshair `pick` button,
  `Ctrl+Shift+P`) outlines and names the node under the pointer over the
  example; a press selects it and `Escape` leaves. Click a row: its rect
  is outlined on the example and an **inspector** opens — the node (key,
  label, kind, role, origin, text, parent, children, layer), its box
  (rect, sizing, min/max, scroll), layout (direction, padding, gap,
  alignment), paint (background, border, radius, opacity), every handler
  with its payload, and its state now — with the parent and every
  ancestor in the breadcrumb a click away.

The same flags everywhere: `--headless` runs the example's self-check and
exits non-zero on a wrong answer; `--dock left|right|bottom|window|off`
places the panel (`side` still means the right) — `window` opens it in a
window of its own; `--light` / `--dark`
pin the theme base and `--accent #rrggbb` the accent; `--motion
full|reduced` pins what `env.system.motion` reads over the OS's setting,
which is how an animation's reduced-motion branch is looked at on a
machine whose owner did not ask for it; `--size WxH` the example's area;
`--key CHORD` respells the chord into the dock (`f12`, `mod+shift+d`) for
an example whose own keymap wants `Ctrl+Shift+I`. The chords are the core's, `Ctrl+Shift+<letter>` on every
platform: `T` cycles the base (the app's own → light → dark), `A` the
accent, `M` toggles native menus (the popups *and* the bar — on macOS
that is how the drawn bar is seen), `D` moves the panel (left → right →
bottom → window → off), `N` the tab, `C` clears the stream, `I` moves the keyboard
into the panel and back, `P` picks.

The panel is a focus region (ADR 0022): the example's Tab ring never
enters it, and inside it Tab walks the panel's own controls. While it is
docked the core wraps the example's root (ADR 0024, decision 2) — the
example's keys do not move for it, and its `configure_root` still lands
where it did. `--dock off` draws nothing and keeps the chords live, which
is what the accessibility audit runs under.

## The smoke rounds

The examples are the repo's only windowed check and half of its by-hand
round, and each is in one of three channels, enrolled from something a
script reads rather than a list somebody keeps:

| Channel | Enrolled by | Run by |
|---|---|---|
| **windowed** | being an `[[example]]` of `kui-native` | `cargo run -p kui-devtools --bin smoke` ([`devtools/src/bin/smoke.rs`](devtools/src/bin/smoke.rs)), on every platform: 120 frames under `KUI_SMOKE_FRAMES`, on both bases; `--node` adds the Node windows |
| **headless** | `[package.metadata.kui] headless = [...]` in the crate's `Cargo.toml`; `npm run smoke` for Node; the round in `cbuild` | `cargo run -p kui-devtools --bin smoke -- --headless`, which CI runs |
| **by hand** | the *By hand* column below | the round before a tag; results into `### Native verification` in the CHANGELOG |

Two tests in the devtools crate pin the mirrors: every `[[example]]` is linked
from this file, and every `headless` name is an example — read through the
same [`manifest`](devtools/src/manifest.rs) reader the `smoke` binary runs
from, so the round and its pins cannot drift.

## Rust — [`rust/`](rust)

Run with `cargo run -p kui-native --example <name>`; `-- --headless` where the
table says so.

### `tutorial/`

The book's ten steps, in the order the book reads them (`cargo run -p
kui-native --example tutorial_NN_<name>`; build the book with `mdbook build
docs/book`). No harness, no `--headless`: each is the program a chapter
ends with. Step 10 carries a `mod tests` that `cargo test -p kui-native
--example tutorial_10_testing` runs.

| Step | Adds | Chapter |
|---|---|---|
| [`01_hello.rs`](rust/tutorial/01_hello.rs) | An `App` with a `view`, one text node, the launcher | [Hello, window](../docs/book/src/hello.md) |
| [`02_layout.rs`](rust/tutorial/02_layout.rs) | Rows and columns; fit, fixed and grow; padding, gap, alignment | [Layout](../docs/book/src/layout.md) |
| [`03_counter.rs`](rust/tutorial/03_counter.rs) | A model, `#[derive(Message)]`, a button's message, `on_event` | [State and messages](../docs/book/src/messages.md) |
| [`04_controls.rs`](rust/tutorial/04_controls.rs) | The stock switch, checkbox, slider, text field and select, each drawn from the model | [Controls](../docs/book/src/controls.md) |
| [`05_list.rs`](rust/tutorial/05_list.rs) | Rows from a `Vec` under keys of their own, a box that scrolls, a filter | [Lists and keys](../docs/book/src/lists.md) |
| [`06_keyboard.rs`](rust/tutorial/06_keyboard.rs) | A key sink, `key_press`, `focusable`, Tab, where focus is | [Keyboard and focus](../docs/book/src/keyboard.md) |
| [`07_floats.rs`](rust/tutorial/07_floats.rs) | A `tooltip`, a `modal` float, `dismiss` | [Floats and modals](../docs/book/src/floats.md) |
| [`08_motion.rs`](rust/tutorial/08_motion.rs) | `transition`, `easing`, `enter` and `exit` on keyed rows | [Motion](../docs/book/src/motion.md) |
| [`09_effects.rs`](rust/tutorial/09_effects.rs) | `on_event_with` and the clipboard, a thread and the `Waker`, the window's title and close | [Effects, the clock and the world](../docs/book/src/effects.md) |
| [`10_testing.rs`](rust/tutorial/10_testing.rs) | A `mod tests` on `testing::Drive`: clicks by label, a key, the frame read back | [Testing without a window](../docs/book/src/testing.md) |

### `apps/`

| Example | Shows | Headless | By hand |
|---|---|---|---|
| [`counter.rs`](rust/apps/counter.rs) | The smallest app that is the whole pattern, and the one every binding has in the same shape: the Elm loop, a button, a right-click that declares a `modal` menu — its messages a `#[derive(Message)]` enum, as the book's step 3 spells them | ✓ the Rosetta drive | |
| [`splitmux.rs`](rust/apps/splitmux.rs) | tmux-style splits, tabs, focus, ⌘-drag pane moves; the pane tree is data, the app owns the chord keymap, and its messages are a `#[derive(Message)]` enum | ✓ the chords, the clicks and a divider drag, and `cargo test` (its `mod tests`) | ⌘-drag a pane |
| [`modal_editor.rs`](rust/apps/modal_editor.rs) | Helix-flavored modal editing; the app owns the document, the keymap, the modes, the mouse (a press carries `line`/`byte`/`clicks`) and the clipboard (`y`/`p`), and its caret blinks on `caret_visible` | ✓ `jjj ww v lll`, `dd` + `p`, `:help`, a click and a double click, the off phase | `pbpaste` after `y` |
| [`syntax_view.rs`](rust/apps/syntax_view.rs) | Syntax highlighting as coalesced style runs; the frame shape the `highlight` bench measures | ✓ `j`, `G`, `k`, `tab` | |

### `widgets/`

| Example | Shows | Headless | By hand |
|---|---|---|---|
| [`button.rs`](rust/widgets/button.rs) | The stock button in every state: rest, hover, pressed, the ring, `accent`, `disabled`; the access rows; a button in the app's own colour off `button_palette` | ✓ | |
| [`edit.rs`](rust/widgets/edit.rs) | The `edit` element: a multiline document and the single-line `text_input`, `changed` and `submit`, the text read back | ✓ | |
| [`text.rs`](rust/widgets/text.rs) | The `text` element: spans shaped as one paragraph, decorations, families, `nowrap`, `max_lines` + `ellipsis`, line height | | |
| [`image.rs`](rust/widgets/image.rs) | The `image` element: Fit sizing, kept aspect, rounded corners; a stream replaced every frame at the size `layout.scale` says (`update_image`), `nearest` beside `linear`, `contain` / `cover` (ADR 0025) | ✓ | |
| [`line.rs`](rust/widgets/line.rs) | The `line` element: a mind map whose links are curves between floats, brightening by transition | | |
| [`polygon.rs`](rust/widgets/polygon.rs) | The `polygon` element: a pie whose wedges light under a hover box, arrowheads on a graph's links, the area under a sparkline, a concave star | ✓ | |
| [`fragment.rs`](rust/widgets/fragment.rs) | The `fragment` element: boxes a WGSL function paints — a gradient, a ring, a shimmer, a card with children; a heatmap reading a data texture the app replaces every frame and a ripple over an atlas-backed icon, the `image` input (V1) | ✓ | |
| [`cells.rs`](rust/widgets/cells.rs) | The `cells` element: a terminal grid with a cursor and an `origin_line`, selecting in cells, copy trimming blanks, the screen scrolled under a selection, a box-drawn table drawn from the cell box | ✓ | |
| [`virtual_list.rs`](rust/widgets/virtual_list.rs) | `widgets::uniform_list`, the same list by hand (`--by-hand`), and `widgets::list` for rows of no fixed height (`--variable`) | ✓ every mode | |
| [`context_menu.rs`](rust/widgets/context_menu.rs) | Who gets a context menu: the stock one over a selectable scope, the app's own over a row, the editor's four, nothing over a plain box | ✓ | |
| [`controls.rs`](rust/widgets/controls.rs) | The stock controls (ADR 0034): a switch, checkboxes under a select-all that goes mixed, a radio group whose arrows move the choice, and two sliders whose `change` events the app stores — one in steps of 5, one in tenths with a `value_text` | ✓ the mixed box, the switch disabling the rows, the arrows, a press, Right, PageDown, End and a tenth down | drag a slider; Tab through them |
| [`select.rs`](rust/widgets/select.rs) | `widgets::select` over labels and `select_items` over `MenuItem`s: the field opens the core's own menu under itself, a choice is one `menu` event on the field, the app holds no open state | ✓ | |
| [`table.rs`](rust/widgets/table.rs) | The table (ADR 0033): `NodeSpec::table()` is a column whose rows' cells line up, each column as wide as its widest cell and the `grow` one taking the rest — no width picked by hand, nothing measured; the rows are clickable rows with a hover wash and a selected fill, a number right-aligned in its column, the header's cells sorting by the column pressed | ✓ the alignment, a row's click, a header's sort | |
| [`menu_bar.rs`](rust/widgets/menu_bar.rs) | The application menu bar the frame declares (ADR 0018): a `checked` row, an `enabled` one, and the same `menu` event whoever showed it | ✓ | |
| [`tooltip.rs`](rust/widgets/tooltip.rs) | The tooltip three ways: the view's float under `is_hovered`, the `tooltip` prop that is also the accessible description, `tooltip_with` around a legend; `fit` near the edge | ✓ | |
| [`titlebar.rs`](rust/widgets/titlebar.rs) | Custom chrome: `titlebar`, `titlebar_with` (tabs in the strip), `window_buttons`, the inset past the OS's own controls, the window facts read back | | drag the strip, double-click it |

### `features/`

| Example | Shows | Headless | By hand |
|---|---|---|---|
| [`hover.rs`](rust/features/hover.rs) | Hover declared, not tracked: `hover_bg`, `hoverable` + `is_hovered`, `hover_group` lighting siblings together, `on_hover` as enter/leave events | ✓ | |
| [`drop.rs`](rust/features/drop.rs) | A drop zone (ADR 0031): `on_drop` as `enter`/`move`/`leave`/`drop` events with the paths and the point, `drop_bg` lighting the zone, a button inside it being the zone's, the banner shown on `enter` looked past, a box that is no zone refusing the release; "Open…" asking for the platform's Open dialog, whose `files` answer lands in the same list | ✓ | drag a file from the Finder: the badge over the zone, none off it, the icon sliding home; Open… shows the sheet |
| [`focus.rs`](rust/features/focus.rs) | Keyboard focus as data (ADR 0002): the Tab ring in tree order, who is in it and who is not, Enter/Space, the ring vs a click, `focus_bg`, the `focus` / `blur` / `focus_next` verbs | ✓ | |
| [`drag.rs`](rust/features/drag.rs) | `on_drag`: start/move/end with the displacement since the press — a slider by travel (drawn by hand: the drag is the subject; an app's is `widgets::slider`), a card moved by its float offset, the pointer captured until the release | ✓ | |
| [`transition.rs`](rust/features/transition.rs) | Motion as data: `transition`, every `easing` racing, `slide` between anchors, `keyframes` with `repeat` and `delay` as a chase light | ✓ | |
| [`selection.rs`](rust/features/selection.rs) | Selection as a scope (ADR 0017) over `text`, spans, `cells` and `edit`: one selection per window, read back | ✓ | force-click a word for Look Up |
| [`clipboard.rs`](rust/features/clipboard.rs) | The clipboard: every way onto the host's one queue — the runner's chords in an `edit`, a `selectable` scope's Copy with the HTML beside, a virtual list's `selectionrange` ask answered from the app's rows, a key sink's own `y`/`p` through `set_clipboard` / `request_paste` — and a paste landing as typing or as the sink's `text` event | ✓ every path | ⌘C then `pbpaste`; `pbcopy` then `p` |
| [`audio.rs`](rust/features/audio.rs) | Sound as data: `click_sound`, `hover_sound`, a looped `audio` node declared while on; the dock's `audio` row is the device's side | ✓ the queued commands | the device closes a while after the last sound |
| [`enter_exit.rs`](rust/features/enter_exit.rs) | `enter` / `exit`: toasts that slide in and back out, the departing copy the core keeps | | Windows: drag the title bar mid-spring (W3) |
| [`exit_budget.rs`](rust/features/exit_budget.rs) | The exit budget at its boundary (ADR 0012): whole or not at all, the newest outranks the old, a virtual list keeps the picture small | | the boundary watch |
| [`popup.rs`](rust/features/popup.rs) | `WindowKind::Popup`: a combobox whose list is taller than the window; `--dock off` by default so the frame stays small | | press in the owner, drag into the popup, release on an item (ADR 0009) |
| [`theme.rs`](rust/features/theme.rs) | The token reference: every `Theme` role as a swatch over every stock widget that reads it; the devtools' base and accent icons are the switch | | |
| [`modal.rs`](rust/features/modal.rs) | `modal` (ADR 0003): a dialog over a form opening on its `initial_focus`, Tab confined to it, a confirm nested inside it that makes the dialog inert, Escape routed by tag and answered by the app, focus restored to the opener | ✓ | |
| [`devtools_tab.rs`](rust/features/devtools_tab.rs) | A tab of the app's own in the devtools panel (ADR 0032), in a tree-sitter inspector's shape: the source coloured by highlight group from a hand-written syntax tree, `devtools_tab_with` drawing the tree as an Inspector beside facts/events/tree — lazily, the closure runs only while the tab is on show — over the panel's tab body; hover both ways (a row lights its node's tokens, a token lights its path and scrolls the tab to it), a row's click selecting the token in the panel's tree, the panel's picker raised from the tab, the page's own button jumping to the tab (`set_devtools_tab`) | ✓ the laziness, the layout, both hovers, the doors, the pick, the jump | Ctrl+Shift+N to the Inspector, or the page's button; hover the source and the tree |
| [`metrics.rs`](rust/features/metrics.rs) | The palette's other axis (T2): the stock set, `compact` and a scaled set switched by a click, the stock widgets rebuilt from each, and a card of the app's own that reads `ui.metrics()` for its radius and padding | ✓ | |
| [`align.rs`](rust/features/align.rs) | Where the free space goes and what lines up (C13): a track of chips under the six main-axis alignments the buttons pick, the three spreads among them; a reading in three sizes at the top and on its baseline; and a `grow` card that keeps 16:9 beside squares sized from their height (C14) | ✓ the spreads' ends, the baselines, the ratios | resize the window: the card keeps its shape |
| [`accessibility.rs`](rust/features/accessibility.rs) | Every accessibility prop in one window, the fixture the platform audit drives; `--dock off` by default | | `scripts/ax-audit.swift` (106 checks) |
| [`waker.rs`](rust/features/waker.rs) | A thread feeds lines and wakes the parked loop through `kui_native::Waker`; `KUI_WAKER_LINES=n` closes after n | | `KUI_WAKER_LINES` |

### `tools/`

| Tool | Run | What it is |
|---|---|---|
| [`conformance-dump.rs`](rust/tools/conformance-dump.rs) | `cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt` | Writes the scene corpus's reference report the other bindings diff against; generated, never checked in |
| [`wire.rs`](rust/tools/wire.rs) | `cargo run -p kui-native --example wire [-- --latency 80]` | An experiment: the app and a headless core on one end of a socket, the window and the renderer on the other, the link held back by a simulated latency (`--serve` / `--connect` for two processes) |

## C — [`c/`](c)

Three programs over one [`common.h`](c/common.h) and the slots story,
built by `cargo run -p kui-devtools --bin cbuild`
([`devtools/src/bin/cbuild.rs`](devtools/src/bin/cbuild.rs); `-- --run`
runs the round it otherwise prints, which is what CI does), the same tool
on every platform. Everything lands in `target/<profile>/`, beside the
library the hosts link.

| Example | Shows | Headless |
|---|---|---|
| [`apps/counter.c`](c/apps/counter.c) | The counter from C, the same shape as the other three: kui as a plain library under `kui_run` | ✓ `counter --headless`, the Rosetta drive |
| [`features/slots/panel.c`](c/features/slots/panel.c) | C as the *extension*: a `dlopen`ed plugin filling the slot a host declares (ADR 0014), the same panel as the Lua one | through its hosts |
| [`features/slots/panel.rs`](c/features/slots/panel.rs) | The Rust host of that plugin: `cargo run -p kui-ffi --example c_panel` | ✓ `c_panel --headless` |
| [`features/slots/host.c`](c/features/slots/host.c) | C on both sides: a C host loading the same plugin through `kui_ctx_add_extension` | ✓ `host --headless` |
| [`tools/surface.c`](c/tools/surface.c) | The header walk: every prototype in `kui.h` called once and checked — the FFI self-test, not an example | ✓ `surface` |
| [`tools/conformance.c`](c/tools/conformance.c) | The C adapter over the scene corpus | ✓ `conformance <report>` |

```bash
cargo run -p kui-devtools --bin cbuild -- --run   # ABI check, the artifacts, the round
./target/debug/counter                   # the counter app, C as the host
./target/debug/host                      # a C host with the C panel inside it
cargo run -p kui-ffi --example c_panel   # the same panel in a Rust host
```

Windows asks for three things the unixes do not, and the plugin half is
where they show — an MSVC-ABI compiler, `kui_ffi.dll` beside each host,
and an import library at each link; `cbuild`'s header comment is the
long version, and it builds both plugin shapes from one compile of
`panel.c`. Two copies of the library in one process is fine as of ABI 10.

## Lua — [`lua/`](lua)

A Lua extension has no window of its own, so the panel is a Rust host with
Lua inside it. [`panel.lua`](lua/features/slots/panel.lua) is deliberately
the same panel as the C one: the extension contract is the contract and
the language is a detail. A script can be a host too — `env.add_extension`
opens a C plugin and `fill` places it — so once `cbuild` has run, the
panel is three languages deep.

| Example | Run | Shows | Headless |
|---|---|---|---|
| [`features/slots/panel.rs`](lua/features/slots/panel.rs) + [`panel.lua`](lua/features/slots/panel.lua) | `cargo run -p kui-lua --example lua_panel` | Rust host and Lua panel sharing one frame: the slot filled, the title from the params, the reply on a toggle, clicks routed by origin | ✓ `lua_panel --headless` |
| [`tools/bench.rs`](lua/tools/bench.rs) | `cargo run -p kui-lua --example bench --release` | Frontend-lowering shootout: the same ~900-node view from Rust and from Lua | |

## Node — [`node/`](node)

Build the addon with `cargo build -p kui-node --release`, then in
[`node/`](node): `npm install`, `npm run typecheck`, and `npm run <name>`
opens a window (`npm run smoke` runs every headless drive). A Node example
exists where Node has a door of its own — the pumped window loop,
`uniformList` and the `index` row, the typed messages — or where a round
needs it; the corpus already proves the four bindings lower alike.

| Example | Shows | Headless |
|---|---|---|
| [`apps/counter.tsx`](node/apps/counter.tsx) | The counter in JSX, the same shape as the other three | ✓ `node dist/apps/counter.mjs --headless` |
| [`features/window.tsx`](node/features/window.tsx) | The Node windowed driver: `init` handed the window, `resize` messages, images and sounds as the window's resources, custom chrome | |
| [`features/relaunch.tsx`](node/features/relaunch.tsx) | A second `runWindowed` in one process: the window closes and the same app reopens under the other chrome, on the event loop the first runner parked (F58) — the harness's `after` hook | **by hand:** press the button; under `KUI_SMOKE_FRAMES` the round passes only if the second window opened |
| [`features/slide.tsx`](node/features/slide.tsx) | `slide`: a canvas of floats and `line` connectors that eases everything or nothing, panned by an `onDrag` root — the by-hand check for F15 | ✓ the model's pan; **by hand:** drag the empty canvas and watch it while the button is down |
| [`features/drop.tsx`](node/features/drop.tsx) | A drop zone from Node, the twin of `rust/features/drop.rs`: `onDrop` and `dropBg` as props, the `drop` message's four phases in `update`, `ctx.dragFiles` / `dropFiles` / `dragCancel` as the headless drive and `dropTarget()` / `isDropTarget` as what it reads; "Open…" through `requestFiles`, answered headlessly with `takeFileRequests` / `answerFiles` | ✓ every path |
| [`features/devtools_tab.tsx`](node/features/devtools_tab.tsx) | `<devtoolsTab>` with a function child (ADR 0032), the twin of `rust/features/devtools_tab.rs`: the source coloured by highlight group, the syntax tree as the app's Inspector tab, the function called only while the tab is on show (once a view — a redraw re-lowers the stored stream), hover both ways through `onHover` tags, `setDevtoolsSelected` / `setDevtoolsPick` / `setDevtoolsTab` from `update` | ✓ the laziness, the layout, both hovers, the doors, the pick, the jump |
| [`features/clipboard.tsx`](node/features/clipboard.tsx) | The clipboard from Node, the twin of `rust/features/clipboard.rs`: `takeMenuActions()` as the queue a host drains, `answerSelectionRange` for a virtual list's ask, `setClipboard` / `requestPaste` from `update` with the surface in hand, the paste as a `text` message | ✓ every path |
| [`widgets/virtual_list.tsx`](node/widgets/virtual_list.tsx) | `uniformList`: 10,000 rows costing a screenful, re-sliced on the wheel with no model change; `list` over `RowHeights` for rows of no fixed height (`--variable`, and in the headless drive) | ✓ |
| [`widgets/select.tsx`](node/widgets/select.tsx) | `<select>` over labels and over menu items with an `id`: the field opens the core's menu under itself, a choice is one `menu` message, the app holds no open state | ✓ |
| [`widgets/controls.tsx`](node/widgets/controls.tsx) | `<switch>`, `<checkbox>` with `mixed`, `<radioGroup>` of `<radio>`s and `<slider>`s whose `ChangeMsg` the `update` stores — the twin of `rust/widgets/controls.rs` | ✓ |
| [`widgets/table.tsx`](node/widgets/table.tsx) | `<box dir="table">` (ADR 0033): rows whose cells line up, each column as wide as its widest cell and the `grow` one taking the rest; clickable rows, a right-aligned number, headers that sort | ✓ |
| [`tools/types.tsx`](node/tools/types.tsx) | The shipped `.d.ts` exercised: a typed drive over every app-facing type, run as code under `--headless` | ✓ |
| [`tools/bench.mjs`](node/tools/bench.mjs) | The JSX/Node side of `lua/tools/bench.rs` | |
| [`tools/smoke.mjs`](node/tools/smoke.mjs) | `npm run smoke`: every example in package.json's `kui.headless` roster, driven `--headless` in turn — the one list the shell round reads too | |

An extension is a C shared library either way, and the same binary loads
into a Rust, C or Node host: `<slot name="ns/panel" params={…}/>` places
it and `ctx.addExtension(ns, path)` (or a window's `extensions` option)
loads it.
