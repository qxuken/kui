# Extensions: components and slots

At the end of this chapter, two Lua scripts draw inside a Rust window:
one a list the host placed, both a button in a toolbar the host
declared without knowing who would fill it.

## A guest in your frame

An **extension** is code a host loads to draw in its frame: a Lua
script, a C library, another Rust type. It does not get a window. It
gets a place in yours, and it draws there with the same nodes, on the
same frame, while you build it. What it draws is tagged with its
origin, keyed under its own name, and its events go to it first.

You load it under a **namespace** you choose, the way an import picks
an alias. The notes plugin below is `notes`, so its list is
`notes/list`:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/11_extensions.rs:main}}
```

`kui-lua` is the crate that runs a Lua script as an extension. Add it
beside `kui-native`:

```bash
cargo add kui-lua
```

## A component: placed by name

The script says what it provides in a `components` table. Each entry
is a function from the environment, the host's props and the instance
to a tree — the same builders a Lua view uses:

```lua
{{#include ../../../examples/rust/tutorial/11_extensions/notes.lua:provides}}
```

The host places it where it wants it, as often as it likes, with props
in:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/11_extensions.rs:component}}
```

Each placement is an **instance**, keyed by where it is (or by a `key`
prop). Its nodes are keyed under the instance, so two lists never share
a key.

## Its events are your messages

The script hears its own nodes' events in `on_event`. It answers the
host by raising one of its component's events with `emit`:

```lua
{{#include ../../../examples/rust/tutorial/11_extensions/notes.lua:events}}
```

`emit("on_pick", …)` arrives as whatever the host passed for `onPick` —
here `Msg::Picked` — with the plugin's fields in the event's field,
named for the prop without its `on`: `pick`. The host never learns the
plugin's own messages, and the plugin never learns the host's.

## A slot: placed by everyone

A component is placed by name, so the host has to know it. A toolbar
that plugins add buttons to is the other way round: the host knows the
place, not who will fill it. That is a **slot**:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/11_extensions.rs:slot}}
```

A plugin contributes by naming the slot and one of its own components:

```lua
{{#include ../../../examples/rust/tutorial/11_extensions/notes.lua:contributes}}
```

The stamp plugin contributes too, with `order = -1` to go first:

```lua
{{#include ../../../examples/rust/tutorial/11_extensions/stamp.lua}}
```

Every contribution is placed in the slot as a sibling — load order,
then `order` — and each gets the slot's props. They all raise the one
message, `Msg::Ran`, and `from` says who raised it:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/11_extensions.rs:on_event}}
```

A slot's name is any string. The convention is `owner.place` — your
app's name, then the place — so two apps' plugins never meet by
accident. A slot nobody contributes to draws nothing and says nothing:
whether it is drawn at all is your view's call.

## Replayed unless something changed

An instance whose props are what they were, and that read nothing that
changed, is not asked again: kui pushes last frame's nodes back. That
is what makes a plugin cheap — and it is why the host passes a
`revision` to the list. "New note" changes the list's state from a
*different* instance, the toolbar button, and the list's own props are
the only thing the host can see change. Pass what moves as a prop, or
`fresh: true` to be asked every frame.

## The test

Chapter 11's `Drive` loads extensions too:

```rust,noplayground
{{#rustdoc_include ../../../examples/rust/tutorial/11_extensions.rs:tests}}
```

```bash
cargo test -p kui-lua --example tutorial_11_extensions
```

## The other bindings

The verbs are the same everywhere: Node's `<component name props/>` and
`<slot name props/>`, Lua's `component(name, props)` and `slot(name,
props)`, C's `kui_component` and `kui_slot`. A C plugin lists
`kui_ext_components` and `kui_ext_contributions`. The next chapter
turns it around: the whole app is the Lua script, and the host is a
binary you do not write.
