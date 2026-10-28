// A host that draws kui's frame with a GPU device of its own (backlog
// F234), on the wgpu-native Odin ships as vendor:wgpu: kui.h's "Drawing it
// yourself" end to end. Nothing here knows kui's instance layout or shader:
// the WGSL is kui.renderer_wgsl, the vertex buffer's layout is read from
// kui.instance_stride and kui.instance_attributes, the instances are
// kui.pack_instances. A later library that moves any of them needs no
// change here. What this file does know is what kui.h states and kui-wgpu
// does (crates/kui-wgpu/src/lib.rs): the bind group, the blend, a target
// whose bytes are kui's colours as written (an Unorm format, no sRGB).
//
// Two modes. Offscreen (the default, and --headless): no window and no
// surface; the frame is drawn into a texture, read back and checked - the
// panel's colour, the clear colour around it, the shadow, the title's
// glyphs, the button - then the button is clicked and the frame drawn
// again, the label's new glyph uploaded as the atlas's dirty rect alone.
// examples/c/tools/wgpu_host.c is the same in C. With no GPU adapter it
// says so and exits 0, unless KUI_REQUIRE_GPU is set (CI sets it over
// lavapipe).
//
// --window: a GLFW window whose "world" is the clear colour, kui drawn over
// it every frame in kui.h's host-loop order (input, drain, build, drain,
// draw). The button counts clicks; a click on the world - where
// kui.pointer_wants says the pointer is not the UI's - changes its colour;
// keys reach kui through kui/glfw; Escape, when kui.keyboard_wants says the
// keyboard is not the UI's, closes it. --frames N quits after N frames.
//
//   nu scripts/odin.nu run wgpu_host                  offscreen, exit 0 on a clean draw
//   nu scripts/odin.nu run wgpu_host -- --window      the window
//   nu scripts/odin.nu test                           runs it with --headless
//
// It needs vendor:wgpu's library for this platform; Odin ships one for
// macOS and Windows, and scripts/odin.nu skips this example where the
// compiler has none (Linux, unless one is unpacked there, as CI does).
package wgpu_host

import "base:runtime"
import "core:fmt"
import "core:os"
import "core:strconv"
import "core:strings"
import "vendor:glfw"
import "vendor:wgpu"
import "vendor:wgpu/glfwglue"
import kui "../../../packages/odin/kui"
import kui_glfw "../../../packages/odin/kui/glfw"

// The frame, in logical px; offscreen it is drawn at twice that.
W :: 320
H :: 200
SCALE :: 2
PANEL_X :: 20
PANEL_Y :: 20
PANEL_W :: 200
PANEL_H :: 120
BUTTON_W :: 112
BUTTON_H :: 28
PANEL_BG :: kui.Color(0x2a3550ff)
BUTTON_BG :: kui.Color(0xe0703aff)

// The worlds a click on them cycles through; the first is the offscreen
// clear colour the checks expect.
WORLDS := [?]wgpu.Color{{0.10, 0.55, 0.35, 1}, {0.55, 0.20, 0.30, 1}, {0.15, 0.30, 0.60, 1}, {0.60, 0.50, 0.15, 1}}

// -- the view -----------------------------------------------------------------

Count :: struct {}

State :: struct {
	clicks: int,
	world:  int,
}

// A panel with a shadow, a title and a button, on a transparent root.
view :: proc(s: ^State, ui: ^kui.Ui) {
	if kui.box(ui, {pad = {l = PANEL_X, t = PANEL_Y}}) {
		panel := kui.Spec {
			width        = kui.px(PANEL_W),
			height       = kui.px(PANEL_H),
			dir          = .Column,
			pad          = kui.pad(16),
			gap          = 12,
			bg           = PANEL_BG,
			radius       = 8,
			shadow_color = 0x00000099,
			shadow_blur  = 10,
			shadow_y     = 4,
		}
		if kui.box(ui, panel) {
			kui.text(ui, "kui over wgpu", {size = 18, color = 0xffffffff})
			button := kui.Spec {
				width       = kui.px(BUTTON_W),
				height      = kui.px(BUTTON_H),
				main_align  = .Center,
				cross_align = .Center,
				bg          = BUTTON_BG,
				radius      = 6,
				label       = "count",
				on_click    = Count{},
			}
			if kui.box(ui, button) {
				kui.text(ui, fmt.tprintf("clicked %d", s.clicks), {size = 13, color = 0xffffffff})
			}
		}
	}
}

// Drains the queue into the model; whether anything changed.
drain :: proc(ui: ^kui.Ui, s: ^State) -> (changed: bool) {
	for ev in kui.poll_event(ui) {
		if ev.event == "click" {
			if _, ok := kui.message(ev, Count); ok {
				s.clicks += 1
				changed = true
			}
		}
	}
	return
}

// -- the device ---------------------------------------------------------------

Gpu :: struct {
	instance: wgpu.Instance,
	adapter:  wgpu.Adapter,
	device:   wgpu.Device,
	queue:    wgpu.Queue,
	name:     string,
}

// Every validation error the device raises: a failure, since a reference
// host that draws with one has drawn nothing worth checking.
wgpu_errors := 0

Answer :: struct {
	done:   bool,
	ok:     bool,
	handle: rawptr,
}

// wgpu-native answers a request inside the call; the loop is for an
// implementation that answers later, through the instance's events.
wait_for :: proc(instance: wgpu.Instance, a: ^Answer) {
	for i := 0; !a.done && i < 10000; i += 1 do wgpu.InstanceProcessEvents(instance)
}

// An adapter (one that can present to `surface`, when there is one) and a
// device. False with no adapter: the machine has no GPU, or no driver.
gpu_open :: proc(g: ^Gpu, surface: wgpu.Surface = nil) -> bool {
	if g.instance == nil do g.instance = wgpu.CreateInstance(nil)
	if g.instance == nil do return false
	a: Answer
	on_adapter :: proc "c" (status: wgpu.RequestAdapterStatus, adapter: wgpu.Adapter, message: string, u1, _: rawptr) {
		context = runtime.default_context()
		a := (^Answer)(u1)
		a.done, a.ok, a.handle = true, status == .Success && adapter != nil, rawptr(adapter)
		if !a.ok do fmt.eprintln("adapter:", message)
	}
	wgpu.InstanceRequestAdapter(g.instance, &{compatibleSurface = surface}, {mode = .AllowProcessEvents, callback = on_adapter, userdata1 = &a})
	wait_for(g.instance, &a)
	if !a.ok do return false
	g.adapter = wgpu.Adapter(a.handle)
	if info, status := wgpu.AdapterGetInfo(g.adapter); status == .Success {
		g.name = strings.clone(info.device)
		wgpu.AdapterInfoFreeMembers(info)
	}

	on_error :: proc "c" (_: ^wgpu.Device, type: wgpu.ErrorType, message: string, _, _: rawptr) {
		context = runtime.default_context()
		fmt.eprintfln("FAIL: wgpu error (%v): %s", type, message)
		wgpu_errors += 1
	}
	on_device :: proc "c" (status: wgpu.RequestDeviceStatus, device: wgpu.Device, message: string, u1, _: rawptr) {
		context = runtime.default_context()
		d := (^Answer)(u1)
		d.done, d.ok, d.handle = true, status == .Success && device != nil, rawptr(device)
		if !d.ok do fmt.eprintln("device:", message)
	}
	d: Answer
	wgpu.AdapterRequestDevice(g.adapter, &{uncapturedErrorCallbackInfo = {callback = on_error}}, {mode = .AllowProcessEvents, callback = on_device, userdata1 = &d})
	wait_for(g.instance, &d)
	if !d.ok do return false
	g.device = wgpu.Device(d.handle)
	g.queue = wgpu.DeviceGetQueue(g.device)
	return true
}

gpu_close :: proc(g: ^Gpu) {
	if g.queue != nil do wgpu.QueueRelease(g.queue)
	if g.device != nil do wgpu.DeviceRelease(g.device)
	if g.adapter != nil do wgpu.AdapterRelease(g.adapter)
	if g.instance != nil do wgpu.InstanceRelease(g.instance)
	delete(g.name)
}

// -- kui's renderer, built from what the library says -------------------------

Renderer :: struct {
	layout:       wgpu.BindGroupLayout,
	pipeline:     wgpu.RenderPipeline,
	globals:      wgpu.Buffer,
	linear:       wgpu.Sampler,
	nearest:      wgpu.Sampler,
	// The glyph atlas, kept between frames: a page the size the draw data
	// says, and the epoch of the page it holds.
	atlas:        wgpu.Texture,
	atlas_view:   wgpu.TextureView,
	atlas_size:   u32,
	atlas_epoch:  u64,
	group:        wgpu.BindGroup,
	instances:    wgpu.Buffer,
	instance_cap: int,
	// What the last frame uploaded of the atlas: x, y, w, h.
	uploaded:     [4]u32,
}

renderer_init :: proc(g: ^Gpu, r: ^Renderer, format: wgpu.TextureFormat) {
	// The shader without dual-source blending: subpixel text stays off (it
	// is unless kui.set_subpixel_text), so no glyph needs per-channel
	// blending.
	module := wgpu.DeviceCreateShaderModule(g.device, &{nextInChain = &wgpu.ShaderSourceWGSL{sType = .ShaderSourceWGSL, code = kui.renderer_wgsl()}})
	defer wgpu.ShaderModuleRelease(module)

	// Group 0 as kui.h gives it: the globals, the atlas, two samplers.
	entries := [4]wgpu.BindGroupLayoutEntry {
		{binding = 0, visibility = {.Vertex, .Fragment}, buffer = {type = .Uniform}},
		{binding = 1, visibility = {.Fragment}, texture = {sampleType = .Float, viewDimension = ._2D}},
		{binding = 2, visibility = {.Fragment}, sampler = {type = .Filtering}},
		{binding = 3, visibility = {.Fragment}, sampler = {type = .Filtering}},
	}
	r.layout = wgpu.DeviceCreateBindGroupLayout(g.device, &{entryCount = len(entries), entries = &entries[0]})
	pipeline_layout := wgpu.DeviceCreatePipelineLayout(g.device, &{bindGroupLayoutCount = 1, bindGroupLayouts = &r.layout})
	defer wgpu.PipelineLayoutRelease(pipeline_layout)

	// The instance layout, read from the library at run time: each
	// attribute a float32x2 or float32x4 at its location and offset,
	// stepped per instance.
	kui_attrs := kui.instance_attributes()
	attrs := make([]wgpu.VertexAttribute, len(kui_attrs), context.temp_allocator)
	for a, i in kui_attrs {
		attrs[i] = {format = .Float32x2 if a.floats == 2 else .Float32x4, offset = u64(a.offset), shaderLocation = a.location}
	}
	// The single variant writes straight colour and its coverage as alpha:
	// blended as kui-wgpu blends it, colour by its alpha, alpha over.
	blend := wgpu.BlendState {
		color = {srcFactor = .SrcAlpha, dstFactor = .OneMinusSrcAlpha, operation = .Add},
		alpha = {srcFactor = .One, dstFactor = .OneMinusSrcAlpha, operation = .Add},
	}
	r.pipeline = wgpu.DeviceCreateRenderPipeline(g.device, &{
		layout = pipeline_layout,
		vertex = {
			module = module,
			entryPoint = "vs_main",
			bufferCount = 1,
			buffers = &wgpu.VertexBufferLayout {
				stepMode = .Instance,
				arrayStride = u64(kui.instance_stride()),
				attributeCount = len(attrs),
				attributes = raw_data(attrs),
			},
		},
		fragment = &{
			module = module,
			entryPoint = "fs_main",
			targetCount = 1,
			targets = &wgpu.ColorTargetState{format = format, blend = &blend, writeMask = wgpu.ColorWriteMaskFlags_All},
		},
		// Six vertices an instance, a triangle list.
		primitive = {topology = .TriangleList},
		multisample = {count = 1, mask = 0xFFFFFFFF},
	})

	// Eight floats: viewport, atlas size, frame clock, scale, padding.
	r.globals = wgpu.DeviceCreateBuffer(g.device, &{usage = {.Uniform, .CopyDst}, size = 8 * size_of(f32)})
	// kui-wgpu's two samplers: clamped, one filtering and one not.
	r.linear = wgpu.DeviceCreateSampler(g.device, &{
		addressModeU = .ClampToEdge, addressModeV = .ClampToEdge, addressModeW = .ClampToEdge,
		magFilter = .Linear, minFilter = .Linear, mipmapFilter = .Nearest, lodMaxClamp = 32, maxAnisotropy = 1,
	})
	r.nearest = wgpu.DeviceCreateSampler(g.device, &{
		addressModeU = .ClampToEdge, addressModeV = .ClampToEdge, addressModeW = .ClampToEdge,
		magFilter = .Nearest, minFilter = .Nearest, mipmapFilter = .Nearest, lodMaxClamp = 32, maxAnisotropy = 1,
	})
}

// A page of `size`, and the bind group that names it.
atlas_resize :: proc(g: ^Gpu, r: ^Renderer, size: u32) {
	if r.atlas != nil {
		wgpu.BindGroupRelease(r.group)
		wgpu.TextureViewRelease(r.atlas_view)
		wgpu.TextureRelease(r.atlas)
	}
	r.atlas = wgpu.DeviceCreateTexture(g.device, &{
		usage = {.TextureBinding, .CopyDst},
		dimension = ._2D,
		size = {size, size, 1},
		format = .RGBA8Unorm,
		mipLevelCount = 1,
		sampleCount = 1,
	})
	r.atlas_view = wgpu.TextureCreateView(r.atlas, nil)
	r.atlas_size, r.atlas_epoch = size, 0
	entries := [4]wgpu.BindGroupEntry {
		{binding = 0, buffer = r.globals, size = 8 * size_of(f32)},
		{binding = 1, textureView = r.atlas_view},
		{binding = 2, sampler = r.linear},
		{binding = 3, sampler = r.nearest},
	}
	r.group = wgpu.DeviceCreateBindGroup(g.device, &{layout = r.layout, entryCount = len(entries), entries = &entries[0]})
}

// The atlas: whole when the page is new to us, else the dirty rect alone.
// The rect's rows are atlas_pixels' own, so the source starts at its corner
// with the page's stride.
atlas_upload :: proc(g: ^Gpu, r: ^Renderer, dd: kui.Draw_Data) {
	size := max(dd.atlas_size, 1)
	if r.atlas == nil || size != r.atlas_size do atlas_resize(g, r, size)
	r.uploaded = {}
	if dd.atlas_size == 0 || dd.atlas_pixels == nil do return
	x, y, w, h := u32(0), u32(0), size, size
	if dd.atlas_epoch == r.atlas_epoch {
		if !dd.atlas_dirty do return
		x, y, w, h = dd.atlas_dirty_rect[0], dd.atlas_dirty_rect[1], dd.atlas_dirty_rect[2], dd.atlas_dirty_rect[3]
		if w == 0 || h == 0 do return
	}
	from := (uint(y) * uint(size) + uint(x)) * 4
	bytes := uint(h - 1) * uint(size) * 4 + uint(w) * 4
	wgpu.QueueWriteTexture(g.queue, &{texture = r.atlas, aspect = .All, origin = {x, y, 0}}, &dd.atlas_pixels[from], bytes, &{bytesPerRow = size * 4, rowsPerImage = h}, &{w, h, 1})
	r.atlas_epoch = dd.atlas_epoch
	r.uploaded = {x, y, w, h}
}

// kui's last finished frame into `target`, cleared to `world` first, in
// one pass on `enc`. Returns the number of instances drawn.
draw :: proc(g: ^Gpu, r: ^Renderer, ui: ^kui.Ui, enc: wgpu.CommandEncoder, target: wgpu.TextureView, world: wgpu.Color) -> int {
	dd, ok := kui.draw_data(ui)
	if !ok do return 0
	atlas_upload(g, r, dd)
	globals := [8]f32{dd.viewport_w, dd.viewport_h, f32(r.atlas_size), f32(r.atlas_size), dd.time, dd.scale, 0, 0}
	wgpu.QueueWriteBuffer(g.queue, r.globals, 0, &globals, size_of(globals))

	packed := kui.pack_instances(ui)
	count := len(packed) / int(kui.instance_stride())
	if len(packed) > r.instance_cap {
		if r.instances != nil do wgpu.BufferRelease(r.instances)
		r.instance_cap = len(packed) * 2
		r.instances = wgpu.DeviceCreateBuffer(g.device, &{usage = {.Vertex, .CopyDst}, size = u64(r.instance_cap)})
	}
	if count > 0 do wgpu.QueueWriteBuffer(g.queue, r.instances, 0, raw_data(packed), uint(len(packed)))

	pass := wgpu.CommandEncoderBeginRenderPass(enc, &{
		colorAttachmentCount = 1,
		colorAttachments = &wgpu.RenderPassColorAttachment {
			view = target,
			loadOp = .Clear,
			storeOp = .Store,
			depthSlice = wgpu.DEPTH_SLICE_UNDEFINED,
			clearValue = world,
		},
	})
	// The draw scissored to where the frame drew (Draw_Data.bounds): what
	// is outside it stays the world.
	b := dd.bounds
	if count > 0 && b[2] > 0 && b[3] > 0 {
		wgpu.RenderPassEncoderSetScissorRect(pass, b[0], b[1], b[2], b[3])
		wgpu.RenderPassEncoderSetPipeline(pass, r.pipeline)
		wgpu.RenderPassEncoderSetBindGroup(pass, 0, r.group)
		wgpu.RenderPassEncoderSetVertexBuffer(pass, 0, r.instances, 0, u64(len(packed)))
		wgpu.RenderPassEncoderDraw(pass, 6, u32(count), 0, 0)
	}
	wgpu.RenderPassEncoderEnd(pass)
	wgpu.RenderPassEncoderRelease(pass)
	return count
}

renderer_free :: proc(r: ^Renderer) {
	if r.instances != nil do wgpu.BufferRelease(r.instances)
	if r.group != nil do wgpu.BindGroupRelease(r.group)
	if r.atlas_view != nil do wgpu.TextureViewRelease(r.atlas_view)
	if r.atlas != nil do wgpu.TextureRelease(r.atlas)
	wgpu.SamplerRelease(r.nearest)
	wgpu.SamplerRelease(r.linear)
	wgpu.BufferRelease(r.globals)
	wgpu.RenderPipelineRelease(r.pipeline)
	wgpu.BindGroupLayoutRelease(r.layout)
}

// -- offscreen: drawn, read back, checked --------------------------------------

fails := 0

check :: proc(ok: bool, what: string) {
	if !ok {
		fmt.eprintln("FAIL:", what)
		fails += 1
	}
}

// An offscreen target and the buffer it is read back through; a copy's
// rows are 256-byte aligned.
Target :: struct {
	texture: wgpu.Texture,
	view:    wgpu.TextureView,
	buffer:  wgpu.Buffer,
	w, h:    u32,
	row:     u32,
	pixels:  []u8,
}

target_make :: proc(g: ^Gpu, w, h: u32) -> (t: Target) {
	t.w, t.h, t.row = w, h, (w * 4 + 255) &~ 255
	t.texture = wgpu.DeviceCreateTexture(g.device, &{usage = {.RenderAttachment, .CopySrc}, dimension = ._2D, size = {w, h, 1}, format = .RGBA8Unorm, mipLevelCount = 1, sampleCount = 1})
	t.view = wgpu.TextureCreateView(t.texture, nil)
	t.buffer = wgpu.DeviceCreateBuffer(g.device, &{usage = {.CopyDst, .MapRead}, size = u64(t.row * h)})
	t.pixels = make([]u8, int(t.row * h))
	return
}

target_free :: proc(t: ^Target) {
	wgpu.BufferRelease(t.buffer)
	wgpu.TextureViewRelease(t.view)
	wgpu.TextureRelease(t.texture)
	delete(t.pixels)
}

// The frame drawn into the target and read back into t.pixels.
render_offscreen :: proc(g: ^Gpu, r: ^Renderer, ui: ^kui.Ui, t: ^Target) -> int {
	enc := wgpu.DeviceCreateCommandEncoder(g.device, nil)
	defer wgpu.CommandEncoderRelease(enc)
	drawn := draw(g, r, ui, enc, t.view, WORLDS[0])
	wgpu.CommandEncoderCopyTextureToBuffer(enc, &{texture = t.texture, aspect = .All}, &{buffer = t.buffer, layout = {bytesPerRow = t.row, rowsPerImage = t.h}}, &{t.w, t.h, 1})
	cmd := wgpu.CommandEncoderFinish(enc, nil)
	defer wgpu.CommandBufferRelease(cmd)
	wgpu.QueueSubmit(g.queue, {cmd})

	m: Answer
	on_mapped :: proc "c" (status: wgpu.MapAsyncStatus, message: string, u1, _: rawptr) {
		context = runtime.default_context()
		m := (^Answer)(u1)
		m.done, m.ok = true, status == .Success
		if !m.ok do fmt.eprintln("map:", message)
	}
	size := uint(len(t.pixels))
	wgpu.BufferMapAsync(t.buffer, {.Read}, 0, size, {mode = .AllowProcessEvents, callback = on_mapped, userdata1 = &m})
	wgpu.DevicePoll(g.device, true, nil)
	wait_for(g.instance, &m)
	if !m.ok {
		check(false, "the readback mapped")
		return 0
	}
	copy(t.pixels, wgpu.BufferGetConstMappedRangeSlice(t.buffer, 0, size, u8))
	wgpu.BufferUnmap(t.buffer)
	return drawn
}

pixel :: proc(t: ^Target, x, y: f32) -> [3]u8 {
	at := int(y) * int(t.row) + int(x) * 4
	return {t.pixels[at], t.pixels[at + 1], t.pixels[at + 2]}
}

// Within `tol` per channel of a colour (kui's 0xRRGGBBAA, alpha ignored).
near :: proc(p: [3]u8, rgba: u32, tol: int) -> bool {
	for c in 0 ..< 3 {
		want := int((rgba >> (24 - 8 * uint(c))) & 0xff)
		if abs(int(p[c]) - want) > tol do return false
	}
	return true
}

world_rgba :: proc(c: wgpu.Color) -> u32 {
	return u32(c.r * 255 + 0.5) << 24 | u32(c.g * 255 + 0.5) << 16 | u32(c.b * 255 + 0.5) << 8 | 0xff
}

// The quad the button's background is: the solid of its size.
button_quad :: proc(dd: kui.Draw_Data) -> (kui.Quad, bool) {
	for q in dd.quads {
		if q.kind == .Solid && q.w == BUTTON_W * SCALE && q.h == BUTTON_H * SCALE do return q, true
	}
	return {}, false
}

// What every drawn frame shows, wherever its pixels came from.
check_frame :: proc(ui: ^kui.Ui, t: ^Target, when_: string) {
	dd, _ := kui.draw_data(ui)
	// The panel's colour at its right edge, clear of the text.
	p := pixel(t, (PANEL_X + PANEL_W - 6) * SCALE, (PANEL_Y + PANEL_H / 2) * SCALE)
	check(near(p, u32(PANEL_BG), 2), fmt.tprintf("%s: the panel is its colour", when_))
	// The world far from it.
	world := world_rgba(WORLDS[0])
	check(near(pixel(t, f32(t.w - 10), f32(t.h - 10)), world, 2), fmt.tprintf("%s: past the panel is the clear colour", when_))
	// The shadow, just under the panel's bottom edge: darker than the world
	// it falls on.
	p = pixel(t, (PANEL_X + PANEL_W / 2) * SCALE, (PANEL_Y + PANEL_H + 3) * SCALE)
	lit := int(world >> 24) + int((world >> 16) & 0xff) + int((world >> 8) & 0xff)
	check(int(p[0]) + int(p[1]) + int(p[2]) < lit * 9 / 10, fmt.tprintf("%s: the shadow darkens the world under the panel", when_))

	// The title: glyph quads in the panel's upper half, and pixels inside
	// their union that are not the flat panel.
	top := f32((PANEL_Y + PANEL_H / 2) * SCALE)
	lo, hi := [2]f32{1e9, 1e9}, [2]f32{-1e9, -1e9}
	for q in dd.quads {
		if (q.kind != .Glyph_Mask && q.kind != .Glyph_Subpixel) || q.y + q.h > top do continue
		lo = {min(lo.x, q.x), min(lo.y, q.y)}
		hi = {max(hi.x, q.x + q.w), max(hi.y, q.y + q.h)}
	}
	inked := 0
	for y := lo.y; y < hi.y; y += 1 {
		for x := lo.x; x < hi.x; x += 1 {
			if !near(pixel(t, x, y), u32(PANEL_BG), 40) do inked += 1
		}
	}
	check(hi.x > lo.x && inked >= 30, fmt.tprintf("%s: the title's glyphs are drawn (atlas uploaded, sampled)", when_))

	// The button's colour at its left edge, clear of its label.
	b, found := button_quad(dd)
	check(found, fmt.tprintf("%s: the button's quad is in the frame", when_))
	if found do check(near(pixel(t, b.x + 6, b.y + b.h / 2), u32(BUTTON_BG), 2), fmt.tprintf("%s: the button is its colour", when_))
}

offscreen :: proc() -> int {
	need, _ := os.lookup_env("KUI_REQUIRE_GPU", context.temp_allocator)
	g: Gpu
	defer gpu_close(&g)
	if !gpu_open(&g) {
		if need != "" {
			fmt.eprintln("FAIL: KUI_REQUIRE_GPU is set, but wgpu found no adapter")
			return 1
		}
		fmt.eprintln("no GPU adapter: wgpu_host has nothing to draw on")
		return 0
	}
	if need != "" && !strings.contains(g.name, need) {
		fmt.eprintfln("FAIL: KUI_REQUIRE_GPU is %s, but the adapter is %s", need, g.name)
		return 1
	}

	ui := kui.new_ui()
	if ui == nil {
		fmt.eprintln("FAIL: libkui_ffi's ABI is not the one this binding was generated against")
		return 1
	}
	defer kui.free_ui(ui)
	r: Renderer
	renderer_init(&g, &r, .RGBA8Unorm)
	defer renderer_free(&r)
	t := target_make(&g, W * SCALE, H * SCALE)
	defer target_free(&t)
	s: State

	// The host loop's turn (kui.h): build, drain, draw.
	kui.frame(ui, &s, view, W, H, SCALE)
	drain(ui, &s)
	drawn := render_offscreen(&g, &r, ui, &t)
	check(drawn > 0, "the first frame drew instances")
	check(r.uploaded[2] == r.atlas_size, "the first frame uploaded the whole atlas page")
	check_frame(ui, &t, "first frame")

	// Input, then the next turn: a click on the button, its message, and
	// the frame rebuilt with the new count. The label's "1" is a glyph the
	// atlas did not hold, so the upload is a rect, not the page.
	dd, _ := kui.draw_data(ui)
	if b, found := button_quad(dd); found {
		kui.input_cursor(ui, (b.x + b.w / 2) / SCALE, (b.y + b.h / 2) / SCALE)
		check(kui.pointer_wants(ui), "the pointer over the button is the UI's")
		kui.input_mouse(ui, true, 1)
		kui.input_mouse(ui, false, 1)
		drain(ui, &s)
	}
	check(s.clicks == 1, "the click arrived as the button's message")
	kui.input_cursor(ui, W - 4, H - 4)
	check(!kui.pointer_wants(ui), "past the panel the pointer is the world's")
	kui.frame(ui, &s, view, W, H, SCALE)
	drain(ui, &s)
	drawn = render_offscreen(&g, &r, ui, &t)
	check(drawn > 0, "the second frame drew instances")
	check(r.uploaded[2] > 0 && r.uploaded[2] < r.atlas_size, "the second frame uploaded a dirty rect alone")
	check_frame(ui, &t, "second frame")
	check(wgpu_errors == 0, "wgpu raised no validation error")

	if fails > 0 {
		fmt.eprintfln("wgpu_host: %d check(s) failed", fails)
		return 1
	}
	fmt.printfln("wgpu host OK: %s, %dx%d, %d instances of %d bytes, the atlas a %d px page then a %dx%d rect", g.name, t.w, t.h, drawn, kui.instance_stride(), r.atlas_size, r.uploaded[2], r.uploaded[3])
	return 0
}

// -- the window -----------------------------------------------------------------

App :: struct {
	ui:      ^kui.Ui,
	state:   State,
	window:  glfw.WindowHandle,
	surface: wgpu.Surface,
	config:  wgpu.SurfaceConfiguration,
}

app: App

// GLFW's input, handed to kui as data - and to the world, when kui says
// the input is not the UI's (kui.h, "The UI over a world of its own").
on_cursor :: proc "c" (_: glfw.WindowHandle, x, y: f64) {
	context = runtime.default_context()
	kui.input_cursor(app.ui, f32(x), f32(y))
}

on_enter :: proc "c" (_: glfw.WindowHandle, entered: i32) {
	context = runtime.default_context()
	if entered == 0 do kui.input_cursor_left(app.ui)
}

on_button :: proc "c" (_: glfw.WindowHandle, button, action, _: i32) {
	context = runtime.default_context()
	if button != glfw.MOUSE_BUTTON_LEFT do return
	if action == glfw.PRESS && !kui.pointer_wants(app.ui) {
		app.state.world = (app.state.world + 1) % len(WORLDS)
	}
	// kui hears every press, the world's too: a press outside a focused
	// field is what blurs it.
	kui.input_mouse(app.ui, action == glfw.PRESS, 1)
}

on_key :: proc "c" (w: glfw.WindowHandle, key, scancode, action, mods: i32) {
	context = runtime.default_context()
	if key == glfw.KEY_ESCAPE && action == glfw.PRESS && !kui.keyboard_wants(app.ui) {
		glfw.SetWindowShouldClose(w, true)
	}
	kui_glfw.on_key(app.ui, key, scancode, action, mods)
}

on_char :: proc "c" (_: glfw.WindowHandle, r: rune) {
	context = runtime.default_context()
	kui_glfw.on_char(app.ui, r)
}

// The surface at the framebuffer's size; false while the window has none.
configure :: proc() -> bool {
	fw, fh := glfw.GetFramebufferSize(app.window)
	if fw <= 0 || fh <= 0 do return false
	if app.config.width != u32(fw) || app.config.height != u32(fh) {
		app.config.width, app.config.height = u32(fw), u32(fh)
		wgpu.SurfaceConfigure(app.surface, &app.config)
	}
	return true
}

window :: proc(max_frames: int) -> int {
	if !glfw.Init() {
		fmt.eprintln("wgpu_host: glfw.Init failed")
		return 1
	}
	defer glfw.Terminate()
	glfw.WindowHint(glfw.CLIENT_API, glfw.NO_API)
	app.window = glfw.CreateWindow(640, 400, "kui over wgpu", nil, nil)
	if app.window == nil {
		fmt.eprintln("wgpu_host: glfw.CreateWindow failed")
		return 1
	}
	defer glfw.DestroyWindow(app.window)

	g: Gpu
	defer gpu_close(&g)
	g.instance = wgpu.CreateInstance(nil)
	app.surface = glfwglue.GetSurface(g.instance, app.window)
	defer wgpu.SurfaceRelease(app.surface)
	if !gpu_open(&g, app.surface) {
		fmt.eprintln("wgpu_host: no GPU adapter that can draw to the window")
		return 1
	}
	// An Unorm format, as kui-wgpu picks: kui's colours are written as
	// they are, with no sRGB encode.
	caps, _ := wgpu.SurfaceGetCapabilities(app.surface, g.adapter)
	defer wgpu.SurfaceCapabilitiesFreeMembers(caps)
	format := caps.formats[0]
	for f in caps.formats[:caps.formatCount] {
		if f == .BGRA8Unorm || f == .RGBA8Unorm {
			format = f
			break
		}
	}
	app.config = {device = g.device, usage = {.RenderAttachment}, format = format, presentMode = .Fifo, alphaMode = .Opaque}
	configure()

	app.ui = kui.new_ui()
	if app.ui == nil {
		fmt.eprintln("FAIL: libkui_ffi's ABI is not the one this binding was generated against")
		return 1
	}
	defer kui.free_ui(app.ui)
	r: Renderer
	renderer_init(&g, &r, format)
	defer renderer_free(&r)

	glfw.SetInputMode(app.window, glfw.LOCK_KEY_MODS, 1)
	glfw.SetCursorPosCallback(app.window, on_cursor)
	glfw.SetCursorEnterCallback(app.window, on_enter)
	glfw.SetMouseButtonCallback(app.window, on_button)
	glfw.SetKeyCallback(app.window, on_key)
	glfw.SetCharCallback(app.window, on_char)

	frames := 0
	for !glfw.WindowShouldClose(app.window) {
		defer free_all(context.temp_allocator)
		// 1. input: the callbacks above, inside the poll. After the first
		// frame the window sleeps between inputs, unless kui is owed a frame
		// (a transition) or a frame count is being run.
		if frames > 0 && max_frames == 0 && kui.owed(app.ui) == {} {
			glfw.WaitEventsTimeout(0.5)
		} else {
			glfw.PollEvents()
		}
		// 2. drain what the input raised.
		drain(app.ui, &app.state)
		if !configure() do continue
		ww, wh := glfw.GetWindowSize(app.window)
		scale := f32(app.config.width) / f32(max(ww, 1))
		// 3. build; 4. drain what the frame raised, and build again when it
		// changed the model.
		kui.set_time(app.ui, glfw.GetTime())
		kui.frame(app.ui, &app.state, view, f32(ww), f32(wh), scale)
		if drain(app.ui, &app.state) do kui.frame(app.ui, &app.state, view, f32(ww), f32(wh), scale)

		// 5. draw: the world, kui over it.
		st := wgpu.SurfaceGetCurrentTexture(app.surface)
		#partial switch st.status {
		case .SuccessOptimal, .SuccessSuboptimal:
		case .Occluded:
			continue
		case:
			// Outdated, lost or timed out: configured again next turn.
			if st.texture != nil do wgpu.TextureRelease(st.texture)
			app.config.width = 0
			continue
		}
		frame := wgpu.TextureCreateView(st.texture, nil)
		enc := wgpu.DeviceCreateCommandEncoder(g.device, nil)
		draw(&g, &r, app.ui, enc, frame, WORLDS[app.state.world])
		cmd := wgpu.CommandEncoderFinish(enc, nil)
		wgpu.QueueSubmit(g.queue, {cmd})
		wgpu.SurfacePresent(app.surface)
		wgpu.CommandBufferRelease(cmd)
		wgpu.CommandEncoderRelease(enc)
		wgpu.TextureViewRelease(frame)
		wgpu.TextureRelease(st.texture)

		frames += 1
		if max_frames > 0 && frames >= max_frames do break
	}
	if wgpu_errors > 0 {
		fmt.eprintfln("wgpu_host: %d wgpu error(s)", wgpu_errors)
		return 1
	}
	fmt.printfln("wgpu host window: %d frames on %s, %d button clicks, world %d", frames, g.name, app.state.clicks, app.state.world)
	return 0
}

main :: proc() {
	windowed := false
	max_frames := 0
	for i := 1; i < len(os.args); i += 1 {
		switch os.args[i] {
		case "--window":
			windowed = true
		case "--headless":
			windowed = false
		case "--frames":
			if i + 1 < len(os.args) {
				max_frames, _ = strconv.parse_int(os.args[i + 1])
				i += 1
			}
		}
	}
	os.exit(window(max_frames) if windowed else offscreen())
}
