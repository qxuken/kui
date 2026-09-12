// The harness every Node example runs inside — the twin of examples/devtools
// (Rust) with the same flags (docs/adr/0021, decision 6). An example is one
// subject; what every example used to carry beside it lives here: the
// window title, the CLI, and the doors that open the core's devtools panel
// around the example (docs/adr/0024). The panel itself — the event stream,
// the facts, the tree with its picker and inspector — is drawn by the core
// beside the example's tree, hears its own controls and chords inside the
// core, and can dock at the side or the bottom, open a window of its own,
// or hide. Nothing of it reaches the example's `update`.
//
//   --headless                     run the example's `headless(app)` self-check
//   --dock left|right|bottom|window|off  where the panel sits
//   --light, --dark                pin the theme base
//   --accent #rrggbb               the accent, instead of the OS's
//   --motion full|reduced          what env.system.motion reads, instead of the OS's
//
// Chords (the core's): Ctrl+Shift+T cycles the base, +A the accent, +M
// toggles native menus, +D moves the panel (the header has a button per
// placement too, and a docked pane's inner edge resizes it), +C clears the stream, +N the
// next tab, +I moves the keyboard into the panel and back out, +P picks a
// node from the example.
//
// The example's root is wrapped by the core while the panel is docked
// (ADR 0024, decision 2); its keys do not move for it, and `title` and
// `windows` still belong on the root — the example's `windows` pass through
// `windows`.
import { createApp, runWindowed } from '@qxuken/kui';
import type { App, CoreMsg, KuiNode, KuiWindow, UiEvent, WindowDecl } from '@qxuken/kui';

export type Dock = 'left' | 'right' | 'bottom' | 'window' | 'off';

export type Example<M, A extends { kind: string }> = {
  /** The subject, as the README names it: the window title is `kui — name`. */
  name: string;
  init: M | ((win: KuiWindow) => M);
  update: (model: M, msg: A | CoreMsg, ev: UiEvent<A | CoreMsg>, win: KuiWindow) => M | undefined | void;
  view: (model: M, win: KuiWindow) => KuiNode;
  /** Runs before the first frame, with the window: register resources here. */
  setup?: (win: KuiWindow) => void;
  /** The windowed loop's time source, handed to `runWindowed` as its
   *  `clock` (backlog F45). Left out, the window runs on `Date.now`; an
   *  example whose ticks should run ahead of the wall — a countdown watched
   *  in seconds — gives `() => Date.now() + ahead`. The headless drive
   *  keeps the loop's own hands, so `app.advance(ms)` still moves them. */
  clock?: () => number;
  windows?: (model: M) => WindowDecl[];
  /** The key legend the panel's facts tab shows. */
  keys?: [string, string][];
  /** The example's own flags, for the usage text. */
  flags?: [string, string][];
  window?: { width?: number; height?: number; minWidth?: number; minHeight?: number; chrome?: 'native' | 'custom' | 'borderless' };
  /** Where the panel goes unless `--dock` says. */
  dock?: Dock;
  /** `false` asks for the core's drawn menus to start with. */
  nativeMenus?: boolean;
  /** The self-check `--headless` runs over a bare `createApp`: return
   *  false (or throw) on a wrong answer. */
  headless?: (app: App<M, A | CoreMsg>) => boolean | Promise<boolean>;
};

/** The dock's extents, logical px — the core's (`kui_core::devtools`), so
 *  the window the harness sizes around them fits. */
const DOCK_SIDE_W = 340;
const DOCK_BOTTOM_H = 280;
const DOCK_SIDE_MIN_H = 600;
const DOCK_BOTTOM_MIN_W = 640;

type Cli = { headless: boolean; dock?: Dock; base?: 'light' | 'dark'; accent?: string; motion?: 'full' | 'reduced' };

function usage(name: string, flags: [string, string][]): string {
  const own = flags.map(([f, d]) => `  ${f.padEnd(16)} ${d}`).join('\n');
  return (
    `usage: ${name} [--headless] [--dock left|right|bottom|window|off] [--light|--dark] [--accent #rrggbb] [--motion full|reduced]` +
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
        // `side` is the right, the name it had before there was a left.
        const dock = v === 'side' ? 'right' : v;
        if (dock !== 'left' && dock !== 'right' && dock !== 'bottom' && dock !== 'window' && dock !== 'off') throw new Error(`--dock: ${v} is not left, right, bottom, window or off`);
        cli.dock = dock; break;
      }
      case '--light': cli.base = 'light'; break;
      case '--dark': cli.base = 'dark'; break;
      case '--accent': {
        const v = value();
        if (!v || !/^#[0-9a-fA-F]{6}$/.test(v)) throw new Error(`--accent: ${v} is not #rrggbb`);
        cli.accent = v; break;
      }
      case '--motion': {
        // The pin over `env.system.motion` (F47): the two answers, since
        // `unknown` is a reading and not a pin.
        const v = value();
        if (v !== 'full' && v !== 'reduced') throw new Error(`--motion: ${v} is not full or reduced`);
        cli.motion = v; break;
      }
      case '-h': case '--help': throw new Error(usage(name, flags));
      default:
        if (flags.some(([f]) => f === flag)) break;
        if (flag.startsWith('-')) throw new Error(`unknown flag ${flag}\n\n${usage(name, flags)}`);
    }
  }
  return cli;
}

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

  const dock = cli.dock ?? example.dock ?? 'right';
  let w = example.window?.width ?? 960;
  let h = example.window?.height ?? 640;
  // The dock's extent is added, so the example's area is what it asked
  // for — and the window is given the floor the dock needs to be read.
  if (dock === 'left' || dock === 'right') { w += DOCK_SIDE_W; h = Math.max(h, DOCK_SIDE_MIN_H); }
  if (dock === 'bottom') { h += DOCK_BOTTOM_H; w = Math.max(w, DOCK_BOTTOM_MIN_W); }

  const config = {
    init: (win: KuiWindow): M =>
      typeof example.init === 'function' ? (example.init as (w: KuiWindow) => M)(win) : example.init,
    update: (m: M, msg: A | CoreMsg, ev: UiEvent<A | CoreMsg>, win: KuiWindow) =>
      example.update(m, msg, ev, win),
    view: (m: M, windowName: string, win: KuiWindow): KuiNode => {
      if (windowName !== 'main') return example.view(m, win);
      const t = win.theme();
      // `title` and `windows` are the root's: the example's tree is the
      // root's child, and the core wraps the root while the panel is docked.
      return (
        <box width="grow" height="grow" bg={t.bg} title={`kui — ${example.name}`} windows={example.windows?.(m)}>
          {example.view(m, win)}
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
    system: cli.motion ? { motion: cli.motion } : undefined,
    clock: example.clock,
    setup(win) {
      // The doors, before the first frame: the panel, where it sits, what
      // the command line pinned, and the example's legend.
      win.setDevtools(true);
      win.setDevtoolsDock(dock);
      win.setDevtoolsTheme(cli.base ?? null, cli.accent ?? null);
      if (example.keys?.length) win.setDevtoolsLegend(example.keys);
      if (example.nativeMenus !== undefined) {
        win.setNativeMenus(example.nativeMenus);
        win.setNativeMenuBar(example.nativeMenus);
      }
      example.setup?.(win);
    },
  });
  await done;
  process.exit(0);
}
