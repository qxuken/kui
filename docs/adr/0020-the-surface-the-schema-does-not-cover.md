---
status: accepted
date: 2026-09-10
---

# The surface the schema does not cover, and what pins it now

> **Accepted (2026-09-10), built the same day.** An audit of the C ABI and
> the three bindings against the core — every prototype, every enum, every
> door a host can call and every shape it can read back — found the
> table-driven surface exactly as sound as its tests say: the props, the
> composites, the elements, the env reading and the theme roles are one
> list each in `schema.rs`, four bindings interpret the list, and a
> corpus makes them agree on behaviour. It found the *rest* of the surface
> drifted in nine places, none of which any test could have caught,
> because the rest of the surface — the entry points, the plain-constant
> enums, the readback shapes, the verbs — was hand-mirrored with nothing
> pinning the mirrors to the original.
>
> This ADR writes down what drifted, closes it, and extends the
> pin-at-build-time discipline `mod abi_parity` already applies to struct
> layout to everything else C can say: prototypes, constants, and the
> header's own audit of who writes what. It also names three doors C did
> not have that Node did, one rule the windowed runner kept to itself that
> every driver needs, and two hand-written TypeScript unions that are now
> generated because one of them had already fallen behind.

## Context

### What was already pinned, and how well

The project's parity story has three layers, and the first two are in
good shape:

1. **The schema.** `PROPS`, `CUSTOM`, `ELEMENTS`, `EVENTS`, `RESOURCES`,
   `ENV_FIELDS`, `THEME_ROLES` and the enum lists (`ROLES`, `CURSORS`,
   `EASINGS`, …) live once in `kui-core::schema`. Node reads them at
   runtime and generates its prop types from them; Lua looks names up by
   their snake spelling; C, whose `KuiSpec` has to be a static layout,
   pins itself with `every_schema_prop_has_a_c_counterpart`. The env shape
   is checked key for key in Lua and Node and prototype for prototype in
   C. A prop added in one place fails in every other.
2. **The corpus.** `kui-core::conformance` has every `CUSTOM` and
   `ELEMENTS` row in a scene, and the four binding adapters reproduce
   every scene byte for byte in CI. Behaviour agrees because a test says
   it must.
3. **The layout.** `mod abi_parity` emits a `_Static_assert` per field —
   offset, size, `_Generic` type — for every `repr(C)` struct, and
   `KUI_ENUM` asserts for the enums that index a list the core owns
   (`Role::ALL`, `CURSORS`, `QuadKind::ALL`). ADR 0006 added the version
   and the size-led `[out]` handshake. The generated translation unit is
   compiled against the header on every C build.

Everything the audit found was outside those three. Checked against the
code, in the order they were found:

### The nine drifts

**1. C had no way to read the open menu.** `kui_set_native_menus` is
documented as "read what is open, show it, and report back", and there was
nothing to read it with: Node has `menu()`, Rust has `Core::menu()`, C had
only the menu *bar*'s readers. A C host that took the documented path
found nothing to read, and the core waiting for an answer it could not
give.

**2. One `MenuItem`, five readings.** The drawn menu, the drawn bar and
C's `kui_menu_bar_item` all read a row as `text()` / `accel_text()` (the
role's `⌘C` where the row declared none) / `checked`. Node's `menu()` read
the *raw* `accel` (so a `copy` row reported no shortcut while the bar's
reported `⌘C`) and omitted `checked` entirely — with a comment in
`index.d.ts` asserting that a context menu's rows "have never had one",
while `openMenu` accepted and stored it. The macOS runner's native
context menu ignored both the declared accelerator and the check state
and used its own four-entry role table, while the native bar beside it
parsed `accel_text()` and set the state. And the core's access tree
dropped a checked menu row's `checked`, so a reader heard "Wrap" where a
sighted user saw "✓ Wrap" — the widget declared the fact and the
derivation only kept it for checkbox / radio / switch.

**3. A C host could not read a boolean, a float, or a list off an
event.** `kui_value_as_int` and `kui_value_as_str` were the only readers.
A key event's `shift` / `ctrl` / `alt` / `super` / `repeat` are bools:
unreadable. A drag's `dx`, a layout's rect, a resize's `scale` are floats:
truncated — `examples/c/counter.c` carries a comment working around
exactly this. A preedit's `cursor` and an `access` request's `anchor` /
`focus` are lists: unindexable. Lua and Node read all seven shapes.

**4. `terminal` was not an `AccessRole`.** Backlog C20 appended it to
`Role::ALL`; the generated `role` *prop* union got it; the hand-written
`AccessRole` union in `index.d.ts` — what `accessTree()` types its nodes
with — did not, so a `cells` grid's node typed as a role it could not be.
The menu role union was spelled by hand three times in two files.

**5. `AccessNode.live` was emitted and never declared.** The addon wrote
`live` on every node of the access tree; the TypeScript interface did not
have the field.

**6. Lua spelled two menu roles the way nothing else does.** `open_menu`
took `"select_all"` and `"look_up"`; the `menu` event reported
`"selectAll"` and `"lookUp"` back, and every other enum *value* in Lua is
the wire spelling (`alternateReverse`, `notAllowed`) — only the *keys* are
snake. A script matching the role it declared against the role it heard
matched nothing.

**7. Focus loss releasing held keys was a runner rule, not a core rule.**
The windowed runner, on the OS reporting a window lost the keyboard,
wrote `env.focused = false` and called `release_held_keys()`, with a
comment explaining why (the OS stops delivering key events to that
window, so the release of anything held over a Cmd-Tab never arrives). A
C host was told to call `kui_release_held_keys` itself; a Node host driving
its own window had no door for it at all, and `setEnv({focused: false})`
did nothing but write the flag. Per the schema's own rule — *per-binding
extraction, never per-binding decisions* — this decision was in the wrong
layer.

**8. C had no listing of font families.** `kui_font_add_system(name)` takes
a family name; Node's `systemFontFamilies()` says which names there are;
C had nothing to ask.

**9. The header's audit of who writes what was stale.** ADR 0006 decision
7 put the `[in]` / `[out]` / `[out[]]` / `[lib]` lists in the header on
purpose, "because the person who needs it is the one adding a field". Four
`[out]` structs (`KuiTheme`, `KuiMenuAction`, `KuiTextHit`,
`KuiCaretRect`) had gained the size handshake without joining the list;
`KuiAnnouncement`, `KuiClip`, `KuiFragmentDraw`, `KuiCell`, `KuiMenuItem`
and `KuiMenu` were in no paragraph. And `KuiTheme` implemented `OutParam`
without `abi_parity` emitting its `KUI_OUT_STRUCT` row, so the "size is
the first field" assert was missing for the one struct a theme-aware C
host reads every frame.

### What none of that was

None of it was a value mismatch. Every constant in the header agreed with
the number Rust read (a script checked all 221 enum members against the
sixty named Rust constants and the literals at the sites that had none),
and every prototype agreed with its Rust signature. The audit found the
*next* drift's mechanism, not a present crash: the `KUI_ACCESS_*` action
bits, the `KUI_AUDIO_*` kinds, the `KUI_KEY_*` codes, the `KUI_MOUSE_*`
buttons and nine other families were spelled once in the header and once
more as bare literals (`o.kind = 3`, `flags & 8`, `12 => EditKey::SelectAll`)
at the Rust site that read them, with nothing between the two. A prototype
that gained an argument in Rust and not in the header — ABI 12's case —
was caught only by a C host noticing garbage. That is the same failure
mode `abi_parity` was written to end for struct fields, one row up.

## Decisions

1. **Every entry point is pinned to the header, both ways.** `abi_parity`
   now carries one `abi_fn!` row per `extern "C" fn`. The row is a `let`
   that coerces the real function to exactly the signature it spells —
   an argument added, removed or retyped in Rust stops the test module
   compiling — and it emits a *second declaration* of the function into
   the generated translation unit, after `kui.h`'s own. C requires two
   declarations of one function to agree, so a header prototype that
   drifted (a `const` dropped, an argument the Rust side gained, a return
   type changed) fails the C build as `conflicting types` instead of
   reading a register the caller never filled. `every_entry_point_is_pinned`
   holds the row set equal to the header's prototypes and to the sources'
   exports, so a function added to any one of the three without the other
   two fails by name. 173 prototypes, checked by mutation: a dropped
   `const` and an appended argument each fail where they should.

2. **Every plain constant the entry points read has a Rust name, and the
   name is pinned.** The literals at the read sites became the constants
   the header spells (`KUI_AUDIO_STOP`, `KUI_KMOD_CTRL`, `KUI_SPAN_BOLD`,
   `KUI_MENU_ITEM_CHECKED`, …), `abi_consts!` emits a `KUI_ENUM` for each,
   and the lists the core owns got the `abi_enum!` treatment `Role::ALL`
   already had: `schema::LIVE`, `CellCursor::NAMES`, the new
   `MenuRole::ALL`, `AccessAction::ALL` as its bits, `MouseButton::code`,
   the cell flag bits, and `KUI_EDIT_KEYS` — a table in `types.rs` rather
   than a cast, because the header's order is not `EditKey`'s declaration
   order and the table is the one place that says so. 207 members, up
   from 89.

3. **The header's audit is a test.** `the_headers_audit_names_every_struct_once`
   parses the "Who writes what" block and holds it to the rows: every
   struct with a layout is in exactly one paragraph, and the `[out]`
   paragraph names exactly the structs that implement `OutParam`. The
   lists were brought up to date and `KuiTheme` got its `KUI_OUT_STRUCT`
   row. ADR 0006's decision 7 stands, and now holds.

4. **The open menu reads back from C the way the bar does.**
   `kui_menu_item_count(ctx, &target, &x, &y)` and
   `kui_menu_item(ctx, i, &label, &accel, &role, &flags)`, spelled
   exactly as `kui_menu_bar_menu_count` / `kui_menu_bar_item` are and
   implemented by the same `write_row`, so the bar and the context menu
   cannot read one row two ways. Zero rows is "no menu", unambiguously,
   because `kui_open_menu` refuses to open an empty one.

5. **One `MenuItem` is one reading, everywhere it is read.** Node's
   `menu()` and `menuBar()` share `menu_item_json`: `text()`,
   `accel_text()`, `enabled`, `checked`, all present. `OpenMenuItem.checked`
   is a required boolean and its comment says what a checked context-menu
   row is. The macOS runner's context menu and bar share `menu_row`, so a
   declared accelerator and a check state show in both; the four-entry
   role table it kept for itself is gone, `MenuRole::default_accel` being
   the same fact in the core. The access tree reports `checked` on a
   `menuItem` that declared it — and only then, the way a list row reports
   `selected` only when it is — so the reader hears what the gutter draws.

6. **A payload's seven shapes each have a C reader.** `kui_value_as_bool`,
   `kui_value_as_float` (an integer widens; the reader for anything in
   pixels), `kui_value_is_null` (true for a NULL pointer too, so a missing
   key reads as an explicit null), `kui_value_len`, `kui_value_at` for a
   list, `kui_value_entry` for walking a map whose keys the host does not
   know, and `kui_value_list` / `kui_value_list_push` for building one.
   `kui_value_as_int` keeps truncating a float, and its comment now says
   which reader to use instead. New functions, no bump (ADR 0006 rule 2).

7. **A window losing the keyboard lets go of held keys in the core.**
   `Core::set_focused(bool)` writes `env.focused` and, on the way to
   `false`, calls `release_held_keys`. The runner, `kui_env_set` and
   Node's `setEnv` all call it, so a headless Node test can hand a
   Cmd-Tab to a context and see the `up`s the window would have produced,
   and a C host owes no second call. `kui_release_held_keys` stays for a
   host with a reason of its own.

8. **`kui_font_families(ctx, out, cap)`** lists what `kui_font_add_system`
   can take, on the `kui_access_tree` pattern: filled up to `cap`, total
   returned, strings borrowed until the next call.

9. **The three name lists in `index.d.ts` are generated.** `protocol()`
   exports `accessRoles` (`Role::ALL`), `accessActions`
   (`AccessAction::ALL`) and `menuRoles` (`MenuRole::ALL`); `npm run gen`
   writes `AccessRole`, `AccessAction` and `MenuItemRole` from them in
   marked regions, the way it already wrote `WarningCode`, and CI's diff
   of the generated files is what catches the next `terminal`.
   `MenuMsg.role`, `MenuItemInput.role` and `OpenMenuItem.role` all name
   `MenuItemRole` now; `AccessNode.live` is declared.

10. **Lua takes a menu role by its wire name.** `MenuRole::from_name` is
    the inverse of `MenuRole::name` in the core, and Node's parser and
    Lua's both use it — so no binding can accept a spelling the event
    will not report back. Lua keeps `"select_all"` / `"look_up"` as
    aliases, the way `direction` is one for `repeat`: a script that
    learned the old spelling keeps working, and a new one reads the
    spelling it will hear.

## Considered options

**Generate the header.** cbindgen would end the layout and prototype
drift at the root. Rejected, as it was when `abi_parity` was first
written: the header carries prose the generator would lose — the ABI
log, the who-writes-what audit, the paragraph beside each function that
says what it means — and that prose is the C documentation. The decision
here keeps the hand-written header and makes the C compiler check it,
which costs one row per function and nothing at runtime.

**Pin the verbs across bindings with a table, the way `ENV_FIELDS` pins
the env.** A `DOORS` list in `schema.rs` — one row per verb with its
Rust, C, Node and Lua spellings — would have caught the menu reader and
the font listing by name. Declined for now: the verbs are not one surface.
Lua is a guest with a view-time env and no resource or input doors by
design; Node's `Ctx` is a driver and its `KuiWindow` deliberately refuses
input injection; C is both a driver and (through `kui_ext_*`) a guest
with the driver's full context. A table would need three "n/a" columns
and a reason in each, which is documentation, not a pin. The audit that
produced this ADR *is* that table, once, in prose; if the verb surface
drifts again the table is the next step, and this section is where it
would be argued for.

**Report a menu row's `checked` for every `menuItem`, `Some(false)` when
unset.** Rejected: AccessKit's own guidance, already applied to list rows
and links, is that "not selected" on every row of a list that is not a
selection is noise. A menu of commands is not a set of settings. A row
that declares `checked` is a setting and reports it; the rest say nothing.

**Leave `kui_release_held_keys` as the host's job, documented.** It was
documented, in the header, and a Node host still had no door. The rule
belongs where every driver reaches it, and the schema's own module doc
says why: extraction is per binding, decisions are not.

**Make `kui_value_as_int` refuse a float.** A truncating reader is the C
convention and `examples/c/counter.c` leans on it (the corpus steps are
integers and print exactly). Adding the float reader beside it changes
nothing that worked.

## Consequences

- `KUI_ABI_VERSION` stays at 13. Every C change here is a new function or
  a constant that already had that value; no struct the library writes
  moved. `examples/c/build.sh` now reports prototypes beside fields and
  enum members.
- A C host that called `kui_release_held_keys` after `kui_env_set(ctx,
  hz, false)` now releases once, not twice: the second call finds nothing
  held and does nothing.
- `Ctx.menu()` rows gained `checked` and their `accel` may now be the
  role's default where it was `null`. A host that drew its own menu from
  `menu()` shows `⌘C` beside Copy, which is what the drawn menu and the
  bar already showed.
- `OpenMenuItem.checked` went from optional to required, a source change
  for TypeScript that built one by hand. Nothing in the repo did.
- The access tree changes for a checked menu row: `checked: true` where
  it was `null`. The corpus's `menu_bar` scene carries one, so its report
  moves by one column on one row (`node 3 … menuItem 0 0 1 …` where the
  `1` was `-`); the checked-in `Expect` omits flags and is unchanged, and
  the four bindings produce the new row from the same core, so the
  cross-binding comparison CI runs still passes. `target/conformance.txt`
  is regenerated every run and never checked in.
- `MenuRole::ALL` and `MenuRole::from_name` are public in the core;
  `AccessAction::ALL` and `Role::ALL` already were. A role appended to
  `MenuRole::ALL` without a header name fails the C build, without a
  `KUI_MENU_*` constant in `types.rs` fails `abi_consts!`, and without
  `npm run gen` fails CI's diff.
- The generated translation unit is 173 prototypes longer and compiles in
  the same front-end pass; nothing links.

### Not done here

- **Verb parity is documented, not pinned** (see the declined table
  above). The one place a Node reader still has no C twin is `stats()` /
  `frameStats()`, which C reads off `KuiDrawData` and the latency HUD
  respectively; and C's `kui_fragment_source` and `kui_set_subpixel_text`
  have no Node twin because a Node host never paints. Both are by design
  and both are one sentence each in the header's and `index.d.ts`'s docs
  rather than a row anywhere.
- **The C payload readers do not coerce.** `as_bool` on an integer is
  false, as it is in `Value::as_bool`. A host that wants JavaScript's
  truthiness writes it.
- **The Windows runner has no native context menu**, so decision 5's
  macOS half has no twin to keep in step there yet.
- **C's runner took a title and nothing else**, and `kui_run_with` let the
  launcher build a fresh session — a font or image registered on the
  context before the call never reached the window. Neither was named
  here at the time; both closed on 2026-09-14 (backlog AR27, ABI 16):
  `kui_run_with` takes a `KuiRunConfig` and opens the window on the
  context's own core (`Launcher::core`).

## Amendment: the rows are read off the functions (2026-09-11)

Decision 1's row was typed by hand — 178 of them, one line of restatement
per entry point beside the `extern "C" fn` and the header's prototype. The
type pin meant a row could never be wrong, only missing, and
`every_entry_point_is_pinned` caught missing; the review of 2026-09-11
(backlog AR2) named the cost and the middle path this ADR had not weighed:
not generating the *header* (the prose would go, as above) but the *row*,
from the function itself, the way P5 generates `index.d.ts`'s addon half.

Built without a proc-macro. `build.rs` already read the sources for the
`kui_*` names Windows' export list needs; it reads one level deeper now —
each function's parameter and return types, as the source spells them —
and writes `OUT_DIR/abi_rows.rs`, one `abi_fn!` row each, which
`abi_parity` includes. A file whose module is `#[cfg]`-gated in `lib.rs`
(`run.rs` behind `runner`) has its rows gated the same way. The pin is
unchanged: the row still coerces the function to the signature it spells,
so a misread by the build script is a compile error in the test build and
never a wrong prototype in the generated C; and the name check still holds
the row set to the header's prototypes, which is now the only way a row
can be missing — a function the sources export is a row by construction.
What the row does not carry is the header's prose, which is why the header
stays hand-written.
