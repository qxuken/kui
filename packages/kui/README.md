# kui

kui for Node: JSX views (a custom jsx-runtime, no React) lowered into the kui
IR in one call per frame, Elm-style messages as data. The library itself is
documented in the [kui repository](https://github.com/qxuken/kui).

The package is published to that Forgejo's npm registry under the `@qxuken`
scope; route the scope there once (Forgejo does not proxy npmjs, so do not
override the default registry) and install:

```
npm config set @qxuken:registry https://drydock9.qxuken.dev/api/packages/qxuken/npm/
npm install @qxuken/kui@alpha
```

Or start from the template: `npm create @qxuken/kui-node my-app`.

Every release so far is a prerelease, so `latest` and `alpha` point at the same
newest alpha and a bare `npm install @qxuken/kui` gets it; `npm view
@qxuken/kui@alpha version` is the query that still answers if `latest` is ever
absent. Ranges do not pin a prerelease — `^0.1.0-alpha.8` and `~0.1.0-alpha.8`
both admit every later alpha of the same `0.1.0` — so an app that wants the
version it tested writes that version exactly and commits its lockfile.

The tarball bundles the native addon for linux-x64, linux-arm64, darwin-arm64,
darwin-x64 and win32-x64 under `prebuilds/`; `native.cjs` picks the one matching
`process.platform`-`process.arch`. On any other platform build it from the
repo (`cargo build -p kui-node --release`) and set `KUI_NODE_LIB` to the
resulting library.

```tsx
// tsconfig: "jsx": "react-jsx", "jsxImportSource": "@qxuken/kui"
import { createApp, runWindowed } from '@qxuken/kui';
import type { CoreMsg, Ctx, KuiWindow, UiEvent } from '@qxuken/kui';

type Model = { count: number };
// This app's own messages plus the ones the core sends by itself.
type Msg = { kind: 'add'; by: number } | { kind: 'reset' } | CoreMsg;

// The fourth argument is the surface the loop drives — the headless `Ctx`
// under `createApp`, the `KuiWindow` under `runWindowed` — for `editText`,
// `focus`, `play` and the rest. Both drivers pass it.
function update(model: Model, msg: Msg, ev: UiEvent<Msg>, ui: Ctx | KuiWindow): Model | undefined {
  switch (msg.kind) {                       // one union, no casts
    case 'add': return { count: model.count + msg.by };
    case 'reset': return { count: 0 };
  }
}

// `init` is the first model, or a function the surface is handed to — after
// `setup`, so the fonts and images it registered are there to measure
// against. Under a window that argument is the window, so a first model can
// be built at the size it really opened at instead of at a constant
// corrected on the first `resize`:
//   runWindowed({ init: (win) => ({ count: 0, size: win.size() }), ... })
const init = (): Model => ({ count: 0 });

// `view(model, window, surface)`: the window's name (`'main'` unless
// `windows` declared others) and the surface, for the measurement a tree
// needs while it is being built. A single-window app that measures nothing
// takes `model` alone.
const view = (model: Model, _window: string, ui: Ctx | KuiWindow) => (
  <box pad={24} gap={16}>
    <button onClick={{ kind: 'add', by: 1 }}>+1</button>
    {/* Wide enough for the widest count it will ever show, so the button
        beside it does not shift as the number grows. */}
    <box width={ui.measureText('count = 0000', { size: 20 }).width}>
      <text size={20}>{`count = ${model.count}`}</text>
    </box>
  </box>
);
const app = createApp({ init, update, view }, { width: 640, height: 480 });   // headless
const final = await runWindowed({ init, update, view }, { title: 'counter' }); // a window
```

Both drivers run one loop over one surface, so what differs between them is
only what really differs: `runWindowed` pumps the OS and resolves with the
final model, `createApp` is synchronous. Everything else — the clock, the
diagnostics gate, the test affordances — is the same code either way. To
drive the window one of these opens — a smoke test reading `win.quads()`,
say — press with `access(key, 'click')`: against a real window that is not
only the screen reader's path but the only synthetic input it takes, since
`click`, `type` and `key` are refused on a surface the OS drives.

That loop takes a clock — `tick: { every: 250, msg: (now) => ({ kind: 'tick',
now }) }` — and re-renders on a tick only when `update` returns a new model,
so a countdown is free between displayed seconds. That contract cuts both
ways; see **A clock** below. A window fires the ticks off its own timer; a
test moves the hands itself with `app.advance(ms)`, so an app with a clock
still runs headless.

An effect the app defines and kui knows nothing about — a file write, a
request, the clipboard — is data on the same terms as everything else the
loop handles ([ADR 0013](../../docs/adr/0013-effects-as-data.md)).
`update` returns it beside the model:

```ts
case 'save': return withEffects({ ...model, dirty: false }, { kind: 'write', path: model.path, text: model.text });
```

and the app says once, in the options, what performing one means —
`effects: (effect, dispatch, surface) => { … }` — which the loop calls
after the frame, so an effect that dispatches its result
(`dispatch({ kind: 'saved' })`) lands in the next turn, and one that reads
the surface sees the frame its cause produced. `update` stays pure, the
same handler serves `createApp` and `runWindowed`, and headless
`app.effects()` drains what `update` returned whether or not a handler
ran, so a test asserts the effect the way it asserts an audio command.
`withEffects(undefined, …)` keeps the model, and a function `init` may
return one too. kui's own effects stay where they are: a sound is
`surface.play` or an `<audio>` node, a window is `windows`.

## Testing

`createApp` runs the same app headless, and everything a frame produces
comes back as data, so a test drives the app the way a user would and
asserts on what the core produced:

- **Input**: `app.click(x, y)`, `app.type(s)`, `app.press(code)` settle the
  loop for you; `app.ctx.cursor` / `mouse` / `scroll` / `modifiers` are the
  raw events (a drag is cursor, mouse down, cursor, mouse up).
  **`press` is the key one.** A real key press is two channels and a window
  drives both — the raw press an `onKey` sink hears, and then what the core
  is asked to do with that key (Escape dismisses a modal, Tab walks the
  ring, an arrow nudges a focused slider, Space presses a focused control).
  `press('escape')` does both; `app.key(name)` and `app.ctx.keyDown(code)`
  are its two halves, for a test that means to drive one channel and not
  the other. `app.release(code)` is the key coming up. The loop sets the frame clock before every frame, so a
  `transition` eases from the frame that changes it and `app.advance(ms)`
  is what moves it; a test that wants only the end state advances past
  the duration. (A bare `Ctx` whose `setTime` is never called snaps.)
- **Time**: `app.advance(ms)` is the window's timer by hand. It fires every
  `tick` that falls inside the span, moves the frame clock behind
  `transition` with it, and re-renders — so a ticking app (a countdown, a
  clock, a game loop) is driven from a test the same way a user's window
  drives it, and a transition can be watched a step at a time
  (`app.ctx.animating()` says when it has settled). `startTime` in the
  options pins where that clock starts, so assertions on `tick.msg(now)`
  are exact.
- **Effects**: `app.effects()` drains what `update` returned with
  `withEffects` since the last drain — the file write or request the app
  asked for, as a value — whether or not an `effects` handler was
  registered; with one, the handler already ran after the frame and its
  `dispatch` has already gone through `update`.
- **Either surface**: `settle`, `access`, `accessTree`, `dispatch`, `render`
  and `step` are the loop's, not the headless driver's, so they work against
  a real window too — `runWindowed`'s `setup(win, app)` hands you the same
  object. Synthetic input (`click` / `type` / `key`) needs a surface that
  takes it: a `Ctx` does, and a window, which the OS drives, says so rather
  than pretending.
- **The frame**: `decodeQuads(app.ctx.quads())` is the display list
  (`x`, `y`, `w`, `h`, `color`, `radii`, `kind`), so "the compact tier fits
  its window" is `every((q) => q.x + q.w <= width)`.
- **Events and sound**: `app.ctx.pollEvents()`, and `app.ctx.audioCommands()`
  is what a window would have played.
- **Measurement**: `app.ctx.measureText(content, style, maxWidth)` is what
  layout gives the same `<text>`, so a breakpoint assertion is arithmetic.
  It is the same object `view`'s third argument is, so a test measures what
  the view measured.
- **Warnings**: `app.warnings` is every silent misconfiguration the core
  noticed (a `grow` weight with nothing to split, a transition on an unkeyed
  list item, a duplicate key); assert it is empty.

## Windowed app checklist

Things the package already does that are easy to miss when building a
real window. The full prop / element / event reference ships with the
package as [props.md](props.md) (`docs/props.md` in the repository), and so
do [CHANGELOG.md](CHANGELOG.md) — every release lists what it adds and,
separately, what you can delete — and the ADRs under `docs/adr/`, which
is where a doc comment pointing at `docs/adr/0003-modal-surfaces.md`
resolves from inside `node_modules`. [howto.md](howto.md) ships with them:
about twenty questions — playing a sound, animating a removal, resetting an
editor, driving a real window in a test — answered in two sentences each and
pointing into the other three.

- **Hover and pressed colors** are props, not queries: `hoverBg`,
  `pressedBg`, and `hoverGroup="name"` to light connected pieces together.
  Add `transition={150}` and the swap eases. `<button>` is exactly that data.
  For hover-dependent *layout* use `onHover={tag}` and react to
  `{kind: 'hover', phase: 'enter' | 'leave'}` events; `win.isHovered(key)`
  and `isPressed` answer for keys you got from events.
- **The pointer shape is declared**, not derived: `cursor="pointer"` on a
  control, `cursor="grab"` on a handle (and `'grabbing'` while its drag
  runs), `cursor="ewResize"` on a splitter. A node that declares nothing is
  `default` — an `onClick`, `focusable` or `onDrag` box included, as a
  native button is — except over an editor or a `selectable` scope, where
  it is `text`. The stock `<button>` declares its own hand. A window applies
  it by itself; `ctx.cursorShape()` reads it back for a test.
- **Motion that never settles** is data too: `keyframes` takes CSS-style
  stops for `width` / `height` / `bg` / `radius`, cycled over
  `transition` ms in CSS's `animation-direction` (`repeat="alternate"`)
  and held back by `delay` ms so siblings stagger —
  `<box transition={1100} easing="easeInOut" repeat="alternate" delay={i * 550}
  width={{ grow: 0 }} keyframes={[{ width: { grow: 1 } }]} />` slides
  forever without the app ever waking up to flip it. Stops spread evenly
  unless they name `at` (0..1); a slot a stop leaves out falls back to the
  node's own prop, so `[{ at: 0.5, bg: '#f5a97f' }]` is a pulse.
- **Arrivals** are data too. A node's first frame snaps, so a slide-in
  needed an off-screen frame and a second render; `enter` states the
  starting point instead: `<box key="toast" float="viewport" transition={200}
  enter={{ dx: 320, bg: '#00000000' }} …/>` slides in from the right and
  fades up on the frame it appears, and enters again after being dismissed.
  Add `slide` if it should also glide when layout moves it later — a float
  whose `dx`/`dy` changes eases to the new offset with `slide` alone.
- **Frame timing without the overlay**: `win.frameStats()` is the latency
  HUD as data (`last.viewMs`, `avgWorkMs`, …), `win.stats()` the display
  list summary, and `win.animating()` tells a test when motion has settled.
- **Tooltips**: `tooltip="hint"` on any box (implies hover tracking).
- **Per-corner radius**: `radius` for all four, `radiusTL` / `radiusTR` /
  `radiusBR` / `radiusBL` after it for the exceptions.
- **Sound**: register bytes once (`win.addSound(buf)` in `setup`, any
  wav/ogg/mp3/flac), then reach for it three ways. As props — `clickSound`
  and `hoverSound` on any box, the audio equivalent of `hoverBg`. As an
  element — `<audio key="music" src={id} loop volume={0.3} />` is a playback
  retained by key: it plays while the view declares it, stops when the view
  drops it, and `volume` / `paused` changes apply to the running sound
  rather than restarting it, so `{model.music && <audio … />}` is the whole
  on/off story. Or imperatively — `win.play(id, { volume, loop, fadeIn,
  tag })` returns a playback id for `stop` / `setVolume` / `pause` /
  `resume`, and a `tag` comes back as a `SoundMsg`
  (`{kind: 'sound', phase: 'ended', tag}`) when that playback finishes on
  its own, which is how a chime chains into the next state. Volumes are
  linear amplitude. Headless (`createApp`) nothing plays: `ctx.audioCommands()`
  hands you what a window would have played, which is what to assert on.
- **Fonts**: `win.loadFontsDir('fonts')` then `win.addSystemFont('Antonio')`,
  or `win.loadFontFile('fonts/Antonio.ttf')`, `win.addFont(bytes)`, or an
  installed family by name (see `systemFontFamilies()`, or `systemFonts()`
  for which are monospaced, their weights and italics); then
  `<text font={id}>`. Register in `setup(win)` before the first frame.
  The installed ones are rescanned when macOS or Windows says a font was
  installed or removed, and a `FontsMsg` follows for a model that keeps
  the list; on Linux `win.reloadSystemFonts()` scans again.
- **Window chrome**: open with `chrome: 'custom'`, put a `<titlebar>` (or
  your own strip with `window="drag"` plus `<windowButtons/>`) in the root;
  on macOS it insets past the traffic lights itself. The root box's
  `title` prop names the window each frame.
- **Overlays**: `float="below" | "above"` or
  `float={{ anchor: 'viewport', at: ['end','end'], self: ['end','end'] }}`
  draws on top without shifting anything.
- **Sliders and dividers**: `onDrag={tag}` gives `{ x, y, dx, dy, parent }`
  — `parent` is the container rect, so a fraction needs no geometry query.
- **Measuring text**: `win.measureText('1,234', { size: 48, font })` (and
  `ctx.measureText` headless) returns `{ width, height, lines }` — what
  layout gives a `<text>` with that content and those props, at the
  window's scale; pass a `maxWidth` to see it wrapped, and `wrap` /
  `maxLines` / `ellipsis` apply. Size a column to its widest label, or pick
  the tier whose labels fit, from these numbers; they follow the font. Both
  are reachable where the sizing happens: `view(model, window, surface)`
  gets the surface third, and `init(surface)` gets it before the first
  model, so neither needs the surface parked in a module-level variable.
- **Where did layout put it**: `onLayout={tag}` on a keyed box brings back
  `{ kind: 'layout', x, y, w, h, parent, tag }` — on its first frame and
  whenever the rect changes, never on a frame that left it alone, so
  keeping it in the model and re-rendering does not loop. A `slide` reports
  every frame it moves. It is the numbers layout already computed, handed
  back, for the case no prop covers yet.
- **Warnings**: the core notices what used to fail silently — a `{ grow: 2 }`
  on the only grow child (or across the parent's main axis), a `transition`
  on an unkeyed list item whose siblings changed count, two nodes on one
  `key` — and `runWindowed` prints each once (`warnings: false` in the
  options to stop it; `win.warnings()` drains them yourself). The checks
  are a development aid: under `NODE_ENV=production` they do not run at
  all (`diagnostics: true` forces them on).
- **Window size**: `win.size()` gives `{width, height, scale}` (logical px)
  — in `setup(win)` before the first frame, in `init(win)` while the first
  model is built, and any time after. It is the viewport the app lays out
  into: the window's inner size, less the devtools' dock while the panel is
  docked (`KUI_DEVTOOLS=1`), and the same number `env().viewport` reads once
  a frame has run. `win.hostArea()` gives that viewport's place in the
  window after a frame, `{x, y, w, h}` — right of the pane under a left
  dock — which is what tells the app's quads from the dock's.
  Headless, `ctx.size()` answers the same shape: the size
  `createApp` frames at (its `width` / `height` / `scale`) before the first
  frame, and the last frame's after. Changes
  arrive as `{kind: 'resize', width, height, scale}` events — a dock coming,
  going or being dragged among them — so a model that
  tracks the size updates in `update` like anything else. Bound what the
  user can resize to with `minWidth` / `minHeight` / `maxWidth` / `maxHeight`
  next to `width` / `height` at open; either half of a pair may stand alone,
  and `width`/`height` are clamped into the bounds the OS will enforce.
- **A clock**: `tick: { every, msg }` on either driver (`app.advance(ms)`
  fires them headless), or `setTimeout` toward the next boundary in your own
  loop; do not call `update` every pump. Ticks are frequent, so unlike UI events **a tick re-renders only
  when `update` returns a new model** — a countdown that returns
  `undefined` until the displayed second changes costs nothing in
  between. The same rule read backwards is the trap: a tick handler that
  mutates the model in place and returns `undefined` (the escape hatch
  the rest of `update` allows) never reaches the screen. Any
  non-`undefined` return renders, so `return model` after a mutation is
  the whole fix. `every` may read the model — `every: (m) => m.running
  ? 16 : 1000` — for an app whose cadence depends on its state: the loop
  re-reads it after every `update`, and since the windowed driver never
  sleeps through a tick, a stopped countdown then costs a pump a second
  instead of pinning the idle backoff at 16 ms.
- **Keys**: `onKey` on the root plus `keyFocus`; presses arrive as
  `{ kind: 'key', phase: 'down', code, ... }` with `code` a character or a
  name (`'space'`, `'enter'`, `'f5'`), and that is all a keymap needs — no
  `phase` check. A held-key interaction (WASD, press-and-hold) adds `keyUp`
  to the sink and hears releases too, as the same shape with
  `phase: 'up'`. `repeat` marks an auto-repeat, and a release carries a
  null `text`. A key only comes up where it went down — focus moving
  delivers the release first — so nothing is left stuck. `location`
  (`'numpad'`, `'left'`, `'right'`, else `'standard'`) says which of a
  key's twins it was, `caps_lock` and `num_lock` what was locked, and a
  sink that adds `modifierKeys` hears the modifier keys themselves
  (`'shift'`, `'capslock'`, …).
  `onKey={null}` is a sink whose events carry no `tag` (the same goes for
  `onDrag`, `onHover` and `onLayout`), so a root sink needs no inert
  message in the app's union.
- **Messages are yours**: annotate `update` and the loop follows —
  `createApp` / `runWindowed` infer the union, so `ev`, `dispatch` and
  `tick.msg` speak it too (a `tick.msg` written inline needs the union named
  — `runWindowed<Model, Msg>(...)`, or a `msg` annotated where it is
  written — since its return is what would be inferred from). `CoreMsg` is what the core sends on its own
  (`DragMsg`, `KeyMsg`, `HoverMsg`, `ModifiersMsg`, `changed` / `submit`),
  each with the payload fields spelled out; `pollEvents<KeyMsg<Tag>>()`
  types a raw poll the same way. To have the *payload props* checked at the
  node as well, register the app's own union once:

  ```ts
  declare module '@qxuken/kui/jsx-runtime' {
    interface KuiMsg { msg: MyMsg }
  }
  ```

  `onClick` then takes exactly `MyMsg` (and `onDrag` / `onHover` / `onKey` /
  `onLayout` take `MyMsg | null`) rather than any plain data, so a typo
  fails where it is written. Register the messages you wrote, not
  `MyMsg | CoreMsg`: `CoreMsg` is typed in terms of the registration (its
  `tag` fields carry your messages), so naming it there makes the alias
  circular — keep that union for `update`. It is a program-wide
  declaration (one app per tsconfig); left out, payload props stay untyped
  and nothing else changes.
