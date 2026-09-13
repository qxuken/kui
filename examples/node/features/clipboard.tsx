// The clipboard from Node: every way onto it, and the two ways a paste
// lands — the twin of `rust/features/clipboard.rs`, from the side a Node
// host sees. The clipboard is the host's: a copy is the core, or the app,
// working out *what* and handing the host the text; a paste is the host
// reading the clipboard and handing the text back as input. In a window
// the runner underneath does both halves; headless, `takeMenuActions()`
// is the queue and `commit()` the answer.
//
//   * **The field.** An `<edit>`: Cmd/Ctrl-C/X/V are the runner's own, no
//     queue; its context menu's Cut / Copy / Paste queue.
//   * **The card.** A `selectable` box: the same chord copies its runs,
//     the bold beside as HTML; its menu's Copy queues the same.
//   * **The log.** A selectable `virtualColumn`: a copy that reaches rows
//     no frame built is a `selectionrange` message, and
//     `win.answerSelectionRange` is what reaches the clipboard.
//   * **The register.** An `onKey` sink: `y` → `win.setClipboard`, `p` →
//     `win.requestPaste`, and the paste comes back as the `text` message
//     an IME's commit arrives on. `update` is handed the surface, so both
//     are one call — no effect needed, though `withEffects` and an
//     `effects` handler calling `surface.setClipboard` is the same thing
//     as `surface.play` for a sound.
//
//   npm run clipboard                              a real window
//   node dist/features/clipboard.mjs --headless    every path, no window
import { virtualColumn } from '@qxuken/kui';
import type { App, CoreMsg, Ctx, KuiWindow, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

const ROWS = 2000;
const ROW_H = 22;

type Model = { lines: string[]; cursor: number; awaitingPaste: boolean; sent: string; received: string };
type AppMsg = { kind: 'register' };
type Msg = AppMsg | CoreMsg;

const init: Model = {
  lines: [
    'j / k move, y copies the line, p pastes a new one',
    'the sink hears Cmd-C raw and binds nothing to it',
    'so the clipboard is two calls on the surface',
  ],
  cursor: 0,
  awaitingPaste: false,
  sent: 'nothing yet',
  received: 'nothing yet',
};

/** The log's row `i`: the app's own data, what a `selectionrange` ask is
 *  answered from. */
const row = (i: number) => `${String(i).padStart(4, ' ')}  log line ${i}`;

/** The text of the rows an ask named, cut to the bytes at each end. */
function rangeText(from: { index: number; byte: number }, to: { index: number; byte: number }): string {
  const last = Math.min(to.index, ROWS - 1);
  const rows: string[] = [];
  for (let i = from.index; i <= last; i++) rows.push(row(i));
  let text = rows.join('\n');
  if (to.index <= ROWS - 1) text = text.slice(0, text.length - row(last).length + Math.min(to.byte, row(last).length));
  return text.slice(Math.min(from.byte, text.length));
}

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  switch (msg.kind) {
    case 'key': {
      switch (msg.code) {
        case 'j': case 'down':
          return { ...model, cursor: Math.min(model.cursor + 1, model.lines.length - 1) };
        case 'k': case 'up':
          return { ...model, cursor: Math.max(model.cursor - 1, 0) };
        case 'y': {
          // The sink's own Cmd-C: the app chose the text, the host puts
          // it on the clipboard.
          const line = model.lines[model.cursor];
          win.setClipboard(line);
          return { ...model, sent: `the register's line: ${JSON.stringify(line)}` };
        }
        case 'p':
          // And its own Cmd-V: the host reads the clipboard and the text
          // comes back as a `text` message, below.
          win.requestPaste();
          return { ...model, awaitingPaste: true };
      }
      return undefined;
    }
    case 'text':
      if (!model.awaitingPaste) return undefined;
      return {
        ...model,
        awaitingPaste: false,
        lines: [...model.lines, msg.text],
        cursor: model.lines.length,
        received: JSON.stringify(msg.text),
      };
    case 'selectionrange': {
      // The log's copy reached rows no frame built: the rows are the
      // app's, so the app answers, and the answer is what is copied.
      const text = rangeText(msg.from, msg.to);
      win.answerSelectionRange(text);
      return { ...model, sent: `the log's rows: ${text.length} bytes` };
    }
  }
  return undefined;
}

type Surface = Pick<Ctx | KuiWindow, 'scrollGeometry' | 'env' | 'theme'>;

function view(model: Model, win: Surface) {
  const t = win.theme();
  const card = { width: 'grow' as const, maxWidth: 600, pad: 14, gap: 8, bg: t.surface, radius: 10, borderW: 1, borderColor: t.border };
  const caption = (s: string) => <text size={12} color={t.muted}>{s}</text>;
  return (
    <box width="grow" height="grow" bg={t.bg} pad={20} gap={10} crossAlign="center" scrollY>
      <box {...card}>
        {caption('an editor: ⌘C ⌘X ⌘V are the runner\'s, its menu queues')}
        <edit key="note" label="note" initial="Select some of this and copy it; paste lands as typing."
              width="grow" pad={8} radius={6} bg={t.bg} borderW={1} borderColor={t.border} />
      </box>
      <box key="article" {...card} selectable>
        {caption('a selectable scope: ⌘C copies the runs, the bold as HTML beside')}
        <text size={14} lineHeight={22} color={t.fg}>
          Drag across this and the <span bold>bold</span> comes along as a second flavour the host may offer.
        </text>
      </box>
      <box {...card} pad={8}>
        {caption('a virtual list: select, scroll away, ⌘C — the app answers')}
        {virtualColumn(
          win,
          { key: 'log', rows: ROWS, rowH: ROW_H, width: 'grow', height: 4 * ROW_H, bg: t.sunken, radius: 6, selectable: true, role: 'list', label: 'log' },
          (i) => (
            <box width="grow" height="grow" padX={8} dir="row" crossAlign="center">
              <text size={12} family="mono" color={t.fg}>{row(i)}</text>
            </box>
          ),
        )}
      </box>
      <box key="register" {...card} onKey={{ kind: 'register' }} keyFocus focusable role="group" label="register">
        {caption('an onKey sink: y → setClipboard, p → requestPaste')}
        {model.lines.map((line, i) => (
          <box key={`l${i}`} width="grow" padX={8} padY={3} radius={4} bg={i === model.cursor ? t.selection : t.surface}>
            <text size={13} family="mono" color={t.fg}>{line}</text>
          </box>
        ))}
      </box>
      <box width="grow" maxWidth={600} gap={2}>
        <text size={12} color={t.accent}>{`→ clipboard: ${model.sent}`}</text>
        <text size={12} color={t.accent}>{`← paste: ${model.received}`}</text>
      </box>
    </box>
  );
}

await run<Model, AppMsg>({
  name: 'clipboard',
  init,
  update,
  view,
  keys: [['⌘C ⌘X ⌘V', 'in the field and the card'], ['right-click', 'the stock menu\'s Copy / Paste'], ['j k y p', 'in the register']],
  window: { width: 640, height: 640 },
  nativeMenus: false,
  // Each path onto the queue, and what it leaves there — read from
  // `takeMenuActions()`, which is what a window's runner drains.
  headless: (app: App<Model, Msg>) => {
    const ok = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    const ctx = app.ctx;
    ctx.setNativeMenus(false);
    ctx.setInspect(true);
    app.render();
    app.render();

    // The field: the chord's copy is answered at once and never queued —
    // the runner writes the clipboard itself; a paste is typing.
    ctx.focus('note');
    ctx.key('selectall');
    ok(ctx.requestCopy().text?.includes('copy it') === true, 'the field\'s copy is answered at once, from the editor\'s selection');
    ok(ctx.takeMenuActions().length === 0, 'and nothing is queued: the runner writes the clipboard itself');
    app.type('pasted');
    ok(ctx.editText('note') === 'pasted', 'a paste into the field is typing over the selection');

    // The card: a drag across the scope selects its runs as one; the
    // stock menu's Copy queues text and HTML for the host.
    const rect = (label: string) => {
      const n = ctx.nodes().find((n) => n.label === label);
      if (!n) throw new Error(`no node labelled ${label}`);
      return n.rect;
    };
    const a = rect('article');
    ctx.cursor(a.x + 30, a.y + 40); ctx.mouse(true); ctx.cursor(a.x + 300, a.y + 44); ctx.mouse(false);
    ok(ctx.selectionText()?.includes('bold') === true, 'a drag across the card selects its runs as one');
    ok(ctx.selectionHtml()?.includes('<b>') === true, 'and the copy carries the bold as HTML');
    app.rightClick(a.x + 30, a.y + 40);
    const menu = ctx.menu();
    const copy = menu?.items.findIndex((i) => i.role === 'copy') ?? -1;
    ok(copy >= 0, 'the card gets the stock menu with Copy');
    ctx.activateMenuItem(copy);
    const queued = ctx.takeMenuActions();
    ok(queued.length === 1 && queued[0].kind === 'setClipboard' && queued[0].text.includes('bold') && (queued[0].html ?? '').includes('<b>'),
      'the menu\'s Copy queues the text and the HTML for the host');

    // The log: select across rows, scroll them away, and the copy is a
    // question for the app — answered from its rows.
    const log = rect('log');
    // Held past the log's bottom edge, the log scrolls toward the pointer a
    // frame at a time and the live end follows (ADR 0029): half a second
    // 60 px past is 600 px/s. The wheel under the held press moves it too,
    // and a Shift-click extends from the anchor instead of starting over.
    ctx.cursor(log.x + 20, log.y + 6); ctx.mouse(true); ctx.cursor(log.x + 200, log.y + log.h + 60);
    for (let i = 0; i < 30; i++) app.advance(1000 / 60);
    ok(ctx.scrollOffset('log').y > 10 * ROW_H, 'a press held past the edge scrolls the log toward the pointer');
    ok((ctx.selectionEnds()?.focus.index ?? 0) >= 10, 'and the live end followed onto the rows that scrolled in');
    ctx.cursor(log.x + 200, log.y + 2.5 * ROW_H); ctx.scroll(0, -20 * ROW_H);
    app.render(); app.render();
    ok((ctx.selectionEnds()?.focus.index ?? 0) >= 30, 'the wheel under a held press moves the live end with the rows');
    ctx.mouse(false);
    ctx.modifiers({ shift: true });
    ctx.cursor(log.x + 100, log.y + 1.5 * ROW_H); ctx.mouse(true); ctx.mouse(false);
    ctx.modifiers({});
    app.render();
    const extended = ctx.selectionEnds();
    ok(extended?.anchor.index === 0 && (extended?.focus.index ?? 0) > 25, 'a Shift-click extends from the anchor instead of starting over');
    ctx.setScroll('log', 0, 0);
    app.render();
    ctx.cursor(log.x + 20, log.y + 6); ctx.mouse(true); ctx.cursor(log.x + 200, log.y + 2.5 * ROW_H); ctx.mouse(false);
    app.render();
    ctx.cursor(log.x + 100, log.y + 40); ctx.scroll(0, -40 * ROW_H);
    app.render();
    ok(ctx.requestCopy().asked, 'a copy over rows the frame never built is asked of the app');
    app.settle();
    ok(app.model.sent.startsWith('the log\'s rows'), 'as a `selectionrange` the app answers from its rows');
    const answered = ctx.takeMenuActions();
    ok(answered.length === 1 && answered[0].kind === 'setClipboard' && answered[0].text.includes('log line 0\n') && answered[0].text.includes('log line 2'),
      'and the answer is what reaches the clipboard');
    // The same drag made backwards — pressed on row 2, released on row 0
    // — is asked for as the same range: `from` precedes `to` whichever
    // end the press was, so the app's `from..=to` answers the same rows.
    ctx.setScroll('log', 0, 0);
    app.render();
    ctx.cursor(log.x + 200, log.y + 2.5 * ROW_H); ctx.mouse(true); ctx.cursor(log.x + 20, log.y + 6); ctx.mouse(false);
    app.render();
    ctx.cursor(log.x + 100, log.y + 40); ctx.scroll(0, -40 * ROW_H);
    app.render();
    ok(ctx.requestCopy().asked, 'a backwards drag over unbuilt rows is asked the same way');
    app.settle();
    const backwards = ctx.takeMenuActions();
    const back = backwards[0];
    const sent = answered[0].kind === 'setClipboard' ? answered[0].text : null;
    ok(backwards.length === 1 && back.kind === 'setClipboard' && back.text === sent,
      'and asks for the same rows in reading order, so the answer is the same text');

    // The register: the sink's own bindings, one call each.
    ctx.focus('register');
    app.press('j');
    app.press('y');
    const yank = ctx.takeMenuActions();
    ok(yank.length === 1 && yank[0].kind === 'setClipboard' && yank[0].text.startsWith('the sink hears'), 'y hands the sink\'s line to the host through setClipboard');
    app.press('p');
    ok(JSON.stringify(ctx.takeMenuActions()) === JSON.stringify([{ kind: 'paste' }]), 'p asks the host for the clipboard');
    // The host reads it and commits; the sink hears the text.
    ctx.commit('from another app');
    app.settle();
    ok(app.model.lines.at(-1) === 'from another app', 'and the paste comes back as the sink\'s text message');
    ok(!app.model.awaitingPaste, 'once');
    return true;
  },
});
