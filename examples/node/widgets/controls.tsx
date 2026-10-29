// The stock controls from JSX (docs/adr/0034-stock-controls-over-the-roles.md):
// `<switch>`, `<checkbox>` with a select-all that goes `mixed`, `<radioGroup>`
// of `<radio>`s whose arrows move the choice, and `<slider>`s whose changes
// arrive as their `onChange` message with `change: {value, phase}` in it. None holds state: each
// is drawn from the model, a toggle's press is its `onClick` message, and a
// slider's value is proposed by the core and stored by `update`. Under them
// the read-only bars (backlog F267): a `<progress>` counting the channels
// set up, a meter showing the volume as a level, and a busy bar with no
// value — none a control, each read with its value. The twin of
// `rust/widgets/controls.rs`.
//
//   npm run controls                           a window to use by hand
//   node dist/widgets/controls.mjs --headless  the same loop, no window
import type { App, SliderChangeMsg, UiEvent } from '@qxuken/kui';
import { run } from '../devtools.js';

const THEMES = ['Light', 'Dark', 'System'];
const CHANNELS = ['Mail', 'Calendar', 'Chat'];

type AppMsg =
  | { kind: 'notify' }
  | { kind: 'all' }
  | { kind: 'channel'; i: number }
  | { kind: 'theme'; i: number }
  | SliderChangeMsg<{ kind: 'volume' }>
  | SliderChangeMsg<{ kind: 'gain' }>;
type Msg = AppMsg;
type Model = { notify: boolean; channels: boolean[]; theme: number; volume: number; gain: number };

const init: Model = { notify: true, channels: [true, false, true], theme: 2, volume: 40, gain: 0.5 };

function update(model: Model, msg: Msg, _ev: UiEvent<Msg>): Model | undefined {
  switch (msg.kind) {
    // A slider's proposal, in the message its `onChange` declared: store
    // it and the next frame draws it.
    case 'volume':
      return { ...model, volume: msg.change.value };
    case 'gain':
      return { ...model, gain: msg.change.value };
    case 'notify':
      return { ...model, notify: !model.notify };
    case 'all': {
      const all = !model.channels.every(Boolean);
      return { ...model, channels: model.channels.map(() => all) };
    }
    case 'channel':
      return { ...model, channels: model.channels.map((c, i) => (i === msg.i ? !c : c)) };
    case 'theme':
      return { ...model, theme: msg.i };
    default:
      return undefined;
  }
}

function view(model: Model) {
  const ticked = model.channels.filter(Boolean).length;
  const heading = (s: string) => <text size={12} color="$muted">{s}</text>;
  return (
    <box width="grow" height="grow" pad={24} gap={12} crossAlign="start">
      {heading('switch')}
      <switch checked={model.notify} onClick={{ kind: 'notify' }}>Notifications</switch>
      {heading('checkbox, with a select-all that can be mixed')}
      <checkbox
        checked={ticked === CHANNELS.length}
        mixed={ticked > 0 && ticked < CHANNELS.length}
        disabled={!model.notify}
        onClick={{ kind: 'all' }}
      >
        All channels
      </checkbox>
      <box padL={24} gap={8}>
        {CHANNELS.map((name, i) => (
          <checkbox checked={model.channels[i]} disabled={!model.notify} onClick={{ kind: 'channel', i }}>
            {name}
          </checkbox>
        ))}
      </box>
      {heading('radio group — Tab to it, then the arrows')}
      <radioGroup label="Theme">
        {THEMES.map((name, i) => (
          <radio checked={model.theme === i} onClick={{ kind: 'theme', i }}>
            {name}
          </radio>
        ))}
      </radioGroup>
      {heading('slider — press, drag, or the arrows, Page keys, Home and End')}
      <box dir="row" gap={12} crossAlign="center">
        <slider label="Volume" valueNow={model.volume} valueMin={0} valueMax={100} valueStep={5} onChange={{ kind: 'volume' }} />
        <text size={13}>{model.volume.toFixed(0)}</text>
      </box>
      <box dir="row" gap={12} crossAlign="center">
        <slider
          label="Gain"
          width={120}
          valueNow={model.gain}
          valueMin={0}
          valueMax={1}
          valueStep={0.1}
          valueText={`gain ${model.gain.toFixed(1)}`}
          tooltip="Steps of a tenth, proposed as the decimal"
          onChange={{ kind: 'gain' }}
        />
        <text size={13}>{model.gain.toFixed(1)}</text>
      </box>
      {heading('progress and meter — read-only, no Tab stop, read with their value')}
      <box dir="row" gap={12} crossAlign="center">
        <progress
          label="Channels set up"
          valueNow={ticked}
          valueMin={0}
          valueMax={CHANNELS.length}
          valueText={`${ticked} of ${CHANNELS.length}`}
        />
        <text size={13}>{`${ticked} of ${CHANNELS.length} channels`}</text>
      </box>
      {/* A level, not a task under way: a meter. */}
      <box dir="row" gap={12} crossAlign="center">
        <progress label="Output level" role="meter" valueNow={model.volume} valueMax={100} />
        <text size={13}>output level</text>
      </box>
      {/* No value: busy, read as indeterminate. */}
      <box dir="row" gap={12} crossAlign="center">
        <progress label="Syncing" />
        <text size={13}>syncing…</text>
      </box>
    </box>
  );
}

await run<Model, AppMsg>({
  name: 'controls',
  init,
  update,
  view,
  keys: [['click / space', 'toggle'], ['↑ ↓', 'move the radio'], ['← → pgup pgdn home end', 'move a slider']],
  window: { width: 520, height: 680 },
  headless: (app: App<Model, Msg>) => {
    const ok = (cond: boolean, what: string) => {
      if (!cond) throw new Error(what);
      console.log(`  ok   ${what}`);
    };
    const node = (role: string, name: string) => app.accessTree().nodes.find((n) => n.role === role && n.name === name)!;
    app.render();
    const setUp = node('progress', 'Channels set up');
    ok(setUp.valueNow === 2 && setUp.value === '2 of 3', 'the progress bar reads two of three');
    ok(node('progress', 'Syncing').valueNow === null, 'the busy bar carries no number');
    ok(node('checkbox', 'All channels').mixed, 'two of three channels: the select-all is mixed');
    app.access('All channels', 'click');
    app.render();
    ok(app.model.channels.every(Boolean), 'and ticking it ticks every row');
    ok(node('checkbox', 'All channels').checked === true, 'then it reads checked');
    ok(node('progress', 'Channels set up').valueNow === 3, 'and the bar follows: three of three');

    const vol = node('slider', 'Volume');
    ok(vol.valueStep === 5, 'the slider reports its step');
    // Three quarters along the track: the node less half the thumb at each end.
    const thumb = vol.rect.h;
    app.click(vol.rect.x + thumb / 2 + (vol.rect.w - thumb) * 0.73, vol.rect.y + vol.rect.h / 2);
    ok(app.model.volume === 75, 'a press three quarters along is 75, snapped to 5');
    app.render();
    app.ctx.focus(node('slider', 'Volume').key);
    app.key('end');
    ok(app.model.volume === 100, 'End is the top');
    app.render();
    ok(node('meter', 'Output level').valueNow === 100, 'the meter reads the volume');
    app.ctx.focus(node('slider', 'Gain').key);
    app.key('left');
    ok(app.model.gain === 0.4, 'a tenth down is 0.4 on the wire');
    ok(app.warnings.length === 0, 'no warnings');
    return true;
  },
});
