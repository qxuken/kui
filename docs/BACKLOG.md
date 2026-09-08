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
C13 and C14 — W3 from the editor-and-mux assessment (2026-09-07), now run and
rebuilt on a real Windows machine; W4–W7 from the first Windows round
(2026-09-08), which is what running it found; and F36,
which fell out of building the four entries the two alpha.9 field reports
and the bake-off produced (F32–F35, filed and built 2026-09-08); and what
comes next; F16–F23 from the two alpha.7 field
reports all closed the day they were filed (2026-09-07), F25–F31 from the
alpha.8 ones by the day after (F27 last, on 2026-09-08), and C16–C23 landed
whole for alpha.9. C15's
remainder was the last split entry, and it closed on 2026-09-07.

Ordered by area, not by priority. What to do next is under "After alpha.9".

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
on 2026-09-08, cutting alpha.9. What stays here is W3, which the first
Windows round below finally ran, reproduced and rebuilt against.

### `~` W3 — On Windows, animations stop while the window is grabbed — **verified, rebuilt and measured on the platform (2026-09-08)**

Reproduced, and the blind fix was half right. Windows 11 26200, winit
0.30.13, a 3840×2160 / 239 Hz display, `Chrome::Native`, release profile,
`fragments` (whose shimmer animates every frame). Frames counted from the
runner itself; the window's own position sampled against the pointer
through a title-bar drag driven by `mouse_event` relative moves at 500 Hz,
which is the injected path a real mouse takes — a `SetCursorPos` warp is
not, and shows none of this.

**The reported bug is real.** With the re-request branch off, a title bar
held perfectly still for 2 s drew **0 frames**. The modal loop dispatches
`WM_PAINT` only when something invalidates the window, and a window that is
not moving never does.

**The blind fix cures it and costs the drag**, which is the second half of
the report and was not in the entry: asking for the next frame from
`RedrawRequested` means a `WM_PAINT` is always pending, and *each answer
blocks on vsync inside the modal loop*, with the coalesced `WM_MOUSEMOVE`
behind it waiting that long. Same drag, three builds:

| | window moves in 2.5 s | ms between moves p50 / p99 / max | px per move p50 / max |
|---|---|---|---|
| nothing animating (`counter`) | 996 | 2.05 / 4.32 / 14.5 | 5 / 14 |
| animating, re-request (the blind fix) | 291 | 8.06 / 18.9 / 38.1 | 18 / 86 |
| animating, re-request off | 983 | 2.04 / 4.21 / 17.4 | 5 / 14 |

Four times fewer position updates, three and a half times the step, stalls
to 38 ms and jumps to 86 px — a window that lags the pointer and moves in
lurches, which is exactly how it was reported the second time. It only
happens while something is alive, because that is the only time the branch
fires.

**Built instead: a timer** — the alternative this entry named, and without
the subclass it assumed. `crates/kui/src/windows_anim.rs` arms
`SetTimer(hwnd, …, 10 ms, Some(tick))` while the pane animates and kills it
when it settles; the `TIMERPROC` is called by `DispatchMessage` wherever the
loop is running, the modal one included, and does one
`RedrawWindow(RDW_INTERNALPAINT)`. `WM_TIMER` is a generated message like
`WM_PAINT`, so real input outranks it, and the pace is the timer's rather
than vsync's. No subclass, so it holds under `Chrome::Native`; outside the
modal loop it is redundant with `about_to_wait`'s own request and coalesces
into the same paint.

Measured on the same rig: the 2 s hold now draws **129 frames** (~64 fps —
Windows' timer granularity is 15.6 ms without `timeBeginPeriod`, which is
a process-wide change not worth making for this), and the drag is back to
**1056 moves, 2.02 ms p50, 5 px steps, 12.8 ms worst** — the static
window's numbers. Frames continue at a full 240 fps through the drag, from
the platform's own invalidation.

(3) of the old "Do" holds as written: the frame clock is the wall's, so the
tween catches up on release rather than replaying.

`~` rather than `!` now: what ships is a hold that animates at 64 fps
instead of the refresh rate. Raising it needs a higher timer resolution,
which is a decision about the whole process, not about this.

## From the first Windows round (2026-09-08)

kui ran on a Windows machine for the first time. Windows 11 Pro 26200,
rustc 1.96 / MSVC, an RTX 5080 driving a 3840×2160 display at 239 Hz with
the desktop at 150%, and an idle AMD integrated adapter beside it. The
round was: `cargo test --workspace`, then every windowed example opened for
real. Four defects, none of which any headless test in the repo could have
seen, and two of them crashes on the first frame.

The round is a script now — [`scripts/smoke-windows.ps1`](../scripts/smoke-windows.ps1),
wired into `smoke-windows` in [`.forgejo/workflows/smoke.yml`](../.forgejo/workflows/smoke.yml)
after the `cargo test` step. It leans on `KUI_SMOKE_FRAMES=n`, new in
`crates/kui/src/lib.rs`: the runner quits once the main window has
presented n frames, which turns every example into a self-terminating check
with an exit code — a wgpu validation panic is a failure with its stderr,
a window that never paints runs out the timeout instead of passing quietly.
14 examples, 120 frames each, about 1.5 s apiece; it was checked against
the bug it was written for by putting W4 back and watching it fail.

This is what P8 said it could not offer ("On Windows there is no equivalent
tool and no plan for one"): it is not the tree-walking audit
`scripts/ax-audit.swift` is on macOS, and it judges that drawing did not
fail rather than what was drawn — but it needs no permission grant and no
human, and it found four things in one afternoon. P8's two follow-ups are
untouched: `windows_nc.rs` and `access_bridge.rs` still have no test.

### `!` W4 — Every `fragment` past the first crashed the app on DX12 — **done (2026-09-08)**

`fragments` panicked on its first paint, from
`Renderer::render` → wgpu validation:

    Dynamic binding index 0 (targeting BindGroup with 'kui.fragment.params'
    label 1, binding 0) with value 64, does not respect device's requested
    `min_uniform_buffer_offset_alignment` limit: 256

`uniform_align` was read from the **adapter**, which reports 64 on DX12,
while the device is opened with `Limits::default()` and its 256 — and
validation holds a dynamic offset to what the device asked for, not to what
the hardware could have done. So the slots were packed 64 bytes apart and
the frame's second fragment sat at an offset validation refused. One
fragment in a frame never hit it, which is why the corpus and the split
bench pass everywhere.

macOS never saw it because Metal's adapter reports 256 and the two agreed —
the exact shape of bug a second platform exists to find. Fixed by reading
the limit from the device (`crates/kui-wgpu/src/lib.rs`), which is the only
number validation ever compares against.

### `!` W5 — A resize handed the surface a size no device can hold — **done (2026-09-08)**

Moving a window to 2600×1500 arrived at `Surface::configure` as
**2578×32711**, and a surface larger than `max_texture_dimension_2d` panics
inside wgpu, taking the app with it. Windows hands out a nonsense size
mid-resize; kui passed it straight through, having clamped only the bottom
(`max(1)`, for the minimized window that reports zero).

`Renderer::resize` and the initial configure now `clamp(1, max)` against the
device's `max_texture_dimension_2d`. A clamped frame is one wrong picture
and the next real size fixes it, which is the right trade against a panic.

### `!` W6 — No C extension could ever load on Windows — **done (2026-09-08)**

`cargo test --workspace` on the platform failed one test, and the failure
was the whole feature:

    left:  "kernel32.dll: path contains a NUL"
    right: "kernel32.dll: plugin declares no ABI; this build is 9"

`sys::path_arg` encoded the path as UTF-16 and packed the code units into a
`CString`'s bytes for `load` to unpack. Every ASCII character puts a zero
byte in its pair, so `CString::new` refused **every path there has ever
been** and `CExtension::open` returned that error before it opened
anything. ADR 0014's C-extension half was dead on Windows for as long as it
has existed, and only a native test run could say so — the cross-compiled
CI path builds this file and never calls it.

`PathArg` is now the platform's own type (`CString` on unix, `Vec<u16>` on
Windows) instead of one type smuggling the other, so there is nothing left
to pack or unpack. The interior-NUL refusal the old comment wanted is kept,
where it belongs: `LoadLibraryW` would stop at one and open something the
caller did not name.

### `.` W7 — Text inside a moving box steps a whole pixel while the box does not

Found while looking for W3 and reported here because it is real, not
because it is what was reported: it is *not* the jitter the round set out
to chase. During a slide the node's own quad moves with sub-pixel precision
while its glyphs are placed at whole physical pixels, so the text wobbles
±0.5 px inside its own background, every frame, for the length of the
animation. Measured headless at scale 1.5 over a 260 ms `enter` — the gap
between the card's left edge and its first glyph swings between 21.52 and
22.49 px while the card's own x reads 525.0, 500.87, 477.51, …

Deliberate, and not obviously wrong: `build_templates` positions a run's
glyphs once and `emit` places the whole run at
`(origin * scale).round()`, which is what keeps text crisp and the atlas to
one raster per glyph. The cost is only visible while something moves, and
it is worse the lower the scale factor — 1 physical px is 0.67 logical at
150% and a whole one at 100%, against half on a 2× Mac, which is why it has
not come up before.

**Do:** nothing yet. If it is worth fixing, the cheap answer is to quantize
an animated displacement to whole physical pixels so the box and its text
step together, rather than to place glyphs at fractional offsets (which
softens moving text and costs the one-raster-per-glyph cache). Either way
it changes what the conformance corpus reports, so it wants a decision
before a patch.

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

### `~` F32 — `setEditText` reaches the editor the next frame declares, but only by a key the app cannot have yet — **done (2026-09-08)**

Built as the "Do" spells it, with one thing the entry did not say and one
door left where it was.

The name defers the way alpha.9's text already did.
`Core::set_edit_text_by_label(label, text)` resolves through `key_of` when
some frame declared the label and is `set_edit_text` on that key; when
nothing has, the text waits in `EditStore::pending_labels` and
`text_edit` claims it by the label it already holds, one line before
`declare`. The claim is the fork the entry named: a key with no state
hands the text to `declare` as its seed, over `initial`; a key that has
one — the editor retained while it was off screen — takes `set_text`,
which is the abandoned-draft case a seed by key cannot express, since
`declare` reseeds nothing that exists. `take_unclaimed_seeds` returns an
`Unclaimed::Key` or `Unclaimed::Label` (sorted within each spelling), and
a label's warning is keyed `Key::ROOT.str(label)` with the label in the
message, so a reader is not sent looking for a hex key they never wrote.

The doors: Node's `setEditText` reads sixteen hex digits as a key and
anything else as a label, the way `focus` does, and `editText`,
`setScroll`, `scrollOffset` and `scrollGeometry` moved from `parse_key` to
`resolve_key` (`textHit` and `caretRect` were already there — the entry
read an older line). `keyOf(label)` is on the shared `core_methods!`
macro, so both `Ctx` and `KuiWindow` have it and `index.d.ts` generated
it: the sentence the `edit-text-without-editor` doc has been printing is
true in Node now. Lua gains `env.set_edit_text(key_or_label, text)`, which
it had no form of at all — and, because `env` only exists inside `view`, a
Lua script's call is *always* mid-build, which is exactly where the label
path wants it: the same view's tree claims the text. C gains
`kui_edit_set_text_label`, appended, `KUI_ABI_VERSION` unmoved (C23's
precedent; the parity assert checks struct field order and a function is
not one).

The one door left alone: **Lua gets no `env.key_of`.** Its verbs take the
label itself through `key_arg`, so a script never needs the hex key, and
the diagnostic's sentence says so rather than naming a verb that is not
there. Node and C are where a key is worth holding on to.

**Tests.** `kui-core/tests/editing.rs` has the three the entry asked for
plus one for the resolving case: a label set before any declare seeds the
new editor with the caret at the end; a draft abandoned, undeclared for a
frame, then set by label from the "update" comes back showing the model's
text (and `key_of` is asserted `None` there, which is what makes the case
what it is); an unclaimed label warns with the label in the message; a
label the last frame declared lands at once. Mutation: with the
`set_text`-on-existing branch removed from `claim_label`, exactly
`set_text_by_label_resets_a_returning_editor` fails. `packages/kui/test.mjs`
runs the report's sequence — `setEditText('edit-n13', …)` from the
`update` that opens the field, render, `editText('edit-n13')` reads it
back — and then its second open over a draft, `keyOf` both ways, and the
unclaimed-label warning. Lua and C get one test each; the C one turns
diagnostics on first, since an FFI context starts with them off.

Docs: alpha.10's CHANGELOG entry corrects alpha.9's F25 "what you can
delete" the way alpha.9 corrected alpha.8's F20 — the advice was true of
the holding and false of the naming — and `howto.md`'s "How do I reset an
editor's text?" and "How do I name a node from outside the view?" are
rewritten around the label spelling. The rest of that page is F33's.

---

The finding as it was filed:

The mind map deleted its `onLayout` latch on the strength of alpha.9's
F25 entry — **What you can delete:** "the frame of waiting … Set the text
in the `update` that opens the editor; the frame that draws it takes the
text with it" — and hit `Error: bad id "edit-n13"`. The held seed works
exactly as F25 built it (their standalone probe: "seeds a closed editor
by its key, reopens it, and the seeded text is there, with no warnings").
It is the *name* that cannot be given: `setEditText` takes the 16-digit
hex key and nothing else, the hex key comes from an event the node fired,
and an editor being opened for the first time has fired none. The
`edit-text-without-editor` warning then says to use "the one
`keyOf`/`kui_key_of` resolves the label to", and Node has no `keyOf`. So
the latch stays, "no longer because the call is a no-op, which is fixed,
but because it is where the key comes from, which is not."

The repo's lines, each as the report read them:

- `crates/kui-node/src/lib.rs:1778` — `set_edit_text` goes through
  `parse_key` (hex or `bad id`), while `focus` / `isFocused` / `reveal` /
  `access` go through `resolve_key` (`:240`), which tries the hex form and
  then `Core::key_of`. `editText`, `setScroll`, `scrollOffset`,
  `scrollGeometry`, `textHit` and `caretRect` are hex-only too, and
  `howto.md`'s "name a node from outside the view" says so as if it were
  a rule rather than an omission.
- `crates/kui-core/src/diag.rs:218` — the warning's doc names `keyOf`.
  `index.d.ts:553` carries it, generated; `packages/kui/index.js` has no
  such method (the Node door built for F5 is `resolve_key` *inside* the
  focus verbs, never a method of its own). Lua has `ui.key_of` behind
  `key_arg` (`kui-lua/src/lib.rs:240`) and C has `kui_key_of`
  (`kui.h:1335`), so the sentence is true in two bindings of three.
- `crates/kui-lua/src/lib.rs:302` — Lua has `edit_text(key)` and **no
  `set_edit_text` at all**; the `<edit>` row's doc (`schema.rs:1303`)
  promises one in every binding.
- `crates/kui-core/src/runtime/builder.rs:328` — `key_of` resolves
  through `key_labels` (this frame so far) and then `key_labels_last`.
  Outside a build that is the last finished frame. An editor a rename
  opens was in neither frame — not on a first open, and not on a second
  one either, since a closed editor is undeclared for every frame in
  between. Only an editor that is *currently* declared resolves by label.
- `crates/kui-core/src/edit.rs:334` — `EditStore::pending` is keyed by
  `Key`; `declare` (`:468`) consumes a seed only on creation, which is
  right for a seed by key (an existing state takes `set_text` where it is
  called) and wrong for a seed by label, because a retained-but-undeclared
  editor (F20, F26: state is kept while its key is off screen) has a state
  and no resolvable label — the mind map's abandoned-draft case exactly.
- `crates/kui-core/src/runtime/builder.rs:475` — `text_edit` has the
  `label` in hand when it calls `edit.declare`, and pushes it into
  `key_labels` two lines later. The seam is already there.

**Do:** the report's own sentence — "deferring the name to the same frame
is the whole remaining distance" — and one more case. (1)
`Core::set_edit_text_by_label(label, text)`: if `key_of(label)` resolves
now (the editor is declared), apply as `set_edit_text` does; otherwise
hold in `EditStore::pending_labels: FxHashMap<String, String>`. In
`text_edit`, before `declare`: `pending_labels.remove(label)` — if the
key already has a state (a returning editor), call `set_text` on it; if
not, hand it to `declare` as the seed. `take_unclaimed_seeds` drains the
labels too; the warning's key for a label is `Key::ROOT.str(label)` so
two unclaimed labels in one frame are two warnings under the
once-per-(code, key) dedup, and the message says which spelling. (2)
Node: `setEditText` takes both spellings the way `focus` does — hex
through `parse_key`, anything else through the label path — and
`editText`, `setScroll`, `scrollOffset`, `scrollGeometry`, `textHit` and
`caretRect` move from `parse_key` to `resolve_key`, since they read state
that exists and last-frame resolution is right for them. (3) `keyOf(label)`
on Node's shared `core_methods` macro (hex string or null), so the
diagnostic's sentence is true in Node; Lua `env.set_edit_text(key_or_label,
text)` through `key_arg` plus the pending path; C
`kui_edit_set_text_label(ctx, KuiStr, KuiStr)` appended — no ABI bump, as
C23's append. (4) Tests, and the mind map's three red checks are the
spec: in `kui-core/tests/editing.rs`, set by label before any declare
seeds the new editor with the caret at the end; abandon a draft, stop
declaring, set by label from the "update", declare again — the editor
shows the model's text, not the draft (the case F25 could not reach and
`declare`'s creation-only consume would miss); an unclaimed label warns
with the label in the message. In `packages/kui/test.mjs`, the report's
sequence verbatim: `setEditText('edit-n13', text)` in the update that
opens it, render, `editText('edit-n13')` reads it back. Mutation: drop
the `set_text`-on-existing branch in `text_edit` and the second test must
fail. (5) alpha.10's CHANGELOG corrects alpha.9's F25 deletion line the
way alpha.9 corrected alpha.8's F20 line: "set the text in the `update`
that opens the editor" was true only with a key the app could not have,
and is true now with the label the view declares. (6) The
`edit-text-without-editor` doc, the `<edit>` row and `howto.md`'s "How do
I reset an editor's text?" say the label spelling first. A `~`: the
workaround is the latch, and this is the third release it has survived
its own bug report.

### `.` F33 — `howto.md` contradicts the release that shipped it, and nothing checks a "today" sentence

**Done (2026-09-08), and the check found a fourth site the entry missed.**
All four parts of the "Do" below are in the tree. The guard —
`crates/kui-core/tests/docs.rs`, one `#[test]` the workspace run already
executes — was written first and run against the stale page, which is how
the fourth turned up: it named F25, F30 **and F27 twice**, the second in
"How do I pin the version I tested?", where the sentence was right and only
the citation was stale. That one now points at alpha.9's `### Changed`
instead. The three answers say what alpha.9 shipped: "find out a release
happened" is `npm view @qxuken/kui version` and `npm outdated` with `@alpha`
as the fallback and why silence is not "no such release"; "reset an editor's
text" is F25's held seed, over `initial`, for one frame, with
`edit-text-without-editor` when nothing declares the key (the label spelling
is F32's line to add, so this touched one sentence and its links); "settled
frame" is `await app.settled(maxMs = 10_000)` and `await app.frame()`, and
says the cap resolves with `animating()` still true rather than throwing.
"How do I use one font in every headless core of a suite?" is under *Test
it*: `setup` registers against that surface, `init(surface)` reads the id
into the model, `addSystemFont` is idempotent per family
(`runtime/resources_api.rs:91`) so the same `setup` is right for every core,
and the module global is what raises `foreign-resource` — the pomodoro's
wish 1, answered where the app will read it rather than in a fourth backlog
entry.

The test itself parses rather than matches: an id is uppercase letters,
digits, an optional lowercase suffix and an optional `(x)`, so a cited
`C5(b)` and a closed `C5` are different strings and the parked half of a
split entry stays citable. It asserts the closed index parsed as more than
twenty entries, since a section that moved would otherwise make the whole
check vacuous. Both halves were mutation-tested: the citation half fails
with the three ids and their sentences before the fix, and misspelling one
`props.md#` anchor fails the second with both links that carry it.

Both reports found it independently. The pomodoro (wish 2): "How do I
find out a release happened?" says the package "publishes one dist-tag,
`alpha`, and no `latest`" — in the tarball of the release whose own
`### Changed` says every alpha now takes `latest`. The mind map: "How do
I reset an editor's text?" says `setEditText` before the declare "is a
silent no-op today … backlog F25 is that gap" — in the release that closed
F25. Checked: `docs/howto.md:325-336` (F27, and today the registry
answers `{"alpha":"0.1.0-alpha.9","latest":"0.1.0-alpha.9"}`), `:148-156`
(F25), and a third neither report reached, `:268` — "backlog F30 wants
that as a promise on the windowed loop", where F30 shipped `settled()`
and `frame()` in the same release. Three of the five backlog ids the page
cites are closed. The cause is the round's shape: F27, F30 and F31 were
built in parallel worktrees the same day, F31 wrote the page against the
alpha.8 tree it was given, and the merge kept both sides — the CHANGELOG
and BACKLOG conflicts were resolved by hand, and this page had no
conflict to resolve because nothing else touched it.

The pomodoro asks for "a CI check that the shipped docs do not contradict
each other on a claim the registry can answer". The registry claim
itself is the wrong thing to check — a phrase match is brittle and the
registry is not reachable from every CI job. What every stale sentence
here hangs on is a backlog id cited as open: "backlog F25 is that gap",
"F27 is the fix", "F30 wants". That is mechanical.

There is also a fourth ask hiding here. The pomodoro's wish 1 —
per-surface resource handles — is on its fourth report, and the answer
has been "theirs" three times (`init(surface)` runs before the first
frame and puts the id in the model; `addSystemFont` is idempotent per
family). Every declined answer lived in a backlog entry the app never
reads. A `howto.md` answer is the cheapest way to retire a wish that is
already answered, and it is what the page is for.

**Do:** (1) Rewrite the three answers: "find out a release happened" is
`npm view @qxuken/kui version` and `npm outdated`, with `@alpha` as the
fallback if `latest` is ever absent; "reset an editor's text" says what
F25 built (and F32, when it lands: the label spelling); "settled frame"
says `await app.settled(maxMs)` and `await app.frame()` on the window
loop and what the cap means (resolves with `animating()` still true
rather than throwing). (2) Add "How do I use one font in every headless
core of a suite?" under *Test it*: register in `setup`, read the id in
`init(surface)` into the model, and never hold it in a module global —
the pomodoro's `useFont` is the shape to describe. (3) The guard: a
`#[test]` in `crates/kui-core/tests/docs.rs` (the workspace test CI
already runs) that reads `docs/howto.md` and `docs/BACKLOG.md` via
`CARGO_MANIFEST_DIR`, collects every `backlog ([A-Z]+\d+[a-z]?(\(b\))?)`
the page cites, collects every `**ID**` the "## Closed — index" section
lists, and asserts the two are disjoint — a closed id in a "today"
sentence is exactly the drift both reports hit. A second assertion in the
same test: every `props.md#anchor` the page links resolves to a heading
`gen` writes, so a renamed section fails here rather than in a reader's
browser. Mutation-test it by leaving one of the three stale citations in
place first. (4) Prose in `howto.md` cites a backlog id only for an open
gap, and the entry's "Do" for any future doc-shaped item says so. A `.`:
a wrong sentence a reader can disprove in one command.

### `.` F34 — A one-shot cut off without `finish` is silent (pomodoro wish 3) — **built (2026-09-08)**

**Built 2026-09-08 (alpha.10).** `truncated-playback`, raised from the
driver's end. `AudioStore` keeps a bounded `stopped` map — written where
`reconcile` stops a non-looped, non-`finish` playback, `Why::Removed` for a
departure and `Why::Restarted` for a changed `src` — and nothing there is a
warning until something answers for it: `audio_ended` drops the entry (it
reached its end, so the stop cut nothing off), and the oldest entry makes
room at 256, which is what keeps a headless suite from growing one. `Audio::apply`
returns the playbacks a `Stop` found with a `state()` other than `Stopped`,
paired with `position()`; the runner's `apply_audio` hands each to
`Core::audio_truncated(playback, at)`, which looks the key up, raises the
warning on it and drops the entry. The driver stays key-blind. A stop that
landed after the sound ended, an imperative `Core::stop`, a loop and a
`finish` release are all silent, and a `Ctx` never raises it at all — the
headless assertion point stays `audioCommands()` holding a `stop` for the
node, which the code's doc, the `finish` doc and the howto answer all say.
Verified through the real device too: a 2 s blip stopped mid-play comes back
as one truncation with a position inside the sound
(`crates/kui/src/audio.rs`, degrading to quiet where CI has no device).

The original finding:

"Presence-as-playback now has the right primitive and still no way to
notice you did not reach for it. A non-`loop` node removed while its
playback is still running is almost always a truncated sound, and the
core knows both facts at that instant." Half true. The core knows the
node went (`AudioStore::reconcile`, `kui-core/src/audio.rs:396-410`,
stops the playback unless `finish`) and knows it was not looped; it does
**not** know whether the playback was still running. `ended` is the
driver's word (`Core::audio_ended`), an untagged one-shot leaves no
`tagged` entry to have heard it, and a headless `Ctx` has no driver, so
`ended` never arrives there at all. A core-side `truncated-playback`
would fire on every one-shot removal in every headless suite — the
pomodoro's included, whose own assertion point for this is the `stop`
command `audioCommands()` hands back (its notes: "checked by deleting
`finish` and re-running: `FAIL … (got stop)`"). The driver, on the other
hand, knows exactly: `Audio::apply_one` receives `Stop { playback }` and
holds the handle whose `state()` is not `Stopped`
(`crates/kui/src/audio.rs:244-248`).

**Do:** raise it from the side that knows. (1) `AudioStore` keeps a
`stopped: FxHashMap<PlaybackId, (Key, Why)>` written where `reconcile`
stops a non-looped, non-`finish` playback — `Why::Removed` or
`Why::Restarted` (a changed `src` is a truncation of the old playback
too, and the message should say which). (2) `Audio::apply` returns the
playbacks it stopped while they were still playing; the runner's
`apply_audio` (`kui/src/lib.rs:1575`) hands each to
`Core::audio_truncated(playback)`, which looks the key up, raises
`diag::TRUNCATED_PLAYBACK` on it ("removed while 0.9 s remained" is not
knowable — kira reports position, so "removed at 0.5 s" is), and drops
the entry; a `Stop` that landed after the end drops the entry silently.
The driver stays key-blind, as `audio_ended` is. (3) The warning's doc
says the headless shape plainly: a `Ctx` never raises it because nothing
plays; a suite asserts on `audioCommands()` seeing a `stop` for the node,
which is the same fact from the other end. (4) The `finish` doc and the
"play a sound when the model changes" howto answer name the warning as
the reason to reach for `finish`. (5) Tests at the core boundary —
`audio_truncated` on a recorded stop raises once with the key; on an
unknown playback raises nothing; on a `finish` release nothing was
recorded — since `StaticSoundHandle` cannot be built without a device.
A `.`: a wrong constant is a truncated sound and today nothing says so;
the pomodoro's six-second `CHIME_MS` is the cost it names.

### `.` F35 — A released playback holds a voice, and a refused play is a stderr line the view never hears (pomodoro wish 4)

**Done (2026-09-08), as the entry asks — the sentence in three places, the
refusal routed, no budget.** `AudioSpec::finish`, the `audio` element's
schema row and `howto.md`'s "How do I play a sound when the model changes?"
now all say what a release costs: one of the device's 128 voices until the
file ends, released or not, and the 129th play refused. 128 stayed kira's
number — `MainTrackBuilder::sound_capacity` is named in the `finish` doc as
where a setting would go, and nothing was built on top of it.

The refusal is data now. `Audio::apply` returns the plays the device would
not take, the way `poll_ended` returns the ones that finished; the
`eprintln!("kui: play failed")` is gone and a decode failure takes the same
exit. The runner's `apply_audio` folds them into `Core::audio_refused`,
which reports `{kind:"sound", phase:"refused", playback, tag}` for a tagged
playback — so a view waiting on `ended` is unstuck, and can tell the two
apart — and raises `diag::PLAYBACK_REFUSED` on the node that asked either
way, so an untagged refusal is not silent. The event is what `ended` is,
pending on the origin, and the routing the two share is one
`route_playback_events`.

Three details worth naming. The warning's key is the node that asked: the
tagged node's, else the mounted `audio` element's, else the origin root an
imperative `play` starts from — so the dedup makes a view that keeps asking
past the limit one line rather than one per refusal. Refusals are buffered
on the driver rather than returned inline, because `flush_pending` runs from
the poll as well as from `apply`, and `active()` counts a buffered one so
the loop comes back around to drain it. And the `audio` node's mount is left
alone on a refusal: unmounting it would have the next frame re-declare,
replay and be refused again, one line per frame.

Four tests beside the F29 ones in `kui-core/src/audio.rs`: a refused tagged
playback is one `refused` event and one warning, and can never `end`
afterwards; an untagged one is the warning alone, on the root; a refused
*released* playback still reports, on the key its node declared; and an
untagged `audio` node warns on its own key rather than the root.
`cargo test --workspace` green (69 binaries), `cargo fmt`, `cargo clippy
--workspace --all-targets` clean, `npm test` 92 pass / 1 skipped. The addon
was rebuilt before `npm run gen` with `target/napi-type-defs` cleared —
`props.md` and `index.d.ts`'s `WarningCode` carry the new code, member count
unchanged at 138 — and the hand-written `SoundMsg` in `index.d.ts` took the
new phase, since only the union is generated.

One adjacent case was left as found, not fixed: a device that fails to
*open* drops every command through `apply_one`'s `manager()` guard, which is
the same forever-wait for a tagged node. It is already announced loudly and
once ("audio device unavailable … sounds are dropped"), and it is a
different failure from the device refusing a play, so it stays out of this
entry rather than being widened into silently.

"`finish` means the driver holds a playback the view has forgotten.
Nothing says whether those are bounded, the way alpha.9 bounded
undeclared editors at 256 — and the app that declares a one-shot per
keystroke is the one that will find out." The bound exists and is
kira's: `Audio::warm` opens the device with `AudioManagerSettings::default()`
(`kui/src/audio.rs:121`), whose main track holds **128** concurrent sounds
(kira 0.12.4, `track/main/builder.rs:26`), released or not; `playing`
(`:76`) keeps a released handle until `poll_ended` sees it stopped. Past
128, `m.play` fails with `SoundLimitReached` and `apply_one` does
`eprintln!("kui: play failed: …")` (`:241`) — and that is the real gap:
the playback is never inserted, so it never ends, so a `tag`ged node
waiting for `ended` (the pattern `howto.md` recommends) waits forever, and
a `finish` node is released to nothing. A one-shot per keystroke with a
1.4 s file needs ninety keystrokes a second to reach it; a loop reaches
it at once.

**Do:** (1) The sentence, in the `audio` row (`schema.rs`), the `finish`
doc (`kui-core/src/audio.rs:127`) and the howto answer: a released
playback holds one of the device's 128 voices until its file ends, and
the 129th play is refused. (2) Route the refusal instead of printing it:
`Core::audio_refused(playback)` reports `ended` for a tagged node (so a
waiting view is unstuck, with `phase: "refused"` rather than `"ended"`
so it can tell) and raises `diag::PLAYBACK_REFUSED` on the key; the
driver calls it where the `eprintln!` is, and a decode failure
(`decoded()` returning `None`) goes the same way. (3) Not a kui budget on
top of kira's — 128 is the device's number and the app that needs
another sets it; note `MainTrackBuilder::sound_capacity` as where a
setting would go, and leave it until a view asks. (4) Tests at the core
boundary, as F34: a refused tagged playback is one `sound` event and one
warning; an untagged one is the warning alone. A `.`: reachable only by
a loop today, but the failure it hides is a hang in the view.

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

**Build next.** Nothing with a written ADR and no code. The `fragment` element, [`docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md`](adr/0015-a-fragment-element-and-the-painter-it-is-not.md), was proposed, measured twice, accepted and **built on 2026-09-08** — a box a registered WGSL function paints, in four bindings with a corpus scene, plus `animate` as a plain row and ABI 9. Its two measurement sections are why it was accepted (naga costs a cold build of `kui-core` alone; the draw-call split costs about 0.6 µs of CPU a fragment and nothing the GPU can see) and its amendment is what the building changed — including a collision the design could not have seen, that JSX's own `Fragment` sentinel was the string `'fragment'`. Before it, slots, [`docs/adr/0014-slots-an-extension-fills-in-place.md`](adr/0014-slots-an-extension-fills-in-place.md), were proposed, accepted and **built on 2026-09-07** for alpha.9 — an extension fills a place the host declares in its own view, under a namespace the host decides, with `Value` parameters in and replies out; its status block records what the building changed, and the first test it pins was a defect before it: an extension whose root carried no `key` was rekeyed whenever the host added a child at the root. Before it, ADR 0012's
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
against iced and gpui beside them; the section above this one holds
what survived the check as F32–F35. Build F32 first — it is the third
release the mind map's latch has outlived, and the fix is the report's
own sentence — then F33's page and guard, then F34 and F35 together,
since both are one `Core::audio_*` door each and the same driver seam.

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

**Windows (2026-09-08).** The platform ran for the first time and the round
is a script: `scripts/smoke-windows.ps1`, in `smoke-windows` beside the
`cargo test` step. W4, W5 and W6 are fixed and W3 is rebuilt; W7 is the one
open thing it found and wants a decision, not a patch. What it still does
not cover is P8's two follow-ups, unchanged: `windows_nc.rs` has no test
that runs anywhere, and neither does `access_bridge.rs`. The next Windows
session's list, in order: run the round under `Chrome::Custom` (the
subclass is uncovered by everything above), run it on the integrated
adapter to see what a second GPU changes, and settle W7.

**Design, wanting an ADR.** A **painter** — the iced-shaped hatch that ADR 0015 (above) names and does not build: a Rust trait or a C extension's function pointers over the shared `Gpu` and the frame's encoder, under the `PainterId` that has been reserved in `resources.rs` since the first commit, placed by a marker quad the renderer splits around as it splits around a fragment. The first thing in a frame that would not be data, so it waits for a view a fragment cannot serve: a 3D viewport, a simulation, a backdrop a copy cannot make. Otherwise nothing new since ADR 0014 was built on 2026-09-07; what it leaves open — a slot element for Node, extensions in `kui_run`, an extension offering slots of its own — waits for a view. Two instances of one extension are answered: the host namespaces them. Effects an app defines (F23) is
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

**Hygiene.** Archiving is done four times over: the forty-six of
2026-09-06, the ten field-report entries that followed them before the tag —
so all of F1–F15 sit together — W2 whole on 2026-09-07, once its driver
half was built, and the fourteen of alpha.9's round on 2026-09-08. This
file is three parked entries, W3, F32–F35 from the alpha.9 reports, and
this section.
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

Eighty-six entries, all in
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

**From two alpha.8 field reports (2026-09-07)** — F25–F31; six closed the day they were filed, and F27 on 2026-09-08, when alpha.9's publish ran the step it built

- `~` **F25** — [`setEditText` before the editor exists does nothing, and alpha.8's deletion list told an app to delete a latch it still needs](backlog/closed-2026-09.md#-f25--setedittext-before-the-editor-exists-does-nothing-and-alpha8s-deletion-list-told-an-app-to-delete-a-latch-it-still-needs--done-2026-09-07) — done (2026-09-07) — `setEditText` seeds the editor the next frame declares, and warns when none does
- `~` **F26** — [Editor and scroll state are kept by key forever](backlog/closed-2026-09.md#-f26--editor-and-scroll-state-are-kept-by-key-forever--done-2026-09-07) — done (2026-09-07) — a ceiling on undeclared editor and scroll state, longest-undeclared evicted first
- `~` **F27** — [There is no supported way to learn a release exists, and every range an app writes floats](backlog/closed-2026-09.md#-f27--there-is-no-supported-way-to-learn-a-release-exists-and-every-range-an-app-writes-floats--done-2026-09-08) — done (2026-09-08) — every alpha also takes `latest`; verified on the registry the day alpha.9 published
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
