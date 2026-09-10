// The counter, from Node: the one example every binding has in the same
// shape (docs/adr/0021, decision 3). `view(model)` is JSX, messages are
// plain data: a click arrives in `update` as the payload the button
// declared and moves the model; nothing else happens.
//
// What every counter holds, so the four stay one shape: the count with
// +1, -1 and reset; a `name` field whose text the greeting reads back
// (`win.editText`); a right-click that declares a `modal` menu on the next
// frame, with +10 and reset in it; and a headless drive that clicks all of
// it by label and exits non-zero on a wrong answer.
//
//   npm run counter                    a window, with the harness dock
//   node dist/apps/counter.mjs --headless
import type { App, CoreMsg, KuiWindow, Theme, UiEvent } from '@qxuken/kui';
import { run } from '../harness.js';

type Model = {
  count: number;
  /** Where the last right-click landed, while its menu is open. */
  menu: { x: number; y: number } | null;
  greeting: string;
};

// The payloads this app's own nodes carry, plus the ones the core sends on
// its own (`changed`, `contextmenu`, `dismiss`, ...): one flat union, so
// `update` switches over `msg.kind` with no casts.
type CounterMsg = { kind: 'add'; by: number } | { kind: 'reset' };
type Msg = CounterMsg | CoreMsg;

const init: Model = { count: 0, menu: null, greeting: 'kui counter' };

function update(model: Model, msg: Msg, ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  switch (msg.kind) {
    case 'add':
      // Either counter button, or the menu's — which closes the menu, the
      // way choosing an item does everywhere.
      return { ...model, count: model.count + msg.by, menu: null };
    case 'reset':
      return { ...model, count: 0, menu: null };
    case 'changed': {
      // The field's text is the core's; read it back through the key.
      const who = (win.editText(ev.key) ?? '').trim();
      return { ...model, greeting: who ? `${who}'s counter` : 'kui counter' };
    }
    case 'contextmenu':
      // A right-click: the core opens nothing, the view declares the menu
      // where the press landed.
      return { ...model, menu: { x: msg.x, y: msg.y } };
    case 'dismiss':
      // Escape, or a press outside the menu.
      return { ...model, menu: null };
  }
}

/** The context menu: a modal float at the press. `modal` scopes Tab and
 *  the pointer to it and brings the `dismiss` above back. */
function Menu({ at, t }: { at: { x: number; y: number }; t: Theme }) {
  return (
    <box
      float={{ anchor: 'viewport', at: ['start', 'start'], self: ['start', 'start'],
               dx: at.x, dy: at.y, fit: true }}
      modal={{ kind: 'menu' }}
      label="Actions"
      pad={4} gap={4} width={120} bg={t.raised} radius={6} borderW={1} borderColor={t.borderStrong}
    >
      <button onClick={{ kind: 'add', by: 10 }}>+10</button>
      <button onClick={{ kind: 'reset' }}>reset</button>
    </box>
  );
}

// Every colour is a theme role (ADR 0019), so the window follows the OS's
// light/dark and its accent without a branch.
const view = (model: Model, win: KuiWindow) => {
  const t = win.theme();
  return (
    <box width="grow" height="grow" center gap={24} onContextMenu={{ kind: 'menu' }}>
      <box pad={32} gap={20} bg={t.surface} radius={12} borderW={1} borderColor={t.border} crossAlign="center" width={320}>
        <text size={14} color={t.muted}>{model.greeting}</text>
        <text size={56} color={t.fg}>{String(model.count)}</text>
        <box dir="row" gap={12}>
          <button onClick={{ kind: 'add', by: -1 }}>-1</button>
          <button onClick={{ kind: 'add', by: 1 }}>+1</button>
          <button onClick={{ kind: 'reset' }}>reset</button>
        </box>
        <edit key="name" label="name" initial="" size={16} width="grow" padX={10} padY={8}
              bg={t.sunken} color={t.fg} radius={6} borderW={1} borderColor={t.border} />
      </box>
      <text size={12} color={t.faint}>right-click for a menu · clicks are data: view() never sees a callback</text>
      {model.menu ? <Menu at={model.menu} t={t} /> : null}
    </box>
  );
};

await run<Model, CounterMsg>({
  name: 'counter',
  init,
  update,
  view,
  keys: [['right-click', 'the modal menu'], ['Esc', 'dismiss it']],
  window: { width: 560, height: 400 },
  // The Rosetta drive: every counter clicks its buttons by label, types
  // into its field, opens and dismisses its menu, and checks the model
  // after each.
  headless: (app: App<Model, Msg>) => {
    app.render();
    const ok = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    app.access('+1', 'click');
    app.access('+1', 'click');
    app.access('-1', 'click');
    ok(app.model.count === 1, 'two +1 and a -1 count to 1');

    app.ctx.focus('name');
    app.type('Ada');
    app.render();
    ok(app.model.greeting === "Ada's counter", 'the greeting reads the field back');

    app.rightClick(40, 40);
    ok(app.model.menu !== null, 'a right-click asks for the menu');
    app.render();
    ok(app.ctx.keyOf('+10') !== null, 'and the next frame declares it');
    app.access('+10', 'click');
    app.render();
    ok(app.model.count === 11 && app.model.menu === null, '+10 counts and closes the menu');

    app.rightClick(40, 40);
    app.render();
    app.key('escape');
    ok(app.model.menu === null, 'escape dismisses the menu');
    app.render();
    ok(app.ctx.keyOf('+10') === null, 'and the frame after has no menu');
    return true;
  },
});
