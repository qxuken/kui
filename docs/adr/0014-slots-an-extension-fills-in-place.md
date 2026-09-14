---
status: accepted
date: 2026-09-07
---

# Slots: an extension fills a place the host declares, with parameters in and replies out

> **Amended 2026-09-08.** The open question below — "an extension cannot
> declare slots for other extensions, which is a decision for the day one
> asks" — is answered: an extension may load extensions and declare their
> slots, replies go to whoever declared the slot rather than always to the
> host, and the one thing refused is an extension filling its own slot. See
> [Amendment: an extension hosts extensions of its own](#amendment-an-extension-hosts-extensions-of-its-own-2026-09-08)
> at the end of this document.
>
> **Accepted and built (2026-09-07), for alpha.9.** Proposed and accepted
> the same day; what follows is the draft as accepted, and this block is
> what the building changed in it. **Slot names are namespaced, and the
> host decides the namespace** — a decision made after the draft, the way
> a new language's importer picks the alias a module is bound to. An
> extension names the slots it fills in its own vocabulary (`"panel"`, no
> `/` in it); the host gives each extension a namespace when it loads it
> (`Launcher::extension_as("fs", ext)`, or `extension(ext)` for the
> extension's own name; `Extensions::push_as` underneath) and declares a
> slot by its full name, `ui.slot("fs/panel")`. So decision 2's "an
> extension names the slots it fills" holds with the namespace in front,
> exactly one extension can fill a slot, the same plugin loaded twice is
> two namespaces with two sets of params, the name-plus-kind option is
> answered rather than deferred, and decision 4's namespace is the slot's
> key alone — `enclosing.str("fs/panel")` — with no extension name
> appended, since the full name already carries it. The reserved fill is
> `ns/root`. Four smaller things the code decided: Lua's `view` takes the
> slot as a **second argument**, `view(env, slot)`, because `env`'s value
> keys are pinned to `schema::ENV_FIELDS` across the bindings and a slot
> is the host's fact, not the driver's; a fourth warning code,
> `extension-view-error`, replaced the runner's per-frame stderr line for
> a failing `view`, now that the loop lives in the core where there is no
> stderr; the C reads follow the header's out-pointer convention,
> `bool kui_slot_name(ctx, KuiStr *out)`, plus a `kui_slot_namespace`
> beside it; and an extension's own `slot` call declares nothing. What
> the runner delivers is not headlessly tested — `Shell::route_events`
> needs a window — so `examples/c/panel.rs --headless` delivers the
> replies itself, the way it always built its own frames.

An extension draws into the host's frame, and today it can draw into
exactly one place: after the host's own view, as the last children of the
root. The host's only lever is `configure_root`, and both panel examples
say so in a comment — "extensions append after the host, so the root row
places the panel to the right". A host with a layout of its own — a
sidebar, a status bar, a split — cannot give an extension a place inside
it, and cannot tell the extension anything (which directory to show) or
hear anything back (which file was chosen). We decided that **the host
declares a slot at a position in its own view**, `ui.slot("oil")` or
`ui.slot_with("oil", &params)`, and **the extension fills it there and
then**, as children of the node the host is inside, under a key namespace
the slot name fixes; that **parameters are a `Value` declared every frame
and never retained**; that **an extension names the slots it fills** and
one naming none fills the root as it does today; and that **an extension
replies to the host as data** returned from `on_event`. The tree stays
preorder by construction, because a slot is filled while the cursor is
there, not appended to a node that has closed.

## Context

- An extension is one trait over a borrowed frame — `name` / `view` /
  `on_event` (`crates/kui-core/src/runtime.rs:649`) — with two
  implementations, `LuaExtension` and `CExtension`, that are the point of
  it being one trait. The runner calls the host's `view`, then each
  extension's `view` in turn with the origin set to its index
  (`crates/kui/src/lib.rs:1777`), and `examples/c/panel.rs::frame` builds
  the same sequence by hand for the headless check. Nothing moves the
  cursor between the two, so an extension's nodes go wherever the host
  left it, which is the root when the host is balanced.
- **The tree is DFS preorder in flat arrays**, and a parent's subtree is
  one contiguous index range: `Tree::subtree_end` walks `next_sibling`
  and `parent` on that assumption, and preorder is paint order
  (`crates/kui-core/src/tree.rs:1`). A child appended to a node that has
  already closed would sit outside its parent's range. C15 spent a round
  keeping `Tree::push` a handful of stores; a splice into twelve parallel
  vectors with every later index rewritten is the opposite of that.
- **Extension keys are not stable today.** `auto_key` derives a node's
  key from its parent's key and a per-parent counter
  (`crates/kui-core/src/runtime/builder.rs:180`). An extension whose root
  node carries no `key` — both shipped panels, `kui_open(ui, &panel,
  NULL)` and a Lua `column { ... }` — gets `root.index(n)` where `n` is
  however many children the host opened at the root. A host that shows
  one conditional toast at the root moves `n`, and with it every key
  beneath: transitions restart from their enter values, focus by key is
  lost, and an editor keyed relative to that root — the Lua panel's
  `filter` — is a new key to `EditStore::declare`, which reinstates
  `initial` (`crates/kui-core/src/edit.rs:389`). Read from the code, not
  run; the fix at the extension's end is one `key`, and nothing tells an
  author to add it.
- **Isolation is by origin and is total.** A hit carries the origin of
  the node that declared it, the runner routes host events to
  `App::on_event` and an extension's to that extension, and the host
  "never inspects the plugin's UI or its events" (`examples/c/panel.rs:1`).
  That is the right default and it has no exception: an extension that
  wants to tell the host something has no door, and a host that wants to
  tell an extension something has none either. `Value` is the one
  dynamic type that crosses the boundary (`crates/kui-core/src/value.rs:1`),
  and every binding already converts it — a Lua table, a `KuiValue`, JSON
  in Node.
- **The codebase's own rules for the two halves.** Everything the host
  wants the core to know is declared every frame and diffed — the window
  title, the declared window set, the focus — and nothing is set once
  and retained. What an app hands back besides the model is data: ADR
  0013 made `update` return effects as values rather than call anything.
  A pair of functions is how `Ui` spells an optional extra — `open` /
  `open_keyed`, `line` / `line_keyed`, `audio` / `audio_keyed` — and a
  `NULL` is how C spells the absence (`kui_open(ui, &spec, NULL)`).
- Who hosts. Extensions are a Rust-host feature: `kui::run` takes them,
  `kui_run` (`crates/kui-ffi/src/run.rs:44`) does not, and Node has no
  extension at all — its mentions of "1+ an extension" are in the `origin`
  field's documentation. `Ui` is a borrowed `&mut Core` and nothing else
  (`crates/kui-core/src/ui.rs:16`).
- ADR 0006's bump rule: a new function does not bump `KUI_ABI_VERSION`
  (`docs/adr/0006-c-abi-versioning.md:82`); a changed [in] or [out] struct
  layout does. The C side of this ADR is functions only.

## Decision

1. **A slot is a position the host declares in its own view.**
   `Ui::slot(name)` and `Ui::slot_with(name, &Value)`; for a C host,
   `kui_slot(ctx, name, params)` with `NULL` for none. The call may sit
   anywhere among the host's children — between the left and right items
   of a status bar, alone in a sidebar column — and the extensions that
   fill it draw **then**, as children of the node the host is currently
   inside, at that position. Preorder holds because nothing is appended
   after the fact. A slot is not a node: it has no spec, no rect and no
   hit region, and the host's own spec on the enclosing node is what
   sizes and places the fill.
2. **An extension names the slots it fills.** `Extension::slots(&self)
   -> Vec<String>`, default empty; a Lua script's `slots = { "oil" }`
   global; a C plugin's optional `const KuiStr *kui_ext_slots(size_t
   *count)`, a pointer to an array the plugin keeps alive for its own
   lifetime and the count written through the out-pointer — the header's
   one convention for strings and arrays, `KuiStr` is a pointer and a
   length and `kui_polyline` / `kui_rich_text` take `const T *, size_t
   count`, and a NUL-terminated list would be the first thing in it that
   is not. `view` receives which slot it is filling: `fn view(&mut self,
   slot: &Slot, ui: &mut Ui<'_>)` with `Slot { name, params }` in Rust,
   `env.slot = { name = ..., params = ... }` in Lua, and `KuiStr
   kui_slot_name(ctx)` / `const KuiValue *kui_slot_params(ctx)` in C, the
   name and the params borrowed for the call like an event payload. A C
   host declares one with `kui_slot(ctx, KuiStr name, const KuiValue
   *params)`, `KUI_STR("oil")` and `NULL` in the common case. An extension
   naming no slot is called once after the host's view with the slot
   named **`"root"`** and no params, which is exactly today's sequence.
   `"root"` is reserved: a host that declares `ui.slot("root")` moves
   that fill to the position it chose, so a host can place an extension
   written before this ADR without the extension changing.
3. **Parameters are `Value`, declared every frame, and never identity.**
   The host passes what it has this frame; the core keeps nothing between
   frames and there is no `set_slot_params` outside a build. A host that
   would rebuild the same map every frame keeps it in its model and
   passes a reference. Parameters do not enter any key: changing `path`
   from `/Users` to `/tmp` must not rekey the extension's subtree, or
   every tween in it restarts. Two instances with different parameters are
   two slot names. What a parameter means is between the host and the
   extension that reads it; the core carries it and does not look.
4. **The fill has a key namespace, and it is the slot's.** Nodes an
   extension opens inside a slot are keyed as if their parent's key were
   `enclosing.str(name).str(extension name)`: a namespace stack beside the
   node stack, which `auto_key` and `child_key` read in place of
   `tree.keys[parent]`, pushed on the fill and popped after it. No wrapper
   node — a wrapper is not layout-transparent, and a child declaring
   `width: "grow"` inside a hugging wrapper would stop growing. This is
   what fixes the third context item: an extension's keys depend on the
   slot's name and the extension's, and on nothing the host does around
   them. The slot's key `enclosing.str(name)` is recorded in the label
   index, so `key_of("oil")` answers and a host can `reveal` or `focus`
   into a fill by the slot's name; a sibling the host keyed with the same
   label is the `ambiguous-key` case that index already reports.
5. **A fill is bounded, and the bound is enforced.** `Ui::slot` records
   the stack depth before each extension's `view`, truncates back to it
   after, and raises an **`unbalanced-extension`** warning naming the
   extension when it had to. Today an unbalanced extension only swallows
   the extensions after it (`layout_frame` truncates to the root at the
   end, `crates/kui-core/src/runtime/emit.rs:228`); inside a host's view
   it would swallow the rest of the host, which is why this is not
   optional. Two more warnings, once each like every diagnostic:
   **`unknown-slot`** when an extension names a slot no host declared this
   frame — the extension is not drawn, because drawing it at the root
   "somewhere" is worse than nothing and the warning says what to declare;
   and **`duplicate-slot`** when a host declares one name twice in a
   frame — the second is ignored, since the fill's keys would collide with
   the first's.
6. **An extension replies to the host as data.** `Extension::on_event`
   returns `Vec<Value>`; a Lua `on_event` returns nothing, a table (one
   reply) or a sequence of tables (several), which is the distinction
   `lua_to_value` already draws; a C plugin calls **`kui_reply(ev, value)`**
   during `kui_ext_on_event`, as often as it likes, and the host copies
   each — no signature changes, so no ABI bump under ADR 0006's rule. Each
   reply reaches `App::on_event` as a `UiEvent` whose `origin` is the
   extension's, whose `window` and `key` are the event's it answered, and
   whose `payload` is the reply. It is not routed by origin — a reply is
   addressed to the host by being a reply — and the origin on it is
   information, the first the host has ever been given about an
   extension. The runner delivers replies in the same `route_events` pass,
   after the extension's `on_event` returns, and they count as "reached
   the app" for the every-window redraw rule.
7. **The host authors the reply's shape.** Nothing in the core says what
   a reply contains. The pattern is the one `on_click` already uses: the
   host puts a message template in the params — `on_open = { kind =
   "open", slot = "left" }` — and the extension returns that map with its
   fields added. An extension cannot invent a host message the host did
   not write, a host distinguishes instances by what it wrote, and a
   headless test of the host asserts on replies the way it asserts on
   clicks.

## Considered options

- **Append to any keyed node after the host's view.** The literal reading
  of "hook into any keyed node", and the one that asks for a splice into
  the flat arrays: a subtree built into a side tree, inserted at the
  target's `subtree_end`, every index after it shifted across twelve
  vectors, per extension per frame. Rejected for the second context item.
  Building the fill *in place* is the same result with no splice, and it
  is the only way the extension's keys can be derived from the slot: a
  key is a hash of the path from the root, and a subtree built elsewhere
  has the wrong path.
- **Every keyed node is a slot, filled on `close`.** No new call: an
  extension names a label and the fill happens when that node closes.
  Rejected. It makes every `key` prop a public extension point, so a host
  renaming an internal key breaks plugins silently; labels are unique
  among siblings and not across the tree, so which node fills is the
  `ambiguous-key` question asked on every frame; and it puts a lookup on
  every keyed `close`, which is a hot path. A slot is opt-in and named,
  and the host chooses its spec by choosing the node it sits in.
- **A wrapper node per extension** instead of the namespace stack of
  decision 4. Rejected: not layout-transparent (a hugging wrapper defeats
  a growing child, a growing wrapper changes the host's layout), and it
  adds a node to every fill for a key derivation the stack does in two
  pushes.
- **Parameters through `env`.** `env` is the driver's facts — refresh
  rate, focus, the window — pinned by `schema::ENV_FIELDS` across three
  bindings. Slot parameters are the app's, and would drift the pinned
  shape. Lua reads them from the env *table* because `view(env)` takes
  one argument, and `env.slot` is a sub-table that is present only
  during a fill; the row list does not change.
- **Retained slot state**, set by the host outside the frame and read by
  the extension. Rejected: nothing in the core is set once and kept, and
  a value the host stops passing would have to be un-set by another call.
- **A new parameter type.** Rejected: `Value` crosses the boundary in the
  other direction already, with a conversion in every binding, and a
  typed map would be a second one.
- **A builder** — `ui.slot("oil").with_params(..)` — filling on drop or
  on a terminal call. Rejected for one optional: a fill on drop hides the
  one thing a slot must make explicit, the position; a terminal `.fill()`
  is three calls for the common case and a silent no-op when forgotten.
  If a third optional ever appears — a *kind*, below — a `#[must_use]`
  builder with a terminal call is the shape to reach for.
- **Replies from the next frame's view**, an extension posting from
  `view` what its `on_event` decided. Rejected: a frame late, and a
  pending field in every extension that wants it. ADR 0013 chose
  returning data for the same choice on the host's side.
- **Replies by changing `kui_ext_on_event`'s signature.** Rejected in
  favour of `kui_reply`: a new host symbol the plugin resolves at load,
  no struct change, no ABI bump, and a plugin that never replies does
  not know the symbol exists.
- **An extension opens nodes under the host's origin** to hand the host
  a click. Rejected: origin is the whole of the isolation and this
  invents an exception to it.
- **Name plus kind** — the extension declares what it *is*
  (`kind = "file-browser"`) and the host names instances,
  `ui.slot("left", "oil", params)`. Not decided here. With parameters the
  case for it is real (a `path` means something only to a file browser,
  and a split view wants the same extension twice), and decision 2's
  name-only contract is a subset of it: a kind defaulting to the
  extension's own name is `ui.slot("oil", params)` unchanged. It waits
  for a view that wants two instances of one extension; when one does,
  the builder form above is where the third argument goes.
- **Slots in Node**, a `<slot name>` element. Not decided here: Node
  hosts no extensions, so the element would fill with nothing. The row
  is one line in the schema on the day that changes.
- **Extensions in `kui_run`**, so a C host can load a Lua or C plugin.
  Not decided here; orthogonal, and one `KuiExtension *` list argument
  when a C host asks.

## Consequences

- **Nothing shipped to an app from this ADR until it was built** (the
  same day, for alpha.9; the CHANGELOG entry is alpha.9's). Until then
  `docs/BACKLOG.md`'s "After alpha.8" — the heading has moved with each
  tag since — named this as the one written ADR with no code.
- **When built, no existing app or extension changes behaviour.** An
  extension with no `slots` fills the root after the host's view, as now;
  a host that never calls `ui.slot` sees the same frame. The one visible
  difference is decision 4: an extension's unkeyed root moves from
  `root.index(n)` to the `"root"` namespace once, at the upgrade, and is
  stable thereafter — a change a running app cannot observe.
- **The build, by crate.** `kui-core`: the namespace stack in the
  builder, `Core::slot` / `slot_with` and the `Slot` type, a `Fill` trait
  the runner implements for its extension list and `Core::frame_with`
  to hand it in, the three warning codes, and `Extension`'s new `slots`
  and `view(slot, ui)` and `on_event -> Vec<Value>`. `kui`: `Shell::redraw`
  passes its extensions as the filler and fills `"root"` after the host's
  view unless the host declared it; `route_events` delivers replies.
  `kui-lua`: the `slots` global, `env.slot`, the return of `on_event`.
  `kui-ffi`: `kui_slot` for a C host, `kui_slot_name` / `kui_slot_params`
  / `kui_reply` for a plugin, `kui_ext_slots` looked up at load, all
  functions and no struct change. The `Ui` façade grows one field,
  `Option<&mut dyn Fill>`, which is `None` inside an extension's own
  `view` — an extension cannot declare slots for other extensions, which
  is a decision for the day one asks.
- **Tests that pin it.** In `kui-core`: an extension's keys are the same
  before and after the host adds a root child (the third context item,
  which today's code fails); a fill under a slot has the slot's key in
  `key_of`; an unbalanced extension is truncated and warned; an unknown
  slot and a duplicate slot warn once. `examples/c/panel.rs --headless`
  gains the slot: the host places the panel with `ui.slot("panel")`
  inside its own row, hands it a parameter, and checks a reply from the
  plugin's click reaches the host with the plugin's origin on it. A Lua
  test drives `env.slot` and a returned reply. No conformance scene: the
  corpus is built by the bindings and has no extension in it.
- **The examples become the fixture.** Both `panel.rs` hosts drop the
  "extensions append after the host" comment and the root-row workaround,
  and declare the slot inside their own layout; `panel.lua` and `panel.c`
  declare `slots` and read one parameter — the panel's title, so the
  change is visible without a file browser.
- **`docs/props.md` does not change.** A slot is not a node and has no
  row; the warnings are three lines in the generated warnings table.
- **Not done here.** Name plus kind and multiple instances of one
  extension; a slot element for Node; extension loading in `kui_run`;
  an extension declaring slots of its own; replies from `view`.

## Amendment: an extension hosts extensions of its own (2026-09-08)

The "not done here" list ends with *an extension declaring slots of its
own*, and the core said the same thing where it refused one: "whether one
may offer slots is a decision for the day one asks". A Lua view wanting a
native panel inside it is the day. The answer is **yes, and it is this
mechanism one level down rather than a second one**.

What changed, and what did not:

1. **A guest declares slots.** `Ui`'s filler is no longer `None` inside a
   fill: `Core::fill_within` hands it down, so `ui.slot(…)` from an
   extension is a slot like the host's, declared where the extension is
   drawing and keyed there — move the guest and its guest moves with it,
   which is decision 4 applied twice. `Core::begin_slot` no longer refuses
   inside a fill.
2. **A guest loads.** `Fill::add` / `Ui::add_extension` puts an extension
   in the list *while a frame is being built*, which is when a guest
   knows it wants one. It joins the **same** list under a namespace of its
   own, so there is one namespace map and one origin per extension however
   deep the loading went: `todos/panel` means one thing to everybody, a
   namespace the host has taken is refused the way `push_as` refuses it,
   and nothing in the routing had to learn about levels.
3. **Replies go to whoever declared the slot.** Decision 6 read as it is
   written — the thing an extension answers is the slot it was put in. For
   every extension a host placed that is the host, unchanged; for one a
   guest placed it is the guest, and what the guest answers travels on up.
   The walk is `Extensions::route`, which all four hosts now call instead
   of each keeping its own copy of the loop, with `MAX_REPLY_HOPS` as the
   bound on two extensions answering each other.
4. **One thing is refused, and it is the cycle.** An extension is *out* of
   the list while it fills (the entry is `None`), so its own slot is the
   one name that finds nobody: an empty position and a **`recursive-slot`**
   warning rather than a view calling itself. This is also what makes
   re-entering the list sound at all.
5. **Loading is C shared libraries, and only that.** `env.add_extension`
   in Lua opens a `.so` / `.dylib` / `.dll` exporting the `kui_ext_*` entry
   points — the same plugin a Rust, C or Node host loads. A script does not
   load another script: a host that wants two scripts loads two, and
   "scripting inside scripting" buys nothing that costs nothing.

Per binding: Lua gets `env.add_extension(namespace, path)`,
`env.extension_namespaces()`, and `fill { name = "ns/slot", params = … }`
in the tree — named `fill` and not `slot` because `slot` is `view`'s second
argument and would shadow the constructor in the one function that needs
it. A reply from a plugin the script loaded arrives at `on_event` with
`from` naming the namespace; the script's own events have no `from`, which
is what tells them apart. C plugins can declare a slot from `kui_ext_view`
(their context now carries the host's `Ui`, the way the runner's own view
callback does) but cannot load one — a plugin's context has no list —
so the name has to be one the host above it already loaded.

This is not a new capability in the security sense, and the Lua side is
where that is worth saying: `Lua::new` has `package`, so a script could
already `package.loadlib` anything on the disk. What it could not do is
put what it loaded in its own tree.
