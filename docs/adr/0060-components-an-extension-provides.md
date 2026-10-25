---
status: accepted
date: 2026-10-25
---

# Components an extension provides

> **Accepted and built (2026-10-25); backlog C60.** Kit's M2: the piece
> that lets one Lua implementation of a button, a table or a dialog be
> placed by any host as often as it likes. What the building changed is
> at the end.

An extension can provide **components**: named fills a host places inline,
as many times as it likes. Each instance is keyed by where it was placed.
An instance can hold the host's own children where its fill says, and its
events reach the host as the messages the host passed in its props. A slot
stays what it is — one place a frame; a component is a slot made cheap,
repeatable and composable.

## Context

- A slot is one place per frame: its key is `parent.str(name)`, and a name
  declared twice is `duplicate-slot`. Twenty buttons would be twenty names.
- A fill could not hold host content: `Extension::view(slot, ui)` takes
  params only, so a panel, a toolbar or a dialog could not wrap the host's
  nodes.
- An extension's answer was a `reply` with an untyped payload, while ADR
  0047 made every other event arrive as the message its handler declared.
- M0 measured the costs: a fresh Lua slot fill was 30 µs a button, a
  replayed one 1.25 µs against a native button's 0.70 µs. Replay (ADR
  0045) is what makes many instances affordable, so it has to be the
  default rather than a host's opt-in.
- Host content inside an area the core owns, under the host's origin,
  had a precedent: `Ui::devtools_tab_with`.

## Decision

1. **Placed by position.** `Ui::component(name, props)` and
   `Ui::component_with(name, props, children)`; Node `<component name props
   groups fresh>children</component>` (wire op `component`); Lua
   `component(name, props, children)`; C `kui_component(ctx, name, props,
   children, user)`; Odin `kui.component` / `kui.component_with`. `name` is
   `namespace/component`. The instance key comes from its place in the
   tree, or from a `key` in its props, so instances repeat as freely as
   nodes.
2. **An extension lists what it provides.** `Extension::components()` (Lua:
   a `components` table of functions `(env, props, inst)`; C: an optional
   `kui_ext_components` export, the same shape as `kui_ext_slots`).
   `Extension::component(inst, ui)` draws an instance, defaulting to
   `view`. A components-only extension gets no `root` fill.
3. **Host children are a door the fill calls.** `Ui::children(group)` (Lua
   `children(group)` as a node; C `kui_children`), `""` for the unnamed
   group. At that point the core restores the placer's origin, namespace
   and label scope, keys the children under `instance.str(group)` — so
   their keys do not depend on how the fill nests them — and records their
   range as the placer's slot, so their events route to the host and
   `ev.slot` does not name the component. A group asked twice, or a
   `children()` outside an instance, is `children-misplaced`. Inside the
   host's children the placer's own children are in reach, so a fill can
   forward its host's content into a component it places.
4. **Events arrive as the host's message.** Props whose name is `on` and an
   upper-case letter or an underscore are event props. The extension
   answers one of its own events with `emit(prop, fields)` (Rust
   `slot::emit`, C `kui_emit`); `Extensions::route` finds the host's
   message for that prop on the instance the event came from and delivers
   it to whoever placed it as `EventKind::Component`, the extension's data
   under the prop's field (`onSelect` and `on_select` both `select`). A prop
   the host did not pass delivers nothing.
5. **Instances are replayed by default.** An instance whose props are equal
   and whose recorded reads of the frame still hold is pushed again from
   last frame's journal without asking the extension; `fresh: true` in the
   props asks every frame. The journal gains `Op::Children`, so a replay
   builds the host's children again from this frame's builder, and
   `Op::Component`, so a component placed inside a kept fill is placed
   again (one with children of its own makes the fill unkept, since a
   replay cannot call the fill's closure).
6. **An extension places its own components in place.** The extension list
   cannot ask an extension that is busy drawing, so `Ui::component_by`
   takes the fill as a closure and keeps the instance boundary — key,
   replay, events (which reach the extension as its own message). The Lua
   binding does this for a `component` node of the script's own namespace;
   elsewhere it is `recursive-slot` with a sentence saying how.

## Considered options

- **Style tokens for the small primitives** (a token kind bundling props).
  Cannot express structure — a switch's knob — and primitives would still
  be written once per binding.
- **Per-binding primitives with Lua composites only.** Five
  implementations that drift, which is the problem Kit exists to remove.
- **Unique-name slots with a naming helper.** The env cost stays per
  instance and the names leak into keys.
- **Replay as the host's opt-in, as for slots.** M0's numbers say a fresh
  instance is 30× a replayed one; a toolbar of components would be the
  slow case by default.

## Consequences

- `Fill` gains `component` and `placed`, `Extension` gains `components`
  and `component` (both defaulted); `EventKind::Component` and
  `Event::Component` are new; `Tree::fills` may hold `NO_SLOT` ranges.
- C gains `kui_component`, `kui_children`, `kui_emit` and the optional
  `kui_ext_components` export, with the `KuiChildrenFn` callback; no ABI
  bump. Node's frame protocol moves to v26 (shared with ADR 0063's
  `svg`).
- Warnings `unknown-component` and `children-misplaced`.
- A conformance scene, `component`, in all five adapters: the reference
  spells the corpus extension in Rust (`conformance::ConfKit`), and every
  other adapter loads `crates/kui-core/tests/fixtures/component/conf.lua`
  through its own door — so the scene also checks that one `.lua`
  extension draws the same in every host (ADR 0059). Events are routed
  through the extension list, so the emitted `onClose` is in the report.
- The C example pair under `examples/c/features/slots` gained a `badge`
  component, placed by the C host with a child of its own, its press
  answered with `kui_emit`.

## What the building changed

- The draft keyed instance replay by name; instances are kept by key in a
  map of their own (`kept_instances`), so a thousand instances of one
  component are a thousand entries and nothing to scan.
- Decision 6 is new: the first test that composed two components of one
  extension met the list's recursion guard.
- Emits match by field, not by prop spelling (decision 4), since a Node
  host passes `onClose` and a Lua host `on_close` to the same extension.
- The draft's Node children were a callback per group; they are
  length-prefixed op runs, read by a reader over their own slots only
  where the fill asks.
