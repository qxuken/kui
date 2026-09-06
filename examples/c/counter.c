/* kui from C: a counter app driven entirely through the C API.
 *
 *   ./build.sh          # builds libkui_ffi + this file
 *   ./counter           # opens a window (winit + wgpu under the hood)
 *   ./counter --headless  # no window: builds a frame, simulates a click,
 *                         # verifies the event round-trip, prints draw stats,
 *                         # then walks the rest of the header (see surface())
 *   ./counter --conformance [report]
 *                         # rebuilds the shared scene corpus through the C
 *                         # API and diffs it against kui-core's reference
 *                         # report (see conformance() at the bottom)
 */
#include <stdio.h>
#include <stdarg.h>
#include <stdlib.h>
#include <string.h>
#include "kui.h"

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
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR("menu")));
    KuiSpec menu = {
        .dir = KUI_COLUMN, .gap = 4, .width = {KUI_FIXED, 120},
        .pad_l = 4, .pad_r = 4, .pad_t = 4, .pad_b = 4,
        .bg = 0x22242cff, .radius = 6,
        .border_w = 1, .border_color = 0x2a2d3aff,
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
        kui_button(ui, KUI_STR("Reset"), reset);
    }
    kui_close(ui);
    kui_value_free(tag);
}

static void view(void *user, KuiCtx *ui) {
    AppState *state = (AppState *)user;

    KuiSpec root = {
        .width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
        .main_align = KUI_CENTER, .cross_align = KUI_CENTER, .gap = 24,
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
        .bg = 0x161820ff, .radius = 12,
        .border_w = 1, .border_color = 0x2a2d3aff,
        .on_context_menu = menu_tag,
    };
    kui_open(ui, &card, NULL);
    {
        KuiTextStyle muted = {.size = 14, .color = 0x8a8fa3ff};
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
            {KUI_STR("same IR as Rust and "), 0, 0},
            {KUI_STR("Lua"), 0x73d98cff, KUI_SPAN_BOLD},
            {KUI_STR(" — just flatter"), 0, KUI_SPAN_ITALIC},
        };
        KuiTextStyle base = {.size = 13, .color = 0x5c6174ff};
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

static bool is(KuiStr s, const char *lit) {
    size_t n = strlen(lit);
    return s.len == n && memcmp(s.ptr, lit, n) == 0;
}

static void apply_event(AppState *state, const KuiEvent *ev) {
    if (!ev->payload) return;
    const KuiValue *kind = kui_value_get(ev->payload, KUI_STR("kind"));
    KuiStr s;
    if (!kind || !kui_value_as_str(kind, &s)) return;
    if (is(s, "inc")) state->count++;
    if (is(s, "dec")) state->count--;
    /* The secondary press: open the menu where it landed. */
    if (is(s, "contextmenu")) {
        state->menu_open = 1;
        state->menu_x = payload_num(ev, "x");
        state->menu_y = payload_num(ev, "y");
    }
    /* Escape, or a press outside the menu: stop declaring it. */
    if (is(s, "dismiss")) state->menu_open = 0;
    if (is(s, "add10")) { state->count += 10; state->menu_open = 0; }
    if (is(s, "reset")) { state->count = 0; state->menu_open = 0; }
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
     * itself the way the dialog in the Rust examples does. */
    kui_frame_begin(ctx, 800, 600, 1.0f);
    view(&state, ctx);
    kui_frame_finish(ctx);
    kui_input_key(ctx, KUI_KEY_ESCAPE, 0);
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
 * from the library would go unnoticed (the ABI asserts examples/c/build.sh
 * compiles cover the structs, not the functions). This second headless pass
 * walks what is left - measurement, keyed nodes and animation, hover and
 * keyboard focus, editors, accessibility, images, fonts, audio, window
 * chrome, diagnostics, values - and checks what comes back, so it is a
 * self-test and not only a link check.
 */

static int fails;

static void check(bool ok, const char *what) {
    if (!ok) {
        fprintf(stderr, "FAIL: %s\n", what);
        fails++;
    }
}

/* KuiStr is not NUL-terminated and memmem is not standard C. */
static bool has(KuiStr s, const char *needle) {
    size_t n = strlen(needle);
    if (n > s.len) return false;
    for (size_t i = 0; i + n <= s.len; i++) {
        if (memcmp(s.ptr + i, needle, n) == 0) return true;
    }
    return false;
}

/* Body callback for the widgets that host custom content. */
static void body_text(void *user, KuiCtx *ui) {
    KuiTextStyle s = {.size = 12};
    kui_text(ui, KUI_STR((const char *)user), &s);
}

typedef struct Keys {
    uint64_t image;    /* in: a registered image handle */
    uint64_t sound;    /* in: a registered sound handle */
    bool claim_focus;  /* in: declare the editor focused this frame */
    uint64_t card, slider, sink, editor, input, drag, child; /* out: node keys */
} Keys;

static void surface_view(void *user, KuiCtx *ui) {
    Keys *k = (Keys *)user;

    KuiSpec root = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1}, .gap = 8};
    kui_root(ui, &root);
    kui_window_title(ui, KUI_STR("surface"));

    /* Window chrome, each piece in its own keyed slot: two titlebars as
     * siblings would share a key, and per-key state (hover, transitions)
     * with it - the core warns about exactly that. */
    KuiSpec slot = {.dir = KUI_ROW, .width = {KUI_GROW, 1}};
    kui_open_keyed(ui, KUI_STR("chrome-plain"), &slot, NULL);
    kui_titlebar(ui, KUI_STR("surface"));
    kui_close(ui);
    kui_open_keyed(ui, KUI_STR("chrome-custom"), &slot, NULL);
    kui_titlebar_with(ui, body_text, (void *)"tab strip");
    kui_close(ui);
    kui_open_keyed(ui, KUI_STR("chrome-buttons"), &slot, NULL);
    kui_window_buttons(ui);
    kui_close(ui);

    /* One card carrying most of KuiSpec at once: clamps, per-corner radii,
     * transition + keyframes + entrance, a hover group, semantics, focus. */
    KuiKeyframe stops[] = {
        {.set = KUI_KF_AT | KUI_KF_BG, .at = 0.0f, .bg = 0x161820ff},
        {.set = KUI_KF_AT | KUI_KF_BG | KUI_KF_RADIUS, .at = 1.0f,
         .bg = 0x1b1e28ff, .radius = 10},
    };
    KuiValue *layout_tag = kui_value_str(KUI_STR("card"));
    KuiSpec card = {
        .dir = KUI_COLUMN, .gap = 8, .cross_align = KUI_START,
        .pad_l = 12, .pad_r = 12, .pad_t = 12, .pad_b = 12,
        .min_w = 240, .max_w = 600, .min_h = 40, .max_h = 500,
        .bg = 0x161820ff, .border_color = 0x2a2d3aff, .border_w = 1,
        .overflow = KUI_CLIP | KUI_SCROLL_Y,
        .per_corner = 1, .radius_tl = 10, .radius_tr = 10, .radius_br = 2, .radius_bl = 2,
        .transition_ms = 120, .easing = KUI_EASE_IN_OUT, .slide = 1,
        .hover_bg = 0x1b1e28ff, .pressed_bg = 0x22252fff,
        .hover_group = KUI_STR("card"),
        .repeat = KUI_REPEAT_ALTERNATE, .delay_ms = 20,
        .keyframes = stops, .keyframes_len = 2,
        .enter = {.set = KUI_ENTER_OFFSET | KUI_ENTER_BG, .dx = -12, .bg = 0},
        .on_layout = layout_tag,
        .role = KUI_ROLE_GROUP, .label = KUI_STR("surface card"),
        .focusable = 1, .focus_bg = 0x2b3350ff,
    };
    k->card = kui_open_keyed(ui, KUI_STR("card"), &card, NULL);
    kui_value_free(layout_tag); /* on_layout is cloned, not adopted */
    {
        k->child = kui_child_key(ui, KUI_STR("slot"));

        /* A slider: declared role, value and range; the arrows nudge it. */
        KuiSpec slider = {
            .width = {KUI_FIXED, 160}, .height = {KUI_FIXED, 20},
            .bg = 0x2a2d3aff, .radius = 10,
            .role = KUI_ROLE_SLIDER, .label = KUI_STR("volume"),
            .value_set = KUI_VALUE_NOW | KUI_VALUE_MIN | KUI_VALUE_MAX,
            .value_now = 40, .value_min = 0, .value_max = 100,
        };
        k->slider = kui_open_keyed(ui, KUI_STR("slider"), &slider, NULL);
        kui_close(ui);

        /* A checked switch that is also disabled: inert, and says so. */
        KuiSpec sw = {
            .width = {KUI_FIXED, 40}, .height = {KUI_FIXED, 20},
            .bg = 0x2a2d3aff, .radius = 10, .hoverable = 1,
            .role = KUI_ROLE_SWITCH, .label = KUI_STR("mute"),
            .checked = 1, .disabled = 1,
        };
        kui_open_keyed(ui, KUI_STR("switch"), &sw, NULL);
        kui_close(ui);

        /* A key sink (on_key) that is also a hover sink (on_hover): both
         * tags ride the events those produce. Focusable, so Tab reaches it
         * even without a click payload. */
        KuiSpec sink = {
            .width = {KUI_FIXED, 120}, .height = {KUI_FIXED, 24},
            .bg = 0x11131aff, .focusable = 1,
            .role = KUI_ROLE_GROUP, .label = KUI_STR("key sink"),
            /* Releases are opt-in: a sink that only says on_key hears
             * presses, which is what a keymap wants. This one counts ups. */
            .key_up = 1,
        };
        k->sink = kui_open_with(ui, KUI_STR("sink"), &sink, NULL, NULL,
                                kui_value_str(KUI_STR("sink")),
                                kui_value_str(KUI_STR("sink")));
        kui_close(ui);

        /* A draggable strip, floated into the card's top-right corner. */
        KuiSpec strip = {
            .width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 16},
            .bg = 0x3a3f52ff,
            .float_mode = KUI_FLOAT_PARENT,
            .float_anchor_x = KUI_END, .float_anchor_y = KUI_START,
            .float_self_x = KUI_END, .float_self_y = KUI_START,
            .float_dx = -4, .float_dy = 4, .float_fit = 1,
        };
        k->drag = kui_open_draggable(ui, KUI_STR("strip"), &strip, NULL,
                                     kui_value_str(KUI_STR("strip")));
        kui_close(ui);

        /* Editors: the styled single-line widget and a raw multiline node. */
        k->input = kui_text_input(ui, KUI_STR("name"), KUI_STR("ada"));
        KuiTextStyle mono = {.size = 13, .family = KUI_FONT_MONO, .wrap = KUI_WRAP_GLYPH};
        KuiSpec editor = {
            .width = {KUI_PERCENT, 0.5f}, .height = {KUI_FIXED, 48},
            .bg = 0x11131aff, .radius = 4, .label = KUI_STR("notes"),
        };
        k->editor = kui_text_edit(ui, KUI_STR("notes"), KUI_STR("line one"), &mono,
                                  KUI_EDIT_MULTILINE, &editor);
        if (k->claim_focus) kui_set_key_focus(ui, k->editor);

        /* An image node, a tooltip gated on hover, and the latency panels. */
        KuiSpec pic = {
            .width = {KUI_FIXED, 32}, .height = {KUI_FIT, 0}, .radius = 4,
            .role = KUI_ROLE_IMAGE, .label = KUI_STR("swatch"),
        };
        kui_image(ui, k->image, &pic);
        if (kui_is_hovered(ui, k->card) || kui_is_pressed(ui, k->card)) {
            kui_tooltip(ui, KUI_STR("the card"));
        } else {
            kui_tooltip_with(ui, body_text, (void *)"custom tip");
        }
        /* An audio node: a playback retained for as long as the frame
         * declares it. Draws nothing; without a device (kui_run supplies
         * one) it just queues commands for the host. */
        KuiAudio track = KUI_AUDIO_INIT(k->sound);
        track.looped = 1;
        track.paused = 1;
        kui_audio(ui, KUI_STR("bed"), &track, kui_value_str(KUI_STR("bed")));

        kui_latency_graph(ui);
        kui_latency_hud(ui, KUI_END, KUI_START);
    }
    kui_close(ui);
}

static int surface(void) {
    KuiCtx *ui = kui_ctx_new();
    if (!ui) {
        fprintf(stderr, "FAIL: kui_ctx_new\n");
        return 1;
    }
    kui_set_diagnostics(ui, true);
    kui_env_set(ui, 120.0f, true);
    kui_env_set_window(ui, KUI_WINDOW_MAIN, true, false, false, 78, 28);
    kui_set_subpixel_text(ui, false);

    /* Measurement works before the first frame. */
    KuiTextStyle body = {.size = 16};
    KuiTextMetrics one = KUI_TEXT_METRICS_INIT, wrapped = KUI_TEXT_METRICS_INIT,
                   rich = KUI_TEXT_METRICS_INIT;
    check(kui_measure_text(ui, KUI_STR("measure me"), &body, 0, &one), "kui_measure_text");
    check(one.width > 0 && one.height > 0 && one.lines == 1, "unwrapped is one line");
    check(kui_measure_text(ui, KUI_STR("measure me measure me"), &body, one.width, &wrapped),
          "kui_measure_text wrapped");
    check(wrapped.lines > 1 && wrapped.height > one.height, "a narrow width wraps");
    KuiSpan spans[] = {
        {KUI_STR("rich "), 0, KUI_SPAN_BOLD},
        {KUI_STR("measure"), 0x73d98cff, KUI_SPAN_ITALIC},
    };
    check(kui_measure_rich_text(ui, spans, 2, &body, 0, &rich), "kui_measure_rich_text");
    check(rich.width > 0 && rich.lines == 1, "spans measure as one line");

    /* Resources. Unusable input is a 0 handle, not a crash. */
    const uint8_t junk[4] = {0, 1, 2, 3};
    check(kui_font_add(ui, junk, sizeof junk) == 0, "garbage is not a font");
    check(kui_font_load_file(ui, KUI_STR("/nonexistent.ttf")) == 0, "missing font file");
    check(kui_font_load_dir(ui, KUI_STR("/nonexistent")) == 0, "missing font dir");
    kui_font_remove(ui, kui_font_add_system(ui, KUI_STR("Menlo"))); /* 0 is a no-op */

    const uint8_t rgba[2 * 2 * 4] = {
        255, 0, 0, 255,   0, 255, 0, 255,
        0, 0, 255, 255,   255, 255, 255, 255,
    };
    Keys k = {0};
    k.image = kui_image_add(ui, 2, 2, rgba);
    check(k.image != 0, "kui_image_add");
    k.sound = kui_sound_add(ui, junk, sizeof junk);
    check(k.sound != 0, "kui_sound_add");

    /* Two frames on a clock: the second leaves the keyframes mid-flight. */
    kui_set_time(ui, 0.0);
    kui_frame_begin(ui, 800, 600, 2.0f);
    surface_view(&k, ui);
    kui_frame_finish(ui);
    kui_set_time(ui, 0.05);
    kui_frame_begin(ui, 800, 600, 2.0f);
    surface_view(&k, ui);
    kui_frame_finish(ui);

    check(k.card && k.slider && k.sink && k.editor && k.input && k.drag && k.child,
          "every node got a key");
    check(kui_animating(ui), "the keyframes keep animating");
    KuiStr title = {0};
    check(kui_window_title_get(ui, &title) && has(title, "surface"), "kui_window_title_get");

    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ui, &dd);
    check(dd.quad_count > 0 && dd.scale == 2.0f, "the surface frame drew at scale 2");
    check(dd.atlas_size > 0 && dd.atlas_pixels != NULL, "the glyph atlas is there");
    size_t images = 0;
    for (size_t i = 0; i < dd.quad_count; i++) {
        if (dd.quads[i].kind == KUI_QUAD_IMAGE) images++;
    }
    check(images > 0, "the image node drew");

    /* Pointer state and scrolling. */
    kui_input_cursor(ui, 400, 300);
    kui_input_scroll(ui, 0, -3);
    kui_input_cursor_left(ui);
    kui_input_modifiers(ui, KUI_KMOD_SHIFT | KUI_KMOD_CTRL);

    /* Keyboard focus: Tab walks the ring, and the ring is visible. */
    kui_focus(ui, 0);
    check(kui_focused(ui) == 0, "kui_focus(0) blurs");
    kui_focus_next(ui, true);
    uint64_t first = kui_focused(ui);
    check(first != 0 && kui_focus_visible(ui), "Tab focuses something, visibly");
    kui_focus_next(ui, true);
    check(kui_focused(ui) != first, "Tab moves on");
    kui_focus_next(ui, false);
    check(kui_focused(ui) == first, "Shift-Tab comes back");

    /* The key sink is focusable and takes focus like anything else, and a
     * host drives its raw keys directly: kui_input_key carries the editing
     * keys (KUI_KEY_*), kui_input_key_down / _up the whole keyboard. A held
     * key on a sink with key_up set is one {kind="key"} payload twice,
     * phase="down" then "up". */
    /* A node the host never got an event from is named by the label it was
     * opened under: kui_key_of walks the path from the root for it. */
    check(kui_key_of(ui, KUI_STR("sink")) == k.sink, "kui_key_of resolves a declared label");
    check(kui_key_of(ui, KUI_STR("no such node")) == 0, "kui_key_of is 0 for an undeclared one");
    kui_focus(ui, kui_key_of(ui, KUI_STR("sink")));
    check(kui_is_focused(ui, k.sink), "kui_is_focused");
    KuiStr no_text = {0};
    /* {0} for `physical` means "the key I just named" - what a host that
     * does not track physical positions passes. One that does hands both
     * over, and a non-Latin layout then still reports a Latin `code`. */
    KuiStr same_key = {0};
    kui_input_key_down(ui, KUI_STR("w"), same_key, 0, no_text, false);
    kui_input_key_down(ui, KUI_STR("w"), same_key, 0, no_text,
                       true); /* OS auto-repeat */
    kui_input_key_up(ui, KUI_STR("w"), same_key, 0);
    /* A Russian layout puts "ц" on the key US-QWERTY prints W on. The host
     * passes both, and the sink still hears code="w" - with physical="w"
     * beside it, so a keymap can bind either. */
    kui_input_key_down(ui, KUI_STR("ц"), KUI_STR("w"), 0, no_text, false);
    kui_input_key_up(ui, KUI_STR("ц"), KUI_STR("w"), 0);
    /* Held over a focus change: the sink hears the release anyway, so a
     * WASD binding cannot be left walking. kui_release_held_keys is the
     * same thing for a window that lost the keyboard. */
    kui_input_key_down(ui, KUI_STR("a"), same_key, KUI_KMOD_CTRL, no_text,
                       false);
    kui_release_held_keys(ui);
    kui_focus(ui, k.sink);
    kui_input_key(ui, KUI_KEY_RIGHT, 0);
    kui_input_key(ui, KUI_KEY_TAB, 0);

    /* Editors: type, compose, select, read back. */
    kui_focus(ui, k.editor);
    kui_input_text(ui, KUI_STR("two "));
    kui_input_preedit(ui, KUI_STR("mid"), 0, 3);
    kui_input_preedit(ui, KUI_STR(""), 0, 0);
    kui_input_key(ui, KUI_KEY_END, KUI_MOD_DOC);
    kui_input_key(ui, KUI_KEY_LEFT, KUI_MOD_SHIFT | KUI_MOD_WORD);
    KuiStr text = {0};
    check(kui_edit_text(ui, k.editor, &text), "kui_edit_text");
    check(has(text, "two ") && has(text, "line one"), "the typing landed");
    kui_edit_set_text(ui, k.editor, KUI_STR("replaced"));
    check(kui_edit_text(ui, k.editor, &text) && has(text, "replaced"), "kui_edit_set_text");
    check(kui_edit_text(ui, k.input, &text) && text.len == 3, "the text input kept \"ada\"");

    /* A third frame: re-lays out the edited text, and picks up the focus the
     * view declares (kui_set_key_focus is edge-triggered, so it lands once). */
    k.claim_focus = true;
    kui_set_time(ui, 0.2);
    kui_frame_begin(ui, 800, 600, 2.0f);
    surface_view(&k, ui);
    kui_frame_finish(ui);
    check(kui_focused(ui) == k.editor, "the view declared the focus");

    /* Accessibility: the tree, an editor's runs, and requests coming back. */
    KuiAccessNode nodes[64];
    size_t total = kui_access_tree(ui, nodes, 0);
    check(total > 1, "the access tree has nodes");
    size_t got = kui_access_tree(ui, nodes, sizeof nodes / sizeof nodes[0]);
    check(got > 0 && got <= total, "kui_access_tree fills the buffer");
    int sliders = 0, disabled = 0, editors = 0;
    size_t run_count = 0;
    for (size_t i = 0; i < got; i++) {
        const KuiAccessNode *n = &nodes[i];
        if (n->role == KUI_ROLE_SLIDER) {
            sliders++;
            check((n->flags & KUI_ACCESS_HAS_NUMBER) != 0, "the slider reports a value");
            check(n->value_now == 40 && n->value_max == 100, "and its range");
            check((n->actions & KUI_ACCESS_INCREMENT) != 0, "and accepts increments");
        }
        if (n->flags & KUI_ACCESS_DISABLED) disabled++;
        if (n->key == k.editor) {
            editors++;
            check(has(n->value, "replaced"), "the editor's text is in the tree");
            check((n->flags & KUI_ACCESS_FOCUSED) != 0, "and it holds focus");
            KuiAccessRun runs[16];
            run_count = kui_access_runs(ui, n->key, runs, sizeof runs / sizeof runs[0]);
            check(run_count == n->run_count, "kui_access_runs matches run_count");
            if (run_count > 0) {
                check(runs[0].text.len > 0 && runs[0].char_count > 0, "a run has characters");
                check(runs[0].char_positions && runs[0].char_widths && runs[0].char_lengths,
                      "and per-character metrics");
                kui_input_access_text(ui, n->key, KUI_ACCESS_SET_TEXT_SELECTION,
                                      runs[0].key, 0, runs[0].key, 1, (KuiStr){NULL, 0});
            }
        }
    }
    check(sliders == 1, "one slider in the tree");
    check(disabled == 1, "the disabled switch says so");
    check(editors == 1, "the multiline editor is in the tree");
    kui_input_access(ui, k.slider, KUI_ACCESS_INCREMENT, (KuiStr){NULL, 0});
    kui_input_access(ui, k.card, KUI_ACCESS_FOCUS, (KuiStr){NULL, 0});

    /* Audio without a device: commands queue for the host to apply. */
    KuiPlay opts = KUI_PLAY_INIT;
    opts.fade_in_ms = 10;
    uint64_t playback = kui_play(ui, k.sound, &opts, kui_value_str(KUI_STR("beep")));
    check(playback != 0, "kui_play");
    kui_set_volume(ui, playback, 0.5f, 20);
    kui_pause(ui, playback, 5);
    kui_resume(ui, playback, 5);
    kui_set_master_volume(ui, 0.8f, 0);
    kui_stop(ui, playback, 0);
    KuiAudioCommand cmds[16];
    size_t cmd_count = kui_take_audio_commands(ui, cmds, sizeof cmds / sizeof cmds[0]);
    check(cmd_count >= 6 && cmds[0].kind == KUI_AUDIO_PLAY, "the audio commands queued");
    check(kui_take_audio_commands(ui, cmds, 1) == 0, "and drained");
    kui_audio_ended(ui, playback);
    kui_sound_remove(ui, k.sound);

    /* Window chrome turns clicks into commands rather than events. */
    {
        KuiWindowCommand cmd = KUI_WINDOW_COMMAND_INIT;
        while (kui_take_window_command(ui, &cmd)) {}
    }

    /* Size and focus are the app asking, rather than chrome or the declared
     * set: queued the same way, drained in order and once, and carrying the
     * window they name. A headless host is free to read them and do nothing,
     * which is what this one does. */
    {
        kui_set_window_size(ui, KUI_WINDOW_MAIN, 640, 480);
        kui_focus_window(ui, KUI_WINDOW_MAIN);
        KuiWindowCommand cmd = KUI_WINDOW_COMMAND_INIT;
        check(kui_take_window_command(ui, &cmd) && cmd.kind == KUI_CMD_SET_SIZE
                  && cmd.window == KUI_WINDOW_MAIN && cmd.width == 640 && cmd.height == 480,
              "a size request drains with the size it asked for");
        check(kui_take_window_command(ui, &cmd) && cmd.kind == KUI_CMD_FOCUS
                  && cmd.window == KUI_WINDOW_MAIN,
              "and the focus request behind it");
        check(!kui_take_window_command(ui, &cmd), "both drained once");
    }

    /* What the size handshake buys, standing in for a host that predates
     * it: a reservation the library cannot recognise is refused outright
     * instead of being written into, and the refusal costs nothing - the
     * event is still queued for the caller that asks properly, below. */
    {
        KuiEvent stale = KUI_EVENT_INIT;
        stale.size = 4; /* what an un-set or pre-handshake `size` looks like */
        stale.key = 0xabcd;
        check(!kui_poll_event(ui, &stale), "a short reservation is refused");
        check(stale.key == 0xabcd, "and nothing was written into it");

        KuiDrawData shortdd = KUI_DRAW_DATA_INIT;
        shortdd.size = 1;
        check(!kui_draw_data(ui, &shortdd), "kui_draw_data refuses it too");
    }

    /* And the other half of the same handshake, which is what it was built
     * for: a host that predates an appended field keeps working. `window`
     * is ABI 4's append, so offsetof(KuiEvent, window) is exactly the size
     * an ABI 3 host reserved - it gets every field it knows, and the
     * library stops before the one it does not. (An ABI 3 host had no
     * `window` member at all; we keep ours, set to a sentinel, because
     * proving it was left alone is the whole point.)
     *
     * This pops a real event, so it names which one: the tallies below
     * count what is left, and a queue that reordered should fail here
     * rather than as a count that no longer adds up. */
    {
        KuiEvent old_host = KUI_EVENT_INIT;
        old_host.size = (uint32_t)offsetof(KuiEvent, window);
        old_host.window = 0xdead;
        check(kui_poll_event(ui, &old_host), "an ABI 3 reservation still polls");
        check(old_host.size == (uint32_t)offsetof(KuiEvent, window),
              "`size` comes back as the prefix that was filled");
        check(old_host.window == 0xdead, "nothing was written past that prefix");
        KuiStr kind;
        const KuiValue *k = old_host.payload
            ? kui_value_get(old_host.payload, KUI_STR("kind")) : NULL;
        check(k && kui_value_as_str(k, &kind) && kind.len == 6
                  && memcmp(kind.ptr, "layout", 6) == 0,
              "the fields it knows are filled, and this is the first on_layout");
    }

    /* Everything above lands as data. */
    KuiEvent ev = KUI_EVENT_INIT;
    int events = 0, layouts = 0, access = 0, downs = 0, ups = 0;
    int latin = 0, physical = 0;
    while (kui_poll_event(ui, &ev)) {
        events++;
        check(ev.size == sizeof ev, "a current host is filled all the way");
        check(ev.window == KUI_WINDOW_MAIN, "one window, so every event is from it");
        if (!ev.payload) continue;
        const KuiValue *kind = kui_value_get(ev.payload, KUI_STR("kind"));
        KuiStr s;
        if (!kind || !kui_value_as_str(kind, &s)) continue;
        if (s.len == 6 && memcmp(s.ptr, "layout", 6) == 0) layouts++;
        if (s.len == 6 && memcmp(s.ptr, "access", 6) == 0) access++;
        if (s.len == 3 && memcmp(s.ptr, "key", 3) == 0) {
            KuiStr phase;
            const KuiValue *p = kui_value_get(ev.payload, KUI_STR("phase"));
            if (!p || !kui_value_as_str(p, &phase)) continue;
            if (phase.len == 4 && memcmp(phase.ptr, "down", 4) == 0) downs++;
            if (phase.len == 2 && memcmp(phase.ptr, "up", 2) == 0) ups++;
            /* The Cyrillic press folded to its position's Latin letter, so
             * a keymap written in ASCII matches every key event here - and
             * `physical` rode along beside it. */
            KuiStr c;
            const KuiValue *code = kui_value_get(ev.payload, KUI_STR("code"));
            if (code && kui_value_as_str(code, &c) && c.len == 1
                && c.ptr[0] < 0x80)
                latin++;
            if (kui_value_get(ev.payload, KUI_STR("physical"))) physical++;
        }
    }
    check(events > 0, "the inputs produced events");
    check(layouts > 0, "on_layout reported the card's rect");
    check(access > 0, "the slider nudge arrived as an access event");
    /* Four presses (w, its repeat, the Cyrillic w, ctrl-a) and a release for
     * each of the three distinct holds — the last synthesized by letting go. */
    check(downs == 4, "the sink took the presses, repeat included");
    check(ups == 3, "every held key came back up exactly once");
    check(latin == downs + ups,
          "every code is a Latin key, the Cyrillic press included");
    check(physical == downs + ups, "and every one carries its position");

    /* Values round-trip, including the ones the counter never builds. */
    KuiValue *map = kui_value_map();
    kui_value_map_set(map, KUI_STR("n"), kui_value_int(-7));
    kui_value_map_set(map, KUI_STR("x"), kui_value_float(1.5));
    kui_value_map_set(map, KUI_STR("on"), kui_value_bool(true));
    kui_value_map_set(map, KUI_STR("nil"), kui_value_null());
    int64_t n = 0;
    check(kui_value_as_int(kui_value_get(map, KUI_STR("n")), &n) && n == -7, "kui_value_as_int");
    check(kui_value_get(map, KUI_STR("missing")) == NULL, "a missing key is NULL");
    kui_value_free(map);

    /* Diagnostics were on the whole way: a clean tree raises none. */
    KuiWarning warnings[8];
    size_t warned = kui_take_warnings(ui, warnings, sizeof warnings / sizeof warnings[0]);
    for (size_t i = 0; i < warned; i++) {
        fprintf(stderr, "  warning: %.*s: %.*s\n",
                (int)warnings[i].code.len, (const char *)warnings[i].code.ptr,
                (int)warnings[i].message.len, (const char *)warnings[i].message.ptr);
    }
    check(warned == 0, "the surface view raises no diagnostics");

    /* Declared windows (docs/adr/0004-multi-window.md): a frame that names
     * one gets a KUI_CMD_OPEN back from the same drain the chrome commands
     * use, carrying the id its events will say and the config it was
     * declared with; the app hears about it as data. Reporting the OS
     * close keeps it closed while it is still declared - a declaration
     * reopens a window only when it starts - and the drain's size
     * handshake refuses a reservation it cannot fill, leaving the command
     * queued, exactly as kui_poll_event does. */
    {
        check(kui_ctx_window(ui) == KUI_WINDOW_MAIN, "this context draws the main window");
        KuiStr name;
        check(kui_ctx_window_name(ui, &name) && name.len == 4 && memcmp(name.ptr, "main", 4) == 0,
              "and is named for it");
        KuiWindowConfig cfg = KUI_WINDOW_CONFIG_INIT;
        cfg.width = 400;
        cfg.height = 300;
        kui_frame_begin(ui, 320, 240, 1);
        kui_window_declare(ui, KUI_STR("palette"), &cfg);
        kui_frame_finish(ui);
        KuiWindowCommand shortcmd = KUI_WINDOW_COMMAND_INIT;
        shortcmd.size = 4;
        shortcmd.kind = 0xdead;
        check(!kui_take_window_command(ui, &shortcmd) && shortcmd.kind == 0xdead,
              "a short command reservation is refused and left untouched");
        KuiWindowCommand cmd = KUI_WINDOW_COMMAND_INIT;
        check(kui_take_window_command(ui, &cmd) && cmd.kind == KUI_CMD_OPEN && cmd.window == 1
                  && cmd.origin == 0 && cmd.config.width == 400 && cmd.config.height == 300
                  && cmd.config.activates == 1,
              "the declared window opens with its config, and the refusal kept it queued");
        check(!kui_take_window_command(ui, &cmd), "one command for one window");
        int opened = 0, closed = 0;
        KuiEvent wev = KUI_EVENT_INIT;
        while (kui_poll_event(ui, &wev)) {
            const KuiValue *kind = wev.payload ? kui_value_get(wev.payload, KUI_STR("kind")) : NULL;
            const KuiValue *phase = wev.payload ? kui_value_get(wev.payload, KUI_STR("phase")) : NULL;
            KuiStr ks, ps;
            if (!kind || !kui_value_as_str(kind, &ks) || ks.len != 6 || memcmp(ks.ptr, "window", 6)) continue;
            if (!phase || !kui_value_as_str(phase, &ps)) continue;
            if (ps.len == 6 && memcmp(ps.ptr, "opened", 6) == 0) opened++;
            if (ps.len == 6 && memcmp(ps.ptr, "closed", 6) == 0) closed++;
        }
        check(opened == 1 && closed == 0, "the app hears the window open");
        kui_window_closed(ui, 1);
        kui_frame_begin(ui, 320, 240, 1);
        kui_window_declare(ui, KUI_STR("palette"), &cfg);
        kui_frame_finish(ui);
        check(!kui_take_window_command(ui, &cmd), "a closed window still declared does not reopen");
        while (kui_poll_event(ui, &wev)) {
            const KuiValue *phase = wev.payload ? kui_value_get(wev.payload, KUI_STR("phase")) : NULL;
            KuiStr ps;
            if (phase && kui_value_as_str(phase, &ps) && ps.len == 6 && memcmp(ps.ptr, "closed", 6) == 0) closed++;
        }
        check(closed == 1, "and hears it close");
        size_t nw = kui_take_warnings(ui, warnings, sizeof warnings / sizeof warnings[0]);
        int trapped = 0;
        for (size_t i = 0; i < nw; i++) {
            if (warnings[i].code.len == 28
                && memcmp(warnings[i].code.ptr, "window-declared-while-closed", 28) == 0) trapped++;
        }
        check(trapped == 1, "the still-declared window is a named diagnostic");
        /* Lapse, then declare again: a new window, with a new id. */
        kui_frame_begin(ui, 320, 240, 1);
        kui_frame_finish(ui);
        kui_frame_begin(ui, 320, 240, 1);
        kui_window_declare(ui, KUI_STR("palette"), NULL);
        kui_frame_finish(ui);
        check(kui_take_window_command(ui, &cmd) && cmd.kind == KUI_CMD_OPEN && cmd.window == 2
                  && cmd.config.width == 640,
              "a declaration that starts again opens anew, at the defaults");
        while (kui_poll_event(ui, &wev)) {}
        kui_frame_begin(ui, 320, 240, 1);
        kui_frame_finish(ui);
        check(kui_take_window_command(ui, &cmd) && cmd.kind == KUI_CMD_CLOSE && cmd.window == 2,
              "and closes when it stops");
        while (kui_poll_event(ui, &wev)) {}
    }

    kui_image_remove(ui, k.image);
    kui_ctx_free(ui);

    printf("surface: %zu access nodes, %zu editor runs, %zu quads, %d events\n",
           total, run_count, dd.quad_count, events);
    if (fails) {
        fprintf(stderr, "surface self-test: %d check(s) failed\n", fails);
        return 1;
    }
    printf("surface self-test OK\n");
    return 0;
}

/* -- conformance: the shared scene corpus -------------------------------
 *
 * The self-tests above prove the C API works. They do not prove it agrees
 * with the other bindings, and `CUSTOM` in crates/kui-core/src/schema.rs
 * only ever claimed the four transports agree on a prop's *name*. The
 * corpus (crates/kui-core/src/conformance.rs) is the missing half: a list
 * of small named scenes, built natively by kui-core into a reference
 * report, and rebuilt here through the C API alone. The two reports have
 * to be the same bytes.
 *
 *     cargo run -p kui-core --features conformance --example conformance-dump -- target/conformance.txt
 *     ./examples/c/counter --conformance target/conformance.txt
 *
 * The report format is documented on `conformance::report`; it carries no
 * formatted floats (quad geometry travels as one FNV-1a digest) precisely
 * so C and Rust and JavaScript can print the same thing.
 */

/* A growable report buffer. Scenes are small; a failure to fit is a bug
 * here, not something to paper over, so appends past the end are fatal. */
typedef struct Rep {
    char *buf;
    size_t len, cap;
} Rep;

static void rep_init(Rep *r) {
    r->cap = 1 << 14;
    r->len = 0;
    r->buf = malloc(r->cap);
    if (!r->buf) abort();
    r->buf[0] = 0;
}

static void rep_free(Rep *r) { free(r->buf); }

static void repf(Rep *r, const char *fmt, ...) {
    for (;;) {
        va_list ap;
        va_start(ap, fmt);
        int n = vsnprintf(r->buf + r->len, r->cap - r->len, fmt, ap);
        va_end(ap);
        if (n < 0) abort();
        if ((size_t)n < r->cap - r->len) {
            r->len += (size_t)n;
            return;
        }
        r->cap *= 2;
        char *grown = realloc(r->buf, r->cap);
        if (!grown) abort();
        r->buf = grown;
    }
}

/* FNV-1a over each quad's words 0..18 and 23..30 - KuiQuad without its uv,
 * which follows glyph insertion order - plus the uv of a KUI_QUAD_SEGMENT,
 * where it is the endpoints. Mirrors conformance::quad_digest. */
_Static_assert(sizeof(KuiQuad) == 31 * sizeof(uint32_t), "KuiQuad is not 31 words");

static uint64_t quad_digest(const KuiQuad *quads, size_t count) {
    uint64_t h = 0xcbf29ce484222325ull;
    for (size_t i = 0; i < count; i++) {
        uint32_t w[31];
        memcpy(w, &quads[i], sizeof w);
        int segment = quads[i].kind == KUI_QUAD_SEGMENT;
        for (int j = 0; j < 31; j++) {
            if (j >= 19 && j <= 22 && !segment) continue; /* uv */
            uint32_t v = w[j];
            for (int b = 0; b < 4; b++) {
                h ^= (uint8_t)(v & 0xff);
                h *= 0x100000001b3ull;
                v >>= 8;
            }
        }
    }
    return h;
}

/* KUI_ROLE_* back to the camelCase spelling every binding prints
 * (kui_core::Role::name); index 0 is unused, the enum starts at 1. */
static const char *role_name(uint32_t role) {
    static const char *names[] = {
        "?", "none", "button", "checkbox", "radio", "switch", "slider", "tab",
        "tabList", "link", "heading", "list", "listItem", "image", "dialog",
        "group", "window", "titleBar", "staticText", "textInput",
        "multilineTextInput", "scrollView", "line",
        "radioGroup", "menu", "menuItem",
    };
    return role < sizeof names / sizeof *names ? names[role] : "?";
}

/* AccessAction names in bit order (kui_core::AccessAction::ALL). */
static const char *ACTION_NAMES[] = {
    "click", "focus", "blur", "setValue", "increment", "decrement",
    "scrollIntoView", "scrollUp", "scrollDown", "scrollLeft", "scrollRight",
    "setTextSelection", "replaceSelectedText",
};

/* The corpus fixtures, registered in the order conformance::fixtures uses
 * so the handles - and the atlas the image lands in - come out the same. */
typedef struct Fixtures {
    uint64_t image;
    uint64_t sound;
} Fixtures;

static Fixtures conf_fixtures(KuiCtx *ctx) {
    uint8_t rgba[4 * 4 * 4];
    memset(rgba, 0xff, sizeof rgba);
    Fixtures f;
    f.image = kui_image_add(ctx, 4, 4, rgba);
    f.sound = kui_sound_add(ctx, (const uint8_t *)"RIFF....WAVE", 12);
    return f;
}

/* -- the scenes, in C ---------------------------------------------------- */

static void conf_layout(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 8, .pad_r = 8, .pad_t = 8, .pad_b = 8, .gap = 6,
                     .bg = 0x14161eff};
    kui_open(ui, &outer, NULL);
    KuiSpec card = {
        .dir = KUI_ROW, .pad_l = 12, .pad_r = 10, .pad_t = 6, .pad_b = 4,
        .gap = 4, .bg = 0x202030ff, .border_w = 2, .border_color = 0x2a2d3aff,
        .radius = 5, .opacity_set = 1, .opacity = 0.75f,
        .shadow_color = 0x00000066, .shadow_blur = 8, .shadow_y = 3,
        .shadow_spread = 1,
        .width = {KUI_FIXED, 180}, .height = {KUI_FIXED, 40},
    };
    kui_open_keyed(ui, KUI_STR("card"), &card, NULL);
    KuiTextStyle s12 = {.size = 12};
    kui_text(ui, KUI_STR("ab"), &s12);
    kui_text(ui, KUI_STR("cd"), &s12);
    kui_close(ui);
    /* C has no shorthand to resolve — the struct is the four edges — so it
     * writes what PadShorthand{x:9, y:3, b:1} resolves to. */
    KuiSpec shorthand = {.pad_l = 9, .pad_r = 9, .pad_t = 3, .pad_b = 1,
                         .bg = 0x2a2d3aff};
    kui_open(ui, &shorthand, NULL);
    kui_close(ui);
    KuiSpan spans[] = {
        {KUI_STR("a "), 0, 0},
        {KUI_STR("b"), 0x73d98cff, KUI_SPAN_BOLD},
        {KUI_STR(" c"), 0, KUI_SPAN_ITALIC},
    };
    KuiTextStyle s13 = {.size = 13};
    kui_rich_text(ui, spans, 3, &s13);
    kui_close(ui);
}

/* conformance::WRAP_BOXES: 92px of content and a 6px gap put 30 + 40 on the
 * first line and 50 + 20 on the second. */
static void conf_wrap(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    static const float boxes[4][2] = {{30, 12}, {40, 16}, {50, 20}, {20, 24}};
    KuiSpec row = {
        .dir = KUI_ROW, .wrap_children = 1,
        .pad_l = 4, .pad_r = 4, .pad_t = 4, .pad_b = 4,
        .gap = 6, .cross_gap = 10,
        .width = {KUI_FIXED, 100}, .bg = 0x101018ff,
    };
    kui_open(ui, &row, NULL);
    for (int i = 0; i < 4; i++) {
        KuiSpec item = {.width = {KUI_FIXED, boxes[i][0]},
                        .height = {KUI_FIXED, boxes[i][1]},
                        .bg = 0x30344aff};
        kui_open(ui, &item, NULL);
        kui_close(ui);
    }
    kui_close(ui);
}

static void conf_overflow(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 4, .pad_r = 4, .pad_t = 4, .pad_b = 4,
                     .overflow = KUI_CLIP};
    kui_open(ui, &outer, NULL);
    KuiSpec list = {.width = {KUI_FIXED, 120}, .height = {KUI_FIXED, 60},
                    .gap = 4, .overflow = KUI_SCROLL_Y, .radius = 8,
                    .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR("list"), &list, NULL);
    KuiSpec item = {.width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 20},
                    .bg = 0x30344aff};
    for (int i = 0; i < 6; i++) {
        char key[8];
        snprintf(key, sizeof key, "i%d", i);
        kui_open_keyed(ui, KUI_STR(key), &item, NULL);
        kui_close(ui);
    }
    kui_close(ui);
    kui_close(ui);
}

/* The four sizing modes in a parent of known width, so each resolves to a
 * width no other mode gives: 30 fixed, 25% of 200 = 50, fit around a
 * 20-wide child, and grow taking the remaining 100. */
static void conf_sizing(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 14, .pad_r = 14, .pad_t = 6, .pad_b = 6};
    kui_open(ui, &outer, NULL);
    KuiSpec bar = {.dir = KUI_ROW, .width = {KUI_FIXED, 200},
                   .height = {KUI_FIXED, 40}, .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR("bar"), &bar, NULL);
    KuiSpec fixed = {.width = {KUI_FIXED, 30}, .height = {KUI_FIXED, 20},
                     .bg = 0x30344aff};
    kui_open(ui, &fixed, NULL);
    kui_close(ui);
    KuiSpec percent = {.width = {KUI_PERCENT, 0.25f}, .height = {KUI_FIXED, 20},
                       .bg = 0x3b5bd4ff};
    kui_open(ui, &percent, NULL);
    kui_close(ui);
    KuiSpec fit = {.width = {KUI_FIT, 0}, .height = {KUI_FIXED, 20},
                   .bg = 0x73d98cff};
    kui_open(ui, &fit, NULL);
    KuiSpec inner = {.width = {KUI_FIXED, 20}, .height = {KUI_FIXED, 10},
                     .bg = 0xff0000ff};
    kui_open(ui, &inner, NULL);
    kui_close(ui);
    kui_close(ui);
    KuiSpec grow = {.width = {KUI_GROW, 1}, .height = {KUI_FIXED, 20},
                    .bg = 0xffcc00ff};
    kui_open(ui, &grow, NULL);
    kui_close(ui);
    kui_close(ui);
    kui_close(ui);
}

static void conf_float(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 20, .pad_r = 20, .pad_t = 20, .pad_b = 20, .gap = 4};
    kui_open(ui, &outer, NULL);
    KuiSpec anchor = {.width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 24},
                      .bg = 0x333333ff};
    kui_open_keyed(ui, KUI_STR("anchor"), &anchor, NULL);
    /* The same "below" the JSX and Lua props name, resolved by the same
     * function in kui-core rather than spelled out here. */
    KuiSpec below = {
        .width = {KUI_FIXED, 40}, .height = {KUI_FIXED, 12}, .bg = 0xff0000ff,
    };
    kui_spec_float_preset(&below, KUI_STR("below"));
    kui_open(ui, &below, NULL);
    kui_close(ui);
    kui_close(ui);
    KuiSpec nudged_anchor = {.width = {KUI_FIXED, 60}, .height = {KUI_FIXED, 20},
                             .bg = 0x444444ff};
    kui_open_keyed(ui, KUI_STR("nudged"), &nudged_anchor, NULL);
    /* A preset as a starting point: dx moves it sideways, and the dy the
     * preset filled in stays, so the 6px gap survives. */
    KuiSpec nudged = {
        .width = {KUI_FIXED, 30}, .height = {KUI_FIXED, 10}, .bg = 0x0000ffff,
    };
    kui_spec_float_preset(&nudged, KUI_STR("below"));
    nudged.float_dx = 6;
    kui_open(ui, &nudged, NULL);
    kui_close(ui);
    kui_close(ui);
    /* Asymmetric in every axis, and attached at (-16, 254) so that
     * .float_fit has to clamp it back on screen. Symmetric values would
     * let a swapped at/self or dx/dy pair go unnoticed here. */
    KuiSpec corner = {
        .float_mode = KUI_FLOAT_VIEWPORT,
        .float_anchor_x = KUI_START, .float_anchor_y = KUI_END,
        .float_self_x = KUI_END, .float_self_y = KUI_START,
        .float_dx = -6, .float_dy = 14, .float_fit = 1,
        .width = {KUI_FIXED, 10}, .height = {KUI_FIXED, 10}, .bg = 0x00ff00ff,
    };
    kui_open(ui, &corner, NULL);
    kui_close(ui);
    kui_close(ui);
}

static void conf_tooltip(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10};
    kui_open(ui, &outer, NULL);
    /* KuiSpec.tooltip is the `tooltip` prop: hover tracking, the accessible
     * description, and the float kui_close hangs below while hovered. */
    KuiSpec tip = {.dir = KUI_ROW, .width = {KUI_FIXED, 100},
                   .height = {KUI_FIXED, 40}, .bg = 0x333333ff,
                   .role = KUI_ROLE_GROUP, .tooltip = KUI_STR("a hint")};
    kui_open_keyed(ui, KUI_STR("tip"), &tip, NULL);
    KuiTextStyle s12 = {.size = 12};
    kui_text(ui, KUI_STR("badge"), &s12);
    kui_close(ui);
    kui_close(ui);
}

static void conf_titlebar_body(void *user, KuiCtx *ui) {
    (void)user;
    KuiTextStyle s12 = {.size = 12};
    kui_text(ui, KUI_STR("app"), &s12);
}

static void conf_chrome(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    kui_window_title(ui, KUI_STR("kui conformance"));
    KuiSpec outer = {.gap = 6};
    kui_open(ui, &outer, NULL);
    /* kui_titlebar_with appends its own cluster after the body; the second
     * one is kui_window_buttons called directly, in a strip laid out here -
     * the "fully custom titlebar" the element exists for. */
    kui_titlebar_with(ui, conf_titlebar_body, NULL);
    KuiSpec strip = {.dir = KUI_ROW, .width = {KUI_GROW, 1}};
    kui_open(ui, &strip, NULL);
    kui_window_buttons(ui);
    kui_close(ui);
    KuiSpec sink = {.width = {KUI_FIXED, 40}, .height = {KUI_FIXED, 16},
                    .bg = 0x22242cff, .focusable = 1, .label = KUI_STR("Sink")};
    uint64_t key = kui_open_keyed(ui, KUI_STR("sink"), &sink, NULL);
    kui_set_key_focus(ui, key);
    kui_close(ui);
    kui_close(ui);
}

static void conf_controls(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiValue *menu = kui_value_map();
    kui_value_map_set(menu, KUI_STR("kind"), kui_value_str(KUI_STR("menu")));
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10, .gap = 6,
                     .on_context_menu = menu};
    kui_open(ui, &outer, NULL);
    KuiValue *go = kui_value_map();
    kui_value_map_set(go, KUI_STR("kind"), kui_value_str(KUI_STR("go")));
    kui_button(ui, KUI_STR("go"), go);
    KuiTextStyle s13 = {.size = 13};
    KuiSpec note = {.width = {KUI_FIXED, 160}, .label = KUI_STR("Note")};
    kui_text_edit(ui, KUI_STR("note"), KUI_STR("hello"), &s13, 0, &note);
    kui_close(ui);
    kui_value_free(menu);
}

static void conf_media(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)phase;
    KuiSpec outer = {.pad_l = 6, .pad_r = 6, .pad_t = 6, .pad_b = 6, .gap = 4};
    kui_open(ui, &outer, NULL);
    KuiSpec img = {.width = {KUI_FIXED, 16}, .radius = 2};
    kui_image(ui, f->image, &img);
    KuiAudio music = {.src = f->sound, .volume = 0.5f, .looped = 1};
    kui_audio(ui, KUI_STR("music"), &music, NULL);
    kui_latency_graph(ui);
    kui_close(ui);
}

/* docs/adr/0010-a-segment-primitive.md: three strokes and a box in a 200x120
 * canvas; the elbow's on_click is the one a line ignores. The curve is keyed
 * through kui_polyline's label; the other two are auto-keyed. */
static void conf_lines(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec canvas = {.width = {KUI_FIXED, 200}, .height = {KUI_FIXED, 120},
                      .bg = 0x14161eff};
    kui_open(ui, &canvas, NULL);
    kui_line(ui, 10, 10, 90, 70, 2, 0x7f9cf5ff, NULL);
    float elbow[] = {100, 20, 140, 20, 140, 60};
    /* The interaction a line ignores: kui_polyline takes no on_click, so the
     * C scene declares the same intent through the one input prop the spec
     * carries, and the warning is the same. */
    KuiSpec hover = {.hoverable = 1};
    kui_polyline(ui, KUI_STR(""), elbow, 3, 3, 0xd8863bff, false, &hover);
    float curve[] = {20, 100, 60, 80, 100, 110, 180, 90};
    KuiSpec faded = {.opacity_set = 1, .opacity = 0.5f};
    kui_polyline(ui, KUI_STR("curve"), curve, 4, 1.5f, 0x9ad9a0ff, true, &faded);
    KuiSpec box = {.width = {KUI_FIXED, 40}, .height = {KUI_FIXED, 20}, .bg = 0x202030ff};
    kui_open(ui, &box, NULL);
    kui_close(ui);
    kui_close(ui);
}

static void conf_modal_titlebar(void *user, KuiCtx *ui) {
    (void)user;
    KuiTextStyle s12 = {.size = 12};
    kui_text(ui, KUI_STR("app"), &s12);
}

/* One button in the dialog: `kind` is the click payload, `label` the
 * accessible name, and the pair of them is the whole Tab ring while the
 * modal is up. */
static void conf_modal_button(KuiCtx *ui, const char *key, const char *kind,
                              const char *label) {
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR(kind)));
    KuiSpec spec = {.dir = KUI_ROW, .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 24},
                    .bg = 0x3b5bd4ff, .label = KUI_STR(label)};
    kui_open_keyed(ui, KUI_STR(key), &spec, tag);
    kui_close(ui);
}

/* Two key sinks clicked into focus in turn: the first says only on_key and
 * hears the press alone, the second sets key_up and hears both halves. An
 * integer tag, so the report's event column shows the phase. */
static void conf_keys_sink(KuiCtx *ui, const char *name, uint32_t key_up) {
    KuiSpec spec = {.dir = KUI_ROW, .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 24},
                    .bg = 0x1b1d27ff, .role = KUI_ROLE_GROUP, .label = KUI_STR(name),
                    .key_up = key_up};
    kui_open_with(ui, KUI_STR(name), &spec, NULL, NULL, kui_value_int(1), NULL);
    kui_close(ui);
}

static void conf_keys(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.pad_l = 10, .pad_r = 10, .pad_t = 10, .pad_b = 10, .gap = 6};
    kui_open(ui, &outer, NULL);
    conf_keys_sink(ui, "press", 0);
    conf_keys_sink(ui, "held", 1);
    kui_close(ui);
}

/* A floated modal over an app with window chrome (see
 * docs/adr/0003-modal-surfaces.md). The column grows so the titlebar is a
 * full-width strip: the scene presses it to show that chrome stays live
 * under a modal. */
static void conf_modal(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    KuiSpec outer = {.gap = 6, .width = {KUI_GROW, 1}};
    kui_open(ui, &outer, NULL);
    kui_titlebar_with(ui, conf_modal_titlebar, NULL);

    KuiValue *open_tag = kui_value_map();
    kui_value_map_set(open_tag, KUI_STR("kind"), kui_value_str(KUI_STR("open")));
    KuiSpec open = {.dir = KUI_ROW, .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 20},
                    .bg = 0x30344aff, .label = KUI_STR("Open")};
    uint64_t open_key = kui_open_keyed(ui, KUI_STR("open"), &open, open_tag);
    kui_close(ui);
    /* The app owns its keyboard while the dialog is shut and says so every
     * frame - an edge once, and no clobber after. */
    if (phase == 0) kui_set_key_focus(ui, open_key);

    /* Phase 1 drops the dialog and declares the node it was renaming
     * focused instead: that change of declaration is an edge, and an edge
     * on the closing frame beats the focus the modal displaced (see
     * docs/adr/0003-modal-surfaces.md, decision 4). */
    if (phase != 0) {
        KuiValue *note_tag = kui_value_map();
        kui_value_map_set(note_tag, KUI_STR("kind"), kui_value_str(KUI_STR("note")));
        KuiSpec note = {.dir = KUI_ROW, .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 20},
                        .bg = 0x30344aff, .label = KUI_STR("Note")};
        uint64_t note_key = kui_open_keyed(ui, KUI_STR("note"), &note, note_tag);
        kui_close(ui);
        kui_set_key_focus(ui, note_key);
        kui_close(ui);
        return;
    }

    /* KuiSpec.modal is the `modal` prop, borrowed for the open call. */
    KuiValue *modal = kui_value_map();
    kui_value_map_set(modal, KUI_STR("kind"), kui_value_str(KUI_STR("dlg")));
    KuiSpec dialog = {
        .width = {KUI_FIXED, 120}, .height = {KUI_FIXED, 100},
        .pad_l = 8, .pad_r = 8, .pad_t = 8, .pad_b = 8, .gap = 6,
        .bg = 0x202030ff,
        .float_mode = KUI_FLOAT_VIEWPORT,
        .float_anchor_x = KUI_END, .float_anchor_y = KUI_END,
        .float_self_x = KUI_END, .float_self_y = KUI_END,
        .modal = modal, .label = KUI_STR("Settings"),
    };
    kui_open_keyed(ui, KUI_STR("dialog"), &dialog, NULL);
    conf_modal_button(ui, "ok", "ok", "OK");
    conf_modal_button(ui, "cancel", "cancel", "Cancel");
    kui_close(ui);

    kui_close(ui);
    kui_value_free(modal);
}

/* conformance::EXIT_BULK_ROWS: with its own root that is one node past
 * kui_core::depart::MAX_NODES, so the whole subtree is refused. */
#define CONF_EXIT_BULK_ROWS 512

/* One of the exit scene's fixed-size slots: dropping the node inside it
 * moves nothing else, so the only geometry that changes between phases is
 * the ghosts'. */
static void conf_exit_slot(KuiCtx *ui, const char *key, float h) {
    KuiSpec slot = {.width = {KUI_FIXED, 140}, .height = {KUI_FIXED, h},
                    .bg = 0x101018ff};
    kui_open_keyed(ui, KUI_STR(key), &slot, NULL);
}

/* A live Tab stop either side of the departing ones, so two Tabs at the end
 * say whether the ring has a place for a ghost. */
static void conf_exit_keep(KuiCtx *ui, const char *key, const char *label) {
    KuiSpec keep = {.dir = KUI_ROW, .width = {KUI_FIXED, 60}, .height = {KUI_FIXED, 16},
                    .bg = 0x22242cff, .focusable = 1, .label = KUI_STR(label)};
    kui_open_keyed(ui, KUI_STR(key), &keep, NULL);
    kui_close(ui);
}

/* Exit transitions (docs/adr/0005-the-paint-vocabulary.md). Four subtrees
 * the view stops declaring in phase 1 - one still in flight at the end, one
 * already over, one that comes back in phase 2, and one past the budget -
 * and two that never leave. */
static void conf_exit(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    KuiSpec outer = {.width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
                     .pad_l = 8, .pad_r = 8, .pad_t = 8, .pad_b = 8,
                     .gap = 6, .bg = 0x14161eff};
    kui_open(ui, &outer, NULL);
    conf_exit_keep(ui, "a", "A");

    conf_exit_slot(ui, "slotFade", 40);
    if (phase == 0) {
        /* Consumed by kui_open_keyed, like every other on_click payload. */
        KuiValue *hit = kui_value_map();
        kui_value_map_set(hit, KUI_STR("kind"), kui_value_str(KUI_STR("hit")));
        /* Focusable, clickable and labelled while it is live, so each of
         * those is a separate thing the ghost has to stop being. */
        KuiSpec fade = {
            .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 24}, .bg = 0x3b5bd4ff,
            .transition_ms = 400,
            .exit = {.set = KUI_ENTER_OFFSET | KUI_ENTER_OPACITY, .dx = 40, .opacity = 0},
            .focusable = 1, .label = KUI_STR("Fade"),
        };
        kui_open_keyed(ui, KUI_STR("fade"), &fade, hit);
        KuiTextStyle s12 = {.size = 12};
        kui_text(ui, KUI_STR("bye"), &s12);
        kui_close(ui);
    }
    kui_close(ui);

    conf_exit_slot(ui, "slotBlink", 16);
    if (phase == 0) {
        KuiSpec blink = {
            .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 12}, .bg = 0x73d98cff,
            .transition_ms = 50, .exit = {.set = KUI_ENTER_OFFSET, .dx = 20},
        };
        kui_open_keyed(ui, KUI_STR("blink"), &blink, NULL);
        kui_close(ui);
    }
    kui_close(ui);

    conf_exit_slot(ui, "slotFlash", 16);
    if (phase != 1) {
        KuiSpec flash = {
            .width = {KUI_FIXED, 100}, .height = {KUI_FIXED, 12}, .bg = 0xffcc00ff,
            .transition_ms = 400, .exit = {.set = KUI_ENTER_OFFSET, .dx = -20},
        };
        kui_open_keyed(ui, KUI_STR("flash"), &flash, NULL);
        kui_close(ui);
    }
    kui_close(ui);

    conf_exit_keep(ui, "b", "B");

    /* Last, and sized by children that have no size: dropping it takes only
     * the trailing gap with it. */
    if (phase == 0) {
        KuiSpec bulk = {.transition_ms = 400,
                        .exit = {.set = KUI_ENTER_OPACITY, .opacity = 0}};
        kui_open_keyed(ui, KUI_STR("bulk"), &bulk, NULL);
        KuiSpec row = {0};
        for (int i = 0; i < CONF_EXIT_BULK_ROWS; i++) {
            kui_open(ui, &row, NULL);
            kui_close(ui);
        }
        kui_close(ui);
    }

    kui_close(ui);
}

/* One item of a composite: a click payload, its own text, and (for a row)
 * `focusable`, which is what makes the list around it a composite at all
 * rather than a navigation list of links. */
static void conf_composite_item(KuiCtx *ui, const char *name, const char *kind,
                                uint32_t role, int selected, int focusable,
                                float w, float h, uint32_t bg) {
    KuiValue *tag = kui_value_map();
    kui_value_map_set(tag, KUI_STR("kind"), kui_value_str(KUI_STR(kind)));
    KuiSpec spec = {.dir = KUI_ROW, .role = role, .selected = (uint32_t)selected,
                    .focusable = (uint32_t)focusable,
                    .width = {KUI_FIXED, w}, .height = {KUI_FIXED, h}, .bg = bg};
    kui_open_keyed(ui, KUI_STR(name), &spec, tag);
    KuiTextStyle s12 = {.size = 12};
    kui_text(ui, KUI_STR(name), &s12);
    kui_close(ui);
}

/* A tab bar and a picker list, each one Tab stop with the arrows moving
 * inside it (docs/adr/0007-composite-keyboard-patterns.md), and an
 * ordinary button between them that keeps a stop of its own. Nothing here
 * declares "composite": the core derives it from the roles and from which
 * nodes are focusable. */
static void conf_composite(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiSpec outer = {.gap = 6, .width = {KUI_GROW, 1}};
    kui_open(ui, &outer, NULL);

    KuiSpec tabs = {.dir = KUI_ROW, .role = KUI_ROLE_TAB_LIST, .gap = 4};
    kui_open_keyed(ui, KUI_STR("tabs"), &tabs, NULL);
    conf_composite_item(ui, "One", "one", KUI_ROLE_TAB, 0, 0, 60, 20, 0x30344aff);
    conf_composite_item(ui, "Two", "two", KUI_ROLE_TAB, 1, 0, 60, 20, 0x30344aff);
    conf_composite_item(ui, "Three", "three", KUI_ROLE_TAB, 0, 0, 60, 20, 0x30344aff);
    kui_close(ui);

    KuiValue *add_tag = kui_value_map();
    kui_value_map_set(add_tag, KUI_STR("kind"), kui_value_str(KUI_STR("add")));
    KuiSpec add = {.dir = KUI_ROW, .width = {KUI_FIXED, 40}, .height = {KUI_FIXED, 20},
                   .bg = 0x3b5bd4ff, .label = KUI_STR("Add")};
    kui_open_keyed(ui, KUI_STR("add"), &add, add_tag);
    kui_close(ui);

    KuiSpec rows = {.role = KUI_ROLE_LIST, .gap = 2};
    kui_open_keyed(ui, KUI_STR("rows"), &rows, NULL);
    conf_composite_item(ui, "Alpha", "alpha", KUI_ROLE_LIST_ITEM, 0, 1, 80, 18, 0x202030ff);
    conf_composite_item(ui, "Bravo", "bravo", KUI_ROLE_LIST_ITEM, 0, 1, 80, 18, 0x202030ff);
    kui_close(ui);

    kui_close(ui);
}

typedef struct ConfScene {
    const char *name;
    /* `phase` is what the "step phase N" lines leave behind: the view's own
     * mind, which only the exit scene ever changes. */
    void (*build)(KuiCtx *ui, const Fixtures *f, int phase);
} ConfScene;

/* conformance::build_windows: the declaration comes and goes with the
 * phase; the tree only says which. Phase 0 declares `palette` twice,
 * disagreeing about the size, so the first wins and the second warns. */
static void conf_windows(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    KuiWindowConfig cfg = KUI_WINDOW_CONFIG_INIT;
    cfg.width = 400;
    cfg.height = 300;
    if (phase == 0 || phase == 2) kui_window_declare(ui, KUI_STR("palette"), &cfg);
    if (phase == 0) {
        cfg.width = 500;
        cfg.height = 500;
        kui_window_declare(ui, KUI_STR("palette"), &cfg);
    }
    KuiSpec box = {0};
    box.pad_l = box.pad_r = box.pad_t = box.pad_b = 8;
    box.bg = 0x14161eff;
    kui_open(ui, &box, NULL);
    KuiTextStyle st = {0};
    st.size = 12;
    kui_text(ui, KUI_STR(phase == 0 || phase == 2 ? "open" : "closed"), &st);
    kui_close(ui);
}

/* conformance::build_popup: one declaration, with the kind and the anchor
 * a menu carries (ADR 0004 decision 9). The dismissals are steps, not
 * anything the tree says. */
static void conf_popup(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    if (phase == 0) {
        KuiWindowConfig cfg = KUI_WINDOW_POPUP_INIT;
        cfg.width = 160;
        cfg.height = 320;
        cfg.anchor_x = 12;
        cfg.anchor_y = 40;
        cfg.anchor_w = 160;
        cfg.anchor_h = 24;
        kui_window_declare(ui, KUI_STR("menu"), &cfg);
    }
    KuiSpec box = {0};
    box.pad_l = box.pad_r = box.pad_t = box.pad_b = 8;
    box.bg = 0x14161eff;
    kui_open(ui, &box, NULL);
    KuiTextStyle st = {0};
    st.size = 12;
    kui_text(ui, KUI_STR(phase == 0 ? "menu" : "closed"), &st);
    kui_close(ui);
}

/* conformance::build_live: the `live` prop on a box that would otherwise be
 * elided, and `kui_announce` for the half with no node behind it. A C host
 * holds the context, so the announce could sit in its event loop; here it
 * is in the builder because the corpus's phase is the only clock a scene
 * has, and phase 1 is built once. */
static void conf_live(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    if (phase == 1) kui_announce(ui, KUI_STR("Saved"), KUI_LIVE_ASSERTIVE);
    KuiSpec box = {0};
    box.pad_l = box.pad_r = box.pad_t = box.pad_b = 8;
    box.gap = 4;
    box.bg = 0x14161eff;
    kui_open(ui, &box, NULL);
    KuiSpec region = {0};
    region.live = KUI_LIVE_POLITE;
    kui_open_keyed(ui, KUI_STR("status"), &region, NULL);
    KuiTextStyle st = {0};
    st.size = 12;
    kui_text(ui, KUI_STR(phase == 0 ? "0 results" : "3 results"), &st);
    kui_close(ui);
    kui_open_keyed(ui, KUI_STR("empty"), &region, NULL);
    kui_close(ui);
    kui_close(ui);
}

/* conformance::build_drag: one keyed 80x40 handle whose drag deltas the
 * event rows carry, measured from the press point in every phase. */
static void conf_drag(KuiCtx *ui, const Fixtures *f, int phase) {
    (void)f;
    (void)phase;
    KuiValue *split = kui_value_map();
    kui_value_map_set(split, KUI_STR("kind"), kui_value_str(KUI_STR("split")));
    KuiSpec handle = {.width = {KUI_FIXED, 80}, .height = {KUI_FIXED, 40}, .bg = 0x30344aff};
    kui_open_draggable(ui, KUI_STR("handle"), &handle, NULL, split);
    kui_close(ui);
}

/* One entry per scene of conformance::SCENES; a scene in the reference with
 * no entry here fails the run rather than being skipped. */
static const ConfScene CONF_SCENES[] = {
    {"layout", conf_layout},
    {"sizing", conf_sizing},
    {"wrap", conf_wrap},
    {"overflow", conf_overflow},
    {"float", conf_float},
    {"tooltip", conf_tooltip},
    {"chrome", conf_chrome},
    /* Same builder: chrome-inset is the same tree under an env that also
     * reports the OS controls, so the two scenes differ only in the env. */
    {"chrome-inset", conf_chrome},
    {"controls", conf_controls},
    {"keys", conf_keys},
    {"media", conf_media},
    {"lines", conf_lines},
    {"modal", conf_modal},
    {"composite", conf_composite},
    {"exit", conf_exit},
    {"windows", conf_windows},
    {"popup", conf_popup},
    {"live", conf_live},
    {"drag", conf_drag},
};

/* -- driving one scene --------------------------------------------------- */

/* A replayed input, parsed back out of the reference report's step lines so
 * the scenes need not restate it. The cap is a limit on one scene's replay,
 * not on the corpus. */
#define CONF_MAX_STEPS 32
typedef struct ConfStep {
    char kind[16];
    int a, b;
    /* How many numbers followed the kind: "phase" and "time" carry one,
     * "cursor" and "scroll" two, the rest none. */
    int args;
} ConfStep;

/* The window facts a scene is driven under, parsed back out of its `env`
 * line the same way - the five arguments kui_env_set_window takes, in its
 * order (conformance::write_env). `set` is 0 for a scene that wrote no
 * line, which is one at the corpus defaults a fresh KuiCtx already has. */
typedef struct ConfEnv {
    int set;
    int custom_chrome, maximized, fullscreen, controls_w, controls_h;
} ConfEnv;

/* Drains the window commands into `cmds`, one line each, the way
 * conformance::write_command spells them. */
static void conf_drain_cmds(KuiCtx *ctx, Rep *cmds) {
    KuiWindowCommand c = KUI_WINDOW_COMMAND_INIT;
    while (kui_take_window_command(ctx, &c)) {
        switch (c.kind) {
        case KUI_CMD_START_DRAG: repf(cmds, "cmd drag %u\n", c.window); break;
        case KUI_CMD_CLOSE: repf(cmds, "cmd close %u\n", c.window); break;
        case KUI_CMD_MINIMIZE: repf(cmds, "cmd minimize %u\n", c.window); break;
        case KUI_CMD_TOGGLE_MAXIMIZE: repf(cmds, "cmd maximize %u\n", c.window); break;
        case KUI_CMD_OPEN:
            repf(cmds, "cmd open %u %u %u %u %d %d %u %d %d %d %d\n", c.window, c.owner,
                 (unsigned)c.origin, c.config.kind, (int)c.config.width,
                 (int)c.config.height, c.config.activates ? 1u : 0u,
                 (int)c.config.anchor_x, (int)c.config.anchor_y,
                 (int)c.config.anchor_w, (int)c.config.anchor_h);
            break;
        default: repf(cmds, "cmd ? %u\n", c.window); break;
        }
    }
}

static void conf_apply(KuiCtx *ctx, const ConfStep *s) {
    if (strcmp(s->kind, "cursor") == 0) kui_input_cursor(ctx, (float)s->a, (float)s->b);
    else if (strcmp(s->kind, "cursorleft") == 0) kui_input_cursor_left(ctx);
    else if (strcmp(s->kind, "mousedown") == 0) kui_input_mouse(ctx, true, 1);
    else if (strcmp(s->kind, "mouseup") == 0) kui_input_mouse(ctx, false, 1);
    else if (strcmp(s->kind, "secondarydown") == 0)
        kui_input_mouse_button(ctx, true, KUI_MOUSE_SECONDARY, 1);
    else if (strcmp(s->kind, "secondaryup") == 0)
        kui_input_mouse_button(ctx, false, KUI_MOUSE_SECONDARY, 1);
    else if (strcmp(s->kind, "scroll") == 0) kui_input_scroll(ctx, (float)s->a, (float)s->b);
    else if (strcmp(s->kind, "tab") == 0) kui_input_key(ctx, KUI_KEY_TAB, 0);
    else if (strcmp(s->kind, "shifttab") == 0) kui_input_key(ctx, KUI_KEY_TAB, KUI_MOD_SHIFT);
    else if (strcmp(s->kind, "escape") == 0) kui_input_key(ctx, KUI_KEY_ESCAPE, 0);
    /* conformance::ARROWS order: left, right, up, down - which is
     * KUI_KEY_LEFT..KUI_KEY_DOWN, so the index is the key. */
    else if (strcmp(s->kind, "arrow") == 0) kui_input_key(ctx, KUI_KEY_LEFT + (uint32_t)s->a, 0);
    else if (strcmp(s->kind, "home") == 0) kui_input_key(ctx, KUI_KEY_HOME, 0);
    else if (strcmp(s->kind, "end") == 0) kui_input_key(ctx, KUI_KEY_END, 0);
    /* A Unicode scalar value, so a step line carries only integers. The
     * corpus types ASCII, which is one UTF-8 byte. */
    else if (strcmp(s->kind, "type") == 0) {
        uint8_t c = (uint8_t)s->a;
        kui_input_text(ctx, (KuiStr){&c, 1});
    }
    /* The same spelling for a raw key on an on_key sink, down and up: no
     * position ({0} = the key named), no modifiers, no text, no repeat. */
    else if (strcmp(s->kind, "keydown") == 0) {
        uint8_t c = (uint8_t)s->a;
        KuiStr none = {0};
        kui_input_key_down(ctx, (KuiStr){&c, 1}, none, 0, none, false);
    }
    else if (strcmp(s->kind, "keyup") == 0) {
        uint8_t c = (uint8_t)s->a;
        KuiStr none = {0};
        kui_input_key_up(ctx, (KuiStr){&c, 1}, none, 0);
    }
    else {
        fprintf(stderr, "conformance: unknown step '%s'\n", s->kind);
        exit(1);
    }
}

/* Drains the queue into `events`; payloads are borrowed until the next
 * poll, so each is formatted before the next call. */
static void conf_drain(KuiCtx *ctx, Rep *events) {
    KuiEvent ev = KUI_EVENT_INIT;
    while (kui_poll_event(ctx, &ev)) {
        KuiStr kind = KUI_STR("-"), tag = KUI_STR("-");
        const KuiValue *k = ev.payload ? kui_value_get(ev.payload, KUI_STR("kind")) : NULL;
        if (k) kui_value_as_str(k, &kind);
        const KuiValue *t = ev.payload ? kui_value_get(ev.payload, KUI_STR("tag")) : NULL;
        const KuiValue *tk = t ? kui_value_get(t, KUI_STR("kind")) : NULL;
        /* The tag column, or - for the two window-level events, which have
         * none - the field that tells one from its siblings
         * (conformance::event_row). */
        if (!tk && ev.payload) {
            tk = kui_value_get(ev.payload, KUI_STR("phase"));
            if (!tk) tk = kui_value_get(ev.payload, KUI_STR("reason"));
        }
        if (tk) kui_value_as_str(tk, &tag);
        repf(events, "event %.*s %.*s", (int)kind.len, kind.ptr, (int)tag.len, tag.ptr);
        /* A drag's phase and deltas ride in the tag column: dx/dy are the
         * displacement from the press point in every phase, and the corpus
         * steps are integers, so they print exactly (kui_value_as_int
         * truncates a float the way the reference's cast does). */
        if (kind.len == 4 && memcmp(kind.ptr, "drag", 4) == 0) {
            KuiStr phase = KUI_STR("-");
            const KuiValue *p = kui_value_get(ev.payload, KUI_STR("phase"));
            if (p) kui_value_as_str(p, &phase);
            int64_t dx = 0, dy = 0;
            const KuiValue *vx = kui_value_get(ev.payload, KUI_STR("dx"));
            const KuiValue *vy = kui_value_get(ev.payload, KUI_STR("dy"));
            if (vx) kui_value_as_int(vx, &dx);
            if (vy) kui_value_as_int(vy, &dy);
            repf(events, " %.*s %lld %lld", (int)phase.len, phase.ptr, (long long)dx, (long long)dy);
        }
        repf(events, "\n");
    }
}

/* The protocol conformance::drive documents: the window facts, then a
 * frame, then each step followed by another frame, then the last frame's
 * output. The env goes in before the first frame because kui_titlebar_with
 * and kui_window_buttons read it while that frame builds. */
static void conf_run(const ConfScene *scene, const ConfEnv *env,
                     const ConfStep *steps, int nsteps, Rep *out) {
    KuiCtx *ctx = kui_ctx_new();
    kui_set_diagnostics(ctx, true);
    if (env->set) {
        kui_env_set_window(ctx, KUI_WINDOW_MAIN, env->custom_chrome != 0,
                           env->maximized != 0, env->fullscreen != 0,
                           (float)env->controls_w, (float)env->controls_h);
    }
    Fixtures f = conf_fixtures(ctx);

    Rep events, cmds;
    rep_init(&events);
    rep_init(&cmds);
    int phase = 0;
    for (int i = 0; i <= nsteps; i++) {
        if (i > 0) {
            /* Four steps are not input: one moves the clock the
             * transitions read, one is the view changing its mind, and two
             * are the OS closing a window and asking a popup to go away. */
            const ConfStep *s = &steps[i - 1];
            if (strcmp(s->kind, "phase") == 0) {
                phase = s->a;
            } else if (strcmp(s->kind, "time") == 0) {
                kui_set_time(ctx, s->a / 1000.0);
            } else if (strcmp(s->kind, "windowclosed") == 0) {
                kui_window_closed(ctx, (uint32_t)s->a);
                conf_drain(ctx, &events);
                conf_drain_cmds(ctx, &cmds);
            } else if (strcmp(s->kind, "windowdismissed") == 0) {
                kui_window_dismissed(ctx, (uint32_t)s->a, (uint32_t)s->b);
                conf_drain(ctx, &events);
            } else {
                conf_apply(ctx, s);
                conf_drain(ctx, &events);
                conf_drain_cmds(ctx, &cmds);
            }
        }
        kui_frame_begin(ctx, 320, 240, 1);
        scene->build(ctx, &f, phase);
        kui_frame_finish(ctx);
        conf_drain(ctx, &events);
        conf_drain_cmds(ctx, &cmds);
    }

    repf(out, "scene %s\n", scene->name);
    if (env->set) {
        repf(out, "env %d %d %d %d %d\n", env->custom_chrome, env->maximized,
             env->fullscreen, env->controls_w, env->controls_h);
    }
    for (int i = 0; i < nsteps; i++) {
        if (steps[i].args >= 2) repf(out, "step %s %d %d\n", steps[i].kind, steps[i].a, steps[i].b);
        else if (steps[i].args == 1) repf(out, "step %s %d\n", steps[i].kind, steps[i].a);
        else repf(out, "step %s\n", steps[i].kind);
    }

    KuiStr title;
    if (kui_window_title_get(ctx, &title)) repf(out, "title %.*s\n", (int)title.len, title.ptr);
    else repf(out, "title -\n");

    KuiDrawData dd = KUI_DRAW_DATA_INIT;
    kui_draw_data(ctx, &dd);
    repf(out, "quads %zu %016llx\n", dd.quad_count,
         (unsigned long long)quad_digest(dd.quads, dd.quad_count));
    size_t kinds[7] = {0};
    for (size_t i = 0; i < dd.quad_count; i++) {
        if (dd.quads[i].kind < 7) kinds[dd.quads[i].kind]++;
    }
    repf(out, "kinds %zu %zu %zu %zu %zu %zu %zu\n", kinds[0], kinds[1], kinds[2],
         kinds[3], kinds[4], kinds[5], kinds[6]);

    KuiAccessNode nodes[128];
    size_t total = kui_access_tree(ctx, nodes, 128);
    if (total > 128) {
        fprintf(stderr, "conformance: scene '%s' has %zu access nodes, buffer holds 128\n",
                scene->name, total);
        exit(1);
    }
    size_t n = total;
    /* Depth from the parent chain: the tree comes back in tree order, so a
     * node's parent is always already in the table. */
    uint64_t keys[128];
    int depths[128];
    for (size_t i = 0; i < n; i++) {
        const KuiAccessNode *a = &nodes[i];
        int depth = 0;
        for (size_t j = 0; j < i; j++) {
            if (keys[j] == a->parent) { depth = depths[j] + 1; break; }
        }
        keys[i] = a->key;
        depths[i] = depth;
        char actions[256];
        size_t off = 0;
        for (int bit = 0; bit < 13; bit++) {
            if (!(a->actions & (1u << bit))) continue;
            off += (size_t)snprintf(actions + off, sizeof actions - off, "%s%s",
                                    off ? "," : "", ACTION_NAMES[bit]);
        }
        const char *checked = !(a->flags & KUI_ACCESS_CHECKED_SET) ? "-"
                              : (a->flags & KUI_ACCESS_CHECKED)    ? "1"
                                                                   : "0";
        const char *selected = !(a->flags & KUI_ACCESS_SELECTED_SET) ? "-"
                               : (a->flags & KUI_ACCESS_SELECTED)    ? "1"
                                                                     : "0";
        const char *orientation = a->orientation == KUI_ORIENTATION_HORIZONTAL ? "h"
                                  : a->orientation == KUI_ORIENTATION_VERTICAL ? "v"
                                                                               : "-";
        const char *live = (a->flags & KUI_ACCESS_LIVE_POLITE)      ? "p"
                           : (a->flags & KUI_ACCESS_LIVE_ASSERTIVE) ? "a"
                                                                    : "-";
        repf(out, "node %d %016llx %s %d %d %s %s %s %s %d %s %.*s | %.*s | %.*s\n",
             depth, (unsigned long long)a->key, role_name(a->role),
             (a->flags & KUI_ACCESS_FOCUSED) ? 1 : 0,
             (a->flags & KUI_ACCESS_DISABLED) ? 1 : 0,
             checked, selected, orientation, live,
             (a->flags & KUI_ACCESS_HAS_SCROLL) ? 1 : 0,
             off ? actions : "-",
             (int)a->name.len, a->name.ptr,
             (int)a->description.len, a->description.ptr,
             (int)a->value.len, a->value.ptr);
    }

    repf(out, "%s", events.buf);
    rep_free(&events);
    repf(out, "%s", cmds.buf);
    rep_free(&cmds);

    KuiAnnouncement said[16];
    size_t na = kui_take_announcements(ctx, said, 16);
    for (size_t i = 0; i < na && i < 16; i++) {
        repf(out, "announce %s %.*s\n",
             said[i].live == KUI_LIVE_ASSERTIVE ? "assertive" : "polite",
             (int)said[i].text.len, said[i].text.ptr);
    }

    KuiWarning warnings[32];
    size_t nw = kui_take_warnings(ctx, warnings, 32);
    for (size_t i = 0; i < nw && i < 32; i++) {
        repf(out, "warn %.*s\n", (int)warnings[i].code.len, warnings[i].code.ptr);
    }
    repf(out, "end\n");
    kui_ctx_free(ctx);
}

/* -- reading the reference ----------------------------------------------- */

static char *read_file(const char *path) {
    FILE *fp = fopen(path, "rb");
    if (!fp) return NULL;
    fseek(fp, 0, SEEK_END);
    long size = ftell(fp);
    rewind(fp);
    char *buf = malloc((size_t)size + 1);
    if (!buf || fread(buf, 1, (size_t)size, fp) != (size_t)size) {
        free(buf);
        fclose(fp);
        return NULL;
    }
    buf[size] = 0;
    fclose(fp);
    return buf;
}

static const ConfScene *conf_find(const char *name) {
    for (size_t i = 0; i < sizeof CONF_SCENES / sizeof *CONF_SCENES; i++) {
        if (strcmp(CONF_SCENES[i].name, name) == 0) return &CONF_SCENES[i];
    }
    return NULL;
}

/* Prints the first line the two reports disagree on - a mismatched digest
 * says "the geometry moved", a mismatched node line says where. */
static void conf_diff(const char *want, const char *got) {
    const char *a = want, *b = got;
    int line = 1;
    while (*a && *b) {
        const char *ae = strchr(a, '\n'), *be = strchr(b, '\n');
        size_t alen = ae ? (size_t)(ae - a) : strlen(a);
        size_t blen = be ? (size_t)(be - b) : strlen(b);
        if (alen != blen || memcmp(a, b, alen) != 0) {
            fprintf(stderr, "  line %d:\n    reference: %.*s\n    C:         %.*s\n",
                    line, (int)alen, a, (int)blen, b);
            return;
        }
        if (!ae || !be) break;
        a = ae + 1;
        b = be + 1;
        line++;
    }
    fprintf(stderr, "  the reports differ in length (reference %zu bytes, C %zu)\n",
            strlen(want), strlen(got));
}

static int conformance(const char *path) {
    char *text = read_file(path);
    if (!text) {
        fprintf(stderr,
                "conformance: cannot read %s\n"
                "  generate it with: cargo run -p kui-core --features conformance --example conformance-dump -- %s\n",
                path, path);
        return 1;
    }

    int scenes = 0, bad = 0;
    char *cursor = text;
    while ((cursor = strstr(cursor, "scene ")) != NULL) {
        /* A block runs from its "scene " line to the "end\n" that closes it. */
        char *stop = strstr(cursor, "\nend\n");
        if (!stop) break;
        size_t block_len = (size_t)(stop - cursor) + 5;
        char *block = malloc(block_len + 1);
        memcpy(block, cursor, block_len);
        block[block_len] = 0;
        cursor += block_len;

        char name[64] = {0};
        sscanf(block, "scene %63s", name);
        ConfStep steps[CONF_MAX_STEPS];
        int nsteps = 0;
        ConfEnv env = {0};
        for (char *line = block; line; ) {
            char *next = strchr(line, '\n');
            if (strncmp(line, "env ", 4) == 0) {
                env.set = 1;
                sscanf(line, "env %d %d %d %d %d", &env.custom_chrome,
                       &env.maximized, &env.fullscreen, &env.controls_w,
                       &env.controls_h);
            }
            if (strncmp(line, "step ", 5) == 0) {
                /* Dropping a step silently would replay a different scene
                 * and blame the difference on the lowering. */
                if (nsteps == CONF_MAX_STEPS) {
                    fprintf(stderr, "conformance: scene '%s' replays more than %d steps\n",
                            name, CONF_MAX_STEPS);
                    exit(1);
                }
                ConfStep *s = &steps[nsteps++];
                s->a = s->b = 0;
                s->args = sscanf(line, "step %15s %d %d", s->kind, &s->a, &s->b) - 1;
            }
            line = next ? next + 1 : NULL;
        }

        const ConfScene *scene = conf_find(name);
        if (!scene) {
            fprintf(stderr, "FAIL: no C scene for '%s' - every corpus scene needs one\n", name);
            bad++;
        } else {
            Rep got;
            rep_init(&got);
            conf_run(scene, &env, steps, nsteps, &got);
            if (strcmp(got.buf, block) != 0) {
                fprintf(stderr, "FAIL: scene '%s' lowers differently from C than from kui-core\n", name);
                conf_diff(block, got.buf);
                bad++;
            }
            rep_free(&got);
        }
        scenes++;
        free(block);
    }
    free(text);

    if (scenes == 0) {
        fprintf(stderr, "conformance: %s holds no scenes\n", path);
        return 1;
    }
    size_t known = sizeof CONF_SCENES / sizeof *CONF_SCENES;
    if ((size_t)scenes != known) {
        fprintf(stderr, "FAIL: the reference has %d scenes, C builds %zu\n", scenes, known);
        bad++;
    }
    if (bad) {
        fprintf(stderr, "conformance: %d of %d scene(s) failed\n", bad, scenes);
        return 1;
    }
    printf("conformance OK (%d scenes)\n", scenes);
    return 0;
}

/* The first thing a C host should do, before it allocates a context or a
 * struct the library will write into.
 *
 * This file and libkui_ffi are settled against each other by build.sh, but
 * that only covers the pair you built. Ship the binary, let it load a
 * libkui_ffi from somewhere else, and the two can disagree - and the way
 * they disagree is not a missing symbol. KuiEvent is caller-allocated: main
 * reserves sizeof(KuiEvent) as this header declares it, and a newer library
 * that appended a field would write past the end of that. The `size` in
 * KUI_EVENT_INIT stops exactly that one, but only for [out] structs; the
 * arrays below (KuiAccessNode, KuiWarning) and the KuiQuad array we stride
 * through KuiDrawData have no in-band guard at all, and this check is what
 * stands in for one. Equality, not >=, for the reason kui.h gives. */
static int abi_ok(void) {
    uint32_t lib = kui_abi_version();
    if (lib == KUI_ABI_VERSION) return 1;
    fprintf(stderr,
            "FAIL: libkui_ffi implements ABI %u, this binary was built "
            "against ABI %u.\n"
            "      Rebuild against the matching kui.h, or link the matching "
            "library.\n",
            lib, (unsigned)KUI_ABI_VERSION);
    return 0;
}

int main(int argc, char **argv) {
    if (!abi_ok()) {
        return 1;
    }
    if (argc > 1 && strcmp(argv[1], "--headless") == 0) {
        return headless() || surface();
    }
    if (argc > 1 && strcmp(argv[1], "--conformance") == 0) {
        return conformance(argc > 2 ? argv[2] : "target/conformance.txt");
    }
    AppState state = {0};
    return kui_run(KUI_STR("kui — C counter"), view, on_event, &state) ? 0 : 1;
}
