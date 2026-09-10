// The Node windowed driver: winit + wgpu underneath, the Elm loop in JS on
// top, both sharing the main thread via a pumped event loop (`runWindowed`
// pumps the OS between turns). What this shows is the door Node has of its
// own — not the counter, which is `apps/counter.tsx`:
//
// - `init` is handed the window and runs after `setup`, so the first model
//   already knows the size the window actually opened at, and `resize`
//   messages keep it current — no module-level variable, no constant
//   corrected on the first frame;
// - resources are the window's: an image registered in `setup` and drawn
//   with `<image src>`, a sound registered there and played by a
//   `clickSound` prop, and a loop as an `<audio>` node the view declares
//   while it is on;
// - custom chrome: the app draws its own `<titlebar>`, and the OS's own
//   controls sit where `env().window.nativeControls` says.
//
//   npm run window
import type { CoreMsg, KuiWindow, UiEvent, WindowSize } from '@qxuken/kui';
import { run } from '../devtools.js';

type Model = { size: WindowSize; clicks: number; hum: boolean };
type AppMsg = { kind: 'hum' } | { kind: 'add'; by: number };
type Msg = AppMsg | CoreMsg;

// `init` is handed the window, and runs after setup(), so the first model
// already knows the size the window actually opened at.
const init = (win: KuiWindow): Model => ({ size: win.size(), clicks: 0, hum: false });

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>): Model | undefined {
  switch (msg.kind) {
    case 'add':
      return { ...model, clicks: model.clicks + msg.by };
    case 'hum':
      return { ...model, hum: !model.hum };
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

const view = (model: Model, win: KuiWindow) => {
  const t = win.theme();
  const env = win.env();
  const controls = env.window.nativeControls;
  return (
    <box gap={16} bg={t.bg} width="grow" height="grow">
      <titlebar title="kui — window" />
      <box pad={24} gap={16}>
        <text size={24} color={t.fg}><span bold color={t.accent}>kui</span> × Node × a real window</text>
        <image src={gradient} width={128} radius={8} label="gradient" />
        <box dir="row" gap={12} crossAlign="center">
          <button onClick={{ kind: 'add', by: 1 }}>+1</button>
          <box padX={14} padY={8} radius={6} bg={t.accent} hoverBg={t.accentHover}
               pressedBg={t.accentPressed} onClick={{ kind: 'hum' }} clickSound={click}>
            <text size={15} color={t.onAccent}>{model.hum ? 'hum: on' : 'hum: off'}</text>
          </box>
          {model.hum && <audio key="hum" src={hum} loop volume={0.3} />}
          <text size={20} color={t.fg}>{`clicks = ${model.clicks}`}</text>
        </box>
        <text size={14} color={t.muted}>
          {`window: ${Math.round(model.size.width)}x${Math.round(model.size.height)} @ ${model.size.scale}x`}
        </text>
        <text size={14} color={t.muted}>
          {controls
            ? `the OS draws its controls over ${Math.round(controls.w)}×${Math.round(controls.h)} at the window origin`
            : 'no OS controls over the content on this platform'}
        </text>
        <text size={13} color={t.faint}>resize the window: the resize message keeps the model current</text>
      </box>
    </box>
  );
};

await run<Model, AppMsg>({
  name: 'window',
  init,
  update,
  view,
  window: { width: 640, height: 420, minWidth: 420, minHeight: 320, chrome: 'custom' },
  dock: 'bottom',
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
  },
});
