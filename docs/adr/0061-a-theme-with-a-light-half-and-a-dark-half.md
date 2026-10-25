---
status: accepted
date: 2026-10-25
---

# A theme with a light half and a dark half

> **Accepted and built (2026-10-25); backlog T7.** Kit's M1 track B
> ([`addons/kit`](../../addons/kit)): a Kit skin has a light and a dark
> palette, and the core's own chrome — focus ring, scrollbars, selection,
> menus, the stock widgets — paints from the theme. What the building
> changed is at the end.

A host can hand the core **one theme with two halves**, and the core
picks the half the OS's appearance asks for at the start of every frame,
the way `Derived` follows it already. A skinned app then follows light
and dark with no listener and no second call.

## Context

- `ThemeSource` was `Derived`, `DerivedWithAccent(Color)` or
  `Pinned(Theme)`, and `Pinned` is one palette (`theme.rs`).
- Every binding's override was one palette: Node's `setTheme` resolved
  once and then "follows nothing afterwards", C's `kui_theme_set` took one
  whole `KuiTheme`, Lua reads `env.theme` only.
- So an app with its own palette for both appearances had to hear
  `onSystemSettingsChange` and set its theme again, in every binding —
  exactly the per-binding decision the schema's rule ("per-binding
  extraction, never per-binding decisions") keeps out of the bindings.
- Colour tokens carry light and dark halves already (ADR 0027), but the
  stock widgets and the core's chrome paint from `Theme`, not tokens; a
  token pair cannot reach a focus ring.

## Decision

1. **`ThemeSource::Pair { light, dark }`.** Resolved at the start of every
   frame from `env.system.appearance`: `Light` takes `light`, `Dark` takes
   `dark`, and `Unknown` takes `dark`, as the dark base stands for an
   appearance nobody reported (ADR 0019). `Core::set_theme_pair(light,
   dark)` is the shorthand; `ThemeSource::stock_pair(accent)` the stock
   bases to start from.
2. **Node takes a pair of overrides.** `setTheme({ light: {…}, dark: {…}
   })`: each half is the single form's overrides on that half's base, an
   `accent` in a half recomputing that half's hover, pressed, label, soft,
   selection and ring; a half left out is the stock base. Mixing the forms
   — `light` beside a role — or naming `appearance` inside a half is
   refused by name, since one of them would be ignored. The single form is
   unchanged. Typed as `ThemePairOverrides`.
3. **C gets the pair and the bases.** `kui_theme_set_pair(ctx, light,
   dark)` (either NULL derives again, as `kui_theme_set(NULL)` does), and
   `kui_theme_derive(appearance, accent, out)`, the stock palette with no
   context — a C host had no way to read the base for the appearance it is
   *not* in, so it could not start the other half. New functions, no
   struct change: no ABI bump. Odin's `theme_set_pair` and `theme_derive`
   are generated from them.
4. **Lua stays read-only for now.** Writing the theme is the host's (ADR
   0019); a script is a guest. The `kui` runner's app script is a host,
   and gets its door with the runner.
5. **The devtools keep the skin.** The panel's base override on a pair
   pins the app's *own* half for that base — a skin's light half, not
   kui's stock light — and its accent override recolours both halves and
   keeps following the OS. The facts tab says `pair`.

## Considered options

- **A theme-switch hook in each binding's runtime.** The per-binding
  decision this removes, four times over.
- **Paint the chrome from tokens instead of `Theme`.** Every stock widget
  and ADR 0019's model changed for one feature, and the roles would still
  need a name per appearance — which is what a pair is.
- **A corpus scene for the pair.** The pair is resolved in one core
  function every binding calls through; the bindings' own tests (Rust, C,
  Node) flip the appearance under it. A scene would need a theme op in
  four adapters to say the same.

## Consequences

- `docs/props.md`'s doors table and Odin's generated layer gain the two
  doors; `index.d.ts` gains `ThemePairOverrides`.
- A skin with two palettes is one call at start-up in every host, and Kit
  hands its skin's pair to the host that way.

## What the building changed

- The ADR's draft promised a corpus scene; decision "Considered options"
  says why the binding tests stand in for it.
- `kui_theme_derive` was not in the draft; building the C test showed a C
  host could not start the half it was not in.
