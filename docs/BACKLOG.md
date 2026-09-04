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

**Reversed by D3 (2026-09-04)**: the alias is in, and `docs/props.md` names
`self`. What changed is that both spellings are now pinned and the fallback
rules live in one core function, so "two shapes for the corpus to cover" no
longer describes it.

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

### `~` P3 — Give Lua imperative focus and `is_pressed` — **done (2026-09-04)**

Shipped as proposed, minus one premise. `env.is_pressed(key)`,
`env.set_focus(key)`, `env.blur()`, `env.focus_next()` and `env.focus_prev()`
are in `env_table`, and the `keyFocus` row now names each binding's verb
rather than promising `focus(key)` to everyone.

**No deprecation, because there was no duplicate reading.** The item was
filed believing `env.focused` had appeared as a second spelling of the node
key. It has not: `env.focused` is `Env::focused`, the *window*'s keyboard
focus, a bool, and it has been in `env` since env existed (`f189ab4`). Node
carries the same fact on its own `env` object and keeps the node reading on
`ctx` — `ctx.focused()`. Lua has one table where Node has two, so the name
`focused` was spent on the window fact before the node reading needed it,
and `env.focus` is the only spelling Lua can have. Deprecating it would have
left the node key unreadable from Lua. Both names are now documented against
each other at the top of `crates/kui-lua/src/lib.rs`; `set_focus` keeps its
longer name for the reason the item gives (alpha.5 shipped `env.focus` as a
value) and the other three verbs match Node and C exactly.

**One core change was needed.** `focus_next` / `focus_prev` could not be
plain wrappers. The Tab ring is built from the finished tree, `begin_frame`
clears it, and `view(env)` is the only place a script ever holds an `env` —
so an immediate step from Lua always walked an empty ring and did nothing.
`Ui::focus_next` / `focus_prev` now defer to `finish` via
`Core::request_focus_step`, mirroring `pending_reveal`, applied after the
modal scope resolves (which is what scopes the ring). This changes only the
frame-scoped `Ui` handle, where an immediate step was never correct; Node
and C call `core().focus_next` directly and are untouched, and `Ui`'s pair
had no callers. Pinned by `a_view_steps_focus_onto_the_frame_it_is_declaring`
in `crates/kui-core/tests/focus.rs`.

A related sharp edge the tests now pin: `env.focus` is a value the host
writes before `view` runs, so it does not see a verb called in the same
frame. `env.is_focused(key)` is the query that does.

The original finding:

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

### `.` P4 — Warn on unknown props — **done (2026-09-04)**

`hoverBg` in a Lua table and `onclick` in JSX now say so, as an
`unknown-prop` warning naming the element and the spelling it was probably
meant to be. The allow-list is the schema tables themselves: `CUSTOM` rows
carry `jsx_names` / `lua_names` (every spelling of each composite —
`borderW` here, `border = {…}` there) and `ELEMENTS` rows carry `jsx_own` /
`lua_own` (the props an element lowers itself: `<edit initial>`,
`<image src>`), so `schema::known_prop(element, name, spelling)` answers
both bindings from one place and `schema::suggest` supplies the guess.

Per spelling, not per binding-with-a-shared-list: JSX's camelCase and Lua's
snake_case are separate allow-lists, so `hover_bg` in JSX is as unknown as
`hoverBgg` — which is the truth, since neither binding reads the other's
names. `schema::LUA_ALIASES` holds the one exception (`direction` for the
`repeat` row, `repeat` being a Lua keyword) and the Lua parser remaps
through the same table it is checked against.

The warning is raised by the binding rather than by the tree walk, since a
name nothing claims never becomes part of a node: `Core::warn` takes a
built `Warning`, behind the same `set_diagnostics` gate and the same
once-per-(code, key) dedup, and the key is derived from (element, name) so
a misspelling costs one line however many nodes carry it. Lua checks the
node table in `build_node`; for Node the *encoder* is the only side that
ever sees the name (an unknown one has no wire id, so it cannot reach the
stream), so it collects `[element, name]` pairs and `frame` / `setView`
hand them to `warnUnknownProps`.

Found on the way: the `title` composite's Lua spelling was documented as
`title`, but the root table has always read `window_title`.

### `.` P5 — Generate `index.d.ts` instead of hand-writing it — **done (2026-09-04)**

The addon's surface — `protocol`, `quadStride`, `Ctx` and `KuiWindow`, 102
members — is now generated into a marked region of `index.d.ts`, the same
shape `jsx-runtime.d.ts` and `docs/props.md` already had, and CI's
`git diff --exit-code` covers it. `crates/kui-node/build.rs` sets
`NAPI_TYPE_DEF_TMP_FOLDER` through `cargo::rustc-env` (napi-rs writes its
derived signatures only when that names a directory, and does not create
one), so any build of the addon leaves them in `target/napi-type-defs` and
`npm run gen` renders them. A method added to one class only — what D1
could not close — now fails the build; adding one to `core_methods!` and
regenerating puts `driftProbe(): number` on both classes, which is how that
was checked.

The output was checked before it was trusted. The risk this codebase was
warned about is not one: `#[napi]` is a proc macro and expands *after*
`core_methods!`, so both classes come out complete with their doc comments,
and `Buffer` params arrive as `Buffer` (napi carries a marker for a CLI that
wants to `import` the type, and a plain name in the `def` for one that does
not).

`Json` is where it was **not** good enough — every payload parameter and
return derives as `any`, which would have turned `accessTree()`,
`pollEvents()`, `measureText()`, `scrollGeometry()` and a dozen more into
untyped calls. That is the case this entry called for the weaker fallback (a
script asserting every `#[napi]` method appears in a hand-written file), and
it was **not** taken: `#[napi(ts_return_type = ...)]`, `ts_args_type` and
`ts_generic_types` let the Rust side name the exact TypeScript type at the
definition, so the generated file is as precisely typed as the hand-written
one was — `pollEvents<A = AppMsg | CoreMsg>(): UiEvent<A>[]` and all. Keeping
a second copy and only checking it is strictly worse than not having one.

Two departures from the plan, both worth arguing with. The generated half is
a **region inside `index.d.ts`** rather than a separate file the hand-written
one re-exports: the repo already generates regions this way, and a separate
file would have needed either a phantom `./native.js` specifier or a subclass
to hang `frame` / `setView` on. Those two are the only JS-side additions to
the classes (they live on the prototypes in `index.js`, since `encoder.js` is
the only thing that encodes a frame) and they reach the generated classes by
class/interface declaration merging, in the same file. And the doc comments
are now the Rust ones — the `.d.ts` copy had prose the Rust copy did not
(what `measureText`'s numbers are for, that `createApp` collects warnings for
you, what the `A` on `pollEvents` is), which was folded into `lib.rs` rather
than dropped.

`npm run gen` reads `target/napi-type-defs/kui-node` and does not build. The
one hole is a target dir whose defs file was deleted but whose addon is still
up to date: a plain `cargo build` says "Fresh" and writes nothing, so gen
re-runs it with `NAPI_FORCE_BUILD_KUI_NODE` — the env var napi-build declares
`rerun-if-env-changed` on — rather than telling the user to run a command
that would not work.

The original finding:

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

`crates/kui-core/src/conformance.rs` holds the corpus: nine scenes
(`layout`, `sizing`, `wrap`, `overflow`, `float`, `tooltip`, `chrome`,
`controls`, `media`), each declaring the `CUSTOM` and `ELEMENTS` rows it exercises, the input to
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
API alone; `packages/kui/test.mjs` drives each scene through the JS encoder
(it skips when no reference is present, so the publish job can still
`npm test` against the prebuilds).

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

**Two scenes added (2026-09-04), from mutation-testing the Node encoder.**
`float`'s full config was symmetric — `at` equal to `self_at`, `dx` equal to
`dy`, and attached at a point already inside the viewport — so a binding
could swap either pair or drop `fit` entirely and every adapter still
passed. It now attaches at (-16, 254), off the viewport on both axes, with
`at(Start, End)` against `self_at(End, Start)` and `dx: -6, dy: 14`, so all
three are load-bearing. A new **`sizing`** scene puts one child of each mode
in a row of *known* width — fixed 30, 25% of 200 = 50, fit around a 20-wide
child, grow taking the remaining 100 — because the four modes are otherwise
only exercised inside fit-sized parents, where a percent and a grow collapse
to the same geometry. Its outer pad is the `padX`/`padY` pair spelled
unequally (14 / 6), which nothing else covered: the `layout` scene sets all
four edges explicitly, so the shorthands never resolve.

Together they close seven mutations that every binding used to pass; the C
and Lua adapters fail on them now too, which is the point of putting the fix
here rather than in `test.mjs`. **The lesson generalizes: a scene built from
symmetric values pins nothing about order.** Prefer distinct numbers per
axis and per slot when adding one.

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

### `~` P9 — Let the corpus drive a frame under custom chrome

Found by making the corpus's coverage derived rather than declared
(`conformance::observe`, 2026-09-04): the `chrome` scene claims the
`windowButtons` element and builds nothing. `widgets::window_buttons` returns
early unless `env.window.custom_chrome` is set, and nothing in the corpus can
set it — `Env` is pushed by a frame driver, and the corpus drives a bare
headless `Core`. So the one element whose whole purpose is custom chrome is
covered by a claim with no tree behind it, and has been since P7 landed. It
is listed in `conformance::UNDERIVED` with that reason, which makes it
visible but does not make it covered.

The fix is one line of protocol — `drive` declares custom chrome before the
first frame — and one call per adapter. Rust and Lua get it free (Lua calls
`conformance::drive`); C already has `kui_env_set_window`; **Node exposes no
env at all**, so it needs a `Ctx` method first, which is the real cost and
also a gap in its own right — a JSX app cannot see `env.window` the way a
Lua script can. The `chrome` scene's `Expect` (solid quads, access rows) then
moves, and the three window buttons start being compared byte-for-byte across
all four bindings, which is what the claim has been promising.

Worth doing with P3 (Lua imperative focus): both are "the binding cannot
reach a thing the core has", and the env surface is the smaller half.

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

### `~` C6 — `selected` and `expanded` on the access tree — **done (2026-09-04)**

Shipped in `5c842f4`; the heading went unmarked, which is why this note is
later than the work. `selected` (`P_SELECTED`) and `expanded` (`P_EXPANDED`)
are rows, so all four bindings got them free and the parity test forced the
`KuiSpec` fields. `AccessNode` carries both plus `pos_in_set` / `set_size`,
and `access_bridge.rs` maps all four onto AccessKit's `set_selected`,
`set_expanded`, `set_position_in_set` and `set_size_of_set`.

Two decisions worth keeping:

`pos_in_set` / `set_size` are **derived, never declared** — the core counts
the semantic children a `list` / `tabList` already has. That was the option
this entry called "fits the design better", and it means an app cannot get
the count wrong by hand. The count lands on the container and the ordinal on
the item, following AccessKit rather than ARIA's `aria-setsize`-on-every-item.

`selected` is `None` unless the view sets it, rather than defaulting to
`false` on every selectable role — "not selected" on all seven rows of an
ordinary list is noise a reader has to wade through.

Still out of scope, as planned: live regions (a side channel, not a tree
property) and `required` / `invalid`.

The original finding:

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

### `.` C7 — Decide the multi-window story on paper — **done (2026-09-04)**

`docs/adr/0004-multi-window.md`, accepted. kui is multi-window; a window is a
`Core` (not a root in a shared one), since `Core` already owns every per-frame
singleton a window has — viewport, scale, `Env`, focus, the modal scope, the hit
list, the display list — while the four expensive shared halves (`Resources`,
`TextSystem`, `GlyphAtlas`, `AudioStore`) hoist into a `Session` a `Core` is
built against, so `Core::new()` and every headless path stay as they are. A
window's **existence is declared** and diffed the way `window_title` and `modal`
already are, edge-triggered like `set_key_focus` so an OS close is not undone by
the next frame; its **geometry stays a command** (`SetSize`, `Focus`), because
the user owns a window's size once it exists. `UiEvent` gains `window` — one app
keeps one `update`, and `origin` is not reused, since it answers which
*frontend* drew the node and an extension draws into every window. An OS popup
is `WindowKind::Popup`: borderless, owned, anchored to a rect `on_layout`
already reports, non-activating so opening a combobox does not blur its field,
and dismissed by the same `{kind:"dismiss", reason}` ADR 0003 gave a modal — so
graduating a dropdown from a `modal` float to a real surface changes the
declaration and not the handler. Modality is per-window (a modal cannot make
another window's hit list inert, and should not: a modal that freezes every
window is a hung app — ADR 0003's own reasoning for keeping chrome live). No
entry point changes shape: `kui::app().run()`, `kui_run` and one `runWindowed`
promise all stay, with `view` called once per live window. Two named C breaks
when it is built, neither reaching the other three bindings:
`kui_take_window_commands` stops being a `uint32_t` array (a source break —
hosts edit their drain loop), and `KuiEvent` gains an appended `window`
(source-compatible, but the struct is caller-allocated, so every C host
recompiles). An `Open` carries no title — a new window's own first frame
declares one through `window_title` — so `WindowCommand` stays `Copy` and
pointer-free.

Nothing shipped, which is the point — the ADR exists so the next twelve
additions stop assuming one window. `FloatConfig::fit` now documents itself as
the in-window approximation and names what it cannot do. The build work is C11.

The original finding:

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

### `.` C8 — Extend the paint vocabulary — **done (2026-09-04)**

Decided and recorded in `docs/adr/0005-the-paint-vocabulary.md`, and amended
there when the fourth was built. Three of the four shipped, one was declined.

**Shipped.** `opacity` is a `VisualStyle` slot inherited multiplicatively in
`finish_frame`'s existing clip pass and multiplied into the alpha of every
quad a subtree emits (scrollbars and the focus ring included); it eases, and
joins `keyframes` and `enter`, so `enter: { opacity: 0 }` fades a whole panel
in. Shadows are a new `QuadKind::Shadow` plus one `blur` on the quad — not a
nine-slice: the core emits the shape already offset, spread and inflated, and
the shader ramps the `sd_rounded_box` it already has, so a shadow is one more
instance in the same draw call. Six plain `PROPS` rows (so three of four
bindings got them free), seven appended `KuiSpec` fields, and both props are
in the `layout` conformance scene. Benched: the extra `f32` on `Quad`
(104 → 108 bytes) is invisible above run-to-run noise, and a new
`frame_10k_rects_with_shadows_and_opacity` (20k quads, every node casting)
costs about what the plain 10k-quad frame costs.

**Declined.** No gradients in v0 — a stop list, a type, a geometry and an
interpolation space are not a paint prop's worth of work, and the README now
says so instead of leaving a reader to infer it.

**Shipped after the design.** Exit animations are `crates/kui-core/src/depart.rs`
and one `exit` schema row (`Kind::Enter`, so an exit needs no parse code — it
is an `Enter` read the other way). A departing subtree is copied into a
`DepartStore` keyed by `Key`, frozen at the rects it left with, replayed inert
(no hit region, no Tab ring, no access row) on top of everything and outside
every clip, and dropped when its transition ends, when the key returns, or
when the store's 512-node budget refuses it — which vanishes it, exactly what
a node without an `exit` does, and says so with an `exit-budget` warning.

Two things changed from the design, both amended into ADR 0005. The subtree
is copied out of the **previous frame's tree**, kept by swapping the two tree
buffers instead of clearing one, and only when the frame that ended declared
an `exit` — the option the ADR had rejected as "doubling the retained frame
state", which conditionally it does not. And a ghost **eases itself** with
one lerp rather than an `AnimStore` tween, since nothing can retarget a node
the view has stopped talking about.

The open question — what a ghost does to `animating()` — was decided against
`examples/rust/toasts.rs`, which now enters and exits: it stays honestly true
while a ghost is in flight, and what bounds the frames that costs is `exit`
being opt-in per node plus the node budget, not a cap on the duration (which
would be a second, inconsistent rule for a declaration `transition` already
honours). Benched: `frame_10k_rects` pays about 5% for `NodeSpec` growing an
`Option<Enter>` and nothing else; one exit in a 10k-node frame costs under 2%
more; a full 512-node store replays in 12 µs a frame.

Also still open, and priced in ADR 0005's consequences: **rounded clipping**.
`Quad::clip` is a rect, so a rounded scroll container does not round its
children's corners. Four more floats on the hot struct plus a second SDF per
fragment is a bigger bill than either of the two above.

### `.` C9 — `KeyUp` and key repeat — **done (2026-09-04)**

Took the one-shape route: both halves are `{kind="key"}` with a
`phase` of `"down"` / `"up"`, matching the `phase` a drag and a hover
already carry, so no binding grew a second event kind. `repeat: bool` was
already on `KeyPress`; `InputEvent::KeyUp` is the new half, routed through
the same `Core::route_key` the press goes through.

Two rules keep a key from sticking. **A key only comes up where it went
down**: the core holds the presses it actually delivered (`keys_held`), and
a release with no matching press — one pressed while an editor held focus,
one already let go of — resolves nothing. **Focus moving lets go first**:
`set_focus` releases everything held to the sink that took the presses, in
press order, before focus lands. `Core::release_held_keys` is the same
thing for a driver whose window lost the keyboard, where the OS sends no
release at all (the winit runner calls it on `Focused(false)`; C hosts get
`kui_release_held_keys`).

The core normalizes a release — no `text`, never `repeat` — so four drivers
cannot disagree. C gained `kui_input_key_down` / `kui_input_key_up`, which
also closes the gap P6 found (a C host could not drive an `on_key` sink at
all); Node gained `ctx.keyUp` and a `repeat` argument on `ctx.keyDown`;
Lua takes the payload as-is. `KeyCode::from_name` is the shared parser
behind all of them.

Cost to apps: a handler that treated every `kind="key"` as a press now
fires twice, and filters on `phase == "down"` — one line in each of
`modal_editor`, `splitmux` and `syntax_view`. Still not covered: physical
scancodes and left/right modifier identity, so a keymap binds a character
and not a position on the board.

### `.` C10 — Flex wrapping — **done (2026-09-04)**

Wrapping shipped as two plain schema rows, `wrapChildren` and `crossGap`, so
all four bindings got it from the table. The name is not `wrap`: that one is
already the text prop that picks where a line breaks inside a paragraph, and
the two meet on `<edit>`, which takes container and text props at once.

**Rows only, and that is structural rather than unfinished.** Breaking needs a
definite main size, and the five passes hand a *row* one at exactly the right
moment — its width is final in pass 2, one pass before the cross-axis fit in
pass 3 that has to sum the lines. A column is the mirror image and does not
work: its main size is not resolved until pass 4, two passes *after* the fit
that would need the lines, and its cross-axis grow (pass 2) would be sizing
children against a container whose columns do not exist yet. `wrapChildren` on
a column, or on a `scrollX` row (an unbounded axis has nothing to break
against), lays out as if it were absent and raises `diag::WRAP_IGNORED` —
silence there would read as "wrapping is broken". Column wrap wants a sixth
pass, or a re-measure; C12 carries it.

The two rules worth knowing: **wrapping answers overflow before shrinking
does**, and greedy breaking guarantees the only line that can still overflow
holds a single child too wide for the box — so `shrink_axis` grew an
`only_line` filter and runs on that line alone, instead of squeezing chips that
are comfortable on other lines. And **lines share the container's leftover
cross space equally** (CSS's `align-content: stretch`), which is what makes a
wrapping row that happens to fit on one line lay out *identically* to an
unwrapped one — `a_row_that_fits_lays_out_exactly_like_an_unwrapped_one`
asserts exactly that, node for node, so the flag is safe to leave on.

Twenty-one tests in `crates/kui-core/tests/wrap_layout.rs` against a
deterministic `TextMeasure` stub, a `wrap` scene in the corpus that all four
bindings reproduce byte for byte, and a bench pair
(`frame_10k_chips_wrapped` / `_unwrapped`): 1.24 ms against 1.06 ms for 10k
chips in 100 rows that each break into several lines — the worst case, and the
same cost as the existing `frame_10k_rects`. Nothing that does not wrap pays.

### `.` C12 — Wrapping a column

C10 shipped rows and says why a column cannot follow in the current pass order.
Doing it means one of: a sixth pass (break columns after `grow_heights`, then
re-run the cross-axis fit and grow for wrapping containers only), or making a
wrapping column's cross size definite by fiat (only `Fixed`/`Grow`/`Percent`
widths wrap, `Fit` warns). The second is cheap and covers the real case — a
column with a definite height in a definite-width parent — but leaves
`width: grow` children inside it sized against the container rather than their
column, which is the half that actually needs the extra pass.

Nobody has asked for it. Wait for a view that wants it, and let that view say
which of the two is enough.

### `.` C13 — `space-between` / `around` / `evenly`, and baseline alignment

Two `Align` variants, and neither needs a new pass.

`Align::SpaceBetween` / `SpaceAround` / `SpaceEvenly` on `main_align` change one
expression in `positions`: today the free space becomes one offset before the
first child, and these spread it between them instead. A grow spacer already
covers `space-between` (`<box width="grow"/>` between two children), which is
why this stayed low priority — but it does not cover `space-around`, and it
costs a node. With wrapping in, they apply per line, which is what CSS does.

`Align::Baseline` on `cross_align` is the one with a real dependency: a line's
baseline is the max ascent of its children, and the core does not carry an
ascent per node — `TextMeasure` returns a `Size`. It needs a third number out
of measurement (text nodes have one; a container's is its first baseline-y
child's, and a box with none falls back to its bottom edge, per CSS). Worth it
for the case it fixes: two text sizes on one row sit on different lines today,
which is visible in any label-plus-value row.

### `.` C14 — Aspect ratio

"Square", or "16:9", without knowing either dimension. `Sizing::Aspect(f32)` on
one axis (the other resolves first, then this multiplies it) is the smaller
change and reads like the rest of `Sizing`; a separate `aspect` clamp applied
beside `min`/`max` is the more CSS-like one and composes with `Grow`. Images
already do half of it — a `Fit` height on an `<image>` preserves the intrinsic
aspect against a final width (`fit_heights`), so the machinery and the pass
ordering are proven; this generalises it to a declared ratio on any node.

### `.` C11 — Build multi-window

The work ADR 0004 (C7) decided but did not do. Each step ships alone, in order:

1. **`Session`.** — **done (2026-09-04)**. `Resources`, `AudioStore` and
   the font database moved out of `Core` and behind a `Session` a `Core` is
   constructed against (`Core::new_in`); `Core::new()` is sugar for a private
   session of one, so nothing headless changed — `cargo test --workspace`,
   the corpus across all four adapters, the C example and `npm test` all
   passed with no test edited. `kui-wgpu` grew `Gpu`: instance, adapter,
   device and queue behind one cloneable handle, with `Renderer::new_in`
   beside the unchanged `Renderer::new`.
   **Two of the four stayed**, and ADR 0004 decision 2 is amended to say so.
   The shaped-text cache and the glyph atlas are one unit with a window's
   texture — `CachedText` stamps its positioned glyphs with the atlas epoch
   they were packed against, so a cache entry is only valid for the page it
   was built from — and the atlas cannot move at all while `Core::output`
   returns `(&DisplayList, &mut GlyphAtlas)`: from a shared `RefCell` that
   half can only be a guard, a guard has a destructor, and the borrow then
   outlives its last use (`crates/kui-core/tests/subpixel.rs` binds the pair
   and touches the core again). `Core.atlas.epoch` / `.size`, read as plain
   fields, fail the same way. So hoisting the atlas is a source break; it
   waits for step 3, which already breaks C. `TextSystem` was split rather
   than moved: the font database is the session's and arrives as a `&mut
   FontSystem` parameter, while the cache, the rasterizer and the per-frame
   text list (`TextId` indexes it, and another window's `begin_frame` would
   clear it) stay the window's. What is left duplicated per window is memory
   and repeated rasterization, not correctness: the handles that must agree
   across windows — `FontId`, `ImageId`, `SoundId`, and the one audio queue
   — all resolve against the session.
2. **`WindowId` plumbing.** `UiEvent.window`, `WindowEnv::id`, `KuiEvent.window`
   (appended), the Node event object and the TS message types. Always 0 until
   step 3, so it is pure plumbing that can land and be reviewed on its own.
3. **The declared set.** `declared_windows` / `declared_windows_last` beside
   the focus pair, the diff into `WindowCommand::Open`/`Close(WindowId)`, the
   `{kind:"window", phase}` event, and the runner opening real `Normal`
   windows. `kui_take_window_commands` becomes a `KuiWindowCommand`
   out-param here — a command now carries a `window` beside its verb, which
   no longer fits a `uint32_t` — and C hosts edit their drain loop. Keep the
   struct pointer-free: an `Open` carries no title (ADR 0004 decision 5), so
   `WindowCommand` stays `Copy` and no borrowed string enters the drain.
   Two rules the ADR's 2026-09-04 amendments added (A5) land in this step,
   because both live in the diff:
   - **Config is read on the opening edge only**, and on that edge the
     lowest declaring `WindowId` wins — main is 0, so it wins whenever it
     declares — with the first declaration winning within a single frame.
     A live window's config is never re-read, which is where decision 5's
     "the user owns geometry once the window exists" is actually enforced:
     not by rejecting a re-declared size, but by never looking at it. A
     conflict on that edge — differing configs, since a config is plain
     data and equality is derived — raises `duplicate-window-config`, and
     the pick stays deterministic, so the warning reports the bug without
     deciding anything.
   - **`window-declared-while-closed`**, when a name is still in the
     declared union on the frame after an OS close it never lapsed across.
     This is the app that declares a window unconditionally and cannot
     reopen it, which is the first version anyone writes; the warning is
     what keeps the edge rule from reading as kui ignoring `windows`.
   Both are raised from the diff, not from `diag`'s tree walk, and keyed by
   the window name rather than a node (the way `unknown-prop` is keyed by
   element and name) — so they go in `diag.rs` beside `modal-behind-content`,
   in the TS warning-code union, and `diag`'s module doc stops saying *one*
   code comes from outside the walk. `window-declared-while-closed` is
   raised on the core whose frame declared the name, so it inherits the
   per-`(code, key)`-per-core dedup and costs one line however many frames
   the app keeps asking.
4. **`WindowKind::Popup`.** Anchoring in screen coordinates, ownership,
   non-activating focus routing, and `dismiss` on the window.
5. **`SetSize` / `Focus`**, queued the way `reveal` and `play` are.

Testing splits the way the ADR says: the conformance corpus can pin the
declaration diff (declare a window, stop declaring it, assert the command
sequence — all four transports reproduce it byte-identically), but a popup
window has no headless equivalent, so its behaviour belongs in P8's
macOS/Windows smoke jobs. Both step-3 warnings fall on the corpus side of
that line: a config conflict is two declarations in one frame, and the reopen
trap is a declaration, an injected close and a frame that still declares —
neither needs an OS surface, and `take_warnings` is already what headless
tests assert on.

---

## Repetition

Four bindings over one contract means some parallel code is the price of the
design. These are where the price is paid without the benefit.

### `.` D1 — Collapse `Ctx` and `KuiWindow` with a macro — **done (2026-09-03)**

Shipped in `935fa66`; the heading went unmarked, which is why this note is
later than the work. `#[napi]` does work inside a macro body — the
load-bearing assumption — so `core_methods!` now generates **39 methods** per
class from one definition list, invoked twice at the bottom of
`crates/kui-node/src/lib.rs`.

The macro takes four knobs rather than the two this entry guessed at:
`core`, `events`, `redraw` (`no_redraw` / `request_redraw`) and `audio`
(`no_flush` / `flush_audio`). The extra two are the real difference between a
headless context and a live window, and naming them made it visible that the
audio flush had been a `KuiWindow`-only behaviour all along.

`crates/kui-node/src/lib.rs` went **1791 → 1529 lines** while *gaining* the
context-menu, cursor, scroll, `KeyUp` and modal surfaces — so the collapse
paid for five features' worth of new API and still came out shorter.

The third copy in `index.d.ts` is untouched; that is P5, still open.

The original finding:

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

### `.` D2 — Route Node's JSON lowering through the encoder — **done (2026-09-04), the other way**

The proposal was to parse JSON in JS and feed the existing encoder. What
shipped is simpler: the JSON and object transports were **deleted**, so
there is no second lowering to route anywhere. `frameObject` / `frameJson`
/ `setViewObject` / `setViewJson` and the `transport: 'json'` option are
gone, `lower` / `lower_element` / `lower_root` with them, and
`binary.rs`'s dispatcher is the addon's only one. The `TreeApp` view is a
`(Vec<f64>, Vec<u8>)` rather than a two-variant enum.

Nothing needed the debug reference: the transports existed to check each
other, and the corpus scenes (P7) had already taken over that job against a
report `kui-core` generates — a stronger oracle than a second hand-written
dispatcher, because it says what the frame must *be* rather than only that
two implementations agree.

Error messages came out ahead, not level. They now all come from the
encoder, in JS, before anything crosses the boundary, and they name the
value: `bad dir "diagonal" (row | column)` and `bad value "middle" for
mainAlign (one of start | center | end)` where serde's arm said only that
the prop was wrong. The one message the JSON dispatcher had and the encoder
did not — "element without a type — did it come from kui/jsx-runtime?" —
was ported over; without it a `{props, children}` object read as `unknown
element <undefined>`.

What replaced the parity test is the part worth arguing with. `assertParity`
compared three transports byte-for-byte; with one transport left, the tests
that used it would have become "it did not throw". They now check that
**every schema prop reaches the stream** — declaring a prop must change the
encoded bytes, since every prop writes at least its own id — and then that
the stream lowers without desyncing the decoder. That is the failure the
JSON path used to catch (a `switch` arm nobody wrote, a name the encoder
falls through on) and it catches it for `msg`/`tag` props too, which the old
quad comparison could not see.

That argument was checked rather than assumed. 22 single-edit mutations of
`encoder.js` — swapped slots in every hand-written composite, wrong mode
numbers, dropped flags, swapped element operands — were run against both
suites, the old one rebuilt from the previous commit:

| | caught |
| --- | --- |
| old, three transports + `assertParity` | 15/22 |
| new, one transport | 15/22 |

The sets are now identical. They were not at first: the old suite caught
`latencyHud`'s `at` operands being swapped and the new one did not, because
`assertParity` compared an asymmetric `at: ['start', 'end']` across two
implementations while nothing else looked at where the HUD landed. The
elements test now asks that directly (`at[0]` places it horizontally,
`at[1]` vertically), which is what closed the gap. Worth noting that the
obvious differential — encode `['start','end']` and `['end','start']` and
require them to differ — does *not* catch a swap, being symmetric itself.

Seven mutations neither suite caught, all predating this change: `float`'s
`dx`/`dy`, its `at`/`self` alignments and its `fit` flag; all three `sizing`
mode confusions; and `padX` falling back to `padY`. The cause was symmetric
test data — the float scene attached at `at == self_at` with `dx == dy`, and
the generic prop loop samples `50%` inside a *fit-sized* parent, where a
percent and a grow resolve to the same geometry. **Closed in the corpus
rather than in Node** (see P7 below): the sweep is now 22/22.

### `.` D3 — Lift composite parsing into `kui-core` — **done (2026-09-04)**

The decisions moved down; the extraction stayed up. `kui_core::spec` gained
`FloatConfig::build(base, at, self_at, dx, dy, fit)` — every override an
`Option`, so a piece a frontend did not see leaves the base's own value —
`FloatConfig::preset` / `preset_at` over a shared `FLOAT_PRESETS` table,
`PadShorthand` with its `resolve`, `NodeSpec::overflow_bits` over shared
`OVERFLOW_*` constants, and `NodeSpec::apply_tooltip` /
`PropsOut::apply_tooltip`, which owns all three of a hint's effects so no
binding can implement two. All four bindings call them; what is left in
each is reading its own value type.

Three things fell out of stating a rule once. **`anchor` takes any preset**,
not just `parent` / `viewport`, in JSX, Lua and the binary stream, and
because an undeclared override keeps the base's value,
`{ anchor: "below", dx: 4 }` hangs below with its 6px gap — the shape that
previously needed the whole config restated. **C reaches the presets by
name** through `kui_spec_float_preset(&spec, KUI_STR("below"))`, which fills
the `float_*` fields and leaves them writable, replacing the five hand-
spelled lines the conformance adapter itself had. And the **binary protocol
is v4**: the pad shorthand and each float offset ride the wire as declared,
behind a set mask, so the JS encoder stopped deciding what `padX` falls back
to as well.

P7's corpus was the check that mattered, and it paid immediately: the
`float` scene grew a preset-with-one-override and `layout` a box whose whole
size is what the pad fallback resolved to, and the JSON and binary paths
promptly disagreed — the first cut gave `dx` and `dy` a shared "declared"
flag, so a lone `dx` flattened the gap. Nothing else in the suite noticed.
Seven unit tests in `crates/kui-core/tests/composites.rs` pin the rules
themselves, and `kui-ffi` round-trips every preset name through `KuiSpec`
and back.

This **reverses P2's "one spelling" call**: Lua's float table now accepts
`self` alongside `self_at`, and its `pad` table gained `all` / `x` / `y`.
P2's reasoning was that two spellings nothing pins are worse than one — but
that was about a docs/parser drift with nowhere to catch it. Both spellings
are pinned now (the corpus adapter uses `self`, the Lua unit test still uses
`self_at`), and with the fallback rules in one core function the argument
for keeping the surfaces deliberately different is gone. `docs/props.md`
names `self`, which is what alpha.5 published.

The original finding:

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

### `.` D4 — Unify `createApp` and `runWindowed` — **done (2026-09-04)**

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

That is what shipped. `createLoop` in `packages/kui/index.js` takes the surface
and a clock; both drivers build one, and the two contracts above are the only
things left outside it. `update`'s fourth argument is the surface in both.

The clock is the half worth arguing with. A window reads the wall clock every
pump and, having fallen behind, resyncs to the cadence rather than firing a
burst of stale ticks. A test calls `app.advance(ms)` and gets the opposite —
every tick inside the span, because it asked for exactly that much time — plus
`setTime(now)`, so the frame clock behind `transition` moves with the ticks and
a transition can be stepped through headless. `startTime` pins the origin so
assertions on `tick.msg(now)` are exact. One policy flag (`catchUp`) separates
the two; everything else is shared.

`settle` / `access` / `accessTree` / `dispatch` / `render` / `step` are the
loop's, so `runWindowed`'s `setup(win, app)` hands them over for a real window.
Synthetic input is the exception, and stays one: D1 keeps input injection on
`Ctx` alone (a window's input comes from the OS), so `click` / `type` / `key`
ask the surface for the call and name the missing one instead of failing as an
undefined method.

---

## Documentation

### `!` X1 — Fix the `width`/`height` main-axis docs — **done (2026-09-04)**

`Dir::Column` is the default, so for the majority of nodes the old
"Main-axis size" / "Cross-axis size" named exactly the wrong axis. Layout
treats them as literal axes regardless of `dir` — `fit_widths` is horizontal
always — and `LayoutSpec`'s own field docs correctly say nothing about main and
cross. The two rows now read "Horizontal size" and "Vertical size", value list
unchanged, and `npm run gen` carried that into `docs/props.md` and the JSDoc on
`width` / `height` in `jsx-runtime.d.ts`, which is where every JS user reads it
on hover. The schema row being the only place the claim was made is why the fix
is three lines in one file plus generated output.

**The scan found nothing else, and the reason is worth keeping.** Every other
`doc` string that says "main axis" or "cross axis" names a prop layout really
does resolve against `dir`: `gap`, `crossGap`, `mainAlign`, `crossAlign` and
`wrapChildren` all branch on `Dir::Row` / `Dir::Column`, and `dir`'s own row
("Main axis; column is the default") is the definition. The horizontal/vertical
claims that remain — `shadowX` / `shadowY`, the four per-corner radii — are
literal and correct. `width` and `height` were the only two rows describing a
literal axis in relative terms, which is the mistake to watch for: main/cross
wording is right exactly where the code switches on `dir`.

### `.` X2 — Automate the CHANGELOG heading — **done (2026-09-04)**

The shipped heading had already been corrected by hand before this ran:
`## 0.1.0-alpha.5 (2026-09-03)`, the tag's own date, with
`## 0.1.0-alpha.6 (unreleased)` opened above it. What was missing was the
machinery that stops the next one shipping stale, and that is what landed.

`set-version.sh` gained a second sed pass, over `CHANGELOG.md`, in the style of
the one it already runs over `Cargo.toml`:
`1,/^## /s/^## .+ \(unreleased\)$/## <ver> (<today>)/`. The range ends at the
first `## `, so no released section below can be rewritten, and the
substitution is the heading line only — what a release adds and what you can
delete stay a person's to write.

`check-version.sh` gained one assertion beside the manifest ones: the first
`## ` line must name the version as a whole token and must not say
"unreleased", in any case. Run against the alpha.5 tag's tree it reports
`CHANGELOG.md top heading is "## 0.1.0-alpha.5 (unreleased)"` and exits 1 — the
hole that produced this item is now what the guard catches, in the release
job (`Tag matches the manifests`) that already refuses a mismatched tag.

**Opening the next `(unreleased)` section stays manual**, deliberately. The
script cannot know what the next version will be called (alpha.7? beta.1?
0.2.0?), and a stub opened automatically would fight the new assertion — the
top heading would then be the *next* section rather than the tagged one. The
failure mode of leaving it out is loud and lands on the person who can fix it:
cutting a release with no open section makes the sed match nothing, and
`set-version.sh`'s own `check-version.sh` call then fails naming the heading it
found, at which point writing the section is the obvious next move.

The change itself is not in the changelog: that file lists what an app gains
and what it can delete, and release tooling is neither.

### `.` X3 — List the missing input modes in Status / next — **mostly done (2026-09-04)**

Both halves landed. The section now names the layout gap and the benches
paragraph covers every frame bench: `frame_10k_rects_with_access_tree` and
`frame_10k_rects_with_shadows_and_opacity` as ratios against the frames they
extend, and C10 added the `frame_10k_chips_wrapped` / `_unwrapped` pair the
same way. All of them are ratios rather than rows because they were measured
on a different machine from the table's.

The flex-wrap line was written as "no flex wrapping" and then rewritten by
C10, which shipped it: what the section carries now is the remainder — a
column that cannot wrap, no `align-content`, no `space-between` / `around` /
`evenly`, no baseline, no aspect ratio, and `Dir` with no reverse. That is
the shape this row wanted, and a gap being closed between writing it down and
reading it back is the system working.

**Still open: no touch or pen input.** It does not reach the core at all, and
the section says nothing about it, so it reads as present. Add it in the
section's existing tone, grouped with the pointer paragraph.

The precedent, for whoever writes that line: modal containment was on this
list until C1 shipped it; the pointer buttons went on it with C2, which routes
only the secondary one; cursor shapes came off it with C3, and programmatic
scrolling with C4. C9 put key releases in the section as a *fixed* line and
left the real remainder there: no physical scancodes, no left/right modifier
identity, and unnamed keys dropped rather than delivered.

---

## From the ADR review (2026-09-04)

Reading ADRs 0003, 0004 and 0005 back against the code. All three describe
what they built accurately — every claim spot-checked held — and each ends by
naming what it left undone. Those named gaps are the first three items here.
The last three came out of re-running the guards rather than reading them.

### `~` A1 — ADR 0003's two named modal gaps — **done (2026-09-04)**

Both shipped. The corpus has a tenth scene, `modal`: a floated dialog over an
app with a titlebar, replayed through a press behind it (no click, one
`dismiss`), a press on the titlebar (nothing — chrome stays live), a press on
the dialog's own button (a click), three ring steps and Escape. Only the last
frame reaches the report, so the ring steps are picked to end where only a
scoped ring can — Shift-Tab, Shift-Tab, Tab over the dialog's two stops end on
Cancel, over the whole tree's three they would end on the button behind.
`Step` grew `tab`, `shifttab` and `escape`, and the Lua, C and Node adapters
replay them (C's step buffer now fails loudly instead of truncating at 16).
`modal-without-name` lives beside `control-without-name` in the same
`check_access` walk, triggered by the derived `Role::Dialog` on a node
declaring `modal`: a dialog is named by its `label` alone, so text inside does
not silence it. It caught both of our own context menus, which now carry one.

The last of the ADR's four is A3 below; A2 closed it on 2026-09-04.

The original finding:

The ADR's Consequences list four things it left. Two are small and verified
missing: **no modal scene in the conformance corpus** (nine scenes, none of
them modal — and the ADR is right that a scene would pin the *behaviour*, not
the mechanical lowering: the scoped ring, the inert hit list, live window
chrome, both dismiss reasons), and **no `modal-without-name` warning**, which
sits directly beside the `control-without-name` that already exists and uses
the same test.

### `~` A2 — A dialog cannot choose which control opens focused — **done (2026-09-04)**

`initialFocus` (id 77, a flag), the fourth of ADR 0003's named gaps. The
entry rule in `resolve_modal_focus` reads it: the first node in the modal's
Tab ring declaring it, and the ring's first when none does — so the ADR's
behaviour is exactly what a dialog that says nothing still gets. The
accessibility example's confirm now declares it on Cancel, which is what
`scripts/ax-audit.swift` is pointed at.

**`keyFocus` was checked first and does not serve**, for a reason worth
writing down rather than re-deriving. The edge does line up — the modal and
its controls start being declared on the same frame, so `keyFocus` on Cancel
does land focus there. What it also does is move focus *while the frame is
being built*, and `resolve_modal_focus` runs after: the modal then remembers
Cancel as the focus it displaced (decision 4), and when the dialog closes
that key is gone from the tree, so focus is dropped instead of returning to
the button that opened it. A dialog that opens on the right control and
loses the keyboard on the way out is not the fix. The new row is resolved
after the scope is known, which is why it composes with decision 4 instead
of fighting it —
`the_entry_does_not_disturb_the_focus_the_modal_gives_back` is that test.

Entry-triggered, not declaration-triggered: only focus *outside* the scope
reaches the branch, so a Tab press stands, a redeclaration is not a second
entry, and a nested confirm closing hands focus back into the dialog without
the dialog's own entry being read again. A declaration the ring skips
(disabled, `role="none"`, not focusable) is no candidate and falls back,
which is the same answer as declaring nothing.

### `.` A3 — Three ADRs have deferred arrow-key composites

ADR 0002 named it as its next step, ADR 0001's follow-ups touch it, ADR 0003
lists it as not-done-here. Three deferrals is the signal it wants its own ADR.
C6 helpfully moved it closer: the core now knows `selected`, `pos_in_set` and
`set_size`, which is what arrow navigation needs. The real question is what a
*roving tabindex* is in a data IR — today every tab in a tab list is its own
Tab stop, which is neither the platform pattern nor what a reader expects.

### `!` A4 — The C ABI has no version negotiation, and 0004 will need one — **done (2026-09-04)**

`docs/adr/0006-c-abi-versioning.md`, accepted and built, closing the gap ADR
0004 named in its own decision 12 and left open. Both guards shipped, because
they cover different failures.

`kui_abi_version()` versus `KUI_ABI_VERSION` is the universal one — compared
for equality before a host's first other call, and bumped only when something
the library *writes or allocates* changes layout, so a `KuiSpec` prop a
release does not make it noise. `mod abi_parity` emits a `_Static_assert` that
the header's macro is Rust's constant, so a bump made in one place and not the
other cannot make the runtime check pass on a mismatched pair.

The `uint32_t size` this entry called for is the stronger half, and it landed
on the four structs the library writes into host memory — `KuiEvent`,
`KuiDrawData`, `KuiTextMetrics`, `KuiScrollGeometry`, each with a `KUI_*_INIT`
that sets it. The library writes no further than the host reserved, so the
*next* append to one of them is compatible rather than fatal. A `size` below
the ABI-1 layout is refused rather than guessed at, and `kui_poll_event`
refuses before it pops, so a rejected poll does not swallow the event.
`kui_draw_data` returns `bool` so it can report a refusal like the other three.

**Where the size field does not reach**, which the audit turned up and the
header now says outright: the four array out-params (`KuiAccessNode`,
`KuiAccessRun`, `KuiWarning`, `KuiAudioCommand`) and the `KuiQuad` array a
host strides through `KuiDrawData`. There the *stride* is the problem — element
1 lands past the host's element 1 whatever element 0 says, before any in-band
handshake could be read. Growing one of those means an explicit stride
argument, a source break every host sees, not a quiet append. The version
check is their whole guard, stated rather than implied.

The asymmetry this entry asked to have written into the header is written into
it, and generalized: a "Who writes what" block plus an `[in]` / `[out]` /
`[out[]]` / `[lib]` tag on every struct. It was previously implicit — `KuiSpec`
was documented as "append-only: the layout is ABI" with nothing saying why the
same did not hold for `KuiEvent`. `mod abi_parity` gained `KUI_OUT_STRUCT`,
asserting that `size` leads each [out] struct and that none has fallen below
its ABI-1 layout, with the floor computed from Rust's own `OutParam` impl.

Verified end to end rather than by construction: grew `KuiEvent` by 16 bytes
in the library alone and ran a host that still reserves 24, with tagged bytes
behind the struct. With the handshake the tail is intact and `size` comes back
24; with it disabled the same host loses 16 bytes past the struct. The whole
binary does not crash either way — which is the argument for the field rather
than against it, and why this was `!`.

`examples/c/counter.c` checks the version in `main` before anything else, and
its `--headless` pass asserts the refusal. One C source break, at the
declaration only: `KuiEvent ev;` becomes `KuiEvent ev = KUI_EVENT_INIT;`.

The original finding:

ADR 0004 names this and explicitly does not solve it: "nothing catches an old
binary against a new library — the ABI has no version negotiation, and this
ADR does not add one." Verified: no `kui_abi_version`, `KUI_VERSION` or
equivalent anywhere in the header or the crate.

The reason this is `!` rather than `.`: **`KuiEvent` is caller-allocated**
(`kui_poll_event(ctx, &ev)` writes into memory the host reserved), and ADR
0004 appends a `window` field to it. An old host reserves the old size and a
newer library writes past it — memory corruption, silent, at run time. P6's
static asserts catch header-vs-Rust drift at *build* time and cannot see this.

Note the asymmetry worth writing into the header: appending to a *host*-written
struct (`KuiSpec`, which already grew `tooltip`) is safe; appending to a
*library*-written out-param is not. A leading `uint32_t size` the caller sets
turns that whole class of future appends from breaking into compatible.

### `.` A5 — Two cases ADR 0004 leaves undefined — **done (2026-09-04)**

Both settled on paper, before anything was built, as amendments to
`docs/adr/0004-multi-window.md`. The build work inherits them through C11.

**Config under the union rule** is now three rules in decision 4, none of which
needs a frame order — which was the whole point, since a frame order is what
the union exists not to need. A config is read **only on the frame its
declaration starts**, so decision 6's edge governs the config and not just the
existence, and a live window's config is never re-read (a frame re-declaring
`"palette"` at 400×300 cannot resize it — decision 5's "the user owns geometry"
falling out of the diff rather than being enforced beside it). That narrows the
conflict to the opening edge without removing it, so on that edge the **lowest
declaring `WindowId` wins**: ids are fixed before any of the frame's views run,
so ordering over them is order-independent in the way ordering over execution is
not, and `WindowId::MAIN` being 0 means the main window wins whenever it
declares — the answer an app would guess. Within one frame declaring a name
twice, the first wins, which needs no rule at all. A genuine conflict — configs
that differ, since a config is plain data and equality is derived — is a
`duplicate-window-config` warning; two windows declaring the same config
identically is the ordinary case and says nothing.

**The reopen trap** stays, because the alternative is a close button that does
nothing, and gets a name instead: `window-declared-while-closed`, added to
decision 6. It fires when a name is still in the declared union on the frame
after an OS close it never lapsed across — the app that declares a window
unconditionally, which is the first version anyone writes. Next to
`modal-behind-content` and for its reason: a silent misconfiguration that reads
from outside as the feature being broken, so the message says the fix (handle
`{kind:"window", phase:"closed"}`, stop declaring, declare again to reopen)
rather than the symptom.

Both warnings are raised from the declaration diff rather than `diag`'s tree
walk and are keyed by the window name, since neither has a node — so `diag.rs`'s
module doc, which says one code (`unknown-prop`) does not come from the walk,
becomes three when this is built.

The original finding:

Cheapest to settle now, while nothing is implemented.

**Config under the union rule.** Decision 4 makes the window set "the union of
what every live window's frame declared", chosen because it is
order-independent. Existence is; *configuration* is not. Two windows declaring
`"palette"` with different kinds or sizes leaves the resulting `Open`'s config
decided by whichever frame ran last — the non-determinism the union was picked
to avoid.

**The reopen trap.** Decision 6's edge rule means an app that declares a window
unconditionally can never reopen it after the user closes it: the declaration
never stops, so it never starts. That is the correct design and the same shape
as "the core closes no modal" — but it will read as a kui bug the first time
someone hits it, and the library has a mechanism for exactly that. A
`window-declared-while-closed` warning belongs next to `modal-behind-content`.

### `~` A6 — The corpus can skip, and its coverage is self-declared

Two soft spots in the guard that P7 built, found by running it rather than
reading it. The corpus itself is real — mutation-tested: removing
`.description(hint)` from `spec_of` fails the C adapter with the exact scene,
line and diff, exit 1.

**It can skip.** `packages/kui/test.mjs` skips the scene test when
`KUI_CONFORMANCE` is unset. The reason is sound (the publish job has no cargo
target dir and still needs the rest of the suite), but nothing asserts the
adapter *ran* in the job where it should — a broken `$GITHUB_ENV` propagation
would take the check away silently. The C adapter takes the path as an
argument and fails hard, so the hole is Node-only.

**Coverage is declared, not measured.** `the_corpus_covers_every_hand_written_row`
is bidirectional and genuinely good, but a scene carrying `custom: &["float"]`
is never checked to actually exercise float. Deriving the claims from the built
tree (a scene claiming `float` must produce a node with `layout.float.is_some()`)
would make a stale claim fail.

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
   describe. ~~P4 (unknown-prop warnings) and D3 (composite parsing in the
   core) are the two that still shrink the surface it has to cover.~~ Both
   shipped (2026-09-04), and both leant on the corpus: the "every element
   lowers" scenes are what pin P4's allow-list, and D3's refactor was
   caught on the way in introducing a JSON/binary disagreement.
6. **Then the designs.** ~~C1 + C2 unlock dialogs, menus and comboboxes
   together~~ — both shipped (ADR 0003 for C1; C2 needed no ADR of its own,
   since it only adds a row and a button to the model 0003 settled), and a
   context menu is now an `onContextMenu` tag plus a `modal` float. ~~C7 is
   the one to decide on paper now and build later, before more API assumes
   a single window.~~ Decided (ADR 0004): a `Core` per window, a declared
   window set, `window` on the event, popups as a window kind. The build is
   C11, and its first step (a `Session` for the shared caches, one wgpu
   device) is worth doing early even alone — it is invisible to the
   bindings and it is what every later step sits on.
