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
real window. The full prop / element / event reference is
[docs/props.md](../../docs/props.md) in the repository.

- **Hover and pressed colors** are props, not queries: `hoverBg`,
  `pressedBg`, and `hoverGroup="name"` to light connected pieces together.
  Add `transition={150}` and the swap eases. `<button>` is exactly that data.
  For hover-dependent *layout* use `onHover={tag}` and react to
  `{kind: 'hover', phase: 'enter' | 'leave'}` events; `win.isHovered(key)`
  and `isPressed` answer for keys you got from events.
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
  tracks the size updates in `update` like anything else.
- **A clock**: `tick` on `runWindowed`, or `setTimeout` toward the next
  boundary in your own loop; do not call `update` every pump.
- **Keys**: `onKey` on the root plus `keyFocus`; presses arrive as
  `{ kind: 'key', code, ... }` with `code` a character or a name
  (`'space'`, `'enter'`, `'f5'`).
- **Testing**: `createApp` runs the same app headless; `ctx.cursor` /
  `mouse` / `keyDown` drive it and `decodeQuads(ctx.quads())` inspects the
  frame (`radii`, `color`, `kind`).
