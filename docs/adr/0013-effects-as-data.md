---
status: accepted
date: 2026-09-07
---

# Effects as data: what `update` returns besides the model

> **Accepted and built (2026-09-07), for alpha.8.** This is backlog F23,
> from the pomodoro's alpha.7 report (wish 3). The report's concrete case
> turned out to need no new API — the first section says why — so what
> this ADR decides is narrower than what was asked. It was proposed with
> *do nothing* as the live option and reviewed the same day for the
> release; what changed the answer is the price and the audience. The
> whole of it is one brand check, two queues and a flush in `createLoop`,
> additive and opt-in — a plain return means what it meant, and an app
> with no handler and no `withEffects` cannot tell it is there — and both
> alpha.7 reports asked for the shape in so many words even though their
> concrete case was covered. Shipping it in the alpha those reports are
> answered by costs less than a third report that asks again; the risk it
> carries is an API shape no view has exercised, which is what the two
> tests and the README's example are for until one does.
>
> **Two things were decided in the building that the draft had not said.**
> `init` may return `withEffects` too — Elm's `init` returns a `Cmd` for the
> same reason `update` does, and the file an app opens at start has
> nowhere else to go (decision 1). And a tick that returns effects without
> a model draws nothing, so the frame that would have handed them on never
> comes; the loop's `step` hands them on itself in that case, which keeps
> decision 3's order — `update`, then whatever drawing there was, then
> effects — without a frame the tick did not ask for. `runWindowed` drains
> `app.effects()` every pump, as ADR 0008's rule for the core's channels
> requires, so a window with no handler does not accumulate (decision 4).

An app has one loop: `update(model, msg, event, surface)` returns the
next model, `view(model, window, surface)` returns a tree, and everything
the core does on the app's behalf comes back out as data — the display
list, the events, the audio commands, the window commands, the
announcements — so a headless test asserts on all of it. The pomodoro's
report says its chime "is the app's one side effect and it costs a model
field (`alarmCount`) plus a module global (`chimed`) purely so the driver
can notice a counter move and call `win.play`", and asks for "an Elm-style
`[model, effects]` return — or just letting `update` push a command the
driver drains".

## Context

### The chime needs nothing new, and the entry has to say so first

Three things the report describes as missing are there:

- **`update` has the surface.** Its fourth argument is the `Ctx` or the
  `KuiWindow` the loop runs on, since D4. `surface.play(id, opts)` from
  inside `update` is the "push a command the driver drains" the report
  asks for: headless, `audioCommands()` is the drain, and the pomodoro's
  own `headless.tsx` already asserts on it (`app.ctx.audioCommands()
  .some((c) => c.kind === 'play')`).
- **`<audio>` is declarative.** `<audio key src>` is a playback retained
  by key: present is playing (once, or looped), gone is stopped. A chime
  is `m.alarm && <audio key={\`chime-${m.alarmCount}\`} src={m.chime}/>`
  in the view — the count that exists to key it stays, the `chimed`
  global and the `update` wrapper in `main.tsx` go. `update` stays pure,
  and the command still lands in `audioCommands()` for the test.
- **Resources reach the model.** `init(surface)` runs after `setup`, so
  the sound id is a model field rather than a module-level `let`.

So the cost the report measured is the app's, not the library's, and the
"what you can delete" line for it is in the CHANGELOG beside this ADR's
backlog entry. What the report is right about is the shape underneath.

### What survives: an effect kui does not know

`play` is an effect kui owns, so it has a command type and a drain.
Everything kui does *not* own — write a file, send a request, copy to the
clipboard, start a timer that is not `tick`, open a URL — has three homes
today, and each is wrong in a way a test can feel:

1. **In `update`, done directly.** `update` is impure; a headless test
   either performs the effect for real or mocks the module the effect
   lives in. Nothing in `app` records that it happened.
2. **In `update`, recorded in the model** (the pomodoro's `alarmCount`),
   with a driver that diffs the model to notice. The model carries a
   field that is not state, the driver keeps a shadow of it (`chimed`),
   and "did it fire" is inferred from two numbers rather than read.
3. **In the driver, keyed off a message.** `runWindowed`'s `update`
   wrapper intercepts messages on their way in — which is what the
   pomodoro built — so the effect's cause is a message rather than a
   state change, and the headless driver has to duplicate the wrapper or
   go without.

Elm's answer is that `update` returns `(Model, Cmd Msg)`: the effect is a
value, the runtime performs it, and its result re-enters as a message.
kui's premise — ADR 0001's, the README's — is the same sentence about the
core: nothing the core does is a side effect it performs, everything is
data it emits. This ADR extends that sentence to the app's own effects.

### What "effects as data" costs, measured in the code as it stands

- `createLoop` (`packages/kui/index.js`) is the one place `update`'s
  return is read: `dispatch`, `ticksTo`, and the drag / access / key
  paths all go through `app.dispatch`. A second return shape is one
  check there.
- The model may be anything — an array in a test, a number in the
  counter example — so a tuple return `[model, effects]` is ambiguous and
  a `{ model, effects }` object collides with any model that has a
  `model` key. The shape has to be branded.
- Lua and C have no `update`: an app on those bindings is a loop the host
  writes. This ADR is Node's, and the Rust `App` trait's if it wants it;
  it changes no schema row, no ABI struct and no core type, which is why
  it can be proposed without an IR discussion.

## Decision

The decisions are written so that a view can argue with them; all six are
built, in `packages/kui/index.js`, and the two notes in the status block
say where the building changed them.

1. **`update` may return `withEffects(model, ...effects)`**, a branded
   value the loop unwraps. A plain return is a model with no effects, as
   today; `undefined` keeps the model, as today; `withEffects(undefined,
   ...)` keeps the model and still queues the effects. Nothing existing
   changes meaning. `init` may return one too.

2. **An effect is the app's own value.** kui does not define an effect
   vocabulary; an effect is whatever `E` the app declares
   (`createApp<M, A, E>`), the way a message is. The exception is kui's
   own effects, which stay where they are: `play` is a surface call or an
   `<audio>` node, a window is a `windows(model)` declaration. A second
   spelling of those would be the thing ADR 0008 refused for
   announcements.

3. **The loop hands each effect to one handler, after the frame.**
   `createApp` / `runWindowed` take `effects: (effect, dispatch, surface)
   => void`. The order is: `update` → draw → effects, so an effect that
   dispatches synchronously (a "done" message) lands in the next turn and
   not inside the frame that caused it, and an effect handler that reads
   the surface sees the frame its cause produced.

4. **Headless, effects are also a queue.** `app.effects()` drains what
   `update` returned since the last drain, whether or not a handler ran —
   a test asserts on the value without a handler, the way it asserts on
   `audioCommands()` without a device. With no handler declared, the
   queue is the only place they go, and it is bounded by the same rule
   as the core's channels (ADR 0008, decision 6): a real driver drains
   every frame.

5. **Results re-enter as messages, by the handler's `dispatch`.** There
   is no `Cmd Msg` type: an effect handler that has something to say
   calls `dispatch(msg)`, which is `app.dispatch` and goes through
   `update` on the next `step`. A promise-shaped effect is the handler's
   business.

6. **`runWindowed` and `createApp` share the handler** through the same
   `createLoop`, so an effect a headless test asserted on is the effect
   the window performs, from the same `update`.

## Considered options

- **A tuple return `[model, effects]`.** The Elm spelling, and the one the
  report names. Rejected because the model is untyped by the loop: an app
  whose model is an array returns a tuple today, and the loop cannot tell
  the two apart. The brand is one symbol and one helper.
- **A kui-defined effect vocabulary** (`{ kind: 'play' }`, `{ kind:
  'open' }`, …) with the app's own under an escape hatch. Rejected: kui's
  effects already have data shapes — `AudioCommand`, `WindowCommand`,
  `Announcement` — reached by declarations and surface calls, and a
  second route to the same queues has to explain when to use which. The
  app's effects are the only ones with nowhere to go.
- **`update` pushes to a queue on the surface** (`surface.effect(e)`) —
  the report's second spelling, and the closest to `play`. Rejected
  narrowly: it keeps `update` impure in the type (a call, not a return),
  and it puts an app value on a surface whose other queues are all the
  core's. It would be right if the surface were the app's; it is the
  core's.
- **Effects from `view`**, keyed like `<audio>`. Rejected for ADR 0008's
  reason: a one-shot spelled as a declaration must vary its key to fire
  twice, and the failure mode is silence.
- **Do nothing** — the chime is covered, and no other effect has been
  asked for. This was the live option while the status was *proposed*,
  and the status block says what outweighed it: the price of building
  against the price of being asked a third time.

## Consequences

- Built: `createLoop` has the brand check (`apply`), the `effects` option,
  the two queues and the flush; `index.d.ts` has `withEffects`,
  `WithEffects`, `EffectHandler`, an `E` parameter (defaulting to `never`)
  on `LoopConfig` / `Loop` / `AppConfig` / `App` / `WindowedConfig` /
  `WindowLoop`, and `effects()` on the loop; the READMEs carry the shape
  and the testing section its assertion. No crate changes. The Rust `App`
  trait is a separate decision and can follow the same shape or not.
- The pomodoro's chime is *not* the motivating view — its fix is the
  `<audio>` line above, and that stays the answer for any effect kui owns.
  The first app whose effect kui does not own should say in its report
  which of homes 1–3 it had been living in, and whether the handler's
  `dispatch` was enough for the result to come back.
- What this ADR does not touch: the core, the IR, any binding's schema,
  the C ABI, and the meaning of any existing return from `update`.
