# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
the six rounds that followed it, and the field reports from two apps built on
alpha.6, alpha.7 and alpha.8 outside this repo (F1–F15 on 2026-09-06,
F16–F23 and F24–F30 on 2026-09-07). Every item names the
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
F24–F30 (2026-09-07), and what comes next; F16–F23 from the two alpha.7
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
map's "there is no shorter way round" (there is none today, and F24 is
it), and the pomodoro's "a window has no equivalent" of `runOut` (the
primitive is there; the promise is not). One entry, F25, is not in either
report: it fell out of checking F24.

### `~` F24 — `setEditText` before the editor exists does nothing, and alpha.8's deletion list told an app to delete a latch it still needs

The mind map deleted its `onLayout` latch on the strength of alpha.8's
F20 entry — **What you can delete:** "the `onLayout` latch … and the
`initial`-does-not-reset surprise on reopen — write the model's draft with
`setEditText` when the editor opens" — and three checks went red: a
reopened rename showed the abandoned draft, with the caret at its end.
The report's reading is right. The suggested replacement *is* the latch
minus the caret as its reason, and the line under `### Changed`
("`initial` seeds a new editor only … `setEditText` is what resets one")
is the accurate one. The app then tried the shorter path — reset from
`beginEdit`, in `update`, where the model already knows the text — and
found `setEditText` a silent no-op there: "seed, type, close, reopen, and
the abandoned draft is still there."

The repo's line: `EditStore::set_text` (`edit.rs:468`) is `if let Some(s)
= self.states.get_mut(&key)` and nothing else. An undeclared key falls
through with no warning, in every binding, and the editor a rename opens
does not exist until the frame after `beginEdit` builds it
(`builder.rs:447` is the only `declare`). So the reset an app wants to
make at the moment it decides to open an editor has no door: `initial`
will not (by design, and a test pins it), `setEditText` cannot yet, and
`onLayout` is the first moment it can — which is the latch.

**Do:** four things, smallest first. (1) The `<edit>` row and
`setEditText`'s doc say the call reaches a declared editor only. (2)
`set_edit_text` on a key with no state raises a warning —
`unknown-editor`, say, beside `duplicate-key` — the shape F16 chose: a
silent no-op under an unchanged signature is the failure mode both
reports rank worst. (3) Make the call from `update` mean something: a
text set for an undeclared key is *pending* and seeds the editor the next
frame declares under that key, beating `initial`; a pending seed nobody
declares by the end of that frame is dropped with the warning from (2).
That is the door the report reached for first, it needs no latch and no
`onLayout`, and it keeps `initial`'s meaning intact. Mutation-test it the
way the corpus was: the F20 test that pins "a returning editor keeps its
draft until `set_edit_text` resets it" must still pass, and a new one
sets the text before the declare. (4) alpha.9's entry corrects the
alpha.8 deletion line — the changelog is shipped and stays; the
correction goes on top. A `~`: the workaround exists and is the latch,
and the report's three checks are what it takes to know the latch is
still needed.

### `~` F25 — Editor and scroll state are kept by key forever — **done (2026-09-07)**

Done: both stores cap the states nobody declares. A declared state is
never evicted — retention across absence is the promise, so this is a
ceiling and not a prune — and past the budget the longest-undeclared
entry goes first, oldest by the frame it was last declared in and by key
inside a frame, so the same view evicts the same states whatever order
the map iterated. `EditState` and the scroll `Entry` each carry a
`last_declared` stamp, written by `declare` / `resolve` (and by
`set` / `scroll_by`, since an offset written between two frames is
usually aimed at the key the next frame builds); `Core::begin_frame`
hands both stores the frame number it just took, and "declared" means
declared by the frame that just ended. The focused editor and the one
being drag-selected are skipped by name as well, since the store still
points at them. Budgets: `MAX_UNDECLARED_EDITS` = 256,
`MAX_UNDECLARED_SCROLLS` = 1024, both public.

The numbers, measured rather than guessed — a counting global allocator
wrapped around a `Core` declaring 200 editors at a time, live bytes
before and after: **3.4 KB** for a fresh empty editor, **4.9 KB** holding
`"hello"`, **22 KB** holding a 39-character line (the shaped glyphs are
most of it), and nothing at all freed when the keys stopped being
declared, which is the finding. 256 of the worst of those is ~5.6 MB and
~1.2 MB at a short field: a bound an app can afford, and one no ordinary
view comes near — 256 editors no longer on screen is already an app
generating keys, which is the idiom this makes safe. A scroll `Entry` is
56 bytes, 64 with its key in the map (`size_of` at the time of writing),
so a full 1024 is ~64 KB — four times the count for a fortieth of the
memory, which is why the two budgets differ.

What it costs a frame that is nowhere near the budget: one length
comparison per store. A store inside its budget never walks itself,
which is why this can run every frame rather than every 240th like the
anim store's cutoff sweep. `scripts/bench-check.sh v0.1.0-alpha.8`
agrees: the four guarded rows came out +1.5% / −0.8% / +0.2% / −0.1%
against the tag, worst run-to-run spread on a guarded row 2.6%.

Not done as the entry asked: **the count is not in `FrameStats`**.
`FrameStats` is the driver's timing ring — a `Vec<FrameSample>` a driver
pushes into, reached through the Node binding as the latency HUD's
`{frames, last, avgTotalMs, …}` — and it can see neither store, so a
count there would have to be pushed by whoever calls `push`, on a type
about frame *cost*. The counts are `EditStore::len()` and
`ScrollStore::len()` instead, both `pub` on `pub` fields, and the tests
are core-only: `crates/kui-core/tests/state_budget.rs` declares budget +
50 of each, then declares one of them for a hundred frames, and pins
that the store settles at exactly budget + 1, that the declared one
survives with the text it was given, that a second wave evicts the first
wave's leftovers rather than growing, and — the promise, in its own two
tests — that an editor and a scroll offset nobody declares for 500
frames come back untouched while the store is inside its budget. One
sentence went into the `<edit>` row, the overflow row and the `key` row.

Found while checking F24. The idiom the retained-by-key model implies — a
fresh key per opening (`edit-${id}-${session}`), so `initial` seeds a
fresh editor every time, the way React remounts on a changed key — would
have answered the mind map without any of F24. It works. It also leaks:
`EditStore::states` (`edit.rs:309`) is an `FxHashMap<Key, EditState>` that
only ever grows — `declare` is `entry(key).or_insert_with`, and no path in
the runtime removes an entry. Each state owns a `cosmic_text::Editor` with
a shaped `Buffer`. `ScrollStore::entries` (`scroll.rs:56`) has the same
shape and the same absence. The anim store is the one that does it right:
`AnimStore` retains tweens by `last_used >= cutoff` (`anim.rs:282`).

Today's cost is bounded by the number of distinct keys an app ever
declares — the mind map's `edit-<id>` is one per node ever renamed, the
pomodoro has none — so no shipped app has felt it. The idiom that would
make it unbounded is the one the docs point at, and nothing outside the
core can drop a state: `setEditText(key, "")` shrinks the buffer and keeps
the entry.

**Do:** decide the lifetime, once, for both stores. Retention across
absence is a documented promise (a tabbed form that undeclares a field and
returns to it expects its draft; the F20 test pins it), so prune-on-
undeclare is the wrong fix. The right one is the exit store's: a budget on
*undeclared* states, the longest-undeclared evicted first, sized so no
ordinary view meets it (256 editors and 1024 scroll entries would be
generous), and a count in `stats()` so a test can see it. A declared state
is never evicted. One sentence in both rows: "kept while declared; an
undeclared one is kept until the budget needs the room". A `~` because
nothing can free it from outside; a `!` the day an app opens editors under
generated keys.

### `~` F26 — There is no supported way to learn a release exists, and every range an app writes floats

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

### `.` F27 — `quads()` on a window does not say how to drive one

Pomodoro wish 3 and finding 4. `win.quads()` landed (F19) so a smoke test
could read the frame the shipping driver painted; the first thing that
test reaches for is `app.click(x, y)`, and a window refuses it. Both
halves are documented on their own methods — `click`'s says "a window is
driven by the OS and says so rather than pretending", `access`'s says
"Works against a real window too" — and neither is reachable from
`quads()`'s doc (`index.d.ts:1034`, generated from the `core_methods!`
macro in `kui-node/src/lib.rs`), "which is the one a person reads when
they sit down to write this exact test". The app found `access` "by
working out that `click` would not do", and it reads as an accessibility
helper rather than as the one synthetic input a window takes.

**Do:** one clause on `quads()` — "drive a window with `access(key,
action)`; `click`, `type` and `key` are refused there" — and one on
`access` saying it is the way into a window, not only a screen reader's
path. Rebuild the addon before `npm run gen` (the doc lives in the Rust
source), and the package README's window example gets the same sentence.
Their `smoke.tsx` is the test that wanted it.

### `~` F28 — An `<audio>` one-shot that must finish has to guess its own length

Pomodoro wish 2 and finding 5. Presence is playback: `<audio key src>`
present is playing, gone is stopped (`AudioStore::reconcile`,
`audio.rs:374-383` — every mounted key not declared this frame is
`stop`ped at once). Correct and small, and the report says so. What it
forces on a one-shot: the view decides how long the node stays declared,
and it does not know how long the asset is, so the chime became `m.now -
m.alarmAt < CHIME_MS` with `CHIME_MS = 6_000` "picked by guessing at the
asset's length" — too short cuts the sound, too long replays it on an
unrelated re-declare, and nothing checks it.

Checked: the core does already know when a playback ends. A `tag` brings
`{kind:"sound", phase:"ended"}` back (`audio.rs:283`), the Rust runner
reports it from the device (`kui/src/lib.rs:1503`), and headless
`ctx.audioEnded(playback)` stands in for the device. So the constant has
a replacement today — keep the node declared until the `ended` message
clears a model flag — which is real state rather than a guess, and the
row should say so. It is still a model field and a message arm for "play
this once, whole", which is the cost the report is describing.

**Do:** a prop on the `audio` element that changes only what *gone*
means: the node's removal releases the playback instead of stopping it,
and it finishes on its own — a looped one still stops, since release is
meaningless for it, and `ended` still fires for a tagged one. Not
`oneShot`: the row already uses "one-shot" for a non-looped playback, so
the flag needs a word for the *release* — `finish` (`<audio src finish/>`,
`finish = true`, `KuiAudio.finish`) is the candidate; the build can pick a
better one. Rows: `schema.rs:1286`'s `jsx_own`/`lua_own`, `AudioSpec`, the
encoder's `audio` op, a field appended to `KuiAudio` (host-allocated and
read by pointer, so no ABI bump per `abi.rs:17`), `props.md`, and the
corpus `resources` scene's audio phase gains a node removed with the flag
whose command list shows no `stop`. **What the app can delete:**
`CHIME_MS` and the `now - alarmAt` window; the node is declared for one
frame and the sound plays whole.

### `~` F29 — A window has no settled frame by name, so a smoke test sleeps

Pomodoro wish 4: `runOut` gives headless the settled frame; "a window has
no equivalent, so `smoke.tsx` sleeps 400 ms and hopes" (`frames(400)`,
then two `frames(300)` — wall-clock `setTimeout`s around each `access`
press). Half true. `animating()` is in the shared surface, so a window
already answers it after every pump — the primitive is there, and a test
could poll it. What is missing is the thing that ties it to the driver:
`runWindowed`'s pump (`index.js:410-431`) is a `setTimeout` loop the test
cannot see into, so the only way to know a frame has been painted, let
alone a settled one, is to guess a duration.

**Do:** on the windowed loop (`Loop`, not `App`), `settled(maxMs =
10_000): Promise<number>` — resolves from inside the pump the first time
a `win.pump()` + `app.step()` leaves `animating()` false and nothing
queued, with the wall-clock milliseconds it took, and resolves at the cap
with `animating()` still true, as `runOut` does. Not `advance`: the
window's clock is the wall's and a test cannot move it, which is why this
is a promise and `runOut` is a loop. The same hook gives `frame():
Promise<void>` — "one more pump has painted" — nearly free, which is what
the first `frames(400)` was waiting for. Their `smoke.tsx` is the test to
convert; its three sleeps become three awaits.

### `.` F30 — Three doc shapes the reports paid for

Neither report found a wrong sentence in the docs this round. What they
found is that the right sentences are not where a reader stands.

- **A task index beside `props.md`.** The pomodoro filed an alpha.7 wish
  for a feature that had shipped in alpha.7 — `<audio key src>` and
  `audioCommands()` were in its own `node_modules`, at
  `jsx-runtime.d.ts:399` and `props.md:123` — and F23's entry answered
  it. Its diagnosis is the right one: `props.md` is a 200-row table sorted
  by name, and nothing is organised by the question a developer arrives
  with. **Do:** a `docs/howto.md` — "play a sound when the model changes",
  "test a real window", "animate a removal", "reset an editor", "the
  settled frame", "pin a version" — each two sentences and a link into
  `props.md`, the changelog entry or the ADR. The changelog's "what you
  can delete" framing is the model; twenty entries would cover both apps.
- **`What breaks` as a checklist.** 3 050 lines, ~30 KB for alpha.8
  alone; "as a document it is worth reading start to finish. As the thing
  you consult at 4 p.m. with a build to fix, it is dense". The `**What
  breaks.**` opener is four paragraphs of argument. **Do:** keep them, and
  put one bullet per break with the symbol names above them, so a reader
  can grep — from alpha.9's entry on, not retrofitted.
- **A deletion line names a symptom, not a workaround.** The mind map's
  lesson, twice now: "a workaround that accreted two purposes only sheds
  the one the release addressed", and the deletion list "is written from
  the library's side, where the workaround has one purpose". **Do:** a
  "what you can delete" line names the *behaviour* the release removed
  the need for, and leaves the app to say which of its lines that was;
  F24's correction is the first instance.

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

## After alpha.8

Grouped by kind, not urgency. Nothing here blocks the tag. It was "After
alpha.7" until 2026-09-07, when the two items it named to build — ADR
0012's remainder and ADR 0013 — both landed for alpha.8 and the heading
moved with the tag; "After alpha.6" before it went to
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
The next thing to build is what the next field reports asked for, the same
day: F24–F30 above — F24 and F28 are the two that touch the core, F25 is
the lifetime question they turned up and is **done (2026-09-07)**, F26 is
the publish step, and F27,
F29 and F30 are a doc clause, a promise on the windowed loop and a task
index.

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

**Hygiene.** Archiving is done three times over: the forty-six of
2026-09-06, the ten field-report entries that followed them before the tag —
so all of F1–F15 sit together — and W2 whole on 2026-09-07, once its driver
half was built. This file is three open headings and this section.
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
- The same gesture into a popup: press in the owner, drag into the popup,
  release on an item. ADR 0009's consequences name four `CGEvent` checks and
  W2's archived entry records what each one showed when the driver half was
  built (2026-09-07). Windows adds a fifth nothing has run — mixed DPI across
  two monitors, which is what the arithmetic is in physical pixels for.

---

## Closed — index

Seventy-one entries, all in
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
the round that watched ADR 0012 at its boundary.

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
