# Presentation style prototype — throwaway

Question: which visual hierarchy, density and interaction feel should Kit's
presentation tools support? Four structurally different native presets
render the same three-slide story using today's Kit and kui primitives.
Their themes and layouts can now be selected independently.

```bash
cargo run -p kui-cli -- run addons/kit/samples/presentation-prototype
```

Or, with the runner installed:

```bash
kui run --watch addons/kit/samples/presentation-prototype
```

- **Paper**: an editorial spread, serif hierarchy and generous margins.
- **Signal**: a technical briefing with chapter navigation and data first.
- **Stage**: an asymmetric composition with oversized type and one visual.
- **Minimal**: a quiet reading column with plain typography, whitespace
  and very little framing.

Captured directions: [Paper](shots/paper.png), [Signal](shots/signal.png),
[Stage](shots/stage.png), [Minimal](shots/minimal.png),
[all four](shots/directions.png). The data and throughput model are illustrative,
not benchmark results.

The floating bar switches presets without changing slide or demo state.
Keys: **1–4** preset, **T** theme, **L** layout, **H** hide/show prototype
controls, **←/→** or **Space** reveals, **O** overview, **P** notes.
The slider and Run button are real Kit controls; the chart has a real
pointer/keyboard readout. Actions print the current state to the runner.

Reproducible starting points and captures:

```bash
kui run addons/kit/samples/presentation-prototype -- --variant signal --slide 2 --step 2
kui run addons/kit/samples/presentation-prototype -- --variant minimal
kui run addons/kit/samples/presentation-prototype -- --theme paper --layout split
kui run addons/kit/samples/presentation-prototype --shot /tmp/paper.png --size 1440x940 -- --variant paper
kui run addons/kit/samples/presentation-prototype --shot /tmp/stage-live.png --size 1440x940 -- --variant stage --slide 3 --step 1
```

Layout keys are `editorial`, `briefing`, `split`, and `minimal`; the original
variant names also work as layout aliases. `--clean` starts without the
prototype controls, for viewing/capturing the composition alone; **H**
brings the controls back.

This is a visual study, not the agreed presentation API. It has no
persistence, actual presenter window, fullscreen/display control, managed
demo effects, fixed canvas or deck export traversal. Notes are a side-panel
composition study. Large syntax-coloured code is drawn locally to explore
the base Kit improvements; the small output block is today's `kit.code`.
Text-only reveal placeholders use opacity and do not implement the future
general accessibility-safe visibility contract.

Preserve the full prototype on `codex/presentation-style-spike`. The user
found value in all original directions and requested Minimal. The design
draft records a theme/layout distinction so these can become independently
usable Kit features. Rewrite validated ideas for production rather than
promoting this spike directly.

## Observations and review

All four directions render with the real native runner. Wide captures,
a narrow reflow capture, code/live-control views and the notes-panel
composition were inspected; the final capture runs reported no warnings.
Syntax checking and `git diff --check` pass. No production test suite is
added for this throwaway visual study.

The spike exposes useful gaps: chart labels and controls retain Kit's
small tool-window typography; larger syntax-coloured code currently needs
local drawing; adaptive composition works without a fixed canvas. These
are implementation observations, not a user's stylistic verdict. The
public authoring interface still needs to be implemented and evaluated.
Minimal's text, chart and demo compositions, narrow reflow, and a mixed
Paper-theme/split-layout composition were also rendered and inspected.
