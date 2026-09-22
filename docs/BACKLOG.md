# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
the six rounds that followed it, and the field reports from two apps built on
alpha.6 through alpha.12 outside this repo (F1–F15 on 2026-09-06,
F16–F31 on 2026-09-07, F32–F35 on 2026-09-08, F37–F41 on
2026-09-09, F42–F49 and F50–F54 on 2026-09-12, F55–F61 and F62–F66
on 2026-09-15)
and, on 2026-09-15, the first requirements list a consumer wrote
against kui (K1–K4). Every item names the
evidence that produced it, so a task that turns out to be wrong can be argued with rather
than guessed at.

**This file is the open list.** Every closed entry — each with its
outcome written on top of the original finding, and the tables, profiles and
evidence it argued from — are in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md); forty-six moved there
on 2026-09-06, the remaining ten field-report entries followed the same day
before the alpha.7 tag, W2 went whole on 2026-09-07 when ADR 0009's driver
half was built, fourteen more cut alpha.9 on 2026-09-08, the fourteen of
the first Windows round — W3, W4–W12 and F32–F35 — went before the alpha.10 tag, F37
followed it the next day, filed and closed after the tag, AR1–AR6 from
the architecture review of 2026-09-11 went the same day they were filed,
the seventy-four closed between the alpha.11 and alpha.12 tags —
F36, F42–F54, T5, B1a, C29–C39, W13–W15 and AR7–AR50 — went with the
alpha.12 tag on 2026-09-14, the largest move so far, the twelve of
the two rounds of 2026-09-15 — F55–F61 and K1–K4 — with the alpha.13
tag, the eight of the four rounds of the same day — W17, W18,
F62–F66 and C40 — with the alpha.14 tag, and the three of 2026-09-16 — C42 and C43 from the kawoosh
binary-file report, C44 from the pre-tag round that followed — the day
they were filed, before the alpha.15 tag, F67–F72 from the kawoosh
syntax-tab, idle-frame, second-Mac, mid-frame-dock and devtools-strip
reports the same day, after it, F73 — the select in the other three
bindings — the day after, F74 — the app hearing its window go — from
the kawoosh session report of the same day, and F75 — the table — from
the devtools-tables report of the same day, and F76 — Shift under the
layout fallback — from the kawoosh Russian-layout report of 2026-09-21,
the day it was filed. The index
at the bottom of this file names every one of them, so an id cited by an open
item, a code comment or a commit message can be resolved without opening the
archive. Nothing was renumbered in any of those moves, and nothing ever is.

What is left here: three parked headings — C12, C13 and C14, each waiting
for a view that wants it — C27 with its measurements (an idle pumped
window's cost, where every way out was worse than the cost), V2–V8 from
the canvas question of 2026-09-11 (five waiting for a view, two declined
with a condition — V1, the one with an order attached, was built the
round after, on 2026-09-11), W16 from the alpha.12 pre-tag round (the
headless round overwriting `kui_ffi.dll` under the C hosts, filed with
two fixes to choose between), two of the three editor wishes parked at
the end of the third editor-and-mux round (the third, the underline, is
K4 — a view asked, and it is built), W19 from the drop-zone round —
the drag's position on Windows and Linux, which winit does not report,
waiting for a round on either — the six rounds of 2026-09-15 keeping
only their introductions (their twenty entries — F55–F61 from the
alpha.12 reports and K1–K4 from the kawoosh list with the alpha.13 tag;
W17 and W18 from the macOS 27 round, F62–F65 from the alpha.13 reports,
F66 from the kawoosh terminal and C40 from the drop-zone ask with the
alpha.14 tag — were built the day they were filed), C41 and E4 from the
alpha.14 pre-tag round (a bench row the guard does not watch, 10%
slower since the drop-zone commit and bisected to it; the harness
sizing its window for the dock at launch only), the kawoosh binary-file
report of 2026-09-16 keeping only its introduction and measurements (its
two entries, C42 and C43 — `rich_text` shaped whole past the long-line
threshold, and a long line's key hashed a byte at a time every frame —
were built the day they were filed; the "thousands of spans" the report
blamed measured as a factor of 1.5 and not the cause), and the "theirs, not ours" lists the field reports left
behind. Everything else that has been filed has
shipped, and the sections that follow keep only what they filed and
where it went: F16–F23 from the two alpha.7 field reports closed the day
they were filed (2026-09-07), F25–F31 from the alpha.8 ones by the day
after, C16–C23 landed whole for alpha.9, W3–W12 and F32–F35 for alpha.10,
the ten rounds between alpha.10 and alpha.11 — the paint order, the
theme, the examples, the devtools, the canvas, the architecture review,
the two virtual-list examples and the rest — for alpha.11, and the
seventy-four above for alpha.12: two upgrade-report rounds, the
pomodoro's devtools round, the third editor-and-mux round, the second
architecture review (forty-four entries under ten decisions, every one
built on 2026-09-14), the standard-menus round, and the pre-tag round's
own two finds. C26 was the last split entry, and it closed on 2026-09-11.

Ordered by area, not by priority. What to do next is under "After alpha.15".

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

Two entries were filed under this heading after the alpha.11 tag and
built the day they were filed — C30, always on top, and C31, Node's
corpus adapter disagreeing with the reference when a scene runs alone —
and went to
[the archive](backlog/closed-2026-09.md#core-capability-c30-and-c31-2026-09-12)
with the alpha.12 tag. What is left is the three parked entries and C27.

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
not cover is in "After alpha.15" below.

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
before the alpha.10 tag. F36, which fell out of building the last two of
them and is in neither report, was filed rather than built on purpose —
the only host driving its own audio device was the runner — and was
**built 2026-09-14**, once the round it waited behind was closed; it is
[in the archive](backlog/closed-2026-09.md#from-two-alpha9-field-reports-and-a-bake-off-2026-09-08-the-remainder)
under this heading's remainder since the alpha.12 tag. What stays here
is the bake-off's claims and the "theirs, not ours" list.

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
neither, and every claim was checked against the tree before it became
an entry. What the round produced is eight entries, **F42–F49, all built
on 2026-09-12** — two defects (a foreign `dispatch` losing a
`setEditText` seed to the redraw the call itself asked for, and
`env.viewport` reading the window against its own schema row), the label
editor's `wrap`, and five wishes, two of them carried unanswered from
alpha.10 — moved whole to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-the-two-alpha11-upgrade-reports-2026-09-12)
with the alpha.12 tag, the round's full introduction with them. What
did not survive the check is below.

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
own. Built whole the same day. Its two follow-ups — D1a, the tree's
keyboard, and D2a, the panel's icons — were built the same day they were
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

## From the editor-and-mux assessment, third round (2026-09-13)

The question asked of `main` at `580c99e` (alpha.11 + 50), the third
time: is kui the view layer for an emacs-plus-helix-plus-multiplexer app
whose document, IO, LSP and extensions are the app's own? The first
round (2026-08-31) answered no and built the key sink; the second
(2026-09-07) answered yes to the editor and no to the mux, and its
C16–C23 all shipped in alpha.9. This one answered **yes to both**, and
what it filed is at the edges of the *custom* editor: **C32–C39, all
eight built on 2026-09-13** — the mono face resolved upright, a
clipboard for a key sink, a mouse and a blinking caret for a custom
editor, headless drives for the three app-shaped examples, the cell doc
corrected, a clipboard example, and a selection that follows its
scroller and knows Shift. All eight, with the round's method, its bench
table and its door-by-door check, are in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-the-editor-and-mux-assessment-third-round-2026-09-13)
since the alpha.12 tag. What stays here is the three wishes.

### Wishes, not entries

Three things an editor will ask for that have an answer today and a
better one later, each parked until a view asks:

- **An underline of its own colour and style** — a diagnostic's red
  wave under keyword-coloured text, a terminal's SGR 58 undercurl.
  `Span::underline` is a bool in the text's colour (C22). Today: a
  `line` element under the run, whose rect a monospace column gives
  for free and `caret_rect` gives otherwise. **A view asked on
  2026-09-15**: this is K4 below, with its shape.
- **The middle button** (C2: "reaches the core and routes nowhere") —
  a Linux terminal's paste and an editor's close-tab. Today: nothing.
- **Window position**, declared and read (the README's Status names
  it) — an editor restoring its last geometry. Today: the size, not
  the place.

## From the alpha.12 pre-tag round (2026-09-14)

Run on Windows 11 (Ryzen 9 9950X3D, RTX 5080, rustc 1.98.1), the round
the CHANGELOG's Native verification section records. One thing it hit
is filed here rather than fixed — in the round's own tooling, not in a
shipped binary; the long-line cull and the two `bench-check.sh` lines
it found are under alpha.12's `### Fixed`, and the bench row it
flagged is in the record, read three ways and not demonstrated.

### `.` W16 — The headless round overwrites `kui_ffi.dll` with a build the C hosts cannot load

`cbuild` builds `kui-ffi` with its default features — `runner` on, so
the cdylib exports `kui_run` and `kui_run_with` — and links
`counter.exe` and `host.exe` against the import library that build
wrote. The headless round then runs `cargo run -p kui-lua --example
lua_panel -- --headless`, and kui-lua depends on kui-ffi with
`default-features = false` (the workspace manifest says why: a host
that already has a window does not want the runner in its plugin's
library). Cargo builds the cdylib again under that feature set and
writes it to the same `target/debug/kui_ffi.dll`, now with 211 exports
and no `kui_run`. The two C hosts beside it then fail to start with
`STATUS_ENTRYPOINT_NOT_FOUND` (0xC0000139) — exit 127 in a git-bash,
−1073741511 in PowerShell — which reads as a broken build until
`dumpbin /exports` is run. The alpha.11 Windows round did not see it
because its hosts were opened straight after `cbuild`; this one ran
the headless round in between. On the unixes the same overwrite
happens and the same hosts would fail the same way; nothing has run
them in that order there.

**Do:** the smallest fix is in `cbuild`'s doc and the README's release
recipe — open the C hosts before the headless round, or rerun `cbuild`
(a fingerprint check) after it — and that is what the alpha.12 record
does. The real fix is for the two builds not to share a path, and it
is not free: on Windows `lua_panel`'s `panel.dll` imports `kui_ffi.dll`
from beside the host (the alpha.11 record), so the runner-less cdylib
is what that example runs on, and the two builds each have a reason to
write `target/debug/kui_ffi.dll`. Either the workspace's kui-lua
example turns `runner` on for its dev build — one feature line, and
the two builds become one — or the `smoke` binary ends its headless
round by rebuilding `kui-ffi` with default features, the way `cbuild`
would. The first is smaller and costs the example 5 MB it does not
use; decide when the round is next run on a unix host, where the same
overwrite happens and nothing has yet opened the hosts in that order.
Until then the recipe's order is the guard.

*Run in that order on macOS 27 on 2026-09-15 (the round below): the
four hosts opened fine after the headless round.* There the runner-less
build lands in `target/debug/deps/libkui_ffi.dylib` only (13 MB, 211
exports) and the copy `cbuild`'s build uplifted to
`target/debug/libkui_ffi.dylib` (24 MB, with `kui_run`) is a separate
file that the second build does not touch — the hosts load the uplifted
one through `@rpath`. So the overwrite is Windows-only, where the
uplifted DLL is the one both builds write, and the choice between the
two fixes is still Windows' to make.

## From the macOS 27 round (2026-09-15)

The by-hand round run the day after the alpha.13 tag on the first
macOS 27 machine (27.0 / 26A428, arm64, Xcode 26.6 with the 27.0
Command Line Tools): the workspace tests, the four adapters, the C
round, the Node suite, the headless drives, the AX audit (106/106),
and the gestures — the strip dragged and double-clicked, the pin, the
popup's press-drag-release, the native context menu, the declared and
the standard menu bars through the AX API, typing an emoji and an
accented letter, the Edit menu's Paste and Emoji & Symbols, undo and
redo, the audio device's hold and close. One thing was wrong, and it
was a number; the user then found a second, watching the popup. Both
entries — W17, the keep-out measured from the window, and W18, the
popup refusing to become key — were built the same day and are in the
archive since the alpha.14 tag.

## From the two alpha.12 upgrade reports (2026-09-15)

Both apps upgraded to alpha.12 the day it was tagged and reported: the
mind map's `FINDINGS.md` (alpha.11 → alpha.12) and the LCARS pomodoro's
`docs/kui-alpha-12.md` (wishes 1–4). The bare bump was a drop-in for one
and broke the other for the first time in five releases — on a *fix*,
`{ percent }`, that the release listed under Fixed and not under What
breaks, which is F61. Every claim below was checked against this tree
before it became an entry, and the round's shape is the alpha.11 one:
two things the reports could not see from outside. The pomodoro's "the
play that opens the device is not counted for ~35 ms" is a Node door
handing the sound over *after* the frame it asked for (F56), and its
smoke test's third-of-the-time race was that late hand-over widening a
window it already had. And the mind map's `KeyMsg` without `physical`
turned out to be one of four gaps a single pin finds (F55): a second
field, and two event kinds the schema's table never listed.

What the reports corrected in their own earlier claims stands and is
worth writing down once: the "echo frame" that alpha.12's deletion line
for F44 named ("the echo frame's last word had somewhere to go") **was
never presented**. The mind map put a presented-frame counter beside its
sampler and read the same count on every reading that put text past the
field: the core's editor store is re-shaped at the old width between the
OS event and the app's step, and that state is visible to `accessTree()`
and to nothing on screen, because every runner's order is pump, step,
paint. The headroom guarded a state no user saw. Nothing to build; the
line in this file's archive and the changelog's stands as written, with
this paragraph beside it.

Seven entries, **F55–F61, all built or written on 2026-09-15** — moved
whole to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-the-two-alpha12-upgrade-reports-2026-09-15)
with the alpha.13 tag, their outcomes on top. What did not survive the
check is below.

### Theirs, not ours

- **The `starting` flag** (pomodoro). The right fix for a press that
  cannot know the time: time enters through the tick and nowhere else.
  F59's `every` is a bound on the staleness, not the time, and the
  changelog says it does not replace this.
- **The blip and the chime sharing a device that opens on the first**
  (pomodoro). A fact about the first sound after launch — the device
  takes ~90 ms — and the right wait is for the device to answer, as the
  report's test now does. F56 is why the race was as wide as it was.
- **The System Events click on a menu row hanging osascript** (mind
  map). A by-hand data point about System Events on a winit window,
  consistent with the standard-menus round's own notes: open the menu
  by its title and walk it with the keyboard.
- **The wide tier under reduced motion drawing a frame a second** and
  **`idlePumpMs`** (pomodoro, "ours to fix"). Theirs, as they say — and
  F57 makes the frame-a-second cheaper than it was, since the frame no
  longer resets the backoff.

## From the kawoosh requirements list (2026-09-15)

The fourth editor-and-mux round, and the first written by a consumer
rather than by kui about itself:
`~/projects/kawoosh/docs/design/kui-requirements.md`, forty-two
requirements checked door by door against alpha.12 with each line either
a fact with the door named or a gap with the change named, written "to
be pasted into kui's `docs/BACKLOG.md`". Its ids are kept (K1–K4);
nothing is renumbered, ever. The doors it names were spot-checked
against the tree: every one exists under the name given, with two lines
half right — R8.2 names `InputEvent::Text` for a headless paste, and a
paste to a key sink is `InputEvent::Commit` (`Text` is never delivered
to a sink, by design: the raw press already carried it), which kawoosh's
M2 tests should know before they wonder why the paste never arrived;
and R3.8's premise, below. **K1, K2 and K3 built or answered on
2026-09-15, and K4 built the same day** — the shape it filed, as
written; all four moved whole to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#from-the-kawoosh-requirements-list-2026-09-15)
with the alpha.13 tag.

## From the two alpha.13 upgrade reports (2026-09-15)

Both apps upgraded to alpha.13 the day it was tagged and reported: the
mind map's `FINDINGS.md` (alpha.12 → alpha.13) and the LCARS pomodoro's
`docs/kui-alpha-13.md` (wishes 1–3). The bare bump was a drop-in for
both — where alpha.12 had broken one on a fix — and the first release
where the mind map's `preview:check` script, not a hand
comparison, got to say so. All four alpha.12 wishes came back (F56–F59)
and the reports measured them: the pomodoro's smoke test asserts
`live=1` on the frame after the click where alpha.12's had to write a
paragraph about why it could not wait on `live` at all; its child
process is gone, two `runWindowed`s in one process; the mind map opened
three windows in a row where alpha.12 refused the second with
`EventLoop can't be recreated`; and the wide tier under reduced motion,
which alpha.12's notes left under *Ours to fix*, dropped from 2.9% to
1.9% of a core with nothing changed in the app (F57). The mind map's
suite also stopped pressing the chord no keyboard produces (F60): `redo
removes it again` presses `Z` now, and the freed slot presses ⌃Y, which
the alpha.12 report guessed worked and no check had ever pressed.

Every claim below was checked against this tree before it became an
entry. Four are small: three wishes the pomodoro wrote and one note the
mind map recorded and did not file. Nothing here is a defect a user
sees; the one of those this round produced came from a third app and
is the section after this one (F66). All four were **built
2026-09-15**, the day they were filed, each with its outcome on top of
its entry in the archive since the alpha.14 tag; the one that changes
what an existing input reports (F65) is under *What breaks* in the
CHANGELOG.

### Theirs, not ours

- **The first window of a process measures high once** (pomodoro,
  "found after the bump"). A launch, not a regression — the second
  reading is the steady state, and the release's own native round saw
  the same shape. A bench that opens one window and reads once is
  reading the launch.
- **`access` reads `live=0` in the call that asked** (pomodoro). Not a
  defect: the doc says the sound reaches the device before the *frame*
  the play asked for, and the test asserts after the frame, as it
  should.
- **The child was hiding a warm-process number** (pomodoro). Two thirds
  of the pinned window's ~78 ms `settled` time was the child starting
  cold; in one process it is ~30 ms. F58 is why the number is readable
  now.
- **The pass over the controls, `idlePumpMs`, a `frames(n)` helper, the
  bench in the scratchpad** (pomodoro, "ours to fix"). Theirs, as they
  say — though F62 is the bench's missing number.
- **The suite's `press` helper is still `keyDown` + `keyUp` + `settle`**
  (mind map, "still this app's own work"). The driver's `press` has been
  the one to reach for since F6; their line to change.

## From the kawoosh terminal field report (2026-09-15)

The first thing kawoosh showed on alpha.13 that a user could see:
lazygit in its terminal pane, every `│` of the panel frames a dashed
line and the scrollbar thumb a column of separate black dashes. The
report asked whether it was the wrong font or something kui should do,
and measured the font before answering: the answer is kui. One entry,
and the one defect of the day's three rounds — F66, **built
2026-09-15**, in the archive since the alpha.14 tag.

## From a feature ask (2026-09-15): a drop zone

"You declare a drop zone on a box and you get events once it happens."
Checked against the tree before it became an entry: nothing in the
core hears a file dragged in from the OS, the runner drops winit's three
file events on the floor, and no binding has a spelling — and winit
0.30's events carry no position, so even a runner that forwarded them
could say "the window", never "which box". ADR 0031 is the design; the
one entry it produced — C40 — was **built 2026-09-15** and is in the
archive since the alpha.14 tag, and the one it could not verify from
here is filed below.

### `.` W19 — The drag's position on Windows and Linux is the pane's last cursor, so `move` never fires there

**Found.** winit 0.30's `IDropTarget` (`drop_handler.rs`) discards the
`POINTL` every method is handed, and X11's XDnD discards
`XdndPosition`'s coordinates the same way; no `CursorMoved` arrives
during an OLE or XDnD drag. The runner's fallback for both takes
winit's per-file events, batches them per turn and dispatches at the
pane's last reported cursor — which is where the pointer was *before*
the user picked the files up, possibly in another window (ADR 0031,
decision 5). So on those platforms a zone is found only when the
pointer happened to be over it before the drag, and nothing lights or
moves while the files are over the window.

**Do.** Windows: `RevokeDragDrop` winit's target and register the
runner's own `IDropTarget` whose `DragEnter` / `DragOver` / `Drop`
convert the `POINTL` through `ScreenToClient` and dispatch as macOS's
override does, `DragLeave` the cancel, the return `DROPEFFECT` from the
same stamp. Linux: X11's `XdndPosition` carries the root coordinates in
its `data.l[2]`; Wayland's `wl_data_device` `motion` event carries
surface-local ones — both reachable only by owning the protocol
winit owns, so this waits for winit's `DragEnter { position }` redesign
unless a Windows round wants it first. Unverified from here either way
(W16's rule).

## From the alpha.14 pre-tag round (2026-09-15)

The round that tagged alpha.14, run the evening of the day the four
rounds above were filed and built. Every check passed and every gesture
the round drove read back as the entry says (the CHANGELOG's *Native
verification* has the list); one thing it noticed is the harness's, not
kui's, and one is a bench row the guard does not watch, bisected to a
commit before the tag and left for the round after.

### `.` C41 — `frame_1k_curves` is 10% slower since the drop-zone commit, and no guarded row moved

**Found.** The alpha.14 bench guard read every guarded row within its
noise and three unguarded rows past ±5%; re-run alone, two were noise
and `frame_1k_curves` was not — 255 → 280 µs, +10.0% at ±1.4% run to
run. Four probes with `scripts/bench-check.sh <commit> frame_1k_curves`
(HEAD against each base, one row, ~4 min a probe) put it in one commit:
`cc070bd` (F62–F65) and `db2ff82` (the cursor) read 260 µs against
HEAD's 280–284; `5e6711e` (ADR 0031, the drop zone) reads 286 and
`3179ceb` after it 284 — HEAD is that commit's number. Nothing in ADR
0032 or the devtools chord moved it. The row is a thousand `polyline`
floats through eight knots each, flattened to 35 segments a curve every
frame; it declares no interaction, so the `HitRegion` that grew an
`Option<DropOwner>` (a `Value` inside) and the per-region `enclosing_drop`
walk in `emit` should not be on its path — and `frame_10k_segments`
(+3.6% at ±1.3%), `frame_1k_polygons` (+4.0% at ±1.5%) and
`frame_1k_closed_lines` (+3.1%) lean the same way while
`frame_10k_rects` reads −3.4%, so it is the side-list emitters, not
every node. Twenty-five nanoseconds a curve is one call that stopped
inlining or one struct that crossed a cache line, which is the shape
C15's remainder and the architecture review's two inlining
regressions had.

**Do.** Profile `frame_1k_curves` at `5e6711e` against `db2ff82`
(`cargo bench -p kui-core --bench frame -- frame_1k_curves` under
`sample`, or `perf`-style counters on Linux); the suspects in that
commit's per-frame path are `emit.rs`'s region closure (now building a
`DropOwner` under `any_drop && interactive`, which grew the closure and
may have un-inlined the segment emitter around it), `Interaction`'s new
`DropHover` field, and `Tree::any_drop`. If it is inlining, an
`#[inline]` or a split of the region build out of the emit loop; if it
is the region struct, box the `DropOwner`. Add `frame_1k_curves` to the
guard list if the fix lands, so the segment path has a row that fails.

### `.` E4 — The examples harness sizes the window for the dock at launch only

**Found.** `kui_devtools::main!` adds the dock's extent to the example's
size when the dock is on at launch (`DOCK_SIDE_W`, `DOCK_BOTTOM_H`), so
the example's area is what it asked for. A panel brought back at
runtime — `--dock off`, then the devtools chord (or `--key f12` and
F12, which is how this round met it) — takes its 340 px from the
example's area instead: `button` at its own 560 × 352 becomes a 220 px
column, its wrapped paragraphs shrunk under the button rows. Closing
the panel with × and bringing it back keeps a window that was sized
for it, so the default launch never shows this. Not a core defect (the
column shrinks as a column does; `min_height: "fit"` is the row's
answer), and the harness is not shipped.

**Do.** Either resize the window by the dock's extent when the panel
comes back into a window that was not sized for it (a window command the
runner already answers, or a resize the harness asks for), or lay the harness's example area out with `min_width:
"fit"` so it is the panel that gives. The first matches what launch
does; the second is a line. Cheap either way; waits for it to matter.

## From the kawoosh binary-file report (2026-09-16)

kawoosh opened a `.ttf` — a binary, so a "line" is tens of KB between
the rare newlines — and the framerate HUD read 17.9 ms a frame average,
229 ms at the worst, in release. The editor draws each of its ~34
visible rows as one `rich_text` of the whole line, and the report's
first pass had already escaped the control characters (a NUL laid out
at infinite width) and capped the dim-escape spans, blaming "thousands
of spans, looks quadratic" for what was left. Measured here before
filing (a scratch probe over `Core::frame`, release, 1200 × 800 at
2×, one row of a 34-row column under `scroll_x`):

| case | cold | steady |
|---|---|---|
| `text`, 50 KB, one row (chunked, C19) | 0.16 ms | 0.09 ms |
| `rich_text`, 50 KB, **one span** | 331 ms | 0.13 ms |
| `text`, 34 rows × 50 KB | 656 ms | 2.9 ms |
| `rich_text`, 34 rows × 50 KB, one span each | 13.5 s | 9.4 ms, then **56 ms** |
| `rich_text`, 16 KB in 250 / 1000 / 8000 spans | 161 / 198 / 275 ms | 0.04 / 0.07 / 0.18 ms |
| `rich_text`, 2000 spans over 8 / 16 / 32 KB | 111 / 234 / 421 ms | — |
| `text` vs `rich_text` below the threshold, 4000 bytes | 36 vs 35 ms | — |

So: shaping is ~9 µs a byte whole-line, plain or rich alike (four-byte
spans ~1.5× that — a factor, not a power); what makes the rich row a
thousand times the plain one is that it is shaped whole where the plain
one is chunked; and a screenful of them thrashes the cache. The span
count was never the story. Two entries, C42 and C43, both **built
2026-09-16**, the day they were filed, and in the archive; what kawoosh
itself can do is under "theirs".

### Theirs, not ours

- kawoosh draws every row through `rich_text` even when it has one
  span and no decoration, which forgoes C19 until C42 lands: a row
  whose segments fold to one plain look should go through `text`. Its
  caret row can stay plain on a long line by drawing the block caret,
  the selection and the hits as floats measured to their byte the way
  its bar caret already is (`measure_text` of a long prefix takes the
  long path — the prefix's chunks share keys with the line's — and
  answers the estimate, exact under monospace); or it can slice to the
  viewport's columns with a spacer either side, the app-side
  `virtual_column` C19's outcome allows a monospace grid editor. With
  C42 built the `rich_text` rows are chunked as they are, so the first
  is moot and the second is a choice; its own per-row work is O(line)
  a frame either way — the line text cloned, the escapes expanded, a
  `Vec` of every grapheme boundary built to snap the span cuts — which
  the slice bounds and C42 does not touch.
- A syntax-run cap on long lines (vim's `synmaxcol`, VS Code's 20k
  tokenisation cap) is the editor's policy, not the toolkit's.

## From the alpha.15 pre-tag round (2026-09-16)

The round that tagged alpha.15, run the night C42 and C43 were built.
Every check passed; the one gesture the round owed that no example
covers — kawoosh itself, on this tree, opening a 2.6 MB font with its
framerate HUD on — found a panic on the first frame, in code that had
shipped six releases earlier. One entry, **built 2026-09-16** in the
round and in the archive.

## From the kawoosh syntax-tab report (2026-09-16)

kawoosh built a Syntax tab into the panel (ADR 0032's host form: its
tree-sitter tree beside facts, events and tree) and found the one thing
the tab could not do from its side: a `:syntax_tree` command has no way
to *show* the tab — the strip's click and `Ctrl+Shift+N` were the only
two, and both are the user's. One entry, **built 2026-09-16**, the day
it was filed, and in the archive.

## From the kawoosh idle-frame report (2026-09-16)

kawoosh, idle in normal mode with the devtools off, drew a frame twice a
second — the blink clock, armed on the `caret` row its block caret
declared for the IME and the access tree, toggling a phase the block
never read. kui's contract said "the `caret` row is what arms the
clock", and offered no way to declare a caret that anchors and reads
but does not blink. One entry, **built 2026-09-16**, the day it was
filed, and in the archive.

## From the kawoosh second-Mac report (2026-09-16)

kawoosh on a Mac it had not been developed on: `j` held moved one line
and `e` held opened the accent picker. macOS's press-and-hold, on by
default and read from a user default the process consults at every key
press — off on the first machine, which is why nothing had shown it.
kui had no door for it, and a modal editor cannot live with it. One
entry, **built 2026-09-16**, the day it was filed, and in the archive.

## From the kawoosh mid-frame-dock report (2026-09-16)

kawoosh's `:kui_debugger` opened the panel in the bottom-left corner of
the window, 340 wide and half the height, and it jumped to its right
dock on the next frame — once there was one: with the idle loop quiet
since F68 there was not, and it stayed. The panel is turned on from the
app's `view`, after `begin_frame` had already laid the root out without
it. One entry, **built 2026-09-16**, the day it was filed, and in the
archive.

## From the kawoosh devtools-strip report (2026-09-16)

kawoosh with five tabs in a right dock: the strip broke the *labels* —
`Synta / x`, `Event / s` — where ADR 0032 said it wrapped tabs, the
theme, accent and menu toggles sat at the strip's end away from the
facts they change, and the Tree tab left a third of the panel empty
under its inspector. Three things, two entries — F71 the layout, F72
the panel and the widget it wanted — both **built 2026-09-16**, the day
they were filed; F73, the select in the other three bindings, was asked
for and built the next day. All three are in the archive.

## From the kawoosh devtools-tables report (2026-09-17)

Every key/value list in the devtools — the panel's Facts, tokens,
legend and inspector groups, kawoosh's Perf and Settings tabs — lined
its values up behind a label box of a width picked by hand: 70, 90, 52,
110 and 180 px, six numbers, each the longest label that list had on
the day it was written and each wrong the day a longer one arrived
(`syntax rows` past 110 in the Perf tab; a settings path as long as the
user's dotted key). The report asked for a table in kui, in every
binding, and for the hand-rolled lists in both devtools to be it. One
entry, F75, **built 2026-09-17**, the day it was filed, and in the
archive.

## From the kawoosh Russian-layout report (2026-09-21)

kawoosh's roadmap had "`:` in a Russian layout" as its first open
correctness gap, and checking it against the code found the hybrid
rule of ADR 0002 decision 11 built and pinned — and Shift missing from
it: the stand-in for a non-ASCII layout key was the position's
*unshifted* US key, so `J` was `j`, `~` was `` ` `` and Shift on the
key printed `;` was `;`. One entry, F76, **built 2026-09-21**, the day
it was filed, and in the archive.

## From the kawoosh virtual-list report (2026-09-22)

kawoosh's memory and undo panes list their rows through
`virtual_column`, and the report was a five-row window in a pane
twenty rows tall, and a flicker at the list's edge while the wheel
turned. The widget slices by the previous frame's geometry — by
design, one frame of lag covered by two rows of overscan — but nothing
asked for the frame that would close the lag: a container that came
out of layout taller than the slice assumed (a split sliding open, a
resize) or scrolled elsewhere (a reveal, a clamp) kept the stale rows
until the next event. And the undo pane's graph — a stroke and a dot
per row, drawn with `line` and `polygon` inside the rows — spilled
over the strip above the list once it scrolled: a stroke is a float
(ADR 0010 decision 5), and a float escaped every ancestor's clip, the
scroller's included. Two entries, F77 and F78, **built 2026-09-22**,
the day they were filed, and in the archive.

## From the kawoosh scrolling-tab report (2026-09-22)

A strip of columns on a ribbon wider than the window, the viewport
revealing the focused one. The columns carried `slide`, so a column
that had just taken the keyboard was still drawn where it came from
for the 200ms of the glide — off the viewport, outside the scroller's
clip, with no hit region — and the keys typed at it in that window
went nowhere, focus plainly on it. And the ribbon itself could not be
made to glide: `slide` eases a node's place in its parent, a scroll
offset moves the content outside that, so the columns eased while the
ribbon jumped. Two entries, F79 and F80, **built 2026-09-22**, the day
they were filed, and in the archive.

## From the kawoosh Lua-types report (2026-09-22)

kawoosh's plugins and settings are Lua, edited in kawoosh with
lua-language-server attached, and every `row`, `column` and `text` in
a view was an undefined global: nothing described the prelude to the
server, where `index.d.ts` has described the JSX half since alpha.1.
One entry, F81, **built 2026-09-22**, the day it was filed, and in the
archive.

## From the kawoosh tab-strip report (2026-09-23)

kawoosh's tabs became a scrolling row that reveals the active tab on
the frame it changes, and the pane ribbon under it reveals its focused
column on the same frame — a tab switch changes both. The strip's ask
never landed: `reveal` held one pending key, and the ribbon's, asked
later in the build, replaced it. One entry, F82, **built 2026-09-23**,
the day it was filed, and in the archive.

## From the regression pass of 2026-09-19

A review of everything since the alpha.15 tag — F67–F75, nine features
in 71 files — run the way the pre-tag rounds are (the workspace suite,
clippy, the corpus in four adapters, `npm run gen`, the headless and
windowed smoke, all green: 1257 tests over 97 suites, 40 scenes, 171
Node tests, 72 + 22 windows, gen diff 0) and then read for what the
suite cannot see, each claim probed against this tree before it was
filed. Fifteen entries, RG1–RG15. The headline was
RG1: F74's fix reached Rust only, and the changelog's reason why the
other two hosts did not need it was wrong — a Node app quit with ⌘Q
ran nothing after `runWindowed`, not even `process.on('exit')`, so
kawoosh's own defect was still open for every app that is not Rust.
RG1 is **built 2026-09-19** and in the archive, RG2 and RG12 **built
2026-09-20** and there too, RG3, RG6, RG7, RG8 and RG11 — the five
on the table's layout — **built 2026-09-20** the same day, RG4, the
left dock's deferral, **built 2026-09-20** too, RG5, the freeze
loop's sign rule, **built 2026-09-20** as well, and RG9 and RG10, the
select's disabled row through the door and its unchecked options and
`current`, **built 2026-09-20** likewise, RG13, the reader's click
behind a modal, **built 2026-09-20** too, and RG15, F69 checked in a
window, **done 2026-09-20** — the pin does not hold — RG14, the ten
nits and the two small devtools defects, **done 2026-09-20** too, and
RG16, the decision RG15 left, **done 2026-09-20** as well: the door is
removed. Nothing of the pass is left open.
Two were regressions this round introduced (RG5 and RG6, both now
built), the rest are gaps the new features opened or holes they made
reachable.

## After alpha.15

Grouped by kind, not urgency. Nothing here blocks the tag. It was "After
alpha.14" until 2026-09-20, when the ten rounds between the alpha.15 and
alpha.16 tags — the kawoosh reports of 2026-09-16 and 17 (F67–F75, nine
entries) and the regression pass over them (RG1–RG16, sixteen), every
one built the day after it was filed at the latest — had landed and the
heading moved with the tag; "After alpha.13" until 2026-09-16, when the two rounds between the alpha.14 and
alpha.15 tags — the kawoosh binary-file report and the pre-tag round's
own find, three entries, every one built the day it was filed — had
landed and the heading moved with the tag; "After alpha.12" until 2026-09-15, when the four rounds between the alpha.13
and alpha.14 tags — the macOS 27 round, the two alpha.13 upgrade
reports, the kawoosh terminal report and the drop-zone ask, eight
entries, every one built the day it was filed — had all landed and the
heading moved with the tag; "After
alpha.11" until 2026-09-14, when the seven rounds between the two tags —
two upgrade reports, the pomodoro's devtools round, the third
editor-and-mux round, the second architecture review, the standard
menus and the pre-tag round's own finds, seventy-four entries — had all
landed and the heading moved with the tag; "After alpha.10" before that,
until the eleven rounds between alpha.10 and alpha.11 —
the paint order, the theme, the examples, the devtools, the canvas, the
architecture review and the rest — had all landed; "After alpha.9" before that, until the first Windows round
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

**Build next.** Nothing of the regression pass of 2026-09-19 is open (RG1, the Node and C hosts hearing ⌘Q, was **built 2026-09-19**; RG2 and RG12, the devtools' menus select and its chord, RG3, RG6, RG7, RG8 and RG11, the table's layout and the round's float-floor regression, RG4, the left dock's deferral, RG5, the freeze loop's sign rule and the round's other regression, RG9 and RG10, the select's disabled row through the door and its unchecked options and `current`, RG13, the reader's click behind a modal, RG15, F69 checked in a window and found inert, RG14, the ten nits and the two devtools defects taken with them, and RG16, the door removed on RG15's finding, **built 2026-09-20**); next is C41 — a profile of `frame_1k_curves` at the drop-zone
commit against the one before, the bisect already done; then W19, when
a Windows or Linux round comes (the macOS half of ADR 0031 is built and
verified; the fallback elsewhere is honest and positionless). Nothing else filed is open. The rounds since the alpha.14 tag, newest first:
the regression pass of 2026-09-19 over F67–F75 (RG1–RG16 — all
sixteen built or done between 2026-09-19 and 2026-09-20, RG14's ten
nits and RG16's removal of the press-and-hold door **done
2026-09-20** last) is what the paragraph opens with. Before it, alpha.16's own nine: F75 — the table
— from the kawoosh devtools-tables report, **built 2026-09-17**; F74 —
the app hearing its window go — from the kawoosh session report of
the same day, **built 2026-09-17** (Rust only, as RG1 found and
finished); F73 — the select in the other three bindings — **built
2026-09-17**, the day after F72; and F67–F72 — the devtools tab
selected from the app, the solid caret, the press-and-hold door (inert,
as RG15 found), the mid-frame dock, the strip's wrap and the select —
from the five kawoosh reports of 2026-09-16, **built the day they were
filed**, after the alpha.15 tag. Before the tag, the three of the same
day — C42 and C43 from the kawoosh binary-file report, C44 from the
pre-tag round's own find — **built 2026-09-16**. Before them: the two alpha.13 reports and the
kawoosh terminal report — F62–F66, in the archive since the alpha.14
tag — were **built 2026-09-15**, F66 first
(the dashed `│` was what every TUI in a `cells` node showed, and the
entry's shapes, atlas key and tests are what was built), then the four
small ones, each as its entry says: two counters (F62), a sentence and an assertion (F63), two
bits and a wait (F64), a default that folds (F65). Before them,
everything filed had shipped: K4 was
**built on 2026-09-15** too, the same day as the rest of its round. The two rounds of 2026-09-15 — the
alpha.12 upgrade reports (F55–F61) and the kawoosh requirements list
(K1–K4) — were **built the day they were filed**, each with its outcome
on top of its entry above; what the round settled beyond its entries is
in the two introductions: the "echo frame" was never presented, and a
paste to a key sink is `Commit`. Before them: the second architecture review, filed 2026-09-13 above
as AR7–AR49, in the order its decisions argue for: AR7 and AR8 (the
session rule written and the audio store and image drops moved under
it — **both done 2026-09-14**), AR9–AR11 together — **all three done
2026-09-14** (the held-key identity, the chord bit carried
into the second channel, one attach pass with a producer-side mark),
AR12 (Node's window handle — **done 2026-09-14**), AR13–AR16 (**all
four done 2026-09-14**) (the `<text>` rows, one token
miss policy, Cut's `changed`, one `prepare_spec`), AR17–AR18 (the focus
stamp and the AT gates — **both done 2026-09-14**), AR19–AR25 as the defects they are
(**all seven done 2026-09-14**, AR26 with them), then B1a's
table with AR26, AR27 and AR40 beside it (**B1a done 2026-09-14** — the
table, its four pins and the one-line rows; **AR40 done 2026-09-14**;
**AR27 done 2026-09-14**, `KuiRunConfig` under ABI 16 and the
context's core handed over through `Launcher::core`), AR46–AR48 for
the tests (**AR46, AR47 and AR48 done 2026-09-14**: the walk complete
and pinned; the `sampler` scene, the `animate` test, the `nodes()`
readback and two more guarded rows; the fixture face and the helpers
hoisted), the small core defects among the `.` entries (**AR31,
AR34–AR39 done 2026-09-14** — hover through `target_at`, one paste ask
at a time, `scaled` leaves the titlebar, `nodes()` in viewport px,
`finish_frame` crate-private, the inspect latch gone, panes re-found by
id; **AR41–AR45 done 2026-09-14** — the button from the palette, the
enum lists pinned to their enums, `windows` as JSON under v12, the role
interfaces generated and `Env` / `NodeInfo` pinned, one frame counter /
label lookup / caret rect; **AR29 and AR30 done 2026-09-14** — the
caret follows the keys to the enclosing sink, `TextHit.line` is the
visual row across the node's runs, a gutter is not its line's text,
`text-beyond-line`; **AR32 and AR33 done 2026-09-14** — a popup is
borderless and not resizable, the retarget frame is points on macOS;
**AR28 done 2026-09-14** — Shift-motions select in a scope; **AR50
decided and done 2026-09-14** — the first way, an [in] append bumps,
ADR 0006 amended and every [in] size pinned), and
AR49 first of all if the alpha.12 notes go out before the rest, since
its first line is the CHANGELOG contradicting itself on the frame
version (**AR49 done 2026-09-14**, the reused ids retired as `B1a` /
`D1a` / `D2a` with a test). The `~` and `.` entries between wait for
the defects. With the round closed, the unconditioned `.` entry left
was F36 (**done 2026-09-14** — the two audio answers in C and Node);
what remains is conditioned (C12–C14, V2–V8) or decided as it stands
(C27); W15, the one by-hand check, was **done 2026-09-14** — none of
AppKit's four Edit rows worked in a winit window, the runner now
answers what Emoji & Symbols and Dictation need and trims the other
two.
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
the open list was C12, C13, C14, F36, B1a and V2–V8 — every one parked on a
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
the six of the round of 2026-09-11 (V1, D1a, D2a, T2, C26 whole, E3) the day
they were built, and T1, E1 and E2 — closed in their rounds and left here
— before the alpha.11 tag, and the seventy-four closed between that tag
and alpha.12's (F36, F42–F54, T5, B1a, C29–C39, W13–W15, AR7–AR50) with
the alpha.12 tag on 2026-09-14, the four sections that partly stayed —
Core capability, the alpha.9 reports, the alpha.11 reports and the third
editor-and-mux round — each keeping a paragraph that says what went
where. This file is now three parked entries, C27 with its measurements,
V2–V8, W16, W17 and W18 (the macOS 27 round's two finds, built the same day), C40 and W19 (the drop zone of ADR 0031, built the day it was asked for, and the Windows/Linux position it could not verify from here), the editor wishes, the "theirs, not ours" lists, and this
section.
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
- **A submenu (AR21):** an app whose popup declares a popup of its own
  — hover a row in a non-activating menu, let it open a submenu, press a
  row in the submenu: the row hears the press, the menu stays until the
  choice closes it, the arrow keys reach the submenu while it is up, and
  the window behind reads focused throughout. Built 2026-09-14 by
  reading; no example opens a second level yet.
- **Node, two windows:** an app whose `windows(model)` opens a second
  window with an `<edit>` in it — type there, then `win.editText(label)`
  from that window's `view` and `win.setEditText` from the `update` its
  `changed` reaches: both the second window's, neither main's. That is
  AR12, built 2026-09-14 with the loop's aiming pinned on a recording
  surface and the real two-window case unpinned, since a `KuiWindow`
  needs a display.
- The same gesture into a popup: press in the owner, drag into the popup,
  release on an item. ADR 0009's consequences name four `CGEvent` checks and
  W2's archived entry records what each one showed when the driver half was
  built (2026-09-07). Windows adds a fifth nothing has run — mixed DPI across
  two monitors, which is what the arithmetic is in physical pixels for.

---

## Closed — index

Every closed entry, all in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md) and all verbatim — one heading
per id, and `tests/docs.rs` holds every id to one entry across both files.
This index is here so an id resolves without opening that file: the open items
above cite A1, C7, C9, C10, D2, P3, P5, P8, R3, R6 and S2, "After alpha.15" and
the hygiene note cite C2, C5(b), P3, R4 and R7, and code comments, ADRs and
commit messages cite ids of their own. All of them are whole in the
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
- `.` **D1a** — [The tree rows have no keyboard](backlog/closed-2026-09.md#-d1a--the-tree-rows-have-no-keyboard--done-2026-09-11) — done (2026-09-11) — the list is the sink with a cursor of its own; Left folds, Right unfolds, built in the panel and not as a fifth composite pair
- `.` **D2a** — [The panel's buttons are Unicode blocks, not icons](backlog/closed-2026-09.md#-d2a--the-panels-buttons-are-unicode-blocks-not-icons--done-2026-09-11) — done (2026-09-11) — `icons.rs`, a `line` per stroke and a `polygon` per fill
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

**Core capability, after the alpha.11 tag** — C30 and C31, both filed and built on 2026-09-12; the heading stays above with C12–C14 and C27

- `~` **C30** — [Always on top](backlog/closed-2026-09.md#-c30--always-on-top--done-2026-09-12) — done (2026-09-12)
- `!` **C31** — [Node's corpus adapter disagrees with the reference on `fragments`, when it runs alone](backlog/closed-2026-09.md#-c31--nodes-corpus-adapter-disagrees-with-the-reference-on-fragments-when-it-runs-alone--done-2026-09-12) — done (2026-09-12)

**From two alpha.9 field reports and a bake-off (2026-09-08), the remainder** — F36, filed rather than built until a host other than the runner drove its own device; built 2026-09-14

- `.` **F36** — [A host driving its own audio device can report an end and nothing else](backlog/closed-2026-09.md#-f36--a-host-driving-its-own-audio-device-can-report-an-end-and-nothing-else--done-2026-09-14) — done (2026-09-14)

**From the two alpha.11 upgrade reports (2026-09-12)** — F42–F49, all eight built the day they were filed; the "theirs, not ours" list stays above

- `!` **F42** — [A foreign `dispatch` loses the `setEditText` seed to the redraw the call itself asked for](backlog/closed-2026-09.md#-f42--a-foreign-dispatch-loses-the-setedittext-seed-to-the-redraw-the-call-itself-asked-for--done-2026-09-12) — done (2026-09-12)
- `!` **F43** — [`env.viewport` is the window, not what the dock leaves, against its own schema row](backlog/closed-2026-09.md#-f43--envviewport-is-the-window-not-what-the-dock-leaves-against-its-own-schema-row--done-2026-09-12) — done (2026-09-12)
- `~` **F44** — [A field cannot wrap and a document cannot submit: the label editor has no spelling](backlog/closed-2026-09.md#-f44--a-field-cannot-wrap-and-a-document-cannot-submit-the-label-editor-has-no-spelling--done-2026-09-12) — done (2026-09-12)
- `.` **F45** — [`clock` is read by `runWindowed` and absent from its options type](backlog/closed-2026-09.md#-f45--clock-is-read-by-runwindowed-and-absent-from-its-options-type--done-2026-09-12) — done (2026-09-12)
- `.` **F46** — [`tick.every` is static, so a fast tick pins the idle pump](backlog/closed-2026-09.md#-f46--tickevery-is-static-so-a-fast-tick-pins-the-idle-pump--done-2026-09-12) — done (2026-09-12)
- `.` **F47** — [An override for `env.system` in a window, at the launcher](backlog/closed-2026-09.md#-f47--an-override-for-envsystem-in-a-window-at-the-launcher--done-2026-09-12) — done (2026-09-12)
- `.` **F48** — [Whether assistive technology is listening, as an `Env` fact](backlog/closed-2026-09.md#-f48--whether-assistive-technology-is-listening-as-an-env-fact--done-2026-09-12) — done (2026-09-12)
- `.` **F49** — [`<audio finish>`'s cost in the app's units](backlog/closed-2026-09.md#-f49--audio-finishs-cost-in-the-apps-units--done-2026-09-12) — done (2026-09-12)

**From the LCARS pomodoro's devtools round (2026-09-12)** — F50–F54, all five built the same evening: the panel living with what the app chose

- `!` **F50** — [The panel paints the accent as ink, and an accent near the base is invisible](backlog/closed-2026-09.md#-f50--the-panel-paints-the-accent-as-ink-and-an-accent-near-the-base-is-invisible--done-2026-09-12) — done (2026-09-12)
- `!` **F51** — [The base "app" keeps the OS's base, not the app's, under an accent override](backlog/closed-2026-09.md#-f51--the-base-app-keeps-the-oss-base-not-the-apps-under-an-accent-override--done-2026-09-12) — done (2026-09-12)
- `!` **F52** — [The panel's own window inherits the launcher's chrome](backlog/closed-2026-09.md#-f52--the-panels-own-window-inherits-the-launchers-chrome--done-2026-09-12) — done (2026-09-12)
- `!` **F53** — [`pick` from the panel's window leaves the keyboard there](backlog/closed-2026-09.md#-f53--pick-from-the-panels-window-leaves-the-keyboard-there--done-2026-09-12) — done (2026-09-12)
- `!` **F54** — [The window's floor is the app's, and the dock is taken out of it](backlog/closed-2026-09.md#-f54--the-windows-floor-is-the-apps-and-the-dock-is-taken-out-of-it--done-2026-09-12) — done (2026-09-12)

**From the design-system audit (2026-09-10)** — T5, the derived tokens ADR 0027 left out, built 2026-09-13 under ADR 0028; T1, T2 and T4 are listed above

- `.` **T5** — [Derived tokens: a colour computed from another](backlog/closed-2026-09.md#-t5--derived-tokens-a-colour-computed-from-another--done-2026-09-13) — done (2026-09-13)

**From the ABI-and-bindings audit (2026-09-10)** — B1a, declined with a condition on 2026-09-10 and built on 2026-09-14, the day the second architecture review met it

- `.` **B1a** — [The verb surface is documented, not pinned](backlog/closed-2026-09.md#-b1a--the-verb-surface-is-documented-not-pinned--done-2026-09-14) — done (2026-09-14)

**From the alpha.11 pre-tag round (2026-09-11)** — C29 and W13, the bench rows read honestly and the Windows `gen` drift, both done 2026-09-12

- `.` **C29** — [Four unguarded rows read 4–12% slower than alpha.10, reproducibly](backlog/closed-2026-09.md#-c29--four-unguarded-rows-read-412-slower-than-alpha10-reproducibly--done-2026-09-12) — done (2026-09-12)
- `.` **W13** — [`npm run gen` writes a different `docs/props.md` on Windows](backlog/closed-2026-09.md#-w13--npm-run-gen-writes-a-different-docspropsmd-on-windows--done-2026-09-12) — done (2026-09-12)

**From the editor-and-mux assessment, third round (2026-09-13)** — C32–C39, all eight built the same day; the three wishes stay above

- `!` **C32** — [`Mono` is whichever monospaced face has the lowest id on a machine without Noto Sans Mono](backlog/closed-2026-09.md#-c32--mono-is-whichever-monospaced-face-has-the-lowest-id-on-a-machine-without-noto-sans-mono--done-2026-09-13) — done (2026-09-13)
- `~` **C33** — [A key sink has no clipboard](backlog/closed-2026-09.md#-c33--a-key-sink-has-no-clipboard--done-2026-09-13) — done (2026-09-13)
- `~` **C34** — [A custom editor has no mouse: a press carries no point, a click no count, and the reference has none](backlog/closed-2026-09.md#-c34--a-custom-editor-has-no-mouse-a-press-carries-no-point-a-click-no-count-and-the-reference-has-none--done-2026-09-13) — done (2026-09-13)
- `~` **C35** — [A custom caret cannot blink](backlog/closed-2026-09.md#-c35--a-custom-caret-cannot-blink--done-2026-09-13) — done (2026-09-13)
- `.` **C36** — [The three app-shaped examples have no headless drive](backlog/closed-2026-09.md#-c36--the-three-app-shaped-examples-have-no-headless-drive--done-2026-09-13) — done (2026-09-13)
- `.` **C37** — [A cell is a scalar, and its doc says it is a grapheme](backlog/closed-2026-09.md#-c37--a-cell-is-a-scalar-and-its-doc-says-it-is-a-grapheme--done-2026-09-13) — done (2026-09-13)
- `.` **C38** — [The clipboard has no example, in any binding](backlog/closed-2026-09.md#-c38--the-clipboard-has-no-example-in-any-binding--done-2026-09-13) — done (2026-09-13)
- `~` **C39** — [A selection stops at the edge, ignores the wheel under a held press, and knows no Shift](backlog/closed-2026-09.md#-c39--a-selection-stops-at-the-edge-ignores-the-wheel-under-a-held-press-and-knows-no-shift--done-2026-09-13) — done (2026-09-13)

**From the architecture review, second round (2026-09-13)** — AR7–AR50, forty-four entries under ten decisions, every one built on 2026-09-14

- `!` **AR7** — [The audio store is the session's and is reconciled against every window's frame](backlog/closed-2026-09.md#-ar7--the-audio-store-is-the-sessions-and-is-reconciled-against-every-windows-frame--done-2026-09-14) — done (2026-09-14)
- `!` **AR8** — [A removed image reaches the GPU of the window that removed it, and a removed fragment reaches no GPU](backlog/closed-2026-09.md#-ar8--a-removed-image-reaches-the-gpu-of-the-window-that-removed-it-and-a-removed-fragment-reaches-no-gpu--done-2026-09-14) — done (2026-09-14)
- `!` **AR9** — [A held key's release is matched on `code`, and Shift changes `code` mid-hold](backlog/closed-2026-09.md#-ar9--a-held-keys-release-is-matched-on-code-and-shift-changes-code-mid-hold--done-2026-09-14) — done (2026-09-14)
- `!` **AR10** — [The editing channel drops the chord bit, so a chord the sink heard also presses the control; Space ignores every modifier](backlog/closed-2026-09.md#-ar10--the-editing-channel-drops-the-chord-bit-so-a-chord-the-sink-heard-also-presses-the-control-space-ignores-every-modifier--done-2026-09-14) — done (2026-09-14)
- `!` **AR11** — [`attach_lines` reads the producer off `kind` against a table entry spelled `"changed / submit"`; `attach_cells` reads no producer at all](backlog/closed-2026-09.md#-ar11--attach_lines-reads-the-producer-off-kind-against-a-table-entry-spelled-changed--submit-attach_cells-reads-no-producer-at-all--done-2026-09-14) — done (2026-09-14)
- `!` **AR12** — [Every `KuiWindow` door but `setViewBinary` addresses the main window](backlog/closed-2026-09.md#-ar12--every-kuiwindow-door-but-setviewbinary-addresses-the-main-window--done-2026-09-14) — done (2026-09-14)
- `!` **AR13** — [`<text>` admits every container and access row, and all four doors drop them silently](backlog/closed-2026-09.md#-ar13--text-admits-every-container-and-access-row-and-all-four-doors-drop-them-silently--done-2026-09-14) — done (2026-09-14)
- `!` **AR14** — [An unresolved `$token` has three outcomes, and the published text matches none](backlog/closed-2026-09.md#-ar14--an-unresolved-token-has-three-outcomes-and-the-published-text-matches-none--done-2026-09-14) — done (2026-09-14)
- `!` **AR15** — [Context-menu Cut mutates an editor without a `changed`, and the runner fakes one for Cmd-X](backlog/closed-2026-09.md#-ar15--context-menu-cut-mutates-an-editor-without-a-changed-and-the-runner-fakes-one-for-cmd-x--done-2026-09-14) — done (2026-09-14)
- `!` **AR16** — [The line and polygon doors skip the hover resolve, so `hover_bg` on a wedge never paints](backlog/closed-2026-09.md#-ar16--the-line-and-polygon-doors-skip-the-hover-resolve-so-hover_bg-on-a-wedge-never-paints--done-2026-09-14) — done (2026-09-14)
- `!` **AR17** — [`set_focus` on the modal's closing frame loses to the restore; a `keyFocus` edge and a Tab step win](backlog/closed-2026-09.md#-ar17--set_focus-on-the-modals-closing-frame-loses-to-the-restore-a-keyfocus-edge-and-a-tab-step-win--done-2026-09-14) — done (2026-09-14)
- `!` **AR18** — [Assistive-technology requests skip the modal and `disabled` gates every other channel obeys](backlog/closed-2026-09.md#-ar18--assistive-technology-requests-skip-the-modal-and-disabled-gates-every-other-channel-obeys--done-2026-09-14) — done (2026-09-14)
- `!` **AR19** — [The glyph atlas grows for one oversized item and resets for an oversized set](backlog/closed-2026-09.md#-ar19--the-glyph-atlas-grows-for-one-oversized-item-and-resets-for-an-oversized-set--done-2026-09-14) — done (2026-09-14)
- `!` **AR20** — [A device that failed to open drops `Play` without answering `refused`](backlog/closed-2026-09.md#-ar20--a-device-that-failed-to-open-drops-play-without-answering-refused--done-2026-09-14) — done (2026-09-14)
- `!` **AR21** — [Popups are one level deep: a press inside a sub-popup dismisses and consumes itself, and keys never reach it](backlog/closed-2026-09.md#-ar21--popups-are-one-level-deep-a-press-inside-a-sub-popup-dismisses-and-consumes-itself-and-keys-never-reach-it--done-2026-09-14-read-and-not-run) — done (2026-09-14), read and not run
- `!` **AR22** — [A window the OS refused stays live in the registry](backlog/closed-2026-09.md#-ar22--a-window-the-os-refused-stays-live-in-the-registry--done-2026-09-14) — done (2026-09-14)
- `!` **AR23** — [`ModifiersChanged` is mirrored to the key target only, so the owner's modifiers go stale while a popup borrows the keyboard](backlog/closed-2026-09.md#-ar23--modifierschanged-is-mirrored-to-the-key-target-only-so-the-owners-modifiers-go-stale-while-a-popup-borrows-the-keyboard--done-2026-09-14) — done (2026-09-14)
- `!` **AR24** — ["One selection per window" is enforced in one direction](backlog/closed-2026-09.md#-ar24--one-selection-per-window-is-enforced-in-one-direction--done-2026-09-14) — done (2026-09-14)
- `!` **AR25** — [`{ percent: 50 }` in JSX is 5000%](backlog/closed-2026-09.md#-ar25---percent-50--in-jsx-is-5000--done-2026-09-14) — done (2026-09-14)
- `!` **AR26** — [Lua's `env.edit_text` takes an integer only, and its `on_event` drops the window](backlog/closed-2026-09.md#-ar26--luas-envedit_text-takes-an-integer-only-and-its-on_event-drops-the-window--done-2026-09-14) — done (2026-09-14)
- `~` **AR27** — [C's runner takes no window, and `kui_run_with` drops what the ctx registered](backlog/closed-2026-09.md#-ar27--cs-runner-takes-no-window-and-kui_run_with-drops-what-the-ctx-registered--done-2026-09-14) — done (2026-09-14)
- `~` **AR28** — [No keyboard path starts or extends a selection in a `selectable` scope](backlog/closed-2026-09.md#-ar28--no-keyboard-path-starts-or-extends-a-selection-in-a-selectable-scope--done-2026-09-14) — done (2026-09-14)
- `~` **AR29** — [The sink's caret and IME anchor read the focused node's subtree; keys read the enclosing sink](backlog/closed-2026-09.md#-ar29--the-sinks-caret-and-ime-anchor-read-the-focused-nodes-subtree-keys-read-the-enclosing-sink--done-2026-09-14) — done (2026-09-14)
- `.` **AR30** — [Three meanings of `line`, and a `byte` that counts text the access tree excludes](backlog/closed-2026-09.md#-ar30--three-meanings-of-line-and-a-byte-that-counts-text-the-access-tree-excludes--done-2026-09-14) — done (2026-09-14)
- `.` **AR31** — [Hover ignores the scrollbar the press and the cursor honour](backlog/closed-2026-09.md#-ar31--hover-ignores-the-scrollbar-the-press-and-the-cursor-honour--done-2026-09-14) — done (2026-09-14)
- `.` **AR32** — [A popup surface is resizable and, under custom chrome, gets the app's non-client treatment](backlog/closed-2026-09.md#-ar32--a-popup-surface-is-resizable-and-under-custom-chrome-gets-the-apps-non-client-treatment--done-2026-09-14) — done (2026-09-14)
- `.` **AR33** — [Retarget's common frame is physical pixels, which AppKit does not have](backlog/closed-2026-09.md#-ar33--retargets-common-frame-is-physical-pixels-which-appkit-does-not-have--done-2026-09-14) — done (2026-09-14)
- `.` **AR34** — [`request_paste` has no outstanding-request guard, so both examples carry one](backlog/closed-2026-09.md#-ar34--request_paste-has-no-outstanding-request-guard-so-both-examples-carry-one--done-2026-09-14) — done (2026-09-14)
- `.` **AR35** — [`Metrics::scaled` scales `titlebar_h`, the row the schema marks the platform's and `compact()` exempts](backlog/closed-2026-09.md#-ar35--metricsscaled-scales-titlebar_h-the-row-the-schema-marks-the-platforms-and-compact-exempts--done-2026-09-14) — done (2026-09-14)
- `.` **AR36** — [`nodes()` rects are window px; every other readback is dock-shifted viewport px](backlog/closed-2026-09.md#-ar36--nodes-rects-are-window-px-every-other-readback-is-dock-shifted-viewport-px--done-2026-09-14) — done (2026-09-14)
- `.` **AR37** — [`Core::finish_frame` is a public second door that skips the fills, the menu and the devtools](backlog/closed-2026-09.md#-ar37--corefinish_frame-is-a-public-second-door-that-skips-the-fills-the-menu-and-the-devtools--done-2026-09-14) — done (2026-09-14)
- `.` **AR38** — [`devtools.inspecting` is a one-way latch](backlog/closed-2026-09.md#-ar38--devtoolsinspecting-is-a-one-way-latch--done-2026-09-14) — done (2026-09-14)
- `.` **AR39** — [The runner indexes a pane by position after a command may have removed it](backlog/closed-2026-09.md#-ar39--the-runner-indexes-a-pane-by-position-after-a-command-may-have-removed-it--done-2026-09-14) — done (2026-09-14)
- `.` **AR40** — [Three small door rules broken: `index` on a button, `cells` cursor names, a lone `width`](backlog/closed-2026-09.md#-ar40--three-small-door-rules-broken-index-on-a-button-cells-cursor-names-a-lone-width--done-2026-09-14) — done (2026-09-14)
- `.` **AR41** — [`button_spec` hard-codes the accent trio `Theme::dark()` also hard-codes](backlog/closed-2026-09.md#-ar41--button_spec-hard-codes-the-accent-trio-themedark-also-hard-codes--done-2026-09-14) — done (2026-09-14)
- `.` **AR42** — [`Easing`, `Repeat`, `Live` and `FontFamily` are index→variant matches with no pin](backlog/closed-2026-09.md#-ar42--easing-repeat-live-and-fontfamily-are-indexvariant-matches-with-no-pin--done-2026-09-14) — done (2026-09-14)
- `.` **AR43** — [Node's `windows` root prop is hand-lowered on both wire sides while `WindowConfig::from_value` exists](backlog/closed-2026-09.md#-ar43--nodes-windows-root-prop-is-hand-lowered-on-both-wire-sides-while-windowconfigfrom_value-exists--done-2026-09-14) — done (2026-09-14)
- `.` **AR44** — [`index.d.ts`'s `Env`, `NodeInfo`, `Theme` and `Metrics` are hand-written mirrors of generated tables](backlog/closed-2026-09.md#-ar44--indexdtss-env-nodeinfo-theme-and-metrics-are-hand-written-mirrors-of-generated-tables--done-2026-09-14) — done (2026-09-14)
- `.` **AR45** — [Three frame counters, two label resolvers, one caret rect twice](backlog/closed-2026-09.md#-ar45--three-frame-counters-two-label-resolvers-one-caret-rect-twice--done-2026-09-14) — done (2026-09-14)
- `.` **AR46** — [`surface.c` says every prototype is called once; twenty-one are called by nothing](backlog/closed-2026-09.md#-ar46--surfacec-says-every-prototype-is-called-once-twenty-one-are-called-by-nothing--done-2026-09-14) — done (2026-09-14)
- `.` **AR47** — [Coverage is pinned for elements; 31 generic rows, `animate` among them, are in no scene and no test](backlog/closed-2026-09.md#-ar47--coverage-is-pinned-for-elements-31-generic-rows-animate-among-them-are-in-no-scene-and-no-test--done-2026-09-14) — done (2026-09-14)
- `.` **AR48** — [Test helpers re-derived beside `kui_core::testing`, and font tests that pass with no font](backlog/closed-2026-09.md#-ar48--test-helpers-re-derived-beside-kui_coretesting-and-font-tests-that-pass-with-no-font--done-2026-09-14) — done (2026-09-14)
- `!` **AR49** — [The documents disagree with the code and with each other in eleven places](backlog/closed-2026-09.md#-ar49--the-documents-disagree-with-the-code-and-with-each-other-in-eleven-places--done-2026-09-14) — done (2026-09-14)
- `~` **AR50** — [The header says an [in] append is safe for a host that did not recompile; the library reads the whole struct](backlog/closed-2026-09.md#-ar50--the-header-says-an-in-append-is-safe-for-a-host-that-did-not-recompile-the-library-reads-the-whole-struct--done-2026-09-14) — done (2026-09-14)

**From the standard-menus round (2026-09-14)** — W14 and W15, filed and built the same day

- `.` **W14** — [The standard Edit menu's rows never grey](backlog/closed-2026-09.md#-w14--the-standard-edit-menus-rows-never-grey--done-2026-09-14) — done (2026-09-14)
- `~` **W15** — [AppKit's own Edit rows arrive unchecked](backlog/closed-2026-09.md#-w15--appkits-own-edit-rows-arrive-unchecked--done-2026-09-14) — done (2026-09-14)

**From the two alpha.12 upgrade reports (2026-09-15)** — F55–F61, all built or written the same day

- `.` **F55** — [`KeyMsg` has no `physical`; the message types are a step behind the payloads](backlog/closed-2026-09.md#-f55--keymsg-has-no-physical-the-message-types-are-a-step-behind-the-payloads--done-2026-09-15) — done (2026-09-15)
- `!` **F56** — [`KuiWindow.access()` hands the click's sound to the device a frame late, and `env.audio.live` reads 0 on the frame that asked](backlog/closed-2026-09.md#-f56--kuiwindowaccess-hands-the-clicks-sound-to-the-device-a-frame-late-and-envaudiolive-reads-0-on-the-frame-that-asked--done-2026-09-15) — done (2026-09-15)
- `.` **F57** — [A frame the loop's own tick draws resets the idle backoff](backlog/closed-2026-09.md#-f57--a-frame-the-loops-own-tick-draws-resets-the-idle-backoff--done-2026-09-15) — done (2026-09-15)
- `~` **F58** — [A second `runWindowed` in one process, after the first window closed](backlog/closed-2026-09.md#-f58--a-second-runwindowed-in-one-process-after-the-first-window-closed--done-2026-09-15) — done (2026-09-15)
- `.` **F59** — [The cadence on the tick](backlog/closed-2026-09.md#-f59--the-cadence-on-the-tick--done-2026-09-15) — done (2026-09-15)
- `.` **F60** — [A shifted letter's spelling is nowhere written, and a headless press can spell one no keyboard produces](backlog/closed-2026-09.md#-f60--a-shifted-letters-spelling-is-nowhere-written-and-a-headless-press-can-spell-one-no-keyboard-produces--done-2026-09-15) — done (2026-09-15)
- `.` **F61** — [`{ percent }` was a break the changelog filed under Fixed](backlog/closed-2026-09.md#-f61---percent--was-a-break-the-changelog-filed-under-fixed--done-2026-09-15) — done (2026-09-15)

**From the kawoosh requirements list (2026-09-15)** — K1–K4, the first consumer-written round, all built the same day

- `~` **K1** — [A slot name not known when the extension loaded](backlog/closed-2026-09.md#-k1--a-slot-name-not-known-when-the-extension-loaded--done-2026-09-15) — done (2026-09-15)
- `.` **K2** — [An event knows which fill it came from](backlog/closed-2026-09.md#-k2--an-event-knows-which-fill-it-came-from--done-2026-09-15) — done (2026-09-15)
- `.` **K3** — [Trailing and repeated spaces measure reliably in a run](backlog/closed-2026-09.md#-k3--trailing-and-repeated-spaces-measure-reliably-in-a-run--answered-2026-09-15) — answered (2026-09-15)
- `.` **K4** — [An underline of its own colour and style](backlog/closed-2026-09.md#-k4--an-underline-of-its-own-colour-and-style--done-2026-09-15) — done (2026-09-15)

**From the macOS 27 round (2026-09-15)** — W17 and W18, both built the same day

- `.` **W17** — [The titlebar strip assumed macOS's numbers, and macOS 27 changed them](backlog/closed-2026-09.md#-w17--the-titlebar-strip-assumed-macoss-numbers-and-macos-27-changed-them--done-2026-09-15) — done (2026-09-15) — measured from the window, `widgets::titlebar_height`
- `.` **W18** — [Picking from a popup flicked the owner's chrome: the popup became key on every press](backlog/closed-2026-09.md#-w18--picking-from-a-popup-flicked-the-owners-chrome-the-popup-became-key-on-every-press--done-2026-09-15) — done (2026-09-15) — `canBecomeKeyWindow` NO on the popup's `NSWindow`

**From the two alpha.13 upgrade reports (2026-09-15)** — F62–F65, all built the same day

- `.` **F62** — [`frameStats().frames` is the ring's length, and nothing counts pumps](backlog/closed-2026-09.md#-f62--framestatsframes-is-the-rings-length-and-nothing-counts-pumps--done-2026-09-15) — done (2026-09-15) — `framesTotal` and `pumps`
- `.` **F63** — [What `env.audio.live` reads for a play that waited on an open the device then refused](backlog/closed-2026-09.md#-f63--what-envaudiolive-reads-for-a-play-that-waited-on-an-open-the-device-then-refused--done-2026-09-15) — done (2026-09-15) — a sentence and an assertion
- `.` **F64** — [`settled()` never resolves while a keyframe cycle runs, and nothing separates a transition owed from a cycle running](backlog/closed-2026-09.md#-f64--settled-never-resolves-while-a-keyframe-cycle-runs-and-nothing-separates-a-transition-owed-from-a-cycle-running--done-2026-09-15) — done (2026-09-15) — `owed()` by kind, `quiet(maxMs)`
- `.` **F65** — [A headless press's default `physical` is `code` verbatim, and a window's is lower-case](backlog/closed-2026-09.md#-f65--a-headless-presss-default-physical-is-code-verbatim-and-a-windows-is-lower-case--done-2026-09-15) — done (2026-09-15) — the fold in `KeyPress::new`, under What breaks

**From the kawoosh terminal field report (2026-09-15)** — F66, the one defect of the day, built the same day

- `!` **F66** — [A `cells` node draws box-drawing and block glyphs from the font, and no font's are the cell's height](backlog/closed-2026-09.md#-f66--a-cells-node-draws-box-drawing-and-block-glyphs-from-the-font-and-no-fonts-are-the-cells-height--done-2026-09-15) — done (2026-09-15) — `cells/boxdraw.rs`, drawn from the cell box

**From a feature ask (2026-09-15): a drop zone** — C40, ADR 0031 built whole; W19 stays open

- `~` **C40** — [A drop zone: `onDrop` on any node, the files as an event, `dropBg` while they hover](backlog/closed-2026-09.md#-c40--a-drop-zone-ondrop-on-any-node-the-files-as-an-event-dropbg-while-they-hover--done-2026-09-15) — done (2026-09-15) — ADR 0031, `onDrop` / `dropBg`, three `InputEvent`s, ABI 18

**From the kawoosh binary-file report (2026-09-16)** — C42 and C43, both built the same day; the report's span-count diagnosis measured and recorded as not the cause

- `~` **C42** — [`rich_text` never takes the long-line path: a 50 KB line with one styled span is shaped whole, and a screenful of them thrashes the cache](backlog/closed-2026-09.md#-c42--rich_text-never-takes-the-long-line-path-a-50-kb-line-with-one-styled-span-is-shaped-whole-and-a-screenful-of-them-thrashes-the-cache--done-2026-09-16) — done (2026-09-16) — rich chunks of the sliced spans, decorations on the wrapped rows
- `.` **C43** — [A long line's cache key is a byte-serial FNV of its whole content, every frame](backlog/closed-2026-09.md#-c43--a-long-lines-cache-key-is-a-byte-serial-fnv-of-its-whole-content-every-frame--done-2026-09-16) — done (2026-09-16) — `key::hash_bulk`, the newline scan only on a miss; the app-supplied key not built

**From the alpha.15 pre-tag round (2026-09-16)** — C44, found by kawoosh on this tree and built in the round

- `!` **C44** — [A long line with a multibyte character on a chunk edge panics in `chunk_ranges`](backlog/closed-2026-09.md#-c44--a-long-line-with-a-multibyte-character-on-a-chunk-edge-panics-in-chunk_ranges--done-2026-09-16) — done (2026-09-16) — the window's end floored to a char boundary

**From the kawoosh syntax-tab report (2026-09-16)** — F67, filed and built the same day, after the alpha.15 tag

- `~` **F67** — [No door selects a devtools tab from the app: a command that jumps to the app's own tab has only the strip's click and `Ctrl+Shift+N`](backlog/closed-2026-09.md#-f67--no-door-selects-a-devtools-tab-from-the-app-a-command-that-jumps-to-the-apps-own-tab-has-only-the-strips-click-and-ctrlshiftn--done-2026-09-16) — done (2026-09-16) — `set_devtools_tab` / `devtools_current_tab` in three bindings

**From the kawoosh idle-frame report (2026-09-16)** — F68, filed and built the same day, after the alpha.15 tag

- `.` **F68** — [A modal editor's block caret arms the blink clock: an idle app in normal mode draws a frame twice a second](backlog/closed-2026-09.md#-f68--a-modal-editors-block-caret-arms-the-blink-clock-an-idle-app-in-normal-mode-draws-a-frame-twice-a-second--done-2026-09-16) — done (2026-09-16) — `caretSolid` on the line, in four bindings; `hasCaret` reaches Node

**From the kawoosh second-Mac report (2026-09-16)** — F69, filed and built the same day, after the alpha.15 tag

- `!` **F69** — [A held key does not repeat on a Mac with press-and-hold on: the accent picker, or nothing, where a modal editor wanted a motion](backlog/closed-2026-09.md#-f69--a-held-key-does-not-repeat-on-a-mac-with-press-and-hold-on-the-accent-picker-or-nothing-where-a-modal-editor-wanted-a-motion--done-2026-09-16) — done (2026-09-16) — `Launcher::press_and_hold` / `pressAndHold` / `kui_press_and_hold`, pinned in the argument domain

**From the kawoosh mid-frame-dock report (2026-09-16)** — F70, filed and built the same day, after the alpha.15 tag

- `!` **F70** — [A panel turned on mid-frame is built into a root not laid out for it: the dock in the bottom-left corner until the next frame](backlog/closed-2026-09.md#-f70--a-panel-turned-on-mid-frame-is-built-into-a-root-not-laid-out-for-it-the-dock-in-the-bottom-left-corner-until-the-next-frame--done-2026-09-16) — done (2026-09-16) — a mid-frame `set_devtools` / `set_devtools_dock` waits for the next frame, and asks for it

**From the kawoosh devtools-strip report (2026-09-16)** — F71 and F72, filed and built the same day, after the alpha.15 tag; F73 the day after

- `!` **F71** — [A grow child's `min` or `max` keeps its share of the run: the room a clamp gives back is a hole, not its siblings'](backlog/closed-2026-09.md#-f71--a-grow-childs-min-or-max-keeps-its-share-of-the-run-the-room-a-clamp-gives-back-is-a-hole-not-its-siblings--done-2026-09-16) — done (2026-09-16) — `distribute_run` freezes a clamped child and shares the rest, as flexbox does
- `~` **F72** — [The devtools strip breaks tab labels, and the theme, accent and menu toggles are icons away from the facts they change](backlog/closed-2026-09.md#-f72--the-devtools-strip-breaks-tab-labels-and-the-theme-accent-and-menu-toggles-are-icons-away-from-the-facts-they-change--done-2026-09-16) — done (2026-09-16) — the strip wraps whole tabs; `widgets::select`, on the Facts rows
- `.` **F73** — [`select` is a Rust widget only: no `<select>`, no Lua `select {}`, no `kui_select`](backlog/closed-2026-09.md#-f73--select-is-a-rust-widget-only-no-select-no-lua-select--no-kui_select--done-2026-09-17) — done (2026-09-17) — `<select>`, `dropdown { }`, `kui_select`, one corpus scene in four adapters, frame v15

**From the kawoosh session report (2026-09-17)** — F74, filed and built the same day

- `!` **F74** — [An app never hears its window go: the close button drops it, ⌘Q ends the process, and neither is a key](backlog/closed-2026-09.md#-f74--an-app-never-hears-its-window-go-the-close-button-drops-it-q-ends-the-process-and-neither-is-a-key--done-2026-09-17) — done (2026-09-17) — `App::teardown`, once, from the loop's `exiting` and a pumped runner's retirement

**From the regression pass of 2026-09-19** — RG1 built the same day, RG2 and RG12 on 2026-09-20, the five table entries RG3, RG6, RG7, RG8 and RG11, the left dock's RG4, the freeze loop's RG5, the select's RG9 and RG10, the reader's RG13, F69's check RG15, the nits RG14 and the removal RG16 the same day; none open

- `!` **RG1** — [A Node or C app still never hears ⌘Q: F74's `teardown` is a Rust `App` method, and the loops the other hosts "own" end the same way](backlog/closed-2026-09.md#-rg1--a-node-or-c-app-still-never-hears-q-f74s-teardown-is-a-rust-app-method-and-the-loops-the-other-hosts-own-end-the-same-way--done-2026-09-19) — done (2026-09-19) — `teardown(model)` in `runWindowed`'s / `createApp`'s config over `KuiWindow.onTeardown`, `kui_on_teardown(fn)` before `kui_run` (ABI 18 kept), the verb-table row, the once-across-ends test; both hosts checked under ⌘Q in the window
- `!` **RG2** — [A devtools select's menu outlives the panel, and its choice reaches the app as a `menu` event from the devtools origin](backlog/closed-2026-09.md#-rg2--a-devtools-selects-menu-outlives-the-panel-and-its-choice-reaches-the-app-as-a-menu-event-from-the-devtools-origin--done-2026-09-20) — done (2026-09-20) — the panel's menu closes when this window stops building the panel; a devtools-origin event is taken back whatever the panel's state
- `~` **RG12** — [`Ctrl+Shift+M` is a two-way toggle from a compile-time default; the select beside it has three choices](backlog/closed-2026-09.md#-rg12--ctrlshiftm-is-a-two-way-toggle-from-a-compile-time-default-the-select-beside-it-has-three-choices--done-2026-09-20) — done (2026-09-20) — the chord walks platform → native → drawn over the select's state and restore path
- `!` **RG3** — [A `scrollX` table with grow rows cannot scroll: the overflow it keeps is clipped, and `scroll_max.x` is 0](backlog/closed-2026-09.md#-rg3--a-scrollx-table-with-grow-rows-cannot-scroll-the-overflow-it-keeps-is-clipped-and-scroll_maxx-is-0--done-2026-09-20) — done (2026-09-20) — a row of a `scrollX` table is at least as wide as its columns, chrome counted; `scroll_max.x` sees them
- `!` **RG6** — [F75's floor fix drops a node-anchored float root's own `min: fit`: the sixth pass clamps it with a floor of 0](backlog/closed-2026-09.md#-rg6--f75s-floor-fix-drops-a-node-anchored-float-roots-own-min-fit-the-sixth-pass-clamps-it-with-a-floor-of-0--done-2026-09-20) — done (2026-09-20) — `anchored` reads the root's spec after each fit pass; the regression of the round
- `!` **RG7** — [Any in-flow container straight under a table is a row: a column wrapper's stacked children become cells and widen the columns](backlog/closed-2026-09.md#-rg7--any-in-flow-container-straight-under-a-table-is-a-row-a-column-wrappers-stacked-children-become-cells-and-widen-the-columns--done-2026-09-20) — done (2026-09-20) — only a `Row` is a row; `LayoutSpec::is_table()` for every reader; ADR 0033 decisions 2 and 9
- `~` **RG8** — [An image cell is stretched to its column and re-aspected](backlog/closed-2026-09.md#-rg8--an-image-cell-is-stretched-to-its-column-and-re-aspected--done-2026-09-20) — done (2026-09-20) — the box is the column wide and the image's own aspect tall; the pixels meet it by `fit`; box an icon
- `~` **RG11** — [A `Fit` table of grow rows collapses to 0, and the howto's only snippet builds exactly that](backlog/closed-2026-09.md#-rg11--a-fit-table-of-grow-rows-collapses-to-0-and-the-howtos-only-snippet-builds-exactly-that--done-2026-09-20) — done (2026-09-20) — a table's fit is its columns' whatever the rows' sizing: pass 1 sizes the grow rows to them too
- `!` **RG4** — [F70's deferral is defeated by a left dock: the panel is built at `begin_frame`, so a mid-frame dock move or panel-off draws the stale panel and asks for no frame](backlog/closed-2026-09.md#-rg4--f70s-deferral-is-defeated-by-a-left-dock-the-panel-is-built-at-begin_frame-so-a-mid-frame-dock-move-or-panel-off-draws-the-stale-panel-and-asks-for-no-frame--done-2026-09-20) — done (2026-09-20) — `finish` compares `on`, the dock and the tab against what `begin_frame` built the left panel from, and a change since asks for the next frame
- `!` **RG5** — [The F71 freeze loop freezes min and max violators in the same pass, and a plain sibling can get nothing](backlog/closed-2026-09.md#-rg5--the-f71-freeze-loop-freezes-min-and-max-violators-in-the-same-pass-and-a-plain-sibling-can-get-nothing--done-2026-09-20) — done (2026-09-20) — flexbox's sign rule: a pass freezes only the violators of the dominant sign and re-shares; a byte per child in `Tree::grow_scratch` for the frozen set; the `frame_1k_grow_rows_capped` row; the regression of the round
- `!` **RG9** — [`activate_menu_item` posts a disabled option's choice](backlog/closed-2026-09.md#-rg9--activate_menu_item-posts-a-disabled-options-choice--done-2026-09-20) — done (2026-09-20) — a row that cannot be chosen (disabled, a separator) is refused in the core: `Option<Vec<UiEvent>>`, `false` at both doors, nothing posted, the menu still open
- `~` **RG10** — [A select's `options` and `current` are checked by C and not by Node or Lua: an empty menu, a blank field, a check on a separator](backlog/closed-2026-09.md#-rg10--a-selects-options-and-current-are-checked-by-c-and-not-by-node-or-lua-an-empty-menu-a-blank-field-a-check-on-a-separator--done-2026-09-20) — done (2026-09-20) — the reader refuses `[]`; `select-current-ignored` from `select_with` for every binding; an option object's unknown key is `unknown-prop` with the row's wording (`MenuItem::KEYS`); Lua's missing label named; C's `count == 0` stays, its rows never pass the reader
- `~` **RG13** — [An AX click on a control behind a modal fires: a second click on an open select's field re-opens it instead of dismissing](backlog/closed-2026-09.md#-rg13--an-ax-click-on-a-control-behind-a-modal-fires-a-second-click-on-an-open-selects-field-re-opens-it-instead-of-dismissing--done-2026-09-20) — done (2026-09-20) — it was dropped, not fired: a reader's `Click` outside the modal is now the press outside, a `dismiss` on the modal and nothing on the node; the select's menu closes

- `.` **RG14** — [Docs and parity nits from the pass, all one line each](backlog/closed-2026-09.md#-rg14--docs-and-parity-nits-from-the-pass-all-one-line-each--done-2026-09-20) — done (2026-09-20) — nine of ten as written, (c) already RG1's, (f) wrong as filed (C's `kui_nodes` carries `table`; pinned); ADR 0033 decision 10 (a percent column's basis), the solid caret's unfocused phase is the view's (F68 amended, `modal_editor` hollow); plus the pick over a pending tab name and case-folded own tab names
- `.` **RG15** — [F69 was not checked in a window: the pin is proven against `NSUserDefaults`, not against AppKit's read](backlog/closed-2026-09.md#-rg15--f69-was-not-checked-in-a-window-the-pin-is-proven-against-nsuserdefaults-not-against-appkits-read--done-2026-09-20) — done (2026-09-20) — checked: HIToolbox reads the global domain by name and no per-process default reaches it, the named fallback included; the door is inert on macOS 27 and says so; RG16 holds the decision
- `!` **RG16** — [F69's door is inert: HIToolbox reads the user's global domain by name, and which mechanism replaces the pin, if any, is a decision](backlog/closed-2026-09.md#-rg16--f69s-door-is-inert-hitoolbox-reads-the-users-global-domain-by-name-and-which-mechanism-replaces-the-pin-if-any-is-a-decision--done-2026-09-20) — done (2026-09-20) — option (c), the user chose: the door removed from all three bindings while alpha.16 is unreleased; the howto says it is the user's setting

**From the kawoosh devtools-tables report (2026-09-17)** — F75, filed and built the same day

- `~` **F75** — [Every key/value list in the devtools lines its values up behind a label box of a width picked by hand](backlog/closed-2026-09.md#-f75--every-keyvalue-list-in-the-devtools-lines-its-values-up-behind-a-label-box-of-a-width-picked-by-hand--done-2026-09-17) — done (2026-09-17) — `NodeSpec::table()`, `dir="table"`, `grid { }`, `KUI_TABLE`: a column whose rows' cells align, in the layout passes (ADR 0033); the six lists are tables

**From the kawoosh Russian-layout report (2026-09-21)** — F76, filed and built the same day

- `!` **F76** — [Shift is lost under the layout fallback: `J` on a Russian layout is `j`, and Shift on the key printed `;` is `;`](backlog/closed-2026-09.md#-f76--shift-is-lost-under-the-layout-fallback-j-on-a-russian-layout-is-j-and-shift-on-the-key-printed--is---done-2026-09-21) — done (2026-09-21) — `from_layout` stands in the US-QWERTY key as Shift prints it; the winit runner's Alt path resolves shift-less, as it reads the key; ADR 0002 decision 11 amended

**From the kawoosh scrolling-tab report (2026-09-22)** — F79 and F80, filed and built the same day

- `!` **F79** — [A focused key sink drawn outside its container's clip hears nothing: the keyboard asks the hit list, where only what a point can reach is](backlog/closed-2026-09.md#-f79--a-focused-key-sink-drawn-outside-its-containers-clip-hears-nothing-the-keyboard-asks-the-hit-list-where-only-what-a-point-can-reach-is--done-2026-09-22) — done (2026-09-22) — the delivery and the `key_up` opt-in resolve the sink from the tree (`Core::sink_node`), `disabled` and the modal boundary refusing as before; the pointer's rule untouched; ADR 0011 decision 9
- `~` **F80** — [A programmatic scroll can only jump: there is no way to ask a container to glide to where a reveal puts it](backlog/closed-2026-09.md#-f80--a-programmatic-scroll-can-only-jump-there-is-no-way-to-ask-a-container-to-glide-to-where-a-reveal-puts-it--done-2026-09-22) — done (2026-09-22) — a `transition` on a scroll container eases the offset a `reveal` or `set_scroll` moves it to; the wheel, the thumb and an edge drag land whole and interrupt a leg; `scroll_geometry` answers the drawn offset, `scroll_offset` the target

**From the kawoosh virtual-list report (2026-09-22)** — F77 and F78, filed and built the same day

- `!` **F77** — [A virtual list sliced by a geometry that moved stays a frame behind until the next event: five rows in a tall pane, a blank edge under the wheel](backlog/closed-2026-09.md#-f77--a-virtual-list-sliced-by-a-geometry-that-moved-stays-a-frame-behind-until-the-next-event-five-rows-in-a-tall-pane-a-blank-edge-under-the-wheel--done-2026-09-22) — done (2026-09-22) — `ScrollStore` records what `geometry` handed out this build and `resliced` says whether layout placed it otherwise; `layout_frame` asks for the frame that closes the lag
- `!` **F78** — [A stroke or a polygon in a scrolled row spills past the scroller: a float escapes every ancestor's clip, the one it was drawn inside included](backlog/closed-2026-09.md#-f78--a-stroke-or-a-polygon-in-a-scrolled-row-spills-past-the-scroller-a-float-escapes-every-ancestors-clip-the-one-it-was-drawn-inside-included--done-2026-09-22) — done (2026-09-22) — a `line` or `polygon` anchored in its parent's box takes the parent's clip as a child would; a declared float and a viewport-anchored stroke still escape; ADR 0010 decision 5 amended

**From the kawoosh Lua-types report (2026-09-22)** — F81, filed and built the same day

- `~` **F81** — [Nothing describes the Lua DSL to lua-language-server: `row`, `text` and every prop are undefined to it](backlog/closed-2026-09.md#-f81--nothing-describes-the-lua-dsl-to-lua-language-server-row-text-and-every-prop-are-undefined-to-it--done-2026-09-22) — done (2026-09-22) — `kui_lua::luals_meta()`: a `---@meta` file of the prelude's constructors and a `kui.Props` class, generated from the schema

**From the kawoosh tab-strip report (2026-09-23)** — F82, filed and built the same day

- `!` **F82** — [Two reveals in one frame into different scroll containers: the first is dropped](backlog/closed-2026-09.md#-f82--two-reveals-in-one-frame-into-different-scroll-containers-the-first-is-dropped--done-2026-09-23) — done (2026-09-23) — the pending reveals are a list, kept last-per-container (the nearest scrolling ancestor) and all resolved after layout
