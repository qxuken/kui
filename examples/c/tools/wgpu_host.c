/* A host that draws kui's frame with a GPU device of its own (backlog
 * F234), on wgpu-native's C API (webgpu.h): kui.h's "Drawing it yourself"
 * done end to end, offscreen. No window and no surface - an instance, an
 * adapter, a device - so it runs on a machine with a display and on CI's
 * software Vulkan alike.
 *
 * Nothing here knows kui's instance layout or shader: the WGSL comes from
 * kui_renderer_wgsl, the vertex buffer's layout from kui_instance_stride
 * and kui_instance_attributes, the instances from kui_pack_instances. A
 * later library that moves any of them needs no change here. What this
 * file does know is what kui.h states and kui-wgpu does
 * (crates/kui-wgpu/src/lib.rs): the bind group, the blend, an RGBA8Unorm
 * target whose bytes are kui's colours as written (no sRGB encode).
 *
 * The frame is drawn, read back and checked: the panel's colour inside the
 * panel, the clear colour far outside it, a shadow under it, glyph coverage
 * where its title is, the button's colour on the button. Then the button
 * is clicked, the frame rebuilt with its new label, and drawn again: the
 * label's new glyph reaches the atlas as a dirty rect, uploaded alone.
 *
 * It calls only the core library (no kui_run, no capture), so it links
 * against the release's no-runner library as well.
 *
 *     cargo run -p kui-devtools --bin cbuild -- --run   builds and runs it
 *
 * cbuild builds it where it finds a wgpu-native: WGPU_NATIVE_DIR (holding
 * include/ and lib/), else the one Odin ships in vendor/wgpu/lib. With no
 * GPU adapter it says so on stderr and exits 0, unless KUI_REQUIRE_GPU is
 * set (CI sets it over lavapipe): then that is a failure, and an adapter
 * whose name lacks that value is one too.
 */
#include <stdlib.h>
#include <webgpu/webgpu.h>
#include <webgpu/wgpu.h>
#include "../common.h"

/* The frame, in logical px, drawn at twice that. */
#define W 320
#define H 200
#define SCALE 2
#define PANEL_X 20
#define PANEL_Y 20
#define PANEL_W 200
#define PANEL_H 120
#define BUTTON_W 112
#define BUTTON_H 28
#define PANEL_BG 0x2a3550ffu
#define BUTTON_BG 0xe0703affu
/* The world behind the UI: what the target is cleared to. */
static const WGPUColor CLEAR = {0.10, 0.55, 0.35, 1.0};

/* --- the view --------------------------------------------------------- */

/* A panel with a shadow, a title and a button, on a transparent root. */
static void build(KuiCtx *ui, int clicks) {
    kui_frame_begin(ui, W, H, SCALE);
    KuiSpec world = {.width = {KUI_FIXED, W}, .height = {KUI_FIXED, H}, .pad_l = PANEL_X, .pad_t = PANEL_Y};
    kui_open_keyed(ui, KUI_STR("world"), &world, NULL);
    KuiSpec panel = {.width = {KUI_FIXED, PANEL_W}, .height = {KUI_FIXED, PANEL_H}, .dir = KUI_COLUMN,
                     .pad_l = 16, .pad_r = 16, .pad_t = 16, .pad_b = 16, .gap = 12,
                     .bg = PANEL_BG, .radius = 8,
                     .shadow_color = 0x00000099u, .shadow_blur = 10, .shadow_y = 4};
    kui_open_keyed(ui, KUI_STR("panel"), &panel, NULL);
    KuiTextStyle title = {.size = 18, .color = 0xffffffffu};
    kui_text(ui, KUI_STR("kui over wgpu"), &title);
    KuiSpec button = {.width = {KUI_FIXED, BUTTON_W}, .height = {KUI_FIXED, BUTTON_H},
                      .main_align = KUI_CENTER, .cross_align = KUI_CENTER,
                      .bg = BUTTON_BG, .radius = 6, .label = KUI_STR("count")};
    /* The click message is the node's to keep, one a frame. */
    kui_open_keyed(ui, KUI_STR("count"), &button, kui_value_str(KUI_STR("count")));
    char label[32];
    snprintf(label, sizeof label, "clicked %d", clicks);
    KuiTextStyle small = {.size = 13, .color = 0xffffffffu};
    kui_text(ui, KUI_STR(label), &small);
    kui_close(ui);
    kui_close(ui);
    kui_close(ui);
    kui_frame_finish(ui);
}

/* Drains the queue; how many clicks it held. */
static int drain(KuiCtx *ui) {
    int clicks = 0;
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ui, &ev)) {
        KuiStr msg = {0};
        if (ev_is(&ev, "click") && ev_message(&ev, &msg) && kui_str_eq(msg, "count")) clicks++;
    }
    return clicks;
}

/* --- the device ------------------------------------------------------- */

static int wgpu_errors;

static void print_view(const char *what, WGPUStringView s) {
    size_t n = s.length == WGPU_STRLEN ? (s.data ? strlen(s.data) : 0) : s.length;
    fprintf(stderr, "%s%.*s\n", what, (int)n, s.data ? s.data : "");
}

/* Every validation error the device raises lands here: a failure, since a
 * reference host that draws with one has drawn nothing worth checking. */
static void on_error(WGPUDevice const *device, WGPUErrorType type, WGPUStringView message, void *u1, void *u2) {
    (void)device, (void)u1, (void)u2;
    fprintf(stderr, "FAIL: wgpu error (type %d): ", (int)type);
    print_view("", message);
    wgpu_errors++;
}

typedef struct Answer {
    bool done;
    bool ok;
    void *handle;
} Answer;

static void on_adapter(WGPURequestAdapterStatus status, WGPUAdapter adapter, WGPUStringView message, void *u1, void *u2) {
    (void)u2;
    Answer *a = u1;
    a->done = true;
    a->ok = status == WGPURequestAdapterStatus_Success && adapter;
    a->handle = adapter;
    if (!a->ok) print_view("adapter: ", message);
}

static void on_device(WGPURequestDeviceStatus status, WGPUDevice device, WGPUStringView message, void *u1, void *u2) {
    (void)u2;
    Answer *a = u1;
    a->done = true;
    a->ok = status == WGPURequestDeviceStatus_Success && device;
    a->handle = device;
    if (!a->ok) print_view("device: ", message);
}

static void on_mapped(WGPUMapAsyncStatus status, WGPUStringView message, void *u1, void *u2) {
    (void)u2;
    Answer *a = u1;
    a->done = true;
    a->ok = status == WGPUMapAsyncStatus_Success;
    if (!a->ok) print_view("map: ", message);
}

/* wgpu-native answers a request inside the call; the loop is for an
 * implementation that answers later, through the instance's events. */
static void wait_for(WGPUInstance instance, const Answer *a) {
    for (int i = 0; !a->done && i < 10000; i++) wgpuInstanceProcessEvents(instance);
}

typedef struct Gpu {
    WGPUInstance instance;
    WGPUAdapter adapter;
    WGPUDevice device;
    WGPUQueue queue;
    char adapter_name[128];
} Gpu;

/* An instance, an adapter and a device, with no surface. False with no
 * adapter: the machine has no GPU (or no driver for one). */
static bool gpu_open(Gpu *g) {
    g->instance = wgpuCreateInstance(NULL);
    if (!g->instance) return false;
    Answer a = {0};
    WGPURequestAdapterCallbackInfo acb = WGPU_REQUEST_ADAPTER_CALLBACK_INFO_INIT;
    acb.mode = WGPUCallbackMode_AllowProcessEvents;
    acb.callback = on_adapter;
    acb.userdata1 = &a;
    wgpuInstanceRequestAdapter(g->instance, NULL, acb);
    wait_for(g->instance, &a);
    if (!a.ok) return false;
    g->adapter = a.handle;

    WGPUAdapterInfo info = WGPU_ADAPTER_INFO_INIT;
    if (wgpuAdapterGetInfo(g->adapter, &info) == WGPUStatus_Success) {
        size_t n = info.device.length == WGPU_STRLEN ? strlen(info.device.data) : info.device.length;
        if (n >= sizeof g->adapter_name) n = sizeof g->adapter_name - 1;
        memcpy(g->adapter_name, info.device.data, n);
        g->adapter_name[n] = 0;
        wgpuAdapterInfoFreeMembers(info);
    }

    Answer d = {0};
    WGPUDeviceDescriptor desc = WGPU_DEVICE_DESCRIPTOR_INIT;
    desc.uncapturedErrorCallbackInfo.callback = on_error;
    WGPURequestDeviceCallbackInfo dcb = WGPU_REQUEST_DEVICE_CALLBACK_INFO_INIT;
    dcb.mode = WGPUCallbackMode_AllowProcessEvents;
    dcb.callback = on_device;
    dcb.userdata1 = &d;
    wgpuAdapterRequestDevice(g->adapter, &desc, dcb);
    wait_for(g->instance, &d);
    if (!d.ok) return false;
    g->device = d.handle;
    g->queue = wgpuDeviceGetQueue(g->device);
    return true;
}

static void gpu_close(Gpu *g) {
    if (g->queue) wgpuQueueRelease(g->queue);
    if (g->device) wgpuDeviceRelease(g->device);
    if (g->adapter) wgpuAdapterRelease(g->adapter);
    if (g->instance) wgpuInstanceRelease(g->instance);
}

/* --- kui's renderer, built from what the library says ----------------- */

typedef struct Renderer {
    WGPUBindGroupLayout layout;
    WGPURenderPipeline pipeline;
    WGPUBuffer globals;
    WGPUSampler linear, nearest;
    /* The glyph atlas, kept between frames: a page the size the draw data
     * says, and the epoch of the page it holds. */
    WGPUTexture atlas;
    WGPUTextureView atlas_view;
    uint32_t atlas_size;
    uint64_t atlas_epoch;
    WGPUBindGroup group;
    WGPUBuffer instances;
    size_t instance_cap;
    /* The offscreen target, and the buffer it is read back through. */
    WGPUTexture target;
    WGPUTextureView target_view;
    WGPUBuffer readback;
    uint32_t width, height, row;
    /* What the last frame uploaded of the atlas, for the summary. */
    uint32_t uploaded[4];
} Renderer;

static const WGPUTextureFormat TARGET = WGPUTextureFormat_RGBA8Unorm;

static bool renderer_init(Gpu *g, Renderer *r, uint32_t width, uint32_t height) {
    memset(r, 0, sizeof *r);
    /* The shader without dual-source blending: subpixel text stays off (it
     * is unless kui_set_subpixel_text), so no glyph needs per-channel
     * blending. */
    KuiStr wgsl = kui_renderer_wgsl(KUI_WGSL_SINGLE);
    WGPUShaderSourceWGSL src = WGPU_SHADER_SOURCE_WGSL_INIT;
    src.code = (WGPUStringView){.data = (const char *)wgsl.ptr, .length = wgsl.len};
    WGPUShaderModuleDescriptor md = WGPU_SHADER_MODULE_DESCRIPTOR_INIT;
    md.nextInChain = &src.chain;
    WGPUShaderModule module = wgpuDeviceCreateShaderModule(g->device, &md);

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
    for (int i = 2; i < 4; i++) {
        entries[i].binding = (uint32_t)i;
        entries[i].visibility = WGPUShaderStage_Fragment;
        entries[i].sampler.type = WGPUSamplerBindingType_Filtering;
    }
    WGPUBindGroupLayoutDescriptor bld = WGPU_BIND_GROUP_LAYOUT_DESCRIPTOR_INIT;
    bld.entryCount = 4;
    bld.entries = entries;
    r->layout = wgpuDeviceCreateBindGroupLayout(g->device, &bld);
    WGPUPipelineLayoutDescriptor pld = WGPU_PIPELINE_LAYOUT_DESCRIPTOR_INIT;
    pld.bindGroupLayoutCount = 1;
    pld.bindGroupLayouts = &r->layout;
    WGPUPipelineLayout pipeline_layout = wgpuDeviceCreatePipelineLayout(g->device, &pld);

    /* The instance layout, read from the library at run time: (location,
     * floats, offset) triples, each a float32x2 or float32x4, stepped per
     * instance. */
    uint32_t n = kui_instance_attributes(NULL, 0);
    uint32_t *triples = malloc(n * 3 * sizeof *triples);
    WGPUVertexAttribute *attrs = calloc(n, sizeof *attrs);
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

    /* The single variant writes straight colour and its coverage as alpha:
     * blended as kui-wgpu blends it, colour by its alpha, alpha over. */
    WGPUBlendState blend = {
        .color = {.operation = WGPUBlendOperation_Add, .srcFactor = WGPUBlendFactor_SrcAlpha,
                  .dstFactor = WGPUBlendFactor_OneMinusSrcAlpha},
        .alpha = {.operation = WGPUBlendOperation_Add, .srcFactor = WGPUBlendFactor_One,
                  .dstFactor = WGPUBlendFactor_OneMinusSrcAlpha},
    };
    WGPUColorTargetState target = WGPU_COLOR_TARGET_STATE_INIT;
    target.format = TARGET;
    target.blend = &blend;
    WGPUFragmentState fs = WGPU_FRAGMENT_STATE_INIT;
    fs.module = module;
    fs.entryPoint = (WGPUStringView){.data = "fs_main", .length = WGPU_STRLEN};
    fs.targetCount = 1;
    fs.targets = &target;
    WGPURenderPipelineDescriptor rpd = WGPU_RENDER_PIPELINE_DESCRIPTOR_INIT;
    rpd.layout = pipeline_layout;
    rpd.vertex.module = module;
    rpd.vertex.entryPoint = (WGPUStringView){.data = "vs_main", .length = WGPU_STRLEN};
    rpd.vertex.bufferCount = 1;
    rpd.vertex.buffers = &vbl;
    /* Six vertices an instance, a triangle list (the default topology). */
    rpd.primitive.topology = WGPUPrimitiveTopology_TriangleList;
    rpd.fragment = &fs;
    r->pipeline = wgpuDeviceCreateRenderPipeline(g->device, &rpd);
    free(attrs);
    free(triples);
    wgpuPipelineLayoutRelease(pipeline_layout);
    wgpuShaderModuleRelease(module);

    /* Eight floats: viewport, atlas size, frame clock, scale, padding. */
    WGPUBufferDescriptor gd = WGPU_BUFFER_DESCRIPTOR_INIT;
    gd.usage = WGPUBufferUsage_Uniform | WGPUBufferUsage_CopyDst;
    gd.size = 8 * sizeof(float);
    r->globals = wgpuDeviceCreateBuffer(g->device, &gd);
    /* kui-wgpu's two samplers: clamped, one filtering and one not. */
    WGPUSamplerDescriptor sd = WGPU_SAMPLER_DESCRIPTOR_INIT;
    sd.magFilter = sd.minFilter = WGPUFilterMode_Linear;
    r->linear = wgpuDeviceCreateSampler(g->device, &sd);
    sd.magFilter = sd.minFilter = WGPUFilterMode_Nearest;
    r->nearest = wgpuDeviceCreateSampler(g->device, &sd);

    /* The target and its readback; a copy's rows are 256-byte aligned. */
    r->width = width;
    r->height = height;
    r->row = (width * 4 + 255) & ~255u;
    WGPUTextureDescriptor td = WGPU_TEXTURE_DESCRIPTOR_INIT;
    td.usage = WGPUTextureUsage_RenderAttachment | WGPUTextureUsage_CopySrc;
    td.size = (WGPUExtent3D){width, height, 1};
    td.format = TARGET;
    r->target = wgpuDeviceCreateTexture(g->device, &td);
    r->target_view = wgpuTextureCreateView(r->target, NULL);
    WGPUBufferDescriptor rd = WGPU_BUFFER_DESCRIPTOR_INIT;
    rd.usage = WGPUBufferUsage_CopyDst | WGPUBufferUsage_MapRead;
    rd.size = (uint64_t)r->row * height;
    r->readback = wgpuDeviceCreateBuffer(g->device, &rd);
    return wgpu_errors == 0;
}

/* A page of `size`, and the bind group that names it. */
static void atlas_resize(Gpu *g, Renderer *r, uint32_t size) {
    if (r->atlas) {
        wgpuBindGroupRelease(r->group);
        wgpuTextureViewRelease(r->atlas_view);
        wgpuTextureRelease(r->atlas);
    }
    WGPUTextureDescriptor td = WGPU_TEXTURE_DESCRIPTOR_INIT;
    td.usage = WGPUTextureUsage_TextureBinding | WGPUTextureUsage_CopyDst;
    td.size = (WGPUExtent3D){size, size, 1};
    td.format = WGPUTextureFormat_RGBA8Unorm;
    r->atlas = wgpuDeviceCreateTexture(g->device, &td);
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
    r->group = wgpuDeviceCreateBindGroup(g->device, &bd);
}

/* The atlas: whole when the page is new to us, else the dirty rect alone.
 * The rect's rows are atlas_pixels' own, so the source starts at its
 * corner with the page's stride. */
static void atlas_upload(Gpu *g, Renderer *r, const KuiDrawData *dd) {
    uint32_t size = dd->atlas_size ? dd->atlas_size : 1;
    if (!r->atlas || size != r->atlas_size) atlas_resize(g, r, size);
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
    dst.origin = (WGPUOrigin3D){x, y, 0};
    WGPUTexelCopyBufferLayout layout = WGPU_TEXEL_COPY_BUFFER_LAYOUT_INIT;
    layout.bytesPerRow = size * 4;
    layout.rowsPerImage = h;
    size_t from = ((size_t)y * size + x) * 4;
    size_t bytes = (size_t)(h - 1) * size * 4 + (size_t)w * 4;
    WGPUExtent3D extent = {w, h, 1};
    wgpuQueueWriteTexture(g->queue, &dst, dd->atlas_pixels + from, bytes, &layout, &extent);
    r->atlas_epoch = dd->atlas_epoch;
    r->uploaded[0] = x, r->uploaded[1] = y, r->uploaded[2] = w, r->uploaded[3] = h;
}

/* kui's last finished frame into the target, read back into `rgba`
 * (r->row bytes a row). Returns the number of instances drawn. */
static size_t render(Gpu *g, Renderer *r, KuiCtx *ui, uint8_t *rgba) {
    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    if (!kui_draw_data(ui, &dd)) return 0;
    atlas_upload(g, r, &dd);

    float globals[8] = {dd.viewport_w, dd.viewport_h, (float)r->atlas_size, (float)r->atlas_size,
                        dd.time, dd.scale, 0, 0};
    wgpuQueueWriteBuffer(g->queue, r->globals, 0, globals, sizeof globals);

    uint32_t stride = kui_instance_stride();
    size_t count = kui_pack_instances(ui, NULL, 0);
    size_t bytes = count * stride;
    if (count > r->instance_cap) {
        if (r->instances) wgpuBufferRelease(r->instances);
        r->instance_cap = count * 2;
        WGPUBufferDescriptor bd = WGPU_BUFFER_DESCRIPTOR_INIT;
        bd.usage = WGPUBufferUsage_Vertex | WGPUBufferUsage_CopyDst;
        bd.size = r->instance_cap * stride;
        r->instances = wgpuDeviceCreateBuffer(g->device, &bd);
    }
    if (count) {
        uint8_t *packed = malloc(bytes);
        kui_pack_instances(ui, packed, bytes);
        wgpuQueueWriteBuffer(g->queue, r->instances, 0, packed, bytes);
        free(packed);
    }

    WGPUCommandEncoder enc = wgpuDeviceCreateCommandEncoder(g->device, NULL);
    WGPURenderPassColorAttachment color = WGPU_RENDER_PASS_COLOR_ATTACHMENT_INIT;
    color.view = r->target_view;
    color.loadOp = WGPULoadOp_Clear;
    color.storeOp = WGPUStoreOp_Store;
    color.clearValue = CLEAR;
    WGPURenderPassDescriptor rp = WGPU_RENDER_PASS_DESCRIPTOR_INIT;
    rp.colorAttachmentCount = 1;
    rp.colorAttachments = &color;
    WGPURenderPassEncoder pass = wgpuCommandEncoderBeginRenderPass(enc, &rp);
    /* The draw scissored to where the frame drew (KuiDrawData.bounds):
     * nothing outside it is touched, so the world shows through. */
    if (count && dd.bounds[2] && dd.bounds[3]) {
        wgpuRenderPassEncoderSetScissorRect(pass, dd.bounds[0], dd.bounds[1], dd.bounds[2], dd.bounds[3]);
        wgpuRenderPassEncoderSetPipeline(pass, r->pipeline);
        wgpuRenderPassEncoderSetBindGroup(pass, 0, r->group, 0, NULL);
        wgpuRenderPassEncoderSetVertexBuffer(pass, 0, r->instances, 0, bytes);
        wgpuRenderPassEncoderDraw(pass, 6, (uint32_t)count, 0, 0);
    }
    wgpuRenderPassEncoderEnd(pass);
    wgpuRenderPassEncoderRelease(pass);

    WGPUTexelCopyTextureInfo src = WGPU_TEXEL_COPY_TEXTURE_INFO_INIT;
    src.texture = r->target;
    WGPUTexelCopyBufferInfo dst = WGPU_TEXEL_COPY_BUFFER_INFO_INIT;
    dst.buffer = r->readback;
    dst.layout.bytesPerRow = r->row;
    dst.layout.rowsPerImage = r->height;
    WGPUExtent3D extent = {r->width, r->height, 1};
    wgpuCommandEncoderCopyTextureToBuffer(enc, &src, &dst, &extent);
    WGPUCommandBuffer cmd = wgpuCommandEncoderFinish(enc, NULL);
    wgpuQueueSubmit(g->queue, 1, &cmd);
    wgpuCommandBufferRelease(cmd);
    wgpuCommandEncoderRelease(enc);

    size_t size = (size_t)r->row * r->height;
    Answer m = {0};
    WGPUBufferMapCallbackInfo mcb = WGPU_BUFFER_MAP_CALLBACK_INFO_INIT;
    mcb.mode = WGPUCallbackMode_AllowProcessEvents;
    mcb.callback = on_mapped;
    mcb.userdata1 = &m;
    wgpuBufferMapAsync(r->readback, WGPUMapMode_Read, 0, size, mcb);
    wgpuDevicePoll(g->device, true, NULL);
    wait_for(g->instance, &m);
    if (!m.ok) {
        check(false, "the readback mapped");
        return 0;
    }
    memcpy(rgba, wgpuBufferGetConstMappedRange(r->readback, 0, size), size);
    wgpuBufferUnmap(r->readback);
    return count;
}

static void renderer_free(Renderer *r) {
    if (r->instances) wgpuBufferRelease(r->instances);
    if (r->group) wgpuBindGroupRelease(r->group);
    if (r->atlas_view) wgpuTextureViewRelease(r->atlas_view);
    if (r->atlas) wgpuTextureRelease(r->atlas);
    wgpuBufferRelease(r->readback);
    wgpuTextureViewRelease(r->target_view);
    wgpuTextureRelease(r->target);
    wgpuSamplerRelease(r->nearest);
    wgpuSamplerRelease(r->linear);
    wgpuBufferRelease(r->globals);
    wgpuRenderPipelineRelease(r->pipeline);
    wgpuBindGroupLayoutRelease(r->layout);
}

/* --- the checks ------------------------------------------------------- */

static const uint8_t *pixel(const Renderer *r, const uint8_t *rgba, float x, float y) {
    return rgba + (size_t)y * r->row + (size_t)x * 4;
}

/* Within `tol` per channel of 0xRRGGBB. */
static bool near(const uint8_t *p, uint32_t rgb, int tol) {
    for (int c = 0; c < 3; c++) {
        int want = (int)((rgb >> (16 - 8 * c)) & 0xff);
        if (abs(p[c] - want) > tol) return false;
    }
    return true;
}

static uint32_t clear_rgb(void) {
    return (uint32_t)(CLEAR.r * 255 + 0.5) << 16 | (uint32_t)(CLEAR.g * 255 + 0.5) << 8 | (uint32_t)(CLEAR.b * 255 + 0.5);
}

/* The quad the button's background is: the solid of its size. */
static const KuiQuad *button_quad(const KuiDrawData *dd) {
    for (size_t i = 0; i < dd->quad_count; i++) {
        const KuiQuad *q = &dd->quads[i];
        if (q->kind == KUI_QUAD_SOLID && q->w == BUTTON_W * SCALE && q->h == BUTTON_H * SCALE) return q;
    }
    return NULL;
}

/* What every drawn frame shows, wherever its pixels came from. */
static void check_frame(KuiCtx *ui, const Renderer *r, const uint8_t *rgba, const char *when) {
    char what[160];
    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ui, &dd);
#define CHECK(ok, msg) (snprintf(what, sizeof what, "%s: %s", when, msg), check(ok, what))

    /* The panel's colour at its right edge, clear of the text. */
    const uint8_t *p = pixel(r, rgba, (PANEL_X + PANEL_W - 6) * SCALE, (PANEL_Y + PANEL_H / 2) * SCALE);
    CHECK(near(p, PANEL_BG >> 8, 2), "the panel is its colour");
    /* The world far from it. */
    p = pixel(r, rgba, r->width - 10, r->height - 10);
    CHECK(near(p, clear_rgb(), 2), "past the panel is the clear colour");
    /* The shadow, just under the panel's bottom edge: darker than the
     * world it falls on. */
    p = pixel(r, rgba, (PANEL_X + PANEL_W / 2) * SCALE, (PANEL_Y + PANEL_H + 3) * SCALE);
    uint32_t c = clear_rgb();
    int world = (int)((c >> 16) + ((c >> 8) & 0xff) + (c & 0xff));
    CHECK(p[0] + p[1] + p[2] < world * 9 / 10, "the shadow darkens the world under the panel");

    /* The title: glyph quads in the panel's upper half, and pixels inside
     * their union that are not the flat panel. */
    float top = (PANEL_Y + PANEL_H / 2) * SCALE;
    float x0 = 1e9f, y0 = 1e9f, x1 = -1e9f, y1 = -1e9f;
    for (size_t i = 0; i < dd.quad_count; i++) {
        const KuiQuad *q = &dd.quads[i];
        if ((q->kind != KUI_QUAD_GLYPH_MASK && q->kind != KUI_QUAD_GLYPH_SUBPIXEL) || q->y + q->h > top) continue;
        if (q->x < x0) x0 = q->x;
        if (q->y < y0) y0 = q->y;
        if (q->x + q->w > x1) x1 = q->x + q->w;
        if (q->y + q->h > y1) y1 = q->y + q->h;
    }
    int inked = 0;
    for (float y = y0; y < y1; y++)
        for (float x = x0; x < x1; x++)
            if (!near(pixel(r, rgba, x, y), PANEL_BG >> 8, 40)) inked++;
    CHECK(x1 > x0 && inked >= 30, "the title's glyphs are drawn (atlas uploaded, sampled)");

    /* The button's colour at its left edge, clear of its label. */
    const KuiQuad *b = button_quad(&dd);
    CHECK(b != NULL, "the button's quad is in the frame");
    if (b) CHECK(near(pixel(r, rgba, b->x + 6, b->y + b->h / 2), BUTTON_BG >> 8, 2), "the button is its colour");
#undef CHECK
}

int main(void) {
    if (!abi_ok()) return 1;
    const char *need = getenv("KUI_REQUIRE_GPU");
    bool required = need && *need;

    Gpu g = {0};
    if (!gpu_open(&g)) {
        gpu_close(&g);
        if (required) {
            fprintf(stderr, "FAIL: KUI_REQUIRE_GPU is set, but wgpu found no adapter\n");
            return 1;
        }
        fprintf(stderr, "no GPU adapter: wgpu_host has nothing to draw on\n");
        return 0;
    }
    if (required && !strstr(g.adapter_name, need)) {
        fprintf(stderr, "FAIL: KUI_REQUIRE_GPU is %s, but the adapter is %s\n", need, g.adapter_name);
        gpu_close(&g);
        return 1;
    }

    KuiCtx *ui = kui_ctx_new();
    Renderer r;
    check(renderer_init(&g, &r, W * SCALE, H * SCALE), "the pipeline built from kui's shader and layout");
    uint8_t *rgba = malloc((size_t)r.row * r.height);

    /* The host loop's turn (kui.h): build, drain, draw. */
    build(ui, 0);
    drain(ui);
    size_t drawn = render(&g, &r, ui, rgba);
    check(drawn > 0, "the first frame drew instances");
    check(r.uploaded[2] == r.atlas_size, "the first frame uploaded the whole atlas page");
    check_frame(ui, &r, rgba, "first frame");

    /* Input, then the next turn: a click on the button, its message, and
     * the frame rebuilt with the new count. The label's "1" is a glyph the
     * atlas did not hold, so the upload is a rect, not the page. */
    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ui, &dd);
    const KuiQuad *b = button_quad(&dd);
    int clicks = 0;
    if (b) {
        float cx = (b->x + b->w / 2) / SCALE, cy = (b->y + b->h / 2) / SCALE;
        kui_input_cursor(ui, cx, cy);
        check(kui_pointer_wants(ui), "the pointer over the button is the UI's");
        kui_input_mouse(ui, true, 1);
        kui_input_mouse(ui, false, 1);
        clicks += drain(ui);
    }
    check(clicks == 1, "the click arrived as the button's message");
    kui_input_cursor(ui, W - 4, H - 4);
    check(!kui_pointer_wants(ui), "past the panel the pointer is the world's");
    build(ui, clicks);
    drain(ui);
    drawn = render(&g, &r, ui, rgba);
    check(drawn > 0, "the second frame drew instances");
    check(r.uploaded[2] > 0 && r.uploaded[2] < r.atlas_size, "the second frame uploaded a dirty rect alone");
    check_frame(ui, &r, rgba, "second frame");
    check(wgpu_errors == 0, "wgpu raised no validation error");

    uint32_t atlas = r.atlas_size, rect_w = r.uploaded[2], rect_h = r.uploaded[3], width = r.width, height = r.height;
    free(rgba);
    renderer_free(&r);
    kui_ctx_free(ui);
    if (fails) {
        fprintf(stderr, "wgpu_host: %d check(s) failed\n", fails);
        gpu_close(&g);
        return 1;
    }
    printf("wgpu host OK: %s, %ux%u, %zu instances of %u bytes, the atlas a %u px page then a %ux%u rect\n",
           g.adapter_name, width, height, drawn, kui_instance_stride(), atlas, rect_w, rect_h);
    gpu_close(&g);
    return 0;
}
