---
status: accepted
date: 2026-09-06
---

# Live regions and announcements: a row for the sustained case, a queue for the one-off

A screen reader reads what the user asks it to read. Two things it must
read *without* being asked are still missing from kui: a region whose
changed text should be spoken as it changes (a result count, a
connection status — ARIA's `aria-live`), and a one-off message with no
node behind it at all ("Saved", "Copied", "3 results"). We decided that
the first is one more schema row, `live`, on the node that holds the
text; that the second is an imperative act, `Core::announce`, queued and
drained the way audio commands, window commands and warnings already
are; and that both reach the platform as **a named live node whose name
changed** — the region's own for the first, one the bridge invents for
the second — because AccessKit's whole event surface is a tree update and
it has no announcement API to call.

## Context

- ADR 0001's own follow-up named the split and the reason it needs an
  ADR: "an announcement is an event on a timeline, not a property of a
  tree, and a frame-by-frame IR that re-declares the whole tree has no
  natural place to say 'this, once'". Everything else 0001 and 0002
  named has shipped — roles, names, values, `selected` / `expanded` /
  set positions, modality (0003), composite keyboard patterns (0007),
  `orientation` — so this is the largest remaining gap and the only one
  that cannot be closed by adding a row.
- **AccessKit has the live-region half and nothing else.** `accesskit`
  0.25 carries `Live { Off, Polite, Assertive }` as a node property
  (`src/lib.rs:584`, `set_live` at 2472) and `is_live_atomic` beside it.
  Its entire public event surface is `TreeUpdate`: grepping the crate for
  an announcement, notification or event API returns `pub struct
  TreeUpdate` and nothing else.
- **All three adapters derive the announcement from a tree diff, in the
  same shape.** `accesskit_macos` 0.27 fires
  `NSAccessibilityAnnouncementRequestedNotification` on the window from
  `node_added` (label present, `live != Off`) and from `node_updated`
  (label changed, or `live` changed, or the node newly passed the
  filter) — `src/event.rs:237` and `:301`, mapping `Assertive` to
  `NSAccessibilityPriorityHigh` and everything else to Medium.
  `accesskit_windows` 0.35 raises `UIA_LiveRegionChangedEventId` from
  exactly the same two places (`src/adapter.rs:256`, `:315`).
  `accesskit_atspi_common` 0.20 emits `ObjectEvent::Announcement` from
  `add_node` and from `notify_property_changes` when the name changes
  (`src/adapter.rs:72`, `src/node.rs:619`). Three platforms, one rule:
  **a named node that is live, newly added or newly renamed.**
- **The platforms do have one-off APIs; AccessKit does not expose
  them.** macOS's announcement notification is what
  `accesskit_macos` posts, but only from that diff; Windows has
  `UiaRaiseNotificationEvent`, which `accesskit_windows` never calls (no
  hit for `Notification` in its sources); AT-SPI's announcement is the
  object event above. Reaching them directly would be three platform
  implementations of one thing — the option ADR 0001 rejected in its
  first paragraph.
- **Liveness inherits, and the inheritance is one code path.**
  `accesskit_consumer::Node::live()` falls back to the parent's when a
  node declares none (`src/node.rs:906`), and all three adapters ask the
  consumer, not the raw node. So a container marked live makes its text
  descendants live, and the string the user hears is the *changed*
  descendant's name — which is how a live region behaves in a browser and
  is what a view will expect. (This is the mechanism decision 3 first
  built on, and the platform did not honour it; see **What macOS actually
  does**.)
- **kui elides plain structure.** `access::derived_role` returns `None`
  for a box that is not a control, an editor, a scroll container, a key
  sink or chrome, and an elided node's semantic descendants attach to the
  nearest semantic ancestor. A `<box live>` wrapping a `<text>` would
  therefore vanish, and the text would inherit liveness from the window
  instead of from it. Liveness has to make a node semantic or it does not
  survive the walk.
- **kui already has four drained channels and they all work the same
  way.** `Core::take_window_commands`, `take_audio_commands`,
  `take_warnings` and the pending-event queue: the core appends, a driver
  drains, a headless test asserts on what it drained and a headless
  driver may simply never call. The conformance corpus's own report is
  built this way — `run()` drains window commands after each step and
  `take_warnings()` once at the end (`conformance.rs:1842`, `:1868`), so
  "what happened during this scene" is already a drain and not a reading
  of the last frame. The report keeps one frame; it keeps every warning.
- **The access tree, by contrast, is derived lazily and only on demand.**
  `Core::access_tree` builds on the first call after a frame and caches
  by frame number (`runtime/dispatch.rs:444`); the bridge asks only while
  assistive technology is attached, and publishes only when the tree hash
  moved. Anything carried *in* the tree is therefore invisible to a frame
  nobody asked about — fine for a property of the frame, wrong for an
  event that happened during it.
- **kui already answers "this, once" for sound, and the answer has two
  halves.** `audio.rs` documents three ways in: declarative props, the
  retained `<audio>` element reconciled by key (present means playing,
  gone means stopped, a finished one-shot stays mounted so a re-render
  does not replay it), and imperative `Core::play` / `stop` / `pause`.
  The element is for sound that is a property of the view; the call is
  for sound that is a consequence of an event. Announcements divide the
  same way, and the declarative half of the division is the `live` row.
- **Only two of the four bindings can act between frames.** Node and C
  hold the core, so `ctx.announce(...)` sits naturally in an event
  handler. Rust's `App::on_event` takes no `Ui`, and Lua's `env` exists
  only inside `view` — so in those two the call is made from the frame
  builder, guarded by the app's own state. That is a footgun worth a
  diagnostic, not a reason to change the shape: `Ui::reveal`,
  `Ui::focus_next` and `env.set_focus` are all called from exactly there
  already.
- **Nothing here is an ABI break.** ADR 0006, decision 2: an [in] struct
  gaining a field does not bump (`KuiSpec` is host-allocated and
  library-read — *a rule withdrawn 2026-09-14 by ADR 0006's amendment,
  backlog AR50*), and a new function does not bump (a host that does not
  call it is unaffected; one that does fails to link, loudly). A new
  [out-array] struct has no older layout to overrun. So the C side is
  additive at ABI 5.
- The next free `PROPS` wire id is 80 (`P_WINDOWS = 79`).

## Decision

1. **One schema row: `live` (id 80), on every node, in every binding.**
   `Kind::Enum(&["off", "polite", "assertive"])`, defaulting to `off`,
   landing on `AccessSpec` beside `role` and `label`, threaded through
   JSX, Lua, `KuiSpec` (`KUI_LIVE_*`) and `docs/props.md` like `expanded`
   is. It says: when the text inside this node changes, a reader should
   read the change without being asked — politely, at the next pause, or
   assertively, interrupting.
2. **A live node is semantic.** `derived_role` returns `Role::Group` for
   a node whose `live` is not `off` and which nothing else gives a role
   to, so a live region survives elision. `role="none"` still wins: a
   subtree declared decorative leaves the tree, `live` or not.
3. **A live region reads as one message.** It is presentational, like a
   button or a heading: the text inside it is its name (name-from-content,
   which the tree already computes) rather than a node of its own, and
   that name is what moves when the message does. `AccessNode` gains
   `live: Live`, carried exactly where the view declared it.

   This is the decision the platform changed — see **What macOS actually
   does** below. The shape first built was the ARIA one: the region
   carries only liveness, the platform consumer inherits it down, and
   each platform announces the *changed descendant's* name. It does not
   work: on macOS a live static text announces nothing at all, and where
   the inherited shape does work it announces once per live descendant
   rather than once per message. One named node is one announcement,
   everywhere.
4. **A one-off announcement is an imperative act, not a declaration.**
   `Core::announce(text: &str, live: Live)` appends an
   `Announcement { text: String, live: Live }`; `Ui::announce` forwards
   to it, so a view can call it too. `Live::Off` is a no-op, so a caller
   can gate politeness without an `if`. An empty string is a no-op as
   well — all three platforms require a name, so there is nothing to
   deliver.
5. **`Core::take_announcements() -> Vec<Announcement>` is the door**, the
   fourth of the four drained channels, with the same properties as the
   other three: the core appends, the driver drains, a headless test
   asserts on the drain, an undrained core accumulates. No cap, for the
   same reason the other three have none — a channel that behaved
   differently from its three siblings would be a rule to remember.
6. **The runner drains every frame, attached or not.** The windowed
   drivers call `take_announcements` after each frame and each input
   dispatch, and discard the result when no assistive technology is
   attached — so a real app never accumulates, and an announcement is
   never delivered stale, minutes after the thing it describes.
7. **The bridge delivers an announcement as a live node, because there is
   no other route through AccessKit.** For each drained announcement the
   bridge appends one node to the update: `Role::Status` (ARIA's status
   region — an `AXGroup` with the `AXApplicationStatus` subrole on macOS,
   and *not* `Role::Label`, whose announcement macOS derives from the
   node's value), `live` set from the announcement, `label` the text,
   parented to the root, with a
   **fresh node id** from a reserved descending range (`u64::MAX` down;
   `Key` is a hash of a tree path and never issues these). A fresh id
   makes every announcement take the adapters' `node_added` path, so the
   same string twice in a row is spoken twice — which the
   `node_updated` path, keyed on the label having changed, would silently
   swallow. The nodes of the most recent announcing frame **stay in the
   tree until the next announcement replaces them**: UIA's live-region
   event carries no text and the client reads the name back afterwards,
   so a node removed in the same update it was announced in is a race.
   They read as a trailing status region, which is what an ARIA `status`
   region is.
8. **Two diagnostics.**
   - `live-region-without-name`: a node declares `live` and neither
     carries a `label` nor has any text in its subtree, so nothing it
     ever does can be announced. Same family as `image-without-label`.
   - `announcement-repeated`: the same text was announced on two
     consecutive frames with no event handed to the app between them.
     That is what an unguarded `ui.announce(...)` in a view looks like,
     and it is never what an app means — a message genuinely repeated is
     repeated across frames the user did something in between, and "did
     something" is read off the events, not the frame count: a window
     that redraws only on input makes two Copy presses two consecutive
     frames (amended 2026-09-11, when the macOS audit's copy-twice check
     raised it). The announcement still goes through; the warning names
     the frame builder that is shouting.
9. **The corpus reports announcements, per scene, like warnings.**
   `run()` drains after each step and each frame and `report()` writes an
   `announce <politeness> <text>` line per announcement, before the
   `warn` lines; `Expect` gains an `announcements` list. `NodeRow` gains
   the declared liveness (`-` / `p` / `a`) so the row half is pinned too.
   A `live` scene covers both, and all four binding adapters reproduce it.
10. **One phase.** ADR 0001 split delivery because its derivation was
    unbuilt and its contract was not. Here the derivation exists, the
    contract is one row and one verb, and the bridge change is a dozen
    lines; splitting would cost more than it settles.

## Considered options

- **An edge-triggered declaration instead of a call** — an `<announce>`
  element, node-less and keyed like `<audio>`, firing on the frame its
  key or text first appears. This was the closer of the two and it lost
  on a specific failure: to say the same thing twice an app must vary the
  key, so "Saved" after a second save needs `announce-{save_count}`, and
  an app that gets that wrong is silent rather than noisy — the failure
  mode you cannot hear. Worse, keys are path hashes, so a list that
  re-orders or a subtree that remounts re-announces everything inside it
  for reasons that have nothing to do with the message. And the honest
  version of a declared announcement — a string the view keeps declaring
  because it is part of the view — is a live region with a node behind
  it, which is decision 1. A second declarative spelling of the same
  thing would have to answer why an app should ever pick it.
- **Announcements carried in the derived `AccessTree`** (a field beside
  `nodes`, materialised by the bridge). Rejected: `access_tree` is built
  on demand and cached per frame, so an announcement made on a frame the
  driver never asked about is lost, and one made on a frame nobody asked
  about *twice* is lost twice with no trace. The corpus would also have
  to grow a second collection mechanism, since its report keeps one
  frame's tree and every scene's warnings.
- **A bounded queue** (keep the newest N, drop and warn). Considered
  because an undrained channel grows without limit. Rejected: the three
  existing channels have exactly this property and it has never been a
  defect, because every real driver drains every frame and decision 6
  makes that a rule rather than a habit; a cap on one of four would be a
  special case to explain in four bindings' docs.
- **Reach the platform announcement APIs directly** —
  `NSAccessibilityAnnouncementRequestedNotification`,
  `UiaRaiseNotificationEvent`, AT-SPI's `Announcement` — since two of the
  three are genuinely better fits than a synthetic node. Rejected on ADR
  0001's premise: that is three platform implementations, in the drivers,
  of something AccessKit already maintains a diff engine for, and it
  would leave kui carrying platform code for exactly one feature. It is
  the right thing to ask *AccessKit* for, and decision 7 is written so
  that adopting it later changes the bridge and nothing else.
- **A `status` / `alert` role instead of a `live` row.** ARIA has both
  spellings. Rejected: AccessKit has a `Role::Status`, but no adapter
  reads it for liveness — all three read `live()` — so a role would have
  to be translated into the row anyway, and it would collide with the
  role a view actually wants on the node (a `list` of results is a list).
  The bridge does now *emit* `Role::Status` for a live region, which is
  the right thing on the platform side and still not the thing a view
  declares.
- **`aria-atomic` and `aria-relevant` now.** AccessKit has
  `is_live_atomic`; nothing in kui needs it yet, and the whole-region
  reading it asks for is what a `label` on the live node already gives.
  A row when a view asks (see Follow-ups).
- **`Live` from the `accesskit` crate as the core's type.** Rejected for
  ADR 0001's reason, unchanged: the contract would be another crate's
  type, each binding needs its own string spelling regardless, and the
  bridge's mapping is a three-arm `match`.

## Consequences

- `AccessSpec` grows one `Live` field (a `u8`-sized enum, inside the
  already-boxed access group, so `NodeSpec` does not move — C15 boxed it
  to 224 bytes and this keeps it there). `AccessNode` grows the same
  field. `Core` grows a `Vec<Announcement>` and the text of the last
  announcement, for the repeat check.
- The four bindings each gain one prop row (mechanical: Lua and Node get
  it from the schema tables, C gets a `KuiSpec.live` field and
  `KUI_LIVE_*`) and one verb — `Ui::announce` / `Core::announce`,
  `ctx.announce(text, live)` on both Node classes, `env.announce(text,
  live)` in Lua, `kui_announce` in C. C also gains
  `kui_take_announcements` and a `KuiAnnouncement` [out-array] row.
  **No ABI bump** (ADR 0006, decision 2), and `index.d.ts` regenerates.
- A view that announces from its frame builder announces every frame
  unless it guards. That is the cost of decision 4 and the reason for
  `announcement-repeated`; the documented pattern is a state field the
  handler sets and the view clears, exactly as an app already does for a
  one-shot sound.
- An app with no assistive technology attached pays one `Vec::drain` per
  frame and nothing else. Deriving the access tree is still gated on
  attachment; the `live` row costs the derivation one field copy per
  semantic node.
- The last announcement stays readable in the platform tree until another
  replaces it. A user navigating to the end of the window finds the last
  status message there. This is deliberate (decision 7) and it is what an
  ARIA status region does, but it does mean the tree has a node the view
  never declared, and `ax-audit` has to know that.
- A live region that is a plain box is no longer elided (decision 2), so
  it appears in the tree as a `group` — an `AXGroup` with the
  `AXApplicationStatus` subrole on macOS. Because it is presentational
  (decision 3) the nodes inside it stop being nodes of their own: a live
  region is one message, and a view that wants several separately
  navigable live things declares several regions. That forecloses a live
  *log* — a chat transcript whose rows a reader walks — inside one
  region; mark the rows, or the newest one, instead. The docs say to mark
  the smallest node that holds the message, which is the same advice for
  a different reason.
- The C read side carries liveness as two `flags` bits
  (`KUI_ACCESS_LIVE_POLITE` / `_ASSERTIVE`), not a new `KuiAccessNode`
  field: that struct is [out-array], the host allocates it, and appending
  to it would be an ABI break (ADR 0006). `KuiSpec.live` is appended
  freely, being [in].
- Two more warning codes for the existing examples to satisfy;
  `live-region-without-name` is the one an app will hit, by marking a
  container live before putting anything in it.
- **What this does not do.** No `aria-atomic` (whole-region reading), no
  `aria-relevant` (which mutations count), no `aria-busy` (suppress while
  updating). No queue discipline of kui's own — the order announcements
  are spoken in, and whether a polite one is dropped when an assertive
  one arrives, are the platform's and the reader's decisions, and kui
  passes politeness through rather than arbitrating. No route to the
  platforms' native one-off announcement APIs while AccessKit does not
  expose them. Nothing about *when* a view should announce, which is the
  hard part and is the app's.

## What macOS actually does with them (2026-09-06)

`scripts/ax-audit.swift` grew an eighth section and 8 checks, and it is
the reason decisions 3 and 7 read as they do. The headless tests pin the
data; only the OS can say whether it speaks.

The audit had to grow a new *kind* of check to do it. Every other fact in
the access tree is an attribute a reader can ask for; an announcement is
a **notification** — `AXAnnouncementRequested`, posted on the window with
the text under `AXAnnouncementKey` and a priority under `AXPriorityKey`,
which is exactly what VoiceOver listens for. So the audit registers an
`AXObserverCreateWithInfoCallback` and spins a run loop, rather than
walking the tree. Two mechanics are worth writing down for whoever
repeats it: `AXObserverCreateWithInfoCallback` is the constructor that
carries the `userInfo` (the plain `AXObserverCreate` callback does not,
and would report an announcement with no text), and registering the same
notification on the **application** element as well as the window
delivers every announcement twice — the audit registers on the window
alone and the doubling is a bug in the observer, not in the app.

- **A live `Role::Label` never announces.** This is what sent decision 3
  back. `accesskit_macos` derives a live node's announcement from
  `NodeWrapper::label()`, and that method reads the node's **value**, not
  its label, for the five roles `label_is_exposed_in_value` names —
  `Label` among them, which is what kui's `staticText` maps to. So the
  ARIA-shaped tree (liveness on a container, the changed static text
  inside it) produced a correct-looking access tree, a correct
  `TreeUpdate`, and complete silence. Naming the region from its content
  and eliding the text fixed it, and is the better shape anyway: one node
  changes, so one announcement is raised, without depending on
  `accesskit_consumer`'s inheritance or on how each platform picks the
  string.
- **The announcement node had the same defect, from the same line.** It
  was `Role::Label` for the obvious reason — it is a piece of text — and
  it was silent for the same reason. `Role::Status` is what it should
  have been from the start: it is literally an ARIA status region, and
  macOS gives it the `AXApplicationStatus` subrole.
- **Politeness arrives.** A `polite` announcement carries
  `NSAccessibilityPriorityMedium` (50); `assertive` would carry High
  (90). The audit pins the number, so a mapping that collapsed the two
  says so.
- **The same message twice in a row is said twice** — the check that
  pins decision 7's fresh node id. Reusing one id takes the adapters'
  `node_updated` path, which fires only when the label *changed*, so the
  second "Copied to clipboard" would have been swallowed. With a fresh id
  it takes `node_added`, which is unconditional. This is the failure a
  headless test cannot see at all: the tree is identical either way.
- **The standing announcement is visible in the tree**, at the end of the
  window, which the audit checks rather than assumes. macOS does not need
  it — its notification carries the text — but UIA's live-region event
  does not, so the node has to outlive the update it was announced in.

Nothing here is a platform gap of the kind ADR 0001 recorded for
`expanded` and `set_size`: both halves reach VoiceOver today. The two
findings were kui's own, and both were in the bridge.

### Follow-ups

- **`aria-atomic`.** One flag on the live node, when a view wants a
  region read whole rather than by the part that changed.
- **Ask AccessKit for an announcement in `TreeUpdate`.** Decision 7's
  synthetic node is a workaround for a missing API on a crate kui
  already depends on, and two of the three platforms have the real thing
  behind it. Worth filing upstream; the bridge is where the change
  would land.
- **`required` / `invalid` and heading `level`** are still ADR 0001's
  open rows, unchanged by this.
