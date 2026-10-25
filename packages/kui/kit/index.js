// Kit, kui's UI kit (addons/kit, ADR 0066), for Node: the components as
// functions that are also JSX components, and the extension that draws
// them. Load it once on the window (or a headless Ctx), then place:
//
//   import { install, Button } from '@qxuken/kui/kit';
//   setup: (win) => { install(win); }
//   <Button label="Run" variant="primary" onPress={{ kind: 'run' }} />

import { fileURLToPath } from 'node:url';
import kit from './components.js';

export * from './components.js';
export default kit;

/** Where Kit's extension is: what `addExtension(namespace, path)` loads. */
export const extensionPath = fileURLToPath(new URL('./lua/kit.lua', import.meta.url));

/** Kit as a window's (or `createApp`'s) `extensions` entry, under `kit`:
 *  `runWindowed(app, { extensions: [extension] })`. */
export const extension = { path: extensionPath, namespace: 'kit' };

/**
 * Loads Kit on a headless `Ctx` under `namespace` ("kit" by default) and
 * answers its components for that namespace. A window takes Kit before it
 * opens, through `extensions: [extension]`.
 */
export function install(target, namespace = 'kit') {
  target.addExtension(namespace, extensionPath);
  return namespace === 'kit' ? kit : kit.at(namespace);
}
