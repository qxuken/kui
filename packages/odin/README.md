# kui for Odin

kui's C API from Odin, in two packages, nearly all of it generated.

| | |
| --- | --- |
| `kui/` | **package kui**, the binding an app imports: `generated.odin` (from kui.h and the prop schema) and the hand-written files beside it |
| `kui/c/` | **package kui_c**, kui.h mirrored: every declaration, prefix dropped (`kui_open` → `c.open`, `KuiSpec` → `c.Spec`, `KUI_GROW` → `c.GROW`) |
| `gen/` | the generator, in Odin |

```odin
import kui "kui:kui"                    // -collection:kui=<this repo>/packages/odin

State :: struct { count: int }
Inc   :: struct {}                       // {kind = "inc"}, as #[derive(Message)] spells it
Dec   :: struct {}
Msg   :: union { Inc, Dec }

view :: proc(s: ^State, ui: ^kui.Ui) {
	t := kui.theme(ui)
	kui.root(ui, {width = kui.GROW, height = kui.GROW, main_align = .Center, cross_align = .Center, bg = t.bg})
	if kui.column(ui, {gap = 12, pad = kui.pad(24), bg = t.surface, radius = 12}) {
		kui.text(ui, fmt.tprint(s.count), {size = 48})
		if kui.row(ui, {gap = 8}) {
			kui.button(ui, "-1", Dec{})
			kui.button(ui, "+1", Inc{})
		}
	}                                        // closed here, by @(deferred_in)
}

on_event :: proc(s: ^State, ev: kui.Event) {
	switch _ in kui.message(ev, Msg) {
	case Inc: s.count += 1
	case Dec: s.count -= 1
	}
}

main :: proc() {
	state: State
	kui.run("Counter", &state, view, on_event)
}
```

The whole counter, with its context menu and the `--headless` self-check
every binding's counter runs, is
[`examples/odin/apps/counter.odin`](../../examples/odin/apps/counter.odin).

## Running it

```sh
nu scripts/odin.nu run counter              # builds libkui_ffi, opens the window
nu scripts/odin.nu test                     # every example's --headless self-check
nu scripts/odin.nu check                    # regenerate, then vet everything
nu scripts/odin.nu gen                      # regenerate from kui.h and the schema
nu scripts/odin.nu gen --check              # fail if a rerun would change anything
nu scripts/odin.nu slots                    # the extension contract across Odin, C and Rust
```

`odin` comes from PATH, or set `ODIN`. The examples link `kui_ffi` as a
system library with `-L` and an rpath into `target/<profile>`; outside this
repository, pass your own `-extra-linker-flags`, or `-define:KUI_LIB=<name>`.

## Extensions

An Odin program can load plugins, and an Odin plugin loads into any host:
Rust, C, Node, Lua or Odin. A host loads one with `ctx_add_extension`
on a context of its own and declares where it draws with `slot`. A
plugin is a shared library exporting the seven `kui_ext_*` symbols of
kui.h's contract, each a line over `extension.odin`'s helpers:

```odin
@(export) kui_ext_abi      :: proc "c" () -> u32 { return kui.ABI_VERSION }
@(export) kui_ext_view     :: proc "c" (user: rawptr, ui: ^kui.Ui) { kui.ext_view(user, ui, view) }
@(export) kui_ext_on_event :: proc "c" (user: rawptr, ev: ^kui.Ext_Event) { kui.ext_on_event(user, ev, on_event) }
// ...and name, slots, init, free: examples/odin/features/slots/panel.odin
```

`view` and `on_event` have `run`'s shapes. `slot_params` reads what the
host passed, and `reply(ev, msg)` answers it. Build with
`-build-mode:shared -define:KUI_PLUGIN=true`, plus
`-extra-linker-flags:"-Wl,-undefined,dynamic_lookup"` on macOS. The plugin
then links no kui at all, and every `kui_*` resolves from whichever host
loads it, as `panel.c` does.

`nu scripts/odin.nu slots` runs the contract across languages, each pair's
`--headless` drive: a click on the panel, routed to it and not the host,
and its reply back with its origin and slot. The pairs are the Odin host
with the Odin and C panels, and the C and Rust hosts with the Odin panel.

## Where each part comes from

```
crates/kui-ffi/include/kui.h ──clang -ast-dump=json──┐
                                                       ├─ gen ─┬─ kui/c/kui_c.odin     the header, mirrored
kui_core::schema ──cargo run --example schema-dump──┘         ├─ layout.c ─ clang ─ kui/c/layout.odin
                                                               └─ kui/generated.odin   the typed layer
packages/odin/kui/*.odin (hand-written) ─ read by gen ─────────┘
```

**From the prop schema** (`kui_core::schema`, the table every binding is
generated from or checked against):

- `Spec` and `Text_Style`: one field per `PROPS` row and `CUSTOM`
  composite, documented by the row's own doc, with the procedures that
  lower them to `KuiSpec` / `KuiTextStyle`. A row that names a
  `KuiSpec` field lands on it by kind. The rest are in
  [`gen/policy.odin`](gen/policy.odin)'s `LOWERING`. A `KuiSpec` field
  no row writes stops the generator.
- The core's events: `events.odin` is hand-written, but the generator
  checks every `EVENTS` kind has its `X_Event` struct with every payload
  field the schema names.
- The verbs: every `DOORS` row with a C cell reaches Odin, documented
  with the row's doc and its Rust name.

**From kui.h**, read by clang:

- An enum or bit set per family of constants (`ENUMS` in the policy),
  member values taken from C and checked against the schema's name lists.
- A *door* for every C function: a proc named as in C without `kui_`,
  whose signature is lowered from the C one. Out-params come back as
  results (`#optional_ok` when it is a value and whether there was one).
  Pointer-and-count pairs are slices. Arrays the call fills come back as
  temp-allocated slices. Codes are enums, ids are distinct handles
  (`Image`, `Font`, `Sound`, `Fragment`, `Playback`), and a `KuiValue`
  is any Odin value. A struct parameter kui.h lets be NULL is a `Maybe`
  (`theme_set(ui)` goes back to deriving). A node opener closes at the
  end of the `if` it is called in.
- A mirror per C struct a door takes or returns. It has no `size` header,
  slices instead of pointer and count, enums and `bool`s instead of
  `uint32_t`, `Maybe` where the INIT macro is not the zero value
  (`Play.volume`), and `_to_c` / `_from_c` procedures. A struct that needs
  none of that is an alias of the C one.

**By hand**, where a nicer shape than the C one matters: the elements
(`view.odin`: `root`, `box` / `row` / `column` with
`@(deferred_in)` closing, `text`, the stock widgets taking their state,
`devtools_tab_open`, which closes only what it opened),
messages (`message.odin`), the run loop with typed state (`run.odin`),
the types with helpers and the twins that cross by a cast
(`types.odin`), the plugin side (`extension.odin`), and the one door whose
shape the header says only in prose (`doors.odin`).

The split is mechanical: **a C function a hand-written file calls is
hand-written; every other one gets a generated door**, unless `SKIP` gives
the reason it needs none (two, both "the hand-written door covers it").
So the binding covers kui.h whole by construction: 272 functions, 219
doors generated, 51 hand-written, 2 skipped at ABI 25. A function added to
the header is a door on the next `gen`, or an error naming it.

And at runtime:
[`examples/odin/tools/surface.odin`](../../examples/odin/tools/surface.odin)
is `examples/c/tools/surface.c` through package `kui`, plus a last section
for what the C binding leaves to its other programs (the drawing
elements, tokens, announcements, a dismissed popup, the OS appearance, the
extension doors' refusals). Between it and the counter, every generated
door and every hand-written one is called, and what comes back is
checked: `nu scripts/odin.nu test`.

## Why it cannot drift silently

- `kui/c/layout.odin` is printed by a C program the generator writes:
  the C compiler's size, alignment and offset for every struct field and
  its value for every constant (848 `#assert`s at ABI 25). `kui/c` does
  not compile against a header it no longer matches.
- The hand-written twins that cross by a cast (`Size`, `Span`, `Cell`,
  `Quad`, `Keyframe`, `Enter`, `Run_Config`, the `Key_Mods` bit_field) are
  pinned field by field, or bit by bit, to `kui/c`.
- Every policy row is checked when it is used. A row that matches
  nothing (a renamed field, a removed function, a typo) stops the
  generator as stale.
- `gen --check` fails when the checked-in output is not what a rerun
  writes, the way `book-examples.nu --check` does.
- `run` and `new_ui` refuse a library whose `kui_abi_version()` is not
  the generated `ABI_VERSION`.

## Not here yet

- Windows: `KuiStr` as Odin's `string` is checked on arm64 macOS only.
  The layout asserts hold everywhere, but how a two-word struct is passed
  by value is each platform's C ABI.
- CI, an ADR, and a book page.
