// A deck is ordinary TSX. `npm run presentation`; --export DIR writes PNGs.
import { createApp } from '@qxuken/kui';
import * as kit from '@qxuken/kui/kit';
import { createPresentation, present, type Deck } from '@qxuken/kui/kit/presentation';

const deck: Deck = {
  title: 'Ideas, running.', preset: 'minimal',
  // Project overrides merge with the selected preset:
  // settings: { fontTitle: ['Georgia', 'serif'], roles: { accent: '#8c3e70' },
  //   sizes: { heading: 58, content: 24, code: 18, chart: 14 } },
  // canvas: { w: 1280, h: 720 }, // optional; otherwise regular adaptive flow
  slides: [
    { key: 'intro', title: 'Make ideas executable.', notes: 'Open the presenter with Cmd/Ctrl+Shift+P. Advance through both reveals.',
      view: c => <c.Content grow preset="title" title="Make ideas executable." subtitle="A presentation can be a place to think.">
        <c.Reveal step={1}><kit.Paragraph text="Compose with ordinary Kit components." /></c.Reveal>
        <c.Reveal step={2}><kit.Paragraph text="Let the audience explore the idea." /></c.Reveal>
      </c.Content> },
    { key: 'source', title: 'Your project, in its own voice.', notes: 'Fonts, colors and sizes are settings overrides, rather than a new renderer.',
      view: c => <c.Content grow title="Your project, in its own voice.">
        <kit.Code label="project.tsx" language="typescript" lineNumbers copy highlightLines={[2, 3]}
          text={'await present({\n  theme: "paper",\n  settings: { sizes: { heading: 58 } },\n  slides,\n});'} />
        <c.Reveal step={1}><kit.Quote text="Keep the useful defaults. Make them yours." attribution="Kit settings" /></c.Reveal>
      </c.Content> },
    { key: 'live', title: 'Show it. Change it.', notes: 'The timer runs only while this slide is active. Preview/export does not activate it.',
      init: () => ({ workers: 4, runs: 0, seconds: 0 }),
      activate: c => { c.every(1000, { kind: 'tick' }); },
      update: (state, message) => ({ ...state,
        workers: message.kind === 'workers' ? message.change.value : state.workers,
        runs: state.runs + (message.kind === 'run' ? 1 : 0),
        seconds: state.seconds + (message.kind === 'tick' ? 1 : 0),
      }),
      view: c => <c.Content grow title="Show it. Change it.">
        <kit.Stat label="Illustrative throughput" display value={`${Math.floor(c.state.workers * 138 / (1 + c.state.workers * .035))} jobs/s`} />
        <kit.Slider label="Concurrency" value={c.state.workers} min={1} max={16} step={1} onChange={c.msg({ kind: 'workers' })} />
        <kit.Button label="Run workload" variant="primary" onPress={c.msg({ kind: 'run' })} />
        <kit.Code language="json" text={JSON.stringify(c.state, null, 2)} copy />
        <c.Reveal step={1}><kit.Paragraph text="The experiment and the explanation belong together." /></c.Reveal>
      </c.Content> },
  ],
};

if (process.argv.includes('--headless')) {
  const controller = createPresentation(deck, { snapshot: true });
  const app = createApp({ init: controller.model, update: (_m, message) => controller.update(message),
    view: (_m, window, surface) => controller.view(surface, window) },
  { width: 1280, height: 800, extensions: [kit.extension], warnings: false });
  for (const frame of controller.exportPlan(true)) {
    controller.jump(frame.key); controller.model.step = frame.step; app.render();
  }
  if (app.warnings.length) throw new Error(JSON.stringify(app.warnings));
  app.teardown(); controller.dispose();
  console.log('presentation: every slide and reveal renders headlessly');
} else {
  await present(deck, { width: 1440, height: 940 });
}
