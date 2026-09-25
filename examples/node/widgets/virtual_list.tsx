// The `uniformList` widget, from JSX: a ten-thousand-row list that costs
// a screenful. The core builds every child a view declares, so the view
// declares a screenful: the widget reads the container's geometry, slices
// the rows that can be seen, and holds the height of the rest in two
// spacers — and opens each row at its data `index`, so a row keeps its
// hover, focus and tweens as the built range slides past it.
//
// It is also what backlog C25 was about. Before it, this file had three
// things a Rust view never needs: a zero-height `onLayout` sentinel (the
// wheel raises no event, and a window redraws by re-lowering the tree it was
// handed, so nothing would have made `view` run again), a `key={String(i)}`
// on every row (no binding had `index`), and a `keyOf` guard (asking for the
// geometry of a container the first frame has not declared *threw*). All
// three are gone; the widget and the two doors under it are the entry.
//
// And when the rows are *not* all one height — a log whose lines wrap —
// `list` is the same idea over the core's `RowHeights` instead of a stride
// (backlog C46): `measure` runs only for the rows a frame is about to build,
// everything else stands at the mean of those, and the row the window starts
// in is put back where it was each time the frame learns something, so the
// content never slides. `--variable` shows that one; `--headless` drives both.
//
//   npm run virtual_list                          a window to scroll by hand
//   npm run virtual_list -- --variable            rows of no fixed height
//   node dist/widgets/virtual_list.mjs --headless the same slicing, no window
import { RowHeights, createApp, list, uniformList } from '@qxuken/kui';
import type { App, CoreMsg, Ctx, KuiWindow, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

const ROWS = 10_000;
const ROW_H = 28;

type Msg = { kind: 'pick'; row: number } | CoreMsg;
type Model = { selected: number };

const init: Model = { selected: 0 };

const VARIABLE = process.argv.includes('--variable');

/** `--variable`'s data: lines of very different lengths, so wrapping gives
 *  the rows three or four different heights and no stride describes them —
 *  the same lines the Rust example's `--variable` wraps. */
function lineOf(i: number): string {
  const words = 2 + ((i * 7 + Math.floor(i / 3)) % 22);
  let s = `${String(i).padStart(5, ' ')}  `;
  for (let w = 0; w < words; w++) s += ['log', 'line', 'of', 'some', 'length', 'here'][w % 6] + ' ';
  return s;
}

/** The style a variable row's text is measured *and* drawn in: the
 *  measurement is only worth anything if it is the same style. */
const BODY = { size: 13, wrap: 'word' } as const;

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
// asks for the two calls both of them have, which is what `uniformList`
// takes as well.
type Surface = Pick<
  Ctx | KuiWindow,
  'scrollGeometry' | 'scrollOffset' | 'shiftScroll' | 'measureText' | 'env' | 'theme'
>;

/** Rows of different heights, from `measureText` — the number layout itself
 *  would give the row's text at that width, so what a row measures is what
 *  it gets. `heights` is the app's, kept between views. */
function variableView(model: Model, ctx: Surface, heights: RowHeights) {
  const t = ctx.theme();
  const rowBg = (i: number) =>
    i === model.selected ? mix(t.surface, t.accent, 0.28) : i % 2 ? t.sunken : t.surface;
  return (
    <box width="grow" height="grow" bg={t.bg} dir="column">
      <box pad={8} bg={t.surface}>
        <text size={14} color={t.fg}>
          {`${ROWS} rows of no fixed height — row ${model.selected} selected`}
        </text>
      </box>
      {list(
        ctx,
        { key: 'log', heights, width: 'grow', height: 'grow', role: 'list', label: 'log' },
        // The row pads itself by 6 on each side, and that padding is part of
        // the height the row gets.
        (i, width) => ctx.measureText(lineOf(i), BODY, width - 12).height + 12,
        (i) => (
          <box
            width="grow"
            height="grow"
            pad={6}
            bg={rowBg(i)}
            hoverBg={mix(rowBg(i), t.accent, 0.12)}
            onClick={{ kind: 'pick', row: i }}
            role="listItem"
            label={`row ${i} of ${ROWS}`}
          >
            <text {...BODY} color={t.fg}>{lineOf(i)}</text>
          </box>
        ),
      )}
    </box>
  );
}

const windowHeights = new RowHeights(ROWS, 40);

function view(model: Model, ctx: Surface) {
  if (VARIABLE) return variableView(model, ctx, windowHeights);
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
      {uniformList(
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

await run<Model, { kind: 'pick'; row: number }>({
  name: 'virtual_list',
  init,
  update,
  view,
  keys: [['wheel', 'scroll 10,000 rows']],
  flags: [['--variable', 'rows of no fixed height, through list()']],
  window: { width: 480, height: 300 },
  // Two frames, because the first has no geometry to slice by; then a
  // wheel to row 300, which reaches no `update` — the widget's own
  // bookkeeping re-runs `view` against the new geometry; then a click on
  // a row the first frame never built.
  headless: (app: App<Model, Msg>) => {
    const ok = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    app.render();
    app.render();
    const listItems = () => app.ctx.accessTree().nodes.filter((n) => n.role === 'listItem');
    const screenful = Math.floor(300 / ROW_H);
    ok(listItems().length >= screenful && listItems().length < 4 * screenful,
      'frame 2 builds a screenful and its overscan, not 10,000 rows');
    ok(app.ctx.scrollGeometry('log')!.contentH >= ROWS * ROW_H * 0.9, 'the content is the whole list\'s height');

    app.ctx.cursor(240, 200);
    app.ctx.scroll(0, -300 * ROW_H);
    app.render();
    app.step();
    const first = listItems()[0]?.name ?? '';
    const row = Number(first.match(/row (\d+)/)?.[1] ?? -1);
    ok(row >= 290 && row <= 300, 'a wheel to row 300 re-slices the built range');
    ok(app.warnings.length === 0, 'and the frame raised no warning');

    app.click(240, 100);
    ok(app.model.selected > 100, 'a row the first frame never built is clickable');

    // The variable list, in an app of its own so the round drives it
    // whichever list the window shows.
    const heights = new RowHeights(ROWS, 40);
    const v = createApp<Model, Msg>(
      { init, update, view: (m, _w, ctx) => variableView(m, ctx, heights) },
      { width: 480, height: 300 },
    );
    v.render();
    v.render();
    const items = () => v.ctx.accessTree().nodes.filter((n) => n.role === 'listItem');
    ok(items().length > 0 && items().length < 40, 'list builds a screenful of rows of no fixed height');
    ok(heights.measured(0) !== null && heights.estimate() !== 40,
      'the rows it built were measured, and the estimate moved off its seed to their mean');
    v.ctx.setScroll('log', 0, 20_000);
    v.render();
    v.click(240, 60);
    const under = v.model.selected;
    for (let n = 0; n < 3; n++) {
      v.render();
      v.click(240, 60);
      ok(v.model.selected === under, `the row under the pointer stays put as the estimate moves (${n + 1})`);
    }
    ok(v.warnings.length === 0, 'and the frames raised no warning');
    return true;
  },
});
