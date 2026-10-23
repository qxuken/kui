//! A frame drawn with no window (backlog C55): a windowless renderer on a
//! windowless device reads back what a window would show, and draws into
//! a texture a host owns. On the machine's GPU; skipped, saying so, where
//! there is no adapter.

use kui_core::{Color, Core, NodeSpec, Size, TextStyle};
use kui_wgpu::{Gpu, RenderError, Renderer, wgpu};

fn gpu() -> Option<Gpu> {
    match pollster::block_on(Gpu::windowless()) {
        Ok(g) => {
            required(g.adapter());
            Some(g)
        }
        Err(e) => {
            skip(&format!("no adapter ({e})"));
            None
        }
    }
}

/// What `KUI_REQUIRE_GPU` asks of the adapter a test found: a name
/// containing its value, unless that is `1`.
fn required(adapter: &wgpu::Adapter) {
    let name = adapter.get_info().name;
    if let Ok(want) = std::env::var("KUI_REQUIRE_GPU")
        && !want.is_empty()
        && want != "1"
    {
        assert!(
            name.contains(&want),
            "KUI_REQUIRE_GPU={want}, but the adapter is {name}"
        );
    }
}

/// A skip, saying why, or a failure where `KUI_REQUIRE_GPU` is set.
fn skip(why: &str) {
    if std::env::var_os("KUI_REQUIRE_GPU").is_some_and(|v| !v.is_empty()) {
        panic!("KUI_REQUIRE_GPU is set, but {why}");
    }
    eprintln!("skipped: {why}");
}

/// A 40x30 frame at scale 2: a blue root padded 5, a red 10x10 box in
/// its corner, and a word, so the atlas has something on it.
fn frame(core: &mut Core) {
    let mut ui = core.frame(Size::new(40.0, 30.0), 2.0);
    ui.configure_root(
        NodeSpec::column()
            .fill()
            .pad(5.0)
            .bg(Color::hex(0x0000ffff)),
    );
    ui.leaf_keyed(
        "red",
        NodeSpec::column()
            .size(10.0, 10.0)
            .bg(Color::hex(0xff0000ff)),
    );
    ui.text("Hi", TextStyle::new(8.0));
    ui.finish();
}

fn at(rgba: &[u8], width: u32, x: u32, y: u32) -> [u8; 4] {
    let o = ((y * width + x) * 4) as usize;
    rgba[o..o + 4].try_into().unwrap()
}

#[test]
fn a_capture_is_the_frame_in_physical_px_as_rgba() {
    let Some(gpu) = gpu() else { return };
    let mut core = Core::new();
    core.set_subpixel_text(gpu.dual_source());
    frame(&mut core);
    for format in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Bgra8Unorm,
    ] {
        let mut r = Renderer::windowless(&gpu, format).expect("a renderer");
        let (dl, atlas) = core.output();
        let c = r.capture(dl, atlas).expect("a capture");
        assert_eq!((c.width, c.height), (80, 60), "{format:?}: 40x30 at 2x");
        assert_eq!(c.rgba.len(), 80 * 60 * 4);
        // The box is (10, 10)..(30, 30) in physical px, the root around it.
        assert_eq!(
            at(&c.rgba, 80, 20, 20),
            [255, 0, 0, 255],
            "{format:?}: red, as RGBA"
        );
        assert_eq!(
            at(&c.rgba, 80, 70, 55),
            [0, 0, 255, 255],
            "{format:?}: the root's blue"
        );
    }
}

#[test]
fn a_translucent_frame_comes_back_straight() {
    let Some(gpu) = gpu() else { return };
    let mut core = Core::new();
    let mut ui = core.frame(Size::new(20.0, 20.0), 1.0);
    ui.configure_root(NodeSpec::column().fill().bg(Color::hex(0xff000080)));
    ui.finish();
    let mut r = Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm).expect("a renderer");
    r.clear_color = wgpu::Color::TRANSPARENT;
    let (dl, atlas) = core.output();
    let c = r.capture(dl, atlas).expect("a capture");
    let [red, g, b, a] = at(&c.rgba, 20, 10, 10);
    assert!((127..=129).contains(&a), "half alpha: {a}");
    assert!(
        red >= 253 && g == 0 && b == 0,
        "full red, not premultiplied: {red} {g} {b}"
    );
}

/// The windowless renderer beside a window's must not take the atlas's
/// changes from it: a capture leaves `dirty` as it found it, and draws
/// the glyphs anyway.
#[test]
fn a_capture_leaves_the_atlas_dirty_for_the_core_s_own_renderer() {
    let Some(gpu) = gpu() else { return };
    let mut core = Core::new();
    frame(&mut core);
    let (dl, atlas) = core.output();
    assert!(atlas.dirty, "the first frame rasterized a word");
    let mut r = Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm).expect("a renderer");
    r.capture(dl, atlas).expect("a capture");
    assert!(atlas.dirty, "still the window renderer's to upload");
}

/// A host's texture: drawn into, nothing presented, the atlas taken as
/// the core's own renderer takes it.
#[test]
fn render_to_draws_into_the_host_s_texture() {
    let Some(gpu) = gpu() else { return };
    let mut core = Core::new();
    frame(&mut core);
    let format = wgpu::TextureFormat::Rgba8Unorm;
    let mut r = Renderer::windowless(&gpu, format).expect("a renderer");
    let device = gpu.device();
    let target = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("host"),
        size: wgpu::Extent3d {
            width: 80,
            height: 60,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let (dl, atlas) = core.output();
    r.render_to(dl, atlas, &target).expect("drawn");
    assert!(!atlas.dirty, "uploaded and cleared, as render does");

    let buf = device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 512 * 60,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut enc = device.create_command_encoder(&Default::default());
    enc.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &buf,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(512),
                rows_per_image: Some(60),
            },
        },
        target.size(),
    );
    gpu.queue().submit([enc.finish()]);
    buf.slice(..).map_async(wgpu::MapMode::Read, |_| {});
    let _ = device.poll(wgpu::PollType::Wait {
        submission_index: None,
        timeout: None,
    });
    let data = buf.slice(..).get_mapped_range().expect("mapped");
    let px = |x: usize, y: usize| &data[y * 512 + x * 4..y * 512 + x * 4 + 4];
    assert_eq!(px(20, 20), [255, 0, 0, 255]);
    assert_eq!(px(70, 55), [0, 0, 255, 255]);
    drop(data);

    // Another format is refused, not drawn into.
    let other = device.create_texture(&wgpu::TextureDescriptor {
        label: Some("bgra"),
        format: wgpu::TextureFormat::Bgra8Unorm,
        ..target_desc(80, 60)
    });
    let (dl, atlas) = core.output();
    assert!(matches!(
        r.render_to(dl, atlas, &other),
        Err(RenderError::Validation)
    ));
}

#[test]
fn a_list_with_no_viewport_is_refused() {
    let Some(gpu) = gpu() else { return };
    let mut core = Core::new();
    let mut r = Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm).expect("a renderer");
    let (dl, atlas) = core.output();
    assert!(matches!(r.capture(dl, atlas), Err(RenderError::Validation)));
    // And a windowless renderer has nothing to present.
    assert!(matches!(r.render(dl, atlas), Err(RenderError::Skip)));
}

fn target_desc(width: u32, height: u32) -> wgpu::TextureDescriptor<'static> {
    wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    }
}

/// A device the host opened itself, handed to kui whole: the renderer on
/// it draws into the host's texture.
#[test]
fn a_host_s_own_device_takes_a_windowless_renderer() {
    let instance =
        wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle_from_env());
    let Ok(adapter) =
        pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))
    else {
        skip("no adapter");
        return;
    };
    required(&adapter);
    let (device, queue) =
        pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor::default()))
            .expect("a device");
    let gpu = Gpu::from_parts(instance, adapter, device.clone(), queue);
    assert!(
        !gpu.dual_source(),
        "the host enabled no dual-source blending"
    );
    let mut core = Core::new();
    frame(&mut core);
    let mut r = Renderer::windowless(&gpu, wgpu::TextureFormat::Rgba8Unorm).expect("a renderer");
    let (dl, atlas) = core.output();
    let c = r.capture(dl, atlas).expect("a capture");
    assert_eq!(at(&c.rgba, 80, 20, 20), [255, 0, 0, 255]);
    let target = device.create_texture(&target_desc(80, 60));
    let (dl, atlas) = core.output();
    r.render_to(dl, atlas, &target)
        .expect("into the host's texture");
}
