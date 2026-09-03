---
status: accepted
date: 2026-09-03
---

# Keyboard focus as data: one focus, every control reachable

Keyboard focus in kui reached editors and key sinks only: Tab cycled the
editors, a click or a per-frame declaration put focus on a sink, and a
button could not be reached or pressed from the keyboard at all. We
decided that the core keeps **one** focus, that every control the access
tree knows about is focusable, that Tab walks them in tree order and
Enter / Space activate them, that keyboard-driven focus draws a ring the
view did not have to ask for, and that all of it is data: a `focusable`
row, a `disabled` row and a `focusBg` row in the schema, `focused` and
`disabled` on the access node, and a focus request from assistive
technology landing on the same focus a Tab press moves.

## Context

- ADR 0001 made semantics data and deferred keyboard reach as a focus-model
  decision. The gap it left is concrete: `Core::focus_adjacent_edit` walked
  `NodeContent::Edit` nodes only, buttons advertised `Click` but not
  `Focus`, and Enter and Space never activated anything, because Space
  became `InputEvent::Text` and Enter an `EditKey` that only an editor
  consumed.
- The same gap is why a screen reader seemed to see editors only. A
  VoiceOver cursor is pulled to whatever the application reports as its
  focused element; kui reported an editor or a key sink or nothing, and
  the remaining controls sat inside the AccessKit view as an unnamed
  group the user had to interact with by hand. AccessKit also treats a
  node as focusable only when it supports the `Focus` action, so a
  reader's own "move keyboard focus here" was a no-op on every control.
- Focus lived in two places: `EditStore::focused` for editors and
  `Core::key_focus` for sinks, with every path (click, Tab, access
  requests, autofocus) keeping the pair consistent by hand.
  `Core::is_focused` answered for editors only, so a binding could not
  draw a focused button even if it wanted to.
- `set_key_focus` was a per-frame declaration that always won ("the next
  declaration wins it back"), and the examples that own their keyboard
  call it every frame. Any focus the core moved by itself would be
  clobbered on the next frame.
- There was no `disabled`: nothing to skip in a Tab ring, nothing to tell
  a screen reader, and a disabled-looking button still clicked.

## Decision

1. **One focus, in the core.** `Core` keeps `focus: Option<Key>`; the
   edit store's focus mirrors it for editor keys (the caret, IME and
   clipboard paths read that mirror as before). `Core::set_focus` is the
   only writer. `is_focused(key)` answers for any node, `focus()` names
   it, and a `focus_visible` flag says whether the keyboard put it there.
2. **What is focusable is what the access tree calls a control.** In tree
   order of the last frame: built-in editors, custom editors, key sinks
   (`on_key`), every declarable control role (`button`, `checkbox`,
   `radio`, `switch`, `slider`, `tab`, `link`), a derived button (a box
   with `on_click`), and any node declaring the new `focusable` flag (a
   list row, a card). Never: `disabled` nodes, `role="none"` subtrees,
   window chrome (the platform's own controls are not Tab stops), plain
   structure.
3. **Tab walks the ring; Enter and Space activate.** Tab and Shift-Tab
   move to the next / previous focusable node and wrap; with nothing
   focused they enter from either end. A multiline editor (built-in or a
   custom editor role) keeps Tab as indentation, and a key sink keeps
   every key, Tab included, because a sink is an app that owns its
   keyboard; it hands focus on with `Ui::focus_next` when it wants to. On
   a focused control that is neither an editor nor a sink, Enter and Space
   emit its click payload through the path `AccessAction::Click` already
   uses, and on a focused slider the arrow keys emit the `increment` /
   `decrement` access events the app already handles. Keyboard-driven
   focus scrolls the node into view through the shared helper.
4. **Focus visible, as data and as pixels.** Focus moved by Tab or by an
   assistive-technology request sets `focus_visible`; a mouse press that
   moves focus clears it, like the web's `:focus-visible`. When it is set,
   the core draws a ring around the focused node on top of the frame, in
   the frame's own display list, so every binding gets it without drawing
   anything. A node that declares `focusBg` swaps its background instead
   (the same resolution as `hoverBg`; pressed wins over focus wins over
   hover) and gets no ring. Editors and sinks get no ring either: an
   editor shows its caret and a sink is an app surface that styles
   itself, through `is_focused` and `focus_visible` like any other state.
5. **Declaration is edge-triggered; there is an imperative call.**
   `take_key_focus(key)` (`keyFocus` in JSX, `key_focus` in Lua,
   `kui_set_key_focus` in C) now means "focus this node when it starts
   being declared": a node declared every frame takes focus once, on the
   first frame, and a Tab press afterwards is not clobbered. To move focus
   at any time a view calls `Ui::focus(key)` (`ctx.focus(key)`,
   `kui_focus`), `blur()`, `focus_next()` / `focus_prev()`. An
   `autofocus` editor takes focus only while nothing else holds it, as
   before, but through the same single writer.
6. **`disabled` is a row.** A disabled node keeps its hit region (a
   tooltip may explain why) and loses everything else: no click, no drag,
   no key sink, no hover or pressed background, no place in the Tab ring,
   and the access tree says `disabled` and drops the `Click` action.
7. **The access tree carries the same focus.** Every focusable node
   advertises `Focus` and `Blur`; `focused` and the tree's `focus` follow
   `Core::focus` for any node, so AccessKit's focus events point a
   screen reader's cursor at the button Tab landed on, and a reader's
   `Focus` request lands on the core's focus the way Tab does. Windows,
   macOS and AT-SPI adapters read `disabled` from the node.
8. **Three schema rows, every binding.** `focusable` (id 61, flag),
   `disabled` (id 62, flag) and `focusBg` (id 63, colour) join `PROPS`;
   JSX, Lua and C get them the way every simple row lands (one `npm run
   gen`, a `KuiSpec` field each, pinned by the parity test). Nothing else
   in the IR changes.

## Considered options

- **Tab through every `on_click` node and leave the rest.** Rejected: a
  checkbox or slider drawn by the app has no click payload, and a row
  that opens on Enter needs to be reachable without pretending to be a
  button. Tying focusability to the role vocabulary reuses the decision
  ADR 0001 already made about what a control is.
- **A `tabIndex` row.** Rejected: an ordering integer is how the web
  patched a document model that had no notion of tree order. kui's tree
  order is reading order, and a view that wants a different order draws
  in a different order.
- **Keep two focus stores and add a third for controls.** Rejected: the
  two existing ones already cost every path a pair of updates; a third
  would triple it, and the access tree needs one answer.
- **Let a key sink give up Tab to the ring.** Rejected: a modal editor,
  a terminal and a game bind Tab, and a sink is by definition an app
  that owns its keyboard. It gets `focus_next` instead.
- **Arrow keys within radio groups, tab lists and lists.** Deferred: the
  ARIA patterns (arrows move within a composite, Tab leaves it) need
  the composite roles' children enumerated, which the access tree can
  do, but the activation rules differ per pattern and no example needs
  them yet. A slider's arrows are in because the app already handles
  the events.
- **A focus ring the view draws.** Rejected as the only option: every
  binding would draw its own ring, or forget to. The core's ring is a
  default; `focusBg` and `is_focused` remain for views that want their
  own look.

## Consequences

- A keyboard alone reaches and operates every control in every example
  and in every binding, and a screen reader's cursor follows. The
  accessibility example needs no change beyond its comments: its
  per-frame `take_key_focus` is now one-shot by definition.
- `take_key_focus` changes meaning for apps that relied on it winning
  back focus every frame after a click elsewhere. None of the examples
  did; an app that wants that calls `focus(key)` when it decides to.
- `Core::key_focus()` keeps working and answers the unified focus.
  `Core::is_focused` widens from editors to every node, which is what
  `widgets::text_input` and the bindings' `isFocused` already assumed.
- `EditStore::declare` no longer touches focus; autofocus is the core's
  decision, so an autofocus editor cannot steal focus from a focused
  button.
- Headless tests pin the ring order, activation, slider arrows, the ring
  quad, `focusBg`, `disabled`, the access tree's focus and actions, and
  the edge-triggered declaration. What only a screen reader session can
  show is announcement and cursor following; the macOS audit script
  gains a Tab check.
- Arrow-key composites (radio groups, tab lists, lists) and a
  configurable ring colour are the next steps, both small.
