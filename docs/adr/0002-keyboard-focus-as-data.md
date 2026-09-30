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
   keyboard; it hands focus on with `Ui::focus_next` when it wants to.
   (What the sink hears *after* it has handed focus on is the amendment at
   the end of this file, and
   `docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`.) On
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

## Amendment: a press inside a sink, built (2026-09-05)

The consequence above — "`take_key_focus` changes meaning for apps that
relied on it winning back focus every frame after a click elsewhere. None
of the examples did" — was true when it was written and stopped being true
the moment `examples/rust/splitmux.rs` landed. Splitmux owns its whole
keyboard through one `on_key` sink and draws its panes *inside* that sink,
each with an `on_click` so a click can focus one. Decision 2 makes a box
with `on_click` a derived button, and so focusable; decision 5 makes the
sink's per-frame declaration one-shot. Together: the first click on a pane
moved focus to the pane, the sink's next declaration was not an edge, and
every ⌥ chord in the app was dead for the life of the process — with no
way for the app to notice, because focus moving is not an event it sees.

That is not a splitmux problem. It is the shape of every app that owns its
keyboard and draws surfaces to click: a multiplexer, a canvas with
handles, a game with a HUD, a diagram editor. Decision 3 already says why:
*a key sink is an app that owns its keyboard*, which is why it keeps Tab.
The pointer had simply not been held to the same rule.

So, refining decisions 2 and 3 rather than reversing either:

9. **A primary press inside a key sink leaves the keyboard on the sink.**
   The press resolves to the nearest enclosing sink, not to the node it
   landed on — a pane, a derived button, a `focusable` row, or dead space,
   which used to blur. Those nodes stay focusable and stay in the Tab
   ring; being focusable makes a node a Tab stop, not a claim on keys the
   app drawing it already owns. A node that owns a keyboard in its own
   right still takes focus from inside a sink: an editor (with its caret
   placed where the click was, as ever) and a nested sink. A `disabled`
   node is not a sink at all (decision 6), so it neither answers nor hides
   a live sink above it.
10. **A press on window chrome leaves focus exactly where it is.** A
    `window_drag` strip or a window button is the platform's control, and
    decision 2 already keeps it out of the Tab ring for that reason;
    dragging a window to another monitor is not the app being asked to
    give up its keyboard. Previously it blurred, because chrome is not
    focusable and a press on nothing focusable blurred.

Nothing changes outside a sink: a click on a button in ordinary content
focuses that button, and a click on the background still blurs.

An app whose keyboard leaves for something genuinely outside its sink —
splitmux's tab bar, which sits above the sink and whose tabs are real
controls that should focus — still asks for it back with `focus(key)`, the
imperative call decision 5 exists to provide. That is the case the original
consequence describes, and it is the right one to make an app spell out:
it is the app choosing to overrule a focus the user moved.

`cargo test -p kui-native --example splitmux` drives the example headlessly
through the `App` trait and presses ⌥v after a pane click, a tab click and
a titlebar grab; `crates/kui-core/tests/keys.rs` pins the three rules on
the shapes themselves, including the editor that must still win.

## Amendment: a key has two codes, built (2026-09-05)

Chasing the sink bug above turned up a worse one underneath it. `code` was
the character the active *layout* produced, and every keymap in the repo is
written in Latin — `match code { "v" => split, "t" => new_tab }`. On a
Cyrillic, Greek, Hebrew or Arabic layout the key US-QWERTY prints V on
produces `м`, `ω`, `ה`, `ر`. Not a different arm: *no* arm. Splitmux was
silently keyboard-dead for a large fraction of the world, and nothing in
the app could detect it — the events arrived, they just matched nothing.

Pure scan codes are the obvious fix and the wrong one. Bind position and a
Dvorak user pressing the key **printed V** gets whatever chord QWERTY keeps
at that slot; the docs say ⌥v, the keycap says V, and the app disagrees
with both. Neither view is right on its own: position is right about the
Cyrillic case and wrong about the Dvorak one, and the layout is exactly the
reverse.

So both, with a stated default:

11. **`code` prefers the label, and falls back to the position.** A press
    carries what the layout produced and which key produced it, and
    `KeyPress::from_layout` resolves the two: while the layout's key is
    ASCII it wins, so a chord lands on the key the user can see (Dvorak's
    ⌥v on the key printed V, AZERTY's ⌘a on the one printed A, QWERTZ's ⌘z
    on the one printed Z). When it is not — or names nothing this
    vocabulary knows, as a dead key does — the US-QWERTY key at that
    position stands in, so a Latin keymap keeps matching; as Shift prints
    it, since the position is reported unshifted (`J`, `:` and `~` on a
    Russian layout, not `j`, `;` and `` ` ``; F76, 2026-09-21) — except under
    Alt, where the unshifted position stands in, since ⌥ composes a
    character (⌥⇧J is `Ô` on a US Mac) and the chord a keymap names is
    ⌥⇧j, as the winit runner always reported it (RG27, 2026-09-25). This is the rule
    browsers use to keep ⌘C copying on a Russian layout, and it is the
    reason an app can stay ignorant that layouts exist. `text` is
    untouched: the typing view is always the layout's own character.
12. **`physical` is a payload field of its own.** The US-QWERTY key at that
    position, in the *same* vocabulary as `code` — `"v"`, `"1"`, `"left"`,
    `"f5"` — so an app switching a keymap from one to the other keeps its
    match arms byte-identical. A keymap that wants the finger rather than
    the label reads it: WASD stays a square on AZERTY, where `code` would
    make it ZQSD. It rides on every key event in every binding.

The derivation lives in `kui_core::KeyPress::from_layout`, not in the winit
runner, so a host with its own windowing gets it by handing over both codes
— C through the new `physical` argument on `kui_input_key_down` / `_up`,
Node through the trailing `physical` argument on `keyDown` / `keyUp`. NULL
or omitted means "the key I named", which is what a host that does not
track positions says, and what every existing call already meant.

`crates/kui-core/tests/keys.rs` pins the fold for Russian, Greek, Hebrew and
Arabic and the pass-through for Dvorak, AZERTY and QWERTZ; splitmux presses
⌥v on a Russian layout and on a Dvorak one and checks it splits both times,
for opposite reasons.

### Rejected

- **Replace `code` with the position outright.** The clean model, and it
  makes every Latin non-QWERTY user's chords disagree with their own
  keycaps. Position is the fallback because it is right less often than
  the label, not more.
- **Leave `code` alone and add `physical` beside it.** Purely additive and
  fixes nothing: the naive binding stays the broken one, and every app has
  to be rewritten one at a time to get what it already assumed it had.
- **W3C `code` names for `physical`** (`"KeyV"`, `"Digit1"`, `"ArrowLeft"`).
  Unambiguous about being a position, at the price of a second vocabulary
  in the docs, a second name table in C and Lua, and match arms that no
  longer line up with `code`'s. Reusing the one vocabulary makes switching
  a keymap between the two a one-word edit.

## Amendment: what a sink hears once it is not the focus, built (2026-09-06)

Decision 3 answers the Tab half of an app that owns some of its keyboard:
a sink keeps Tab and hands the ring on with `focus_next`. It has no answer
to the half that comes next. Once focus is on a control, `route_key`
resolves nothing — a sink hears a press only while it holds focus — so an
app shell that hands the ring on goes deaf, and the pomodoro in backlog F7
loses the Space that starts its timer the moment its slider becomes
reachable. Both field reports on alpha.6 found it, from opposite ends: one
had a ring and no shortcuts, the other shortcuts and no ring.

The rejected option above, "let a key sink give up Tab to the ring", is
still rejected and still for the same reason. What was missing is not a
way to take keys *from* a sink but a way to give it the ones nothing else
wanted:

13. **A press the focused node does not claim goes to the nearest
    enclosing sink.** A focused control keeps the keys the core presses it
    with — Enter and Space where there is a click payload, a slider's
    arrows, a composite's arrows, Home, End and type-ahead — and Tab stays
    the ring's wherever focus is. Every other press, chords included,
    walks up to the first non-disabled `on_key` ancestor, which hears it
    as it hears everything today. A sink that holds focus keeps every key,
    exactly as decision 3 says.

The rules, the alternatives (a `keys` allow-list on the sink, claiming by
role, bubbling dynamically), the phase question and the modal boundary are
in `docs/adr/0011-keys-bubble-to-the-enclosing-sink.md`. Decisions 2, 3, 5
and 9 are unchanged; decision 3's `focus_next` is now half of a pattern
rather than a whole answer.

## Amendment: a key that acts on the focus shows it, built (2026-09-09)

Decision 4 names two ways focus starts showing — a Tab step and an
assistive-technology request — and one way it stops: a mouse press. That
is the whole of `:focus-visible` as a *starting* rule and only half of it
as a live one. A browser also turns the indicator on when the keyboard is
used on focus a click placed, and kui did not:

Click the `+1` button in `examples/rust/counter`, then press Space. The
count goes up — Space presses the focused control, decision 3 — and
nothing on screen says which of the three buttons answered. Press Tab
first and the ring appears, and from then on Space looks like it works.
So the honest reading of the report this came from is not "Space does not
press the button" but "I cannot tell that it did".

4a. **A key the core acts on the focused control with shows that focus.**
    Space and Enter pressing it, a slider's arrows, a composite's arrows,
    Home, End and type-ahead — the same set decision 13 says a control
    keeps. Escape is the exception, because it acts by letting go and a
    ring around nothing is not a ring; and a key that bubbles to an
    enclosing sink (ADR 0011) never reaches this arm, so a shortcut
    handled above the control does not pop a ring around it.

Composite motion already did this (`docs/adr/0007`, decision 5, through
`move_within_composite`), which is the same rule reached one pattern at a
time. The clearing rule is untouched: the next mouse press hides it again.

### Rejected

- **Show on any key, as the browsers do.** They set the flag on *every*
  keydown that is not a modifier, wherever it goes. In kui that would
  light a ring on a control while a shortcut layer above it handled the
  key — the app's own surface answering, and the ring naming a node that
  did nothing — because ADR 0011 sends unclaimed keys past the control.
  "The core acted on this node" is the fact the ring is drawn from.
- **Show it on the click.** A pointer user gets a ring around whatever
  they last touched, which is the noise `:focus-visible` exists to
  remove, and the app that reads `focus_visible` for its own styling
  loses the distinction entirely.

## Amendment: rings, not a ring — and the two rules beside them (2026-09-10)

Decision 2 says every control is in *the* Tab ring, and decision 5 says an
`autofocus` editor "takes focus only while nothing else holds it". The
first is now "in the ring of the region it is in", and the second is an
edge rather than a standing claim; a third rule, that a key sink on the
root hears keys when nothing is focused, closes the gap that made shells
take focus on the root. All three are
`docs/adr/0022-focus-regions.md`; decisions 2, 3, 5 and 8 here are
otherwise unchanged.


## Amendment: where a key is, the modifier keys, the locks, built (2026-09-28)

A terminal speaking kitty's keyboard protocol asked for the keyboard
whole (backlog F108, from kawoosh): the protocol reports the keypad's
digits apart from the main block's, the left Shift apart from the
right, the modifier keys pressed and released on their own, Caps Lock
and Num Lock, F13 to F35, and the media keys. A press carried none of
it — the keypad's `1` was the main block's `1` in both codes, the
modifiers were only ever held, nothing said what was locked, and the
runner dropped every key it had no name for.

14. **A key says which of its twins it is.** `location` —
    `"standard"`, `"left"`, `"right"`, `"numpad"` (`KeyLocation`) — rides
    on every key payload, as winit and the DOM report it. `code` and
    `physical` stay what the key is: the keypad's `1` is still `"1"` and
    its Enter `"enter"`, so every keymap written against decision 11 is
    unchanged, and one that cares reads the place. A press and its
    release are matched by place as well as position, so the two Shifts
    and the two `1`s are two keys held.
15. **The modifier keys are keys only to a sink that asks.** `shift`,
    `ctrl`, `alt`, `super`, `capslock`, `numlock`, `scrolllock` are codes
    of their own, delivered to a sink that says `modifierKeys`
    (`NodeSpec::modifier_keys`, `KuiSpec.modifier_keys`) with the side in
    `location`, and to no other. A keymap is the common case, and a
    keymap reading `<leader>F` would see Space, Shift, F — the Shift a
    key between two others, the sequence broken — so the default is the
    old reading: a modifier is held, in the next key's `shift` / `ctrl` /
    `alt` / `super` and the `modifiers` event, and never pressed. A
    modifier key's own event carries the state *after* it — Shift's
    press has `shift`, its release none unless the other Shift is still
    down — as a terminal speaking kitty's protocol reports it; winit
    tells the modifiers after the key, so the runner sets that bit
    itself.
16. **The lock state is the press's, not a modifier.** `caps_lock` and
    `num_lock` (`KeyLocks`) ride on every key payload beside `location`.
    `KeyMods` stays what is held down: an accelerator and the devtools
    chord compare it exactly, and a Caps Lock left on must not make
    ⇧⌘I miss. The runner asks the OS where it answers cheaply — macOS's
    `NSEvent.modifierFlags` (a Mac has no Num Lock, so it reads off, as a
    Mac terminal reports it),
    Windows' `GetKeyState` and the X server's XKB state (amended by
    backlog RG104) — and elsewhere (Wayland) tracks the lock keys' own
    presses, which knows nothing of a lock set before the app's first
    window opened or turned while another app had the keyboard. One
    record for the app, since there is one keyboard,
    and a lock key's own press reports the state it made on all three —
    what the two OS answers are read after the toggle (amended by
    backlog RG96, where Linux said what it found and each window kept
    its own).

The keys past the editing block — F13–F35, `printscreen`, `pause`,
`menu`, `clear`, and the media keys (`mediaplaypause`, `volumeup`, …) —
are named and delivered like any other; their names join the one table
`KeyCode::name` and `KeyCode::from_name` share. The doors take the place
and the locks with the key: C in bits of the `kmods` word that were
zero (`KUI_KLOC_*`, `KUI_KLOCK_*`), so a host passing only `KUI_KMOD_*`
sends what it sent; Node in the mods object; Rust as `with_location` and
`with_locks`.

### Rejected

- **Keypad codes of their own** (`"kp1"`, `"kpenter"`). Every keymap that
  binds `1` or Enter would miss the keypad's until it learned a second
  name, which is the breakage decision 11 exists to avoid. The place is
  the new fact; the key is the same key.
- **The modifier keys to every sink.** The browser model, where every
  app filters Shift out of its keydown handler. Here it breaks every
  sequence keymap on upgrade, silently, for a fact few sinks want.
- **Lock bits in `KeyMods`.** One word for the C door, and exact
  comparisons of held modifiers — accelerators, chords — start failing
  whenever Caps Lock is on.
