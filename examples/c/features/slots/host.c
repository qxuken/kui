/* C as the host of a C extension: the third corner of the extension story.
 *
 * apps/counter.c is C with kui as a library. panel.c is C as a *guest*
 * inside a Rust host (panel.rs). This is the pair those two left open - C on
 * both sides - and until ABI 10 the C API could not express it: loading a
 * plugin was `kui_ffi::CExtension`, which is Rust, so a C host had nothing to
 * place.
 *
 * It loads panel.c, beside this file, under a namespace of its own choosing,
 * places that plugin's `panel` component in the middle of its own view with
 * kui_component (docs/adr/0067-slots-fold-into-components.md), and counts the
 * `on_toggle` message it passed in the props as it comes back. It also
 * declares a contribution slot, "todos.toolbar", with kui_slot: it names no
 * plugin there, and whatever the loaded plugins contribute to it is placed
 * in its column - the panel's chip - its press arriving as the host's
 * on_press with KuiEvent.from saying who raised it. The panel is
 * the *same binary* panel.rs loads: an extension does not know or care what
 * language its host is written in, which is the point of the extension
 * contract being C in the first place.
 *
 * Run:
 *   cargo run -p kui-devtools --bin cbuild -- --run   # every platform
 *   ./target/debug/host --headless        # no window: a frame, a click, asserts
 *   ./target/debug/host                   # a window
 *   ./target/debug/host --headless path/to/plugin
 *
 * --headless is the contract without a display, and it is what CI runs: it
 * builds a frame the way the runner does, finds a row the plugin drew,
 * clicks it, and checks that the click reached the plugin and not the host
 * and that the plugin's emit reached the host as the host's own message,
 * about the panel's instance; then that the toolbar slot placed the
 * panel's chip and that a press on it arrives with `from` naming the
 * panel's namespace. Which is panel.rs's --headless, from the other side.
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "kui.h"

/* The namespace we load the panel under: ours to choose, and what fronts
 * the component's name. The plugin calls its own component "panel". */
#define NS "todos"
#define PANEL NS "/panel"

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
 * the cbuild tool's header (examples/devtools/src/bin/cbuild.rs). A DLL must name the module each import comes from. panel.dll
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
#define BUILD_HINT "cargo run -p kui-devtools --bin cbuild"
#else
#define DEFAULT_PLUGIN PLUGIN_DIR "panel.so"
#define BUILD_HINT "cargo run -p kui-devtools --bin cbuild"
#endif

typedef struct {
    long long clicks;     /* our own button */
    long long toggles;    /* the panel's on_toggle, heard as our own message */
    int64_t toggled;      /* which row it said, from the fields riding with it */
    uint64_t toggle_from; /* the instance the event is about: KuiEvent.key */
    long long pressed;    /* the badge's onPress, heard as our own message */
    int32_t panel_fill;   /* how the last frame filled the panel: a KUI_FILL_* code */
    size_t toolbar;       /* how many contributions the toolbar slot placed */
    long long chipped;    /* the slot's on_press, from whichever contributor */
    char chip_from[32];   /* who raised the last one: KuiEvent.from, copied */
} Host;

static KuiValue *msg(const char *kind) {
    KuiValue *m = kui_value_map();
    kui_value_map_set(m, KUI_STR("kind"), kui_value_str(KUI_STR(kind)));
    return m;
}

/* Our children of the badge, built where it asks: under our origin, so
 * their events are ours. */
static void badge_children(void *user, KuiCtx *ui, KuiStr group) {
    (void)user;
    if (group.len != 0) return;
    KuiTextStyle small = {.size = 12};
    kui_text(ui, KUI_STR("host child"), &small);
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

    /* Our half. Its colours are theme roles (ADR 0019), so the host and
     * the plugin it loads agree on what a surface is without either of
     * them being told. */
    KuiTheme t = KUI_THEME_INIT;
    kui_theme(ui, &t);
    KuiSpec col = {
        .dir = KUI_COLUMN,
        .width = {KUI_GROW, 1},
        .height = {KUI_GROW, 1},
        .pad_l = 24, .pad_r = 24, .pad_t = 24, .pad_b = 24,
        .gap = 12,
        .bg = t.surface,
        .radius = 10,
    };
    kui_open(ui, &col, NULL);
    {
        KuiTextStyle title = {.size = 18};
        kui_text(ui, KUI_STR("the host, in C"), &title);

        char line[80];
        snprintf(line, sizeof line, "clicks %lld, toggles heard %lld", h->clicks,
                 h->toggles);
        KuiTextStyle body = {.size = 13, .color = t.muted};
        kui_text(ui, (KuiStr){(const uint8_t *)line, strlen(line)}, &body);

        kui_button(ui, KUI_STR("count"), msg("count"));

        /* A contribution slot (ADR 0067): we name a place, not a plugin.
         * Every loaded extension that contributes to "todos.toolbar" puts
         * one of its components here, as siblings in this column, each with
         * these props - so on_press is the message any of them raises, and
         * the event's `from` says which. The answer is how many were
         * placed; nothing contributing is 0 and no warning. */
        KuiValue *slot_props = kui_value_map();
        kui_value_map_set(slot_props, KUI_STR("on_press"), msg("chipped"));
        h->toolbar = kui_slot(ui, KUI_STR("todos.toolbar"), slot_props);
        kui_value_free(slot_props);
    }
    kui_close(ui);

    /* The plugin's half, in place: a component placed among our own
     * children, drawn then and there by whoever we loaded under `todos`.
     * The props are its title and the message we want back when a todo is
     * toggled - declared every frame, retained by nobody, ours to free.
     * `key` keys the instance by name rather than by position.
     *
     * Replayed by default (ADR 0067): nothing this host feeds the panel
     * changes between frames, so the core pushes the panel's nodes again
     * without asking it - unless something it read of the frame moved (the
     * pointer onto a row it hover-tracks) or it heard an event, when it
     * runs the panel and the answer says why. */
    KuiValue *params = kui_value_map();
    kui_value_map_set(params, KUI_STR("key"), kui_value_str(KUI_STR(PANEL)));
    kui_value_map_set(params, KUI_STR("title"), kui_value_str(KUI_STR("todos, from C")));
    kui_value_map_set(params, KUI_STR("on_toggle"), msg("toggled"));
    h->panel_fill = kui_component(ui, KUI_STR(PANEL), params, NULL, NULL);
    kui_value_free(params);

    /* And a second component the panel provides (docs/adr/0060): placed
     * like a node, as often as we like, with our own child inside it and
     * the message its press arrives as in its props. */
    KuiValue *props = kui_value_map();
    kui_value_map_set(props, KUI_STR("label"), kui_value_str(KUI_STR("badge")));
    kui_value_map_set(props, KUI_STR("onPress"), msg("pressed"));
    kui_component(ui, KUI_STR(NS "/badge"), props, badge_children, NULL);
    kui_value_free(props);
}

static void on_event(void *user, const KuiEvent *ev) {
    Host *h = (Host *)user;
    if (!ev->payload) return;
    const KuiValue *kind = kui_value_get(ev->payload, KUI_STR("kind"));
    KuiStr s;
    if (!kind || !kui_value_as_str(kind, &s)) return;

    /* Our own nodes come back with origin 0, and so do the messages we
     * passed a component: they are ours, raised by the extension with
     * kui_emit, and `event` says "component". The extension's own clicks
     * carry its origin and never reach us. */
    if (ev->origin != 0) return;
    if (kui_str_eq(s, "count")) {
        h->clicks++;
    } else if (kui_str_eq(s, "pressed") && kui_str_eq(ev->event, "component")) {
        h->pressed++;
    } else if (kui_str_eq(s, "chipped") && kui_str_eq(ev->event, "component")) {
        /* One message for the whole slot; `from` is the namespace of the
         * extension that raised it, borrowed like the payload - copied out,
         * since the next poll releases it. */
        h->chipped++;
        snprintf(h->chip_from, sizeof h->chip_from, "%.*s", (int)ev->from.len,
                 (const char *)ev->from.ptr);
    } else if (kui_str_eq(s, "toggled") && kui_str_eq(ev->event, "component")) {
        h->toggles++;
        /* What the panel said rides under the prop's field: on_toggle's is
         * `toggle`. And `key` is the instance it is about - one plugin may
         * be placed many times, and `origin` cannot say which. */
        const KuiValue *toggle = kui_value_get(ev->payload, KUI_STR("toggle"));
        const KuiValue *index = toggle ? kui_value_get(toggle, KUI_STR("index")) : NULL;
        if (!index || !kui_value_as_int(index, &h->toggled)) h->toggled = -1;
        h->toggle_from = ev->key;
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

/* The centre of the first node of `origin` named exactly `name`. */
static bool find_named(KuiCtx *ctx, uint32_t origin, const char *name, float *x, float *y) {
    size_t n = kui_access_tree(ctx, NULL, 0);
    if (n == 0) return false;
    KuiAccessNode *nodes = calloc(n, sizeof *nodes);
    if (!nodes) return false;
    n = kui_access_tree(ctx, nodes, n);
    bool found = false;
    for (size_t i = 0; i < n && !found; i++) {
        const KuiAccessNode *a = &nodes[i];
        if (a->origin != origin || !kui_str_eq(a->name, name)) continue;
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
    if (host.panel_fill != KUI_FILL_NOT_KEPT) {
        fprintf(stderr, "FAIL: the first frame did not fill the panel (%d)\n",
                host.panel_fill);
        kui_ctx_free(ctx);
        return 1;
    }
    /* A second frame with nothing moved: the kept fill is replayed, and
     * the row is still there to find (ADR 0067). */
    frame(ctx, &host);
    if (host.panel_fill != KUI_FILL_REPLAYED) {
        fprintf(stderr, "FAIL: the second frame did not replay the panel (%d)\n",
                host.panel_fill);
        kui_ctx_free(ctx);
        return 1;
    }

    float x = 0, y = 0;
    if (!find_row(ctx, 1, &x, &y)) {
        fprintf(stderr, "FAIL: the panel drew no rows - was it placed?\n");
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
        ev = KUI_EVENT_INIT;
    }

    /* The badge: press its dot, and the panel raises the onPress we passed. */
    float bx = 0, by = 0;
    if (!find_named(ctx, 1, "badge", &bx, &by)) {
        fprintf(stderr, "FAIL: the badge component drew no dot\n");
        kui_ctx_free(ctx);
        return 1;
    }
    kui_input_cursor(ctx, bx, by);
    kui_input_mouse(ctx, true, 1);
    kui_input_mouse(ctx, false, 1);
    frame(ctx, &host);
    ev = KUI_EVENT_INIT;
    while (kui_poll_event(ctx, &ev)) {
        on_event(&host, &ev);
        ev = KUI_EVENT_INIT;
    }

    /* The toolbar slot: one contribution, the panel's chip - found by the
     * name it gave itself from kui_instance_slot - and its press. */
    if (host.toolbar != 1) {
        fprintf(stderr, "FAIL: the toolbar slot placed %zu contributions, not 1\n",
                host.toolbar);
        kui_ctx_free(ctx);
        return 1;
    }
    float cx = 0, cy = 0;
    if (!find_named(ctx, 1, "chip in todos.toolbar", &cx, &cy)) {
        fprintf(stderr, "FAIL: the panel's chip is not in the toolbar slot\n");
        kui_ctx_free(ctx);
        return 1;
    }
    kui_input_cursor(ctx, cx, cy);
    kui_input_mouse(ctx, true, 1);
    kui_input_mouse(ctx, false, 1);
    frame(ctx, &host);
    ev = KUI_EVENT_INIT;
    while (kui_poll_event(ctx, &ev)) {
        on_event(&host, &ev);
        ev = KUI_EVENT_INIT;
    }

    int rc = 0;
    if (host.chipped != 1) {
        fprintf(stderr, "FAIL: expected the slot's on_press once, heard %lld\n",
                host.chipped);
        rc = 1;
    }
    if (strcmp(host.chip_from, NS) != 0) {
        fprintf(stderr, "FAIL: the on_press came from \"%s\", not \"%s\"\n",
                host.chip_from, NS);
        rc = 1;
    }
    if (host.pressed != 1) {
        fprintf(stderr, "FAIL: expected the badge's onPress once, heard %lld\n",
                host.pressed);
        rc = 1;
    }
    if (host.clicks != 0) {
        fprintf(stderr, "FAIL: the plugin's click reached the host (%lld)\n",
                host.clicks);
        rc = 1;
    }
    if (host.toggles != 1) {
        fprintf(stderr, "FAIL: expected the panel's on_toggle once, heard %lld\n",
                host.toggles);
        rc = 1;
    }
    if (rc == 0 && host.toggled < 0) {
        fprintf(stderr, "FAIL: the on_toggle carried no toggle.index\n");
        rc = 1;
    }
    /* The instance it is about is the panel's: placed with `key`, so
     * kui_key_of finds it by that name like a keyed node. */
    if (rc == 0 && host.toggle_from != kui_key_of(ctx, KUI_STR(PANEL))) {
        fprintf(stderr, "FAIL: the on_toggle names instance %llx, not %s's\n",
                (unsigned long long)host.toggle_from, PANEL);
        rc = 1;
    }
    /* Who drew the panel: the first extension, origin 1, named back. */
    KuiStr ns = {0};
    if (rc == 0 && !kui_ctx_extension_namespace(ctx, 1, &ns)) {
        fprintf(stderr, "FAIL: origin 1 names no extension\n");
        rc = 1;
    }
    if (rc == 0)
        printf("ok: `%.*s/panel` placed and replayed, the click reached the "
               "plugin and not the host, its on_toggle (row %lld), the "
               "badge's press and the toolbar chip's (from `todos`) arrived as "
               "our own messages\n",
               (int)ns.len, (const char *)ns.ptr, (long long)host.toggled);
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
    /* The window is the host's to describe (backlog AR27): a size and the
     * bound below which the panel's rows would wrap. NULL would be the
     * launcher's defaults. What is registered on the context before this
     * call - the plugin above, and any font or image - reaches the window. */
    KuiRunConfig window = KUI_RUN_CONFIG_INIT;
    window.width = 720;
    window.height = 480;
    window.min_w = 360;
    bool ok = kui_run_with(ctx, KUI_STR("kui - a C host and a C panel"), &window,
                           view, on_event, &host);
    kui_ctx_free(ctx);
    return ok ? 0 : 1;
}
