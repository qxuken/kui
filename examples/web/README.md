# Web counter

Run from the repository root with the pinned Rust toolchain, Nushell,
Python 3, and the workspace's `wasm-bindgen` CLI version.

```sh
rustup target add wasm32-unknown-unknown
cargo install --locked wasm-bindgen-cli --version 0.2.129
nu scripts/web-counter.nu
```

Open [the counter](http://127.0.0.1:8080/). Use **−1**, **+1** and
**reset**. Right-click the canvas for **+10**, or press **Escape** to
dismiss the menu. The browser runner builds a static site in
`target/web-counter/`. Use `--build-only` to stage it for another HTTP
server, or `--port` to change the local port.

The desktop and web examples share the [counter's view and event handler](../rust/apps/counter/app.rs).
The browser supplies monotonic time, pointer events and the canvas size.
Wasm supplies a core session with bundled Noto Sans and routes input
through kui's hit tree. The default renderer is the native host's
`kui-wgpu`, drawing the full frame through browser WebGPU. The runner
uses Canvas 2D if WebGPU is unavailable or fails before the first frame.
If the device is lost after startup, the runner retries on the same app
instance so edits stay in memory.
Add `?renderer=canvas` to force Canvas 2D or `?renderer=wgpu` to
require WebGPU. The page names the selected renderer.

The build leaves SVG and devtools out by default. Pass `--svg` or
`--devtools` to enable the corresponding core feature in both Wasm
artifacts.

## Notes workspace

Open [the notes workspace](http://127.0.0.1:8080/notes.html) from the same
server. Click the editor or Tab into it. Type, select text, or use your
usual copy, cut, and paste shortcuts. Tab moves between the editor, Save
and the list. Use the wheel to scroll the list or press the arrow keys
while a note button is focused. Enter opens the focused note. Save marks
the current draft as saved for this tab.

Tap to open notes or focus the editor. Drag over a scrollable region to
scroll it, or over text to select it. Sliders, scrollbar thumbs and
explicit drag handlers get the primary press. The browser owns pinch
zoom on the notes canvas.

The textarea is transparent. It receives browser text and composition
events, while kui retains the edit buffer, selection and undo history.
The Canvas 2D renderer paints into one offscreen canvas and copies only
the changed region to the visible canvas.
A DOM mirror exposes the note buttons, editor, list and live save
status to assistive technology. Notes are stored in memory for the
demo. The web examples bundle Noto Sans and a six-glyph Noto Sans
Symbols 2 subset for keyboard shortcut labels. The font licenses are
included in the staged site.

The keyboard subset uses [Noto Sans Symbols 2 from Google Fonts](https://github.com/google/fonts/blob/7b6724ac7ececc713e9ba93af309f7520c9a80a3/ofl/notosanssymbols2/NotoSansSymbols2-Regular.ttf).
To regenerate it with FontTools 4.68.0, run:

```sh
pyftsubset NotoSansSymbols2-Regular.ttf --unicodes=U+21E7,U+2318,U+2325,U+2326,U+232B,U+2423 \
  --output-file=examples/web/fonts/NotoSansSymbols2-Keyboard.ttf
```

### Browser smoke test

With the local server running and Playwright installed, run
`node examples/web/notes-smoke.mjs`. The test covers Chromium, Firefox
and WebKit, including reuse of tinted glyphs and partial redraws during
drag selection.
Pass engine names to select a subset. For an installation
outside this checkout, set `KUI_PLAYWRIGHT` to the absolute path to
Playwright's `index.mjs`. Set `KUI_CHROME_CHANNEL=chrome` to use installed
Google Chrome. Set `KUI_WEB_RENDERER=wgpu` to exercise the shared GPU
renderer. Run `node examples/web/touch-smoke.mjs` to test touch input
through Chromium's mobile emulation with both renderers. With a
WebGPU-capable Chrome, run `node examples/web/renderer-smoke.mjs` to
exercise failed startup and a failed first recovery attempt.
