// kui for Node - Elm-shaped: `view(model)` returns a JSX tree, event payloads
// are your messages, `update(model, msg)` returns the next model.
import native from './native.cjs';
import { createEncoder } from './encoder.js';

export const { Ctx, KuiWindow, quadStride, protocol } = native;
export { createEncoder };

// The default transport is the binary IR stream: JS encodes the tree into one
// Float64Array + string table and the addon lowers it zero-copy. The addon's
// own object walk (`frameObject` / `setViewObject`) stays exposed as the
// reference path, but every property read there is an N-API call into V8,
// which makes it ~10x slower — so `frame` / `setView` encode first. One
// encoder serves every context: its buffers are consumed synchronously.
const encoder = createEncoder(native.protocol());

Ctx.prototype.frame = function frame(width, height, scale, tree) {
  const { stream, strings } = encoder.encode(tree);
  this.frameBinary(width, height, scale, stream, strings);
};

// The core reports silent misconfigurations (a grow weight with nothing to
// split against, a transition on a positional key, two nodes on one key) as
// data. The loops below run the checks in development only (off under
// NODE_ENV=production, so a shipped app pays and prints nothing) and print
// each once unless `warnings: false`; `setDiagnostics` overrides either way.
const formatWarning = (w) => `kui: warning [${w.code}] node ${w.key}: ${w.message}`;
const diagnosticsByDefault = () => process.env.NODE_ENV !== 'production';

KuiWindow.prototype.setView = function setView(tree) {
  const { stream, strings } = encoder.encode(tree);
  this.setViewBinary(stream, strings);
};

/**
 * Opens a real kui window (winit + wgpu) and runs the Elm loop against it.
 * The winit event loop is pumped from a timer so it shares the main thread
 * with libuv — Node stays fully responsive while the window is open.
 *
 * Resolves with the final model when the window closes. One window per
 * process (winit event loops are not recreatable everywhere).
 */
export function runWindowed({ init, update, view, tick }, opts = {}) {
  const { width, height, minWidth, minHeight, maxWidth, maxHeight, chrome } = opts;
  const win = new KuiWindow(opts.title ?? 'kui', {
    width, height, minWidth, minHeight, maxWidth, maxHeight, chrome,
  });
  win.setDiagnostics(opts.diagnostics ?? diagnosticsByDefault());
  opts.setup?.(win);
  let model = typeof init === 'function' ? init() : init;
  // Binary IR path by default; `transport: 'json'` keeps the readable
  // stringified path for debugging.
  const setView =
    opts.transport === 'json'
      ? (tree) => win.setViewJson(JSON.stringify(tree))
      : (tree) => win.setView(tree);
  setView(view(model));
  // The clock, when asked for: `tick.msg` (or `tick.msg(now)`) goes through
  // `update` every `tick.every` ms. Ticks are frequent, so unlike UI events
  // they re-render only when `update` returns a new model — so a tick that
  // mutates in place has to return the model to be drawn.
  const every = tick?.every > 0 ? tick.every : 0;
  let nextTick = every ? Date.now() + every : Infinity;
  return new Promise((resolve, reject) => {
    const pump = () => {
      let alive;
      try {
        alive = win.pump();
        if (opts.warnings !== false) {
          for (const w of win.warnings()) console.warn(formatWarning(w));
        }
        let render = false;
        const events = win.pollEvents();
        for (const ev of events) {
          const next = update(model, ev.payload, ev, win);
          if (next !== undefined) model = next;
        }
        if (events.length) render = true;
        const now = Date.now();
        if (now >= nextTick) {
          // Keep the cadence; if the loop fell behind, resync rather than
          // firing a burst.
          nextTick += every;
          if (nextTick <= now) nextTick = now + every;
          const msg = typeof tick.msg === 'function' ? tick.msg(now) : tick.msg;
          const next = update(model, msg, { origin: 0, key: '', payload: msg }, win);
          if (next !== undefined) {
            model = next;
            render = true;
          }
        }
        if (render) setView(view(model));
      } catch (e) {
        reject(e);
        return;
      }
      if (!alive) {
        resolve(model);
        return;
      }
      setTimeout(pump, opts.pumpMs ?? 8);
    };
    pump();
  });
}

/**
 * Headless Elm-style app driver.
 *
 * const app = createApp({ init, update, view }, { width, height });
 * app.render(); app.click(x, y); app.model
 *
 * `update(model, msg, event)` returns the next model (returning undefined
 * keeps the current one - useful when you mutate in place).
 */
export function createApp({ init, update, view }, opts = {}) {
  const ctx = new Ctx();
  ctx.setDiagnostics(opts.diagnostics ?? diagnosticsByDefault());
  const width = opts.width ?? 800;
  const height = opts.height ?? 600;
  const scale = opts.scale ?? 1;
  let model = typeof init === 'function' ? init() : init;

  const app = {
    ctx,
    /** Every warning the core raised while rendering, in order (each is
     *  also printed unless `warnings: false`). Assert on it, or on its
     *  emptiness. */
    warnings: [],
    get model() {
      return model;
    },
    dispatch(msg, event) {
      const next = update(model, msg, event);
      if (next !== undefined) model = next;
    },
    render() {
      // Binary IR path by default; `transport: 'json'` keeps the readable
      // stringified path for debugging.
      if (opts.transport === 'json') {
        ctx.frameJson(width, height, scale, JSON.stringify(view(model)));
      } else {
        ctx.frame(width, height, scale, view(model));
      }
      const ws = ctx.warnings();
      if (ws.length) {
        app.warnings.push(...ws);
        if (opts.warnings !== false) for (const w of ws) console.warn(formatWarning(w));
      }
      return ctx.stats();
    },
    /** Drain events -> update -> re-render until no events remain. */
    settle() {
      for (let round = 0; round < 8; round++) {
        const events = ctx.pollEvents();
        if (events.length === 0) return;
        for (const ev of events) app.dispatch(ev.payload, ev);
        app.render();
      }
    },
    click(x, y, clicks = 1) {
      ctx.cursor(x, y);
      ctx.mouse(true, clicks);
      ctx.mouse(false, clicks);
      app.settle();
    },
    rightClick(x, y) {
      ctx.cursor(x, y);
      ctx.mouse(true, 1, 'secondary');
      ctx.mouse(false, 1, 'secondary');
      app.settle();
    },
    type(text) {
      ctx.text(text);
      app.settle();
    },
    key(name, mods) {
      ctx.key(name, mods);
      app.settle();
    },
    /** What assistive technology sees of the last render. */
    accessTree() {
      return ctx.accessTree();
    },
    /** Drive the app the way a screen reader would: `access(key, 'click')`
     *  activates a node, `access(key, 'setValue', text)` types into an
     *  editor; the events that follow go through `update`. */
    access(key, action, value) {
      ctx.access(key, action, value);
      app.settle();
    },
  };
  return app;
}

/**
 * Decodes `ctx.quads()` into JS objects (debugging/testing aid).
 * Layout mirrors KuiQuad in include/kui.h; coordinates are physical px.
 */
export function decodeQuads(buffer) {
  const stride = quadStride();
  const quads = [];
  for (let off = 0; off + stride <= buffer.byteLength; off += stride) {
    const f = new Float32Array(buffer.buffer, buffer.byteOffset + off, stride / 4);
    const u = new Uint32Array(buffer.buffer, buffer.byteOffset + off, stride / 4);
    quads.push({
      x: f[0], y: f[1], w: f[2], h: f[3],
      color: [f[4], f[5], f[6], f[7]],
      borderColor: [f[8], f[9], f[10], f[11]],
      // Corner radii clockwise from the top-left; `radius` is the top-left
      // one, which is the uniform value for boxes rounded with `radius`.
      radii: [f[12], f[13], f[14], f[15]],
      radius: f[12],
      borderW: f[16],
      kind: u[17],
      uv: [u[18], u[19], u[20], u[21]],
      clip: [f[22], f[23], f[24], f[25]],
    });
  }
  return quads;
}
