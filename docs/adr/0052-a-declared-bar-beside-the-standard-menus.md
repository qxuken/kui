---
status: accepted
date: 2026-10-23
---

# A declared bar beside the standard menus

> **Accepted and built 2026-10-23; backlog F185.** What the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed). Raised by the Cookie Mapping
> Builder, a Node app on alpha.63, which wanted one menu of its own —
> «Пакеты», with ⌘F and ⌘R — in the macOS bar. [ADR
> 0030](0030-the-standard-menus-the-runner-keeps.md) gives a process
> that declares no bar the standard one: the application menu, Edit and
> Window. [ADR 0018](0018-a-menu-bar-the-app-declares.md) and ADR 0030's
> decision 2 make a declared bar exactly the declaration. So the one menu
> cost four: the app re-declared an application menu, an Edit menu with
> the standard roles (in Russian, «Правка»), and a `Window` menu so
> tiling kept working, and then its own.

## Context

- **What a declared bar loses.** The application menu becomes the
  declaration's first menu, titled by the process whatever its label. The
  standard Edit menu, whose rows replay ⌘C, ⌘V and the rest to the
  key-focused sink (ADR 0030, decision 3) and grey from the runner's
  stamps (W14), is gone, and an app's own Edit menu of `copy` / `paste`
  roles is a different thing: roles the core performs, not the chord.
  The Window menu goes too. A declared `Window` is registered by its
  title (it is not localised), and its Enter Full Screen is kui's row.
- **What an app adding one menu means.** "Keep what the platform gives
  me and add this." Re-declaring the platform's menus is code the app
  should not have, and it drifts: the declared Edit menu does not grey
  like the standard one, and a Russian `Правка` cannot be found by
  AppKit's title scan.
- **Off macOS there is no platform bar.** Windows and Linux draw the
  declared bar in the window (`widgets::menu_bar`). Their window
  managers own tiling, and the Edit chords work from the keyboard with
  no menu. A core that draws its own bar on macOS
  (`set_native_menu_bar(false)`) has the standard bar above and the
  declared one in the window already (ADR 0030, decision 1).

## Decision

1. **A flag on the bar: `standardMenus`** (`<menuBar standardMenus>`,
   Lua `standard_menus = true`, Rust `MenuBar::standard_menus(true)`, C
   `kui_menu_bar_standard_menus`, Odin `kui.menu_bar_standard_menus`).
   The declared menus go beside the platform's standard menus instead of
   replacing them. It is part of the declaration: sticky, diffed, and
   read back (`menuBar().standardMenus`,
   `kui_menu_bar_has_standard_menus`).

2. **On macOS the bar is: the process's application menu, the standard
   Edit menu, the declared menus in order, the standard Window menu.**
   That is the HIG's place for an app's own menus, between Edit and
   Window. None of the declared menus is the application menu, so the
   first one keeps its title. A declared menu titled `Window` is only a
   menu: the standard one is registered. A declared `Edit` gets AppKit's
   appended rows trimmed like any Edit menu's, and nothing else.
   Accelerators bind as any declared row's do.

3. **Where the bar is drawn in the window, the flag changes nothing**:
   there are no platform menus to keep, and the strip is the declared
   menus. One view stays portable.

4. **The standard Edit and Window menus are built new for each bar**, as
   a declared bar's menus are; only the application menu's item, winit's,
   with its Services registration, moves between bars. AppKit appends
   Emoji & Symbols and Dictation to an Edit menu in the scan of a root it
   has not seen, and fills the registered Window menu when first asked.
   A kept menu moved into a second root would gather a second copy.

## What was declined, and why

- **Placing a declared menu before Edit** (a `File` menu, which the HIG
  puts there). Every app menu goes between Edit and Window, File
  included, which reads one place off on a Mac. Declined for now: the
  app that asked has no File menu. A per-menu `place: "beforeEdit"`, or
  markers for the standard menus in the declared list, would do it. The
  condition that would build it: an app that wants a File menu and the
  standard Edit.
- **Localised standard titles.** The standard Edit and Window menus read
  in English, as winit's application menu does, whatever the app's
  language. An app that needs `Правка` still declares its bar. The
  condition: a localisation story for the platform's rows, which AppKit
  gives a bundled app from its `.lproj` and a process run as a CLI does
  not have.
- **Standard menus in the drawn bar** (an Edit menu replaying chords on
  Windows and Linux). The platforms do not expect one, and an app that
  wants it declares it.

## Consequences

- The Cookie Mapping Builder declares «Пакеты» alone with
  `standardMenus`, and its application, Edit and Window menus go. Its ⌘F
  and ⌘R bind from the bar, and ⌘C, ⌘V and the tiling chords work from
  the standard menus as in an app that declares nothing.
- Node's wire is v25 (a flags word after `menuBar`'s message). C gains
  two functions with ABI 35. Odin gets both generated.

## What the building changed

- **The application menu's item moves between roots.** It is the one
  row that cannot be built again (winit's, with its Services
  registration), and an `NSMenuItem` has one menu. `home` takes it out of
  whichever root holds it and puts it at the head of the one going up:
  the bar beside the standard menus, or the standard root again when an
  app declares an empty bar. The Edit and Window menus are built new each
  time by the same two functions the standard bar's are built by
  (`edit_menu`, `window_menu`), so the two bars cannot drift.
- **Checked on macOS 27 in a window**, with a Node app declaring
  «Пакеты» (⌘F, ⌘R) with `standardMenus` and read through the AX API: the
  bar is `node` (About, Services, Hide, Quit), Edit (Undo through Select
  All, then Start Dictation and Emoji & Symbols once each, as on the
  plain standard bar), «Пакеты», and Window with AppKit's Fill, Center,
  Move & Resize, Full Screen Tile, Enter Full Screen and the window list,
  so the Window menu registered. ⌘F and ⌘R, posted to the process,
  arrived as the bar's `onMenu` with `find` and `reload`. A bar flipped at
  runtime between beside, alone and none went up each time with the
  application menu in place. Read right after a switch, the AX tree of
  the bar can show untitled heads for half a second; the path that
  existed before (none to alone) shows the same, so it is AX catching
  up, not a second bar.
