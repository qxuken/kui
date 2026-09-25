// A process that opens its window again: the chrome a window was created
// with is the one it keeps, so an app whose setting says "custom title bar"
// closes the window it has and opens another — and a smoke test that wants
// two configurations of the same app in sequence (pinned to reduced
// motion, then not) does the same. Both are a second `runWindowed` in one
// process, which used to be refused with `EventLoop can't be recreated`:
// winit builds one loop per process, so the runner parks it when its main
// window closes and the next `Launcher::open` takes it back (backlog F58).
//
// Press the button, or close the window: the same app reopens under the
// other chrome, once. Under `KUI_SMOKE_FRAMES` each window closes itself
// and the round passes only if the second one opened.
//
//   npm run relaunch
import type { CoreMsg, KuiWindow, UiEvent } from '@qxuken/kui';
import { run, type Example } from '../devtools.js';

type Model = { launch: number; chrome: 'native' | 'custom' };
type AppMsg = { kind: 'reopen' };
type Msg = AppMsg | CoreMsg;

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>, win: KuiWindow): Model | undefined {
  if (msg.kind === 'reopen') {
    // Closing is what asks for the next window: `after` below reads the
    // model this one closed on.
    win.close();
    return model;
  }
}

const view = (model: Model, win: KuiWindow) => {
  const t = win.theme();
  const other = model.chrome === 'native' ? 'custom' : 'native';
  // The titlebar is the window's top edge, outside the content's padding,
  // as `window.tsx` has it: inside the padded box it was a strip 24 px in
  // from every edge, its buttons in the middle of the window (backlog RG52).
  return (
    <box bg={t.bg} width="grow" height="grow">
      {model.chrome === 'custom' && <titlebar title={`launch ${model.launch}`} />}
      <box pad={24} gap={12}>
        <text size={20} color={t.fg}>{`launch ${model.launch}: ${model.chrome} chrome`}</text>
        <text size={13} color={t.faint}>
          {model.launch < 2
            ? `close this window, or press the button, and it reopens with ${other} chrome`
            : 'the second window of this process; closing it ends the process'}
        </text>
        {model.launch < 2 && (
          <box padX={12} padY={6} radius={6} bg={t.accent} hoverBg={t.accentHover} pressedBg={t.accentPressed}
               role="button" onClick={{ kind: 'reopen' }}>
            <text size={14} color={t.onAccent}>reopen with {other} chrome</text>
          </box>
        )}
      </box>
    </box>
  );
};

function launch(model: Model): Example<Model, AppMsg> {
  return {
    name: 'relaunch',
    init: model,
    update,
    view,
    window: { width: 520, height: 200, chrome: model.chrome },
    // Under custom chrome the dock goes below, so the app's strip spans
    // the window's top edge and its buttons sit in the window's corner.
    dock: model.chrome === 'custom' ? 'bottom' : undefined,
    // Once: the second window is the last.
    after: (m) => (m.launch < 2 ? launch({ launch: m.launch + 1, chrome: m.chrome === 'native' ? 'custom' : 'native' }) : undefined),
  };
}

await run(launch({ launch: 1, chrome: 'native' }));
