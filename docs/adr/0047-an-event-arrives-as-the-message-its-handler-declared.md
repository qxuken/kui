---
status: accepted
date: 2026-10-21
---

# An event arrives as the message its handler declared

> **Accepted and built (2026-10-21); backlog F174.** This came out of a review of the
> `<edit>` widget's API. Matching a `changed` event to its field meant
> `ev.key === ctx.keyOf('title')`, and a list of fields could not be
> matched at all. The fix grew to cover every node event. The handler's
> payload *is* the app's message, and the core writes its data into one
> field of it, named for the event, rather than wrapping the message as a
> `tag` inside a core message. The editor then takes a declared `value`,
> as the slider took `valueNow` (ADR 0034), and `labelOf` gets a door in
> every binding. This amends ADR 0034's `change` shape. Decision 6, the
> root events, is superseded by [ADR
> 0048](0048-a-window-declares-the-messages-its-events-arrive-as.md):
> they became declared messages too, and the building notes below about
> `is_root`, `KuiEvent.root` and `RootMsg` record what that ADR removed.

## Context

A node event today is the core's message with the app's message inside
it: `{kind: "drag", phase, x, y, dx, dy, parent, tag}`. `UiEvent::tagged`
([`input.rs`](../../crates/kui-core/src/input.rs)) pushes the declared
tag in as `tag`, the one funnel every row with a tag goes through. Four
costs follow from that shape.

- **The handler matches twice.** `update` switches on the core's `kind`
  (`case 'drag'`), then on `msg.tag?.kind`. In Node, `update` takes
  `AppMsg | CoreMsg`, and `CoreMsg` is 27 types
  ([`index.d.ts`](../../packages/kui/index.d.ts)).
- **The app registers messages that never arrive.** `examples/node/kui-msg.d.ts`
  registers `{kind: 'pan'}`, `{kind: 'zone'}` and `{tool}`. Each one only
  ever arrives inside `DragMsg.tag` or `KeyMsg.tag`, and it is in the
  union so the JSX prop type-checks.
- **The two namespaces collide.** An app's `Msg::Key` is `{kind: "key"}`,
  and so is every key event a sink hears. That was backlog F148, and
  `UiEvent::message` now reads the tag before the payload to work around
  it ([`message.rs`](../../crates/kui-core/src/message.rs)). Rust gets
  the app's message typed, but `dx` and `phase` are still read untyped
  from `ev.payload` beside it.
- **The editor is the worst case.** Its events carry nothing but
  `{kind}` and the key (`push_edit_event`, `runtime/dispatch.rs`). It has
  no handler row, so it cannot carry a tag. Matching means `keyOf`. Key
  labels are unique among siblings only, so in a list of rows that each
  declare `<edit key="name">`, `keyOf('name')` answers the first row and
  warns `ambiguous-key`. The app then has to keep a table of hex keys.
  Reading the text is a second call, `editText(ev.key)`. The model reaches
  the editor only through `initial`, which seeds a new editor, and
  `setEditText`, which puts the caret at the end and clears undo
  (`EditState::set_text`). So "clear after submit" and "uppercase as you
  type" are imperative calls next to an otherwise declarative view.

`Core::label_of`, the reverse of `key_of`, exists in Rust.
`schema/doors.rs` declines it in C, Odin, Node and Lua, because the
devtools inspector was its only reader.

## Decision

### The message and its data

1. **A handler's message is the event's payload.** When a node declares
   `onDrag={{kind: 'pan'}}`, a drag arrives as
   `{kind: 'pan', drag: {phase, x, y, dx, dy, parent}}`: the declared map
   plus one field. A plain click arrives exactly as declared, as it does
   today. A click that has data gets a `click` field (decision 2).
   `kind` is always the app's. The core no
   longer writes a `kind` of its own onto a node event, and no core kind
   reaches `update` for one.
2. **The field is named per event, after its kind:** `click`, `drag`,
   `key`, `text`, `preedit`, `selectionrange`, `contextmenu`, `button`,
   `scroll`, `focus`, `hover`, `drop`, `files`, `layout`, `forceclick`,
   `menu`, `dismiss`, `select`, `sound`, `change`, `boundary`. It holds
   that event's payload as it is today, minus `kind` and `tag`.
   - **A click's data moves into `click`.** Today a click on a cells
     grid, or on a sink that draws `line` rows, gets `cell`, `line`,
     `byte`, `inside` and `clicks` written straight into the app's map
     (`attach_pointer`, `runtime/dispatch.rs`), the flat merge this ADR
     declines. They move under `click: {cell?, line?, byte?, inside?,
     clicks}`. The field is present only where a click has data, so a
     plain button's click still arrives exactly as declared, and
     `WithClick` types it for the messages that want it. The derive
     reads fields by name, so a Rust message without the field still
     decodes. A per-event name
   rather than one generic name (`data`) gives each field one type, not
   a union of every event's, and a name that says which event it came
   from.
   - **One message per event, in the typed bindings.** The core would
     accept one message on two handlers, but the types can't express it.
     If `Tok` declares `hover`, then `onClick={{kind: 'tok'}}` lacks it
     and fails to type-check. If `hover` is optional, `Tok` no longer
     matches `onHover`'s prop type. So the devtools tab's `tok`, today
     both a hover tag and a click payload, becomes two messages, `tok`
     and `tokClick`.
3. **A message that is not a map** gets a map made for it. `Null`, which
   declares the behaviour and names nothing, arrives as `{drag: {…}}`.
   A string or number arrives as `{tag: "card", drag: {…}}`. The typed
   bindings take only maps (decision 10), so this is for Rust's
   `Value` tags and C.
4. **A field the map already has is the core's.** If an app declares
   `{kind: 'k', key: 'a'}` as an `onKey` message, the event overwrites `key`
   and raises `tag-field-taken` once per node, naming the field.
5. **No handler, no event.** Each event the core raises on a node gets a
   handler row, and a node that declares none of them hears nothing, the
   way a box without `onClick` is not a button. The schema's `EVENTS`
   table gains two columns, `handler` (the row) and `field` (decision 2),
   and a test pins that every node event names both. Checked against
   the tree on 2026-10-21, most node events are already gated by a row:
   `drag`, `key`, `text`, `preedit`, `hover`, `focus`, `scroll`,
   `layout`, `drop`, `button`, `contextmenu`, `forceclick`, `change`.
   The ones that are not:
   - **The four editor events** fire on every `<edit>`. Decision 7 gives
     them rows.
   - **`menu`** fires on a select's key with no message. It gets an
     `onMenu` row, which the select and the target of `openMenu`
     declare, and the choice arrives as `{…msg, menu: {role, item}}`.
     The stock right-click menu, which the app never declared and whose
     roles (copy, paste, …) the core performs itself, sends nothing.
   - **`selectionrange`** fires on a `selectable` scope when a copy
     reaches rows a virtual list has not built. It gets an
     `onSelectionRange` row on the scope. A virtual list widget that
     answers it itself declares it for the app, as the `layout` sentinel
     is already absorbed.
   - **`dismiss`** is gated by `modal`, which is already a row holding a
     message. `modal` stays the handler, and the dismissal arrives as
     `{…modal, dismiss: {reason}}`.
   - **Messages declared outside a node** take the same shape. A
     playback's message from `play(id, {tag})` and the message given to
     `requestFiles` arrive on the root as `{…msg, sound: {phase,
     playback}}` and `{…msg, files: {paths}}`. A `Null` message arrives
     as `{sound: …}`. So the rule is that an event arrives as the
     message *something* declared, not only a node.
   - **`access` stops being an event.** Today it has two sources, and in
     both the app owns the state: a hand-drawn `role="slider"` without
     `onChange`, and a custom editor (a node with an editor role that is
     not an `<edit>`). Its message is borrowed from `onClick`, else
     `onDrag`, else `onKey`. Each request now becomes the event a user
     doing the same thing would cause:
     - A slider's increment, decrement and setValue go through
       `onChange` only, as `change: {value, phase}`. Without `onChange`,
       assistive technology cannot move a slider, and the core warns
       `slider-without-handler`. The nudge path is deleted, and with it
       the bug where increment reached any enabled node whatever its
       role (`runtime/dispatch.rs`, `AccessAction::Increment`).
     - A custom editor's setValue arrives on its `onChange` as
       `change: {text}`, the same `WithEditChange` an `<edit>` sends.
       So `onChange` applies to editor roles as well as to sliders,
       typed per role.
     - replaceSelectedText arrives on `onKey` as `text: {text}`. That
       already means "insert this over the selection", which is what a
       paste or an IME commit says there.
     - setTextSelection arrives on `onKey` as `select: {anchor: {line,
       offset}, focus: {line, offset}}`.

     There is no `onAccess` row, no `access` field, and no message
     borrowed from another handler.
   - **One handler may deliver several events.** `onKey` delivers
     `key`, `text`, `preedit` and `select`, so its field type is the
     union `WithKeySink = WithKey | WithText | WithPreedit | WithSelect`
     and its helper is `KeySinkMsg<T>`. `update` tells them apart with
     `'key' in msg`. A custom editor declares one sink and handles
     everything that types into it. The `EVENTS` table's `handler`
     column is many-to-one, and the test pins the set of fields each row
     delivers. Every other handler delivers one event.
6. **What stays global** is what has no node to declare a handler on:
   `resize`, `window`, `system`, `fonts`, `modifiers`, `open`, a menu
   bar's `menu`, and a popup window's root `dismiss`. These keep the core's `kind`. In Node,
   `CoreMsg` shrinks to these and is renamed `RootMsg`. Handlers declared
   on a window could absorb them later. That is not part of this ADR.

### The editor

7. **The editor's events get handler rows.** `onChange` (the row ADR
   0034 made for the slider, which every other role ignores today) now
   applies to an editor as well. Three rows are new: `onSubmit`,
   `onCancel` and `onBoundary`. An edit arrives as
   `{…msg, change: {text}}`, Enter as the `onSubmit` message as declared,
   Escape as the `onCancel` message, and an edge as
   `{…msg, boundary: {key, edge, word, doc}}`. The `changed`, `submit`
   and `cancel` kinds go away. The text is on the event because the
   handler asked for it: an editor without `onChange` sends nothing, so a
   100 KB document that nobody listens to is never copied. A `secure`
   editor carries its text too, since `editText` already hands it over,
   but the devtools events tab and the frame trace print it as bullets.
8. **An editor takes a declared `value`, and it always wins.** On every
   frame where the declared `value` differs from the editor's text, the
   editor is brought to it. When they are equal, which is every frame of
   ordinary typing, the cost is a string compare and nothing else. The
   rules for applying it:
   - **As a splice, not a replacement.** Only the span between the
     common prefix and suffix changes, on grapheme boundaries. A caret in
     the prefix stays where it is. A caret in the suffix moves by the
     length difference. A caret inside the replaced span goes to the end
     of the new text. A selection that overlaps the span collapses.
     `setEditText` keeps its current behaviour (replace everything, caret
     at the end).
   - **Undo.** A splice applied on the frame after this editor sent
     `change` joins the undo step of the edit that caused it. Without
     this, undo after a filter would loop: undo `A` back to `a`, the
     filter makes it `A` again. Any other splice, such as a reset driven
     by the model, is its own undo step. A `secure` editor keeps no
     undo, as before.
   - **Waiting for a composition.** While an input method is composing,
     the value is held. When the composition commits, the value is
     applied if it still differs. The preedit is never cancelled by it.
   - **With `initial`.** `value` wins, and the editor raises
     `edit-value-and-initial`.
   - **Without `onChange`.** The field reverts every keystroke. That is
     React's frozen-input trap, and here it raises
     `edit-value-without-handler` once per key.
   - **Why not apply it only when it changes.** Applying `value` only
     when it differs from the last frame's declaration was considered and
     declined. A field the model had already cleared (`""`) could never
     be cleared again, since `""` to `""` is not a change.
   - **Ordering.** The pump runs events, then `update`, then the view
     (`packages/kui/index.js`, `drainEvents` before `draw`). Rust's
     `App` runs `on_event` before `view` the same way. So an app that
     stores `change.text` declares exactly the editor's text, and
     `value` never fights typing.
9. **How each binding spells `value`.** The core takes it through the
   text argument the editor already has: `EditOptions::value` is a flag
   meaning that argument is the value and not a seed. C gets
   `KUI_EDIT_VALUE` beside `KUI_EDIT_SECURE`, and `kui_text_edit`'s
   `initial` argument then carries the value. JSX gets `value?: string`,
   Lua `value =`, Odin `value` in the options, and Rust
   `Ui::text_edit_value(label, value, opts, spec)`. The stock `<input>`
   (`widgets::text_input`, `kui_text_input`, Lua `input {}`) takes
   `value` and `onChange` too.

### The helper types

10. **Each binding with types gets helpers that name a message carrying
    an event,** so the app writes its own part and the binding adds the
    field.

    Each event has three types: its data, the field that carries the
    data, and the helper that adds that field to the app's own message.

    | Binding | Event data | The field alone | Message helper | Prop type |
    |---|---|---|---|---|
    | TypeScript | `DragEvent`, `EditChangeEvent` (`{text}`), `SliderChangeEvent`, … | `interface WithEditChange { change: EditChangeEvent }`, `WithDrag`, `WithSliderChange`, … | `EditChangeMsg<T>` = `T & WithEditChange`, `DragMsg<T>`, `SliderChangeMsg<T>`, `KeyMsg<T>`, … | `onChange?: Omit<Extract<AppMsg, WithEditChange>, 'change'>` on `<edit>`, `WithSliderChange` on `<slider>`, so only a registered message with that field fits the prop, and pairing a message with the wrong handler fails at the node. The `Extract` half is exported as a util of its own (decision 12) |
    | Rust | `kui::DragEvent`, `kui::EditChangeEvent`, …, each implementing `EventField` (which names its field), and `kui::Event`, one variant per event | none: the derive does not flatten a struct into a variant | `#[message(on = drag)]` on a variant makes the derive emit the arriving twin, `MsgIn::Pan { drag: DragEvent }` (decision 13) | `.on_drag(Msg::Pan)`: the message is declared as it is, with no placeholder |
    | Odin | `Drag_Event`, `Edit_Change_Event`, … (today's structs minus `tag`) | `With_Edit_Change :: struct { change: Edit_Change_Event }`, … | none needed: the app's struct embeds the field with `using _: kui.With_Edit_Change` | `kui.decode(payload, &msg)` decodes the whole message |
    | Lua | `kui.DragEvent`, `kui.EditChangeEvent`, … classes in the generated `---@meta` file | `kui.WithEditChange`, … classes with the one field | none: LuaLS has no intersection. The app's class inherits the field, `---@class Rename: kui.WithEditChange` | none |
    | C | none: payloads are `KuiValue` maps | none | none | the field names are in `docs/props.md`, generated from `EVENTS` |

    The field type is named `With…`, not `…Payload`, because the
    payload is the whole message (see `GLOSSARY.md`). In TypeScript it
    is an `interface`, not an alias, so all three spellings of the same
    message work and mean the same thing:

    ```ts
    type Rename = EditChangeMsg<{ kind: 'rename'; id: string }>;
    type Rename = WithEditChange & { kind: 'rename'; id: string };
    interface Rename extends WithEditChange { kind: 'rename'; id: string }
    ```

    `Extract` matches all three by structure, so the prop type accepts
    each of them.

    The names reuse today's `DragMsg` and friends, with the new meaning.
    The old meaning, the core message with a `tag`, has nothing left to
    name once decision 1 lands. Old code that uses those names then fails
    to compile rather than quietly meaning something else.

### `labelOf`

11. **`labelOf(key)` gets a door in every binding:** `kui_label_of`,
    Odin `label_of`, Node `labelOf`, Lua `env.label_of`. It answers the
    label the key was opened under, in this frame so far and then the
    last one, and `null` for an auto-keyed node. The DOORS row flips
    from `No` to `Is`. It is a door and not a field on every event:
    `LabelIndex::label_of` is a linear scan, fine for one call but not
    for every hover. It answers the local label, so in a list it is
    still ambiguous. Decision 7's tags are the answer for lists, and
    this is the answer for a key from `focused()`, a frame trace or a
    log.

### The TypeScript utils

12. **The `Extract` the prop types are built from is exported**, so an
    app can name "my messages that carry this event" without restating
    it. `HandlerMsg<W>` = `Omit<Extract<AppMsg, W>, W extends unknown ?
    keyof W : never>` is what a node declares for the handler whose
    event field is `W`, and every handler prop is typed as it. It is the
    one util exported; the bare `Extract` half gets no name of its own.
    The `keyof` is distributive so that `W` can be a union, for a
    handler that receives several events (checked under `tsc --strict`
    on 2026-10-21).

### Reading an event in Rust

13. **Rust reads a message and its data in one `match`, in two
    spellings.** Neither gets TypeScript's compile-time pairing, because
    an enum's variants are not types. Both were written out by hand and
    compiled on 2026-10-21.
    - **`kui::Event`, beside the message.** `ev.split::<Msg>()` returns
      `(Option<Msg>, Event)`, and `Event` has one variant per event
      (`Click`, `Drag(DragEvent)`, `Change(EditChangeEvent)`, `Submit`,
      `Resize(ResizeEvent)`, …). It needs no macro, and root events go
      through the same `match` as `(None, Event::Resize(r))`. A wrong
      pairing falls through to `_`. It is built from the envelope's
      event kind (decision 14) and the event field.
      ```rust
      match ev.split::<Msg>() {
          (Some(Msg::Pan), Event::Drag(d)) => self.pan(d.dx),
          (Some(Msg::Rename { id }), Event::Change(c)) => self.rename(id, c.text),
          (None, Event::Resize(r)) => self.resize(r),
          _ => {}
      }
      ```
    - **The derive's arriving twin.** `#[message(on = drag)]` on a
      variant makes `#[derive(Message)]` emit a second enum, `MsgIn`
      (`#[message(incoming = "…")]` renames it), in which that variant
      carries the event's data. It is the Rust counterpart of
      TypeScript's `EditChangeMsg<T>`. A message declared on the wrong
      handler fails to decode, since its field is missing, and the core
      can say so as `message-on-wrong-handler`. It is built on
      `kui::Event`'s data structs.
      ```rust
      #[derive(Message)]
      enum Msg {
          #[message(on = drag)] Pan,
          #[message(on = change)] Rename { id: u64 },
          Save,
      }

      match ev.message::<MsgIn>() {
          Some(MsgIn::Pan { drag }) => self.pan(drag.dx),
          Some(MsgIn::Rename { id, change }) => self.rename(id, change.text),
          _ => {}
      }
      ```

    `Event` is not optional, since the envelope and root events need it.
    The twin is sugar on top of it and is built last (see the build
    order).

### The envelope

14. **The event's kind moves onto the envelope**, beside `key`,
    `origin`, `window` and `slot`, as `event`: `ev.event` in every
    binding (`'drag' | 'click' | …` in Node and Lua), `EventKind` in
    Rust, a string in C and Odin. The payload no longer says what
    happened: a plain click and a submit can both arrive as the bare
    message. The Rust decode, the devtools events tab and the corpus
    read it here. `type`, the DOM's word, was declined because it is a
    keyword in Rust and would split the spelling across bindings.

### Words

15. **"Tag" is retired.** Docs and types say "the handler's message".
    The schema's `Kind::Tag` becomes `Kind::Handler`. `tag` survives
    only as the key a non-map message is wrapped under (decision 3).
    `GLOSSARY.md` lists it as a word to avoid.

## Considered options

- **Keep the tag inside the core's message and add only the editor's
  `onChange`.** Fixes routing for fields and lists. It keeps the double
  match, the messages registered only to type a prop, and F148's
  workaround, and the editor's events would be the 28th core type.
- **A flat merge, `{...msg, ...data}`.** It reads the flattest, but it
  collides with the field names apps use most: `kind`, `text`, `x`,
  `phase`, `id`, `value`.
- **One generic field (`data`).** Simpler to explain. One message can no
  longer serve two handlers and be told apart, and the field's type is
  the union of every event's.
- **Placeholders in the message** (`{kind: 'rename', name: from.change.text}`,
  the core filling `name`). The flattest result, and the app picks its
  own field names. But it needs a new value type on all four wires, and a
  Rust `String` field cannot hold a placeholder. It can come later as
  sugar over decision 2.
- **Closures** (`onInput={text => ({kind: 'title', text})}`). Elm's
  shape, but Node and Lua only, and the tree stops being data: the
  devtools could no longer show what a node will send. ADR 0013 kept
  effects as data for the same reason.
- **`label` on every event** instead of a `labelOf` door. That is a scan
  per event, and it is ambiguous in exactly the case (lists) that
  matters.

## Consequences

- **Every handler in every binding changes.** That covers the examples,
  the tutorial and book, the conformance corpus (its tag column becomes
  the field), the devtools events tab, and `UiEvent::message`, which
  becomes a plain decode of the payload. F148's tag-first rule and
  `is_core_event` go away.
- **A hard break, with no shim.** The old shapes (`changed`, `submit`,
  `cancel`, `access`, `{kind: 'drag', …, tag}` and the rest) go away in
  the release that lands this. No release sends both shapes, behind a
  flag or otherwise. kui is in alpha and nothing is on crates.io, and an
  app has to change its `update` either way. The CHANGELOG entry shows
  each binding's code before and after.
- **The ABI moves.** `KuiSpec` gains `on_submit`, `on_cancel`,
  `on_boundary`, `on_menu` and `on_selection_range`. `KUI_EDIT_VALUE` is
  a new constant, and the event struct C reads gains the envelope's
  `event`. That makes ABI 33. The frame protocol goes to v24: the editor
  op's value flag and the new rows. The Odin typed layer is regenerated from `kui.h`.
- **`value` is optional, and the imperative way stays.** An editor
  without `value` works as it does today: `initial` seeds it, and
  `setEditText` replaces its text. A `docs/howto.md` entry sets the two
  side by side. It says when to pick each, and that a normaliser belongs
  in `update`, not in a delayed effect, or its correction becomes an
  undo step of its own (decision 8).
- **Event and update in sync, pinned.** A test types into an editor
  whose app stores `change.text` and declares it as `value`. It checks
  that after every frame the editor's text equals the model, the caret
  has not moved, and undo still reaches back to the first keystroke. It
  runs in every adapter, as a corpus drive.
- **The cost is measured before the ADR is accepted as built.** Two
  rows join the bench guard: `edit_value_field` (a 40-byte field) and
  `edit_value_doc` (100 KB). Each declares a value equal to the editor's
  text, and a variant of each declares a one-character splice. Node's
  wire cost of carrying `value` at those sizes goes into
  `docs/performance.md`. No threshold blocks the build. The numbers
  decide what the howto tells a document editor. In Node,
  `value` crosses the wire on every frame, so a 100 KB document declared
  by `value` would cost 100 KB per frame, and the expectation is that a
  document keeps `initial` plus `onChange`.
- **ADR 0034 is amended.** The slider's event becomes
  `{…msg, change: {value, phase}}`. Its rule, proposed and never
  applied, is unchanged. Its slider-without-`onChange` nudge, kept from
  ADR 0007 decision 13, is removed (decision 5).
- **Build order:** (1) `EVENTS` columns, the envelope's `event`,
  `tagged` turned around, and Rust's `kui::Event` with `split`; (2) the
  editor's rows, `value` and the splice; (3) Node, Lua, C and Odin with
  their helpers; (4) the corpus, devtools and docs; (5) `labelOf`;
  (6) the derive's arriving twin. Each step is green before the
  next. Pinned in
  `crates/kui-core/tests/edit_value.rs` (splice and caret, undo joining,
  waiting for a composition, the two warnings, a list routed by message) and
  by a corpus drive every adapter replays: type into two fields and a
  list row, uppercase through `value`, then submit and clear. The
  corpus prints each event's envelope `event` and its field.

## What the building changed

- **The warning is `message-field-taken`,** not `tag-field-taken`
  (decision 4): "tag" is retired as a word (decision 15). The core marks
  an event whose data replaced a field the message had, and raises it on
  the way out, once per node.
- **Extension replies have a kind of their own,** `reply`: a reply is an
  extension's message to whoever placed its slot, which no handler row
  declares, so it is neither a node event nor a root one. Its payload is
  the message as sent.
- **A copy question arrives without a handler.** `selectionrange` is the
  answer to `requestCopy`, which the app asked for, so a scope that
  declares no `onSelectionRange` still hears it, as the field alone — as
  `requestFiles` answers without a row.
- **The stock widgets declare a `Null` message by default.** The stock
  `select` declares `onMenu(Null)` and the stock text input `onChange`,
  `onSubmit` and `onCancel` (`Null`), unless the binding hands them a
  message (`widgets::text_input_with`, `InputMessages`): a stock widget is
  never declared to be ignored, and its events arrive on its key as the
  field alone. The `select` element now reads the `onMenu` row, and
  `edit` takes `value` among its own props.
- **The Rust data types keep the names the views already had** — `Drag`,
  `Scroll`, `Hover`, `Layout`, `ButtonEvent` — where decision 10's table
  wrote `DragEvent`; the new ones are `Click`, `KeyEvent`, `Text`,
  `Preedit`, `Select`, `SelectionRange`, `ContextMenu`, `MenuChoice`,
  `ForceClick`, `Focus`, `FileDrop` (not `Drop`, which `use kui::*` would
  set over `std`'s trait), `Files`, `Resize`, `WindowEvent`,
  `SliderChange`, `EditChange`, `Boundary`, `Sound` and `Dismiss`. A drag's
  displacement is `drag.delta.x`, not `drag.dx`. `kui::Event` has
  `SliderChange` and `EditChange` apart, told by the data's shape, and
  `Unread(kind)` for a payload this core could not read.
- **The twin's attributes:** `#[message(on = drag)]` (any event name;
  `on = change` is an editor's change, `on = slider_change` a slider's,
  both under the field `change`), `incoming = "Name"` to rename the twin
  and `incoming_derive(…)` for its derives (a derive cannot read the
  original's). `submit`, `cancel`, `reply` and the root events are refused
  at compile time: they carry no data to put in the twin. One `on` per
  variant, so a sink that hears several of `onKey`'s events reads them
  with `ev.split::<Msg>()`. The `message-on-wrong-handler` warning is not
  built: a message declared on the wrong handler fails to decode as the
  twin, which is the signal, and the core has no record of which variant
  a value was built from.
- **A slider nudge checks the role.** Increment and decrement reach only a
  `slider` role; before, a reader's increment reached any enabled node.
- **An assistive `setValue` on an `<edit>` is a recorded edit**
  (`EditStore::replace_all`): the whole text replaced as select-all and
  typing would, so it undoes in one step, the history before it survives,
  and a filter's correction joins its step as after typing. It went
  through `setEditText` at first, which clears undo; changed the same day.
- **The corpus prints the envelope.** Each event row is `event message`:
  the envelope's kind, then the message by its `kind` (or the string it
  was kept under `tag` as), else the data's `phase` or `reason`, then each
  event's data extras as before; an editor's `change` prints its text.
  The `ime` and `paste` scenes' stock editor declares `onChange`.
- **Measured** (decision 8's rows, `cargo bench -p kui-core --bench frame
  -- edit_`, this Mac): a 40-byte field declared by `value` costs 1.25 µs a
  frame, the same as one seeded; a 100 KB document 47.2 µs against 42.2 µs
  seeded, so the compare is about 5 µs; a one-character splice into it
  147 µs, against about 195 µs for the keystroke itself.
- **`is_root` is the constructor's flag,** not the payload's shape. A
  data-less event whose message is spelled like it — `Msg::Submit` on
  `onSubmit` is `{kind: "submit"}` — looks exactly like a root event, so
  `UiEvent::root` marks what it builds and C carries the mark as
  `KuiEvent.root`. Node reads root-ness from the events table's `handler`
  column.
- **`HandlerMsg` distributes its `Omit` too.** `Omit<Extract<AppMsg, W>,
  …>` over a union of messages keeps only their shared keys, so
  `{kind: 'zoom'}` fit where `{kind: 'zoom'; id}` was registered; it is
  `Omit` per member. With no messages registered it falls back to `Msg`,
  or every handler prop of an unregistered app would be `never`.
- **An app's kind can still meet a root event's** in TypeScript, where
  `update` takes `AppMsg | RootMsg`: an app `{kind: 'menu'}` and the menu
  bar's `MenuBarMsg` narrow together. The examples renamed theirs
  (`openMenu`, `browse`); the envelope's `event` tells them apart at run
  time. Node's own clock ticks arrive with `event: 'tick'`.
- **Every node payload is a map.** `UiEvent::declared` makes one always:
  `{}` for a `Null` message with no data (the stock `<input>`'s Escape
  with no `onCancel` is `cancel` with `{}`), and `{tag: msg}` for a
  non-map message whether or not the event has data — so decision 3's
  wrapping holds for a plain click too, and a bare `on_click("save")`
  arrives as `{tag: "save"}`, which `ev.message::<String>()` reads back.
  At first a data-less event passed a non-map message through bare and a
  `Null` one as `null`, and Lua made its own tables of them; the rule
  moved into the core the same day, and Lua's special case went.
- **Binding spellings:** C `KUI_EDIT_VALUE = 1 << 5`,
  `kui_text_input_with`; Odin `Edit_Flag.Value`, `text_input_with`,
  `event_data`; Node bit 32 of the edit op's flags for `value` and bit 64
  for "both `initial` and `value` declared", the frame protocol at v24;
  `schema-dump` and Node's protocol spell the handler kind `"handler"`.
- **The corpus pins the rows.** The `edit-rows` scene declares `value`
  and the four editor rows on one field, plus `onMenu` raised through the
  stock editing menu (C's `kui_select` takes no spec, so the select's own
  `onMenu` is not spellable from C and Odin; a select there reports with
  its `Null` message). A new `enter` step sends Enter as an editing key,
  and a row now prints a `boundary`'s edge and key and a `menu`'s item.
  `onSelectionRange` stays uncovered: only `requestCopy` past a virtual
  list raises it, and no corpus step asks for a copy.
