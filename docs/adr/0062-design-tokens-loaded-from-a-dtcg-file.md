---
status: accepted
date: 2026-10-25
---

# Design tokens loaded from a DTCG file and its resolver

> **Accepted and built (2026-10-25); backlog T8.** Kit's M1 track B,
> second half: a company's design team exports its tokens as DTCG, and a
> Kit brand is that export mapped onto roles. What the building changed is
> at the end.

The core reads a **DTCG 2025.10** token file, or a **Resolver Module**
document that composes several, into kui's own token shape — colours with
a light and a dark half, lengths in logical px, family fallback lists — in
one loader every binding reaches. A brand's export drives a kui app with no
hand-copying and no build tool in between.

## Context

- kui's tokens are colours (`{light, dark}` or derived), lengths (a number)
  and families (a fallback list) (`tokens.rs`). Nothing loaded them from a
  file; Node's `setTokens` took a JSON-shaped object nothing produced from
  a standard file.
- DTCG 2025.10 is the first stable version (a Final Community Group Report,
  2025-10-28): Format, Color and Resolver modules. Colours are
  `{colorSpace, components, alpha?, hex?}` in fourteen spaces; dimensions
  `{value, unit: "px" | "rem"}`. Light and dark are not in Format: they are
  a Resolver modifier's contexts — `sets` with ordered `sources`,
  `modifiers` with `contexts` and a `default`, a `resolutionOrder` of
  `$ref`s or inline entries, the last declaration of a token winning.
- Real exports still carry the legacy spellings: a colour as `"#rrggbb"`,
  a dimension as `"12px"`. Figma, Tokens Studio and Style Dictionary write
  DTCG; Style Dictionary does not read resolver files.
- kui-core had no JSON reader, and depends on nothing that has one.

## Decision

1. **One loader, in the core, that returns data.**
   `kui_core::dtcg::from_path(path, opts)` (and `from_str(text, base,
   opts)`) answers a [`Dtcg`]: the [`Tokens`] to declare, every name with
   its DTCG path and value, and the warnings. A document with a
   `resolutionOrder` is a resolver, anything else a token file, so one call
   takes either. A resolver's relative `$ref`s (a file, or
   `file#/pointer`) resolve against its folder; a remote one is refused
   with a sentence saying to save the file beside it.
2. **The halves come from a modifier, found by its contexts.** The
   options name it (`theme`) and its two contexts (`light`, `dark`);
   when no modifier has that name, the one whose contexts are called
   `light` and `dark` is used, so an export calling it `mode` or
   `appearance` loads with no options. Each other modifier takes the
   context the options' `inputs` give it, else its `default`; one with
   neither is an error naming its contexts.
3. **Types map where kui has a kind.** `color` → a colour token, from all
   fourteen 2025.10 spaces through the CSS Color 4 matrices (a space it
   could not read falls back to `hex`), clamped into sRGB with a warning;
   `dimension` → a length (`rem` × `opts.rem`, 16); `number` → a length;
   `fontFamily` → a family. Aliases (`{group.token}`) follow, a cycle is a
   warning. `$type` inherits down groups. Composite types (`shadow`,
   `border`, `typography`, `gradient`, `transition`, `strokeStyle`) and
   `duration`, `cubicBezier`, `fontWeight` are skipped with a warning naming
   the token.
4. **Names are the path in camelCase, behind a prefix.**
   `background.elevation-1` → `backgroundElevation1`; `prefix: "brand"` →
   `brandBackgroundElevation1`, so a brand cannot collide with an app's own
   names. Two paths that make one name: the second is skipped, warned.
5. **A door per binding, in the shape each already speaks.**
   - Rust: the module above; `Core::add_dtcg(&loaded)` adds a load to the
     running origin's own table and raises `dtcg-skipped` per warning.
   - Node: `loadTokens(pathOrDocument, options)` → `{ tokens, names,
     warnings, resolver }`, `tokens` the object `setTokens` takes;
     `DtcgOptions` and `LoadedTokens` typed; an unknown option is refused
     by name.
   - Lua: `env.load_tokens(path, opts)`, the same table; a relative path is
     the script's folder's.
   - C: `kui_tokens_load_dtcg(ctx, path, prefix, inputs)` adds to the
     calling origin's table, as `kui_tokens_derive` adds, and
     `kui_tokens_dtcg_error` says why a load said false. `inputs` is
     `name=context` pairs (`;` or `,`), with `@modifier`, `@light`, `@dark`
     and `@rem` setting the loader — plain strings, so no new struct.
   - Odin: `tokens_load_dtcg`, `tokens_dtcg_error`, generated.
6. **The brand's role mapping is not the core's.** Kit maps a loaded
   brand onto its skin's roles in Lua; any app can declare the table as it
   comes.

## Considered options

- **A Style Dictionary step outside kui.** Needs Node, reads no resolver,
  and puts a build between a designer's export and the app.
- **Load in Kit only.** Any app with a brand wants it; Kit is one of them.
- **`serde_json` in kui-core.** A dependency for one reader the core needs
  in two places; `kui_core::json` is a strict RFC 8259 reader into
  `Value`, keys in file order, a repeated key's last value kept.
- **A `KuiDtcgOptions` struct for C.** A new `[in]` struct for four
  rarely-set options; the `@` keys in `inputs` say the same with no layout
  to keep.

## Consequences

- A new parsing surface, fuzzed: target `dtcg` (JSON, then a token file or
  an inline-only resolver, added to a core under both appearances), seeded
  from the fixture.
- A shared fixture, `crates/kui-core/tests/fixtures/dtcg` (one brand, two
  appearances, one skipped shadow), loaded by the Rust, C, Node and Lua
  tests to the one table its README lists.
- A `dtcg-skipped` warning code.

## What the building changed

- The draft's `Tokens::from_dtcg` became the `dtcg` module (`from_path`,
  `from_str`) plus `Core::add_dtcg`: a load answers more than a table
  (names, paths, warnings), and adding on top of an origin's own tokens is
  what C needs and Rust may want.
- `$extensions` modes (Figma's) are not read yet; a resolver is.
