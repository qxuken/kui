// The table from JSX (ADR 0033): `<box dir="table">` is a column whose
// rows' children line up in columns, each column as wide as its widest
// cell. No width is picked by hand and nothing is measured: the name
// column sits at the longest name, the size column at the widest size,
// and the kind column grows into the rest because its cells say `grow`.
// The rows are rows — clickable here, with a hover wash and a selected
// background — and the header's cells sort by the column pressed.
//
//   npm run table                          a window to click in by hand
//   node dist/widgets/table.mjs --headless the same table, no window
import type { App, CoreMsg, KuiNode, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

type File = { name: string; bytes: number; kind: string };
const FILES: File[] = [
  { name: 'Cargo.toml', bytes: 1204, kind: 'manifest' },
  { name: 'src', bytes: 0, kind: 'directory' },
  { name: 'README.md', bytes: 18930, kind: 'markdown' },
  { name: 'target', bytes: 0, kind: 'directory' },
  { name: 'a-rather-long-file-name.rs', bytes: 402, kind: 'rust source' },
  { name: 'LICENSE', bytes: 1067, kind: 'text' },
];

type Sort = 'name' | 'size' | 'kind';
type Msg = { kind: 'select'; row: number } | { kind: 'sort'; by: Sort } | CoreMsg;
type Model = { sort: Sort; selected: number | undefined };

const init: Model = { sort: 'name', selected: undefined };

function order(sort: Sort): number[] {
  const rows = FILES.map((_, i) => i);
  if (sort === 'name') rows.sort((a, b) => (FILES[a].name < FILES[b].name ? -1 : 1));
  if (sort === 'size') rows.sort((a, b) => FILES[b].bytes - FILES[a].bytes);
  if (sort === 'kind') rows.sort((a, b) => (FILES[a].kind + FILES[a].name < FILES[b].kind + FILES[b].name ? -1 : 1));
  return rows;
}

function size(bytes: number): string {
  if (bytes === 0) return '—';
  return bytes < 10000 ? `${bytes} B` : `${(bytes / 1024).toFixed(1)} KB`;
}

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>): Model | undefined {
  if (msg.kind === 'select') return { ...model, selected: msg.row };
  if (msg.kind === 'sort') return { ...model, sort: msg.by };
  return undefined;
}

function view(model: Model) {
  const header = (label: Sort, align: 'start' | 'end', width: 'fit' | 'grow'): KuiNode => (
    <box
      key={`sort-${label}`}
      dir="row"
      width={width}
      mainAlign={align}
      onClick={{ kind: 'sort', by: label }}
      label={`sort by ${label}`}
    >
      <text size={11} color={model.sort === label ? '$accent' : '$muted'}>{label}</text>
    </box>
  );
  return (
    <box width="grow" height="grow" pad={24} gap={12}>
      <text size={12} color="$muted">a table: each column as wide as its widest cell, the last one growing</text>
      <box key="files" dir="table" width="grow" gap={2} bg="$sunken" radius={6} pad={4}>
        <box dir="row" width="grow" gap={16} padX={8} padY={4}>
          {header('name', 'start', 'fit')}
          {header('size', 'end', 'fit')}
          {header('kind', 'start', 'grow')}
        </box>
        {order(model.sort).map((i) => {
          const f = FILES[i];
          const selected = model.selected === i;
          const fg = selected ? '$onAccent' : '$fg';
          return (
            <box
              key={f.name}
              dir="row"
              width="grow"
              gap={16}
              padX={8}
              padY={3}
              radius={4}
              bg={selected ? '$accent' : '$sunken'}
              hoverBg={selected ? '$accent' : '$hover'}
              onClick={{ kind: 'select', row: i }}
              label={f.name}
            >
              {/* A bare text is a cell, held to its column. */}
              <text size={13} color={fg} wrap="none">{f.name}</text>
              {/* A number sits at the column's right edge. */}
              <box key="size" dir="row" mainAlign="end">
                <text size={13} color={fg} family="mono" wrap="none">{size(f.bytes)}</text>
              </box>
              <text size={13} color={selected ? '$onAccent' : '$muted'}>{f.kind}</text>
            </box>
          );
        })}
      </box>
      <text size={12} color="$faint" family="mono">
        {model.selected === undefined
          ? 'click a row to select it, a header to sort by it'
          : `selected: ${FILES[model.selected].name}`}
      </text>
    </box>
  );
}

await run<Model, Msg>({
  name: 'table',
  init,
  update,
  view,
  keys: [['click a row', 'select it'], ['click a header', 'sort by it']],
  window: { width: 560, height: 320 },
  headless: (app: App<Model, Msg>) => {
    const ok = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    app.ctx.setInspect(true);
    app.render();
    const sizes = () => app.ctx.nodes().filter((n) => n.label === 'size').map((n) => n.rect);
    let cells = sizes();
    ok(cells.length === FILES.length, 'a size cell per row');
    const x = cells[0].x;
    ok(cells.every((r) => r.x === x && r.w === cells[0].w), 'every size cell starts at one x and is one width');
    const table = app.ctx.nodes().find((n) => n.label === 'files')!;
    ok(table.table && table.dir === 'column', 'the table is a column that says so');
    const row = () => app.accessTree().nodes.find((n) => n.name === 'a-rather-long-file-name.rs')!;
    ok(row().rect.w === table.rect.w - 8, 'a row is as wide as the table\'s content');
    const before = row().rect;
    app.click(before.x + 4, before.y + before.h / 2);
    ok(app.model.selected === 4, 'a row\'s click selects it');
    app.render();
    const header = app.accessTree().nodes.find((n) => n.name === 'sort by kind')!;
    app.click(header.rect.x + 4, header.rect.y + header.rect.h / 2);
    ok(app.model.sort === 'kind', 'a header\'s click sorts by its column');
    app.render();
    ok(row().rect.y !== before.y, 'and the rows moved');
    cells = sizes();
    ok(cells.every((r) => r.x === x), 'the columns stayed where they were');
    ok(app.warnings.length === 0, 'no warnings');
    return true;
  },
});
