# Ideas, running.

A native Kit presentation authored in Lua, with reveals, notes, a chart,
highlighted source and an interactive workload demo.

```sh
kui run addons/kit/samples/presentations --watch -- --presenter
kui run addons/kit/samples/presentations -- --theme paper --layout editorial
kui run addons/kit/samples/presentations -- --theme stage --layout split
kui run addons/kit/samples/presentations -- --project-theme
kui run addons/kit/samples/presentations --export /tmp/ideas-slides --steps --size 1440x940 --appearance light
```

Right/Space advances, Left goes back, O opens private presenter overview, F toggles
fullscreen and Cmd/Ctrl+Shift+P opens or focuses the presenter. `?` opens
private shortcut help. The audience has no presentation controls; on macOS,
use the Presentation menu to recover the presenter.
The demo's data is illustrative. Its state survives navigation and its
timer runs only while active. `--project-theme` supplies project fonts,
a purple accent and custom content sizes over any selected preset.

See [Presentations as code](../../PRESENTATIONS.md) for authoring,
themes, lifecycle, watch reload and export.
