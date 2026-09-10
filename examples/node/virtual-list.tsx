// A ten-thousand-row list in JSX. The core builds every child a view
// declares, so the view declares a screenful: `virtualColumn` reads the
// container's geometry, slices the rows that can be seen, and holds the
// height of the rest in two spacers.
//
// It is also what backlog C25 was about. Before it, this file had three
// things a Rust view never needs: a zero-height `onLayout` sentinel (the
// wheel raises no event, and a window redraws by re-lowering the tree it was
// handed, so nothing would have made `view` run again), a `key={String(i)}`
// on every row (no binding had `index`), and a `keyOf` guard (asking for the
// geometry of a container the first frame has not declared *threw*). All
// three are gone; the widget and the two doors under it are the entry.
//
//   npx esbuild virtual-list.tsx --jsx=automatic --jsx-import-source=@qxuken/kui \
//     --format=esm --outfile=dist/virtual-list.mjs
//   node dist/virtual-list.mjs              a real window to scroll by hand
//   node dist/virtual-list.mjs --headless   the same slicing, no window
import { createApp, runWindowed, virtualColumn } from '@qxuken/kui';
import type { CoreMsg, Ctx, KuiWindow, UiEvent } from '@qxuken/kui';

const ROWS = 10_000;
const ROW_H = 28;

type Msg = { kind: 'pick'; row: number } | CoreMsg;
type Model = { selected: number };

const init: Model = { selected: 0 };

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>): Model | undefined {
  // Nothing about scrolling reaches here: the widget's own bookkeeping never
  // becomes a message the app has to know about.
  return msg.kind === 'pick' ? { selected: msg.row } : undefined;
}

/// `a` moved `t` of the way toward `b`, per channel, keeping `a`'s alpha —
/// `Color::mix` from the Rust side, for the two places a view wants a
/// colour *between* two roles. Both are `0xRRGGBBAA`.
function mix(a: number, b: number, t: number): number {
  const ch = (shift: number) => {
    const x = (a >>> shift) & 0xff;
    const y = (b >>> shift) & 0xff;
    return Math.round(x + (y - x) * t) & 0xff;
  };
  return (((ch(24) << 24) | (ch(16) << 16) | (ch(8) << 8) | (a & 0xff)) >>> 0);
}

// The surface is a `Ctx` headless and a `KuiWindow` in a window; this view
// asks for the two calls both of them have, which is what `virtualColumn`
// takes as well.
type Surface = Pick<Ctx | KuiWindow, 'scrollGeometry' | 'env' | 'theme'>;

function view(model: Model, _window: string, ctx: Surface) {
  // The palette the core derived from the OS (ADR 0019). The surface a
  // view is handed already answers for it, headless and windowed alike,
  // so a striped list follows the appearance without a branch.
  const t = ctx.theme();
  // The two surfaces a striped list alternates between, and the selected
  // row as an opaque step toward the accent — `hoverBg` replaces a
  // background rather than compositing over it, so a wash would not do.
  const rowBg = (i: number) =>
    i === model.selected ? mix(t.surface, t.accent, 0.28) : i % 2 ? t.sunken : t.surface;
  return (
    <box width="grow" height="grow" bg={t.bg} dir="column">
      <box pad={8} bg={t.surface}>
        <text size={14} color={t.fg}>
          {`${ROWS} rows — row ${model.selected} selected`}
        </text>
      </box>
      {virtualColumn(
        ctx,
        { key: 'log', rows: ROWS, rowH: ROW_H, width: 'grow', height: 'grow', role: 'list', label: 'log' },
        (i) => (
          // The row's contents. The widget owns the row's own node — `rowH`
          // tall, keyed by `index={i}` — so the click goes on a child that
          // fills it.
          <box
            width="grow"
            height="grow"
            pad={6}
            dir="row"
            gap={8}
            bg={rowBg(i)}
            hoverBg={mix(rowBg(i), t.accent, 0.12)}
            onClick={{ kind: 'pick', row: i }}
            role="listItem"
            label={`row ${i} of ${ROWS}`}
          >
            <text size={13} color={t.faint}>{String(i).padStart(5, ' ')}</text>
            <text size={13} color={t.fg}>{`log line ${i}`}</text>
          </box>
        ),
      )}
    </box>
  );
}

const config = { init, update, view };

if (process.argv.includes('--headless')) {
  const app = createApp(config, { width: 480, height: 300 });
  app.render();
  app.render(); // the second frame is the first with geometry to slice by
  const listItems = () => app.ctx.accessTree().nodes.filter((n) => n.role === 'listItem');
  console.log('rows built:', listItems().length, 'of', ROWS);

  // What a window does on a wheel: the core moves the retained offset and
  // repaints, the widget's sentinel says the container moved, and the step
  // after that frame re-runs `view` against the new geometry — with no
  // model change anywhere.
  app.ctx.cursor(240, 200);
  app.ctx.scroll(0, -3000);
  app.render();
  app.step();
  const names = listItems().map((n) => n.name);
  console.log('offset', app.ctx.scrollGeometry('log')!.offset.y, '->', names[0], '..', names.at(-1));
  console.log('content height:', app.ctx.scrollGeometry('log')!.contentH, '(the whole list)');
  console.log('warnings:', app.warnings.map((w) => w.code));
} else {
  await runWindowed(config, { title: 'virtual list', width: 480, height: 300 });
}
