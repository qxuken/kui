/* A host that draws kui's frame with a GPU device of its own (backlog
 * F234), on wgpu-native's C API (webgpu.h), through kui_wgpu.h (backlog
 * F263): kui.h's "Drawing it yourself" done end to end, offscreen. No
 * window and no surface - an instance, an adapter, a device - so it runs
 * on a machine with a display and on CI's software Vulkan alike.
 *
 * The pipeline, the atlas and the instances are kui_wgpu.h's; this file
 * is what stays the host's: the device, a target and the pass over it,
 * cleared to the "world" kui draws over, and a readback. Since CI runs it
 * over lavapipe, its checks are the header's.
 *
 * The frame is drawn, read back and checked: the panel's colour inside the
 * panel, the clear colour far outside it, a shadow under it, glyph coverage
 * where its title is, the button's colour on the button. Then the button
 * is clicked, the frame rebuilt with its new label, and drawn again: the
 * label's new glyph reaches the atlas as a dirty rect, uploaded alone.
 * That runs once with KUI_WGSL_SINGLE, and again with KUI_WGSL_DUAL and
 * subpixel text where the adapter offers dual-source blending.
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
#include "kui_wgpu.h"

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
/* A texture-backed image between the title and the button: a quad
 * kui_wgpu.h leaves out, so where it is shows the panel. */
#define IMAGE_W 32
#define IMAGE_H 6
/* The world behind the UI: what the target is cleared to. */
static const WGPUColor CLEAR = {0.10, 0.55, 0.35, 1.0};

/* --- the view --------------------------------------------------------- */

/* A panel with a shadow, a title, an image and a button, on a transparent
 * root. */
static void build(KuiCtx *ui, uint64_t image, int clicks) {
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
    KuiSpec picture = {.width = {KUI_FIXED, IMAGE_W}, .height = {KUI_FIXED, IMAGE_H}};
    kui_image(ui, image, &picture);
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
    /* Whether the device was given dual-source blending, which
     * KUI_WGSL_DUAL wants. */
    bool dual;
} Gpu;

/* An instance, an adapter and a device, with no surface: the device with
 * dual-source blending when the adapter offers it. False with no adapter:
 * the machine has no GPU (or no driver for one). */
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
    WGPUFeatureName dual = WGPUFeatureName_DualSourceBlending;
    WGPUDeviceDescriptor desc = WGPU_DEVICE_DESCRIPTOR_INIT;
    if (wgpuAdapterHasFeature(g->adapter, dual)) {
        desc.requiredFeatureCount = 1;
        desc.requiredFeatures = &dual;
    }
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
    g->dual = wgpuDeviceHasFeature(g->device, dual);
    return true;
}

static void gpu_close(Gpu *g) {
    if (g->queue) wgpuQueueRelease(g->queue);
    if (g->device) wgpuDeviceRelease(g->device);
    if (g->adapter) wgpuAdapterRelease(g->adapter);
    if (g->instance) wgpuInstanceRelease(g->instance);
}

/* --- the target: the host's pass, kui drawn into it ------------------- */

static const WGPUTextureFormat FORMAT = WGPUTextureFormat_RGBA8Unorm;

/* An offscreen target and the buffer it is read back through; a copy's
 * rows are 256-byte aligned. */
typedef struct Target {
    WGPUTexture texture;
    WGPUTextureView view;
    WGPUBuffer readback;
    uint32_t width, height, row;
} Target;

static void target_make(Gpu *g, Target *t, uint32_t width, uint32_t height) {
    t->width = width;
    t->height = height;
    t->row = (width * 4 + 255) & ~255u;
    WGPUTextureDescriptor td = WGPU_TEXTURE_DESCRIPTOR_INIT;
    td.usage = WGPUTextureUsage_RenderAttachment | WGPUTextureUsage_CopySrc;
    td.size = (WGPUExtent3D){width, height, 1};
    td.format = FORMAT;
    t->texture = wgpuDeviceCreateTexture(g->device, &td);
    t->view = wgpuTextureCreateView(t->texture, NULL);
    WGPUBufferDescriptor rd = WGPU_BUFFER_DESCRIPTOR_INIT;
    rd.usage = WGPUBufferUsage_CopyDst | WGPUBufferUsage_MapRead;
    rd.size = (uint64_t)t->row * height;
    t->readback = wgpuDeviceCreateBuffer(g->device, &rd);
}

static void target_free(Target *t) {
    wgpuBufferRelease(t->readback);
    wgpuTextureViewRelease(t->view);
    wgpuTextureRelease(t->texture);
}

/* kui's last finished frame over the world: a pass that clears the target
 * to it - the host's load op - with kui_wgpu_draw inside, then the target
 * read back into `rgba` (t->row bytes a row). Returns the instances
 * drawn. */
static size_t render(Gpu *g, KuiWgpu *kr, Target *t, KuiCtx *ui, uint8_t *rgba) {
    WGPUCommandEncoder enc = wgpuDeviceCreateCommandEncoder(g->device, NULL);
    WGPURenderPassColorAttachment color = WGPU_RENDER_PASS_COLOR_ATTACHMENT_INIT;
    color.view = t->view;
    color.loadOp = WGPULoadOp_Clear;
    color.storeOp = WGPUStoreOp_Store;
    color.clearValue = CLEAR;
    WGPURenderPassDescriptor rp = WGPU_RENDER_PASS_DESCRIPTOR_INIT;
    rp.colorAttachmentCount = 1;
    rp.colorAttachments = &color;
    WGPURenderPassEncoder pass = wgpuCommandEncoderBeginRenderPass(enc, &rp);
    size_t drawn = kui_wgpu_draw(kr, ui, pass);
    wgpuRenderPassEncoderEnd(pass);
    wgpuRenderPassEncoderRelease(pass);

    WGPUTexelCopyTextureInfo src = WGPU_TEXEL_COPY_TEXTURE_INFO_INIT;
    src.texture = t->texture;
    WGPUTexelCopyBufferInfo dst = WGPU_TEXEL_COPY_BUFFER_INFO_INIT;
    dst.buffer = t->readback;
    dst.layout.bytesPerRow = t->row;
    dst.layout.rowsPerImage = t->height;
    WGPUExtent3D extent = {t->width, t->height, 1};
    wgpuCommandEncoderCopyTextureToBuffer(enc, &src, &dst, &extent);
    WGPUCommandBuffer cmd = wgpuCommandEncoderFinish(enc, NULL);
    wgpuQueueSubmit(g->queue, 1, &cmd);
    wgpuCommandBufferRelease(cmd);
    wgpuCommandEncoderRelease(enc);

    size_t size = (size_t)t->row * t->height;
    Answer m = {0};
    WGPUBufferMapCallbackInfo mcb = WGPU_BUFFER_MAP_CALLBACK_INFO_INIT;
    mcb.mode = WGPUCallbackMode_AllowProcessEvents;
    mcb.callback = on_mapped;
    mcb.userdata1 = &m;
    wgpuBufferMapAsync(t->readback, WGPUMapMode_Read, 0, size, mcb);
    wgpuDevicePoll(g->device, true, NULL);
    wait_for(g->instance, &m);
    if (!m.ok) {
        check(false, "the readback mapped");
        return 0;
    }
    memcpy(rgba, wgpuBufferGetConstMappedRange(t->readback, 0, size), size);
    wgpuBufferUnmap(t->readback);
    return drawn;
}

/* --- the checks ------------------------------------------------------- */

static const uint8_t *pixel(const Target *t, const uint8_t *rgba, float x, float y) {
    return rgba + (size_t)y * t->row + (size_t)x * 4;
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

/* The quad the image is: drawn from a texture of its own. */
static const KuiQuad *texture_quad(const KuiDrawData *dd) {
    for (size_t i = 0; i < dd->quad_count; i++) {
        if (dd->quads[i].kind == KUI_QUAD_TEXTURE) return &dd->quads[i];
    }
    return NULL;
}

/* What every drawn frame shows, wherever its pixels came from. `glyph` is
 * the kind the title's glyphs should be: a mask, or per-channel coverage
 * under subpixel text. */
static void check_frame(KuiCtx *ui, const Target *t, const uint8_t *rgba, uint32_t glyph, const char *when) {
    char what[160];
    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ui, &dd);
#define CHECK(ok, msg) (snprintf(what, sizeof what, "%s: %s", when, msg), check(ok, what))

    /* The panel's colour at its right edge, clear of the text. */
    const uint8_t *p = pixel(t, rgba, (PANEL_X + PANEL_W - 6) * SCALE, (PANEL_Y + PANEL_H / 2) * SCALE);
    CHECK(near(p, PANEL_BG >> 8, 2), "the panel is its colour");
    /* The world far from it. */
    p = pixel(t, rgba, t->width - 10, t->height - 10);
    CHECK(near(p, clear_rgb(), 2), "past the panel is the clear colour");
    /* The shadow, just under the panel's bottom edge: darker than the
     * world it falls on. */
    p = pixel(t, rgba, (PANEL_X + PANEL_W / 2) * SCALE, (PANEL_Y + PANEL_H + 3) * SCALE);
    uint32_t c = clear_rgb();
    int world = (int)((c >> 16) + ((c >> 8) & 0xff) + (c & 0xff));
    CHECK(p[0] + p[1] + p[2] < world * 9 / 10, "the shadow darkens the world under the panel");

    /* The title: glyph quads in the panel's upper half, and pixels inside
     * their union that are not the flat panel. */
    float top = (PANEL_Y + PANEL_H / 2) * SCALE;
    float x0 = 1e9f, y0 = 1e9f, x1 = -1e9f, y1 = -1e9f;
    bool kind_ok = true;
    for (size_t i = 0; i < dd.quad_count; i++) {
        const KuiQuad *q = &dd.quads[i];
        if ((q->kind != KUI_QUAD_GLYPH_MASK && q->kind != KUI_QUAD_GLYPH_SUBPIXEL) || q->y + q->h > top) continue;
        if (q->kind != glyph) kind_ok = false;
        if (q->x < x0) x0 = q->x;
        if (q->y < y0) y0 = q->y;
        if (q->x + q->w > x1) x1 = q->x + q->w;
        if (q->y + q->h > y1) y1 = q->y + q->h;
    }
    CHECK(kind_ok, glyph == KUI_QUAD_GLYPH_SUBPIXEL ? "the title's glyphs are subpixel" : "the title's glyphs are masks");
    int inked = 0;
    for (float y = y0; y < y1; y++)
        for (float x = x0; x < x1; x++)
            if (!near(pixel(t, rgba, x, y), PANEL_BG >> 8, 40)) inked++;
    CHECK(x1 > x0 && inked >= 30, "the title's glyphs are drawn (atlas uploaded, sampled)");

    /* The button's colour at its left edge, clear of its label. */
    const KuiQuad *b = button_quad(&dd);
    CHECK(b != NULL, "the button's quad is in the frame");
    if (b) CHECK(near(pixel(t, rgba, b->x + 6, b->y + b->h / 2), BUTTON_BG >> 8, 2), "the button is its colour");

    /* The texture-backed image, between the title's quads and the
     * button's: left out, so the panel shows where it is, and the draws
     * either side of it still land. */
    const KuiQuad *img = texture_quad(&dd);
    CHECK(img != NULL, "the image is a KUI_QUAD_TEXTURE quad");
    if (img) CHECK(near(pixel(t, rgba, img->x + img->w / 2, img->y + img->h / 2), PANEL_BG >> 8, 2),
                   "the left-out image draws nothing over the panel");
#undef CHECK
}

/* The whole turn with one variant of kui's shader, on a context of its
 * own: two frames drawn and checked, a click between them. */
static void run(Gpu *g, uint32_t variant) {
    const char *name = variant == KUI_WGSL_DUAL ? "dual" : "single";
    char what[160];
#define CHECK(ok, msg) (snprintf(what, sizeof what, "%s: %s", name, msg), check(ok, what))
    KuiCtx *ui = kui_ctx_new();
    /* Per-channel glyphs only where the blend is per channel. */
    kui_set_subpixel_text(ui, variant == KUI_WGSL_DUAL);
    uint32_t glyph = variant == KUI_WGSL_DUAL ? KUI_QUAD_GLYPH_SUBPIXEL : KUI_QUAD_GLYPH_MASK;
    /* Magenta, and updated once, which moves it off the atlas onto a
     * texture of its own. */
    uint8_t magenta[IMAGE_W * IMAGE_H * 4];
    for (size_t i = 0; i < sizeof magenta; i += 4) {
        magenta[i] = 0xff, magenta[i + 1] = 0, magenta[i + 2] = 0xff, magenta[i + 3] = 0xff;
    }
    uint64_t image = kui_image_add(ui, IMAGE_W, IMAGE_H, magenta);
    kui_image_update(ui, image, IMAGE_W, IMAGE_H, magenta);
    KuiWgpu kr;
    CHECK(kui_wgpu_init(&kr, g->device, g->queue, FORMAT, variant) && !wgpu_errors,
          "kui_wgpu_init built the pipeline from kui's shader and layout");
    Target t;
    target_make(g, &t, W * SCALE, H * SCALE);
    uint8_t *rgba = malloc((size_t)t.row * t.height);

    /* The host loop's turn (kui.h): build, drain, draw. */
    build(ui, image, 0);
    drain(ui);
    size_t drawn = render(g, &kr, &t, ui, rgba);
    CHECK(drawn > 0, "the first frame drew instances");
    CHECK(drawn == kui_pack_instances(ui, NULL, 0) - 1, "the first frame drew every instance but the image's");
    CHECK(kr.uploaded[2] == kr.atlas_size, "the first frame uploaded the whole atlas page");
    snprintf(what, sizeof what, "%s, first frame", name);
    check_frame(ui, &t, rgba, glyph, what);

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
        CHECK(kui_pointer_wants(ui), "the pointer over the button is the UI's");
        kui_input_mouse(ui, true, 1);
        kui_input_mouse(ui, false, 1);
        clicks += drain(ui);
    }
    CHECK(clicks == 1, "the click arrived as the button's message");
    kui_input_cursor(ui, W - 4, H - 4);
    CHECK(!kui_pointer_wants(ui), "past the panel the pointer is the world's");
    build(ui, image, clicks);
    drain(ui);
    drawn = render(g, &kr, &t, ui, rgba);
    CHECK(drawn > 0, "the second frame drew instances");
    CHECK(kr.uploaded[2] > 0 && kr.uploaded[2] < kr.atlas_size, "the second frame uploaded a dirty rect alone");
    snprintf(what, sizeof what, "%s, second frame", name);
    check_frame(ui, &t, rgba, glyph, what);
#undef CHECK

    printf("wgpu host %s: %s, %ux%u, %zu instances of %u bytes, the atlas a %u px page then a %ux%u rect\n",
           name, g->adapter_name, t.width, t.height, drawn, kui_instance_stride(), kr.atlas_size, kr.uploaded[2],
           kr.uploaded[3]);
    free(rgba);
    target_free(&t);
    kui_wgpu_free(&kr);
    kui_ctx_free(ui);
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

    run(&g, KUI_WGSL_SINGLE);
    if (g.dual) {
        run(&g, KUI_WGSL_DUAL);
    } else {
        /* Without the feature the dual variant is refused, not built. */
        KuiWgpu kr;
        check(!kui_wgpu_init(&kr, g.device, g.queue, FORMAT, KUI_WGSL_DUAL) && !kr.pipeline,
              "kui_wgpu_init refuses KUI_WGSL_DUAL on a device without dual-source blending");
        kui_wgpu_free(&kr);
        printf("wgpu host dual: skipped, %s offers no dual-source blending\n", g.adapter_name);
    }
    check(wgpu_errors == 0, "wgpu raised no validation error");
    gpu_close(&g);
    if (fails) {
        fprintf(stderr, "wgpu_host: %d check(s) failed\n", fails);
        return 1;
    }
    printf("wgpu host OK\n");
    return 0;
}
