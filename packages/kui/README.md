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
