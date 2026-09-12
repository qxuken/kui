---
status: proposed
date: 2026-09-12
---

# Tokens beside the theme: named colours and lengths an app declares, referenced by name, resolved by the binding

> **Proposed (2026-09-12), nothing built.** Backlog T4 asked whether an app
> whose palette *is* the design — the LCARS pomodoro, the mind map — should
> get the theme's mechanism for a vocabulary of its own. This document was
> written twice on the same day. The first draft counted the two apps and
> declined: both already hold their colours in a typed constant, 43 of the
> pomodoro's 69 reads are a ternary the model decides, and a `$peach` string
> in a prop would trade a compile-time name for a runtime warning. The
> review of that draft pushed back on the *shape* rather than the count:
> the typo objection is answered at the declaration if the names are typed
> there; a token should be themed or not with one spelling for the common
> case; and the same vocabulary problem exists one axis over, in
> **lengths** — the pomodoro carries six size constants and a
> twenty-five-field tier table beside its twelve colours.
>
> So this is the second draft, and the count still stands underneath it.
> What is proposed: two small tables beside `Theme` and `Metrics` —
> **colour tokens** with a light and a dark half (`same()` for the common
> case) and **length tokens** in logical px — declared once, resolved once a
> frame, read back in four bindings, and **referenced by name in any colour
> or length prop** (`bg={T.peach}`, `width={T.sideW}`), the reference typed
> at the declaration by `defineTokens` so a misspelt name is a type error
> and not a warning. **Every binding declares, Lua included**: a table is
> kept per origin, so an extension's names are its own and a guest cannot
> shadow its host. The reference rides the wire as a **tagged prop id**
> and is resolved by the binding as it lowers the node, from the core's
> table — so `NodeSpec` does not change, the core resolves nothing at open,
> and the prototype's number below is an upper bound on a cost the built
> shape does not pay. The theme's roles and the metrics' roles are
> reachable by the same spelling (`$surface`, `$radius`), so an app has one
> way to say "this named thing" whoever named it.

## Context

### What ADR 0019 built, and what it declined

`Theme` is twenty-three colour roles, derived once a frame from
`env.system`'s appearance and accent unless the app pins or accents it
(`crates/kui-core/src/theme.rs`); `schema::THEME_ROLES` pins the four
bindings' readings to it; the stock widgets and the core's own chrome
paint from it. `Metrics` (T2, `metrics.rs`) is the same shape for sixteen
lengths, with `METRIC_ROLES`. Neither has a prop reference: an app reads
`theme.surface` or `metrics.radius` and passes the value.

ADR 0019's "considered options" reject **a registry of arbitrary named
tokens** on three grounds, answered here by name:

1. *the stock widgets could not read it without agreeing on names* —
   still true, and still not the point: the widgets read roles, an app's
   tokens are the app's, and the pomodoro's wish for its tooltip is a role
   override `setTheme` takes today (`raised: C.black, fg: C.peach`).
2. *a typo is a missing colour at runtime* — true of a string registry
   looked up by hand. Decision 5 puts the names into the type system at
   the declaration, so `T.peech` does not compile; the runtime warning
   exists for the bindings without a type checker and is the same
   `unknown-prop` family.
3. *no binding could be generated from it* — conceded: the set is the
   app's, so nothing per token is generated. What *is* pinned is the
   shape — one table per kind, one tag on the wire, one reader per
   binding — and that is what the corpus scene checks.

### The two apps, counted

ADR 0019 counted literals across the repo — 199, 87 distinct, `#8a8fa3`
by hand in sixteen files. The same kind of count over the two apps that
raised T4, from their source and from a headless frame of each
(`nodes()` after `setInspect(true)`, glyph and segment quads from
`decodeQuads`), on 2026-09-12:

**The LCARS pomodoro** (`playground/kui/my-app`, alpha.11):

| what | count |
|---|---|
| named colours (`const C` in `lcars.tsx`) | **12** |
| reads of them, in two files | 69 |
| hex literals anywhere else in `src/` | **0** |
| reads inside a ternary or an index the model decides (`m.blink ? C.red : C.tangerine`, `MODE_COLOR[mode]`, `i < filled ? color : C.dim`) | **43** |
| call sites deriving a colour by arithmetic (`hoverOf` / `pressOf`, a lift toward white) | 9 |
| named lengths as module constants (`SIDE_W`, `BAR_H`, `ELBOW_H`, `GAP`, `OUTER_R`, `INNER_R`) | **6**, read 31 times |
| lengths in the viewport tier table (`layout(w, h)` → `L`) | **25 fields × 3 tiers**, read 55 times as `L.x` |
| nodes per frame, wide tier (1040×720), stopped | 176 |
| of them with a `bg` / text | 72 (the smoke test's "72 solid") / 35 |
| distinct `bg` values on screen / distinct text colours | 12 / 8 |
| nodes painting `dim` (the pips and segments) | **40** |
| nodes painting `peach` — the entry's example | 3 boxes + 6 labels |
| nodes with a hover and a pressed shade the app computed | 11, so 22 derived colours a frame that no name in `C` is |
| compact tier (700×480): nodes / bg / text | 114 / 43 / 25 |

Its alpha.11 notes: "`ctx.theme()` roles for the panel — deliberately not
adopted. The theme is for a view that wants to follow the OS; this one
does not, and its palette is the design." And: "the next release that
gives the tooltip a role we would want to colour — its background to
LCARS black, its text to the peach — is the release the pin becomes a
palette."

**The mind map** (`playground/kui/mind-maps`):

| what | count |
|---|---|
| named colours in `theme.ts` | 10 flat + a branch palette of 7 × {solid, soft, edge} = **31** |
| hex literals inline in `view.tsx` | **13** — hover and pressed shades, shadow washes with alpha, white, one translucent overlay |
| named lengths as module constants | **15** |
| text runs given a colour by hand | 15 (14 `<text color>`, one `<span color>`) |
| nodes per frame, starter doc (13 mind nodes) | 74 |
| of them with a `bg` / a border / text / a `line` | 41 / 17 / 31 / 12 |
| text nodes painting `ink` — the theme's `fg` role a shade off (`#e8ecf5` against `#e8e8ea`) | **19 of 31** |
| flat constants that are an ADR 0019 role under another name (`bg`, `panel`→`surface`, `panelBorder`→`border`, `ink`→`fg`, `inkDim`→`muted`, `inkFaint`→`faint`, `selectRing`→`focus_ring`) | 7 of 10 |
| branch colours chosen by data (`theme.hue(n.hue).soft`, `n.hue` a model field) | every node, edge and dot of every branch |

### What the count settles, and what it does not

- **Both apps already keep one table, and keep it well.** Zero stray
  literals in the pomodoro; one `theme.ts` in the mind map. The feature
  cannot be justified as "give them a table" — it has to be justified by
  what a table *in the core* does that a module constant cannot: a light
  half the core picks, a name the inspector can print, a value a Lua or
  C guest can read, and one spelling for the app's names and the stock
  roles.
- **The view chooses between names, and that is neutral.** With
  `bg={m.blink ? T.red : T.tangerine}` the ternary reads as it does with
  `C.red`. A reference does not remove the branch and does not add one.
  What it removes is the `isDark ? … : …` an app would write to give a
  colour a light half — the mind map's seven role-alikes, today pinned
  dark.
- **A name has to be checked where it is written.** The first draft's
  strongest objection, and the pushback's answer: `defineTokens` returns
  the names as literal types, so the reference is `T.peach` and not
  `'$peach'`. The string is what rides; the type is what the author
  sees.
- **Lengths have the same shape and a better reason.** The pomodoro's
  tier table is twenty-five names re-chosen on every resize; a token
  table re-declared on `resize` is exactly that, with the inspector
  printing `sideW` on the sidebar's box instead of `132`. Nothing in
  `Metrics` covers it — those are the stock widgets' sixteen.
- **Derived shades are outside v1.** Twenty-two hover and pressed
  colours a frame in the pomodoro are `lift(c, t)`; the mind map writes
  the same thirteen out. A derived token (`peach.hover = lift(peach,
  0.3)`) is a later step, not this one; the app keeps its arithmetic on
  `hoverBg`.
- **The inspector is a reader.** ADR 0024 decision 10's paint group
  prints `bg` as hex and `padding` as numbers. With a table in the core,
  a reverse lookup at display time names both.

### What a token reference costs on the colour path

The entry's first bullet: is the prop worth its wire shape? Read from
the code, then measured on one row — for the *core-side* resolve, which
is the costlier shape and the one the decision does not take.

**Today.** Node's encoder (`packages/kui/encoder.js`, `color(v)`) takes a
number as-is and a `'#rrggbb'` string through a `Map` from string to
`u32`, filled once per distinct string for the encoder's life — a hex
string is already one map hit per node per frame, and a token name is
the same hit. Every prop rides the frame stream as its `id` then its
value(s), a colour as one `f64` holding a `u32`, a length as one `f64`, a
sizing as `(mode, value)`. `binary::lower_binary`
(`crates/kui-node/src/binary.rs`) walks the ids through `schema::by_id`
and already holds the `Ui` — it reads `ui.theme().fg` for a line's
default stroke — so it can read a token table from the same place.
Eight rows are `Kind::Color`, two more carry a colour in a composite
(`border`, `cursorColor`); twenty-odd rows are `Kind::F32` or
`Kind::Sizing`.

**In the core**, if the reference had to survive to `open`, it would need
a `Color`-shaped slot to hide in: the prototype used a NaN in `r`
carrying the index in `g` (`Color::token(i)`), resolved in
`open_content` beside the `accent` substitution — the door ADR 0019
decision 7 already walks through for one role — so nothing downstream
sees the sentinel. That is the shape decision 5 of ADR 0019 rejected for
`Option<Color>`; it is not the same case, because it dies at the door.
It is also not the shape taken here, for the reason under decision 4.

**Measured** (`scripts/bench-check.sh af043d4`, HEAD the prototype commit
`41798cd` on `scratch/t4-token-resolve`, one NaN test per node on `bg`,
this machine alone after waiting for three sibling test runs to end):

| row | af043d4 | prototype | Δ |
|---|---|---|---|
| `frame_10k_rects` (no token used — the unused-path cost) | 731 µs | 742 µs | +1.6%, at a ±4.8% run-to-run spread |
| `frame_1k_typical` (same) | 118 µs | 117 µs | −0.5%, at ±1.2% |
| `frame_10k_rects_tokens` (every `bg` a token — the used-path cost, against `frame_10k_rects` in the same run) | — | 745 µs (run 2), 737 µs (run 1) | +0.3% and −0.2% against that run's `frame_10k_rects` |

Read: the check passed on all four guarded rows (`deep_nesting_64_levels`
−5.1%, `frame_10k_rects_with_text_and_hits` −1.1%), and the two rows the
question named moved by less than their own run-to-run spread — the
unused path's one NaN test per node is not visible at 10k nodes, and the
used path's table read costs **three microseconds per ten thousand
nodes** at most, the price of one array index. Two things the run says
that the table does not: `frame_10k_rects_all_transitioning` read +5.2%
at ±0.8%, an unguarded row bench-check marks *touched* because `grid()`
grew the `tokens` branch, so one run cannot tell the resolve from the
builder's new shape — C29's class, and why this does not say "free";
and the base's first run of `frame_10k_rects` was 768 µs against 731 µs
on its second, which is the spread the machine showed.

Since the built shape resolves in the binding, the core's own path is
untouched and the table above is a ceiling. Node's cost is the map hit
it already pays per string plus one mask on the prop id per prop
decoded.

### Who else reads a host's tokens

ADR 0014 decision 3: a slot's parameters are a `Value` the host declares
every frame, and a guest could read `params.peach` today. A table in the
core is better for the case a guest paints in the host's vocabulary
across several slots — one `env.tokens` rather than the same map on
every `slot_with` — and a Lua guest that writes `bg = '$peach'` is
reading the host's table through the host's own spelling. The warning
matters more at that seam, which is why it names the token *and* the
node.

## Decision

1. **Two tables beside `Theme` and `Metrics`, per `Core`.**

   ```rust
   pub struct ColorToken { pub light: Color, pub dark: Color }  // same(c) sets both

   pub struct Tokens {
       colors:  Vec<(String, ColorToken)>, // index = ColorTokenId, declaration order
       lengths: Vec<(String, f32)>,        // index = LengthTokenId, logical px
       by_name: HashMap<String, TokenRef>, // TokenRef::Color(u16) | Length(u16)
       resolved_colors: Vec<Color>,        // this frame's half, picked at refresh_theme
   }
   ```

   Not inside `Theme` or `Metrics`: both are `Copy` on purpose
   (`ThemeSource::Pinned` carries a `Theme` by value, and `theme.rs` says
   why), and a `Vec` beside them costs nothing they have. **One `Tokens`
   per origin**, held as `HashMap<OriginId, Tokens>` on the core — the
   host's under `OriginId::HOST`, each extension's under its own — because
   an extension has a vocabulary as much as a host does (a Lua file panel
   has its own greys) and a guest writing into the host's table would be
   the one write the isolation by origin forbids. Per core, like the
   theme, so a window is one set of tables; hoisting to the session waits
   for a two-window app that wants it. Declaration order is the index, so a
   binding that lowered the declaration knows every index without a round
   trip. Size: the two apps would hold 12–31 colours and 6–40 lengths.
2. **Declared whole, by every binding, into the declarer's own table.**
   The declaration has one shape everywhere — colours and lengths named
   apart, since a colour and a length are both a number in Lua and can
   both be one in Node, so the kind cannot be read off the value:
   Rust `Core::set_tokens(Tokens::new().color("peach", c)
   .color_themed("dim", light, dark).length("sideW", 132.0))`, with
   `Tokens::color_id("peach")` / `length_id` for an app that holds the
   index; Node `ctx.setTokens({ colors: {...}, lengths: {...} })` and, as
   sugar, `setTheme({ ..., tokens })`; C `kui_tokens_set(ctx, const
   KuiColorToken *colors, size_t n, const KuiLengthToken *lengths, size_t
   m)` — new functions and new `[in]` structs, no ABI bump under ADR
   0006; **Lua** a `tokens = { colors = {...}, lengths = {...} }` global
   read at load beside `slots`, and `env.set_tokens { ... }` for a script
   whose table moves (a tier on resize), taking effect for the nodes the
   same `view` opens after the call. ADR 0019 made Lua read the theme
   only because the theme is *the host's*; a token table is the
   declarer's, and a Lua script that loads its own extensions (ADR 0014's
   amendment) is a host one level down. A write lands in the table of the
   origin that made it — a C or Lua plugin's `set_tokens` never touches
   the host's — and each call replaces that table whole. Tokens are
   orthogonal to `ThemeSource`: `setAccent(null)` / `derive_theme` leave
   them alone.
3. **Resolved once a frame, read back resolved.** `refresh_theme`
   fills `resolved_colors` from `theme.is_dark()` right after it resolves
   the theme — unknown appearance takes dark, ADR 0019 decision 4
   unchanged — so a themed token is frame-stable for the same reason the
   palette is. Readers get this frame's value, and they read **their own table
   over the host's**: an extension's `env.tokens.colors.peach` is its own
   `peach` if it declared one and the host's otherwise, so a guest can
   paint in the host's vocabulary without the host passing it and still
   name a grey of its own. Node `ctx.tokens()` as `{ colors, lengths }`
   (colours as `0xRRGGBBAA`, lengths as numbers), Lua `env.tokens.colors
   .peach` / `env.tokens.lengths.side_w`, C `bool kui_token_color(ctx,
   KuiStr, uint32_t*)` and `kui_token_length(ctx, KuiStr, float*)`, Rust
   `ui.token_color(id)` / `ui.token_length(id)` and the by-name forms.
4. **A reference by name in a prop, resolved by the binding as it
   lowers.** `'$peach'` is accepted on every colour slot (the eight
   `Kind::Color` rows, `border`'s colour, `cursorColor`, a `<span>`'s
   `color` and `bg`) and `'$sideW'` on every length slot (every
   `Kind::F32` row, every `Kind::Sizing` row as `Fixed(px)`, the `pad`
   shorthand's values, `border`'s width). On Node's wire the prop's id
   carries a tag — `id | 0x8000`, ids being small — and the value slot
   holds the token index; **zero extra bytes, one mask per prop on
   decode**. `lower_binary` resolves it from `ui`'s table before
   `schema::apply`, so `Parsed` and `NodeSpec` never see a reference and
   the core's open path is untouched. Lua's spec reader does the same
   where it parses a colour string today (`color_hex_str`'s caller),
   **against the table of the origin whose view is running**, then the
   host's — the same two-step the readback takes, so `$peach` means the
   same thing read and written. Node's index refers to the host's table,
   Node hosting no extensions. C passes a `u32` or a `float` with no
   room for a tag, so **a C prop carries no reference**: a C host or
   plugin declares its table, reads the value back and writes it, which
   is what a C app does with `#define`. Rust reads the value too — a Rust
   app holds the id. A kind mismatch (`bg={T.sideW}`) is an encode-time error like
   `bad color`; an undeclared name raises **`unknown-token`** naming the
   token and the node, once per name per session as `unknown-prop` does,
   and paints transparent / measures 0.
5. **The names are typed at the declaration.** Node ships
   `defineTokens`: given `{ colors: { peach: '#FFCC99', dim: { light,
   dark } }, lengths: { sideW: 132 } }` it returns `{ peach: '$peach',
   dim: '$dim', sideW: '$sideW' }` typed as literal strings, branded by
   kind —
   `ColorToken` / `LengthToken` — so `bg={T.peach}` type-checks,
   `bg={T.sideW}` does not, and `T.peech` does not exist. Zero runtime:
   the object *is* the references. The colour prop types widen from
   `number | string` to `number | \`#${string}\` | ColorToken`, the
   length props from `number` to `number | LengthToken`, which is a
   `.d.ts` change and a generated one (`ButtonProps` is already
   generated, F37).
6. **The stock roles have the same spelling.** `$surface`, `$fg`,
   `$accent` resolve to the theme's role and `$radius`, `$controlPadX`
   to the metric's, through the same tag with the role's index in the
   reserved range below the app's — so an app has one way to say "the
   named thing", and the mind map's seven role-alikes are `$surface`,
   `$border`, `$fg` with no `theme()` read. A declared token whose name
   is a role raises **`reserved-token`** and is ignored: the roles are
   the corpus's contract and an app does not shadow them. `defineTokens`
   ships `roles` — the twenty-three and the sixteen, typed — so `T.surface`
   and `roles.surface` are the same kind of thing.
7. **The devtools list and name them.** The facts tab shows every token
   with its light and dark swatch or its length, grouped by origin (the
   host's, then each extension's under its namespace); the node inspector's
   paint and box groups print the name after the value by reverse
   lookup over `resolved_colors` and `lengths` — `#ffcc99 peach`, `132
   sideW`. Two tokens with one value print both names; a value no token
   holds prints as it does today. No index is stored on the node, so a
   frame with the panel off pays nothing.

## Considered options

- **Decline the whole thing with a condition** — the first draft, and
  the count it rests on is still in this document. It was right that the
  apps have a table and that the view chooses between names; it was
  wrong that the typo objection was fatal (decision 5) and it did not
  look at lengths, where the pomodoro's tier table is the strongest case
  for a name the inspector can print. Superseded the same day.
- **Resolve in the core at `open`** — the prototype, measured above.
  Declined for the built shape: it needs a reference to hide in a
  `Color` (a sentinel) and in an `f32` (no room at all — a length can be
  any float), and every binding that lowers already has the core in
  hand. Resolving at the lower keeps `NodeSpec`, `Tree` and the digest
  exactly as they are.
- **A negative `f64` for colour references** — the first sketch's wire.
  Declined once lengths joined: `shadowY`, a float's `x`, a scroll offset
  can be negative, so the tag has to live on the id, and then colours
  use the same tag.
- **NaN-boxing the index into the value slot.** Declined: engines do
  not promise a NaN payload survives a `Float64Array` round trip.
- **Resolve in `defineTokens` at runtime** — the helper substitutes the
  hex itself and the core never knows. Declined: it is the module
  constant with extra steps, and it loses the light half, the inspector's
  name and the Lua guest.
- **Tokens inside `Theme` / `Metrics`** as an open tail. Declined: both
  are `Copy` by design.
- **A `<tokens>` element declared every frame**, immediate-mode purity.
  Declined: the table is a setting the way the theme source is, and
  re-sending forty strings a frame is what the encoder's cache exists
  to avoid.
- **Lua reads and does not write**, as it does for the theme. The
  first sketch's rule, borrowed from ADR 0019 without re-asking.
  Declined: the theme is the host's and a script has no business
  pinning it, but a token table is the declarer's, and a script already
  writes shared state through `set_window_size`, `set_focus` and
  `add_extension`. What the rule was protecting — the host's names —
  is protected by origin instead.
- **One table per core, last writer wins.** Simpler storage; declined
  because a plugin's `set_tokens` would replace the host's palette, and
  the `unknown-token` warning would then fire on the host's own nodes.
- **Derived tokens** (`hover: lift(peach, 0.3)`). Not in v1; a `derive`
  field on a colour token is the shape when an app asks, and the
  pomodoro's `lift` is the arithmetic to lift.
- **Per-density lengths** — a `scale` flag following `Metrics::scaled`.
  Declined as T2 declined it for the metrics: a length never scales by
  itself; re-declare on the event that changes the tier.

## Consequences

- Nothing ships to an app until it is built. `CHANGELOG.md` and
  `docs/props.md` are untouched now; when built, `props.md` gains one
  sentence on the colour and length rows and two warning lines.
- **The build, by crate.** `kui-core`: `Tokens`, `Core::set_tokens` /
  `tokens()`, the resolve in `refresh_theme`, the two warning codes, the
  reserved role range, the devtools rows and reverse lookup. `kui-node`:
  `setTokens` / `tokens()`, the tag in `lower_binary` at the ten colour
  and the length sites, `defineTokens` and the widened prop types in
  `index.d.ts`. `kui-lua`: the `tokens` global, `env.set_tokens`, `env.tokens`, `$`
  in the spec reader resolved by origin.
  `kui-ffi`: `kui_tokens_set`, the two readers, the header. Corpus: a
  `tokens` scene with a themed and an unthemed colour, a length on a
  sizing row and a pad, the appearance flipped mid-scene, and one
  undeclared name — Rust and C adapters write values, Lua and Node write
  `$`. The per-origin fallback is a `kui-core` test over a Lua panel
  (`examples/lua/features/slots` is the fixture): the guest's `$peach`
  is the host's until the guest declares its own.
- **The two apps.** The pomodoro becomes `const T =
  defineTokens({ ...C, ...tiers.wide })` with `setTokens({ ...C,
  ...tiers[L.tier] })` on resize, and `bg={T.peach}` reads as `C.peach`
  did; its hover arithmetic stays. The mind map's seven role-alikes become
  `$surface` and friends and gain a light half for free; its branch
  palette stays data-indexed and declares twenty-one tokens if it wants
  the inspector to name them.
- ADR 0019's "considered options" paragraph is amended by this document
  rather than edited: the registry it declined was untyped and unowned;
  this one is typed where it is written, owned by the app, and resolved
  by the binding that lowered it.
- The prototype on `scratch/t4-token-resolve` is the measurement only,
  and is not the shape to build from.

### Not done here

- **Derived tokens** and **per-density lengths**, as above.
- **C writing a reference.** A `KuiColor` with a tag would be an ABI
  change for a binding whose apps hold constants; the readers are enough
  until one asks.
- **Tokens in the session** rather than per core, for a two-window app.
- **A guest reading another guest's table.** The fallback is one step,
  own then host; a plugin that wants a sibling's names asks the host
  to pass them (ADR 0014 params).
- **Names for the mind map's branch palette** — whether a legend wants a
  *family* (`hue3` naming three values) is a question for the day the
  panel shows one.
