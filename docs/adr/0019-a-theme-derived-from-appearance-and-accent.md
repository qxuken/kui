---
status: accepted
date: 2026-09-10
---

# A theme derived from appearance and accent

> **Accepted (2026-09-10), built the same day.** `env.system.appearance`
> has been reported to every binding since the day it was plumbed, and
> read by nothing. Not by the core, which is deliberate and stays that
> way; not by the stock widgets, which is not; and not by a single example
> in the repo, which is the tell. A view that wanted to honour the
> appearance had nothing to honour it *with* — no name for "the colour a
> card is" — so it wrote the hex out again, and so did the crate.
>
> This ADR adds the missing half: a **`Theme`**, plain data, derived from
> the two facts the OS already reports, replaceable whole by an app that
> has its own. The core still acts on the appearance exactly as much as it
> did before, which is not at all. What changed is that the *widgets* do,
> and that a view asking "what colour is a card here" now has an answer.

## Context

### The fact was plumbed and nobody could use it

`SystemEnv` carries four things the user set in the OS, and its own module
doc is firm about what the core does with them:

> Reduced motion does not shorten an animation, a dark appearance does not
> repaint anything: the view decides, because only it knows which of its
> colours is the background and which of its animations carries meaning.
> — `crates/kui-core/src/env.rs`

That is right, and this ADR does not touch it. But it describes only one
half of a contract. The other half — *the view decides, with what?* — was
never built. `appearance` reaches Rust (`ui.env().system.appearance`), Lua
(`env.system.appearance`), Node (`ctx.env().system.appearance`) and C
(`kui_env_set_system`), it has a row in `schema::ENV_FIELDS`, it has a
`system` event so a retained-tree host learns when it changes, and a grep
across `crates/` and `examples/` before this change found **not one reader
that branched on it**. The one place it appeared outside `env.rs` and the
schema was `runtime.rs`, putting it into that event's payload.

The accent was luckier and barely: `NodeSpec::accent` substituted
`env.system.accent` for a node's `bg`, and `widgets::button_with` took the
hover and pressed shades and the label colour off it. Two readers, both
about one control.

### The same six roles, spelled twenty-eight ways

Counting the colour literals in `crates/kui-core/src`, `crates/kui/src`
and `examples/` (excluding the conformance corpus, whose colours are
fixtures): **199 literals, 87 distinct values**. They are not 87 different
intentions. Grouped by what they are *for*:

| role | distinct values found | the most common |
|---|---|---|
| the window background | 7 | `#14161e` (9 uses) |
| a card or panel | 7 | `#1d202b`, `#242733` |
| body text | 5 | `#e8e8ea` (the `TextStyle` default) |
| secondary text | 5 | `#8a8fa3` — **22 uses across 16 files** |
| the accent | 6 | `#3b5bd4`, and `#7f9cf5` for the ring |
| a border | 4 | `#2a2d3a`, `#3a3e4e` |

`#8a8fa3` appearing by hand in sixteen files is the whole argument. It is
not a colour anyone chose sixteen times; it is a token nobody had.

Three examples went further and built the token set themselves.
`examples/rust/splitmux.rs`, `syntax_view.rs` and `modal_editor.rs` each
declare a `struct Pal` — independently, and with the *same field names and
the same values*: `bg`, `panel`, `border`, `fg`, `dim`, `faint`, `accent`.
`examples/rust/context_menu.rs` has the same set as loose consts under
different names (`BG`, `CARD`, `EDGE`, `TEXT`, `MUTED`, `ACCENT`). The
vocabulary in this ADR is not invented: it is what four files voted for.

### The default text colour is a dark-theme decision

`TextStyle::new` set `color: Color::rgb8(0xe8, 0xe8, 0xea)`
(`crates/kui-core/src/spec.rs`), and the schema row for `color` has said,
the whole time, "Text color; **default foreground** when omitted". There
was no default foreground — there was one constant, and it was near-white.

On the light base this ADR adds, `#e8e8ea` on the page background is
**1.22:1**. WCAG AA wants 4.5:1 for body text. A light-mode app built out
of `<text>` without a `color` prop was not merely off-brand; it was blank.

### Half the chrome was already the core's, and was already a constant

Three of the colours a frame paints are not a widget's at all:

- the focus ring, `#7f9cf5`, `crates/kui-core/src/runtime/emit.rs`
- the scrollbar thumb, white at 0.18 / 0.4 alpha, same file
- the selection tint, `select::TINT`, the editor's `EditOptions::accent`

ADR 0002 said of the first: "The focus ring's colour is still a constant
in the core, not a prop and not a theme value." That was the right call
when there was no theme to make it a value of. It is the wrong call now,
for a reason that is not aesthetic: a pale blue ring reads on `#14161e`
and is invisible on `#f6f7f9`, and a focus indicator nobody can see is not
a focus indicator. The scrollbar is worse — a white wash on a white page
is nothing at all.

### The same app already looked themed on one platform and not on another

Building the corpus for this, `examples/rust/context_menu.rs` under macOS
shows the platform's `NSMenu`, which follows the OS appearance because
AppKit draws it. The same app on Windows or Linux shows
`widgets::context_menu`, which was `MENU_BG` `#1d202b` and four more
constants. One app, two answers, and the drawn one was the wrong answer on
a light desktop.

### What the codebase already believed

ADR 0017, decision 7, drew the line this ADR needs, a year of design
before there was anything to draw it with:

> a paragraph drawn light grey on a dark card is grey **because of the
> app's theme**, and pasting it into a white document as grey-on-white is
> exactly how "copy with formatting" earns its bad name

"The app's theme" was already load-bearing vocabulary. It just had no
referent.

## Decision

### 1. A `Theme` is data derived from the two facts, and the core still acts on neither

`kui_core::theme::Theme` is a plain `Copy` struct of named colours.
`Theme::derive(appearance, accent)` builds one: the appearance picks a
base, the accent recolours the family that comes off it. Nothing about
this is a behaviour — it is a value, computed from `env.system`, that a
view may read, ignore, mutate or replace.

`env.rs`'s claim survives intact. A dark appearance still repaints nothing
*by itself*. What repaints is `widgets::button_with`, `widgets::tooltip`,
`widgets::context_menu` and their siblings — views, all of them, making
exactly the decision `env.rs` says views should make. The core's own
chrome is decision 6 below and is argued separately.

### 2. The roles are the ones four files already voted for

Twenty-three colours, in five groups: four surfaces (`bg`, `surface`,
`raised`, `sunken`), two lines (`border`, `border_strong`), three text
tiers (`fg`, `muted`, `faint`), six accent (`accent`, `accent_hover`,
`accent_pressed`, `on_accent`, `accent_soft`, `selection`), and the rest —
`focus_ring`, the two neutral interaction washes, three status colours,
two scrollbar states. Beside them ride `appearance` (which base this came
from) and `disabled_opacity`, which is not a colour.

**Roles, not a ramp.** `surface` is not "grey 800": it is *the colour a
card is*, and on the light base it is nearly white. There is no promised
ordering and no numbered scale, because a scale is a value system wearing
a role system's clothes — it forces every consumer to know which end is
"up", which is exactly the branch a theme exists to delete. `Theme::raise`
is the one place that branch lives.

`raised` earns its place against `surface` for a reason worth stating: a
float on a dark page separates by being lighter, and a float on a white
page cannot be. On the light base `raised` *is* `surface`, and the float
separates by `border_strong` instead. A design that only ever inverted
would get this wrong.

### 3. Three sources, and the default follows the OS

```rust
pub enum ThemeSource {
    Derived,                  // the OS's appearance, the OS's accent
    DerivedWithAccent(Color), // the OS's appearance, the app's colour
    Pinned(Theme),            // exactly this, following nothing
}
```

Three, because every combination of "who picks the base" and "who picks
the accent" is one of them, and the middle one is not a nicety: an app
with a brand colour still wants to go light when its user does. It is the
case a two-state design (follow / don't) forces you to hand-roll.

Resolved at the start of every frame, which is why it is recomputed rather
than invalidated — two dozen float operations once a frame, against a
cache that would need poking from every writer of `env.system`. Frame
stability is the point: every widget in one frame paints from one palette,
whatever the driver does to `env` while the view runs.

### 4. An unknown appearance takes the **dark** base

Not a guess about the user. It is the honest answer to "what did kui paint
before it could ask", and it is what lets this ship on by default rather
than behind a flag: **a host that reports nothing sees no change at all.**
Every value in `Theme::dark()` is one this crate already painted — the
muted grey from sixteen files, the field background from the stock input,
ADR 0002's ring, the stock button's own hand-picked trio. A test asserts
that, colour by colour (`theme.rs`,
`an_unknown_appearance_is_what_kui_always_painted`).

### 5. A text run that names no colour resolves to `theme.fg`

`TextStyle::color` becomes `Option<Color>`, and `None` means what the
schema row has claimed since it was written: the default foreground.

Resolved at the three doors text enters the tree by — `text_node`,
`rich_text_node`, `text_edit` — so the shaping cache, the display list,
the access tree and all four bindings see a real colour and never the
question mark. `EditOptions::accent` gets the same treatment against
`theme.selection`, so a selection over a label and one over a field are
the same tint, which is what `select::TINT`'s doc always asked for.

The cost is a public type change in Rust and a handful of `.color` reads
that become `.color_or_default()`. The alternative — a sentinel `Color`
whose alpha is impossible — was rejected: it puts a value into the type
that every consumer must know not to paint, and there are four bindings
and a cache between the declaration and the quad.

### 6. The core's own chrome reads the theme, revisiting ADR 0002

The focus ring, the scrollbar thumb and the selection tint now come from
`theme.focus_ring`, `theme.scrollbar` / `scrollbar_active` and
`theme.selection`.

ADR 0002's ruling was that the ring's colour is "not a prop and not a
theme value", and the *prop* half stands: no node styles the default ring,
the geometry is still fixed, and an app that wants its own uses `focusBg`.
What changes is the second half, and only because the premise did. When
there was one background there was one readable ring. With two bases there
are two, and the constant is unreadable on one of them.

### 6a. The window's ground is a theme role too

Added while migrating the examples. `kui-wgpu` clears the surface to a
constant `(0.06, 0.065, 0.08)` and nothing ever set it, so a view that
paints no root background — which is most of them, since a root that fills
and centres has no reason to — showed the renderer's near-black whatever
the theme said. It is the one surface a `bg` prop cannot reach: it is
*behind* the tree.

The runner now writes `theme.bg` into `renderer.clear_color` each frame,
straight through, since the renderer asks for a non-sRGB surface and a
clear component lands as the byte it is. Node and C inherit it for free —
both drive `kui::App` through the same `PumpRunner`.

### 7. `accent` stays a question, not a colour

`NodeSpec::accent` meant "the OS accent where there is one, the `bg` I
declared where there is not" — a per-node fallback four bindings document.
The theme always *has* an accent (kui's blue when nobody chose one), so
reading it unconditionally would have quietly deleted that fallback.

`Core::has_accent()` keeps it: true when the OS reported an accent or the
app set or pinned one, false when the palette is falling back. The row
repaints only when it is true. So the meaning widened — an app's brand
colour now drives `accent` too, which it never could — and no existing
call site changed behaviour.

### 8. A selected menu row is a wash, not a fill

`widgets::context_menu` filled the hovered row with the raw accent. A
filled row needs its label to flip to `on_accent` in the same frame the
fill lands, and `hover_bg` is resolved by the core *after* the view has
already chosen that label — so on the light base the row would spend a
frame as dark-on-blue.

`accent_soft` is the fix and is a role in its own right: the accent at
0.30 alpha on the dark base, 0.16 on the light, over `raised`. `fg` stays
readable over it on both, and the highlight stays declarative — no frame
of lag, no view guessing ahead of the core.

### 9. Contrast is a test, not an opinion

`every_text_role_is_readable_on_every_surface` walks both bases and
asserts WCAG AA: 4.5:1 for `fg` and `muted` on all four surfaces, 4.5:1
for the button label on the accent and for each status colour on a
surface, 3:1 for `faint` and for the focus ring.

It failed on the first run, against the *existing* palette: `faint`
`#505566` is 2.43:1 on `bg` and 2.00:1 on `raised`, and it is used for
gutter numbers and inactive tabs — text a person is expected to read.
It moved to `#6e758a` (3.24:1 at its worst). That is the audit paying for
itself: the number found a real defect in a value four files had copied.

### 10. One table, four bindings, and Lua reads without writing

`schema::THEME_ROLES` is to the palette what `ENV_FIELDS` is to the
environment: one row per role, carrying the doc, Node's camelCase spelling
and a `get` function. Every binding's reading is *generated* from it —
Lua's `env.theme`, Node's `ctx.theme()`, C's `KuiTheme`, the Theme table
in `docs/props.md` — rather than restated beside it, so adding a token is
one row and nothing else. `theme_roles_restate_the_theme_exactly`
destructures `Theme` exhaustively, so a field added without a row does not
compile.

Reading is every binding's business. **Writing is a host's**: Rust
(`Core::set_theme` / `set_accent` / `derive_theme`), Node (`ctx.setTheme` /
`setAccent`) and C (`kui_theme_set` / `kui_theme_set_accent`) are hosts and
set it; a Lua script is a guest in someone else's frame and reads it only,
the same way it does not own the window.

The C side needs no ABI bump: `KuiTheme` is a new `[out]` struct with
`size` leading it and three new functions, and `abi.rs`'s own rule is that
new functions and new types do not bump the version.

## Considered options

**A cascade — colours inherited down the tree, CSS-style.** The obvious
shape, and wrong for this core. The tree is a flat per-frame array with no
inheritance of anything; adding one for colour means a resolve pass, a
per-node "inherited paint" slot, and a second way for a node to get a
background. A theme read off `ui` costs nothing at all and composes with
the existing `bg` row instead of competing with it.

**A registry of arbitrary named tokens** (`theme.get("brand.primary")`).
Maximum flexibility, no contract: the stock widgets could not read it
without agreeing on names anyway, a typo is a missing colour at runtime,
and no binding could be generated from it. A closed struct is the thing
the four bindings and the corpus can be pinned to.

**Invert the dark base to get the light one.** Cheap and wrong twice: a
float that is lighter than the page inverts into one that is darker than a
white page, when it should be the same colour with a stronger edge; and
the accent does not invert at all — a blue button is a blue button, and
only its ring and its tint need to move.

**Let the core repaint on a dark appearance.** Rejected for the reason
`env.rs` already gives: the core does not know which of an app's colours
is a background. It knows which of the *stock widgets'* colours are, which
is exactly the scope taken here.

**Leave it to apps.** The status quo, and the evidence against it is the
audit: five applications' worth of examples, four independent token sets,
twenty-eight near-duplicate values, one contrast failure nobody caught,
and a stock menu that was dark on a light desktop.

## Consequences

- A host that reports nothing paints exactly what it painted before, byte
  for byte. A host that reports a light appearance gets a light app.
- `TextStyle::color` is `Option<Color>` — a source break for Rust code
  that reads the field. `.color_or_default()` is the one-word fix.
- `widgets::button_spec()` is unchanged. The stock button was already
  theme-correct: an accent-filled control with a luminance-picked label
  reads on both bases. What was wrong was everything *around* it.
  *(Revised 2026-09-14, backlog AR41: it takes the theme now —
  `button_spec(&theme, &metrics)` — and its trio is the theme's `accent`
  / `accent_hover` / `accent_pressed`, so a brand colour set on the
  palette recolours a plain button along with the ring and the menu
  rows. On a silent host the trio is byte-for-byte the one it always
  had, which is the same "no change" this ADR promised; on a host that
  reports an OS accent, a stock button is that colour now, where before
  only one declaring `accent` was.)*
- The corpus report changes where the widget colours consolidated (the
  menu's surface is now `raised`, its hovered row a wash). One
  regeneration of `target/conformance.txt` covers all four bindings.
- `examples/rust/theme.rs` is the reference page: every role as a swatch
  over every stock widget, with the base and the accent switchable live.
  It is where the next role gets checked before it is added.
- **Every example in the repo is on the theme** — all sixteen Rust ones,
  the two Rust hosts that load a Lua and a C panel, the Lua panel itself,
  the four JSX ones, and C on both sides of the FFI. Fragment parameters
  went with them, which is where the idea reads best: the WGSL says *what*
  a gradient, a ring or a shimmer is, and the palette says which colours
  it is made of, so `examples/rust/fragments.rs` follows the OS without a
  line of any shader changing. The
  three `struct Pal`s survive as *renames*: `pal.bg2` reads better than
  `theme.sunken` at twenty call sites, and a `From<Theme>` rebuilt each
  frame is what makes them follow the OS. Two honest remainders keep app
  colours: `syntax_view`'s six syntax hues, which are authored the way ADR
  0017 says a span's colour is — and which therefore ship in two sets,
  darkened for the light base and each checked past 4.5:1 — and two
  fixtures whose literals are the point: the C self-test, whose checks
  assert exact colours, and `examples/lua/bench.rs`, whose two halves have
  to declare the same tree for the comparison to mean anything.

### Not done here

- **Metrics are still constants.** `BUTTON_TEXT`, `MENU_TEXT`,
  `MENU_WIDTH`, `TITLEBAR_H`, the paddings and the radii are as
  hard-coded as the colours were. A `Theme` with a `Metrics` beside it is
  the obvious next step and deliberately not this step: colour is where
  the duplication and the accessibility failure both were.
- **`on_context_menu` does not bubble.** Found while building the
  reference page: a secondary press asks the topmost hit region and stops,
  so a full-window sink above a root that declared the row swallows it.
  Keys bubble (ADR 0011) and this does not. Unrelated to themes, real,
  and filed rather than fixed here.
