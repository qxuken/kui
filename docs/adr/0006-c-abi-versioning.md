---
status: accepted
date: 2026-09-04
amended: 2026-09-14
---

# The C ABI has a version, and the structs the library writes carry their size

`crates/kui-ffi/include/kui.h` is hand-mirrored from the `repr(C)` structs
in `crates/kui-ffi/src/lib.rs`, and P6's `mod abi_parity` settles the two at
build time — `sizeof`, `_Alignof`, `offsetof`, member size and a `_Generic`
type check for all 209 fields. That covers a header that has drifted from
Rust. It does not cover **an old binary against a new library**, because a
C host does not build the library it links: it loads whatever
`libkui_ffi` the system hands it. ADR 0004 named this gap in decision 12
and left it open.

It is not a theoretical gap, because ADR 0004's own next step walks into
it. `KuiEvent` is **caller-allocated** — `kui_poll_event(ctx, &ev)` writes
into memory the host reserved — and the ADR appends a `window` field to it.
A host built against the old header reserves the old size; a newer library
writes past it. That is memory corruption, silent, in the hottest loop a C
host has.

We are shipping **both** guards, because they cover different failures:
`kui_abi_version()` (a number the host compares before its first call) and a
leading `uint32_t size` on the four **[out]** structs (a reservation the
library will not write past). And we are writing the audit down — which
struct is written by whom — in the header, because that distinction is what
decides whether appending to a struct is free or fatal, and it was
previously implicit.

## Context

- `KuiSpec` has already grown this way once: `tooltip` was appended for P1.
  It was safe, and the reason it was safe is the direction of travel —
  `KuiSpec` is host-allocated and *read* by the library, so a host that
  predates a field passes a shorter struct, the library reads no further
  than the host wrote, and the zeroed tail is the documented default.
- The out-param direction has no such property, and nothing in the header
  distinguished the two. A contributor reading it would find `KuiSpec`
  documented as "append-only: the layout is ABI" and reasonably conclude
  the same held for `KuiEvent`.
- There are more than two directions. Auditing every public `repr(C)`
  struct turned up four:
  - **[in]** host allocates, library reads — `KuiSpec`, `KuiSizing`,
    `KuiKeyframe`, `KuiEnter`, `KuiTextStyle`, `KuiSpan`, `KuiPlay`,
    `KuiAudio`. Appending is free.
  - **[out]** host allocates, library writes — `KuiEvent`, `KuiDrawData`,
    `KuiTextMetrics`, `KuiScrollGeometry`. Appending overruns the host's
    buffer.
  - **[out-array]** host allocates an array, library fills `cap` elements —
    `KuiAccessNode`, `KuiAccessRun`, `KuiWarning`, `KuiAudioCommand`.
    Appending overruns it *worse*: the library strides by its own `sizeof`,
    so the damage starts at element 1 and scales with `cap`.
  - **[lib]** library allocates, host reads — `KuiQuad`, through
    `KuiDrawData.quads`. The stride problem mirrored, but read-only: a
    mismatch misreads every quad after the first rather than corrupting
    anything.
- `KuiStr` is the one struct that crosses both ways (`kui_edit_text` and
  `kui_value_as_str` write one). It is a `(ptr, len)` pair and will never be
  anything else, so it is simply frozen and says so.
- `KuiEvent` also reaches `kui_run`'s `on_event` as a library-owned
  `const KuiEvent *`. A single struct behind a pointer is safe to append to
  — the host reads the prefix it knows, at unmoved offsets — so it is the
  `kui_poll_event` use that constrains the type, not this one.

## Decision

1. **`uint32_t kui_abi_version(void)`, and `KUI_ABI_VERSION` in the
   header.** The function is what the loaded library implements; the macro
   is what the host compiled against. A host compares them, for equality,
   before its first other call. `mod abi_parity` emits a `_Static_assert`
   that the header's macro matches the Rust constant, so a bump made in one
   place and not the other cannot make that check pass on a mismatched
   pair.
2. **The bump rule, written down.** It bumps when the layout of anything
   the library **writes or allocates** changes — an [out], [out-array] or
   [lib] struct, in any way, appends included. It does **not** bump when an
   [in] struct gains a field: old hosts survive that by construction, and
   pretending otherwise would make every `KuiSpec` prop a flag day. It does
   not bump for a new function either — a host that does not call one is
   unaffected, and one that does fails to *link*, loudly. *Amended
   2026-09-14 (backlog AR50): the [in] exemption is withdrawn — see the
   amendment at the end. The rule is now: any struct's layout, in any way,
   and any existing function's signature; never a new function.*
3. **Equality, not `>=`.** The mismatch worth catching is a *newer* library
   against an older host, which is the direction that corrupts memory. An
   older library against a newer host is also a mismatch, just a duller
   one. Neither is worth a compatibility matrix at this size.
4. **A leading `uint32_t size` on every [out] struct** — the four there
   were when this was written (`KuiEvent`, `KuiDrawData`,
   `KuiTextMetrics`, `KuiTextHit`) and every one added since, which is
   how the header has come to carry it on eleven. The host sets it to
   `sizeof` its own copy; the library writes no further than that, and
   writes back the number of bytes it filled. That is the stronger guard,
   because it turns a future append from a break into a compatible change:
   an un-recompiled host keeps getting the prefix it knows. `KUI_EVENT_INIT`
   and its siblings — one `KUI_*_INIT` per such struct — set it, so the
   handshake is one token at the declaration and invisible after that.
   A `size` below the ABI-1 layout — which is what a zeroed or never-set one
   looks like — is refused rather than guessed at: the call writes nothing
   and returns false. `kui_poll_event` checks *before* it pops, so a refused
   poll leaves the event queued rather than swallowing it.
5. **No `size` on the [out-array] and [lib] structs, and the reason is that
   it would not work.** The library strides by its own `sizeof`, so element
   1 lands past the host's element 1 whatever element 0's `size` says — and
   it lands there before any in-band handshake could be read. If one of
   these ever has to grow, it grows by gaining an explicit **stride
   argument**, which is a source break every host sees and fixes at one
   call, plus a version bump. Until then `KUI_ABI_VERSION` is the whole
   guard, and the header says so rather than implying a protection that
   is not there.
6. **`kui_draw_data` returns `bool`.** It was `void`, which left it the one
   [out] call with no way to report a refusal. A host that ignores the
   result still compiles, so this costs nothing.
7. **The audit lives in the header**, as a "Who writes what" block plus an
   `[in]` / `[out]` / `[out[]]` / `[lib]` tag on every struct — not in this
   ADR alone. The person who needs it is the one adding a field, and they
   are reading `kui.h`.
8. **Bumps are per change, not per release, and the version history is a
   log.** *Added 2026-09-06 (backlog S8), after the number went 1 → 6
   between two releases and the header's history started reading as a
   per-merge changelog.* The number moves when a change under rule 2 lands,
   which means published numbers can skip: nothing shipped ABI 1 through 5,
   and 0.1.0-alpha.6 is the first release to carry a number at all. That is
   the intended reading rather than an accident to tidy up, because under
   rule 3 the number is only ever compared for equality — an unpublished
   number is compared against nothing, so a gap costs a host nothing, while
   coalescing would leave two mutually incompatible builds *from git,
   between releases* reporting the same number, which is the silent short
   write this ADR exists to catch. The header says all of this where a C
   host reads it, so a reader knows an unpublished number is normal, and
   keeps a per-bump entry saying what each one changed — which is what a
   host crossing several at once actually needs.

## Considered options

- **The version check alone.** Rejected as insufficient, not as wrong: it
  is necessary (nothing else covers the arrays) but it makes every future
  append to an [out] struct a flag day for every C host, which is precisely
  the cost ADR 0004 flagged for `KuiEvent.window`. It ships as half of the
  answer rather than all of it.
- **The size field alone.** Rejected: it covers four structs out of
  thirteen. `KuiAccessNode`, `KuiQuad` and the two other arrays would have
  no guard at all, and those are the ones where a mismatch scales with the
  data rather than costing one struct.
- **A `size` on the array elements, read from element 0.** Tempting, since
  it makes the rule uniform. Rejected: it puts a subtle contract on every
  array call site (`nodes[0].size = sizeof nodes[0];`, on element 0 only,
  before every call), and a host that forgets it gets a silent zero-length
  result rather than an error it can read. An explicit stride argument says
  the same thing in the signature, where a host cannot skip it.
- **Bump the version on [in] appends too, for one simple rule.** Rejected:
  it is simple in the wrong direction. Every new `KuiSpec` prop — and the
  prop schema grows most releases — would become a hard break for hosts
  that are provably unaffected, and a version that bumps for harmless
  reasons is one hosts learn to ignore. *Reversed 2026-09-14: the hosts
  were not unaffected — see the amendment below.*
- **Coalesce bumps within a release window**, so the number moves at most
  once per release and every published number is one a host could have
  linked against (considered 2026-09-06, backlog S8). Rejected: it buys
  tidiness in a number nobody reads as a range, and pays for it in the one
  place the version does real work. Two builds from inside the same release
  window would share a number while writing different [out] layouts, so a
  developer running a host against a locally built `libkui_ffi` — the case
  where header and library routinely disagree, and the case a released
  number cannot help with anyway — would get no check at all. It also
  charges the release process with remembering whether the current number
  is already unreleased, a step that fails silently the first time it is
  skipped.
- **`cbindgen`, so the header is generated and drift is impossible.**
  Out of scope, and already weighed in P6: the header's prose is most of
  its value, and the static asserts already settle drift. Neither of those
  addresses the old-binary case this ADR is about.

## Consequences

- **Every C host recompiles once**, and edits the declaration of any
  `KuiEvent`, `KuiDrawData`, `KuiTextMetrics` or `KuiScrollGeometry` it
  keeps — `KuiEvent ev;` becomes `KuiEvent ev = KUI_EVENT_INIT;`. That is
  the last time an append to one of those four costs a host anything, which
  is the trade: one break now for none later. It is also strictly cheaper
  than the break ADR 0004 already committed to, and it lands before it
  rather than after.
- **ADR 0004's `KuiEvent.window` becomes a compatible change** — for the
  size handshake. It still bumps `KUI_ABI_VERSION`, because a host that
  skipped the version check and reserved the old struct now gets a short
  write it did not expect, and because the same release changes
  `kui_take_window_commands`.
- **`mod abi_parity` grew a second macro.** `KUI_OUT_STRUCT(T, v1)` asserts
  that `size` leads the struct and that the layout has never fallen below
  what ABI 1 shipped; the ABI-1 floor comes from Rust's own `OutParam` impl
  (measured through the last ABI-1 field, so it is per-target correct and
  does not follow future growth), so the two cannot drift.
- **The truncating write path is exercised now, not later.** Nothing has
  been appended to a real [out] struct yet, so every `ABI_V1_SIZE` still
  equals its `size_of` and the public API cannot reach the interesting
  branch. A unit test stands in with a struct that *has* grown, so the
  first real append is not also the first run of the code it depends on.
- **`examples/c/counter.c` checks the version in `main` before anything
  else**, and its `--headless` pass asserts the refusal — that a short
  reservation is turned away and that the event it refused is still there.
  It is the worked example a C host copies.
- **Not covered, and knowingly.** The version says nothing about *why* two
  builds differ, so a host cannot degrade gracefully — it can only refuse.
  That is the right answer at this size (there is one supported pairing),
  and the moment it is not, the fix is a second entry point reporting a
  minimum compatible version rather than more meaning packed into this one.
  Nothing here helps a host that never calls `kui_abi_version()`; the
  `size` handshake is what still catches those, on four structs.

## Amendment 2026-09-14: an [in] append bumps too (backlog AR50)

The context above rests on one sentence — a host that predates an [in]
field "passes a shorter struct, the library reads no further than the host
wrote, and the zeroed tail is the documented default" — and the second
clause was never true. Nothing in `kui-ffi` reads an [in] struct field by
field up to a length the host supplied, because no [in] struct carries a
length: `kui_open` copies `*spec` whole, `window_config_of` reads every
field of a `KuiWindowConfig`, `kui_audio` reads the whole `KuiAudio`. A host
binary built against the shorter `KuiSpec` and run against a library that
had appended `tooltip` passed the version check (equality, and [in]
appends did not bump) and then had `tooltip` read from whatever followed
its struct on the stack — a `KuiStr` whose pointer was garbage, dereferenced
whenever the length word happened not to be zero. Every append made under
the rule (`accent` after ABI 9, `selectable` and `on_force_click` after 11,
`focus_region` and the scrollbar quartet after 13, `anchor` after 14,
`on_scroll` after 15, `KuiTextStyle.features` / `.decoration`,
`KuiAudio.finish`) carried this hazard. None of it bit, because every host
in the tree is built against the header it links; the claim was masked, not
kept. It was found building `KuiRunConfig` for backlog AR27, where a new
[in] struct had to choose between the rule as written and a third category.

**Decision.** The rejected option above becomes the rule: `KUI_ABI_VERSION`
bumps when **any** struct's layout changes, [in] included, and when an
existing function's signature changes (the ABI 12 and 16 case, which the
original text did not name). Only a new function is free. The alternative
— a leading `size` on every [in] struct that is not an array, read by a
`read_in` mirroring `write_out` — would have kept the exemption honest,
but at a `size` on every `KuiSpec` literal in every C host, which is the
tax the "size on array elements" option above was already rejected for
levying on a subtler contract. The "flag day" the original rejection
feared is a recompile, and a recompile is what an [in] append has always
required of a host that wanted the field to be read from its own struct;
the number now says so instead of pretending otherwise.

**What holds it.** `abi_parity::an_in_struct_s_size_is_the_abi_s` pins
the size of every struct the header's `[in]` paragraph lists to a table in
the test, on 64-bit targets. An append fails that test until the row and
the version both move, with the rule in the failure message. The header's
ABI block and `abi.rs`'s note say the new rule and keep the old sentence
as a quotation of what was wrong with it; the per-field comments that said
"the compatible way" now say "without a bump, under the rule as it then
stood". No number moved for the amendment itself: ABI 16 was current and
covers every append the old rule let through, so a host that checks the
number is served from here on.
