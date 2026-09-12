# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
the six rounds that followed it, and the field reports from two apps built on
alpha.6 through alpha.11 outside this repo (F1–F15 on 2026-09-06,
F16–F23 and F25–F31 on 2026-09-07, F32–F35 on 2026-09-08, F37–F41 on
2026-09-09, F42–F49 on 2026-09-12). Every item names the
evidence that produced it, so a task that turns out to be wrong can be argued with rather
than guessed at.

**This file is the open list.** The hundred and nine closed entries — each with its
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

What is left here: F42–F49 from the two alpha.11 upgrade reports, filed
2026-09-12 (two defects — a foreign `dispatch` losing a `setEditText` seed
to the redraw the call asked for, and `env.viewport` reading the window
against its own schema row — one gap with no spelling, the label editor,
and five wishes with their shapes written, two of them carried unanswered
from alpha.10), T4 the same day (tokens beside the theme — a question
with the ADR as its deliverable), C30, filed 2026-09-12 (always on top — a window level
no binding can ask for, with the shape to build it written), C29 from the
alpha.11 pre-tag round (four unguarded
bench rows reproducibly slower than alpha.10, filed with the numbers and
a bisect to run), V2–V8 from the canvas question of 2026-09-11 (five
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
itself out of) are both closed now, T3 the day it was filed; AR1–AR6 from
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

### `~` C30 — Always on top

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
  in its own code, the same line `KUI_SMOKE_FRAMES` draws.
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

### `!` F42 — A foreign `dispatch` loses the `setEditText` seed to the redraw the call itself asked for

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

### `!` F43 — `env.viewport` is the window, not what the dock leaves, against its own schema row

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

### `~` F44 — A field cannot wrap and a document cannot submit: the label editor has no spelling

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

### `.` F45 — `clock` is read by `runWindowed` and absent from its options type

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

### `.` F46 — `tick.every` is static, so a fast tick pins the idle pump

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

### `.` F47 — An override for `env.system` in a window, at the launcher

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

### `.` F48 — Whether assistive technology is listening, as an `Env` fact

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

### `.` F49 — `<audio finish>`'s cost in the app's units

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
under this heading. What is open under it is T4, filed 2026-09-12 out of
the alpha.11 field reports and a question asked over them — not from the
audit, but the audit's ADR is what it argues with.

### `.` T4 — Tokens an app declares beside the theme, for the app whose palette is the design

**[ADR 0027](adr/0027-tokens-beside-the-theme.md) written 2026-09-12,
proposed.** Written twice that day: the first draft counted the two apps
and declined (the pomodoro's twelve names are a typed constant with no
literal outside it, 43 of its 69 reads are a ternary the model decides,
the mind map's palette is indexed by data); the review pushed back on the
shape, and the second draft proposes it — colour tokens with a light and
a dark half (`same()` for the common case) and **length tokens** beside
`Theme` and `Metrics`, declared whole, resolved once a frame, referenced
by name in any colour or length prop with the names typed at the
declaration (`defineTokens`), the reference a tagged prop id on the wire
resolved by the binding as it lowers, the stock roles reachable by the
same `$` spelling, every binding declaring into a table of its own
origin (Lua included; a C prop carries no reference). The core-side
resolve was prototyped and
measured inside the noise floor (+1.6% at ±4.8% unused, +0.3% used) and
is not the shape taken. Condition unchanged: build waits for an app to
adopt it.

**The question**, raised 2026-09-12 over the alpha.11 reports: the theme
is a *mechanism* — a value derived once a frame from a source (the OS,
the OS plus a colour, a pin), readable in four bindings, painted by the
stock widgets, with a `system` event when the source moves and an
inspector that can name a role — and only the twenty-three roles ride it.
An app that is not a conventional desktop app has a vocabulary of its own
and gets none of the mechanism for it. The LCARS pomodoro is the case in
hand: "its palette is the design", so it pins the base and paints
everything else from constants of its own; the mind map gives all fifteen
text runs a colour by hand. Both are doing what `examples/rust/*` did
before ADR 0019 — three independent `struct Pal`s — one level up.

**What ADR 0019 already declined, and why this is not that.** Its
"considered options" reject *a registry of arbitrary named tokens* on
three grounds: the stock widgets could not read it without agreeing on
names, a typo is a missing colour at runtime, and no binding could be
generated from it. All three are answered by keeping `Theme` exactly as
it is — the closed struct is what the widgets read and the corpus pins —
and adding an **open map beside it**, not inside it:

- the widgets never read a token; they read roles, and an app that pins
  `surface` to LCARS black has already made every stock widget follow it
  (that is what `setTheme`'s role overrides are for, and the pomodoro's
  "the pin becomes a palette" is reachable today);
- a name nothing declared is a warning in the family `unknown-prop`
  already has — `unknown-token`, naming the token and the node — not a
  silent black;
- a map of `name → colour` is one shape in every binding (`Value`
  already carries it across Lua, Node and C), so nothing per token is
  generated, and the *set* is the app's contract, not the schema's.

**The shape to think about.** `setTheme({ appearance, ...roles,
tokens: { peach: '#ffcc99', tomato: '#ff5555' } })`, or the same under
`light`/`dark` keys so a token set can follow the appearance the way the
roles do and be resolved per frame with them; read back as
`theme().tokens.peach` (Lua `theme.tokens.peach`, C
`kui_theme_token(name)`, Rust `ui.theme().token("peach")`); and — the
part that makes it a mechanism rather than a `const PAL` in the app's own
file — a colour prop that names a token instead of a value, `bg="$peach"`
(`color`, `border`, `hoverBg`, the fragment parameters and the rest),
resolved in the core when the node is opened, so the inspector shows
*peach* on the node and a token change repaints without the view running.
On Node's wire a colour is a `u32`; a token reference is a distinct
encoding — the encoder resolves the name to an index the core hands out
when the set is declared — so the common path pays nothing.

**What is not obvious, and is the reason to think rather than build:**

- *Is the prop reference worth its wire shape?* Without it the feature is
  a map the app could hold itself, and for a JS or Lua app it buys only
  the inspector's name and the cross-frame resolve. With it every colour
  row grows a second encoding in four bindings and the corpus. Metrics
  (T2) had the same question and answered it with a closed struct and no
  prop reference; colour may answer differently because colours are where
  the duplication was.
- *Who else reads a host's tokens?* An extension filling a slot (ADR
  0014) follows the host's *roles* already; a token it wants by name is a
  contract the two make, which is fine — but the map is then the seam
  between a host and a guest, and its warning matters more.
- *Does a token follow the appearance?* The pomodoro's does not and the
  mind map's does not; a token that does is a role wearing a new name,
  and the answer may be "add the role" (ADR 0019's `raise` branch is the
  thing a per-appearance token set would duplicate).
- *Does the devtools' facts tab list them?* It should if they exist —
  the panel is where "which peach is this" gets asked.

**Condition:** one view in the repo or the field that wants a token by
name in a prop — the LCARS app declaring its palette once and writing
`bg="$peach"` on forty pills would be it — and the answer to the first
question above written down first. ADR-sized; the ADR is the deliverable,
and it may decline the prop half and keep the map.


## From the ABI-and-bindings audit (2026-09-10)

The round that produced
[ADR 0020](adr/0020-the-surface-the-schema-does-not-cover.md). Nine drifts
were found and all nine closed the same day — the prototypes, the plain
constants and the header's audit are pinned now the way struct layout has
been since P6 — and one thing was declined with a condition, which is what
this entry keeps.

### `.` B1 — The verb surface is documented, not pinned

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

### `.` C29 — Four unguarded rows read 4–12% slower than alpha.10, reproducibly

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

### `.` W13 — `npm run gen` writes a different `docs/props.md` on Windows

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

**Build next.** The alpha.11 field round, filed 2026-09-12: F42 and F43
first, since both are defects that ship — the pump order in `runWindowed`
plus the held seed's redraw, and `env_facts().viewport` reading `dt_area`
with `size()` answering for the dock — then F44 (`wrap` on a single-line
editor), then F45, F46, F47 and F49 in any order, each a small change with
its guard named in the entry, and F48 once the bridge's activation signal
is confirmed to reach the shell on all three platforms. T4's ADR is
written — [ADR 0027](adr/0027-tokens-beside-the-theme.md), proposed
2026-09-12: colour and length tokens beside the theme and the metrics,
referenced by typed name and resolved by the binding — and it is the
next ADR-sized build once an app in the field says it would adopt it. Before them
the open list was C12, C13, C14, F36, B1 and V2–V8 — every one parked on a
condition — and two entries with work in them: C29, a bisect of four
bench rows, filed by the alpha.11 pre-tag round, and C30, always on top,
filed 2026-09-12 as a per-frame fact in `window_title`'s shape with a
door per binding and a readback in `env.window`. C27 is parked with its measurements. What that round settled
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
with its measurements, C29, C30, F36, B1, V2–V8 and this section.
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

A hundred and nine entries, all in
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

**From the design-system audit (2026-09-10)** — T1, the defect the round turned up, closed 2026-09-11 with the walk at emission; T2 is above under the round of 2026-09-11, T3 closed the day it was filed and was never an entry

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
