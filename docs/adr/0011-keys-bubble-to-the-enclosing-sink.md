---
status: accepted
date: 2026-09-06
---

# Keys bubble: a control takes what the core presses it with, a sink hears the rest

An app cannot have a keyboard shortcut and a Tab ring at the same time. A
box declaring `on_key` is a key sink, and a key sink hears a press only
while it *holds* focus (`runtime/dispatch.rs`, `route_key`), so the shell
of an app has one of two shapes: hold focus on the sink and every Tab is
the sink's, or let focus land on the controls and every shortcut is dead.
Both field reports hit it from opposite ends and neither could get out
(backlog F7).

We are making **a press the focused node does not claim walk up to the
nearest enclosing sink**, which then hears it exactly as it hears
everything today. A focused control keeps the keys the core presses it
with — Enter and Space where there is something to activate, a slider's
arrows, a composite's arrows, Home, End and type-ahead — and Tab stays the
ring's wherever focus is. Everything else, chords included, reaches the
sink above. A sink that holds focus keeps every key, exactly as ADR 0002
decision 3 says. There is **no new prop**: an app shell is an `on_key` box
around its content, which is what both reports already wrote.

## Context

- **The pomodoro (report 2.5).** A root box with `onKey` + `keyFocus`
  keeps focus through every Tab, so the three duration sliders are
  reachable by VoiceOver and not by keyboard. The report verified it both
  ways: the same app with the sink removed tabs to the sliders.
- **This is decision 3 working as designed.** A key sink is an app that
  owns its keyboard, Tab included, and ADR 0002 rejected "let a key sink
  give up Tab to the ring" by name because a modal editor, a terminal and
  a game all bind Tab. Its answer to the Tab half is that the sink hands
  focus on with `Ui::focus_next`, which works, and which neither report
  found — a sink hears the Tab press like any other and can move the ring
  itself in one line.
- **The half with no answer is what happens next.** The moment focus is on
  a slider, `route_key` resolves nothing: it takes `self.focus`, looks for
  a hit region with that key *and* an `on_key` payload, and returns false
  when the focused node is not itself a sink. So the sink goes deaf, and
  the pomodoro's Space stops starting the timer. `focus_next` hands away
  the ring and the keyboard together, and only one of those was meant.
- **Both apps want the same thing:** a few keys, globally, with the ring
  and the controls' own keys intact. That is what every app shell wants
  and what a browser gives for free, because the DOM bubbles a `keydown`
  from the focused element to the document and an app listens there.
- **The other half of the confusion (mind map, report #5).** A node that
  is both a control and a sink: the author could not tell whether
  Enter/Space would be claimed out from under their `on_key`, or whether
  Tab traversal would eat their `tab` binding. Neither happens —
  `focused_control` (`runtime/dispatch.rs`) excludes any node with an
  `on_key`, so a sink's Enter, Space and Tab are its own data — but
  nothing an app author reads says so, and the report shipped an `insert`
  alias to hedge against a collision that was never there.
- **The core already knows which presses did nothing.** The arms that act
  on a focused control end in `_ => {}`: a letter on a button, F5 on a
  slider, Backspace on a card. That set is exactly the set a shortcut
  layer wants, and it was being dropped on the floor.
- **The walk already exists, for the pointer.** ADR 0002's amendment
  (decision 9) resolves a *press* inside a sink to the sink rather than to
  the node under the cursor, by walking parents from the pressed node
  (`runtime/focus.rs`, `press_focus`). A key had no equivalent: the
  keyboard's version of "this belongs to the app drawing it" was missing,
  and that asymmetry is the bug.
- **A press arrives on two channels, in a fixed order.** The runner
  dispatches `InputEvent::KeyDown` first (the sink's channel, carrying the
  whole `KeyPress`: both codes, the modifiers, the text, the repeat flag),
  and only then the `EditKey` or `Text` the key maps to (the core's
  channel, which is what presses a control). Anything that decides where a
  press goes has to decide it on the way down, before the channel that
  would act on it has arrived.

## Decision

1. **A press the focused node does not claim goes to the nearest
   enclosing sink.** `Core::key_target` resolves each raw press: the
   focused node when it is itself a sink (unchanged), otherwise nothing
   when the focused control claims the key, otherwise the first ancestor
   declaring `on_key`. The walk is `press_focus`'s, factored out as
   `Core::enclosing_sink` and now shared by both: a `disabled` node is not
   a sink at all (ADR 0002, decision 6), so it neither answers nor hides a
   live sink further up. The sink hears the press as it hears every press
   today — same payload, same tag, same `key` on the event — so an app
   that already handles its chords handles them from a focused control
   with no new code. Nothing about the sink changes; what changed is which
   presses reach it.
2. **A control claims the keys the core acts on for it, and only those.**
   Statically, from the node and the key:
   - **Tab** is the ring's wherever focus is. A shell sink that heard
     every Tab would be this ADR's own bug in reverse.
   - **Enter** and **Space** activate — when there is something to
     activate. The test is the node's click payload, which is what
     `click_node` emits: a button, a checkbox, a row with `onClick`. A
     control with no payload (a slider drawn as a drag target, a
     `focusable` card) presses nothing, so its Space is free.
   - **The arrows** on a `slider` role, which emit the `increment` /
     `decrement` the app already handles.
   - **The arrows, Home, End and printable characters** on an item of a
     composite, which are that pattern's navigation and type-ahead
     (ADR 0007).
   - **Nothing else.** A letter, a function key, Backspace, Escape.
   A chord — any modifier but Shift — is never a control's key. It is what
   a shortcut layer is made of, so ⌘Enter on a focused button reaches the
   sink and does not also press the button.
3. **Both channels ask the same question.** The `EditKey` and `Text` arms
   consult the same claim before acting (`Core::bubbles`), so a key that
   bubbled as a raw press does not also press the control a moment later
   through the other channel. The claim is static for the reason in the
   context: the raw press arrives first, and a decision made from what a
   handler *did* would have to either re-deliver the press on the wrong
   channel (an `EditKey` payload has no `physical`, no `text`, no
   modifiers) or hold the press until the next event proved nothing wanted
   it.
4. **A sink that holds focus still keeps every key, Tab included.** ADR
   0002 decision 3 is unchanged and is the other half of the pattern: a
   shell keeps `keyFocus` on itself, hears Tab like anything else, and
   calls `focus_next` to hand the ring on — after which it keeps hearing
   everything the ring's controls do not claim. That is the app-shell
   pattern, complete: `keyFocus` for the keys before the user has touched
   anything, `focus_next` for the ring, bubbling for everything after.
5. **A bubbled release follows its press** (F3's question). Delivery is
   what makes a key held, so a claimed key is never held and has no
   release to deliver, and a bubbled one is held and resolves its release
   through the same walk against the same focus — the same sink. The
   `key_up` opt-in is untouched: a sink hears releases only by asking, and
   a sink that did not ask hears one half of a bubbled key exactly as it
   hears one half of a key it holds focus for. Focus moving mid-hold still
   releases first (`release_held_keys` runs before `Core::focus` changes,
   so the walk resolves to the sink that took the press).
6. **Bubbling stops at the modal boundary.** The walk refuses to climb
   through a node the modal scope made inert (`Core::interactive`), so a
   shell sink under its own dialog hears nothing while the dialog is up.
   The alternative is an app running commands against a surface whose
   state the user cannot see, which is the whole of what ADR 0003 means by
   inert. A sink *inside* the modal hears what the modal's own controls do
   not claim, as any sink does.
7. **With nothing focused, nothing bubbles.** There is no node to walk up
   from, and picking a sink out of a frame that holds several would be
   arbitrary. This is what happens today, so nothing regresses; an app
   whose shortcuts must work before the user has touched anything declares
   `keyFocus` on its shell, which is decision 4's pattern and what both
   reports already wrote.
8. **No new row, and no new event.** Nothing is added to `PROPS`, to
   `CUSTOM`, to the payloads or to the C ABI. Four bindings get this by
   rebuilding against a core that routes one more press.
9. **A sink is resolved from the tree, not from the hit list**
   (amended 2026-09-22, backlog F79). Every step above reads the frame's
   tree — `key_target`, `enclosing_sink`, the claim, the modal
   boundary — and only the delivery itself used to look the sink up
   among the pointer's hit regions, which is a different question: a
   hit region is where a *point* finds a node, and a node the frame
   drew outside its scroller's clip is under no point at all, so it
   has none. A key reaches a node by holding focus, which a node keeps
   wherever it is drawn, so `deliver_to_sink` and the `key_up` opt-in
   ask the tree (`Core::sink_node`, which still refuses a `disabled`
   node and one a modal shut out, as decisions 1 and 6 say). The
   pointer's rule is untouched: a click past the clip still finds
   nothing.

## Considered options

- **A `keys` allow-list on the sink** (`onKey={handler} keys={['space',
  'r', 'cmd+k']}`), the shape the backlog names as the alternative.
  Rejected on three counts. It needs a chord vocabulary in data — every
  binding parsing `"cmd+shift+k"`, a second spelling of the modifiers that
  already ride on the payload, and a decision about what `"space"` means
  when the layout does not produce one. It answers only the Tab half: the
  list says which keys the sink hears, not which keys the *control* keeps,
  so an app still has to know that its focused slider will swallow the
  arrows, and the list has to grow to say so. And it asks the app to
  enumerate, up front, keys it cannot know: a shell wants "everything my
  controls did not want", which is a fact about the frame, not about the
  shell. Bubbling states that fact once, in the core, for every app.
- **Do nothing; the answer is `focus_next`.** It is a real answer to the
  Tab half and it is why decision 3 was right to reject giving up Tab. It
  is not an answer to the second half: after `focus_next` the sink is
  deaf, and the only way back is `focus(sink)` on every keypress the app
  cannot hear. Both reports had already found the shape and stopped there.
- **Claim Enter and Space by role instead of by payload** — a focused
  slider eats Space because sliders are controls. Rejected: it takes the
  key away from the app *and* does nothing with it, which is the worst of
  both, and it is exactly the pomodoro's complaint (its Space starts the
  timer, and its sliders have nothing to press). Claiming only what the
  core would act on keeps the rule honest: a key the core drops is a key
  the app can have. It also matches the ARIA reading, where Space
  activates a button and does nothing to a slider.
- **Bubble what nothing handled, dynamically** — deliver to the control,
  see whether an arm fired, and forward the leftovers. The backlog's own
  phrasing ("the core already knows which presses did nothing") points
  here, and the channel order forbids it: the sink's press arrives before
  the channel that would handle it, so the sink would hear its shortcuts
  one event late, on a channel whose payload has no modifiers and no
  positions. Static claiming gets the same set with the payload the sink
  is documented to receive.
- **Let a sink give up Tab after all**, so a shell can hold focus and
  still be tabbed out of. Rejected again, for ADR 0002's reason: a
  terminal, a modal editor and a game bind Tab, and no structural test
  tells those apart from a shell (splitmux's sink encloses focusable
  panes and binds chords; a terminal's encloses nothing and binds Tab).
  Decision 4 gives a shell the same outcome in one line of its own code.
- **Bubble out of a focused editor too.** Deferred, deliberately. An
  editor claims the whole printable keyboard plus the editing keys, so
  what is left is chords — and the clipboard chords are handled *above*
  the core, in the runner (`crates/kui/src/lib.rs`), which the core cannot
  see, so ⌘C in a text field would reach the shell and copy. Neither
  report asked for it: the mind map wants typing while typing. What would
  change it is an app that wants ⌘K from inside its search box, and the
  change is a claim function for editors beside decision 2's — plus moving
  the runner's chord table into the core so both agree.
- **A capture phase**, sinks hearing every press on the way down and
  marking it handled. Rejected: it makes an event two-directional and
  stateful, needs a `handled` flag crossing four bindings, and makes every
  app's shell see every keystroke of every editor.
- **A separate `shortcut` declaration**, a global table of key to payload.
  Rejected: it is the allow-list with a new element, and a shortcut layer
  that is not a node in the tree has no answer for modal scope, for
  windows, or for two shells in two panes.

## Consequences

- **The pomodoro works, for one added line.** Its root sink keeps
  `keyFocus` and hears Space, `r` and the rest from the moment the window
  opens; the line it gains is a `focus_next` when it hears Tab; the ring
  then walks the three sliders; the arrows nudge the focused one and every
  other key still reaches the shell. The mind map's shell is the same shape, and its
  `insert` alias can go: a sink's Enter, Space and Tab were never at risk,
  and now the README says so.
- **A control's own keys can be taken by a chord.** ⌘Enter on a focused
  button no longer presses it. That is the intent — a chord is a
  shortcut — but an app that had bound ⌘Enter to "press the focused
  thing" now hears it in the shell instead and presses what it likes.
- **Escape reaches a shell that is listening, and blurs where none is.**
  A control's Escape was an undocumented blur; it is now the shell's when
  a shell exists, which is what "Escape closes this" needs. An app with a
  root sink that ignores Escape leaves focus where it is, and Tab still
  moves it.
- **A sink hears more presses than before**, including presses of keys it
  does not bind, from anywhere in its subtree. That is the point, and it
  is the same volume a sink already hears while it holds focus.
- **Two sinks nested is a real hierarchy now.** The nearest one wins and
  the outer one hears nothing, so a pane's own keymap shadows the shell's
  the way an inner handler should. An app wanting both forwards from the
  inner sink.
- **`press_focus` gained the modal guard** by sharing the walk: a press
  inside a modal can no longer resolve the keyboard onto a sink outside
  it. No scene or example did that; it is the pointer half of decision 6.
- **Tested** in `crates/kui-core/tests/key_bubbling.rs`: the ring moving
  under a shell, a letter bubbling and its release following, a space and
  an Enter pressing the button instead, a slider keeping its arrows and
  giving up its space, a chord bubbling past a claim, the nearest sink
  winning, a disabled sink hearing nothing, Escape both ways, and the
  modal boundary. The corpus's `keys` scene gains the shell over a ring,
  so all four bindings reproduce it byte for byte.
- **What is still one keyboard away:** an editor's leftovers (deferred
  above), and a sink hearing a press that landed on a *window* it does not
  own, which is ADR 0004's routing and not this one's.
