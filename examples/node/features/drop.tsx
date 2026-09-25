// A drop zone from Node — the twin of `rust/features/drop.rs`
// (docs/adr/0031-a-drop-zone-is-a-row-and-the-files-are-an-event.md).
// Drag files in from the Finder:
//
//   * `onDrop` makes the box a zone: the files over it arrive as four
//     phases of one `drop` message — `enter`, `move`, `leave`, `drop` —
//     with their paths and the pointer's position;
//   * `dropBg` lights the zone while they hover, swapped by the core the
//     way `hoverBg` is, with no state in the model;
//   * the button inside the zone is the zone's: files over it land here;
//   * the banner the view shows on `enter` is a float over the zone that
//     is no zone, and the files look past it — no `leave` for it;
//   * the box below takes nothing: a release there slides the icon home;
//   * "Open…" asks for the platform's Open dialog instead (backlog C51):
//     `win.requestFiles` from `update`, answered by one `files` message
//     whose `paths` are what a drop's are, so the same list takes both.
//
//   npm run drop                              a real window
//   node dist/features/drop.mjs --headless    every path, no window
import type { App, CoreMsg, Ctx, KuiWindow, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

type Model = { landed: string[]; hovering: string[]; at: [number, number] | null; events: number; cleared: number };
type AppMsg = { kind: 'zone' } | { kind: 'clear' } | { kind: 'open' };
type Msg = AppMsg | CoreMsg;

const init: Model = { landed: [], hovering: [], at: null, events: 0, cleared: 0 };

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  switch (msg.kind) {
    case 'clear':
      return { ...model, landed: [], cleared: model.cleared + 1 };
    case 'open':
      // One dialog at a time: a click while it is up asks for nothing more.
      win.requestFiles({ mode: 'open', multiple: true, title: 'Add files', tag: { kind: 'open' } });
      return undefined;
    // The dialog's answer: the same paths a drop carries, none when it was
    // cancelled.
    case 'files':
      return { ...model, landed: [...model.landed, ...msg.paths] };
    case 'drop': {
      const at: [number, number] | null = msg.x !== undefined && msg.y !== undefined ? [msg.x, msg.y] : null;
      const events = model.events + 1;
      switch (msg.phase) {
        case 'enter': return { ...model, events, hovering: msg.paths, at };
        case 'move': return { ...model, events, at };
        case 'leave': return { ...model, events, hovering: [], at: null };
        case 'drop': return { ...model, events, hovering: [], at: null, landed: [...model.landed, ...msg.paths] };
      }
    }
  }
  return undefined;
}

type Surface = Pick<Ctx | KuiWindow, 'theme'>;

function view(model: Model, win: Surface) {
  const t = win.theme();
  const caption = (s: string) => <text size={12} color={t.muted}>{s}</text>;
  const heading = model.hovering.length ? `${model.hovering.length} file(s) over the zone` : 'drop files here';
  // The accent at a third: a colour is 0xRRGGBBAA, so the alpha is the low byte.
  const lit = (t.accent >>> 8) * 256 + 0x59;
  return (
    <box width="grow" height="grow" bg={t.bg} pad={24} gap={14} crossAlign="start">
      {caption('onDrop + dropBg · drag files from the Finder onto the zone')}
      <box key="zone" label="zone" width={400} height={180} pad={14} gap={8} radius={10} bg={t.surface}
           dropBg={lit} borderW={1} borderColor={t.border} onDrop={{ kind: 'zone' }}>
        <text size={14} color={t.fg}>{heading}</text>
        {model.at && <text size={12} color={t.faint}>{`pointer at ${model.at[0].toFixed(0)}, ${model.at[1].toFixed(0)}`}</text>}
        {/* Inside the zone: a button, and files over it are the zone's. */}
        <box key="clear" label="Clear" padX={12} padY={6} radius={6} bg={t.raised} hoverBg={t.hover} onClick={{ kind: 'clear' }}>
          <text size={12} color={t.fg}>Clear the list</text>
        </box>
        <box key="open" label="Open…" padX={12} padY={6} radius={6} bg={t.raised} hoverBg={t.hover} onClick={{ kind: 'open' }}>
          <text size={12} color={t.fg}>Open…</text>
        </box>
        {/* What an app shows in answer to `enter`: a banner floated over the
            zone. It takes no files, so the files look past it (decision 2). */}
        {model.hovering.length > 0 && (
          <box key="banner" label="banner" float={{ anchor: 'parent', at: ['center', 'end'], self: ['center', 'end'], dx: 0, dy: -8 }}
               padX={14} padY={8} radius={8} bg={t.accent} hoverable>
            <text size={12} color={t.onAccent}>release to add</text>
          </box>
        )}
      </box>
      <box key="nowhere" label="nowhere" width={400} pad={12} radius={10} bg={t.surface} borderW={1} borderColor={t.border} hoverable>
        {caption('not a zone · the cursor says no, a release slides home')}
      </box>
      <text size={12} color={t.fg}>{`landed: ${model.landed.length ? model.landed.join(', ') : 'nothing yet'}`}</text>
      <text size={12} color={t.faint}>{`${model.events} drop events · cleared ${model.cleared}×`}</text>
    </box>
  );
}

await run<Model, AppMsg>({
  name: 'drop',
  init,
  update,
  view,
  window: { width: 460, height: 440 },
  // Files in from the OS, headlessly: over the zone, over its button,
  // over the banner the view showed, off every zone, and landed.
  headless: (app: App<Model, Msg>) => {
    const ok = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    const ctx = app.ctx;
    ctx.setInspect(true);
    app.render();
    const rect = (label: string) => {
      const n = ctx.nodes().find((n) => n.label === label);
      if (!n) throw new Error(`no node labelled ${label}`);
      return n.rect;
    };
    const zone = rect('zone');
    const paths = ['/tmp/a.txt', '/tmp/b.png'];
    const inside: [number, number] = [zone.x + 30, zone.y + 30];
    ctx.dragFiles(paths, ...inside);
    app.settle();
    ok(app.model.hovering.length === 2 && ctx.dropTarget() !== null, 'files over the zone: `enter`, and the zone is the target');
    ok(ctx.isDropTarget('zone'), 'the zone reads lit');
    app.render();
    const button = rect('clear');
    ctx.dragFiles(paths, button.x + 4, button.y + 4);
    app.settle();
    ok(app.model.hovering.length === 2 && app.model.events === 2, 'over the button inside the zone: a `move` on the zone, not a leave');
    app.render();
    const banner = rect('banner');
    ctx.dragFiles(paths, banner.x + 4, banner.y + 4);
    app.settle();
    ok(app.model.hovering.length === 2 && ctx.isDropTarget('zone'), 'over the banner: the files look past it to the zone');
    const nowhere = rect('nowhere');
    ctx.dragFiles(paths, nowhere.x + 4, nowhere.y + 4);
    app.settle();
    ok(app.model.hovering.length === 0 && ctx.dropTarget() === null, 'over the box that is no zone: `leave`, and no target');
    ctx.dragFiles(paths, ...inside);
    app.settle();
    const before = app.model.events;
    ctx.dropFiles(paths, ...inside);
    app.settle();
    ok(app.model.landed.length === 2 && app.model.hovering.length === 0 && app.model.events === before + 1,
      'released over the zone: `drop` with both paths, and no `leave` after it');
    app.render();
    ok(!ctx.isDropTarget('zone'), 'and nothing is lit');
    ctx.dragFiles(paths, ...inside);
    ctx.dragCancel();
    app.settle();
    ok(app.model.hovering.length === 0 && ctx.dropTarget() === null, 'out of the window: the lit zone\'s `leave`');

    // "Open…": update asks, the host (this drive) takes the ask and answers
    // it, and the answer lands in the same list.
    app.render();
    const open = rect('open');
    app.click(open.x + 4, open.y + 4);
    const asks = ctx.takeFileRequests();
    ok(asks.length === 1 && asks[0].multiple === true, 'Open… asks the host for one multiple-file dialog');
    app.click(open.x + 4, open.y + 4);
    ok(ctx.takeFileRequests().length === 0, 'a second click while it is up asks for nothing more');
    ctx.answerFiles(['/tmp/c.md']);
    app.settle();
    ok(app.model.landed.at(-1) === '/tmp/c.md' && !ctx.awaitingFiles(), 'the answer lands like a drop, and the ask is spent');
    return true;
  },
});
