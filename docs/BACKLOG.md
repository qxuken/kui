# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
the six rounds that followed it, and the field reports from two apps built on
alpha.6, alpha.7 and alpha.8 outside this repo (F1–F15 on 2026-09-06,
F16–F23 and F25–F31 on 2026-09-07). Every item names the
evidence that produced it, so a task that turns out to be wrong can be argued with rather
than guessed at.

**This file is the open list.** The sixty-three closed entries — each with its
outcome written on top of the original finding, and the tables, profiles and
evidence it argued from — are in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md); forty-six moved there
on 2026-09-06, the remaining ten field-report entries followed the same day
before the alpha.7 tag, and W2 went whole on 2026-09-07 when ADR 0009's driver
half was built. The index at the bottom of this file names every one of them,
so an id cited by an open item, a code comment or a commit message can be
resolved without opening the archive. Nothing was renumbered in any of those
moves, and nothing ever is. What is left here is three parked headings — C12,
C13 and C14 — the seven entries from the two alpha.8 field reports,
F25–F31 (2026-09-07), the eight from the editor-and-mux assessment,
C16–C23, with W3 filed beside them (2026-09-07), and what comes next; F16–F23 from the two alpha.7
field reports all closed the day they were filed (2026-09-07). C15's
remainder was the last split entry, and it closed on 2026-09-07.

Ordered by area, not by priority. What to do next is under "After alpha.8".

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
on 2026-09-08, cutting alpha.9. What stays here is F27, built and waiting
for the first publish that runs it, and the three claims that were not ours.

### `~` F27 — There is no supported way to learn a release exists, and every range an app writes floats — **built (2026-09-07), unverified until alpha.9 publishes**

**Built 2026-09-07 — unverified on the registry until alpha.9 ships.** Both
halves of the "Do" below are in the tree. `ci.yml`'s npm publish step follows
`npm publish --tag "$tag"` with `npm dist-tag add "@qxuken/kui@$ver" latest`
whenever the tag is not already `latest`, guarded on `$ver` sorting highest
among the versions the registry already holds: the list comes from `npm view
"@qxuken/kui@$tag" versions --json` (asked through the tag just written,
because a plain `npm view` defaults to the `latest` whose absence is the bug
and then prints nothing and exits 0), and the sort is npm's own bundled
`semver`, resolved through `npm root -g` so nothing has to be installed for
it. Checked here against the live registry: it picks `0.1.0-alpha.8` out of
the seven published versions, orders `alpha.10` above `alpha.9`, and picks
`0.1.0` over an `alpha.10` published after it — which is the guard doing its
job. The step is `bash -n` clean and the file still parses as YAML. Both
READMEs and the template's now say that `latest` and `alpha` point at the
same newest alpha, that `^` and `~` float alike across the alphas of one
tuple so an app pins an exact version and leans on its lockfile, and that
`npm view @qxuken/kui@alpha version` is the query if `latest` is ever absent.
**What is not verified:** the tag itself. `latest` exists on the registry only
after a publish runs the new step, so `npm view @qxuken/kui version` keeps
printing nothing until alpha.9 goes out; this entry stays open until it
answers.

The pomodoro's headline, verified here from the app's own directory with
the scope routed to Forgejo: `npm view @qxuken/kui version` prints nothing
and exits 0, so does `dist-tags`, `npm outdated` lists `@types/node` and
not kui, and only `npm view @qxuken/kui@alpha version` answers
`0.1.0-alpha.8`. The registry has one dist-tag, `alpha`
(`{"alpha":"0.1.0-alpha.8"}`), because the publish step
(`ci.yml:577-583`) gives a prerelease its identifier as the tag and
`latest` only to a plain version — which there has never been. `npm view`
defaults to `latest`, and against a package without one it says nothing
and succeeds. "I found out alpha.8 existed because I was told."

The second half: `"^0.1.0-alpha.7"` installs alpha.8 in a clean directory,
so `package.json` "is decoration" without a lockfile. True — and the
report's fix is not. `~0.1.0-alpha.7` satisfies `0.1.0-alpha.8` too
(checked against npm's own `semver`: both ranges admit any prerelease of
the same `0.1.0` tuple). Only an exact version pins an alpha. The
template's `^0.1.0-alpha.8` floats the same way, which is what a floor is
for and what its README says ("pins the minimum"); an *app* that wants
the version it tested is a different case and nothing tells it so.

**Do:** (1) `ci.yml`: after `npm publish --tag alpha`, `npm dist-tag add
@qxuken/kui@$ver latest` — an alpha under `latest` is what every 0.x does,
and it is what makes `npm view`, `npm outdated` and a bare `npm install
@qxuken/kui` mean something. Guard it against a future stable: add
`latest` only when the version being published sorts highest among the
registry's versions. (2) Both READMEs: `latest` and `alpha` both point at
the newest alpha; `^` and `~` both float across the alphas of one tuple;
an app pins an exact version and its lockfile is what holds; `npm view
@qxuken/kui@alpha version` is the query if `latest` is ever absent. `.` as
work, `~` as an outcome — an app "can be running a release it has no way
to discover it is running".

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
on 2026-09-08, cutting alpha.9. What stays here is W3, built blind against
winit's source and waiting for a Windows machine.

### `!` W3 — On Windows, animations stop while the window is grabbed — **built blind (2026-09-08), unverified**

Built as the hypothesis's second branch, since reading winit 0.30's
Windows backend answered the first: `WM_ENTERSIZEMOVE` only sets
`MARKER_IN_SIZE_MOVE` and arms no timer, so nothing ticks `about_to_wait`
inside the modal loop and the redraw it would have asked for never comes.
The modal loop does dispatch `WM_PAINT`, so the runner now re-requests a
redraw from `RedrawRequested` on Windows while the pane's core is
`animating()` — the chain sustains itself through the hold, vsync-paced
as before, and the other platforms are untouched. Not the subclass
timer: this needs no subclass, so it holds under `Chrome::Native` too.
`cargo check -p kui --target x86_64-pc-windows-msvc` passes; nothing
ran. (1) and (3) of the "Do" are still the by-hand round's, and the
entry stays open until a Windows machine watches `toasts` mid-spring
with the title bar held.

Not from the measurement: reported beside it, on 2026-09-07, as a
regression — a transition mid-flight freezes for as long as the title
bar is held, and resumes on release. Not reproduced here (no Windows
machine, and P8's `SMOKE_WINDOWS` job is still waiting for a runner), so
the mechanism below is the hypothesis the report fits, not a finding.

Windows moves and resizes a window inside its own modal loop
(`WM_ENTERSIZEMOVE` … `WM_EXITSIZEMOVE`, run by `DefWindowProc`), and
for its duration the process's message loop is that one, not winit's.
kui's animation pacing is `about_to_wait` (`lib.rs:2234`): a pane whose
core is `animating()` asks for a redraw there, and the loop parks in
`ControlFlow::WaitUntil` for the caret and audio clocks. Inside the
modal loop `about_to_wait` runs only when a message reaches winit's
handler, and the redraw it requests is what `RedrawRequested` answers —
so whether frames keep coming depends on whether anything ticks in
there. If winit already arms a timer for the modal loop, the bug is
kui's (a redraw asked for once and never re-asked); if it does not, the
subclass `windows_nc.rs` already installs for custom chrome is the
place to arm one: `SetTimer` on `WM_ENTERSIZEMOVE`, a redraw on each
`WM_TIMER`, `KillTimer` on exit — and it has to apply under
`Chrome::Native` too, where the subclass is not installed today. Which
alpha introduced it is not known here; the driver's pacing moved with
ADR 0004 step 3 (multi-window) and F15's tween fix, and either could be
the edge.

**Do:** (1) Reproduce on Windows first, by hand — `toasts` with a panel
springing open, grab the title bar mid-spring — and read whether
`RedrawRequested` arrives at all during the hold (a counter in the HUD
is enough). (2) Fix at the layer the answer names: re-request inside
`RedrawRequested` while animating if winit ticks, the timer in the
subclass if it does not. (3) Since the frame clock is the wall's, the
tween catches up on release rather than replaying, which is right; pin
it in the same by-hand check. It joins the by-hand round's list under
"After alpha.8" until the smoke job exists. A `!` because it ships
today on one platform; the mixed-DPI drag check W2 left for Windows is
the same session's work.

## After alpha.9

Grouped by kind, not urgency. Nothing here blocks the tag. It was "After
alpha.8" until 2026-09-08, when the editor-and-mux round it described had
landed whole for alpha.9 and the heading moved with the tag; "After
alpha.7" before that, until ADR 0012's remainder and ADR 0013 landed for
alpha.8; "After alpha.6" before it went to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#after-alpha6) whole
rather than accumulating strikethroughs, because every line of it had closed.

**Build.** ADR 0004 step 1's leftover: the glyph atlas and shape cache are
still per window because `Core::output` hands out `&mut GlyphAtlas` (see
`kui-session-atlas-constraint`); it waits for a case where two windows share
enough text to matter. C15 is closed (2026-09-07): its remainder was
profiled and the passes that could be skipped are, and what is still above
the 2026-08-31 baseline is the struct's size in the app's own builder chain,
which the archived entry measures and leaves.

**Build next.** Nothing with a written ADR and no code. Slots, [`docs/adr/0014-slots-an-extension-fills-in-place.md`](adr/0014-slots-an-extension-fills-in-place.md), were proposed, accepted and **built on 2026-09-07** for alpha.9 — an extension fills a place the host declares in its own view, under a namespace the host decides, with `Value` parameters in and replies out; its status block records what the building changed, and the first test it pins was a defect before it: an extension whose root carried no `key` was rekeyed whenever the host added a child at the root. Before it, ADR 0012's
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
what a deletion line names). Six of them are in the archive as of
2026-09-08; F27 waits for alpha.9's publish, the first to run the
`latest` step it built.

**Editor and mux (2026-09-07, built 2026-09-07/08).** The assessment
section above was the order of work, and it was followed: C16, C18,
C17, C19 for the editor and C21, C23, C22 for the mux all landed with
their outcomes written on top, then C20's element in every binding, the
corpus `ime` scene and C19's wrapped long lines the next day — all eight
are in the archive. W3 is built blind against winit's source and waits
for a Windows machine. What the round found beside the entries: Lua's
`text()` mutated a shared options table, `npm run gen` described the
previous build, the C parity assert caught a field appended in two
orders, and two things the examples showed once run — the counter's
first click waiting 92 ms for the audio device to open, and a departing
list drawing past its own box — are fixed under alpha.9's `### Fixed`.

**Design, wanting an ADR.** Nothing new since ADR 0014 (above) was built on 2026-09-07; what it leaves open — a slot element for Node, extensions in `kui_run`, an extension offering slots of its own — waits for a view. Two instances of one extension are answered: the host namespaces them. Effects an app defines (F23) is
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
core and route nowhere"). Physical key positions beyond what `60ca137`
carried. One did ask, on 2026-09-07: an i3-style tab bar wanted `grow`
tabs floored at their labels, and `minWidth: "fit"` / `minHeight: "fit"`
landed the same day for alpha.9 (the CHANGELOG entry says why it is not
the default — a fit width is the unwrapped one).

**Parked on their own terms.** C12 (column wrapping), C13 (`space-between`
and baseline), C14 (aspect ratio), C5(b) (core-side virtualisation), rounded
clip nesting. Each says "wait for a view that wants it", and each should keep
saying it until one does.

**Hygiene.** Archiving is done four times over: the forty-six of
2026-09-06, the ten field-report entries that followed them before the tag —
so all of F1–F15 sit together — W2 whole on 2026-09-07, once its driver
half was built, and the fourteen of alpha.9's round on 2026-09-08. This
file is three parked entries, F27 and W3, and this section.
Still open, both waiting on something outside the repo: enable `SMOKE_MACOS`
/ `SMOKE_WINDOWS` the day a runner exists (P8) — which has two jobs waiting
for it now, F13's launch probe beside the AX audit, sharing the one
Accessibility permission — and remove Lua `env.focus` at 0.2 (P3, R7).

**For the by-hand round**, which is R4's list in the archive and now also
`### Native verification` under each release heading in the CHANGELOG. Two
items are in it because the frame they live in is between a press and a
release, which no headless assertion reads:

- `npm run mindmap` in `examples/node`, then **drag the empty canvas and
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

Eighty-five entries, all in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md) and all verbatim.
This index is here so an id resolves without opening that file: the open items
above cite A1, C7, C9, C10, D2, P3, P5, P8, R3, R6 and S2, "After alpha.8" and
the hygiene note cite C2, C5(b), P3, R4 and R7, and code comments, ADRs and
commit messages cite ids of their own. All sixty-three are whole in the
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

**From two alpha.8 field reports (2026-09-07)** — F25, F26 and F28–F31, closed the day they were filed; F27 stays above until a publish runs it

- `~` **F25** — [`setEditText` before the editor exists does nothing, and alpha.8's deletion list told an app to delete a latch it still needs](backlog/closed-2026-09.md#-f25--setedittext-before-the-editor-exists-does-nothing-and-alpha8s-deletion-list-told-an-app-to-delete-a-latch-it-still-needs--done-2026-09-07) — done (2026-09-07) — `setEditText` seeds the editor the next frame declares, and warns when none does
- `~` **F26** — [Editor and scroll state are kept by key forever](backlog/closed-2026-09.md#-f26--editor-and-scroll-state-are-kept-by-key-forever--done-2026-09-07) — done (2026-09-07) — a ceiling on undeclared editor and scroll state, longest-undeclared evicted first
- `.` **F28** — [`quads()` on a window does not say how to drive one](backlog/closed-2026-09.md#-f28--quads-on-a-window-does-not-say-how-to-drive-one--done-2026-09-07) — done (2026-09-07) — `quads()` says how to drive the window it reads
- `~` **F29** — [An `<audio>` one-shot that must finish has to guess its own length](backlog/closed-2026-09.md#-f29--an-audio-one-shot-that-must-finish-has-to-guess-its-own-length--done-2026-09-07) — done (2026-09-07) — the `audio` element's `finish`, so a one-shot need not guess its length
- `~` **F30** — [A window has no settled frame by name, so a smoke test sleeps](backlog/closed-2026-09.md#-f30--a-window-has-no-settled-frame-by-name-so-a-smoke-test-sleeps--done-2026-09-07) — done (2026-09-07) — `settled` and `frame` on `WindowLoop`
- `.` **F31** — [Three doc shapes the reports paid for](backlog/closed-2026-09.md#-f31--three-doc-shapes-the-reports-paid-for--done-2026-09-07) — done (2026-09-07) — `docs/howto.md`, the `**What breaks.**` list, and what a deletion line names

**From the editor-and-mux assessment (2026-09-07)** — C16–C23, all eight built between 2026-09-07 and 2026-09-08; W3 stays above, built blind

- `!` **C16** — [The shaped-text cache has a clock and no budget](backlog/closed-2026-09.md#-c16--the-shaped-text-cache-has-a-clock-and-no-budget--done-2026-09-07) — done (2026-09-07) — a byte budget on the shaped-text cache, LRU past it, never what the last frame drew
- `~` **C17** — [IME reaches the stock editor only](backlog/closed-2026-09.md#-c17--ime-reaches-the-stock-editor-only--done-2026-09-07) — done (2026-09-07) — `InputEvent::Commit`, preedit and commit to the sink an app-owned editor is, the candidate window at its caret; corpus `ime` scene on 2026-09-08
- `~` **C18** — [Nothing maps a point to a byte offset, or an offset to a rect, on text the app owns](backlog/closed-2026-09.md#-c18--nothing-maps-a-point-to-a-byte-offset-or-an-offset-to-a-rect-on-text-the-app-owns--done-2026-09-07) — done (2026-09-07) — `text_hit` and `caret_rect` by the enclosing key, across runs, in every binding
- `~` **C19** — [A long line is shaped whole, and slicing it from outside costs more than not slicing](backlog/closed-2026-09.md#-c19--a-long-line-is-shaped-whole-and-slicing-it-from-outside-costs-more-than-not-slicing--done-2026-09-08) — done (2026-09-08) — a long line shaped in ~1 KB chunks on demand; step 5, a wrapped one, on 2026-09-08
- `.` **C20** — [A cell grid inside the core, if it beats per-cell nodes by an order of magnitude](backlog/closed-2026-09.md#-c20--a-cell-grid-inside-the-core-if-it-beats-per-cell-nodes-by-an-order-of-magnitude--done-2026-09-08) — done (2026-09-08) — `cells` — a terminal screen as one node, ~37× cheaper than nodes, the element in every binding on 2026-09-08
- `~` **C21** — [Nothing outside the main thread can wake `run`, and `pump` never waits](backlog/closed-2026-09.md#-c21--nothing-outside-the-main-thread-can-wake-run-and-pump-never-waits--done-2026-09-07) — done (2026-09-07) — `kui::Waker` and `pump_until`, a thread wakes the parked loop
- `.` **C22** — [Underline, strikethrough, and a background per span](backlog/closed-2026-09.md#-c22--underline-strikethrough-and-a-background-per-span--done-2026-09-07) — done (2026-09-07) — underline, strikethrough and a background per span (ABI 8)
- `~` **C23** — [No way to turn ligatures off, or tabular figures on](backlog/closed-2026-09.md#-c23--no-way-to-turn-ligatures-off-or-tabular-figures-on--done-2026-09-07) — done (2026-09-07) — `features`: ligatures off, tabular figures on
