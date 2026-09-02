// The counter in a real window, from Node: winit + wgpu underneath, the
// Elm loop in JS on top, both sharing the main thread via a pumped event
// loop. Run with --smoke to auto-close after 2 seconds (CI/sanity).
import { runWindowed } from '@qxuken/kui';
import type { CoreMsg, KuiWindow, UiEvent, WindowSize } from '@qxuken/kui';

type Model = { count: number; note: string; size: WindowSize; hum: boolean };

// This app's own payloads plus the core's (`changed`, `key`, `hover`, ...):
// one discriminated union, switched over directly.
type CounterMsg = { kind: 'add'; by: number } | { kind: 'reset' } | { kind: 'hum' };
type Msg = CounterMsg | CoreMsg;

// setup() runs before init(), so the first model already knows the size the
// window actually opened at; `resize` messages keep it current after that.
let openedAt: WindowSize = { width: 0, height: 0, scale: 1 };

const init = (): Model => ({ count: 0, note: '', size: openedAt, hum: false });

function update(model: Model, msg: Msg, ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  switch (msg.kind) {
    case 'add':
      return { ...model, count: model.count + msg.by };
    case 'reset':
      return { ...model, count: 0 };
    case 'hum':
      return { ...model, hum: !model.hum };
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
// Sounds are resources too: synthesized here (no asset files), played by
// the `clickSound` prop and an `<audio>` node the view declares while on.
let click = '';
let hum = '';

/** Mono float samples to a 16-bit PCM WAV buffer. */
function wav(sampleRate: number, samples: Float32Array): Buffer {
  const b = Buffer.alloc(44 + samples.length * 2);
  b.write('RIFF', 0);
  b.writeUInt32LE(36 + samples.length * 2, 4);
  b.write('WAVEfmt ', 8);
  b.writeUInt32LE(16, 16);
  b.writeUInt16LE(1, 20);
  b.writeUInt16LE(1, 22);
  b.writeUInt32LE(sampleRate, 24);
  b.writeUInt32LE(sampleRate * 2, 28);
  b.writeUInt16LE(2, 32);
  b.writeUInt16LE(16, 34);
  b.write('data', 36);
  b.writeUInt32LE(samples.length * 2, 40);
  samples.forEach((s, i) => b.writeInt16LE(Math.round(Math.max(-1, Math.min(1, s)) * 32767), 44 + i * 2));
  return b;
}

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
    <box dir="row" gap={12} crossAlign="center">
      <box padX={14} padY={8} radius={6} bg="#3b5bd4" hoverBg="#476ce0" pressedBg="#2f54c4"
           onClick={{ kind: 'hum' }} clickSound={click}>
        <text size={15} color="#ffffff">{model.hum ? 'hum: on' : 'hum: off'}</text>
      </box>
      {model.hum && <audio key="hum" src={hum} loop volume={0.3} />}
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
  minWidth: 420,
  minHeight: 320,
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
    const rate = 44_100;
    const blip = Float32Array.from({ length: 2646 }, (_, i) => {
      const env = 1 - i / 2646;
      return Math.sin((i / rate) * 880 * 2 * Math.PI) * env * env * 0.4;
    });
    // A 200-sample period loops seamlessly.
    const tone = Float32Array.from({ length: 2000 }, (_, i) => Math.sin((i / 200) * 2 * Math.PI) * 0.25);
    click = win.addSound(wav(rate, blip));
    hum = win.addSound(wav(rate, tone));
    openedAt = win.size();
  },
});

if (process.argv.includes('--smoke')) {
  setTimeout(() => process.exit(0), 2000);
}

const finalModel = await done;
console.log('window closed, final model:', JSON.stringify(finalModel));
