---
status: accepted
date: 2026-10-25
---

# An add-on describes its components once, and every binding gets typed wrappers

> **Accepted (2026-10-25); backlog C67.** Kit's M3: ADR 0060 made a
> component placeable from every binding by name and a bag of props; this
> makes it pleasant — typed props, typed events, autocomplete — without
> writing the wrappers five times.

An add-on that provides components ships one **schema**, a Lua file
returning a table that names each component, its props with their types
and docs, its events with their fields, and its child groups. A generic
Rust crate, `kui-addon-schema`, reads it and writes typed wrappers for
every binding, committed beside the add-on:

| binding | what it gets |
|---|---|
| Lua | a host module (`require "kit.ui"`) with LuaLS annotations, and its Teal declarations |
| Node | ESM functions that are also JSX function components, and their `.d.ts` |
| Rust | a crate of props structs with builders and `place` functions |
| C | a header of props structs and `static inline` place functions |
| Odin | a package of props structs and place procedures |

## Context

- ADR 0060's door is untyped everywhere: `component("kit/button", props)`
  with whatever map the host builds. A typo in a prop name draws a default
  and says nothing; an event's fields are found by reading Lua.
- Kit is written once, in Teal, and must reach hosts in five languages
  (the plan's decision: one add-on language, typed wrappers generated).
- kui already generates bindings from tables (the Odin package from
  `kui.h`, `index.d.ts` from the schema); an add-on is the same problem one
  level up.

## Decision

1. **The schema is Lua data, in the add-on.** `schema.lua` returns
   `{ addon, doc, enums, records, components }`; every list is an array, so
   order is the file's. A component is `{ name, doc, props, events,
   children }`; a prop is `{ name, type, doc }`; an event is `{ name, doc,
   fields }`; children are `{ group, doc }` (`""` the unnamed group).
2. **A small type language.** `string`, `number`, `integer`, `boolean`,
   `color` (a colour or a `$token`), `length` (px or a `$token`), `channel`
   (ADR 0065), `any`, the schema's own enums and records, `{T}` for a
   list, and a trailing `?` for optional. Each binding maps them to its
   own types; a type the schema does not define is an error naming it.
3. **One wire spelling.** Props cross as snake_case — the add-on is Lua —
   and Node's wrappers take camelCase and translate; event props are
   `on_<event>` (either spelling reaches the extension, ADR 0060).
4. **Wrappers place components, nothing else.** Each wrapper builds the
   props value and calls the binding's door (`component`,
   `component_with`, `kui_component`, `kui.component`, `el('component')`);
   the extension is unchanged, and an add-on that ships no schema still
   works untyped.
5. **Generated files are committed and checked.** `kui-addon-schema gen
   <schema.lua> <out>` writes them; `--check` fails on a stale one, and the
   add-on's test loads the extension and checks its `components` against
   the schema's names. `nu scripts/gen.nu` runs it for every add-on under
   `addons/` that has a `schema.lua`, then the add-on's `build.lua`.
6. **Events are typed where the binding reads them.** Rust gets a struct
   per event with `from_payload`; Node a `<Component><Event>Msg<T>` type;
   Odin a struct per event and the `With_` a message embeds (`using _:
   kit.With_Segmented_Change`), as `events.odin` spells the core's own; Lua
   and Teal document the fields; C reads the payload as it reads any.
7. **Names a binding cannot take are the schema's error or the
   generator's escape.** A type named for one the wrappers or a language
   already has in scope (`Option`, `String`, `Value`, `Node`, `Channel`…)
   is refused with the reason; a field or a component that is a keyword in
   one language is escaped there (`switch_` in Odin, `r#type` in Rust).
   Records get a constructor of their required fields and setters for the
   rest, in the bindings that build them (`kit::Column::new("ms",
   "Time").align(kit::Align::End)`).

## Considered options

- **JSON or TOML for the schema.** The add-on is Lua, and a Lua schema can
  share its enums with the code; the user's rule for configuration is
  "why a format when we have a language".
- **Generate from Teal types.** Teal's records carry types but not docs,
  events or child groups, and parsing Teal from Rust is a project of its
  own.
- **Hand-written wrappers.** Five copies drift, which is the problem Kit
  exists to remove.

## Consequences

- A new workspace crate, `kui-addon-schema` (library and binary), with
  tests over a small schema: what it reads, what it refuses and why, the
  files it writes, keyword escapes.
- Kit gains `schema.lua` and `bindings/` (Lua, Teal, Node, Rust crate
  `kui-kit` as a workspace member, C header, Odin package), each run
  against Kit at runtime: `crates/kui-lua/tests/kit.rs`, a Node test,
  `examples/c/features/kit/kit.c` and `examples/odin/features/kit/kit.odin`.
- Building Kit on the wrappers found four faults in the instance path,
  fixed beside this (backlog C68–C71): an instance that heard its own
  event is asked again, token lookups are a replay read, a host's children
  inside a component can place that extension's components
  (`Extension::reentry`), and a label's key no longer collides with an
  index's.
- Untyped placement stays: a host that builds the props map itself, or an
  add-on with no schema, works as ADR 0060 has it.
