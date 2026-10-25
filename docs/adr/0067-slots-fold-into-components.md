---
status: accepted
date: 2026-10-25
---

# Slots fold into components; a slot is where extensions contribute

> **Accepted (2026-10-25); backlog C86.** Supersedes [ADR 0014](0014-slots-an-extension-fills-in-place.md)
> and [ADR 0045](0045-a-slot-replayed-by-its-host.md); amends [ADR 0032](0032-a-devtools-tab-mounts-a-slot.md)
> and [ADR 0060](0060-components-an-extension-provides.md). Settled by a
> grilling round with the user; built in two steps, the fold and then the
> new slot.

There is one way to place what an extension draws: a **component**. The
old slot — one named place a frame, filled by the extension loaded under
its namespace — was a component placement with fewer powers and an event
path of its own, and folds into it. The word **slot** comes back with the
one meaning a component cannot have: a place the host declares where
**any number** of extensions append components.

## Context

- A slot name is `namespace/name`, and the namespace is bound when the
  extension is loaded (`extension_as("app", ext)`). The host never chose
  *who* fills a slot beyond that binding — which is exactly what a
  component placement already does. The two differed only by history:

  | | slot (ADR 0014) | component (ADR 0060) |
  |---|---|---|
  | key | `parent.str(full_name)`, once a frame (`duplicate-slot`) | by position or `key`, as often as nodes |
  | events | an untyped `reply`, routed through `asked_by` | the host's own message, typed (ADR 0047) |
  | replay | opt-in, `slot_kept` / `slot_replay` (ADR 0045) | by default, `fresh` to opt out |
  | host children | none | by group |
  | read-back | `slot_fill(name)` | the placement's answer, inline |

  ADR 0060's own context called the reply inconsistent with ADR 0047.
- Two extensions could never fill one slot: namespaces are unique. What
  plugins cannot do today is *contribute* — three extensions each adding a
  button to one toolbar. Costed before deciding: every per-instance
  structure (keys, fill ranges, `kept_instances`, heard, emit routing) is
  keyed by the instance, so N contributions under one host position need
  only a declaration, a verb and a door per binding — about 600–900 lines
  of code and 400 of tests.
- The stock controls (ADR 0034) were looked at in the same round and are
  **not** this ADR's subject: they stay as they are; their name is a
  backlog entry of its own.

## Decision

### The fold

1. **One placement verb.** `component(name, props, children)` in every
   binding places what an extension provides. `Extension::slots()` merges
   into `components()`; an extension lists the names it provides, and
   `"*"` still provides every name under its namespace.
2. **An extension's `view` is its component `root`.** An extension that
   provides nothing else is placed as `ns/root` after the host's view,
   unless the host places `ns/root` itself — the convention the runner's
   app and every one-file plugin rely on, unchanged.
3. **Typed messages only.** The untyped reply goes: `EventKind::Reply`,
   `asked_by`, `MAX_REPLY_HOPS`, `kui_reply`, Odin's `reply`, and a Lua
   `on_event` return value read as a reply. An extension speaks to whoever
   placed it by `emit`, arriving as the message the placer declared.
4. **Replay by default**, `fresh` to opt out; `slot_kept` and
   `slot_replay` go.
5. **Keyed by position**, host children by group, as any component.
   `duplicate-slot` goes with the name-keyed key.
6. **No read-back by name.** `slot_fill(name)` / `slotFill` /
   `kui_slot_fill` / `env.slot_fill` / `slot_fill_why` go; a placement
   answers inline where the binding can (Rust, C, Odin), and the devtools
   show the rest.
7. **A devtools tab places a component**: `devtools_tab { name, label,
   component = "ns/panel" }`, its instance placed with the panel's facts as
   props while the tab is shown.
8. **Renames.** `Slot` (an instance's name, namespace, params and key) is
   `Instance`; `SlotFill` is `FillAnswer`; `Slot::root()` is
   `Instance::root()`. `unknown-slot` and `recursive-slot` fold into
   `unknown-component` and `recursive-component`. Lua's `fill {}` element
   goes: a script places with `component(...)`.

### The new slot

9. **`slot(name, props)` declares a contribution point**: Rust
   `Ui::slot`, Lua `slot(name, props)`, Node `<slot name props/>`, C
   `kui_slot`, Odin `slot`. Its names are a space of their own, never
   read as `namespace/component`: any string, by the convention
   `owner.place` (`app.toolbar`, `files.actions`), the owner the
   declarer's stable name and never a namespace a host chose at load.
10. **An extension contributes by naming a slot and one of its own
    components**: Lua `contributes = { ["app.toolbar"] = "save_button" }`
    or a list `{ { slot = "app.toolbar", component = "save_button", order =
    10 } }`; Rust `Extension::contributions()`; C an optional
    `kui_ext_contributions` export; Odin a helper beside
    `ext_components`. The component must be one the extension provides,
    checked at load. One extension may contribute several components to
    one slot.
11. **Order** is load order, then an optional `order`, stable-sorted.
    Contributions are laid out as siblings in the host's current
    container — no wrapper node; a host that wants a box wraps the slot.
12. **Each contribution is a component instance** keyed
    `slot_key.str(provider).str(component)`, so adding or removing a
    plugin leaves the others' keys, focus and scroll where they were. It
    is kept and replayed per instance; a slot placed inside a kept fill
    journals an op of its own so a replay runs the provider loop again.
13. **Props only.** Every contributor gets the slot's props; its `inst`
    says which slot it was placed in. No host children in a slot.
14. **Who raised an event** rides in the envelope as `from`, the
    provider's namespace: Lua's `at.from` (already there for plugins),
    Node's `ev.from`, a field appended to C's `KuiEvent`. The host's
    message stays its own.
15. **The verb answers the number of contributions** in Rust, C and Odin;
    Node answers nothing, as for any placement.
16. **No slot warnings.** An empty slot is silent, and a contribution
    whose slot is not placed this frame is silent — whether a slot is
    drawn at all is the app's call (a closed panel, another page). The
    devtools tree shows a slot's contributions under it.

### Rollout

17. **One break, one release.** Every door above that goes is listed under
    "What breaks" with its replacement; the Node wire protocol and C's ABI
    bump. Pre-1.0: no deprecated aliases, which would have kept two event
    paths and two replay modes alive.
18. Two commits: the fold, then the new slot.

## Considered options

- **Keep slots as sugar over components** (`slot(name)` = a placement
  whose provider is the namespace). Rejected: it renames a component
  placement and keeps the word for nothing a component cannot do.
- **Call the contribution point an "island".** Considered; the user kept
  "slot" for it — a place where something can append components is what
  the word says.
- **Contributions later**, the word reserved. Rejected: the word changes
  meaning once, in the release that removes the old slot, with one
  migration.
- **Host children in a slot.** Each contributor would build the host's
  children again — legal, and odd; props only until a use shows up.
- **`$from` inside the message.** Rejected for the envelope: the message
  is the host's.
- **An `unknown-slot` warning** for a contribution whose slot never
  appeared. Rejected for now: not drawing a slot is a legitimate choice.
- **A component face for the stock controls** (`kui/button`). Rejected:
  a second way to place the same button at a higher cost per instance.

## Consequences

- One event path, one replay mode, one key rule for everything an
  extension draws; the C, Odin and Node surfaces shrink by the slot doors.
- Extensions can extend each other and the host without the host naming
  them — the plugin model ADR 0014 promised, now with many providers.
- Every existing slot user migrates: the runner and Kit use components
  already; the examples' `features/slots` pairs, the devtools-tab
  examples, kawoosh's wildcard views and the tests move to components.
- `Instance::root()` replaces `Slot::root()` in about sixty tests and
  doctests.

### What the building changed

Settled while the fold was built, beyond the grilling:

- **An extension's `root` is drawn fresh every frame**, never replayed:
  an app's state moves through timers, tasks and dispatch, which no
  replay check can see. A component placed inside it is replayed as any.
- **A component nested in a kept fill journals its key**, so a replay
  places it at the key it had and its own kept nodes are found.
- **`MAX_REPLY_HOPS` is `MAX_EMIT_HOPS`**, kept for an emit that passes
  from guest to guest.
- **A devtools tab's props merge under the panel's facts** (the facts
  win), and the tab's instance is recorded as placed by the host, so its
  emits reach the host as the messages its `on*` props declare.
- **An event's `slot` field is `instance`**, in every binding, so the
  word is free for decision 9; Lua's envelope carries `component`, the
  instance's full name, beside it.
- `FillAnswer`'s codes are kept with `Moved` gone (Heard stays 6), and a
  C placement answers `KUI_FILL_NONE` (-1) when nothing was kept.

### Migration

| Was | Is |
|---|---|
| `ui.slot("ns/name")`, `ui.slot_with(name, params)` | `ui.component("ns/name", props)` |
| `ui.slot_kept`, `ui.slot_replay` | `ui.component(...)` (replayed by default; `fresh: true` to opt out) |
| `ui.slot_fill(name)`, `slot_fill_why` | the placement's answer; the devtools Perf tab |
| Lua `fill { name=, params=, keep=, replay= }` | `component(name, props)` |
| Lua `slots = { "panel" }` | `components = { panel = function(env, props, inst) … end }`, or `view` for `root` |
| Node `<slot name params keep replay/>` | `<component name props/>` |
| Node `ctx.slotFill(name)` | — |
| C `kui_slot`, `kui_slot_kept`, `kui_slot_replay`, `kui_slot_fill` | `kui_component` |
| C `kui_ext_slots` | `kui_ext_components` |
| C `kui_reply`, Odin `reply` | `kui_emit`, `emit` |
| `EventKind::Reply` | `EventKind::Component` |
| `devtools_tab { slot = }` | `devtools_tab { component = }` |
| `Slot`, `SlotFill`, `Slot::root()` | `Instance`, `FillAnswer`, `Instance::root()` |
| an event's `slot` (`UiEvent::slot`, `KuiEvent.slot`, `ev.slot`, `at.slot`) | `instance` (`at.component` beside it in Lua) |
| `unknown-slot`, `recursive-slot`, `duplicate-slot` | `unknown-component`, `recursive-component`, — |
| — | `slot(name, props)` + `contributes`: many providers |
