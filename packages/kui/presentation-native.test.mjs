import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import { join } from 'node:path';
import { Ctx, createApp } from './index.js';
import * as kit from './kit/index.js';
import { createPresentation, exportPresentation, present } from './kit/presentation.js';

test('the watched launcher retains overrides and normalizes live window flags', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'kit-watch-'));
  const entry = join(directory, 'deck.mjs');
  const source = title => `export default {title:${JSON.stringify(title)},theme:'minimal',layout:'minimal',slides:[{key:'a',view:()=>({type:'box',props:{},children:[]})}]};`;
  const ctx = new Ctx(); ctx.addExtension('kit', kit.extension.path);
  let controller, app, alive = true;
  const deadline = Date.now() + 5000;
  const surface = new Proxy(ctx, { get(target, name) {
    if (name === 'setView') return tree => ctx.frame(800, 600, 1, tree);
    if (name === 'windows') return () => ['main'];
    if (name === 'nextDeadlineMs') return () => null;
    if (name === 'pump') return () => {
      if (Date.now() > deadline) throw new Error('watch reload timed out');
      return alive && controller?.deck.title !== 'After';
    };
    const value = Reflect.get(target, name); return typeof value === 'function' ? value.bind(target) : value;
  } });
  try {
    await writeFile(entry, source('Before'));
    await present(entry, { surface, args: ['--watch', '--theme', 'paper', '--layout', 'split', '--canvas', '1280x720', '--size', '800x600', '--appearance', 'dark'],
      setup: (loop, p) => { app = loop; controller = p; void writeFile(entry, source('After')); } });
    assert.equal(controller.deck.title, 'After');
    assert.equal(controller.deck.theme, 'paper'); assert.equal(controller.deck.layout, 'split');
    assert.deepEqual(controller.deck.canvas, { w: 1280, h: 720 });
    assert.deepEqual(controller.options.size, { width: 800, height: 600 });
    assert.equal(controller.options.appearance, 'dark');
  } finally { alive = false; app?.teardown(); await rm(directory, { recursive: true, force: true }); }
});

const demo = {
  key: 'demo', init: () => ({ value: 2 }),
  update: (state, message) => message.change ? { value: message.change.value } : state,
  view: c => c.content({ title: 'A native deck' }, [
    kit.Slider({ label: 'Concurrency', min: 0, max: 10, step: 1, value: c.state.value, onChange: c.msg({ kind: 'edit' }) }),
    c.reveal(1, kit.Paragraph({ text: 'Revealed' })),
  ]),
};

for (const openingKey of ['space', 'down']) test(`a keyboard-focused presentation Select opens with ${openingKey} and Tab leaves it`, () => {
  const p = createPresentation({ slides: [{ key: 'select', view: () => ({ type: 'box', props: { gap: 12 }, children: [
    kit.Button({ label: 'Before' }),
    kit.Select({ label: 'Mode', value: 'a', options: [{ value: 'a', label: 'Alpha' }, { value: 'b', label: 'Beta' }] }),
    kit.Button({ label: 'After' }),
  ] }) }] });
  const app = createApp({ init: p.model, update: (_, message) => p.update(message),
    view: (_, window, surface) => p.view(surface, window) },
  { width: 900, height: 700, extensions: [kit.extension], warnings: false });
  p.options.dispatch = message => app.dispatch(message); p.start();
  try {
    app.render(); app.settle();
    const field = app.ctx.accessTree().nodes.find(n => n.name === 'Mode: Alpha');
    for (let i = 0; i < 6 && app.ctx.focused() !== field.key; i++) app.press('tab');
    assert.equal(app.ctx.focused(), field.key, 'Tab enters Select');
    app.press('tab');
    const after = app.ctx.accessTree().nodes.find(n => n.name === 'After');
    assert.equal(app.ctx.focused(), after.key, 'Tab leaves Select');
    app.press('tab', { shift: true });
    assert.equal(app.ctx.focused(), field.key);
    // macOS opens on a bare arrow; Windows/Linux use Alt+arrow.
    app.press(openingKey, openingKey === 'down' && app.ctx.env().platform !== 'macos' ? { alt: true } : undefined);
    assert.ok(app.ctx.accessTree().nodes.some(n => n.name === 'Beta'), 'the options open from the keyboard');
    app.press('escape'); app.press('tab', { shift: true });
    assert.equal(app.ctx.focused(), app.ctx.accessTree().nodes.find(n => n.name === 'Before').key, 'Shift-Tab leaves Select');
    assert.deepEqual(app.warnings, []);
  } finally { app.teardown(); p.dispose(); }
});

test('both window views receive navigation and demo updates with the timer paused', () => {
  const p = createPresentation({ slides: [demo] }, {
    every: () => { throw new Error('synchronization must not schedule a timer'); },
  });
  const events = [];
  const frames = new Map();
  const surface = {
    setDiagnostics() {}, warnings: () => [], stats: () => ({}), animating: () => false,
    windows: () => ['main', 'presenter'], useWindow() {},
    pollEvents: () => events.splice(0),
    setView: (tree, name) => frames.set(name, tree),
  };
  const app = createApp({ init: p.model, update: (_, message) => p.update(message),
    view: (_, name) => ({ step: p.model.step, value: p.model.states.demo.value, name }) },
  { surface, clock: () => 0 });
  p.options.dispatch = message => app.dispatch(message);
  p.start(); app.render();
  const expect = (step, value) => {
    for (const name of ['main', 'presenter']) assert.deepEqual(frames.get(name), { step, value, name });
    assert.equal(p.model.running, false);
  };
  try {
    expect(0, 2);
    for (const [window, message, step, value] of [
      [0, { presentation: 'next' }, 1, 2],
      [2, { presentation: 'previous' }, 0, 2],
      [0, { ...p.context(0, 0, 'live').msg({ kind: 'edit' }), change: { value: 8 } }, 0, 8],
    ]) {
      events.push({ origin: 0, window, key: 'control', event: 'click', payload: message });
      app.step(); // One event-loop turn; time never advances.
      expect(step, value);
    }
    p.context(0, 0, 'live').dispatch({ change: { value: 6 } });
    app.step();
    expect(0, 6);
  } finally { p.dispose(); }
});

test('presenter live-sync controls browse privately and explicitly present the selection', () => {
  const p = createPresentation({ slides: ['First', 'Second'].map(title => ({ key: title, notes: `Notes for ${title}`,
    view: () => kit.Heading({ text: title }) })) });
  let window = 'presenter';
  const app = createApp({ init: p.model, update: (_, message) => p.update(message),
    view: (_, _window, surface) => p.view(surface, window) },
  { width: 1180, height: 860, extensions: [kit.extension], warnings: false });
  p.options.dispatch = message => app.dispatch(message); p.start();
  const click = name => {
    const node = app.ctx.accessTree().nodes.find(n => n.name === name);
    assert.ok(node, name); app.click(node.rect.x + node.rect.w / 2, node.rect.y + node.rect.h / 2);
  };
  try {
    app.render(); app.settle();
    const control = name => app.ctx.accessTree().nodes.find(n => n.name === name);
    assert.equal(control('Previous').disabled, true, 'Previous is disabled at the first step');
    assert.equal(control('Next').disabled, false);
    assert.ok(control('Live sync').rect.y >= 830, 'Live sync belongs in the footer');
    assert.ok(!control('Timer paused') && !control('Timer running'));
    const toggleKey = control('Live sync').key;
    for (let i = 0; i < 20 && app.ctx.focused() !== toggleKey; i++) app.press('tab');
    assert.equal(app.ctx.focused(), toggleKey, 'Tab reaches Live sync');
    for (const enabled of [false, true, false]) {
      app.press('space');
      assert.equal(p.model.liveSync, enabled);
      assert.equal(app.ctx.focused(), toggleKey, 'toggling keeps keyboard focus on the same switch');
    }
    app.press('tab'); assert.notEqual(app.ctx.focused(), toggleKey, 'Tab still leaves the switch');
    app.press('tab', { shift: true }); assert.equal(app.ctx.focused(), toggleKey, 'Shift-Tab returns to the switch');
    assert.equal(p.model.liveSync, false); click('Next');
    assert.equal(control('Next').disabled, true, 'Next follows the private selection at the end');
    assert.equal(control('Previous').disabled, false);
    assert.equal(p.model.key, 'First'); assert.equal(p.model.preview.key, 'Second');
    assert.ok(app.ctx.accessTree().nodes.some(n => n.name === 'Private preview'));
    window = 'main'; app.render(); app.settle();
    assert.ok(app.ctx.accessTree().nodes.some(n => n.name === 'First'));
    assert.ok(!app.ctx.accessTree().nodes.some(n => n.name === 'Second'));
    window = 'presenter'; app.render(); app.settle(); click('Present slide');
    assert.equal(p.model.key, 'Second'); assert.equal(p.model.liveSync, false);
    click('Previous');
    assert.equal(control('Previous').disabled, true);
    app.ctx.blur(); app.render(); app.settle();
    app.press('enter', { ctrl: true });
    assert.equal(p.model.key, 'First', 'the presenter commit accelerator publishes its private selection');
    click('Live sync'); assert.equal(p.model.liveSync, true); click('Next');
    assert.equal(p.model.key, 'Second');
    assert.deepEqual(app.warnings, []);
  } finally { app.teardown(); p.dispose(); }
});

test('native focused controls keep arrows while unclaimed keys navigate the deck', () => {
  const p = createPresentation({ slides: [demo] });
  const app = createApp({ init: p.model, update: (_, message) => p.update(message),
    view: (_, window, surface) => p.view(surface, window) },
  { width: 900, height: 700, extensions: [kit.extension], warnings: false });
  p.options.dispatch = message => app.dispatch(message); p.start(); app.render(); app.settle();
  try {
    app.press('right'); assert.equal(p.model.step, 1, 'the root receives unclaimed navigation');
    app.press('tab');
    const slider = app.ctx.accessTree().nodes.find(n => n.role === 'slider' && n.name === 'Concurrency');
    assert.ok(slider, 'the real Kit slider is accessible');
    for (let i = 0; i < 4 && app.ctx.focused() !== slider.key; i++) app.press('tab');
    assert.equal(app.ctx.focused(), slider.key, 'Tab enters the live control');
    app.press('right');
    assert.equal(p.model.states.demo.value, 3, 'the slider consumes its own arrow');
    app.press('right', {}, true);
    assert.equal(p.model.states.demo.value, 4, 'repeated arrows remain with the focused slider');
    assert.equal(p.model.step, 1, 'focused control input did not navigate');
    app.press('p', { ctrl: true, shift: true });
    assert.equal(p.model.presenter, true, 'an unclaimed recovery chord reaches the root');
    assert.deepEqual(app.warnings, []);
  } finally { app.teardown(); p.dispose(); }
});

test('native repeated arrow presses navigate slide reveals', () => {
  const p = createPresentation({ slides: [{ key: 'repeat', view: c => c.content({},
    [1, 2, 3].map(step => c.reveal(step, kit.Paragraph({ text: `Step ${step}` })))) }] });
  const app = createApp({ init: p.model, update: (_, message) => p.update(message),
    view: (_, window, surface) => p.view(surface, window) },
  { width: 900, height: 700, extensions: [kit.extension], warnings: false });
  p.options.dispatch = message => app.dispatch(message); p.start();
  try {
    app.render(); app.settle();
    app.press('right'); app.press('right', {}, true); app.press('right', {}, true);
    assert.equal(p.model.step, 3);
    app.press('left', {}, true); assert.equal(p.model.step, 2);
    assert.deepEqual(app.warnings, []);
  } finally { app.teardown(); p.dispose(); }
});

test('native exports are deterministic snapshots with project fonts and size overrides', async () => {
  const directory = await mkdtemp(join(tmpdir(), 'kit-presentation-'));
  let activations = 0;
  const deck = { theme: 'paper', settings: { fontTitle: ['mono'], roles: { accent: '#8c3e70' }, sizes: { heading: 58 } },
    canvas: { w: 640, h: 360 }, slides: [{ ...demo, activate: () => { activations++; } }] };
  try {
    const options = { directory, size: { width: 960, height: 540 }, timestamp: 2000, allSteps: true };
    const manifest = await exportPresentation(deck, options);
    assert.deepEqual(manifest.frames.map(f => [f.key, f.step, f.timestamp_ms]), [['demo', 0, 2000], ['demo', 1, 2000]]);
    assert.equal(manifest.width, 960);
    const first = await readFile(join(directory, '0002.png'));
    await exportPresentation(deck, options);
    assert.deepEqual(await readFile(join(directory, '0002.png')), first);
    assert.equal(first.readUInt32BE(16), 960); assert.equal(first.readUInt32BE(20), 540);
    assert.equal(activations, 0);
  } finally { await rm(directory, { recursive: true, force: true }); }
});


test('window requests expose fullscreen, visibility and close policy across the native binding', () => {
  const ctx = new Ctx();
  assert.deepEqual(ctx.displayNames(), [], 'headless has no physical display enumeration');
  ctx.setWindowFullscreen(0, true, 1); ctx.setWindowVisible(0, false); ctx.setWindowCloseHides(0, true);
  assert.deepEqual(ctx.windowCommands(), [
    { kind: 'setFullscreen', window: 0, enabled: true, display: 1 },
    { kind: 'setVisible', window: 0, visible: false },
    { kind: 'setCloseHides', window: 0, enabled: true },
  ]);
});

test('presenter chrome keeps overview and numbered reveal guides off the audience view', () => {
  const p = createPresentation({ slides: [demo] }, { snapshot: true });
  let window = 'presenter';
  const app = createApp({ init: p.model, update: (_, message) => p.update(message),
    view: (_, _window, surface) => p.view(surface, window) },
  { width: 1180, height: 860, extensions: [kit.extension], warnings: false });
  app.ctx.setEnv({ window: { customChrome: true }, system: { appearance: 'light', motion: 'reduced' } });
  try {
    app.render(); app.settle();
    const nodes = app.ctx.accessTree().nodes;
    for (const name of ['Previous', 'Next', 'Start timer', 'Reset timer']) {
      const control = nodes.find(n => n.name === name);
      assert.ok(control && control.rect.y >= 34 && control.rect.y < 860 && control.rect.w > 0, `${name} is visible below the titlebar`);
    }
    assert.ok(nodes.some(n => n.name === 'Slide 1 of 1' && n.rect.y >= 830), 'position belongs in the footer');
    assert.ok(!nodes.some(n => ['Theme', 'Layout'].includes(n.name)));
    assert.ok(!nodes.some(n => n.role === 'slider'), 'preview content cannot take live input');
    app.press('o');
    assert.equal(p.model.overview, true);
    assert.ok(app.ctx.accessTree().nodes.some(n => n.name === 'Overview'));
    window = 'main'; app.render(); app.settle();
    const audience = app.ctx.accessTree().nodes;
    assert.ok(!audience.some(n => n.name === 'Overview' || n.name === 'Speaker notes' || n.name === 'Revealed'));
    assert.ok(audience.some(n => n.name === 'A native deck'), 'audience stays on the original slide');
    for (const name of ['Presenter', 'Restart talk', 'Restart…', 'Slide 1 of 1', 'Step 0 of 1']) assert.ok(!audience.some(n => n.name === name));
    assert.deepEqual(app.warnings, []);
  } finally { app.teardown(); p.dispose(); }
});

test('Toolbar chrome is opt-in, clears native controls and leaves gaps draggable', () => {
  for (const windowChrome of [false, true]) {
    const ctx = new Ctx(); ctx.addExtension('kit', kit.extension.path);
    ctx.setEnv({ window: { customChrome: true, nativeControls: { x: 0, y: 0, w: 86, h: 52 } } });
    const tree = { type: 'box', props: { width: 'grow', height: 'grow' }, children: [kit.Settings({ skin: 'neutral' }),
      kit.Toolbar({ windowChrome, trailing: [kit.Button({ label: 'Right', onPress: { kind: 'right' } })] },
        kit.Button({ label: 'Left', onPress: { kind: 'left' } }))] };
    ctx.frame(800, 300, 1, tree); ctx.frame(800, 300, 1, tree); ctx.pollEvents(); ctx.windowCommands();
    const nodes = ctx.accessTree().nodes;
    const left = nodes.find(n => n.name === 'Left'), right = nodes.find(n => n.name === 'Right');
    assert.ok(left && right);
    assert.ok(windowChrome ? left.rect.x >= 86 : left.rect.x < 86, 'only opt-in reserves the native control inset');
    const y = left.rect.y + left.rect.h / 2;
    const gap = (left.rect.x + left.rect.w + right.rect.x) / 2;
    ctx.cursor(gap, y); ctx.mouse(true); ctx.mouse(false);
    assert.equal(ctx.windowCommands().some(c => c.kind === 'startDrag'), windowChrome, 'empty space is draggable only with chrome enabled');
    ctx.cursor(left.rect.x + left.rect.w / 2, y); ctx.mouse(true); ctx.mouse(false);
    assert.ok(!ctx.windowCommands().some(c => c.kind === 'startDrag'), 'controls keep their own click');
    assert.ok(ctx.pollEvents().some(e => e.payload?.kind === 'left'));
    assert.deepEqual(ctx.warnings(), []);
  }
});

test('audience controls stay private and macOS menu accelerators recover the presenter', () => {
  const p = createPresentation({ slides: [demo] }); let window = 'main';
  const app = createApp({ init: p.model, update: (_, message) => p.update(message),
    view: (_, _window, surface) => p.view(surface, window) },
  { width: 1180, height: 860, extensions: [kit.extension], warnings: false });
  app.ctx.setNativeMenuBar(true);
  app.ctx.setEnv({ system: { motion: 'reduced' } });
  p.options.dispatch = message => app.dispatch(message); p.start();
  try {
    app.render(); app.settle();
    const audience = app.ctx.accessTree().nodes;
    for (const name of ['Presenter', 'Restart talk', 'Restart…', 'Slide 1 of 1', 'Step 0 of 1']) assert.ok(!audience.some(n => n.name === name));
    if (app.ctx.env().platform === 'macos') {
      const menu = app.ctx.menuBar();
      assert.ok(menu?.standardMenus);
      assert.equal(menu.menus[0].items[0].label, 'Show Presenter');
      assert.ok(menu.menus[0].items[0].accel, 'the native menu exposes the recovery accelerator');
      assert.ok(app.ctx.activateMenuBarItem(0, 0)); app.settle();
      assert.equal(p.model.presenter, true);
    }
    app.press('?');
    assert.equal(p.model.help, true);
    assert.ok(!app.ctx.accessTree().nodes.some(n => n.name === 'Keyboard shortcuts'), 'help never covers the audience');
    window = 'presenter'; app.render(); app.settle();
    assert.ok(app.ctx.accessTree().nodes.some(n => n.name === 'Keyboard shortcuts'));
    app.press('escape'); assert.equal(p.model.help, false);
    p.model.states.demo.value = 9;
    p.update({ presentation: 'restart-request' }); app.render(); app.settle();
    assert.equal(p.model.states.demo.value, 9, 'restart waits for confirmation');
    assert.ok(app.ctx.accessTree().nodes.some(n => n.name === 'Restart this talk?'));
    p.update({ presentation: 'restart-cancel' }); assert.equal(p.model.states.demo.value, 9);
    assert.deepEqual(app.warnings, []);
  } finally { app.teardown(); p.dispose(); }
});
