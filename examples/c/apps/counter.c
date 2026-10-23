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
#include <math.h>
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
 * Tab ring is its own and Escape or a press outside emits a dismiss event
 * on it: the modal's message, {kind="menu", dismiss={reason}}. Nothing here is special-cased in the core — it is a float plus
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

    /* Right-clicking the card asks for a menu; the message comes back as
     * the payload of the contextmenu event, the point in it:
     * {kind="card", contextmenu={x, y}}. Borrowed: the spec clones it
     * while the node opens, so it is freed below. */
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

/* Reads an integer field off the event's data (x / y arrive as numbers). */
static float data_num(const KuiEvent *ev, const char *key) {
    const KuiValue *v =
        kui_value_get(ev_data(ev), (KuiStr){(const uint8_t *)key, strlen(key)});
    int64_t n = 0;
    return v && kui_value_as_int(v, &n) ? (float)n : 0.0f;
}

/* The message says which of the app's things it is, KuiEvent.event what
 * happened to it (ADR 0047). */
static void apply_event(AppState *state, const KuiEvent *ev) {
    KuiStr s;
    if (!ev_message(ev, &s)) return;
    if (kui_str_eq(s, "inc")) state->count++;
    if (kui_str_eq(s, "dec")) state->count--;
    /* The card's secondary press: open the menu where it landed. */
    if (kui_str_eq(s, "card") && ev_is(ev, "contextmenu")) {
        state->menu_open = 1;
        state->menu_x = data_num(ev, "x");
        state->menu_y = data_num(ev, "y");
    }
    /* Escape, or a press outside the menu: stop declaring it. */
    if (kui_str_eq(s, "menu") && ev_is(ev, "dismiss")) state->menu_open = 0;
    if (kui_str_eq(s, "add10")) { state->count += 10; state->menu_open = 0; }
    if (kui_str_eq(s, "reset")) { state->count = 0; state->menu_open = 0; }
}

static void on_event(void *user, const KuiEvent *ev) {
    apply_event((AppState *)user, ev);
}

/* The window going for good - its close button, Quit from the menu or
 * the dock - before kui_run returns or, on a Mac's Quit, the process
 * ends without it returning: the place an app saves what it would lose.
 * This one only says the count it went with. */
static void teardown(void *user) {
    printf("teardown at count %lld\n", ((AppState *)user)->count);
    fflush(stdout);
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

    /* The last frame as pixels (backlog C55), drawn again on a GPU device
     * of the process's own. A context that never finished a frame has
     * none; a machine with no GPU adapter has no pixels for either, and
     * says so on stderr - a failure where KUI_REQUIRE_GPU is set, as CI
     * sets it over Mesa's software Vulkan. */
    uint32_t cw = 7, ch = 7;
    KuiCtx *blank = kui_ctx_new();
    uint8_t *none = kui_capture(blank, &cw, &ch);
    kui_ctx_free(blank);
    if (none || cw || ch) {
        fprintf(stderr, "FAIL: kui_capture of a context with no frame\n");
        kui_pixels_free(none);
        kui_ctx_free(ctx);
        return 1;
    }
    uint8_t *shot = kui_capture(ctx, &cw, &ch);
    if (shot) {
        if (cw != 800 || ch != 600 || shot[3] != 255) {
            fprintf(stderr, "FAIL: kui_capture: %ux%u, alpha %u\n", cw, ch, shot[3]);
            kui_pixels_free(shot);
            kui_ctx_free(ctx);
            return 1;
        }
        printf("captured the last frame: %ux%u\n", cw, ch);
    } else {
        const char *require = getenv("KUI_REQUIRE_GPU");
        if (require && *require) {
            fprintf(stderr, "FAIL: KUI_REQUIRE_GPU is set, but kui_capture drew nothing\n");
            kui_ctx_free(ctx);
            return 1;
        }
        printf("no GPU adapter: kui_capture had nothing to draw on\n");
    }
    kui_pixels_free(shot);

    /* Pixels to a file's bytes and back through the runner's own codecs,
     * which a test keeping a capture as a file needs no library for: two
     * pixels as a PNG, then a three-frame GIF whose middle frame repeats
     * the first and folds into its time. */
    static const uint8_t two[] = {255, 0, 0, 255, 0, 0, 255, 128};
    size_t png_len = 0;
    uint8_t *png = kui_encode_image(KUI_ENCODE_PNG, two, 2, 1, 0, &png_len);
    uint32_t pw = 0, ph = 0;
    uint8_t *again = png ? kui_decode_image(png, png_len, &pw, &ph) : NULL;
    int png_ok = png && png_len > 8 && png[1] == 'P' && again && pw == 2 && ph == 1 &&
                 memcmp(again, two, sizeof two) == 0;
    kui_pixels_free(again);
    kui_pixels_free(png);
    static const uint8_t red[] = {255, 0, 0, 255, 255, 0, 0, 255}, blue[] = {0, 0, 255, 255, 0, 0, 255, 255};
    KuiAnimationEncoder *enc = kui_animation_encoder_new(KUI_ENCODE_GIF, 2, 1, 0);
    int added = enc && kui_animation_encoder_add(enc, red, 0.1) && kui_animation_encoder_add(enc, red, 0.1) &&
                kui_animation_encoder_add(enc, blue, 0.3);
    size_t gif_len = 0;
    uint8_t *gif_out = enc ? kui_animation_encoder_finish(enc, &gif_len) : NULL;
    uint32_t gw = 0, gh = 0, gcount = 0, gloops = 7;
    const double *gdelays = NULL;
    uint8_t *gframes = gif_out ? kui_decode_animation(gif_out, gif_len, &gw, &gh, &gcount, &gloops, &gdelays) : NULL;
    int gif_ok = added && gframes && gcount == 2 && gloops == 0 && fabs(gdelays[0] - 0.2) < 1e-9 &&
                 gframes[2 * 4 + 2] == 255;
    kui_pixels_free(gframes);
    kui_pixels_free(gif_out);
    kui_animation_encoder_free(kui_animation_encoder_new(KUI_ENCODE_PNG, 2, 1, 0));
    if (!png_ok || !gif_ok || kui_encode_image(KUI_ENCODE_PNG, NULL, 2, 1, 0, &png_len) || png_len ||
        kui_animation_encoder_new(KUI_ENCODE_JPEG, 2, 1, 0)) {
        fprintf(stderr, "FAIL: kui_encode_image / kui_animation_encoder_*\n");
        kui_ctx_free(ctx);
        return 1;
    }
    printf("encoded a PNG and a GIF animation and read them back\n");

    kui_ctx_free(ctx);
    if (state.count != 1) {
        fprintf(stderr, "FAIL: expected count 1\n");
        return 1;
    }
    /* The icon's refusals (backlog F86), which need no window: a size
     * with no pixels, and a resource id past 16 bits, keeping nothing. */
    if (kui_set_icon(NULL, 32, 32, 0) || kui_set_icon(NULL, 0, 0, 70000) ||
        !kui_set_icon(NULL, 0, 0, 0)) {
        fprintf(stderr, "FAIL: kui_set_icon took what is not an icon\n");
        return 1;
    }
    printf("kui_set_icon refused a size with no pixels and a 17-bit resource\n");

    /* The runner's decoder (backlog F138), which needs no window either:
     * a 2x2 GIF, red for 50 ms then blue for 100, played once. */
    static const uint8_t gif[] = {
    0x47, 0x49, 0x46, 0x38, 0x39, 0x61, 0x02, 0x00, 0x02, 0x00, 0x80, 0x00,
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x21, 0xff, 0x0b, 0x4e, 0x45,
    0x54, 0x53, 0x43, 0x41, 0x50, 0x45, 0x32, 0x2e, 0x30, 0x03, 0x01, 0x01,
    0x00, 0x00, 0x21, 0xf9, 0x04, 0x08, 0x05, 0x00, 0x00, 0x00, 0x2c, 0x00,
    0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00, 0x80, 0xff, 0x00, 0x00, 0x00,
    0x00, 0x00, 0x02, 0x02, 0x84, 0x51, 0x00, 0x21, 0xf9, 0x04, 0x08, 0x0a,
    0x00, 0x00, 0x00, 0x2c, 0x00, 0x00, 0x00, 0x00, 0x02, 0x00, 0x02, 0x00,
    0x80, 0x00, 0x00, 0xff, 0x00, 0x00, 0x00, 0x02, 0x02, 0x84, 0x51, 0x00,
    0x3b,
    };
    uint32_t w = 0, h = 0, count = 0, loops = 0;
    uint8_t *still = kui_decode_image(gif, sizeof gif, &w, &h);
    int still_ok = still && w == 2 && h == 2 && still[0] == 255 && still[2] == 0;
    kui_pixels_free(still);
    const double *delays = NULL;
    uint8_t *frames = kui_decode_animation(gif, sizeof gif, &w, &h, &count, &loops, &delays);
    double next = 0;
    int anim_ok = frames && count == 2 && loops == 1 && delays[0] == 0.05 && delays[1] == 0.1 &&
                  frames[w * h * 4 + 2] == 255 &&
                  kui_animation_at(delays, count, loops, 0.07, &next) == 1 && fabs(next - 0.15) < 1e-9 &&
                  kui_animation_at(delays, count, loops, 5.0, &next) == 1 && isinf(next);
    kui_pixels_free(frames);
    if (!still_ok || !anim_ok || kui_decode_image((const uint8_t *)"nope", 4, &w, &h) || w) {
        fprintf(stderr, "FAIL: kui_decode_image / kui_decode_animation\n");
        return 1;
    }
    printf("decoded a GIF's first frame, both frames and their delays\n");

    printf("headless self-test OK\n");
    return 0;
}

/* Every window's icon (backlog F86): a disc in the buttons' blue, drawn
 * here rather than read from a file. Windows shows it in the title bar,
 * Alt-Tab and the taskbar, X11 in the window manager's; a Mac has no
 * window icon, so it is the app's in the Dock (backlog F181). A shipped Windows program
 * would pass its .rc's icon resource as the last argument instead. */
static int set_icon(void) {
    enum { N = 64 };
    static uint8_t px[N * N * 4];
    for (int y = 0; y < N; y++) {
        for (int x = 0; x < N; x++) {
            /* r - d, near the edge, is (r² - d²) / 2r: one pixel of
             * antialiasing without libm. */
            float dx = x + 0.5f - N / 2.0f, dy = y + 0.5f - N / 2.0f, r = N / 2.0f - 2.0f;
            float edge = (r * r - dx * dx - dy * dy) / (2.0f * r);
            float a = edge < 0 ? 0 : edge > 1 ? 1 : edge;
            uint8_t *p = &px[(y * N + x) * 4];
            p[0] = 0x3b; p[1] = 0x82; p[2] = 0xf6; p[3] = (uint8_t)(a * 255.0f);
        }
    }
    return kui_set_icon(px, N, N, 0);
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
    /* And the count is printed as the window goes, whichever way it goes. */
    kui_on_teardown(teardown);
    if (!set_icon()) return 1;
    /* The Linux desktop's name for the window (backlog F172): Wayland's
     * app_id, X11's WM_CLASS, matched to a kui-c-counter.desktop if one is
     * installed. */
    kui_set_app_id(KUI_STR("kui-c-counter"));
    return kui_run(KUI_STR("kui — C counter"), view, on_event, &state) ? 0 : 1;
}
