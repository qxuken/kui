---
status: accepted
date: 2026-09-03
---

# Accessibility as data: semantics in the IR, AccessKit at the edge

kui draws its own pixels, so assistive technology (screen readers, switch
access, voice control, UI automation) sees a window with nothing in it. We
decided that the per-frame tree carries semantics as two more plain-data
props, `role` and `label`; that the core derives an **access tree** from
them every frame, as data; and that only the windowed drivers translate that
tree into platform accessibility APIs, through AccessKit. Nothing about
accessibility is a callback, an object, or a runner-only side channel: it
is declared, derived and consumed the way layout, events, audio and
warnings already are.

## Context

- The IR is the contract. Rust builders, Lua tables, JSX and the C API all
  lower into `NodeSpec` + `Value`; whatever is not in the IR does not exist
  for three of the four frontends. An accessibility story that lives only
  in the Rust runner would cover one binding.
- The tree is immediate-mode and flat, rebuilt every frame. There are no
  widget objects to attach accessibility to, but there is stable identity:
  `Key` is a 64-bit hash of the path from the root, retained state
  (editors, scroll offsets, transitions) already hangs off it, and
  AccessKit node ids are also 64-bit integers. The identity problem that
  makes accessibility hard in immediate-mode toolkits is already solved
  here.
- The core is headless. Layout events, warnings, window commands and
  audio commands all leave as data a driver drains, and `createApp` tests
  assert on them without a window. Accessibility built the same way is
  testable in CI; accessibility built against platform APIs is testable
  only by hand with a screen reader running.
- Today the tree knows what things *do* (`on_click`, `on_drag`, `on_key`,
  `NodeContent::Edit`) but not what they *are*. A button is a box with a
  click payload and a text child. That is enough to derive most roles,
  but not names for icon buttons or images, and never a checkbox, slider
  or tab.
- Keyboard reach is narrower than pointer reach: Tab cycles edit widgets
  only (`Core::focus_adjacent_edit`), key sinks take focus by click or by
  the app declaring `keyFocus`, and a click-only node cannot be reached or
  activated from the keyboard at all.
- The per-frame text list keeps a cache key and a colour per text node,
  not the string (`TextSystem::add` interns content by hash). Deriving a
  control's name from its text children needs the text back.
- AccessKit is the ecosystem answer: one Rust data model (`accesskit`,
  pure data, no platform deps) with adapters for Windows UIA, macOS
  NSAccessibility, Unix AT-SPI and Android, and a winit adapter.
  `accesskit_winit` 0.34 pins winit 0.30.5, the same line the workspace
  pins. egui, Bevy and Masonry ship on it.

## Decision

1. **Two schema rows, on every node, in every binding.** `role` (id 53,
   an enum string) and `label` (id 54, a string) join `PROPS`, land on
   `NodeSpec`, and thread through JSX, Lua, `KuiSpec` and `docs/props.md`
   like `tooltip` does. The role vocabulary is kui's own small enum, not
   AccessKit's 150 roles: the roles kui can honour and a binding can spell
   as a string. Initial set: `none`, `button`, `checkbox`, `radio`,
   `switch`, `slider`, `tab`, `tabList`, `link`, `heading`, `list`,
   `listItem`, `image`, `dialog`, `group`. Roles that need a value
   (`checkbox`, `slider`) read it from companion rows landed with them:
   `checked` (id 55), `valueNow` / `valueMin` / `valueMax` (ids 56–58;
   ARIA's spelling, because Lua's text tables already use `value` for
   their content).
2. **The core derives the access tree; it is data.** After layout, when
   asked, `Core` produces an `AccessTree`: the semantic nodes of the frame
   in tree order, each with its key, role, name, description, rect (final
   viewport coordinates, like `onLayout`), state (focused, disabled,
   scrollable, edit value and selection) and its supported actions.
   Derivation rules, in precedence order:
   - an explicit `role` wins; `role="none"` removes the node and its
     subtree from the tree (decorative);
   - `NodeContent::Edit` is a text input (multiline or not), with value,
     caret and selection from the `EditStore`;
   - `NodeContent::Text` is static text with its content;
   - `NodeContent::Image` is an image, named by `label`;
   - a node with `on_click` and no role is a button; a `window` role maps
     to the window-button roles and the drag strip to the title bar;
   - a scrolling container is a scroll view; the root is the window,
     named by `title`;
   - everything else is structure and is **elided**: plain boxes do not
     appear, their semantic descendants attach to the nearest semantic
     ancestor. A 10k-rect frame produces a tree of a handful of nodes.
   - a name comes from `label`, else from the node's own text content
     (static text, edit placeholder), else from the concatenated text of
     its descendants (name-from-content, as ARIA does for buttons and
     links); `tooltip` becomes the description.
3. **Assistive requests come back in as input.** Platform actions
   (activate, focus, set value, set text selection, scroll into view)
   arrive as one new `InputEvent::Access(AccessRequest { key, action })`
   and are resolved inside the core: activating a button emits the same
   `UiEvent` a pointer click would, focusing an edit sets edit focus,
   setting a value writes the editor. A headless test can therefore drive
   an app the way a screen reader would and assert on what it produced.
4. **The bridge lives in the drivers, behind AccessKit.** The Rust runner
   creates an `accesskit_winit::Adapter` next to the window, forwards
   winit events to it, and on frames where the access tree changed (a
   hash, so idle frames send nothing) converts it to a `TreeUpdate` inside
   `update_if_active`, so an app with no assistive technology attached
   pays nothing per frame. Action requests come through the event loop
   proxy and become `InputEvent::Access`. The Node windowed loop rides on
   the Rust runner; `createApp` exposes the tree as `app.accessTree()`; C
   drains it through the FFI as `repr(C)` rows and integrates whatever
   platform layer it owns.
5. **Missing semantics are diagnostics.** An image without a label and a
   button with no computable name each raise a `Warning` code
   (`image-without-label`, `control-without-name`) through the existing
   diagnostics path, once per node, so a test suite can fail on an
   unnamed control.
6. **Two-phase delivery.** Phase 1 lands the schema rows, the `NodeSpec`
   fields, the bindings and the docs, with no behaviour, so the contract
   is settled and apps can start labelling. Phase 2 lands the derivation,
   the input variant, the warnings and the runner bridge. Phase 1 is small
   and is not blocked on Phase 2 design questions.

## Considered options

- **Leave it to the app.** Rejected: an app cannot reach the platform
  accessibility API of a window kui owns without kui cooperating, and an
  immediate-mode tree gives it nothing to attach semantics to.
- **A runner-side API** (`launcher.access(|tree| ...)`, the app builds an
  AccessKit tree itself). Rejected: breaks IR-first; Lua, JSX and C apps
  could not participate, and the app would redo the tree walk kui already
  does.
- **Depend on `accesskit` in the core** and expose `TreeUpdate` directly.
  Tempting (pure data, MSRV 1.85, no duplication of the role enum).
  Rejected for now: the contract would then be another crate's types,
  every binding needs its own string spelling of the role vocabulary
  anyway, and a small kui enum keeps the promise honest about which roles
  kui actually honours. The runner's mapping to `accesskit::Role` is a
  `match`. Revisit if the mapping grows past that.
- **Nested `a11y = { role, label }` prop** instead of flat rows. Rejected:
  every binding threads flat rows already (`tooltip`, `title`); nested
  objects (`enter`, `keyframes`) exist but cost each binding a decoder.
  Flat matches the schema's grain.
- **Export every node, let AccessKit filter.** Rejected: the 10k-node
  bench would send 10k generic nodes per frame while an AT is attached,
  and platform APIs expose exactly what they are given. Eliding structure
  is what browsers do with `<div>`.
- **Platform APIs directly** (NSAccessibility, UIA, AT-SPI). Rejected:
  three implementations of one thing that AccessKit already maintains and
  the winit ecosystem already pairs with.
- **Bundle keyboard focus traversal for buttons into this ADR.** Deferred:
  it is a real gap (Tab cannot reach a button), but it is a focus-model
  decision (a `focusable` row, a focus ring, arrow keys within lists and
  tab lists), independent of semantics, and AccessKit activation does not
  need it: an AT activates a node by id. It gets its own ADR; the `role`
  vocabulary here is chosen so that ADR can build on it.

## Consequences

- `NodeSpec` grows a `role`, a `label` and a `description` (what the
  `tooltip` prop sets in the bindings). Labels are `Arc<str>`: cloning a
  spec is a refcount bump, a Rust view can keep one across frames and pay
  nothing, and the bindings allocate once where they already turn a
  string into a `Value`. The C API borrows them (`KuiStr`) while the node
  opens.
- Text content must be recoverable per frame for name-from-content.
  `TextSystem` (or the tree) keeps the string, or a reference to the
  interned entry, alongside the cache key. A small memory cost on text
  nodes that the existing cache already mostly pays.
- The 5-pass layout gains no work. Derivation is a post-layout walk that
  runs only when the driver asks for the tree; when no AT is attached the
  runner never asks. The bench gains a "10k rects, access tree requested"
  row so the elision claim is measured, not assumed. Measured on
  2026-09-03 (M-series, debug-free bench build): the 10k-rect frame with
  text and 2,500 buttons costs ~1.38 ms; deriving its access tree (2,500
  buttons named from content, ~3,750 semantic nodes, the other 6,000+
  rects elided) adds ~0.32 ms, paid only on frames a screen reader is
  attached and the tree changed.
- The `kui` runner takes `accesskit_winit` as a default dependency (an
  accessibility feature that is off by default does not get tested). On
  Linux this brings an AT-SPI bus connection at startup; on Windows and
  macOS the adapters are lazy until an AT asks. An `accesskit` cargo
  feature can turn it off for kiosk builds. The adapters must hook the
  window before it is shown, so the runner creates it hidden and shows
  it once the bridge exists; the event loop carries a user event type
  for the adapter's callbacks.
- `InputEvent` gains a variant, so C and Node drivers that match on it
  exhaustively need a case. The FFI gains `kui_access_tree` (rows) and
  `kui_access_request`.
- Warnings become an accessibility lint. Existing example apps will
  start warning on unlabelled images and icon buttons; that is the
  intended pressure, and the examples get labels in Phase 2.
- Semantic changes are not animated and not eased: the access tree
  reports final rects and current values, like `onLayout`.
- What this does not solve: keyboard reach for non-edit controls (next
  ADR), reduced-motion and high-contrast facts (`Env` rows, small and
  separate), colour contrast and target size (the app's design, though
  a lint on contrast between `bg` and text `color` fits the warnings
  channel later).

### Follow-ups

- **Live regions and announcements.** A node whose changed text a reader
  should read without being asked (`aria-live`), and a one-off
  announcement with no node behind it at all ("Saved", "3 results"), are
  both still missing. The second is the reason this is not just another
  row: an announcement is an event on a timeline, not a property of a
  tree, and a frame-by-frame IR that re-declares the whole tree has no
  natural place to say "this, once". A polite/assertive row on the node
  plus a per-frame announcement channel alongside the tree is the shape
  to design, and it wants its own ADR rather than a prop.
- **`required` and `invalid` on form fields.** AccessKit has both (and
  `invalid` distinguishes spelling and grammar); nothing in the IR
  carries them, so an editor cannot say it must be filled in or that
  what it holds is wrong. Two schema rows when the first app needs them.
- **`level` on a heading.** `Role::Heading` has no depth, so every
  heading is the same size to a reader walking by heading.

## Outcome: text (2026-09-03)

The tree goes below the node for editors only. An editor's access node
carries its laid-out lines as runs, built once from cosmic-text's layout
runs (per-character byte length, x and width from the glyph cluster
covering the character, word starts from Unicode word boundaries, the
trailing newline as a zero-width character, runs split past 200
characters so indices fit the platform's byte), and its caret and
selection as run positions. `setTextSelection` and `replaceSelectedText`
resolve in the edit store. Static text keeps its name only: labels are
read whole, and giving every label runs would double a text-heavy tree
for no reader benefit.

### Verified against the platform (macOS, 2026-09-03)

The headless tests pin the data; they cannot tell whether the OS accepts
it. `scripts/ax-audit.swift` drives a running window through the macOS
accessibility API — the same one VoiceOver calls — against the
`examples/rust/accessibility.rs` fixture: 70 checks over roles, names, values,
the whole text protocol (`AXNumberOfCharacters`, `AXStringForRange`,
`AXLineForIndex`, `AXRangeForLine`, `AXBoundsForRange`,
`AXSelectedTextRange` read *and* written, `AXSelectedText`,
`AXInsertionPointLineNumber`) and the actions (`AXPress`, increment,
decrement, setting a selection), each confirmed by the change it makes.
It needs Accessibility permission for the calling terminal, so it stays
a manual tool rather than CI.

Two things it caught that no headless test would have. A drawn titlebar
was named the window title *and* kept its text child, so the title was
announced twice; the strip is now unnamed, since the window already
carries it and the strip has to keep its children for the window
buttons. And the latency HUD was exposed as static text between the
controls, so `widgets::latency_graph` is now `role="none"` — a
development overlay is not content.

Still open by ear: AccessKit maps `Role::Heading` to the literal role
string `"Heading"` rather than a standard `AX` constant, so how
VoiceOver announces headings is worth listening to; and nothing here
tests announcement phrasing or focus-following, which only a real screen
reader session shows.

An app that owns its text is the same tree with the markup inverted: the
`on_key` sink declares an editor role, every drawn line a `role="line"`
row (its text nodes, in whatever pieces the app draws them, are that
line), and the lines holding the caret and the selection's other end
declare `caret` / `selectionAnchor` byte offsets. The runs come from the
same text buffers the frame drew. Because the core cannot apply a
selection or an edit to a buffer it does not own, those requests reach
the app as `access` events in its own terms (line ordinal among the drawn
lines, byte offset), exactly as a slider nudge does. A virtualised editor
therefore exposes the lines it draws; `label` is the place to say "line
120 of 4000". A key sink with no role became a focusable group, since a
sink that was elided could not be reached at all.

## Outcome: selection state (2026-09-04)

`checked` covered checkbox / radio / switch and nothing else, so a row of
tabs read out with no way to tell which was open. Three more facts now
reach the tree, and the split between them is the point — AccessKit and
ARIA both keep `toggled` and `selected` apart, and a screen reader says
different things for each:

- **`selected`** (a flag) is the current one of a set. A `tab` reports it
  either way, so its siblings read as "not selected"; a `listItem` or a
  `link` reports it only where the view sets it. That asymmetry is
  deliberate: a tab is one of a set by definition, while most lists are
  not selections and every navigation bar is not one, and AccessKit's own
  documentation calls a stray "not selected" on each of them the common
  annoyance to avoid. The bridge maps it to `set_selected`, not
  `set_toggled`.
- **`expanded`** names its state (`"collapsed"` / `"expanded"`) rather
  than being a flag. A flag cannot say "collapsed": absent has to keep
  meaning "this node does not expand", and a disclosure that is shut has
  to announce that it is shut or the user never learns it opens. It lands
  wherever it is declared — a twisty, an accordion header and a menu
  button share no role, and gating it on one would have excluded the
  others.
- **"3 of 7" is derived, not declared.** A `list` already holds its rows
  and a `tabList` its tabs as semantic children, so the core numbers them
  itself: the zero-based ordinal on each item, the count on the container
  (AccessKit's model; ARIA repeats the count on every item instead). A
  view that had to declare both would restate what the tree knows and get
  it wrong the first time a row was filtered out — the same reason names
  come from content rather than from a required `label`.

`selected` and `expanded` are one `PROPS` row each, so all four bindings
carry them; the ordinals are core-side and no binding declares anything.
The AccessKit bridge gained four lines.

### What macOS actually does with them (2026-09-04)

`scripts/ax-audit.swift` grew a tab list and a disclosure in the fixture
and 14 checks over them, and the three facts land in three quite
different places — which is the reason to ask the OS rather than trust
the tree:

- A `tabList` becomes an `AXTabGroup` exposing `AXTabs`; a `tab` becomes
  an `AXRadioButton` with the `AXTabButton` subrole, and **`selected`
  arrives as its `AXValue`**, not as `AXSelected`. `accesskit_macos`
  treats a tab as checkable and keeps `AXSelected` for item-like nodes
  (a list row), so the audit pins both halves: one tab reads 1 and the
  others 0, and no tab answers `AXSelected`. Pressing a tab moves the
  state to it and off the one that had it — a tab that switched on
  without its sibling switching off is what a reader announces as two
  open tabs, and it is the failure a `checked`-shaped API would have
  given us.
- **`expanded` reaches macOS nowhere.** `accesskit_macos` 0.27 maps no
  disclosure state at all — there is no `isAccessibilityExpanded` in it —
  so the tree carries the fact, UIA and AT-SPI receive it, and VoiceOver
  hears nothing. The audit asserts the absence so it fails the day
  AccessKit adds it. What a reader can observe today is the panel
  arriving in the tree, which the audit checks instead. Putting the state
  back into the label would paper over this and is exactly what the
  release notes say to delete, so the fixture does not.
- A `listItem` takes the other spelling entirely: it becomes an `AXGroup`
  (macOS has no row role outside tables) and, because a row is item-like
  there, its `selected` arrives as **`AXSelected`** — the attribute a tab
  deliberately does not answer. So the two roles that share one `selected`
  row in the schema are read through two different platform attributes,
  and the audit pins both directions: a tab carries no `AXSelected`, a row
  carries no `AXValue`.
- **`set_size` has no macOS attribute behind it.** Neither
  `position_in_set` nor `size_of_set` is mapped there; VoiceOver derives
  "1 of 3" for a tab group from `AXTabs` itself. So the ordinals are for
  UIA and AT-SPI, and on macOS they are carried by the platform's own
  notion of a tab group — which the audit checks holds three tabs and
  only tabs.

Two platform gaps, then, neither of them kui's to fix: they belong to
`accesskit_macos`, and the data is already there when it grows them.

### A gap that *is* ours: an unpicked row cannot be selected

Adding the list turned up something the headless tests could not have.
`AXSelected` is settable, and a reader uses it to move a selection — but
`accesskit_macos` honours it only on a node it already considers
*selectable*, and `accesskit_consumer` defines that as
`is_selected().is_some()`: carrying the state at all, true or false.

The gate this ADR chose for rows — `Role::ListItem if spec.selected` —
gives an unpicked row `None`. `Some(false)` is unreachable for a row: a
view has no way to say "part of this selection, not the current one". So
setting `AXSelected` on any row but the one that already has it is
dropped, and a reader can only move the selection with `AXPress`. The
audit pins exactly that, in both halves, so the day either side changes it
says so.

The trade was made to keep an ordinary bullet list quiet — a reader saying
"not selected" on each of its rows is the annoyance AccessKit's own
documentation warns about — and it was made before the platform had a
vote. The way to keep both is to derive the selection set the way the
ordinals already are: `set_positions` is walking a `list`'s `listItem`
children anyway, so if *any* of them declares `selected` the list is a
selection and all of them get `Some(spec.selected)`, and if none does they
all stay `None`. A quiet list stays quiet, and a real selection becomes
settable. That is a change to accepted semantics, so it is written down
here rather than made in passing.

## Outcome: a slider's reading (2026-09-06)

The companion rows this ADR landed for `slider` were the three numbers,
and that turned out to be half a slider. A field report (backlog F8) ran
an alpha.6 pomodoro under real VoiceOver and heard all three of its
sliders announce percentages: 25 minutes in [5..60] is "36 percent". The
numbers are right and the reading is useless, because only the app knows
that the unit is minutes. ARIA has `aria-valuetext` for exactly this, and
kui had no row for it.

`valueText` (id 82) is that row, and where it *goes* was decided by asking
the platform rather than by reading the ARIA mapping. The harness is the
one `docs/adr/0008` used: `examples/rust/accessibility` driven through the
macOS AX API, with a scratch patch that gave the slider a string value
beside its `numeric_value`.

**`accesskit_macos` 0.27 has one value slot, and the string takes it.**
`NodeWrapper::value` (`node.rs:344`) tries a toggle state, then a tab's
selection, then the roles whose label is exposed as their value, then the
node's string value — and only reaches `numeric_value` when all of those
are absent. With both set, `AXValue` came back as the string and the
number was not readable there at all. `AXMinValue` and `AXMaxValue` were
untouched, since the adapter reads them straight off the node, and
`AXIncrement` / `AXDecrement` stayed advertised and still worked.

Two things follow. First, the semantics: a reading **replaces** the
number, it does not join it — which is what `aria-valuetext` means, so
kui's row means the same thing on every platform its backends map to.
Second, the shape of the IR: `AccessNode` gets no new field. The reading
lands in `value`, the string slot an editor's text already used, because
an IR with two string fields feeding one `set_value` could express a frame
no platform can draw. The bridge was already correct, `KuiAccessNode` did
not grow — it is an [out-array] struct, so growing it would have bumped
`KUI_ABI_VERSION` for every C host — and the corpus report's value column
pinned the row across all four bindings for free.

Where it is *not*: a progress-shaped `group`. A group carries none of
`valueNow` / `valueMin` / `valueMax` — they are set in the `Role::Slider`
arm and nowhere else — and a role whose numbers are ignored should not
have a reading that is not. The adapter would put the string in `AXValue`
on any role, but whether VoiceOver speaks an `AXGroup`'s value is its
policy, and a harness that reads attributes cannot settle what a reader
says. A progress role of its own is the honest way to want this.
