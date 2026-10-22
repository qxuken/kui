---
status: accepted
date: 2026-10-22
---

# The disabled opacity and font families by token

> **Accepted and built (2026-10-22); backlog T6.** What the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed). This extends [ADR
> 0027](0027-tokens-beside-the-theme.md). That ADR let an app name a
> colour or a length once and reference it as `'$name'` in any prop, so a
> view reaches the theme without a `ctx`/`env` handle threaded through
> it. Two things a view still needs the handle for are named here. The
> theme's **`disabledOpacity`**, the one theme value that is not a colour
> and so has no `$` spelling. And **font families**, which an app spells
> as a string per text node with no shared name for "the code face" or
> "the display face". The proposal: `$disabledOpacity` becomes a role in
> the space the length tokens already occupy, which is in fact every
> `f32` slot, `opacity` included. Family tokens become a third kind
> beside colours and lengths, declared as a name or a fallback list and
> referenced as `family="$code"`. Neither changes Node's wire.

## Context

### Why the question came up

An app that wants its own disabled look (a dimmed row, a greyed card)
reads `ctx.theme().disabledOpacity` in TSX or `env.theme.disabled_opacity`
in Lua, and so passes `ctx`/`env` down to every component that dims.
Every other theme value is reachable as `roles.surface` / `"$surface"`
from module scope (ADR 0027, decision 6). `disabledOpacity` is left out
on purpose: `roles` excludes it with `appearance`
(`packages/kui/index.d.ts`, the `roles` type), because ADR 0019 carries
it "beside the roles rather than among them", since it is not a colour.
It cannot be hardcoded either. It is 0.5 in both bases (`theme.rs`), but
a host's theme override can set it (`disabledOpacity?` in the override
type), and the stock widgets read the live value (`widgets.rs`, the
`opacity * disabled_opacity` sites).

Families have the same shape one axis over. A text's `family` is a
string (`'sans' | 'serif' | 'mono'` or an installed or loaded family's
name, registered as it is read; ADR 0037). An app with a code face writes
`family="JetBrains Mono"` on every code text, or holds the name in a
constant, and has nowhere to say "JetBrains Mono if installed, else
Cascadia Code, else mono". A name nothing matches today is sans plus an
`unknown-family` warning.

### What ADR 0027 built that this has to fit

- **The length kind is already a number kind.** The wire tag
  (`id | 0x8000`, the token index in the value slot) applies to every
  `Kind::F32` row. That is 31 rows, and lengths are only some of them:
  `opacity`, `rotate`, `scale`, `pivotX`, `transition`, `lineHeight` and
  `valueNow` all resolve a `$length` today. The generated JSX types agree:
  `opacity?: LengthProp`. So `lengths: { fade: 0.4 }` with
  `opacity={T.fade}` already works and type-checks. What is missing is
  only a **role** for the theme's value.
- **Role indices come first.** `$surface` is colour index 1 whatever the
  app declared. The encoder reads the role names from `protocol()
  .tokenRoles` and does not hardcode them, so a role added to a space
  moves the app's indices for a matched addon and JS together.
- **Families already cross by name.** Node carries `family` as a strref
  (v18), Lua as a string, and both lower it through
  `NameRefs::family`. A `$name` in that string can be resolved where the
  name is resolved now, with no tag and no wire change. C passes a
  `KUI_FONT_*` and a font handle, which have no room for a name. ADR 0027
  decision 4 already says a C prop carries no reference.

## Decision

1. **`$disabledOpacity` is a role in the length space.** It is appended
   after the sixteen `METRIC_ROLES` as one more role in the length range.
   It reads `Theme::disabled_opacity`, so a host's override is what it
   resolves to. Both spellings resolve, as every role does:
   `$disabledOpacity` and `$disabled_opacity`. Wherever a `$length`
   resolves, so does this role. That covers every `f32` row, the
   `enter: { opacity }` and keyframe stops (through `NameRefs::length`),
   and Lua's spec reader. `roles.disabledOpacity` is typed `LengthToken`.
   The role is not a metric: `ctx.metrics()` and `env.metrics` do not
   list it, and the theme's reading of it stays where it is. The schema
   gets a `NUMBER_ROLES` list for roles that read the theme rather than
   `Metrics`. `LENGTH_ROLES` counts both lists, so the wire's reserved
   range grows by one.

   The docs say what the code already does: a length token is a number.
   It is logical px where the slot is a length, and the number itself
   where it is not (`opacity`, `scale`, `transition`). The type names
   `LengthToken` and `LengthProp` stay.

2. **Family tokens are a third kind.** They are declared beside colours
   and lengths:

   ```ts
   const T = defineTokens({
     families: { code: ['JetBrains Mono', 'Cascadia Code', 'mono'], display: 'Fraunces' },
   });
   <text family={T.code}>…</text>
   ```

   ```lua
   tokens = { families = { code = { "JetBrains Mono", "Cascadia Code", "mono" } } }
   text("…", { family = "$code" })
   ```

   A token's value is one name or a list. **The first entry that
   resolves wins**: a stock name, or a family installed or loaded. That
   pick is the token's family for every text that names it. It is the
   face the text asks for, and cosmic-text's per-glyph fallback still
   runs under it, unchanged. A list where nothing matches resolves to sans
   and raises `unknown-family` once, naming the token and its list. That
   is the warning a bare name raises today.

   - **Picked at declaration and again when the fonts change, not per
     node.** `register_family` is a fontdb query, and a list that misses
     its first two entries would pay three queries on every node. The
     table holds the picked `FontFamily` per token. The core re-picks
     when the session's fonts revision moves (`loadFontsDir`, a font file
     loaded, the system's fonts changing). A lowering then reads a slot.
   - **Referenced in every family slot**: a text's and an editor's
     `family` and a span's `family`. `NameRefs::family` checks for the
     `$` before the stock names. A `$name` of another kind (`family=
     "$peach"`) raises the kind error `unknown-token` already gives.
   - **The stock names are not roles.** `sans`, `serif` and `mono` are
     already spelled bare, and adding `$mono` would give the same face
     two spellings. A family token named `sans`, `serif` or `mono` is
     refused with `reserved-token`, as a role name is.
   - **One table per origin, own over host**, as ADR 0027 decision 1
     made it. An extension can write `family="$code"` and get the host's
     face.
   - **Declared and read in every binding.**
     - Rust: `Tokens::family("code", ["JetBrains Mono", "mono"])` and
       `ui.token_family("code") -> Option<FontFamily>`.
     - Node: the `families` key of `setTokens` / `defineTokens`, and
       `ctx.tokens().families` as `{ code: 'JetBrains Mono' }` (the
       picked name).
     - Lua: `tokens.families`, `env.set_tokens`, and
       `env.tokens.families.code`.
     - C: a `KuiFamilyToken { KuiStr name; const KuiStr *names; size_t
       count; }`, declared through `kui_tokens_set_with(ctx, const
       KuiTokens *)`, a struct with all three arrays. `kui_tokens_set`
       stays as the colours-and-lengths shorthand, and both replace the
       caller's table whole. Read back with `kui_token_family(ctx,
       KuiStr, uint32_t *family, uint64_t *font)`, which fills the two
       fields a `KuiTextStyle` takes. These are new functions and new
       `[in]` structs, so no ABI bump under ADR 0006. Odin's layer is
       generated from the header.
   - **TS.** A `FamilyToken` brand (`'family'`). `bg={T.code}` and
     `width={T.code}` are type errors, and `family={T.peach}` is not
     (`family` takes any string). That is the same limit ADR 0027 hit
     with a length in a colour slot.

3. **The devtools list and name them.** The facts tab's token rows gain
   the families, each with its list and its pick ("code → JetBrains
   Mono"), and the role row for `disabledOpacity`. The node inspector
   prints a text's family with the token names that pick it, as it prints
   colours and lengths.

4. **No wire or protocol change for Node beyond the role list.** The
   frame version stays at v24. The protocol's `tokenRoles.lengths` gains
   `disabledOpacity`, which moves the app's length indices by one for an
   addon and a JS file of the same build. That is what the protocol
   version exists to pin, so it moves.

## Considered options

- **A `dimmed` flag on any node** that multiplies its opacity by the
  theme's value. This was the narrower first answer. It is declined
  because `opacity={roles.disabledOpacity}` is the same one line with
  nothing new to learn, and it composes: `enter: { opacity:
  '$disabledOpacity' }` fades something in to the dimmed look, which a
  flag cannot do.
- **A third numeric kind (`numbers`)** apart from lengths, with the 31
  `f32` rows split into lengths and scalars. Declined. It needs a unit
  column in the schema and a second branded prop type, and it turns
  today's working `opacity={T.fade}` into an error. Mixing a unitless
  number and a length costs nothing at runtime, so the split would buy
  nothing.
- **`$sans` / `$serif` / `$mono` as family roles.** Declined: the bare
  names already exist, and two spellings for one face is what decision 6
  of ADR 0027 avoided for colours.
- **Pick the list per node**, CSS-style, at every lowering. Declined:
  three fontdb queries a node for a list that misses twice, and the pick
  cannot differ between two nodes of one frame anyway.
- **Theme-level family roles that the stock widgets read** (a `$body`
  face for every button and label). That is a bigger question: the stock
  widgets read `sans` today, and changing it moves every stock control
  for every app. It is left out, and a family token is the piece it would
  be built from.

## Consequences

- **The build, by crate.**
  - `kui-core`: `NUMBER_ROLES` and the grown length range, family tokens
    in `Tokens` (picked set, re-pick on the fonts revision),
    `TokenKind::Family`, `$` in `NameRefs::family`, `reserved-token` for
    the stock names, `ui.token_family`, and the devtools rows.
  - `kui-node`: `setTokensRaw` and `tokens()` with `families`,
    `defineTokens` and the `FamilyToken` brand, `roles.disabledOpacity`,
    and the protocol version.
  - `kui-lua`: `families` in the declaration and in `env.tokens`.
  - `kui-ffi`: `KuiFamilyToken`, `KuiTokens`, `kui_tokens_set_with`,
    `kui_token_family`, and the header. Odin regenerated, with its
    hand-written token wrapper.
  - The corpus `tokens` scene gains a family token whose first entry is
    not installed, and a box at `opacity: $disabledOpacity` under a
    host override.
- `docs/props.md`: one sentence on the `f32` rows ("a `$length` is a
  number") and the `family` row's `$name`. `docs/howto.md`: the tokens
  answer gains families and the dimmed look.
- An app that wants a dimmed component no longer needs the handle for
  it. An app that wants to *compute* with the value (`opacity * 0.8`)
  still reads `ctx.theme()`, as ADR 0027 left every computation.

### Not done here

- Theme-level family roles for the stock widgets, as above.
- A family token per weight or style. The text's `bold` / `italic`
  props already pick within the family.

## What the building changed

- **C declares families the way it declares derived tokens.** The
  proposal's `KuiTokens` struct and `kui_tokens_set_with` would have been
  a second way to do what `kui_tokens_set` does. ADR 0028 had already
  settled how C adds a kind: `kui_tokens_derive` appends to the drawing
  origin's table after `kui_tokens_set`. So families arrive the same way,
  through `kui_tokens_families(ctx, const KuiFamilyToken *, size_t)`,
  where `KuiFamilyToken` is `{ name, names, name_count }`. It is still a
  new `[in]` array beside ABI 34, with no bump.
- **Odin's `token_family` is hand-written.** It returns `Font_Family` and
  `Font`, the types `Text_Style` takes, where the generated door would
  have returned raw integers. `tokens_families` is generated.
- **No version moved.** The encoder reads the role names from
  `protocol().tokenRoles` at runtime, so an addon and its JS agree on
  the grown length range whatever their versions. Decision 4's protocol
  bump would have refused nothing that could go wrong. The frame stays
  at v24.
- **The devtools list the families, not the role.** The facts tab lists
  no roles at all; they live in the theme's own rows. The node inspector
  prints no family for a text, so there was no value to name. The facts
  rows print each family token's pick, "sans (none matched)" for an
  empty pick, and the list when it has more than one name.
- **JSX's `family` admits `FamilyToken`** in its union, on the text row
  and on a span. Since the union already ends in `string & {}`, this
  documents the token rather than narrowing what `family` accepts.
- **`bg={T.code}` type-checks.** Decision 2 said it would not. A colour
  prop takes any string, which is the limit ADR 0027 already hit with a
  length token in a colour slot. So the brand refuses a family token in a
  length slot only, and in a colour slot it is `unknown-token` when the
  node is encoded. `examples/node/tools/types.tsx` pins the length case.

