// kui from Node, Elm-style: view(model) is JSX, messages are plain data.
// Runs headlessly - builds real frames, clicks real buttons via hit-testing,
// types into a real editor - and prints what happened.
import { createApp, decodeQuads, Ctx } from '@qxuken/kui';
import type { App, CoreMsg, KeyMsg, UiEvent } from '@qxuken/kui';

type Model = {
  count: number;
  note: string;
  /** Where the last right-click landed, while its menu is open. */
  menu: { x: number; y: number } | null;
};

// The payloads this app's own nodes carry, plus the ones the core sends on
// its own (`changed`, `key`, `hover`, `drag`, ...): one flat discriminated
// union, so `update` switches over `msg.kind` with no casts and no
// "is this even an object" preamble.
type CounterMsg = { kind: 'add'; by: number } | { kind: 'reset' };
type Msg = CounterMsg | CoreMsg;

const init: Model = { count: 0, note: '', menu: null };

function update(model: Model, msg: Msg, ev: UiEvent<Msg>): Model | undefined {
  switch (msg.kind) {
    case 'add':
      // Either counter button, or the menu's — which closes the menu, the
      // way choosing an item does everywhere.
      return { ...model, count: model.count + msg.by, menu: null };
    case 'reset':
      return { ...model, count: 0, menu: null };
    case 'changed':
      // An editor changed; read its text back through the event's node key.
      return { ...model, note: app.ctx.editText(ev.key) ?? '' };
    case 'contextmenu':
      // A right-click: the core opens nothing, the view declares the menu
      // where the press landed.
      return { ...model, menu: { x: msg.x, y: msg.y } };
    case 'dismiss':
      // Escape, or a press outside the menu.
      return { ...model, menu: null };
  }
}

/** The context menu: a modal float at the press. `modal` scopes Tab and
 *  the pointer to it and brings the `dismiss` above back. */
function Menu({ at }: { at: { x: number; y: number } }) {
  return (
    <box
      float={{ anchor: 'viewport', at: ['start', 'start'], self: ['start', 'start'],
               dx: at.x, dy: at.y, fit: true }}
      modal={{ kind: 'menu' }}
      label="Actions"
      pad={4} gap={4} width={120} bg="#22242c" radius={6}
    >
      <button onClick={{ kind: 'add', by: 10 }}>+10</button>
      <button onClick={{ kind: 'reset' }}>reset</button>
    </box>
  );
}

function Counter({ count }: { count: number }) {
  return (
    <box dir="row" gap={12} crossAlign="center">
      <button onClick={{ kind: 'add', by: 1 }}>+1</button>
      <button onClick={{ kind: 'add', by: -1 }}>-1</button>
      <button onClick={{ kind: 'reset' }}>reset</button>
      <text size={20} color="#e8e8f0">{`count = ${count}`}</text>
    </box>
  );
}

const view = (model: Model) => (
  <box pad={24} gap={16} bg="#14141c" width="grow" height="grow" onContextMenu={{ kind: 'menu' }}>
    <text size={24} color="#ffffff"><span bold color="#7aa2ff">kui</span> × Node × JSX</text>
    <Counter count={model.count} />
    <edit key="note" label="note" initial="" size={16} width={280} padX={10} padY={6}
          bg="#1f2030" color="#e8e8f0" radius={4} autofocus />
    <text size={14} color="#99a0b0">{`note: ${model.note || '(empty)'}`}</text>
    {model.menu ? <Menu at={model.menu} /> : null}
  </box>
);

const app: App<Model, Msg> = createApp({ init, update, view }, { width: 640, height: 480 });

// ---------------------------------------------------------------------------
// Headless drive

const stats = app.render();
console.log(`frame: ${stats.quadCount} quads @ ${stats.viewportW}x${stats.viewportH}`);

// Buttons are the radius-6 solid quads, in row order: +1, -1, reset.
const buttons = decodeQuads(app.ctx.quads())
  .filter((q) => q.kind === 0 && q.radius === 6)
  .sort((a, b) => a.x - b.x);
console.log(`found ${buttons.length} buttons`);
const center = (q: { x: number; y: number; w: number; h: number }) =>
  [q.x + q.w / 2, q.y + q.h / 2] as const;

app.click(...center(buttons[0])); // +1
app.click(...center(buttons[0])); // +1
app.click(...center(buttons[1])); // -1
console.log(`after +1 +1 -1: count = ${app.model.count}`);

// The editor is declared `autofocus`, but that only takes the keyboard
// while nothing else holds it (ADR 0002) — the three clicks above left
// focus on a button. So click into it first, the way a user would: a press
// moves focus and places the caret, and the field is still empty, so the
// caret lands at the start either way. The access tree is the tidiest way
// to find it: it carries each node's rect, no quad archaeology needed.
const note = app.accessTree().nodes.find((n) => n.name === 'note');
if (!note) throw new Error('the editor is missing from the access tree');
app.click(...center(note.rect));
app.type('hello from node');
console.log(`note: "${app.model.note}"`);

app.key('backspace', { word: true });
app.type('jsx');
console.log(`after alt-backspace + "jsx": "${app.model.note}"`);

app.click(...center(buttons[2])); // reset
console.log(`after reset: count = ${app.model.count}`);

// A right-click on the background opens the menu where it landed, and does
// not disturb the editor holding focus.
app.rightClick(500, 400);
console.log(`menu at ${JSON.stringify(app.model.menu)}, note still "${app.model.note}"`);
// Its items are the rounded quads inside the menu's own 120-wide box,
// top to bottom: +10 first.
const items = decodeQuads(app.ctx.quads())
  .filter((q) => q.kind === 0 && q.radius === 6 && q.x > 500 && q.w < 120)
  .sort((a, b) => a.y - b.y);
app.click(...center(items[0]));
console.log(`after the menu's +10: count = ${app.model.count}, menu = ${app.model.menu}`);

// ---------------------------------------------------------------------------
// Part 2: images + key sinks (a fresh context so no editor holds focus)

const modal = new Ctx();
const pixels = Buffer.alloc(8 * 8 * 4);
for (let i = 0; i < 8 * 8; i++) pixels.writeUInt32BE(0xff8800ff, i * 4);
const img = modal.addImage(8, 8, pixels);

modal.frame(240, 240, 1, (
  <box pad={12} gap={8} keyFocus onKey={{ tool: 'brush' }} keyUp>
    <image src={img} width={32} radius={4} label="swatch" />
    <text><span bold>bold</span> and <span italic color="#ff8888">red italic</span></text>
  </box>
));
const imageQuads = decodeQuads(modal.quads()).filter((q) => q.kind === 3).length;
console.log(`image quads: ${imageQuads}`);

modal.keyDown('x', { ctrl: true });
modal.keyUp('x', { ctrl: true });
// Poll for one known payload shape: a key, both halves (the sink asked for
// releases with `keyUp`), carrying this sink's tag. `phase` is what tells a
// press from a release — one shape, two events.
const kevs = modal.pollEvents<KeyMsg<{ tool: string }>>().filter((e) => e.payload.kind === 'key');
const p = kevs[0]?.payload;
const up = kevs[1]?.payload;
console.log(`key sink got: code=${p?.code} ctrl=${p?.ctrl} tag=${JSON.stringify(p?.tag)}`);
console.log(`phases: ${kevs.map((e) => e.payload.phase).join(', ')}`);

const ok =
  app.model.count === 10 &&
  app.model.menu === null &&
  app.model.note === 'hello from jsx' &&
  imageQuads === 1 &&
  p?.code === 'x' &&
  p?.ctrl === true &&
  p?.tag?.tool === 'brush' &&
  p?.phase === 'down' &&
  up?.phase === 'up' &&
  up?.text === null;
console.log(ok ? 'OK' : 'MISMATCH');
process.exit(ok ? 0 : 1);
