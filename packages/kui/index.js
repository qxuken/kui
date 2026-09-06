// kui for Node - Elm-shaped: `view(model)` returns a JSX tree, event payloads
// are your messages, `update(model, msg)` returns the next model.
import native from './native.cjs';
import { createEncoder } from './encoder.js';

export const { Ctx, KuiWindow, quadStride, protocol } = native;
export { createEncoder };

// A frame crosses the boundary one way: JS encodes the tree into one
// Float64Array + string table and the addon lowers it zero-copy. One encoder
// serves every context — its buffers are consumed synchronously.
const encoder = createEncoder(native.protocol());

// A prop name the schema does not know has no wire id, so it never crosses:
// the encoder is the only side that can see it, and it reports what it
// dropped so the core can warn about it like any other misconfiguration.
function reportUnknown(surface, unknown) {
  if (unknown.length) surface.warnUnknownProps(unknown);
}

Ctx.prototype.frame = function frame(width, height, scale, tree) {
  const { stream, strings, unknown } = encoder.encode(tree);
  this.frameBinary(width, height, scale, stream, strings);
  reportUnknown(this, unknown);
};

// `window` names which window the tree is for: 'main' when left out, else
// one of the names `windows()` lists.
KuiWindow.prototype.setView = function setView(tree, window) {
  const { stream, strings, unknown } = encoder.encode(tree);
  this.setViewBinary(stream, strings, window);
  reportUnknown(this, unknown);
};

// ---------------------------------------------------------------------------
// One loop, two surfaces.
//
// A surface is whatever the loop drives: a headless `Ctx`, or a `KuiWindow`
// for a real window. Both answer the same handful of calls — `pollEvents`,
// `warnings`, `setDiagnostics`, and a way to be shown a tree — so the Elm
// loop, the diagnostics gate and every test affordance are written once,
// here, over whichever one they were handed. The two drivers below differ
// only where they really differ: `runWindowed` pumps the OS and resolves
// with the final model, `createApp` is synchronous and holds its own clock.

// The core reports silent misconfigurations (a grow weight with nothing to
// split against, a transition on a positional key, two nodes on one key) as
// data. The checks run in development only (off under NODE_ENV=production, so
// a shipped app pays and prints nothing); `diagnostics` overrides either way.
const formatWarning = (w) => `kui: warning [${w.code}] node ${w.key}: ${w.message}`;
const diagnosticsByDefault = () => process.env.NODE_ENV !== 'production';

/** How a tree reaches a surface — the transport the two really differ on.
 *  A window surface keeps a view per open window and redraws each at its
 *  own size, so `open()` lists the names to draw. A headless `Ctx` *is* one
 *  window, the main: it lowers one frame at the size it is told, and a
 *  second frame for another name would be a second frame of the same core
 *  — so it opens for `main` alone, whatever its `windows()` says the
 *  declared set has grown to. */
function transport(surface, opts) {
  if (typeof surface.setView === 'function') {
    return {
      show: (tree, name) => surface.setView(tree, name),
      // A surface that cannot say which windows it has, has one.
      open: () => (typeof surface.windows === 'function' ? surface.windows() : ['main']),
    };
  }
  const width = opts.width ?? 800;
  const height = opts.height ?? 600;
  const scale = opts.scale ?? 1;
  return {
    show: (tree) => surface.frame(width, height, scale, tree),
    open: () => ['main'],
  };
}

/** The declared window set on the root box, where the encoder reads it:
 *  `windows(model)` is the config-level spelling of the root's `windows`
 *  prop, so an app never writes the prop by hand. */
function withWindows(tree, windows) {
  if (!windows || tree == null || typeof tree !== 'object' || tree.type !== 'box') return tree;
  return { ...tree, props: { ...(tree.props ?? {}), windows } };
}

/**
 * The Elm loop over one surface: the model, `update`, the clock, and the
 * affordances a test drives it with. Both drivers build one of these.
 *
 * `clock` is where time comes from — `Date.now` under a window. Without one
 * the loop holds the hands itself and `advance(ms)` moves them, which is what
 * makes a ticking app drivable headless. Passing a fake one is how the
 * windowed half of the tick bookkeeping gets tested without a display.
 */
function createLoop({ init, update, view, tick, windows }, opts, surface, clock) {
  surface.setDiagnostics(opts.diagnostics ?? diagnosticsByDefault());
  const { show, open } = transport(surface, opts);
  let model;

  let now = clock ? clock() : (opts.startTime ?? Date.now());
  const at = () => (clock ? clock() : now);
  // The frame clock behind `transition` is set here and before every frame,
  // so `render()` and an event-driven frame see the same hands `advance`
  // moves: a tween's baseline is taken under a clock, not under none (where
  // the core snaps). A real window has no `setTime` — its runner stamps the
  // frame from its own epoch — so a wall-clock loop over one is a no-op here.
  //
  // That stamping is why the surface's own `setTime` is taken away from the
  // app (backlog F16): a time set by hand is overwritten by the next draw,
  // and the hands it is overwritten with only move in `advance`, so a
  // tween driven that way never leaves its start and `animating()` never
  // falls — a test waiting on it hangs. The loop keeps the native method
  // for itself and the instance gets one that says so, the way `advance`
  // refuses a wall clock.
  const stamp = typeof surface.setTime === 'function' ? surface.setTime.bind(surface) : null;
  if (stamp) {
    surface.setTime = () => {
      throw new Error('kui: this loop owns the frame clock; app.advance(ms) moves it');
    };
  }
  stamp?.(at() / 1000);
  // The clock, when asked for: `tick.msg` (or `tick.msg(now)`) goes through
  // `update` every `tick.every` ms.
  const every = tick?.every > 0 ? tick.every : 0;
  let nextTick = every ? at() + every : Infinity;

  // Fires the ticks owed at `t`. `catchUp` fires every one inside the span —
  // what `advance(ms)` asked for. Without it the loop takes one and resyncs
  // to the cadence: real time jumps (a drag, a GC pause) and a burst of stale
  // ticks helps nobody. Ticks are frequent, so unlike UI events they redraw
  // only when `update` returns a new model — so a tick that mutates in place
  // has to return the model to be drawn.
  function ticksTo(t, catchUp) {
    let redraw = false;
    while (t >= nextTick) {
      nextTick += every;
      if (!catchUp && nextTick <= t) nextTick = t + every;
      const msg = typeof tick.msg === 'function' ? tick.msg(t) : tick.msg;
      const next = update(model, msg, { origin: 0, key: '', payload: msg }, surface);
      if (next !== undefined) {
        model = next;
        redraw = true;
      }
      if (!catchUp) break;
    }
    return redraw;
  }

  // Warnings collect on `app.warnings` and print once unless `warnings: false`.
  function drainWarnings() {
    const ws = surface.warnings();
    if (!ws.length) return;
    app.warnings.push(...ws);
    if (opts.warnings !== false) for (const w of ws) console.warn(formatWarning(w));
  }

  // Everything the surface has queued, through `update`. The loop draws once
  // after, not once per event.
  function drainEvents() {
    const events = surface.pollEvents();
    for (const ev of events) app.dispatch(ev.payload, ev);
    return events.length > 0;
  }

  // A frame, without the `stats()` round trip `render` hands back — the
  // windowed pump draws sixty times a second and asks for none of it.
  // `view(model, name, surface)` runs once per open window; the main window's
  // tree also carries what `windows(model)` declares, so the set the loop
  // draws is the set the core diffs. A headless `Ctx` is one window, the main.
  // The surface rides along as the third argument so a view can measure —
  // `measureText` to size a column to its widest label, `size()` to pick a
  // tier — without the app parking it in a module-level variable.
  function draw() {
    stamp?.(at() / 1000);
    const declared = windows ? windows(model) : undefined;
    for (const name of open()) {
      const tree = view(model, name, surface);
      show(name === 'main' ? withWindows(tree, declared) : tree, name);
    }
    drainWarnings();
  }

  // An input call the surface does not take: a real window gets its input
  // from the OS, so it offers only some of them.
  function must(name) {
    const fn = surface[name];
    if (typeof fn !== 'function') {
      throw new Error(`kui: this loop's surface has no ${name}() to drive it with`);
    }
    return fn.bind(surface);
  }

  const app = {
    /** The surface this loop drives. */
    surface,
    /** The same object under the name headless tests reach for. */
    ctx: surface,
    /** Every warning the core raised while rendering, in order (each is
     *  also printed unless `warnings: false`). Assert on it, or on its
     *  emptiness. */
    warnings: [],
    get model() {
      return model;
    },
    dispatch(msg, event) {
      const next = update(model, msg, event, surface);
      if (next !== undefined) model = next;
    },
    render() {
      draw();
      return surface.stats();
    },
    /** One turn of the loop: whatever the surface queued, then the ticks the
     *  clock owes, then a frame if either changed the model. The windowed
     *  driver runs one after every `win.pump()`. */
    step() {
      drainWarnings();
      let redraw = drainEvents();
      if (ticksTo(at(), false)) redraw = true;
      if (redraw) draw();
    },
    /** Drain events -> update -> re-render until no events remain. */
    settle() {
      for (let round = 0; round < 8; round++) {
        if (!drainEvents()) return;
        draw();
      }
    },
    /** Moves the loop's own clock `ms` forward: every tick inside the span
     *  fires, the frame clock behind `transition` follows it, and the app
     *  re-renders. The test-driver counterpart of the window's timer — a
     *  window runs on the wall clock and ticks itself. */
    advance(ms) {
      if (clock) {
        throw new Error('kui: advance() moves the loop\'s own clock; this one runs on the wall clock');
      }
      now += ms;
      ticksTo(now, true);
      draw();
      app.settle();
    },
    /** Draws a frame, then advances the clock in frame steps until nothing
     *  is animating — the settled frame a capture or an end-state assertion
     *  wants, by name rather than by the loop both alpha.7 field reports
     *  wrote (backlog F22). Returns the milliseconds advanced; stops at
     *  `maxMs` with `animating()` still true if something never settles. */
    runOut(maxMs = 10_000, stepMs = 16) {
      draw();
      let elapsed = 0;
      while (surface.animating() && elapsed < maxMs) {
        app.advance(stepMs);
        elapsed += stepMs;
      }
      return elapsed;
    },
    click(x, y, clicks = 1) {
      const mouse = must('mouse');
      must('cursor')(x, y);
      mouse(true, clicks);
      mouse(false, clicks);
      app.settle();
    },
    rightClick(x, y) {
      const mouse = must('mouse');
      must('cursor')(x, y);
      mouse(true, 1, 'secondary');
      mouse(false, 1, 'secondary');
      app.settle();
    },
    type(text) {
      must('text')(text);
      app.settle();
    },
    key(name, mods) {
      must('key')(name, mods);
      app.settle();
    },
    /** A whole key going down, the way a window sends it: the raw press to
     *  an `onKey` sink, and then what the core is asked to do with that key
     *  — Escape dismisses a modal, Tab walks the focus ring, an arrow
     *  nudges a focused slider, Space presses a focused control, a
     *  printable character reaches the focused editor. This is the one a
     *  test that means "the user pressed this key" wants; `key` and
     *  `ctx.keyDown` are its two halves. */
    press(code, mods, repeat, physical) {
      must('press')(code, mods, repeat, physical);
      app.settle();
    },
    /** The same key coming up, spelled the way `press` spells it. One
     *  channel: the editing keys act on the way down, so this is the
     *  release a sink that declared `keyUp` hears. */
    release(code, mods, physical) {
      must('release')(code, mods, physical);
      app.settle();
    },
    /** What assistive technology sees of the last render. */
    accessTree() {
      return surface.accessTree();
    },
    /** Drive the app the way a screen reader would: `access(key, 'click')`
     *  activates a node, `access(key, 'setValue', text)` types into an
     *  editor; the events that follow go through `update`. */
    access(key, action, value) {
      surface.access(key, action, value);
      app.settle();
    },
  };

  // Resources before the model: `setup` registers fonts and images so `init`
  // can name their ids. `init` is handed the surface too, so a first model
  // can be built against the real window size and the fonts just registered
  // rather than against constants corrected on the first `resize`.
  opts.setup?.(surface, app);
  model = typeof init === 'function' ? init(surface) : init;
  return app;
}

/**
 * Opens a real kui window (winit + wgpu) and runs the loop against it.
 * The winit event loop is pumped from a timer so it shares the main thread
 * with libuv — Node stays fully responsive while the window is open.
 *
 * Resolves with the final model when the main window closes. One event
 * loop per process (winit event loops are not recreatable everywhere); any
 * number of windows on it — `windows: (model) => [...]` declares them, and
 * `view(model, window, win)` is called once per open window.
 */
export function runWindowed(config, opts = {}) {
  const { width, height, minWidth, minHeight, maxWidth, maxHeight, chrome } = opts;
  const win = new KuiWindow(opts.title ?? 'kui', {
    width, height, minWidth, minHeight, maxWidth, maxHeight, chrome,
  });
  const app = createLoop(config, opts, win, opts.clock ?? Date.now);
  app.render();
  return new Promise((resolve, reject) => {
    const pump = () => {
      let alive;
      try {
        alive = win.pump();
        app.step();
      } catch (e) {
        reject(e);
        return;
      }
      if (!alive) {
        resolve(app.model);
        return;
      }
      setTimeout(pump, opts.pumpMs ?? 8);
    };
    pump();
  });
}

/**
 * Headless app driver: the same loop over a headless `Ctx`.
 *
 * const app = createApp({ init, update, view }, { width, height });
 * app.render(); app.click(x, y); app.model
 *
 * `update(model, msg, event, ctx)` returns the next model (returning
 * undefined keeps the current one - useful when you mutate in place). A
 * `tick` runs here too: `app.advance(ms)` is the window's timer, by hand.
 */
export function createApp(config, opts = {}) {
  return createLoop(config, opts, opts.surface ?? new Ctx(), opts.clock ?? null);
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
      // Shadow quads only: the blur radius, which is also how far the rect
      // is inflated past the shape being blurred.
      blur: f[17],
      kind: u[18],
      uv: [u[19], u[20], u[21], u[22]],
      // Segment quads (kind 6) only: the endpoints `uv` carries as float
      // bits, x0, y0, x1, y1 in physical px; `borderW` is the stroke width.
      ends: u[18] === 6 ? [f[19], f[20], f[21], f[22]] : null,
      clip: [f[23], f[24], f[25], f[26]],
      // Corner radii of the clip, same order as `radii`: a clipping node
      // with a radius rounds what it clips. All zero = a plain rect clip.
      clipRadii: [f[27], f[28], f[29], f[30]],
    });
  }
  return quads;
}
