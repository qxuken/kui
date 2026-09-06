# Backlog

From the architecture review of `93169ed` (2026-09-03), after 0.1.0-alpha.5,
and the six rounds that followed it. Every item names the evidence that
produced it, so a task that turns out to be wrong can be argued with rather
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

The work ADR 0004 (C7) decided but did not do. **Steps 1, 2, 3 and 5
shipped** (2026-09-04 and 2026-09-05), and the record of
each — what the build settled, the two C breaks, the `Pane`-per-window runner —
is in
[the archived entry](backlog/closed-2026-09.md#-c11--build-multi-window).
Step 4 is what is left, and it is reproduced here as written:

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

And the half of the entry's testing note that is about this step: "Testing
splits the way the ADR says: the conformance corpus can pin the declaration
diff (declare a window, stop declaring it, assert the command sequence — all
four transports reproduce it byte-identically), but a popup window has no
headless equivalent, so its behaviour belongs in P8's macOS/Windows smoke
jobs." The rest of that paragraph is about the two step-3 warnings, which
shipped.

---

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
cap).

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
`SMOKE_MACOS` / `SMOKE_WINDOWS` the day a runner exists (P8), and remove Lua
`env.focus` at 0.2 (P3, R7).

---

## Closed — index

Forty-six entries, all in
[`backlog/closed-2026-09.md`](backlog/closed-2026-09.md) and all verbatim.
This index is here so an id resolves without opening that file: the open items
above cite C7, C10, P8 and R3, "After alpha.6" and the hygiene note cite C2,
C5(b), P3 and R7, and code comments, ADRs and commit messages cite ids of
their own. Forty-four of these are simply closed; **C11** and **C15** appear
in both files, whole there and trimmed to what is still open here.

The archive also holds two sections that are records rather than work: the
suggested sequence as it stood on 2026-09-05, and
[Release 0.1.0-alpha.6](backlog/closed-2026-09.md#release-010-alpha6-2026-09-05),
whose R1–R7 are the half-baked items finished before the tag.

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
- `.` **C11** — [Build multi-window](backlog/closed-2026-09.md#-c11--build-multi-window) — steps 1, 2, 3 and 5 shipped; step 4 is above

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
