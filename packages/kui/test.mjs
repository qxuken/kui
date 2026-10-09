// The encoder is the addon's only door: `frame()` turns a tree into the flat
// binary IR and `binary.rs` lowers it, with no second dispatcher to check it
// against. So the tests below pin it from both ends — every schema prop, the
// hand-written composites and every element have to reach the stream (a
// dropped prop encodes to the same bytes as not declaring it) and then lower
// without desyncing the decoder, and the corpus scenes say what the result
// has to *be*, against a report kui-core generates.
// Needs the addon built: npm run build:native. Run: npm test
import test from 'node:test';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import { constants as osConstants, tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import * as pkg from './index.js';
import { Ctx, KuiWindow, RowHeights, animationAt, clipStride, createApp, createEncoder, decodeAnimation, decodeImage, decodeQuads, defineTokens, list, protocol, quadStride, revealRow, roles, rowsInView, runWindowed, splitter, uniformList, windowOptions, withEffects } from './index.js';

const box = (props, children = [], key) => ({ type: 'box', key, props, children });
const text = (children, props = {}) => ({ type: 'text', props, children: [].concat(children) });
const el = (type, props = {}, children = [], key) => ({ type, key, props, children });

/** Index of the `kind` word inside one quad of `ctx.quads()` — KuiQuad's
 *  x,y,w,h + color + border_color + radius + border_w + blur come first. The
 *  tests below read it raw rather than through `decodeQuads`. */
const KIND_WORD = 18;

// `build(ctx)` returns the tree so per-context resources (images) can be
// registered first.
function run(build) {
  const ctx = new Ctx();
  const tree = build(ctx);
  ctx.frame(320, 240, 1, tree);
  return { quads: Buffer.from(ctx.quads()), stats: ctx.stats(), ctx };
}

// A private encoder, so the bytes can be looked at without disturbing the
// one `frame()` uses (its buffers are only valid until the next encode).
const probe = createEncoder(protocol());
function encoded(tree) {
  const { stream, strings } = probe.encode(tree);
  return Buffer.concat([
    Buffer.from(new Uint8Array(stream.buffer, stream.byteOffset, stream.byteLength)),
    Buffer.from(strings),
  ]);
}

/** The tree encodes to something, and the addon lowers it into quads
 *  without the decoder desyncing. */
function assertLowers(label, build) {
  const r = run(build);
  assert.ok(r.stats.quadCount > 0, `${label}: no quads`);
  return r;
}

const SAMPLE = {
  f32: 12,
  color: '#3b5bd4',
  flag: true,
  sizing: '50%',
  min: 'fit',
  max: 12,
  msg: { kind: 'm', n: 1, list: [1, 'two', null] },
  tag: { kind: 't' },
  str: 'group-a',
  family: 'mono',
  resource: '0000000000000007',
  keyframes: [
    { width: { grow: 0 }, bg: '#112233' },
    { at: 0.75, width: 'grow', height: '50%', radius: 9, opacity: 0.25, dx: 3, dy: -2 },
  ],
  enter: { dx: -40, dy: 8, width: { grow: 0 }, bg: '#11223300', radius: 0, opacity: 0 },
  gradient: { to: 'bottom right', stops: ['#112233', ['#ffffff', 0.5], '#000000'] },
};

/** Per-prop overrides, where the shared sample for a kind would not survive
 *  the prop's own clamping — `opacity`'s default is the top of its range, so
 *  a sample above 1 clamps straight back to it. */
// The align rows' last value is `baseline`, which means nothing on the
// main axis or a column's cross axis, so they take a value that does.
// `buttons` is a list of names, and the shared string sample names none.
const SAMPLE_BY_NAME = { opacity: 0.5, mainAlign: 'spaceEvenly', crossAlign: 'end', buttons: 'middle', scrollMods: 'ctrl' };

test('protocol exports a version and the schema rows', () => {
  const p = protocol();
  assert.equal(typeof p.version, 'number');
  assert.ok(p.op.root >= 0 && p.op.end >= 0);
  assert.ok(Object.keys(p.prop).length > 20);
});

// What the node snapshot (`nodes()`, ADR 0024) reads back for a prop
// declared with its sample, where the snapshot carries that prop at all:
// the check that the value landed *under its own id* (backlog AR47) — a
// prop encoded under a neighbour's id still changes the bytes and still
// lowers, and only a readback tells the two apart. The snapshot carries
// the layout and paint spec, the flags and the handlers; a row it does
// not carry (a sound, a cursor, an easing) has no reader here.
const READBACK = {
  width: (n) => n.width === '50%',
  height: (n) => n.height === '50%',
  // A `fit` floor reads as the number the fit pass resolved it to — the
  // text's own width and height here — since the pass writes it back.
  minWidth: (n) => n.minWidth > 0,
  minHeight: (n) => n.minHeight > 0,
  maxWidth: (n) => n.maxWidth === 12,
  maxHeight: (n) => n.maxHeight === 12,
  gap: (n) => n.gap === 12,
  wrapChildren: (n) => n.wrap === true,
  mainAlign: (n) => n.mainAlign === 'spaceEvenly',
  crossAlign: (n) => n.crossAlign === 'end',
  bg: (n) => n.bg === 0x3b5bd4ff,
  radius: (n) => n.radius.every((r) => r === 12),
  radiusTL: (n) => n.radius[0] === 12 && n.radius[1] === 0,
  radiusTR: (n) => n.radius[1] === 12 && n.radius[0] === 0,
  radiusBR: (n) => n.radius[2] === 12 && n.radius[1] === 0,
  radiusBL: (n) => n.radius[3] === 12 && n.radius[2] === 0,
  opacity: (n) => n.opacity === 0.5,
  center: (n) => n.mainAlign === 'center' && n.crossAlign === 'center',
  hoverable: (n) => n.flags.includes('hoverable'),
  selectable: (n) => n.flags.includes('selectable'),
  focusable: (n) => n.flags.includes('focusable'),
  disabled: (n) => n.flags.includes('disabled'),
  transition: (n) => n.flags.includes('transition'),
  onClick: (n) => n.events.click?.kind === 'm',
  onDrag: (n) => n.events.drag?.kind === 't',
  onKey: (n) => n.events.key?.kind === 't',
  onHover: (n) => n.events.hover?.kind === 't',
  onContextMenu: (n) => n.events['context-menu']?.kind === 't',
  onForceClick: (n) => n.events['force-click']?.kind === 't',
  onButton: (n) => n.events.button?.kind === 't',
  onLayout: (n) => n.events.layout?.kind === 't',
  modal: (n) => n.events.modal?.kind === 't',
  role: (n) => n.role === 'terminal',
};

// Every prop writes at least its own id, so declaring one has to change the
// encoded bytes. That is the check the JSON transport used to provide by
// disagreeing: it catches the `switch` arm that never got written, or the
// name the encoder quietly falls through on. And where the snapshot can
// say so, the value is read back off the node it was declared on.
test('every generic schema prop reaches the stream, lowers, and reads back where the snapshot carries it', () => {
  const { prop } = protocol();
  for (const name of Object.keys(READBACK)) assert.ok(name in prop, `READBACK names ${name}, which is not a prop`);
  let readBack = 0;
  for (const [name, def] of Object.entries(prop)) {
    if (def.kind === 'custom') continue;
    const value =
      SAMPLE_BY_NAME[name] ?? (def.kind === 'enum' ? def.values[def.values.length - 1] : SAMPLE[def.kind]);
    assert.notEqual(value, undefined, `${name}: no sample for kind ${def.kind}`);
    const build = (props) =>
      def.target === 'style'
        ? box({ pad: 4, bg: '#101010' }, [text('sample', { size: 14, ...props })])
        : box({ pad: 4, bg: '#101010' }, [
            box({ width: 60, height: 20, bg: '#333333', ...props }, [text('x', { size: 12 })], 'probe'),
          ]);
    assert.ok(
      !encoded(build({ [name]: value })).equals(encoded(build({}))),
      `${name}: the encoder dropped it — the stream is the same as without it`,
    );
    const { ctx } = assertLowers(name, () => build({ [name]: value }));
    if (name in READBACK) {
      ctx.setInspect(true);
      ctx.frame(320, 240, 1, build({ [name]: value }));
      const probe = ctx.nodes().find((n) => n.label === 'probe');
      assert.ok(probe, `${name}: the probe node is in the snapshot`);
      assert.ok(READBACK[name](probe), `${name}: declared as ${JSON.stringify(value)}, the snapshot reads ${JSON.stringify(probe)}`);
      readBack++;
    }
  }
  assert.equal(readBack, Object.keys(READBACK).length);
});

// Size expressions (backlog F109): spelled, a string rides as a strref
// the addon parses once; as data, `{ clamp: [...] }` rides as numbers in
// prefix code and nothing is parsed. Both lay out alike, against the
// parent's content box, as a width and as a clamp.
test('a size expression spelled and as data lays out the same', () => {
  const widths = (width, maxWidth) => {
    const ctx = new Ctx();
    ctx.setInspect(true);
    ctx.frame(1000, 200, 1, box({ width: 600, pad: 0 }, [
      box({ width, height: 4 }, [], 'w'),
      box({ width: 900, maxWidth, height: 4 }, [], 'm'),
    ]));
    const n = (k) => ctx.nodes().find((x) => x.label === k).rect.w;
    return [n('w'), n('m')];
  };
  const spelled = widths('clamp(100px, 50%, 250px)', 'min(40%, 500px)');
  const data = widths({ clamp: [100, { percent: 50 }, 250] }, { min: ['40%', { px: 500 }] });
  assert.deepEqual(spelled, [250, 240]);
  assert.deepEqual(data, spelled);
  assert.throws(() => encoded(box({ width: { clamp: [1, 2] } }, [])), /clamp takes three/);
});

// The size spellings every binding is run through (backlog RG94): one
// table, `crates/kui-core/tests/fixtures/size_spellings.json`, read by the
// core and Lua too. A string rides to the addon as its spelling and is
// parsed by the core; nested in data it is read by the encoder's own
// `parseSize`, which must agree with it — `{ min: ['max(1px, 2%)', 30] }`
// was refused here and taken there. A row marked `"only": "lua"` is Lua's
// `pct`, which JS spells `percent`.
test('every size spelling lays out as the shared table says', () => {
  const here = dirname(fileURLToPath(import.meta.url));
  const table = JSON.parse(readFileSync(join(here, '../../crates/kui-core/tests/fixtures/size_spellings.json'), 'utf8'));
  const laid = (width) => {
    const ctx = new Ctx();
    ctx.setInspect(true);
    ctx.frame(1200, 100, 1, box({ width: 1000, pad: 0 }, [box({ width, height: 4 }, [], 'r')]));
    return ctx.nodes().find((x) => x.label === 'r').rect.w;
  };
  const failures = [];
  for (const row of table.rows) {
    const want = row.only === 'lua' ? null : row.px;
    const check = (how, width) => {
      let got;
      try {
        got = laid(width);
      } catch (e) {
        got = null;
      }
      const ok = want === null ? got === null : got !== null && Math.abs(got - want) < 1e-3;
      if (!ok) failures.push(`${JSON.stringify(row.width)} ${how}: ${got}, want ${want}`);
    };
    check('as a width', row.width);
    if (typeof row.width === 'string' || typeof row.width === 'number') check('nested in a max', { max: [row.width] });
  }
  assert.deepEqual(failures, []);
});

// The size-expression table is the process's and keeps at most 65 536
// (backlog RG93), so a view that makes one per frame — a splitter's
// fractional drag in a `max` — reaches it. Past it a new expression is
// the prop left at its default and a warning, where the addon failed the
// frame whole. A process of its own, since the full table is for life.
test('a size expression past the full table leaves its prop undeclared, not the frame failed', () => {
  const r = spawnSync(
    process.execPath,
    [
      '--input-type=module',
      '-e',
      `import { Ctx } from './index.js';
       const box = (props, children = [], key) => ({ type: 'box', key, props, children });
       const ctx = new Ctx();
       ctx.setInspect(true);
       const drag = [];
       for (let i = 0; i < 70000; i++) drag.push(box({ width: { max: [i + 0.5, { percent: 50 }] }, height: 0 }));
       ctx.frame(1000, 200, 1, box({ width: 400, gap: 0 }, drag));
       ctx.frame(1000, 200, 1, box({ width: 400, gap: 0 }, [
         box({ width: { max: [0.5, { percent: 50 }] }, height: 4 }, [], 'kept'),
         box({ width: 'clamp(1px, 50%, 300px)', height: 4 }, [], 'spelled'),
         box({ width: { min: [7, { percent: 50 }] }, height: 4 }, [], 'data'),
         box({ width: 350, maxWidth: 'min(3px, 50%)', height: 4 }, [], 'capped'),
         box({ width: 10, minWidth: { max: [11, { percent: 50 }] }, height: 4 }, [], 'floored'),
       ]));
       const w = (k) => ctx.nodes().find((x) => x.label === k).rect.w;
       const full = ctx.warnings().filter((x) => x.code === 'size-expressions-full');
       console.log(JSON.stringify({ w: ['kept', 'spelled', 'data', 'capped', 'floored'].map(w), full: full.length }));`,
    ],
    { cwd: dirname(fileURLToPath(import.meta.url)), encoding: 'utf8' },
  );
  assert.equal(r.status, 0, `the frame failed: ${r.stderr}`);
  const out = JSON.parse(r.stdout.trim().split('\n').at(-1));
  assert.deepEqual(out, { w: [200, 0, 0, 350, 10], full: 1 });
});

// `alwaysOnTop` is a root declaration with no node, like `title`, and a
// per-frame one with a default: the frame that stops saying it is the
// lowering (backlog C30). The ask is what `alwaysOnTop()` answers; what
// the platform did is `env().window.alwaysOnTop`, which a bare `Ctx`
// never writes.
test('alwaysOnTop is asked per frame from the root and read back as the ask', () => {
  const ctx = new Ctx();
  assert.equal(ctx.alwaysOnTop(), false);
  ctx.frame(320, 240, 1, box({ title: 'pinned', alwaysOnTop: true }, [text('a', { size: 12 })]));
  assert.equal(ctx.alwaysOnTop(), true);
  assert.equal(ctx.windowTitle(), 'pinned');
  assert.equal(ctx.env().window.alwaysOnTop, false, 'asking is not having');
  ctx.frame(320, 240, 1, box({ title: 'pinned' }, [text('a', { size: 12 })]));
  assert.equal(ctx.alwaysOnTop(), false);
  // `false` and a non-root box both encode nothing, like `title` off the root.
  ctx.frame(320, 240, 1, box({ alwaysOnTop: false }, [box({ alwaysOnTop: true })]));
  assert.equal(ctx.alwaysOnTop(), false);
  assert.deepEqual(ctx.warnings(), []);
});

// `secureInput` is the same shape (backlog F85): a root declaration with
// no node and a default, so the frame that stops saying it turns it off.
test('secureInput is asked per frame from the root and read back as the ask', () => {
  const ctx = new Ctx();
  assert.equal(ctx.secureInput(), false);
  ctx.frame(320, 240, 1, box({ secureInput: true }, [text('password:', { size: 12 })]));
  assert.equal(ctx.secureInput(), true);
  ctx.frame(320, 240, 1, box({}, [text('$', { size: 12 })]));
  assert.equal(ctx.secureInput(), false, 'the frame that stops asking turns it off');
  // `false` and a non-root box both encode nothing, like `alwaysOnTop`.
  ctx.frame(320, 240, 1, box({ secureInput: false }, [box({ secureInput: true })]));
  assert.equal(ctx.secureInput(), false);
  assert.deepEqual(ctx.warnings(), []);
});

// `imeOff` is the same shape again (backlog F125): the frame that stops
// saying it gives the window its input method back.
test('imeOff is asked per frame from the root and read back as the ask', () => {
  const ctx = new Ctx();
  assert.equal(ctx.imeOff(), false);
  ctx.frame(320, 240, 1, box({ imeOff: true }, [text('-- NORMAL --', { size: 12 })]));
  assert.equal(ctx.imeOff(), true);
  ctx.frame(320, 240, 1, box({}, [text('-- INSERT --', { size: 12 })]));
  assert.equal(ctx.imeOff(), false, 'the frame that stops asking gives the IME back');
  // `false` and a non-root box both encode nothing, like `secureInput`.
  ctx.frame(320, 240, 1, box({ imeOff: false }, [box({ imeOff: true })]));
  assert.equal(ctx.imeOff(), false);
  assert.deepEqual(ctx.warnings(), []);
});

// `optionAsAlt` is the same shape with a side for a value (backlog F113):
// "none" is the default and the frame that stops saying it goes back to it;
// a name kui does not have is refused before it crosses.
test('optionAsAlt is asked per frame from the root and read back as the side', () => {
  const ctx = new Ctx();
  assert.equal(ctx.optionAsAlt(), 'none');
  for (const side of ['left', 'right', 'both']) {
    ctx.frame(320, 240, 1, box({ optionAsAlt: side }, [text('keys', { size: 12 })]));
    assert.equal(ctx.optionAsAlt(), side);
  }
  ctx.frame(320, 240, 1, box({}, [text('keys', { size: 12 })]));
  assert.equal(ctx.optionAsAlt(), 'none', 'the frame that stops asking gives the Option keys back');
  // "none" and a non-root box both encode nothing.
  ctx.frame(320, 240, 1, box({ optionAsAlt: 'none' }, [box({ optionAsAlt: 'left' })]));
  assert.equal(ctx.optionAsAlt(), 'none');
  assert.deepEqual(ctx.warnings(), []);
  assert.throws(() => ctx.frame(320, 240, 1, box({ optionAsAlt: 'meta' })), /optionAsAlt "meta" \(none \| left \| right \| both\)/);
  assert.deepEqual(protocol().optionAsAlt, ['none', 'left', 'right', 'both']);
});

test('the hand-written composites and constructor specials lower', () => {
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
  assertLowers('composites', build);
});

test('every element lowers', () => {
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
      el('line', { from: [0, 0], to: [40, 20], width: 2, color: '#7f9cf5' }),
      el('line', { points: [[0, 30], [20, 10], [40, 30]], curve: true }, [], 'curve'),
      el('latencyGraph'),
      el('latencyHud', { at: ['start', 'end'] }),
      el('audio', { src: ctx.addSound(Buffer.from('RIFF....WAVE')), loop: true, volume: 0.5, tag: { k: 1 } }, [], 'music'),
      'bare text child',
      42,
      [text('nested', { size: 10 }), null, false],
    ]);
  };
  const b = assertLowers('elements', build);
  // Image quads (kind 3) prove the registered image reached the atlas.
  const stride = b.quads.byteLength / b.stats.quadCount;
  let images = 0;
  for (let off = 0; off < b.quads.byteLength; off += stride) {
    if (b.quads.readUInt32LE(off + KIND_WORD * 4) === 3) images++;
  }
  assert.equal(images, 1, 'one image quad');
  // Segment quads (kind 6): the straight line is one, the curve is what the
  // core's flattening makes of two 28.3px chords (5 pieces each).
  let segments = 0;
  for (let off = 0; off < b.quads.byteLength; off += stride) {
    if (b.quads.readUInt32LE(off + KIND_WORD * 4) === 6) segments++;
  }
  assert.equal(segments, 11, 'one segment plus a flattened curve');

  // An element writing its own operands by hand (rather than through a
  // schema row) needs its argument *order* pinned, not just its presence:
  // reading `at` as [y, x] still encodes and still lowers, it just puts the
  // HUD in the wrong corner. So this asks where it actually landed — a
  // swap-for-a-swap comparison would not notice, being symmetric.
  const corner = (at) => {
    const { ctx } = run(() => box({ width: 'grow', height: 'grow' }, [el('latencyHud', { at })]));
    const qs = decodeQuads(ctx.quads());
    return [Math.min(...qs.map((q) => q.x)), Math.min(...qs.map((q) => q.y))];
  };
  const [leftX, bottomY] = corner(['start', 'end']);
  const [rightX, topY] = corner(['end', 'start']);
  assert.ok(leftX < rightX, 'latencyHud: at[0] places it horizontally');
  assert.ok(topY < bottomY, 'latencyHud: at[1] places it vertically');

  // Every prop above is one some table claims. This scene is the
  // allow-list's fixture: an element prop nobody put in `ELEMENTS.jsx_own`
  // fails here rather than warning at whoever writes it next.
  assert.deepEqual(b.ctx.warnings().filter((w) => w.code === 'unknown-prop'), []);
});

// A name outside the schema has no wire id, so the encoder is the only side
// that ever sees it. It reports what it dropped instead of dropping it in
// silence.
test('a stroke decodes with its endpoints and a shadow with its blur (F17)', () => {
  // The mind map's preview filtered the display list to `kind === 0` and
  // silently lost every connector: the `Quad` type stopped at kind 4 and
  // declared no `ends`, though the decoder had carried both since ADR 0010.
  const { quads } = run(() =>
    box({ width: 200, height: 100 }, [
      box({ width: 40, height: 20, bg: '#ffffff', shadowBlur: 6, shadowColor: '#000000' }, [], 'lit'),
      el('line', { from: [10, 10], to: [90, 60], width: 3, color: '#7f9cf5' }, [], 'seg'),
    ]),
  );
  const all = decodeQuads(quads);
  const seg = all.find((q) => q.kind === 6);
  assert.ok(seg, 'the line is a segment quad');
  assert.deepEqual(seg.ends, [10, 10, 90, 60]);
  assert.equal(seg.borderW, 3);
  const shadow = all.find((q) => q.kind === 5);
  assert.ok(shadow, 'the shadow is its own quad');
  assert.equal(shadow.blur, 6);
  assert.equal(shadow.ends, null);
  assert.ok(all.filter((q) => q.kind === 0).every((q) => q.ends === null && q.blur === 0));
});

test('an unknown prop warns once, with the name it was probably meant to be', () => {
  const view = () =>
    box({ pad: 4 }, [
      box({ hover_bg: '#333333', width: 10, height: 10 }),
      box({ hover_bg: '#333333', width: 10, height: 10 }),
      el('edit', { initial: 'hi', autofocus: true, colour: '#fff' }, [], 'note'),
    ]);
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, view());
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-prop');
  assert.equal(ws.length, 2, `one per name, not per node: ${JSON.stringify(ws)}`);
  const hover = ws.find((w) => w.message.includes('hover_bg'));
  assert.match(hover.message, /is not a prop of box/);
  assert.match(hover.message, /did you mean `hoverBg`\?/);
  // Nothing near `colour`, so no guess is offered — and the element it was
  // written on is named.
  const colour = ws.find((w) => w.message.includes('colour'));
  assert.match(colour.message, /is not a prop of edit/);
  assert.doesNotMatch(colour.message, /did you mean/);
  // Once per name: the second frame is silent, like every other check.
  ctx.frame(320, 240, 1, view());
  assert.deepEqual(ctx.warnings(), []);
  // And behind the same gate.
  const quiet = new Ctx();
  quiet.setDiagnostics(false);
  quiet.frame(320, 240, 1, view());
  assert.deepEqual(quiet.warnings(), []);
});

// The stock button reads the access rows and nothing else
// (`schema::BUTTON_ROWS_JSX`): a row it would drop is warned about with
// the rows it does read, and never changes its look.
test('the stock button admits the access rows and warns about the rest', () => {
  const view = (extra = {}) =>
    box({ pad: 4, gap: 4 }, [
      el('button', { onClick: 'go', description: 'Starts the run' }, ['go']),
      el('button', { onClick: 'stop', label: 'Stop the run', disabled: true, tooltip: 'Nothing is running' }, ['stop']),
      el('button', { onClick: 'x', ...extra }, ['x']),
    ]);
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, view({ radius: 12, hoverbg: '#333333' }));
  const tree = ctx.accessTree();
  const byName = (n) => tree.nodes.find((x) => x.name === n);
  const go = byName('go');
  assert.equal(go.role, 'button');
  assert.equal(go.description, 'Starts the run');
  assert.equal(go.disabled, false);
  const stop = byName('Stop the run');
  assert.equal(stop.role, 'button', 'named past its text');
  assert.equal(stop.description, 'Nothing is running', 'the tooltip is the description');
  assert.equal(stop.disabled, true);
  assert.equal(tree.nodes.length, 4, 'window and three buttons');

  // A real row the button does not read says so, and names the rows it
  // does; a misspelling is a misspelling, but the fix offered is never a
  // row the button would drop.
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-prop');
  assert.equal(ws.length, 2, JSON.stringify(ws));
  const radius = ws.find((w) => w.message.includes('`radius`'));
  assert.match(radius.message, /is a prop, but not one button reads/);
  assert.match(radius.message, /`description`/);
  assert.match(radius.message, /a box with `role` set takes every row/);
  const hoverbg = ws.find((w) => w.message.includes('hoverbg'));
  assert.match(hoverbg.message, /is not a prop of button/);
  assert.doesNotMatch(hoverbg.message, /did you mean/);

  // The dropped rows drew nothing: the same quads as a button without them.
  const plain = new Ctx();
  plain.frame(320, 240, 1, view());
  assert.deepEqual(ctx.quads(), plain.quads(), 'the look is the widget\'s');

  // A disabled button keeps its hit region, so its tooltip can say why:
  // hovered, the hint floats under it.
  const before = plain.quads().length;
  plain.cursor(stop.rect.x + 2, stop.rect.y + 2);
  plain.frame(320, 240, 1, view());
  assert.ok(plain.quads().length > before, 'the tooltip floated under the disabled button');
});

test('a text reads its style rows and warns about every other (AR13)', () => {
  // `text` admitted every shared row and every door dropped all but the
  // style: `<text live="polite">` — what the `live` doc tells you to write
  // — `<text role="heading">`, `<text label>` and `<text onClick>` reached
  // no tree and raised nothing. They are `unknown-prop` now, naming the
  // rows a text does read.
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  ctx.frame(320, 240, 1, box({ pad: 4 }, [
    el('text', { live: 'polite', role: 'heading', label: 'x', onClick: 'go', size: 14, color: '#ff0000', maxLines: 2 }, ['hi']),
  ]));
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-prop');
  const named = ws.map((w) => w.message.match(/`(\w+)`/)[1]).sort();
  assert.deepEqual(named, ['label', 'live', 'onClick', 'role'], JSON.stringify(ws));
  assert.match(ws[0].message, /not one text reads/);
  assert.match(ws[0].message, /`maxLines`/, 'names the rows it does take');
  // A span's own rows and the style rows are silent.
  const quiet = new Ctx();
  quiet.setDiagnostics(true);
  quiet.frame(320, 240, 1, box({ pad: 4 }, [
    el('text', { size: 14, lineHeight: 20, wrap: 'word' }, [el('span', { bold: true, bg: '#00ff00', underline: true }, ['a']), 'b']),
  ]));
  assert.deepEqual(quiet.warnings().filter((w) => w.code === 'unknown-prop'), []);
});

test('createApp reports unknown props too, and a shipped build does not', () => {
  const app = createApp(
    { init: 0, update: () => undefined, view: () => box({ onclick: 'go', width: 10, height: 10 }) },
    { warnings: false },
  );
  app.render();
  assert.equal(app.warnings.length, 1);
  assert.equal(app.warnings[0].code, 'unknown-prop');
  assert.match(app.warnings[0].message, /did you mean `onClick`\?/);
  const shipped = createApp(
    { init: 0, update: () => undefined, view: () => box({ onclick: 'go', width: 10, height: 10 }) },
    { diagnostics: false },
  );
  shipped.render();
  assert.deepEqual(shipped.warnings, []);
});

// A malformed view is rejected in JS, before anything crosses the boundary,
// and the message names the element or the prop and what it accepts.
test('a malformed view is rejected, with the offending name in the message', () => {
  const bad = [
    [() => el('nope'), /unknown element <nope>/],
    [() => ({ props: {}, children: [] }), /element without a type/],
    [() => el('edit', { initial: '' }), /<edit> needs a key or id prop/],
    [() => el('span', {}, ['x']), /<span> only works inside <text>/],
    [() => el('image', {}), /<image> needs a src/],
    [() => el('fragment', { src: '0000000000000001', image: 7 }), /<fragment> image must be an id/],
    [() => el('line', {}), /<line> needs from and to, or points/],
    [() => el('line', { points: [[0, 0]] }), /<line> needs at least two points/],
    [() => el('line', { from: [0, 0], to: [1], }), /bad point \[1\] for <line>/],
    [() => el('line', { from: [0, 0], to: [1, 1], width: 'grow' }), /bad width "grow" for <line>/],
    [() => box({ dir: 'diagonal' }), /bad dir "diagonal" \(row \| column \| table\)/],
    [() => box({ mainAlign: 'middle' }), /bad value "middle" for mainAlign \(one of start \| center \| end \| spaceBetween \| spaceAround \| spaceEvenly \| baseline\)/],
    [() => box({ bg: 'blue' }), /bad color "blue"/],
    [() => box({ width: 'huge' }), /bad sizing "huge"/],
    // Both places a float names a preset answer to the one table, so the
    // shorthand and the config object's `anchor` fail the same way.
    [() => box({ float: 'beneath' }), /bad float preset "beneath" \(parent \| viewport \| below \| above\)/],
    [() => box({ float: { anchor: 'beneath' } }), /bad float preset "beneath" \(parent \| viewport \| below \| above\)/],
    [
      () => text([el('span', {}, ['x']), el('button', {}, ['y'])]),
      /only strings and <span> may nest inside rich <text>/,
    ],
  ];
  for (const [build, message] of bad) assert.throws(() => run(build), message);
  // `null` is "none" for an optional prop, the way `sampling={null}` is
  // on `<image>`: a fragment with `image: null` encodes as one without.
  const src = '0000000000000001';
  assert.ok(
    encoded(el('fragment', { src, image: null, width: 8, height: 8 })).equals(encoded(el('fragment', { src, width: 8, height: 8 }))),
    '<fragment image={null}> is <fragment>',
  );
});

// A null tag declares the behaviour (here: a key sink) and leaves `tag` off
// the event, so a typed app needs no inert message for a sink that only
// needs the node key.
test('a null tag declares the behaviour without a tag on the event', () => {
  const build = () => box({}, [box({ onKey: null, keyFocus: true, width: 100, height: 50 }, [], 'sink')]);
  const { ctx } = run(build);
  ctx.keyDown('a');
  const evs = ctx.pollEvents();
  assert.equal(evs.length, 1, 'the sink got the key');
  const p = evs[0].payload;
  assert.equal(p.kind, 'key');
  assert.equal(p.code, 'a');
  assert.ok(!('tag' in p), 'no tag field');
});

// A keymap is the common sink, and it hears presses only: Space starts the
// timer once, not once on the way down and again on the way up. This is the
// alpha.4 shape of a key sink — an app that never asked for releases keeps
// toggling once after the bump (backlog F3).
test('a sink without keyUp hears a press once, release and all', () => {
  const build = () => box({ onKey: null, keyFocus: true, width: 100, height: 50 }, [], 'timer');
  const { ctx } = run(build);
  let running = false;
  const keymap = { space: () => (running = !running) };
  const pump = () => {
    for (const { payload: p } of ctx.pollEvents()) {
      if (p.kind === 'key' && !p.repeat) keymap[p.code]?.();
    }
  };
  ctx.keyDown('space');
  ctx.keyUp('space');
  pump();
  assert.equal(running, true, 'one toggle for a press and its release');
  // A repeat is still a press, and the guard against it is the app's.
  ctx.keyDown('space', {}, true);
  ctx.keyUp('space');
  pump();
  assert.equal(running, true, 'a repeat is filtered by `repeat`, not by a phase');
  // Focus leaving with a key held owes no release to a sink that never
  // asked for one — nothing arrives, and the stray physical release is
  // resolved silently.
  ctx.keyDown('m');
  ctx.pollEvents();
  ctx.blur();
  ctx.keyUp('m');
  assert.deepEqual(ctx.pollEvents(), [], 'no release, synthetic or real');
});

// Held keys: with `keyUp`, a press and its release are one payload shape
// apart by `phase`, so a game binds one handler. The release carries no
// `text` and never repeats, and a key held while focus moves comes up on
// the sink that took the press — nothing stays stuck down.
test('a key sink that asks for releases hears both halves of a held key', () => {
  const build = () =>
    box({}, [
      box({ onKey: { pane: 0 }, keyUp: true, keyFocus: true, width: 100, height: 50 }, [], 'a'),
      box({ onKey: { pane: 1 }, keyUp: true, width: 100, height: 50 }, [], 'b'),
    ]);
  const { ctx } = run(build);
  ctx.keyDown('w');
  ctx.keyDown('w', {}, true); // OS auto-repeat: still the same key down
  ctx.keyUp('w');
  const evs = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual(
    evs.map((p) => [p.phase, p.code, p.text, p.repeat]),
    [
      ['down', 'w', 'w', false],
      ['down', 'w', 'w', true],
      ['up', 'w', null, false],
    ],
    'down, repeat, up',
  );
  for (const p of evs) {
    assert.equal(p.kind, 'key', 'one kind for both phases');
    assert.deepEqual(p.tag, { pane: 0 }, "the sink's tag rides along");
  }
  // A release the sink never saw the press of resolves nothing.
  ctx.keyUp('w');
  assert.equal(ctx.pollEvents().length, 0, 'no phantom release');
});

// A focused editor keeps what it acts on and lets a chord it does not
// reach the sink above it; `keepTab` makes a field's Tab the app's
// (backlog F144, F146).
test('a chord and a keepTab field\'s Tab reach the sink above the editor', () => {
  const build = () =>
    box({ onKey: 'shell' }, [el('edit', { initial: 'ab', autofocus: true, keepTab: true, width: 200 }, [], 'f')]);
  const { ctx } = run(build);
  ctx.frame(320, 240, 1, build());
  ctx.pollEvents();
  ctx.keyDown('tab');
  ctx.keyDown('n', { ctrl: true });
  ctx.keyDown('q');
  const keys = ctx.pollEvents().filter((e) => e.payload.kind === 'key').map((e) => [e.payload.code, e.payload.tag]);
  assert.deepEqual(keys, [['tab', 'shell'], ['n', 'shell']], 'typing stays in the field');
});

// `bold` on a whole text and on an editor is a style row like
// `strikethrough` (backlog F150): read, not warned about, and wider.
test('bold is a style row on a text and an editor', () => {
  const view = (bold) =>
    box({ dir: 'row' }, [
      el('text', { bold }, ['Heading one'], 't'),
      el('edit', { initial: 'Title', label: 'Title', bold }, [], 'e'),
    ]);
  const ctx = new Ctx();
  ctx.frame(400, 100, 1, view(false));
  assert.deepEqual(ctx.warnings(), []);
  ctx.frame(400, 100, 1, view(true));
  assert.deepEqual(ctx.warnings(), [], 'no unknown-prop for bold');
});

// Where a key is and what the locks hold ride on every key payload, and
// the modifier keys themselves reach only a sink that asks (backlog F108).
test('a key says where it is, the locks, and the modifier keys when asked', () => {
  const build = (mods) =>
    box({ onKey: null, keyUp: true, modifierKeys: mods, keyFocus: true, width: 100, height: 50 }, [], 'a');
  let { ctx } = run(() => build(false));
  ctx.keyDown('1', { location: 'numpad', capsLock: true, numLock: true });
  ctx.keyDown('shift', { shift: true, location: 'left' });
  ctx.keyDown('f13');
  let evs = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual(
    evs.map((p) => [p.code, p.location, p.caps_lock, p.num_lock]),
    [
      ['1', 'numpad', true, true],
      ['f13', 'standard', false, false],
    ],
    'the keypad 1 with its place and locks; no Shift unasked; F13 a key',
  );
  ({ ctx } = run(() => build(true)));
  ctx.keyDown('shift', { shift: true, location: 'right' });
  ctx.keyUp('shift', { location: 'right' });
  evs = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual(
    evs.map((p) => [p.phase, p.code, p.location]),
    [
      ['down', 'shift', 'right'],
      ['up', 'shift', 'right'],
    ],
  );
  assert.throws(() => ctx.keyDown('a', { location: 'middle' }), /unknown key location/);
  // A press heard hands back as it came: the event's `caps_lock` spelling
  // is taken too (backlog RG86).
  ({ ctx } = run(() => build(false)));
  ctx.keyDown('1', { location: 'numpad', capsLock: true, numLock: true });
  const heard = ctx.pollEvents()[0].payload;
  ctx.keyDown(heard.code, heard);
  const again = ctx.pollEvents()[0].payload;
  assert.deepEqual([again.caps_lock, again.num_lock, again.location], [true, true, 'numpad']);
});

test('a keymap written in Latin survives the layout under it', () => {
  const build = () => box({ onKey: null, keyUp: true, keyFocus: true, width: 100, height: 50 }, [], 'a');
  const { ctx } = run(build);
  // What a driver reports: the layout's key, then the key's position. Omit
  // the position and it is the key you named.
  ctx.keyDown('w');
  // Russian: the key US-QWERTY prints W on produces "ц".
  ctx.keyDown('ц', {}, false, 'w');
  // Dvorak: the key printed V sits where QWERTY prints ".".
  ctx.keyDown('v', {}, false, '.');
  const evs = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual(
    evs.map((p) => [p.code, p.physical]),
    [
      ['w', 'w'],
      // Non-Latin: the position stands in, so `match code` keeps working.
      ['w', 'w'],
      // Latin: the layout wins, so the chord is on the key printed V —
      // and `physical` still says where that key actually is.
      ['v', '.'],
    ],
    'code follows the label while it is ASCII, the position otherwise',
  );
  // The release is spelled the same way and resolves its press.
  ctx.keyUp('ц', {}, 'w');
  const [up] = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual([up.phase, up.code, up.physical], ['up', 'w', 'w']);
  // The default folds a letter (F65): ⇧Z is spelled `Z` (F60), and a
  // window reports its position as `z` — its table never sees Shift — so
  // the omitted `physical` is `z` here too, the same pair, and not the
  // `Z` no window ever sends. A spelled one is delivered as spelled.
  ctx.press('Z', { shift: true });
  ctx.release('Z', { shift: true });
  ctx.press('Z', { shift: true }, false, 'Z');
  const shifted = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual(
    shifted.map((p) => [p.phase, p.code, p.physical, p.shift]),
    [
      ['down', 'Z', 'z', true],
      ['up', 'Z', 'z', true],
      ['down', 'Z', 'Z', true],
    ],
  );
  // Names and non-letters are their own position.
  ctx.press('$', { shift: true });
  ctx.press('left');
  assert.deepEqual(
    ctx.pollEvents().map((e) => [e.payload.code, e.payload.physical]),
    [
      ['$', '$'],
      ['left', 'left'],
    ],
  );
});

// RG27 + RG28: the two things F76's Shift-aware stand-in must leave alone.
// What the press types is the layout's own key — the stand-in is for the
// keymap, and an editor handed it typed `:` for Russian ⇧Ж — and under
// Alt the stand-in is the unshifted position, the `j` the winit runner
// reads for ⌥⇧J with every modifier stripped, where a door passing the
// composed `Ô` got `J`.
test('the stand-in is for the keymap: text is the layout key, and Alt keeps it unshifted', () => {
  const build = () => box({ onKey: null, keyFocus: true, width: 100, height: 50 }, [], 'a');
  const { ctx } = run(build);
  ctx.keyDown('Ж', { shift: true }, false, ';');
  ctx.keyDown('Ô', { alt: true, shift: true }, false, 'j');
  ctx.keyDown('О', { alt: true }, false, 'j');
  assert.deepEqual(
    ctx.pollEvents().map((e) => [e.payload.code, e.payload.physical, e.payload.text]),
    [
      [':', ';', 'Ж'],
      ['j', 'j', null],
      ['j', 'j', null],
    ],
  );

  const edit = new Ctx();
  const view = () => box({ pad: 8, width: 200 }, [el('input', { label: 'field', initial: '' })]);
  edit.frame(300, 200, 1, view());
  edit.focus('field');
  edit.frame(300, 200, 1, view());
  edit.press('Ж', { shift: true }, false, ';');
  edit.frame(300, 200, 1, view());
  assert.equal(edit.editText('field'), 'Ж', 'the editor gets what the layout typed');
});

// F115: on a layout the host says is not Latin, the US key stands in for
// the layout's ASCII too — macOS Russian's `]` on the key printed `` ` `` —
// and judged by itself, as before, the layout's ASCII wins. The text is the
// layout's either way.
test('a non-Latin layout reads every key as US-QWERTY', () => {
  const build = () => box({ onKey: null, keyFocus: true, width: 100, height: 50 }, [], 'a');
  const { ctx } = run(build);
  ctx.keyDown(']', {}, false, '`');
  ctx.keyDown(']', { layout: 'latin' }, false, '`');
  ctx.keyDown(']', { layout: 'nonLatin' }, false, '`');
  ctx.keyDown('"', { shift: true, layout: 'nonLatin' }, false, '2');
  assert.deepEqual(
    ctx.pollEvents().map((e) => [e.payload.code, e.payload.physical, e.payload.text]),
    [
      [']', '`', ']'],
      [']', '`', ']'],
      ['`', '`', ']'],
      ['@', '2', '"'],
    ],
  );
  assert.throws(() => ctx.keyDown(']', { layout: 'cyrillic' }, false, '`'), /unknown layout script/);
});

test('focus moving releases the keys the old sink held', () => {
  const build = () => box({ onKey: { pane: 0 }, keyUp: true, keyFocus: true, width: 100, height: 50 }, [], 'a');
  const { ctx } = run(build);
  ctx.keyDown('w');
  ctx.keyDown('a');
  const downs = ctx.pollEvents();
  assert.deepEqual(
    downs.map((e) => e.payload.phase),
    ['down', 'down'],
  );
  const sink = downs[0].key;
  // Focus dropped with both keys still down: two synthetic releases reach
  // the sink that took the presses, in press order, so a WASD binding
  // cannot be left walking forever.
  ctx.blur();
  const ups = ctx.pollEvents();
  assert.deepEqual(
    ups.map((e) => [e.payload.phase, e.payload.code, e.payload.text]),
    [
      ['up', 'w', null],
      ['up', 'a', null],
    ],
  );
  for (const e of ups) {
    assert.equal(e.key, sink, 'the sink that took the press hears the release');
    assert.deepEqual(e.payload.tag, { pane: 0 });
  }
  // And the physical release, arriving after the move, is not a second one.
  ctx.keyUp('w');
  assert.equal(ctx.pollEvents().length, 0);
});

test('the window losing the keyboard releases the keys its sink held', () => {
  // What the windowed driver does on Cmd-Tab, reachable headless: the OS
  // stops delivering key events to a window that lost the keyboard, so
  // the core lets go on the report itself (`Core::set_focused`) rather
  // than each driver remembering to. Before this a custom Node driver had
  // no door for it at all.
  const build = () => box({ onKey: { pane: 0 }, keyUp: true, keyFocus: true, width: 100, height: 50 }, [], 'a');
  const { ctx } = run(build);
  ctx.keyDown('w');
  const [down] = ctx.pollEvents();
  ctx.setEnv({ focused: false });
  const evs = ctx.pollEvents();
  assert.deepEqual(
    evs.filter((e) => e.payload.kind === 'key').map((e) => [e.key, e.payload.phase, e.payload.code]),
    [[down.key, 'up', 'w']],
  );
  // And the app hears the window go, once (backlog DX18).
  assert.deepEqual(evs.filter((e) => e.payload.kind === 'window').map((e) => e.payload.phase), ['blurred']);
  assert.equal(ctx.env().focused, false);
  // Saying it again changes nothing, and getting the keyboard back
  // releases nothing: there is nothing held — only the window's return.
  ctx.setEnv({ focused: false });
  ctx.setEnv({ focused: true });
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload.kind + ' ' + e.payload.phase), ['window focused']);
  ctx.keyUp('w');
  assert.equal(ctx.pollEvents().length, 0, 'the physical release is not a second one');
});

// Keyboard focus as data: Tab reaches a button, a disabled box is not a
// stop, Enter presses the focused button, and the access tree reports the
// same focus.
test('tab reaches a button and enter presses it', () => {
  const build = () =>
    box({ pad: 4 }, [
      box({ width: 60, height: 20, bg: '#333333', onClick: { kind: 'go' } }, [text('go', { size: 12 })], 'go'),
      box({ width: 60, height: 20, bg: '#333333', focusable: true, disabled: true }, [], 'off'),
    ]);
  const { ctx } = run(build);
  assert.equal(ctx.focused(), null, 'nothing focused at first');
  ctx.key('tab');
  const focused = ctx.focused();
  assert.ok(focused, 'tab landed on the button');
  assert.ok(ctx.focusVisible(), 'keyboard focus shows');
  assert.ok(ctx.isFocused(focused));
  const tree = ctx.accessTree();
  assert.equal(tree.focus, focused);
  const node = tree.nodes.find((n) => n.key === focused);
  assert.equal(node.role, 'button');
  assert.ok(node.focused && !node.disabled);
  assert.ok(node.actions.includes('focus'));
  const off = tree.nodes.find((n) => n.disabled);
  assert.equal(off.role, 'group', 'a focusable box is in the tree');
  assert.ok(!off.actions.includes('focus'), 'disabled: not focusable');
  ctx.key('tab');
  assert.equal(ctx.focused(), focused, 'the disabled box is not a stop');
  ctx.key('enter');
  const evs = ctx.pollEvents();
  assert.equal(evs.length, 1, 'enter pressed the button');
  assert.deepEqual(evs[0].payload, { kind: 'go' });
  ctx.blur();
  assert.equal(ctx.focused(), null);
  ctx.focus(focused);
  assert.equal(ctx.focused(), focused, 'focus(key) moves focus');
  assert.ok(ctx.focusVisible(), 'programmatic focus keeps the keyboard modality');
  ctx.focusNext();
  assert.equal(ctx.focused(), focused, 'the only stop wraps to itself');
});

// Focus regions (ADR 0022): a `focusRegion` box is a Tab ring of its own
// that the app's ring never enters; `focusRegion(name)` enters it — by a
// label the frame resolves, so the update that toggles a dock on can enter
// it in the same turn — and `focusRegion(null)` comes back to what the app
// last held. `region()` says which ring Tab is walking.
test('a focusRegion is its own Tab ring, entered by name', () => {
  const btn = (key) => box({ width: 60, height: 20, onClick: { kind: key }, label: key }, [], key);
  const view = (dock) =>
    box({ pad: 4 }, [
      btn('a'),
      btn('b'),
      dock ? box({ width: 200, height: 60, focusRegion: true, label: 'Devtools' }, [btn('d1'), btn('d2')], 'dock') : null,
    ]);
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, view(true));
  assert.equal(ctx.region(), null, 'the main ring to start');
  const seen = [];
  for (let i = 0; i < 3; i++) {
    ctx.key('tab');
    seen.push(ctx.focused());
  }
  assert.deepEqual(seen, [ctx.keyOf('a'), ctx.keyOf('b'), ctx.keyOf('a')], 'Tab wraps over the app and never enters the dock');
  // Enter by name: deferred to the frame, and it shows.
  ctx.focusRegion('dock');
  ctx.frame(320, 240, 1, view(true));
  assert.equal(ctx.region(), ctx.keyOf('dock'));
  assert.equal(ctx.focused(), ctx.keyOf('d1'), 'first stop, nothing remembered');
  assert.ok(ctx.focusVisible());
  ctx.key('tab');
  ctx.key('tab');
  assert.equal(ctx.focused(), ctx.keyOf('d1'), 'inside, Tab wraps over the dock alone');
  ctx.key('tab');
  // Back to the app, where `a` was the last focus; the dock remembers d2.
  ctx.focusRegion(null);
  ctx.frame(320, 240, 1, view(true));
  assert.equal(ctx.region(), null);
  assert.equal(ctx.focused(), ctx.keyOf('a'));
  ctx.focusRegion(ctx.keyOf('dock'));
  ctx.frame(320, 240, 1, view(true));
  assert.equal(ctx.focused(), ctx.keyOf('d2'), 'the hex spelling, and what the dock last held');
  // The dock toggled off with focus inside it: the app gets its focus back.
  ctx.frame(320, 240, 1, view(false));
  assert.equal(ctx.region(), null);
  assert.equal(ctx.focused(), ctx.keyOf('a'));
  // Toggled on and entered in one turn: the label names a node the last
  // frame did not have, and the frame that draws it resolves the call.
  ctx.focusRegion('dock');
  ctx.frame(320, 240, 1, view(true));
  assert.equal(ctx.focused(), ctx.keyOf('d2'));
  assert.deepEqual(ctx.warnings(), []);
  // A name no region answers to is a warning, and nothing moves.
  ctx.focusRegion('inspector');
  ctx.frame(320, 240, 1, view(true));
  assert.equal(ctx.focused(), ctx.keyOf('d2'));
  assert.deepEqual(ctx.warnings().map((w) => w.code), ['focus-region-without-node']);
});

// Backlog F5: a node the app never interacted with is named by the label
// its `key` declared. `focus`, `isFocused`, `reveal` and `access` take that
// spelling beside the hex one, resolved through the last frame — the path
// from the root runs through auto-keyed ancestors JS cannot spell.
test('focus, isFocused and access resolve a declared label', () => {
  const build = () =>
    box({ pad: 4 }, [
      box({}, [
        box({ width: 60, height: 20, focusable: true }, [], 'alpha'),
        box({ width: 60, height: 20, onClick: { kind: 'beta' }, label: 'beta' }, [], 'beta'),
        el('edit', { initial: '', label: 'Note', width: 100 }, [], 'note'),
      ]),
    ]);
  const { ctx } = run(build);
  assert.equal(ctx.focused(), null);
  // The case the finding came from: an editor the app just created.
  ctx.focus('note');
  assert.equal(ctx.focused(), nodesByName(ctx).Note.key, 'the editor, by its key prop');
  ctx.focus('beta'); // never clicked, tabbed to, or reported by an event
  const hex = ctx.focused();
  assert.match(hex, /^[0-9a-f]{16}$/, 'focused() still reports the hex key');
  assert.ok(ctx.isFocused('beta') && ctx.isFocused(hex), 'either spelling');
  assert.ok(!ctx.isFocused('alpha'));
  // The hex path is unchanged: what an event carried is still accepted.
  ctx.blur();
  ctx.focus(hex);
  assert.equal(ctx.focused(), hex);
  ctx.access('beta', 'click');
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [{ kind: 'beta' }]);
  // A label nothing declared is an error that names both spellings —
  // "bad id" named neither. On a *command*: a name nothing answers to is a
  // typo the app wants told about. A query answers instead (C25), which is
  // its own test below.
  assert.throws(() => ctx.focus('gamma'), /no node is keyed "gamma".*`key` prop.*hex key/);
  assert.throws(() => ctx.access('gamma', 'click'), /no node is keyed/);
  assert.equal(ctx.isFocused('gamma'), false);
  assert.deepEqual(ctx.warnings(), []);
});

test('two nodes on one label: the first in tree order wins, and the frame warns once', () => {
  const build = () =>
    box({ pad: 4 }, [
      box({}, [box({ width: 60, height: 20, focusable: true }, [], 'beta')]),
      box({}, [box({ width: 60, height: 20, focusable: true }, [], 'beta')]),
    ]);
  const { ctx } = run(build);
  ctx.focus('beta');
  const first = ctx.focused();
  const groups = ctx.accessTree().nodes.filter((n) => n.role === 'group');
  assert.equal(groups.length, 2, 'both focusable boxes are in the tree');
  assert.equal(first, groups[0].key, 'the first in tree order');
  const ws = ctx.warnings();
  assert.deepEqual(ws.map((w) => w.code), ['ambiguous-key']);
  assert.equal(ws[0].key, first);
  assert.match(ws[0].message, /2 nodes are keyed "beta"/);
  assert.ok(ctx.isFocused('beta'));
  assert.deepEqual(ctx.warnings(), [], 'once per label');
});

// Modal surfaces (docs/adr/0003-modal-surfaces.md): the dialog takes focus
// and keeps it, the app behind it is inert, and Escape and a press outside
// both ask it to close.
test('a modal contains focus and asks to be dismissed', () => {
  const build = () =>
    box({ pad: 4 }, [
      box(
        { width: 60, height: 20, bg: '#333333', onClick: { kind: 'open' }, label: 'Open' },
        [],
        'open',
      ),
      box(
        {
          width: 80,
          height: 40,
          bg: '#222222',
          float: { anchor: 'viewport', at: ['end', 'end'], self: ['end', 'end'] },
          modal: { kind: 'settings' },
          label: 'Settings',
        },
        [
          box(
            { width: 60, height: 20, bg: '#444444', onClick: { kind: 'ok' }, label: 'OK' },
            [],
            'ok',
          ),
        ],
        'dialog',
      ),
    ]);
  const { ctx } = run(build);
  const tree = ctx.accessTree();
  const dialog = tree.nodes.find((n) => n.modal);
  assert.ok(dialog, 'the access tree reports the modal');
  assert.equal(dialog.role, 'dialog', 'a modal box is a dialog');
  assert.equal(dialog.name, 'Settings');
  const ok = tree.nodes.find((n) => n.name === 'OK');
  assert.equal(tree.focus, ok.key, 'focus entered the modal');
  assert.equal(ctx.focused(), ok.key);

  // The one stop in the ring is inside the dialog: Tab cannot leave.
  ctx.key('tab');
  assert.equal(ctx.focused(), ok.key, 'tab stays inside');

  // A press on the button behind emits no click, only the dismiss.
  ctx.cursor(10, 10);
  ctx.mouse(true, 1);
  ctx.mouse(false, 1);
  const outside = ctx.pollEvents();
  assert.equal(outside.length, 1, 'the button behind is inert');
  assert.deepEqual(outside[0].payload, {
    kind: 'dismiss',
    reason: 'outside',
    tag: { kind: 'settings' },
  });

  ctx.key('escape');
  const escaped = ctx.pollEvents();
  assert.equal(escaped.length, 1);
  assert.deepEqual(escaped[0].payload, {
    kind: 'dismiss',
    reason: 'escape',
    tag: { kind: 'settings' },
  });
  assert.equal(escaped[0].key, dialog.key, 'on the modal node');
  assert.equal(ctx.focused(), ok.key, 'escape does not let go');
});

// Backlog F6: a real key press is two channels and a window drives both —
// the raw press to an `onKey` sink, and then what the core is asked to do
// with that key. `press` is the pair. Six of the mind map's first-run
// failures were `keyDown('escape')` reaching the editor's keymap and
// leaving the modal it sat in open, because the second channel is where
// dismissal lives.
test('press drives both channels: the sink hears the key and the core acts on it', () => {
  const build = () =>
    box({ pad: 4 }, [
      box({ width: 60, height: 20, bg: '#333333', onClick: { kind: 'behind' } }, [], 'behind'),
      box(
        {
          width: 80,
          height: 40,
          bg: '#222222',
          float: { anchor: 'viewport', at: ['end', 'end'], self: ['end', 'end'] },
          modal: { kind: 'editor' },
          label: 'Editor',
        },
        [
          box(
            {
              width: 60,
              height: 20,
              bg: '#444444',
              onKey: { pane: 'notes' },
              keyFocus: true,
              label: 'Notes',
            },
            [],
            'notes',
          ),
        ],
        'dialog',
      ),
    ]);
  const { ctx } = run(build);
  assert.ok(ctx.isFocused('notes'), 'the sink inside the modal holds the keyboard');

  // The half that was never enough: the sink hears the key, the modal
  // stays.
  ctx.keyDown('escape');
  const half = ctx.pollEvents();
  assert.deepEqual(
    half.map((e) => e.payload.kind),
    ['key'],
    'keyDown alone is the sink channel and nothing else',
  );

  // The whole press: the same event, and then the dismissal, in the order
  // a window sends them.
  ctx.press('escape');
  const whole = ctx.pollEvents();
  assert.deepEqual(
    whole.map((e) => e.payload.kind),
    ['key', 'dismiss'],
    'press reaches the sink and dismisses the modal',
  );
  assert.equal(whole[0].payload.code, 'escape');
  assert.deepEqual(whole[0].payload.tag, { pane: 'notes' });
  assert.deepEqual(whole[1].payload, {
    kind: 'dismiss',
    reason: 'escape',
    tag: { kind: 'editor' },
  });

  // The release is the other end of the same key, and one channel: the
  // editing keys act on the way down. This sink never asked for `keyUp`,
  // so it hears nothing at all.
  ctx.release('escape');
  assert.deepEqual(ctx.pollEvents(), [], 'a sink without keyUp hears no release');
});

// The other three keys the split hid, from the pomodoro report's side of
// it: they are all on the channel `keyDown` is not.
test('press walks the focus ring, presses a control and nudges a slider', () => {
  const app = createApp(
    {
      init: { pressed: 0, nudged: null },
      update: (m, msg) => {
        if (msg === 'go') return { ...m, pressed: m.pressed + 1 };
        if (msg?.kind === 'access') return { ...m, nudged: msg.action };
      },
      view: () =>
        box({ pad: 4, gap: 4 }, [
          box({ onClick: 'go', width: 60, height: 20, bg: '#333333', label: 'Go' }, [], 'go'),
          box(
            {
              role: 'slider',
              label: 'Volume',
              valueNow: 3,
              valueMin: 0,
              valueMax: 10,
              onDrag: 'vol',
              width: 100,
              height: 10,
            },
            [],
            'vol',
          ),
        ]),
    },
    { warnings: false },
  );
  app.render();
  assert.equal(app.ctx.focused(), null, 'nothing focused at first');
  app.press('tab');
  assert.ok(app.ctx.isFocused('go'), 'tab moved focus onto the button');
  app.press(' ');
  assert.equal(app.model.pressed, 1, 'space pressed the focused control');
  app.press('tab');
  assert.ok(app.ctx.isFocused('vol'), 'tab moved on to the slider');
  app.press('right');
  assert.equal(app.model.nudged, 'increment', 'right nudged the focused slider');
  assert.deepEqual(app.warnings, []);
});

// The row that says where a modal opens focused: a destructive confirm on
// its Cancel rather than on whichever control is declared first.
test('initialFocus names the control a modal opens on', () => {
  const build = (withRow) =>
    box({ pad: 4 }, [
      box(
        { width: 60, height: 20, bg: '#333333', onClick: { kind: 'open' }, label: 'Open' },
        [],
        'open',
      ),
      box(
        {
          width: 80,
          height: 60,
          bg: '#222222',
          float: { anchor: 'viewport', at: ['end', 'end'], self: ['end', 'end'] },
          modal: null,
          label: 'Delete note',
        },
        [
          // The destructive one first: declaration order alone would open
          // the dialog on it.
          box(
            { width: 60, height: 20, bg: '#444444', onClick: { kind: 'delete' }, label: 'Delete' },
            [],
            'delete',
          ),
          box(
            {
              width: 60,
              height: 20,
              bg: '#444444',
              onClick: { kind: 'cancel' },
              label: 'Cancel',
              ...(withRow ? { initialFocus: true } : {}),
            },
            [],
            'cancel',
          ),
        ],
        'dialog',
      ),
    ]);

  const named = new Ctx();
  named.frame(320, 240, 1, build(true));
  const keyOf = (name) => named.accessTree().nodes.find((n) => n.name === name).key;
  assert.equal(named.focused(), keyOf('Cancel'), 'opened on the safe option');
  // Read on entry only: a Tab press moves off it, and the next frame -
  // declaring the same row again - leaves focus where the user put it.
  named.key('tab');
  assert.equal(named.focused(), keyOf('Delete'));
  named.frame(320, 240, 1, build(true));
  assert.equal(named.focused(), keyOf('Delete'), 'a redeclaration is not an entry');

  // Without the row, ADR 0003's first-focusable rule stands.
  const plain = new Ctx();
  plain.frame(320, 240, 1, build(false));
  const delete_ = plain.accessTree().nodes.find((n) => n.name === 'Delete').key;
  assert.equal(plain.focused(), delete_, 'the first control, as before');
});

// Measurement is a query on the same text stack layout uses.
test('measureText answers what layout gives the text', () => {
  const ctx = new Ctx();
  const m = ctx.measureText('hello world', { size: 14 });
  assert.ok(m.width > 0 && m.height > 0 && m.lines === 1, JSON.stringify(m));
  // A fit-sized box with a background is exactly its text's size.
  ctx.frame(320, 240, 1, box({}, [box({ bg: '#ffffff' }, [text('hello world', { size: 14 })])]));
  const q = decodeQuads(ctx.quads()).find((q) => q.kind === 0);
  assert.equal(q.w, m.width);
  assert.equal(q.h, m.height);
  const narrow = ctx.measureText('hello world again', { size: 14 }, m.width / 2);
  assert.ok(narrow.lines > 1 && narrow.width < m.width, JSON.stringify(narrow));
  const clamped = ctx.measureText('hello world again', { size: 14, ellipsis: true }, m.width / 2);
  assert.equal(clamped.lines, 1);
  const rich = ctx.measureText(['hello ', el('span', { bold: true }, ['world'])], { size: 14 });
  assert.ok(rich.width > 0 && rich.lines === 1);
});

// A measured rich text and a drawn one are flattened by the same encoder
// and read by the same decoder, so their widths agree to the pixel — the
// invariant a second span walker in the addon could not promise.
test('measureText of a rich tree is the width the same tree draws at', () => {
  const ctx = new Ctx();
  const content = ['hello ', el('span', { bold: true }, ['big ', el('span', { italic: true }, ['world'])])];
  const m = ctx.measureText(content, { size: 14 });
  const plain = ctx.measureText('hello big world', { size: 14 });
  assert.notEqual(m.width, plain.width, 'bold and italic runs shape differently from plain text');
  ctx.frame(320, 240, 1, box({}, [box({ bg: '#ffffff' }, [el('text', { size: 14 }, content)])]));
  const q = decodeQuads(ctx.quads()).find((q) => q.kind === 0);
  assert.equal(q.w, m.width);
  assert.equal(q.h, m.height);
  // A style name the schema does not know is reported, as a view's would be.
  ctx.setDiagnostics(true);
  ctx.measureText('x', { size: 14, sizee: 3 });
  assert.ok(ctx.warnings().some((w) => w.code === 'unknown-prop'), 'unknown style prop reported');
});

// A terminal's screen as one node (backlog C20): four entries a cell in a
// Uint32Array, a click that names its cell, and the screen as the access
// tree's value.
test('cells draws a screen from a Uint32Array and a click names its cell', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const rows = 2, cols = 11;
  const grid = new Uint32Array(rows * cols * 4);
  const put = (r, c, ch, fg = 0xffffffff, bg = 0, flags = 0) => {
    const i = (r * cols + c) * 4;
    grid[i] = ch.codePointAt(0); grid[i + 1] = fg; grid[i + 2] = bg; grid[i + 3] = flags;
  };
  for (let c = 0; c < cols; c++) put(0, c, 'hello world'[c]);
  put(1, 2, 'b', 0xff0000ff, 0x0000ffff, 1); put(1, 3, 'y', 0xff0000ff, 0x0000ffff); put(1, 4, 'e');
  const view = box({ pad: 10 }, [
    el('cells', { ...mono, rows, cols, cells: grid, cursorAt: [1, 4], cursorShape: 'bar', onClick: { kind: 'hit' } }, [], 'term'),
  ]);
  ctx.frame(400, 200, 1, view);
  assert.deepEqual(ctx.warnings(), []);
  const quads = decodeQuads(ctx.quads());
  const glyphs = quads.filter((q) => q.kind === 1);
  assert.equal(glyphs.length, 13, 'hello world + bye');
  const solids = quads.filter((q) => q.kind === 0 && q.h === 20);
  assert.equal(solids.length, 2, 'one blue run and the bar cursor (plus the root)');
  // The access tree reads the screen.
  const term = ctx.accessTree().nodes.find((n) => n.role === 'terminal');
  assert.equal(term.value, 'hello world\n  bye');
  // A click in the fourth cell of the second row names it.
  const w = ctx.measureText('M', mono).width;
  ctx.cursor(10 + 3.5 * Math.round(w), 10 + 25);
  ctx.mouse(true);
  ctx.mouse(false);
  const hit = ctx.pollEvents().map((e) => e.payload).find((p) => p.kind === 'hit');
  assert.deepEqual(hit, { kind: 'hit', cell: { row: 1, col: 3 } });
});

// A `selectable` grid selects in cells, and the click count picks the
// grain the same way it does over a paragraph: one a cell, two the word,
// three the whole row (ADR 0017, decision 4). Driven from Node because
// the click count is the binding's to pass on — `mouse(down, clicks)`.
test('a cells grid selects by cell, word and row from the click count', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const rows = 2, cols = 12;
  const grid = new Uint32Array(rows * cols * 4);
  const put = (r, c, ch) => {
    const i = (r * cols + c) * 4;
    grid[i] = ch.codePointAt(0); grid[i + 1] = 0xffffffff; grid[i + 2] = 0; grid[i + 3] = 0;
  };
  for (let c = 0; c < cols; c++) {
    put(0, c, 'hello world'[c] ?? ' ');
    put(1, c, 'bye there  '[c] ?? ' ');
  }
  const view = box({}, [
    el('cells', { ...mono, rows, cols, cells: grid, originLine: 900, selectable: true }, [], 'term'),
  ]);
  ctx.frame(400, 200, 1, view);
  assert.deepEqual(ctx.warnings(), []);
  const w = Math.round(ctx.measureText('M', mono).width);
  // The middle of (row, col), in the grid's own metrics.
  const at = (r, c) => ctx.cursor((c + 0.5) * w, (r + 0.5) * 20);

  // One click: a cell, and a drag from it selects cells.
  at(0, 0); ctx.mouse(true, 1); at(0, 5); ctx.mouse(false);
  assert.equal(ctx.selectionText(), 'hello');

  // Two: the word under the pointer, whole.
  at(0, 8); ctx.mouse(true, 2); ctx.mouse(false);
  assert.equal(ctx.selectionText(), 'world');

  // Two, held and dragged: word by word, both ends rounding outwards, so
  // the word the drag started in stays whole across the rows.
  at(0, 8); ctx.mouse(true, 2); at(1, 1); ctx.mouse(false);
  assert.equal(ctx.selectionText(), 'world\nbye');

  // Three: the whole row, edge to edge; the copy trims its blanks.
  at(1, 5); ctx.mouse(true, 3); ctx.mouse(false);
  assert.equal(ctx.selectionText(), 'bye there');

  // And the stock menu over it acts on the cells, not on a text
  // selection the grid does not have. The right-click leaves the row it
  // is about selected, so Copy takes that whole row.
  at(1, 5); ctx.mouse(true, 1, 'secondary'); ctx.mouse(false, 1, 'secondary');
  const menu = ctx.menu();
  const copy = menu.items.findIndex((i) => i.role === 'copy');
  assert.ok(copy >= 0 && menu.items[copy].enabled, 'Copy is lit by the cell selection');
  ctx.activateMenuItem(copy);
  assert.deepEqual(ctx.takeMenuActions(), [{ kind: 'setClipboard', text: 'bye there', html: null }]);
});

// Underline, strikethrough and a background per span (backlog C22): solid
// quads beside the glyphs, the background under them and the lines over.
test('dash cuts a line and a path\'s stroke into marks, and a count nobody spells is refused (V2)', () => {
  const segments = (props) => {
    const b = run(() => root({}, [box({ width: 200, height: 40 }, [el('line', { from: [0, 20], to: [100, 20], width: 2, ...props })])]));
    const stride = b.quads.byteLength / b.stats.quadCount;
    let n = 0;
    for (let off = 0; off < b.quads.byteLength; off += stride) {
      if (b.quads.readUInt32LE(off + KIND_WORD * 4) === 6) n++;
    }
    return n;
  };
  assert.equal(segments({}), 1);
  // One length is marks and gaps alike; a pair is a mark and a gap.
  assert.equal(segments({ dash: 5 }), 10);
  assert.equal(segments({ dash: [6, 4] }), 10);
  assert.equal(segments({ dash: [6, 4], dashOffset: 5 }), 10);
  assert.equal(segments({ dash: [10, 4, 2, 4] }), 10);
  // No gap is no pattern.
  assert.equal(segments({ dash: [6, 0] }), 1);
  assert.throws(() => probe.encode(el('line', { from: [0, 0], to: [9, 9], dash: [1, 2, 3] })), /bad dash/);
  assert.throws(() => probe.encode(el('line', { from: [0, 0], to: [9, 9], dash: 'dotted' })), /bad dash/);
  assert.throws(() => probe.encode(el('line', { from: [0, 0], to: [9, 9], dash: 4, dashOffset: '1' })), /bad dashOffset/);
  assert.throws(() => probe.encode(el('path', { d: 'M0 0 H9', width: 1, dash: [] })), /bad dash/);
  // A path's dash rides after its pivot and the stream stays in step.
  assertLowers('dashed path', () => root({}, [box({ width: 200, height: 40 }, [el('path', { d: 'M0 20 H100', width: 2, dash: [6, 4], dashOffset: 2 }), text('after', { size: 12 })])]));
});

test('a span carries its own background and lines', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  ctx.frame(300, 100, 1, box({}, [
    text(['let ', el('span', { bg: '#3b5bd455', underline: true }, ['value']), ' = 1;'], mono),
  ]));
  const quads = decodeQuads(ctx.quads());
  const solids = quads.filter((q) => q.kind === 0);
  assert.equal(solids.length, 2, JSON.stringify(solids));
  const [bg, line] = solids;
  const w = ctx.measureText('M', mono).width;
  assert.ok(Math.abs(bg.x - 4 * w) < 1 && Math.abs(bg.w - 5 * w) < 1.5, JSON.stringify(bg));
  assert.equal(bg.h, 20, 'the whole line');
  assert.ok(line.h >= 1 && line.h < 20 && line.y > bg.y, 'a thin line lower down');
  const firstGlyph = quads.findIndex((q) => q.kind === 1);
  assert.ok(quads.indexOf(bg) < firstGlyph, 'background under the glyphs');
  assert.ok(quads.indexOf(line) > firstGlyph, 'line over them');
  // Whole-text decorations are style rows.
  ctx.frame(300, 100, 1, box({}, [text('struck', { ...mono, strikethrough: true })]));
  assert.equal(decodeQuads(ctx.quads()).filter((q) => q.kind === 0).length, 1);
});

// An underline's own colour and shape (backlog K4): on a span, on a whole
// text and on a cell. A solid one is the rect C22 drew; a wave or dots are
// runs of the segment quad (kind 6) a `line` draws, so no backend learned
// a kind; a colour of its own tints the line and leaves the glyphs alone.
test("an underline can be its own colour and a wave or dots (K4)", () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const isRed = (q) => q.color[0] === 1 && q.color[1] === 0 && q.color[2] === 0;
  const draw = (tree) => {
    ctx.frame(400, 100, 1, box({}, [tree]));
    return decodeQuads(ctx.quads());
  };
  // A span: the wave's pieces are segments in the underline's colour; the
  // glyphs keep the text's.
  let quads = draw(text(['let ', el('span', { underlineColor: '#ff0000', underlineStyle: 'wavy' }, ['value']), ';'], mono));
  const segs = quads.filter((q) => q.kind === 6);
  assert.ok(segs.length >= 4, `a wave is pieces: ${segs.length}`);
  assert.ok(segs.every(isRed), 'in the underline colour');
  assert.equal(quads.filter((q) => q.kind === 0).length, 0, 'no solid line beside them');
  assert.ok(quads.filter((q) => q.kind === 1).every((q) => !isRed(q)), 'glyphs keep the text colour');
  const w = ctx.measureText('M', mono).width;
  const x0 = Math.min(...segs.map((q) => q.x));
  const x1 = Math.max(...segs.map((q) => q.x + q.w));
  assert.ok(Math.abs(x0 - 4 * w) < 4 && Math.abs(x1 - 9 * w) < 4, `under the span: ${x0}..${x1} vs ${4 * w}..${9 * w}`);
  // Dots: zero-length segments.
  quads = draw(text(['a', el('span', { underlineStyle: 'dotted' }, ['bcdef'])], mono));
  const dots = quads.filter((q) => q.kind === 6);
  assert.ok(dots.length >= 3, `dots: ${dots.length}`);
  assert.ok(dots.every((q) => q.ends[0] === q.ends[2] && q.ends[1] === q.ends[3]), 'each a dot');
  // A whole text: the style rows.
  quads = draw(text('warn', { ...mono, underlineColor: '#ff0000', underlineStyle: 'wavy' }));
  assert.ok(quads.filter((q) => q.kind === 6).length >= 3);
  // The colour alone: still a solid line, in that colour.
  quads = draw(text('warn', { ...mono, underlineColor: '#ff0000' }));
  const line = quads.filter((q) => q.kind === 0);
  assert.equal(line.length, 1);
  assert.ok(isRed(line[0]));
  // A `$name` resolves for it like any colour row.
  ctx.setTokens({ colors: { err: { light: '#ff0000', dark: '#ff0000' } } });
  quads = draw(text(['x', el('span', { underlineColor: '$err' }, ['y'])], mono));
  assert.equal(quads.filter((q) => q.kind === 0 && isRed(q)).length, 1, 'a token underline colour on a span');
  // Cells: five entries a cell, the fifth the underline's colour; the
  // wave bit implies the line.
  const cells = new Uint32Array(3 * 5);
  for (let i = 0; i < 3; i++) {
    cells[i * 5] = 'a'.charCodeAt(0) + i;
    cells[i * 5 + 1] = 0xffffffff;
    cells[i * 5 + 3] = 32; // wavy
    cells[i * 5 + 4] = 0xff0000ff;
  }
  quads = draw(el('cells', { rows: 1, cols: 3, cells, ...mono }));
  const curl = quads.filter((q) => q.kind === 6);
  assert.ok(curl.length >= 3, `an undercurl over three cells: ${curl.length}`);
  assert.ok(curl.every(isRed));
  // Four entries a cell still work: the underline is the foreground's.
  const four = new Uint32Array([0x61, 0xffffffff, 0, 4]);
  quads = draw(el('cells', { rows: 1, cols: 1, cells: four, ...mono }));
  assert.equal(quads.filter((q) => q.kind === 0).length, 1);
  // And a bad style is refused where it is declared.
  assert.throws(() => draw(text([el('span', { underlineStyle: 'squiggly' }, ['x'])], mono)), /bad underlineStyle/);
});

// A rounded span background (backlog F101): `bgRadius` makes a span's
// background a piece of the stock join fragment (kind 7) instead of a
// solid, one a line, told the pieces it meets in the texts above and below.
// Square is what it was; a nested span inherits the radius.
test('a rounded span background is joined with the ones it meets (F101)', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const sel = '#3b5bd466';
  const draw = (rows) => {
    ctx.frame(400, 200, 1, box({}, rows));
    return decodeQuads(ctx.quads());
  };
  const row = (s, props) => text([el('span', { bg: sel, ...props }, [s])], mono);
  let quads = draw([row('first line', { bgRadius: 4 }), row('second', { bgRadius: 4 }), row('third line here', { bgRadius: 4 })]);
  assert.equal(quads.filter((q) => q.kind === 7).length, 3, 'a piece a row');
  assert.equal(quads.filter((q) => q.kind === 0).length, 0, 'no square background beside them');
  // Inherited from the enclosing span.
  quads = draw([text([el('span', { bg: sel, bgRadius: 4 }, ['a ', el('span', { bold: true }, ['b'])])], mono)]);
  assert.equal(quads.filter((q) => q.kind === 7).length, 1, 'one run, one piece');
  // Without a radius, the square background it always was.
  quads = draw([row('first line'), row('second')]);
  assert.equal(quads.filter((q) => q.kind === 7).length, 0);
  assert.equal(quads.filter((q) => q.kind === 0).length, 2);
  assert.throws(() => draw([row('x', { bgRadius: 'round' })]), /bad bgRadius/);
});

// A span's own face and size: inline code in `mono` measures as the
// monospace face does, a larger span makes its line taller, and both are
// inherited by a nested span; a bad one is refused where it is declared.
test('a span takes a face and a size of its own', () => {
  const ctx = new Ctx();
  const style = { size: 14 };
  const sans = ctx.measureText(['iiiiii'], style);
  const code = ctx.measureText([el('span', { family: 'mono' }, ['iiiiii'])], style);
  const mono = ctx.measureText('iiiiii', { size: 14, family: 'mono' });
  assert.ok(Math.abs(code.width - mono.width) < 0.5, `${code.width} against ${mono.width}`);
  assert.ok(code.width > sans.width + 4, `${code.width} against ${sans.width}`);
  const big = ctx.measureText(['a', el('span', { size: 28 }, [el('span', { bold: true }, ['B'])])], style);
  assert.ok(big.height > sans.height * 1.8, `${big.height} against ${sans.height}`);
  assert.throws(() => ctx.measureText([el('span', { size: -1 }, ['x'])], style), /bad size/);
  assert.throws(() => ctx.measureText([el('span', { family: 3 }, ['x'])], style), /bad family/);
  // A span's own family beats a font handle the enclosing span gave it:
  // the handle wins only on the span that declares both.
  const face = ['Helvetica', 'Arial', 'DejaVu Sans', 'Liberation Sans'].map((n) => ctx.addSystemFont(n)).find((id) => id != null);
  if (face != null) {
    const nested = ctx.measureText([el('span', { font: face }, [el('span', { family: 'mono' }, ['iiiiii'])])], style);
    assert.ok(Math.abs(nested.width - mono.width) < 0.5, `${nested.width} against ${mono.width}`);
  }
});

// OpenType features on a text style (backlog C23): one string every
// binding shares, part of what the text is shaped as. The ligature half runs
// only where a font with one is installed.
test('features reach the shaper and are part of what a text is shaped as', () => {
  const ctx = new Ctx();
  ctx.frame(300, 100, 1, box({}, [
    text('fi ->', { size: 16 }),
    text('fi ->', { size: 16, features: 'liga=0 calt=0' }),
    text('fi ->', { size: 16 }),
  ]));
  assert.equal(ctx.textCacheBytes() > 0, true);
  const id = ['Fira Code', 'Cascadia Code', 'JetBrains Mono'].map((n) => ctx.addSystemFont(n)).find(Boolean);
  if (!id) return; // no ligature font here: the Rust test pins the cache key
  const glyphs = (features) => {
    ctx.frame(300, 100, 1, box({}, [text('-> != www', { size: 24, font: id, features })]));
    return decodeQuads(ctx.quads()).filter((q) => q.kind === 1 || q.kind === 4).length;
  };
  assert.ok(glyphs('liga=0 calt=0 dlig=0') > glyphs(undefined), 'the ligatures come apart');
});

// IME for an editor the app owns (backlog C17): a composition and its
// commit reach the focused sink as data, the candidate window is anchored
// at the `line` carrying `caret`, and plain typing is never doubled.
test('a composition and its commit reach the focused sink, anchored at its caret', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono' };
  const editor = (caret) =>
    box({ onKey: { kind: 'ed' }, role: 'multilineTextInput', label: 'Buffer' }, [
      box({ dir: 'row', role: 'line' }, [text('first', mono)], 'l0'),
      box({ dir: 'row', role: 'line', caret }, [text('let ', mono), text('value', mono)], 'l1'),
    ], 'editor');
  // Inside a root box: the root's own key is dropped, so a label on the
  // top-level element would name nothing.
  const view = (caret) => box({}, [editor(caret)]);
  ctx.frame(400, 100, 1, view(6));
  ctx.focus('editor');
  ctx.frame(400, 100, 1, view(6));
  ctx.pollEvents();
  ctx.preedit('日本', [0, 6]);
  ctx.commit('日本語');
  ctx.preedit('');
  assert.deepEqual(
    ctx.pollEvents().map((e) => e.payload),
    [
      { kind: 'preedit', text: '日本', cursor: [0, 6], tag: { kind: 'ed' } },
      { kind: 'text', text: '日本語', tag: { kind: 'ed' } },
      { kind: 'preedit', text: '', cursor: null, tag: { kind: 'ed' } },
    ],
    'the sink hears both halves, tagged',
  );
  // The candidate window sits at the caret: byte 6 of "let value" on line 1.
  const anchor = ctx.imeRect();
  const w = ctx.measureText('M', mono).width;
  assert.ok(Math.abs(anchor.x - 6 * w) < 0.75, JSON.stringify(anchor));
  assert.deepEqual(anchor, ctx.caretRect('l1', 6));
  // Typing is one event, not two: the key carries its text, and the text
  // channel a driver sends beside the press stays away from sinks.
  ctx.press('a');
  const typed = ctx.pollEvents().map((e) => e.payload);
  assert.equal(typed.length, 1, JSON.stringify(typed));
  assert.equal(typed[0].kind, 'key');
  assert.equal(typed[0].text, 'a');
  // Nothing focused, nothing to anchor.
  ctx.blur();
  ctx.frame(400, 100, 1, view(6));
  assert.equal(ctx.imeRect(), null);
});

// A custom editor's caret blinks (backlog C35): the `caret` row on a line
// under the focused sink is a caret to blink, the phase the driver sets
// is what the view reads, and the `caret` row stays declared through the
// off phase so the clock stays armed.
// The view reads the frame clock (backlog F134): `now()` is the seconds
// `setTime` set, 0 before it, so a deadline kept on it moves with the clock.
test('now() reads the frame clock setTime set', () => {
  const ctx = new Ctx();
  assert.equal(ctx.now(), 0, 'no clock yet');
  ctx.setTime(4.5);
  assert.equal(ctx.now(), 4.5);
  const view = () => box({}, ctx.now() < 5 ? [box({ width: 4, height: 4 }, [], 'toast')] : []);
  ctx.setInspect(true);
  ctx.frame(100, 100, 1, view());
  assert.ok(ctx.nodes().some((n) => n.label === 'toast'), 'before the deadline');
  ctx.setTime(5.5);
  ctx.frame(100, 100, 1, view());
  assert.ok(!ctx.nodes().some((n) => n.label === 'toast'), 'after it');
});

// A frame at a time (backlog F135): a deadline the driver sleeps to, with
// nothing owed until then; a past time is a frame now.
test('requestFrameAt sets a deadline and owes nothing until it', () => {
  const ctx = new Ctx();
  ctx.setTime(1);
  ctx.frame(100, 100, 1, box({}));
  assert.equal(ctx.nextFrameAt(), null, 'nothing asked for');
  ctx.requestFrameAt(4);
  assert.equal(ctx.nextFrameAt(), 4);
  assert.equal(ctx.animating(), false);
  ctx.setTime(4);
  ctx.frame(100, 100, 1, box({}));
  assert.equal(ctx.nextFrameAt(), null, 'the frame at the time spends it');
});

// The runner's decoder (backlog F138): a 2x2 GIF, red for 50 ms and then
// blue for 100, played once — decoded, registered and played on the frame
// clock with no image package.
const TINY_GIF = Buffer.from(
  'R0lGODlhAgACAIAAAAAAAAAAACH/C05FVFNDQVBFMi4wAwEBAAAh+QQIBQAAACwAAAAAAgACAID/AAAAAAACAoRRACH5BAgKAAAALAAAAAACAAIAgAAA/wAAAAIChFEAOw==',
  'base64',
);

test('decodeImage and decodeAnimation read a GIF, and animationAt plays it', () => {
  const still = decodeImage(TINY_GIF);
  assert.equal(still.width, 2);
  assert.equal(still.height, 2);
  assert.deepEqual([...still.rgba.subarray(0, 4)], [255, 0, 0, 255], 'the first frame');
  const gif = decodeAnimation(TINY_GIF);
  assert.equal(gif.frames.length, 2);
  assert.deepEqual(gif.delays, [0.05, 0.1]);
  assert.equal(gif.loops, 1);
  assert.deepEqual([...gif.frames[1].subarray(0, 4)], [0, 0, 255, 255]);
  assert.deepEqual(animationAt(gif.delays, gif.loops, 0.02), { index: 0, next: 0.05 });
  assert.equal(animationAt(gif.delays, gif.loops, 0.07).index, 1);
  const over = animationAt(gif.delays, gif.loops, 5);
  assert.equal(over.index, 1);
  assert.equal(over.next, Infinity, 'played out');
  assert.deepEqual(animationAt(gif.delays, null, 0.16), { index: 0, next: 0.2 }, 'for ever');
  assert.deepEqual(animationAt(gif.delays, 0, 0.16), { index: 0, next: 0.2 }, '0 is for ever, as in C');

  const ctx = new Ctx();
  const id = ctx.addImage(gif.width, gif.height, gif.frames[0]);
  assert.match(id, /^[0-9a-f]{16}$/);
  assert.throws(() => decodeImage(Buffer.from('not an image')), /decodeImage: not an image/);
});

// A lookup by accessible name (backlog F137): the `label` prop or a
// button's text, where keyOf reads the key label.
test('keyNamed finds a node by the name a reader hears', () => {
  const ctx = new Ctx();
  const view = (deletes) => box({ dir: 'row' }, [
    box({ width: 40, height: 40, onClick: { kind: 'like' }, label: 'Like' }, [], 'like'),
    ...Array.from({ length: deletes }, (_, i) =>
      box({ onClick: { kind: 'delete' } }, [text('Delete', { size: 14 })], `delete-${i}`)),
  ]);
  ctx.frame(400, 300, 1, view(1));
  assert.equal(ctx.keyNamed('Like'), ctx.keyOf('like'));
  assert.equal(ctx.keyNamed('like'), null, 'the key label is not the name');
  assert.equal(ctx.keyNamed('Delete'), ctx.keyOf('delete-0'), 'a button by its text');
  ctx.warnings();
  ctx.frame(400, 300, 1, view(2));
  assert.equal(ctx.keyNamed('Delete'), ctx.keyOf('delete-0'), 'the first of two');
  assert.deepEqual(ctx.warnings().map((w) => w.code), ['ambiguous-name']);
});

// The exit named at the removal (backlog F136): a card thrown right, over
// the fade it declared, removed in the same handler that aims it.
test('exitWith aims the exit a node leaves by in the frame that drops it', () => {
  const ctx = new Ctx();
  const card = () => box({
    width: 40, height: 40, bg: '#ffffff',
    transition: 1000, easing: 'linear', exit: { opacity: 0 },
  }, [], 'card');
  ctx.setTime(0);
  ctx.frame(600, 200, 1, box({ pad: 20 }, [card()]));
  ctx.exitWith('card', { dx: 400, opacity: 0 });
  ctx.frame(600, 200, 1, box({ pad: 20 }));
  ctx.setTime(0.5);
  ctx.frame(600, 200, 1, box({ pad: 20 }));
  const xs = decodeQuads(ctx.quads()).filter((q) => q.kind === 0).map((q) => q.x);
  assert.ok(xs.some((x) => Math.abs(x - 220) < 1), `halfway along the throw: ${xs}`);
  assert.throws(() => ctx.exitWith('card', { dx: 'far' }), /exitWith/);
});

test('a sink with a caret line reads the blink phase and draws its caret on it', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const view = () =>
    box({}, [
      box({ onKey: { kind: 'ed' }, keyFocus: true, role: 'multilineTextInput', label: 'Buffer' }, [
        box({ dir: 'row', role: 'line', caret: 3 }, [
          text('let ', mono),
          ...(ctx.caretVisible() ? [box({ width: 2, height: 16 }, [], 'caret')] : []),
          text('value', mono),
        ], 'l0'),
      ], 'editor'),
    ]);
  ctx.setInspect(true);
  ctx.frame(400, 100, 1, view());
  assert.ok(ctx.caretVisible(), 'solid until a driver says otherwise');
  assert.ok(ctx.nodes().some((n) => n.label === 'caret'), 'the on phase draws the caret');
  ctx.setCaretVisible(false);
  assert.ok(!ctx.caretVisible());
  ctx.frame(400, 100, 1, view());
  assert.ok(!ctx.nodes().some((n) => n.label === 'caret'), 'the off phase draws none');
  assert.equal(ctx.imeRect() !== null, true, 'and the caret row is still declared: the IME anchor and the clock keep it');
  ctx.setCaretVisible(true);
});

// A custom editor's solid caret (backlog F68): `caretSolid` beside `caret`
// on the line is a block caret in normal mode — still the IME's anchor and
// the access tree's caret, but not a caret to blink, so an app idling in
// normal mode draws no frame for it. The plain `caret` row blinks again.
test('a caret line declaring caretSolid anchors and reads but arms no blink clock', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const view = (solid) =>
    box({}, [
      box({ onKey: { kind: 'ed' }, keyFocus: true, role: 'multilineTextInput', label: 'Buffer' }, [
        box({ dir: 'row', role: 'line', caret: 3, ...(solid ? { caretSolid: true } : {}) }, [
          text('let value', mono),
        ], 'l0'),
      ], 'editor'),
    ]);
  ctx.setInspect(true);
  ctx.frame(400, 100, 1, view(false));
  assert.ok(ctx.hasCaret(), 'a plain caret row is a caret to blink');
  const anchor = ctx.imeRect();
  assert.ok(anchor !== null);
  ctx.frame(400, 100, 1, view(true));
  assert.ok(!ctx.hasCaret(), 'a solid caret is not a caret to blink');
  assert.deepEqual(ctx.imeRect(), anchor, 'and still the IME anchor');
  const editor = ctx.accessTree().nodes.find((n) => n.name === 'Buffer');
  assert.equal(editor.caret, 3, 'and still where assistive technology hears the caret');
  assert.deepEqual(ctx.warnings(), []);
  ctx.frame(400, 100, 1, view(false));
  assert.ok(ctx.hasCaret());
});

// A custom editor's mouse and clipboard (backlog C34, C33): a press or
// drag inside an `onKey` sink carries `line`, `byte` and `clicks` the way
// a grid's carries `cell`, and the sink's own Ctrl-c / Ctrl-v bind to
// `setClipboard` / `requestPaste`, the paste coming back as `text`.
test('a press in a sink names the line, the byte and the click count, and the sink has a clipboard', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const view = box({ pad: 10 }, [
    box({ onKey: { kind: 'ed' }, onDrag: 'sel', onClick: { kind: 'hit' }, role: 'multilineTextInput', label: 'Buffer', dir: 'row', width: 300, height: 60 }, [
      box({ width: 30, role: 'none' }, [
        box({ dir: 'row', height: 20 }, [text('1', mono)]),
        box({ dir: 'row', height: 20 }, [text('2', mono)]),
      ]),
      box({ width: 'grow' }, [
        box({ dir: 'row', height: 20, role: 'line' }, [text('hello ', mono), text('world', mono)]),
        box({ dir: 'row', height: 20, role: 'line' }, [text('second', mono)]),
      ]),
    ], 'editor'),
  ]);
  ctx.frame(400, 200, 1, view);
  ctx.focus('editor');
  ctx.frame(400, 200, 1, view);
  ctx.pollEvents();
  const w = ctx.measureText('M', mono).width;
  // A double click on the second line, after its third glyph: the drag
  // starts there and the click follows, both carrying the count.
  ctx.cursor(10 + 30 + 3.3 * w, 10 + 20 + 5);
  ctx.mouse(true, 2);
  ctx.mouse(false);
  const evs = ctx.pollEvents().map((e) => e.payload);
  const start = evs.find((p) => p.kind === 'drag' && p.phase === 'start');
  assert.equal(start.line, 1, JSON.stringify(start));
  assert.equal(start.byte, 3);
  assert.equal(start.clicks, 2);
  const hit = evs.find((p) => p.kind === 'hit');
  assert.deepEqual(hit, { kind: 'hit', line: 1, byte: 3, inside: true, clicks: 2 });
  // The clipboard: the two doors queue what a menu's Copy and Paste
  // would, and the host answers a paste with `commit`, which the sink
  // hears as `text`.
  ctx.setClipboard('yanked', null);
  assert.equal(ctx.awaitingPaste(), false);
  ctx.requestPaste();
  ctx.requestPaste(); // one ask at a time (AR34): dropped
  assert.equal(ctx.awaitingPaste(), true);
  assert.deepEqual(ctx.takeMenuActions(), [
    { kind: 'setClipboard', text: 'yanked', html: null },
    { kind: 'paste' },
  ]);
  ctx.commit('from the clipboard');
  assert.equal(ctx.awaitingPaste(), false, 'the commit is the answer');
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [
    // `pasted`: the answer to the ask, not an IME's commit (backlog DX14).
    { kind: 'text', text: 'from the clipboard', pasted: true, tag: { kind: 'ed' } },
  ]);
  // A secret goes out as its own kind, for the window to write marked
  // concealed and transient; a paste the pasteboard marked comes back
  // with its markers, only those set (backlog F84).
  ctx.setClipboardSecret('hunter2');
  ctx.requestPaste();
  assert.deepEqual(ctx.takeMenuActions(), [
    { kind: 'setClipboardSecret', text: 'hunter2' },
    { kind: 'paste' },
  ]);
  ctx.paste('s3cret', { concealed: true, transient: true });
  assert.equal(ctx.awaitingPaste(), false, 'a paste is the answer too');
  ctx.paste('brief', { transient: true });
  ctx.paste('plain');
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [
    { kind: 'text', text: 's3cret', pasted: true, concealed: true, transient: true, tag: { kind: 'ed' } },
    { kind: 'text', text: 'brief', pasted: true, transient: true, tag: { kind: 'ed' } },
    { kind: 'text', text: 'plain', pasted: true, tag: { kind: 'ed' } },
  ]);
});

// Context menus (ADR 0017, decision 5): a list of items and a point, not
// a node — the core holds the open one and draws it, so a view asks with
// a call and hears what was chosen as an event on the node it named.
test('openMenu draws a menu whose chosen row posts on the target', () => {
  const ctx = new Ctx();
  const view = () => box({}, [box({ selectable: true }, [text('one', { size: 14 })], 'card')]);
  ctx.frame(320, 240, 1, view());
  assert.equal(ctx.selectionText(), null);
  assert.equal(
    ctx.openMenu('card', 40, 30, [
      { role: 'copy' },
      { role: 'separator' },
      { label: 'Inspect', id: { do: 'inspect' } },
    ]),
    true,
  );
  ctx.frame(320, 240, 1, view());
  // The rows are in the access tree as menuItems under a menu.
  const tree = ctx.accessTree();
  const rows = tree.nodes.filter((n) => n.role === 'menuItem');
  assert.deepEqual(rows.map((r) => r.name), ['Copy', 'Inspect']);
  const inspect = rows[1].rect;
  ctx.cursor(inspect.x + inspect.w / 2, inspect.y + inspect.h / 2);
  ctx.mouse(true, 1);
  ctx.mouse(false);
  const events = ctx.pollEvents();
  assert.equal(events.length, 1, JSON.stringify(events));
  const p = events[0].payload;
  assert.equal(p.kind, 'menu');
  assert.equal(p.role, 'custom');
  assert.deepEqual(p.item, { do: 'inspect' });
  assert.equal(ctx.closeMenu(), false, 'choosing closed it already');
});

// A row that is its chord plays it where the keyboard was and posts no
// `menu` event: the shell around the field hears it (backlog F151).
test('a replay row plays its chord to the sink instead of posting', () => {
  const ctx = new Ctx();
  const view = () => box({ onKey: 'shell' }, [el('edit', { initial: 'ab', autofocus: true, label: 'Field', width: 200 }, [], 'field')]);
  ctx.frame(320, 240, 1, view());
  ctx.frame(320, 240, 1, view());
  ctx.pollEvents();
  ctx.openMenu('field', 20, 60, [{ label: 'New Note', accel: 'ctrl+n', replay: true }]);
  ctx.frame(320, 240, 1, view());
  const row = ctx.accessTree().nodes.find((n) => n.role === 'menuItem').rect;
  ctx.cursor(row.x + row.w / 2, row.y + row.h / 2);
  ctx.mouse(true, 1);
  ctx.mouse(false);
  const events = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual(
    events.map((p) => [p.kind, p.code, p.tag]),
    [['key', 'n', 'shell']],
    JSON.stringify(events),
  );
});

// A host that shows menus itself reads a row the way it reads a bar's:
// the drawn text, the accelerator the drawn menu would show (the role's
// where the row declared none), `enabled` and `checked` both present. One
// emitter for both readers, so a context menu's Copy does not read as a
// row with no shortcut while the bar's reads `⌘C`.
test('menu() reads a row the way menuBar() does', () => {
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, box({}, [box({ selectable: true }, [text('one', { size: 14 })], 'card')]));
  ctx.setNativeMenus(true);
  ctx.openMenu('card', 40, 30, [
    { role: 'copy' },
    { label: 'Wrap', checked: true, accel: '⌥Z' },
    { label: 'Gone', enabled: false },
  ]);
  const menu = ctx.menu();
  assert.deepEqual(
    [menu.target, menu.x, menu.y],
    [ctx.keyOf('card'), 40, 30],
  );
  // The role's default accelerator is the platform's: the glyph on
  // macOS, `Ctrl+C` on CI's Linux runner.
  const copyAccel = process.platform === 'darwin' ? '⌘C' : 'Ctrl+C';
  // A declared one kui can parse reads in the platform's spelling too, as
  // the drawn menu shows it (backlog F127).
  const wrapAccel = process.platform === 'darwin' ? '⌥Z' : 'Alt+Z';
  assert.deepEqual(menu.items, [
    { label: 'Copy', role: 'copy', enabled: true, checked: false, accel: copyAccel },
    { label: 'Wrap', role: 'custom', enabled: true, checked: true, accel: wrapAccel },
    { label: 'Gone', role: 'custom', enabled: false, checked: false, accel: null },
  ]);
  ctx.closeMenu();
  assert.equal(ctx.menu(), null);
  // Drawn by the core instead, the checked row is a checked item in the
  // tree, and every node says how live it is — a field the addon always
  // wrote and the type never declared.
  ctx.setNativeMenus(false);
  ctx.openMenu('card', 40, 30, [{ label: 'Wrap', checked: true }]);
  const view = box({}, [box({ selectable: true, live: 'polite' }, [text('one', { size: 14 })], 'card')]);
  ctx.frame(320, 240, 1, view);
  const nodes = ctx.accessTree().nodes;
  const wrap = nodes.find((n) => n.name === 'Wrap');
  assert.equal(wrap.checked, true);
  assert.deepEqual(
    nodes.map((n) => n.live).filter((l) => l !== 'off'),
    ['polite'],
  );
});

// A row's `items` are its submenu (backlog F128): `menu()` reads them
// back under the row, and a host that shows menus itself reports a row
// inside by its path — never the row that opens it.
test('a row with items is a submenu, and activateMenuPath reaches inside it', () => {
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, box({}, [box({ selectable: true }, [text('one', { size: 14 })], 'card')]));
  ctx.setNativeMenus(true);
  ctx.openMenu('card', 40, 30, [
    { label: 'Open', id: 'open' },
    { label: 'Move to', items: [{ label: 'Inbox', id: 'inbox' }, { label: 'Archive', id: 'archive' }] },
  ]);
  const move = ctx.menu().items[1];
  assert.deepEqual(move.items.map((r) => r.label), ['Inbox', 'Archive']);
  assert.equal(ctx.menu().items[0].items, undefined, 'a plain row reads as it did');
  assert.equal(ctx.activateMenuItem(1), false, 'the row that opens it is never chosen');
  assert.equal(ctx.activateMenuPath([1]), false);
  ctx.pollEvents();
  assert.equal(ctx.activateMenuPath([1, 1]), true);
  const events = ctx.pollEvents();
  assert.equal(events.length, 1);
  assert.equal(events[0].payload.item, 'archive');
  assert.equal(ctx.menu(), null);
});

// A key no row reads warns inside a submenu too — `disabled` there was
// silently an enabled row — in openMenu's rows and a menuBar's; and a
// <select> option is chosen, never opened, so its `items` warn and are
// dropped (backlog RG150).
test('a stray key inside a submenu warns, and a select option takes no items', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  const card = () => box({}, [box({ selectable: true }, [text('one', { size: 14 })], 'card')]);
  ctx.frame(320, 240, 1, card());
  ctx.openMenu('card', 40, 30, [{ label: 'Move to', items: [{ label: 'Inbox', disabled: true }] }]);
  ctx.frame(320, 240, 1, card());
  let warned = ctx.warnings();
  assert.equal(warned.length, 1, JSON.stringify(warned));
  assert.match(warned[0].message, /`disabled` is not a key of a menu item.*did you mean `enabled: false`/);
  ctx.closeMenu();

  ctx.frame(320, 240, 1, box({}, [el('menuBar', { menu: [{ label: 'View', items: [{ label: 'Sort by', items: [{ label: 'Name', Label: 'x' }] }] }] })]));
  warned = ctx.warnings();
  assert.equal(warned.length, 1, JSON.stringify(warned));
  assert.match(warned[0].message, /`Label` is not a key of a menu item/);

  const select = box({ pad: 10 }, [el('select', { label: 'sort', options: ['Name', { label: 'Date', items: [{ label: 'Newest' }] }] })]);
  ctx.frame(320, 240, 1, select);
  warned = ctx.warnings();
  assert.equal(warned.length, 1, JSON.stringify(warned));
  assert.match(warned[0].message, /`items` on a select's option is dropped: an option is chosen, never opened/);
  ctx.access('sort', 'click');
  assert.equal(ctx.menu().items[1].items, undefined, 'the option opens nothing');
});

test('openMenu refuses an item it cannot read', () => {
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, box({}, [box({ selectable: true }, [text('one', { size: 14 })], 'card')]));
  assert.throws(() => ctx.openMenu('card', 0, 0, [{ role: 'frobnicate' }]), /frobnicate/);
  assert.throws(() => ctx.openMenu('card', 0, 0, [{}]), /needs a label/);
});

// A `selectable` container makes the text under it one selection
// (ADR 0017): three labels select as three lines of one text, and a run
// the frame built but never drew is part of it.
test('selectable scopes one selection over the runs inside them', () => {
  const ctx = new Ctx();
  const style = { size: 14 };
  const card = box({ selectable: true, dir: 'column' }, [
    text('one', style),
    text('two', style),
    text('three', style),
  ], 'card');
  ctx.frame(400, 200, 1, box({}, [card, box({ width: 10, height: 10 }, [], 'plain')]));
  assert.equal(ctx.selectionText(), null, 'nothing is selected until something selects it');
  assert.equal(ctx.selectAllIn('card'), true);
  assert.equal(ctx.selectionText(), 'one\ntwo\nthree');
  assert.equal(ctx.selectAllIn('plain'), false, 'a node that drew no text is not a scope');
  assert.equal(ctx.clearSelection(), true);
  assert.equal(ctx.selectionText(), null);
});

test('a selection reaches the runs a scroller clipped away', () => {
  const ctx = new Ctx();
  const style = { size: 14 };
  const rows = [];
  for (let i = 0; i < 6; i++) rows.push(text(`row ${i}`, style));
  const list = box({ selectable: true, dir: 'column', height: 40, scrollY: true }, rows, 'list');
  ctx.frame(400, 200, 1, box({}, [list]));
  assert.equal(ctx.selectAllIn('list'), true);
  assert.equal(
    ctx.selectionText(),
    'row 0\nrow 1\nrow 2\nrow 3\nrow 4\nrow 5',
    'tier 2: what was built but not drawn still copies',
  );
});

// A held drag follows its scroller, and Shift extends (ADR 0029, backlog
// C39): the live end is placed again when the rows move under a still
// pointer, a press held past the edge scrolls the list a frame at a time,
// and a Shift-press keeps the anchor — read back through `selectionEnds`.
test('a drag held past the edge scrolls, follows, and Shift extends from the anchor', () => {
  const ctx = new Ctx();
  const style = { size: 14 };
  const rows = [];
  for (let i = 0; i < 10; i++) rows.push(box({ height: 20, width: 'grow' }, [text(`row ${i}`, style)], `r${i}`));
  const list = box({ selectable: true, dir: 'column', height: 60, width: 'grow', scrollY: true }, rows, 'list');
  const tree = box({ width: 'grow', height: 'grow' }, [list]);
  ctx.frame(400, 300, 1, tree);
  ctx.cursor(1, 6); ctx.mouse(true); ctx.cursor(200, 26);
  assert.equal(ctx.selectionText(), 'row 0\nrow 1');
  // Two rows scroll under the still pointer: the text moves on the next
  // frame, the highlight follows a frame later.
  ctx.scroll(0, -40);
  ctx.frame(400, 300, 1, tree);
  ctx.frame(400, 300, 1, tree);
  assert.equal(ctx.selectionText(), 'row 0\nrow 1\nrow 2\nrow 3');
  // Held 60 px below the list: 600 px/s, ten px a clockless frame.
  ctx.cursor(200, 120);
  for (let i = 0; i < 6; i++) ctx.frame(400, 300, 1, tree);
  assert.ok(ctx.animating(), 'a held pointer past the edge asks for frames');
  assert.equal(ctx.scrollOffset('list').y, 100);
  ctx.mouse(false);
  ctx.frame(400, 300, 1, tree);
  assert.ok(!ctx.animating());
  assert.ok(ctx.selectionText().endsWith('row 7'), ctx.selectionText());
  // Shift-click one character into row 7 (the last on screen) keeps the
  // anchor on row 0.
  ctx.modifiers({ shift: true });
  ctx.cursor(5, 50); ctx.mouse(true); ctx.mouse(false);
  ctx.modifiers({});
  assert.equal(ctx.selectionText(), 'row 0\nrow 1\nrow 2\nrow 3\nrow 4\nrow 5\nrow 6\nr');
  const ends = ctx.selectionEnds();
  assert.deepEqual(ends, { anchor: { index: null, byte: 0 }, focus: { index: null, byte: 1 } });
  assert.equal(ctx.clearSelection(), true);
  assert.equal(ctx.selectionEnds(), null);
});

// An `onScroll` that names modifiers hears a wheel turned with one of them
// held, ahead of the scroller under the pointer, and no other (backlog
// F122).
test('scrollMods makes onScroll the modified wheel\'s, ahead of a scroller inside it', () => {
  const ctx = new Ctx();
  const tree = box({ width: 'grow', height: 'grow', onScroll: { kind: 'zoom' }, scrollMods: 'ctrl super' }, [
    box({ width: 'grow', height: 200, scrollY: true }, [box({ width: 'grow', height: 900 }, [], 'tall')], 'list'),
  ]);
  ctx.frame(400, 200, 1, tree);
  ctx.cursor(50, 20);
  ctx.scroll(0, -40);
  assert.deepEqual(ctx.pollEvents(), [], 'nothing held: the list\'s');
  ctx.frame(400, 200, 1, tree);
  assert.equal(ctx.scrollOffset('list').y, 40);
  ctx.modifiers({ ctrl: true });
  ctx.pollEvents();
  ctx.scroll(0, 40);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [{
    kind: 'scroll', x: 50, y: 20, dx: 0, dy: 40, lines: null,
    mods: { shift: false, ctrl: true, alt: false, super: false }, tag: { kind: 'zoom' },
  }]);
  ctx.frame(400, 200, 1, tree);
  assert.equal(ctx.scrollOffset('list').y, 40, 'the list stood still');
});

// An `onScroll` node hears the wheel instead of scrolling: pixels on any
// node, whole lines on a `cells` grid with the fraction carried, and it
// takes the notch from the scroller above it (ADR 0029, decision 4).
test('onScroll takes the wheel as a message, in lines on a cells grid', () => {
  const ctx = new Ctx();
  const screen = new Uint32Array(3 * 11 * 4);
  for (let i = 0; i < 33; i++) { screen[i * 4] = 32; screen[i * 4 + 1] = 0xd6d8e0ff; }
  const outer = box({ width: 'grow', height: 200, scrollY: true }, [
    box({ width: 'grow', height: 40, onScroll: { kind: 'zoom' } }, [], 'canvas'),
    el('cells', { rows: 3, cols: 11, cells: screen, size: 13, family: 'mono', lineHeight: 18, onScroll: { kind: 'term' } }, [], 'term'),
    box({ width: 'grow', height: 400 }, [], 'filler'),
  ], 'outer');
  const tree = box({ width: 'grow', height: 'grow' }, [outer]);
  ctx.frame(400, 200, 1, tree);
  ctx.cursor(50, 20);
  ctx.scroll(3, -12);
  let evs = ctx.pollEvents().map((e) => e.payload);
  assert.deepEqual(evs, [{ kind: 'scroll', x: 50, y: 20, dx: 3, dy: -12, lines: null, tag: { kind: 'zoom' } }]);
  ctx.frame(400, 200, 1, tree);
  assert.equal(ctx.scrollOffset('outer').y, 0, 'the canvas took the notch from the scroller above it');
  // Two and a half rows down on the grid: two lines, a half carried; then
  // the half made whole.
  ctx.cursor(50, 60);
  ctx.scroll(0, -45);
  evs = ctx.pollEvents().map((e) => e.payload);
  assert.equal(evs.length, 1);
  assert.equal(evs[0].lines, 2);
  assert.deepEqual(evs[0].tag, { kind: 'term' });
  ctx.scroll(0, -9);
  assert.equal(ctx.pollEvents()[0].payload.lines, 1);
  ctx.scroll(0, 36);
  assert.equal(ctx.pollEvents()[0].payload.lines, -2, 'earlier history is negative');
  // Below both: the scroller scrolls, nothing is heard.
  ctx.cursor(50, 150);
  ctx.scroll(0, -12);
  assert.deepEqual(ctx.pollEvents(), []);
  ctx.frame(400, 200, 1, tree);
  assert.equal(ctx.scrollOffset('outer').y, 12);
});

// A scroll gesture keeps the target it began on (backlog F107, ADR 0038):
// `scrollGesture` is the headless door to latching, as C's
// `kui_input_scroll_gesture` is. A sideways swipe begun over the list
// moves the strip on when the terminal comes under the still pointer.
test('scrollGesture latches: a swipe that moved the strip keeps it past a terminal', () => {
  const ctx = new Ctx();
  const strip = box({ dir: 'row', width: 300, height: 200, scrollX: true }, [
    box({ width: 150, height: 200, scrollY: true }, [box({ width: 150, height: 800 })], 'list'),
    box({ width: 150, height: 200, onScroll: { kind: 'term' } }, [], 'term'),
    box({ width: 600, height: 200 }),
  ], 'strip');
  const tree = box({ width: 'grow', height: 'grow' }, [strip]);
  ctx.frame(300, 200, 1, tree);
  ctx.frame(300, 200, 1, tree);
  ctx.cursor(130, 50);
  ctx.scrollGesture(-30, 0, true);
  ctx.frame(300, 200, 1, tree);
  for (let i = 0; i < 3; i++) {
    ctx.scrollGesture(-30, 0, false);
    assert.deepEqual(ctx.pollEvents(), [], 'latched: not the terminal\'s');
    ctx.frame(300, 200, 1, tree);
  }
  assert.equal(ctx.scrollOffset('strip').x, 120);
  // A new gesture over the terminal is the terminal's.
  ctx.scrollGesture(-30, 0, true);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload.tag), [{ kind: 'term' }]);
});

// A point on the text a keyed node drew is a byte offset, and a byte
// offset is a caret rect (backlog C18): the `line` row of a custom editor
// answers across its token runs, so a click becomes a caret with one call.
test('textHit and caretRect answer across the runs of a keyed line', () => {
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono' };
  const line = box(
    { dir: 'row' },
    [text('let ', mono), box({ bg: '#3b5bd455' }, [text('value', mono)]), text(' = 1;', mono)],
    'line',
  );
  ctx.frame(400, 100, 1, box({}, [line, box({ width: 10, height: 10 }, [], 'plain')]));
  const w = ctx.measureText('M', mono).width;
  // "let value = 1;" — a point in the fourth cell of "value" is byte 7.
  assert.deepEqual(ctx.textHit('line', 7.2 * w, 5), { byte: 7, line: 0 });
  const seam = ctx.caretRect('line', 4);
  assert.ok(Math.abs(seam.x - 4 * w) < 0.75, JSON.stringify(seam));
  assert.equal(seam.w, 0);
  assert.ok(seam.h > 0);
  assert.equal(ctx.textHit('line', 390, 5).byte, 14, 'far right is the end, across the runs');
  assert.equal(ctx.textHit('line', 0, 5).byte, 0);
  assert.equal(ctx.caretRect('line', 999).x, ctx.caretRect('line', 14).x, 'past the end is the end');
  assert.equal(ctx.caretRect('plain', 0), null, 'a node that drew no text');
});

// Layout is data: an onLayout node reports its rect on first sight and
// again when it changes.
test('onLayout reports the rect once and again when it changes', () => {
  const tree = (w) =>
    box({ dir: 'row', width: 'grow', height: 'grow' }, [
      box({ width: w, height: 'grow', onLayout: { kind: 'panel' } }, [], 'panel'),
    ]);
  const { ctx } = run(() => tree(100));
  const evs = ctx.pollEvents().filter((e) => e.payload.kind === 'layout');
  assert.equal(evs.length, 1, 'one layout event');
  assert.deepEqual(evs[0].payload, {
    kind: 'layout',
    x: 0, y: 0, w: 100, h: 240,
    parent: { x: 0, y: 0, w: 320, h: 240 },
    // Physical px per logical px at the node: the frame's, until a zoom
    // composes into it (ADR 0025, decision 5).
    scale: 1,
    tag: { kind: 'panel' },
  });
  ctx.frame(320, 240, 1, tree(100));
  assert.equal(ctx.pollEvents().length, 0, 'same rect, silence');
  ctx.frame(320, 240, 1, tree(150));
  const again = ctx.pollEvents();
  assert.equal(again.length, 1);
  assert.equal(again[0].payload.w, 150);
  // The query shape of the same numbers (backlog C26 step 2): the rect
  // the last frame laid the node out at, with no event; null for a node
  // that declared no onLayout.
  assert.deepEqual(ctx.layoutOf('panel'), { x: 0, y: 0, w: 150, h: 240 });
  assert.equal(ctx.layoutOf('nowhere'), null);
});

// Silent misconfigurations come back as data, once each.
test('warnings report a lone weighted grow child once', () => {
  const tree = box({ dir: 'row', width: 'grow', height: 'grow' }, [
    box({ width: 50, height: 10 }),
    box({ width: { grow: 2 }, height: 10 }, [], 'wide'),
  ]);
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, tree);
  const ws = ctx.warnings();
  assert.equal(ws.length, 1);
  assert.equal(ws[0].code, 'grow-weight-ignored');
  assert.match(ws[0].message, /only grow child/);
  ctx.frame(320, 240, 1, tree);
  assert.equal(ctx.warnings().length, 0, 'once');
  const quiet = new Ctx();
  quiet.setDiagnostics(false);
  quiet.frame(320, 240, 1, tree);
  assert.equal(quiet.warnings().length, 0);
});

test('createApp collects warnings on the app', () => {
  const app = createApp(
    {
      init: 0,
      update: () => undefined,
      view: () => box({ dir: 'row' }, [box({ width: { grow: 2 }, height: 10 })]),
    },
    { warnings: false },
  );
  app.render();
  app.render();
  assert.equal(app.warnings.length, 1);
  assert.equal(app.warnings[0].code, 'grow-weight-ignored');
  // A production build runs no checks at all.
  const shipped = createApp(
    { init: 0, update: () => undefined, view: () => box({ dir: 'row' }, [box({ width: { grow: 2 }, height: 10 })]) },
    { diagnostics: false },
  );
  shipped.render();
  assert.equal(shipped.warnings.length, 0);
});

test('a ticking app runs headless: advance is the window timer, by hand', () => {
  const surface = new Ctx();
  const seen = [];
  const app = createApp(
    {
      // A countdown that redraws only when the displayed second changes:
      // the ticks in between mutate and return undefined.
      init: { ticks: 0, second: 0 },
      update: (m, msg, ev, ctx) => {
        seen.push([msg.kind, ctx]);
        const second = Math.floor(msg.now / 1000);
        if (second === m.second) {
          m.ticks += 1;
          return;
        }
        return { ticks: m.ticks + 1, second };
      },
      view: (m) => box({ pad: 4 }, [text(`${m.second}`)]),
      tick: { every: 250, msg: (now) => ({ kind: 'tick', now }) },
    },
    { surface, startTime: 0 },
  );
  // The surface is injected, and `update`'s fourth argument is it — the same
  // slot `runWindowed` fills with the window.
  assert.equal(app.surface, surface);
  assert.equal(app.ctx, surface);
  app.render();
  // Every tick inside the span fires: 250, 500, 750, 1000.
  app.advance(1000);
  assert.equal(app.model.ticks, 4);
  assert.equal(app.model.second, 1);
  assert.deepEqual(seen.map((s) => s[0]), ['tick', 'tick', 'tick', 'tick']);
  assert.equal(seen[0][1], surface);
  // A span shorter than the cadence owes nothing.
  app.advance(100);
  assert.equal(app.model.ticks, 4);
  app.advance(200);
  assert.equal(app.model.ticks, 5);
});

test('init and view are handed the surface, so a tree can be measured where it is built (F11)', () => {
  // Before this, the surface reached only `setup` and `update`: an app that
  // wanted its own measurements parked the surface in a module-level
  // variable from `setup` and built its first model against constants,
  // correcting them on the first `resize`. `init(surface)` and
  // `view(model, window, surface)` are the two arguments that removes.
  const surface = new Ctx();
  const LABEL = 'Wednesday';
  const STYLE = { size: 14 };
  const PAD = 6;
  let initSurface = null;
  let viewSurface = null;
  const app = createApp(
    {
      // A first model measured against the real surface, after `setup` —
      // so the font `setup` registered is the one measured with.
      init: (ui) => {
        initSurface = ui;
        return { column: Math.round(ui.measureText(LABEL, STYLE).width) };
      },
      update: () => undefined,
      // The column is as wide as its widest label, which is arithmetic the
      // view can only do with the surface in hand.
      view: (model, window, ui) => {
        viewSurface = ui;
        return box({ pad: 0 }, [
          box({ width: model.column + PAD * 2, padX: PAD, bg: '#3b5bd4' }, [text(LABEL, STYLE)], 'col'),
        ]);
      },
    },
    { surface, width: 320, height: 240 },
  );
  // Both arguments are the surface the loop drives — the slot `runWindowed`
  // fills with the `KuiWindow` whose `size()` a first model wants.
  assert.equal(initSurface, surface);
  app.render();
  assert.equal(viewSurface, surface);

  const measured = surface.measureText(LABEL, STYLE);
  assert.ok(measured.width > 0, 'the label measures to something');
  assert.equal(app.model.column, Math.round(measured.width));
  // What layout drew is what the view measured: the column is the label
  // plus its padding, at scale 1, where logical and physical px agree.
  const column = decodeQuads(app.ctx.quads()).find((q) => q.kind === 0 && q.w > 0 && q.h > 0);
  assert.equal(column.w, Math.round(measured.width) + PAD * 2);
  // And the glyphs inside it fit the box that was sized for them.
  const glyphs = decodeQuads(app.ctx.quads()).filter((q) => q.kind !== 0);
  assert.ok(glyphs.length > 0, 'the label drew');
  assert.ok(glyphs.every((q) => q.x >= column.x && q.x + q.w <= column.x + column.w), 'no overflow');
});

test('a plain init is still a value, and a view that ignores the surface still draws', () => {
  // Both arguments are additive: the old spellings are untouched.
  const app = createApp(
    { init: { n: 7 }, update: () => undefined, view: (m) => box({ pad: 4 }, [text(`${m.n}`)]) },
    { width: 320, height: 240 },
  );
  assert.deepEqual(app.model, { n: 7 });
  app.render();
  assert.ok(app.ctx.stats().quadCount > 0);
});

test('advance moves the frame clock, so a transition runs headless', () => {
  const app = createApp(
    {
      init: { wide: false },
      update: (m, msg) => (msg === 'go' ? { wide: true } : undefined),
      view: (m) =>
        box({ pad: 0 }, [
          box({ transition: 200, width: m.wide ? 200 : 20, height: 10, bg: '#ffffff' }, [], 'bar'),
        ]),
    },
    { startTime: 0, width: 320, height: 240 },
  );
  const barWidth = () => decodeQuads(app.ctx.quads()).find((q) => q.h === 10).w;
  app.advance(0);
  app.dispatch('go');
  app.advance(100);
  assert.ok(app.ctx.animating(), 'the tween is mid-flight');
  assert.equal(barWidth(), 20, 'it starts where the node was');
  app.advance(100);
  assert.ok(barWidth() > 20 && barWidth() < 200, 'halfway');
  app.advance(100);
  assert.equal(app.ctx.animating(), false);
  assert.equal(barWidth(), 200);
});

test('setTime under a loop throws and names advance; a bare Ctx keeps it (F16)', () => {
  // Both alpha.7 field reports drove `ctx.setTime` around `render()` under
  // `createApp` — the alpha.6 idiom — and got a bar pinned at its baseline
  // with `animating()` true forever: the loop stamps the clock before every
  // frame from hands only `advance` moves. Silent, and non-terminating for
  // a test that waits on `animating()`; so it is not silent any more.
  const config = {
    init: { wide: false },
    update: (m, msg) => (msg === 'go' ? { wide: true } : m),
    view: (m) =>
      box({ pad: 0 }, [
        box({ transition: 200, width: m.wide ? 200 : 20, height: 10, bg: '#ffffff' }, [], 'bar'),
      ]),
  };
  const app = createApp(config, { startTime: 0, width: 320, height: 240 });
  app.render();
  assert.throws(() => app.ctx.setTime(0.5), /advance/);
  // The loop's own stamping is untouched: its frames still ease.
  app.dispatch('go');
  app.render(); // the frame that applies the change takes the baseline
  app.advance(100);
  const barWidth = (c) => decodeQuads(c.quads()).find((q) => q.h === 10).w;
  assert.ok(barWidth(app.ctx) > 20 && barWidth(app.ctx) < 200, 'mid-flight under advance');
  // A surface handed in is the one taken over, not a copy of it.
  const own = new Ctx();
  createApp(config, { surface: own, startTime: 0, width: 320, height: 240 });
  assert.throws(() => own.setTime(1), /advance/);
  // A bare Ctx keeps the method and snaps without it, as documented.
  const bare = new Ctx();
  bare.setTime(0);
  bare.frame(320, 240, 1, config.view({ wide: false }));
  bare.setTime(0.1);
  bare.frame(320, 240, 1, config.view({ wide: true }));
  bare.setTime(0.2);
  bare.frame(320, 240, 1, config.view({ wide: true }));
  assert.ok(barWidth(bare) > 20 && barWidth(bare) < 200, 'a bare Ctx eases under its own clock');
});

test('runOut advances until nothing animates and says how long it took (F22)', () => {
  // Both alpha.7 reports captured a frame straight after a change and got
  // frame 0 of every transition — panels fully transparent, labels
  // mid-slide — then wrote the same `for (…animating()) advance(16)` loop.
  const app = createApp(
    {
      init: { wide: false },
      update: (m, msg) => (msg === 'go' ? { wide: true } : m),
      view: (m) =>
        box({ pad: 0 }, [
          box({ transition: 200, width: m.wide ? 200 : 20, height: 10, bg: '#ffffff' }, [], 'bar'),
        ]),
    },
    { startTime: 0, width: 320, height: 240 },
  );
  const barWidth = () => decodeQuads(app.ctx.quads()).find((q) => q.h === 10).w;
  app.render();
  assert.equal(app.runOut(), 0, 'nothing to run out on a still frame');
  app.dispatch('go');
  // No render in between: runOut draws the frame that applies the change
  // itself, so a stale `animating()` cannot make it return early.
  const ms = app.runOut();
  assert.equal(app.ctx.animating(), false);
  assert.equal(barWidth(), 200, 'the settled frame');
  assert.ok(ms >= 200 && ms < 400, `ran the 200 ms transition out in ${ms} ms`);
  // A cap: something that never settles hands control back with the truth.
  const looping = createApp(
    {
      init: {},
      view: () =>
        box({ pad: 0 }, [
          box({ width: 20, height: 10, bg: '#ffffff', transition: 100, keyframes: [{ opacity: 0 }, { opacity: 1 }] }, [], 'k'),
        ]),
    },
    { startTime: 0, width: 320, height: 240 },
  );
  assert.equal(looping.runOut(160), 160);
  assert.equal(looping.ctx.animating(), true);
  // What is owed, by kind (F64): the cycle alone, once its node's own
  // 100 ms entrance leg has run — `animating()` cannot say which.
  assert.deepEqual(looping.ctx.owed(), { transition: false, cycle: true, depart: false, requested: false, autoscroll: false, scroll: false });
});

test('owedBy names who holds an owed frame, frameCause says why a frame ran (F111)', () => {
  const app = createApp(
    {
      init: { wide: false },
      update: (m, msg) => (msg === 'go' ? { wide: true } : m),
      view: (m) =>
        box({ pad: 0 }, [
          box({ transition: 200, width: m.wide ? 200 : 20, height: 10, bg: '#ffffff' }, [], 'bar'),
        ]),
    },
    { startTime: 0, width: 320, height: 240 },
  );
  const empty = { transitions: [], cycles: [], departures: [], scrolls: [], autoscroll: null, animate: [], requests: [] };
  app.render();
  assert.deepEqual(app.ctx.owedBy(), empty, 'off by default');
  assert.equal(app.ctx.frameUnchanged(), null);
  app.ctx.setFrameTrace(true);
  app.dispatch('go');
  app.advance(16);
  app.advance(16);
  // Between frames: the one just built, and who held it.
  assert.ok(app.ctx.frameCause().includes('owed'), JSON.stringify(app.ctx.frameCause()));
  const by = app.ctx.owedBy();
  assert.deepEqual(by.transitions.map((h) => [h.name, h.slots]), [['bar', ['width']]]);
  assert.equal(by.transitions[0].key, app.ctx.keyOf('bar'));
  assert.equal(app.ctx.frameUnchanged(), false, 'the bar moved');
  app.runOut();
  app.advance(16);
  assert.deepEqual(app.ctx.frameCause(), []);
  assert.deepEqual(app.ctx.owedBy(), empty);
  app.advance(16);
  assert.equal(app.ctx.frameUnchanged(), true, 'nothing moved');
});

test('a view reads the frame it is for: frameCause and owedBy from inside createApp\'s view (RG81)', () => {
  // A view runs before its frame begins — it returns the tree the frame is
  // handed — so it read the frame before: `[]` on the key's frame, `key`
  // only after it.
  const seen = [];
  const app = createApp(
    {
      init: { wide: false },
      update: (m, msg) => (msg === 'go' ? { wide: true } : m),
      view: (m, _name, ctx) => {
        seen.push({ cause: ctx.frameCause(), held: ctx.owedBy().transitions.map((h) => h.name) });
        return box({ pad: 0 }, [box({ transition: 200, width: m.wide ? 200 : 20, height: 10, bg: '#ffffff' }, [], 'bar')]);
      },
    },
    { startTime: 0, width: 320, height: 240 },
  );
  app.render();
  app.ctx.setFrameTrace(true);
  app.ctx.keyDown('a');
  seen.length = 0;
  app.render();
  assert.deepEqual(seen, [{ cause: ['key'], held: [] }], 'the key\'s frame, read from its view');
  assert.deepEqual(app.ctx.frameCause(), ['key'], 'and between frames, the frame just built');
  app.dispatch('go');
  app.advance(16);
  seen.length = 0;
  app.advance(16);
  assert.deepEqual(seen.at(-1), { cause: ['owed'], held: ['bar'] }, 'a transition\'s frame names its holder to the view');
  assert.deepEqual(app.ctx.frameCause(), ['owed']);
});

// The windowed half of the same question, which cannot be a loop: a window
// runs on the wall clock, so a test has no `advance` to run a transition out
// with and the driver's pump is the only thing that can say a frame
// happened. These drive that pump by hand — `createApp` over a stand-in
// surface and a clock the test holds, which is what `runWindowed` fills with
// `Date.now` — and check what the waiters are answered with.
/** A stand-in for the window: it shows a view and polls; its `animating()`
 *  is the test's to flip. */
function fakeWindow(state) {
  return {
    setView() {},
    pollEvents: () => state.events.splice(0),
    warnings: () => [],
    setDiagnostics() {},
    stats: () => ({}),
    animating: () => state.animating,
  };
}
/** 'done' if `p` has settled by the end of this turn of the event loop,
 *  'pending' if it has not. */
const outcome = (p) =>
  Promise.race([
    p.then((v) => ['done', v]),
    new Promise((r) => setImmediate(() => r(['pending']))),
  ]);

// -- teardown (RG1) ----------------------------------------------------------
// The window calls the loop's `teardown` from inside the pump that saw it
// go, before that pump returns: on a Mac's ⌘Q that pump is the process's
// last, so nothing after `await runWindowed(...)` — not `process.on('exit')`
// either — ever runs. These drive the loop over a stand-in window whose
// `pump` does what the runner does: run the callback, then return false.

test('teardown runs once with the model, from the window inside the pump that saw it go, before runWindowed resolves', async () => {
  const state = { animating: false, events: [] };
  let onTeardown = null;
  const seen = [];
  let pumps = 0;
  const win = {
    ...fakeWindow(state),
    onTeardown: (cb) => {
      onTeardown = cb;
    },
    pump: () => {
      pumps += 1;
      if (pumps < 3) return true;
      // The runner's end: `App::teardown` runs inside this pump, and the
      // pump then says the window is gone.
      onTeardown();
      seen.push('pump-returned-false');
      return false;
    },
    nextDeadlineMs: () => null,
  };
  // A click before the end, so the model the callback gets is the one
  // `update` made and not `init`'s.
  state.events.push({ origin: 0, key: '', payload: 'bump' });
  const model = await runWindowed(
    {
      init: { n: 0 },
      update: (m, msg) => (msg === 'bump' ? { n: m.n + 1 } : m),
      view: () => box({ pad: 4 }),
      teardown: (m) => seen.push(['teardown', m]),
    },
    { surface: win, warnings: false },
  );
  assert.ok(onTeardown, 'the driver registered the loop with the window before the first pump');
  assert.deepEqual(model, { n: 1 });
  assert.deepEqual(seen, [['teardown', { n: 1 }], 'pump-returned-false'], 'once, with the model, and before the pump came back');
});

test('a stand-in window without the door still tears down once, at the pump that returned false', async () => {
  const state = { animating: false, events: [] };
  let pumps = 0;
  const win = {
    ...fakeWindow(state),
    pump: () => {
      pumps += 1;
      return pumps < 2;
    },
    nextDeadlineMs: () => null,
  };
  const seen = [];
  const model = await runWindowed(
    { init: 'm', update: (m) => m, view: () => box({ pad: 4 }), teardown: (m) => seen.push(m) },
    { surface: win, warnings: false },
  );
  assert.equal(model, 'm');
  assert.deepEqual(seen, ['m']);
});

test('a teardown that throws rejects runWindowed with the throw', async () => {
  const state = { animating: false, events: [] };
  const win = { ...fakeWindow(state), pump: () => false, nextDeadlineMs: () => null };
  await assert.rejects(
    runWindowed(
      {
        init: 0,
        update: (m) => m,
        view: () => box({ pad: 4 }),
        teardown: () => {
          throw new Error('could not save');
        },
      },
      { surface: win, warnings: false },
    ),
    /could not save/,
  );
});

test('headless, app.teardown() runs the config teardown once with the model, and a loop without one has nothing to run', () => {
  const seen = [];
  const app = createApp({
    init: { n: 0 },
    update: (m, msg) => (msg === 'bump' ? { n: m.n + 1 } : m),
    view: () => box({ pad: 4 }),
    teardown: (m) => seen.push(m),
  });
  app.dispatch('bump');
  app.teardown();
  app.dispatch('bump');
  app.teardown();
  assert.deepEqual(seen, [{ n: 1 }], 'once, with the model as it stood');
  const bare = createApp({ init: 0, update: (m) => m, view: () => box({ pad: 4 }) });
  bare.teardown();
});

test('settled() resolves from inside the pump, with the milliseconds it waited (F30)', async () => {
  const state = { animating: true, events: [] };
  let t = 0;
  const app = createApp(
    { init: 0, update: (m) => m, view: () => box({ pad: 4 }) },
    { surface: fakeWindow(state), clock: () => t },
  );
  app.render();
  const pump = (ms) => {
    t += ms;
    app.step();
  };
  const p = app.settled();
  // Three pumps of motion answer nothing: the promise is for the frame that
  // leaves nothing moving, not for the next one.
  for (let i = 0; i < 3; i++) pump(16);
  assert.deepEqual(await outcome(p), ['pending'], 'still animating');
  state.animating = false;
  pump(16);
  assert.deepEqual(await outcome(p), ['done', 64], 'the fourth pump settled it, 64 ms in');
  // Asked again on a still window it is the next pump, not this instant —
  // the answer always comes from a frame that really happened.
  const again = app.settled();
  assert.deepEqual(await outcome(again), ['pending']);
  pump(8);
  assert.equal(await again, 8);
});

test('quiet() resolves under a keyframe cycle, where settled() can only hit its cap (F64)', async () => {
  // The pomodoro's unpinned window has a `repeat="alternate"` cycle, so
  // its transitions running out is a fact `settled()` could never report.
  // `owed()` says what is owed by kind and `quiet()` waits on everything
  // but the cycle.
  const state = { animating: true, events: [], owed: { transition: true, cycle: true, depart: false, requested: false, autoscroll: false, scroll: false } };
  let t = 0;
  const app = createApp(
    { init: 0, update: (m) => m, view: () => box({ pad: 4 }) },
    { surface: { ...fakeWindow(state), owed: () => state.owed }, clock: () => t },
  );
  app.render();
  const pump = (ms) => {
    t += ms;
    app.step();
  };
  const q = app.quiet();
  const s = app.settled(100);
  pump(16);
  assert.deepEqual(await outcome(q), ['pending'], 'a transition is still owed');
  state.owed = { ...state.owed, transition: false };
  pump(16);
  assert.deepEqual(await outcome(q), ['done', 32], 'only the cycle is left: quiet');
  assert.deepEqual(await outcome(s), ['pending'], 'settled still waits on it');
  pump(80);
  assert.equal(await s, 112, 'and gives up at its cap, as before');
  // A requested frame or a departing ghost is not a cycle: quiet waits.
  state.owed = { ...state.owed, requested: true };
  const again = app.quiet();
  pump(16);
  assert.deepEqual(await outcome(again), ['pending']);
  state.owed = { ...state.owed, requested: false };
  pump(16);
  assert.deepEqual(await outcome(again), ['done', 32]);
});

test('settled() gives up at its cap the way runOut returns one (F30)', async () => {
  const state = { animating: true, events: [] };
  let t = 0;
  const app = createApp(
    { init: 0, update: (m) => m, view: () => box({ pad: 4 }) },
    { surface: fakeWindow(state), clock: () => t },
  );
  app.render();
  const capped = app.settled(50);
  t += 16;
  app.step();
  assert.deepEqual(await outcome(capped), ['pending'], 'inside the cap');
  t += 40;
  app.step();
  // Resolved, not rejected, and with the truth: nothing settled.
  assert.equal(await capped, 56);
  assert.equal(state.animating, true);
});

test('frame() is one more pump, and a pump that throws rejects both (F30)', async () => {
  const state = { animating: true, events: [] };
  let t = 0;
  const surface = fakeWindow(state);
  const app = createApp(
    { init: 0, update: (m) => m, view: () => box({ pad: 4 }) },
    { surface, clock: () => t },
  );
  app.render();
  // `frame` does not care what is moving: the next pump answers it.
  const f = app.frame();
  assert.deepEqual(await outcome(f), ['pending'], 'nothing has pumped yet');
  t += 8;
  app.step();
  assert.deepEqual(await outcome(f), ['done', undefined]);
  // A throw inside the turn takes the waiters with it: an awaited frame that
  // will never be painted is a hang, and `runWindowed` rejects its own
  // promise with the same error.
  const boom = new Error('the pump threw');
  surface.pollEvents = () => {
    throw boom;
  };
  const dead = [app.settled(), app.frame()];
  assert.throws(() => app.step(), /the pump threw/);
  await assert.rejects(dead[0], /the pump threw/);
  await assert.rejects(dead[1], /the pump threw/);
});

test('a loop that holds its own clock is told to use runOut instead (F30)', () => {
  const app = createApp({ init: 0, update: (m) => m, view: () => box({ pad: 4 }) }, { startTime: 0 });
  app.render();
  // Headless there is nothing to wait for — the test moves time itself —
  // so the wait that would never be answered is refused, as `advance`
  // refuses a wall clock.
  assert.throws(() => app.settled(), /runOut/);
  assert.throws(() => app.frame(), /runOut/);
  assert.equal(app.runOut(), 0);
});

/** A stand-in window over a real headless `Ctx`, for running `runWindowed`
 *  itself — its pump order and all — without a display. What makes it a
 *  window and not a `Ctx`: it keeps the last tree `setView` showed, and
 *  its `pump()` re-lowers that tree through the same core, which is what
 *  a real runner does between pumps for a caret blink, a pointer crossing
 *  a hover node, a live resize, or a redraw a call asked for. A seed the
 *  core holds for the next view expires on whichever frame comes first,
 *  so an older tree re-lowered there drops it with the warning. Closes
 *  after `pumps` pumps, which is when `runWindowed` resolves. */
function retainedWindow({ pumps }) {
  const ctx = new Ctx();
  let last = null;
  let left = pumps;
  return {
    relowered: 0,
    setView(tree) {
      last = tree;
      ctx.frame(320, 240, 1, tree);
    },
    pump() {
      if (last) {
        this.relowered += 1;
        ctx.frame(320, 240, 1, last);
      }
      left -= 1;
      return left > 0;
    },
    animating: () => ctx.animating(),
    nextDeadlineMs: () => null,
    pollEvents: () => ctx.pollEvents(),
    warnings: () => ctx.warnings(),
    setDiagnostics: (on) => ctx.setDiagnostics(on),
    stats: () => ctx.stats(),
    setEditText: (key, text) => ctx.setEditText(key, text),
    editText: (key) => ctx.editText(key),
  };
}

test('a dispatch outside the loop is drawn before the runner pumps, so the seed it held lands (F42)', async () => {
  // The mind map's sequence, from a promise rather than an event: the
  // `update` that opens a rename sets the field's text by label, and the
  // core holds it for the frame that declares the editor. On the event
  // path `update`, `view` and `setView` share one turn, so the next frame
  // the runner paints is the new tree and the seed lands. From a promise
  // the `update` runs in its own turn, and the driver used to go
  // `win.pump()` then `app.step()` — so the runner's redraw came first,
  // re-lowered the tree the window already had, which declares no editor,
  // and the hold expired there with `edit-text-without-editor`; `step()`
  // then drew the editor seeded from `initial`. Now the driver draws the
  // model that `dispatch` changed *before* it pumps.
  const win = retainedWindow({ pumps: 3 });
  let app;
  let views = 0;
  const model = await runWindowed(
    {
      init: { editing: false, name: 'Ideas' },
      update: (m, msg) => {
        if (msg !== 'beginEdit') return m;
        win.setEditText('field', m.name);
        return { ...m, editing: true };
      },
      view: (m) => {
        views += 1;
        return box({ pad: 4 }, [
          m.editing
            ? el('edit', { initial: '', label: 'Name', width: 200 }, [], 'field')
            : box({ width: 50, height: 20 }, [], 'row'),
        ]);
      },
    },
    {
      surface: win,
      warnings: false,
      setup: (_win, loop) => {
        app = loop;
        // A dispatch the loop did not make: after the first pump has
        // painted, from the promise that says so.
        app.frame().then(() => app.dispatch('beginEdit'));
      },
    },
  );
  assert.equal(model.editing, true, 'the window closed on the model the dispatch made');
  assert.deepEqual(
    app.warnings.filter((w) => w.code === 'edit-text-without-editor'),
    [],
    'no runner redraw lowered a tree older than the dispatch',
  );
  assert.equal(win.editText('field'), 'Ideas', 'the editor opened with the text update held');
  assert.equal(views, 2, 'the first frame and the one the dispatch owed; a re-lower runs no view');
  assert.ok(win.relowered >= 2, `the runner painted between pumps: ${win.relowered}`);
});

test('render and an event-driven frame share the clock advance moves (F1)', () => {
  // The mind map's repro (`playground/kui/mind-maps/repro/transition-advance.tsx`):
  // the loop used to set the frame clock only inside `advance`, so a frame
  // drawn by `render()` or by an event ran with none — where the core snaps
  // — and a keyed box going 100 → 400 under `transition: 200` was already at
  // 400 in the frame that applied the change, with `animating()` true for
  // 200 ms of nothing moving. Now the loop stamps the clock before every
  // frame, so the baseline is taken at t0 and the first `advance` is
  // mid-flight.
  const app = createApp(
    {
      init: { wide: false },
      update: (m, msg) => (msg === 'go' ? { wide: true } : m),
      view: (m) =>
        box({ width: 'grow', height: 'grow', pad: 20 }, [
          box({ width: m.wide ? 400 : 100, height: 30, bg: '#7aa2ff', transition: 200 }, [], 'bar'),
        ]),
    },
    { width: 640, height: 480, startTime: 0 },
  );
  const barWidth = () => decodeQuads(app.ctx.quads()).find((q) => Math.round(q.h) === 30).w;
  app.render();
  assert.equal(barWidth(), 100);
  app.dispatch('go');
  app.render();
  assert.equal(barWidth(), 100, 'the frame that applies the change is the baseline');
  app.advance(50);
  assert.ok(barWidth() > 100 && barWidth() < 400, `mid-flight at 50 ms, not ${barWidth()}`);
  assert.ok(app.ctx.animating(), 'animating while it moves');
  app.advance(200);
  assert.equal(barWidth(), 400);
  assert.equal(app.ctx.animating(), false);
});

test('a target the view moves every frame still reaches the screen (F15)', () => {
  // The mind map's pan: a canvas of `slide` floats dragged by the pointer
  // moves every card's target on every frame. Each retarget starts a fresh
  // leg at p == 0, and the tween used to retarget from its *stale* value —
  // so it spent none of the frame's time, never advanced, and the map sat
  // frozen while the model panned under it. Only a window showed it: a
  // headless assertion reads the model, which was right all along, and
  // before F1 the clock never moved so nothing eased at all.
  const app = createApp(
    {
      init: { pan: 0 },
      update: (m, msg) => (typeof msg === 'number' ? { pan: msg } : undefined),
      view: (m) =>
        box({ width: 'grow', height: 'grow' }, [
          box(
            { float: { anchor: 'parent', dx: 100 + m.pan, dy: 40 }, width: 60, height: 30,
              bg: '#7aa2ff', transition: 160, slide: true },
            [],
            'card',
          ),
        ]),
    },
    { width: 640, height: 480, startTime: 0 },
  );
  const cardX = () => decodeQuads(app.ctx.quads()).find((q) => Math.round(q.h) === 30).x;
  app.render();
  assert.equal(cardX(), 100);
  // 16 ms frames, 9.6 px of pan each: a second of a drag in flight.
  const behind = [];
  for (let f = 1; f <= 60; f++) {
    app.dispatch(f * 9.6);
    app.advance(16);
    behind.push(100 + f * 9.6 - cardX());
  }
  const target = 100 + 60 * 9.6;
  assert.ok(cardX() > 100 + 0.8 * 60 * 9.6, `the pan reaches the screen: ${cardX()} of ${target}`);
  assert.ok(
    Math.abs(behind[29] - behind[59]) < 1,
    `it trails by a fixed distance rather than falling further behind: ${behind[29]} then ${behind[59]}`,
  );
});

test("a loop on a wall clock resyncs rather than firing a burst of ticks", () => {
  // The windowed half of the same bookkeeping: `runWindowed` fills `clock`
  // with `Date.now`, so a fake one drives it without a display.
  let t = 0;
  let frames = 0;
  const app = createApp(
    {
      init: { ticks: 0 },
      update: (m) => ({ ticks: m.ticks + 1 }),
      view: (m) => {
        frames += 1;
        return box({ pad: 4 }, [text(`${m.ticks}`)]);
      },
      tick: { every: 100, msg: 'tick' },
    },
    { clock: () => t },
  );
  app.render();
  // Ten cadences of real time went past in one turn (a drag, a GC pause):
  // one tick, and the cadence picks up from here.
  t = 1000;
  app.step();
  assert.equal(app.model.ticks, 1);
  t = 1050;
  app.step();
  assert.equal(app.model.ticks, 1, 'not due yet');
  const drawn = frames;
  t = 1100;
  app.step();
  assert.equal(app.model.ticks, 2);
  assert.equal(frames, drawn + 1, 'a tick that returned a model drew');
  // Its hands are not the loop's to move.
  assert.throws(() => app.advance(100), /wall clock/);
});

test('tick.every may read the model, and a change moves the next tick (F46)', () => {
  // The pomodoro's shape: 16 ms while the countdown runs so a second is
  // never drawn late, a second while it is stopped. One static cadence
  // was the faster of the two forever.
  let ticks = 0;
  const app = createApp(
    {
      init: { running: true },
      update: (m, msg) => {
        if (msg === 'tick') {
          ticks += 1;
          return undefined;
        }
        return { running: msg === 'start' };
      },
      view: () => box({ pad: 4 }),
      tick: { every: (m) => (m.running ? 16 : 1000), msg: 'tick' },
    },
    { startTime: 0 },
  );
  app.render();
  // Running: 16 ms is the cadence, as the number would be.
  app.advance(160);
  assert.equal(ticks, 10);
  // Stopped from outside the tick: the reading is taken after the dispatch,
  // and the tick queued at 176 does not stand — the next is a second after
  // the last one that fired, so three over 3000 ms and not 188.
  app.dispatch('stop');
  ticks = 0;
  app.advance(3000);
  assert.equal(ticks, 3);
  // Started again after a long quiet: the last tick was at 3160, and
  // 3160 + 16 is in the past, so the cadence counts from the change at
  // 3660 — not a burst of the ones a 16 ms cadence would have fired
  // since, and not one this instant.
  app.advance(500);
  app.dispatch('start');
  ticks = 0;
  app.advance(15);
  assert.equal(ticks, 0, 'nothing owed at the change itself');
  app.advance(145);
  assert.equal(ticks, 10, 'then 3676 … 3820');
  // The reading is also taken after a tick's own update: a handler that
  // stops the model from inside a tick moves the cadence with it.
  let stopFromTick = false;
  const inner = createApp(
    {
      init: { running: true, n: 0 },
      update: (m, msg) => {
        if (msg !== 'tick') return m;
        ticks += 1;
        if (stopFromTick) return { running: false, n: m.n + 1 };
        return { running: true, n: m.n + 1 };
      },
      view: () => box({ pad: 4 }),
      tick: { every: (m) => (m.running ? 16 : 1000), msg: 'tick' },
    },
    { startTime: 0 },
  );
  inner.render();
  ticks = 0;
  stopFromTick = true;
  inner.advance(2000);
  // One at 16 that stops it, then 1016 — the 16 ms ticks in between are
  // not owed.
  assert.equal(ticks, 2);
  assert.equal(inner.model.running, false);
  // A function returning 0 (or less) means no tick, as the number does.
  const none = createApp(
    {
      init: { on: false, n: 0 },
      update: (m, msg) => (msg === 'tick' ? { ...m, n: m.n + 1 } : { ...m, on: msg === 'on' }),
      view: () => box({ pad: 4 }),
      tick: { every: (m) => (m.on ? 100 : 0), msg: 'tick' },
    },
    { startTime: 0 },
  );
  none.render();
  none.advance(1000);
  assert.equal(none.model.n, 0, 'off: nothing fires');
  none.dispatch('on');
  none.advance(1000);
  assert.equal(none.model.n, 10, 'on: the cadence counts from the change');
});

test('a model that says 1000 lets the windowed driver idle, where 16 pinned it (F46)', () => {
  // The other half of the same entry, over the fake surface the windowed
  // bookkeeping is tested on: the driver asks the loop how long it may
  // park (`[BUDGET]`, private to index.js), and a 16 ms tick capped the
  // answer at 16 ms whatever the backoff had reached. The symbol is found
  // by name so the loop's public shape stays what `Loop` declares.
  const state = { animating: false, events: [] };
  let t = 0;
  const app = createApp(
    {
      init: { running: true },
      update: (m, msg) => (msg === 'tick' ? undefined : { running: msg === 'start' }),
      view: () => box({ pad: 4 }),
      tick: { every: (m) => (m.running ? 16 : 1000), msg: 'tick' },
    },
    { surface: fakeWindow(state), clock: () => t },
  );
  const budget = Object.getOwnPropertySymbols(app).find((s) => s.description === 'kui.budget');
  const ask = (busy, idle) => app[budget](busy, idle, state.animating);
  app.render();
  // Running: the next tick is 16 ms out, so the gap is 16 even with the
  // backoff at 32 — the driver may not sleep through a tick.
  assert.equal(ask(8, 32), 16);
  t = 16;
  app.step();
  assert.equal(ask(8, 32), 16);
  // Stopped: the reading moves the next tick to 1016, and the backoff's
  // own gap is the answer — this is the pomodoro's 7% becoming ~3%.
  app.dispatch('stop');
  assert.equal(ask(8, 32), 32);
  assert.equal(ask(8, 250), 250, 'and a deeper idle is not capped by the tick either');
  assert.equal(app.step(), true, 'the dispatch itself is a frame on the next turn');
  t = 500;
  assert.equal(app.step(), false, 'and after it, nothing until 1016');
  t = 1016;
  assert.equal(app.step(), false, 'the tick fired and returned undefined, so no frame');
  assert.equal(ask(8, 32), 32, 'the next is at 2016');
  // Started again well after the last tick: the cadence counts from the
  // change, 16 ms out, and the driver is back at frame rate.
  t = 1900;
  app.dispatch('start');
  assert.equal(ask(8, 32), 16);
  t = 1916;
  app.step();
  assert.equal(ask(8, 32), 16, 'and on from 1932');
  // Something animating still wins over everything: the frame cadence.
  state.animating = true;
  assert.equal(ask(8, 32), 8);
});

test("a tick's own frame is not the user, so the windowed driver keeps idling through it (F57)", () => {
  // The pomodoro took `every: (m) => m.running ? 16 : 1000` and found its
  // stopped mid tier at 4.5% of a core against 5.6% at 16 ms, for a change
  // that should have cost sixty times less: the once-a-second tick drew a
  // frame, the frame counted as work, and the backoff went back to 8 ms
  // for the `quietMs` after every digit. `step` now says what the turn was
  // for, and only something other than the loop's clock counts.
  const state = { animating: false, events: [] };
  let t = 0;
  const app = createApp(
    {
      init: { n: 0 },
      update: (m, msg) => (msg === 'tick' ? { n: m.n + 1 } : { n: 100 }),
      view: () => box({ pad: 4 }),
      tick: { every: 1000, msg: 'tick' },
    },
    { surface: fakeWindow(state), clock: () => t },
  );
  const STEP = Object.getOwnPropertySymbols(app).find((s) => s.description === 'kui.step');
  app.render();
  t = 500;
  assert.deepEqual(app[STEP](), { drew: false, used: false }, 'nothing to do');
  t = 1000;
  assert.deepEqual(app[STEP](), { drew: true, used: false }, 'the tick drew and is not use');
  assert.equal(app.model.n, 1);
  state.events.push({ origin: 0, key: '', payload: 'click' });
  assert.deepEqual(app[STEP](), { drew: true, used: true }, 'an event is');
  app.dispatch('foreign');
  assert.deepEqual(app[STEP](), { drew: true, used: true }, 'and so is a dispatch from outside the loop');
  assert.equal(app.step(), false, 'the public `step` still answers whether it drew');

  // The driver's backoff over those answers: a tick frame a second leaves
  // the gap at its idle value; an event brings it back to the busy one.
  const PACE = Object.getOwnPropertySymbols(runWindowed).find((s) => s.description === 'kui.pace');
  const pace = runWindowed[PACE]({ busyMs: 8, idleMs: 32, quietMs: 500 });
  let now = 0;
  const quiet = (ms) => {
    for (let i = 0; i < ms / 8; i++) pace.after(false, (now += 8));
    return pace.gap;
  };
  assert.equal(quiet(496), 8, 'the first half-second keeps the busy cadence');
  assert.equal(quiet(16), 16, 'then doubles');
  assert.equal(quiet(16), 32, 'to the ceiling');
  // Ticks at 1000 and 2000: not used, so the gap stays where it was.
  assert.equal(pace.after(false, (now = 1000)), 32, 'a tick frame at 1000 changes nothing');
  assert.equal(quiet(1000), 32);
  assert.equal(pace.after(true, (now = 2016)), 8, 'a click is use');
  assert.equal(quiet(496), 8, 'and the quiet counts from it');
  assert.equal(quiet(40), 32);
});

test('a stopped app pumps at a lower rate in its second second than in the 100 ms after a click, and the count is readable (F62)', async (t) => {
  // The pomodoro measured F57 through `top`, "because the runner knows
  // how often it pumped and the app cannot ask" — and `frameStats().frames`
  // was no counter either, being the 120-sample ring's fill. `pumps` and
  // `framesTotal` are the monotonic counts now; here the driver runs over
  // a fake surface that counts its own pumps, on mocked timers so the
  // backoff's whole second passes in no time.
  t.mock.timers.enable({ apis: ['setTimeout', 'Date'], now: 0 });
  const state = { animating: false, events: [] };
  let pumps = 0;
  let alive = true;
  const win = {
    ...fakeWindow(state),
    pump: () => {
      pumps += 1;
      return alive;
    },
    nextDeadlineMs: () => null,
  };
  const done = runWindowed(
    {
      init: { n: 0 },
      update: (m, msg) => (msg === 'tick' ? { n: m.n + 1 } : m),
      view: () => box({ pad: 4 }),
      tick: { every: 1000, msg: 'tick' },
    },
    { surface: win, warnings: false },
  );
  // A millisecond at a time: a mocked `tick(n)` runs only the timers
  // that were pending when it started, and every pump schedules the next.
  const pass = (ms) => {
    for (let i = 0; i < ms; i++) t.mock.timers.tick(1);
  };
  // A click at 0: the driver stays at the busy cadence for the half
  // second after it.
  state.events.push({ origin: 0, key: '', payload: 'click' });
  const before = pumps;
  pass(100);
  const first = pumps - before;
  // Through the rest of the first second and the whole of the second:
  // one tick at 2000, otherwise nothing, so the backoff sits at its
  // ceiling and the tick's own frame does not bring it down (F57).
  pass(900);
  const mid = pumps;
  pass(1000);
  const second = pumps - mid;
  assert.ok(first >= 10 && first <= 14, `~12 pumps at 8 ms in the first 100 ms: ${first}`);
  assert.ok(second >= 28 && second <= 36, `~31 pumps at 32 ms over the second second: ${second}`);
  assert.ok(second / 1000 < first / 100, 'a lower rate, not a lower count');
  alive = false;
  pass(100);
  await done;
});

test('tick.msg is told the cadence it fired on, beside the time (F59)', () => {
  // The pomodoro's fourth wish: a press cannot anchor on `model.now` when
  // the tick that wrote it may be a second old, and the model had no way
  // to know which. The cadence in force when the tick fired is the bound
  // on that staleness, and `msg(now, every)` carries it.
  const app = createApp({
    init: { seen: [] },
    update: (m, msg) => (msg.kind === 'tick' ? { ...m, seen: [...m.seen, [msg.now, msg.every]] } : msg.kind === 'fast' ? { ...m, fast: true } : m),
    view: () => box({ pad: 4 }),
    tick: { every: (m) => (m.fast ? 16 : 1000), msg: (now, every) => ({ kind: 'tick', now, every }) },
  }, { startTime: 0 });
  app.advance(1000);
  app.advance(1000);
  assert.deepEqual(app.model.seen, [[1000, 1000], [2000, 1000]]);
  app.dispatch({ kind: 'fast' });
  app.advance(16);
  app.advance(16);
  assert.deepEqual(app.model.seen.slice(2), [[2016, 16], [2032, 16]]);
});

test('a loop over a surface that takes no synthetic input says so', () => {
  const app = createApp(
    { init: 0, update: () => undefined, view: () => box({ pad: 4 }) },
    // A stand-in for a real window: it shows a view and polls, but its input
    // comes from the OS.
    {
      surface: {
        setView() {},
        pollEvents: () => [],
        warnings: () => [],
        setDiagnostics() {},
        stats: () => ({}),
      },
    },
  );
  app.render();
  app.settle();
  assert.throws(() => app.click(1, 1), /no mouse\(\) to drive it with/);
});

test('the access tree derives roles and names, and requests drive the app', () => {
  const app = createApp(
    {
      init: { n: 0, text: 'hi', nudged: null, tag: null },
      update: (m, msg, ev) => {
        if (msg === 'bump') return { ...m, n: m.n + 1 };
        if (msg?.kind === 'changed') return { ...m, text: app.ctx.editText(ev.key) };
        if (msg?.kind === 'access') return { ...m, nudged: msg.action, tag: msg.tag };
      },
      view: (m) =>
        box({ title: 'Demo', gap: 4, pad: 4 }, [
          // Named by its text; the text is read as part of the button.
          box({ onClick: 'bump', pad: 4, bg: '#333333' }, [text(`count ${m.n}`)], 'bump'),
          // An icon button, named by its label.
          box({ onClick: 'save', label: 'Save', width: 20, height: 20, bg: '#333333' }, [], 'save'),
          box(
            { role: 'slider', label: 'Volume', valueNow: 3, valueMin: 0, valueMax: 10, onDrag: 'vol', width: 100, height: 10 },
            [],
            'vol',
          ),
          // Decoration: gone from the tree, subtree included.
          box({ role: 'none', onClick: 'hidden' }, [text('Hidden')]),
          // Plain structure: elided, its editor hangs off the window.
          box({ pad: 8 }, [el('edit', { initial: 'hi', label: 'Name', width: 100 }, [], 'name')]),
        ]),
    },
    { warnings: false },
  );
  app.render();
  const tree = app.accessTree();
  assert.equal(tree.nodes[0].role, 'window');
  assert.equal(tree.nodes[0].name, 'Demo');
  const byName = (n) => tree.nodes.find((x) => x.name === n);
  const bump = byName('count 0');
  assert.equal(bump.role, 'button');
  assert.equal(bump.parent, tree.nodes[0].key);
  assert.ok(bump.actions.includes('click'));
  assert.equal(byName('Save').role, 'button');
  const vol = byName('Volume');
  assert.equal(vol.role, 'slider');
  assert.deepEqual([vol.valueNow, vol.valueMin, vol.valueMax], [3, 0, 10]);
  assert.ok(vol.actions.includes('increment'));
  assert.equal(byName('Hidden'), undefined);
  const name = byName('Name');
  assert.equal(name.role, 'textInput');
  assert.equal(name.value, 'hi');
  assert.equal(name.parent, tree.nodes[0].key);
  assert.equal(tree.nodes.length, 5, 'window, two buttons, slider, editor');
  assert.equal(tree.focus, null);
  assert.equal(typeof tree.hash, 'string');

  app.access(bump.key, 'click');
  assert.equal(app.model.n, 1);
  assert.equal(app.accessTree().nodes[1].name, 'count 1');
  app.access(vol.key, 'increment');
  assert.deepEqual([app.model.nudged, app.model.tag], ['increment', 'vol']);
  app.access(name.key, 'setValue', 'world');
  assert.equal(app.model.text, 'world');
  app.access(name.key, 'focus');
  app.render();
  assert.equal(app.accessTree().focus, name.key);
  assert.ok(app.accessTree().nodes.find((x) => x.key === name.key).focused);
  assert.equal(app.warnings.length, 0);
  assert.throws(() => app.access(name.key, 'teleport'), /unknown access action/);
});

// `selected` and `expanded` are one schema row each, so every binding
// carries them; "3 of 7" is not a row at all — the core numbers what a
// list holds.
test('selection, disclosure and set position reach the access tree', () => {
  const build = () =>
    box({ pad: 4 }, [
      box({ role: 'tabList', gap: 2 }, [
        box({ role: 'tab', selected: false, onClick: 0 }, [text('General', { size: 12 })], 't0'),
        box({ role: 'tab', selected: true, onClick: 1 }, [text('Network', { size: 12 })], 't1'),
      ]),
      box({ role: 'list' }, [
        box({ role: 'listItem', selected: true }, [text('one', { size: 12 })], 'r0'),
        box({ role: 'listItem' }, [text('two', { size: 12 })], 'r1'),
      ]),
      box({ onClick: 'toggle', expanded: 'collapsed' }, [text('Advanced', { size: 12 })], 'adv'),
      box({ onClick: 'go' }, [text('Save', { size: 12 })], 'save'),
    ]);
  const { ctx } = run(build);
  const nodes = ctx.accessTree().nodes;
  const byName = (n) => nodes.find((x) => x.name === n);

  // Every tab reports the state; a reader can say which one is on.
  assert.deepEqual([byName('General').selected, byName('Network').selected], [false, true]);
  // A row says so only where it is picked: a plain list is not a
  // selection. A `listItem` is not named by its content, so the rows
  // are found through the list that holds them.
  const list = nodes.find((n) => n.role === 'list');
  const rows = nodes.filter((n) => n.parent === list.key && n.role === 'listItem');
  assert.deepEqual(rows.map((n) => n.selected), [true, null]);
  // A shut disclosure says it is shut; an ordinary button says nothing.
  assert.equal(byName('Advanced').expanded, false);
  assert.equal(byName('Save').expanded, null);
  assert.equal(byName('Save').selected, null);

  // Derived, not declared: the ordinal on the item, the count on the
  // container, and nothing outside a list or tab list.
  const tabs = nodes.find((n) => n.role === 'tabList');
  assert.deepEqual([tabs.setSize, list.setSize], [2, 2]);
  assert.equal(tabs.posInSet, null);
  assert.deepEqual([byName('General').posInSet, byName('Network').posInSet], [0, 1]);
  assert.deepEqual(rows.map((n) => n.posInSet), [0, 1]);
  assert.equal(byName('Save').posInSet, null);
});

test('setEditText names an editor by the label the view declares (F32)', () => {
  // The mind map's sequence. A rename opens from `update`, which fills the
  // field with the model's text in the same turn — and has no key to name
  // it with, since the hex key comes from an event an editor being opened
  // has not fired. The name is the one the view's `key` prop declares, and
  // the frame that declares the editor takes the text.
  const app = createApp(
    {
      init: { editing: null, names: { n13: 'Ideas' } },
      update: (m, msg) => {
        if (msg?.kind === 'rename') {
          app.ctx.setEditText(`edit-${msg.id}`, m.names[msg.id]);
          return { ...m, editing: msg.id };
        }
        if (msg === 'cancel') return { ...m, editing: null };
      },
      view: (m) =>
        box({ pad: 4 }, [
          m.editing
            ? el('edit', { initial: '', label: 'Node text', width: 200 }, [], `edit-${m.editing}`)
            : box({ onClick: { kind: 'rename', id: 'n13' }, width: 50, height: 20 }, [], 'row'),
        ]),
    },
    { warnings: false },
  );
  app.render();
  app.dispatch({ kind: 'rename', id: 'n13' });
  app.render();
  assert.equal(app.ctx.editText('edit-n13'), 'Ideas', 'the frame that opened it took the text');
  assert.deepEqual(app.warnings.filter((w) => w.code === 'edit-text-without-editor'), []);
  // The label reads back as a key too, which is what the diagnostic names.
  assert.match(app.ctx.keyOf('edit-n13'), /^[0-9a-f]{16}$/);
  assert.equal(app.ctx.keyOf('edit-n99'), null);

  // A second open, over an abandoned draft: the editor's state is retained
  // while its key is off screen, so the frame that brings it back has to
  // take the model's text and not what was typed into it.
  app.ctx.setEditText('edit-n13', 'Ideas and more');
  app.dispatch('cancel');
  app.render();
  assert.equal(app.ctx.keyOf('edit-n13'), null, 'a closed editor is in no frame to resolve');
  app.dispatch({ kind: 'rename', id: 'n13' });
  app.render();
  assert.equal(app.ctx.editText('edit-n13'), 'Ideas', 'the draft did not come back');

  // A label nothing declares is a line, not a silent drop.
  app.ctx.setEditText('edit-n99', 'nowhere');
  app.render();
  const ws = app.warnings.filter((w) => w.code === 'edit-text-without-editor');
  assert.equal(ws.length, 1);
  assert.match(ws[0].message, /edit-n99/);
});

test('editors expose runs and take selection requests; custom editors get them as messages', () => {
  const app = createApp(
    {
      init: { text: 'hello world\nsecond', sel: null, replaced: null },
      update: (m, msg, ev) => {
        if (msg?.kind === 'changed') return { ...m, text: app.ctx.editText(ev.key) };
        if (msg?.kind === 'access' && msg.action === 'setTextSelection')
          return { ...m, sel: [msg.anchor, msg.focus, msg.tag] };
        if (msg?.kind === 'access' && msg.action === 'replaceSelectedText')
          return { ...m, replaced: msg.text };
      },
      view: () =>
        box({ gap: 8, pad: 8 }, [
          el('edit', { initial: 'hello world\nsecond', multiline: true, autofocus: true, label: 'Doc', width: 300 }, [], 'doc'),
          // An editor the app draws itself: two lines, the caret at the
          // end of the second, the anchor three bytes into the first.
          box({ role: 'multilineTextInput', label: 'Mine', onKey: 'k' }, [
            box({ role: 'none' }, [text('1'), text('2')]),
            box({ role: 'line', selectionAnchor: 3 }, [text('fn ma'), text('in()')]),
            box({ role: 'line', caret: 2 }, [text('hi')]),
          ], 'mine'),
        ]),
    },
    { warnings: false },
  );
  app.render();
  let tree = app.accessTree();
  const doc = tree.nodes.find((n) => n.name === 'Doc');
  assert.equal(doc.role, 'multilineTextInput');
  assert.equal(doc.value, 'hello world\nsecond');
  assert.equal(doc.runs.length, 2);
  assert.equal(doc.runs[0].text, 'hello world\n');
  assert.deepEqual(doc.runs[0].wordStarts, [0, 6]);
  assert.equal(doc.runs[0].charLengths.length, 12);
  assert.equal(doc.runs[0].charPositions[0], 0);
  assert.ok(doc.runs[0].charWidths[0] > 0);
  assert.deepEqual(doc.focus, { run: doc.runs[0].key, character: 0 });
  assert.ok(doc.actions.includes('setTextSelection'));

  // Select "world\nsec" and type over it.
  app.access(doc.key, 'setTextSelection', {
    anchor: { run: doc.runs[0].key, character: 6 },
    focus: { run: doc.runs[1].key, character: 3 },
  });
  app.render();
  tree = app.accessTree();
  const doc2 = tree.nodes.find((n) => n.name === 'Doc');
  assert.deepEqual(doc2.selection, [6, 15]);
  assert.deepEqual(doc2.anchor, { run: doc2.runs[0].key, character: 6 });
  app.access(doc.key, 'replaceSelectedText', 'there ');
  assert.equal(app.model.text, 'hello there ond');

  // The custom editor: value from its lines, runs from their text nodes,
  // caret and anchor from the props; a request comes back as a message.
  const mine = tree.nodes.find((n) => n.name === 'Mine');
  assert.equal(mine.role, 'multilineTextInput');
  assert.equal(mine.value, 'fn main()\nhi');
  assert.equal(mine.runs.length, 3);
  assert.equal(mine.runs[1].text, 'in()\n');
  assert.deepEqual([mine.runs[1].line, mine.runs[1].start, mine.runs[1].end], [0, 5, 9]);
  assert.deepEqual(mine.anchor, { run: mine.runs[0].key, character: 3 });
  assert.deepEqual(mine.focus, { run: mine.runs[2].key, character: 2 });
  assert.equal(mine.caret, 12);
  assert.deepEqual(mine.selection, [3, 12]);
  assert.equal(tree.nodes.filter((n) => n.role === 'staticText').length, 0, 'lines and gutter are the editor');
  app.access(mine.key, 'setTextSelection', {
    anchor: { run: mine.runs[1].key, character: 1 },
    focus: { run: mine.runs[2].key, character: 0 },
  });
  assert.deepEqual(app.model.sel, [{ line: 0, offset: 6 }, { line: 1, offset: 0 }, 'k']);
  app.access(mine.key, 'replaceSelectedText', 'x');
  assert.equal(app.model.replaced, 'x');
  assert.equal(app.warnings.length, 0);
});

test('unnamed controls and unlabelled images warn once', () => {
  const ctx = new Ctx();
  const id = ctx.addImage(1, 1, Buffer.alloc(4, 255));
  const tree = box({}, [
    el('image', { src: id, width: 10, height: 10 }),
    el('image', { src: id, width: 10, height: 10, label: 'Logo' }),
    el('image', { src: id, width: 10, height: 10, role: 'none' }),
    box({ onClick: 'x', width: 10, height: 10 }),
    box({ onClick: 'y', width: 10, height: 10, tooltip: 'Do y' }, [text('Y')]),
  ]);
  ctx.frame(320, 240, 1, tree);
  const ws = ctx.warnings();
  assert.deepEqual(
    ws.map((w) => w.code),
    ['image-without-label', 'control-without-name'],
  );
  ctx.frame(320, 240, 1, tree);
  assert.equal(ctx.warnings().length, 0, 'once');
  const y = ctx.accessTree().nodes.find((n) => n.name === 'Y');
  assert.equal(y.description, 'Do y', 'the tooltip is the description');
});

// Declarative hover styling: the core swaps hoverBg / pressedBg in while
// the pointer is over the node (or its hoverGroup), and onHover reports
// enter/leave as events — no isHovered query in the view.
const solidColor = (quads, stride, x) => {
  for (let off = 0; off < quads.byteLength; off += stride) {
    if (quads.readUInt32LE(off + KIND_WORD * 4) !== 0) continue; // solid only
    if (quads.readFloatLE(off) === x && quads.readFloatLE(off + 2 * 4) === 50) {
      return [quads.readFloatLE(off + 4 * 4), quads.readFloatLE(off + 5 * 4), quads.readFloatLE(off + 6 * 4)];
    }
  }
  return null;
};

test('hoverBg, pressedBg and hoverGroup resolve in the core', () => {
  const ctx = new Ctx();
  const pair = (extra) =>
    box({ dir: 'row' }, [
      box({ width: 50, height: 50, bg: '#102030', hoverBg: '#405060', pressedBg: '#708090', ...extra }, [], 'a'),
      box({ width: 50, height: 50, bg: '#102030', hoverBg: '#405060', pressedBg: '#708090', ...extra }, [], 'b'),
    ]);
  const render = (extra) => {
    ctx.frame(320, 240, 1, pair(extra));
    const quads = Buffer.from(ctx.quads());
    const stride = quads.byteLength / ctx.stats().quadCount;
    return [solidColor(quads, stride, 0), solidColor(quads, stride, 50)];
  };
  const near = (rgb, hex) => {
    const want = [1, 3, 5].map((i) => parseInt(hex.slice(i, i + 2), 16) / 255);
    assert.ok(rgb && rgb.every((c, i) => Math.abs(c - want[i]) < 0.01), `${rgb} should be ${hex}`);
  };
  let [a, b] = render({});
  near(a, '#102030');
  ctx.cursor(10, 10);
  [a, b] = render({});
  near(a, '#405060');
  near(b, '#102030');
  ctx.mouse(true);
  [a] = render({});
  near(a, '#708090');
  ctx.mouse(false);
  // A group lights both blocks from either side.
  [a, b] = render({ hoverGroup: 'pair' });
  [a, b] = render({ hoverGroup: 'pair' });
  near(a, '#405060');
  near(b, '#405060');
});

test('onDrop hears every phase of an OS file drag, and dropBg lights the zone (ADR 0031)', () => {
  const ctx = new Ctx();
  const tree = (lit) => box({ dir: 'row' }, [
    box(
      { width: 100, height: 100, bg: '#102030', dropBg: '#405060', onDrop: { kind: 'files', zone: 'a' } },
      // A button inside the zone is the zone's.
      [box({ width: 30, height: 30, onClick: { kind: 'pick' } }, [], 'pick')],
      'a',
    ),
    box({ width: 100, height: 100, onDrop: { kind: 'files', zone: 'b' } }, [], 'b'),
    // What an app shows on enter: a hoverable float that is no zone, and
    // is looked past.
    lit && box({ float: { anchor: 'viewport', at: ['start', 'start'], self: ['start', 'start'], dx: 0, dy: 0 }, width: 200, height: 100, hoverable: true }, [], 'overlay'),
  ].filter(Boolean));
  ctx.frame(320, 240, 1, tree(false));
  const paths = ['/tmp/one.txt', '/tmp/two.png'];
  ctx.dragFiles(paths, 10, 10);
  let evs = ctx.pollEvents();
  assert.deepEqual(evs.map((e) => e.payload), [
    { kind: 'drop', phase: 'enter', paths, x: 10, y: 10, tag: { kind: 'files', zone: 'a' } },
  ]);
  const aKey = evs[0].key;
  assert.equal(ctx.dropTarget(), aKey);
  assert.ok(ctx.isDropTarget('a'));
  // The zone lights up, in the frame the view builds in answer.
  ctx.frame(320, 240, 1, tree(true));
  const zone = decodeQuads(ctx.quads()).find((q) => q.kind === 0 && q.w === 100 && q.h === 100);
  assert.ok(zone && Math.abs(zone.color[0] - 0x40 / 255) < 0.01, 'dropBg painted');
  // Over the overlay, at a new point: a move on the zone, not a leave.
  ctx.dragFiles(paths, 50, 50);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload.phase), ['move']);
  // Into the other zone: leave (no point), then enter.
  ctx.dragFiles(paths, 150, 50);
  evs = ctx.pollEvents();
  assert.deepEqual(evs.map((e) => `${e.payload.phase}:${e.payload.tag.zone}`), ['leave:a', 'enter:b']);
  assert.equal(evs[0].payload.x, undefined);
  // Landed: the drop, no leave after it, nothing lit.
  ctx.dropFiles(paths, 150, 50);
  evs = ctx.pollEvents();
  assert.deepEqual(evs.map((e) => `${e.payload.phase}:${e.payload.tag.zone}`), ['drop:b']);
  assert.equal(ctx.dropTarget(), null);
  // Out of the window: the lit zone's leave.
  ctx.dragFiles(paths, 10, 10);
  ctx.dragCancel();
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload.phase), ['enter', 'leave']);
});

test('onHover emits enter and leave with the tag', () => {
  const ctx = new Ctx();
  const tree = box({ dir: 'row' }, [
    box({ width: 50, height: 50, onHover: { kind: 'hov', id: 'a' } }, [], 'a'),
    box({ width: 50, height: 50, onHover: { kind: 'hov', id: 'b' } }, [], 'b'),
  ]);
  ctx.frame(320, 240, 1, tree);
  ctx.cursor(10, 10);
  let evs = ctx.pollEvents();
  assert.deepEqual(evs.map((e) => e.payload), [{ kind: 'hover', phase: 'enter', by: 'pointer', tag: { kind: 'hov', id: 'a' } }]);
  const aKey = evs[0].key;
  assert.ok(ctx.isHovered(aKey));
  ctx.cursor(60, 10);
  evs = ctx.pollEvents();
  assert.deepEqual(
    evs.map((e) => e.payload.phase + ':' + e.payload.tag.id),
    ['leave:a', 'enter:b'],
  );
  // A frame that removes the hovered node under a still cursor reports the
  // leave right after the frame, without further input.
  ctx.frame(320, 240, 1, box({}));
  evs = ctx.pollEvents();
  assert.deepEqual(evs.map((e) => e.payload.phase + ':' + e.payload.tag.id), ['leave:b']);
  ctx.cursorLeft();
  assert.deepEqual(ctx.pollEvents(), []);
});

// The pointer shape is declared, not derived: `cursorShape()` reads which
// node's `cursor` the core resolved under the pointer, in the prop's own
// words, so a driver applies it and a test asserts it — the I-beam over
// text is the one shape the core implies, and a node that declared nothing
// is the arrow whatever it does. The bands and the expectations mirror
// crates/kui-core/tests/cursor.rs so both suites agree on the vocabulary.
test('cursorShape is what the node under the pointer declared', () => {
  const ctx = new Ctx();
  const BAND = 40;
  const band = (props, key, children = []) => box({ width: 'grow', height: BAND, ...props }, children, key);
  // One full-width band per thing a cursor can be over, stacked, so a y
  // picks one.
  const tree = box({ dir: 'column', width: 'grow', height: 'grow' }, [
    el('edit', { initial: 'hello', width: 'grow', height: BAND }, [], 'doc'), // 0
    band({ onClick: { kind: 'go' }, cursor: 'pointer' }, 'button'), // 1: says it is a hand
    band({}, 'plain'), // 2: no hit region at all
    band({ onDrag: { kind: 'h' }, cursor: 'grab' }, 'handle'), // 3: says it is a grab
    band({ onDrag: { kind: 's' }, cursor: 'ewResize' }, 'splitter'), // 4: resizes rather than moves
    band({ onClick: { kind: 'nope' }, disabled: true, cursor: 'notAllowed' }, 'refused'), // 5
    band({ onClick: { kind: 'nope' }, disabled: true }, 'quiet'), // 6
    band({ focusable: true }, 'row'), // 7: opens on Enter, no click payload
    band({ hoverable: true }, 'badge'), // 8: hover-only
    band({ onClick: { kind: 'go' } }, 'silent'), // 9: clickable, said nothing
    band({ onDrag: { kind: 'm' } }, 'mute'), // 10: draggable, said nothing
    // 11: the stock button, centred in a band so the middle lands on it.
    band({ dir: 'column', mainAlign: 'center', crossAlign: 'center' }, 'stock', [
      el('button', { onClick: { kind: 'go' } }, ['Go']),
    ]),
  ]);
  ctx.frame(400, 400, 1, tree);
  const over = (i) => {
    ctx.cursor(200, i * BAND + BAND / 2);
    return ctx.cursorShape();
  };

  assert.equal(ctx.cursorShape(), 'default', 'before any input');
  assert.equal(over(0), 'text', 'an editor is a caret');
  assert.equal(over(1), 'pointer', 'a declared hand is a hand');
  assert.equal(over(2), 'default', 'a plain box is the arrow');
  assert.equal(over(9), 'default', 'an onClick that declared nothing is the arrow');
  assert.equal(over(7), 'default', 'so is a focusable node');
  assert.equal(over(10), 'default', 'and an onDrag');
  assert.equal(over(8), 'default', 'and a hover-only node');
  assert.equal(over(11), 'pointer', 'the stock <button> declares the hand itself');

  // A declared grab holds through the captured drag, off the node too; the
  // core promotes nothing to grabbing — the view declares that as its drag
  // state changes.
  assert.equal(over(3), 'grab', 'resting');
  ctx.mouse(true);
  assert.equal(ctx.cursorShape(), 'grab', 'pressed: as declared');
  ctx.cursor(200, 2 * BAND + BAND / 2);
  assert.equal(ctx.cursorShape(), 'grab', 'off the node, still captured');
  ctx.mouse(false);
  assert.equal(ctx.cursorShape(), 'default', 'over the plain box now');
  assert.equal(over(3), 'grab', 'back to rest');

  // A splitter's own arrows, through the drag too.
  assert.equal(over(4), 'ewResize', 'resting');
  ctx.mouse(true);
  ctx.cursor(200, 2 * BAND + BAND / 2);
  assert.equal(ctx.cursorShape(), 'ewResize', 'the captured drag keeps the declared shape');
  ctx.mouse(false);

  // A disabled control is the arrow unless it declares otherwise.
  assert.equal(over(5), 'notAllowed', 'declared');
  assert.equal(over(6), 'default', 'quiet');

  ctx.cursorLeft();
  assert.equal(ctx.cursorShape(), 'default', 'no pointer is the arrow');
  ctx.pollEvents();
});

test('cursorShape answers for the topmost node, as a click would', () => {
  const ctx = new Ctx();
  const tree = box({ width: 'grow', height: 'grow', pad: 20, hoverable: true }, [
    box({ width: 100, height: 30, onClick: { kind: 'go' }, cursor: 'pointer' }, [], 'button'),
  ]);
  ctx.frame(400, 400, 1, tree);
  ctx.cursor(50, 30);
  assert.equal(ctx.cursorShape(), 'pointer', 'over the button');
  ctx.cursor(50, 200);
  assert.equal(ctx.cursorShape(), 'default', 'over the card');
});

test('onContextMenu answers the secondary button and nothing else', () => {
  const ctx = new Ctx();
  const tree = box({ width: 'grow', height: 'grow', onContextMenu: { kind: 'menu', on: 'panel' } }, [
    box({ width: 50, height: 50, onClick: { kind: 'open' }, onContextMenu: { kind: 'menu', on: 'row' } }, [], 'row'),
  ]);
  ctx.frame(320, 240, 1, tree);

  // The press is what opens a menu, at the point to open it at, and the
  // release adds nothing — the node's onClick stays out of it.
  ctx.cursor(20, 30);
  ctx.mouse(true, 1, 'secondary');
  let evs = ctx.pollEvents();
  assert.deepEqual(
    evs.map((e) => e.payload),
    [{ kind: 'contextmenu', x: 20, y: 30, tag: { kind: 'menu', on: 'row' } }],
  );
  const rowKey = evs[0].key;
  ctx.mouse(false, 1, 'secondary');
  assert.deepEqual(ctx.pollEvents(), []);

  // The same node still clicks with the primary button.
  ctx.mouse(true, 1);
  ctx.mouse(false, 1);
  assert.deepEqual(
    ctx.pollEvents().map((e) => e.payload),
    [{ kind: 'open' }],
  );

  // Routed like a click: the container answers where no child covers it.
  ctx.cursor(200, 100);
  ctx.mouse(true, 1, 'secondary');
  evs = ctx.pollEvents();
  assert.deepEqual(evs.map((e) => e.payload.tag.on), ['panel']);
  assert.notEqual(evs[0].key, rowKey);
  ctx.mouse(false, 1, 'secondary');

  // A middle press no `onButton` claims routes nowhere.
  ctx.mouse(true, 1, 'middle');
  ctx.mouse(false, 1, 'middle');
  assert.deepEqual(ctx.pollEvents(), []);
  assert.throws(() => ctx.mouse(true, 1, 'left'), /unknown mouse button/);
});

// Backlog F105: the non-primary buttons on the node that claims them,
// captured from press to release, the secondary one left to the menu when
// the mask says so.
test('onButton hears the middle button from press to release, wherever it goes', () => {
  const ctx = new Ctx();
  const tree = box({ width: 'grow', height: 'grow', onContextMenu: { kind: 'menu' } }, [
    box({ width: 100, height: 50, onButton: { kind: 'pane' }, buttons: 'middle' }, [], 'pane'),
  ]);
  ctx.frame(320, 240, 1, tree);
  ctx.cursor(20, 30);
  ctx.pollEvents();
  ctx.mouse(true, 1, 'middle');
  ctx.cursor(200, 100);
  ctx.mouse(false, 1, 'middle');
  const evs = ctx.pollEvents();
  assert.deepEqual(
    evs.map((e) => e.payload),
    [
      { kind: 'button', phase: 'press', button: 'middle', x: 20, y: 30, clicks: 1, tag: { kind: 'pane' } },
      { kind: 'button', phase: 'move', button: 'middle', x: 200, y: 100, tag: { kind: 'pane' } },
      { kind: 'button', phase: 'release', button: 'middle', x: 200, y: 100, tag: { kind: 'pane' } },
    ],
  );
  assert.ok(evs.every((e) => e.key === evs[0].key), 'the release is the pane\'s, off it');

  // `buttons: 'middle'` leaves the secondary button to the menu around it.
  ctx.cursor(20, 30);
  ctx.mouse(true, 1, 'secondary');
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload.kind), ['contextmenu']);
  ctx.mouse(false, 1, 'secondary');
  ctx.pollEvents();

  // Claimed, it is the pane's instead.
  const all = box({ width: 'grow', height: 'grow', onContextMenu: { kind: 'menu' } }, [
    box({ width: 100, height: 50, onButton: { kind: 'pane' } }, [], 'pane'),
  ]);
  ctx.frame(320, 240, 1, all);
  ctx.pollEvents();
  ctx.mouse(true, 1, 'secondary');
  assert.deepEqual(
    ctx.pollEvents().map((e) => [e.payload.kind, e.payload.phase, e.payload.button]),
    [['button', 'press', 'secondary']],
  );
  ctx.mouse(false, 1, 'secondary');
  ctx.pollEvents();

  // A button past the middle one is its number, as the event names it
  // (backlog RG75): the back button, 3 + 0.
  ctx.mouse(true, 1, 3);
  ctx.mouse(false, 1, 3);
  assert.deepEqual(
    ctx.pollEvents().map((e) => [e.payload.phase, e.payload.button]),
    [['press', 3], ['release', 3]],
  );
});

test('a right-click leaves keyboard focus where it was', () => {
  const ctx = new Ctx();
  const tree = box({}, [
    el('edit', { initial: 'hello', size: 13, width: 160, height: 40, label: 'Note', autofocus: true }, [], 'note'),
    box({ width: 100, height: 40, onClick: { kind: 'open' }, onContextMenu: null }, [], 'row'),
  ]);
  ctx.frame(320, 240, 1, tree);
  const noteKey = ctx.accessTree().nodes.find((n) => n.name === 'Note').key;
  assert.equal(ctx.focused(), noteKey);

  // A null tag declares the behaviour without a payload, as onKey does.
  ctx.cursor(20, 60);
  ctx.mouse(true, 1, 'secondary');
  assert.deepEqual(
    ctx.pollEvents().map((e) => e.payload),
    [{ kind: 'contextmenu', x: 20, y: 60 }],
  );
  ctx.mouse(false, 1, 'secondary');
  assert.equal(ctx.focused(), noteKey, 'the editor is still focused');

  // Where the primary button moves focus to the row it pressed.
  ctx.mouse(true, 1);
  ctx.mouse(false, 1);
  assert.notEqual(ctx.focused(), noteKey);
});

// F44: a single-line editor with `wrap` declared folds to its width the way
// a document does, and keeps a field's keyboard. The row is a shared text
// row, so the allow-list admits it on `<edit>` without a word.
test('an edit with wrap folds to its width, submits on Enter, and warns nothing', () => {
  const ctx = new Ctx();
  const seed = 'a considerably longer label than the box is wide';
  const view = (initial) =>
    box({}, [el('edit', { initial, size: 16, width: 209, wrap: 'word', label: 'Note', autofocus: true }, [], 'note')]);
  ctx.frame(320, 240, 1, view(seed));
  const note = () => ctx.accessTree().nodes.find((n) => n.name === 'Note');
  assert.equal(note().runs.length, 2, 'the draft folds onto two lines');
  assert.ok(note().rect.h > 30, `and the box is two lines tall: ${note().rect.h}`);
  assert.deepEqual(ctx.warnings().filter((w) => w.code === 'unknown-prop'), []);

  ctx.key('enter');
  const evs = ctx.pollEvents();
  assert.equal(evs.length, 1);
  assert.equal(evs[0].payload.kind, 'submit', 'Enter on a folded field submits');
  assert.equal(ctx.editText('note'), seed, 'and no newline went in');

  // Without the row the same field is one line, scrolled (F41).
  const plain = new Ctx();
  plain.frame(320, 240, 1, box({}, [el('edit', { initial: seed, size: 16, width: 209, label: 'Note' }, [], 'note')]));
  assert.equal(plain.accessTree().nodes.find((n) => n.name === 'Note').runs.length, 1);
});

test('a changed viewport emits one resize event', () => {
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, box({}));
  // The first frame establishes the viewport rather than resizing it.
  assert.deepEqual(ctx.pollEvents(), []);
  ctx.frame(640, 480, 2, box({}));
  assert.deepEqual(
    ctx.pollEvents().map((e) => e.payload),
    [{ kind: 'resize', width: 640, height: 480, scale: 2 }],
  );
  // Same viewport again: nothing.
  ctx.frame(640, 480, 2, box({}));
  assert.deepEqual(ctx.pollEvents(), []);
});

// Registered fonts: installed families by name, file bytes, and the `font`
// prop shaping through them.
test('fonts register by installed name or bytes and shape text', () => {
  const ctx = new Ctx();
  assert.equal(ctx.addSystemFont('kui-no-such-family-2026'), null);
  assert.throws(() => ctx.addFont(Buffer.alloc(64)), /no usable font face/);
  const families = ctx.systemFontFamilies();
  assert.ok(Array.isArray(families));
  if (families.length === 0) return; // a fontless machine
  const id = ctx.addSystemFont(families[0]);
  assert.match(id, /^[0-9a-f]{16}$/);
  const glyphs = (tree) => {
    ctx.frame(320, 240, 1, tree);
    const quads = Buffer.from(ctx.quads());
    const stride = quads.byteLength / ctx.stats().quadCount;
    let n = 0;
    for (let off = 0; off < quads.byteLength; off += stride) {
      if (quads.readUInt32LE(off + KIND_WORD * 4) !== 0) n++;
    }
    return n;
  };
  assert.ok(glyphs(box({}, [text('Fonts', { size: 20, font: id })])) > 0);
  assertLowers('font prop', () => box({}, [text('Fonts', { size: 20, font: id })]));
  ctx.removeFont(id);
  // A stale handle falls back to sans instead of failing.
  assert.ok(glyphs(box({}, [text('Fonts', { size: 20, font: id })])) > 0);
});

test('systemFonts() is the families with what their faces say they are (F97)', () => {
  const ctx = new Ctx();
  const fonts = ctx.systemFonts();
  assert.deepEqual(
    fonts.map((f) => f.family),
    ctx.systemFontFamilies(),
    'the same families, in the same order',
  );
  for (const f of fonts) {
    assert.deepEqual(Object.keys(f).sort(), ['family', 'italic', 'monospaced', 'weights']);
    assert.equal(typeof f.monospaced, 'boolean');
    assert.equal(typeof f.italic, 'boolean');
    assert.ok(f.weights.length > 0, `${f.family}: a face has a weight`);
    assert.deepEqual(f.weights, [...new Set(f.weights)].sort((a, b) => a - b), f.family);
  }
  // The platform's own, where they are installed: the Rust test pins the
  // reading on fixture faces that are everywhere.
  const byName = new Map(fonts.map((f) => [f.family, f]));
  for (const mono of ['Menlo', 'DejaVu Sans Mono', 'Consolas']) {
    if (byName.has(mono)) assert.ok(byName.get(mono).monospaced, `${mono} is monospaced`);
  }
  for (const prop of ['Helvetica', 'DejaVu Sans', 'Arial']) {
    if (byName.has(prop)) assert.ok(!byName.get(prop).monospaced, `${prop} is not`);
  }
});

test('a face whose glyphs cannot be measured is not a family to pick (F98)', () => {
  // macOS's GB18030 Bitmap has no head, hhea or hmtx: Han in mono fell
  // back to it and drew at infinity. The Rust tests pin the refusal on a
  // fixture face; here, the platform's own is not offered.
  const ctx = new Ctx();
  assert.ok(!ctx.systemFontFamilies().includes('GB18030 Bitmap'));
  assert.equal(ctx.addSystemFont('GB18030 Bitmap'), null);
});

test('font files and folders load by path', () => {
  const ctx = new Ctx();
  assert.throws(() => ctx.loadFontFile('/no/such/font.ttf'), /no usable font face/);
  assert.equal(ctx.loadFontsDir('/no/such/dir'), 0);
  const dirs = ['/usr/share/fonts/truetype/dejavu', '/System/Library/Fonts/Supplemental'];
  const dir = dirs.find((d) => existsSync(d));
  if (!dir) return;
  const n = ctx.loadFontsDir(dir);
  assert.ok(n > 0);
  const family = ctx.systemFontFamilies()[0];
  const id = ctx.addSystemFont(family);
  assert.equal(ctx.addSystemFont(family), id, 'idempotent per family');
});

test('a rescan of the system fonts finds nothing new when nothing was installed', () => {
  const ctx = new Ctx();
  const families = ctx.systemFontFamilies();
  assert.equal(ctx.reloadSystemFonts(), 0);
  assert.deepEqual(ctx.systemFontFamilies(), families);
});

test('the fallback fonts take ids, and an empty list is the platform\'s own', () => {
  const ctx = new Ctx();
  const id = ctx.addSystemFont(ctx.systemFontFamilies()[0]);
  ctx.setFallbackFonts([id]);
  ctx.setFallbackFonts([]);
  assert.throws(() => ctx.setFallbackFonts(['not an id']));
});

test('wrap, maxLines and ellipsis cut text instead of wrapping it', () => {
  const ctx = new Ctx();
  const LONG = 'A window title that is far too long to fit inside a narrow header strip';
  const glyphs = (props) => {
    ctx.frame(320, 240, 1, box({ width: 120 }, [text(LONG, props)]));
    const quads = Buffer.from(ctx.quads());
    const stride = quads.byteLength / ctx.stats().quadCount;
    let n = 0;
    let right = 0;
    for (let off = 0; off < quads.byteLength; off += stride) {
      if (quads.readUInt32LE(off + KIND_WORD * 4) === 0) continue;
      n++;
      right = Math.max(right, quads.readFloatLE(off) + quads.readFloatLE(off + 2 * 4));
    }
    return { n, right };
  };
  const wrapped = glyphs({});
  const nowrap = glyphs({ wrap: 'none' });
  const ellipsis = glyphs({ ellipsis: true });
  const clamped = glyphs({ maxLines: 2 });
  assert.ok(nowrap.n < wrapped.n, 'no-wrap emits only the glyphs inside the box');
  assert.ok(ellipsis.n < nowrap.n, 'ellipsis cuts the line short');
  assert.ok(ellipsis.right <= 120.5, 'the ellipsized line fits the box');
  assert.ok(clamped.n < wrapped.n && clamped.n > ellipsis.n, 'two lines sit between one and all');
  assertLowers('wrap props', () => box({ width: 120 }, [text(LONG, { wrap: 'none', maxLines: 2, ellipsis: true })]));
});

test('sounds: click/hover props, the audio element, tagged playbacks', () => {
  const ctx = new Ctx();
  assert.throws(() => ctx.addSound(Buffer.alloc(0)), /empty/);
  const snd = ctx.addSound(Buffer.from('RIFF....WAVE'));
  assert.match(snd, /^[0-9a-f]{16}$/);
  const view = (music) =>
    box({ pad: 8 }, [
      box({ width: 60, height: 20, bg: '#333333', clickSound: snd, onClick: { kind: 'go' } }, [], 'btn'),
      box({ width: 60, height: 20, bg: '#333333', hoverSound: snd }, [], 'hov'),
      ...(music ? [el('audio', { src: snd, loop: true, volume: 0.5, tag: { kind: 'music' } }, [], 'music')] : []),
    ]);
  // The audio node starts its playback when first declared.
  ctx.frame(320, 240, 1, view(true));
  let cmds = ctx.audioCommands();
  assert.equal(cmds.length, 1);
  assert.equal(cmds[0].kind, 'play');
  assert.equal(cmds[0].sound, snd);
  assert.equal(cmds[0].loop, true);
  assert.equal(cmds[0].volume, 0.5);
  const music = cmds[0].playback;
  // A click on a clickSound node plays it and still emits the click.
  ctx.cursor(20, 18);
  ctx.mouse(true);
  ctx.mouse(false);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [{ kind: 'go' }]);
  cmds = ctx.audioCommands();
  assert.equal(cmds.length, 1);
  assert.equal(cmds[0].kind, 'play');
  assert.equal(cmds[0].loop, false);
  // Entering the hoverSound node plays; moving inside it does not.
  ctx.cursor(20, 38);
  assert.equal(ctx.audioCommands().filter((c) => c.kind === 'play').length, 1);
  ctx.cursor(25, 40);
  assert.equal(ctx.audioCommands().length, 0);
  // Re-rendering keeps the music; dropping the node stops it.
  ctx.frame(320, 240, 1, view(true));
  assert.equal(ctx.audioCommands().length, 0);
  ctx.frame(320, 240, 1, view(false));
  assert.deepEqual(ctx.audioCommands(), [{ kind: 'stop', playback: music, fade: 0 }]);
  // play() with a tag reports ended through pollEvents.
  const p = ctx.play(snd, { volume: 0.75, fadeIn: 50, tag: { kind: 'chime' } });
  assert.deepEqual(ctx.audioCommands(), [{ kind: 'play', playback: p, sound: snd, volume: 0.75, loop: false, fadeIn: 50 }]);
  ctx.setVolume(p, 0.25, 100);
  ctx.pause(p);
  ctx.resume(p, 10);
  ctx.setMasterVolume(0.5);
  assert.deepEqual(
    ctx.audioCommands().map((c) => c.kind),
    ['setVolume', 'pause', 'resume', 'masterVolume'],
  );
  ctx.audioEnded(p);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [
    { kind: 'sound', phase: 'ended', playback: p, tag: { kind: 'chime' } },
  ]);
  ctx.removeSound(snd);
  assert.deepEqual(ctx.audioCommands(), [{ kind: 'unload', sound: snd }]);
});

// `finish` is a bit in the `audio` op's flags word, so the encoder is the
// only thing between `<audio finish>` and the core's release (backlog F29).
test('an <audio finish> node releases its playback when the view drops it', () => {
  const ctx = new Ctx();
  const snd = ctx.addSound(Buffer.from('RIFF....WAVE'));
  const view = (playing) =>
    box({ pad: 8 }, playing
      ? [
          el('audio', { src: snd, finish: true, tag: { kind: 'chime' } }, [], 'chime'),
          el('audio', { src: snd }, [], 'blip'),
          el('audio', { src: snd, loop: true, finish: true }, [], 'bed'),
        ]
      : []);
  ctx.frame(320, 240, 1, view(true));
  const [chime, blip, bed] = ctx.audioCommands().map((c) => c.playback);
  // Dropping all three: only the two that cannot play themselves out are
  // stopped — `blip` never asked, and `bed` is a loop with no end to reach.
  ctx.frame(320, 240, 1, view(false));
  assert.deepEqual(
    ctx.audioCommands().map((c) => [c.kind, c.playback]).sort((a, b) => a[1] - b[1]),
    [['stop', blip], ['stop', bed]],
  );
  // The released playback still reports `ended`, which is what the view
  // waits on instead of a guessed duration.
  ctx.audioEnded(chime);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [
    { kind: 'sound', phase: 'ended', playback: chime, tag: { kind: 'chime' } },
  ]);
});

// A driver with its own device owes two more answers than `ended` (backlog
// F36): a stop it found still playing, and a play its device refused. Both
// warnings are in `WarningCode`, and until these doors only the Rust runner
// could raise them.
test('a custom audio driver reports a truncation and a refusal through the same doors the runner uses', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  const snd = ctx.addSound(Buffer.from('RIFF....WAVE'));
  const view = (playing) =>
    box({ pad: 8 }, playing ? [el('audio', { src: snd, tag: { kind: 'jingle' } }, [], 'jingle')] : []);
  const codes = () => ctx.warnings().map((w) => [w.code, w.key]);

  // A one-shot node declared once and gone the next frame is a stop; the
  // warning waits for the device to say the sound was still running.
  ctx.frame(320, 240, 1, view(true));
  const [play] = ctx.audioCommands();
  assert.equal(play.kind, 'play');
  ctx.frame(320, 240, 1, view(false));
  assert.deepEqual(ctx.audioCommands().map((c) => c.kind), ['stop']);
  assert.deepEqual(codes(), []);
  ctx.audioTruncated(play.playback, 0.5);
  const truncated = codes();
  assert.equal(truncated.length, 1);
  const [code, jingle] = truncated[0];
  assert.equal(code, 'truncated-playback');
  assert.match(jingle, /^[0-9a-f]{16}$/, 'named on the audio node');
  // A second answer for the same playback is nothing: the entry is gone.
  ctx.audioTruncated(play.playback, 0.5);
  assert.deepEqual(codes(), []);

  // Declared again, and the device will not take it: the tag comes back
  // as a refused sound event, and the node is warned either way.
  ctx.frame(320, 240, 1, view(true));
  const [again] = ctx.audioCommands();
  ctx.audioRefused(again.playback);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [
    { kind: 'sound', phase: 'refused', playback: again.playback, tag: { kind: 'jingle' } },
  ]);
  assert.deepEqual(codes(), [['playback-refused', jingle]], 'the same node, redeclared under its label');
});

// ---------------------------------------------------------------------------
// The scene corpus (crates/kui-core/src/conformance.rs)
//
// Transport parity above proves the three JS encoders agree with each other.
// This proves they agree with the *reference*: kui-core builds the same
// named scenes natively, dumps a report, and every binding reproduces it.
// The dump is generated, not checked in — its quad digests cover real glyph
// geometry — so it is only compared when the reference file is there.

const CONFORMANCE =
  process.env.KUI_CONFORMANCE ?? fileURLToPath(new URL('../../target/conformance.txt', import.meta.url));

/** The corpus in JSX-object form, one builder per scene of `SCENES`.
 *  Every scene's top level is a root `<box>` sized exactly like the core's
 *  implicit root, so it configures the root without changing it — which is
 *  what leaves the tree below it identical to the reference's. */
/** `conformance::WRAP_BOXES`, as (width, height). */
const WRAP_BOXES = [[30, 12], [40, 16], [50, 20], [20, 24]];
/** `conformance::TABLE_ROWS`, as (label, fixed width, height). */
const TABLE_ROWS = [['abc', 30, 10], ['abcde', 50, 12], ['ab', 20, 8]];
/** `conformance::TAB_ROOMY` as (width, height) and `TAB_CROWDED` as widths. */
const TAB_ROOMY = [[30, 12], [50, 8]];
const TAB_CROWDED = [60, 70, 80, 90];

// `chrome` and `chrome-inset` are one tree driven under two envs, so the
// second is defined off the first below rather than restated.
const SCENE_TREES = {
  layout: () =>
    root({}, [
      box({ pad: 8, gap: 6, bg: '#14161e' }, [
        box(
          {
            dir: 'row',
            padL: 12, padR: 10, padT: 6, padB: 4,
            gap: 4,
            bg: '#202030',
            borderW: 2, borderColor: '#2a2d3a',
            radius: 5,
            opacity: 0.75,
            shadowColor: '#00000066', shadowBlur: 8, shadowY: 3, shadowSpread: 1,
            width: 180, height: 40,
          },
          [text('ab', { size: 12 }), text('cd', { size: 12 })],
          'card',
        ),
        box({ padX: 9, padY: 3, padB: 1, bg: '#2a2d3a' }),
        text(
          ['a ', el('span', { bold: true, color: '#73d98c' }, ['b']), el('span', { italic: true }, [' c']), el('span', { family: 'mono' }, [' d()']), el('span', { size: 20 }, [' E'])],
          { size: 13 },
        ),
      ]),
    ]),
  sizing: () =>
    root({}, [
      box({ padX: 14, padY: 6 }, [
        box({ dir: 'row', width: 200, height: 40, bg: '#101018' }, [
          box({ width: 30, height: 20, bg: '#30344a' }),
          box({ width: '25%', height: 20, bg: '#3b5bd4' }),
          box({ width: 'fit', height: 20, bg: '#73d98c' }, [
            box({ width: 20, height: 10, bg: '#ff0000' }),
          ]),
          box({ width: 'grow', height: 20, bg: '#ffcc00' }),
        ], 'bar'),
      ]),
    ]),
  wrap: () =>
    root({}, [
      box(
        { dir: 'row', wrapChildren: true, pad: 4, gap: 6, crossGap: 10, width: 100, bg: '#101018' },
        WRAP_BOXES.map(([w, h]) => box({ width: w, height: h, bg: '#30344a' })),
      ),
    ]),
  // `conformance::build_align`: the three spreads, a baseline row, and a
  // ratio sizing each axis.
  align: () => {
    const sq = () => box({ width: 10, height: 10, bg: '#30344a' });
    const spread = (mainAlign) => box({ dir: 'row', width: 120, mainAlign }, [sq(), sq(), sq()]);
    return root({}, [
      box({ pad: 4, gap: 6, width: 128, bg: '#101018' }, [
        spread('spaceBetween'),
        spread('spaceAround'),
        spread('spaceEvenly'),
        box({ dir: 'row', gap: 4, crossAlign: 'baseline' }, [
          text('ab', { size: 12 }),
          text('cd', { size: 20 }),
          sq(),
        ]),
        box({ width: 'grow', aspectRatio: 4, bg: '#3b5bd4' }),
        box({ height: 12, aspectRatio: 2, bg: '#73d98c' }),
      ]),
    ]);
  },
  // `conformance::build_stock_controls` (ADR 0034).
  'stock-controls': () =>
    root({}, [
      box({ pad: 8, gap: 8 }, [
        el('slider', {
          label: 'Volume', width: 216, valueNow: 30, valueMin: 0, valueMax: 100, valueStep: 10,
          onChange: { kind: 'vol' },
        }),
        el('checkbox', { onClick: { kind: 'mute' } }, ['Mute']),
        el('checkbox', { checked: true, onClick: { kind: 'sync' } }, ['Sync']),
        el('checkbox', { mixed: true, onClick: { kind: 'all' } }, ['All']),
        el('radioGroup', { label: 'Theme' }, [
          el('radio', { onClick: { kind: 'light' } }, ['Light']),
          el('radio', { checked: true, onClick: { kind: 'dark' } }, ['Dark']),
        ]),
        el('switch', { checked: true, onClick: { kind: 'wifi' } }, ['Wi-Fi']),
      ]),
    ]),
  // `conformance::build_table` (ADR 0033): a fit header row of two bare
  // texts, then TABLE_ROWS as grow rows of a bare text, a fixed box and a
  // grow box; the label column is its longest label, the fixed column its
  // widest cell, the grow column the rest.
  table: () =>
    root({}, [
      box(
        { dir: 'table', width: 200, pad: 4, gap: 2, bg: '#101018', rules: '#2b3350', ruleWidth: 1 },
        [
          box({ dir: 'row', gap: 6 }, [text('name', { size: 12 }), text('w', { size: 12 })]),
          ...TABLE_ROWS.map(([label, w, h]) =>
            box({ dir: 'row', width: 'grow', gap: 6 }, [
              text(label, { size: 12 }),
              box({ width: w, height: h, bg: '#30344a' }),
              box({ width: 'grow', height: h, bg: '#3b5bd4' }),
            ]),
          ),
        ],
        'table',
      ),
    ]),
  tabs: () =>
    root({}, [
      box({ pad: 4, gap: 4 }, [
        box(
          { dir: 'row', width: 200, height: 20, bg: '#101018' },
          TAB_ROOMY.map(([w, h]) =>
            box({ width: 'grow', minWidth: 'fit', height: '50%', minHeight: 'fit', bg: '#30344a' }, [
              box({ width: w, height: h, bg: '#3b5bd4' }),
            ]),
          ),
          'roomy',
        ),
        box(
          { dir: 'row', width: 200, height: 20, scrollX: true, bg: '#101018' },
          TAB_CROWDED.map((w) =>
            box({ width: 'grow', minWidth: 'fit', height: 'grow', bg: '#30344a' }, [
              box({ width: w, height: 12, bg: '#3b5bd4' }),
            ]),
          ),
          'crowded',
        ),
      ]),
    ]),
  overflow: () =>
    root({}, [
      box({ pad: 4, clip: true }, [
        box(
          { width: 120, height: 60, gap: 4, scrollY: true, radius: 8, bg: '#101018' },
          ITEM_KEYS.map((k) => box({ width: 100, height: 20, bg: '#30344a' }, [], k)),
          'list',
        ),
      ]),
    ]),
  // `conformance::build_scrollbar`: the four scrollbar rows on three
  // scrollers of the same list — hidden, styled, auto.
  scrollbar: () => {
    const list = (key, extra) =>
      box(
        { width: 90, height: 60, gap: 0, scrollY: true, bg: '#101018', ...extra },
        ITEM_KEYS.map((k) => box({ width: 80, height: 20, bg: '#30344a' }, [], k)),
        key,
      );
    return root({}, [
      box({ dir: 'row', pad: 10, gap: 10 }, [
        list('hidden', { scrollbar: 'hidden' }),
        list('styled', { scrollbarWidth: 8, scrollbarColor: '#3b5bd4', scrollbarActiveColor: '#ffcc00' }),
        list('auto', { scrollbar: 'auto' }),
      ]),
    ]);
  },
  // `conformance::build_tokens` (ADR 0027): the table declared on the
  // context every build — the Rust reference declares every frame too —
  // and every value a `$name`: the app's, the two roles, and `$nothing`.
  tokens: (_fx, _phase, ctx) => {
    ctx.setTokens({
      colors: Object.fromEntries([
        ...TOKEN_COLORS.map(([n, l, d]) => [n, { light: l, dark: d }]),
        ...TOKEN_DERIVED.map(([n, from, ops]) => [n, { from, ops }]),
      ]),
      lengths: Object.fromEntries(TOKEN_LENGTHS),
    });
    const cell = (key, extra) => box({ width: '$side_w', height: 30, ...extra }, [], key);
    return root({}, [
      box({ dir: 'row', padL: '$gap', padR: 10, padT: 10, padB: 10, gap: '$gap' }, [
        cell(TOKEN_KEYS[0], { bg: '$peach' }),
        cell(TOKEN_KEYS[1], { bg: '$ink', borderW: '$gap', borderColor: '$peach' }),
        cell(TOKEN_KEYS[2], { bg: '$surface', radius: '$radius' }),
        cell(TOKEN_KEYS[3], { bg: '$nothing' }),
        ...TOKEN_KEYS.slice(4).map((k) => cell(k, { bg: "$" + k })),
        el('text', { size: '$big', color: '$peach' }, ['tokens', el('span', { color: '$ink' }, ['x'])]),
      ]),
    ]);
  },
  // Scroll anchoring (backlog C26 step 3): two scrollers of the same rows,
  // one with `anchor`; phase 1 prepends a taller row to both.
  anchor: (_fx, phase) => {
    const list = (key, extra) =>
      box(
        { width: 90, height: 60, scrollY: true, bg: '#101018', ...extra },
        [
          ...(phase >= 1 ? [box({ width: 80, height: 30, bg: '#30344a' }, [], 'new')] : []),
          ...ITEM_KEYS.map((k) => box({ width: 80, height: 20, bg: '#30344a' }, [], k)),
        ],
        key,
      );
    return root({}, [
      box({ dir: 'row', pad: 10, gap: 10 }, [
        list('anchored', { anchor: true }),
        list('plain', {}),
      ]),
    ]);
  },
  float: () =>
    root({}, [
      box({ pad: 20, gap: 4 }, [
        box({ width: 80, height: 24, bg: '#333333' }, [
          box({ float: 'below', width: 40, height: 12, bg: '#ff0000' }),
        ], 'anchor'),
        box({ width: 60, height: 20, bg: '#444444' }, [
          box({ float: { anchor: 'below', dx: 6 }, width: 30, height: 10, bg: '#0000ff' }),
        ], 'nudged'),
        box({
          float: { anchor: 'viewport', at: ['start', 'end'], self: ['end', 'start'], dx: -6, dy: 14, fit: true },
          width: 10, height: 10, bg: '#00ff00',
        }),
      ]),
    ]),
  // A float that takes its parent's clip (backlog F90): two nodes on a
  // `clip` canvas panned half past its top, the first declaring `clip`.
  'clip-float': () => {
    const node = (key, dx, clip, bg, label) =>
      box(
        {
          float: { anchor: 'parent', dx, dy: -20, clip },
          width: 80, height: 40, bg, onClick: { kind: key }, label,
        },
        [],
        key,
      );
    return root({}, [box({ width: 'grow', height: 'grow' }, [
      box(
        { dir: 'row', width: 'grow', height: 40, bg: '#3a3f52', onClick: { kind: 'toolbar' }, label: 'Toolbar' },
        [],
        'toolbar',
      ),
      box(
        { width: 'grow', height: 'grow', clip: true, bg: '#101018' },
        [node('node', 40, true, '#3b5bd4', 'Node'), node('free', 160, false, '#73d98c', 'Free')],
        'canvas',
      ),
    ])]);
  },
  // Boxes on whole pixels (`pixelSnap`): two snapped, the first with a
  // hard shadow, and one drawn where layout put it.
  'pixel-snap': () => {
    const cell = (bg, extra = {}) => box({ width: 40.5, height: 20.25, bg, ...extra });
    return root({}, [box({ dir: 'row' }, [
      cell('#d9738c', { pixelSnap: true, shadowColor: '#000000' }),
      cell('#73d98c', { pixelSnap: true }),
      cell('#3b5bd4'),
    ])]);
  },
  // ADR 0043: a rounded, clipping card tilted an eighth of a turn about
  // its top-left corner, holding a child wider than itself and a badge
  // floating with it; beside it a box scaled half again.
  'transform': () => root({}, [box({ dir: 'row' }, [
    box({ key: 'card', width: 60, height: 40, radius: 8, clip: true, bg: '#3b5bd4', rotate: 0.125, pivotX: 0, pivotY: 0 }, [
      box({ key: 'wide', width: 90, height: 20, bg: '#d9738c' }),
      box({ key: 'badge', width: 12, height: 12, bg: '#f6d55c', float: { anchor: 'parent', dx: 54, dy: -6 } }),
    ]),
    box({ key: 'big', width: 40, height: 40, bg: '#73d98c', scale: 1.5 }),
  ])]),
  // Rounded span backgrounds joined across four texts (backlog F101), the
  // fourth in another colour.
  'joined-backgrounds': () => {
    const mono = { size: 14, family: 'mono', lineHeight: 20 };
    const sel = (s) => el('span', { bg: '#3b5bd466', bgRadius: 4 }, [s]);
    return root({}, [box({ pad: 10 }, [
      text(['let ', sel('a = 1;')], mono),
      text([sel('let b = 22;')], mono),
      text([sel('c'), ' + d'], mono),
      text([el('span', { bg: '#d9738c66', bgRadius: 4 }, ['find'])], mono),
    ])]);
  },
  // Whitespace that takes its room (backlog F106): `ab  c` in a box 4 px
  // wide, each glyph a row of its own, the spaces too.
  'break-spaces': () => {
    const mono = { size: 14, family: 'mono', lineHeight: 20, wrap: 'break-spaces' };
    return root({}, [box({ pad: 10 }, [
      box({ width: 4 }, [
        text(['ab', el('span', { bg: '#3b5bd4' }, ['  ']), 'c'], mono),
      ]),
    ])]);
  },
  // ADR 0042: a strip, a square over a bg and under a border, a radial.
  gradients: () => {
    const band = (props) => box({ width: 200, height: 40, ...props });
    return root({}, [box({ pad: 10, gap: 6 }, [
      band({ radius: 8, gradient: { to: 'right', stops: ['#7f9cf5', '#e07a8a'] } }),
      band({ bg: '#14161e', borderW: 2, borderColor: '#ffffff', gradient: { angle: 0.125, stops: ['#f5d67f', ['#00000000', 0.8]] } }),
      band({ gradient: { radial: true, at: [0.5, 0], stops: ['#9ad9a0', '#14161e'] } }),
    ])]);
  },
  // Size expressions (backlog F109): four bars in a 400 px column, two
  // widths spelled and two as data, which ride as numbers.
  'size-expressions': () => {
    const bar = (props) => box({ height: 10, bg: '#3b5bd4', ...props });
    return root({}, [box({ width: 400, gap: 4 }, [
      bar({ width: 'clamp(100px, 50%, 150px)' }),
      bar({ width: { min: [{ percent: 80 }, 300] } }),
      bar({ width: 900, maxWidth: '25%' }),
      bar({ minWidth: { max: ['40%', 50] } }),
    ])]);
  },
  // Shares that give (backlog F110): two halves and a gap, two clamps and
  // a gap, each pair fitting its row.
  'relative-shrink': () => {
    const bar = (width) => box({ width, height: 10, bg: '#3b5bd4' });
    return root({}, [box({ gap: 4 }, [
      box({ dir: 'row', width: 200, gap: 20 }, [bar('50%'), bar('50%')]),
      box({ dir: 'row', width: 300, gap: 20 }, [
        bar('clamp(100px, 60%, 400px)'),
        bar({ clamp: [100, { percent: 60 }, 400] }),
      ]),
    ])]);
  },
  // A column of fit children gives none less than its content (backlog
  // F114): three rows of a 20 px bar in 50 px at 0, 20 and 40; a clip
  // under a row giving to the 30 px left; a wrapping row's two lines kept.
  'column-squeeze': () => {
    const bar = () => box({ width: 100, height: 20, bg: '#3b5bd4' });
    const chip = () => box({ width: 60, height: 20, bg: '#73d98c' });
    return root({}, [box({ dir: 'row', gap: 20 }, [
      box({ width: 100, height: 50 }, [0, 1, 2].map(() => box({ dir: 'row' }, [bar()]))),
      box({ width: 100, height: 50 }, [
        box({ dir: 'row' }, [bar()]),
        box({ clip: true }, [bar(), bar()]),
      ]),
      box({ width: 100, height: 50 }, [
        box({ dir: 'row', width: 'grow', wrapChildren: true, gap: 10, crossGap: 5 }, [chip(), chip()]),
        box({ dir: 'row' }, [bar()]),
      ]),
    ])]);
  },
  // A fit box across a column is no wider than its box (backlog F116): a
  // fit column in a card capped at 100 held to it, its wrapping row of
  // three 40 px chips on two lines; beside it, `minWidth: 'fit'`, 140.
  'fit-across': () => {
    const chip = () => box({ width: 40, height: 20, bg: '#73d98c' });
    const card = (wrapper) => box({ maxWidth: 100 }, [
      box({ ...wrapper, bg: '#30344a' }, [
        box({ dir: 'row', wrapChildren: true, gap: 10, crossGap: 5 }, [chip(), chip(), chip()]),
      ]),
    ]);
    return root({}, [box({ dir: 'row', gap: 20 }, [card({}), card({ minWidth: 'fit' })])]);
  },
  // Access rects cut to the clip (backlog F93): three nodes on a `clip`
  // canvas past its top, three rows in a short scroller.
  'clip-access': () => {
    const button = (key, label, width, height, bg, extra = {}) =>
      box({ width, height, bg, onClick: { kind: key }, label, ...extra }, [], key);
    const node = (key, label, dx, dy, clip) =>
      button(key, label, 60, 40, '#3b5bd4', { float: { anchor: 'parent', dx, dy, clip } });
    return root({}, [box({ width: 'grow', height: 'grow' }, [
      button('toolbar', 'Toolbar', 'grow', 40, '#3a3f52'),
      box({ dir: 'row', width: 'grow', height: 'grow' }, [
        box(
          { width: 'grow', height: 'grow', clip: true, bg: '#101018' },
          [node('cut', 'Cut', 20, -20, true), node('past', 'Past', 100, -60, true), node('free', 'Free', 140, -60, false)],
          'canvas',
        ),
        box(
          { width: 100, height: 50, scrollY: true, bg: '#202030' },
          [['row0', 'Row 0'], ['row1', 'Row 1'], ['row2', 'Row 2']].map(([k, l]) => button(k, l, 100, 30, '#73d98c')),
          'list',
        ),
      ]),
    ])]);
  },
  tooltip: () =>
    root({}, [
      box({ pad: 10 }, [
        box(
          { dir: 'row', width: 100, height: 40, bg: '#333333', role: 'group', tooltip: 'a hint' },
          [text('badge', { size: 12 })],
          'tip',
        ),
        box({
          dir: 'row', width: 100, height: 20, role: 'button', label: 'Save',
          description: 'Nothing to save yet',
        }),
      ]),
    ]),
  chrome: () =>
    root({ title: 'kui conformance', alwaysOnTop: true, secureInput: true, optionAsAlt: 'left', imeOff: true }, [
      box({ gap: 6 }, [
        // `<titlebar>` appends its own cluster; the second one goes through
        // the `<windowButtons>` element, in a strip laid out by hand.
        el('titlebar', {}, [text('app', { size: 12 })]),
        box({ dir: 'row', width: 'grow', keepFocus: true }, [el('windowButtons')]),
        box({ width: 40, height: 16, bg: '#22242c', focusable: true, keyFocus: true, onFocus: { kind: 'sink' }, label: 'Sink' }, [], 'sink'),
      ]),
    ]),
  controls: () =>
    root({}, [
      box({ pad: 10, gap: 6, onContextMenu: { kind: 'menu' }, onButton: { kind: 'panel' }, buttons: 'middle' }, [
        el('button', { onClick: { kind: 'go' }, description: 'Starts the run' }, ['go']),
        el('button', { onClick: { kind: 'stop' }, label: 'Stop the run', disabled: true, tooltip: 'Nothing is running' }, ['stop']),
        el('edit', { initial: 'hello, on two lines in a narrow field', size: 13, width: 160, wrap: 'word', label: 'Note' }, [], 'note'),
        box(
          {
            width: 120,
            height: 12,
            role: 'slider',
            label: 'Focus length',
            valueNow: 25,
            valueMin: 5,
            valueMax: 60,
            valueText: '25 minutes',
          },
          [],
          'focus',
        ),
      ]),
    ]),
  // Two key sinks: the press-only default and one that asked for releases
  // (`keyUp`). The tag is an integer, so the report's event column shows
  // the phase instead of a tag kind. Then a third with a button inside it,
  // which holds focus: a shell over a ring, hearing what the button does
  // not claim (docs/adr/0011-keys-bubble-to-the-enclosing-sink.md).
  keys: () =>
    root({}, [
      box({ pad: 10, gap: 6 }, [
        box({ width: 100, height: 24, bg: '#1b1d27', onKey: 1, role: 'group', label: 'press' }, [], 'press'),
        box({ width: 100, height: 24, bg: '#1b1d27', onKey: 1, keyUp: true, role: 'group', label: 'held' }, [], 'held'),
        box({ width: 100, height: 24, bg: '#1b1d27', onKey: 1, keyUp: true, role: 'group', label: 'shell' }, [
          box({ width: 80, height: 16, bg: '#3b5bd4', onClick: { kind: 'go' }, label: 'Go', keyFocus: true }, [], 'go'),
        ], 'shell'),
      ]),
    ]),
  // Two sinks asking for releases, tagged by kind so the report tells them
  // apart; the second asks for the modifier keys (backlog F108).
  'modifier-keys': () =>
    root({}, [
      box({ pad: 10, gap: 6 }, [
        box({ width: 100, height: 24, bg: '#1b1d27', onKey: { kind: 'plain' }, keyUp: true, role: 'group', label: 'plain' }, [], 'plain'),
        box({ width: 100, height: 24, bg: '#1b1d27', onKey: { kind: 'mods' }, keyUp: true, modifierKeys: true, role: 'group', label: 'mods' }, [], 'mods'),
      ]),
    ]),
  ime: () =>
    root({}, [
      box({ pad: 10, gap: 6 }, [
        box({ width: 200, height: 24, bg: '#1b1d27', onKey: { kind: 'ed' }, role: 'multilineTextInput', label: 'Buffer' }, [
          box({ dir: 'row', height: 20, role: 'line', caret: 1 }, [text('ab', { size: 13, family: 'mono' })], 'l0'),
        ], 'buffer'),
        el('edit', { initial: '', width: 200, size: 13, label: 'Note' }, [], 'note'),
      ]),
    ]),
  // The ime scene's two editors, answered with pastes the pasteboard
  // marked (backlog F84).
  paste: () => SCENE_TREES.ime(),
  cells: () => {
    const screen = new Uint32Array(11 * 4);
    'hello world'.split('').forEach((ch, i) => {
      screen[i * 4] = ch.codePointAt(0);
      screen[i * 4 + 1] = 0xd6d8e0ff;
      screen[i * 4 + 2] = i < 3 ? 0x1a1d27ff : 0;
    });
    return root({}, [
      box({ pad: 10 }, [
        el('cells', {
          rows: 1, cols: 11, cells: screen, size: 13, family: 'mono', lineHeight: 18,
          cursorAt: [0, 3], cursorShape: 'block', cursorColor: '#6a8bff',
          onClick: { kind: 'hit' }, label: 'term',
        }, [], 'term'),
      ]),
    ]);
  },
  // docs/adr/0003-modal-surfaces.md: the app behind the dialog is inert,
  // the titlebar is not, and both dismiss gestures reach the dialog. Then
  // the way out (backlog F4): the app declares `open` focused while the
  // dialog is shut and the freshly created `note` on the frame that drops
  // it, and that change of declaration is what the restore yields to.
  modal: (_fx, phase) =>
    root({}, [
      box({ width: 'grow', gap: 6 }, [
        el('titlebar', {}, [text('app', { size: 12 })]),
        box(
          {
            dir: 'row', width: 100, height: 20, bg: '#30344a',
            onClick: { kind: 'open' }, label: 'Open',
            keyFocus: phase === 0,
          },
          [],
          'open',
        ),
        phase !== 0 && box(
          {
            dir: 'row', width: 100, height: 20, bg: '#30344a',
            onClick: { kind: 'note' }, label: 'Note',
            keyFocus: true,
          },
          [],
          'note',
        ),
        phase === 0 && box(
          {
            width: 120, height: 100, pad: 8, gap: 6, bg: '#202030',
            float: { anchor: 'viewport', at: ['end', 'end'], self: ['end', 'end'] },
            modal: { kind: 'dlg' },
            label: 'Settings',
          },
          [
            box(
              { dir: 'row', width: 100, height: 24, bg: '#3b5bd4', onClick: { kind: 'ok' }, label: 'OK' },
              [],
              'ok',
            ),
            box(
              { dir: 'row', width: 100, height: 24, bg: '#3b5bd4', onClick: { kind: 'cancel' }, label: 'Cancel' },
              [],
              'cancel',
            ),
          ],
          'dialog',
        ),
      ].filter(Boolean)),
    ]),
  // docs/adr/0007-composite-keyboard-patterns.md: a tab bar and a picker
  // list, each one Tab stop because its items are focusable, with an
  // ordinary button between them that keeps a stop of its own. Nothing
  // says "composite" — the core derives it from the roles.
  composite: () => {
    const tab = (name, kind, selected) =>
      box(
        {
          dir: 'row', role: 'tab', selected,
          width: 60, height: 20, bg: '#30344a',
          onClick: { kind },
        },
        [text(name, { size: 12 })],
        name,
      );
    // `focusable` on the row is what makes the list a composite: a
    // navigation list holds links, a picker holds rows.
    const pick = (name, kind) =>
      box(
        {
          dir: 'row', role: 'listItem', focusable: true,
          width: 80, height: 18, bg: '#202030',
          onClick: { kind },
        },
        [text(name, { size: 12 })],
        name,
      );
    return root({}, [
      box({ width: 'grow', gap: 6 }, [
        box({ dir: 'row', role: 'tabList', gap: 4 }, [
          tab('One', 'one', false),
          tab('Two', 'two', true),
          tab('Three', 'three', false),
        ], 'tabs'),
        box(
          { dir: 'row', width: 40, height: 20, bg: '#3b5bd4', onClick: { kind: 'add' }, label: 'Add' },
          [],
          'add',
        ),
        box({ role: 'list', gap: 2 }, [pick('Alpha', 'alpha'), pick('Bravo', 'bravo')], 'rows'),
      ]),
    ]);
  },
  // docs/adr/0015-a-fragment-element-and-the-painter-it-is-not.md: four
  // fragments — plain, keyed with a child over it, a dead handle that draws
  // nothing, and one with eighteen params so the truncation warning fires
  // — then the image input (backlog V1): the sampling fixture over the
  // atlas-backed icon, over the texture-backed stream, and over the removed
  // fixture (`dead`; 0 would mean "no image"), which draws nothing.
  fragments: (fx) =>
    root({}, [
      box({ width: 200, height: 120, gap: 4, bg: '#14161e' }, [
        el('fragment', { src: fx().fragment, params: FRAGMENT_PARAMS, width: 80, height: 40 }),
        el(
          'fragment',
          { src: fx().fragment, params: FRAGMENT_PARAMS, width: 80, height: 40, pad: 6, radius: 8, opacity: 0.5 },
          [box({ width: 20, height: 10, bg: '#202030' })],
          'card',
        ),
        el('fragment', { src: '0000000000000000', params: FRAGMENT_PARAMS, width: 20, height: 10 }),
        el('fragment', { src: fx().fragment, params: FRAGMENT_PARAMS_LONG, width: 30, height: 12 }),
        box({ dir: 'row', gap: 4 }, [
          el('fragment', { src: fx().sampler, image: fx().image, params: FRAGMENT_IMAGE_PARAMS, width: 24, height: 24 }),
          el('fragment', { src: fx().sampler, image: fx().stream, params: FRAGMENT_IMAGE_PARAMS, width: 32, height: 8 }),
          el('fragment', { src: fx().sampler, image: fx().dead, params: FRAGMENT_IMAGE_PARAMS, width: 24, height: 24 }),
        ]),
      ]),
    ]),
  // docs/adr/0025-the-image-is-the-canvas.md, decision 6: five fills — the
  // triangle takes a click, hit by its outline (ADR 0026), the star is
  // keyed, the nine-point outline loses its ninth with a warning, the quad
  // fades.
  polygon: () =>
    root({}, [
      box({ width: 200, height: 120, bg: '#14161e' }, [
        el('polygon', { points: [[10, 10], [60, 20], [20, 50]], bg: '#7f9cf5', onClick: { kind: 'tri' }, label: 'Triangle' }),
        el('polygon', { points: [[80, 10], [130, 30], [80, 50], [95, 30]], bg: '#d8863b' }),
        el('polygon', { points: [[170, 10], [176, 24], [190, 30], [176, 36], [170, 50], [164, 36], [150, 30], [164, 24]], bg: '#f5d67f' }, [], 'star'),
        el('polygon', { points: [[10, 70], [30, 65], [50, 70], [70, 65], [90, 70], [90, 110], [50, 100], [10, 110], [5, 90]], bg: '#9ad9a0' }),
        el('polygon', { points: [[110, 70], [190, 70], [180, 110], [120, 110]], bg: '#e07a8a', opacity: 0.5 }),
      ]),
    ]),
  // docs/adr/0040-a-path-is-a-mask-in-the-atlas.md: seven paths; the first
  // wedge takes a click, hit by its arc, the second is keyed, the ring is
  // the flat op form, the cubic is a stroke alone, the triangle is both and
  // faded, and the last does not parse. Data is `conformance::PATH_*`.
  path: () =>
    root({}, [
      box({ width: 200, height: 120, bg: '#14161e' }, [
        el('path', { d: 'M60 60 L100 60 A40 40 0 0 1 60 100 Z', bg: '#7f9cf5', onClick: { kind: 'wedge' }, label: 'Wedge' }),
        el('path', { d: 'M60 60 L60 100 A40 40 0 0 1 20 60 Z', bg: '#d8863b', tooltip: 'The other quarter' }, [], 'wedge2'),
        el('path', { d: [0, 120, 10, 1, 190, 10, 1, 190, 80, 1, 120, 80, 5, 0, 140, 30, 1, 170, 30, 1, 170, 60, 1, 140, 60, 5], bg: '#f5d67f', fillRule: 'evenodd' }),
        el('path', { d: 'M110 90 C130 70 150 110 190 90', width: 2, color: '#9ad9a0', dash: [8, 4], dashOffset: 3 }),
        el('path', { d: 'M20 10 L50 10 L35 40 Z', bg: '#e07a8a', width: 1.5, color: '#ffffff', opacity: 0.5 }),
        el('path', { d: 'M30 104 H50 V110 H30 Z', bg: '#7fd6f5', rotate: 0.125, pivot: [40, 107] }),
        el('path', { d: 'M10 10 L20', bg: '#ffffff' }, [], 'bad'),
      ]),
    ]),
  // docs/adr/0010-a-segment-primitive.md: three strokes and a box; the
  // elbow takes a click, hit by its stroke (ADR 0026).
  underlines: () => {
    // Backlog K4: a wave in red under a span, a green solid line through
    // the style rows, dots in their own colour, and an undercurl over
    // three cells carrying the wave bit and a red fifth entry.
    const mono = { size: 14, family: 'mono', lineHeight: 20 };
    const screen = new Uint32Array(3 * 5);
    'abc'.split('').forEach((ch, i) => {
      screen[i * 5] = ch.codePointAt(0);
      screen[i * 5 + 1] = 0xd6d8e0ff;
      screen[i * 5 + 3] = 32;
      screen[i * 5 + 4] = 0xff0000ff;
    });
    return root({}, [
      box({ pad: 10, gap: 4 }, [
        text(['let ', el('span', { underlineColor: '#ff0000', underlineStyle: 'wavy' }, ['value'])], mono),
        text('warn', { ...mono, underlineColor: '#00ff00' }),
        text('dots', { ...mono, underlineColor: '#7f9cf5', underlineStyle: 'dotted' }),
        el('cells', { rows: 1, cols: 3, cells: screen, ...mono, label: 'term' }, [], 'term'),
      ]),
    ]);
  },
  lines: () =>
    root({}, [
      box({ width: 200, height: 120, bg: '#14161e' }, [
        el('line', { from: [10, 10], to: [90, 70], width: 2, color: '#7f9cf5' }),
        el('line', { points: [[100, 20], [140, 20], [140, 60]], width: 3, color: '#d8863b', onClick: { kind: 'elbow' }, label: 'Elbow' }),
        el('line', { points: [[20, 100], [60, 80], [100, 110], [180, 90]], curve: true, width: 1.5, color: '#9ad9a0', opacity: 0.5 }, [], 'curve'),
        el('line', { points: [[150, 70], [190, 70], [190, 110]], width: 2, color: '#e07a8a', dash: [10, 4, 2, 4], dashOffset: 3 }),
        box({ width: 40, height: 20, bg: '#202030' }),
      ]),
    ]),
  media: (fx, phase) =>
    root({}, [
      box({ pad: 6, gap: 4 }, [
        el('image', { src: fx().image, width: 16, radius: 2 }),
        // ADR 0025: the icon as `contain` in a box twice its aspect, then
        // the stream fixture plain, `nearest`, and `cover` in a square box.
        el('image', { src: fx().image, width: 32, height: 16, fit: 'contain', label: 'Icon' }),
        el('image', { src: fx().stream, width: 16, label: 'Stream' }),
        el('image', { src: fx().stream, width: 16, sampling: 'nearest', label: 'Crisp' }),
        el('image', { src: fx().stream, width: 12, height: 12, fit: 'cover', label: 'Cropped' }),
        el('audio', { src: fx().sound, volume: 0.5, loop: true }, [], 'music'),
        el('latencyGraph'),
        // The two phase 1 drops: `chime` asked to finish, so its removal
        // releases the playback and no `stop` reaches the driver; `blip`
        // did not, and is stopped.
        ...(phase === 0
          ? [
              el('audio', { src: fx().sound, finish: true }, [], 'chime'),
              el('audio', { src: fx().sound }, [], 'blip'),
            ]
          : []),
      ]),
    ]),
  // docs/adr/0005-the-paint-vocabulary.md: three subtrees the view stops
  // declaring in phase 1 — `fade` still in flight at the end, `blink`
  // already over, `flash` back in phase 2 while its own exit runs — then
  // `bulk`, one node past the budget, in phase 3 and 4200 one-node rows in
  // phase 4, each frame refused whole (docs/adr/0012-the-exit-budget.md);
  // and two that never leave, so two Tabs say whether the ring has a place
  // for a ghost.
  exit: (_fx, phase) =>
    root({}, [
      box({ width: 'grow', height: 'grow', pad: 8, gap: 6, bg: '#14161e' }, [
        keep('a', 'A'),
        slot('slotFade', 40, phase === 0 && box(
          {
            width: 100, height: 24, bg: '#3b5bd4',
            transition: 400, exit: { dx: 40, opacity: 0 },
            focusable: true, label: 'Fade', onClick: { kind: 'hit' },
          },
          [text('bye', { size: 12 })],
          'fade',
        )),
        slot('slotBlink', 16, phase === 0 && box(
          { width: 100, height: 12, bg: '#73d98c', transition: 50, exit: { dx: 20 } },
          [],
          'blink',
        )),
        slot('slotFlash', 16, phase !== 1 && box(
          { width: 100, height: 12, bg: '#ffcc00', transition: 400, exit: { dx: -20 } },
          [],
          'flash',
        )),
        keep('b', 'B'),
        // More one-node departures than the budget, each a solid quad half
        // a pixel wide, in a slot that keeps its size when they go.
        box(
          { dir: 'row', width: 300, height: 4, bg: '#101018' },
          phase < 4
            ? Array.from({ length: EXIT_ROWS }, () => box({
                width: 0.5, height: 4, bg: '#8a8fa3', transition: 400, exit: { opacity: 0 },
              }))
            : [],
          'slotRows',
        ),
        // Last, and sized by children that have no size: dropping it takes
        // only the trailing gap with it.
        phase < 3 && box(
          { transition: 400, exit: { opacity: 0 } },
          Array.from({ length: EXIT_BULK_ROWS }, () => box({})),
          'bulk',
        ),
      ].filter(Boolean)),
    ]),
};

// The traffic-lights half: the same tree, driven under a custom chrome that
// also reports the OS controls. Both button clusters go away and the title
// insets past them — the env is the only difference, which is the point.
SCENE_TREES['chrome-inset'] = SCENE_TREES.chrome;

// `conformance::build_windows`: the declaration rides on the root box, the
// way `title` does, and comes and goes with the phase — twice in phase 0,
// disagreeing on the size.
SCENE_TREES.windows = (_fx, phase) =>
  root(
    {
      windows:
        phase === 0
          ? [{ name: 'palette', width: 400, height: 300 }, { name: 'palette', width: 500, height: 500 }]
          : phase === 2
            ? [{ name: 'palette', width: 400, height: 300 }]
            : undefined,
    },
    [box({ pad: 8, bg: '#14161e' }, [text(phase === 0 || phase === 2 ? 'open' : 'closed', { size: 12 })])],
  );

// `conformance::build_popup`: one declaration, with the kind and the anchor
// a menu carries. The dismissals are steps, not anything the tree says.
SCENE_TREES.popup = (_fx, phase) =>
  root(
    {
      windows:
        phase === 0
          ? [
              {
                name: 'menu',
                kind: 'popup',
                width: 160,
                height: 320,
                anchor: { x: 12, y: 40, w: 160, h: 24 },
              },
            ]
          : undefined,
    },
    [box({ pad: 8, bg: '#14161e' }, [text(phase === 0 ? 'menu' : 'closed', { size: 12 })])],
  );

// `conformance::build_select`: the stock select over four options, the
// fourth a menu-item object posting an `id` and disabled, the second in
// force. The steps open its menu, choose a row and open it again; the
// report keeps the field and the menu.
SCENE_TREES.select = () =>
  root({}, [
    box({ pad: 10, gap: 6 }, [
      el('select', {
        label: 'language',
        options: ['English', 'Deutsch', 'Français', { label: 'Latin', id: 'la', enabled: false }],
        current: 1,
      }),
      text('body', { size: 12 }),
    ]),
  ]);

// `conformance::build_menu_bar`: the application menu bar (ADR 0018). The
// declaration rides on the root box the way `windows` does, and the
// `<menuBar/>` element draws it — on a machine with no bar of its own,
// which is what a headless corpus always is.
SCENE_TREES.menubar = () =>
  root({}, [
    box({ gap: 6, width: 'grow' }, [
      el('menuBar', {
        menu: [
          {
            label: 'File',
            items: [
              { label: 'New', id: 'file.new', accel: 'mod+n' },
              { role: 'separator' },
              { label: 'Wrap', id: 'file.wrap', checked: true },
              { label: 'Print', id: 'file.print', enabled: false },
            ],
          },
          { label: 'Edit', items: [{ role: 'copy' }] },
        ],
      }),
      text('body', { size: 12 }),
    ]),
  ]);

// `conformance::build_live`: the row on a box that would otherwise be
// elided, and the queue through `ctx.announce` — the third argument, since
// an announcement is an act and a tree is not.
SCENE_TREES.live = (_fx, phase, ctx) => {
  if (phase === 1) ctx.announce('Saved', 'assertive');
  return root({}, [
    box({ pad: 8, gap: 4, bg: '#14161e' }, [
      box({ live: 'polite' }, [text(phase === 0 ? '0 results' : '3 results', { size: 12 })], 'status'),
      box({ live: 'polite' }, [], 'empty'),
    ]),
  ]);
};

// `conformance::build_virtual`: the rows a virtual list builds, each at its
// own data index rather than at the position it occupies — `index` is what
// makes those two different, and a binding that drops the prop auto-keys the
// rows 1, 2, 3 instead and reports a different access tree for the same
// quads. The spacers are keyed by name, in the other namespace.
SCENE_TREES.virtual = () =>
  root({}, [
    box(
      // `rowCount` is `conformance::VIRTUAL_ROW_COUNT`: how many rows the list
      // has, built or not.
      { width: 120, height: 60, gap: 0, scrollY: true, bg: '#101018', role: 'list', label: 'log', rowCount: 109 },
      [
        box({ width: 'grow', height: 20 }, [], 'lead'),
        ...VIRTUAL_ROWS.map((i) =>
          box({
            index: i,
            width: 'grow',
            height: 20,
            bg: '#30344a',
            role: 'listItem',
            label: `row ${i}`,
          }),
        ),
        box({ width: 'grow', height: 100 }, [], 'tail'),
      ],
      'list',
    ),
  ]);

// `conformance::build_layers`: two floats over a scroller's bar (ADR 0023).
// The toast is later in the tree than the popover, so it is over it in
// phase 0; the popover closes in phase 1 and reopens in phase 2, which puts
// it over the toast. The rows overflow so the page has a bar to cover.
SCENE_TREES.layers = (_fx, phase) => {
  const at = (dx, dy) => ({ anchor: 'viewport', at: ['start', 'start'], self: ['start', 'start'], dx, dy });
  return root({}, [box({ width: 'grow', height: 'grow' }, [
    box(
      { width: 'grow', height: 'grow', scrollY: true, bg: '#101018' },
      Array.from({ length: LAYERS_ROWS }, (_, i) =>
        box({ dir: 'row', width: 'grow', height: 30, bg: i % 2 === 0 ? '#22242c' : '#30344a' }, [], `row${i}`),
      ),
      'page',
    ),
    phase !== 1 && box(
      { float: at(200, 40), width: 120, height: 80, bg: '#3b5bd4', onClick: { kind: 'popover' }, label: 'Popover' },
      [],
      'popover',
    ),
    box(
      { float: at(140, 60), width: 120, height: 80, bg: '#73d98c', onClick: { kind: 'toast' }, label: 'Toast' },
      [],
      'toast',
    ),
  ].filter(Boolean))]);
};

// `conformance::build_drop` (ADR 0031): two zones, a button inside the
// first, and across the phases a hoverable float over the first zone that
// is no zone (phase 1) and a modal over it (phase 2).
SCENE_TREES.drop = (_fx, phase) => {
  const over = { anchor: 'viewport', at: ['start', 'start'], self: ['start', 'start'], dx: 20, dy: 20 };
  return root({}, [box({ dir: 'row', width: 'grow', height: 'grow' }, [
    box(
      { width: 200, height: 'grow', pad: 10, bg: '#22242c', dropBg: '#2b3350', onDrop: { kind: 'files' } },
      [box({ dir: 'row', width: 60, height: 40, bg: '#3b5bd4', onClick: { kind: 'pick' }, label: 'Pick' }, [], 'pick')],
      'files',
    ),
    box({ width: 'grow', height: 'grow', bg: '#30344a', onDrop: { kind: 'other' } }, [], 'other'),
    phase === 1 && box({ float: over, width: 160, height: 160, hoverable: true }, [], 'overlay'),
    phase === 2 && box({ float: over, width: 160, height: 160, bg: '#101018', modal: { kind: 'dismiss' } }, [], 'confirm'),
  ].filter(Boolean))]);
};

// `conformance::build_selection`: a `selectable` card the pointer drags
// across, so the frame carries the three highlight quads under its glyphs
// (ADR 0017). One row on the container is the whole declaration — the
// labels inside say nothing about selection.
const SELECTION_LINES = ['one', 'two', 'three'];
// `conformance::build_sampler` (backlog AR47): the generic rows no other
// scene declares. Every prop below is spelled by its schema name, which is
// what the coverage test after the corpus reads.
SCENE_TREES.sampler = (fx) =>
  root({}, [
    box({ pad: 8, gap: 6 }, [
      box(
        {
          dir: 'row',
          width: 120,
          height: 40,
          maxWidth: 100,
          maxHeight: 30,
          center: true,
          bg: '#1b1d27',
          radiusTL: 8,
          radiusTR: 2,
          radiusBR: 8,
          radiusBL: 2,
          shadowColor: '#00000080',
          shadowX: 3,
          shadowY: 2,
          shadowBlur: 2,
          hoverable: true,
          hoverBg: '#262a3a',
          pressedBg: '#30364a',
          hoverGroup: 'cards',
          focusable: true,
          focusBg: '#2b3350',
          initialFocus: true,
          accent: true,
          cursor: 'pointer',
          selected: true,
          expanded: 'expanded',
          onClick: { kind: 'card' },
          onHover: { kind: 'hov' },
          onLayout: { kind: 'lay' },
          onForceClick: { kind: 'force' },
          clickSound: fx().sound,
          hoverSound: fx().sound,
          animate: true,
          transition: 100,
          easing: 'easeInOut',
          bounce: 0.3,
          slide: true,
          delay: 20,
          iterations: 0.25,
          repeat: 'alternate',
          keyframes: [{ bg: '#1b1d27' }, { at: 1, bg: '#3b5bd4', radius: 12, dy: -4 }],
          enter: { dx: -12, opacity: 0 },
          role: 'tab',
          label: 'Card',
        },
        [text('ab', { size: 12 })],
        'card',
      ),
      box({ dir: 'row', width: 60, height: 10, bg: '#3a3f52', window: 'drag' }, [], 'strip'),
      box({ dir: 'row', focusRegion: true, gap: 4, height: 30, mainAlign: 'center', crossAlign: 'end', backdropBlur: 6 }, [
        box({ dir: 'row', width: 20, height: 20, bg: '#2a2d3a', focusable: true, role: 'button', label: 'Stop' }, [], 'stop'),
      ], 'dock'),
      box({ width: 60 }, [
        text('a long line that is cut short', { size: 12, maxLines: 1, ellipsis: true, underline: true, strikethrough: true, features: 'liga=0' }),
      ]),
      box({ dir: 'row', height: 16, role: 'line', caret: 2, selectionAnchor: 0, caretSolid: true }, [text('sel', { size: 12 })], 'line'),
    ]),
  ]);

SCENE_TREES.selection = () =>
  root({}, [
    box(
      { width: 200, pad: 8, gap: 4, bg: '#14161e', selectable: true },
      SELECTION_LINES.map((line) => text(line, { size: 13 })),
      'card',
    ),
  ]);

// `menu` is the same tree under different steps: the stock context menu
// is the core's, opened by a secondary press that nothing claimed, so no
// binding declares it and all four must still draw it identically.
SCENE_TREES.menu = () => SCENE_TREES.selection();
// And so is `selection-extend`: the Shift-press that keeps the anchor is
// the core's reading of the modifier, nothing the view declares (ADR 0029).
SCENE_TREES['selection-extend'] = () => SCENE_TREES.selection();

// `conformance::build_selection_scroll`: the same card, forty px tall and
// scrolling, over six runs — what a press held past its edge scrolls.
const SELECTION_SCROLL_LINES = ['one', 'two', 'three', 'four', 'five', 'six'];
SCENE_TREES['selection-scroll'] = () =>
  root({}, [
    box(
      { width: 200, height: 40, pad: 8, gap: 4, bg: '#14161e', scrollY: true, selectable: true },
      SELECTION_SCROLL_LINES.map((line) => text(line, { size: 13 })),
      'card',
    ),
  ]);

// `conformance::build_cells_scroll`: the `cells` screen three rows tall,
// `selectable` and hearing the wheel, row 0 at 100 plus the phase — the
// phase being how the scene's view answers a `scroll` event.
const CELLS_SCROLL_ROWS = ['hello world', 'brave', 'bye'];
SCENE_TREES['cells-scroll'] = (_fx, phase) => {
  const cols = 11;
  const screen = new Uint32Array(3 * cols * 4);
  for (let i = 0; i < 3 * cols; i++) {
    screen[i * 4] = ' '.codePointAt(0);
    screen[i * 4 + 1] = 0xd6d8e0ff;
  }
  CELLS_SCROLL_ROWS.forEach((row, r) => {
    row.split('').forEach((ch, c) => {
      screen[(r * cols + c) * 4] = ch.codePointAt(0);
    });
  });
  return root({}, [
    box({ pad: 10 }, [
      el('cells', {
        rows: 3, cols, cells: screen, size: 13, family: 'mono', lineHeight: 18,
        originLine: 100 + phase, selectable: true, onScroll: { kind: 'term' }, label: 'term',
      }, [], 'term'),
    ]),
  ]);
};

// `conformance::build_scroll_gestures` (backlog F107): a page (y) holding a
// strip (x) holding a list that contains its gestures and a wheel handler
// that takes only `y`.
SCENE_TREES['scroll-gestures'] = () =>
  root({}, [
    box({ pad: 4, onScroll: { kind: 'zoom' }, scrollMods: 'ctrl' }, [
      box({ width: 200, height: 100, scrollY: true, bg: '#101018' }, [
        box({ dir: 'row', width: 200, height: 80, scrollX: true }, [
          box(
            { width: 100, height: 80, gap: 4, scrollY: true, overscroll: 'contain', bg: '#161820' },
            ITEM_KEYS.map((k) => box({ width: 90, height: 20, bg: '#30344a' }, [], k)),
            'list',
          ),
          box({ width: 100, height: 80, bg: '#3b5bd4', onScroll: { kind: 'term' }, scrollAxes: 'y' }, [], 'term'),
          box({ width: 100, height: 80, bg: '#2a2d3a' }),
        ], 'strip'),
        box({ width: 200, height: 60, bg: '#22252f' }),
      ], 'page'),
    ]),
  ]);

// `conformance::build_scroll_handler_room` (backlog F118): a strip (x)
// holding a code box that scrolls x and hears the wheel, then a spacer.
SCENE_TREES['scroll-handler-room'] = () =>
  root({}, [
    box({ pad: 4 }, [
      box({ dir: 'row', width: 200, height: 80, scrollX: true, bg: '#101018' }, [
        box({ width: 100, height: 80, scrollX: true, onScroll: { kind: 'code' }, bg: '#161820' }, [
          box({ width: 300, height: 80, bg: '#3b5bd4' }),
        ], 'code'),
        box({ width: 200, height: 80, bg: '#2a2d3a' }),
      ], 'strip'),
    ]),
  ]);

// `conformance::build_drag`: one keyed handle whose drag deltas the event
// rows carry, measured from the press point in every phase.
SCENE_TREES.drag = () =>
  root({}, [box({ width: 80, height: 40, bg: '#30344a', onDrag: { kind: 'split' } }, [], 'handle')]);

/** `conformance::EXIT_BULK_ROWS`: with its own root, one node past
 *  `kui_core::depart::MAX_NODES`, so the whole subtree is refused. */
const EXIT_BULK_ROWS = 4096;
/** `conformance::EXIT_ROWS`: more one-node subtrees than the budget,
 *  dropped in one frame and refused whole (ADR 0012). */
const EXIT_ROWS = 4200;
/** A fixed-size box holding at most one departing node, so dropping that
 *  node moves nothing else on the frame the ghost is compared on. */
const slot = (key, h, child) =>
  box({ width: 140, height: h, bg: '#101018' }, child ? [child] : [], key);
/** A live Tab stop either side of the departing ones. */
const keep = (key, label) =>
  box({ dir: 'row', width: 60, height: 16, bg: '#22242c', focusable: true, label }, [], key);

const ITEM_KEYS = ['i0', 'i1', 'i2', 'i3', 'i4', 'i5'];
/** `conformance::VIRTUAL_ROWS`: data indices past what auto-keying could
 *  have reached under five children, so the keys are the view's own. */
const VIRTUAL_ROWS = [100, 101, 102];
/** `conformance::LAYERS_ROWS`: enough rows to overflow the viewport, so the
 *  page has a bar for the popover to cover. */
const LAYERS_ROWS = 16;
/** `conformance::TOKEN_COLORS` / `TOKEN_LENGTHS` / `TOKEN_KEYS`: the
 *  `tokens` scene's table, with `surface` in it on purpose. */
const TOKEN_COLORS = [
  ['peach', 0xffcc99ff, 0xffcc99ff],
  ['ink', 0x202020ff, 0xe0e0e0ff],
  ['surface', 0xff0000ff, 0xff0000ff],
];
const TOKEN_LENGTHS = [['side_w', 60], ['gap', 8], ['big', 16]];
/** `conformance::TOKEN_DERIVED` (ADR 0028): declared after the values,
 *  each `[name, from, ops]` with `ops` the tuples as the app writes them
 *  — `dim`'s two do not commute, `up` derives from the `surface` *role*
 *  (the declared `surface` above was refused), `deep` from a derived
 *  token, `read` is the contrast loop, and `bad` is dropped. */
const TOKEN_DERIVED = [
  ['lit', 'peach', [['lift', 0.3]]],
  ['dim', 'ink', [['mix', 'peach', 0.5], ['darken', 0.5]]],
  ['up', 'surface', [['raise', 0.25]]],
  ['deep', 'lit', [['alpha', 0.5]]],
  ['read', 'peach', ['readable', 'ink', 4.5]], // the bare-tuple sugar
  ['bad', 'nothing', [['lift', 0.1]]],
];
const TOKEN_KEYS = ['peach', 'ink', 'role', 'missing', 'lit', 'dim', 'up', 'deep', 'read'];
/** A root box sized like the core's implicit root: `configure_root` with
 *  the same data it already has, so only `title` actually lands. */
const root = (props, children) => box({ width: 'grow', height: 'grow', ...props }, children);
/** The corpus fixtures, byte-identical to `conformance::image_pixels` /
 *  `SOUND_BYTES` so the handles and the atlas come out the same. */
const addFixtureImage = (ctx) => ctx.addImage(4, 4, Buffer.alloc(4 * 4 * 4, 0xff));
/** `conformance::Fixtures::stream`: the image again, then `updateImage`d
 *  to 8×2 opaque grey before the first frame, so it is texture-backed
 *  (docs/adr/0025-the-image-is-the-canvas.md). */
const addFixtureStream = (ctx) => {
  const id = addFixtureImage(ctx);
  const px = Buffer.alloc(8 * 2 * 4, 0x80);
  for (let i = 3; i < px.length; i += 4) px[i] = 0xff;
  ctx.updateImage(id, 8, 2, px);
  return id;
};
/** `conformance::FRAGMENT_PARAMS` and `FRAGMENT_PARAMS_LONG`. */
const FRAGMENT_PARAMS = [
  0.85, 0.30, 0.25, 1.0,
  0.20, 0.45, 0.90, 1.0,
  10.0, 2.0, 0.0, 0.0,
  1.0, 1.0, 1.0, 1.0,
];
const FRAGMENT_PARAMS_LONG = [
  0.1, 0.2, 0.3, 1.0, 0.4, 0.5, 0.6, 1.0, 4.0, 1.0, 0.0, 0.0, 0.9, 0.9, 0.2, 1.0, 7.0, 8.0,
];
/** `conformance::FRAGMENT_WGSL`, character for character. */
const FIXTURE_WGSL = `fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let t = clamp(in.local.y / max(in.size.y, 1.0), 0.0, 1.0);
    let base = mix(params[0], params[1], t);
    let d = kui_sd_rounded_box(in.local - in.size * 0.5, in.size * 0.5, vec4<f32>(params[2].x));
    let ring = 1.0 - smoothstep(-KUI_AA, KUI_AA, abs(d) - params[2].y);
    return vec4<f32>(mix(base.rgb, params[3].rgb, ring), base.a);
}`;
const addFixtureFragment = (ctx) => ctx.addFragment(FIXTURE_WGSL);
/** `conformance::FRAGMENT_IMAGE_WGSL`, character for character: the one
 *  that reads its `image` (backlog V1). */
const FIXTURE_IMAGE_WGSL = `fn fragment(in: FragmentIn, params: array<vec4<f32>, 4>) -> vec4<f32> {
    let uv = in.local / max(in.size, vec2<f32>(1.0));
    let c = kui_sample(uv) * params[0];
    return vec4<f32>(c.rgb, c.a * step(1.0, in.image.z));
}`;
const addFixtureSampler = (ctx) => ctx.addFragment(FIXTURE_IMAGE_WGSL);
/** `conformance::Fixtures::dead`: an image registered and removed, the
 *  handle the dead-handle rule is pinned on. Removed rather than made up:
 *  `'0000000000000000'` is "no image" at this door, and any other number
 *  is the first session's handle in some process — raw 1 is the first key
 *  the mint hands out, and a Node process keeps every session the GC has
 *  not collected, so the filtered run saw it as foreign (C31). */
const addFixtureDead = (ctx) => {
  const id = addFixtureImage(ctx);
  ctx.removeImage(id);
  return id;
};
/** `conformance::FRAGMENT_IMAGE_PARAMS`. */
const FRAGMENT_IMAGE_PARAMS = [1.0, 0.5, 0.25, 1.0];
const addFixtureSound = (ctx) => ctx.addSound(Buffer.from('RIFF....WAVE'));

const FNV_OFFSET = 0xcbf29ce484222325n;
const FNV_PRIME = 0x100000001b3n;
const MASK = 0xffffffffffffffffn;

/** FNV-1a over each quad's words 0..18 — `KuiQuad` without its `uv`, which
 *  depends on glyph insertion order, and without the clip index — plus the
 *  `uv` of a segment quad (kind 6), where it is the endpoints, and then the
 *  twenty words of the clip that index names (its rect, radii, transform,
 *  inner clip and its radii). The clip is digested resolved
 *  rather than as the index, so the number says what a backend clips by and
 *  not how this frame interned it. Mirrors `conformance::quad_digest`. */
function quadDigest(buffer, clipBuffer) {
  const stride = quadStride();
  const clipStrideBytes = clipStride();
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  const clipView = new DataView(clipBuffer.buffer, clipBuffer.byteOffset, clipBuffer.byteLength);
  let h = FNV_OFFSET;
  const mix = (word) => {
    let v = BigInt(word);
    for (let b = 0; b < 4; b++) {
      h = (h ^ (v & 0xffn)) & MASK;
      h = (h * FNV_PRIME) & MASK;
      v >>= 8n;
    }
  };
  for (let off = 0; off + stride <= buffer.byteLength; off += stride) {
    const kind = view.getUint32(off + KIND_WORD * 4, true);
    // A segment's `uv` is its endpoints, a texture quad's the index of its
    // side-list entry: geometry both, mixed like the core mixes them.
    const geometry = kind === 6 || kind === 8;
    // Word 19 is the clip index, digested through the table below.
    const words = [...Array(19).keys(), ...(geometry ? [20, 21, 22, 23] : [])];
    for (const i of words) mix(view.getUint32(off + i * 4, true));
    const clip = view.getUint32(off + 19 * 4, true) * clipStrideBytes;
    for (let i = 0; i < 20; i++) {
      mix(clip + (i + 1) * 4 <= clipBuffer.byteLength ? clipView.getUint32(clip + i * 4, true) : 0);
    }
  }
  return h.toString(16).padStart(16, '0');
}

/** The protocol every binding drives: a frame, then each replayed step
 *  followed by another frame, then the last frame's output. Mirrors
 *  `conformance::drive`. */
function driveScene(env, steps, build) {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  // Step 0 of the protocol: the window facts, before the first frame —
  // `<titlebar>` and `<windowButtons>` read them while that frame builds.
  // `null` is a scene at the corpus defaults, which is what a bare `Ctx`
  // already has, so nothing is declared for one.
  if (env) ctx.setEnv({ window: env });
  // The scene is rebuilt every frame, because `exit` only happens to a node
  // the view stops declaring — `phase` is what it stops declaring for. The
  // fixtures behind it are registered once, on the first build that asks,
  // so the handles stay what `conformance::fixtures` hands out.
  let registered = null;
  const fx = () =>
    (registered ??= {
      image: addFixtureImage(ctx),
      stream: addFixtureStream(ctx),
      sound: addFixtureSound(ctx),
      fragment: addFixtureFragment(ctx),
      sampler: addFixtureSampler(ctx),
      dead: addFixtureDead(ctx),
    });
  let phase = 0;
  const events = [];
  const commands = [];
  const audio = [];
  // `ctx` is the third argument because one scene has an imperative half:
  // `live` announces through `ctx.announce`, which is where the other
  // three bindings call `ui.announce` / `env.announce` / `kui_announce`.
  const frame = () => {
    ctx.frame(320, 240, 1, build(fx, phase, ctx));
    events.push(...ctx.pollEvents());
    commands.push(...ctx.windowCommands());
    audio.push(...ctx.audioCommands());
  };
  frame();
  for (const step of steps) {
    // None of the first four is input: the frame clock the transitions
    // read, the view changing its mind, and the OS closing a window or
    // asking a popup to go away.
    if (step[0] === 'phase') phase = step[1];
    else if (step[0] === 'time') ctx.setTime(step[1] / 1000);
    else if (step[0] === 'windowclosed') ctx.windowClosed(step[1]);
    // `conformance::DISMISS_REASONS` order: outside, escape.
    else if (step[0] === 'windowdismissed')
      ctx.windowDismissed(step[1], ['outside', 'escape'][step[2]]);
    // The OS appearance moving under the app: `Appearance::ALL` order.
    else if (step[0] === 'appearance')
      ctx.setEnv({ system: { appearance: ['unknown', 'light', 'dark'][step[1]] } });
    else if (step[0] === 'cursor') ctx.cursor(step[1], step[2]);
    else if (step[0] === 'cursorleft') ctx.cursorLeft();
    else if (step[0] === 'mousedown') ctx.mouse(true, 1);
    else if (step[0] === 'mouseup') ctx.mouse(false);
    else if (step[0] === 'secondarydown') ctx.mouse(true, 1, 'secondary');
    else if (step[0] === 'secondaryup') ctx.mouse(false, 1, 'secondary');
    else if (step[0] === 'middledown') ctx.mouse(true, 1, 'middle');
    else if (step[0] === 'middleup') ctx.mouse(false, 1, 'middle');
    else if (step[0] === 'scroll') ctx.scroll(step[1], step[2]);
    else if (step[0] === 'tab') ctx.key('tab');
    else if (step[0] === 'shifttab') ctx.key('tab', { shift: true });
    // The modifier state as bits — Shift 1, Ctrl 2, Alt 4, Super 8
    // (`KeyMods::bits`): what a Shift-press reads (ADR 0029).
    else if (step[0] === 'modifiers') {
      const m = step[1];
      ctx.modifiers({ shift: !!(m & 1), ctrl: !!(m & 2), alt: !!(m & 4), super: !!(m & 8) });
    }
    else if (step[0] === 'escape') ctx.key('escape');
    // `conformance::ARROWS` order: left, right, up, down.
    else if (step[0] === 'arrow') ctx.key(['left', 'right', 'up', 'down'][step[1]]);
    else if (step[0] === 'home') ctx.key('home');
    else if (step[0] === 'end') ctx.key('end');
    // A Unicode scalar value, so a step line carries only integers.
    else if (step[0] === 'type') ctx.text(String.fromCodePoint(step[1]));
    // The same spelling for a raw key on an `onKey` sink, down and up.
    else if (step[0] === 'keydown') ctx.keyDown(String.fromCodePoint(step[1]));
    else if (step[0] === 'keyup') ctx.keyUp(String.fromCodePoint(step[1]));
    // A named key at a place (backlog F108): `conformance::STEP_KEYS` by
    // index, the place as `KeyLocation::bits`'s number.
    else if (step[0] === 'keyatdown' || step[0] === 'keyatup') {
      const code = ['shift', 'enter', 'capslock'][step[1]];
      const location = ['standard', 'left', 'right', 'numpad'][step[2]];
      if (step[0] === 'keyatdown') ctx.keyDown(code, { location });
      else ctx.keyUp(code, { location });
    }
    // An IME composing one character (its caret at the end, as a byte
    // range) or ending its composition (0), and committing one.
    else if (step[0] === 'preedit') {
      const s = step[1] ? String.fromCodePoint(step[1]) : '';
      ctx.preedit(s, s ? [0, Buffer.byteLength(s)] : null);
    } else if (step[0] === 'commit') ctx.commit(String.fromCodePoint(step[1]));
    // A paste's answer: one character and the pasteboard's markers as
    // `ClipboardMarks::bits` — 1 concealed, 2 transient (backlog F84).
    else if (step[0] === 'paste') {
      ctx.paste(String.fromCodePoint(step[1]), { concealed: !!(step[2] & 1), transient: !!(step[2] & 2) });
    }
    // Files dragged in from the OS (ADR 0031): `n` files spelled
    // `/drop/1.txt` … `/drop/n.txt` (`conformance::drop_paths`), at a point.
    else if (step[0] === 'dragfiles' || step[0] === 'dropfiles') {
      const paths = Array.from({ length: step[1] }, (_, i) => `/drop/${i + 1}.txt`);
      if (step[0] === 'dragfiles') ctx.dragFiles(paths, step[2], step[3]);
      else ctx.dropFiles(paths, step[2], step[3]);
    } else if (step[0] === 'dragcancel') ctx.dragCancel();
    // The OS asking the app to open `n` documents, spelled as the files
    // above (backlog F124).
    else if (step[0] === 'open')
      ctx.openDocuments(Array.from({ length: step[1] }, (_, i) => `/drop/${i + 1}.txt`));
    else throw new Error(`unknown conformance step ${step[0]}`);
    events.push(...ctx.pollEvents());
    commands.push(...ctx.windowCommands());
    audio.push(...ctx.audioCommands());
    frame();
  }
  return { ctx, events, commands, audio };
}

/** A window command as its `cmd` line (`conformance::write_command`). */
function commandLine(c) {
  const verb = { startDrag: 'drag', close: 'close', minimize: 'minimize', toggleMaximize: 'maximize' }[c.kind];
  if (verb) return `cmd ${verb} ${c.window}`;
  const k = ['normal', 'popup'].indexOf(c.config.kind);
  const a = c.config.anchor;
  return [
    'cmd open', c.window, c.owner, c.origin, k,
    Math.trunc(c.config.width), Math.trunc(c.config.height),
    c.config.activates ? 1 : 0,
    Math.trunc(a.x), Math.trunc(a.y), Math.trunc(a.w), Math.trunc(a.h),
  ].join(' ');
}

/** An audio command as its `audio` line (`conformance::write_audio_command`):
 *  the verb and the playback, plus a play's looped bit. Volumes, fades and
 *  the sound handle are left out — none of them would compare across four
 *  bindings. */
function audioLine(c) {
  if (c.kind === 'play') return `audio play ${c.playback} ${c.loop ? 1 : 0}`;
  if (c.kind === 'setVolume') return `audio volume ${c.playback}`;
  if (c.kind === 'masterVolume') return 'audio master';
  if (c.kind === 'unload') return 'audio unload';
  return `audio ${c.kind} ${c.playback}`;
}

/** Renders a scene block in the report format `conformance::report`
 *  documents: integers, hex and strings only, so the bytes match Rust's. */
function sceneReport(name, env, steps, { ctx, events, commands, audio }) {
  const lines = [`scene ${name}`];
  if (env) {
    const { customChrome, maximized, fullscreen, nativeControls } = ctx.env().window;
    const r = nativeControls ?? { w: 0, h: 0 };
    lines.push(`env ${+customChrome} ${+maximized} ${+fullscreen} ${r.w} ${r.h}`);
  }
  for (const step of steps) lines.push(`step ${step.join(' ')}`);
  lines.push(`title ${ctx.windowTitle() ?? '-'}`);
  lines.push(`always-on-top ${+ctx.alwaysOnTop()}`);
  lines.push(`secure-input ${+ctx.secureInput()}`);
  lines.push(`option-as-alt ${ctx.optionAsAlt()}`);
  lines.push(`ime-off ${+ctx.imeOff()}`);
  const quads = Buffer.from(ctx.quads());
  const stride = quadStride();
  const count = quads.byteLength / stride;
  lines.push(`quads ${count} ${quadDigest(quads, Buffer.from(ctx.clips()))}`);
  const kinds = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];
  for (let off = 0; off < quads.byteLength; off += stride) kinds[quads.readUInt32LE(off + KIND_WORD * 4)]++;
  lines.push(`kinds ${kinds.join(' ')}`);
  // A fragment's parameters ride a side list, not the quad, so the digest
  // cannot reach them; the report carries them as bits, like the core's.
  const draws = ctx.fragmentDraws();
  const f32 = new DataView(new ArrayBuffer(4));
  const FD = 24;
  for (let i = 0; i * FD < draws.length; i++) {
    const params = draws.slice(i * FD + 2, i * FD + 18).map((v) => {
      f32.setFloat32(0, v, true);
      return f32.getUint32(0, true).toString(16).padStart(8, '0');
    });
    lines.push(`fragment ${i} ${params.join(' ')}`);
  }
  // Where a fragment's image is, for the draws that have one: the source
  // word (1 atlas, 2 texture), the texture index, the texel rect.
  for (let i = 0; i * FD < draws.length; i++) {
    const from = draws[i * FD + 18];
    if (from === 0) continue;
    const index = from === 2 ? draws[i * FD + 19] : '-';
    lines.push(`fragment-image ${i} ${from === 1 ? 'atlas' : 'texture'} ${index} ${draws.slice(i * FD + 20, i * FD + 24).join(' ')}`);
  }
  // A texture quad's texel rect rides its side list the same way.
  const textures = ctx.textureDraws();
  for (let i = 0; i * 9 < textures.length; i++) {
    lines.push(`texture ${i} ${textures.slice(i * 9 + 5, i * 9 + 9).join(' ')}`);
  }
  const depth = new Map();
  // The access rect as f32 bits, cut to the node's clip (backlog F93).
  const bits = (v) => {
    f32.setFloat32(0, v, true);
    return f32.getUint32(0, true).toString(16).padStart(8, '0');
  };
  for (const n of ctx.accessTree().nodes) {
    const d = n.parent === null ? 0 : depth.get(n.parent) + 1;
    depth.set(n.key, d);
    lines.push(
      [
        'node', d, n.key, n.role,
        n.focused ? 1 : 0,
        n.disabled ? 1 : 0,
        n.mixed ? 'm' : n.checked === null || n.checked === undefined ? '-' : n.checked ? 1 : 0,
        n.selected === null || n.selected === undefined ? '-' : n.selected ? 1 : 0,
        n.orientation === 'horizontal' ? 'h' : n.orientation === 'vertical' ? 'v' : '-',
        n.live === 'polite' ? 'p' : n.live === 'assertive' ? 'a' : '-',
        n.scroll ? 1 : 0,
        bits(n.rect.x), bits(n.rect.y), bits(n.rect.w), bits(n.rect.h),
        n.actions.length ? n.actions.join(',') : '-',
        `${n.name ?? ''} | ${n.description ?? ''} | ${n.value ?? ''}`,
      ].join(' '),
    );
  }
  for (const ev of events) {
    // The tag column, or — for the two window-level events, which have none
    // — the field that tells one from its siblings (`conformance::event_row`).
    const p = ev.payload;
    let tag = p?.tag?.kind ?? p?.phase ?? p?.reason ?? '-';
    // A drag's phase and deltas ride in the tag column: `dx`/`dy` are the
    // displacement from the press point in every phase, and the corpus
    // steps are integers, so the deltas print exactly.
    if (p?.kind === 'drag') tag += ` ${p.phase} ${Math.trunc(p.dx)} ${Math.trunc(p.dy)}`;
    // A slider's change carries its phase and the value the core worked
    // out (ADR 0034); the corpus steps land on whole values.
    if (p?.kind === 'change') tag += ` ${p.phase} ${Math.trunc(p.value)}`;
    // A scroll's lines ride the same way — the whole lines a grid's notch
    // covers, `-` off a grid — so a lost carry disagrees here (ADR 0029).
    if (p?.kind === 'scroll') tag += ` ${p.lines ?? '-'}`;
    // A held button's phase and which button ride the same way (backlog
    // F105): a binding that lost the mask claims the secondary presses.
    if (p?.kind === 'button') tag += ` ${p.phase} ${p.button ?? '-'}`;
    // A key from one of a key's twins says which, with its phase and code
    // (backlog F108); a standard key prints as it did.
    if (p?.kind === 'key' && p.location && p.location !== 'standard')
      tag += ` ${p.phase} ${p.code} ${p.location}`;
    // A drop's phase and its path count ride the same way (ADR 0031).
    if (p?.kind === 'drop') tag += ` ${p.phase} ${p.paths.length}`;
    // The documents the OS handed over, as a count (backlog F124).
    if (p?.kind === 'open') tag += ` ${p.paths.length}`;
    // A paste's markers ride the same way, each only when set (backlog F84).
    if (p?.kind === 'text') for (const m of ['concealed', 'transient']) if (p[m] === true) tag += ` ${m}`;
    lines.push(`event ${p?.kind ?? '-'} ${tag}`);
  }
  for (const c of commands) lines.push(commandLine(c));
  for (const c of audio) lines.push(audioLine(c));
  for (const a of ctx.announcements()) lines.push(`announce ${a.live} ${a.text}`);
  for (const w of ctx.warnings()) lines.push(`warn ${w.code}`);
  lines.push('end', '');
  return lines.join('\n');
}

/** Splits the reference dump into `{name, steps, block}` — mirrors
 *  `conformance::blocks`, and reads the steps back out so the scenes need
 *  not restate the input they replay. */
function referenceBlocks(text) {
  const out = [];
  let cur = null;
  for (const line of text.split('\n')) {
    if (line.startsWith('scene ')) cur = { name: line.slice(6), env: null, steps: [], lines: [] };
    if (!cur) continue;
    if (line.startsWith('env ')) {
      // `conformance::write_env`: the five numbers `kui_env_set_window`
      // takes, in its order. A scene at the defaults writes no line.
      const [chrome, max, full, w, h] = line.slice(4).split(' ').map(Number);
      cur.env = {
        customChrome: !!chrome,
        maximized: !!max,
        fullscreen: !!full,
        // Not on the line: no scene is driven under it, and a headless
        // `Ctx` never has it applied — the ask is the `always-on-top` line.
        alwaysOnTop: false,
        nativeControls: w > 0 && h > 0 ? { w, h } : null,
        // Not on the line either: no scene asks for one, and a headless
        // `Ctx` is opaque (backlog F126).
        backdrop: 'opaque',
      };
    }
    if (line.startsWith('step ')) {
      const [kind, ...args] = line.slice(5).split(' ');
      cur.steps.push([kind, ...args.map(Number)]);
    }
    cur.lines.push(line);
    if (line === 'end') {
      out.push({ ...cur, block: cur.lines.join('\n') + '\n' });
      cur = null;
    }
  }
  return out;
}

// -- Live regions and announcements ----------------------------------------
// docs/adr/0008-live-regions-and-announcements.md: the `live` prop is the
// sustained half, `announce` the one-off.

test('a live box survives elision and is named by its message', () => {
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, box({ pad: 8 }, [box({ live: 'polite' }, [text('3 results', { size: 12 })], 'status')]));
  const rows = ctx.accessTree().nodes.map((n) => [n.role, n.live, n.name]);
  assert.deepEqual(rows, [
    ['window', 'off', null],
    // A plain box: without `live` it would not be in the tree at all, and
    // the text inside is read as part of it — a live region is one
    // message, and its name is what moves when the message does.
    ['group', 'polite', '3 results'],
  ]);
  // The same box without the row leaves no trace.
  ctx.frame(320, 240, 1, box({ pad: 8 }, [box({}, [text('3 results', { size: 12 })], 'status')]));
  assert.deepEqual(ctx.accessTree().nodes.map((n) => n.role), ['window', 'staticText']);
});

test('announce queues, drains once, and rejects a politeness it does not know', () => {
  const ctx = new Ctx();
  ctx.announce('Saved');
  ctx.announce('3 results', 'assertive');
  // 'off' and an empty string are both no-ops.
  ctx.announce('dropped', 'off');
  ctx.announce('');
  assert.deepEqual(ctx.announcements(), [
    { text: 'Saved', live: 'polite' },
    { text: '3 results', live: 'assertive' },
  ]);
  assert.deepEqual(ctx.announcements(), []);
  assert.throws(() => ctx.announce('Saved', 'shouting'), /bad politeness/);
});

test('a live region with nothing to say is a warning', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  ctx.frame(320, 240, 1, box({ pad: 8 }, [box({ live: 'polite' }, [], 'status')]));
  assert.deepEqual(ctx.warnings().map((w) => w.code), ['live-region-without-name']);
});

// -- Declared windows ------------------------------------------------------
// `windows(model)` is the root's `windows` prop, written by the loop; the
// core diffs the set and the app hears about it as data.

test('the loop declares windows(model) and views each open window by name', () => {
  const seen = [];
  const app = createApp(
    {
      init: { palette: true },
      update: (model, msg) => {
        if (msg.kind === 'window') seen.push(`${msg.phase} ${msg.name} ${msg.id}`);
        if (msg.kind === 'toggle') return { ...model, palette: !model.palette };
      },
      windows: (model) => (model.palette ? [{ name: 'palette', width: 400, height: 300 }] : []),
      view: (model, window) => box({ width: 'grow', height: 'grow' }, [text(`${window}:${model.palette}`)]),
    },
    { warnings: false },
  );
  app.render();
  app.settle();
  // The command the driver would apply, and the message the app got.
  assert.deepEqual(app.ctx.windows(), ['main', 'palette']);
  assert.deepEqual(seen, ['opened palette 1']);
  assert.equal(app.ctx.windowName(), 'main');
  // Declared again: nothing new.
  app.render();
  app.settle();
  assert.deepEqual(seen, ['opened palette 1']);
  // Stop declaring it: closed, with the message.
  app.dispatch({ kind: 'toggle' });
  app.render();
  app.settle();
  assert.deepEqual(seen, ['opened palette 1', 'closed palette 1']);
  assert.deepEqual(app.ctx.windows(), ['main']);
  assert.deepEqual(app.warnings, []);
});

test('a window the user closed stays closed while declared, and says so', () => {
  const ctx = new Ctx();
  const declare = () => ctx.frame(320, 240, 1, box({ windows: ['palette'] }, []));
  declare();
  assert.deepEqual(ctx.windowCommands(), [
    {
      kind: 'open',
      window: 1,
      owner: 0,
      origin: 0,
      config: {
        kind: 'normal',
        width: 640,
        height: 480,
        activates: true,
        anchor: { x: 0, y: 0, w: 0, h: 0 },
      },
    },
  ]);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [{ kind: 'window', phase: 'opened', name: 'palette', id: 1 }]);
  ctx.windowClosed(1);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [{ kind: 'window', phase: 'closed', name: 'palette', id: 1 }]);
  declare();
  assert.deepEqual(ctx.windowCommands(), [], 'still declared: nothing reopens');
  assert.deepEqual(ctx.warnings().map((w) => w.code), ['window-declared-while-closed']);
  // Lapse, then declare again: a new window, a new id.
  ctx.frame(320, 240, 1, box({}, []));
  declare();
  assert.deepEqual(ctx.windowCommands().map((c) => [c.kind, c.window]), [['open', 2]]);
  // And the chrome commands say which window too.
  assert.deepEqual(ctx.windows(), ['main', 'palette']);
});

test('a windows entry cannot name a kind kui does not have', () => {
  const ctx = new Ctx();
  assert.throws(
    () => ctx.frame(320, 240, 1, box({ windows: [{ name: 'palette', kind: 'sheet' }] }, [])),
    /sheet/,
    'opening a normal window for it would read as the unknown kind having worked',
  );
});

// -- Popup windows ---------------------------------------------------------
// ADR 0004 decision 9: the declaration differs from a normal window's in a
// kind and an anchor, and the dismissal is an event that closes nothing.

test('a popup declaration carries its kind and anchor and does not activate', () => {
  const ctx = new Ctx();
  const menu = { name: 'menu', kind: 'popup', width: 160, height: 320, anchor: { x: 12, y: 40, w: 160, h: 24 } };
  ctx.frame(320, 240, 1, box({ windows: [menu] }, []));
  assert.deepEqual(ctx.windowCommands(), [
    {
      kind: 'open',
      window: 1,
      owner: 0,
      origin: 0,
      config: {
        kind: 'popup',
        width: 160,
        height: 320,
        // Not asked for, and off: a popup that takes OS focus blurs the
        // field that opened it.
        activates: false,
        anchor: { x: 12, y: 40, w: 160, h: 24 },
      },
    },
  ]);
  // `activates: true` is still available for the surface that wants it.
  ctx.frame(320, 240, 1, box({ windows: [{ ...menu, name: 'other', activates: true }, menu] }, []));
  assert.equal(ctx.windowCommands()[0].config.activates, true);
});

test('a dismissed popup closes nothing until the app stops declaring it', () => {
  const ctx = new Ctx();
  const menu = { name: 'menu', kind: 'popup', width: 160, height: 320 };
  const declare = () => ctx.frame(320, 240, 1, box({ windows: [menu] }, []));
  declare();
  ctx.windowCommands();
  ctx.pollEvents();

  ctx.windowDismissed(1, 'outside');
  ctx.windowDismissed(1, 'escape');
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [
    { kind: 'dismiss', reason: 'outside', name: 'menu', id: 1 },
    { kind: 'dismiss', reason: 'escape', name: 'menu', id: 1 },
  ]);
  assert.deepEqual(ctx.windowCommands(), [], 'the core closes nothing in answer');
  declare();
  assert.deepEqual(ctx.windows(), ['main', 'menu'], 'still open, still declared');

  // The app answers on the frame it chooses — the same answer a `modal`
  // node's dismissal gets.
  ctx.frame(320, 240, 1, box({}, []));
  assert.deepEqual(ctx.windowCommands().map((c) => [c.kind, c.window]), [['close', 1]]);
  ctx.pollEvents();
  // And a stale id invents nothing.
  ctx.windowDismissed(1, 'outside');
  assert.deepEqual(ctx.pollEvents(), []);
});

test('every corpus scene lowers the way kui-core does', (t) => {
  if (!existsSync(CONFORMANCE)) {
    const missing =
      `no reference report at ${CONFORMANCE} — generate it with ` +
      '`cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt`';
    // A skipped test is green, so the skip alone cannot say whether this
    // ever ran where it was supposed to. CI's `check` job — which generates
    // the report and is the one place all four adapters meet — sets
    // KUI_CONFORMANCE_REQUIRED, and there the missing report is a failure.
    // The skip exists for `publish`, which has no cargo target dir and runs
    // the rest of the suite against the prebuilds.
    assert.ok(!process.env.KUI_CONFORMANCE_REQUIRED, `KUI_CONFORMANCE_REQUIRED is set, but ${missing}`);
    t.skip(missing);
    return;
  }
  const blocks = referenceBlocks(readFileSync(CONFORMANCE, 'utf8'));
  assert.ok(blocks.length > 0, 'the reference report has no scenes');
  assert.deepEqual(
    Object.keys(SCENE_TREES).sort(),
    blocks.map((b) => b.name).sort(),
    'the JSX scenes and the corpus have drifted apart',
  );
  for (const { name, env, steps, block } of blocks) {
    const build = SCENE_TREES[name];
    assert.ok(build, `no JSX scene for ${name} — every corpus scene needs one`);
    const out = driveScene(env, steps, build);
    const actual = sceneReport(name, env, steps, out);
    assert.equal(actual, block, `scene ${name} lowers differently than kui-core does`);
    // The readback, beyond the `env` line the report already derives from
    // `ctx.env()`: what the last frame reads back is what the scene was
    // driven under, whole — `chrome` and `chrome-inset` report
    // `customChrome: true` and the rest the defaults. Node's rect is the
    // whole `Rect` where the line carries two extents at the origin
    // (`schema::ENV_FIELDS` names that divergence).
    const declared = env ?? { customChrome: false, maximized: false, fullscreen: false, alwaysOnTop: false, nativeControls: null, backdrop: 'opaque' };
    assert.deepEqual(
      out.ctx.env().window,
      { id: 0, ...declared, nativeControls: declared.nativeControls && { x: 0, y: 0, ...declared.nativeControls } },
      `scene ${name}: env().window reads back differently than it was declared`,
    );
  }
});

// The corpus covers every generic row, or says why not (backlog AR47).
// `the_corpus_covers_every_hand_written_row` in kui-core holds `CUSTOM` and
// `ELEMENTS` to the scenes' claims; the generic `PROPS` rows had no claim
// list and thirty-one of them were in no scene. This is the pin for those,
// at the binding that spells a prop by its schema name: every generic row
// is written in some scene's source — `name:` in the JSX-object form — and
// the other three adapters then have to agree on its bytes, which is what
// a scene is for. Not a claim list: the source is what the scene declares.
const UNCOVERED_PROPS = {
  font: 'a registered font is a file, and the corpus carries no font bytes (backlog AR48 files the fixture)',
};
// `index.d.ts`'s `Theme`, `ThemeOverrides` and `Metrics` are generated
// from the role tables now (backlog AR44). `Env` and `NodeInfo` cannot be —
// the tables carry no TypeScript types — so they are pinned instead: the
// property names each interface declares are exactly the keys the runtime
// object has, both ways, nested interfaces included. A field added to
// `NodeInfo::to_value` or `ENV_FIELDS` is a red test here until the
// interface says so.
test("index.d.ts's Env and NodeInfo name exactly the keys the objects carry (AR44)", () => {
  const dts = readFileSync(new URL('./index.d.ts', import.meta.url), 'utf8');
  const declared = (name) => {
    const m = new RegExp(`export interface ${name} \\{([\\s\\S]*?)\\n\\}`).exec(dts);
    assert.ok(m, `interface ${name} in index.d.ts`);
    // Property lines at the interface's own indent, comments skipped.
    return [...m[1].matchAll(/^  ([A-Za-z_][A-Za-z0-9_]*)\??:/gm)].map((x) => x[1]).sort();
  };
  const ctx = new Ctx();
  ctx.setInspect(true);
  ctx.frame(320, 240, 1, box({ pad: 4 }, [box({ width: 10, height: 10 }, [], 'probe')]));
  const env = ctx.env();
  assert.deepEqual(declared('Env'), Object.keys(env).sort());
  assert.deepEqual(declared('SystemEnv'), Object.keys(env.system).sort());
  assert.deepEqual(declared('WindowEnv'), Object.keys(env.window).sort());
  assert.deepEqual(declared('AudioEnv'), Object.keys(env.audio).sort());
  assert.deepEqual(declared('WindowSize'), Object.keys(env.viewport).sort());
  const node = ctx.nodes().find((n) => n.label === 'probe');
  assert.deepEqual(declared('NodeInfo'), Object.keys(node).sort());
  // And the three generated ones read back whole: every role is a key.
  assert.deepEqual(declared('Theme'), Object.keys(ctx.theme()).sort());
  assert.deepEqual(declared('Metrics'), Object.keys(ctx.metrics()).sort());
});

// The message types — `KeyMsg`, `LayoutMsg` and the rest — are written by
// hand too, against the payload shapes `schema::EVENTS` documents, and
// two of them were a field behind the object (`physical` on a key,
// `scale` on a layout) while two kinds the core spells (`menu`,
// `forceclick`) had no row in the table at all (backlog F55). The pin is
// static: every field a payload shape names is a property of the type
// whose `kind` literal it is, and every property that type declares is in
// some shape of the payload — the dismiss payload has two, one per
// surface, and the type's optional `name` / `id` are the second's.
test("index.d.ts's message types name exactly the fields the event payloads carry (F55)", () => {
  // As LF: Git for Windows checks the file out with CRLF by default, and a
  // `\r` left on every line matched no block below (backlog RG39).
  const dts = readFileSync(new URL('./index.d.ts', import.meta.url), 'utf8').replace(/\r\n/g, '\n');
  const { events } = protocol();
  // The fields of every `{ … }` group in a payload string, at depth 0:
  // `a`, `b: …`, `c?: …` and `kind: "x"` each name one; a nested `{ x, y }`
  // is one field's value.
  const shapes = (payload) => {
    const out = [];
    for (let at = payload.indexOf('{'); at !== -1; at = payload.indexOf('{', at)) {
      let depth = 0;
      let end = at;
      for (let i = at; i < payload.length; i++) {
        const c = payload[i];
        if (c === '{' || c === '[') depth++;
        else if (c === '}' || c === ']') depth--;
        if (depth === 0) {
          end = i;
          break;
        }
      }
      // Names before `:` or bare, `?` dropped, nested groups skipped.
      const names = [];
      let d = 0;
      let cur = '';
      for (const c of payload.slice(at + 1, end) + ',') {
        if (c === '{' || c === '[') d++;
        if (c === '}' || c === ']') d--;
        if (c === ',' && d === 0) {
          const m = /^\s*([A-Za-z_][A-Za-z0-9_]*)\??\s*(:|$)/.exec(cur);
          if (m) names.push(m[1]);
          cur = '';
        } else cur += c;
      }
      out.push(names);
      at = end + 1;
    }
    return out;
  };
  // Every multi-line `export type X = {` / `export interface X {` block,
  // to the `}` at column 0 that ends it (a nested `{ x: number; … }` sits
  // on one indented line), with the property names at the block's own
  // indent; and the one-line union members such as `EditMsg`'s.
  const blocks = [];
  const lines = dts.split('\n');
  for (let i = 0; i < lines.length; i++) {
    const m = /^export (?:type|interface) (\w+)(?:<[^>]*>)? (?:= )?\{$/.exec(lines[i]);
    if (!m) continue;
    const body = [];
    for (i++; i < lines.length && !/^\};?$/.test(lines[i]); i++) body.push(lines[i]);
    blocks.push({ name: m[1], body });
  }
  const declared = (kind) => {
    for (const { name, body } of blocks) {
      if (!body.includes(`  kind: '${kind}';`)) continue;
      const fields = body.map((l) => /^  ([A-Za-z_][A-Za-z0-9_]*)\??:/.exec(l)).filter(Boolean).map((x) => x[1]).sort();
      return { name, fields };
    }
    return dts.includes(`{ kind: '${kind}' }`) ? { name: `{ kind: '${kind}' }`, fields: ['kind'] } : null;
  };
  const untyped = [];
  for (const { kind, payload } of events) {
    const groups = shapes(payload);
    if (kind === 'click') {
      assert.equal(groups.length, 0, 'the click payload is the app\'s own, and names no shape');
      continue;
    }
    assert.ok(groups.length > 0, `the ${kind} payload names a shape`);
    const type = declared(kind);
    if (!type) {
      untyped.push(kind);
      continue;
    }
    const named = [...new Set(groups.flat())].sort();
    for (const f of named) {
      assert.ok(type.fields.includes(f), `${type.name} declares \`${f}\`, which the ${kind} payload carries`);
    }
    for (const f of type.fields) {
      assert.ok(named.includes(f), `the ${kind} payload carries \`${f}\`, which ${type.name} declares`);
    }
  }
  assert.deepEqual(untyped, [], 'every event kind has a message type');
  // And no type in the union names a kind the table does not: a message
  // type is a claim the payload table has to back.
  const union = /export type CoreMsg =\n((?:  \| \w+\n)+)/.exec(dts);
  assert.ok(union, 'the CoreMsg union');
  const kinds = new Set(events.map((e) => e.kind));
  for (const member of union[1].match(/\w+/g)) {
    const block = blocks.find((b) => b.name === member);
    const text = block ? block.body.join('\n') : (new RegExp(`export type ${member} = ((?:\\{ kind: '\\w+' \\}(?: \\| )?)+);`).exec(dts) ?? [])[1];
    assert.ok(text, `${member} is declared`);
    for (const k of text.match(/kind: '(\w+)'/g).map((x) => /'(\w+)'/.exec(x)[1])) {
      assert.ok(kinds.has(k), `${member}'s kind '${k}' is a row of the event table`);
    }
  }
});

test('every generic schema prop is declared by some corpus scene (AR47)', () => {
  const source = Object.values(SCENE_TREES).map((f) => f.toString()).join('\n');
  const { prop } = protocol();
  const missing = Object.entries(prop)
    .filter(([name, def]) => def.kind !== 'custom' && !(name in UNCOVERED_PROPS))
    .map(([name]) => name)
    .filter((name) => !new RegExp(`\\b${name}\\s*:`).test(source));
  assert.deepEqual(missing, [], 'no corpus scene declares these generic props');
  for (const [name, why] of Object.entries(UNCOVERED_PROPS)) {
    assert.ok(name in prop, `UNCOVERED_PROPS names ${name}, which is not a prop`);
    assert.ok(why, `${name} is exempted without a reason`);
    assert.ok(!new RegExp(`\\b${name}\\s*:`).test(source), `${name} is exempted, but a scene declares it — drop the exemption`);
  }
});

// -- Scrolling from the app ------------------------------------------------
// Scroll offsets are retained by the core, keyed by node; `reveal`,
// `scrollOffset` and `setScroll` are the only way an app reaches them.

const ROW_H = 30;
const LIST_H = 200;
const SCROLL_ROWS = 20; // 20 * 30 = 600 of content in a 200-tall window

function scrollingList(rows = SCROLL_ROWS) {
  return box({ width: 'grow', height: 'grow' }, [
    box(
      { width: 'grow', height: 'grow', scrollY: true },
      Array.from({ length: rows }, (_, i) =>
        box({ width: 'grow', height: ROW_H, bg: '#282840', role: 'button', label: `row ${i}` }, [], `row${i}`),
      ),
      'list',
    ),
  ]);
}

/** Node keys and rects of the last frame, by accessible name — the only
 *  keys a JS caller has without recomputing the core's hashing. */
function nodesByName(ctx) {
  const by = {};
  for (const n of ctx.accessTree().nodes) if (n.name) by[n.name] = n;
  return by;
}

function listCtx(rows = SCROLL_ROWS) {
  const ctx = new Ctx();
  const render = (n = rows) => ctx.frame(400, LIST_H, 1, scrollingList(n));
  render();
  // The scroll container is the one node reporting scroll state.
  const list = ctx.accessTree().nodes.find((n) => n.scroll).key;
  return { ctx, render, list };
}

test('reveal scrolls a row into view against the frame that follows it', () => {
  const { ctx, render, list } = listCtx();
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 0 });

  // Row 15 sits at 450..480 in a 200-tall window: off the bottom.
  ctx.reveal(nodesByName(ctx)['row 15'].key);
  render();
  const after = ctx.scrollOffset(list);
  assert.ok(after.y > 0, `reveal moved nothing: ${JSON.stringify(after)}`);
  const row = nodesByName(ctx)['row 15'];
  assert.ok(row.rect.y >= 0 && row.rect.y + row.rect.h <= LIST_H, `row 15 not in view: ${JSON.stringify(row.rect)}`);

  // Resolved and spent: further frames do not drift, and revealing
  // something already visible is not a re-alignment.
  render();
  assert.deepEqual(ctx.scrollOffset(list), after);
  ctx.reveal(row.key);
  render();
  assert.deepEqual(ctx.scrollOffset(list), after);
});

test('reveal takes the row\'s declared label too', () => {
  const { ctx, render, list } = listCtx();
  ctx.reveal('row15');
  render();
  const after = ctx.scrollOffset(list);
  assert.ok(after.y > 0, `reveal moved nothing: ${JSON.stringify(after)}`);
  const row = nodesByName(ctx)['row 15'];
  assert.ok(row.rect.y >= 0 && row.rect.y + row.rect.h <= LIST_H, `row 15 not in view: ${JSON.stringify(row.rect)}`);
});

// -- Window requests -------------------------------------------------------
// `setWindowSize` / `focusWindow` queue commands for the driver (ADR 0004
// step 5) — the two things a declaration cannot say, since a window's config
// is read on the frame it opens and never again.

test('setWindowSize and focusWindow queue commands carrying what they ask for', () => {
  const ctx = new Ctx();
  const fill = () => box({ width: 'grow', height: 'grow', role: 'button', label: 'fill' });
  ctx.frame(400, 300, 1, fill());
  ctx.windowCommands();
  ctx.pollEvents();

  ctx.setWindowSize(0, 640, 480);
  ctx.focusWindow(0);
  assert.deepEqual(ctx.windowCommands(), [
    { kind: 'setSize', window: 0, width: 640, height: 480 },
    { kind: 'focus', window: 0 },
  ]);
  assert.deepEqual(ctx.windowCommands(), [], 'drained once');

  // A request, not a declaration: nothing the core owns moved, and no
  // `resize` was invented. The size a real window becomes comes back from
  // the driver, not from the asking.
  ctx.frame(400, 300, 1, fill());
  const { rect } = nodesByName(ctx).fill;
  assert.deepEqual([rect.w, rect.h], [400, 300]);
  assert.deepEqual(ctx.pollEvents(), []);
});

test('reveal of a key the next frame does not declare is a no-op', () => {
  const { ctx, render, list } = listCtx();
  ctx.reveal('0123456789abcdef');
  render();
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 0 });
  // Not remembered either: the request is spent by the frame that could
  // not find it, so a later frame does not act on it.
  render();
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 0 });
  // A label no frame declared waits for the coming one, and that frame not
  // declaring it either is a warning naming it (backlog DX15).
  ctx.warnings();
  ctx.reveal('nope');
  render();
  assert.deepEqual(ctx.warnings().map((w) => w.code), ['label-without-node']);
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 0 });
});

test('reveal reaches a row the coming frame declares for the first time', () => {
  const { ctx, render, list } = listCtx();
  // The list grows to 40 rows and the app reveals the new last one in the
  // same update — there is no earlier frame that row appears in.
  const rows40 = scrollingList(40);
  const key = (() => {
    ctx.frame(400, LIST_H, 1, rows40);
    const k = nodesByName(ctx)['row 39'].key;
    render(); // back to 20 rows: the key is now absent again
    return k;
  })();
  ctx.reveal(key);
  ctx.frame(400, LIST_H, 1, rows40);
  // 40 * 30 = 1200 of content in 200: the last row is flush with the end.
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 1000 });
});

test('setScroll and scrollOffset round-trip a saved position', () => {
  const { ctx, render, list } = listCtx();
  ctx.cursor(200, 100);
  ctx.scroll(0, -150);
  render();
  const saved = ctx.scrollOffset(list);
  assert.deepEqual(saved, { x: 0, y: 150 });

  // A fresh context (a restarted app) restores it before its first frame.
  const fresh = new Ctx();
  fresh.setScroll(list, saved.x, saved.y);
  fresh.frame(400, LIST_H, 1, scrollingList());
  assert.deepEqual(fresh.scrollOffset(list), saved);
  assert.equal(nodesByName(fresh)['row 5'].rect.y, 0);
});

test('scrollGeometry reports the container box, its content and the travel', () => {
  const { ctx, render, list } = listCtx();
  const g = ctx.scrollGeometry(list);
  assert.deepEqual(g, {
    x: 0,
    y: 0,
    w: 400,
    h: LIST_H,
    contentW: 400,
    contentH: SCROLL_ROWS * ROW_H,
    offset: { x: 0, y: 0 },
    maxOffset: { x: 0, y: SCROLL_ROWS * ROW_H - LIST_H },
  });

  ctx.cursor(200, 100);
  ctx.scroll(0, -150);
  render();
  assert.deepEqual(ctx.scrollGeometry(list).offset, { x: 0, y: 150 });

  // Null for anything no layout has resolved as a scroll container — a row,
  // and a key this frame never declared.
  assert.equal(ctx.scrollGeometry(nodesByName(ctx)['row 0'].key), null);
  assert.equal(ctx.scrollGeometry('0123456789abcdef'), null);
});

// -- Virtual lists (backlog C25) --------------------------------------------
// `uniformList` is the slicing above as a widget: the spacers, the row
// keys, and the one thing a retained-tree binding needs that Rust does not —
// something that makes the view run again when the wheel moves the core's
// offset and no model changed.

// -- The variable-height list (backlog C46) ----------------------------------
// `list` is the Rust `widgets::list` from JSX: the core's `RowHeights` and
// its slicing, driven around the app's `measure`. Rows 0..100 are 20 px and
// the rest 60, so the estimate the first screenful produces is badly wrong
// for the middle of the list — which is what makes the anchor observable.

const varH = (i) => (i < 100 ? 20 : 60);

/** A `list` app over a thousand rows that records what it built and
 *  measured; `box` goes on the container (a `transition`, for RG18). */
function variableApp(box = {}) {
  const heights = new RowHeights(1000, 20);
  const seen = { range: null, measured: 0 };
  const view = (_model, _window, ctx) =>
    box_({ width: 'grow', height: 'grow' }, [
      list(
        ctx,
        { key: 'log', heights, width: 'grow', height: 'grow', ...box },
        (i) => {
          seen.measured += 1;
          return varH(i);
        },
        (i) => {
          seen.range = seen.range === null ? [i, i + 1] : [Math.min(seen.range[0], i), i + 1];
          return box_({ width: 'grow', height: 'grow', label: `row ${i}`, onClick: { kind: 'pick', row: i } });
        },
      ),
    ]);
  const app = createApp(
    {
      init: { picked: -1 },
      update: (model, msg) => (msg.kind === 'pick' ? { picked: msg.row } : undefined),
      view,
    },
    { width: 400, height: 200, warnings: false },
  );
  // The row under the top edge, by a click rather than by arithmetic — the
  // question "did the content move" asks of the pixels.
  const rowUnder = () => {
    app.click(200, 4);
    return app.model.picked;
  };
  return { app, seen, heights, rowUnder };
}
const box_ = (props, children = []) => ({ type: 'box', props, children });

test('list builds a screenful of a thousand rows of two heights', () => {
  const { app, seen, heights } = variableApp();
  app.render();
  seen.range = null;
  app.render();
  const [first, last] = seen.range;
  assert.equal(first, 0);
  assert.ok(last <= 14, `built ${last} rows`);
  // Only what was built has been measured; the rest stands at the mean.
  assert.equal(heights.measured(last - 1), 20);
  assert.equal(heights.measured(last + 50), null);
  assert.equal(app.ctx.scrollGeometry('log').contentH, 1000 * 20);
});

test('a list keeps the row under the pointer while the estimate moves (C46)', () => {
  const { app, heights, rowUnder } = variableApp();
  app.render();
  app.render();
  app.ctx.setScroll('log', 0, 5000);
  app.render();
  const settled = rowUnder();
  assert.ok(settled > 100, `expected to be deep in the list, at ${settled}`);
  for (let n = 0; n < 4; n++) {
    app.render();
    assert.equal(rowUnder(), settled, `the content slid on frame ${n} as the estimate moved`);
  }
  assert.ok(heights.total() > 1000 * 20 * 1.5, `the list learned it is longer: ${heights.total()}`);
});

test('a list glides a long setScroll to the row asked for (RG18)', () => {
  const { app, heights, rowUnder } = variableApp({ transition: 100 });
  app.render();
  app.render();
  const target = 400;
  app.ctx.setScroll('log', 0, heights.offsetOf(target));
  for (let n = 0; n < 30; n++) app.advance(1000 / 60);
  assert.equal(rowUnder(), target);
});

test('list says what it is missing', () => {
  const ctx = new Ctx();
  assert.throws(() => list(ctx, { key: 'log' }, () => 1, () => []), /RowHeights/);
  assert.throws(() => list(ctx, { heights: new RowHeights(1, 1) }, () => 1, () => []), /key/);
  assert.throws(() => list(ctx, { key: 'log', heights: new RowHeights(1, 1) }, null, () => []), /measure/);
});

// -- File dialogs (backlog C51) ----------------------------------------------

test('requestFiles asks once, a host takes it, and the answer reaches update', () => {
  const seen = [];
  const app = createApp(
    {
      init: { ask: false },
      update: (model, msg, _ev, ctx) => {
        seen.push(msg);
        if (msg.kind === 'open') {
          // The ask is made where the surface is: update's fourth argument.
          const asked = ctx.requestFiles({
            mode: 'open',
            multiple: true,
            title: 'Add files',
            filters: [{ name: 'Images', extensions: ['png', '.jpg'] }],
            tag: 'add',
          });
          const again = ctx.requestFiles({});
          seen.push({ asked, again, waiting: ctx.awaitingFiles() });
        }
        return model;
      },
      view: () => box_({ width: 'grow', height: 'grow' }, [
        box_({ width: 100, height: 40, label: 'Open', onClick: { kind: 'open' } }),
      ]),
    },
    { width: 200, height: 100, warnings: false },
  );
  app.render();
  app.click(50, 20);
  assert.deepEqual(seen.at(-1), { asked: true, again: false, waiting: true });

  const asks = app.ctx.takeFileRequests();
  assert.equal(asks.length, 1);
  assert.equal(asks[0].mode, 'open');
  assert.equal(asks[0].multiple, true);
  assert.equal(asks[0].title, 'Add files');
  assert.deepEqual(asks[0].filters, [{ name: 'Images', extensions: ['png', 'jpg'] }]);
  assert.equal(asks[0].tag, 'add');
  assert.deepEqual(app.ctx.takeFileRequests(), [], 'taken once');
  assert.equal(app.ctx.awaitingFiles(), true, 'and still out until it is answered');

  app.ctx.answerFiles(['/tmp/a.png', '/tmp/b.jpg']);
  app.step();
  const files = seen.find((m) => m.kind === 'files');
  assert.deepEqual(files, { kind: 'files', paths: ['/tmp/a.png', '/tmp/b.jpg'], tag: 'add' });
  assert.equal(app.ctx.awaitingFiles(), false);
});

test('a cancelled dialog answers with no paths, and a stray answer is dropped', () => {
  const ctx = new Ctx();
  ctx.answerFiles(['/tmp/nobody-asked']);
  assert.deepEqual(ctx.pollEvents().filter((e) => e.payload?.kind === 'files'), []);
  assert.equal(ctx.requestFiles({ mode: 'save', fileName: 'notes.md' }), true);
  const [ask] = ctx.takeFileRequests();
  assert.equal(ask.fileName, 'notes.md');
  ctx.answerFiles([]);
  const evs = ctx.pollEvents().filter((e) => e.payload?.kind === 'files');
  assert.deepEqual(evs.map((e) => e.payload.paths), [[]]);
  assert.throws(() => ctx.requestFiles({ mode: 'sideways' }), /unknown dialog mode/);
});

// -- Documents the OS opens (backlog F124) ------------------------------------

test('openDocuments reaches update unasked, and none is nothing', () => {
  const seen = [];
  const app = createApp(
    {
      init: {},
      update: (model, msg) => {
        seen.push(msg);
        return model;
      },
      view: () => box_({ width: 'grow', height: 'grow' }, []),
    },
    { width: 200, height: 100, warnings: false },
  );
  app.render();
  app.ctx.openDocuments(['/tmp/a.txt', '/tmp/b.md']);
  app.step();
  assert.deepEqual(seen.filter((m) => m?.kind === 'open'), [{ kind: 'open', paths: ['/tmp/a.txt', '/tmp/b.md'] }]);

  const ctx = new Ctx();
  ctx.openDocuments([]);
  assert.deepEqual(ctx.pollEvents().filter((e) => e.payload?.kind === 'open'), []);
});

/** A `uniformList` app that records the range each frame built. */
function virtualApp(opts = {}) {
  const ROWS = opts.rows ?? 10_000;
  const ROW_H = opts.rowH ?? 28;
  const seen = { range: null, updates: [] };
  const view = (_model, _window, ctx) =>
    box({ width: 'grow', height: 'grow' }, [
      uniformList(
        ctx,
        { key: 'log', rows: ROWS, rowH: ROW_H, width: 'grow', height: 'grow', ...(opts.box ?? {}) },
        (i) => {
          seen.range = seen.range === null ? [i, i + 1] : [Math.min(seen.range[0], i), i + 1];
          return box({ width: 'grow', height: 'grow', label: `row ${i}`, onClick: { kind: 'pick', row: i } });
        },
      ),
    ]);
  const app = createApp(
    {
      init: { picked: -1 },
      update: (model, msg) => {
        seen.updates.push(msg);
        return msg.kind === 'pick' ? { picked: msg.row } : undefined;
      },
      view,
    },
    { width: 480, height: 300, warnings: false },
  );
  return { app, seen, ROWS, ROW_H };
}

test('uniformList builds a screenful of a ten-thousand-row list', () => {
  const { app, seen, ROWS, ROW_H } = virtualApp();
  app.render();
  seen.range = null;
  app.render(); // the second frame is the first with geometry to slice by
  const [first, last] = seen.range;
  assert.equal(first, 0);
  assert.ok(last <= Math.ceil(300 / ROW_H) + 3, `built ${last} rows`);

  // The spacers make it the whole list: the content, the travel and so the
  // scrollbar are the ten thousand rows', not the dozen that were built.
  const g = app.ctx.scrollGeometry('log');
  assert.equal(g.contentH, ROWS * ROW_H);
  assert.equal(g.maxOffset.y, ROWS * ROW_H - g.h);
});

test('a uniformList re-slices on the wheel, with no model change anywhere', () => {
  // The gate: the wheel raises no event of its own and the driver redraws by
  // re-lowering the tree it was handed, so before C25 a JSX list sliced once
  // and froze. One `step()` — what the pump runs after every pump — has to
  // move the built range.
  const { app, seen, ROW_H } = virtualApp();
  app.render();
  app.render();
  const before = seen.range;

  app.ctx.cursor(240, 150);
  app.ctx.scroll(0, -3000);
  seen.range = null;
  seen.updates.length = 0;
  app.step();

  assert.notDeepEqual(seen.range, before, 'the built range did not follow the wheel');
  assert.equal(seen.range[0], Math.floor(3000 / ROW_H) - 2);
  // And `update` never heard about it: the sentinel is the widget's own
  // bookkeeping, not a message the app wrote.
  assert.deepEqual(seen.updates, []);
});

test('a uniformList row is keyed by its data index, so a full list agrees', () => {
  const { app, seen } = virtualApp({ rows: 200, rowH: 20 });
  app.render();
  app.render();

  // Scroll so the built range starts past the top, then click the row under
  // a known y and check the key against the one a list that built every row
  // would have given it.
  app.ctx.setScroll('log', 0, 1000);
  app.render(); // the offset lands on this frame's layout
  seen.range = null;
  app.render(); // ...and this one slices by it
  const [first] = seen.range;
  assert.ok(first > 0, `expected to be scrolled past the top, built from ${first}`);

  const named = app.ctx.accessTree().nodes.filter((n) => n.name?.startsWith('row '));
  assert.ok(named.length > 0);
  // A full list of the same rows: the row at data index `i` gets the same
  // key either way, which is what `index` buys.
  // The same rows built whole. They carry the click the widget's rows carry,
  // because a labelled box with no role and no click is not a semantic node
  // and would not be in the tree to compare.
  const full = new Ctx();
  full.frame(480, 300, 1, box({ width: 'grow', height: 'grow' }, [
    box({ width: 'grow', height: 'grow', scrollY: true, gap: 0 },
      Array.from({ length: 200 }, (_, i) =>
        box({ width: 'grow', height: 20 }, [
          box({ width: 'grow', height: 'grow', label: `row ${i}`, onClick: { kind: 'pick', row: i } }),
        ])), 'log'),
  ]));
  const fullKeys = new Map(
    full.accessTree().nodes.filter((n) => n.name?.startsWith('row ')).map((n) => [n.name, n.key]),
  );
  for (const n of named) {
    assert.equal(n.key, fullKeys.get(n.name), `${n.name} is keyed differently than in a full list`);
  }
});

// DX22: the row spec, the row reveal and the divider Rust has as
// `uniform_list_with`, `widgets::reveal_row` and `widgets::splitter`.
// ADR 0037: a family by name, carried on the wire as a string (v18) and
// resolved by the addon to the handle `addSystemFont` gives.
test('family names an installed face, measured as its handle measures it', () => {
  const ctx = new Ctx();
  const stock = ['sans', 'serif', 'mono'];
  const fam = ctx.systemFonts().find((f) => !stock.includes(f.family))?.family;
  const w = (props) => ctx.measureText('iiiWWW', { size: 20, ...props }).width;
  ctx.warnings();
  if (fam) {
    const handle = ctx.addSystemFont(fam);
    assert.equal(w({ family: fam }), w({ font: handle }), `${fam} by name and by handle`);
  }
  assert.equal(w({ family: 'mono' }), w({ family: 'mono' }), 'a stock name is still stock');
  assert.deepEqual(ctx.warnings(), [], 'nothing missed yet');
  assert.equal(w({ family: 'No Such Family 7' }), w({ family: 'sans' }), 'a miss is sans');
  assert.deepEqual(ctx.warnings().map((x) => x.code), ['unknown-family']);
});

test('revealRow centres an unbuilt row, rowProps styles each row, and a splitter drags', () => {
  const ctx = new Ctx();
  let target = null;
  let scrolled = null;
  const tree = () => {
    if (target !== null) scrolled = revealRow(ctx, 'list', target, 20);
    return box({ dir: 'row', width: 300, height: 100 }, [
      uniformList(
        ctx,
        {
          key: 'list', rows: 50, rowH: 20, width: 200, height: 100,
          rowProps: (i) => ({ onClick: { kind: 'pick', row: i } }),
        },
        (i) => [text(`row ${i}`)],
      ),
      splitter(ctx, { key: 'bar', onDrag: { kind: 'split' } }),
      box({ width: 'grow', height: 'grow' }),
    ]);
  };
  ctx.frame(300, 100, 1, tree());
  ctx.frame(300, 100, 1, tree());
  assert.equal(rowsInView(ctx, 'list', 20), 5);
  assert.deepEqual(ctx.warnings(), [], 'every prop the helpers spell is known');

  target = 40;
  ctx.frame(300, 100, 1, tree());
  assert.equal(scrolled, true);
  assert.deepEqual(ctx.scrollOffset('list'), { x: 0, y: 760 }, 'row 40 to the middle');
  ctx.pollEvents();
  ctx.cursor(10, 45);
  ctx.mouse(true);
  ctx.mouse(false);
  const pick = ctx.pollEvents().map((e) => e.payload).find((p) => p.kind === 'pick');
  assert.deepEqual(pick, { kind: 'pick', row: 40 }, 'the row under the pointer, clicked through its own node');

  // The bar sits at 200..204; its drag reports the split in its parent.
  ctx.cursor(202, 50);
  ctx.mouse(true);
  ctx.cursor(225, 50);
  ctx.mouse(false);
  const end = ctx.pollEvents().map((e) => e.payload).filter((p) => p.kind === 'drag').pop();
  assert.equal((end.x - end.parent.x) / end.parent.w, 0.75);

  // A row past the end, or no stride, scrolls nothing (backlog RG75).
  target = 500;
  ctx.frame(300, 100, 1, tree());
  assert.equal(scrolled, false, 'past the end');
  assert.deepEqual(ctx.scrollOffset('list'), { x: 0, y: 760 });
  assert.equal(revealRow(ctx, 'list', 3, 0), false);
  assert.equal(rowsInView(ctx, 'list', 0), 0);
});

test('a uniformList whose rows shrank under it lands in one frame', () => {
  // The geometry is the previous frame's, so a list that shrank while
  // scrolled slices past its own new end. Both ends have to be clamped to
  // the row count, not just the far one: an unclamped `first` builds a lead
  // spacer taller than the whole list and no rows at all, and that spacer
  // keeps the offset legal, so it unwinds a viewport a frame instead of
  // landing in one.
  let rows = 200;
  const seen = { range: null };
  const view = (_m, _w, ctx) => {
    seen.range = null;
    return box({ width: 'grow', height: 'grow' }, [
      uniformList(ctx, { key: 'log', rows, rowH: 20, width: 'grow', height: 'grow' }, (i) => {
        seen.range = seen.range === null ? [i, i + 1] : [Math.min(seen.range[0], i), i + 1];
        return box({ width: 'grow', height: 'grow', label: `row ${i}` });
      }),
    ]);
  };
  const app = createApp({ init: {}, update: (m) => m, view }, { width: 480, height: 300, warnings: false });
  app.render();
  app.render();
  app.ctx.setScroll('log', 0, 3000);
  app.render();
  app.render();
  assert.ok(seen.range[0] > 100, `expected to be deep in the list, built from ${seen.range[0]}`);

  rows = 10;
  app.render();
  assert.equal(app.ctx.scrollGeometry('log').contentH, 10 * 20, 'the content is the list it has now');
  app.render();
  assert.deepEqual(seen.range, [0, 10], 'and every row of it is built');
  assert.equal(app.ctx.scrollGeometry('log').offset.y, 0);
});

test('a uniformList read before the frame begins still owes the frame that re-slices it (RG24)', () => {
  // Node's view runs before the frame begins, so its `scrollGeometry`
  // read lands between two frames; the core cleared its reads at the
  // frame's start and the window tripling left five rows on screen until
  // the next event (F77 fixed for Rust and Lua only).
  const ctx = new Ctx();
  const view = () =>
    box({ width: 'grow', height: 'grow' }, [
      uniformList(ctx, { key: 'log', rows: 1000, rowH: 20, width: 'grow', height: 'grow' }, (i) =>
        box({ width: 'grow', height: 'grow', label: `row ${i}` }),
      ),
    ]);
  ctx.frame(200, 100, 1, view());
  ctx.frame(200, 100, 1, view());
  assert.equal(ctx.owed().requested, false, 'settled');
  ctx.frame(200, 300, 1, view());
  assert.equal(ctx.owed().requested, true, 'sliced for a 100 box, laid out 300 tall');
  ctx.frame(200, 300, 1, view());
  assert.equal(ctx.owed().requested, false, 'and settled again');
});

test('a stock button takes an index, as a box does, and the index wins over its text (AR40)', () => {
  // In a virtual column `<button index={i}>` warned `unknown-prop` and was
  // keyed by its text, so two rows saying "Open" were one node and focus
  // slid with the range. The row is one of the button's now.
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  const view = (from) => box({ pad: 4 }, [
    ...[from, from + 1].map((i) => el('button', { onClick: { row: i }, index: i }, ['Open'])),
    el('button', { onClick: 'k', index: 7 }, ['Keyed'], 'named'),
  ]);
  ctx.frame(200, 200, 1, view(0));
  assert.deepEqual(ctx.warnings(), [], 'index is a button row');
  const buttons = ctx.accessTree().nodes.filter((n) => n.role === 'button');
  assert.equal(buttons.length, 3, 'two rows with the same text are two nodes');
  const [row0, row1] = buttons;
  assert.notEqual(row0.key, row1.key);
  ctx.focus(row1.key);
  ctx.frame(200, 200, 1, view(1));
  const slid = ctx.accessTree().nodes.filter((n) => n.role === 'button');
  assert.equal(slid[0].key, row1.key, 'row 1 keeps its key as it moves up the list');
  assert.equal(ctx.focused(), row1.key, 'and its focus');
  assert.equal(ctx.keyOf('named'), null, 'declared beside `key`, the index wins');
  ctx.cursor(20, 12); ctx.mouse(true); ctx.mouse(false);
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload), [{ row: 1 }], 'the click carries the row its index named');
});

test("a cells cursor's shapes are the addon's list, and a window's size and chrome words are refused, not dropped (AR40)", () => {
  assert.deepEqual(protocol().cellCursors, ['block', 'bar', 'underline']);
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const grid = new Uint32Array(4 * 4);
  assert.throws(
    () => ctx.frame(100, 100, 1, box({}, [el('cells', { ...mono, rows: 1, cols: 4, cells: grid, cursorAt: [0, 0], cursorShape: 'blob' })])),
    /block \| bar \| underline/,
  );
  // The constructor refuses before it opens anything, so this runs headless.
  assert.throws(() => new KuiWindow('t', { width: 320 }), /width` and `height` go together/);
  assert.throws(() => new KuiWindow('t', { chrome: 'frameless' }), /chrome must be/);
  // `textAa` is the launcher's `text_aa`, C's `KuiRunConfig.text_aa`
  // (AR27); it passes through `windowOptions` and an unknown word is
  // refused the same way.
  assert.equal(windowOptions({ textAa: 'gray' }).textAa, 'gray');
  assert.throws(() => new KuiWindow('t', { textAa: 'lcd' }), /textAa must be "auto", "gray" or "subpixel"/);
  // `titlebar` is the launcher's (backlog W22), the same way.
  assert.equal(windowOptions({ titlebar: 'tall' }).titlebar, 'tall');
  assert.throws(() => new KuiWindow('t', { titlebar: 'huge' }), /titlebar is one of \["standard", "medium", "tall"\]/);
});

test('an index is a row number, and anything else is refused', () => {
  // Folded to 0 it would silently take row 0's key, and two of them in one
  // frame would share it - the `duplicate-key` case, arrived at in silence.
  const ctx = new Ctx();
  for (const bad of [-1, 1.5, NaN]) {
    assert.throws(
      () => ctx.frame(320, 240, 1, box({ width: 'grow', height: 'grow' }, [
        box({ index: bad, width: 'grow', height: 10 }),
      ])),
      /index must be a whole row number/,
      `index ${bad}`,
    );
  }
});

test('uniformList says what it needs rather than drawing nothing', () => {
  const ctx = new Ctx();
  assert.throws(() => uniformList(ctx, { rows: 10, rowH: 10 }, () => box({})), /string `key`/);
  assert.throws(() => uniformList(ctx, { key: 'l', rows: 10 }, () => box({})), /positive `rowH`/);
  assert.throws(() => uniformList(ctx, { key: 'l', rowH: 10 }, () => box({})), /`rows` count/);
  assert.throws(() => uniformList(ctx, { key: 'l', rows: 10, rowH: 10 }), /row builder/);
});

test('a query answers for a label no frame declared; a command still throws', () => {
  // Both spellings name a node, but the two kinds of call want different
  // answers for "nothing is called that". A query has one already — false,
  // null, a zero offset — and a view asks *before* the node exists: the
  // first frame of a virtual list asks its own container for geometry.
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, box({ width: 'grow', height: 'grow' }));
  assert.equal(ctx.scrollGeometry('nothing'), null);
  assert.deepEqual(ctx.scrollOffset('nothing'), { x: 0, y: 0 });
  assert.equal(ctx.isHovered('nothing'), false);
  assert.equal(ctx.isPressed('nothing'), false);
  assert.equal(ctx.isFocused('nothing'), false);
  assert.equal(ctx.editText('nothing'), null);
  assert.equal(ctx.textHit('nothing', 1, 1), null);
  assert.equal(ctx.caretRect('nothing', 0), null);
  // A command is a typo the app wants named.
  assert.throws(() => ctx.focus('nothing'), /no node is keyed/);
  // Except the two a view makes before the node exists — the list it is
  // about to show, a row it grows into — which wait for the frame and
  // name the typo as a warning when that frame declares nothing under it
  // (backlog DX15).
  ctx.warnings();
  ctx.reveal('nothing');
  ctx.setScroll('nothing', 0, 0);
  ctx.frame(320, 240, 1, box({ width: 'grow', height: 'grow' }));
  assert.deepEqual(ctx.warnings().map((w) => w.code), ['label-without-node', 'label-without-node']);
});

test('index keys a node the way auto-keying would have, wherever it sits', () => {
  // Three rows at data indices 5, 6, 7 with a spacer before them: the keys
  // are the ones a list that built rows 0..8 would have given, not the ones
  // their positions (1, 2, 3) would.
  const indexed = new Ctx();
  indexed.frame(320, 240, 1, box({ width: 'grow', height: 'grow' }, [
    box({ width: 'grow', height: 10 }, [], 'lead'),
    ...[5, 6, 7].map((i) => box({ index: i, width: 'grow', height: 10, label: `r${i}` })),
  ]));
  const full = new Ctx();
  full.frame(320, 240, 1, box({ width: 'grow', height: 'grow' },
    Array.from({ length: 8 }, (_, i) => box({ width: 'grow', height: 10, label: `r${i}` }))));

  const keys = (ctx) =>
    new Map(ctx.accessTree().nodes.filter((n) => /^r\d$/.test(n.name ?? '')).map((n) => [n.name, n.key]));
  const a = keys(indexed);
  const b = keys(full);
  for (const name of ['r5', 'r6', 'r7']) {
    assert.equal(a.get(name), b.get(name), `${name} differs between an indexed list and a full one`);
  }
});

test('scrollGeometry is one coherent moment, so a view can slice by it', () => {
  const { ctx, list } = listCtx();
  // "Jump to the end" leaves a raw number in the store...
  ctx.setScroll(list, 0, 1e9);
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 1e9 });
  // ...but the geometry a view slices by is already a position in the list.
  const g = ctx.scrollGeometry(list);
  assert.deepEqual(g.offset, g.maxOffset);
  assert.equal(g.offset.y, SCROLL_ROWS * ROW_H - LIST_H);
});

test('a view slices a 10k-row list from the geometry alone', () => {
  const ctx = new Ctx();
  const ROWS = 10_000;
  let built = 0;
  const key = 'list';
  let listKey = null;
  const render = () => {
    const g = listKey ? ctx.scrollGeometry(listKey) : null;
    const h = g ? g.h : LIST_H;
    const top = g ? g.offset.y : 0;
    const first = Math.min(ROWS, Math.max(0, Math.floor(top / ROW_H)));
    const last = Math.min(ROWS, Math.ceil((top + h) / ROW_H));
    built = last - first;
    const kids = [];
    if (first > 0) kids.push(box({ width: 'grow', height: first * ROW_H }, [], 'lead'));
    for (let i = first; i < last; i++) {
      kids.push(box({ width: 'grow', height: ROW_H, bg: '#282840' }, [], `row${i}`));
    }
    if (last < ROWS) kids.push(box({ width: 'grow', height: (ROWS - last) * ROW_H }, [], 'tail'));
    ctx.frame(400, LIST_H, 1, box({ width: 'grow', height: 'grow' }, [
      box({ width: 'grow', height: 'grow', scrollY: true }, kids, key),
    ]));
  };
  // First frame has no geometry and falls back to the window height.
  render();
  listKey = ctx.accessTree().nodes.find((n) => n.scroll).key;
  render();
  assert.equal(built, Math.ceil(LIST_H / ROW_H));

  // The spacers make it the whole list: full travel, and "jump to the end"
  // lands on the last row even though it was never built.
  const g = ctx.scrollGeometry(listKey);
  assert.equal(g.contentH, ROWS * ROW_H);
  ctx.setScroll(listKey, 0, 1e9);
  render();
  render();
  assert.equal(ctx.scrollOffset(listKey).y, ROWS * ROW_H - LIST_H);
  assert.equal(built, Math.ceil(LIST_H / ROW_H), 'still a screenful at the far end');
});

test('setScroll is clamped by the next layout', () => {
  const { ctx, render, list } = listCtx();
  // "Jump to the end" without knowing the content height.
  ctx.setScroll(list, 0, 1e9);
  render();
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: SCROLL_ROWS * ROW_H - LIST_H });
  ctx.setScroll(list, 0, -1e9);
  render();
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 0 });
  // A node that never scrolled reads zero rather than failing.
  assert.deepEqual(ctx.scrollOffset(nodesByName(ctx)['row 0'].key), { x: 0, y: 0 });
});


// -- Env --------------------------------------------------------------------
// The host facts a frame driver pushes in. A window's runner refreshes all of
// them every frame; headless, `setEnv` is the only writer, which is what lets
// a test declare custom chrome and see what a view would build under it.

const CHROME = () =>
  box({}, [el('titlebar', {}, [text('app', { size: 12 })])]);

test('env() reports the defaults a headless Ctx starts with', () => {
  const ctx = new Ctx();
  // Before any frame there is no viewport, and `env()` still answers.
  assert.deepEqual(ctx.env().viewport, { width: 0, height: 0, scale: 1 });
  ctx.frame(320, 240, 2, box({}, []));
  const env = ctx.env();
  assert.equal(env.refreshHz, null, 'a headless host cannot tell');
  // 120 Hz is the fallback the budget is computed at when it cannot.
  assert.ok(Math.abs(env.frameBudgetMs - 1000 / 120) < 1e-3);
  assert.equal(env.focused, true, 'the window, not a node');
  // Nobody asked the OS anything, and the reading says exactly that
  // rather than "light, unreduced, English".
  assert.deepEqual(env.system, {
    appearance: 'unknown',
    accent: null,
    motion: 'unknown',
    locale: null,
    assistive: 'unknown',
  });
  assert.deepEqual(env.viewport, { width: 320, height: 240, scale: 2 });
  assert.deepEqual(env.window, {
    id: 0,
    customChrome: false,
    maximized: false,
    fullscreen: false,
    alwaysOnTop: false,
    nativeControls: null,
    backdrop: 'opaque',
  });
  // No device and nothing playing, which is the truth for a headless host.
  assert.deepEqual(env.audio, { device: 'closed', live: 0 });
});

// `schema::ENV_FIELDS` is the one statement of the env shape; this is
// Node's pin to it. Every documented key path is a leaf however deep its
// value goes (a `Rect`), so the walk stops there; anything else that is an
// object is descended into, so a key added anywhere in `env()` — or one
// dropped — is a difference against the table.
test('env() is the documented env shape, key for key', () => {
  const ctx = new Ctx();
  // Every fact that is sometimes null, present.
  ctx.setEnv({
    refreshHz: 60,
    system: { accent: 0x3b82f6ff, locale: 'en-US' },
    window: { nativeControls: { w: 78, h: 28 } },
  });
  ctx.frame(320, 240, 1, box({}, []));
  const documented = protocol().env.flatMap((f) => f.node);
  const actual = [];
  const walk = (o, prefix) => {
    for (const [k, v] of Object.entries(o)) {
      const path = prefix ? `${prefix}.${k}` : k;
      if (documented.includes(path) || v === null || typeof v !== 'object') actual.push(path);
      else walk(v, path);
    }
  };
  walk(ctx.env(), '');
  assert.deepEqual(actual.sort(), [...documented].sort(), "env()'s keys and schema::ENV_FIELDS's Node column disagree");
});

test('setEnv writes the facts a window would push, and env() reads them back', () => {
  const ctx = new Ctx();
  ctx.setEnv({
    refreshHz: 60,
    focused: false,
    window: { customChrome: true, maximized: true, fullscreen: true, alwaysOnTop: true, nativeControls: { x: 8, y: 4, w: 70, h: 20 }, backdrop: 'blur' },
  });
  const env = ctx.env();
  assert.equal(env.refreshHz, 60);
  assert.ok(Math.abs(env.frameBudgetMs - 1000 / 60) < 1e-3, 'the budget follows the rate');
  assert.equal(env.focused, false);
  assert.deepEqual(env.window, {
    id: 0,
    customChrome: true,
    maximized: true,
    fullscreen: true,
    alwaysOnTop: true,
    nativeControls: { x: 8, y: 4, w: 70, h: 20 },
    backdrop: 'blur',
  });
  // A backdrop is one of four effects (backlog F126).
  assert.throws(() => ctx.setEnv({ window: { backdrop: 'mica' } }), /backdrop is one of/);
  // Only what you pass moves — a test declares the one fact it is about.
  ctx.setEnv({ window: { maximized: false } });
  assert.equal(ctx.env().refreshHz, 60);
  assert.equal(ctx.env().window.customChrome, true);
  assert.equal(ctx.env().window.maximized, false);
  // Both spellings of "the host cannot tell" / "nothing is drawn over us".
  ctx.setEnv({ refreshHz: null, window: { nativeControls: null } });
  assert.equal(ctx.env().refreshHz, null);
  assert.equal(ctx.env().window.nativeControls, null);
  ctx.setEnv({ refreshHz: 0, window: { nativeControls: { w: 0, h: 0 } } });
  assert.equal(ctx.env().refreshHz, null, 'a rate of zero is no rate');
  assert.equal(ctx.env().window.nativeControls, null, 'and a zero-sized rect is no rect');
});

// `env().viewport` is what the dock leaves, not the window (backlog F43).
// The pomodoro under `KUI_DEVTOOLS=1` read 1040 from it and from
// `win.size()`, sized its tiers to that and was squeezed into the ~700 px
// the right dock left: the getter filled the row from the window while the
// row's own `ENV_FIELDS` entry said `Core::viewport()`, the frame's. The
// shape test above could not see it — a key-for-key walk — and no corpus
// scene has a dock in its tree, where the two numbers are equal. A headless
// `Ctx` never reads `KUI_DEVTOOLS` but `setDevtools(true)` works on it, so
// the dock is put in by hand; `KuiWindow.size()` answers with the same
// number and needs a display, so the core pins that half
// (`the_env_reading_and_the_pre_frame_size_are_what_the_dock_leaves`).
test('env().viewport is the window less the devtools dock (F43)', () => {
  const ctx = new Ctx();
  ctx.setDevtools(true);
  ctx.setDevtoolsDock('right');
  assert.deepEqual(ctx.env().viewport, { width: 0, height: 0, scale: 1 }, 'before any frame: nothing established');
  ctx.frame(1040, 720, 1, box({}, []));
  const vp = ctx.env().viewport;
  assert.ok(vp.width < 1040, `the app's width, not the window's: ${vp.width}`);
  assert.equal(vp.height, 720, 'a right dock takes width only');
  assert.equal(vp.scale, 1);
  // A bottom dock takes height instead — the reading follows the dock.
  ctx.setDevtoolsDock('bottom');
  ctx.frame(1040, 720, 1, box({}, []));
  assert.equal(ctx.env().viewport.width, 1040);
  assert.ok(ctx.env().viewport.height < 720, 'a bottom dock takes height');
  // Off again: the window.
  ctx.setDevtools(false);
  ctx.frame(1040, 720, 1, box({}, []));
  assert.deepEqual(ctx.env().viewport, { width: 1040, height: 720, scale: 1 });
});

// `hostArea()` is where the frame put the app in its window (backlog F92):
// `env().viewport` with its origin. The pomodoro's smoke test under
// `KUI_DEVTOOLS=1` sized itself to the host area through F43 but could not
// check that nothing of its own overflowed it: `quads()` is the whole
// display list, dock and all, and the origin — the pane's width under a
// left dock — reached only the Rust runner. The rect, scaled into the
// quads' physical px, is the filter. Both classes have it; a window needs
// a display, so the headless one pins it.
test('hostArea() places the app beside the dock and filters its quads (F92)', () => {
  const APP = [1, 0, 1, 1];
  const ROOT = [0, 128 / 255, 0, 1];
  const card = () => box({ width: 40, height: 30, bg: '#ff00ff' });
  // The root's background is the window's too: under a dock it paints the
  // app's container and, beneath the pane, the whole window (ADR 0024's
  // split), which is the one quad of the app's the rect does not hold.
  const view = () =>
    box({ bg: '#008000' }, [
      box({ width: 'grow', height: 'grow', bg: '#ff00ff' }, [
        card(),
        box({ height: 'grow' }),
        box({ dir: 'row', width: 'grow', mainAlign: 'end' }, [card()]),
      ]),
    ]);
  const ctx = new Ctx();
  assert.equal(typeof KuiWindow.prototype.hostArea, 'function');
  assert.deepEqual(ctx.hostArea(), { x: 0, y: 0, w: 0, h: 0 }, 'before any frame, like env().viewport');
  ctx.frame(1040, 720, 1, view());
  assert.deepEqual(ctx.hostArea(), { x: 0, y: 0, w: 1040, h: 720 }, 'no dock: the window');

  const inside = (q, r, s) =>
    q.x >= r.x * s - 0.01 && q.y >= r.y * s - 0.01 && q.x + q.w <= (r.x + r.w) * s + 0.01 && q.y + q.h <= (r.y + r.h) * s + 0.01;
  const is = (colour) => (q) => q.color.every((c, i) => Math.abs(c - colour[i]) < 1e-3);
  ctx.setDevtools(true);
  for (const [dock, scale] of [['left', 1], ['bottom', 1], ['left', 2]]) {
    const at = `${dock}@${scale}`;
    ctx.setDevtoolsDock(dock);
    ctx.frame(1040, 720, scale, view());
    ctx.frame(1040, 720, scale, view());
    const r = ctx.hostArea();
    const vp = ctx.env().viewport;
    assert.deepEqual([r.w, r.h], [vp.width, vp.height], `${at}: the viewport's size`);
    if (dock === 'left') {
      assert.ok(r.x > 0 && r.x + r.w === 1040, `${at}: right of the pane: ${JSON.stringify(r)}`);
      assert.deepEqual([r.y, r.h], [0, 720]);
    } else {
      assert.deepEqual([r.x, r.y, r.w], [0, 0, 1040], `${at}: the top of the window`);
      assert.ok(r.h < 720, `${at}: above the strip: ${r.h}`);
    }
    const quads = decodeQuads(ctx.quads());
    const mine = quads.filter((q) => inside(q, r, vp.scale));
    const theirs = quads.filter((q) => !inside(q, r, vp.scale));
    assert.equal(mine.filter(is(APP)).length, 3, `${at}: the fill and two cards are inside`);
    assert.equal(mine.filter(is(ROOT)).length, 1, `${at}: and the container's background`);
    assert.equal(mine.length, 4, `${at}: and nothing else is`);
    assert.ok(!theirs.some(is(APP)), `${at}: nothing of the app's own is outside`);
    const windowFill = theirs.filter(is(ROOT));
    assert.equal(windowFill.length, 1, `${at}: but the root's background, once`);
    assert.deepEqual(
      [windowFill[0].x, windowFill[0].y, windowFill[0].w, windowFill[0].h],
      [0, 0, 1040 * scale, 720 * scale],
      `${at}: which is the window`,
    );
    assert.ok(theirs.length > 1, `${at}: the rest is the dock's`);
    const fill = mine.find((q) => is(APP)(q) && q.w === r.w * scale);
    assert.deepEqual([fill.x, fill.y, fill.h], [r.x * scale, r.y * scale, r.h * scale], `${at}: the fill is the rect, physical`);
  }
});

// A headless `Ctx` knows its size before its first frame (backlog F91).
// `init`'s doc promised the function form "the real `size()`", and under
// `createApp` there was none: `Ctx` had no `size()`, `createApp`'s
// `width`/`height` lived only inside `transport()`, and `env().viewport` is
// 0×0 until a frame establishes it, with no `resize` after. The mind map's
// suite fitted its map to a 1000×700 fallback and drew it in an 1100×760
// frame for four releases; the view and the checks read the same wrong
// numbers, so nothing caught it.
test('a headless Ctx answers size() before its first frame and after (F91)', () => {
  const seen = { init: null, view: [] };
  const app = createApp(
    {
      init: (s) => (seen.init = s.size()),
      update: (m) => m,
      view: (m, _w, s) => {
        seen.view.push(s.size());
        return box({ width: 'grow', height: 'grow' });
      },
    },
    { width: 1100, height: 760, warnings: false },
  );
  assert.deepEqual(seen.init, { width: 1100, height: 760, scale: 1 }, 'init reads the size createApp will frame at');
  assert.deepEqual(app.model, seen.init);
  app.render();
  assert.deepEqual(seen.view.at(-1), { width: 1100, height: 760, scale: 1 }, 'and view the frame it is building');
  assert.deepEqual(app.surface.size(), app.surface.env().viewport, 'after a frame, the reading env().viewport gives');
  // A later frame at another size is what size() answers from then on.
  app.surface.frame(640, 480, 2, box({}, []));
  assert.deepEqual(app.surface.size(), { width: 640, height: 480, scale: 2 });
  // Without options, the size createApp's frames default to.
  const plain = createApp({ init: (s) => s.size(), update: (m) => m, view: () => box({}) });
  assert.deepEqual(plain.model, { width: 800, height: 600, scale: 1 });
  // The answer is a copy: a model that keeps it and changes it does not
  // change what the surface answers.
  plain.model.width = 1;
  assert.equal(plain.surface.size().width, 800);

  // A bare Ctx nobody framed answers what env().viewport does.
  const bare = new Ctx();
  assert.deepEqual(bare.size(), { width: 0, height: 0, scale: 1 });
  // One the app framed before handing it over keeps that frame's answer
  // until the loop's first frame replaces it.
  bare.frame(320, 200, 1, box({}, []));
  const handed = createApp({ init: (s) => s.size(), update: (m) => m, view: () => box({}) }, { surface: bare, width: 900, height: 700 });
  assert.deepEqual(handed.model, { width: 320, height: 200, scale: 1 });
  handed.render();
  assert.deepEqual(bare.size(), { width: 900, height: 700, scale: 1 });

  // Docked devtools come off after a frame, as KuiWindow.size()'s do (F43).
  const docked = new Ctx();
  docked.setDevtools(true);
  docked.setDevtoolsDock('right');
  docked.frame(1040, 720, 1, box({}, []));
  assert.ok(docked.size().width < 1040, `the app's width, not the frame's: ${docked.size().width}`);
  assert.equal(docked.size().height, 720);
});

// The chord that moves the keyboard into the devtools dock is the app's
// to respell: `Ctrl+Shift+I` unless `setDevtoolsKey` says otherwise, in
// any spelling a menu item's `accel` takes. With `F12` set, `F12` enters
// and leaves the dock and `Ctrl+Shift+I` reaches the app's sink like any
// other press; the panel's other chords stay where they were.
test('the devtools inspect chord is the app\'s to respell (setDevtoolsKey)', () => {
  const ctx = new Ctx();
  ctx.setDevtools(true);
  ctx.setDevtoolsDock('right');
  const view = () => box({}, [box({ onKey: null, keyFocus: true, width: 100, height: 50 }, [], 'sink')]);
  ctx.frame(1040, 720, 1, view());
  ctx.frame(1040, 720, 1, view());
  const dock = ctx.keyOf('kui-devtools');
  assert.ok(dock, 'the dock is in the frame');
  assert.equal(ctx.devtoolsKey(), 'ctrl+shift+i', 'the default, in the portable spelling');
  assert.throws(() => ctx.setDevtoolsKey('f99'), /not a chord kui can name/);
  assert.equal(ctx.devtoolsKey(), 'ctrl+shift+i', 'a bad spelling leaves it');

  // The default: the chord is the panel's, nothing reaches the sink.
  ctx.keyDown('i', { ctrl: true, shift: true });
  assert.deepEqual(ctx.pollEvents(), [], 'Ctrl+Shift+I is the panel\'s');
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.region(), dock, 'and it enters the dock');
  ctx.keyDown('i', { ctrl: true, shift: true });
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.region(), null, 'and leaves');

  ctx.setDevtoolsKey('F12');
  assert.equal(ctx.devtoolsKey(), 'f12');
  ctx.keyDown('i', { ctrl: true, shift: true });
  const evs = ctx.pollEvents();
  assert.equal(evs.length, 1, 'Ctrl+Shift+I is the app\'s now');
  assert.equal(evs[0].payload.code, 'i');
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.region(), null, 'and the panel did nothing with it');
  ctx.keyDown('f12');
  assert.deepEqual(ctx.pollEvents(), [], 'F12 is the panel\'s');
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.region(), dock, 'F12 enters the dock');
  ctx.keyDown('f12');
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.region(), null, 'F12 leaves');
  // The rest of the family did not move.
  ctx.keyDown('d', { ctrl: true, shift: true });
  assert.equal(ctx.devtoolsDock(), 'bottom', 'Ctrl+Shift+D still walks the dock');
  // Every spelling a menu accel takes, read back portably.
  ctx.setDevtoolsKey('⌥⌘I');
  assert.equal(ctx.devtoolsKey(), 'super+alt+i');
  ctx.setDevtoolsKey('mod+shift+d');
  assert.equal(ctx.devtoolsKey(), process.platform === 'darwin' ? 'super+shift+d' : 'ctrl+shift+d');
});

// A tab in the devtools panel (ADR 0032): `<devtoolsTab>` with a function
// child is the app's own content, called by the encoder only while the
// tab is on show — `frame` reads which tab that is once, before encoding
// — and drawn over the panel's tab body as the app's own nodes; with a
// `slot` it is a plugin's, declared for the panel to mount. The facts
// doors beside it, and the throws for a malformed one.
test('a devtoolsTab is lazy through its function child, and its content is the app\'s (ADR 0032)', () => {
  const btn = (key) => box({ width: 60, height: 20, onClick: { kind: key }, label: key }, [], key);
  const ctx = new Ctx();
  ctx.setDevtools(true);
  ctx.setDevtoolsDock('right');
  ctx.setInspect(true);
  let calls = 0;
  const view = () =>
    box({}, [
      btn('press'),
      el('devtoolsTab', { name: 'syntax', label: 'Tree-sitter' }, [
        () => {
          calls++;
          return box({ gap: 4 }, [el('text', {}, ['from js']), btn('jump')]);
        },
      ]),
      el('devtoolsTab', { name: 'plug', label: 'Plugin', slot: 'ts/panel' }),
    ]);
  ctx.frame(1040, 720, 1, view());
  ctx.frame(1040, 720, 1, view());
  assert.equal(calls, 0, 'the events tab is up: the function child is not called');
  assert.equal(ctx.devtoolsShownTab(), null);
  assert.ok(ctx.nodes().some((n) => n.label === 'kui-devtools/tab-custom:syntax'), 'the strip lists the tab');
  assert.ok(ctx.nodes().some((n) => n.label === 'kui-devtools/tab-custom:plug'), 'and the plugin\'s');
  // N: tree, then Tree-sitter.
  ctx.keyDown('n', { ctrl: true, shift: true });
  ctx.keyDown('n', { ctrl: true, shift: true });
  assert.equal(ctx.devtoolsShownTab(), 'syntax', 'the reading the encoder takes');
  ctx.frame(1040, 720, 1, view());
  assert.equal(calls, 1, 'called once the tab is on show');
  ctx.frame(1040, 720, 1, view());
  assert.equal(calls, 2);
  const body = ctx.nodes().find((n) => n.label === 'kui-devtools/tab/syntax');
  const jump = ctx.nodes().find((n) => n.label === 'jump');
  assert.ok(body && jump, 'the body and the content are both in the frame');
  assert.ok(jump.rect.x >= body.rect.x && jump.rect.x + jump.rect.w <= body.rect.x + body.rect.w, `${JSON.stringify(jump.rect)} in ${JSON.stringify(body.rect)}`);
  assert.ok(ctx.nodes().some((n) => n.text === 'from js'), 'the content\'s text painted');
  // The content's click is the app's.
  ctx.cursor(jump.rect.x + 2, jump.rect.y + 2);
  ctx.mouse(true, 1);
  ctx.mouse(false, 1);
  const evs = ctx.pollEvents();
  assert.equal(evs.length, 1, `one event, the app's: ${JSON.stringify(evs)}`);
  assert.deepEqual(evs[0].payload, { kind: 'jump' });
  // The facts doors: nothing selected until the app selects.
  assert.equal(ctx.devtoolsSelected(), null);
  assert.equal(ctx.devtoolsHovered(), null);
  assert.equal(ctx.devtoolsPicked(), null);
  ctx.setDevtoolsSelected(ctx.keyOf('press'));
  assert.equal(ctx.devtoolsSelected(), ctx.keyOf('press'), 'selected from the app\'s side');
  ctx.setDevtoolsSelected(null);
  assert.equal(ctx.devtoolsSelected(), null);
  // The picker, raised from the app's side while the tab is up: the tab
  // stays, the press lands the node in `devtoolsSelected`.
  assert.equal(ctx.devtoolsPicking(), false);
  ctx.setDevtoolsPick(true);
  assert.equal(ctx.devtoolsPicking(), true);
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.devtoolsShownTab(), 'syntax', 'the tab stayed up while picking');
  const press = ctx.nodes().find((n) => n.label === 'press');
  ctx.cursor(press.rect.x + 2, press.rect.y + 2);
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.devtoolsPicked(), ctx.keyOf('press'), 'the node under the pointer');
  ctx.mouse(true, 1);
  ctx.mouse(false, 1);
  assert.deepEqual(ctx.pollEvents(), [], 'the press was the picker\'s');
  assert.equal(ctx.devtoolsPicking(), false);
  assert.equal(ctx.devtoolsSelected(), ctx.keyOf('press'), 'and landed in selected');
  ctx.frame(1040, 720, 1, view());
  assert.equal(ctx.devtoolsShownTab(), 'syntax', 'the tab is still on show');
  // Off show again: not called, not drawn.
  ctx.keyDown('n', { ctrl: true, shift: true }); // Plugin
  assert.equal(ctx.devtoolsShownTab(), 'plug');
  const before = calls;
  ctx.frame(1040, 720, 1, view());
  assert.equal(calls, before, 'another tab: the function child rests');
  assert.ok(!ctx.nodes().some((n) => n.label === 'jump'));
  // The tab selected from the app's side: a declared name, one of the
  // panel's own, a name nobody declares (kept, the strip falls back), and
  // the selection read back — the strip's own reading, not the encoder's.
  assert.equal(ctx.devtoolsCurrentTab(), 'plug');
  assert.equal(ctx.setDevtoolsTab('syntax'), true, 'listed');
  assert.equal(ctx.devtoolsCurrentTab(), 'syntax');
  assert.equal(ctx.devtoolsShownTab(), 'syntax', 'the encoder reads it too');
  ctx.frame(1040, 720, 1, view());
  assert.equal(calls, before + 1, 'on show: the function child is called');
  assert.equal(ctx.setDevtoolsTab('tree'), true);
  assert.equal(ctx.devtoolsCurrentTab(), 'tree');
  assert.equal(ctx.devtoolsShownTab(), null, 'one of the panel\'s own: nothing for the encoder');
  assert.equal(ctx.setDevtoolsTab('nobody'), false, 'a name no frame declared');
  assert.equal(ctx.devtoolsCurrentTab(), 'tree', 'kept, and the strip falls back');
  ctx.frame(1040, 720, 1, view());
  assert.equal(calls, before + 1, 'the syntax tab rests');
  assert.ok(ctx.nodes().some((n) => n.label === 'kui-devtools/tree-filter'), 'the tree tab is up');
  // A hidden panel comes back docked.
  ctx.setDevtoolsDock('off');
  ctx.setDevtoolsTab('syntax');
  assert.equal(ctx.devtoolsDock(), 'right');
  // Malformed declarations throw at the encoder, the app's own error.
  assert.throws(() => ctx.frame(100, 100, 1, el('devtoolsTab', { name: 'x' }, ['not a function'])), /function child/);
  assert.throws(() => ctx.frame(100, 100, 1, el('devtoolsTab', { name: 'x', slot: 'a/b' }, [() => null])), /not both/);
  assert.throws(() => ctx.frame(100, 100, 1, el('devtoolsTab', { label: 'no name' })), /needs a name/);
});

// `accent` is the one paint row the stock button takes, and the one prop
// whose colour the environment decides: with no accent pushed it is exactly
// the stock button, and with one it is that colour, its shades, and a label
// that stays readable on it.
test('a button follows the theme accent, and the stock blue until there is one', () => {
  const view = (extra = {}) => el('button', { onClick: 'go', ...extra }, ['go']);
  // The button's own quad is the solid one, its label a glyph quad;
  // `decodeQuads` gives both colours as floats.
  const paint = (ctx) => {
    const qs = decodeQuads(ctx.quads());
    return { bg: qs.find((q) => q.kind === 0).color, label: qs.find((q) => q.kind !== 0).color };
  };
  const hex = (n) => [(n >>> 24) & 255, (n >>> 16) & 255, (n >>> 8) & 255, n & 255].map((c) => c / 255);
  const near = (got, want, what) =>
    assert.ok(
      got.every((c, i) => Math.abs(c - want[i]) < 1 / 255),
      `${what}: ${got} is not ${want}`,
    );

  // Nobody told this host anything: byte for byte the stock button.
  const unknown = new Ctx();
  unknown.frame(320, 240, 1, view({ accent: true }));
  const plain = new Ctx();
  plain.frame(320, 240, 1, view());
  assert.deepEqual(unknown.quads(), plain.quads(), 'no accent, no change');
  assert.deepEqual(unknown.warnings(), [], 'and it is a row the button reads');

  // A host that knows: the accent paints the button, and the label stays
  // readable on it — white on this one, black on a light one, which is the
  // reason the row exists rather than a `bg` the app sets itself.
  const dark = new Ctx();
  dark.setEnv({ system: { accent: 0x007affff } });
  dark.frame(320, 240, 1, view({ accent: true }));
  near(paint(dark).bg, hex(0x007affff), 'the accent');
  near(paint(dark).label, [1, 1, 1, 1], 'white on blue');

  const light = new Ctx();
  light.setEnv({ system: { accent: '#ffc409' } });
  light.frame(320, 240, 1, view({ accent: true }));
  near(paint(light).bg, hex(0xffc409ff), 'the accent, however it was spelled');
  near(paint(light).label, [0, 0, 0, 1], 'white on yellow is not a button');

  // And a button that did not ask paints the same: the stock button's
  // trio is the theme's accent trio (backlog AR41), so on a host that
  // reports an accent every button is that colour, `accent` or not — the
  // row is kept for a box's sake and changes nothing on a button.
  const other = new Ctx();
  other.setEnv({ system: { accent: 0x007affff } });
  other.frame(320, 240, 1, view());
  assert.deepEqual(other.quads(), dark.quads(), 'the plain button is the accent one');
  near(paint(other).bg, hex(0x007affff), 'the accent, unasked');
});

// The OS settings a host pushes: two enums whose third reading is "nobody
// asked", and two values that are null until someone did. A view reads them
// and decides; nothing in the core acts on any of it.
test('setEnv carries the OS settings, each with an unknown of its own', () => {
  const ctx = new Ctx();
  ctx.setEnv({ system: { appearance: 'dark', accent: 0x3b82f6ff, motion: 'reduced', locale: 'pt-BR', assistive: 'listening' } });
  assert.deepEqual(ctx.env().system, {
    appearance: 'dark',
    accent: 0x3b82f6ff,
    motion: 'reduced',
    locale: 'pt-BR',
    assistive: 'listening',
  });

  // Only what you pass moves, here as everywhere in setEnv.
  ctx.setEnv({ system: { appearance: 'light' } });
  assert.equal(ctx.env().system.appearance, 'light');
  assert.equal(ctx.env().system.motion, 'reduced', 'the rest kept what it had');

  // The accent takes the spelling a prop takes, and comes back as the
  // number a prop takes — so a view paints with it unconverted.
  ctx.setEnv({ system: { accent: '#3b82f6' } });
  assert.equal(ctx.env().system.accent, 0x3b82f6ff);

  // Every "the host cannot tell" is writable, because a host can stop
  // knowing: a window dragged to a screen whose settings it cannot read.
  ctx.setEnv({ system: { appearance: 'unknown', accent: null, motion: 'unknown', locale: null, assistive: 'unknown' } });
  assert.deepEqual(ctx.env().system, {
    appearance: 'unknown',
    accent: null,
    motion: 'unknown',
    locale: null,
    assistive: 'unknown',
  });
  ctx.setEnv({ system: { accent: 0 } });
  assert.equal(ctx.env().system.accent, null, 'a transparent accent is no accent');
  ctx.setEnv({ system: { accent: '#00000000' } });
  assert.equal(ctx.env().system.accent, null, 'however it was spelled');
});

// The fifth `system` row (backlog F48): whether assistive technology is
// listening, declared the way reduced motion is, read back the same way,
// and reported through the same `system` event — so an app that already
// follows that event for its palette hears when to announce instead of
// blink. A headless Ctx says unknown until told: it has no bridge.
test('setEnv declares that assistive technology is listening, and the system event says so', () => {
  const ctx = new Ctx();
  assert.equal(ctx.env().system.assistive, 'unknown', 'no bridge, no answer');
  ctx.frame(320, 240, 1, box({}, []));
  ctx.pollEvents();

  ctx.setEnv({ system: { assistive: 'listening' } });
  assert.equal(ctx.env().system.assistive, 'listening');
  assert.equal(ctx.env().system.motion, 'unknown', 'only what you pass moves');
  ctx.frame(320, 240, 1, box({}, []));
  const evs = ctx.pollEvents().filter((e) => e.payload.kind === 'system');
  assert.equal(evs.length, 1, 'one system event, on the root');
  assert.equal(evs[0].payload.assistive, 'listening');
  assert.equal(evs[0].payload.appearance, 'unknown', 'the whole reading rides along');

  // Once: a reading that stops changing stops reporting.
  ctx.frame(320, 240, 1, box({}, []));
  assert.equal(ctx.pollEvents().filter((e) => e.payload.kind === 'system').length, 0);

  // Every reading is writable — a host on AT-SPI hears the client leave.
  ctx.setEnv({ system: { assistive: 'none' } });
  assert.equal(ctx.env().system.assistive, 'none');
});

// The audio row: what a driver with a device would push, by the schema's
// names, and only what you pass moves.
test('setEnv carries the audio device state and the live count', () => {
  const ctx = new Ctx();
  ctx.setEnv({ audio: { device: 'open', live: 2 } });
  assert.deepEqual(ctx.env().audio, { device: 'open', live: 2 });
  ctx.setEnv({ audio: { live: 0 } });
  assert.deepEqual(ctx.env().audio, { device: 'open', live: 0 }, 'the idle stream the row is for');
  assert.throws(() => ctx.setEnv({ audio: { device: 'humming' } }), /audio\.device is one of/);
  assert.throws(() => ctx.setEnv({ audio: { volume: 1 } }), /unknown audio key/);
});

// ADR 0004's `window` on the event: the id the driver declared, carried out
// of the core on everything it produces. `origin` is the other half and is a
// different question — which frontend drew the node — so a plain app event
// answers 0 there and whatever window it happened in here.
test('every event says which window it came from', () => {
  const ctx = new Ctx();
  const build = () => box({}, [box({ onClick: { kind: 'hit' }, width: 100, height: 50 }, [], 'b')]);
  ctx.frame(320, 240, 1, build());
  ctx.cursor(50, 25);
  ctx.mouse(true, 1);
  ctx.mouse(false, 1);
  const main = ctx.pollEvents().filter((e) => e.payload.kind === 'hit');
  assert.equal(main.length, 1);
  assert.equal(main[0].window, 0, 'the window an app starts in');
  assert.equal(main[0].origin, 0, 'and the frontend that drew it');

  // A driver standing in for a second window stamps everything it hands
  // out, pending events included — the `resize` this frame produces.
  ctx.setEnv({ window: { id: 7 } });
  assert.equal(ctx.env().window.id, 7, 'a view reads it without a query');
  ctx.frame(400, 300, 1, build());
  ctx.mouse(true, 1);
  ctx.mouse(false, 1);
  const second = ctx.pollEvents();
  assert.ok(second.length > 1, 'the click and the resize');
  assert.deepEqual([...new Set(second.map((e) => e.window))], [7]);
  assert.ok(
    second.some((e) => e.payload.kind === 'resize'),
    'including the ones raised outside handleInput',
  );
});

test('setEnv rejects a key or a type it does not know', () => {
  const ctx = new Ctx();
  assert.throws(() => ctx.setEnv({ customChrome: true }), /unknown key "customChrome"/);
  assert.throws(() => ctx.setEnv({ window: { chrome: true } }), /unknown window key "chrome"/);
  assert.throws(() => ctx.setEnv({ refreshHz: '60' }), /refreshHz must be a number or null/);
  assert.throws(() => ctx.setEnv({ focused: 1 }), /focused must be a boolean/);
  assert.throws(() => ctx.setEnv({ window: { maximized: 'yes' } }), /window.maximized must be a boolean/);
  assert.throws(() => ctx.setEnv({ window: { nativeControls: { w: 'wide', h: 20 } } }), /nativeControls.w must be a number/);
  assert.throws(() => ctx.setEnv({ window: { id: -1 } }), /window.id must be a u32/);
  assert.throws(() => ctx.setEnv({ system: { dark: true } }), /unknown system key "dark"/);
  assert.throws(() => ctx.setEnv({ system: { appearance: 'Dark' } }), /system.appearance is one of/);
  assert.throws(() => ctx.setEnv({ system: { motion: 'none' } }), /system.motion is one of/);
  assert.throws(() => ctx.setEnv({ system: { assistive: 'yes' } }), /system.assistive is one of/);
  assert.throws(() => ctx.setEnv({ system: { accent: 'blue' } }), /bad color/);
  // A tag that does not fit is not stored truncated: a test that lost half
  // its locale would read as a host that could not tell.
  assert.throws(() => ctx.setEnv({ system: { locale: 'x'.repeat(32) } }), /system.locale must be an ASCII BCP-47 tag/);
  assert.throws(() => ctx.setEnv({ system: { locale: 'ру-RU' } }), /system.locale must be an ASCII BCP-47 tag/);
  assert.throws(() => ctx.setEnv(null), /takes an object/);
});

// This is the read the whole thing is for: `widgets::titlebar` and
// `widgets::window_buttons` decide what to build from `env.window` and
// nothing else, so under native decorations they build a strip with no
// controls, and only a declared custom chrome puts the three buttons in it.
test('a declared custom chrome is what makes the window buttons exist', () => {
  const native = new Ctx();
  native.frame(320, 240, 1, CHROME());
  assert.equal(
    native.accessTree().nodes.filter((n) => n.role === 'button').length,
    0,
    'the OS draws the controls; the titlebar draws none',
  );

  const custom = new Ctx();
  custom.setEnv({ window: { customChrome: true } });
  custom.frame(320, 240, 1, CHROME());
  assert.deepEqual(
    custom.accessTree().nodes.filter((n) => n.role === 'button').map((n) => n.name),
    ['Minimize', 'Maximize', 'Close'],
  );
  assert.ok(custom.stats().quadCount > native.stats().quadCount, 'and they are drawn');
});

// The other half of the same read: macOS keeps drawing its traffic lights
// over our content under custom chrome, so the titlebar insets past them and
// draws no buttons of its own.
test('nativeControls inset the titlebar and take its buttons away', () => {
  const titleX = (controls) => {
    const ctx = new Ctx();
    ctx.setEnv({ window: { customChrome: true, nativeControls: controls } });
    ctx.frame(320, 240, 1, CHROME());
    return {
      x: ctx.accessTree().nodes.find((n) => n.name === 'app').rect.x,
      buttons: ctx.accessTree().nodes.filter((n) => n.role === 'button').length,
    };
  };
  // Without them, a plain leading margin and our own three buttons.
  assert.deepEqual(titleX(null), { x: 12, buttons: 3 });
  // With them, the title starts past the reported extent (which includes the
  // trailing gap) and the OS is drawing the controls, so we draw none.
  assert.deepEqual(titleX({ w: 78, h: 28 }), { x: 78, buttons: 0 });
});

// The palette (ADR 0019): derived from what the host said about the OS,
// pinnable by the app, and read back as the numbers a prop takes. The
// roles are `protocol().theme`, so this checks the reading against the one
// table every binding is generated from rather than against a list here.
test('theme() is the roles the protocol declares, derived from env.system', () => {
  const roles = protocol().theme;
  assert.ok(roles.length >= 20, 'the protocol declares the roles');
  const ctx = new Ctx();
  ctx.setEnv({ system: { appearance: 'dark' } });
  const dark = ctx.theme();
  for (const r of roles) {
    assert.equal(typeof dark[r.node], 'number', `${r.node} is a 0xRRGGBBAA number`);
  }
  assert.equal(dark.appearance, 'dark');
  assert.equal(typeof dark.disabledOpacity, 'number');

  // The light base is a different palette, and a readable one: what a
  // hardcoded near-white foreground could not be on a light page.
  ctx.setEnv({ system: { appearance: 'light' } });
  const light = ctx.theme();
  assert.notEqual(light.bg, dark.bg);
  assert.notEqual(light.fg, dark.fg);
  assert.equal(light.appearance, 'light');
  // Roles, not a ramp: the light `surface` is near white, the dark near black.
  assert.ok((light.surface >>> 24) > 0xe0);
  assert.ok((dark.surface >>> 24) < 0x40);
});

// -- Tokens (ADR 0027) -------------------------------------------------------

/** The solid quads' colours as `0xRRGGBBAA`, in paint order. */
const solidColors = (ctx) =>
  decodeQuads(ctx.quads())
    .filter((q) => q.kind === 0)
    .map((q) => q.color.map((c) => Math.round(c * 255)))
    .map(([r, g, b, a]) => ((r << 24) | (g << 16) | (b << 8) | a) >>> 0);

test('a $name in a colour or length prop paints the declared token, and a themed one follows the appearance', () => {
  const T = defineTokens({
    colors: { peach: '#ffcc99', ink: { light: '#111111', dark: '#eeeeee' } },
    lengths: { sideW: 132, gap: 6 },
  });
  assert.deepEqual(T, { peach: '$peach', ink: '$ink', sideW: '$sideW', gap: '$gap' });
  const ctx = new Ctx();
  ctx.setTokens({
    colors: { peach: '#ffcc99', ink: { light: '#111111', dark: '#eeeeee' } },
    lengths: { sideW: 132, gap: 6 },
  });
  const tree = () =>
    root({ gap: T.gap }, [
      box({ width: T.sideW, height: 20, bg: T.peach, borderW: T.gap, borderColor: T.ink }, [], 'a'),
      box({ width: 40, height: T.sideW, bg: T.ink, padL: T.gap, pad: 2 }, [], 'b'),
      el('text', { size: T.gap, color: T.peach }, ['x']),
    ]);
  ctx.setInspect(true);
  ctx.setEnv({ system: { appearance: 'dark' } });
  ctx.frame(320, 240, 1, tree());
  assert.deepEqual(ctx.warnings(), []);
  const byLabel = (l) => ctx.nodes().find((n) => n.label === l);
  const a = byLabel('a');
  const b = byLabel('b');
  assert.equal(a.rect.w, 132, 'width from a length token');
  assert.equal(b.rect.h, 132);
  assert.equal(a.bg, 0xffcc99ff, 'bg from a colour token');
  assert.equal(a.borderWidth, 6, 'a border width token');
  assert.equal(a.borderColor, 0xeeeeeeff, 'the dark half on the dark base');
  assert.equal(b.bg, 0xeeeeeeff);
  assert.equal(b.padding.l, 6, 'a pad edge token over the shorthand');
  assert.equal(b.padding.r, 2);
  assert.equal(a.parent, ctx.nodes()[0].key);
  assert.equal(ctx.nodes()[0].gap, 6, 'a root gap token');
  // The light half, without the view changing.
  ctx.setEnv({ system: { appearance: 'light' } });
  ctx.frame(320, 240, 1, tree());
  assert.equal(byLabel('b').bg, 0x111111ff);
  // Readback is this frame's half, roles left to theme().
  const resolved = ctx.tokens();
  assert.deepEqual(resolved, {
    colors: { peach: 0xffcc99ff, ink: 0x111111ff },
    lengths: { sideW: 132, gap: 6 },
  });
});

test('a role takes the same $ spelling, and a declared role name is refused with reserved-token', () => {
  const ctx = new Ctx();
  ctx.setInspect(true);
  ctx.setEnv({ system: { appearance: 'light' } });
  // The role names come first on purpose: the core drops them, so the
  // encoder must not count them either, or `peach` and `gap` land one
  // index past where the core holds them.
  ctx.setTokens({ colors: { surface: '#ff0000', peach: '#ffcc99' }, lengths: { radius: 99, gap: 5 } });
  assert.equal(roles.surface, '$surface');
  assert.equal(roles.radius, '$radius');
  ctx.frame(320, 240, 1, root({}, [box({ width: 20, height: 20, bg: roles.surface, radius: roles.radius }, [], 'r')]));
  const r = ctx.nodes().find((n) => n.label === 'r');
  assert.equal(r.bg, ctx.theme().surface, '$surface is the theme\'s, not the app\'s red');
  assert.equal(r.radius[0], ctx.metrics().radius, '$radius is the metric');
  const codes = ctx.warnings().map((w) => w.code);
  assert.deepEqual(codes, ['reserved-token', 'reserved-token']);
  assert.deepEqual(ctx.tokens().colors, { peach: 0xffcc99ff }, 'the refused names never entered the table');
  ctx.frame(320, 240, 1, root({}, [box({ width: 20, height: 20, bg: '$peach', padL: '$gap' }, [], 'p')]));
  const p = ctx.nodes().find((n) => n.label === 'p');
  assert.equal(p.bg, 0xffcc99ff, 'the token after the refused one still resolves to itself');
  assert.equal(p.padding.l, 5);
  assert.deepEqual(ctx.warnings().filter((w) => w.code === 'unknown-token'), []);
});

test('a $name nothing declared, or of the other kind, is dropped and warned about once', () => {
  const ctx = new Ctx();
  ctx.setInspect(true);
  ctx.setTokens({ colors: { peach: '#ffcc99' }, lengths: { gap: 6 } });
  const tree = root({}, [
    box({ width: 20, height: 20, bg: '$peech', padL: '$peach', gap: '$nothing' }, [], 'x'),
    box({ width: '$peach', height: 20, bg: '$gap' }, [], 'y'),
  ]);
  ctx.frame(320, 240, 1, tree);
  ctx.frame(320, 240, 1, tree);
  const x = ctx.nodes().find((n) => n.label === 'x');
  assert.equal(x.bg, 0, 'the slot keeps its default');
  assert.equal(x.padding.l, 0);
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-token');
  const named = (w) => /`\$([a-z]+)`/.exec(w.message)[1];
  assert.deepEqual(ws.map(named).sort(), ['gap', 'nothing', 'peach', 'peech'], 'once per name across two frames');
  assert.match(ws.find((w) => named(w) === 'gap').message, /is a length token, and this slot takes a color/);
  assert.match(ws.find((w) => named(w) === 'peech').message, /names no token/);
});

test('{ percent: 50 } and "50%" are the same sizing (AR25)', () => {
  // The object form wrote `v.percent` raw where the string form divides
  // by 100 and the core reads a fraction: `{ percent: 50 }` was 5000%.
  // In no doc, example or test, which is why nothing caught it.
  const enc = createEncoder(protocol());
  const bytes = (w) => enc.encode(box({ width: w, height: 10 })).stream.slice();
  assert.deepEqual(bytes({ percent: 50 }), bytes('50%'));
  const ctx = new Ctx();
  ctx.setInspect(true);
  ctx.frame(200, 100, 1, box({ width: 'grow', height: 'grow' }, [box({ width: { percent: 50 }, height: 10 }, [], 'half')]));
  assert.equal(ctx.nodes().find((n) => n.label === 'half').rect.w, 100);
});

test('a $name reaches a min, a stroke width, a cursor colour and a keyframe stop, and misses by leaving the slot (AR14)', () => {
  // The three slots outside the prop list threw on a `$` — `bad min`,
  // `bad width for <line>`, `bad color` — and a `$` in a keyframe or
  // `enter` stop failed the whole frame in the core. Frame v11: the min
  // row tags like a sizing, the line's flags word says its width slot is
  // an index, the cursor-shape slot says the colour is; a stop resolves
  // in the core through the same lookup, and every miss is the one
  // policy — the slot at its default, one `unknown-token` naming it.
  const ctx = new Ctx();
  ctx.setInspect(true);
  ctx.setTokens({ colors: { peach: '#ffcc99' }, lengths: { gap: 6, wide: 40 } });
  const cells = new Uint32Array([0x61, 0xffffffff, 0, 0, 0x62, 0xffffffff, 0, 0]);
  const tree = root({ pad: 4 }, [
    box({ width: 10, height: 10, minWidth: '$wide' }, [], 'clamp'),
    box({ width: 10, height: 10, minWidth: '$nope' }, [], 'typo'),
    el('line', { from: [0, 0], to: [30, 0], width: '$gap', color: '$peach' }, [], 'stroke'),
    el('line', { from: [0, 0], to: [30, 0], width: '$nothing' }, [], 'plain'),
    el('cells', { rows: 1, cols: 2, cells, cursorAt: [0, 0], cursorColor: '$peach', size: 14, family: 'mono' }, [], 'term'),
    box(
      {
        width: 10,
        height: 10,
        transition: 100,
        keyframes: [{ bg: '$peach', width: '$wide' }, { bg: '$peech', radius: '$gap' }],
        enter: { width: '$missing', bg: '$peach' },
      },
      [],
      'anim',
    ),
  ]);
  ctx.frame(320, 240, 1, tree);
  const node = (label) => ctx.nodes().find((n) => n.label === label);
  assert.equal(node('clamp').rect.w, 40, 'the clamp is the token');
  assert.equal(node('typo').rect.w, 10, 'a miss leaves the row at its default');
  const segs = decodeQuads(ctx.quads()).filter((q) => q.kind === 6);
  assert.equal(segs.length, 2);
  assert.equal(segs[0].borderW, 6, 'the stroke width is the token');
  assert.equal(segs[1].borderW, 1, 'a miss is the default stroke');
  const cursor = decodeQuads(ctx.quads()).find((q) => q.kind === 0 && q.color[0] > 0.99 && q.color[1] > 0.79 && q.color[1] < 0.81);
  assert.ok(cursor, 'the cursor painted in peach');
  ctx.frame(320, 240, 1, tree);
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-token');
  const named = (w) => /`\$([a-z]+)`/.exec(w.message)?.[1] ?? w.message;
  assert.deepEqual(ws.map(named).sort(), ['missing', 'nope', 'nothing', 'peech'], JSON.stringify(ws));
});

test('a derived token is a recipe over an earlier one, resolved by the core and indexed like any other', () => {
  // ADR 0028: `{ from, ops }`, the ops `[verb, …]` tuples folded in order.
  const ctx = new Ctx();
  ctx.setInspect(true);
  ctx.setTokens({
    colors: {
      peach: '#ffcc99',
      ink: { light: '#202020', dark: '#e0e0e0' },
      bad: { from: 'nothing', ops: [['lift', 0.1]] }, // dropped: takes no index
      lit: { from: 'peach', ops: ['lift', 0.5] }, // the bare-tuple sugar
      wash: { from: 'lit', ops: [['alpha', 0.5]] },
      up: { from: 'surface', ops: [['raise', 0.25]] },
      same: { from: 'ink' }, // an alias
    },
  });
  const t = ctx.tokens().colors;
  assert.equal(t.lit, 0xffe6ccff, 'peach lifted halfway to white');
  assert.equal(t.wash, 0xffe6cc80, 'then half alpha');
  assert.equal(t.same, t.ink);
  // `surface` is the role (nothing declared it here), raised toward white
  // on the default dark base.
  const s = ctx.theme().surface >>> 0;
  const ch = (c, i) => (c >>> (24 - 8 * i)) & 0xff;
  const raised = [0, 1, 2].map((i) => Math.round(ch(s, i) + (255 - ch(s, i)) * 0.25));
  assert.deepEqual([0, 1, 2].map((i) => ch(t.up, i)), raised, 'a role source resolves through the theme');
  assert.ok(!('bad' in t), 'the unresolved one never entered the table');
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-token');
  assert.equal(ws.length, 1);
  assert.match(ws[0].message, /`\$bad` is dropped: it derives from `\$nothing`/);
  // The dropped token took no index, so the ones after it line up with the
  // core's — `$lit` paints lit, not the next name over.
  ctx.frame(320, 240, 1, root({}, [box({ width: 20, height: 20, bg: '$lit' }, [], 'l'), box({ width: 20, height: 20, bg: '$same' }, [], 's')]));
  const l = ctx.nodes().find((n) => n.label === 'l');
  assert.equal(l.bg, 0xffe6ccff);
  assert.equal(ctx.nodes().find((n) => n.label === 's').bg, t.ink);
  assert.equal(ctx.warnings().filter((w) => w.code === 'unknown-token').length, 0, 'no new warning for the reference');
});

test('a recipe with a bad verb, arity or operand is refused at the declaration', () => {
  const ctx = new Ctx();
  const declare = (ops) => () => ctx.setTokens({ colors: { peach: '#ffcc99', x: { from: 'peach', ops } } });
  assert.throws(declare([['glow', 0.5]]), /unknown verb "glow"/);
  assert.throws(declare([['lift', 'peach', 0.5]]), /lift takes one number/);
  assert.throws(declare([['mix', 0.5]]), /mix takes a colour and a number/);
  assert.throws(declare([['mix', 3, 0.5]]), /colour is a token or role name/);
  assert.throws(declare([['lift', 'lots']]), /number is a number/);
  assert.throws(declare('lift'), /list of \[verb/);
  assert.throws(() => ctx.setTokens({ colors: { x: { from: 'peach', glow: 1 } } }), /unknown key "glow"/);
  assert.throws(() => ctx.setTokens({ colors: { x: { from: 3 } } }), /from names a colour token or role/);
  // A refused declaration left no table behind.
  assert.deepEqual(ctx.tokens().colors, {});
});

test('a span colour may be a token, and measureText resolves one the same way', () => {
  const ctx = new Ctx();
  ctx.setTokens({ colors: { peach: '#ffcc99' }, lengths: { big: 24 } });
  ctx.frame(320, 240, 1, root({}, [el('text', { size: 12 }, ['a', el('span', { color: '$peach', bg: '$peach' }, ['b'])])]));
  assert.deepEqual(ctx.warnings(), []);
  const glyphs = decodeQuads(ctx.quads()).filter((q) => q.kind === 1 || q.kind === 4);
  assert.ok(glyphs.some((q) => Math.round(q.color[0] * 255) === 0xff && Math.round(q.color[2] * 255) === 0x99), 'the span painted peach');
  const small = ctx.measureText('hello', { size: 12 });
  const big = ctx.measureText('hello', { size: '$big' });
  assert.ok(big.width > small.width, 'a size token reaches the measurement');
  assert.deepEqual(ctx.warnings(), []);
});

test('setTokens replaces the table whole and rejects what it is not', () => {
  const ctx = new Ctx();
  ctx.setTokens({ colors: { peach: '#ffcc99' }, lengths: { gap: 6 } });
  ctx.setTokens({ lengths: { gap: 8 } });
  assert.deepEqual(ctx.tokens(), { colors: {}, lengths: { gap: 8 } });
  assert.throws(() => ctx.setTokens({ colors: { bad: 'red' } }), /bad: /);
  assert.throws(() => ctx.setTokens({ colors: { half: { light: '#fff' } } }), /needs both light and dark/);
  assert.throws(() => ctx.setTokens({ lengths: { w: 'wide' } }), /a length is a number/);
  assert.throws(() => ctx.setTokens({ tokens: {} }), /unknown key/);
  // A refused declaration leaves the encoder's map as it was: `$gap` still
  // resolves after the throw.
  ctx.setInspect(true);
  ctx.frame(320, 240, 1, root({}, [box({ width: 20, height: 20, padL: '$gap' }, [], 'z')]));
  assert.equal(ctx.nodes().find((n) => n.label === 'z').padding.l, 8);
  assert.deepEqual(ctx.warnings(), []);
});

test('an index the core\'s table does not hold keeps the default and warns, rather than refusing the frame', () => {
  // The encoder's map is the surface's and the table is a core's: a
  // second window lowers against a core `setTokens` never reached. Stand
  // in for it with a bare encoder whose map says `peach` is index
  // COLOR_ROLES+5 and a context that declared nothing.
  const P = protocol();
  const enc = createEncoder(P);
  const map = new Map([['peach', { kind: 'color', index: P.tokenRoles.colors.length + 5 }], ['w', { kind: 'length', index: P.tokenRoles.lengths.length + 2 }]]);
  const ctx = new Ctx();
  ctx.setInspect(true);
  const { stream, strings, unknownTokens } = enc.encode(
    root({}, [box({ width: '$w', height: 20, bg: '$peach', padL: '$w', borderW: 2, borderColor: '$peach' }, [], 'q')]),
    map,
  );
  assert.deepEqual(unknownTokens, [], 'the encoder resolved every name against its own map');
  ctx.frameBinary(320, 240, 1, stream, strings);
  const q = ctx.nodes().find((n) => n.label === 'q');
  assert.equal(q.bg, 0, 'transparent, the default');
  assert.equal(q.padding.l, 0);
  assert.equal(q.rect.w, 0, 'a fit width around nothing');
  assert.equal(q.borderColor, 0);
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-token');
  assert.equal(ws.length, 2, 'once per (kind, index)');
  assert.match(ws[0].message, /this window's table does not hold/);
});

test('the token tag and the role indices the encoder writes are the protocol\'s', () => {
  const P = protocol();
  assert.equal(P.tokenTag, 0x8000);
  assert.equal(P.tokenRoles.colors[1], 'surface');
  assert.ok(P.tokenRoles.lengths.includes('radius'));
  // The bare encoder, no surface: a role still resolves, an app name does not.
  const enc = createEncoder(P);
  const out = enc.encode(box({ bg: '$surface', width: '$peach' }));
  assert.deepEqual(out.unknownTokens, [['peach', 'length']]);
  const ids = Array.from(out.stream);
  assert.ok(ids.includes(P.prop.bg.id | 0x8000), 'the bg id is tagged');
  assert.ok(!ids.includes(P.prop.width.id | 0x8000), 'the unresolved width is left out');
});

test('metrics() is the sizes the protocol declares, and setMetrics reaches the stock button', () => {
  const rows = protocol().metrics;
  assert.ok(rows.length >= 12, 'the protocol declares the metrics');
  const ctx = new Ctx();
  const stock = ctx.metrics();
  for (const r of rows) {
    assert.equal(stock[r.node], r.stock, `${r.node} is the stock value`);
  }
  assert.equal(stock.radius, 6);
  assert.equal(stock.controlText, 15);

  const buttonH = () => {
    ctx.frame(320, 240, 1, root({}, [el('button', { onClick: 'ok' }, ['OK'])]));
    const quads = Buffer.from(ctx.quads());
    return quads.readFloatLE(3 * 4);
  };
  const tall = buttonH();
  // Overrides on top of what is in effect; the button is built from them.
  ctx.setMetrics({ controlPadY: 2, controlText: 10 });
  assert.equal(ctx.metrics().controlPadY, 2);
  assert.equal(ctx.metrics().radius, 6, 'the rest is kept');
  assert.ok(buttonH() < tall, 'the button shrank');
  // A named base, then a scale over it: a density slider.
  ctx.setMetrics({ base: 'compact', scale: 2 });
  assert.equal(ctx.metrics().radius, 8);
  // The platform's row is the OS's number, not a density: it stays (AR35).
  assert.equal(ctx.metrics().titlebarH, stock.titlebarH);
  assert.equal(ctx.metrics().controlText, 26);
  // null is the stock set again, and an unknown key is a typo.
  ctx.setMetrics(null);
  assert.deepEqual(ctx.metrics(), stock);
  assert.throws(() => ctx.setMetrics({ raduis: 3 }), /no such metric/);
  assert.throws(() => ctx.setMetrics({ base: 'cozy' }), /comfortable/);
});

test('setAccent keeps the OS light/dark, setTheme follows nothing, null derives again', () => {
  const ctx = new Ctx();
  ctx.setEnv({ system: { appearance: 'light', accent: 0x007affff } });
  assert.equal(ctx.theme().accent, 0x007affff, 'the OS accent, derived');

  // A brand colour, still following the OS's appearance.
  ctx.setAccent('#d2691e');
  assert.equal(ctx.theme().accent, 0xd2691eff);
  assert.equal(ctx.theme().appearance, 'light');
  ctx.setEnv({ system: { appearance: 'dark' } });
  assert.equal(ctx.theme().appearance, 'dark', 'still following');
  assert.equal(ctx.theme().accent, 0xd2691eff, 'still ours');

  // Naming the accent carries its family with it, and a light accent
  // flips the label that goes on top.
  ctx.setAccent(0xffc409ff);
  assert.equal(ctx.theme().onAccent, 0x000000ff, 'white on yellow is not a button');
  assert.notEqual(ctx.theme().accentHover, ctx.theme().accent);

  // Pinned: this base, this one role changed, and nothing follows.
  ctx.setTheme({ appearance: 'light', surface: '#fafafa' });
  assert.equal(ctx.theme().surface, 0xfafafaff);
  assert.equal(ctx.theme().appearance, 'light');
  ctx.setEnv({ system: { appearance: 'dark' } });
  assert.equal(ctx.theme().appearance, 'light', 'pinned follows nothing');

  // And back to the OS for both.
  ctx.setAccent(null);
  assert.equal(ctx.theme().appearance, 'dark');

  // A role that is not one is a typo worth hearing about.
  assert.throws(() => ctx.setTheme({ surfce: '#fff' }), /no such role/);
});

test('a <text> with no color takes the theme\'s foreground, so light mode is legible', () => {
  // The colour a glyph quad carries, for a text that named none. Before
  // ADR 0019 this was the constant `#e8e8ea` on every base, which is
  // 1.22:1 on a light page.
  const fg = (appearance, props) => {
    const ctx = new Ctx();
    ctx.setEnv({ system: { appearance } });
    ctx.frame(200, 100, 1, text('hello', props));
    // Glyph quads are the non-solid ones; kind 0 is a box. The colour is
    // four floats, so compare it as a string rather than by identity.
    return JSON.stringify(decodeQuads(ctx.quads()).find((q) => q.kind !== 0).color);
  };
  assert.notEqual(fg('dark', {}), fg('light', {}), 'the default follows the base');
  // An explicit colour is still exactly itself on both bases.
  const c = { color: '#ff00ff' };
  assert.equal(fg('dark', c), fg('light', c));
  assert.notEqual(fg('light', c), fg('light', {}), 'and it is not the default');
});

test('quads() is on both classes, so a smoke test can read what a window drew (F19)', () => {
  // It sat on `Ctx` alone while `accessTree()`, `stats()` and `animating()`
  // answered for a window through the same core; the pomodoro's report
  // asked for `win.quads()` so its smoke test could cover the driver that
  // ships rather than the one that is convenient. A window needs a display,
  // so the shape is what a headless run can check.
  assert.equal(typeof Ctx.prototype.quads, 'function');
  assert.equal(typeof KuiWindow.prototype.quads, 'function');
});

test('the loop aims the surface at the window it is drawing or answering for (AR12)', () => {
  // Every `KuiWindow` door but `setViewBinary` addressed the main window:
  // `editText('note')` for a second window's editor was null, a
  // `setEditText` from `update` landed on main and warned, `focus('row')`
  // moved the wrong window's focus, and every `$token` in the second
  // window's tree missed. The doors address whatever `useWindow` last
  // named now, and the loop names the window whose view it is calling
  // and the window an event came from before handing the surface to
  // `update` — then main again, so nothing an app forgot lingers.
  const aimed = [];
  const state = { animating: false, events: [] };
  const surface = {
    ...fakeWindow(state),
    windows: () => ['main', 'side'],
    useWindow: (w) => {
      aimed.push(w);
      return true;
    },
  };
  const seen = [];
  const app = createApp(
    {
      init: 0,
      update: (m, msg, ev) => {
        seen.push(['update', msg, aimed.at(-1)]);
        return m + 1;
      },
      view: (m, name) => {
        seen.push(['view', name, aimed.at(-1)]);
        return box({ pad: 4 });
      },
    },
    { surface },
  );
  app.render();
  assert.deepEqual(seen, [
    ['view', 'main', 'main'],
    ['view', 'side', 'side'],
  ]);
  assert.equal(aimed.at(-1), undefined, 'main again once every view has run');
  // An event from the second window: `update` sees the surface aimed at
  // its id; a message with no event behind it sees main (0).
  seen.length = 0;
  state.events.push({ origin: 0, window: 2, key: 'row', payload: 'pick' });
  app.step();
  assert.deepEqual(seen[0], ['update', 'pick', 2]);
  app.dispatch('tick');
  assert.deepEqual(seen.at(-1), ['update', 'tick', 0]);
  assert.equal(aimed.at(-1), undefined);
});

test('useWindow is on both classes, and a headless Ctx is the main window alone (AR12)', () => {
  assert.equal(typeof Ctx.prototype.useWindow, 'function');
  assert.equal(typeof KuiWindow.prototype.useWindow, 'function');
  const ctx = new Ctx();
  assert.equal(ctx.useWindow(), true);
  assert.equal(ctx.useWindow('main'), true);
  assert.equal(ctx.useWindow(0), true);
  assert.equal(ctx.useWindow('side'), false, 'a headless context has no second window');
  assert.equal(ctx.useWindow(2), false);
  // And a surface without the door — a stand-in in a test — is left alone
  // by the loop rather than thrown at.
  const app = createApp({ init: 0, update: (m) => m, view: () => box({ pad: 4 }) }, { surface: fakeWindow({ animating: false, events: [] }) });
  app.render();
});

// The verb table (`schema::DOORS`, backlog B1a) is the one statement of
// which doors each binding has; this is Node's pin to it, both ways. A
// row's Node cell names a method — on both classes, or on the one it is
// prefixed with, or a function the package exports when the prefix is
// `kui` (the runner's decoder, which needs no context) — and every method
// of either class is a row's cell or one
// of the few named here as plumbing: the wire's own ends (the binary
// frame, the raw tokens, the encoder's warnings), the two-class mechanics
// (`useWindow`, `pump`, `size`, `close`), and the draw list's other
// readers beside `quads`. A method added to `lib.rs` or `index.js`
// without a row is a red test here; a row spelling a method that is not
// there is the same.
test('the two classes are the verb table\'s Node column, both ways (B1a)', () => {
  const methods = (cls) =>
    Object.getOwnPropertyNames(cls.prototype).filter((n) => n !== 'constructor' && typeof cls.prototype[n] === 'function');
  const on = { Ctx: new Set(methods(Ctx)), KuiWindow: new Set(methods(KuiWindow)) };
  const named = new Set();
  for (const d of protocol().doors) {
    if (!('is' in d.node)) continue;
    const [cls, name] = d.node.is.includes('.') ? d.node.is.split('.') : [null, d.node.is];
    if (cls === 'kui') {
      assert.equal(typeof pkg[name], 'function', `${d.rust}: the package exports no ${name}`);
      continue;
    }
    for (const c of cls ? [cls] : ['Ctx', 'KuiWindow']) {
      assert.ok(on[c].has(name), `${d.rust}: ${c}.prototype.${name} is not a function`);
      named.add(`${c}.${name}`);
    }
  }
  const plumbing = new Set([
    // The wire: the binary frame and its readers, the raw tokens, the
    // encoder's warnings, and the tree's other readers beside `quads`.
    'frameBinary', 'setViewBinary', 'setView', 'measureTextBinary', 'setTokensRaw',
    'warnUnknownProps', 'warnUnknownTokens', 'clips', 'fragmentDraws', 'textureDraws', 'stats',
    // The input injection, one per `InputEvent` (the table's `handle_input` row).
    'cursor', 'cursorLeft', 'mouse', 'scroll', 'scrollGesture', 'text', 'commit', 'paste', 'preedit', 'key', 'keyDown', 'keyUp',
    'press', 'release', 'access', 'dragFiles', 'dropFiles', 'dragCancel', 'answerFiles', 'openDocuments',
    // The two-class mechanics: the window's own loop and its lifetime.
    'useWindow', 'pump', 'pumpUntil', 'nextDeadlineMs', 'size', 'frameStats', 'close',
  ]);
  const unrowed = [];
  for (const c of ['Ctx', 'KuiWindow']) {
    for (const name of on[c]) {
      if (!named.has(`${c}.${name}`) && !plumbing.has(name)) unrowed.push(`${c}.${name}`);
    }
  }
  assert.deepEqual(unrowed, [], 'methods with no row in schema::DOORS (a verb needs a row with its three other cells)');
});

test('cellSelection() reads a grid\'s selection as absolute lines and columns (B1a)', () => {
  // The row ADR 0017 §4 offered "because a grid's ends mean something to
  // the app" and only Rust had. Directed like `selectionEnds()`, and the
  // lines are the session's own: row 1 of a screen whose row 0 is line
  // 900 is line 901, so a scroll does not move them.
  const ctx = new Ctx();
  const mono = { size: 14, family: 'mono', lineHeight: 20 };
  const rows = 2, cols = 12;
  const grid = new Uint32Array(rows * cols * 4);
  for (let i = 0; i < rows * cols; i++) { grid[i * 4] = 'x'.codePointAt(0); grid[i * 4 + 1] = 0xffffffff; }
  const view = box({}, [
    el('cells', { ...mono, rows, cols, cells: grid, originLine: 900, selectable: true }, [], 'term'),
  ]);
  ctx.frame(400, 200, 1, view);
  assert.equal(ctx.cellSelection(), null, 'nothing selected yet');
  const w = Math.round(ctx.measureText('M', mono).width);
  const at = (r, c) => ctx.cursor((c + 0.5) * w, (r + 0.5) * 20);
  at(1, 4); ctx.mouse(true, 1); at(0, 1); ctx.mouse(false);
  assert.deepEqual(ctx.cellSelection(), {
    node: ctx.keyOf('term'),
    anchor: { line: 901, col: 4 },
    focus: { line: 900, col: 1 },
    block: false,
  });
  assert.equal(ctx.selectionEnds(), null, 'a grid\'s selection is not a text selection');
  ctx.clearSelection();
  assert.equal(ctx.cellSelection(), null);
});

test('<input> is the stock field and <tooltip> the node form, the doors Lua and C had (B1a)', () => {
  // `<input label initial>` is `widgets::text_input`: an editor read back
  // by its label, in the field's own chrome, and nothing else is read —
  // the same two fields Lua's `input { }` takes. `<tooltip>` always draws
  // where the prop is hover-gated, for a hint the view gates itself.
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  ctx.frame(300, 200, 1, box({ pad: 8, width: 200 }, [
    el('input', { label: 'search', initial: 'kui' }),
  ]));
  assert.equal(ctx.editText('search'), 'kui', 'the editor is keyed by its label');
  const field = ctx.accessTree().nodes.find((n) => n.role === 'textInput');
  assert.ok(field, 'an editor in the tree');
  assert.equal(field.name, 'search', 'and the label is its name');
  assert.ok(decodeQuads(ctx.quads()).some((q) => q.kind === 0), 'with the field\'s chrome behind it');
  assert.deepEqual(ctx.warnings(), []);
  assert.throws(() => ctx.frame(300, 200, 1, box({}, [el('input', {})])), /needs a label/);

  // The node form: `value` alone, or children.
  const hint = box({ pad: 8 }, [
    el('box', { hoverable: true, width: 100, height: 40, bg: '#333333' }, [
      el('tooltip', { value: 'a hint' }),
    ]),
  ]);
  ctx.frame(300, 200, 1, hint);
  const before = decodeQuads(ctx.quads()).length;
  ctx.frame(300, 200, 1, box({ pad: 8 }, [el('box', { hoverable: true, width: 100, height: 40, bg: '#333333' }, [])]));
  assert.ok(before > decodeQuads(ctx.quads()).length, 'the tooltip drew without a hover');
  ctx.frame(300, 200, 1, box({ pad: 8 }, [
    el('box', { width: 100, height: 40 }, [el('tooltip', {}, [text('legend'), text('⌘K')])]),
  ]));
  assert.ok(ctx.accessTree().nodes.some((n) => n.name === 'legend'), 'children are the float\'s content');
  assert.deepEqual(ctx.warnings(), []);
  assert.throws(() => ctx.frame(300, 200, 1, box({}, [el('tooltip', {})])), /needs a value or children/);
});

test('the stock toggles read their state and their rows only, and press through onClick (ADR 0034)', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  ctx.frame(320, 240, 1, box({ pad: 10, gap: 6 }, [
    el('checkbox', { checked: true, radius: 4, onClick: { kind: 'sync' } }, ['Sync']),
    el('checkbox', { mixed: true, onClick: { kind: 'all' } }, ['All']),
    el('radioGroup', { label: 'Size', dir: 'row' }, [
      el('radio', { checked: true, onClick: { kind: 's' } }, ['S']),
      el('radio', { onClick: { kind: 'm' } }, ['M']),
    ]),
    el('switch', { onClick: { kind: 'wifi' } }, ['Wi-Fi']),
  ]));
  const nodes = ctx.accessTree().nodes;
  const by = (role, name) => nodes.find((n) => n.role === role && n.name === name);
  assert.equal(by('checkbox', 'Sync').checked, true);
  assert.equal(by('checkbox', 'All').mixed, true, 'mixed reads as mixed');
  assert.equal(by('radioGroup', 'Size').orientation, 'horizontal', 'a row group runs across');
  assert.equal(by('radio', 'S').checked, true);
  assert.equal(by('switch', 'Wi-Fi').checked, false);
  // A paint row the toggle does not read is dropped, and said so.
  const ws = ctx.warnings().filter((w) => w.code === 'unknown-prop');
  assert.equal(ws.length, 1, JSON.stringify(ws));
  assert.match(ws[0].message, /`radius`/);
  ctx.access('Wi-Fi', 'click');
  assert.deepEqual(ctx.pollEvents().map((e) => e.payload.kind), ['wifi']);
});

test('a <radio> outside a <radioGroup> and a tab outside a tabList are warned about, once (F96)', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  const view = () => box({ pad: 10, gap: 6 }, [
    el('radio', { onClick: { kind: 'lone' } }, ['Lone']),
    box({ role: 'tab', label: 'Tab', onClick: 0 }),
    box({ role: 'menuItem', label: 'Item', onClick: 1 }),
    el('radioGroup', { label: 'Size' }, [
      box({}, [el('radio', { onClick: { kind: 's' } }, ['S'])]),
    ]),
    box({ role: 'tabList', label: 'Tabs' }, [box({ role: 'tab', label: 'In', onClick: 2 })]),
  ]);
  ctx.frame(320, 240, 1, view());
  ctx.frame(320, 240, 1, view());
  const ws = ctx.warnings().filter((w) => w.code === 'item-outside-container');
  const named = (name) => ctx.accessTree().nodes.find((n) => n.name === name).key;
  assert.deepEqual(ws.map((w) => w.key), [named('Lone'), named('Tab')], JSON.stringify(ws));
  assert.match(ws[0].message, /`radioGroup`/);
  assert.match(ws[1].message, /`tabList`/);
});

test('<slider> proposes values from the keys, snapped to its step, and needs a label (ADR 0034)', () => {
  const ctx = new Ctx();
  const view = (v) => box({ pad: 10 }, [
    el('slider', { label: 'Gain', valueNow: v, valueMin: 0, valueMax: 1, valueStep: 0.1, onChange: { kind: 'gain' } }),
  ]);
  ctx.frame(320, 240, 1, view(0.2));
  const gain = ctx.accessTree().nodes.find((n) => n.role === 'slider');
  assert.equal(gain.name, 'Gain');
  // The tree reads the declared f32 back as it is, as it does valueNow.
  assert.ok(Math.abs(gain.valueStep - 0.1) < 1e-6);
  ctx.focus(gain.key);
  ctx.key('right');
  const evs = ctx.pollEvents().filter((e) => e.payload.kind === 'change');
  assert.equal(evs.length, 1);
  assert.equal(evs[0].payload.value, 0.3, 'the decimal the step names, not 0.30000001');
  assert.equal(evs[0].payload.phase, 'end');
  assert.equal(evs[0].payload.tag.kind, 'gain');
  assert.throws(() => ctx.frame(320, 240, 1, box({}, [el('slider', { valueNow: 1 })])), /needs a label/);
});

test('<select> is the stock select: the click opens the menu under it, a row is one menu message on the field (F73)', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  const view = (current) => box({ pad: 10 }, [
    el('select', { label: 'language', options: ['English', 'Deutsch', { label: 'Latin', id: 'la' }], current }),
  ]);
  ctx.frame(320, 240, 1, view(0));
  const field = () => ctx.accessTree().nodes.find((n) => n.role === 'button' && n.name === 'language');
  assert.equal(field().description, 'English', 'described by the choice in force');
  assert.equal(field().expanded, false);
  assert.equal(ctx.menu(), null);
  // The click is the core's: nothing reaches the app, and the menu is open
  // under the field, about it, with the current row checked.
  const rect = field().rect;
  ctx.access('language', 'click');
  assert.deepEqual(ctx.pollEvents(), [], 'the field\'s click never reaches the app');
  const menu = ctx.menu();
  assert.ok(menu, 'the menu opened');
  assert.equal(menu.x, rect.x);
  assert.equal(menu.y, rect.y + rect.h);
  assert.deepEqual(menu.items.map((i) => i.label), ['English', 'Deutsch', 'Latin']);
  assert.deepEqual(menu.items.map((i) => i.checked), [true, false, false]);
  ctx.frame(320, 240, 1, view(0));
  assert.equal(field().expanded, true, 'and the field reads as expanded');
  // A row chosen — the way a host's own menu answers — is one `menu`
  // message on the field, naming the option: the label, or the item's id.
  assert.equal(ctx.activateMenuItem(2), true);
  const evs = ctx.pollEvents();
  assert.equal(evs.length, 1);
  assert.equal(evs[0].payload.kind, 'menu');
  assert.equal(evs[0].payload.item, 'la');
  assert.equal(ctx.menu(), null, 'chosen closes it');
  ctx.frame(320, 240, 1, view(2));
  assert.equal(field().description, 'Latin');
  assert.equal(field().expanded, false);
  assert.deepEqual(ctx.warnings(), []);
  // What the encoder refuses: no label, no options, a bad option, a bad
  // index; and a row the field does not read is dropped with the warning
  // that says it reads none.
  assert.throws(() => ctx.frame(320, 240, 1, box({}, [el('select', { options: ['a'] })])), /needs a label/);
  assert.throws(() => ctx.frame(320, 240, 1, box({}, [el('select', { label: 'x' })])), /needs options/);
  assert.throws(() => ctx.frame(320, 240, 1, box({}, [el('select', { label: 'x', options: [''] })])), /non-empty strings/);
  assert.throws(() => ctx.frame(320, 240, 1, box({}, [el('select', { label: 'x', options: ['a'], current: -1 })])), /option index/);
});

test('a disabled option is refused by activateMenuItem, and a select\'s options and current are checked (RG9, RG10)', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  const view = (options, current) => box({ pad: 10 }, [el('select', { label: 'language', options, current })]);
  // RG9, the probe as filed: the row is disabled, so the door answers as
  // the pointer would — false, nothing posted, the menu still open.
  ctx.frame(320, 240, 1, view(['English', { label: 'Latin', id: 'la', enabled: false }], 0));
  ctx.access('language', 'click');
  assert.ok(ctx.menu());
  assert.equal(ctx.activateMenuItem(1), false, 'a disabled row is refused');
  assert.deepEqual(ctx.pollEvents(), [], 'and posts nothing');
  assert.equal(ctx.menu().items.length, 2, 'the menu stays open');
  assert.equal(ctx.activateMenuItem(0), true);
  assert.equal(ctx.pollEvents()[0].payload.item, 'English');
  assert.equal(ctx.menu(), null);
  assert.equal(ctx.activateMenuItem(0), false, 'no menu open');
  assert.deepEqual(ctx.warnings(), []);
  // RG10: the one shared reader refuses an empty list.
  assert.throws(() => ctx.frame(320, 240, 1, view([])), /at least one option/);
  // `current` past the end is none, with one warning on the field.
  ctx.frame(320, 240, 1, view(['a', 'b'], 9));
  const field = () => ctx.accessTree().nodes.find((n) => n.role === 'button' && n.name === 'language');
  assert.equal(field().description, '');
  let warned = ctx.warnings();
  assert.equal(warned.length, 1, JSON.stringify(warned));
  assert.equal(warned[0].code, 'select-current-ignored');
  assert.equal(warned[0].key, ctx.keyOf('language'));
  assert.match(warned[0].message, /names option 9 counted from 0, and the field has 2 options/);
  ctx.frame(320, 240, 1, view(['a', 'b'], 9));
  assert.deepEqual(ctx.warnings(), [], 'once per field');
  // On a separator: none, and the divider is not checked — every row used
  // to grow a check gutter for a mark never drawn. A fresh context, since
  // the line is once per field and this is the same field.
  const ctx2 = new Ctx();
  ctx2.setDiagnostics(true);
  ctx2.frame(320, 240, 1, view(['a', { role: 'separator' }, 'b'], 1));
  assert.equal(ctx2.accessTree().nodes.find((n) => n.name === 'language').description, '');
  warned = ctx2.warnings();
  assert.equal(warned.length, 1, JSON.stringify(warned));
  assert.match(warned[0].message, /option 1 counted from 0, which is a separator/);
  ctx2.access('language', 'click');
  assert.deepEqual(ctx2.menu().items.map((i) => i.checked), [false, false, false]);
  // An unknown key on an option object warns as an unknown prop does,
  // under the row's name, with the value `disabled` was after — the row
  // reads `enabled`, so `disabled: true` was silently an enabled row.
  ctx.frame(320, 240, 1, view(['a', { label: 'b', disabled: true, Label: 'c' }], 0));
  warned = ctx.warnings();
  assert.deepEqual(warned.map((w) => w.code), ['unknown-prop', 'unknown-prop'], JSON.stringify(warned));
  assert.match(warned[0].message, /`disabled` is not a key of a menu item: a row takes `label`, `role`, `enabled`, `checked`, `id`, `accel`.*did you mean `enabled: false`/);
  assert.match(warned[1].message, /`Label` is not a key of a menu item.*did you mean `label`/);
  assert.deepEqual(protocol().menuItem, { name: 'menuItem', keys: ['label', 'role', 'enabled', 'checked', 'id', 'accel', 'items', 'replay'] });
  ctx.frame(320, 240, 1, view(['a', { label: 'b', disabled: true }], 0));
  assert.deepEqual(ctx.warnings(), [], 'once per key');
});

test('<box dir="table"> lines its rows\' cells up: a column is its widest cell, a grow cell grows it (ADR 0033)', () => {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  ctx.setInspect(true);
  ctx.frame(300, 200, 1, box({}, [box({ dir: 'table', width: 300 }, [
    box({ dir: 'row', width: 'grow', gap: 8 }, [
      text('ab', { size: 12 }),
      box({ width: 10, height: 10, bg: '#ff0000' }, [], 'b1'),
      box({ width: 'grow', height: 10, bg: '#00ff00' }, [], 'c1'),
    ], 'r1'),
    box({ dir: 'row', width: 'grow', gap: 8 }, [
      text('abcdef', { size: 12 }),
      box({ width: 50, height: 10, bg: '#ff0000' }, [], 'b2'),
      box({ width: 20, height: 10, bg: '#00ff00' }, [], 'c2'),
    ], 'r2'),
  ], 't')]));
  const nodes = ctx.nodes();
  const rect = (label) => nodes.find((n) => n.label === label).rect;
  const [b1, b2, c1, c2] = ['b1', 'b2', 'c1', 'c2'].map(rect);
  assert.equal(b1.x, b2.x, 'the fixed column starts after the longest label');
  assert.ok(b1.x > 8, `past a label and the gap: ${b1.x}`);
  assert.deepEqual([b1.w, b2.w], [50, 50], 'the fixed column is its widest cell');
  assert.equal(c1.x, c2.x);
  assert.equal(c1.w, c2.w, 'the grow column is one width in both rows');
  assert.ok(Math.abs(c1.x + c1.w - 300) < 0.01, `and it takes the rest: ${c1.x + c1.w}`);
  const t = nodes.find((n) => n.label === 't');
  assert.equal(t.dir, 'column', 'a table is a column');
  assert.equal(t.table, true, 'that says so');
  assert.equal(nodes.find((n) => n.label === 'r1').table, false);
  assert.deepEqual(ctx.warnings(), []);
});

test('env() is on both classes and setEnv is only on the headless one', () => {
  // A window's runner reports the real window every frame, so a fact set on
  // one would be overwritten before the next view ran; the read is shared.
  assert.equal(typeof Ctx.prototype.env, 'function');
  assert.equal(typeof KuiWindow.prototype.env, 'function');
  assert.equal(typeof Ctx.prototype.setEnv, 'function');
  assert.equal(KuiWindow.prototype.setEnv, undefined);
});

test("a window's icon reaches the constructor, and pixels that are not the size are refused (F86)", () => {
  // Every window's icon is the launcher's (the title bar, Alt-Tab and the
  // taskbar on Windows, X11's window manager). A window needs a display,
  // so a headless run checks the hand-over and the refusals, which come
  // before anything opens.
  const icon = { rgba: new Uint8Array(2 * 2 * 4), width: 2, height: 2, resource: 1 };
  assert.equal(windowOptions({ title: 'x', icon }).icon, icon, 'runWindowed passes it by reference');
  assert.equal(windowOptions({}).icon, undefined);
  assert.throws(() => new KuiWindow('t', { icon: { ...icon, rgba: new Uint8Array(15) } }), /icon: .*(divisible by 4|dimensions)/);
  assert.throws(() => new KuiWindow('t', { icon: { rgba: new Uint8Array(0), width: 0, height: 0 } }), /at least one pixel/);
  assert.throws(() => new KuiWindow('t', { icon: { rgba: [0, 0, 0, 0], width: 1, height: 1 } }), /`rgba` must be a Uint8Array/);
  assert.throws(() => new KuiWindow('t', { icon: { rgba: new Uint8Array(4), width: 1 } }), /`rgba` \(a Uint8Array\), `width` and `height` go together/);
  assert.throws(() => new KuiWindow('t', { icon: { resource: 70000 } }), /resource id from 0 to 65535/);
  assert.throws(() => new KuiWindow('t', { icon: 'kawoosh.png' }), /icon must be \{rgba, width, height\}/);
});

test("runWindowed's `system` reaches the KuiWindow constructor (F47)", () => {
  // A window has no `setEnv` — its runner writes the real `env.system`
  // before every frame — so the pomodoro's reduced-motion branch was
  // assertable headless only. The pin goes in at the launcher instead:
  // `runWindowed(config, { system: { motion: 'reduced' } })` hands it to
  // `new KuiWindow(title, options)`, whose runner merges it over the OS's
  // reading inside that per-frame write. A window needs a display, so what
  // a headless run checks is the hand-over: `windowOptions` is the exact
  // object `runWindowed` constructs the window with, and the loop's own
  // options do not leak into it.
  const system = { motion: 'reduced' };
  const o = windowOptions({ title: 'x', pumpMs: 3, width: 320, chrome: 'custom', system, setup() {} });
  assert.equal(o.system, system, 'the same partial, by reference');
  assert.equal(o.width, 320);
  assert.equal(o.chrome, 'custom');
  assert.equal('title' in o, false);
  assert.equal('pumpMs' in o, false);
  assert.equal('setup' in o, false);
  // Left out is left out: the constructor sees no pin, not an empty one.
  assert.equal(windowOptions({ width: 1 }).system, undefined);
  assert.equal(windowOptions().system, undefined);
  // The type is `EnvInput['system']`, so a headless `Ctx` takes the same
  // partial — the branch a test asserts headless and the window it then
  // looks at read one spelling.
  const ctx = new Ctx();
  ctx.setEnv({ system });
  assert.equal(ctx.env().system.motion, 'reduced');
});


// -- The addon resolver -----------------------------------------------------
// `native.cjs` picks the newest of the prebuild and the two cargo profiles,
// and a candidate can exist without being loadable — most easily by building
// the workspace with `--all-targets`, which unifies kui-node's `napi/noop`
// dev-dependency feature into the cdylib and leaves one at target/debug/ that
// Node refuses with "Module did not self-register". Before this was handled,
// the raw dlopen failure escaped and `npm test` died before its first test,
// pointing at native.cjs rather than at the build that caused it.
//
// Driven in a child process because this one has the addon loaded already.

const HERE = dirname(fileURLToPath(import.meta.url));

function resolveWith(lib) {
  return spawnSync(process.execPath, ['-e', "require('./native.cjs')"], {
    cwd: HERE,
    env: { ...process.env, KUI_NODE_LIB: lib },
    encoding: 'utf8',
  });
}

test('an unloadable native library is reported, not thrown raw from the resolver', () => {
  const dir = mkdtempSync(join(tmpdir(), 'kui-resolver-'));
  const bogus = join(dir, 'kui_node.node');
  writeFileSync(bogus, 'this is not a shared library');
  try {
    const r = resolveWith(bogus);
    assert.notEqual(r.status, 0, 'a library that cannot load is still a failure');
    assert.match(r.stderr, /found but not loadable/, 'says what went wrong');
    assert.ok(r.stderr.includes(bogus), `names the file it tried: ${r.stderr}`);
    assert.match(r.stderr, /cargo build -p kui-node/, 'says how to get a good one');
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});

test('a native library that is not there is reported as missing, not as broken', () => {
  const r = resolveWith(join(tmpdir(), 'kui-nothing-is-here.node'));
  assert.notEqual(r.status, 0);
  assert.match(r.stderr, /not found for/);
  assert.doesNotMatch(r.stderr, /not loadable/);
});

test('the addon is loaded RTLD_GLOBAL, so a C extension can resolve `kui_*` against it', (t) => {
  // A plugin leaves the whole API undefined and the dynamic linker resolves
  // it against what the process has already loaded — but only what is in the
  // *global* scope. glibc's dlopen defaults to RTLD_LOCAL, so on Linux the
  // addon provided nothing and `ctx.addExtension` failed with `undefined
  // symbol: kui_slot_params`; Darwin defaults to RTLD_GLOBAL, which is why
  // the round trip above passed here and only CI saw it.
  //
  // Asserted through the flags native.cjs asks for rather than through a
  // load, because the load this governs succeeds on this platform either
  // way: a revert would go green on macOS and red only on Linux.
  //
  // Windows has no dlopen flags at all — `os.constants.dlopen` is `{}` —
  // and native.cjs then calls `process.dlopen` with none, which is what its
  // own comment promises. There is nothing to assert here on that platform,
  // and this test first ran there after alpha.10 had already shipped it.
  const { RTLD_GLOBAL, RTLD_LAZY } = osConstants.dlopen;
  if (RTLD_GLOBAL === undefined || RTLD_LAZY === undefined) {
    t.skip(`${process.platform} offers no dlopen flags; native.cjs passes none`);
    return;
  }
  const r = spawnSync(
    process.execPath,
    [
      '-e',
      `const real = process.dlopen.bind(process);
       process.dlopen = (m, f, flags) => { console.log('FLAGS', flags); return real(m, f, flags); };
       require('./native.cjs');`,
    ],
    { cwd: HERE, encoding: 'utf8' },
  );
  assert.equal(r.status, 0, `the resolver failed: ${r.stderr}`);
  const flags = Number(r.stdout.match(/FLAGS (\d+)/)?.[1]);
  assert.ok(Number.isFinite(flags), `no dlopen flags recorded: ${r.stdout}${r.stderr}`);
  assert.equal(flags & RTLD_GLOBAL, RTLD_GLOBAL, 'RTLD_GLOBAL, or a plugin resolves nothing on glibc');
  assert.equal(flags & RTLD_LAZY, RTLD_LAZY, "RTLD_LAZY, which is Node's own default");
});

// ---------------------------------------------------------------------------
// Effects as data (docs/adr/0013-effects-as-data.md, backlog F23)

test('update may return withEffects: the model lands, the effects queue, and effects() drains them (ADR 0013)', () => {
  const app = createApp(
    {
      init: { n: 0 },
      update: (m, msg) => {
        if (msg === 'save') return withEffects({ n: m.n + 1 }, { kind: 'write', path: 'a.txt' });
        if (msg === 'ping') return withEffects(undefined, { kind: 'beep' });
        return m;
      },
      view: (m) => box({ pad: 0 }, [text(`n=${m.n}`, { size: 12 })]),
    },
    { width: 320, height: 240 },
  );
  app.render();
  assert.deepEqual(app.effects(), [], 'nothing until update says so');
  app.dispatch('save');
  assert.equal(app.model.n, 1, 'the model landed');
  assert.deepEqual(app.effects(), [{ kind: 'write', path: 'a.txt' }], 'and the effect is readable');
  assert.deepEqual(app.effects(), [], 'a drain is a drain');
  app.dispatch('ping');
  assert.equal(app.model.n, 1, 'withEffects(undefined, …) keeps the model');
  assert.deepEqual(app.effects(), [{ kind: 'beep' }], 'and still queues the effect');
  // The reason for the brand: an array model is a tuple already, so a
  // plain return of one has to be the model and nothing else.
  const list = createApp(
    { init: [1], update: (m, msg) => [...m, msg], view: () => box({ pad: 0 }) },
    { width: 320, height: 240 },
  );
  list.dispatch(2);
  assert.deepEqual(list.model, [1, 2]);
  assert.deepEqual(list.effects(), []);
});

test('an effects handler runs after the frame, its dispatch lands in the next turn, and init may return effects (ADR 0013)', () => {
  const log = [];
  const app = createApp(
    {
      // The effect an app starts with: Elm's init returns a Cmd too.
      init: () => withEffects({ n: 0, loaded: false }, { kind: 'load' }),
      update: (m, msg) => {
        if (msg === 'save') return withEffects({ ...m, n: m.n + 1 }, { kind: 'write' });
        if (msg === 'loaded') return { ...m, loaded: true };
        return m;
      },
      view: (m) => {
        log.push(`draw n=${m.n} loaded=${m.loaded}`);
        return box({ pad: 0 });
      },
    },
    {
      width: 320,
      height: 240,
      effects: (effect, dispatch, surface) => {
        log.push(`effect ${effect.kind}`);
        assert.equal(typeof surface.quads, 'function', "the surface is the loop's");
        if (effect.kind === 'load') dispatch('loaded');
      },
    },
  );
  assert.deepEqual(log, [], 'nothing runs before the first frame');
  app.render();
  assert.deepEqual(log, ['draw n=0 loaded=false', 'effect load'], 'after the frame, not before it');
  assert.equal(app.model.loaded, true, 'the dispatch went through update…');
  app.render();
  assert.equal(log.at(-1), 'draw n=0 loaded=true', '…and reached the screen on the next frame');
  assert.deepEqual(app.effects(), [{ kind: 'load' }], 'the drain has it whether or not a handler ran');
  app.dispatch('save');
  assert.ok(!log.includes('effect write'), 'queued until a frame');
  app.render();
  assert.equal(log.at(-1), 'effect write');
  assert.deepEqual(app.effects(), [{ kind: 'write' }]);
});

test("a dispatch the loop did not make itself — an effect handler's, a timer's — is a frame on the next step (ADR 0013)", () => {
  let draws = 0;
  const app = createApp(
    {
      init: { loaded: false, n: 0 },
      update: (m, msg) => {
        if (msg === 'go') return withEffects(m, { kind: 'load' });
        if (msg === 'loaded') return { ...m, loaded: true };
        if (msg === 'bump') return { ...m, n: m.n + 1 };
        return undefined;
      },
      view: (m) => {
        draws += 1;
        return box({ pad: 0 });
      },
    },
    {
      width: 320,
      height: 240,
      effects: (effect, dispatch) => {
        if (effect.kind === 'load') dispatch('loaded');
      },
    },
  );
  app.render();
  assert.equal(app.step(), false, 'a quiet turn');
  app.dispatch('go');
  assert.equal(app.step(), true, 'the model the dispatch changed is drawn…');
  assert.equal(app.model.loaded, true, '…and the effect ran after that frame, dispatching');
  assert.equal(app.step(), true, "the handler's dispatch is a frame of its own, not a wait for the next OS event");
  assert.equal(app.step(), false, 'and then it is quiet again');
  app.dispatch('bump');
  assert.equal(app.step(), true, "the app's own dispatch from outside the loop (a timer, a promise) draws too");
  const before = draws;
  app.dispatch('nothing');
  assert.equal(app.step(), false, 'an update that returned undefined changed nothing, so no frame');
  assert.equal(draws, before);
});

test('step() reports whether the turn drew, which is what paces the windowed driver', () => {
  let t = 0;
  const app = createApp(
    {
      init: { n: 0 },
      update: (m, msg) => (msg === 'tick' ? { n: m.n + 1 } : m),
      view: () => box({ pad: 0 }),
      tick: { every: 100, msg: 'tick' },
    },
    { width: 320, height: 240, clock: () => t },
  );
  app.render();
  assert.equal(app.step(), false, 'a turn with nothing owed says so');
  t = 100;
  assert.equal(app.step(), true, 'a tick that changed the model drew');
  assert.equal(app.step(), false, 'and the next turn is quiet again');
});

test('a tick that returns effects and no model hands them on without a frame (ADR 0013)', () => {
  const seen = [];
  let draws = 0;
  let t = 0;
  const app = createApp(
    {
      init: {},
      update: (m, msg) => (msg === 'tick' ? withEffects(undefined, { kind: 'poll' }) : m),
      view: () => {
        draws += 1;
        return box({ pad: 0 });
      },
      tick: { every: 100, msg: 'tick' },
    },
    { width: 320, height: 240, clock: () => t, effects: (e) => seen.push(e.kind) },
  );
  app.render();
  const before = draws;
  t = 100;
  app.step();
  assert.equal(draws, before, 'a tick without a model draws nothing, as before');
  assert.deepEqual(seen, ['poll'], 'but the step that owed the effect handed it on');
  assert.deepEqual(app.effects(), [{ kind: 'poll' }]);
});

// -- Extensions (ADR 0014) --------------------------------------------------
//
// The mechanism is C shared libraries and only that: a plugin is a .so /
// .dylib / .dll exporting the seven kui_ext_* entry points kui.h describes.
// There is no script-loads-script path — a Lua extension is loaded by a Rust
// host or not at all.

test('a slot places its node with nothing loaded, and rejects what it is not', () => {
  const ctx = new Ctx();
  assert.deepEqual(ctx.extensionNamespaces(), []);

  // A slot with no extension behind it still places a node, so a view can
  // declare its layout before it has a plugin to put in it.
  const enc = createEncoder(protocol());
  const tree = box({ pad: 0 }, [el('slot', { name: 'todos/panel' })]);
  const { stream, strings } = enc.encode(tree);
  ctx.frameBinary(320, 240, 1, stream, strings);
  assert.equal(ctx.warnings().length, 0, 'an unfilled slot is not a warning');

  // A position, not a box.
  assert.throws(() => enc.encode(box({}, [el('slot', {})])), /needs a name/);
  assert.throws(
    () => enc.encode(box({}, [el('slot', { name: 'panel' })])),
    /a full "namespace\/slot"/,
  );
  assert.throws(
    () => enc.encode(box({}, [el('slot', { name: 'todos/panel', bg: '#fff' })])),
    /takes name, params, keep and replay/,
  );
});

test('addExtension reports why a library is not a plugin, and keeps nothing', () => {
  const ctx = new Ctx();
  // The platform's own C runtime: loads, and declares no kui_ext_abi, which
  // is the plugin built against a header from before the symbol existed.
  const libc =
    process.platform === 'win32'
      ? 'kernel32.dll'
      : process.platform === 'darwin'
        ? '/usr/lib/libSystem.B.dylib'
        : 'libc.so.6';
  assert.throws(() => ctx.addExtension('libc', libc), /declares no ABI/);
  assert.throws(() => ctx.addExtension('nope', 'no/such/library'), /./);
  assert.deepEqual(ctx.extensionNamespaces(), [], 'a refusal keeps nothing');
});

// The whole round trip needs a real plugin, which needs a C compiler, so it
// runs only where `cargo run -p kui-devtools --bin cbuild` has been run. The C
// half of the same check is examples/c/features/slots/host.c's --headless, which CI runs.
test('a C extension fills the slot the view declares, and its reply comes back', (t) => {
  // Where both build scripts leave it, whichever profile was built: on
  // Windows this is the shape that imports kui_ffi.dll rather than a host
  // executable, which is the only one an addon can load.
  const name = process.platform === 'win32' ? 'panel.dll' : 'panel.so';
  const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
  const plugin = ['debug', 'release']
    .map((p) => join(root, 'target', p, name))
    .find(existsSync);
  if (!plugin) {
    t.skip(`build examples/c first (no target/*/${name})`);
    return;
  }
  const ctx = new Ctx();
  ctx.addExtension('todos', plugin);
  assert.deepEqual(ctx.extensionNamespaces(), ['todos']);

  const enc = createEncoder(protocol());
  const draw = () => {
    // The root fills, or the panel's own grow has nothing to grow into
    // and its list collapses to nothing - which is what the first run of
    // this test found.
    const tree = box({ width: 'grow', height: 'grow', pad: 0 }, [
      el('slot', {
        name: 'todos/panel',
        params: { title: 'todos, from Node', on_toggle: { kind: 'toggled' } },
      }),
    ]);
    const { stream, strings } = enc.encode(tree);
    ctx.frameBinary(900, 600, 1, stream, strings);
  };
  draw();

  // A row the plugin drew, found the way panel.rs and host.c find it: by
  // origin and name, off the access tree the frame published.
  const row = ctx
    .accessTree()
    .nodes.find((n) => n.origin === 1 && typeof n.name === 'string' && n.name.startsWith('[ '));
  assert.ok(row, 'the plugin drew no rows — did the slot fill?');

  ctx.cursor(row.rect.x + row.rect.w / 2, row.rect.y + row.rect.h / 2);
  ctx.mouse(true);
  ctx.mouse(false);
  draw();

  const events = ctx.pollEvents();
  const replies = events.filter((e) => e.payload?.kind === 'toggled');
  assert.equal(replies.length, 1, 'the plugin replied once');
  assert.equal(replies[0].origin, 1, 'and the reply carries the plugin’s origin');
  // And which slot the row was filled into — the reply keeps the slot of
  // the event it answers (backlog K2). `keyOf` answers the slot's key.
  assert.equal(replies[0].slot, ctx.keyOf('todos/panel'), 'the reply names the slot it came from');
  assert.ok(
    !events.some((e) => e.payload?.kind === 'toggle'),
    'the plugin’s own event stayed with the plugin',
  );
});

// A slot replayed by its host (ADR 0045), through the same C plugin.
test('a replayed slot spares the plugin, and says why when it cannot (ADR 0045)', (t) => {
  const name = process.platform === 'win32' ? 'panel.dll' : 'panel.so';
  const root = join(dirname(fileURLToPath(import.meta.url)), '..', '..');
  const plugin = ['debug', 'release']
    .map((p) => join(root, 'target', p, name))
    .find(existsSync);
  if (!plugin) {
    t.skip(`build examples/c first (no target/*/${name})`);
    return;
  }
  const ctx = new Ctx();
  ctx.addExtension('todos', plugin);
  const enc = createEncoder(protocol());
  const draw = (title, how) => {
    const tree = box({ width: 'grow', height: 'grow', pad: 0 }, [
      el('slot', { name: 'todos/panel', params: { title }, ...how }),
    ]);
    const { stream, strings } = enc.encode(tree);
    ctx.frameBinary(900, 600, 1, stream, strings);
  };
  const row = () =>
    ctx
      .accessTree()
      .nodes.find((n) => n.origin === 1 && typeof n.name === 'string' && n.name.startsWith('[ '));

  draw('todos', { keep: true });
  assert.ok(row(), 'the plugin drew its rows');
  assert.equal(ctx.slotFill('todos/panel'), null, 'nothing asked yet');
  draw('todos', { replay: true });
  assert.equal(ctx.slotFill('todos/panel'), 'replayed');
  assert.ok(row(), 'the rows are there, replayed');
  draw('todos', { replay: true });
  assert.equal(ctx.slotFill('todos/panel'), 'replayed');

  // Other params: the plugin runs.
  draw('chores', { replay: true });
  assert.equal(ctx.slotFill('todos/panel'), 'params');
  draw('chores', { replay: true });
  assert.equal(ctx.slotFill('todos/panel'), 'replayed');

  // A plain frame forgets what was kept.
  draw('chores', {});
  draw('chores', { replay: true });
  assert.equal(ctx.slotFill('todos/panel'), 'not-kept');

  // The pointer onto a row the plugin hover-tracks: a replay still, since
  // the row's hover is declared, not read — and the row paints hovered.
  const r = row();
  ctx.cursor(r.rect.x + r.rect.w / 2, r.rect.y + r.rect.h / 2);
  draw('chores', { replay: true });
  assert.ok(['replayed', 'reads'].includes(ctx.slotFill('todos/panel')));

  assert.throws(
    () => enc.encode(box({}, [el('slot', { name: 'todos/panel', keep: true, replay: true })])),
    /keep or replay, not both/,
  );
});

test('an event names the slot its node was filled into, and null for the app’s own (K2)', () => {
  // Without a plugin: the stamp is the core's, from the fill ranges the
  // frame recorded, so a host's own node reads null and nothing else
  // changes shape. (The C plugin test above reads a real slot.)
  const ctx = new Ctx();
  ctx.frame(320, 240, 1, box({ pad: 4 }, [box({ width: 40, height: 20, onClick: 'hit' }, [], 'b')]));
  ctx.cursor(20, 20);
  ctx.mouse(true);
  ctx.mouse(false);
  const [ev] = ctx.pollEvents();
  assert.equal(ev.payload, 'hit');
  assert.deepEqual(Object.keys(ev).sort(), ['key', 'origin', 'payload', 'slot', 'window']);
  assert.equal(ev.slot, null);
});

test('a text size or line height of nothing frames, and dir null is absent', () => {
  // A size or line height of 0 aborted the process (cosmic-text asserts a
  // line height is not 0) and a negative one spun its layout; null dir threw.
  const ctx = new Ctx();
  for (const props of [{ lineHeight: 0 }, { size: 0 }, { size: 0.3 }, { size: -1 }, { lineHeight: -0.5 }]) {
    ctx.frame(320, 240, 1, { type: 'text', props, children: ['hi'] });
    ctx.frame(320, 240, 1, { type: 'edit', props: { ...props, id: 'e', initial: 'x' }, children: [] });
    const m = ctx.measureText('hi', props);
    assert.ok(Number.isFinite(m.width) && Number.isFinite(m.height), JSON.stringify(props));
  }
  ctx.frame(320, 240, 1, { type: 'box', props: { dir: null }, children: ['x'] });
});

test('measureText inside a devtoolsTab function child leaves the frame being encoded alone', () => {
  // The tab's child is called mid-encode; measureText shared the frame's
  // encoder and reset its buffers ("binary frame must start with the root op").
  const ctx = new Ctx();
  const el = (type, props = {}, children = []) => ({ type, props, children });
  const tab = () => el('text', {}, [`w=${Math.round(ctx.measureText('measured', { size: 14 }).width)}`]);
  const tree = () => el('box', { title: 'Root' }, [el('devtoolsTab', { name: 'mine', label: 'Mine' }, [tab]), el('text', {}, ['after the tab'])]);
  ctx.setDevtools(true);
  ctx.frame(800, 600, 1, tree());
  ctx.setDevtoolsTab('mine');
  ctx.frame(800, 600, 1, tree());
  const access = JSON.stringify(ctx.accessTree());
  assert.ok(access.includes('after the tab') && access.includes('w='));
  assert.equal(ctx.windowTitle(), 'Root');
});

test('a message comes back as it was sent: a whole number past 2^53, a string cut through an emoji', () => {
  // The number came back a BigInt; the lone surrogate failed the frame.
  const ctx = new Ctx();
  const big = 9007199254740994;
  const cut = 'ab😀'.slice(0, 3);
  const frame = (onClick) => ctx.frame(200, 100, 1, { type: 'box', props: { width: 200, height: 100, onClick }, children: [] });
  for (const [sent, back] of [[big, big], [{ s: cut }, { s: 'ab�' }]]) {
    frame(sent);
    ctx.cursor(10, 10);
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    const evs = ctx.pollEvents();
    assert.equal(evs.length, 1);
    assert.deepEqual(evs[0].payload, back);
  }
});

test('a select option or a menu bar row cut through an emoji draws U+FFFD, not a failed frame', () => {
  // Both ride as JSON like a message, and RG138's repair missed them.
  const cut = 'ab😀'.slice(0, 3);
  const el = (type, props = {}, children = []) => ({ type, props, children });
  for (const node of [
    el('select', { label: 'f', options: [cut, 'ok'], current: 0 }),
    el('menuBar', { menu: [{ label: cut, items: [{ label: cut }] }] }),
  ]) {
    const ctx = new Ctx();
    ctx.frame(300, 200, 1, el('box', {}, [node]));
    assert.ok(JSON.stringify(ctx.accessTree()).includes('ab�'), node.type);
  }
});
