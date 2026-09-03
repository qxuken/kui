# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5.
Every item names the evidence that produced it, so a task that turns out to be
wrong can be argued with rather than guessed at.

Ordered by area, not by priority; the suggested sequence is at the bottom.

**Legend** — `!` a defect that ships today · `~` a gap with no workaround ·
`.` cost without correctness risk.

---

## Binding parity

The `PROPS` table is real infrastructure: it drives Lua by runtime lookup and
Node by kind, generates the TS types and `docs/props.md`, and pins `KuiSpec`
through `every_schema_prop_has_a_c_counterpart`. The ten `CUSTOM` composites
have none of that — each binding implements them by hand, and nothing checks
behaviour. Every defect below is in that gap.

### `!` P1 — Add a `description` prop row — **partly done (2026-09-03)**

The half that shipped is the C tooltip. `KuiSpec` gained a `tooltip: KuiStr`
field (appended): `spec_of` applies `hoverable()` + `description(hint)` and
`kui_close` floats `widgets::tooltip` below the node while it is hovered —
the same three effects, at the same point in the lowering, as the Lua and
Node parsers. That needed a per-context stack of open nodes
(`KuiCtx.open_tooltips`, pushed by every `kui_open*`), since the hint is
declared when the node opens and drawn when it closes. The `CustomProp`
row's C column now names the field, and P7's `tooltip` scene pins the
behaviour in all four bindings — reverting the C side to the old
`kui_tooltip`-gated-on-`kui_is_hovered` spelling fails the build with the
missing description on the access row.

Two things are still open. There is **no `description` `PROPS` row**, so a C
app (or any app) still cannot set an accessible description without a visible
tooltip; add one when something needs it, and the parity test will force the
rest. And the last question below is answered the other way: `kui_tooltip` /
`kui_tooltip_with` were left alone as the always-draws node form, matching
Lua's `tooltip("…")` node and `widgets::tooltip` in Rust. Making them imply
`hoverable` would have given C two spellings of the prop and no spelling of
the node.

The original finding:

`NodeSpec::description` (`crates/kui-core/src/spec.rs:348`) is reachable only
as a side effect of the `tooltip` composite, and C does not implement that side
effect:

```
kui-node/src/schema.rs:185   s.hoverable().description(hint)
kui-node/src/binary.rs:217   s.hoverable().description(hint)
kui-lua/src/lib.rs:504       s.hoverable().description(hint)
kui-ffi/src/lib.rs:2050      widgets::tooltip(ui, &kstr(text))   // and nothing else
```

So a C app cannot set an accessible description at all. `KuiAccessNode.description`
(`crates/kui-ffi/src/lib.rs:292`) exists on the read side with nothing able to write it.

Add a `description` row to `PROPS` (`Kind::Str`, `Apply::SpecStr`, next free wire
id after `P_FOCUS_BG = 63`). The parity test then forces the `KuiSpec` field, the
`include/kui.h` mirror, and the `spec_of` application. Keep `tooltip` as the
shorthand. Also decide whether `kui_tooltip` should imply `hoverable` and set
`description` like the other three — its `CustomProp` row documents it as if it does.

### `!` P2 — Fix the Lua float `self_at` docs mismatch — **done (2026-09-03)**

The `CustomProp` row now says `self_at`, and `docs/props.md` is regenerated.
P7's `float` scene pins it: spelling the key `self` in the Lua adapter fails
with a quad-digest mismatch on that scene, which is the diagnostic the
silent drop never gave.

The "accept both keys" half was deliberately **not** done, and is worth
arguing with rather than assuming. One spelling that a scene pins beats two
spellings that nothing does: an alias makes `docs/props.md` ambiguous again
and gives the corpus two shapes to cover for a key with no users yet
(alpha.5 shipped the wrong docs, so nobody can be relying on `self`). If
that reasoning is wrong — someone is following the published table — the
alias is a two-line change in `parse_float`.

While the row was open, three neighbours turned out to be wrong the same
way and were corrected: the Lua `pad` and `border` tables take **named**
keys (`{ l=, r=, t=, b= }`, `{ w=, color= }`, per `parse_edges` and the
`border` arm), not positional ones, and `title`'s C column named
`kui_set_window_title`, which is not a function — it is `kui_window_title`.
All three are now pinned by scenes.

The original finding:

`crates/kui-core/src/schema.rs:730` documents the Lua spelling as
`float = { anchor=, at=, self=, dx=, dy=, fit= }`. `parse_float` reads `self_at`
(`crates/kui-lua/src/lib.rs:647`). Unknown float-table keys are dropped silently,
so a user following `docs/props.md` gets a float attached by its top-left corner
with no diagnostic. The crate's own module doc says `self_at` — the `CustomProp`
row is the copy that is wrong, and it is the one that generates the docs.

Accept both keys, correct the row, regenerate.

### `~` P3 — Give Lua imperative focus and `is_pressed`

`env.focus` in Lua is the focused node's key as a value
(`crates/kui-lua/src/lib.rs:131`); `ctx.focus(key)` in Node
(`crates/kui-node/src/lib.rs:880`) and `kui_focus` in C are verbs that *move*
focus. Lua has no verb at all — no `focus`, `blur`, `focus_next`, `focus_prev` —
and no `is_pressed`, though it has `is_hovered` and `is_focused`. The `keyFocus`
row's doc promises "call `focus(key)`" to every reader of `docs/props.md`.

alpha.5 shipped, so renaming `env.focus` is now breaking. Prefer additive:
`env.focused` as the reading, `env.set_focus` / `env.blur` / `env.focus_next` /
`env.focus_prev` / `env.is_pressed` as the verbs, and a deprecation note on
`env.focus` so a later major release can converge on the Node/C spelling.

### `~` P4 — Warn on unknown props

Both dynamic bindings end their prop loop by ignoring unknown names
(`crates/kui-lua/src/lib.rs:511`; the equivalent in `crates/kui-node/src/schema.rs`),
because element-level props like `initial` and `src` fall through the same path.
So `hoverBg` in Lua, or `onclick` in JSX, does nothing and says nothing. This is
the class of silent misconfiguration `crates/kui-core/src/diag.rs` exists to
catch, and a bigger one than `grow-weight-ignored`.

Add an `unknown-prop` code. Put the per-element allow-list of legitimate
non-schema props in `schema.rs` so both bindings read it from one place, and warn
on anything outside schema ∪ allow-list. The warning has to be raised from the
binding, so a small `Core` entry point may be needed; keep it behind
`set_diagnostics`.

### `.` P5 — Generate `index.d.ts` instead of hand-writing it

`packages/kui/index.d.ts` is 686 hand-written lines describing the napi surface,
and CI's `git diff --exit-code` covers only `jsx-runtime.d.ts` and
`docs/props.md`. The `examples/node` typecheck catches a broken type only for
methods that example calls. Combined with P13 (two classes, 36 shared method
names), a method added to one class only is currently invisible.

Let napi emit its own `index.d.ts`, hand-write only the JS-side additions
(`createApp`, `runWindowed`, `decodeQuads`, the message types) in a separate
file, and add the generated one to the CI diff list. Check napi's output quality
for `Json` and `Buffer` params first; the weaker fallback is a script asserting
every `#[napi]` method appears in the hand-written file.

### `.` P6 — Guard `include/kui.h` against ABI drift — **done (2026-09-03)**

Took the static-assert route. `mod abi_parity` (`crates/kui-ffi/src/lib.rs`)
emits `target/kui-abi-assert.c` from the Rust layout — `sizeof` / `_Alignof`
per struct and `offsetof`, member `sizeof` and a `_Generic` type check per
field, for all 17 public `repr(C)` structs (176 fields), not just the six
listed below. `examples/c/build.sh` regenerates and compiles it against the
header before building the example, so the CI `check` job covers it. The
table cannot drift from Rust either: it rebuilds each struct field by field,
so a new field is a missing-field error naming it. The header needed no
correction — there was no drift to find.

`examples/c/counter.c` now uses all 90 exported functions (was 20): its
`--headless` mode runs a second `surface()` pass over measurement, keyed
nodes, focus, editors, accessibility, images, fonts, audio, window chrome,
diagnostics and values, asserting on what comes back. One gap it exposed: a C
host cannot drive an `on_key` sink at all — `kui_input_key` carries only the
editing keys, and raw presses reach a sink through `kui_run`'s loop alone.

The original finding:

`crates/kui-ffi/include/kui.h` is hand-mirrored from the `repr(C)` structs with
nothing enforcing it. The parity test's panic message says to "mirror it in
include/kui.h" but does not check that you did; the kui-ffi unit tests call the
Rust symbols directly and never go through the header; `examples/c/counter.c`
uses 21 of roughly 90 exported functions. A field added to `KuiSpec` but omitted
from the header misaligns every field after it, silently, at runtime.

Either generate a static-assert file (`sizeof` / `offsetof` per field for
`KuiSpec`, `KuiTextStyle`, `KuiQuad`, `KuiAccessNode`, `KuiKeyframe`, `KuiEnter`)
and compile it in the CI C step, or run `cbindgen` in check mode. The
static-assert route keeps the header's prose comments.

### `!` P7 — Build a cross-binding conformance corpus — **done (2026-09-03)**

`crates/kui-core/src/conformance.rs` holds the corpus: seven scenes
(`layout`, `overflow`, `float`, `tooltip`, `chrome`, `controls`, `media`),
each declaring the `CUSTOM` and `ELEMENTS` rows it exercises, the input to
replay, and its expected semantics. A test asserts the union covers both
tables — in both directions, so a scene cannot claim a row that no longer
exists either. Adding a hand-written prop or element without a scene now
fails `cargo test`.

Two tiers, because only one of them is portable between machines:

* `Scene::expect` is checked in and **font-independent** — access rows,
  events, warnings, exact solid and image quad counts, a lower bound on
  glyphs. `crates/kui-core/tests/conformance.rs` asserts it, which is what
  pins the reference behaviour.
* The cross-binding report digests **full quad geometry**, which moves with
  the installed fonts. It is generated per run (`cargo run -p kui-core
  --example conformance-dump -- target/conformance.txt`) and never checked
  in, which is why all four adapters have to run in CI's single `check`
  job, on one machine, against one dump.

The report format is the load-bearing detail: line-oriented, integers, hex
and strings only, **no formatted floats** — that is what lets Rust, C and
JavaScript emit the same bytes. Geometry travels as one FNV-1a-64 over each
quad's words 0..17 and 22..25 (`KuiQuad` minus `uv`, which follows glyph
insertion order and is a binding's own business). Node keys ride along as
hex, so the report pins tree shape too — keys are pure path hashes, and
match across bindings exactly when the structure and the labels do. Events
compare `kind` and `tag.kind` only, because the C `KuiValue` API cannot
enumerate a map.

The four adapters: Rust asserts natively; `crates/kui-lua/tests/conformance.rs`
re-expresses each scene as the table a script returns and diffs in-process;
`./examples/c/counter --conformance <report>` rebuilds them through the C
API alone; `packages/kui/test.mjs` runs each scene on all three transports
beside its existing `assertParity` (it skips when no reference is present,
so the publish job can still `npm test` against the prebuilds).

Two traps worth knowing before adding a scene. **The root node**: Lua's root
table is a *child* (`build_node` opens it) while a JSX top-level `<box>`
calls `configure_root`, so scenes never reconfigure the root and the JS
adapter wraps each one in `box({width:'grow',height:'grow'})` — the core's
implicit root spec exactly, so only `title` actually lands. **Window env**:
nothing exposes `env.window` to a bare `Ctx`, so `windowButtons` draws
nothing headlessly and its scene pins the lowering rather than the pixels;
closing that needs a window-env setter on the Node `Ctx` first.

It found both of the defects it was built from — P1's missing C description
and P2's `self_at` — and each was verified by reintroducing it and watching
the build go red.

The original finding:

**This is the root cause of P1–P4, not a fifth instance of them.**

`packages/kui/test.mjs` renders every schema prop, every composite and every
element through the binary, JSON and object paths and asserts byte-identical
quads. That mechanism exists because those three paths live in one language with
one harness. Lua, C and Rust each stand alone; nothing renders the same scene
through all four and compares. Which is why a whole accessibility behaviour is
missing from C and a documented Lua key is wrong, both with a green build.

Build a scene corpus in `kui-core` — named scenes plus expected quads, access
tree and events, covering every `CUSTOM` and `ELEMENTS` row — and four thin
adapters: Rust natively, `kui-lua`'s existing harness, a `--conformance` mode on
the C example beside its `--headless` one, and `assertParity` in `test.mjs`.
Wire all four into the `check` job.

Goal: the parity table becomes a build failure instead of a document.

### `.` P8 — Add macOS and Windows smoke jobs — **partly done (2026-09-03)**

Everything platform-specific is cross-compiled on one Linux runner and never
executed on the platform it exists for: `crates/kui/src/windows_nc.rs` (292 lines
of WM_NCHITTEST / DWM caption work), the macOS traffic-light inset in
`widgets::titlebar_with`, and the whole AccessKit bridge (UIA, NSAccessibility,
AT-SPI). `scripts/ax-audit.swift` is manual.

What landed is the honest half. The CI header no longer claims more than it has:
"Verified 2026-09-02: the Linux-built arm64 dylib loads on a Mac and passes the
parity tests" is now scoped to the addon's N-API surface, and a paragraph beside
it names the three uncovered pieces outright. `smoke-macos` and `smoke-windows`
are checked in — `cargo test --workspace` natively, `continue-on-error`, in no
release path and in no `needs:` — gated on the repository variables
`SMOKE_MACOS` / `SMOKE_WINDOWS`, because a Forgejo job whose `runs-on` label
matches no runner queues rather than failing fast, and this instance has only
the `docker` runner. The macOS command is known good (run on macOS 26.6 / rustc
1.98: the whole workspace passes with no display, no installed fonts and no GPU,
since the corpus splits its font-independent checked-in expectations from the
per-machine glyph digest). Neither job has been run by Forgejo, because there is
nothing to run it on.

What did not land is the coverage itself — and registering a runner would not
deliver it either: **neither `windows_nc.rs` nor `access_bridge.rs` contains a
single `#[test]`**, so both smoke jobs would prove that the platform code
compiles, links and loads against the real SDK, not that it behaves. Two
follow-ups, in order of value per unit of work:

* Make the hit-testing pure and test it on the Linux runner that already exists.
  `hit_code` (`crates/kui/src/windows_nc.rs:175`) reads `NcState` — scale,
  `resize_border`, `maximized`, the region list — plus a point and the window
  size, and that size is its only OS dependency (`client_rect(hwnd)`). Split out
  `hit_code_in(&NcState, size, pt)` and the border band, the
  topmost-region-wins rule and the role → `HTCAPTION`/`HTCLOSE`/`HTMINBUTTON`/
  `HTMAXBUTTON` mapping all become testable with no Windows involved — most of
  the substance of those 292 lines. The catch: the `HT*` constants come from
  `windows-sys` and the module is `#[cfg(target_os = "windows")]`, so the pure
  half has to move somewhere compiled everywhere, with the codes as local `u32`
  consts. They are stable ABI numbers, so that is safe, but it is a deliberate
  trade rather than a free refactor.
* What remains after that genuinely needs the OS: `DefSubclassProc`, the
  non-client mouse-message mirroring, `WM_NCMOUSELEAVE`, and the AccessKit
  bridges. On macOS that is exactly what `scripts/ax-audit.swift` covers, and it
  cannot be made unattended on an ephemeral runner — the window needs a
  logged-in GUI session and a Metal device, and the Accessibility permission is
  a TCC grant, per machine and given by hand, unscriptable without disabling
  SIP. So it is now written down as a pre-release step in the README instead of
  living only in ADR 0001; a permanently self-hosted Mac with auto-login and the
  grant in place could fold it into `smoke-macos`. On Windows there is no
  equivalent tool and no plan for one.

---

## Core capability

Composition-over-traits keeps widgets reachable from Lua and C, and it holds for
buttons, titlebars, tooltips and the latency HUD. It stops holding wherever the
widget needs state the core owns exclusively: pointer capture, the Tab ring,
retained scroll offsets, paint order, the OS window.

**The test used here:** could an app author build it from what the bindings
expose today? If no *because the core keeps the state privately*, it belongs in
the core. If no because the data isn't in the IR, it needs a schema row first.

### `~` C1 — Modal scope — **done (2026-09-03)**

Shipped as `docs/adr/0003-modal-surfaces.md`: a `modal` row (`Kind::Tag`,
so the node and its dismiss tag are one declaration), the Tab ring scoped
to the last declaring subtree with focus pulled in and handed back, no hit
regions outside it (window chrome excepted, so a dialog cannot trap the
window), inert wheel and scrollbar thumbs behind it, `{kind:"dismiss",
reason:"escape"|"outside"}` on the modal node, `AccessNode::modal` →
AccessKit `set_modal` with a derived `dialog` role, and a
`modal-behind-content` warning for a modal that is not floated. Nesting is
tree order — the last modal wins, so a confirm inside a dialog needs no
stack API — and a modal that is itself a float needs nothing special: the
scope is a tree range, so a dialog's own dropdown stays live.
Focus entering a modal keeps the visibility it had, so a keyboard-opened
dialog shows its ring at once (decided 2026-09-04, pinned in
`tests/tab_focus.rs`).

Deferred, and named in the ADR: a corpus scene for the behaviour, a
`modal-without-name` warning, initial focus placement inside a dialog
(`autofocus`), and the ARIA composite patterns (arrow keys inside a menu).
Found in review and left open, each small:

- An assistive-technology `Focus` request on a node behind the modal still
  lands there for one frame (`handle_access` checks the tree, not the modal
  scope) before containment pulls focus back in with the ring showing. The
  access tree also still advertises `Click` / `Focus` on inert nodes.
- A press on the modal's own padding drops focus, and containment then
  pulls it to the *first* control rather than leaving it dropped, so
  clicking a dialog's background moves focus out of its editor and onto OK.
  Treating `None` as already contained would keep the ADR 0002 behaviour.
- `widgets::button` keys a node by its text, so a label that changes on
  press is a new node: focus drops, and a screen reader is left holding a
  dead element (the accessibility example hit this with its counter). A
  `button_keyed` helper, or a warning when the focused key leaves the tree,
  would make it hard to repeat.

The gap it closed, as found:

`Role::Dialog` is declarable, but `focus_ring()`
(`crates/kui-core/src/runtime.rs:812`) walks every node from index 0, skipping
only `role="none"` subtrees and disabled nodes. So a modal dialog announces
itself as a dialog and then lets Tab walk straight out into the content it is
supposedly blocking. No `modal` flag, no focus containment, no inert-behind, no
dismiss-on-Escape convention. Not fixable in userland — the ring is core-private.

This changes the focus model ADR 0002 just settled, so write ADR 0003 first.
Sketch: a `modal` row; `focus_ring()` scoped to the last declaring subtree;
hit-testing outside it inert; Escape emitting `{kind:"dismiss"}`; the access tree
marking it modal for AccessKit. Decide nested modals and modal-that-is-a-float.

Unblocks dialogs, popovers and menus together, with C2 (also done).

### `~` C2 — Secondary mouse button and `onContextMenu` — **done (2026-09-04)**

Shipped as `MouseDown { button, clicks }` / `MouseUp { button }` over a
`MouseButton` enum (`Primary` / `Secondary` / `Middle` / `Other(n)`, with
`code()` / `from_code()` / `from_name()` so C takes a number and Node a
name), plus an `onContextMenu` `PROPS` row (`Kind::Tag`) that emits
`{kind:"contextmenu", x, y, tag}` on the topmost node under the press, at
the logical viewport point to open a menu at. The two decisions, both the
way the question expected:

- **A secondary press moves no focus** — and places no caret, starts no
  drag, grabs no scrollbar thumb. Right-clicking a selection has to leave
  it selected, or a "Copy" item cannot work; the platforms agree.
- **It suppresses `on_click` by construction**: only the primary button
  sets `Interaction::pressed`, so nothing is held to release, a right
  press in the middle of a held left one changes nothing, and the menu
  comes out on the press rather than the release.

Additive across the drivers as predicted: `kui_input_mouse` still means
the primary button (exported ABI, untouched) alongside
`kui_input_mouse_button`; `ctx.mouse(down, clicks, button?)` takes an
optional name; winit maps Left/Right/Middle/Back/Forward/Other, and
counts multi-clicks for the primary button only. `InputEvent::mouse_down`
/ `mouse_up` are the primary spellings the Rust drivers and tests use.
The corpus's `controls` scene grew the prop and two secondary steps, so
all four bindings lower it. Nothing routes `Middle` or `Other(n)` yet:
no middle-click-to-close, no right-drag, no per-button `on_click`; the
README's Status section now says so.

### `~` C3 — Derived cursor shape — **done (2026-09-04)**

Shipped as `kui_core::cursor::CursorShape` (ten shapes: the arrow, `Text`,
`Pointer`, `Grab` / `Grabbing`, `NotAllowed`, and the four resize
diagonals the band already used), resolved by `Interaction::cursor_shape`
from the topmost hit region under the pointer — the same region a click
would go to — and read back through `Core::cursor_shape` /
`kui_cursor_shape`. The derivation is the one the question proposed:
window chrome and a plain box are the arrow, an editor is `Text`, an
`on_drag` node is `Grab`, and anything clickable or focusable is
`Pointer`. A `disabled` node derives nothing, because its payloads are
already stripped by the time a hit region exists — quiet is the right
default, and `cursor="notAllowed"` is how a control says otherwise.

Two things the topmost-node rule alone would have got wrong, both
decided in the core:

- **A captured drag owns the pointer.** The shape stays the dragged
  node's however far the cursor wanders off it, so a splitter that
  declared `ewResize` keeps it for the whole gesture instead of turning
  into `grabbing` — the override survives the capture, it does not just
  seed it.
- **Scrollbars win the shape, as they win the press.** They draw over
  content and hit-test over it, so an overlay bar across an editor is the
  arrow, not an I-beam.

Output is a query, not a drain: the shape is a state rather than a queue,
so `take_*` would have been the wrong shape of API. The winit runner reads
it after each input dispatch and each frame, maps it one-to-one onto
`CursorIcon` and only touches the window when the icon changes; its
synthesized resize band still wins on top, since a press there resizes the
window rather than reaching the UI. Headless drivers never read it and the
core stays device-free. One `PROPS` row (`Kind::Enum(CURSORS)`) gave all
four bindings the override; `KuiSpec.cursor` is `KUI_CURSOR_*` = index + 1,
so zero still means "derive".

### `~` C4 — Export `reveal()` and `set_scroll()` — **done (2026-09-04)**

Shipped as `Core::reveal(key)`, `Core::set_scroll(key, offset)` and
`Core::scroll_offset(key)`, in all four bindings (`ui.reveal` /
`ui.set_scroll` / `ui.scroll_offset`, `ctx.reveal` and `KuiWindow.reveal`
with `scrollOffset` / `setScroll`, `kui_reveal` / `kui_set_scroll` /
`kui_scroll_offset`, `env.reveal` / `env.scroll_offset` /
`env.set_scroll`). `set_scroll` and `scroll_offset` are the retained
`ScrollStore` handed out directly, which is all they ever needed to be.

`reveal` is the one that needed a decision. Resolving it immediately
against the last frame's layout — the obvious reading, and what
`focus_next` does — would have been useless in two places. Lua's `env`
only exists inside `view(env)`, where the tree has already been cleared
and is being rebuilt, so a script's reveal would have found nothing every
time; and an app that appends a row and reveals it in the same update has
no earlier frame the row appears in. So a reveal is a request that the
next `finish_frame` resolves after layout, next to `scroll_caret_into_view`
and through the same `relayout` path, so the frame it resolves in already
draws the node in view. It requests a frame, since a retained offset that
nothing redraws is not a scroll. A key that frame does not declare is a
no-op and is not held for a later one — the alternative, a request that
waits, fires at whatever unrelated moment the key next appears.

This unblocks C5: a virtualizing list needs `scroll_offset` to decide what
to build, and `reveal` to answer "scroll to row N" when row N is not one of
the ones it built.

### `~` C5 — Make long lists affordable — **(a) done (2026-09-04)**

Option (a) shipped: `Core::scroll_geometry(key)` retains what the last layout
resolved for a scroll container — its box, its content size, the offset and
the travel — in all four bindings (`ui.scroll_geometry`,
`ctx.scrollGeometry` / `KuiWindow.scrollGeometry`, `kui_scroll_geometry`,
`env.scroll_geometry`). The numbers already existed: `layout::positions`
computed the container's rect and its overflow on the line it called
`ScrollStore::clamp`, and threw both away with the tree. `clamp` became
`resolve`, which keeps them; the export is a getter, as C4's was.

**`on_layout` was the alternative and is the wrong shape for this**, though
it can carry the same rect. It is an event, so learning a number the core
already has costs a prop, a tag, an `on_event` arm, a model field and a
re-render. It is edge-triggered, so the first frame has no rect at all and
builds the whole list — the case being removed. And it reports viewport
coordinates *after* scrolling, so a list inside another scrolling container
posts an event on every frame of an unrelated scroll, and every one of them
forces a rebuild. A query has none of that, and pairs with `scroll_offset`,
which is already one.

The geometry is deliberately one moment rather than the live store: its
offset is the retained one already clamped to that container's travel. Without
that, C4's documented "a huge value means the end" hands a slicing view a row
index a million past its data — which is exactly what the Lua test did before
the clamp moved.

`widgets::virtual_column` is the uniform-row case done (visible rows, two of
overscan, two spacers), `widgets::visible_rows` the arithmetic alone, and
`Ui::open_indexed` / `with_indexed` / `child_key_index` give a row the key
auto-keying would have given it, so a virtualized list and a full one agree
on identity. Benched: `list_10k_rows_naive` ~4.8 ms → `list_10k_rows_virtual`
~19 µs, and `list_100k_rows_virtual` the same ~19 µs. The pre-existing grid
benches are unchanged (`resolve` runs only for scroll containers); the ~5%
they appear to move is the bench binary gaining functions, and goes away when
both sides are measured with the same set of benches.

**(b) is still not started, and now needs a case (a) does not serve.** A
`virtual` flag on a uniform-height container where the core skips the
off-screen range is a design in its own right — it touches auto `Key`
assignment, the Tab ring (a focusable node that was skipped is not in the
ring), the access tree and `on_layout` — and (a) answers the log viewer, the
data table and the chat history. The one thing (a) does not answer is rows of
*varying* height, where the app cannot compute a spacer without measuring
every row; a case like that is what would justify (b).

### `~` C6 — `selected` and `expanded` on the access tree

`crates/kui-core/src/access.rs:738` lands `checked` for `Checkbox | Radio |
Switch` only. `Role::Tab`, `TabList`, `ListItem` and `Link` have no selected
state, so a screen reader reads a row of tabs and never says which is active —
`crates/kui/src/access_bridge.rs:246` only ever sets AccessKit's `toggled`. Also
absent: `expanded`, `posInSet`/`setSize`, `required`/`invalid`. ADR 0002's own
next-steps name arrow keys in radio groups, tab lists and lists; this is the data
half of that step.

A `selected` row, then `expanded` — each one `PROPS` row plus one
`access_bridge` line, and all four bindings get them free. `posInSet`/`setSize`
could be derived from a `Role::List` parent rather than declared, which fits the
design better. Live regions are a side channel, not a tree property — note them
in ADR 0001's follow-ups and leave them out of scope.

### `.` C7 — Decide the multi-window story on paper

`runWindowed` says "One window per process" outright; `kui_run` takes one app;
`WindowCommand` covers drag, close, minimize, maximize and nothing else. A native
menu, a dropdown extending past the window edge, and a tear-off panel all need a
second OS surface. Nothing in the architecture forbids it — `Core` is already
per-viewport and per-scale — but nothing supports it, and the FFI runner shape
hardcodes the assumption.

Write the ADR now, build later. Cover: `Core`-per-window vs one `Core` with
several roots; what `WindowCommand` grows; whether `UiEvent` needs a window id;
whether OS popups are a distinct chrome-less window role with
click-outside-to-dismiss (and how that meets C1); what the four entry points look
like. `FloatConfig::fit` is the in-window approximation and should be documented
as such.

### `.` C8 — Extend the paint vocabulary

`crates/kui-core/src/display.rs` is the whole renderer contract: fill, border,
four radii, glyph, image. No shadow, no gradient, no group opacity. `Enter` fades
a node's own `bg` but not a subtree, which is the underlying reason there is no
exit animation. Clipping is rect-only, so a rounded scroll container does not
round its children.

Highest value per unit of work is probably **group opacity** — an `opacity` slot
multiplied into every quad from a subtree during `finish_frame`. **Exit
animations** need the core to keep a departing node alive for one transition,
which is a real change to the frame model (sketch how it is tracked and dropped;
`AnimStore` already retains by `Key`). **Shadows** need a new `QuadKind` with a
blur radius plus shader work, or a nine-slice. **Gradients** are probably out of
scope for v0 — say so rather than leaving it unstated. Anything added needs a
schema row and a `kui-wgpu` shader path; the quad struct is hot, so benchmark.

### `.` C9 — `KeyUp` and key repeat

`InputEvent::KeyDown(KeyPress)` has no `KeyUp` and `KeyPress` has no repeat flag.
`Modifiers` covers modifier release only. So a held-key interaction — WASD, press-
and-hold to preview, a key that arms a mode — cannot be written, though
`KeyDown`'s own doc names "a game" as its audience.

Add `KeyUp` and `repeat: bool`, route to key focus the way `KeyDown` is routed,
and decide the payload shape (`{kind:"key", phase:"up"|"down"}` keeps one shape;
whichever you pick has to round-trip through the `EVENTS` table and all four
bindings). Decide what happens on focus change while a key is held — a synthetic
release is usually right.

### `.` C10 — Flex wrapping, and the smaller layout gaps

`crates/kui-core/src/layout.rs` has no wrapping; `Align` is Start/Center/End
only. Missing: **wrapping** (tag lists, chip toolbars, responsive button rows —
no userland workaround short of measuring everything by hand), **space-between /
around / evenly** (a grow spacer covers it, so low priority), **baseline
cross-alignment**, **aspect ratio**.

Wrapping first, the rest as follow-ups. It changes the passes structurally —
children group into lines before main-axis distribution, and the cross-axis fit
becomes a sum of line heights. Read `shrink_axis`'s comment carefully: wrapping
and shrinking are alternative responses to the same overflow and need a defined
interaction. Bench it; the solver is the hot path.

---

## Repetition

Four bindings over one contract means some parallel code is the price of the
design. These are where the price is paid without the benefit.

### `.` D1 — Collapse `Ctx` and `KuiWindow` with a macro

Two `#[napi]` classes in `crates/kui-node/src/lib.rs` — `Ctx` (52 methods, from
line 466) and `KuiWindow` (43, from line 1034) — share **36 method names**. Each
pair differs only in the accessor and a trailing redraw:

```
lib.rs:872    pub fn focus_visible(&self) -> bool { self.core.focus_visible() }
lib.rs:1200   pub fn focus_visible(&mut self) -> bool { self.runner.core_mut().focus_visible() }
```

The doc comments are duplicated and have already drifted — `focus_visible` reads
"when it shows (the ring, or `focusBg`)" on one and "(when the ring / `focusBg`
shows)" on the other. Then all 36 are written a third time by hand in
`index.d.ts`. Three copies of one API is where a method quietly ends up on one
class only.

A `macro_rules! core_methods` taking an accessor and a redraw policy, invoked
once per class — verify `#[napi]` works inside a macro body early, it is the
load-bearing assumption. ~700 lines become one definition list. Keep the
genuinely divergent methods hand-written (constructors, `pump`, `size`,
`frame_*` vs `set_view_*`, `quads`, input injection). With P5, three copies
become one.

### `.` D2 — Route Node's JSON lowering through the encoder

`crates/kui-node/src/lib.rs` dispatches JSON element names and
`crates/kui-node/src/binary.rs` dispatches opcodes — two ~200-line functions, one
carrying the comment "Mirrors the JSON path's button styling exactly."
`test.mjs` genuinely holds them equal, so this is maintenance cost rather than a
correctness risk — but `index.js` documents the cost being paid: the object walk
is "~10x slower" because every property read is an N-API call into V8.

Parse JSON in JS and feed the existing encoder, lowering once. Then decide the
fate of `frameObject` / `setViewObject`: keep them only if something needs the
debug reference, otherwise retire them with their share of the parity test.
Check what this does to error messages first — `test.mjs` has an "errors are the
same on every transport" test, and whatever replaces the JSON-path errors has to
be at least as good.

### `.` D3 — Lift composite parsing into `kui-core`

`float_of` (`crates/kui-node/src/schema.rs:57`) and `parse_float`
(`crates/kui-lua/src/lib.rs:623`) are the same twenty lines in two languages,
with a third at `P_FLOAT` in `binary.rs` and a fourth as `KuiSpec` fields. Same
shape for `pad`, `border`, the overflow bits, and the
`tooltip`/`keyFocus`/`key`/`title` quartet. **P1 and P2 are what this costs.**

Push the logic down, keep the extraction up. A binding should pull already-typed
scalars out of its own value type and call one core constructor —
`FloatConfig::build(anchor, at, self_at, dx, dy, fit)`,
`NodeSpec::apply_tooltip(&str)` owning all three of the tooltip's effects. It
should not decide what "below" means or which key holds the self-attachment.
This does not eliminate per-binding code — extracting from a Lua table, a
`serde_json` Map, a binary stream and a C struct is genuinely different work —
but it moves every *decision* into one place.

### `.` D4 — Unify `createApp` and `runWindowed`

`packages/kui/index.js` has two drivers duplicating the diagnostics gate, the
warning formatter and the transport switch — and diverging where they should not.
`createApp` calls `update(model, msg, event)`; `runWindowed` calls
`update(model, ev.payload, ev, win)`. `tick` exists only in the windowed loop, so
**a ticking app cannot be driven by the headless test driver** — which undercuts
the README's central claim that the same app runs headless. The test affordances
(`click`, `type`, `key`, `settle`, `access`, accumulated `warnings`) exist only
on the headless side.

One loop object over an injected surface; `Ctx` and `KuiWindow` satisfy the same
handful of calls (more obviously so after D1). Make the fourth `update` argument
unconditional, move `tick` into the shared loop with an injected clock (the core
is already clock-free — `setTime` is how the headless driver supplies time), and
let the driver helpers work against either. Keep `runWindowed`'s
promise-resolves-with-final-model contract and `createApp`'s synchronous shape;
those differences are real.

---

## Documentation

### `!` X1 — Fix the `width`/`height` main-axis docs

```
crates/kui-core/src/schema.rs:288   doc: "Main-axis size: px | \"fit\" | \"grow\" | \"N%\"."
crates/kui-core/src/schema.rs:295   doc: "Cross-axis size: …"
```

`Dir::Column` is the default, so for the majority of nodes these name exactly the
wrong axis. Layout treats them as literal axes regardless of `dir` — `fit_widths`
is horizontal always — and `LayoutSpec`'s own field docs correctly say nothing
about main and cross. The schema row is the only place the claim is made, and the
one place that propagates: into `docs/props.md` (lines 28, 59) and into the JSDoc
on `width`/`height` in `jsx-runtime.d.ts`, where every JS user reads it on hover.

"Horizontal size" and "Vertical size", then `npm run gen`. Scan the other `doc`
strings for the same mistake while there.

### `.` X2 — Automate the CHANGELOG heading

`CHANGELOG.md`'s top heading still reads `## 0.1.0-alpha.5 (unreleased)` after
alpha.5 shipped. `scripts/set-version.sh` does not touch the changelog and
`scripts/check-version.sh` does not check it, so the one guard that refuses a
mismatched tag has a hole exactly where the human step is.

Correct the shipped heading; add the rewrite to `set-version.sh` (version +
today's date, in the sed pass it already uses); add one assertion to
`check-version.sh` (top heading names the version, does not say "unreleased").
Keep the automation to the heading line — the "what you can delete" convention in
the body is worth keeping human.

### `.` X3 — List the missing input modes in Status / next

The section is unusually honest about z-index, exit animations, layout-query
depth, audio and editing scope. That honesty is why the omissions it *doesn't*
mention read as present: no touch or pen input, no flex wrapping (C10). Add
them, grouped, in the section's existing tone. (Modal containment was on this
list until C1 shipped it; the pointer buttons went on it with C2, which routes
only the secondary one; cursor shapes came off it with C3, and programmatic
scrolling with C4.)

The performance table also lists four benches where `benches/frame.rs` has eight —
`frame_10k_rects_with_access_tree` is omitted, and it is the one a reader worried
about the cost of the accessibility work would look for. (The three list benches
C5 added are described in a paragraph under the table rather than as rows,
because they were measured on a slower machine than the table's.)

---

## Suggested sequence

By leverage-to-effort, not severity.

1. **Ship-wrong first.** X1, X2, P2 — small, and they are wrong in published
   artifacts right now.
2. **Two schema rows.** P1 and C6. Each is one `PROPS` row plus one
   `access_bridge` line; the parity test forces the C side and all four bindings
   get them free. Worth doing early to confirm the schema mechanism still does
   its job.
3. ~~**Export what already works.** C4 (`reveal`, `set_scroll`); C3 (derived
   cursor).~~ Both done — exports of working internals, and C5(a)
   (`scroll_geometry`) turned out to be a third: the number was already
   computed, one line above where it was thrown away.
4. **D1.** The largest single duplication, mechanical, and it stops the drift
   that has already started in the doc comments.
5. ~~**P7 — the conformance corpus.**~~ Done (2026-09-03), out of order: it
   was cheaper to fix P1 and P2 *through* the corpus than beside it, and a
   fix with no scene behind it is the state this document exists to
   describe. P4 (unknown-prop warnings) and D3 (composite parsing in the
   core) are the two that still shrink the surface it has to cover.
6. **Then the designs.** ~~C1 + C2 unlock dialogs, menus and comboboxes
   together~~ — both shipped (ADR 0003 for C1; C2 needed no ADR of its own,
   since it only adds a row and a button to the model 0003 settled), and a
   context menu is now an `onContextMenu` tag plus a `modal` float. C7 is
   the one to decide on paper now and build later, before more API assumes
   a single window.
