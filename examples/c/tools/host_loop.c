/* The host loop, without a window (backlog F238): what a host that brings
 * its own window runs every turn - kui.h's "The host loop": build, drain,
 * draw, then input and drain again - with nothing from the runner. It
 * links against either library the release ships: the one with the runner
 * (lib/<platform>/shared, static) and the one without
 * (lib/<platform>/core/...), which pack-ffi.nu's verify links it against.
 *
 *     cargo run -p kui-devtools --bin cbuild -- --run   builds and runs it
 */
#include "../common.h"

/* A panel holding a button, on a transparent root. */
static uint64_t build(KuiCtx *ui) {
    kui_frame_begin(ui, 320, 200, 2);
    KuiSpec panel = {.width = {KUI_FIXED, 160}, .height = {KUI_FIXED, 80},
                     .pad_l = 8, .pad_t = 8, .bg = 0x202020ccu, .radius = 6};
    uint64_t panel_key = kui_open_keyed(ui, KUI_STR("panel"), &panel, NULL);
    KuiSpec button = {.width = {KUI_FIXED, 60}, .height = {KUI_FIXED, 24},
                      .bg = 0x3a6ff0ffu, .radius = 4, .label = KUI_STR("go")};
    /* The click message is the node's to keep, one a frame. */
    kui_open_keyed(ui, KUI_STR("go"), &button, kui_value_str(KUI_STR("go")));
    KuiTextStyle label = {.size = 13, .color = 0xffffffffu};
    kui_text(ui, KUI_STR("go"), &label);
    kui_close(ui);
    kui_close(ui);
    kui_frame_finish(ui);
    return panel_key;
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
