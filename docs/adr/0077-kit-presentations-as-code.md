---
status: accepted
date: 2026-11-04
---

# Kit presentations are authored as code

The user approved native Lua/TSX decks after an interview and a visual
spike comparing Paper, Signal, Stage and Minimal. They requested all four
as reusable presets, with project fonts, colors and sizes. The throwaway
spike remains on `codex/presentation-style-spike` at `ba6a610`; production
examples use the shared content components and presentation controller.

## Context

Kit already supplies controls, charts, Markdown, code and shaders, but
assembling a talk also required navigation, reveal semantics, presenter
windows, lifecycle and capture orchestration. A restrictive slide format
would still need an escape hatch for custom views and live demonstrations.
The feature should improve ordinary Kit apps as well as presentations.

## Decision

1. **Ordinary code is the source.** A deck contains ordered slide records
   with unique stable keys, optional titles/notes and `view(context)`
   callbacks. Stateful slides add `init`, `update`, `activate` and a state
   compatibility version. The callbacks stay in the host language;
   only ordinary visual components cross the extension boundary. Lua's
   `kit.presentation` and Node's `@qxuken/kui/kit/presentation` supply
   standalone launchers and embeddable controllers.
2. **Reusable content belongs in base Kit.** Add Heading, Paragraph,
   Content, Columns, Comparison, Quote, Figure and Visibility; extend
   Code with lexical highlighting, line emphasis and readable sizes,
   and Stat with display typography. Charts gain a configurable label
   size. Generate all existing bindings and references from the schema,
   and exercise the additions in the Kit gallery.
3. **Themes are Settings presets.** Paper, Signal, Stage and Minimal
   choose palette, content fonts, sizes and spacing. Explicit project
   settings win; role and size maps merge by field. Title, content and
   data faces are independent. Existing compact tool/control dimensions
   retain their defaults. Ledger, Phosphor and Neutral remain control
   skins. Content themes are distinct from core's `Theme` palette type.
4. **Theme and layout are independent.** `preset` conveniently pairs a
   theme with editorial, briefing, split or minimal layout. Explicit
   `theme` and `layout` override either half. Themes and layouts are authored
   settings; presenter controls do not change them during a talk. Watch
   reload applies authored settings while preserving compatible demo state.
   Layout defaults apply to the Content helper; arbitrary trees retain the author's structure.
5. **Adaptive layout is the default.** Optional `canvas = { w, h }` or
   `--canvas WIDTHxHEIGHT` fits and letterboxes fixed logical content.
   Existing transform-aware hits, caret queries and clips remain the
   input model. Static glyphs enlarged by transforms rasterize at a
   bucketed output resolution without changing layout, instead of
   enlarging a low-resolution mask. No web renderer is introduced.
6. **Navigation has one contract.** Next reveals before advancing;
   Previous rewinds before returning to the previous slide's final step.
   Jump starts at zero. Ends do not wrap. Reveal step numbers are positive
   integers, and the total is derived from declarations. Several reveals
   can share a step. Explicit `steps` supports snapshot replacements or
   compositions whose reveals cannot be discovered. Root shortcuts handle
   unclaimed keys; focused controls keep their own normal keys.
7. **Visibility is stronger than opacity.** Unrevealed content reserves
   layout by default, while disappearing from paint, hits, Tab and
   accessibility. Collapse is opt-in. Kit's Visibility composition uses
   a zero-scale subtree and `role = none`, with a hidden subtree for
   collapse; integration tests pin the whole contract.
8. **The controller owns the session.** Demo state is keyed and survives
   navigation. Activation helpers own timers and asynchronous results;
   deactivate on navigation, reload, restart and shutdown.
   Cancel timers, run cleanup, and reject obsolete generation results.
   Node tasks receive AbortSignal; Lua tasks need cooperative resources
   for cancellation. This does not sandbox author code.
9. **Snapshots do not activate demos.** Overview, presenter previews and
   export render isolated copies of plain state. They suppress view
   transitions and continuous component animation. Native resources remain
   shared references, so views must be pure and data deterministic.
   `snapshot(context)` supplies a safe substitute when needed and requires
   an explicit `steps` count. Previews have no input or accessibility
   subtree; overview adds an outer jump target.
10. **Presenter and audience share a session.** Notes remain private.
    The native runner invalidates sibling windows after extension-handled
    events as well as host-handled events, so synchronization never relies
    on the elapsed timer. Decks use ordinary `kui run`; the host-language
    `present` function installs the controller without a separate CLI mode.
    Extension event routes and component state are scoped by window as
    well as instance key; drawing one window cannot discard or overwrite
    another window's Select state or event invalidations.
    The presenter supplies current/next snapshots, navigation, overview,
    timer and audience display choice.
    Live sync defaults on. With it off, a separate keyed presenter cursor
    browses notes, steps and overview without moving or activating audience
    content. Present slide (Cmd/Ctrl+Enter) publishes that cursor; turning
    sync on publishes it and resumes following. Reload preserves both
    cursors by slide key. Only navigation arrows/pages accept auto-repeat.
    Overview is private to presenter and
    keeps the audience demo running. Private previews show future reveals
    faintly with numbered boxes; audience/export retain normal visibility.
    Audience content has no presentation controls or position counters.
    Recovery is in the macOS menu and `?` opens private shortcut help.
    Presenter uses a normal titlebar, Kit toolbar, content panels and status
    footer; restart asks for confirmation. Launchers do not force custom
    chrome on decks or ordinary samples. Kit Toolbar's `window_chrome`
    option explicitly integrates it into custom chrome, reserving native
    controls and keeping gaps draggable. Shared window
    commands request fullscreen, visibility and close-to-hide behavior;
    Lua and Node expose display names. C/Odin hosts enumerate displays
    themselves and receive equivalent commands. Fullscreen is advisory
    and falls back to the current display. The ABI command struct does
    not grow: the three appended command kinds encode booleans and an
    optional display in its existing numeric fields.
11. **Closing a window need not end the talk.** Close-to-hide is opt-in
    per window, leaving ordinary apps' close behavior intact. Closing
    either presentation window hides it while another regular window is
    visible; closing the last ends the session. Cmd/Ctrl+Shift+P opens
    or focuses presenter, configurable per deck and app-local. Show
    audience works from the visible presenter even when audience cannot
    redraw. Leaving fullscreen retains position and demo state.
12. **Reload preserves identity.** Preserve the current slide key, clamp
    its step, retain states with compatible `state_version` (Lua) /
    `stateVersion` (Node), and select the nearest index when a key was
    removed. Dispose/recreate managed effects. Restart explicitly resets
    state and navigation. Lua uses the runner's file watch; Node can watch
    a pure default-exported deck entry or a compiler-produced bundle.
13. **Export is PNG plus manifest.** Capture final reveal steps by
    default, every step optionally. Render independent headless audience
    sessions at a fixed configurable timestamp, reduced motion and
    settled layout. Record viewport, scale, appearance, key, title,
    one-based slide index, step, timestamp and relative PNG filename.
    Do not activate live work or mutate the running talk. Font files and
    source image resolution remain the author's inputs. Failures identify
    their slide/step. PDF, editable PPTX and web rendering are deferred.

## Boundaries and implementation

The [authoring guide](../../addons/kit/PRESENTATIONS.md) defines commands,
interfaces, keyboard behavior, overrides and lifecycle responsibilities.
The [Lua example](../../addons/kit/samples/presentations/main.lua) and
[TSX example](../../examples/node/apps/presentation.tsx) are runnable decks.

- Base visuals: [`schema.lua`](../../addons/kit/schema.lua),
  [`content.tl`](../../addons/kit/src/content.tl),
  [`themes.tl`](../../addons/kit/src/themes.tl), and existing skin/data/chart modules.
- Host behavior: [Lua controller](../../addons/kit/host/lua/kit/presentation.lua),
  [Node controller](../../packages/kui/kit/presentation.js).
- Shared navigation fixture: [trace](../../addons/kit/tests/presentation-navigation.json),
  exercised through both hosts, with lifecycle/reload/snapshot tests.
- Host integration: runner export orchestration and type declarations,
  shared window commands, native window policy, and transformed glyph rasterization.

This keeps the behavioral controller separate from visual components.
Future bindings can implement the same small callback contract and shared
navigation trace without serializing user callbacks or inventing another
presentation format.
