use kui_core::Core;
use kui_wgpu::{RenderError, Renderer, wgpu};
use web_sys::HtmlCanvasElement;

/// The browser surface uses the same renderer as a desktop window.
pub struct WgpuPainter {
    renderer: Renderer,
    backing_size: [u32; 2],
}

impl WgpuPainter {
    pub async fn new(canvas: HtmlCanvasElement) -> Result<Self, Box<dyn std::error::Error>> {
        let [width, height] = [canvas.width(), canvas.height()];
        let renderer = Renderer::new(wgpu::SurfaceTarget::Canvas(canvas), width, height).await?;
        Ok(Self {
            renderer,
            backing_size: [width, height],
        })
    }

    pub(crate) fn paint(
        &mut self,
        core: &mut Core,
        backing_size: [u32; 2],
    ) -> Result<bool, RenderError> {
        let [width, height] = backing_size;
        if self.backing_size != backing_size {
            self.renderer.resize(width, height);
            self.backing_size = backing_size;
        }
        let bg = core.theme().bg;
        self.renderer.clear_color = wgpu::Color {
            r: bg.r.into(),
            g: bg.g.into(),
            b: bg.b.into(),
            a: bg.a.into(),
        };
        let (list, atlas) = core.output();
        match self.renderer.render(list, atlas) {
            Ok(_) => Ok(false),
            Err(RenderError::Skip) => Ok(true),
            Err(RenderError::Reconfigure) => {
                self.renderer.resize(width, height);
                Ok(true)
            }
            Err(err) => Err(err),
        }
    }
}
