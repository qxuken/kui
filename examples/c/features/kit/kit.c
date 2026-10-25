/* Kit from C: the components addons/kit provides, placed through the header
 * its schema generates (addons/kit/bindings/c/kit.h, docs/adr/0066).
 *
 * The props are plain structs, an optional number or flag carries a `_set`
 * beside it, an enum's zero is "unset", and a message (`on_press`) is a
 * KuiValue the call takes. A component with children of yours takes a
 * KuiChildrenFn, called with the group it wants built.
 *
 * Run:
 *   cargo run -p kui-devtools --bin cbuild -- --run   # builds and runs it
 *   ./target/debug/kit --headless                     # a frame, a press, asserts
 *   ./target/debug/kit                                # a window
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "kit.h"
#include "kui.h"

#define KIT_LUA "addons/kit/lua/kit.lua"

typedef struct {
    long long runs;
    long long seg_b;
    const char *segment;
} App;

static KuiValue *msg(const char *kind) {
    KuiValue *m = kui_value_map();
    kui_value_map_set(m, KUI_STR("kind"), kui_value_str(KUI_STR(kind)));
    return m;
}

/* The toolbar's children: our Run button, a Kit component inside a Kit
 * component, heard as our own message. */
static void toolbar_children(void *user, KuiCtx *ui, KuiStr group) {
    (void)user;
    if (group.len == 0) {
        KitButton run = {.label = KUI_STR("Run"), .variant = KIT_VARIANT_PRIMARY,
                         .icon = KUI_STR("play"), .on_press = msg("run")};
        kit_button(ui, &run);
    } else if (kui_str_eq(group, "trailing")) {
        KitBadge live = {.label = KUI_STR("live"), .tone = KIT_TONE_OK};
        kit_badge(ui, &live);
    }
}

static void view(void *user, KuiCtx *ui) {
    App *app = (App *)user;
    KuiSpec root = {
        .dir = KUI_COLUMN,
        .width = {KUI_GROW, 1},
        .height = {KUI_GROW, 1},
        .pad_l = 12, .pad_r = 12, .pad_t = 12, .pad_b = 12,
        .gap = 10,
    };
    kui_root(ui, &root);

    KitSettings settings = {.skin = KIT_SKIN_LEDGER, .density = KIT_DENSITY_COMFORTABLE};
    kit_settings(ui, &settings);

    KitToolbar bar = {0};
    kit_toolbar_with(ui, &bar, toolbar_children, NULL);

    KitChoice choices[] = {
        {.value = KUI_STR("a"), .label = KUI_STR("Alpha")},
        {.value = KUI_STR("b"), .label = KUI_STR("Beta")},
    };
    KitSegmented seg = {
        .options = choices,
        .options_count = 2,
        .value = {(const uint8_t *)app->segment, strlen(app->segment)},
        .on_change = msg("segment"),
    };
    kit_segmented(ui, &seg);

    KitSlider zoom = {.value = 0.4, .min = 0, .max = 1, .label = KUI_STR("Zoom")};
    kit_slider(ui, &zoom);

    KitStat stat = {.label = KUI_STR("Runs"), .value = KUI_STR("p95 412 ms"),
                    .delta = KUI_STR("-8%"), .tone = KIT_TONE_OK};
    kit_stat(ui, &stat);
}

static void on_event(void *user, const KuiEvent *ev) {
    App *app = (App *)user;
    if (!ev->payload || !kui_str_eq(ev->event, "component")) return;
    const KuiValue *kind = kui_value_get(ev->payload, KUI_STR("kind"));
    KuiStr s;
    if (!kind || !kui_value_as_str(kind, &s)) return;
    if (kui_str_eq(s, "run")) {
        app->runs++;
    } else if (kui_str_eq(s, "segment")) {
        /* The event's data rides under its name: {change: {value}}. */
        const KuiValue *change = kui_value_get(ev->payload, KUI_STR("change"));
        const KuiValue *value = change ? kui_value_get(change, KUI_STR("value")) : NULL;
        KuiStr v;
        if (value && kui_value_as_str(value, &v) && kui_str_eq(v, "b")) {
            app->seg_b++;
            app->segment = "b";
        }
    }
}

static bool find_named(KuiCtx *ctx, const char *name, float *x, float *y) {
    size_t n = kui_access_tree(ctx, NULL, 0);
    if (n == 0) return false;
    KuiAccessNode *nodes = calloc(n, sizeof *nodes);
    if (!nodes) return false;
    n = kui_access_tree(ctx, nodes, n);
    bool found = false;
    for (size_t i = 0; i < n && !found; i++) {
        const KuiAccessNode *a = &nodes[i];
        if (!kui_str_eq(a->name, name)) continue;
        *x = a->x + a->w * 0.5f;
        *y = a->y + a->h * 0.5f;
        found = true;
    }
    free(nodes);
    return found;
}

static void frame(KuiCtx *ctx, App *app) {
    kui_frame_begin(ctx, 900, 500, 1.0f);
    view(app, ctx);
    kui_frame_finish(ctx);
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ctx, &ev)) {
        on_event(app, &ev);
        ev = KUI_EVENT_INIT;
    }
}

static bool press(KuiCtx *ctx, App *app, const char *name) {
    float x = 0, y = 0;
    if (!find_named(ctx, name, &x, &y)) {
        fprintf(stderr, "FAIL: no node named %s\n", name);
        return false;
    }
    kui_input_cursor(ctx, x, y);
    kui_input_mouse(ctx, true, 1);
    kui_input_mouse(ctx, false, 1);
    frame(ctx, app);
    return true;
}

static KuiCtx *load(void) {
    KuiCtx *ctx = kui_ctx_new();
    if (!ctx) return NULL;
    if (!kui_ctx_add_extension(ctx, KUI_STR(KIT_NAMESPACE), KUI_STR(KIT_LUA))) {
        KuiStr err = {0};
        if (kui_ctx_extension_error(ctx, &err))
            fprintf(stderr, "kui: %.*s\n", (int)err.len, (const char *)err.ptr);
        fprintf(stderr, "run from the repository root, where %s is\n", KIT_LUA);
        kui_ctx_free(ctx);
        return NULL;
    }
    return ctx;
}

static int headless(void) {
    App app = {.segment = "a"};
    KuiCtx *ctx = load();
    if (!ctx) return 1;
    frame(ctx, &app);
    frame(ctx, &app);
    int rc = 0;
    KuiWarning w;
    if (kui_take_warnings(ctx, &w, 1) > 0) {
        fprintf(stderr, "FAIL: warning %.*s: %.*s\n", (int)w.code.len, (const char *)w.code.ptr,
                (int)w.message.len, (const char *)w.message.ptr);
        rc = 1;
    }
    /* Ledger labels in capitals: the name is the label as written. */
    if (rc == 0 && !press(ctx, &app, "Run")) rc = 1;
    if (rc == 0 && !press(ctx, &app, "Beta")) rc = 1;
    if (rc == 0 && app.runs != 1) {
        fprintf(stderr, "FAIL: Run reached the host %lld times\n", app.runs);
        rc = 1;
    }
    if (rc == 0 && app.seg_b != 1) {
        fprintf(stderr, "FAIL: the segment's change did not carry b (%lld)\n", app.seg_b);
        rc = 1;
    }
    if (rc == 0)
        printf("ok: Kit's toolbar, button and segmented control answered through kit.h\n");
    kui_ctx_free(ctx);
    return rc;
}

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--headless") == 0) return headless();
    App app = {.segment = "a"};
    KuiCtx *ctx = load();
    if (!ctx) return 1;
    KuiRunConfig window = KUI_RUN_CONFIG_INIT;
    window.width = 900;
    window.height = 500;
    bool ok = kui_run_with(ctx, KUI_STR("Kit from C"), &window, view, on_event, &app);
    kui_ctx_free(ctx);
    return ok ? 0 : 1;
}
