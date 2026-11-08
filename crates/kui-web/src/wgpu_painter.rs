use kui_core::{Color, Core};
use kui_wgpu::{RenderError, RenderReport, Renderer, wgpu};
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
        self.configure(core.theme().bg, backing_size);
        let (list, atlas) = core.output();
        let result = self.renderer.render(list, atlas);
        self.finish(result)
    }

    #[cfg(feature = "packed-gpu")]
    pub(crate) fn paint_uploaded(
        &mut self,
        list: &kui_core::DisplayList,
        bg: Color,
        backing_size: [u32; 2],
    ) -> Result<bool, RenderError> {
        self.configure(bg, backing_size);
        let result = self.renderer.render_with_uploaded_atlas(list);
        self.finish(result)
    }

    #[cfg(feature = "packed-gpu")]
    pub(crate) fn on_device_lost(&self, notify: impl Fn(String) + Send + 'static) {
        self.renderer
            .gpu()
            .device()
            .set_device_lost_callback(move |_, message| notify(message));
    }

    #[cfg(feature = "packed-gpu")]
    pub(crate) fn upload_atlas(
        &mut self,
        size: u32,
        rect: [u32; 4],
        rgba: &[u8],
    ) -> Result<(), RenderError> {
        self.renderer.upload_atlas_region(size, rect, rgba)
    }

    fn configure(&mut self, bg: Color, backing_size: [u32; 2]) {
        let [width, height] = backing_size;
        if self.backing_size != backing_size {
            self.renderer.resize(width, height);
            self.backing_size = backing_size;
        }
        self.renderer.clear_color = wgpu::Color {
            r: bg.r.into(),
            g: bg.g.into(),
            b: bg.b.into(),
            a: bg.a.into(),
        };
    }

    fn finish(&mut self, result: Result<RenderReport, RenderError>) -> Result<bool, RenderError> {
        match result {
            Ok(_) => Ok(false),
            Err(RenderError::Skip) => Ok(true),
            Err(RenderError::Reconfigure) => {
                let [width, height] = self.backing_size;
                self.renderer.resize(width, height);
                Ok(true)
            }
            Err(err) => Err(err),
        }
    }
}
