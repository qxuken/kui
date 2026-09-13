# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
the six rounds that followed it, and the field reports from two apps built on
alpha.6 through alpha.11 outside this repo (F1–F15 on 2026-09-06,
F16–F23 and F25–F31 on 2026-09-07, F32–F35 on 2026-09-08, F37–F41 on
2026-09-09, F42–F49 and F50–F54 on 2026-09-12). Every item names the
evidence that produced it, so a task that turns out to be wrong can be argued with rather
than guessed at.

**This file is the open list.** The hundred and ten closed entries — each with its
outcome written on top of the original finding, and the tables, profiles and
evidence it argued from — are in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md); forty-six moved there
on 2026-09-06, the remaining ten field-report entries followed the same day
before the alpha.7 tag, W2 went whole on 2026-09-07 when ADR 0009's driver
half was built, fourteen more cut alpha.9 on 2026-09-08, the fourteen of
this round — W3, W4–W12 and F32–F35 — went before the alpha.10 tag, F37
followed it the next day, filed and closed after the tag, and AR1–AR6 from
the architecture review of 2026-09-11 went the same day they were filed. The index
at the bottom of this file names every one of them, so an id cited by an open
item, a code comment or a commit message can be resolved without opening the
archive. Nothing was renumbered in any of those moves, and nothing ever is.

What is left here: F50–F54 from the LCARS pomodoro's devtools round, filed
2026-09-12 and built the same evening (the panel painting the accent as
ink, its "app" base being the OS's, its window inheriting the launcher's
chrome, `pick` leaving the keyboard in that window, and the app's floor
not counting the dock); F42–F49 from the two alpha.11 upgrade reports, filed
2026-09-12 and **all eight built the same day** (two defects — a foreign
`dispatch` losing a `setEditText` seed to the redraw the call asked for,
and `env.viewport` reading the window against its own schema row — the
label editor's `wrap`, and five wishes, two of them carried unanswered
from alpha.10); they stay here with their outcomes on top until the
alpha.12 tag moves them. T5 the same day (derived tokens, what
building ADR 0027 left out — **built 2026-09-13** under
[ADR 0028](adr/0028-derived-tokens.md)), C30, filed 2026-09-12 (always on top — a window level
no binding could ask for — **done the same day**, as the per-frame fact
the entry chose, with `env.window.always_on_top` reporting what the
platform did), AR7–AR49 from the second architecture review, filed
2026-09-13 and **none built yet** — forty-three entries under ten
decisions, twenty of them defects, the audio store reconciled against
every window's frame and the two key channels disagreeing about a
chord at the top — with the amendment on B1 whose condition that round
met; C31, found
the same day building the tokens (Node's corpus adapter disagreeing with
the reference on one scene when run alone — **done the same evening**:
the handle was raw 1, the first the mint hands out, and the fixtures now
remove one instead), C29 from the alpha.11 pre-tag round (four unguarded
bench rows reproducibly slower than alpha.10 — **swept the same
evening**, 63 commits on four rows: one step at ADR 0023, half of it
taken back, the row guarded), W13 from the Windows half of that round
(`npm run gen` writing a different `props.md` on Windows — **done the
same evening**, the row carries both platforms' values), V2–V8 from the
canvas question of 2026-09-11 (five
waiting for a view, two declined with a condition — V1, the one with an
order attached, was built the round after, on 2026-09-11), three parked
headings — C12, C13 and C14, each waiting for a view that wants it — B1
from the ABI-and-bindings audit of 2026-09-10 (a table declined with the
condition that would build it), and F36, which fell out of building the
last two of the four entries the two alpha.9 field reports and the
bake-off produced, and which is filed rather than built on purpose (the
only host driving its own audio device today is the runner). Everything
else that has been filed has shipped: the six the round of 2026-09-11 took
together — V1, D1, D2, T2, C26's last two steps and E3, each in the
archive under its round with what the building settled on top; the
design-system audit's T1 (the defect) and T2 (the axis ADR 0019 scoped
itself out of) are both closed now, T3 the day it was filed, and T4 —
tokens beside the theme, ADR 0027 — was written and built on 2026-09-12; AR1–AR6 from
the architecture review of 2026-09-11 were all built the same day; F16–F23
from the two alpha.7 field reports closed the day they were filed
(2026-09-07), F25–F31 from the alpha.8 ones by the day after (F27 last, on
2026-09-08), C16–C23 landed whole for alpha.9, and W3–W12 and F32–F35 for
alpha.10. C26 was the last split entry, and it closed on 2026-09-11.

Ordered by area, not by priority. What to do next is under "After alpha.11".

**Legend** — `!` a defect that ships today · `~` a gap with no workaround ·
`.` cost without correctness risk.

---

## Core capability

Composition-over-traits keeps widgets reachable from Lua and C, and it holds for
buttons, titlebars, tooltips and the latency HUD. It stops holding wherever the
widget needs state the core owns exclusively: pointer capture, the Tab ring,
retained scroll offsets, paint order, the OS window.

**The test used here:** could an app author build it from what the bindings
expose today? If no *because the core keeps the state privately*, it belongs in
the core. If no because the data isn't in the IR, it needs a schema row first.

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

### `~` C27 — An idle pumped window costs ~1 ms a pump, and every way out of it is worse

**Parked, measured and closed as "nothing to build" on 2026-09-09.** The
entry began as "park the pump and wake it on libuv's backend descriptor",
which would have been three native run-loop integrations whose failure mode
is a hang. Measuring first killed it twice over, and the measurements are
the point of keeping the entry.

**A pump is ~1 ms of real CPU, and it is the door, not the room.** M3 Pro,
macOS 26.6, marginal (two rates, background subtracted):

| | cost |
| --- | --- |
| `PumpRunner::pump` from outside, pure Rust | **~1.0-1.7 ms** a call |
| the same through Node's addon | **~1.2 ms** a call |
| a `ControlFlow::WaitUntil` wake *inside* `run_app` | **~46 µs** |

Not rendering — 800 pumps on an idle window drew **0 frames**. Not the
binding — `pollEvents` + `warnings` + `animating` + `app.step()` are ~1 µs a
turn together. Not other threads, and not blocking: bracketing a pump with
`CLOCK_THREAD_CPUTIME_ID` puts **987 of 1011 µs on the main thread**, awake.
An idle `run_app` window is 0.00% and one context switch a second, so the
loop is free and the bill is entirely leaving and re-entering it.

`sample` is honest here *because* the thread is running, and it splits one
pump three ways: ~14% winit's `stop_app_immediately`, which breaks out of
`[NSApp run]` by posting a synthetic `NSEvent` and waking the run loop
through `mach_msg`; **~20% AppKit's `_NS_SetBasicPasteTelemetry` →
`_LSCopyFrontApplication` → a LaunchServices `sysctl`**, dragged in by
dequeuing that very synthetic event; and the remaining ~66% the machinery of
one `[NSApp run]` entry and exit. So a third of it is the break-out
mechanism and two thirds is the door itself. There is no 10× upstream fix
hiding in it.

**And parking does not help, which is what the entry was built on.** winit
0.30's macOS `pump_events(Some(t))` sets `stop_after_wait`, and `wakeup`
stops the app on the *first* wake of any kind — so an unrelated run-loop
wake ends the park and the caller pays a full re-entry to resume it. A
200 ms park runs **101 ms** on average (min 172 µs, max 203 ms). Against
polling at the same period:

| period | polling | parking |
| --- | --- | --- |
| 32 ms | 30 pumps/s, **1.80%** | 49 pumps/s, **4.59%** |
| 200 ms | 5 pumps/s, **0.70%** | 11 pumps/s, **0.70%** |

Parking is never cheaper and at the periods a driver would actually use it
is 2.5× dearer. What parking *buys* is latency — the wake is the event, so
the period stops setting the response time — and it costs libuv, which
cannot be made safe from Node: `process.getActiveResourcesInfo()` and
`process._getActiveRequests()` both report nothing for an in-flight
`fs.readFile`, and behind a 50 ms park 200 sequential `await readFile` went
from 6 ms to 4 s. Adding a libuv wake source to a park that is already being
woken spuriously would have bought nothing and risked a hang.

**So the driver polls with a backoff and that is not a compromise, it is the
best of the three.** `idlePumpMs` is a real dial — 8 ms is 34 ms a click and
~9% of a core, 32 ms is ~55 ms and ~3% — and only the first event after
`quietMs` of silence pays it.

**What would actually move it,** in order of how likely it is to happen:

- **Upstream winit.** Two separate things: resume the park after a wake that
  is not ours instead of stopping on the first one, and break out of
  `[NSApp run]` without posting an event AppKit answers with a `sysctl`.
  Together they are the difference between "polling is the best option" and
  "parking is free". winit 0.31 is in beta; the pinned 0.30 is deliberate.
- **Owning the loop.** `uv_run(loop, UV_RUN_NOWAIT)` from winit's
  `about_to_wait`, capped by `uv_backend_timeout` — portable, no
  per-platform code, ~0.5% at 100 Hz. Blocked because `uv_run` may not be
  called from inside a libuv callback and `runWindowed` is called from JS,
  which is inside one: taking the loop means a launcher binary that embeds
  Node. That is Electron's shape and it is not worth an idle 3%.
- **Nobody minding.** The most likely, and the reason this is parked rather
  than open. A Rust window idles at 0.00%; a Node one at ~3% with a knob.

### `~` C30 — Always on top — **done (2026-09-12)**

**Done (2026-09-12), as the per-frame fact.** `Core::set_always_on_top` /
`always_on_top()` beside the title in `runtime/windows.rs`, cleared each
`begin_frame` to `false` — the one place it departs from the title's
`Option`, and on purpose: the undeclared reading is the lowering, so the
pin button toggles the app's flag and undoes nothing. `Ui::always_on_top`,
a root `alwaysOnTop` in JSX (`P_ALWAYS_ON_TOP`, a `CUSTOM` row beside
`title`, flag-shaped on the wire like `keyFocus`), `always_on_top = true`
on a Lua root table, `kui_set_always_on_top` in C, and
`PropsOut::always_on_top` so `configure_root_from` applies it for every
binding at once. The runner keeps `Pane::applied_on_top` and
`pane::level_change` decides the call: `set_window_level` once per change,
never per frame, and never for a popup (its record starts true and is
left alone), which is the by-hand defect the entry named, pinned as a
unit test instead. The readback is `WindowEnv::always_on_top` — an
`ENV_FIELDS` row, so Node's `env()` and Lua's `env.window` grew the key
without a line of their own, `setEnv({window: {alwaysOnTop}})` and the
devtools' window row followed — written by `sync_env` from the runner's
record **and** `Pane::level_supported`, which the raw handle answers
(`RawWindowHandle::Wayland` is the one backend winit has no level for),
so on Wayland the app asks, nothing moves, and the reading says false.
Two readers for hosts driving their own window: `kui_always_on_top_get`
and Node's `ctx.alwaysOnTop()`. Corpus: `build_chrome` declares it, so
`chrome` and `chrome-inset` claim the row across all four adapters, and
every report carries an `always-on-top 0|1` line after `title`. The
titlebar example gained the pin, and the Windows check is in the
changelog (`WS_EX_TOPMOST` set on pin, cleared on unpin, the fact
following both). **Two departures from the entry.** `KUI_ABI_VERSION`
stays 15: the three C doors are new prototypes, and `abi.rs`'s own rule
says a new function does not bump ("a host that does not call one is
unaffected, and one that does fails to *link*, which is loud") — the
entry's "moves from 15 to 16" reasoned from the pin, not the rule. And
C's report is its own additive setter, `kui_env_set_always_on_top`,
rather than a seventh argument on `kui_env_set_window`, for the reason
`kui_env_set_assistive` already argues in the header: an argument is the
signature break the ABI-12 note warns about, a setter is off the version.
No `Launcher::always_on_top()`: with the fact defaulting false, a builder
flag would be undone by the first frame unless it were OR'd in, and a
Rust app that never toggles writes `ui.always_on_top(true)` at the top
of `view` instead — one line, in the one place the fact lives.

Filed 2026-09-12. A window an app wants kept above every other app's
windows — a floating palette, a picture-in-picture player, a timer, a
pinned note — has no way to ask for it: the OS level is
`WindowLevel::Normal` for every surface the runner opens except a popup,
which is *already* `AlwaysOnTop` by construction
(`crates/kui/src/windows.rs:131`), so the driver seam exists and is
exercised on every combobox; what is missing is the row, the doors and
the decision of where it lives. Nothing in four bindings reaches it, and
a `Popup` is not a workaround — non-activating, undecorated, closed with
its owner.

**Where it lives.** Two shapes are on the table and the title settles
which:

- **On the opening edge, as a `WindowConfig` field** beside `kind`,
  `size`, `activates`, `anchor`. Wrong for the thing every app that wants
  this draws next: a pin button, which is a *toggle*. `WindowConfig` is
  read once and never again by design ("the user owns a window's geometry
  once it exists"), and a level is not geometry the user owns — it is
  state the app owns, so the opening-edge rule does not apply to it.
- **As a per-frame fact the way `window_title` is** —
  `Core::set_window_title` (`runtime/windows.rs:263`) is a value the
  frame declares and the driver applies when it differs from the last
  frame's. A `Core::set_always_on_top(bool)` in the same shape, defaulted
  to false, read by the runner after each frame and applied through
  `winit::Window::set_window_level` on change, is a toggle for free and
  costs a frame nothing when it does not change. **This is the one to
  build.** The main window gets it from the same door, so the `Launcher`
  needs no builder method — though `Launcher::always_on_top()` for a
  Rust app that never toggles is one line and harmless.

**Doors.** The title's doors are the template: the `window_title` row in
Node's window options / `setWindowTitle`, Lua's `title`, and C's
`kui_set_window_title` — one each, spelled `alwaysOnTop` /
`always_on_top` / `kui_set_always_on_top(ctx, bool)`. The C door is a
new prototype, so the generated TU pins it (`abi_fn!`) and
`KUI_ABI_VERSION` moves from 15 to 16; the size handshake absorbs nothing
here because no struct grows. `env.window` should report it back the
way it reports `maximized`, so a pin button draws its state from the
window and not from the app's guess — the OS can refuse or drop the
level (a fullscreen space on macOS, a tiling manager on X11), and a
report keeps one frame.

**What the platforms do with it.** `NSWindowLevel` floating on macOS,
`HWND_TOPMOST` on Windows, `_NET_WM_STATE_ABOVE` on X11 and nothing on
Wayland (winit 0.30 documents `WindowLevel` as unsupported there, and every level as "just a hint to the OS") — so the Wayland
build is the case where the report matters: the app asks, the window
does not move, `env.window` says so. A popup's level must stay what it
is regardless of its owner's; an owner pinned above everything with a
dropdown open under it is the visible defect a naive "apply to every
window" would ship, and the corpus cannot see it (a headless `Ctx` draws
only main) — it goes on the by-hand round beside the popup drag.

**Test.** A `runtime::windows` pin that the fact round-trips per window
and defaults false; the `ENV_FIELDS` readback for the new `env.window`
field across Lua/Node/C; a runner check that a toggled fact reaches
`set_window_level` once per change and not per frame.


### `!` C31 — Node's corpus adapter disagrees with the reference on `fragments`, when it runs alone — **done (2026-09-12)**

**Done (2026-09-12), and the handle was raw 1, not raw 0.** The entry
below points at the fragment's `src: 0`; that one is dead in every
process — `from_ffi(0)` is index 0 at generation 1, and index 0 is the
slot the mint never fills (slotmap's sentinel; `free_head` starts at 1),
so it misses everywhere and is nobody's. The line the Node report
carried came from the *image* three fragments later, `image:
'0000000000000001'`, which the reference wrote as `ImageId::from_ffi(1)`
with a comment calling it "index 1 at version 0, which no live slot ever
has". `from_ffi` ORs the generation with 1, so raw 1 is index 1 at
generation 1 — exactly the first key a fresh process's mint hands out.
The reference, Lua and C never saw it because the session that minted
it (the first scene's) is dropped before `fragments` builds; a Node
process keeps every `Ctx` the GC has not collected, so in the filtered
run the first scene's icon was still live in its session and the miss
was, correctly, *foreign*. In the full suite an earlier test's session
had minted and dropped index 1 at generation 1, and the same lookup was
a removal. So the corpus assumed a number was dead that no number is.

The fix is what a dead handle actually is: `Fixtures::dead` is a sixth
image, registered after the sampler and removed in the same call —
mirrored in `conf_fixtures` (`kui_image_add` + `kui_image_remove`) and
Node's `addFixtureDead` (`addImage` + `removeImage`); Lua takes the
struct's field. All four adapters match the reference on every scene,
the Node corpus test passes alone and in the suite, and
`resources::tests::raw_zero_is_dead_whatever_was_minted` pins the half
of the original claim that holds: raw 0 misses in every session and
records no foreign hit, whatever was minted before — which is what the
scene's dead `src` and the doors' "no image" both rest on. Nothing on
the wire changed; nothing an app wrote is affected.

Found 2026-09-12 while adding the `tokens` scene, and confirmed to predate
it at `af043d4` (the adapter run in a clean checkout of that commit,
against its own addon and dump). `packages/kui/test.mjs`'s "every corpus
scene lowers the way kui-core does" fails on the `fragments` scene alone
**when it is the only test that runs** (`--test-name-pattern`), the Node
report carrying one line the reference does not — `warn foreign-resource`
— after the `fragment-params-truncated` both sides raise; the same test
passes inside the full suite, which is what CI runs, so nothing is red
today. The scene's fourth fragment names a dead handle
(`src: '0000000000000000'`, raw handle 0); the reference's dead
`FragmentRef` draws nothing and warns nothing. That the answer depends on
what ran before in the process points at `resources::Mint`'s
process-unique handles: in a fresh process nothing has been minted and
handle 0 reads as another session's, after other tests have minted it
does not. The Lua and C adapters match the reference on every scene,
`fragments` included.

**Do:** decide what a raw handle 0 *is* under ADR 0025's rule for a dead
handle — "no image", never "foreign" — and make the Mint say so whatever
was minted before; then the scene's `expect.warnings` pins it and the
filtered run passes like the full one. Not this round's: it was in the
way before the tokens, and only in the way of a developer running one
test.

## From two alpha.7 field reports (2026-09-07)

Both apps that reported on alpha.6 upgraded to alpha.7 and reported again:
the LCARS pomodoro's upgrade notes (`kuialpha7.md`, five numbered wishes)
and the mind map's rewritten `FINDINGS.md`. Each claim below was checked
against `main` at `980bca4` before it became an entry, and two of the
wishes did not survive the check — they are under "Theirs, not ours" at the
end. The theme of both reports is the same sentence: none of what surprised
them was a type error or a failed access-tree assertion, every one was only
visible in the pixels. **All eight, F16–F23, closed the same day** and are in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-two-alpha7-field-reports-2026-09-07)
whole — F23 as ADR 0013, accepted and built for alpha.8 rather than left
for a view. What stays here is the two wishes that were not ours.

### Theirs, not ours

- **Wish 5 (per-surface font handles).** `init(surface)` runs after
  `setup` and can put the font id in the model; `addSystemFont` is
  idempotent per family. The module global both apps carry is a choice
  the alpha.7 `init` made unnecessary, and the pomodoro's report says as
  much under "Ours to fix".
- **`key()` on `Ctx` only.** By design and closed as F6: a window's keys
  come from the OS.
- **`onLayout` firing on every rect change.** That is what the row says
  it does; a first-frame-only hook has no view asking for it yet.

## From two alpha.8 field reports (2026-09-07)

Both apps upgraded to alpha.8 the day it was tagged and reported again:
the mind map's `FINDINGS.md` (an `alpha.7 → alpha.8` section on top of the
alpha.7 one) and the LCARS pomodoro's `docs/findings.md` — a review of
the release's DX rather than of the app — beside its upgrade notes,
`docs/kui-alpha-8.md`, whose "Wishes for the next alpha" are numbered 1–4.
Each claim below was checked against `main` at `19ead84` (the alpha.8 tag)
before it became an entry, and every command the pomodoro ran was run
again from its own directory. Both reports agree on the release: "the
bare version bump broke nothing", `setTime` throwing is "the single best
thing in the release", and the changelog in the tarball made the reports
"materially cheaper to write". What they found is smaller than last
round's and further from the core: two of the seven entries are the
package's distribution, two are doc sentences, and the two that touch the
core are about state the core keeps by key — an editor's and a
playback's — and when it lets go of it.

Three claims did not survive the check whole, and the entries say so: the
pomodoro's fix for caret ranges (`~` floats exactly as `^` does), the mind
map's "there is no shorter way round" (there is none today, and F25 is
it), and the pomodoro's "a window has no equivalent" of `runOut` (the
primitive is there; the promise is not). One entry, F26, is not in either
report: it fell out of checking F25. **Six of the seven, F25, F26 and
F28–F31, closed the day they were filed** and moved whole to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-two-alpha8-field-reports-2026-09-07)
on 2026-09-08, cutting alpha.9; **F27 followed the same day**, once that
release's publish ran the step it built and the registry answered `latest`.
What stays here is the three claims that were not ours.

### Theirs, not ours

- **Per-surface resource handles** (pomodoro wish 1, carried from its
  wish 5). Same answer as last round: `init(surface)` and `view(_, _,
  surface)` reach the surface; the module globals are the app's, and its
  own notes say "our half of this is below".
- **The mind map's `preview.svg` is gitignored**, so the tool it built to
  catch paint regressions has no baseline. The report says it is the
  app's work, and it is.
- **`withEffects` for the chime.** Both reports declined it, for the
  reason ADR 0013 gives.

## From the editor-and-mux assessment (2026-09-07)

The question asked of `main` at `51ed27b`: is kui ready to be the backbone
of a performant editor that opens large files, and of a terminal
multiplexer? The answer was yes to the editor, provided the app owns the
document and hands kui the visible lines (the `modal_editor` and
`syntax_view` shape), and no to the mux, for reasons that are one cache
policy and a few missing primitives rather than the architecture. Each of
the eight entries below names the measurement that produced it. Two
sources: `cargo bench -p kui-core --bench highlight` as it stands in the
tree, and a scratch program against `kui-core` that is **not** in the repo
— fifty rows of two hundred columns at 13 px mono, 18 px line height,
1920×1080 at scale 2, one frame = build + layout + emit, no GPU. C16's "Do"
adds it as a bench so the numbers stop being a one-off. Apple M3 Pro,
release, 2026-09-07:

| frame | cost |
|---|---|
| `highlight_2x55_warm` — two panes × 55 highlighted code lines, nothing changed | 160 µs |
| `highlight_2x55_typing` — one line retyped per frame | 228 µs |
| `highlight_2x55_scrolling` — one line per frame scrolls in | 282 µs |
| `highlight_2x55_cold` — every run on screen is new (a file opened) | 1.65 ms median; one sample of thirty at 101 ms |
| `highlight_2x55_warm_rich` — each line one `Span` list instead of ~8 nodes | 81 µs |
| 50 × 200 terminal, one text node per line, warm | 0.08 ms, 9.9k quads |
| 50 lines × 10 coloured runs, each under its own `bg` box, warm | 0.18 ms, 10.4k quads |
| 10k cells as one text node each — the strict grid | 1.8 ms, 10k quads |
| 50 new log-like lines every frame (thirty-word vocabulary + numbers) | 21 ms |
| 50 new random-ASCII lines every frame | 63 ms — cosmic-text alone on the same fifty lines: 62.6 ms `Advanced`, 45.8 ms `Basic` |
| resident memory: at start / after ~135 streaming frames / after 1500 more log-like frames / after 600 random | 6 MB / 747 MB / 2.8 GB / 3.6 GB |

What the table says: a screenful of code costs a fraction of a
millisecond warm and under two cold, which is the editor's case; a
terminal pane whose fifty visible lines are *all new every frame* costs
one to four frames of shaping, and that cost is the shaper's, not kui's
(the cosmic-text-only row is the same number). The memory row is kui's
own, and it is C16.

**All eight, C16–C23, were built between 2026-09-07 and 2026-09-08** and
moved whole, with this table, to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-the-editor-and-mux-assessment-2026-09-07)
on 2026-09-08, cutting alpha.9. **W3 followed them on 2026-09-09**, once
the first Windows round below had run it, found it half right and rebuilt
it as a timer; nothing from this assessment is open.

## From the first Windows round (2026-09-08)

kui ran on a Windows machine for the first time. Windows 11 Pro 26200,
rustc 1.96 / MSVC, an RTX 5080 driving a 3840×2160 display at 239 Hz with
the desktop at 150%, and an idle AMD integrated adapter beside it. The
round was: `cargo test --workspace`, then every windowed example opened for
real, then the C examples built and run in every direction. Six defects,
none of which any headless test in the repo could have seen, and two of them
crashes on the first frame.

The round was a script from that day — `scripts/smoke-windows.ps1`, and
since 2026-09-11 (AR4) the `smoke` binary in `examples/devtools` that
replaced it and its unix twin — wired into `smoke-windows` in
[`.forgejo/workflows/smoke.yml`](../.forgejo/workflows/smoke.yml)
after the `cargo test` step. It leans on `KUI_SMOKE_FRAMES=n`, new in
`crates/kui/src/lib.rs`: the runner quits once the main window has
presented n frames, which turns every example into a self-terminating check
with an exit code — a wgpu validation panic is a failure with its stderr,
a window that never paints runs out the timeout instead of passing quietly.
14 examples, 120 frames each, about 1.5 s apiece; it was checked against
the bug it was written for by putting W4 back and watching it fail. The C
half was `examples/c/build.ps1` (since 2026-09-11 the `cbuild` tool in
`examples/devtools`, one program for every platform), a step of its own in
the same job.

This is what P8 said it could not offer ("On Windows there is no equivalent
tool and no plan for one"): it is not the tree-walking audit
`scripts/ax-audit.swift` is on macOS, and it judges that drawing did not
fail rather than what was drawn — but it needs no permission grant and no
human, and it found six things in one afternoon. P8's two follow-ups are
untouched: `windows_nc.rs` and `access_bridge.rs` still have no test.

**All nine, W4–W12, were built between 2026-09-08 and 2026-09-09** and
moved whole to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-the-first-windows-round-2026-09-08)
before the alpha.10 tag, W7 last — the one that wanted a decision rather
than a patch, settled as: a displacement moves a subtree by whole physical
pixels, scrolling included. Nothing from the round is open; what it did
not cover is in "After alpha.11" below.

## From two alpha.9 field reports and a bake-off (2026-09-08)

Both apps upgraded to alpha.9 the day it was tagged and reported again:
the mind map's `FINDINGS.md` (an `alpha.8 → alpha.9` section on top of the
alpha.8 one) and the LCARS pomodoro's `docs/kui-alpha-9.md`, whose "Wishes
for the next alpha" are numbered 1–4. Beside them, a measured comparison
of kui, iced 0.14 and gpui 0.2.2 — one counter and two stress apps per
framework, same machine, same day — whose kui-facing claims are checked
under "From the bake-off" below. Each claim was checked against `main` at
`e6d96ac` (the alpha.9 tag) before it became an entry, and the mind map's
probe commands were re-read against the Node binding's source rather than
re-run.

The two reports agree on the release: "a drop-in with nothing to change"
(mind map), "the bare version bump broke nothing" (pomodoro), and both
found a release the way F27 said they would — `npm view @qxuken/kui
version` answers `0.1.0-alpha.9`, which closes F27 above. What they found
is one gap in the core's key space, two documents that contradict the
release they shipped in, and two audio edges that `finish` made
askable. **The pomodoro's five ranked asks from alpha.8 all landed**, and
its own notes say three were cheap; the one that was not, `finish`,
"arrived better than what was asked for".

One claim bent under the check, in the direction the report did not
expect: the mind map says a first open is the case `setEditText` cannot
reach. It is worse than that — a *second* open cannot either, because
`key_of` resolves through the last frame and a closed editor was in no
recent frame. The app's per-node key cache is what makes its second
opens work, and the fix below has to seed a retained editor as well as a
new one.

**All four, F32–F35, were built on 2026-09-08** and moved whole to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-two-alpha9-field-reports-and-a-bake-off-2026-09-08)
before the alpha.10 tag. What stays here is F36, which fell out of
building the last two of them and is in neither report.

### `.` F36 — A host driving its own audio device can report an end and nothing else

Found reviewing F34 and F35 together, and in neither report. A host that
drains `take_audio_commands` and drives a device itself — which is the
whole point of audio being data — can answer `kui_audio_ended` (C,
`kui-ffi/src/resources.rs:359`) or `audioEnded` (Node,
`kui-node/src/lib.rs:730`) when a playback finishes. There is no
`kui_audio_truncated` / `kui_audio_refused` and no Node equivalent, so the
two new codes are raised only by the Rust runner's own kira backend.

Both codes nonetheless ship to a Node audience: `truncated-playback` and
`playback-refused` are in `docs/props.md`'s warnings table and in
`index.d.ts`'s `WarningCode` union, because both are generated from
`diag.rs` and that is the right thing for one table to do. So a Node reader
is told about two warnings their binding cannot produce, and a C host that
finds a stop landed on a live handle has nowhere to say so.

Not urgent: the only host driving its own device today is the runner, and
the codes are correct for it. It is an asymmetry in a door that already
exists, and the fix is the same shape as the door — two appended C
functions (no ABI bump, as C23) and two `#[napi]` methods, each calling the
`Core` method the runner calls.

**Do:** `kui_audio_truncated(ctx, playback, at)` and
`kui_audio_refused(ctx, playback)` beside `kui_audio_ended`; `audioTruncated`
/ `audioRefused` on Node's shared `core_methods!` macro beside `audioEnded`;
one test each driving the warning through the binding, the way
`crates/kui-ffi/src/tests.rs` drives `kui_audio_ended` today.

### From the bake-off

The comparison's kui numbers are its own and stand as reported: on the
animated grid kui holds 120 fps at 1.3× less CPU than iced and 2.5× less
than gpui, warm text is a wash with iced, cold text buys 41% CPU against
iced's 82% with a resident-set rise to 183 MB that is the 64 MB text
budget plus the atlas behind it (C16), and idle is 100 MB at 0% CPU.
Four kui-facing claims were checked against the tree:

- "kui links an audio engine, a clipboard and AccessKit into the default
  runner whether or not you use them" — two-thirds wrong.
  `crates/kui/Cargo.toml` has `audio` and `accesskit` as default features
  a build turns off; only `arboard` is unconditional. A `clipboard`
  feature is a two-line change, and nobody has asked — under "Rows, when
  a view asks".
- "the middle mouse button routes nowhere" — true and already parked
  (C2, same paragraph).
- "no gradients" — ADR 0005 declined them with the reasoning written
  down; "no z-index" and "one shadow" are the same ADR's paint vocabulary.
- "`Value::as_str` can fail at runtime where iced's `match` cannot" —
  true, and the price of the one contract Lua, C and JSX share; the
  comparison says as much.

Its one recommendation — "publish kui to crates.io, even as an alpha" —
is a decision, not an entry, and it has a fact attached: **the name
`kui` is taken on crates.io** (`XuShaohua`, "WebGL charts", one version,
0.1.0, published 2023-04-04, no repository, 1,629 downloads);
`kui-core`, `kui-ffi`, `kui-lua`, `kui-node` and `kui-wgpu` are free. So
the runner crate would publish under another name, or the name would
have to be asked for from its owner — crates.io does not reassign a name
without one. Under "Distribution" in *After alpha.9*.

### Theirs, not ours

- **Per-surface resource handles** (pomodoro wish 1, fourth report).
  Same answer, delivered where the app reads: a `howto.md` answer under
  F33 rather than a fourth backlog paragraph.
- **The stock `<button>` paints its own look.** The mind map converted
  its toolbar and got "a row of seven primary-blue buttons across a dark
  toolbar", and the report reads the line right: `ButtonProps`' doc says
  a button that needs any other row is a `<box role="button">` with the
  rows spelled out. The deletion line's condition was a button that
  looked like the stock one; theirs never did. Nothing to change.
- **`preview.svg` is gitignored**, third report running. Theirs, and
  their notes say so.
- **`justFinished` depends on the tick's liveness** (pomodoro, "What to
  watch"). A model-side sound lifetime that holds while the clock moves
  is the app's design and its headless assertions guard it.

## From the two alpha.10 upgrade reports (2026-09-09)

Both apps upgraded to alpha.10 the day it was tagged and reported again:
the mind map's `FINDINGS.md` and the LCARS pomodoro's
`docs/kui-alpha-10.md`. F37 came out of the same pair and closed first;
what the rest of the round produced is four entries, **F38–F41, all built
on 2026-09-09** and moved whole to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-the-two-alpha10-upgrade-reports-2026-09-09).

The theme is not last round's. Every one of them is a fact the core or the
driver owns that never reached the view — the width an editor measures
itself at, the settings a window learns as it opens, the setting the user
changes while it is open — and **three of the four were filed by the apps
as their own problem**. The check was done by running the reports' probes
against a build rather than by reading the tree, which is what turned two
paragraphs of workaround into defects: the mind map's "the mechanism is a
property of the design rather than a slip" is F41, and it is a slip.

Three claims did not survive the check, and are below. One of them —
"the self-terminating window is not in the package" — is a facility the
package has and does not document, which is F33's shape again and became a
`howto.md` answer rather than an entry.

### Theirs, not ours

- **`KUI_SMOKE_FRAMES` is not in the npm prebuilds** (mind map). True,
  and deliberate — the changelog's own reasoning — but the conclusion
  drawn from it, that "the one facility for getting a windowed app under
  test is unavailable to every app consuming the package", is not. `setup`
  is handed the window *and the loop*: `await loop.frame()` counts
  presented frames, `win.close()` ends it, and `runWindowed`'s promise
  resolves with the final model, so the process exits with a code. Run
  from this tree: five frames in 302 ms, then `window closed, final model
  = {...}`, exit 0. It is the better shape for an app — it is the app
  asking, in the only place that knows a window is being opened to be
  looked at rather than used — and it was reachable in alpha.7. A `Test
  it` answer in `howto.md` now says so.
- **`truncated-playback` "did not come"** (pomodoro). It comes. A Node
  window, a real device, a 3 s sound and an `<audio>` node removed 0.28 s
  in raises it with the full message; the report's own run is the one
  thing here that cannot be reproduced, and the likeliest reading is that
  its chime had finished (a stop that lands after the sound ended is not a
  truncation, and the core says so). What the report is right about is
  that it had no way to tell, and that is F36 and the pomodoro's wish 1:
  `audioCommands()` is on `Ctx` and not on `KuiWindow`, so a window can
  neither see the queue nor say whether a device is open. Still open,
  still not urgent.
- **A second is drawn 1562 ms long** (pomodoro, "the seconds were not
  equal"). Filed by the app as its own and it is: `tick: { every: 250 }`
  is how far past its boundary a second can be drawn, and `every: 16` is
  what the tick doc asks for. kui's share was said to be the 1062 ms pump
  stall — which is not the driver: 2390 pumps over 20 s idle, worst gap
  41 ms, and that one was the first frame. `pump_app_events` is starved
  for as long as the platform holds the run loop (a live resize or a
  window drag on macOS and Windows), which a pumped loop cannot pre-empt
  and a `setTimeout` cannot outrun. Worth a sentence in the docs if it
  comes up again; not an entry.
- **`setEnv` for a window** (pomodoro wish 2). Refused, as its doc
  comment says: the runner reports the real window every frame, so
  anything pushed would be overwritten before the next view ran. The
  reduced-motion branch is asserted headless, which is where a machine
  the test is not running on can be described. If a view ever needs the
  override in a window, the place for it is the launcher — an app asking
  in its own code, the same line `KUI_SMOKE_FRAMES` draws. (Asked a third
  time and built in that shape on 2026-09-12: F47.)
- **`preview.svg` is gitignored**, fourth report running. Theirs, and
  their notes say so.


## From the two alpha.11 upgrade reports (2026-09-12)

Both apps upgraded to alpha.11 the day after it was tagged and reported
again: the mind map's `FINDINGS.md` (alpha.10 → alpha.11) and the LCARS
pomodoro's `docs/kui-alpha-11.md` (wishes 1–5). The bare bump broke
neither — five byte-identical previews on one side, an
assertion-for-assertion identical frame on the other — and both reports
open by correcting their own last one: the mind map that the window was
testable from app code since alpha.9, the pomodoro that its smoke test's
chime had mostly never sounded. Every claim below was checked against
this tree before it became an entry, and two of the eight are sharper
than the report that raised them: the `dispatch` that drops a
`setEditText` seed loses it to *the redraw the call itself asked for*
(F42), and "no reading says what the dock leaves" is one getter reading
the wrong field against its own schema row (F43).

What is new this round is that both apps found the same seam from
different sides. A `dispatch` the loop did not make — from `setup`, a
timer, a promise — runs `update` in one JS turn and its view in the
`step()` after the next `win.pump()`, and *between* those the runner
paints. The mind map lost a seed to that paint; the pomodoro's test lost
a one-frame `<audio>` node to the tick that fired first. F42 is the half
that is the driver's.

### `!` F42 — A foreign `dispatch` loses the `setEditText` seed to the redraw the call itself asked for — **done (2026-09-12)**

**Done (2026-09-12), both halves as the entry asks.** (1) `createLoop`
gained an `[OWED]` door beside `[BUDGET]` and `[FAILED]` — draw the
model a `dispatch` outside the loop changed, say whether it did — and
`runWindowed`'s pump calls it *before* `win.pump()`, so no runner redraw
lowers a tree older than that `dispatch`; pump-then-step for events
stays, `dispatch` still draws nothing itself, and the draw counts as
work for the pacing. (2) `EditStore::set_text`, `Core::set_edit_text`
and `set_edit_text_by_label` (and `Ui`'s two) return whether the text
landed or was held, and kui-node's `set_edit_text` redraws only on the
first; Lua and C ignore the flag, their prototypes unchanged. The guard
runs the driver itself: `runWindowed` takes `surface` the way
`createApp` does, and `retainedWindow` in `packages/kui/test.mjs` is a
stand-in whose `pump()` re-lowers its last tree through a real `Ctx` —
the report's sequence, from a `frame()` promise, pins no warning, the
editor seeded, and two views for three pumps; with the pre-pump draw
removed it goes red on exactly the warning. `set_text_says_whether_it_
landed_or_was_held` in `crates/kui-core/tests/editing.rs` pins the flag
in both spellings. CHANGELOG under alpha.12.

**Symptom** (mind map, "a foreign `dispatch` draws now, and drops what
`update` held"): dispatching `beginEdit` from outside the loop, then
reading the tree after each frame — `sync: views=1 field=absent`,
`frame +1: views=2 field=absent`, `frame +2: field=present` — with
`edit-text-without-editor` raised between, so a *reopened* editor comes
back with the abandoned draft over the model's text. The input path is
unaffected. The report's diagnosis, "a frame painted between them is a
redraw of the tree the window already had, and that is the frame the
one-frame hold expires on", is right; what it could not see is which
frame and why.

**The line.** `KuiWindow::set_edit_text` ends with `self.$redraw()`
(`crates/kui-node/src/lib.rs`, the `set_edit_text` arm of the shared
macro): every write, including one the core *held* because nothing had
declared the name, asks the runner for a redraw. `runWindowed`'s pump is
`win.pump()` **then** `app.step()` (`packages/kui/index.js`, `runWindowed`),
and a window's redraw re-lowers the retained tree — `TreeApp::view` lowers
the bytes the last `setView` stored — through `Ui`, whose build ends in
`Core::finish_frame`, whose `layout_frame` drains
`EditStore::take_unclaimed_seeds` and raises the warning. So on the
dispatch path the order is: `update` (seed held, redraw requested) →
`win.pump()` paints the **old** tree, which declares no editor, and the
seed is dropped → `step()` draws the new tree, whose editor seeds from
`initial`. On the event path `update`, `view` and `setView` share one JS
turn, so the redraw that follows lowers the new tree and the seed lands.
The hold's contract — "the view that draws the editor the same `update`
opened" — is kept by the core for the only frame it can count; the frame
between is the binding's.

It is not only `setEditText`'s own redraw. Anything that has the runner
paint inside that `win.pump()` — the caret blink, a pointer crossing a
hover node, a live resize — lowers the old tree the same way, so removing
the `$redraw()` on a held seed closes the deterministic case and leaves a
race the app cannot see.

**Fix**, two halves. (1) In `runWindowed`'s pump, a model the loop has
not drawn yet (`dirty`) is drawn **before** `win.pump()` — the same
`update` → frame ordering the event path has, so no runner redraw can
lower a tree older than the dispatch; the pump-then-step for events stays.
(2) `set_edit_text` on a seed the core held requests no redraw: nothing on
screen changed, and the frame that will change it is the app's. Guard:
the windowed half of `createLoop` is already tested over an injected fake
surface (D4); give the fake a `pump()` that re-lowers its last tree and
pin that a `dispatch` + `setEditText` between pumps raises no warning and
seeds the editor. Not `dispatch` drawing synchronously — an effect handler
dispatches from inside `flushEffects`, which runs inside `draw()`.

**The pomodoro's half of the same seam is theirs**, and is under "Theirs,
not ours" below with the mechanism.

### `!` F43 — `env.viewport` is the window, not what the dock leaves, against its own schema row — **done (2026-09-12)**

Done: `Core::env_facts()` fills `viewport` from `self.viewport()` — the
frame's, as its `ENV_FIELDS` row said all along — so `env().viewport` is
what the dock leaves in Node, Lua and C at once; and `Ui::viewport()`,
which the entry took for `Core::viewport()`, read the same raw field and
now delegates, so a Rust view under a dock reads the app's width too.
`KuiWindow.size()` answers with the new `Core::host_area(window)`: what the
dock leaves of a window that size, from the window and the dock's state
alone (a `pub` wrapper over `devtools_area`, which was already
frame-free), so `setup` and `init` read the right number before
`env().viewport` is anything but 0×0. Lua and C have no `size` twin —
Lua's `view(env)` carries `viewport_w`/`viewport_h` and C passes the
viewport to `kui_frame_begin` itself — and ADR 0020's parity tests moved
nothing. The `resize` event's schema doc and `size()`'s own say the same
thing now (the dock is a resize; a dock present at launch posts none, by
the first-frame rule, which stands), and the `viewport.w` row, the
`Env.viewport` and `WindowSize` d.ts docs and the README's window-size
bullet follow. `PumpRunner::window_size` stays the window's — its name
says so — and its doc points at `host_area` for what the next frame lays
out into. Guards:
`the_env_reading_and_the_pre_frame_size_are_what_the_dock_leaves` in the
core (right dock, 1040×720, `env_facts().viewport.w < 1040`, `host_area`
before and after the frame, `Ui::viewport` mid-frame, the `ENV_FIELDS`
row's `get`; it fails against the old getter, checked by restoring it) and
`env().viewport is the window less the devtools dock (F43)` in the Node
suite on a headless `Ctx` — `KUI_DEVTOOLS` is never read there but
`setDevtools(true)` works, so the dock is put in by hand; `size()` needs a
display, so the core pins that half. Both test comments say why the
existing `ENV_FIELDS` readback could not see it: no corpus scene has a
dock in its tree. CHANGELOG under alpha.12's `### Fixed`.

**Symptom** (pomodoro, "with the devtools docked at launch, the app draws
for the whole window", wish 2): under `KUI_DEVTOOLS=1` the app is squeezed
into ~660 px of a 1040 px window with everything `grow` absorbing it;
`win.size()` says 1040×720 in `init`, `env().viewport` 0×0 there and
1040×720 three frames later, and no `resize` ever arrives. The changelog
says "the app's viewport is what the dock leaves: `viewport()` says so".

**The lines.** Three, and the report blamed none of them by name:

- `Core::env_facts()` fills `viewport: self.viewport`
  (`crates/kui-core/src/runtime.rs`) — the window — while the
  `ENV_FIELDS` row it feeds says `from: "Core::viewport(), the frame's"`,
  and `Core::viewport()` returns `dt_area`, the dock-adjusted host area.
  The schema row and the getter disagree, and the readback test that pins
  `ENV_FIELDS` against the corpus never has a dock in the tree, so it
  could not see it. **This is the defect**: fix the getter, and
  `env().viewport` says ~660 under a right dock, in every binding at once.
- `KuiWindow::size()` is `runner.window_size()` — the window's inner
  size, as its doc says — but the `resize` event's schema doc says
  "`KuiWindow.size()` queries the same numbers", and `begin_frame` puts
  the dock-adjusted `area` into that event. Under a dock they differ.
  Either `size()` answers with what the dock leaves (`devtools_area` is
  computable from the window size and the dock state before any frame, so
  it can answer in `setup` and `init` too, where `env().viewport` is by
  design still 0×0) or the doc stops promising. The first is what an app
  seeding its tiers from `win.size()` in `init` — the README's own advice —
  needs.
- A dock present at launch posts no `resize`, by `begin_frame`'s "the
  first frame establishes the viewport" rule. Right in itself, and moot
  once the two readings above are the dock's: the app read the right
  number before its first view.

Rust is unaffected — `ui.viewport()` is `Core::viewport()` — which is why
the changelog sentence was true where it was written. Guard: a core test
with `set_devtools(true)` + `set_devtools_dock(Right)` and a
`begin_frame(1040×720)` asserting `env_facts().viewport.w < 1040`, and
`size()`'s answer pinned beside it in the Node suite.

### `~` F44 — A field cannot wrap and a document cannot submit: the label editor has no spelling — **done (2026-09-12)**

Done: `wrap` declared on a single-line editor folds it. `EditOptions` has
a `wrap: bool` beside `style.wrap` — the row *declared*, which the style
cannot say since its default is `Word` — and `EditState` a `folds` it
computes at every declaration as `multiline || (wrap && style.wrap !=
None)`; `wrapped`, `line_offset` and emission's clip decide by that where
they decided by `multiline`, so a folded field wraps to its width, never
scrolls, and keeps the node's clip, while `admitted`, `apply_key`'s Enter
and the caret-at-end rule still read `multiline` and stay a field's. The
buffer takes the row's mode too (`Wrap::Glyph` for `wrap="glyph"`), and a
document ignores the row as it always did. The bindings: `PropsOut::wrap`
set by `schema::apply` on the `wrap` row, which Node and Lua both read
into the option; C has `KUI_EDIT_WRAP` on `kui_text_edit`'s flags, the
three `KUI_EDIT_*` now constants the parity TU pins. `wrap` was already on
`edit`'s allow-list in both checks (a shared row, `jsx_rows: None`), and a
`known_prop` line plus a Node case pin that it stays there. Guards:
`tests/field.rs` — the two-line draft at 209 px that submits on Enter with
no newline, the three doors that admit none, `wrap="none"` as the plain
field, glyph vs word, the document-turned-field re-measure, and
`width="fit"` + `maxWidth` growing down on the keystroke frame with every
glyph inside the box; each branch mutation-tested, one survivor —
`line_offset`'s `folds` guard is equivalent within the 2 px caret margin,
since a wrapped line never outruns its width. The corpus's `controls`
scene seeds its editor past its box with `wrap` in all four adapters and
the digest moves without it (`f54e…` vs `4247…`), so a dropped row fails
there; an Enter step would be a new `Step` kind and was not added. The
schema's `edit` and `wrap` docs name the exception. The other half —
`submit="enter"` on a multiline editor — is not built, as the paragraph
below says.

**Symptom** (mind map, "the rename field is a field now"): F41 made a
single-line `<edit>` take one line whatever its box, so a rename field
that declared `wrap="word"` to break where the node's label breaks went
from 38 px / two lines to 19 px / one line scrolled 62 characters in
209 px — inside a box the app still sizes to the wrapped draft. Every
check passed, because the checks asserted containment and the box's
height. `multiline` wraps it and makes Enter insert (`submitted 0,
changed 1`, text `"\na considerably…"`); `width="fit"` lays 393 px of
field through a 231 px box.

**The lines.** `EditStore::apply_key` (`crates/kui-core/src/edit.rs`):
`EditKey::Enter` inserts on `multiline` and submits otherwise — there is
no other submit path, no modifier-Enter — and the layout since F41 wraps
`multiline` editors only. The schema's own sentence is exact and is the
gap: "a field takes one line whatever its box … while a document wraps to
its box". Two modes, each carrying its layout *and* its keyboard, and the
editor the report wants — one paragraph, Enter submits, caret opens at the
end, folds to a width — is a field's keyboard on a document's layout.

**Fix.** Honour `wrap` on a single-line editor: with it declared, the
field folds to its width (the `fit_heights` branch that F41 made
`multiline`-only becomes `multiline || wrap`), still admits no newline
(`admitted` stays), still submits on Enter, still opens with the caret at
the end, and does not scroll horizontally. That is the whole reported
case, and it closes the report's other section too: the field becomes
`width="fit" maxWidth={MAX_TEXT_W} wrap="word"` — `clamp_w` already
bounds a `Fit` width — so the core measures the draft on the keystroke
frame and the "headroom, sideways" margin (below) has nothing left to
cover. The other half — a `multiline` editor that submits on plain Enter
and inserts on Shift-Enter, the chat-input shape — is the same kind of
gap from the other side, has its own users, and is one row
(`submit="enter"`) when a view asks; not built here on the strength of a
report that does not want it.

### `.` F45 — `clock` is read by `runWindowed` and absent from its options type — **done (2026-09-12)**

Done: `clock?: () => number` on `runWindowed`'s options in
`packages/kui/index.d.ts`, with a doc that says what it moves in a window
(the ticks and `tick.msg(now)`; the frame clock behind `transition` is the
runner's own, since a `KuiWindow` has no `setTime`) and that `startTime`
has no counterpart there. The block is the hand-written half of the file,
after the last `-- end generated --` marker, so `npm run gen` leaves it
alone. The guard is the one the entry asked for: `Example.clock` on the
examples harness (`examples/node/devtools.tsx`), handed to `runWindowed` as
`clock: example.clock`, so `npm run typecheck` in `examples/node` exercises
the field — removing it from the type again produces one `TS2353`, checked
by removing it. No example sets it; the headless drive keeps the loop's own
hands so `advance` still works there. CHANGELOG under alpha.12's
`### Fixed`.

**Symptom** (pomodoro, wish 3): the one line passing `clock` to
`runWindowed` carries a `@ts-expect-error`, deliberately, so it fails the
day the type catches up.

**The line.** `runWindowed` reads `opts.clock ?? Date.now`
(`packages/kui/index.js`); its declared options in `index.d.ts` are
`WindowOptions & { title, pumpMs, idlePumpMs, quietMs, setup, effects }`
— no `clock`. `createApp`'s options declare it. F37's class exactly: a
`.d.ts`-only gap no test in this repo can see, because CI's typecheck of
`examples/node` is the guard and no example passes it. Fix the type and
have one example pass a clock — the smoke channel's headless drive is the
natural one. (`startTime` is also read and rightly absent: under a clock
it is dead.)

### `.` F46 — `tick.every` is static, so a fast tick pins the idle pump — **done (2026-09-12)**

Done: `every: number | ((model: M) => number)` in `LoopConfig.tick`
(`packages/kui/index.d.ts`, the hand-written half) and in `createLoop`
(`packages/kui/index.js`): the reading is taken after `init` and after
every `apply` — whether or not the model changed, since a tick handler
that mutates in place and returns `undefined` still moved what the
function reads — and when it changes, `nextTick` moves to
`lastTick + every`, keeping the beat, unless that is already past, when it
counts `every` from the change instead (the way a fresh loop counts from
its start), so a cadence that shortens after a long quiet owes one tick
`every` from now and not a burst back to `lastTick` and not one this
instant. `0` or less is no tick, as the number is. The one trap building
it: a change a *tick* makes has to be rescheduled from the tick's own time
(`inTick`), because under `advance(ms)` the clock is already at the end of
the span and a replay that read it skipped the ticks the new cadence owed
inside it — the headless guard caught it. `[BUDGET]` is unchanged; with
the next tick a second out, `min(gap, untilTick)` is the gap. Two guards in
`test.mjs`, each checked by mutation (a reading that sticks after the first,
and a reschedule from the clock instead of the tick): a headless loop whose
`every` flips 16 → 1000 when the model stops counts 10 ticks over
`advance(160)`, 3 over `advance(3000)`, 0 at the change back and 10 over the
next 160 — plus the flip made from inside a tick, and a function returning
0; and over the injected fake surface with a fake clock, the budget answers
16 while the model runs and the idle gap (32, or 250) once it says 1000.
CHANGELOG under alpha.12's `### Added` and "what you can delete".

**Symptom** (pomodoro, wish 1, with the numbers): alpha.11's backoff takes
the counter from 8.7% to ~3% of a core; the pomodoro's mid tier, with
nothing moving, stays at 7–8%, because `tick: { every: 16 }` — chosen so a
second is never drawn late while running — is also its cadence while
stopped, when its clock matters once a second.

**The line.** `createLoop` reads `every` once
(`const every = tick?.every > 0 ? tick.every : 0`) and the driver's
budget is `max(busyMs, min(idleMs, untilTick))` (`[BUDGET]`), so a 16 ms
tick floors the gap at 16 ms forever. Both correct; the config has no way
to say the rate depends on the model.

**Fix.** `every: number | ((model) => number)`, re-read after every
`apply` — `nextTick` moves to `lastTick + every(model)` when the answer
changes — so `every: (m) => m.endsAt ? 16 : 1000` costs a stopped app a
pump a second. Headless `advance(ms)` fires the same schedule, so it is
testable; a function returning `0` or less means no tick, as the number
does. Node-only, as `tick` is.

### `.` F47 — An override for `env.system` in a window, at the launcher — **done (2026-09-12)**

Done: the shape the alpha.10 refusal named, built as written. `SystemEnv`
is its own override — its four "cannot tell" readings are the defaults,
so unknown *means* not pinned and no second type crosses a binding —
and `SystemEnv::over(base)` in the core lays it over a reading field by
field. The runner applies it in `sync_env`, the per-frame write that made
`setEnv` on a window a refusal (F39 made it a function; it is called from
`Shell::redraw` and `push_pane`), which is what makes the pin survive
every frame; the fields left unknown are still the OS's, so a real change
to one of them arrives, and the `system` event that reports it carries
the pin, since the event is the reading — a change to the pinned field
itself raises nothing, because nothing the view can see moved. Doors:
Rust `kui::app(..).system(SystemEnv { motion: MotionPref::Reduced,
..Default::default() })`; Node `runWindowed(config, { system: { motion:
'reduced' } })` and `new KuiWindow(title, { system })`, typed as
`EnvInput['system']` so the headless assertion and the window read one
partial, handed over by `windowOptions(opts)` — the one function that
picks the constructor's options out of the loop's, exported so the
hand-over is testable without a display; C by calling
`kui_env_set_system` on the context handed to `kui_run_with`, which
already carried the plugins in — no new prototype, nothing for the ABI or
the header audit, and `kui_run` untouched. Both example harnesses take
`--motion full|reduced`. Guards: `env.rs` pins the merge and the event
carrying the pinned reading; `pane.rs` the merge over a `Queried`;
kui-node the partial's parse (and that its errors name the door);
test.mjs that `runWindowed`'s option reaches the constructor and that the
loop's own options do not. Checked on a real window on this machine, which
reads `motion: 'full'` bare: `'reduced'` before the first frame and after
five, accent/appearance/locale the OS's, no `system` event. Not an
environment variable, for the `KUI_SMOKE_FRAMES` reason. `howto.md` has
the answer under *Test it*.

**Symptom** (pomodoro wish 4, the third ask: alpha.10 wish 2, alpha.11
wish 4): the reduced-motion half is asserted headless with `setEnv`; a
window on a machine whose owner did not ask for less motion has nowhere
to say "as if they had".

**Where it stood.** Declined for alpha.10 with a reason that still holds
for `setEnv` — the runner writes the real reading every frame, so a push
is overwritten before the next view — and with the shape that would work
named in the refusal: "the place for it is the launcher — an app asking
in its own code, the same line `KUI_SMOKE_FRAMES` draws". Three asks in,
build the shape. `runWindowed(config, { system: { motion: 'reduced' } })`
(Rust: `kui::app(..).system(..)`, C: the `kui_env_set_system` it already
has, made sticky) — a partial `SystemEnv` the shell merges *over* what
`system_env::query()` returns, in `sync_env`, so it survives every frame
and a real OS change still arrives for the fields not pinned. The
`system` event fires for the pinned reading as for any other. Not an
environment variable: an app you ship should not change its motion
because of one, which is the same line the smoke frames draw.

### `.` F48 — Whether assistive technology is listening, as an `Env` fact — **done (2026-09-12)**

**Done (2026-09-12):** `system.assistive` is the fifth row beside the four
OS readings — `"unknown"` | `"none"` | `"listening"`, `Assistive` in
`env.rs` with the same `ALL`/`name`/`code` shape as `Appearance`, so the
schema test named every restating site and each moved together: the row,
the `system` event's payload, the devtools facts, Node's `setEnv` and
d.ts, the C header, `props.md`. The runner's `sync_env` reads it off the
pane's bridge (`Bridge::assistive`: `active` was already the fact), a
change in `active` asks for a redraw so the reading reaches a frame and
the core reports it — an app that only redraws on input would otherwise
have heard of the screen reader with its next click. C got its own
additive `kui_env_set_assistive` + `KUI_ASSISTIVE_*` rather than a fifth
argument, the shape the header already argues for (`kui_env_set_audio`),
so `kui_env_set_system`'s prototype and `KUI_ABI_VERSION` stand; the
settings setter keeps the field, since a host re-pushes the four on every
OS notification. The entry's condition — that the activation signal
reaches the shell on all three platforms — was checked against the pinned
adapters and came back **half wrong**: `InitialTreeRequested` reaches it
everywhere, but of `accesskit_winit` 0.34's three desktop adapters only
`unix` ever sends `AccessibilityDeactivated` (on the AT-SPI bus's enabled
flag flipping, a session-level fact); the macOS *and Windows* adapters
bind `_deactivation_handler` and never call it. The row's doc says so:
any client counts, and the reading falls to `"none"` on AT-SPI alone —
on macOS and Windows it rises once and stays for the window's life.
Guards: `assistive_technology_attaching_is_a_system_event` (core), the
round-trip and wire-order tests, the schema pin, the ffi setter test
(`kui_env_set_system` keeps it), the Node test through `setEnv` reading
`env()` and the event back, `cbuild`'s header compile (188 prototypes),
and the C and Node corpus adapters over a regenerated report.

**Symptom** (pomodoro, alpha.10 wish 4, carried to alpha.11 wish 5 and
never answered): `motion` reached the view; the reading that would change
what this app *says* rather than what it draws — whether anything is
listening — is not in `Env`. "It is the difference between an alert that
blinks and one that announces."

**The fact exists at the driver.** ADR 0016's own measurement is stated
"while a screen reader is attached", and the bridge knows: AccessKit
calls the activation handler when a client asks for the initial tree, and
the runner derives the tree only from then on. Nothing carries that bit to
`env`. Fix: a row in `ENV_FIELDS` — `system.assistive: "unknown" |
"none" | "listening"` beside the four OS readings, with the explicit
unknown every `system` field has, set by the shell when the bridge is
first asked and reported through the existing `system` event so a
retained-tree host hears the change. Two honest limits to write into the
row: any AX client counts (a probe, an inspector, VoiceOver alike), and on
macOS nothing says when the client leaves, so the reading rises and does
not fall for the window's life — Windows and Unix adapters do report
deactivation, and the row says `none` again there.

### `.` F49 — `<audio finish>`'s cost in the app's units — **done (2026-09-12)**

**Done (2026-09-12), as the entry asks — the arithmetic, in the row and the
howto, with the 128 re-read from the driver first.** `Audio::warm` opens
the device with `AudioManagerSettings::default()` (`kui/src/audio.rs`),
whose `MainTrackBuilder::new()` sets `sound_capacity: 128` (kira 0.12.4,
`track/main/builder.rs:26`), and `AudioManager::play` is
`main_track().play` — so every play, declared or released, counts against
one 128, and past it `m.play` fails and `apply_one` pushes the playback
onto `refused` (F35), which the runner folds into `Core::audio_refused`
for the `refused` `sound` event and the `playback-refused` warning. The
sentence in the `finish` row and in howto's audio answer now reads:
voices held = sound length × release rate — a 1.4 s chime released four
times a second holds 6 of the 128 at any moment (5.6, rounded up to what
is ever held at once), a 10 s ambience released once a second holds 10,
every playback still declared (a loop included) counts beside them, and a
`refused` `sound` event is what arriving at 128 sounds like. That agrees
with F34/F35 as built: `finish` releases rather than stops (a loop still
stops, a paused playback has nothing to finish), and `refused` is the
phase a tagged play gets when it never starts, so it can never `end`. The
`AudioSpec::finish` rustdoc already carried the same number from the
other end (ninety releases a second of a 1.4 s file) and was left alone.
One half of the entry's premise was wrong: the row generates into
`props.md` only — the `audio` element's doc in `jsx-runtime.d.ts` is
hand-written outside the generated region, and had never carried the 128
sentence — so it took one sentence of the same by hand; a rerun of `gen`
leaves it. `cargo test -p kui-core --test docs` green, `npm test` 122
pass / 2 skipped, `git diff --stat` after `gen` touches `schema.rs`,
`props.md`, `howto.md`, `jsx-runtime.d.ts` and nothing else.

**Symptom** (pomodoro, alpha.10 wish 3, carried): "the 128-voice number is
the device's … how many one-shots per second can it release before it
matters?"

A doc sentence, not code. The arithmetic is `voices held = sound length ×
release rate`: a 1.4 s chime released at 4 Hz holds 6 of the main track's
128 at any moment, a 10 s ambience released once a second holds 10, and a
`refused` `sound` event (F35) is what arriving at 128 sounds like. Write
it into the `finish` row's doc in `schema.rs` — it is generated into
`props.md` and `index.d.ts` from there — and `howto.md`'s audio answer.

### Theirs, not ours

- **The headroom, sideways** (mind map). Measured well — fifteen of
  sixteen keystrokes 0.4–14.4 px past the field for one frame with no
  margin — and correctly attributed to the echo frame laying out inside
  the width the *previous* frame declared. It is the app sizing the field
  because it cannot use `fit` (it wants wrap), which is F44; with F44 the
  field is `fit` + `maxWidth` + `wrap` and the margin goes. Until then the
  margin is right.
- **The chime the smoke test never played** (pomodoro). The app was fine
  and the test was not, as the report says, and `clock` was the fix. The
  mechanism, written down once: `step()` is events → ticks → one frame,
  so a model dispatched between pumps is coalesced with whatever the
  ticks owed before the frame is built, exactly as two clicks in one pump
  are — a node declared for one frame needs that frame to exist, and a
  fake `now` dispatched against a real tick guarantees it does not. That
  is the loop's design, not a defect; F42 is the different half, where a
  *runner* frame lands between the dispatch and the step.
- **Deleting `keyFocus` fixed Tab** (pomodoro). A sink that holds focus
  keeps every key, Tab included — `key_target`, ADR 0002 decision 3, a
  terminal owns its keyboard — so a root sink given focus was a ring of
  one. alpha.11's two rules (the root is never a Tab stop; a root sink
  hears keys with nothing focused) are the remedy, and the release's
  deletion line named the line. Nothing to build; the report's "probe a
  deletion line before and after" is the right advice.
- **"The release the pin becomes a palette"** (pomodoro, "what to
  watch"). It already is one: `setTheme` takes role overrides on top of
  the base — `{ appearance: 'dark', raised: '#000000', fg: '#ffcc99',
  border_strong: … }` — and the stock tooltip paints `raised`,
  `border_strong` and `fg`, so an LCARS-black tooltip with peach text is
  one call today. What the report is reaching for past that is T4.
- **`autofocus` is an edge** (mind map). Read right, and the app's
  `keyFocus={selected && editing === null}` is what keeps it firing —
  written for another reason and load-bearing for this one. Worth their
  comment; not an entry.
- **The preview baselines and the windowed smoke** (mind map). Theirs,
  and their notes say so, for the sixth report.


## From the LCARS pomodoro's devtools round (2026-09-12)

The pomodoro ran alpha.11's panel against its own window — custom
chrome, a 620×500 floor, `setTheme({ appearance: "dark" })` in `init`, a
Windows desktop with a light appearance and an "automatic" accent off a
dark wallpaper — and reported five things the same evening, with three
screenshots. All five reproduce in this tree against `features/window`
(custom chrome, a 420×320 floor) with `--dark --accent #101a30`, and all
five were built the same day. Two are the panel's (F50, F51), two are the
runner's (F52, F54) and one is a hand-off between them (F53). Each is the
class ADR 0024 warned about in its consequences: the panel is drawn into
*the app's* window, so what the app chose — its chrome, its floor, its
base — is what the panel has to live with, and three of the five were
the panel assuming the OS's choice instead of the app's.

### `!` F50 — The panel paints the accent as ink, and an accent near the base is invisible — **done (2026-09-12)**

**Done (2026-09-12).** `panel::ink(theme)`: the theme the panel paints
with is the app's, with the accent moved toward the front of the base —
white on dark, black on light — in steps of 0.05 until it clears 3:1 on
`surface`, and `with_accent` recomputes the family from that; an accent
that already reads is painted verbatim, so nothing changes for the
accents that were fine. 3:1 is the UI-edge grade, since the panel's
accent is strokes, borders and short labels and not body text; the same
promise `Theme::ring_for` makes for the ring, which was already the one
role held to it. `theme::contrast` is `pub(crate)` for it. The accent
*swatch* in the tab row is the exception: it shows the accent in force
(the core's, not the ink) as a disc inside a hairline in `muted`, the way
the tokens list's swatches carry one, so a navy on a near-black still
reads as a sample; the facts row still prints the accent in force.
Guard: `the_panel_s_ink_is_a_readable_accent` — `#101a30` on the dark
base is under 1.5:1 and comes out over 3:1 with the ring in step, kui's
own blue on the light base is untouched, and on the dark base — where it
is 2.97:1, a hair under — moves a hair.

**Symptom** (screenshot 1): under the app's pinned dark, the accent dot
in the tab row, the lit placement button and the picker's border were
a navy on `#1a1d27`, and read as nothing. Windows 11's "automatic"
accent is picked from the wallpaper and lands there often.

**The line.** The stock widgets use the accent as a *fill* with
`on_accent` on top, which is readable whatever the accent is, and that
is the contract ADR 0019 checked. The panel uses it as *ink* in a dozen
places — `icons::draw`'s strokes, `icon_lit`'s lit state, `small_button`'s
on-border and label, the filter field's focused border, the legend's
keys, the tree's kind column and breadcrumbs, the outlines over the app
— and nothing held that to a contrast. The theme's `focus_ring` is the
one role that was, and for the same reason.

### `!` F51 — The base "app" keeps the OS's base, not the app's, under an accent override — **done (2026-09-12)**

**Done (2026-09-12).** `State::theme_override` takes the app's own
source and the system env and keeps the half it leaves alone from the
*app*: with the base at "app" and an accent chosen, a `Derived` or
`DerivedWithAccent` app goes out as `DerivedWithAccent(c)` as before and a
`Pinned` app as `Pinned(t.with_accent(c))` — the same palette,
recoloured; with a base chosen and the accent at "app", the accent under
the new base is the app's — the OS's for a `Derived` app, so a host that
reports none still paints kui's blue byte for byte, the app's own for the
other two. The core's `dt_saved_theme` is `dt_theme: (app, applied)`, so a
source the app sets *under* an override is told from the override by not
being it, and is what the override is lifted back to
(`Core::app_theme_source`). The base toggle's hint says which base "the
app's own" is (`base: app (dark)`), from a new `Facts::app_appearance`.
Guard: `an_override_keeps_the_half_the_app_chose` — pinned dark on a
light desktop stays dark under `Ctrl+Shift+A`, a brand accent survives
`Ctrl+Shift+T`, and a `set_accent` made under the override is what comes
back when both are off.

**Symptom** (screenshot 2): the facts read `appearance light · theme
light · derived + accent · accent #007aff` in an app that had pinned
dark, with the base toggle at "the app's own". Two presses of
`Ctrl+Shift+A` had flipped the app light.

**The line.** `(None, Some(c)) => DerivedWithAccent(c)`: the override
for "the app's base, this accent" went out as the source that follows
`env.system.appearance`, which is the OS's base, not the app's. Right
for the two sources that follow the OS, wrong for the one that does not.

### `!` F52 — The panel's own window inherits the launcher's chrome — **done (2026-09-12)**

**Done (2026-09-12).** `WindowCommand::Open` has carried the declaring
origin since ADR 0004, and the runner reads it now: a window declared
under `OriginId::DEVTOOLS` opens with `Chrome::Native` whatever the
launcher asked for, since it is the core's window and nothing in the
app draws a titlebar into it. `Chrome` is a `Pane` field — the launcher's
for the app's windows — and everything that read the shell's reads the
pane's: `window_attrs`, `sync_env`'s `custom_chrome` and
`native_controls`, the Windows non-client hook's install, and
`synthesizes_resize`. Checked in the real runner on Windows: the panel's
window has the OS's titlebar and buttons and moves; the app's keeps its
custom chrome.

**Symptom** (screenshot 2): under `chrome: 'custom'`, `Ctrl+Shift+D` to
`window` opened the panel undecorated — no titlebar, no way to move it,
the OS close button gone — over the app, which was drawing its own
titlebar and had none to lend.

**The line.** `Shell::window_attrs` applied `self.chrome` to every
window of the app, which is right for a second window the app declares
(it draws its own chrome there too, or asked not to) and wrong for the
one window the app did not declare.

### `!` F53 — `pick` from the panel's window leaves the keyboard there — **done (2026-09-12)**

**Done (2026-09-12).** `act("pick")` sets `focus_main` when the picker
comes up with the panel in its own window, and `devtools_act` — the one
path every chord and control goes through — turns it into
`WindowCommand::Focus(WindowId::MAIN)`, the mirror of what `inspect`
does with `focus_window`. Docked, nothing is asked: the pointer is
already in the main window. Guard:
`a_pick_from_the_panel_s_window_focuses_the_main_window`. Checked in
the real runner: `Ctrl+Shift+P` in the panel's window brings the app's
window to the front.

**Symptom.** Pick pressed in the popped-out panel: the crosshair is up
in the main window, the pointer and the keyboard are in the panel's, and
the main window is behind it.

### `!` F54 — The window's floor is the app's, and the dock is taken out of it — **done (2026-09-12)**

**Done (2026-09-12).** `Core::devtools_inset()`: what a docked pane takes
off the main window in the axis it takes it — `(side_w, 0)` for a side
column, `(0, bottom_h)` for the bottom strip, as the handle left it and
not as the window clamps it; zero popped out, off, or asked of any other
window. The runner adds it to the launcher's `min_size` after every
main-window frame (the handle's drag and the placement buttons land in
one) and re-applies `set_min_inner_size` on change, capped by `max_size`
as `clamp_size` already did. An app that declared no floor gets none
added: the dock's own floor is the pane's, and the app keeps 160 px
past it as before. Checked in the real runner on Windows by asking the
window `WM_GETMINMAXINFO` — 630×480 physical (420×320 at 1.5×) with the
panel off, 1140×480 docked right, 630×900 docked at the bottom, 630×480
again popped out. Guard:
`the_dock_s_inset_is_what_a_driver_adds_to_the_minimum_size`.

**Symptom** (screenshot 3): a 620×500 floor, the dock at the bottom,
and the app drawn into 552 px of height it had said it could not fit —
the compact tier overlapping itself. The OS enforced the floor on the
window, and the window is the app plus the dock.

**The line.** `Launcher::min_size` reaches `with_min_inner_size` once,
at `resumed`, and nothing re-asked it; the dock's coming, moving and
resizing all changed what the floor should be, and ADR 0024 decision 1
had the pane "squeeze the app" when the window is small on purpose —
which is the right answer for a window the user shrank past both floors
and the wrong one for the floor the app had asked the OS to hold.

## From the two virtual-list examples (2026-09-09)

`examples/rust/virtual_list.rs` and `examples/node/virtual-list.tsx` were
written on 2026-09-09 to answer "what does a virtualised list look like in
JSX" — the same 10,000 rows through `widgets::virtual_column`, through the
arithmetic unrolled (`child_key` + `scroll_geometry` + `visible_rows` +
`with_indexed`), and through JSX. The Rust two agree row for row (12 built,
`298..312` after a wheel to row 300, a click on row 302 that frame 1 never
built). The JSX one works too, but only after three things the Rust one never had to
do, and the third of them was a list that silently freezes. **C25, which was
those three, closed the day it was filed** and is in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#-c25--a-virtual-list-in-jsx-needs-a-sentinel-a-hand-written-key-and-a-guard-make-it-one-call--done-2026-09-09)
whole. **C26 followed it there on 2026-09-11**, when its last two steps
landed — scroll anchoring in the core (`anchor`, CSS's `overflow-anchor`)
and `layout_of(key)`; its first two had been built the day it was filed.
What it leaves open is not an entry: a JS/Lua `virtualRows` and variable
widths, each waiting for a view that asks.

## From the design-system audit (2026-09-10)

The round that produced
[ADR 0019](adr/0019-a-theme-derived-from-appearance-and-accent.md). The
palette itself is built, and so is every example's migration onto it (T3,
closed the day it was filed); T1, the defect it turned up, closed
2026-09-11, and T2 — the metrics, the axis the ADR scoped itself out of —
closed the same day. Both are in
[the archive](backlog/closed-2026-09.md#from-the-design-system-audit-2026-09-10-the-defect-and-the-axis-it-scoped-out)
under this heading, and so is T4 now — filed 2026-09-12 out of the
alpha.11 field reports and a question asked over them, answered by
[ADR 0027](adr/0027-tokens-beside-the-theme.md) and built the same day.
T5, the derived tokens that building left out, was built the day after
under [ADR 0028](adr/0028-derived-tokens.md); it stays here with its
outcome on top until the alpha.12 tag moves it.

### `.` T5 — Derived tokens: a colour computed from another

**Done 2026-09-13** — [ADR 0028](adr/0028-derived-tokens.md), written as
a menu of every place the arithmetic could live (the table, the
reference, the prop, the binding's helper, the app) and built as the
table: `ColorToken::Derived { from, ops }`, a chain of `[verb, …]` tuples
(`lift`, `darken`, `raise`, `alpha`, `mix`, `readable` — each a `Color` or
`Theme` method that already existed) folded by the core on read, so a
recipe over a themed source runs on the half in effect and a role can be
a source. One flat name per shade, the shape this entry sketched, chosen
over a `variants` sugar that would have declared the pomodoro's kit as 12
+ 2 — the map from a chosen colour to its shade is the app's, and
`hover` / `pressed` turned out to be role names, so the shades are
`peachHover` / `peachPressed`. Declared in all four bindings (`{ from, ops
}` in Node and Lua, `kui_tokens_derive` in C, `Tokens::derive` in Rust);
nothing on the wire changed. Two things the building found: a derived
colour is rounded to eight bits a channel so C's `uint32_t` readback
lowers the same quad, and a source must be the same table or a role — a
guest wanting a shade of the host's colour declares the value. The
corpus `tokens` scene grew by six derived tokens. The condition below —
an app declaring its shades as tokens — is the pomodoro's next alpha.

The original entry:


Filed 2026-09-12 out of building ADR 0027, as the one piece of the
pomodoro's palette the tokens do not cover. Eleven of its controls carry
a hover and a pressed shade computed by `lift(c, 0.3)` and `lift(c,
0.55)` — a mix toward white — so 22 colours a frame are arithmetic on a
name and no name themselves; the mind map writes the same thirteen out as
literals. A token set today either grows a variant per shade (36 names
for 12 colours, each kept in step by hand) or the app keeps `hoverBg =
hoverOf(C.peach)` beside `bg = T.peach`, half a mechanism.

**The shape:** a colour token declared *from* another — `hover: { from:
'peach', lift: 0.3 }`, or a small closed set of operations (`lift`,
`darken`, `alpha`) the core evaluates against the source token's half in
effect, so a derived token follows the appearance with its source. Stored
as the operation, resolved on read like the rest (`ColorToken::resolve`
gains a case); read back resolved; listed by the devtools with its
recipe. Nothing on the wire changes — a derived token is a name like any
other. `Theme::raise` is the arithmetic that already exists for "one step
up from this surface", and is the one to reuse rather than invent — and
since 2026-09-12 the contrast half is on `Color` too:
`Color::contrast` is the ratio and `Color::toward_contrast` the loop
`ring_for` and the panel's ink share, so a `readable` operation ("this,
moved until it clears 4.5:1 on that") is a fourth verb the set can name
without a fourth spelling of the loop.

**Condition:** an app in the field declaring hover and pressed shades as
tokens rather than computing them — the pomodoro's kit is the case in
hand, and its `hoverOf` / `pressOf` are the two operations to cover.
Small; the ADR's "not done here" names it.

## From the ABI-and-bindings audit (2026-09-10)

The round that produced
[ADR 0020](adr/0020-the-surface-the-schema-does-not-cover.md). Nine drifts
were found and all nine closed the same day — the prototypes, the plain
constants and the header's audit are pinned now the way struct layout has
been since P6 — and one thing was declined with a condition, which is what
this entry keeps.

### `.` B1 — The verb surface is documented, not pinned

**The condition is met (2026-09-13).** The second architecture review
(AR7–AR49 below) read the four door lists side by side and found thirteen
verbs that reach some bindings and not others, none with a stated reason:
`set_inspect`/`nodes` and the devtools readers (`set_devtools_theme`,
`set_devtools_legend`, `devtools()`, `devtools_dock()`) with no C door;
`Ui::cell_selection` Rust-only, though ADR 0017 §4 offers it "because a
grid's ends mean something to the app"; `widgets::text_input` with no
`<input>` in JSX (`schema.rs:1426` lists `<edit>` alone) and node-form
`tooltip` a prop only there; `Ui::window_command(Minimize / ToggleMaximize
/ StartDrag)` as `close()` alone in Node and absent in Lua; `label_of`,
`Core::cursor()`, `is_group_hovered`/`_pressed`, `EditOptions.accent` and
`tokens_declared` Rust-only (a C plugin re-declares every `kui_ext_view`);
`Ui::play/stop/set_volume/pause/resume` absent from Lua, plausibly because
its env is view-time, unstated; `Launcher::text_aa` an env var in C and
Node. AR27 (the C runner) and AR26 (Lua's two doors) are the rows with a
defect in them; AR40 the three small rules. **Build the table now,** as
a test rather than a doc: one row per verb, four columns, an `n/a` cell
carrying its reason, and the audit's own matrix as the draft. The rows
above that are one napi/extern line — `set_inspect`/`nodes` and the
readers in C, `cellSelection` in the three, `<input>`/`<tooltip>` as
elements, `window` on a Lua event — close with it.

The schema pins props, the corpus pins behaviour and `abi_parity` pins
what C can say; nothing pins *which doors each binding has*. The audit
found three that Node had and C did not (the open-menu reader, the font
family listing, the seven payload shapes) by reading all four lists side by
side, and ADR 0020 declines a `DOORS` table in `schema.rs` — one row per
verb with its Rust, C, Node and Lua spellings — because the verbs are not
one surface: Lua is a guest with a view-time env, Node's `KuiWindow`
refuses input injection on purpose, C is both a driver and a guest. A table
would carry three "n/a" columns with a reason in each, which is
documentation rather than a pin.

**The condition:** the next time a verb reaches one binding and not the
others, build the table, and put the "n/a" reasons in it. The audit's own
matrix is the draft; the ADR's "Not done here" names the two intentional
holes (`stats()` / `frameStats()` with no C twin, `kui_fragment_source` /
`kui_set_subpixel_text` with no Node twin) so the first two rows write
themselves.

## From the examples round (2026-09-10)

The round that produced
[ADR 0021](adr/0021-one-subject-per-example.md): every example moved,
most renamed, all put inside a harness with a dock, twenty of them given a
drive with an exit code, seven subjects given their first example, and
`env.audio` added across the bindings. Built whole the same day; what the
building found, in order:

- **A root sink was a Tab stop.** The harness's sink is the root, and the
  first Tab after the last control put the ring around the whole window.
  The ring skips index 0 now (`focus_ring`, pinned in `tests/focus.rs`);
  nothing had depended on reaching it.
- **`on_hover` is per node; `hover_group` lights together and does not
  report together.** The `hover` example's drive said otherwise on its
  first run and was corrected, and the prose of the ADR with it.
- **A `drag` event's `dx`/`dy` are the displacement since the press**, not
  the step since the last event — what the Node mindmap's "no delta is
  ever summed" always meant. The `drag` example's first drive summed them.
- **The Node addon never closed under `KUI_SMOKE_FRAMES`**: `native.cjs`
  loads the release cdylib, which ignores the variable by design. So the
  Node windows had never been in a windowed round; `kui-node` has a
  `smoke` feature now and the windowed round's `--node` (then
  `scripts/smoke-examples.sh`, since AR4 the `smoke` binary) builds with
  it. The four `--smoke` timeouts the examples carried instead are gone.
- **`text_input` without a `label` warns** (`control-without-name`), which
  two of the split examples did on the first windowed round and the round
  reported. An example of all things should not produce a warning; the
  windowed scripts print them for that reason.
- **The C span initializers had warned since ABI 8** (`bg` appended to
  `KuiSpan`, three files short one initializer each). Fixed in passing;
  `build.sh` is warning-free.

A second round by hand on the built thing (2026-09-10) renamed the crate
`kui-devtools`, put the controls in an icon strip, split the dock into
facts / events / tree tabs and gave the tree a node inspector over a new
`Core::set_inspect` / `nodes()` door; it also found the runner showing
the native *and* the drawn context menu, the root sink keeping Tab and
the menu-bar choices, a Node `dispatch` from an effect handler that never
drew, and eight smaller things in the examples (ADR 0021, *What the
building changed*, 11). E1 closed with it.

Three follow-ups, all closed — E1 with that second round, E2 by ADR 0024
the next day, and E3 last, on 2026-09-11, with `features/modal` and its
nested confirm — all three in
[the archive](backlog/closed-2026-09.md#from-the-examples-round-2026-09-10-the-three-follow-ups)
under this heading.

## From the devtools round (2026-09-11)

The round that produced [ADR 0024](adr/0024-the-devtools-are-the-cores.md):
the examples' dock moved into `kui-core`, drawn by the core into any
app's frame with one door per binding, and grew what the brief asked for
— the events tab as a virtual list with a payload viewer, a collapsible
and filterable tree, a picker, a fuller inspector, and a window of its
own. Built whole the same day. Its two follow-ups — D1, the tree's
keyboard, and D2, the panel's icons — were built the same day they were
filed and are in
[the archive](backlog/closed-2026-09.md#from-the-devtools-round-2026-09-11-the-two-follow-ups).

## From the paint-order round (2026-09-10)

One bug reported by hand on `features/theme` — right-click the page and
the drawn menu comes up under the page's scrollbar — turned out to be the
frame's paint sequence and not the menu's, and
[ADR 0023](adr/0023-layers-stack-in-the-order-they-open.md) is the answer:
a frame is a stack of layers ordered by when each opened, chrome paints at
the end of the layer that owns it, and input reads the same stack. Four
defects were measured on the tree before it was written; all four were
one entry, C28, filed and closed the same day — the ADR was built whole
within the hour, and the entry is in
[the archive](backlog/closed-2026-09.md#-c28--floats-are-under-scrollbars-and-the-ring-and-stack-in-tree-order--done-2026-09-10).

## From the canvas question (2026-09-11)

The round that produced
[ADR 0025](adr/0025-the-image-is-the-canvas.md) — proposed and **built the
same day**, its measurements and amendment in the document. The question
was whether kui wants a canvas (raw GPU commands, declarative or
callback-shaped) and, if not, which primitives make an app not need one.
The ADR's answer is an image whose pixels the app replaces, backed by a
texture of its own past the atlas, `sampling`/`fit` rows, `scale` on the
`layout` payload, and a `polygon` filled by a stock fragment; what it named
and did not build is here, each with the condition that builds it. None is
a defect. V1, the one with an order attached, was built the round after —
the fragment image input, in
[the archive](backlog/closed-2026-09.md#-v1--fragment-image-input--done-2026-09-11)
— so what stays is V2–V8.

### `.` V2 — `dash` on `line`

ADR 0010 deferred it with the shape written: a pattern along arc length
that keeps its phase across the joins of a polyline — `params.w` is free
to carry a phase per quad and the core knows the cumulative length. A
`dash` declared today would restart at every join of a curve, which reads
as a bug, so it is not free to get right blind. **Condition:** a view that
draws a dashed connector or a selection marquee.

### `.` V3 — `cap` on `line`: `round | butt | arrow`

ADR 0010 deferred arrowheads to "a `cap` vocabulary"; a filled arrowhead
was a fill the vocabulary did not have, and ADR 0025's `polygon` is that
fill — so an app can draw one today from the endpoint and the direction
it has. The row is worth building when a view wants the head to follow a
tweening endpoint without a second node. **Condition:** that view.

### `.` V4 — `backdrop` on `fragment`

ADR 0015 decision 9: the box's region of the framebuffer copied to a
texture at the flush before it, exposed to the fragment — frosted glass,
one bounded copy per box, data. **Condition:** a view that wants glass;
nothing has asked.

### `.` V5 — `blend` on `fragment`

ADR 0015 decision 9: `blend="add"` as a second pipeline variant per
fragment. **Condition:** a glow that stacks.

### `.` V6 — Streaming's two deferrals: dirty rects and mipmaps

ADR 0025 uploads a replaced image whole (`write_texture` of the full
image, 8 MB at 1080p) and samples without mips — and its split bench
shows what the second costs: a hundred 320×180 boxes each sampling a
1080p texture run the GPU at 0.59 ms against 0.36 for fragments of the
same size, which is minified sampling with no mip chain and not the
split.
A **dirty rect** on `update_image` is an API that grows a rect without
changing shape; **mipmaps** are what a photo viewer minifying a
12-megapixel texture needs and an app rendering at `w × scale` does not.
**Condition:** a stream that changes a corner of a large frame, or a
viewer that shows a photo smaller than it is.

### `.` V7 — A core `zoom` row — declined with a condition

A per-subtree scale as scroll's sibling: layout in the subtree's own
logical space, `env.scale × zoom` composed at emit (`runtime/emit.rs:418`
is the one seam), local-space payloads, `Core::project/unproject(key,
point)` as its doors. What it buys is deleting `× zoom` from every row of
a camera app; what it costs is the composed transform through hit-testing,
`onLayout`, access bounds, caret and IME rects, popup anchors, ghosts and
W7's whole-pixel snapping. The one camera app on kui zooms by hand and
its reports never asked. ADR 0025 decision 5 (`scale` on `layout`) is
written so an image loop reads the right number the day this lands.
**Condition:** a second app that writes the mind map's camera, or a
request for UI zoom (Cmd+/Cmd−). One fact for either design: continuous
zoom re-shapes text per fractional size and churns the shape cache (C16),
so a core zoom would quantise its steps as the mind map does (×1.45).

### `.` V8 — A drawing-ops `canvas` element — declined with a condition

One node, an op list lowered to quads, no per-op node. Measured before
being declined: the core builds 10k nodes in ~0.8 ms (~80 ns a node),
Node ~290 ns a node (`examples/node/tools/bench.mjs`, 89% at the
boundary), so an op list buys 4–5× in Rust and ~25× in Node at 50k
primitives — a scene no app on kui has — and nothing new to draw, since
an op is a box, a segment or a glyph and a fill is the path primitive ADR
0010 rejected. **Condition:** a view with more than ~10k primitives from
Node, or a fill eight points cannot make. V1 is the answer for data.

## From the alpha.11 pre-tag round (2026-09-11)

The bench guard was green — all four guarded rows within ±3.1% of
alpha.10 on a readable run — and four unguarded rows were not, which the
round reports rather than judges. Filed here so the next performance
round starts from numbers instead of from a feeling.

### `.` C29 — Four unguarded rows read 4–12% slower than alpha.10, reproducibly — **done (2026-09-12)**

**Done (2026-09-12): swept, not bisected, and one cause taken back.**
Rather than six probes a row, every one of the 63 commits between the
two tags that touch `crates/kui-core/src` was benched in tree order on
the four rows, two runs each, the lower median kept (Ryzen 9 9950X3D,
Windows 11, rustc 1.98.1, run-to-run spread under 1% — the M3 Pro's
readings reproduce here first: seg +11.5%, drop +3.8%, sq +3.4%, rd
+5.2% at alpha.11, and HEAD reads the same as the tag). The sweep is
one line a commit, and it reads:

- **`frame_10k_segments`: one step, `f2ff4e7` (ADR 0023).** Flat at
  ~705 µs through the 36 commits before it, 782 µs at it (+11%), 761
  after `a3f6877` gave a third back, then a drift to ~775 and 789 at the
  tag. The row is ten thousand one-node floats, and ADR 0023 made each a
  layer: a stack entry, a `subtree_end`, and **a chrome call per layer**
  — `emit_layer_chrome` with no scroll region in the layer and no ring
  to draw, which still fetched the cursor, walked an empty slice and
  asked `emit_focus_ring` to say no. That last one is taken back: the
  float pass now skips the call when the layer added no scroll region
  and `focus_visible` is off, which is every leaf float at rest. The row
  reads **796 → 759 µs** here (−4.6%), so of ADR 0023's +78 µs half is
  gone and half — ~3 ns a float for the stack's steady check, the order
  vector and the per-layer loop — is what the design costs and stays,
  written down here. `frame_1k_typical_with_100_floats` is unchanged
  (its hundred floats are tooltips with content, ~250 ns each, and the
  call was noise inside that), and no corpus scene moved.
- **`frame_10k_rects_square_clip` and `rounded_clip`: `b30cc98`, plus
  the clip shrink.** Square is flat at ~707 µs for 60 commits and reads
  730 at `b30cc98` (+2%); rounded takes +8 µs at `8ab3607` (the clip
  shrink, +1.1% — priced in the CHANGELOG as +2.5% on the M3), sits at
  ~745, and reads 766 at `b30cc98` (+2.8%). `b30cc98` is the six-entry
  round (V1, D1/D2, T2, C26, E3): 1,431 lines, and none of them per node
  on the rects path — `NodeSpec` is 224 bytes on both sides, `Core` grew
  the 64 bytes of `Metrics`, the anchor arithmetic in `positions` is per
  container and behind `spec.anchor`. A +2% that no line accounts for,
  in a change that re-lays most of the binary, is code placement; its
  own message recorded "guarded rows +1.2–1.7% inside the spread". Kept
  and written down; not worth a line-by-line at 2%.
- **`drop_1k_rows_plain`: no step.** 54–55 µs for the first thirty
  commits, 55–56 through the selection work, 57 by the tag: +1 µs at
  `b30cc98` and the rest a drift of a few hundred ns spread over the
  round, under this machine's own 1% at every commit.

So the entry's arithmetic holds: ADR 0023 was the segment row, the clip
shrink was a third of the rounded one, and the remaining 2–3% is one
large merge that moved code rather than added work. **The guard:**
`frame_10k_segments` is the fifth guarded row in `bench-check.sh`,
under the same 10% — its spread is under 1% here and under 3% on the
M3, and it is the only row that sees a per-float cost, which the four
others read as flat while it read +12%. The sweep's per-commit lines are
in `target/bench-check/c29-sweep.txt` on the machine that ran them and
are not checked in; the table above is what to compare the next round
against.

`scripts/bench-check.sh v0.1.0-alpha.10` on 2026-09-11, run alone on the
M3 Pro, then the four rows again alone (second run in parentheses; every
row's run-to-run spread was under 3% both times, so these are readings,
not noise):

| bench | alpha.10 | HEAD | change |
|---|---|---|---|
| `frame_10k_segments` | 774 µs | 868 µs | **+12.2%** (+9.3%) |
| `drop_1k_rows_plain` | 57.6 µs | 61.8 µs | **+7.4%** (+7.6%) |
| `frame_10k_rects_square_clip` | 769 µs | 820 µs | **+6.7%** (+3.8%) |
| `frame_10k_rects_rounded_clip` | 802 µs | 853 µs | **+6.3%** (+5.5%) |

What is accounted for: the clip shrink measured itself at **+2.5% on the
rounded clip and −0.1% on the square one** (CHANGELOG, *Changed*), so
about half of the rounded row and none of the square one. What is not:
the rest, which arrived somewhere in the eleven rounds after it — ADR
0023's float stack (a `line` is a float, so `frame_10k_segments` is ten
thousand of them; `frame_1k_typical_with_100_floats` was added to price
exactly that and reads ~30 µs for a hundred, which does not scale to 75
µs for ten thousand, so it is not the whole story), ADR 0026's `HitShape`
on every region, ADR 0025's texture side list, ADR 0016's per-node
access-inputs digest (priced at 0.8% of a frame when built), T1's
`any_context_menu` walk. `drop_1k_rows_plain` is the odd one: no exit, so
`depart` has nothing to keep, and 4 µs over a thousand removed rows is
4 ns a row of something new on the drop path.

The guarded rows moved +0.9%, +1.0%, +3.1% and −2.6%, so it is not a
per-node cost across the board; it is on the clip path, the float path
and the removal path. `frame_10k_rects_with_access_tree` went the other
way, **−23%**, which is ADR 0016 decision 3 landing.

**Do:** bisect each row with `bench-check.sh <commit> <row>` over the
commits between `v0.1.0-alpha.10` and `v0.1.0-alpha.11` that touch
`crates/kui-core/src/{runtime,layout,emit,paint}` — the script benches
any base ref and takes a divan filter, so each probe is one row and a
few minutes. Then decide per cause as C15 and C24 did: keep it and write
the number down, or take it back. Guarding a row is the other half: if
the segment row is what ADR 0023 costs, it belongs on the guarded list
under the tolerance that fits it, since nothing else in the round would
have caught a second +10%.

The Windows half of the round ran on 2026-09-12, the day after, and found
three things the macOS run could not: two lines in the C examples that
`cl` had never compiled (`__attribute__((unused))` in `common.h`, a
struct cast to its own type in `slots/host.c`) and a `test.mjs` pin on
dlopen flags Windows does not have. All three fixed before the tag and
recorded in the CHANGELOG's Native verification. One thing seen and left:

### `.` W13 — `npm run gen` writes a different `docs/props.md` on Windows — **done (2026-09-12)**

**Done (2026-09-12), the first way.** `MetricRole` carries
`platform: Option<PlatformValue { windows, elsewhere }>`, `Some` on
`titlebar_h` and `None` on the fifteen densities; the two numbers are
`metrics::TITLEBAR_H_WINDOWS` / `TITLEBAR_H_ELSEWHERE` and
`Metrics::comfortable()` picks the running one from them, so the row
and the struct cannot drift. The addon's `protocol()` writes the pair
beside `stock` and `compact` (which stay the running platform's
reading, as every other consumer wants), and `gen-types.mjs` prints
`32 / 34` in both columns for a platform row, with one sentence above
the table saying what the slash means. Both columns because
`compact()` leaves a platform row alone, and
`metric_roles_restate_the_metrics_exactly` now pins that — and that
the pair's member for this platform is what `default()` and
`compact()` read. Generated on Windows, the diff against the committed
unix file is exactly the intended line and nothing else; CI's Linux
run will write the same bytes. Nothing shipped changed.

`docs/props.md`'s metrics table has a Stock and a Compact column, and
`gen-types.mjs` fills them by asking the addon for
`(r.get)(&Metrics::default())` and `(r.get)(&Metrics::compact())`
(`crates/kui-node/src/schema.rs`, the `metrics` array of `protocol()`).
One role is platform-conditional — `titlebar_h` is `cfg!(target_os =
"windows") ? 32 : 34` in `metrics.rs`, the platform's real caption
height, and `compact()` leaves it alone — so the file says `34 | 34`
when generated on macOS or Linux and `32 | 32` on Windows. The committed
file is the unix one; CI's `git diff --exit-code` after `gen` runs on
Linux and agrees with it, so the gate holds. On Windows the same command
produces a one-line diff that is not staleness, and either gets
committed (and then fails CI's Linux run as stale) or has to be known
about and reverted. The row's own doc says "32 on Windows and 34
elsewhere"; the two columns beside it contradict it for a third of the
readers.

**Do:** make the generated output byte-identical on every platform by
making the row say what is true. `MetricRole` (or `metric_role!`) needs
to carry that a value is platform-conditional — a per-platform pair, or
a flag the generator turns into "32 / 34" in both columns — since
`protocol()` today only has the one number the running platform
evaluated. The cheaper alternative, pinning the generator to unix
values, leaves the Windows number in no generated document and encodes
"as seen from Linux" in a file that claims to be the reference; prefer
the first. Nothing shipped is affected either way: `Metrics::default()`
is right on every platform, only the printed table is not.


## From the editor-and-mux assessment, third round (2026-09-13)

The question asked of `main` at `580c99e` (alpha.11 + 50), the third
time: is kui the view layer for an emacs-plus-helix-plus-multiplexer app
whose document, IO, LSP and extensions are the app's own? The first
round (2026-08-31, before alpha.1) answered no — nothing gave the app a
keystroke — and built the key sink. The second (2026-09-07, `51ed27b`)
answered yes to the editor and no to the mux, and its eight entries
C16–C23 all shipped in alpha.9. This one answers **yes to both**: the
shape the second round asked for — the app hands kui the visible lines,
a `cells` grid per terminal pane, a `Waker` from the PTY thread — is
built, bound four ways, and measured flat under streaming. What is left
is at the edges of the *custom* editor, where the reference example
stops: a mouse it does not have, a clipboard it cannot reach, a caret
that never blinks — and one defect older than any alpha, which makes
every monospaced glyph on this machine italic.

Method, as before: the three benches in the tree, the four examples run
headless and on screen with real OS keystrokes (`SendKeys`), the
workspace tests (**1087 pass, 0 fail**), and a read of each door the app
needs. AMD Ryzen 9 9950X3D, Windows 11, release, 2026-09-13:

| frame | cost |
|---|---|
| `highlight_2x55_warm` — two panes × 55 highlighted code lines, nothing changed | 171 µs |
| `highlight_2x55_typing` — one line retyped per frame | 182 µs |
| `highlight_2x55_scrolling` — one line per frame scrolls in | 195 µs |
| `highlight_2x55_cold` — every run on screen is new (a file opened) | 617 µs median; one sample of thirty at 20 ms |
| `highlight_2x55_warm_rich` — each line one `Span` list instead of ~8 nodes | 69 µs |
| `cells_200x50_warm` — a terminal screenful, nothing changed | 53 µs |
| `cells_200x50_streaming` — every character new each frame | 55 µs |
| `cells_200x50_as_text_nodes` — the same grid as one text node a cell | 2.01 ms |
| `long_line_100k_first_frame` / `_edit` / `_scroll` | 4.4 ms / 112 µs / 112 µs |
| `long_line_100k_wrapped_first_frame` / `_wrapped_scroll` | 8.5 ms / 126 µs |

Against the second round's M3 Pro numbers the editor rows are the same
order (the cold open is faster here, the warm rows a shade slower), and
the two rows that round could not have — a terminal pane whose every
cell is new costing the same as one that never changes, and a 100 k
character line editing in a tenth of a millisecond — are what C20 and
C19 were for. On screen: `modal_editor` took `jjj ww v lll` and selected
"docu" with the block caret on the `u`; `splitmux` took `Alt-v`, `Alt-s`,
`Alt-t`, `Alt-1` and showed five panes on tab 1; `cells` drew its
terminal and passed its own drive. All of them in italic.

What the app has, checked door by door and not repeated below: the
whole keyboard as `{kind:"key"}` with `code`, `physical`, `text`,
`repeat` and `key_up` on request, in four bindings; IME commit and
preedit on a sink (C17); `text_hit` / `caret_rect` on the keyed line
(C18); long lines chunked (C19); the `cells` element with cursor
shapes, `origin_line`, cell selection and `Role::Terminal` (C20); the
`Waker` and `pump_until` (C21); underline, strikethrough and `bg` per
span (C22); `FontFeatures` (C23); the shaped-text cache under a byte
budget (C16); `scroll_geometry` for a document that declares its own
height and draws the rows the wheel reveals; tokens per origin for a
highlighter's groups (ADR 0027); the menu bar, popups, floats,
`always_on_top`; slots for a C or Lua panel with its own sink. Nothing
in that list needed a workaround to check.

### `!` C32 — `Mono` is whichever monospaced face has the lowest id on a machine without Noto Sans Mono — **done (2026-09-13)**

Done as written, in `text::new_font_system`: the three generic families
are set to the first installed name of a per-platform list
(`DEFAULT_FAMILIES` — the lists the entry gives, with a second sans and
serif choice on each platform), matched the way `fontdb::Database::query`
matches a name, and cosmic-text's own name is left where nothing on the
list is present. `text::default_families` reads the three back and
`Core::default_font_families` (crate-private, no binding door — the
entry asked for the panel) feeds the devtools' facts tab a `fonts` row,
`Helvetica Neue · Times New Roman · Menlo` here. Three unit tests beside
the function: `M` shaped in `Mono` is upright and monospaced, `M` in
`Sans` and `Serif` upright and not, and every pinned name is one an
installed face answers to; each skips with a message on a machine with
no such face. One correction to the entry: sans was already pinned
(`Helvetica Neue` / `Segoe UI`, blind, since alpha.1) — serif and mono
were not, and the pin now checks the face is installed before naming it.
What the test cannot show on this Mac: cosmic-text's fallback lands on
`Menlo-Regular` here by luck, as the entry guessed, so the mutation
(plain `FontSystem::new()`) passes on this machine and fails on the one
the round ran on. CHANGELOG under alpha.12's `### Fixed`.

Observed 2026-08-31 in the first round ("everything monospace renders
*italic* — cosmic-text's `Family::Monospace` is resolving to an italic
face on your system") and unchanged at `580c99e`: `modal_editor`,
`syntax_view` and the `cells` grid all draw in `BerkeleyMonoVariable-Italic`
here. A probe against cosmic-text 0.19 alone says why. Its `FontSystem::new`
sets the database's monospace family to `"Noto Sans Mono"` (and sans to
`"Open Sans"`, serif to `"DejaVu Serif"`) under a `//TODO: configurable
default fonts`; none of the three is installed on a stock Windows or macOS
machine. Sans survives because the platform fallback list carries Segoe
UI. Monospace does not: with the named family absent, the fallback
collects every face the database flags `monospaced`, ranks them by
`(font_weight_diff, codepoint_non_matches, font_weight, id)` —
`MonospaceFallbackInfo` in `font/fallback/mod.rs` — and pops the first.
**Style is not in the key**, so the lowest-id monospaced face wins, and
on this machine that is a variable font's italic instance. `Family::Name("Cascadia Mono")`
and `Family::Name("Consolas")` both resolve upright. The M3 Pro the
second round ran on did not show it, which means either Noto Sans Mono
is installed there or its lowest-id monospaced face happens to be
upright — luck, not a fix.

kui never names a monospace family: `Resources::family_of` maps
`FontFamily::Mono` to `cosmic_text::Family::Monospace`
(`resources.rs:426`) and `TextSystem::new` takes `FontSystem::new()` as
it comes (`text.rs:732`). So every `Mono` text, every rich-text
paragraph in `Mono`, the stock editor in `Mono` and every `cells` grid
inherit cosmic-text's choice — and the target app lives in mono.

**Do:** after `FontSystem::new()`, set the three default families to the
first installed of a per-platform list — monospace: Windows `Cascadia
Mono`, `Consolas`, `Courier New`; macOS `SF Mono`, `Menlo`, `Monaco`;
Linux `DejaVu Sans Mono`, `Noto Sans Mono`, `Liberation Mono`, `Ubuntu
Mono` — and sans and serif likewise (`Segoe UI` / `Helvetica Neue` /
`DejaVu Sans`; `Times New Roman` / `Times` / `DejaVu Serif`), leaving
cosmic-text's name in place when nothing on the list is present so its
fallback still runs. A headless test that shapes `M` in `Mono` and
asserts the face's style is `Normal` and its family is monospaced,
skipped with a message on a machine with no monospaced face at all. The
devtools' facts tab shows the three resolved families, so the next
machine this differs on says so on screen.

### `~` C33 — A key sink has no clipboard — **done (2026-09-13)**

Done as written: `Core::set_clipboard(text, html)` and
`Core::request_paste` in `select_api.rs`, queueing the two `MenuAction`s
a menu's Copy and Paste do; `Ui::set_clipboard` / `request_paste`, Node
`setClipboard` / `requestPaste` on the shared macro (so `Ctx` and
`KuiWindow` alike), Lua `env.set_clipboard` / `env.request_paste`, C
`kui_set_clipboard(text, html)` / `kui_request_paste` (an empty `html`
is none; no ABI bump, two functions appended). The runner drains menu
actions after every frame now as well as after every input, since a
view is where `Ui` is, and delivers `Paste` as `InputEvent::Commit` —
the entry's choice — which also changes a *menu's* Paste from `Text` to
`Commit`: an editor reads both the same, and `Text` never reached a
sink. Two things the building added: `sink_event` falls through to a
root sink with nothing focused, the way `key_target` does (a shell that
asked for a paste with nothing focused lost it), and a `text` event's
doc names the paste as its second source. `modal_editor`'s `y`, `dd`
and `p` moved onto it (`clip_out` stashed for the next view, since
`on_event` has no `Ui`; `awaiting_paste` marks the next `text` event as
the paste; linewise text ends in a newline). Tests: the sink hears
`{kind:"text"}` after a `Commit` in `tests/sink_pointer.rs`, the C
parity (`a_c_sink_has_a_clipboard…` drains both kinds), a Lua script
yanking and pasting from its keymap, and the Node test beside C34's.
Checked on screen: a drag-select then `y` puts the lines where
`pbpaste` reads them, and `p` after `pbcopy` inserts at the caret.
CHANGELOG under alpha.12's `### Added`.

The runner performs the clipboard chords itself only when an edit
widget or a selection scope has focus (`crates/kui/src/keys.rs:221`); a
sink hears the raw `Ctrl-c` / `Ctrl-v` and "brings its own bindings",
which is right — and then has nowhere to bind them to. The only ways
onto the system clipboard are `MenuAction::SetClipboard`, queued by a
menu's Copy or Cut (`runtime/menu_api.rs:399`) or by
`answer_selection_range` (`select_api.rs:520`, and only while a
`selectionrange` ask is outstanding), and `MenuAction::Paste`, queued by
a menu's Paste (`menu_api.rs:417`) and answered by the runner as
`InputEvent::Text`. Neither is reachable from a view or an event
handler. `modal_editor`'s `y` and `p` are an in-process `Vec<String>`
(`modal_editor.rs:130`): the reference editor cannot yank to another app
or paste from one. A Rust host can link `arboard` itself and reach
around the runner; a Lua extension cannot, and a C host is writing the
platform code the runner already has.

**Do:** two `Core` doors that become the two actions the runner already
applies. `Core::set_clipboard(text, html: Option<String>)` — `ui.set_clipboard`,
Lua `env.set_clipboard`, Node `ctx.setClipboard`, C `kui_set_clipboard`
— queues `MenuAction::SetClipboard`. `Core::request_paste()` queues
`MenuAction::Paste`, and what the runner reads comes back as
`InputEvent::Commit`, which already routes to the focused sink as
`{kind:"text"}` (C17) and to a focused editor as typing — so the app
that asked for the paste inserts it the way it inserts a committed IME
string, and never sees the clipboard's contents any other way, which
keeps the read on the driver's side where the permission lives. A host
driving its own window drains both from `take_menu_actions` as it does
today. `modal_editor` moves `y` and `p` onto them. Tests: the sink hears
`{kind:"text"}` with what a stand-in driver handed back; the C parity
check; a Lua script yanking from a slot.

### `~` C34 — A custom editor has no mouse: a press carries no point, a click no count, and the reference has none — **done (2026-09-13)**

Done as written, as `attach_lines` beside `attach_cells` in
`dispatch.rs`: a `drag` payload in any phase, or a map `click` payload,
whose node is or is inside an `on_key` sink gains `line`, `byte` and
`clicks`. `line` comes from `access::lines_under`, the walk
`custom_editor` used to do inline (factored out so the two numberings
are one), picked as the line nearest the point vertically with ties to
the earlier one — above the first is the first, below the last the
last, a gutter is the line beside it; `byte` is `TextSystem::hit_at` on
that line's key at the point (window coordinates, before
`devtools_translate`, as `attach_cells` reads them); `clicks` is the
last primary press's count, now kept in `Interaction::press_clicks`.
One fix underneath: `hit_at` clamps the point into the run it chose,
because cosmic-text answers a point above a run's first row with byte 0
whatever the x, and a press in the 3 px above a line's text put the
caret at the line's start. `modal_editor` carries `on_drag` on its sink
and `on_drag` in the app does the three: one click places the caret,
two take the word (`word_at`), three the line, a `move` extends from the
press; `drag_pos` maps `byte` back through the NBSP'd line one char for
one. Pinned in four bindings: `tests/sink_pointer.rs` (five cases,
including a key event gaining nothing and a sink with no lines adding
nothing), `test.mjs`, the Lua and C suites. Checked on screen with a
CGEvent driver: click, double-click on "document", a three-line drag.
The question the entry left — whether a plain `click` should carry
`x`/`y` in general — is still not asked. CHANGELOG under alpha.12's
`### Added`.

`modal_editor::on_event` handles `key` and `access` and nothing else
(`modal_editor.rs:419`): a click in the buffer moves nothing, a drag
selects nothing, a double-click on a word is a double nothing. What the
pieces offer, read together: a `click` payload is the `onClick` value
as-is (the events table in `docs/props.md`) — no point, so nothing to
hand `text_hit`; an `on_drag` `start` does carry `x`/`y`, so a press can
be read off a drag; `text_hit(key, point)` (C18) answers from `Ui`,
which Rust's `on_event` and Lua's `on_event(ev)` do not have, so the point
is stashed and resolved a frame later in `view` — C18's own outcome says
this is why the example was never moved onto it; and the click count
the core uses for word and line select in the stock editor and in a
`cells` grid (`dispatch.rs:443`, `:459`) reaches no payload, so a custom
editor's double-click-word is a timer the app keeps. The `selectable`
route does give drag-select with word and run on multi-click and the
`selectionrange` ask — but it is the core's selection, painted by the
core beside the editor's own, and a press in it places no caret.

**Do:** the `attach_cells` precedent (`dispatch.rs:59`), which adds
`cell: {row, col}` to any pointer payload on a grid. A press or drag
that lands on a `Role::Line` inside a key sink gains `line` (its ordinal
among the sink's lines — the addressing `access` events already use, and
`modal_editor::access_pos` already reads), `byte` (from `text_hit` at
the point), and `clicks`. Then click-to-caret, drag-select and
double-click-word are `on_event` arithmetic in every binding, with no
`Ui` and no frame of lag, and `modal_editor` gets all three. Whether a
plain `click` should carry `x`/`y` in general is a separate question and
not asked here.

### `~` C35 — A custom caret cannot blink — **done (2026-09-13)**

Done as written, one step further than the entry asked: the core keeps
the sink's caret across frames (`sink_caret`, from the same walk
`focused_caret_rect` used for the IME anchor, now `sink_caret_line`),
bumps `sink_caret_stamp` when it changes, and exposes `has_caret` /
`caret_stamp` (the two stamps summed) / `caret_visible` /
`set_caret_visible` — so the runner's clock reads the core and not
`core.edit` directly, and a C host driving its own window can run the
same clock (`kui_has_caret`, `kui_caret_stamp`, `kui_set_caret_visible`
beside `kui_caret_visible`). `caret_visible` is an `ENV_FIELDS` row —
`env.caret_visible` in Lua as the entry spelled it, `caretVisible()` on
Node's context with `setCaretVisible` for a headless test, one line in
`props.md`. `modal_editor` draws its caret on the phase and keeps the
`caret` row through the off phase — the trap: an app that stopped
declaring `caret` when hidden would un-arm the clock, which would park
it solid, which would declare it again. Tests: `tests/sink_caret.rs`
(three cases), the Node, Lua and C suites. Checked on screen with eight
screenshots 200 ms apart: the block caret alternates. CHANGELOG under
alpha.12's `### Added`.

The blink clock arms only while `pane.core.edit.focused()` is `Some`
(`crates/kui/src/lib.rs:1749`): a sink's caret — the inline node
`modal_editor` draws as `caret_bar` or a block — is solid forever. The
two ways around it are both wrong. Keyframes on the node's `opacity`
cycle for ever (`Repeat` is "always infinite") and ask for a frame every
vsync while they do, so a blink the stock editor draws at two frames a
second costs a hundred and twenty, never stops in a window that does not
have the keyboard — which the runner deliberately does for the stock
caret, for the reason in the comment above the clock — and cannot be
re-armed solid on caret motion without re-keying the node. A thread and
the `Waker` every 500 ms works from Rust, not from Lua, and has the same
background-window problem.

**Do:** the core already knows the focused sink's caret — `focused_caret_rect`
falls through to the `caret` line for the IME anchor (C17). Arm the same
clock when a sink whose subtree carries a `caret` holds focus, re-arm it
solid whenever that caret's offset changes between frames, and expose
the phase: `ui.caret_visible()`, Lua `env.caret_visible`, Node
`ctx.caretVisible()`, C `kui_caret_visible` — a view draws its caret
node on the on phase and skips it on the off. The stock editor and the
custom one then blink in step, and in the background neither does.

### `.` C36 — The three app-shaped examples have no headless drive — **done (2026-09-13)**

Done as written, each as `Example::headless` and enrolled in
`crates/kui/Cargo.toml`'s `headless` list, so the smoke round's headless
channel runs them: `modal_editor` — `jjj ww v lll` selects "docu",
Escape, `dd` deletes the line and queues it linewise, `p` twice asks and
the drive plays the host with a `Commit`, `:help` fills the minibuffer
and Enter runs it, then C34's first pin (a click places the caret on the
line and column, a double click selects "document") and C35's (the off
phase keeps the `caret` row); `splitmux` — Alt-v, Alt-s, Alt-o, Alt-t,
Alt-1 leave five panes on tab 1, Alt-w closes one, and the keymap is
still the sink's; `syntax_view` — `j`, `G` (and the view scrolls to keep
the line on screen), `k`, `tab` twice wrapping. Two things the drives
found about driving: `Drive::key` carries no `text`, so a keymap that
reads `text` in command mode (the minibuffer) needs `KeyPress::with_text`;
and `dd` after `v lll` deletes the selection, not the line — the by-hand
check had an Escape in it nobody wrote down.

`modal_editor --headless`, `splitmux --headless` and `syntax_view
--headless` each print "no headless drive" and exit 0; `cells` runs its
six checks. ADR 0021 says every example runs inside the harness with
`--headless` as a self-check with an exit code, and the smoke job runs
these three windowed for 120 frames — which pins that they draw, not
that `hjkl` moves the caret or `Alt-v` splits. That was checked by hand
this round with `SendKeys`, the way it was checked by hand in the first.
The three are the reference for the target app; they are the examples
whose keymaps most deserve a drive.

**Do:** one drive each — `modal_editor`: `jjj ww v lll` selects
"docu", `dd` then `p` twice, `:help` fills the minibuffer; `splitmux`:
`Alt-v`, `Alt-s`, `Alt-t`, `Alt-1` leave five panes on tab 1 with pane 1
focused; `syntax_view`: `j`, `G`, `tab` move the view and the buffer.
The C34 payloads get their first pin in the same drive.

### `.` C37 — A cell is a scalar, and its doc says it is a grapheme — **done (2026-09-13)**

The doc line, fixed with C36: `cells.rs` says a cell is one scalar in
all three transports, what that excludes (a base with marks, a ZWJ
sequence, a flag, a conjunct — precompose what NFC can, drop the rest),
and names the side-table shape the cluster would take when a view asks,
so it does not grow the cell. "Rust-only for now" is gone; the paragraph
lists the four bindings. No `grapheme` claim remains in the schema, the
header or the Node types.

`cells.rs:11` says "a grapheme cluster (an emoji, a base with its
combining marks) is one cell's `text`, shaped once"; the field is
`ch: char` (`cells.rs:46`), C's `KuiCell.ch` is a `uint32_t`, and Node's
stream packs `codepoint | flags << 21`. So `e` + U+0301, a ZWJ emoji, a
Devanagari conjunct or a flag cannot sit in a cell: the app precomposes
what NFC can (the accent) and drops what it cannot (the ZWJ sequence,
which has no precomposed form). The same doc says "Rust-only for now",
which stopped being true the day the element row landed. Neither is
urgent — a terminal's screen model is the app's, and it knows which
cells carry marks — but the doc promises what the struct refuses.

**Do:** fix the doc now. The cluster waits for a view that needs it, and
the shape is named so it does not grow the cell: a side table of
`(cell index, &str)` on `CellGrid` for the few cells whose content is
more than a scalar, keyed into the same `other` glyph map by the
cluster's string, so a 200 × 50 pane stays 160 KB a frame.

### `.` C38 — The clipboard has no example, in any binding — **done (2026-09-13)**

Filed while building C33, from the question "is there an example of the
clipboard mechanism?" — there was not. Four examples touched it in
passing: `widgets/edit` lists the chords, `features/selection` reads
`selection_text` / `selection_html` in its drive, `widgets/context_menu`
shows the stock rows, and `apps/modal_editor` (since C33) the sink's two
doors from Rust. None showed the **drain** a host sees, the virtual
list's `selectionrange` ask (`tests/virtual_selection.rs` was its only
pin, and the Node `virtual_list` never answered one), or the C33 doors
from Node — and the `selectionrange` event was in no events table and
not in Node's `CoreMsg`.

Done: `examples/rust/features/clipboard.rs` and its Node twin
`examples/node/features/clipboard.tsx`, one subject each (ADR 0021): the
four ways onto the one queue — an `edit` under the runner's chords (no
queue: the drive reads `request_copy` answering at once and
`take_menu_actions` empty), a `selectable` card whose stock menu's Copy
queues text and HTML, a selectable `virtual_column` whose copy over
unbuilt rows is a `selectionrange` the app answers from its rows, and an
`on_key` register bound to `set_clipboard` / `request_paste` with the
paste landing as the sink's `text` event — each pinned by the headless
drive, in both. Both checked on screen with `pbpaste` / `pbcopy`. With
them: a `selectionrange` row in `schema::EVENTS` (so `props.md` and the
Node docs carry it), `SelectionRangeMsg` in Node's `CoreMsg`, and
`docs/howto.md` "How does copy and paste work?" — the map of the
mechanism, which the breakdown that produced this entry was. What the
round did not change: the chord path still bypasses the queue (a
headless test cannot see a chord's copy), a paste still has no address,
and `MenuAction` is still the queue's name — all three discussed, none
asked for yet.

### `~` C39 — A selection stops at the edge, ignores the wheel under a held press, and knows no Shift — **done (2026-09-13)**

Filed and built the same day under
[ADR 0029](adr/0029-a-selection-follows-the-pointer-past-the-edge.md),
whose *What the building changed* section is the record. Found by
running C38's `clipboard` example: drag the log's rows and keep going
below the card, and the selection stops at the last row the frame drew
while the list stays put; roll the wheel with the button held and the
rows move under a selection that does not; Shift-click anywhere and it
is a plain click. Read from the code, the three are one gap in all three
places a selection lives (the stock editor, a `selectable` scope, a
`cells` grid): the live end moved only from the `CursorMoved` arm,
`scope_hit` clamped a point past the edge to the nearest *drawn* run
with nothing asking the scroller to move, `EditStore::drag` never
`touch_caret`ed, and the press read `modifiers().alt` for a block
selection and never `.shift`. ADR 0017 had written "autoscroll is a
call, not a mechanism" and left the call unmade. And a fourth, larger
than the three for the mux use case: a `cells` grid heard no wheel at
all — `scroll_target` knew only scroll containers — so a terminal pane
scrolled the column it sat in, or nothing.

Done as the ADR decided, with `onScroll` widened from `cells` to every
node at the user's ask: `runtime/follow.rs` re-places the live end at
the start of a frame whenever the scroller's *laid* offset (new
`ScrollStore::laid_offset`, since a notch writes the store a frame before
the text moves) or a grid's `origin_line` changed, steps the nearest
ancestor-or-self scroller at 10 px/s per px past the edge (capped 100 px
out, the clock's delta or a sixtieth without one, `animating()` while
stepping, the last point kept across `CursorLeft`, one more re-hit on the
release), and for an `on_scroll` node emits the step as a `scroll` event
with the lines it covers on a grid, the fraction carried; a Shift-press
inside the selection's scope, grid or focused editor keeps the anchor
(cosmic-text's `Drag` is the editor's whole gesture); `EventSpec::on_scroll`
as `P_ON_SCROLL` 99 with the wheel routed through `ScrollRegion::handler`
by paint order; `Core::selection_ends` with doors in Node, Lua and C;
`Step::Modifiers(u32)` and the `selection-extend`,
`selection-scroll` and `cells-scroll` scenes in four adapters (35 scenes
agree); `tests/follow.rs` (nine cases), a C-surface test, two Node tests;
the `clipboard` drives in Rust and Node pin all three gestures on the
log, and `widgets/cells` scrolls its session through the row instead of
two buttons. Checked on screen with a CGEvent driver: the log scrolls
to row 35 in a second past the edge and the highlight follows, Cmd-C
after a Shift-click puts rows 0–37 on the OS clipboard, the terminal's
edge drag reaches line 1215 with the anchor on 1205 and the wheel moves
the screen under the selection. Two defects found on the way and filed as
their own tasks rather than folded in: `selection_range` hands a
backwards drag's ends unordered to the `selectionrange` ask, and a Cmd-C
over the virtual log leaves the windowed runner drawing ~130 frames/s
(on main too). CHANGELOG under alpha.12's `### Added` and `What breaks`.

### Wishes, not entries

Three things an editor will ask for that have an answer today and a
better one later, each parked until a view asks:

- **An underline of its own colour and style** — a diagnostic's red
  wave under keyword-coloured text, a terminal's SGR 58 undercurl.
  `Span::underline` is a bool in the text's colour (C22). Today: a
  `line` element under the run, whose rect a monospace column gives
  for free and `caret_rect` gives otherwise.
- **The middle button** (C2: "reaches the core and routes nowhere") —
  a Linux terminal's paste and an editor's close-tab. Today: nothing.
- **Window position**, declared and read (the README's Status names
  it) — an editor restoring its last geometry. Today: the size, not
  the place.

## From the architecture review, second round (2026-09-13)

The second reading of the whole tree for its own shape, at `6f03459`
(alpha.11 + 69), eleven days and ~490 commits after the first
(2026-09-11, AR1–AR6, all built that day). The first round asked what was
written twice; this one asked what was decided twice — a rule the code
implies in one place and breaks in another — and what was decided once
and never written down. Method: seven readers, one per area (the
runtime, input and text, the schema and tokens, the four bindings, the
drivers and renderer, the tests, the documents), each reading the code
and its tests rather than grepping, each rejecting its own first pass
before reporting; then every defect marked `!` below re-read against
the code by hand, and every claim that dissolved on that reading
dropped. Nothing from AR1–AR6 or from an open entry is repeated. Forty-three
entries, AR7–AR49, and one amendment on top of B1, whose condition this
round met.

Ten of them are one decision each, and the entries under a decision
share a fix; they are grouped that way, defects first within each. The
decisions, in the order they matter:

- **What lives on the session is not written down** (AR7, AR8). The
  rule the code implies: a session member is a registry keyed by a
  process-unique handle, or a revision counter; anything reconciled
  against a *frame* is per window. The audio store crossed it, the
  image drop list crossed it the other way.
- **An event's producer is read off a payload string** (AR11). Two
  post-passes compare `kind` to the events table, whose edit entry is
  spelled `"changed / submit"` and matches nothing.
- **The two key channels disagree about a chord** (AR9, AR10). ADR
  0011 decision 3 says they must not; `edit_event` folds the modifier
  away before the second channel asks.
- **A token miss means three things** (AR14). ADR 0027 resolves at the
  lower, and each lower chose its own default.
- **Focus has four doors and three precedences** at frame end (AR17,
  AR18, AR24).
- **Five leaf doors run five subsets of the spec pipeline** (AR16).
  AR5 fixed two of them; the two it did not touch have AR5's defect.
- **Node's window handle is the main window** (AR12). C and Lua are
  per window because their view-time handle is the drawing core's.
- **B1's condition is met** (B1, AR26, AR27, AR40): thirteen verbs
  reach some bindings and not others, and the C runner takes no window
  at all.
- **Coverage is enforced for elements, not rows** (AR46, AR47, AR48).
- **Backlog ids are reused** (AR49): B1, D1 and D2 each name two
  entries, and code cites both senses.

Rejected on the same reading, so the next round need not repeat them:
the pending Tab step against scroll; `Interaction::pending` beside
`Core::pending`; the `Tree::any_*` flags on the devtools, slot and root
paths; the exit-ghost swap; the float stack's steady state; F15's
retarget; ADR 0009's press/release pairing across a popup and its
owner; the `autofocus` and `keyFocus` edges; F44's `folds` against
`multiline`; `text_hit` against `attach_lines` on coordinates; the
clipboard drain against a redraw; derived-token cycles; the C command
drain; the `Waker` on a closed loop; the access bridge on a closed
window; the drawn and native menu bars on one accelerator; the
Occluded first frame; the audio thread against shutdown; two devtools
windows; the atlas and shape cache (still per core, as ADR 0004 step 1
left them); the driver-side verbs ADR 0020 names; the ABI since 15
(only `[in]` shapes and new functions, no bump owed); the 34 per-file
`fn frame` builders in the tests (scene builders, not copies); the
`inputs_hash` of the access-tree cache (hashes every field `build`
reads); the generated files (all three current and diffed in CI); every
warning literal a test names; every identifier in `howto.md`.

### `!` AR7 — The audio store is the session's and is reconciled against every window's frame

`SessionState` has six members — fonts, resources, audio, `fonts_rev`,
windows, devtools (`crates/kui-core/src/session.rs:88-93`). Five are
registries or a counter. `AudioStore` is not: it holds `declared` and
`mounted` (`audio.rs:320-321`), and `finish_frame` on **every core** ends
with `self.session.state().audio.reconcile()` (`runtime/emit.rs:788`),
which takes this frame's `declared` and stops every `mounted` key not in
it (`audio.rs:594-614`). A popup, a second window, or the popped devtools
window finishes a frame that declared no `<audio>` node, and the main
window's looping sound is stopped and `truncated-playback` raised; the
main window's next frame finds it unmounted and starts it from zero. A
`finish` one-shot is released and replayed. Two readers reached this
independently; `tests/session.rs` covers the imperative `play` across two
cores (`:108`) and never a mount. The `ended` and `refused` events are
stamped `WindowId::MAIN` by hand (`audio.rs:451,480`) for the same reason:
the store does not know its window.

**Fix:** write the rule above into `session.rs`'s module doc; key
`declared`/`mounted` by `(WindowId, Key)` and reconcile the calling
window's slice, or keep the store per core and share only the command
queue; stamp the two events with the mounting window; and add the
two-window declarative case to `tests/session.rs`.

### `!` AR8 — A removed image reaches the GPU of the window that removed it, and a removed fragment reaches no GPU

`remove_image` evicts *this* core's atlas slot and pushes onto *this*
core's `dropped_images` (`runtime/resources_api.rs:412-418`); only a core
that draws a frame forwards the list (`runtime.rs:1217-1219`). A
texture-backed image removed through a window that closes before its next
frame never reaches `dl.dropped_textures`; the other windows' atlases keep
the blit until their own eviction. `fragment_pipelines` in kui-wgpu is
keyed by id and never evicted (`crates/kui-wgpu/src/lib.rs:329-340`);
`remove_fragment` frees nothing on the device. Fonts solved the same
fan-out with `fonts_rev` + `sync_font_names`; images and fragments have no
revision. Read, not measured: a long-running app streaming images through
a secondary window leaks device textures.

**Fix:** a removed-ids list beside `fonts_rev` on `SessionState`, drained
by whichever core renders next, with fragments on it too.

### `!` AR9 — A held key's release is matched on `code`, and Shift changes `code` mid-hold

`KeyDown` pushes onto `keys_held` unless one with the same `code` is
there, and `KeyUp` removes by `code`
(`crates/kui-core/src/runtime/dispatch.rs:448,457`). The runner builds
`code` from winit's `logical_key` at each event with no case folding
(`crates/kui/src/keys.rs:155-158`): hold `w`, press Shift, and the OS
repeat arrives as `W` — a second held key. Release `W` and `w` stays held
until focus moves or the window blurs, when a synthetic `up w` fires at
the wrong time. This is the WASD case ADR 0002 sells `keyUp` for. The
test's premise (`tests/keys.rs:818-830`: "`code` is stable across the
press") is false whenever Shift moves.

**Fix:** identify a held key by `physical` when it is not `Unknown`, `code`
otherwise, for both the repeat check and the release.

### `!` AR10 — The editing channel drops the chord bit, so a chord the sink heard also presses the control; Space ignores every modifier

`route_key` calls any of ctrl/alt/super a chord
(`runtime/dispatch.rs:919`). `edit_event` then folds them to `word: alt,
doc: primary()` (`input.rs:570-576`), so Ctrl on macOS (Super elsewhere)
is gone by the time the `Key` arm asks `bubbles(i, c, mods.word ||
mods.doc)` (`dispatch.rs:396`). Ctrl+Enter on a focused button inside a
sink: the raw press bubbles to the sink as a chord, then `Key(Enter,
Mods{doc:false})` claims and clicks — the disagreement ADR 0011 decision 3
forbids. The test helper `window_press` builds `doc: ctrl || super_key`
(`tests/key_bubbling.rs:113`), a mapping no driver uses, so the test
passes on a path the runner never takes (the F6 trap). Space is
`Text(" ")` "whatever is held" (`input.rs:556`) and the `Text` arm passes
`chord = false` unconditionally (`dispatch.rs:307`): Ctrl+Space (an IME
toggle, an Emacs mark) inserts a space into an editor, or clicks a control
and reaches the sink both.

**Fix:** carry one `chord` bit from the `KeyDown` into `Key`/`Text` (on
`Mods`, or the last `KeyDown`'s `KeyMods` kept on the core) and have both
arms ask it; return `None` from `edit_event` for a Space under
ctrl/alt/super like every other chord; make the test helper derive `doc`
the way `primary()` does.

### `!` AR11 — `attach_lines` reads the producer off `kind` against a table entry spelled `"changed / submit"`; `attach_cells` reads no producer at all

Both post-passes (`runtime/dispatch.rs:60-113`, `:127-172`) add fields to
events after the fact. `attach_lines` decides "pointer-made" by `kind`:
`drag` yes, `menu` no, anything in `schema::EVENTS` no, anything else yes.
The edit entry's kind is the string `"changed / submit"`
(`schema.rs:1655`), so `"changed"` and `"submit"` match nothing and count
as pointer events — a stock `<edit>` under a sink that draws `role="line"`
rows (a shell with a minibuffer, the reference shape) gets `line`, `byte`
and `clicks` on every keystroke, resolved from wherever the mouse rests,
after a `lines_under` walk of the whole sink and a `hit_at`. An app whose
click payload is `{kind:"click"}` gets the opposite: excluded as the
core's. `attach_cells`, written a day earlier, has neither the filter nor
the `press_clicks() == 0` guard `6f03459` gave its twin: a `cells` grid
with `onKey` gets `cell:{row,col}` on every `key`, `text`, `preedit` and
`access` event, and on an Enter-made click, from the cursor's resting
place (`click_node` calls `note_synthetic_click` precisely so this cannot
happen). It also restates `cell_row_col` (`select_api.rs:74-99`) by hand
— agreement by copy, which the copy's own comment promises.

**Fix:** mark an event pointer-made where it is built (a flag on
`UiEvent`, or attach inside `Interaction::handle` for click and drag
only); one attach pass with one `pointer_point(&ev)` that carries the
clicks guard, used by both; `attach_cells` calls `cell_row_col`; the
schema row split into two kinds.

### `!` AR12 — Every `KuiWindow` door but `setViewBinary` addresses the main window

`KuiWindow::core_mut` is `Shell::core_mut` is `panes.first_mut()`
(`crates/kui-node/src/lib.rs:3101-3104`, `crates/kui/src/lib.rs:930-937`).
The `core_methods!` macro puts `focus`, `editText`, `setEditText`,
`isHovered`, `scrollGeometry`, `openMenu`, `selectionText`, `setTheme`,
`setMetrics`, `setTokens` and the devtools setters on that core;
`set_view_binary(…, window)` (`:1500-1515`) is the one per-window door,
and `view(model, name, surface)` hands every window the same surface
(`packages/kui/index.js:443-448`). In a Node app with a second window,
`editText('note')` for that window's editor is `null`, `setEditText` from
`update` lands on main and warns `edit-text-without-editor`, `focus('row')`
moves the wrong window's focus, and every `$token` in the second window's
tree misses. ADR 0027 admits the token case (finding 7, line 473); nothing
documents the rest. C's view ctx is the drawing window
(`kui-ffi/src/run.rs:11-13`) and Lua's env is the drawing `Ui`
(`kui-lua/src/lib.rs:422-431`), so this is Node's alone.

**Fix:** a surface bound to a window (the way `setViewBinary` takes one),
or `KuiWindow` methods routed through `Shell` by window id; a two-window
case in `test.mjs` on `editText`/`focus`/`setTokens`.

### `!` AR13 — `<text>` admits every container and access row, and all four doors drop them silently

`text`'s `ElementDef` has `jsx_rows: None, lua_rows: None`
(`schema.rs:1403-1413`), which `known_prop` reads as "every shared name"
(`:2455-2463`); the encoder maps `null` to `KNOWN`
(`packages/kui/encoder.js:51,90-98`). Every lowering then keeps
`p.style` alone: `text_node(content, p.style)` (`kui-node/src/binary.rs:669-679`),
`.style` in Lua (`kui-lua/src/lib.rs:1218-1229`), `kui_text(…, style)`
(`kui-ffi/src/frame.rs:483`). So `<text live="polite">` — the `live` doc
says "put it on the smallest node that holds the message" — `<text
role="heading">`, `<text label>`, `<text onClick>` and `<text key>` reach
no tree, raise no `unknown-prop`, and `live-region-without-name` can never
fire for them.

**Fix:** give `text` `jsx_rows`/`lua_rows` = the `Target::Style` rows
(+ `key`), or let `known_prop` derive admission from `PropDef::target`
for a style-only element; a `types.tsx` line that `<text live>` is
refused.

### `!` AR14 — An unresolved `$token` has three outcomes, and the published text matches none

Rust: `Ui::token_color` answers transparent and `token_length` zero
(`ui.rs:204-223`). Node: the slot is left out and the core keeps the row's
default (`encoder.js:80-88,399-406`). Lua: the same, and its doc says why —
"not an explicit transparent or zero, which would hide the text a typo
was on" (`kui-lua/src/lib.rs:1795-1803`). `diag.rs:319-331` and
`props.md:229` document the Rust behaviour as everyone's. `color="$typo"`
on a text paints in the theme's fg in Node and Lua and invisibly in Rust;
`width="$typo"` is `Fit` there and `Fixed(0)` here. Coverage differs too:
`cursorColor` throws on `$` in Node (`encoder.js:802`) and resolves in Lua
(`:1386`); `minWidth="$x"` (`encoder.js:399`, `lua:2019`) and `<line
width="$x">` (`encoder.js:761`, `lua:1326`) are "bad" in both while
`color` on the same line resolves; and a `$` in a `keyframes`/`enter`/
`exit` stop errors the whole frame (`slots.rs:71,177`, `binary.rs:540`,
`lua:2036`) — the one place a token is fatal, against ADR 0027's "any
colour or length prop". `Kind`'s type text omits the `$` form
(`schema.rs:335`, `gen-types.mjs:142`), and the sentence the ADR promised
`props.md` never landed.

**Fix:** one miss policy, in the core — `Ui::token_color/length` return
`Option` and "left at the row's default" is the sentence in both docs;
`min`, the stroke width and `cursorColor` through the existing ref
paths; the stops walked for `$` in the binding, or `keyframes::parse`
given a `&TokenLookup`; the `$` form in the `Kind` docs so gen-types
prints it.

### `!` AR15 — Context-menu Cut mutates an editor without a `changed`, and the runner fakes one for Cmd-X

`MenuRole::Cut` deletes the selection and pushes `SetClipboard`
(`runtime/menu_api.rs:402-415`); only `{kind:"menu", role:"cut"}` is
posted. Every other mutation — `apply_text`, `apply_key`,
`ReplaceSelectedText`, AT `SetValue` — calls `push_edit_event(key,
"changed")`. The runner knows: `after_direct_edit` builds a `changed` by
hand after `cut_selection` with the comment "A cut is input too"
(`crates/kui/src/lib.rs:1081-1098`). The drawn context menu and
`activate_menu_item` from a native menu take the core path and post
nothing; a Node/Lua/C app mirroring a field through `changed` desyncs
after Cut. `tests/menu.rs:176-230` asserts the clipboard and `edit_text`,
never the event.

**Fix:** `cut_selection` returns the `changed` (or `perform_menu_item`
pushes it) and the runner's copy is deleted; the test asserts the event.

### `!` AR16 — The line and polygon doors skip the hover resolve, so `hover_bg` on a wedge never paints

`open_content` runs accent → `resolve_hover_style` → `ease_spec`
(`runtime/builder.rs:382-397`); `cells_at` and the image door run hover +
ease (`:572-588`, `:661-676`); `text_edit`, `line` and `polygon` run
`ease_spec` alone (`:602`, `:869`, `:1006`). `hover_tracked()` is true for
`hover_bg` (`spec.rs:1030`), so the wedge is hit-tracked and
`is_hovered`, and `paint_box` reads the declared `bg`. The test titled for
it (`tests/hit.rs:166-193`, "and the wedge's `hover_bg` with it") asserts
`is_hovered` only. This is AR5's defect on the two doors AR5 did not
touch; `accent` is honoured in `open_content` alone; `cells_keyed` pushes
its label before the `tree.is_empty()` check (`:559-563`) unlike every
other keyed door; and `line`/`polygon` carry ~20 identical lines building
the float box.

**Fix:** one `prepare_spec(key, &mut spec)` (accent, hover, ease) called by
every door; one `float_box_for(rect, spec)`; `hover_is_by_shape` asserts
the quad's colour.

### `!` AR17 — `set_focus` on the modal's closing frame loses to the restore; a `keyFocus` edge and a Tab step win

`resolve_modal_focus` restores what the modal displaced unless
`declared_focus` has an edge this frame (`runtime/focus.rs:438-457`). A
pending Tab step is applied after (`emit.rs:355`) and wins; a `keyFocus`
edge wins by the test; an imperative `set_focus` — `Ui::focus`
(`ui.rs:673`), Node `focus('label')` (`kui-node/src/lib.rs:2464`) — made
in the dismissing handler or the view is overwritten. That is F4's
scenario through the door F4 did not cover: an app that closes a dialog
and names where focus lands gets the pre-dialog node instead. Three focus
doors, three precedences, one frame-end pass.

**Fix:** any `set_focus` that moved `focus` since the last frame is an
edge (a `focus_moved` stamp), and the rule is one sentence in ADR 0003's
decision 4.

### `!` AR18 — Assistive-technology requests skip the modal and `disabled` gates every other channel obeys

`handle_access` (`runtime/dispatch.rs:727-859`): `SetValue` sets the text
and posts `changed` with no `interactive(idx)` and no `disabled` check
(`:747-770`); `SetTextSelection`/`ReplaceSelectedText` the same
(`:780-830`); `Scroll*` calls `scroll_by` where the wheel path honours
`ScrollRegion.inert` (`:838-858`); `nudge` checks `disabled` alone
(`runtime/composites.rs:265-283`). Only `Click` "resolves against the
same hit list". With a dialog up, a reader — or a headless test — edits,
nudges and scrolls the inert page, and `Focus` on an editor behind the
modal routes typed text there until the next frame's containment. ADR 0003
decision 5 says the modal is the one boundary.

**Fix:** early-return in `handle_access` when `idx` is `Some(i)` and
`!self.interactive(i) || specs[i].disabled`, which is what the access tree
already refuses to advertise (`access.rs:1158,1181,1188`).

### `!` AR19 — The glyph atlas grows for one oversized item and resets for an oversized set

`alloc_or_make_room` (`atlas.rs:108-128`): on a full page, `reset()`, then
`grow_to` only if the *one* item still does not fit an empty page. After a
reset any normal glyph fits, so a 1024² page never doubles for a working
set that overflows it — three 800×600 atlas-backed images
(`resources.rs:446-451` puts images up to `MAX_ATLAS_SIZE` in the atlas),
or a code view with many sizes plus CJK and emoji. Each frame then resets
mid-emit, every text template is invalidated (`glyphs_built_for` epoch),
and the quads emitted before the reset sample the overwritten page. The
comment at `text.rs:1836-1839` says "rebuild next frame" and nothing asks
for that frame: `animating()` never sees the epoch, so an input-driven app
keeps the corrupt frame until the next event, and an animating one
re-corrupts every frame. Read, not reproduced; the mechanism is the whole
of the function.

**Fix:** grow when a reset happens twice in one frame (or when the frame's
blitted area exceeds the page), and set `frame_requested` whenever
`atlas.epoch` moves during `finish_frame`; a test with a set larger than
one page.

### `!` AR20 — A device that failed to open drops `Play` without answering `refused`

`apply_one` in the runner's audio: `let Some(m) = self.manager() else {
return; }` (`crates/kui/src/audio.rs:277-279`), `self.refused` untouched;
`Device::Failed` is a state (`:166,173`) and the module's contract says a
refused play "goes back as `Core::audio_refused`". On a machine with no
output device — CI, a container, a muted VM — every `play(..).tag()` and
`<audio tag>` neither ends nor is refused; a view sequenced on `sound
ended` hangs, and `active()` stays false so nothing polls. The
decode-failure branch two lines above does push `refused`.

**Fix:** push `playback` onto `self.refused` when `manager()` is `None`.

### `!` AR21 — Popups are one level deep: a press inside a sub-popup dismisses and consumes itself, and keys never reach it

A sub-popup declared by a popup's frame has `owner == popup`
(`session.rs`, `union`). `popups_outside` lists every popup but the
pressed pane (`crates/kui/src/popups.rs:36-44`), so a press in the child
names its parent "outside"; the runner dismisses it and, for a
non-activating parent, sets `consumed` before dispatch and returns
(`crates/kui/src/lib.rs:1484-1500`). The app stops declaring the parent,
the child closes with it, and the row the user pressed never hears the
press. `key_target` is non-transitive (`popups.rs:19-27`) and `together`
pairs a popup with its direct owner only (`:99-108`), so keys at the
OS-focused owner reach the first level; the sub-popup reads
`env.focused == false`. `windows.rs:200-206` expects "a submenu the app
opened from a retargeted hover". Only the press-drag-release path works
for one. Read, not run.

**Fix:** walk the owner chain — exclude a pressed popup's ancestors from
`popups_outside`, and follow `owner` transitively in `key_target` and
`together` to the deepest non-activating popup; a corpus step for a
second level.

### `!` AR22 — A window the OS refused stays live in the registry

`open_pane` on a `create_window`/`new_in` error prints and returns
(`crates/kui/src/windows.rs:164-181`). `WindowRegistry.windows` still
lists the id live, the app already received `phase:"opened"`, and only
`window_closed` tells the registry otherwise
(`runtime/windows.rs:100-113`). `Core::windows()` reports a window that
does not exist; the app never gets `closed`; declaring the name again is
a no-op; `dismiss`/`Focus`/`SetSize` on the id are silent forever. Read,
not run.

**Fix:** on failure call `window_closed(id)` on the declaring core and
route its events, as `close_pane` does.

### `!` AR23 — `ModifiersChanged` is mirrored to the key target only, so the owner's modifiers go stale while a popup borrows the keyboard

The OS delivers the edge to the owner; the runner writes it to
`self.panes[t].modifiers` for `t = key_target(i)` alone
(`crates/kui/src/lib.rs:1409-1414`), and `primary()`/`kmods()` read each
pane's own copy (`pane.rs:359-371`, `keys.rs:160`). Hold Shift, open a
menu, release Shift while it is up, choose: the next key in the owner is
a Shift chord (Shift+Tab walks backwards; Cmd-C checks a stale Cmd) until
the next modifier edge. Read, not run.

**Fix:** write the mirror to both `i` and `t`; the keyboard's state is
one fact for the pair.

### `!` AR24 — "One selection per window" is enforced in one direction

`set_selection` collapses the editor's selection
(`runtime/select_api.rs:31-35,265-269,763-768`), but a keyboard-started
editor selection — `Key(SelectAll)`, Shift+arrows, AT `SetTextSelection`
— never clears `self.selection`; only a primary press does
(`dispatch.rs:538,706`). Drag-select a label, Tab into a field, Cmd-A:
the runner reads `core.selection()` first (`keys.rs:229-236,275-283`) and
select-alls the *scope*, Cmd-C copies the label, and two highlights are
drawn. ADR 0017 decision 1; `tests/selection.rs:171` covers scope→editor
only. Read, not run.

**Fix:** clear the scope/cell selection when `set_focus` lands on an
editor, or when `EditStore` reports a non-collapsed selection after
`apply_key`; the reverse case in `tests/selection.rs`.

### `!` AR25 — `{ percent: 50 }` in JSX is 5000%

`"50%"` lowers to `parseFloat(v) / 100` (`packages/kui/encoder.js:163-165`)
and Lua's `{pct = 50}` to `Sizing::Percent(p / 100.0)`
(`kui-lua/src/lib.rs:2072-2074`); the object form writes `v.percent` raw
(`encoder.js:171-173`), and the core reads `Percent(value)` as a fraction
(`schema.rs:454-458`). `{ percent: number }` is in `SizingProp`
(`jsx-runtime.d.ts:74-81`) and in no doc, example or test — which is why
nothing caught it.

**Fix:** divide by 100, or drop the object form from the type; a
`test.mjs` line that `{percent: 50}` and `"50%"` encode the same bytes.

### `!` AR26 — Lua's `env.edit_text` takes an integer only, and its `on_event` drops the window

`edit_text` is `move |_, key: i64|` (`kui-lua/src/lib.rs:502-505`); its
own doc says "each takes either spelling" (`:393-396`) and `set_edit_text`
two doors below takes a label. `env.edit_text("filter")` fails with mlua's
"integer expected, got string", and the one Lua example keeps the integer
from a `changed` event to work around it
(`examples/lua/features/slots/panel.lua:61,74-75`) — the shape F5 and F32
removed everywhere else. The event table sets `node_key` and `from` and
nothing for `ev.window` (`:234-260`), where Node sends `{origin, window,
key, payload}` and C's `KuiEvent` carries `window`; `env.window.id`'s doc
promises "every event from it carries the same number"
(`schema.rs:1890-1896`), so a Lua panel drawn into two windows cannot tell
which one clicked.

**Fix:** `edit_text` takes `mlua::Value` through `key_query` like
`is_focused` beside it; `t.set("window", ev.window.0)` beside `node_key`;
the example reads back by label.

### `~` AR27 — C's runner takes no window, and `kui_run_with` drops what the ctx registered

`kui_run`/`kui_run_with` build `kui::app(title).with_extensions().system()`
and nothing else (`kui-ffi/src/run.rs:31-60`; `kui.h:2705,2728`):
no size, min, max, chrome or text AA, where `Launcher` has all five
(`crates/kui/src/lib.rs:164-290`) and Node's window options have them
(`kui-node/src/lib.rs:1432-1484`). A C app cannot open at a size (only
`kui_set_window_size` from its first view, after the window has shown),
cannot ask for custom or borderless chrome (so `kui_titlebar` draws
under a native title bar), and chooses AA through `KUI_TEXT_AA` alone.
`kui_run_with` then takes the extensions and the system pin off the ctx
(`run.rs:86-97`) and lets `kui::app` build a fresh session: fonts, images,
sounds, tokens, `kui_set_devtools`, `set_native_menus(false)` and the text
cache budget registered on the ctx before the call never reach the
window, and a C host that mirrors Node's register-then-run order gets
`foreign-resource` warnings from its first frame. ADR 0020's "Not done
here" names neither.

**Fix:** a size-led `[in]` `KuiRunConfig {size, width, height, min_w,
min_h, max_w, max_h, chrome, text_aa, diagnostics}` on `kui_run_with`
(no ABI bump under the `[in]` rule), and the ctx's `Core` handed to the
launcher as its main core via `Launcher::setup_core`; the header says what
still cannot cross.

### `~` AR28 — No keyboard path starts or extends a selection in a `selectable` scope

A scope's drag is armed from a press only (`runtime/dispatch.rs:559-571`)
and `select_all_in` is a chord (`select_api.rs:278`); the stock editor has
Shift+arrows in `apply_key` and cells have `select_all_in`. ADR 0017 makes
a scope "the same family as keyboard focus" and says nothing about the
keyboard; AT gets `SetTextSelection` on editors only. A keyboard user
cannot select a label.

**Fix:** Shift+arrow/Home/End on a focused scope (or a control inside one)
moves `Selection.focus` through `scope_offset`, mirroring the editor's
motions; or a sentence in ADR 0017 declining it.

### `~` AR29 — The sink's caret and IME anchor read the focused node's subtree; keys read the enclosing sink

`sink_caret_line` walks `(focus_index() .. subtree_end)` for the first
`role="line"` with a caret (`runtime/emit.rs:1425-1440`), while
`sink_event` and `key_target` route to `enclosing_sink`
(`runtime/dispatch.rs:1128-1136`, `:994`). Focus on a control inside the
custom editor (AT `Focus`, `ui.focus`, a pane button) still delivers keys
and commits to the sink, but `has_caret()` turns false — the blink clock
un-arms (C35), the caret parks solid, the IME candidate window loses its
anchor. The walk also does not skip `Role::None` subtrees or nested lines
as `lines_under` does, and takes the *first* caret where `custom_editor`
takes the *last* (`access.rs:1349`).

**Fix:** start the walk at `enclosing_sink`-or-self and take the candidates
from `lines_under`.

### `.` AR30 — Three meanings of `line`, and a `byte` that counts text the access tree excludes

`TextHit.line` is the wrapped row *within one run's buffer*
(`text.rs:645-660`, `visual_line`) while `byte` in the same struct spans
the node's runs; the payload's `line` is the ordinal `role="line"` node
(`dispatch.rs:229`); the AT's `line` is the same ordinal (`access.rs:1364`).
A Node app reading `ev.line` from a drag and `ui.textHit(key, pt).line`
gets different numbers for the same point. `attach_lines` says `byte` is
"what `text_hit` would answer", and `text_hit` concatenates *all* text
under the line key (`runs_of`, `text.rs:2053-2068`) where `custom_editor`
skips `role="none"` subtrees (`access.rs:1349-1352`): a fold marker or
inline line number in the row shifts `byte` against the `access` events'
`offset` by its length. A text nested more than `PLACE_ANCESTORS = 4`
levels under its line (`text.rs:589-592`) answers `byte: 0` with no
diagnostic.

**Fix:** `TextHit.line` counts rows across the node's runs and is
documented as "visual row"; `runs_of` gets the `Role::None` skip; a diag
when a text under a `line` row exceeds `PLACE_ANCESTORS`.

### `.` AR31 — Hover ignores the scrollbar the press and the cursor honour

ADR 0023 decision 4 routes press and cursor through `target_at`
(`input.rs:1179`, `:1395`); `refresh_hover` resolves against `hits` alone
(`:1131-1136`). Over an overlay bar the cursor is `Default` and a press
grabs the thumb, while the node beneath lights `hover_bg` and fires
`onHover enter`.

**Fix:** hover through `target_at`, with `Target::Bar` as "nothing hovered".

### `.` AR32 — A popup surface is resizable and, under custom chrome, gets the app's non-client treatment

Popup attrs come from `window_attrs("", size, chrome)` plus `undecorated`
and no `with_resizable(false)` (`crates/kui/src/windows.rs:138-146`);
popups take `self.chrome` (`:79-86`); `nc` is installed for any non-Native
chrome with `resize_border: true` (`:281-285`); `synthesizes_resize` is
true for a popup on Linux under Custom (`pane.rs:296-302`). Dragging a
menu's edge resizes it (AppKit keeps edge resizing on a hidden-titlebar
window; Win32 keeps `WS_THICKFRAME` undecorated), and under
`Chrome::Custom` the outer 6 px of every popup answers resize cursors and
`HTCAPTION` from the popup's own chrome regions. Read, not run.

**Fix:** `.with_resizable(false)` on popup attrs, `Chrome::Borderless`
semantics for `WindowKind::Popup`, and no `NcHitTest`/`synthesizes_resize`
for one.

### `.` AR33 — Retarget's common frame is physical pixels, which AppKit does not have

`retarget.rs:9-12` and `Surface::origin` (`pane.rs:285-293`, from
`inner_position()`) assume one physical screen frame — the Win32 model,
which `mixed_dpi_maps_through_physical_pixels` pins. winit's macOS
`inner_position` is points × *that window's* `scale_factor`
(`window_delegate.rs:928-932` in winit 0.30.13), so two windows on
displays of different scale have origins in two frames; `popup_position`
converts with the owner's scale (`windows.rs:213-236`). A popup on a
Retina/non-Retina boundary retargets moves to the wrong rows and
classifies the release as `Outside`; on Windows the inverse hits
`popup_position` (`with_position(LogicalPosition)` resolves against the
creating monitor). Mechanism read; frequency low.

**Fix:** `Surface` platform-aware — points as the common frame on macOS,
`PhysicalPosition` into `with_position` on Windows.

### `.` AR34 — `request_paste` has no outstanding-request guard, so both examples carry one

`pub fn request_paste(&mut self) { self.menu_actions.push(MenuAction::Paste) }`
(`runtime/select_api.rs:561-563`). The drain dispatches `Commit`, `dispatch`
asks for a redraw, the redraw runs `view`; a view that calls
`request_paste` while a flag is set loops paste→redraw→paste until the
answer lands. Both Rust examples hit it and added `paste_pending` app-side
(`6f03459`); a Lua or Node view — the only place `Ui` exists there — has
no other place to put one. `answer_selection_range` has the
`awaiting_selection` gate the paste lacks (`:512,521`).

**Fix:** one `awaiting_paste` in the core, duplicate `Paste` dropped while
set, cleared on `Commit`, readable so a view can ask.

### `.` AR35 — `Metrics::scaled` scales `titlebar_h`, the row the schema marks the platform's and `compact()` exempts

`scaled` loops every `METRIC_ROLES` row (`metrics.rs:153-159`); the row
is `platform: Some` — "the OS's number and not a density"
(`schema.rs:2225-2231`), which `compact()` honours (`metrics.rs:124-127`)
and `tests/metrics.rs:124-125` pins the other way (`scaled(2.0)` doubles
the bar). A density slider at 1.5 draws a 51 px caption strip beside 34 px
traffic lights.

**Fix:** skip `platform.is_some()` rows in `scaled` and flip the test, or
delete the "not a density" claim.

### `.` AR36 — `nodes()` rects are window px; every other readback is dock-shifted viewport px

`layout_of` returns `r.x - shift.x` (`runtime/inspect.rs:63,238-242`), as
do `selection_rect`, `scroll_geometry`, `text_hit`, `caret_rect` and
`cursor` (`select_api.rs:367-395`, `scrolling.rs:68-95`,
`runtime.rs:853-866`, `builder.rs:285-287`); `snapshot_nodes` stores
`tree.pos[i]` unshifted and Node exposes it as `nodes()`
(`kui-node/src/lib.rs:2312`). Under a left dock, `nodes()[k].rect.x` and
`layoutOf(k).x` disagree by the dock width for the same node. The panel's
own outlines need window px, so the snapshot cannot move in place.

**Fix:** keep `inspected` in window px for the panel and translate in
`Core::nodes()`, the way every other readback does.

### `.` AR37 — `Core::finish_frame` is a public second door that skips the fills, the menu and the devtools

`Core::frame`'s doc says "then `finish_frame()`" (`runtime.rs:1040-1047`);
`Ui::finish` runs filler → `devtools_finish` → `build_menu` →
`finish_frame` (`ui.rs:831-845`). No caller outside `Ui::finish` (one Node
unit test aside), but a Rust host following the doc gets a frame where
`open_menu` draws nothing and `KUI_DEVTOOLS` does nothing, with no
warning.

**Fix:** `pub(crate)`, or the three steps inside it behind an optional
filler; the doc points at `Ui::finish`.

### `.` AR38 — `devtools.inspecting` is a one-way latch

`want_inspect = d.tab == Tab::Tree && !d.inspecting` →
`set_inspect(true); inspecting = true`
(`runtime/devtools/mod.rs:928,969-971`); nothing writes `false`, and
`set_devtools(false)` resets `pick` alone (`:690-704`). After the tree tab
was opened once, `snapshot_nodes` (an O(nodes) clone per frame) keeps
running after the panel is closed; a host that calls `set_inspect(false)`
itself leaves `inspecting == true`, so the panel never re-asks and its
tree tab stays blank for the session. No test covers either.

**Fix:** drop the latch and derive `set_inspect(d.on && d.tab == Tree ||
host_asked)` each `devtools_begin_frame`, the host's ask kept apart.

### `.` AR39 — The runner indexes a pane by position after a command may have removed it

`dispatch` runs `apply_window_commands` then `self.panes.get_mut(i)`
(`crates/kui/src/lib.rs:952-960`); `lib.rs:1520` and `:1601` re-find by
id. A chrome close on pane *i* queues `Close(i)` → `panes.remove(i)` →
`get_mut(i)` is the *next* pane, which gets the cursor apply, the
`pending_input_ms` charge and the redraw. Cosmetic today; `on_key` indexes
`self.panes[i]` after its own `dispatch` (`keys.rs:225`), one chrome-shaped
key away from an out-of-bounds.

**Fix:** `let here = self.panes[i].id` at the top and `pane_of(here)` after
the drain.

### `.` AR40 — Three small door rules broken: `index` on a button, `cells` cursor names, a lone `width`

The stock `<button>` admits `key` and not `index` (`schema.rs:1372-1389`
vs `:1196`, "declared beside `key` the index wins"); in a `virtual_column`
a `<button index={i}>` warns `unknown-prop` and is keyed by its text,
losing focus and tween identity as the range slides
(`encoder.js:659-668`, `lua:1505-1507`). The `cells` cursor shape is a
literal `['block','bar','underline']` in the encoder (`encoder.js:799`)
where every other value table comes from `protocol()`; `CursorShape::NAMES`
exists and is not exported (`cells.rs:93`); Lua defaults an unknown name
to `Block` silently (`lua:1382-1385`) where Node throws. Node's window
options honour a lone `minWidth` and ignore a lone `width` and an unknown
`chrome` (`kui-node/src/lib.rs:1432-1436,1480-1484`), against the
encoder's "refuse, don't drop".

**Fix:** `index` in both `BUTTON_ROWS` lists and the three lowerings;
`NAMES` through `protocol_tables`, Lua `bad(...)` on a miss; an error on a
half-given size and an unknown chrome word.

### `.` AR41 — `button_spec` hard-codes the accent trio `Theme::dark()` also hard-codes

`widgets.rs:483-491` and `theme.rs:193-195` carry the same three hex
values, cross-referenced by comment; `button_with` substitutes the theme's
only when `spec.accent && has_accent()` (`widgets.rs:568-574`). ADR 0019's
"`button_spec()` is unchanged" is why: a `set_theme(Theme::light()
.with_accent(brand))` recolours the ring, selection, menu rows and
scrollbar, and a plain `<button>` stays `#3b5bd4` — the one stock widget
not painting from the palette.

**Fix:** `button_spec(&Theme, &Metrics)` reading `accent`/`accent_hover`/
`accent_pressed` (byte-identical for `Derived` with no accent, since the
trio is the same) and no `has_accent` gate.

### `.` AR42 — `Easing`, `Repeat`, `Live` and `FontFamily` are index→variant matches with no pin

`schema.rs:299-329,930-934` map an index to a variant by hand; `anim.rs`
has no `ALL`/`name()` (`:32-46,78-90`); `Live::from_index` is a second map
beside `LIVE` (`access.rs:243-249`). CURSORS (`schema.rs:162`), the env
enums (`env.rs:449-462`), ROLES (`schema.rs:3232`) and SCROLLBARS
(`ScrollbarMode::ALL[i]`) are pinned. Appending to `Easing` or reordering
`EASINGS` compiles and maps every C/Lua/Node index one off; `abi_enum!`
pins the header to the *names*.

**Fix:** `ALL` + `name()` on the four, the lists as `ALL.map(name)`, one
round-trip test, as `Appearance` already has.

### `.` AR43 — Node's `windows` root prop is hand-lowered on both wire sides while `WindowConfig::from_value` exists

A ten-slot stanza in the encoder (`encoder.js:340-380`) and its mirror in
`binary.rs:462-484`; `WindowConfig::from_value` (`window.rs:208-250`) is
what Lua uses (`lua:1130-1145`), and `menuBar` already rides as one JSON
blob for the same reason (`encoder.js:846`, `binary.rs:914`). The two
already disagree: `{width: 0, height: 0}` is "default size" in Node
(`w > 0.0 && h > 0.0`) and `Size(0,0)` in Lua and the core.

**Fix:** `windows` as a JSON string through `from_value`, both arms
deleted (a protocol bump, as v7 was for this stanza).

### `.` AR44 — `index.d.ts`'s `Env`, `NodeInfo`, `Theme` and `Metrics` are hand-written mirrors of generated tables

`index.d.ts:919,947,1034,1141` restate `ENV_FIELDS`, `THEME_ROLES` and
`METRIC_ROLES`; `gen-types.mjs:46` already destructures `env, theme,
metrics` from `protocol()` and uses them for `props.md` alone. In sync
today (23/23, 16/16); a role added to `THEME_ROLES` shows in `ctx.theme()`
and every generated list and is a type error until someone edits the
interface, and `types.tsx` catches only the roles it uses.

**Fix:** the four interfaces emitted between the `-- generated --` markers
the access, warning and input lists already use.

### `.` AR45 — Three frame counters, two label resolvers, one caret rect twice

`Core::frame_no` (`runtime.rs:385`) is passed to edit and scroll;
`AnimStore` and `DepartStore` each `frame_no += 1` in their own
`begin_frame` (`anim.rs:311,333`, `depart.rs:272,291`), in lockstep only
because `begin_frame` happens to call all three, and their `sweep_cutoff`
runs on a different modulus than `layouts.retain` (240). `key_of`
(`builder.rs:335-353`) and `resolve_regions` (`focus.rs:279-292`) do the
same label find and spell `ambiguous_key` twice. The stock caret's viewport
rect is computed in `emit.rs:1400-1412` (IME anchor) and
`scrolling.rs:117-137` (scroll into view), each `(0..len).find(content ==
Edit(key))` then `pos + pad + caret/scale`.

**Fix:** `Core::frame_no` into the two stores' `begin_frame`; one
`find_label` under both callers; one `stock_caret_viewport_rect(key)`.

### `.` AR46 — `surface.c` says every prototype is called once; twenty-one are called by nothing

`examples/c/tools/surface.c:1-2`: "every prototype in kui.h called once".
Of 202 `pub extern "C" fn kui_*`, 90 are absent from it, and these are
referenced by no test, adapter or example in the tree:
`kui_activate_menu_item`, `kui_answer_selection_range`,
`kui_clear_selection`, `kui_cursor_shape`, `kui_focus_region`,
`kui_fragment_open_with`, `kui_fragment_remove`, `kui_fragment_source`,
`kui_image_pixels`, `kui_region`, `kui_reveal`, `kui_scroll_offset`,
`kui_select_all_in`, `kui_selection_html`, `kui_selection_text`,
`kui_set_devtools`, `kui_set_devtools_dock`, `kui_set_native_menu_bar`,
`kui_set_scroll`, `kui_set_text_cache_budget`, `kui_text_cache_bytes`.
`every_entry_point_is_pinned` (`abi_parity.rs:1032`) pins signatures, not
calls — the header audit is a test, but a different one from what the
walk claims. A door nothing calls can decode its arguments wrong for a
release without failing anything.

**Fix:** a test in kui-ffi that every `abi_fn!` name appears in
`surface.c`, with an exempt list and the reason per line; the walk's
comment made true.

### `.` AR47 — Coverage is pinned for elements; 31 generic rows, `animate` among them, are in no scene and no test

`the_corpus_covers_every_hand_written_row` iterates `CUSTOM` and
`ELEMENTS` (`tests/conformance.rs:136-183`). Absent from every scene in
both spellings: accent, animate, center, clickSound, crossAlign, delay,
easing, ellipsis, enter, expanded, features, focusBg, focusRegion,
hoverGroup, hoverSound, initialFocus, keyframes, mainAlign, maxHeight,
maxWidth, maxLines, onForceClick, onHover, onLayout, radiusTL/TR/BL/BR,
slide, repeat, selectionAnchor, shadowX, strikethrough, underline. Six of
those shipped since 09-06. `animate` — the row that takes a window off
input-driven pacing — has no behavioural test in any binding (`grep
'.animate(\|animate:'` over every test tree: the C struct round-trip in
`schema_parity.rs:380` alone); a regression there is the silent idle-CPU
class C27 measured. Node's "every generic prop reaches the stream" test
asserts `encoded(with) != encoded(without)` and `quadCount > 0`
(`test.mjs:88-108`) — a prop under a neighbour's id passes — and the only
test that catches a tag or width swap is the corpus digest, which skips
without `KUI_CONFORMANCE` (`:4016`); the Rust twin re-implements the
encoder's slot layout by hand, so a lock-step change to both passes.
`bench-check.sh` guards 5 of 35 `frame` rows (`:139-147`) and no other
bench file; `frame_10k_rects_with_access_tree` (the row `18cf953`'s cache
is justified by) and every `list_*` row are unguarded, the class C29
found regressing.

**Fix:** the coverage test over `PROPS` with an `UNDERIVED`-style exempt
list, or one sampler scene; a `tests/anim.rs` case — one frame with
`.animate()` → `animating()`, the next without → not; the Node test reads
the node back through `nodes()` and asserts the value survived; the two
bench rows guarded and `bench-check.sh` taking a bench name.

### `.` AR48 — Test helpers re-derived beside `kui_core::testing`, and font tests that pass with no font

`fn ks(&str) -> KuiStr` appears five times in kui-ffi's tests
(`src/tests.rs:9,348,686,1430,1585`); devtools' `click_at` is a
byte-identical copy of `testing::click_at` (`devtools/tests.rs:59-64`)
though `testing` is reachable in-crate; a raw-key `press` is in
`tests/keys.rs:38` and `tests/focus_regions.rs:111` and not in `testing`;
four payload extractors do one "string-or-tag" read. `d0dd348` stopped
one directory short. Separately: `tests/fonts.rs:47` and
`tests/session.rs:91,192` return green when none of four hard-coded font
paths exist, and `font_features.rs:59-66` prints "skipped" and returns —
on CI (`fonts-dejavu-core` only, `ci.yml:233`) C23's ligature effect is
pinned on nobody's machine.

**Fix:** `key_down`/`payload_str` into `testing.rs`, `ks` hoisted, devtools
tests on `crate::testing`; a tiny test-only font with a `liga` table under
`tests/fixtures/`, or at least a stderr line when a test skips.

### `!` AR49 — The documents disagree with the code and with each other in eleven places

The `!` is for the first; the rest are `.`.

- `CHANGELOG.md`'s alpha.12 "What breaks" says the Node frame is
  **version 10** (`:24`), then "`KUI_ABI_VERSION` stays 15 and the Node
  binary frame stays 9" (`:40`), then a bare "Nothing." (`:45`) — three
  leftovers of the eight-branch merge (`47eb0ea`); `binary.rs:62` is 10.
- Backlog ids reused: `B1` is the open verb-surface entry (`BACKLOG.md:1434`)
  and the archived `schema::ROLES` one (`closed-2026-09.md:4895`), and the
  index resolves it to the archive; `D1` and `D2` each name two archived
  entries (`:4275`/`:7643`, `:4319`/`:7684`), the index lists each twice,
  and code cites both senses with nothing to tell them apart
  (`kui-node/src/schema.rs:8` the old pair, `devtools/icons.rs:2` and
  `devtools/tree.rs:215` the new). `tests/docs.rs:146` builds its closed
  set from the index, so a `backlog B1` in `howto.md` meaning the open
  entry would be refused as stale. The archive is counted "a hundred and
  ten" (`:11`, `:2411`) and "all hundred" (`:2416`); it has 125 headings
  and 123 unique ids. The hygiene inventory (`:2379`) omits T5 and
  C32–C38; the intro's F-range skips F24.
- ADR 0017 is `status: proposed` (`adr/0017:2,8`) and built (`schema.rs:144`,
  `select_api.rs`, seven CHANGELOG entries, `README.md:835`); its decisions
  run 8 before 7 (`:430,460`) and the CHANGELOG cites "decision 7".
- ADR 0006 puts the size handshake on "the four [out] structs"
  (`adr/0006:88-94`); the header has it on ten. ADR 0021 ticks
  `smoke-examples.sh`/`smoke-headless.sh` (`adr/0021:682-686`), which AR4
  deleted for the `smoke` binary, unmentioned there. ADR 0028 shows
  `ColorOp::Mix(TokenRef, f32)` (`adr/0028:344`); the enum takes names
  (`tokens.rs:92-99`). ADR 0014:284 and ADR 0009:249 point at headings that
  moved. `access.rs:528` sends F8 to `BACKLOG.md`; it is in the archive.
- `ENV_FIELDS` and `SystemEnv` say the core acts on none of `system.*`
  (`schema.rs:1841`, `env.rs:66-69`, `props.md:264`); since ADR 0019
  `set_system` refreshes the theme (`runtime.rs:501-512`) and forty lines
  down `props.md:303` says a button, a field and a scrollbar all follow the
  OS.
- `env.window.always_on_top` is documented as "what the platform did"
  (`runtime/windows.rs:262-268`, `window.rs:389-394`, C30) and is the
  request echoed back (`pane.rs:59-63`): winit has no level getter, and the
  only platform input is `RawWindowHandle::Wayland`. A pin button draws
  "pinned" on a fullscreen space.
- `binary.rs:44-64`'s version history stops at v9 under `VERSION = 10`;
  `KUI_WINDOW=WxH` is read by the runner (`crates/kui/src/lib.rs:1263`)
  and documented nowhere; C34's CHANGELOG bullet predates `6f03459`'s
  narrowing (a keyboard- or AT-made click carries none of the three).

**Fix:** delete CHANGELOG lines 40–45; retire the reused ids with a suffix
(`B1a`) and a uniqueness assertion in `tests/docs.rs`; the ADR headers and
the five doc comments reworded to the code; `NSWindow.level` /
`WS_EX_TOPMOST` queried in `sync_env` or the two docs say "what was asked".


## From the standard-menus round (2026-09-14)

[ADR 0030](adr/0030-the-standard-menus-the-runner-keeps.md) gave every
macOS process the Edit and Window menus a Mac app is expected to have
when it declares none, and registered a declared `Window` menu as the
platform's. Two things were seen while building it and left, on
purpose.

### `.` W14 — The standard Edit menu's rows never grey

ADR 0030, decision 3: the rows are the chords — Copy *is* ⌘C — and
they carry no `MenuRole`, so nothing validates them. Copy with nothing
selected, Undo with nothing to undo and Paste with no editor focused
are all lit, and choosing one does what the key would have done, which
is nothing. The keyboard has no greying either, so nothing is lost
against the state before; what is lost is the Mac convention. The
condition that would build it: a `BarTarget` that answers
`validateMenuItem:` by asking the front pane's core (`edit.focused()`,
`selection()`, `cell_selection()`, and an undo-depth reading the
`EditStore` does not expose today), on every menu open, with the
platform's own autoenable turned back on for that one menu. Not free —
it is a read of the core from inside an AppKit callback the runner did
not schedule, the kind of re-entrance `macos_menu` exists to avoid —
and nothing has asked.

### `~` W15 — AppKit's own Edit rows arrive unchecked

Setting a bar with a menu titled `Edit` makes AppKit append Writing
Tools ▸, AutoFill ▸, Start Dictation… and Emoji & Symbols to it — to
the standard bar and to a declared one alike, now that both go through
`setMainMenu:` of a fresh root. Emoji & Symbols (`orderFrontCharacterPalette:`)
and Dictation insert through `NSTextInputClient`, which winit's view
implements and kui reads as `InputEvent::Commit` (C17), so they most
likely type into a focused editor and reach a key sink as text; Writing
Tools and AutoFill want an `NSTextView`-shaped responder and most likely
do nothing. Neither was driven: the AX audit can press the rows but not
the palette they open. To check by hand, and to say in `howto.md` which
of the four work — or, if the two that cannot work are worth hiding,
to name the menu something other than `Edit` in the standard bar, which
is the one lever the runner has (ADR 0018, *what running it showed*).

## After alpha.11

Grouped by kind, not urgency. Nothing here blocks the tag. It was "After
alpha.10" until 2026-09-11, when the eleven rounds between the two tags —
the paint order, the theme, the examples, the devtools, the canvas, the
architecture review and the rest — had all landed and the heading moved
with the tag; "After alpha.9" before that, until the first Windows round
landed for alpha.10; "After
alpha.8" before that, until the editor-and-mux round landed for alpha.9;
"After alpha.7" before that, until ADR 0012's remainder and ADR 0013 landed
for alpha.8; "After alpha.6" before it went to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#after-alpha6) whole
rather than accumulating strikethroughs, because every line of it had closed.

**Build.** ADR 0004 step 1's leftover: the glyph atlas and shape cache are
still per window because `Core::output` hands out `&mut GlyphAtlas` (see
`kui-session-atlas-constraint`); it waits for a case where two windows share
enough text to matter. C15 is closed (2026-09-07): its remainder was
profiled and the passes that could be skipped are, and what is still above
the 2026-08-31 baseline is the struct's size in the app's own builder chain,
which the archived entry measures and leaves.

**Build next.** The second architecture review, filed 2026-09-13 above
as AR7–AR49, in the order its decisions argue for: AR7 and AR8 (the
session rule written and the audio store and image drops moved under
it), AR9–AR11 together (the held-key identity, the chord bit carried
into the second channel, one attach pass with a producer-side mark),
AR12 (Node's window handle), AR13–AR16 (the `<text>` rows, one token
miss policy, Cut's `changed`, one `prepare_spec`), AR17–AR18 (the focus
stamp and the AT gates), AR19–AR25 as the defects they are, then B1's
table with AR26, AR27 and AR40 beside it, AR46–AR48 for the tests, and
AR49 first of all if the alpha.12 notes go out before the rest, since
its first line is the CHANGELOG contradicting itself on the frame
version. The `~` and `.` entries between wait for the defects.
Before it, the third editor-and-mux round, filed 2026-09-13 above,
in the order its entries argue for: C32 first (every mono glyph on a
machine without Noto Sans Mono is whatever face cosmic-text's fallback
pops, italic here — a defect under the flagship use case and older than
alpha.1 — **done the same day**: the three generic families pinned to
the first installed face of a per-platform list, a `fonts` row in the
facts tab), then C33 and C34 together — **both done the same day**, the
sink's clipboard as the two menu actions with doors in four bindings
and a paste as a commit, the press's `line` / `byte` / `clicks` as
`attach_lines` beside `attach_cells`, `modal_editor` on all of it — and
C35 **the same day** (the clock armed on the sink's `caret` row, the
phase readable in four bindings), with C38 (the clipboard example and
article) filed and built between them, and C36 + C37 **the same day**
(the three drives, the doc line), and C39 **the same day** — the
held drag following its scroller, Shift extending, `onScroll` on every
node with a grid's lines, [ADR 0029](adr/0029-a-selection-follows-the-pointer-past-the-edge.md)
— (a sink's clipboard, and the press
carrying `line`, `byte` and `clicks` the way a grid's carries `cell`),
C35 (the blink clock armed for a sink's `caret`), and C36's drives to
pin all four; C37's doc line goes with whichever lands first. Before
it, the alpha.11 field round, filed 2026-09-12, is built
whole and merged the day after — F42 (the pump order in `runWindowed`
plus the held seed's redraw), F43 (`env_facts().viewport` is `dt_area`
and `size()` answers for the dock), F44 (`wrap` on a single-line editor,
in four bindings and the `controls` scene), F45 and F46 (`clock` in the
type, `tick.every` a function of the model), F47 (the launcher pin over
`env.system`), F48 (`system.assistive`, with `kui_env_set_assistive`) and
F49 (the `finish` arithmetic) — each with its outcome on top of its entry
above. The merge itself settled one thing no chip could see: F47's pin
and F48's field meet in `system_reading`, which now takes the bridge's
reading beside the pin, and `SystemEnv::over` leaves `assistive` to the
bridge (a pin cannot say whether anyone is listening). T4 is built —
[ADR 0027](adr/0027-tokens-beside-the-theme.md), written and built
2026-09-12: colour and length tokens beside the theme and the metrics,
referenced by typed name and resolved by the binding — and T5, the
derived tokens it left out, was **built 2026-09-13** under
[ADR 0028](adr/0028-derived-tokens.md): a colour token that is a chain
of operations over an earlier one, resolved by the core on read. C31, C29 and W13 went together on the evening of
2026-09-12: the Node adapter's `fragments` scene named raw 1 as a dead
image and raw 1 is the first handle a fresh process mints, so the
fixtures remove one now; the four bench rows were swept commit by
commit, the segment row's +12% was ADR 0023's chrome call per leaf
float and half of it is back, the row guarded; and `props.md`'s
metrics table carries both platforms' `titlebar_h`. Before them
the open list was C12, C13, C14, F36, B1 and V2–V8 — every one parked on a
condition — and one entry with work in it: C30, always on top,
filed 2026-09-12 as a per-frame fact in `window_title`'s shape with a
door per binding and a readback in `env.window`, and **built the same
evening** as written, less the ABI bump the entry assumed and the
header's own rule does not ask for. C27 is parked with its measurements. What that round settled
is on top of each entry in the archive; the two questions worth carrying
forward are the ones it answered by building: a metric never scales by
itself and the stock set is the corpus's contract (T2), and the core keeps
a row still rather than skipping rows (C26).
[`docs/adr/0016-caching-against-the-last-frame.md`](adr/0016-caching-against-the-last-frame.md)
was **proposed and its one yes built on 2026-09-09** out of the performance round that shipped the quad
shrink and closed C24, and it is written to be mostly declined: no general
subtree cache (a 2.5× ceiling, against `virtual_column`'s 218× and `cells`'s
37× where either applies, and a list of inputs a digest cannot see that has
to stay complete forever), no `memo` element ever in the form that asks a
view to declare its own dependencies, and one narrow yes — the derived
access tree, which is about 470 µs and +40% of a frame while a screen reader
is attached, is output the core never acts on, and has inputs you can
enumerate. Its two measurement sections are why: detection is affordable (a
per-node digest at push costs about 0.8% of a frame, if the fold happens at
close rather than in a pass of its own) and the ceiling is not (the app
rebuilds the tree either way, which is 40% of the bill). Decision 3 was the only part with code
to write and it is written: `access::inputs_hash` beside `access::build`,
hit-checked in `Core::access_tree`, worth **−21.7%** on
`frame_10k_rects_with_access_tree` against a ±3.0% floor. Its gate is 21
cases in `runtime::dispatch::access_cache`, one per input, and the
amendment records what building it changed — including that two of those
cases failed first and both were the test's fault, not the hash's. The `fragment` element, [`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`](adr/0015-a-fragment-element-and-the-painter-it-is-not.md), was proposed, measured twice, accepted and **built on 2026-09-08** — a box a registered WGSL function paints, in four bindings with a corpus scene, plus `animate` as a plain row and ABI 9. Its two measurement sections are why it was accepted (naga costs a cold build of `kui-core` alone; the draw-call split costs about 0.6 µs of CPU a fragment and nothing the GPU can see) and its amendment is what the building changed — including a collision the design could not have seen, that JSX's own `Fragment` sentinel was the string `'fragment'`. Before it, slots, [`docs/adr/0014-slots-an-extension-fills-in-place.md`](adr/0014-slots-an-extension-fills-in-place.md), were proposed, accepted and **built on 2026-09-07** for alpha.9 — an extension fills a place the host declares in its own view, under a namespace the host decides, with `Value` parameters in and replies out; its status block records what the building changed, and the first test it pins was a defect before it: an extension whose root carried no `key` was rekeyed whenever the host added a child at the root. Before it, ADR 0012's
decisions 2, 3 and 6 — the last such item — **landed on 2026-09-07**, the
day after decision 5 did: a frame's departures are admitted or refused
together, a new removal outranks ghosts already in flight, the
`exit-budget` warning names the frame's removal, and the corpus `exit`
scene pins the change in four bindings with the phase the ADR asked for
(and the correction it needed: `bulk` had to leave in a frame of its own).
The next thing to build was what the next field reports asked for, and
all seven landed the same day they were filed (2026-09-07), each with its
outcome written on top of its entry above: F25 (`setEditText` seeds the
editor the next frame declares, and warns when none does), F26 (a budget
on undeclared editor and scroll state), F27 (every alpha also takes the
`latest` dist-tag — built, and unverified until the tag it writes exists),
F28 (the clause on `quads()` and `access`), F29 (the `audio` element's
`finish`), F30 (`settled` and `frame` on `WindowLoop`) and F31
(`docs/howto.md`, the `**What breaks.**` bullet list from alpha.9 on, and
what a deletion line names). All seven are in the archive as of
2026-09-08; F27 last, once alpha.9's publish ran the `latest` step it
built and the registry answered.

The alpha.9 reports came the same day the tag did, and with a bake-off
against iced and gpui beside them; F32–F35 are what survived the check,
and all four were built on 2026-09-08 in the order this paragraph asked
for — F32 first, then F33's page and guard, then F34 and F35 together as
one `Core::audio_*` door each on the same driver seam. F36 fell out of the
last two and is filed rather than built, for the reason its entry gives.
The alpha.10 reports came the day after that tag, F37 first and then the
four of F38–F41, all built on 2026-09-09 with their outcomes written on top
of their entries in the archive. What is open from that round is nothing:
its three surviving "theirs, not ours" are answered above, one of them by a
`howto.md` answer.

**Editor and mux (2026-09-07, built 2026-09-07/08).** The assessment
section above was the order of work, and it was followed: C16, C18,
C17, C19 for the editor and C21, C23, C22 for the mux all landed with
their outcomes written on top, then C20's element in every binding, the
corpus `ime` scene and C19's wrapped long lines the next day — all eight
are in the archive. W3 was built blind against winit's source; the first
Windows round (2026-09-08) ran it, found it half right, and rebuilt it as
a timer — see the entry. What the round found beside the entries: Lua's
`text()` mutated a shared options table, `npm run gen` described the
previous build, the C parity assert caught a field appended in two
orders, and two things the examples showed once run — the counter's
first click waiting 92 ms for the audio device to open, and a departing
list drawing past its own box — are fixed under alpha.9's `### Fixed`.

**Windows (2026-09-08), and what a second platform is worth.** The platform
ran for the first time and the round became a script, `scripts/smoke-windows.ps1`
(the `smoke` binary since AR4), in `smoke-windows` beside the `cargo test` step, with the C round from
`examples/c/build.ps1` beside it. W3–W12 all shipped in alpha.10 and are in
the archive; W7 was the one that wanted a decision rather than a patch, and
it was settled on 2026-09-09 — a displacement moves a subtree by whole
physical pixels, scrolling included.

The lesson is worth more than the entries. Nine of the ten defects the round
found were invisible to every headless test in this repo, and two were
crashes on the first frame; the tenth kind arrived the same week from the
other direction, on **Linux**, where CI caught a C extension that could not
load into a Node host at all (glibc's `dlopen` defaults to `RTLD_LOCAL`, so
the addon offered none of its `kui_*` to a plugin — fixed in `native.cjs`
before the tag, and under alpha.10's `### Fixed`). Both are the same shape:
a loader or a driver behaving differently per platform, under an API whose
tests all pass on the machine the work was done on. Three platforms build
here; only one of them is developed on.

What the round still does not cover is P8's two follow-ups, unchanged:
`windows_nc.rs` has no test that runs anywhere, and neither does
`access_bridge.rs`. The next Windows session's list, in order: run the round
under `Chrome::Custom` (the subclass is uncovered by everything above), and
run it on the integrated adapter to see what a second GPU changes. The Linux
equivalent is smaller and worth naming: the C round (then
`examples/c/build.sh`, now `cbuild --run`) runs in `check`, and the plugin half of it now has a Node host in the same
job — that is what caught this one.

**Built.** [`docs/adr/0025-the-image-is-the-canvas.md`](adr/0025-the-image-is-the-canvas.md) (proposed, accepted and built 2026-09-11) answers the canvas question: `update_image` and texture-backed images past the atlas (closing the silent drop of an image over 4096 px), `sampling`/`fit` rows, `scale` on the `layout` payload, and a `polygon` filled by a stock fragment — in four bindings with two corpus scenes and two example drives, its four measurements run and its amendment recording what the building changed (a per-node lock that made a fill cost more than six strokes, and a Lua door the draft got wrong). What it named and did not build is V2–V8 above; V1 was built the round after. **Design, wanting an ADR.** A **painter** — the iced-shaped hatch that ADR 0015 (above) names and does not build: a Rust trait or a C extension's function pointers over the shared `Gpu` and the frame's encoder, under the `PainterId` that has been reserved in `resources.rs` since the first commit, placed by a marker quad the renderer splits around as it splits around a fragment. The first thing in a frame that would not be data, so it waits for a view a fragment cannot serve: a 3D viewport, a simulation, a backdrop a copy cannot make. Otherwise nothing new since ADR 0014 was built on 2026-09-07 and amended on 2026-09-08; of what it left open, a slot for Node (W11), extensions in `kui_run` (W9) and an extension offering slots of its own (W12) are built, and what is left — replies from `view`, name-plus-kind — waits for a view. Two instances of one extension are answered: the host namespaces them. Effects an app defines (F23) is
[`docs/adr/0013-effects-as-data.md`](adr/0013-effects-as-data.md),
proposed and then accepted and built on 2026-09-07 — reviewed for alpha.8
rather than left for a view, and its status block says what outweighed
*do nothing*. The exit animations' `animating()` policy is
**done (2026-09-07)**, as
[`docs/adr/0012-the-exit-budget.md`](adr/0012-the-exit-budget.md) —
accepted and built whole, decision 5 first and the rest a day later. It
moved the question rather than answering it: `animating()` is measured free
(a full store owes the driver 12 frames at 0.09% of a budget each) and does
not change, and the boundary is what was wrong — a 1000-row list gets its
first 512 rows sliding out and the rest blinking away. It also found the
two things this entry used to say that the code disagreed with: the
`exit-budget` warning has existed since C8, and `mindmap.tsx` removes
nothing. Rounded hit-testing, which ADR 0010 declined to settle, is
[ADR 0026](adr/0026-hit-testing-by-shape.md) (built 2026-09-11): a region
carries its shape — rounded corners, a stroke's pieces, a fill's outline —
and one `contains` evaluates it after the rect; two storage variants were
measured and the inline one won on simplicity at no cost. Two things ADR 0008 left
open and did not think worth a row yet: `aria-atomic`, and asking AccessKit
upstream for a real announcement in `TreeUpdate` — two of the three platforms
have the API behind it, and AccessKit's whole event surface is a tree diff.

**Rows, when a view asks.** A configurable focus-ring colour (README names
it). `required` / `invalid` and heading `level` (ADR 0001 follow-ups).
Per-button `on_click` and middle-button routing (C2 left them "reach the
core and route nowhere"; the bake-off noticed the same). A `clipboard`
feature beside `audio` and `accesskit`, so a build that wants no `arboard`
can say so — the bake-off's idle figure counted it, and nobody has asked. Physical key positions beyond what `60ca137`
carried. One did ask, on 2026-09-07: an i3-style tab bar wanted `grow`
tabs floored at their labels, and `minWidth: "fit"` / `minHeight: "fit"`
landed the same day for alpha.9 (the CHANGELOG entry says why it is not
the default — a fit width is the unwrapped one).

**Distribution.** The bake-off's one recommendation is to publish to
crates.io, "even as an alpha", because "distribution is the one [risk]
that time makes worse". A decision, not an entry, and it has a fact
attached (checked 2026-09-08): the name `kui` is held there by another
crate ("WebGL charts", one 0.1.0 from 2023-04-04, no repository), while
`kui-core`, `kui-ffi`, `kui-lua`, `kui-node` and `kui-wgpu` are free.
Publishing means asking the owner for the name or shipping the runner
under another — crates.io does not reassign a name without its owner —
and the npm side has no such problem. Nothing in the tree blocks it: the
`publish = ["forgejo"]` lines are the only registry-specific thing.

**Parked on their own terms.** C12 (column wrapping), C13 (`space-between`
and baseline), C14 (aspect ratio), C5(b) (core-side virtualisation), rounded
clip nesting. Each says "wait for a view that wants it", and each should keep
saying it until one does.

**Hygiene.** Archiving is done six times over: the forty-six of
2026-09-06, the ten field-report entries that followed them before the tag —
so all of F1–F15 sit together — W2 whole on 2026-09-07, once its driver
half was built, the fourteen of alpha.9's round on 2026-09-08, and the
fourteen of this one (W3, W4–W12, F32–F35) before the alpha.10 tag, and
the six of the round of 2026-09-11 (V1, D1, D2, T2, C26 whole, E3) the day
they were built, and T1, E1 and E2 — closed in their rounds and left here
— before the alpha.11 tag. This file is now three parked entries, C27
with its measurements, F36, B1, V2–V8, this section, and the
entries built since the tag with their outcomes on top (F42–F54, C30, C31,
C29, W13), waiting for alpha.12 to move them.
Still open, both waiting on something outside the repo: enable `SMOKE_MACOS`
/ `SMOKE_WINDOWS` the day a runner exists (P8) — which has two jobs waiting
for it now, F13's launch probe beside the AX audit, sharing the one
Accessibility permission — and remove Lua `env.focus` at 0.2 (P3, R7).

**For the by-hand round**, which is R4's list in the archive and now also
`### Native verification` under each release heading in the CHANGELOG. Two
items are in it because the frame they live in is between a press and a
release, which no headless assertion reads:

- `npm run slide` in `examples/node`, then **drag the empty canvas and
  watch it while the button is down** — the map has to follow the cursor,
  cards and connectors together, not jump into place on release. That is
  ~~F15~~, checked for alpha.7 with `screencapture` inside a synthetic drag
  (`kui-macos-window-quirks` has the recipe); by hand, a slow drag is enough.
- **Windows: grab the title bar while something animates** (`toasts`,
  mid-spring) and watch whether it keeps moving — W3, filed 2026-09-07
  from a report and not reproduced here.
- The same gesture into a popup: press in the owner, drag into the popup,
  release on an item. ADR 0009's consequences name four `CGEvent` checks and
  W2's archived entry records what each one showed when the driver half was
  built (2026-09-07). Windows adds a fifth nothing has run — mixed DPI across
  two monitors, which is what the arithmetic is in physical pixels for.

---

## Closed — index

A hundred and ten entries, all in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md) and all verbatim.
This index is here so an id resolves without opening that file: the open items
above cite A1, C7, C9, C10, D2, P3, P5, P8, R3, R6 and S2, "After alpha.11" and
the hygiene note cite C2, C5(b), P3, R4 and R7, and code comments, ADRs and
commit messages cite ids of their own. All hundred are whole in the
archive. **C11**, **W2** and **C15** were each split for a while — an entry
appearing there in full and here trimmed to what was still open — until their
remainders landed: C11's last step on 2026-09-06, ADR 0009's driver half on
2026-09-07, and C15's profile the same day. Each move took the whole entry to
the archive, and none renumbered it.

The archive also holds four sections that are records rather than work: the
suggested sequence as it stood on 2026-09-05, "After alpha.6" as it stood on
2026-09-06, and the three release sections —
[Release 0.1.0-alpha.6](backlog/closed-2026-09.md#release-010-alpha6-2026-09-05),
whose R1–R7 are the half-baked items finished before that tag, and
[Release 0.1.0-alpha.7](backlog/closed-2026-09.md#release-010-alpha7-2026-09-06),
which is what the pre-tag round ran and what it could not answer, and
[Release 0.1.0-alpha.8](backlog/closed-2026-09.md#release-010-alpha8-2026-09-07),
the round that watched ADR 0012 at its boundary, and
[Release 0.1.0-alpha.9](backlog/closed-2026-09.md#release-010-alpha9-2026-09-08),
the editor-and-mux round.

**From two field reports (2026-09-06)** — F1–F15, all fifteen. The first
five are in the order they closed; the ten below them followed in the second
move.

- `!` **F2** — [Drag deltas lie twice: `end` zeroes them, and sub-slop motion never reaches them](backlog/closed-2026-09.md#-f2--drag-deltas-lie-twice-end-zeroes-them-and-sub-slop-motion-never-reaches-them--done-2026-09-06) — done (2026-09-06) — F14's `DragMsg` bullet closed with it
- `~` **F5** — [Nothing outside Rust can name a node by the key it declared](backlog/closed-2026-09.md#-f5--nothing-outside-rust-can-name-a-node-by-the-key-it-declared--done-2026-09-06) — done (2026-09-06)
- `~` **F6** — [Headless key input is two channels, and a window drives both](backlog/closed-2026-09.md#-f6--headless-key-input-is-two-channels-and-a-window-drives-both--done-2026-09-06) — done (2026-09-06) — one table in the core, `press` in every binding
- `~` **F7** — [An app with global shortcuts cannot also have a Tab ring](backlog/closed-2026-09.md#-f7--an-app-with-global-shortcuts-cannot-also-have-a-tab-ring-wants-an-adr--done-2026-09-06) — done (2026-09-06) — ADR 0011, accepted and built
- `.` **F14** — [Four doc gaps the two reports paid for](backlog/closed-2026-09.md#-f14--four-doc-gaps-the-two-reports-paid-for-three-of-them-open--done-2026-09-06) — done (2026-09-06) — its `DragMsg` bullet closed with F2
- `!` **F1** — [The Node loop never sets the frame clock until the first `advance`](backlog/closed-2026-09.md#-f1--the-node-loop-never-sets-the-frame-clock-until-the-first-advance--done-2026-09-06) — done (2026-09-06) — one place stamps the clock, so nothing eased was testable from Node before it
- `!` **F3** — [`onKey` fires on release too, and an alpha.4 keymap runs every binding twice](backlog/closed-2026-09.md#-f3--onkey-fires-on-release-too-and-an-alpha4-keymap-runs-every-binding-twice--done-2026-09-06) — done (2026-09-06) — closed as its (a), the `keyUp` flag; a "what breaks" line carries it
- `!` **F4** — [A modal's focus restore beats the closing frame's own `keyFocus` edge](backlog/closed-2026-09.md#-f4--a-modals-focus-restore-beats-the-closing-frames-own-keyfocus-edge--done-2026-09-06) — done (2026-09-06) — the rule is a sentence in ADR 0003's decision 4
- `!` **F15** — [Panning does not work in the window](backlog/closed-2026-09.md#-f15--panning-does-not-work-in-the-window--done-2026-09-06) — done (2026-09-06) — an animation bug, not a window one: a tween retargeted every frame never advanced
- `~` **F8** — [Sliders announce as percentages: no `valueText`](backlog/closed-2026-09.md#-f8--sliders-announce-as-percentages-no-valuetext--done-2026-09-06) — done (2026-09-06) — measured on the platform first; the reading lands in `AccessNode.value` and the ABI did not move
- `.` **F9** — [`AccessMsg` is the one core message without a type parameter](backlog/closed-2026-09.md#-f9--accessmsg-is-the-one-core-message-without-a-type-parameter--done-2026-09-06) — done (2026-09-06) — landed with F10, which shares its slider fixture
- `.` **F10** — [A slider's declared range is never checked against its value](backlog/closed-2026-09.md#-f10--a-sliders-declared-range-is-never-checked-against-its-value--done-2026-09-06) — done (2026-09-06) — landed with F9
- `~` **F11** — [`init` and `view` cannot reach the surface](backlog/closed-2026-09.md#-f11--init-and-view-cannot-reach-the-surface--done-2026-09-06) — done (2026-09-06) — landed with F5
- `~` **F12** — [There is no line](backlog/closed-2026-09.md#-f12--there-is-no-line--done-2026-09-06) — done (2026-09-06) — ADR 0010, accepted and built
- `~` **F13** — [VoiceOver says "node is not responding" at every windowed Node launch](backlog/closed-2026-09.md#-f13--voiceover-says-node-is-not-responding-at-every-windowed-node-launch--diagnosed-not-nodes-and-not-the-trees-2026-09-06) — diagnosed, not Node's and not the tree's (2026-09-06) — the only F entry closed without a fix; it leaves `scripts/ax-launch-probe.swift` behind

**From two alpha.7 field reports (2026-09-07)** — F16–F23, all closed the day they were filed

- `!` **F16** — [`ctx.setTime` under `createApp` is overwritten by a clock that never moves](backlog/closed-2026-09.md#-f16--ctxsettime-under-createapp-is-overwritten-by-a-clock-that-never-moves--done-2026-09-07) — done (2026-09-07) — it throws now, the mirror of `advance()` on a wall clock
- `!` **F17** — [The published `Quad` type stops at kind 4 and has no `ends`](backlog/closed-2026-09.md#-f17--the-published-quad-type-stops-at-kind-4-and-has-no-ends--done-2026-09-07) — done (2026-09-07)
- `~` **F18** — [The npm package ships no changelog and none of the ADRs its types cite](backlog/closed-2026-09.md#-f18--the-npm-package-ships-no-changelog-and-none-of-the-adrs-its-types-cite--done-2026-09-07) — done (2026-09-07)
- `~` **F19** — [A real window has no `quads()`](backlog/closed-2026-09.md#-f19--a-real-window-has-no-quads--done-2026-09-07) — done (2026-09-07) — `capture()` waits for a smoke test that needs pixels
- `~` **F20** — [An `<edit>` seeded with `initial` opens with the caret at 0, and nothing declares otherwise](backlog/closed-2026-09.md#-f20--an-edit-seeded-with-initial-opens-with-the-caret-at-0-and-nothing-declares-otherwise--done-2026-09-07) — done (2026-09-07) — single-line fields open at their end, documents at their top
- `.` **F21** — [A curve's quad count is a constant nobody published](backlog/closed-2026-09.md#-f21--a-curves-quad-count-is-a-constant-nobody-published--done-2026-09-07) — done (2026-09-07)
- `.` **F22** — [Both apps wrote the same "advance until nothing animates" loop](backlog/closed-2026-09.md#-f22--both-apps-wrote-the-same-advance-until-nothing-animates-loop--done-2026-09-07) — done (2026-09-07) — `app.runOut()`
- `~` **F23** — [Effects an app defines have nowhere to go but a side channel](backlog/closed-2026-09.md#-f23--effects-an-app-defines-have-nowhere-to-go-but-a-side-channel-wants-an-adr--done-2026-09-07) — done (2026-09-07) — ADR 0013, accepted and built; the chime itself needed only `<audio>`

**From updating `kui-node-template` to alpha.8 (2026-09-07)** — F24, closed the day it was filed

- `~` **F24** — [The stock `<button>` cannot take the `description` its own changelog entry is about](backlog/closed-2026-09.md#-f24--the-stock-button-cannot-take-the-description-its-own-changelog-entry-is-about--done-2026-09-07) — done (2026-09-07) — the button admits the access rows in all four bindings, and a row it does not read warns instead of vanishing

**From building C11 step 4 (2026-09-06)** — W1

- `!` **W1** — [A `Chrome::Borderless` window is dead to the mouse on macOS](backlog/closed-2026-09.md#-w1--a-chromeborderless-window-is-dead-to-the-mouse-on-macos--done-2026-09-06) — done (2026-09-06)
- `~` **W2** — [Press-drag-release does not reach a popup, and only the driver can make it](backlog/closed-2026-09.md#-w2--press-drag-release-does-not-reach-a-popup-and-only-the-driver-can-make-it--done-2026-09-07) — done (2026-09-07) — ADR 0009, accepted and built; a press that dismisses a popup is consumed now, and the CHANGELOG says so

**Binding parity** — P1–P9

- `!` **P1** — [Add a `description` prop row](backlog/closed-2026-09.md#-p1--add-a-description-prop-row--done-2026-09-06) — done (2026-09-06) — the C tooltip in 2026-09-03, the `PROPS` row itself three days later
- `!` **P2** — [Fix the Lua float `self_at` docs mismatch](backlog/closed-2026-09.md#-p2--fix-the-lua-float-self_at-docs-mismatch--done-2026-09-03) — done (2026-09-03)
- `~` **P3** — [Give Lua imperative focus and `is_pressed`](backlog/closed-2026-09.md#-p3--give-lua-imperative-focus-and-is_pressed--done-2026-09-04) — done (2026-09-04)
- `.` **P4** — [Warn on unknown props](backlog/closed-2026-09.md#-p4--warn-on-unknown-props--done-2026-09-04) — done (2026-09-04)
- `.` **P5** — [Generate `index.d.ts` instead of hand-writing it](backlog/closed-2026-09.md#-p5--generate-indexdts-instead-of-hand-writing-it--done-2026-09-04) — done (2026-09-04)
- `.` **P6** — [Guard `include/kui.h` against ABI drift](backlog/closed-2026-09.md#-p6--guard-includekuih-against-abi-drift--done-2026-09-03) — done (2026-09-03)
- `!` **P7** — [Build a cross-binding conformance corpus](backlog/closed-2026-09.md#-p7--build-a-cross-binding-conformance-corpus--done-2026-09-03) — done (2026-09-03)
- `.` **P8** — [Add macOS and Windows smoke jobs](backlog/closed-2026-09.md#-p8--add-macos-and-windows-smoke-jobs--partly-done-2026-09-03) — partly done (2026-09-03) — `SMOKE_MACOS` / `SMOKE_WINDOWS` are still gated on a runner that does not exist
- `~` **P9** — [Let the corpus drive a frame under custom chrome](backlog/closed-2026-09.md#-p9--let-the-corpus-drive-a-frame-under-custom-chrome--done-and-independently-verified-2026-09-04) — done and independently verified (2026-09-04)

**Core capability** — C1–C11

- `~` **C1** — [Modal scope](backlog/closed-2026-09.md#-c1--modal-scope--done-2026-09-03) — done (2026-09-03)
- `~` **C2** — [Secondary mouse button and `onContextMenu`](backlog/closed-2026-09.md#-c2--secondary-mouse-button-and-oncontextmenu--done-2026-09-04) — done (2026-09-04)
- `~` **C3** — [Derived cursor shape](backlog/closed-2026-09.md#-c3--derived-cursor-shape--done-2026-09-04) — done (2026-09-04)
- `~` **C4** — [Export `reveal()` and `set_scroll()`](backlog/closed-2026-09.md#-c4--export-reveal-and-set_scroll--done-2026-09-04) — done (2026-09-04)
- `~` **C5** — [Make long lists affordable](backlog/closed-2026-09.md#-c5--make-long-lists-affordable--a-done-2026-09-04) — (a) done (2026-09-04) — (b), core-side virtualisation, is parked; the entry says on what
- `~` **C6** — [`selected` and `expanded` on the access tree](backlog/closed-2026-09.md#-c6--selected-and-expanded-on-the-access-tree--done-2026-09-04) — done (2026-09-04)
- `.` **C7** — [Decide the multi-window story on paper](backlog/closed-2026-09.md#-c7--decide-the-multi-window-story-on-paper--done-2026-09-04) — done (2026-09-04)
- `.` **C8** — [Extend the paint vocabulary](backlog/closed-2026-09.md#-c8--extend-the-paint-vocabulary--done-2026-09-04) — done (2026-09-04)
- `.` **C9** — [`KeyUp` and key repeat](backlog/closed-2026-09.md#-c9--keyup-and-key-repeat--done-2026-09-04) — done (2026-09-04)
- `.` **C10** — [Flex wrapping](backlog/closed-2026-09.md#-c10--flex-wrapping--done-2026-09-04) — done (2026-09-04)
- `~` **C15** — [`NodeSpec` was 728 bytes and the frame got ~2.5× more expensive](backlog/closed-2026-09.md#-c15--nodespec-was-728-bytes-and-the-frame-got-25-more-expensive--fixed-2026-09-05-closed-2026-09-07) — fixed (2026-09-05), closed (2026-09-07) — the bisect, the two profiles, the padding experiments, the boxing fix and the pass gates; what is still above the 2026-08-31 baseline is measured and named
- `.` **C11** — [Build multi-window](backlog/closed-2026-09.md#-c11--build-multi-window--done-2026-09-06) — done (2026-09-06), all five steps

**Repetition** — D1–D4

- `.` **D1** — [Collapse `Ctx` and `KuiWindow` with a macro](backlog/closed-2026-09.md#-d1--collapse-ctx-and-kuiwindow-with-a-macro--done-2026-09-03) — done (2026-09-03)
- `.` **D2** — [Route Node's JSON lowering through the encoder](backlog/closed-2026-09.md#-d2--route-nodes-json-lowering-through-the-encoder--done-2026-09-04-the-other-way) — done (2026-09-04), the other way
- `.` **D3** — [Lift composite parsing into `kui-core`](backlog/closed-2026-09.md#-d3--lift-composite-parsing-into-kui-core--done-2026-09-04) — done (2026-09-04)
- `.` **D4** — [Unify `createApp` and `runWindowed`](backlog/closed-2026-09.md#-d4--unify-createapp-and-runwindowed--done-2026-09-04) — done (2026-09-04)

**Documentation** — X1–X3

- `!` **X1** — [Fix the `width`/`height` main-axis docs](backlog/closed-2026-09.md#-x1--fix-the-widthheight-main-axis-docs--done-2026-09-04) — done (2026-09-04)
- `.` **X2** — [Automate the CHANGELOG heading](backlog/closed-2026-09.md#-x2--automate-the-changelog-heading--done-2026-09-04) — done (2026-09-04)
- `.` **X3** — [List the missing input modes in Status / next](backlog/closed-2026-09.md#-x3--list-the-missing-input-modes-in-status--next--done-2026-09-04) — done (2026-09-04)

**From the ADR review (2026-09-04)** — A1–A6

- `~` **A1** — [ADR 0003's two named modal gaps](backlog/closed-2026-09.md#-a1--adr-0003s-two-named-modal-gaps--done-2026-09-04) — done (2026-09-04)
- `~` **A2** — [A dialog cannot choose which control opens focused](backlog/closed-2026-09.md#-a2--a-dialog-cannot-choose-which-control-opens-focused--done-2026-09-04) — done (2026-09-04)
- `.` **A3** — [Three ADRs have deferred arrow-key composites](backlog/closed-2026-09.md#-a3--three-adrs-have-deferred-arrow-key-composites--done-2026-09-05) — done (2026-09-05)
- `!` **A4** — [The C ABI has no version negotiation, and 0004 will need one](backlog/closed-2026-09.md#-a4--the-c-abi-has-no-version-negotiation-and-0004-will-need-one--done-2026-09-04) — done (2026-09-04)
- `.` **A5** — [Two cases ADR 0004 leaves undefined](backlog/closed-2026-09.md#-a5--two-cases-adr-0004-leaves-undefined--done-2026-09-04) — done (2026-09-04)
- `~` **A6** — [The corpus can skip, and its coverage is self-declared](backlog/closed-2026-09.md#-a6--the-corpus-can-skip-and-its-coverage-is-self-declared--done-2026-09-04) — done (2026-09-04)

**From the third review (2026-09-04)** — B1–B4

- `!` **B1** — [`schema::ROLES` is the last unpinned enum restatement](backlog/closed-2026-09.md#-b1--schemaroles-is-the-last-unpinned-enum-restatement--done-2026-09-04) — done (2026-09-04)
- `~` **B2** — [Node is the only binding with no `Env`](backlog/closed-2026-09.md#-b2--node-is-the-only-binding-with-no-env--done-2026-09-04) — done (2026-09-04)
- `~` **B3** — [Exit animations changed the frame model and have no scene](backlog/closed-2026-09.md#-b3--exit-animations-changed-the-frame-model-and-have-no-scene--done-2026-09-04) — done (2026-09-04)
- `.` **B4** — [Backlog headings lag the work, three rounds running](backlog/closed-2026-09.md#-b4--backlog-headings-lag-the-work-three-rounds-running--done-2026-09-05) — done (2026-09-05)

**From the fresh sweep of the grown surface (2026-09-05)** — S1–S8

- `!` **S1** — [A C plugin that omits `kui_ext_abi` loads unchecked](backlog/closed-2026-09.md#-s1--a-c-plugin-that-omits-kui_ext_abi-loads-unchecked--done-2026-09-05) — done (2026-09-05)
- `.` **S2** — [The warning codes drifted across three hand-written copies](backlog/closed-2026-09.md#-s2--the-warning-codes-drifted-across-three-hand-written-copies--done-2026-09-05) — done (2026-09-05)
- `.` **S3** — [The scene corpus ships in every release binary](backlog/closed-2026-09.md#-s3--the-scene-corpus-ships-in-every-release-binary--done-2026-09-05) — done (2026-09-05)
- `.` **S4** — [`runtime.rs` and `kui-ffi/lib.rs` are the god files now](backlog/closed-2026-09.md#-s4--runtimers-and-kui-ffilibrs-are-the-god-files-now--done-2026-09-05) — done (2026-09-05)
- `!` **S5** — [Resource handles are session-blind](backlog/closed-2026-09.md#-s5--resource-handles-are-session-blind--done-2026-09-05) — done (2026-09-05)
- `~` **S6** — [Node cannot read the derived cursor shape](backlog/closed-2026-09.md#-s6--node-cannot-read-the-derived-cursor-shape--done-2026-09-05) — done (2026-09-05)
- `.` **S7** — [`env` reaches three bindings through three restatements and nothing pins them](backlog/closed-2026-09.md#-s7--env-reaches-three-bindings-through-three-restatements-and-nothing-pins-them--done-2026-09-05) — done (2026-09-05)
- `.` **S8** — [ABI bumps per merge, and the header knows it](backlog/closed-2026-09.md#-s8--abi-bumps-per-merge-and-the-header-knows-it--done-2026-09-06) — done (2026-09-06)

**From two alpha.8 field reports (2026-09-07)** — F25–F31; six closed the day they were filed, and F27 on 2026-09-08, when alpha.9's publish ran the step it built

- `~` **F25** — [`setEditText` before the editor exists does nothing, and alpha.8's deletion list told an app to delete a latch it still needs](backlog/closed-2026-09.md#-f25--setedittext-before-the-editor-exists-does-nothing-and-alpha8s-deletion-list-told-an-app-to-delete-a-latch-it-still-needs--done-2026-09-07) — done (2026-09-07) — `setEditText` seeds the editor the next frame declares, and warns when none does
- `~` **F26** — [Editor and scroll state are kept by key forever](backlog/closed-2026-09.md#-f26--editor-and-scroll-state-are-kept-by-key-forever--done-2026-09-07) — done (2026-09-07) — a ceiling on undeclared editor and scroll state, longest-undeclared evicted first
- `~` **F27** — [There is no supported way to learn a release exists, and every range an app writes floats](backlog/closed-2026-09.md#-f27--there-is-no-supported-way-to-learn-a-release-exists-and-every-range-an-app-writes-floats--done-2026-09-08) — done (2026-09-08) — every alpha also takes `latest`; verified on the registry the day alpha.9 published
- `.` **F28** — [`quads()` on a window does not say how to drive one](backlog/closed-2026-09.md#-f28--quads-on-a-window-does-not-say-how-to-drive-one--done-2026-09-07) — done (2026-09-07) — `quads()` says how to drive the window it reads
- `~` **F29** — [An `<audio>` one-shot that must finish has to guess its own length](backlog/closed-2026-09.md#-f29--an-audio-one-shot-that-must-finish-has-to-guess-its-own-length--done-2026-09-07) — done (2026-09-07) — the `audio` element's `finish`, so a one-shot need not guess its length
- `~` **F30** — [A window has no settled frame by name, so a smoke test sleeps](backlog/closed-2026-09.md#-f30--a-window-has-no-settled-frame-by-name-so-a-smoke-test-sleeps--done-2026-09-07) — done (2026-09-07) — `settled` and `frame` on `WindowLoop`
- `.` **F31** — [Three doc shapes the reports paid for](backlog/closed-2026-09.md#-f31--three-doc-shapes-the-reports-paid-for--done-2026-09-07) — done (2026-09-07) — `docs/howto.md`, the `**What breaks.**` list, and what a deletion line names

**From the first Windows round (2026-09-08)** — W3 and W4–W12, all ten built between 2026-09-08 and 2026-09-09 and shipped in alpha.10; nine of them were invisible to every headless test in this repo

- `~` **W3** — [On Windows, animations stop while the window is grabbed](backlog/closed-2026-09.md#-w3--on-windows-animations-stop-while-the-window-is-grabbed--verified-rebuilt-and-measured-on-the-platform-2026-09-08) — verified, rebuilt and measured on the platform (2026-09-08) — the alpha.9 build was blind and half right; the Windows round rebuilt it as a `SetTimer`, which a modal loop dispatches
- `!` **W4** — [Every `fragment` past the first crashed the app on DX12](backlog/closed-2026-09.md#-w4--every-fragment-past-the-first-crashed-the-app-on-dx12--done-2026-09-08) — done (2026-09-08) — the alignment came from the adapter and the device asked for its own
- `!` **W5** — [A resize handed the surface a size no device can hold](backlog/closed-2026-09.md#-w5--a-resize-handed-the-surface-a-size-no-device-can-hold--done-2026-09-08) — done (2026-09-08)
- `!` **W6** — [No C extension could ever load on Windows](backlog/closed-2026-09.md#-w6--no-c-extension-could-ever-load-on-windows--done-2026-09-08) — done (2026-09-08) — ADR 0014's C half had been dead on Windows for as long as it existed
- `.` **W7** — [Text inside a moving box steps a whole pixel while the box does not](backlog/closed-2026-09.md#-w7--text-inside-a-moving-box-steps-a-whole-pixel-while-the-box-does-not--done-2026-09-09) — done (2026-09-09) — the one that wanted a decision: a displacement moves a subtree by whole physical pixels, scrolling included
- `~` **W8** — [The C examples had never been built on Windows, and the plugin half could not be](backlog/closed-2026-09.md#-w8--the-c-examples-had-never-been-built-on-windows-and-the-plugin-half-could-not-be--done-2026-09-08) — done (2026-09-08) — `build.ps1`, and a `/DEF:` naming every `kui_*` so link.exe writes the import library
- `~` **W9** — [A precompiled plugin could not be handed to anyone, and C could not host one](backlog/closed-2026-09.md#-w9--a-precompiled-plugin-could-not-be-handed-to-anyone-and-c-could-not-host-one--done-2026-09-08) — done (2026-09-08) — one plugin binary, two hosts, either language
- `.` **W10** — [Half the C library is two functions nobody headless needs](backlog/closed-2026-09.md#-w10--half-the-c-library-is-two-functions-nobody-headless-needs--done-2026-09-08) — done (2026-09-08)
- `~` **W11** — [Node could not host an extension, and had no way to place one](backlog/closed-2026-09.md#-w11--node-could-not-host-an-extension-and-had-no-way-to-place-one--done-2026-09-08) — done (2026-09-08) — and it is what found the Linux loader bug in `native.cjs`
- `~` **W12** — [A Lua view could not put a native panel inside it](backlog/closed-2026-09.md#-w12--a-lua-view-could-not-put-a-native-panel-inside-it--done-2026-09-08) — done (2026-09-08)

**From two alpha.9 field reports and a bake-off (2026-09-08)** — F32–F35, all four built the day they were filed; F36 stays open above, filed rather than built

- `~` **F32** — [`setEditText` reaches the editor the next frame declares, but only by a key the app cannot have yet](backlog/closed-2026-09.md#-f32--setedittext-reaches-the-editor-the-next-frame-declares-but-only-by-a-key-the-app-cannot-have-yet--done-2026-09-08) — done (2026-09-08) — the label the view declares, in four bindings; it corrects alpha.9's F25 entry
- `.` **F33** — [`howto.md` contradicts the release that shipped it, and nothing checks a "today" sentence](backlog/closed-2026-09.md#-f33--howtomd-contradicts-the-release-that-shipped-it-and-nothing-checks-a-today-sentence--done-2026-09-08) — done (2026-09-08) — and the guard that keeps the page honest is a workspace test
- `.` **F34** — [A one-shot cut off without `finish` is silent (pomodoro wish 3)](backlog/closed-2026-09.md#-f34--a-one-shot-cut-off-without-finish-is-silent-pomodoro-wish-3--built-2026-09-08) — built (2026-09-08)
- `.` **F35** — [A released playback holds a voice, and a refused play is a stderr line the view never hears (pomodoro wish 4)](backlog/closed-2026-09.md#-f35--a-released-playback-holds-a-voice-and-a-refused-play-is-a-stderr-line-the-view-never-hears-pomodoro-wish-4--done-2026-09-08) — done (2026-09-08) — the refusal is an event and a warning; F36 is the asymmetry it left

**From the two virtual-list examples (2026-09-09)** — C25 and C26, filed and built the same day; C26's last two steps stay open above

- `.` **C25** — [A virtual list in JSX needs a sentinel, a hand-written key and a guard; make it one call](backlog/closed-2026-09.md#-c25--a-virtual-list-in-jsx-needs-a-sentinel-a-hand-written-key-and-a-guard-make-it-one-call--done-2026-09-09) — done (2026-09-09) — `virtualColumn` / `virtual_column`, an `index` row in all four bindings, and every query answering for a name no frame declared

**From an alpha.10 field report (2026-09-09)** — F37, closed the day it was filed, and the first entry to land after the alpha.10 tag

- `~` **F37** — [`<button accent>` is in the changelog, the docs and every binding, and `tsc` rejects it](backlog/closed-2026-09.md#-f37--button-accent-is-in-the-changelog-the-docs-and-every-binding-and-tsc-rejects-it--done-2026-09-09) — done (2026-09-09) — `ButtonProps` is generated from `BUTTON_ROWS_JSX` now, so F24's fix cannot come undone a third time

**From the design-system audit (2026-09-10)** — T1, the defect the round turned up, closed 2026-09-11 with the walk at emission; T2 is above under the round of 2026-09-11, T3 closed the day it was filed and was never an entry; T4 filed and built 2026-09-12

- `x` **T4** — [Tokens an app declares beside the theme, for the app whose palette is the design](backlog/closed-2026-09.md#x-t4--tokens-an-app-declares-beside-the-theme-for-the-app-whose-palette-is-the-design--done-2026-09-12) — done (2026-09-12) — ADR 0027: colour and length tokens per origin, `$name` in every colour and length slot, resolved by the binding
- `x` **T1** — [`on_context_menu` does not bubble, and keys do](backlog/closed-2026-09.md#x-t1--on_context_menu-does-not-bubble-and-keys-do--done-2026-09-11) — done (2026-09-11) — `enclosing_menu` beside `enclosing_sink`, walked at emission because the hit stack is paint order and not ancestry; three pins, the `controls` scene, `onForceClick` left topmost-only

**From the examples round (2026-09-10)** — E1 and E2, closed 2026-09-10 and 2026-09-11; E3 is above under the round of 2026-09-11

- `x` **E1** — [The Node dock has no warnings in its stream](backlog/closed-2026-09.md#x-e1--the-node-dock-has-no-warnings-in-its-stream--done-2026-09-10) — done (2026-09-10) — `KuiWindow.warningsRaised()` beside `nodes()` / `setInspect()`
- `x` **E2** — [No C dock](backlog/closed-2026-09.md#x-e2--no-c-dock--done-2026-09-11) — done (2026-09-11) — the dock is the core's (ADR 0024): `kui_set_devtools` or `KUI_DEVTOOLS=1`

**From the paint-order round (2026-09-10)** — C28, filed and closed the day ADR 0023 was written and built

- `!` **C28** — [Floats are under scrollbars and the ring, and stack in tree order](backlog/closed-2026-09.md#-c28--floats-are-under-scrollbars-and-the-ring-and-stack-in-tree-order--done-2026-09-10) — done (2026-09-10) — ADR 0023 built whole: layers stack in the order they opened, chrome ends its layer, one `target_at` for the press and the cursor

**From the round of 2026-09-11** — six entries from five rounds, built together the day after the canvas question and archived under their own rounds

- `.` **V1** — [Fragment image input](backlog/closed-2026-09.md#-v1--fragment-image-input--done-2026-09-11) — done (2026-09-11) — `image` on `fragment`, `kui_sample` / `kui_sample_nearest`, the atlas or the image's own texture bound by the core, ABI 15, the removal order pinned both ways
- `.` **D1** — [The tree rows have no keyboard](backlog/closed-2026-09.md#-d1--the-tree-rows-have-no-keyboard--done-2026-09-11) — done (2026-09-11) — the list is the sink with a cursor of its own; Left folds, Right unfolds, built in the panel and not as a fifth composite pair
- `.` **D2** — [The panel's buttons are Unicode blocks, not icons](backlog/closed-2026-09.md#-d2--the-panels-buttons-are-unicode-blocks-not-icons--done-2026-09-11) — done (2026-09-11) — `icons.rs`, a `line` per stroke and a `polygon` per fill
- `.` **T2** — [The metrics are still constants](backlog/closed-2026-09.md#-t2--the-metrics-are-still-constants--done-2026-09-11) — done (2026-09-11) — `Metrics` beside `Theme` in four bindings; a metric never scales by itself, the stock set is the corpus's contract
- `.` **C26** — [Rows of varying height](backlog/closed-2026-09.md#-c26--rows-of-varying-height--steps-0-and-1-built-2026-09-09-2-and-3-built-2026-09-11) — steps 0 and 1 built (2026-09-09), 2 and 3 built (2026-09-11) — `anchor` (CSS's `overflow-anchor`) in the core with a corpus scene, and `layout_of(key)`
- `.` **E3** — [`features/modal` is still inside two other examples](backlog/closed-2026-09.md#-e3--featuresmodal-is-still-inside-two-other-examples--done-2026-09-11) — done (2026-09-11) — the dialog, the nested confirm, Escape by tag, focus restored; with a drive

**From the architecture review (2026-09-11)** — AR1–AR6, four filed with a reason not to build them and two things noticed; all six built the same day, the reasons argued with in each entry

- `.` **AR1** — [Readback shapes have no `to_value`](backlog/closed-2026-09.md#-ar1--readback-shapes-have-no-to_value--done-2026-09-11) — done (2026-09-11) — the key-set pin first, then `to_value` on nine shapes, `Handles::{HEX, INT}` for the one thing a binding still spells, Node's ~330 lines one `readback` pass
- `.` **AR2** — [`abi_parity`'s prototype rows restate the `extern "C" fn`s](backlog/closed-2026-09.md#-ar2--abi_paritys-prototype-rows-restate-the-extern-c-fns--done-2026-09-11) — done (2026-09-11) — build.rs reads the signatures and writes the rows; no proc-macro; ADR 0020 amended
- `.` **AR3** — [The two selection geometries share their arithmetic by hand](backlog/closed-2026-09.md#-ar3--the-two-selection-geometries-share-their-arithmetic-by-hand--done-2026-09-11) — done (2026-09-11) — `clip_to_unit`, `grained_edges`, and three dispatcher forks folded into three select_api doors; no `Selection` merged
- `.` **AR4** — [The smoke round is written three times](backlog/closed-2026-09.md#-ar4--the-smoke-round-is-written-three-times--done-2026-09-11) — done (2026-09-11) — `cargo run -p kui-devtools --bin smoke`, one program on every platform, the three scripts gone
- `.` **AR5** — [`cells_at` neither eases nor resolves hover](backlog/closed-2026-09.md#-ar5--cells_at-neither-eases-nor-resolves-hover--done-2026-09-11) — done (2026-09-11) — the box eases and hovers, the cells stay the app's picture
- `.` **AR6** — [`text.rs` dispatches on `long` at fifteen sites](backlog/closed-2026-09.md#-ar6--textrs-dispatches-on-long-at-fifteen-sites--done-2026-09-11) — done (2026-09-11) — one map of `Entry::{Run, Long}`, the decision made once at `add`

**From the two alpha.10 upgrade reports (2026-09-09)** — F38–F41, all four built the day they were filed. Three of the four were filed by the apps as their own problem

- `!` **F38** — [A field that hugs its text ratchets down to one character](backlog/closed-2026-09.md#-f38--a-field-that-hugs-its-text-ratchets-down-to-one-character--done-2026-09-09) — done (2026-09-09) — the fit width was measured off a buffer still carrying last frame's wrap; the metrics half of the same cache went with it
- `!` **F39** — [Every fact the driver owns reaches the core after the first view](backlog/closed-2026-09.md#-f39--every-fact-the-driver-owns-reaches-the-core-after-the-first-view--done-2026-09-09) — done (2026-09-09) — `sync_env` runs when the pane is created, not only before a frame
- `~` **F40** — [An OS setting changes and a retained-tree host never hears](backlog/closed-2026-09.md#-f40--an-os-setting-changes-and-a-retained-tree-host-never-hears--done-2026-09-09) — done (2026-09-09) — a `system` event, the way a changed viewport is a `resize`
- `!` **F41** — [A single-line editor wraps like a document](backlog/closed-2026-09.md#-f41--a-single-line-editor-wraps-like-a-document--done-2026-09-09) — done (2026-09-09) — `multiline` decides layout too: a field takes one line and scrolls it under the caret

**From the editor-and-mux assessment (2026-09-07)** — C16–C23, all eight built between 2026-09-07 and 2026-09-08; W3 stays above, built blind

- `!` **C16** — [The shaped-text cache has a clock and no budget](backlog/closed-2026-09.md#-c16--the-shaped-text-cache-has-a-clock-and-no-budget--done-2026-09-07) — done (2026-09-07) — a byte budget on the shaped-text cache, LRU past it, never what the last frame drew
- `~` **C17** — [IME reaches the stock editor only](backlog/closed-2026-09.md#-c17--ime-reaches-the-stock-editor-only--done-2026-09-07) — done (2026-09-07) — `InputEvent::Commit`, preedit and commit to the sink an app-owned editor is, the candidate window at its caret; corpus `ime` scene on 2026-09-08
- `~` **C18** — [Nothing maps a point to a byte offset, or an offset to a rect, on text the app owns](backlog/closed-2026-09.md#-c18--nothing-maps-a-point-to-a-byte-offset-or-an-offset-to-a-rect-on-text-the-app-owns--done-2026-09-07) — done (2026-09-07) — `text_hit` and `caret_rect` by the enclosing key, across runs, in every binding
- `~` **C19** — [A long line is shaped whole, and slicing it from outside costs more than not slicing](backlog/closed-2026-09.md#-c19--a-long-line-is-shaped-whole-and-slicing-it-from-outside-costs-more-than-not-slicing--done-2026-09-08) — done (2026-09-08) — a long line shaped in ~1 KB chunks on demand; step 5, a wrapped one, on 2026-09-08
- `.` **C20** — [A cell grid inside the core, if it beats per-cell nodes by an order of magnitude](backlog/closed-2026-09.md#-c20--a-cell-grid-inside-the-core-if-it-beats-per-cell-nodes-by-an-order-of-magnitude--done-2026-09-08) — done (2026-09-08) — `cells` — a terminal screen as one node, ~37× cheaper than nodes, the element in every binding on 2026-09-08
- `~` **C21** — [Nothing outside the main thread can wake `run`, and `pump` never waits](backlog/closed-2026-09.md#-c21--nothing-outside-the-main-thread-can-wake-run-and-pump-never-waits--done-2026-09-07) — done (2026-09-07) — `kui::Waker` and `pump_until`, a thread wakes the parked loop
- `.` **C22** — [Underline, strikethrough, and a background per span](backlog/closed-2026-09.md#-c22--underline-strikethrough-and-a-background-per-span--done-2026-09-07) — done (2026-09-07) — underline, strikethrough and a background per span (ABI 8)
- `~` **C23** — [No way to turn ligatures off, or tabular figures on](backlog/closed-2026-09.md#-c23--no-way-to-turn-ligatures-off-or-tabular-figures-on--done-2026-09-07) — done (2026-09-07) — `features`: ligatures off, tabular figures on
- `.` **C24** — [`NodeSpec`'s stride is not what the layout passes cost](backlog/closed-2026-09.md#-c24--nodespecs-stride-is-not-what-the-layout-passes-cost--measured-and-declined-2026-09-09) — measured and declined (2026-09-09) — a `layout` column made the passes 14% cheaper and `Tree::push` 38% dearer; the table came out flat
