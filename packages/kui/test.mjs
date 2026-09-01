// Transport parity: the same tree through frame() (binary stream), frameJson
// and frameObject must yield byte-identical quads — for every schema prop,
// the hand-written composites, and every element. The Rust side pins each
// decoder to the schema; this pins the JS encoder to the addon's JSON path.
// Needs the addon built: npm run build:native. Run: npm test
import test from 'node:test';
import assert from 'node:assert/strict';
import { Ctx, protocol } from './index.js';

const box = (props, children = [], key) => ({ type: 'box', key, props, children });
const text = (children, props = {}) => ({ type: 'text', props, children: [].concat(children) });
const el = (type, props = {}, children = [], key) => ({ type, key, props, children });

// `build(ctx)` returns the tree so per-context resources (images) can be
// registered first.
function run(transport, build) {
  const ctx = new Ctx();
  const tree = build(ctx);
  if (transport === 'binary') ctx.frame(320, 240, 1, tree);
  else if (transport === 'json') ctx.frameJson(320, 240, 1, JSON.stringify(tree));
  else ctx.frameObject(320, 240, 1, tree);
  return { quads: Buffer.from(ctx.quads()), stats: ctx.stats(), ctx };
}

function assertParity(label, build) {
  const b = run('binary', build);
  const j = run('json', build);
  const o = run('object', build);
  assert.ok(b.stats.quadCount > 0, `${label}: no quads`);
  assert.equal(b.stats.quadCount, j.stats.quadCount, `${label}: binary/json quad counts`);
  assert.ok(b.quads.equals(j.quads), `${label}: binary and json quads differ`);
  assert.ok(b.quads.equals(o.quads), `${label}: binary and object quads differ`);
  return b;
}

const SAMPLE = {
  f32: 12,
  color: '#3b5bd4',
  flag: true,
  sizing: '50%',
  msg: { kind: 'm', n: 1, list: [1, 'two', null] },
};

test('protocol exports a version and the schema rows', () => {
  const p = protocol();
  assert.equal(typeof p.version, 'number');
  assert.ok(p.op.root >= 0 && p.op.end >= 0);
  assert.ok(Object.keys(p.prop).length > 20);
});

test('every generic schema prop lowers identically on all transports', () => {
  const { prop } = protocol();
  for (const [name, def] of Object.entries(prop)) {
    if (def.kind === 'custom') continue;
    const value = def.kind === 'enum' ? def.values[def.values.length - 1] : SAMPLE[def.kind];
    assert.notEqual(value, undefined, `${name}: no sample for kind ${def.kind}`);
    const build =
      def.target === 'style'
        ? () => box({ pad: 4, bg: '#101010' }, [text('sample', { size: 14, [name]: value })])
        : () =>
            box({ pad: 4, bg: '#101010' }, [
              box({ width: 60, height: 20, bg: '#333333', [name]: value }, [text('x', { size: 12 })]),
            ]);
    assertParity(name, build);
  }
});

test('composites and constructor specials lower identically', () => {
  const build = () =>
    box({ title: 'frame', pad: 6, gap: 3, bg: '#14161e', keyFocus: true, onKey: 'k' }, [
      box(
        {
          dir: 'row',
          padX: 8,
          padY: 2,
          padL: 1,
          padT: 3,
          borderW: 2,
          borderColor: '#2a2d3a',
          radius: 5,
          clip: true,
          scrollX: true,
          scrollY: true,
          width: 'grow',
          height: 40,
          tooltip: 'hint',
        },
        [text('a', { size: 12 }), text('b', { size: 12 })],
        'keyed-row',
      ),
      box({ float: 'below', bg: '#ff0000', width: 10, height: 10 }),
      box(
        {
          float: { anchor: 'viewport', at: ['end', 'end'], self: ['end', 'end'], dx: -4, dy: -4, fit: true },
          bg: '#00ff00',
          width: 10,
          height: 10,
        },
      ),
    ]);
  assertParity('composites', build);
});

test('every element lowers identically', () => {
  const build = (ctx) => {
    const px = Buffer.alloc(4 * 4 * 4, 0xff);
    const img = ctx.addImage(4, 4, px);
    return box({ pad: 8, gap: 4, bg: '#202030' }, [
      el('titlebar', { title: 'bar' }),
      el('titlebar', {}, [text('custom', { size: 12 }), el('windowButtons')]),
      text(['plain ', el('span', { bold: true }, ['bold ']), el('span', { italic: true, color: '#ff8888' }, ['red'])], {
        size: 14,
      }),
      el('button', { onClick: { kind: 'go' } }, ['go'], 'go-btn'),
      el('edit', { initial: 'hello', width: 'grow', size: 13, multiline: true }, [], 'note'),
      el('image', { src: img, width: 16, radius: 2 }),
      el('latencyGraph'),
      el('latencyHud', { at: ['start', 'end'] }),
      'bare text child',
      42,
      [text('nested', { size: 10 }), null, false],
    ]);
  };
  const b = assertParity('elements', build);
  // Image quads (kind 3) prove the registered image reached the atlas.
  const stride = b.quads.byteLength / b.stats.quadCount;
  let images = 0;
  for (let off = 0; off < b.quads.byteLength; off += stride) {
    if (b.quads.readUInt32LE(off + 14 * 4) === 3) images++;
  }
  assert.equal(images, 1, 'one image quad');
});

test('errors are the same on every transport', () => {
  const bad = [
    ['unknown element', () => el('nope')],
    ['edit without key', () => el('edit', { initial: '' })],
    ['span outside text', () => el('span', {}, ['x'])],
    ['bad dir', () => box({ dir: 'diagonal' })],
    ['bad align', () => box({ mainAlign: 'middle' })],
  ];
  for (const [label, build] of bad) {
    for (const transport of ['binary', 'json', 'object']) {
      assert.throws(() => run(transport, build), `${label} should throw on ${transport}`);
    }
  }
});
