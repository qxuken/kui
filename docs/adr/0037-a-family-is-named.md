---
status: accepted
date: 2026-09-27
---

# A family is named where the text declares it

> **Accepted and built 2026-09-27**, the day it was proposed (backlog
> DX17, from the DX sweep of kawoosh's views). The user chose frame v18
> for decision 4. What the building settled: the parser resolves the
> name. `NameRefs::family`, over a token lookup that now carries the
> session, registers it through `Session::register_family`, the body
> `add_system_font` always had, moved where a parse holding the core's
> lookup can reach it. So Lua and Node resolve it in the one step they
> already resolve a `$token` in. A miss is remembered beside the token
> misses and raised by the same code. The schema gained `Kind::Family`
> and `Parsed::Family`. C keeps the index and the handle door (decision
> 5), and its parity sample is the stock family at index 1. `font` and
> `family` write the same field, so the last one declared wins, and the
> row's doc says so rather than "the handle wins".

`family` takes `sans`, `serif` or `mono`. Any other face is drawn with a
handle: the host registers the family by name (`Core::add_system_font`,
`addSystemFont`, `kui_font_add_system`) and hands the `FontId` down, and
the text names it as `font`. A Lua view cannot register anything (ADR
0014: registering is the host's), so a script that shows an installed
face needs its host to register it and pass it in. kawoosh built that
door, `kawoosh.fonts.face(NAME)`. Its first cut registered a family
when a card first asked for it, at the next frame, so every card the
fonts pane scrolled to drew one frame in kui's mono and then in its own
face. The fix it shipped registers all 613 installed families at once,
10 ms once in a debug build (`kawoosh/src/fonts.rs:7-12`,
`docs/design/fonts.md:48-60`).

## Context

- **Registering a family by name loads nothing.** The font database
  scans every installed face's metadata when it starts, which is how
  `system_fonts` (F97) lists them without opening a file. So
  `add_system_font` is a query against that scan and an idempotent
  registry entry. The face's bytes are memory-mapped and shaped only
  when a text first uses it, as for any face. Resolving a name in the
  frame that declares it costs a lookup. The frame of mono in kawoosh
  came from its host deferring the registration a frame, not from kui.
  (Since DX24 the session's first registration of any font also maps
  the installed files once, ~30 ms on a Mac, so the first frame to name
  a family pays that; noted by the alpha.22 regression pass.)
- **`TextStyle` is `Copy`,** and views rely on that everywhere (a
  closure capturing a style, `style` passed by value). A name cannot
  live in it. A handle can, which is why `FontFamily::Custom(FontId)`
  exists.
- **Every binding lowers a style where the core is at hand.** Lua's
  parser runs inside `view` with the `Ui`. Node's frame is lowered by
  the addon (`binary.rs`), with the window's `Core`. C builds a
  `KuiTextStyle` itself and has `kui_font_add_system`. Rust has the
  `Ui`.
- **The weights follow a family, not a handle.** RG59 reweighs text
  already shaped when a face of its family is loaded, and F100
  synthesizes bold for a family with none. Both key on the family. A
  named family resolves to the handle `add_system_font` returns, so
  both keep working.

## Decisions

1. **`family` takes an installed family's name.** `family = "Berkeley
   Mono"` in Lua, `family="Berkeley Mono"` in JSX. The stock names
   `sans`, `serif` and `mono`, lowercase, stay the stock families. Any
   other string is a family name, matched as `add_system_font` matches
   it: installed, or loaded with `load_fonts_dir` / `load_font_file`.
2. **The name is resolved where the style is lowered, to the handle.**
   The binding's lowering calls `Core::add_system_font(name)` and sets
   `FontFamily::Custom(id)`, so the text shapes in its face in the frame
   that declares it. `TextStyle` stays `Copy`, and nothing after
   lowering knows a name was involved. The registry's answer is
   idempotent. A name lowered every frame costs a lookup, and a
   name→handle map in the session makes it constant if a profile asks.
3. **A name nothing matches is a warning, and sans.** `unknown-family`,
   keyed by the name, so a typo in a view drawn every frame is one line.
   The text shapes as sans, as a stale handle does today.
4. **Node carries the name on the wire.** The frame's `family` slot is
   an index into `schema::FAMILIES`. A name needs the string table the
   frame already has for text. So `family` becomes an index for the
   stock three and a string for anything else, and the frame is v18.
   The encoder and addon ship together, so this breaks only a stale
   prebuilt addon, which refuses the stream by its version. The other
   route is below, with why it is second.
5. **Rust and C gain nothing new but a pass-through.** Both already hold
   the door. `Ui::system_font(name) -> Option<FontId>` is added so a
   Rust view needs no `core()` to reach it. C keeps
   `kui_font_add_system` and `KuiTextStyle.font`.
6. **The prop's type says it.** The schema row's kind becomes a family
   kind, the three stock names or a string. Lua's meta types it
   `"sans"|"serif"|"mono"|string`, and TS `'sans' | 'serif' | 'mono' |
   (string & {})`, so an editor still offers the three.

## Considered options

- **Leave it to the host** (today). It works for Rust, C and Node, which
  can call the door. For Lua it means every host that shows a face
  builds kawoosh's `fonts.face` and learns about its flicker.
- **A Lua `env.add_system_font` door.** It would let a script register.
  But registering is the host's (ADR 0014), and a handle is a thing to
  keep and pass around. A family by name has nothing to own or free,
  like `family = "mono"`, so it can be a declaration, not a verb.
- **Node resolves in JS**, with `ctx.addSystemFont(name)` from the
  encoder, cached per name, emitting the `font` handle the wire already
  has. No frame version, but the encoder would need a context. Today
  there is one encoder per module, built from `protocol()` alone
  (`index.js:56`) and shared by every window and every test. A string on
  the wire keeps it that way.
- **Load asynchronously and swap** (CSS's `font-display: swap`): draw a
  fallback, register off the frame, reshape when ready. It answers a
  cost that is not there (decision 2's context). The swap is the flicker
  kawoosh removed.
- **A root `fonts = { … }` preload.** A second place to name a family,
  for the same lookup the prop makes.

## Consequences

- A Lua view draws an installed face by naming it. kawoosh's
  `kawoosh.fonts.face` and its 613-family registration go, and the fonts
  pane's cards say `family = name`.
- The Node frame is v18: one slot's encoding, under What breaks.
  `schema::FAMILIES` and C's `KUI_FONT_*` are unchanged.
- One new warning, `unknown-family`, in `diag::WARNINGS`, the TS union
  and the Lua docs.
- A family's handles and weights come from one registry. A family named
  in a view and one registered by the host get the same handle,
  pinned by a test.
- Built with a core test that a named family shapes in its face in the
  frame that names it (no frame of sans), the warning for a name
  nothing matches, a Lua and a Node scene naming a family the corpus
  loads from its fixture font, and the reweigh (RG59) still following a
  named family's new face.
