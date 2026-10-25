---
status: accepted
date: 2026-10-25
---

# A `.lua` extension loads in every host

> **Accepted and built (2026-10-25); backlog C59.** Kit's track A of M1
> ([`addons/kit`](../../addons/kit), the M0 findings in
> [`M0.md`](../../addons/kit/M0.md)): Kit is one UI kit written once in
> Teal and loaded as a Lua extension, and only a Rust host could load
> one. What the building changed is at the end, under [*What the
> building changed*](#what-the-building-changed).

Every door that loads an extension — Node's `addExtension` and a window's
`extensions`, C's `kui_ctx_add_extension`, Odin's `ctx_add_extension`, a
script's `env.add_extension` — now loads a `.lua` file, or a folder
holding `init.lua`, as a `LuaExtension`, and anything else as a C
library, as before. One extension written in Lua then reaches every
binding.

## Context

- **Only a Rust host could load Lua.** Node's `addExtension` said so on
  purpose — "a Lua extension is loaded by a Rust host or not at all" —
  and [ADR 0014](0014-slots-an-extension-fills-in-place.md) listed "a C
  host can load a Lua or C plugin" as not decided. A script's
  `env.add_extension` loaded C libraries only.
- **Node is one line away.** kui-node depends on kui-core, kui-native and
  kui-ffi; kui-lua on kui-core and kui-ffi. kui-node → kui-lua is no
  cycle, both load sites push one `Box<dyn Extension>`, and `Extensions`
  already holds boxes that are not `Send`, so mlua's `!Send` costs
  nothing.
- **C and Odin have one cycle in the way.** kui-lua depended on kui-ffi
  for a single call, `CExtension::open` in `env.add_extension`, so
  kui-ffi could not depend on kui-lua.
- **A Lua script packaged as a C plugin is not a way round it.** kui-lua
  lowers through `kui_core::Ui`, so a plugin carrying its own kui-core
  would dereference the host's `Core` from a second compiled copy.
- **A script could not find its own folder** (M0). `from_file` named the
  chunk by the file's name alone and a script has no `debug` library, so
  a script made of modules — Kit's entry `require`s eight — could not
  `require` the files beside it; Kit's host prepended a `package.path`
  line to the source by hand.

## Decision

1. **One door, and the path says what it is.** A path ending in `.lua`,
   or a directory holding `init.lua`, loads as a `LuaExtension`; anything
   else as a C library, exactly as before. `kui_lua::is_lua_path` is the
   test, and `kui_ffi::open_extension(path)` the one loader every host
   calls: C's `kui_ctx_add_extension` (and so Odin's), Node's
   `addExtension` and a window's `extensions`. Namespacing, slots, tokens
   by origin, replies and replay are unchanged: a Lua extension is an
   `Extension` like any other.
2. **The cycle is broken by injection.** kui-lua no longer depends on
   kui-ffi. A script's `env.add_extension` loads a `.lua` path itself and
   hands any other path to a *native loader*, a plain
   `fn(&Path) -> Result<Box<dyn Extension>, String>` that whichever crate
   links both registers with `kui_lua::set_native_loader`; kui-ffi does on
   its first `open_extension`, and exposes `register_native_loader` for a
   Rust host that builds `LuaExtension`s itself. With none registered, a C
   path is answered with a message naming it, the way a missing file
   always was.
3. **A loaded file knows its folder.** `LuaExtension::from_file` names
   the chunk `@<path>`, so errors and tracebacks name the file, and puts
   the file's directory first on `package.path` (`dir/?.lua` and
   `dir/?/init.lua`), so `require "skin"` finds `skin.lua` beside it.
   `LuaExtension::from_path` takes a file or a directory with `init.lua`.
   Each loaded extension keeps its own Lua state, as before.
4. **Lua ships wherever kui-ffi does.** The Lua VM and kui-lua link into
   `libkui_ffi` and the Node addon unconditionally; the size it adds is
   measured below.

## Considered options

- **A separate `addLuaExtension` door.** Doubles every binding's surface
  for a fact the path already carries, and a host that takes a plugin
  path from its user would have to sniff the extension itself anyway.
- **Keep Lua Rust-only.** Blocks a single Kit implementation: the
  alternative is the same components written five times.
- **A native add-on per platform.** Needs an exact ABI match (the ABI
  moves with almost every change), a second `kui_ffi.dll` on Windows,
  four prebuilt targets and a plugin-side Rust crate that does not exist.
- **A feature flag on kui-lua's kui-ffi dependency.** Cargo refuses the
  cycle whatever the features, and a flag would leave the dependency
  graph depending on who turned what on.

## Consequences

- No C ABI bump and no Node protocol bump: the entry points are the same,
  and they accept one more kind of path.
- `libkui_ffi` and the Node addon grow by the Lua VM and kui-lua.
- A Rust host that built a `LuaExtension` itself and whose script loads a
  **C** plugin with `env.add_extension` now registers the native loader
  first (`kui_ffi::register_native_loader()`); one that loads through
  `kui_ffi::open_extension` gets it registered. That is the one break.
- Kit's host loads `addons/kit/lua/kit.lua` with `from_file` and drops
  its `package.path` line.

## What the building changed

- The ADR's draft put Lua behind a default-on `lua` feature. kui-ffi is a
  `default-features = false` workspace dependency, so a default feature
  would have been off in exactly the crates that need it (kui-node); Lua
  is unconditional instead, and the size is the price, measured here.
- The size, release `libkui_ffi.dylib` on aarch64-apple-darwin
  (M3 Pro, `cargo build -p kui-ffi --release`): 16.88 MB → 18.45 MB with
  the runner, 7.72 MB → 9.26 MB without (`--no-default-features`) —
  about 1.55 MB for the Lua VM and kui-lua either way.
- `Box<E: Extension>` is an `Extension` (kui-core, forwarding), so the
  boxed answer `open_extension` gives goes to `Launcher::extension_as` as
  it is; Node's window `extensions` option needed exactly that.
