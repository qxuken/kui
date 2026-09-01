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
export function runWindowed({ init, update, view }, opts = {}) {
  const { width, height, chrome } = opts;
  const win = new KuiWindow(opts.title ?? 'kui', { width, height, chrome });
  opts.setup?.(win);
  let model = typeof init === 'function' ? init() : init;
  // Binary IR path by default; `transport: 'json'` keeps the readable
  // stringified path for debugging.
  const setView =
    opts.transport === 'json'
      ? (tree) => win.setViewJson(JSON.stringify(tree))
      : (tree) => win.setView(tree);
  setView(view(model));
  return new Promise((resolve, reject) => {
    const tick = () => {
      let alive;
      try {
        alive = win.pump();
        const events = win.pollEvents();
        if (events.length) {
          for (const ev of events) {
            const next = update(model, ev.payload, ev, win);
            if (next !== undefined) model = next;
          }
          setView(view(model));
        }
      } catch (e) {
        reject(e);
        return;
      }
      if (!alive) {
        resolve(model);
        return;
      }
      setTimeout(tick, opts.pumpMs ?? 8);
    };
    tick();
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
  const width = opts.width ?? 800;
  const height = opts.height ?? 600;
  const scale = opts.scale ?? 1;
  let model = typeof init === 'function' ? init() : init;

  const app = {
    ctx,
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
    type(text) {
      ctx.text(text);
      app.settle();
    },
    key(name, mods) {
      ctx.key(name, mods);
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
      radius: f[12],
      borderW: f[13],
      kind: u[14],
      uv: [u[15], u[16], u[17], u[18]],
      clip: [f[19], f[20], f[21], f[22]],
    });
  }
  return quads;
}
