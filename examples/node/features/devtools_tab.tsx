// A tab of the app's own in the core's devtools panel (ADR 0032,
// docs/adr/0032-a-devtools-tab-mounts-a-slot.md), in the shape a
// tree-sitter inspector has — the twin of `rust/features/devtools_tab.rs`.
// The page is a "source file" whose tokens are coloured by **highlight
// group** (`@keyword`, `@function`, …) from a small syntax tree written by
// hand; `<devtoolsTab>` with a **function child** adds an Inspector tab
// beside facts, events and tree that lists the tree. Hover goes both ways:
// a row of the tab lights the node's tokens in the source, a token in the
// source lights its path in the tab — both are the app's own nodes, so both
// are the app's own `onHover`. A row's click selects the token in the
// panel's tree tab (`setDevtoolsSelected`), and the tab raises the panel's
// picker itself (`setDevtoolsPick`): the pick lands in `devtoolsSelected()`,
// read back as the node it names, with the tab still up.
//
// The function is called only while the tab is on show — `frame` reads
// which tab that is once before encoding — so the counter the page prints
// stays at 0 until you walk to it (Ctrl+Shift+N, or click it in the strip).
//
//   npm run devtools_tab                              a real window
//   node dist/features/devtools_tab.mjs --headless    the same, no window
import type { App, CoreMsg, KuiWindow, Theme, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

/** One node of the syntax tree: its kind, the highlight group a token
 *  carries (none for an inner node), the line it starts on, its parent,
 *  and — for a token — the text. */
type Node = { kind: string; group?: string; line: number; parent: number | null; text?: string };

const tok = (kind: string, group: string, line: number, parent: number, text: string): Node => ({ kind, group, line, parent, text });
const inner = (kind: string, line: number, parent: number): Node => ({ kind, line, parent });

/** The tree, in preorder, over the six lines — what a parser would have
 *  said about them. Tokens appear in source order within a line. */
const TREE: Node[] = [
  { kind: 'source_file', line: 0, parent: null }, // 0
  inner('function_item', 0, 0), // 1
  tok('fn', 'keyword', 0, 1, 'fn'), // 2
  tok('identifier', 'function', 0, 1, 'main'), // 3
  inner('parameters', 0, 1), // 4
  tok('(', 'punctuation', 0, 4, '('), // 5
  tok(')', 'punctuation', 0, 4, ')'), // 6
  inner('block', 0, 1), // 7
  tok('{', 'punctuation', 0, 7, '{'), // 8
  inner('let_declaration', 1, 7), // 9
  tok('let', 'keyword', 1, 9, 'let'), // 10
  tok('identifier', 'variable', 1, 9, 'tree'), // 11
  tok('=', 'operator', 1, 9, '='), // 12
  inner('call_expression', 1, 9), // 13
  tok('identifier', 'function', 1, 13, 'parse'), // 14
  inner('arguments', 1, 13), // 15
  tok('(', 'punctuation', 1, 15, '('), // 16
  tok('identifier', 'variable', 1, 15, 'source'), // 17
  tok(')', 'punctuation', 1, 15, ')'), // 18
  tok(';', 'punctuation', 1, 9, ';'), // 19
  inner('for_expression', 2, 7), // 20
  tok('for', 'keyword', 2, 20, 'for'), // 21
  tok('identifier', 'variable', 2, 20, 'node'), // 22
  tok('in', 'keyword', 2, 20, 'in'), // 23
  inner('call_expression', 2, 20), // 24
  inner('field_expression', 2, 24), // 25
  tok('identifier', 'variable', 2, 25, 'tree'), // 26
  tok('.', 'punctuation', 2, 25, '.'), // 27
  tok('field_identifier', 'function', 2, 25, 'walk'), // 28
  inner('arguments', 2, 24), // 29
  tok('(', 'punctuation', 2, 29, '('), // 30
  tok(')', 'punctuation', 2, 29, ')'), // 31
  inner('block', 2, 20), // 32
  tok('{', 'punctuation', 2, 32, '{'), // 33
  inner('expression_statement', 3, 32), // 34
  inner('macro_invocation', 3, 34), // 35
  tok('identifier', 'macro', 3, 35, 'println'), // 36
  tok('!', 'macro', 3, 35, '!'), // 37
  inner('token_tree', 3, 35), // 38
  tok('(', 'punctuation', 3, 38, '('), // 39
  tok('string_literal', 'string', 3, 38, '"{node:?}"'), // 40
  tok(')', 'punctuation', 3, 38, ')'), // 41
  tok(';', 'punctuation', 3, 34, ';'), // 42
  tok('}', 'punctuation', 4, 32, '}'), // 43
  tok('}', 'punctuation', 5, 7, '}'), // 44
];
/** How far each line is indented, in spaces. */
const INDENT = [0, 4, 4, 8, 4, 0];

/** The colour a highlight group paints with, from the theme's roles. */
function groupColor(t: Theme, group: string | undefined): number {
  switch (group) {
    case 'keyword': return t.accent;
    case 'function': return t.success;
    case 'string': return t.warning;
    case 'macro': return t.danger;
    case 'punctuation': case 'operator': return t.muted;
    default: return t.fg;
  }
}
/** Whether `node` is `ancestor` or under it. */
function under(node: number, ancestor: number): boolean {
  for (let n: number | null = node; n != null; n = TREE[n].parent) if (n === ancestor) return true;
  return false;
}
function depth(i: number): number {
  let d = 0;
  for (let n = TREE[i].parent; n != null; n = TREE[n].parent) d++;
  return d;
}
/** The node's path from the root, `source_file › function_item › …`. */
function path(i: number): string {
  const names: string[] = [];
  for (let n: number | null = i; n != null; n = TREE[n].parent) names.unshift(TREE[n].kind);
  return names.join(' › ');
}

type Model = {
  /** The tab row under the pointer: its node's tokens light in the source. */
  hoverRow: number | null;
  /** The source token under the pointer: its path lights in the tab. */
  hoverTok: number | null;
};
// What the tab's function child keeps between runs — beside the model, not
// in it: the function runs inside the encode, where the model is what the
// view was handed and is not the place to write. `built` counts the runs
// (the laziness, on screen, a frame late since the page's text is encoded
// before the tab); `revealed` is the token the tab last scrolled to, so a
// hover reveals its row once and the list stays where the user put it.
let built = 0;
let revealed: number | null = null;
type AppMsg = { kind: 'tok'; id: number } | { kind: 'row'; id: number } | { kind: 'inspect-pick' };
type Msg = AppMsg | CoreMsg;

const init: Model = { hoverRow: null, hoverTok: null };

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  switch (msg.kind) {
    case 'hover': {
      // The tag is the node's own message; enter names it, leave clears it.
      const tag = msg.tag;
      if (!tag || !('kind' in tag) || (tag.kind !== 'tok' && tag.kind !== 'row')) return undefined;
      const field = tag.kind === 'tok' ? 'hoverTok' : 'hoverRow';
      if (msg.phase === 'enter') return { ...model, [field]: tag.id };
      return model[field] === tag.id ? { ...model, [field]: null } : undefined;
    }
    case 'tok':
    case 'row': {
      // A click on a row or a token: select it in the panel's tree. An
      // inner node has no token to select, and clears nothing.
      const key = win.keyOf(`tok:${msg.id}`);
      if (key != null) win.setDevtoolsSelected(key);
      return undefined;
    }
    case 'inspect-pick':
      win.setDevtoolsPick(true);
      return undefined;
    default:
      return undefined;
  }
}

const view = (model: Model, win: KuiWindow) => {
  const t = win.theme();
  // What the panel holds, in the tree's terms.
  const nodeOf = (key: string | null): number | null => {
    if (key == null) return null;
    const i = TREE.findIndex((n, j) => n.text != null && win.keyOf(`tok:${j}`) === key);
    return i < 0 ? null : i;
  };
  const selected = nodeOf(win.devtoolsSelected());
  const picked = nodeOf(win.devtoolsPicked());
  // A token lights up when the tab row over it is the token itself or an
  // ancestor, or when the panel selected or is picking it.
  const lit = (i: number) => (model.hoverRow != null && under(i, model.hoverRow)) || selected === i || picked === i;
  const shown = model.hoverTok ?? selected;
  return (
    <box width="grow" height="grow" pad={16} gap={4} bg={t.bg}>
      <text size={12} color={t.muted}>the source, coloured by highlight group — hover a token to see its node in the Inspector</text>
      {INDENT.map((indent, line) => {
        let first = true;
        return (
          <box key={`line:${line}`} dir="row" width="grow" crossAlign="center">
            <box width={8 * indent} />
            {TREE.flatMap((n, i) => {
              if (n.text == null || n.line !== line) return [];
              // A space before a word, or before `=` and `{`: what a
              // formatter leaves.
              const word = /^[A-Za-z0-9"]/.test(n.text);
              const gap = !first && (word || n.text === '=' || n.text === '{') ? [<box key={`gap:${i}`} width={7} />] : [];
              first = false;
              return [
                ...gap,
                <box
                  key={`tok:${i}`}
                  label={n.kind}
                  dir="row"
                  padX={1}
                  padY={2}
                  radius={3}
                  bg={lit(i) ? t.accentSoft : undefined}
                  hoverBg={t.hover}
                  onHover={{ kind: 'tok', id: i }}
                  onClick={{ kind: 'tok', id: i }}
                >
                  <text size={14} family="mono" color={groupColor(t, n.group)}>{n.text}</text>
                </box>,
              ];
            })}
          </box>
        );
      })}
      <text size={12} color={t.muted}>{shown == null ? 'hover a token, or pick one from the Inspector' : `${path(shown)} · @${TREE[shown].group ?? 'none'}`}</text>
      <text size={11} color={t.faint}>{`Inspector built ${built} time(s) — once a view, only while its tab is on show`}</text>
      <devtoolsTab name="inspector" label="Inspector">
        {() => {
          built++;
          // A token hovered in the source scrolls the tab to its row, once
          // per token.
          if (model.hoverTok != null && revealed !== model.hoverTok) {
            const k = win.keyOf(`node:${model.hoverTok}`);
            if (k) win.reveal(k);
            revealed = model.hoverTok;
          }
          const picking = win.devtoolsPicking();
          return (
            <box width="grow" height="grow" gap={4}>
              <box dir="row" gap={6} crossAlign="center">
                <button onClick={{ kind: 'inspect-pick' }}>{picking ? 'picking… (Escape leaves)' : 'pick a node'}</button>
                <text size={11} color={t.muted}>
                  {picked != null ? `picking ${TREE[picked].kind}` : selected != null ? `selected ${TREE[selected].kind}` : 'nothing selected in the panel'}
                </text>
              </box>
              <text size={11} color={t.muted}>the syntax tree — hover a row to light its tokens, click to select it in the panel's tree</text>
              <box width="grow" height="grow" scrollY>
                {TREE.map((n, i) => {
                  // A row is lit when the source token under the pointer is
                  // it or under it, or when the panel selected or is picking it.
                  const on = (model.hoverTok != null && under(model.hoverTok, i)) || selected === i || picked === i;
                  return (
                    <box
                      key={`node:${i}`}
                      label={n.kind}
                      dir="row"
                      width="grow"
                      padX={6}
                      padY={2}
                      gap={6}
                      radius={3}
                      crossAlign="center"
                      bg={on ? t.accentSoft : undefined}
                      hoverBg={t.hover}
                      onHover={{ kind: 'row', id: i }}
                      onClick={{ kind: 'row', id: i }}
                    >
                      <box width={10 * depth(i)} />
                      <text size={12} family="mono" wrap="none" color={n.text != null ? t.fg : t.muted}>{n.kind}</text>
                      {n.group != null ? <text size={11} family="mono" wrap="none" color={groupColor(t, n.group)}>{`@${n.group}`}</text> : null}
                      {n.text != null ? <text size={11} family="mono" wrap="none" color={t.faint}>{n.text}</text> : null}
                      <box width="grow" />
                      <text size={10} color={t.faint}>{String(n.line + 1)}</text>
                    </box>
                  );
                })}
              </box>
            </box>
          );
        }}
      </devtoolsTab>
    </box>
  );
};

await run<Model, AppMsg>({
  name: 'devtools_tab',
  init,
  update,
  view,
  keys: [
    ['Ctrl+Shift+N', 'the next tab — facts, events, tree, Inspector'],
    ['Ctrl+Shift+P', 'pick a token; the Inspector reads it back as its node'],
  ],
  window: { width: 760, height: 480 },
  // The tab is declared and not built while another is up; on show it is
  // built once a frame, over the panel's body; hovering a row lights the
  // node's tokens and hovering a token lights its path; a row's click
  // selects the token in the panel's tree; the tab's picker lands a pick
  // in `selected` with the tab still up.
  headless: (app: App<Model, Msg>) => {
    const ctx = app.ctx;
    ctx.setDevtools(true);
    ctx.setDevtoolsDock('right');
    ctx.setInspect(true);
    app.render();
    app.render();
    const check = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    const node = (label: string) => ctx.nodes().find((n) => n.label === label);
    const litOf = (prefix: string) =>
      TREE.map((_, i) => i).filter((i) => {
        const k = ctx.keyOf(`${prefix}:${i}`);
        // `bg` reads back as the packed colour; 0 is none.
        return k != null && ctx.nodes().some((n) => n.key === k && n.bg !== 0);
      });
    check(built === 0, 'the tab is declared, not built, while events is up');
    check(!!node('kui-devtools/tab-custom:inspector'), 'and the strip lists it');
    const chord = () => ctx.keyDown('n', { ctrl: true, shift: true });
    chord(); // tree
    chord(); // Inspector
    app.render();
    check(built === 1, 'on show, the function child ran once');
    app.render();
    check(built === 2, 'and once a frame');
    const byKey = (label: string) => ctx.nodes().find((n) => n.key === ctx.keyOf(label));
    const body = node('kui-devtools/tab/inspector')!;
    const row = byKey('node:9')!;
    check(!!body && !!row && row.rect.x >= body.rect.x && row.rect.x + row.rect.w <= body.rect.x + body.rect.w, "the tab's content is laid out over the panel's body");
    // Hover the `let_declaration` row: its tokens light up on line 2.
    ctx.cursor(row.rect.x + 4, row.rect.y + row.rect.h / 2);
    app.settle();
    check(app.model.hoverRow === 9, 'hovering a row names its node');
    check(JSON.stringify(litOf('tok')) === JSON.stringify([10, 11, 12, 14, 16, 17, 18, 19]), "and the node's tokens are lit in the source, no others");
    // Hover a token in the source: its path lights up in the tab.
    const walk = byKey('tok:28')!;
    ctx.cursor(walk.rect.x + 2, walk.rect.y + walk.rect.h / 2);
    app.settle();
    check(app.model.hoverTok === 28 && app.model.hoverRow === null, "hovering a token names it, and the row's hover ended");
    check(JSON.stringify(litOf('node')) === JSON.stringify([0, 1, 7, 20, 24, 25, 28]), 'and its path is lit in the tab, root to leaf');
    // The hover scrolled the tab to the token's row; a click on it selects
    // the token in the panel's tree.
    app.render();
    const walkRow = byKey('node:28')!;
    ctx.cursor(walkRow.rect.x + 4, walkRow.rect.y + walkRow.rect.h / 2);
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    app.settle();
    check(ctx.devtoolsSelected() === ctx.keyOf('tok:28'), "a row's click selected its token in the panel's tree");
    // The picker, raised from the tab: the tab stays, the pick lands.
    const pick = node('pick a node')!;
    ctx.cursor(pick.rect.x + 2, pick.rect.y + 2);
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    app.settle();
    check(ctx.devtoolsPicking(), 'the tab raised the picker');
    check(ctx.devtoolsShownTab() === 'inspector', 'and stayed up while picking');
    const str = byKey('tok:40')!;
    ctx.cursor(str.rect.x + 4, str.rect.y + str.rect.h / 2);
    app.render();
    check(ctx.devtoolsPicked() === ctx.keyOf('tok:40'), 'the token is under the picker');
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    app.settle();
    check(!ctx.devtoolsPicking() && ctx.devtoolsSelected() === ctx.keyOf('tok:40'), 'the press picked the token into `selected`');
    check(ctx.devtoolsShownTab() === 'inspector', 'and the tab is still the one on show');
    chord(); // facts
    const runs = built;
    app.render();
    check(built === runs, 'another tab up: the function child rests');
    return true;
  },
});
