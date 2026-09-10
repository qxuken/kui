// The shipped `.d.ts`, exercised: a typed drive that uses the app-facing
// types the type checker would otherwise never see used — `App`, `Ctx`,
// `CoreMsg` and the message union, `KeyMsg` with a tag, `UiEvent`, the
// access messages, `decodeQuads`. `npm run typecheck` is what runs it as
// types; `--headless` runs it as code and exits non-zero on a wrong answer,
// since a stale assumption typechecks fine (the `autofocus` one from
// 0.1.0-alpha.5 did). A tool, not an example: nothing here is a subject.
//
//   node dist/tools/types.mjs --headless
import { createApp, decodeQuads, Ctx } from '@qxuken/kui';
import type { App, CoreMsg, KeyMsg, Theme, UiEvent } from '@qxuken/kui';

type Model = { count: number; note: string };
type CounterMsg = { kind: 'add'; by: number } | { kind: 'reset' };
type Msg = CounterMsg | CoreMsg;

const init: Model = { count: 0, note: '' };

function update(model: Model, msg: Msg, ev: UiEvent<Msg>): Model | undefined {
  switch (msg.kind) {
    case 'add':
      return { ...model, count: model.count + msg.by };
    case 'reset':
      return { ...model, count: 0 };
    case 'changed':
      return { ...model, note: app.ctx.editText(ev.key) ?? '' };
    case 'access': {
      // A screen reader nudged the slider. `msg.tag` is the slider's own
      // payload, typed as this app's union — no cast — so the step the
      // node declared is the step the nudge takes.
      const tag = msg.tag;
      const by = tag && 'kind' in tag && tag.kind === 'add' ? tag.by : 1;
      return { ...model, count: model.count + (msg.action === 'increment' ? by : -by) };
    }
  }
}

function Counter({ count, t }: { count: number; t: Theme }) {
  return (
    <box dir="row" gap={12} crossAlign="center">
      <button onClick={{ kind: 'add', by: 1 }} accent>+1</button>
      <button onClick={{ kind: 'add', by: -1 }}>-1</button>
      <button onClick={{ kind: 'reset' }} description="Back to zero">reset</button>
      <text size={20} color={t.fg}>{`count = ${count}`}</text>
      {/* The count as a slider, for assistive technology: the range it
          declares is the range the value is held to, or the core warns
          (`slider-value-out-of-range`). Its payload is what a nudge
          carries back. */}
      <box key="tally" role="slider" label="count" valueNow={count} valueMin={0} valueMax={10}
           onClick={{ kind: 'add', by: 1 }} width={100} height={8} bg={t.sunken} radius={4} />
    </box>
  );
}

const view = (model: Model) => {
  const t = app.ctx.theme();
  return (
    <box pad={24} gap={16} bg={t.bg} width="grow" height="grow">
      <text size={24} color={t.fg}><span bold color={t.accent}>kui</span> × Node × JSX</text>
      <Counter count={model.count} t={t} />
      <edit key="note" label="note" initial="" size={16} width={280} padX={10} padY={6}
            bg={t.sunken} color={t.fg} radius={4} autofocus />
      <text size={14} color={t.muted}>{`note: ${model.note || '(empty)'}`}</text>
    </box>
  );
};

const app: App<Model, Msg> = createApp({ init, update, view }, { width: 640, height: 480 });

if (!process.argv.includes('--headless')) {
  console.log('types: a typecheck fixture; run with --headless to execute it');
  process.exit(0);
}

const stats = app.render();
console.log(`frame: ${stats.quadCount} quads @ ${stats.viewportW}x${stats.viewportH}`);

// Buttons are the radius-6 solid quads, in row order: +1, -1, reset.
const buttons = decodeQuads(app.ctx.quads())
  .filter((q) => q.kind === 0 && q.radius === 6)
  .sort((a, b) => a.x - b.x);
const center = (q: { x: number; y: number; w: number; h: number }) =>
  [q.x + q.w / 2, q.y + q.h / 2] as const;

app.click(...center(buttons[0])); // +1
app.click(...center(buttons[0])); // +1
app.click(...center(buttons[1])); // -1

// The editor is declared `autofocus`, but that only takes the keyboard
// while nothing else holds it (ADR 0002) — the three clicks above left
// focus on a button. So move it there by name.
app.ctx.focus('note');
app.type('hello from node');
app.key('backspace', { word: true });
app.type('jsx');
app.click(...center(buttons[2])); // reset
app.access('tally', 'decrement');

// A fresh context: images and a key sink with a tag, both halves of a key.
const modal = new Ctx();
const pixels = Buffer.alloc(8 * 8 * 4);
for (let i = 0; i < 8 * 8; i++) pixels.writeUInt32BE(0xff8800ff, i * 4);
const img = modal.addImage(8, 8, pixels);
modal.frame(240, 240, 1, (
  <box pad={12} gap={8} keyFocus onKey={{ tool: 'brush' }} keyUp>
    <image src={img} width={32} radius={4} label="swatch" />
    <text><span bold>bold</span> and <span italic color={modal.theme().danger}>red italic</span></text>
  </box>
));
const imageQuads = decodeQuads(modal.quads()).filter((q) => q.kind === 3).length;
modal.keyDown('x', { ctrl: true });
modal.keyUp('x', { ctrl: true });
const kevs = modal.pollEvents<KeyMsg<{ tool: string }>>().filter((e) => e.payload.kind === 'key');
const p = kevs[0]?.payload;
const up = kevs[1]?.payload;

const checks: [boolean, string][] = [
  [app.model.count === -1, 'clicks by quad, a reset, and a decrement nudge through the slider\'s own tag'],
  [app.model.note === 'hello from jsx', 'typing, and a word-wise backspace'],
  [imageQuads === 1, 'an image is one image quad'],
  [p?.code === 'x' && p?.ctrl === true && p?.tag?.tool === 'brush', 'a key sink hears the chord with its tag'],
  [p?.phase === 'down' && up?.phase === 'up' && up?.text === null, 'both halves of the key, with keyUp'],
];
let ok = true;
for (const [cond, what] of checks) {
  console.log(`  ${cond ? 'ok  ' : 'FAIL'} ${what}`);
  ok &&= cond;
}
console.log(ok ? 'types: headless drive OK' : 'types: FAILED');
process.exit(ok ? 0 : 1);
