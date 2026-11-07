//! Drawables acquired off the UI thread, sharing the ordinary painter.
use super::*;
use objc2::rc::Retained;
use objc2_metal::{MTLCommandBuffer, MTLCommandQueue, MTLResource, MTLTexture, MTLTextureType};
use objc2_quartz_core::{CAMetalDrawable, CAMetalLayer};

impl Renderer {
    /// The configured Metal surface layer, if this renderer owns one.
    pub fn metal_layer(&self) -> Option<Retained<CAMetalLayer>> {
        // SAFETY: retain the live layer while holding the HAL guard. It is
        // neither reconfigured nor destroyed through this handle.
        let surface = unsafe { self.surface.as_ref()?.as_hal::<wgpu::hal::api::Metal>() }?;
        let layer = surface.render_layer().lock().clone();
        Some(layer)
    }

    /// Draws and presents a drawable acquired from this surface’s Metal layer.
    ///
    /// # Safety
    /// The drawable must come from `metal_layer()` on this renderer, must
    /// not have been presented, and must not be used by another encoder.
    pub unsafe fn render_metal_drawable(
        &mut self,
        dl: &DisplayList,
        atlas: &mut GlyphAtlas,
        drawable: &objc2::runtime::ProtocolObject<dyn CAMetalDrawable>,
    ) -> Result<RenderReport, RenderError> {
        if self.gpu.lost() {
            return Err(RenderError::DeviceLost);
        }
        let _ = self.gpu.device().poll(wgpu::PollType::Poll);
        let raw = drawable.texture();
        let device = unsafe { self.gpu.device().as_hal::<wgpu::hal::api::Metal>() }
            .ok_or(RenderError::DeviceLost)?;
        if Retained::as_ptr(&raw.device()) != Retained::as_ptr(device.raw_device()) {
            return Err(RenderError::DeviceLost);
        }
        drop(device);
        let layer = self.metal_layer().ok_or(RenderError::Reconfigure)?;
        if raw.pixelFormat() != layer.pixelFormat()
            || raw.textureType() != MTLTextureType::Type2D
            || raw.mipmapLevelCount() != 1
            || raw.sampleCount() != 1
        {
            return Err(RenderError::Validation);
        }
        let size = wgpu::Extent3d {
            width: raw.width() as u32,
            height: raw.height() as u32,
            depth_or_array_layers: 1,
        };
        if size.width != self.config.width || size.height != self.config.height {
            return Err(RenderError::Reconfigure);
        }
        let desc = wgpu::TextureDescriptor {
            label: Some("kui acquired drawable"),
            size,
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        };
        // SAFETY: the layer uses this renderer's device/format and supplies
        // a single-level 2D drawable. The clear discards its old contents.
        let texture = unsafe {
            let hal = wgpu::hal::metal::Device::texture_from_raw(
                raw,
                self.config.format,
                MTLTextureType::Type2D,
                1,
                1,
                size.into(),
                None,
            );
            self.gpu
                .device()
                .create_texture_from_hal::<wgpu::hal::api::Metal>(
                    hal,
                    &desc,
                    wgpu::TextureUses::UNINITIALIZED,
                )
        };
        self.sync_atlas(atlas, true);
        atlas.mark_uploaded();
        let prep = self.prepare(dl, true);
        let view = texture.create_view(&Default::default());
        let label = self.frame_label();
        let mut encoder =
            self.gpu
                .device()
                .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                    label: label.as_deref(),
                });
        self.draw(&mut encoder, dl, &prep, &view);
        self.submit(encoder.finish());
        // SAFETY: the rendering work has been committed on this same queue.
        // Match wgpu HAL's presentation ordering without acquiring again.
        let queue = unsafe { self.gpu.queue().as_hal::<wgpu::hal::api::Metal>() }
            .ok_or(RenderError::DeviceLost)?;
        let present = queue
            .as_raw()
            .commandBuffer()
            .ok_or(RenderError::DeviceLost)?;
        present.presentDrawable(objc2::runtime::ProtocolObject::from_ref(drawable));
        present.commit();
        Ok(RenderReport { vsync_wait_ms: 0.0 })
    }
}
