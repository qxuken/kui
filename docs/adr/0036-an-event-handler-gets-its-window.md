---
status: proposed
date: 2026-09-27
---

# An event handler gets the window its event came from

> **Proposed 2026-09-27** (backlog DX9, from the DX sweep of kawoosh's
> views). Nothing is built. The shape below is for review. The one thing
> it leaves open is the method's name (decision 1).

A Rust app hears its events in `App::on_event(&mut self, ev: UiEvent)`
and has nothing else in hand. Everything it wants to *do* about an event
beyond changing its model — write the clipboard, ask for a paste, move
focus, reveal a row, scroll, ask for a frame, open a menu — is a call on
the window's `Core`, and the only place a Rust app reaches a `Core` is
`view`, through `Ui`. So kawoosh parks each of those in its model for
the next `view` to carry out:

- `clip_out`, the text to copy, because "`on_event` has no `Ui`"
  (`kawoosh/src/app.rs:191`);
- `reclaim_focus`, set by ten handlers (`app.rs:224`; DX10's `keepFocus`
  removed most of the need);
- the devtools tab and alignment requests (`app.rs:137`, `:253`).

Each is a field that is not state, a branch in `view` that is not
drawing, and a frame of delay between the event and its effect. ADR 0008
(decision 6) and the closed C18 and C33 each met this and wrote around
it. None of them filed it.

## Context

- **Node already has it.** `update(model, msg, event, surface)` gets the
  `Ctx` or `KuiWindow` the loop runs on as its fourth argument since D4.
  `surface.setClipboard(…)` from `update` is the Node spelling of what
  kawoosh parks. ADR 0013 kept it that way on purpose: kui's effects
  are calls on the surface, and only the app's own effects are returned
  as data.
- **C already has it.** A C host drives its core and polls its events
  itself, so its handler holds the `KuiCtx`.
- **Lua is a guest (ADR 0014).** A script's `on_event(ev)` returns
  replies for its host to act on, and its `env` is a reading of one
  `view`. That is the design, not a gap: what a script wants done
  outside its own tree, it asks its host for (kawoosh's `kawoosh.copy`).
- **The runner can hand one over.** `Shell::route_events` calls
  `app.on_event(ev)` with `panes` (each window's `Core`) and `app` as
  separate fields of `Shell`, so it can lend the handler the pane's core
  while it runs. `ev.window` names the pane. A pumped runner's
  `route_events(events, to_app)` and `testing::Drive` route the same way.
- **The core's verbs already work between frames.** `set_clipboard` and
  `request_paste` queue menu actions the runner drains after routing.
  `reveal`, `focus_region` and the DX15 labels resolve when the next
  frame finishes. `set_scroll`, `set_key_focus` and `request_frame` ask
  for a frame. The verb table (`schema::DOORS`) already lists the
  between-frames doors a host has, and it is the same list Node's
  surface is checked against.

## Decisions

1. **`App` gains a handler that takes the window's core, and the old
   one stays.**

   ```rust
   fn on_event_with(&mut self, ev: UiEvent, core: &mut Core) {
       self.on_event(ev)
   }
   ```

   The runner calls `on_event_with`. An app that overrides only
   `on_event` behaves as it does today, so nothing breaks. The name is
   open: `on_event_with` says what changed, and `update` would match
   Node's word, but `update` in Rust usually means the model's step.

2. **The surface is the `Core`, not a new context type.** It is the core
   of the window `ev.window` names: the pane's, or the main window's for
   an event a host made itself. It is `&mut Core` because the core's
   between-frames doors already are the curated list. `DOORS` pins them
   against Node's surface and C's header. A `EventCx` with a chosen
   subset would be a fourth list to keep in step, and it would drift.

3. **What the handler does lands where the verb says.** A clipboard write
   goes out with the batch the runner drains after routing. A focus move
   or a reveal is seen by the next frame, which the verb asks for. The
   handler adds no ordering of its own.

4. **Every driver passes it.** The winit runner, the pumped runner's
   `route_events` (its `to_app` becomes `FnMut(&mut A, UiEvent, &mut
   Core)`, a break for the few hosts that call it), and
   `testing::Drive`. An event raised while a frame is being built, such
   as a slot's reply, is routed after the frame like every other, so the
   core is never lent while `Ui` holds it.

5. **Lua and C are unchanged.** C already holds its context. A Lua
   script stays a guest and asks through replies (ADR 0014). This is
   Rust's door catching up with Node's, so no verb is added to the table.

## Considered options

- **Change `on_event`'s signature.** One method, the obvious spelling,
  and a break for every Rust app on kui (kawoosh has one, the examples
  thirty-odd). The default method in decision 1 gets the same result
  without the break. Kept as the thing to do once it has shipped a
  release, if the old method is to go.
- **An `EventCx` of chosen verbs** (the backlog's first shape). Easier
  to document and to keep an app from calling `frame()` from a handler.
  But it is a second list of the core's verbs, beside the one `DOORS`
  pins, in Rust only. A verb added to the core would need adding here
  too or be missing only in Rust handlers. What it would buy — a handler
  that cannot call `frame()` and build a frame the runner did not ask
  for — is a rule the core's docs already state for any host holding a
  `Core` between frames, and a debug assertion can hold it here.
- **Effects as data** (ADR 0013's shape for Rust). Return
  `Vec<Effect>` for the runner to apply. kui's effects are core verbs,
  and ADR 0013 kept those as calls. Only the app's own effects are data.
  A Rust app's own effects are its own code.
- **Leave it**, since kawoosh's workaround works. The cost is not
  kawoosh's three fields but that every Rust app meets the same wall at
  its first clipboard shortcut, and the Node app next to it does not.

## Consequences

- A Rust handler copies, pastes, focuses, reveals and scrolls in answer
  to the event, as a Node `update` does. The model fields and `view`
  branches that parked those go. kawoosh's `clip_out`, the devtools
  requests and what `reclaim_focus` has left are the first to go.
- `route_events`' closure gains the core: a break for pumped hosts,
  listed under What breaks. `testing::Drive` routes the same way, so a
  test sees what the window does.
- Nothing in the core, the IR, the schema, the C ABI, Lua or Node
  changes.
- Built with a runner test that a handler's `set_clipboard` reaches the
  drained actions of the same turn and a `reveal` the next frame, and a
  `Drive` test that the handler's core is the window its event came
  from when two windows are open.
