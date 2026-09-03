// Transport parity: the same tree through frame() (binary stream), frameJson
// and frameObject must yield byte-identical quads — for every schema prop,
// the hand-written composites, and every element. The Rust side pins each
// decoder to the schema; this pins the JS encoder to the addon's JSON path.
// Needs the addon built: npm run build:native. Run: npm test
import test from 'node:test';
import assert from 'node:assert/strict';
import { existsSync, readFileSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { Ctx, createApp, decodeQuads, protocol, quadStride } from './index.js';

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

// Modal surfaces (docs/adr/0003-modal-surfaces.md): the dialog takes focus
// and keeps it, the app behind it is inert, and Escape and a press outside
// both ask it to close — the same on every transport.
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
  for (const transport of ['binary', 'json', 'object']) {
    const { ctx } = run(transport, build);
    const tree = ctx.accessTree();
    const dialog = tree.nodes.find((n) => n.modal);
    assert.ok(dialog, `${transport}: the access tree reports the modal`);
    assert.equal(dialog.role, 'dialog', `${transport}: a modal box is a dialog`);
    assert.equal(dialog.name, 'Settings');
    const ok = tree.nodes.find((n) => n.name === 'OK');
    assert.equal(tree.focus, ok.key, `${transport}: focus entered the modal`);
    assert.equal(ctx.focused(), ok.key);

    // The one stop in the ring is inside the dialog: Tab cannot leave.
    ctx.key('tab');
    assert.equal(ctx.focused(), ok.key, `${transport}: tab stays inside`);

    // A press on the button behind emits no click, only the dismiss.
    ctx.cursor(10, 10);
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    const outside = ctx.pollEvents();
    assert.equal(outside.length, 1, `${transport}: the button behind is inert`);
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
    assert.equal(escaped[0].key, dialog.key, `${transport}: on the modal node`);
    assert.equal(ctx.focused(), ok.key, `${transport}: escape does not let go`);
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

  // Nothing routes the middle button yet.
  ctx.mouse(true, 1, 'middle');
  ctx.mouse(false, 1, 'middle');
  assert.deepEqual(ctx.pollEvents(), []);
  assert.throws(() => ctx.mouse(true, 1, 'left'), /unknown mouse button/);
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
            width: 180, height: 40,
          },
          [text('ab', { size: 12 }), text('cd', { size: 12 })],
          'card',
        ),
        text(
          ['a ', el('span', { bold: true, color: '#73d98c' }, ['b']), el('span', { italic: true }, [' c'])],
          { size: 13 },
        ),
      ]),
    ]),
  overflow: () =>
    root({}, [
      box({ pad: 4, clip: true }, [
        box(
          { width: 120, height: 60, gap: 4, scrollY: true, bg: '#101018' },
          ITEM_KEYS.map((k) => box({ width: 100, height: 20, bg: '#30344a' }, [], k)),
          'list',
        ),
      ]),
    ]),
  float: () =>
    root({}, [
      box({ pad: 20, gap: 4 }, [
        box({ width: 80, height: 24, bg: '#333333' }, [
          box({ float: 'below', width: 40, height: 12, bg: '#ff0000' }),
        ], 'anchor'),
        box({
          float: { anchor: 'viewport', at: ['end', 'end'], self: ['end', 'end'], dx: -4, dy: -4, fit: true },
          width: 10, height: 10, bg: '#00ff00',
        }),
      ]),
    ]),
  tooltip: () =>
    root({}, [
      box({ pad: 10 }, [
        box(
          { dir: 'row', width: 100, height: 40, bg: '#333333', role: 'group', tooltip: 'a hint' },
          [text('badge', { size: 12 })],
          'tip',
        ),
      ]),
    ]),
  chrome: () =>
    root({ title: 'kui conformance' }, [
      box({ gap: 6 }, [
        el('titlebar', {}, [text('app', { size: 12 }), el('windowButtons')]),
        box({ width: 40, height: 16, bg: '#22242c', focusable: true, keyFocus: true, label: 'Sink' }, [], 'sink'),
      ]),
    ]),
  controls: () =>
    root({}, [
      box({ pad: 10, gap: 6, onContextMenu: { kind: 'menu' } }, [
        el('button', { onClick: { kind: 'go' } }, ['go']),
        el('edit', { initial: 'hello', size: 13, width: 160, label: 'Note' }, [], 'note'),
      ]),
    ]),
  media: (ctx) =>
    root({}, [
      box({ pad: 6, gap: 4 }, [
        el('image', { src: addFixtureImage(ctx), width: 16, radius: 2 }),
        el('audio', { src: addFixtureSound(ctx), volume: 0.5, loop: true }, [], 'music'),
        el('latencyGraph'),
      ]),
    ]),
};

const ITEM_KEYS = ['i0', 'i1', 'i2', 'i3', 'i4', 'i5'];
/** A root box sized like the core's implicit root: `configure_root` with
 *  the same data it already has, so only `title` actually lands. */
const root = (props, children) => box({ width: 'grow', height: 'grow', ...props }, children);
/** The corpus fixtures, byte-identical to `conformance::image_pixels` /
 *  `SOUND_BYTES` so the handles and the atlas come out the same. */
const addFixtureImage = (ctx) => ctx.addImage(4, 4, Buffer.alloc(4 * 4 * 4, 0xff));
const addFixtureSound = (ctx) => ctx.addSound(Buffer.from('RIFF....WAVE'));

const FNV_OFFSET = 0xcbf29ce484222325n;
const FNV_PRIME = 0x100000001b3n;
const MASK = 0xffffffffffffffffn;

/** FNV-1a over each quad's words 0..17 and 22..25 — `KuiQuad` without its
 *  `uv`, which depends on glyph insertion order. Mirrors
 *  `conformance::quad_digest`. */
function quadDigest(buffer) {
  const stride = quadStride();
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  let h = FNV_OFFSET;
  for (let off = 0; off + stride <= buffer.byteLength; off += stride) {
    for (const i of [...Array(18).keys(), 22, 23, 24, 25]) {
      let word = BigInt(view.getUint32(off + i * 4, true));
      for (let b = 0; b < 4; b++) {
        h = (h ^ (word & 0xffn)) & MASK;
        h = (h * FNV_PRIME) & MASK;
        word >>= 8n;
      }
    }
  }
  return h.toString(16).padStart(16, '0');
}

/** The protocol every binding drives: a frame, then each replayed step
 *  followed by another frame, then the last frame's output. Mirrors
 *  `conformance::drive`. */
function driveScene(transport, steps, build) {
  const ctx = new Ctx();
  ctx.setDiagnostics(true);
  const tree = build(ctx);
  const events = [];
  const frame = () => {
    if (transport === 'binary') ctx.frame(320, 240, 1, tree);
    else if (transport === 'json') ctx.frameJson(320, 240, 1, JSON.stringify(tree));
    else ctx.frameObject(320, 240, 1, tree);
    events.push(...ctx.pollEvents());
  };
  frame();
  for (const step of steps) {
    if (step[0] === 'cursor') ctx.cursor(step[1], step[2]);
    else if (step[0] === 'cursorleft') ctx.cursorLeft();
    else if (step[0] === 'mousedown') ctx.mouse(true, 1);
    else if (step[0] === 'mouseup') ctx.mouse(false);
    else if (step[0] === 'secondarydown') ctx.mouse(true, 1, 'secondary');
    else if (step[0] === 'secondaryup') ctx.mouse(false, 1, 'secondary');
    else if (step[0] === 'scroll') ctx.scroll(step[1], step[2]);
    else throw new Error(`unknown conformance step ${step[0]}`);
    events.push(...ctx.pollEvents());
    frame();
  }
  return { ctx, events };
}

/** Renders a scene block in the report format `conformance::report`
 *  documents: integers, hex and strings only, so the bytes match Rust's. */
function sceneReport(name, steps, { ctx, events }) {
  const lines = [`scene ${name}`];
  for (const step of steps) lines.push(`step ${step.join(' ')}`);
  lines.push(`title ${ctx.windowTitle() ?? '-'}`);
  const quads = Buffer.from(ctx.quads());
  const stride = quadStride();
  const count = quads.byteLength / stride;
  lines.push(`quads ${count} ${quadDigest(quads)}`);
  const kinds = [0, 0, 0, 0, 0];
  for (let off = 0; off < quads.byteLength; off += stride) kinds[quads.readUInt32LE(off + 17 * 4)]++;
  lines.push(`kinds ${kinds.join(' ')}`);
  const depth = new Map();
  for (const n of ctx.accessTree().nodes) {
    const d = n.parent === null ? 0 : depth.get(n.parent) + 1;
    depth.set(n.key, d);
    lines.push(
      [
        'node', d, n.key, n.role,
        n.focused ? 1 : 0,
        n.disabled ? 1 : 0,
        n.checked === null || n.checked === undefined ? '-' : n.checked ? 1 : 0,
        n.scroll ? 1 : 0,
        n.actions.length ? n.actions.join(',') : '-',
        `${n.name ?? ''} | ${n.description ?? ''} | ${n.value ?? ''}`,
      ].join(' '),
    );
  }
  for (const ev of events) {
    lines.push(`event ${ev.payload?.kind ?? '-'} ${ev.payload?.tag?.kind ?? '-'}`);
  }
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
    if (line.startsWith('scene ')) cur = { name: line.slice(6), steps: [], lines: [] };
    if (!cur) continue;
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

test('every corpus scene lowers the way kui-core does', (t) => {
  if (!existsSync(CONFORMANCE)) {
    t.skip(
      `no reference report at ${CONFORMANCE} — generate it with ` +
        '`cargo run -p kui-core --example conformance-dump -- target/conformance.txt`',
    );
    return;
  }
  const blocks = referenceBlocks(readFileSync(CONFORMANCE, 'utf8'));
  assert.ok(blocks.length > 0, 'the reference report has no scenes');
  assert.deepEqual(
    Object.keys(SCENE_TREES).sort(),
    blocks.map((b) => b.name).sort(),
    'the JSX scenes and the corpus have drifted apart',
  );
  for (const { name, steps, block } of blocks) {
    const build = SCENE_TREES[name];
    assert.ok(build, `no JSX scene for ${name} — every corpus scene needs one`);
    // The three encoders still have to agree with each other on frame one;
    // that is what assertParity is for. The reference says what that frame
    // has to *be*.
    assertParity(name, build);
    for (const transport of ['binary', 'json', 'object']) {
      const actual = sceneReport(name, steps, driveScene(transport, steps, build));
      assert.equal(actual, block, `scene ${name} lowers differently on the ${transport} transport`);
    }
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

test('reveal of a key the next frame does not declare is a no-op', () => {
  const { ctx, render, list } = listCtx();
  ctx.reveal('0123456789abcdef');
  render();
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 0 });
  // Not remembered either: the request is spent by the frame that could
  // not find it, so a later frame does not act on it.
  render();
  assert.deepEqual(ctx.scrollOffset(list), { x: 0, y: 0 });
  assert.throws(() => ctx.reveal('nope'), /bad id/);
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
