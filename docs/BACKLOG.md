# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
the six rounds that followed it, and the field reports from two apps built on
alpha.6, alpha.7 and alpha.8 outside this repo (F1–F15 on 2026-09-06,
F16–F23 and F25–F31 on 2026-09-07). Every item names the
evidence that produced it, so a task that turns out to be wrong can be argued with rather
than guessed at.

**This file is the open list.** The hundred closed entries — each with its
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

What is left here: three parked headings — C12, C13 and C14,
each waiting for a view that wants it — the two the design-system audit left
open on 2026-09-10 (T1, a defect that ships today; T2, the axis ADR 0019
scoped itself out of; T3 closed the same day, when every example in the repo
went onto the theme), B1 from the same day's ABI-and-bindings audit (a table
declined with the condition that would build it) —
C26, whose first two steps were built
on 2026-09-09 and whose last two wait for a view (C25 beside it closed the day
it was filed), and F36, which fell out
of building the last two of the four entries the two alpha.9 field reports and
the bake-off produced, and which is filed rather than built on purpose (the
only host driving its own audio device today is the runner). Everything else
that has been filed has shipped: AR1–AR6 from the architecture review of
2026-09-11 — four filed with the reason each was not built beside it and
two small things the building noticed — were all built the same day, in
the archive under their round; F16–F23 from the two alpha.7 field reports
closed the day they were filed (2026-09-07), F25–F31 from the alpha.8 ones by
the day after (F27 last, on 2026-09-08), C16–C23 landed whole for alpha.9, and
W3–W12 and F32–F35 for alpha.10. C15's remainder was the last split entry, and
it closed on 2026-09-07.

Ordered by area, not by priority. What to do next is under "After alpha.10".

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
not cover is in "After alpha.10" below.

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
whole. What stays here is C26.

### `.` C26 — Rows of varying height — **steps 0 and 1 built (2026-09-09); 2 and 3 open**

**Built the day this was filed, and the plan below survived it with two
corrections.** `widgets::RowHeights` and `widgets::virtual_rows` are steps 0
and 1 together — there was no reason to ship known heights without the
estimate, since the same `measure` callback serves both — with the tests the
step list asked for (`crates/kui-core/tests/scroll.rs`: the anchoring one is
`the_row_under_the_pointer_stays_there_while_the_estimate_moves`) and the
bench (`list_10k_rows_variable*`). `examples/rust/virtual_list.rs
--variable` is a wrapped log through it.

**Correction 1: the anchor is an in-frame correction, not a cross-frame
one.** The plan had the widget remembering the row its last slice started at
and correcting on the *next* frame. It does not need to: it takes the
anchor row and the fraction scrolled into it *before* it measures, and puts
that pair back *after*, in the same frame — so there is no stored anchor,
nothing to invalidate when the data changes under it, and the corrected
frame is the first one drawn. `set_scroll` from inside a view landing on the
frame being built is the fact that makes it work, and it held exactly as the
plan claimed.

**Correction 2: sources 2 and 3 are not both needed, and 3 is not needed at
all here.** Because what `measure` returns is the height the row *gets* (the
row's node is fixed to it), the measurement is authoritative and the layout
cannot disagree with it. That removes the reason for step 2's `layout_of`
query in this design — a row whose height is not text says what it is in
`measure` like any other — and leaves step 3 standing on its own merits,
which are not about virtual lists.

**On the perf question this was filed with** (*keep it a separate widget
unless the general one costs nothing where a stride is known*): at one
height it costs what `virtual_column` costs. Holding the built rows equal —
`list_10k_rows_variable_at_one_height`, the variable widget told row by row
that every row is 24px, against `list_10k_rows_virtual`, the same 38 rows
through the stride — it is **16.79 µs against 16.22 µs**, and three runs of
these benches move their own medians by up to 10%, so that gap is not a
reading. It stays separate anyway, for a reason the frame time does not
show: `virtual_rows` needs a `RowHeights` the app owns, one allocation per
row, and a `set_len` whenever the list changes length. The cost is the
state, not the frame, and a caller who can name the stride should carry
none of it.

**Read the frame totals per row or they mislead.** Every virtualised bench
here costs 427–455 ns a row and a naive row costs 393 ns, so the 242×
against `list_10k_rows_naive` (3.93 ms) is entirely "38 rows instead of ten
thousand" and not a cheaper row. It is also why `list_10k_rows_variable`
reads *under* the uniform bench (13.66 µs): its rows average 32px against
24px, so eight fewer fit the window. Only the at-one-height pair holds that
equal, which is what it is for.

**What the lazy prefix sums and the galloping search were worth.** The first
cut rebuilt the sums whole on every change and bisected `0..len`. Both
changes were needed — laziness alone buys nothing, because a bisection
probes the midpoint first and fills everything under it — and the same
benches run against both implementations say so:

| | first cut | shipped |
|---|---|---|
| learns nothing, 10k | 14.41 µs | 13.66 µs |
| learns a row it is looking at, 10k | 21.24 µs | **13.20 µs** |
| the same at 100k | 84.43 µs | **13.31 µs** |
| a row nowhere near the window, 10k | 28.08 µs | 18.83 µs |

Learning a row's height is now free and flat in the length of the list,
where at 100k rows it used to be six times the steady cost. The Fenwick
tree the plan named is unbuilt and now has nothing to buy. The one case
that still pays is a list told about a row nowhere near its window — a list
whose data changed under it — which pays for the distance between that row
and the window, and is the honest cost of a prefix sum.

**What is still open**, and what the original plan said about each:

- **Step 2, a measured-size query** (`Core::layout_of(key)`). Not needed by
  `virtual_rows` any more (correction 2), and worth building only if
  something else wants the rect the core already keeps for an `on_layout`
  key without the event.
- **Step 3, anchoring in the core** (`anchor` on a scroll container, CSS's
  `overflow-anchor`). Untouched, and the argument for it is unchanged and
  not about virtual lists: a chat that prepends history and a log that
  inserts above the viewport want it with no virtualisation at all. If it
  is built, this is the part of C5(b) that was worth building — not the
  core skipping rows.
- **A JS/Lua `virtualRows`.** The bindings got the *uniform* widget in C25.
  The variable one needs the height cache to live in the app's model there,
  which is a different shape from Rust's `&mut RowHeights` and wants a view
  that has asked for it.
- **Variable widths, and heights that depend on scroll position.** Still
  nobody has asked.

The plan as filed follows, unchanged.

---

`virtual_column` takes one number and every row must come out that tall;
C5's close said in so many words that rows of *varying* height are the one
case (a) does not serve and the one that would justify (b). This entry is
the plan for them, written before any of it is built so that the core-side
part can be argued from a view-side attempt rather than guessed. Nothing
here is scheduled; the first view that is a chat history, a log with wrapped
lines or a feed of mixed cards is what starts it.

**What breaks without a stride.** Three things `virtual_column` does with
`i * row_h` have no closed form: the lead spacer (the height of every row
above the range), the search from `offset.y` to the first visible row, and
`set_scroll(i * row_h)` as "scroll to row `i`". All three need a prefix sum
over heights the view does not have for rows it has never built.

**Where a height can come from, in kui.** Three sources, and the design is
which of them a row uses:

1. *Known from the data.* A `kind` per row with a height per kind, a fixed
   thumbnail size. Exact and free; the prefix sum is over the data.
2. *Measured before layout.* `measure_text(content, style, Some(width))`
   is what layout would give the text node, wrap and `max_lines` included,
   shaped through the same cache — so measuring a row and then drawing it
   shapes once (`runtime.rs`, "Measurement"). The width is
   `scroll_geometry(key).rect.w` less the row's own padding, from the frame
   before, and a resize changes it for every row at once. Exact for text
   rows, which is most of the case; the catch is that measuring the *unbuilt*
   rows is the cost virtualisation exists to avoid, so it has to be lazy and
   cached per `(row, width)`, and rows never measured need an estimate.
3. *Reported after layout.* `on_layout` on each built row posts its rect
   when it changes — the only source for a row whose height is not text (a
   `Fit` image, a nested layout). It is an event, it is one frame late, and
   it is the shape C5 called wrong for the container; for rows it is the
   right shape only when 2 cannot answer.

**The design that follows** (the one every browser-side virtualiser
converged on — estimate, measure, prefix sums, anchor — in kui's terms):

- The view keeps `heights: Vec<Option<f32>>` (measured or known) and one
  `estimate` (the running mean of the measured ones, or the caller's guess
  before any). A row's height is `heights[i].unwrap_or(estimate)`. Prefix
  sums over that: a Fenwick tree for O(log n) point updates when 3 is in
  play, a plain cumulative array rebuilt on change when only 1 and 2 are —
  measure before choosing; 10,000 rows is a 40 KB array and a rebuild is a
  microsecond-scale loop.
- Slicing: binary-search the prefix sum for the first row whose bottom is
  past `offset.y`, build until a row's top is past `offset.y + rect.h`, plus
  overscan **in rows**, not pixels. Lead spacer = prefix[first]; tail =
  total − prefix[last]. Rows are opened with `with_indexed(i)` and are
  `Fit`-height (they measure themselves); the stride assumption is gone.
- **Anchoring is the whole difficulty.** The retained offset is a pixel
  count (`ScrollStore`, `resolve` clamps it and nothing else), so when a row
  *above* the range gets measured and differs from its estimate by Δ, the
  lead spacer grows by Δ and everything on screen jumps by Δ. Correction:
  the frame that learns Δ for rows above `first` calls
  `set_scroll(key, offset + Δ)` in the same view. `positions` reads the store
  at `finish_frame`, after the view has run, so a write from inside `view`
  lands on the frame being built — the same fact `reveal`'s doc comment
  states for itself — and the screen never shows the uncorrected frame.
  **Pin that with a test before anything else**; if it turns out to be one
  frame late in any binding, the jitter is the argument for the core doing
  it (below). Two boundary cases the test suite needs: at
  `offset == max_offset` (tailing a log) a shrinking estimate must keep the
  view at the end, and "scroll to row `i`" is
  `set_scroll(prefix[i])` followed by the correction loop converging over
  the next frame or two as the rows around `i` measure — the scrollbar thumb
  drifts a little while it does, as it does in a browser.
- Node/JSX: the same, with C25's `virtualColumn` grown a `rowH: (i) =>
  number | undefined` — the height cache lives in the model because source
  3 arrives through `update` there, and source 2 is `ctx.measureText`.

**Steps, each with its gate, each usable on its own:**

0. **Known heights** — `widgets::virtual_rows(ui, label, spec, rows, |i|
   height, |ui, i| row)`: source 1 only, prefix sums, `visible_range` from
   a binary search. `virtual_column` becomes the `|_| row_h` case of it (or
   stays as the fast path; measure whether the search costs anything at
   10k). Gate: a `scroll.rs` test with heights `20, 40, 60, …` that builds
   only what shows and where `set_scroll(prefix[i])` lands row `i` at the
   top. This alone is the chat history whose rows are `measure_text`-able:
   the view measures the rows it builds and the rows the search touches,
   caches them, and hands `virtual_rows` the cached number.
1. **Estimate and correct** — the height cache, the estimate, and the
   anchoring write. Gate: a test with an estimate wrong by 2× where the row
   at the top of the window stays at the top across the frame that measures
   the rows above it; the tailing case; the jump-to-row convergence within N
   frames. Bench: `list_10k_rows_variable` beside `list_10k_rows_virtual`
   (~19 µs); the budget is a prefix-sum search and a handful of
   `measure_text` hits per frame.
2. **A measured-size query, if 3 turns out to matter** — the core keeps the
   last rect per `on_layout` key already (`layouts`, for the edge trigger).
   `Core::layout_of(key) -> Option<Rect>` reads it back during the next
   build with no event, no tag and no model field — the query shape C5
   preferred over the event, for a row this time. Only if step 1 shows the
   event traffic (one per built row per scroll frame, ~15 at a screenful)
   costing something a profile can see, or the JSX model plumbing being the
   bulk of the widget.
3. **Anchoring in the core, if step 1's correction is late** — `anchor`
   on a scroll container (a row id naming a child key), meaning "keep this
   node's top where it was when content above it changes size": the core
   has the previous frame's rect for any `on_layout` key and this frame's
   layout, so it is a subtraction in `positions` before `resolve` clamps.
   This is CSS `overflow-anchor`, and it is bigger than virtual lists — a
   chat that prepends history and a log that inserts above the viewport
   want it with no virtualisation at all. If it is built, this is the part
   of C5(b) that was worth building: **not the core skipping rows, which
   breaks tree-as-data in every binding (Node encodes the tree whole; a
   layout pass cannot call back into a view), but the core keeping a row
   still.** Corpus scene, since it is a one-frame property once the
   previous frame is given.

**Not this:** the core building rows lazily (above); variable *widths*
(a horizontal list is the same plan turned sideways, and nobody has asked);
heights that depend on the row's own scroll position; a `virtual` flag.

## From the design-system audit (2026-09-10)

The round that produced
[ADR 0019](adr/0019-a-theme-derived-from-appearance-and-accent.md). The
palette itself is built, and so is every example's migration onto it (T3,
closed the day it was filed); these are the two the audit turned up and
did not close.

### `!` T1 — `on_context_menu` does not bubble, and keys do

Found building `examples/rust/theme.rs`. A secondary press asks
`Interaction::hit_at` for the **topmost** hit region and reads
`context_menu` off that one region alone
(`crates/kui-core/src/input.rs:952`); there is no walk to an ancestor. So a
full-window node that declares `on_key` — the shell pattern
`examples/rust/splitmux.rs` uses, and the one the reference page needed —
sits above a root that declared `on_context_menu` and swallows every
secondary press, silently. Moving the row onto the sink is the workaround,
and it is not discoverable: nothing warns, and the app looks like it has no
menu.

This is exactly the shape ADR 0011 settled for keys — "unclaimed keys
bubble to the nearest enclosing sink" — decided for keys alone. A press
that lands on a node with no menu of its own is unclaimed in the same
sense, and a container offering a menu for everything inside it is the
common case, not an exotic one.

**Do:** walk the hit stack from the topmost outward for the first region
carrying a `context_menu` tag, the way `enclosing_sink` walks for keys.
`hits` is already in paint order and already reversed for the topmost
lookup, so it is the same iterator without the `.next()`. One corpus step
(a menu-declaring root under a full-size sink) and one test that a nested
declaration still wins over its ancestor's.

### `.` T2 — The metrics are still constants

`BUTTON_TEXT` 15, `MENU_TEXT` 13, `MENU_WIDTH` 200, `TITLEBAR_H`, every
`pad_xy(14.0, 8.0)` and every `radius(6.0)` in
`crates/kui-core/src/widgets.rs` are hard-coded the way the colours were,
and the same three examples that each grew a `Pal` also each picked their
own row height and gutter. ADR 0019 scoped itself to colour on purpose —
that is where both the duplication and the accessibility failure were —
but the argument for a `Metrics` beside `Theme` is the argument that
document already makes, one axis over.

The scrollbar's are per node now rather than constants — `scrollbarWidth`
and the two colours (2026-09-10, beside ADR 0023's build) — which is the
per-node half of this and not the `Metrics`; the inset (2) and the
minimum thumb (24) stayed constants, since nothing asked.

Two things worth settling before building it: whether a metric scales
(a density setting is the obvious use, and it interacts with `env`'s scale
factor, which is the renderer's and not the palette's), and whether the
stock widgets' geometry is a *contract* the corpus pins — changing
`MENU_WIDTH` today changes the corpus report, which is the cheap kind of
break, but changing it per app changes what a conformance scene means.


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

Two follow-ups, none blocking, and one closed:

### `x` E1 — The Node dock has no warnings in its stream

Closed 2026-09-10: `KuiWindow.warningsRaised()` is in the addon beside
`nodes()` / `setInspect()`, and the Node stream shows warnings.

### `x` E2 — No C dock

Closed 2026-09-11 by [ADR 0024](adr/0024-the-devtools-are-the-cores.md):
the dock is the core's, and a C program has it with `kui_set_devtools`
or `KUI_DEVTOOLS=1` in its environment, the same panel as everyone.

### `.` E3 — `features/modal` is still inside two other examples

The dialog with `initial_focus`, Tab confined and `dismiss` is shown by
`apps/counter`'s menu (the Rosetta copy of it) and by `features/focus`'s
prose. A page of its own would be that menu with a title; declined until
a modal-specific behaviour — a nested confirm, the entry-focus precedence
of ADR 0007 — wants a picture.

## From the devtools round (2026-09-11)

The round that produced [ADR 0024](adr/0024-the-devtools-are-the-cores.md):
the examples' dock moved into `kui-core`, drawn by the core into any
app's frame with one door per binding, and grew what the brief asked for
— the events tab as a virtual list with a payload viewer, a collapsible
and filterable tree, a picker, a fuller inspector, and a window of its
own. Built whole the same day. One follow-up:

### `.` D1 — The tree rows have no keyboard

Browser inspectors walk the tree with the arrows: Up/Down move the
selection, Left folds (or goes to the parent), Right unfolds. The panel's
rows are buttons in a focus region, so Tab reaches them and Enter
selects, and that is all. A key sink per row is the wrong shape; the
right one is a sink on the list with a cursor of its own — the composite
pattern of ADR 0007, with the rows as its items — and it wants that
reading rather than a special case. Declined in the ADR (*What was
declined*), to be built when the composite owner grows a virtual list.

### `.` D2 — The panel's buttons are Unicode blocks, not icons

The placement buttons (▌ ▐ ▄ ❐ ✕), the toggles (◐ ☀ ☾ ● ☰) and the
picker (⊕) are characters from whatever font the platform falls back to,
and they read as a row of blots. The panel is drawn by the core with the
core's own vocabulary, so the icons should be too: a small set of
`line`-stroked glyphs (ADR 0010) or a `fragment` (ADR 0015) per button,
16 px, in the theme's `muted`/`accent`, with the lit one filled — a
dock-position icon that *is* a little window with the pane shaded. Wanted
by the round that added the buttons (2026-09-11); not blocking.

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

## After alpha.10

Grouped by kind, not urgency. Nothing here blocks the tag. It was "After
alpha.9" until 2026-09-09, when the first Windows round it described had
landed whole for alpha.10 and the heading moved with the tag; "After
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

**Build next.** Nothing with a written ADR and no code, and nothing filed
that is not either parked or deliberately unbuilt: after alpha.10 the open
list is C12, C13, C14, F36, T1, T2, B1 and C26's last two steps — T1 first, as
the only defect among them — a measured-size query
nothing needs since `virtual_rows` measures its own rows, and core-side
scroll anchoring, which is CSS's `overflow-anchor` and is wanted by lists
that are not virtual at all. C27 is parked with its measurements.
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

**Design, wanting an ADR.** A **painter** — the iced-shaped hatch that ADR 0015 (above) names and does not build: a Rust trait or a C extension's function pointers over the shared `Gpu` and the frame's encoder, under the `PainterId` that has been reserved in `resources.rs` since the first commit, placed by a marker quad the renderer splits around as it splits around a fragment. The first thing in a frame that would not be data, so it waits for a view a fragment cannot serve: a 3D viewport, a simulation, a backdrop a copy cannot make. Otherwise nothing new since ADR 0014 was built on 2026-09-07 and amended on 2026-09-08; of what it left open, a slot for Node (W11), extensions in `kui_run` (W9) and an extension offering slots of its own (W12) are built, and what is left — replies from `view`, name-plus-kind — waits for a view. Two instances of one extension are answered: the host namespaces them. Effects an app defines (F23) is
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
nothing. Still wanting an ADR: rounded hit-testing, which
ADR 0010 declined to settle and Status / next names: `HitRegion.clip` is a
`Rect`, so a hit near a rounded corner is a hit. Two things ADR 0008 left
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

**Hygiene.** Archiving is done five times over: the forty-six of
2026-09-06, the ten field-report entries that followed them before the tag —
so all of F1–F15 sit together — W2 whole on 2026-09-07, once its driver
half was built, the fourteen of alpha.9's round on 2026-09-08, and the
fourteen of this one (W3, W4–W12, F32–F35) before the alpha.10 tag. This
file is now three parked entries, F36, and this section — the shortest it
has been since it was written.
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

A hundred entries, all in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md) and all verbatim.
This index is here so an id resolves without opening that file: the open items
above cite A1, C7, C9, C10, D2, P3, P5, P8, R3, R6 and S2, "After alpha.10" and
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

**From the paint-order round (2026-09-10)** — C28, filed and closed the day ADR 0023 was written and built

- `!` **C28** — [Floats are under scrollbars and the ring, and stack in tree order](backlog/closed-2026-09.md#-c28--floats-are-under-scrollbars-and-the-ring-and-stack-in-tree-order--done-2026-09-10) — done (2026-09-10) — ADR 0023 built whole: layers stack in the order they opened, chrome ends its layer, one `target_at` for the press and the cursor

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
