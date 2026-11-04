// Host-side deck callbacks stay in JavaScript; Kit paints the resulting
// ordinary component trees. Importing this controller does not load native.
import kit from './components.js';

const layouts = { paper: 'editorial', signal: 'briefing', stage: 'split', minimal: 'minimal' };
const themes = Object.keys(layouts);
function presenterShortcut(deck, key) {
  const chord = deck.presenterShortcut;
  if (!chord) return (key.super || key.ctrl) && key.shift && key.code.toLowerCase() === 'p';
  return key.code.toLowerCase() === chord.key.toLowerCase() && !!key.shift === !!chord.shift && !!key.alt === !!chord.alt
    && (chord.primary ? key.super || key.ctrl : !!key.ctrl === !!chord.ctrl && !!key.super === !!chord.super);
}
function presenterAccel(deck) {
  const chord = deck.presenterShortcut;
  if (!chord) return 'mod+shift+p';
  return [...['primary', 'ctrl', 'super', 'alt', 'shift'].filter(name => chord[name]).map(name => name === 'primary' ? 'mod' : name), chord.key.toLowerCase()].join('+');
}
function presenterKeyLabel(deck, platform) {
  const chord = deck.presenterShortcut ?? { primary: true, shift: true, key: 'p' }, mac = platform === 'macos', parts = [];
  if (chord.ctrl || (chord.primary && !mac)) parts.push(mac ? '⌃' : 'Ctrl');
  if (chord.alt) parts.push(mac ? '⌥' : 'Alt');
  if (chord.shift) parts.push(mac ? '⇧' : 'Shift');
  if (chord.super || (chord.primary && mac)) parts.push(mac ? '⌘' : platform === 'windows' ? 'Win' : 'Super');
  return [...parts, chord.key.toUpperCase()].join(mac ? '' : '+');
}
const msg = (presentation, fields = {}) => ({ presentation, ...fields });
const plain = x => x != null && (Array.isArray(x) || Object.getPrototypeOf(x) === Object.prototype || Object.getPrototypeOf(x) === null);
function copy(x, seen = new Map()) {
  if (!plain(x)) return x;
  if (seen.has(x)) return seen.get(x);
  const out = Array.isArray(x) ? [] : Object.create(null);
  seen.set(x, out);
  for (const [k, v] of Object.entries(x)) out[k] = copy(v, seen);
  return out;
}
function still(x) {
  if (!plain(x)) return x;
  if (Array.isArray(x)) return x.map(still);
  const out = {};
  for (const [k, v] of Object.entries(x)) {
    if (!['enter', 'exit', 'keyframes', 'transition', 'animate'].includes(k)) out[k] = still(v);
  }
  if ('animate' in x) out.animate = false;
  if (x.type === 'component' && x.props?.name?.endsWith('/shader')) out.props.props.animate = false;
  if (x.type === 'component' && x.props?.name?.endsWith('/line_chart')) { out.props.props.readout = false; out.props.props.focusable = false; }
  return out;
}
const sizeOf = s => ({ width: s?.width ?? s?.w ?? 1280, height: s?.height ?? s?.h ?? 720 });
const box = ({ key, ...props }, ...children) => ({ type: 'box', key, props, children: children.flat(Infinity).filter(x => x != null && x !== false) });
const row = (props, ...children) => box({ dir: 'row', ...props }, ...children);
const text = (value, size = '$kitSize', color = '$kitFg') => ({ type: 'text', props: { size, color, family: '$kitUi', wrap: 'word' }, children: [value] });

/** A controller that can be embedded in an ordinary kui application. */
export class Presentation {
  constructor(deck, options = {}, previous) {
    if (!deck?.slides?.length) throw new Error('a presentation needs slides');
    for (const theme of [deck.theme, deck.preset, deck.settings?.theme]) {
      if (theme !== undefined && !themes.includes(theme)) throw new Error('theme must be paper, signal, stage or minimal');
    }
    if (deck.layout !== undefined && !Object.values(layouts).includes(deck.layout)) throw new Error('layout must be editorial, briefing, split or minimal');
    if (deck.canvas && !Object.values(sizeOf(deck.canvas)).every(n => Number.isFinite(n) && n > 0)) throw new Error('canvas dimensions must be finite and positive');
    const seen = new Set();
    for (const slide of deck.slides) {
      if (typeof slide.key !== 'string' || !slide.key || seen.has(slide.key)) throw new Error('slide keys must be nonempty and unique');
      if (typeof slide.view !== 'function') throw new Error(`slide ${slide.key} needs view(context)`);
      if (slide.steps !== undefined && (!Number.isInteger(slide.steps) || slide.steps < 0)) throw new Error(`slide ${slide.key}: steps must be a nonnegative integer`);
      if (slide.snapshot && (!Number.isInteger(slide.steps) || slide.steps < 0)) throw new Error(`slide ${slide.key}: a replacement snapshot needs an explicit nonnegative steps count`);
      seen.add(slide.key);
    }
    this.deck = deck; this.options = options; this.counts = Object.create(null); this.loadedFonts = new Set();
    this.generation = (previous?.generation ?? 0) + 1; this.disposers = []; this.requests = []; this.size = sizeOf(options.size); this.initializedWindows = new Set();
    const old = previous?.model;
    const states = Object.create(null), versions = Object.create(null);
    for (const slide of deck.slides) {
      const version = slide.stateVersion ?? 0;
      states[slide.key] = old && old.versions[slide.key] === version ? old.states[slide.key] : slide.init?.() ?? {};
      versions[slide.key] = version;
    }
    let index = old ? deck.slides.findIndex(s => s.key === old.key) : 0;
    if (index < 0) index = Math.min(old.index, deck.slides.length - 1);
    this.model = { index, key: deck.slides[index].key, step: old?.step ?? 0, states, versions,
      theme: deck.theme ?? deck.preset ?? deck.settings?.theme ?? 'minimal', layout: deck.layout ?? layouts[deck.preset] ?? 'minimal',
      presenter: old?.presenter ?? options.presenter ?? false, overview: false, help: false, confirmRestart: false, display: old?.display ?? 'current', elapsed: previous?.elapsed() ?? 0, running: false };
    for (let i = 0; i < deck.slides.length; i++) this.renderSlide(i, 0, 'snapshot', this.size);
    this.model.step = Math.min(this.model.step, this.counts[this.model.key] ?? 0);
    this.model.liveSync = old?.liveSync ?? true;
    const previewIndex = deck.slides.findIndex(s => s.key === old?.preview?.key);
    const privateIndex = previewIndex >= 0 ? previewIndex : Math.min(old?.preview?.index ?? index, deck.slides.length - 1);
    const privateKey = deck.slides[privateIndex].key;
    this.model.preview = !this.model.liveSync && old?.preview
      ? { index: privateIndex, key: privateKey, step: previewIndex >= 0 ? Math.min(old.preview.step, this.counts[privateKey] ?? 0) : 0 }
      : this.position();
    previous?.dispose();
    if (old?.running && !options.snapshot) this.update(msg('timer'));
  }
  now() { return (this.options.clock ?? Date.now)(); }
  send(message) { return this.options.dispatch ? this.options.dispatch(message) : this.update(message); }
  context(index, step, mode, size, revealGuide = false) {
    const slide = this.deck.slides[index], generation = this.generation;
    let revealCount = 0;
    const c = { state: mode === 'snapshot' ? copy(this.model.states[slide.key]) : this.model.states[slide.key],
      step, mode, layout: this.model.layout, theme: this.model.theme, size: sizeOf(size), now: mode === 'snapshot' ? this.options.snapshotTime ?? 0 : this.now() };
    c.msg = message => ({ ...(message && typeof message === 'object' ? message : {}), ...msg('demo', { slide: slide.key, generation, mode, message }) });
    c.dispatch = message => { if (mode === 'live' && generation === this.generation) this.send(c.msg(message)); };
    c.reveal = (at, children, options = {}) => {
      if (!Number.isInteger(at) || at < 1) throw new Error('reveal steps must be positive integers');
      this.counts[slide.key] = Math.max(this.counts[slide.key] ?? 0, at);
      if (revealGuide) return box({ key: `reveal.${options.key ?? ++revealCount}`, width: 'grow',
        borderW: step < at ? 1 : undefined, borderColor: step < at ? '$kitAccent' : undefined, radius: '$kitRadius', bg: step < at ? '$kitHover' : undefined },
        box({ width: 'grow', opacity: step >= at ? 1 : 0.28 }, children),
        box({ width: 24, height: 24, radius: 12, bg: '$kitAccent', mainAlign: 'center', crossAlign: 'center',
          float: { anchor: 'parent', dx: -32 } }, { type: 'text', props: { size: 14, bold: true, family: '$kitUi', color: '$kitOnAccent' }, children: [String(at)] }));
      return kit.Visibility({ key: `reveal.${options.key ?? ++revealCount}`, visible: step >= at, collapse: options.collapse }, children);
    };
    c.Reveal = ({ step: at, children, ...options }) => c.reveal(at, children, options);
    c.content = (props = {}, children) => kit.Content({ layout: this.model.layout, ...props }, children);
    c.Content = ({ children, ...props }) => c.content(props, children);
    const timer = (ms, message, once) => {
      if (mode !== 'live' || this.active !== slide.key || generation !== this.generation) throw new Error('managed timers belong in activate(context)');
      if (!Number.isFinite(ms) || ms <= 0) throw new Error('timer interval must be positive');
      const callback = () => { if (generation === this.generation) c.dispatch(typeof message === 'function' ? message() : message); };
      const schedule = once ? this.options.after ?? ((fn, t) => { const id = setTimeout(fn, t); return () => clearTimeout(id); })
        : this.options.every ?? ((fn, t) => { const id = setInterval(fn, t); return () => clearInterval(id); });
      const cancel = schedule(callback, ms); this.disposers.push(cancel); return cancel;
    };
    c.every = (ms, message) => timer(ms, message, false);
    c.after = (ms, message) => timer(ms, message, true);
    c.spawn = async task => {
      if (mode !== 'live' || this.active !== slide.key) throw new Error('managed tasks belong in activate(context)');
      const abort = new AbortController(); this.disposers.push(() => abort.abort());
      try { const message = await task(abort.signal); if (message != null) c.dispatch(message); }
      catch (error) { if (!abort.signal.aborted) (this.options.onError ?? console.error)(error); }
    };
    return c;
  }
  stopActive() {
    this.generation++; this.active = undefined;
    const cleanup = this.disposers; this.disposers = [];
    for (const cancel of cleanup.reverse()) cancel();
  }
  start() { this.pace(); }
  pace() {
    const wanted = !this.options.snapshot ? this.model.key : undefined;
    if (this.active === wanted) return;
    this.stopActive();
    if (wanted) {
      this.active = wanted;
      const cleanup = this.deck.slides[this.model.index].activate?.(this.context(this.model.index, this.model.step, 'live', this.size));
      if (cleanup) this.disposers.push(cleanup);
    }
  }
  dispose() { this.stopActive(); this.timer?.(); this.timer = undefined; }
  position(source) {
    const m = source === 'presenter' && !this.model.liveSync ? this.model.preview : this.model;
    return { index: m.index, key: m.key, step: m.step };
  }
  navigate(position, source) {
    if (source === 'presenter' && !this.model.liveSync) this.model.preview = position;
    else {
      Object.assign(this.model, position);
      if (this.model.liveSync) this.model.preview = this.position();
      this.pace();
    }
  }
  presentSlide() { this.navigate({ ...this.model.preview }); }
  next(source) {
    const m = this.position(source);
    if (m.step < (this.counts[m.key] ?? 0)) m.step++;
    else if (m.index < this.deck.slides.length - 1) { m.index++; m.step = 0; }
    m.key = this.deck.slides[m.index].key; this.navigate(m, source);
  }
  previous(source) {
    const m = this.position(source);
    if (m.step > 0) m.step--;
    else if (m.index > 0) { m.index--; m.step = this.counts[this.deck.slides[m.index].key] ?? 0; }
    m.key = this.deck.slides[m.index].key; this.navigate(m, source);
  }
  jump(key, source) {
    const index = this.deck.slides.findIndex(s => s.key === key);
    if (index < 0) throw new Error(`unknown slide: ${key}`);
    this.model.overview = false;
    this.navigate({ index, key, step: 0 }, source);
  }
  reload(deck) {
    const fresh = new Presentation(deck, this.options, this);
    Object.assign(this, fresh); this.pace();
    return this.model;
  }
  elapsed() { return this.model.elapsed + (this.model.running ? this.now() - this.startedAt : 0); }
  update(message) {
    const m = this.model, kind = message.presentation;
    if (kind === 'next') this.next(message.source);
    else if (kind === 'previous') this.previous(message.source);
    else if (kind === 'jump') this.jump(message.slide, message.source);
    else if (kind === 'present-slide') this.presentSlide();
    else if (kind === 'live-sync') {
      const enabled = message.toggle?.on === true;
      if (!enabled && m.liveSync) m.preview = this.position();
      if (enabled && !m.liveSync) this.presentSlide();
      m.liveSync = enabled;
      if (enabled) m.preview = this.position();
    }
    else if (kind === 'start') this.start();
    else if (kind === 'overview') { m.overview = !m.overview; this.update(msg('presenter')); }
    else if (kind === 'help') { m.help = true; m.confirmRestart = false; this.update(msg('presenter')); }
    else if (kind === 'help-close') m.help = false;
    else if (kind === 'restart-request') { m.confirmRestart = true; m.help = false; }
    else if (kind === 'restart-cancel') m.confirmRestart = false;
    else if (kind === 'menu' && ['presenter', 'overview', 'help'].includes(message.menu?.item)) this.update(msg(message.menu.item));
    else if (kind === 'presenter') {
      m.presenter = true;
      if (this.presenterId != null) this.requests.push({ kind: 'focus', id: this.presenterId });
    } else if (kind === 'presenter-window' && message.window?.phase === 'closed') { m.presenter = false; this.presenterId = undefined; }
    else if (kind === 'display') m.display = message.change.value;
    else if (kind === 'audience-layout') this.size = this.audienceSize = { width: message.layout.w, height: message.layout.h };
    else if (kind === 'audience' || kind === 'fullscreen') this.requests.push({ kind: 'fullscreen', enabled: kind === 'audience' || !this.fullscreen });
    else if (kind === 'timer') {
      if (m.running) { m.elapsed = this.elapsed(); m.running = false; this.timer?.(); this.timer = undefined; }
      else {
        m.running = true; this.startedAt = this.now();
        const schedule = this.options.every ?? ((fn, ms) => { const id = setInterval(fn, ms); return () => clearInterval(id); });
        this.timer = schedule(() => this.send(msg('tick')), 1000);
      }
    } else if (kind === 'reset-timer') { m.elapsed = 0; this.startedAt = this.now(); }
    else if (kind === 'restart') {
      this.stopActive(); m.confirmRestart = false; m.elapsed = 0; this.startedAt = this.now();
      for (const slide of this.deck.slides) m.states[slide.key] = slide.init?.() ?? {};
      this.jump(this.deck.slides[0].key);
      m.preview = this.position();
    } else if (kind === 'demo' && message.mode === 'live' && this.active === message.slide && message.generation === this.generation) {
      const slide = this.deck.slides[m.index];
      const arrived = message.message && typeof message.message === 'object' ? { ...message.message } : { tag: message.message };
      for (const [k, v] of Object.entries(message)) if (!['presentation', 'slide', 'generation', 'mode', 'message'].includes(k)) arrived[k] = v;
      const value = slide.update?.(m.states[slide.key], arrived);
      if (value !== undefined) m.states[slide.key] = value;
    } else if (kind === 'keys' && message.key?.phase === 'down') {
      const k = message.key, code = k.code.toLowerCase();
      if (k.repeat && !['left', 'right', 'up', 'down', 'pageup', 'pagedown'].includes(code)) return m;
      if (presenterShortcut(this.deck, k)) this.update(msg('presenter'));
      else if ((k.super || k.ctrl) && code === 'enter' && message.source === 'presenter') this.presentSlide();
      else if (!(k.super || k.ctrl || k.alt)) {
        if (code === '?' || (code === '/' && k.shift)) this.update(msg('help'));
        else if (['right', 'down', 'pagedown', 'space'].includes(code)) k.shift ? this.previous(message.source) : this.next(message.source);
        else if (['left', 'up', 'pageup'].includes(code)) this.previous(message.source);
        else if (code === 'home') this.jump(this.deck.slides[0].key, message.source);
        else if (code === 'end') this.jump(this.deck.slides.at(-1).key, message.source);
        else if (code === 'o') this.update(msg('overview'));
        else if (code === 'f') this.update(msg('fullscreen'));
        else if (code === 'escape') {
          if (m.help) m.help = false; else if (m.confirmRestart) m.confirmRestart = false;
          else if (m.overview) { m.overview = false; this.pace(); } else if (this.fullscreen) this.update(msg('fullscreen'));
        }
        else if (code === 'tab') this.tab = { window: message.windowId ?? 0, direction: k.shift ? -1 : 1 };
      }
    }
    return m;
  }
  renderSlide(index, step, mode = 'live', size = this.size, revealGuide = false) {
    const slide = this.deck.slides[index];
    this.counts[slide.key] = slide.steps ?? (mode === 'snapshot' ? this.counts[slide.key] : 0) ?? 0;
    try {
      const node = (mode === 'snapshot' && slide.snapshot ? slide.snapshot : slide.view)(this.context(index, step, mode, this.deck.canvas ?? size, revealGuide));
      return mode === 'snapshot' ? still(node) : node;
    } catch (error) { throw new Error(`slide ${slide.key}, step ${step}: ${error.message}`, { cause: error }); }
  }
  settings() { return kit.Settings({ ...this.deck.settings, theme: this.model.theme }); }
  preview(index, width, height, step = 0, action) {
    const size = sizeOf(this.deck.canvas ?? this.size), scale = Math.min(width / size.width, height / size.height);
    return box({ width, height, clip: true, role: 'none', bg: '$kitBg' },
      box({ width: size.width, height: size.height, scale, pivotX: 0, pivotY: 0 }, this.renderSlide(index, step, 'snapshot', size, true)),
      box({ width, height, float: { anchor: 'parent' }, keepFocus: true, cursor: action ? 'pointer' : undefined, onClick: action ?? msg('preview'), onHover: msg('preview') }));
  }
  overview() {
    return box({ width: 'grow', height: 'grow', pad: 32, gap: 24, scrollY: true }, { type: 'text', props: { size: '$kitSizeTitle', family: '$kitUi', color: '$kitFg', bold: true }, children: ['Overview'] },
      row({ width: 'grow', gap: 20, crossGap: 20, minColumn: 320 }, this.deck.slides.map((slide, index) =>
        box({ width: 'grow', gap: 10 }, this.preview(index, 320, 200, undefined, msg('jump', { slide: slide.key, source: 'presenter' })),
          kit.Button({ label: `${index + 1}  ${slide.title ?? slide.key}`, onPress: msg('jump', { slide: slide.key, source: 'presenter' }) })))));
  }
  menu(env) {
    if (this.options.snapshot || env.platform !== 'macos') return undefined;
    return { type: 'menuBar', props: { standardMenus: true, onMenu: msg('menu'), menu: [{ label: 'Presentation', items: [
      { label: 'Show Presenter', id: 'presenter', accel: presenterAccel(this.deck) },
      { label: 'Slide Overview', id: 'overview', accel: 'mod+shift+o' },
      { label: 'Keyboard Shortcuts', id: 'help', accel: 'mod+/' },
    ] }] }, children: [] };
  }
  dialogs(env) {
    const m = this.model;
    if (m.help) {
      const keys = [
        ['→ / ↓ / Space', 'Next reveal or slide (also Page Down)'], ['← / ↑ / Shift+Space', 'Previous reveal or slide (also Page Up)'],
        ['Home / End', 'First or last slide'], ['O', 'Private slide overview'], ['F', 'Audience fullscreen'],
        [env.platform === 'macos' ? '⌘Enter' : 'Ctrl+Enter', 'Present the selected slide from the presenter'],
        ['Esc', 'Close a dialog, overview or fullscreen'], [presenterKeyLabel(this.deck, env.platform), 'Open or focus the presenter'],
        ['?', 'Show these shortcuts'], ['Tab / Shift+Tab', 'Move between controls; focused controls keep their keys'],
      ];
      return kit.Dialog({ title: 'Keyboard shortcuts', width: Math.min(540, env.viewport.width - 32), onDismiss: msg('help-close'),
        actions: [kit.Button({ label: 'Close shortcuts', onPress: msg('help-close') })] }, [
        text('These controls are private. The audience stays on the current slide.', '$kitSize', '$kitMuted'),
        ...keys.map(([key, action]) => row({ width: 'grow', gap: 12 }, box({ width: 180 }, kit.Badge({ label: key })), box({ width: 'grow' }, text(action)))),
      ]);
    }
    if (m.confirmRestart) return kit.Dialog({ title: 'Restart this talk?', width: 420, onDismiss: msg('restart-cancel'), actions: [
      kit.Button({ label: 'Keep presenting', onPress: msg('restart-cancel') }),
      kit.Button({ label: 'Restart talk', variant: 'danger', onPress: msg('restart') }),
    ] }, text('Return to the first slide and reset every demo and the timer.'));
  }
  presenter(surface, env) {
    const m = this.model, selected = this.position('presenter'), slide = this.deck.slides[selected.index]; this.presenterId = env.window.id;
    const wide = env.viewport.width >= 900;
    const width = Math.max(240, Math.min(900, env.viewport.width - (wide ? 392 : 56)));
    const next = Math.min(selected.index + 1, this.deck.slides.length - 1), nextWidth = wide ? 296 : Math.min(400, width);
    const elapsed = Math.floor(this.elapsed() / 1000);
    const integrated = this.options.toolbarInTitlebar === true && env.window.customChrome;
    const current = box({ width: 'grow', height: wide ? 'grow' : undefined, gap: 16 },
      kit.Panel({ title: m.liveSync ? 'Current slide' : 'Private preview', subtitle: slide.title ?? slide.key },
        this.preview(selected.index, width, width * this.size.height / this.size.width, selected.step)),
      kit.Panel({ title: 'Speaker notes', grow: wide, scroll: wide }, kit.Markdown({ text: slide.notes || 'No notes for this slide.' })));
    const sidebar = box({ width: wide ? 320 : 'grow', height: wide ? 'grow' : undefined, gap: 16, scrollY: wide },
      kit.Panel({ title: 'Up next', subtitle: this.deck.slides[next].title ?? this.deck.slides[next].key },
        this.preview(next, nextWidth, nextWidth * this.size.height / this.size.width, 0)),
      kit.Panel({ title: 'Timer' }, [
        kit.Stat({ label: 'Elapsed', value: `${Math.floor(elapsed / 60).toString().padStart(2, '0')}:${(elapsed % 60).toString().padStart(2, '0')}` }),
        row({ gap: 6 }, kit.Button({ label: m.running ? 'Pause timer' : 'Start timer', icon: m.running ? 'pause' : 'play', onPress: msg('timer') }),
          kit.Button({ label: 'Reset timer', onPress: msg('reset-timer') })),
      ]),
      kit.Panel({ title: 'Audience' }, [
        kit.Select({ label: 'Display', value: m.display, options: [{ value: 'current', label: 'Current display' },
          ...surface.displayNames().map((name, i) => ({ value: String(i), label: name }))], onChange: msg('display') }),
        kit.Button({ label: 'Show audience', icon: 'external', onPress: msg('audience') }),
      ]));
    const body = m.overview ? this.overview() : box({ dir: wide ? 'row' : 'column', width: 'grow', height: 'grow', pad: 16, gap: 16, scrollY: !wide }, current, sidebar);
    return box({ width: 'grow', height: 'grow', bg: '$kitBg' },
      env.window.customChrome && !integrated ? { type: 'titlebar', props: { title: `${this.deck.title ?? 'Presentation'} — Presenter` }, children: [] } : undefined,
      kit.Toolbar({ windowChrome: integrated, trailing: [
        kit.Button({ label: 'Restart…', icon: 'reset', onPress: msg('restart-request') }),
        kit.Button({ label: 'Previous', disabled: selected.index === 0 && selected.step === 0, onPress: msg('previous', { source: 'presenter' }) }),
        kit.Button({ label: 'Next', variant: m.liveSync ? 'primary' : 'default',
          disabled: selected.index === this.deck.slides.length - 1 && selected.step >= (this.counts[selected.key] ?? 0),
          onPress: msg('next', { source: 'presenter' }) }),
        !m.liveSync ? kit.Button({ label: 'Present slide', variant: 'primary', onPress: msg('present-slide') }) : undefined,
      ] }, [{ type: 'text', props: { size: '$kitSizeTitle', bold: true, family: '$kitUi', color: '$kitFg' }, children: [this.deck.title ?? 'Presentation'] },
        kit.Button({ label: m.overview ? 'Back to notes' : 'Overview', onPress: msg('overview') })]),
      body, kit.Statusbar({ trailing: [
        !m.liveSync ? text(`Audience: slide ${m.index + 1} · step ${m.step}`, '$kitSize', '$kitMuted') : undefined,
        kit.Switch({ key: 'live-sync', label: 'Live sync', on: m.liveSync, onToggle: msg('live-sync') }), text('?  Shortcuts', '$kitSize', '$kitMuted')] }, [
        text(`Slide ${selected.index + 1} of ${this.deck.slides.length}`, '$kitSize', '$kitMuted'), text(`Step ${selected.step} of ${this.counts[selected.key]}`, '$kitSize', '$kitMuted'),
      ]), this.dialogs(env));
  }
  view(surface, window = 'main') {
    const facts = surface.env(), fallback = surface.size(), m = this.model;
    const env = { ...facts, viewport: { width: facts.viewport.width || fallback.width, height: facts.viewport.height || fallback.height } };
    if (!this.initializedWindows.has(env.window.id)) { surface.setWindowCloseHides(env.window.id, true); this.initializedWindows.add(env.window.id); }
    for (const path of this.deck.fonts ?? []) if (!this.loadedFonts.has(path)) {
      if (surface.loadFontFile(path) == null) throw new Error(`could not load font ${path}`);
      this.loadedFonts.add(path);
    }
    if (this.tab?.window === env.window.id) { this.tab.direction < 0 ? surface.focusPrev() : surface.focusNext(); this.tab = undefined; }
    const presenter = window === 'presenter';
    if (!presenter) {
      this.size = this.audienceSize ?? { width: env.viewport.width, height: env.viewport.height }; this.fullscreen = env.window.fullscreen;
    }
    const requests = this.requests; this.requests = [];
    for (const request of requests) {
      if (request.kind === 'focus') { surface.setWindowVisible(request.id, true); surface.focusWindow(request.id); }
      else {
        surface.setWindowVisible(0, true);
        surface.setWindowFullscreen(0, request.enabled, m.display === 'current' ? undefined : Number(m.display));
        surface.focusWindow(0);
      }
    }
    let content;
    if (presenter) content = this.presenter(surface, env);
    else {
      let node = this.renderSlide(m.index, m.step, this.options.snapshot ? 'snapshot' : 'live');
      if (this.deck.canvas) {
        const size = sizeOf(this.deck.canvas), scale = Math.min(this.size.width / size.width, this.size.height / size.height);
        node = box({ width: size.width, height: size.height, scale, pivotX: 0, pivotY: 0,
          float: { anchor: 'parent', dx: (this.size.width - size.width * scale) / 2, dy: (this.size.height - size.height * scale) / 2 } }, node);
      }
      const slide = box({ key: `slide.${m.key}`, width: 'grow', height: 'grow', clip: true, transition: this.options.snapshot ? 0 : 180,
        onLayout: msg('audience-layout'), enter: this.options.snapshot ? undefined : { opacity: 0 }, when: [{ reducedMotion: true, transition: 0 }] }, node);
      content = box({ width: 'grow', height: 'grow' },
        !this.options.snapshot && !env.window.fullscreen && env.window.customChrome ? { type: 'titlebar', props: { title: this.deck.title ?? 'Presentation' }, children: [] } : undefined, slide);
    }
    return box({ width: 'grow', height: 'grow', title: `${this.deck.title ?? 'Presentation'} — ${presenter ? 'Presenter' : 'Audience'}`,
      onKey: msg('keys', { windowId: env.window.id, source: presenter ? 'presenter' : 'audience' }), keyFocus: (surface.focused?.() ?? null) == null || undefined,
      windows: !presenter && m.presenter && !this.options.snapshot ? [{ name: 'presenter', width: 1180, height: 860, onWindow: msg('presenter-window') }] : [] },
      this.settings(), this.menu(env), box({ width: 'grow', height: 'grow', bg: '$kitBg' }, content));
  }
  exportPlan(allSteps = false) {
    const frames = [];
    this.deck.slides.forEach((slide, index) => {
      this.renderSlide(index, 0, 'snapshot'); const last = this.counts[slide.key] ?? 0;
      for (let step = allSteps ? 0 : last; step <= last; step++) frames.push({ key: slide.key, title: slide.title ?? slide.key, index: index + 1, step });
    });
    return frames;
  }
}

export function createPresentation(deck, options) { return new Presentation(deck, options); }

/** Start a standalone deck. Pass a module path/URL for --watch support;
 * that module must export a deck as default and have no launch side effect. */
export async function present(source, options = {}) {
  const { runWindowed } = await import('../index.js');
  const { extension } = await import('./index.js');
  const { pathToFileURL, fileURLToPath } = await import('node:url');
  const { resolve, dirname } = await import('node:path');
  const args = options.args ?? process.argv.slice(2);
  const flags = {};
  for (let i = 0; i < args.length; i++) if (args[i].startsWith('--')) flags[args[i].slice(2)] = args[i + 1] ?? true;
  const url = typeof source === 'string' ? pathToFileURL(resolve(source)) : source instanceof URL ? source : undefined;
  const override = loaded => {
    const deck = { ...loaded, theme: flags.theme ?? loaded.theme, layout: flags.layout ?? loaded.layout };
    if (flags.canvas) {
      const match = /^(\d+)x(\d+)$/.exec(flags.canvas);
      if (!match || Number(match[1]) <= 0 || Number(match[2]) <= 0) throw new Error('--canvas needs WIDTHxHEIGHT');
      deck.canvas = { w: Number(match[1]), h: Number(match[2]) };
    }
    return deck;
  };
  let revision = 0;
  const load = async () => {
    if (!url) return override(source);
    const fresh = new URL(url); fresh.searchParams.set('kui', `${Date.now()}-${++revision}`);
    return override((await import(fresh.href)).default);
  };
  const deck = await load();
  let size = options.size;
  if (flags.size) {
    const match = /^(\d+)x(\d+)$/.exec(flags.size);
    if (!match || Number(match[1]) <= 0 || Number(match[2]) <= 0) throw new Error('--size needs positive WIDTHxHEIGHT');
    size = { width: Number(match[1]), height: Number(match[2]) };
  }
  const appearance = flags.appearance ?? options.appearance;
  if (appearance !== undefined && !['light', 'dark'].includes(appearance)) throw new Error('appearance must be light or dark');
  if (flags.export || options.export) {
    return exportPresentation(deck, { ...options, size, directory: flags.export ?? options.export,
      scale: Number(flags.scale ?? options.scale ?? 1), appearance,
      allSteps: flags.steps !== undefined, timestamp: Number(flags.time ?? 0) });
  }
  if (flags.scale !== undefined) throw new Error('--scale applies only to --export; live windows use the display scale');
  const controller = createPresentation(deck, { ...options, size, appearance, presenter: options.presenter || flags.presenter !== undefined });
  let watch;
  return runWindowed({ init: controller.model,
    update: (_model, message) => message.presentation === 'reload' ? controller.reload(message.deck) : controller.update(message),
    view: (_model, window, surface) => controller.view(surface, window),
    teardown: () => { watch?.close(); controller.dispose(); },
  }, { ...options, width: size?.width ?? options.width, height: size?.height ?? options.height,
    system: { ...options.system, ...(appearance === undefined ? {} : { appearance }) },
    extensions: [extension, ...(options.extensions ?? [])], title: deck.title ?? 'Presentation',
    setup: async (_surface, app) => {
      controller.options.dispatch = message => app.dispatch(message); controller.start();
      if ((flags.watch || options.watch) && url) {
        const { watch: watchFiles } = await import('node:fs');
        let reloading = false, again = false;
        const reload = async () => {
          if (reloading) { again = true; return; }
          reloading = true;
          try { app.dispatch(msg('reload', { deck: await load() })); }
          catch (error) { (options.onError ?? console.error)(error); }
          finally { reloading = false; if (again) { again = false; void reload(); } }
        };
        watch = watchFiles(dirname(fileURLToPath(url)), (_event, name) => { if (name === undefined || resolve(dirname(fileURLToPath(url)), name) === fileURLToPath(url)) void reload(); });
      }
      options.setup?.(app, controller);
    } });
}

/** PNG sequence + manifest, rendered in independent headless contexts. */
export async function exportPresentation(deck, options = {}) {
  const { createApp } = await import('../index.js');
  const { extension } = await import('./index.js');
  const { mkdir, writeFile } = await import('node:fs/promises');
  const { resolve, join } = await import('node:path');
  const directory = resolve(options.directory ?? 'slides');
  const size = sizeOf(options.size), timestamp = options.timestamp ?? 0;
  if (![size.width, size.height].every(n => Number.isFinite(n) && n > 0)) throw new Error('export size must be finite and positive');
  if (options.scale !== undefined && (!Number.isFinite(options.scale) || options.scale < 0.25 || options.scale > 8)) throw new Error('export scale must be between 0.25 and 8');
  if (options.appearance !== undefined && !['light', 'dark'].includes(options.appearance)) throw new Error('appearance must be light or dark');
  if (!Number.isFinite(timestamp) || timestamp < 0) throw new Error('export timestamp must be finite and nonnegative');
  await mkdir(directory, { recursive: true });
  const planController = createPresentation(deck, { snapshot: true, snapshotTime: timestamp, size });
  const frames = planController.exportPlan(options.allSteps); planController.dispose();
  for (let i = 0; i < frames.length; i++) {
    const frame = frames[i], image = `${String(i + 1).padStart(4, '0')}.png`;
    const controller = createPresentation(deck, { snapshot: true, snapshotTime: timestamp, size });
    controller.jump(frame.key); controller.model.step = frame.step;
    const app = createApp({ init: controller.model, update: (_, m) => controller.update(m), view: (_m, window, surface) => controller.view(surface, window) },
      { width: size.width, height: size.height, scale: options.scale ?? 1, startTime: timestamp, extensions: [extension], warnings: false });
    try {
      app.ctx.setEnv({ system: { appearance: options.appearance ?? 'light', motion: 'reduced' } });
      app.render(); app.settle(); app.render();
      if (app.warnings.length) throw new Error(app.warnings.map(w => w.message ?? w.code).join('\n'));
      app.ctx.captureTo(join(directory, image), msg('capture'));
      app.ctx.serveCaptures();
      const capture = app.ctx.pollEvents().find(event => event.event === 'capture')?.payload?.capture;
      if (capture?.phase !== 'saved') throw new Error(capture?.error ?? 'capture did not finish');
      Object.assign(frame, { image, timestamp_ms: timestamp });
    } catch (error) { throw new Error(`export slide ${frame.key}, step ${frame.step}: ${error.message}`, { cause: error }); }
    finally { app.teardown(); controller.dispose(); }
  }
  const manifest = { version: 1, ...size, scale: options.scale ?? 1, appearance: options.appearance ?? 'light', frames };
  await writeFile(join(directory, 'manifest.json'), JSON.stringify(manifest, null, 2));
  return manifest;
}
