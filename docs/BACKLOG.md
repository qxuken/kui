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
report: it fell out of checking F25.

### `~` F25 — `setEditText` before the editor exists does nothing, and alpha.8's deletion list told an app to delete a latch it still needs — **done (2026-09-07)**

Done: a text set for a key nothing has declared is held as a pending seed
and seeds the editor the next frame declares under that key, over its
`initial`; the caret lands at the end, document or not, because a held
call is that call arriving where it can land and not a second kind of
`initial`. Held for that one frame only — a seed nobody claims by the end
of it is dropped (`layout_frame` drains what is left after the build,
whatever `diag.enabled` says) and raises the new `edit-text-without-editor`
warning, declared in `diag.rs` like every other code, so `docs/props.md`
and `index.d.ts`'s `WarningCode` union carry it from the one table. The
`<edit>` row, `Core::set_edit_text`, `kui_edit_set_text` and Node's
`setEditText` all say what the call reaches now. Three tests in
`crates/kui-core/tests/editing.rs` pin it — the seed lands over `initial`
with the caret at its end, a seeded *document* opens at its end too (which
`initial` does not), and an unclaimed seed is one warning and no text —
beside F20's `a_returning_editor_keeps_its_draft`, which stays green:
`initial` still never reseeds a returning editor. Removing the consume in
`EditStore::declare` fails two of the three. Declined: keeping the seed
indefinitely, which would make a typo'd key a field that opens with the
wrong text at some later frame instead of a warning now. The alpha.9
CHANGELOG entry carries the correction to alpha.8's deletion line.


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

### `~` F26 — Editor and scroll state are kept by key forever — **done (2026-09-07)**

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

Found while checking F25. The idiom the retained-by-key model implies — a
fresh key per opening (`edit-${id}-${session}`), so `initial` seeds a
fresh editor every time, the way React remounts on a changed key — would
have answered the mind map without any of F25. It works. It also leaks:
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

### `.` F28 — `quads()` on a window does not say how to drive one — done (2026-09-07)

**Done (2026-09-07), as the entry asks — three doc sites, no code.**
`quads()`'s doc in the `core_methods!` macro
(`kui-node/src/lib.rs`) gained the clause, so it renders onto both `Ctx`
and `KuiWindow` in `index.d.ts` from the one source: drive that window
with `access(key, action)`, and `click`, `type` and `key` are refused
there because the OS is what drives a real window. `access`'s own doc, in
the hand-written `Loop` interface, no longer stops at "Works against a
real window too" — it says that against a window it is not only the
screen reader's path but the *only* synthetic input one takes, and names
`win.quads()` as the test that wants it. The README's window example says
the same where it shows `runWindowed`, in the paragraph on what actually
differs between the two drivers. So all three of the places a person
lands — the generated method doc, the method they would find next, and
the page they read before writing either — now point at `access`, and the
app's "worked out that `click` would not do" is a sentence instead.

Rebuilt the addon before `npm run gen` (the doc lives in the Rust source,
and the type defs are appended to, so `target/napi-type-defs` was cleared
first — `gen` re-runs the build itself when it finds none). `npm test` in
`packages/kui`: 82 pass, 0 fail. The generated region grew only these
lines, on the two classes; the member count is unchanged at 126.

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

### `~` F29 — An `<audio>` one-shot that must finish has to guess its own length — **done (2026-09-07)**

Done, and named `finish` as the entry proposed. `AudioSpec::finish()` /
`<audio finish/>` / `finish = true` / `KuiAudio.finish` changes what *gone*
means and nothing else: `reconcile`'s departure branch removes the key from
`mounted` without queueing a `Stop`, so the playback runs to its end on the
device. The three edges the entry implied are each a test beside
`stop_cancels_the_ended_event`: a loop is stopped on removal whatever it
asked (release is meaningless without an end), a changed `src` still
restarts (a replacement, not a departure), and a `tag` still reports
`ended` after the release — which a `stop` would have cancelled, and which
is what makes the flag and the workaround the same mechanism rather than
two. One corner the entry did not name and the row now does: a playback
that is `paused` when its node goes has nothing to finish, so pause and
release do not combine.

On the name. `finish` was kept over the two alternatives worth weighing.
`release` names the mechanism rather than what the app wants, and the word
is already spoken for on this surface — a pointer release is in every
drag's vocabulary. `playOut` is the audio-engineering term and the most
precise of the three, but it splits into `play_out` in Lua and C, and it
reads as an instruction where the `audio` element's other props — `loop`,
`paused` — are states. `finish` sits with those, one word in all four
spellings, and says what happens to the playback.

Where it went: `schema.rs`'s `audio` row (both spellings, and the doc
string now names the `ended` workaround as what the flag replaces), the
encoder's flags word (bit 4, mutation-tested), `binary.rs`'s `OP_AUDIO`,
the Lua parser, and a field **appended** to `KuiAudio` — `abi.rs:17`'s
[in] rule holds, a host that predates it writes the shorter struct and
reads the old behaviour out of the zeroed tail, `KUI_AUDIO_INIT` needs no
change because a designated initializer zeroes it, and `abi_parity` settled
the header mirror at build time with no `KUI_ABI_VERSION` bump.

The corpus took more than the entry asked for, and had to: an `audio`
element builds no tree node and the report had no audio column at all, so
"the command list shows no `stop`" was not a thing any binding could be
diffed on. `Output` now carries the drained audio commands and the report
spells one per line (`audio play 1 1`, `audio stop 3`) — the verb, the
playback and a play's `looped` bit, with volumes, fades and the sound
handle left out because none of them compares across four runs. The `media`
scene declares three playbacks and drops two in a second phase: `chime`
asked to finish and leaves nothing behind, `blip` did not and is stopped.
Two things the building corrected: the scene had to declare the dropped
nodes **last**, because Lua's `sequence_values` stops at the first `nil` and
the phase guard would otherwise have swallowed the `latency_graph()` after
it; and the entry's `resources` is the `media` scene.

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

### `~` F30 — A window has no settled frame by name, so a smoke test sleeps — **done (2026-09-07)**

Done, as written. `WindowLoop` — the loop `runWindowed` builds, and only
it — has `settled(maxMs = 10_000): Promise<number>` and
`frame(): Promise<void>`. Both are answered from inside the pump: the
loop keeps a list of waiters and `step()` drains it at the end of every
turn, so `frame` resolves on the next pump and `settled` on the first one
that leaves `animating()` false with no effect unflushed, with `at()`'s
milliseconds since it was asked. The cap resolves rather than throws,
`animating()` still true, as `runOut` returns `maxMs`. `step()`'s body is
wrapped so a throw rejects the waiters before it rethrows, and
`runWindowed`'s own `catch` rejects them too — a throw out of `win.pump()`
happens before `step` — through a module-private symbol rather than a
public method. A window that closes rejects what is still waiting, since
no further frame will be painted. Headless is untouched and keeps
`runOut`; `settled` and `frame` on a loop that holds its own clock throw
and name `runOut`, the way `advance` refuses a wall clock. Four tests in
`packages/kui/test.mjs` drive the pump by hand — `createApp` over a
stand-in surface whose `animating()` the test flips, and a clock the test
holds, which is what `runWindowed` fills with `Date.now` — and cover the
settle, the cap (56 ms of a 50 ms cap, still animating), `frame`, the
rejection, and the headless refusal.

Two things the doing found. **The type is `WindowLoop`, not `Loop`:** the
entry said "the windowed loop (`Loop`, not `App`)", but `App` *extends*
`Loop`, so a method on `Loop` is a method on `App` too. `WindowLoop` was a
bare alias for `Loop<M, A, KuiWindow, E>`; it is now an interface
extending it with these two, which is the mirror of `runOut` living on
`App` alone. A type-level check (three `@ts-expect-error`s) says each half
has what it should and not the other's.

**And the app this was filed for cannot use `settled`.** Converting
`smoke.tsx` (outside this repo, uncommitted): `settled(600)` answered at
its cap — 608, 609, 610, 610 ms over four runs — every time. The window
opens at 1040×720, which is the wide tier, and the wide tier draws the
LCARS cascade, whose two `repeat="alternate"` keyframe legs never stop:
`animating()` is true for the life of that window. This is the case the
cap was written for and it reports it honestly, but the wait that fits
the test is the other one. All three sleeps became `await app.frame()` —
the first `frames(400)` included, which the entry already predicted
`frame()` was really waiting for; the geometry it asserts on is identical
after one pump (333 quads, 72 solid, extends to 1040×720) to what 600 ms
of pumping gave, over three runs each. `npm run smoke` passes with no
failures and finishes about a second sooner, and the `frames` helper is
gone. The comment in its place names `settled` and why that window is the
one that cannot have it.

The original finding follows.

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

### `.` F31 — Three doc shapes the reports paid for — done (2026-09-07)

**Outcome: all three, as written.** (1)
[`docs/howto.md`](howto.md) is the task index: 23 questions in five
groups — draw and animate, interaction, sound and effects, testing,
shipping — each two sentences and a link line into a `props.md` row, the
release entry or the ADR. It is seeded from what the two apps actually
needed (playing a sound, testing a real window, animating a removal,
resetting an editor, the settled frame, pinning a version, a connector,
popup versus modal, effects an app defines, a spoken-only hint, a tab
bar, shortcuts beside a Tab ring, an announcement, naming a node, reading
warnings) and it says what is true today rather than what F25, F27 and
F30 would make true: `setEditText` reaches a declared editor by the hex
key an event carried, so the reset still runs on the first `onLayout`; a
window is polled through `animating()` because there is no `settled()`
yet; and both `^` and `~` float, so an app pins exactly. Every link was
checked mechanically, file and anchor, in both the repository tree and the
packed one — and checking turned up two the prose had wrong: `tooltip` is
a composite row and not a container one, and `editText` / `setEditText` /
`setScroll` take the hex key only, where `focus` / `isFocused` / `reveal`
/ `access` also take a label (`resolve_key` versus `parse_key` in
`kui-node/src/lib.rs`). It is linked from the README's *Reference* section
above `props.md` and from the package README, and `prepack.mjs` copies it
into the tarball beside `CHANGELOG.md` — rewriting its two relative
prefixes on the way, since `docs/howto.md` in the repository and
`howto.md` at the package root do not agree about where the changelog and
the ADRs are. `npm pack --dry-run` lists it at 13.8 kB.

(2) The `**What breaks.**` convention starts at alpha.9, which also
needed the heading: the section was `## Unreleased`, which
`scripts/set-version.sh` does not rewrite (it matches `## <ver>
(unreleased)`), so a tag would have failed `check-version.sh`. It is
`## 0.1.0-alpha.9 (unreleased)` now, opening with five bullets — the two
`Extension` methods, Lua's `on_event` reply, the refused duplicate
namespace, the extension keys that move once, and a row the stock button
does not read — above the three paragraphs that argue them. Nothing older
was retrofitted.

(3) Both conventions are written down at the top of `CHANGELOG.md`: the
bullet list is for the reader with a build to fix and the paragraphs for
the reader deciding whether to upgrade, and a "what you can delete" line
names the *behaviour* the release removed the need for, not the
workaround it guesses an app wrote — a workaround that accreted two
purposes only sheds the one the release addressed.

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
  F25's correction is the first instance.

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

### `!` C16 — The shaped-text cache has a clock and no budget — **done (2026-09-07)**

Done, as (1), (3) and (4) of the "Do" below; (2) declined for now. The
cache charges itself an estimate per entry — `ENTRY_BASE_BYTES` (4 KB)
plus `ENTRY_GLYPH_BYTES` (480) a glyph plus the content — and
`evict_to_budget` runs at the end of every `begin_frame`: inside the
budget it is one comparison, past it the entries not drawn by the frame
that just finished go in (last-used, key) order until the cache is under
three quarters of the budget, so a stream walks the cache once per
quarter-budget of new text rather than every frame. The estimate's
constants were measured first with a counting allocator around a `Core`
drawing fifty new lines a frame: 7.6 KB per 10-glyph line, 22 KB per 40,
92 KB per 200, of which cosmic-text's `Buffer` is 284 B/glyph and its
shape-run cache 118 B/glyph. `DEFAULT_TEXT_CACHE_BYTES` = 64 MB;
`Core::set_text_cache_budget` / `text_cache_budget` / `text_cache_bytes`
/ `text_cache_len`, `setTextCacheBudget` / `textCacheBytes` in Node,
`kui_set_text_cache_budget` / `kui_text_cache_bytes` in C.

What the first test run found: with the entries at the budget the
process was still nine times over it, and the remainder was the
shape-run cache — `trim(1)` every thirty frames keeps sixty frames of
words, because cosmic-text's cache ages only when trimmed. It is now
`trim(0)` on every eviction pass (words not used since the last one),
and the 240-frame `trim(2)` stays for the idle case. A line still on
screen keeps its shaped entry whatever the trim does; a line edited
afterwards shapes its words again, once. The second thing the test
found was its own harness: the allocator counts the process and the
tests ran in parallel, so they hold a mutex now.

`tests/text_budget.rs` pins six things: the default, a 2 MB budget
against the allocator (the stream that would be ~88 MB settles at ~1.6
MB live, the estimate never more than one frame past the line), the
estimate against the allocator (ratio 0.7–1.6 at 200 columns; it lands
at 1.13), that what the last frame drew is never evicted under a
one-byte budget, that the least recently drawn goes first, and that the
clock still empties an idle cache. `benches/stream.rs` is (4)'s frame
half: `warm_50x200` ~85 µs, `stream_50x200_log` ~25 ms,
`stream_50x200_random` ~64 ms, and the README's table carries the three
rows with the sentence that says whose number the streaming ones are.
The memory assertion is a test rather than a bench because bytes, not
microseconds, are what pin this.

Declined for now, (2): dropping the `Buffer` from cold entries and
keeping the templates. The budget bounds the cache without it, and the
`Buffer` is what re-wrap, measurement and the access tree read; a cold
entry that loses it would have to reshape on the next read, which is a
second cache policy to get right. Worth it the day a document viewer
wants a bigger budget than its `Buffer`s allow; the numbers to argue it
from are above.

`TextSystem::cache` (`text.rs`) is an `FxHashMap<u64, CachedText>` whose
only eviction is `EVICT_AFTER_FRAMES = 300` (`text.rs:150`), applied every
240th frame in `begin_frame` (`text.rs:346`), and the shape-run cache
beside it gets `fs.shape_run_cache.trim(2)` on the same schedule
(`text.rs:350`). Nothing in either policy is a size. A view that shows
text nobody showed before — a terminal streaming, a log tailing, a file
scrolled through fast — keeps every line it showed for the last five
seconds at 60 fps, and a cached 200-column line is ~130 KB: 2.0 GB over
~15k lines in the streaming run above, and F26's counting allocator got
the same per-character rate from the other end (22 KB for a 39-character
editor line, ~560 B/char — the shaped `Buffer` keeps a `ShapeGlyph` and a
`LayoutGlyph` per glyph, then kui keeps a 40-byte `GlyphTemplate` on
top). So the resident set is *frames × lines-per-frame × 130 KB* until
the clock runs, and the clock is what bounds it at ~3.5 GB, which is not
a bound. Nothing outside the core can shrink it: the constants are
private and there is no verb.

The frame benches never saw it because every one of them shows the same
text every frame, and the highlight bench's `cold` row is 900 runs of
~10 characters — 30 samples of that is 27k short entries and a few tens
of megabytes. A `!` because the memory ships today under any streaming
view; the frame-time half of the streaming row is C20's.

**Do:** (1) A byte budget on the cache, LRU by `last_used`, checked at
the end of every `begin_frame` rather than every 240th — a store inside
its budget costs one comparison, the way F26's do. Size an entry by what
it actually holds (glyph count × the measured per-glyph cost, plus the
content) rather than guessing; default the budget to something a
screenful of code never meets and a streaming pane meets every second
(64 MB), and make it a `Core` setter so a terminal can lower it and a
document viewer raise it. A text *this frame draws* is never evicted,
whatever the budget — the promise is the same as F26's "declared is
never evicted". (2) Measure where the 130 KB goes before deciding what
to keep: the `GlyphTemplate` list is what emission reads and the
`Buffer` is what re-wrap, measurement and the access tree read; an entry
that has not been re-wrapped or walked for N frames could drop the
`Buffer` and keep the templates, which is most of the memory for none of
the steady-state cost. (3) The shape-run cache is the second store with
the same absence: trim it on the same budget check, not every 240th
frame. (4) `benches/stream.rs`: the fifty-lines-all-new frame, log-like
and random, and a resident-memory assertion through the counting
allocator F26 used — the number that pins this is bytes, not
microseconds. The CHANGELOG entry says what an app can delete: nothing,
and what changes: a text cache that used to hold five seconds of
everything now holds a budget.

### `~` C17 — IME reaches the stock editor only — **done (2026-09-07)**

Done as (1) and (2); (3)'s corpus half was not built, and the tests went
elsewhere. (1) needed one thing the "Do" did not name: a commit has to be
its own input. Every driver sends `InputEvent::Text` beside a key press
(the winit one at three sites: typing, paste, and `Ime::Commit`), and a
sink already hears the press as a `key` event with `text`, so routing
`Text` to sinks would have typed every character twice — ADR 0011's
"both channels agree" in reverse. `InputEvent::Commit(String)` is the
new variant: a focused editor takes it exactly as `Text`, otherwise
`sink_event` delivers `{kind:"text", text, tag}` to the sink the focused
node reports to (itself, or `enclosing_sink`), the tag merged in the way
`route_key` does it; `Preedit` goes the same way as `{kind:"preedit",
text, cursor:[a,b]|null, tag}`, empty text meaning the composition ended.
(2) `focused_caret_rect` falls through to the focused node's subtree:
the `line` carrying `caret` and C18's `caret_at` give the rect, after
the text pass so the places are this frame's. Doors: `ctx.commit` and
`ctx.preedit(text, cursor)` in Node — Node had no preedit input at all —
`kui_input_commit` in C, `Ime::Commit` → `Commit` in the driver, and
`imeRect()` / `kui_ime_rect` to read the anchor back: a C host driving
its own window could not place the candidate window for the *stock*
editor either, which this closes on the way. Two `EventDef` rows
(`text`, `preedit`) so `docs/props.md` and the TS `CoreMsg` union carry
them. Tests: `tests/ime.rs` (the sink route with tag and cursor, typing
reaching a sink once while the `Text` channel reaches none, the anchor
following the caret and vanishing on blur, the stock editor's commit
still a `changed`) and a Node test driving the same headless. Not the
corpus: a step line carries integers only, so a preedit step would be
one codepoint with the cursor at its end — enough to pin the route in
four adapters, and a round of its own; the Lua adapter has no input to
drive it with. Left for the day a Lua host wants IME.

`InputEvent::Text` is routed to `self.edit.focused()` and otherwise to the
focused control's type-ahead (`dispatch.rs:85`); `InputEvent::Preedit`
goes to the focused stock editor and nowhere else (`dispatch.rs:114`);
and `ime_rect` — what the driver hands `set_ime_cursor_area` so the OS
candidate window sits at the caret — is computed from
`focused_caret_rect`, which finds the node whose content is
`NodeContent::Edit(key)` (`emit.rs:974-982`). An app-owned editor is an
`onKey` sink with `line` children carrying `caret` and `selectionAnchor`
(ADR 0001's custom-editor rows), and to all three of those it does not
exist: a Japanese or Chinese user composing into `modal_editor` sees no
preedit, the commit arrives only if the platform also delivered it as a
key press with `text` (it does not while an IME is active), and the
candidate window opens at the window's origin. The `key` event's `text`
is what plain typing rides today, which is why nobody noticed: ASCII
never composes.

**Do:** (1) Route composition to the focused sink as data: the commit as
`{kind:"text", text}` and the preedit as `{kind:"preedit", text,
cursor:[a,b]}` on the sink's `onKey` tag, with `text` null on a preedit
that ended without a commit. Both channels have to agree the way ADR
0011 made the press and the character agree: a commit that the platform
also reports as a key press is delivered once. (2) Derive `ime_rect` for
a custom editor from the rows it already declares: the `line` node
carrying `caret` is a text node the cache has shaped, so the caret's
rect is `Buffer::hit`'s inverse on that entry — which is C18's caret
verb, and C17 is its first caller inside the core. (3) The corpus
`controls` scene gets a preedit step against the stock editor (it has
none today) and a custom-editor scene gets the same step against a
sink, so all four bindings pin both routes. Node's `Ctx` grows
`preedit(text, cursor)` beside `type`, and C's `kui_input_preedit`
already exists for the stock editor and needs no new door.

### `~` C18 — Nothing maps a point to a byte offset, or an offset to a rect, on text the app owns — **done (2026-09-07)**

Done, with one change to the shape the "Do" below proposed: the key is
**the keyed node the text is inside**, not the text node. No binding can
name a text node — `text` takes no `key` anywhere, and its auto-key is an
index under its parent that only Rust can spell — and the node a custom
editor keys is the `line` row, which holds one run per token and a
selection box around some of them. So `text_hit(key, point)` and
`caret_rect(key, byte)` answer for every text run inside `key` (its own
key, or any of four ancestors), byte offsets running across the runs in
tree order, which is the offset the access tree's `caret` row already
uses. `TextSystem` records a `TextPlace` per emitted text node — key,
ancestors, cache key, origin — and swaps the list each frame, so a query
during a build answers from the frame that finished, which is what lets
a Rust view resolve a click it stashed in `on_event`; a node the frame
culled has no place and answers `None`. Rects and points are logical
viewport px; cosmic-text's `Buffer::hit` and the layout runs answer the
rest, so a query is a lookup and a run walk. `TextHit { byte, line }` in
Rust; `textHit` / `caretRect` in Node with a `TextHit` interface;
`env.text_hit` / `env.caret_rect` in Lua by label or key; `kui_text_hit`
/ `kui_caret_rect` in C with `KuiTextHit` / `KuiCaretRect` out-structs
in the parity check. Tests in all four; the Rust one pins seven shapes,
the line of runs among them. `modal_editor` was not moved onto it: its
click handling runs in `on_event`, which has no `Ui`, and the example is
monospace by construction — the entry that moves it is the one that
gives `on_event` a way to ask the core, which C17 does not need either
(it derives the IME rect in the core). Found on the way: Lua's
`text(s, opts)` mutated and returned the options table, so three texts
sharing a style were one node thrice — fixed in the prelude, pinned, and
in the CHANGELOG under Fixed.

The stock editor answers clicks and drags from cosmic-text's `hit`
(`buffer.rs:1144` in cosmic-text 0.19). Text the app owns has no such
door: `modal_editor.rs:474` computes `col_at` from a monospace cell width
it measures once, `syntax_view.rs:240` pads runs with NBSP because
trailing spaces measure unreliably, and both say "no text measurement
anywhere" as a feature — which it is, until the font falls back for a
glyph (CJK, an icon, an emoji) and the cell arithmetic drifts from where
the glyph was painted. `measure_text` (`ui.rs:173`) is the only
alternative and the wrong shape for it: it measures a *string*, so
finding the byte under x means measuring prefixes, each of which shapes
that prefix and caches it (C16 bites again), O(n) per query and O(n²)
across a drag. A proportional-font editor — a notes app, a chat
composer, anything not code — has no correct path at all.

**Do:** two verbs on the key of any text node, answered from the cache
entry the frame already shaped, so they cost a hash lookup and a binary
search: `text_hit(key, point) -> Option<{line, byte}>` (the point in the
node's box, as `on_click` and `on_drag` report it) and
`caret_rect(key, {line, byte}) -> Option<Rect>`. Rich text answers in the
concatenated content's bytes, which is what the access tree already
names spans by. Node: `ctx.textHit` / `ctx.caretRect`; Lua: `env.text_hit`
/ `env.caret_rect`; C: `kui_text_hit` / `kui_caret_rect` with an `[out]`
struct, no ABI change to what exists. The custom-editor access actions
(`setTextSelection` carries `{line, offset}`) and C17's `ime_rect` are
the two callers inside the repo; `modal_editor` moves its click handling
onto it and drops `col_at`. C19's chunks make both verbs O(log chunks) on
a long line instead of O(glyphs).

### `~` C19 — A long line is shaped whole, and slicing it from outside costs more than not slicing

A text node with `wrap: none` shapes its whole content on first sight
(`intern`, `text.rs:383`), keeps a `GlyphTemplate` per glyph, and at
emission walks every template to find the ones inside the clip
(`text.rs:649-660`: a `filter` over all of them, the `take_while` only
stops on rows). For a 100k-character line — a minified bundle, a log
line with a JSON blob, a base64 field — that is 0.2–0.6 s of shaping the
first frame at the 2–6 µs/glyph the streaming rows measured, ~65 MB
resident for the line at the C16 rate, and a 100k-template walk every
frame after; then the same again the frame after the user types into
it, because the content hash changed.

**Why slicing it from the app is not the answer.** The obvious move is
the horizontal `virtual_column`: show characters `[c0, c1)`, place the
node at the width of the prefix `[0, c0)`, put a spacer after it so the
`scrollX` content width is the whole line's. The width of the prefix is
the problem. Under a monospace font with ASCII content it is `c0 ×
cell_w` and the app can do it today, with `unicode-width` for wide
characters and its own tab expansion — that is the terminal's grid
assumption, and a code editor can live with it. Outside that assumption
the only door is `measure_text(prefix)`, which shapes the prefix whole,
caches it whole, and does so once per distinct `c0` — the first scroll
through the line costs more than shaping it once did, and leaves more in
the cache. So the app-side path works exactly where the arithmetic
works, and where it does not, nothing does.

**Do — chunked, lazy shaping inside the text node**, so a long line is
one node the app hands over whole:

1. Content past a threshold (4096 characters, say; shorter text takes
   the path it takes today, so the corpus and the benches do not move)
   is split into chunks of ~1024 characters at grapheme boundaries
   (`unicode-segmentation` is already a dependency), preferring the last
   whitespace before the cut. cosmic-text already shapes per
   whitespace-delimited word (`ShapeWord`, `shape.rs:753`) and its
   shape-run cache keys on words, so a cut at whitespace loses no kerning
   or ligature the whole-line path would have kept; a cut inside a
   whitespace-free run loses one kern pair per 1024 characters.
2. Each chunk is its own cache entry keyed by *its* content and the
   style — which is what makes a keystroke into the line cost one chunk:
   every chunk whose text did not change hits. The line's entry holds the
   chunk keys and a prefix sum of chunk widths.
3. A chunk is shaped when something needs it: emission for the chunks
   intersecting the clip plus one of overscan either side, `text_hit` and
   `caret_rect` (C18) for the chunk the query lands in, found by binary
   search over the prefix sum. An unshaped chunk contributes an
   *estimated* width — its character count times the mean advance of the
   chunks already shaped, or of the style's `M` before any is — corrected
   when it shapes. The content width the scrollbar sees can therefore
   move a little as chunks fill in, exact under monospace and the same
   tolerance `virtual_column`'s uniform rows already accept; the
   alternative, shaping everything to know the width, is the cost this
   entry removes.
4. Emission walks only the shaped chunks inside the clip, so the
   per-frame cost of a long line is the visible screenful, like a tall
   document's already is (README's "glyph emission is viewport-culled").
5. `wrap: word` on a long line is deliberately the follow-up, not this:
   a wrapped chunk's first row depends on where the previous chunk's last
   row ended, so wrapping is a one-direction prefix computation (cheap:
   positions, not glyphs) rather than an independent one. Ship `none`
   first; it is the case minified files and log lines are.

Memory under C16's budget is the shaped chunks, so a 100k-character line
the user scrolls through costs what a screenful costs. Pin it with a
bench: a 100k-character line, first frame, scrolled by a viewport per
frame, one character inserted per frame — each should be a chunk's cost,
and the first frame should not be the whole line's. A `~` because the
general case has no path today, and the special case (monospace ASCII)
is the app's to keep getting right.

### `.` C20 — A cell grid inside the core, if it beats per-cell nodes by an order of magnitude

The three terminal shapes an app can build today are the streaming
rows in the table: one text node per line (0.08 ms warm, 21–63 ms when
every line is new, because a *line* is shaped as words and a terminal's
words are new every frame), coalesced runs with backgrounds (0.18 ms
warm, the same streaming cost), and one node per cell (1.8 ms warm,
flat under streaming, because single characters always hit the cache —
but 10k nodes of layout every frame, and 10k hit regions). None is what
a terminal wants: glyphs placed at `col × cell_w` with no shaping at all,
which is why every terminal emulator keeps a glyph cache keyed by
character and never calls a shaper for ASCII. Beside the cost, the grid
drifts under shaped runs — ligature fonts join `->` and `==`, a fallback
glyph advances at its own width — and a per-cell node is the only shape
that holds the grid, at the price above.

The core can do this an order of magnitude cheaper than an app can
through nodes, because the cell is the unit and there is nothing to lay
out: a `cells` node is one node in the tree with `rows × cols` cells of
data, and its emission is a table walk.

**Do, gated on a measurement:** prototype the emission alone in a bench
first — 10k cells, a glyph cache keyed by (grapheme, weight, style,
font, size), ASCII resolved by charmap lookup straight through the
`Font::as_swash()` handle the raster already uses and rasterised once
into the same atlas, a non-ASCII grapheme shaped once through a one-cell
buffer and cached the same way, wide graphemes taking two cells with a
spacer after them — and build the element only if that frame comes in
under 0.2 ms warm and stays there under streaming, against the 1.8 ms
per-cell-node row. If the prototype cannot get there, the per-cell node
path is what an app has and this entry closes as measured. If it can:

1. One element, `cells` (`<cells>` / `cells {}` / `kui_cells`), holding
   `rows`, `cols`, a `CellStyle` (font, size, the cell size derived from
   the font's advance rounded to physical px so the grid is
   pixel-aligned) and a flat cell array: grapheme (a `char`, or an index
   into a per-frame string table for clusters), `fg`, `bg`, attribute
   bits (bold, italic, underline, strikethrough, inverse, dim, wide,
   blink is the app's). Node sends it as one packed `Uint32Array` through
   the encoder, one op; Lua as a string per row plus colour runs; C as
   a `repr(C)` `KuiCell` array, which is an ABI bump.
2. Emission: one bg quad per run of equal `bg` in a row, one glyph quad
   per non-space cell, underline and strikethrough as thin quads from
   the font's metrics (shared with C22), a cursor as `{row, col, shape}`
   on the node — block, bar, underline — drawn by the core so the blink
   clock the stock editor already has can drive it.
3. Input: the node is one hit region; its `onClick` / `onDrag` payloads
   gain `cell: {row, col}` so the app never divides by the cell size it
   did not choose. Selection stays the app's: a `bg` override per cell is
   already how a terminal draws it. Keys already arrive through `onKey`.
4. Access: one row, AccessKit's `Role::Terminal` (`accesskit` 0.25 has
   it), value = the rows joined — a screen reader reads the screen, which
   is what the platform terminals do.
5. What it deliberately does not do: ligatures (a cell is a cell), text
   wrapping, anything a `text` node does. A mux that wants a ligature
   font uses runs and C23.

A `.`, not a `~`: an app has the per-cell path today and it is correct,
only slow. The numbers say it is slow by a factor the core can remove and
the app cannot.

### `~` C21 — Nothing outside the main thread can wake `run`, and `pump` never waits — **done (2026-09-07)**

Done as (1) and (2); (3) is a by-hand check, not a smoke job yet.
`access_bridge::UserEvent` grew a `Wake` variant in both of its
configurations (with `accesskit` and without, where it used to be an
empty enum), `kui::Waker` wraps the proxy and is `Clone + Send`,
`App::setup(&mut self, waker)` is called by `run` and `open` before the
window opens with a default that keeps it, and `user_event(Wake)`
requests a redraw on every pane before the AccessKit routing that needs
a window. `PumpRunner::waker()` hands out the same handle and
`pump_until(deadline)` is `pump_app_events(Some(timeout))`. Verified with
the `waker` example instead of a test: a thread pushes a line every 40
ms and wakes, nothing else touches the window, and under
`KUI_WAKER_LINES=30` it printed `30 lines arrived, 28 frames drawn` and
closed itself — the two frames short are wakes that coalesced into one
turn, which is the queue doing what the doc says. The example is in the
README's list and the examples map; the smoke job that would run it is
still P8's. What was not needed: an `on_wake` callback — `view` reads the
app's own channel on the frame the wake produces, and nothing else in
the loop changed.

`Launcher::run` keeps the `EventLoopProxy` it creates for itself
(`lib.rs:219-224`, `shell.proxy`, private and typed to
`access_bridge::UserEvent`, which has one variant, AccessKit's), and
`Launcher::open`'s `PumpRunner::pump` is `pump_app_events(Some(ZERO))`
(`lib.rs:254`) — it processes what is pending and returns, never
blocking for the next event. So an app whose data arrives on another
thread — a PTY reader, a file watcher, an LSP client, a network socket —
has no way to make the loop draw: under `run` it cannot reach the loop at
all, and under `open` it can only call `pump` on a timer, paying either
the timer's latency or its idle CPU. Node does not feel it (libuv is
the timer, by design, D4); every Rust app that is not a pure
view-of-input does. The loop is `ControlFlow::Wait` on purpose and this
keeps it so — the wake is the one event the app owns.

**Do:** (1) `kui::Waker`, `Clone + Send`, wrapping the proxy: `wake()`
posts `UserEvent::Wake`, whose handling is `request_redraw` on every
pane — the app's `view` reads its channel on the frame that follows, and
nothing else changes. `Launcher::run` hands it over through an
`App::setup(&mut self, waker: Waker)` with a default no-op body, so
existing apps compile untouched; `PumpRunner::waker()` returns the same.
(2) `PumpRunner::pump_until(deadline)`: `pump_app_events(Some(timeout))`
so a host that owns the loop can block on OS events *and* its own
wake instead of polling. (3) A test drives `open`, wakes from a thread,
and pins that a frame was drawn without any OS input — headless cannot
(the wake is the driver's), so it is a `SMOKE_*` job beside F13's probe.

### `.` C22 — Underline, strikethrough, and a background per span — **done (2026-09-07)**

Done as the "Do" says, all three at once rather than one on request,
since the editor and the mux both wanted every one and the mechanism is
shared. `SpanDeco` per span (underline, strikethrough, bg; plain text
has one from its style) and `DecoTemplate` rects built beside the glyph
templates in `emit`: each span's index rides its glyphs as cosmic-text
`metadata`, `build_decorations` groups consecutive glyphs of one span
per run, and the rects come out one per line the span covers — the
background under the glyphs (emitted first), the lines over them
(emitted last), placed by swash's `underline_offset` /
`strikeout_offset` / `stroke_size` scaled to the first glyph's size
through `FontSystem::get_font`. Decorations are mixed into the cache
key (a decorated text is a second entry) and nothing else about shaping
or measurement moves, which `decorations_do_not_change_what_the_text_
measures` pins. Rows: `underline` / `strikethrough` on the style
(`Kind::Flag`), `bg` / `underline` / `strikethrough` on a span in every
binding — Node's encoder now writes four slots a span, Lua's span tables
take the three keys, and C's `KuiSpan` grew `bg`, which is **ABI 8**:
the entry's "the bits are additive" was wrong for the array a span list
travels as, since an append moves the stride, so `abi.rs`'s note names
the exception now. The parity assert caught the one mistake on the way
(a field appended in a different order in Rust and in the header). A
curly underline for diagnostics stays a `line` curve, as the entry
said. Four tests in the core, one each in Node, Lua and the C self-test.

`Span` is `text`, `color`, `bold`, `italic` (`text.rs`, and `<span bold
italic color>` is the whole JSX row); `TextStyle` has no decoration
either. An editor's links, diagnostics and search hits, a terminal's
`SGR 4` / `SGR 9`, and any "highlight this word" all want one of three
things a text node cannot say: an underline, a strikethrough, or a
background behind a span rather than behind the node. The workarounds
exist and cost nodes: a `line` float per underline (positioned by C18's
`caret_rect`, or by cell arithmetic), a `bg` box per run for a
background — which is the "coalesced runs" shape the bench measured at
0.18 ms for 500 runs, fine for a terminal row, wrong for a paragraph
that wraps (a span's background has to follow the span across the
break, and a box cannot).

**Do:** three rows on `Span` and two on `TextStyle` — `underline`,
`strikethrough`, and span `bg` — emitted as rect quads beside the
glyphs from the run geometry the layout already has, using the face's
underline offset and thickness (`swash` metrics: `underline_offset`,
`stroke_size`, `strikeout_offset`) so they sit where the font designer
put them; the span `bg` is a quad per run of the span per line, which is
what makes it wrap. Paint-only, so the cache key does not change
(colour is already excluded from it); the `GlyphTemplate` list grows a
decoration list beside it. A curly underline for diagnostics is a
`line` curve today and can stay one. JSX `<span underline>`, Lua `{ "x",
underline = true }`, C `KuiSpan.flags` — `kui_rich_text` already takes
spans; the bits are additive. Parked on the same terms as C13: a view
that wants it names which of the three it wants first.

### `~` C23 — No way to turn ligatures off, or tabular figures on — **done (2026-09-07)**

Done as the "Do" says, with two adjustments. `FontFeatures` is a `Copy`
value on `TextStyle` — eight `(tag, value)` slots and a length, since
`TextStyle` is `Copy` and is not stored per node (the tree keeps a
`TextId`; the style lives in the cache key) — with `parse` for the one
spelling every binding shares (`"liga=0 calt=0 tnum"`; commas or spaces,
bare tag = 1, `-tag` = 0) and `set(tag, value)` for Rust. Mixed into
`style_key`, and handed to cosmic-text's `Attrs::font_features` in
`intern`, `intern_rich` (base and every span) and the stock editor's
attrs. The schema row is `Kind::Str` with a new `Apply::StyleStr`, so
Node's encoder, Lua's `parse_props` and `docs/props.md` needed nothing
of their own; the C field is a `KuiStr` appended to `KuiTextStyle`, and
per `abi.rs`'s rule an [in] append does **not** bump `KUI_ABI_VERSION`
(the entry assumed it would; `KuiSpec.tooltip` is the precedent, and
nothing embeds `KuiTextStyle` by value). The corpus question — a font
with a ligature to pin against — is answered by skipping instead: the
Rust and Node tests look for Fira Code, Cascadia Code or JetBrains Mono
through `add_system_font` and, finding one, pin that `->` is one glyph
by default and two with `liga`/`calt` off; finding none they say so and
return, and the cache-key test beside them holds everywhere.

Every text node shapes with `Shaping::Advanced` and default features
(`text.rs:398`, `text.rs:558`), and nothing in `TextStyle` or the schema
reaches `Attrs::font_features` (cosmic-text 0.19 has it, `attrs.rs:377`).
So a coding font with ligatures joins `->`, `!=` and `www` in every
node, and there is no way to say otherwise — a terminal built on runs
(the C20 alternative) sees its grid drift by exactly the ligatures, and
an editor cannot offer "ligatures: off" as a setting. The other
direction matters as much: a gutter or a table wants `tnum`, a
stylistic set (`ss01`) is how several coding fonts spell their
alternate `0` and `l`, and none is reachable.

**Do:** a `features` row on `TextStyle` — a small list of (tag, value)
pairs, `{ liga: 0, calt: 0, tnum: 1 }` in JSX and Lua, a fixed-size
`KuiFontFeature[8]` on `KuiTextStyle` in C (an ABI bump, since the
struct is by value; ADR 0006's size handshake absorbs it as long as the
field goes at the end). Mixed into `style_key` (`text.rs:354`) because
it changes the shape, and into the editor's attrs so a `<edit>` in a
ligature font matches the text beside it. A corpus scene needs a font
that has a ligature to pin against, which the bundled test font may
not; measure the width of `->` with and without and assert they differ
where the font has the glyph, or skip where it does not.

### `!` W3 — On Windows, animations stop while the window is grabbed

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
The next thing to build was what the next field reports asked for, and
all seven landed the same day they were filed (2026-09-07), each with its
outcome written on top of its entry above: F25 (`setEditText` seeds the
editor the next frame declares, and warns when none does), F26 (a budget
on undeclared editor and scroll state), F27 (every alpha also takes the
`latest` dist-tag — built, and unverified until the tag it writes exists),
F28 (the clause on `quads()` and `access`), F29 (the `audio` element's
`finish`), F30 (`settled` and `frame` on `WindowLoop`) and F31
(`docs/howto.md`, the `**What breaks.**` bullet list from alpha.9 on, and
what a deletion line names). They move to the archive at the next
archiving round; nothing is queued behind them.

**Editor and mux (2026-09-07).** The assessment section above is the
order of work for each. For an editor that owns its document: C16 (the
text cache gets a byte budget — the one `!`), then C18 (point ↔ byte
offset on app-owned text), C17 (IME to the focused sink, whose caret
rect is C18's verb), then C19 (a long line shaped in chunks). For a
mux: C16 again, then C20's *prototype* — the element is built only if
the bench clears the bar the entry sets — then C21 (a waker for the
loop) and C23 (ligatures off); C22 is parked like C13 until a view names
what it wants first.

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
