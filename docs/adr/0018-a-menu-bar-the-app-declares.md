---
status: accepted
date: 2026-09-10
---

# A menu bar the app declares, and the one place it can live

> **Accepted and built (2026-09-10).** This is the entry
> [ADR 0004](0004-multi-window.md) parked at its end — "a native menu
> **bar** (which is a platform resource, not a window, and closer to the
> access tree in shape than to anything here)" — asked for by an app that
> wants File and Edit at the top of the screen and has no way to say so.
> It lands a week after [ADR 0017](0017-selection-as-a-scope.md) built the
> context menu, and the reason it is short is that ADR 0017 built most of
> it: a bar is the same rows one level up. **The whole of the new
> vocabulary is `MenuBar`, `BarMenu`, one flag on `MenuItem`, and two host
> facts.** What it does not reuse is the one thing a bar has that a
> context menu does not — a *place*, which on one platform of three is not
> in the window at all.

An application menu is the one part of an app's UI that may not be in its
window. On macOS it lives at the top of the screen, belongs to the process
rather than to any window, is drawn by the OS, and is where ⌘Q and ⌘, are
looked for. On Windows a menu bar is a Win32 relic that nothing shipping
uses (Explorer, Office, VS Code, Terminal all draw their own). On Linux it
depends on the desktop, and the global protocol is opt-in on the two that
have it.

So "add a menu bar" is two features that have to be one: hand a description
to a platform that owns menus, and draw the same description where nothing
does. An app that has to write the menu twice has not been given a menu.

## Context

- **The declaration has a shape to copy.** `title` is a string a frame
  declares and the driver diffs against what it applied (ADR 0004,
  decision 5); `windows` is a set a frame declares and the session diffs
  into `Open`/`Close`. A menu is the same kind of fact, and belongs beside
  them on the root.

- **ADR 0017 already decided what a row is.** `MenuItem` carries a label,
  a `MenuRole`, an enabled flag, an `id` payload and an accelerator
  *string* that "the core binds nothing to". `MenuRole::Copy` is performed
  by the core — clipboard work queued as a `MenuAction` for the host —
  and `MenuRole::Custom` is the app's. Every binding already parses that
  shape, and the corpus already pins how it draws.

- **The core already owns menus in the small.** `Role::Menu` /
  `Role::MenuItem` are a composite (ADR 0007): one Tab stop, arrows inside
  it, type-ahead, no activation on motion. `modal` (ADR 0003) gives a
  surface Escape and press-outside. A drawn bar is those two plus one
  retained fact — which menu is open.

- **The Windows question was answered three commits ago.** `71f381f` took
  the Win32 context menu back out: an `HMENU` is themed against the
  process's app mode and nothing else, so it came up white over a dark app,
  and "the native menu was charging the app's appearance for the system's
  metrics — which for a library whose argument is that the app owns its
  pixels is the wrong side of the exchange". A menu *bar* is the same
  exchange at ten times the surface area, and unlike macOS's it has no
  Look Up and no Services to buy with it.

## Decisions

1. **One call declares the menu and places it.** `<menuBar menu={[…]}/>`,
   `menu_bar { menu = {…} }`, `kui_menu_bar(ctx, menus, count)`,
   `widgets::menu_bar(ui, bar)` — the data is what the menu *is*, and where
   the call sits is where its titles go when they have to be drawn in the
   window. Sticky and diffed: a frame that never calls it leaves the last
   declaration in force, the same declaration again costs one comparison,
   and an empty list takes it away. `Core::menu_bar_revision` is what a
   driver diffs, so re-declaring an unchanged bar sixty times a second
   rebuilds no platform menu.

   **This was two things first, and one of them was redundant.** The draft
   put the description on the root beside `title` and `windows` and left
   `<menuBar/>` as a bare placement marker — so the prop was invisible
   without the element and the element was meaningless without the prop,
   and an app had to know both to get either. The root is the right home
   for a fact with *no* place in the tree (a title, a window set); this one
   has a place on every platform but macOS, and the call is where that place
   is said. On macOS the call draws nothing and the declaration still
   stands, which is exactly `windowButtons` under native decorations.

   It is not, for the same reason, a *tree* of `menuItem` elements
   (`menu_bar > menu_group > menu_item`). ADR 0017 settled that a row is
   data because something other than kui's renderer has to draw it — an
   `NSMenu` cannot be handed nodes — so items-as-elements would mean two
   spellings of `MenuItem` in one library, the second derived back into the
   first before anyone could use it, plus node props for `accel` and a menu
   `role` where `role` already names the access role.

2. **Its rows are the context menu's rows.** `BarMenu` is a label and a
   `Vec<MenuItem>`; there is no second item type and no second parser
   (decision 1's last paragraph is why).
   An Edit menu's `{ role: "copy" }` *is* the right-click Copy: the core
   performs it, queues the same `MenuAction`, and posts the same
   `{kind:"menu", role, item}` event. An app that wired that event once has
   both menus.

3. **`MenuItem` gains one flag: `checked`.** A bar has rows that are
   settings rather than commands (View ▸ Show Sidebar), which is a fact
   about the row and not a role — it is orthogonal to Copy, and a
   `menuitemcheckbox` role would have to be spelled for the six roles that
   already exist. It draws as a checkmark in a gutter the panel only opens
   when a row has one, reports `checked` to assistive technology, and
   becomes `NSMenuItem.state` where the platform draws. It costs an ABI
   bump (13), because `KuiMenuItem` travels as an array.

4. **`set_native_menu_bar` is the host fact**, beside ADR 0017's
   `set_native_menus` and spelled the same way in all four bindings. On:
   the driver hands the declaration to the platform and reports a choice
   through `activate_menu_bar_item`; the drawn bar draws nothing. Off (the
   default, and what every headless test sees): `widgets::menu_bar` draws
   it. One fact, so the two paths can never both be live or both be dead.

   It is a fact about the *host*, not a row in `env.system`, for the
   reason ADR 0017 made the same call: a view does not need it. The widget
   draws nothing where the platform owns the bar, so a view that calls it
   unconditionally is already portable, and nothing else about the frame
   changes.

5. **The drawn bar is the stock menu, one level up.** A title's menu is
   `widgets::menu_panel` — the same rows, roles, accelerators, disabled
   dimming and access tree the context menu draws — floated out of the
   title. What is the bar's own is the *scope*: while a menu is open the
   **bar** declares `modal`, not the dropdown. That is what makes hovering
   across the titles move the open menu (the titles stay live inside the
   scope), a press on the open title close it, and Escape or a press in the
   app below close it through ADR 0003's one mechanism.

6. **The open menu is retained by the core, not by the app.** It is the one
   thing the widget cannot derive: hover, press and focus are the core's
   already, but "the File menu is open" has to survive the click that
   opened it. It goes where scroll offsets and editor drafts go —
   `Core::menu_bar_open`, one per window, because a *top* menu bar is one
   per window.

   This is not ADR 0003 decision 6 overturned. That decision is about
   surfaces the *app* declares: kui raises `dismiss` and closes nothing,
   because only the app knows what its modal was for. This is a control kui
   itself ships, and the alternative is an app that carries `openMenu` in
   its model on Linux and not on macOS — which is decision 4's whole point
   given away.

7. **The core still binds no accelerator.** ADR 0017's rule stands: an
   `accel` says which key runs the row, and the app's own keymap is what
   runs it. The one addition is that a platform bar *can* bind one, so a
   declaration kui can parse (`"mod+s"`, `"⇧⌘S"`, `"Ctrl+Alt+F5"`) becomes
   a real `NSMenuItem` key equivalent — and, because the OS then consumes
   the key before the window sees it, exactly one of the two paths fires.
   `Accel::parse` reads both spellings, and `declare_menu_bar` rewrites
   what it can into the platform's own, so `"mod+s"` reads as `⌘S` on macOS
   and `Ctrl+S` elsewhere in *both* bars. A spelling kui cannot parse is
   drawn exactly as written and bound by nobody, which is what it was
   before.

8. **The bar is per application, and the frontmost window's declaration
   wins.** macOS has one menu bar for the process; kui has a `Core` per
   window. The runner applies the declaration of the window that holds the
   keyboard and re-applies when either that window or its revision changes.
   A window that declares nothing leaves the last bar standing rather than
   blanking it, which is what a palette or an inspector wants.

## What was declined, and why

- **A Win32 `HMENU`.** `71f381f`'s argument, one level up. Windows reports
  no native bar and draws decision 5's, which is what every Windows
  application in the field does.

- **A Linux global menu** (`com.canonical.AppMenu` over D-Bus, with
  `dbusmenu` behind it). Two desktops honour it, both optionally, and the
  failure mode of getting it wrong is a menu that exists and will not open.
  The seam is one `apply` per platform in the runner; nothing else moves.

- **Submenus.** A nested menu means a second shape for `MenuItem` (its own
  `items`), a second thing for the stock panel to draw, a hover-to-open
  timer, and a second thing every binding's parser can half-implement — for
  a Recent Files list. `BarMenu` is deliberately one level deep. When
  something wants one, it wants it in the context menu too, and that is one
  change to `MenuItem` rather than two features.

- **Standard application-menu roles** (`about`, `quit`, `services`, and
  macOS's `NSApplication` responders). They change what an item *is* — one
  the platform performs rather than one the app hears — so they need their
  own answer for what the *drawn* bar does with each. An app spells ⌘Q
  today with an item and a close in its handler.

- **A row in `env.system`.** Decision 4.

## Consequences

- An app describes its menu once, in one call in its view. On macOS that
  call draws nothing and the menu is at the top of the screen with working
  ⌘-shortcuts; everywhere else the same call draws the same menus as a
  strip, with the same rows, the same event, and the same standard roles
  performed by the core.

- The declaration is data, so a headless test asserts on it:
  `core.menu_bar()` is what was declared, `activate_menu_bar_item` is the
  platform's half by hand, and the corpus scene `menubar` pins the drawn
  bar's geometry, access rows and events across four bindings — which is
  how a binding that drops `checked` or spells `accel` differently fails to
  build rather than shipping.

- `Core` grows a declaration, a revision, one retained index and the drawn
  bar's node keys. A frame that declares no menu and draws no bar pays one
  `Option` check.

- macOS gains `NSApplication` and `NSCell` in its `objc2-app-kit` feature
  list and a second `define_class!` — the target the items send their
  action to. It rings the loop's `Waker` when a row is picked, because a
  ⌘-shortcut chosen from the bar is the whole of the event as far as winit
  is concerned: AppKit consumed the key, and without the wake the app would
  sit still until the user moved the mouse.

- ABI 13, for decision 3. One new [in] struct (`KuiMenu`), five new C
  entry points, and no change to any existing signature.

## What running it on macOS showed

Driven through the AX API against the `context_menu` example (the same way
`scripts/ax-audit.swift` drives a window's own controls), on macOS 26.6:

- The bar is `Apple | context_menu | Edit | View`. The first declared menu
  lands in the application-menu position and the OS titles it with the
  process name, exactly as decision 8 says — `"Demo"` in the declaration is
  not what is drawn.
- `"mod+shift+w"` reaches AppKit as `cmd="W"`, `mods=1` (Shift+Command),
  and a disabled row reads back disabled.
- The whole round trip closes without the mouse: `AXPress` on *Wrap the
  article* posts the event, the app toggles its model, the next frame
  re-declares the bar, the revision diff rebuilds the `NSMenu`, and reading
  the row back shows `mark="✓"`. That is the wake in decision 8's
  consequence doing its job — nothing else would have drawn a frame.
- **`env.focused` is not enough to say whose bar is up, and a Node window
  found it.** The runner first applied the declaration of the window that
  had the keyboard, which is decision 8 read literally. Under Node's
  windowed driver the bar went up at launch and was never rebuilt again:
  the app is *active* — its menu is the one on screen, and the one an item
  is chosen from — while no window of it holds the keyboard, which is an
  ordinary macOS state and precisely the state a menu is used in. The pick
  still arrived and `update` still ran; the frame after it re-declared the
  bar and nothing applied it, so a `checked` row never grew its checkmark.
  The rule is now "the window with the keyboard, or else whichever window
  declared one", and the same round trip closes from Node.

- **macOS adds its own rows to menus it recognises by title.** A menu
  called `View` came back with *Show Tab Bar*, *Show All Tabs* and *Enter
  Full Screen* in it, and `Window` and `Help` get the same treatment.
  Nothing kui declares put them there and nothing kui does can take them
  out of a menu with that name; an app that wants only its own rows names
  the menu something else. Not worth `allowsAutomaticWindowTabbing = false`
  in the runner: that would take window tabbing away from every kui app to
  tidy one menu.
