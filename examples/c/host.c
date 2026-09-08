/* C as the host of a C extension: the third corner of examples/c.
 *
 * counter.c is C with kui as a library. panel.c is C as a *guest* inside a
 * Rust host (panel.rs). This is the pair those two left open - C on both
 * sides - and until ABI 10 the C API could not express it: loading a plugin
 * was `kui_ffi::CExtension`, which is Rust, so a C host could declare a slot
 * with kui_slot and had nothing to put in it.
 *
 * It loads examples/c/panel.c under a namespace of its own choosing,
 * declares that plugin's slot in the middle of its own view, and counts what
 * comes back. The panel is the *same binary* panel.rs loads: an extension
 * does not know or care what language its host is written in, which is the
 * point of ADR 0014's contract being C in the first place.
 *
 * Run:
 *   ./examples/c/build.sh --run           # or pwsh examples/c/build.ps1 -Run
 *   ./target/debug/host --headless        # no window: a frame, a click, asserts
 *   ./target/debug/host                   # a window
 *   ./target/debug/host --headless path/to/plugin
 *
 * --headless is the contract without a display, and it is what CI runs: it
 * builds a frame the way the runner does, finds a row the plugin drew,
 * clicks it, and checks that the click reached the plugin and not the host
 * and that the plugin's reply reached the host with the plugin's origin on
 * it. Which is panel.rs's --headless, from the other side.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "kui.h"

/* The namespace we load the panel under: ours to choose, and what fronts
 * the slot name. The plugin calls its own slot "panel". */
#define NS "todos"
#define PANEL_SLOT NS "/panel"

/* Which panel to load when none is named: the one the build script leaves
 * beside this executable, in target/<profile>/. The profile is a define
 * rather than a guess, so a --release host looks in target/release; the
 * two-step stringify is how a macro's *text* becomes a string literal.
 *
 * On the unixes there is one plugin and this is it: it leaves every kui_*
 * undefined and takes them from whatever executable loaded it, so panel.so
 * loads into this host and into panel.rs's equally.
 *
 * On Windows there are two, and the difference is the whole lesson of
 * build.ps1. A DLL must name the module each import comes from. panel.dll
 * - this one - names `kui_ffi.dll` and so loads into any host that ships
 * it, which is the shape a plugin you hand to somebody wants.
 * panel-host.dll names `c_panel.exe` and loads into that host and nothing
 * else; try `host.exe --headless target/debug/panel-host.dll` to watch it
 * be refused here. That is not a bug in either file. */
#ifndef KUI_PROFILE_DIR
#define KUI_PROFILE_DIR debug
#endif
#define KUI_STRINGIFY_(x) #x
#define KUI_STRINGIFY(x) KUI_STRINGIFY_(x)
#define PLUGIN_DIR "target/" KUI_STRINGIFY(KUI_PROFILE_DIR) "/"

#if defined(_WIN32)
#define DEFAULT_PLUGIN PLUGIN_DIR "panel.dll"
#define BUILD_HINT "pwsh examples/c/build.ps1"
#else
#define DEFAULT_PLUGIN PLUGIN_DIR "panel.so"
#define BUILD_HINT "./examples/c/build.sh"
#endif

typedef struct {
    long long clicks;  /* our own button */
    long long toggles; /* replies from the panel */
    uint16_t reply_from;
} Host;

static KuiValue *msg(const char *kind) {
    KuiValue *m = kui_value_map();
    kui_value_map_set(m, KUI_STR("kind"), kui_value_str(KUI_STR(kind)));
    return m;
}

static void view(void *user, KuiCtx *ui) {
    Host *h = (Host *)user;

    KuiSpec root = {
        .dir = KUI_ROW,
        .width = {KUI_GROW, 1},
        .height = {KUI_GROW, 1},
        .pad_l = 16, .pad_r = 16, .pad_t = 16, .pad_b = 16,
        .gap = 16,
    };
    kui_root(ui, &root);

    /* Our half. */
    KuiSpec col = {
        .dir = KUI_COLUMN,
        .width = {KUI_GROW, 1},
        .height = {KUI_GROW, 1},
        .pad_l = 24, .pad_r = 24, .pad_t = 24, .pad_b = 24,
        .gap = 12,
        .bg = 0x161820ff,
        .radius = 10,
    };
    kui_open(ui, &col, NULL);
    {
        KuiTextStyle title = {.size = 18};
        kui_text(ui, KUI_STR("the host, in C"), &title);

        char line[80];
        snprintf(line, sizeof line, "clicks %lld, toggles heard %lld", h->clicks,
                 h->toggles);
        KuiTextStyle body = {.size = 13, .color = 0x8a8fa3ff};
        kui_text(ui, (KuiStr){(const uint8_t *)line, strlen(line)}, &body);

        kui_button(ui, KUI_STR("count"), msg("count"));
    }
    kui_close(ui);

    /* The plugin's half, in place: a position among our own children,
     * filled then and there by whoever we loaded under `todos`. The params
     * are its title and the shape of the reply we want back on a toggle -
     * declared every frame, retained by nobody, ours to free. */
    KuiValue *params = kui_value_map();
    kui_value_map_set(params, KUI_STR("title"), kui_value_str(KUI_STR("todos, from C")));
    kui_value_map_set(params, KUI_STR("on_toggle"), msg("toggled"));
    kui_slot(ui, KUI_STR(PANEL_SLOT), params);
    kui_value_free(params);
}

static void on_event(void *user, const KuiEvent *ev) {
    Host *h = (Host *)user;
    if (!ev->payload) return;
    const KuiValue *kind = kui_value_get(ev->payload, KUI_STR("kind"));
    KuiStr s;
    if (!kind || !kui_value_as_str(kind, &s)) return;

    /* Our own nodes come back with origin 0. A reply carries the origin of
     * the extension that made it, which is what tells the two apart - and
     * kui_ctx_extension_namespace turns that number back into `todos`. */
    if (kui_str_eq(s, "count") && ev->origin == 0) {
        h->clicks++;
    } else if (kui_str_eq(s, "toggled")) {
        h->toggles++;
        h->reply_from = ev->origin;
    }
}

/* -- headless ------------------------------------------------------------ */

/* The centre of the first node the plugin drew whose name starts with
 * `[ ` - one of its unfinished todo rows. By origin and name, the way
 * panel.rs's --headless does it and the way a screen reader would find it,
 * rather than by picking a quad out by its colour. The access tree is the
 * honest place to look: it is what the frame published. */
static bool find_row(KuiCtx *ctx, uint32_t origin, float *x, float *y) {
    size_t n = kui_access_tree(ctx, NULL, 0);
    if (n == 0) return false;
    KuiAccessNode *nodes = calloc(n, sizeof *nodes);
    if (!nodes) return false;
    n = kui_access_tree(ctx, nodes, n);
    bool found = false;
    for (size_t i = 0; i < n && !found; i++) {
        const KuiAccessNode *a = &nodes[i];
        if (a->origin != origin || a->name.len < 2) continue;
        if (memcmp(a->name.ptr, "[ ", 2) != 0) continue;
        *x = a->x + a->w * 0.5f;
        *y = a->y + a->h * 0.5f;
        found = true;
    }
    free(nodes);
    return found;
}

static void frame(KuiCtx *ctx, Host *host) {
    kui_frame_begin(ctx, 900, 600, 1.0f);
    view(host, ctx);
    kui_frame_finish(ctx);
}

/* A context with the panel loaded under NS, or NULL with the reason
 * printed. Both ways of running start here: --headless builds frames on the
 * context itself, and a window takes its extensions through kui_run_with -
 * one loader, and one place a refusal is explained. */
static KuiCtx *load(const char *plugin) {
    KuiCtx *ctx = kui_ctx_new();
    if (!ctx) return NULL;
    KuiStr path = {(const uint8_t *)plugin, strlen(plugin)};
    if (!kui_ctx_add_extension(ctx, KUI_STR(NS), path)) {
        KuiStr err = {0};
        if (kui_ctx_extension_error(ctx, &err))
            fprintf(stderr, "kui: %.*s\n", (int)err.len, (const char *)err.ptr);
        fprintf(stderr, "build it first: %s\n", BUILD_HINT);
        kui_ctx_free(ctx);
        return NULL;
    }
    return ctx;
}

static int headless(const char *plugin) {
    Host host = {0};
    KuiCtx *ctx = load(plugin);
    if (!ctx) return 1;
    printf("loaded %u extension(s)\n", kui_ctx_extension_count(ctx));

    frame(ctx, &host);

    float x = 0, y = 0;
    if (!find_row(ctx, 1, &x, &y)) {
        fprintf(stderr, "FAIL: the panel drew no rows - did the slot fill?\n");
        kui_ctx_free(ctx);
        return 1;
    }

    /* Click it, then build again so the press resolves into an event. */
    kui_input_cursor(ctx, x, y);
    kui_input_mouse(ctx, true, 1);
    kui_input_mouse(ctx, false, 1);
    frame(ctx, &host);

    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ctx, &ev)) {
        on_event(&host, &ev);
        ev = (KuiEvent)KUI_EVENT_INIT;
    }

    int rc = 0;
    if (host.clicks != 0) {
        fprintf(stderr, "FAIL: the plugin's click reached the host (%lld)\n",
                host.clicks);
        rc = 1;
    }
    if (host.toggles != 1) {
        fprintf(stderr, "FAIL: expected one reply from the panel, got %lld\n",
                host.toggles);
        rc = 1;
    }
    KuiStr ns = {0};
    if (rc == 0 && !kui_ctx_extension_namespace(ctx, host.reply_from, &ns)) {
        fprintf(stderr, "FAIL: the reply's origin %u names no extension\n",
                host.reply_from);
        rc = 1;
    }
    if (rc == 0)
        printf("ok: the slot filled, the click reached the plugin and not the "
               "host, and its reply came back from `%.*s`\n",
               (int)ns.len, (const char *)ns.ptr);
    kui_ctx_free(ctx);
    return rc;
}

int main(int argc, char **argv) {
    bool no_window = false;
    const char *plugin = DEFAULT_PLUGIN;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--headless") == 0) no_window = true;
        else plugin = argv[i];
    }
    if (no_window) return headless(plugin);

    KuiCtx *ctx = load(plugin);
    if (!ctx) return 1;
    Host host = {0};
    bool ok = kui_run_with(ctx, KUI_STR("kui - a C host and a C panel"), view,
                           on_event, &host);
    kui_ctx_free(ctx);
    return ok ? 0 : 1;
}
