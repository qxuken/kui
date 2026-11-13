# Lua playground

<a data-kui-example="760" data-kui-playground href="live/playground/index.html?example=counter">Open the Lua playground</a>

Edit the Lua on the left, then choose **Run** or press **Ctrl+Enter**
(**⌘+Enter** on a Mac). The preview is a real kui view: try the buttons,
text fields, and checkboxes. Choose Counter, Layout, or Controls as a starting
point. **Reset example** restores the selected example's source.

Each Run starts with fresh state and temporary application files. Your editor
draft stays in this browser, separately for each example. Code runs locally;
there is no build server or upload. If new code fails, its error appears below
the editor and the last working preview stays available.

`init()` returns the initial `model`, `view(env)` returns the UI tree, and
`update(message)` handles messages. A static layout needs only `view`.
Kit controls are available through `require("kit.ui")`, just as in the
[runner chapter](runner.md).

```lua
{{#include ../../../examples/book-web/playground/examples/counter.lua}}
```

The [Layout](playground.md?example=layout) and
[Controls](playground.md?example=controls) presets show the same API with
different views. You can also open the editor directly from its toolbar link
above when you want more room.

The editor limits source size, Lua memory, and instructions per interaction.
An endless loop reports an error so you can fix it and Run again. This is a
small interactive example runner; native libraries, worker threads, HTTP,
processes, and the other desktop-only facilities are unavailable here, as in
the packed browser runner.

The playground and the book's task list use the same prebuilt Lua runtime
under `live/runtime/`. Running edited source reuses its downloaded, compiled
module; each preview has its own Lua state. Future Lua examples can use that
same runtime without shipping a separate Wasm build per example.
The runtime URL includes a fingerprint of the build's contents, so a book
update invalidates the cached runtime even before a new kui release.
