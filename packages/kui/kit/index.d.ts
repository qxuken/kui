// Kit for Node: see index.js.
import kit from './components.js';

export * from './components.js';
export default kit;

/** Where Kit's extension is: what `addExtension(namespace, path)` loads. */
export declare const extensionPath: string;

/** Kit as a window's (or `createApp`'s) `extensions` entry, under `kit`:
 *  `runWindowed(app, { extensions: [extension] })`. */
export declare const extension: { path: string; namespace: 'kit' };

/**
 * Loads Kit on a headless `Ctx` under `namespace` ("kit" by default) and
 * answers its components for that namespace. A window takes Kit before it
 * opens, through `extensions: [extension]`.
 */
export declare function install(
  target: { addExtension(namespace: string, path: string): void },
  namespace?: string
): typeof kit;
