---
status: accepted
date: 2026-09-14
---

# The standard menus the runner keeps

> **Accepted (2026-09-14), built the same day** — what the building
> changed is at the end, under [*What the building
> changed*](#what-the-building-changed). Raised by using a kui app on
> macOS 26 and pressing fn+ctrl+F: nothing. The system's window
> shortcuts — Fill, Center, the tiling arrows, Return to Previous Size,
> Enter Full Screen on fn+F — are not system shortcuts. They are **menu
> items**, rows AppKit adds to the application's Window menu, and their
> key equivalents fire through that menu and nowhere else. winit installs
> only the application menu (About, Hide, Quit), and
> [ADR 0018](0018-a-menu-bar-the-app-declares.md) replaces the bar with
> exactly what the app declared, so a kui process has no Window menu
> unless the app happens to declare one — and a declared one is not
> registered, so it gets nothing either. This document gives every kui
> process on macOS the menus a Mac app is expected to have when the app
> has said nothing about menus, and says what a declared bar gets.

## Context

- **What AppKit needs, measured** (`macOS 26.6`, a Swift probe posting
  the chords through the HID tap and reading the frame back):

  | bar | fn+ctrl+F (Fill) | fn+F (Full Screen) |
  |---|---|---|
  | no Window menu | nothing | nothing |
  | a menu merely *titled* "Window" | nothing | nothing |
  | a Window menu registered via `NSApp.windowsMenu` | fills | full screen |

  Registering is the whole of it. Once registered, AppKit populates the
  menu itself the first time it is asked for — Fill, Center, Move &
  Resize ▸, Full Screen Tile ▸, Remove Window from Set, Move to iPad,
  Minimize All, the window list, the tab rows — and the chords work.
  **Enter Full Screen is the exception:** AppKit adds that one row once,
  at `finishLaunching`, and only to a Window (or View) menu registered by
  then. A menu registered from the run loop gets tiling and no full
  screen. A row kui adds itself — `toggleFullScreen:` on the responder
  chain, key equivalent `f` with the Function modifier — fires on fn+F
  *and* fn+shift+F, and AppKit's own validation retitles it *Exit Full
  Screen* while the window is in one, so it is indistinguishable from
  the row AppKit would have added.

- **ADR 0018 declined standard application-menu roles** — `about`,
  `quit`, `services` — because they change what an item *is*: one the
  platform performs rather than one the app hears, needing their own
  answer for the drawn bar. That reasoning stands and this document does
  not reopen it. A Window menu is not a role: no binding spells it, no
  drawn bar draws it, and Windows and Linux window managers own tiling
  already (Win+arrows, the compositor's edges). It is a fact about the
  macOS runner, and lives there.

- **The runner already performs the Edit chords.** ⌘C/X/V/A, ⌘Z/⇧⌘Z and
  ⌘Y are handled in `Shell::on_key` after the press has gone to the
  key-focused sink (ADR 0011: chords bubble, and a sink that binds ⌘C
  itself hears it). An Edit menu of `MenuRole::Copy` rows bound to ⌘C
  would change that: AppKit consumes a key equivalent before the window
  sees it, so the sink would stop hearing the chord and the role would
  run instead. The menu has to spell the chord without taking it.

- **The empty declaration took ⌘Q away.** `apply(&MenuBar::default())`
  was `setMainMenu(None)`: a process that declared a bar and then
  declared none, or told the core to draw the bar itself
  (`set_native_menu_bar(false)` — the devtools' `menus` toggle, every
  example that defaults to drawn menus), had no bar at all, winit's Quit
  included.

## Decisions

1. **A process that declares no bar has the standard bar.** winit's
   application menu — kept, not rebuilt, so its Services entry stays the
   one the app registered — with **Edit** (Undo, Redo, Cut, Copy, Paste,
   Select All) and **Window** (Minimize ⌘M, Zoom, Enter Full Screen,
   Bring All to Front) beside it, the Window menu registered as
   `NSApp.windowsMenu`. AppKit adds the rest: Fill, Center and the tiling
   submenus to Window; Emoji & Symbols, Dictation and Writing Tools to
   Edit. It goes up at the first pump, comes back when an app that
   declared a bar declares an empty one, and is what the platform shows
   while a core draws its own bar — the platform's standard menus above,
   the app's drawn ones in the window, never two copies of the app's.

2. **A declared bar is exactly what the app said, and a menu it titles
   `Window` is the platform's Window menu.** Nothing is appended to a
   declaration — an app that wants tiling names a menu `Window`, and the
   runner registers that one, which is how AppKit itself reads a nib. It
   gains the one row AppKit cannot add after launch, Enter Full Screen;
   its other rows are the app's, and AppKit's rows land above them.
   Declaring `Edit`, `View` or `Help` was already this (ADR 0018, *what
   running it showed*): macOS adds rows to menus it knows by title, and
   a declared bar was never only its declaration.

3. **The standard Edit menu's rows are chords, not roles.** Choosing
   Copy — by mouse or by the ⌘C AppKit consumed — *replays* ⌘C: the
   press to the key-focused sink, the runner's own half of the chord
   (`Shell::edit_chord`, the block `on_key` always ran, now callable
   from both), the release. A sink that binds ⌘C itself hears the same
   thing from the menu; an editor copies through the code the key takes;
   `request_copy`'s tier-3 ask still goes out. Nothing arrives twice,
   because the key never reached winit. The rows carry no `MenuRole` and
   post no `menu` event — they are the keyboard, spelled where a Mac user
   looks for it.

4. **A popup is not in the Window menu.** `WindowKind::Popup` windows
   are marked `excludedFromWindowsMenu`: the list is for windows the user
   would switch to, and a menu surface with no title is not one.

## What was declined, and why

- **Appending the Window menu to a declared bar too.** It would give
  every kui app tiling for free, but a declared bar would no longer be
  what the app declared, and an app that wanted the menu elsewhere or
  under another name would have no way to say so. Naming a menu `Window`
  costs one word.

- **`MenuRole::Minimize` / `Zoom` / `FullScreen` / `Quit`.** ADR 0018's
  decline, unchanged: each needs a drawn-bar answer, an ABI bump and four
  bindings, for rows the platform performs on its own responder chain.
  The condition that would build them is an app that wants to compose a
  Window menu with its own rows *between* the platform's, which the
  register-by-name rule cannot give.

- **Validating the Edit rows** (Copy greyed with nothing to copy). The
  keyboard has no such greying either; the rows are the keyboard. Filed
  as W14 for the day a design wants it.

- **Registering the standard Window menu before launch** so AppKit adds
  Enter Full Screen itself. It works (a menu registered before
  `finishLaunching` and attached to the bar after still gets the row),
  but a declared `Window` menu is always registered after launch and
  would then be the one path without full screen. One row kui adds, in
  both paths, is one rule.

## Consequences

- fn+ctrl+F fills, fn+ctrl+arrows tile, fn+ctrl+R returns, fn+F and
  fn+shift+F enter and leave full screen, ⌘M minimizes — in every kui
  window on macOS, with no line in the app. Driven through the AX API
  against `examples/rust/widgets/edit.rs` (standard bar) and
  `examples/rust/widgets/menu_bar.rs` (declared bar with a `Window`
  menu): the frame goes to the screen's size and back, the row reads
  *Exit Full Screen* while it is in one, and Edit ▸ Select All / Copy /
  Cut / Paste / Undo / Redo chosen from the bar do to the field what the
  chords do, read back through the clipboard.

- `MacMenuBar` gains the standard bar, built once and kept; `apply` of
  an empty bar installs it instead of nothing; `take_chosen` answers
  `BarPick::Item` or `BarPick::Chord`. The runner's `applied_menu_bar`
  is an enum (`Standard` / `Declared(window, revision)`) rather than a
  tuple with a sentinel revision. `Shell::edit_chord` is the chord block
  of `on_key`, factored so the menu can call it with the letter and
  Shift it means rather than with a winit event it does not have.

- The `menu_bar` example declares a fourth menu, `Window`, with one
  checked row, and its headless drive counts four. Nothing changes in
  any binding, the ABI, the corpus or the drawn bar: this is macOS's
  runner doing what a macOS runner owes.

## What the building changed

- The first version re-parented winit's application menu into a fresh
  root and aborted with a foreign exception: an `NSMenu` has one
  supermenu. The second appended Edit and Window to winit's own root and
  re-set it — and got no Emoji & Symbols, because AppKit scans a bar for
  the menus it knows by title only in `setMainMenu:` of a bar it has not
  seen. The third moves winit's *item* (`removeItemAtIndex(0)`) into a
  fresh root, which is what ships: one bar object, scanned once, with
  the app menu, its Services registration and AppKit's Edit rows all
  where a declared bar has them.

- Enter Full Screen was expected to come from AppKit and did not; the
  probe found the launch-time rule above, and the row became kui's.

- The probe itself: a CGEvent posted from a process that exits
  immediately never arrives — the poster has to outlive the post by a
  run-loop turn. Half an hour of "the chord does nothing" was that.
