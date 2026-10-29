/* kui_wgpu.h - kui's frame drawn on a WebGPU device the host owns (backlog
 * F263): kui.h's "Drawing it yourself" as three calls, so a host with its
 * own device does not copy the pipeline out of an example.
 *
 * Include after <webgpu/webgpu.h> and "kui.h". Header-only, against
 * webgpu.h alone (wgpu-native's and Dawn's are the same header now), so
 * libkui_ffi links no WebGPU and nothing here is in it.
 *
 *     KuiWgpu kr;
 *     kui_wgpu_init(&kr, device, queue, surface_format, KUI_WGSL_SINGLE);
 *     ...each frame, after kui_frame_finish:
 *     kui_wgpu_upload(&kr, ui, enc);        before the pass: optional, faster
 *     WGPURenderPassEncoder pass = wgpuCommandEncoderBeginRenderPass(enc, &yours);
 *     ...your world...
 *     kui_wgpu_draw(&kr, ui, pass);
 *     wgpuRenderPassEncoderEnd(pass);
 *     ...at exit:
 *     kui_wgpu_free(&kr);
 *
 * The draw goes into a pass the host began, so the load op, the clear and
 * any passes before or after are the host's; kui draws over what the pass
 * holds, scissored to KuiDrawData.bounds, so a HUD over a game touches
 * the HUD's pixels alone. Before it records anything the frame's data goes
 * up: the atlas (the whole page when it is new, else atlas_dirty_rect
 * alone) and the uniform through the queue, and the instances one of two
 * ways (backlog F277):
 *
 * - kui_wgpu_upload, called on the encoder before the pass begins (a copy
 *   cannot be recorded inside a pass), packs the instances straight into a
 *   staging buffer mapped ahead of time and records its copy into the
 *   vertex buffer on that encoder. The staging buffers are a ring of
 *   KUI_WGPU_RING, each mapped again once the GPU is done with it; one is
 *   asked for at the next upload, after the host's submit. The callback is
 *   the device's to fire (WGPUCallbackMode_AllowSpontaneous): wgpu-native
 *   fires it from its own submit and present, so no poll is asked of the
 *   host. With every one still in flight it falls back to the writes below.
 * - kui_wgpu_draw alone writes them through the queue, in pieces under a
 *   page (KUI_WGPU_WRITE_PIECE): wgpu-native stages each write in a buffer
 *   of its own, and on Metal one of 16 KiB or more costs ~100 µs more.
 *
 * Measured on an M3 Pro through wgpu-native (ui-compare, 93 instances):
 * the pieces take a third of one write's draw, kui_wgpu_upload a quarter.
 * Queue writes land before the next submit, which is the one that carries
 * the pass. So one KuiWgpu draws one frame a submit, upload and draw of
 * the same finished frame; two contexts drawn in one submit want a KuiWgpu
 * each.
 *
 * The target is the viewport's size (KuiDrawData's viewport_w/h, the
 * frame's size times its scale) and Unorm (RGBA8Unorm, BGRA8Unorm): kui's
 * colours are written as they are, no sRGB encode, as kui-wgpu draws
 * them. After the draw the pass's pipeline, bind group 0, vertex buffer 0
 * and scissor rect are kui's: set yours again to draw after it in the
 * same pass.
 *
 * Subpixel text: `variant` is kui_renderer_wgsl's. KUI_WGSL_SINGLE blends
 * straight colour by its alpha and draws on any device. KUI_WGSL_DUAL
 * blends per channel, which LCD subpixel glyphs need, and wants a device
 * created with WGPUFeatureName_DualSourceBlending (kui_wgpu_init returns
 * false without it). Pass kui_set_subpixel_text(ctx, true) only with
 * DUAL; under SINGLE kui rasterizes grayscale masks, which is the
 * default.
 *
 * What it leaves out are kui.h's passes, and it draws nothing where they
 * would be: a texture-backed image (KUI_QUAD_TEXTURE, which wants its own
 * texture bound in the atlas's place), a registered fragment
 * (KUI_QUAD_FRAGMENT, a pipeline of its own), a backdrop blur
 * (KUI_QUAD_BACKDROP) and an opacity group's picture (KUI_QUAD_GROUP_END,
 * only after kui_set_opacity_groups). kui_run and kui-wgpu draw all four;
 * a host that needs one draws it itself, or opens its window with kui.
 *
 * examples/c/tools/wgpu_host.c draws through this offscreen and checks
 * the pixels; packages/odin/kui/wgpu is the same in Odin. */
#ifndef KUI_WGPU_H
#define KUI_WGPU_H

#include <stdbool.h>
#include <stdint.h>
#include <stdlib.h>
#include <string.h>

/* The staging buffers kui_wgpu_upload cycles through: enough for the
 * frames a swap chain keeps in flight and the map that follows each. */
#ifndef KUI_WGPU_RING
#define KUI_WGPU_RING 4
#endif
/* A queue write's largest piece: under a page (16 KiB on Apple silicon),
 * a multiple of 4 as a buffer write must be. */
#ifndef KUI_WGPU_WRITE_PIECE
#define KUI_WGPU_WRITE_PIECE 16128
#endif

/* One staging buffer of the ring. `phase`: 0 none, 1 mapped and free, 2
 * copied from this frame, 3 waiting for its map. `cell` is the map
 * callback's (KUI_WGPU_CELL_*), on the heap so that a callback arriving
 * after kui_wgpu_free has somewhere to write; whichever of the two comes
 * last frees it. */
typedef struct KuiWgpuStaging {
    WGPUBuffer buffer;
    int *cell;
    int phase;
} KuiWgpuStaging;

typedef struct KuiWgpu {
    WGPUDevice device;
    WGPUQueue queue;
    uint32_t variant;               /* KUI_WGSL_SINGLE or _DUAL */
    WGPUBindGroupLayout layout;
    WGPURenderPipeline pipeline;
    WGPUBuffer globals;             /* the eight floats kui.h gives */
    WGPUSampler linear, nearest;
    /* The glyph atlas, kept between frames: a page the size the draw data
     * says, and the epoch of the page it holds (0: none yet). */
    WGPUTexture atlas;
    WGPUTextureView atlas_view;
    uint32_t atlas_size;
    uint64_t atlas_epoch;
    WGPUBindGroup group;
    /* The instances, grown to twice a frame that outgrew them, and the
     * memory a queue write packs them into on the way. */
    WGPUBuffer instances;
    size_t instance_cap;
    uint8_t *packed;
    /* What the last upload put up of the atlas (kui_wgpu_upload's, or
     * kui_wgpu_draw's without one), x, y, w, h in texels: the page, a
     * dirty rect, or zero for nothing. */
    uint32_t uploaded[4];
    /* kui_wgpu_upload's ring, each staging buffer ring_cap bytes (the
     * instance buffer's size), and how many instances it put up for the
     * draw that follows (0: none; the draw writes them itself). */
    KuiWgpuStaging ring[KUI_WGPU_RING];
    size_t ring_cap;
    size_t staged;
    /* How the last frame's instances went up: KUI_WGPU_STAGED (copied from
     * the ring) or KUI_WGPU_WRITTEN (queue writes), 0 before any. */
    uint32_t instances_by;
} KuiWgpu;

enum { KUI_WGPU_STAGED = 1, KUI_WGPU_WRITTEN = 2 };

/* kui_renderer_wgsl(`variant`) as a pipeline drawing into `format`, the
 * bind group's layout, samplers and uniform. False, with nothing built
 * and `r` zeroed, on a NULL device or queue, or for KUI_WGSL_DUAL on a
 * device without dual-source blending. A validation error is the
 * device's own, raised through its uncaptured-error callback. */
static inline bool kui_wgpu_init(KuiWgpu *r, WGPUDevice device, WGPUQueue queue,
                                 WGPUTextureFormat format, uint32_t variant) {
    memset(r, 0, sizeof *r);
    if (!device || !queue) return false;
    bool dual = variant == KUI_WGSL_DUAL;
    if (dual && !wgpuDeviceHasFeature(device, WGPUFeatureName_DualSourceBlending)) return false;
    r->device = device;
    r->queue = queue;
    r->variant = dual ? KUI_WGSL_DUAL : KUI_WGSL_SINGLE;

    KuiStr wgsl = kui_renderer_wgsl(r->variant);
    WGPUShaderSourceWGSL src = WGPU_SHADER_SOURCE_WGSL_INIT;
    src.code.data = (const char *)wgsl.ptr;
    src.code.length = wgsl.len;
    WGPUShaderModuleDescriptor md = WGPU_SHADER_MODULE_DESCRIPTOR_INIT;
    md.nextInChain = &src.chain;
    WGPUShaderModule module = wgpuDeviceCreateShaderModule(device, &md);

    /* Group 0 as kui.h gives it: the globals, the atlas, two samplers. */
    WGPUBindGroupLayoutEntry entries[4] = {
        WGPU_BIND_GROUP_LAYOUT_ENTRY_INIT, WGPU_BIND_GROUP_LAYOUT_ENTRY_INIT,
        WGPU_BIND_GROUP_LAYOUT_ENTRY_INIT, WGPU_BIND_GROUP_LAYOUT_ENTRY_INIT,
    };
    entries[0].binding = 0;
    entries[0].visibility = WGPUShaderStage_Vertex | WGPUShaderStage_Fragment;
    entries[0].buffer.type = WGPUBufferBindingType_Uniform;
    entries[1].binding = 1;
    entries[1].visibility = WGPUShaderStage_Fragment;
    entries[1].texture.sampleType = WGPUTextureSampleType_Float;
    entries[1].texture.viewDimension = WGPUTextureViewDimension_2D;
    for (uint32_t i = 2; i < 4; i++) {
        entries[i].binding = i;
        entries[i].visibility = WGPUShaderStage_Fragment;
        entries[i].sampler.type = WGPUSamplerBindingType_Filtering;
    }
    WGPUBindGroupLayoutDescriptor bld = WGPU_BIND_GROUP_LAYOUT_DESCRIPTOR_INIT;
    bld.entryCount = 4;
    bld.entries = entries;
    r->layout = wgpuDeviceCreateBindGroupLayout(device, &bld);
    WGPUPipelineLayoutDescriptor pld = WGPU_PIPELINE_LAYOUT_DESCRIPTOR_INIT;
    pld.bindGroupLayoutCount = 1;
    pld.bindGroupLayouts = &r->layout;
    WGPUPipelineLayout pipeline_layout = wgpuDeviceCreatePipelineLayout(device, &pld);

    /* The instance layout, read from the library at run time: (location,
     * floats, offset) triples, each a float32x2 or float32x4, stepped per
     * instance. A later library that moves it needs no change here. */
    uint32_t n = kui_instance_attributes(NULL, 0);
    uint32_t *triples = (uint32_t *)malloc((size_t)n * 3 * sizeof *triples);
    WGPUVertexAttribute *attrs = (WGPUVertexAttribute *)calloc(n, sizeof *attrs);
    kui_instance_attributes(triples, n);
    for (uint32_t i = 0; i < n; i++) {
        attrs[i].shaderLocation = triples[i * 3];
        attrs[i].format = triples[i * 3 + 1] == 2 ? WGPUVertexFormat_Float32x2 : WGPUVertexFormat_Float32x4;
        attrs[i].offset = triples[i * 3 + 2];
    }
    WGPUVertexBufferLayout vbl = WGPU_VERTEX_BUFFER_LAYOUT_INIT;
    vbl.stepMode = WGPUVertexStepMode_Instance;
    vbl.arrayStride = kui_instance_stride();
    vbl.attributeCount = n;
    vbl.attributes = attrs;

    /* As kui-wgpu blends. SINGLE writes straight colour with its coverage
     * as alpha: colour by its alpha, alpha over. DUAL writes premultiplied
     * colour and a per-channel coverage as the second source, out = src +
     * dst * (1 - coverage); on a quad whose channels' coverage is its
     * alpha that is premultiplied alpha blending. */
    WGPUBlendState blend;
    blend.color.operation = WGPUBlendOperation_Add;
    blend.color.srcFactor = WGPUBlendFactor_SrcAlpha;
    blend.color.dstFactor = WGPUBlendFactor_OneMinusSrcAlpha;
    blend.alpha.operation = WGPUBlendOperation_Add;
    blend.alpha.srcFactor = WGPUBlendFactor_One;
    blend.alpha.dstFactor = WGPUBlendFactor_OneMinusSrcAlpha;
    if (dual) {
        blend.color.srcFactor = WGPUBlendFactor_One;
        blend.color.dstFactor = WGPUBlendFactor_OneMinusSrc1;
        blend.alpha.dstFactor = WGPUBlendFactor_OneMinusSrc1Alpha;
    }
    WGPUColorTargetState target = WGPU_COLOR_TARGET_STATE_INIT;
    target.format = format;
    target.blend = &blend;
    WGPUFragmentState fs = WGPU_FRAGMENT_STATE_INIT;
    fs.module = module;
    fs.entryPoint.data = "fs_main";
    fs.entryPoint.length = WGPU_STRLEN;
    fs.targetCount = 1;
    fs.targets = &target;
    WGPURenderPipelineDescriptor rpd = WGPU_RENDER_PIPELINE_DESCRIPTOR_INIT;
    rpd.layout = pipeline_layout;
    rpd.vertex.module = module;
    rpd.vertex.entryPoint.data = "vs_main";
    rpd.vertex.entryPoint.length = WGPU_STRLEN;
    rpd.vertex.bufferCount = 1;
    rpd.vertex.buffers = &vbl;
    /* Six vertices an instance, a triangle list. */
    rpd.primitive.topology = WGPUPrimitiveTopology_TriangleList;
    rpd.fragment = &fs;
    r->pipeline = wgpuDeviceCreateRenderPipeline(device, &rpd);
    free(attrs);
    free(triples);
    wgpuPipelineLayoutRelease(pipeline_layout);
    wgpuShaderModuleRelease(module);

    /* Eight floats: viewport, atlas size, frame clock, scale, padding. */
    WGPUBufferDescriptor gd = WGPU_BUFFER_DESCRIPTOR_INIT;
    gd.usage = WGPUBufferUsage_Uniform | WGPUBufferUsage_CopyDst;
    gd.size = 8 * sizeof(float);
    r->globals = wgpuDeviceCreateBuffer(device, &gd);
    /* kui-wgpu's two samplers: clamped, one filtering and one not. */
    WGPUSamplerDescriptor sd = WGPU_SAMPLER_DESCRIPTOR_INIT;
    sd.magFilter = sd.minFilter = WGPUFilterMode_Linear;
    r->linear = wgpuDeviceCreateSampler(device, &sd);
    sd.magFilter = sd.minFilter = WGPUFilterMode_Nearest;
    r->nearest = wgpuDeviceCreateSampler(device, &sd);
    return r->pipeline != NULL;
}

/* A page of `size`, and the bind group that names it. */
static inline void kui_wgpu_atlas_page(KuiWgpu *r, uint32_t size) {
    if (r->atlas) {
        wgpuBindGroupRelease(r->group);
        wgpuTextureViewRelease(r->atlas_view);
        wgpuTextureRelease(r->atlas);
    }
    WGPUTextureDescriptor td = WGPU_TEXTURE_DESCRIPTOR_INIT;
    td.usage = WGPUTextureUsage_TextureBinding | WGPUTextureUsage_CopyDst;
    td.size.width = td.size.height = size;
    td.size.depthOrArrayLayers = 1;
    td.format = WGPUTextureFormat_RGBA8Unorm;
    r->atlas = wgpuDeviceCreateTexture(r->device, &td);
    r->atlas_view = wgpuTextureCreateView(r->atlas, NULL);
    r->atlas_size = size;
    r->atlas_epoch = 0;
    WGPUBindGroupEntry e[4] = {
        WGPU_BIND_GROUP_ENTRY_INIT, WGPU_BIND_GROUP_ENTRY_INIT,
        WGPU_BIND_GROUP_ENTRY_INIT, WGPU_BIND_GROUP_ENTRY_INIT,
    };
    e[0].binding = 0;
    e[0].buffer = r->globals;
    e[0].size = 8 * sizeof(float);
    e[1].binding = 1;
    e[1].textureView = r->atlas_view;
    e[2].binding = 2;
    e[2].sampler = r->linear;
    e[3].binding = 3;
    e[3].sampler = r->nearest;
    WGPUBindGroupDescriptor bd = WGPU_BIND_GROUP_DESCRIPTOR_INIT;
    bd.layout = r->layout;
    bd.entryCount = 4;
    bd.entries = e;
    r->group = wgpuDeviceCreateBindGroup(r->device, &bd);
}

/* The atlas: whole when the page is new to us, else the dirty rect alone.
 * The rect's rows are atlas_pixels' own, so the source starts at its
 * corner with the page's stride. */
static inline void kui_wgpu_atlas_upload(KuiWgpu *r, const KuiDrawData *dd) {
    uint32_t size = dd->atlas_size ? dd->atlas_size : 1;
    if (!r->atlas || size != r->atlas_size) kui_wgpu_atlas_page(r, size);
    memset(r->uploaded, 0, sizeof r->uploaded);
    if (!dd->atlas_size || !dd->atlas_pixels) return;
    uint32_t x = 0, y = 0, w = size, h = size;
    if (dd->atlas_epoch == r->atlas_epoch) {
        if (!dd->atlas_dirty) return;
        x = dd->atlas_dirty_rect[0], y = dd->atlas_dirty_rect[1];
        w = dd->atlas_dirty_rect[2], h = dd->atlas_dirty_rect[3];
        if (!w || !h) return;
    }
    WGPUTexelCopyTextureInfo dst = WGPU_TEXEL_COPY_TEXTURE_INFO_INIT;
    dst.texture = r->atlas;
    dst.origin.x = x;
    dst.origin.y = y;
    dst.origin.z = 0;
    WGPUTexelCopyBufferLayout layout = WGPU_TEXEL_COPY_BUFFER_LAYOUT_INIT;
    layout.bytesPerRow = size * 4;
    layout.rowsPerImage = h;
    size_t from = ((size_t)y * size + x) * 4;
    size_t bytes = (size_t)(h - 1) * size * 4 + (size_t)w * 4;
    WGPUExtent3D extent = {w, h, 1};
    wgpuQueueWriteTexture(r->queue, &dst, dd->atlas_pixels + from, bytes, &layout, &extent);
    r->atlas_epoch = dd->atlas_epoch;
    r->uploaded[0] = x, r->uploaded[1] = y, r->uploaded[2] = w, r->uploaded[3] = h;
}

/* A quad this pipeline draws, rather than one of the passes it leaves
 * out (the list at the top). */
static inline bool kui_wgpu_kind_drawn(uint32_t kind) {
    return kind != KUI_QUAD_TEXTURE && kind != KUI_QUAD_FRAGMENT && kind != KUI_QUAD_GROUP_END;
}

/* The instance buffer, grown to twice a frame that outgrew it, and the
 * memory the queue writes are packed into. */
static inline void kui_wgpu_grow(KuiWgpu *r, size_t count) {
    if (count <= r->instance_cap) return;
    uint32_t stride = kui_instance_stride();
    if (r->instances) wgpuBufferRelease(r->instances);
    r->instance_cap = count * 2;
    WGPUBufferDescriptor bd = WGPU_BUFFER_DESCRIPTOR_INIT;
    bd.usage = WGPUBufferUsage_Vertex | WGPUBufferUsage_CopyDst;
    bd.size = r->instance_cap * stride;
    r->instances = wgpuDeviceCreateBuffer(r->device, &bd);
    free(r->packed);
    r->packed = (uint8_t *)malloc(r->instance_cap * stride);
}

/* What every draw puts up first, with kui_wgpu_upload or without: the
 * atlas and the uniform through the queue, and the instance buffer grown to
 * the frame. Its instance count; 0 with nothing to draw (no finished frame,
 * or an empty one). */
static inline size_t kui_wgpu_prepare(KuiWgpu *r, KuiCtx *ctx, KuiDrawData *dd) {
    memset(dd, 0, sizeof *dd);
    dd->size = sizeof *dd;
    if (!r->pipeline || !kui_draw_data(ctx, dd)) return 0;
    kui_wgpu_atlas_upload(r, dd);
    float globals[8] = {dd->viewport_w, dd->viewport_h, (float)r->atlas_size, (float)r->atlas_size,
                        dd->time, dd->scale, 0, 0};
    wgpuQueueWriteBuffer(r->queue, r->globals, 0, globals, sizeof globals);
    size_t count = kui_pack_instances(ctx, NULL, 0);
    if (!count || !dd->bounds[2] || !dd->bounds[3]) return 0;
    kui_wgpu_grow(r, count);
    return count;
}

/* The instances through the queue, in pieces under a page. */
static inline void kui_wgpu_write_instances(KuiWgpu *r, KuiCtx *ctx, size_t bytes) {
    kui_pack_instances(ctx, r->packed, bytes);
    for (size_t off = 0; off < bytes; off += KUI_WGPU_WRITE_PIECE) {
        size_t n = bytes - off < KUI_WGPU_WRITE_PIECE ? bytes - off : KUI_WGPU_WRITE_PIECE;
        wgpuQueueWriteBuffer(r->queue, r->instances, off, r->packed + off, n);
    }
    r->instances_by = KUI_WGPU_WRITTEN;
}

/* A staging buffer's map, as its callback leaves it. The callback may run
 * on the device's thread (AllowSpontaneous), hence the exchange: of the
 * callback and kui_wgpu_free, the one that comes second frees the cell. */
enum { KUI_WGPU_CELL_PENDING = 0, KUI_WGPU_CELL_MAPPED = 1, KUI_WGPU_CELL_FAILED = 2, KUI_WGPU_CELL_DEAD = 3 };
#if defined(_MSC_VER) && !defined(__clang__)
#include <intrin.h>
#define KUI_WGPU_XCHG(p, v) ((int)_InterlockedExchange((volatile long *)(p), (long)(v)))
#define KUI_WGPU_LOAD(p) ((int)_InterlockedCompareExchange((volatile long *)(p), 0, 0))
#else
#define KUI_WGPU_XCHG(p, v) __atomic_exchange_n((p), (v), __ATOMIC_ACQ_REL)
#define KUI_WGPU_LOAD(p) __atomic_load_n((p), __ATOMIC_ACQUIRE)
#endif

static void kui_wgpu_mapped(WGPUMapAsyncStatus status, WGPUStringView message, void *cell, void *unused) {
    (void)message, (void)unused;
    int now = status == WGPUMapAsyncStatus_Success ? KUI_WGPU_CELL_MAPPED : KUI_WGPU_CELL_FAILED;
    if (KUI_WGPU_XCHG((int *)cell, now) == KUI_WGPU_CELL_DEAD) free(cell);
}

/* One staging buffer gone: released, and its cell freed, or left to the
 * callback still to come. */
static inline void kui_wgpu_staging_drop(KuiWgpuStaging *s) {
    if (s->phase != 3 || KUI_WGPU_XCHG(s->cell, KUI_WGPU_CELL_DEAD) != KUI_WGPU_CELL_PENDING) free(s->cell);
    if (s->buffer) wgpuBufferRelease(s->buffer);
    memset(s, 0, sizeof *s);
}

/* The instances packed into a mapped staging buffer, its copy recorded on
 * `enc`. Before picking one: the buffers last frame copied from (the host
 * has submitted since) are asked to map, and those whose map has come back
 * are free again. False when none is free, or one cannot be made. */
static inline bool kui_wgpu_stage(KuiWgpu *r, KuiCtx *ctx, size_t bytes, WGPUCommandEncoder enc) {
    size_t cap = r->instance_cap * kui_instance_stride();
    if (cap != r->ring_cap) {
        for (int i = 0; i < KUI_WGPU_RING; i++) kui_wgpu_staging_drop(&r->ring[i]);
        r->ring_cap = cap;
    }
    KuiWgpuStaging *pick = NULL;
    for (int i = 0; i < KUI_WGPU_RING; i++) {
        KuiWgpuStaging *s = &r->ring[i];
        if (s->phase == 2) {
            s->cell = (int *)malloc(sizeof *s->cell);
            if (!s->cell) {
                kui_wgpu_staging_drop(s);
                continue;
            }
            *s->cell = KUI_WGPU_CELL_PENDING;
            s->phase = 3;
            WGPUBufferMapCallbackInfo info = WGPU_BUFFER_MAP_CALLBACK_INFO_INIT;
            info.mode = WGPUCallbackMode_AllowSpontaneous;
            info.callback = kui_wgpu_mapped;
            info.userdata1 = s->cell;
            wgpuBufferMapAsync(s->buffer, WGPUMapMode_Write, 0, cap, info);
        }
        if (s->phase == 3) {
            int cell = KUI_WGPU_LOAD(s->cell);
            if (cell == KUI_WGPU_CELL_MAPPED) {
                free(s->cell);
                s->cell = NULL;
                s->phase = 1;
            } else if (cell == KUI_WGPU_CELL_FAILED) {
                kui_wgpu_staging_drop(s);
            }
        }
        if (!pick && s->phase == 1) pick = s;
    }
    for (int i = 0; i < KUI_WGPU_RING && !pick; i++) {
        KuiWgpuStaging *s = &r->ring[i];
        if (s->phase != 0) continue;
        WGPUBufferDescriptor bd = WGPU_BUFFER_DESCRIPTOR_INIT;
        bd.usage = WGPUBufferUsage_MapWrite | WGPUBufferUsage_CopySrc;
        bd.size = cap;
        bd.mappedAtCreation = true;
        s->buffer = wgpuDeviceCreateBuffer(r->device, &bd);
        if (!s->buffer) return false;
        s->phase = 1;
        pick = s;
    }
    if (!pick) return false;
    uint8_t *dst = (uint8_t *)wgpuBufferGetMappedRange(pick->buffer, 0, cap);
    if (!dst) return false;
    kui_pack_instances(ctx, dst, bytes);
    wgpuBufferUnmap(pick->buffer);
    wgpuCommandEncoderCopyBufferToBuffer(enc, pick->buffer, 0, r->instances, 0, bytes);
    pick->phase = 2;
    return true;
}

/* `ctx`'s last finished frame put up on `enc`, before the host begins the
 * pass kui_wgpu_draw goes into (backlog F277): the atlas and the uniform
 * through the queue, the instances packed into a staging buffer of the ring
 * and copied on the encoder - or, with every staging buffer still in
 * flight, through the queue in pieces. Then kui_wgpu_draw records the
 * draws alone. Once a frame, after the last frame's submit: a staging
 * buffer is asked to map again at the next call. */
static inline void kui_wgpu_upload(KuiWgpu *r, KuiCtx *ctx, WGPUCommandEncoder enc) {
    KuiDrawData dd;
    r->staged = 0;
    size_t count = kui_wgpu_prepare(r, ctx, &dd);
    if (!count) return;
    size_t bytes = count * kui_instance_stride();
    if (kui_wgpu_stage(r, ctx, bytes, enc))
        r->instances_by = KUI_WGPU_STAGED;
    else
        kui_wgpu_write_instances(r, ctx, bytes);
    r->staged = count;
}

/* `ctx`'s last finished frame into `pass`, over what it holds, scissored to
 * KuiDrawData.bounds. After kui_wgpu_upload, what it put up; without one,
 * the atlas, uniform and instances written through the queue first. A
 * frame finished between the two is not drawn (its instances would be
 * overwritten by the upload's copy): upload it again. Returns the instances
 * drawn; 0 with nothing to draw (no finished frame, or an empty one),
 * which records nothing into the pass. */
static inline size_t kui_wgpu_draw(KuiWgpu *r, KuiCtx *ctx, WGPURenderPassEncoder pass) {
    if (!r->pipeline) return 0;
    KuiDrawData dd;
    size_t count, staged = r->staged;
    r->staged = 0;
    if (staged) {
        memset(&dd, 0, sizeof dd);
        dd.size = sizeof dd;
        if (!kui_draw_data(ctx, &dd)) return 0;
        count = kui_pack_instances(ctx, NULL, 0);
        if (count != staged) return 0;
    } else {
        count = kui_wgpu_prepare(r, ctx, &dd);
        if (!count) return 0;
        kui_wgpu_write_instances(r, ctx, count * kui_instance_stride());
    }
    size_t bytes = count * kui_instance_stride();

    /* Only where the frame drew: what is outside it stays the host's. */
    wgpuRenderPassEncoderSetScissorRect(pass, dd.bounds[0], dd.bounds[1], dd.bounds[2], dd.bounds[3]);
    wgpuRenderPassEncoderSetPipeline(pass, r->pipeline);
    wgpuRenderPassEncoderSetBindGroup(pass, 0, r->group, 0, NULL);
    wgpuRenderPassEncoderSetVertexBuffer(pass, 0, r->instances, 0, bytes);
    /* The runs between the quads it leaves out, in order: one draw for
     * the usual frame, which has none of them. */
    size_t drawn = 0, run = 0;
    for (size_t i = 0; i <= count; i++) {
        if (i < count && kui_wgpu_kind_drawn(dd.quads[i].kind)) continue;
        if (i > run) {
            wgpuRenderPassEncoderDraw(pass, 6, (uint32_t)(i - run), 0, (uint32_t)run);
            drawn += i - run;
        }
        run = i + 1;
    }
    return drawn;
}

/* Everything kui_wgpu_init and the draws made; `r` zeroed after, so a
 * second free, or one after a failed init, does nothing. The device and
 * queue are the host's and stay. A staging buffer still waiting for its
 * map leaves its cell to the callback. */
static inline void kui_wgpu_free(KuiWgpu *r) {
    for (int i = 0; i < KUI_WGPU_RING; i++) kui_wgpu_staging_drop(&r->ring[i]);
    free(r->packed);
    if (r->instances) wgpuBufferRelease(r->instances);
    if (r->group) wgpuBindGroupRelease(r->group);
    if (r->atlas_view) wgpuTextureViewRelease(r->atlas_view);
    if (r->atlas) wgpuTextureRelease(r->atlas);
    if (r->nearest) wgpuSamplerRelease(r->nearest);
    if (r->linear) wgpuSamplerRelease(r->linear);
    if (r->globals) wgpuBufferRelease(r->globals);
    if (r->pipeline) wgpuRenderPipelineRelease(r->pipeline);
    if (r->layout) wgpuBindGroupLayoutRelease(r->layout);
    memset(r, 0, sizeof *r);
}

#endif
