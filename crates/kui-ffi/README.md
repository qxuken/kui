# kui-ffi

C API for kui: a cdylib plus `include/kui.h`, and `CExtension` for loading a C plugin into a Rust host.

kui splits a UI into a model that lays out and paints into a display list (`kui-core`), a renderer (`kui-wgpu`) and a windowed runner (`kui-native`). This crate puts the model, and optionally the runner, behind a flat C ABI: every `kui_*` function declared in `kui.h` is exported from `libkui_ffi`, and the `Kui*` structs are its `repr(C)` mirrors. A C, C++ or other-language host links the library and either hands a window to `kui_run` or drives its own event loop and renderer; a Rust host running `kui-native` loads a C shared library as a guest extension through `CExtension`. Strings cross as `(ptr, len)` UTF-8, values you build are yours until a call consumes them, every entry point catches panics, and a host checks `kui_abi_version()` against the header's `KUI_ABI_VERSION` first.

## Install

For a Rust host loading C plugins:

```sh
cargo add kui-ffi
```

For a C host, build the library and include the header from the repository:

```sh
cargo build -p kui-ffi --release                       # target/release/libkui_ffi.{so,dylib,dll}
cargo build -p kui-ffi --release --no-default-features --features access  # without the windowed runner
```

The `runner` feature (on by default) provides `kui_run` and `kui_run_with`; a host that owns its own window and renderer turns it off and ships less than half the library. The `access` feature (on by default) is the platform accessibility bridge such a host attaches to its window, `kui_access_attach`; without it that call answers false.

A program that links another Rust static library too (wgpu-native's `libwgpu_native.a`, say) links the shared `libkui_ffi`, not the archive: each Rust archive carries its own standard library, so two in one link collide at `rust_eh_personality`, and the runner's winit adds `RawWindowMetalLayer` on macOS. Turning the runner off drops the second, not the first.

## Example

A C host's frame loop without `kui_run` (the complete programs are under `examples/c/` in the repository):

```c
#include "kui.h"

if (kui_abi_version() != KUI_ABI_VERSION) return 1;
KuiCtx *ctx = kui_ctx_new();
for (;;) {
    kui_input_cursor(ctx, mouse_x, mouse_y);              /* input, logical px */
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ctx, &ev)) { /* read ev.payload with kui_value_get */ }

    kui_frame_begin(ctx, width, height, scale);           /* build the frame */
    KuiSpec root = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}};
    kui_root(ctx, &root);
    KuiTextStyle big = {.size = 56};
    kui_text(ctx, KUI_STR("Hello from C"), &big);
    kui_frame_finish(ctx);

    while (kui_poll_event(ctx, &ev)) { /* what the frame raised: resize, layout */ }

    KuiDrawData dd = KUI_DRAW_DATA_INIT;                   /* draw it */
    if (kui_draw_data(ctx, &dd)) draw_quads(dd.quads, dd.quad_count, dd.clips);
}
kui_ctx_free(ctx);
```

The two drains, which events only the second one sees, when to build again and when the draw can be skipped are kui.h's "The host loop" section.

A Rust app loading a C plugin:

```rust
use kui_ffi::CExtension;
use kui_native::{App, NodeSpec, Ui, Value};

struct Host;
impl App for Host {
    fn view(&mut self, ui: &mut Ui<'_>) {
        ui.open(NodeSpec::row().fill());
        // the plugin's `panel` component draws here (ADR 0067)
        ui.component("todos/panel", &Value::map([("title", "todos".into())]));
        // and whatever any loaded plugin contributes to this slot
        ui.slot("app.toolbar", &Value::Null);
        ui.close();
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let ext = unsafe { CExtension::open("target/debug/panel.so")? };
    kui_native::app("host").extension_as("todos", ext).run(Host)
}
```

## Where it fits

| Crate | What it is |
| --- | --- |
| [kui-core](https://crates.io/crates/kui-core) | The model: tree, layout, text, display list |
| [kui-wgpu](https://crates.io/crates/kui-wgpu) | The renderer |
| [kui-native](https://crates.io/crates/kui-native) | The windowed runner most apps use |
| [kui-derive](https://crates.io/crates/kui-derive) | `#[derive(Message)]` |
| [kui-lua](https://crates.io/crates/kui-lua) | Lua binding |
| [kui-ffi](https://crates.io/crates/kui-ffi) | This crate: the C API |

## Documentation

- API reference: <https://docs.rs/kui-ffi>
- The book: <https://kui-book.qxuken.dev>
- Repository: <https://github.com/qxuken/kui>

## License

MIT
