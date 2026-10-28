/* The host loop, without a window (backlog F238): what a host that brings
 * its own window runs every turn - kui.h's "The host loop": build, drain,
 * draw, then input and drain again - with nothing from the runner. It
 * links against either library the release ships: the one with the runner
 * (lib/<platform>/shared, static) and the one without
 * (lib/<platform>/core/...), which pack-ffi.nu's verify links it against.
 *
 *     cargo run -p kui-devtools --bin cbuild -- --run   builds and runs it
 */
#include <stdlib.h>
#include "../common.h"

/* A panel holding a button, on a transparent root, opened and closed by
 * kui.h's scoped openers (backlog F261). The click message is the node's
 * to keep, so KUI_BOX_CLICK makes one a frame. */
static uint64_t build(KuiCtx *ui) {
    uint64_t panel = 0;
    KuiTextStyle label = {.size = 13, .color = 0xffffffffu};
    KUI_FRAME(ui, 320, 200, 2) {
        KUI_BOX_KEY(ui, &panel, "panel", .width = {KUI_FIXED, 160}, .height = {KUI_FIXED, 80},
                    .pad_l = 8, .pad_t = 8, .bg = 0x202020ccu, .radius = 6) {
            KUI_BOX_CLICK(ui, "go", "go", .width = {KUI_FIXED, 60}, .height = {KUI_FIXED, 24},
                          .bg = 0x3a6ff0ffu, .radius = 4, .label = KUI_STR("go")) {
                kui_text(ui, KUI_STR("go"), &label);
            }
        }
    }
    return panel;
}

/* Drains the queue; how many clicks said "go". */
static int drain(KuiCtx *ui) {
    int clicks = 0;
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ui, &ev)) {
        KuiStr msg = {0};
        if (ev_is(&ev, "click") && ev_message(&ev, &msg) && kui_str_eq(msg, "go")) clicks++;
    }
    return clicks;
}

int main(void) {
    if (!abi_ok()) return 1;
    KuiCtx *ui = kui_ctx_new();

    /* Build, drain, draw. */
    uint64_t panel = build(ui);
    drain(ui);
    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    check(kui_draw_data(ui, &dd) && dd.quad_count > 0, "the frame drew");
    check(dd.atlas_dirty && dd.atlas_dirty_rect[2] > 0, "and its glyphs are an atlas rect to upload");
    check(dd.bounds[0] == 0 && dd.bounds[2] == 320 && dd.bounds[3] == 160,
          "the bounds are the panel's, in physical px");

    /* Draw it with kui's own shader and packing (backlog F234): the
     * layout read at run time, the frame packed as instances. */
    uint32_t attrs[13 * 3];
    uint32_t stride = kui_instance_stride();
    uint32_t n_attrs = kui_instance_attributes(attrs, 13);
    check(n_attrs == 13 && attrs[(n_attrs - 1) * 3 + 2] + attrs[(n_attrs - 1) * 3 + 1] * 4 == stride,
          "kui_instance_attributes: the last attribute ends at the stride");
    size_t count = kui_pack_instances(ui, NULL, 0);
    uint8_t *instances = malloc(count * stride);
    check(count == dd.quad_count && kui_pack_instances(ui, instances, count * stride) == count,
          "kui_pack_instances: an instance a quad");
    float pos[2];
    memcpy(pos, instances + attrs[2], sizeof pos);
    check(pos[0] == dd.quads[0].x && pos[1] == dd.quads[0].y, "the first instance is where the first quad is");
    free(instances);
    KuiStr wgsl = kui_renderer_wgsl(KUI_WGSL_SINGLE);
    check(has(wgsl, "fn vs_main") && !has(wgsl, "dual_source_blending"), "kui_renderer_wgsl: the single variant");
    check(has(kui_renderer_wgsl(KUI_WGSL_DUAL), "dual_source_blending"), "and the dual one");

    /* Input: the pointer over the button is the UI's, then a click. */
    kui_input_cursor(ui, 38, 20);
    check(kui_pointer_wants(ui) && kui_hit(ui, 38, 20) != panel, "the button is under the pointer");
    kui_input_mouse(ui, true, 1);
    kui_input_mouse(ui, false, 1);
    check(drain(ui) == 1, "the click arrived as its message");
    kui_input_cursor(ui, 300, 180);
    check(!kui_pointer_wants(ui), "past the panel is the world's");

    /* The next turn draws what the last drew. */
    build(ui);
    check(kui_draw_data(ui, &dd) && dd.quad_count > 0 && !dd.atlas_dirty, "nothing new to upload");

    kui_ctx_free(ui);
    if (fails) {
        fprintf(stderr, "host_loop: %d check(s) failed\n", fails);
        return 1;
    }
    printf("host loop OK: %zu quads, bounds %ux%u\n", dd.quad_count, dd.bounds[2], dd.bounds[3]);
    return 0;
}
