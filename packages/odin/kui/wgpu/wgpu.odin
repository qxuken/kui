// kui's frame drawn on a WebGPU device the host owns (backlog F263): kui.h's
// "Drawing it yourself" as three calls on vendor:wgpu, so a host with its
// own device does not copy the pipeline out of an example. kui_wgpu.h is
// the same in C.
//
// A package of its own, so importing kui links no wgpu:
//
//     import kui_wgpu "kui/wgpu"
//     r: kui_wgpu.Renderer
//     kui_wgpu.init(&r, device, queue, surface_format)
//     // each frame, after kui.frame:
//     kui_wgpu.upload(&r, ui, enc)   // before the pass: optional, faster
//     pass := wgpu.CommandEncoderBeginRenderPass(enc, &yours)
//     // ...your world...
//     kui_wgpu.draw(&r, ui, pass)
//     wgpu.RenderPassEncoderEnd(pass)
//     // at exit:
//     kui_wgpu.destroy(&r)
//
// The draw goes into a pass the host began, so the load op, the clear and
// any passes before or after are the host's; kui draws over what the pass
// holds, scissored to Draw_Data.bounds, so a HUD over a game touches the
// HUD's pixels alone. Before it records anything the frame's data goes up:
// the atlas (the whole page when it is new, else atlas_dirty_rect alone)
// and the uniform through the queue, and the instances one of two ways
// (backlog F277), as kui_wgpu.h does:
//
// - upload, called on the encoder before the pass begins (a copy cannot be
//   recorded inside a pass), packs them straight into a staging buffer
//   mapped ahead of time and records its copy into the vertex buffer. The
//   staging buffers are a ring of RING, each mapped again once the GPU is
//   done with it, asked for at the next upload, after the host's submit;
//   the device fires the callback (.AllowSpontaneous), so no poll is asked
//   of the host. With every one in flight it falls back to the writes below.
// - draw alone writes them through the queue, in pieces under a page
//   (WRITE_PIECE): wgpu-native stages each write in a buffer of its own,
//   and on Metal one of 16 KiB or more costs ~100 µs more.
//
// Queue writes land before the next submit, which is the one that carries
// the pass. So one Renderer draws one frame a submit, upload and draw of
// the same finished frame; two Uis drawn in one submit want a Renderer each.
//
// The target is the viewport's size (Draw_Data's viewport_w/h, the frame's
// size times its scale) and Unorm (.RGBA8Unorm, .BGRA8Unorm): kui's
// colours are written as they are, no sRGB encode, as kui-wgpu draws them.
// After the draw the pass's pipeline, bind group 0, vertex buffer 0 and
// scissor rect are kui's: set yours again to draw after it in the same
// pass.
//
// Subpixel text: `dual` picks kui.renderer_wgsl's variant. The single one
// blends straight colour by its alpha and draws on any device. The dual
// one blends per channel, which LCD subpixel glyphs need, and wants a
// device created with .DualSourceBlending (init returns false without it).
// Call kui.set_subpixel_text(ui, true) only with dual; otherwise kui
// rasterizes grayscale masks, which is the default.
//
// What it leaves out are kui.h's passes, and it draws nothing where they
// would be: a texture-backed image (.Texture, which wants its own texture
// bound in the atlas's place), a registered fragment (.Fragment, a
// pipeline of its own), a backdrop blur (.Backdrop) and an opacity group's
// picture (.Group_End, only after kui.set_opacity_groups). kui.run and
// kui-wgpu draw all four; a host that needs one draws it itself, or opens
// its window with kui.
//
// vendor:wgpu links wgpu-native's static library, which carries its own
// Rust standard library as libkui_ffi's does: link kui shared
// (-define:KUI_SHARED=true) beside it, as packages/odin/README.md says.
// examples/odin/tools/wgpu_host.odin draws through this offscreen, its
// pixels checked, and in a GLFW window with --window.
package kui_wgpu

import "base:intrinsics"
import "base:runtime"
import "vendor:wgpu"
import kui ".."
import kui_c "../c"

// The staging buffers upload cycles through, and a queue write's largest
// piece (under a page, a multiple of 4).
RING :: 4
WRITE_PIECE :: 16128

// How the last frame's instances went up.
Instances_By :: enum u8 {
	None,
	Staged, // copied from the ring
	Written, // queue writes
}

// One staging buffer of the ring. `cell` is its map callback's (Cell), on
// the heap so a callback arriving after destroy has somewhere to write;
// whichever of the two comes last frees it.
@(private)
Staging :: struct {
	buffer: wgpu.Buffer,
	cell:   ^i32,
	phase:  enum u8 {
		None,
		Free, // mapped, free
		Copied, // copied from this frame
		Mapping, // waiting for its map
	},
}

@(private)
Cell :: enum i32 {
	Pending,
	Mapped,
	Failed,
	Dead,
}

Renderer :: struct {
	device:       wgpu.Device,
	queue:        wgpu.Queue,
	dual:         bool,
	layout:       wgpu.BindGroupLayout,
	pipeline:     wgpu.RenderPipeline,
	// The eight floats kui.h gives.
	globals:      wgpu.Buffer,
	linear:       wgpu.Sampler,
	nearest:      wgpu.Sampler,
	// The glyph atlas, kept between frames: a page the size the draw data
	// says, and the epoch of the page it holds (0: none yet).
	atlas:        wgpu.Texture,
	atlas_view:   wgpu.TextureView,
	atlas_size:   u32,
	atlas_epoch:  u64,
	group:        wgpu.BindGroup,
	// The instances, grown to twice a frame that outgrew them, and the
	// memory they are packed into on the way.
	instances:    wgpu.Buffer,
	instance_cap: int,
	packed:       []u8,
	allocator:    runtime.Allocator,
	// What the last upload put up of the atlas (upload's, or draw's without
	// one), x, y, w, h in texels: the page, a dirty rect, or zero for
	// nothing.
	uploaded:     [4]u32,
	// upload's ring, each staging buffer ring_cap bytes (the instance
	// buffer's size), and how many instances it put up for the draw that
	// follows (0: none; the draw writes them itself).
	ring:         [RING]Staging,
	ring_cap:     int,
	staged:       int,
	instances_by: Instances_By,
}

// kui.renderer_wgsl(dual) as a pipeline drawing into `format`, the bind
// group's layout, samplers and uniform; `allocator` holds the packed
// instances. False, with nothing built and `r` zeroed, on a nil device or
// queue, or for dual on a device without dual-source blending. A
// validation error is the device's own, raised through its
// uncaptured-error callback.
init :: proc(r: ^Renderer, device: wgpu.Device, queue: wgpu.Queue, format: wgpu.TextureFormat, dual := false, allocator := context.allocator) -> bool {
	r^ = {}
	if device == nil || queue == nil do return false
	if dual && !wgpu.DeviceHasFeature(device, .DualSourceBlending) do return false
	r.device, r.queue, r.dual, r.allocator = device, queue, dual, allocator

	module := wgpu.DeviceCreateShaderModule(device, &{nextInChain = &wgpu.ShaderSourceWGSL{sType = .ShaderSourceWGSL, code = kui.renderer_wgsl(dual)}})
	defer wgpu.ShaderModuleRelease(module)

	// Group 0 as kui.h gives it: the globals, the atlas, two samplers.
	entries := [4]wgpu.BindGroupLayoutEntry {
		{binding = 0, visibility = {.Vertex, .Fragment}, buffer = {type = .Uniform}},
		{binding = 1, visibility = {.Fragment}, texture = {sampleType = .Float, viewDimension = ._2D}},
		{binding = 2, visibility = {.Fragment}, sampler = {type = .Filtering}},
		{binding = 3, visibility = {.Fragment}, sampler = {type = .Filtering}},
	}
	r.layout = wgpu.DeviceCreateBindGroupLayout(device, &{entryCount = len(entries), entries = &entries[0]})
	pipeline_layout := wgpu.DeviceCreatePipelineLayout(device, &{bindGroupLayoutCount = 1, bindGroupLayouts = &r.layout})
	defer wgpu.PipelineLayoutRelease(pipeline_layout)

	// The instance layout, read from the library at run time: each
	// attribute a float32x2 or float32x4 at its location and offset,
	// stepped per instance. A later library that moves it needs no change
	// here.
	kui_attrs := kui.instance_attributes(context.temp_allocator)
	attrs := make([]wgpu.VertexAttribute, len(kui_attrs), context.temp_allocator)
	for a, i in kui_attrs {
		attrs[i] = {format = .Float32x2 if a.floats == 2 else .Float32x4, offset = u64(a.offset), shaderLocation = a.location}
	}
	// As kui-wgpu blends. Single writes straight colour with its coverage
	// as alpha: colour by its alpha, alpha over. Dual writes premultiplied
	// colour and a per-channel coverage as the second source, out = src +
	// dst * (1 - coverage); on a quad whose channels' coverage is its alpha
	// that is premultiplied alpha blending.
	blend := wgpu.BlendState {
		color = {srcFactor = .SrcAlpha, dstFactor = .OneMinusSrcAlpha, operation = .Add},
		alpha = {srcFactor = .One, dstFactor = .OneMinusSrcAlpha, operation = .Add},
	}
	if dual {
		blend.color = {srcFactor = .One, dstFactor = .OneMinusSrc1, operation = .Add}
		blend.alpha = {srcFactor = .One, dstFactor = .OneMinusSrc1Alpha, operation = .Add}
	}
	r.pipeline = wgpu.DeviceCreateRenderPipeline(device, &{
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
	r.globals = wgpu.DeviceCreateBuffer(device, &{usage = {.Uniform, .CopyDst}, size = 8 * size_of(f32)})
	// kui-wgpu's two samplers: clamped, one filtering and one not.
	r.linear = wgpu.DeviceCreateSampler(device, &{
		addressModeU = .ClampToEdge, addressModeV = .ClampToEdge, addressModeW = .ClampToEdge,
		magFilter = .Linear, minFilter = .Linear, mipmapFilter = .Nearest, lodMaxClamp = 32, maxAnisotropy = 1,
	})
	r.nearest = wgpu.DeviceCreateSampler(device, &{
		addressModeU = .ClampToEdge, addressModeV = .ClampToEdge, addressModeW = .ClampToEdge,
		magFilter = .Nearest, minFilter = .Nearest, mipmapFilter = .Nearest, lodMaxClamp = 32, maxAnisotropy = 1,
	})
	return r.pipeline != nil
}

// A page of `size`, and the bind group that names it.
@(private)
atlas_page :: proc(r: ^Renderer, size: u32) {
	if r.atlas != nil {
		wgpu.BindGroupRelease(r.group)
		wgpu.TextureViewRelease(r.atlas_view)
		wgpu.TextureRelease(r.atlas)
	}
	r.atlas = wgpu.DeviceCreateTexture(r.device, &{
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
	r.group = wgpu.DeviceCreateBindGroup(r.device, &{layout = r.layout, entryCount = len(entries), entries = &entries[0]})
}

// The atlas: whole when the page is new to us, else the dirty rect alone.
// The rect's rows are atlas_pixels' own, so the source starts at its corner
// with the page's stride.
@(private)
atlas_upload :: proc(r: ^Renderer, dd: ^kui_c.DrawData) {
	size := max(dd.atlas_size, 1)
	if r.atlas == nil || size != r.atlas_size do atlas_page(r, size)
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
	wgpu.QueueWriteTexture(r.queue, &{texture = r.atlas, aspect = .All, origin = {x, y, 0}}, &dd.atlas_pixels[from], bytes, &{bytesPerRow = size * 4, rowsPerImage = h}, &{w, h, 1})
	r.atlas_epoch = dd.atlas_epoch
	r.uploaded = {x, y, w, h}
}

// A quad this pipeline draws, rather than one of the passes it leaves out
// (the list at the top).
@(private)
kind_drawn :: proc(kind: u32) -> bool {
	return kind != kui_c.QUAD_TEXTURE && kind != kui_c.QUAD_FRAGMENT && kind != kui_c.QUAD_GROUP_END
}

// The instance buffer, grown to twice a frame that outgrew it, and the
// memory the queue writes are packed into.
@(private)
grow :: proc(r: ^Renderer, count: int) {
	if count <= r.instance_cap do return
	stride := int(kui_c.instance_stride())
	if r.instances != nil do wgpu.BufferRelease(r.instances)
	r.instance_cap = count * 2
	r.instances = wgpu.DeviceCreateBuffer(r.device, &{usage = {.Vertex, .CopyDst}, size = u64(r.instance_cap * stride)})
	delete(r.packed, r.allocator)
	r.packed = make([]u8, r.instance_cap * stride, r.allocator)
}

// What every draw puts up first, with upload or without: the atlas and the
// uniform through the queue, and the instance buffer grown to the frame.
// Its instance count; 0 with nothing to draw.
@(private)
prepare :: proc(r: ^Renderer, ui: ^kui.Ui, dd: ^kui_c.DrawData) -> int {
	dd^ = {size = size_of(kui_c.DrawData)}
	if r.pipeline == nil || !kui_c.draw_data(ui, dd) do return 0
	atlas_upload(r, dd)
	globals := [8]f32{dd.viewport_w, dd.viewport_h, f32(r.atlas_size), f32(r.atlas_size), dd.time, dd.scale, 0, 0}
	wgpu.QueueWriteBuffer(r.queue, r.globals, 0, &globals, size_of(globals))
	count := int(kui_c.pack_instances(ui, nil, 0))
	if count == 0 || dd.bounds[2] == 0 || dd.bounds[3] == 0 do return 0
	grow(r, count)
	return count
}

// The instances through the queue, in pieces under a page.
@(private)
write_instances :: proc(r: ^Renderer, ui: ^kui.Ui, bytes: int) {
	kui_c.pack_instances(ui, raw_data(r.packed), uint(bytes))
	for off := 0; off < bytes; off += WRITE_PIECE {
		wgpu.QueueWriteBuffer(r.queue, r.instances, u64(off), &r.packed[off], uint(min(bytes - off, WRITE_PIECE)))
	}
	r.instances_by = .Written
}

@(private)
mapped :: proc "c" (status: wgpu.MapAsyncStatus, message: wgpu.StringView, cell: rawptr, _: rawptr) {
	now := Cell.Mapped if status == .Success else Cell.Failed
	if Cell(intrinsics.atomic_exchange((^i32)(cell), i32(now))) == .Dead {
		context = runtime.default_context()
		free((^i32)(cell), runtime.heap_allocator())
	}
}

// One staging buffer gone: released, and its cell freed, or left to the
// callback still to come.
@(private)
staging_drop :: proc(s: ^Staging) {
	if s.phase != .Mapping || Cell(intrinsics.atomic_exchange(s.cell, i32(Cell.Dead))) != .Pending {
		if s.cell != nil do free(s.cell, runtime.heap_allocator())
	}
	if s.buffer != nil do wgpu.BufferRelease(s.buffer)
	s^ = {}
}

// The instances packed into a mapped staging buffer, its copy recorded on
// `enc`. Before picking one: the buffers last frame copied from (the host
// has submitted since) are asked to map, and those whose map has come back
// are free again. False when none is free, or one cannot be made.
@(private)
stage :: proc(r: ^Renderer, ui: ^kui.Ui, bytes: int, enc: wgpu.CommandEncoder) -> bool {
	cap := r.instance_cap * int(kui_c.instance_stride())
	if cap != r.ring_cap {
		for &s in r.ring do staging_drop(&s)
		r.ring_cap = cap
	}
	pick: ^Staging
	for &s in r.ring {
		if s.phase == .Copied {
			s.cell = new(i32, runtime.heap_allocator())
			s.phase = .Mapping
			// .AllowSpontaneous, spelled AllowSpontaneos in vendor:wgpu.
			wgpu.BufferMapAsync(s.buffer, {.Write}, 0, uint(cap), {mode = wgpu.CallbackMode(3), callback = mapped, userdata1 = s.cell})
		}
		if s.phase == .Mapping {
			switch Cell(intrinsics.atomic_load(s.cell)) {
			case .Mapped:
				free(s.cell, runtime.heap_allocator())
				s.cell, s.phase = nil, .Free
			case .Failed:
				staging_drop(&s)
			case .Pending, .Dead:
			}
		}
		if pick == nil && s.phase == .Free do pick = &s
	}
	for &s in r.ring {
		if pick != nil do break
		if s.phase != .None do continue
		s.buffer = wgpu.DeviceCreateBuffer(r.device, &{usage = {.MapWrite, .CopySrc}, size = u64(cap), mappedAtCreation = true})
		if s.buffer == nil do return false
		s.phase = .Free
		pick = &s
	}
	if pick == nil do return false
	dst := wgpu.RawBufferGetMappedRange(pick.buffer, 0, uint(cap))
	if dst == nil do return false
	kui_c.pack_instances(ui, ([^]u8)(dst), uint(bytes))
	wgpu.BufferUnmap(pick.buffer)
	wgpu.CommandEncoderCopyBufferToBuffer(enc, pick.buffer, 0, r.instances, 0, u64(bytes))
	pick.phase = .Copied
	return true
}

// `ui`'s last finished frame put up on `enc`, before the host begins the
// pass draw goes into (backlog F277): the atlas and the uniform through the
// queue, the instances packed into a staging buffer of the ring and copied
// on the encoder - or, with every staging buffer in flight, through the
// queue in pieces. Then draw records the draws alone. Once a frame, after
// the last frame's submit.
upload :: proc(r: ^Renderer, ui: ^kui.Ui, enc: wgpu.CommandEncoder) {
	dd: kui_c.DrawData
	r.staged = 0
	count := prepare(r, ui, &dd)
	if count == 0 do return
	bytes := count * int(kui_c.instance_stride())
	if stage(r, ui, bytes, enc) {
		r.instances_by = .Staged
	} else {
		write_instances(r, ui, bytes)
	}
	r.staged = count
}

// `ui`'s last finished frame into `pass`, over what it holds, scissored to
// Draw_Data.bounds. After upload, what it put up; without one, the atlas,
// uniform and instances written through the queue first. A frame finished
// between the two is not drawn (its instances would be overwritten by the
// upload's copy): upload it again. Returns the instances drawn; 0 with
// nothing to draw (no finished frame, or an empty one), which records
// nothing into the pass.
draw :: proc(r: ^Renderer, ui: ^kui.Ui, pass: wgpu.RenderPassEncoder) -> int {
	if r.pipeline == nil do return 0
	dd: kui_c.DrawData
	count: int
	staged := r.staged
	r.staged = 0
	if staged != 0 {
		dd = {size = size_of(kui_c.DrawData)}
		if !kui_c.draw_data(ui, &dd) do return 0
		count = int(kui_c.pack_instances(ui, nil, 0))
		if count != staged do return 0
	} else {
		count = prepare(r, ui, &dd)
		if count == 0 do return 0
		write_instances(r, ui, count * int(kui_c.instance_stride()))
	}
	bytes := count * int(kui_c.instance_stride())
	b := dd.bounds

	// Only where the frame drew: what is outside it stays the host's.
	wgpu.RenderPassEncoderSetScissorRect(pass, b[0], b[1], b[2], b[3])
	wgpu.RenderPassEncoderSetPipeline(pass, r.pipeline)
	wgpu.RenderPassEncoderSetBindGroup(pass, 0, r.group)
	wgpu.RenderPassEncoderSetVertexBuffer(pass, 0, r.instances, 0, u64(bytes))
	// The runs between the quads it leaves out, in order: one draw for the
	// usual frame, which has none of them.
	drawn, run := 0, 0
	for i in 0 ..= count {
		if i < count && kind_drawn(dd.quads[i].kind) do continue
		if i > run {
			wgpu.RenderPassEncoderDraw(pass, 6, u32(i - run), 0, u32(run))
			drawn += i - run
		}
		run = i + 1
	}
	return drawn
}

// Everything init and the draws made; `r` zeroed after, so a second
// destroy, or one after a failed init, does nothing. The device and queue
// are the host's and stay. A staging buffer still waiting for its map
// leaves its cell to the callback.
destroy :: proc(r: ^Renderer) {
	for &s in r.ring do staging_drop(&s)
	if r.packed != nil do delete(r.packed, r.allocator)
	if r.instances != nil do wgpu.BufferRelease(r.instances)
	if r.group != nil do wgpu.BindGroupRelease(r.group)
	if r.atlas_view != nil do wgpu.TextureViewRelease(r.atlas_view)
	if r.atlas != nil do wgpu.TextureRelease(r.atlas)
	if r.nearest != nil do wgpu.SamplerRelease(r.nearest)
	if r.linear != nil do wgpu.SamplerRelease(r.linear)
	if r.globals != nil do wgpu.BufferRelease(r.globals)
	if r.pipeline != nil do wgpu.RenderPipelineRelease(r.pipeline)
	if r.layout != nil do wgpu.BindGroupLayoutRelease(r.layout)
	r^ = {}
}
