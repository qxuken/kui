// Transport parity: the same tree through frame() (binary stream), frameJson
// and frameObject must yield byte-identical quads — for every schema prop,
// the hand-written composites, and every element. The Rust side pins each
// decoder to the schema; this pins the JS encoder to the addon's JSON path.
// Needs the addon built: npm run build:native. Run: npm test
import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync } from 'node:fs';
import { Ctx, createApp, decodeQuads, protocol } from './index.js';

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
  tag: { kind: 't' },
  str: 'group-a',
  resource: '0000000000000007',
  keyframes: [{ width: { grow: 0 }, bg: '#112233' }, { at: 0.75, width: 'grow', height: '50%', radius: 9 }],
  enter: { dx: -40, dy: 8, width: { grow: 0 }, bg: '#11223300', radius: 0 },
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
      el('audio', { src: ctx.addSound(Buffer.from('RIFF....WAVE')), loop: true, volume: 0.5, tag: { k: 1 } }, [], 'music'),
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
    if (b.quads.readUInt32LE(off + 17 * 4) === 3) images++;
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

// A null tag declares the behaviour (here: a key sink) and leaves `tag` off
// the event — the same on every transport, so a typed app needs no inert
// message for a sink that only needs the node key.
test('a null tag declares the behaviour without a tag on the event', () => {
  const build = () => box({}, [box({ onKey: null, keyFocus: true, width: 100, height: 50 }, [], 'sink')]);
  const payloads = ['binary', 'json', 'object'].map((transport) => {
    const { ctx } = run(transport, build);
    ctx.keyDown('a');
    const evs = ctx.pollEvents();
    assert.equal(evs.length, 1, `${transport}: the sink got the key`);
    return evs[0].payload;
  });
  for (const p of payloads) {
    assert.equal(p.kind, 'key');
    assert.equal(p.code, 'a');
    assert.ok(!('tag' in p), 'no tag field');
  }
  assert.deepEqual(payloads[0], payloads[1]);
  assert.deepEqual(payloads[0], payloads[2]);
});

// Keyboard focus as data: Tab reaches a button on every transport, a
// disabled box is not a stop, Enter presses the focused button, and the
// access tree reports the same focus.
test('tab reaches a button and enter presses it', () => {
  const build = () =>
    box({ pad: 4 }, [
      box({ width: 60, height: 20, bg: '#333333', onClick: { kind: 'go' } }, [text('go', { size: 12 })], 'go'),
      box({ width: 60, height: 20, bg: '#333333', focusable: true, disabled: true }, [], 'off'),
    ]);
  for (const transport of ['binary', 'json', 'object']) {
    const { ctx } = run(transport, build);
    assert.equal(ctx.focused(), null, `${transport}: nothing focused at first`);
    ctx.key('tab');
    const focused = ctx.focused();
    assert.ok(focused, `${transport}: tab landed on the button`);
    assert.ok(ctx.focusVisible(), `${transport}: keyboard focus shows`);
    assert.ok(ctx.isFocused(focused));
    const tree = ctx.accessTree();
    assert.equal(tree.focus, focused);
    const node = tree.nodes.find((n) => n.key === focused);
    assert.equal(node.role, 'button');
    assert.ok(node.focused && !node.disabled);
    assert.ok(node.actions.includes('focus'));
    const off = tree.nodes.find((n) => n.disabled);
    assert.equal(off.role, 'group', `${transport}: a focusable box is in the tree`);
    assert.ok(!off.actions.includes('focus'), `${transport}: disabled: not focusable`);
    ctx.key('tab');
    assert.equal(ctx.focused(), focused, `${transport}: the disabled box is not a stop`);
    ctx.key('enter');
    const evs = ctx.pollEvents();
    assert.equal(evs.length, 1, `${transport}: enter pressed the button`);
    assert.deepEqual(evs[0].payload, { kind: 'go' });
    ctx.blur();
    assert.equal(ctx.focused(), null);
    ctx.focus(focused);
    assert.equal(ctx.focused(), focused, `${transport}: focus(key) moves focus`);
    assert.ok(ctx.focusVisible(), `${transport}: programmatic focus keeps the keyboard modality`);
    ctx.focusNext();
    assert.equal(ctx.focused(), focused, `${transport}: the only stop wraps to itself`);
  }
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

// Layout is data: an onLayout node reports its rect on first sight and on
// change, identically on every transport.
test('onLayout reports the rect once and again when it changes', () => {
  const tree = (w) =>
    box({ dir: 'row', width: 'grow', height: 'grow' }, [
      box({ width: w, height: 'grow', onLayout: { kind: 'panel' } }, [], 'panel'),
    ]);
  for (const transport of ['binary', 'json', 'object']) {
    const { ctx } = run(transport, () => tree(100));
    const evs = ctx.pollEvents().filter((e) => e.payload.kind === 'layout');
    assert.equal(evs.length, 1, `${transport}: one layout event`);
    assert.deepEqual(evs[0].payload, {
      kind: 'layout',
      x: 0, y: 0, w: 100, h: 240,
      parent: { x: 0, y: 0, w: 320, h: 240 },
      tag: { kind: 'panel' },
    });
    ctx.frame(320, 240, 1, tree(100));
    assert.equal(ctx.pollEvents().length, 0, `${transport}: same rect, silence`);
    ctx.frame(320, 240, 1, tree(150));
    const again = ctx.pollEvents();
    assert.equal(again.length, 1);
    assert.equal(again[0].payload.w, 150);
  }
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
    if (quads.readUInt32LE(off + 17 * 4) !== 0) continue; // solid only
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

test('onHover emits enter and leave with the tag', () => {
  const ctx = new Ctx();
  const tree = box({ dir: 'row' }, [
    box({ width: 50, height: 50, onHover: { kind: 'hov', id: 'a' } }, [], 'a'),
    box({ width: 50, height: 50, onHover: { kind: 'hov', id: 'b' } }, [], 'b'),
  ]);
  ctx.frame(320, 240, 1, tree);
  ctx.cursor(10, 10);
  let evs = ctx.pollEvents();
  assert.deepEqual(evs.map((e) => e.payload), [{ kind: 'hover', phase: 'enter', tag: { kind: 'hov', id: 'a' } }]);
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
      if (quads.readUInt32LE(off + 17 * 4) !== 0) n++;
    }
    return n;
  };
  assert.ok(glyphs(box({}, [text('Fonts', { size: 20, font: id })])) > 0);
  assertParity('font prop', () => box({}, [text('Fonts', { size: 20, font: id })]));
  ctx.removeFont(id);
  // A stale handle falls back to sans instead of failing.
  assert.ok(glyphs(box({}, [text('Fonts', { size: 20, font: id })])) > 0);
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
      if (quads.readUInt32LE(off + 17 * 4) === 0) continue;
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
  assertParity('wrap props', () => box({ width: 120 }, [text(LONG, { wrap: 'none', maxLines: 2, ellipsis: true })]));
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
