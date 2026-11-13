# kui-web

A single-canvas browser host for [kui](https://github.com/qxuken/kui).
The app and its core stay in WebAssembly; the browser supplies input,
clipboard access, an accessibility mirror and a canvas.

Implement `App` and create a `Runner` with a `kui_core::Session` containing
your bundled fonts. The host delivers input with browser timestamps and
calls `frame` to draw through Canvas 2D. With the `rust-wgpu` feature,
attach a `WgpuPainter` and call `frame_wgpu` to use the shared `kui-wgpu`
renderer through WebGPU. This crate targets `wasm32`; its API is absent
on native targets.

The package includes `canvas.js` and `editor.js`, the browser input,
drawing and textarea bridges. An app supplies its Wasm exports and loads
the bridge alongside its Wasm artifact. See the complete
[counter and notes examples](https://github.com/qxuken/kui/tree/main/examples/web)
for that host ABI, build commands and browser smoke tests.

Features:

- `rust-wgpu`: the browser WebGPU painter and wasm-bindgen callbacks.
- `svg`: SVG support in the core.
- `devtools`: the core's inspection support.
- `packed-gpu`: the packed frame protocol used by the prebuilt browser runner.

To run an existing Lua app in a browser, use the separately shipped
`kui pack <app> --format web` command. The `kui-web-lua` runner remains
an internal workspace crate.
