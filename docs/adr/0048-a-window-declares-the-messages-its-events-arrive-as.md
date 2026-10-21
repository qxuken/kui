---
status: accepted
date: 2026-10-22
---

# A window declares the messages its events arrive as

> **Accepted and built (2026-10-22); backlog F175.** This amends [ADR
> 0047](0047-an-event-arrives-as-the-message-its-handler-declared.md),
> decision 6. ADR 0047 made every node event arrive as the message its
> handler declared, and left the events with no node to declare a handler
> on as root events, spelled `{kind, …}`. This ADR gives them handlers
> too, declared where the app already declares a window: its own root, its
> entry in the window list, and the menu bar. Every event then arrives as
> a message the app chose, `RootMsg` goes away, and `kind` belongs to the
> app alone.

## Context

ADR 0047 decision 6 kept eight events as root events: `resize`, `window`,
`system`, `fonts`, `modifiers`, `open`, the menu bar's `menu` and a popup
window's `dismiss`. They arrive as `{kind: "resize", width, height,
scale}`, with the core's own `kind`. Two costs came out of building 0047.

- **The namespace collision is still there, for these eight.** Node's
  `update` takes `AppMsg | RootMsg`, so an app's own `{kind: 'menu'}`
  narrows together with the menu bar's `MenuBarMsg`, and an app's
  `{kind: 'open'}` with the OS's documents. The Node examples renamed
  their messages to get out of the way (`openMenu`, `browse`). This is the
  F148 collision that 0047 removed for node events, still present for
  these eight.
- **Two shapes for one rule.** A Rust handler reads `ev.message::<Msg>()`
  for every event but these, and `ev.read()` for these. C and Odin carry a
  `root` flag on every event just to say which shape it is (`KuiEvent.root`).

Each window is its own core with its own tree, and each tree has a root
whether the app configures it or not: Rust's default root is a fit column,
and in JSX and Lua the root is whatever element or table the view returns.
The other windows are entries of a `windows` row, any window's frame may
declare one, and the core diffs the union. The menu bar is a `menuBar`
element. So every one of these events is about something the app already
declares each frame.

## Decision

1. **A window's root carries one boxed group, `windowEvents`.** It is a
   root-only prop, like `title` and `windows`, and holds the messages for
   the events about that window:

   | key | event | data |
   |---|---|---|
   | `onResize` | `resize` | `resize: {width, height, scale}` |
   | `onFocus` | `window` | `window: {phase: "focused" \| "blurred"}`, this window's own keyboard |
   | `onSystemSettingsChange` | `system` | `system: {appearance, accent, motion, locale, assistive}` |
   | `onFonts` | `fonts` | none: the message as declared |
   | `onModifiers` | `modifiers` | `modifiers: {shift, ctrl, alt, super}` |
   | `onOpen` | `open` | `open: {paths}`, read on the main window only |

   - **It is boxed.** The group lives in the frame's window state as an
     `Option<Box<WindowEvents>>`, not on `NodeSpec`. No node's spec, prop
     list or TypeScript type grows by six rows. Inside the group the names
     are scoped, so `onFocus` there cannot be mistaken for the node row
     `onFocus`.
   - **The root is implicit, and the group is optional.** No window
     element wraps a view: each window's tree stands alone, and its root
     is the frame's own. A root that declares no `windowEvents`, or a
     group that leaves a key out, hears none of those events. `env` still
     reads every fact they report.
   - **Off the root, it is ignored.** `windowEvents` on any other node
     raises `window-events-off-root`, as `title` would.
   - **`onOpen` is the main window's.** The OS opens documents to the app,
     not to one of its windows, so the key is read on the main window's
     root alone. Reading it on every root would make the app deduplicate
     the documents.
2. **A declared window's lifecycle lives on its entry.** A `windows` entry
   gains two keys:
   - `onWindow`: its `opened` and `closed` arrive as this message with
     `window: {phase, id}`, on whichever window's frame noticed, as today.
   - `onDismiss`: a popup's dismissal arrives as this message with
     `dismiss: {reason, id}`.

   `name` is gone from both payloads, because the message was declared on
   that window's entry and so names it. The split is deliberate. Lifecycle
   belongs where the app decided the window should exist, and it works
   for a closed window, which has no root left to declare anything.
   Everything else about a window is on its root.
3. **The menu bar declares `onMenu`.** A choice from the application menu
   bar arrives as the `menuBar` element's `onMenu` message with
   `menu: {role, item}`, the same field a context menu's choice carries
   (0047, decision 5). The element is declared whether the bar is drawn in
   the frame or by the platform (ADR 0018), so it has a message to arrive
   as either way.
4. **No handler, no event**, for these as for every other event.
5. **There are no root events left.** These go:
   - `UiEvent::root` and `UiEvent::is_root`;
   - the `root` flag (`KuiEvent.root`, Odin's `Event.root`);
   - Node's `RootMsg` and its eight types.

   Every event is `UiEvent::declared`, and its payload is a map. The core
   never writes a `kind`. The `EVENTS` table's `handler` column names, for
   each kind, the key or row that declares it.
6. **The typed helpers grow by these fields.**
   - TypeScript: `WithResize`, `WithWindow`, `WithSystem`, `WithModifiers`,
     `WithOpen` and `WithDismiss`, plus the existing `WithMenu`; the
     `*Msg<T>` helpers; a `WindowEvents<A>` type for the group, each key
     typed `HandlerMsg<With…>`, and `onFonts` as any message.
   - Odin: the matching `With_*` structs.
   - Lua: the `kui.With…` classes.
   - Rust: `kui::Event` keeps its data types for these kinds. The
     derive's twin takes `on = resize` and the rest, and its refusal of
     root events goes.
7. **One door per binding.**
   - Rust: `Ui::window_events(WindowEvents)` beside `configure_root`, a
     builder with `on_resize`, `on_focus`, `on_system_settings_change`,
     `on_fonts`, `on_modifiers` and `on_open`. `WindowConfig` gains
     `on_window` and `on_dismiss`, and `MenuBar` gains `on_menu`.
   - C: `kui_root_with(ctx, &spec, &events)` with a `KuiWindowEvents` of
     six borrowed messages, so `KuiSpec` does not grow.
     `kui_window_declare_with(…, on_window, on_dismiss)` and
     `kui_menu_bar_with(…, on_menu)` follow `kui_select_with`, and each
     consumes its messages.
   - Odin: generated from those.
   - JSX: `windowEvents={{…}}` on the root box, `onWindow` and `onDismiss`
     on a `windows` entry, `<menuBar onMenu>`.
   - Lua: `window_events = { on_resize = …, … }` on the root table, and
     the same keys in its spelling on entries and the menu bar.
8. **It joins ADR 0047's unreleased versions.** C's ABI 33 and the frame
   protocol v24 are not released yet (alpha.60 is unreleased), so these
   join them, as ADR 0034's did ABI 19. It is a hard break with no shim,
   like 0047's, and the alpha.60 CHANGELOG carries both.

## Considered options

- **Six ordinary root-only rows** (`onResize` and the rest as `PROPS`,
  read on the root alone). The first draft of this ADR. Every node's spec,
  prop list and TypeScript type would carry six rows only a root reads,
  and the window's focus row would share its name with the node's
  `onFocus`. Boxing them costs one prop.
- **A `<window>` element wrapping each view.** It would make the
  declaration point explicit, but it reads as a window nested inside a
  tree, and no window is: each tree stands alone. The root already
  carries the window-wide rows (`title`, `windows`, `alwaysOnTop`,
  `secureInput`), and a wrapper would be a second place for the same
  thing.
- **The lifecycle on the window's own root too.** Everything about a
  window would then be in one place, but a `closed` would have to be
  delivered from the root's last declaration, after its view has stopped
  running.
- **`onSystem`, or `onThemeChange`.** The event carries the appearance and
  the accent, and also reduced motion, the UI language and whether
  assistive technology is listening. `onThemeChange` would file a language
  change under the theme, and `onSystem` names the source, not the change.
  `onSystemSettingsChange` names what it reports.
- **Keep root events, and rename the core's kinds out of the app's way**
  (`kui.resize`, `@menu`). This leaves the two shapes, and it moves the
  collision rather than removing it.
- **One `onRoot` handler for all eight.** It rebuilds the union 0047 took
  apart: every root event would share one message type.

## Consequences

- **The devtools keep hearing their own window.** The panel reads its
  window's `closed` (`devtools_consume`) and must not depend on the app
  declaring `onWindow`. The core routes that one internally, before it
  becomes an event.
- **Every binding's root, window and menu-bar declarations change,** along
  with apps that used these events as a redraw signal. A host that retains
  its tree (Node, C, Lua) re-runs its view only for a message, so an app
  that follows the OS's appearance declares `onSystemSettingsChange`. The
  howto's "follow the OS's dark mode" entry says so.
- **The corpus pins it.** Scenes that close a window, dismiss a popup or
  change the system settings declare the group or the entry keys. A new
  scene declares every key and the menu bar's `onMenu`, and drives each
  event. Every adapter reproduces it.
- **ADR 0047 decision 6 is superseded.** Its building notes about
  `is_root`, `KuiEvent.root` and the TypeScript collision record what this
  ADR removes.

## What the building changed

- **`WindowConfig` stays `Copy`, and the lifecycle messages sit beside
  it** (decision 7). The config is the driver's half — it rides
  `WindowCommand::Open` to every runner — and is read on the opening
  edge alone, where the messages are read on every declaration. So Rust
  has `WindowLifecycle { on_window, on_dismiss }` and
  `Core::declare_window_with(name, config, lifecycle)` / `Ui::window_with`;
  `declare_window` is the one with no messages. The registry keeps each
  open window's last messages and its declaring origin, refreshed by
  every diff, so a `closed` arrives as what was last declared and an
  extension that declared a window hears it.
- **The declaration in force is the last finished frame's.** A window's
  own events are noticed where the message is not known — the router's
  modifier change, the resize a frame begins with — so the core keeps
  them as facts and reads the root's group on the way out, at the two
  exits every event leaves through. A fact noticed before any frame has
  finished is held for the first drain after one has, so the documents a
  launcher hands over before the first frame reach a root that declares
  `onOpen` on it.
- **`open` on another window is dropped**, not routed to the main one:
  the drivers already send it to the main window's core.
- **The devtools hear their own window by their own message.** The
  panel's window declares an `onWindow` the panel takes back, so it
  needs no name match and no app declaration (the first consequence).
- **`kui::Event` follows the data.** `WindowEvent` drops `name` and its
  `id` is `None` for a focus change; `Dismiss.window` is the id alone;
  `Event::System` holds a `SystemEnv`, read by the new
  `SystemEnv::from_value`. The derive's twin takes `on = resize`,
  `window`, `system`, `modifiers` and `open` (`event::Open` is new), and
  refuses `fonts` with the other data-less events. `KeyMods::to_value`
  stopped writing a `kind`.
- **`window-events-off-root` is raised by `Core::open_from`**, the door
  every binding lowers a prop list through; `WindowEvents::from_value`
  and `WindowLifecycle::from_entry` read the group and an entry's keys in
  JSX's spelling or Lua's, so the two parsers cannot disagree, and an
  unknown key in the group is an error.
- **The corpus** gained the steps `viewport W H` and `focused 0|1` and
  the `window-events` scene, which declares every key and the bar's
  `onMenu` and drives each but `fonts`: a rescan that finds what was
  there says nothing, so no headless step makes one (a core test bumps
  the session's revision instead). A named `window` or `dismiss` row
  carries its `phase` or `reason` after the name. The scenes whose
  `modifiers` or `system` rows were incidental lost them.
- **Lua's `ev.window` is the data on a `window` event.** Lua merges the
  envelope into the event's table, and the envelope's window id is named
  `window` — the name of the `window` event's field. The message's field
  wins: on a `window` event `ev.window` is `{phase, id?}` — the `id` of
  the window that opened or closed, none for a focus change, which is
  the receiving window's own — and on every other event it is the
  number it was.
- **C's `kui_root_with` takes a NULL `spec`**, which declares the group
  and leaves the root as the frame began it, since a zeroed `KuiSpec` is
  not the core's default root and a host that never configured its root
  should not have to start. `KuiWindowEvents` is an `[in]` struct of six
  borrowed messages with no `size` field, zeroed for none; Odin's
  `root_with` takes a `Maybe(Spec)` and a hand-written `Window_Events`,
  which the generator cannot lower.
- **Node's encoder sends `windowEvents` from any node**, unlike `title`,
  which it drops off the root, so the core sees a misplaced group and
  warns. The `menuBar` op carries `onMenu` as a string reference after
  its menu, as `select`'s does, a slot inside protocol v24 rather than a
  bump. `activateMenuBarItem` still answers whether anything was posted,
  so a bar with no `onMenu` answers `false` for a real row, whose
  standard role is still performed.
- **The Node examples' harness declares `onSystemSettingsChange`** on
  the root it wraps every example in, and swallows the message, so a
  retained tree keeps following the OS's light and dark without every
  example declaring it; an example that declares the key hears its own.
