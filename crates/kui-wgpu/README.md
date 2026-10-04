# kui-wgpu

wgpu renderer for kui: draws a `kui_core::DisplayList` with one instanced pipeline, a single draw call per frame.

kui splits a UI into a model that lays out and paints into a display list (`kui-core`), a renderer that puts that list on screen (this crate) and a runner that owns the window and the event loop (`kui-native`, the one most apps use). Reach for `kui-wgpu` directly when you are writing your own runner: you already have a window, or an event loop, that `kui-native` does not fit. A `Renderer` owns one window's swapchain and a GPU copy of the core's glyph atlas; several windows share one device through `Gpu`.

## Install

```sh
cargo add kui-wgpu kui-core pollster
```

`wgpu` is re-exported as `kui_wgpu::wgpu`, so you build against the version this crate was.

## Example

Open a renderer on a window, then build, draw and present one frame at a time. `window` is anything wgpu can make a surface from, such as a `winit` window.

```rust
use kui_core::{Core, Size, TextStyle};
use kui_wgpu::{RenderError, Renderer};

fn run(
    window: impl Into<kui_wgpu::wgpu::SurfaceTarget<'static>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let (width, height) = (800u32, 600u32);
    let mut renderer = pollster::block_on(Renderer::new(window, width, height))?;
    let mut core = Core::new();
    core.set_subpixel_text(renderer.subpixel_text());

    loop {
        // On a resize event: renderer.resize(new_width, new_height);
        let scale = 1.0;
        let mut ui = core.frame(Size::new(width as f32, height as f32), scale);
        ui.text("Hello from a custom runner", TextStyle::new(24.0));
        ui.finish();

        let (list, atlas) = core.output();
        match renderer.render(list, atlas) {
            Ok(_report) => {}
            Err(RenderError::Reconfigure | RenderError::Validation) => {
                renderer.resize(width, height);
            }
            Err(RenderError::Skip) => {}
            Err(RenderError::DeviceLost) => break, // open a new Renderer
        }
    }
    Ok(())
}
```

## Where it fits

| Crate | What it is |
| --- | --- |
| [kui-core](https://crates.io/crates/kui-core) | The model: tree, layout, text, display list |
| [kui-wgpu](https://crates.io/crates/kui-wgpu) | This crate: the renderer |
| [kui-native](https://crates.io/crates/kui-native) | The windowed runner most apps use |
| [kui-derive](https://crates.io/crates/kui-derive) | `#[derive(Message)]` |
| [kui-lua](https://crates.io/crates/kui-lua), [kui-ffi](https://crates.io/crates/kui-ffi) | Lua and C bindings |

## Documentation

- API reference: <https://docs.rs/kui-wgpu>
- The book: <https://kui-book.qxuken.dev>
- Repository: <https://github.com/qxuken/kui>

## License

MIT
