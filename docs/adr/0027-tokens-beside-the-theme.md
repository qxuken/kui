---
status: proposed
date: 2026-09-12
---

# Tokens beside the theme: a constant the app already has, and a name the inspector can borrow

> **Proposed (2026-09-12), nothing built.** Backlog T4 asked whether an app
> whose palette *is* the design — the LCARS pomodoro, the mind map — should
> get the theme's mechanism for a vocabulary of its own: an open
> `tokens: { name → colour }` map beside the closed `Theme`, read back in
> four bindings, warned about when misspelt, listed by the devtools, and
> — the contested half — a colour prop that names a token, `bg="$peach"`,
> resolved in the core when the node opens. The entry said the ADR may
> keep the map and decline the prop, or decline the whole thing with a
> condition, and to decide from a count rather than an adjective.
>
> The count changed the answer. The entry's picture was "the LCARS app
> declaring its palette once and writing `bg="$peach"` on forty pills".
> The app declares its palette once *already*, in one `const C` of twelve
> names with not one literal outside it; `peach` is on **three** boxes and
> six labels a frame, the colour on forty nodes is `dim`, and **43 of the
> 69 reads** of those twelve names sit inside a ternary the model decides
> — the view is choosing the colour, not naming it. The mind map's
> branch palette is indexed by *data*, so a token name for it would be a
> string built per node per frame. Neither palette follows the OS.
> Neither app has a reader of its colours that a module constant does not
> reach. What survives is one thing: a person at the inspector asking
> "which peach is this", and that is a legend the devtools can hold, not
> a mechanism the theme needs. So: **the map is declined, the prop is
> declined, a devtools palette legend is proposed**, and the condition
> that reopens the map is written down.

## Context

### What ADR 0019 built, and what it declined

`Theme` is twenty-three colour roles, derived once a frame from
`env.system`'s appearance and accent unless the app pins or accents it
(`crates/kui-core/src/theme.rs`); `schema::THEME_ROLES` pins the four
bindings' readings to it; the stock widgets and the core's own chrome
paint from it. Its "considered options" reject **a registry of arbitrary
named tokens** on three grounds, each of which this ADR has to answer by
name:

1. *the stock widgets could not read it without agreeing on names;*
2. *a typo is a missing colour at runtime;*
3. *no binding could be generated from it.*

T2 (`docs/backlog/closed-2026-09.md`) is the precedent one axis over:
`Metrics` shipped as a closed sixteen-role struct with a door per
binding and **no prop reference** — `<box radius={metrics.radius}>` reads
the value and passes it, the way `<box bg={theme.surface}>` does. The
question here is whether colour, "where the duplication was", answers
differently.

### The two apps, counted

ADR 0019 counted literals across the repo — 199, 87 distinct, `#8a8fa3`
by hand in sixteen files — and that number was its argument. The same
kind of count over the two apps that raised T4, from their source and
from a headless frame of each (`nodes()` after `setInspect(true)`, glyph
and segment quads from `decodeQuads`), on 2026-09-12:

**The LCARS pomodoro** (`playground/kui/my-app`, alpha.11):

| what | count |
|---|---|
| named colours (`const C` in `lcars.tsx`) | **12** |
| reads of them, in two files | 69 |
| hex literals anywhere else in `src/` | **0** |
| reads inside a ternary or an index the model decides (`m.blink ? C.red : C.tangerine`, `MODE_COLOR[mode]`, `i < filled ? color : C.dim`) | **43** |
| call sites deriving a colour by arithmetic (`hoverOf` / `pressOf`, a lift toward white) | 9 |
| nodes per frame, wide tier (1040×720), stopped | 176 |
| of them with a `bg` | 72 (the smoke test's "72 solid") |
| of them text | 35 |
| distinct `bg` values on screen / distinct text colours | 12 / 8 |
| nodes painting `dim` (the pips and segments) | **40** |
| nodes painting `peach` — the entry's example | 3 boxes + 6 labels |
| nodes with a hover and a pressed shade the app computed | 11, so **22 derived colours** a frame that no name in `C` is |
| compact tier (700×480): nodes / bg / text | 114 / 43 / 25 |

Its alpha.11 notes say what it wants, in its own words: "`ctx.theme()`
roles for the panel — deliberately not adopted. The theme is for a view
that wants to follow the OS; this one does not, and its palette is the
design." And under *what to watch*: "the next release that gives the
tooltip a role we would want to colour — its background to LCARS black,
its text to the peach — is the release the pin becomes a palette." That
is a wish for **role overrides on the stock widgets**, which `setTheme({
appearance: 'dark', raised: C.black, fg: C.peach })` is today. It is not
a wish for tokens.

**The mind map** (`playground/kui/mind-maps`):

| what | count |
|---|---|
| named colours in `theme.ts` | 10 flat + a branch palette of 7 × {solid, soft, edge} = **31** |
| hex literals inline in `view.tsx` | **13** — hover and pressed shades, shadow washes with alpha, white, one translucent overlay |
| text runs given a colour by hand | 15 (14 `<text color>`, one `<span color>`) |
| nodes per frame, starter doc (13 mind nodes) | 74 |
| of them with a `bg` / a border / text / a `line` | 41 / 17 / 31 / 12 |
| text nodes painting `ink` — which is the theme's `fg` role a shade off (`#e8ecf5` against `#e8e8ea`) | **19 of 31** |
| flat constants that are an ADR 0019 role under another name (`bg`, `panel`→`surface`, `panelBorder`→`border`, `ink`→`fg`, `inkDim`→`muted`, `inkFaint`→`faint`, `selectRing`→`focus_ring`) | 7 of 10 |
| branch colours chosen by data (`theme.hue(n.hue).soft`, `n.hue` a model field) | every node, edge and dot of every branch |

Its findings say "this app hardcodes a dark theme and adopts none of it",
and "gives all fifteen text runs a colour" — nineteen of the runs on
screen are `ink`, which a `<text>` with no `color` would paint as `fg` on
the pinned dark base, one shade apart.

### What the numbers say, one at a time

- **Both apps already have the map.** A TypeScript `const C = {...} as
  const` is typed, tree-shaken, and a compile error when misspelt.
  `theme().tokens.peach` would be a `number` off an index signature —
  `peech` types the same — and a *runtime* warning when misspelt. For the two apps in evidence the map is a
  regression in the property ADR 0019's second ground was about.
- **The view is the resolver.** Sixty-two per cent of the pomodoro's
  colour reads are chosen by model state. A token change in a frame where
  the view does not run cannot happen in this core: every binding is
  immediate mode, the view runs on every frame that is drawn, and the
  only way an app's tokens change is the app calling a setter and
  redrawing. "A token change repaints without the view running" is a
  property the *theme* needs — the OS changes it under the app, hence
  the `system` event — and no app-declared table has.
- **A name is not the unit.** The pomodoro's forty most-painted nodes
  paint `dim` on one arm of a ternary and a data-chosen colour on the
  other; the mind map's branches index a palette by a model integer. A
  prop that names a token would be `bg={i < filled ? '$gold' : '$dim'}`
  and ``bg={`$hue${n.hue}.soft`}`` — the second a string allocated per
  node per frame to be hashed back into the index it came from.
- **A fifth of the paint is derived, and outside any name.** Eleven pomodoro controls carry
  a hover and a pressed shade computed by `lift(c, 0.3)` and `lift(c,
  0.55)`; the mind map's thirteen inline literals are the same thing
  written out. A token set either grows a variant per shade (36 names
  for 12 colours) or the app keeps the arithmetic, and then `$peach` on
  `bg` sits beside `hoverBg={hoverOf(C.peach)}` — half a mechanism.
- **Nothing follows the appearance.** Neither palette has a light half.
  The only per-appearance token in evidence is the mind map's seven
  flat constants that are theme roles wearing other names, and the
  answer for those is ADR 0019's, unchanged: read the role.
- **The one benefit left standing is a name in the inspector.** The
  devtools' node inspector (ADR 0024, decision 10) shows `bg` as
  `#ffcc99`. A person reading it would rather see `peach`. That needs
  the names to exist somewhere the panel can see, and it needs nothing
  else — no wire encoding, no per-node resolve, because a *reverse*
  lookup from the painted value to a name at display time gives the
  same answer for every node that painted a named colour.

### What a token reference would cost on the colour path

The entry's first bullet: is the prop worth its wire shape? Read from
the code, then measured on one row.

**Today.** Node's encoder (`packages/kui/encoder.js`, `color(v)`) takes a
number as-is (`v >>> 0`) and a `'#rrggbb'` string through a `Map` from
string to `u32`, filled once per distinct string for the encoder's life
— so a hex string is already one map hit per node per frame, and a token
name would be the same hit. Every colour rides the frame stream as one
`f64` holding the `u32`; `binary::lower_binary`
(`crates/kui-node/src/binary.rs`) turns it into a `Color` through
`color_num`. Eight schema rows are `Kind::Color` (`bg`, `shadowColor`,
`scrollbarColor`, `scrollbarActiveColor`, `hoverBg`, `pressedBg`,
`focusBg`, `color`), and two more carry a colour inside a composite
(`border`'s second half, `cells`' `cursorColor`), so a distinct encoding
is **ten decode sites** in Node and one encode site, plus the `border`
and text-style paths in Lua and C. The `u32` slot has room for the
distinction — a negative `f64`, or a reserved alpha — so the wire grows
by zero bytes and one sign test per colour on decode; that half of the
entry's claim ("the common `u32` path pays nothing") holds.

**In the core**, the reference has to live in a `Color`-shaped slot
until the node opens. `Color` is four `f32`s, `NodeSpec` is pinned under
256 bytes (`node_spec_stays_small`), and a side `Option<u16>` per colour
row is the field growth C15 spent a round avoiding. The prototype used
the other shape: a NaN in `r` carrying the index in `g`
(`Color::token(i)`), resolved in `open_content` beside the `accent`
substitution — the exact door ADR 0019's decision 7 already walks
through for one role — so nothing downstream sees the sentinel. That is
the shape ADR 0019 decision 5 rejected for `Option<Color>`, and it is
not the same case: there the `None` had to survive to the shaping cache
and four bindings; here it dies at the door it was resolved at.

**Measured** (`scripts/bench-check.sh af043d4`, HEAD the prototype commit
on `scratch/t4-token-resolve`, one NaN test per node on `bg`, this
machine alone):

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
nodes** at most, which is the price of one array index. Two things the
run says that the table does not: `frame_10k_rects_all_transitioning`
read +5.2% at ±0.8%, an unguarded row bench-check marks *touched*
because `grid()` grew the `tokens` branch, so one run cannot tell the
resolve from the builder's new shape — it is the class C29 is already
bisecting, and it is why the ADR does not say "free"; and the base's
first run of `frame_10k_rects` was 768 µs against 731 µs on its second,
which is the spread the machine showed and the reason the rows are
read against it rather than against each other.

So the cost is not the argument against the prop: on the core's side it
is inside the noise floor on both paths, and on Node's wire it is a sign
test. The argument is that the two apps in evidence would pay it to
get a name they already have, in a form their type checker can no longer
see, for a resolve their view already performs.

### Who else reads a host's colours

ADR 0014 decision 3: a slot's parameters are a `Value` the host declares
every frame. A guest that paints in the host's vocabulary reads
`params.peach` — a map of hex strings the host builds from its own `C`
— and the contract is the two of them, as the entry says it should be.
The one reader a module constant cannot reach, an extension written
against a host's palette *without* the host passing it, is a plugin
ecosystem; there is none in the field, and the condition below names
it.

## Decision

1. **`Theme` stays closed, and nothing is added beside it.** No `tokens`
   on `Theme`, on `ThemeOverrides` / `setTheme`, on `KuiTheme`, on Lua's
   `env.theme`, and no `theme().tokens.x` / `kui_theme_token` /
   `ui.theme().token("x")` readers. ADR 0019's first ground stands as
   written — *the widgets read roles* — and it is also why the map buys
   the app that asked nothing: what the pomodoro wants for its tooltip is
   `raised` and `fg` overridden, which `setTheme` takes today. Its second
   ground is answered by the count rather than by a warning: in both apps
   the name is a typed constant and a typo is a compile error; an
   `unknown-token` warning would be the runtime error ADR 0019 declined,
   arriving one layer later. Its third ground is conceded: a map is one
   shape per binding, but what could not be generated was the *type*,
   and an index signature that types `peech` as it types `peach` is what
   a TypeScript app would get back for a `const` it can misspell nowhere.
2. **No colour prop names a token.** No `$name` spelling on any of the
   ten colour slots, in any binding, no distinct wire encoding, no
   per-node resolve, no `unknown-token`. Not for cost — the table above
   says the core would not notice — but because the thing it would buy,
   a repaint without the view, does not exist in an immediate-mode core
   where the only writer of the table is the app; because 43 of 69 reads
   are the view choosing between names, which a name-in-a-prop makes into
   a ternary of strings; because a data-indexed palette turns into a
   string built per node; and because the derived shades — 22 a frame in
   the pomodoro — are outside any name.
3. **A token that follows the appearance is a role.** No per-appearance
   token set (`tokens: { light: {...}, dark: {...} }`). An app colour
   that has a light and a dark value is either one of the twenty-three
   roles under another name (seven of the mind map's ten are) and reads
   the role, or a role the theme lacks and is proposed against
   `examples/rust/features/theme.rs` the way ADR 0019 says the next one
   is. `Theme::raise` and the derive are the one place the base branch
   lives, and a second table beside it would be that branch written
   twice.
4. **A guest reads a host's colours through the slot's parameters.**
   ADR 0014 decision 3 is the seam and it already carries a `Value` map;
   a host with a vocabulary hands the guest the names it means the guest
   to use, per slot, per frame. Nothing new.
5. **The devtools take a palette legend.** Proposed, unbuilt, and the
   one piece of T4 the count left standing:
   `Core::set_devtools_palette(&[(&str, Color)])` beside
   `set_devtools_legend`, `setDevtoolsPalette` on Node's `Ctx` and
   `KuiWindow`, `kui_set_devtools_palette` in C (a new function, no ABI
   bump under ADR 0006), stored in `SessionState::devtools` with the key
   legend. The facts tab lists the names with their swatches; the node
   inspector's paint group, when a node's `bg`, border, shadow or text
   colour equals a legend entry, prints the name after the hex. It is a
   reverse lookup at display time over a table of a dozen entries, so the
   wire, the spec, the open path and the corpus are untouched, a frame
   with the panel off pays nothing, and a colour the legend does not name
   prints as it does today. Names are the app's, and two entries with
   one value print both.
6. **What reopens decision 1.** An extension in the field — not the
   host's own code — that paints from a host's palette across more slots
   than a per-slot `Value` map is reasonable for, and wants the table by
   name; or a binding with no constant of its own to hold a colour in,
   which today is none of the four. Reopening decision 2 wants a host
   whose view does *not* run when its colours change, which would be a
   retained-tree host this core does not have.

## Considered options

- **The entry's shape whole** — map, four readers, `unknown-token`, facts
  tab, and `$name` on ten colour slots with a distinct encoding.
  Declined by the count: each half is argued above, and the sum is a
  mechanism whose two users in the field would have to un-type a constant
  to adopt it.
- **The map without the prop** — the entry's own fallback, and T2's
  shape (a table with readers and no reference). Declined, and this is
  where the count changed the answer: T2's table is *the stock widgets'*
  metrics, read by the widgets, so a door to read it back is the only way
  an app's own control can agree with `<button>` on a radius. A token map
  is read by no widget — decision 1 of ADR 0019 is what makes the widgets
  themed — so its readers would be the app reading back what it wrote,
  through a door slower and less typed than the constant it wrote it
  from.
- **The prop without the map**, names resolved in the binding
  (`'$peach'` looked up in the encoder's own table, the core never
  knowing). Declined: it is what `C.peach` is, spelt as a string the type
  checker cannot check.
- **Tokens as an open extension of the closed struct** — `Theme` grows a
  `custom: Vec<(String, Color)>` so `THEME_ROLES` and the corpus stay
  pinned and the rest floats. Declined: it is the map with the struct's
  `Copy` gone (`ThemeSource::Pinned` carries a `Theme` by value on
  purpose, `theme.rs`'s own doc says why), for the readers decision 1
  declines.
- **A per-appearance token set** — declined as decision 3; the `raise`
  branch written twice.
- **Resolve at paint rather than at open**, a token index carried in the
  quad so the renderer substitutes. Declined without measuring: it moves
  the table across the display-list boundary and into `KuiQuad`'s
  layout, which is an ABI bump (ADR 0006) for a benefit that does not
  exist in immediate mode.
- **Nothing at all, including the legend.** The nearest alternative to
  what is proposed, and a fair one: the pomodoro's rounds found their
  controls by the access tree and not by colour, and the inspector's
  hex is what the app's own `C` says. Kept as decision 5 because it is
  the only piece with a reader the count did not remove — "which peach
  is this" is the question T4 itself says the panel is for, and the
  inspector answers it in hex today — and because it costs a table in the
  session and a lookup in a tab that is already reading `NodeInfo`.

## Consequences

- Nothing ships to an app from this ADR. `CHANGELOG.md` and
  `docs/props.md` are untouched; no row, no warning, no reader.
- The two apps keep their `const C` and their `theme.ts`, which is the
  outcome the count recommends. The pomodoro's actual wish is one line
  it can write today — `setTheme({ appearance: 'dark', raised: C.black,
  fg: C.peach })` — and this ADR is where that is written down against
  its report. The mind map could drop `color` from nineteen of its
  thirty-one text nodes and paint the role.
- When decision 5 is built: one door per binding, one row group in the
  facts tab, one suffix in the inspector, tests that a named colour
  prints its name and an unnamed one does not, and that the panel off
  costs no lookup. No corpus scene — the corpus does not draw the panel.
- The prototype on `scratch/t4-token-resolve` (one commit over
  `af043d4`) is the measurement and is **not for merge**: it carries the
  sentinel `Color`, `Core::set_tokens`, one resolve in `open_content`
  and a bench row, and its number is in the table above.
- ADR 0019's "considered options" paragraph stands. This ADR is the
  second time a registry was asked for and the first time it was counted
  against a real app; the answer was the same and the reasons are now
  numbers.

### Not done here

- **The legend itself** (decision 5): proposed, waits for the build.
- **A `raised`/`fg` override in the pomodoro** is the app's line to
  write, not this repo's.
- **Names for the mind map's branch palette** would be twenty-one
  legend entries for seven hues; whether a legend wants a *family*
  (`hue3` naming three values) is a question for the day the panel
  shows one.
- **A retained-tree host** is the one thing that would make decision 2
  wrong. None is planned; ADR 0016 is the nearest the core has come to
  retaining a frame, and it declined.
