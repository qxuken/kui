//! A core's finished frame as pixels (backlog C55, ADR 0054): what a
//! window shows, read back for a test, a screenshot or a smoke run, from
//! a window's core or a headless one alike.
//!
//! The frame is drawn again, from the core's display list, by a renderer
//! of the process's own on a device with no window ([`kui_wgpu::Gpu::windowless`]),
//! opened by the first capture and kept for the next. Not the window's
//! swapchain: that is a render attachment only, and making it readable
//! would cost every frame for a rare capture. So a headless core, which
//! never had a renderer, captures the same way a window's does.
//!
//! What the window shows and the capture does not: what the OS draws
//! behind a translucent window (a backdrop's material, the desktop), and
//! the wallpaper kui draws where there is no material. The capture is
//! cleared to the theme's `bg`, which is what an opaque window is cleared
//! to.

use kui_core::Core;
use kui_wgpu::wgpu;

/// A frame read back by [`capture`]: `width` by `height` physical pixels,
/// four bytes each (RGBA), row by row from the top left, alpha not
/// premultiplied — the shape `Core::add_image` and [`crate::decode_image`]
/// use — and the scale the frame was drawn at.
#[derive(Clone, Debug, PartialEq)]
pub struct Capture {
    pub width: u32,
    pub height: u32,
    pub scale: f32,
    pub rgba: Vec<u8>,
}

/// The process's capture renderer, opened by the first capture. A
/// `Mutex` because a capture may come from any thread a binding calls on;
/// one at a time is what a GPU readback is anyway.
static RENDERER: std::sync::Mutex<Option<kui_wgpu::Renderer>> = std::sync::Mutex::new(None);

/// The core's last finished frame, drawn again and read back.
///
/// Call it between frames: while one builds, the list is the one the
/// frame began by emptying. The first capture in a process opens a GPU
/// device of its own and builds its pipelines: about 40–100 ms on an M3
/// Pro. Later ones cost the draw, the wait for the GPU and the 30 MB copy
/// out: about 10 ms for a 3456 by 2234 frame. The window's own renderer is
/// not touched: its atlas changes stay its to upload.
///
/// Fails, saying why, for a core that has not finished a frame, a frame
/// larger than the device's largest texture, or a machine with no GPU
/// adapter (a container with no software Vulkan); after a lost device
/// the next capture opens a new one.
pub fn capture(core: &Core) -> Result<Capture, String> {
    let (dl, atlas) = core.drawn();
    if dl.viewport.w < 1.0 || dl.viewport.h < 1.0 {
        return Err("no finished frame to capture".into());
    }
    let mut slot = RENDERER.lock().unwrap_or_else(|e| e.into_inner());
    if slot.as_ref().is_some_and(|r| r.gpu().lost()) {
        *slot = None;
    }
    if slot.is_none() {
        let gpu = pollster::block_on(kui_wgpu::Gpu::windowless())
            .map_err(|e| format!("no GPU to draw the frame on: {e}"))?;
        let r = kui_wgpu::Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm)
            .map_err(|e| format!("no renderer to draw the frame with: {e}"))?;
        *slot = Some(r);
    }
    let r = slot.as_mut().expect("opened above");
    // What the runner clears an opaque window to (ADR 0019).
    let bg = core.theme().bg;
    r.clear_color = wgpu::Color {
        r: bg.r as f64,
        g: bg.g as f64,
        b: bg.b as f64,
        a: bg.a as f64,
    };
    match r.capture(dl, atlas) {
        Ok(c) => Ok(Capture {
            width: c.width,
            height: c.height,
            scale: dl.scale,
            rgba: c.rgba,
        }),
        Err(kui_wgpu::RenderError::DeviceLost) => {
            *slot = None;
            Err("the GPU device was lost; the next capture opens another".into())
        }
        Err(_) => Err(format!(
            "the device refused a {} by {} frame (its largest texture is {})",
            dl.viewport.w,
            dl.viewport.h,
            r.gpu().device().limits().max_texture_dimension_2d
        )),
    }
}
