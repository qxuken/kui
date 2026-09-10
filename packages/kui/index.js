// kui for Node - Elm-shaped: `view(model)` returns a JSX tree, event payloads
// are your messages, `update(model, msg)` returns the next model.
import native from './native.cjs';
import { createEncoder } from './encoder.js';

export const { Ctx, KuiWindow, quadStride, clipStride, protocol } = native;
export { createEncoder };

// The brand on what `update` (or a function `init`) returns to hand the loop
// effects beside the model. A Symbol rather than a shape, because the model
// is untyped by the loop — an array model *is* a tuple, and `{ model,
// effects }` is a model with two keys — so nothing but a brand tells a
// wrapped return from a plain one. `docs/adr/0013-effects-as-data.md`.
const EFFECTS = Symbol.for('kui.effects');

// The driver's door to the loop's waiters (`settled` / `frame`). A throw
// from `win.pump()` happens outside `step()`, and an awaited frame that will
// never be painted has to reject rather than hang, so the pump's `catch`
// needs a way in. Private to this module — the loop's public shape is what
// `Loop` in index.d.ts says it is.
const FAILED = Symbol('kui.failed');
// How long the windowed driver may park in one pump — see `runWindowed`.
const BUDGET = Symbol('kui.budget');

/**
 * What `update` returns when it has effects to hand the loop besides the
 * next model: the model — or `undefined` to keep the current one, as a bare
 * `undefined` does — and the effects, which are the app's own values (kui
 * defines no effect vocabulary; its own effects stay where they are — a
 * sound is `surface.play` or an `<audio>` node, a window is `windows`).
 * The loop unwraps it, sets the model, queues the effects, and after the
 * next frame hands each one to the `effects` handler the app was created
 * with, as `effects(effect, dispatch, surface)`; headless, `app.effects()`
 * drains them for a test whether or not a handler ran. `init` may return
 * one too, for the effect an app starts with.
 */
export function withEffects(model, ...effects) {
  return { [EFFECTS]: true, model, effects };
}

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
/** The name of the core's devtools window while the panel is popped out
 *  (`setDevtoolsDock('window')`): what `windows()` lists it as, and what
 *  the loop never asks the app to draw. */
export const DEVTOOLS_WINDOW = 'kui-devtools';

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
      // A surface that cannot say which windows it has, has one. The
      // devtools' own window is the core's: it draws itself, and the app's
      // `view` is not asked for it (docs/adr/0024, decision 6).
      open: () =>
        typeof surface.windows === 'function'
          ? surface.windows().filter((name) => name !== DEVTOOLS_WINDOW)
          : ['main'],
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

  // ADR 0013: what `update` returned besides the model. `pending` is what
  // the handler has not been handed yet, flushed after the next frame — so
  // an effect that dispatches synchronously lands in the next turn rather
  // than inside the frame that caused it, and one that reads the surface
  // sees the frame its cause produced. `unread` is the same effects kept
  // for `app.effects()`, the assertion point a headless test has whether or
  // not a handler ran; a real driver drains it every frame, as it drains
  // the core's own channels (ADR 0008, decision 6).
  const handler = typeof opts.effects === 'function' ? opts.effects : null;
  let pending = [];
  let unread = [];
  // A model changed since the last frame by a `dispatch` the loop did not
  // make itself — an effect handler's, or the app's own from a timer or a
  // promise. `step()` draws on it, the way it draws on an event: without
  // this a "loaded" message an effect dispatched sat in the model until
  // the next OS event happened to redraw (the devtools' tree tab asked for
  // a frame after its own and got none).
  let dirty = false;
  // Reads what `update` (or `init`) returned: a branded `withEffects` queues
  // its effects and sets the model unless that model is `undefined`, which
  // keeps the current one; anything else but `undefined` is the model.
  // Returns whether the model changed, which is what a tick redraws on.
  function apply(next) {
    if (next === undefined) return false;
    if (next !== null && typeof next === 'object' && next[EFFECTS] === true) {
      for (const e of next.effects) {
        pending.push(e);
        unread.push(e);
      }
      if (next.model === undefined) return false;
      model = next.model;
      dirty = true;
      return true;
    }
    model = next;
    dirty = true;
    return true;
  }
  // Hands the handler what `update` returned since the last flush. With no
  // handler the queue for it is simply dropped — `unread` still has them.
  function flushEffects() {
    if (pending.length === 0) return;
    const batch = pending;
    pending = [];
    if (!handler) return;
    for (const e of batch) handler(e, (msg) => app.dispatch(msg), surface);
  }

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
      if (apply(update(model, msg, { origin: 0, key: '', payload: msg }, surface))) {
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
  //
  // All but one kind: a `virtualColumn`'s sentinel says its container
  // scrolled, and the widget slices by the geometry the next frame reads for
  // itself. So it is a redraw and nothing else — an app that had to add a
  // `case 'layout'` for a widget to work has not been given a widget.
  function drainEvents() {
    const events = surface.pollEvents();
    for (const ev of events) {
      if (isVirtualLayout(ev.payload)) continue;
      app.dispatch(ev.payload, ev);
    }
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
    dirty = false;
    stamp?.(at() / 1000);
    const declared = windows ? windows(model) : undefined;
    for (const name of open()) {
      const tree = view(model, name, surface);
      show(name === 'main' ? withWindows(tree, declared) : tree, name);
    }
    drainWarnings();
    flushEffects();
  }

  // What `settled()` and `frame()` are waiting for (backlog F30). A driver
  // pumps from a timer the test cannot see into, so instead of guessing a
  // duration the test parks a waiter here and the pump answers it: `step()`
  // drains the list at the end of every turn, and a throw anywhere in the
  // pump rejects it. Empty unless something is awaiting.
  let waiters = [];

  // Nothing left to draw for: no transition, ghost or keyframe in flight,
  // and no effect the handler has not been handed. The second half matters
  // because an effect that dispatches is a model change the next turn owes
  // a frame to — a frame "settled" would otherwise promise had happened.
  function quiet() {
    return !surface.animating() && pending.length === 0;
  }

  // The pump has painted: every `frame` waiter is answered, and a `settled`
  // one when this frame left nothing moving — or when its cap has passed,
  // which resolves with `animating()` still true, the way `runOut` returns
  // `maxMs` headless rather than throwing.
  function drainWaiters() {
    if (waiters.length === 0) return;
    const still = quiet();
    const t = at();
    const keep = [];
    for (const w of waiters) {
      if (w.frame) w.resolve();
      else if (still || t - w.started >= w.maxMs) w.resolve(t - w.started);
      else keep.push(w);
    }
    waiters = keep;
  }

  // A pump that threw, or a window that closed, takes the waiters with it.
  function failWaiters(err) {
    const all = waiters;
    waiters = [];
    for (const w of all) w.reject(err);
  }

  // Both promises are answered from inside the driver's pump, so a loop that
  // holds its own clock has nobody to answer them — it moves time by hand,
  // and `runOut` is the same wait, synchronous. Refused rather than hung,
  // the way `advance` refuses a wall clock.
  function mustPump(name) {
    if (!clock) {
      throw new Error(
        `kui: ${name}() waits on the driver's pump; this loop moves its own clock — app.runOut() / app.advance(ms)`,
      );
    }
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
      apply(update(model, msg, event, surface));
    },
    /** What `update` (and `init`) returned besides the model since the
     *  last drain — every effect, whether or not a handler ran. Headless
     *  this is the assertion point; a window drains it every frame. */
    effects() {
      const out = unread;
      unread = [];
      return out;
    },
    render() {
      draw();
      return surface.stats();
    },
    /** One turn of the loop: whatever the surface queued, then the ticks the
     *  clock owes, then a frame if either changed the model. The windowed
     *  driver runs one after every `win.pump()`.
     *
     *  Returns whether the turn drew — which is how the windowed driver
     *  knows the app is being used, and paces itself from it. */
    step() {
      let redraw = false;
      try {
        drainWarnings();
        redraw = drainEvents();
        if (ticksTo(at(), false)) redraw = true;
        if (dirty) redraw = true;
        // A tick that returned effects and no model draws nothing, so the
        // step that owed them is what hands them on.
        if (redraw) draw();
        else flushEffects();
      } catch (e) {
        failWaiters(e);
        throw e;
      }
      // The turn is over, so whatever was waiting for one has its answer.
      drainWaiters();
      return redraw;
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
    /** The windowed `runOut`: resolves the first time a pump plus its step
     *  leaves nothing animating and nothing queued, with the wall-clock
     *  milliseconds it waited — and at `maxMs` with `animating()` still
     *  true, as `runOut` returns its cap. A promise rather than a loop
     *  because a window's clock is the wall's: `advance` cannot move it, so
     *  the driver's pump is the only thing that can say a frame happened
     *  (backlog F30). */
    settled(maxMs = 10_000) {
      mustPump('settled');
      must('animating');
      const started = at();
      return new Promise((resolve, reject) => {
        waiters.push({ frame: false, started, maxMs, resolve, reject });
      });
    },
    /** How long the windowed driver may wait before the next pump, in ms —
     *  the loop answering for its own timing. `idleMs` is what it would
     *  like to wait; a tick due sooner shortens it, and `animating` cuts it
     *  to the frame cadence, since a gap past the next frame is a dropped
     *  frame. The driver passes `animating` in rather than letting this ask
     *  again: it has just read it to pace itself, and one turn should not
     *  decide two things from two answers. */
    [BUDGET](busyMs, idleMs, animating) {
      if (animating) return busyMs;
      const untilTick = nextTick - at();
      return Math.max(busyMs, Math.min(idleMs, untilTick));
    },
    /** One more pump has painted. The cheap half of `settled` — what a test
     *  that only needs the window to have drawn *something* was buying with
     *  a `setTimeout`. */
    frame() {
      mustPump('frame');
      return new Promise((resolve, reject) => {
        waiters.push({ frame: true, resolve, reject });
      });
    },
    /** The driver's door to those waiters; see `FAILED`. */
    [FAILED](err) {
      failWaiters(err);
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
  // `init` may return `withEffects` too: the effect an app starts with — a
  // file to open, a request to send — has nowhere else to go.
  apply(typeof init === 'function' ? init(surface) : init);
  return app;
}

/**
 * Opens a real kui window (winit + wgpu) and runs the loop against it.
 * The winit event loop is pumped from the main thread between libuv turns,
 * so Node stays fully responsive while the window is open.
 *
 * Resolves with the final model when the main window closes. One event
 * loop per process (winit event loops are not recreatable everywhere); any
 * number of windows on it — `windows: (model) => [...]` declares them, and
 * `view(model, window, win)` is called once per open window.
 *
 * **What the loop costs when nothing is happening.** A pump is not free and
 * costs the same empty as full: on macOS it runs a whole `NSApp` iteration
 * and posts a synthetic event through the window server to break out of it
 * again, about 1.2 ms of CPU. The rate is therefore the whole bill, and it
 * is linear — at a pump every 8 ms an idle window costs 8-15% of a core,
 * at 16 ms half that, at 100 ms a tenth. So the gap between pumps grows
 * while nothing happens: `pumpMs` (8) while the app is being used, doubling
 * once it has been quiet for `quietMs` (500), up to `idlePumpMs` (32).
 * Anything at all — an event, a tick, a frame, a transition, and any OS
 * event the app itself never sees — puts it back to `pumpMs` on the spot.
 *
 * `idlePumpMs` is the whole trade and it was measured both ways, clicking a
 * real window with posted `CGEvent`s. A driver that never backs off answers
 * a click in 34 ms; at 32 ms it answers in ~55 ms, and an idle window costs
 * ~3% of a core instead of ~9%. Raising it is linear in both directions.
 * The delay is only ever paid by the *first* event after half a second of
 * complete silence — the second is already back at `pumpMs`.
 *
 * The gap is only ever the *ceiling*. `win.nextDeadlineMs()` is the runner
 * saying when it next needs pumping — the caret blink, a transition's next
 * frame, the audio poll, a window's first-frame retry — and the driver
 * sleeps to that instead whenever it is sooner, so a deadline the shell
 * worked out precisely is not then missed by a whole interval. What it
 * cannot say is whether an OS event is waiting, which is why there is a
 * ceiling at all.
 *
 * The cost of the backoff is that an OS event arriving into a *deep* idle
 * waits for the next pump, up to that gap. In practice a pointer moves
 * before it clicks and the first move has already reset it, so what pays
 * is the first event after a window has been left alone — raise
 * `idlePumpMs` to spend less and feel that more.
 *
 * What the driver deliberately does *not* do is park inside the pump.
 * `KuiWindow.pumpUntil` blocks until an OS event, which sounds strictly
 * better — the wake is the event itself, so there is no latency at all —
 * and it is wrong twice over. A blocked main thread is a blocked libuv:
 * 200 sequential `await readFile` went from 6 ms to 4 s behind a 50 ms
 * park. And it does not even save CPU, because winit stops a parked pump
 * on the first wake of any kind: a 200 ms park runs 101 ms, and a 32 ms
 * period costs 4.59% of a core parked against 1.80% polled. Parking buys
 * latency and nothing else; a driver that owns its whole process and does
 * no async I/O can spend that way, but this one cannot assume either.
 */
export function runWindowed(config, opts = {}) {
  const { width, height, minWidth, minHeight, maxWidth, maxHeight, chrome } = opts;
  const win = new KuiWindow(opts.title ?? 'kui', {
    width, height, minWidth, minHeight, maxWidth, maxHeight, chrome,
  });
  const app = createLoop(config, opts, win, opts.clock ?? Date.now);
  const busyMs = opts.pumpMs ?? 8;
  const idleMs = Math.max(opts.idlePumpMs ?? 32, busyMs);
  const quietMs = opts.quietMs ?? 500;
  // How long to wait before the next pump, and since when there has been
  // nothing to pump for.
  let gap = busyMs;
  let quietSince = 0;
  app.render();
  return new Promise((resolve, reject) => {
    const pump = () => {
      let alive;
      let worked;
      let animating;
      try {
        alive = win.pump();
        // `step` draws if anything arrived; a transition draws without it.
        animating = win.animating();
        worked = app.step() || animating;
        // A real driver drains every channel every frame, the app's
        // effects included: the handler had them at the frame, and what
        // `effects()` keeps is for a headless test.
        app.effects();
      } catch (e) {
        // `step` has already rejected what it was holding; this is for a
        // throw out of `win.pump()` itself, which happens before it.
        app[FAILED](e);
        reject(e);
        return;
      }
      if (!alive) {
        // No further frames will be painted, so an awaited one is told
        // rather than left waiting for a pump that has stopped.
        app[FAILED](new Error('kui: the window closed while a frame was awaited'));
        resolve(app.model);
        return;
      }
      // When the runner next wants pumping. Zero means it has an event's
      // redraw to present — which is also the only way this driver hears
      // about OS input the *app* never sees: a pointer crossing a window
      // that declares no hover produces no app event at all, and a driver
      // pacing itself on app events alone reads that as an idle window and
      // goes on backing off while somebody is reaching for a button. It
      // measured 520 ms from click to update before this counted as work,
      // against 34 ms for a driver that never backs off.
      const due = win.nextDeadlineMs();
      const now = Date.now();
      if (worked || (due !== null && due <= 1)) {
        gap = busyMs;
        quietSince = 0;
      } else if (!quietSince) {
        quietSince = now;
      } else if (now - quietSince >= quietMs) {
        // Doubling rather than jumping: the window that just went quiet is
        // the one most likely to be used again in a moment, and it keeps
        // its cadence for the first half-second either way.
        gap = Math.min(gap * 2, idleMs);
      }
      // The loop's own timing (ticks, anything animating) capped by the
      // backoff, and then the runner's deadline if it is sooner. Floored at
      // 1ms rather than `busyMs`: every deadline the shell reports is one it
      // set in the future and recomputes each pump, so there is no gap to
      // spin in — and rounding a 2 ms blink up to 8 would put the precision
      // back where it was.
      let wait = app[BUDGET](busyMs, gap, animating);
      if (due !== null) wait = Math.min(wait, Math.max(1, due));
      setTimeout(pump, wait);
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
// -- Virtual lists ----------------------------------------------------------
// The core builds every child a view declares, so a ten-thousand-row list
// costs ten thousand rows of build and layout on every frame. A view that
// knows how tall the container is and how far it is scrolled can declare a
// screenful and two spacers instead — `ctx.scrollGeometry(key)` is that
// knowledge, and `widgets::virtual_column` is this same arithmetic in Rust.
//
// What Rust does not need and this does: a *reason to run again*. The wheel
// moves the core's retained offset and raises no event, and a window redraws
// by re-lowering the tree it was last handed — so a view that sliced by the
// geometry once would slice by it once and never again. The zero-height
// first child below has an `onLayout` whose rect changes whenever the
// content moves, and `createLoop` redraws on it *without* handing it to
// `update`: it is the widget's own bookkeeping, not a message the app wrote.

/** The tag on that sentinel — a string key so it survives the wire as the
 *  plain data every payload is, and one nothing else would spell. */
const VIRTUAL_TAG = '@kui/virtual';

/** Whether an event is one of those sentinels rather than an app message. */
function isVirtualLayout(payload) {
  return (
    payload !== null &&
    typeof payload === 'object' &&
    payload.kind === 'layout' &&
    payload.tag !== null &&
    typeof payload.tag === 'object' &&
    typeof payload.tag[VIRTUAL_TAG] === 'string'
  );
}

const spacer = (h, key) => ({
  type: 'box',
  key,
  props: { width: 'grow', height: h },
  children: [],
});

/**
 * A vertically scrolling column of `rows` uniform rows that declares only
 * the visible ones, in JSX.
 *
 *   virtualColumn(ctx, { key: 'log', rows: lines.length, rowH: 28, bg: '#111' },
 *     (i) => <box width="grow" height="grow" onClick={{ kind: 'pick', row: i }}>
 *              <text>{lines[i]}</text>
 *            </box>)
 *
 * `ctx` is the surface `view(model, window, ctx)` is handed. `key` names the
 * container — the geometry is read back by that name, so it has to be one
 * (two lists on one name is the `ambiguous-key` warning). Every other prop
 * is the container's own; it is forced to `scrollY` with no `gap`, because
 * `rowH` is the whole stride and spacing belongs inside a row that pads
 * itself.
 *
 * `row(i)` returns row `i`'s *contents*. The widget owns the row's own node:
 * `rowH` tall and keyed by the row's data index (`index`), so a row keeps
 * its hover, focus, edit buffer and tweens as the built range slides over
 * it, and so a virtualised list and a full one agree on identity. A
 * clickable row puts its `onClick` on a `width="grow" height="grow"` child,
 * which is what the example above does.
 *
 * The geometry it slices by is the previous frame's, so the first frame —
 * before any layout has resolved the container — slices by the viewport
 * instead, and a resize is one frame late and covered by `overscan` (two
 * rows each side by default).
 *
 * To reach a row that is not built, scroll to it: `ctx.setScroll(key, 0, i *
 * rowH)` puts row `i` at the top. `ctx.reveal` of an unbuilt row finds
 * nothing, because nothing declared it.
 */
export function virtualColumn(ctx, opts, row) {
  const { key, rows, rowH, overscan = 2, ...box } = opts ?? {};
  if (typeof key !== 'string' || key === '') {
    throw new Error('kui: virtualColumn needs a string `key` — its geometry is read back by that name');
  }
  if (typeof rowH !== 'number' || !(rowH > 0)) {
    throw new Error('kui: virtualColumn needs a positive `rowH` — one row\'s height is the whole stride');
  }
  // Said rather than coerced: `rows` misspelled draws an empty list, which
  // looks like a broken widget rather than a typo.
  if (typeof rows !== 'number' || !Number.isFinite(rows) || rows < 0) {
    throw new Error('kui: virtualColumn needs a `rows` count — how many rows the list has, built or not');
  }
  if (typeof row !== 'function') {
    throw new Error('kui: virtualColumn needs a row builder — virtualColumn(ctx, opts, (i) => node)');
  }
  const n = Math.floor(rows);
  // Null until a layout has resolved the container, which is the first
  // frame: a container is not taller than the window in the ordinary case,
  // so a screenful of the viewport is a safe over-build for one frame.
  const g = ctx.scrollGeometry(key);
  const vh = Math.max(0, g ? g.h : ctx.env().viewport.height);
  // Layout puts the flow's origin at `padT - offset`, so the visible band
  // starts there. Only the top padding shifts it; the shorthand family
  // falls back the same way `PadShorthand` does in the core.
  const padT = box.padT ?? box.padY ?? box.pad ?? 0;
  const top = (g ? g.offset.y : 0) - padT;
  // Both ends are clamped to the list, `first` included: the geometry is the
  // previous frame's, so a list that shrank under its own scroll offset
  // slices past its new end. Left unclamped that builds a lead spacer taller
  // than the whole list and no rows at all, and the oversized spacer keeps
  // the offset legal, so it unwinds one viewport a frame instead of landing
  // in one. `widgets::visible_rows` clamps the same way.
  const first = Math.min(n, Math.max(0, Math.floor(top / rowH) - overscan));
  const last = Math.max(first, Math.min(n, Math.ceil((top + vh) / rowH) + overscan));

  const children = [
    {
      type: 'box',
      key: 'kui:at',
      props: { width: 'grow', height: 0, onLayout: { [VIRTUAL_TAG]: key } },
      children: [],
    },
  ];
  // Keyed, not auto-keyed: an auto key *is* the sibling index, and the rows
  // already occupy that namespace at their data indices.
  if (first > 0) children.push(spacer(first * rowH, 'kui:lead'));
  for (let i = first; i < last; i++) {
    children.push({
      type: 'box',
      props: { index: i, width: 'grow', height: rowH },
      children: row(i),
    });
  }
  if (last < n) children.push(spacer((n - last) * rowH, 'kui:tail'));

  return {
    type: 'box',
    key,
    props: { ...box, scrollY: true, gap: 0 },
    children,
  };
}

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
      // Which entry of `decodeClips(ctx.clips())` clips this quad. An
      // index rather than the clip itself since ABI 11: a clip is 32 bytes
      // and a frame has a handful of them, so carrying one per quad was
      // paid by every quad of every frame. Entry 0 clips nothing.
      clip: u[19],
      uv: [u[20], u[21], u[22], u[23]],
      // Segment quads (kind 6) only: the endpoints `uv` carries as float
      // bits, x0, y0, x1, y1 in physical px; `borderW` is the stroke width.
      ends: u[18] === 6 ? [f[20], f[21], f[22], f[23]] : null,
    });
  }
  return quads;
}

/**
 * Decodes `ctx.clips()` into JS objects (debugging/testing aid).
 * Layout mirrors KuiClip in include/kui.h; coordinates are physical px.
 * A quad's `clip` indexes this list.
 */
export function decodeClips(buffer) {
  const stride = clipStride();
  const clips = [];
  for (let off = 0; off + stride <= buffer.byteLength; off += stride) {
    const f = new Float32Array(buffer.buffer, buffer.byteOffset + off, stride / 4);
    clips.push({
      rect: [f[0], f[1], f[2], f[3]],
      // Corner radii clockwise from the top-left: a clipping node with a
      // radius rounds what it clips. All zero = a plain rect clip.
      radii: [f[4], f[5], f[6], f[7]],
    });
  }
  return clips;
}
