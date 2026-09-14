/* The counter, from C: the one example every binding has in the same
 * shape (docs/adr/0021, decision 3), driven entirely through the C API.
 * `view` rebuilds the tree from the state; a click arrives in `on_event`
 * as data and moves the state; a right-click asks for a menu the next
 * frame declares as a `modal` float; nothing else happens.
 *
 *   cargo run -p kui-devtools --bin cbuild   # builds libkui_ffi + this file
 *   ./target/debug/counter          # opens a window (winit + wgpu)
 *   ./target/debug/counter --headless
 *       no window: builds a frame, clicks +1 by its quad, right-clicks
 *       for the menu, dismisses it with Escape, and exits non-zero on a
 *       wrong answer — the Rosetta drive every counter runs.
 *
 * The header walk (every prototype in kui.h called once) and the corpus
 * adapter used to live in this file; they are tools/surface.c and
 * tools/conformance.c now, since neither is a counter.
 */
#include <stdlib.h>
#include "../common.h"

typedef struct AppState {
    long long count;
    /* The context menu: open where the last secondary press landed. The
     * core opens nothing — the view declares the menu while this is set,
     * and a dismiss event clears it. */
    int menu_open;
    float menu_x, menu_y;
} AppState;

/* -- view: rebuild the whole tree from state, every frame ---------------- */

/* The menu the right-click asks for: a float at the press, modal so the
 * Tab ring is its own and Escape or a press outside emits {kind="dismiss"}
 * on it. Nothing here is special-cased in the core — it is a float plus
 * the `modal` prop. */
static void context_menu(KuiCtx *ui, const AppState *state) {
    KuiTheme t = KUI_THEME_INIT;
    kui_theme(ui, &t);
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR("menu")));
    KuiSpec menu = {
        .dir = KUI_COLUMN, .gap = 4, .width = {KUI_FIXED, 120},
        .pad_l = 4, .pad_r = 4, .pad_t = 4, .pad_b = 4,
        .bg = t.raised, .radius = 6,
        .border_w = 1, .border_color = t.border_strong,
        .float_mode = KUI_FLOAT_VIEWPORT,
        .float_anchor_x = KUI_START, .float_anchor_y = KUI_START,
        .float_self_x = KUI_START, .float_self_y = KUI_START,
        .float_dx = state->menu_x, .float_dy = state->menu_y,
        .float_fit = 1, /* keep it on screen near the right or bottom edge */
        .modal = tag,
        /* A modal is announced as a dialog, and a dialog is named by its
         * label alone: without one the core says so (modal-without-name). */
        .label = KUI_STR("Actions"),
    };
    kui_open(ui, &menu, NULL);
    {
        KuiValue *add = kui_value_map();
        kui_value_map_set(add, KUI_STR("kind"), kui_value_str(KUI_STR("add10")));
        kui_button(ui, KUI_STR("+10"), add);

        KuiValue *reset = kui_value_map();
        kui_value_map_set(reset, KUI_STR("kind"), kui_value_str(KUI_STR("reset")));
        /* The sentence a reader hears after the name: the stock button
         * takes the access rows off a spec and nothing else (kui_button_with). */
        KuiSpec reset_rows = {.description = KUI_STR("Back to zero")};
        kui_button_with(ui, KUI_STR("Reset"), &reset_rows, reset);
    }
    kui_close(ui);
    kui_value_free(tag);
}

static void view(void *user, KuiCtx *ui) {
    AppState *state = (AppState *)user;

    /* The palette the core derived from what this host pushed through
     * kui_env_set_system - the appearance picks the base, the accent
     * recolours it (ADR 0019). Every colour below is a role, so the
     * window follows the OS with no branch of its own; a host that says
     * nothing gets the dark base, which is what C always painted. */
    KuiTheme t = KUI_THEME_INIT;
    kui_theme(ui, &t);

    KuiSpec root = {
        .width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
        .main_align = KUI_CENTER, .cross_align = KUI_CENTER, .gap = 24,
        .bg = t.bg,
    };
    kui_root(ui, &root);

    /* Right-clicking the card asks for a menu; the tag comes back under
     * `tag` on the {kind="contextmenu", x, y} event. Borrowed: the spec
     * clones it while the node opens, so it is freed below. */
    KuiValue *menu_tag = kui_value_map();
    kui_value_map_set(menu_tag, KUI_STR("kind"), kui_value_str(KUI_STR("card")));
    KuiSpec card = {
        .dir = KUI_COLUMN, .gap = 20, .cross_align = KUI_CENTER,
        .pad_l = 32, .pad_r = 32, .pad_t = 32, .pad_b = 32,
        .bg = t.surface, .radius = 12,
        .border_w = 1, .border_color = t.border,
        .on_context_menu = menu_tag,
    };
    kui_open(ui, &card, NULL);
    {
        KuiTextStyle muted = {.size = 14, .color = t.muted};
        kui_text(ui, KUI_STR("kui from C"), &muted);

        char buf[32];
        snprintf(buf, sizeof buf, "%lld", state->count);
        KuiTextStyle big = {.size = 56};
        kui_text(ui, KUI_STR(buf), &big);

        KuiSpec row = {.dir = KUI_ROW, .gap = 12};
        kui_open(ui, &row, NULL);
        {
            KuiValue *dec = kui_value_map();
            kui_value_map_set(dec, KUI_STR("kind"), kui_value_str(KUI_STR("dec")));
            kui_button(ui, KUI_STR("-1"), dec);

            KuiValue *inc = kui_value_map();
            kui_value_map_set(inc, KUI_STR("kind"), kui_value_str(KUI_STR("inc")));
            kui_button(ui, KUI_STR("+1"), inc);
        }
        kui_close(ui);

        KuiSpan spans[] = {
            {.text = KUI_STR("same IR as Rust and ")},
            {.text = KUI_STR("Lua"), .color = t.success, .flags = KUI_SPAN_BOLD},
            {.text = KUI_STR(" — just flatter"), .flags = KUI_SPAN_ITALIC},
        };
        KuiTextStyle base = {.size = 13, .color = t.faint};
        kui_rich_text(ui, spans, 3, &base);
    }
    kui_close(ui);
    kui_value_free(menu_tag);

    if (state->menu_open) context_menu(ui, state);
}

/* -- events: clicks arrive as data --------------------------------------- */

/* Reads an integer field off a payload (x / y arrive as numbers). */
static float payload_num(const KuiEvent *ev, const char *key) {
    const KuiValue *v =
        kui_value_get(ev->payload, (KuiStr){(const uint8_t *)key, strlen(key)});
    int64_t n = 0;
    return v && kui_value_as_int(v, &n) ? (float)n : 0.0f;
}

static void apply_event(AppState *state, const KuiEvent *ev) {
    if (!ev->payload) return;
    const KuiValue *kind = kui_value_get(ev->payload, KUI_STR("kind"));
    KuiStr s;
    if (!kind || !kui_value_as_str(kind, &s)) return;
    if (kui_str_eq(s, "inc")) state->count++;
    if (kui_str_eq(s, "dec")) state->count--;
    /* The secondary press: open the menu where it landed. */
    if (kui_str_eq(s, "contextmenu")) {
        state->menu_open = 1;
        state->menu_x = payload_num(ev, "x");
        state->menu_y = payload_num(ev, "y");
    }
    /* Escape, or a press outside the menu: stop declaring it. */
    if (kui_str_eq(s, "dismiss")) state->menu_open = 0;
    if (kui_str_eq(s, "add10")) { state->count += 10; state->menu_open = 0; }
    if (kui_str_eq(s, "reset")) { state->count = 0; state->menu_open = 0; }
}

static void on_event(void *user, const KuiEvent *ev) {
    apply_event((AppState *)user, ev);
}

/* -- headless self-test: full loop without a window ---------------------- */

static int headless(void) {
    AppState state = {0};
    KuiCtx *ctx = kui_ctx_new();
    if (!ctx) return 1;

    /* Frame 1: lay out so hit regions exist. */
    kui_frame_begin(ctx, 800, 600, 1.0f);
    view(&state, ctx);
    kui_frame_finish(ctx);

    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ctx, &dd);
    printf("frame 1: %zu quads, viewport %.0fx%.0f, atlas %ux%u (dirty=%d)\n",
           dd.quad_count, dd.viewport_w, dd.viewport_h, dd.atlas_size, dd.atlas_size,
           dd.atlas_dirty);
    if (dd.quad_count == 0) {
        fprintf(stderr, "FAIL: empty display list\n");
        return 1;
    }

    /* Find the "+1" button's quad center: solid quads with the button blue. */
    float bx = -1, by = -1;
    for (size_t i = 0; i < dd.quad_count; i++) {
        const KuiQuad *q = &dd.quads[i];
        if (q->kind == KUI_QUAD_SOLID && q->color[2] > 0.7f && q->color[0] < 0.4f) {
            /* Second button (larger x) is +1. */
            float cx = q->x + q->w / 2, cy = q->y + q->h / 2;
            if (cx > bx) { bx = cx; by = cy; }
        }
    }
    if (bx < 0) {
        fprintf(stderr, "FAIL: no button quad found\n");
        return 1;
    }

    /* Click it (logical == physical at scale 1). */
    kui_input_cursor(ctx, bx, by);
    kui_input_mouse(ctx, true, 1);
    kui_input_mouse(ctx, false, 1);

    KuiEvent ev = KUI_EVENT_INIT;
    int got = 0;
    while (kui_poll_event(ctx, &ev)) {
        apply_event(&state, &ev);
        got++;
    }
    printf("clicked (%.0f, %.0f): %d event(s), count = %lld\n", bx, by, got, state.count);

    /* The secondary button: the card asks for a menu at the press, and the
     * press moves nothing else. Above the buttons, so the card is the
     * topmost node under the pointer — a right-click is routed like a
     * click, and a button that declared no menu of its own takes it. */
    kui_input_cursor(ctx, bx, by - 60);
    kui_input_mouse_button(ctx, true, KUI_MOUSE_SECONDARY, 1);
    kui_input_mouse_button(ctx, false, KUI_MOUSE_SECONDARY, 1);
    got = 0;
    while (kui_poll_event(ctx, &ev)) {
        apply_event(&state, &ev);
        got++;
    }
    printf("right-clicked: %d event(s), menu at (%.0f, %.0f)\n", got, state.menu_x, state.menu_y);
    if (got != 1 || !state.menu_open || state.count != 1) {
        fprintf(stderr, "FAIL: expected one contextmenu event and no click\n");
        kui_ctx_free(ctx);
        return 1;
    }

    /* Declare it, then let Escape ask for it back: a modal float dismisses
     * itself the way the dialog in the Rust examples does.
     *
     * kui_input_press is the whole key, which is what a host with a real
     * keyboard sends: the raw press to an on_key sink, and then what the
     * core is asked to do with that key - dismissal, here. Driving only
     * kui_input_key_down leaves this menu open, because dismissal is on
     * the other channel. */
    kui_frame_begin(ctx, 800, 600, 1.0f);
    view(&state, ctx);
    kui_frame_finish(ctx);
    {
        KuiStr none = {0};
        kui_input_press(ctx, KUI_STR("escape"), none, 0, none, false);
        kui_input_release(ctx, KUI_STR("escape"), none, 0);
    }
    while (kui_poll_event(ctx, &ev)) apply_event(&state, &ev);
    if (state.menu_open) {
        fprintf(stderr, "FAIL: escape did not dismiss the menu\n");
        kui_ctx_free(ctx);
        return 1;
    }
    printf("escape dismissed the menu\n");

    kui_ctx_free(ctx);
    if (state.count != 1) {
        fprintf(stderr, "FAIL: expected count 1\n");
        return 1;
    }
    printf("headless self-test OK\n");
    return 0;
}

/* -- surface self-test: the rest of the header --------------------------
 *
 * The counter above uses about a fifth of kui.h, so most of the header is
 * never compiled against a real call, and a declaration that has drifted
 * from the library would go unnoticed (the ABI asserts the cbuild tool
 * compiles cover the structs, not the functions). This second headless pass
 * walks what is left - measurement, keyed nodes and animation, hover and
 * keyboard focus, editors, accessibility, images, fonts, audio, window
 * chrome, diagnostics, values - and checks what comes back, so it is a
 * self-test and not only a link check.
 */

int main(int argc, char **argv) {
    if (!abi_ok()) return 1;
    if (argc > 1 && strcmp(argv[1], "--headless") == 0) return headless();
    AppState state = {0};
    return kui_run(KUI_STR("kui — C counter"), view, on_event, &state) ? 0 : 1;
}
