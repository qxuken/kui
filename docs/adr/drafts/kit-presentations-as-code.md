---
status: proposed
date: 2026-11-04
---

# Kit presentations are authored as code

The interview decisions below are confirmed. The proposed interface and
implementation sequence are ready for the user's final review of the
shared understanding. This record does not describe an implemented feature.

The user requested a visual spike before production implementation.
[The throwaway native prototype](../../../addons/kit/samples/presentation-prototype/README.md)
compares Paper, Signal and Stage compositions on branch
`codex/presentation-style-spike`. It exercises real Kit charts, controls,
adaptive layout and reveal navigation. No visual direction has been
selected; the proposed public interface below is not implemented by the
spike.

## Context

Kit already provides content useful in native interactive decks: charts,
code blocks, Markdown, shaders and interactive controls. Authors currently
have to assemble the presentation structure and presenting behavior
themselves. The aim is to make feature-rich presentations easy to create
as code using Kit.

## Settled decisions

- Prioritize native interactive decks written in Lua/TSX using Kit.
- Ordinary Lua/TSX code is the source of truth. Supply reusable slide
  layouts while allowing arbitrary Kit content and custom layouts inside
  each slide, including live demonstrations.
- The first complete version includes keyboard and clicker navigation,
  progressive reveals, overview and jumping, speaker notes, a separate
  presenter window with a timer and next-slide preview, fullscreen, themes
  and transitions.
- Start with equivalent Lua and TSX features. Prefer a runner-specific
  capability when it offers a substantial benefit that other bindings
  cannot match without a large change; parity is not an absolute
  constraint. No specific exception has been selected yet.
- Authors define slides, notes, content and demo state, then use one
  presentation launcher. It supplies presenting behavior; existing
  applications can also embed a deck. Runner conveniences can include
  launching a deck file directly and watching it for changes.
- Fixed logical-canvas sizing is opt-in through a flag/configuration,
  rather than the default. By default slides use regular adaptive kui
  layout and reflow with the available window size. A fixed canvas
  preserves the chosen logical size and composition. The flag's spelling
  has not yet been agreed.
- Reusable content improvements belong in base Kit and its demos, not
  only in the presentation layer. Scope includes presentation typography;
  title, section, columns, comparison, quote, image and big-number layouts;
  code syntax highlighting and line emphasis; and reuse of charts,
  tables, SVG and drawing primitives. Automatic diagram routing/layout
  remains outside this first scope.
- Initial export scope is a headless PNG sequence plus a manifest, as a
  secondary milestone. Image-backed PDF is a subsequent option. Editable
  PPTX and an HTML image viewer are outside the current scope.
- Future web rendering may enable web presentations, but web rendering
  is not part of this work.
- Next advances a reveal step before advancing to the next slide.
  Previous rewinds a step, then returns to the previous slide's final
  step. Jumping to a slide starts at its initial step.
- Unrevealed content reserves layout space by default while being
  excluded from input, Tab navigation and accessibility. Authors can
  opt into collapsing its layout space.
- Focused controls keep their normal keys; deck navigation handles
  unconsumed keys.
- Demo state survives navigating away and back. Deck-managed timers and
  effects pause while the slide is inactive. Overview, next-slide
  previews and export use a separate snapshot mode. Authors can provide
  static substitutes for content that cannot safely render in that mode;
  arbitrary application code must respect the lifecycle contract.
- The presenter explicitly chooses the audience display, with a
  single-window fallback. Closing the audience window leaves the
  presenter open. Leaving fullscreen preserves the talk's position.
- A hotkey must reopen the presenter window when it has been closed.
- A deck is a list of slide definitions with stable keys, notes and render
  functions. Only interactive slides need state, update or activation
  hooks. Reveal helpers declare step numbers; the launcher derives the
  total rather than asking the author to maintain a separate count.
- Export defaults to the final reveal step of every slide, with an option
  for every step. Use snapshot mode, settled transitions, a fixed clock
  and a configurable viewport. The manifest names the slide, step and
  image. Responsive decks reflow at that viewport; fixed-canvas decks
  retain their composition.
- Watch reload preserves the current slide, step and compatible demo
  state through stable keys. Cancel and recreate managed effects rather
  than duplicating timers. An explicit restart resets the session.
- The configurable app-local presenter recovery shortcut is Cmd+Shift+P
  on macOS and Ctrl+Shift+P elsewhere. It opens or focuses the presenter
  window from either presentation window. Closing either window leaves
  the session alive while another remains; closing the last ends it.

This preserves the ability to generate content programmatically and reuse
application views. A constrained slide description or Markdown format
would make common content concise but would need escape hatches for
interactive demonstrations and custom compositions.

## Capability audit

- Kit has content components, but no deck, slide, presentation type scale
  or slide layouts (`addons/kit/src/kit.tl`, `addons/kit/schema.lua`).
- Core aspect ratios and paint transforms exist. Subtree scaling does
  not change layout coordinates, and enlargement softens text and edge
  ramps; a crisp logical slide canvas needs investigation (ADR 0043).
- Unclaimed keys bubble to a root key handler, allowing presentation
  navigation to coexist with editing and control keys
  (`crates/kui-core/src/runtime/dispatch.rs`).
- Multiple windows can share the application model. Explicit fullscreen
  control and monitor placement are absent from the current window
  interface (`crates/kui-core/src/window.rs`). Fullscreen therefore needs
  platform work; it is not just a new Kit control.
- Deterministic clocks, synthetic input and pixel capture already exist.
  Deck traversal and export orchestration do not. Headless operation does
  not automatically open all declared windows; export should explicitly
  target the audience view (ADR 0057, `packages/kui/index.d.ts`).
- Kit settings are currently shared within one extension/window, not
  scoped to subtrees. Typography exports are a small UI scale, so reusable
  large typography needs a richer interface without enlarging every
  compact control (`addons/kit/src/kit.tl`, `addons/kit/src/skin.tl`).
- Timers are not owned by slide visibility, and TSX function components
  do not supply state or lifecycle hooks. The presentation launcher must
  own its managed lifecycle explicitly rather than rely on unmounting
  (`crates/kui-cli/src/lua/runtime.lua`, `packages/kui/jsx-runtime.js`).
- Lua watch reload preserves globals/model but reloads app modules and
  does not cancel existing timers. Preserving navigation while preventing
  duplicated managed effects needs explicit reload handling
  (`crates/kui-cli/src/host.rs`).

## Proposed module interfaces

### Base Kit content

Add reusable content typography and composition to Kit's schema and Teal
implementation. Candidate public names are `heading`, `paragraph`,
`content`, `columns`, `comparison`, `quote` and `figure`. A `content`
preset supplies common title/section composition rather than overloading
the existing collapsible `section`. Extend `stat` for large-number
compositions and `code` for syntax highlighting and line emphasis rather
than introducing parallel components for the same content.

All components retain useful defaults for regular applications. Reading
sizes, spacing, alignment, colours and content presets are configurable;
large slide text must not enlarge presenter chrome or every embedded
control. Reuse adaptive layout conditions. Existing Kit defaults remain
the defaults for existing applications. Theme and skin choices continue
to apply, and reduced motion remains respected.

Generic layout-preserving visibility should support reveals while also
being usable outside decks. Invisible content must be absent from paint,
input, Tab order and accessibility while retaining layout when requested;
opacity alone does not satisfy this contract. Collapsing visibility is a
separate option. Verify that measurement and retained component identity
remain correct for both.

Generate all existing Kit bindings and references from the schema, and add
gallery demos and component guidelines for these changes. These are base
Kit improvements, not an internal slide-only design system.

### Deck behavior

Keep one session's navigation, slide state, managed effect lifetimes,
timer and window intentions outside individual view instances. Audience
and presenter views observe the same session; previews cannot navigate it
or start slide effects. An embeddable controller provides the same deck
behavior without requiring the standalone launcher.

The deck description contains ordinary code, including render functions.
It must not be serialized wholesale into a Lua extension prop. Host
helpers consume callbacks and place the resulting content through Kit's
ordinary typed components. Keep wire-compatible visual components in the
shared schema; share navigation rules and conformance fixtures across the
host helpers so the semantics cannot drift between Lua and TSX.

Proposed slide fields:

| Field | Purpose |
| --- | --- |
| `key` | Stable identity for navigation, reload, state and export. |
| `title` | Human-readable name in overview and presenter navigation. |
| `notes` | Private speaker notes. |
| `view(context)` | Pure composition from the current state, step, size and render mode. |
| `init()` | Optional initial demo state. |
| `update(state, message)` | Optional demo state update. |
| `activate(context)` | Optional managed live effects; may return cleanup. |
| `snapshot(context)` | Optional safe/static composition for content unsuitable for snapshot mode. |
| `state_version` | Optional compatibility marker for retained state across edits. |

The context exposes slide state, the current step, audience dimensions,
live/snapshot mode, reveal helpers and slide-addressed messages. Managed
timers/tasks belong to activation, not rendering. Dispose effects on
deactivation, reload, restart and shutdown; discard late results from an
obsolete activation. This does not sandbox arbitrary author code or stop
external work the author starts outside the managed interface.

Render declarations must expose their reveals regardless of the current
step so the total can be derived reliably. Step zero is the initial
composition; positive step numbers stage additional content. Several
reveals can use the same step. If content cannot declare its future steps,
report a useful authoring diagnostic rather than silently guessing.

### Host and runner integration

Proposed entry points are `require("kit.presentation")` in the kui runner
and `@qxuken/kui/kit/presentation` in Node. A launcher installs the needed
Kit extension and manages the standard presentation application. The
controller remains usable inside an existing app. Lua runner watch,
capture and resource loading are conveniences; no runner-exclusive feature
has yet been selected as a reason to drop TSX parity.

Use regular adaptive layout unless `canvas = { w, h }` is supplied. The
matching CLI canvas option is a convenience over that setting. Fixed
canvas mode fits and letterboxes the chosen logical frame. Establish
crisp text, clips and hit testing at actual output resolution; validate
existing paint scaling before deciding whether core renderer work is
needed. Do not silently accept blurry screenshots as the final solution.

Window/session ownership must allow the original presenter window to close
without killing an audience window, and allow it to reopen. Add explicit
fullscreen and display-selection support to the shared host interfaces,
with graceful fallback where a window manager cannot honor a request.
Keep the recovery shortcut app-local and configurable. A single-window
mode must keep private notes off the audience view unless the user
explicitly enters the presenter interface.

### Export

Traverse only audience compositions in an isolated headless session. Use
the same adaptive layout, canvas and snapshot contract as previews. Do
not run live activation hooks or modify the live talk's state. Capture
settled slide/reveal transitions at a fixed configurable timestamp;
continuous animations sample that time instead of waiting forever to
settle. Authors supply deterministic sample data or snapshot substitutes
for live resources.

Default to one PNG at each slide's final step, with an every-step option.
Specify viewport, pixel scale and appearance explicitly in the manifest,
along with slide key, title, step, timestamp and image path. Diagnostics
must identify the slide and step that failed. Image-backed PDF remains a
later option, not a requirement of this implementation.

## Illustrative authoring examples

These examples show the proposed interface; the imports and components do
not exist yet. Exact property names will follow the typed schema.

Lua deck on the runner:

```lua
local kit = require("kit.ui")
local present = require("kit.presentation")

present({
  title = "A live system",
  -- canvas = { w = 1280, h = 720 }, -- opt in; otherwise regular layout
  slides = {
    {
      key = "intro",
      title = "One source, live content",
      notes = "Advance once to introduce the demo.",
      view = function(c)
        return kit.content({ preset = "title" }, {
          kit.heading({ text = "One source, live content", level = 1 }),
          c.reveal(1, {
            kit.paragraph({ text = "Use ordinary Kit components." }),
          }),
        })
      end,
    },
  },
})
```

Equivalent TSX entry:

```tsx
import * as kit from '@qxuken/kui/kit';
import { present } from '@qxuken/kui/kit/presentation';

await present({
  title: 'A live system',
  // canvas: { w: 1280, h: 720 },
  slides: [{
    key: 'intro',
    title: 'One source, live content',
    notes: 'Advance once to introduce the demo.',
    view: c => (
      <kit.Content preset="title">
        <kit.Heading text="One source, live content" level={1} />
        <c.Reveal step={1}>
          <kit.Paragraph text="Use ordinary Kit components." />
        </c.Reveal>
      </kit.Content>
    ),
  }],
});
```

## Implementation sequence and acceptance

1. **Base Kit content.** Add typography/composition and code improvements
   with generated wrappers, guidelines and gallery demos. Check adaptive
   behavior, existing defaults, skins, appearance and reduced motion.
2. **Deck controller and host helpers.** Implement definitions, derived
   steps, navigation, layout-preserving reveals, retained demo state,
   snapshot mode and explicit managed lifecycle. Use shared behavior
   fixtures for Lua and TSX; verify that focused editors/controls keep
   their keys and unrevealed content has no input/accessibility presence.
3. **Launcher and presenting.** Add overview, notes, timer, presenter and
   audience windows, fullscreen/display choice, recovery hotkey and
   single-window fallback. Verify close/reopen from either window and
   last-window shutdown with real-window smoke tests.
4. **Watch and fixed canvas.** Preserve position and compatible state on
   reload, clamp steps after edits, and relocate to a neighboring slide
   if its key was removed. Dispose stale effects; reset incompatible
   state using its compatibility marker. Verify restart and rapid
   repeated reloads. Validate fixed-canvas text, clips and input at
   different sizes and display scales.
5. **Headless export.** Produce PNGs and a manifest for final steps or all
   steps. Verify deterministic snapshot rendering, settled transitions,
   continuous animation sampling and isolation from live state/effects.
6. **Complete examples and documentation.** Ship Lua and TSX decks covering
   adaptive layouts, optional canvas, interactive demos, every content
   helper, notes, reveals, themes, reload and export. Render and inspect
   representative slides and their exported images.

Each implementation milestone must remain independently usable. The
complete presenting feature set is the delivery target; the sequence is
not a reduction to a navigation-only release. Export remains secondary.

## Final review

The preference questions Q1–Q15 are answered. The proposed interfaces,
module placement and sequence above are the concrete design for final
shared-understanding review. Implementation starts after that agreement.
