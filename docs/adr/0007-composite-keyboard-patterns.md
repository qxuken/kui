---
status: accepted
date: 2026-09-04
---

# Composite keyboard patterns: one Tab stop, arrows inside it

Three ADRs have now deferred the same item, so it gets a document rather
than a fourth mention. A tab list, a radio group, a menu and a picker list
are *composites*: on every platform they are **one** Tab stop, and the
arrow keys move an inner selection. kui's ring is derived from
`access::focusable`, so today each tab is its own Tab stop, which is
neither what the platform patterns describe nor what a screen reader user
expects. We decide that a composite is **derived, not declared** — a
container role that the core already numbers the items of, plus items that
are focusable — that it contributes **one** stop to the Tab ring, that the
arrows, Home / End and type-ahead move focus *inside* it, that the core
**moves that focus itself** because focus is core state, that it **never
writes `selected`** because that is app state, and that for the two roles
whose pattern defines selection as following focus (`radio`, `tab`) the
motion also emits the item's activation payload — the event Enter already
emits, so an app that handles clicks on its tabs handles arrows on them
with no new code. The IR grows **no prop row**: three role spellings and a
derived orientation are the whole surface.

## Context

- Three deferrals, one item. ADR 0002 names "arrow keys within radio
  groups, tab lists and lists" as deferred because "the activation rules
  differ per pattern and no example needs them yet". ADR 0001's follow-ups
  touch the same ground from the semantics side. ADR 0003 lists "the ARIA
  composite patterns ADR 0002 deferred (arrow keys inside a menu)" as not
  done there. Each deferral was right on its own and the three together
  are the signal that the rules want settling in one place.
- What exists to build on. ADR 0002 gave the core one focus, a Tab ring
  (`Core::focus_ring`, `crates/kui-core/src/runtime.rs:1008`) over every
  node `access::focusable` accepts, Enter / Space activation through
  `click_node`, and arrow keys **for sliders only**
  (`crates/kui-core/src/runtime.rs:469`), which emit
  `{kind:"access", action}` because the core cannot know what a step
  means. ADR 0003 scoped that ring to a modal subtree.
- The ring is too wide, and the fixture shows it. In
  `examples/rust/accessibility.rs` the three tabs and the three mailbox
  rows are six of the ring's stops, all of them before the first button. A
  keyboard user Tabs through all six; a screen reader user who knows the
  platform expects two, with arrows inside each.
- **The core already computes the composite relation and throws it away.**
  C6's `set_positions` (`crates/kui-core/src/access.rs:834`) walks a
  `list`'s rows and a `tabList`'s tabs to derive "3 of 7" for the
  platform. That walk — which nodes are the items of this container, in
  which order — is exactly what arrow navigation needs, and it exists for
  a different consumer.
- kui has no `radioGroup`, `menu` or `menuItem` role. A `radio` is
  therefore a lone control that reports no set position at all, and a
  context menu — buildable since C2's `onContextMenu` and ADR 0003's
  `modal` — is a float of buttons that announces itself as a dialog full
  of buttons.
- Orientation is the one fact the IR does not carry and AccessKit does.
  `accesskit` 0.25 has `Node::set_orientation`, and `accesskit_macos`
  0.27 maps it to `accessibilityOrientation` (`AXOrientation`), so the
  audit can check it. But the *layout* knows: every container declares
  `dir` (`Dir::Row` / `Dir::Column`), and a wrapped row even knows which
  line each child is on (`Tree::line`, from the flex-wrap work).
- The core has a clock and it is optional: `Core::set_time(now_secs)`
  feeds `anim`, and `anim.time()` is `None` for a driver that never sets
  one. Input routing, by contrast, is entirely timeless today — which is
  what made type-ahead look expensive.
- Roles can only be appended. `KUI_ROLE_* = Role::ALL index + 1`
  (`crates/kui-ffi/src/lib.rs:643`) and the Lua / Node wire carries the
  `ROLES` index, so under ADR 0006 a new role is free at the tail and
  breaking anywhere else.

## Decision

1. **A composite is derived from the container's role and its items being
   focusable.** Four container / item pairs, and nothing else:
   `radioGroup`/`radio`, `tabList`/`tab`, `menu`/`menuItem`,
   `list`/`listItem`. A composite exists when at least one of the
   container's items is focusable; a `group` of buttons, a row of links
   and a bar of icon buttons stay ordinary Tab rings as they are today.
   Derived rather than declared for the reason that already made
   `pos_in_set` derived, the modal "the last one declared", and the ring
   itself a function of `focusable`: a declared flag is a second source of
   truth that can disagree with what is on screen, and a tab list that
   forgot the flag is still a tab list to every reader looking at it.
2. **A `list` is a composite when the row itself is focusable.** This is
   the line between the two things kui spells `list`. A navigation list is
   rows *containing* links: the focusable node is the link, so the list is
   not a composite and every link keeps its own Tab stop, which is what
   ARIA intends for a list. A picker is rows that are themselves focusable
   and carry `selected` — the mailbox list in the accessibility example,
   declared exactly that way today. No new prop distinguishes them,
   because the way they are already built does: `listItem` is not in
   `Role::is_control`, so a row is focusable only where it says
   `focusable`, which is the picker and not the navigation list.
3. **One walk enumerates items, and `set_positions` uses it.** From the
   container, over its subtree, skipping `role="none"` subtrees whole and
   not descending into a nested composite of the same kind: the
   descendants carrying the item role, in tree order. "3 of 7" and the
   order the arrows walk must be the same seven in the same order or the
   announcement is a lie, so they become one function rather than two that
   agree by inspection. It is O(subtree), inside a ring walk that already
   covers the tree.
4. **The composite is one Tab stop, and which item it is, is derived.** In
   precedence order: the item that currently holds focus (so Tab out and
   back lands where the user was, and so `focus_next`'s position lookup
   still finds the ring entry); else an item declaring `initial_focus` (so
   a menu can open on a particular item, the way ADR 0003's modal entry
   already reads that row); else the item declaring `selected`; else the
   first item. **No new retained state.** The roving tabindex that
   browsers keep per composite is, in every pattern kui has, the item the
   app already marks `selected` — the same fact that tells a reader which
   one is current tells the keyboard where to enter.
5. **A focusable node inside an *item* leaves the ring, and the core says
   so.** It is inside a roving stop, so nothing can reach it, and that is
   the kind of declared-but-impossible configuration `modal-behind-content`
   set the precedent for: a warning, `focusable-inside-item`, once per
   node. A focusable node inside the *container* but outside every item
   keeps its own Tab stop — a "+" at the end of a tab bar is reachable,
   which is why the line is drawn at the item and not at the container.
6. **The arrows move inside the composite, both pairs.** Left / Up move to
   the previous item, Right / Down to the next. Both pairs, because the
   perpendicular pair costs nothing — no other consumer of arrows on a
   focused, non-slider control exists — while refusing it turns a
   mis-derived axis into a keyboard dead end that only a screen reader
   user finds. In a **wrapped** container (`wrapChildren`) the cross-axis
   pair moves by a line instead of by one item, using the `line` index the
   layout already computed, which is what a wrapped grid of items needs
   and the only place the two pairs differ.
7. **Orientation is derived from `dir`, reported and not obeyed.** The
   container's own `dir` becomes `horizontal` / `vertical` on its access
   node and reaches the platform through `Node::set_orientation`. It is an
   announcement, not a gate (decision 6 accepts both pairs regardless), so
   a container whose visual arrangement does not match its `dir` costs a
   less precise announcement rather than a dead keyboard — which is the
   right way round for a fact the core derives rather than is told.
8. **Home and End go to the first and last item. A choice wraps; a
   sequence clamps.** `radioGroup`, `tabList` and `menu` wrap at both
   ends: they are a choice among *n*, so past the end is the next choice.
   A `list` clamps, and the reason is specific to lists rather than a
   taste: **a list can be windowed.** With 4,000 rows and 40 in the tree,
   the last item is not the last row, so wrapping from the first to "the
   last" would land on row 40 and claim it was row 4,000. Home and End
   have the same limit and take the same answer ADR 0001 already gives for
   virtualised lists — `label` is where a row says "row 120 of 4,000".
9. **Type-ahead, aged by the frame clock.** While a composite item holds
   focus, printable characters (`InputEvent::Text`) search the items'
   access names — case-insensitive prefix, from the item after the focused
   one, wrapping — and a match moves focus the way an arrow does.
   Consecutive characters extend a buffer; the buffer is cleared at the
   start of the first frame more than a second after the last keystroke.
   **The aging happens where time already lives** (`anim.time()`), so
   input routing stays timeless as it is today. With no clock set — a
   headless driver that never calls `set_time` — every keystroke starts a
   fresh search, which is the useful half of type-ahead and a behaviour
   the tests can pin on both sides. Space extends a non-empty buffer
   instead of activating the item: the rule every platform uses, and the
   only interaction between type-ahead and ADR 0002's Space-activates.
10. **The core moves the focus. It never moves the selection.** Focus is
    core state — ADR 0002 decision 1 made `Core::set_focus` the one writer
    — and arrows inside a composite are the motion Tab already performs
    with a narrower scope, so the core performs it: it moves focus,
    sets `focus_visible`, and scrolls the landing item into view, exactly
    as `focus_next` does. ADR 0003's rule that the core changes nothing is
    about **app** state — which dialog is open, what a slider's value is —
    and `selected` is precisely that: a row the app re-declares every
    frame from its own model. So the core never writes it, and a view that
    wants selection to follow focus writes `selected(ui.is_focused(key))`,
    which is one expression and true by construction.
11. **For `radio` and `tab`, moving also activates** — by arrow, by Home /
    End, or by type-ahead; one rule for every way focus moves inside a
    composite, so none of the three can drift from the others. These two patterns
    *define* selection as following focus — arrows check a radio, and a
    tab bar's automatic activation is what every platform does — and kui
    already has the event for it: the item's `on_click` payload, emitted
    through `click_node`, the same path Enter and an assistive-technology
    `Click` take. So the accessibility example's tab handler needs no
    change to answer an arrow key. `menuItem` and `listItem` do **not**
    activate on motion: a menu that ran whatever you passed over would be
    unusable, and manual activation is the norm for a list. Enter and
    Space activate any of them, unchanged.
12. **Three role spellings, appended; the access node carries
    orientation.** `radioGroup`, `menu` and `menuItem` join `ROLES` and
    `Role::ALL` **at the end**, because that is the only free position
    (ADR 0006), which makes the C header's "the first fifteen can be
    declared" a list rather than a range. The bridge maps them to
    `AkRole::RadioGroup`, `AkRole::Menu` and `AkRole::MenuItem`, which
    `accesskit_macos` 0.27 spells `AXRadioGroup`, `AXMenu` and
    `AXMenuItem`. `AccessNode` gains `orientation`. `radioGroup` and
    `menu` join `set_positions`' pairs through decision 3, so a radio
    finally reports "2 of 3", which it does not today. `menuItem` joins
    `Role::is_control`, so a menu item is focusable by its role the way a
    `tab` and a `radio` already are, and an unnamed one is reported by
    `control-without-name`. `radioGroup` and `menu` do not: a composite
    container is not a Tab stop, only its items are.
13. **What does not change.** A slider is not a composite: its arrows
    nudge as before, and the app clamps. An item that owns the arrows
    itself — an editor, an `on_key` sink — keeps them, and the composite
    gets what is left, the same precedence Tab already gives a sink.
    `disabled` items are not items here, as they are not in the ring.
    Modal scoping is untouched: the ring is computed inside the scope and
    a composite inside it collapses the same way. And nothing here adds a
    `PROPS` row.

## Considered options

- **A `composite` (or `arrowNav`) row.** Rejected: it can be declared on
  something that is not one and forgotten on something that is, and then
  the keyboard and the platform disagree about the same node. Every other
  set fact in this area is derived for that reason.
- **`listBox` / `option` roles, instead of the focusable-row gate.**
  Tempting, and the ARIA-faithful move — `list` really is doing double
  duty. Rejected: it splits a deliberately small vocabulary into two
  spellings of one picture, retires the `list` the example and any early
  app already declares, and buys a distinction the tree can already see
  (decision 2). Revisit if a real listbox needs something a `list` cannot
  say, such as multi-select.
- **An `orientation` row.** Rejected: `dir` already says it, and a row
  that can disagree with the layout is a fact with two owners. The
  argument that this is *AccessKit's* concept and not kui's cuts the other
  way — it is an announcement about how the items are arranged, and the
  layout is what arranges them.
- **Only the derived axis moves.** Rejected: see decision 6. Precision
  where being wrong costs a dead keyboard is worse than tolerance where
  being right costs nothing.
- **A roving item remembered per composite in the core.** Rejected:
  retained state for something `selected` and `Core::focus` already hold
  between them, and it would need eviction rules for composites that stop
  being declared — the `modal_focus` list is the smallest such thing that
  earns its keep, and this would not.
- **`active_descendant` instead of real focus.** Rejected: kui moves real
  focus, so the item's `focused` is the truth rather than a pointer to it,
  a reader's own `Focus` request already lands on the same focus, and
  `focus_visible` and the ring keep working without a second notion of
  where focus is.
- **The core emits an event and the app moves focus** — the slider
  precedent, `{kind:"access", action:"increment"}`. Rejected: focus is not
  app state, and this is the one place where ADR 0003's "the core changes
  nothing" genuinely does not apply. Every app in every binding would
  otherwise reimplement the same ordered walk over a tree the core
  computes and does not expose, and a Lua app could not do it at all.
- **The core writes `selected`.** Rejected: the frame is data the app
  re-declares, so the write is overwritten on the next frame unless the
  core retains a shadow of the app's model — which is the retained widget
  state the whole IR exists to avoid.
- **Arrows never activate; Enter always does.** Rejected for `radio` and
  `tab`: a radio group whose arrows move focus without checking is not a
  radio group on any platform, and it is the behaviour ARIA specifies
  rather than suggests. Kept for `menuItem` and `listItem`.
- **Arrows activate everywhere.** Rejected: a menu that runs the item you
  passed over on the way down.
- **Wrap everywhere, or clamp everywhere.** Rejected: one rule would have
  to be wrong for either menus or long lists, and decision 8's split has a
  reason that is not a preference — a windowed list does not have its last
  row in the tree to wrap to.
- **Defer type-ahead once more.** Rejected: the deferrals are why this
  document exists, and the clock question that made it look expensive has
  a cheap answer (decision 9) that leaves input routing timeless.
- **Manage every focusable inside the container**, as ARIA's composites
  do. Rejected: a list row with a delete button is the common case, and
  under that rule the button is unreachable with no warning. Decision 5
  draws the line at the item, warns where it still bites, and leaves the
  full answer to a grid vocabulary.
- **Fold this into ADR 0002 as an amendment.** Rejected: 0002 is accepted
  and implemented, and three of these decisions (derived composites,
  activation on motion, type-ahead's clock) are new commitments rather
  than clarifications of it.

## Consequences

- **The Tab ring shrinks wherever a composite exists, and that is the
  point.** The accessibility fixture loses four stops (three tabs become
  one, three rows become one). Any app with a tab list changes behaviour
  on upgrade; it changes toward what its users expect, and there is no
  opt-out, because a per-app opt-out would be the declared flag decision 1
  rejects.
- **The examples.** `cargo run -p kui --example accessibility` gains a
  radio group — the one pattern with no example today, and the one whose
  arrows must activate — and a menu opened from a button (`modal` +
  `role="menu"`), which is the shape a context menu takes now that C2
  landed `onContextMenu`. Its tab list and its mailbox list become one
  Tab stop each with no change to their declarations, which is the
  clearest demonstration that this is derived. Its module comment gains
  the arrow / Home / End / type-ahead lines beside the Tab ones.
- **`scripts/ax-audit.swift` gains a composite section, and it is one the
  platform can actually answer** — unlike `expanded`, where the audit had
  to pin an absence. `AXRadioGroup` with its `AXOrientation`, `AXMenu`
  holding `AXMenuItem`s, the tab bar as one Tab stop (post Tab from a tab
  and land on the disclosure, not on the second tab), and arrow keys
  posted as `CGEvent`s moving the selection the way the existing
  press-a-tab check does — with the same both-halves assertion, since a
  tab that turns on without its sibling turning off is still the failure
  worth catching. The script's existing focus checks are unaffected: both
  stops it walks ("count 1" → "Save") are after the list.
- **The conformance corpus gains a `composite` scene**, and `Step` gains
  `Arrow`, `Home`, `End` and `Type(char)` — four report spellings each of
  the four binding adapters must parse. The scene pins behaviour rather
  than lowering, the way `modal` does, and it can: the ring is a solid
  quad, an arrow on a tab is an event, and the access tree is compared
  line by line, so all three halves of decisions 4, 10 and 11 are visible
  to the corpus.
- **Headless tests pin** the ring collapsing to one stop and the entry
  precedence, both arrow pairs, line motion in a wrapped container, Home /
  End, wrap against clamp per role, type-ahead with a clock and without
  one, Space extending a buffer against Space activating, activation on
  motion for `radio` and `tab` and its absence for `menuItem` and
  `listItem`, the `focusable-inside-item` warning, orientation on the
  access node, and a composite inside a modal.
- **Per-keystroke cost** is one walk of the container's subtree inside the
  ring walk that already covers the tree; per-frame cost is one
  comparison to age the type-ahead buffer. Nothing is added to the layout
  passes, and a frame with no key pressed pays nothing.
- **Binding cost is three strings and one field.** `radioGroup`, `menu`
  and `menuItem` land in `ROLES`, `Role::ALL`, the `KUI_ROLE_*` tail and
  `docs/props.md`'s regenerated role list; `orientation` lands on the
  access node and so in the FFI's `KuiAccessNode`. No `PROPS` row, no
  parser in any binding, no new event kind — decision 11 deliberately
  reuses the click payload so that the typed `CoreMsg` unions do not grow
  a variant.
- **What only a screen reader session can show**, as ever: whether
  VoiceOver announces "tab, 1 of 3" once the bar is one stop rather than
  three, and whether Narrator and Orca read a `menu` the way they read a
  native one. The audit pins the data underneath; the announcement is
  heard, not asserted.

## Follow-ups

- **Grid navigation.** A row of cells needs Left / Right *into* the row
  and Up / Down between rows, and a `grid` / `row` / `cell` vocabulary to
  say so. Decision 6's line motion is the geometric half of it already;
  decision 5's warning is where an app feels the missing semantic half.
- **Submenus.** Left / Right in a `menu` should close and open them. kui
  has no submenu relation (a `menu` inside a `menuItem`), so today those
  arrows step like any other menu's.
- **`radio-without-group`.** A lone `radio` is invalid ARIA and now also
  means "no arrows here". A warning in the shape of `control-without-name`
  once something in the repo declares radios — the fixture will, so this
  is small and immediate.
- **Multi-select.** `selected` is one flag per node and Shift-arrow /
  Ctrl-arrow need a range and an anchor. Out of scope here; it is the
  first thing a real `listBox` role would be for.
- **ADR 0001's list selection-set derivation** (a `list` where any row
  declares `selected` gives every row `Some(..)`, so a reader can move the
  selection through `AXSelected`) is independent of decision 2's gate and
  still worth doing; the two agree in the common case, where a picker's
  rows are both focusable and selectable.
- **`KUI_ROLE_LINE` is referenced in three comments in
  `crates/kui-ffi/include/kui.h` and defined in none**, so a C app cannot
  declare a custom editor's lines. Found while checking that a role can
  only be appended; unrelated to this ADR, and a one-line fix.
