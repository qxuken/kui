// The counter in a real window, from Node: winit + wgpu underneath, the
// Elm loop in JS on top, both sharing the main thread via a pumped event
// loop. Run with --smoke to auto-close after 2 seconds (CI/sanity).
import { runWindowed } from '@qxuken/kui';
import type { KuiWindow, Msg, UiEvent } from '@qxuken/kui';

type Model = { count: number; note: string };

const init: Model = { count: 0, note: '' };

function update(model: Model, msg: Msg, ev: UiEvent, win: KuiWindow): Model | undefined {
  if (msg === null || typeof msg !== 'object' || Array.isArray(msg)) return;
  switch ((msg as { kind: string }).kind) {
    case 'add':
      return { ...model, count: model.count + (msg as { by: number }).by };
    case 'reset':
      return { ...model, count: 0 };
    case 'changed':
      return { ...model, note: win.editText(ev.key) ?? '' };
  }
}

// Registered in setup() before the first frame; drawn with <image src={..}>.
let gradient = '';

const view = (model: Model) => (
  <box gap={16} bg="#14141c" width="grow" height="grow" title={`kui counter — ${model.count}`}>
    <titlebar title="kui counter" />
    <box pad={24} gap={16}>
    <text size={24} color="#ffffff"><span bold color="#7aa2ff">kui</span> × Node × JSX</text>
    <image src={gradient} width={128} radius={8} />
    <box dir="row" gap={12} crossAlign="center">
      <button onClick={{ kind: 'add', by: 1 }}>+1</button>
      <button onClick={{ kind: 'add', by: -1 }}>-1</button>
      <button onClick={{ kind: 'reset' }}>reset</button>
      <text size={20} color="#e8e8f0">{`count = ${model.count}`}</text>
    </box>
    <edit key="note" initial="" size={16} width={280} padX={10} padY={6}
          bg="#1f2030" color="#e8e8f0" radius={4} autofocus />
    <text size={14} color="#99a0b0">{`note: ${model.note || '(empty)'}`}</text>
    </box>
    <latencyHud />
  </box>
);

const done = runWindowed({ init, update, view }, {
  title: 'kui counter',
  width: 640,
  height: 480,
  chrome: 'custom',
  setup(win) {
    const [w, h] = [64, 32];
    const px = Buffer.alloc(w * h * 4);
    for (let y = 0; y < h; y++) {
      for (let x = 0; x < w; x++) {
        const i = (y * w + x) * 4;
        px[i] = Math.round((x / (w - 1)) * 255);
        px[i + 1] = 0x66;
        px[i + 2] = Math.round((y / (h - 1)) * 255);
        px[i + 3] = 0xff;
      }
    }
    gradient = win.addImage(w, h, px);
  },
});

if (process.argv.includes('--smoke')) {
  setTimeout(() => process.exit(0), 2000);
}

const finalModel = await done;
console.log('window closed, final model:', JSON.stringify(finalModel));
