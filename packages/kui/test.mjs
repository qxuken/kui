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
import { tmpdir } from 'node:os';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
import { Ctx, KuiWindow, createApp, createEncoder, decodeQuads, protocol, quadStride } from './index.js';

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
  msg: { kind: 'm', n: 1, list: [1, 'two', null] },
  tag: { kind: 't' },
  str: 'group-a',
  resource: '0000000000000007',
  keyframes: [
    { width: { grow: 0 }, bg: '#112233' },
    { at: 0.75, width: 'grow', height: '50%', radius: 9, opacity: 0.25 },
  ],
  enter: { dx: -40, dy: 8, width: { grow: 0 }, bg: '#11223300', radius: 0, opacity: 0 },
};

/** Per-prop overrides, where the shared sample for a kind would not survive
 *  the prop's own clamping — `opacity`'s default is the top of its range, so
 *  a sample above 1 clamps straight back to it. */
const SAMPLE_BY_NAME = { opacity: 0.5 };

test('protocol exports a version and the schema rows', () => {
  const p = protocol();
  assert.equal(typeof p.version, 'number');
  assert.ok(p.op.root >= 0 && p.op.end >= 0);
  assert.ok(Object.keys(p.prop).length > 20);
});

// Every prop writes at least its own id, so declaring one has to change the
// encoded bytes. That is the check the JSON transport used to provide by
// disagreeing: it catches the `switch` arm that never got written, or the
// name the encoder quietly falls through on.
test('every generic schema prop reaches the stream and lowers', () => {
  const { prop } = protocol();
  for (const [name, def] of Object.entries(prop)) {
    if (def.kind === 'custom') continue;
    const value =
      SAMPLE_BY_NAME[name] ?? (def.kind === 'enum' ? def.values[def.values.length - 1] : SAMPLE[def.kind]);
    assert.notEqual(value, undefined, `${name}: no sample for kind ${def.kind}`);
    const build = (props) =>
      def.target === 'style'
        ? box({ pad: 4, bg: '#101010' }, [text('sample', { size: 14, ...props })])
        : box({ pad: 4, bg: '#101010' }, [
            box({ width: 60, height: 20, bg: '#333333', ...props }, [text('x', { size: 12 })]),
          ]);
    assert.ok(
      !encoded(build({ [name]: value })).equals(encoded(build({}))),
      `${name}: the encoder dropped it — the stream is the same as without it`,
    );
    assertLowers(name, () => build({ [name]: value }));
  }
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
    [() => el('line', {}), /<line> needs from and to, or points/],
    [() => el('line', { points: [[0, 0]] }), /<line> needs at least two points/],
    [() => el('line', { from: [0, 0], to: [1], }), /bad point \[1\] for <line>/],
    [() => el('line', { from: [0, 0], to: [1, 1], width: 'grow' }), /bad width "grow" for <line>/],
    [() => box({ dir: 'diagonal' }), /bad dir "diagonal" \(row \| column\)/],
    [() => box({ mainAlign: 'middle' }), /bad value "middle" for mainAlign \(one of start \| center \| end\)/],
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
  // "bad id" named neither.
  assert.throws(() => ctx.focus('gamma'), /no node is keyed "gamma".*`key` prop.*hex key/);
  assert.throws(() => ctx.isFocused('gamma'), /no node is keyed/);
  assert.throws(() => ctx.reveal('gamma'), /no node is keyed/);
  assert.throws(() => ctx.access('gamma', 'click'), /no node is keyed/);
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
    tag: { kind: 'panel' },
  });
  ctx.frame(320, 240, 1, tree(100));
  assert.equal(ctx.pollEvents().length, 0, 'same rect, silence');
  ctx.frame(320, 240, 1, tree(150));
  const again = ctx.pollEvents();
  assert.equal(again.length, 1);
  assert.equal(again[0].payload.w, 150);
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

// The pointer shape is derived, not declared: `cursorShape()` reads what the
// core resolved for the node under the pointer, in the `cursor` prop's own
// words, so a driver applies it and a test asserts it. The bands and the
// expectations mirror crates/kui-core/tests/cursor.rs so both suites agree
// on the vocabulary.
test('cursorShape derives from what the node under the pointer does', () => {
  const ctx = new Ctx();
  const BAND = 40;
  const band = (props, key) => box({ width: 'grow', height: BAND, ...props }, [], key);
  // One full-width band per thing a cursor can be over, stacked, so a y
  // picks one.
  const tree = box({ dir: 'column', width: 'grow', height: 'grow' }, [
    el('edit', { initial: 'hello', width: 'grow', height: BAND }, [], 'doc'), // 0
    band({ onClick: { kind: 'go' } }, 'button'), // 1
    band({}, 'plain'), // 2: no hit region at all
    band({ onDrag: { kind: 'h' } }, 'handle'), // 3
    band({ onDrag: { kind: 's' }, cursor: 'ewResize' }, 'splitter'), // 4: resizes rather than moves
    band({ onClick: { kind: 'nope' }, disabled: true, cursor: 'notAllowed' }, 'refused'), // 5
    band({ onClick: { kind: 'nope' }, disabled: true }, 'quiet'), // 6
    band({ focusable: true }, 'row'), // 7: opens on Enter, no click payload
    band({ hoverable: true }, 'badge'), // 8: hover-only
  ]);
  ctx.frame(400, 400, 1, tree);
  const over = (i) => {
    ctx.cursor(200, i * BAND + BAND / 2);
    return ctx.cursorShape();
  };

  assert.equal(ctx.cursorShape(), 'default', 'before any input');
  assert.equal(over(0), 'text', 'an editor is a caret');
  assert.equal(over(1), 'pointer', 'a button is a hand');
  assert.equal(over(2), 'default', 'a plain box is the arrow');
  assert.equal(over(7), 'pointer', 'a focusable node is a hand');
  assert.equal(over(8), 'default', 'a hover-only node is not');

  // A drag source grabs, and the captured drag holds the shape off the node.
  assert.equal(over(3), 'grab', 'resting');
  ctx.mouse(true);
  assert.equal(ctx.cursorShape(), 'grabbing', 'pressed');
  ctx.cursor(200, 2 * BAND + BAND / 2);
  assert.equal(ctx.cursorShape(), 'grabbing', 'off the node, still captured');
  ctx.mouse(false);
  assert.equal(ctx.cursorShape(), 'default', 'over the plain box now');
  assert.equal(over(3), 'grab', 'back to rest');

  // A declared cursor overrides the derivation, through the drag too.
  assert.equal(over(4), 'ewResize', 'the splitter would derive grab');
  ctx.mouse(true);
  ctx.cursor(200, 2 * BAND + BAND / 2);
  assert.equal(ctx.cursorShape(), 'ewResize', 'the captured drag keeps the declared shape, not grabbing');
  ctx.mouse(false);

  // Disabled strips the click payload, so there is nothing to derive a hand
  // from: a disabled control is the arrow unless it declares otherwise.
  assert.equal(over(5), 'notAllowed', 'declared');
  assert.equal(over(6), 'default', 'quiet');

  ctx.cursorLeft();
  assert.equal(ctx.cursorShape(), 'default', 'no pointer is the arrow');
  ctx.pollEvents();
});

test('cursorShape answers for the topmost node, as a click would', () => {
  const ctx = new Ctx();
  const tree = box({ width: 'grow', height: 'grow', pad: 20, hoverable: true }, [
    box({ width: 100, height: 30, onClick: { kind: 'go' } }, [], 'button'),
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
          ['a ', el('span', { bold: true, color: '#73d98c' }, ['b']), el('span', { italic: true }, [' c'])],
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
    root({ title: 'kui conformance' }, [
      box({ gap: 6 }, [
        // `<titlebar>` appends its own cluster; the second one goes through
        // the `<windowButtons>` element, in a strip laid out by hand.
        el('titlebar', {}, [text('app', { size: 12 })]),
        box({ dir: 'row', width: 'grow' }, [el('windowButtons')]),
        box({ width: 40, height: 16, bg: '#22242c', focusable: true, keyFocus: true, label: 'Sink' }, [], 'sink'),
      ]),
    ]),
  controls: () =>
    root({}, [
      box({ pad: 10, gap: 6, onContextMenu: { kind: 'menu' } }, [
        el('button', { onClick: { kind: 'go' } }, ['go']),
        el('edit', { initial: 'hello', size: 13, width: 160, label: 'Note' }, [], 'note'),
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
  // docs/adr/0010-a-segment-primitive.md: three strokes and a box; the
  // elbow's onClick is the one a line ignores.
  lines: () =>
    root({}, [
      box({ width: 200, height: 120, bg: '#14161e' }, [
        el('line', { from: [10, 10], to: [90, 70], width: 2, color: '#7f9cf5' }),
        el('line', { points: [[100, 20], [140, 20], [140, 60]], width: 3, color: '#d8863b', onClick: 'elbow' }),
        el('line', { points: [[20, 100], [60, 80], [100, 110], [180, 90]], curve: true, width: 1.5, color: '#9ad9a0', opacity: 0.5 }, [], 'curve'),
        box({ width: 40, height: 20, bg: '#202030' }),
      ]),
    ]),
  media: (fx) =>
    root({}, [
      box({ pad: 6, gap: 4 }, [
        el('image', { src: fx().image, width: 16, radius: 2 }),
        el('audio', { src: fx().sound, volume: 0.5, loop: true }, [], 'music'),
        el('latencyGraph'),
      ]),
    ]),
  // docs/adr/0005-the-paint-vocabulary.md: four subtrees the view stops
  // declaring in phase 1 — `fade` still in flight at the end, `blink`
  // already over, `flash` back in phase 2 while its own exit runs, and
  // `bulk` one node past the budget — and two that never leave, so two
  // Tabs at the end say whether the ring has a place for a ghost.
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
        // Last, and sized by children that have no size: dropping it takes
        // only the trailing gap with it.
        phase === 0 && box(
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

// `conformance::build_drag`: one keyed handle whose drag deltas the event
// rows carry, measured from the press point in every phase.
SCENE_TREES.drag = () =>
  root({}, [box({ width: 80, height: 40, bg: '#30344a', onDrag: { kind: 'split' } }, [], 'handle')]);

/** `conformance::EXIT_BULK_ROWS`: with its own root, one node past
 *  `kui_core::depart::MAX_NODES`, so the whole subtree is refused. */
const EXIT_BULK_ROWS = 512;
/** A fixed-size box holding at most one departing node, so dropping that
 *  node moves nothing else on the frame the ghost is compared on. */
const slot = (key, h, child) =>
  box({ width: 140, height: h, bg: '#101018' }, child ? [child] : [], key);
/** A live Tab stop either side of the departing ones. */
const keep = (key, label) =>
  box({ dir: 'row', width: 60, height: 16, bg: '#22242c', focusable: true, label }, [], key);

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

/** FNV-1a over each quad's words 0..18 and 23..30 — `KuiQuad` without its
 *  `uv`, which depends on glyph insertion order — plus the `uv` of a segment
 *  quad (kind 6), where it is the endpoints. Mirrors
 *  `conformance::quad_digest`. */
function quadDigest(buffer) {
  const stride = quadStride();
  const view = new DataView(buffer.buffer, buffer.byteOffset, buffer.byteLength);
  let h = FNV_OFFSET;
  for (let off = 0; off + stride <= buffer.byteLength; off += stride) {
    const segment = view.getUint32(off + KIND_WORD * 4, true) === 6;
    const words = [...Array(19).keys(), ...(segment ? [19, 20, 21, 22] : []), 23, 24, 25, 26, 27, 28, 29, 30];
    for (const i of words) {
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
    (registered ??= { image: addFixtureImage(ctx), sound: addFixtureSound(ctx) });
  let phase = 0;
  const events = [];
  const commands = [];
  // `ctx` is the third argument because one scene has an imperative half:
  // `live` announces through `ctx.announce`, which is where the other
  // three bindings call `ui.announce` / `env.announce` / `kui_announce`.
  const frame = () => {
    ctx.frame(320, 240, 1, build(fx, phase, ctx));
    events.push(...ctx.pollEvents());
    commands.push(...ctx.windowCommands());
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
    else if (step[0] === 'cursor') ctx.cursor(step[1], step[2]);
    else if (step[0] === 'cursorleft') ctx.cursorLeft();
    else if (step[0] === 'mousedown') ctx.mouse(true, 1);
    else if (step[0] === 'mouseup') ctx.mouse(false);
    else if (step[0] === 'secondarydown') ctx.mouse(true, 1, 'secondary');
    else if (step[0] === 'secondaryup') ctx.mouse(false, 1, 'secondary');
    else if (step[0] === 'scroll') ctx.scroll(step[1], step[2]);
    else if (step[0] === 'tab') ctx.key('tab');
    else if (step[0] === 'shifttab') ctx.key('tab', { shift: true });
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
    else throw new Error(`unknown conformance step ${step[0]}`);
    events.push(...ctx.pollEvents());
    commands.push(...ctx.windowCommands());
    frame();
  }
  return { ctx, events, commands };
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

/** Renders a scene block in the report format `conformance::report`
 *  documents: integers, hex and strings only, so the bytes match Rust's. */
function sceneReport(name, env, steps, { ctx, events, commands }) {
  const lines = [`scene ${name}`];
  if (env) {
    const { customChrome, maximized, fullscreen, nativeControls } = ctx.env().window;
    const r = nativeControls ?? { w: 0, h: 0 };
    lines.push(`env ${+customChrome} ${+maximized} ${+fullscreen} ${r.w} ${r.h}`);
  }
  for (const step of steps) lines.push(`step ${step.join(' ')}`);
  lines.push(`title ${ctx.windowTitle() ?? '-'}`);
  const quads = Buffer.from(ctx.quads());
  const stride = quadStride();
  const count = quads.byteLength / stride;
  lines.push(`quads ${count} ${quadDigest(quads)}`);
  const kinds = [0, 0, 0, 0, 0, 0, 0];
  for (let off = 0; off < quads.byteLength; off += stride) kinds[quads.readUInt32LE(off + KIND_WORD * 4)]++;
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
        n.selected === null || n.selected === undefined ? '-' : n.selected ? 1 : 0,
        n.orientation === 'horizontal' ? 'h' : n.orientation === 'vertical' ? 'v' : '-',
        n.live === 'polite' ? 'p' : n.live === 'assertive' ? 'a' : '-',
        n.scroll ? 1 : 0,
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
    lines.push(`event ${p?.kind ?? '-'} ${tag}`);
  }
  for (const c of commands) lines.push(commandLine(c));
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
        nativeControls: w > 0 && h > 0 ? { w, h } : null,
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
    const declared = env ?? { customChrome: false, maximized: false, fullscreen: false, nativeControls: null };
    assert.deepEqual(
      out.ctx.env().window,
      { id: 0, ...declared, nativeControls: declared.nativeControls && { x: 0, y: 0, ...declared.nativeControls } },
      `scene ${name}: env().window reads back differently than it was declared`,
    );
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
  assert.throws(() => ctx.reveal('nope'), /no node is keyed "nope"/);
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
  assert.deepEqual(env.viewport, { width: 320, height: 240, scale: 2 });
  assert.deepEqual(env.window, {
    id: 0,
    customChrome: false,
    maximized: false,
    fullscreen: false,
    nativeControls: null,
  });
});

// `schema::ENV_FIELDS` is the one statement of the env shape; this is
// Node's pin to it. Every documented key path is a leaf however deep its
// value goes (a `Rect`), so the walk stops there; anything else that is an
// object is descended into, so a key added anywhere in `env()` — or one
// dropped — is a difference against the table.
test('env() is the documented env shape, key for key', () => {
  const ctx = new Ctx();
  // Every fact that is sometimes null, present.
  ctx.setEnv({ refreshHz: 60, window: { nativeControls: { w: 78, h: 28 } } });
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
    window: { customChrome: true, maximized: true, fullscreen: true, nativeControls: { x: 8, y: 4, w: 70, h: 20 } },
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
    nativeControls: { x: 8, y: 4, w: 70, h: 20 },
  });
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

test('env() is on both classes and setEnv is only on the headless one', () => {
  // A window's runner reports the real window every frame, so a fact set on
  // one would be overwritten before the next view ran; the read is shared.
  assert.equal(typeof Ctx.prototype.env, 'function');
  assert.equal(typeof KuiWindow.prototype.env, 'function');
  assert.equal(typeof Ctx.prototype.setEnv, 'function');
  assert.equal(KuiWindow.prototype.setEnv, undefined);
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
