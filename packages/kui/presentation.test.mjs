import test from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { createPresentation } from './kit/presentation.js';

const node = children => ({ type: 'box', props: {}, children });
const slide = (key, steps) => ({ key, view: c => node(Array.from({ length: steps }, (_, i) => c.reveal(i + 1, node([])))) });

test('held navigation arrows repeat while toggles remain single press', () => {
  const p = createPresentation({ slides: [slide('a', 3), slide('b', 0)] });
  const key = (code, repeat = false, fields = {}) => p.update({ presentation: 'keys', key: { code, phase: 'down', repeat, ...fields } });
  key('right'); key('right', true); key('right', true);
  assert.equal(p.model.step, 3);
  key('left', true); assert.equal(p.model.step, 2);
  key('o'); key('o', true); assert.equal(p.model.overview, true, 'held O does not toggle repeatedly');
  key('space', true); assert.equal(p.model.step, 2, 'only navigation arrows/pages repeat');
  p.dispose();
});

test('paused live sync separates presenter browsing from audience navigation and effects', () => {
  const active = [];
  const p = createPresentation({ slides: ['a', 'b', 'c'].map(key => ({ ...slide(key, 2),
    activate: () => { active.push(key); return () => active.push(`stop ${key}`); },
  })) });
  p.start();
  p.update({ presentation: 'live-sync', toggle: { on: false } });
  p.update({ presentation: 'jump', slide: 'c', source: 'presenter' });
  p.update({ presentation: 'next', source: 'presenter' });
  assert.deepEqual([p.model.key, p.model.step], ['a', 0], 'audience stays put');
  assert.deepEqual([p.model.preview.key, p.model.preview.step], ['c', 1]);
  assert.deepEqual(active, ['a'], 'private browsing never activates or stops a live demo');
  p.update({ presentation: 'present-slide' });
  assert.deepEqual([p.model.key, p.model.step], ['c', 1]);
  assert.deepEqual(active, ['a', 'stop a', 'c']);
  p.update({ presentation: 'previous', source: 'presenter' });
  p.update({ presentation: 'next', source: 'audience' });
  assert.equal(p.model.step, 2); assert.equal(p.model.preview.step, 0, 'audience input does not move a private selection');
  p.reload({ slides: [slide('c', 2), slide('b', 2), slide('a', 2)] });
  assert.equal(p.model.liveSync, false);
  assert.deepEqual(p.model.preview, { index: 0, key: 'c', step: 0 }, 'reload preserves the private selection by key');
  p.update({ presentation: 'live-sync', toggle: { on: true } });
  assert.equal(p.model.step, 0, 'enabling sync presents the selected position');
  p.update({ presentation: 'next', source: 'presenter' });
  assert.equal(p.model.step, 1); assert.equal(p.model.preview.step, 1);
  p.dispose();
});

test('Lua and TSX share the same navigation contract', () => {
  const trace = JSON.parse(readFileSync(new URL('../../addons/kit/tests/presentation-navigation.json', import.meta.url)));
  const p = createPresentation({ slides: [slide('a', 2), slide('b', 1), slide('c', 0)] });
  for (const { action, key, step, slide: target } of trace) {
    if (action === 'jump') p.jump(target); else p[action]();
    assert.deepEqual([p.model.key, p.model.step], [key, step], action);
  }
  p.dispose();
});

test('removing a private selection keeps the nearest surviving slide independently of the audience', () => {
  const p = createPresentation({ slides: ['a', 'b', 'c', 'd'].map(key => slide(key, 2)) });
  p.update({ presentation: 'live-sync', toggle: { on: false } }); p.jump('d', 'presenter'); p.next('presenter');
  p.reload({ slides: ['a', 'b', 'c'].map(key => slide(key, 2)) });
  assert.equal(p.model.key, 'a'); assert.deepEqual(p.model.preview, { index: 2, key: 'c', step: 0 });
  p.dispose();
});

test('demo event fields arrive beside the original user message', () => {
  const p = createPresentation({ slides: [{ key: 'demo', init: () => ({ value: 1 }),
    update: (s, m) => ({ value: m.kind === 'edit' ? m.change.value : s.value }), view: () => node([]) }] });
  p.start();
  const c = p.context(0, 0, 'live');
  p.update({ ...c.msg({ kind: 'edit' }), change: { value: 9 } });
  assert.equal(p.model.states.demo.value, 9);
  p.dispose();
});

test('activation work pauses offscreen and snapshot state cannot mutate live state', () => {
  const scheduled = [];
  const p = createPresentation({ slides: [{ key: 'live', init: () => ({ n: 0 }),
    activate: c => { c.every(100, { kind: 'tick' }); },
    update: s => ({ n: s.n + 1 }),
    view: c => { if (c.mode === 'snapshot') c.state.n = 99; return c.reveal(1, node([])); } }, slide('other', 0)] }, {
    every: callback => { const job = { callback, canceled: false }; scheduled.push(job); return () => { job.canceled = true; }; },
  });
  assert.equal(p.model.states.live.n, 0);
  assert.equal(scheduled.length, 0, 'discovery/preview must not activate');
  p.start(); scheduled[0].callback(); assert.equal(p.model.states.live.n, 1);
  p.renderSlide(0, 1, 'snapshot'); assert.equal(p.model.states.live.n, 1);
  p.jump('other'); assert.equal(scheduled[0].canceled, true);
  scheduled[0].callback(); assert.equal(p.model.states.live.n, 1, 'late canceled results are ignored');
  p.jump('live'); assert.equal(scheduled.length, 2);
  p.update({ presentation: 'overview' }); assert.equal(scheduled[1].canceled, false, 'private overview leaves the audience demo active');
  p.dispose();
  assert.equal(scheduled[1].canceled, true);
});

test('reload preserves keyed compatible state and rejects obsolete activation results', () => {
  let oldContext;
  const live = { key: 'live', init: () => ({ n: 0 }), view: c => c.reveal(2, node([])),
    activate: c => { oldContext ??= c; }, update: s => ({ n: s.n + 1 }) };
  const p = createPresentation({ slides: [live, slide('other', 0)] });
  p.start(); oldContext.dispatch({}); p.model.step = 2;
  p.reload({ slides: [slide('other', 0), { ...live, view: c => c.reveal(1, node([])) }] });
  assert.deepEqual([p.model.index, p.model.step, p.model.states.live.n], [1, 1, 1]);
  p.update(oldContext.msg({})); oldContext.dispatch({}); assert.equal(p.model.states.live.n, 1);
  p.reload({ slides: [{ ...live, stateVersion: 2 }] }); assert.equal(p.model.states.live.n, 0);
  p.dispose();
});

test('export plans final steps or every step without live activation', () => {
  const p = createPresentation({ slides: [slide('a', 2), slide('b', 0)] }, { snapshot: true });
  assert.deepEqual(p.exportPlan().map(f => [f.key, f.step]), [['a', 2], ['b', 0]]);
  assert.deepEqual(p.exportPlan(true).map(f => [f.key, f.step]), [['a', 0], ['a', 1], ['a', 2], ['b', 0]]);
  assert.throws(() => createPresentation({ slides: [slide('x', 0), slide('x', 1)] }), /unique/);
  assert.throws(() => createPresentation({ slides: [{ ...slide('x', 1), snapshot: () => node([]) }] }), /steps count/);
  p.dispose();
});

test('the visible presenter can reopen a hidden audience without an audience redraw', () => {
  const commands = [];
  const p = createPresentation({ slides: [slide('a', 0)] });
  const surface = {
    env: () => ({ window: { id: 7 }, viewport: { width: 1180, height: 860 } }),
    size: () => ({ width: 1180, height: 860 }), focused: () => null, displayNames: () => ['Projector'],
    setWindowCloseHides: (...args) => commands.push(['policy', ...args]),
    setWindowVisible: (...args) => commands.push(['visible', ...args]),
    setWindowFullscreen: (...args) => commands.push(['fullscreen', ...args]),
    focusWindow: (...args) => commands.push(['focus', ...args]),
  };
  p.model.display = '0'; p.update({ presentation: 'audience' });
  assert.equal(p.view(surface, 'presenter').props.title, 'Presentation — Presenter');
  assert.deepEqual(commands, [['policy', 7, true], ['visible', 0, true], ['fullscreen', 0, true, 0], ['focus', 0]]);
  commands.length = 0;
  p.update({ presentation: 'keys', key: { phase: 'down', code: 'p', ctrl: true, shift: true } });
  p.view(surface, 'presenter');
  assert.deepEqual(commands, [['visible', 7, true], ['focus', 7]]);
  assert.equal(p.view(surface, 'main').props.title, 'Presentation — Audience');
  p.dispose();
});

test('themes remain authored settings while a style preset can choose both', () => {
  const p = createPresentation({ theme: 'paper', slides: [slide('a', 0)] });
  assert.equal(p.model.layout, 'minimal');
  p.update({ presentation: 'theme', change: { value: 'stage' } });
  assert.equal(p.model.theme, 'paper', 'theme is not a live presenter action');
  assert.equal(p.model.layout, 'minimal');
  p.dispose();
  const preset = createPresentation({ preset: 'paper', theme: 'signal', slides: [slide('a', 0)] });
  assert.deepEqual([preset.model.theme, preset.model.layout], ['signal', 'editorial']);
  preset.dispose();
});


test('presenter reveal guides show numbered future content without changing audience snapshots', () => {
  const p = createPresentation({ slides: [slide('a', 2)] }, { snapshot: true });
  const guide = JSON.stringify(p.renderSlide(0, 0, 'snapshot', undefined, true));
  assert.ok(guide.includes('"opacity":0.28'));
  assert.ok(guide.includes('"borderW":1') && guide.includes('"borderColor":"$kitAccent"'), 'pending reveals have a box');
  assert.ok(guide.includes('"children":["1"]') && guide.includes('"children":["2"]'));
  const audience = JSON.stringify(p.renderSlide(0, 0, 'snapshot'));
  assert.ok(audience.includes('"visible":false'));
  assert.ok(!audience.includes('"opacity":0.28'));
  assert.ok(!audience.includes('"borderW":1'));
  p.update({ presentation: 'overview' });
  assert.equal(p.model.presenter, true);
  assert.equal(p.model.key, 'a'); assert.equal(p.model.step, 0);
  assert.equal(JSON.stringify(p.renderSlide(0, 0, 'snapshot')), audience);
  p.reload({ theme: 'stage', layout: 'split', slides: [slide('a', 2)] });
  assert.deepEqual([p.model.theme, p.model.layout], ['stage', 'split'], 'watch reload uses authored settings');
  p.dispose();
});
