// A tab of the app's own in the core's devtools panel (ADR 0032,
// docs/adr/0032-a-devtools-tab-mounts-a-slot.md). The page is a "source
// file" of lines; `<devtoolsTab>` with a **function child** adds an
// Inspector tab beside facts, events and tree. The function is called only
// while the tab is on show — `frame` reads which tab that is once before
// encoding — so the counter the page prints stays at 0 until you walk to it
// (Ctrl+Shift+N, or click it in the strip). What it returns is the app's:
// its keys, its clicks reaching `update`, laid out over the panel's tab
// body. The tab reads the panel's facts (`devtoolsSelected` and friends),
// selects a line in the panel's tree (`setDevtoolsSelected`) and raises the
// panel's picker itself (`setDevtoolsPick`), the way a tree-sitter
// inspector would.
//
//   npm run devtools_tab                              a real window
//   node dist/features/devtools_tab.mjs --headless    the same, no window
import type { App, CoreMsg, KuiWindow, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

const LINES = ['fn main() {', '    let tree = parse(source);', '    for node in tree.walk() {', '        println!("{node:?}");', '    }', '}'];

type Model = { built: number; clicked: number | null };
type AppMsg = { kind: 'line'; i: number } | { kind: 'reveal'; i: number } | { kind: 'inspect-pick' };
type Msg = AppMsg | CoreMsg;

const init: Model = { built: 0, clicked: null };

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  switch (msg.kind) {
    case 'line':
      return { ...model, clicked: msg.i };
    case 'reveal':
      // The door is the surface's: select the page's line in the panel's tree.
      win.setDevtoolsSelected(win.keyOf(`line:${msg.i}`));
      return undefined;
    case 'inspect-pick':
      win.setDevtoolsPick(true);
      return undefined;
    default:
      return undefined;
  }
}

const view = (model: Model, win: KuiWindow) => {
  const t = win.theme();
  // A key's label is not a reading a window has; the page's own lines
  // are found by their key, and anything else prints as its key.
  const name = (k: string | null) =>
    k == null ? '—' : (LINES.map((_, i) => `line:${i}`).find((l) => win.keyOf(l) === k) ?? k.slice(-8));
  return (
    <box width="grow" height="grow" pad={16} gap={6} bg={t.bg}>
      <text size={12} color={t.muted}>the source — each line is a node the Inspector tab can select in the panel's tree</text>
      {LINES.map((line, i) => (
        <box
          key={`line:${i}`}
          label={`line ${i}`}
          width="grow"
          padX={8}
          padY={3}
          radius={4}
          bg={model.clicked === i ? t.accentSoft : t.surface}
          hoverBg={t.hover}
          onClick={{ kind: 'line', i }}
        >
          <text size={13} family="mono" color={t.fg}>{line}</text>
        </box>
      ))}
      <text size={12} color={t.muted}>{`Inspector built ${model.built} time(s) — only while its tab is on show`}</text>
      <devtoolsTab name="inspector" label="Inspector">
        {() => {
          // Counted on the model's behalf: the function runs inside the
          // encode, so it reads the model and mutates nothing but this.
          model.built++;
          return (
            <box width="grow" height="grow" gap={6} scrollY>
              <text size={11} color={t.muted}>the panel's facts, read through the doors</text>
              {(
                [
                  ['selected', win.devtoolsSelected()],
                  ['hovered', win.devtoolsHovered()],
                  ['picked', win.devtoolsPicked()],
                ] as [string, string | null][]
              ).map(([k, v]) => (
                <box key={k} gap={8}>
                  <box width={64}><text size={12} color={t.muted}>{k}</text></box>
                  <text size={12} family="mono" color={t.fg}>{name(v)}</text>
                </box>
              ))}
              <button onClick={{ kind: 'inspect-pick' }}>{win.devtoolsPicking() ? 'picking… (Escape leaves)' : 'pick a node'}</button>
              <text size={11} color={t.muted}>select a line in the panel's tree from here</text>
              {LINES.map((line, i) => (
                <box key={`sel:${i}`} gap={2}>
                  <button onClick={{ kind: 'reveal', i }}>{`select line ${i}`}</button>
                  <text size={11} family="mono" color={t.muted}>{line.trim()}</text>
                </box>
              ))}
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
    ['Ctrl+Shift+P', 'pick a node; the Inspector reads it as `picked`'],
  ],
  window: { width: 620, height: 360 },
  // The tab is declared and not built while another is up; on show it is
  // built once a frame, over the panel's body; its button selects the
  // page's line in the panel's tree, and its picker lands a pick in
  // `selected` with the tab still up.
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
    check(app.model.built === 0, 'the tab is declared, not built, while events is up');
    check(ctx.nodes().some((n) => n.label === 'kui-devtools/tab-custom:inspector'), 'and the strip lists it');
    const chord = () => ctx.keyDown('n', { ctrl: true, shift: true });
    chord(); // tree
    chord(); // Inspector
    app.render();
    check(app.model.built === 1, 'on show, the function child ran once');
    app.render();
    check(app.model.built === 2, 'and once a frame');
    const body = ctx.nodes().find((n) => n.label === 'kui-devtools/tab/inspector')!;
    // The first of the select buttons: the rest scroll below the 360 px
    // window, and a press below the fold hits nothing.
    const button = ctx.nodes().find((n) => n.label === 'select line 0')!;
    check(
      !!body && !!button && button.rect.x >= body.rect.x && button.rect.x + button.rect.w <= body.rect.x + body.rect.w,
      "the tab's content is laid out over the panel's body",
    );
    ctx.cursor(button.rect.x + 2, button.rect.y + 2);
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    app.settle();
    check(ctx.devtoolsSelected() === ctx.keyOf('line:0'), "the tab's button selected the page's line in the panel's tree");
    const pick = ctx.nodes().find((n) => n.label === 'pick a node')!;
    ctx.cursor(pick.rect.x + 2, pick.rect.y + 2);
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    app.settle();
    check(ctx.devtoolsPicking(), 'the tab raised the picker');
    check(ctx.devtoolsShownTab() === 'inspector', 'and stayed up while picking');
    const line2 = ctx.nodes().find((n) => n.label === 'line:2')!;
    ctx.cursor(line2.rect.x + 4, line2.rect.y + line2.rect.h / 2);
    app.render();
    check(ctx.devtoolsPicked() === ctx.keyOf('line:2'), 'the line is under the picker');
    ctx.mouse(true, 1);
    ctx.mouse(false, 1);
    app.settle();
    check(!ctx.devtoolsPicking() && ctx.devtoolsSelected() === ctx.keyOf('line:2'), 'the press picked the line into `selected`');
    check(ctx.devtoolsShownTab() === 'inspector', 'and the tab is still the one on show');
    chord(); // facts
    const runs = app.model.built;
    app.render();
    check(app.model.built === runs, 'another tab up: the function child rests');
    return true;
  },
});
