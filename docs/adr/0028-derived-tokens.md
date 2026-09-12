---
status: accepted
date: 2026-09-13
---

# Derived tokens: a colour computed from another

> **Accepted (2026-09-13), built the same day** — what the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed). Backlog T5, filed out of building
> ADR 0027: the one piece of the pomodoro's palette the tokens do not
> cover is the 22 hover and pressed shades a frame that are `lift(c, t)`
> — arithmetic on a name and no name themselves. ADR 0027 said "a
> `derive` field on a colour token is the shape when an app asks". This
> document does not take that shape on trust: it lays out **every place
> the arithmetic could live** — the table, the reference, the prop, the
> binding's helper, or where it is now — against the doors ADR 0027
> actually built, and then takes one: **A, with a chain of operations** —
> one flat name per derived token, its recipe a list of verbs folded in
> order, chosen 2026-09-13 over B's variants.

## Context

### What the two apps compute, counted again

ADR 0027 counted 22 derived colours a frame in the pomodoro and 13
literals in the mind map. Read from the source on 2026-09-13 (both apps
still on alpha.11, neither declaring tokens yet), the *shape* of those
numbers matters more than the numbers:

**The pomodoro** (`playground/kui/kui-lcars-pomodoro/src`):

| what | where | count |
|---|---|---|
| the arithmetic | `lcars.tsx:32` — `lift(c, t)`: each channel `+ (255 − ch) · t`, a mix toward white; `hoverOf = lift(·, 0.3)`, `pressOf = lift(·, 0.55)` | **2 recipes**, no third |
| call sites with a **constant** source | `app.tsx:476` `hoverOf(C.dim)`, `app.tsx:563` `hoverOf(C.red)` | 2 |
| call sites with a **variable** source | `lcars.tsx:151` `BarButton`, `lcars.tsx:168` `Block` — `hoverOf(color)` where `color` is the component's prop | 2, behind every lit control |
| what feeds the variable | `MODE_COLOR[mode]`, `m.mode === mode ? MODE_COLOR[mode] : C.violet`, `C.lilac`, `C.red` | the model decides |
| distinct sources ever lifted | any of the 12 | up to 12, so up to 24 shades |

The kit is **two operations over any colour**, not twenty-four chosen
shades. Nobody in the pomodoro ever wrote `peachHover`; they wrote
`hoverOf(color)` and let the model pick `color`. Any shape that asks the
app to name each shade has to also give it a way back from a *chosen*
token to that token's shade.

**The mind map** (`playground/kui/kui-mind-maps/src/view.tsx`): of the 13
literals, the hover and pressed ones are not lifts of anything — `#242938`
/ `#2e3446` on a `#1a1d27` panel are hand-picked greys, `hoverBg={isRoot
? '#333d61' : paint.edge}` and `hoverBg={paint.solid}` pick *another
palette entry*, and `#4a1f28` (danger hover) is a colour of its own. Four
are alpha washes (`#00000088`, `#00000055`, `#000000aa`, `#0f111788`) and
two are white. So the mind map's case for a derived token is `alpha`, and
otherwise it is served by ADR 0027 as built (declare the greys, reference
them). It is not the case that decides this.

### What already exists to compute with

All on `Color` and `Theme`, public since 2026-09-12 (`crates/kui-core/src/color.rs`, `theme.rs`):

| verb | what it is | who uses it today |
|---|---|---|
| `Color::mix(other, t)` | per-channel, keeps own alpha, extrapolates past 1 | `button_palette` (`mix(WHITE, 0.09)` / `mix(BLACK, 0.10)`), `Theme::raise` |
| `Color::with_alpha(a)` | the wash | the devtools' overlays |
| `Theme::raise(c, t)` | `mix(theme.front(), t)`: toward white on a dark base, black on a light one | ADR 0019's "one step up from this surface" |
| `Color::contrast(other)` | WCAG ratio | `ring_for`, the panel's ink |
| `Color::toward_contrast(toward, on, ratio, from)` | the loop: step toward `toward` until it clears `ratio` on `on` | `Theme::ink_for` |

The pomodoro's `lift` is `mix(WHITE, t)` exactly — LCARS lights up
whatever the base, so it is *not* `raise`, which would darken on a light
theme. Both verbs are wanted: one for a palette that is the design, one
for a palette that follows the base.

### What ADR 0027 built that a derived token has to fit

Read from `tokens.rs`, `index.js`, `encoder.js`, `kui_lua`'s
`parse_tokens`, `kui.h`:

1. **`ColorToken` is `{ light, dark }`, `Copy`, resolved on read**
   (`ColorToken::resolve(&theme)` picks the half; nothing is cached per
   frame). A derived token that is a third variant of this type resolves
   in the same place, one `mix` later.
2. **The index is the declaration order**, and Node's `index.js` mirrors
   the core's numbering with no round trip. Anything that appends entries
   (an expanded variant) has to append in an order both sides derive
   identically.
3. **Lua sorts its names** (`parse_tokens` sorts before declaring), so
   "the source is declared before the derived" is not a rule Lua can keep
   by position; a source has to be found by name.
4. **The wire carries a tag and an index, zero extra slots**
   (`id | tokenTag`, value = index). A reference that carries an
   *operation* needs somewhere to put it.
5. **C's `KuiColorToken` is an `[in]` struct**: appending a field moves
   the stride and is an ABI bump. A new struct and a new function are not.
6. **Roles are reserved names** that resolve through `role_ref` — a
   derived token *from* a role (`from: 'accent'`) is free to allow, and
   is the one case an eager per-half computation cannot cover (the accent
   changes under a fixed appearance).
7. **The devtools list the table** by origin with both halves, and the
   inspector reverse-looks-up a painted value. A recipe is one more column.

### The question, split

"Derived tokens" is three questions wearing one name, and the options
below differ on which they answer:

- **Where is the recipe written?** In the token table (declared, named),
  in the reference (`$peach.lift(0.3)`, anonymous), on the widget prop
  (`hoverBg` as "0.3 up from `bg`"), or in the binding's helper (JS
  computes, core never knows).
- **Where is it evaluated?** In the core at read (knows the theme's half
  and the roles) or in the binding at encode (knows neither; can only
  compute both halves ahead of time).
- **How is the result named?** One flat name per shade (`peachHover`), a
  variant on the source (`$peach.hover`), or no name (a value).

## Options

### Option 0 — Leave it in the app (status quo)

`hoverBg={hoverOf(color)}` keeps computing a hex string; `bg={T.peach}`
becomes a reference and `hoverBg` does not. ADR 0027's consequences
already say this ("its hover arithmetic stays").

| Dimension | Assessment |
|---|---|
| Core / wire / ABI change | none |
| Covers the variable source | yes — it is JS |
| Follows the appearance | no: `lift` sees the hex, not the half; an app with a themed source lifts the wrong half |
| Devtools name the shade | no — `#ffdbb8` with no name beside it |
| Lua / C parity | none needed; a Lua app writes its own `lift` |
| Type safety | `hoverOf` returns `string`, which `ColorProp` admits — nothing to check |

**Pros:** zero cost; the condition on T5 ("an app in the field declaring
hover and pressed shades as tokens") is not met by either app yet.
**Cons:** `hoverBg` is half a mechanism beside `bg`; `ColorToken` sources
with two halves cannot be lifted correctly from JS; the recipe is
invisible to the inspector — and it is the only literal-generating
arithmetic left in a codebase that just removed every other literal.

### Option A — A derived token in the table, one name per shade

The backlog's shape. A colour token is a value *or* a recipe over an
earlier token or a role — a **chain** of operations, applied in order —
stored as the recipe, resolved on read, listed with it.

```ts
const T = defineTokens({ colors: {
  peach: '#FFCC99',
  peachHover:   { from: 'peach', ops: [['lift', 0.3]] },
  peachPressed: { from: 'peach', ops: [['lift', 0.55]] },
  peachWash:    { from: 'peach', ops: [['lift', 0.3], ['alpha', 0.5]] },
  peachInk:     { from: 'peach', ops: [['readable', 'black', 4.5]] },
  // … ×12
}});
```

```rust
pub enum ColorToken {
    Value { light: Color, dark: Color },
    Derived { from: TokenRef /* Color(i) | ColorRole(i) */, ops: OpRange },  // a range into Tokens::ops
}
pub enum ColorOp { Lift(f32), Darken(f32), Raise(f32), Alpha(f32), Mix(TokenRef, f32), Readable { on: TokenRef, ratio: f32 } }
```

`Tokens::resolve(i, &theme)` follows `from` (a role reads the theme; a
token recurses — bounded, since `from` must resolve at declaration and can
only name what is already in the table), then folds the chain. Nothing on
the wire changes: a derived token has an index like any other.

| Dimension | Assessment |
|---|---|
| Core change | `ColorToken` gains a variant and an `Op`; `resolve` moves onto `Tokens`; `Tokens::derive(name, from, ops)`; the devtools' `TokenFact` gains a recipe |
| Wire / `NodeSpec` | untouched |
| Binding doors | Node: `setTokensRaw` pairs accept `{ from, ops }` objects; `index.js` numbers them as it does values. Lua: `parse_tokens` two-pass (values, then derived in dependency order by name). C: a new `KuiDerivedToken { name, from, ops, op_count }` `[in]` array and `kui_tokens_derive(ctx, …)` — no bump |
| Covers the variable source | **only through a map the app writes**: `hoverBg={HOVER[color]}` where `HOVER = { [T.peach]: T.peachHover, … }` — twelve lines the pomodoro does not have today |
| Follows the appearance | yes: the recipe runs on the half in effect |
| Devtools | `peachHover ■ #ffdbb8 lift(peach, 0.3)` |
| Declaration count, pomodoro | 12 + 24 = **36 names**, kept in step by hand or by a loop the app writes |
| Type safety | full: `T.peachHover` is `'$peachHover'` branded colour; a typo does not compile |
| Per-frame cost | one `mix` per derived read (~a dozen a frame) |

**Pros:** the smallest core change that names the shade; general (a
`readable` ink, an `alpha` wash, a `raise` off a role are the same
mechanism); every binding's story is a straight extension of ADR 0027's.
**Cons:** it answers a question the pomodoro did not ask. Twenty-four
declared names for two recipes, and the variable case — the one behind
every lit control — needs a lookup the app maintains beside the table.

### Option B — Recipes over the table: variants (A's storage, a different declaration)

The pomodoro's kit *is* two recipes over every colour. Declare the recipe
once; every colour token gets the variant; the name is the source's with
a suffix.

```ts
const T = defineTokens({
  colors: { peach: '#FFCC99', red: '#CC6666', /* …12 */ },
  variants: { hover: { lift: 0.3 }, pressed: { lift: 0.55 } },
});
T.peach            // '$peach'
T.peach.hover      // '$peach.hover'   — typed: `${'$peach'}.hover`
hoverBg={variant(color, 'hover')}   // `${color}.hover`, typed over ColorToken
```

In the core this is **Option A's storage with a helper**:
`Tokens::variant("hover", ColorOp::Lift(0.3))` appends one derived token
per value token, named `<source>.<variant>`, in source order — and
`index.js` appends in the same order. Lua's `variants = { hover = { lift
= 0.3 } }` expands the same way after its two-pass; C declares variants
through the same `kui_tokens_derive` array (expanded by the binding) or a
`kui_tokens_variant(ctx, name, op, t)` that expands in the core. A
per-token override (`peach: { value, hover: { lift: 0.5 } }`) is a
derived token declared by hand and wins by name, since `color_token`
already re-declares in place.

| Dimension | Assessment |
|---|---|
| Core change | A's, plus `Tokens::variant` and a name-with-a-dot convention |
| Wire / `NodeSpec` | untouched |
| Covers the variable source | **yes, by construction**: a variant of a chosen token is a string the type system can spell (`` `${C}.hover` ``), no map |
| Follows the appearance | yes, as A |
| Devtools | 12 rows, each with a swatch per variant and the recipe once in the header — or 36 rows as A; a panel choice |
| Declaration count, pomodoro | 12 colours + **2 recipes** — the same 14 things `lcars.tsx` holds today |
| Type safety | `T.peach.hover` exists, `T.peach.hovr` does not; `variant(c, 'hover')` is typed over the declared variant names |
| Naming risk | `.` in a token name; `$peach.hover` rides as one string and resolves as one name — nothing parses the dot at lower time |

**Pros:** it is the pomodoro's shape, so the migration is `hoverOf = c =>
variant(c, 'hover')` and no site moves; the table stays A's, so anything A
can express (a hand-derived `readable` ink) still can; the declaration is
n + k, not n × k.
**Cons:** two ways to declare a derived token (a recipe over all, a
recipe over one); the expansion order is one more thing `index.js` and
the core must agree on (the corpus pins it); a variant of a *role*
(`$accent.hover`) either applies to the reserved range too (then the
reserved range grows with the app's variants — no) or is declared by hand
as A's `{ from: 'accent', … }` — the latter, and the doc has to say so.

### Option C — The operation in the reference

Nothing is declared; the reference carries the recipe:
`hoverBg={lift(color, 0.3)}` where `lift(c: ColorToken, t) =>
`${c}|lift:${t}``, and the encoder writes the source index and the op.

| Dimension | Assessment |
|---|---|
| Core change | `TokenLookup::color_at` gains an op argument; the Lua spec reader parses `$name|op:t` |
| Wire | **changes**: the value slot holds an index today; an op needs a second tag bit or two more slots. ADR 0027's "zero extra bytes" is gone for these props, and frame version bumps |
| Covers the variable source | yes, directly |
| Follows the appearance | yes — evaluated in the binding against the resolved source, which is the half in effect |
| Devtools | **cannot name it**: nothing in the table holds `peach lifted 0.3`; the inspector prints `#ffdbb8` and a reverse lookup finds no token. The panel could print the recipe if the node kept it — which is the per-node cost ADR 0027 decision 7 refused |
| Lua / C | Lua parses an expression grammar in the hot path (cached per distinct string, as the encoder does); C has no reference at all, so nothing |
| Type safety | a template-literal type `` `${ColorToken}|lift:${number}` `` types the shape but not the range |

**Pros:** no declaration, no naming, no expansion order; the one shape
that is *exactly* `hoverOf(color)`.
**Cons:** it is a value with extra steps — the reason ADR 0027 declined
"resolve in `defineTokens` at runtime" applies: it loses the name. It
adds a grammar to every binding's reference reader and a wire shape for a
case the table can hold. Declined below unless the panel's naming is
given up on purpose.

### Option D — A relative shade on the hover and pressed props

Not a token at all: `hoverBg` and `pressedBg` accept a *shade relative to
`bg`* — `hoverBg={{ lift: 0.3 }}` — and the core resolves it against
whatever `bg` resolved to, when it paints.

| Dimension | Assessment |
|---|---|
| Core change | `InteractSpec::hover_bg: Option<Color>` → `Option<Shade>` (`Shade::Color(c) | Shade::Relative(ColorOp)`); `runtime/builder.rs:250` picks the state's colour today and would apply the op to the node's `bg` there. Two schema rows change kind |
| Wire | the two rows gain a second value shape (a `Kind::Shade`), mirrored in the encoder, Lua's parser and `KuiSpec` (a `uint32_t` today — so C carries a flag field, or keeps values) |
| Covers the variable source | **yes, trivially**: the app writes `hoverBg={HOVER}` once with `HOVER = { lift: 0.3 }`, and never mentions the source |
| Follows the appearance | inherited from `bg`, which is a token |
| Devtools | the inspector prints `hoverBg: +0.3 lift` — a recipe, no name, and none wanted |
| Generality | **hover and pressed only.** A `readable` ink, an alpha wash, a `dimText` off `dim` — none are this |
| Type safety | `hoverBg?: ColorProp | Shade` — fine |

**Pros:** the cheapest thing that makes *every* pomodoro site one
constant; a stock-widget-shaped idea (`button_palette` is exactly this,
hard-coded at 0.09 / 0.10 — a `Shade` would let the stock button read it
too); no table, no naming, no expansion order, no Lua two-pass.
**Cons:** it is a second mechanism for one pair of props. The day an app
wants a derived token that is not a hover — the mind map's washes, a
readable label colour — Option A is built anyway, and then there are two
ways to say "lifted 0.3". The `KuiSpec` change is the one place it costs
more than A.

### Option E — Derive in the binding's helper, declare plain tokens

`defineTokens` (and a Lua helper, and nothing in C) expands the variants
**to values** at declaration and hands the core 36 ordinary tokens:

```ts
const T = defineTokens(withVariants(
  { colors: C }, { hover: lift(0.3), pressed: lift(0.55) }));   // 36 plain colours, both halves lifted
```

| Dimension | Assessment |
|---|---|
| Core / wire / ABI change | **none** |
| Covers the variable source | yes, with B's naming (`$peach.hover` is just a declared name) |
| Follows the appearance | yes — the helper lifts each half; a *role* source cannot be followed (the helper does not know the accent) |
| Devtools | 36 rows, swatches and hex, **no recipe** — the panel cannot say `peach.hover` is `peach` lifted; the reverse lookup does name a painted hover |
| Lua / C | Lua gets a helper module or writes its own loop; C writes its own loop (as it does for `#define`) |
| Type safety | B's, in the helper's return type |

**Pros:** ships in `index.js` and `index.d.ts` alone; a v0 that proves
whether anyone declares variants before the core learns a verb.
**Cons:** the core never learns the relationship, so `readable` cannot
exist (it needs the theme and the loop), a role cannot be a source, and
Lua and C each carry a copy of the arithmetic — the four-copies problem
`Color::toward_contrast` was just moved to `Color` to end.

### Side by side

| | 0 app | A table, flat | B table, variants | C reference | D prop | E helper |
|---|---|---|---|---|---|---|
| core change | — | small | small + helper | small | small, in paint | — |
| wire / frame version | — | — | — | **yes** | two rows | — |
| C ABI | — | new struct, no bump | same as A | — | `KuiSpec` field | — |
| variable source (`hoverOf(color)`) | JS | app map | typed suffix | direct | one constant | typed suffix |
| themed source lifted right | no | yes | yes | yes | yes | yes |
| role as source (`accent.hover`) | no | yes | by hand | yes | n/a | no |
| `readable` / contrast verb | no | yes | yes | yes | no | no |
| devtools: name + recipe | none | both | both | neither | recipe only | name only |
| pomodoro declares | 12 + 2 fns | 36 | 12 + 2 | 12 + 2 fns | 12 + 1 const | 12 + 2 |
| Lua parity | own `lift` | two-pass parse | two-pass + expand | grammar | parser row | own loop |
| what it does not cover | everything | the variable case, cheaply | a role variant | naming | anything not hover | roles, contrast |

## Decision: Option A, with a chain of operations

**Build Option A — a derived token in the table, one flat name per
shade — and let its recipe be a chain, not one verb.** The table is
declared once and resolved on read, so a token that is "`peach`, lifted
0.3, at half alpha" costs a `Vec` walk at declaration and a fold of two
`Color` calls per read; there is no reason to make it two names. B's
`variants` were declined for now: one way to declare, no expansion order
for `index.js` and the core to agree on, and the variable-source map the
pomodoro needs is written in the app where it can be read — and B is a
loop that emits flat names, so it can be added as sugar later without
touching the core. In detail:

1. **`ColorToken` gains `Derived { from, ops }`.** `from` is a `TokenRef`
   — `Color(i)` for an earlier token in the same table, `ColorRole(i)`
   for a theme role. `ops` is a range into `Tokens::ops: Vec<ColorOp>`,
   so the token stays `Copy` and the table owns the chain. Resolved on
   read: `Tokens::resolve(i, &theme)` follows `from` until a value (a
   role reads the theme), then folds `ops` left to right. The chain is
   acyclic by construction: at declaration `from` (and every `TokenRef`
   inside an op) must already resolve in the table or to a role, or the
   declaration raises `unknown-token` naming the source and the derived
   token is dropped, as `reserved-token` drops a role's name. A derived
   token may derive from a derived one; the recursion is bounded by the
   index order.
2. **A closed verb set, each an existing method:** `lift(t)` =
   `mix(WHITE, t)`; `darken(t)` = `mix(BLACK, t)`; `raise(t)` =
   `Theme::raise`; `alpha(a)` = `with_alpha`; `mix(other, t)` where
   `other` is another `TokenRef`, resolved to its value for this frame;
   `readable(on, ratio)` = `toward_contrast(front-for-`on`, on, ratio,
   0.0)` where the "front" is black on a light `on` and white on a dark
   one (`readable_on`'s split), so the loop can always reach. **Ops
   compose in order** — `[lift 0.3, alpha 0.5]` is a translucent lit
   shade, `[alpha 0.5, lift 0.3]` is the same colour because `mix`
   keeps alpha, `[lift 0.3, readable on black]` is a lit shade pushed
   until it reads. An empty chain is an alias (`accent2: { from:
   'accent' }`). Not an expression language: a flat list, no nesting; a
   verb's operand is a number or a token name, never another chain.
3. **An op is a tuple, and the chain is a list of them, in every
   binding.** `[verb, operand…]`: `['lift', 0.3]`, `['alpha', 0.5]`,
   `['mix', 'ink', 0.5]`, `['readable', 'black', 4.5]`. A JSON object's
   key order does not survive the Node boundary (ADR 0027's finding 2),
   so the chain is an array; and the op itself is an array rather than a
   one-key object because the verb is then a position, not a key to
   search for — the same shape `setTokensRaw` already crosses in
   (`[name, value]` pairs), and in TypeScript a discriminated tuple
   union (`['lift', number] | ['mix', ColorRef, number] | …`) where an
   object would need an "exactly one key" check. Node `{ from: 'peach',
   ops: [['lift', 0.3], ['alpha', 0.5]] }`; a one-op declaration may
   write the tuple bare (`ops: ['lift', 0.3]`) and `index.js` wraps it,
   the two being told apart by the first element's type. Lua `{ from =
   'peach', ops = { { 'lift', 0.3 }, { 'alpha', 0.5 } } }` — the array
   part keeps order, and the parser reads `[1]` as the verb. C
   `KuiColorOp { uint8_t op; float t; KuiStr other; }` in an `[in]`
   array on `KuiDerivedToken { KuiStr name; KuiStr from; const
   KuiColorOp *ops; size_t op_count; }`, declared through
   `kui_tokens_derive(ctx, const KuiDerivedToken *, size_t n)` after
   `kui_tokens_set`, appending to the drawing origin's table. Rust
   `Tokens::derive("peach_hover", "peach", [ColorOp::Lift(0.3)])`, by
   name, so the builder checks the source at the call. No ABI bump
   (ADR 0006: new functions and new `[in]` structs).
4. **Node:** `setTokensRaw` colour pairs accept the recipe object;
   `index.js` numbers derived tokens as it numbers values, in declaration
   order, so the encoder's map needs no new case — `$peachHover` is a
   name. `defineTokens` types it as a colour reference like any other;
   `ColorProp` is unchanged. **The variable case is the app's map**:
   `const HOVER: Record<ColorToken, ColorToken> = { [T.peach]:
   T.peachHover, … }` and `hoverBg={HOVER[color]}` — twelve lines in
   `lcars.tsx` where `chan` / `hex2` / `lift` were, typed so a missing
   entry is a compile error if the record is declared over the union of
   `T`'s colour names.
5. **Lua:** `parse_tokens` declares values first, then derived tokens in
   dependency order by name (a token whose source is not yet declared
   waits; a cycle or a missing source is a load error naming both) —
   Lua sorts its names, so position cannot carry the order. The spec
   reader is untouched; `$peach_hover` is a name.
6. **Devtools:** the token list prints a derived token's recipe after
   its hex — `peachHover ■ #ffdbb8 peach → lift 0.3 → alpha 0.5` — and
   the inspector's reverse lookup already names it. `TokenFact` carries
   the two halves resolved against a light and a dark theme (a derived
   token has no stored halves) and the recipe as a string the core
   formats once when the facts are built.
7. **Resolve-on-read stays.** ADR 0027's building settled that nothing is
   rebuilt at `refresh_theme`; a derived token is a dozen `mix` calls a
   frame at the pomodoro's count. If a table with long chains ever shows
   on a bench row, the cache is a per-half `Vec<Color>` filled at
   `set_tokens` for tokens whose chain touches no role — a later step,
   measured first.
8. **Lengths stay values.** `gap2: { from: 'gap', ops: [['times', 2]] }`
   is the same mechanism one kind over, and nothing in the two apps asks
   for it. Named under "not done here".

**Why not B (variants).** It is A plus a loop that emits `<source>.<name>`
for every value token, and the pomodoro's declaration would be 12 + 2
instead of 12 + 24. It was set aside because it adds a second way to
declare a derived token and an expansion order every binding must
reproduce, and because the map it saves the app is the honest place for
"which shade goes with which colour". It stays the obvious sugar if a
third app declares the same two recipes over its whole palette.

**Why not D (a relative shade on the hover props).** The cheapest answer
to the pomodoro alone and the one closest to the stock button's own
`button_palette`. Not taken because it is a second mechanism: the mind
map's alpha washes and any `readable` ink need A regardless, and then a
`Shade` on two props is a third spelling of `mix`.

**Why not E (the helper expands to values).** `readable` and a role
source are the two things the mechanism exists for that a helper cannot
do, and Lua and C each carrying a `lift` is the four-copies shape the
repo just ended for `toward_contrast`.

**Why not C (the op in the reference).** It loses the name — the reason
ADR 0027 declined "resolve in `defineTokens` at runtime" — and adds a
grammar to every reference reader and a slot to the wire for a case the
table holds.

## Consequences

- **Nothing on the wire changes**, and `NodeSpec`, `Tree`, the digest and
  the frame version are untouched. A derived token is an index like any
  other; `color_at` resolves a chain deeper.
- **The pomodoro** declares its twelve colours and twenty-four shades in
  one `defineTokens` — the shades generated by a loop over `C` at module
  load if it likes, since the declaration is data — keeps a
  `HOVER` / `PRESS` map from token to shade, and `hoverBg={hoverOf(color)}`
  becomes `hoverBg={HOVER[color]}`. `lcars.tsx`'s `chan` / `hex2` /
  `lift` go. The inspector names every lit control's hover.
- **The mind map** declares its washes as `{ from: 'black', ops: [['alpha',
  0.53]] }` if it wants them named; its hovers are palette picks and
  stay references.
- **The corpus** `tokens` scene gains a derived token with a two-op
  chain, a role-sourced one, a derived-from-derived one and a missing
  source; the appearance flip already in the scene pins that a derived
  token follows its source's half. Rust and C write the resolved values,
  Lua and Node write `$` — the scene's existing split.
- **`props.md`** gains one sentence on `unknown-token` (a derived token's
  missing source, raised at declaration) and the recipe column on the
  tokens row.
- **What gets harder:** `ColorToken` is an enum where it was a struct, so
  the devtools' `TokenFact` and every `tok.light` / `tok.dark` read become
  a resolve; the two halves a panel shows for a derived token are computed
  against a light and a dark theme rather than read. A verb added later
  is a `ColorOp` variant, a `u8` in C, a key in Node and Lua, and a corpus
  row — four places, pinned by `abi_parity` and the scene.
- **Revisit** when a third app declares the same two recipes over its
  whole palette: that is B's `variants`, and it is a loop over this
  table.

## Action items

1. [x] `kui-core`: `ColorOp`, `ColorToken::Derived` with an op range,
   `Tokens::ops`, `Tokens::derive(name, from, ops)`, declaration-time
   source check raising `unknown-token`, `Tokens::resolve(i, &theme)`
   folding the chain, `TokenFact` recipe string, panel column. Tests: a
   two-op chain, order (`lift` then `alpha` against `alpha` then `lift`),
   a role source, a derived-from-derived, a missing source, an empty
   chain, `readable` reaching on a light and a dark `on`, both halves
   under a flip.
2. [x] `kui-node`: `setTokensRaw` accepting `{ from, ops }` with tuple ops (and the
   bare-tuple sugar wrapped in `index.js`), `index.js` numbering unchanged,
   `ColorTokenValue` widened in `index.d.ts` with `ops` a discriminated tuple union;
   `examples/node/tools/types.tsx` gains a derived token and a
   `Record<…>` map over the declared colour names.
3. [x] `kui-lua`: `parse_tokens` two-pass with dependency order and a
   load error naming a cycle or a missing source; a `slots` fixture with
   a guest deriving from the host's `peach`.
4. [x] `kui-ffi`: `KuiColorOp`, `KuiDerivedToken`, `kui_tokens_derive`,
   `abi_parity`, header comment; `examples/c/tools/conformance.c`.
5. [x] Corpus: extend the `tokens` scene; all four adapters.
6. [x] `props.md`, `CHANGELOG.md`, README's tokens paragraph; close T5 in
   the backlog with the outcome on top.
7. [ ] Migrate the pomodoro on the next alpha and count what the inspector
   names.

## Not done here

- **Derived lengths** (`times`, `plus`) — the same shape, no app asking.
- **Variants** (Option B) — a loop over this table that emits
  `<source>.<name>`; sugar for the day a third app declares the same
  recipes over its whole palette.
- **A `Shade` on `hoverBg` / `pressedBg`** (Option D) and the stock
  button reading its palette from one — filed as the follow-up if the
  stock widgets ever want their `0.09 / 0.10` to be a metric-like setting.
- **A cache of resolved halves** at `set_tokens` — decision 7, measured
  first.
- **Derived tokens in an expression on the wire** (Option C) — declined
  for losing the name; reopened only if the panel's naming is given up.

## What the building changed

1. **A derived colour is rounded to eight bits a channel** at the end of
   its chain. Found by the C adapter: C has no reference, so it reads
   `kui_token_color` (a `uint32_t`) and writes the value, and a computed
   colour — `0.86` in a channel — is not a multiple of `1/255`, so its
   quad digest disagreed with the reference's by an invisible amount. The
   core now resolves a derived token to `Color::hex(c.to_hex())`, which is
   the value every reader gets anyway, so all four bindings paint one
   quad. A chain over a derived source folds over the rounded value.
2. **`hover` and `pressed` are theme role names.** A token called `hover`
   is refused with `reserved-token`, so the pomodoro's shades are
   `peachHover` / `peachPressed`, never `hover`. The first unit test found
   it by declaring one.
3. **A source is the same table or a role — never the host's table.** The
   ADR's action item 3 imagined a guest deriving from the host's `peach`;
   the built rule is decision 1's as written (`from` resolves in the
   declaring table or to a role, checked at declaration, where the host's
   table is not in hand). A guest that wants a shade of the host's colour
   declares the value itself. The Lua test covers a script's own chain.
4. **Lua's missing source is the core's warning, not a load error.**
   Decision 5 said a cycle or a missing source is a load error; the built
   parser orders what it can by dependency and hands the rest to the core
   as written, so `unknown-token` names the pair the same way in every
   binding (the corpus scene's `bad` relies on it). A cycle's members all
   drop, each naming the other.
5. **C refuses a malformed op with `false` and adds nothing**, since a C
   call has no other way to say no: an `op` past `KUI_OP_READABLE`,
   `other` given to a verb that takes none or missing from one that does.
   `kui_tokens_derive` appends to the drawing origin's table after
   `kui_tokens_set` (the table is re-set whole underneath), so a C app
   declares values then recipes in two calls.
6. **`ColorOp` tuples are `readonly` in the type**, because a declaration
   written `as const` — the way `defineTokens` wants it, for the literal
   names — makes every tuple readonly. `ColorToken` / `LengthToken` are
   re-exported from the package root, so an app can type the map from a
   chosen colour to its shade (`new Map<ColorToken, ColorToken>`); a
   `Record` keyed by the branded literal does not index.
7. **The corpus scene grew by six tokens and five cells** (`solid` 3 → 8,
   a second `unknown-token`): a lift, a two-step chain that does not
   commute, a `raise` off the `surface` *role* (the declared `surface` was
   already refused, so the source *is* the role — one declaration pins
   two things), a step off a derived token, a `readable` against the
   themed ink that has to move on the dark base only, and the dropped
   one. Node writes `read`'s single op bare and Lua writes it bare too,
   so the sugar is pinned on both sides that have it.
8. **`ColorToken::resolve` is gone**; `Tokens::resolve_color(i, &theme)`
   is the one place a token becomes a colour, and the devtools'
   `TokenFact` gets its halves from `Tokens::halves`, which resolves a
   derived token under the frame's theme and under a theme of the other
   appearance with the same accent for the other swatch.
