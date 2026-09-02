# kui

kui for Node: JSX views (a custom jsx-runtime, no React) lowered into the kui
IR in one call per frame, Elm-style messages as data. The library itself is
documented in the [kui repository](https://drydock9.qxuken.dev/qxuken/kui).

The package is published to that Forgejo's npm registry under the `@qxuken`
scope; route the scope there once (Forgejo does not proxy npmjs, so do not
override the default registry) and install:

```
npm config set @qxuken:registry https://drydock9.qxuken.dev/api/packages/qxuken/npm/
npm install @qxuken/kui@alpha
```

Or start from the template: `npm create @qxuken/kui-node my-app`.

The tarball bundles the native addon for linux-x64, linux-arm64, darwin-arm64,
darwin-x64 and win32-x64 under `prebuilds/`; `native.cjs` picks the one matching
`process.platform`-`process.arch`. On any other platform build it from the
repo (`cargo build -p kui-node --release`) and set `KUI_NODE_LIB` to the
resulting library.

```tsx
// tsconfig: "jsx": "react-jsx", "jsxImportSource": "@qxuken/kui"
import { createApp, runWindowed } from '@qxuken/kui';
import type { CoreMsg, UiEvent } from '@qxuken/kui';

type Model = { count: number };
// This app's own messages plus the ones the core sends by itself.
type Msg = { kind: 'add'; by: number } | { kind: 'reset' } | CoreMsg;

function update(model: Model, msg: Msg, ev: UiEvent<Msg>): Model | undefined {
  switch (msg.kind) {                       // one union, no casts
    case 'add': return { count: model.count + msg.by };
    case 'reset': return { count: 0 };
  }
}

const view = (model: Model) => (
  <box pad={24} gap={16}>
    <button onClick={{ kind: 'add', by: 1 }}>+1</button>
    <text size={20}>{`count = ${model.count}`}</text>
  </box>
);
const app = createApp({ init, update, view }, { width: 640, height: 480 });   // headless
const final = await runWindowed({ init, update, view }, { title: 'counter' }); // a window
```

The `runWindowed` loop also takes a clock — `tick: { every: 250, msg: (now) =>
({ kind: 'tick', now }) }` — and re-renders on a tick only when `update`
returns a new model, so a countdown is free between displayed seconds.

## Windowed app checklist

Things the package already does that are easy to miss when building a
real window. The full prop / element / event reference ships with the
package as [props.md](props.md) (`docs/props.md` in the repository).

- **Hover and pressed colors** are props, not queries: `hoverBg`,
  `pressedBg`, and `hoverGroup="name"` to light connected pieces together.
  Add `transition={150}` and the swap eases. `<button>` is exactly that data.
  For hover-dependent *layout* use `onHover={tag}` and react to
  `{kind: 'hover', phase: 'enter' | 'leave'}` events; `win.isHovered(key)`
  and `isPressed` answer for keys you got from events.
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
- **Fonts**: `win.loadFontsDir('fonts')` then `win.addSystemFont('Antonio')`,
  or `win.loadFontFile('fonts/Antonio.ttf')`, `win.addFont(bytes)`, or an
  installed family by name (see `systemFontFamilies()`); then
  `<text font={id}>`. Register in `setup(win)` before the first frame.
- **Window chrome**: open with `chrome: 'custom'`, put a `<titlebar>` (or
  your own strip with `window="drag"` plus `<windowButtons/>`) in the root;
  on macOS it insets past the traffic lights itself. The root box's
  `title` prop names the window each frame.
- **Overlays**: `float="below" | "above"` or
  `float={{ anchor: 'viewport', at: ['end','end'], self: ['end','end'] }}`
  draws on top without shifting anything.
- **Sliders and dividers**: `onDrag={tag}` gives `{ x, y, dx, dy, parent }`
  — `parent` is the container rect, so a fraction needs no geometry query.
- **Window size**: `win.size()` gives `{width, height, scale}` (logical px)
  — in `setup(win)` before the first frame, and any time after. Changes
  arrive as `{kind: 'resize', width, height, scale}` events, so a model that
  tracks the size updates in `update` like anything else. Bound what the
  user can resize to with `minWidth` / `minHeight` / `maxWidth` / `maxHeight`
  next to `width` / `height` at open; either half of a pair may stand alone,
  and `width`/`height` are clamped into the bounds the OS will enforce.
- **A clock**: `tick` on `runWindowed`, or `setTimeout` toward the next
  boundary in your own loop; do not call `update` every pump.
- **Keys**: `onKey` on the root plus `keyFocus`; presses arrive as
  `{ kind: 'key', code, ... }` with `code` a character or a name
  (`'space'`, `'enter'`, `'f5'`).
- **Messages are yours**: annotate `update` and the loop follows —
  `createApp` / `runWindowed` infer the union, so `ev`, `dispatch` and
  `tick.msg` speak it too. `CoreMsg` is what the core sends on its own
  (`DragMsg`, `KeyMsg`, `HoverMsg`, `ModifiersMsg`, `changed` / `submit`),
  each with the payload fields spelled out; `pollEvents<KeyMsg<Tag>>()`
  types a raw poll the same way. To have the *payload props* checked at the
  node as well, register the app's own union once:

  ```ts
  declare module '@qxuken/kui/jsx-runtime' {
    interface KuiMsg { msg: MyMsg }
  }
  ```

  `onClick` / `onDrag` / `onHover` / `onKey` then take exactly `MyMsg`
  rather than any plain data, so a typo fails where it is written. It is a
  program-wide declaration (one app per tsconfig); left out, payload props
  stay untyped and nothing else changes.
- **Testing**: `createApp` runs the same app headless; `ctx.cursor` /
  `mouse` / `keyDown` drive it and `decodeQuads(ctx.quads())` inspects the
  frame (`radii`, `color`, `kind`).
