---
status: accepted
date: 2026-09-04
amended: 2026-09-05
---

# Multi-window: a `Core` per window, and a window set the app declares

kui is single-window by assumption, not by design: `runWindowed` says "One
window per process" in a doc comment, `kui_run` takes one app, and
`WindowCommand` (`crates/kui-core/src/window.rs:41`) covers drag, close,
minimize and maximize and nothing else. A native menu, a dropdown taller
than the window and a tear-off panel each need a second OS surface, so
each is unbuildable. We decided that kui **is** multi-window, that a
window is a **`Core`** (not a root inside a shared one), that the shared
caches move behind a `Session` while every per-frame singleton stays
per-window, that a window's **existence is declared** and diffed the way
`window_title` and `modal` already are while transient acts
(`SetSize`, `Focus`, `Minimize`) stay commands, that `UiEvent` gains a
`window` so one app keeps one `update`, and that an OS popup is a
**window kind** — borderless, non-activating, dismissed by the same
`{kind:"dismiss", reason}` ADR 0003 gave a modal. Nothing is implemented
here; the point is that the next twelve API additions stop assuming one
window.

**Amended 2026-09-04**, before any of it was built, to close the two cases
it left undefined: decision 4 now says which config an `Open` carries when
two frames declare one name differently (read on the opening edge, lowest
declaring `WindowId` wins, `duplicate-window-config` on a real conflict),
and decision 6 names the trap its edge rule creates with a
`window-declared-while-closed` warning.

**Amended 2026-09-05**, by building step 3, in three places the build
settled differently from the text. Decision 3 said the *driver* assigns a
`WindowId`; the core's diff does, because it is the diff that emits the
`Open` and nothing above it exists yet — the driver reads the id off the
command. Decision 6's `{kind:"window"}` event is raised at both edges of
the diff (an app-initiated close reports too), carries `name` and `id` in
its payload, and its `window` field is the core that noticed, since the
diff runs on whichever core finished its frame. And decision 12's
`kui_take_window_commands` became `kui_take_window_command`, one
`KuiWindowCommand` [out] struct per call under ADR 0006's size handshake,
because an array of [out] structs cannot have one (0006, decision 5).

**Amended again 2026-09-04**, by building step 1: decision 2's list of four
shared things is a list of three. `Resources`, `AudioStore` and the font
database move into the `Session`; the shaped-text cache and the glyph atlas
stay per window, because they are one unit with the window's texture and
because `Core::output` lends the atlas out as `&mut`. Hoisting them is a
source break and belongs with the breaks step 3 already carries.

**Amended 2026-09-05** (backlog S5): the "handles that silently belong to
the wrong slotmap" risk the rejected alternative names was not removed by
the `Session`, only moved — two sessions in one process each minted their
own keys from zero, so a handle from one resolved in the other to whatever
it registered first. Handles are now unique to the process (one minting
slotmap per kind, recording the owning `SessionId`; a session's registry is
secondary to it), so a handle used in the wrong session is a miss that
behaves as a removed handle and raises `foreign-resource`. No ABI change:
the `u64` form is untouched.

## Context

- The assumption is written down in four places and true in all of them:
  `packages/kui/index.js:40` ("One window per process (winit event loops
  are not recreatable everywhere)"), `crates/kui-node/src/lib.rs:1117`
  (the same sentence on `KuiWindow`), `crates/kui/src/lib.rs:197` (on
  `Launcher::open`), and `kui_run` (`crates/kui-ffi/src/lib.rs:2675`),
  which takes one title and one pair of callbacks. Only the first of
  those is a real platform constraint, and it is about the **event
  loop**, not the window.
- **`Core` is already per-window in everything but name.** It owns
  `viewport` and `scale` (`crates/kui-core/src/runtime.rs:87`), `env` —
  which is entirely window facts: `focused`, `refresh_hz`, and
  `WindowEnv`'s `custom_chrome` / `maximized` / `fullscreen` /
  `native_controls` — plus one `focus`, one `focus_visible`, one `modal`
  scope, one `ime_rect`, one `window_title`, one hit list, one
  `DisplayList`, and the retained per-key stores (`ScrollStore`,
  `EditStore`, `AnimStore`, `Interaction`'s hover / press / drag
  capture). Every one of those is singular *per window*, not per app.
- The platform layers below it are per-window too. `access_bridge::Bridge`
  wraps an `accesskit_winit::Adapter` built from a window
  (`crates/kui/src/access_bridge.rs:45`), and an `AccessTree`'s root
  *is* the window. `windows_nc::NcHitTest` answers `WM_NCHITTEST` for one
  HWND from one frame's chrome regions.
- What is **not** per-window is exactly the expensive half.
  `Resources` hands out slotmap handles for fonts, images and sounds
  (`crates/kui-core/src/resources.rs`), and an `ImageId` registered while
  the main window was up has to draw in a palette window or the app has
  to register every asset N times. `TextSystem`'s shaping caches and
  `GlyphAtlas` would be duplicated per window — a second atlas texture
  and a second shaping pass over the same strings. `AudioStore` drives
  one device per process, not one per window.
- `kui-wgpu` builds its own `wgpu::Instance` and `Device` per renderer
  (`crates/kui-wgpu/src/lib.rs:109`). N windows today would mean N GPU
  devices, which is not how any of the three platforms want to be driven.
- **The declarative machinery for this already exists.** `window_title`
  is immediate-mode: cleared each `begin_frame`, and "the driver diffs
  and applies" (`crates/kui-core/src/runtime.rs:69`). `declared_focus` /
  `declared_focus_last` implement an *edge-triggered* declaration, so a
  repeated declaration does not clobber a Tab press. ADR 0003 decided
  that the core closes no modal — the app stops declaring the node. A
  window set is the same shape one level up.
- **The imperative machinery exists too, and is already the shape for
  transient acts.** `Core::reveal` (`runtime.rs:1176`) and `Core::play`
  (`runtime.rs:1389`) queue a request that the next frame or the driver
  applies; `take_window_commands` (`runtime.rs:1352`) is drained by the
  frame driver and ignored by headless ones ("the core never touches a
  window — headless drivers just never drain",
  `crates/kui-core/src/window.rs:5`).
- `UiEvent` is `{origin, key, payload}` (`crates/kui-core/src/input.rs:324`).
  `origin` is an `OriginId` — "which frontend produced a node: 0 is the
  host app, extensions get 1+" (`tree.rs:11`). It answers a different
  question than "which window": a Lua extension can draw into every
  window kui opens, and the host draws into all of them too.
- `FloatConfig::fit` (`crates/kui-core/src/spec.rs:161`) mirrors a float
  across its anchor and then clamps it into the viewport. That is the
  right answer for a tooltip and for most menus, and it is a *clamp*: a
  dropdown taller than the window, a menu opened near the window's edge
  with nowhere in-window to go, and a panel the user wants beside the
  app rather than inside it are the three cases it cannot approximate.
- ADR 0003 shipped the in-window half of all of this: a floated subtree
  with `modal` on it contains Tab, blocks the app behind it, reports
  `aria-modal` and emits `{kind:"dismiss", reason:"escape"|"outside"}`.
  With `onContextMenu` (C2) a context menu is already a tag plus a
  `modal` float. So the question this ADR answers is not "how do menus
  work" — it is "what happens when a menu has to leave the window", and
  the answer should cost an app as close to nothing as possible.
- `on_layout` already reports a node's rect as an event, so an app can
  know where its combobox field is without a geometry query API. That is
  the anchor a popup window needs.

## Decision

1. **kui is multi-window, and a window is a `Core`.** One `Core`, one
   viewport, one scale, one `Env`, one focus, one modal scope, one hit
   list, one display list, one access tree, one surface. Nothing in the
   per-frame code learns a window index, because the window *is* the
   index: `finish_frame`, `focus_ring`, `hit_at`, the scroll stores and
   the modal scope stay exactly as ADRs 0002 and 0003 left them. Keys
   are content-addressed path hashes (`crates/kui-core/src/key.rs`), so
   two windows holding equal keys is not a collision — their stores are
   different stores.
2. **A `Session` owns what is shared; `Core::new()` keeps working.** The
   four things that must not be duplicated — `Resources`, `TextSystem`,
   `GlyphAtlas`, `AudioStore` (with the process's one audio device) —
   move behind a `Session` a `Core` is constructed against
   (`Core::new_in(&Session)`); `Core::new()` stays as sugar for a private
   session of one, so every headless test, the conformance corpus,
   `createApp` and `Ctx` are untouched by this ADR. A font or image
   registered anywhere in a session draws everywhere in it. Each
   renderer keeps its own `atlas_epoch` against the shared atlas — the
   comparison in `sync_atlas` already works that way — and `kui-wgpu`
   grows a shared `Device`/`Queue` with a surface per window.
   **Amended (2026-09-04), by building it: three move, two stay.**
   `Resources`, `AudioStore` and the **font database** are the session's;
   the **shaped-text cache** and the **glyph atlas** stay per window. The
   list of four was written from what *looks* shareable rather than from
   what the code says, and the code says two things:
   - **The shaped-text cache and the atlas are one unit, and the unit is a
     window's texture.** A `CachedText` holds positioned glyph quads
     stamped with the atlas epoch they were packed against
     (`glyphs_built_for`), so an entry is only valid for the page it was
     built from. Sharing the cache without the page hands one window UVs
     into another window's atlas.
   - **The atlas cannot move, because `Core::output` lends it.**
     `output` returns `(&DisplayList, &mut GlyphAtlas)`; from a shared
     `RefCell` the second half can only be a guard, and a guard has a
     destructor, so the borrow it holds no longer ends at its last use the
     way an `&mut` does. Every caller that binds the pair and then touches
     the core again breaks — `crates/kui-core/tests/subpixel.rs` does
     exactly that. `Core.atlas.epoch` and `.size`, read as plain fields by
     tests and by renderers, have the same problem from the other side.
     Hoisting the atlas is therefore a **source break**, and belongs with
     the other breaks this ADR schedules (decision 5, step 3), not in a
     step whose whole claim is that it is invisible.

   What is lost by leaving them is memory and repeated rasterization: two
   windows each keep a page and re-raster the glyphs they both draw.
   What is kept is everything the ADR's own sentence promises — "a font or
   image registered anywhere in a session draws everywhere in it" — because
   the things whose *handles must agree across windows* are exactly the
   three that moved: a `FontId` resolves against one font database, an
   `ImageId` / `SoundId` against one registry, and one audio device drains
   one queue. `TextSystem` is split rather than moved: the font database is
   the session's and is passed into shaping as a parameter; the cache, the
   rasterizer and the per-frame text list (which `TextId` indexes, and which
   a second window's `begin_frame` would otherwise clear under the first)
   stay the window's, beside its tree and its display list.
3. **`WindowId` is an opaque integer, assigned by the driver;
   `WindowId::MAIN` is 0.** It crosses every transport as a plain
   integer, and `WindowEnv` gains `id`, so a view can read which window
   it is drawing without a new query. Apps do not construct one.
4. **A window's existence is declared; the core diffs the set.** A frame
   declares windows the way it declares a title — `ui.window(name, cfg)`
   in the imperative bindings, a `windows` list beside the view in the
   returned-tree bindings — and the core keeps `declared_windows` /
   `declared_windows_last` exactly as it keeps `declared_focus`. The
   set in effect is the **union of what every live window's frame
   declared this frame**, plus the main window, which the launcher opens
   and which is always live. Union rather than "the main window's frame
   is authoritative": it is order-independent, it needs no rule about
   which frame wins, and it lets a popup declare its own submenu.
   A window nobody declares any more closes. The app names windows with
   a **stable string**, the identity mechanism the whole library already
   uses, so there is no asynchronous handshake in which the window
   exists before the app has a way to refer to it.
   **Amended (2026-09-04): existence unions, configuration does not.**
   The union makes *existence* order-independent and does nothing at all
   for *configuration*. Two live windows can each declare `"palette"` with
   a different `WindowKind`, initial size or `activates`, and "whichever
   frame ran last" is exactly the non-determinism the union was chosen to
   avoid. Three rules settle it, none of which needs a frame order:
   - **A config is read only on the frame its declaration starts.** That
     is decision 6's edge, applied to the config and not just to the
     existence: the union is diffed, and the config that reaches
     `WindowCommand::Open` is the one carried by the declaration on the
     opening edge. A live window's config is never re-read, so a frame
     that re-declares `"palette"` at 400×300 cannot resize it — decision
     5's rule that the user owns geometry once the window exists, falling
     out of the diff instead of being enforced beside it. This narrows the
     conflict to one frame per window rather than removing it, which is
     why the next two rules exist.
   - **On that edge, the lowest declaring `WindowId` wins.** Ids are
     assigned by the driver before any of the frame's views run, so the
     live set and its ids are fixed for the whole frame: comparing them is
     order-independent in the way comparing execution order is not.
     `WindowId::MAIN` is 0, so the main window's config wins whenever the
     main window declares — the answer an app would guess. Within a single
     window's frame, where the same name is declared twice, the *first*
     declaration wins; that one is well-defined without a rule about
     frames, because one frame is sequential code. The winning declaration
     is also the one whose `OriginId` decision 8 puts on the `Open`.
   - **A genuine conflict is a warning**, `duplicate-window-config`,
     raised by the core's diff and keyed by the window name — there is no
     node to hang it on, so it is keyed the way `unknown-prop` is. A
     config is plain data by decision 5, so equality is derived: two
     windows declaring `"palette"` *identically* — the ordinary case for
     an app that declares one palette from wherever it happens to need it
     — is not a conflict and says nothing. Only differing configs warn,
     and the pick is still deterministic, so the warning reports a
     latent bug rather than deciding anything.
5. **Transient acts stay commands, and every command stays `Copy`.**
   `WindowCommand` grows `Open { id, config }` and `Close(WindowId)` —
   emitted by the core's diff of the declared set, into the queue every
   driver already drains — plus `SetSize { window, size }` and
   `Focus(WindowId)`, queued by `Core::set_window_size` /
   `Core::focus_window` the way `reveal` and `play` queue theirs.
   `StartDrag`, `Minimize` and `ToggleMaximize` gain a `WindowId`; the
   chrome close button emits `Close(self)`. Size is a command and not a
   declared row because **the user owns a window's size once it exists**
   — a declared size would fight every drag of the window's edge, every
   frame. Existence is the app's; geometry, after the initial config, is
   the user's and the OS's.
   **An `Open` carries no string.** The name the app declared is core
   identity — the driver never needs it, since it gets the `WindowId` the
   core assigned — and the window's *title* arrives the way every title
   already does: `window_title`, declared by the new window's own first
   frame and diffed by the driver (`crates/kui-core/src/runtime.rs:69`).
   So the config is an id, a kind, an initial size and `activates`: all
   plain data. That keeps `WindowCommand` `Copy` and plain-old-data,
   keeps a borrowed string lifetime out of the FFI drain, and leaves one
   mechanism owning window titles instead of two that can disagree. The
   cost is that a new window is untitled for the one frame before its
   first `window_title` lands, which is a frame the OS spends mapping the
   surface anyway.
6. **An OS close is reported, and the declaration is edge-triggered.**
   Closing a window from its titlebar or from the OS emits
   `{kind:"window", phase:"closed"}` and closes it; the driver does not
   reopen it merely because the app's next frame still declares it.
   A window reopens only when its declaration **starts** again — the
   same edge rule `set_key_focus` already uses, and the reason a
   repeated declaration cannot clobber the user's decision. `phase:
   "opened"` is reported likewise, so an app can seed a model when a
   window appears.
   **Amended (2026-09-04): the trap this creates is named by a
   diagnostic.** Put beside the union of decision 4, the edge rule has a
   consequence worth writing down, because the first version of any app
   walks straight into it: a frame that declares a window
   **unconditionally** can never reopen it once the user closes it. The
   declaration never stops, so it never starts; the window stays shut
   while the app asks for it every frame. To get it back the app has to
   observe `{kind:"window", phase:"closed"}` and thread "is it open"
   through its model, exactly as ADR 0003 makes an app thread "is the
   dialog up" — this is "the core closes no modal" one level up, and the
   alternative (reopen on the next frame that still declares) is a close
   button that does nothing. So the rule stays, and the mistake gets a
   name: **`window-declared-while-closed`**, a warning raised when a name
   is still in the declared union on the frame after the OS close it
   never lapsed across. It sits next to `modal-behind-content` for the
   same reason that one exists — a silent misconfiguration that reads
   from outside as the feature being broken — and its message says the
   fix rather than the symptom: handle the `closed` event, stop declaring
   the name, declare it again to reopen. Like
   `duplicate-window-config` it comes from the declaration diff and not
   from `diag`'s tree walk, and it is keyed by the window name, since
   there is no node; it is raised on the core whose frame declared it,
   so it inherits the once-per-`(code, key)`-per-core dedup and costs one
   line however many frames the app keeps asking.
7. **`UiEvent` gains `window`; there is one queue per `Core` and one
   `on_event` per app.** Each `Core`'s `handle_input` already returns its
   own events; the driver stamps each with the window it came from and
   merges them into the app's single handler. One app has one model and
   one `update`, and an app that had to poll N queues would route by hand
   anyway — with every binding first growing an N-queue API. `window` is
   a new field, not a reuse of `origin`: origin says which *frontend*
   drew the node, and an extension draws into every window. For a
   single-window app the field is always 0, so nothing existing changes
   meaning.
8. **`Open` carries the declaring `OriginId`.** An extension is a
   frontend hosted inside a window (`Extension`, `runtime.rs:2440`);
   letting it open OS surfaces is a real capability, so the command says
   who asked and a host can refuse. One field, and it keeps the extension
   boundary as honest as origins already keep the node boundary.
9. **A popup is a window kind, and it reuses `dismiss`.**
   `WindowKind::{Normal, Popup}`. A `Popup` is borderless (`Chrome::Borderless`
   already exists — it is not a new chrome concept), owned by the window
   that declared it, absent from the taskbar, positioned in screen
   coordinates against an anchor rect the app already gets from
   `on_layout`, closed when its owner closes, and **non-activating** by
   default (`config.activates`): it must not take OS focus, or opening a
   combobox would blur the field that opened it. While a non-activating
   popup is open, the driver routes the owner's keyboard input to it, and
   the owner's `env.focused` stays true — so the field still draws
   focused while arrow keys walk the list. A press outside the popup or
   Escape emits `{kind:"dismiss", reason:"outside"|"escape"}` — **the
   same event ADR 0003 defined**, on the window rather than on a node —
   and closes nothing by itself: the app stops declaring the window, on
   the frame it decides to, for the reasons decision 6 of ADR 0003 gives
   verbatim. An app graduating a dropdown from a `modal` float to a popup
   window changes its declaration and not its handler.
10. **Modality is per-window.** A modal in window W confines W's Tab ring
    and W's hit list and says nothing about any other window. It cannot
    do otherwise — inertness is "emit no hit region", and another window
    has another hit list — and it should not: a modal that freezes every
    window of the app is a hung app to everyone outside it, which is the
    same reasoning ADR 0003 used to keep window chrome live under a
    modal. A popup owned by a modal dialog is therefore live, which
    extends ADR 0003's "the scope is a tree range, so a dialog's own
    dropdown stays live" to the case where the dropdown is its own
    surface. **App-modal and owner-modal windows are out of scope for
    v0** and named as such: they are an OS-level window attribute, not a
    core concept, and nothing we want to build needs them before the rest
    of this exists.
11. **Floats stay the default; `FloatConfig::fit` is the in-window
    approximation and now says so.** A float costs one frame, one tree,
    one hit list and one draw call, and it can be asserted headlessly; a
    popup window costs an OS surface, a swapchain, a `Core` and an access
    adapter, and cannot be. So `fit` is not deprecated by this ADR — it
    is what a tooltip, a context menu and a dropdown should use until
    they provably do not fit, and its doc comment gains the sentence
    saying where that limit is and what to reach for past it.
12. **The four entry points do not change shape.**
    - **Rust**: `kui::app(title).run(app)` still opens the main window and
      runs the loop, and `App::view(&mut self, ui)` still has one
      signature — it is simply called once per live window per frame,
      with `ui.window()` naming which. Extra windows are a declaration
      inside `view`, so a single-window app is byte-for-byte unchanged.
    - **C**: `kui_run(title, view, on_event, user)` is unchanged; a
      `KuiCtx` *is* the window's core, so the view callback already
      receives the right thing, and `kui_ctx_window(ctx)` names it.
      `kui_window_declare(ctx, name, cfg)` declares. Two breaks, both
      named, and neither of them silent. **`kui_take_window_commands`
      cannot stay a `uint32_t` array** — a command now carries a
      `window` beside its verb, and an `Open` a config — so it becomes a
      `KuiWindowCommand` out-param, a flat `repr(C)` struct with no
      pointers in it by decision 5. That is a *source* break: a host's
      `uint32_t cmds[8]` becomes `KuiWindowCommand cmds[8]` and its
      `switch (cmds[i])` becomes `switch (cmds[i].kind)` applied to
      `cmds[i].window`. **`KuiEvent` gains an appended `window`** —
      appended, the way `KuiSpec.tooltip` was, so no existing field
      moves and no existing line of C changes meaning. That one is a
      *source-compatible ABI* break: `KuiEvent` is caller-allocated
      (`kui_poll_event(ctx, &ev)`), so a host built against the old
      header reserves the old size and a newer library writes past it.
      Every C host recompiles; none of them edits that loop. P6's
      `abi_parity` static asserts catch header-vs-Rust drift at build
      time, but nothing catches an old binary against a new library —
      the ABI has no version negotiation, and this ADR does not add one.
      **Closed by [ADR 0006](0006-c-abi-versioning.md)** (2026-09-04),
      before any of this was built: `kui_abi_version()` versions the ABI,
      and `KuiEvent` now leads with a `size` the host sets, so the library
      writes no further than the host reserved. The appended `window`
      still bumps the version, but it no longer writes past an
      un-recompiled host's struct.
    - **Node**: `runWindowed` keeps returning **one** promise, resolving
      with the final model when the **main** window closes; its doc line
      changes from "one window per process" to "one event **loop** per
      process", which is the part that was ever true. `view(model)`
      becomes `view(model, window)` with `window` defaulting to `'main'`,
      and an optional `windows: (model) => [...]` declares the set —
      both additive, so every existing app keeps working untouched.
    - **Lua**: kui-lua is a `Core` wrapper with no runner, so it gains
      only an optional `windows` key beside `window_title` at the root of
      the returned tree; the embedding host applies the commands it
      drains, subject to decision 8.
13. **Window content is never nested in another window's tree.** No
    `<window>` node. A tree is laid out against one viewport, and a
    lifted subtree would need excluding from layout, hit testing, the
    access tree, the Tab ring and the modal scope in the host `Core` —
    five special cases in the hottest code — while its retained per-key
    state sat in the wrong store.

## Considered options

- **Stay single-window; every popup is a float.** Rejected: `fit` clamps,
  and three cases cannot be clamped into existence — a dropdown taller
  than the window, a menu opened where no in-window placement fits, and a
  panel the user wants *beside* the app. Every platform's users expect
  all three, and an app cannot work around a missing OS surface.
- **One `Core`, many roots.** Rejected: `focus`, `focus_visible`, the
  modal scope and its remembered focus, hover / press / drag capture,
  `viewport`, `scale`, `env`, `ime_rect`, `window_title`, the hit list
  and the display list would each become a map keyed by window. That is
  Core-per-window spelled worse, and it puts a window lookup in the
  per-frame path that ADR 0002's ring, ADR 0003's scope and every
  emission loop walk. Cheap to build, expensive forever.
- **A `<window>` element, so windows nest in the tree.** Tempting — one
  lowering pass, and JSX would get it free. Rejected for decision 13's
  reasons: it buys ergonomics in the bindings by paying for it in the
  five hottest passes in the core, and it puts window B's retained state
  in window A's stores.
- **Imperative `open_window()` returning an id asynchronously.**
  Rejected: the app must then handle a window that exists before it has a
  way to name it, and the id becomes a second source of truth beside
  whatever the app already calls that window — the same objection ADR
  0003 made to an imperative modal stack. A declared name is known the
  frame it is written.
- **Declaring the size every frame along with existence.** Rejected: it
  fights the user. A window the app re-asserts at 800×600 cannot be
  resized, and the workaround — declare the size the user last resized
  to — makes every app mirror window geometry into its model to hand it
  straight back.
- **The first declaration in the frame wins, for a conflicting config**
  (2026-09-04). Rejected: it reads as the simplest rule and is the only
  one here that needs a *defined frame order* — which is precisely what
  the union was chosen not to need, so it would buy a tie-break by
  spending the property the whole decision rests on. Worse, the order it
  would depend on is the driver's, so an app's palette would change kind
  when the driver changed how it walked its windows. Ordering over the
  driver-assigned `WindowId` costs the same and is fixed before any view
  runs.
- **Letting a conflicting config be whatever the last frame declared**
  (2026-09-04). Rejected — it is the status quo of the unamended
  decision 4, and it is non-deterministic in the exact way this ADR
  spent a paragraph avoiding for existence.
- **Making a conflicting config an error rather than a warning**
  (2026-09-04). Rejected: the library has one mechanism for "this is
  silently not what you meant", `take_warnings`, and every other
  misconfiguration in `diag.rs` uses it. A panic or a refused `Open`
  would also make an app's *recovery* from the conflict worse than its
  bug — no window instead of one window with the main frame's config.
- **Reopening a window because the next frame still declares it**
  (2026-09-04). Rejected: it makes the close button do nothing, which is
  worse than the trap it removes — the app that declares
  unconditionally is now unclosable rather than un-reopenable, and there
  is no event an app can handle to fix it. Decision 6's edge rule stands;
  `window-declared-while-closed` is what makes it discoverable.
- **Per-window event queues exposed to the app.** Rejected: one app, one
  model, one `update`. N queues means `pollEvents(windowId)` in Node,
  an N-way callback in C, and an app that fans in by hand at the top of
  its update anyway — which is what stamping the event does for it once,
  in the driver.
- **Folding the window into `OriginId`.** Rejected: origin answers "which
  frontend drew this node" and extensions draw into every window; the two
  would alias the moment a Lua extension ran in two of them.
- **The core closing popups on Escape or an outside press.** Rejected for
  ADR 0003's reason, unchanged by the surface being an OS window: a menu
  that closes and a picker that asks before closing are both legitimate,
  and only the app knows which it is.
- **Popups that take OS focus.** Rejected as the default: it blurs the
  control that opened them, which is visible (the field loses its ring)
  and wrong (the combobox is still the thing being edited). Left as
  `activates: true` for the cases that want it — a tear-off panel is a
  window the user works in.
- **App-modal or owner-modal windows in v0.** Deferred, not rejected:
  they are an OS window attribute the driver would set, they compose with
  everything above, and nothing on the roadmap needs them before
  multi-window exists at all.
- **Duplicating `Resources`, the text caches and the atlas per window.**
  Rejected: a second atlas texture and a second shaping pass over the
  same strings, and an app forced to register every font and image once
  per window — with `ImageId` handles that silently belong to the wrong
  slotmap if it gets it wrong.

## Consequences

- Nothing ships from this ADR, deliberately. `CHANGELOG.md` is untouched:
  the changelog lists what a release adds and what an app can delete, and
  this release adds no behaviour. C7 becomes done — it is now in
  `docs/backlog/closed-2026-09.md` — and the build work it points at
  becomes a new item.
- Two named breaks when it is built, both in C, and neither reaching the
  other three bindings: `kui_take_window_commands` stops being a
  `uint32_t` array (a source break — hosts edit their drain loop), and
  `KuiEvent` gains an appended `window` (source-compatible, but
  caller-allocated, so every C host recompiles). `WindowCommand` stays
  `Copy` and pointer-free, because an `Open` carries no title — see
  decision 5.
- `UiEvent` gains a field in every transport — `window` in the TS message
  types and the Node event object, `KuiEvent.window` in C, a field on the
  Lua event table. It is 0 everywhere today.
- `Core::new()` and every headless path are unchanged, so the conformance
  corpus, `createApp` and every existing test keep running as a session
  of one. What the corpus **can** cover of this is the declaration diff —
  a scene that declares a window, stops declaring it, and asserts the
  `Open`/`Close` command sequence, which all four transports can
  reproduce byte-identically. What it cannot cover is the OS surface: a
  popup window has no headless equivalent, so its behaviour belongs in
  the macOS/Windows smoke jobs (P8) rather than in the corpus.
- Two warnings join `diag.rs`'s codes and the TS warning-code union when
  this is built (2026-09-04): `duplicate-window-config` and
  `window-declared-while-closed`. Neither comes from the frame's tree
  walk — both are raised from the declaration diff, keyed by the window
  name rather than by a node — so `diag`'s module doc, which today says
  "One code does not come from the tree walk" of `unknown-prop`, becomes
  three. Both are also in reach of the conformance corpus, unlike the
  popup half of this ADR: a conflict is two declarations in one frame,
  and the reopen trap is a declaration, an injected close and a frame
  that still declares.
- `kui-wgpu` needs a shared device before a second window is worth
  opening; that is the first piece of implementation work, and it is
  invisible to every binding. Built 2026-09-04 as `kui_wgpu::Gpu`: the
  instance, adapter, device and queue behind one cloneable handle, with
  `Renderer::new_in(&Gpu, target, w, h)` beside the unchanged
  `Renderer::new`, which now makes a private `Gpu` for its own window.
- A suggested build order, each step shippable alone: (1) `Session` —
  hoist `Resources`, `AudioStore` and the font database, and share the wgpu
  device (built 2026-09-04; the glyph atlas and the shaped-text cache stay
  per window — see decision 2's amendment); (2) `WindowId` on `UiEvent`, `WindowEnv::id`, always 0
  — pure plumbing through four transports; (3) the declared set, the
  diff, `Open`/`Close`, and the `window` event, with the runner opening
  real `Normal` windows; (4) `WindowKind::Popup`, anchoring, the
  non-activating key route and `dismiss`; (5) `SetSize` / `Focus` (built
  2026-09-05, before step 4: it depends on nothing step 4 adds, and step 3
  is what makes it necessary rather than merely available — once a config
  is read on the opening edge only, these two verbs are the only way an app
  moves a window that already exists).
- **Amended 2026-09-05, at the alpha.6 release: this ships without
  popups.** Steps 1, 2, 3 and 5 are built; **step 4 is not**, so
  `WindowKind` has exactly one variant, `Normal`, and the release has no
  spelling of a popup anywhere — no `KUI_WINDOW_KIND_POPUP`, no `kind` key
  in a JSX or Lua `windows` entry. That is a deliberate v0 line and not an
  oversight, so it is written here rather than found: what step 4 owes —
  anchoring in screen coordinates, ownership, non-activating focus routing
  and `dismiss` on the window — is exactly what the three cases the Context
  names still need, and all three stay unbuildable. A dropdown taller than
  the window, a menu opened near the edge with nowhere in-window to go, and
  a panel the user wants beside the app are what decision 11 keeps
  `FloatConfig::fit` as the approximation *of* and what it says `fit`
  cannot reach. Everything that *does* fit in the window is covered today,
  and the spelling for it is `fit` plus a `modal` float
  ([ADR 0003](0003-modal-surfaces.md)) — which is where a dropdown or a
  context menu belongs until it provably does not fit. Decision 9 is
  unchanged as a decision — a popup is a window kind and reuses `dismiss`
  — it is simply not built, and graduating a float to one still changes a
  declaration and not a handler when it is.
- **The absent kind is absent loudly** (2026-09-05, with the amendment
  above). Nothing may declare a kind that quietly opens a normal window,
  because that would read as the popup having shipped. JSX and Lua *refuse*
  a `kind` on a `windows` entry — an entry is plain data with a fixed shape
  by decision 5, not a node's loose prop bag, so a key that does nothing is
  an error and not a dropped declaration. C cannot throw across the ABI, so
  `kui_window_declare` warns: `KuiWindowConfig.kind` is a `uint32_t` that a
  host built against a later header can fill with anything, and any value
  but `KUI_WINDOW_KIND_NORMAL` still opens a normal window — degrading to a
  window rather than to nothing — while raising **`unknown-window-kind`**,
  the third code this ADR adds to `diag.rs`. Like the other two it is
  keyed by the window rather than by a node, and like `unknown-prop` it is
  raised by the binding that saw the declaration through `Core::warn` —
  `diag`'s module doc, which step 3 took from three codes outside the tree
  walk to four, now says five.
- Not decided here, and each wanting its own answer when something needs
  it: window position as a declared or reported fact (the app cannot
  currently restore a window where the user left it), multi-monitor and
  per-monitor DPI beyond the scale a `Core` already carries, a native
  menu **bar** (which is a platform resource, not a window, and closer to
  the access tree in shape than to anything here), drag-and-drop between
  windows, and app-modal windows per decision 10.
