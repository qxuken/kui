# Lua types

The view DSL a [kui-lua](../../crates/kui-lua) script writes in, described
for the tools that check one. Both files are generated from kui-core's
schema and the prelude, never edited: `nu scripts/gen.nu` writes them, and
kui-lua's tests fail while they are stale.

| File | For | Written by |
|---|---|---|
| [`kui.lua`](kui.lua) | [lua-language-server](https://luals.github.io): completion, hover docs, and a warning on a wrong-typed prop | `kui_lua::luals_meta()` |
| [`kui.d.tl`](kui.d.tl) | [Teal](https://teal-language.org), typed Lua: `tl check` refuses a misspelt prop, a wrong-typed value, a word no enum has, an `env` read that is not there; `tl gen` writes the plain Lua the host loads | `kui_lua::teal_decls()` |

Copy the file for the kui-lua version the host runs.

**lua-language-server.** Put the directory holding `kui.lua` on
`workspace.library` (`.luarc.json`: `{ "workspace.library": ["types"] }`).

**Teal.** Put `kui.d.tl` where `tl` finds modules and name it the global
environment in `tlconfig.lua`:

```lua
return { include_dir = { "types" }, global_env_def = "kui" }
```

`view(env: kui.Env, slot: kui.Slot): kui.Node` and `on_event(ev: kui.Event,
at: kui.Envelope)` are the script's own globals; a message is a record that
extends `kui.Event`, and the `kui.With…` field its event fills, cast from
`ev`. The file is checked against Teal 0.24.8. The
[howto](../../docs/howto.md#how-do-i-type-check-a-lua-view-or-write-it-in-teal)
has the longer answer.
