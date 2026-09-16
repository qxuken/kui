---
status: accepted
date: 2026-09-15
---

# A devtools tab: a slot for an extension, a lazy subtree for the host

> **Accepted and built (2026-09-15), the same day it was proposed** —
> what the building changed is at the end, under [*What the building
> changed*](#what-the-building-changed). Raised by wanting a
> tree-sitter inspector beside the panel's three tabs: a tab that shows
> the syntax tree under the editor's cursor, highlights the node the
> panel's picker has, and scrolls the editor when a row is clicked.
> Nothing in the core knows tree-sitter and nothing should; what the
> core can offer is the *place* — a fourth tab in the strip, with the
> dock's placement, resize handle, chords and on/off — and the panel's
> own facts as input. The question is who builds the tab's content and
> when, since the panel is built in `Ui::finish`, after the app's view
> has returned, and an app cannot be "inside" it. The answer is that
> **the tab body is a node, and the content is a layer anchored to it**
> — from either of two parties. An extension fills it as a slot
> ([ADR 0014](0014-slots-an-extension-fills-in-place.md)) the panel
> declares; a host builds it from its own view, **lazily** in every
> binding: a closure in Rust, an open that answers in C, a function
> child in Node and Lua, each run only when the tab is the one shown.
> The first draft of this document generalised the host's half into
> "a host provides the content of any slot"; that collided with ADR
> 0014's first decision and was cut — see *Considered options*.

## Context

- **What a tab needs from the core.** Three things, unequal in cost.
  (1) A place: a tab in the strip, `Ctrl+Shift+N` cycling to it, and
  everything the dock already has — placement, the resize handle, the
  chords, the on/off, the focus region. (2) The panel's facts as input:
  the picked and selected node, the hovered row, so an inspector can say
  "this node is `identifier` at 12:4". (3) Its events reaching the
  *app* — a click on a syntax node scrolls the editor — where every
  event of `OriginId::DEVTOOLS` is consumed inside `handle_input` (ADR
  0024, decision 4). (2) and (3) are small doors. (1) is the whole
  question.

- **When the panel is built.** Docked right or bottom, in `Ui::finish`,
  after the host's view returned; docked left, in `devtools_begin_frame`,
  *before* the host's view, from the last frame's facts, because a left
  dock precedes the app in the root row and the tree is preorder by
  construction; in its own window, in that window's `finish`, where the
  host's view is called and builds nothing (every builder door is a
  no-op on the empty tree the window keeps until `finish`). So the app's
  view is never running while the panel is being built, in any
  placement.

- **What a slot is** (ADR 0014). The host declares a position in its own
  view, `ui.slot("fs/panel")`; the extension that names `panel` fills it
  there and then, as children of the node the host is inside, under the
  slot's key namespace; params are a `Value` declared every frame and
  never retained; replies come back to whoever declared the slot. **A
  slot is not a node** — no spec, no rect — and **a fill is in place,
  with no wrapper**, both by decision. Since the 2026-09-08 amendment an
  extension may declare slots and load extensions of its own. Only an
  extension fills: a C shared library or a Rust `Extension` impl. Lua's
  `fill { name = }` *declares* a slot (the name is a trap, kept for
  compatibility), a script loads no script, and a Node app has no way
  to be an extension at all.

- **Where the filler is.** `Ui` carries it (`Ui::with_filler`); `Ui::finish`
  runs `filler.finish` — the `ns/root` fills and the `unknown-slot`
  check — *and then* `core.devtools_finish()`. A slot the panel declares
  would be declared after the check that warns about undeclared ones.
  `build_panel` is a `Core` method that builds with `Ui::new(self)`: no
  filler reaches it today, and the panel's own window's frame is built
  without one.

- **Where the data bindings build.** A Node app's tree is data the
  encoder (`packages/kui`, JS) turns into bytes for one `frame` call; a
  Lua app's tree is a table the runner's converter turns into builder
  calls. Neither runs a closure of the app's mid-frame — but both are
  a pass over the app's data that can *ask the core a question* before
  descending into a subtree, which is what makes laziness possible
  there without a stored closure.

- **What already paints out of tree order.** A float (`NodeSpec::float`)
  is a node taken out of flow, sized against its anchor (the parent or
  the viewport), positioned by attach points, escaping ancestor clips,
  painted and hit-tested as a layer in opening order
  ([ADR 0023](0023-paint-order.md)). Ghosts keep a float's place through
  the three-shaped `depart::Place`. A subtree that is built in one place
  and shown in another is a float whose anchor is a node elsewhere in
  the tree — an anchor kind the float does not have yet.

- **The alpha.13 lesson.** The main window's build takes the panel's
  state out of the session (`mem::take`) for the duration of the build,
  so a reader of a devtools door inside the build sees the default — a
  real window showed `^⇧I` over an `F12` the chord already answered to
  (ADR 0024 item 22). Anything a tab's content needs from the panel
  during the build must be handed in, not read; and the state *is* in
  the session while the host's view runs, which decision 3 leans on.

## Decision

1. **A tab is declared by name and label, in one of two forms, and any
   origin may declare one.** The *extension form* names a slot:
   `ui.devtools_tab("syntax", "Tree-sitter", "ts/panel")` in Rust,
   `<devtoolsTab name="syntax" label="Tree-sitter" slot="ts/panel" />`
   in Node, `devtools_tab { name = , label = , slot = }` in Lua,
   `kui_devtools_tab(ctx, name, label, slot)` in C — a declaration
   node, no layout, no children, like a menu declaration. The *host
   form* carries the content: `ui.devtools_tab_with("syntax",
   "Tree-sitter", |ui| …)` in Rust; `if (kui_devtools_tab_open(ctx,
   name, label)) { …; kui_close(ctx); }` in C; in Node a **function
   child**, `<devtoolsTab name="syntax" label="Tree-sitter">{() =>
   <…/>}</devtoolsTab>`; in Lua `devtools_tab { name = , label = ,
   view = function(env) return … end }`. A `devtoolsTab` with a child
   that is not a function, or with both `slot` and a child, is refused
   by the encoder (a throw, since it is the app's own tree) and by the
   converter (`extension-view-error`'s sibling, `bad-devtools-tab`).
   The tab's identity is `name`; `label` is what the strip shows.
   Declared tabs follow `facts`, `events`, `tree` in declaration order;
   `Ctrl+Shift+N` cycles through them; the strip wraps whole tabs onto
   another line (a tab's label never breaks inside it — backlog F72,
   which found the row breaking labels instead). **A tab declared twice in a frame warns
   `duplicate-tab`, keyed by the name the way `duplicate-slot` is, and
   keeps the first** — including a host form and an extension form of
   the same name, and a tab an extension declares from inside its fill
   under a name the host took. A declaration is made every frame, panel
   on or off, because it is cheap and because it is what makes the
   slot *known* (decision 5).

2. **The tab body is a node, and the content is a layer anchored to
   it.** This is the one mechanism the document adds, and it is what
   makes every placement and both parties uniform. When the panel is on
   and the tab is current, the panel builds the tab body: an empty node
   in the tab area, Grow both ways, clipped, keyed
   `kui-devtools/tab/<name>` so `key_of` finds it and the tree tab
   lists it. Whatever the tab's content is — an extension's fill or a
   host's subtree — is a subtree of its own, laid out against the
   body's rect, positioned over it, clipped to it, painted and
   hit-tested as a layer at the body's stacking place, and in the
   dock's focus region. `FloatAnchor` gains a third variant,
   `Node(Key)`, and the content's root is a float with that anchor; the
   retained float stack, `target_at`, the ghost `Place` and the
   `layers` corpus scene already cover the rest. Nothing is filled in
   place: for the extension form the panel declares
   `ui.slot_with("ts/panel", &facts)` inside the body and records the
   mount, and `Ui::finish` — which holds the filler — fills every
   recorded mount after the panel is built, each as a layer anchored
   to its body. That is what lets a left dock, built at `begin_frame`
   with no filler in reach, mount a tab: the body is built early, the
   content late, and the layer closes the gap. For the host form the
   subtree is built wherever the host's view reached the declaration —
   before the body in a right or bottom dock, after it in a left one —
   and the layer does not care which.

3. **The host form is lazy in every binding: the content is built only
   when the tab is the one shown.** The rule is "the panel is on, the
   tab is current, and the placement is docked" (decision 6 says why
   docked). The panel's state is in the session while the host's view
   runs — it is taken only during the build — so the rule is answerable
   before the content is reached, with no retention:
   - **Rust**: `devtools_tab_with` runs the closure only when the rule
     holds; otherwise it declares the tab and returns.
   - **C**: `bool kui_devtools_tab_open(ctx, name, label)` answers the
     rule; on `false` it declared the tab and opened nothing, and the
     host skips the body and does not `kui_close` — the way `kui_slot`
     answers whether it declared.
   - **Node**: the encoder calls the function child only when the rule
     holds. It learns the answer from the driver: `KuiWindow` (and a
     headless `Ctx.frame`) hands the encoder the current tab's name and
     the placement before encoding — one reading per frame, from the
     core, not a per-tab call — and `devtoolsTab` compares. A function
     child that throws is the app's throw, as any view's is.
   - **Lua**: the converter calls `view` only when the rule holds, with
     the same `env` the view got, so the tab reads the model the view
     read.
   The extension form is lazy by construction: a tab that is not
   current declares no slot, and a slot nobody declares is not filled.
   Off, a frame with no declared tab pays one flag, the way
   `any_float` gates the float pass; a frame with declared tabs and
   none current pays the declarations.

4. **The panel's facts are params for the extension and doors for the
   host.** The extension form's `slot_with` carries `{selected,
   hovered, picked, region, focus}` as keys and labels, the rows the
   facts tab prints. The host form is built from the host's view and
   reads the same facts through doors — `devtools_selected()`,
   `devtools_hovered()`, `devtools_picked()`, each an `Option<Key>`,
   on `Ui` and `Core` — and one writer, `set_devtools_selected(
   Option<Key>)`, so an inspector can drive the tree tab's highlight
   (and its reveal, the picker's path) from its side. Four bindings,
   four `schema::DOORS` rows. The readers answer from the session state
   and never from the taken copy (the alpha.13 lesson), which is why
   they are doors and not something the build hands the subtree; the
   extension gets them as params because that is the channel a fill
   already has, and an extension reading `ui.devtools_selected()`
   inside its view gets the same answer.

5. **The slot side of the extension form, against ADR 0014.** Four
   small things, none of them a change to what a slot *is*:
   - *Known.* A slot named by a `devtools_tab` this frame counts as
     declared for the `unknown-slot` check whether or not the panel
     mounted it, so a plugin whose only slot is a tab is quiet with the
     panel off or another tab up.
   - *Order.* `Ui::finish` runs the devtools build *before* the
     filler's `finish` from now on, so the check sees the panel's
     declarations and the recorded mounts are filled with the filler
     in hand; the `ns/root` fills still land after the host's tree, as
     they do.
   - *Replies.* "Replies go to whoever declared the slot" — the panel
     declared it under `OriginId::DEVTOOLS`, which is nobody's;
     `Extensions::route` treats a devtools-declared slot as
     host-declared, so what the plugin answers reaches the app.
   - *Exactly one.* Unchanged: one extension fills the tab's slot, a
     second naming it is the ordinary second-fill case. A wildcard
     extension (`"*"`) sees the panel's declaration as one more name
     under its namespace, as it should.

6. **Placement.** Docked left, right and bottom, both forms work — the
   layer is what makes the left dock possible at all. In `window`
   placement the extension form works once the pane builds that
   window's frame `with_filler` (the same list as the main window's; an
   addition to the runner). The host form is the one gap: its subtree
   lives in the main window's tree, and there is no cross-window node
   transport — facts and `nodes()` cross the session as data, not as
   paintable nodes. So decision 3's rule says *docked*, the closure
   does not run, and the panel's window shows the tab with "docked
   only" in the body, the way a left dock reads last frame's facts.
   The host's view *is* called for the panel's window, so a host form
   reached there could build into that window's own tree — a possible
   step 2, deferred until a host asks, since it doubles the host's view
   work for the tab's sake.

7. **Keys and ghosts.** The host form's nodes are keyed where the host
   put them — its keys, its labels, its origin — so `key_of`, focus,
   `reveal` and the tree tab treat them as the app's, and a tween in
   the tab survives the dock moving side to side. An extension form's
   keys are the slot's, `kui-devtools/tab/<name>/<slot>/<ext>`, which
   is the same in every docked placement and differs in the panel's
   window (a different root): a tween restarts when the panel pops out,
   which is stated rather than fixed. Exit ghosts of either keep their
   place through the layer-shaped `Place`, as a float's do.

## Considered options

- **A. A slot the panel declares, filled by an extension only.** Half
  of what is decided here and the cheapest core change (~150 lines).
  Declined alone because it fails "bindings first": a Node app cannot
  be an extension, and a Rust app would have to split its inspector
  into a separate object and share the editor's state through
  `Rc<RefCell>`. The tree-sitter case is the app's own state, and the
  tab belongs beside it.

- **B. A `devtoolsTab` element whose children are hoisted into the
  panel.** The other half, and where decision 2's layer comes from.
  Declined alone because an extension could not provide a tab, and the
  hoist would be a special case of the panel rather than a slot any
  declarer can mount.

- **The first draft: `provide_slot`, "a host provides the content of
  any slot".** Generalised B into a second party for every slot, so
  that a tab was a slot for both. Cut, because it collides with ADR
  0014's first decision: *a slot is not a node*. A host-declared
  `ui.slot("fs/panel")` has no rect to anchor a layer to — the nearest
  is the enclosing node, and a layer over the status bar is not "between
  the left and right items" of it. The two parties in "exactly one
  party fills a slot" would then lay out differently for the same slot:
  an extension in flow at the position, a host floating over the
  container. Filling in place instead would need the content at
  declaration time, which declare-first-provide-after cannot give, and
  provide-first would reparent a built preorder subtree; the data
  bindings could look ahead in `lower`, Rust could store the closure, C
  could not — three answers per binding, which is a smell and not a
  design. What the tab actually relied on was that *its body is a
  node*, which is unique to the panel; so the host's half is a form of
  the tab and not a verb on slots, ADR 0014 stays as written, and a
  host overriding a plugin's panel — which nobody has asked for — is a
  separate, small ADR about a slot declared *as* a node.

- **C. A second view call** (`view` with `window_name ==
  "kui-devtools/syntax"`, or a registered per-tab closure the core
  keeps). Would need a nested frame inside `finish`, and in Node a
  second bridge crossing per frame while the tab shows; it does not fit
  one tree per window per frame, which the encoder, the digest and the
  exit buffer swap all assume. Rejected — the function child *is* the
  per-tab closure, but run by the encoder on the app's side of the
  bridge, inside the one tree.

- **D. Nothing in the core.** An app can draw its own inspector as a
  `focusRegion` pane in its own tree today. What it loses is the strip,
  the placement, the resize handle, the picker's selection — the
  reasons to be *in* the panel. Kept as the answer for an app that does
  not want those.

- **In-place fill for an extension's tab, a layer only for the host's.**
  Declined: it makes the left dock a special case (no filler at
  `begin_frame`), the panel's window another (no filler in its frame),
  and gives the two parties different focus, clip and ghost paths for
  the same tab. One mechanism, every placement.

- **A stored closure for the Rust host form** (run at mount time
  whatever the order). Not needed once the rule is answerable at view
  time from the session's state; the ordering problem the first draft
  had ("declared after its provision") does not exist for a tab, whose
  declarer is always the panel.

- **A reading instead of a function child in Node and Lua**
  (`devtoolsTabShown(name)` before building the subtree). Declined in
  favour of the function child: the reading puts the laziness in the
  app's hands and the subtree in the tree either way when the app
  forgets; the function child puts it in the encoder's, once, and the
  shape says what it is.

## Consequences

- The core gains one anchor kind (`FloatAnchor::Node`), one declaration
  with two forms (`devtools_tab` / `devtools_tab_with`), two warnings
  (`duplicate-tab`, `bad-devtools-tab`), one frame-order change
  (devtools before the filler's finish), a rule for devtools-declared
  slots in `Extensions::route` and the `unknown-slot` check, and four
  doors (three readers, one writer), each in four bindings and in
  `schema::DOORS` / `ELEMENTS`. Rough size: 350–500 lines in the core,
  a schema row per element, the encoder's function-child case and the
  per-frame reading it needs, the Lua converter's `view` case, the
  generated `d.ts` and `props.md`, and a `runtime/devtools` test that
  reads the painted frame through `nodes()` (no corpus scene has a dock
  in its tree today — F43's note — and a scene would first need the
  corpus to learn to turn the panel on).

- ADR 0014 is not amended. Its decisions 1, 4 and 5 stand verbatim; the
  panel is one more declarer of ordinary slots, with the two rules in
  decision 5 here about *known* and *replies*. Lua's `fill` trap does
  not grow.

- Every tab's content is a layer. That is the right thing for clipping,
  focus and paint, and it means a tab's content cannot be found by
  walking the body's children — the tree tab shows the body and, under
  it, a row that names the layer, the way it shows a float.

- The host form costs nothing while the tab is not shown, in every
  binding: the closure does not run, the open answers false, the
  function child is not called. Off, an app that declares no tab pays
  one flag per frame.

- The one visible limit is the host form in `window` placement
  (decision 6). It is stated in the panel rather than silently empty.

## Open questions

- Whether a tab's label should come from the extension when the
  extension is the one that fills it (a plugin knows what it is called)
  rather than from whoever declared the tab. As decided, the declarer
  names it; a plugin that wants its own name declares its own tab from
  its fill, and `duplicate-tab` settles a clash in the host's favour.

- What `env` the Lua `view` function gets: the same table the view got
  (decided above), or one with the facts of decision 4 merged in as
  `env.devtools`. The doors are on `env` either way; merging is sugar.

- Whether the Node encoder should pass the facts to the function child
  as its argument — `{() => …}` becoming `{(facts) => …}` — for the
  same reason. Cheap, and it keeps a reader from calling a door in the
  middle of a render; leaning yes.

## Action items

- [x] `FloatAnchor::Node(Key)` — layout against a named node's rect,
  positioned over it, clipped to it, focus region of the anchor. (No
  corpus case: a corpus scene cannot turn the panel on, and nothing but
  the panel builds one; `tests/devtools_tab.rs` reads the painted frame.)
- [x] `devtools_tab` (extension form) and `devtools_tab_with` / the
  function child / `view` / `kui_devtools_tab_open` (host form) in four
  bindings; the strip, `N`, `duplicate-tab`, `bad-devtools-tab`.
- [x] The tab body; the mount filled from `Ui::finish` as a layer; the
  *known* and *replies* rules.
- [x] The encoder: the function child, the per-frame reading from the
  driver (`KuiWindow` and `Ctx.frame`), the refusal of a non-function
  child. The Lua converter: `view`, the same refusal.
- [ ] The devtools window's frame `with_filler` (the runner's pane; the
  core's half — `devtools_fill_mount` after the panel's build in that
  window — is in). "Docked only" in the body for a host form there: in.
- [x] The doors (`devtools_selected` / `hovered` / `picked`,
  `set_devtools_selected` with reveal, and — added while building —
  `set_devtools_pick` / `devtools_picking`) in four bindings;
  `schema::DOORS` rows.
- [x] `examples/rust/features/devtools_tab.rs` and
  `examples/node/features/devtools_tab.tsx`, the Inspector in both;
  the extension form is exercised by `tests/devtools_tab.rs` and the C
  surface walk rather than the C panel plugin.
- [x] `CHANGELOG`, `README`, ADR 0014's pointer.

## What the building changed

Built 2026-09-15, the day it was written, in the core, the three
bindings, the harness and two examples. What differs from the text above:

1. **The picker is a door too** (decision 4 gained `set_devtools_pick`
   / `devtools_picking`). The tree-sitter case wants to *ask* "which
   node?" from its tab, not only read what the panel has. Raised while
   a declared tab is on show, the pick lands in `selected` and the tab
   stays up (`State::pick_keep_tab`); raised otherwise it is the chord's
   pick and shows the tree tab, as before. A press while picking is the
   picker's whichever way it was raised.

2. **The mount is filled before the filler's `finish`, not after the
   panel's build.** Decision 5 said "devtools before the filler's
   finish"; that would have put the `ns/root` fills after the dock in
   the root row, to the right of a right dock. The order stands as it
   was — the host's tree, the root fills, then the panel — and the
   extension form's slot is declared and filled *first*, from `Ui::finish`,
   as a float anchored by key to a body the panel builds afterwards.
   That is what the layer was for, and it also answers the *known* rule
   for the current tab by declaring it for real; `slot_declared` counts
   a tab's slot for the tabs not on show.

3. **The strip's tab keys, the body's label.** A declared tab's strip
   entry is keyed `kui-devtools/tab-custom:<name>` and posts
   `tab:custom:<name>`; the body is `kui-devtools/tab/<name>` for
   `key_of`, with the declaration's label as its accessible name. The
   `Shown` enum (`Builtin(Tab)` / `Custom(index)`) is what the strip, the
   body and `dt_inspect` read; `State::custom` holds the name, so a tab
   gone from the list falls back to `tab`.

4. **The laziness rule is one function.** `Core::devtools_tab_shown`
   (on, docked, main window, `custom == name`) is what the Rust closure,
   the C open, and — through `devtools_shown_tab` — the Node driver and
   the Lua converter ask. Node and Lua decide on their side and hand the
   core the content through `Ui::devtools_tab_declared`, which builds
   whether or not the core agrees (a content whose body was not built
   anchors to nothing and paints nothing; a duplicate name builds into a
   node of no size), so a stream or a table stays in step with what the
   binding decided a moment earlier.

5. **The ring.** `focus_ring` skips a node-anchored float's subtree in
   its range walk and walks it after the range when the float's region
   (through `Tree::region_parent`, which jumps from such a float to its
   anchor) is the ring's — so the content comes after the panel's own
   stops, and never into the app's ring. `region_of` uses the same jump.

6. **`nodes()` and the declaration's label.** The Lua `view` function
   takes no arguments and closes over the script's `env` (decision 1
   said "with the same env the view got"; threading it through the
   converter bought nothing). Node's function child takes none either,
   and reads `win.devtoolsSelected()` from its closure.

7. **Not built.** The runner's pane for the panel's own window does
   not yet build that frame `with_filler`, so an extension-form tab
   shows its body empty there; the core's half is in place and the
   pane's is a follow-up. No `layers` corpus case, for the reason the
   action item states.

8. **From the review after the build.** `dt_tabs` is cleared at
   `begin_frame` (it was drained only by the main window's `after_frame`,
   which returns early with the panel off, so a per-frame declaration
   piled up into `duplicate-tab` from the second frame); a cancelled tab
   pick clears `pick_keep_tab`, so the next chord pick reveals in the
   tree tab as before; and the laziness rule (`devtools_tab_shown`,
   `devtools_shown_tab`) checks `custom` names a tab the panel *lists*,
   the way `shown()` does, so a name the app stopped declaring builds
   nothing while the panel remembers it.

9. **The tab is a door too** (backlog F67, 2026-09-16, from kawoosh's
   Syntax tab). Decision 4 gave a tab every door *in* and none that
   *shows* it: an app's own command (`:syntax_tree`) had only the
   strip's click and `Ctrl+Shift+N`, both the user's. `set_devtools_tab
   (name)` in the core, Node and C makes the strip's two writes —
   `custom = name` for a declared tab, `show(Tab)` for one of the
   panel's own — reachable from outside: a declared name the panel does
   not list yet is kept and shows once a frame declares it (the command
   works before the first frame; the return says whether the panel
   lists it now), a hidden dock comes back as the picker's does, `on`
   stays `set_devtools`'s. `devtools_current_tab()` reads the selection
   back by the same names — the strip's own reading, where
   `devtools_shown_tab` is the encoder's. Lua stays a guest.

Traps met: the *fit* passes needed a range parameter to run over a
subtree, and `positions` too, which the scroll-into-view relayout also
calls (`layout::reposition` keeps the anchored floats with their
anchors); `open_with_key` registers no label, so the body needed
`open_with_key_named` to be found by `key_of`; a Node example's tab
content below a 360 px window's fold cannot be clicked, which is
scrolling working, not the tab.
