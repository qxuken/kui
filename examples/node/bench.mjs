// Node side of the frontend-lowering shootout (same view as
// crates/kui-lua/examples/bench.rs): 300 rows of text + swatch, headless.
// Splits the cost of a frame into JS tree building vs encoding vs the
// boundary crossing and lowering.
import { Ctx, createEncoder, protocol } from '@qxuken/kui';

const ROWS = 300;
const WARMUP = 30;
const ITERS = 200;

const box = (props, children = []) => ({ type: 'box', props, children });
const text = (s) => ({ type: 'text', props: { size: 14 }, children: [s] });

function tree() {
  const rows = [];
  for (let i = 0; i < ROWS; i++) {
    rows.push(
      box({ dir: 'row', gap: 4, bg: '#202030' }, [
        text('row ' + i),
        box({ width: 40, height: 12, bg: '#3b5bd4' }),
      ]),
    );
  }
  return box({ pad: 8, gap: 2 }, rows);
}

function bench(name, fn) {
  for (let i = 0; i < WARMUP; i++) fn();
  const t0 = process.hrtime.bigint();
  for (let i = 0; i < ITERS; i++) fn();
  const ms = Number(process.hrtime.bigint() - t0) / 1e6 / ITERS;
  console.log(`${name.padEnd(22)} ${ms.toFixed(3).padStart(7)} ms/frame`);
  return ms;
}

const ctx = new Ctx();
const enc = createEncoder(protocol());

// frame(): encode the tree to the binary IR stream, lower it once.
const full = bench('node: build + frame', () => ctx.frame(800, 600, 1, tree()));
const prebuilt = tree();
const frameOnly = bench('node: frame only', () => ctx.frame(800, 600, 1, prebuilt));
bench('node: tree build only', () => tree());
bench('node: encode only', () => enc.encode(prebuilt));

console.log(`frame() boundary share: ${((frameOnly / full) * 100).toFixed(0)}%`);
console.log('quads:', ctx.stats().quadCount);
