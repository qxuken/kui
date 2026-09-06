---
status: accepted
date: 2026-09-06
---

# Press-drag-release into a popup: the driver retargets the opening press

A native select on every platform is one gesture: press the field, the
menu opens under the pointer, drag through it, **release** on an item.
kui has the other half of the pair — click to open, click to choose —
and the popup example is built on it. The gesture itself cannot be
built by an app, because the OS gives the whole drag to the window that
received the mouse-down: pressing in the owner and releasing over the
popup delivers nothing to the popup and everything to the owner, in
coordinates that mean nothing to it. We decided that **the driver
retargets it**: a non-activating popup that opens while its owner's
primary button is down **joins that press**, the owner's moves are
translated into the popup's space and fed to the popup's core as
ordinary `CursorMoved`s, a release over the popup is synthesised into
the press-and-release the popup never saw, a release anywhere but the
popup or the anchor dismisses it, the press that dismisses a popup is
**consumed** rather than passed through, and the app opens on
**`onDrag`'s `start` phase** — no new row. The popup's core never learns
it was retargeted; the only headless part is the arithmetic.

## Context

- ADR 0004 decision 9 made a popup a window kind: borderless, owned,
  anchored in screen coordinates, non-activating, and dismissed by the
  same `{kind:"dismiss", reason}` ADR 0003 gave a modal. C11 step 4 built
  it (2026-09-06). `examples/rust/popup.rs` is a combobox on it, and it
  opens on `on_click` — so press-and-hold does nothing until release,
  which is right for what it declares and wrong for a native select.
- **The measurement** (backlog W2). Press inside the popup and drag up
  out of it: every move still arrives at the popup's own core, with
  coordinates walking off its top edge — `93,-1`, `95,-5`, `97,-10` —
  and so does the release. The OS routes a captured drag to the window
  of the mouse-down, on all three platforms. So for the real gesture,
  pressed on the field in the owner and released over the popup, the
  popup would receive nothing at all. This is what `NSMenu`'s nested
  tracking loop, Win32's menu message loop and a GTK pointer grab exist
  to do, and none of them is reachable through winit.
- **Those coordinates are not garbage.** The driver knows both windows'
  screen positions — `Shell::popup_position` computed the popup's from
  the anchor, and `Window::inner_position` answers for either at any
  time — so a point in the owner's space is a point in the popup's space
  by two subtractions. `93,-1` in the popup, mapped back, lands on the
  anchor: the pointer had been dragged up onto the field.
- What the driver can and cannot see. It cannot see "the press that
  opened this popup": the app opens the popup a frame later, in answer
  to the `drag start` (or `click`) the press produced, and the `Open`
  arrives in the redraw after the press. It *can* see that a popup is
  opening while the owner's primary button is held, and it can see where
  every later move and the release land.
- Today's press-outside rule (`Shell::window_event`, the `MouseInput`
  arm): a press in any window but the popup reports `dismiss` on the
  popup **and then dispatches the press** to the window it landed in.
  The comment says this mirrors ADR 0003. It does not, quite: under an
  in-window modal, everything outside the modal emits no hit region, so
  the outside press dismisses and lands on nothing. A popup's owner is
  live (ADR 0004 decision 10, modality is per-window), so there the
  press dismisses *and* acts. That difference is invisible with
  click-to-open and decisive with open-on-press, as decision 5 shows.
- The core's pointer model (`Interaction::handle`,
  `crates/kui-core/src/input.rs`): a press on an `onDrag` node emits
  `{kind:"drag", phase:"start"}` at once, before any slop; moves emit
  `move` once past the slop; the release emits `end`, and a `click` only
  if the press never moved. A press captures the node — `is_pressed`
  holds while the pointer wanders — and hover keeps following the
  pointer, so an out-of-bounds move clears hover. There is no "on press"
  event; `start` is a press by another name, with a capture and an
  `end` that a bare press event would not have.
- An `onDrag` node derives a `grab` cursor. The `cursor` row exists to
  override a derivation that cannot know better, and this is one.

## Decision

1. **A popup that opens during a press joins the press.** When
   `Shell::open_pane` opens a non-activating `WindowKind::Popup` while the
   primary button is down in some pane, that popup is *armed* for the
   rest of that press. This is the observable form of "the drag whose
   press opened the popup": the press-outside rule guarantees that a
   press in the owner while a popup is up dismisses the popup, so a popup
   opening under a held button was opened by that button, unless the app
   declined a dismissal — in which case it is still the popup the user is
   pressing towards. The rule is evaluated once, at open, and needs no
   geometry. A second popup opening during the same press (a submenu the
   app opens from a retargeted hover) joins the same press, whoever owns
   it, so the driver keeps a list; where two overlap, the last opened is
   on top.
2. **Moves are translated and fed as ordinary input.** While armed, every
   `CursorMoved` the pressed pane receives is mapped to screen and back
   into each armed popup's space (decision 7's arithmetic). Inside a
   popup's rect it is dispatched to that popup's core as
   `InputEvent::CursorMoved`, which is all the popup needs: hover,
   `hover_bg`, `onHover` and its own drag state follow from it. Leaving
   the rect dispatches one `CursorLeft` to the popup it left, so a row
   stops highlighting when the pointer drags off the list, the way a
   native menu's does. The popup's core is not told any of this is
   synthetic; it is ordinary input from a driver that owns both windows.
3. **The owner sees exactly what it sees today, on purpose.** The pressed
   pane's core still receives every move with its own out-of-bounds
   coordinates: its hover clears, its pressed node stays pressed
   (capture), and if the field declared `onDrag` it receives `move`
   events it should ignore. That is the right picture for the owner —
   nothing in it should light up under a drag that belongs to the menu,
   and the field looking pressed while the menu is tracked is what a
   native popup button does. No event is withheld from the owner and none
   is added.
4. **The release is classified by where it lands.** On the primary
   release in the pressed pane, while armed:
   - **over an armed popup** — the driver dispatches
     `MouseDown { Primary, clicks: 1 }` then `MouseUp { Primary }` to
     that popup's core at the translated point, straight into
     `Core::handle_input` rather than through the driver's `MouseInput`
     arm. The popup's core produces the `click` it would have produced
     for a real press-and-release there, with the focus move, pressed
     styling, click sound and `pressed == hovered` check that entails.
   - **inside the anchor** — nothing extra. The press opened the menu,
     the release on the field keeps it: this is how the gesture
     degrades into the two-click interaction, which stays complete.
   - **anywhere else** — `dismiss` with reason `outside` on every armed
     popup. A native menu closes when the pointer is dragged off it and
     released, and "outside" is the truth about where the release
     landed.
   The owner's core then receives its own `MouseUp` as today, which
   ends its drag and, out of bounds, clicks nothing; inside the anchor
   it clicks the field, and decision 6 makes that harmless. The press is
   disarmed. The same classification applies to a press that **began in
   a popup**: a release outside the popup and outside its owner's anchor
   dismisses it, so the measured drag-off-the-top ends the way it does
   natively.
5. **The press that dismisses a non-activating popup is consumed.** A
   primary press in any window but the popup still reports `dismiss`
   with reason `outside`, and is then **not dispatched** to the window
   it landed in; its release is swallowed with it. This replaces today's
   dismiss-then-dispatch, for non-activating popups only. It is what
   ADR 0003's outside press actually does (it lands on nothing), what
   every native menu does (the first click closes the menu and nothing
   else), and the only ordering under which open-on-press works: with
   the press passed through, a second press on the field would dismiss
   and then reopen, and no ordering of the two events lets the app tell
   that press from the first. The owner stays live for everything else —
   hover, the wheel, the next press. An **activating** popup (a tear-off
   panel, `activates: true`) keeps the pass-through it has, since a user
   working in a panel beside the app expects a click in the app to act.
   The two halves of the press-outside rule the entry asked about are
   met by construction: the opening press cannot dismiss the popup it
   opens, because the press is evaluated at mouse-down and the popup does
   not exist until the redraw after it; and the release is a release,
   evaluated by decision 4 and never by the press-outside rule, while the
   synthesised press of decision 4 enters the popup's core directly and
   is never seen by that rule either.
6. **A popup opens on `onDrag`'s `start`; there is no new row.** `start`
   is delivered on mouse-down, before slop, on the node under the
   pointer, with a capture that holds the field pressed until release
   and an `end` that closes the gesture — everything an `onPress` row
   would carry and two things it would not. The app's protocol is
   therefore **everything opens, only a dismissal or a choice closes**:
   `start` on the field sets `open`; `click` on the field sets `open`
   too, which is the keyboard (Enter, Space), assistive technology and a
   stationary release — idempotent after the `start` that already opened
   it; `dismiss` clears it; choosing clears it. Nothing toggles, so
   nothing has to know which press it is answering, and decision 5 is
   what makes a press on the open field close rather than reopen. The
   field sets `cursor: "pointer"` over the `grab` its `onDrag` would
   derive. The `move` events it receives during the gesture carry the
   owner's coordinates and are ignored.
7. **The arithmetic is a pure function, and the only headless part.**
   `crates/kui/src/retarget.rs`: a `Surface` is a window's client origin
   in **physical** screen pixels, its scale and its logical size;
   `retarget(from, p, to)` maps a logical point in one to a logical point
   in the other when it lands inside; `landing(owner, p, anchor, popups)`
   is decision 4's classification, topmost popup last. Physical pixels
   are the common frame because two windows on two monitors can have two
   scales, and the mapping has to hold on a mixed-DPI Windows desktop
   where logical coordinates do not share an origin. The unit tests pin
   the same-scale case, the mixed-scale case, the anchor, the overlap
   order and the measured `93,-1` walk-off landing on the anchor. Nothing
   else here is testable without two OS windows and a captured drag; the
   consequences say how that is checked.

## Considered options

- **Arm on "a non-activating popup owned by this window is open, and the
  drag is over its rect".** Evaluable, and the rule the entry proposed.
  Rejected: it is a fact about geometry evaluated on every move rather
  than a fact about the press evaluated once; it arms a drag that began
  before the popup and wanders over it (harmless) and, in an app that
  declined a dismissal, any later drag in the owner (not harmless); and
  it does not cover a submenu owned by the first popup, which is not
  "owned by this window". Decision 1 is the same thing observed at the
  moment that decides it.
- **Arm on "the drag whose press opened the popup".** The true
  statement, and invisible: the app opens the popup a frame after the
  press, and the driver has no way to connect an `Open` to the input
  that caused it. Decision 1 is its observable proxy, and the
  press-outside rule is what makes the proxy tight.
- **Hand the drag to the popup with an OS grab.** What the platforms do
  natively. Rejected: winit exposes no cross-window capture, an
  `NSMenu`-style tracking loop is not a window at all, and the driver
  already holds both windows and both cores — it does not need the OS to
  move the drag when it can move the events.
- **Let the popup's core know it was retargeted.** A `CursorMoved` with a
  "from window" or a dedicated input variant. Rejected: the popup does
  nothing different with the knowledge, and every headless test of a
  popup would then have to cover two shapes of the same input.
- **Synthesise the `click` event directly on the release** instead of a
  press-and-release into the popup's core. Rejected: the core's release
  path does more than emit — it moves focus, checks
  `pressed == hovered`, plays the click sound, clears pressed styling —
  and a click that skipped it would be a click the popup's own tests do
  not describe.
- **An `onPress` row.** The entry's open question. Rejected: it is
  `onDrag`'s `start` with the capture and the `end` removed, so a node
  would carry both to get either, and the schema would have two spellings
  of "the pointer went down here". If a view ever wants a press with no
  capture, that is the moment to argue for it; a combobox is not that
  view.
- **Toggle on `start` instead of set.** Rejected because of the
  stationary release: a press opens, and the release's `click` would
  close it again, and the payload of a `click` does not say whether a
  pointer or a key produced it. Consuming the dismissing press
  (decision 5) is what lets every handler set rather than toggle.
- **Keep passing the dismissing press through.** The status quo, and the
  comment in `Shell::window_event` argues for it by analogy with
  ADR 0003. Rejected: the analogy is backwards — a modal's outside press
  hits nothing — and with open-on-press it makes a press on the open
  field reopen the menu. The one thing pass-through buys, a click in the
  owner acting on the first press, is what activating popups keep.
- **Dismiss on any release outside the popup, anchor included.** Rejected:
  it deletes the two-click interaction, which is complete today and is
  how the gesture degrades when the user releases without dragging.
- **Suppress the pointer `click` on a node that declared `onDrag`,
  motion or not.** Would let `click` mean "keyboard" on the field.
  Rejected: it changes the meaning of a row that existing views carry
  on nodes that are both draggable and clickable, to save the combobox
  an idempotent assignment.
- **Open the popup under the pointer** — the current item under the
  cursor, as macOS does — rather than below the anchor. Not decided
  here: it is a placement question for `WindowConfig`, orthogonal to
  where the release is routed, and Windows and GTK open below the field.

## Consequences

- **Nothing ships to an app from this ADR.** `CHANGELOG.md` is untouched,
  as ADR 0004 left it: the changelog lists what a release adds and what
  an app can delete, and the only code here is `crates/kui/src/retarget.rs`
  with its tests, which nothing calls yet. Backlog W2 becomes *accepted,
  unbuilt*, whole in `docs/backlog/closed-2026-09.md` and trimmed in
  `docs/BACKLOG.md` to the build.
- **The build is one change in one file**, `crates/kui/src/lib.rs`:
  `Shell` gains the armed set and a per-pane "primary held"; `open_pane`
  arms; the `CursorMoved` arm retargets; the `MouseInput` arm classifies
  the release and consumes the dismissing press; `close_pane` disarms.
  Roughly a hundred and fifty lines, none of them headlessly testable
  beyond decision 7.
- **When built, one behaviour changes for existing apps**, and the
  release that builds it says so under "what you can delete": a press
  that dismisses a non-activating popup no longer reaches the owner, so
  an app that guarded against the click it was dismissed by can drop the
  guard. Nothing else an app sees changes: a popup that opens on `click`
  keeps working, with the press that dismisses it consumed rather than
  passed through.
- **`examples/rust/popup.rs` becomes the fixture**: the field gains
  `on_drag` with `cursor: "pointer"`, and its handlers become decision
  6's four assignments. Its doc comment lists what to watch, and gains
  the drag.
- **Checked by hand, with real `CGEvent`s**, the way W1 was — a swift
  script posting `leftMouseDown` on the field, `leftMouseDragged` through
  the list, `leftMouseUp` on a row, and `screencapture` for the pixels,
  since the state between press and release is otherwise unphotographable
  (`kui-macos-window-quirks` has the recipe). Four checks: a drag from
  the field onto a row chooses it and the menu closes; a press and
  stationary release leaves the menu open and a second click on a row
  chooses; a press on the open field closes it and does not reopen; a
  drag off the list and a release on the desktop dismisses it. Windows
  adds a fifth, mixed DPI across two monitors, which is what decision 7
  is in physical pixels for.
- **Not done here.** A popup placed under the pointer; a submenu's
  ownership chain beyond decision 1's "joins the same press"; whether an
  activating popup should be dismissed by an owner press at all, which
  the driver does today and nothing has argued for.
