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

// `init` is handed the window, and runs after setup(), so the first model
// already knows the size the window actually opened at — no module-level
// variable, no constant corrected on the first `resize`. `resize` messages
// keep it current after that.
const init = (win: KuiWindow): Model => ({ count: 0, note: '', size: win.size(), hum: false });

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
/// The window handle, kept from `setup` so the view can read the palette
/// the core derived from the OS. `runWindowed` calls `setup` before the
/// first frame, so it is set by the time `view` runs.
let window_: KuiWindow | null = null;
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

// Every colour here is a theme role (ADR 0019): the window follows the
// OS's light/dark and its accent, and `win.theme()` is the palette the
// core derived — the same `0xRRGGBBAA` numbers a colour prop takes.
const view = (model: Model) => {
  const t = window_!.theme();
  return (
  <box gap={16} bg={t.bg} width="grow" height="grow" title={`kui counter — ${model.count}`}>
    <titlebar title="kui counter" />
    <box pad={24} gap={16}>
    <text size={24} color={t.fg}><span bold color={t.accent}>kui</span> × Node × JSX</text>
    <image src={gradient} width={128} radius={8} label="gradient" />
    <box dir="row" gap={12} crossAlign="center">
      <button onClick={{ kind: 'add', by: 1 }}>+1</button>
      <button onClick={{ kind: 'add', by: -1 }}>-1</button>
      <button onClick={{ kind: 'reset' }} description="Back to zero">reset</button>
      <text size={20} color={t.fg}>{`count = ${model.count}`}</text>
    </box>
    <box dir="row" gap={12} crossAlign="center">
      <box padX={14} padY={8} radius={6} bg={t.accent} hoverBg={t.accentHover}
           pressedBg={t.accentPressed} onClick={{ kind: 'hum' }} clickSound={click}>
        <text size={15} color={t.onAccent}>{model.hum ? 'hum: on' : 'hum: off'}</text>
      </box>
      {model.hum && <audio key="hum" src={hum} loop volume={0.3} />}
    </box>
    <edit key="note" label="note" initial="" size={16} width={280} padX={10} padY={6}
          bg={t.sunken} color={t.fg} radius={4} autofocus />
    <text size={14} color={t.muted}>{`note: ${model.note || '(empty)'}`}</text>
    <text size={14} color={t.muted}>
      {`window: ${Math.round(model.size.width)}x${Math.round(model.size.height)} @ ${model.size.scale}x`}
    </text>
    </box>
    <latencyHud />
  </box>
  );
};

const done = runWindowed({ init, update, view }, {
  title: 'kui counter',
  width: 640,
  height: 480,
  minWidth: 420,
  minHeight: 320,
  chrome: 'custom',
  setup(win) {
    window_ = win;
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
  },
});

if (process.argv.includes('--smoke')) {
  setTimeout(() => process.exit(0), 2000);
}

const finalModel = await done;
console.log('window closed, final model:', JSON.stringify(finalModel));
