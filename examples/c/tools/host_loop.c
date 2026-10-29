/* The host loop, without a window (backlog F238): what a host that brings
 * its own window runs every turn - kui.h's "The host loop": build, drain,
 * draw, then input and drain again - with nothing from the runner. It
 * links against either library the release ships: the one with the runner
 * (the kui-ffi archive's lib/<platform>/shared, static) and the one
 * without (the kui-ffi-core archive's lib/<platform>/shared), which
 * pack-ffi.nu's verify links it against.
 *
 *     cargo run -p kui-devtools --bin cbuild -- --run   builds and runs it
 */
#include <stdlib.h>
#include "../common.h"

/* A panel holding two buttons, on a transparent root, opened and closed by
 * kui.h's scoped openers (backlog F261). The click message is the node's
 * to keep, so each frame makes one: "go" a string (KUI_BOX_CLICK), "row" a
 * map that says which row (KUI_BOX_CLICK_MSG and kui_msg_int, backlog
 * F266). */
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
            KUI_BOX_CLICK_MSG(ui, "row", kui_msg_int("row", "row", 7), .width = {KUI_FIXED, 60},
                              .height = {KUI_FIXED, 24}, .bg = 0x3a6ff0ffu, .radius = 4,
                              .label = KUI_STR("row")) {
                kui_text(ui, KUI_STR("row"), &label);
            }
        }
    }
    return panel;
}

/* Drains the queue; how many clicks said "go", and the row the last
 * "row" said. kui_event_message reads both shapes: "go" arrives as
 * {tag:"go"}, the row as {kind:"row", row:7} (backlog F264). */
static int drain(KuiCtx *ui, int64_t *row) {
    int clicks = 0;
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ui, &ev)) {
        KuiStr msg = {0};
        if (!ev_is(&ev, "click") || !kui_event_message(&ev, &msg)) continue;
        if (kui_str_eq(msg, "go")) clicks++;
        if (kui_str_eq(msg, "row") && row) kui_value_as_int(kui_value_get(ev.payload, KUI_STR("row")), row);
    }
    return clicks;
}

/* kui_event_message and kui_event_data on payloads made by hand: the three
 * shapes a message comes back in, and an event with no payload. */
static void messages(void) {
    KuiStr name = {0};
    KuiEvent ev = KUI_EVENT_INIT;
    check(!kui_event_message(&ev, &name) && !kui_event_data(&ev), "no payload names nothing");
    ev.payload = kui_value_str(KUI_STR("bare"));
    check(kui_event_message(&ev, &name) && kui_str_eq(name, "bare"), "a bare string is its own name");
    kui_value_free((KuiValue *)ev.payload);
    KuiValue *wrapped = kui_value_map();
    kui_value_map_set(wrapped, KUI_STR("tag"), kui_value_str(KUI_STR("go")));
    ev.payload = wrapped;
    check(kui_event_message(&ev, &name) && kui_str_eq(name, "go"), "a wrapped message is named by its tag");
    kui_value_free(wrapped);
    KuiValue *pan = kui_msg_int("pan", "lane", 2);
    KuiValue *drag = kui_value_map();
    kui_value_map_set(drag, KUI_STR("dx"), kui_value_float(1.5));
    kui_value_map_set(pan, KUI_STR("drag"), drag);
    ev.payload = pan;
    ev.event = KUI_STR("drag");
    double dx = 0;
    int64_t lane = 0;
    check(kui_event_message(&ev, &name) && kui_str_eq(name, "pan"), "a map is named by its kind");
    check(kui_value_as_float(kui_value_get(kui_event_data(&ev), KUI_STR("dx")), &dx) && dx == 1.5,
          "kui_event_data is the field named for the event");
    check(kui_value_as_int(kui_value_get(pan, KUI_STR("lane")), &lane) && lane == 2, "kui_msg_int's field");
    kui_value_free(pan);
    KuiValue *nameless = kui_value_map();
    ev.payload = nameless;
    name = KUI_STR("kept");
    check(!kui_event_message(&ev, &name) && kui_str_eq(name, "kept"), "a map without kind names nothing");
    kui_value_free(nameless);
}

int main(void) {
    if (!abi_ok()) return 1;
    KuiCtx *ui = kui_ctx_new();

    messages();

    /* Build, drain, draw. */
    uint64_t panel = build(ui);
    drain(ui, NULL);
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
    check(drain(ui, NULL) == 1, "the click arrived as its message");
    int64_t row = -1;
    kui_input_cursor(ui, 38, 44);
    kui_input_mouse(ui, true, 1);
    kui_input_mouse(ui, false, 1);
    check(drain(ui, &row) == 0 && row == 7, "the second button's click said which row");
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
