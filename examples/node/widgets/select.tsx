// The stock select from JSX: `<select label options current/>` is a field
// showing the choice in force that, clicked, opens the core's own menu of
// the options under it — drawn in the frame, or the platform's where the
// host shows menus itself — with the current one checked. The app holds no
// open state; what reaches `update` is the `menu` message a menu row posts,
// on the field's key, its `item` the option's label or the item's `id`.
// Drawing the field again with the new `current` is the whole loop
// (backlog F72, F73).
//
//   npm run select                          a window to pick in by hand
//   node dist/widgets/select.mjs --headless the same loop, no window
import type { App, CoreMsg, KuiNode, MenuMsg, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

const LANGUAGES = ['English', 'Deutsch', 'Français', '日本語'];
const SIZES = [11, 13, 15, 18];

type Msg = CoreMsg;
type Model = { language: number; size: number; last: unknown };

const init: Model = { language: 0, size: 1, last: undefined };

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>): Model | undefined {
  // `{kind: 'menu', role, item}` on the field's key: the language field's
  // posts the label, the size field's the item's `id` — the size itself —
  // so the item's type says which field it came from.
  if (msg.kind !== 'menu') return undefined;
  const item = (msg as MenuMsg<unknown>).item;
  const next = { ...model, last: item };
  if (typeof item === 'string') {
    const i = LANGUAGES.indexOf(item);
    if (i >= 0) next.language = i;
  }
  if (typeof item === 'number') {
    const i = SIZES.indexOf(item);
    if (i >= 0) next.size = i;
  }
  return next;
}

function view(model: Model) {
  const row = (label: string, field: KuiNode) => (
    <box dir="row" gap={12} width="grow" crossAlign="center">
      <box width={90}>
        <text size={13} color="$muted">{label}</text>
      </box>
      {field}
    </box>
  );
  return (
    <box width="grow" height="grow" pad={24} gap={14} crossAlign="start">
      <text size={12} color="$muted">{'<select>: the choice posts its label'}</text>
      {row('language', <select label="language" options={LANGUAGES} current={model.language} />)}
      <text size={12} color="$muted">{"options as menu items: the choice posts the item's `id` — the size itself"}</text>
      {row(
        'size',
        <select
          label="size"
          options={SIZES.map((s) => ({ label: `${s} pt`, id: s }))}
          current={model.size}
        />,
      )}
      <box height={10} />
      <text size={SIZES[model.size]}>{`${LANGUAGES[model.language]} at ${SIZES[model.size]} pt`}</text>
      <text size={12} color="$faint" family="mono">
        {model.last === undefined
          ? 'no choice yet — click a field, or Tab to it and press Space'
          : `last message item: ${JSON.stringify(model.last)}`}
      </text>
    </box>
  );
}

await run<Model, never>({
  name: 'select',
  init,
  update,
  view,
  keys: [['click', 'open a field'], ['↑ ↓ ⏎', 'walk and choose'], ['esc', 'close']],
  window: { width: 520, height: 300 },
  // The drawn menu, so the rows are in the frame to click.
  nativeMenus: false,
  headless: (app: App<Model, Msg>) => {
    const ok = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    app.render();
    const field = () => app.accessTree().nodes.find((n) => n.role === 'button' && n.name === 'language')!;
    ok(field().description === 'English', 'the field describes itself by the choice in force');
    app.access('language', 'click');
    const menu = app.ctx.menu();
    ok(menu !== null && menu.items.length === 4 && menu.items[0].checked, 'the click opened the menu with the current row checked');
    ok(app.model.last === undefined, 'and nothing reached update');
    app.render();
    ok(field().expanded === true, 'the field reads as expanded while it is open');
    const row = app.accessTree().nodes.find((n) => n.role === 'menuItem' && n.name === 'Deutsch')!;
    app.click(row.rect.x + row.rect.w / 2, row.rect.y + row.rect.h / 2);
    ok(app.model.language === 1 && app.model.last === 'Deutsch', 'a row chosen is one menu message naming the option');
    ok(app.ctx.menu() === null, 'and the menu closed');
    app.render();
    ok(field().description === 'Deutsch', 'the field shows the new choice');
    // The item form posts the id, not the label.
    app.access('size', 'click');
    app.render();
    const big = app.accessTree().nodes.find((n) => n.role === 'menuItem' && n.name === '18 pt')!;
    app.click(big.rect.x + big.rect.w / 2, big.rect.y + big.rect.h / 2);
    ok(app.model.size === 3 && app.model.last === 18, "the item's id is what the message carries");
    ok(app.warnings.length === 0, 'no warnings');
    return true;
  },
});
