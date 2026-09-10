// The devtools every Node example runs inside — the twin of
// examples/devtools (Rust) with the same flags, the same chords and the same
// dock (docs/adr/0021, decision 6). An example is one subject; what every
// example used to carry beside it lives here: the window title, the CLI,
// and a dock beside the example's tree showing what the runtime is doing.
// The dock's header is the example's name, the frame counter and an icon
// strip (base, accent, native menus, dock); under it three tabs — facts
// (the latency graph, the status block: `env()`, the theme, focus, the
// windows, `env().audio`, and the key legend), events (every message the
// example's `update` is handed, as the data it is, and every warning the
// core raised), and tree (the last frame's nodes from `win.nodes()`, with
// an inspector for the one clicked).
//
//   --headless              run the example's `headless(app)` self-check
//   --dock side|bottom|off  where the dock sits
//   --light, --dark         pin the theme base
//   --accent #rrggbb        the accent, instead of the OS's
//
// Chords: Ctrl+Shift+T cycles the base, +A the accent, +M toggles native
// menus, +D moves the dock, +C clears the stream, +N the next tab.
//
// The dock is a sibling of the example's tree, so the example's root box
// is a child of the devtools'; `title` and `windows` belong on the root and
// so are the devtools' to declare (an example that declares windows passes
// them through `windows`). The dock carries `role="none"`, so neither the
// Tab ring nor assistive technology sees it. `--dock off` also declares no
// key sink and takes no focus.
import { createApp, runWindowed, withEffects } from '@qxuken/kui';
import type { App, CoreMsg, KeyMsg, KuiNode, KuiWindow, UiEvent, WindowDecl } from '@qxuken/kui';

export type Dock = 'side' | 'bottom' | 'off';

export type Example<M, A extends { kind: string }> = {
  /** The subject, as the README names it: the window title is `kui — name`. */
  name: string;
  init: M | ((win: KuiWindow) => M);
  update: (model: M, msg: A | CoreMsg, ev: UiEvent<A | CoreMsg>, win: KuiWindow) => M | undefined | void;
  view: (model: M, win: KuiWindow) => KuiNode;
  /** Runs before the first frame, with the window: register resources here. */
  setup?: (win: KuiWindow) => void;
  windows?: (model: M) => WindowDecl[];
  /** The key legend the dock shows. */
  keys?: [string, string][];
  /** The example's own flags, for the usage text. */
  flags?: [string, string][];
  window?: { width?: number; height?: number; minWidth?: number; minHeight?: number; chrome?: 'native' | 'custom' | 'borderless' };
  /** Where the dock goes unless `--dock` says. */
  dock?: Dock;
  /** `false` asks for the core's drawn menus to start with. */
  nativeMenus?: boolean;
  /** The self-check `--headless` runs over a bare `createApp`: return
   *  false (or throw) on a wrong answer. */
  headless?: (app: App<M, A | CoreMsg>) => boolean | Promise<boolean>;
};

/** The root's key as Node spells it: the FNV-1a offset basis, since the
 *  root is the path of no segments. */
const ROOT_KEY = 'cbf29ce484222325';
const DOCK_SIDE_W = 330;
const DOCK_BOTTOM_H = 280;
const DOCK_SIDE_MIN_H = 600;
const DOCK_BOTTOM_MIN_W = 640;
const STREAM_CAP = 64;
const ACCENTS: [string, number][] = [
  ['kui blue', 0x3b5bd4ff],
  ['macOS blue', 0x007affff],
  ['macOS yellow', 0xffc409ff],
  ['macOS pink', 0xf74f9eff],
  ['forest', 0x2f7d4fff],
];

type Cli = { headless: boolean; dock?: Dock; base?: 'light' | 'dark'; accent?: string };

function usage(name: string, flags: [string, string][]): string {
  const own = flags.map(([f, d]) => `  ${f.padEnd(16)} ${d}`).join('\n');
  return (
    `usage: ${name} [--headless] [--dock side|bottom|off] [--light|--dark] [--accent #rrggbb]` +
    (flags.length ? ' ' + flags.map(([f]) => `[${f}]`).join(' ') : '') +
    '\n' +
    (own ? '\n' + own + '\n' : '')
  );
}

export function parseCli(name: string, argv: string[], flags: [string, string][] = []): Cli {
  const cli: Cli = { headless: false };
  const args = [...argv];
  while (args.length) {
    const arg = args.shift()!;
    const [flag, inline] = arg.includes('=') ? arg.split(/=(.*)/s, 2) : [arg, undefined];
    const value = () => inline ?? args.shift();
    switch (flag) {
      case '--headless': cli.headless = true; break;
      case '--dock': {
        const v = value();
        if (v !== 'side' && v !== 'bottom' && v !== 'off') throw new Error(`--dock: ${v} is not side, bottom or off`);
        cli.dock = v; break;
      }
      case '--light': cli.base = 'light'; break;
      case '--dark': cli.base = 'dark'; break;
      case '--accent': {
        const v = value();
        if (!v || !/^#[0-9a-fA-F]{6}$/.test(v)) throw new Error(`--accent: ${v} is not #rrggbb`);
        cli.accent = v; break;
      }
      case '-h': case '--help': throw new Error(usage(name, flags));
      default:
        if (flags.some(([f]) => f === flag)) break;
        if (flag.startsWith('-')) throw new Error(`unknown flag ${flag}\n\n${usage(name, flags)}`);
    }
  }
  return cli;
}

type Entry = { frame: number; kind: 'event' | 'note' | 'warning'; text: string };
type Tab = 'facts' | 'events' | 'tree';
const TABS: Tab[] = ['facts', 'events', 'tree'];
type HarnessState = {
  dock: Dock;
  base: 'light' | 'dark' | null;
  accent: number | null;
  customAccent: string | null;
  nativeMenus: boolean | null;
  stream: Entry[];
  frames: number;
  tab: Tab;
  /** The node the tree tab selected, outlined over the example. */
  selected: string | null;
  /** How many of `warningsRaised()` are already in the stream. */
  warningsSeen: number;
};
type HarnessMsg = { kind: '@harness'; what: string };
type Wrapped<M> = { app: M; h: HarnessState };

/** A `Value` as one line of data: `{kind: click, n: 3}`. */
export function fmtValue(v: unknown): string {
  if (v === null || v === undefined) return 'null';
  if (typeof v === 'string') return /[\s,}]/.test(v) || v === '' ? JSON.stringify(v) : v;
  if (typeof v === 'number') return Number.isInteger(v) ? String(v) : v.toFixed(2);
  if (typeof v === 'boolean') return String(v);
  if (Array.isArray(v)) return `[${v.map(fmtValue).join(', ')}]`;
  if (typeof v === 'object') return `{${Object.entries(v).map(([k, x]) => `${k}: ${fmtValue(x)}`).join(', ')}}`;
  return String(v);
}

/** Runs `example` under the harness: the CLI, then either its headless
 *  drive or a window with the dock beside it. Exits the process. */
export async function run<M, A extends { kind: string }>(example: Example<M, A>): Promise<never> {
  let cli: Cli;
  try {
    cli = parseCli(example.name, process.argv.slice(2), example.flags ?? []);
  } catch (e) {
    console.error((e as Error).message);
    process.exit(2);
  }
  if (cli.headless) {
    if (!example.headless) {
      console.error(`${example.name}: no headless drive`);
      process.exit(2);
    }
    const w = example.window?.width ?? 960;
    const h = example.window?.height ?? 640;
    // The drive's own surface: `view` is handed the same object the window
    // would be, since `Ctx` and `KuiWindow` read alike.
    let surface: KuiWindow | null = null;
    const app: App<M, A | CoreMsg> = createApp<M, A | CoreMsg>(
      {
        init: example.init as M,
        update: (m: M, msg: A | CoreMsg, ev: UiEvent<A | CoreMsg>): M | undefined =>
          example.update(m, msg, ev, surface!) ?? undefined,
        view: (m: M): KuiNode => example.view(m, surface!),
      },
      { width: w, height: h },
    );
    surface = app.ctx as unknown as KuiWindow;
    let ok = false;
    try {
      ok = await example.headless(app);
    } catch (e) {
      console.error(`${example.name}: FAILED: ${(e as Error).message ?? e}`);
      process.exit(1);
    }
    console.log(ok ? `${example.name}: headless drive OK` : `${example.name}: FAILED`);
    process.exit(ok ? 0 : 1);
  }

  const dock = cli.dock ?? example.dock ?? 'side';
  let w = example.window?.width ?? 960;
  let h = example.window?.height ?? 640;
  if (dock === 'side') { w += DOCK_SIDE_W; h = Math.max(h, DOCK_SIDE_MIN_H); }
  if (dock === 'bottom') { h += DOCK_BOTTOM_H; w = Math.max(w, DOCK_BOTTOM_MIN_W); }
  const smokeFrames = Number(process.env.KUI_SMOKE_FRAMES) || null;
  let win_: KuiWindow | null = null;

  const state = (): HarnessState => ({
    dock,
    base: cli.base ?? null,
    accent: null,
    customAccent: cli.accent ?? null,
    nativeMenus: example.nativeMenus ?? null,
    stream: [],
    frames: 0,
    tab: 'events',
    selected: null,
    warningsSeen: 0,
  });
  const push = (hs: HarnessState, kind: Entry['kind'], text: string) => {
    if (hs.stream.length >= STREAM_CAP) hs.stream.shift();
    hs.stream.push({ frame: hs.frames, kind, text });
  };
  const baseName = (hs: HarnessState) => hs.base ?? 'OS';
  const accentName = (hs: HarnessState) =>
    hs.accent !== null ? ACCENTS[hs.accent][0] : hs.customAccent ?? 'OS';

  /** Applies the theme and menus the state asks for to the window. */
  const apply = (hs: HarnessState) => {
    const win = win_!;
    const accent = hs.accent !== null ? ACCENTS[hs.accent][1] : hs.customAccent;
    if (hs.base) win.setTheme(accent ? { appearance: hs.base, accent } : { appearance: hs.base });
    else win.setAccent(accent ?? null);
    // Both the popup menus and the bar, like the Rust twin: on macOS the
    // bar is the platform's by default, and "drawn" is how the strip is
    // seen on that host at all.
    if (hs.nativeMenus !== null) {
      win.setNativeMenus(hs.nativeMenus);
      win.setNativeMenuBar(hs.nativeMenus);
    }
  };

  const act = (hs: HarnessState, what: string): HarnessState => {
    const next = { ...hs, stream: [...hs.stream] };
    switch (what) {
      case 'base':
        next.base = hs.base === null ? 'light' : hs.base === 'light' ? 'dark' : null;
        push(next, 'note', `theme base: ${baseName(next)}`);
        break;
      case 'accent':
        next.customAccent = null;
        next.accent = hs.accent === null ? 0 : hs.accent + 1 < ACCENTS.length ? hs.accent + 1 : null;
        push(next, 'note', `accent: ${accentName(next)}`);
        break;
      case 'menus':
        next.nativeMenus = !(hs.nativeMenus ?? process.platform === 'darwin');
        push(next, 'note', `menus: ${next.nativeMenus ? 'native' : 'drawn'}`);
        break;
      case 'dock':
        next.dock = hs.dock === 'side' ? 'bottom' : hs.dock === 'bottom' ? 'off' : 'side';
        push(next, 'note', `dock: ${next.dock}`);
        break;
      case 'clear':
        next.stream = [];
        break;
      case 'tab':
        next.tab = TABS[(TABS.indexOf(hs.tab) + 1) % TABS.length];
        break;
      default:
        if (what.startsWith('tab:')) next.tab = what.slice(4) as Tab;
        else if (what.startsWith('node:')) {
          const key = what.slice(5);
          next.selected = hs.selected === key ? null : key;
        }
    }
    apply(next);
    return next;
  };

  const chord = (msg: CoreMsg): string | null => {
    if (msg.kind !== 'key') return null;
    const k = msg as KeyMsg;
    if (k.phase !== 'down' || !k.ctrl || !k.shift || k.alt) return null;
    const c = (k.code || '').toLowerCase();
    const table: Record<string, string> = { t: 'base', a: 'accent', m: 'menus', d: 'dock', c: 'clear', n: 'tab' };
    return table[c] ?? null;
  };

  const small = (what: string, text: string, hint: string, t: ReturnType<KuiWindow['theme']>) => (
    <box key={`h-${what}`} padX={7} padY={1} radius={4} bg={t.raised} hoverBg={t.hover} pressedBg={t.pressed}
         borderW={1} borderColor={t.border} onClick={{ kind: '@harness', what }} tooltip={hint}>
      <text size={10} color={t.fg}>{text}</text>
    </box>
  );

  const status = (hs: HarnessState, win: KuiWindow) => {
    const t = win.theme();
    const env = win.env();
    const hex = (c: number | null) => (c === null ? '—' : '#' + (c >>> 8).toString(16).padStart(6, '0'));
    const focus = win.focused();
    const rows: [string, string][] = [
      ['appearance', env.system.appearance],
      ['motion', env.system.motion],
      ['locale', env.system.locale ?? '—'],
      ['base', `${t.appearance} · ${hs.base ? 'pinned' : 'derived'}`],
      ['accent', `${hex(t.accent)} · ${accentName(hs)}`],
      ['menus', hs.nativeMenus === null ? 'default' : hs.nativeMenus ? 'native' : 'drawn'],
      ['window', `#${env.window.id}${env.window.customChrome ? ' custom-chrome' : ''}${env.window.maximized ? ' maximized' : ''} · ${win.windows().join(' ')}`],
      ['viewport', `${Math.round(env.viewport.width)}×${Math.round(env.viewport.height)} @${env.viewport.scale} · ${env.refreshHz ? `${Math.round(env.refreshHz)} Hz` : '—'}`],
      ['keyboard', env.focused ? 'this window' : 'elsewhere'],
      ['focus', focus ? `${focus === ROOT_KEY ? 'root' : focus}${win.focusVisible() ? ' · ring' : ''}` : '—'],
      ['audio', `${env.audio.device} · ${env.audio.live} live`],
    ];
    return (
      <box width="grow" minHeight="fit" gap={3}>
        {rows.map(([k, v]) => (
          <box key={`row-${k}`} dir="row" width="grow" gap={8} crossAlign="center">
            <box width={70}><text size={11} color={t.muted}>{k}</text></box>
            <text size={11} color={t.fg} family="mono" wrap="none">{v}</text>
          </box>
        ))}
      </box>
    );
  };

  const streamPanel = (hs: HarnessState, win: KuiWindow, height: 'grow' | number) => {
    const t = win.theme();
    // The core's warnings since the last frame, into the stream: the
    // runner drains and prints them, so this reads the log it leaves.
    const raised = win.warningsRaised();
    if (raised.length > hs.warningsSeen) {
      for (const w of raised.slice(hs.warningsSeen)) push(hs, 'warning', `warning [${w.code}] ${w.message}`);
      hs.warningsSeen = raised.length;
    }
    return (
      <box width="grow" height={height} minHeight={60} gap={4}>
        <box dir="row" width="grow" minHeight="fit" gap={8} crossAlign="center">
          <text size={11} color={t.muted}>{`${hs.stream.length} of the last ${STREAM_CAP}`}</text>
          <box width="grow" />
          {small('clear', 'clear', 'Ctrl+Shift+C · empty the stream', t)}
        </box>
        <box key="stream" width="grow" height="grow" minHeight={40} bg={t.sunken} radius={6} pad={6} gap={1} scrollY>
          {hs.stream.length === 0 ? <text size={11} color={t.faint}>events arrive here as data</text> : null}
          {hs.stream.map((e, i) => (
            <text key={`e${i}`} size={11} family="mono" color={e.kind === 'note' ? t.muted : e.kind === 'warning' ? t.warning : t.fg}>
              {`${String(e.frame).padStart(4)} ${e.text}`}
            </text>
          ))}
        </box>
      </box>
    );
  };

  const legend = (win: KuiWindow) => {
    const t = win.theme();
    if (!example.keys?.length) return null;
    return (
      <box width="grow" minHeight="fit" gap={2}>
        {example.keys.map(([k, what]) => (
          <box key={`k-${k}`} dir="row" width="grow" gap={8}>
            <box width={74}><text size={11} color={t.accent} family="mono">{k}</text></box>
            <text size={11} color={t.muted}>{what}</text>
          </box>
        ))}
      </box>
    );
  };

  /** One glyph in the header's strip, its state in its colour, the chord
   *  in its tooltip. */
  const icon = (what: string, glyph: string, color: number, hint: string, t: ReturnType<KuiWindow['theme']>) => (
    <box key={`i-${what}`} width={22} height={22} center radius={4} hoverBg={t.hover} pressedBg={t.pressed}
         onClick={{ kind: '@harness', what }} tooltip={hint}>
      <text size={13} color={color}>{glyph}</text>
    </box>
  );

  const header = (hs: HarnessState, win: KuiWindow) => {
    const t = win.theme();
    const base = hs.base === null ? '◐' : hs.base === 'light' ? '☀' : '☾';
    const dockGlyph = hs.dock === 'side' ? '▐' : hs.dock === 'bottom' ? '▄' : '✕';
    const menus = hs.nativeMenus === null ? "the platform's default" : hs.nativeMenus ? 'native' : 'drawn';
    return (
      <box dir="row" width="grow" minHeight="fit" crossAlign="center" gap={6}>
        <text size={13} color={t.fg}>{example.name}</text>
        <text size={11} color={t.faint} family="mono">
          {smokeFrames ? `${hs.frames} / ${smokeFrames}` : `${hs.frames}`}
        </text>
        <box width="grow" />
        {icon('base', base, t.fg, `base: ${baseName(hs)} · Ctrl+Shift+T cycles follow the OS → light → dark`, t)}
        {icon('accent', '●', t.accent, `accent: ${accentName(hs)} · Ctrl+Shift+A cycles the OS's, kui's, four the OS might report`, t)}
        {icon('menus', '☰', hs.nativeMenus === false ? t.accent : t.fg, `menus: ${menus} · Ctrl+Shift+M toggles native and drawn`, t)}
        {icon('dock', dockGlyph, t.fg, `dock: ${hs.dock} · Ctrl+Shift+D moves it side → bottom → off`, t)}
      </box>
    );
  };

  const tabs = (hs: HarnessState, win: KuiWindow) => {
    const t = win.theme();
    return (
      <box dir="row" width="grow" minHeight="fit" gap={2} crossAlign="end">
        {TABS.map((tab) => {
          const on = tab === hs.tab;
          return (
            <box key={`tab-${tab}`} padX={10} padY={4} radiusTL={5} radiusTR={5} bg={on ? t.sunken : t.surface}
                 hoverBg={on ? t.sunken : t.hover} onClick={{ kind: '@harness', what: `tab:${tab}` }}
                 tooltip="Ctrl+Shift+N · the next tab">
              <text size={11} color={on ? t.fg : t.muted}>{tab}</text>
            </box>
          );
        })}
      </box>
    );
  };

  /** The last frame's nodes (`nodes()`), the selected one in detail, and
   *  an outline over the example where it was laid out. */
  const treePanel = (hs: HarnessState, win: KuiWindow) => {
    const t = win.theme();
    const all = win.nodes();
    // The example's nodes only: the dock's own subtree is not what anyone
    // opened this tab to see.
    const dock = all.findIndex((n) => n.label === 'dock');
    let end = all.length;
    if (dock >= 0) {
      const d = all[dock].depth;
      const rest = all.slice(dock + 1).findIndex((n) => n.depth <= d);
      end = rest < 0 ? all.length : dock + 1 + rest;
    }
    const nodes = all.filter((_, i) => !(dock >= 0 && i >= dock && i < end));
    const selected = nodes.find((n) => n.key === hs.selected) ?? null;
    const rows: [string, string][] = selected
      ? [
          ['key', selected.key],
          ['kind', selected.kind],
          ['label', selected.label ?? '—'],
          ['role', selected.role ?? '—'],
          ['rect', `${Math.round(selected.rect.x)}, ${Math.round(selected.rect.y)} · ${Math.round(selected.rect.w)}×${Math.round(selected.rect.h)}`],
          ['size', `${selected.width} × ${selected.height}${selected.float ? ' · float' : ''}`],
          ['dir', selected.dir],
          ['bg', (selected.bg & 0xff) === 0 ? 'none' : '#' + (selected.bg >>> 0).toString(16).padStart(8, '0')],
          ['flags', selected.flags.length ? selected.flags.join(' ') : '—'],
          ['text', selected.text ?? '—'],
        ]
      : [];
    return (
      <>
        <text size={11} color={t.muted}>{`${nodes.length} nodes · click one to inspect it`}</text>
        <box key="nodes" width="grow" height="grow" minHeight={80} bg={t.sunken} radius={6} pad={4} gap={0} scrollY>
          {nodes.map((n) => {
            const on = n.key === hs.selected;
            const what = n.label ?? (n.text ? `“${n.text}”` : '');
            const flags = n.flags.length ? ` · ${n.flags.join(' ')}` : '';
            return (
              <box key={`node:${n.key}`} dir="row" width="grow" height={18} padX={4} radius={3} crossAlign="center" gap={4}
                   bg={on ? t.accentSoft : t.sunken} hoverBg={on ? t.accentSoft : t.hover}
                   onClick={{ kind: '@harness', what: `node:${n.key}` }}>
                <box width={n.depth * 10} />
                <text size={11} color={t.accent} family="mono">{n.kind}</text>
                <text size={11} color={what ? t.fg : t.faint} family="mono" wrap="none">{`${what}${flags}`}</text>
              </box>
            );
          })}
        </box>
        {selected ? (
          <box width="grow" minHeight="fit" gap={2} pad={6} bg={t.raised} radius={6} borderW={1} borderColor={t.border}>
            {rows.map(([k, v]) => (
              <box key={`i-${k}`} dir="row" width="grow" gap={8}>
                <box width={40}><text size={11} color={t.muted}>{k}</text></box>
                <text size={11} color={t.fg} family="mono">{v}</text>
              </box>
            ))}
          </box>
        ) : null}
        {selected ? (
          <box key="outline" float={{ anchor: 'viewport', at: ['start', 'start'], self: ['start', 'start'], dx: selected.rect.x, dy: selected.rect.y }}
               width={Math.max(1, selected.rect.w)} height={Math.max(1, selected.rect.h)} borderW={2} borderColor={t.fg}
               bg={(t.accent & 0xffffff00) | 0x1a} role="none" />
        ) : null}
      </>
    );
  };

  const dockPanel = (hs: HarnessState, win: KuiWindow) => {
    const t = win.theme();
    const side = hs.dock === 'side';
    const body =
      hs.tab === 'facts' ? (
        <box width="grow" height="grow" gap={10} scrollY>
          <box width="grow" minHeight="fit"><latencyGraph /></box>
          {status(hs, win)}
          {legend(win)}
        </box>
      ) : hs.tab === 'events' ? (
        streamPanel(hs, win, 'grow')
      ) : (
        treePanel(hs, win)
      );
    return (
      <box key="dock" width={side ? DOCK_SIDE_W : 'grow'} height={side ? 'grow' : DOCK_BOTTOM_H} bg={t.surface}
           borderW={1} borderColor={t.border} pad={10} gap={8} role="none" clip>
        {header(hs, win)}
        {tabs(hs, win)}
        {body}
      </box>
    );
  };

  const config = {
    init: (win: KuiWindow): Wrapped<M> => {
      win_ = win;
      const hs = state();
      apply(hs);
      const app = typeof example.init === 'function' ? (example.init as (w: KuiWindow) => M)(win) : example.init;
      return { app, h: hs };
    },
    update: (m: Wrapped<M>, msg: A | CoreMsg | HarnessMsg, ev: UiEvent<A | CoreMsg | HarnessMsg>, win: KuiWindow): Wrapped<M> | undefined => {
      if (msg.kind === '@harness') {
        const what = (msg as HarnessMsg).what;
        const h = what === 'refresh' ? { ...m.h } : act(m.h, what);
        // The tree tab reads the frame *before* this one: a window redraws
        // by re-lowering the tree it was handed, so a snapshot taken at
        // the end of this frame is seen only by a view that runs after
        // it — which is what this effect asks for, once.
        const wants = h.tab === 'tree' && what !== 'refresh';
        return wants ? (withEffects({ ...m, h }, { kind: '@refresh' }) as unknown as Wrapped<M>) : { ...m, h };
      }
      const what = chord(msg as CoreMsg);
      if (what) return { ...m, h: act(m.h, what) };
      const h = { ...m.h, stream: [...m.h.stream] };
      // The harness's own sink is the root: whatever lands there — a key
      // nothing claimed, a `modifiers` change — is logged and not the
      // example's.
      if (m.h.dock !== 'off' && ev.key === ROOT_KEY) {
        push(h, 'note', `(root) ${fmtValue(msg)}`);
        return { ...m, h };
      }
      push(h, 'event', `${ev.key.slice(0, 8)} ${fmtValue(msg)}`);
      const next = example.update(m.app, msg as A | CoreMsg, ev as UiEvent<A | CoreMsg>, win);
      return { app: next === undefined ? m.app : next, h };
    },
    view: (m: Wrapped<M>, windowName: string, win: KuiWindow): KuiNode => {
      if (windowName !== 'main') return example.view(m.app, win);
      m.h.frames += 1;
      const t = win.theme();
      const title = `kui — ${example.name}`;
      const windows = example.windows?.(m.app);
      if (m.h.dock === 'off') {
        return (
          <box width="grow" height="grow" bg={t.bg} title={title} windows={windows}>
            {example.view(m.app, win)}
          </box>
        );
      }
      // Somewhere for the chords to land when the example has nothing
      // focused, from the second frame so an autofocus of the example's
      // gets the first; the root is never a Tab stop.
      const takeFocus = m.h.frames >= 2 && win.focused() === null;
      if (m.h.tab === 'tree') win.setInspect(true);
      return (
        <box dir={m.h.dock === 'bottom' ? 'column' : 'row'} width="grow" height="grow" bg={t.bg}
             title={title} windows={windows} onKey={{ kind: '@harness', what: 'sink' }} keyFocus={takeFocus}>
          <box key="example" width="grow" height="grow" clip>
            {example.view(m.app, win)}
          </box>
          {dockPanel(m.h, win)}
        </box>
      );
    },
  };

  const done = runWindowed(config as never, {
    title: `kui — ${example.name}`,
    width: w,
    height: h,
    minWidth: example.window?.minWidth,
    minHeight: example.window?.minHeight,
    chrome: example.window?.chrome,
    setup(win) {
      win_ = win;
      example.setup?.(win);
    },
    effects(effect: { kind: string }, dispatch: (msg: never) => void) {
      if (effect.kind === '@refresh') dispatch({ kind: '@harness', what: 'refresh' } as never);
    },
  });
  await done;
  process.exit(0);
}
