/* kui from C: a counter app driven entirely through the C API.
 *
 *   ./build.sh          # builds libkui_ffi + this file
 *   ./counter           # opens a window (winit + wgpu under the hood)
 *   ./counter --headless  # no window: builds a frame, simulates a click,
 *                         # verifies the event round-trip, prints draw stats,
 *                         # then walks the rest of the header (see surface())
 */
#include <stdio.h>
#include <string.h>
#include "kui.h"

typedef struct AppState {
    long long count;
} AppState;

/* -- view: rebuild the whole tree from state, every frame ---------------- */

static void view(void *user, KuiCtx *ui) {
    AppState *state = (AppState *)user;

    KuiSpec root = {
        .width = {KUI_GROW, 1}, .height = {KUI_GROW, 1},
        .main_align = KUI_CENTER, .cross_align = KUI_CENTER, .gap = 24,
    };
    kui_root(ui, &root);

    KuiSpec card = {
        .dir = KUI_COLUMN, .gap = 20, .cross_align = KUI_CENTER,
        .pad_l = 32, .pad_r = 32, .pad_t = 32, .pad_b = 32,
        .bg = 0x161820ff, .radius = 12,
        .border_w = 1, .border_color = 0x2a2d3aff,
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
}

/* -- events: clicks arrive as data --------------------------------------- */

static void apply_event(AppState *state, const KuiEvent *ev) {
    if (!ev->payload) return;
    const KuiValue *kind = kui_value_get(ev->payload, KUI_STR("kind"));
    KuiStr s;
    if (!kind || !kui_value_as_str(kind, &s)) return;
    if (s.len == 3 && memcmp(s.ptr, "inc", 3) == 0) state->count++;
    if (s.len == 3 && memcmp(s.ptr, "dec", 3) == 0) state->count--;
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

    KuiDrawData dd;
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

    KuiEvent ev;
    int got = 0;
    while (kui_poll_event(ctx, &ev)) {
        apply_event(&state, &ev);
        got++;
    }
    printf("clicked (%.0f, %.0f): %d event(s), count = %lld\n", bx, by, got, state.count);

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
    kui_env_set_window(ui, true, false, false, 78, 28);
    kui_set_subpixel_text(ui, false);

    /* Measurement works before the first frame. */
    KuiTextStyle body = {.size = 16};
    KuiTextMetrics one = {0}, wrapped = {0}, rich = {0};
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

    KuiDrawData dd;
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

    /* The key sink is focusable and takes focus like anything else. Its
     * {kind="key"} events cannot be driven from here: kui_input_key carries
     * the editing keys (KUI_KEY_*), and raw presses only reach a sink
     * through kui_run's own event loop. */
    kui_focus(ui, k.sink);
    check(kui_is_focused(ui, k.sink), "kui_is_focused");
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
    uint32_t wcmds[8];
    kui_take_window_commands(ui, wcmds, sizeof wcmds / sizeof wcmds[0]);

    /* Everything above lands as data. */
    KuiEvent ev;
    int events = 0, layouts = 0, access = 0;
    while (kui_poll_event(ui, &ev)) {
        events++;
        if (!ev.payload) continue;
        const KuiValue *kind = kui_value_get(ev.payload, KUI_STR("kind"));
        KuiStr s;
        if (!kind || !kui_value_as_str(kind, &s)) continue;
        if (s.len == 6 && memcmp(s.ptr, "layout", 6) == 0) layouts++;
        if (s.len == 6 && memcmp(s.ptr, "access", 6) == 0) access++;
    }
    check(events > 0, "the inputs produced events");
    check(layouts > 0, "on_layout reported the card's rect");
    check(access > 0, "the slider nudge arrived as an access event");

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

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--headless") == 0) {
        return headless() || surface();
    }
    AppState state = {0};
    return kui_run(KUI_STR("kui — C counter"), view, on_event, &state) ? 0 : 1;
}
