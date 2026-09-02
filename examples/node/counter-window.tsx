// The counter in a real window, from Node: winit + wgpu underneath, the
// Elm loop in JS on top, both sharing the main thread via a pumped event
// loop. Run with --smoke to auto-close after 2 seconds (CI/sanity).
import { runWindowed } from '@qxuken/kui';
import type { CoreMsg, KuiWindow, UiEvent, WindowSize } from '@qxuken/kui';

type Model = { count: number; note: string; size: WindowSize };

// This app's own payloads plus the core's (`changed`, `key`, `hover`, ...):
// one discriminated union, switched over directly.
type CounterMsg = { kind: 'add'; by: number } | { kind: 'reset' };
type Msg = CounterMsg | CoreMsg;

// setup() runs before init(), so the first model already knows the size the
// window actually opened at; `resize` messages keep it current after that.
let openedAt: WindowSize = { width: 0, height: 0, scale: 1 };

const init = (): Model => ({ count: 0, note: '', size: openedAt });

function update(model: Model, msg: Msg, ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  switch (msg.kind) {
    case 'add':
      return { ...model, count: model.count + msg.by };
    case 'reset':
      return { ...model, count: 0 };
    case 'changed':
      return { ...model, note: win.editText(ev.key) ?? '' };
    // Sent by the core when the window changes size or DPI; `win.size()`
    // answers the same at any time.
    case 'resize': {
      const { width, height, scale } = msg;
      return { ...model, size: { width, height, scale } };
    }
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
    <text size={14} color="#99a0b0">
      {`window: ${Math.round(model.size.width)}x${Math.round(model.size.height)} @ ${model.size.scale}x`}
    </text>
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
    openedAt = win.size();
  },
});

if (process.argv.includes('--smoke')) {
  setTimeout(() => process.exit(0), 2000);
}

const finalModel = await done;
console.log('window closed, final model:', JSON.stringify(finalModel));
