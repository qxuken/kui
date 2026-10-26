---
status: accepted
date: 2026-10-26
---

# Props by condition

> **Accepted and built (2026-10-26); backlog C100.** Settled by a grilling round
> with the user. A node carries rules — props that apply when conditions
> hold — over its own size, the window's, the system's settings and its
> own interaction state; the core resolves them, so a view declares a
> responsive, themed, interactive box once and is not re-run for any of
> it. A lazy branch is built only when there is room or a query holds,
> and a row can fill itself with equal columns.

## Context

- A view learns its size one frame late, if at all: Kit's narrow-row
  fixes (`ctx.up_to`, a control that gives way, Tidewater's setting rows
  putting a control under its label) all guess at a width the core knows
  exactly during layout. CSS answers this with container queries; kui
  has nothing between a fixed layout and re-running the view on a
  resize event.
- Kit reads `is_hovered` / `is_pressed` in Lua to pick its colours, so a
  hover re-runs the view and refills the component; replay (ADR 0067)
  cannot keep an instance whose looks depend on the pointer. `hoverBg`,
  `pressedBg` and `focusBg` are the core resolving exactly this, for one
  prop each.
- Every view that honours reduced motion or a dark scheme branches on
  `env.system` by hand.
- What the core already knows: the window's content size and scale
  (`WindowEnv`), the appearance and the motion preference
  (`SystemEnv`), the platform, and each node's hover, press, focus,
  focus-visible and disabled state. It does **not** know the increased
  contrast preference; this ADR adds it.
- Layout solves widths before heights, top down for grow and percent
  after a bottom-up fit pass. A node whose size in an axis comes from
  its content cannot change that content's layout by its own size
  without a loop — CSS requires containment (`container-type`) for the
  same reason.

## Decision

### Conditions

1. **One condition shape.** A condition set is a table whose entries
   must all hold. Its vocabulary:
   - **own size** — `width`, `height`, each `{ below = n }`,
     `{ above = n }` or both, with an optional `band`;
   - **window size** — `window = { width = …, height = … }`, the same
     shape, against the window's content size;
   - **environment** — `dark = true`, `light = true`,
     `reduced_motion = true`, `contrast = "more"`,
     `platform = "macos" | "windows" | "linux"` (or a list of them),
     `scale = { above = 1.5 }` (the window's scale factor);
   - **state** — `hovered`, `pressed`, `focused`, `focus_visible`,
     `disabled`, `selected` (where the node's access spec knows it),
     each `true` or `false`. A node in a hover group is hovered when its
     group is, as `hoverBg` reads it today.
2. **Thresholds are lengths**, so a length token works: `below =
   "$narrow"` (ADR 0027). `below` holds for a size under `n`, `above`
   for a size at or over it.
3. **A band stops the flicker at a threshold.** `{ below = 700, band =
   16 }` starts to hold under 700 and stops holding at 716 or more; the
   default band is 0. The match is kept per node, so the band works on
   the node's own history.
4. **The contrast preference is a new system fact**: `SystemEnv` gains
   `contrast: ContrastPref` (`Normal`, `More`, `Unknown`), the
   `kui-native` runner reads it on macOS (`accessibilityDisplay
   ShouldIncreaseContrast`) and Windows (high contrast on), and the
   `system` event carries it.

### Rules on a node

5. **`when` is a list of entries**, each a condition set and the props
   it sets: `{ width = { below = 700 }, dark = true, gap = 8 }`. Every
   entry that matches applies, in list order, over the node's own props;
   a later entry wins a prop an earlier one set.
6. **The shorthands stay.** `hoverBg`, `pressedBg` and `focusBg` mean
   entries ahead of the node's `when` list, in today's order among
   themselves, so an explicit entry wins over them.
7. **What an entry may set depends on what it reads.**
   - An entry with a condition on the node's **own size** sets layout
     props only — direction, gap, cross gap, padding, wrap, alignment,
     `min_column`, and its children's display through `hidden` — never
     the node's own width, height, min or max in the axis it reads.
   - Every other entry (window, environment, state) may set any prop but
     children and keys: colours, border, radius, opacity, shadow,
     transition, cursor, and layout props too.
   - An entry that breaks this is refused, with a warning naming the
     prop (`diag::RULE_PROP_REFUSED`); the rest of the entry applies.
8. **A node that reads its own size in an axis is a container in it**:
   its size in that axis does not come from its content. A `Fit` sizing
   there is refused with a warning (`diag::QUERY_ON_FIT`) and the
   condition reads the size the parent offered instead. Its own fit
   width (for its parent's fit and for `Min::FIT`) is measured with the
   props its last match set.
9. **Children's state survives a match change.** A child a rule hides
   with `hidden` keeps its keys, its focus history and its retained
   state, as a hidden node does today.
10. **Transitions run on a match change.** The props an entry sets
    animate under the node's `transition`, as `hoverBg` does; layout
    props jump unless `slide` is on; under `reduced_motion` everything
    jumps.

### When they resolve

11. **Window, environment and state rules are known before layout.** A
    state rule that sets only paint props is applied at emit, without a
    re-layout or a re-run of the view; one that sets a layout prop marks
    layout dirty and lays out again without re-running the view.
12. **Own-size rules resolve inside layout**: a container's width rules
    are applied when its width is final and before its children are
    laid across it; height rules, whose props can change the widths
    already solved, take one more layout pass on the frame a height
    match changes. No frame reaches the screen laid out with a stale
    match.

### The lazy form

13. **A branch built only when it can be shown.** A child given as
    `{ needs = { w = 320, h = 200 }, render = function(ui) … end,
    fallback = function(ui) … end }` is built when its parent offers it
    that much room — the parent's content box less its other in-flow
    children and gaps, in each axis `needs` names — and `fallback`
    (optional) is built otherwise. The branch is laid out at exactly the
    size it needs, so the decision is a pure function of the parent's
    space and never feeds back into it; the parent is a container in the
    axes `needs` names (decision 8).
14. **A branch chosen by a query.** `{ query = { width = { below = 700 }
    }, render = …, fallback = … }` builds `render` while the query holds
    over its parent's size (or the window's, `query = { window = … }`,
    or the environment's). State conditions are refused in a query
    (`diag::QUERY_STATE`): a hover must not rebuild a subtree; use `when`.
15. **Last layout's sizes, rebuilt once on a crossing.** A lazy decision
    is taken at build from the sizes the last layout gave. When this
    frame's layout says otherwise, the core re-runs the view once and
    lays out again before emitting. A second disagreement in the same
    frame waits for the next one and warns (`diag::QUERY_UNSETTLED`).
    On the first frame, before any layout, the core lays out first and
    builds after.
16. **A lazy branch is replayed like a component instance** while its
    decision and its inputs hold; a branch not built drops its state,
    which is what not building means.

### A row of equal columns

17. **`min_column = n` on a row** wraps it into equal columns, as many as
    fit at `n` wide or more —
    `max(1, floor((w + gap) / (n + gap)))` — every child as wide as a
    column, the last line's children too (they do not stretch). It is
    CSS's `repeat(auto-fill, minmax(n, 1fr))`, and an own-size rule may
    set it.

### Telling the view

18. **`onQuery`** arrives when a node's matched set of size and window
    entries changes, or a lazy branch's decision does: `{ matched =
    {indices}, width, height }`, the indices into its `when` list. State
    and environment changes have their own events and do not fire it.

### Cost

19. **A node remembers what it matched and on what.** Per node: the
    matched set as a bit set, which side of each threshold its sizes
    were on, the environment revision and the state bits it read. When
    none changed, matching is a comparison; nodes with no rules pay
    nothing beyond one pointer (`rules: Option<Box<RulesSpec>>`, sparse
    like `EventSpec`).
20. **A bench row in the guard**: a thousand nodes with three rules
    each, through a resize and a hover, and the existing `frame_*` rows
    must not move for a tree without rules.

### Doors

21. **`when` is a prop row and the lazy form a verb row** in the schema,
    so `schema::DOORS` pins them in every binding:
    - Rust: `NodeSpec::when(Cond, |p| p.gap(8.0))`, conditions built as
      `Cond::width_below(700.0).band(16.0).and(Cond::Dark)`;
      `Ui::lazy(Needs::w(320.0), |ui| …)` and `Ui::lazy_query(Cond, …)`
      with `.fallback(|ui| …)`.
    - Lua and Teal: `when = { … }` in a node's props; the lazy form as a
      child table; Teal records for both.
    - Node and JSX: `when={[…]}`, `<Lazy needs={{ w: 320 }} render={…}
      fallback={…} />`.
    - C and Odin: `when` as a list value in the props map; `kui_lazy`
      with render and fallback callbacks.
    - The devtools tree shows a node's rules and which matched.
22. **Kit adopts it in the same round**: its colours by state instead of
    `is_hovered` / `is_pressed`, its narrow-row handling by own-size
    rules, the samples' responsive layouts by `when`, the gallery's card
    grid by `min_column`, its transitions off under `reduced_motion`.

## What the building changed

Steps 1–5 were built; step 6, Kit's adoption, is a pass of its own.

- **Window, environment and state entries resolve as the node opens**
  (`prepare_spec`, after the shorthands and before the easing), which is
  where `hoverBg` always resolved, rather than at emit. Every frame's
  build — a replayed instance's included — runs it, so a hover still
  repaints without running a view; a state entry that sets a layout prop
  is simply laid out with the frame. The shorthands were already applied
  first, so "entries ahead of the list" needed no reordering.
- **`dark` and `light` read the palette's appearance** (`Theme::
  appearance`), not the OS's reading, so a theme an app pinned counts and
  an unknown appearance reads as light — the palette kui draws with.
- **`hidden` is two things.** A plain `hidden` prop (a flag, a schema row)
  drops the node it is on: out of flow — no room, no gap — sized to
  nothing, so it paints nothing and is hit nowhere, out of the access
  tree and the Tab ring, its keys and retained state kept; any entry but
  an own-size one may set it. Inside an own-size entry `hidden` is the
  list of the keyed children the entry hides (`hidden = { "chooser" }`,
  `p.hide("chooser")` in Rust): an entry cannot hide its own node, which
  is what it reads, and a child cannot read its parent's size.
- **A `fit` container is refused its `fit` by becoming `grow`**
  (`Grow(1)`) in the axis it reads, with `query-on-fit`: the room a
  parent offers a growing child is what "the size the parent offered"
  is. The parent of a lazy branch is treated the same in the axes the
  branch names.
- **The height pass repeats whole.** When a height entry's match moves,
  the five passes run once more from the clamps as declared (a `fit`
  floor, a size expression); a match that moves again in that run waits
  for the next frame.
- **A lazy branch's room counts a growing sibling at its floor** and the
  gaps as if the branch were in flow, so the room is the same whether the
  branch was built, its fallback was, or neither: with a sibling's laid
  width the decision fed back into itself and stuck either way. A branch
  that built nothing — no fallback, or the first frame — is hidden, so it
  takes no gap.
- **The re-run on a crossing is the runner's.** `Core::rerun_owed`
  (`kui_rerun_owed`, `rerunOwed()`) says a frame's layout overturned a
  lazy decision; kui-native's window, its headless runners, `kui_run` and
  Node's `createApp` build the frame once more before anything reads it.
  A host's own loop asks; one that does not still gets the corrected
  frame next, since the core owes one. A Node window, whose tree is set
  between frames, rebuilds on the loop's next turn.
- **Node decides by position.** A view is encoded before its frame is
  lowered, so the encoder cannot ask the core; it builds each `<Lazy>`
  from `lazyChoices()` — what the last layout decided for each branch the
  last view held, in order (`Core::lazy_open_as`,
  `hosted_lazy_choices`) — and a branch that moved is a crossing like any
  other.
- **A lazy branch is replayed with the instance it is in.** Its decision
  is a read of the fill it is built in, so a kept instance replays the
  branch while the decision holds and is filled again when it moves. A
  branch in a host's own view has no instance to keep and is built when
  chosen.
- **`onQuery` arrives on first sight too**, as `onLayout` does, so a view
  that keeps the fact has it from the first frame; `matched` lists every
  matching entry, 0-based, though only own-size and window changes raise
  it.
- **What an entry may set is a box's props**: the layout fields, the
  paint fields, `cursor`, `transition`, `slide`, `hidden`, `minColumn`.
  A text's style props, handlers, access props, the interaction group
  (`hoverBg`, `gradient`, …), floats, tables and anchors are refused by
  name with `rule-prop-refused`. A list holds 64 entries, the match being
  a 64-bit set.
- **The match cache** keeps, per key, each window and own-size range's
  last answer (what the band reads), the static matches and the matched
  set (what `onQuery` and the devtools read). An entry's static
  conditions are evaluated as the node opens — a few comparisons each,
  cheaper than a fingerprint of the facts would be to check. A node's
  `when` list sits behind an `Arc`, so the tree's copy of a spec and a
  journal's share it.
- **Under reduced motion a match change jumps** by zeroing the node's
  transition for the frame the static match moved.
- **`minColumn`'s lines are `crossGap` apart**, as a wrapping row's are.
- **The Rust spelling**: the conditions are associated constants
  (`Cond::Dark`, `Cond::Hovered`, `Cond::ReducedMotion`,
  `Cond::MoreContrast`, …), `Cond::Hovered.is(false)` for a state
  wanted off, `Cond::platforms(&[…])`; `NodeSpec::dir(Dir)` turns a row
  into a column inside an entry; `NodeSpec::rules_spec()` reads the
  group, since `NodeSpec::rules(Color)` was already a table's rules;
  `Ui::lazy_with(LazySpec)` carries an `onQuery`. An entry's props are
  read off what its builder does to two unlike specs (`Patch::capture`),
  so a Lua table, a JSX object, C's `KuiValue` list and the closure all
  land through the same builders.
- **Teal types `when` as a list of tables**: one record cannot hold a
  `width` that is a range and a `width` that is a sizing. The lazy form
  has a `lazy { }` constructor typed by a `kui.Lazy` record; a bare child
  table with a `render` works in plain Lua.
- **C and Odin**: `KuiSpec` appends `min_column`, `hidden`, `on_query`
  and `when` (a `KuiValue` list) inside the unreleased ABI 39; the
  contrast setting is its own setter, `kui_env_set_contrast`, so
  `kui_env_set_system` keeps its four arguments. Odin's field is `when_`,
  `when` being Odin's. A `$token` threshold resolves in Lua's and JSX's
  entries; C and Odin, and a `<Lazy>`'s `query`, take numbers.
- **`NodeSpec` is 264 bytes**: the pointer decision 19 set, past the
  256-byte bound the size test held, which moved with it.

## Considered options

- **The view reads its own size** (a layout callback, or last frame's
  size passed to the view): a frame late, or a second build on every
  resize; rules resolve in layout and never re-run the view.
- **Media queries on the window only**: the window is a condition here,
  but a panel inside a split pane is narrow in a wide window, which is
  why CSS moved to container queries.
- **Size rules that may set the node's own size**: a loop; CSS refuses
  it by containment, as decision 8 does.
- **First match wins**: one entry per combination of conditions; with
  every match applying in order, `{ dark = true, … }` and `{ width = {
  below = 700 }, … }` compose.
- **Re-running the view on hover** (Kit today): every pointer move a
  refill, and replay lost for every instance that colours itself.
- **A lazy branch decided by a callback during layout**: a view call in
  the middle of layout; the declared `needs` keeps the decision outside
  the solver.

## Consequences

- A resize, a theme switch, a hover or a reduced-motion change no longer
  re-runs a view that only changes its looks or its arrangement, and
  Kit's components replay through all of them.
- `NodeSpec` gains one boxed pointer; `SystemEnv` gains `contrast`; the
  schema gains a `when` row, a `minColumn` row, a `lazy` verb and an
  `onQuery` event, across Rust, Lua/Teal, Node/JSX, C and Odin.
- Layout gains a rule step for containers and, on the frame a height
  match changes, a second pass; the build gains one possible re-run on
  the frame a lazy decision crosses.

## Building order

1. Conditions, `RulesSpec`, the match cache and the environment and
   window rules; `contrast` in `SystemEnv` and the runner.
2. State rules at emit, the shorthands as entries, transitions on a
   match change.
3. Own-size rules in layout, containers, the band, `min_column`.
4. The lazy form, the re-run on a crossing, `onQuery`.
5. Doors in every binding, the devtools view, the bench row, the book's
   page.
6. Kit's adoption.
