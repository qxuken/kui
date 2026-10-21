---
status: proposed
date: 2026-10-21
---

# A window declares the messages its events arrive as

> **Proposed (2026-10-21); backlog F175.** This amends [ADR
> 0047](0047-an-event-arrives-as-the-message-its-handler-declared.md),
> decision 6. ADR 0047 made every node event arrive as the message its
> handler declared, and left the events with no node to declare a handler
> on as root events, spelled `{kind, …}`. This ADR gives those events
> handler rows too, declared where the app already declares its window,
> its other windows and its menu bar. Every event then arrives as a
> message the app chose, `RootMsg` goes away, and `kind` belongs to the
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
  their messages to get out of the way (`openMenu`, `browse`). The
  envelope's `event` tells them apart at run time, but in TypeScript they
  share a union. This is the F148 collision that 0047 removed for node
  events, still present for these eight.
- **Two shapes for one rule.** A Rust handler matches `ev.message::<Msg>()`
  for every event but these, and `ev.read()` for these. C and Odin carry a
  `root` flag on every event just to say which shape it is (`KuiEvent.root`,
  added in 0047's build). Lua builds a table either way.

Every one of these events is about something the app declares each
frame. The window's own root box is declared in every binding (Rust
`Ui::configure_root`, C `kui_root`, the root element in JSX, the root table
in Lua). The other windows are entries of its `windows` row. The menu bar
is a `menuBar` element. A root event has a place to declare a handler
after all.

## Decision

1. **Six handler rows on a window's root**: `onResize`, `onWindow`,
   `onSystem`, `onFonts`, `onModifiers` and `onOpen` (`Kind::Handler`, in
   `schema::PROPS`, read only on the root node — a window's own tree
   root). Each event arrives as that message with its data under its
   name, as 0047's node events do:
   - `resize: {width, height, scale}`
   - `window: {phase: "focused" | "blurred"}`, this window's own keyboard
   - `system: {appearance, accent, motion, locale, assistive}`
   - `fonts`, data-less: the message as declared
   - `modifiers: {shift, ctrl, alt, super}`
   - `open: {paths}`, the OS's documents. This one is the main window's
     root only, since the OS opens documents to the app and not to one of
     its windows.

   A root row declared on any other node does nothing, and raises
   `root-handler-off-root`.
2. **A declared window says its own.** A `windows` entry gains `onWindow`
   and `onDismiss`. Its `opened` and `closed` arrive as its `onWindow`
   message with `window: {phase, id}`, on whichever window's frame noticed,
   as today. A popup's dismissal arrives as its `onDismiss` message with
   `dismiss: {reason, id}`. `name` is gone from both payloads, because the
   message was declared on that window's entry and so already names it.
3. **The menu bar declares `onMenu`.** A choice from the application menu
   bar arrives as the `menuBar` element's `onMenu` message with
   `menu: {role, item}`, the same field a context menu's choice carries
   (0047, decision 5). The declaration exists in the frame whether the bar
   is drawn there or by the platform (ADR 0018), so it has a message to
   arrive as either way.
4. **No handler, no event**, for these as for every other event. An app
   that declares no `onResize` hears no resize. `env` still reads every
   fact these events report, as before.
5. **There are no root events left.** `UiEvent::root`, `UiEvent::is_root`,
   the `root` flag (`KuiEvent.root`, Odin's `Event.root`) and Node's
   `RootMsg` with its eight types go. Every event is `UiEvent::declared`,
   and its payload is a map (0047's building notes). The core never
   writes a `kind`. The `EVENTS` table's `handler` column names a row for
   every kind, and its `root` handler value goes.
6. **The typed helpers grow by these fields.**
   - TypeScript: `WithResize`, `WithWindow`, `WithSystem`, `WithModifiers`,
     `WithOpen` and `WithDismiss`, plus the existing `WithMenu`; the `*Msg<T>`
     helpers; each new row typed as `HandlerMsg<With…>`; `onFonts` as any
     message.
   - Odin: the matching `With_*` structs.
   - Lua: the `kui.With…` classes.
   - Rust: `kui::Event`'s `Resize`, `Window`, `System`, `Fonts`, `Modifiers`,
     `Open`, `Menu` and `Dismiss` keep their data types, read beside the
     message as every other event's are. The derive's twin takes `on =
     resize` and the rest; its refusal of root events goes.
7. **One door per binding.**
   - Rust: the six rows are `NodeSpec` setters on the root spec, and
     `WindowConfig` gains `on_window` and `on_dismiss`. `MenuBar` gains
     `on_menu`.
   - C: `KuiSpec` gains the six fields (read on the spec `kui_root` is
     given). The window and the menu bar get two doors:
     - `kui_window_declare_with(…, on_window, on_dismiss)`
     - `kui_menu_bar_with(…, on_menu)`

     These follow `kui_select_with`; each consumes its messages.
   - Odin: generated from those.
   - JSX: the rows are props of the root box; `windows` entries take
     `onWindow` and `onDismiss`; `<menuBar onMenu>`.
   - Lua: the same keys in its spelling.
8. **It joins ADR 0047's unreleased versions.** C's ABI 33 and the frame
   protocol v24 are not released yet (alpha.60 is unreleased), so these
   fields join them, as ADR 0034's did ABI 19. It is a hard break with no
   shim, like 0047's, and the alpha.60 CHANGELOG carries both.

## Considered options

- **Keep root events, and rename the core's kinds out of the app's way**
  (`kui.resize`, `@menu`). This leaves the two shapes, and it moves the
  collision rather than removing it: an app can still spell a reserved
  kind.
- **One root `onRoot` handler for all eight**, with the event telling them
  apart. One row is less to declare. But it rebuilds the union 0047 took
  apart: every root event would share one message type, and a TypeScript
  `onRoot` prop could not be typed per event.
- **Every window event on the root's `onWindow`, the main window's and the
  declared windows' alike.** Simpler to describe, but the app would then
  match a window by its `name` inside the data, which is the key-matching
  0047 set out to remove. The message on the window's own entry names it.
- **`onOpen` on every window's root.** The OS opens documents to the app.
  Delivering them to each window that declares a row would make the app
  deduplicate them, so the row is read on the main window's root.

## Consequences

- **The devtools keep hearing their own window.** The panel reads its
  window's `closed` (`devtools_consume`) and must not depend on the app
  declaring `onWindow`. The core routes that one internally, before it
  becomes an event.
- **Every binding's root and window declarations change,** along with
  `env`-reading apps that used the events as a redraw signal. A host that
  retains its tree (Node, C, Lua) re-runs its view only for a message, so
  an app that wants to follow the OS's appearance declares `onSystem`.
  The howto's "follow the OS's dark mode" entry says so.
- **The corpus pins it.** Scenes that close a window, dismiss a popup or
  change the system appearance declare the rows. A new scene declares
  every root row and the menu bar's `onMenu`, and drives each event. Every
  adapter reproduces it.
- **Decision 6 of ADR 0047 is superseded,** and its building notes about
  `is_root`, `KuiEvent.root` and the TypeScript collision record what this
  ADR removes.
