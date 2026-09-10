// A mind map you pan by dragging the canvas: float-positioned cards under
// an `onDrag` root, `line` connectors between them, and every one of them
// easing its position with `transition` + `slide` — a canvas of floats
// eases everything or nothing (see `slide` in docs/props.md).
//
// It is here because that combination is what backlog F15 was: a target the
// view moves on *every* frame used to restart its tween every frame and
// never advance, so the pan was live in the model and frozen on screen —
// invisible to a headless test, which asserts the model. Drag the empty
// canvas and the map has to follow the cursor while the button is down,
// not jump into place when it comes up.
//
//   npm run mindmap                       a real window to drag by hand
//   node dist/mindmap.mjs --headless      the same gesture, no window
//   node dist/mindmap.mjs --trace         print every event the loop gets
import { createApp, runWindowed } from '@qxuken/kui';
import type { CoreMsg, Theme, UiEvent } from '@qxuken/kui';

type Card = { id: string; x: number; y: number; label: string };
type Model = {
  pan: { x: number; y: number };
  from: { x: number; y: number } | null;
  panAt: { x: number; y: number };
  log: string;
};

type AppMsg = { kind: 'pan' } | { kind: 'card'; id: string };
type Msg = AppMsg | CoreMsg;

const CARDS: Card[] = [
  { id: 'a', x: 60, y: 60, label: 'root' },
  { id: 'b', x: 300, y: 40, label: 'still open' },
  { id: 'c', x: 280, y: 200, label: 'done' },
  { id: 'd', x: 90, y: 260, label: 'later' },
];
const LINKS: [string, string][] = [['a', 'b'], ['a', 'c'], ['a', 'd']];
const EASE = 160;

const TRACE = process.argv.includes('--trace');
const card = (id: string) => CARDS.find((c) => c.id === id)!;

const init: Model = { pan: { x: 0, y: 0 }, from: null, panAt: { x: 0, y: 0 }, log: 'drag the canvas' };

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>): Model | undefined {
  if (TRACE) console.log('ev', JSON.stringify(msg));
  if (msg.kind !== 'drag') return undefined;
  const { phase, x, y } = msg;
  if (phase === 'start') {
    return { ...model, from: { x, y }, panAt: model.pan, log: `start ${Math.round(x)},${Math.round(y)}` };
  }
  const from = model.from;
  if (!from) return { ...model, log: `${phase} with no start` };
  // Absolute x/y against the press point, so no delta is ever summed.
  const pan = { x: model.panAt.x + (x - from.x), y: model.panAt.y + (y - from.y) };
  return {
    ...model,
    pan,
    from: phase === 'end' ? null : from,
    log: `${phase} pan ${Math.round(pan.x)},${Math.round(pan.y)}`,
  };
}

/// Whichever driver is running — the headless `Ctx` or the real window —
/// set before the first frame. Both spell the palette the same way, so
/// the view reads roles rather than hex (ADR 0019).
let ui: { theme(): Theme } | null = null;

const view = (model: Model) => {
  const t = ui!.theme();
  const at = (c: Card) => ({ x: c.x + model.pan.x, y: c.y + model.pan.y });
  return (
    <box width="grow" height="grow" bg={t.bg} onDrag={{ kind: 'pan' }} title="kui mindmap">
      {LINKS.map(([a, b]) => {
        const p = at(card(a));
        const q = at(card(b));
        return (
          <line
            key={`${a}${b}`}
            from={[p.x + 40, p.y + 18]}
            to={[q.x + 40, q.y + 18]}
            width={2}
            color={t.borderStrong}
            transition={EASE}
            slide
          />
        );
      })}
      {CARDS.map((c) => {
        const p = at(c);
        return (
          <box
            key={c.id}
            float={{ anchor: 'parent', dx: p.x, dy: p.y }}
            padX={14}
            padY={10}
            radius={8}
            bg={t.raised}
            hoverBg={t.accentSoft}
            transition={EASE}
            slide
            onClick={{ kind: 'card', id: c.id }}
          >
            <text size={15} color={t.fg}>{c.label}</text>
          </box>
        );
      })}
      <box float={{ anchor: 'parent', dx: 8, dy: 8 }} padX={8} padY={4} bg={t.sunken} radius={4}>
        <text size={13} color={t.success}>{model.log}</text>
      </box>
    </box>
  );
};

// The headless half of the same gesture: the model pans, which is all a
// test can see — the frozen half F15 was about lives in the drawn frame,
// and `advance` is what makes it observable (see packages/kui/test.mjs).
if (process.argv.includes('--headless')) {
  const app = createApp({ init, update, view }, { width: 640, height: 480 });
  ui = app.ctx;
  app.render();
  app.ctx.cursor(400, 300);
  app.ctx.mouse(true, 1);
  app.settle();
  app.ctx.cursor(430, 340);
  app.settle();
  app.ctx.mouse(false, 1);
  app.settle();
  const { x, y } = app.model.pan;
  console.log('headless pan:', x, y, '|', app.model.log);
  if (x !== 30 || y !== 40) {
    console.error('MISMATCH: expected a pan of 30,40');
    process.exit(1);
  }
  process.exit(0);
}

const chrome = (process.argv.find((a) => a.startsWith('--chrome='))?.split('=')[1] ??
  'native') as 'native' | 'custom' | 'borderless';

const done = runWindowed({ init, update, view }, {
  title: 'kui mindmap',
  width: 640,
  height: 480,
  chrome,
  setup(win) {
    ui = win;
  },
});

if (process.argv.includes('--smoke')) setTimeout(() => process.exit(0), 2000);

const finalModel = await done;
console.log('window closed, final pan:', JSON.stringify(finalModel.pan));
