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
  --features conformance --example conformance-dump -- target/conformance.txt`) and never checked
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
implicit root spec exactly, so only `title` actually lands. ~~**Window env**:
nothing exposes `env.window` to a bare `Ctx`, so `windowButtons` draws
nothing headlessly and its scene pins the lowering rather than the pixels;
closing that needs a window-env setter on the Node `Ctx` first.~~ Closed by
B2 (`ctx.setEnv`) and P9 (the scene's `env` line), 2026-09-04: the `chrome`
scene drives under custom chrome and the buttons are compared as pixels.

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

### `~` P9 — Let the corpus drive a frame under custom chrome — **done and independently verified (2026-09-04)**

A scene declares the window facts it is driven under. `Scene` gained an
`env: WindowEnv` (`NATIVE_CHROME` for every scene but the two about chrome),
`conformance::drive` takes it and assigns `core.env.window`
**before the first frame** — which is the whole of why it is a parameter and
not something a driver pushes afterwards: `titlebar_with`'s inset and
`window_buttons`'s early return are read while that frame builds. It travels
to the other three adapters as a new report line,

    env <customChrome> <maximized> <fullscreen> <controlsW> <controlsH>

written only when a scene departs from `NATIVE_CHROME`, so every scene that
is not about chrome carries no line and an adapter that sees none drives
under the defaults it already had. The five numbers are `kui_env_set_window`'s
arguments in its order, which also settles the shape question: the controls
rect travels as a `w`/`h` extent at the window origin rather than a free
rect, because that is what all four bindings can express. Each adapter reads
it back the way it already reads the `step` lines — C `sscanf`s it and calls
`kui_env_set_window`, Node parses it and calls `ctx.setEnv({window})`, Lua
gets it free through `conformance::drive`.

**`conformance::UNDERIVED` is now empty**, which was the point. The constant
stays, with its doc rewritten to say so and `the_underived_rows_are_real_and_still_underived`
still standing over the next exemption — an empty list is a state to hold,
not a constant to delete.

**The six-buttons question below is settled the second way**, and the reason
is in `schema::ELEMENTS` where it always was: `windowButtons` is "just the
min/max/close buttons, **for fully custom titlebars**". It is not meant to go
inside `<titlebar>`, which appends its own cluster by contract
(`titlebar_with`'s doc: content goes "between the platform inset and the
window buttons"). The scene was written when the element drew nothing, so
calling it inside the titlebar was the only way to claim the row and cost
nothing visible. The `chrome` scene now builds **both** forms — a
`titlebar_with` whose appended cluster is the adaptive path, and a hand-laid
plain row holding a second cluster through each binding's own
`windowButtons` element — so six buttons is the expectation, in two clusters,
and the element is exercised as an element rather than only transitively.
The strip is a plain row, not a second `window_drag`: a drag handle would
derive a second `titleBar` role, which is a thing to tell a screen reader
rather than a side effect of where the corpus put a box.

Mutation-tested, each failing at the right layer: the `chrome` scene reverted
to `NATIVE_CHROME` fails `every_scene_delivers_the_coverage_it_claims` with
`claims schema::ELEMENTS rows its builder does not exercise:
["windowButtons"]` — the claim is no longer vacuous; Node's `driveScene` not
calling `setEnv` fails the corpus test on the `chrome` block; and C's
`conf_run` skipping `kui_env_set_window` fails with `quads 4` against the
reference's `quads 10`.

**The traffic-lights inset got its own scene** (`chrome-inset`, added the
same day). A controls rect makes `window_buttons` return early, so one scene
cannot show both halves — but it costs no second builder: `chrome-inset` is
the *same tree* under `CUSTOM_CHROME_INSET` (custom chrome plus the real
78x28 rect `kui::MACOS_TRAFFIC_LIGHTS` reports), and the adapters point their
existing chrome builder at the new name. The env being the only difference is
the point: both clusters go away and the title moves from the bare 12pt
margin out to the controls' right edge.

That leaves one term the corpus provably cannot reach, found by mutating the
widget rather than assumed: `titlebar_with` insets by `r.x + r.w`, and
changing it to `r.w` **does not move the digest**, because the protocol
carries the controls as a `w`/`h` extent at the origin (what
`kui_env_set_window` can express) and `r.x` is therefore always 0 in a scene.
An inset wrong in the ordinary way *is* caught — a five-pixel error changes
`chrome-inset`'s digest at an unchanged quad count. The `r.x` term is covered
instead by `titlebar_insets_past_the_native_controls` in
`crates/kui-core/tests/window.rs`, which asserts 12 / 78 / 82 across the
widget's two branches; the report carries no coordinates, so a position is
not something a checked-in `Expect` can hold.

**Verified from outside the session that wrote it**, since the failure this
item existed to fix was a claim that passed while building nothing, and a
second such claim would look exactly like a green run. Four adapters over the
regenerated reference: `cargo test --workspace` (Rust and Lua), the C example
against `target/conformance.txt` (`conformance OK (12 scenes)`), the dlopen
extension, and `npm test` under `KUI_CONFORMANCE_REQUIRED=1` — 51 tests, none
skipped, which is the point of the flag. All three mutations named above
reproduce, each still failing at its own layer.

**The "other nine scenes" question was settled by measurement rather than by
scoping**, which is the stronger of the two options the item offered. A
worktree at `b2e9e6d^` dumped the pre-P9 reference, split both reports per
scene and diffed: `layout`, `sizing`, `wrap`, `overflow`, `float`, `tooltip`,
`controls`, `media`, `modal` and `exit` are byte-identical, `chrome` moved,
`chrome-inset` is new. That result is structural rather than lucky — a scene
equal to `NATIVE_CHROME` writes no `env` line at all, so an adapter that sees
none drives under the defaults it already had, and the only way to disturb a
non-chrome scene is to give it an env it did not ask for. Worth repeating the
same way if `drive` ever grows a second declared fact: the reference report
makes the check cost one dump and a diff.

The original finding:

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
`conformance::drive`); C already has `kui_env_set_window`; ~~**Node exposes no
env at all**, so it needs a `Ctx` method first, which is the real cost and
also a gap in its own right — a JSX app cannot see `env.window` the way a
Lua script can.~~ Node has `ctx.env()` and `ctx.setEnv()` as of 2026-09-04
(B2), so every adapter can now declare it. The `chrome` scene's `Expect`
(solid quads, access rows) then moves, and the three window buttons start
being compared byte-for-byte across all four bindings, which is what the
claim has been promising.

One thing to settle while moving that `Expect`, found by driving the scene
from Node under a declared custom chrome: `<titlebar>` with an explicit
`<windowButtons>` child renders **six** buttons, because `titlebar_with`
calls `window_buttons(ui)` after the content regardless. The `chrome` scene
declares exactly that, so the corpus will pin six the moment the `Expect`
moves. Either the explicit element should suppress the implicit cluster or
the scene should stop declaring both; deciding that is part of P9.

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

**Rounded clipping** was left open here, priced in ADR 0005's consequences
at "four more floats on the hot struct plus a second SDF per fragment" — a
bigger bill than either of the two above. That was an estimate; it has been
measured and it is **done (2026-09-04)**, amended into ADR 0005.

A node that clips (`clip`, `scroll_x`, `scroll_y`) and has a `radius` now
rounds the clip its descendants inherit, which is CSS's rule and therefore
**no new prop in any of the four bindings** — the radius is the clipping
node's own. `Quad` grew `clip_radius: [f32; 4]` (108 → 124 bytes) and
`display::Clip` is the rect-plus-radii pair the propagation pass, `Paint`
and the emitters carry; the shader takes one `sd_rounded_box` *instead of*
the rect test on the branch where the radii are not all zero, which is every
quad of every frame that has no rounded clipper.

Benched with the same bench file on both sides, two copies of the baseline
binary in the same rounds to floor the noise at ±0.6%: about **1%** on every
10k-node frame whether or not it clips (the bigger `Quad`), about **3.5%**
on a frame where 100 rows clip (a `Clip` is 32 bytes where a `Rect` was 16),
and the per-corner intersect itself is the last **0.9%** on top of that — in
the shape a real view never declares, since it rounds the card and not each
of its rows. The four floats were priced against a one-float uniform
`clip_radius` first: indistinguishable, so the generality was free.

What it does not do, deliberately: hit testing and culling stay rectangular,
and nesting two rounded clippers keeps only the corners neither of them
moved (an ancestor edge cutting partway into a rounded corner leaves a
sliver unclipped). `KUI_ABI_VERSION` went 1 → 2, since `KuiQuad` is a
**[lib]** struct.

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

### `~` C15 — `NodeSpec` was 728 bytes and the frame got ~2.5× more expensive — **fixed (2026-09-05), ~two-thirds recovered**

Found by R6, which re-measured the README table for the release rather than
trusting it. `frame_10k_rects` — the 100×100 grid of plain rects that
declares **none** of what alpha.6 added — went from **516 µs to 1.37 ms**
between `dabe671` (2026-08-31) and `b3cb849`, measured back to back on the
same Apple M3 Pro under the same load. The other three original rows moved
with it: 72 → 175 µs (`frame_1k_typical`), 754 µs → 1.81 ms (text and hits),
60 → 122 µs (deep nesting). That is 2.4–2.7×, against a ~5% noise floor.

**The old numbers were not wrong and the machine is not the difference.**
Checking out `dabe671` into a worktree and benching it on this machine
reproduces the published table to within 2% (72 µs, 516 µs, 754 µs, 60 µs),
and `[profile.bench]` and the divan version are identical at both ends. The
cost is real code.

**It is not one commit.** Bisecting `frame_10k_rects` over the 193 commits
since gives a staircase, not a cliff. Each row is the *cumulative* cost at
that commit, not the cost of that commit alone — the sampling was every 16th
commit, then narrowed around the two largest steps, so a row names the commit
measured rather than a culprit:

| commit measured | landing there or just before | median |
|---|---|---|
| `dabe671` | the measurement the README carried | 516 µs |
| `056effc` | titlebar decoration, IME composition, +14 more | 611 µs |
| `e4cdab5` | tab clicks under the hover column | 642 µs |
| `8d622e2` | CSS-style keyframes on transitions | 788 µs |
| `83ef155` | wrap / `max_lines` / ellipsis | 827 µs |
| `f964c15` | entrance transitions | 848 µs |
| `d11e411` | audio as data | 865 µs |
| `028894f` | measurement, layout events, diagnostics | 973 µs |
| `93169ed` | keyboard focus as data | 1.03 ms |
| `462f704` | modal surfaces as data | 1.05 ms |
| `f04e76e` | through opacity/shadows, cursor, scroll, `core_methods!` | 1.25 ms |
| `b3cb849` | today | 1.37 ms |

Every feature added tens of microseconds per 10k nodes to a frame that does
not use it, and no single step was large enough to argue with on its own —
**the review of each feature had no number it could fail.** The staircase is
the shape of the problem; the cause is below.

Worth knowing before anyone optimises: the features that were gated *were*
gated properly, and the benches prove it — `frame_10k_rects_one_exit` is
within 2% of the plain grid, a non-wrapping row pays nothing for wrapping,
and a radius on a clipping node costs ~1%. So this is not a case of a flag
that leaks.

**The cause, measured (2026-09-05).** `NodeSpec` went from **152 bytes to
728** — 4.8× — and it is moved by value at every step of building a node.
`sample`(1) over `frame_10k_rects` puts **31% of the frame in
`_platform_memmove`**, which does not appear in the old commit's profile at
all (below its 5-sample floor). Every caller of it is a `NodeSpec` move:

| samples | where |
|---|---|
| 1071 | `Tree::push` — the copy into `Vec<NodeSpec>` |
| 501 | `Core::open_with_key` — the by-value parameter |
| 457 | `Core::open` — the by-value parameter |
| 1028 | the bench's own `grid()` — the `NodeSpec::column().width().height().bg().radius()` builder chain, each method taking and returning `Self` by value |

That last row matters beyond the core: **every app that builds a spec with
the builder chain pays this too**, in its own code, before the core sees the
node.

At 152 bytes the compiler inlined those copies (the old profile's `Tree::push`
carries 1783 self samples and calls nothing); at 728 they became out-of-line
`memmove` calls.

**A controlled experiment confirms it.** Adding an inert `[u64; 72]` padding
field to `NodeSpec` at `dabe671` — changing its size to 728 and *nothing
else*, no new logic, no new passes — reproduces about half the regression on
its own:

| bench | `dabe671` (152 B) | `dabe671` padded to 728 B | HEAD (728 B) | size alone |
|---|---|---|---|---|
| `frame_10k_rects` | 516 µs | 915 µs | 1.37 ms | 47% |
| `frame_1k_typical` | 72 µs | 116 µs | 175 µs | 43% |
| `..._with_text_and_hits` | 754 µs | 1.22 ms | 1.81 ms | 44% |
| `deep_nesting_64_levels` | 60 µs | 84 µs | 122 µs | 39% |

So **~45% of the regression is the struct's size alone** and the rest is the
per-node logic added around it — consistent across all four benches, which is
what a per-node cost looks like. The two are not independent: the added
passes read fields scattered through a 728-byte struct, so `resolve_hover_style`
is the top named function in the new profile (11%) on a bench that declares no
hover styles at all and early-returns from it.

Where the 728 bytes are: seven `Option<Value>` event payloads at 32 each
(`on_click`, `on_drag`, `on_key`, `on_context_menu`, `on_hover`, `on_layout`,
`modal`) = **224**; `enter` and `exit` at 60 each = **120**; `VisualStyle`
**88**; `LayoutSpec` **80**; `Vec<Keyframe>` **24**; two `Label` = **32**; the
access fields and flags the rest. Nearly all of it is cold on nearly every
node.

**Fixed for the tag (2026-09-05): `NodeSpec` is 224 bytes.** The four cold
groups are behind a `Option<Box<…>>` each — `EventSpec` (the seven payloads),
`AnimSpec` (`enter`/`exit`/`keyframes`), `AccessSpec` (the declared
accessibility properties) and `InteractSpec` (hover / pressed / focus
backgrounds, the hover group, the two sounds). Reads go through an accessor
(`spec.events().on_click`) that hands back a shared `EMPTY` const rather than
allocating; writes go through `events_mut()`, which allocates on first use.
**The builder API did not change** — `.on_click(v)`, `.role(r)`, `.hover_bg(c)`
all still read the same, which is why `kui-ffi` needed one line and the
bindings needed none.

| bench | `dabe671` | before the fix | after | recovered |
|---|---|---|---|---|
| `frame_10k_rects` | 516 µs | 1.37 ms | **788 µs** | 68% |
| `frame_1k_typical` | 72 µs | 175 µs | **116 µs** | 57% |
| `..._with_text_and_hits` | 754 µs | 1.81 ms | **1.20 ms** | 58% |
| `deep_nesting_64_levels` | 60 µs | 122 µs | **79 µs** | 69% |

Every other bench moved with them — `all_declaring_exit` 3.87 → 2.21 ms,
`list_10k_rows_naive` 5.59 → 3.67 ms, `drop_1k_rows_plain` 102 → 63 µs —
including the ones that touch none of the boxed fields, which is the point:
the cost was the struct, not the features. `_platform_memmove` is gone from
the profile (below its 5-sample floor, where it was 31%), and so is
`resolve_hover_style`, which is now one null check on the boxed group instead
of three `Option`s read out of a 728-byte spec.

That is **more than the ~45% the padding experiment predicted**, because
shrinking the struct also fixed the cache behaviour of the per-node passes
that read it — the two costs were compounding, as the entry guessed.

**Two things guard it now.** `spec::size_tests::node_spec_stays_small` fails
above 256 bytes and says what to do instead, so an inline field has a number
to fail rather than a release audit to wait for. And `NodeSpec`'s `PartialEq`
is hand-written: boxing introduced a difference between "group never
allocated" and "group allocated and left at its defaults" (`.checked(false)`
does the latter), and the derived impl called those unequal. The manual one
compares through the accessors, so it does not.

**What is left.** `frame_10k_rects` is still ~1.5× its 2026-08-31 cost, and
that half is the per-node logic the features added, not the struct. It wants
its own profile now that the cache behaviour has changed — the shape of the
answer is which of the added passes can be skipped wholesale with a tree-level
flag, the way `any_exit` already skips the depart diff. Also still open: a CI
threshold on `frame_10k_rects` and `frame_1k_typical`. The size test catches
the specific mistake that caused this one; it does not catch a slow pass, and
a threshold would.

A note for whoever picks that up: `Quad` also grew, 92 → 124 bytes, and the
display list is one per quad. It did not show up in either profile — emission
is a small share of a frame — but it is the same mistake in the same place,
and `frame_10k_rects_with_shadows_and_opacity` (20k quads) is the bench that
would show it.

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
2. **`WindowId` plumbing** — **done (2026-09-05)**. `WindowId` (opaque,
   `MAIN` is 0) reaches every transport: `UiEvent::window`, `WindowEnv::id`,
   `KuiEvent.window`, `window` on a JSX `UiEvent`, `env.window.id` in Lua
   and `env().window.id` / `setEnv({window:{id}})` in Node. Zero everywhere,
   and it held: `cargo test --workspace`, the corpus **byte-identical**
   across all four adapters, the C example and `npm test` all pass, and the
   only edits to existing tests were the two Node `deepEqual`s over the
   whole of `env().window`, which gained `id: 0`. The Lua unit test that
   builds a `UiEvent` by hand gained `window: WindowId::MAIN` — an
   exhaustive struct literal in another crate, which is the cost of the
   field being public and not a sign the step was wider than it looked.
   **The producers cannot fill the field in, so the core stamps it.** A hit
   test, the edit buffer and the audio queue are all below the level at
   which a window exists; a `Core` is not — it *is* a window, and the driver
   already tells it which one through `env.window`. So `Core::stamp` writes
   `env.window.id` over every event leaving by `handle_input` or
   `take_pending_events`, and nothing else in any binding routes by window.
   That is the one design choice here that step 3 inherits: it has to give
   each core its id (which it must do anyway, for `WindowEnv::id`) and
   `UiEvent.window` follows for free, in all four bindings, with no further
   plumbing. The stamp early-returns on `MAIN`, so the single-window case
   costs nothing.
   **`KuiEvent.window` was the first field ever appended to an [out]
   struct**, which made it ADR 0006's first real test rather than a
   restatement of the raw append ADR 0004 described. It works as designed:
   `ABI_V1_SIZE` is measured through `payload`, so the append does not move
   the floor, an ABI-3 host's reservation is still accepted, and
   `write_out` stops before the new field. `KUI_ABI_VERSION` went 3 → 4
   anyway, per ADR 0006 decision 2 — the bump is for hosts that skipped the
   version check, not for ones that set `size`. (3, not 2: A3's access-node
   field moved it first, in the same unreleased version.) Both directions
   are now asserted through the public API rather than only on the `Grown`
   stand-in: in Rust
   (`an_abi_3_host_polls_events_without_seeing_the_appended_window`) and in
   C, where `offsetof(KuiEvent, window)` *is* the ABI-3 reservation and says
   so in one line.
   **Two things deliberately left for step 3.** C has no way to *set*
   `WindowEnv::id`: `kui_env_set_window` would have to grow a parameter,
   which is a source break, and step 3 breaks C anyway
   (`kui_take_window_commands`) — so the id joins that break instead of
   spending a second one. And Lua's event has no `window`: a Lua `on_event`
   receives the payload table plus `node_key` and carries no `origin`
   either, so there is no field there to append to without inventing one.
   One incidental find: `kui`'s runner imports `winit::window::WindowId`,
   which now collides with `kui_core`'s through `pub use kui_core::*` — it
   is aliased `WinitWindowId`, or `kui::WindowId` would not have been
   nameable at all.
3. **The declared set** — **done (2026-09-05)**. `declared_windows` /
   `declared_windows_last` beside the focus pair, the diff into
   `WindowCommand::Open`/`Close(WindowId)`, the `{kind:"window", phase}`
   event, and the runner opening real `Normal` windows.
   `kui_take_window_commands` becomes a `KuiWindowCommand` out-param here
   — a command now carries a `window` beside its verb, which no longer
   fits a `uint32_t` — and C hosts edit their drain loop. Keep the struct
   pointer-free: an `Open` carries no title (ADR 0004 decision 5), so
   `WindowCommand` stays `Copy` and no borrowed string enters the drain.
   **What the build settled.** The diff cannot live on a `Core`: the
   union is over every core's frame, and a core sees only its own. So the
   pair on the core is each frame's declarations and the fast path (a
   frame that declared what the last one did, with nothing sitting
   closed-but-declared, never touches the session), and
   `session::WindowRegistry` holds the rest — one slot of declarations per
   core keyed by its `WindowId` and kept in id order, so "lowest declaring
   window wins" is the first slot that names a window and needs no frame
   order; the windows opened so far with their ids; and the closed-by-OS
   entries whose names have not lapsed. `finish_frame` hands its slot in
   and takes the diff's `Open`/`Close` back into its own queue, so the
   driver drains what it always drained. Three things followed from that
   shape rather than from the text. **The core assigns ids**, not the
   driver (decision 3 said the driver): the `Open` has to carry one and
   nothing above the core exists yet. **The `window` event is raised at
   both edges** and carries `name` and `id` in its payload, because
   `UiEvent::window` is the core that ran the diff, not the window the
   event is about. And **a closed window's slot leaves the union with
   it**, so a window that declared a child closes the child in the same
   diff, parent first — the "popup declares its own submenu" case
   costing nothing extra.
   **The C break is a rename**, `kui_take_window_command`, popping one
   `KuiWindowCommand` per call the way `kui_poll_event` does: an array
   of [out] structs cannot carry ADR 0006's size handshake (0006,
   decision 5), and a renamed function fails an un-edited host at link
   time where a changed pointer type would only warn. `KUI_ABI_VERSION`
   is 5, `kui_env_set_window` leads with the id (C's way to set
   `WindowEnv::id`, deferred here from step 2), and `abi_parity` pins
   both structs plus the `KUI_CMD_*` / `KUI_WINDOW_*` constants.
   **The runner became a `Pane` per window** — surface, renderer, core,
   cursor, click counter, caret blink, resize band, access adapter — on
   one event loop, one `Session`, one `Gpu`; every handler is indexed by
   the pane winit's event named, and `PumpRunner::core_mut` is the main
   pane's. Node's `KuiWindow` keeps a tree per window name and
   `setView(tree, window)` fills one; the loop calls `view(model, name)`
   for each name `windows()` lists. **One bug the tests caught**: a
   headless `Ctx` also answers `windows()`, and a loop that drew a frame
   per listed name drew a second frame on the same core without the
   declaration, closing and reopening the window every render — a
   headless surface is one window, and only a real window surface fans
   out. Real windows opening is P8's smoke job; every check here is
   headless, and the four adapters reproduce the `windows` scene
   byte-identically, including the `cmd` lines the report gained (which
   also pinned the modal scene's titlebar `drag` for the first time).
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
   **Open, and it is the first build item after alpha.6** — see "After
   alpha.6" below. The release ships steps 1-3 and 5 and states the line
   rather than leaving it to be found (R3): ADR 0004's Consequences carry a
   dated amendment, the README's Status / next has a Windows group, and
   nothing can name a kind that quietly opens a normal window — `WindowKind`
   has only `Normal`, a JSX or Lua `windows` entry with a `kind` key is
   refused, and C's `KuiWindowConfig.kind` warns `unknown-window-kind` and
   opens a normal window. So the three cases the ADR opens with — a dropdown
   taller than the window, a menu with nowhere in-window to go, a panel
   beside the app — stay unbuildable until this step, and everything that
   fits in the window is `FloatConfig::fit` plus a `modal` float meanwhile.
5. **`SetSize` / `Focus`** — **done (2026-09-05)**, after step 3 and
   through its struct. `WindowCommand::SetSize { window, size }` and
   `Focus(WindowId)`, queued by `Core::set_window_size` /
   `Core::focus_window` into the queue chrome and the declaration diff
   already fill, drained in order by `take_window_commands`, and never
   applied headlessly because a headless driver never drains — the shape
   `reveal` and `play` have. In all four bindings the way C4 was:
   `ui.set_window_size` / `ui.focus_window` in Rust, `setWindowSize` /
   `focusWindow` on both Node classes via `core_methods!`,
   `kui_set_window_size` / `kui_focus_window` in C, `env.set_window_size` /
   `env.focus_window` in Lua. The runner resolves the command's window with
   the `pane_of` step 3 built and applies `Window::request_inner_size` /
   `Window::focus_window`; a command naming a window that is not open is
   dropped rather than applied to another.
   **This is the other half of step 3's edge rule**, not an addition beside
   it. Config is read on the opening edge only, so after step 3 a
   declaration *cannot* move a live window — which is the point ("the user
   owns geometry once the window exists") but leaves an app that legitimately
   wants to resize its own window with nothing to say. These two verbs are
   that, and the ADR's reason for keeping size a command reads as the reason
   it is one: there is still no `size` prop, and the tests pin that a
   request changes neither the viewport nor the frame after it.
   **C: `KuiWindowCommand` gained `width`/`height`**, and the size did not
   ride in `config`. The core's `SetSize` carries a bare `Size` and not a
   `WindowConfig` — a config is what a window *opens* with — so C says the
   same thing rather than overloading the one field step 3 documented as
   `KUI_CMD_OPEN` only. That makes this the second append to an [out]
   struct, which is what `size` leading one is for: the floor stays at
   `abi_through!(.., config, ..)`, an ABI-5 host's reservation is still
   accepted and still drains (asserted both in Rust and by the C example),
   and no host that never calls `kui_set_window_size` can receive the verb
   that fills the new fields. `KUI_ABI_VERSION` went 5 → 6 anyway, per ADR
   0006 decision 2 — the bump is for the host that skipped the check.
   **No corpus scene, and the reason is sharper than "the report cannot say
   it".** Step 3 gave the report a `cmd` line, so it can; what it cannot do
   is exercise all four *bindings*. The corpus compares what a declared tree
   lowers to, and these two are deliberately not declarations, so each
   adapter would have to reach them another way — and the two ways available
   each miss a binding. A step kind (the `Step::WindowClosed` pattern) is
   applied by the shared Rust harness for the Lua adapter, whose scenes are
   views and not step loops, so Lua's `env.set_window_size` would never run;
   an imperative call inside the scene builder covers Lua but is impossible
   for Node, whose corpus scenes are pure tree functions handed to
   `ctx.frame`. Neither covers four, so the per-binding headless tests stay
   the coverage — and each does drive its own entry point: `Core` and `Ui`
   in `tests/window.rs`, `kui_set_window_size` through the struct drain in
   `mod window_commands_headless` and again in the C example,
   `env.set_window_size` from a script in kui-lua, `ctx.setWindowSize`
   through `windowCommands()` in `test.mjs`.

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

### `.` X3 — List the missing input modes in Status / next — **done (2026-09-04)**

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

**The last half landed: touch and pen are named.** The sentence joins the
pointer-buttons paragraph, which is where the other input-mode limits live.
The section was not quite silent — C3 had tacked a bare "Touch and pen input
do not reach the core at all" onto the end of the *cursor-shape* paragraph,
which is about what the pointer looks like rather than about which input
modes exist, and which says the gap without saying what it costs. That
sentence is gone; what stands in its place says the consequence: a finger on
a touchscreen arrives as whatever the platform synthesises as mouse input,
so a tap presses and clicks and nothing past that exists — no multi-touch,
no pinch/rotate/two-finger gestures, no pressure, no stylus tilt. It ends by
stating the position rather than hedging it: v0 is desktop-first.

That matches the code exactly. `InputEvent` has cursor motion, mouse buttons
(`Primary`, `Secondary`, `Middle`, `Other`), the wheel, text, preedit, keys,
modifiers and access requests and nothing else; `touch`, `pen`, `stylus`, `pressure` and `tilt` do not appear
anywhere in the crates; and the winit driver's `WindowEvent` match has no
`Touch` arm and no gesture arms, so the only path from a touchscreen is
whatever the OS synthesises for a mouse.

**The two consistency checks that came with it both passed, and changed
nothing.** The key line still reads true after C9: `KeyPress` carries a
code, mods, text and repeat with no scancode field, `KeyMods` is four bools
with no left/right identity, and the winit driver drops a key it cannot name
(`if code != KeyCode::Unknown` guards the dispatch) rather than delivering
`Unknown` — while the phase rides every key event through `to_value`, and
`keys_held` is private with no query beside it, which is what "the core
keeps no 'which keys are down' query" claims. And the stale exit-animation
wording C8 was supposed to leave behind does not exist: every surviving "a
removed node vanishes at once" states the *opt-in fallback* (no `exit`, or
no `transition`, and the node still goes at once), which is current and
correct. ADR 0005 has two of them and neither is stale either: one is in its
Context, quoting what the README said at the time, under an amendment banner
that already supersedes the decision it belongs to, and the other is in the
amendment itself, describing what a subtree refused by the 512-node budget
does — which is that same fallback.

The precedent it was written against: modal containment was on this
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

All four are closed: A2 on 2026-09-04, and the last of them — the
arrow-key composites — as A3 below on 2026-09-05.

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

### `.` A3 — Three ADRs have deferred arrow-key composites — **done (2026-09-05)**

`docs/adr/0007-composite-keyboard-patterns.md`, accepted and now built. The
answer to this entry's "real question — what a *roving tabindex* is in a data
IR" turned out to be **nothing new**: the item a browser keeps a roving
tabindex on is, in every pattern kui has, the item the app already marks
`selected`, and the same fact that tells a reader which one is current tells
the keyboard where to enter. So the entry precedence is focused → declaring
`initialFocus` → declaring `selected` → first, and the core retains not one
byte for it.

The composite itself is derived, not declared, for the reason that already
made `pos_in_set` derived: a flag can be forgotten on something that is a tab
list and set on something that is not, and then the keyboard and the platform
disagree about the same node. A container role whose items are focusable *is*
a composite, which also settles the `list` question with no prop — a
navigation list is rows containing links (the link is focusable, so every link
keeps its stop), a picker is rows that are themselves `focusable`.

**C6 helped more than this entry guessed.** `set_positions`' walk was not just
"closer": it *is* the walk arrow navigation needs, and the two are now one
function — "3 of 7" and the order the arrows take have to be the same seven in
the same order or the announcement is a lie.

Three roles at the tail (`radioGroup`, `menu`, `menuItem`), `orientation` on
the access node from the container's own `dir`, one warning
(`focusable-inside-item`), a `composite` corpus scene with four new steps, and
no `PROPS` row at all. `KUI_ABI_VERSION` is 3, since `KuiAccessNode` grew a
field.

**What the guards caught that the tests did not.** The three roles first went
in after `group` rather than at the tail of `Role::ALL`, renumbering every
`KUI_ROLE_*` from `window` on — kui-core's own suite was green, and all
thirteen scenes failed from C on the first run. And two facts only the
platform could give: `AXOrientation` reaches an AX client as a *string* where
the app hands AppKit an `NSInteger`, and a `radio` is an `AXRadioButton`
exactly like a `tab`, so the audit's existing tab checks had to be narrowed to
the `AXTabButton` subrole they were already asserting. `scripts/ax-audit.swift`
runs 88/88 on macOS 26.

**Still open, and named in the ADR's own follow-ups**: grid navigation (Left /
Right *into* a row, Up / Down between rows, which wants a `grid` / `row` /
`cell` vocabulary — decision 6's line motion is its geometric half already),
submenus, a `radio-without-group` warning now that the fixture declares
radios, and multi-select. `KUI_ROLE_LINE`, which the ADR listed as referenced
in three comments and defined in none, is in fact defined in the header's role
enum; nothing to do there.

The original finding:

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

### `~` A6 — The corpus can skip, and its coverage is self-declared — **done (2026-09-04)**

Both halves shipped (`2e0aba3`, `59fe451`); the heading went unmarked, which
is the third time this round pattern has happened — see B4.

**The skip** is now assertable: `KUI_CONFORMANCE_REQUIRED` turns a missing
report into a failure, and the `check` job sets it, so the publish job keeps
the skip it needs and the job that must run the adapter cannot lose it
quietly. `blocks.length > 0` also catches a truncated reference.

**Coverage is derived**, not declared: `conformance::observe` builds the
`custom` / `elements` sets from what each frame actually built, and every
hand-written claim has to appear in the derived set. Rows that genuinely
cannot be derived are enumerated in `conformance::UNDERIVED` with a reason
each, and `the_underived_rows_are_real_and_still_underived` stops that list
becoming a dumping ground. It had exactly one entry, and it produced P9 —
the `chrome` scene's `windowButtons` claim had no tree behind it and had not
since P7 landed, which is the derived check paying for itself immediately.
P9 then closed it (2026-09-04) and the list is empty.

The original finding:

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

## From the third review (2026-09-04)

479 Rust tests, 45 Node, ten corpus scenes across four adapters, fmt and
clippy clean. `3cbf96a` is the round's most instructive commit — the header's
role enum had silently stopped at `KUI_ROLE_SCROLL_VIEW` while `Role::ALL`
grew, so C alone could not mark an editor's lines. It was found by hand, and
`abi_enum!` now pins eight enum families so it cannot recur **in C**. B1 is
the same bug one binding over.

### `!` B1 — `schema::ROLES` is the last unpinned enum restatement — **done (2026-09-04)**

Both asserts shipped, in `schema`'s own tests.
`every_declarable_role_name_is_a_real_role` pins the names and the mapping
together — each `ROLES` entry parses, and index `i` still means `ROLES[i]`, so
a `role_idx` that stops reading the list fails as loudly as a misspelling in
it. `every_role_is_declarable_or_derived` pins the other direction: every
`Role::ALL` variant is declarable or on the new `schema::DERIVED_ONLY`, never
neither and never both.

`DERIVED_ONLY` carries the reason per role, and the test keeps it honest by
*deriving* rather than trusting: it builds one frame — root, a `window_drag`
row, a scrolling box, text inside it — and asserts the core really produces
every role the list exempts. So a variant merely forgotten from `ROLES` cannot
be parked there to quiet the failure, which was the risk in giving the guard
an escape hatch at all. Mutation-tested, each failing with the role named: a
typo'd name, a role dropped from `ROLES`, a forgotten role moved into
`DERIVED_ONLY`, an exemption with no reason, an exemption the core no longer
derives, and a `role_idx` rewritten to index `Role::ALL`.

**The fallback is now `Role::Group`, and the entry's `role_idx(99)` no longer
returns `None`.** Both ways of reaching it turned out to be closed: a drifted
spelling now fails the test rather than a frame, and no transport passes an
index it has not bounds-checked (Node's binary reader rejects
`i >= names.len()` before building a `Parsed::Enum`, Node's JSON and Lua
resolve a *name* through `enum_index`, C carries a bounded `Role::ALL`
position). So nothing that runs today changes — the change is in what a future
transport that forgets its check gets. Hiding a subtree is a destructive answer
to "an index I do not have"; a group is what the core already derives for a box
that is merely somewhere focus can land, the node keeps its children, and a
wrong role is recoverable where a missing subtree is not. No warning: this is a
pure schema function with no sink, and the transports' own errors already name
the prop and the index.

Two doc comments carried the drift the guard is against and now point at the
lists instead of restating them — `Role`'s said the declarable roles are "the
first group" (the derived four are interleaved), and `ROLES`' named those four
in prose, three lines above the list itself.

The original finding:

`Role::ALL` has 22 variants; `schema::ROLES` has 18 — the declarable subset,
correct today (the four omissions are exactly the derived roles: `window`,
`titleBar`, `staticText`, `scrollView`). **Nothing pins the relationship.**
`abi_enum!` now catches this for C by construction; Lua, Node and JSX read
`ROLES` at runtime and have no equivalent.

The failure mode is worse than C's was. `role_idx` is

    ROLES.get(i).and_then(|n| Role::parse(n)).unwrap_or(Role::None)

and `Role::None` **hides the node and its whole subtree from the access
tree**. So a name in `ROLES` that does not parse — a typo, or a role renamed
in `access.rs` — does not fail, does not warn, and silently removes a subtree
from every screen reader. Verified: `role_idx(99)` returns `None` rather than
erroring.

Two asserts in `kui-core` close it: every `ROLES` name parses, and every
`Role::ALL` variant is either in `ROLES` or in a short list of derived-only
roles that the test also checks is still accurate — the shape
`the_underived_rows_are_real_and_still_underived` already uses for the corpus.

### `~` B2 — Node is the only binding with no `Env` — **done (2026-09-04)**

Both directions shipped. **`ctx.env()` / `win.env()`** return
`{refreshHz, frameBudgetMs, focused, viewport, window}`, with `window` as
`{customChrome, maximized, fullscreen, nativeControls}`. It went into the
`core_methods!` list rather than onto either class, so it lands on both by
construction and there is no second copy to forget (D1's whole point).

Two spellings depart from Lua's table deliberately, and the doc comment on
`env_json` says which and why. `refreshHz` is `null` where Lua omits the key
— a stable shape is worth more to JS that destructures it, and it types as
`number | null`. `nativeControls` is the whole rect where Lua flattens it to
`controls_w` / `controls_h`; that flattening assumes the OS controls sit at
the window origin, which is true of the macOS traffic lights and of nothing
in particular, and the core holds a rect. `viewport` rides along as the
frame's `{width, height, scale}` — Lua carries it in the same table too, and
`WindowSize` is the shape `runWindowed` already uses.

**`ctx.setEnv({...})`** is the write side: one call where C has two
(`kui_env_set` + `kui_env_set_window`), shaped like what `env()` reads back,
and merging rather than replacing, so `setEnv({window: {customChrome: true}})`
is the whole of "pretend this app draws its own titlebar". It is on `Ctx`
only, and a test says so — a `KuiWindow`'s runner reports the real window
every frame, so a fact set on one would be overwritten before the next view
ran. Unknown keys and wrong types throw: a silently ignored `refreshHz` looks
exactly like the 120 Hz fallback.

`index.d.ts`'s generated half came out of the `#[napi]` attributes (P5) with
no hand-editing; only the `Env` / `WindowEnv` / `Rect` / `EnvInput` shapes are
hand-written. Six Node tests, mutation-tested: dropping `customChrome` in
`setEnv` and forcing `nativeControls` to null in `env()` each fail them.

P9 shipped on top of this and is done too.

The original finding:

Found while reading P9. Rust has `ui.env()`, Lua builds the whole `env.window`
table (`custom_chrome`, `maximized`, `fullscreen`, `controls_w/h`), C has
`kui_env_set` / `kui_env_set_window`. **Node has nothing** — no read, no
write, verified by grep across `crates/kui-node/src/lib.rs`.

So a JSX app cannot know whether it is under custom chrome, whether the window
is maximized, where the macOS traffic lights are, the refresh rate, or whether
the window is focused. The `<titlebar>` element papers over the common case by
being a built-in that reads `env` in Rust — but a JSX app cannot write its
*own* titlebar, which is the case custom chrome exists for, and `widgets::
titlebar`'s whole design is "reads `env.window` and adapts by itself".

It also blocks P9: the corpus cannot cover `windowButtons` until an adapter
can declare custom chrome, and Node is the adapter that cannot.

### `~` B3 — Exit animations changed the frame model and have no scene — **done (2026-09-04)**

Shipped as an eleventh scene, `exit`, plus two additions to the corpus
protocol. ADR 0005 had argued against a scene under "No corpus scene, and
why", and that section is now superseded rather than deleted: its reasoning
about *lowering* holds — `exit` is a plain `PROPS` row three bindings read by
table lookup and the fourth is forced to carry by
`every_schema_prop_has_a_c_counterpart` — and its conclusion does not, for
the reason this item gives. The corpus compares quads, access rows and events
across driven steps, and a ghost is the first node that draws while being
absent from the hit regions, the Tab ring and the access tree at once.

A report keeps one frame, so — the lesson A1's `modal` scene wrote down — the
steps end where the claims differ, 80 ms into a 400 ms exit:

- `fade` is mid-flight and **draws**, text and all, which is the previous
  frame's *text list* as well as its tree.
- It is **inert** three separate ways in that one frame: a press on ground
  covered by both where the node was and where its ghost now is emits
  nothing, where the earlier press on the live node emitted `hit`; two Tabs
  walk from `A` to `B`; and the access tree lists neither.
- `flash` left and came back mid-exit, so the frame holds **one** picture of
  it — the toast-dismissed-and-reshown case — and `blink` ran 50 ms and is
  **over**. Both rest on the checked-in `solid: 9`: a store that kept either
  would count ten, both eleven.
- `bulk` is one node past `depart::MAX_NODES`, so the **budget** refused it
  whole: no ghost, and an `exit-budget` warning.

`Step::Phase(n)` and `Step::Time(ms)` are the two protocol additions, and
neither is specific to exits nor an input: the phase is the view changing its
mind (a builder is a function of it; the other ten scenes ignore it), the
time is the frame clock (without one every transition snaps and there is no
ghost). Each adapter grew one arm that does not call `handle_input`. Node
builds its tree per frame now rather than once — a departure *is* a changed
tree — with the fixtures registered on the first build that asks, so its
handles are unchanged; Lua seeds the phase as a script global through a new
`LuaExtension::lua()`.

It caught nothing: all four agreed on the first run that compiled, which is
what a row three bindings lower by table lookup should do. The scene earns
its keep on the next change to `depart.rs`, which now has four readers.

The original finding:

C8's second half shipped: `exit` (P_EXIT = 78), a `DepartStore`, ghosts that
self-ease, opt-in with a 512-node budget. It is the first feature that makes a
node **outlive the frame that declared it** — the previous frame's tree and
text list are kept by buffer swap — and it is the only behaviour to land this
round without a conformance scene. Ten scenes; none of them exits.

That matters more here than for a paint prop. The corpus drives steps and
compares quads, access rows and events, which is exactly the shape a ghost
needs pinned: that it draws, that it is **inert** (no hit region, no Tab ring
place, no access row), and that a returning key discards it rather than
doubling. Those are four bindings' worth of behaviour resting on one Rust
test suite.

### `.` B4 — Backlog headings lag the work, three rounds running — **done (2026-09-05)**

Closed by behaviour rather than by a rule: in the round after this was
filed, every closing session marked its heading — `e238a0e` says so in its
subject ("B1 is closed, and the heading says so this time"), and `79ae943`
went further, correcting a P9 heading that had said done before it was
verified. The review of that round found zero stale headings against
twenty commits. No preamble line was added; the entries themselves now
carry the convention.

The original finding:

C6 and D1 shipped unmarked in round two; A6 shipped unmarked in round three.
Each time the body text was left as the original finding and the heading said
open, so the backlog under-reported and the next reviewer had to diff commits
against headings to find out.

Not worth a process gate. Worth one line in this file's preamble: a session
that closes an item marks the heading and appends what it learned, in the
shape the closed entries already use — original finding kept, outcome on top.

---

## From the fresh sweep of the grown surface (2026-09-05)

The rounds added ~20k lines the first review never saw. This pass asked the
same question of them that found the original defects — what is hand-written
without enforcement, and what state does the core keep that a binding cannot
reach — plus a new one for the new mechanisms: does each guard refuse what it
exists to refuse? State at the sweep: 512 Rust tests, 55 Node, 14 scenes in
four adapters, `KUI_ABI_VERSION` 6, fmt and clippy clean.

### `!` S1 — A C plugin that omits `kui_ext_abi` loads unchecked — **done (2026-09-05)**

`kui_ext_abi` is required. `CExtension::open` now refuses a plugin without
the symbol — `{path}: plugin declares no ABI; this build is {KUI_ABI_VERSION}`,
the same shape as the mismatch error — and does so before `kui_ext_view` is
looked up, so a pre-0006 plugin is turned away as the ABI mismatch it is
rather than as anything else. The six-function list in `ext.rs`'s module
doc and in `include/kui.h` now says which two are required (`kui_ext_abi`,
`kui_ext_view`) and which four are optional; the header's `kui_ext_abi` row
says why absence cannot mean unchecked. The alpha.6 CHANGELOG entry, which
had said "five of them optional", is corrected in place.

Two checks pin it. A unit test in `ext.rs` opens the platform's own C
runtime (`libSystem.B.dylib` / `libc.so.6` / `kernel32.dll`) as a plugin —
a library that loads everywhere and defines no `kui_ext_*` symbol at all,
so no compiler is needed in the test — and asserts the whole refusal
message. And the mutation recipe from the finding is automated:
`examples/c/build.sh` builds `target/panel-noabi.so` from `panel.c` with
the one `kui_ext_abi` line deleted by `sed` (and fails if the deletion
missed), and a new CI step after "C extension (dlopen + origin routing)"
runs `c_panel --headless target/panel-noabi.so` and requires exit 1 plus the
message. Verified locally: the un-mutated plugin still prints `ok: clicks
routed by origin, both ways`; the mutant is refused.

**Lua has no analogous hole**, confirmed. `kui_lua::LuaExtension` is a
`Core` wrapper over `mlua::Lua::new()`: it `dlopen`s nothing, and no
`repr(C)` struct crosses to the script — the prelude hands it tables and
functions, and layout is settled by the Rust build that contains both
sides. There is no version to check because there is nothing that can be
compiled against an older header.

The original finding:

`crates/kui-ffi/src/ext.rs:103`: the ABI check runs only `if let Some(abi) =
sym("kui_ext_abi")`. A plugin without the symbol skips it. **Verified by
mutation**: `examples/c/panel.c` with the `kui_ext_abi` line deleted, built
with the same `cc` line as `build.sh`, loads via `c_panel --headless` and
prints `ok: clicks routed by origin, both ways`. No warning.

This inverts ADR 0006. The plugin most likely to lack the symbol is one built
against a header from before 0006 — which is precisely the mismatched plugin
the check exists to refuse, and the direction (older plugin, newer host) that
corrupts memory rather than merely missing a feature. The header's own six
functions list calls `kui_ext_abi` one of the five optional ones; it should
be the one required one, or absence should refuse with the same message a
mismatch gives.

### `.` S2 — The warning codes drifted across three hand-written copies — **done (2026-09-05)**

Closed by the mechanism the props already use. `diag.rs` declares its codes
through a `warnings!` macro that emits the same `pub const`s and a
`WARNINGS: &[WarningDef]` table whose `doc` is each const's own doc
comment, so there is one text to edit; a unit test reads the file's
`pub const NAME: &str = "code";` lines back and asserts they are the table,
so a const declared outside the block fails the test instead of drifting.
The addon's `protocol()` exports the table as `warnings`, and `npm run gen`
writes it into `index.d.ts` as the `WarningCode` union (between generated
markers, the way `jsx-runtime.d.ts` takes its props) and into `docs/props.md`
as a Warnings table. `index.d.ts` was already in CI's `git diff
--exit-code` list from P5. The README keeps its three examples and points
at the table; the doc comments lost their rustdoc intra-doc links (they now
name the other code in backticks) because the same text is now read by
TypeScript and Markdown, and the test refuses `[\``.

The original finding:

`crates/kui-core/src/diag.rs` defines 13 warning codes as `pub const`s with
doc comments. `packages/kui/index.d.ts`'s `code` union — the hand-written
half; P5 generated only the addon's `#[napi]` surface — listed 11:
`exit-budget` and `focusable-inside-item` were missing, so a TypeScript app
with a typed `switch (w.code)` could not name two live warnings. The README
named three by name. `docs/props.md`, the generated cross-binding reference,
had no warnings section at all. Same shape as P5 and X1: a list Rust owns,
restated by hand, drifting.

### `.` S3 — The scene corpus ships in every release binary — **done (2026-09-05)**

Built: `kui-core` has a `conformance` feature, off by default, gating
`pub mod conformance` and the accessors that existed only for its
`observe` — `Core::declared_focus`, `Core::declared_windows` and the
two `any_mounted`s on the audio store and its session handle.
`window_title` stays; the runner, Node, C and Lua all read it.

Who turns it on, and why it cannot be turned off by accident:

- `kui-core` depends on itself in `[dev-dependencies]` with the feature
  — the one way to enable a feature for a crate's own tests without
  putting `required-features` on the test target, which `cargo test`
  would then skip silently: A6's hole one binding over. Under resolver 3 a
  dev-dependency's features reach only the builds that need
  dev-dependencies, so `cargo test --workspace` and `cargo clippy
  --all-targets` see the corpus and `cargo build` does not. `cargo tree
  -e features -i kui-core` confirms it: zero `conformance` rows for
  `kui`, `kui-wgpu`, `kui-ffi`, `kui-node` and `kui-lua`'s normal graph.
- `kui-lua` does the same, in its `[dev-dependencies]` only.
- `conformance-dump` carries `required-features = ["conformance"]`; the
  documented command spells `--features conformance` (README, the C and
  Node adapters' hints, CI's reference step), though the dev-dependency
  would carry it anyway.
- **C and Node need nothing**, which was the surprise. The task expected
  `kui_conformance_*` exports in `libkui_ffi`; there are none.
  `counter.c --conformance` rebuilds every scene through the public C API
  and reads the reference as a file, and Node does the same through the
  addon. So `libkui_ffi` in CI and in the release is built exactly as it
  ships, `cargo build -p kui-node --release` and `cargo publish` resolve
  the feature off, and option (b) — moving an adapter out — has nothing to
  move.

The `check` job still runs all four adapters: Rust and Lua under `cargo
test --workspace`, C against the reference file, Node under
`KUI_CONFORMANCE_REQUIRED=1`. Verified locally with the whole sequence:
fmt, clippy over all targets, 54 test binaries with the two corpus tests
among them and nothing filtered, the dump, `build.sh`, `--headless`,
`--conformance` (conformance OK (14 scenes)), the dlopen panel, and 55 Node tests with none
skipped.

**The cost, measured** (`cargo build --release`, macOS arm64, one machine):

| artifact | before | after | delta |
|---|---|---|---|
| `libkui_core.rlib` | 3 678 144 | 3 313 088 | −365 056 (−9.9 %) |
| `examples/counter` (Rust) | 11 676 512 | 11 676 816 | +304 |
| `libkui_ffi.dylib` | 11 493 968 | 11 495 328 | +1 360 |
| `libkui_node.dylib` | 12 351 472 | 12 352 256 | +784 |

The executables did not shrink, and that is the honest number: `nm` finds
zero `conformance` symbols in the *before* binaries too, because nothing
shipped referenced the module and the linker's section GC already dropped
it. What the feature buys is the rlib — what `cargo publish` verifies and
every downstream compile reads — and the compile of `kui-core` itself: a
release rebuild of the crate alone went from about 5.5 s to about 2.9 s on
the second of two runs. A tenth of the crate's lines, a good half of its
codegen, which is what one formatting routine per scene costs.

The original finding:

`crates/kui-core/src/conformance.rs` is ~2 000 lines of a ~21 000-line
crate, `lib.rs` declared it with no `cfg`, and `kui-core` had no
`[features]` table. So every binary that links `kui-core` — every Rust
app, the Node addon, `libkui_ffi`, every Lua host — carried the fourteen
scene builders, `observe`, the report formatter, the digest and the
fixture bytes (`SOUND_BYTES`, `image_pixels`). Test infrastructure in the
release build. It could not simply be `#[cfg(test)]`: the dump example
writes the reference CI uses, the C example's `--conformance` mode reads
it, and `kui-lua`'s tests call `conformance::drive`.

### `.` S4 — `runtime.rs` and `kui-ffi/lib.rs` are the god files now — **done (2026-09-05)**

Split by concern as pure moves — one commit per file, so `git diff
--color-moved=zebra` on each shows the module header, the re-export, and
the `pub(crate)` a private method gains when its callers now live in
another file, and nothing else — plus one seam kept to its own commit:
`finish_frame` is `layout_frame` (layout and everything resolved against
it) followed by `emit_frame` (the display list and the hit regions). No
signature, name or order changed; the in-flow and floating passes inside
`emit_frame` share the hit and scroll-region buffers and stay one body.

`impl Core` continues in eight *children* of `runtime` rather than
siblings, so no field changed visibility (a child sees its parent's
private items) and the 92 public methods still render on one rustdoc
page:

| file | lines | holds |
|---|---|---|
| `runtime.rs` | 562 | the struct, `new`, measurement, diagnostics, the frame clock, `begin_frame` |
| `runtime/emit.rs` | 985 | `finish_frame`, the per-node emitter, ghosts, layout events, the focus ring |
| `runtime/dispatch.rs` | 632 | `handle_input`, routing, access requests, edit events |
| `runtime/builder.rs` | 432 | open / close / text / editors / images, keyframe easing |
| `runtime/focus.rs` | 290 | focus, the Tab ring, the modal scope |
| `runtime/composites.rs` | 290 | arrow-key motion and type-ahead (ADR 0007) |
| `runtime/resources_api.rs` | 273 | fonts, sounds, images |
| `runtime/windows.rs` | 227 | the declared set, its diff, window commands, the title |
| `runtime/scrolling.rs` | 168 | `reveal`, `set_scroll`, the caret and reveal nudges |

`kui-ffi/src/lib.rs` is 226 lines (the context, the host facts, the
diagnostics and the module list; every `pub` item is re-exported so the
crate's surface is flat): `types.rs` 939 (the repr(C) mirrors), `frame.rs`
383, `convert.rs` 349, `resources.rs` 318, `input.rs` 308, `access.rs`
257, `windows.rs` 198, `abi.rs` 170, `widgets.rs` 146, `value.rs` 118,
`scrolling.rs` 96, `focus.rs` 65, `run.rs` 59; the tests are `tests.rs`
545, `abi_parity.rs` 518, `schema_parity.rs` 449, `abi_handshake.rs` 418.

Guards run on the result: `cargo test --workspace`, clippy with
`-D warnings`, `cargo fmt --check`, the conformance reference regenerated
and byte-identical to the one dumped before the first move, the C adapter
(`counter --headless`, `counter --conformance`, `c_panel --headless`) and
the Node adapter over that reference, and `cargo doc -p kui-core` naming
every public method. What is still large is honest: `emit.rs` is the frame
in paint order, `types.rs` is `KuiSpec`'s two hundred documented fields.

The original finding:

`runtime.rs`: 3759 lines (was 2137), **92 public methods on `Core`**, two
`impl` blocks in total, `finish_frame` 294 lines. It owns input, focus, the
modal scope, scroll, the declared window set, departures, cursor, layout
events, IME, fonts, audio and the access tree. `kui-ffi/src/lib.rs`: 5468
lines, 107 `extern "C"` functions, 1952 of them tests; `ext.rs` was the first
split and should not be the last. Rust allows `impl Core` blocks across
sibling files — a split by concern (`focus.rs`, `scrolling.rs`, `windows.rs`,
`emit.rs`) is zero behaviour change and the corpus is the regression guard
that makes it safe to do in one sitting.

### `!` S5 — Resource handles are session-blind — **done (2026-09-05)**

Landed as a variant of option (b), because (b) as written could not fire.
A side table *in the asked session*, keyed by handle, finds that session's
own entry for a handle that also exists there — the alias case is exactly
the one where both sessions hold the same bits — so it can only tell
"minted here" from "never minted here", never "minted here" from "minted
there with the same bits". What fires is making the bits differ: one
process-wide minting `SlotMap` per kind (`resources::Mint`, behind a
`Mutex`) hands out every `ImageId` / `FontId` / `SoundId` and records the
owning `SessionId`; a session's `Resources` holds its entries in a
`SparseSecondaryMap` over those keys, so it can only ever hold what it
registered. A foreign handle is then a miss (the generation check is
process-wide, so a handle removed in one session and reused in another
misses too), and on the miss path the mint says whether the handle is live
elsewhere (foreign — recorded on the registry, drained into a
`foreign-resource` warning by the next `Core::take_warnings` of that
session) or nowhere (removed — silent, as documented). Live lookups never
lock; the mint is touched on registration, removal, the miss path and
`Resources::drop`, which gives a dead session's slots back.

Not (a): a tag folded into the `u64` needs a cap on the index or the
version, and the version is the one that moves — an app re-registering an
image every frame walks one slot's generation, so a 16-bit cap wraps in
minutes and the stale-handle check becomes probabilistic. Not (c): an ABI
bump for a guard that needs none. Not `debug_assert!`: the warning path
has to be testable under `cargo test`, and the diagnostics channel already
is the dev-build loudness (the runners print in debug builds). The warning
fires in every build; a headless `Core` has diagnostics on.

`Session::id` is public; `SessionId` is exported. The code is in
`diag.rs` with the other three codes that do not come from the tree walk,
hand-added to `index.d.ts`'s union until S2 generates it. Tests:
`crates/kui-core/tests/session.rs` covers the alias (two sessions, two
first images, the foreign one draws nothing and warns once, the owner
draws and says nothing, the asked session's own handle is unaffected), a
foreign font (shapes as sans, warns), a foreign sound (`play` warns,
`remove_sound` through the wrong session touches nothing), the hit
reported by whichever window of the session drains first, and the promise
C11 step 1 made — one handle across two `Core`s in one session draws in
both with no line. `resources.rs`'s unit tests pin process-unique minting,
the miss/foreign/removed distinction, and slots returning on drop.

The original finding:

`FontId`, `ImageId` and `SoundId` are slotmap keys crossing every boundary
as raw `u64` (`crates/kui-core/src/resources.rs`, `to_ffi` / `from_ffi`).
Since C11 step 1, resources live in a `Session` and every `Core` is
constructed against one; `Core::new()` makes a private session. A handle
minted in session A and passed to a `Core` of session B was looked up in
B's slotmap: a miss if the slot's generation differed (draws nothing,
shapes as sans — the documented behaviour for a *removed* handle), a
**silent alias** if it matched. Nothing detected either. `Session::is`
existed and was used only in one test. ADR 0004 decision 2's rationale for
`Session` includes "`ImageId` handles that silently belong to the wrong
slotmap if it gets it wrong"; the mechanism that was meant to remove the
risk had only moved where it lived. Two headless `Core::new()`s in one
test sharing an image id is the ordinary way to hit it.

### `~` S6 — Node cannot read the derived cursor shape — **done (2026-09-05)**

`cursorShape()` is on both `Ctx` and `KuiWindow`: one entry in the
`core_methods!` list, returning the schema name (`CursorShape::name`, the
same strings the `cursor` prop takes), so a test compares against `'text'`
and never an index. `npm run gen` rendered it into the generated half of
`index.d.ts` from the `#[napi]` attribute; the one hand-written line is the
`CursorShape` type it returns, `NonNullable<GeneratedSpecProps['cursor']>`,
so the query's vocabulary is the prop's by construction. Two Node tests
mirror `crates/kui-core/tests/cursor.rs` band for band: an `<edit>` is
`text`, an `onClick` box `pointer`, a plain box `default`, an `onDrag` box
`grab` and `grabbing` through the captured drag, a splitter with
`cursor="ewResize"` keeps its own shape through the drag, a disabled
control is `default` unless it declares `notAllowed`, `focusable` is a hand
and `hoverable` is not, no pointer is `default`, and a button inside a
hover-tracked card answers over the button. The two suites now assert the
same strings from both ends of the binding.

**Lua stays as it is.** `env` does expose per-frame derived state a script
reacts to — `focus`, `focus_visible`, `is_hovered`, `is_pressed`,
`scroll_offset`, `scroll_geometry` — but every one of those is an *input*
to the view: state it draws from. The cursor shape is the frame's *output*,
derived from what the view declared, and no view draws differently because
of it; its consumers are the host (which applies it to the real window) and
tests. Lua's host is Rust and reads `Core::cursor_shape` itself, and Lua's
tests are Rust and can read the core directly, so putting it on `env` would
add a reading nothing in a script has a use for. Node is different only in
that its tests are JavaScript, with no other way to the core.

The original finding:

`Core::cursor_shape()` exists (C3, `crates/kui-core/src/cursor.rs` and
`runtime.rs`) and C exports `kui_cursor_shape`. `crates/kui-node/src/lib.rs`
had no `cursor_shape` / `cursorShape` on `Ctx` or `KuiWindow` (verified by
grep 2026-09-05; the only `cursor` hits were the `cursor(x, y)` input
injector and `CursorLeft`). So a JavaScript test could not assert that
hovering an editor yields `text`, a button `pointer`, a drag handle `grab`
— which was C3's whole deliverable — and the windowed `KuiWindow` applied
the shape internally through the Rust runner without exposing it.

### `.` S7 — `env` reaches three bindings through three restatements and nothing pins them — **done (2026-09-05)**

**The shape is written down once**: `schema::ENV_FIELDS`, one row per value
in the reading a view gets, with the canonical (Rust) name, where it comes
from, the key path(s) it occupies in Node's `ctx.env()` and Lua's
`view(env)`, the C setter argument that writes it, and a doc. `docs/props.md`
gained an *Env* section generated from it the way *Resources* is, through
`protocol().env`; the rows for the frame facts that ride in the same reading
(the viewport, the focused node) say so in their `from` column rather than
being left out, since a key-set test that ignored them would not be a
key-set test.

**Both decisions went the way the code already leaned, and are now recorded
as decisions rather than accidents.** Node keeps `nativeControls` nested as
the `Rect` the core holds, where Lua and C carry `controls_w` / `controls_h`
at the window origin: B2's reasoning stands (the flattening assumes the
controls sit at the origin, which is true of the macOS traffic lights and of
nothing in particular), and the row says so. `frame_budget_ms` is in the
shape: it is derived, but both bindings that have a reading already carry
it, and its job is that no view restates the 120 Hz fallback — a C host has
no reading and is the one that knows the rate, so its cell is a dash.

**Each binding is pinned, in the direction that fails when the surface
moves.** In `schema`, an exhaustive pattern over `Env` and `WindowEnv` is
the pin on the structs — a new field stops the test compiling until it has a
row — and the row list is then checked against the table both ways.
`the_env_table_is_the_documented_env_shape` (kui-lua) walks the table
`view(env)` receives under an env with every optional fact present and
asserts its value keys equal the Lua column, key for key; the queries and
verbs beside them are pinned as a literal list, since they are Lua's own
surface. `env() is the documented env shape, key for key` (test.mjs) walks
`ctx.env()` recursively, stopping at any documented path so a `Rect` is a
leaf, and asserts against the Node column. C has no struct to `abi_struct!`
— the setters take the fields as arguments — so
`the_env_setters_take_exactly_the_documented_fields` (kui-ffi) reads
`include/kui.h`, parses the two prototypes, and asserts each parameter
list is exactly what the C column names for that setter, in order; the
header carries a note naming the fields against the same table. Every one
of these was mutation-tested and fails naming the cause: a key dropped
from `env_table`, a key dropped from `env_json`, a row dropped from
`ENV_FIELDS` (both the schema test and the kui-ffi test), a parameter
renamed in the header.

**The corpus asserts the readback** in the two adapters that read: every
Lua scene now records `env.window` from inside `view(env)` and the test
asserts it equals the `WindowEnv` the scene was driven under; the Node
adapter asserts `ctx.env().window` against the parsed `env` line after the
drive, beyond the report line it already derived from it. So `chrome` and
`chrome-inset` report `custom_chrome = true` and the traffic-lights extent
the same way everywhere, and the other twelve scenes pin the defaults for
free. Mutation-tested: a Lua `env_table` that reports `custom_chrome =
false` builds the same tree (the script only reads it) and the report still
matches — only the readback assert catches it, which is why the readback is
its own assert.

Two doc references in the C header and `kui-ffi` pointed at a
`kui_env_set_focused` that never existed; they name `kui_env_set`'s
`focused` now.

**Left open, and small**: `index.d.ts`'s hand-written `Env` / `WindowEnv` /
`EnvInput` shapes are types, not values, so no test reaches them; they are
one restatement the table does not pin. Lua has no `scale` reading, which
the row records rather than fixes.

The original finding:

`Env` (`refresh_hz`, `focused`, `window`) and `WindowEnv` (`id`,
`custom_chrome`, `maximized`, `fullscreen`, `native_controls`) reach three
bindings through three hand-written restatements: Lua's `env_table` builds
`refresh_hz`, `frame_budget_ms`, `focused`, `focus`, `focus_visible`,
`viewport_w/h` and a `window` table with `native_controls` **flattened** to
`controls_w` / `controls_h`; Node's `env()` (B2) builds `{refreshHz,
frameBudgetMs, focused, viewport, window}` with `nativeControls` nested; C
writes through `kui_env_set` / `kui_env_set_window` and has no read side.
Nothing pins the three against each other or against the Rust structs, and
`docs/props.md` — the cross-binding reference for props, composites,
elements, events and resources — does not document `env` at all. Not a
verified defect; a verified unguarded surface, which is exactly the state
the first review found P1 and P2 in before they were defects.

### `.` S8 — ABI bumps per merge, and the header knows it

`KUI_ABI_VERSION` went 3 → 6 in one day, each bump documented in the header
with its reason. Correct under ADR 0006's rule, and since nothing has shipped
since alpha.5 a host sees one jump. But the header's history now reads as a
per-merge log, and ADR 0006 says nothing about cadence. One sentence there —
bumps coalesce within a release window, or they do not — settles it. No chip.

## Suggested sequence

Rewritten 2026-09-05. The original six-step order is history now: every
step in it shipped, and the strikethroughs it had accumulated said less
about what to do next than about what had been done. What is left, in the
order the dependencies run:

1. **C11 step 3 — the declared window set.** The first step that changes
   behaviour and the first that breaks C (`kui_take_window_commands`
   becomes a `KuiWindowCommand` out-param, under ADR 0006's size rule, so
   `KUI_ABI_VERSION` moves again). Both A5 warnings land here because both
   live in the diff. Steps 1 and 2 exist so this one can be reviewed on
   its own.
2. **C11 step 5 — `SetSize` / `Focus`.** Independent of step 3: `WindowId`
   is plumbed, `MAIN` is 0, and the commands queue the way `reveal` and
   `play` already do. Small, and it can land before or beside step 3.
3. **C11 step 4 — popups.** After step 3, since a popup is a window kind the
   declared set opens. Its OS-surface behaviour belongs to P8's smoke jobs,
   not the corpus; the corpus can still pin the `dismiss` event and the
   non-activating focus route headlessly.
4. **C12, C13, C14, C5(b)** stay parked on their own terms — each waits for
   a view that needs it, and each says so.

The next *chip set* should not come from this list. Six rounds have added
roughly twenty thousand lines — modal surfaces, composites, exit
animations, a session, ABI versioning, Node `env`, C as an extension —
and none of that surface has had the full review the original 36k lines
got. A fresh sweep of the grown surface is the input worth having; the
first review found its defects in the gap between what the schema
enforced and what the composites hand-wrote, and the same question asked
of the new surfaces is where the next set will be.


---

## Release 0.1.0-alpha.6 (2026-09-05)

Goal set 2026-09-05: tag alpha.6, finish what is half-baked before it, and
plan what follows. State at the decision: 542 Rust tests, 59 Node, 14 corpus
scenes in four adapters, the plugin host green, all examples building,
`KUI_ABI_VERSION` 6, 48 findings filed across seven rounds and 43 closed.

### What ships

Since alpha.5 (`93169ed`, 127 commits, +41k / −7.8k lines): modal surfaces
(ADR 0003), composite keyboard patterns (ADR 0007), exit animations, group
opacity, drop shadows and rounded clipping (ADR 0005), flex wrapping, a
`Session` and the declared window set with `SetSize` / `Focus` (ADR 0004
steps 1–3, 5), a versioned C ABI with caller-bounded out-params (ADR 0006),
C as an extension, secondary mouse button and `onContextMenu`, `KeyUp`,
derived cursor shapes, `reveal` / `set_scroll` / `scroll_geometry`,
`selected` / `expanded` / set positions, `initialFocus`, Lua focus verbs,
Node `env`, unknown-prop warnings, a generated warning-code union, and the
binding-parity corpus that pins all of it.

### Half-baked — finish before the tag

- `!` **R1 — README "Status / next" is stale and unreadable.** It still says
  "arrow keys inside radio groups, tab lists and lists … and initial focus
  inside a dialog, are the next steps" (README.md:800) — ADR 0007 and A2
  shipped both. And it has grown into wall paragraphs with no structure; a
  reader evaluating alpha.6 cannot find what is and is not there. Rewrite as
  short grouped paragraphs (paint · layout · input · focus/a11y · text ·
  audio · windows), each naming the limit and the ADR or backlog id behind it.
  **Done (2026-09-05, `570e8fb`).** Seven groups under bold labels, each limit
  carrying its ADR or backlog id, with the tone and the short reasonings kept
  where they were still true. **The re-read found a second stale claim, and it
  was not this entry's.** "No physical scancode … so a keymap cannot bind a
  position on the board (WASD on AZERTY is ZQSD)" was contradicted by
  `KeyPress::physical`, which ADR 0002's decision 11 added and `docs/props.md`
  has documented since; the section now says the position is there and the
  modifiers are what still carry no left/right distinction. Every other limit
  was checked against the code rather than assumed — `MouseButton`,
  `CursorShape::ALL`, `Dir`, `Align`, `Clip::intersect`, `HitRegion.clip` (a
  `Rect`, so hit-testing really is square), `depart::MAX_NODES`, `QuadKind`,
  `StaticSoundData`, and the absence of any gradient or z-index — and one
  clause was tightened rather than deleted: nested rounded clips take the
  tighter cut per corner, and only a corner an ancestor's straight edge crosses
  goes square, where "the corners neither of them moved" implied both are
  dropped. R3's half of the work is in the same commit: a **windows** group
  naming `Normal` / `SetSize` / `Focus`, the missing `Popup`, and
  `FloatConfig::fit` plus a `modal` float as the in-window approximation. R3's
  other half — ADR 0004's Consequences — is not done here.
- `!` **R2 — The alpha.6 changelog section is 1,571 lines and 87 bullets.**
  alpha.5's was 162. The detail is right and should stay, but nobody reads a
  1,500-line release note; put a **twenty-line headline summary** at the top
  of the section (what changed, what breaks — the C ABI and the Lua
  `focus` / `focused` pair — and where the detail is), and make sure "What
  you can delete" is complete for the big three: the scrim `onClick` box a
  modal replaces, the hand-rolled tab-stop juggling a composite replaces,
  and per-window resource registration a `Session` replaces.
  **Done (2026-09-05, `2dbdb81`).** A ~30-line headline block sits under the
  heading, before `### Added`: one paragraph of what alpha.6 is, an explicit
  **What breaks** (the ABI 2 → 6 with the `kui_take_window_command` rename,
  `kui_env_set_window`'s leading window id and the three grown [out] structs;
  Node's five removed transport entry points, D2; the `focus` / `focused`
  pair), and a line pointing at the detail and at this file. Of the big
  three, the scrim bullet was already there; the composite one and the
  `Session` one were not, and were added in the list's voice — C11 step 1's
  entry had said "nothing yet" because step 3 had not landed when it was
  written. No existing bullet was shortened or removed (85 insertions, 0
  deletions), and the heading is untouched for X2.
- `~` **R3 — Multi-window ships without popups (C11 step 4).** Decide it
  rather than let it be implicit: alpha.6 is multi-window with `Normal`
  windows, `SetSize` and `Focus`, and no `Popup` kind. Say so in Status /
  next and in ADR 0004's Consequences, and name the in-window approximation
  (`FloatConfig::fit` plus a `modal` float) as what a dropdown uses today.
  **Done (2026-09-05, `0154d87`).** ADR 0004's Consequences carry a dated
  amendment in the form the earlier ones use: steps 1, 2, 3 and 5 are built
  and step 4 is not, so `WindowKind` has exactly one variant and the release
  has no spelling of a popup anywhere. The three cases the ADR's Context
  names — a dropdown taller than the window, a menu opened near the edge
  with nowhere in-window to go, a panel the user wants beside the app — stay
  unbuildable until step 4, and decision 9 is unchanged *as a decision*: a
  popup is still a window kind that reuses `dismiss`, it is simply not
  built. `FloatConfig::fit`'s own doc comment now says the popup it points
  at is a later release, so a reader who follows the pointer is not left to
  work that out. **Status / next needed no edit** — R1 had already written
  the Windows group per this entry, and every claim in it was re-read
  against the code rather than trusted.
  **The half that was a real hole, not a missing sentence, is that the kind
  was absent silently.** `WindowKind` has only `Normal` and JSX/Lua
  `windows` entries have no `kind` key, but `KuiWindowConfig.kind` is a
  `uint32_t` and `window_config_of` mapped *every* value to `Normal`, with a
  comment saying it did so deliberately — so a C host that asked for a popup
  got a window and no word about it. It still gets the window (a host built
  against a later header should degrade to a window rather than to nothing)
  and now also gets **`unknown-window-kind`**, the fifth code that does not
  come from the tree walk; `diag`'s module doc, which C11 step 3 took from
  three to four, says five. JSX and Lua go the other way and *refuse* a
  `kind` key: a `windows` entry is plain data with a fixed shape by ADR 0004
  decision 5, not a node's loose prop bag, so a key that does nothing is an
  error rather than a dropped declaration — and neither is in C's position
  of not being able to throw. Tests in kui-ffi (the unknown kind opens and
  warns; the one kind the header has does not), kui-lua and `test.mjs`;
  `npm run gen` carried the code into `index.d.ts`'s `WarningCode` union and
  `docs/props.md`, and the `windows` row's doc now says there is no `kind`
  key and what to reach for instead. C11 step 4 stays open and says it is
  the first build item after the release, which is where **After alpha.6**
  already puts it.
- `!` **R4 — Nothing has run natively on macOS or Windows since alpha.5.**
  P8's smoke jobs are gated on repository variables and this instance has
  only the `docker` runner, so the release's five prebuilds have been
  cross-compiled and never executed on-platform. alpha.6's headline features
  — modal `aria-modal`, composites, `orientation`, `set_selected` — are all
  AccessKit-bridge behaviour that only a native run exercises. Before the
  tag: `cargo test --workspace` natively on the Mac (P8 recorded it known
  good), `scripts/ax-audit.swift` against `examples/rust/accessibility.rs`
  (it gained radio-group and menu fixtures in `0ae02f2`), and whatever a
  Windows machine can give; **record what ran and what did not in the
  release notes** so the gap is stated rather than assumed.
- `.` **R5 — Version and generated files.** `scripts/set-version.sh
  0.1.0-alpha.6` moves Cargo, npm and the changelog heading (X2). Then `npm
  run gen` and `git diff --exit-code` on the three generated files — parallel
  worktree merges left them stale once this round (`80baacc`), and the CI
  guard catches it, but catch it before the tag commit, not after.
- `.` **R6 — The README benchmark table predates most of what ships.** The
  numbers were measured before exit animations, opacity, shadows, rounded
  clipping, wrapping and composites touched the hot path, and half the
  benches are given as ratios "from a different (slower) machine". Re-run
  `cargo bench -p kui-core` on the M-series machine and refresh the table so
  the numbers describe the release.
  **Done (2026-09-05), and it found a regression.** Measured on an Apple M3
  Pro MacBook Pro, macOS 26.6.2, rustc 1.98.0, release, two consecutive runs
  agreeing within ~3% and the second one read. All nineteen frame benches are
  now absolute medians from that one machine, so the "ratios from a slower
  machine" paragraph is gone and the four cross-machine ratios are restated
  as differences between rows in the same table; the editing bench and the
  three list benches are in the section as well.
  **The regression is the reason this took a bisect rather than an edit.**
  All four old rows were not merely stale, they were 2.4–2.7× better than
  what HEAD measures: `frame_10k_rects` 516 µs → 1.37 ms,
  `frame_1k_typical` 72 → 175 µs, text-and-hits 754 µs → 1.81 ms, deep
  nesting 60 → 122 µs. Benching `dabe671` in a throwaway worktree on the same
  machine reproduced the published table to within 2%, so the old numbers
  were sound and the machine is not the difference. Nothing was dropped and
  nothing was quietly improved: the section stated the regression, and
  **C15** carries the per-commit bisect (a staircase across ~10 feature
  commits, none individually large), the profile and the experiment that
  found the cause — `NodeSpec` at 728 bytes against 152, moved by value per
  node, putting 31% of a frame in `memmove`. **That cause was then fixed
  before the tag** (C15): the cold fields are boxed, `NodeSpec` is 224 bytes,
  and the table above is the post-fix measurement — `frame_10k_rects` 788 µs
  against the 1.37 ms this item found and the 516 µs it started from. The
  README section records the whole arc rather than only the good number. The editing bench did *not* regress — its frame is 1860 quads,
  not 10k nodes — which is itself evidence that the cost is per node.
- `.` **R7 — Deprecation notice for Lua's `env.focus`.** P3 kept it beside
  `env.focused` because alpha.5 had shipped it. alpha.6 is where the
  changelog says: `env.focus` (the value) is deprecated, `env.focused` is
  the reading, and 0.2 removes it.
  **Done (2026-09-05, `2dbdb81`), with one correction.** A `### Deprecated`
  section between `### Changed` and `### What you can delete` announces it.
  But "`env.focused` is the reading" is not true *today* and the entry could
  not say it was: `env.focused` is `Env::focused`, the **window**'s keyboard
  focus as a bool, and P3's whole finding was that Lua cannot converge on
  that name inside 0.1 without leaving the node key unreadable. So the entry
  promises no change at all for 0.1 — no warning, no edit needed — and puts
  the convergence at 0.2, where `focused` becomes the node reading in every
  binding and the window fact takes an unambiguous name of its own. That is
  the removal this item and the hygiene list both ask for, with the second
  half of the rename named rather than assumed.

### Pre-tag run list

`cargo fmt --all --check` · `cargo clippy --workspace --all-targets -- -D
warnings` · `cargo test --workspace` · `cargo build --workspace --examples` ·
conformance reference + C adapter + `c_panel --headless` +
`KUI_CONFORMANCE_REQUIRED=1 npm test` · `npm run typecheck` in
`examples/node` · `scripts/check-version.sh 0.1.0-alpha.6` · R4's native
runs · then commit, `git tag v0.1.0-alpha.6`, push the tag.

## After alpha.6

Grouped by kind, not urgency. Nothing here blocks the tag.

**Build.** C11 step 4, `WindowKind::Popup` — the one multi-window step
left, and the case (`fit` cannot place a dropdown taller than the window)
that forces it. ADR 0004 step 1's leftover: the glyph atlas and shape cache
are still per window because `Core::output` hands out `&mut GlyphAtlas`
(see `kui-session-atlas-constraint`); it waits for a case where two windows
share enough text to matter. C15's remainder: `frame_10k_rects` is still
~1.5× its 2026-08-31 cost after the boxing fix, and that half is the per-node
logic rather than the struct — it wants a profile now the cache behaviour has
changed. Plus the CI threshold on `frame_10k_rects` and `frame_1k_typical`,
which the size test does not replace.

**Design, each wanting an ADR.** Live regions and announcements (ADR 0001's
open follow-up — an event on a timeline, not a tree property). The exit
animations' `animating()` policy, revisited against a real view that removes
many nodes (ADR 0005 left it opt-in + a 512-node budget with no duration
cap). S8: one sentence in ADR 0006 on bump cadence.

**Rows, when a view asks.** A configurable focus-ring colour (README names
it). `required` / `invalid` and heading `level` (ADR 0001 follow-ups).
Per-button `on_click` and middle-button routing (C2 left them "reach the
core and route nowhere"). Physical key positions beyond what `60ca137`
carried.

**Parked on their own terms.** C12 (column wrapping), C13 (`space-between`
and baseline), C14 (aspect ratio), C5(b) (core-side virtualisation), rounded
clip nesting and rounded hit-testing (Status / next names both). Each says
"wait for a view that wants it", and each should keep saying it until one
does.

**Hygiene.** Archive the closed entries of this file into
`docs/backlog/closed-2026-09.md` and keep `BACKLOG.md` to open items and
the sequence — 2,500 lines is the record but not a working list. Enable
`SMOKE_MACOS` / `SMOKE_WINDOWS` the day a runner exists. Remove Lua
`env.focus` at 0.2.
