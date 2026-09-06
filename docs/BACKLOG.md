# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
the six rounds that followed it, and two field reports from apps built on
alpha.6 outside this repo (F1–F15, 2026-09-06). Every item names the
evidence that produced it, so a task that turns out to be wrong can be argued with rather
than guessed at.

**This file is the open list.** The forty-six closed entries — each with its
outcome written on top of the original finding, and the tables, profiles and
evidence it argued from — moved to
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md) on 2026-09-06. The
index at the bottom of this file names every one of them, so an id cited by an
open item, a code comment or a commit message can be resolved without opening
the archive. Nothing was renumbered in the move, and nothing ever is.

Ordered by area, not by priority. What to do next is under "After alpha.6".

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

### `~` C15 — `NodeSpec` was 728 bytes and the frame got ~2.5× more expensive — **fixed (2026-09-05), ~two-thirds recovered**

Fixed for the tag by boxing the four cold field groups, which took `NodeSpec`
from 728 bytes to 224 and made alpha.6 16–22% faster than the release before
it. The measurement record — the bisect staircase, the `sample` profiles, the
padding experiment, the recovery table and the two guards now in place — is in
[the archived entry](backlog/closed-2026-09.md#-c15--nodespec-was-728-bytes-and-the-frame-got-25-more-expensive--fixed-2026-09-05-two-thirds-recovered).
What that entry ends on is what is still open, and it is reproduced here as
written:

**What is left.** `frame_10k_rects` is still ~1.5× its 2026-08-31 cost, and
that half is the per-node logic the features added, not the struct. It wants
its own profile now that the cache behaviour has changed — the shape of the
answer is which of the added passes can be skipped wholesale with a tree-level
flag, the way `any_exit` already skips the depart diff. Also still open: a
threshold on `frame_10k_rects` and `frame_1k_typical`. The size test catches
the specific mistake that caused this one; it does not catch a slow pass, and
a threshold would. **Not in CI** — decided 2026-09-06: the docker runner is
too weak, and a check that false-fails gets disabled. The guard is a local
`scripts/bench-check.sh` that benches HEAD against the previous tag in a
worktree (the method C15's bisect and the alpha.5-vs-alpha.6 measurement
both used) and sits on the pre-tag run list.

A note for whoever picks that up: `Quad` also grew, 92 → 124 bytes, and the
display list is one per quad. It did not show up in either profile — emission
is a small share of a frame — but it is the same mistake in the same place,
and `frame_10k_rects_with_shadows_and_opacity` (20k quads) is the bench that
would show it.

---

## From two field reports (2026-09-06)

Two apps built against the alpha.6 npm package, by one author each, outside
this repo: a mind map (`playground/kui/mind-maps/FINDINGS.md` — a canvas of
floats with drag, a keyboard-first edit flow, three modal overlays and undo)
and an LCARS pomodoro migrated from alpha.4 and tested under real VoiceOver
(`playground/kui/my-app/ALPHA6-FINDINGS.md`). Both say the model held:
messages as data, one `update`, the access tree as the test surface, `modal`
on a float, typed props catching typos before `unknown-prop` could. What
follows is every item of theirs that survived being checked against `main`
at `2e0f14e`, with the check written in; two of their claims were half wrong
and the entry says which half. Numbered F1–F15 in one sequence, ordered by
the legend mark rather than by report; "After alpha.6" says where each goes.
The reports' own app bugs (a visible label used as a key, a font id shared
across cores) are theirs and are not here.

### `!` F1 — The Node loop never sets the frame clock until the first `advance` — **done (2026-09-06)**

`createLoop` stamps the frame clock when it is built and at the top of
`draw()` (`surface.setTime?.(at() / 1000)`), and `advance` no longer sets
it itself — one place, so `render()`, `click()`, `type()`, `step()` and
`advance()` all draw under the same hands. The window needed nothing:
`KuiWindow` has no `setTime` (the optional call is a no-op there) and the
Rust runner already stamps every frame from its own epoch
(`crates/kui/src/lib.rs`, `pane.core.set_time(epoch.elapsed())`), so a
wall-clock loop does not double it; the `clock` option is only where `at()`
reads from. The repro is a test in `packages/kui/test.mjs` ("render and an
event-driven frame share the clock advance moves (F1)"): render, dispatch,
render — still 100, the baseline — then `advance(50)` is strictly between
100 and 400 with `animating()` true, and `advance(200)` is 400 and false.

Mutation-tested: deleting the `setTime` in `draw()` fails that test at
"mid-flight at 50 ms, not 100" and also fails the older `advance`
transition test, which had only passed because it called `advance(0)`
before the change — the workaround in disguise. Deleting the creation-time
line alone fails nothing, because `draw()` covers it; it stays for anything
that reads the surface before the first frame. The rest of the suite passed
unchanged, so no in-repo test was leaning on the snap. Docs: `setTime`'s
doc (in `lib.rs` and `index.d.ts`, edited by hand to the same words) no
longer calls the unset clock "the default for headless tests", the README's
Testing section says the loop keeps the clock, and the changelog carries it
under Fixed and "what you can delete". F14's first bullet still stands.

Evidence: mind-map #8, `repro/transition-advance.tsx`. A keyed box going
100 → 400 under `transition={200}` is already at 400 in the frame that
applies the change, and `animating()` then reports true for the whole
200 ms while nothing moves; the same change driven by `ctx.setTime` around
each render eases 100 → 273 → 395. Verified in `packages/kui/index.js`:
`createLoop` (line 94) keeps `now` from line 99 on, but the only
`surface.setTime` call is inside `advance` (line 213). `render()`,
`click()`, `type()` and every event-driven `draw()` run with the clock
unset — where transitions snap by documented design (`setTime`'s doc,
`index.d.ts:717`) — so the first `advance` is the first clock the core sees,
the transition's baseline was taken under none, and the first advanced frame
is already past the end. The pomodoro report praises `advance` for moving
the transition clock with the ticks; it only ever checked that the ticks
land.

The consequence is larger than one number: nothing `transition`, `slide`,
`enter` or `exit` does is observable from a Node test, and the mind map's
fifty-eight assertions passed against a map that was visibly torn on screen
(F14, first bullet).

**Do:** set the clock at loop creation (`surface.setTime?.(now / 1000)`
right after line 99) and before every `draw()`, so `render()` and an
event-driven frame share the clock `advance` moves; a `clock`-driven loop
(the window) sets it from that clock the same way — check what `KuiWindow`'s
runner already does before doubling it. **Test:** the repro as a Node test —
click, `advance(50)`, assert the decoded quad's width is strictly between
100 and 400 and `animating()` is true; `advance(200)`, assert 400 and false.
Mutation-test it by deleting the new `setTime` line (the D2 rule for
`encoder.js` applies to the loop too).

### `!` F3 — `onKey` fires on release too, and an alpha.4 keymap runs every binding twice — done (2026-09-06)

**Done (2026-09-06), as (a).** `keyUp` is a `Flag` row (id 81) beside
`onKey` — `key_up` in Lua and C, `.key_up()` in Rust: `onKey` alone hears
presses, `onKey` + `keyUp` hears both, the payload unchanged. The argument
for (a) over (b): a keymap is every sink in this repo's examples and in
both field reports, and a default the dominant case has to guard against
is the wrong default however well the note is written — (b) would have
moved the sentence, not the bug. C9's guarantee survives for the sinks
that opt in. The routing change is one guard in `route_key`
(`runtime/dispatch.rs`): a release is dropped at delivery when the hit
region's `key_up` is unset, and the held-key bookkeeping is untouched, so
a stray release still resolves to nothing, focus leaving still lets go
(silently, to a sink that never asked), and a sink that opts in mid-hold
hears the release it is owed. One schema row, so the four bindings got it
mechanically; the ABI parity test forced `KuiSpec.key_up` (an [in]
append, no bump). Tests: `keys.rs`
`a_sink_without_key_up_hears_presses_only`; `test.mjs` has the keymap
fixture that presses *and* releases and asserts one toggle; the corpus's
`keys` scene — two sinks clicked into focus in turn, driven by the new
`keydown` / `keyup` steps, reporting `key down` / `key down` / `key up` —
pins it across Rust, Lua, C and Node. Examples: the C and Node counters'
sinks, which deliberately count both halves, say `key_up`; the four Rust
examples that guarded on `phase == "down"` lost the guard. The CHANGELOG
carries the "what breaks" line for the alpha.7 tag: a held-key binding
written against alpha.6 adds `keyUp`.

Evidence: pomodoro 1.1. C9 made `KeyMsg` carry `phase: 'down' | 'up'` and
deliver both to one sink (`CHANGELOG.md` line 849, `index.d.ts:30`). A bare
version bump type-checked, ran, and toggled every shortcut back: Space
started and paused the timer, `m` and `a` flipped twice. Seven headless
assertions failed only because that test releases keys; a test that only
presses passes. The fix is one guard (`phase !== 'down' || repeat`), and
nothing — not `tsc`, not a warning, not a test that never calls `keyUp` —
points at it.

C9's decision stands on a real guarantee (a key only comes up where it went
down, which split sinks cannot offer) and this is not a request to reverse
it. It is that press-only is the dominant case and it became the case that
needs a guard.

**Do, one of:** (a) a `keyUp` flag on the sink — `onKey` alone hears
presses, `onKey` + `keyUp` hears both, the payload shape unchanged, the
guarantee kept for the sinks that opt in; or (b) keep the default and make
the note impossible to miss: a **Migrating from alpha.4** section at the top
of `packages/kui/README.md` and of the alpha.6 CHANGELOG entry, leading with
this one. No warning can catch it (a sink that ignores `phase` looks like a
sink that wants both). (a) is the honest fix; (b) is the fallback if C9's
"one sink, both phases" is to stay the only shape. Either way `test.mjs`
gets a keymap fixture that presses *and releases* and asserts one toggle.

### `!` F4 — A modal's focus restore beats the closing frame's own `keyFocus` edge

Evidence: mind-map #3. A rename editor opened on a freshly created node: the
node was created and the modal opened in one frame, so the focus the modal
"displaced" was the node the user was on *before*, and dismissing the
editor put focus back there — the next Enter added a sibling in the wrong
place. The app's fix was to stop trusting the key sink's tag for selection,
at the cost of the ring drifting from the selection, because nothing could
put focus where it belonged: `focus()` needs a key the app does not have
(F5), and `keyFocus` on the new node had already spent its edge.

Verified: `set_key_focus` edges are consumed during the build;
`resolve_modal_focus` (`runtime/focus.rs:151`) runs in `finish_frame` after
it (`runtime/emit.rs:250`) and calls `set_focus(saved)` unconditionally, so
a `keyFocus` edge in the frame that drops the modal is overwritten by the
restore. The comment two lines down already grants the exception to
`pending_focus_step` ("wins over both the modal's own focus move and a
same-frame `set_focus`"); the declarative edge is the one path without it.

**Do:** the restore yields to a same-frame `keyFocus` edge — if this frame's
`declared_focus` differs from last frame's, the edge stands and the saved
focus is dropped. That is `initialFocus`'s missing half: the app says where
focus lands on the way out by declaring it, and no new row is needed.
**Test:** kui-core: frame N declares node A focused and opens a modal; frame
N+1 drops the modal and declares `keyFocus` on new node B; `focus() == B`.
And the ADR 0003 corpus scene `modal` gains the step — a report keeps one
frame and this is a two-frame fact (A1 says how those are pinned).

### `~` F6 — Headless key input is two channels, and a window drives both

Evidence: mind-map #4, pomodoro 2.4. `ctx.keyDown(code)` reaches `onKey`
sinks and never dismissed a modal; `app.key(name)` drives the core (Escape
dismisses, Tab traverses, the arrows nudge a slider) and never reaches a
sink. Six of the mind map's first-run failures were `keyDown('escape')`
leaving an editor open and everything after typing into it. The pomodoro's
"arrows do nothing on a focused slider" is the same split from the other
side: the arrows are in (`runtime/dispatch.rs:119-126`), on the `Key`
channel, and `keyDown('right')` is the sink channel — with F7 explaining why
a control in that app never held focus in the first place. A real press does
both, in order: `crates/kui/src/lib.rs:1045-1058` dispatches `KeyDown`, then
lines 1136-1150 the `EditKey` for a named key (or `Text(" ")` for Space).
The `focused()` doc says Tab is `key("tab")`, which is the one place the
split is written down, and it is on a method nobody reads to learn how to
press a key.

**Do:** `press(code, mods)` / `release(code, mods)` on `Ctx` and `App` that
do what the window does — `keyDown`, then the edit-key mapping for the named
keys the runner maps (lines 1136-1150 are the table, and should become one
shared function rather than a second copy), then `text` for a printable,
and `keyUp` on release — with `key()` / `keyDown()` staying as the halves
for precise tests and documented as halves. Lua's and C's headless injectors
have the same split; C at least gets `kui_press`. **Test:** `press('escape')`
dismisses a modal *and* reaches a sink under it; `press('tab')` moves focus;
`press('right')` on a focused slider nudges it.

### `~` F8 — Sliders announce as percentages: no `valueText`

Evidence: pomodoro 2.1, confirmed under real VoiceOver: 25 min in [5..60]
reads as "36 percent", and the arithmetic matches for all three sliders.
`schema.rs:870-884` has `valueNow` / `valueMin` / `valueMax` and nothing
else — no `aria-valuetext`. The workaround bakes the reading into the name
(`"FOCUS LENGTH, 25 MINUTES"`), which is the wrong attribute and renames the
control on every nudge.

**Do:** a `valueText` row (`Kind::Str`, meaningful on the slider role)
carried on `AccessNode` and set on the AccessKit node as its string value —
the attribute the ARIA mapping uses for `aria-valuetext`. Check what
`accesskit_macos` 0.27 makes of a slider with both a numeric and a string
value before choosing where it goes (`kui-ax-repro` is the VoiceOver-free
harness). ADR 0008's rule applies: a nudge announces the text, not the
number. **Test:** the AX audit script asserts the string on the slider; the
corpus pins the row across transports; `docs/props.md` regenerates.

### `.` F9 — `AccessMsg` is the one core message without a type parameter

Evidence: pomodoro 2.2. `index.d.ts:176`: `interface AccessMsg { … tag?:
unknown }` while `DragMsg<T = AppMsg>` and `KeyMsg<T>` carry the app's
union; handling a nudge needs `p.tag as PomoMsg` in a library whose pitch is
one union and no casts. `CoreMsg` already lists it (line 158), so half of
the report's ask is done. **Do:** `AccessMsg<T = AppMsg>`, `tag?: T`. It is
in the hand-written half of the file (P5), so no generator change. The
second half of this entry — the `access()` doc saying the key is hex and
naming the other spelling — closed with F5 (2026-09-06): the doc on the
generated method names both, and the label spelling works.

### `.` F10 — A slider's declared range is never checked against its value

Evidence: pomodoro 2.3: `valueNow={999} valueMin={0} valueMax={10}` is
advertised verbatim and warns nothing; the app clamps in its own `update`,
so the range lives in two places with nothing tying them, and the drift is
visible only to a screen-reader user. **Do:** `slider-value-out-of-range` in
`diag.rs`'s tree walk (one more constant in S2's single list, checked beside
`image-without-label`), also firing when `min > max`. **Test:** the diag
tests gain the three cases.

### `~` F11 — `init` and `view` cannot reach the surface

Evidence: mind-map #7 and #11. `LoopConfig.init` is `M | (() => M)` and
`view` is `(model, window)` (`index.d.ts:1445-1453`), so `measureText` and
`win.size()` are reachable only from `setup` and `update`, and the README's
own suggestion for `measureText` — size a column to its widest label —
cannot be done in a view. The app parks the surface in a module-level
variable from `setup` (the README sanctions the shape for resource ids),
builds its first model against a hardcoded window size, and corrects it on
the first `resize` — with the camera and every hit test off until then.
**Do:** the function form of `init` takes the surface (`init: M | ((surface:
S) => M)`) and `view` gets it third (`view(model, window, surface)`); both
additive, both typed by the `S` that `LoopConfig` already carries. Lua's
`view(env)` already has measurement on `env`; C hosts own their loop.
**Test:** `test.mjs`: an `init` that reads `ctx.size()` and a `view` that
sizes a column from `measureText`.

### `~` F12 — There is no line — **done (2026-09-06)**

**Done as `docs/adr/0010-a-segment-primitive.md`, accepted and built.**
`QuadKind::Segment` (an SDF capsule in the über-pipeline, endpoints in the
`uv` slot, width in `border_w`, `Quad` still 124 bytes); the `line` element
in all four bindings (`<line from to width color/>`, `<line points curve/>`,
`line { … }`, `kui_line` / `kui_polyline`, `ui.line` / `ui.polyline`);
polylines and curves flattened in the core; the `lines` corpus scene; the
`frame_10k_segments` and `frame_1k_curves` benches (a 10k-segment frame is
~8% over the 10k-rect frame with the same quad count, the plain frame is
unchanged); `examples/rust/connectors.rs` as the mind-map shape. What it
settled that the entry asked about: a line is **always a float** positioned
by its endpoints in the parent's box space and `on_layout` reports its
bounding box; it takes **no input**, and rounded hit-testing is **not**
settled — the ADR names the one change (a shape on `HitRegion`) that would
settle both, and leaves it for a view that clicks a connector. Declined or
deferred with reasons: paths, fills, dashes, arrowheads, a tweening width.
For the mind-map author: `examples/rust/connectors.rs` is the three-box
connector as one curve, and the CHANGELOG's "what you can delete" names the
rest. The original entry, for the record:

Evidence: mind-map #6. Every connector is three thin boxes (a stub, a
vertical run, a stub), which works for orthogonal elbows and is the ceiling:
no diagonal, no curve, three nodes per link, and a stub two children share
is drawn twice in two colours. ADR 0005 declined gradients and did not
consider a stroke; `QuadKind` (`display.rs:10`) is six rounded-rect
variants. **Do:** an ADR for a segment primitive: `QuadKind::Segment` — the
two endpoints in the slot `uv` uses, a width, round caps, an SDF capsule in
the über-pipeline the way `Shadow` reused the rounded-rect SDF, one quad per
segment and still one draw call; a `<line from to width color>` element that
emits it; and polylines and curves as flattened segment runs in the core (a
`points` list, a `curve` flag) so the app never sees the flattening. What it
is not: arbitrary paths, fills, dashes. Rounded hit-testing (Status / next)
is the same shape of question and the ADR should settle it or say it does
not.

### `!` F13 — VoiceOver says "node is not responding" at every windowed Node launch (undiagnosed)

Evidence: pomodoro §3, reproduced at every launch; it recovers and reads the
tree correctly afterwards. The report measured out blocking `setup` work
(0.4 ms), an expensive tree (0.7 ms) and tree churn (only `hash` changes)
and left one lead: the window shows before any tree exists. Checked against
`crates/kui/src/lib.rs`, the lead is half wrong. `push_pane` (line 829)
creates the AccessKit bridge and shows the window *after* the surface and
renderer exist (the window is built `with_visible(false)`, line 879), so GPU
setup is not in the gap. What is: `KuiWindow`'s constructor
(`kui-node/src/lib.rs:872`) opens and shows synchronously, JS then runs
`setup`, `init` and the first `render` before the first `pump`, and the
loop is pumped from an 8 ms `setTimeout` for the life of the process — every
AX request waits for the JS thread to yield. And `InitialTreeRequested`
(`access_bridge.rs:101`) only flips `active`; the tree goes out on the next
`publish_access`, which is the next frame.

**Do, in order:** (1) instrument — stamp `new KuiWindow` returning, the
first `pump` and the first `publish_access`, and run under VoiceOver; the
AX-audit script (`kui-ax-repro`) is the harness if it can observe the
timeout, and that is the first question. (2) If the tree is the gap: answer
`InitialTreeRequested` with the current tree (the core has one after the
first frame) instead of waiting for a redraw. (3) If the JS gap is it:
`runWindowed` shows the window from the first pump rather than the
constructor (`kui-macos-window-quirks`: `set_visible(true)` is
`makeKeyAndOrderFront`, and a just-shown surface reports Occluded once).
(4) The process is an unbundled `node`, which is why it is *named* "node";
probably cosmetic, rule it out last. Not a headless test; a P8 smoke item
once a runner exists.

### `~` F15 — Panning does not work in the window (unreproduced)

Evidence: mind-map "Still open". A canvas `onDrag` pans headlessly (there is
a test) and not in the window; the pan reads only absolute `x`/`y`, so F2 is
not the cause. The report's next check is the right one: hover the empty
canvas and read the cursor — `grab` means the canvas has the press and the
fault is downstream; `default` means the press lands somewhere else. Two
things this repo knows that the report does not: an undecorated window gets
no `mouseUp` (`kui-macos-window-quirks`, W1) — if that app runs `chrome:
'borderless'`, that is it — and the same note has the recipe for driving a
real window by hand. **Do:** reproduce first, with a Node example shaped
like the app (float-positioned children under an `onDrag` root, panned by
the drag); if it reproduces, it is a `!` and goes above F1.

---

## After alpha.6

Grouped by kind, not urgency. Nothing here blocks the tag.

**Build.** ADR 0004 step 1's leftover: the glyph atlas and shape cache
are still per window because `Core::output` hands out `&mut GlyphAtlas`
(see `kui-session-atlas-constraint`); it waits for a case where two windows
share enough text to matter. C15's remainder: `frame_10k_rects` is still
~1.5× its 2026-08-31 cost after the boxing fix, and that half is the per-node
logic rather than the struct — it wants a profile now the cache behaviour has
changed. Plus the CI threshold on `frame_10k_rects` and `frame_1k_typical`,
which the size test does not replace.

**From the field (F1–F15).** Of the four defects, three are done:
~~F1~~ (the Node loop never set the frame clock, which is why nothing eased
was testable from Node), ~~F2~~ (drag deltas) and ~~F3~~ (an alpha.4 keymap
ran twice, closed as its (a), the `keyUp` flag) all landed **2026-09-06**,
and both of the last two carry a "what breaks" line in the CHANGELOG for the
next tag. **F4** (the modal restore overriding a `keyFocus` edge) is the one
left. Then the gaps in rough order of cost: F9 and F10 are an afternoon, F6
and F11 a day each (~~F5~~ was one, and is **done (2026-09-06)**), F8
needs one AccessKit question answered first. ~~F7~~ (global shortcuts under
a Tab ring) went to the ADR group below and landed there as
`docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`, as ~~F12~~ (a line
primitive) had as `docs/adr/0010-a-segment-primitive.md`. F13
(VoiceOver at launch) and F15 (panning in a window) are reports nobody in
this repo has reproduced yet, and each says what to try first. ~~F14~~ —
**done (2026-09-06)**: F2 closed its `DragMsg` bullet and the other three
sentences are written, so the whole entry is in the archive.

**Design, each wanting an ADR.** ~~Live regions and announcements~~ —
**done (2026-09-06)**, as `docs/adr/0008-live-regions-and-announcements.md`,
accepted *and* built. The split it settles: the sustained half is a `live`
row on the node whose text changes, the one-off half is `Core::announce`
queued and drained like window commands, audio commands and warnings —
because a message with a place on screen is a property of the view and one
without is a consequence of an event. Two things the ADR only learned by
running `scripts/ax-audit.swift` against the OS, both now written into it:
a live `Role::Label` announces nothing on macOS (the adapter reads its
*value*, not its label), which is why a live region reads as one named
message rather than as the ARIA-shaped container whose descendant changed;
and an announcement needs a **fresh** AccessKit node id each time or the
same message twice in a row is silently swallowed. Left open by it, and
not yet worth a row: `aria-atomic`, and asking AccessKit upstream for a
real announcement in `TreeUpdate` (two of the three platforms have the API
behind it; AccessKit's whole event surface is a tree diff). The exit
animations' `animating()` policy, revisited against a real view that removes
many nodes (ADR 0005 left it opt-in + a 512-node budget with no duration
cap). ~~And from the field, F7, whether keys a focused control did not
take should bubble to the enclosing sink — ADR 0002 rejected the narrower
"sink gives up Tab" and both reports need the wider thing~~ — **done
(2026-09-06)**, as `docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`,
accepted *and* built: they bubble, a control keeps only the keys the core
presses it with, Tab stays the ring's, and ADR 0002 decision 3 keeps
everything a sink that holds focus hears. ~~And F12, a
segment primitive beside the six rounded-rect quad kinds, which ADR 0005
never considered~~ — **done (2026-09-06)**, as
`docs/adr/0010-a-segment-primitive.md`, accepted *and* built: a seventh
quad kind that is an SDF capsule, the `line` element in four bindings,
curves flattened in the core, a corpus scene and two benches. The one
thing it declined to settle is rounded hit-testing, which stays parked
below with the change that would settle it named.

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

**Hygiene.** **Archiving the closed entries — done (2026-09-06).** Forty-six
of them moved verbatim into
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md); what stayed here is
the five open headings, this section and the index below. The suggested
sequence went with them rather than staying: three of its four steps had
shipped, and this section is what says what is next. Still open: enable
`SMOKE_MACOS` / `SMOKE_WINDOWS` the day a runner exists (P8) and remove Lua
`env.focus` at 0.2 (P3, R7); F14's doc sentences are written (2026-09-06).

---

## From building C11 step 4 (2026-09-06)

Two findings, neither about the popup itself. Both are decided and in the
archive; this is what W2 left to build.

### `~` W2 — Press-drag-release does not reach a popup, and only the driver can make it — **accepted, unbuilt (2026-09-06)**

The whole entry, with its measurement and the outcome on top, is in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md#-w2--press-drag-release-does-not-reach-a-popup-and-only-the-driver-can-make-it--accepted-unbuilt-2026-09-06).
`docs/adr/0009-press-drag-release-into-a-popup.md` settled the four
questions it raised; what shipped with it is the arithmetic only
(`crates/kui/src/retarget.rs`, decision 7, with its tests). **Still to
build**, all in `crates/kui/src/lib.rs` and none of it headlessly testable:

- `Shell` arms a non-activating popup that opens while the owner's primary
  button is down (decision 1), retargets the owner's `CursorMoved`s into it
  (decision 2), classifies the release with `retarget::landing` — synthesise
  the press-and-release, keep the menu, or dismiss (decision 4) — and
  disarms in `close_pane`.
- The press that dismisses a non-activating popup is consumed rather than
  dispatched (decision 5). This is the one visible change for existing
  apps, and the release that carries it says so under "what you can delete".
- `examples/rust/popup.rs` opens on `on_drag`'s `start` with
  `cursor: "pointer"`, handlers set rather than toggle (decision 6), and the
  four `CGEvent` checks the ADR's consequences list are run on macOS.

---

## Closed — index

Fifty-one entries, all in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md) and all verbatim.
This index is here so an id resolves without opening that file: the open items
above cite A1, C7, C9, C10, D2, P3, P5, P8, R3 and S2, "After alpha.6" and the hygiene note cite C2,
C5(b), P3 and R7, and code comments, ADRs and commit messages cite ids of
their own. Forty-nine of these are simply closed; **C15** and **W2** appear in
both files, whole there and trimmed to what is still open here. **C11** was a
third, until its last step landed on 2026-09-06 and took the whole entry to
the archive.

The archive also holds two sections that are records rather than work: the
suggested sequence as it stood on 2026-09-05, and
[Release 0.1.0-alpha.6](backlog/closed-2026-09.md#release-010-alpha6-2026-09-05),
whose R1–R7 are the half-baked items finished before the tag.

**From two field reports (2026-09-06)** — F2, F5, F7, F14

- `!` **F2** — [Drag deltas lie twice: `end` zeroes them, and sub-slop motion never reaches them](backlog/closed-2026-09.md#-f2--drag-deltas-lie-twice-end-zeroes-them-and-sub-slop-motion-never-reaches-them--done-2026-09-06) — done (2026-09-06) — F14's `DragMsg` bullet closed with it
- `~` **F5** — [Nothing outside Rust can name a node by the key it declared](backlog/closed-2026-09.md#-f5--nothing-outside-rust-can-name-a-node-by-the-key-it-declared--done-2026-09-06) — done (2026-09-06)
- `.` **F14** — [Four doc gaps the two reports paid for](backlog/closed-2026-09.md#-f14--four-doc-gaps-the-two-reports-paid-for-three-of-them-open--done-2026-09-06) — done (2026-09-06) — its `DragMsg` bullet closed with F2
- `~` **F7** — [An app with global shortcuts cannot also have a Tab ring](backlog/closed-2026-09.md#-f7--an-app-with-global-shortcuts-cannot-also-have-a-tab-ring-wants-an-adr--done-2026-09-06) — done (2026-09-06) — ADR 0011, accepted and built

**From building C11 step 4 (2026-09-06)** — W1

- `!` **W1** — [A `Chrome::Borderless` window is dead to the mouse on macOS](backlog/closed-2026-09.md#-w1--a-chromeborderless-window-is-dead-to-the-mouse-on-macos--done-2026-09-06) — done (2026-09-06)
- `~` **W2** — [Press-drag-release does not reach a popup, and only the driver can make it](backlog/closed-2026-09.md#-w2--press-drag-release-does-not-reach-a-popup-and-only-the-driver-can-make-it--accepted-unbuilt-2026-09-06) — accepted, unbuilt (2026-09-06) — ADR 0009; the arithmetic shipped, the driver half is the entry above

**Binding parity** — P1–P9

- `!` **P1** — [Add a `description` prop row](backlog/closed-2026-09.md#-p1--add-a-description-prop-row--partly-done-2026-09-03) — partly done (2026-09-03) — the `description` `PROPS` row is still unbuilt, and the entry says so
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
- `~` **C15** — [`NodeSpec` was 728 bytes and the frame got ~2.5× more expensive](backlog/closed-2026-09.md#-c15--nodespec-was-728-bytes-and-the-frame-got-25-more-expensive--fixed-2026-09-05-two-thirds-recovered) — fixed (2026-09-05), ~two-thirds recovered — the measurement record — the profile, the bisect and the fix; the remainder is above
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
